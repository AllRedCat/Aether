use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, LazyLock, Mutex, RwLock};
use std::thread;
use std::time::{Duration, Instant};

use aether_core::media::MediaType;
use aether_core::timeline::Rational;
use aether_media::decoder::traits::MediaDecoder;
use aether_media::frame_index::FrameIndex;
use aether_media::playback::{
    FrameAction, FrameDropper, LookaheadRingBuffer, MasterClock, DEFAULT_LOOKAHEAD_CAPACITY,
};
use aether_media::MmapFrameIndex;

use crate::api::{BridgeFrame, PlaybackState, PreviewSessionInfo};
pub use crate::frb_generated::StreamSink;

/// Internal states of the video decoding worker distinguishing idle, scrubbing, and linear playback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecoderState {
    /// Decoder worker is idle or paused.
    Idle,
    /// Decoder worker is seeking / scrubbing to target timestamp.
    Seeking(i64),
    /// Decoder worker is actively streaming sequential frames (linear playback).
    Streaming,
}

pub struct PendingSeekRequest {
    pub target_pts: i64,
    pub epoch: u64,
    pub responders: Vec<std::sync::mpsc::SyncSender<Result<Option<BridgeFrame>, String>>>,
}

pub struct DecoderWorkerControl {
    pub state: DecoderState,
    pub pending_seek: Option<PendingSeekRequest>,
    pub play_pending: bool,
    pub pause_pending: bool,
}

/// Status emitted during sequential playback ticker execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackTickStatus {
    FrameDecoded,
    AudioBufferDecoded,
    EndOfStream,
}

/// Represents an active media preview playback session featuring a decoupled
/// lookahead ring buffer, audio master clock synchronization, and frame dropping.
pub struct PlaybackSession {
    pub session_id: String,
    pub texture_id: i64,
    pub file_path: String,
    pub width: u32,
    pub height: u32,
    pub duration_pts: i64,
    pub duration_seconds: f64,
    pub fps: f64,
    pub timebase: Rational,
    pub media_type: MediaType,

    pub is_playing: Arc<AtomicBool>,
    pub current_pts: Arc<AtomicI64>,
    pub lookahead_buffer: Arc<LookaheadRingBuffer>,
    pub master_clock: Arc<MasterClock>,
    pub frame_index: Option<Arc<FrameIndex>>,
    pub mmap_frame_index: Option<Arc<MmapFrameIndex>>,

    pub seek_count: Arc<AtomicUsize>,
    pub sequential_frame_count: Arc<AtomicUsize>,

    current_epoch: AtomicU64,
    worker_shutdown: Arc<AtomicBool>,
    decoder: Arc<Mutex<Box<dyn MediaDecoder>>>,
    frame_sinks: Mutex<Vec<StreamSink<BridgeFrame>>>,
    state_sinks: Mutex<Vec<StreamSink<PlaybackState>>>,
    last_frame: Mutex<Option<BridgeFrame>>,

    decoder_control: Arc<Mutex<DecoderWorkerControl>>,
    decoder_condvar: Arc<Condvar>,
    pub frame_dropper: FrameDropper,
}

impl PlaybackSession {
    /// Returns the session metadata descriptor.
    pub fn info(&self) -> PreviewSessionInfo {
        PreviewSessionInfo {
            session_id: self.session_id.clone(),
            texture_id: self.texture_id,
            width: self.width,
            height: self.height,
            duration_pts: self.duration_pts,
            duration_seconds: self.duration_seconds,
            fps: self.fps,
            timebase: self.timebase,
        }
    }

    /// Returns the instantaneous playback state.
    pub fn get_state(&self) -> PlaybackState {
        let current_pts = self.current_pts.load(Ordering::SeqCst);
        let timeline_fps = if self.timebase.den != 0 && self.timebase.num > 0 {
            self.timebase.num as f64 / self.timebase.den as f64
        } else if self.fps > 0.0 {
            self.fps
        } else {
            60.0
        };
        let current_seconds = if timeline_fps > 0.0 {
            current_pts as f64 / timeline_fps
        } else {
            0.0
        };
        PlaybackState {
            session_id: self.session_id.clone(),
            is_playing: self.is_playing.load(Ordering::SeqCst),
            current_pts,
            current_seconds,
            duration_pts: self.duration_pts,
            duration_seconds: self.duration_seconds,
        }
    }

    /// Returns a copy of the most recently decoded frame, if available.
    pub fn get_last_frame(&self) -> Option<BridgeFrame> {
        self.last_frame.lock().ok().and_then(|g| g.clone())
    }

    /// Returns total count of frames dropped by the presentation engine due to deadlines.
    pub fn dropped_frames(&self) -> u64 {
        self.lookahead_buffer.dropped_frames()
    }

    /// Subscribes a new StreamSink for video frames.
    /// Emits the cached initial/last frame immediately so the UI is responsive.
    pub fn subscribe_frames(&self, sink: StreamSink<BridgeFrame>) {
        if let Some(frame) = self.get_last_frame() {
            let _ = sink.add(frame);
        }
        if let Ok(mut sinks) = self.frame_sinks.lock() {
            sinks.push(sink);
        }
    }

    /// Subscribes a new StreamSink for playback state events.
    /// Emits the current state immediately upon subscription.
    pub fn subscribe_state(&self, sink: StreamSink<PlaybackState>) {
        let state = self.get_state();
        let _ = sink.add(state);
        if let Ok(mut sinks) = self.state_sinks.lock() {
            sinks.push(sink);
        }
    }

    /// Broadcasts a decoded video frame to all active sinks, dropping any disconnected sinks.
    pub fn broadcast_frame(&self, frame: &BridgeFrame) {
        if let Ok(mut sinks) = self.frame_sinks.lock() {
            sinks.retain(|sink| sink.add(frame.clone()).is_ok());
        }
    }

