//! Lookahead Ring Buffer, Audio Master Clock (A/V Sync), and Frame Dropper
//! for high-performance preview player playback.

use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, LazyLock, Mutex};
use std::time::{Duration, Instant};

use aether_core::timeline::Rational;

use crate::decoder::types::VideoFrame;

static MONOTONIC_CLOCK_BASE: LazyLock<Instant> = LazyLock::new(Instant::now);

#[inline]
fn now_monotonic_nanos() -> u64 {
    Instant::now().duration_since(*MONOTONIC_CLOCK_BASE).as_nanos() as u64
}

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

/// Pre-allocated reusable byte buffer pool for decoded video frames.
/// Eliminates dynamic memory allocations in the playback rendering loop.
#[derive(Debug)]
pub struct FrameBufferPool {
    free_list: Mutex<Vec<Vec<u8>>>,
    max_buffers: usize,
    default_capacity: AtomicUsize,
    alloc_count: AtomicUsize,
    recycle_count: AtomicUsize,
}

impl FrameBufferPool {
    /// Creates a new `FrameBufferPool` with the specified maximum pool size and default buffer capacity.
    pub fn new(max_buffers: usize, default_capacity: usize) -> Self {
        let max_b = max_buffers.max(4);
        let mut initial = Vec::with_capacity(max_b);
        if default_capacity > 0 {
            for _ in 0..max_b {
                initial.push(Vec::with_capacity(default_capacity));
            }
        }
        let initial_allocs = initial.len();
        Self {
            free_list: Mutex::new(initial),
            max_buffers: max_b,
            default_capacity: AtomicUsize::new(default_capacity),
            alloc_count: AtomicUsize::new(initial_allocs),
            recycle_count: AtomicUsize::new(0),
        }
    }

    /// Acquires a byte buffer from the free list, or allocates a new one if empty.
    pub fn acquire(&self, required_bytes: usize) -> Vec<u8> {
        let mut list = self.free_list.lock().unwrap();
        if let Some(mut buf) = list.pop() {
            if buf.capacity() < required_bytes {
                buf.reserve(required_bytes - buf.capacity());
                self.alloc_count.fetch_add(1, Ordering::Relaxed);
            }
            buf
        } else {
            self.alloc_count.fetch_add(1, Ordering::Relaxed);
            Vec::with_capacity(required_bytes)
        }
    }

