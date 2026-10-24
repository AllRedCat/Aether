//! SQLite persistence for `.aether` project files.
//!
//! The `.aether` file is a single SQLite database (rollback journal, so no
//! `-wal`/`-shm` sidecars are left next to the project). Legacy JSON projects
//! are detected on load and migrated transparently (the original file is kept
//! as `<name>.aether.json.bak`).
//!
//! Only descriptive/project metadata lives here. Per-frame packet indexes and
//! derived caches (waveforms, thumbnails) are binary sidecar files managed by
//! `aether_media::cache`.

use std::fs;
use std::io::Read;
use std::path::Path;
use std::time::Duration;

use rusqlite::{params, Connection, OpenFlags, OptionalExtension, Row, Transaction};
use uuid::Uuid;

use crate::media::{MediaItem, MediaMetadata, MediaPool, MediaTechInfo, MediaType};
use crate::project::Project;
use crate::timeline::{Clip, Rational, Timeline, Track, TrackKind};

/// `PRAGMA application_id` marker: ASCII "AETH".
pub const AETHER_APPLICATION_ID: i32 = 0x4145_5448;
/// Current schema version stored in `PRAGMA user_version`.
pub const SCHEMA_VERSION: i32 = 1;

const SQLITE_MAGIC: &[u8; 16] = b"SQLite format 3\0";
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

const SCHEMA_V1: &str = r#"
CREATE TABLE IF NOT EXISTS project_meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS media_assets (
    row                 INTEGER PRIMARY KEY,
    id                  TEXT NOT NULL,
    file_path           TEXT NOT NULL,
    file_name           TEXT NOT NULL,
    media_type          TEXT NOT NULL CHECK (media_type IN ('Video','Audio','Image')),
    file_size_bytes     INTEGER NOT NULL DEFAULT 0,
    width               INTEGER,
    height              INTEGER,
    duration_pts        INTEGER NOT NULL DEFAULT 0,
    duration_seconds    REAL    NOT NULL DEFAULT 0,
    timebase_num        INTEGER,
    timebase_den        INTEGER,
    audio_channels      INTEGER,
    sample_rate         INTEGER,
    video_codec         TEXT,
    audio_codec         TEXT,
    pixel_format        TEXT,
    is_vfr              INTEGER,
    -- deep inspection (MediaTechInfo); has_tech = 0 means `tech: None`
    has_tech            INTEGER NOT NULL DEFAULT 0,
    content_hash        TEXT,
    container           TEXT,
    coded_width         INTEGER,
    coded_height        INTEGER,
    display_width       INTEGER,
    display_height      INTEGER,
    bit_depth           INTEGER,
    track_timebase_num  INTEGER,
    track_timebase_den  INTEGER,
    channel_layout      TEXT,
    video_packet_count  INTEGER,
    keyframe_count      INTEGER
);
CREATE INDEX IF NOT EXISTS idx_media_assets_id   ON media_assets(id);
CREATE INDEX IF NOT EXISTS idx_media_assets_hash ON media_assets(content_hash);

