mod fixtures;

use aether_core::media::MediaType;
use aether_core::timeline::Rational;
use aether_media::decoder::{
    open_media_decoder, open_video_decoder, AudioDecoder, DecoderError, PixelFormat,
    SymphoniaAudioDecoder, SyntheticPattern, SyntheticVideoDecoder, VideoDecoder, VideoFrame,
};
use std::fs;
use std::sync::Arc;
use std::thread;
use tempfile::tempdir;

// ============================================================================
// Stress Vector 1: Repeated Sequential Extraction to End-of-Stream (EOS)
// ============================================================================

#[test]
fn test_stress_mp4_repeated_eos_and_rewind_cycles() {
    let mp4_path = fixtures::ensure_sample_mp4();
    let path_str = mp4_path.to_str().expect("Valid path");

    let mut video_dec = open_video_decoder(path_str).expect("Open MP4 decoder");
    assert_eq!(video_dec.width(), 64);
    assert_eq!(video_dec.height(), 64);

    // Run 5 full cycles: decode all 10 frames to EOS, call past EOS, then rewind to 0 and repeat
    for cycle in 0..5 {
        let mut frame_count = 0;
        let mut prev_pts: Option<i64> = None;

        while let Some(mut frame) = video_dec.next_frame().expect("Frame decoding") {
            frame_count += 1;
            frame.ensure_rgba8();
            assert_eq!(frame.width, 64);
            assert_eq!(frame.height, 64);
            assert_eq!(frame.format, PixelFormat::Rgba8);
            assert_eq!(frame.data.len(), (frame.row_stride_bytes * frame.height) as usize);

            if let Some(p) = prev_pts {
                assert!(
                    frame.pts > p,
                    "Cycle {}: PTS must be strictly ascending: got {} after {}",
                    cycle,
                    frame.pts,
                    p
                );
            }
            prev_pts = Some(frame.pts);
        }

        assert_eq!(
            frame_count, 10,
            "Cycle {}: expected exactly 10 frames before EOS",
            cycle
        );

        // Consecutive calls past EOS must consistently return Ok(None)
        for post_eos_idx in 0..5 {
            let res = video_dec.next_frame().expect("Call past EOS should not error");
            assert!(
                res.is_none(),
                "Cycle {} post-EOS call {} should return None",
                cycle,
                post_eos_idx
            );
        }

        // Rewind to 0 for next cycle
        video_dec.seek_pts(0).expect("Rewind to PTS 0 failed");
    }
}

#[test]
fn test_stress_synthetic_eos_across_all_patterns() {
    let patterns = [
        SyntheticPattern::ColorCycle,
        SyntheticPattern::MovingBar,
        SyntheticPattern::SMPTEBars,
    ];

    for pattern in patterns {
        let mut dec = SyntheticVideoDecoder::with_pattern(64, 64, 30.0, 1.0, pattern);
        assert_eq!(dec.duration_pts(), 30);

        let mut frames_decoded = 0;
        while let Some(frame) = dec.next_frame().unwrap() {
            frames_decoded += 1;
            assert_eq!(frame.format, PixelFormat::Rgba8);
            assert_eq!(frame.data.len(), 64 * 64 * 4);
        }
        assert_eq!(frames_decoded, 30);

        // Post-EOS calls
        for _ in 0..10 {
            assert!(dec.next_frame().unwrap().is_none());
        }

        // Rewind and re-decode
        dec.seek_pts(0).unwrap();
        let first_frame = dec.next_frame().unwrap().expect("First frame after rewind");
        assert_eq!(first_frame.pts, 0);
    }
}

// ============================================================================
// Stress Vector 2: Non-Monotonic, Backward, and Random-Access Seeking
// ============================================================================

