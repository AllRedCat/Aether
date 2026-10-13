//! Tolerant, zero-panic ISO-BMFF (`mp4`, `mov`, `m4v`, `m4a`) container and
//! sample-table (`stbl`) inspector.
//!
//! Unlike the external `mp4` crate (which aborts when encountering QuickTime
//! `mp4a` v1 sound descriptions or vendor extension atoms), this parser bounds
//! every atom to its parent range and extracts the complete video packet table:
//! `DTS`, `PTS`, `file_offset`, `packet_size`, `Is_Keyframe` (`stss`), and
//! `Is_Disposable` (`sdtp`).

use std::collections::HashSet;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use aether_core::timeline::Rational;

use crate::frame_index::{
    FrameIndex, FrameIndexEntry, FLAG_IS_DISPOSABLE, FLAG_IS_KEYFRAME,
};
use crate::MediaError;

/// Maximum number of samples expanded from run-length tables (`stts`, `ctts`, `stsc`)
/// to protect against crafted headers claiming billions of zero-byte samples.
const MAX_SAMPLES_PER_TRACK: usize = 4 * 1024 * 1024;

/// Detailed packet record in the track's native timescale, preserving both
/// `dts` and `pts` to resolve P/B-frame decode dependencies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PacketRecord {
    /// 1-based sample number in decode order.
    pub sample_number: u32,
    /// Decode timestamp in track timescale units.
    pub dts: i64,
    /// Presentation timestamp in track timescale units (`dts + ctts_offset`).
    pub pts: i64,
    /// Sample duration in track timescale units.
    pub duration: u32,
    /// Byte offset of the compressed sample inside the container file.
    pub file_offset: u64,
    /// Compressed sample size in bytes.
    pub packet_size: u32,
    /// Bitfield (`FLAG_IS_KEYFRAME`, `FLAG_IS_DISPOSABLE`).
    pub flags: u8,
}

impl PacketRecord {
    #[inline]
    pub const fn is_keyframe(&self) -> bool {
        (self.flags & FLAG_IS_KEYFRAME) != 0
    }

    #[inline]
    pub const fn is_disposable(&self) -> bool {
        (self.flags & FLAG_IS_DISPOSABLE) != 0
    }

    /// Converts this packet record into a 24-byte [`FrameIndexEntry`] whose
    /// `pts` is expressed in `target_timebase` ticks (or track timescale if
    /// `track_timescale == 0`).
    pub fn to_frame_index_entry(
        &self,
        track_timescale: u32,
        target_timebase: Rational,
    ) -> FrameIndexEntry {
        let converted_pts = if track_timescale > 0
            && target_timebase.num > 0
            && target_timebase.den > 0
        {
            let sec = self.pts as f64 / track_timescale as f64;
            let fps_ratio = target_timebase.num as f64 / target_timebase.den as f64;
            (sec * fps_ratio).round() as i64
        } else {
            self.pts
        };

        FrameIndexEntry {
            pts: converted_pts,
            file_offset: self.file_offset,
            packet_size: self.packet_size,
            flags: self.flags,
            _pad: [0u8; 3],
        }
    }
}

/// Complete inspection result extracted from an ISO-BMFF (`mp4`/`mov`) container.
#[derive(Debug, Clone)]
pub struct IsoBmffAnalysis {
    /// Major brand from `ftyp` (e.g. `"qt  "`, `"isom"`, `"mp42"`).
    pub major_brand: Option<String>,
    /// Container timescale from `mvhd`.
    pub movie_timescale: u32,
    /// Container duration in seconds from `mvhd` (or longest track fallback).
    pub duration_seconds: f64,
    /// Primary video track analysis, if a video track is present.
    pub video: Option<VideoTrackAnalysis>,
    /// Primary audio track analysis, if an audio track is present.
    pub audio: Option<AudioTrackAnalysis>,
}

/// Metadata and packet table for the primary video track.
#[derive(Debug, Clone)]
pub struct VideoTrackAnalysis {
    pub track_id: u32,
    pub timescale: u32,
    pub duration_ticks: u64,
    pub duration_seconds: f64,
    pub coded_width: u32,
    pub coded_height: u32,
    pub display_width: u32,
    pub display_height: u32,
    pub codec_fourcc: String,
    pub codec_name: String,
    pub pixel_format: String,
    pub bit_depth: u8,
    pub fps: Option<f64>,
    pub is_vfr: bool,
    /// Packets in decode order with both `dts` and `pts`.
    pub packets: Vec<PacketRecord>,
    /// Number of sync samples (I/IDR frames).
    pub keyframe_count: usize,
}

impl VideoTrackAnalysis {
    /// Builds a [`FrameIndex`] in `target_timebase` from the extracted packet table.
    pub fn build_frame_index(
        &self,
        target_timebase: Rational,
        source_digest: [u8; 32],
    ) -> FrameIndex {
        let entries: Vec<FrameIndexEntry> = self
            .packets
            .iter()
            .map(|p| p.to_frame_index_entry(self.timescale, target_timebase))
            .collect();
        FrameIndex::new(target_timebase, source_digest, entries)
    }

