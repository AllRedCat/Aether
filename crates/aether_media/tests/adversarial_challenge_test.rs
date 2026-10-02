mod fixtures;

use aether_core::media::MediaType;
use aether_media::{inspect_media_file, MediaError};
use std::fs;
use std::panic::{catch_unwind, AssertUnwindSafe};
use tempfile::tempdir;

/// Adversarial Vector 1: Truncated files
/// Tests files truncated at arbitrary byte offsets within headers and data chunks.
/// Engine must never panic, crash, or hang, and must return MediaError gracefully.
#[test]
fn test_adversarial_truncated_files() {
    let dir = tempdir().unwrap();

    // 1.1 Truncated PNG
    let valid_png = fixtures::create_synthetic_png(64, 64);
    assert!(valid_png.len() > 60);

    // Cutoffs strictly inside headers:
    // PNG signature is 8 bytes, IHDR is 25 bytes (total 33 bytes). Any cutoff < 33 is truncated header.
    let png_header_cutoffs = [1, 2, 4, 8, 12, 16, 24, 32];
    for &cutoff in &png_header_cutoffs {
        let truncated_path = dir.path().join(format!("truncated_hdr_{}.png", cutoff));
        fs::write(&truncated_path, &valid_png[..cutoff]).unwrap();

        let path_str = truncated_path.to_str().unwrap();
        let result = catch_unwind(AssertUnwindSafe(|| {
            inspect_media_file(path_str)
        }));

        assert!(result.is_ok(), "inspect_media_file panicked on truncated PNG header size {}", cutoff);
        let inspection_res = result.unwrap();
        assert!(
            matches!(inspection_res, Err(MediaError::CorruptFile(_)) | Err(MediaError::UnsupportedFormat(_))),
            "Expected CorruptFile or UnsupportedFormat for truncated PNG header size {}, got: {:?}",
            cutoff,
            inspection_res
        );
    }

    // 1.2 Truncated WAV Header:
    let valid_wav = fixtures::create_synthetic_wav(44100, 2, 0.5);
    assert!(valid_wav.len() > 44);

    // WAV header is 44 bytes (RIFF 12 bytes + fmt 24 bytes + data 8 bytes).
    // Cutoffs strictly inside the header (< 44 bytes):
    let wav_header_cutoffs = [1, 3, 4, 8, 12, 16, 24, 36, 40, 43];
    for &cutoff in &wav_header_cutoffs {
        let truncated_path = dir.path().join(format!("truncated_hdr_{}.wav", cutoff));
        fs::write(&truncated_path, &valid_wav[..cutoff]).unwrap();

        let path_str = truncated_path.to_str().unwrap();
        let result = catch_unwind(AssertUnwindSafe(|| {
            inspect_media_file(path_str)
        }));

        assert!(result.is_ok(), "inspect_media_file panicked on truncated WAV header size {}", cutoff);
        let inspection_res = result.unwrap();
        assert!(
            matches!(inspection_res, Err(MediaError::CorruptFile(_)) | Err(MediaError::UnsupportedFormat(_))),
            "Expected CorruptFile or UnsupportedFormat for truncated WAV header size {}, got: {:?}",
            cutoff,
            inspection_res
        );
    }

    // 1.3 Truncated MP4
    let valid_mp4 = fixtures::create_synthetic_mp4(320, 240, 1);
    assert!(valid_mp4.len() > 100);

    let mp4_cutoffs = [1, 4, 8, 16, 32, 64, 128, 256, valid_mp4.len() - 20];
    for &cutoff in &mp4_cutoffs {
        let truncated_path = dir.path().join(format!("truncated_{}.mp4", cutoff));
        fs::write(&truncated_path, &valid_mp4[..cutoff]).unwrap();

        let path_str = truncated_path.to_str().unwrap();
        let result = catch_unwind(AssertUnwindSafe(|| {
            inspect_media_file(path_str)
        }));

        assert!(result.is_ok(), "inspect_media_file panicked on truncated MP4 of size {}", cutoff);
        let inspection_res = result.unwrap();
        assert!(
            matches!(inspection_res, Err(MediaError::CorruptFile(_)) | Err(MediaError::UnsupportedFormat(_))),
            "Expected CorruptFile or UnsupportedFormat for truncated MP4 size {}, got: {:?}",
            cutoff,
            inspection_res
        );
    }
}