    /// Recycles a byte buffer back to the free list for subsequent reuse.
    pub fn recycle(&self, mut buffer: Vec<u8>) {
        buffer.clear();
        let mut list = self.free_list.lock().unwrap();
        if list.len() < self.max_buffers {
            list.push(buffer);
            self.recycle_count.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Number of free buffers currently available in the pool.
    pub fn free_count(&self) -> usize {
        self.free_list.lock().unwrap().len()
    }

    /// Total number of heap allocations performed by this pool.
    pub fn alloc_count(&self) -> usize {
        self.alloc_count.load(Ordering::Relaxed)
    }

    /// Total number of buffer recycle operations performed.
    pub fn recycle_count(&self) -> usize {
        self.recycle_count.load(Ordering::Relaxed)
    }

    /// Default buffer capacity in bytes configured for this pool.
    pub fn default_capacity(&self) -> usize {
        self.default_capacity.load(Ordering::Relaxed)
    }
}

impl Default for FrameBufferPool {
    fn default() -> Self {
        Self::new(32, 0)
    }
}

/// Single slot in the lookahead ring buffer holding a video frame and its presentation metadata.
struct RingSlot {
    frame: Mutex<Option<VideoFrame>>,
    pts: AtomicI64,
    duration_pts: AtomicI64,
    epoch: AtomicU64,
}

impl RingSlot {
    fn new() -> Self {
        Self {
            frame: Mutex::new(None),
            pts: AtomicI64::new(0),
            duration_pts: AtomicI64::new(0),
            epoch: AtomicU64::new(0),
        }
    }
}

/// Thread-safe bounded ring buffer holding decoded lookahead video frames.
///
/// Refactored for SPSC (Single-Producer Single-Consumer) low-contention execution:
/// the producer (decoder worker) and consumer (presenter) write and read to independent
/// slot indices with atomic release/acquire synchronization, eliminating lock contention.
pub struct LookaheadRingBuffer {
    capacity: usize,
    slots: Box<[RingSlot]>,

    head: AtomicUsize,
    tail: AtomicUsize,
    epoch: AtomicU64,
    eos_reached: AtomicBool,
    closed: AtomicBool,
    dropped_frames: AtomicU64,

    producer_mutex: Mutex<()>,
    consumer_mutex: Mutex<()>,

    not_full_cond: Condvar,
    not_full_mutex: Mutex<()>,

    not_empty_cond: Condvar,
    not_empty_mutex: Mutex<()>,

    pool: Arc<FrameBufferPool>,
}

impl LookaheadRingBuffer {
    /// Creates a new `LookaheadRingBuffer` with the specified frame capacity.
    pub fn new(capacity: usize) -> Self {
        Self::with_pool(capacity, Arc::new(FrameBufferPool::new(capacity * 2, 0)))
    }

    /// Creates a new `LookaheadRingBuffer` with a shared `FrameBufferPool`.
    pub fn with_pool(capacity: usize, pool: Arc<FrameBufferPool>) -> Self {
        let cap = capacity.clamp(MIN_LOOKAHEAD_CAPACITY, 64);
        let mut slots = Vec::with_capacity(cap);
        for _ in 0..cap {
            slots.push(RingSlot::new());
        }
        Self {
            capacity: cap,
            slots: slots.into_boxed_slice(),
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
            epoch: AtomicU64::new(0),
            eos_reached: AtomicBool::new(false),
            closed: AtomicBool::new(false),
            dropped_frames: AtomicU64::new(0),
            producer_mutex: Mutex::new(()),
            consumer_mutex: Mutex::new(()),
            not_full_cond: Condvar::new(),
            not_full_mutex: Mutex::new(()),
            not_empty_cond: Condvar::new(),
            not_empty_mutex: Mutex::new(()),
            pool,
        }
    }

    /// Access the underlying `FrameBufferPool`.
    #[inline]
    pub fn pool(&self) -> &Arc<FrameBufferPool> {
        &self.pool
    }

    /// Acquires a reusable buffer from the internal frame buffer pool.
    #[inline]
    pub fn acquire_buffer(&self, required_bytes: usize) -> Vec<u8> {
        self.pool.acquire(required_bytes)
    }

    /// Recycles a buffer back to the internal frame buffer pool.
    #[inline]
    pub fn recycle_buffer(&self, buffer: Vec<u8>) {
        self.pool.recycle(buffer);
    }

    /// Maximum capacity of the ring buffer.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Number of frames currently buffered (lock-free).
    #[inline]
    pub fn len(&self) -> usize {
        let t = self.tail.load(Ordering::Acquire);
        let h = self.head.load(Ordering::Acquire);
        t.saturating_sub(h)
    }

    /// Returns `true` if no frames are currently queued (lock-free).
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns `true` if the buffer is currently at full capacity (lock-free).
    #[inline]
    pub fn is_full(&self) -> bool {
        self.len() >= self.capacity
    }

    /// Current seek epoch identifier (lock-free).
    #[inline]
    pub fn current_epoch(&self) -> u64 {
        self.epoch.load(Ordering::Acquire)
    }

    /// Returns `true` if end of stream has been flagged for the current epoch (lock-free).
    #[inline]
    pub fn is_eos(&self) -> bool {
        self.eos_reached.load(Ordering::Acquire)
    }

    /// Flags whether the current epoch reached end of stream.
    pub fn set_eos(&self, epoch: u64, eos: bool) {
        if self.epoch.load(Ordering::Acquire) == epoch {
            self.eos_reached.store(eos, Ordering::Release);
            self.not_empty_cond.notify_all();
        }
    }

    /// Pushes a decoded frame into the back of the buffer.
    ///
    /// The producer locks only its own producer mutex, never contending with consumer peek/pop.
    /// If full, yields on a condition variable until space is freed or timeout expires.
    pub fn push_wait(
        &self,
        frame: VideoFrame,
        epoch: u64,
        timeout: Option<Duration>,
    ) -> Result<(), BufferError> {
        let start = Instant::now();

        loop {
            if self.closed.load(Ordering::Acquire) {
                self.pool.recycle(frame.data);
                return Err(BufferError::Closed);
            }
            if self.epoch.load(Ordering::Acquire) != epoch {
                self.pool.recycle(frame.data);
                return Err(BufferError::StaleEpoch);
            }

            {
                let _prod_guard = self.producer_mutex.lock().unwrap();
                if self.closed.load(Ordering::Acquire) {
                    self.pool.recycle(frame.data);
                    return Err(BufferError::Closed);
                }
                if self.epoch.load(Ordering::Acquire) != epoch {
                    self.pool.recycle(frame.data);
                    return Err(BufferError::StaleEpoch);
                }

                let t = self.tail.load(Ordering::Acquire);
                let h = self.head.load(Ordering::Acquire);

                if t.saturating_sub(h) < self.capacity {
                    let slot = &self.slots[t % self.capacity];
                    slot.pts.store(frame.pts, Ordering::Release);
                    slot.duration_pts.store(frame.duration_pts, Ordering::Release);
                    slot.epoch.store(epoch, Ordering::Release);

                    let mut slot_frame = slot.frame.lock().unwrap();
                    *slot_frame = Some(frame);
                    drop(slot_frame);

                    self.tail.store(t + 1, Ordering::Release);
                    self.not_empty_cond.notify_one();
                    return Ok(());
                }
            }

            // Buffer is full: wait on not_full_cond without holding producer_mutex
            let guard = self.not_full_mutex.lock().unwrap();
            let t = self.tail.load(Ordering::Acquire);
            let h = self.head.load(Ordering::Acquire);
            if t.saturating_sub(h) < self.capacity {
                drop(guard);
                continue;
            }

            if let Some(dur) = timeout {
                let elapsed = start.elapsed();
                if elapsed >= dur {
                    drop(guard);
                    self.pool.recycle(frame.data);
                    return Err(BufferError::Timeout);
                }
                let remaining = dur - elapsed;
                let (new_guard, timeout_res) =
                    self.not_full_cond.wait_timeout(guard, remaining).unwrap();
                drop(new_guard);
                if timeout_res.timed_out() {
                    let t = self.tail.load(Ordering::Acquire);
                    let h = self.head.load(Ordering::Acquire);
                    if t.saturating_sub(h) >= self.capacity {
                        self.pool.recycle(frame.data);
                        return Err(BufferError::Timeout);
                    }
                }
            } else {
                let new_guard = self.not_full_cond.wait(guard).unwrap();
                drop(new_guard);
            }
        }
    }

    /// Non-blocking, lock-free peek of presentation timestamp (PTS) and duration of the front frame.
    #[inline]
    pub fn peek_pts_and_duration(&self) -> Option<(i64, i64)> {
        let h = self.head.load(Ordering::Acquire);
        let t = self.tail.load(Ordering::Acquire);
        if h >= t {
            return None;
        }
        let slot = &self.slots[h % self.capacity];
        Some((
            slot.pts.load(Ordering::Acquire),
            slot.duration_pts.load(Ordering::Acquire),
        ))
    }

    /// Non-blocking, lock-free peek of the front frame `(pts, duration_pts)` plus the PTS of the
    /// frame queued right behind it (if any).
    #[inline]
    pub fn peek_front_with_successor(&self) -> Option<(i64, i64, Option<i64>)> {
        let h = self.head.load(Ordering::Acquire);
        let t = self.tail.load(Ordering::Acquire);
        if h >= t {
            return None;
        }
        let front_slot = &self.slots[h % self.capacity];
        let front_pts = front_slot.pts.load(Ordering::Acquire);
        let front_dur = front_slot.duration_pts.load(Ordering::Acquire);

        let next_pts = if t > h + 1 {
            let next_slot = &self.slots[(h + 1) % self.capacity];
            Some(next_slot.pts.load(Ordering::Acquire))
        } else {
            None
        };
        Some((front_pts, front_dur, next_pts))
    }

    /// Non-blocking, lock-free check of the presentation timestamp (PTS) of the newest/back frame.
    #[inline]
    pub fn highest_buffered_pts(&self) -> Option<i64> {
        let h = self.head.load(Ordering::Acquire);
        let t = self.tail.load(Ordering::Acquire);
        if h >= t {
            return None;
        }
        let slot = &self.slots[(t - 1) % self.capacity];
        Some(slot.pts.load(Ordering::Acquire))
    }

    /// Checks whether the frames currently buffered are capable of catching up to `master_pts`
    /// within `tolerance_pts` without needing a decoder keyframe seek (lock-free).
    #[inline]
    pub fn can_catch_up(&self, master_pts: i64, tolerance_pts: i64) -> bool {
        self.highest_buffered_pts()
            .map(|pts| pts.saturating_add(tolerance_pts.max(0)) >= master_pts)
            .unwrap_or(false)
    }

    /// Pops the front frame from the queue, signaling waiting producers.
    ///
    /// Consumer locks only its consumer mutex, never contending with producer writes.
    pub fn pop_front(&self) -> Option<VideoFrame> {
        let _cons_guard = self.consumer_mutex.lock().unwrap();
        let h = self.head.load(Ordering::Acquire);
        let t = self.tail.load(Ordering::Acquire);
        if h >= t {
            return None;
        }
        let slot = &self.slots[h % self.capacity];
        let mut slot_frame = slot.frame.lock().unwrap();
        let frame = slot_frame.take();
        drop(slot_frame);

        self.head.store(h + 1, Ordering::Release);
        self.not_full_cond.notify_one();
        frame
    }

    /// Pops the front frame from the queue, blocking until a frame is available, closed, or timeout expires.
    pub fn pop_wait(&self, timeout: Option<Duration>) -> Option<VideoFrame> {
        let start = Instant::now();
        loop {
            if let Some(frame) = self.pop_front() {
                return Some(frame);
            }
            if self.closed.load(Ordering::Acquire) || self.eos_reached.load(Ordering::Acquire) {
                return None;
            }
            let guard = self.not_empty_mutex.lock().unwrap();
            if !self.is_empty() {
                drop(guard);
                continue;
            }
            if let Some(dur) = timeout {
                let elapsed = start.elapsed();
                if elapsed >= dur {
                    drop(guard);
                    return None;
                }
                let remaining = dur - elapsed;
                let (new_guard, _) = self.not_empty_cond.wait_timeout(guard, remaining).unwrap();
                drop(new_guard);
            } else {
                let new_guard = self.not_empty_cond.wait(guard).unwrap();
                drop(new_guard);
            }
        }
    }

    /// Drops the front frame without copying, automatically recycling its pixel buffer
    /// and incrementing the `dropped_frames` counter.
    pub fn drop_front(&self) -> bool {
        let _cons_guard = self.consumer_mutex.lock().unwrap();
        let h = self.head.load(Ordering::Acquire);
        let t = self.tail.load(Ordering::Acquire);
        if h >= t {
            return false;
        }
        let slot = &self.slots[h % self.capacity];
        let mut slot_frame = slot.frame.lock().unwrap();
        if let Some(frame) = slot_frame.take() {
            self.pool.recycle(frame.data);
        }
        drop(slot_frame);

        self.head.store(h + 1, Ordering::Release);
        self.dropped_frames.fetch_add(1, Ordering::Relaxed);
        self.not_full_cond.notify_one();
        true
    }

    /// Flushes all queued frames, recycles their pixel buffers, and updates the seek epoch.
    pub fn flush_and_set_epoch(&self, new_epoch: u64) {
        let _prod_guard = self.producer_mutex.lock().unwrap();
        let _cons_guard = self.consumer_mutex.lock().unwrap();

        let h = self.head.load(Ordering::Acquire);
        let t = self.tail.load(Ordering::Acquire);
        for idx in h..t {
            let slot = &self.slots[idx % self.capacity];
            let mut slot_frame = slot.frame.lock().unwrap();
            if let Some(frame) = slot_frame.take() {
                self.pool.recycle(frame.data);
            }
        }

        self.head.store(0, Ordering::Release);
        self.tail.store(0, Ordering::Release);
        self.epoch.store(new_epoch, Ordering::Release);
        self.eos_reached.store(false, Ordering::Release);

        self.not_full_cond.notify_all();
        self.not_empty_cond.notify_all();
    }

    /// Closes the buffer, waking up all waiting producers and consumers.
    pub fn close(&self) {
        self.closed.store(true, Ordering::Release);
        self.not_full_cond.notify_all();
        self.not_empty_cond.notify_all();
    }

    /// Total count of frames dropped by frame dropping logic.
    #[inline]
    pub fn dropped_frames(&self) -> u64 {
        self.dropped_frames.load(Ordering::Acquire)
    }
}

/// Master Clock synchronizing A/V playback against audio stream presentation timestamps,
/// with high-resolution monotonic system clock fallback and completely lock-free readers.
#[derive(Debug)]
pub struct MasterClock {
    timebase: Rational,
    anchor_pts: AtomicI64,
    anchor_nanos: AtomicU64,
    last_audio_pts: AtomicI64,
    is_running: AtomicBool,
    seq: AtomicUsize,
    writer_mutex: Mutex<()>,
}

impl MasterClock {
    /// Creates a new `MasterClock` configured for the specified timeline timebase.
    pub fn new(timebase: Rational) -> Self {
        Self {
            timebase,
            anchor_pts: AtomicI64::new(0),
            anchor_nanos: AtomicU64::new(0),
            last_audio_pts: AtomicI64::new(-1),
            is_running: AtomicBool::new(false),
            seq: AtomicUsize::new(0),
            writer_mutex: Mutex::new(()),
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
        let _guard = self.writer_mutex.lock().unwrap();
        let now_nanos = now_monotonic_nanos();
        self.seq.fetch_add(1, Ordering::Release);
        self.anchor_pts.store(from_pts, Ordering::Relaxed);
        self.anchor_nanos.store(now_nanos, Ordering::Relaxed);
        self.is_running.store(true, Ordering::Relaxed);
        self.seq.fetch_add(1, Ordering::Release);
    }

    /// Pauses the master clock and returns the instantaneous PTS at the time of pause.
    pub fn pause(&self) -> i64 {
        let _guard = self.writer_mutex.lock().unwrap();
        let pts = self.current_pts();
        let now_nanos = now_monotonic_nanos();
        self.seq.fetch_add(1, Ordering::Release);
        self.anchor_pts.store(pts, Ordering::Relaxed);
        self.anchor_nanos.store(now_nanos, Ordering::Relaxed);
        self.is_running.store(false, Ordering::Relaxed);
        self.seq.fetch_add(1, Ordering::Release);
        pts
    }

    /// Seeks the master clock to a new PTS anchor.
    pub fn seek(&self, target_pts: i64) {
        let _guard = self.writer_mutex.lock().unwrap();
        let now_nanos = now_monotonic_nanos();
        self.seq.fetch_add(1, Ordering::Release);
        self.anchor_pts.store(target_pts, Ordering::Relaxed);
        self.anchor_nanos.store(now_nanos, Ordering::Relaxed);
        self.last_audio_pts.store(-1, Ordering::Relaxed);
        self.seq.fetch_add(1, Ordering::Release);
    }

    /// Updates the master clock with a newly decoded audio buffer presentation timestamp.
    ///
    /// The audio stream acts as the absolute master clock: video presentation synchronizes
    /// to these PTS updates.
    pub fn update_audio_pts(&self, audio_pts: i64) {
        let _guard = self.writer_mutex.lock().unwrap();
        let now_nanos = now_monotonic_nanos();
        self.seq.fetch_add(1, Ordering::Release);
        self.last_audio_pts.store(audio_pts, Ordering::Relaxed);
        self.anchor_pts.store(audio_pts, Ordering::Relaxed);
        self.anchor_nanos.store(now_nanos, Ordering::Relaxed);
        self.seq.fetch_add(1, Ordering::Release);
    }

    /// Returns the current instantaneous presentation timestamp according to the master clock.
    ///
    /// Fully lock-free: uses atomic sequence counters (SeqLock) without acquiring any Mutex,
    /// safe for low-latency concurrent readers across presentation, audio, and UI threads.
    #[inline]
    pub fn current_pts(&self) -> i64 {
        loop {
            let s0 = self.seq.load(Ordering::Acquire);
            if s0 & 1 != 0 {
                std::hint::spin_loop();
                continue;
            }
            let running = self.is_running.load(Ordering::Relaxed);
            let pts = self.anchor_pts.load(Ordering::Relaxed);
            let anchor_nanos = self.anchor_nanos.load(Ordering::Relaxed);

            std::sync::atomic::fence(Ordering::Acquire);
            let s1 = self.seq.load(Ordering::Acquire);
            if s0 == s1 {
                if !running {
                    return pts;
                }
                let now_nanos = now_monotonic_nanos();
                let elapsed_nanos = now_nanos.saturating_sub(anchor_nanos);
                let elapsed_secs = elapsed_nanos as f64 / 1_000_000_000.0;
                let elapsed_pts = (elapsed_secs * self.fps()).round() as i64;
                return pts.saturating_add(elapsed_pts);
            }
        }
    }

    /// Lock-free current presentation time in nanoseconds.
    #[inline]
    pub fn current_nanos(&self) -> u64 {
        loop {
            let s0 = self.seq.load(Ordering::Acquire);
            if s0 & 1 != 0 {
                std::hint::spin_loop();
                continue;
            }
            let running = self.is_running.load(Ordering::Relaxed);
            let pts = self.anchor_pts.load(Ordering::Relaxed);
            let anchor_nanos = self.anchor_nanos.load(Ordering::Relaxed);

            std::sync::atomic::fence(Ordering::Acquire);
            let s1 = self.seq.load(Ordering::Acquire);
            if s0 == s1 {
                let pts_nanos = if self.fps() > 0.0 {
                    ((pts.max(0) as f64 / self.fps()) * 1_000_000_000.0).round() as u64
                } else {
                    0
                };
                if !running {
                    return pts_nanos;
                }
                let now_nanos = now_monotonic_nanos();
                let elapsed_nanos = now_nanos.saturating_sub(anchor_nanos);
                return pts_nanos.saturating_add(elapsed_nanos);
            }
        }
    }

    /// Lock-free current presentation time in microseconds.
    #[inline]
    pub fn current_micros(&self) -> u64 {
        self.current_nanos() / 1_000
    }

    /// Lock-free current presentation time in fractional seconds.
    #[inline]
    pub fn current_seconds(&self) -> f64 {
        self.current_nanos() as f64 / 1_000_000_000.0
    }

    /// Returns `true` if the clock is actively progressing (lock-free).
    #[inline]
    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::Acquire)
    }

    /// Returns the most recent audio PTS update, if any (lock-free).
    #[inline]
    pub fn last_audio_pts(&self) -> Option<i64> {
        let p = self.last_audio_pts.load(Ordering::Acquire);
        if p >= 0 {
            Some(p)
        } else {
            None
        }
    }
}

/// Action determined by the frame dropping evaluator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameAction {
    /// The frame is on time (within sync tolerance, e.g. <= 25ms delay): present immediately.
    Present,
    /// The frame is in the future: presentation should wait until its PTS arrives.
    Wait { pts_ahead: i64 },
    /// Mild or moderate lag (e.g. 25ms to 180ms/200ms):
    /// Drop frame from presentation ring buffer without interrupting or seeking the decoder.
    SoftDrop,
    /// Severe lag (e.g. > 200ms) with buffer unable to catch up:
    /// Initiate keyframe recovery seek via .index, subject to cooldown (min 500ms).
    HardDrop { target_pts: i64 },
    /// Legacy drop variant (maps to SoftDrop behavior for backwards compatibility).
    Drop,
}