    /// Builds a [`FrameIndex`] preserving the track's native timescale (`1 / timescale`).
    pub fn build_native_frame_index(&self, source_digest: [u8; 32]) -> FrameIndex {
        let native_tb = Rational {
            num: (self.timescale.min(i32::MAX as u32) as i32).max(1),
            den: 1,
        };
        let entries: Vec<FrameIndexEntry> = self
            .packets
            .iter()
            .map(|p| FrameIndexEntry {
                pts: p.pts,
                file_offset: p.file_offset,
                packet_size: p.packet_size,
                flags: p.flags,
                _pad: [0u8; 3],
            })
            .collect();
        FrameIndex::new(native_tb, source_digest, entries)
    }
}

/// Metadata for the primary audio track in an ISO-BMFF container.
#[derive(Debug, Clone)]
pub struct AudioTrackAnalysis {
    pub track_id: u32,
    pub timescale: u32,
    pub duration_seconds: f64,
    pub codec_fourcc: String,
    pub codec_name: String,
    pub channels: u16,
    pub sample_rate: u32,
    pub bit_depth: Option<u8>,
}

// ---------------------------------------------------------------------------
// Top-level parser entry point
// ---------------------------------------------------------------------------

/// Parses an ISO-BMFF (`mp4`, `mov`, `m4v`, `m4a`) file from disk, extracting
/// stream metadata and the full video packet table without decoding pixel data.
pub fn parse_isobmff_file(path: &Path) -> Result<IsoBmffAnalysis, MediaError> {
    let mut file = File::open(path).map_err(|e| MediaError::IoError(e.to_string()))?;
    let file_size = file
        .metadata()
        .map_err(|e| MediaError::IoError(e.to_string()))?
        .len();
    if file_size < 8 {
        return Err(MediaError::CorruptFile(
            "ISO-BMFF container smaller than 8-byte box header".to_string(),
        ));
    }

    let mut major_brand: Option<String> = None;
    let mut moov_payload: Option<Vec<u8>> = None;

    let mut pos = 0u64;
    while pos + 8 <= file_size {
        file.seek(SeekFrom::Start(pos))
            .map_err(|e| MediaError::IoError(e.to_string()))?;
        let mut hdr = [0u8; 16];
        let n = file
            .read(&mut hdr[..8])
            .map_err(|e| MediaError::IoError(e.to_string()))?;
        if n < 8 {
            break;
        }
        let size32 = u32::from_be_bytes([hdr[0], hdr[1], hdr[2], hdr[3]]);
        let fourcc = [hdr[4], hdr[5], hdr[6], hdr[7]];

        let (header_len, box_size) = if size32 == 1 {
            if pos + 16 > file_size {
                return Err(MediaError::CorruptFile(
                    "Truncated 64-bit ISO-BMFF atom header".to_string(),
                ));
            }
            file.read_exact(&mut hdr[8..16])
                .map_err(|e| MediaError::IoError(e.to_string()))?;
            let size64 = u64::from_be_bytes(hdr[8..16].try_into().unwrap());
            if size64 < 16 {
                return Err(MediaError::CorruptFile(
                    "Invalid 64-bit atom size (< 16)".to_string(),
                ));
            }
            (16u64, size64)
        } else if size32 == 0 {
            (8u64, file_size - pos)
        } else if (size32 as u64) < 8 {
            return Err(MediaError::CorruptFile(format!(
                "Invalid atom size {} at offset {}",
                size32, pos
            )));
        } else {
            (8u64, size32 as u64)
        };

        let box_end = pos.checked_add(box_size).ok_or_else(|| {
            MediaError::CorruptFile(format!("ISO-BMFF atom size overflow at offset {}", pos))
        })?;
        if box_end > file_size {
            return Err(MediaError::CorruptFile(format!(
                "Truncated ISO-BMFF atom '{}' at offset {} (declared end {}, file size {})",
                String::from_utf8_lossy(&fourcc),
                pos,
                box_end,
                file_size
            )));
        }
        let payload_start = pos + header_len;
        let payload_len = box_end.saturating_sub(payload_start);

        if &fourcc == b"ftyp" && payload_len >= 4 {
            let mut brand = [0u8; 4];
            if file.read_exact(&mut brand).is_ok() {
                major_brand = Some(String::from_utf8_lossy(&brand).to_string());
            }
        } else if &fourcc == b"moov" {
            // Cap `moov` allocation to 256 MiB for memory safety.
            if payload_len > 256 * 1024 * 1024 {
                return Err(MediaError::CorruptFile(
                    "moov atom exceeds 256 MiB safety limit".to_string(),
                ));
            }
            let mut buf = vec![0u8; payload_len as usize];
            file.seek(SeekFrom::Start(payload_start))
                .map_err(|e| MediaError::IoError(e.to_string()))?;
            file.read_exact(&mut buf)
                .map_err(|e| MediaError::CorruptFile(format!("Truncated moov atom: {}", e)))?;
            moov_payload = Some(buf);
        }

        if box_end <= pos {
            break;
        }
        pos = box_end;
    }

    let moov = moov_payload.ok_or_else(|| {
        MediaError::CorruptFile("Missing 'moov' atom in MP4/MOV container".to_string())
    })?;

    parse_moov(&moov, major_brand, file_size)
}

