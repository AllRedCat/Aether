use aether_bridge::api;

use std::fs;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;
use uuid::Uuid;

use aether_core::media::{MediaItem, MediaMetadata, MediaType};
use aether_core::project::Project;
use aether_core::timeline::{Rational, TrackKind};

/// Helper to create a unique temporary directory in std::env::temp_dir()
struct TestDir {
    path: PathBuf,
}

impl TestDir {
    fn new(suffix: &str) -> Self {
        let dir_name = format!("aether_bridge_test_{}_{}", suffix, Uuid::new_v4());
        let path = std::env::temp_dir().join(dir_name);
        fs::create_dir_all(&path).expect("Failed to create test directory");
        Self { path }
    }

    fn file(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// Helper to create a minimal valid 1x1 PNG in memory without external crates
fn create_synthetic_png(_width: u32, _height: u32) -> Vec<u8> {
    vec![
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, // PNG signature (8 bytes)
        0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52, // IHDR chunk (13 bytes)
        0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, // 1x1
        0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4, 0x89, // CRC
        0x00, 0x00, 0x00, 0x0a, 0x49, 0x44, 0x41, 0x54, // IDAT chunk
        0x78, 0x9c, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 
        0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4,
        0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, // IEND chunk
        0xae, 0x42, 0x60, 0x82,
    ]
}

/// Helper to create a minimal valid WAV in memory
fn create_synthetic_wav(sample_rate: u32, channels: u16, duration_seconds: f32) -> Vec<u8> {
    let mut buf = Vec::new();
    let num_samples = (sample_rate as f32 * duration_seconds * channels as f32) as u32;
    let data_len = num_samples * 2; // 16-bit PCM = 2 bytes per sample
    let file_len = 36 + data_len;

    // RIFF Header
    buf.extend_from_slice(b"RIFF");
    buf.extend_from_slice(&file_len.to_le_bytes());
    buf.extend_from_slice(b"WAVE");

    // fmt chunk
    buf.extend_from_slice(b"fmt ");
    buf.extend_from_slice(&16u32.to_le_bytes()); // Chunk size: 16
    buf.extend_from_slice(&1u16.to_le_bytes()); // Audio format: PCM (1)
    buf.extend_from_slice(&channels.to_le_bytes());
    buf.extend_from_slice(&sample_rate.to_le_bytes());
    let byte_rate = sample_rate * channels as u32 * 2;
    buf.extend_from_slice(&byte_rate.to_le_bytes());
    let block_align = channels * 2;
    buf.extend_from_slice(&block_align.to_le_bytes());
    buf.extend_from_slice(&16u16.to_le_bytes()); // Bits per sample: 16

    // data chunk
    buf.extend_from_slice(b"data");
    buf.extend_from_slice(&data_len.to_le_bytes());
    buf.resize(buf.len() + data_len as usize, 0u8);

    buf
}

fn dummy_media_item(media_type: MediaType, duration_pts: i64) -> MediaItem {
    MediaItem {
        id: Uuid::new_v4(),
        file_path: "/dummy/path.mp4".to_string(),
        file_name: "path.mp4".to_string(),
        media_type,
        metadata: MediaMetadata {
            width: Some(1920),
            height: Some(1080),
            duration_pts,
            duration_seconds: duration_pts as f64 / 60.0,
            timebase: Some(Rational { num: 60, den: 1 }),
            audio_channels: Some(2),
            sample_rate: Some(48000),
            file_size_bytes: 1024, video_codec: None, audio_codec: None, pixel_format: None, is_vfr: None, keyframe_pts: None,
        },
    }
}

// =========================================================================
// VECTOR 1: Empty, Whitespace, Nonexistent, and Directory Path Handling
// =========================================================================

#[test]
fn test_import_media_empty_and_whitespace_paths() {
    let empty_inputs = ["", "   ", "\t\t", "\n\r\n", "  \t \n "];
    for input in empty_inputs {
        let res = api::import_media_file(None, input.to_string());
        assert!(res.is_err(), "Expected Err for empty input '{:?}'", input);
        assert_eq!(
            res.unwrap_err(),
            "File path cannot be empty",
            "Expected exact error message for input '{:?}'",
            input
        );

        let inspect_res = api::inspect_media_file(input.to_string());
        assert!(inspect_res.is_err());
        assert_eq!(inspect_res.unwrap_err(), "File path cannot be empty");
    }
}

#[test]
fn test_import_media_nonexistent_paths() {
    let nonexistent_paths = [
        "/tmp/non_existent_aether_file_987654.mp4",
        "relative/non_existent/path.png",
        "/var/root/forbidden/video.mov",
    ];
    for path in nonexistent_paths {
        let res = api::import_media_file(None, path.to_string());
        assert!(res.is_err(), "Expected Err for nonexistent path: {}", path);
        let err = res.unwrap_err();
        assert!(
            err.contains("File does not exist"),
            "Expected 'File does not exist' in error: {}",
            err
        );

        let inspect_res = api::inspect_media_file(path.to_string());
        assert!(inspect_res.is_err());
        assert!(inspect_res.unwrap_err().contains("File does not exist"));
    }
}

#[test]
fn test_import_media_directory_paths() {
    let test_dir = TestDir::new("dir_check");
    let dir_path = test_dir.path.to_str().unwrap().to_string();

    let res = api::import_media_file(None, dir_path.clone());
    assert!(res.is_err(), "Expected Err for directory path");
    let err = res.unwrap_err();
    assert!(
        err.contains("Path is not a regular file"),
        "Expected 'Path is not a regular file' in error: {}",
        err
    );

    let inspect_res = api::inspect_media_file(dir_path);
    assert!(inspect_res.is_err());
    assert!(inspect_res.unwrap_err().contains("Path is not a regular file"));
}

// =========================================================================
// VECTOR 2: Corrupted Media Files via import_media_file
// =========================================================================

#[test]
fn test_import_media_zero_byte_file() {
    let test_dir = TestDir::new("zero_byte");
    let empty_file = test_dir.file("empty.mp4");
    fs::write(&empty_file, b"").unwrap();

    let res = api::import_media_file(None, empty_file.to_str().unwrap().to_string());
    assert!(res.is_err(), "Expected Err for zero-byte file");
    let err = res.unwrap_err();
    assert!(
        err.contains("File is empty") || err.contains("0 bytes") || err.contains("Failed to inspect"),
        "Expected empty file error, got: {}",
        err
    );
}

#[test]
fn test_import_media_truncated_png_headers() {
    let test_dir = TestDir::new("trunc_png");
    let valid_png = create_synthetic_png(1, 1);

    let cutoffs = [1, 2, 4, 8, 12, 16, 24];
    for &cutoff in &cutoffs {
        if cutoff >= valid_png.len() {
            continue;
        }
        let trunc_path = test_dir.file(&format!("trunc_{}.png", cutoff));
        fs::write(&trunc_path, &valid_png[..cutoff]).unwrap();

        let path_str = trunc_path.to_str().unwrap().to_string();
        let res = catch_unwind(AssertUnwindSafe(|| {
            api::import_media_file(None, path_str)
        }));

        assert!(res.is_ok(), "import_media_file panicked on truncated PNG cutoff {}", cutoff);
        let inner_res = res.unwrap();
        assert!(inner_res.is_err(), "Expected Err for truncated PNG cutoff {}", cutoff);
    }
}

#[test]
fn test_import_media_truncated_wav_headers() {
    let test_dir = TestDir::new("trunc_wav");
    let valid_wav = create_synthetic_wav(44100, 2, 0.1);

    let cutoffs = [1, 4, 12, 16, 24, 36, 40];
    for &cutoff in &cutoffs {
        let trunc_path = test_dir.file(&format!("trunc_{}.wav", cutoff));
        fs::write(&trunc_path, &valid_wav[..cutoff]).unwrap();

        let path_str = trunc_path.to_str().unwrap().to_string();
        let res = catch_unwind(AssertUnwindSafe(|| {
            api::import_media_file(None, path_str)
        }));

        assert!(res.is_ok(), "import_media_file panicked on truncated WAV cutoff {}", cutoff);
        let inner_res = res.unwrap();
        assert!(inner_res.is_err(), "Expected Err for truncated WAV cutoff {}", cutoff);
    }
}

#[test]
fn test_import_media_corrupted_audio_zero_parameters() {
    let test_dir = TestDir::new("corrupt_audio_zero");

    // WAV with 0 channels
    let mut wav_0_channels = create_synthetic_wav(44100, 2, 0.1);
    wav_0_channels[22] = 0;
    wav_0_channels[23] = 0;
    let path_0_ch = test_dir.file("zero_channels.wav");
    fs::write(&path_0_ch, &wav_0_channels).unwrap();

    let res_ch = catch_unwind(AssertUnwindSafe(|| {
        api::import_media_file(None, path_0_ch.to_str().unwrap().to_string())
    }));
    assert!(res_ch.is_ok(), "Panic on 0 channels audio!");
    assert!(res_ch.unwrap().is_err());

    // WAV with 0 sample rate
    let mut wav_0_sr = create_synthetic_wav(44100, 2, 0.1);
    wav_0_sr[24] = 0;
    wav_0_sr[25] = 0;
    wav_0_sr[26] = 0;
    wav_0_sr[27] = 0;
    let path_0_sr = test_dir.file("zero_sample_rate.wav");
    fs::write(&path_0_sr, &wav_0_sr).unwrap();

    let res_sr = catch_unwind(AssertUnwindSafe(|| {
        api::import_media_file(None, path_0_sr.to_str().unwrap().to_string())
    }));
    assert!(res_sr.is_ok(), "Panic on 0 sample rate audio!");
    assert!(res_sr.unwrap().is_err());
}

#[test]
fn test_import_media_pseudo_random_fuzzing() {
    let test_dir = TestDir::new("fuzzing");
    let extensions = ["png", "jpg", "wav", "mp3", "mp4", "mov"];

    for (i, ext) in extensions.iter().enumerate() {
        for len in [16, 64, 256, 1024] {
            let fuzz_bytes: Vec<u8> = (0..len)
                .map(|b| ((b * 31 + i * 17) % 256) as u8)
                .collect();

            let file_path = test_dir.file(&format!("fuzz_{}_{}.{}", i, len, ext));
            fs::write(&file_path, &fuzz_bytes).unwrap();

            let path_str = file_path.to_str().unwrap().to_string();
            let res = catch_unwind(AssertUnwindSafe(|| {
                api::import_media_file(None, path_str)
            }));

            assert!(res.is_ok(), "import_media_file panicked during fuzz test with ext '{}' and length {}", ext, len);
            assert!(res.unwrap().is_err(), "Fuzzed file must return Err, not Ok");
        }
    }
}

// =========================================================================
// VECTOR 3: add_clip_to_track_from_media Boundary & Error Cases
// =========================================================================

#[test]
fn test_add_clip_to_track_from_media_invalid_track_id() {
    let timeline = api::create_timeline();
    let invalid_track_id = Uuid::new_v4();
    let media = dummy_media_item(MediaType::Video, 600);

    // Case A: timeline_in = None (auto-append mode)
    let res_none = api::add_clip_to_track_from_media(
        timeline.clone(),
        invalid_track_id,
        media.clone(),
        None,
        None,
        None,
    );
    assert!(res_none.is_err(), "Expected Err for invalid track_id with timeline_in = None");
    let err_none = res_none.unwrap_err();
    assert!(
        err_none.contains("not found") || err_none.contains(&invalid_track_id.to_string()),
        "Expected track not found error, got: {}",
        err_none
    );

    // Case B: timeline_in = Some(100) (explicit timeline_in mode)
    let res_some = api::add_clip_to_track_from_media(
        timeline,
        invalid_track_id,
        media,
        Some(0),
        Some(100),
        Some(100),
    );
    assert!(res_some.is_err(), "Expected Err for invalid track_id with timeline_in = Some");
    let err_some = res_some.unwrap_err();
    assert!(
        err_some.contains("not found") || err_some.contains(&invalid_track_id.to_string()),
        "Expected track not found error, got: {}",
        err_some
    );
}

#[test]
fn test_add_clip_to_track_from_media_negative_bounds() {
    let timeline = api::create_timeline();
    let valid_track_id = timeline.tracks[0].id;
    let media = dummy_media_item(MediaType::Video, 600);

    // Negative source_in
    let res_neg_source_in = api::add_clip_to_track_from_media(
        timeline.clone(),
        valid_track_id,
        media.clone(),
        Some(-10),
        Some(100),
        Some(0),
    );
    assert!(res_neg_source_in.is_err(), "Expected Err for negative source_in");
    let err = res_neg_source_in.unwrap_err();
    assert!(
        err.contains("InvalidSourceBounds") || err.contains("negative") || err.contains("-10"),
        "Unexpected error: {}",
        err
    );

    // Negative timeline_in
    let res_neg_timeline_in = api::add_clip_to_track_from_media(
        timeline.clone(),
        valid_track_id,
        media.clone(),
        Some(0),
        Some(100),
        Some(-50),
    );
    assert!(res_neg_timeline_in.is_err(), "Expected Err for negative timeline_in");
    let err = res_neg_timeline_in.unwrap_err();
    assert!(
        err.contains("InvalidClipBounds") || err.contains("negative") || err.contains("-50"),
        "Unexpected error: {}",
        err
    );
}

#[test]
fn test_add_clip_to_track_from_media_inverted_source_bounds() {
    let timeline = api::create_timeline();
    let valid_track_id = timeline.tracks[0].id;
    let media = dummy_media_item(MediaType::Video, 600);

    // Inverted source bounds: source_in (500) > source_out (100)
    let res = api::add_clip_to_track_from_media(
        timeline,
        valid_track_id,
        media,
        Some(500),
        Some(100),
        Some(0),
    );
    assert!(res.is_err(), "Expected Err for inverted source bounds (500 > 100)");
    let err = res.unwrap_err();
    assert!(
        err.contains("InvalidSourceBounds") || err.contains("bounds"),
        "Unexpected error: {}",
        err
    );
}

#[test]
fn test_add_clip_to_track_from_media_valid_fallbacks() {
    let timeline = api::create_timeline();
    let valid_track_id = timeline.tracks[0].id;

    // Image media with 0 duration_pts: should fallback to 300 PTS
    let image_media = dummy_media_item(MediaType::Image, 0);
    let updated = api::add_clip_to_track_from_media(
        timeline,
        valid_track_id,
        image_media,
        None,
        None,
        None,
    )
    .expect("Failed to add image clip with default bounds");

    assert_eq!(updated.duration_pts, 300);
    let track = updated.tracks.iter().find(|t| t.id == valid_track_id).unwrap();
    assert_eq!(track.clips.len(), 1);
    assert_eq!(track.clips[0].source_in, 0);
    assert_eq!(track.clips[0].source_out, 300);
    assert_eq!(track.clips[0].timeline_in, 0);
    assert_eq!(track.clips[0].timeline_out, 300);

    // Sequential second clip with timeline_in = None should auto-append at 300
    let video_media = dummy_media_item(MediaType::Video, 120);
    let updated2 = api::add_clip_to_track_from_media(
        updated,
        valid_track_id,
        video_media,
        None,
        None,
        None,
    )
    .expect("Failed to auto-append second clip");

    assert_eq!(updated2.duration_pts, 420);
    let track2 = updated2.tracks.iter().find(|t| t.id == valid_track_id).unwrap();
    assert_eq!(track2.clips.len(), 2);
    assert_eq!(track2.clips[1].timeline_in, 300);
    assert_eq!(track2.clips[1].timeline_out, 420);
}

// =========================================================================
// VECTOR 4: get_media_items & get_media_pool on Nonexistent Projects
// =========================================================================

#[test]
fn test_get_media_items_on_nonexistent_project() {
    let nonexistent_projects = [
        "/tmp/non_existent_project_123456.aether",
        "some/relative/missing.aether",
        "",
    ];

    for proj in nonexistent_projects {
        let res = api::get_media_items(proj.to_string());
        assert!(res.is_err(), "Expected Err for nonexistent project: '{}'", proj);
        let err = res.unwrap_err();
        assert!(
            err.contains("Failed to load project"),
            "Expected 'Failed to load project', got: {}",
            err
        );
    }
}

#[test]
fn test_get_media_pool_on_nonexistent_project() {
    let proj = "/tmp/another_missing_project_789.aether";
    let res = api::get_media_pool(proj.to_string());
    assert!(res.is_err(), "Expected Err for nonexistent project in get_media_pool");
    let err = res.unwrap_err();
    assert!(
        err.contains("Failed to load project"),
        "Expected 'Failed to load project', got: {}",
        err
    );
}

#[test]
fn test_get_media_items_on_corrupted_project_file() {
    let test_dir = TestDir::new("corrupt_proj");
    let corrupt_proj = test_dir.file("corrupted.aether");
    fs::write(&corrupt_proj, b"{ invalid json: 12345").unwrap();

    let res = api::get_media_items(corrupt_proj.to_str().unwrap().to_string());
    assert!(res.is_err(), "Expected Err for corrupt project file");
    let err = res.unwrap_err();
    assert!(
        err.contains("Failed to load project"),
        "Expected 'Failed to load project' error, got: {}",
        err
    );
}

// =========================================================================
// VECTOR 5: add_clip_from_media_pool Edge Cases
// =========================================================================

#[test]
fn test_add_clip_from_media_pool_missing_media() {
    let test_dir = TestDir::new("pool_missing");
    let dir_str = test_dir.path.to_str().unwrap();
    let mut project = Project::create_new("TestProject", dir_str).unwrap();
    let track_id = project.timeline.add_track(TrackKind::Video);
    let missing_media_id = Uuid::new_v4();

    let res = api::add_clip_from_media_pool(
        project,
        track_id,
        missing_media_id,
        None,
        None,
        None,
    );

    assert!(res.is_err(), "Expected Err when media_id is not in MediaPool");
    let err = res.unwrap_err();
    assert!(
        err.contains("not found in project MediaPool"),
        "Expected 'not found in project MediaPool' in error, got: {}",
        err
    );
}