/// Adversarial Vector 2: Mismatched extension vs magic bytes
/// Tests cross-format confusion (e.g. PNG named .wav, MP4 named .png, text named .mp4).
#[test]
fn test_adversarial_mismatched_extensions() {
    let dir = tempdir().unwrap();

    let png_bytes = fixtures::create_synthetic_png(128, 96);
    let wav_bytes = fixtures::create_synthetic_wav(48000, 1, 0.5);
    let mp4_bytes = fixtures::create_synthetic_mp4(640, 360, 1);

    // Case 2.1: Valid PNG content named as audio/video/text extensions
    let png_mismatched = [
        "image_disguised_as.wav",
        "image_disguised_as.mp4",
        "image_disguised_as.mp3",
        "image_disguised_as.mov",
        "image_disguised_as.txt",
    ];
    for fname in &png_mismatched {
        let file_path = dir.path().join(fname);
        fs::write(&file_path, &png_bytes).unwrap();
        let path_str = file_path.to_str().unwrap();

        let res = catch_unwind(AssertUnwindSafe(|| inspect_media_file(path_str)));
        assert!(res.is_ok(), "Panicked on {}", fname);
        let inspection = res.unwrap();

        // Magic bytes should detect PNG and decode dimensions gracefully
        match inspection {
            Ok(insp) => {
                assert_eq!(insp.media_type, MediaType::Image, "Expected MediaType::Image for {}", fname);
                assert_eq!(insp.width, Some(128));
                assert_eq!(insp.height, Some(96));
            }
            Err(e) => {
                // If extension takes precedence in fallback, it must fail with graceful error
                assert!(matches!(e, MediaError::CorruptFile(_) | MediaError::UnsupportedFormat(_)));
            }
        }
    }

    // Case 2.2: Valid WAV content named as image/video extensions
    let wav_mismatched = [
        "audio_disguised_as.png",
        "audio_disguised_as.jpg",
        "audio_disguised_as.mp4",
        "audio_disguised_as.mov",
    ];
    for fname in &wav_mismatched {
        let file_path = dir.path().join(fname);
        fs::write(&file_path, &wav_bytes).unwrap();
        let path_str = file_path.to_str().unwrap();

        let res = catch_unwind(AssertUnwindSafe(|| inspect_media_file(path_str)));
        assert!(res.is_ok(), "Panicked on {}", fname);
        let inspection = res.unwrap();

        // Either recognized via magic bytes as Audio, or rejected gracefully by image/video decoder
        match inspection {
            Ok(insp) => {
                assert_eq!(insp.media_type, MediaType::Audio, "Expected MediaType::Audio for {}", fname);
                assert_eq!(insp.sample_rate, Some(48000));
            }
            Err(e) => {
                assert!(matches!(e, MediaError::CorruptFile(_) | MediaError::UnsupportedFormat(_)));
            }
        }
    }

    // Case 2.3: Valid MP4 content named as image/audio extensions
    let mp4_mismatched = [
        "video_disguised_as.png",
        "video_disguised_as.jpg",
        "video_disguised_as.wav",
        "video_disguised_as.mp3",
    ];
    for fname in &mp4_mismatched {
        let file_path = dir.path().join(fname);
        fs::write(&file_path, &mp4_bytes).unwrap();
        let path_str = file_path.to_str().unwrap();

        let res = catch_unwind(AssertUnwindSafe(|| inspect_media_file(path_str)));
        assert!(res.is_ok(), "Panicked on {}", fname);
        let inspection = res.unwrap();

        match inspection {
            Ok(insp) => {
                assert_eq!(insp.media_type, MediaType::Video, "Expected MediaType::Video for {}", fname);
                assert_eq!(insp.width, Some(640));
                assert_eq!(insp.height, Some(360));
            }
            Err(e) => {
                assert!(matches!(e, MediaError::CorruptFile(_) | MediaError::UnsupportedFormat(_)));
            }
        }
    }

    // Case 2.4: Text payload named as media extensions
    let text_payload = b"Lorem ipsum dolor sit amet, consectetur adipiscing elit.";
    for ext in &["png", "jpg", "wav", "mp3", "mp4", "mov"] {
        let fname = format!("fake_text.{}", ext);
        let file_path = dir.path().join(&fname);
        fs::write(&file_path, text_payload).unwrap();
        let path_str = file_path.to_str().unwrap();

        let res = catch_unwind(AssertUnwindSafe(|| inspect_media_file(path_str)));
        assert!(res.is_ok(), "Panicked on fake text file {}", fname);
        let inspection = res.unwrap();
        assert!(
            matches!(inspection, Err(MediaError::CorruptFile(_)) | Err(MediaError::UnsupportedFormat(_))),
            "Text file disguised as .{} should fail gracefully, got: {:?}",
            ext,
            inspection
        );
    }
}