// ---------------------------------------------------------------------------
// In-memory box walker over `moov`
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct BoxRef<'a> {
    fourcc: [u8; 4],
    payload: &'a [u8],
}

struct BoxIter<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> BoxIter<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }
}

impl<'a> Iterator for BoxIter<'a> {
    type Item = BoxRef<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos + 8 > self.data.len() {
            return None;
        }
        let s = &self.data[self.pos..];
        let size32 = u32::from_be_bytes([s[0], s[1], s[2], s[3]]) as usize;
        let fourcc = [s[4], s[5], s[6], s[7]];

        let (hdr, total) = if size32 == 1 {
            if s.len() < 16 {
                self.pos = self.data.len();
                return None;
            }
            let size64 = u64::from_be_bytes(s[8..16].try_into().unwrap()) as usize;
            if size64 < 16 {
                self.pos = self.data.len();
                return None;
            }
            (16usize, size64.min(s.len()))
        } else if size32 == 0 {
            (8usize, s.len())
        } else if size32 < 8 {
            // Malformed child box: stop iterating this level cleanly without panicking.
            self.pos = self.data.len();
            return None;
        } else {
            // Clamp oversized child box to parent boundary so vendor quirks don't panic.
            (8usize, size32.min(s.len()))
        };

        let payload = &s[hdr..total];
        self.pos += total;
        Some(BoxRef { fourcc, payload })
    }
}

fn parse_moov(
    moov: &[u8],
    major_brand: Option<String>,
    file_size: u64,
) -> Result<IsoBmffAnalysis, MediaError> {
    let mut movie_timescale: Option<u32> = None;
    let mut movie_duration_ticks: u64 = 0;
    let mut video: Option<VideoTrackAnalysis> = None;
    let mut audio: Option<AudioTrackAnalysis> = None;

    for b in BoxIter::new(moov) {
        match &b.fourcc {
            b"mvhd" => {
                let (ts, dur) = parse_mvhd(b.payload)?;
                if ts == 0 {
                    return Err(MediaError::CorruptFile(
                        "MP4/MOV mvhd timescale is zero".to_string(),
                    ));
                }
                movie_timescale = Some(ts);
                movie_duration_ticks = dur;
            }
            b"trak" => match parse_trak(b.payload, file_size) {
                Ok(ParsedTrack::Video(v)) => {
                    if video.is_none() {
                        video = Some(v);
                    }
                }
                Ok(ParsedTrack::Audio(a)) => {
                    if audio.is_none() {
                        audio = Some(a);
                    }
                }
                Ok(ParsedTrack::Other) => {}
                Err(e) => {
                    // Propagate fatal timescale=0 corruption; ignore non-AV auxiliary track quirks.
                    if matches!(&e, MediaError::CorruptFile(msg) if msg.contains("timescale is zero"))
                    {
                        return Err(e);
                    }
                }
            },
            _ => {}
        }
    }

    let movie_ts = movie_timescale.ok_or_else(|| {
        MediaError::CorruptFile("Missing 'mvhd' box inside 'moov'".to_string())
    })?;

    let mut duration_seconds = if movie_ts > 0 {
        movie_duration_ticks as f64 / movie_ts as f64
    } else {
        0.0
    };
    if let Some(ref v) = video {
        if v.duration_seconds > duration_seconds {
            duration_seconds = v.duration_seconds;
        }
    }
    if let Some(ref a) = audio {
        if a.duration_seconds > duration_seconds {
            duration_seconds = a.duration_seconds;
        }
    }

    Ok(IsoBmffAnalysis {
        major_brand,
        movie_timescale: movie_ts,
        duration_seconds,
        video,
        audio,
    })
}

fn parse_mvhd(d: &[u8]) -> Result<(u32, u64), MediaError> {
    if d.len() < 20 {
        return Err(MediaError::CorruptFile("Truncated mvhd box".to_string()));
    }
    let version = d[0];
    if version == 1 {
        if d.len() < 32 {
            return Err(MediaError::CorruptFile("Truncated v1 mvhd box".to_string()));
        }
        let ts = u32::from_be_bytes(d[20..24].try_into().unwrap());
        let dur = u64::from_be_bytes(d[24..32].try_into().unwrap());
        Ok((ts, dur))
    } else {
        let ts = u32::from_be_bytes(d[12..16].try_into().unwrap());
        let dur = u32::from_be_bytes(d[16..20].try_into().unwrap()) as u64;
        Ok((ts, dur))
    }
}