impl FrameAction {
    /// Returns `true` if the action is a drop (SoftDrop, HardDrop, or legacy Drop).
    #[inline]
    pub fn is_drop(&self) -> bool {
        matches!(
            self,
            FrameAction::SoftDrop | FrameAction::HardDrop { .. } | FrameAction::Drop
        )
    }

    /// Returns `true` if the action is Present.
    #[inline]
    pub fn is_present(&self) -> bool {
        matches!(self, FrameAction::Present)
    }
}

/// Evaluates frame presentation timing against the master clock to enforce A/V sync
/// with a two-tiered tolerance policy (Soft Drop vs. Hard Drop).
///
/// # Two-Tiered Policy:
/// 1. **On-Time (`Present`):** Lag is within `SYNC_TOLERANCE_MS` (<= 25ms). Frame is presented immediately.
/// 2. **Mild/Moderate Lag (`SoftDrop`):** Lag is between 25ms and `HARD_DROP_THRESHOLD_MS` (<= 200ms),
///    OR the lookahead buffer already holds frames that can catch up to the master clock.
///    The presenter discards stale frames from the ring buffer without touching the decoder, preserving
///    the inter-frame P/B reference chain.
/// 3. **Severe Lag (`HardDrop`):** Lag exceeds `HARD_DROP_THRESHOLD_MS` (> 200ms) AND the buffer cannot
///    catch up. Initiates a keyframe seek via `.index`. Enforces `HARD_DROP_COOLDOWN_MS` (>= 500ms)
///    to prevent "death spiral" seek loops on underpowered machines.
#[derive(Debug)]
pub struct FrameDropper {
    pub sync_tolerance_ms: u64,
    pub soft_drop_threshold_ms: u64,
    pub hard_drop_threshold_ms: u64,
    pub hard_drop_cooldown: Duration,
    last_hard_drop: Mutex<Option<Instant>>,
}

