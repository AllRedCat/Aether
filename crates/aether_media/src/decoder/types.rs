use aether_core::timeline::Rational;
use serde::{Deserialize, Serialize};

/// Supported pixel formats for video frame representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PixelFormat {
    /// 32-bit RGBA (Red, Green, Blue, Alpha) - Standard Flutter & WebGPU format.
    Rgba8,
    /// 32-bit BGRA (Blue, Green, Red, Alpha) - Native macOS/iOS AVFoundation format.
    Bgra8,
    /// 24-bit RGB (Red, Green, Blue).
    Rgb8,
    /// Planar YUV 4:2:0.
    Yuv420p,
}

impl PixelFormat {
    pub fn bytes_per_pixel(&self) -> usize {
        match self {
            PixelFormat::Rgba8 | PixelFormat::Bgra8 => 4,
            PixelFormat::Rgb8 => 3,
            PixelFormat::Yuv420p => 1,
        }
    }
}

/// Represents an uncompressed decoded video frame with explicit presentation timestamp.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VideoFrame {
    pub width: u32,
    pub height: u32,
    pub format: PixelFormat,
    pub data: Vec<u8>,
    pub row_stride_bytes: u32,
    pub pts: i64,
    pub duration_pts: i64,
    pub timebase: Rational,
    pub is_keyframe: bool,
}

impl VideoFrame {
    /// Creates a new `VideoFrame` with default contiguous row stride and keyframe flag.
    pub fn new(
        width: u32,
        height: u32,
        format: PixelFormat,
        data: Vec<u8>,
        pts: i64,
        duration_pts: i64,
        timebase: Rational,
    ) -> Self {
        let bpp = format.bytes_per_pixel() as u32;
        Self {
            width,
            height,
            format,
            data,
            row_stride_bytes: width * bpp,
            pts,
            duration_pts,
            timebase,
            is_keyframe: true,
        }
    }

    /// Creates a new `VideoFrame` with explicit row stride and keyframe flag.
    pub fn with_stride(
        width: u32,
        height: u32,
        format: PixelFormat,
        data: Vec<u8>,
        row_stride_bytes: u32,
        pts: i64,
        duration_pts: i64,
        timebase: Rational,
        is_keyframe: bool,
    ) -> Self {
        Self {
            width,
            height,
            format,
            data,
            row_stride_bytes,
            pts,
            duration_pts,
            timebase,
            is_keyframe,
        }
    }

    /// Converts the frame in-place to RGBA8 format if needed.
    ///
    /// - If `PixelFormat::Bgra8`, performs in-place byte swapping of R and B channels,
    ///   respecting potential row stride padding.
    /// - If `PixelFormat::Rgb8`, converts 24-bit RGB into 32-bit RGBA (Alpha = 255).
    /// - If already `PixelFormat::Rgba8`, this operation is a zero-cost no-op.
    pub fn ensure_rgba8(&mut self) {
        match self.format {
            PixelFormat::Rgba8 => {}
            PixelFormat::Bgra8 => {
                let row_bytes = (self.width * 4) as usize;
                let stride = self.row_stride_bytes as usize;
                if stride == row_bytes && self.data.len() >= (self.width * self.height * 4) as usize {
                    for chunk in self.data.chunks_exact_mut(4) {
                        chunk.swap(0, 2);
                    }
                } else {
                    for row in 0..self.height as usize {
                        let start = row * stride;
                        let end = start + row_bytes;
                        if end <= self.data.len() {
                            for chunk in self.data[start..end].chunks_exact_mut(4) {
                                chunk.swap(0, 2);
                            }
                        }
                    }
                }
                self.format = PixelFormat::Rgba8;
            }
            PixelFormat::Rgb8 => {
                let pixel_count = (self.width * self.height) as usize;
                let mut rgba = Vec::with_capacity(pixel_count * 4);
                let row_bytes = (self.width * 3) as usize;
                let stride = self.row_stride_bytes as usize;
                if stride == row_bytes {
                    for chunk in self.data.chunks_exact(3) {
                        rgba.extend_from_slice(&[chunk[0], chunk[1], chunk[2], 255]);
                    }
                } else {
                    for row in 0..self.height as usize {
                        let start = row * stride;
                        let end = start + row_bytes;
                        if end <= self.data.len() {
                            for chunk in self.data[start..end].chunks_exact(3) {
                                rgba.extend_from_slice(&[chunk[0], chunk[1], chunk[2], 255]);
                            }
                        }
                    }
                }
                self.data = rgba;
                self.row_stride_bytes = self.width * 4;
                self.format = PixelFormat::Rgba8;
            }
            PixelFormat::Yuv420p => {
                // Reserved for future planar YUV conversion hook
            }
        }
    }