enum ParsedTrack {
    Video(VideoTrackAnalysis),
    Audio(AudioTrackAnalysis),
    Other,
}

fn parse_trak(trak: &[u8], file_size: u64) -> Result<ParsedTrack, MediaError> {
    let mut track_id = 1u32;
    let mut tkhd_width = 0u32;
    let mut tkhd_height = 0u32;
    let mut mdia_slice: Option<&[u8]> = None;

    for b in BoxIter::new(trak) {
        match &b.fourcc {
            b"tkhd" => {
                if let Some((id, w, h)) = parse_tkhd(b.payload) {
                    track_id = id;
                    tkhd_width = w;
                    tkhd_height = h;
                }
            }
            b"mdia" => mdia_slice = Some(b.payload),
            _ => {}
        }
    }

    let Some(mdia) = mdia_slice else {
        return Ok(ParsedTrack::Other);
    };

    let mut mdhd_timescale: Option<u32> = None;
    let mut mdhd_duration: u64 = 0;
    let mut handler_type = [0u8; 4];
    let mut minf_slice: Option<&[u8]> = None;

    for b in BoxIter::new(mdia) {
        match &b.fourcc {
            b"mdhd" => {
                let (ts, dur) = parse_mdhd(b.payload)?;
                if ts == 0 {
                    return Err(MediaError::CorruptFile(
                        "Track mdhd timescale is zero".to_string(),
                    ));
                }
                mdhd_timescale = Some(ts);
                mdhd_duration = dur;
            }
            b"hdlr" => {
                if b.payload.len() >= 12 {
                    handler_type.copy_from_slice(&b.payload[8..12]);
                }
            }
            b"minf" => minf_slice = Some(b.payload),
            _ => {}
        }
    }

    let timescale = mdhd_timescale.unwrap_or(0);
    if timescale == 0 {
        return Err(MediaError::CorruptFile(
            "Track timescale is zero".to_string(),
        ));
    }

    let Some(minf) = minf_slice else {
        return Ok(ParsedTrack::Other);
    };
    let stbl = BoxIter::new(minf)
        .find(|b| &b.fourcc == b"stbl")
        .map(|b| b.payload);
    let Some(stbl) = stbl else {
        return Ok(ParsedTrack::Other);
    };

    match &handler_type {
        b"vide" => {
            let v = parse_video_stbl(
                stbl,
                track_id,
                timescale,
                mdhd_duration,
                tkhd_width,
                tkhd_height,
                file_size,
            )?;
            Ok(ParsedTrack::Video(v))
        }
        b"soun" => {
            let a = parse_audio_stbl(stbl, track_id, timescale, mdhd_duration)?;
            Ok(ParsedTrack::Audio(a))
        }
        _ => Ok(ParsedTrack::Other),
    }
}

fn parse_tkhd(d: &[u8]) -> Option<(u32, u32, u32)> {
    if d.is_empty() {
        return None;
    }
    let version = d[0];
    if version == 1 {
        if d.len() < 92 {
            return None;
        }
        let id = u32::from_be_bytes(d[20..24].try_into().ok()?);
        let w = (u32::from_be_bytes(d[84..88].try_into().ok()?) >> 16) as u32;
        let h = (u32::from_be_bytes(d[88..92].try_into().ok()?) >> 16) as u32;
        Some((id, w, h))
    } else {
        if d.len() < 84 {
            return None;
        }
        let id = u32::from_be_bytes(d[12..16].try_into().ok()?);
        let w = (u32::from_be_bytes(d[76..80].try_into().ok()?) >> 16) as u32;
        let h = (u32::from_be_bytes(d[80..84].try_into().ok()?) >> 16) as u32;
        Some((id, w, h))
    }
}

fn parse_mdhd(d: &[u8]) -> Result<(u32, u64), MediaError> {
    if d.len() < 20 {
        return Err(MediaError::CorruptFile("Truncated mdhd box".to_string()));
    }
    let version = d[0];
    if version == 1 {
        if d.len() < 32 {
            return Err(MediaError::CorruptFile("Truncated v1 mdhd box".to_string()));
        }
        let ts = u32::from_be_bytes(d[20..24].try_into().unwrap());
        let dur = u64::from_be_bytes(d[24..32].try_into().unwrap());
        Ok((ts, dur))
    } else {
        let ts = u32::from_be_bytes(d[12..16].try_into().unwrap());
        let dur = u32::from_be_bytes(d[16..20].try_into().unwrap()) as u64;
        Ok((ts, dur))
    }
}

// ---------------------------------------------------------------------------
// Video & Audio `stbl` parsing
// ---------------------------------------------------------------------------