    /// Broadcasts current playback state to all active sinks, dropping any disconnected sinks.
    pub fn broadcast_state(&self) {
        let state = self.get_state();
        if let Ok(mut sinks) = self.state_sinks.lock() {
            sinks.retain(|sink| sink.add(state.clone()).is_ok());
        }
    }

    /// Total count of decoder seeks performed during the session lifecycle.
    pub fn seek_count(&self) -> usize {
        self.seek_count.load(Ordering::SeqCst)
    }

    /// Total count of sequential frames decoded without seeks.
    pub fn sequential_frame_count(&self) -> usize {
        self.sequential_frame_count.load(Ordering::SeqCst)
    }

    /// Current state of the decoder worker.
    pub fn decoder_state(&self) -> DecoderState {
        self.decoder_control.lock().unwrap().state
    }

    /// Starts sequential playback governed by the master clock.
    pub fn play(self: &Arc<Self>) -> Result<(), String> {
        // If at or beyond EOS, restart from beginning
        if self.current_pts.load(Ordering::SeqCst) >= self.duration_pts {
            let _ = self.seek_pts(0);
        }

        if self.is_playing.swap(true, Ordering::SeqCst) {
            // Already playing
            return Ok(());
        }

        let cur = self.current_pts.load(Ordering::SeqCst);
        self.master_clock.start(cur);

        // Transition decoder worker to Streaming mode for linear playback
        {
            let mut ctrl = self.decoder_control.lock().unwrap();
            ctrl.play_pending = true;
            ctrl.pause_pending = false;
            if !matches!(ctrl.state, DecoderState::Seeking(_)) {
                ctrl.state = DecoderState::Streaming;
            }
            self.decoder_condvar.notify_one();
        }

        self.broadcast_state();
        Ok(())
    }

    /// Pauses sequential playback without killing background worker threads.
    pub fn pause(&self) -> Result<(), String> {
        self.is_playing.store(false, Ordering::SeqCst);
        let paused_pts = self.master_clock.pause();
        self.current_pts
            .store(paused_pts.clamp(0, self.duration_pts), Ordering::SeqCst);

        // Transition decoder worker to Idle / Paused
        {
            let mut ctrl = self.decoder_control.lock().unwrap();
            ctrl.pause_pending = true;
            ctrl.play_pending = false;
            if matches!(ctrl.state, DecoderState::Streaming) {
                ctrl.state = DecoderState::Idle;
            }
            self.decoder_condvar.notify_one();
        }

        self.broadcast_state();
        Ok(())
    }

    /// Spawns the long-running decoupled decoder and presentation threads.
    pub fn start_background_workers(self: &Arc<Self>) {
        self.spawn_decoder_worker();
        self.spawn_emitter_worker();
    }

