//! Document Package / Bundle storage layer for Aether projects.
//!
//! Encapsulates the project directory bundle (`<name>.aether`), containing:
//! - `<name>.aether/project.db` (SQLite database in WAL mode)
//! - `<name>.aether/cache/index/` (per-asset binary frame indexes)
//! - `<name>.aether/cache/peaks/` (audio waveform peak pyramids)
//! - `<name>.aether/cache/thumbs/` (thumbnail filmstrip sprite atlases)

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

/// Busy timeout for SQLite connections to avoid contention.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// Minimal project bundle metadata stored in `project.db`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectMetadata {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub updated_at: String,
}

pub type ProjectMeta = ProjectMetadata;

/// Represents an Aether project bundle directory (`<root_path>.aether`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectBundle {
    pub root_path: PathBuf,
}

impl ProjectBundle {
    /// Creates a new `ProjectBundle` from the given path.
    ///
    /// - Normalizes trailing slashes and redundant path components.
    /// - If `path` is inside an existing `<name>.aether` ancestor (e.g. `project.db`,
    ///   `project.db-wal`, `cache/`), resolves directly to that `.aether` root.
    /// - If `path` points to `project.db*`, climbs one level (`parent()`) to obtain the bundle root.
    /// - Ensures that the root directory always has the `.aether` extension.
    pub fn new(path: impl AsRef<Path>) -> Self {
        let normalized: PathBuf = path.as_ref().components().collect();

        // 1. If normalized path or any ancestor already has the `.aether` extension, use that ancestor
        if let Some(aether_ancestor) = normalized.ancestors().find(|ancestor| {
            ancestor
                .extension()
                .map_or(false, |ext| ext.eq_ignore_ascii_case("aether"))
        }) {
            return Self {
                root_path: aether_ancestor.to_path_buf(),
            };
        }

        // 2. If path points to `project.db` (or WAL/SHM sidecars), climb one level to get bundle root
        let is_project_db_file = normalized
            .file_name()
            .and_then(|n| n.to_str())
            .map_or(false, |name| {
                name == "project.db" || name == "project.db-wal" || name == "project.db-shm"
            });

        let mut root = if is_project_db_file {
            match normalized.parent() {
                Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
                _ => PathBuf::from("project"),
            }
        } else {
            normalized
        };

        // 3. Ensure root directory always has `.aether` extension
        if !root
            .extension()
            .map_or(false, |ext| ext.eq_ignore_ascii_case("aether"))
        {
            root = root.with_extension("aether");
        }

        Self { root_path: root }
    }

    /// Returns a reference to the bundle's root directory path (`<name>.aether`).
    pub fn root_path(&self) -> &Path {
        &self.root_path
    }

    /// Path to the SQLite database: `<root_path>/project.db`.
    pub fn db_path(&self) -> PathBuf {
        self.root_path.join("project.db")
    }

    /// Root path for project cache: `<root_path>/cache`.
    pub fn cache_dir(&self) -> PathBuf {
        self.root_path.join("cache")
    }

    /// Directory for frame indexes: `<root_path>/cache/index`.
    pub fn index_dir(&self) -> PathBuf {
        self.cache_dir().join("index")
    }

    /// Directory for audio waveform peaks: `<root_path>/cache/peaks`.
    pub fn peaks_dir(&self) -> PathBuf {
        self.cache_dir().join("peaks")
    }

    /// Directory for thumbnail atlases: `<root_path>/cache/thumbs`.
    pub fn thumbs_dir(&self) -> PathBuf {
        self.cache_dir().join("thumbs")
    }

    /// Ensures that the bundle root and all required cache directories exist on disk:
    /// - `<root_path>/`
    /// - `<root_path>/cache/index/`
    /// - `<root_path>/cache/peaks/`
    /// - `<root_path>/cache/thumbs/`
    pub fn ensure_directories(&self) -> io::Result<()> {
        fs::create_dir_all(&self.root_path)?;
        fs::create_dir_all(self.index_dir())?;
        fs::create_dir_all(self.peaks_dir())?;
        fs::create_dir_all(self.thumbs_dir())?;
        Ok(())
    }

