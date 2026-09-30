use std::fs;
use std::io::Cursor;
use std::panic::{catch_unwind, AssertUnwindSafe};
use tempfile::tempdir;

use aether_core::media::MediaType;
use aether_core::timeline::Rational;
use aether_media::decoder::{
    open_audio_decoder, open_media_decoder, open_video_decoder, AudioDecoder, AudioSampleFormat,
    DecoderError, ImageSequenceDecoder, PixelFormat, SymphoniaAudioDecoder, VideoDecoder,
};

/// Helper to generate a valid synthetic 16-bit PCM RIFF/WAVE audio file in memory.
fn create_synthetic_wav(sample_rate: u32, channels: u16, duration_seconds: f64) -> Vec<u8> {
    let bits_per_sample: u16 = 16;
    let num_frames = (sample_rate as f64 * duration_seconds).round() as usize;
    let block_align = channels * (bits_per_sample / 8);
    let byte_rate = sample_rate * block_align as u32;
    let data_size = (num_frames * block_align as usize) as u32;
    let file_size = 36 + data_size;

    let mut buf = Vec::with_capacity(44 + data_size as usize);
    // RIFF Header
    buf.extend_from_slice(b"RIFF");
    buf.extend_from_slice(&file_size.to_le_bytes());
    buf.extend_from_slice(b"WAVE");

    // fmt subchunk
    buf.extend_from_slice(b"fmt ");
    buf.extend_from_slice(&16u32.to_le_bytes()); // Subchunk1Size
    buf.extend_from_slice(&1u16.to_le_bytes());  // AudioFormat (1 = PCM)
    buf.extend_from_slice(&channels.to_le_bytes());
    buf.extend_from_slice(&sample_rate.to_le_bytes());
    buf.extend_from_slice(&byte_rate.to_le_bytes());
    buf.extend_from_slice(&block_align.to_le_bytes());
    buf.extend_from_slice(&bits_per_sample.to_le_bytes());

    // data subchunk
    buf.extend_from_slice(b"data");
    buf.extend_from_slice(&data_size.to_le_bytes());

    // Generate sinusoidal or ramp audio values to ensure non-trivial PCM samples
    for i in 0..num_frames {
        let t = i as f64 / sample_rate as f64;
        let val = (t * 440.0 * 2.0 * std::f64::consts::PI).sin();
        let sample_i16 = (val * 16000.0) as i16;
        for _ in 0..channels {
            buf.extend_from_slice(&sample_i16.to_le_bytes());
        }
    }

    buf
}

/// Helper to generate a valid minimal PNG image in memory.
fn create_synthetic_png(width: u32, height: u32) -> Vec<u8> {
    let img = image::ImageBuffer::<image::Rgba<u8>, _>::new(width, height);
    let mut cursor = Cursor::new(Vec::new());
    img.write_to(&mut cursor, image::ImageFormat::Png)
        .expect("Failed to encode PNG");
    cursor.into_inner()
}

/// Helper to generate a valid minimal JPEG image in memory.
fn create_synthetic_jpeg(width: u32, height: u32) -> Vec<u8> {
    let img = image::ImageBuffer::<image::Rgb<u8>, _>::from_fn(width, height, |x, y| {
        image::Rgb([(x % 256) as u8, (y % 256) as u8, 128u8])
    });
    let mut cursor = Cursor::new(Vec::new());
    img.write_to(&mut cursor, image::ImageFormat::Jpeg)
        .expect("Failed to encode JPEG");
    cursor.into_inner()
}

// ============================================================================
// 1. Audio Decoding Stress Tests (Symphonia)
// ============================================================================

