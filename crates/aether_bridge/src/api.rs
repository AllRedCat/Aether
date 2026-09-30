use flutter_rust_bridge::frb;
pub use aether_core::timeline::{Clip, Rational, Timeline, Track, TrackKind};
pub use aether_core::project::Project;
pub use aether_core::media::{MediaItem, MediaMetadata, MediaPool, MediaType};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
pub use crate::frb_generated::StreamSink;

// ---------------------------------------------------------
// MIRRORED TYPES
// ---------------------------------------------------------

#[allow(dead_code)]
#[frb(mirror(Rational))]
pub struct _Rational {
    pub num: i32,
    pub den: i32,
}

#[allow(dead_code)]
#[frb(mirror(TrackKind))]
pub enum _TrackKind {
    Video,
    Audio,
    Overlay,
}

#[allow(dead_code)]
#[frb(mirror(Clip))]
pub struct _Clip {
    pub id: Uuid,
    pub source_id: Uuid,
    pub source_in: i64,
    pub source_out: i64,
    pub timeline_in: i64,
    pub timeline_out: i64,
}

#[allow(dead_code)]
#[frb(mirror(Track))]
pub struct _Track {
    pub id: Uuid,
    pub kind: TrackKind,
    pub clips: Vec<Clip>,
}

#[allow(dead_code)]
#[frb(mirror(Timeline))]
pub struct _Timeline {
    pub id: Uuid,
    pub timebase: Rational,
    pub duration_pts: i64,
    pub tracks: Vec<Track>,
}

#[allow(dead_code)]
#[frb(mirror(MediaType))]
pub enum _MediaType {
    Video,
    Audio,
    Image,
}

#[allow(dead_code)]
#[frb(mirror(MediaMetadata))]
pub struct _MediaMetadata {
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub duration_pts: i64,
    pub duration_seconds: f64,
    pub timebase: Option<Rational>,
    pub audio_channels: Option<u16>,
    pub sample_rate: Option<u32>,
    pub file_size_bytes: u64,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub pixel_format: Option<String>,
    pub is_vfr: Option<bool>,
    pub keyframe_pts: Option<Vec<i64>>,
}

#[allow(dead_code)]
#[frb(mirror(MediaItem))]
pub struct _MediaItem {
    pub id: Uuid,
    pub file_path: String,
    pub file_name: String,
    pub media_type: MediaType,
    pub metadata: MediaMetadata,
}

#[allow(dead_code)]
#[frb(mirror(MediaPool))]
pub struct _MediaPool {
    pub items: Vec<MediaItem>,
}

#[allow(dead_code)]
#[frb(mirror(Project))]
pub struct _Project {
    pub id: String,
    pub name: String,
    pub project_path: String,
    pub file_path: String,
    pub timeline: Timeline,
    pub media_pool: MediaPool,
}

pub fn init_engine() {
    flutter_rust_bridge::setup_default_user_utils();
    aether_render::init_render();
    aether_media::init_media();
}

// ---------------------------------------------------------
// PROJECT MANAGEMENT
// ---------------------------------------------------------

pub fn create_project(name: String, base_dir: String) -> Result<Project, String> {
    Project::create_new(&name, &base_dir)
}

pub fn load_project(file_path: String) -> Result<Project, String> {
    Project::load(&file_path)
}

pub fn save_project(project: Project) -> Result<(), String> {
    project.save()
}

// ---------------------------------------------------------
// TIMELINE MANAGEMENT
// ---------------------------------------------------------

pub fn create_timeline() -> Timeline {
    let mut timeline = Timeline::new(Rational { num: 60, den: 1 });
    timeline.add_track(TrackKind::Video);
    timeline
}

pub fn add_track(mut timeline: Timeline, kind: TrackKind) -> Timeline {
    timeline.add_track(kind);
    timeline
}

pub fn add_clip_to_track(
    mut timeline: Timeline,
    track_id: Uuid,
    source_id: Uuid,
    source_in: i64,
    source_out: i64,
    timeline_in: i64,
) -> Result<Timeline, String> {
    let clip = Clip::new(source_id, source_in, source_out, timeline_in);
    timeline
        .add_clip(track_id, clip)
        .map_err(|e| e.to_string())?;
    Ok(timeline)
}

