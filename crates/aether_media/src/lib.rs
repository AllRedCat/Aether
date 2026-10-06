use std::fs::File;
use std::io::BufReader;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;

use aether_core::media::{MediaItem, MediaMetadata, MediaType};
use aether_core::timeline::Rational;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const DEFAULT_TIMEBASE: Rational = Rational { num: 60, den: 1 };
pub const DEFAULT_IMAGE_DURATION_SECONDS: f64 = 5.0;
pub const DEFAULT_IMAGE_DURATION_PTS: i64 = 300; // 5.0s @ 60fps

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MediaError {
    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Path is not a regular file: {0}")]
    NotAFile(String),

    #[error("File is empty (0 bytes): {0}")]
    EmptyFile(String),

    #[error("Unsupported media format: {0}")]
    UnsupportedFormat(String),

    #[error("Corrupt file or invalid header: {0}")]
    CorruptFile(String),

    #[error("I/O error: {0}")]
    IoError(String),
}

impl From<std::io::Error> for MediaError {
    fn from(err: std::io::Error) -> Self {
        match err.kind() {
            std::io::ErrorKind::NotFound => MediaError::FileNotFound(err.to_string()),
            _ => MediaError::IoError(err.to_string()),
        }
    }
}

/// Inspection output from analyzing a media file with pure-Rust decoders.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MediaInspection {
    pub media_type: MediaType,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub duration_seconds: f64,
    pub duration_pts: i64,
    pub timebase: Option<Rational>,
    pub fps: Option<f64>,
    pub audio_channels: Option<u16>,
    pub sample_rate: Option<u32>,
    pub file_size_bytes: u64,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub pixel_format: Option<String>,
    pub is_vfr: Option<bool>,
    pub keyframe_pts: Option<Vec<i64>>,
}

impl MediaInspection {
    pub fn to_metadata(&self) -> MediaMetadata {
        MediaMetadata {
            width: self.width,
            height: self.height,
            duration_pts: self.duration_pts,
            duration_seconds: self.duration_seconds,
            timebase: self.timebase,
            audio_channels: self.audio_channels,
            sample_rate: self.sample_rate,
            file_size_bytes: self.file_size_bytes,
            video_codec: self.video_codec.clone(),
            audio_codec: self.audio_codec.clone(),
            pixel_format: self.pixel_format.clone(),
            is_vfr: self.is_vfr,
            keyframe_pts: self.keyframe_pts.clone(),
        }
    }

    pub fn to_media_item(&self, file_path: &str) -> MediaItem {
        MediaItem::new(
            file_path.to_string(),
            self.media_type,
            self.to_metadata(),
        )
    }
}

impl From<MediaInspection> for MediaMetadata {
    fn from(inspection: MediaInspection) -> Self {
        inspection.to_metadata()
    }
}

/// Initialize media engine (called by bridge during init_engine).
pub fn init_media() {
    println!("Init native media decoders");
}

/// Inspect a media file using default 60 FPS timebase.
pub fn inspect_media_file(path: &str) -> Result<MediaInspection, MediaError> {
    inspect_media_file_with_timebase(path, DEFAULT_TIMEBASE)
}

/// Inspect a media file with a project-specific timebase.
pub fn inspect_media_file_with_timebase(
    path_str: &str,
    timebase: Rational,
) -> Result<MediaInspection, MediaError> {
    let path = Path::new(path_str);

    // 1. Check existence
    if !path.exists() {
        return Err(MediaError::FileNotFound(path_str.to_string()));
    }

    // 2. Check if it is a regular file
    if !path.is_file() {
        return Err(MediaError::NotAFile(path_str.to_string()));
    }

    // 3. Check file size
    let meta = std::fs::metadata(path).map_err(|e| MediaError::IoError(e.to_string()))?;
    let file_size_bytes = meta.len();
    if file_size_bytes == 0 {
        return Err(MediaError::EmptyFile(path_str.to_string()));
    }

    // 4. Sniff media type via magic numbers with extension fallback
    let detected_type = detect_media_type(path)?;

    // 5. Inspect metadata based on format with panic barrier protection
    catch_unwind(AssertUnwindSafe(|| match detected_type {
        MediaType::Image => inspect_image(path, file_size_bytes, timebase),
        MediaType::Audio => inspect_audio(path, file_size_bytes, timebase),
        MediaType::Video => inspect_video(path, file_size_bytes, timebase),
    }))
    .unwrap_or_else(|payload| {
        let panic_msg = if let Some(s) = payload.downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = payload.downcast_ref::<String>() {
            s.clone()
        } else {
            "Decoder panicked while parsing media file".to_string()
        };
        Err(MediaError::CorruptFile(format!("Decoder panicked: {}", panic_msg)))
    })
}