#[test]
fn test_audio_mono_wav_decoding_and_buffer_extraction() {
    let dir = tempdir().unwrap();
    let mono_path = dir.path().join("mono_48k.wav");
    let wav_bytes = create_synthetic_wav(48000, 1, 1.5);
    fs::write(&mono_path, wav_bytes).unwrap();

    let path_str = mono_path.to_str().unwrap();
    let mut audio_dec = SymphoniaAudioDecoder::open(path_str).expect("Failed to open mono WAV");

    assert_eq!(audio_dec.sample_rate(), 48000);
    assert_eq!(audio_dec.channels(), 1);
    assert_eq!(audio_dec.format(), AudioSampleFormat::F32Interleaved);
    assert!((audio_dec.duration_seconds() - 1.5).abs() < 0.05);

    let mut total_samples = 0;
    let mut buffer_count = 0;

    while let Some(buffer) = audio_dec.next_audio_buffer().expect("Decode mono buffer") {
        buffer_count += 1;
        assert_eq!(buffer.sample_rate, 48000);
        assert_eq!(buffer.channels, 1);
        assert_eq!(buffer.sample_count(), buffer.frame_count());
        assert!(!buffer.samples_f32.is_empty());

        for &s in &buffer.samples_f32 {
            assert!(s.is_finite(), "Sample must be finite");
            assert!(s >= -1.0 && s <= 1.0, "Sample out of bounds: {}", s);
        }

        total_samples += buffer.samples_f32.len();
    }

    assert!(buffer_count > 0);
    // 48000 samples/sec * 1.5 sec * 1 channel = 72000 samples
    assert_eq!(total_samples, 72000);

    // Assert clean EOS
    let eof = audio_dec.next_audio_buffer().expect("EOS check");
    assert!(eof.is_none());
}

#[test]
fn test_audio_stereo_wav_multi_sample_rates() {
    let dir = tempdir().unwrap();
    let rates = [8000, 22050, 44100, 48000, 96000];

    for &rate in &rates {
        let wav_path = dir.path().join(format!("stereo_{}.wav", rate));
        let wav_bytes = create_synthetic_wav(rate, 2, 0.5);
        fs::write(&wav_path, wav_bytes).unwrap();

        let mut audio_dec = SymphoniaAudioDecoder::open(wav_path.to_str().unwrap())
            .unwrap_or_else(|e| panic!("Failed to open {} Hz stereo WAV: {:?}", rate, e));

        assert_eq!(audio_dec.sample_rate(), rate);
        assert_eq!(audio_dec.channels(), 2);

        let mut total_samples = 0;
        while let Some(buf) = audio_dec.next_audio_buffer().unwrap() {
            assert_eq!(buf.sample_rate, rate);
            assert_eq!(buf.channels, 2);
            assert_eq!(buf.samples_f32.len() % 2, 0, "Stereo samples must be even");
            for &s in &buf.samples_f32 {
                assert!(s.is_finite());
                assert!(s >= -1.0 && s <= 1.0);
            }
            total_samples += buf.samples_f32.len();
        }

        // rate * 0.5 sec * 2 channels = rate samples
        assert_eq!(total_samples, rate as usize);
    }
}

