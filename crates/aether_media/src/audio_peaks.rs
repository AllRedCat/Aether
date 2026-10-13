//! Multi-resolution (LOD pyramid) audio waveform peak extractor, binary `.peak`
//! sidecar serializer, and non-blocking background worker.
//!
//! # Binary Format (`AETHPEAK` v2)
//! ```text
//! Offset  Size  Field
//! ------  ----  -----
//! 0       8     magic = b"AETHPEAK"
//! 8       4     version = 2 (u32 LE)
//! 12      4     channels (u32 LE)
//! 16      4     sample_rate (u32 LE)
//! 20      4     lod_count (u32 LE)
//! 24      8     total_frames (u64 LE)
//! 32      16*L  LOD Directory Table (L = lod_count entries, 16 bytes each):
//!                 [0..4]   samples_per_peak (u32 LE)
//!                 [4..8]   bins_per_channel (u32 LE)
//!                 [8..16]  byte_offset (u64 LE) from start of file
//! ...     ...   LOD Payloads:
//!                 For each channel `0..channels`:
//!                   `bins_per_channel` contiguous records of `AudioPeakSample`:
//!                     min: f32 LE (4B), max: f32 LE (4B), rms: f32 LE (4B) = 12 bytes
//! ```
//! Legacy `AETHPEAK` v1 files (single-level `(min, max)` pairs) remain supported
//! on read via [`AudioPeakPyramid::from_bytes`].

use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use serde::{Deserialize, Serialize};

use crate::decoder::open_audio_decoder;

pub const AETHER_PEAKS_MAGIC: &[u8; 8] = b"AETHPEAK";
pub const AETHER_PEAKS_VERSION_V1: u32 = 1;
pub const AETHER_PEAKS_VERSION: u32 = 2;

/// Default base window size (LOD 0) in PCM frames per peak bin.
pub const DEFAULT_BASE_SAMPLES_PER_PEAK: u32 = 256;

/// Default reduction factor between consecutive LOD pyramid levels (4x).
/// LOD 0 = 256 samples/pico, LOD 1 = 1024 samples/pico, LOD 2 = 4096 samples/pico.
pub const DEFAULT_LOD_FACTOR: u32 = 4;

/// Default number of mipmap LOD levels in the waveform pyramid
/// (e.g. 256, 1024, 4096, 16384, 65536, 262144 samples/bin with 4x reduction).
pub const DEFAULT_LOD_LEVELS: usize = 6;

const HEADER_FIXED_SIZE: usize = 32;
const LOD_DIR_ENTRY_SIZE: usize = 16;
const PEAK_SAMPLE_BYTE_SIZE: usize = 12;

/// Single waveform peak bin storing minimum amplitude, maximum amplitude, and
/// Root-Mean-Square (RMS) energy in normalized `[-1.0, 1.0]` float range.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AudioPeak {
    pub min: f32, // Amplitude mínima no bloco (-1.0 a 0.0)
    pub max: f32, // Amplitude máxima no bloco (0.0 a 1.0)
    pub rms: f32, // Energia média (Root Mean Square)
}

const _: [(); 12] = [(); std::mem::size_of::<AudioPeak>()];
const _: [(); 4] = [(); std::mem::align_of::<AudioPeak>()];

/// Backwards-compatible alias for [`AudioPeak`].
pub type AudioPeakSample = AudioPeak;

impl AudioPeak {
    #[inline]
    pub const fn new(min: f32, max: f32, rms: f32) -> Self {
        Self { min, max, rms }
    }

    #[inline]
    pub const fn silence() -> Self {
        Self {
            min: 0.0,
            max: 0.0,
            rms: 0.0,
        }
    }
}

/// One level of detail (LOD) in the multi-resolution waveform pyramid.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AudioPeakLodLevel {
    /// Zero-based LOD index (`0` = finest resolution, `k` = $4^k \times$ base window).
    pub lod: u32,
    /// Number of PCM frames aggregated per peak bin at this level.
    pub samples_per_peak: u32,
    /// Peak bins per audio channel (`channels[ch][bin_idx]`).
    pub channels: Vec<Vec<AudioPeak>>,
}

impl AudioPeakLodLevel {
    /// Number of peak bins per channel at this LOD level.
    #[inline]
    pub fn bins_per_channel(&self) -> usize {
        self.channels.first().map(|c| c.len()).unwrap_or(0)
    }

    /// Combines all channels at this LOD level into a single mono waveform
    /// (`min` of channel mins, `max` of channel maxes, quadratic mean of channel RMS).
    pub fn mixdown_mono(&self) -> Vec<AudioPeakSample> {
        let bins = self.bins_per_channel();
        let ch_count = self.channels.len();
        if ch_count == 0 || bins == 0 {
            return Vec::new();
        }
        if ch_count == 1 {
            return self.channels[0].clone();
        }

        let mut out = Vec::with_capacity(bins);
        for i in 0..bins {
            let mut min_v = f32::MAX;
            let mut max_v = f32::MIN;
            let mut rms_sq_sum = 0.0f64;
            for ch in &self.channels {
                let p = ch[i];
                if p.min < min_v {
                    min_v = p.min;
                }
                if p.max > max_v {
                    max_v = p.max;
                }
                rms_sq_sum += (p.rms as f64) * (p.rms as f64);
            }
            let rms = ((rms_sq_sum / ch_count as f64).sqrt() as f32).clamp(0.0, 1.0);
            out.push(AudioPeakSample::new(min_v, max_v, rms));
        }
        out
    }
}

