use std::fs::File;
use std::io::BufReader;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;

use aether_core::media::{MediaItem, MediaMetadata, MediaTechInfo, MediaType};
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
    /// Deep-inspection data (filled by the ingest pipeline / container indexer).
    #[serde(default)]
    pub tech: Option<MediaTechInfo>,
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
            tech: self.tech.clone(),
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

    let container = reader
        .format()
        .map(|f| format!("{:?}", f).to_lowercase());

    let (width, height) = reader.into_dimensions()
        .map_err(|e| MediaError::CorruptFile(format!("Failed to read image dimensions: {}", e)))?;

    let fps_ratio = if timebase.den != 0 {
        timebase.num as f64 / timebase.den as f64
    } else {
        60.0
    };
    let duration_seconds = DEFAULT_IMAGE_DURATION_SECONDS;
    let duration_pts = (duration_seconds * fps_ratio).round() as i64;

    let content_hash = cache::compute_source_fingerprint(path)
        .map(|fp| fp.cache_key)
        .unwrap_or_default();

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
        video_codec: container.clone(),
        audio_codec: None,
        pixel_format: Some("rgba8".to_string()),
        is_vfr: Some(false),
        keyframe_pts: None,
        tech: Some(MediaTechInfo {
            content_hash,
            container,
            coded_width: Some(width),
            coded_height: Some(height),
            display_width: Some(width),
            display_height: Some(height),
            bit_depth: Some(8),
            track_timebase: Some(timebase),
            channel_layout: None,
            video_packet_count: Some(1),
            keyframe_count: Some(1),
        }),
    })
}

fn channel_layout_label(channels: u16) -> String {
    match channels {
        1 => "mono".to_string(),
        2 => "stereo".to_string(),
        6 => "5.1".to_string(),
        8 => "7.1".to_string(),
        n => format!("{}ch", n),
    }
}