#[test]
fn test_audio_seek_accuracy_and_repeat_eos() {
    let dir = tempdir().unwrap();
    let wav_path = dir.path().join("seek_test.wav");
    let wav_bytes = create_synthetic_wav(48000, 2, 4.0); // 4.0 seconds
    fs::write(&wav_path, wav_bytes).unwrap();

    let mut audio_dec = SymphoniaAudioDecoder::open(wav_path.to_str().unwrap()).unwrap();

    // 1. Initial sequential read to advance position
    let first_buf = audio_dec.next_audio_buffer().unwrap().unwrap();
    assert_eq!(first_buf.pts, 0);

    // 2. Seek to 2.0s
    audio_dec.seek_second(2.0).expect("Seek to 2.0s failed");
    let buf_2s = audio_dec.next_audio_buffer().unwrap().unwrap();
    let sec = buf_2s.timestamp_seconds();
    assert!(
        (sec - 2.0).abs() < 0.1,
        "Expected timestamp near 2.0s, got: {}",
        sec
    );

    // 3. Seek to near end (3.8s)
    audio_dec.seek_second(3.8).expect("Seek to 3.8s failed");
    let buf_near_end = audio_dec.next_audio_buffer().unwrap().unwrap();
    assert!(buf_near_end.timestamp_seconds() >= 3.7);

    // 4. Read until EOS
    while let Some(_) = audio_dec.next_audio_buffer().unwrap() {}

    // 5. Subsequent calls after EOS must consistently return Ok(None) without panics
    for i in 0..5 {
        let post_eos = audio_dec.next_audio_buffer().expect("Post-EOS call failed");
        assert!(
            post_eos.is_none(),
            "Post-EOS call {} should return Ok(None)",
            i
        );
    }

    // 6. Seek rewind to 0.0s after hitting EOS
    audio_dec.seek_second(0.0).expect("Rewind to 0.0s after EOS failed");
    let buf_start = audio_dec.next_audio_buffer().unwrap().unwrap();
    assert_eq!(buf_start.pts, 0);
    assert!(buf_start.timestamp_seconds() < 0.05);

    // 7. Rapid bidirectional seeking
    let seek_targets = [1.5, 3.5, 0.5, 2.5, 1.0];
    for &target in &seek_targets {
        audio_dec.seek_second(target).unwrap_or_else(|e| {
            panic!("Rapid seek to {} failed: {:?}", target, e);
        });
        let b = audio_dec.next_audio_buffer().unwrap().unwrap();
        assert!(
            (b.timestamp_seconds() - target).abs() < 0.15,
            "Rapid seek to {} deviated too far: {}",
            target,
            b.timestamp_seconds()
        );
    }
}

// ============================================================================
// 2. Still Image Decoding Stress Tests (PNG & JPEG)
// ============================================================================

#[test]
fn test_still_image_jpeg_extraction_and_timeline_duration() {
    let dir = tempdir().unwrap();
    let jpg_path = dir.path().join("photo.jpg");
    let jpg_bytes = create_synthetic_jpeg(640, 480);
    fs::write(&jpg_path, jpg_bytes).unwrap();

    let path_str = jpg_path.to_str().unwrap();

    // 1. Test ImageSequenceDecoder direct open
    let mut img_dec = ImageSequenceDecoder::open(path_str).expect("Failed to open JPEG directly");
    assert_eq!(img_dec.width(), 640);
    assert_eq!(img_dec.height(), 480);
    assert_eq!(img_dec.pixel_format(), PixelFormat::Rgba8);
    assert_eq!(img_dec.fps(), 60.0);
    assert_eq!(img_dec.duration_pts(), 300); // 5.0s @ 60 FPS
    assert!((img_dec.duration_seconds() - 5.0).abs() < 1e-4);

    // Extract first frame
    let frame0 = img_dec.next_frame().unwrap().expect("First JPEG frame");
    assert_eq!(frame0.width, 640);
    assert_eq!(frame0.height, 480);
    assert_eq!(frame0.format, PixelFormat::Rgba8);
    assert_eq!(frame0.pts, 0);
    assert_eq!(frame0.duration_pts, 300);
    assert_eq!(frame0.data.len(), 640 * 480 * 4);

    // 2. Test unified open_media_decoder with JPEG
    let mut media = open_media_decoder(path_str).expect("Failed to open JPEG via open_media_decoder");
    assert_eq!(media.media_type(), MediaType::Image);
    assert!(media.audio().is_none());
    assert!(media.video().is_some());

    let video = media.video().unwrap();
    assert_eq!(video.duration_pts(), 300);

    // Seek within still image
    video.seek_pts(150).expect("Seek to PTS 150");
    let frame_seek = video.next_frame().unwrap().unwrap();
    assert_eq!(frame_seek.pts, 150);

    // Seek past duration clamps / returns None
    video.seek_pts(300).expect("Seek to PTS 300");
    let frame_eof = video.next_frame().unwrap();
    assert!(frame_eof.is_none(), "Frame past duration must be None");

    // Repeated calls after EOF return None
    for _ in 0..3 {
        assert!(video.next_frame().unwrap().is_none());
    }
}