// ---------------------------------------------------------
// MEDIA POOL FFI ENDPOINTS
// ---------------------------------------------------------

/// Imports a media file, extracting its type and metadata via `aether_media`.
/// If `project_path` is provided, the imported media is appended to the project's
/// MediaPool and persisted to disk.
pub fn import_media_file(
    project_path: Option<String>,
    file_path: String,
) -> Result<MediaItem, String> {
    if file_path.trim().is_empty() {
        return Err("File path cannot be empty".to_string());
    }

    let path = std::path::Path::new(&file_path);
    if !path.exists() {
        return Err(format!("File does not exist: {}", file_path));
    }
    if !path.is_file() {
        return Err(format!("Path is not a regular file: {}", file_path));
    }

    let inspection = aether_media::inspect_media_file(&file_path)
        .map_err(|e| format!("Failed to inspect media file '{}': {}", file_path, e))?;

    let media_item = inspection.to_media_item(&file_path);

    if let Some(proj_path) = project_path {
        if !proj_path.trim().is_empty() {
            let mut project = Project::load(&proj_path)
                .map_err(|e| format!("Failed to load project at '{}': {}", proj_path, e))?;
            project.media_pool.items.push(media_item.clone());
            project
                .save()
                .map_err(|e| format!("Failed to save project after importing media: {}", e))?;
        }
    }

    Ok(media_item)
}

/// Standalone inspection of a media file without touching project persistence.
pub fn inspect_media_file(file_path: String) -> Result<MediaItem, String> {
    import_media_file(None, file_path)
}

/// Returns the list of imported media items in the specified project.
pub fn get_media_items(project_path: String) -> Result<Vec<MediaItem>, String> {
    let project = Project::load(&project_path)
        .map_err(|e| format!("Failed to load project at '{}': {}", project_path, e))?;
    Ok(project.media_pool.items)
}

/// Returns the entire `MediaPool` structure for the specified project.
pub fn get_media_pool(project_path: String) -> Result<MediaPool, String> {
    let project = Project::load(&project_path)
        .map_err(|e| format!("Failed to load project at '{}': {}", project_path, e))?;
    Ok(project.media_pool)
}

/// Adds a clip referencing a media item's UUID to a specific track in the timeline.
/// Defaults `source_in` to 0, `source_out` to media duration (or 300 PTS for images),
/// and `timeline_in` to the timeline's current duration PTS (append).
pub fn add_clip_to_track_from_media(
    mut timeline: Timeline,
    track_id: Uuid,
    media_item: MediaItem,
    source_in: Option<i64>,
    source_out: Option<i64>,
    timeline_in: Option<i64>,
) -> Result<Timeline, String> {
    let s_in = source_in.unwrap_or(0);
    let duration = if media_item.metadata.duration_pts > 0 {
        media_item.metadata.duration_pts
    } else {
        300 // 5 seconds at 60 FPS for images or zero-duration media
    };
    let s_out = source_out.unwrap_or(s_in.saturating_add(duration));
    let t_in = match timeline_in {
        Some(t) => t,
        None => {
            let track = timeline
                .tracks
                .iter()
                .find(|t| t.id == track_id)
                .ok_or_else(|| format!("Track with ID {} not found", track_id))?;
            track.duration_pts()
        }
    };

    let clip = Clip::new(media_item.id, s_in, s_out, t_in);
    timeline
        .add_clip(track_id, clip)
        .map_err(|e| e.to_string())?;
    Ok(timeline)
}

/// Adds a clip referencing a media asset in the project's MediaPool to the project's timeline
/// and persists the project to disk.
pub fn add_clip_from_media_pool(
    mut project: Project,
    track_id: Uuid,
    media_id: Uuid,
    source_in: Option<i64>,
    source_out: Option<i64>,
    timeline_in: Option<i64>,
) -> Result<Project, String> {
    let media = project
        .media_pool
        .items
        .iter()
        .find(|m| m.id == media_id)
        .ok_or_else(|| format!("Media with ID {} not found in project MediaPool", media_id))?
        .clone();

    project.timeline = add_clip_to_track_from_media(
        project.timeline,
        track_id,
        media,
        source_in,
        source_out,
        timeline_in,
    )?;

    if !project.file_path.is_empty() {
        let _ = project.save();
    }

    Ok(project)
}