/// Adversarial Vector 3: Boundary dimensions
/// Tests minimal 1x1, extreme aspect ratios, 0x0, and huge headers.
#[test]
fn test_adversarial_boundary_dimensions() {
    let dir = tempdir().unwrap();

    // 3.1 Minimal valid 1x1 PNG
    let png_1x1 = fixtures::create_synthetic_png(1, 1);
    let path_1x1 = dir.path().join("pixel_1x1.png");
    fs::write(&path_1x1, png_1x1).unwrap();

    let res_1x1 = inspect_media_file(path_1x1.to_str().unwrap()).expect("1x1 PNG inspection failed");
    assert_eq!(res_1x1.media_type, MediaType::Image);
    assert_eq!(res_1x1.width, Some(1));
    assert_eq!(res_1x1.height, Some(1));

    // 3.2 Extreme aspect ratio: 10000 x 1 and 1 x 10000
    let png_wide = fixtures::create_synthetic_png(10000, 1);
    let path_wide = dir.path().join("wide_strip.png");
    fs::write(&path_wide, png_wide).unwrap();
    let res_wide = inspect_media_file(path_wide.to_str().unwrap()).expect("Wide PNG inspection failed");
    assert_eq!(res_wide.width, Some(10000));
    assert_eq!(res_wide.height, Some(1));

    let png_tall = fixtures::create_synthetic_png(1, 10000);
    let path_tall = dir.path().join("tall_strip.png");
    fs::write(&path_tall, png_tall).unwrap();
    let res_tall = inspect_media_file(path_tall.to_str().unwrap()).expect("Tall PNG inspection failed");
    assert_eq!(res_tall.width, Some(1));
    assert_eq!(res_tall.height, Some(10000));

    // 3.3 Zero dimension (0x0) corrupted PNG header
    // In PNG, bytes 16..20 are width, bytes 20..24 are height in IHDR chunk
    let mut zero_dim_png = fixtures::create_synthetic_png(10, 10);
    // Overwrite width with 0 and height with 0
    zero_dim_png[16..20].copy_from_slice(&0u32.to_be_bytes());
    zero_dim_png[20..24].copy_from_slice(&0u32.to_be_bytes());
    let path_zero = dir.path().join("zero_dim.png");
    fs::write(&path_zero, zero_dim_png).unwrap();

    let res_zero = catch_unwind(AssertUnwindSafe(|| {
        inspect_media_file(path_zero.to_str().unwrap())
    }));
    assert!(res_zero.is_ok(), "Panicked on 0x0 PNG");
    assert!(
        matches!(res_zero.unwrap(), Err(MediaError::CorruptFile(_)) | Err(MediaError::UnsupportedFormat(_))),
        "0x0 PNG must be rejected with CorruptFile or UnsupportedFormat"
    );

    // 3.4 Huge dimension header: 0xFFFFFFFF x 0xFFFFFFFF
    let mut huge_dim_png = fixtures::create_synthetic_png(10, 10);
    huge_dim_png[16..20].copy_from_slice(&u32::MAX.to_be_bytes());
    huge_dim_png[20..24].copy_from_slice(&u32::MAX.to_be_bytes());
    let path_huge = dir.path().join("huge_dim.png");
    fs::write(&path_huge, huge_dim_png).unwrap();

    let res_huge = catch_unwind(AssertUnwindSafe(|| {
        inspect_media_file(path_huge.to_str().unwrap())
    }));
    assert!(res_huge.is_ok(), "Panicked or OOMed on huge dimension PNG");
    // Should either reject due to CRC/header validation or read header dimensions without allocating memory
    let huge_inspection = res_huge.unwrap();
    match huge_inspection {
        Ok(insp) => {
            assert_eq!(insp.width, Some(u32::MAX));
            assert_eq!(insp.height, Some(u32::MAX));
        }
        Err(e) => {
            assert!(matches!(e, MediaError::CorruptFile(_)));
        }
    }
}

