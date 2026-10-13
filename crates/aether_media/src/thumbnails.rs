//! Timeline filmstrip thumbnail extraction, binary `.atlas` sequence/spritesheet
//! packaging with $O(\log N)$ `PTS -> offset` index, and non-blocking background worker.
//!
//! # Binary Layout (`AETHTHMB` v1)
//! ```text
//! Offset  Size    Field
//! ------  ----    -----
//! 0       8       magic = b"AETHTHMB"
//! 8       4       version = 1 (u32 LE)
//! 12      1       codec (1 = JPEG, 2 = WebP)
//! 13      3       reserved (0)
//! 16      4       thumb_width (u32 LE, e.g. 160)
//! 20      4       thumb_height (u32 LE, e.g. 90)
//! 24      2       atlas_columns (u16 LE)
//! 26      2       atlas_rows (u16 LE)
//! 28      4       timebase.num (i32 LE)
//! 32      4       timebase.den (i32 LE)
//! 36      4       entry_count (u32 LE)
//! 40      8       payload_section_offset (u64 LE)
//! 48      24*N    Contiguous `ThumbnailIndexEntry` table (sorted by `pts` ascending)
//! 48+24N  ...     Concatenated encoded thumbnail payloads (JPEG/WebP bytes)
//! ```

use std::fs::File;
use std::io::{Cursor, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use aether_core::timeline::Rational;
use image::codecs::jpeg::JpegEncoder;
use image::codecs::webp::WebPEncoder;
use image::imageops::FilterType;
use image::{ColorType, ExtendedColorType, ImageEncoder, RgbaImage};
use serde::{Deserialize, Serialize};

use crate::cache::CacheManager;
use crate::decoder::{open_video_decoder_with_timebase, VideoFrame};
use crate::importer::build_or_load_frame_index_with_timebase;
use crate::{MediaError, DEFAULT_TIMEBASE};

pub const THUMBNAIL_ATLAS_MAGIC: &[u8; 8] = b"AETHTHMB";
pub const THUMBNAIL_ATLAS_VERSION: u32 = 1;
pub const THUMBNAIL_HEADER_SIZE: usize = 48;
pub const THUMBNAIL_INDEX_ENTRY_SIZE: usize = 24;

/// Default low-resolution timeline thumbnail width (`160` px, 16:9).
pub const DEFAULT_THUMBNAIL_WIDTH: u32 = 160;
/// Default low-resolution timeline thumbnail height (`90` px, 16:9).
pub const DEFAULT_THUMBNAIL_HEIGHT: u32 = 90;

/// Compression codec used for individual filmstrip thumbnails and spritesheets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThumbnailCodec {
    /// Baseline JPEG compression with quality in `1..=100` (default `80`).
    Jpeg { quality: u8 },
    /// Lossless WebP compression via `image::codecs::webp`.
    WebP,
}

impl Default for ThumbnailCodec {
    fn default() -> Self {
        Self::Jpeg { quality: 80 }
    }
}

impl ThumbnailCodec {
    fn to_tag(self) -> (u8, u8) {
        match self {
            ThumbnailCodec::Jpeg { quality } => (1, quality.clamp(1, 100)),
            ThumbnailCodec::WebP => (2, 100),
        }
    }

    fn from_tag(code: u8, param: u8) -> Result<Self, MediaError> {
        match code {
            1 => Ok(ThumbnailCodec::Jpeg {
                quality: if param == 0 { 80 } else { param.clamp(1, 100) },
            }),
            2 => Ok(ThumbnailCodec::WebP),
            other => Err(MediaError::CorruptFile(format!(
                "Unsupported thumbnail codec tag: {}",
                other
            ))),
        }
    }
}

/// Strategy used to choose which presentation timestamps (`PTS`) are sampled
/// when building a timeline filmstrip.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum ThumbnailStrategy {
    /// Samples only I-Frames/IDR-Frames from the binary `.fidx` index, enforcing
    /// a minimum `min_interval_pts` gap between consecutive thumbnails.
    /// Falls back to fixed intervals if the asset has no keyframe table.
    Keyframes { min_interval_pts: i64 },
    /// Samples frames at fixed `interval_pts` steps (`0, step, 2*step, ...`).
    FixedIntervalPts { interval_pts: i64 },
    /// Samples frames every `interval_seconds` seconds (`0.0, s, 2*s, ...`).
    FixedIntervalSeconds { interval_seconds: f64 },
}

impl Default for ThumbnailStrategy {
    fn default() -> Self {
        Self::Keyframes {
            min_interval_pts: 30, // At most 2 thumbnails/sec at 60fps
        }
    }
}