#[test]
fn test_stress_mp4_random_access_scrubbing() {
    let mp4_path = fixtures::ensure_sample_mp4();
    let path_str = mp4_path.to_str().expect("Valid path");
    let mut video_dec = open_video_decoder(path_str).expect("Open MP4");

    // Non-monotonic target timestamps in seconds
    let target_seconds = [
        0.7, 0.2, 0.9, 0.0, 0.5, 0.1, 0.8, 0.3, 0.6, 0.4, 0.0, 0.9, 0.2,
    ];

    for &target_sec in &target_seconds {
        video_dec.seek_seconds(target_sec).expect("Seek failed");
        let frame = video_dec
            .next_frame()
            .expect("Decode after seek failed")
            .expect("Expected frame after seeking within bounds");

        let actual_sec = frame.timestamp_seconds();
        // Hardware keyframe seeking tolerance: within 0.25 seconds of target
        assert!(
            (actual_sec - target_sec).abs() < 0.25,
            "Target {}s resulted in frame at {}s (diff {})",
            target_sec,
            actual_sec,
            (actual_sec - target_sec).abs()
        );
        assert_eq!(frame.width, 64);
        assert_eq!(frame.height, 64);
        assert_eq!(frame.format, PixelFormat::Rgba8);
    }
}

#[test]
fn test_stress_mp4_extract_frame_at_pts_convenience_scrubbing() {
    let mp4_path = fixtures::ensure_sample_mp4();
    let path_str = mp4_path.to_str().expect("Valid path");
    let mut video_dec = open_video_decoder(path_str).expect("Open MP4");

    let fps = video_dec.fps();
    // Non-monotonic sequence of target PTS
    let target_pts_list = [7, 1, 9, 0, 5, 2, 8, 4, 3, 6, 0];

    for &pts in &target_pts_list {
        let frame = video_dec
            .extract_frame_at_pts(pts)
            .expect("extract_frame_at_pts")
            .expect("Expected frame");

        let target_sec = pts as f64 / fps;
        let actual_sec = frame.timestamp_seconds();
        assert!(
            (actual_sec - target_sec).abs() < 0.25,
            "Target PTS {} ({}s) yielded {}s",
            pts,
            target_sec,
            actual_sec
        );
    }
}

#[test]
fn test_stress_synthetic_exact_random_access_and_color_verification() {
    // 64x64, 30 fps, 2.0s = 60 frames, ColorCycle pattern
    let mut dec = SyntheticVideoDecoder::with_pattern_and_timebase(
        64,
        64,
        30.0,
        2.0,
        SyntheticPattern::ColorCycle,
        Rational { num: 30, den: 1 },
    );

    // Random access frame indices
    let probe_indices = [45, 2, 59, 0, 30, 15, 58, 1, 40, 29, 0, 59];

    for &idx in &probe_indices {
        let frame = dec
            .extract_frame_at_pts(idx)
            .expect("extract_frame_at_pts")
            .expect("frame");

        assert_eq!(frame.pts, idx);

        // Verify mathematical exactness of pixel generation:
        // r = ((frame_idx * 67 + 31) % 256) as u8
        // g = ((frame_idx * 131 + 73) % 256) as u8
        // b = ((frame_idx * 197 + 127) % 256) as u8
        // a = 255
        let expected_r = ((idx * 67 + 31) % 256) as u8;
        let expected_g = ((idx * 131 + 73) % 256) as u8;
        let expected_b = ((idx * 197 + 127) % 256) as u8;

        assert_eq!(frame.data[0], expected_r, "Frame {}: R mismatch", idx);
        assert_eq!(frame.data[1], expected_g, "Frame {}: G mismatch", idx);
        assert_eq!(frame.data[2], expected_b, "Frame {}: B mismatch", idx);
        assert_eq!(frame.data[3], 255, "Frame {}: Alpha must be 255", idx);
    }
}

// ============================================================================
// Stress Vector 3: Boundary PTS, Clamping, and Rapid Consecutive Seeks
// ============================================================================