    /// Background thread: state machine managing Idle, Seeking (Scrubbing with debouncing/coalescing),
    /// and Streaming (Linear Playback with strictly continuous sequential packet/frame reading).
    fn spawn_decoder_worker(self: &Arc<Self>) {
        let decoder_arc = Arc::clone(&self.decoder);
        let buffer_arc = Arc::clone(&self.lookahead_buffer);
        let shutdown = Arc::clone(&self.worker_shutdown);
        let control_arc = Arc::clone(&self.decoder_control);
        let condvar_arc = Arc::clone(&self.decoder_condvar);
        let is_playing = Arc::clone(&self.is_playing);
        let current_pts = Arc::clone(&self.current_pts);
        let seek_count = Arc::clone(&self.seek_count);
        let seq_count = Arc::clone(&self.sequential_frame_count);
        let mmap_fidx = self.mmap_frame_index.clone();
        let fidx = self.frame_index.clone();
        let session_weak = Arc::downgrade(self);
        let session_id = self.session_id.clone();
        let duration_pts = self.duration_pts;

        let _ = thread::Builder::new()
            .name(format!("aether-dec-{}", session_id))
            .spawn(move || {
                while !shutdown.load(Ordering::SeqCst) {
                    // Check if a seek command is queued or state transition requested
                    let maybe_seek = {
                        let mut ctrl = control_arc.lock().unwrap();
                        if let Some(seek) = ctrl.pending_seek.take() {
                            ctrl.state = DecoderState::Seeking(seek.target_pts);
                            Some(seek)
                        } else {
                            if ctrl.play_pending {
                                ctrl.play_pending = false;
                                ctrl.state = DecoderState::Streaming;
                            } else if ctrl.pause_pending {
                                ctrl.pause_pending = false;
                                ctrl.state = DecoderState::Idle;
                            }
                            None
                        }
                    };

                    // =========================================================
                    // 1. FLUXO DE SEEKING (Scrubbing / Random Access)
                    // =========================================================
                    if let Some(mut seek_req) = maybe_seek {
                        let mut target_pts = seek_req.target_pts.clamp(0, duration_pts);

                        // Coalescing check: if another seek arrived while taking the lock
                        {
                            let mut ctrl = control_arc.lock().unwrap();
                            if let Some(mut newer) = ctrl.pending_seek.take() {
                                seek_req.responders.append(&mut newer.responders);
                                target_pts = newer.target_pts.clamp(0, duration_pts);
                                ctrl.state = DecoderState::Seeking(target_pts);
                            }
                        }

                        // Query .index mapped via mmap for the nearest keyframe at or before target_pts
                        let keyframe_pts = if let Some(ref m_idx) = mmap_fidx {
                            m_idx.seek_keyframe(target_pts).map(|kf| kf.pts).unwrap_or(target_pts)
                        } else if let Some(ref idx) = fidx {
                            idx.seek_keyframe(target_pts).map(|kf| kf.pts).unwrap_or(target_pts)
                        } else {
                            target_pts
                        };

                        let seeks = seek_count.fetch_add(1, Ordering::SeqCst) + 1;
                        eprintln!(
                            "[DEBUG DEC] [Seeking] Seek command executed: target_pts={}, keyframe_pts={} (total_seeks={})",
                            target_pts, keyframe_pts, seeks
                        );

                        let mut dec = match decoder_arc.lock() {
                            Ok(g) => g,
                            Err(_) => break,
                        };

                        // Issue 1 seek command on container / decoder
                        let seek_res = dec.seek_pts(keyframe_pts);
                        if let Err(e) = seek_res {
                            eprintln!("[DEBUG DEC] Seek error: {:?}", e);
                            let err_msg = e.to_string();
                            for r in seek_req.responders {
                                let _ = r.send(Err(err_msg.clone()));
                            }
                            continue;
                        }

                        // Advance discarding intermediate frames until target_pts
                        let mut target_frame = None;
                        if dec.video().is_some() {
                            while let Ok(Some(mut vf)) = dec.next_video_frame() {
                                // While advancing, check if a newer seek request arrived (coalesce/debounce)!
                                let has_newer = {
                                    let mut ctrl = control_arc.lock().unwrap();
                                    if let Some(mut newer) = ctrl.pending_seek.take() {
                                        seek_req.responders.append(&mut newer.responders);
                                        target_pts = newer.target_pts.clamp(0, duration_pts);
                                        ctrl.state = DecoderState::Seeking(target_pts);
                                        true
                                    } else {
                                        false
                                    }
                                };

                                if has_newer {
                                    let newer_kf = if let Some(ref m_idx) = mmap_fidx {
                                        m_idx.seek_keyframe(target_pts).map(|kf| kf.pts).unwrap_or(target_pts)
                                    } else if let Some(ref idx) = fidx {
                                        idx.seek_keyframe(target_pts).map(|kf| kf.pts).unwrap_or(target_pts)
                                    } else {
                                        target_pts
                                    };
                                    if newer_kf != keyframe_pts {
                                        let seeks = seek_count.fetch_add(1, Ordering::SeqCst) + 1;
                                        eprintln!(
                                            "[DEBUG DEC] [Seeking Coalesced] Seek to newer keyframe_pts={} (total_seeks={})",
                                            newer_kf, seeks
                                        );
                                        let _ = dec.seek_pts(newer_kf);
                                    }
                                    continue;
                                }

                                let pts = vf.pts;
                                vf.ensure_rgba8();
                                target_frame = Some(vf);
                                if pts >= target_pts {
                                    break;
                                }
                            }

                            if target_frame.is_none() && keyframe_pts != target_pts {
                                let _ = dec.seek_pts(target_pts);
                                if let Ok(Some(mut vf)) = dec.next_video_frame() {
                                    vf.ensure_rgba8();
                                    target_frame = Some(vf);
                                }
                            }
                        }

                        drop(dec);

                        let bridge_frame = if let Some(vf) = target_frame {
                            if target_pts > 0 && target_pts < duration_pts {
                                current_pts.store(vf.pts, Ordering::SeqCst);
                            }
                            let timestamp_seconds = vf.timestamp_seconds();
                            let bf = BridgeFrame {
                                width: vf.width,
                                height: vf.height,
                                pts: vf.pts,
                                duration_pts: vf.duration_pts,
                                rgba_bytes: vf.data,
                                row_stride_bytes: vf.row_stride_bytes,
                                timestamp_seconds,
                            };
                            if let Some(session) = session_weak.upgrade() {
                                if let Ok(mut last) = session.last_frame.lock() {
                                    *last = Some(bf.clone());
                                }
                                session.broadcast_frame(&bf);
                                session.broadcast_state();
                            }
                            Some(bf)
                        } else {
                            None
                        };

                        // Deliver result frame to all waiting callers
                        for r in seek_req.responders {
                            let _ = r.send(Ok(bridge_frame.clone()));
                        }

                        // Transition state after seek:
                        {
                            let mut ctrl = control_arc.lock().unwrap();
                            if ctrl.pending_seek.is_some() {
                                continue;
                            } else if is_playing.load(Ordering::SeqCst) {
                                eprintln!("[DEBUG DEC] State changed: Seeking -> Streaming");
                                ctrl.state = DecoderState::Streaming;
                            } else {
                                eprintln!("[DEBUG DEC] State changed: Seeking -> Idle");
                                ctrl.state = DecoderState::Idle;
                            }
                        }
                        continue;
                    }

                    // Get current state
                    let current_state = {
                        control_arc.lock().unwrap().state
                    };

                    // =========================================================
                    // 2. FLUXO DE STREAMING (Linear Playback)
                    // =========================================================
                    if current_state == DecoderState::Streaming {
                        // In Streaming state, per-frame seeks are strictly prohibited.
                        let epoch = buffer_arc.current_epoch();

                        if buffer_arc.is_full() || buffer_arc.is_eos() {
                            thread::sleep(Duration::from_millis(4));
                            continue;
                        }

                        let mut dec = match decoder_arc.try_lock() {
                            Ok(g) => g,
                            Err(_) => {
                                thread::sleep(Duration::from_millis(2));
                                continue;
                            }
                        };

                        if buffer_arc.current_epoch() != epoch {
                            continue;
                        }

                        if dec.video().is_some() {
                            let decode_res = dec.next_video_frame();
                            drop(dec);

                            match decode_res {
                                Ok(Some(mut vf)) => {
                                    let seq = seq_count.fetch_add(1, Ordering::SeqCst) + 1;
                                    eprintln!(
                                        "[DEBUG DEC] [Streaming] Sequential frame decoded: PTS={} (total_sequential={})",
                                        vf.pts, seq
                                    );
                                    vf.ensure_rgba8();
                                    let _ = buffer_arc.push_wait(
                                        vf,
                                        epoch,
                                        Some(Duration::from_millis(50)),
                                    );
                                }
                                Ok(None) => {
                                    eprintln!("[DEBUG DEC] End of stream reached (None)");
                                    buffer_arc.set_eos(epoch, true);
                                    let mut ctrl = control_arc.lock().unwrap();
                                    ctrl.state = DecoderState::Idle;
                                }
                                Err(e) => {
                                    eprintln!("[DEBUG DEC] Decode error: {:?}", e);
                                    buffer_arc.set_eos(epoch, true);
                                    let mut ctrl = control_arc.lock().unwrap();
                                    ctrl.state = DecoderState::Idle;
                                }
                            }
                        } else {
                            drop(dec);
                            buffer_arc.set_eos(epoch, true);
                            thread::sleep(Duration::from_millis(20));
                        }
                        continue;
                    }

                    // =========================================================
                    // 3. FLUXO DE IDLE / PAUSED
                    // =========================================================
                    let epoch = buffer_arc.current_epoch();
                    if !buffer_arc.is_full() && !buffer_arc.is_eos() {
                        let mut dec = match decoder_arc.try_lock() {
                            Ok(g) => g,
                            Err(_) => {
                                thread::sleep(Duration::from_millis(2));
                                continue;
                            }
                        };
                        if buffer_arc.current_epoch() != epoch {
                            continue;
                        }
                        if dec.video().is_some() {
                            let decode_res = dec.next_video_frame();
                            drop(dec);
                            if let Ok(Some(mut vf)) = decode_res {
                                let seq = seq_count.fetch_add(1, Ordering::SeqCst) + 1;
                                eprintln!(
                                    "[DEBUG DEC] [Idle Pre-fill] Sequential frame decoded: PTS={} (total_sequential={})",
                                    vf.pts, seq
                                );
                                vf.ensure_rgba8();
                                let _ = buffer_arc.push_wait(
                                    vf,
                                    epoch,
                                    Some(Duration::from_millis(50)),
                                );
                            } else {
                                buffer_arc.set_eos(epoch, true);
                            }
                        } else {
                            drop(dec);
                        }
                    } else {
                        let ctrl = control_arc.lock().unwrap();
                        if ctrl.pending_seek.is_none()
                            && !ctrl.play_pending
                            && !shutdown.load(Ordering::SeqCst)
                        {
                            let _ = condvar_arc.wait_timeout(ctrl, Duration::from_millis(10));
                        }
                    }
                }
            });
    }