#[test]
fn test_still_image_png_full_sequential_playback_to_eos() {
    let dir = tempdir().unwrap();
    let png_path = dir.path().join("graphic.png");
    let png_bytes = create_synthetic_png(16, 16);
    fs::write(&png_path, png_bytes).unwrap();

    let mut img_dec = ImageSequenceDecoder::open_with_duration(
        png_path.to_str().unwrap(),
        0.1, // 0.1s @ 60 FPS = 6 frames
        Rational { num: 60, den: 1 },
    )
    .expect("Open short PNG");

    assert_eq!(img_dec.duration_pts(), 6);

    let mut frame_count = 0;
    while let Some(f) = img_dec.next_frame().unwrap() {
        assert_eq!(f.pts, frame_count);
        assert_eq!(f.width, 16);
        assert_eq!(f.height, 16);
        frame_count += 1;
    }
    assert_eq!(frame_count, 6);

    // Repeated EOS calls
    assert!(img_dec.next_frame().unwrap().is_none());
    assert!(img_dec.next_frame().unwrap().is_none());
}

#[test]
fn test_still_image_custom_timebases() {
    let dir = tempdir().unwrap();
    let png_path = dir.path().join("timebase_png.png");
    fs::write(&png_path, create_synthetic_png(32, 32)).unwrap();
    let path_str = png_path.to_str().unwrap();

    // 24 FPS (Cinematic standard): 5.0s * 24 = 120 PTS
    let dec_24 = ImageSequenceDecoder::open_with_timebase(path_str, Rational { num: 24, den: 1 }).unwrap();
    assert_eq!(dec_24.fps(), 24.0);
    assert_eq!(dec_24.duration_pts(), 120);

    // 30 FPS: 5.0s * 30 = 150 PTS
    let dec_30 = ImageSequenceDecoder::open_with_timebase(path_str, Rational { num: 30, den: 1 }).unwrap();
    assert_eq!(dec_30.fps(), 30.0);
    assert_eq!(dec_30.duration_pts(), 150);

    // 120 FPS: 5.0s * 120 = 600 PTS
    let dec_120 = ImageSequenceDecoder::open_with_timebase(path_str, Rational { num: 120, den: 1 }).unwrap();
    assert_eq!(dec_120.fps(), 120.0);
    assert_eq!(dec_120.duration_pts(), 600);
}

// ============================================================================
// 3. Error Conditions & Graceful Recovery Stress Tests
// ============================================================================

#[test]
fn test_error_recovery_nonexistent_files() {
    let fake_path = "/tmp/aether_stress_test_nonexistent_file_987654321.wav";

    // 1. open_media_decoder
    let res1 = open_media_decoder(fake_path);
    assert!(matches!(res1.err(), Some(DecoderError::FileNotFound(_))));

    // 2. open_audio_decoder
    let res2 = open_audio_decoder(fake_path);
    assert!(matches!(res2.err(), Some(DecoderError::FileNotFound(_))));

    // 3. open_video_decoder
    let res3 = open_video_decoder(fake_path);
    assert!(matches!(res3.err(), Some(DecoderError::FileNotFound(_))));

    // 4. SymphoniaAudioDecoder::open
    let res4 = SymphoniaAudioDecoder::open(fake_path);
    assert!(matches!(res4.err(), Some(DecoderError::FileNotFound(_))));

    // 5. ImageSequenceDecoder::open
    let res5 = ImageSequenceDecoder::open(fake_path);
    assert!(matches!(res5.err(), Some(DecoderError::FileNotFound(_))));
}

