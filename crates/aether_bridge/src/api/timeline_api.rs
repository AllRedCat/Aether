use aether_core::db::timeline as db_tl;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

// ---------------------------------------------------------
// FFI DATA TRANSFER OBJECTS (Timeline, Track, Clip)
// ---------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimelineDto {
    pub id: String,
    pub name: String,
    pub fps_num: i32,
    pub fps_den: i32,
    pub width: u32,
    pub height: u32,
    pub duration_frames: i64,
    pub created_at: String,
    pub updated_at: String,
}

impl From<db_tl::Timeline> for TimelineDto {
    fn from(t: db_tl::Timeline) -> Self {
        Self {
            id: t.id,
            name: t.name,
            fps_num: t.fps_num,
            fps_den: t.fps_den,
            width: t.width,
            height: t.height,
            duration_frames: t.duration_frames,
            created_at: t.created_at,
            updated_at: t.updated_at,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackDto {
    pub id: String,
    pub timeline_id: String,
    pub name: String,
    pub track_type: String,
    pub is_video: bool,
    pub order_index: i32,
    pub is_muted: bool,
    pub is_locked: bool,
}

impl From<db_tl::Track> for TrackDto {
    fn from(t: db_tl::Track) -> Self {
        let is_video = t.track_type == db_tl::TrackType::Video;
        Self {
            id: t.id,
            timeline_id: t.timeline_id,
            name: t.name,
            track_type: t.track_type.as_str().to_string(),
            is_video,
            order_index: t.order_index,
            is_muted: t.is_muted,
            is_locked: t.is_locked,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimelineClipDto {
    pub id: String,
    pub track_id: String,
    pub asset_id: String,
    pub timeline_start_frame: i64,
    pub in_frame: i64,
    pub out_frame: i64,
    pub speed: f64,
}

impl From<db_tl::TimelineClip> for TimelineClipDto {
    fn from(c: db_tl::TimelineClip) -> Self {
        Self {
            id: c.id,
            track_id: c.track_id,
            asset_id: c.asset_id,
            timeline_start_frame: c.timeline_start_frame,
            in_frame: c.in_frame,
            out_frame: c.out_frame,
            speed: c.speed,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FullTimelineStateDto {
    pub timeline: TimelineDto,
    pub tracks: Vec<TrackDto>,
    pub clips: Vec<TimelineClipDto>,
}

impl FullTimelineStateDto {
    #[flutter_rust_bridge::frb(ignore)]
    pub fn new(
        timeline: db_tl::Timeline,
        tracks: Vec<db_tl::Track>,
        clips: Vec<db_tl::TimelineClip>,
    ) -> Self {
        Self {
            timeline: TimelineDto::from(timeline),
            tracks: tracks.into_iter().map(TrackDto::from).collect(),
            clips: clips.into_iter().map(TimelineClipDto::from).collect(),
        }
    }
}

// ---------------------------------------------------------
// SESSION SQLITE CONNECTION MANAGEMENT
// ---------------------------------------------------------

struct ActiveSessionDb {
    path: Option<String>,
    conn: Connection,
}

static ACTIVE_SESSION_DB: LazyLock<Mutex<Option<ActiveSessionDb>>> =
    LazyLock::new(|| Mutex::new(None));

/// Sets an active database connection in the session (useful for tests or in-memory DBs).
#[flutter_rust_bridge::frb(ignore)]
pub fn set_active_db_connection(conn: Connection) {
    if let Ok(mut guard) = ACTIVE_SESSION_DB.lock() {
        *guard = Some(ActiveSessionDb { path: None, conn });
    }
}

/// Clears the active database connection in the session.
#[flutter_rust_bridge::frb(ignore)]
pub fn clear_active_db_connection() {
    if let Ok(mut guard) = ACTIVE_SESSION_DB.lock() {
        *guard = None;
    }
}

/// Executes an operation using the active SQLite connection in the session.
#[flutter_rust_bridge::frb(ignore)]
pub fn with_active_db<F, R>(f: F) -> Result<R, String>
where
    F: FnOnce(&Connection) -> Result<R, String>,
{
    let mut guard = ACTIVE_SESSION_DB
        .lock()
        .map_err(|e| format!("Failed to acquire active DB lock: {}", e))?;

    let active_project_file = crate::api::get_active_project_file();

    let needs_open = match &*guard {
        Some(session_db) => {
            if let (Some(open_path), Some(curr_path)) = (&session_db.path, &active_project_file) {
                open_path != curr_path
            } else {
                false
            }
        }
        None => true,
    };

    if needs_open {
        if let Some(path) = active_project_file {
            let conn = Connection::open(&path)
                .map_err(|e| format!("Failed to open project SQLite database at '{}': {}", path, e))?;
            conn.busy_timeout(Duration::from_secs(5))
                .map_err(|e| format!("Failed to set DB busy timeout: {}", e))?;
            conn.execute_batch("PRAGMA foreign_keys = ON;")
                .map_err(|e| format!("Failed to enable foreign keys: {}", e))?;
            db_tl::init_timeline_schema(&conn)
                .map_err(|e| format!("Failed to initialize timeline schema: {}", e))?;

            *guard = Some(ActiveSessionDb {
                path: Some(path),
                conn,
            });
        }
    }

    match &*guard {
        Some(session_db) => f(&session_db.conn),
        None => Err("No active project or SQLite database connection in session".to_string()),
    }
}

// ---------------------------------------------------------
// FFI EXPORTED FUNCTIONS
// ---------------------------------------------------------

/// Creates a new timeline within a SQL transaction, automatically creating
/// default Video and Audio tracks, and returns the full timeline state.
pub fn create_new_timeline(
    name: String,
    width: u32,
    height: u32,
    fps_num: i32,
    fps_den: i32,
) -> Result<FullTimelineStateDto, String> {
    with_active_db(|conn| {
        let timeline = db_tl::create_timeline(conn, &name, fps_num, fps_den, width, height)
            .map_err(|e| format!("Failed to create timeline: {}", e))?;
        let (tl, tracks, clips) = db_tl::get_timeline_with_tracks(conn, &timeline.id)
            .map_err(|e| format!("Failed to load timeline with tracks: {}", e))?;
        Ok(FullTimelineStateDto::new(tl, tracks, clips))
    })
}

/// Lists all timelines in the current active project.
pub fn get_project_timelines() -> Result<Vec<TimelineDto>, String> {
    with_active_db(|conn| {
        let timelines = db_tl::list_timelines(conn)
            .map_err(|e| format!("Failed to list timelines: {}", e))?;
        Ok(timelines.into_iter().map(TimelineDto::from).collect())
    })
}

/// Loads the full timeline state (timeline, tracks, clips) by timeline ID.
pub fn load_timeline_state(timeline_id: String) -> Result<FullTimelineStateDto, String> {
    with_active_db(|conn| {
        let (tl, tracks, clips) = db_tl::get_timeline_with_tracks(conn, &timeline_id)
            .map_err(|e| format!("Failed to load timeline state for ID '{}': {}", timeline_id, e))?;
        Ok(FullTimelineStateDto::new(tl, tracks, clips))
    })
}

/// Creates a new track (video or audio) on the specified timeline.
pub fn create_track(timeline_id: String, is_video: bool, name: String) -> Result<TrackDto, String> {
    let track_type = if is_video {
        db_tl::TrackType::Video
    } else {
        db_tl::TrackType::Audio
    };

    with_active_db(|conn| {
        let track = db_tl::add_track(conn, &timeline_id, track_type, &name)
            .map_err(|e| format!("Failed to create track: {}", e))?;
        Ok(TrackDto::from(track))
    })
}

/// Deletes a timeline and its cascaded tracks and clips by timeline ID.
pub fn delete_timeline_by_id(timeline_id: String) -> Result<(), String> {
    with_active_db(|conn| {
        db_tl::delete_timeline(conn, &timeline_id)
            .map_err(|e| format!("Failed to delete timeline '{}': {}", timeline_id, e))
    })
}

// ---------------------------------------------------------
// UNIT TESTS
// ---------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timeline_api_lifecycle_in_memory() {
        let conn = Connection::open_in_memory().unwrap();
        db_tl::init_timeline_schema(&conn).unwrap();
        set_active_db_connection(conn);

        // 1. Create a new timeline
        let state = create_new_timeline("Main Feature".to_string(), 3840, 2160, 60, 1).unwrap();
        assert_eq!(state.timeline.name, "Main Feature");
        assert_eq!(state.timeline.width, 3840);
        assert_eq!(state.timeline.height, 2160);
        assert_eq!(state.timeline.fps_num, 60);
        assert_eq!(state.timeline.fps_den, 1);
        assert_eq!(state.tracks.len(), 2);
        assert_eq!(state.clips.len(), 0);

        let tl_id = state.timeline.id.clone();

        let v1 = state.tracks.iter().find(|t| t.is_video).unwrap();
        assert_eq!(v1.name, "Video 1");
        assert_eq!(v1.order_index, 0);

        let a1 = state.tracks.iter().find(|t| !t.is_video).unwrap();
        assert_eq!(a1.name, "Audio 1");
        assert_eq!(a1.order_index, 0);

        // 2. Add extra tracks
        let v2 = create_track(tl_id.clone(), true, "Video 2".to_string()).unwrap();
        assert_eq!(v2.name, "Video 2");
        assert_eq!(v2.order_index, 1);
        assert!(v2.is_video);

        let a2 = create_track(tl_id.clone(), false, "Audio 2".to_string()).unwrap();
        assert_eq!(a2.name, "Audio 2");
        assert_eq!(a2.order_index, 1);
        assert!(!a2.is_video);

        // 3. List timelines
        let timelines = get_project_timelines().unwrap();
        assert_eq!(timelines.len(), 1);
        assert_eq!(timelines[0].id, tl_id);

        // 4. Load timeline state
        let loaded_state = load_timeline_state(tl_id.clone()).unwrap();
        assert_eq!(loaded_state.timeline.id, tl_id);
        assert_eq!(loaded_state.tracks.len(), 4);

        // 5. Delete timeline
        delete_timeline_by_id(tl_id.clone()).unwrap();
        let empty_list = get_project_timelines().unwrap();
        assert_eq!(empty_list.len(), 0);

        let load_res = load_timeline_state(tl_id);
        assert!(load_res.is_err());

        clear_active_db_connection();
    }

    #[test]
    fn test_no_active_db_error() {
        clear_active_db_connection();
        let res = get_project_timelines();
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("No active project or SQLite database connection"));
    }
}