/// Complete multi-level LOD audio peak pyramid persisted in `.peak` / `.aether_peaks` files.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AudioPeakPyramid {
    pub channels: u32,
    pub sample_rate: u32,
    pub total_frames: u64,
    pub levels: Vec<AudioPeakLodLevel>,
}

impl AudioPeakPyramid {
    /// Returns a specific LOD level by index (`0..levels.len()`).
    #[inline]
    pub fn level(&self, lod: usize) -> Option<&AudioPeakLodLevel> {
        self.levels.get(lod)
    }

    /// Selects the optimal LOD level for a given timeline zoom ratio
    /// (`samples_per_pixel`), picking the highest-resolution level whose
    /// `samples_per_peak <= samples_per_pixel` (or LOD 0 when zoomed in closely).
    pub fn select_lod_for_zoom(&self, samples_per_pixel: f64) -> Option<&AudioPeakLodLevel> {
        if self.levels.is_empty() {
            return None;
        }
        let mut chosen = &self.levels[0];
        for lvl in &self.levels {
            if (lvl.samples_per_peak as f64) <= samples_per_pixel {
                chosen = lvl;
            } else {
                break;
            }
        }
        Some(chosen)
    }

    /// Serializes the entire multi-LOD pyramid into the `AETHPEAK` v2 binary format.
    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        let lod_count = self.levels.len();
        let dir_bytes = lod_count * LOD_DIR_ENTRY_SIZE;
        let payload_start = HEADER_FIXED_SIZE + dir_bytes;

        let mut total_payload_bytes = 0usize;
        for lvl in &self.levels {
            let bins = lvl.bins_per_channel();
            total_payload_bytes += (self.channels as usize) * bins * PEAK_SAMPLE_BYTE_SIZE;
        }

        let mut buf = Vec::with_capacity(payload_start + total_payload_bytes);

        // Fixed 32-byte header
        buf.write_all(AETHER_PEAKS_MAGIC)
            .map_err(|e| e.to_string())?;
        buf.write_u32::<LittleEndian>(AETHER_PEAKS_VERSION)
            .map_err(|e| e.to_string())?;
        buf.write_u32::<LittleEndian>(self.channels)
            .map_err(|e| e.to_string())?;
        buf.write_u32::<LittleEndian>(self.sample_rate)
            .map_err(|e| e.to_string())?;
        buf.write_u32::<LittleEndian>(lod_count as u32)
            .map_err(|e| e.to_string())?;
        buf.write_u64::<LittleEndian>(self.total_frames)
            .map_err(|e| e.to_string())?;

        // Directory table (16 bytes per LOD)
        let mut running_offset = payload_start as u64;
        for lvl in &self.levels {
            let bins = lvl.bins_per_channel() as u32;
            buf.write_u32::<LittleEndian>(lvl.samples_per_peak)
                .map_err(|e| e.to_string())?;
            buf.write_u32::<LittleEndian>(bins)
                .map_err(|e| e.to_string())?;
            buf.write_u64::<LittleEndian>(running_offset)
                .map_err(|e| e.to_string())?;
            running_offset +=
                (self.channels as u64) * (bins as u64) * (PEAK_SAMPLE_BYTE_SIZE as u64);
        }

        // Contiguous LOD payloads
        for lvl in &self.levels {
            let bins = lvl.bins_per_channel();
            for ch in 0..(self.channels as usize) {
                let ch_slice = lvl.channels.get(ch);
                for bin_idx in 0..bins {
                    let p = ch_slice
                        .and_then(|v| v.get(bin_idx))
                        .copied()
                        .unwrap_or_else(AudioPeakSample::silence);
                    buf.write_f32::<LittleEndian>(p.min)
                        .map_err(|e| e.to_string())?;
                    buf.write_f32::<LittleEndian>(p.max)
                        .map_err(|e| e.to_string())?;
                    buf.write_f32::<LittleEndian>(p.rms)
                        .map_err(|e| e.to_string())?;
                }
            }
        }