fn detect_media_type(path: &Path) -> Result<MediaType, MediaError> {
    // 1. Magic bytes inference
    if let Ok(Some(kind)) = infer::get_from_path(path) {
        match kind.matcher_type() {
            infer::MatcherType::Image => return Ok(MediaType::Image),
            infer::MatcherType::Audio => return Ok(MediaType::Audio),
            infer::MatcherType::Video => return Ok(MediaType::Video),
            _ => {}
        }
    }

    // 2. Extension fallback
    if let Some(ext) = path.extension().and_then(|s| s.to_str()).map(|s| s.to_lowercase()) {
        match ext.as_str() {
            "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" | "tiff" | "ico" => Ok(MediaType::Image),
            "mp3" | "wav" | "flac" | "aac" | "ogg" | "m4a" | "opus" | "wma" | "aiff" => Ok(MediaType::Audio),
            "mp4" | "mov" | "m4v" => Ok(MediaType::Video),
            "mkv" | "webm" | "avi" => {
                Err(MediaError::UnsupportedFormat(format!("Container '.{}' not supported by pure-Rust MP4 reader", ext)))
            }
            other => Err(MediaError::UnsupportedFormat(format!("Unsupported file extension: .{}", other))),
        }
    } else {
        Err(MediaError::UnsupportedFormat("Unrecognized file format: no magic bytes or valid extension".to_string()))
    }
}

fn inspect_image(
    path: &Path,
    file_size_bytes: u64,
    timebase: Rational,
) -> Result<MediaInspection, MediaError> {
    let reader = image::ImageReader::open(path)
        .map_err(|e| MediaError::IoError(e.to_string()))?
        .with_guessed_format()
        .map_err(|e| MediaError::UnsupportedFormat(format!("Failed to determine image format: {}", e)))?;

    let (width, height) = reader.into_dimensions()
        .map_err(|e| MediaError::CorruptFile(format!("Failed to read image dimensions: {}", e)))?;

    let fps_ratio = if timebase.den != 0 {
        timebase.num as f64 / timebase.den as f64
    } else {
        60.0
    };
    let duration_seconds = DEFAULT_IMAGE_DURATION_SECONDS;
    let duration_pts = (duration_seconds * fps_ratio).round() as i64;

    Ok(MediaInspection {
        media_type: MediaType::Image,
        width: Some(width),
        height: Some(height),
        duration_seconds,
        duration_pts,
        timebase: Some(timebase),
        fps: None,
        audio_channels: None,
        sample_rate: None,
        file_size_bytes,
        video_codec: None,
        audio_codec: None,
        pixel_format: None,
        is_vfr: None,
        keyframe_pts: None,
    })
}

fn inspect_audio(
    path: &Path,
    file_size_bytes: u64,
    timebase: Rational,
) -> Result<MediaInspection, MediaError> {
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;

    let file = File::open(path).map_err(|e| MediaError::IoError(e.to_string()))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
        hint.with_extension(ext);
    }

    catch_unwind(AssertUnwindSafe(|| {
        let probe_result = symphonia::default::get_probe()
            .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
            .map_err(|e| MediaError::CorruptFile(format!("Audio probe failed: {}", e)))?;

        let format = probe_result.format;
        let track = format.default_track()
            .or_else(|| {
                format.tracks().iter().find(|t| t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL)
            })
            .ok_or_else(|| MediaError::CorruptFile("No playable audio track found in file".to_string()))?;

        let params = &track.codec_params;
        let sample_rate = params.sample_rate;
        if let Some(0) = sample_rate {
            return Err(MediaError::CorruptFile("Audio sample rate cannot be zero".to_string()));
        }

        let audio_channels = params.channels.map(|c| c.count() as u16);
        if let Some(0) = audio_channels {
            return Err(MediaError::CorruptFile("Audio channels cannot be zero".to_string()));
        }

        let duration_seconds = if let (Some(n_frames), Some(rate)) = (params.n_frames, params.sample_rate) {
            if rate > 0 {
                n_frames as f64 / rate as f64
            } else {
                0.0
            }
        } else if let (Some(time_base), Some(n_frames)) = (params.time_base, params.n_frames) {
            let time = time_base.calc_time(n_frames);
            time.seconds as f64 + time.frac
        } else {
            0.0
        };

        let fps_ratio = if timebase.den != 0 {
            timebase.num as f64 / timebase.den as f64
        } else {
            60.0
        };
        let duration_pts = (duration_seconds * fps_ratio).round() as i64;

        Ok(MediaInspection {
            media_type: MediaType::Audio,
            width: None,
            height: None,
            duration_seconds,
            duration_pts,
            timebase: Some(timebase),
            fps: None,
            audio_channels,
            sample_rate,
            file_size_bytes,
            video_codec: None,
            audio_codec: None, // Could sniff later from track codec, but keeping simple for now
            pixel_format: None,
            is_vfr: None,
            keyframe_pts: None,
        })
    }))
    .unwrap_or_else(|payload| {
        let panic_msg = if let Some(s) = payload.downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = payload.downcast_ref::<String>() {
            s.clone()
        } else {
            "Internal panic during audio probing".to_string()
        };
        Err(MediaError::CorruptFile(format!("Audio probe failed: {}", panic_msg)))
    })
}

