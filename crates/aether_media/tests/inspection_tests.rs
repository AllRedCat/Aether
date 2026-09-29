mod fixtures;

use aether_core::media::MediaType;
use aether_core::timeline::Rational;
use aether_media::{inspect_media_file, inspect_media_file_with_timebase};
use std::fs;
use tempfile::tempdir;

#[test]
fn test_inspect_png_metadata_extraction() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test_card.png");

    let png_bytes = fixtures::create_synthetic_png(640, 480);
    fs::write(&file_path, png_bytes).unwrap();

    let path_str = file_path.to_str().unwrap();
    let inspection = inspect_media_file(path_str).expect("Inspection failed for valid PNG");

    assert_eq!(inspection.media_type, MediaType::Image);
    assert_eq!(inspection.width, Some(640));
    assert_eq!(inspection.height, Some(480));
    assert_eq!(inspection.duration_seconds, 5.0);
    assert_eq!(inspection.duration_pts, 300);
    assert_eq!(inspection.audio_channels, None);
    assert_eq!(inspection.sample_rate, None);
    assert!(inspection.file_size_bytes > 0);

    // Convert to MediaMetadata and MediaItem
    let metadata = inspection.to_metadata();
    assert_eq!(metadata.width, Some(640));
    assert_eq!(metadata.height, Some(480));
    assert_eq!(metadata.duration_pts, 300);

    let item = inspection.to_media_item(path_str);
    assert_eq!(item.file_name, "test_card.png");
    assert_eq!(item.media_type, MediaType::Image);
    assert_eq!(item.metadata.width, Some(640));
}

#[test]
fn test_inspect_wav_audio_metadata_extraction() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("stereo_tone.wav");

    // 44.1 kHz, Stereo, 1.5 seconds
    let wav_bytes = fixtures::create_synthetic_wav(44100, 2, 1.5);
    fs::write(&file_path, wav_bytes).unwrap();

    let path_str = file_path.to_str().unwrap();
    let inspection = inspect_media_file(path_str).expect("Inspection failed for valid WAV");

    assert_eq!(inspection.media_type, MediaType::Audio);
    assert_eq!(inspection.width, None);
    assert_eq!(inspection.height, None);
    assert_eq!(inspection.sample_rate, Some(44100));
    assert_eq!(inspection.audio_channels, Some(2));
    assert!((inspection.duration_seconds - 1.5).abs() < 0.05);
    // 1.5s * 60 fps = 90 PTS
    assert_eq!(inspection.duration_pts, 90);

    let item = inspection.to_media_item(path_str);
    assert_eq!(item.file_name, "stereo_tone.wav");
    assert_eq!(item.media_type, MediaType::Audio);
    assert_eq!(item.metadata.sample_rate, Some(44100));
}

#[test]
fn test_inspect_wav_mono_48000_metadata() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("mono_voice.wav");

    // 48 kHz, Mono, 2.0 seconds
    let wav_bytes = fixtures::create_synthetic_wav(48000, 1, 2.0);
    fs::write(&file_path, wav_bytes).unwrap();

    let path_str = file_path.to_str().unwrap();
    let inspection = inspect_media_file_with_timebase(
        path_str,
        Rational { num: 30, den: 1 }, // 30 fps timebase
    ).expect("Inspection failed for mono WAV");

    assert_eq!(inspection.media_type, MediaType::Audio);
    assert_eq!(inspection.sample_rate, Some(48000));
    assert_eq!(inspection.audio_channels, Some(1));
    assert!((inspection.duration_seconds - 2.0).abs() < 0.05);
    // 2.0s * 30 fps = 60 PTS
    assert_eq!(inspection.duration_pts, 60);
}

#[test]
fn test_inspect_mp4_video_metadata_extraction() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("sample_clip.mp4");

    // 1280x720, 3 seconds
    let mp4_bytes = fixtures::create_synthetic_mp4(1280, 720, 3);
    fs::write(&file_path, mp4_bytes).unwrap();

    let path_str = file_path.to_str().unwrap();
    let inspection = inspect_media_file(path_str).expect("Inspection failed for valid MP4");

    assert_eq!(inspection.media_type, MediaType::Video);
    assert_eq!(inspection.width, Some(1280));
    assert_eq!(inspection.height, Some(720));
    assert!((inspection.duration_seconds - 3.0).abs() < 0.1);
    // 3.0s * 60 fps = 180 PTS
    assert_eq!(inspection.duration_pts, 180);
    assert!(inspection.file_size_bytes > 0);

    let item = inspection.to_media_item(path_str);
    assert_eq!(item.file_name, "sample_clip.mp4");
    assert_eq!(item.media_type, MediaType::Video);
    assert_eq!(item.metadata.width, Some(1280));
    assert_eq!(item.metadata.height, Some(720));
}
