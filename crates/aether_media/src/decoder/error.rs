use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DecoderError {
    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Path is not a regular file: {0}")]
    NotAFile(String),

    #[error("File is empty (0 bytes): {0}")]
    EmptyFile(String),

    #[error("Unsupported codec: {0}")]
    UnsupportedCodec(String),

    #[error("Unsupported container format: {0}")]
    UnsupportedContainer(String),

    #[error("Decoder initialization failed: {0}")]
    InitializationFailed(String),

    #[error("Demuxer error: {0}")]
    DemuxError(String),

    #[error("Frame decode error: {0}")]
    DecodeError(String),

    #[error("Seek error: {0}")]
    SeekError(String),

    #[error("End of stream reached")]
    EndOfStream,

    #[error("No video stream present in media asset")]
    NoVideoStream,

    #[error("No audio stream present in media asset")]
    NoAudioStream,

    #[error("Decoder backend unavailable: {0}")]
    BackendUnavailable(String),

    #[error("Corrupt media data: {0}")]
    CorruptData(String),

    #[error("I/O error: {0}")]
    IoError(String),
}

impl From<std::io::Error> for DecoderError {
    fn from(err: std::io::Error) -> Self {
        match err.kind() {
            std::io::ErrorKind::NotFound => DecoderError::FileNotFound(err.to_string()),
            _ => DecoderError::IoError(err.to_string()),
        }
    }
}

impl From<crate::MediaError> for DecoderError {
    fn from(err: crate::MediaError) -> Self {
        match err {
            crate::MediaError::FileNotFound(s) => DecoderError::FileNotFound(s),
            crate::MediaError::NotAFile(s) => DecoderError::NotAFile(s),
            crate::MediaError::EmptyFile(s) => DecoderError::EmptyFile(s),
            crate::MediaError::UnsupportedFormat(s) => DecoderError::UnsupportedContainer(s),
            crate::MediaError::CorruptFile(s) => DecoderError::CorruptData(s),
            crate::MediaError::IoError(s) => DecoderError::IoError(s),
        }
    }
}