    /// Background thread: continuously consumes frames from the lookahead ring buffer,
    /// synchronized against the master clock, executing frame dropping on late frames.
    fn spawn_emitter_worker(self: &Arc<Self>) {
        let session_weak = Arc::downgrade(self);
        let buffer_arc = Arc::clone(&self.lookahead_buffer);
        let clock_arc = Arc::clone(&self.master_clock);
        let is_playing = Arc::clone(&self.is_playing);
        let current_pts = Arc::clone(&self.current_pts);
        let shutdown = Arc::clone(&self.worker_shutdown);
        let session_id = self.session_id.clone();
        let duration_pts = self.duration_pts;
        let clock_fps = self.master_clock.fps().max(1.0);

        let _ = thread::Builder::new()
            .name(format!("aether-emit-{}", session_id))
            .spawn(move || {
                let mut last_state_broadcast = Instant::now();

                while !shutdown.load(Ordering::SeqCst) {
                    if !is_playing.load(Ordering::SeqCst) {
                        thread::sleep(Duration::from_millis(4));
                        continue;
                    }

                    let session = match session_weak.upgrade() {
                        Some(s) => s,
                        None => break,
                    };

                    let master_pts = clock_arc.current_pts();

                    // Check for reaching or exceeding EOS
                    if master_pts >= duration_pts {
                        current_pts.store(duration_pts, Ordering::SeqCst);
                        is_playing.store(false, Ordering::SeqCst);
                        clock_arc.pause();
                        session.broadcast_state();
                        continue;
                    }

                    if session.media_type == MediaType::Audio {
                        current_pts.store(master_pts, Ordering::SeqCst);
                        if last_state_broadcast.elapsed() >= Duration::from_millis(50) {
                            session.broadcast_state();
                            last_state_broadcast = Instant::now();
                        }
                        thread::sleep(Duration::from_millis(15));
                        continue;
                    }

                    match buffer_arc.peek_front_with_successor() {
                        Some((frame_pts, frame_dur, _next_pts)) => {
                            let tolerance_pts = ((FrameDropper::SYNC_TOLERANCE_MS as f64 / 1000.0) * clock_fps).round() as i64;
                            let buffer_can_catch_up = buffer_arc.can_catch_up(master_pts, tolerance_pts);

                            match session.frame_dropper.evaluate_pts(frame_pts, master_pts, clock_fps, buffer_can_catch_up) {
                                FrameAction::SoftDrop | FrameAction::Drop => {
                                    eprintln!(
                                        "[DEBUG EMIT] Frame SOFT DROPPED! frame_pts: {}, master_pts: {}",
                                        frame_pts, master_pts
                                    );
                                    buffer_arc.drop_front();
                                    continue;
                                }
                                FrameAction::HardDrop { target_pts } => {
                                    eprintln!(
                                        "[DEBUG EMIT] Frame HARD DROP! Recovering via .index: target_pts: {}, master_pts: {}",
                                        target_pts, master_pts
                                    );
                                    session.trigger_hard_drop(target_pts);
                                    thread::sleep(Duration::from_millis(4));
                                    continue;
                                }
                                FrameAction::Present => {
                                    let present_start = Instant::now();
                                    eprintln!("[DEBUG EMIT] Frame APRESENTADO! PTS: {}, master_pts: {}", frame_pts, master_pts);
                                    if let Some(vf) = buffer_arc.pop_front() {
                                        current_pts.store(vf.pts, Ordering::SeqCst);
                                        clock_arc.seek(vf.pts);

                                        let timestamp_seconds = vf.timestamp_seconds();
                                        let bf = BridgeFrame {
                                            width: vf.width,
                                            height: vf.height,
                                            pts: vf.pts,
                                            duration_pts: vf.duration_pts,
                                            rgba_bytes: vf.data,
                                            row_stride_bytes: vf.row_stride_bytes,
                                            timestamp_seconds,
                                        };
                                        session.broadcast_frame(&bf);

                                        // Throttled state broadcast (reduces Riverpod rebuilds in Flutter)
                                        if last_state_broadcast.elapsed() >= Duration::from_millis(100) {
                                            session.broadcast_state();
                                            last_state_broadcast = Instant::now();
                                        }

                                        if let Ok(mut last) = session.last_frame.lock() {
                                            *last = Some(bf);
                                        }

                                        // Cadence pacing: keep frame visible on screen for its nominal duration
                                        let frame_dur_secs = (frame_dur.max(1) as f64 / clock_fps).clamp(0.005, 0.2);
                                        let elapsed = present_start.elapsed();
                                        if let Some(remaining) = Duration::from_secs_f64(frame_dur_secs).checked_sub(elapsed) {
                                            thread::sleep(remaining);
                                        }
                                    }
                                }
                                FrameAction::Wait { pts_ahead } => {
                                    let sleep_secs = (pts_ahead as f64 / clock_fps).clamp(0.001, 0.05);
                                    thread::sleep(Duration::from_secs_f64(sleep_secs));
                                }
                            }
                        }
                        None => {
                            if buffer_arc.is_eos() {
                                current_pts.store(duration_pts, Ordering::SeqCst);
                                is_playing.store(false, Ordering::SeqCst);
                                clock_arc.pause();
                                session.broadcast_state();
                            } else {
                                // Buffer underrun: yield briefly for decoder thread
                                // If master clock ran far ahead (> HARD_DROP_THRESHOLD_MS), trigger HardDrop if cooldown allows
                                let cur = current_pts.load(Ordering::SeqCst);
                                let delta_pts = master_pts.saturating_sub(cur);
                                let delta_ms = if clock_fps > 0.0 {
                                    ((delta_pts as f64 / clock_fps) * 1000.0).round() as i64
                                } else {
                                    0
                                };
                                if delta_ms > FrameDropper::HARD_DROP_THRESHOLD_MS as i64 {
                                    let action = session.frame_dropper.evaluate_delta(delta_ms, false, master_pts);
                                    if let FrameAction::HardDrop { target_pts } = action {
                                        eprintln!(
                                            "[DEBUG EMIT] Starvation HARD DROP! target_pts: {}, master_pts: {}",
                                            target_pts, master_pts
                                        );
                                        session.trigger_hard_drop(target_pts);
                                    }
                                }
                                thread::sleep(Duration::from_millis(2));
                            }
                        }
                    }
                }
            });
    }

