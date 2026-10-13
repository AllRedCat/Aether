use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex, RwLock};
use std::thread;
use std::time::Duration;

use flutter_rust_bridge::frb;
pub use aether_core::timeline::{Clip, Rational, Timeline, Track, TrackKind};
pub use aether_core::project::Project;
pub use aether_core::media::{MediaItem, MediaMetadata, MediaPool, MediaTechInfo, MediaType};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
pub use crate::frb_generated::StreamSink;

// ---------------------------------------------------------
// FFI DATA TRANSFER OBJECTS (Media Ingestion, Waveform & Progress)
// ---------------------------------------------------------

/// Single audio waveform peak bin (`min`, `max`, `rms` in normalized `[-1.0, 1.0]`)
/// exposed across the FFI boundary for zero-decode timeline waveform rendering.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AudioPeak {
    pub min: f32,
    pub max: f32,
    pub rms: f32,
}

/// Complete descriptive, technical, and sidecar cache metadata returned when
/// importing a media asset through the FFI bridge.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MediaAssetMetadata {
    pub id: String,
    pub file_path: String,
    pub file_name: String,
    pub media_type: MediaType,
    pub metadata: MediaMetadata,
    pub cache_key: String,
    pub frame_index_path: Option<String>,
    pub peak_cache_path: Option<String>,
    pub thumb_atlas_path: Option<String>,
}

/// Stage of the asynchronous media ingestion and sidecar cache pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImportStage {
    Inspecting,
    IndexingFrames,
    GeneratingWaveform,
    GeneratingThumbnails,
    Completed,
    Failed,
}

/// Progress event emitted asynchronously via `StreamSink<ImportProgress>`
/// while a media asset is being inspected, indexed (`.fidx`), and cached
/// (`.peak` waveform pyramid and `.atlas` thumbnails).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportProgress {
    pub asset_id: String,
    pub file_name: String,
    pub stage: ImportStage,
    /// Normalized progress in `[0.0, 1.0]`.
    pub progress: f32,
    pub message: String,
    pub is_done: bool,
    pub error: Option<String>,
}

#[derive(Clone)]
struct RegisteredAsset {
    media_item: MediaItem,
    #[allow(dead_code)]
    cache_key: String,
    cache_dir: PathBuf,
    #[allow(dead_code)]
    frame_index_path: Option<PathBuf>,
    peak_cache_path: PathBuf,
    thumb_atlas_path: PathBuf,
    waveform_lock: Arc<Mutex<()>>,
}

static ASSET_REGISTRY: LazyLock<RwLock<HashMap<String, RegisteredAsset>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

static ACTIVE_PROJECT_FILE: LazyLock<RwLock<Option<String>>> =
    LazyLock::new(|| RwLock::new(None));

static ACTIVE_PROJECT_DIR: LazyLock<RwLock<Option<PathBuf>>> =
    LazyLock::new(|| RwLock::new(None));

static IMPORT_PROGRESS_SINKS: LazyLock<Mutex<Vec<StreamSink<ImportProgress>>>> =
    LazyLock::new(|| Mutex::new(Vec::new()));

static RECENT_IMPORT_PROGRESS: LazyLock<Mutex<HashMap<String, ImportProgress>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn default_standalone_cache_mgr() -> aether_media::CacheManager {
    let dir = std::env::temp_dir().join("aether_media_cache");
    aether_media::CacheManager::new(dir)
}

fn resolve_cache_manager(project_dir: Option<&Path>) -> aether_media::CacheManager {
    if let Some(dir) = project_dir {
        return aether_media::CacheManager::for_project(dir);
    }
    if let Ok(guard) = ACTIVE_PROJECT_DIR.read() {
        if let Some(ref active_dir) = *guard {
            return aether_media::CacheManager::for_project(active_dir);
        }
    }
    default_standalone_cache_mgr()
}

