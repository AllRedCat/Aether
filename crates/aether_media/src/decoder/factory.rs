use std::path::Path;

use super::error::DecoderError;
use super::image_decoder::ImageSequenceDecoder;
use super::symphonia_audio::SymphoniaAudioDecoder;
use super::synthetic::SyntheticVideoDecoder;
use super::traits::{AudioDecoder, DecoderResult, MediaDecoder, VideoDecoder};

#[cfg(target_os = "macos")]
use super::avfoundation::AvFoundationVideoDecoder;

use aether_core::media::{MediaMetadata, MediaType};
use aether_core::timeline::Rational;

/// Composite decoder holding optional video and audio decoder backends.
pub struct CompositeMediaDecoder {
    metadata: MediaMetadata,
    media_type: MediaType,
    video_decoder: Option<Box<dyn VideoDecoder>>,
    audio_decoder: Option<Box<dyn AudioDecoder>>,
}

impl CompositeMediaDecoder {
    pub fn new(
        metadata: MediaMetadata,
        media_type: MediaType,
        video_decoder: Option<Box<dyn VideoDecoder>>,
        audio_decoder: Option<Box<dyn AudioDecoder>>,
    ) -> Self {
        Self {
            metadata,
            media_type,
            video_decoder,
            audio_decoder,
        }
    }

    pub fn from_video(video: Box<dyn VideoDecoder>, metadata: MediaMetadata) -> Self {
        Self::new(metadata, MediaType::Video, Some(video), None)
    }

    pub fn from_audio(audio: Box<dyn AudioDecoder>, metadata: MediaMetadata) -> Self {
        Self::new(metadata, MediaType::Audio, None, Some(audio))
    }
}

impl MediaDecoder for CompositeMediaDecoder {
    fn metadata(&self) -> &MediaMetadata {
        &self.metadata
    }

    fn media_type(&self) -> MediaType {
        self.media_type
    }