CREATE TABLE IF NOT EXISTS timelines (
    id           TEXT PRIMARY KEY,
    timebase_num INTEGER NOT NULL,
    timebase_den INTEGER NOT NULL,
    duration_pts INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS tracks (
    row         INTEGER PRIMARY KEY,
    timeline_id TEXT NOT NULL REFERENCES timelines(id) ON DELETE CASCADE,
    id          TEXT NOT NULL,
    kind        TEXT NOT NULL CHECK (kind IN ('Video','Audio','Overlay')),
    position    INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS clips (
    row          INTEGER PRIMARY KEY,
    track_row    INTEGER NOT NULL REFERENCES tracks(row) ON DELETE CASCADE,
    id           TEXT NOT NULL,
    source_id    TEXT NOT NULL,
    source_in    INTEGER NOT NULL,
    source_out   INTEGER NOT NULL,
    timeline_in  INTEGER NOT NULL,
    timeline_out INTEGER NOT NULL,
    position     INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_clips_source ON clips(source_id);
"#;

const MEDIA_ASSETS_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS media_assets (
    row                 INTEGER PRIMARY KEY,
    id                  TEXT NOT NULL,
    file_path           TEXT NOT NULL,
    file_name           TEXT NOT NULL,
    media_type          TEXT NOT NULL CHECK (media_type IN ('Video','Audio','Image')),
    file_size_bytes     INTEGER NOT NULL DEFAULT 0,
    width               INTEGER,
    height              INTEGER,
    duration_pts        INTEGER NOT NULL DEFAULT 0,
    duration_seconds    REAL    NOT NULL DEFAULT 0,
    timebase_num        INTEGER,
    timebase_den        INTEGER,
    audio_channels      INTEGER,
    sample_rate         INTEGER,
    video_codec         TEXT,
    audio_codec         TEXT,
    pixel_format        TEXT,
    is_vfr              INTEGER,
    -- deep inspection (MediaTechInfo); has_tech = 0 means `tech: None`
    has_tech            INTEGER NOT NULL DEFAULT 0,
    content_hash        TEXT,
    container           TEXT,
    coded_width         INTEGER,
    coded_height        INTEGER,
    display_width       INTEGER,
    display_height      INTEGER,
    bit_depth           INTEGER,
    track_timebase_num  INTEGER,
    track_timebase_den  INTEGER,
    channel_layout      TEXT,
    video_packet_count  INTEGER,
    keyframe_count      INTEGER
);
CREATE INDEX IF NOT EXISTS idx_media_assets_id   ON media_assets(id);
CREATE INDEX IF NOT EXISTS idx_media_assets_hash ON media_assets(content_hash);
"#;

/// Resolves a path that may point either to a `.aether` bundle directory or directly to a database file.
fn resolve_db_file_path(file_path: &str) -> String {
    let p = Path::new(file_path);
    if p.is_dir() {
        return p.join("project.db").to_string_lossy().to_string();
    }
    file_path.to_string()
}

/// Checks whether a given table in SQLite contains a specific column.
fn table_has_column(conn: &Connection, table: &str, col_name: &str) -> Result<bool, String> {
    let mut stmt = conn
        .prepare(&format!("PRAGMA table_info({})", table))
        .map_err(sql_err)?;
    let cols = stmt
        .query_map([], |r| r.get::<_, String>(1))
        .map_err(sql_err)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(sql_err)?;
    Ok(cols.iter().any(|c| c.eq_ignore_ascii_case(col_name)))
}

/// On-disk format detected from the file header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectFileFormat {
    Sqlite,
    LegacyJson,
    Unknown,
}

/// Sniffs the first bytes of a project file.
pub fn detect_format(file_path: &str) -> Result<ProjectFileFormat, String> {
    let resolved = resolve_db_file_path(file_path);
    let mut f = fs::File::open(&resolved)
        .map_err(|e| format!("Falha ao ler o arquivo .aether: {}", e))?;
    let mut head = [0u8; 16];
    let mut read = 0;
    while read < head.len() {
        match f.read(&mut head[read..]) {
            Ok(0) => break,
            Ok(n) => read += n,
            Err(e) => return Err(format!("Falha ao ler o arquivo .aether: {}", e)),
        }
    }
    if read == 16 && &head == SQLITE_MAGIC {
        return Ok(ProjectFileFormat::Sqlite);
    }
    let first = head[..read].iter().find(|b| !b.is_ascii_whitespace());
    // UTF-8 BOM (EF BB BF) followed by '{' is also accepted as JSON.
    if first == Some(&b'{') || head[..read].starts_with(&[0xEF, 0xBB, 0xBF]) {
        return Ok(ProjectFileFormat::LegacyJson);
    }
    Ok(ProjectFileFormat::Unknown)
}

fn open_connection(file_path: &str, create: bool) -> Result<Connection, String> {
    let resolved = resolve_db_file_path(file_path);
    let mut flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    if create {
        flags |= OpenFlags::SQLITE_OPEN_CREATE;
    }
    let conn = Connection::open_with_flags(&resolved, flags)
        .map_err(|e| format!("Falha ao abrir banco do projeto: {}", e))?;
    conn.busy_timeout(BUSY_TIMEOUT)
        .map_err(|e| format!("Falha ao configurar banco do projeto: {}", e))?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")
        .map_err(|e| format!("Falha ao configurar banco do projeto: {}", e))?;
    Ok(conn)
}

fn ensure_schema(conn: &Connection) -> Result<(), String> {
    let version: i32 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(sql_err)?;
    if version > SCHEMA_VERSION {
        return Err(format!(
            "Projeto criado por uma versão mais nova do Aether (schema {} > {})",
            version, SCHEMA_VERSION
        ));
    }

    // Check if this database was initialized by ProjectBundle (columnar project_meta)
    // or db::timeline (timelines with fps_num).
    let is_bundle_meta = table_has_column(conn, "project_meta", "created_at")?;
    let is_domain_timeline = table_has_column(conn, "timelines", "fps_num")?;

    if is_bundle_meta || is_domain_timeline {
        conn.execute_batch(MEDIA_ASSETS_SCHEMA).map_err(sql_err)?;
        if !table_has_column(conn, "timelines", "timebase_num")? {
            crate::db::timeline::init_timeline_schema(conn).map_err(sql_err)?;
        }
        return Ok(());
    }

    if version < SCHEMA_VERSION {
        conn.execute_batch(SCHEMA_V1).map_err(sql_err)?;
        conn.execute_batch(&format!(
            "PRAGMA application_id = {}; PRAGMA user_version = {};",
            AETHER_APPLICATION_ID, SCHEMA_VERSION
        ))
        .map_err(sql_err)?;
    } else {
        conn.execute_batch(MEDIA_ASSETS_SCHEMA).map_err(sql_err)?;
    }
    Ok(())
}

fn sql_err(e: rusqlite::Error) -> String {
    format!("Erro no banco do projeto: {}", e)
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Writes the full project state to `project.file_path` in one transaction.
///
/// The timeline is replaced entirely; the media pool is synchronized (rows not
/// present in `project.media_pool` are removed). If the target file is a legacy
/// JSON project it is first moved aside to `<file>.json.bak`.
pub fn save_project(project: &Project) -> Result<(), String> {
    let resolved_path = resolve_db_file_path(project.file_path.as_str());
    let path = resolved_path.as_str();
    if Path::new(path).exists() {
        match detect_format(path)? {
            ProjectFileFormat::Sqlite => {}
            ProjectFileFormat::LegacyJson => backup_legacy_json(path)?,
            ProjectFileFormat::Unknown => {
                // Empty files (e.g. just touched) are safe to overwrite.
                let len = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
                if len != 0 {
                    return Err(format!(
                        "Falha ao escrever arquivo no disco: '{}' não é um projeto Aether",
                        path
                    ));
                }
                fs::remove_file(path)
                    .map_err(|e| format!("Falha ao escrever arquivo no disco: {}", e))?;
            }
        }
    }
    if let Some(parent) = Path::new(path).parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            return Err(format!(
                "Falha ao escrever arquivo no disco: diretório inexistente '{}'",
                parent.display()
            ));
        }
    }

    let mut conn = open_connection(path, true)
        .map_err(|e| format!("Falha ao escrever arquivo no disco: {}", e))?;
    ensure_schema(&conn)?;
    let tx = conn.transaction().map_err(sql_err)?;
    write_meta(&tx, project)?;
    write_timeline(&tx, &project.timeline)?;
    tx.execute("DELETE FROM media_assets", []).map_err(sql_err)?;
    for item in &project.media_pool.items {
        insert_media_row(&tx, item)?;
    }
    tx.commit().map_err(sql_err)?;
    Ok(())
}

/// Loads a project. Legacy JSON files are migrated to SQLite in place.
pub fn load_project(file_path: &str) -> Result<Project, String> {
    let resolved = resolve_db_file_path(file_path);
    match detect_format(&resolved)? {
        ProjectFileFormat::Sqlite => load_sqlite(&resolved),
        ProjectFileFormat::LegacyJson => {
            let json = fs::read_to_string(&resolved)
                .map_err(|e| format!("Falha ao ler o arquivo .aether: {}", e))?;
            let json = json.trim_start_matches('\u{feff}');
            let mut project: Project = serde_json::from_str(json)
                .map_err(|e| format!("Falha ao decodificar projeto corrompido: {}", e))?;
            relocate(&mut project, &resolved);
            // The per-frame keyframe list is no longer persisted.
            for item in &mut project.media_pool.items {
                item.metadata.keyframe_pts = None;
            }
            // Best-effort migration: a read-only location must not block opening.
            if let Err(e) = save_project(&project) {
                eprintln!("[aether_core] migração JSON→SQLite falhou para '{}': {}", resolved, e);
            }
            Ok(project)
        }
        ProjectFileFormat::Unknown => Err(format!(
            "Falha ao decodificar projeto corrompido: '{}' não é um projeto Aether",
            file_path
        )),
    }
}

/// Appends a single media item without rewriting the rest of the project.
/// Returns `false` if an item with the same id already exists.
pub fn insert_media(file_path: &str, item: &MediaItem) -> Result<bool, String> {
    ensure_sqlite(file_path)?;
    let mut conn = open_connection(file_path, false)?;
    ensure_schema(&conn)?;
    let tx = conn.transaction().map_err(sql_err)?;
    let exists: Option<i64> = tx
        .query_row(
            "SELECT row FROM media_assets WHERE id = ?1 LIMIT 1",
            [item.id.to_string()],
            |r| r.get(0),
        )
        .optional()
        .map_err(sql_err)?;
    if exists.is_some() {
        return Ok(false);
    }
    insert_media_row(&tx, item)?;
    tx.commit().map_err(sql_err)?;
    Ok(true)
}

/// Replaces the metadata of an existing media item (e.g. after deep inspection).
pub fn update_media(file_path: &str, item: &MediaItem) -> Result<bool, String> {
    ensure_sqlite(file_path)?;
    let mut conn = open_connection(file_path, false)?;
    ensure_schema(&conn)?;
    let tx = conn.transaction().map_err(sql_err)?;
    let row: Option<i64> = tx
        .query_row(
            "SELECT row FROM media_assets WHERE id = ?1 ORDER BY row LIMIT 1",
            [item.id.to_string()],
            |r| r.get(0),
        )
        .optional()
        .map_err(sql_err)?;
    let Some(row) = row else { return Ok(false) };
    tx.execute("DELETE FROM media_assets WHERE row = ?1", [row])
        .map_err(sql_err)?;
    insert_media_row_at(&tx, item, Some(row))?;
    tx.commit().map_err(sql_err)?;
    Ok(true)
}

/// Removes a media item by id. Returns the number of removed rows.
pub fn delete_media(file_path: &str, media_id: Uuid) -> Result<usize, String> {
    ensure_sqlite(file_path)?;
    let conn = open_connection(file_path, false)?;
    ensure_schema(&conn)?;
    conn.execute("DELETE FROM media_assets WHERE id = ?1", [media_id.to_string()])
        .map_err(sql_err)
}

/// Finds a media item already imported from the same content (hash match).
pub fn find_media_by_hash(file_path: &str, content_hash: &str) -> Result<Option<MediaItem>, String> {
    ensure_sqlite(file_path)?;
    let conn = open_connection(file_path, false)?;
    ensure_schema(&conn)?;
    conn.query_row(
        "SELECT * FROM media_assets WHERE content_hash = ?1 ORDER BY row LIMIT 1",
        [content_hash],
        read_media_row,
    )
    .optional()
    .map_err(sql_err)
}

/// Converts a legacy JSON project file into SQLite if necessary.
fn ensure_sqlite(file_path: &str) -> Result<(), String> {
    match detect_format(file_path)? {
        ProjectFileFormat::Sqlite => Ok(()),
        ProjectFileFormat::LegacyJson => {
            let project = load_project(file_path)?;
            if detect_format(file_path)? == ProjectFileFormat::Sqlite {
                Ok(())
            } else {
                save_project(&project)
            }
        }
        ProjectFileFormat::Unknown => Err(format!(
            "Falha ao decodificar projeto corrompido: '{}' não é um projeto Aether",
            file_path
        )),
    }
}

// ---------------------------------------------------------------------------
// Internals
// ---------------------------------------------------------------------------

/// If the project was moved, trust the actual location over stored paths.
fn relocate(project: &mut Project, actual_file_path: &str) {
    if project.file_path != actual_file_path {
        project.file_path = actual_file_path.to_string();
        project.project_path = Path::new(actual_file_path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
    }
}

fn backup_legacy_json(path: &str) -> Result<(), String> {
    let mut backup = format!("{}.json.bak", path);
    let mut n = 1;
    while Path::new(&backup).exists() {
        backup = format!("{}.json.{}.bak", path, n);
        n += 1;
    }
    fs::rename(path, &backup)
        .map_err(|e| format!("Falha ao criar backup do projeto JSON legado: {}", e))
}

fn write_meta(tx: &Transaction, p: &Project) -> Result<(), String> {
    if table_has_column(tx, "project_meta", "key")? {
        let mut stmt = tx
            .prepare("INSERT OR REPLACE INTO project_meta(key, value) VALUES (?1, ?2)")
            .map_err(sql_err)?;
        for (k, v) in [
            ("id", p.id.as_str()),
            ("name", p.name.as_str()),
            ("project_path", p.project_path.as_str()),
            ("file_path", p.file_path.as_str()),
            ("timeline_id", &p.timeline.id.to_string()),
        ] {
            stmt.execute(params![k, v]).map_err(sql_err)?;
        }
    } else {
        // ProjectBundle columnar format: (id, name, created_at, updated_at)
        let existing_created_at: Option<String> = tx
            .query_row(
                "SELECT created_at FROM project_meta WHERE id = ?1 LIMIT 1",
                [&p.id],
                |r| r.get(0),
            )
            .optional()
            .map_err(sql_err)?;
        let now: String = tx
            .query_row("SELECT datetime('now')", [], |r| r.get(0))
            .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string());
        let created_at = existing_created_at.unwrap_or_else(|| now.clone());
        tx.execute(
            "INSERT OR REPLACE INTO project_meta (id, name, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
            params![p.id, p.name, created_at, now],
        )
        .map_err(sql_err)?;
    }
    Ok(())
}

fn write_timeline(tx: &Transaction, tl: &Timeline) -> Result<(), String> {
    if table_has_column(tx, "timelines", "fps_num")? {
        // Domain timeline schema (db::timeline): timelines(id, name, fps_num, fps_den, width, height, duration_frames, created_at, updated_at)
        let tl_id_str = tl.id.to_string();
        let existing: Option<(String, u32, u32, String)> = tx
            .query_row(
                "SELECT name, width, height, created_at FROM timelines WHERE id = ?1 LIMIT 1",
                [&tl_id_str],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()
            .map_err(sql_err)?;

        let total_timelines: i64 = tx
            .query_row("SELECT COUNT(*) FROM timelines", [], |r| r.get(0))
            .unwrap_or(0);

        // If the in-memory project timeline has no tracks and there are already domain timelines in the DB,
        // do not overwrite or create an empty duplicate timeline.
        if tl.tracks.is_empty() && existing.is_none() && total_timelines > 0 {
            return Ok(());
        }

        let now: String = tx
            .query_row("SELECT datetime('now')", [], |r| r.get(0))
            .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string());

        if let Some((name, width, height, _created_at)) = existing {
            tx.execute(
                "UPDATE timelines SET name = ?2, fps_num = ?3, fps_den = ?4, width = ?5, height = ?6, duration_frames = ?7, updated_at = ?8 WHERE id = ?1",
                params![tl_id_str, name, tl.timebase.num, tl.timebase.den, width, height, tl.duration_pts, now],
            )
            .map_err(sql_err)?;
        } else if !tl.tracks.is_empty() {
            tx.execute(
                "INSERT INTO timelines (id, name, fps_num, fps_den, width, height, duration_frames, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![tl_id_str, "Timeline 1", tl.timebase.num, tl.timebase.den, 1920u32, 1080u32, tl.duration_pts, now, now],
            )
            .map_err(sql_err)?;
        } else {
            return Ok(());
        }

        if !tl.tracks.is_empty() {
            for (ti, track) in tl.tracks.iter().enumerate() {
                let track_id_str = track.id.to_string();
                let t_type = match track.kind {
                    TrackKind::Audio => "Audio",
                    _ => "Video",
                };
                let existing_track: Option<(String, i32, i32)> = tx
                    .query_row(
                        "SELECT name, is_muted, is_locked FROM tracks WHERE id = ?1 LIMIT 1",
                        [&track_id_str],
                        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                    )
                    .optional()
                    .map_err(sql_err)?;

                let (t_name, muted, locked) = existing_track.unwrap_or_else(|| {
                    (format!("{} {}", t_type, ti + 1), 0, 0)
                });

                tx.execute(
                    "INSERT OR REPLACE INTO tracks (id, timeline_id, name, track_type, order_index, is_muted, is_locked)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![track_id_str, tl_id_str, t_name, t_type, ti as i32, muted, locked],
                )
                .map_err(sql_err)?;

                tx.execute("DELETE FROM timeline_clips WHERE track_id = ?1", [&track_id_str])
                    .map_err(sql_err)?;

                for c in &track.clips {
                    tx.execute(
                        "INSERT INTO timeline_clips (id, track_id, asset_id, timeline_start_frame, in_frame, out_frame, speed)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1.0)",
                        params![
                            c.id.to_string(),
                            track_id_str,
                            c.source_id.to_string(),
                            c.timeline_in,
                            c.source_in,
                            c.source_out,
                        ],
                    )
                    .map_err(sql_err)?;
                }
            }
        }
        return Ok(());
    }

    // Legacy / SCHEMA_V1 format: timelines(id, timebase_num, timebase_den, duration_pts)
    // Cascades remove tracks and clips.
    tx.execute("DELETE FROM timelines", []).map_err(sql_err)?;
    tx.execute(
        "INSERT INTO timelines(id, timebase_num, timebase_den, duration_pts) VALUES (?1, ?2, ?3, ?4)",
        params![tl.id.to_string(), tl.timebase.num, tl.timebase.den, tl.duration_pts],
    )
    .map_err(sql_err)?;
    let mut ins_track = tx
        .prepare("INSERT INTO tracks(timeline_id, id, kind, position) VALUES (?1, ?2, ?3, ?4)")
        .map_err(sql_err)?;
    let mut ins_clip = tx
        .prepare(
            "INSERT INTO clips(track_row, id, source_id, source_in, source_out, timeline_in, timeline_out, position)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )
        .map_err(sql_err)?;
    for (ti, track) in tl.tracks.iter().enumerate() {
        ins_track
            .execute(params![tl.id.to_string(), track.id.to_string(), track_kind_str(track.kind), ti as i64])
            .map_err(sql_err)?;
        let track_row = tx.last_insert_rowid();
        for (ci, c) in track.clips.iter().enumerate() {
            ins_clip
                .execute(params![
                    track_row,
                    c.id.to_string(),
                    c.source_id.to_string(),
                    c.source_in,
                    c.source_out,
                    c.timeline_in,
                    c.timeline_out,
                    ci as i64
                ])
                .map_err(sql_err)?;
        }
    }
    Ok(())
}

fn insert_media_row(tx: &Transaction, item: &MediaItem) -> Result<(), String> {
    insert_media_row_at(tx, item, None)
}

fn insert_media_row_at(tx: &Transaction, item: &MediaItem, row: Option<i64>) -> Result<(), String> {
    let m = &item.metadata;
    let t = m.tech.as_ref();
    tx.execute(
        "INSERT INTO media_assets(
            row, id, file_path, file_name, media_type, file_size_bytes, width, height,
            duration_pts, duration_seconds, timebase_num, timebase_den, audio_channels, sample_rate,
            video_codec, audio_codec, pixel_format, is_vfr,
            has_tech, content_hash, container, coded_width, coded_height, display_width, display_height,
            bit_depth, track_timebase_num, track_timebase_den, channel_layout, video_packet_count, keyframe_count
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18,
                  ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31)",
        params![
            row,
            item.id.to_string(),
            item.file_path,
            item.file_name,
            item.media_type.to_string(),
            // SQLite integers are i64; sizes beyond i64::MAX are clamped.
            i64::try_from(m.file_size_bytes).unwrap_or(i64::MAX),
            m.width,
            m.height,
            m.duration_pts,
            m.duration_seconds,
            m.timebase.map(|r| r.num),
            m.timebase.map(|r| r.den),
            m.audio_channels,
            m.sample_rate,
            m.video_codec,
            m.audio_codec,
            m.pixel_format,
            m.is_vfr,
            t.is_some(),
            t.map(|t| t.content_hash.as_str()),
            t.and_then(|t| t.container.as_deref()),
            t.and_then(|t| t.coded_width),
            t.and_then(|t| t.coded_height),
            t.and_then(|t| t.display_width),
            t.and_then(|t| t.display_height),
            t.and_then(|t| t.bit_depth),
            t.and_then(|t| t.track_timebase).map(|r| r.num),
            t.and_then(|t| t.track_timebase).map(|r| r.den),
            t.and_then(|t| t.channel_layout.as_deref()),
            t.and_then(|t| t.video_packet_count),
            t.and_then(|t| t.keyframe_count),
        ],
    )
    .map_err(sql_err)?;
    Ok(())
}

fn rational(num: Option<i32>, den: Option<i32>) -> Option<Rational> {
    match (num, den) {
        (Some(num), Some(den)) => Some(Rational { num, den }),
        _ => None,
    }
}

fn parse_uuid(s: &str, what: &str) -> rusqlite::Result<Uuid> {
    Uuid::parse_str(s).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(
            0,
            rusqlite::types::Type::Text,
            format!("{} inválido '{}': {}", what, s, e).into(),
        )
    })
}