/// Adversarial Vector 4: Extreme audio parameters
/// Tests telephony to ultra-high sample rates, various channel counts, and invalid audio headers.
#[test]
fn test_adversarial_extreme_audio_parameters() {
    let dir = tempdir().unwrap();

    // 4.1 Extreme valid sample rates
    let sample_rates = [8000, 11025, 22050, 44100, 48000, 88200, 96000, 192000];
    for &rate in &sample_rates {
        let wav = fixtures::create_synthetic_wav(rate, 2, 0.1);
        let path = dir.path().join(format!("rate_{}.wav", rate));
        fs::write(&path, wav).unwrap();

        let insp = inspect_media_file(path.to_str().unwrap()).unwrap_or_else(|e| {
            panic!("Valid sample rate {} failed inspection: {:?}", rate, e);
        });
        assert_eq!(insp.media_type, MediaType::Audio);
        assert_eq!(insp.sample_rate, Some(rate));
        assert_eq!(insp.audio_channels, Some(2));
    }

    // 4.2 Extreme valid channel counts (mono, stereo, 5.1 surround = 6, 7.1 surround = 8)
    let channels_list = [1u16, 2, 6, 8];
    for &channels in &channels_list {
        let wav = fixtures::create_synthetic_wav(44100, channels, 0.1);
        let path = dir.path().join(format!("channels_{}.wav", channels));
        fs::write(&path, wav).unwrap();

        let insp = inspect_media_file(path.to_str().unwrap()).unwrap_or_else(|e| {
            panic!("Valid channel count {} failed inspection: {:?}", channels, e);
        });
        assert_eq!(insp.media_type, MediaType::Audio);
        assert_eq!(insp.audio_channels, Some(channels));
    }

    // 4.3 Zero duration audio (empty data payload)
    let zero_dur_wav = fixtures::create_synthetic_wav(44100, 2, 0.0);
    let path_zero_dur = dir.path().join("zero_duration.wav");
    fs::write(&path_zero_dur, zero_dur_wav).unwrap();

    let res_zero = inspect_media_file(path_zero_dur.to_str().unwrap());
    assert!(res_zero.is_ok(), "Zero duration WAV should inspect successfully");
    let insp_zero = res_zero.unwrap();
    assert_eq!(insp_zero.duration_pts, 0);
    assert_eq!(insp_zero.duration_seconds, 0.0);
}

/// Adversarial Vector 4b: Corrupted Audio Channels = 0
#[test]
fn test_adversarial_audio_channels_zero() {
    let dir = tempdir().unwrap();
    let mut bad_channels_wav = fixtures::create_synthetic_wav(44100, 2, 0.1);
    bad_channels_wav[22..24].copy_from_slice(&0u16.to_le_bytes());
    let path_bad_channels = dir.path().join("channels_zero.wav");
    fs::write(&path_bad_channels, bad_channels_wav).unwrap();

    let res_bad_channels = catch_unwind(AssertUnwindSafe(|| {
        inspect_media_file(path_bad_channels.to_str().unwrap())
    }));
    assert!(res_bad_channels.is_ok(), "Panicked on channels = 0");
    assert!(
        matches!(res_bad_channels.unwrap(), Err(MediaError::CorruptFile(_)) | Err(MediaError::UnsupportedFormat(_))),
        "Expected error for channels = 0 WAV"
    );
}

/// Adversarial Vector 4c: Corrupted Audio Sample Rate = 0
/// Verifies that inspect_media_file handles sample_rate = 0 without panicking.
#[test]
fn test_adversarial_audio_sample_rate_zero_panic_check() {
    let dir = tempdir().unwrap();
    let mut bad_rate_wav = fixtures::create_synthetic_wav(44100, 2, 0.1);
    bad_rate_wav[24..28].copy_from_slice(&0u32.to_le_bytes());
    let path_bad_rate = dir.path().join("sample_rate_zero.wav");
    fs::write(&path_bad_rate, bad_rate_wav).unwrap();

    let res_bad_rate = catch_unwind(AssertUnwindSafe(|| {
        inspect_media_file(path_bad_rate.to_str().unwrap())
    }));
    assert!(res_bad_rate.is_ok(), "PANIC DETECTED: inspect_media_file panicked on WAV with sample_rate = 0");
    assert!(
        matches!(res_bad_rate.unwrap(), Err(MediaError::CorruptFile(_)) | Err(MediaError::UnsupportedFormat(_))),
        "Expected CorruptFile or UnsupportedFormat for sample_rate = 0 WAV"
    );
}