        Ok(buf)
    }

    /// Deserializes an `AudioPeakPyramid` from `AETHPEAK` v2 or legacy v1 bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() < 28 {
            return Err("Peak file smaller than minimum header size".to_string());
        }
        if &bytes[0..8] != AETHER_PEAKS_MAGIC {
            return Err("Invalid peak file magic header (expected AETHPEAK)".to_string());
        }

        let mut cursor = std::io::Cursor::new(&bytes[8..]);
        let version = cursor
            .read_u32::<LittleEndian>()
            .map_err(|e| e.to_string())?;
        let channels = cursor
            .read_u32::<LittleEndian>()
            .map_err(|e| e.to_string())?;
        let sample_rate = cursor
            .read_u32::<LittleEndian>()
            .map_err(|e| e.to_string())?;

        if channels == 0 || channels > 64 {
            return Err(format!("Invalid channel count in peak file: {}", channels));
        }

        match version {
            AETHER_PEAKS_VERSION_V1 => {
                let samples_per_peak = cursor
                    .read_u32::<LittleEndian>()
                    .map_err(|e| e.to_string())?;
                let peak_count = cursor
                    .read_u32::<LittleEndian>()
                    .map_err(|e| e.to_string())? as usize;
                let expected_len = 28 + (channels as usize) * peak_count * 8;
                if bytes.len() < expected_len {
                    return Err("Truncated AETHPEAK v1 payload".to_string());
                }

                let mut ch_vecs = Vec::with_capacity(channels as usize);
                for _ in 0..channels {
                    let mut v = Vec::with_capacity(peak_count);
                    for _ in 0..peak_count {
                        let min = cursor
                            .read_f32::<LittleEndian>()
                            .map_err(|e| e.to_string())?;
                        let max = cursor
                            .read_f32::<LittleEndian>()
                            .map_err(|e| e.to_string())?;
                        let rms = (((min as f64).powi(2) + (max as f64).powi(2)) * 0.5).sqrt()
                            as f32;
                        v.push(AudioPeakSample::new(min, max, rms.clamp(0.0, 1.0)));
                    }
                    ch_vecs.push(v);
                }

                let total_frames = (peak_count as u64) * (samples_per_peak as u64);
                let base_level = AudioPeakLodLevel {
                    lod: 0,
                    samples_per_peak,
                    channels: ch_vecs,
                };
                let levels = build_lod_chain_from_base(base_level, DEFAULT_LOD_LEVELS);
                Ok(Self {
                    channels,
                    sample_rate,
                    total_frames,
                    levels,
                })
            }
            AETHER_PEAKS_VERSION => {
                if bytes.len() < HEADER_FIXED_SIZE {
                    return Err("Truncated AETHPEAK v2 header".to_string());
                }
                let lod_count = cursor
                    .read_u32::<LittleEndian>()
                    .map_err(|e| e.to_string())? as usize;
                let total_frames = cursor
                    .read_u64::<LittleEndian>()
                    .map_err(|e| e.to_string())?;

                if lod_count > 32 {
                    return Err(format!("Excessive LOD count in peak file: {}", lod_count));
                }
                if bytes.len() < HEADER_FIXED_SIZE + lod_count * LOD_DIR_ENTRY_SIZE {
                    return Err("Truncated AETHPEAK v2 LOD directory".to_string());
                }

                let mut dir = Vec::with_capacity(lod_count);
                for _ in 0..lod_count {
                    let spp = cursor
                        .read_u32::<LittleEndian>()
                        .map_err(|e| e.to_string())?;
                    let bins = cursor
                        .read_u32::<LittleEndian>()
                        .map_err(|e| e.to_string())? as usize;
                    let offset = cursor
                        .read_u64::<LittleEndian>()
                        .map_err(|e| e.to_string())? as usize;
                    dir.push((spp, bins, offset));
                }

                let mut levels = Vec::with_capacity(lod_count);
                for (lod_idx, (spp, bins, offset)) in dir.into_iter().enumerate() {
                    let level_byte_len = (channels as usize)
                        .checked_mul(bins)
                        .and_then(|v| v.checked_mul(PEAK_SAMPLE_BYTE_SIZE))
                        .ok_or_else(|| "Peak level size overflow".to_string())?;
                    let end = offset
                        .checked_add(level_byte_len)
                        .ok_or_else(|| "Peak level offset overflow".to_string())?;
                    if end > bytes.len() {
                        return Err(format!("Truncated AETHPEAK v2 payload at LOD {}", lod_idx));
                    }

                    let mut lvl_cursor = std::io::Cursor::new(&bytes[offset..end]);
                    let mut ch_vecs = Vec::with_capacity(channels as usize);
                    for _ in 0..channels {
                        let mut v = Vec::with_capacity(bins);
                        for _ in 0..bins {
                            let min = lvl_cursor
                                .read_f32::<LittleEndian>()
                                .map_err(|e| e.to_string())?;
                            let max = lvl_cursor
                                .read_f32::<LittleEndian>()
                                .map_err(|e| e.to_string())?;
                            let rms = lvl_cursor
                                .read_f32::<LittleEndian>()
                                .map_err(|e| e.to_string())?;
                            v.push(AudioPeakSample::new(min, max, rms));
                        }
                        ch_vecs.push(v);
                    }

                    levels.push(AudioPeakLodLevel {
                        lod: lod_idx as u32,
                        samples_per_peak: spp,
                        channels: ch_vecs,
                    });
                }

                Ok(Self {
                    channels,
                    sample_rate,
                    total_frames,
                    levels,
                })
            }
            other => Err(format!("Unsupported AETHPEAK version: {}", other)),
        }
    }

    /// Saves the pyramid to disk atomically (writes to `.tmp` then renames).
    pub fn save_to_file(&self, path: &Path) -> Result<(), String> {
        let bytes = self.to_bytes()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create peak cache dir: {}", e))?;
        }
        let tmp_path = path.with_extension("peak.tmp");
        {
            let mut file = File::create(&tmp_path)
                .map_err(|e| format!("Failed to create temp peak file: {}", e))?;
            file.write_all(&bytes)
                .map_err(|e| format!("Failed to write peak data: {}", e))?;
            file.flush()
                .map_err(|e| format!("Failed to flush peak file: {}", e))?;
        }
        fs::rename(&tmp_path, path)
            .map_err(|e| format!("Failed to finalize peak file: {}", e))?;
        Ok(())
    }

    /// Loads an `AudioPeakPyramid` from a `.peak` / `.aether_peaks` file on disk.
    pub fn load_from_file(path: &Path) -> Result<Self, String> {
        let bytes =
            fs::read(path).map_err(|e| format!("Failed to read peak file {:?}: {}", path, e))?;
        Self::from_bytes(&bytes)
    }

    /// Reads only a single requested `lod` level directly from disk via the
    /// v2 LOD offset directory without loading higher-density levels into memory.
    pub fn load_lod_from_file(path: &Path, lod: u32) -> Result<AudioPeakLodLevel, String> {
        let mut file =
            File::open(path).map_err(|e| format!("Failed to open peak file {:?}: {}", path, e))?;
        let mut hdr = [0u8; HEADER_FIXED_SIZE];
        file.read_exact(&mut hdr)
            .map_err(|e| format!("Failed to read peak header: {}", e))?;

        if &hdr[0..8] != AETHER_PEAKS_MAGIC {
            return Err("Invalid peak file magic header".to_string());
        }
        let version = u32::from_le_bytes(hdr[8..12].try_into().unwrap());
        if version != AETHER_PEAKS_VERSION {
            // Fallback for v1 files: load full file and return requested LOD
            let pyramid = Self::load_from_file(path)?;
            let idx = (lod as usize).min(pyramid.levels.len().saturating_sub(1));
            return pyramid
                .levels
                .into_iter()
                .nth(idx)
                .ok_or_else(|| "Peak file contains no LOD levels".to_string());
        }

        let channels = u32::from_le_bytes(hdr[12..16].try_into().unwrap()) as usize;
        let lod_count = u32::from_le_bytes(hdr[20..24].try_into().unwrap()) as usize;
        if lod_count == 0 {
            return Err("Peak file has 0 LOD levels".to_string());
        }
        let target_lod = (lod as usize).min(lod_count - 1);

        let dir_offset = HEADER_FIXED_SIZE + target_lod * LOD_DIR_ENTRY_SIZE;
        file.seek(SeekFrom::Start(dir_offset as u64))
            .map_err(|e| e.to_string())?;
        let mut dir_entry = [0u8; LOD_DIR_ENTRY_SIZE];
        file.read_exact(&mut dir_entry)
            .map_err(|e| e.to_string())?;

        let samples_per_peak = u32::from_le_bytes(dir_entry[0..4].try_into().unwrap());
        let bins = u32::from_le_bytes(dir_entry[4..8].try_into().unwrap()) as usize;
        let payload_offset = u64::from_le_bytes(dir_entry[8..16].try_into().unwrap());

        let byte_len = channels * bins * PEAK_SAMPLE_BYTE_SIZE;
        let mut payload = vec![0u8; byte_len];
        file.seek(SeekFrom::Start(payload_offset))
            .map_err(|e| e.to_string())?;
        file.read_exact(&mut payload)
            .map_err(|e| format!("Truncated peak LOD payload: {}", e))?;

        let mut cursor = std::io::Cursor::new(&payload);
        let mut ch_vecs = Vec::with_capacity(channels);
        for _ in 0..channels {
            let mut v = Vec::with_capacity(bins);
            for _ in 0..bins {
                let min = cursor.read_f32::<LittleEndian>().map_err(|e| e.to_string())?;
                let max = cursor.read_f32::<LittleEndian>().map_err(|e| e.to_string())?;
                let rms = cursor.read_f32::<LittleEndian>().map_err(|e| e.to_string())?;
                v.push(AudioPeakSample::new(min, max, rms));
            }
            ch_vecs.push(v);
        }

        Ok(AudioPeakLodLevel {
            lod: target_lod as u32,
            samples_per_peak,
            channels: ch_vecs,
        })
    }
}