fn inspect_video(
    path: &Path,
    file_size_bytes: u64,
    timebase: Rational,
) -> Result<MediaInspection, MediaError> {
    let file = File::open(path).map_err(|e| MediaError::IoError(e.to_string()))?;
    let reader = BufReader::new(file);

    let mp4_result = catch_unwind(AssertUnwindSafe(|| {
        let mp4 = mp4::Mp4Reader::read_header(reader, file_size_bytes)
            .map_err(|e| MediaError::CorruptFile(format!("Invalid MP4/video header: {}", e)))?;

        // Guard against integer divide-by-zero panic in mp4.duration() (mp4::reader:145:31)
        if mp4.moov.mvhd.timescale == 0 {
            return Err(MediaError::CorruptFile("MP4 container timescale is zero".to_string()));
        }

        let duration_seconds = mp4.duration().as_secs_f64();
        let fps_ratio = if timebase.den != 0 {
            timebase.num as f64 / timebase.den as f64
        } else {
            60.0
        };
        let duration_pts = (duration_seconds * fps_ratio).round() as i64;

        let video_track = mp4.tracks().values().find(|t| t.track_type().ok() == Some(mp4::TrackType::Video));
        let (width, height, fps) = match video_track {
            Some(vt) => {
                // Guard against integer divide-by-zero panic in vt.frame_rate() -> vt.duration() (mp4::track:209:31)
                if vt.timescale() == 0 {
                    return Err(MediaError::CorruptFile("Video track timescale is zero".to_string()));
                }
                let rate = vt.frame_rate();
                let fps = if rate > 0.0 && rate.is_finite() {
                    Some(rate)
                } else {
                    None
                };
                (Some(vt.width() as u32), Some(vt.height() as u32), fps)
            }
            None => (None, None, None),
        };

        let audio_track = mp4.tracks().values().find(|t| t.track_type().ok() == Some(mp4::TrackType::Audio));
        let (audio_channels, sample_rate) = match audio_track {
            Some(at) => {
                if at.timescale() == 0 {
                    return Err(MediaError::CorruptFile("Audio track timescale is zero".to_string()));
                }
                let ch = at.channel_config().ok().map(|c| c as u16).or(Some(2));
                let sr = Some(at.timescale());
                (ch, sr)
            }
            None => (None, None),
        };

        let video_codec = video_track.and_then(|vt| {
            match vt.media_type().ok() {
                Some(mp4::MediaType::H264) => Some("H.264".to_string()),
                Some(mp4::MediaType::H265) => Some("HEVC".to_string()),
                Some(mp4::MediaType::VP9) => Some("VP9".to_string()),
                _ => Some("Unknown".to_string()),
            }
        });

        let audio_codec = audio_track.and_then(|at| {
            match at.media_type().ok() {
                Some(mp4::MediaType::AAC) => Some("AAC".to_string()),
                Some(mp4::MediaType::TTXT) => Some("Text".to_string()),
                _ => Some("Unknown".to_string()),
            }
        });

        let pixel_format = video_track.and_then(|_vt| {
            Some("yuv420p".to_string())
        });

        let (is_vfr, keyframe_pts) = match video_track {
            Some(vt) => {
                let stbl = &vt.trak.mdia.minf.stbl;
                let mut detected_vfr = false;
                
                if stbl.stts.entries.len() > 1 {
                    detected_vfr = true;
                }

                let mut kf_list: Vec<i64> = Vec::new();
                let timescale = vt.timescale();
                
                let mut pts_by_sample_id = std::collections::HashMap::new();
                let mut current_dts: u64 = 0;
                let mut sample_id = 1u32;
                
                for entry in &stbl.stts.entries {
                    for _ in 0..entry.sample_count {
                        pts_by_sample_id.insert(sample_id, current_dts);
                        current_dts += entry.sample_delta as u64;
                        sample_id += 1;
                    }
                }
                
                if let Some(ctts) = &stbl.ctts {
                    let mut sample_id = 1u32;
                    for entry in &ctts.entries {
                        for _ in 0..entry.sample_count {
                            if let Some(pts) = pts_by_sample_id.get_mut(&sample_id) {
                                *pts = (*pts as i64 + entry.sample_offset as i64).max(0) as u64;
                            }
                            sample_id += 1;
                        }
                    }
                }
                
                if let Some(stss) = &stbl.stss {
                    for &sync_sample_id in &stss.entries {
                        if let Some(&pts) = pts_by_sample_id.get(&sync_sample_id) {
                            let pts_seconds = pts as f64 / timescale as f64;
                            let project_pts = (pts_seconds * fps_ratio).round() as i64;
                            kf_list.push(project_pts);
                        }
                    }
                } else {
                    for sample_id in 1..pts_by_sample_id.len() as u32 + 1 {
                        if let Some(&pts) = pts_by_sample_id.get(&sample_id) {
                            let pts_seconds = pts as f64 / timescale as f64;
                            let project_pts = (pts_seconds * fps_ratio).round() as i64;
                            kf_list.push(project_pts);
                        }
                    }
                }
                
                kf_list.sort_unstable();

                (Some(detected_vfr), Some(kf_list))
            },
            None => (None, None),
        };

        Ok(MediaInspection {
            media_type: MediaType::Video,
            width,
            height,
            duration_seconds,
            duration_pts,
            timebase: Some(timebase),
            fps,
            audio_channels,
            sample_rate,
            file_size_bytes,
            video_codec,
            audio_codec,
            pixel_format,
            is_vfr,
            keyframe_pts,
        })
    }))
    .unwrap_or_else(|payload| {
        let panic_msg = if let Some(s) = payload.downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = payload.downcast_ref::<String>() {
            s.clone()
        } else {
            "Internal panic while parsing MP4 container".to_string()
        };
        Err(MediaError::CorruptFile(format!("MP4 parser panicked: {}", panic_msg)))
    });

    // If the pure-Rust mp4 crate succeeded, return the result.
    if mp4_result.is_ok() {
        return mp4_result;
    }

    // Fallback: on macOS, use AVFoundation to inspect containers that the mp4 crate can't parse
    // (e.g., QuickTime .mov with proprietary Apple boxes).
    #[cfg(target_os = "macos")]
    {
        let path_str = path.to_str().unwrap_or_default();
        eprintln!(
            "[aether_media] mp4 crate failed for '{}': {}. Falling back to AVFoundation.",
            path_str,
            mp4_result.as_ref().unwrap_err()
        );

        match decoder::AvFoundationVideoDecoder::open_with_timebase(path_str, timebase) {
            Ok(dec) => {
                let fps_ratio = if timebase.den != 0 {
                    timebase.num as f64 / timebase.den as f64
                } else {
                    60.0
                };
                let duration_seconds = dec.duration_seconds();
                let duration_pts = (duration_seconds * fps_ratio).round() as i64;
                let fps_val = dec.fps();

                let mut audio_channels = None;
                let mut sample_rate = None;
                if let Ok(symphonia_dec) = decoder::SymphoniaAudioDecoder::open(path_str) {
                    let ch = symphonia_dec.channels() as u16;
                    if ch > 0 {
                        audio_channels = Some(ch);
                        sample_rate = Some(symphonia_dec.sample_rate());
                    }
                }

                return Ok(MediaInspection {
                    media_type: MediaType::Video,
                    width: Some(dec.width()),
                    height: Some(dec.height()),
                    duration_seconds,
                    duration_pts,
                    timebase: Some(timebase),
                    fps: if fps_val > 0.0 { Some(fps_val) } else { None },
                    audio_channels,
                    sample_rate,
                    file_size_bytes,
                    video_codec: Some("QuickTime".to_string()),
                    audio_codec: None,
                    pixel_format: Some("bgra".to_string()), // AVFoundation outputs BGRA
                    is_vfr: None,       // Cannot determine without box-level parsing
                    keyframe_pts: None, // Cannot extract without sample table access
                });
            }
            Err(avf_err) => {
                eprintln!(
                    "[aether_media] AVFoundation fallback also failed for '{}': {}",
                    path_str, avf_err
                );
            }
        }
    }

    // Neither mp4 crate nor AVFoundation could handle it: return the original mp4 error.
    mp4_result
}

pub mod audio_peaks;
pub mod decoder;

pub use decoder::{
    open_audio_decoder, open_audio_decoder_with_timebase, open_media_decoder,
    open_media_decoder_with_timebase, open_video_decoder, open_video_decoder_with_timebase,
    AudioBuffer, AudioDecoder, AudioSampleFormat, CompositeMediaDecoder, DecoderError,
    DecoderResult, ImageSequenceDecoder, MediaDecoder, PixelFormat, SymphoniaAudioDecoder,
    SyntheticPattern, SyntheticVideoDecoder, VideoDecoder, VideoFrame,
};

#[cfg(target_os = "macos")]
pub use decoder::AvFoundationVideoDecoder;
