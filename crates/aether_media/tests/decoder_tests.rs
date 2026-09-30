mod fixtures;

use aether_core::media::MediaType;
use aether_core::timeline::Rational;
use aether_media::decoder::{
    open_audio_decoder, open_media_decoder, open_video_decoder, AudioBuffer, AudioDecoder,
    AudioSampleFormat, DecoderError, PixelFormat, SymphoniaAudioDecoder, VideoFrame,
};
use std::fs;
use tempfile::tempdir;

#[test]
#[cfg(target_os = "macos")]
fn test_video_frame_extraction_sequential() {
    let mp4_path = fixtures::ensure_sample_mp4();
    let path_str = mp4_path.to_str().expect("Valid path");

    let mut video_dec = open_video_decoder(path_str).expect("Failed to open test MP4 video decoder");

    assert_eq!(video_dec.width(), 64);
    assert_eq!(video_dec.height(), 64);
    assert!((video_dec.fps() - 10.0).abs() < 1.0);
    assert!(video_dec.duration_seconds() > 0.8 && video_dec.duration_seconds() < 1.2);

    let mut frame_count = 0;
    let mut prev_pts: Option<i64> = None;

    while let Some(mut frame) = video_dec.next_frame().expect("Frame decoding error") {
        frame_count += 1;

        assert_eq!(frame.width, 64);
        assert_eq!(frame.height, 64);

        // Ensure RGBA8 buffer normalization
        frame.ensure_rgba8();
        assert_eq!(frame.format, PixelFormat::Rgba8);
        assert!(frame.data.len() >= 64 * 64 * 4);

        // Verify strictly ascending PTS
        if let Some(prev) = prev_pts {
            assert!(
                frame.pts > prev,
                "Frame {} PTS ({}) not greater than previous PTS ({})",
                frame_count,
                frame.pts,
                prev
            );
        }
        prev_pts = Some(frame.pts);

        // Verify timestamp seconds are within bounds [0.0, 1.1]
        let sec = frame.timestamp_seconds();
        assert!(
            sec >= 0.0 && sec <= 1.1,
            "Frame {} timestamp out of bounds: {}",
            frame_count,
            sec
        );
    }

    assert_eq!(
        frame_count, 10,
        "Expected exactly 10 frames from sample_64x64_10frames.mp4"
    );

    // Call after EOF should return Ok(None) cleanly
    let eof_call = video_dec.next_frame().expect("EOF call failed");
    assert!(eof_call.is_none(), "Expected Ok(None) on stream termination");
}

#[test]
#[cfg(target_os = "macos")]
fn test_video_frame_seek_to_pts() {
    let mp4_path = fixtures::ensure_sample_mp4();
    let path_str = mp4_path.to_str().expect("Valid path");

    let mut video_dec = open_video_decoder(path_str).expect("Failed to open video decoder");

    // Seek to middle (0.5 seconds)
    video_dec.seek_second(0.5).expect("Seek to 0.5s failed");

    let frame = video_dec
        .next_frame()
        .expect("Decode after seek failed")
        .expect("Expected a frame after seeking to 0.5s");

    assert_eq!(frame.width, 64);
    assert_eq!(frame.height, 64);
    let sec = frame.timestamp_seconds();
    assert!(
        (sec - 0.5).abs() < 0.2,
        "Expected frame near 0.5s, got: {}",
        sec
    );

    // Seek back to 0.0 seconds
    video_dec.seek_second(0.0).expect("Seek to 0.0s failed");
    let frame_start = video_dec
        .next_frame()
        .expect("Decode after rewind failed")
        .expect("Expected frame at start");
    assert!(
        frame_start.timestamp_seconds() < 0.15,
        "Expected frame at start near 0.0s, got: {}",
        frame_start.timestamp_seconds()
    );
}

#[test]
#[cfg(target_os = "macos")]
fn test_video_extract_frame_at_pts_convenience() {
    let mp4_path = fixtures::ensure_sample_mp4();
    let path_str = mp4_path.to_str().expect("Valid path");

    let mut video_dec = open_video_decoder(path_str).expect("Failed to open video decoder");

    let fps = video_dec.fps();
    let target_pts = (0.3 * fps).round() as i64;

    let frame = video_dec
        .extract_frame_at_pts(target_pts)
        .expect("Extract frame failed")
        .expect("Expected frame at target PTS");

    assert_eq!(frame.width, 64);
    assert_eq!(frame.height, 64);
    assert!((frame.timestamp_seconds() - 0.3).abs() < 0.2);
}

