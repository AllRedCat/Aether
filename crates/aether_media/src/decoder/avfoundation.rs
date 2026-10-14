#[cfg(target_os = "macos")]
mod native {
    use std::ffi::{c_char, c_void, CString};
    use std::path::Path;

    use crate::decoder::error::DecoderError;
    use crate::decoder::traits::{DecoderResult, VideoDecoder};
    use crate::decoder::types::{PixelFormat, VideoFrame};
    use aether_core::timeline::Rational;

    #[repr(C)]
    struct AvfMediaInfo {
        width: u32,
        height: u32,
        duration_seconds: f64,
        duration_value: i64,
        duration_timescale: i32,
        fps: f64,
    }

    #[repr(C)]
    struct AvfRawFrame {
        width: u32,
        height: u32,
        bytes_per_row: u32,
        pts_value: i64,
        pts_timescale: i32,
        duration_value: i64,
        duration_timescale: i32,
        is_keyframe: i32,
        data: *mut u8,
        data_len: usize,
    }

    extern "C" {
        #[allow(dead_code)]
        fn avf_decoder_create(
            file_path: *const c_char,
            out_info: *mut AvfMediaInfo,
            err_buf: *mut c_char,
            err_len: usize,
        ) -> *mut c_void;
        fn avf_decoder_create_with_max_size(
            file_path: *const c_char,
            max_width: u32,
            max_height: u32,
            out_info: *mut AvfMediaInfo,
            err_buf: *mut c_char,
            err_len: usize,
        ) -> *mut c_void;
        fn avf_decoder_next_frame(handle: *mut c_void, out_frame: *mut AvfRawFrame) -> i32;
        fn avf_decoder_next_frame_at_or_after(
            handle: *mut c_void,
            min_pts_value: i64,
            min_pts_timescale: i32,
            out_frame: *mut AvfRawFrame,
        ) -> i32;
        fn avf_decoder_free_frame(frame: *mut AvfRawFrame);
        fn avf_decoder_seek_pts(
            handle: *mut c_void,
            target_value: i64,
            target_timescale: i32,
        ) -> i32;
        fn avf_decoder_destroy(handle: *mut c_void);
    }

    /// Hardware-accelerated macOS video decoder powered by AVFoundation.
    pub struct AvFoundationVideoDecoder {
        handle: *mut c_void,
        width: u32,
        height: u32,
        fps: f64,
        duration_seconds: f64,
        duration_pts: i64,
        timebase: Rational,
    }

    unsafe impl Send for AvFoundationVideoDecoder {}
    unsafe impl Sync for AvFoundationVideoDecoder {}

    impl AvFoundationVideoDecoder {
        pub fn open(path: &str) -> DecoderResult<Self> {
            Self::open_with_config(path, crate::DEFAULT_TIMEBASE, None, None)
        }

        pub fn open_with_timebase(path: &str, timebase: Rational) -> DecoderResult<Self> {
            Self::open_with_config(path, timebase, None, None)
        }

        pub fn open_with_config(
            path: &str,
            timebase: Rational,
            max_width: Option<u32>,
            max_height: Option<u32>,
        ) -> DecoderResult<Self> {
            let p = Path::new(path);
            if !p.exists() {
                return Err(DecoderError::FileNotFound(path.to_string()));
            }
            if !p.is_file() {
                return Err(DecoderError::NotAFile(path.to_string()));
            }
            let file_size = std::fs::metadata(p)
                .map_err(|e| DecoderError::IoError(e.to_string()))?
                .len();
            if file_size == 0 {
                return Err(DecoderError::EmptyFile(path.to_string()));
            }

            let c_path = CString::new(path)
                .map_err(|e| DecoderError::InitializationFailed(e.to_string()))?;
            let mut info = std::mem::MaybeUninit::<AvfMediaInfo>::uninit();
            let mut err_buf = [0 as c_char; 512];

            let max_w = max_width.unwrap_or(0);
            let max_h = max_height.unwrap_or(0);

            let handle = unsafe {
                avf_decoder_create_with_max_size(
                    c_path.as_ptr(),
                    max_w,
                    max_h,
                    info.as_mut_ptr(),
                    err_buf.as_mut_ptr(),
                    err_buf.len(),
                )
            };

            if handle.is_null() {
                let err_str = unsafe {
                    std::ffi::CStr::from_ptr(err_buf.as_ptr())
                        .to_string_lossy()
                        .to_string()
                };
                if err_str.contains("No video track") {
                    return Err(DecoderError::NoVideoStream);
                }
                if err_str.contains("File does not exist") {
                    return Err(DecoderError::FileNotFound(path.to_string()));
                }
                return Err(DecoderError::InitializationFailed(err_str));
            }

            let info = unsafe { info.assume_init() };
            let effective_timebase = if timebase == crate::DEFAULT_TIMEBASE {
                Rational {
                    num: (info.fps.round() as i32).max(1),
                    den: 1,
                }
            } else {
                timebase
            };
            let fps_ratio = if effective_timebase.den != 0 && effective_timebase.num > 0 {
                effective_timebase.num as f64 / effective_timebase.den as f64
            } else {
                info.fps.max(1.0)
            };
            let duration_pts = (info.duration_seconds * fps_ratio).round() as i64;

            Ok(Self {
                handle,
                width: info.width,
                height: info.height,
                fps: info.fps,
                duration_seconds: info.duration_seconds,
                duration_pts,
                timebase: effective_timebase,
            })
        }
    }