// ---------------------------------------------------------
// PREVIEW & TEXTURE STREAMING FFI ENDPOINTS (Milestone M2)
// ---------------------------------------------------------

/// Technical metadata and Flutter Texture widget handle for an active preview session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PreviewSessionInfo {
    pub session_id: String,
    pub texture_id: i64,
    pub width: u32,
    pub height: u32,
    pub duration_pts: i64,
    pub duration_seconds: f64,
    pub fps: f64,
    pub timebase: Rational,
}

/// Raw RGBA8 video frame transferred across the FFI boundary to Flutter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BridgeFrame {
    pub width: u32,
    pub height: u32,
    pub pts: i64,
    pub duration_pts: i64,
    pub rgba_bytes: Vec<u8>,
    pub row_stride_bytes: u32,
    pub timestamp_seconds: f64,
}

/// Instantaneous playback state snapshot for reactive UI synchronization.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlaybackState {
    pub session_id: String,
    pub is_playing: bool,
    pub current_pts: i64,
    pub current_seconds: f64,
    pub duration_pts: i64,
    pub duration_seconds: f64,
}

/// Opens a media asset, extracts container metadata, pre-fetches the initial frame at PTS 0,
/// allocates a unique texture ID for Flutter's `Texture(textureId: ...)` widget,
/// and registers the active playback session in the `SessionRegistry`.
pub fn create_preview_session(file_path: String) -> Result<PreviewSessionInfo, String> {
    crate::session::SessionRegistry::global().create_session(&file_path)
}

/// Closes the active playback session, terminates background ticker threads,
/// drops active stream sinks, and frees all decoder memory.
pub fn close_preview_session(session_id: String) -> Result<(), String> {
    crate::session::SessionRegistry::global().close_session(&session_id)
}

/// Starts sequential playback at the media asset's native frame rate.
pub fn preview_play(session_id: String) -> Result<(), String> {
    crate::session::SessionRegistry::global().play(&session_id)
}

/// Pauses sequential playback.
pub fn preview_pause(session_id: String) -> Result<(), String> {
    crate::session::SessionRegistry::global().pause(&session_id)
}

/// Seeks to a specific presentation timestamp (PTS).
/// Clamps out-of-range targets to [0, duration_pts].
/// Decodes and returns the frame at that position immediately for zero-latency scrubber response,
/// while simultaneously broadcasting to any active frame and state stream sinks.
pub fn preview_seek_pts(session_id: String, target_pts: i64) -> Result<Option<BridgeFrame>, String> {
    crate::session::SessionRegistry::global().seek_pts(&session_id, target_pts)
}

/// Convenience seeking jumping to target fractional seconds.
pub fn preview_seek_seconds(session_id: String, seconds: f64) -> Result<Option<BridgeFrame>, String> {
    crate::session::SessionRegistry::global().seek_seconds(&session_id, seconds)
}

/// Returns the current playback position and playing state.
pub fn get_preview_state(session_id: String) -> Result<PlaybackState, String> {
    crate::session::SessionRegistry::global().get_state(&session_id)
}

/// Attaches a `StreamSink` to receive continuous RGBA8 video frames as they are decoded.
/// Emits the cached initial frame immediately upon subscription.
pub fn subscribe_preview_frames(
    session_id: String,
    sink: StreamSink<BridgeFrame>,
) -> Result<(), String> {
    crate::session::SessionRegistry::global().subscribe_frames(&session_id, sink)
}

/// Attaches a `StreamSink` to receive reactive playback state updates and PTS position ticks.
/// Emits current playback state immediately upon subscription.
pub fn subscribe_playback_state(
    session_id: String,
    sink: StreamSink<PlaybackState>,
) -> Result<(), String> {
    crate::session::SessionRegistry::global().subscribe_state(&session_id, sink)
}

/// Stateless thumbnail and scrub extractor: decodes a single frame at PTS without
/// maintaining a persistent playback session.
pub fn extract_single_frame(file_path: String, target_pts: i64) -> Result<BridgeFrame, String> {
    crate::session::extract_single_frame(file_path, target_pts)
}

