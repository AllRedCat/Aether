//! Lookahead Ring Buffer, Audio Master Clock (A/V Sync), and Frame Dropper
//! for high-performance preview player playback.

use std::collections::VecDeque;
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

use aether_core::timeline::Rational;

use crate::decoder::types::VideoFrame;

/// Default capacity for the lookahead ring buffer (16 decoded RGBA8 frames).
pub const DEFAULT_LOOKAHEAD_CAPACITY: usize = 16;
/// Minimum capacity for lookahead buffer (8 frames).
pub const MIN_LOOKAHEAD_CAPACITY: usize = 8;

/// Error conditions during ring buffer operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferError {
    /// The ring buffer was closed/terminated.
    Closed,
    /// The frame being pushed has a stale seek generation/epoch.
    StaleEpoch,
    /// Timed out waiting for available buffer space.
    Timeout,
}

/// Thread-safe bounded ring buffer holding decoded lookahead video frames.
pub struct LookaheadRingBuffer {
    capacity: usize,
    state: Mutex<RingBufferState>,
    not_full: Condvar,
    not_empty: Condvar,
}

struct RingBufferState {
    queue: VecDeque<VideoFrame>,
    epoch: u64,
    closed: bool,
    eos_reached: bool,
    dropped_frames: u64,
}

impl LookaheadRingBuffer {
    /// Creates a new `LookaheadRingBuffer` with the specified frame capacity.
    pub fn new(capacity: usize) -> Self {
        let cap = capacity.clamp(MIN_LOOKAHEAD_CAPACITY, 64);
        Self {
            capacity: cap,
            state: Mutex::new(RingBufferState {
                queue: VecDeque::with_capacity(cap),
                epoch: 0,
                closed: false,
                eos_reached: false,
                dropped_frames: 0,
            }),
            not_full: Condvar::new(),
            not_empty: Condvar::new(),
        }
    }

    /// Maximum capacity of the ring buffer.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Number of frames currently buffered.
    #[inline]
    pub fn len(&self) -> usize {
        self.state.lock().unwrap().queue.len()
    }