    impl VideoDecoder for AvFoundationVideoDecoder {
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
            self.next_frame_with_buffer(Vec::new())
        }

        fn next_frame_with_buffer(&mut self, mut buffer: Vec<u8>) -> DecoderResult<Option<VideoFrame>> {
            let mut raw = std::mem::MaybeUninit::<AvfRawFrame>::uninit();
            let res = unsafe { avf_decoder_next_frame(self.handle, raw.as_mut_ptr()) };

            match res {
                0 => Ok(None), // End of stream
                1 => {
                    let mut raw = unsafe { raw.assume_init() };
                    let data = unsafe {
                        let slice = std::slice::from_raw_parts(raw.data, raw.data_len);
                        buffer.clear();
                        if buffer.capacity() < raw.data_len {
                            buffer.reserve(raw.data_len - buffer.capacity());
                        }
                        buffer.extend_from_slice(slice);
                        avf_decoder_free_frame(&mut raw);
                        buffer
                    };

                    let fps_ratio = if self.timebase.den != 0 && self.timebase.num > 0 {
                        self.timebase.num as f64 / self.timebase.den as f64
                    } else {
                        60.0
                    };

                    let sec = if raw.pts_timescale != 0 {
                        raw.pts_value as f64 / raw.pts_timescale as f64
                    } else {
                        0.0
                    };
                    let pts = (sec * fps_ratio).round() as i64;

                    let dur_sec = if raw.duration_timescale != 0 {
                        raw.duration_value as f64 / raw.duration_timescale as f64
                    } else {
                        1.0 / self.fps
                    };
                    let duration_pts = (dur_sec * fps_ratio).round().max(1.0) as i64;

                    let mut frame = VideoFrame {
                        width: raw.width,
                        height: raw.height,
                        format: PixelFormat::Bgra8,
                        data,
                        row_stride_bytes: raw.bytes_per_row,
                        pts,
                        duration_pts,
                        timebase: self.timebase,
                        is_keyframe: raw.is_keyframe != 0,
                    };
                    frame.ensure_rgba8();
                    Ok(Some(frame))
                }
                _ => Err(DecoderError::DecodeError(
                    "AVAssetReader failed decoding frame".to_string(),
                )),
            }
        }

        fn next_frame_at_or_after_with_buffer(
            &mut self,
            min_pts: i64,
            mut buffer: Vec<u8>,
        ) -> DecoderResult<Option<VideoFrame>> {
            let fps_ratio = if self.timebase.den != 0 && self.timebase.num > 0 {
                self.timebase.num as f64 / self.timebase.den as f64
            } else {
                60.0
            };
            let sec = (min_pts.max(0) as f64 / fps_ratio).max(0.0);
            let min_timescale = 600;
            let min_value = (sec * min_timescale as f64).round() as i64;

            let mut raw = std::mem::MaybeUninit::<AvfRawFrame>::uninit();
            let res = unsafe {
                avf_decoder_next_frame_at_or_after(
                    self.handle,
                    min_value,
                    min_timescale,
                    raw.as_mut_ptr(),
                )
            };

            match res {
                0 => Ok(None),
                1 => {
                    let mut raw = unsafe { raw.assume_init() };
                    let data = unsafe {
                        let slice = std::slice::from_raw_parts(raw.data, raw.data_len);
                        buffer.clear();
                        if buffer.capacity() < raw.data_len {
                            buffer.reserve(raw.data_len - buffer.capacity());
                        }
                        buffer.extend_from_slice(slice);
                        avf_decoder_free_frame(&mut raw);
                        buffer
                    };

                    let sec = if raw.pts_timescale != 0 {
                        raw.pts_value as f64 / raw.pts_timescale as f64
                    } else {
                        0.0
                    };
                    let pts = (sec * fps_ratio).round() as i64;

                    let dur_sec = if raw.duration_timescale != 0 {
                        raw.duration_value as f64 / raw.duration_timescale as f64
                    } else {
                        1.0 / self.fps
                    };
                    let duration_pts = (dur_sec * fps_ratio).round().max(1.0) as i64;

                    let mut frame = VideoFrame {
                        width: raw.width,
                        height: raw.height,
                        format: PixelFormat::Bgra8,
                        data,
                        row_stride_bytes: raw.bytes_per_row,
                        pts,
                        duration_pts,
                        timebase: self.timebase,
                        is_keyframe: raw.is_keyframe != 0,
                    };
                    frame.ensure_rgba8();
                    Ok(Some(frame))
                }
                _ => Err(DecoderError::DecodeError(
                    "AVAssetReader failed decoding frame".to_string(),
                )),
            }
        }