#[test]
fn test_error_recovery_directory_as_file() {
    let dir = tempdir().unwrap();
    let dir_str = dir.path().to_str().unwrap();

    let res1 = open_media_decoder(dir_str);
    assert!(matches!(res1.err(), Some(DecoderError::NotAFile(_))));

    let res2 = open_audio_decoder(dir_str);
    assert!(matches!(res2.err(), Some(DecoderError::NotAFile(_))));

    let res3 = open_video_decoder(dir_str);
    assert!(matches!(res3.err(), Some(DecoderError::NotAFile(_))));

    let res4 = SymphoniaAudioDecoder::open(dir_str);
    assert!(matches!(res4.err(), Some(DecoderError::NotAFile(_))));

    let res5 = ImageSequenceDecoder::open(dir_str);
    assert!(matches!(res5.err(), Some(DecoderError::NotAFile(_))));
}

#[test]
fn test_error_recovery_zero_byte_file() {
    let dir = tempdir().unwrap();
    let empty_path = dir.path().join("zero_length.wav");
    fs::write(&empty_path, b"").unwrap();
    let path_str = empty_path.to_str().unwrap();

    let res1 = open_media_decoder(path_str);
    assert!(matches!(res1.err(), Some(DecoderError::EmptyFile(_))));

    let res2 = open_audio_decoder(path_str);
    assert!(matches!(res2.err(), Some(DecoderError::EmptyFile(_))));

    let res3 = open_video_decoder(path_str);
    assert!(matches!(res3.err(), Some(DecoderError::EmptyFile(_))));

    let res4 = SymphoniaAudioDecoder::open(path_str);
    assert!(matches!(res4.err(), Some(DecoderError::EmptyFile(_))));

    let res5 = ImageSequenceDecoder::open(path_str);
    assert!(matches!(res5.err(), Some(DecoderError::EmptyFile(_))));
}

#[test]
fn test_error_recovery_corrupted_headers_and_payloads() {
    let dir = tempdir().unwrap();

    // Corrupted WAV (only 12 bytes of header)
    let bad_wav = dir.path().join("broken.wav");
    fs::write(&bad_wav, b"RIFF\x20\x00\x00\x00WAVEfmt ").unwrap();
    let res_wav = open_media_decoder(bad_wav.to_str().unwrap());
    assert!(res_wav.is_err(), "Truncated WAV must return error");
    let err = res_wav.err().unwrap();
    assert!(
        matches!(err, DecoderError::CorruptData(_) | DecoderError::DemuxError(_) | DecoderError::UnsupportedContainer(_)),
        "Expected corrupt data or demux error, got: {:?}",
        err
    );

    // Direct open on bad WAV
    let res_symphonia = SymphoniaAudioDecoder::open(bad_wav.to_str().unwrap());
    assert!(res_symphonia.is_err());

    // Corrupted PNG (broken signature)
    let bad_png = dir.path().join("broken.png");
    fs::write(&bad_png, b"\x89PNG\r\n\x1a\nCORRUPTED_PAYLOAD_HERE").unwrap();
    let res_png = open_media_decoder(bad_png.to_str().unwrap());
    assert!(res_png.is_err(), "Corrupted PNG must return error");

    // Corrupted JPEG (SOI marker followed by random junk)
    let bad_jpg = dir.path().join("broken.jpg");
    fs::write(&bad_jpg, b"\xFF\xD8\xFF\xE0\x00\x10JFIF\x00_TRUNCATED_JUNK").unwrap();
    let res_jpg = open_media_decoder(bad_jpg.to_str().unwrap());
    assert!(res_jpg.is_err(), "Corrupted JPEG must return error");
}

#[test]
fn test_error_recovery_invalid_file_extensions() {
    let dir = tempdir().unwrap();

    // Plain text file with .txt extension
    let txt_path = dir.path().join("notes.txt");
    fs::write(&txt_path, b"Just some plain text notes, not media.").unwrap();
    let path_str = txt_path.to_str().unwrap();

    let res_media = open_media_decoder(path_str);
    assert!(res_media.is_err());
    assert!(matches!(res_media.err(), Some(DecoderError::UnsupportedContainer(_))));

    let res_audio = open_audio_decoder(path_str);
    assert!(res_audio.is_err());

    let res_video = open_video_decoder(path_str);
    assert!(res_video.is_err());

    // Arbitrary binary file with .bin extension
    let bin_path = dir.path().join("data.bin");
    fs::write(&bin_path, &[0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]).unwrap();
    let bin_str = bin_path.to_str().unwrap();

    let res_bin = open_media_decoder(bin_str);
    assert!(res_bin.is_err());
}