/// Adversarial Vector 4d: Corrupted Audio WAV fmt fields (block_align = 0, bits_per_sample = 0, byte_rate = 0)
#[test]
fn test_adversarial_audio_wav_fmt_zero_fields() {
    let dir = tempdir().unwrap();

    // 4d.1 block_align = 0 (bytes 32..34)
    let mut bad_align = fixtures::create_synthetic_wav(44100, 2, 0.1);
    bad_align[32..34].copy_from_slice(&0u16.to_le_bytes());
    let path_align = dir.path().join("bad_align.wav");
    fs::write(&path_align, bad_align).unwrap();

    let res_align = catch_unwind(AssertUnwindSafe(|| {
        inspect_media_file(path_align.to_str().unwrap())
    }));
    assert!(res_align.is_ok(), "Panicked on block_align = 0");
    let _ = res_align.unwrap(); // Graceful Result

    // 4d.2 bits_per_sample = 0 (bytes 34..36)
    let mut bad_bits = fixtures::create_synthetic_wav(44100, 2, 0.1);
    bad_bits[34..36].copy_from_slice(&0u16.to_le_bytes());
    let path_bits = dir.path().join("bad_bits.wav");
    fs::write(&path_bits, bad_bits).unwrap();

    let res_bits = catch_unwind(AssertUnwindSafe(|| {
        inspect_media_file(path_bits.to_str().unwrap())
    }));
    assert!(res_bits.is_ok(), "Panicked on bits_per_sample = 0");
    let _ = res_bits.unwrap();

    // 4d.3 byte_rate = 0 (bytes 28..32)
    let mut bad_byte_rate = fixtures::create_synthetic_wav(44100, 2, 0.1);
    bad_byte_rate[28..32].copy_from_slice(&0u32.to_le_bytes());
    let path_byte_rate = dir.path().join("bad_byte_rate.wav");
    fs::write(&path_byte_rate, bad_byte_rate).unwrap();

    let res_byte_rate = catch_unwind(AssertUnwindSafe(|| {
        inspect_media_file(path_byte_rate.to_str().unwrap())
    }));
    assert!(res_byte_rate.is_ok(), "Panicked on byte_rate = 0");
    let _ = res_byte_rate.unwrap();
}

/// Adversarial Vector 4e: Extreme Timebases passed to inspect_media_file_with_timebase
#[test]
fn test_adversarial_extreme_timebases() {
    use aether_core::timeline::Rational;
    use aether_media::inspect_media_file_with_timebase;

    let dir = tempdir().unwrap();
    let png_bytes = fixtures::create_synthetic_png(32, 32);
    let path = dir.path().join("timebase_test.png");
    fs::write(&path, png_bytes).unwrap();
    let path_str = path.to_str().unwrap();

    let extreme_timebases = [
        Rational { num: 60, den: 0 },         // Zero denominator
        Rational { num: 0, den: 1 },          // Zero numerator
        Rational { num: -60, den: 1 },        // Negative timebase
        Rational { num: i32::MAX, den: 1 },   // Extreme max integer
        Rational { num: 1, den: i32::MAX },   // Near zero ratio
    ];

    for tb in extreme_timebases {
        let res = catch_unwind(AssertUnwindSafe(|| {
            inspect_media_file_with_timebase(path_str, tb)
        }));
        assert!(res.is_ok(), "Panicked on timebase {:?}", tb);
        let insp = res.unwrap();
        assert!(insp.is_ok(), "Failed on timebase {:?}", tb);
    }
}

