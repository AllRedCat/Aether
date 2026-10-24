use rusqlite::{params, Connection, OptionalExtension, Result};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrackType {
    Video,
    Audio,
}

impl TrackType {
    pub fn as_str(&self) -> &'static str {
        match self {
            TrackType::Video => "Video",
            TrackType::Audio => "Audio",
        }
    }

    pub fn from_str(s: &str) -> std::result::Result<Self, String> {
        match s {
            "Video" | "video" => Ok(TrackType::Video),
            "Audio" | "audio" => Ok(TrackType::Audio),
            other => Err(format!("Invalid track type: '{}'", other)),
        }
    }
}

impl std::fmt::Display for TrackType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl rusqlite::ToSql for TrackType {
    fn to_sql(&self) -> Result<rusqlite::types::ToSqlOutput<'_>> {
        Ok(rusqlite::types::ToSqlOutput::from(self.as_str()))
    }
}

impl rusqlite::types::FromSql for TrackType {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        let s = value.as_str()?;
        TrackType::from_str(s).map_err(|e| {
            rusqlite::types::FromSqlError::Other(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                e,
            )))
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Timeline {
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Track {
    pub id: String,
    pub timeline_id: String,
    pub name: String,
    pub track_type: TrackType,
    pub order_index: i32,
    pub is_muted: bool,
    pub is_locked: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimelineClip {
    pub id: String,
    pub track_id: String,
    pub asset_id: String,
    pub timeline_start_frame: i64,
    pub in_frame: i64,
    pub out_frame: i64,
    pub speed: f64,
}

fn current_timestamp() -> String {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs();

    let days = (secs / 86400) as i64;
    let day_secs = (secs % 86400) as u32;

    let hour = day_secs / 3600;
    let min = (day_secs % 3600) / 60;
    let sec = day_secs % 60;

    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, m, d, hour, min, sec)
}

struct TxGuard<'a> {
    conn: &'a Connection,
    committed: bool,
}

impl<'a> TxGuard<'a> {
    fn begin(conn: &'a Connection) -> Result<Self> {
        conn.execute("BEGIN TRANSACTION", [])?;
        Ok(Self {
            conn,
            committed: false,
        })
    }

    fn commit(mut self) -> Result<()> {
        self.conn.execute("COMMIT", [])?;
        self.committed = true;
        Ok(())
    }
}

impl<'a> Drop for TxGuard<'a> {
    fn drop(&mut self) {
        if !self.committed {
            let _ = self.conn.execute("ROLLBACK", []);
        }
    }
}

/// Initializes the SQLite schema for timelines, tracks, and timeline_clips.
pub fn init_timeline_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        PRAGMA foreign_keys = ON;

        CREATE TABLE IF NOT EXISTS timelines (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            fps_num INTEGER NOT NULL,
            fps_den INTEGER NOT NULL,
            width INTEGER NOT NULL,
            height INTEGER NOT NULL,
            duration_frames INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS tracks (
            id TEXT PRIMARY KEY,
            timeline_id TEXT NOT NULL REFERENCES timelines(id) ON DELETE CASCADE,
            name TEXT NOT NULL,
            track_type TEXT NOT NULL CHECK (track_type IN ('Video', 'Audio')),
            order_index INTEGER NOT NULL,
            is_muted INTEGER NOT NULL DEFAULT 0,
            is_locked INTEGER NOT NULL DEFAULT 0
        );

        CREATE INDEX IF NOT EXISTS idx_tracks_timeline_id ON tracks(timeline_id);

        CREATE TABLE IF NOT EXISTS timeline_clips (
            id TEXT PRIMARY KEY,
            track_id TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
            asset_id TEXT NOT NULL,
            timeline_start_frame INTEGER NOT NULL,
            in_frame INTEGER NOT NULL,
            out_frame INTEGER NOT NULL,
            speed REAL NOT NULL DEFAULT 1.0
        );

        CREATE INDEX IF NOT EXISTS idx_timeline_clips_track_id ON timeline_clips(track_id);
        "#,
    )?;
    Ok(())
}