fn inspect_audio(
    path: &Path,
    file_size_bytes: u64,
    timebase: Rational,
) -> Result<MediaInspection, MediaError> {
    use symphonia::core::codecs::{
        CODEC_TYPE_AAC, CODEC_TYPE_ALAC, CODEC_TYPE_FLAC, CODEC_TYPE_MP3, CODEC_TYPE_NULL,
        CODEC_TYPE_OPUS, CODEC_TYPE_VORBIS,
    };
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;

    let file = File::open(path).map_err(|e| MediaError::IoError(e.to_string()))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let ext_lower = path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase());
    let mut hint = Hint::new();
    if let Some(ref ext) = ext_lower {
        hint.with_extension(ext);
    }

    let content_hash = cache::compute_source_fingerprint(path)
        .map(|fp| fp.cache_key)
        .unwrap_or_default();

    catch_unwind(AssertUnwindSafe(|| {
        let probe_result = symphonia::default::get_probe()
            .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
            .map_err(|e| MediaError::CorruptFile(format!("Audio probe failed: {}", e)))?;

        let format = probe_result.format;
        let track = format.default_track()
            .or_else(|| {
                format.tracks().iter().find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
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

        let audio_codec = Some(match params.codec {
            c if c == CODEC_TYPE_AAC => "AAC".to_string(),
            c if c == CODEC_TYPE_MP3 => "MP3".to_string(),
            c if c == CODEC_TYPE_FLAC => "FLAC".to_string(),
            c if c == CODEC_TYPE_VORBIS => "Vorbis".to_string(),
            c if c == CODEC_TYPE_OPUS => "Opus".to_string(),
            c if c == CODEC_TYPE_ALAC => "ALAC".to_string(),
            _ => "PCM".to_string(),
        });

        let bit_depth = params.bits_per_sample.and_then(|b| u8::try_from(b).ok());

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
            audio_codec,
            pixel_format: None,
            is_vfr: None,
            keyframe_pts: None,
            tech: Some(MediaTechInfo {
                content_hash,
                container: ext_lower,
                coded_width: None,
                coded_height: None,
                display_width: None,
                display_height: None,
                bit_depth,
                track_timebase: sample_rate.map(|sr| Rational {
                    num: (sr.min(i32::MAX as u32) as i32).max(1),
                    den: 1,
                }),
                channel_layout: audio_channels.map(channel_layout_label),
                video_packet_count: None,
                keyframe_count: None,
            }),
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
    let fps_ratio = if timebase.den != 0 {
        timebase.num as f64 / timebase.den as f64
    } else {
        60.0
    };

    let content_hash = cache::compute_source_fingerprint(path)
        .map(|fp| fp.cache_key)
        .unwrap_or_default();
    let ext_lower = path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase());

    // 1. Run our tolerant ISO-BMFF (`mp4`/`mov`) box & sample-table parser first.
    //    This handles QuickTime `.mov` files whose `mp4a` v1 audio boxes trip the `mp4` crate,
    //    and strictly rejects corrupt `timescale == 0` headers.
    let iso_result = catch_unwind(AssertUnwindSafe(|| isobmff::parse_isobmff_file(path)))
        .unwrap_or_else(|_| {
            Err(MediaError::CorruptFile(
                "ISO-BMFF parser panicked".to_string(),
            ))
        });

    // Fatal structural errors (e.g. `timescale is zero` or missing `moov`) must be
    // rejected immediately and never masked by a platform decoder fallback.
    if let Err(MediaError::CorruptFile(ref msg)) = iso_result {
        if msg.contains("timescale is zero") || msg.contains("Missing 'moov'") {
            return Err(MediaError::CorruptFile(msg.clone()));
        }
    }

    if let Ok(ref iso) = iso_result {
        if let Some(ref vt) = iso.video {
            if vt.coded_width > 0 && vt.coded_height > 0 {
                let mut duration_seconds = if vt.duration_seconds > 0.0 {
                    vt.duration_seconds
                } else {
                    iso.duration_seconds
                };

                let mut audio_channels = iso.audio.as_ref().map(|a| a.channels);
                let mut sample_rate = iso.audio.as_ref().map(|a| a.sample_rate);
                let audio_codec = iso.audio.as_ref().map(|a| a.codec_name.clone());

                // If the container audio stsd had no channel/rate info, probe with Symphonia.
                if audio_channels.is_none() {
                    if let Some(path_str) = path.to_str() {
                        if let Ok(symphonia_dec) = decoder::SymphoniaAudioDecoder::open(path_str) {
                            let ch = symphonia_dec.channels();
                            if ch > 0 {
                                audio_channels = Some(ch);
                                sample_rate = Some(symphonia_dec.sample_rate());
                            }
                        }
                    }
                }

                // If container duration is 0 (e.g. QuickTime movies with fragmented/edts timing),
                // refine duration/fps via AVFoundation on macOS when available.
                #[cfg(target_os = "macos")]
                if duration_seconds <= 0.0 {
                    if let Some(path_str) = path.to_str() {
                        if let Ok(dec) =
                            decoder::AvFoundationVideoDecoder::open_with_timebase(path_str, timebase)
                        {
                            if dec.duration_seconds() > 0.0 {
                                duration_seconds = dec.duration_seconds();
                            }
                        }
                    }
                }

                let duration_pts = (duration_seconds * fps_ratio).round() as i64;
                let idx = vt.build_frame_index(timebase, [0u8; 32]);
                let keyframe_pts: Vec<i64> = idx.keyframes().map(|e| e.pts).collect();

                let container = iso
                    .major_brand
                    .as_deref()
                    .map(|b| b.trim().to_string())
                    .filter(|b| !b.is_empty())
                    .or(ext_lower);

                return Ok(MediaInspection {
                    media_type: MediaType::Video,
                    width: Some(vt.display_width),
                    height: Some(vt.display_height),
                    duration_seconds,
                    duration_pts,
                    timebase: Some(timebase),
                    fps: vt.fps,
                    audio_channels,
                    sample_rate,
                    file_size_bytes,
                    video_codec: Some(vt.codec_name.clone()),
                    audio_codec,
                    pixel_format: Some(vt.pixel_format.clone()),
                    is_vfr: Some(vt.is_vfr),
                    keyframe_pts: Some(keyframe_pts),
                    tech: Some(MediaTechInfo {
                        content_hash,
                        container,
                        coded_width: Some(vt.coded_width),
                        coded_height: Some(vt.coded_height),
                        display_width: Some(vt.display_width),
                        display_height: Some(vt.display_height),
                        bit_depth: Some(vt.bit_depth),
                        track_timebase: Some(Rational {
                            num: (vt.timescale.min(i32::MAX as u32) as i32).max(1),
                            den: 1,
                        }),
                        channel_layout: audio_channels.map(channel_layout_label),
                        video_packet_count: u32::try_from(vt.packets.len()).ok(),
                        keyframe_count: u32::try_from(vt.keyframe_count).ok(),
                    }),
                });
            }
        }
    }

    // 2. Fallback to the `mp4` crate for edge-case containers where `stsd` dimensions
    //    were stored only in SPS/AVCC and not in the VisualSampleEntry header.
    let file = File::open(path).map_err(|e| MediaError::IoError(e.to_string()))?;
    let reader = BufReader::new(file);

    let mp4_result = catch_unwind(AssertUnwindSafe(|| {
        let mp4 = mp4::Mp4Reader::read_header(reader, file_size_bytes)
            .map_err(|e| MediaError::CorruptFile(format!("Invalid MP4/video header: {}", e)))?;

        if mp4.moov.mvhd.timescale == 0 {
            return Err(MediaError::CorruptFile("MP4 container timescale is zero".to_string()));
        }

        let duration_seconds = mp4.duration().as_secs_f64();
        let duration_pts = (duration_seconds * fps_ratio).round() as i64;

        let video_track = mp4.tracks().values().find(|t| t.track_type().ok() == Some(mp4::TrackType::Video));
        let (width, height, fps) = match video_track {
            Some(vt) => {
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

        let video_codec = video_track.and_then(|vt| match vt.media_type().ok() {
            Some(mp4::MediaType::H264) => Some("H.264".to_string()),
            Some(mp4::MediaType::H265) => Some("HEVC".to_string()),
            Some(mp4::MediaType::VP9) => Some("VP9".to_string()),
            _ => Some("Unknown".to_string()),
        });

        let audio_codec = audio_track.and_then(|at| match at.media_type().ok() {
            Some(mp4::MediaType::AAC) => Some("AAC".to_string()),
            Some(mp4::MediaType::TTXT) => Some("Text".to_string()),
            _ => Some("Unknown".to_string()),
        });

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
            pixel_format: video_track.map(|_| "yuv420p".to_string()),
            is_vfr: Some(false),
            keyframe_pts: Some(Vec::new()),
            tech: Some(MediaTechInfo {
                content_hash: content_hash.clone(),
                container: ext_lower.clone(),
                coded_width: width,
                coded_height: height,
                display_width: width,
                display_height: height,
                bit_depth: Some(8),
                track_timebase: Some(timebase),
                channel_layout: audio_channels.map(channel_layout_label),
                video_packet_count: None,
                keyframe_count: None,
            }),
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

    if mp4_result.is_ok() {
        return mp4_result;
    }

    // Do not fall back to AVFoundation if the MP4 reader detected a zero timescale.
    if let Err(MediaError::CorruptFile(ref msg)) = mp4_result {
        if msg.contains("timescale is zero") {
            return mp4_result;
        }
    }

    // 3. Last-resort fallback on macOS via AVFoundation for non-ISO-BMFF or heavily
    //    proprietary QuickTime containers (e.g. compressed `cmov` headers).
    #[cfg(target_os = "macos")]
    {
        let path_str = path.to_str().unwrap_or_default();
        if let Ok(dec) = decoder::AvFoundationVideoDecoder::open_with_timebase(path_str, timebase) {
            let duration_seconds = dec.duration_seconds();
            let duration_pts = (duration_seconds * fps_ratio).round() as i64;
            let fps_val = dec.fps();

            let mut audio_channels = None;
            let mut sample_rate = None;
            if let Ok(symphonia_dec) = decoder::SymphoniaAudioDecoder::open(path_str) {
                let ch = symphonia_dec.channels();
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
                pixel_format: Some("bgra".to_string()),
                is_vfr: None,
                keyframe_pts: None,
                tech: Some(MediaTechInfo {
                    content_hash,
                    container: ext_lower,
                    coded_width: Some(dec.width()),
                    coded_height: Some(dec.height()),
                    display_width: Some(dec.width()),
                    display_height: Some(dec.height()),
                    bit_depth: Some(8),
                    track_timebase: Some(dec.timebase()),
                    channel_layout: audio_channels.map(channel_layout_label),
                    video_packet_count: None,
                    keyframe_count: None,
                }),
            });
        }
    }

    mp4_result
}

pub mod audio_peaks;
pub mod cache;
pub mod decoder;
pub mod frame_index;
pub mod importer;
pub mod isobmff;

pub use cache::{
    compute_fingerprint_from_parts, compute_source_fingerprint, CacheManager, SourceFingerprint,
    PARTIAL_HASH_WINDOW_BYTES,
};
pub use decoder::{
    open_audio_decoder, open_audio_decoder_with_timebase, open_media_decoder,
    open_media_decoder_with_timebase, open_video_decoder, open_video_decoder_with_timebase,
    AudioBuffer, AudioDecoder, AudioSampleFormat, CompositeMediaDecoder, DecoderError,
    DecoderResult, ImageSequenceDecoder, MediaDecoder, PixelFormat, SymphoniaAudioDecoder,
    SyntheticPattern, SyntheticVideoDecoder, VideoDecoder, VideoFrame,
};
pub use frame_index::{
    binary_search_keyframe_at_or_before, binary_search_keyframe_clamped, decode_entries_raw,
    encode_entries_raw, FrameIndex, FrameIndexEntry, FLAG_IS_DISPOSABLE, FLAG_IS_KEYFRAME,
    FRAME_INDEX_ENTRY_SIZE, FRAME_INDEX_HEADER_SIZE, FRAME_INDEX_MAGIC, FRAME_INDEX_VERSION,
};
pub use importer::{
    build_or_load_frame_index, build_or_load_frame_index_with_timebase, ingest_media_with_cache,
    seek_nearest_keyframe, IngestedMedia,
};

#[cfg(target_os = "macos")]
pub use decoder::AvFoundationVideoDecoder;