fn conversion_err(msg: String) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, msg.into())
}

fn read_media_row(r: &Row) -> rusqlite::Result<MediaItem> {
    let id: String = r.get("id")?;
    let media_type: String = r.get("media_type")?;
    let media_type = match media_type.as_str() {
        "Video" => MediaType::Video,
        "Audio" => MediaType::Audio,
        "Image" => MediaType::Image,
        other => return Err(conversion_err(format!("media_type inválido '{}'", other))),
    };
    let size: i64 = r.get("file_size_bytes")?;
    let has_tech: bool = r.get("has_tech")?;
    let tech = if has_tech {
        Some(MediaTechInfo {
            content_hash: r.get::<_, Option<String>>("content_hash")?.unwrap_or_default(),
            container: r.get("container")?,
            coded_width: r.get("coded_width")?,
            coded_height: r.get("coded_height")?,
            display_width: r.get("display_width")?,
            display_height: r.get("display_height")?,
            bit_depth: r.get("bit_depth")?,
            track_timebase: rational(r.get("track_timebase_num")?, r.get("track_timebase_den")?),
            channel_layout: r.get("channel_layout")?,
            video_packet_count: r.get("video_packet_count")?,
            keyframe_count: r.get("keyframe_count")?,
        })
    } else {
        None
    };
    Ok(MediaItem::with_id(
        parse_uuid(&id, "media id")?,
        r.get("file_path")?,
        r.get("file_name")?,
        media_type,
        MediaMetadata {
            width: r.get("width")?,
            height: r.get("height")?,
            duration_pts: r.get("duration_pts")?,
            duration_seconds: r.get("duration_seconds")?,
            timebase: rational(r.get("timebase_num")?, r.get("timebase_den")?),
            audio_channels: r.get("audio_channels")?,
            sample_rate: r.get("sample_rate")?,
            file_size_bytes: size.max(0) as u64,
            video_codec: r.get("video_codec")?,
            audio_codec: r.get("audio_codec")?,
            pixel_format: r.get("pixel_format")?,
            is_vfr: r.get("is_vfr")?,
            keyframe_pts: None,
            tech,
        },
    ))
}