#[derive(Default)]
struct SttsRun {
    count: u32,
    delta: u32,
}

#[derive(Default)]
struct CttsRun {
    count: u32,
    offset: i64,
}

#[derive(Default, Clone, Copy)]
struct StscEntry {
    first_chunk: u32,
    samples_per_chunk: u32,
}

fn parse_video_stbl(
    stbl: &[u8],
    track_id: u32,
    timescale: u32,
    mdhd_duration: u64,
    tkhd_width: u32,
    tkhd_height: u32,
    file_size: u64,
) -> Result<VideoTrackAnalysis, MediaError> {
    let mut coded_width = 0u32;
    let mut coded_height = 0u32;
    let mut codec_fourcc = String::from("unkn");
    let mut bit_depth = 8u8;

    let mut stts_runs: Vec<SttsRun> = Vec::new();
    let mut ctts_runs: Vec<CttsRun> = Vec::new();
    let mut sync_samples: Option<HashSet<u32>> = None;
    let mut sdtp_flags: Vec<u8> = Vec::new();
    let mut sample_sizes: Vec<u32> = Vec::new();
    let mut uniform_sample_size: Option<(u32, usize)> = None;
    let mut stsc_entries: Vec<StscEntry> = Vec::new();
    let mut chunk_offsets: Vec<u64> = Vec::new();

    for b in BoxIter::new(stbl) {
        match &b.fourcc {
            b"stsd" => {
                if let Some((fourcc, w, h, depth)) = parse_video_stsd(b.payload) {
                    codec_fourcc = fourcc;
                    coded_width = w;
                    coded_height = h;
                    bit_depth = depth;
                }
            }
            b"stts" => stts_runs = parse_stts(b.payload),
            b"ctts" => ctts_runs = parse_ctts(b.payload),
            b"stss" => sync_samples = Some(parse_stss(b.payload)),
            b"sdtp" => {
                if b.payload.len() > 4 {
                    sdtp_flags = b.payload[4..].to_vec();
                }
            }
            b"stsz" => {
                let (uniform, sizes) = parse_stsz(b.payload);
                uniform_sample_size = uniform;
                sample_sizes = sizes;
            }
            b"stz2" => {
                sample_sizes = parse_stz2(b.payload);
            }
            b"stsc" => stsc_entries = parse_stsc(b.payload),
            b"stco" => chunk_offsets = parse_stco(b.payload),
            b"co64" => chunk_offsets = parse_co64(b.payload),
            _ => {}
        }
    }

    // Determine total sample count from stsz (or stts sum fallback).
    let stts_total: usize = stts_runs
        .iter()
        .fold(0usize, |acc, r| acc.saturating_add(r.count as usize))
        .min(MAX_SAMPLES_PER_TRACK);

    let sample_count = if let Some((_, count)) = uniform_sample_size {
        count.min(MAX_SAMPLES_PER_TRACK)
    } else if !sample_sizes.is_empty() {
        sample_sizes.len().min(MAX_SAMPLES_PER_TRACK)
    } else {
        stts_total
    };

    // Expand per-sample DTS and duration from `stts`, and detect VFR.
    let mut dts_list: Vec<(i64, u32)> = Vec::with_capacity(sample_count);
    let mut cur_dts = 0i64;
    let mut first_delta: Option<u32> = None;
    let mut is_vfr = false;

    for (run_idx, run) in stts_runs.iter().enumerate() {
        let is_last_run = run_idx + 1 == stts_runs.len();
        if run.count > 0 {
            match first_delta {
                None => first_delta = Some(run.delta),
                Some(fd) => {
                    // Ignore a trailing 1-sample run that only trims the final frame duration.
                    let is_single_tail = is_last_run && run.count == 1;
                    if run.delta != fd && !is_single_tail {
                        is_vfr = true;
                    }
                }
            }
        }
        for _ in 0..run.count {
            if dts_list.len() >= sample_count {
                break;
            }
            dts_list.push((cur_dts, run.delta));
            cur_dts = cur_dts.saturating_add(run.delta as i64);
        }
    }
    while dts_list.len() < sample_count {
        let delta = first_delta.unwrap_or(1);
        dts_list.push((cur_dts, delta));
        cur_dts = cur_dts.saturating_add(delta as i64);
    }

    // Expand composition offsets (`ctts`) to compute PTS = DTS + offset.
    let mut ctts_offsets: Vec<i64> = Vec::with_capacity(if ctts_runs.is_empty() {
        0
    } else {
        sample_count
    });
    for run in &ctts_runs {
        for _ in 0..run.count {
            if ctts_offsets.len() >= sample_count {
                break;
            }
            ctts_offsets.push(run.offset);
        }
    }

    // Helper closure to get size of 0-based sample `idx`.
    let size_of = |idx: usize| -> u32 {
        if let Some((u, _)) = uniform_sample_size {
            u
        } else {
            sample_sizes.get(idx).copied().unwrap_or(0)
        }
    };

    // Expand byte offsets for each sample from `stsc` + `stco`/`co64` + sample sizes.
    let mut file_offsets: Vec<u64> = Vec::with_capacity(sample_count);
    if !chunk_offsets.is_empty() && !stsc_entries.is_empty() {
        let num_chunks = chunk_offsets.len();
        let mut sample_idx = 0usize;

        for (stsc_i, entry) in stsc_entries.iter().enumerate() {
            let start_chunk_1 = entry.first_chunk.max(1) as usize;
            let end_chunk_1 = stsc_entries
                .get(stsc_i + 1)
                .map(|next| (next.first_chunk as usize).saturating_sub(1))
                .unwrap_or(num_chunks)
                .min(num_chunks);

            if start_chunk_1 > end_chunk_1 {
                continue;
            }

            for chunk_1 in start_chunk_1..=end_chunk_1 {
                let mut byte_cursor = chunk_offsets[chunk_1 - 1];
                for _ in 0..entry.samples_per_chunk {
                    if sample_idx >= sample_count {
                        break;
                    }
                    file_offsets.push(byte_cursor.min(file_size));
                    let sz = size_of(sample_idx) as u64;
                    byte_cursor = byte_cursor.saturating_add(sz);
                    sample_idx += 1;
                }
                if sample_idx >= sample_count {
                    break;
                }
            }
            if sample_idx >= sample_count {
                break;
            }
        }
    }
    while file_offsets.len() < sample_count {
        file_offsets.push(0);
    }

    // Build `PacketRecord` table in decode order.
    let mut packets = Vec::with_capacity(sample_count);
    let mut keyframe_count = 0usize;
    let mut max_pts_seen = i64::MIN;
    let has_ctts = !ctts_offsets.is_empty();

    for i in 0..sample_count {
        let sample_number = (i + 1) as u32;
        let (dts, duration) = dts_list[i];
        let ctts_off = ctts_offsets.get(i).copied().unwrap_or(0);
        let pts = dts.saturating_add(ctts_off).max(0);
        let packet_size = size_of(i);
        let file_offset = file_offsets[i];

        // If `stss` is present, only samples listed in `stss` are keyframes;
        // if `stss` is absent, all samples in the track are sync samples (intra-only).
        let is_keyframe = match &sync_samples {
            Some(set) => set.contains(&sample_number),
            None => true,
        };
        if is_keyframe {
            keyframe_count += 1;
        }

        // Disposable flag from `sdtp` (`sample_is_depended_on` in bits 2..3 == 2),
        // or reordered B-frame heuristic when `ctts` reorders frames behind `max_pts_seen`.
        let is_disposable = if let Some(&sdtp_byte) = sdtp_flags.get(i) {
            let depended_on = (sdtp_byte >> 2) & 0b11;
            depended_on == 2
        } else {
            !is_keyframe && has_ctts && pts < max_pts_seen
        };

        if pts > max_pts_seen {
            max_pts_seen = pts;
        }

        let mut flags = 0u8;
        if is_keyframe {
            flags |= FLAG_IS_KEYFRAME;
        }
        if is_disposable {
            flags |= FLAG_IS_DISPOSABLE;
        }

        packets.push(PacketRecord {
            sample_number,
            dts,
            pts,
            duration,
            file_offset,
            packet_size,
            flags,
        });
    }

    let total_ticks = if mdhd_duration > 0 {
        mdhd_duration
    } else {
        cur_dts.max(0) as u64
    };
    let duration_seconds = total_ticks as f64 / timescale as f64;

    let fps = if duration_seconds > 0.0 && !packets.is_empty() {
        let rate = packets.len() as f64 / duration_seconds;
        if rate.is_finite() && rate > 0.0 {
            Some(rate)
        } else {
            None
        }
    } else {
        first_delta.and_then(|d| {
            if d > 0 {
                Some(timescale as f64 / d as f64)
            } else {
                None
            }
        })
    };

    let display_width = if tkhd_width > 0 {
        tkhd_width
    } else {
        coded_width
    };
    let display_height = if tkhd_height > 0 {
        tkhd_height
    } else {
        coded_height
    };
    let (codec_name, pixel_format) = classify_video_codec(&codec_fourcc, bit_depth);

    Ok(VideoTrackAnalysis {
        track_id,
        timescale,
        duration_ticks: total_ticks,
        duration_seconds,
        coded_width: if coded_width > 0 {
            coded_width
        } else {
            display_width
        },
        coded_height: if coded_height > 0 {
            coded_height
        } else {
            display_height
        },
        display_width,
        display_height,
        codec_fourcc,
        codec_name,
        pixel_format,
        bit_depth,
        fps,
        is_vfr,
        packets,
        keyframe_count,
    })
}