/// Fixed 24-byte `#[repr(C)]` index entry mapping a presentation timestamp (`pts`)
/// to its byte range inside the `.atlas` binary sequence and its `(col, row)`
/// tile coordinates inside the spritesheet grid.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThumbnailIndexEntry {
    /// Presentation timestamp in the filmstrip's `timebase`.
    pub pts: i64,
    /// Absolute byte offset of the compressed image payload inside the `.atlas` file.
    pub byte_offset: u64,
    /// Compressed byte size of this thumbnail payload.
    pub byte_len: u32,
    /// Zero-based column index in the logical spritesheet grid.
    pub grid_col: u16,
    /// Zero-based row index in the logical spritesheet grid.
    pub grid_row: u16,
}

const _: [(); THUMBNAIL_INDEX_ENTRY_SIZE] = [(); std::mem::size_of::<ThumbnailIndexEntry>()];
const _: [(); 8] = [(); std::mem::align_of::<ThumbnailIndexEntry>()];

impl ThumbnailIndexEntry {
    /// Computes normalized `[0.0, 1.0]` UV texture coordinates `(u0, v0, u1, v1)`
    /// for this tile inside a spritesheet of `atlas_columns x atlas_rows`.
    #[inline]
    pub fn uv_rect(&self, atlas_columns: u16, atlas_rows: u16) -> (f32, f32, f32, f32) {
        let cols = (atlas_columns.max(1)) as f32;
        let rows = (atlas_rows.max(1)) as f32;
        let u0 = (self.grid_col as f32) / cols;
        let v0 = (self.grid_row as f32) / rows;
        let u1 = ((self.grid_col + 1) as f32) / cols;
        let v1 = ((self.grid_row + 1) as f32) / rows;
        (u0, v0, u1, v1)
    }

    #[inline]
    pub fn to_le_bytes(self) -> [u8; THUMBNAIL_INDEX_ENTRY_SIZE] {
        let mut out = [0u8; THUMBNAIL_INDEX_ENTRY_SIZE];
        out[0..8].copy_from_slice(&self.pts.to_le_bytes());
        out[8..16].copy_from_slice(&self.byte_offset.to_le_bytes());
        out[16..20].copy_from_slice(&self.byte_len.to_le_bytes());
        out[20..22].copy_from_slice(&self.grid_col.to_le_bytes());
        out[22..24].copy_from_slice(&self.grid_row.to_le_bytes());
        out
    }

    #[inline]
    pub fn from_le_bytes(b: [u8; THUMBNAIL_INDEX_ENTRY_SIZE]) -> Self {
        Self {
            pts: i64::from_le_bytes(b[0..8].try_into().unwrap()),
            byte_offset: u64::from_le_bytes(b[8..16].try_into().unwrap()),
            byte_len: u32::from_le_bytes(b[16..20].try_into().unwrap()),
            grid_col: u16::from_le_bytes(b[20..22].try_into().unwrap()),
            grid_row: u16::from_le_bytes(b[22..24].try_into().unwrap()),
        }
    }
}

/// Indexed timeline filmstrip containing low-resolution thumbnails and an
/// $O(\log N)$ `PTS -> byte_offset` lookup table.
#[derive(Clone, Debug, PartialEq)]
pub struct ThumbnailFilmstrip {
    pub codec: ThumbnailCodec,
    pub thumb_width: u32,
    pub thumb_height: u32,
    pub atlas_columns: u16,
    pub atlas_rows: u16,
    pub timebase: Rational,
    pub entries: Vec<ThumbnailIndexEntry>,
    /// Contiguous buffer containing all encoded thumbnail payloads.
    pub payload_blob: Vec<u8>,
}

impl ThumbnailFilmstrip {
    /// Constructs a `ThumbnailFilmstrip` from ordered `(pts, encoded_bytes)` pairs.
    pub fn from_encoded_frames(
        codec: ThumbnailCodec,
        thumb_width: u32,
        thumb_height: u32,
        timebase: Rational,
        mut frames: Vec<(i64, Vec<u8>)>,
    ) -> Self {
        frames.sort_by_key(|(pts, _)| *pts);

        let count = frames.len();
        let atlas_columns = if count == 0 {
            1
        } else {
            ((count as f64).sqrt().ceil() as u16).max(1)
        };
        let atlas_rows = if count == 0 {
            1
        } else {
            ((count + (atlas_columns as usize) - 1) / (atlas_columns as usize)) as u16
        };

        let payload_section_offset =
            (THUMBNAIL_HEADER_SIZE + count * THUMBNAIL_INDEX_ENTRY_SIZE) as u64;
        let total_payload: usize = frames.iter().map(|(_, b)| b.len()).sum();

        let mut entries = Vec::with_capacity(count);
        let mut payload_blob = Vec::with_capacity(total_payload);

        for (idx, (pts, bytes)) in frames.into_iter().enumerate() {
            let rel_offset = payload_blob.len() as u64;
            let abs_offset = payload_section_offset + rel_offset;
            let byte_len = bytes.len() as u32;
            let grid_col = (idx as u16) % atlas_columns;
            let grid_row = (idx as u16) / atlas_columns;

            entries.push(ThumbnailIndexEntry {
                pts,
                byte_offset: abs_offset,
                byte_len,
                grid_col,
                grid_row,
            });
            payload_blob.extend_from_slice(&bytes);
        }

        Self {
            codec,
            thumb_width,
            thumb_height,
            atlas_columns,
            atlas_rows,
            timebase,
            entries,
            payload_blob,
        }
    }