    /// Single-step advance method for backward compatibility and manual stepping.
    pub fn tick_playback(&self) -> Result<PlaybackTickStatus, String> {
        let mut dec = self.decoder.lock().map_err(|e| e.to_string())?;

        if dec.video().is_some() {
            match dec.next_video_frame() {
                Ok(Some(mut vf)) => {
                    vf.ensure_rgba8();
                    self.current_pts.store(vf.pts, Ordering::SeqCst);
                    let timestamp_seconds = vf.timestamp_seconds();
                    let bf = BridgeFrame {
                        width: vf.width,
                        height: vf.height,
                        pts: vf.pts,
                        duration_pts: vf.duration_pts,
                        rgba_bytes: vf.data,
                        row_stride_bytes: vf.row_stride_bytes,
                        timestamp_seconds,
                    };
                    if let Ok(mut last) = self.last_frame.lock() {
                        *last = Some(bf.clone());
                    }
                    self.broadcast_frame(&bf);
                    self.broadcast_state();
                    Ok(PlaybackTickStatus::FrameDecoded)
                }
                Ok(None) => Ok(PlaybackTickStatus::EndOfStream),
                Err(e) => Err(e.to_string()),
            }
        } else if dec.audio().is_some() {
            match dec.next_audio_buffer() {
                Ok(Some(ab)) => {
                    self.current_pts.store(ab.pts, Ordering::SeqCst);
                    self.broadcast_state();
                    Ok(PlaybackTickStatus::AudioBufferDecoded)
                }
                Ok(None) => Ok(PlaybackTickStatus::EndOfStream),
                Err(e) => Err(e.to_string()),
            }
        } else {
            Ok(PlaybackTickStatus::EndOfStream)
        }
    }