fn emit_import_progress(event: ImportProgress) {
    if let Ok(mut recent) = RECENT_IMPORT_PROGRESS.lock() {
        recent.insert(event.asset_id.clone(), event.clone());
    }
    if let Ok(mut sinks) = IMPORT_PROGRESS_SINKS.lock() {
        sinks.retain(|sink| sink.add(event.clone()).is_ok());
    }
}

fn register_media_item_in_cache(
    item: &MediaItem,
    cache_mgr: &aether_media::CacheManager,
    explicit_cache_key: Option<String>,
    frame_index_path: Option<PathBuf>,
) -> RegisteredAsset {
    let asset_id = item.id.to_string();
    let cache_key = explicit_cache_key
        .or_else(|| {
            item.metadata
                .tech
                .as_ref()
                .map(|t| t.content_hash.clone())
                .filter(|h| !h.is_empty())
        })
        .or_else(|| {
            aether_media::compute_source_fingerprint(Path::new(&item.file_path))
                .ok()
                .map(|fp| fp.cache_key)
        })
        .unwrap_or_else(|| asset_id.clone());

    let peak_cache_path = cache_mgr.audio_peaks_path(&cache_key);
    let thumb_atlas_path = cache_mgr.thumbnails_path(&cache_key);
    let fidx_path = frame_index_path.or_else(|| {
        let candidate = cache_mgr.frame_index_path(&cache_key);
        if candidate.exists() {
            Some(candidate)
        } else {
            None
        }
    });

    if let Ok(mut reg) = ASSET_REGISTRY.write() {
        let existing_lock = reg
            .get(&asset_id)
            .map(|r| Arc::clone(&r.waveform_lock))
            .unwrap_or_else(|| Arc::new(Mutex::new(())));

        let entry = RegisteredAsset {
            media_item: item.clone(),
            cache_key,
            cache_dir: cache_mgr.root().to_path_buf(),
            frame_index_path: fidx_path,
            peak_cache_path,
            thumb_atlas_path,
            waveform_lock: existing_lock,
        };
        reg.insert(asset_id, entry.clone());
        return entry;
    }

    RegisteredAsset {
        media_item: item.clone(),
        cache_key,
        cache_dir: cache_mgr.root().to_path_buf(),
        frame_index_path: fidx_path,
        peak_cache_path,
        thumb_atlas_path,
        waveform_lock: Arc::new(Mutex::new(())),
    }
}

fn register_project_assets(project: &Project) {
    let proj_dir = PathBuf::from(&project.project_path);
    if !project.project_path.is_empty() {
        if let Ok(mut guard) = ACTIVE_PROJECT_DIR.write() {
            *guard = Some(proj_dir.clone());
        }
    }
    if !project.file_path.is_empty() {
        if let Ok(mut guard) = ACTIVE_PROJECT_FILE.write() {
            *guard = Some(project.file_path.clone());
        }
    }
    let cache_mgr = aether_media::CacheManager::for_project(&proj_dir);
    for item in &project.media_pool.items {
        register_media_item_in_cache(item, &cache_mgr, None, None);
    }
}