fn parse_video_stsd(d: &[u8]) -> Option<(String, u32, u32, u8)> {
    // FullBox(4) + entry_count(4) + first VisualSampleEntry:
    // [0..4] size, [4..8] fourcc, [8..14] reserved, [14..16] data_ref_idx,
    // [16..32] pre_defined/reserved, [32..34] width, [34..36] height, ..., [82..84] depth
    if d.len() < 8 + 86 {
        return None;
    }
    let entry = &d[8..];
    let fourcc = String::from_utf8_lossy(&entry[4..8]).trim().to_string();
    let width = u16::from_be_bytes([entry[32], entry[33]]) as u32;
    let height = u16::from_be_bytes([entry[34], entry[35]]) as u32;
    let raw_depth = u16::from_be_bytes([entry[82], entry[83]]);
    let bit_depth = match raw_depth {
        10 | 30 => 10,
        12 | 36 => 12,
        _ => 8,
    };
    Some((fourcc, width, height, bit_depth))
}

fn classify_video_codec(fourcc: &str, bit_depth: u8) -> (String, String) {
    let codec = match fourcc {
        "avc1" | "avc3" => "H.264",
        "hvc1" | "hev1" => "HEVC",
        "vp09" => "VP9",
        "av01" => "AV1",
        "apch" | "apcn" | "apcs" | "apco" | "ap4h" | "ap4x" => "ProRes",
        "mp4v" => "MPEG-4",
        "jpeg" | "mjpa" | "mjpb" => "MJPEG",
        other if !other.is_empty() => other,
        _ => "Unknown",
    };
    let pix_fmt = if codec == "ProRes" {
        if fourcc == "ap4h" || fourcc == "ap4x" {
            "yuva444p10le"
        } else {
            "yuv422p10le"
        }
    } else if bit_depth == 10 {
        "yuv420p10le"
    } else {
        "yuv420p"
    };
    (codec.to_string(), pix_fmt.to_string())
}