    /// Number of indexed thumbnails in the filmstrip.
    #[inline]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Locates the nearest thumbnail with `entry.pts <= target_pts` in $O(\log N)$
    /// time via binary search (clamped to the first thumbnail if `target_pts` precedes it).
    pub fn find_entry_at_or_before(&self, target_pts: i64) -> Option<&ThumbnailIndexEntry> {
        if self.entries.is_empty() {
            return None;
        }
        let pos = self.entries.partition_point(|e| e.pts <= target_pts);
        if pos == 0 {
            Some(&self.entries[0])
        } else {
            Some(&self.entries[pos - 1])
        }
    }

    /// Returns the compressed image bytes (JPEG/WebP) for a given [`ThumbnailIndexEntry`].
    pub fn thumbnail_bytes(&self, entry: &ThumbnailIndexEntry) -> Option<&[u8]> {
        let payload_section_offset =
            (THUMBNAIL_HEADER_SIZE + self.entries.len() * THUMBNAIL_INDEX_ENTRY_SIZE) as u64;
        let rel_start = entry.byte_offset.checked_sub(payload_section_offset)? as usize;
        let rel_end = rel_start.checked_add(entry.byte_len as usize)?;
        self.payload_blob.get(rel_start..rel_end)
    }

    /// Serializes the filmstrip (`header + 24B index table + payloads`) into bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let payload_section_offset =
            (THUMBNAIL_HEADER_SIZE + self.entries.len() * THUMBNAIL_INDEX_ENTRY_SIZE) as u64;
        let total_size = (payload_section_offset as usize) + self.payload_blob.len();
        let mut buf = Vec::with_capacity(total_size);

        let (codec_tag, codec_param) = self.codec.to_tag();

        buf.extend_from_slice(THUMBNAIL_ATLAS_MAGIC);
        buf.extend_from_slice(&THUMBNAIL_ATLAS_VERSION.to_le_bytes());
        buf.push(codec_tag);
        buf.push(codec_param);
        buf.extend_from_slice(&[0u8; 2]);
        buf.extend_from_slice(&self.thumb_width.to_le_bytes());
        buf.extend_from_slice(&self.thumb_height.to_le_bytes());
        buf.extend_from_slice(&self.atlas_columns.to_le_bytes());
        buf.extend_from_slice(&self.atlas_rows.to_le_bytes());
        buf.extend_from_slice(&self.timebase.num.to_le_bytes());
        buf.extend_from_slice(&self.timebase.den.to_le_bytes());
        buf.extend_from_slice(&(self.entries.len() as u32).to_le_bytes());
        buf.extend_from_slice(&payload_section_offset.to_le_bytes());
        debug_assert_eq!(buf.len(), THUMBNAIL_HEADER_SIZE);