    /// Seeks the session to a specific presentation timestamp (PTS).
    ///
    /// Delegates to the decoder worker state machine which debounces/coalesces
    /// rapid scrub events, queries .index via mmap for the nearest keyframe,
    /// performs container seek, and delivers the decoded target frame.
    pub fn seek_pts(&self, target_pts: i64) -> Result<Option<BridgeFrame>, String> {
        if self.worker_shutdown.load(Ordering::SeqCst) {
            return Err("Session is closed".to_string());
        }

        let clamped_pts = target_pts.clamp(0, self.duration_pts);
        let new_epoch = self.current_epoch.fetch_add(1, Ordering::SeqCst) + 1;

        // 1. Immediately flush lookahead buffer
        self.lookahead_buffer.flush_and_set_epoch(new_epoch);

        // 2. Set current PTS and update master clock
        if clamped_pts == 0 {
            self.current_pts.store(0, Ordering::SeqCst);
        } else if clamped_pts == self.duration_pts {
            self.current_pts.store(self.duration_pts, Ordering::SeqCst);
        } else {
            self.current_pts.store(clamped_pts, Ordering::SeqCst);
        }
        self.master_clock.seek(clamped_pts);

        // 3. Queue the seek command with debounce / coalescing
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        {
            let mut ctrl = self.decoder_control.lock().unwrap();
            if let Some(ref mut pending) = ctrl.pending_seek {
                // Coalesce / Debounce: discard intermediate target, update to newest target!
                pending.target_pts = clamped_pts;
                pending.epoch = new_epoch;
                pending.responders.push(tx);
            } else {
                ctrl.pending_seek = Some(PendingSeekRequest {
                    target_pts: clamped_pts,
                    epoch: new_epoch,
                    responders: vec![tx],
                });
            }
            ctrl.state = DecoderState::Seeking(clamped_pts);
            self.decoder_condvar.notify_one();
        }

        // 4. Wait for the seek to complete and receive the decoded frame
        match rx.recv_timeout(Duration::from_millis(3000)) {
            Ok(res) => res,
            Err(_) => {
                if self.worker_shutdown.load(Ordering::SeqCst) {
                    Err("Session is closed".to_string())
                } else {
                    Ok(self.get_last_frame())
                }
            }
        }
    }

    /// Convenience method to seek by fractional seconds.
    pub fn seek_seconds(&self, seconds: f64) -> Result<Option<BridgeFrame>, String> {
        let clamped_sec = seconds.clamp(0.0, self.duration_seconds);
        let timeline_fps = if self.timebase.den != 0 && self.timebase.num > 0 {
            self.timebase.num as f64 / self.timebase.den as f64
        } else if self.fps > 0.0 {
            self.fps
        } else {
            60.0
        };
        let target_pts = if timeline_fps > 0.0 {
            (clamped_sec * timeline_fps).round() as i64
        } else {
            0
        };
        self.seek_pts(target_pts)
    }

    /// Triggers an asynchronous Hard Drop keyframe recovery seek via the decoder worker
    /// without blocking presentation caller threads.
    pub fn trigger_hard_drop(&self, target_pts: i64) {
        if self.worker_shutdown.load(Ordering::SeqCst) {
            return;
        }
        let clamped_pts = target_pts.clamp(0, self.duration_pts);
        let new_epoch = self.current_epoch.fetch_add(1, Ordering::SeqCst) + 1;
        self.lookahead_buffer.flush_and_set_epoch(new_epoch);
        self.master_clock.seek(clamped_pts);
        {
            let mut ctrl = self.decoder_control.lock().unwrap();
            if let Some(ref mut pending) = ctrl.pending_seek {
                pending.target_pts = clamped_pts;
                pending.epoch = new_epoch;
            } else {
                ctrl.pending_seek = Some(PendingSeekRequest {
                    target_pts: clamped_pts,
                    epoch: new_epoch,
                    responders: Vec::new(),
                });
            }
            ctrl.state = DecoderState::Seeking(clamped_pts);
            self.decoder_condvar.notify_one();
        }
    }

    /// Terminates background threads and frees resources.
    pub fn close(&self) {
        self.is_playing.store(false, Ordering::SeqCst);
        self.worker_shutdown.store(true, Ordering::SeqCst);
        self.lookahead_buffer.close();
        {
            let mut ctrl = self.decoder_control.lock().unwrap();
            if let Some(pending) = ctrl.pending_seek.take() {
                for r in pending.responders {
                    let _ = r.send(Ok(self.get_last_frame()));
                }
            }
        }
        self.decoder_condvar.notify_all();
    }
}

impl Drop for PlaybackSession {
    fn drop(&mut self) {
        self.close();
    }
}

/// Global registry managing active playback sessions and unique texture allocations.
pub struct SessionRegistry {
    sessions: RwLock<HashMap<String, Arc<PlaybackSession>>>,
    next_texture_id: AtomicI64,
}