/// Creates a new timeline within a SQL transaction, automatically creating
/// a default "Video 1" track and "Audio 1" track (order_index: 0).
pub fn create_timeline(
    conn: &Connection,
    name: &str,
    fps_num: i32,
    fps_den: i32,
    width: u32,
    height: u32,
) -> Result<Timeline> {
    let tx = TxGuard::begin(conn)?;

    let now = current_timestamp();
    let timeline_id = Uuid::new_v4().to_string();

    let timeline = Timeline {
        id: timeline_id.clone(),
        name: name.to_string(),
        fps_num,
        fps_den,
        width,
        height,
        duration_frames: 0,
        created_at: now.clone(),
        updated_at: now,
    };

    conn.execute(
        "INSERT INTO timelines (id, name, fps_num, fps_den, width, height, duration_frames, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            timeline.id,
            timeline.name,
            timeline.fps_num,
            timeline.fps_den,
            timeline.width,
            timeline.height,
            timeline.duration_frames,
            timeline.created_at,
            timeline.updated_at,
        ],
    )?;

    // Default Video Track ("Video 1", order_index: 0)
    let video_track_id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO tracks (id, timeline_id, name, track_type, order_index, is_muted, is_locked)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            video_track_id,
            timeline_id,
            "Video 1",
            TrackType::Video.as_str(),
            0,
            0,
            0,
        ],
    )?;

    // Default Audio Track ("Audio 1", order_index: 0)
    let audio_track_id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO tracks (id, timeline_id, name, track_type, order_index, is_muted, is_locked)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            audio_track_id,
            timeline_id,
            "Audio 1",
            TrackType::Audio.as_str(),
            0,
            0,
            0,
        ],
    )?;

    tx.commit()?;

    Ok(timeline)
}

/// Lists all timelines in the database ordered by creation date.
pub fn list_timelines(conn: &Connection) -> Result<Vec<Timeline>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, fps_num, fps_den, width, height, duration_frames, created_at, updated_at
         FROM timelines
         ORDER BY created_at ASC, id ASC",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(Timeline {
            id: row.get(0)?,
            name: row.get(1)?,
            fps_num: row.get(2)?,
            fps_den: row.get(3)?,
            width: row.get(4)?,
            height: row.get(5)?,
            duration_frames: row.get(6)?,
            created_at: row.get(7)?,
            updated_at: row.get(8)?,
        })
    })?;

    let mut timelines = Vec::new();
    for t in rows {
        timelines.push(t?);
    }
    Ok(timelines)
}

/// Retrieves a timeline along with all of its tracks and clips.
pub fn get_timeline_with_tracks(
    conn: &Connection,
    timeline_id: &str,
) -> Result<(Timeline, Vec<Track>, Vec<TimelineClip>)> {
    let timeline = conn.query_row(
        "SELECT id, name, fps_num, fps_den, width, height, duration_frames, created_at, updated_at
         FROM timelines
         WHERE id = ?1",
        [timeline_id],
        |row| {
            Ok(Timeline {
                id: row.get(0)?,
                name: row.get(1)?,
                fps_num: row.get(2)?,
                fps_den: row.get(3)?,
                width: row.get(4)?,
                height: row.get(5)?,
                duration_frames: row.get(6)?,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
            })
        },
    )?;

    let mut track_stmt = conn.prepare(
        "SELECT id, timeline_id, name, track_type, order_index, is_muted, is_locked
         FROM tracks
         WHERE timeline_id = ?1
         ORDER BY order_index ASC, id ASC",
    )?;

    let track_rows = track_stmt.query_map([timeline_id], |row| {
        let is_muted_int: i32 = row.get(5)?;
        let is_locked_int: i32 = row.get(6)?;
        Ok(Track {
            id: row.get(0)?,
            timeline_id: row.get(1)?,
            name: row.get(2)?,
            track_type: row.get(3)?,
            order_index: row.get(4)?,
            is_muted: is_muted_int != 0,
            is_locked: is_locked_int != 0,
        })
    })?;

    let mut tracks = Vec::new();
    for tr in track_rows {
        tracks.push(tr?);
    }

    let mut clip_stmt = conn.prepare(
        "SELECT c.id, c.track_id, c.asset_id, c.timeline_start_frame, c.in_frame, c.out_frame, c.speed
         FROM timeline_clips c
         JOIN tracks t ON c.track_id = t.id
         WHERE t.timeline_id = ?1
         ORDER BY c.timeline_start_frame ASC, c.id ASC",
    )?;

    let clip_rows = clip_stmt.query_map([timeline_id], |row| {
        Ok(TimelineClip {
            id: row.get(0)?,
            track_id: row.get(1)?,
            asset_id: row.get(2)?,
            timeline_start_frame: row.get(3)?,
            in_frame: row.get(4)?,
            out_frame: row.get(5)?,
            speed: row.get(6)?,
        })
    })?;

    let mut clips = Vec::new();
    for cl in clip_rows {
        clips.push(cl?);
    }

    Ok((timeline, tracks, clips))
}