        for entry in &self.entries {
            buf.extend_from_slice(&entry.to_le_bytes());
        }
        buf.extend_from_slice(&self.payload_blob);
        buf
    }

    /// Deserializes a `ThumbnailFilmstrip` from a `.atlas` binary buffer.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, MediaError> {
        if bytes.len() < THUMBNAIL_HEADER_SIZE {
            return Err(MediaError::CorruptFile(
                "Thumbnail atlas file smaller than 48-byte header".to_string(),
            ));
        }
        if &bytes[0..8] != THUMBNAIL_ATLAS_MAGIC {
            return Err(MediaError::CorruptFile(
                "Invalid thumbnail atlas magic (expected AETHTHMB)".to_string(),
            ));
        }
        let version = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
        if version != THUMBNAIL_ATLAS_VERSION {
            return Err(MediaError::CorruptFile(format!(
                "Unsupported thumbnail atlas version: {}",
                version
            )));
        }
        let codec = ThumbnailCodec::from_tag(bytes[12], bytes[13])?;
        let thumb_width = u32::from_le_bytes(bytes[16..20].try_into().unwrap());
        let thumb_height = u32::from_le_bytes(bytes[20..24].try_into().unwrap());
        let atlas_columns = u16::from_le_bytes(bytes[24..26].try_into().unwrap());
        let atlas_rows = u16::from_le_bytes(bytes[26..28].try_into().unwrap());
        let tb_num = i32::from_le_bytes(bytes[28..32].try_into().unwrap());
        let tb_den = i32::from_le_bytes(bytes[32..36].try_into().unwrap());
        if tb_num <= 0 || tb_den <= 0 {
            return Err(MediaError::CorruptFile(
                "Invalid timebase in thumbnail atlas header".to_string(),
            ));
        }
        let entry_count = u32::from_le_bytes(bytes[36..40].try_into().unwrap()) as usize;
        let payload_offset = u64::from_le_bytes(bytes[40..48].try_into().unwrap()) as usize;

        let index_end = THUMBNAIL_HEADER_SIZE
            .checked_add(
                entry_count
                    .checked_mul(THUMBNAIL_INDEX_ENTRY_SIZE)
                    .ok_or_else(|| {
                        MediaError::CorruptFile("Thumbnail index count overflow".to_string())
                    })?,
            )
            .ok_or_else(|| MediaError::CorruptFile("Thumbnail index end overflow".to_string()))?;

        if index_end > bytes.len() || payload_offset != index_end {
            return Err(MediaError::CorruptFile(
                "Truncated or misaligned thumbnail atlas index table".to_string(),
            ));
        }

        let mut entries = Vec::with_capacity(entry_count);
        for i in 0..entry_count {
            let start = THUMBNAIL_HEADER_SIZE + i * THUMBNAIL_INDEX_ENTRY_SIZE;
            let raw: [u8; THUMBNAIL_INDEX_ENTRY_SIZE] =
                bytes[start..start + THUMBNAIL_INDEX_ENTRY_SIZE]
                    .try_into()
                    .unwrap();
            let entry = ThumbnailIndexEntry::from_le_bytes(raw);
            let end = (entry.byte_offset as usize)
                .checked_add(entry.byte_len as usize)
                .ok_or_else(|| {
                    MediaError::CorruptFile("Thumbnail payload offset overflow".to_string())
                })?;
            if (entry.byte_offset as usize) < payload_offset || end > bytes.len() {
                return Err(MediaError::CorruptFile(format!(
                    "Thumbnail entry {} points outside file payload bounds",
                    i
                )));
            }
            entries.push(entry);
        }

        let payload_blob = bytes[payload_offset..].to_vec();

        Ok(Self {
            codec,
            thumb_width,
            thumb_height,
            atlas_columns,
            atlas_rows,
            timebase: Rational {
                num: tb_num,
                den: tb_den,
            },
            entries,
            payload_blob,
        })
    }

    /// Saves the `.atlas` file atomically via [`crate::cache::write_atomic`].
    pub fn save_to_file(&self, path: &Path) -> Result<(), MediaError> {
        use std::io::Write;
        let bytes = self.to_bytes();
        crate::cache::write_atomic(path, |f| f.write_all(&bytes))
    }

    /// Loads a `ThumbnailFilmstrip` from disk.
    pub fn load_from_file(path: &Path) -> Result<Self, MediaError> {
        let bytes = std::fs::read(path).map_err(|e| MediaError::IoError(e.to_string()))?;
        Self::from_bytes(&bytes)
    }

    /// Performs an $O(\log N)$ binary search over the on-disk `.atlas` index table
    /// and reads **only** the matching compressed thumbnail payload via `Seek`.
    pub fn read_thumbnail_at_or_before_from_file(
        path: &Path,
        target_pts: i64,
    ) -> Result<Option<(ThumbnailIndexEntry, Vec<u8>)>, MediaError> {
        let mut file = File::open(path).map_err(|e| MediaError::IoError(e.to_string()))?;
        let mut hdr = [0u8; THUMBNAIL_HEADER_SIZE];
        file.read_exact(&mut hdr)
            .map_err(|e| MediaError::CorruptFile(format!("Truncated atlas header: {}", e)))?;

        if &hdr[0..8] != THUMBNAIL_ATLAS_MAGIC {
            return Err(MediaError::CorruptFile(
                "Invalid thumbnail atlas magic".to_string(),
            ));
        }
        let entry_count = u32::from_le_bytes(hdr[36..40].try_into().unwrap()) as usize;
        if entry_count == 0 {
            return Ok(None);
        }

        let mut index_bytes = vec![0u8; entry_count * THUMBNAIL_INDEX_ENTRY_SIZE];
        file.read_exact(&mut index_bytes)
            .map_err(|e| MediaError::CorruptFile(format!("Truncated atlas index: {}", e)))?;

        let mut entries = Vec::with_capacity(entry_count);
        for chunk in index_bytes.chunks_exact(THUMBNAIL_INDEX_ENTRY_SIZE) {
            let raw: [u8; THUMBNAIL_INDEX_ENTRY_SIZE] = chunk.try_into().unwrap();
            entries.push(ThumbnailIndexEntry::from_le_bytes(raw));
        }

        let pos = entries.partition_point(|e| e.pts <= target_pts);
        let chosen = if pos == 0 {
            entries[0]
        } else {
            entries[pos - 1]
        };

        let mut payload = vec![0u8; chosen.byte_len as usize];
        file.seek(SeekFrom::Start(chosen.byte_offset))
            .map_err(|e| MediaError::IoError(e.to_string()))?;
        file.read_exact(&mut payload)
            .map_err(|e| MediaError::CorruptFile(format!("Truncated thumbnail payload: {}", e)))?;

        Ok(Some((chosen, payload)))
    }

    /// Stitches all thumbnails into a single RGBA8 spritesheet image of dimensions
    /// `(atlas_columns * thumb_width, atlas_rows * thumb_height)`.
    pub fn build_spritesheet_rgba(&self) -> Result<RgbaImage, MediaError> {
        let sheet_w = (self.atlas_columns.max(1) as u32) * self.thumb_width.max(1);
        let sheet_h = (self.atlas_rows.max(1) as u32) * self.thumb_height.max(1);
        let mut sheet = RgbaImage::new(sheet_w, sheet_h);

        for entry in &self.entries {
            if let Some(encoded) = self.thumbnail_bytes(entry) {
                let decoded = image::load_from_memory(encoded)
                    .map_err(|e| MediaError::CorruptFile(format!("Invalid tile image: {}", e)))?
                    .to_rgba8();
                let x0 = (entry.grid_col as u32) * self.thumb_width;
                let y0 = (entry.grid_row as u32) * self.thumb_height;
                for y in 0..decoded.height().min(self.thumb_height) {
                    for x in 0..decoded.width().min(self.thumb_width) {
                        sheet.put_pixel(x0 + x, y0 + y, *decoded.get_pixel(x, y));
                    }
                }
            }
        }

        Ok(sheet)
    }

    /// Exports the combined spritesheet grid as a single compressed image file (JPEG/WebP).
    pub fn export_spritesheet_image(&self, output_path: &Path) -> Result<(), MediaError> {
        use std::io::Write;
        let sheet = self.build_spritesheet_rgba()?;
        let bytes = encode_rgba_image(
            sheet.as_raw(),
            sheet.width(),
            sheet.height(),
            self.codec,
        )?;
        crate::cache::write_atomic(output_path, |f| f.write_all(&bytes))
    }
}