// ---------------------------------------------------------------------------
// Peak math & streaming accumulators
// ---------------------------------------------------------------------------

/// Sanitizes an individual PCM float sample: replaces `NaN` with `0.0` and clamps
/// amplitude to full-scale `[-1.0, 1.0]`.
#[inline]
pub fn sanitize_pcm_sample(sample: f32) -> f32 {
    if sample.is_nan() {
        0.0
    } else {
        sample.clamp(-1.0, 1.0)
    }
}

/// Computes `(min, max, rms)` for a single window of mono PCM samples.
///
/// - `NaN` values are treated as `0.0`.
/// - Values outside `[-1.0, 1.0]` are clamped to full-scale `[-1.0, 1.0]`.
/// - Empty slices return `AudioPeakSample::silence()` (`0.0, 0.0, 0.0`).
pub fn compute_window_peak(samples: &[f32]) -> AudioPeakSample {
    if samples.is_empty() {
        return AudioPeakSample::silence();
    }

    let mut min = f32::MAX;
    let mut max = f32::MIN;
    let mut sum_sq = 0.0f64;

    for &raw in samples {
        let s = sanitize_pcm_sample(raw);
        if s < min {
            min = s;
        }
        if s > max {
            max = s;
        }
        sum_sq += (s as f64) * (s as f64);
    }

    if min > max {
        return AudioPeakSample::silence();
    }

    let rms = ((sum_sq / (samples.len() as f64)).sqrt() as f32).clamp(0.0, 1.0);
    AudioPeakSample::new(min, max, rms)
}