    /// Returns `true` if no frames are currently queued.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.state.lock().unwrap().queue.is_empty()
    }

    /// Returns `true` if the buffer is currently at full capacity.
    #[inline]
    pub fn is_full(&self) -> bool {
        let st = self.state.lock().unwrap();
        st.queue.len() >= self.capacity
    }

    /// Current seek epoch identifier.
    #[inline]
    pub fn current_epoch(&self) -> u64 {
        self.state.lock().unwrap().epoch
    }

    /// Returns `true` if end of stream has been flagged for the current epoch.
    #[inline]
    pub fn is_eos(&self) -> bool {
        self.state.lock().unwrap().eos_reached
    }

    /// Flags whether the current epoch reached end of stream.
    pub fn set_eos(&self, epoch: u64, eos: bool) {
        let mut st = self.state.lock().unwrap();
        if st.epoch == epoch {
            st.eos_reached = eos;
            self.not_empty.notify_all();
        }
    }

    /// Pushes a decoded frame into the back of the buffer.
    ///
    /// If full, blocks until space is made available or until the timeout expires.
    /// Aborts immediately if the epoch changes or the buffer is closed.
    pub fn push_wait(
        &self,
        frame: VideoFrame,
        epoch: u64,
        timeout: Option<Duration>,
    ) -> Result<(), BufferError> {
        let mut st = self.state.lock().unwrap();
        loop {
            if st.closed {
                return Err(BufferError::Closed);
            }
            if st.epoch != epoch {
                return Err(BufferError::StaleEpoch);
            }
            if st.queue.len() < self.capacity {
                st.queue.push_back(frame);
                self.not_empty.notify_one();
                return Ok(());
            }

            if let Some(dur) = timeout {
                let (new_st, timeout_res) = self.not_full.wait_timeout(st, dur).unwrap();
                st = new_st;
                if timeout_res.timed_out() {
                    if st.closed {
                        return Err(BufferError::Closed);
                    }
                    if st.epoch != epoch {
                        return Err(BufferError::StaleEpoch);
                    }
                    if st.queue.len() < self.capacity {
                        st.queue.push_back(frame);
                        self.not_empty.notify_one();
                        return Ok(());
                    }
                    return Err(BufferError::Timeout);
                }
            } else {
                st = self.not_full.wait(st).unwrap();
            }
        }
    }

    /// Non-blocking peek of presentation timestamp (PTS) and duration of the front frame.
    ///
    /// Returns `None` if the queue is empty. Avoids copying pixel buffers.
    pub fn peek_pts_and_duration(&self) -> Option<(i64, i64)> {
        let st = self.state.lock().unwrap();
        st.queue.front().map(|f| (f.pts, f.duration_pts))
    }

    /// Non-blocking peek of the front frame `(pts, duration_pts)` plus the PTS of the
    /// frame queued right behind it (if any). Avoids copying pixel buffers.
    pub fn peek_front_with_successor(&self) -> Option<(i64, i64, Option<i64>)> {
        let st = self.state.lock().unwrap();
        st.queue
            .front()
            .map(|f| (f.pts, f.duration_pts, st.queue.get(1).map(|n| n.pts)))
    }

    /// Pops the front frame from the queue, signaling waiting producers.
    pub fn pop_front(&self) -> Option<VideoFrame> {
        let mut st = self.state.lock().unwrap();
        let frame = st.queue.pop_front();
        if frame.is_some() {
            self.not_full.notify_one();
        }
        frame
    }

    /// Drops the front frame without copying, incrementing `dropped_frames` counter.
    ///
    /// Returns `true` if a frame was dropped, or `false` if the buffer was empty.
    pub fn drop_front(&self) -> bool {
        let mut st = self.state.lock().unwrap();
        if st.queue.pop_front().is_some() {
            st.dropped_frames += 1;
            self.not_full.notify_one();
            true
        } else {
            false
        }
    }

    /// Flushes all queued frames and increments/updates the seek epoch.
    pub fn flush_and_set_epoch(&self, new_epoch: u64) {
        let mut st = self.state.lock().unwrap();
        st.queue.clear();
        st.epoch = new_epoch;
        st.eos_reached = false;
        self.not_full.notify_all();
        self.not_empty.notify_all();
    }

    /// Closes the buffer, waking up all waiting producers and consumers.
    pub fn close(&self) {
        let mut st = self.state.lock().unwrap();
        st.closed = true;
        self.not_full.notify_all();
        self.not_empty.notify_all();
    }

    /// Total count of frames dropped by frame dropping logic.
    #[inline]
    pub fn dropped_frames(&self) -> u64 {
        self.state.lock().unwrap().dropped_frames
    }
}

/// Master Clock synchronizing A/V playback against audio stream presentation timestamps,
/// with high-resolution monotonic system clock fallback.
#[derive(Debug)]
pub struct MasterClock {
    timebase: Rational,
    state: Mutex<ClockInner>,
}

#[derive(Debug)]
struct ClockInner {
    anchor_pts: i64,
    anchor_instant: Option<Instant>,
    last_audio_pts: Option<i64>,
    is_running: bool,
}

impl MasterClock {
    /// Creates a new `MasterClock` configured for the specified timeline timebase.
    pub fn new(timebase: Rational) -> Self {
        Self {
            timebase,
            state: Mutex::new(ClockInner {
                anchor_pts: 0,
                anchor_instant: None,
                last_audio_pts: None,
                is_running: false,
            }),
        }
    }

    /// Associated stream timebase.
    #[inline]
    pub fn timebase(&self) -> Rational {
        self.timebase
    }

    /// Frames-per-second derived from timebase.
    #[inline]
    pub fn fps(&self) -> f64 {
        if self.timebase.den != 0 && self.timebase.num > 0 {
            self.timebase.num as f64 / self.timebase.den as f64
        } else {
            60.0
        }
    }

    /// Starts running the master clock from the specified PTS anchor.
    pub fn start(&self, from_pts: i64) {
        let mut st = self.state.lock().unwrap();
        st.anchor_pts = from_pts;
        st.anchor_instant = Some(Instant::now());
        st.is_running = true;
    }

    /// Pauses the master clock and returns the instantaneous PTS at the time of pause.
    pub fn pause(&self) -> i64 {
        let mut st = self.state.lock().unwrap();
        let pts = self.calc_pts(&st);
        st.anchor_pts = pts;
        st.anchor_instant = None;
        st.is_running = false;
        pts
    }