fn parse_audio_stbl(
    stbl: &[u8],
    track_id: u32,
    timescale: u32,
    mdhd_duration: u64,
) -> Result<AudioTrackAnalysis, MediaError> {
    let mut codec_fourcc = String::from("mp4a");
    let mut channels = 2u16;
    let mut sample_rate = timescale;
    let mut bit_depth = Some(16u8);

    for b in BoxIter::new(stbl) {
        if &b.fourcc == b"stsd" && b.payload.len() >= 8 + 36 {
            let entry = &b.payload[8..];
            codec_fourcc = String::from_utf8_lossy(&entry[4..8]).trim().to_string();
            let ch = u16::from_be_bytes([entry[24], entry[25]]);
            if ch > 0 {
                channels = ch;
            }
            let depth = u16::from_be_bytes([entry[26], entry[27]]);
            if depth > 0 && depth <= 64 {
                bit_depth = Some(depth as u8);
            }
            let sr = u32::from_be_bytes([entry[32], entry[33], entry[34], entry[35]]) >> 16;
            if sr > 0 {
                sample_rate = sr;
            }
        }
    }

    let codec_name = match codec_fourcc.as_str() {
        "mp4a" => "AAC",
        "alac" => "ALAC",
        "Opus" | "opus" => "Opus",
        "fLaC" => "FLAC",
        "lpcm" | "sowt" | "twos" | "in24" | "in32" | "fl32" | "fl64" => "PCM",
        "ac-3" => "AC-3",
        "ec-3" => "E-AC-3",
        other => other,
    }
    .to_string();

    let duration_seconds = if timescale > 0 {
        mdhd_duration as f64 / timescale as f64
    } else {
        0.0
    };

    Ok(AudioTrackAnalysis {
        track_id,
        timescale,
        duration_seconds,
        codec_fourcc,
        codec_name,
        channels,
        sample_rate,
        bit_depth,
    })
}

fn parse_stts(d: &[u8]) -> Vec<SttsRun> {
    if d.len() < 8 {
        return Vec::new();
    }
    let count = u32::from_be_bytes(d[4..8].try_into().unwrap()) as usize;
    let avail = (d.len() - 8) / 8;
    let n = count.min(avail);
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let off = 8 + i * 8;
        let sample_count = u32::from_be_bytes(d[off..off + 4].try_into().unwrap());
        let sample_delta = u32::from_be_bytes(d[off + 4..off + 8].try_into().unwrap());
        out.push(SttsRun {
            count: sample_count,
            delta: sample_delta,
        });
    }
    out
}

fn parse_ctts(d: &[u8]) -> Vec<CttsRun> {
    if d.len() < 8 {
        return Vec::new();
    }
    let version = d[0];
    let count = u32::from_be_bytes(d[4..8].try_into().unwrap()) as usize;
    let avail = (d.len() - 8) / 8;
    let n = count.min(avail);
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let off = 8 + i * 8;
        let sample_count = u32::from_be_bytes(d[off..off + 4].try_into().unwrap());
        let raw = d[off + 4..off + 8].try_into().unwrap();
        let offset = if version == 1 {
            i32::from_be_bytes(raw) as i64
        } else {
            u32::from_be_bytes(raw) as i64
        };
        out.push(CttsRun {
            count: sample_count,
            offset,
        });
    }
    out
}