#[test]
fn test_error_cross_decoder_type_mismatches() {
    let dir = tempdir().unwrap();

    // WAV audio file passed to open_video_decoder
    let wav_path = dir.path().join("sound.wav");
    fs::write(&wav_path, create_synthetic_wav(44100, 2, 0.5)).unwrap();
    let wav_str = wav_path.to_str().unwrap();

    let res_video_on_audio = open_video_decoder(wav_str);
    assert!(
        matches!(res_video_on_audio.err(), Some(DecoderError::NoVideoStream)),
        "Opening audio file with open_video_decoder must yield NoVideoStream"
    );

    // PNG image file passed to open_audio_decoder
    let png_path = dir.path().join("photo.png");
    fs::write(&png_path, create_synthetic_png(64, 64)).unwrap();
    let png_str = png_path.to_str().unwrap();

    let res_audio_on_image = open_audio_decoder(png_str);
    assert!(
        matches!(res_audio_on_image.err(), Some(DecoderError::NoAudioStream)),
        "Opening image file with open_audio_decoder must yield NoAudioStream"
    );
}

#[test]
fn test_audio_surround_5_1_multichannel_decoding() {
    let dir = tempdir().unwrap();
    let surround_path = dir.path().join("surround_5_1.wav");
    // 6 channels (5.1 surround), 48 kHz, 1.0 second
    let wav_bytes = create_synthetic_wav(48000, 6, 1.0);
    fs::write(&surround_path, wav_bytes).unwrap();

    let mut audio_dec = SymphoniaAudioDecoder::open(surround_path.to_str().unwrap())
        .expect("Failed to open 5.1 surround WAV");

    assert_eq!(audio_dec.sample_rate(), 48000);
    assert_eq!(audio_dec.channels(), 6);

    let mut total_samples = 0;
    while let Some(buf) = audio_dec.next_audio_buffer().unwrap() {
        assert_eq!(buf.channels, 6);
        assert_eq!(buf.samples_f32.len() % 6, 0);
        assert_eq!(buf.frame_count(), buf.samples_f32.len() / 6);
        for &s in &buf.samples_f32 {
            assert!(s.is_finite());
            assert!(s >= -1.0 && s <= 1.0);
        }
        total_samples += buf.samples_f32.len();
    }

    // 48000 frames * 6 channels = 288000 samples
    assert_eq!(total_samples, 288000);
}

#[test]
fn test_audio_negative_and_extreme_beyond_duration_seek() {
    let dir = tempdir().unwrap();
    let wav_path = dir.path().join("seek_bounds.wav");
    fs::write(&wav_path, create_synthetic_wav(44100, 2, 2.0)).unwrap();

    let mut audio_dec = SymphoniaAudioDecoder::open(wav_path.to_str().unwrap()).unwrap();

    // 1. Negative PTS seek should safely clamp to 0 without panicking
    audio_dec.seek_pts(-1000).expect("Negative PTS seek should clamp to 0");
    let buf0 = audio_dec.next_audio_buffer().unwrap().unwrap();
    assert_eq!(buf0.pts, 0);

    // 2. Seeking far beyond duration (e.g. 50.0s on a 2.0s file)
    let res_seek_beyond = audio_dec.seek_second(50.0);
    // Depending on Symphonia backend, it either succeeds and returns None on next buffer,
    // or returns a graceful SeekError. Neither should panic.
    match res_seek_beyond {
        Ok(()) => {
            let next = audio_dec.next_audio_buffer().unwrap();
            assert!(next.is_none(), "Buffer after seeking beyond end must be EOS");
        }
        Err(e) => {
            assert!(matches!(e, DecoderError::SeekError(_)));
        }
    }

    // 3. Rewind to 0.5s after beyond-end seek
    audio_dec.seek_second(0.5).expect("Rewind after beyond-end seek");
    let buf_rewound = audio_dec.next_audio_buffer().unwrap().unwrap();
    assert!((buf_rewound.timestamp_seconds() - 0.5).abs() < 0.1);
}