    /// Scales the frame in-place into the destination buffer `dst_data` using fast
    /// high-quality box-area downsampling.
    ///
    /// The destination buffer `dst_data` is cleared and populated with the downscaled
    /// RGBA pixels. Then `self.data` and `*dst_data` are swapped so `self.data` contains
    /// the downscaled frame and `*dst_data` contains the old unscaled buffer (available for recycling).
    ///
    /// If the frame is already within `(target_width, target_height)`, this is a zero-cost no-op.
    pub fn downscale_into(
        &mut self,
        target_width: u32,
        target_height: u32,
        dst_data: &mut Vec<u8>,
    ) {
        if self.width <= target_width && self.height <= target_height {
            return;
        }
        if self.width == 0 || self.height == 0 || target_width == 0 || target_height == 0 {
            return;
        }
        self.ensure_rgba8();

        let src_w = self.width as usize;
        let src_h = self.height as usize;
        let dst_w = target_width as usize;
        let dst_h = target_height as usize;

        let src_stride = self.row_stride_bytes as usize;
        let dst_stride = dst_w * 4;
        let required_bytes = dst_stride * dst_h;

        dst_data.clear();
        if dst_data.capacity() < required_bytes {
            dst_data.reserve(required_bytes - dst_data.capacity());
        }

        for y in 0..dst_h {
            let src_y_start = (y * src_h) / dst_h;
            let src_y_end = (((y + 1) * src_h) / dst_h).max(src_y_start + 1).min(src_h);
            let y_span = (src_y_end - src_y_start) as u32;

            for x in 0..dst_w {
                let src_x_start = (x * src_w) / dst_w;
                let src_x_end = (((x + 1) * src_w) / dst_w).max(src_x_start + 1).min(src_w);
                let x_span = (src_x_end - src_x_start) as u32;

                let area = (x_span * y_span).max(1) as u32;
                let mut sum_r = 0u32;
                let mut sum_g = 0u32;
                let mut sum_b = 0u32;
                let mut sum_a = 0u32;

                for sy in src_y_start..src_y_end {
                    let row_offset = sy * src_stride;
                    for sx in src_x_start..src_x_end {
                        let px_offset = row_offset + sx * 4;
                        if px_offset + 3 < self.data.len() {
                            sum_r += self.data[px_offset] as u32;
                            sum_g += self.data[px_offset + 1] as u32;
                            sum_b += self.data[px_offset + 2] as u32;
                            sum_a += self.data[px_offset + 3] as u32;
                        }
                    }
                }

                dst_data.push((sum_r / area) as u8);
                dst_data.push((sum_g / area) as u8);
                dst_data.push((sum_b / area) as u8);
                dst_data.push((sum_a / area) as u8);
            }
        }

        std::mem::swap(&mut self.data, dst_data);
        self.width = target_width;
        self.height = target_height;
        self.row_stride_bytes = (target_width * 4) as u32;
    }

    /// Convenience in-place downscale allocating an internal temporary buffer if scaling is required.
    pub fn downscale_to(&mut self, target_width: u32, target_height: u32) {
        let mut tmp = Vec::new();
        self.downscale_into(target_width, target_height, &mut tmp);
    }

    /// Returns the effective frames-per-second (FPS) derived from the timebase.
    pub fn fps(&self) -> f64 {
        if self.timebase.den != 0 && self.timebase.num > 0 {
            self.timebase.num as f64 / self.timebase.den as f64
        } else {
            60.0
        }
    }