#[test]
fn test_stress_boundary_negative_pts_clamping() {
    let mp4_path = fixtures::ensure_sample_mp4();
    let path_str = mp4_path.to_str().expect("Valid path");
    let mut video_dec = open_video_decoder(path_str).expect("Open MP4");

    // Negative PTS seeking must clamp to 0 and not fail or crash
    let negative_pts_values = [-1, -10, -500, -100_000, i64::MIN];
    for &neg_pts in &negative_pts_values {
        video_dec.seek_pts(neg_pts).expect("Negative PTS seek should clamp safely");
        let frame = video_dec
            .next_frame()
            .expect("Decode after negative seek")
            .expect("Expected frame near 0");

        assert!(
            frame.timestamp_seconds() < 0.15,
            "Negative PTS {} should yield first frame near 0.0s, got: {}",
            neg_pts,
            frame.timestamp_seconds()
        );
    }
}

#[test]
fn test_stress_boundary_past_duration_seeking() {
    let mp4_path = fixtures::ensure_sample_mp4();
    let path_str = mp4_path.to_str().expect("Valid path");
    let mut video_dec = open_video_decoder(path_str).expect("Open MP4");

    // Seek past total duration: duration is ~1.0s (10 PTS)
    let past_pts_values = [20, 100, 10_000, 1_000_000, i64::MAX / 2, i64::MAX];
    for &pts in &past_pts_values {
        video_dec.seek_pts(pts).expect("Past duration seek should succeed");
        let frame = video_dec.next_frame().expect("Next frame after past duration seek");
        assert!(
            frame.is_none(),
            "Seek past duration to PTS {} must return Ok(None) EOS",
            pts
        );
    }

    // Now verify we can recover from past-duration EOF by seeking back to 0
    video_dec.seek_pts(0).expect("Rewind from past-duration EOF");
    let recovered_frame = video_dec.next_frame().unwrap().expect("Recovered frame at 0");
    assert!(recovered_frame.timestamp_seconds() < 0.15);
}

#[test]
fn test_stress_rapid_consecutive_seeks_without_decoding() {
    let mp4_path = fixtures::ensure_sample_mp4();
    let path_str = mp4_path.to_str().expect("Valid path");
    let mut video_dec = open_video_decoder(path_str).expect("Open MP4");

    // Rapidly seek 50 times without calling next_frame
    let seek_targets = [0.1, 0.8, 0.2, 0.7, 0.0, 0.9, 0.3, 0.5, 0.4];
    for (i, &sec) in seek_targets.iter().cycle().take(50).enumerate() {
        video_dec
            .seek_seconds(sec)
            .unwrap_or_else(|e| panic!("Rapid seek {} to {}s failed: {:?}", i, sec, e));
    }

    // Final seek to 0.5s
    video_dec.seek_seconds(0.5).expect("Final seek failed");
    let frame = video_dec
        .next_frame()
        .expect("Decode after rapid seeks failed")
        .expect("Expected frame at final seek position");

    assert!(
        (frame.timestamp_seconds() - 0.5).abs() < 0.25,
        "Expected frame near 0.5s after rapid seeks, got: {}",
        frame.timestamp_seconds()
    );
}

#[test]
fn test_stress_synthetic_decoder_pathological_parameters() {
    // 0-duration, negative duration, 0-fps, negative fps
    let dec1 = SyntheticVideoDecoder::new(0, 0, 0.0, 0.0);
    assert!(dec1.width() >= 16);
    assert!(dec1.height() >= 16);
    assert!(dec1.fps() > 0.0);
    assert!(dec1.duration_seconds() > 0.0);

    let dec2 = SyntheticVideoDecoder::new(8, 8, -60.0, -10.0);
    assert!(dec2.width() >= 16);
    assert!(dec2.height() >= 16);
    assert!(dec2.fps() > 0.0);
    assert!(dec2.duration_seconds() > 0.0);

    // Extremely large dimensions
    let mut dec_large = SyntheticVideoDecoder::new(1920, 1080, 60.0, 0.1);
    let frame = dec_large.next_frame().unwrap().unwrap();
    assert_eq!(frame.width, 1920);
    assert_eq!(frame.height, 1080);
    assert_eq!(frame.data.len(), 1920 * 1080 * 4);
}