/// Computes `(min, max)` for a slice of PCM samples.
pub fn compute_min_max(samples: &[f32]) -> (f32, f32) {
    let peak = compute_window_peak(samples);
    (peak.min, peak.max)
}

/// Constant-memory ($O(1)$) streaming accumulator for a single channel window.
#[derive(Clone, Copy, Debug)]
struct WindowAccumulator {
    min: f32,
    max: f32,
    sum_sq: f64,
    count: u32,
}

impl WindowAccumulator {
    #[inline]
    fn new() -> Self {
        Self {
            min: f32::MAX,
            max: f32::MIN,
            sum_sq: 0.0,
            count: 0,
        }
    }

    #[inline]
    fn push(&mut self, raw_sample: f32) {
        let s = sanitize_pcm_sample(raw_sample);
        if s < self.min {
            self.min = s;
        }
        if s > self.max {
            self.max = s;
        }
        self.sum_sq += (s as f64) * (s as f64);
        self.count += 1;
    }

    #[inline]
    fn flush(&mut self) -> Option<AudioPeakSample> {
        if self.count == 0 {
            return None;
        }
        let rms = ((self.sum_sq / (self.count as f64)).sqrt() as f32).clamp(0.0, 1.0);
        let sample = AudioPeakSample::new(self.min, self.max, rms);
        *self = Self::new();
        Some(sample)
    }
}

/// Downsamples an [`AudioPeakLodLevel`] by a given factor (default 4x: 256 -> 1024 -> 4096),
/// preserving exact `min`/`max` envelopes and quadratic-mean `rms`.
pub fn downsample_lod_level_with_factor(
    prev: &AudioPeakLodLevel,
    next_lod: u32,
    factor: u32,
) -> AudioPeakLodLevel {
    let f = (factor as usize).max(2);
    let next_spp = prev.samples_per_peak.saturating_mul(factor);
    let mut next_channels = Vec::with_capacity(prev.channels.len());

    for ch in &prev.channels {
        let out_len = (ch.len() + f - 1) / f;
        let mut out = Vec::with_capacity(out_len);
        for chunk in ch.chunks(f) {
            let mut min_v = f32::MAX;
            let mut max_v = f32::MIN;
            let mut sum_sq = 0.0f64;
            for p in chunk {
                if p.min < min_v {
                    min_v = p.min;
                }
                if p.max > max_v {
                    max_v = p.max;
                }
                sum_sq += (p.rms as f64) * (p.rms as f64);
            }
            let rms = ((sum_sq / (chunk.len() as f64)).sqrt() as f32).clamp(0.0, 1.0);
            out.push(AudioPeak::new(min_v, max_v, rms));
        }
        next_channels.push(out);
    }

    AudioPeakLodLevel {
        lod: next_lod,
        samples_per_peak: next_spp,
        channels: next_channels,
    }
}

/// Downsamples an [`AudioPeakLodLevel`] by the default 4x factor (256 -> 1024 -> 4096).
pub fn downsample_lod_level(prev: &AudioPeakLodLevel, next_lod: u32) -> AudioPeakLodLevel {
    downsample_lod_level_with_factor(prev, next_lod, DEFAULT_LOD_FACTOR)
}

/// Builds a multi-level LOD chain starting from `base_level` (LOD 0) using 4x downsampling.
pub fn build_lod_chain_from_base(
    base_level: AudioPeakLodLevel,
    lod_count: usize,
) -> Vec<AudioPeakLodLevel> {
    build_lod_chain_from_base_with_factor(base_level, lod_count, DEFAULT_LOD_FACTOR)
}

/// Builds a multi-level LOD chain starting from `base_level` (LOD 0) with a custom downsampling factor.
pub fn build_lod_chain_from_base_with_factor(
    base_level: AudioPeakLodLevel,
    lod_count: usize,
    factor: u32,
) -> Vec<AudioPeakLodLevel> {
    let total_levels = lod_count.max(1);
    let mut levels = Vec::with_capacity(total_levels);
    levels.push(base_level);

    for lod_idx in 1..total_levels {
        let next = downsample_lod_level_with_factor(&levels[lod_idx - 1], lod_idx as u32, factor);
        levels.push(next);
    }

    levels
}

/// Builds a complete [`AudioPeakPyramid`] in memory from an interleaved `f32` PCM buffer.
pub fn build_peak_pyramid_from_interleaved(
    interleaved_samples: &[f32],
    channels: usize,
    sample_rate: u32,
    base_samples_per_peak: u32,
    lod_count: usize,
) -> Result<AudioPeakPyramid, String> {
    if channels == 0 {
        return Err("channels must be greater than 0".to_string());
    }
    if base_samples_per_peak == 0 {
        return Err("base_samples_per_peak must be greater than 0".to_string());
    }
    if sample_rate == 0 {
        return Err("sample_rate must be greater than 0".to_string());
    }

    let total_frames = interleaved_samples.len() / channels;
    let est_bins = (total_frames + base_samples_per_peak as usize - 1)
        .checked_div(base_samples_per_peak as usize)
        .unwrap_or(0);

    let mut base_channels: Vec<Vec<AudioPeak>> =
        (0..channels).map(|_| Vec::with_capacity(est_bins)).collect();
    let mut accumulators = vec![WindowAccumulator::new(); channels];

    for frame_idx in 0..total_frames {
        let frame_base = frame_idx * channels;
        for ch in 0..channels {
            let acc = &mut accumulators[ch];
            acc.push(interleaved_samples[frame_base + ch]);
            if acc.count == base_samples_per_peak {
                if let Some(peak) = acc.flush() {
                    base_channels[ch].push(peak);
                }
            }
        }
    }

    for ch in 0..channels {
        if let Some(rem) = accumulators[ch].flush() {
            base_channels[ch].push(rem);
        }
    }

    let base_level = AudioPeakLodLevel {
        lod: 0,
        samples_per_peak: base_samples_per_peak,
        channels: base_channels,
    };
    let levels = build_lod_chain_from_base(base_level, lod_count);

    Ok(AudioPeakPyramid {
        channels: channels as u32,
        sample_rate,
        total_frames: total_frames as u64,
        levels,
    })
}