// ---------------------------------------------------------------------------
// Frame scaling & encoding helpers
// ---------------------------------------------------------------------------

/// Resizes a decoded [`VideoFrame`] to `(target_width, target_height)` and encodes
/// it with `codec` (JPEG or WebP).
pub fn encode_video_frame_thumbnail(
    frame: &VideoFrame,
    target_width: u32,
    target_height: u32,
    codec: ThumbnailCodec,
) -> Result<Vec<u8>, MediaError> {
    let mut owned = frame.clone();
    owned.ensure_rgba8();

    let src_w = owned.width.max(1);
    let src_h = owned.height.max(1);
    let expected_len = (src_w as usize) * (src_h as usize) * 4;
    if owned.data.len() < expected_len {
        return Err(MediaError::CorruptFile(
            "VideoFrame RGBA buffer smaller than dimensions".to_string(),
        ));
    }

    let img = RgbaImage::from_raw(src_w, src_h, owned.data[..expected_len].to_vec()).ok_or_else(
        || MediaError::CorruptFile("Failed to construct RgbaImage from VideoFrame".to_string()),
    )?;

    let resized = if src_w == target_width && src_h == target_height {
        img
    } else {
        image::imageops::resize(&img, target_width.max(1), target_height.max(1), FilterType::Triangle)
    };

    encode_rgba_image(
        resized.as_raw(),
        resized.width(),
        resized.height(),
        codec,
    )
}

