use std::path::Path;

use super::error::DecoderError;
use super::traits::{DecoderResult, VideoDecoder};
use super::types::{PixelFormat, VideoFrame};
use aether_core::timeline::Rational;

pub const DEFAULT_IMAGE_DURATION_SECONDS: f64 = 5.0;
pub const DEFAULT_IMAGE_DURATION_PTS: i64 = 300; // 5.0s @ 60 FPS

/// Decodes still images (PNG, JPEG, WebP, BMP, GIF) into sequential video frames with standard NLE duration.
pub struct ImageSequenceDecoder {
    width: u32,
    height: u32,
    timebase: Rational,
    duration_pts: i64,
    duration_seconds: f64,
    fps: f64,
    current_pts: i64,
    cached_rgba: Vec<u8>,
}

impl ImageSequenceDecoder {
    pub fn open(path_str: &str) -> DecoderResult<Self> {
        Self::open_with_duration(
            path_str,
            DEFAULT_IMAGE_DURATION_SECONDS,
            crate::DEFAULT_TIMEBASE,
        )
    }

    pub fn open_with_timebase(path_str: &str, timebase: Rational) -> DecoderResult<Self> {
        Self::open_with_duration(path_str, DEFAULT_IMAGE_DURATION_SECONDS, timebase)
    }

    pub fn open_with_duration(
        path_str: &str,
        duration_seconds: f64,
        timebase: Rational,
    ) -> DecoderResult<Self> {
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

        let reader = image::ImageReader::open(path)
            .map_err(|e| DecoderError::IoError(e.to_string()))?
            .with_guessed_format()
            .map_err(|e| DecoderError::UnsupportedCodec(e.to_string()))?;

        let dynamic_image = reader
            .decode()
            .map_err(|e| DecoderError::DecodeError(format!("Failed to decode image: {}", e)))?;

        let rgba_image = dynamic_image.to_rgba8();
        let width = rgba_image.width();
        let height = rgba_image.height();
        let cached_rgba = rgba_image.into_raw();

        let fps = if timebase.den != 0 && timebase.num > 0 {
            timebase.num as f64 / timebase.den as f64
        } else {
            60.0
        };
        let duration_pts = (duration_seconds * fps).round().max(1.0) as i64;

        Ok(Self {
            width,
            height,
            timebase,
            duration_pts,
            duration_seconds,
            fps,
            current_pts: 0,
            cached_rgba,
        })
    }
}

impl VideoDecoder for ImageSequenceDecoder {
    fn width(&self) -> u32 {
        self.width
    }

    fn height(&self) -> u32 {
        self.height
    }

    fn timebase(&self) -> Rational {
        self.timebase
    }

    fn duration_pts(&self) -> i64 {
        self.duration_pts
    }

    fn duration_seconds(&self) -> f64 {
        self.duration_seconds
    }

    fn fps(&self) -> f64 {
        self.fps
    }

    fn pixel_format(&self) -> PixelFormat {
        PixelFormat::Rgba8
    }

    fn next_frame(&mut self) -> DecoderResult<Option<VideoFrame>> {
        if self.current_pts >= self.duration_pts {
            return Ok(None);
        }

        let frame = VideoFrame {
            width: self.width,
            height: self.height,
            format: PixelFormat::Rgba8,
            data: self.cached_rgba.clone(),
            row_stride_bytes: self.width * 4,
            pts: self.current_pts,
            duration_pts: self.duration_pts,
            timebase: self.timebase,
            is_keyframe: true,
        };
        self.current_pts += 1;
        Ok(Some(frame))
    }

    fn seek_pts(&mut self, target_pts: i64) -> DecoderResult<()> {
        self.current_pts = target_pts.clamp(0, self.duration_pts);
        Ok(())
    }
}