/// Adds a new track to a timeline, automatically computing the next order_index.
pub fn add_track(
    conn: &Connection,
    timeline_id: &str,
    track_type: TrackType,
    name: &str,
) -> Result<Track> {
    let exists: bool = conn
        .query_row(
            "SELECT 1 FROM timelines WHERE id = ?1",
            [timeline_id],
            |_| Ok(true),
        )
        .optional()?
        .unwrap_or(false);

    if !exists {
        return Err(rusqlite::Error::QueryReturnedNoRows);
    }

    let next_order: i32 = conn.query_row(
        "SELECT COALESCE(MAX(order_index) + 1, 0) FROM tracks WHERE timeline_id = ?1 AND track_type = ?2",
        params![timeline_id, track_type.as_str()],
        |r| r.get(0),
    )?;

    let track = Track {
        id: Uuid::new_v4().to_string(),
        timeline_id: timeline_id.to_string(),
        name: name.to_string(),
        track_type,
        order_index: next_order,
        is_muted: false,
        is_locked: false,
    };

    conn.execute(
        "INSERT INTO tracks (id, timeline_id, name, track_type, order_index, is_muted, is_locked)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            track.id,
            track.timeline_id,
            track.name,
            track.track_type.as_str(),
            track.order_index,
            track.is_muted as i32,
            track.is_locked as i32,
        ],
    )?;

    Ok(track)
}

/// Deletes a timeline by id. Foreign key CASCADE constraints automatically
/// remove its associated tracks and clips.
pub fn delete_timeline(conn: &Connection, timeline_id: &str) -> Result<()> {
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    conn.execute("DELETE FROM timelines WHERE id = ?1", [timeline_id])?;
    Ok(())
}