fn parse_stss(d: &[u8]) -> HashSet<u32> {
    if d.len() < 8 {
        return HashSet::new();
    }
    let count = u32::from_be_bytes(d[4..8].try_into().unwrap()) as usize;
    let avail = (d.len() - 8) / 4;
    let n = count.min(avail);
    let mut out = HashSet::with_capacity(n);
    for i in 0..n {
        let off = 8 + i * 4;
        out.insert(u32::from_be_bytes(d[off..off + 4].try_into().unwrap()));
    }
    out
}

fn parse_stsz(d: &[u8]) -> (Option<(u32, usize)>, Vec<u32>) {
    if d.len() < 12 {
        return (None, Vec::new());
    }
    let sample_size = u32::from_be_bytes(d[4..8].try_into().unwrap());
    let sample_count = (u32::from_be_bytes(d[8..12].try_into().unwrap()) as usize)
        .min(MAX_SAMPLES_PER_TRACK);
    if sample_size != 0 {
        return (Some((sample_size, sample_count)), Vec::new());
    }
    let avail = (d.len() - 12) / 4;
    let n = sample_count.min(avail);
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let off = 12 + i * 4;
        out.push(u32::from_be_bytes(d[off..off + 4].try_into().unwrap()));
    }
    (None, out)
}

fn parse_stz2(d: &[u8]) -> Vec<u32> {
    if d.len() < 12 {
        return Vec::new();
    }
    let field_size = d[7];
    let sample_count = (u32::from_be_bytes(d[8..12].try_into().unwrap()) as usize)
        .min(MAX_SAMPLES_PER_TRACK);
    let payload = &d[12..];
    let mut out = Vec::new();
    match field_size {
        8 => {
            for &b in payload.iter().take(sample_count) {
                out.push(b as u32);
            }
        }
        16 => {
            for chunk in payload.chunks_exact(2).take(sample_count) {
                out.push(u16::from_be_bytes([chunk[0], chunk[1]]) as u32);
            }
        }
        _ => {}
    }
    out
}

fn parse_stsc(d: &[u8]) -> Vec<StscEntry> {
    if d.len() < 8 {
        return Vec::new();
    }
    let count = u32::from_be_bytes(d[4..8].try_into().unwrap()) as usize;
    let avail = (d.len() - 8) / 12;
    let n = count.min(avail);
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let off = 8 + i * 12;
        let first_chunk = u32::from_be_bytes(d[off..off + 4].try_into().unwrap());
        let samples_per_chunk = u32::from_be_bytes(d[off + 4..off + 8].try_into().unwrap());
        out.push(StscEntry {
            first_chunk,
            samples_per_chunk,
        });
    }
    out
}

fn parse_stco(d: &[u8]) -> Vec<u64> {
    if d.len() < 8 {
        return Vec::new();
    }
    let count = u32::from_be_bytes(d[4..8].try_into().unwrap()) as usize;
    let avail = (d.len() - 8) / 4;
    let n = count.min(avail);
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let off = 8 + i * 4;
        out.push(u32::from_be_bytes(d[off..off + 4].try_into().unwrap()) as u64);
    }
    out
}

fn parse_co64(d: &[u8]) -> Vec<u64> {
    if d.len() < 8 {
        return Vec::new();
    }
    let count = u32::from_be_bytes(d[4..8].try_into().unwrap()) as usize;
    let avail = (d.len() - 8) / 8;
    let n = count.min(avail);
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let off = 8 + i * 8;
        out.push(u64::from_be_bytes(d[off..off + 8].try_into().unwrap()));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn parses_real_mp4_fixture_and_builds_searchable_frame_index() {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("sample_64x64_10frames.mp4");
        let analysis = parse_isobmff_file(&fixture).expect("should parse MP4 fixture");
        let video = analysis.video.as_ref().expect("should have video track");

        assert_eq!(video.coded_width, 64);
        assert_eq!(video.coded_height, 64);
        assert_eq!(video.packets.len(), 10);
        assert!(video.keyframe_count >= 1);

        // Every packet must have a valid positive size and in-bounds file offset
        for pkt in &video.packets {
            assert!(pkt.packet_size > 0);
            assert!(pkt.file_offset > 0);
        }

        let idx = video.build_frame_index(Rational { num: 60, den: 1 }, [0xAB; 32]);
        assert_eq!(idx.len(), 10);
        let kf = idx.seek_keyframe(30).expect("should find keyframe");
        assert!(kf.is_keyframe());
        assert!(kf.pts <= 30);
    }
}