    /// Seeks the master clock to a new PTS anchor.
    pub fn seek(&self, target_pts: i64) {
        let mut st = self.state.lock().unwrap();
        st.anchor_pts = target_pts;
        st.last_audio_pts = None;
        if st.is_running {
            st.anchor_instant = Some(Instant::now());
        }
    }

    /// Updates the master clock with a newly decoded audio buffer presentation timestamp.
    ///
    /// The audio stream acts as the absolute master clock: video presentation synchronizes
    /// to these PTS updates.
    pub fn update_audio_pts(&self, audio_pts: i64) {
        let mut st = self.state.lock().unwrap();
        st.last_audio_pts = Some(audio_pts);
        st.anchor_pts = audio_pts;
        if st.is_running {
            st.anchor_instant = Some(Instant::now());
        }
    }

    /// Returns the current instantaneous presentation timestamp according to the master clock.
    pub fn current_pts(&self) -> i64 {
        let st = self.state.lock().unwrap();
        self.calc_pts(&st)
    }

    /// Returns `true` if the clock is actively progressing.
    pub fn is_running(&self) -> bool {
        self.state.lock().unwrap().is_running
    }

    fn calc_pts(&self, st: &ClockInner) -> i64 {
        if !st.is_running {
            return st.anchor_pts;
        }
        if let Some(inst) = st.anchor_instant {
            let elapsed_secs = inst.elapsed().as_secs_f64();
            let elapsed_pts = (elapsed_secs * self.fps()).round() as i64;
            st.anchor_pts.saturating_add(elapsed_pts)
        } else {
            st.anchor_pts
        }
    }
}

/// Action determined by the frame dropping evaluator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameAction {
    /// The frame is late: the playhead has reached or exceeded frame N+1 before frame N
    /// was presented. Drop frame N immediately to avoid cumulative drift.
    Drop,
    /// The frame is on time: due for presentation in the current display interval.
    Present,
    /// The frame is in the future: presentation should wait until its PTS arrives.
    Wait { pts_ahead: i64 },
}

/// Evaluates frame presentation timing against the master clock to enforce A/V sync.
pub struct FrameDropper;

impl FrameDropper {
    /// Default maximum allowed late frame multiplier before authorizing a frame drop (2x frame duration).
    pub const DEFAULT_MAX_LATE_MULTIPLIER: i64 = 2;

    /// Evaluates whether the front frame should be dropped, presented, or waited on,
    /// applying hysteresis tolerance to preserve visual smoothness.
    ///
    /// # Hysteresis Rules:
    /// - **Aguardar (`Wait`):** Se o quadro estiver no futuro (`master_pts < front_frame_pts`),
    ///   aguarda a diferença respeitando o framerate.
    /// - **Apresentar Imediatamente (`Present`):** Se o quadro estiver no tempo exato ou
    ///   ligeiramente atrasado dentro de um limiar tolerável (`delay <= 2 * duration`),
    ///   o frame é apresentado imediatamente para manter a fluidez visual sem engasgos.
    /// - **Descarte (`Drop`):** Apenas descarta se o quadro estiver criticamente atrasado
    ///   por mais de 2x a duração de um frame (`master_pts > front_frame_pts + 2 * duration`),
    ///   evitando atraso acumulado sob sobrecarga severa.
    #[inline]
    pub fn evaluate(
        front_frame_pts: i64,
        front_frame_duration: i64,
        master_pts: i64,
    ) -> FrameAction {
        Self::evaluate_with_tolerance(
            front_frame_pts,
            front_frame_duration,
            master_pts,
            Self::DEFAULT_MAX_LATE_MULTIPLIER,
        )
    }

    /// Evaluates frame timing with an explicit late frame multiplier.
    #[inline]
    pub fn evaluate_with_tolerance(
        front_frame_pts: i64,
        front_frame_duration: i64,
        master_pts: i64,
        max_late_multiplier: i64,
    ) -> FrameAction {
        let duration = front_frame_duration.max(1);
        let multiplier = max_late_multiplier.max(1);
        let drop_threshold = front_frame_pts.saturating_add(duration.saturating_mul(multiplier));

        if master_pts < front_frame_pts {
            FrameAction::Wait {
                pts_ahead: front_frame_pts.saturating_sub(master_pts),
            }
        } else if master_pts <= drop_threshold {
            FrameAction::Present
        } else {
            FrameAction::Drop
        }
    }