/// Helper function to add a clip to a track.
pub fn add_clip(
    conn: &Connection,
    track_id: &str,
    asset_id: &str,
    timeline_start_frame: i64,
    in_frame: i64,
    out_frame: i64,
    speed: f64,
) -> Result<TimelineClip> {
    let clip = TimelineClip {
        id: Uuid::new_v4().to_string(),
        track_id: track_id.to_string(),
        asset_id: asset_id.to_string(),
        timeline_start_frame,
        in_frame,
        out_frame,
        speed,
    };

    conn.execute(
        "INSERT INTO timeline_clips (id, track_id, asset_id, timeline_start_frame, in_frame, out_frame, speed)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            clip.id,
            clip.track_id,
            clip.asset_id,
            clip.timeline_start_frame,
            clip.in_frame,
            clip.out_frame,
            clip.speed,
        ],
    )?;

    Ok(clip)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_timeline_schema() {
        let conn = Connection::open_in_memory().unwrap();
        init_timeline_schema(&conn).unwrap();

        // Verify tables exist in sqlite_master
        let table_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('timelines', 'tracks', 'timeline_clips')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(table_count, 3);

        // Verify indexes exist
        let index_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name IN ('idx_tracks_timeline_id', 'idx_timeline_clips_track_id')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(index_count, 2);

        // Idempotency check: running schema initialization again should succeed
        assert!(init_timeline_schema(&conn).is_ok());
    }

    #[test]
    fn test_create_timeline_with_default_tracks() {
        let conn = Connection::open_in_memory().unwrap();
        init_timeline_schema(&conn).unwrap();

        let timeline = create_timeline(&conn, "4K 60fps Master", 60, 1, 3840, 2160).unwrap();
        assert_eq!(timeline.name, "4K 60fps Master");
        assert_eq!(timeline.fps_num, 60);
        assert_eq!(timeline.fps_den, 1);
        assert_eq!(timeline.width, 3840);
        assert_eq!(timeline.height, 2160);
        assert_eq!(timeline.duration_frames, 0);
        assert!(!timeline.id.is_empty());
        assert!(!timeline.created_at.is_empty());
        assert!(!timeline.updated_at.is_empty());

        let (tl, tracks, clips) = get_timeline_with_tracks(&conn, &timeline.id).unwrap();
        assert_eq!(tl, timeline);
        assert_eq!(tracks.len(), 2);
        assert_eq!(clips.len(), 0);

        let video_track = tracks.iter().find(|t| t.track_type == TrackType::Video).unwrap();
        assert_eq!(video_track.name, "Video 1");
        assert_eq!(video_track.order_index, 0);
        assert!(!video_track.is_muted);
        assert!(!video_track.is_locked);
        assert_eq!(video_track.timeline_id, timeline.id);

        let audio_track = tracks.iter().find(|t| t.track_type == TrackType::Audio).unwrap();
        assert_eq!(audio_track.name, "Audio 1");
        assert_eq!(audio_track.order_index, 0);
        assert!(!audio_track.is_muted);
        assert!(!audio_track.is_locked);
        assert_eq!(audio_track.timeline_id, timeline.id);
    }

    #[test]
    fn test_list_timelines() {
        let conn = Connection::open_in_memory().unwrap();
        init_timeline_schema(&conn).unwrap();

        let empty = list_timelines(&conn).unwrap();
        assert!(empty.is_empty());

        let tl1 = create_timeline(&conn, "Timeline 1", 30, 1, 1920, 1080).unwrap();
        let tl2 = create_timeline(&conn, "Timeline 2", 24, 1, 1280, 720).unwrap();

        let list = list_timelines(&conn).unwrap();
        assert_eq!(list.len(), 2);
        assert!(list.iter().any(|t| t.id == tl1.id && t.name == "Timeline 1"));
        assert!(list.iter().any(|t| t.id == tl2.id && t.name == "Timeline 2"));
    }

    #[test]
    fn test_add_track_and_order_index() {
        let conn = Connection::open_in_memory().unwrap();
        init_timeline_schema(&conn).unwrap();

        let tl = create_timeline(&conn, "Multi-Track", 60, 1, 1920, 1080).unwrap();

        // Default tracks already added: Video 1 (order 0) and Audio 1 (order 0)
        let v2 = add_track(&conn, &tl.id, TrackType::Video, "Video 2").unwrap();
        assert_eq!(v2.name, "Video 2");
        assert_eq!(v2.order_index, 1);
        assert_eq!(v2.track_type, TrackType::Video);

        let v3 = add_track(&conn, &tl.id, TrackType::Video, "Video 3").unwrap();
        assert_eq!(v3.name, "Video 3");
        assert_eq!(v3.order_index, 2);

        let a2 = add_track(&conn, &tl.id, TrackType::Audio, "Audio 2").unwrap();
        assert_eq!(a2.name, "Audio 2");
        assert_eq!(a2.order_index, 1);
        assert_eq!(a2.track_type, TrackType::Audio);

        // Add track to non-existent timeline should fail
        let err = add_track(&conn, "non-existent-timeline-id", TrackType::Video, "Video X");
        assert!(err.is_err());
    }

    #[test]
    fn test_cascade_deletion() {
        let conn = Connection::open_in_memory().unwrap();
        init_timeline_schema(&conn).unwrap();

        let tl = create_timeline(&conn, "To Delete", 60, 1, 1920, 1080).unwrap();
        let (_, tracks, _) = get_timeline_with_tracks(&conn, &tl.id).unwrap();
        let video_track = tracks.iter().find(|t| t.track_type == TrackType::Video).unwrap();
        let audio_track = tracks.iter().find(|t| t.track_type == TrackType::Audio).unwrap();

        // Add clips
        let _clip1 = add_clip(&conn, &video_track.id, "asset-v1", 0, 0, 120, 1.0).unwrap();
        let _clip2 = add_clip(&conn, &video_track.id, "asset-v2", 120, 0, 240, 1.0).unwrap();
        let _clip3 = add_clip(&conn, &audio_track.id, "asset-a1", 0, 0, 300, 1.0).unwrap();

        let (_, tracks_before, clips_before) = get_timeline_with_tracks(&conn, &tl.id).unwrap();
        assert_eq!(tracks_before.len(), 2);
        assert_eq!(clips_before.len(), 3);

        // Verify count queries
        let t_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM timelines WHERE id = ?1", [&tl.id], |r| r.get(0))
            .unwrap();
        let tr_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM tracks WHERE timeline_id = ?1", [&tl.id], |r| r.get(0))
            .unwrap();
        let c_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM timeline_clips", [], |r| r.get(0))
            .unwrap();
        assert_eq!(t_count, 1);
        assert_eq!(tr_count, 2);
        assert_eq!(c_count, 3);

        // Delete the timeline
        delete_timeline(&conn, &tl.id).unwrap();

        // Verify cascade deletion
        let t_count_after: i64 = conn
            .query_row("SELECT COUNT(*) FROM timelines WHERE id = ?1", [&tl.id], |r| r.get(0))
            .unwrap();
        let tr_count_after: i64 = conn
            .query_row("SELECT COUNT(*) FROM tracks WHERE timeline_id = ?1", [&tl.id], |r| r.get(0))
            .unwrap();
        let c_count_after: i64 = conn
            .query_row("SELECT COUNT(*) FROM timeline_clips", [], |r| r.get(0))
            .unwrap();

        assert_eq!(t_count_after, 0);
        assert_eq!(tr_count_after, 0);
        assert_eq!(c_count_after, 0);

        // get_timeline_with_tracks should fail with QueryReturnedNoRows
        let res = get_timeline_with_tracks(&conn, &tl.id);
        assert_eq!(res.unwrap_err(), rusqlite::Error::QueryReturnedNoRows);
    }

    #[test]
    fn test_track_cascade_clip_deletion() {
        let conn = Connection::open_in_memory().unwrap();
        init_timeline_schema(&conn).unwrap();

        let tl = create_timeline(&conn, "Partial Delete", 60, 1, 1920, 1080).unwrap();
        let (_, tracks, _) = get_timeline_with_tracks(&conn, &tl.id).unwrap();
        let video_track = tracks.iter().find(|t| t.track_type == TrackType::Video).unwrap();
        let audio_track = tracks.iter().find(|t| t.track_type == TrackType::Audio).unwrap();

        let clip_v = add_clip(&conn, &video_track.id, "asset-v", 0, 0, 60, 1.0).unwrap();
        let clip_a = add_clip(&conn, &audio_track.id, "asset-a", 0, 0, 60, 1.0).unwrap();

        // Delete video track
        conn.execute("DELETE FROM tracks WHERE id = ?1", [&video_track.id])
            .unwrap();

        let clip_v_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM timeline_clips WHERE id = ?1", [&clip_v.id], |r| r.get(0))
            .unwrap();
        let clip_a_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM timeline_clips WHERE id = ?1", [&clip_a.id], |r| r.get(0))
            .unwrap();
        assert_eq!(clip_v_count, 0);
        assert_eq!(clip_a_count, 1);
    }
}