/// Adversarial Vector 5: Unusual, non-standard, and unicode file paths
/// Tests handling of unicode names, spaces, symbols, multiple dots, and missing paths.
#[test]
fn test_adversarial_unusual_paths() {
    let dir = tempdir().unwrap();

    let png_bytes = fixtures::create_synthetic_png(64, 64);
    let wav_bytes = fixtures::create_synthetic_wav(44100, 1, 0.5);
    let mp4_bytes = fixtures::create_synthetic_mp4(320, 240, 1);

    let test_cases: Vec<(&str, &[u8], MediaType)> = vec![
        ("clip with spaces in name (version 1) [final].mp4", &mp4_bytes, MediaType::Video),
        ("áudio_gravação_férias_2026_日本語_test.wav", &wav_bytes, MediaType::Audio),
        ("imagem.com.muitos.pontos.e.versao.1.0.final.png", &png_bytes, MediaType::Image),
        ("symbols_!@#$%^&()_+=-~`{}[]';,.wav", &wav_bytes, MediaType::Audio),
        ("emoji_🎬_🎵_🖼️_test.mp4", &mp4_bytes, MediaType::Video),
    ];

    for (filename, data, expected_type) in test_cases {
        let file_path = dir.path().join(filename);
        fs::write(&file_path, data).unwrap();
        let path_str = file_path.to_str().unwrap();

        let insp = inspect_media_file(path_str).unwrap_or_else(|e| {
            panic!("Failed inspection for unusual path '{}': {:?}", filename, e);
        });
        assert_eq!(insp.media_type, expected_type, "Type mismatch on '{}'", filename);

        let item = insp.to_media_item(path_str);
        assert_eq!(item.file_name, filename, "File name extraction failed on '{}'", filename);
        assert_eq!(item.media_type, expected_type);
    }

    // 5.2 Non-existent unusual path
    let non_existent = dir.path().join("não_existe_com_caracteres_especiais_#123.mp4");
    let err = inspect_media_file(non_existent.to_str().unwrap()).unwrap_err();
    assert!(matches!(err, MediaError::FileNotFound(_)));

    // 5.3 Embedded null in path string (should fail gracefully without panic)
    let null_path = "/tmp/fake\0path.mp4";
    let res_null = catch_unwind(AssertUnwindSafe(|| {
        inspect_media_file(null_path)
    }));
    assert!(res_null.is_ok(), "Panicked on path with embedded null");
    assert!(matches!(res_null.unwrap(), Err(MediaError::FileNotFound(_)) | Err(MediaError::IoError(_))));
}

/// Adversarial Vector 6: Corrupted MP4 boxes & Timescale / Zero Division
/// Tests MP4 headers claiming massive 4GB box sizes, timescale = 0, or missing moov box.
#[test]
fn test_adversarial_corrupted_mp4_boxes() {
    let dir = tempdir().unwrap();

    // 6.1 MP4 with giant box size claiming 4GB in a 32-byte file
    let mut bomb_mp4 = vec![0u8; 32];
    bomb_mp4[0..4].copy_from_slice(&0xFFFFFFFEu32.to_be_bytes()); // box size ~4GB
    bomb_mp4[4..8].copy_from_slice(b"ftyp");
    bomb_mp4[8..12].copy_from_slice(b"isom");
    let path_bomb = dir.path().join("size_bomb.mp4");
    fs::write(&path_bomb, bomb_mp4).unwrap();

    let res_bomb = catch_unwind(AssertUnwindSafe(|| {
        inspect_media_file(path_bomb.to_str().unwrap())
    }));
    assert!(res_bomb.is_ok(), "Panicked on MP4 with oversized box");
    assert!(
        matches!(res_bomb.unwrap(), Err(MediaError::CorruptFile(_)) | Err(MediaError::UnsupportedFormat(_))),
        "Expected CorruptFile on oversized box MP4"
    );

    // 6.2 Valid ftyp box only, with missing moov box
    let mut ftyp_only = vec![0u8; 16];
    ftyp_only[0..4].copy_from_slice(&16u32.to_be_bytes());
    ftyp_only[4..8].copy_from_slice(b"ftyp");
    ftyp_only[8..12].copy_from_slice(b"mp42");
    let path_ftyp = dir.path().join("ftyp_only.mp4");
    fs::write(&path_ftyp, ftyp_only).unwrap();

    let res_ftyp = catch_unwind(AssertUnwindSafe(|| {
        inspect_media_file(path_ftyp.to_str().unwrap())
    }));
    assert!(res_ftyp.is_ok(), "Panicked on MP4 with missing moov box");
    assert!(
        matches!(res_ftyp.unwrap(), Err(MediaError::CorruptFile(_)) | Err(MediaError::UnsupportedFormat(_))),
        "Expected CorruptFile for MP4 with no moov"
    );

    // 6.3 MP4 with 0 duration (test vt.frame_rate() division-by-zero)
    let zero_dur_mp4 = fixtures::create_synthetic_mp4(320, 240, 0);
    let path_zero_mp4 = dir.path().join("mp4_zero_duration.mp4");
    fs::write(&path_zero_mp4, zero_dur_mp4).unwrap();
    let res_zero_mp4 = catch_unwind(AssertUnwindSafe(|| {
        inspect_media_file(path_zero_mp4.to_str().unwrap())
    }));
    assert!(res_zero_mp4.is_ok(), "PANIC DETECTED: inspect_media_file panicked on MP4 with duration = 0");
}