#[test]
fn test_symphonia_audio_decoding_wav() {
    let dir = tempdir().unwrap();
    let wav_path = dir.path().join("test_tone_44100.wav");

    // Generate 44.1 kHz, Stereo, 1.0 second synthetic WAV
    let wav_bytes = fixtures::create_synthetic_wav(44100, 2, 1.0);
    fs::write(&wav_path, wav_bytes).unwrap();

    let path_str = wav_path.to_str().unwrap();
    let mut audio_dec = SymphoniaAudioDecoder::open(path_str).expect("Failed to open WAV audio decoder");

    assert_eq!(audio_dec.sample_rate(), 44100);
    assert_eq!(audio_dec.channels(), 2);
    assert_eq!(audio_dec.format(), AudioSampleFormat::F32Interleaved);
    assert!((audio_dec.duration_seconds() - 1.0).abs() < 0.05);

    let mut total_samples = 0;
    let mut buffer_count = 0;

    while let Some(buffer) = audio_dec.next_audio_buffer().expect("Audio decode error") {
        buffer_count += 1;
        assert_eq!(buffer.sample_rate, 44100);
        assert_eq!(buffer.channels, 2);
        assert!(!buffer.samples_f32.is_empty());
        assert_eq!(buffer.samples_f32.len() % 2, 0, "Stereo samples must be even");

        // Validate sample numbers are finite floats in [-1.0, 1.0]
        for &s in &buffer.samples_f32 {
            assert!(s.is_finite());
            assert!(s >= -1.0 && s <= 1.0, "Sample out of bounds: {}", s);
        }

        total_samples += buffer.samples_f32.len();
    }

    assert!(buffer_count > 0);
    // 44100 samples per channel * 2 channels = 88200 samples
    assert_eq!(total_samples, 88200);

    // Call after EOF returns Ok(None)
    let eof = audio_dec.next_audio_buffer().expect("EOF check");
    assert!(eof.is_none());
}

#[test]
fn test_symphonia_audio_seek() {
    let dir = tempdir().unwrap();
    let wav_path = dir.path().join("seekable_audio.wav");

    // 48 kHz, Stereo, 2.0 seconds
    let wav_bytes = fixtures::create_synthetic_wav(48000, 2, 2.0);
    fs::write(&wav_path, wav_bytes).unwrap();

    let path_str = wav_path.to_str().unwrap();
    let mut audio_dec = SymphoniaAudioDecoder::open(path_str).expect("Failed to open audio decoder");

    // Seek to 1.0 second
    audio_dec.seek_second(1.0).expect("Seek to 1.0s failed");

    let buf = audio_dec
        .next_audio_buffer()
        .expect("Decode after seek")
        .expect("Expected buffer after seek");

    let sec = buf.timestamp_seconds();
    assert!(
        (sec - 1.0).abs() < 0.1,
        "Expected buffer timestamp near 1.0s, got: {}",
        sec
    );

    // Seek back to 0.0s
    audio_dec.seek_second(0.0).expect("Seek to 0.0s failed");
    let buf_start = audio_dec
        .next_audio_buffer()
        .expect("Decode after rewind")
        .expect("Expected buffer at start");
    assert_eq!(buf_start.pts, 0);
}

#[test]
fn test_image_decoder_as_video_stream() {
    let dir = tempdir().unwrap();
    let png_path = dir.path().join("slide.png");

    let png_bytes = fixtures::create_synthetic_png(320, 240);
    fs::write(&png_path, png_bytes).unwrap();

    let path_str = png_path.to_str().unwrap();
    let mut media = open_media_decoder(path_str).expect("Failed to open image as media decoder");

    assert_eq!(media.media_type(), MediaType::Image);
    let video = media.video().expect("Image must expose video decoder interface");
    assert_eq!(video.width(), 320);
    assert_eq!(video.height(), 240);
    assert_eq!(video.duration_pts(), 300); // 5.0s @ 60 FPS
    assert!((video.duration_seconds() - 5.0).abs() < 0.01);

    // Decode frame
    let frame = video.next_frame().expect("Frame decode").expect("Image frame");
    assert_eq!(frame.width, 320);
    assert_eq!(frame.height, 240);
    assert_eq!(frame.pts, 0);
    assert_eq!(frame.duration_pts, 300);
    assert_eq!(frame.data.len(), 320 * 240 * 4);
}