/// Encodes an RGBA8 buffer of `(width, height)` into JPEG or WebP bytes in memory.
pub fn encode_rgba_image(
    rgba: &[u8],
    width: u32,
    height: u32,
    codec: ThumbnailCodec,
) -> Result<Vec<u8>, MediaError> {
    let mut out = Vec::new();
    match codec {
        ThumbnailCodec::Jpeg { quality } => {
            // Convert RGBA8 -> RGB8 for baseline JPEG compatibility
            let mut rgb = Vec::with_capacity((width as usize) * (height as usize) * 3);
            for px in rgba.chunks_exact(4) {
                rgb.push(px[0]);
                rgb.push(px[1]);
                rgb.push(px[2]);
            }
            let mut cursor = Cursor::new(&mut out);
            let mut encoder = JpegEncoder::new_with_quality(&mut cursor, quality.clamp(1, 100));
            encoder
                .encode(&rgb, width, height, ColorType::Rgb8.into())
                .map_err(|e| MediaError::IoError(format!("JPEG encode failed: {}", e)))?;
        }
        ThumbnailCodec::WebP => {
            let mut cursor = Cursor::new(&mut out);
            let encoder = WebPEncoder::new_lossless(&mut cursor);
            encoder
                .write_image(rgba, width, height, ExtendedColorType::Rgba8)
                .map_err(|e| MediaError::IoError(format!("WebP encode failed: {}", e)))?;
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Filmstrip extraction & Non-blocking Background Worker
// ---------------------------------------------------------------------------

/// Computes the sorted list of presentation timestamps (`PTS`) to sample for a media asset.
pub fn resolve_sample_pts_list(
    media_path: &Path,
    duration_pts: i64,
    timebase: Rational,
    strategy: ThumbnailStrategy,
    cache_manager: Option<&CacheManager>,
) -> Vec<i64> {
    let fps_ratio = if timebase.den > 0 && timebase.num > 0 {
        timebase.num as f64 / timebase.den as f64
    } else {
        60.0
    };

    match strategy {
        ThumbnailStrategy::Keyframes { min_interval_pts } => {
            let min_gap = min_interval_pts.max(1);
            if let Ok(fidx) =
                build_or_load_frame_index_with_timebase(media_path, timebase, cache_manager)
            {
                let mut pts_list = Vec::new();
                let mut last_pts: Option<i64> = None;
                for entry in fidx.entries() {
                    if entry.is_keyframe() {
                        if last_pts.map_or(true, |prev| entry.pts - prev >= min_gap) {
                            pts_list.push(entry.pts);
                            last_pts = Some(entry.pts);
                        }
                    }
                }
                if !pts_list.is_empty() {
                    return pts_list;
                }
            }
            // Fallback if container has no `.fidx` index (e.g. still image or non-ISO-BMFF):
            build_fixed_pts_schedule(duration_pts, min_gap.max((fps_ratio.round() as i64).max(1)))
        }
        ThumbnailStrategy::FixedIntervalPts { interval_pts } => {
            build_fixed_pts_schedule(duration_pts, interval_pts.max(1))
        }
        ThumbnailStrategy::FixedIntervalSeconds { interval_seconds } => {
            let step_pts = ((interval_seconds.max(0.05)) * fps_ratio).round() as i64;
            build_fixed_pts_schedule(duration_pts, step_pts.max(1))
        }
    }
}

fn build_fixed_pts_schedule(duration_pts: i64, step_pts: i64) -> Vec<i64> {
    let step = step_pts.max(1);
    if duration_pts <= 0 {
        return vec![0];
    }
    let mut out = Vec::new();
    let mut cur = 0i64;
    while cur < duration_pts {
        out.push(cur);
        cur = cur.saturating_add(step);
    }
    if out.is_empty() {
        out.push(0);
    }
    out
}

/// Extracts a [`ThumbnailFilmstrip`] from a video (or image/synthetic URI) and
/// optionally writes the resulting `.atlas` file to `output_path`.
pub fn extract_thumbnail_filmstrip(
    media_path_or_uri: &str,
    output_path: Option<&Path>,
    thumb_width: u32,
    thumb_height: u32,
    codec: ThumbnailCodec,
    strategy: ThumbnailStrategy,
    timebase: Rational,
    cache_manager: Option<&CacheManager>,
    cancel_flag: Option<&AtomicBool>,
    progress_per_mille: Option<&AtomicU32>,
) -> Result<ThumbnailFilmstrip, MediaError> {
    let mut decoder = open_video_decoder_with_timebase(media_path_or_uri, timebase)
        .map_err(|e| MediaError::CorruptFile(format!("Failed to open video decoder: {}", e)))?;

    let duration_pts = decoder.duration_pts();
    let sample_pts = if media_path_or_uri.starts_with("synthetic://") {
        match strategy {
            ThumbnailStrategy::FixedIntervalPts { interval_pts } => {
                build_fixed_pts_schedule(duration_pts, interval_pts.max(1))
            }
            ThumbnailStrategy::FixedIntervalSeconds { interval_seconds } => {
                let fps = if timebase.den > 0 && timebase.num > 0 {
                    timebase.num as f64 / timebase.den as f64
                } else {
                    60.0
                };
                let step = ((interval_seconds.max(0.05)) * fps).round() as i64;
                build_fixed_pts_schedule(duration_pts, step.max(1))
            }
            ThumbnailStrategy::Keyframes { min_interval_pts } => {
                build_fixed_pts_schedule(duration_pts, min_interval_pts.max(15))
            }
        }
    } else {
        resolve_sample_pts_list(
            Path::new(media_path_or_uri),
            duration_pts,
            timebase,
            strategy,
            cache_manager,
        )
    };

    let total = sample_pts.len().max(1);
    let mut encoded_frames = Vec::with_capacity(sample_pts.len());

    for (idx, &pts) in sample_pts.iter().enumerate() {
        if let Some(cancel) = cancel_flag {
            if cancel.load(Ordering::Relaxed) {
                return Err(MediaError::IoError(
                    "Thumbnail filmstrip extraction cancelled".to_string(),
                ));
            }
        }

        if let Ok(Some(frame)) = decoder.extract_frame_at_pts(pts) {
            let encoded = encode_video_frame_thumbnail(
                &frame,
                thumb_width.max(1),
                thumb_height.max(1),
                codec,
            )?;
            encoded_frames.push((pts, encoded));
        }

        if let Some(prog) = progress_per_mille {
            let p = (((idx + 1) * 950) / total).min(950) as u32;
            prog.store(p, Ordering::Relaxed);
        }
    }

    let strip = ThumbnailFilmstrip::from_encoded_frames(
        codec,
        thumb_width.max(1),
        thumb_height.max(1),
        timebase,
        encoded_frames,
    );

    if let Some(out) = output_path {
        strip.save_to_file(out)?;
    }

    if let Some(prog) = progress_per_mille {
        prog.store(1000, Ordering::Release);
    }

    Ok(strip)
}

/// Handle to an asynchronous background thumbnail filmstrip extraction worker.
pub struct ThumbnailJobHandle {
    cancel_flag: Arc<AtomicBool>,
    progress_per_mille: Arc<AtomicU32>,
    finished_flag: Arc<AtomicBool>,
    join_handle: Option<JoinHandle<Result<ThumbnailFilmstrip, MediaError>>>,
}

impl ThumbnailJobHandle {
    /// Signals the background worker to abort before the next frame extraction.
    pub fn cancel(&self) {
        self.cancel_flag.store(true, Ordering::SeqCst);
    }

    /// Returns progress in `[0, 1000]` (`1000` = 100.0%).
    pub fn progress_per_mille(&self) -> u32 {
        self.progress_per_mille.load(Ordering::Relaxed)
    }

    /// Returns `true` once the background worker thread has finished.
    pub fn is_finished(&self) -> bool {
        self.finished_flag.load(Ordering::Acquire)
    }

    /// Waits for completion and returns the extracted [`ThumbnailFilmstrip`].
    pub fn join(mut self) -> Result<ThumbnailFilmstrip, MediaError> {
        match self.join_handle.take() {
            Some(h) => h
                .join()
                .map_err(|_| MediaError::IoError("Thumbnail worker thread panicked".to_string()))?,
            None => Err(MediaError::IoError(
                "Thumbnail worker already joined".to_string(),
            )),
        }
    }
}

/// Spawns a non-blocking background worker thread to extract timeline thumbnails
/// (`160x90` default) and persist the `.atlas` file without blocking playback or demuxing.
pub fn spawn_thumbnail_filmstrip_worker(
    media_path_or_uri: impl Into<String>,
    output_path: Option<PathBuf>,
    thumb_width: u32,
    thumb_height: u32,
    codec: ThumbnailCodec,
    strategy: ThumbnailStrategy,
    timebase: Rational,
) -> Result<ThumbnailJobHandle, MediaError> {
    let media_str = media_path_or_uri.into();
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let progress = Arc::new(AtomicU32::new(0));
    let finished = Arc::new(AtomicBool::new(false));

    let cancel_clone = Arc::clone(&cancel_flag);
    let progress_clone = Arc::clone(&progress);
    let finished_clone = Arc::clone(&finished);

    let handle = thread::Builder::new()
        .name("aether-thumbnail-worker".to_string())
        .spawn(move || {
            let res = extract_thumbnail_filmstrip(
                &media_str,
                output_path.as_deref(),
                thumb_width,
                thumb_height,
                codec,
                strategy,
                timebase,
                None,
                Some(&cancel_clone),
                Some(&progress_clone),
            );
            finished_clone.store(true, Ordering::Release);
            res
        })
        .map_err(|e| MediaError::IoError(format!("Failed to spawn thumbnail worker: {}", e)))?;

    Ok(ThumbnailJobHandle {
        cancel_flag,
        progress_per_mille: progress,
        finished_flag: finished,
        join_handle: Some(handle),
    })
}

/// Convenience helper using default `160x90` dimensions and 60fps timebase.
pub fn spawn_default_thumbnail_worker(
    media_path_or_uri: impl Into<String>,
    output_path: Option<PathBuf>,
    codec: ThumbnailCodec,
    strategy: ThumbnailStrategy,
) -> Result<ThumbnailJobHandle, MediaError> {
    spawn_thumbnail_filmstrip_worker(
        media_path_or_uri,
        output_path,
        DEFAULT_THUMBNAIL_WIDTH,
        DEFAULT_THUMBNAIL_HEIGHT,
        codec,
        strategy,
        DEFAULT_TIMEBASE,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn thumbnail_index_entry_is_24_bytes_and_roundtrips() {
        assert_eq!(
            std::mem::size_of::<ThumbnailIndexEntry>(),
            THUMBNAIL_INDEX_ENTRY_SIZE
        );
        let entry = ThumbnailIndexEntry {
            pts: 180,
            byte_offset: 4096,
            byte_len: 2048,
            grid_col: 2,
            grid_row: 1,
        };
        let raw = entry.to_le_bytes();
        let decoded = ThumbnailIndexEntry::from_le_bytes(raw);
        assert_eq!(decoded, entry);

        let (u0, v0, u1, v1) = decoded.uv_rect(4, 2);
        assert!((u0 - 0.5).abs() < 1e-6);
        assert!((v0 - 0.5).abs() < 1e-6);
        assert!((u1 - 0.75).abs() < 1e-6);
        assert!((v1 - 1.0).abs() < 1e-6);
    }

    #[test]
    fn extracts_160x90_jpeg_and_webp_filmstrips_and_seeks_by_pts() {
        let dir = tempdir().unwrap();
        let atlas_path = dir.path().join("filmstrip.atlas");
        let sheet_path = dir.path().join("spritesheet.webp");

        let uri = "synthetic://colorbars?width=640&height=360&fps=30&duration=1.0";
        let worker = spawn_default_thumbnail_worker(
            uri,
            Some(atlas_path.clone()),
            ThumbnailCodec::WebP,
            ThumbnailStrategy::FixedIntervalPts { interval_pts: 15 },
        )
        .unwrap();

        let strip = worker.join().unwrap();
        assert_eq!(strip.thumb_width, 160);
        assert_eq!(strip.thumb_height, 90);
        assert_eq!(strip.len(), 4); // PTS 0, 15, 30, 45
        assert!(atlas_path.exists());

        // Roundtrip from disk
        let loaded = ThumbnailFilmstrip::load_from_file(&atlas_path).unwrap();
        assert_eq!(loaded, strip);

        // Binary search by timestamp (`PTS -> offset`)
        let (entry, payload) =
            ThumbnailFilmstrip::read_thumbnail_at_or_before_from_file(&atlas_path, 35)
                .unwrap()
                .unwrap();
        assert_eq!(entry.pts, 30);
        let img = image::load_from_memory(&payload).unwrap();
        assert_eq!(img.width(), 160);
        assert_eq!(img.height(), 90);

        // Spritesheet export
        strip.export_spritesheet_image(&sheet_path).unwrap();
        assert!(sheet_path.exists());
    }

    #[test]
    fn extracts_keyframe_thumbnails_from_mp4_fixture() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("sample_64x64_10frames.mp4");
        if !fixture.exists() {
            return;
        }

        let dir = tempdir().unwrap();
        let cache_mgr = CacheManager::for_project(dir.path());
        let atlas_path = dir.path().join("mp4_keyframes.atlas");

        let strip = extract_thumbnail_filmstrip(
            fixture.to_str().unwrap(),
            Some(&atlas_path),
            160,
            90,
            ThumbnailCodec::Jpeg { quality: 80 },
            ThumbnailStrategy::Keyframes {
                min_interval_pts: 1,
            },
            Rational { num: 60, den: 1 },
            Some(&cache_mgr),
            None,
            None,
        )
        .unwrap();

        assert!(!strip.is_empty());
        assert_eq!(strip.entries[0].pts, 0);
        assert!(atlas_path.exists());
    }
}
