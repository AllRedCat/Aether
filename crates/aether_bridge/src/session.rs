use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, LazyLock, Mutex, RwLock};
use std::thread;
use std::time::Duration;

use aether_core::media::MediaType;
use aether_core::timeline::Rational;
use aether_media::decoder::traits::MediaDecoder;

use crate::api::{BridgeFrame, PlaybackState, PreviewSessionInfo};
pub use crate::frb_generated::StreamSink;

/// Status emitted during sequential playback ticker execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackTickStatus {
    FrameDecoded,
    AudioBufferDecoded,
    EndOfStream,
}

/// Represents an active media preview playback session.
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

    decoder: Mutex<Box<dyn MediaDecoder>>,
    frame_sinks: Mutex<Vec<StreamSink<BridgeFrame>>>,
    state_sinks: Mutex<Vec<StreamSink<PlaybackState>>>,
    ticker_stop_tx: Mutex<Option<std::sync::mpsc::Sender<()>>>,
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

    /// Starts sequential playback at the media's native frame rate.
    pub fn play(self: &Arc<Self>) -> Result<(), String> {
        // If at or beyond EOS, restart from beginning
        if self.current_pts.load(Ordering::SeqCst) >= self.duration_pts {
            let _ = self.seek_pts(0);
        }

        if self.is_playing.swap(true, Ordering::SeqCst) {
            // Already playing
            return Ok(());
        }

        // Stop any dangling ticker thread
        self.stop_ticker();

        let (stop_tx, stop_rx) = std::sync::mpsc::channel::<()>();
        if let Ok(mut tx_slot) = self.ticker_stop_tx.lock() {
            *tx_slot = Some(stop_tx);
        }

        self.broadcast_state();

        let session = Arc::clone(self);
        let fps = if session.fps > 0.0 { session.fps } else { 60.0 };
        let frame_interval = Duration::from_secs_f64(1.0 / fps);

        thread::Builder::new()
            .name(format!("aether-playback-{}", session.session_id))
            .spawn(move || {
                loop {
                    match stop_rx.recv_timeout(frame_interval) {
                        Ok(_) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                            break;
                        }
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                    }

                    if !session.is_playing.load(Ordering::SeqCst) {
                        break;
                    }

                    match session.tick_playback() {
                        Ok(PlaybackTickStatus::FrameDecoded) => {
                            // Frame was decoded and broadcasted in tick_playback
                        }
                        Ok(PlaybackTickStatus::AudioBufferDecoded) => {
                            // Audio tick dispatched state update
                        }
                        Ok(PlaybackTickStatus::EndOfStream) => {
                            session.current_pts.store(session.duration_pts, Ordering::SeqCst);
                            session.is_playing.store(false, Ordering::SeqCst);
                            session.broadcast_state();
                            break;
                        }
                        Err(_) => {
                            session.is_playing.store(false, Ordering::SeqCst);
                            session.broadcast_state();
                            break;
                        }
                    }
                }
            })
            .map_err(|e| format!("Failed to spawn playback thread: {}", e))?;

        Ok(())
    }

    /// Pauses sequential playback.
    pub fn pause(&self) -> Result<(), String> {
        self.is_playing.store(false, Ordering::SeqCst);
        self.stop_ticker();
        self.broadcast_state();
        Ok(())
    }

    /// Stops the ticker background thread.
    fn stop_ticker(&self) {
        if let Ok(mut tx_slot) = self.ticker_stop_tx.lock() {
            if let Some(tx) = tx_slot.take() {
                let _ = tx.send(());
            }
        }
    }

    /// Advances the decoder by one frame/buffer during playback loop.
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
    /// Clamps out-of-bounds PTS to [0, duration_pts].
    /// Decodes and returns the frame at that position immediately for responsive scrubbing.
    pub fn seek_pts(&self, target_pts: i64) -> Result<Option<BridgeFrame>, String> {
        let clamped_pts = target_pts.clamp(0, self.duration_pts);
        let mut dec = self.decoder.lock().map_err(|e| e.to_string())?;

        dec.seek_pts(clamped_pts)
            .map_err(|e| format!("Decoder seek failed to PTS {}: {}", clamped_pts, e))?;

        self.current_pts.store(clamped_pts, Ordering::SeqCst);

        let frame = if dec.video().is_some() {
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
                    Some(bf)
                }
                Ok(None) => None,
                Err(_) => None,
            }
        } else {
            None
        };

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
}

impl Drop for PlaybackSession {
    fn drop(&mut self) {
        self.is_playing.store(false, Ordering::SeqCst);
        self.stop_ticker();
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
            decoder: Mutex::new(decoder),
            frame_sinks: Mutex::new(Vec::new()),
            state_sinks: Mutex::new(Vec::new()),
            ticker_stop_tx: Mutex::new(None),
            last_frame: Mutex::new(initial_frame),
        });

        let info = session.info();

        let mut map = self.sessions.write().map_err(|e| e.to_string())?;
        map.insert(session_id, session);

        Ok(info)
    }

    /// Closes an active session, stopping playback threads and freeing resources.
    pub fn close_session(&self, session_id: &str) -> Result<(), String> {
        let mut map = self.sessions.write().map_err(|e| e.to_string())?;
        if let Some(session) = map.remove(session_id) {
            session.pause()?;
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
                let _ = session.pause();
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