    fn video(&mut self) -> Option<&mut (dyn VideoDecoder + '_)> {
        self.video_decoder
            .as_mut()
            .map(|v| v.as_mut() as &mut (dyn VideoDecoder + '_))
    }

    fn audio(&mut self) -> Option<&mut (dyn AudioDecoder + '_)> {
        self.audio_decoder
            .as_mut()
            .map(|a| a.as_mut() as &mut (dyn AudioDecoder + '_))
    }

    fn seek_pts(&mut self, target_pts: i64) -> DecoderResult<()> {
        let tb = self.metadata.timebase.unwrap_or(crate::DEFAULT_TIMEBASE);
        let timebase_fps = if tb.den != 0 && tb.num > 0 {
            tb.num as f64 / tb.den as f64
        } else {
            60.0
        };
        let sec = target_pts as f64 / timebase_fps;
        if let Some(video) = self.video_decoder.as_mut() {
            video.seek_second(sec)?;
        }
        if let Some(audio) = self.audio_decoder.as_mut() {
            audio.seek_second(sec)?;
        }
        Ok(())
    }
}

/// Opens any media asset with default project timebase (60 FPS) and returns a unified `MediaDecoder`.
pub fn open_media_decoder(path: &str) -> DecoderResult<Box<dyn MediaDecoder>> {
    open_media_decoder_with_timebase(path, crate::DEFAULT_TIMEBASE)
}

/// Opens any media asset with an explicit project timebase.
pub fn open_media_decoder_with_timebase(
    path_str: &str,
    timebase: Rational,
) -> DecoderResult<Box<dyn MediaDecoder>> {
    // 1. Synthetic URI handling (e.g. synthetic://test?width=64&height=64&fps=30&duration=1.0)
    if path_str.starts_with("synthetic://") {
        let syn_dec = SyntheticVideoDecoder::open_with_timebase(path_str, timebase)?;
        let metadata = MediaMetadata {
            width: Some(syn_dec.width()),
            height: Some(syn_dec.height()),
            duration_pts: syn_dec.duration_pts(),
            duration_seconds: syn_dec.duration_seconds(),
            timebase: Some(timebase),
            audio_channels: None,
            sample_rate: None,
            file_size_bytes: 0,
        };
        return Ok(Box::new(CompositeMediaDecoder::new(
            metadata,
            MediaType::Video,
            Some(Box::new(syn_dec)),
            None,
        )));
    }

    // 2. Validate filesystem path
    let path = Path::new(path_str);
    if !path.exists() {
        return Err(DecoderError::FileNotFound(path_str.to_string()));
    }
    if !path.is_file() {
        return Err(DecoderError::NotAFile(path_str.to_string()));
    }
    let file_size = std::fs::metadata(path)
        .map_err(|e| DecoderError::IoError(e.to_string()))?
        .len();
    if file_size == 0 {
        return Err(DecoderError::EmptyFile(path_str.to_string()));
    }

    // 3. Inspect container metadata
    let inspection = crate::inspect_media_file_with_timebase(path_str, timebase)?;
    let metadata = inspection.to_metadata();

    // 4. Dispatch based on detected media type
    match inspection.media_type {
        MediaType::Image => {
            let image_dec = ImageSequenceDecoder::open_with_timebase(path_str, timebase)?;
            Ok(Box::new(CompositeMediaDecoder::new(
                metadata,
                MediaType::Image,
                Some(Box::new(image_dec)),
                None,
            )))
        }
        MediaType::Audio => {
            let audio_dec = SymphoniaAudioDecoder::open_with_timebase(path_str, timebase)?;
            Ok(Box::new(CompositeMediaDecoder::new(
                metadata,
                MediaType::Audio,
                None,
                Some(Box::new(audio_dec)),
            )))
        }
        MediaType::Video => {
            #[cfg(target_os = "macos")]
            let video_dec: Box<dyn VideoDecoder> =
                Box::new(AvFoundationVideoDecoder::open_with_timebase(path_str, timebase)?);

            #[cfg(not(target_os = "macos"))]
            let video_dec: Box<dyn VideoDecoder> =
                Box::new(SyntheticVideoDecoder::open_with_timebase(path_str, timebase)?);

            // Attempt to open optional audio track if present
            let audio_dec: Option<Box<dyn AudioDecoder>> = if metadata.audio_channels.unwrap_or(0) > 0 {
                match SymphoniaAudioDecoder::open_with_timebase(path_str, timebase) {
                    Ok(a) => Some(Box::new(a)),
                    Err(_) => None,
                }
            } else {
                None
            };

            Ok(Box::new(CompositeMediaDecoder::new(
                metadata,
                MediaType::Video,
                Some(video_dec),
                audio_dec,
            )))
        }
    }
}

/// Convenience entry point to open a standalone `VideoDecoder`.
pub fn open_video_decoder(path: &str) -> DecoderResult<Box<dyn VideoDecoder>> {
    open_video_decoder_with_timebase(path, crate::DEFAULT_TIMEBASE)
}

/// Opens a standalone `VideoDecoder` with an explicit timebase.
pub fn open_video_decoder_with_timebase(
    path_str: &str,
    timebase: Rational,
) -> DecoderResult<Box<dyn VideoDecoder>> {
    if path_str.starts_with("synthetic://") {
        let dec = SyntheticVideoDecoder::open_with_timebase(path_str, timebase)?;
        return Ok(Box::new(dec));
    }

    let path = Path::new(path_str);
    if !path.exists() {
        return Err(DecoderError::FileNotFound(path_str.to_string()));
    }
    if !path.is_file() {
        return Err(DecoderError::NotAFile(path_str.to_string()));
    }
    let file_size = std::fs::metadata(path)
        .map_err(|e| DecoderError::IoError(e.to_string()))?
        .len();
    if file_size == 0 {
        return Err(DecoderError::EmptyFile(path_str.to_string()));
    }

    let inspection = crate::inspect_media_file_with_timebase(path_str, timebase)?;

    match inspection.media_type {
        MediaType::Image => {
            let dec = ImageSequenceDecoder::open_with_timebase(path_str, timebase)?;
            Ok(Box::new(dec))
        }
        MediaType::Video => {
            #[cfg(target_os = "macos")]
            {
                let dec = AvFoundationVideoDecoder::open_with_timebase(path_str, timebase)?;
                Ok(Box::new(dec))
            }
            #[cfg(not(target_os = "macos"))]
            {
                let dec = SyntheticVideoDecoder::open_with_timebase(path_str, timebase)?;
                Ok(Box::new(dec))
            }
        }
        MediaType::Audio => Err(DecoderError::NoVideoStream),
    }
}

/// Convenience entry point to open a standalone `AudioDecoder`.
pub fn open_audio_decoder(path: &str) -> DecoderResult<Box<dyn AudioDecoder>> {
    open_audio_decoder_with_timebase(path, crate::DEFAULT_TIMEBASE)
}

/// Opens a standalone `AudioDecoder` with an explicit timebase.
pub fn open_audio_decoder_with_timebase(
    path_str: &str,
    timebase: Rational,
) -> DecoderResult<Box<dyn AudioDecoder>> {
    let path = Path::new(path_str);
    if !path.exists() {
        return Err(DecoderError::FileNotFound(path_str.to_string()));
    }
    if !path.is_file() {
        return Err(DecoderError::NotAFile(path_str.to_string()));
    }
    let file_size = std::fs::metadata(path)
        .map_err(|e| DecoderError::IoError(e.to_string()))?
        .len();
    if file_size == 0 {
        return Err(DecoderError::EmptyFile(path_str.to_string()));
    }

    let inspection = crate::inspect_media_file_with_timebase(path_str, timebase)?;
    if inspection.media_type != MediaType::Audio && inspection.audio_channels.unwrap_or(0) == 0 {
        return Err(DecoderError::NoAudioStream);
    }

    let audio = SymphoniaAudioDecoder::open_with_timebase(path_str, timebase)?;
    Ok(Box::new(audio))
}
