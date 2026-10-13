//! Media ingestion and binary temporal index builder.
//!
//! Coordinates deep container inspection, deterministic source fingerprinting
//! (`partial_header + file_size + mtime`), and creation/lookup of the contiguous
//! binary [`FrameIndex`] (`.fidx`) inside the project cache.

use std::path::{Path, PathBuf};

use aether_core::media::MediaItem;
use aether_core::timeline::Rational;

use crate::cache::{compute_source_fingerprint, CacheManager, SourceFingerprint};
use crate::frame_index::{FrameIndex, FrameIndexEntry};
use crate::isobmff::parse_isobmff_file;
use crate::{inspect_media_file_with_timebase, MediaError, DEFAULT_TIMEBASE};

/// Result of ingesting a media file and building (or reusing) its binary frame index.
#[derive(Debug, Clone)]
pub struct IngestedMedia {
    /// Populated `MediaItem` ready to be stored in the project `MediaPool`.
    pub media_item: MediaItem,
    /// Deterministic fingerprint (`partial_hash + file_size + mtime`).
    pub fingerprint: SourceFingerprint,
    /// Path to the persisted `.fidx` binary frame index on disk (for video assets).
    pub frame_index_path: Option<PathBuf>,
    /// Loaded [`FrameIndex`] (for video assets with an indexable packet table).
    pub frame_index: Option<FrameIndex>,
}

/// Inspects a media file, computes its deterministic [`SourceFingerprint`], and
/// (for video files) extracts and persists the contiguous binary [`FrameIndex`]
/// inside `cache_manager`.
///
/// If a valid `.fidx` file matching the source fingerprint already exists on
/// disk, it is loaded directly without re-parsing the container.
pub fn ingest_media_with_cache(
    media_path: &Path,
    timebase: Rational,
    cache_manager: Option<&CacheManager>,
) -> Result<IngestedMedia, MediaError> {
    let path_str = media_path
        .to_str()
        .ok_or_else(|| MediaError::IoError("Non-UTF8 media path".to_string()))?;

    let inspection = inspect_media_file_with_timebase(path_str, timebase)?;
    let fingerprint = compute_source_fingerprint(media_path)?;
    let media_item = inspection.to_media_item(path_str);

    let mut frame_index_path: Option<PathBuf> = None;
    let mut frame_index: Option<FrameIndex> = None;

    if let Ok(iso) = parse_isobmff_file(media_path) {
        if let Some(video) = iso.video {
            if let Some(mgr) = cache_manager {
                let target_path = mgr.frame_index_path(&fingerprint.cache_key);
                // Reuse existing validated index if present on disk
                let idx = match FrameIndex::load_validated(&target_path, &fingerprint.digest) {
                    Ok(existing) if existing.timebase == timebase => existing,
                    _ => {
                        let built = video.build_frame_index(timebase, fingerprint.digest);
                        built.save_to_file(&target_path)?;
                        built
                    }
                };
                frame_index_path = Some(target_path);
                frame_index = Some(idx);
            } else {
                frame_index = Some(video.build_frame_index(timebase, fingerprint.digest));
            }
        }
    }

    Ok(IngestedMedia {
        media_item,
        fingerprint,
        frame_index_path,
        frame_index,
    })
}

/// Extracts (or loads from cache) the binary [`FrameIndex`] for a video file
/// using the default 60 FPS project timebase.
pub fn build_or_load_frame_index(
    video_path: &Path,
    cache_manager: Option<&CacheManager>,
) -> Result<FrameIndex, MediaError> {
    build_or_load_frame_index_with_timebase(video_path, DEFAULT_TIMEBASE, cache_manager)
}

/// Extracts (or loads from cache) the binary [`FrameIndex`] for a video file
/// with an explicit `timebase`.
pub fn build_or_load_frame_index_with_timebase(
    video_path: &Path,
    timebase: Rational,
    cache_manager: Option<&CacheManager>,
) -> Result<FrameIndex, MediaError> {
    let fingerprint = compute_source_fingerprint(video_path)?;

    if let Some(mgr) = cache_manager {
        let target_path = mgr.frame_index_path(&fingerprint.cache_key);
        if let Ok(existing) = FrameIndex::load_validated(&target_path, &fingerprint.digest) {
            if existing.timebase == timebase {
                return Ok(existing);
            }
        }
        let iso = parse_isobmff_file(video_path)?;
        let video = iso.video.ok_or_else(|| {
            MediaError::CorruptFile("Container has no indexable video track".to_string())
        })?;
        let built = video.build_frame_index(timebase, fingerprint.digest);
        built.save_to_file(&target_path)?;
        Ok(built)
    } else {
        let iso = parse_isobmff_file(video_path)?;
        let video = iso.video.ok_or_else(|| {
            MediaError::CorruptFile("Container has no indexable video track".to_string())
        })?;
        Ok(video.build_frame_index(timebase, fingerprint.digest))
    }
}

/// Convenience helper: locates the nearest preceding keyframe (`pts <= target_pts`,
/// clamped to the first keyframe if `target_pts` precedes stream start) in
/// $O(\log N)$ time over the cached binary frame index.
pub fn seek_nearest_keyframe(
    video_path: &Path,
    target_pts: i64,
    timebase: Rational,
    cache_manager: Option<&CacheManager>,
) -> Result<Option<FrameIndexEntry>, MediaError> {
    let index = build_or_load_frame_index_with_timebase(video_path, timebase, cache_manager)?;
    Ok(index.seek_keyframe(target_pts).copied())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn ingest_video_persists_binary_fidx_cache_and_supports_seek() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("sample_64x64_10frames.mp4");
        if !fixture.exists() {
            return;
        }

        let dir = tempdir().unwrap();
        let cache_mgr = CacheManager::for_project(dir.path());
        let tb = Rational { num: 60, den: 1 };

        let ingested =
            ingest_media_with_cache(&fixture, tb, Some(&cache_mgr)).expect("ingest failed");

        let index_path = ingested
            .frame_index_path
            .as_ref()
            .expect("expected persisted index path");
        assert!(index_path.exists(), "expected index file to exist on disk");

        let idx = ingested.frame_index.as_ref().expect("expected FrameIndex");
        assert_eq!(idx.len(), 10);
        assert!(idx.entries()[0].is_keyframe());
        assert!(idx.entries()[0].file_offset > 0);
        assert!(idx.entries()[0].packet_size > 0);

        // Reloading via `build_or_load_frame_index_with_timebase` must hit the disk cache
        let reloaded = build_or_load_frame_index_with_timebase(&fixture, tb, Some(&cache_mgr))
            .expect("reload failed");
        assert_eq!(reloaded.entries(), idx.entries());

        // Seeking at an arbitrary mid-clip PTS must return the preceding keyframe
        let kf = seek_nearest_keyframe(&fixture, 30, tb, Some(&cache_mgr))
            .expect("seek failed")
            .expect("expected keyframe");
        assert!(kf.is_keyframe());
        assert!(kf.pts <= 30);
    }
}
