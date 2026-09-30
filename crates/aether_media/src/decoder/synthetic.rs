use super::traits::{DecoderResult, VideoDecoder};
use super::types::{PixelFormat, VideoFrame};
use aether_core::timeline::Rational;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntheticPattern {
    /// Each frame has an algebraically distinct, deterministic solid color.
    ColorCycle,
    /// Dark gray background with a bright moving vertical scrubber bar.
    MovingBar,
    /// 8-column SMPTE standard test pattern.
    SMPTEBars,
}

pub struct SyntheticVideoDecoder {
    width: u32,
    height: u32,
    fps: f64,
    timebase: Rational,
    total_frames: u64,
    current_frame: i64,
    pattern: SyntheticPattern,
    duration_pts: i64,
    pts_per_frame: i64,
}

impl SyntheticVideoDecoder {
    pub fn new(width: u32, height: u32, fps: f64, duration_seconds: f64) -> Self {
        Self::with_pattern_and_timebase(
            width,
            height,
            fps,
            duration_seconds,
            SyntheticPattern::ColorCycle,
            Rational {
                num: (fps.round() as i32).max(1),
                den: 1,
            },
        )
    }

    pub fn with_pattern(
        width: u32,
        height: u32,
        fps: f64,
        duration_seconds: f64,
        pattern: SyntheticPattern,
    ) -> Self {
        Self::with_pattern_and_timebase(
            width,
            height,
            fps,
            duration_seconds,
            pattern,
            Rational {
                num: (fps.round() as i32).max(1),
                den: 1,
            },
        )
    }

    pub fn with_pattern_and_timebase(
        width: u32,
        height: u32,
        fps: f64,
        duration_seconds: f64,
        pattern: SyntheticPattern,
        timebase: Rational,
    ) -> Self {
        let fps = if fps > 0.0 { fps } else { 30.0 };
        let duration_seconds = if duration_seconds > 0.0 {
            duration_seconds
        } else {
            1.0
        };
        let total_frames = (duration_seconds * fps).round().max(1.0) as u64;

        let timeline_fps = if timebase.den != 0 && timebase.num > 0 {
            timebase.num as f64 / timebase.den as f64
        } else {
            60.0
        };
        let duration_pts = (duration_seconds * timeline_fps).round().max(1.0) as i64;
        let pts_per_frame = if total_frames > 0 {
            (duration_pts / total_frames as i64).max(1)
        } else {
            1
        };

        Self {
            width: width.max(16),
            height: height.max(16),
            fps,
            timebase,
            total_frames,
            current_frame: 0,
            pattern,
            duration_pts,
            pts_per_frame,
        }
    }

    /// Parses a `synthetic://` URI or query string (e.g. `synthetic://test?width=64&height=64&fps=30&duration=1.0`)
    pub fn open_with_timebase(uri_str: &str, timebase: Rational) -> DecoderResult<Self> {
        let mut width = 64u32;
        let mut height = 64u32;
        let mut fps = 30.0f64;
        let mut duration_seconds = 1.0f64;
        let mut pattern = SyntheticPattern::ColorCycle;

        if let Some(query) = uri_str.split('?').nth(1) {
            for param in query.split('&') {
                let mut parts = param.split('=');
                if let (Some(key), Some(val)) = (parts.next(), parts.next()) {
                    match key {
                        "width" | "w" => {
                            if let Ok(w) = val.parse::<u32>() {
                                width = w;
                            }
                        }
                        "height" | "h" => {
                            if let Ok(h) = val.parse::<u32>() {
                                height = h;
                            }
                        }
                        "fps" | "r" => {
                            if let Ok(f) = val.parse::<f64>() {
                                fps = f;
                            }
                        }
                        "duration" | "dur" | "d" => {
                            if let Ok(d) = val.parse::<f64>() {
                                duration_seconds = d;
                            }
                        }
                        "pattern" | "p" => match val {
                            "moving_bar" | "movingbar" | "bar" => pattern = SyntheticPattern::MovingBar,
                            "smpte" | "bars" => pattern = SyntheticPattern::SMPTEBars,
                            _ => pattern = SyntheticPattern::ColorCycle,
                        },
                        _ => {}
                    }
                }
            }
        }

        Ok(Self::with_pattern_and_timebase(
            width,
            height,
            fps,
            duration_seconds,
            pattern,
            timebase,
        ))
    }