impl SessionRegistry {
    /// Creates a new `SessionRegistry`.
    pub fn new() -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
            next_texture_id: AtomicI64::new(1001),
        }
    }

    /// Access the global singleton registry.
    pub fn global() -> &'static SessionRegistry {
        static REGISTRY: LazyLock<SessionRegistry> = LazyLock::new(SessionRegistry::new);
        &REGISTRY
    }

    /// Creates and registers a new preview playback session for the given media path.
    pub fn create_session(&self, file_path: &str) -> Result<PreviewSessionInfo, String> {
        if file_path.trim().is_empty() {
            return Err("File path cannot be empty".to_string());
        }

        if !file_path.starts_with("synthetic://") {
            let path = Path::new(file_path);
            if !path.exists() {
                return Err(format!("File does not exist: {}", file_path));
            }
            if !path.is_file() {
                return Err(format!("Path is not a regular file: {}", file_path));
            }
            let metadata = std::fs::metadata(path)
                .map_err(|e| format!("Failed to read metadata for '{}': {}", file_path, e))?;
            if metadata.len() == 0 {
                return Err(format!("File is empty (0 bytes): {}", file_path));
            }
        }

        let mut decoder = aether_media::open_media_decoder(file_path)
            .map_err(|e| format!("Failed to open media decoder for '{}': {}", file_path, e))?;

        let meta = decoder.metadata().clone();
        let media_type = decoder.media_type();

        let width = meta.width.unwrap_or(0);
        let height = meta.height.unwrap_or(0);

        let (timebase, duration_pts, valid_fps, duration_seconds) = if let Some(v) = decoder.video() {
            let tb = v.timebase();
            let v_fps = v.fps();
            let fps = if v_fps > 0.0 {
                v_fps
            } else if tb.den != 0 && tb.num > 0 {
                tb.num as f64 / tb.den as f64
            } else {
                meta.fps().unwrap_or(60.0)
            };
            let dur_pts = if v.duration_pts() > 0 {
                v.duration_pts()
            } else if meta.duration_pts > 0 {
                meta.duration_pts
            } else {
                1
            };
            let dur_sec = if v.duration_seconds() > 0.0 {
                v.duration_seconds()
            } else if meta.duration_seconds > 0.0 {
                meta.duration_seconds
            } else {
                let timeline_fps = if tb.den != 0 && tb.num > 0 {
                    tb.num as f64 / tb.den as f64
                } else {
                    fps
                };
                dur_pts as f64 / timeline_fps
            };
            (tb, dur_pts, fps, dur_sec)
        } else if let Some(a) = decoder.audio() {
            let tb = a.timebase();
            let fps = meta.fps().unwrap_or(60.0);
            let dur_pts = if a.duration_pts() > 0 {
                a.duration_pts()
            } else {
                meta.duration_pts.max(1)
            };
            let dur_sec = if a.duration_seconds() > 0.0 {
                a.duration_seconds()
            } else {
                meta.duration_seconds
            };
            (tb, dur_pts, fps, dur_sec)
        } else {
            let tb = meta.timebase.unwrap_or(Rational { num: 60, den: 1 });
            let fps = meta.fps().unwrap_or(60.0);
            let dur_pts = meta.duration_pts.max(1);
            let dur_sec = meta.duration_seconds;
            (tb, dur_pts, fps, dur_sec)
        };

        let session_id = uuid::Uuid::new_v4().to_string();
        let texture_id = self.next_texture_id.fetch_add(1, Ordering::SeqCst);

        // Pre-fetch initial frame at PTS 0 for video assets
        let mut initial_frame: Option<BridgeFrame> = None;
        let mut actual_width = width;
        let mut actual_height = height;

        if decoder.video().is_some() {
            let _ = decoder.seek_pts(0);
            if let Ok(Some(mut vf)) = decoder.next_video_frame() {
                vf.ensure_rgba8();
                actual_width = vf.width;
                actual_height = vf.height;
                let timestamp_seconds = vf.timestamp_seconds();
                initial_frame = Some(BridgeFrame {
                    width: vf.width,
                    height: vf.height,
                    pts: vf.pts,
                    duration_pts: vf.duration_pts,
                    rgba_bytes: vf.data,
                    row_stride_bytes: vf.row_stride_bytes,
                    timestamp_seconds,
                });
            }
        }

        // Load or build GOP FrameIndex and memory-mapped MmapFrameIndex for video files
        let (frame_index, mmap_frame_index) = if !file_path.starts_with("synthetic://") {
            let path = Path::new(file_path);
            let sidecar_candidate = PathBuf::from(format!("{}.index", path.display()));
            let cache_dir = std::env::temp_dir().join("aether_media_cache");
            let mgr = aether_media::CacheManager::new(cache_dir);

            let mut mmap_opt = if sidecar_candidate.exists() {
                aether_media::MmapFrameIndex::open(&sidecar_candidate).ok()
            } else if let Ok((_, target_path)) = mgr.resolve_frame_index_for_source(path) {
                if target_path.exists() {
                    aether_media::MmapFrameIndex::open(&target_path).ok()
                } else {
                    None
                }
            } else {
                None
            };

            let fidx = aether_media::build_or_load_frame_index_with_timebase(path, timebase, Some(&mgr))
                .ok();

            if mmap_opt.is_none() {
                if let Ok((_, target_path)) = mgr.resolve_frame_index_for_source(path) {
                    mmap_opt = aether_media::MmapFrameIndex::open(&target_path).ok();
                }
            }

            (fidx.map(Arc::new), mmap_opt.map(Arc::new))
        } else {
            (None, None)
        };

        let lookahead_buffer = Arc::new(LookaheadRingBuffer::new(DEFAULT_LOOKAHEAD_CAPACITY));
        let master_clock = Arc::new(MasterClock::new(timebase));
        let seek_count = Arc::new(AtomicUsize::new(if initial_frame.is_some() { 1 } else { 0 }));
        let sequential_frame_count = Arc::new(AtomicUsize::new(0));

        let decoder_control = Arc::new(Mutex::new(DecoderWorkerControl {
            state: DecoderState::Idle,
            pending_seek: None,
            play_pending: false,
            pause_pending: false,
        }));
        let decoder_condvar = Arc::new(Condvar::new());

        let session = Arc::new(PlaybackSession {
            session_id: session_id.clone(),
            texture_id,
            file_path: file_path.to_string(),
            width: actual_width,
            height: actual_height,
            duration_pts,
            duration_seconds,
            fps: valid_fps,
            timebase,
            media_type,
            is_playing: Arc::new(AtomicBool::new(false)),
            current_pts: Arc::new(AtomicI64::new(0)),
            lookahead_buffer,
            master_clock,
            frame_index,
            mmap_frame_index,
            seek_count,
            sequential_frame_count,
            current_epoch: AtomicU64::new(0),
            worker_shutdown: Arc::new(AtomicBool::new(false)),
            decoder: Arc::new(Mutex::new(decoder)),
            frame_sinks: Mutex::new(Vec::new()),
            state_sinks: Mutex::new(Vec::new()),
            last_frame: Mutex::new(initial_frame),
            decoder_control,
            decoder_condvar,
            frame_dropper: FrameDropper::with_defaults(),
        });

        session.start_background_workers();

        let info = session.info();

        let mut map = self.sessions.write().map_err(|e| e.to_string())?;
        map.insert(session_id, session);

        Ok(info)
    }

    /// Closes an active session, stopping playback threads and freeing resources.
    pub fn close_session(&self, session_id: &str) -> Result<(), String> {
        let mut map = self.sessions.write().map_err(|e| e.to_string())?;
        if let Some(session) = map.remove(session_id) {
            session.close();
            Ok(())
        } else {
            Err(format!("Session with ID '{}' not found", session_id))
        }
    }

    /// Retrieves an active session by ID.
    pub fn get_session(&self, session_id: &str) -> Result<Arc<PlaybackSession>, String> {
        let map = self.sessions.read().map_err(|e| e.to_string())?;
        map.get(session_id)
            .cloned()
            .ok_or_else(|| format!("Session with ID '{}' not found", session_id))
    }

    /// Starts playback on the target session.
    pub fn play(&self, session_id: &str) -> Result<(), String> {
        let session = self.get_session(session_id)?;
        session.play()
    }

    /// Pauses playback on the target session.
    pub fn pause(&self, session_id: &str) -> Result<(), String> {
        let session = self.get_session(session_id)?;
        session.pause()
    }

    /// Seeks the target session to presentation timestamp (PTS).
    pub fn seek_pts(
        &self,
        session_id: &str,
        target_pts: i64,
    ) -> Result<Option<BridgeFrame>, String> {
        let session = self.get_session(session_id)?;
        session.seek_pts(target_pts)
    }

    /// Seeks the target session to fractional seconds.
    pub fn seek_seconds(
        &self,
        session_id: &str,
        seconds: f64,
    ) -> Result<Option<BridgeFrame>, String> {
        let session = self.get_session(session_id)?;
        session.seek_seconds(seconds)
    }

    /// Returns the active playback state of the target session.
    pub fn get_state(&self, session_id: &str) -> Result<PlaybackState, String> {
        let session = self.get_session(session_id)?;
        Ok(session.get_state())
    }

    /// Attaches a video frame StreamSink to the target session.
    pub fn subscribe_frames(
        &self,
        session_id: &str,
        sink: StreamSink<BridgeFrame>,
    ) -> Result<(), String> {
        let session = self.get_session(session_id)?;
        session.subscribe_frames(sink);
        Ok(())
    }

    /// Attaches a playback state StreamSink to the target session.
    pub fn subscribe_state(
        &self,
        session_id: &str,
        sink: StreamSink<PlaybackState>,
    ) -> Result<(), String> {
        let session = self.get_session(session_id)?;
        session.subscribe_state(sink);
        Ok(())
    }

    /// Clears and terminates all registered sessions (useful for test teardown).
    pub fn clear_all(&self) {
        if let Ok(mut map) = self.sessions.write() {
            for (_, session) in map.drain() {
                session.close();
            }
        }
    }
}