// ---------------------------------------------------------------------------
// File-based waveform extraction & Non-blocking Background Worker
// ---------------------------------------------------------------------------

/// Extracts a multi-level LOD [`AudioPeakPyramid`] from an audio or video file
/// and persists it into a structured binary `.peak` / `.aether_peaks` file.
pub fn generate_waveform_cache_pyramid(
    input_path: &str,
    output_path: &str,
    base_samples_per_peak: u32,
    lod_count: usize,
    cancel_flag: Option<&AtomicBool>,
    progress_per_mille: Option<&AtomicU32>,
) -> Result<AudioPeakPyramid, String> {
    if base_samples_per_peak == 0 {
        return Err("samples_per_peak must be greater than 0".to_string());
    }

    let mut decoder = open_audio_decoder(input_path)
        .map_err(|e| format!("Failed to open audio decoder for '{}': {}", input_path, e))?;

    let channels = decoder.channels() as usize;
    if channels == 0 {
        return Err("Media has no audio channels".to_string());
    }
    let sample_rate = decoder.sample_rate();
    let duration_seconds = decoder.duration_seconds();
    let estimated_total_frames = if duration_seconds > 0.0 && sample_rate > 0 {
        (duration_seconds * sample_rate as f64).round() as u64
    } else {
        0
    };

    // Preallocate a bounded initial capacity if duration is known (~at most 2M bins per channel)
    // to avoid continuous Vec reallocations during long file processing, while guarding against
    // memory exhaustion on corrupt metadata.
    let initial_cap = if estimated_total_frames > 0 && base_samples_per_peak > 0 {
        ((estimated_total_frames / base_samples_per_peak as u64) as usize).min(2 * 1024 * 1024)
    } else {
        1024
    };

    let mut base_channels: Vec<Vec<AudioPeak>> =
        (0..channels).map(|_| Vec::with_capacity(initial_cap)).collect();
    let mut accumulators = vec![WindowAccumulator::new(); channels];
    let mut decoded_frames = 0u64;

    while let Ok(Some(buffer)) = decoder.next_audio_buffer() {
        if let Some(cancel) = cancel_flag {
            if cancel.load(Ordering::Relaxed) {
                return Err("Waveform peak generation cancelled".to_string());
            }
        }

        let buf_channels = (buffer.channels as usize).max(1);
        let frame_count = buffer.frame_count();
        let samples = &buffer.samples_f32;

        for frame in 0..frame_count {
            let base_idx = frame * buf_channels;
            for ch in 0..channels {
                let src_ch = ch.min(buf_channels - 1);
                let sample_idx = base_idx + src_ch;
                if sample_idx < samples.len() {
                    let acc = &mut accumulators[ch];
                    acc.push(samples[sample_idx]);
                    if acc.count == base_samples_per_peak {
                        if let Some(peak) = acc.flush() {
                            base_channels[ch].push(peak);
                        }
                    }
                }
            }
        }

        decoded_frames += frame_count as u64;
        if let Some(prog) = progress_per_mille {
            if estimated_total_frames > 0 {
                let p = ((decoded_frames.saturating_mul(950)) / estimated_total_frames).min(950) as u32;
                prog.store(p, Ordering::Relaxed);
            }
        }
    }

    for ch in 0..channels {
        if let Some(rem) = accumulators[ch].flush() {
            base_channels[ch].push(rem);
        }
    }

    let base_level = AudioPeakLodLevel {
        lod: 0,
        samples_per_peak: base_samples_per_peak,
        channels: base_channels,
    };
    let levels = build_lod_chain_from_base(base_level, lod_count.max(1));

    let pyramid = AudioPeakPyramid {
        channels: channels as u32,
        sample_rate,
        total_frames: decoded_frames,
        levels,
    };

    pyramid.save_to_file(Path::new(output_path))?;

    if let Some(prog) = progress_per_mille {
        prog.store(1000, Ordering::Release);
    }

    Ok(pyramid)
}

/// Extracts audio peaks (`min`, `max`, `rms`) across a multi-level LOD pyramid
/// (starting at `samples_per_peak` for LOD 0) and saves them into `output_path`.
pub fn generate_waveform_cache(
    input_path: &str,
    output_path: &str,
    samples_per_peak: u32,
) -> Result<(), String> {
    generate_waveform_cache_pyramid(
        input_path,
        output_path,
        samples_per_peak,
        DEFAULT_LOD_LEVELS,
        None,
        None,
    )
    .map(|_| ())
}