impl Clone for FrameDropper {
    fn clone(&self) -> Self {
        let last = *self.last_hard_drop.lock().unwrap();
        Self {
            sync_tolerance_ms: self.sync_tolerance_ms,
            soft_drop_threshold_ms: self.soft_drop_threshold_ms,
            hard_drop_threshold_ms: self.hard_drop_threshold_ms,
            hard_drop_cooldown: self.hard_drop_cooldown,
            last_hard_drop: Mutex::new(last),
        }
    }
}

impl FrameDropper {
    /// Nominal sync tolerance: <= 25ms lag is considered on-time -> Present.
    pub const SYNC_TOLERANCE_MS: u64 = 25;
    /// Upper bound for soft drop range: mild/moderate lag up to 180ms -> SoftDrop.
    pub const SOFT_DROP_THRESHOLD_MS: u64 = 180;
    /// Threshold beyond which severe lag triggers a HardDrop (> 200ms).
    pub const HARD_DROP_THRESHOLD_MS: u64 = 200;
    /// Minimum cooldown between consecutive Hard Drops (500ms) to prevent seek death spirals.
    pub const HARD_DROP_COOLDOWN_MS: u64 = 500;
    /// Default maximum allowed late frame multiplier (legacy).
    pub const DEFAULT_MAX_LATE_MULTIPLIER: i64 = 2;