fn spawn_background_asset_indexing(registered: RegisteredAsset, timebase: Rational) {
    let asset_id = registered.media_item.id.to_string();
    let file_name = registered.media_item.file_name.clone();
    let file_path = registered.media_item.file_path.clone();
    let media_type = registered.media_item.media_type.clone();
    let has_audio = registered.media_item.metadata.audio_channels.unwrap_or(0) > 0
        || matches!(media_type, MediaType::Audio);
    let is_video = matches!(media_type, MediaType::Video);

    let _ = thread::Builder::new()
        .name(format!("aether-index-{}", &asset_id[..asset_id.len().min(8)]))
        .spawn(move || {
            // 1. Audio waveform pyramid (.peak)
            if has_audio {
                emit_import_progress(ImportProgress {
                    asset_id: asset_id.clone(),
                    file_name: file_name.clone(),
                    stage: ImportStage::GeneratingWaveform,
                    progress: 0.45,
                    message: "Gerando pirâmide de waveform (.peak)...".to_string(),
                    is_done: false,
                    error: None,
                });

                let _guard = registered.waveform_lock.lock().ok();
                if !registered.peak_cache_path.exists() {
                    if let Ok(handle) = aether_media::audio_peaks::spawn_waveform_cache_worker(
                        file_path.clone(),
                        registered.peak_cache_path.to_string_lossy().to_string(),
                        aether_media::audio_peaks::DEFAULT_BASE_SAMPLES_PER_PEAK,
                        aether_media::audio_peaks::DEFAULT_LOD_LEVELS,
                    ) {
                        while !handle.is_finished() {
                            let pm = handle.progress_per_mille().min(1000) as f32 / 1000.0;
                            let scaled = 0.45 + pm * 0.35;
                            emit_import_progress(ImportProgress {
                                asset_id: asset_id.clone(),
                                file_name: file_name.clone(),
                                stage: ImportStage::GeneratingWaveform,
                                progress: scaled,
                                message: format!(
                                    "Calculando waveform ({:.0}%)...",
                                    pm * 100.0
                                ),
                                is_done: false,
                                error: None,
                            });
                            thread::sleep(Duration::from_millis(15));
                        }
                        let _ = handle.join();
                    }
                }
            }

            // 2. Video timeline thumbnails (.atlas)
            if is_video {
                emit_import_progress(ImportProgress {
                    asset_id: asset_id.clone(),
                    file_name: file_name.clone(),
                    stage: ImportStage::GeneratingThumbnails,
                    progress: 0.82,
                    message: "Extraindo miniaturas da timeline (.atlas)...".to_string(),
                    is_done: false,
                    error: None,
                });

                if !registered.thumb_atlas_path.exists() {
                    if let Ok(handle) = aether_media::thumbnails::spawn_thumbnail_filmstrip_worker(
                        file_path.clone(),
                        Some(registered.thumb_atlas_path.clone()),
                        160,
                        90,
                        aether_media::thumbnails::ThumbnailCodec::Jpeg { quality: 80 },
                        aether_media::thumbnails::ThumbnailStrategy::Keyframes {
                            min_interval_pts: 30,
                        },
                        timebase,
                    ) {
                        while !handle.is_finished() {
                            let pm = handle.progress_per_mille().min(1000) as f32 / 1000.0;
                            let scaled = 0.82 + pm * 0.16;
                            emit_import_progress(ImportProgress {
                                asset_id: asset_id.clone(),
                                file_name: file_name.clone(),
                                stage: ImportStage::GeneratingThumbnails,
                                progress: scaled,
                                message: format!(
                                    "Gerando miniaturas ({:.0}%)...",
                                    pm * 100.0
                                ),
                                is_done: false,
                                error: None,
                            });
                            thread::sleep(Duration::from_millis(15));
                        }
                        let _ = handle.join();
                    }
                }
            }

            emit_import_progress(ImportProgress {
                asset_id,
                file_name,
                stage: ImportStage::Completed,
                progress: 1.0,
                message: "Indexação e cache concluídos".to_string(),
                is_done: true,
                error: None,
            });
        });
}

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
    pub tech: Option<MediaTechInfo>,
}

#[allow(dead_code)]
#[frb(mirror(MediaTechInfo))]
pub struct _MediaTechInfo {
    pub content_hash: String,
    pub container: Option<String>,
    pub coded_width: Option<u32>,
    pub coded_height: Option<u32>,
    pub display_width: Option<u32>,
    pub display_height: Option<u32>,
    pub bit_depth: Option<u8>,
    pub track_timebase: Option<Rational>,
    pub channel_layout: Option<String>,
    pub video_packet_count: Option<u32>,
    pub keyframe_count: Option<u32>,
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
    let project = Project::create_new(&name, &base_dir)?;
    register_project_assets(&project);
    Ok(project)
}