    /// Presentation timestamp converted to fractional seconds.
    pub fn timestamp_seconds(&self) -> f64 {
        let fps = self.fps();
        if fps > 0.0 {
            self.pts as f64 / fps
        } else {
            0.0
        }
    }

    /// Duration converted to fractional seconds.
    pub fn duration_seconds(&self) -> f64 {
        let fps = self.fps();
        if fps > 0.0 {
            self.duration_pts as f64 / fps
        } else {
            0.0
        }
    }

    /// Total byte length of image data.
    pub fn byte_len(&self) -> usize {
        self.data.len()
    }
}

/// Supported sample formats for uncompressed audio PCM buffers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AudioSampleFormat {
    /// 32-bit floating point PCM, interleaved channels [L0, R0, L1, R1, ...].
    F32Interleaved,
    /// 16-bit signed integer PCM, interleaved channels.
    I16Interleaved,
}

/// Represents an uncompressed decoded audio buffer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioBuffer {
    pub sample_rate: u32,
    pub channels: u16,
    pub format: AudioSampleFormat,
    pub samples_f32: Vec<f32>,
    pub pts: i64,
    pub duration_pts: i64,
    pub timebase: Rational,
}

impl AudioBuffer {
    /// Creates a new `AudioBuffer` with `F32Interleaved` sample format.
    pub fn new(
        sample_rate: u32,
        channels: u16,
        samples_f32: Vec<f32>,
        pts: i64,
        duration_pts: i64,
        timebase: Rational,
    ) -> Self {
        Self {
            sample_rate,
            channels,
            format: AudioSampleFormat::F32Interleaved,
            samples_f32,
            pts,
            duration_pts,
            timebase,
        }
    }

    /// Total number of individual audio samples across all channels.
    pub fn sample_count(&self) -> usize {
        self.samples_f32.len()
    }

    /// Number of audio frames (sample periods): `samples / channels`.
    pub fn frame_count(&self) -> usize {
        if self.channels > 0 {
            self.samples_f32.len() / self.channels as usize
        } else {
            0
        }
    }

    /// Duration in fractional seconds based on sample rate.
    pub fn duration_seconds(&self) -> f64 {
        if self.sample_rate > 0 {
            self.frame_count() as f64 / self.sample_rate as f64
        } else {
            0.0
        }
    }

    /// Timestamp in fractional seconds based on timebase and PTS.
    pub fn timestamp_seconds(&self) -> f64 {
        let fps = if self.timebase.den != 0 && self.timebase.num > 0 {
            self.timebase.num as f64 / self.timebase.den as f64
        } else {
            60.0
        };
        if fps > 0.0 {
            self.pts as f64 / fps
        } else {
            0.0
        }
    }
}

/// Calculates target preview proxy dimensions constrained by `max_width` and `max_height`,
/// strictly preserving the original aspect ratio and ensuring even pixel dimensions.
///
/// If the source dimensions are already within the bounds, returns `(source_width, source_height)`
/// without upscaling.
pub fn calculate_preview_dimensions(
    source_width: u32,
    source_height: u32,
    max_width: Option<u32>,
    max_height: Option<u32>,
) -> (u32, u32) {
    if source_width == 0 || source_height == 0 {
        return (source_width, source_height);
    }
    let max_w = match max_width {
        Some(w) if w > 0 => w,
        _ => return (source_width, source_height),
    };
    let max_h = match max_height {
        Some(h) if h > 0 => h,
        _ => return (source_width, source_height),
    };

    if source_width <= max_w && source_height <= max_h {
        return (source_width, source_height);
    }

    let scale_w = max_w as f64 / source_width as f64;
    let scale_h = max_h as f64 / source_height as f64;
    let scale = scale_w.min(scale_h);

    let target_w = ((source_width as f64 * scale).round() as u32).max(2) & !1;
    let target_h = ((source_height as f64 * scale).round() as u32).max(2) & !1;
    (target_w, target_h)
}