    /// Creates a new `FrameDropper` with custom tolerance thresholds and cooldown.
    pub fn new(
        sync_tolerance_ms: u64,
        soft_drop_threshold_ms: u64,
        hard_drop_threshold_ms: u64,
        hard_drop_cooldown: Duration,
    ) -> Self {
        Self {
            sync_tolerance_ms,
            soft_drop_threshold_ms,
            hard_drop_threshold_ms,
            hard_drop_cooldown,
            last_hard_drop: Mutex::new(None),
        }
    }

    /// Creates a `FrameDropper` with default thresholds (25ms sync, 180ms soft, 200ms hard, 500ms cooldown).
    pub fn with_defaults() -> Self {
        Self::new(
            Self::SYNC_TOLERANCE_MS,
            Self::SOFT_DROP_THRESHOLD_MS,
            Self::HARD_DROP_THRESHOLD_MS,
            Duration::from_millis(Self::HARD_DROP_COOLDOWN_MS),
        )
    }

    /// Evaluates time delta in milliseconds at a specific timestamp for deterministic testing.
    pub fn evaluate_delta_at(
        &self,
        delta_ms: i64,
        buffer_can_catch_up: bool,
        target_pts: i64,
        now: Instant,
    ) -> FrameAction {
        // Frame is in the future: presentation clock hasn't arrived at frame time
        if delta_ms < -(self.sync_tolerance_ms as i64) {
            let pts_ahead = delta_ms.abs();
            return FrameAction::Wait { pts_ahead };
        }

        // 1. Within sync tolerance (-SYNC_TOLERANCE_MS <= delta_ms <= SYNC_TOLERANCE_MS): Present
        if delta_ms <= self.sync_tolerance_ms as i64 {
            return FrameAction::Present;
        }

        // 2. Soft Drop zone: lag is mild to moderate (<= HARD_DROP_THRESHOLD_MS)
        // OR the buffer already has frames that can catch up to the clock
        if delta_ms <= self.hard_drop_threshold_ms as i64 || buffer_can_catch_up {
            return FrameAction::SoftDrop;
        }

        // 3. Severe lag (> HARD_DROP_THRESHOLD_MS) AND buffer cannot catch up:
        let mut last_drop = self.last_hard_drop.lock().unwrap();
        if let Some(prev) = *last_drop {
            if now.saturating_duration_since(prev) < self.hard_drop_cooldown {
                // Cooldown active: avoid seek death spiral, fall back to SoftDrop
                return FrameAction::SoftDrop;
            }
        }

        // Cooldown expired: trigger HardDrop seek recovery
        *last_drop = Some(now);
        FrameAction::HardDrop { target_pts }
    }