pub fn load_project(file_path: String) -> Result<Project, String> {
    let project = Project::load(&file_path)?;
    register_project_assets(&project);
    Ok(project)
}

pub fn save_project(project: Project) -> Result<(), String> {
    register_project_assets(&project);
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
// MEDIA POOL & CACHE FFI ENDPOINTS
// ---------------------------------------------------------

fn ingest_and_schedule_caches(
    project_path: Option<String>,
    file_path: String,
) -> Result<(MediaItem, MediaAssetMetadata), String> {
    if file_path.trim().is_empty() {
        return Err("File path cannot be empty".to_string());
    }

    let path = Path::new(&file_path);
    if !path.exists() {
        return Err(format!("File does not exist: {}", file_path));
    }
    if !path.is_file() {
        return Err(format!("Path is not a regular file: {}", file_path));
    }

    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    // Determine effective project path (explicit argument or active loaded project)
    let effective_project_path = project_path
        .filter(|p| !p.trim().is_empty())
        .or_else(|| {
            ACTIVE_PROJECT_FILE
                .read()
                .ok()
                .and_then(|g| g.clone())
                .filter(|p| Path::new(p).exists())
        });

    if let Some(proj_path) = effective_project_path {
        let mut project = Project::load(&proj_path)
            .map_err(|e| format!("Failed to load project at '{}': {}", proj_path, e))?;
        register_project_assets(&project);

        let cache_mgr = aether_media::CacheManager::for_project(&project.project_path);
        let timebase = project.timeline.timebase;

        let ingested = aether_media::ingest_media_with_cache(path, timebase, Some(&cache_mgr))
            .map_err(|e| format!("Failed to inspect media file '{}': {}", file_path, e))?;

        let asset_id = ingested.media_item.id.to_string();
        emit_import_progress(ImportProgress {
            asset_id: asset_id.clone(),
            file_name: file_name.clone(),
            stage: ImportStage::IndexingFrames,
            progress: 0.40,
            message: "Índice temporal de quadros (.fidx) gerado".to_string(),
            is_done: false,
            error: None,
        });

        let registered = register_media_item_in_cache(
            &ingested.media_item,
            &cache_mgr,
            Some(ingested.fingerprint.cache_key.clone()),
            ingested.frame_index_path.clone(),
        );

        project.media_pool.items.push(ingested.media_item.clone());
        project
            .save()
            .map_err(|e| format!("Failed to save project after importing media: {}", e))?;

        let asset_meta = MediaAssetMetadata {
            id: asset_id,
            file_path: ingested.media_item.file_path.clone(),
            file_name: ingested.media_item.file_name.clone(),
            media_type: ingested.media_item.media_type.clone(),
            metadata: ingested.media_item.metadata.clone(),
            cache_key: ingested.fingerprint.cache_key,
            frame_index_path: ingested
                .frame_index_path
                .map(|p| p.to_string_lossy().to_string()),
            peak_cache_path: Some(registered.peak_cache_path.to_string_lossy().to_string()),
            thumb_atlas_path: Some(registered.thumb_atlas_path.to_string_lossy().to_string()),
        };

        spawn_background_asset_indexing(registered, timebase);
        return Ok((ingested.media_item, asset_meta));
    }

    // Standalone ingestion (when no project file is active)
    let cache_mgr = resolve_cache_manager(None);
    let timebase = aether_media::DEFAULT_TIMEBASE;
    let ingested = aether_media::ingest_media_with_cache(path, timebase, Some(&cache_mgr))
        .map_err(|e| format!("Failed to inspect media file '{}': {}", file_path, e))?;

    let asset_id = ingested.media_item.id.to_string();
    emit_import_progress(ImportProgress {
        asset_id: asset_id.clone(),
        file_name,
        stage: ImportStage::IndexingFrames,
        progress: 0.40,
        message: "Índice temporal de quadros (.fidx) gerado".to_string(),
        is_done: false,
        error: None,
    });

    let registered = register_media_item_in_cache(
        &ingested.media_item,
        &cache_mgr,
        Some(ingested.fingerprint.cache_key.clone()),
        ingested.frame_index_path.clone(),
    );

    let asset_meta = MediaAssetMetadata {
        id: asset_id,
        file_path: ingested.media_item.file_path.clone(),
        file_name: ingested.media_item.file_name.clone(),
        media_type: ingested.media_item.media_type.clone(),
        metadata: ingested.media_item.metadata.clone(),
        cache_key: ingested.fingerprint.cache_key,
        frame_index_path: ingested
            .frame_index_path
            .map(|p| p.to_string_lossy().to_string()),
        peak_cache_path: Some(registered.peak_cache_path.to_string_lossy().to_string()),
        thumb_atlas_path: Some(registered.thumb_atlas_path.to_string_lossy().to_string()),
    };

    spawn_background_asset_indexing(registered, timebase);
    Ok((ingested.media_item, asset_meta))
}

/// Imports a media asset at `path`, builds its binary temporal frame index (`.fidx`),
/// triggers non-blocking background generation of `.peak` waveform and `.atlas`
/// thumbnail caches, and returns [`MediaAssetMetadata`].
pub fn import_media(path: String) -> Result<MediaAssetMetadata, String> {
    let (_item, meta) = ingest_and_schedule_caches(None, path)?;
    Ok(meta)
}

/// Alias for [`import_media`] exposing the `import_media_asset` contract.
pub fn import_media_asset(path: String) -> Result<MediaAssetMetadata, String> {
    import_media(path)
}

/// Imports a media file, extracting its type and metadata via `aether_media`.
/// If `project_path` is provided, the imported media is appended to the project's
/// MediaPool and persisted to disk.
pub fn import_media_file(
    project_path: Option<String>,
    file_path: String,
) -> Result<MediaItem, String> {
    let (item, _meta) = ingest_and_schedule_caches(project_path, file_path)?;
    Ok(item)
}

/// Reads a single Level-of-Detail (`lod`) slice of audio waveform peaks (`min`, `max`, `rms`)
/// for `asset_id` directly from the binary `.peak` / `.aether_peaks` sidecar cache on disk,
/// without reading or decoding the original audio file during timeline rendering.
///
/// If the `.peak` sidecar has not yet been written to disk, it is generated once and cached.
pub fn get_audio_waveform(asset_id: String, lod: u32) -> Result<Vec<AudioPeak>, String> {
    let trimmed = asset_id.trim();
    if trimmed.is_empty() {
        return Err("asset_id cannot be empty".to_string());
    }

    // 1. Look up in the active asset registry
    let registered_opt = ASSET_REGISTRY
        .read()
        .ok()
        .and_then(|reg| reg.get(trimmed).cloned());

    let peak_file_path = if let Some(registered) = registered_opt {
        let _guard = registered.waveform_lock.lock().ok();
        let legacy_peak = registered
            .cache_dir
            .join("peaks")
            .join(format!("{}.aether_peaks", trimmed));

        if registered.peak_cache_path.exists() {
            registered.peak_cache_path.clone()
        } else if legacy_peak.exists() {
            legacy_peak
        } else {
            // Generate on demand into the deterministic `.peak` cache path
            let out_str = registered.peak_cache_path.to_string_lossy().to_string();
            aether_media::audio_peaks::generate_waveform_cache_pyramid(
                &registered.media_item.file_path,
                &out_str,
                aether_media::audio_peaks::DEFAULT_BASE_SAMPLES_PER_PEAK,
                aether_media::audio_peaks::DEFAULT_LOD_LEVELS,
                None,
                None,
            )?;
            registered.peak_cache_path.clone()
        }
    } else {
        // 2. Fallback: check if `asset_id` is a direct path to a `.peak`/`.aether_peaks` file
        // or a media file path, or search in active project cache directory.
        let as_path = Path::new(trimmed);
        if as_path.exists() && as_path.is_file() {
            let ext = as_path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if ext == "peak" || ext == "aether_peaks" {
                as_path.to_path_buf()
            } else {
                let cache_mgr = resolve_cache_manager(None);
                let fp = aether_media::compute_source_fingerprint(as_path)
                    .map_err(|e| e.to_string())?;
                let peak_path = cache_mgr.audio_peaks_path(&fp.cache_key);
                if !peak_path.exists() {
                    aether_media::audio_peaks::generate_waveform_cache_pyramid(
                        trimmed,
                        &peak_path.to_string_lossy(),
                        aether_media::audio_peaks::DEFAULT_BASE_SAMPLES_PER_PEAK,
                        aether_media::audio_peaks::DEFAULT_LOD_LEVELS,
                        None,
                        None,
                    )?;
                }
                peak_path
            }
        } else {
            let cache_mgr = resolve_cache_manager(None);
            let direct_peak = cache_mgr.audio_peaks_path(trimmed);
            let legacy_peak = cache_mgr
                .root()
                .join("peaks")
                .join(format!("{}.aether_peaks", trimmed));
            if direct_peak.exists() {
                direct_peak
            } else if legacy_peak.exists() {
                legacy_peak
            } else {
                return Err(format!(
                    "Waveform cache not found for asset '{}'",
                    trimmed
                ));
            }
        }
    };

    let level =
        aether_media::audio_peaks::AudioPeakPyramid::load_lod_from_file(&peak_file_path, lod)?;
    let mono = level.mixdown_mono();
    Ok(mono
        .into_iter()
        .map(|s| AudioPeak {
            min: s.min,
            max: s.max,
            rms: s.rms,
        })
        .collect())
}

/// Attaches a `StreamSink<ImportProgress>` to receive asynchronous progress notifications
/// while media assets are inspected, indexed (`.fidx`), and cached (`.peak`, `.atlas`).
pub fn subscribe_import_progress(sink: StreamSink<ImportProgress>) -> Result<(), String> {
    if let Ok(recent) = RECENT_IMPORT_PROGRESS.lock() {
        for progress in recent.values() {
            let _ = sink.add(progress.clone());
        }
    }
    if let Ok(mut sinks) = IMPORT_PROGRESS_SINKS.lock() {
        sinks.push(sink);
    }
    Ok(())
}

/// Generates an audio waveform cache file (Peak Data) for a specific media item.
/// The resulting `.aether_peaks` binary file is saved inside the project's `.aether_cache/peaks` directory.
/// Returns the absolute path to the generated cache file.
pub fn generate_audio_peaks(
    project_path: String,
    media_id: String,
    samples_per_peak: u32,
) -> Result<String, String> {
    let project = Project::load(&project_path)
        .map_err(|e| format!("Failed to load project: {}", e))?;
    register_project_assets(&project);
    
    let media_item = project
        .media_pool
        .items
        .iter()
        .find(|item| item.id.to_string() == media_id)
        .ok_or_else(|| format!("Media item {} not found in project", media_id))?;
        
    let cache_dir = project.get_peaks_cache_dir()?;
    let peak_file_path = cache_dir.join(format!("{}.aether_peaks", media_id));
    let peak_file_str = peak_file_path.to_string_lossy().to_string();
    
    aether_media::audio_peaks::generate_waveform_cache(
        &media_item.file_path,
        &peak_file_str,
        samples_per_peak,
    )?;
    
    Ok(peak_file_str)
}

/// Standalone inspection of a media file without touching project persistence.
pub fn inspect_media_file(file_path: String) -> Result<MediaItem, String> {
    import_media_file(None, file_path)
}

/// Returns the list of imported media items in the specified project.
pub fn get_media_items(project_path: String) -> Result<Vec<MediaItem>, String> {
    let project = Project::load(&project_path)
        .map_err(|e| format!("Failed to load project at '{}': {}", project_path, e))?;
    register_project_assets(&project);
    Ok(project.media_pool.items)
}

/// Returns the entire `MediaPool` structure for the specified project.
pub fn get_media_pool(project_path: String) -> Result<MediaPool, String> {
    let project = Project::load(&project_path)
        .map_err(|e| format!("Failed to load project at '{}': {}", project_path, e))?;
    register_project_assets(&project);
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;

    fn write_test_wav(path: &Path, sample_rate: u32, num_samples: usize) {
        let num_channels: u16 = 1;
        let bits_per_sample: u16 = 16;
        let byte_rate = sample_rate * (num_channels as u32) * (bits_per_sample as u32 / 8);
        let block_align = num_channels * (bits_per_sample / 8);
        let data_size = (num_samples * 2) as u32;
        let riff_size = 36 + data_size;

        let mut f = File::create(path).expect("failed to create test wav");
        f.write_all(b"RIFF").unwrap();
        f.write_all(&riff_size.to_le_bytes()).unwrap();
        f.write_all(b"WAVE").unwrap();
        f.write_all(b"fmt ").unwrap();
        f.write_all(&16u32.to_le_bytes()).unwrap();
        f.write_all(&1u16.to_le_bytes()).unwrap(); // PCM
        f.write_all(&num_channels.to_le_bytes()).unwrap();
        f.write_all(&sample_rate.to_le_bytes()).unwrap();
        f.write_all(&byte_rate.to_le_bytes()).unwrap();
        f.write_all(&block_align.to_le_bytes()).unwrap();
        f.write_all(&bits_per_sample.to_le_bytes()).unwrap();
        f.write_all(b"data").unwrap();
        f.write_all(&data_size.to_le_bytes()).unwrap();

        for i in 0..num_samples {
            let phase = (i as f64) * 2.0 * std::f64::consts::PI * 440.0 / (sample_rate as f64);
            let sample = (phase.sin() * 24000.0) as i16;
            f.write_all(&sample.to_le_bytes()).unwrap();
        }
    }

    #[test]
    fn import_media_and_get_audio_waveform_across_lods() {
        let tmp_dir = std::env::temp_dir().join(format!("aether_bridge_test_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&tmp_dir).unwrap();
        let wav_path = tmp_dir.join("tone.wav");
        // 4096 samples -> 16 bins at LOD 0 (256 spp), 8 bins at LOD 1 (512 spp), 4 bins at LOD 2 (1024 spp)
        write_test_wav(&wav_path, 48_000, 4096);

        let meta = import_media(wav_path.to_string_lossy().to_string())
            .expect("import_media should succeed");
        assert!(!meta.id.is_empty());
        assert_eq!(meta.file_name, "tone.wav");
        assert!(!meta.cache_key.is_empty());
        assert!(meta.peak_cache_path.is_some());

        let lod0 = get_audio_waveform(meta.id.clone(), 0).expect("get_audio_waveform LOD 0");
        let lod1 = get_audio_waveform(meta.id.clone(), 1).expect("get_audio_waveform LOD 1");
        let lod2 = get_audio_waveform(meta.id.clone(), 2).expect("get_audio_waveform LOD 2");

        assert_eq!(lod0.len(), 16);
        assert_eq!(lod1.len(), 8);
        assert_eq!(lod2.len(), 4);

        for peak in &lod0 {
            assert!(peak.min < -0.5 && peak.min >= -1.0);
            assert!(peak.max > 0.5 && peak.max <= 1.0);
            assert!(peak.rms > 0.3 && peak.rms <= 1.0);
        }

        // Verify progress was recorded in RECENT_IMPORT_PROGRESS
        let recorded = RECENT_IMPORT_PROGRESS
            .lock()
            .unwrap()
            .get(&meta.id)
            .cloned();
        assert!(recorded.is_some());

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }
}

