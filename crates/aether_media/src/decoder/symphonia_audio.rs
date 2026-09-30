use std::fs::File;
use std::path::Path;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{Decoder, DecoderOptions};
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use super::error::DecoderError;
use super::traits::{AudioDecoder, DecoderResult};
use super::types::{AudioBuffer, AudioSampleFormat};
use aether_core::timeline::Rational;

/// Cross-platform audio decoder powered by Symphonia.
/// Supports uncompressed PCM decoding for WAV, MP3, AAC, FLAC, Vorbis, and OGG containers.
pub struct SymphoniaAudioDecoder {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn Decoder>,
    track_id: u32,
    sample_rate: u32,
    channels: u16,
    timebase: Rational,
    duration_pts: i64,
    duration_seconds: f64,
    current_frame_pos: u64,
    sample_buf: Option<SampleBuffer<f32>>,
}

// Symphonia's FormatReader and Decoder traits are Send + Sync
unsafe impl Send for SymphoniaAudioDecoder {}
unsafe impl Sync for SymphoniaAudioDecoder {}

impl SymphoniaAudioDecoder {
    /// Opens an audio file using the default 60 FPS timeline timebase.
    pub fn open(path: &str) -> DecoderResult<Self> {
        Self::open_with_timebase(path, crate::DEFAULT_TIMEBASE)
    }

    /// Opens an audio file with an explicit timeline timebase for PTS synchronization.
    pub fn open_with_timebase(path_str: &str, timebase: Rational) -> DecoderResult<Self> {
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

        let file = File::open(path).map_err(|e| DecoderError::IoError(e.to_string()))?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());

        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
            hint.with_extension(ext);
        }

        let format_opts = FormatOptions::default();
        let metadata_opts = MetadataOptions::default();

        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &format_opts, &metadata_opts)
            .map_err(|e| DecoderError::DemuxError(e.to_string()))?;

        let format = probed.format;

        // Find the first audio track with a valid codec and sample rate
        let track = format
            .tracks()
            .iter()
            .find(|t| {
                t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL
                    && t.codec_params.sample_rate.is_some()
            })
            .or_else(|| format.default_track())
            .ok_or(DecoderError::NoAudioStream)?;

        let track_id = track.id;
        let codec_params = track.codec_params.clone();

        let sample_rate = codec_params
            .sample_rate
            .ok_or_else(|| DecoderError::CorruptData("Audio track missing sample rate".to_string()))?;

        if sample_rate == 0 {
            return Err(DecoderError::CorruptData("Audio sample rate is 0".to_string()));
        }

        let channels = codec_params
            .channels
            .map(|c| c.count() as u16)
            .unwrap_or(2);

        let duration_seconds = if let (Some(n_frames), Some(rate)) = (codec_params.n_frames, codec_params.sample_rate) {
            if rate > 0 {
                n_frames as f64 / rate as f64
            } else {
                0.0
            }
        } else if let (Some(tb), Some(n_frames)) = (codec_params.time_base, codec_params.n_frames) {
            let time = tb.calc_time(n_frames);
            time.seconds as f64 + time.frac
        } else {
            0.0
        };

        let fps = if timebase.den != 0 && timebase.num > 0 {
            timebase.num as f64 / timebase.den as f64
        } else {
            60.0
        };
        let duration_pts = (duration_seconds * fps).round() as i64;

        let decoder_opts = DecoderOptions::default();
        let decoder = symphonia::default::get_codecs()
            .make(&codec_params, &decoder_opts)
            .map_err(|e| DecoderError::InitializationFailed(e.to_string()))?;

        Ok(Self {
            format,
            decoder,
            track_id,
            sample_rate,
            channels,
            timebase,
            duration_pts,
            duration_seconds,
            current_frame_pos: 0,
            sample_buf: None,
        })
    }
}

impl AudioDecoder for SymphoniaAudioDecoder {
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

    fn format(&self) -> AudioSampleFormat {
        AudioSampleFormat::F32Interleaved
    }

    fn next_audio_buffer(&mut self) -> DecoderResult<Option<AudioBuffer>> {
        loop {
            let packet = match self.format.next_packet() {
                Ok(packet) => packet,
                Err(symphonia::core::errors::Error::IoError(ref e))
                    if e.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    return Ok(None);
                }
                Err(symphonia::core::errors::Error::ResetRequired) => {
                    return Err(DecoderError::DemuxError("Stream reset required".to_string()));
                }
                Err(e) => {
                    // Check if it's end of stream
                    let err_msg = e.to_string();
                    if err_msg.contains("end of file") || err_msg.contains("unexpected end") {
                        return Ok(None);
                    }
                    return Err(DecoderError::DemuxError(err_msg));
                }
            };

            if packet.track_id() != self.track_id {
                continue;
            }

            match self.decoder.decode(&packet) {
                Ok(decoded) => {
                    let spec = *decoded.spec();
                    let channels = spec.channels.count() as u16;
                    let num_frames = decoded.frames();

                    if num_frames == 0 {
                        continue;
                    }

                    // Re-use sample buffer if capacity permits, otherwise allocate new
                    let needs_new_buf = match &self.sample_buf {
                        Some(buf) => buf.capacity() < decoded.capacity(),
                        None => true,
                    };
                    if needs_new_buf {
                        self.sample_buf =
                            Some(SampleBuffer::<f32>::new(decoded.capacity() as u64, spec));
                    }

                    let sbuf = self.sample_buf.as_mut().unwrap();
                    sbuf.copy_interleaved_ref(decoded);
                    let samples_f32 = sbuf.samples().to_vec();

                    let fps = if self.timebase.den != 0 && self.timebase.num > 0 {
                        self.timebase.num as f64 / self.timebase.den as f64
                    } else {
                        60.0
                    };

                    let start_sec = self.current_frame_pos as f64 / self.sample_rate as f64;
                    let pts = (start_sec * fps).round() as i64;
                    let dur_sec = num_frames as f64 / self.sample_rate as f64;
                    let duration_pts = (dur_sec * fps).round().max(1.0) as i64;

                    self.current_frame_pos += num_frames as u64;

                    return Ok(Some(AudioBuffer::new(
                        self.sample_rate,
                        channels,
                        samples_f32,
                        pts,
                        duration_pts,
                        self.timebase,
                    )));
                }
                Err(symphonia::core::errors::Error::DecodeError(_)) => {
                    // Soft error on corrupted packet: skip packet and proceed to next
                    continue;
                }
                Err(e) => {
                    return Err(DecoderError::DecodeError(e.to_string()));
                }
            }
        }
    }

    fn seek_pts(&mut self, target_pts: i64) -> DecoderResult<()> {
        let target_pts = target_pts.max(0);
        let fps = if self.timebase.den != 0 && self.timebase.num > 0 {
            self.timebase.num as f64 / self.timebase.den as f64
        } else {
            60.0
        };
        let target_seconds = target_pts as f64 / fps;
        let target_frame = (target_seconds * self.sample_rate as f64).round() as u64;

        let seeked = self
            .format
            .seek(
                SeekMode::Accurate,
                SeekTo::TimeStamp {
                    ts: target_frame,
                    track_id: self.track_id,
                },
            )
            .map_err(|e| DecoderError::SeekError(e.to_string()))?;

        self.decoder.reset();
        self.current_frame_pos = seeked.actual_ts;
        Ok(())
    }
}