    /// Evaluates time delta in milliseconds using Instant::now().
    pub fn evaluate_delta(
        &self,
        delta_ms: i64,
        buffer_can_catch_up: bool,
        target_pts: i64,
    ) -> FrameAction {
        self.evaluate_delta_at(delta_ms, buffer_can_catch_up, target_pts, Instant::now())
    }

    /// Evaluates PTS against master clock PTS at a specific timestamp.
    pub fn evaluate_pts_at(
        &self,
        front_frame_pts: i64,
        master_pts: i64,
        fps: f64,
        buffer_can_catch_up: bool,
        now: Instant,
    ) -> FrameAction {
        if master_pts < front_frame_pts {
            let pts_ahead = front_frame_pts.saturating_sub(master_pts);
            return FrameAction::Wait { pts_ahead };
        }

        let delta_pts = master_pts.saturating_sub(front_frame_pts);
        let delta_ms = if fps > 0.0 {
            ((delta_pts as f64 / fps) * 1000.0).round() as i64
        } else {
            0
        };

        self.evaluate_delta_at(delta_ms, buffer_can_catch_up, master_pts, now)
    }

    /// Evaluates PTS against master clock PTS using Instant::now().
    pub fn evaluate_pts(
        &self,
        front_frame_pts: i64,
        master_pts: i64,
        fps: f64,
        buffer_can_catch_up: bool,
    ) -> FrameAction {
        self.evaluate_pts_at(
            front_frame_pts,
            master_pts,
            fps,
            buffer_can_catch_up,
            Instant::now(),
        )
    }

    /// Checks if hard drop cooldown is currently active at `now`.
    pub fn is_cooldown_active_at(&self, now: Instant) -> bool {
        if let Some(prev) = *self.last_hard_drop.lock().unwrap() {
            now.saturating_duration_since(prev) < self.hard_drop_cooldown
        } else {
            false
        }
    }

    /// Checks if hard drop cooldown is currently active.
    pub fn is_cooldown_active(&self) -> bool {
        self.is_cooldown_active_at(Instant::now())
    }

    /// Resets the hard drop cooldown timer.
    pub fn reset_cooldown(&self) {
        *self.last_hard_drop.lock().unwrap() = None;
    }

    /// Static evaluation using default thresholds and Instant::now().
    pub fn evaluate_static(
        delta_ms: i64,
        buffer_can_catch_up: bool,
        target_pts: i64,
    ) -> FrameAction {
        static DEFAULT_DROPPER: std::sync::LazyLock<FrameDropper> =
            std::sync::LazyLock::new(FrameDropper::with_defaults);
        DEFAULT_DROPPER.evaluate_delta(delta_ms, buffer_can_catch_up, target_pts)
    }

    /// Evaluates whether the front frame should be dropped, presented, or waited on,
    /// applying hysteresis tolerance to preserve visual smoothness.
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
            FrameAction::SoftDrop
        }
    }

    /// Successor-aware evaluation used by the realtime emitter.
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
                FrameAction::SoftDrop
            }
            _ => FrameAction::Present,
        }
    }
}

impl Default for FrameDropper {
    fn default() -> Self {
        Self::with_defaults()
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
        // para recuperar a sincronia e evitar acúmulo de latência (SoftDrop preserva integridade do decodificador).
        assert_eq!(FrameDropper::evaluate(10, 2, 15), FrameAction::SoftDrop);
        assert_eq!(FrameDropper::evaluate(10, 2, 25), FrameAction::SoftDrop);

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
            FrameAction::SoftDrop
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
        // Successor already due -> SoftDrop current (newer image is available)
        assert_eq!(
            FrameDropper::evaluate_with_successor(10, Some(12), 12),
            FrameAction::SoftDrop
        );
        // Critically late but it is the only decoded frame -> must Present, never freeze
        assert_eq!(
            FrameDropper::evaluate_with_successor(2, None, 271),
            FrameAction::Present
        );
    }

