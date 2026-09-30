use super::error::DecoderError;
use super::types::{AudioBuffer, AudioSampleFormat, PixelFormat, VideoFrame};
use aether_core::media::{MediaMetadata, MediaType};
use aether_core::timeline::Rational;

pub type DecoderResult<T> = Result<T, DecoderError>;

/// Abstract interface for video stream extraction, playback iteration, and seeking.
pub trait VideoDecoder: Send + Sync {
    /// Video width in pixels.
    fn width(&self) -> u32;

    /// Video height in pixels.
    fn height(&self) -> u32;

    /// Stream timebase.
    fn timebase(&self) -> Rational;

    /// Total duration in PTS ticks.
    fn duration_pts(&self) -> i64;

    /// Total duration in fractional seconds.
    fn duration_seconds(&self) -> f64;

    /// Frames-per-second rate.
    fn fps(&self) -> f64;

    /// Native pixel format output by this decoder.
    fn pixel_format(&self) -> PixelFormat;

    /// Decodes the next sequential frame (playback iteration).
    ///
    /// Returns `Ok(Some(frame))` when a frame is decoded.
    /// Returns `Ok(None)` when the end of stream (EOS) is reached.
    fn next_frame(&mut self) -> DecoderResult<Option<VideoFrame>>;

    /// Seeks the video stream to the target presentation timestamp (random access).
    fn seek_pts(&mut self, target_pts: i64) -> DecoderResult<()>;

    /// Convenience seeking to target fractional seconds.
    fn seek_second(&mut self, seconds: f64) -> DecoderResult<()> {
        let tb = self.timebase();
        let fps = if tb.den != 0 && tb.num > 0 {
            tb.num as f64 / tb.den as f64
        } else {
            self.fps()
        };
        let target_pts = if fps > 0.0 {
            (seconds * fps).round() as i64
        } else {
            0
        };
        self.seek_pts(target_pts)
    }

    /// Alias for `seek_second` for ergonomic flexibility.
    fn seek_seconds(&mut self, seconds: f64) -> DecoderResult<()> {
        self.seek_second(seconds)
    }

    /// Timeline scrubber convenience: seek to target PTS and decode a single frame.
    fn extract_frame_at_pts(&mut self, target_pts: i64) -> DecoderResult<Option<VideoFrame>> {
        self.seek_pts(target_pts)?;
        self.next_frame()
    }
}

/// Abstract interface for audio stream extraction and seeking.
pub trait AudioDecoder: Send + Sync {
    /// Audio sampling rate in Hertz (e.g. 44100, 48000).
    fn sample_rate(&self) -> u32;

    /// Number of audio channels (e.g. 1 for mono, 2 for stereo).
    fn channels(&self) -> u16;

    /// Stream timebase.
    fn timebase(&self) -> Rational;

    /// Total duration in PTS ticks.
    fn duration_pts(&self) -> i64;

    /// Total duration in fractional seconds.
    fn duration_seconds(&self) -> f64;

    /// Output PCM sample format.
    fn format(&self) -> AudioSampleFormat;

    /// Decodes the next chunk of audio samples into an uncompressed buffer.
    ///
    /// Returns `Ok(Some(buffer))` when decoded.
    /// Returns `Ok(None)` on end of stream (EOS).
    fn next_audio_buffer(&mut self) -> DecoderResult<Option<AudioBuffer>>;

    /// Seeks the audio stream to target presentation timestamp.
    fn seek_pts(&mut self, target_pts: i64) -> DecoderResult<()>;

    /// Convenience seeking to target fractional seconds.
    fn seek_second(&mut self, seconds: f64) -> DecoderResult<()> {
        let tb = self.timebase();
        let fps = if tb.den != 0 && tb.num > 0 {
            tb.num as f64 / tb.den as f64
        } else {
            60.0
        };
        let target_pts = (seconds * fps).round() as i64;
        self.seek_pts(target_pts)
    }

    /// Alias for `seek_second` for ergonomic flexibility.
    fn seek_seconds(&mut self, seconds: f64) -> DecoderResult<()> {
        self.seek_second(seconds)
    }
}

/// Unified media decoder facade providing access to underlying video and audio decoders.
pub trait MediaDecoder: Send + Sync {
    /// Static technical metadata of the opened media container.
    fn metadata(&self) -> &MediaMetadata;

    /// Primary media asset type (Video, Audio, Image).
    fn media_type(&self) -> MediaType;

    /// Mutable reference to the video decoder, if video is present.
    fn video(&mut self) -> Option<&mut (dyn VideoDecoder + '_)>;

    /// Mutable reference to the audio decoder, if audio is present.
    fn audio(&mut self) -> Option<&mut (dyn AudioDecoder + '_)>;

    /// Synchronously seeks all active streams to target PTS.
    fn seek_pts(&mut self, target_pts: i64) -> DecoderResult<()>;

    /// Synchronously seeks all active streams to target fractional seconds.
    fn seek_second(&mut self, seconds: f64) -> DecoderResult<()> {
        let fps = self.metadata().fps().unwrap_or(60.0);
        let target_pts = (seconds * fps).round() as i64;
        self.seek_pts(target_pts)
    }

    /// Alias for `seek_second`.
    fn seek_seconds(&mut self, seconds: f64) -> DecoderResult<()> {
        self.seek_second(seconds)
    }

    /// Convenience method to decode the next video frame.
    fn next_video_frame(&mut self) -> DecoderResult<Option<VideoFrame>> {
        match self.video() {
            Some(v) => v.next_frame(),
            None => Err(DecoderError::NoVideoStream),
        }
    }

    /// Convenience method to decode the next audio buffer.
    fn next_audio_buffer(&mut self) -> DecoderResult<Option<AudioBuffer>> {
        match self.audio() {
            Some(a) => a.next_audio_buffer(),
            None => Err(DecoderError::NoAudioStream),
        }
    }
}