    fn generate_frame_at(&self, frame_idx: i64) -> VideoFrame {
        let pixel_count = (self.width * self.height) as usize;
        let mut data = vec![0u8; pixel_count * 4];

        match self.pattern {
            SyntheticPattern::ColorCycle => {
                let r = ((frame_idx * 67 + 31) % 256) as u8;
                let g = ((frame_idx * 131 + 73) % 256) as u8;
                let b = ((frame_idx * 197 + 127) % 256) as u8;
                let a = 255u8;
                for chunk in data.chunks_exact_mut(4) {
                    chunk[0] = r;
                    chunk[1] = g;
                    chunk[2] = b;
                    chunk[3] = a;
                }
            }
            SyntheticPattern::MovingBar => {
                // Background: #202020
                for chunk in data.chunks_exact_mut(4) {
                    chunk[0] = 32;
                    chunk[1] = 32;
                    chunk[2] = 32;
                    chunk[3] = 255;
                }
                let bar_width = (self.width / 16).max(2);
                let max_x = self.width.saturating_sub(bar_width);
                let bar_x = if self.total_frames > 1 {
                    ((frame_idx as u64 % self.total_frames) * max_x as u64 / (self.total_frames - 1)) as u32
                } else {
                    0
                };
                for y in 0..self.height {
                    for x in bar_x..(bar_x + bar_width).min(self.width) {
                        let offset = ((y * self.width + x) * 4) as usize;
                        data[offset] = 0;       // R
                        data[offset + 1] = 255; // G (Cyan bar)
                        data[offset + 2] = 255; // B
                        data[offset + 3] = 255; // A
                    }
                }
            }
            SyntheticPattern::SMPTEBars => {
                let colors: [[u8; 4]; 8] = [
                    [255, 255, 255, 255], // White
                    [255, 255, 0, 255],   // Yellow
                    [0, 255, 255, 255],   // Cyan
                    [0, 255, 0, 255],     // Green
                    [255, 0, 255, 255],   // Magenta
                    [255, 0, 0, 255],     // Red
                    [0, 0, 255, 255],     // Blue
                    [0, 0, 0, 255],       // Black
                ];
                let col_width = (self.width / 8).max(1);
                for y in 0..self.height {
                    for x in 0..self.width {
                        let col_idx = ((x / col_width) as usize).min(7);
                        let offset = ((y * self.width + x) * 4) as usize;
                        data[offset..offset + 4].copy_from_slice(&colors[col_idx]);
                    }
                }
            }
        }

        let timeline_fps = if self.timebase.den != 0 && self.timebase.num > 0 {
            self.timebase.num as f64 / self.timebase.den as f64
        } else {
            60.0
        };
        let frame_sec = frame_idx as f64 / self.fps;
        let pts = (frame_sec * timeline_fps).round() as i64;

        VideoFrame {
            width: self.width,
            height: self.height,
            format: PixelFormat::Rgba8,
            data,
            row_stride_bytes: self.width * 4,
            pts,
            duration_pts: self.pts_per_frame,
            timebase: self.timebase,
            is_keyframe: true,
        }
    }
}

impl VideoDecoder for SyntheticVideoDecoder {
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
        self.total_frames as f64 / self.fps
    }

    fn fps(&self) -> f64 {
        self.fps
    }

    fn pixel_format(&self) -> PixelFormat {
        PixelFormat::Rgba8
    }

    fn next_frame(&mut self) -> DecoderResult<Option<VideoFrame>> {
        if self.current_frame >= self.total_frames as i64 {
            return Ok(None);
        }
        let frame = self.generate_frame_at(self.current_frame);
        self.current_frame += 1;
        Ok(Some(frame))
    }

    fn seek_pts(&mut self, target_pts: i64) -> DecoderResult<()> {
        if target_pts < 0 {
            self.current_frame = 0;
            return Ok(());
        }
        if target_pts >= self.duration_pts {
            self.current_frame = self.total_frames as i64;
            return Ok(());
        }

        let timeline_fps = if self.timebase.den != 0 && self.timebase.num > 0 {
            self.timebase.num as f64 / self.timebase.den as f64
        } else {
            60.0
        };
        let target_sec = target_pts as f64 / timeline_fps;
        self.current_frame = ((target_sec * self.fps).round() as i64).clamp(0, self.total_frames as i64);
        Ok(())
    }
}