    #[test]
    fn test_frame_dropper_two_tiered_soft_and_hard_drop_decisions() {
        let dropper = FrameDropper::with_defaults();
        let t0 = Instant::now();

        // 1. On-time / within SYNC_TOLERANCE_MS (<= 25ms) -> Present
        assert_eq!(
            dropper.evaluate_delta_at(0, false, 100, t0),
            FrameAction::Present
        );
        assert_eq!(
            dropper.evaluate_delta_at(15, false, 100, t0),
            FrameAction::Present
        );
        assert_eq!(
            dropper.evaluate_delta_at(25, false, 100, t0),
            FrameAction::Present
        );

        // 2. Future frame (< -25ms) -> Wait
        assert_eq!(
            dropper.evaluate_delta_at(-50, false, 100, t0),
            FrameAction::Wait { pts_ahead: 50 }
        );

        // 3. Mild/moderate lag (26ms to 200ms) -> SoftDrop (decoder is NOT disturbed)
        assert_eq!(
            dropper.evaluate_delta_at(26, false, 100, t0),
            FrameAction::SoftDrop
        );
        assert_eq!(
            dropper.evaluate_delta_at(100, false, 100, t0),
            FrameAction::SoftDrop
        );
        assert_eq!(
            dropper.evaluate_delta_at(180, false, 100, t0),
            FrameAction::SoftDrop
        );
        assert_eq!(
            dropper.evaluate_delta_at(200, false, 100, t0),
            FrameAction::SoftDrop
        );

        // 4. Severe lag (> 200ms) BUT buffer can catch up -> SoftDrop (no decoder seek needed!)
        assert_eq!(
            dropper.evaluate_delta_at(250, true, 100, t0),
            FrameAction::SoftDrop
        );
        assert_eq!(
            dropper.evaluate_delta_at(500, true, 100, t0),
            FrameAction::SoftDrop
        );

        // 5. Severe lag (> 200ms) AND buffer cannot catch up -> HardDrop
        let action = dropper.evaluate_delta_at(250, false, 300, t0);
        assert_eq!(action, FrameAction::HardDrop { target_pts: 300 });
        assert!(dropper.is_cooldown_active_at(t0));

        // 6. Cooldown behavior: subsequent severe lag before 500ms cooldown elapses -> SoftDrop (death spiral prevented!)
        let t_100ms = t0 + Duration::from_millis(100);
        assert_eq!(
            dropper.evaluate_delta_at(300, false, 350, t_100ms),
            FrameAction::SoftDrop
        );

        let t_499ms = t0 + Duration::from_millis(499);
        assert_eq!(
            dropper.evaluate_delta_at(400, false, 450, t_499ms),
            FrameAction::SoftDrop
        );

        // 7. Cooldown expiration: at/after 500ms, HardDrop is authorized again
        let t_500ms = t0 + Duration::from_millis(500);
        let action_after_cooldown = dropper.evaluate_delta_at(350, false, 500, t_500ms);
        assert_eq!(
            action_after_cooldown,
            FrameAction::HardDrop { target_pts: 500 }
        );
    }

    #[test]
    fn test_frame_dropper_pts_framerate_evaluation() {
        let dropper = FrameDropper::with_defaults();
        let fps = 30.0; // 33.33ms per frame
        let t0 = Instant::now();

        // Frame at PTS 100, Clock at 100 -> delta = 0ms -> Present
        assert_eq!(
            dropper.evaluate_pts_at(100, 100, fps, false, t0),
            FrameAction::Present
        );

        // Clock at 101 -> delta = 1 frame = 33.3ms (> 25ms, <= 200ms) -> SoftDrop
        assert_eq!(
            dropper.evaluate_pts_at(100, 101, fps, false, t0),
            FrameAction::SoftDrop
        );

        // Clock at 103 -> delta = 3 frames = 100ms -> SoftDrop
        assert_eq!(
            dropper.evaluate_pts_at(100, 103, fps, false, t0),
            FrameAction::SoftDrop
        );

        // Clock at 110 -> delta = 10 frames = 333.3ms (> 200ms) with buffer catchup -> SoftDrop
        assert_eq!(
            dropper.evaluate_pts_at(100, 110, fps, true, t0),
            FrameAction::SoftDrop
        );

        // Clock at 110 without buffer catchup -> HardDrop { target_pts: 110 }
        assert_eq!(
            dropper.evaluate_pts_at(100, 110, fps, false, t0),
            FrameAction::HardDrop { target_pts: 110 }
        );
    }

    #[test]
    fn test_lookahead_ring_buffer_highest_pts_and_can_catch_up() {
        let buffer = LookaheadRingBuffer::new(8);
        assert!(buffer.highest_buffered_pts().is_none());
        assert!(!buffer.can_catch_up(100, 2));

        buffer.push_wait(make_test_frame(10, 2), 0, None).unwrap();
        buffer.push_wait(make_test_frame(12, 2), 0, None).unwrap();
        buffer.push_wait(make_test_frame(14, 2), 0, None).unwrap();

        assert_eq!(buffer.highest_buffered_pts(), Some(14));
        assert!(buffer.can_catch_up(14, 0));
        assert!(buffer.can_catch_up(15, 2));
        assert!(!buffer.can_catch_up(20, 2));
    }

    #[test]
    fn test_master_clock_lock_free_concurrent_readers() {
        use std::sync::atomic::AtomicBool;
        use std::sync::Arc;

        let clock = Arc::new(MasterClock::new(Rational { num: 60, den: 1 }));
        clock.start(100);

        let stop = Arc::new(AtomicBool::new(false));
        let mut readers = Vec::new();

        // Spawn 4 concurrent readers calling lock-free getters
        for _ in 0..4 {
            let c = Arc::clone(&clock);
            let s = Arc::clone(&stop);
            readers.push(thread::spawn(move || {
                let mut last_pts = 0;
                while !s.load(Ordering::Relaxed) {
                    let pts = c.current_pts();
                    let micros = c.current_micros();
                    let nanos = c.current_nanos();
                    let secs = c.current_seconds();
                    let _running = c.is_running();
                    assert!(pts >= 0);
                    assert!(secs >= 0.0);
                    let _ = (nanos, micros);
                    last_pts = pts;
                    std::hint::spin_loop();
                }
                last_pts
            }));
        }

        // Writer updates clock concurrently
        for i in 0..10 {
            clock.seek(200 + i * 50);
            clock.update_audio_pts(250 + i * 50);
            thread::sleep(Duration::from_millis(5));
        }

        stop.store(true, Ordering::Relaxed);
        for r in readers {
            let pts = r.join().unwrap();
            assert!(pts >= 200);
        }
    }