#[test]
fn test_synthetic_decoder_procedural_playback() {
    // 64x64, 30 fps, 1.0 second duration = 30 frames
    let syn_uri = "synthetic://test?width=64&height=64&fps=30&duration=1.0";
    let mut video_dec = open_video_decoder(syn_uri).expect("Failed to open synthetic decoder");

    assert_eq!(video_dec.width(), 64);
    assert_eq!(video_dec.height(), 64);
    assert_eq!(video_dec.fps(), 30.0);
    assert_eq!(video_dec.duration_pts(), 60); // In 60 FPS timeline units

    let mut frames_decoded = 0;
    while let Some(frame) = video_dec.next_frame().expect("Synthetic decode") {
        frames_decoded += 1;
        assert_eq!(frame.width, 64);
        assert_eq!(frame.height, 64);
        assert_eq!(frame.data.len(), 64 * 64 * 4);
    }
    assert_eq!(frames_decoded, 30);
}

#[test]
fn test_synthetic_decoder_seek_bounds() {
    let syn_uri = "synthetic://test?width=64&height=64&fps=30&duration=2.0";
    let mut video_dec = open_video_decoder(syn_uri).expect("Open synthetic decoder");

    // Seek to frame 15 (0.5s)
    video_dec.seek_second(0.5).expect("Seek 0.5s");
    let f1 = video_dec.next_frame().unwrap().unwrap();
    assert!((f1.timestamp_seconds() - 0.5).abs() < 0.05);

    // Negative PTS seek clamps to 0
    video_dec.seek_pts(-100).expect("Negative seek should clamp safely");
    let f_start = video_dec.next_frame().unwrap().unwrap();
    assert_eq!(f_start.pts, 0);

    // Seek past duration returns EOF on next frame
    video_dec.seek_pts(99999).expect("Past duration seek");
    let past = video_dec.next_frame().unwrap();
    assert!(past.is_none());
}

#[test]
#[cfg(target_os = "macos")]
fn test_unified_factory_dispatch_and_composite() {
    let mp4_path = fixtures::ensure_sample_mp4();
    let mut media = open_media_decoder(mp4_path.to_str().unwrap()).expect("Open MP4");
    assert_eq!(media.media_type(), MediaType::Video);
    assert!(media.video().is_some());

    // Synchronized seek
    media.seek_pts(30).expect("Media seek failed");
    let vframe = media.next_video_frame().unwrap().unwrap();
    assert!(vframe.pts >= 0);
}

#[test]
fn test_open_audio_decoder_standalone() {
    let dir = tempdir().unwrap();
    let wav_path = dir.path().join("standalone.wav");
    let wav_bytes = fixtures::create_synthetic_wav(44100, 2, 0.5);
    fs::write(&wav_path, wav_bytes).unwrap();

    let mut audio_dec = open_audio_decoder(wav_path.to_str().unwrap()).expect("Open audio decoder");
    assert_eq!(audio_dec.sample_rate(), 44100);
    assert_eq!(audio_dec.channels(), 2);
    let buf = audio_dec.next_audio_buffer().unwrap().unwrap();
    assert!(!buf.samples_f32.is_empty());
}

#[test]
fn test_error_handling_invalid_inputs() {
    // 1. Non-existent file
    let res = open_media_decoder("/tmp/definitely_not_a_real_file_12345.mp4");
    assert!(matches!(res.err(), Some(DecoderError::FileNotFound(_))));

    // 2. Directory path
    let dir = tempdir().unwrap();
    let res_dir = open_media_decoder(dir.path().to_str().unwrap());
    assert!(matches!(res_dir.err(), Some(DecoderError::NotAFile(_))));

    // 3. 0-byte file
    let empty_file = dir.path().join("zero.mp4");
    fs::write(&empty_file, b"").unwrap();
    let res_empty = open_media_decoder(empty_file.to_str().unwrap());
    assert!(matches!(res_empty.err(), Some(DecoderError::EmptyFile(_))));
}

#[test]
fn test_video_frame_creation_and_properties() {
    let data = vec![255, 0, 0, 255]; // 1x1 red RGBA pixel
    let frame = VideoFrame::new(
        1,
        1,
        PixelFormat::Rgba8,
        data,
        60,
        1,
        Rational { num: 60, den: 1 },
    );

    assert_eq!(frame.width, 1);
    assert_eq!(frame.height, 1);
    assert_eq!(frame.format, PixelFormat::Rgba8);
    assert_eq!(frame.row_stride_bytes, 4);
    assert_eq!(frame.pts, 60);
    assert_eq!(frame.duration_pts, 1);
    assert_eq!(frame.fps(), 60.0);
    assert_eq!(frame.timestamp_seconds(), 1.0);
    assert_eq!(frame.duration_seconds(), 1.0 / 60.0);
    assert_eq!(frame.byte_len(), 4);
}