/// Handle to an asynchronous background waveform peak generation job.
///
/// Runs on an isolated OS worker thread with its own decoder instance so it
/// never blocks playback or demuxing threads.
pub struct WaveformJobHandle {
    cancel_flag: Arc<AtomicBool>,
    progress_per_mille: Arc<AtomicU32>,
    finished_flag: Arc<AtomicBool>,
    join_handle: Option<JoinHandle<Result<AudioPeakPyramid, String>>>,
}

impl WaveformJobHandle {
    /// Signals the background worker to abort at the next packet boundary.
    pub fn cancel(&self) {
        self.cancel_flag.store(true, Ordering::SeqCst);
    }

    /// Returns progress in `[0, 1000]` (where `1000` = 100.0%).
    pub fn progress_per_mille(&self) -> u32 {
        self.progress_per_mille.load(Ordering::Relaxed)
    }

    /// Returns `true` once the background worker has completed (or errored/cancelled).
    pub fn is_finished(&self) -> bool {
        self.finished_flag.load(Ordering::Acquire)
    }

    /// Waits for the background worker to finish and returns the generated [`AudioPeakPyramid`].
    pub fn join(mut self) -> Result<AudioPeakPyramid, String> {
        match self.join_handle.take() {
            Some(handle) => handle
                .join()
                .map_err(|_| "Waveform worker thread panicked".to_string())?,
            None => Err("Waveform worker already joined".to_string()),
        }
    }
}

/// Spawns a non-blocking background worker thread that decodes audio PCM and
/// writes the `.peak` multi-LOD sidecar file atomically.
pub fn spawn_waveform_cache_worker(
    input_path: impl Into<PathBuf>,
    output_path: impl Into<PathBuf>,
    base_samples_per_peak: u32,
    lod_count: usize,
) -> Result<WaveformJobHandle, String> {
    let input = input_path.into();
    let output = output_path.into();
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let progress = Arc::new(AtomicU32::new(0));
    let finished = Arc::new(AtomicBool::new(false));

    let cancel_clone = Arc::clone(&cancel_flag);
    let progress_clone = Arc::clone(&progress);
    let finished_clone = Arc::clone(&finished);

    let handle = thread::Builder::new()
        .name("aether-waveform-worker".to_string())
        .spawn(move || {
            let in_str = input.to_string_lossy().to_string();
            let out_str = output.to_string_lossy().to_string();
            let res = generate_waveform_cache_pyramid(
                &in_str,
                &out_str,
                base_samples_per_peak,
                lod_count,
                Some(&cancel_clone),
                Some(&progress_clone),
            );
            finished_clone.store(true, Ordering::Release);
            res
        })
        .map_err(|e| format!("Failed to spawn waveform worker thread: {}", e))?;

    Ok(WaveformJobHandle {
        cancel_flag,
        progress_per_mille: progress,
        finished_flag: finished,
        join_handle: Some(handle),
    })
}