    #[test]
    fn test_frame_buffer_pool_zero_allocations_after_warmup() {
        let pool = Arc::new(FrameBufferPool::new(8, 1024));
        assert_eq!(pool.free_count(), 8);
        assert_eq!(pool.alloc_count(), 8);
        assert_eq!(pool.recycle_count(), 0);
        assert_eq!(pool.default_capacity(), 1024);

        // Acquire all 8 pre-warmed buffers
        let mut acquired = Vec::new();
        for _ in 0..8 {
            let buf = pool.acquire(1024);
            assert!(buf.capacity() >= 1024);
            acquired.push(buf);
        }
        assert_eq!(pool.free_count(), 0);
        assert_eq!(pool.alloc_count(), 8);

        // Recycle all 8 buffers
        for buf in acquired {
            pool.recycle(buf);
        }
        assert_eq!(pool.free_count(), 8);
        assert_eq!(pool.recycle_count(), 8);
        assert_eq!(pool.alloc_count(), 8); // Still 8! Zero new allocations!

        // Continuous acquire and recycle cycle: alloc_count MUST remain strictly 8!
        for _ in 0..100 {
            let mut buf = pool.acquire(1024);
            buf.extend_from_slice(&[42u8; 1024]);
            assert_eq!(buf.len(), 1024);
            pool.recycle(buf);
        }
        assert_eq!(pool.free_count(), 8);
        assert_eq!(pool.alloc_count(), 8); // Zero new allocations!
        assert_eq!(pool.recycle_count(), 108);
    }

    #[test]
    fn test_lookahead_ring_buffer_spsc_concurrency() {
        let pool = Arc::new(FrameBufferPool::new(16, 2048));
        let buffer = Arc::new(LookaheadRingBuffer::with_pool(8, pool.clone()));
        let initial_allocs = pool.alloc_count();
        assert_eq!(initial_allocs, 16);

        let total_frames = 100;
        let buf_producer = Arc::clone(&buffer);
        let producer_handle = thread::spawn(move || {
            for i in 0..total_frames {
                let mut data = buf_producer.acquire_buffer(2048);
                data.resize(2048, (i % 255) as u8);
                let vf = VideoFrame {
                    pts: i,
                    duration_pts: 1,
                    width: 32,
                    height: 16,
                    format: PixelFormat::Rgba8,
                    data,
                    row_stride_bytes: 128,
                    timebase: Rational { num: 60, den: 1 },
                    is_keyframe: i % 10 == 0,
                };
                buf_producer.push_wait(vf, 0, None).unwrap();
            }
        });

        let buf_consumer = Arc::clone(&buffer);
        let consumer_handle = thread::spawn(move || {
            let mut received = 0;
            for i in 0..total_frames {
                let frame = buf_consumer.pop_wait(Some(Duration::from_secs(3))).expect("Frame must arrive");
                assert_eq!(frame.pts, i);
                assert_eq!(frame.data.len(), 2048);
                assert_eq!(frame.data[0], (i % 255) as u8);
                // Presenter recycles buffer upon consumption
                buf_consumer.recycle_buffer(frame.data);
                received += 1;
            }
            received
        });

        producer_handle.join().unwrap();
        let received = consumer_handle.join().unwrap();
        assert_eq!(received, total_frames);

        // Verify all 16 buffers are safely back in the pool and NO allocations occurred during the loop!
        assert_eq!(pool.free_count(), 16);
        assert_eq!(pool.alloc_count(), initial_allocs);
    }

    #[test]
    fn test_lookahead_ring_buffer_flush_and_epoch_invalidation() {
        let pool = Arc::new(FrameBufferPool::new(12, 1024));
        let buffer = Arc::new(LookaheadRingBuffer::with_pool(6, pool.clone()));
        assert_eq!(pool.free_count(), 12);

        // Fill buffer at epoch 0
        for i in 0..6 {
            let mut data = buffer.acquire_buffer(1024);
            data.resize(1024, i as u8);
            let vf = VideoFrame {
                pts: i as i64,
                duration_pts: 1,
                width: 16,
                height: 16,
                format: PixelFormat::Rgba8,
                data,
                row_stride_bytes: 64,
                timebase: Rational { num: 60, den: 1 },
                is_keyframe: true,
            };
            buffer.push_wait(vf, 0, None).unwrap();
        }
        assert_eq!(buffer.len(), 6);
        assert_eq!(pool.free_count(), 6);

        // Flush on seek to epoch 1: should immediately drain and recycle all 6 frames
        buffer.flush_and_set_epoch(1);
        assert_eq!(buffer.len(), 0);
        assert_eq!(buffer.current_epoch(), 1);
        assert_eq!(pool.free_count(), 12); // All 6 frame buffers recycled!

        // Attempting to push with old epoch 0 is rejected with StaleEpoch and recycles its buffer
        let stale_buf = buffer.acquire_buffer(1024);
        assert_eq!(pool.free_count(), 11);
        let stale_vf = VideoFrame {
            pts: 99,
            duration_pts: 1,
            width: 16,
            height: 16,
            format: PixelFormat::Rgba8,
            data: stale_buf,
            row_stride_bytes: 64,
            timebase: Rational { num: 60, den: 1 },
            is_keyframe: true,
        };
        let err = buffer.push_wait(stale_vf, 0, None).unwrap_err();
        assert_eq!(err, BufferError::StaleEpoch);
        assert_eq!(pool.free_count(), 12); // Stale frame's buffer was recycled!
    }
}

