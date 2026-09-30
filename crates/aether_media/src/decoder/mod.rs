pub mod error;
pub mod factory;
pub mod image_decoder;
pub mod symphonia_audio;
pub mod synthetic;
pub mod traits;
pub mod types;

#[cfg(target_os = "macos")]
pub mod avfoundation;

pub use error::DecoderError;
pub use factory::{
    open_audio_decoder, open_audio_decoder_with_timebase, open_media_decoder,
    open_media_decoder_with_timebase, open_video_decoder, open_video_decoder_with_timebase,
    CompositeMediaDecoder,
};
pub use image_decoder::ImageSequenceDecoder;
pub use symphonia_audio::SymphoniaAudioDecoder;
pub use synthetic::{SyntheticPattern, SyntheticVideoDecoder};
pub use traits::{AudioDecoder, DecoderResult, MediaDecoder, VideoDecoder};
pub use types::{AudioBuffer, AudioSampleFormat, PixelFormat, VideoFrame};

#[cfg(target_os = "macos")]
pub use avfoundation::AvFoundationVideoDecoder;