/// Legacy helper kept for backwards-compatible unit tests; writes a single-LOD
/// `(min, max)` list into a v2 `.peak` file (calculating RMS from `(min, max)`).
pub fn save_peaks_binary(
    path: &str,
    channels: u32,
    sample_rate: u32,
    samples_per_peak: u32,
    peaks: &[Vec<(f32, f32)>],
) -> Result<(), String> {
    let ch_samples: Vec<Vec<AudioPeakSample>> = peaks
        .iter()
        .map(|ch| {
            ch.iter()
                .map(|&(min, max)| {
                    let rms =
                        ((((min as f64).powi(2) + (max as f64).powi(2)) * 0.5).sqrt() as f32)
                            .clamp(0.0, 1.0);
                    AudioPeakSample::new(min, max, rms)
                })
                .collect()
        })
        .collect();

    let bins = ch_samples.first().map(|c| c.len()).unwrap_or(0) as u64;
    let base = AudioPeakLodLevel {
        lod: 0,
        samples_per_peak,
        channels: ch_samples,
    };
    let pyramid = AudioPeakPyramid {
        channels,
        sample_rate,
        total_frames: bins * (samples_per_peak as u64),
        levels: build_lod_chain_from_base(base, DEFAULT_LOD_LEVELS),
    };
    pyramid.save_to_file(Path::new(path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_compute_min_max() {
        let samples = vec![0.1, -0.5, 0.8, -0.2, 0.0];
        let (min, max) = compute_min_max(&samples);
        assert_eq!(min, -0.5);
        assert_eq!(max, 0.8);
    }

    #[test]
    fn test_synthetic_pcm_silence_sine_and_clipping() {
        // 1. Silence buffer: min=0.0, max=0.0, rms=0.0
        let silence = vec![0.0f32; 512];
        let p_silence = compute_window_peak(&silence);
        assert_eq!(p_silence.min, 0.0);
        assert_eq!(p_silence.max, 0.0);
        assert_eq!(p_silence.rms, 0.0);

        // 2. Full-cycle sine wave: min ≈ -1.0, max ≈ +1.0, rms ≈ 1/sqrt(2) ≈ 0.70710678
        let n = 4096usize;
        let sine: Vec<f32> = (0..n)
            .map(|i| (2.0 * std::f32::consts::PI * (i as f32) / (n as f32)).sin())
            .collect();
        let p_sine = compute_window_peak(&sine);
        assert!((p_sine.min - (-1.0)).abs() < 1e-3, "sine min={}", p_sine.min);
        assert!((p_sine.max - 1.0).abs() < 1e-3, "sine max={}", p_sine.max);
        let expected_rms = std::f32::consts::FRAC_1_SQRT_2;
        assert!(
            (p_sine.rms - expected_rms).abs() < 1e-3,
            "sine rms={} expected={}",
            p_sine.rms,
            expected_rms
        );

        // 3. Full-scale clipping & over-0dBFS saturation (+ NaN/Inf resilience)
        let mut clipped = Vec::with_capacity(256);
        for i in 0..128 {
            clipped.push(if i % 2 == 0 { -1.75 } else { 1.75 });
        }
        clipped.push(f32::INFINITY);
        clipped.push(f32::NEG_INFINITY);
        let p_clip = compute_window_peak(&clipped);
        assert_eq!(p_clip.min, -1.0);
        assert_eq!(p_clip.max, 1.0);
        assert!((p_clip.rms - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_pyramid_lod_generation_and_binary_roundtrip() {
        let dir = tempdir().unwrap();
        let peak_path = dir.path().join("pyramid.peak");

        // 16384 stereo frames (256 samples/bin -> 64 bins at LOD0, 16 at LOD1 (1024), 4 at LOD2 (4096), 1 at LOD3 (16384))
        let frames = 16384usize;
        let mut interleaved = Vec::with_capacity(frames * 2);
        for i in 0..frames {
            let phase = 2.0 * std::f32::consts::PI * (i as f32) / 256.0;
            interleaved.push(phase.sin()); // Ch 0: full-scale sine
            interleaved.push(0.5 * phase.cos()); // Ch 1: half-scale cosine
        }

        let pyramid =
            build_peak_pyramid_from_interleaved(&interleaved, 2, 48_000, 256, 5).unwrap();
        assert_eq!(pyramid.channels, 2);
        assert_eq!(pyramid.sample_rate, 48_000);
        assert_eq!(pyramid.total_frames, 16384);
        assert_eq!(pyramid.levels.len(), 5);

        // Verify exact 4x LOD mipmap progression: 256 -> 1024 -> 4096 -> 16384 -> 65536
        assert_eq!(pyramid.levels[0].samples_per_peak, 256);
        assert_eq!(pyramid.levels[1].samples_per_peak, 1024);
        assert_eq!(pyramid.levels[2].samples_per_peak, 4096);
        assert_eq!(pyramid.levels[3].samples_per_peak, 16384);
        assert_eq!(pyramid.levels[4].samples_per_peak, 65536);

        assert_eq!(pyramid.levels[0].bins_per_channel(), 64);
        assert_eq!(pyramid.levels[1].bins_per_channel(), 16);
        assert_eq!(pyramid.levels[2].bins_per_channel(), 4);
        assert_eq!(pyramid.levels[3].bins_per_channel(), 1);
        assert_eq!(pyramid.levels[4].bins_per_channel(), 1);

        pyramid.save_to_file(&peak_path).unwrap();
        let loaded = AudioPeakPyramid::load_from_file(&peak_path).unwrap();
        assert_eq!(loaded, pyramid);

        // Selective single-LOD read from disk
        let lod2 = AudioPeakPyramid::load_lod_from_file(&peak_path, 2).unwrap();
        assert_eq!(lod2, pyramid.levels[2]);
        assert_eq!(lod2.samples_per_peak, 4096);
        assert_eq!(lod2.bins_per_channel(), 4);
    }

    #[test]
    fn test_save_peaks_binary() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.aether_peaks");
        let path_str = file_path.to_str().unwrap();

        let peaks = vec![
            vec![(-0.5, 0.5), (-0.8, 0.9)], // Channel 0
            vec![(-0.4, 0.4), (-0.7, 0.8)], // Channel 1
        ];

        let result = save_peaks_binary(path_str, 2, 44100, 1000, &peaks);
        assert!(result.is_ok());

        let loaded = AudioPeakPyramid::load_from_file(&file_path).unwrap();
        assert_eq!(loaded.channels, 2);
        assert_eq!(loaded.sample_rate, 44100);
        assert_eq!(loaded.levels[0].bins_per_channel(), 2);
    }

    #[test]
    fn test_audio_peaks_generation_and_async_worker() {
        let dir = tempdir().unwrap();

        // 1. Create a synthetic wav file
        let wav_path = dir.path().join("synthetic.wav");
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 44100,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(&wav_path, spec).unwrap();
        for t in 0..44100 {
            let sample =
                ((t as f32 * 440.0 * 2.0 * std::f32::consts::PI / 44100.0).sin() * 16384.0) as i16;
            writer.write_sample(sample).unwrap();
        }
        writer.finalize().unwrap();

        // 2. Run non-blocking background worker
        let output_path = dir.path().join("output.peak");
        let handle = spawn_waveform_cache_worker(&wav_path, &output_path, 256, 4).unwrap();
        let pyramid = handle.join().unwrap();

        assert_eq!(pyramid.channels, 1);
        assert_eq!(pyramid.sample_rate, 44100);
        assert_eq!(pyramid.levels.len(), 4);
        assert!(output_path.exists());
    }
}