        fn seek_pts(&mut self, target_pts: i64) -> DecoderResult<()> {
            let clamped_pts = target_pts.max(0);
            let fps_ratio = if self.timebase.den != 0 && self.timebase.num > 0 {
                self.timebase.num as f64 / self.timebase.den as f64
            } else {
                60.0
            };
            let sec = clamped_pts as f64 / fps_ratio;
            let target_timescale = 600;
            let target_value = (sec * target_timescale as f64).round() as i64;

            let res = unsafe { avf_decoder_seek_pts(self.handle, target_value, target_timescale) };
            if res != 0 {
                return Err(DecoderError::SeekError(format!(
                    "AVAssetReader seek to PTS {} failed",
                    target_pts
                )));
            }
            Ok(())
        }
    }

    impl Drop for AvFoundationVideoDecoder {
        fn drop(&mut self) {
            if !self.handle.is_null() {
                unsafe { avf_decoder_destroy(self.handle) };
                self.handle = std::ptr::null_mut();
            }
        }
    }

    #[repr(C)]
    struct AvfAudioInfo {
        sample_rate: u32,
        channels: u16,
        duration_seconds: f64,
        duration_value: i64,
        duration_timescale: i32,
    }

    #[repr(C)]
    struct AvfRawAudioBuffer {
        sample_rate: u32,
        channels: u16,
        frame_count: u32,
        pts_value: i64,
        pts_timescale: i32,
        samples: *mut f32,
        sample_count: usize,
    }

    extern "C" {
        fn avf_audio_decoder_create(
            file_path: *const c_char,
            out_info: *mut AvfAudioInfo,
            err_buf: *mut c_char,
            err_len: usize,
        ) -> *mut c_void;
        fn avf_audio_decoder_next_buffer(
            handle: *mut c_void,
            out_buf: *mut AvfRawAudioBuffer,
        ) -> i32;
        fn avf_audio_decoder_free_buffer(buf: *mut AvfRawAudioBuffer);
        fn avf_audio_decoder_seek_pts(
            handle: *mut c_void,
            target_value: i64,
            target_timescale: i32,
        ) -> i32;
        fn avf_audio_decoder_destroy(handle: *mut c_void);
    }

    /// Native macOS audio decoder backed by AVFoundation `AVAssetReader` (32-bit float interleaved PCM).
    pub struct AvFoundationAudioDecoder {
        handle: *mut c_void,
        sample_rate: u32,
        channels: u16,
        duration_seconds: f64,
        duration_pts: i64,
        timebase: Rational,
    }

    unsafe impl Send for AvFoundationAudioDecoder {}
    unsafe impl Sync for AvFoundationAudioDecoder {}

    impl AvFoundationAudioDecoder {
        pub fn open(path: &str) -> DecoderResult<Self> {
            Self::open_with_timebase(path, crate::DEFAULT_TIMEBASE)
        }

        pub fn open_with_timebase(path: &str, timebase: Rational) -> DecoderResult<Self> {
            let p = Path::new(path);
            if !p.exists() {
                return Err(DecoderError::FileNotFound(path.to_string()));
            }
            if !p.is_file() {
                return Err(DecoderError::NotAFile(path.to_string()));
            }
            let file_size = std::fs::metadata(p)
                .map_err(|e| DecoderError::IoError(e.to_string()))?
                .len();
            if file_size == 0 {
                return Err(DecoderError::EmptyFile(path.to_string()));
            }

            let c_path = CString::new(path)
                .map_err(|e| DecoderError::InitializationFailed(e.to_string()))?;
            let mut info = std::mem::MaybeUninit::<AvfAudioInfo>::uninit();
            let mut err_buf = [0 as c_char; 512];

            let handle = unsafe {
                avf_audio_decoder_create(
                    c_path.as_ptr(),
                    info.as_mut_ptr(),
                    err_buf.as_mut_ptr(),
                    err_buf.len(),
                )
            };

            if handle.is_null() {
                let err_str = unsafe {
                    std::ffi::CStr::from_ptr(err_buf.as_ptr())
                        .to_string_lossy()
                        .to_string()
                };
                if err_str.contains("No audio track") {
                    return Err(DecoderError::NoAudioStream);
                }
                if err_str.contains("File does not exist") {
                    return Err(DecoderError::FileNotFound(path.to_string()));
                }
                return Err(DecoderError::InitializationFailed(err_str));
            }

            let info = unsafe { info.assume_init() };
            if info.sample_rate == 0 || info.channels == 0 {
                unsafe { avf_audio_decoder_destroy(handle) };
                return Err(DecoderError::CorruptData(
                    "AVFoundation audio track reported zero sample rate or channels".to_string(),
                ));
            }

            let fps_ratio = if timebase.den != 0 && timebase.num > 0 {
                timebase.num as f64 / timebase.den as f64
            } else {
                60.0
            };
            let duration_pts = (info.duration_seconds * fps_ratio).round() as i64;

            Ok(Self {
                handle,
                sample_rate: info.sample_rate,
                channels: info.channels,
                duration_seconds: info.duration_seconds,
                duration_pts,
                timebase,
            })
        }
    }

    impl crate::decoder::traits::AudioDecoder for AvFoundationAudioDecoder {
        fn sample_rate(&self) -> u32 {
            self.sample_rate
        }

        fn channels(&self) -> u16 {
            self.channels
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

        fn format(&self) -> crate::decoder::types::AudioSampleFormat {
            crate::decoder::types::AudioSampleFormat::F32Interleaved
        }

        fn next_audio_buffer(&mut self) -> DecoderResult<Option<crate::decoder::types::AudioBuffer>> {
            let mut raw = std::mem::MaybeUninit::<AvfRawAudioBuffer>::uninit();
            let res = unsafe { avf_audio_decoder_next_buffer(self.handle, raw.as_mut_ptr()) };

            match res {
                0 => Ok(None),
                1 => {
                    let mut raw = unsafe { raw.assume_init() };
                    let samples = unsafe {
                        let slice = std::slice::from_raw_parts(raw.samples, raw.sample_count);
                        let vec = slice.to_vec();
                        avf_audio_decoder_free_buffer(&mut raw);
                        vec
                    };

                    let fps_ratio = if self.timebase.den != 0 && self.timebase.num > 0 {
                        self.timebase.num as f64 / self.timebase.den as f64
                    } else {
                        60.0
                    };
                    let sec = if raw.pts_timescale != 0 {
                        raw.pts_value as f64 / raw.pts_timescale as f64
                    } else {
                        0.0
                    };
                    let pts = (sec * fps_ratio).round() as i64;
                    let dur_sec = if self.sample_rate > 0 {
                        raw.frame_count as f64 / self.sample_rate as f64
                    } else {
                        0.0
                    };
                    let duration_pts = (dur_sec * fps_ratio).round().max(1.0) as i64;

                    Ok(Some(crate::decoder::types::AudioBuffer::new(
                        raw.sample_rate,
                        raw.channels,
                        samples,
                        pts,
                        duration_pts,
                        self.timebase,
                    )))
                }
                _ => Err(DecoderError::DecodeError(
                    "AVAssetReader failed decoding audio buffer".to_string(),
                )),
            }
        }

        fn seek_pts(&mut self, target_pts: i64) -> DecoderResult<()> {
            let clamped = target_pts.max(0);
            let fps_ratio = if self.timebase.den != 0 && self.timebase.num > 0 {
                self.timebase.num as f64 / self.timebase.den as f64
            } else {
                60.0
            };
            let sec = clamped as f64 / fps_ratio;
            let target_timescale = self.sample_rate.max(600) as i32;
            let target_value = (sec * target_timescale as f64).round() as i64;

            let res =
                unsafe { avf_audio_decoder_seek_pts(self.handle, target_value, target_timescale) };
            if res != 0 {
                return Err(DecoderError::SeekError(format!(
                    "AVAssetReader audio seek to PTS {} failed",
                    target_pts
                )));
            }
            Ok(())
        }
    }

    impl Drop for AvFoundationAudioDecoder {
        fn drop(&mut self) {
            if !self.handle.is_null() {
                unsafe { avf_audio_decoder_destroy(self.handle) };
                self.handle = std::ptr::null_mut();
            }
        }
    }
}

#[cfg(target_os = "macos")]
pub use native::{AvFoundationAudioDecoder, AvFoundationVideoDecoder};
