use aether_core::media::{MediaItem, MediaMetadata, MediaPool, MediaType};
use aether_core::project::Project;
use aether_core::timeline::{Clip, Rational, Timeline};

#[test]
fn test_project_json_roundtrip_preserves_media_pool_and_clips() {
    let mut project = Project {
        id: "proj_test_001".to_string(),
        name: "Test Project".to_string(),
        project_path: "/tmp/test_project".to_string(),
        file_path: "/tmp/test_project/test_project.aether".to_string(),
        timeline: Timeline::new_with_default_tracks(Rational { num: 60, den: 1 }),
        media_pool: MediaPool::new(),
    };

    // Add media items to media_pool
    let video_item = MediaItem::new(
        "/footage/intro.mp4".to_string(),
        MediaType::Video,
        MediaMetadata {
            width: Some(3840),
            height: Some(2160),
            duration_seconds: 12.0,
            duration_pts: 720,
            timebase: Some(Rational { num: 60, den: 1 }),
            audio_channels: Some(2),
            sample_rate: Some(48000),
            file_size_bytes: 45_000_000, video_codec: None, audio_codec: None, pixel_format: None, is_vfr: None, keyframe_pts: None,
        },
    );
    let video_id = project.media_pool.add(video_item);

    let audio_item = MediaItem::new(
        "/audio/theme.wav".to_string(),
        MediaType::Audio,
        MediaMetadata {
            width: None,
            height: None,
            duration_seconds: 30.0,
            duration_pts: 1800,
            timebase: Some(Rational { num: 60, den: 1 }),
            audio_channels: Some(2),
            sample_rate: Some(44100),
            file_size_bytes: 5_292_000, video_codec: None, audio_codec: None, pixel_format: None, is_vfr: None, keyframe_pts: None,
        },
    );
    let audio_id = project.media_pool.add(audio_item);

    // Place non-destructive clips on tracks referencing media_pool items
    let video_track_id = project.timeline.tracks[0].id;
    let audio_track_id = project.timeline.tracks[1].id;

    let v_clip = Clip::new(video_id, 0, 720, 0);
    project.timeline.add_clip(video_track_id, v_clip).unwrap();

    let a_clip = Clip::new(audio_id, 0, 720, 0);
    project.timeline.add_clip(audio_track_id, a_clip).unwrap();

    // Serialize to JSON
    let json_str = serde_json::to_string_pretty(&project).expect("Serialization failed");

    // Assert that json contains media_pool and source_id
    assert!(json_str.contains("\"media_pool\""));
    assert!(json_str.contains(&video_id.to_string()));
    assert!(json_str.contains(&audio_id.to_string()));

    // Deserialize back
    let restored: Project = serde_json::from_str(&json_str).expect("Deserialization failed");

    // Deep equality
    assert_eq!(project, restored);
    assert_eq!(restored.media_pool.len(), 2);
    assert_eq!(restored.timeline.total_clip_count(), 2);

    // Verify clips reference restored media_pool
    let restored_v_clip = &restored.timeline.tracks[0].clips[0];
    assert_eq!(restored_v_clip.source_id, video_id);
    let restored_media = restored.media_pool.get(restored_v_clip.source_id).unwrap();
    assert_eq!(restored_media.file_name, "intro.mp4");
    assert_eq!(restored_media.metadata.width, Some(3840));
}

#[test]
fn test_backwards_compatible_deserialization_without_media_pool() {
    // Legacy .aether JSON from before M1 (no "media_pool" field)
    let legacy_json = r#"{
        "id": "legacy_project_123",
        "name": "Legacy Project",
        "project_path": "/Users/test/Documents/Aether/legacy",
        "file_path": "/Users/test/Documents/Aether/legacy/legacy.aether",
        "timeline": {
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "timebase": { "num": 60, "den": 1 },
            "duration_pts": 0,
            "tracks": []
        }
    }"#;

    // Deserialization MUST succeed due to #[serde(default)]
    let result: Result<Project, _> = serde_json::from_str(legacy_json);
    assert!(result.is_ok(), "Failed to deserialize legacy project: {:?}", result.err());

    let mut project = result.unwrap();
    assert_eq!(project.name, "Legacy Project");
    assert!(project.media_pool.is_empty());
    assert_eq!(project.media_pool.len(), 0);

    // Upgrading: adding an item to the deserialized legacy project works flawlessly
    let new_item = MediaItem::new(
        "/footage/new_asset.png".to_string(),
        MediaType::Image,
        MediaMetadata {
            width: Some(800),
            height: Some(600),
            duration_seconds: 0.0,
            duration_pts: 0,
            timebase: None,
            audio_channels: None,
            sample_rate: None,
            file_size_bytes: 1024, video_codec: None, audio_codec: None, pixel_format: None, is_vfr: None, keyframe_pts: None,
        },
    );
    let id = project.media_pool.add(new_item);
    assert_eq!(project.media_pool.len(), 1);
    assert_eq!(project.media_pool.get(id).unwrap().file_name, "new_asset.png");

    // Re-serializing now includes the new media_pool
    let updated_json = serde_json::to_string(&project).unwrap();
    assert!(updated_json.contains("\"media_pool\""));
}