fn track_kind_str(k: TrackKind) -> &'static str {
    match k {
        TrackKind::Video => "Video",
        TrackKind::Audio => "Audio",
        TrackKind::Overlay => "Overlay",
    }
}

fn parse_track_kind(s: &str) -> rusqlite::Result<TrackKind> {
    match s {
        "Video" => Ok(TrackKind::Video),
        "Audio" => Ok(TrackKind::Audio),
        "Overlay" => Ok(TrackKind::Overlay),
        other => Err(conversion_err(format!("track kind inválido '{}'", other))),
    }
}

fn load_sqlite(file_path: &str) -> Result<Project, String> {
    let conn = open_connection(file_path, false)
        .map_err(|e| format!("Falha ao ler o arquivo .aether: {}", e))?;
    ensure_schema(&conn)?;
    let decode = |e: rusqlite::Error| format!("Falha ao decodificar projeto corrompido: {}", e);

    let (id, name, project_path, stored_file_path) = if table_has_column(&conn, "project_meta", "key")? {
        let meta = |key: &str| -> Result<Option<String>, String> {
            conn.query_row("SELECT value FROM project_meta WHERE key = ?1", [key], |r| r.get(0))
                .optional()
                .map_err(decode)
        };
        let id = meta("id")?.ok_or("Falha ao decodificar projeto corrompido: id ausente")?;
        let name = meta("name")?.ok_or("Falha ao decodificar projeto corrompido: nome ausente")?;
        let project_path = meta("project_path")?.unwrap_or_default();
        let stored_file_path = meta("file_path")?.unwrap_or_default();
        (id, name, project_path, stored_file_path)
    } else {
        // ProjectBundle columnar format: (id, name, created_at, updated_at)
        let row: Option<(String, String)> = conn
            .query_row("SELECT id, name FROM project_meta LIMIT 1", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .optional()
            .map_err(decode)?;
        let (id, name) = row.ok_or("Falha ao decodificar projeto corrompido: project_meta vazio")?;
        let proj_dir = Path::new(file_path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        (id, name, proj_dir, file_path.to_string())
    };

    let timeline = if table_has_column(&conn, "timelines", "fps_num")? {
        // Domain timeline schema (db::timeline)
        let tl_opt: Option<(String, i32, i32, i64)> = conn
            .query_row(
                "SELECT id, fps_num, fps_den, duration_frames FROM timelines ORDER BY created_at ASC, id ASC LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()
            .map_err(decode)?;

        if let Some((tl_id, num, den, duration_frames)) = tl_opt {
            let mut timeline = Timeline {
                id: parse_uuid(&tl_id, "timeline id").unwrap_or_else(|_| Uuid::new_v4()),
                timebase: Rational {
                    num: if num > 0 { num } else { 60 },
                    den: if den > 0 { den } else { 1 },
                },
                duration_pts: duration_frames,
                tracks: Vec::new(),
            };

            let mut tracks_stmt = conn
                .prepare("SELECT id, track_type FROM tracks WHERE timeline_id = ?1 ORDER BY order_index ASC, id ASC")
                .map_err(decode)?;
            let mut clips_stmt = conn
                .prepare(
                    "SELECT id, asset_id, in_frame, out_frame, timeline_start_frame
                     FROM timeline_clips WHERE track_id = ?1 ORDER BY timeline_start_frame ASC, id ASC",
                )
                .map_err(decode)?;

            let track_rows = tracks_stmt
                .query_map([&tl_id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
                .map_err(decode)?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(decode)?;

            let mut max_out = duration_frames;
            for (tid, kind_str) in track_rows {
                let clips = clips_stmt
                    .query_map([&tid], |r| {
                        let cid: String = r.get(0)?;
                        let sid: String = r.get(1)?;
                        let s_in: i64 = r.get(2)?;
                        let s_out: i64 = r.get(3)?;
                        let t_in: i64 = r.get(4)?;
                        let t_out = t_in.saturating_add(s_out.saturating_sub(s_in));
                        Ok(Clip {
                            id: parse_uuid(&cid, "clip id").unwrap_or_else(|_| Uuid::new_v4()),
                            source_id: parse_uuid(&sid, "source id").unwrap_or_else(|_| Uuid::new_v4()),
                            source_in: s_in,
                            source_out: s_out,
                            timeline_in: t_in,
                            timeline_out: t_out,
                        })
                    })
                    .map_err(decode)?
                    .collect::<rusqlite::Result<Vec<_>>>()
                    .map_err(decode)?;

                for c in &clips {
                    if c.timeline_out > max_out {
                        max_out = c.timeline_out;
                    }
                }

                timeline.tracks.push(Track {
                    id: parse_uuid(&tid, "track id").unwrap_or_else(|_| Uuid::new_v4()),
                    kind: parse_track_kind(&kind_str).unwrap_or(TrackKind::Video),
                    clips,
                });
            }
            timeline.duration_pts = max_out;
            timeline
        } else {
            Timeline::new(Rational { num: 60, den: 1 })
        }
    } else {
        // Legacy / SCHEMA_V1 format
        let tl_opt: Option<(String, i32, i32, i64)> = conn
            .query_row(
                "SELECT id, timebase_num, timebase_den, duration_pts FROM timelines LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()
            .map_err(decode)?;

        if let Some((tl_id, num, den, duration_pts)) = tl_opt {
            let mut timeline = Timeline {
                id: parse_uuid(&tl_id, "timeline id").map_err(decode)?,
                timebase: Rational { num, den },
                duration_pts,
                tracks: Vec::new(),
            };

            let mut tracks_stmt = conn
                .prepare("SELECT row, id, kind FROM tracks WHERE timeline_id = ?1 ORDER BY position, row")
                .map_err(decode)?;
            let mut clips_stmt = conn
                .prepare(
                    "SELECT id, source_id, source_in, source_out, timeline_in, timeline_out
                     FROM clips WHERE track_row = ?1 ORDER BY position, row",
                )
                .map_err(decode)?;
            let track_rows = tracks_stmt
                .query_map([&tl_id], |r| {
                    Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
                })
                .map_err(decode)?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(decode)?;
            for (row, tid, kind) in track_rows {
                let clips = clips_stmt
                    .query_map([row], |r| {
                        let cid: String = r.get(0)?;
                        let sid: String = r.get(1)?;
                        Ok(Clip {
                            id: parse_uuid(&cid, "clip id")?,
                            source_id: parse_uuid(&sid, "source id")?,
                            source_in: r.get(2)?,
                            source_out: r.get(3)?,
                            timeline_in: r.get(4)?,
                            timeline_out: r.get(5)?,
                        })
                    })
                    .map_err(decode)?
                    .collect::<rusqlite::Result<Vec<_>>>()
                    .map_err(decode)?;
                timeline.tracks.push(Track {
                    id: parse_uuid(&tid, "track id").map_err(decode)?,
                    kind: parse_track_kind(&kind).map_err(decode)?,
                    clips,
                });
            }
            timeline
        } else {
            Timeline::new(Rational { num: 60, den: 1 })
        }
    };

    let mut media_stmt = conn
        .prepare("SELECT * FROM media_assets ORDER BY row")
        .map_err(decode)?;
    let items = media_stmt
        .query_map([], read_media_row)
        .map_err(decode)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(decode)?;

    let mut project = Project {
        id,
        name,
        project_path,
        file_path: stored_file_path,
        timeline,
        media_pool: MediaPool { items },
    };
    relocate(&mut project, file_path);
    Ok(project)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::{MediaItem, MediaMetadata, MediaTechInfo, MediaType};

    fn tmp_project(name: &str) -> (tempfile::TempDir, Project) {
        let dir = tempfile::tempdir().unwrap();
        let p = Project::create_new(name, dir.path().to_str().unwrap()).unwrap();
        (dir, p)
    }

    fn item_with_tech(path: &str) -> MediaItem {
        MediaItem::new(
            path.to_string(),
            MediaType::Video,
            MediaMetadata {
                width: Some(1920),
                height: Some(1080),
                duration_pts: 600,
                duration_seconds: 10.0,
                timebase: Some(Rational { num: 60, den: 1 }),
                audio_channels: Some(2),
                sample_rate: Some(48_000),
                file_size_bytes: 123_456,
                video_codec: Some("H.264".into()),
                audio_codec: Some("AAC".into()),
                pixel_format: Some("yuv420p".into()),
                is_vfr: Some(false),
                keyframe_pts: None,
                tech: Some(MediaTechInfo {
                    content_hash: "ab".repeat(32),
                    container: Some("mov".into()),
                    coded_width: Some(1920),
                    coded_height: Some(1088),
                    display_width: Some(1920),
                    display_height: Some(1080),
                    bit_depth: Some(10),
                    track_timebase: Some(Rational { num: 1, den: 600 }),
                    channel_layout: Some("stereo".into()),
                    video_packet_count: Some(600),
                    keyframe_count: Some(20),
                }),
            },
        )
    }

    #[test]
    fn new_project_is_sqlite_with_app_id() {
        let (_d, p) = tmp_project("Sqlite Check");
        assert_eq!(detect_format(&p.file_path).unwrap(), ProjectFileFormat::Sqlite);
        let conn = Connection::open(&p.file_path).unwrap();
        let app_id: i32 = conn.query_row("PRAGMA application_id", [], |r| r.get(0)).unwrap();
        let ver: i32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(app_id, AETHER_APPLICATION_ID);
        assert_eq!(ver, SCHEMA_VERSION);
    }

    #[test]
    fn roundtrip_preserves_tech_info_and_timeline() {
        let (_d, mut p) = tmp_project("Roundtrip");
        let id = p.add_media(item_with_tech("/m/a.mov"));
        let track = p.timeline.add_track(TrackKind::Video);
        p.add_clip_from_media(track, id, Some(10), Some(300), Some(0)).unwrap();
        p.save().unwrap();
        let loaded = Project::load(&p.file_path).unwrap();
        assert_eq!(loaded, p);
        assert_eq!(loaded.media_pool.items[0].metadata.tech.as_ref().unwrap().bit_depth, Some(10));
    }

    #[test]
    fn keyframe_list_is_not_persisted() {
        let (_d, mut p) = tmp_project("No Keyframes");
        let mut item = item_with_tech("/m/k.mp4");
        item.metadata.keyframe_pts = Some(vec![0, 30, 60]);
        p.add_media(item);
        p.save().unwrap();
        let loaded = Project::load(&p.file_path).unwrap();
        assert_eq!(loaded.media_pool.items[0].metadata.keyframe_pts, None);
    }

    #[test]
    fn insert_update_delete_media_granular() {
        let (_d, p) = tmp_project("Granular");
        let mut item = item_with_tech("/m/g.mov");
        assert!(insert_media(&p.file_path, &item).unwrap());
        assert!(!insert_media(&p.file_path, &item).unwrap(), "duplicate id must be skipped");
        item.metadata.duration_pts = 999;
        assert!(update_media(&p.file_path, &item).unwrap());
        let hash = item.metadata.tech.as_ref().unwrap().content_hash.clone();
        let found = find_media_by_hash(&p.file_path, &hash).unwrap().unwrap();
        assert_eq!(found.metadata.duration_pts, 999);
        assert_eq!(delete_media(&p.file_path, item.id).unwrap(), 1);
        assert!(Project::load(&p.file_path).unwrap().media_pool.is_empty());
    }

    #[test]
    fn legacy_json_is_migrated_with_backup() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("old.aether");
        let file_str = file.to_str().unwrap().to_string();
        let mut legacy = Project {
            id: Uuid::new_v4().to_string(),
            name: "Old".into(),
            project_path: "/somewhere/else".into(),
            file_path: "/somewhere/else/old.aether".into(),
            timeline: Timeline::new_with_default_tracks(Rational { num: 60, den: 1 }),
            media_pool: MediaPool::new(),
        };
        let mut item = item_with_tech("/m/legacy.mp4");
        item.metadata.tech = None;
        legacy.add_media(item);
        fs::write(&file, serde_json::to_string_pretty(&legacy).unwrap()).unwrap();

        let loaded = Project::load(&file_str).unwrap();
        assert_eq!(loaded.file_path, file_str, "moved project must be relocated");
        assert_eq!(loaded.media_pool.len(), 1);
        assert_eq!(detect_format(&file_str).unwrap(), ProjectFileFormat::Sqlite);
        assert!(dir.path().join("old.aether.json.bak").exists());
        // Second load reads SQLite and yields the same data.
        assert_eq!(Project::load(&file_str).unwrap(), loaded);
    }

    #[test]
    fn garbage_and_empty_files_error_without_panic() {
        let dir = tempfile::tempdir().unwrap();
        let garbage = dir.path().join("g.aether");
        fs::write(&garbage, b"\x00\x01\x02 not a project").unwrap();
        assert!(Project::load(garbage.to_str().unwrap()).is_err());
        let empty = dir.path().join("e.aether");
        fs::write(&empty, b"").unwrap();
        assert!(Project::load(empty.to_str().unwrap()).is_err());
        // Truncated SQLite header
        let trunc = dir.path().join("t.aether");
        fs::write(&trunc, &SQLITE_MAGIC[..10]).unwrap();
        assert!(Project::load(trunc.to_str().unwrap()).is_err());
    }

    #[test]
    fn newer_schema_is_rejected() {
        let (_d, p) = tmp_project("Future");
        Connection::open(&p.file_path)
            .unwrap()
            .execute_batch("PRAGMA user_version = 99;")
            .unwrap();
        let err = Project::load(&p.file_path).unwrap_err();
        assert!(err.contains("versão mais nova"), "{}", err);
    }

    #[test]
    fn project_bundle_and_domain_timeline_compatibility() {
        let dir = tempfile::tempdir().unwrap();
        let bundle_path = dir.path().join("testando.aether");
        let bundle = crate::bundle::ProjectBundle::new(&bundle_path);
        let conn = bundle.open_connection().unwrap();

        let meta = crate::bundle::ProjectMeta {
            id: Uuid::new_v4().to_string(),
            name: "Testando Bundle".to_string(),
            created_at: "2026-10-24T12:00:00Z".to_string(),
            updated_at: "2026-10-24T12:00:00Z".to_string(),
        };
        crate::bundle::ProjectBundle::write_meta(&conn, &meta).unwrap();

        // Initialize domain timeline schema and a default timeline
        crate::db::timeline::init_timeline_schema(&conn).unwrap();
        let domain_tl = crate::db::timeline::create_timeline(&conn, "Timeline 1", 60, 1, 1920, 1080).unwrap();
        drop(conn);

        let db_file_str = bundle.db_path().to_string_lossy().to_string();

        // 1. Load via Project::load using project.db path
        let mut loaded = Project::load(&db_file_str).expect("Project::load should succeed on ProjectBundle db");
        assert_eq!(loaded.id, meta.id);
        assert_eq!(loaded.name, "Testando Bundle");
        assert_eq!(loaded.timeline.id.to_string(), domain_tl.id);
        assert_eq!(loaded.timeline.tracks.len(), 2);

        // 2. Add media item and save via Project::save
        let media = item_with_tech("/Users/test/gravacao_show.m4a");
        let media_id = loaded.add_media(media);
        loaded.save().expect("Project::save should succeed on ProjectBundle db");

        // 3. Reload via bundle directory path directly and verify media + timeline + meta preserved
        let reloaded = Project::load(bundle.root_path().to_str().unwrap())
            .expect("Project::load should also accept .aether bundle directory path");
        assert_eq!(reloaded.id, meta.id);
        assert_eq!(reloaded.name, "Testando Bundle");
        assert_eq!(reloaded.media_pool.items.len(), 1);
        assert_eq!(reloaded.media_pool.items[0].id, media_id);
        assert_eq!(reloaded.timeline.tracks.len(), 2);

        let conn_check = bundle.open_connection().unwrap();
        let meta_after = crate::bundle::ProjectBundle::read_meta(&conn_check).unwrap().unwrap();
        assert_eq!(meta_after.id, meta.id);
        assert_eq!(meta_after.name, "Testando Bundle");
        assert_eq!(meta_after.created_at, meta.created_at);
    }
}