#[test]
fn test_video_frame_ensure_rgba8_from_bgra8() {
    // BGRA pixel: Blue=200, Green=100, Red=50, Alpha=255
    let mut frame = VideoFrame::new(
        1,
        1,
        PixelFormat::Bgra8,
        vec![200, 100, 50, 255],
        0,
        1,
        Rational { num: 60, den: 1 },
    );

    assert_eq!(frame.format, PixelFormat::Bgra8);
    frame.ensure_rgba8();

    assert_eq!(frame.format, PixelFormat::Rgba8);
    // After byte swap (0 and 2): Red=50, Green=100, Blue=200, Alpha=255
    assert_eq!(frame.data, vec![50, 100, 200, 255]);
}

#[test]
fn test_video_frame_ensure_rgba8_with_padding_stride() {
    // 2x1 BGRA image with row stride of 16 bytes (8 bytes padding)
    let mut data = vec![0u8; 16];
    // Pixel 0: Blue=1, Green=2, Red=3, Alpha=255
    data[0] = 1;
    data[1] = 2;
    data[2] = 3;
    data[3] = 255;
    // Pixel 1: Blue=10, Green=20, Red=30, Alpha=255
    data[4] = 10;
    data[5] = 20;
    data[6] = 30;
    data[7] = 255;
    // Padding bytes: data[8..16] = 99
    data[8..16].fill(99);

    let mut frame = VideoFrame::with_stride(
        2,
        1,
        PixelFormat::Bgra8,
        data,
        16,
        0,
        1,
        Rational { num: 60, den: 1 },
        true,
    );

    frame.ensure_rgba8();

    assert_eq!(frame.format, PixelFormat::Rgba8);
    // Pixel 0 swapped
    assert_eq!(frame.data[0..4], [3, 2, 1, 255]);
    // Pixel 1 swapped
    assert_eq!(frame.data[4..8], [30, 20, 10, 255]);
    // Padding preserved
    assert_eq!(frame.data[8..16], [99, 99, 99, 99, 99, 99, 99, 99]);
}

#[test]
fn test_video_frame_ensure_rgba8_from_rgb8() {
    // 1x2 RGB image (3 bytes per pixel)
    let rgb_data = vec![
        10, 20, 30, // Row 0
        40, 50, 60, // Row 1
    ];
    let mut frame = VideoFrame::new(
        1,
        2,
        PixelFormat::Rgb8,
        rgb_data,
        0,
        1,
        Rational { num: 60, den: 1 },
    );

    assert_eq!(frame.format, PixelFormat::Rgb8);
    assert_eq!(frame.row_stride_bytes, 3);

    frame.ensure_rgba8();

    assert_eq!(frame.format, PixelFormat::Rgba8);
    assert_eq!(frame.row_stride_bytes, 4);
    assert_eq!(frame.data, vec![10, 20, 30, 255, 40, 50, 60, 255]);
}

#[test]
fn test_audio_buffer_creation_and_properties() {
    let samples = vec![0.0f32, 0.5f32, -0.5f32, 1.0f32]; // 2 frames stereo
    let audio = AudioBuffer::new(
        48000,
        2,
        samples,
        120,
        2,
        Rational { num: 60, den: 1 },
    );

    assert_eq!(audio.sample_rate, 48000);
    assert_eq!(audio.channels, 2);
    assert_eq!(audio.format, AudioSampleFormat::F32Interleaved);
    assert_eq!(audio.sample_count(), 4);
    assert_eq!(audio.frame_count(), 2);
    assert_eq!(audio.timestamp_seconds(), 2.0); // 120 PTS / 60 FPS = 2.0s
    assert_eq!(audio.duration_seconds(), 2.0 / 48000.0);
}

#[test]
fn test_decoder_error_conversions() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "test_file.mp4");
    let dec_err: DecoderError = io_err.into();
    assert!(matches!(dec_err, DecoderError::FileNotFound(_)));

    let media_err = aether_media::MediaError::EmptyFile("empty.mp4".to_string());
    let dec_err2: DecoderError = media_err.into();
    assert_eq!(dec_err2, DecoderError::EmptyFile("empty.mp4".to_string()));
}