    /// Opens a connection to the SQLite database at `self.db_path()`, creating
    /// bundle directories if needed and configuring:
    /// - `PRAGMA journal_mode = WAL;`
    /// - `PRAGMA foreign_keys = ON;`
    ///
    /// Also initializes the minimal `project_meta` table if it does not exist.
    pub fn open_connection(&self) -> rusqlite::Result<Connection> {
        self.ensure_directories().map_err(|e| {
            rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CANTOPEN),
                Some(e.to_string()),
            )
        })?;

        let conn = Connection::open(self.db_path())?;
        Self::configure_connection(&conn)?;
        Self::ensure_meta_table(&conn)?;
        Ok(conn)
    }

    /// Alias for [`open_connection`].
    pub fn open_db(&self) -> rusqlite::Result<Connection> {
        self.open_connection()
    }

    /// Configures SQLite connection pragmas:
    /// - `PRAGMA journal_mode = WAL;`
    /// - `PRAGMA foreign_keys = ON;`
    pub fn configure_connection(conn: &Connection) -> rusqlite::Result<()> {
        conn.busy_timeout(BUSY_TIMEOUT)?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;",
        )?;
        Ok(())
    }

    /// Creates the minimal `project_meta` table if it does not exist.
    ///
    /// Table schema:
    /// - `id`: TEXT PRIMARY KEY
    /// - `name`: TEXT NOT NULL
    /// - `created_at`: TEXT NOT NULL
    /// - `updated_at`: TEXT NOT NULL
    pub fn ensure_meta_table(conn: &Connection) -> rusqlite::Result<()> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS project_meta (
                id         TEXT PRIMARY KEY,
                name       TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );",
        )
    }

    /// Inserts or replaces project metadata into `project_meta`.
    pub fn write_meta(conn: &Connection, meta: &ProjectMeta) -> rusqlite::Result<()> {
        Self::ensure_meta_table(conn)?;
        conn.execute(
            "INSERT OR REPLACE INTO project_meta (id, name, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4);",
            params![meta.id, meta.name, meta.created_at, meta.updated_at],
        )?;
        Ok(())
    }

    /// Reads project metadata from `project_meta`.
    pub fn read_meta(conn: &Connection) -> rusqlite::Result<Option<ProjectMeta>> {
        Self::ensure_meta_table(conn)?;
        let mut stmt = conn.prepare(
            "SELECT id, name, created_at, updated_at FROM project_meta LIMIT 1",
        )?;
        let mut rows = stmt.query([])?;
        if let Some(row) = rows.next()? {
            Ok(Some(ProjectMeta {
                id: row.get(0)?,
                name: row.get(1)?,
                created_at: row.get(2)?,
                updated_at: row.get(3)?,
            }))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bundle_path_resolvers() {
        let bundle = ProjectBundle::new("/projects/Vacation.aether");
        assert_eq!(bundle.root_path(), Path::new("/projects/Vacation.aether"));
        assert_eq!(
            bundle.db_path(),
            Path::new("/projects/Vacation.aether/project.db")
        );
        assert_eq!(
            bundle.cache_dir(),
            Path::new("/projects/Vacation.aether/cache")
        );
        assert_eq!(
            bundle.index_dir(),
            Path::new("/projects/Vacation.aether/cache/index")
        );
        assert_eq!(
            bundle.peaks_dir(),
            Path::new("/projects/Vacation.aether/cache/peaks")
        );
        assert_eq!(
            bundle.thumbs_dir(),
            Path::new("/projects/Vacation.aether/cache/thumbs")
        );
    }

    #[test]
    fn test_bundle_new_from_various_inputs() {
        // 1. Path without extension
        let b1 = ProjectBundle::new("/home/user/MyProject");
        assert_eq!(b1.root_path(), Path::new("/home/user/MyProject.aether"));

        // 2. Path already ending in .aether
        let b2 = ProjectBundle::new("/home/user/MyProject.aether");
        assert_eq!(b2.root_path(), Path::new("/home/user/MyProject.aether"));

        // 3. Path pointing to project.db inside a .aether bundle
        let b3 = ProjectBundle::new("/home/user/MyProject.aether/project.db");
        assert_eq!(b3.root_path(), Path::new("/home/user/MyProject.aether"));

        // 4. Path pointing to project.db inside a non-.aether directory
        let b4 = ProjectBundle::new("/home/user/MyProject/project.db");
        assert_eq!(b4.root_path(), Path::new("/home/user/MyProject.aether"));

        // 5. Relative project.db
        let b5 = ProjectBundle::new("project.db");
        assert_eq!(b5.root_path(), Path::new("project.aether"));

        // 6. Case insensitivity check for .AETHER
        let b6 = ProjectBundle::new("/home/user/MyProject.AETHER");
        assert_eq!(b6.root_path(), Path::new("/home/user/MyProject.AETHER"));

        // 7. Path with other extension
        let b7 = ProjectBundle::new("/home/user/MyProject.bundle");
        assert_eq!(b7.root_path(), Path::new("/home/user/MyProject.aether"));

        // 8. Path with trailing slash
        let b8 = ProjectBundle::new("/home/user/MyProject.aether/");
        assert_eq!(b8.root_path(), Path::new("/home/user/MyProject.aether"));

        // 9. Path pointing to SQLite WAL/SHM sidecar or cache inside a .aether bundle
        let b9 = ProjectBundle::new("/home/user/MyProject.aether/project.db-wal");
        assert_eq!(b9.root_path(), Path::new("/home/user/MyProject.aether"));
        let b10 = ProjectBundle::new("/home/user/MyProject.aether/cache/index");
        assert_eq!(b10.root_path(), Path::new("/home/user/MyProject.aether"));
    }

    #[test]
    fn test_ensure_directories_creates_full_tree() {
        let tmp = tempfile::tempdir().expect("tempdir failed");
        let bundle_path = tmp.path().join("Nome.aether");
        let bundle = ProjectBundle::new(&bundle_path);

        assert!(!bundle.root_path().exists());
        bundle.ensure_directories().expect("ensure_directories failed");

        // Validate complete directory tree creation
        assert!(bundle.root_path().is_dir(), "root .aether must exist");
        assert!(bundle.cache_dir().is_dir(), "cache/ must exist");
        assert!(bundle.index_dir().is_dir(), "cache/index/ must exist");
        assert!(bundle.peaks_dir().is_dir(), "cache/peaks/ must exist");
        assert!(bundle.thumbs_dir().is_dir(), "cache/thumbs/ must exist");
    }

    #[test]
    fn test_open_connection_creates_db_and_meta_table_with_pragmas() {
        let tmp = tempfile::tempdir().expect("tempdir failed");
        let bundle_path = tmp.path().join("Nome.aether");
        let bundle = ProjectBundle::new(&bundle_path);

        // Opening connection should ensure directories, create project.db, configure pragmas, and create project_meta
        let conn = bundle.open_connection().expect("open_connection failed");

        // Verify file and directory tree
        assert!(bundle.root_path().is_dir());
        assert!(bundle.db_path().is_file());
        assert!(bundle.index_dir().is_dir());
        assert!(bundle.peaks_dir().is_dir());
        assert!(bundle.thumbs_dir().is_dir());

        // Verify PRAGMA journal_mode=WAL
        let journal_mode: String = conn
            .query_row("PRAGMA journal_mode;", [], |r| r.get(0))
            .expect("query journal_mode failed");
        assert_eq!(journal_mode.to_lowercase(), "wal");

        // Verify PRAGMA foreign_keys=ON
        let foreign_keys: i32 = conn
            .query_row("PRAGMA foreign_keys;", [], |r| r.get(0))
            .expect("query foreign_keys failed");
        assert_eq!(foreign_keys, 1);

        // Verify project_meta table columns
        let mut stmt = conn
            .prepare("PRAGMA table_info(project_meta);")
            .expect("table_info failed");
        let columns: Vec<(String, String, bool)> = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(1)?, // column name
                    row.get::<_, String>(2)?, // type
                    row.get::<_, bool>(5)?,   // pk
                ))
            })
            .expect("query_map failed")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect columns failed");

        assert_eq!(columns.len(), 4);
        assert_eq!(columns[0], ("id".to_string(), "TEXT".to_string(), true));
        assert_eq!(columns[1], ("name".to_string(), "TEXT".to_string(), false));
        assert_eq!(columns[2], ("created_at".to_string(), "TEXT".to_string(), false));
        assert_eq!(columns[3], ("updated_at".to_string(), "TEXT".to_string(), false));

        // Test writing and reading project_meta
        let meta = ProjectMeta {
            id: "proj-12345".to_string(),
            name: "Meu Filme".to_string(),
            created_at: "2026-10-15T11:00:00Z".to_string(),
            updated_at: "2026-10-15T11:30:00Z".to_string(),
        };

        ProjectBundle::write_meta(&conn, &meta).expect("write_meta failed");
        let loaded = ProjectBundle::read_meta(&conn)
            .expect("read_meta failed")
            .expect("meta should exist");
        assert_eq!(loaded, meta);
    }

    #[test]
    fn test_guardrail_no_domain_tables_created() {
        let tmp = tempfile::tempdir().expect("tempdir failed");
        let bundle = ProjectBundle::new(tmp.path().join("Isolated.aether"));
        let conn = bundle.open_connection().expect("open_connection failed");

        // Verify that domain tables (timelines, tracks, clips) are NOT created
        let forbidden_tables = ["timelines", "tracks", "clips"];
        for table in &forbidden_tables {
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1;",
                    [table],
                    |r| r.get(0),
                )
                .expect("query sqlite_master failed");
            assert_eq!(
                count, 0,
                "Domain table '{}' must NOT be created in bundle initialization",
                table
            );
        }

        // Only project_meta should exist as a user table
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%';")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(tables, vec!["project_meta".to_string()]);
    }
}