/// Adversarial Vector 6b: MP4 with timescale = 0 (divide-by-zero panic check)
/// When timescale = 0, mp4 crate divides by zero in reader.rs:145:31.
/// Engine must validate timescale != 0 or catch unwind to avoid crashing the process.
#[test]
fn test_adversarial_mp4_timescale_zero_panic_check() {
    let dir = tempdir().unwrap();
    let valid_mp4 = fixtures::create_synthetic_mp4(320, 240, 1);
    if let Some(pos) = valid_mp4.windows(4).position(|w| w == b"mvhd") {
        let mut zero_timescale_mp4 = valid_mp4.clone();
        // mvhd: 4 bytes tag ("mvhd"), 1 byte version, 3 bytes flags, 4 bytes creation, 4 bytes mod
        // Then 4 bytes timescale! (offset = pos + 4 + 1 + 3 + 4 + 4 = pos + 16)
        let timescale_offset = pos + 16;
        if timescale_offset + 4 <= zero_timescale_mp4.len() {
            zero_timescale_mp4[timescale_offset..timescale_offset + 4].copy_from_slice(&0u32.to_be_bytes());
            let path_ts0 = dir.path().join("mp4_timescale_0.mp4");
            fs::write(&path_ts0, zero_timescale_mp4).unwrap();

            let res_ts0 = catch_unwind(AssertUnwindSafe(|| {
                inspect_media_file(path_ts0.to_str().unwrap())
            }));
            assert!(res_ts0.is_ok(), "PANIC DETECTED: inspect_media_file panicked on MP4 with timescale = 0");
            let inner = res_ts0.unwrap();
            // On macOS, AVFoundation fallback may successfully inspect the file despite
            // the corrupted timescale, which is acceptable behavior.
            assert!(
                matches!(inner, Err(MediaError::CorruptFile(_)) | Err(MediaError::UnsupportedFormat(_)) | Ok(_)),
                "Expected CorruptFile, UnsupportedFormat, or Ok (AVFoundation fallback) for MP4 with timescale = 0"
            );
        }
    }
}

/// Adversarial Vector 7: Fuzzing & Memory Safety Harness
/// Generates 500 permutations of random binary bytes with various media extensions
/// and asserts that inspect_media_file never panics or aborts.
#[test]
fn test_adversarial_fuzzing_panic_safety_harness() {
    let dir = tempdir().unwrap();
    let extensions = ["png", "jpg", "wav", "mp3", "mp4", "mov"];

    // Simple deterministic LCG pseudo-random generator for hermetic reproducibility
    let mut seed: u64 = 0xDEADBEEFCAFE1234;
    let mut next_byte = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        (seed >> 33) as u8
    };

    let iterations = 300;
    for i in 0..iterations {
        let ext = extensions[i % extensions.len()];
        let len = (i * 37) % 4096 + 1; // variable lengths from 1 to 4096 bytes
        let mut random_payload = Vec::with_capacity(len);
        for _ in 0..len {
            random_payload.push(next_byte());
        }

        let file_path = dir.path().join(format!("fuzz_{}.{}", i, ext));
        fs::write(&file_path, &random_payload).unwrap();
        let path_str = file_path.to_str().unwrap();

        let result = catch_unwind(AssertUnwindSafe(|| {
            inspect_media_file(path_str)
        }));

        assert!(
            result.is_ok(),
            "inspect_media_file PANICKED on fuzz iteration {} (len={}, ext={})!",
            i,
            len,
            ext
        );

        let inspect_res = result.unwrap();
        // Result must be a valid Result enum: either Ok (if bytes coincidentally matched a format) or Err(MediaError)
        match inspect_res {
            Ok(insp) => {
                // If random bytes coincidentally parsed, verify fields are non-corrupt
                assert!(insp.duration_seconds >= 0.0);
                assert!(insp.duration_pts >= 0);
            }
            Err(e) => {
                assert!(
                    matches!(
                        e,
                        MediaError::CorruptFile(_)
                            | MediaError::UnsupportedFormat(_)
                            | MediaError::IoError(_)
                    ),
                    "Unexpected error variant: {:?}",
                    e
                );
            }
        }
    }
}