#[test]
fn test_still_image_odd_dimensions_and_rewind_after_eos() {
    let dir = tempdir().unwrap();
    let odd_png = dir.path().join("odd_17x23.png");
    fs::write(&odd_png, create_synthetic_png(17, 23)).unwrap();

    let mut dec = ImageSequenceDecoder::open(odd_png.to_str().unwrap()).unwrap();
    assert_eq!(dec.width(), 17);
    assert_eq!(dec.height(), 23);

    // Frame data length must be exactly 17 * 23 * 4 = 1564 bytes
    let frame = dec.next_frame().unwrap().unwrap();
    assert_eq!(frame.width, 17);
    assert_eq!(frame.height, 23);
    assert_eq!(frame.row_stride_bytes, 17 * 4);
    assert_eq!(frame.data.len(), 17 * 23 * 4);

    // Negative seek clamps to 0
    dec.seek_pts(-500).expect("Negative seek on image");
    let frame_rewound = dec.next_frame().unwrap().unwrap();
    assert_eq!(frame_rewound.pts, 0);

    // Seek past duration (e.g. PTS 9999) -> None
    dec.seek_pts(9999).expect("Seek past duration");
    assert!(dec.next_frame().unwrap().is_none());

    // Rewind back into valid range after EOS -> succeeds
    dec.seek_pts(100).expect("Rewind back into range");
    let frame_restored = dec.next_frame().unwrap().unwrap();
    assert_eq!(frame_restored.pts, 100);
}

#[test]
fn test_adversarial_decoder_fuzzing_panic_safety_harness() {
    let dir = tempdir().unwrap();
    let extensions = ["wav", "mp3", "png", "jpg", "mp4", "txt", "bin"];

    // Deterministic LCG pseudo-random generator
    let mut seed: u64 = 0xCAFEBABE12345678;
    let mut next_byte = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        (seed >> 33) as u8
    };

    let iterations = 100;
    for i in 0..iterations {
        let ext = extensions[i % extensions.len()];
        let len = (i * 43) % 2048 + 1;
        let mut payload = Vec::with_capacity(len);
        for _ in 0..len {
            payload.push(next_byte());
        }

        let file_path = dir.path().join(format!("fuzz_decoder_{}.{}", i, ext));
        fs::write(&file_path, &payload).unwrap();
        let path_str = file_path.to_str().unwrap();

        // 1. open_media_decoder panic safety
        let res_media = catch_unwind(AssertUnwindSafe(|| {
            open_media_decoder(path_str)
        }));
        assert!(
            res_media.is_ok(),
            "open_media_decoder PANICKED on iteration {} (ext={})",
            i,
            ext
        );

        // 2. SymphoniaAudioDecoder::open panic safety
        let res_symphonia = catch_unwind(AssertUnwindSafe(|| {
            let _ = SymphoniaAudioDecoder::open(path_str);
        }));
        assert!(
            res_symphonia.is_ok(),
            "SymphoniaAudioDecoder::open PANICKED on iteration {} (ext={})",
            i,
            ext
        );

        // 3. ImageSequenceDecoder::open panic safety
        let res_image = catch_unwind(AssertUnwindSafe(|| {
            let _ = ImageSequenceDecoder::open(path_str);
        }));
        assert!(
            res_image.is_ok(),
            "ImageSequenceDecoder::open PANICKED on iteration {} (ext={})",
            i,
            ext
        );
    }
}