// ============================================================================
// Stress Vector 4: Pixel Buffer Integrity, Stride, Idempotency, Independence
// ============================================================================

#[test]
fn test_stress_pixel_buffer_format_and_alpha_integrity() {
    let mp4_path = fixtures::ensure_sample_mp4();
    let path_str = mp4_path.to_str().expect("Valid path");
    let mut video_dec = open_video_decoder(path_str).expect("Open MP4");

    while let Some(frame) = video_dec.next_frame().unwrap() {
        assert_eq!(frame.width, 64);
        assert_eq!(frame.height, 64);
        assert_eq!(frame.format, PixelFormat::Rgba8);

        let expected_min_bytes = (frame.width * frame.height * 4) as usize;
        assert!(
            frame.data.len() >= expected_min_bytes,
            "Buffer size {} < expected minimum {}",
            frame.data.len(),
            expected_min_bytes
        );

        // Verify Alpha channel is 255 (fully opaque) for all pixels in 64x64 frame
        let stride = frame.row_stride_bytes as usize;
        for row in 0..frame.height as usize {
            let row_start = row * stride;
            for col in 0..frame.width as usize {
                let pixel_offset = row_start + col * 4;
                let alpha = frame.data[pixel_offset + 3];
                assert_eq!(alpha, 255, "Alpha channel must be 255 (fully opaque)");
            }
        }
    }
}

#[test]
fn test_stress_ensure_rgba8_idempotency() {
    let mp4_path = fixtures::ensure_sample_mp4();
    let path_str = mp4_path.to_str().expect("Valid path");
    let mut video_dec = open_video_decoder(path_str).expect("Open MP4");

    let mut frame = video_dec.next_frame().unwrap().expect("Frame");
    assert_eq!(frame.format, PixelFormat::Rgba8);

    // Snapshot data after 1st implicit conversion
    let snapshot = frame.data.clone();

    // Repeatedly call ensure_rgba8: MUST BE AN IDEMPOTENT NO-OP!
    for _ in 0..10 {
        frame.ensure_rgba8();
        assert_eq!(frame.format, PixelFormat::Rgba8);
        assert_eq!(
            frame.data, snapshot,
            "ensure_rgba8 must be strictly idempotent — data mutated on repeated call!"
        );
    }
}

#[test]
fn test_stress_buffer_independence_across_sequential_frames() {
    let mp4_path = fixtures::ensure_sample_mp4();
    let path_str = mp4_path.to_str().expect("Valid path");
    let mut video_dec = open_video_decoder(path_str).expect("Open MP4");

    // Collect all frames into memory
    let mut frames = Vec::new();
    while let Some(frame) = video_dec.next_frame().unwrap() {
        frames.push(frame);
    }

    assert_eq!(frames.len(), 10);

    // Verify frames are pairwise distinct in pixel content or presentation timestamp
    for i in 0..frames.len() {
        for j in (i + 1)..frames.len() {
            assert_ne!(
                frames[i].pts, frames[j].pts,
                "Frames {} and {} have duplicate PTS {}",
                i, j, frames[i].pts
            );
        }
    }

    // Verify data buffers are completely independent heap allocations
    for i in 0..frames.len() {
        for j in (i + 1)..frames.len() {
            let ptr_i = frames[i].data.as_ptr();
            let ptr_j = frames[j].data.as_ptr();
            assert_ne!(ptr_i, ptr_j, "Frames {} and {} share same buffer pointer", i, j);
        }
    }
}