    /// Successor-aware evaluation used by the realtime emitter.
    ///
    /// A late frame is dropped **only** if the frame queued right behind it is
    /// itself already due (`master_pts >= next_pts`): skipping is then free, since a
    /// newer image can be shown immediately. When the late frame is the only one
    /// available (decoder behind the playhead), it is presented instead of being
    /// discarded — dropping it would leave the screen frozen on the previous image,
    /// which is exactly the cascade observed when the decoder cannot keep up.
    #[inline]
    pub fn evaluate_with_successor(
        front_frame_pts: i64,
        next_frame_pts: Option<i64>,
        master_pts: i64,
    ) -> FrameAction {
        if master_pts < front_frame_pts {
            return FrameAction::Wait {
                pts_ahead: front_frame_pts.saturating_sub(master_pts),
            };
        }
        match next_frame_pts {
            Some(next_pts) if next_pts > front_frame_pts && master_pts >= next_pts => {
                FrameAction::Drop
            }
            _ => FrameAction::Present,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decoder::types::PixelFormat;
    use std::thread;

    fn make_test_frame(pts: i64, duration_pts: i64) -> VideoFrame {
        VideoFrame::new(
            16,
            16,
            PixelFormat::Rgba8,
            vec![255u8; 16 * 16 * 4],
            pts,
            duration_pts,
            Rational { num: 60, den: 1 },
        )
    }

    #[test]
    fn test_lookahead_ring_buffer_push_pop_capacity() {
        let buffer = LookaheadRingBuffer::new(8);
        assert_eq!(buffer.capacity(), 8);
        assert_eq!(buffer.len(), 0);
        assert!(buffer.is_empty());
        assert!(!buffer.is_full());

        for i in 0..8 {
            let res = buffer.push_wait(make_test_frame(i, 1), 0, None);
            assert_eq!(res, Ok(()));
        }

        assert_eq!(buffer.len(), 8);
        assert!(buffer.is_full());

        // Peek does not remove
        let peek = buffer.peek_pts_and_duration();
        assert_eq!(peek, Some((0, 1)));
        assert_eq!(buffer.len(), 8);

        // Pop front
        let popped = buffer.pop_front().expect("Frame expected");
        assert_eq!(popped.pts, 0);
        assert_eq!(buffer.len(), 7);
        assert!(!buffer.is_full());

        // Can push again now
        assert_eq!(buffer.push_wait(make_test_frame(8, 1), 0, None), Ok(()));
        assert_eq!(buffer.len(), 8);
    }

    #[test]
    fn test_lookahead_ring_buffer_flush_and_epoch() {
        let buffer = LookaheadRingBuffer::new(8);
        buffer.push_wait(make_test_frame(10, 1), 0, None).unwrap();
        assert_eq!(buffer.len(), 1);

        // Flush and increment epoch
        buffer.flush_and_set_epoch(1);
        assert_eq!(buffer.len(), 0);
        assert_eq!(buffer.current_epoch(), 1);

        // Push with old epoch fails with StaleEpoch
        let stale_res = buffer.push_wait(make_test_frame(11, 1), 0, None);
        assert_eq!(stale_res, Err(BufferError::StaleEpoch));
        assert_eq!(buffer.len(), 0);

        // Push with new epoch succeeds
        let valid_res = buffer.push_wait(make_test_frame(12, 1), 1, None);
        assert_eq!(valid_res, Ok(()));
        assert_eq!(buffer.len(), 1);
    }

    #[test]
    fn test_lookahead_ring_buffer_drop_front() {
        let buffer = LookaheadRingBuffer::new(8);
        buffer.push_wait(make_test_frame(1, 1), 0, None).unwrap();
        buffer.push_wait(make_test_frame(2, 1), 0, None).unwrap();

        assert_eq!(buffer.dropped_frames(), 0);
        assert!(buffer.drop_front());
        assert_eq!(buffer.dropped_frames(), 1);
        assert_eq!(buffer.len(), 1);

        let peek = buffer.peek_pts_and_duration();
        assert_eq!(peek, Some((2, 1)));
    }

    #[test]
    fn test_master_clock_monotonic_and_pause_resume() {
        let clock = MasterClock::new(Rational { num: 60, den: 1 });
        assert_eq!(clock.current_pts(), 0);
        assert!(!clock.is_running());

        clock.start(100);
        assert!(clock.is_running());

        thread::sleep(Duration::from_millis(50));
        let pts_adv = clock.current_pts();
        assert!(pts_adv >= 102, "Clock should advance PTS: got {}", pts_adv);

        let paused_pts = clock.pause();
        assert!(!clock.is_running());
        assert_eq!(clock.current_pts(), paused_pts);

        thread::sleep(Duration::from_millis(50));
        assert_eq!(clock.current_pts(), paused_pts, "PTS must not advance while paused");

        clock.start(paused_pts);
        thread::sleep(Duration::from_millis(50));
        assert!(clock.current_pts() > paused_pts);
    }

    #[test]
    fn test_master_clock_audio_pts_sync() {
        let clock = MasterClock::new(Rational { num: 60, den: 1 });
        clock.start(0);

        // Audio clock update re-anchors master clock
        clock.update_audio_pts(500);
        assert!(clock.current_pts() >= 500);

        clock.seek(1000);
        assert!(clock.current_pts() >= 1000);
    }

    #[test]
    fn test_frame_dropper_decisions() {
        // Frame PTS: 10, Duration: 2.
        // Tolerância padrão: 2x duração -> drop_threshold = 10 + 2 * 2 = 14.

        // 1. No tempo exato (on-time): PTS do relógio coincide ou está no primeiro intervalo do frame
        assert_eq!(FrameDropper::evaluate(10, 2, 10), FrameAction::Present);
        assert_eq!(FrameDropper::evaluate(10, 2, 11), FrameAction::Present);

        // 2. Ligeiramente atrasado (dentro da histerese tolerada: <= 2 * duration):
        // 1 frame de atraso (master_pts = 12) e 2 frames de atraso (master_pts = 14) são apresentados
        // imediatamente para preservar fluidez visual sem engasgos ou quedas em cascata.
        assert_eq!(FrameDropper::evaluate(10, 2, 12), FrameAction::Present);
        assert_eq!(FrameDropper::evaluate(10, 2, 13), FrameAction::Present);
        assert_eq!(FrameDropper::evaluate(10, 2, 14), FrameAction::Present);

        // 3. Criticamente atrasado (> 2 frames de atraso):
        // Quando o atraso ultrapassa 2x a duração do frame (master_pts >= 15), descarta o frame
        // para recuperar a sincronia e evitar acúmulo de latência.
        assert_eq!(FrameDropper::evaluate(10, 2, 15), FrameAction::Drop);
        assert_eq!(FrameDropper::evaluate(10, 2, 25), FrameAction::Drop);

        // 4. Adiantado (no futuro em relação ao relógio mestre):
        // Quando master_pts < front_frame_pts, aguarda até o tempo do quadro.
        assert_eq!(
            FrameDropper::evaluate(10, 2, 7),
            FrameAction::Wait { pts_ahead: 3 }
        );

        // Teste explícito com tolerância customizada (ex: multiplicador 1)
        assert_eq!(
            FrameDropper::evaluate_with_tolerance(10, 2, 12, 1),
            FrameAction::Present
        );
        assert_eq!(
            FrameDropper::evaluate_with_tolerance(10, 2, 13, 1),
            FrameAction::Drop
        );
    }

    #[test]
    fn test_frame_dropper_never_starves_display() {
        // Ahead -> Wait
        assert_eq!(
            FrameDropper::evaluate_with_successor(10, Some(12), 7),
            FrameAction::Wait { pts_ahead: 3 }
        );
        // On time -> Present
        assert_eq!(
            FrameDropper::evaluate_with_successor(10, Some(12), 10),
            FrameAction::Present
        );
        // Late but successor not due yet -> Present
        assert_eq!(
            FrameDropper::evaluate_with_successor(10, Some(12), 11),
            FrameAction::Present
        );
        // Successor already due -> Drop current (newer image is available)
        assert_eq!(
            FrameDropper::evaluate_with_successor(10, Some(12), 12),
            FrameAction::Drop
        );
        // Critically late but it is the only decoded frame -> must Present, never freeze
        assert_eq!(
            FrameDropper::evaluate_with_successor(2, None, 271),
            FrameAction::Present
        );
    }
}

