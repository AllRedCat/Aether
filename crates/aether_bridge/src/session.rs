use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex, RwLock};
use std::thread;
use std::time::Duration;

use aether_core::media::MediaType;
use aether_core::timeline::Rational;
use aether_media::decoder::traits::MediaDecoder;
use aether_media::frame_index::FrameIndex;
use aether_media::playback::{
    FrameAction, FrameDropper, LookaheadRingBuffer, MasterClock, DEFAULT_LOOKAHEAD_CAPACITY,
};

use crate::api::{BridgeFrame, PlaybackState, PreviewSessionInfo};
pub use crate::frb_generated::StreamSink;

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

    current_epoch: AtomicU64,
    worker_shutdown: Arc<AtomicBool>,
    decoder: Arc<Mutex<Box<dyn MediaDecoder>>>,
    frame_sinks: Mutex<Vec<StreamSink<BridgeFrame>>>,
    state_sinks: Mutex<Vec<StreamSink<PlaybackState>>>,
    last_frame: Mutex<Option<BridgeFrame>>,
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
        self.broadcast_state();
        Ok(())
    }

    /// Pauses sequential playback without killing background worker threads.
    pub fn pause(&self) -> Result<(), String> {
        self.is_playing.store(false, Ordering::SeqCst);
        let paused_pts = self.master_clock.pause();
        self.current_pts
            .store(paused_pts.clamp(0, self.duration_pts), Ordering::SeqCst);
        self.broadcast_state();
        Ok(())
    }

    /// Spawns the long-running decoupled decoder and presentation threads.
    pub fn start_background_workers(self: &Arc<Self>) {
        self.spawn_decoder_worker();
        self.spawn_emitter_worker();
    }

    /// Background thread: continuously decodes frames ahead of the playhead,
    /// populating the lookahead ring buffer up to capacity.
    fn spawn_decoder_worker(self: &Arc<Self>) {
        let decoder_arc = Arc::clone(&self.decoder);
        let buffer_arc = Arc::clone(&self.lookahead_buffer);
        let clock_arc = Arc::clone(&self.master_clock);
        let shutdown = Arc::clone(&self.worker_shutdown);
        let session_id = self.session_id.clone();

        let _ = thread::Builder::new()
            .name(format!("aether-dec-{}", session_id))
            .spawn(move || {
                while !shutdown.load(Ordering::SeqCst) {
                    let epoch = buffer_arc.current_epoch();

                    // If buffer is full or EOS reached for this epoch, sleep briefly
                    if buffer_arc.is_full() || buffer_arc.is_eos() {
                        thread::sleep(Duration::from_millis(4));
                        continue;
                    }

                    // Try acquiring decoder lock without blocking any in-flight seeks
                    let mut dec_guard = match decoder_arc.try_lock() {
                        Ok(g) => g,
                        Err(std::sync::TryLockError::WouldBlock) => {
                            thread::sleep(Duration::from_millis(2));
                            continue;
                        }
                        Err(std::sync::TryLockError::Poisoned(_)) => break,
                    };

                    // Check if seek epoch changed while acquiring lock
                    if buffer_arc.current_epoch() != epoch {
                        continue;
                    }

                    if dec_guard.video().is_some() {
                        let decode_res = dec_guard.next_video_frame();
                        // Also decode audio chunk to advance master audio clock if present
                        if dec_guard.audio().is_some() {
                            if let Ok(Some(ab)) = dec_guard.next_audio_buffer() {
                                clock_arc.update_audio_pts(ab.pts);
                            }
                        }
                        // Drop lock immediately before RGBA conversion and buffer wait!
                        drop(dec_guard);

                        match decode_res {
                            Ok(Some(mut vf)) => {
                                vf.ensure_rgba8();
                                let _ = buffer_arc.push_wait(
                                    vf,
                                    epoch,
                                    Some(Duration::from_millis(50)),
                                );
                            }
                            Ok(None) => {
                                buffer_arc.set_eos(epoch, true);
                            }
                            Err(_) => {
                                buffer_arc.set_eos(epoch, true);
                            }
                        }
                    } else if dec_guard.audio().is_some() {
                        let audio_res = dec_guard.next_audio_buffer();
                        drop(dec_guard);

                        match audio_res {
                            Ok(Some(ab)) => {
                                clock_arc.update_audio_pts(ab.pts);
                                thread::sleep(Duration::from_millis(10));
                            }
                            Ok(None) => {
                                buffer_arc.set_eos(epoch, true);
                            }
                            Err(_) => {
                                buffer_arc.set_eos(epoch, true);
                            }
                        }
                    } else {
                        drop(dec_guard);
                        buffer_arc.set_eos(epoch, true);
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
        let fps = if self.fps > 0.0 { self.fps } else { 60.0 };

        let _ = thread::Builder::new()
            .name(format!("aether-emit-{}", session_id))
            .spawn(move || {
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
                        session.broadcast_state();
                        thread::sleep(Duration::from_millis(15));
                        continue;
                    }

                    match buffer_arc.peek_pts_and_duration() {
                        Some((frame_pts, frame_dur)) => {
                            match FrameDropper::evaluate(frame_pts, frame_dur, master_pts) {
                                FrameAction::Drop => {
                                    // Late frame: drop immediately to prevent cumulative A/V drift!
                                    buffer_arc.drop_front();
                                    continue;
                                }
                                FrameAction::Present => {
                                    if let Some(vf) = buffer_arc.pop_front() {
                                        current_pts.store(vf.pts, Ordering::SeqCst);
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
                                        if let Ok(mut last) = session.last_frame.lock() {
                                            *last = Some(bf.clone());
                                        }
                                        session.broadcast_frame(&bf);
                                        session.broadcast_state();
                                    }
                                    thread::sleep(Duration::from_millis(4));
                                }
                                FrameAction::Wait { pts_ahead } => {
                                    let sleep_secs = (pts_ahead as f64 / fps).clamp(0.001, 0.016);
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
    /// Instantly flushes the lookahead ring buffer, performs $O(\log K)$ keyframe lookup
    /// using `FrameIndex`, decodes the target frame immediately for zero-latency UI response,
    /// and pre-fills the lookahead buffer forward from the new seek point.
    pub fn seek_pts(&self, target_pts: i64) -> Result<Option<BridgeFrame>, String> {
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

        // 3. Acquire decoder lock
        let mut dec = self.decoder.lock().map_err(|e| e.to_string())?;

        // 4. Seek to preceding keyframe in FrameIndex when available
        let seek_point = if let Some(ref fidx) = self.frame_index {
            if let Some(kf) = fidx.find_keyframe_at_or_before(clamped_pts) {
                kf.pts
            } else {
                clamped_pts
            }
        } else {
            clamped_pts
        };

        dec.seek_pts(seek_point)
            .map_err(|e| format!("Decoder seek failed to PTS {}: {}", seek_point, e))?;

        let frame = if dec.video().is_some() {
            let mut target_frame = None;
            while let Ok(Some(mut vf)) = dec.next_video_frame() {
                let pts = vf.pts;
                vf.ensure_rgba8();
                target_frame = Some(vf);
                if pts >= clamped_pts {
                    break;
                }
            }

            if target_frame.is_none() && seek_point != clamped_pts {
                let _ = dec.seek_pts(clamped_pts);
                if let Ok(Some(mut vf)) = dec.next_video_frame() {
                    vf.ensure_rgba8();
                    target_frame = Some(vf);
                }
            }

            if let Some(vf) = target_frame {
                if clamped_pts > 0 && clamped_pts < self.duration_pts {
                    self.current_pts.store(vf.pts, Ordering::SeqCst);
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
                if let Ok(mut last) = self.last_frame.lock() {
                    *last = Some(bf.clone());
                }
                self.broadcast_frame(&bf);
                Some(bf)
            } else {
                None
            }
        } else {
            None
        };

        drop(dec);
        self.broadcast_state();
        Ok(frame)
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

    /// Terminates background threads and frees resources.
    pub fn close(&self) {
        self.is_playing.store(false, Ordering::SeqCst);
        self.worker_shutdown.store(true, Ordering::SeqCst);
        self.lookahead_buffer.close();
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
        let fps = if let Some(v) = decoder.video() {
            let v_fps = v.fps();
            if v_fps > 0.0 {
                v_fps
            } else {
                meta.fps().unwrap_or(60.0)
            }
        } else {
            meta.fps().unwrap_or(60.0)
        };
        let valid_fps = if fps > 0.0 { fps } else { 60.0 };
        let duration_pts = if meta.duration_pts > 0 {
            meta.duration_pts
        } else {
            1
        };
        let timebase = meta.timebase.unwrap_or(Rational { num: 60, den: 1 });
        let duration_seconds = if meta.duration_seconds > 0.0 {
            meta.duration_seconds
        } else {
            let timeline_fps = if timebase.den != 0 && timebase.num > 0 {
                timebase.num as f64 / timebase.den as f64
            } else {
                valid_fps
            };
            duration_pts as f64 / timeline_fps
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

        // Load or build GOP FrameIndex for video files
        let frame_index = if !file_path.starts_with("synthetic://") {
            let path = Path::new(file_path);
            aether_media::build_or_load_frame_index_with_timebase(path, timebase, None)
                .ok()
                .map(Arc::new)
        } else {
            None
        };

        let lookahead_buffer = Arc::new(LookaheadRingBuffer::new(DEFAULT_LOOKAHEAD_CAPACITY));
        let master_clock = Arc::new(MasterClock::new(timebase));

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
            current_epoch: AtomicU64::new(0),
            worker_shutdown: Arc::new(AtomicBool::new(false)),
            decoder: Arc::new(Mutex::new(decoder)),
            frame_sinks: Mutex::new(Vec::new()),
            state_sinks: Mutex::new(Vec::new()),
            last_frame: Mutex::new(initial_frame),
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