/// Standalone stateless frame extractor that opens a decoder, seeks to `target_pts`,
/// decodes the frame to RGBA8, and returns it without retaining a session.
pub fn extract_single_frame(file_path: String, target_pts: i64) -> Result<BridgeFrame, String> {
    if file_path.trim().is_empty() {
        return Err("File path cannot be empty".to_string());
    }

    if !file_path.starts_with("synthetic://") {
        let path = Path::new(&file_path);
        if !path.exists() {
            return Err(format!("File does not exist: {}", file_path));
        }
        if !path.is_file() {
            return Err(format!("Path is not a regular file: {}", file_path));
        }
        let metadata = std::fs::metadata(path)
            .map_err(|e| format!("Failed to read metadata for '{}': {}", file_path, e))?;
        if metadata.len() == 0 {
            return Err(format!("File is empty (0 bytes): {}", file_path));
        }
    }

    let mut decoder = aether_media::open_media_decoder(&file_path)
        .map_err(|e| format!("Failed to open media decoder for '{}': {}", file_path, e))?;

    let duration = decoder.metadata().duration_pts.max(0);
    let clamped_pts = target_pts.clamp(0, duration);

    decoder
        .seek_pts(clamped_pts)
        .map_err(|e| format!("Failed to seek to PTS {}: {}", clamped_pts, e))?;

    let mut frame_opt = decoder
        .next_video_frame()
        .map_err(|e| format!("Failed to decode frame at PTS {}: {}", clamped_pts, e))?;

    if frame_opt.is_none() && clamped_pts > 0 {
        let fps = if let Some(v) = decoder.video() { v.fps() } else { 30.0 };
        let tb = decoder.metadata().timebase.unwrap_or(Rational { num: 60, den: 1 });
        let timeline_fps = if tb.den != 0 && tb.num > 0 { tb.num as f64 / tb.den as f64 } else { 60.0 };
        let pts_step = ((timeline_fps / fps.max(1.0)).ceil() as i64).max(1);
        let fallback_pts = (clamped_pts - pts_step).max(0);
        let _ = decoder.seek_pts(fallback_pts);
        frame_opt = decoder.next_video_frame().unwrap_or(None);
    }

    let mut frame = frame_opt
        .ok_or_else(|| format!("No video frame found at PTS {}", clamped_pts))?;

    frame.ensure_rgba8();
    let timestamp_seconds = frame.timestamp_seconds();

    Ok(BridgeFrame {
        width: frame.width,
        height: frame.height,
        pts: frame.pts,
        duration_pts: frame.duration_pts,
        rgba_bytes: frame.data,
        row_stride_bytes: frame.row_stride_bytes,
        timestamp_seconds,
    })
}