#[test]
fn test_stress_ensure_rgba8_arbitrary_stride_padding() {
    // 3x3 BGRA frame with 128 bytes row stride (116 bytes of padding per row)
    let width = 3u32;
    let height = 3u32;
    let row_stride = 128u32;
    let total_bytes = (row_stride * height) as usize;
    let mut data = vec![0xEEu8; total_bytes]; // padding marker

    // Write specific pixels in BGRA: B=10, G=20, R=30, A=255
    for r in 0..height as usize {
        let row_start = r * row_stride as usize;
        for c in 0..width as usize {
            let offset = row_start + c * 4;
            data[offset] = 10;     // B
            data[offset + 1] = 20; // G
            data[offset + 2] = 30; // R
            data[offset + 3] = 255;// A
        }
    }

    let mut frame = VideoFrame::with_stride(
        width,
        height,
        PixelFormat::Bgra8,
        data,
        row_stride,
        0,
        1,
        Rational { num: 60, den: 1 },
        true,
    );

    frame.ensure_rgba8();
    assert_eq!(frame.format, PixelFormat::Rgba8);

    // Verify pixels are now RGBA: R=30, G=20, B=10, A=255
    for r in 0..height as usize {
        let row_start = r * row_stride as usize;
        for c in 0..width as usize {
            let offset = row_start + c * 4;
            assert_eq!(frame.data[offset], 30, "R swapped");
            assert_eq!(frame.data[offset + 1], 20, "G preserved");
            assert_eq!(frame.data[offset + 2], 10, "B swapped");
            assert_eq!(frame.data[offset + 3], 255, "A preserved");
        }
        // Verify padding bytes were untouched
        for p in (width as usize * 4)..row_stride as usize {
            assert_eq!(frame.data[row_start + p], 0xEE, "Padding byte corrupted");
        }
    }
}

// ============================================================================
// Stress Vector 5: Audio Decoder Seeking and Boundary Robustness
// ============================================================================

#[test]
fn test_stress_symphonia_audio_rapid_seeking_and_boundaries() {
    let dir = tempdir().unwrap();
    let wav_path = dir.path().join("audio_stress.wav");
    let wav_bytes = fixtures::create_synthetic_wav(48000, 2, 2.5); // 2.5s stereo
    fs::write(&wav_path, wav_bytes).unwrap();

    let path_str = wav_path.to_str().unwrap();
    let mut audio_dec = SymphoniaAudioDecoder::open(path_str).expect("Open audio decoder");

    // 1. Rapid non-monotonic seek sequence
    let seek_targets = [1.5, 0.2, 2.0, 0.0, 1.8, 0.5, 2.2, 0.1];
    for &sec in &seek_targets {
        audio_dec.seek_second(sec).expect("Seek audio failed");
        let buf = audio_dec
            .next_audio_buffer()
            .expect("Decode audio after seek")
            .expect("Expected audio buffer");

        assert_eq!(buf.sample_rate, 48000);
        assert_eq!(buf.channels, 2);
        assert!(!buf.samples_f32.is_empty());
        assert!((buf.timestamp_seconds() - sec).abs() < 0.2);

        // Validate finite numbers
        for &s in &buf.samples_f32 {
            assert!(s.is_finite());
            assert!(s >= -1.0 && s <= 1.0);
        }
    }

    // 2. Negative seek
    audio_dec.seek_second(-1.0).expect("Negative audio seek");
    let buf_zero = audio_dec.next_audio_buffer().unwrap().expect("Buffer at start");
    assert_eq!(buf_zero.pts, 0);

    // 3. Seek boundary investigation:
    // Symphonia format reader returns SeekError when timestamp exceeds stream range.
    // Verify that seeking past stream duration produces a graceful DecoderError::SeekError
    // instead of crashing or panicking.
    let past_res = audio_dec.seek_second(100.0);
    assert!(
        matches!(past_res, Err(DecoderError::SeekError(_))),
        "Expected SeekError when seeking past audio duration, got: {:?}",
        past_res
    );

    // Verify recovery after SeekError: can rewind to 0 and continue decoding!
    audio_dec.seek_second(0.0).expect("Rewind to 0 after SeekError");
    let recovered_buf = audio_dec.next_audio_buffer().unwrap().expect("Recovered buffer");
    assert_eq!(recovered_buf.pts, 0);
}

// ============================================================================
// Stress Vector 6: Multi-Threaded Concurrent Decoding
// ============================================================================

#[test]
fn test_stress_multithreaded_concurrent_video_decoding() {
    let mp4_path = fixtures::ensure_sample_mp4();
    let path_str = Arc::new(mp4_path.to_str().unwrap().to_string());

    let mut handles = Vec::new();

    // Spawn 8 concurrent threads simultaneously decoding and seeking the same MP4 fixture
    for thread_id in 0..8 {
        let p = Arc::clone(&path_str);
        let handle = thread::spawn(move || {
            let mut dec = open_video_decoder(&p).expect("Thread decoder init");
            assert_eq!(dec.width(), 64);
            assert_eq!(dec.height(), 64);

            // Each thread performs random seeking and frame extraction
            let seeks = [0.2, 0.7, 0.0, 0.5, 0.9, 0.1];
            for &s in &seeks {
                dec.seek_seconds(s).expect("Thread seek");
                let frame = dec.next_frame().expect("Thread frame decode").expect("Frame");
                assert_eq!(frame.width, 64);
                assert_eq!(frame.height, 64);
                assert_eq!(frame.format, PixelFormat::Rgba8);
                assert!((frame.timestamp_seconds() - s).abs() < 0.25);
            }

            // Decode to EOS
            dec.seek_pts(0).expect("Rewind");
            let mut count = 0;
            while let Some(_) = dec.next_frame().expect("Decode") {
                count += 1;
            }
            assert_eq!(count, 10, "Thread {} decoded unexpected frame count", thread_id);
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().expect("Thread panicked during concurrent decoding stress test");
    }
}

// ============================================================================
// Stress Vector 7: Unified Factory & Composite Media Stress
// ============================================================================

#[test]
fn test_stress_unified_composite_media_decoder() {
    let mp4_path = fixtures::ensure_sample_mp4();
    let path_str = mp4_path.to_str().unwrap();

    let mut media = open_media_decoder(path_str).expect("Open media decoder");
    assert_eq!(media.media_type(), MediaType::Video);
    assert!(media.video().is_some());

    // Synchronous seeking across streams
    for &pts in &[5, 0, 8, 2, 9, 1] {
        media.seek_pts(pts).expect("Media seek PTS");
        let vframe = media.next_video_frame().unwrap().expect("Video frame");
        assert_eq!(vframe.width, 64);
        assert_eq!(vframe.height, 64);
        assert_eq!(vframe.format, PixelFormat::Rgba8);
    }
}

// ============================================================================
// Stress Vector 8: Floating Point Extremes (NaN, Inf, -Inf) & Sanitization
// ============================================================================

#[test]
fn test_stress_boundary_float_extremes_nan_inf() {
    let mp4_path = fixtures::ensure_sample_mp4();
    let path_str = mp4_path.to_str().expect("Valid path");
    let mut video_dec = open_video_decoder(path_str).expect("Open MP4");

    // 1. NaN seconds should convert to PTS 0 or clamp safely without panic
    let nan_seek = video_dec.seek_seconds(f64::NAN);
    assert!(nan_seek.is_ok(), "NaN seek should clamp/handle safely, got {:?}", nan_seek);
    let frame_nan = video_dec.next_frame().unwrap().expect("Frame after NaN seek");
    assert!(frame_nan.timestamp_seconds() < 0.15);

    // 2. Negative Infinity should clamp to 0
    let neg_inf_seek = video_dec.seek_seconds(f64::NEG_INFINITY);
    assert!(neg_inf_seek.is_ok(), "-Inf seek should clamp to 0, got {:?}", neg_inf_seek);
    let frame_neg_inf = video_dec.next_frame().unwrap().expect("Frame after -Inf seek");
    assert!(frame_neg_inf.timestamp_seconds() < 0.15);

    // 3. Positive Infinity should seek to duration / EOF
    let pos_inf_seek = video_dec.seek_seconds(f64::INFINITY);
    assert!(pos_inf_seek.is_ok(), "+Inf seek should handle safely, got {:?}", pos_inf_seek);
    let past = video_dec.next_frame().unwrap();
    assert!(past.is_none(), "Frame after +Inf seek should be None (EOS)");
}

// ============================================================================
// Stress Vector 9: High-Volume Frame Drain & Memory Stress
// ============================================================================

#[test]
fn test_stress_high_volume_frame_drain_and_no_leak() {
    let mp4_path = fixtures::ensure_sample_mp4();
    let path_str = mp4_path.to_str().expect("Valid path");
    let mut video_dec = open_video_decoder(path_str).expect("Open MP4");

    // Loop 50 cycles: decode 10 frames = 500 total video frames decoded & converted
    let mut total_decoded = 0;
    for _ in 0..50 {
        video_dec.seek_pts(0).expect("Rewind");
        while let Some(mut frame) = video_dec.next_frame().expect("Next frame") {
            frame.ensure_rgba8();
            assert_eq!(frame.format, PixelFormat::Rgba8);
            assert_eq!(frame.width, 64);
            assert_eq!(frame.height, 64);
            total_decoded += 1;
        }
    }
    assert_eq!(total_decoded, 500);
}

// ============================================================================
// Stress Vector 10: Corrupt Media File Rejection Without Crash
// ============================================================================

#[test]
fn test_stress_corrupted_media_bytes_error_handling() {
    let dir = tempdir().unwrap();

    // 1. Pseudo-MP4 containing pure random garbage
    let corrupt_mp4 = dir.path().join("corrupt_garbage.mp4");
    let garbage: Vec<u8> = (0..2048).map(|i| (i * 37 % 256) as u8).collect();
    fs::write(&corrupt_mp4, &garbage).unwrap();

    let path_str = corrupt_mp4.to_str().unwrap();
    let dec_res = open_media_decoder(path_str);
    assert!(
        dec_res.is_err(),
        "Corrupted MP4 must return Err, got Ok"
    );

    // 2. Pseudo-WAV containing pure random garbage
    let corrupt_wav = dir.path().join("corrupt_garbage.wav");
    fs::write(&corrupt_wav, &garbage).unwrap();

    let wav_res = open_media_decoder(corrupt_wav.to_str().unwrap());
    assert!(
        wav_res.is_err(),
        "Corrupted WAV must return Err, got Ok"
    );
}

// ============================================================================
// Stress Vector 11: Image Decoder Video Stream Seeking & Boundary Stress
// ============================================================================

#[test]
fn test_stress_open_media_decoder_image_seeking_bounds() {
    let dir = tempdir().unwrap();
    let png_path = dir.path().join("stress_slide.png");
    let png_bytes = fixtures::create_synthetic_png(128, 128);
    fs::write(&png_path, png_bytes).unwrap();

    let mut media = open_media_decoder(png_path.to_str().unwrap()).expect("Open PNG as media");
    assert_eq!(media.media_type(), MediaType::Image);

    let video = media.video().expect("Image exposes video interface");
    assert_eq!(video.duration_pts(), 300); // 5.0s @ 60 FPS

    // Seek within bounds: frame pts is preserved or advances
    video.seek_pts(150).expect("Seek image stream to 150 PTS");
    let f1 = video.next_frame().unwrap().expect("Frame at 150");
    assert_eq!(f1.width, 128);
    assert_eq!(f1.height, 128);

    // Negative seek clamps safely
    video.seek_pts(-50).expect("Seek negative on image");
    let f0 = video.next_frame().unwrap().expect("Frame at 0");
    assert_eq!(f0.pts, 0);

    // Seek past duration returns None (EOS)
    video.seek_pts(1000).expect("Seek past duration on image");
    let past = video.next_frame().unwrap();
    assert!(past.is_none(), "Image stream past duration must return None");
}

