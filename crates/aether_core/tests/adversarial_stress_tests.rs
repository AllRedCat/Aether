use std::fs;
use std::time::Instant;
use uuid::Uuid;

use aether_core::media::{MediaItem, MediaMetadata, MediaPool, MediaType};
use aether_core::project::Project;
use aether_core::timeline::{Clip, Rational, Timeline, TrackKind};

// ============================================================================
// SUITE 1: Scale Testing (MediaPool Scale & Query Performance)
// ============================================================================

#[test]
fn test_scale_mediapool_1500_items_query_and_mutation() {
    let mut pool = MediaPool::new();
    let num_items = 1500;
    let mut generated_ids = Vec::with_capacity(num_items);
    let mut generated_paths = Vec::with_capacity(num_items);

    let start_insert = Instant::now();
    for i in 0..num_items {
        let media_type = match i % 3 {
            0 => MediaType::Video,
            1 => MediaType::Audio,
            _ => MediaType::Image,
        };

        let path = format!("/Volumes/MediaStorage/project_assets/asset_{:05}.raw", i);
        generated_paths.push(path.clone());

        let metadata = MediaMetadata {
            width: if media_type == MediaType::Audio { None } else { Some(1920) },
            height: if media_type == MediaType::Audio { None } else { Some(1080) },
            duration_pts: (i as i64 + 1) * 60,
            duration_seconds: (i as f64 + 1.0),
            timebase: Some(Rational { num: 60, den: 1 }),
            audio_channels: if media_type == MediaType::Image { None } else { Some(2) },
            sample_rate: if media_type == MediaType::Image { None } else { Some(48000) },
            file_size_bytes: (i as u64 + 1) * 1024 * 1024,
        };

        let item = MediaItem::new(path, media_type, metadata);
        let id = pool.add(item);
        generated_ids.push(id);
    }
    let insert_duration = start_insert.elapsed();
    println!("Inserted {} items in {:?}", num_items, insert_duration);

    assert_eq!(pool.len(), num_items);
    assert!(!pool.is_empty());

    // 1. Query by UUID for all 1,500 items
    let start_query_uuid = Instant::now();
    for (i, &id) in generated_ids.iter().enumerate() {
        let item = pool.get(id).unwrap_or_else(|| panic!("Item with id {} must exist", id));
        assert_eq!(item.id, id);
        assert_eq!(item.file_path, generated_paths[i]);
        assert_eq!(item.file_name, format!("asset_{:05}.raw", i));
    }
    let query_uuid_duration = start_query_uuid.elapsed();
    println!("Queried {} items by UUID in {:?}", num_items, query_uuid_duration);

    // 2. Query 200 non-existent UUIDs
    for _ in 0..200 {
        let fake_id = Uuid::new_v4();
        assert!(pool.get(fake_id).is_none(), "Non-existent UUID should return None");
    }

    // 3. Query by path for all 1,500 items
    let start_query_path = Instant::now();
    for (i, path) in generated_paths.iter().enumerate() {
        let item = pool
            .find_by_path(path)
            .unwrap_or_else(|| panic!("Item with path {} must exist", path));
        assert_eq!(item.id, generated_ids[i]);
    }
    let query_path_duration = start_query_path.elapsed();
    println!("Queried {} items by path in {:?}", num_items, query_path_duration);

    // 4. Query 200 non-existent paths
    for i in 0..200 {
        let fake_path = format!("/Volumes/NonExistent/ghost_asset_{}.mp4", i);
        assert!(pool.find_by_path(&fake_path).is_none());
    }

    // 5. Remove 500 items
    let remove_count = 500;
    for i in 0..remove_count {
        let target_id = generated_ids[i];
        let removed = pool.remove(target_id).expect("Removal must return the item");
        assert_eq!(removed.id, target_id);
        assert_eq!(removed.file_path, generated_paths[i]);
    }

    assert_eq!(pool.len(), num_items - remove_count);

    // Verify removed items are no longer present
    for i in 0..remove_count {
        assert!(pool.get(generated_ids[i]).is_none());
        assert!(pool.find_by_path(&generated_paths[i]).is_none());
    }

    // Verify remaining 1,000 items are still fully accessible
    for i in remove_count..num_items {
        let item = pool.get(generated_ids[i]).expect("Remaining item must exist");
        assert_eq!(item.id, generated_ids[i]);
    }

    // 6. Clear pool
    pool.clear();
    assert_eq!(pool.len(), 0);
    assert!(pool.is_empty());
}

// ============================================================================
// SUITE 2: Non-Destructive Multi-Clip Stress Test
// ============================================================================

#[test]
fn test_nondestructive_stress_100_subclips_same_media() {
    let mut project = Project {
        id: Uuid::new_v4().to_string(),
        name: "NonDestructive_Stress".to_string(),
        project_path: "/tmp/nd_stress".to_string(),
        file_path: "/tmp/nd_stress/nd_stress.aether".to_string(),
        timeline: Timeline::new(Rational { num: 60, den: 1 }),
        media_pool: MediaPool::new(),
    };

    // Master 4K Video asset (10,000 PTS = ~166.6 seconds)
    let master_item = MediaItem::new(
        "/footage/master_interview_8k.mp4".to_string(),
        MediaType::Video,
        MediaMetadata {
            width: Some(7680),
            height: Some(4320),
            duration_pts: 10_000,
            duration_seconds: 166.666,
            timebase: Some(Rational { num: 60, den: 1 }),
            audio_channels: Some(8),
            sample_rate: Some(96000),
            file_size_bytes: 40_000_000_000,
        },
    );
    let master_snapshot = master_item.clone();
    let master_id = project.add_media(master_item);

    // Create 5 video tracks
    let mut track_ids = Vec::new();
    for _ in 0..5 {
        track_ids.push(project.timeline.add_track(TrackKind::Video));
    }

    struct ExpectedClip {
        track_id: Uuid,
        source_in: i64,
        source_out: i64,
        timeline_in: i64,
        timeline_out: i64,
    }
    let mut expected_clips = Vec::with_capacity(100);

    // Generate 100 diverse subclips
    for i in 0..100 {
        let track_id = track_ids[i % track_ids.len()];

        // Slice sub-sections: varied sizes, some zero-length, some overlapping
        let source_in = (i as i64 * 80) % 9500;
        let slice_len = if i == 42 {
            0 // zero-duration boundary check
        } else {
            ((i as i64 * 37) % 500) + 10
        };
        let source_out = source_in + slice_len;

        // Spread clips across timeline
        let timeline_in = (i as i64) * 120;
        let timeline_out = timeline_in + slice_len;

        let clip = project
            .add_clip_from_media(
                track_id,
                master_id,
                Some(source_in),
                Some(source_out),
                Some(timeline_in),
            )
            .unwrap_or_else(|e| panic!("Failed to add clip {}: {}", i, e));

        assert_eq!(clip.source_id, master_id);
        assert_eq!(clip.source_in, source_in);
        assert_eq!(clip.source_out, source_out);
        assert_eq!(clip.timeline_in, timeline_in);
        assert_eq!(clip.timeline_out, timeline_out);
        assert_eq!(clip.duration(), slice_len);

        expected_clips.push(ExpectedClip {
            track_id,
            source_in,
            source_out,
            timeline_in,
            timeline_out,
        });

        // INVARIANT CHECK DURING INSERTION:
        // Master MediaItem in MediaPool must NEVER change!
        let current_master = project.get_media(master_id).unwrap();
        assert_eq!(
            current_master, &master_snapshot,
            "Master MediaItem was mutated at step {}!",
            i
        );
    }

    // Post-insertion assertions:
    assert_eq!(project.timeline.total_clip_count(), 100);
    assert!(project.is_media_used(master_id));

    // Calculate expected timeline duration
    let expected_max_pts = expected_clips
        .iter()
        .map(|c| c.timeline_out)
        .max()
        .unwrap_or(0);
    assert_eq!(project.timeline.duration_pts, expected_max_pts);

    // Verify all 100 clips retain their exact untouched coordinates
    let mut verified_count = 0;
    for expected in &expected_clips {
        let track = project
            .timeline
            .tracks
            .iter()
            .find(|t| t.id == expected.track_id)
            .unwrap();

        let matching_clip = track
            .clips
            .iter()
            .find(|c| {
                c.source_in == expected.source_in
                    && c.source_out == expected.source_out
                    && c.timeline_in == expected.timeline_in
                    && c.timeline_out == expected.timeline_out
            })
            .expect("Each clip must retain its exact untouched bounds");

        assert_eq!(matching_clip.source_id, master_id);
        verified_count += 1;
    }
    assert_eq!(verified_count, 100);

    // Final check: master MediaItem still identical to original snapshot
    let final_master = project.get_media(master_id).unwrap();
    assert_eq!(final_master, &master_snapshot);
}

// ============================================================================
// SUITE 3: Corrupt JSON Deserialization & Schema Permutations
// ============================================================================

#[test]
fn test_corrupt_json_malformed_syntax() {
    let bad_payloads = [
        "",                                         // empty string
        "{",                                        // unclosed object
        "{\"id\": \"proj1\", \"name\": \"Te",       // truncated
        "{\"id\": 12345 ",                          // syntax error
        "{\"id\": \"p1\"} trailing_text",           // trailing tokens
        "null",                                     // null literal
        "12345",                                    // number
        "\"string\"",                               // string literal
        "[1, 2, 3]",                                // array
        "{\x00\x01\x02}",                           // binary control characters
    ];

    for payload in bad_payloads {
        let res: Result<Project, _> = serde_json::from_str(payload);
        assert!(
            res.is_err(),
            "Malformed JSON payload should fail deserialization: {:?}",
            payload
        );
    }
}

#[test]
fn test_corrupt_json_missing_required_fields() {
    // Missing "timeline"
    let missing_timeline = r#"{
        "id": "p1",
        "name": "Missing Timeline",
        "project_path": "/tmp",
        "file_path": "/tmp/p.aether",
        "media_pool": { "items": [] }
    }"#;
    assert!(serde_json::from_str::<Project>(missing_timeline).is_err());

    // Missing "id"
    let missing_id = r#"{
        "name": "Missing ID",
        "project_path": "/tmp",
        "file_path": "/tmp/p.aether",
        "timeline": {
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "timebase": { "num": 60, "den": 1 },
            "duration_pts": 0,
            "tracks": []
        }
    }"#;
    assert!(serde_json::from_str::<Project>(missing_id).is_err());

    // Missing "name"
    let missing_name = r#"{
        "id": "p1",
        "project_path": "/tmp",
        "file_path": "/tmp/p.aether",
        "timeline": {
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "timebase": { "num": 60, "den": 1 },
            "duration_pts": 0,
            "tracks": []
        }
    }"#;
    assert!(serde_json::from_str::<Project>(missing_name).is_err());

    // Timeline missing "tracks"
    let missing_tracks = r#"{
        "id": "p1",
        "name": "Missing Tracks",
        "project_path": "/tmp",
        "file_path": "/tmp/p.aether",
        "timeline": {
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "timebase": { "num": 60, "den": 1 },
            "duration_pts": 0
        }
    }"#;
    assert!(serde_json::from_str::<Project>(missing_tracks).is_err());

    // Timeline missing "timebase"
    let missing_timebase = r#"{
        "id": "p1",
        "name": "Missing Timebase",
        "project_path": "/tmp",
        "file_path": "/tmp/p.aether",
        "timeline": {
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "duration_pts": 0,
            "tracks": []
        }
    }"#;
    assert!(serde_json::from_str::<Project>(missing_timebase).is_err());

    // Note: Missing "media_pool" MUST SUCCEED (backwards compatibility test)
    let missing_media_pool = r#"{
        "id": "p1",
        "name": "Missing MediaPool",
        "project_path": "/tmp",
        "file_path": "/tmp/p.aether",
        "timeline": {
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "timebase": { "num": 60, "den": 1 },
            "duration_pts": 0,
            "tracks": []
        }
    }"#;
    let ok_proj: Result<Project, _> = serde_json::from_str(missing_media_pool);
    assert!(ok_proj.is_ok(), "Missing media_pool should deserialize with default");
    assert!(ok_proj.unwrap().media_pool.is_empty());
}

#[test]
fn test_corrupt_json_schema_permutations_and_invalid_types() {
    // 1. Invalid UUID in MediaItem
    let bad_uuid = r#"{
        "id": "p1",
        "name": "Bad UUID",
        "project_path": "/tmp",
        "file_path": "/tmp/p.aether",
        "timeline": {
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "timebase": { "num": 60, "den": 1 },
            "duration_pts": 0,
            "tracks": []
        },
        "media_pool": {
            "items": [
                {
                    "id": "not-a-valid-uuid-here",
                    "file_path": "/asset.mp4",
                    "file_name": "asset.mp4",
                    "media_type": "Video",
                    "metadata": {
                        "width": 1920,
                        "height": 1080,
                        "duration_pts": 100,
                        "duration_seconds": 1.66,
                        "timebase": null,
                        "audio_channels": 2,
                        "sample_rate": 48000,
                        "file_size_bytes": 1000
                    }
                }
            ]
        }
    }"#;
    assert!(serde_json::from_str::<Project>(bad_uuid).is_err());

    // 2. Invalid MediaType enum variant ("VR360")
    let bad_enum = r#"{
        "id": "p1",
        "name": "Bad Enum",
        "project_path": "/tmp",
        "file_path": "/tmp/p.aether",
        "timeline": {
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "timebase": { "num": 60, "den": 1 },
            "duration_pts": 0,
            "tracks": []
        },
        "media_pool": {
            "items": [
                {
                    "id": "550e8400-e29b-41d4-a716-446655440000",
                    "file_path": "/asset.mp4",
                    "file_name": "asset.mp4",
                    "media_type": "VR360",
                    "metadata": {
                        "width": null,
                        "height": null,
                        "duration_pts": 0,
                        "duration_seconds": 0.0,
                        "timebase": null,
                        "audio_channels": null,
                        "sample_rate": null,
                        "file_size_bytes": 0
                    }
                }
            ]
        }
    }"#;
    assert!(serde_json::from_str::<Project>(bad_enum).is_err());

    // 3. Negative value for unsigned u32 width
    let negative_width = r#"{
        "id": "p1",
        "name": "Negative Width",
        "project_path": "/tmp",
        "file_path": "/tmp/p.aether",
        "timeline": {
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "timebase": { "num": 60, "den": 1 },
            "duration_pts": 0,
            "tracks": []
        },
        "media_pool": {
            "items": [
                {
                    "id": "550e8400-e29b-41d4-a716-446655440000",
                    "file_path": "/asset.mp4",
                    "file_name": "asset.mp4",
                    "media_type": "Video",
                    "metadata": {
                        "width": -1920,
                        "height": 1080,
                        "duration_pts": 0,
                        "duration_seconds": 0.0,
                        "timebase": null,
                        "audio_channels": null,
                        "sample_rate": null,
                        "file_size_bytes": 0
                    }
                }
            ]
        }
    }"#;
    assert!(serde_json::from_str::<Project>(negative_width).is_err());

    // 4. String for numeric duration_pts
    let string_pts = r#"{
        "id": "p1",
        "name": "String PTS",
        "project_path": "/tmp",
        "file_path": "/tmp/p.aether",
        "timeline": {
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "timebase": { "num": 60, "den": 1 },
            "duration_pts": "six_hundred",
            "tracks": []
        }
    }"#;
    assert!(serde_json::from_str::<Project>(string_pts).is_err());

    // 5. Null in non-nullable field
    let null_name = r#"{
        "id": "p1",
        "name": null,
        "project_path": "/tmp",
        "file_path": "/tmp/p.aether",
        "timeline": {
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "timebase": { "num": 60, "den": 1 },
            "duration_pts": 0,
            "tracks": []
        }
    }"#;
    assert!(serde_json::from_str::<Project>(null_name).is_err());
}

#[test]
fn test_json_forward_compatibility_extra_unknown_fields() {
    let forward_json = r#"{
        "id": "proj_fwd",
        "name": "Future Project",
        "project_path": "/tmp/fwd",
        "file_path": "/tmp/fwd/p.aether",
        "future_field_v2": "experimental_flag",
        "schema_version": 42,
        "timeline": {
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "timebase": { "num": 60, "den": 1, "extra_time_unit": "nanoseconds" },
            "duration_pts": 0,
            "tracks": [],
            "unknown_timeline_prop": true
        },
        "media_pool": {
            "items": [],
            "pool_tag": "archive"
        }
    }"#;

    let res: Result<Project, _> = serde_json::from_str(forward_json);
    assert!(
        res.is_ok(),
        "Serde should ignore unrecognized fields for forward compatibility: {:?}",
        res.err()
    );
    let p = res.unwrap();
    assert_eq!(p.id, "proj_fwd");
    assert_eq!(p.name, "Future Project");
}

// ============================================================================
// SUITE 4: Large Project Serialization Round-Trip
// ============================================================================

#[test]
fn test_large_project_scale_serialization_roundtrip() {
    let tmp_dir = std::env::temp_dir().join(format!("aether_scale_{}", Uuid::new_v4()));
    let tmp_file = tmp_dir.join("large_scale.aether");

    let mut project = Project {
        id: Uuid::new_v4().to_string(),
        name: "Massive_Production_Project".to_string(),
        project_path: tmp_dir.to_string_lossy().to_string(),
        file_path: tmp_file.to_string_lossy().to_string(),
        timeline: Timeline::new(Rational { num: 60, den: 1 }),
        media_pool: MediaPool::new(),
    };

    // 1. Populate MediaPool with 100 media assets
    let num_assets = 100;
    let mut media_ids = Vec::with_capacity(num_assets);
    for i in 0..num_assets {
        let (media_type, dur) = match i % 3 {
            0 => (MediaType::Video, 1800), // 30s @ 60fps
            1 => (MediaType::Audio, 3600), // 60s @ 60fps
            _ => (MediaType::Image, 0),    // Image
        };

        let item = MediaItem::new(
            format!("/raw/camera_reel_{:03}.mov", i),
            media_type,
            MediaMetadata {
                width: if media_type == MediaType::Audio { None } else { Some(3840) },
                height: if media_type == MediaType::Audio { None } else { Some(2160) },
                duration_pts: dur,
                duration_seconds: dur as f64 / 60.0,
                timebase: Some(Rational { num: 60, den: 1 }),
                audio_channels: if media_type == MediaType::Image { None } else { Some(2) },
                sample_rate: if media_type == MediaType::Image { None } else { Some(48000) },
                file_size_bytes: (i as u64 + 1) * 50 * 1024 * 1024,
            },
        );
        media_ids.push(project.add_media(item));
    }
    assert_eq!(project.media_pool.len(), 100);

    // 2. Populate Timeline with 50 tracks
    let num_tracks = 50;
    let mut track_ids = Vec::with_capacity(num_tracks);
    for i in 0..num_tracks {
        let kind = match i % 3 {
            0 => TrackKind::Video,
            1 => TrackKind::Audio,
            _ => TrackKind::Overlay,
        };
        track_ids.push(project.timeline.add_track(kind));
    }
    assert_eq!(project.timeline.tracks.len(), 50);

    // 3. Populate tracks with 600 clips
    let num_clips = 600;
    for i in 0..num_clips {
        let track_id = track_ids[i % num_tracks];
        let media_id = media_ids[i % num_assets];

        let source_in = (i as i64 * 15) % 1500;
        let source_out = source_in + 120; // 2 seconds
        let timeline_in = (i as i64 / num_tracks as i64) * 150 + (i as i64 % 50) * 10;

        let clip = Clip::new(media_id, source_in, source_out, timeline_in);
        project.timeline.add_clip(track_id, clip).unwrap();
    }
    assert_eq!(project.timeline.total_clip_count(), 600);
    assert!(project.timeline.duration_pts > 0);

    // 4. Measure serialization performance & size
    let start_serialize = Instant::now();
    let json_str = serde_json::to_string_pretty(&project).expect("Serialization failed");
    let serialize_duration = start_serialize.elapsed();
    println!(
        "Serialized large project (100 assets, 50 tracks, 600 clips) in {:?} (JSON size: {} bytes)",
        serialize_duration,
        json_str.len()
    );

    // 5. Measure deserialization performance
    let start_deserialize = Instant::now();
    let restored: Project = serde_json::from_str(&json_str).expect("Deserialization failed");
    let deserialize_duration = start_deserialize.elapsed();
    println!("Deserialized large project in {:?}", deserialize_duration);

    // Deep equality
    assert_eq!(project, restored);
    assert_eq!(restored.media_pool.len(), 100);
    assert_eq!(restored.timeline.tracks.len(), 50);
    assert_eq!(restored.timeline.total_clip_count(), 600);
    assert_eq!(restored.timeline.duration_pts, project.timeline.duration_pts);

    // 6. Test roundtrip via file persistence
    fs::create_dir_all(&tmp_dir).unwrap();
    project.save().expect("Project save to disk failed");
    let loaded = Project::load(&project.file_path).expect("Project load from disk failed");
    assert_eq!(project, loaded);

    // Cleanup
    let _ = fs::remove_dir_all(&tmp_dir);
}

// ============================================================================
// SUITE 5: Adversarial Domain Boundaries & Invariants
// ============================================================================

#[test]
fn test_adversarial_clip_bounds_and_error_handling() {
    let mut project = Project {
        id: "test_proj".to_string(),
        name: "Test Bounds".to_string(),
        project_path: "/tmp/tb".to_string(),
        file_path: "/tmp/tb/tb.aether".to_string(),
        timeline: Timeline::new(Rational { num: 60, den: 1 }),
        media_pool: MediaPool::new(),
    };

    let track_id = project.timeline.add_track(TrackKind::Video);
    let media = MediaItem::new(
        "/video.mp4".to_string(),
        MediaType::Video,
        MediaMetadata {
            width: Some(1920),
            height: Some(1080),
            duration_pts: 600,
            duration_seconds: 10.0,
            timebase: Some(Rational { num: 60, den: 1 }),
            audio_channels: Some(2),
            sample_rate: Some(48000),
            file_size_bytes: 1024,
        },
    );
    let media_id = project.add_media(media);

    // 1. Inverted source bounds: source_in > source_out
    let res1 = project.add_clip_from_media(track_id, media_id, Some(500), Some(200), Some(0));
    assert!(res1.is_err());
    assert!(res1.unwrap_err().contains("Invalid source bounds"));

    // 2. Negative source bounds: source_in < 0
    let res2 = project.add_clip_from_media(track_id, media_id, Some(-10), Some(100), Some(0));
    assert!(res2.is_err());
    assert!(res2.unwrap_err().contains("Invalid source bounds"));

    // 3. Negative timeline_in: timeline_in < 0
    let res3 = project.add_clip_from_media(track_id, media_id, Some(0), Some(100), Some(-50));
    assert!(res3.is_err());
    assert!(res3.unwrap_err().contains("Invalid clip timeline bounds"));

    // 4. Non-existent media_id
    let ghost_media_id = Uuid::new_v4();
    let res4 = project.add_clip_from_media(track_id, ghost_media_id, Some(0), Some(100), Some(0));
    assert!(res4.is_err());
    assert!(res4.unwrap_err().contains("not found in MediaPool"));

    // 5. Non-existent track_id
    let ghost_track_id = Uuid::new_v4();
    let res5 = project.add_clip_from_media(ghost_track_id, media_id, Some(0), Some(100), Some(0));
    assert!(res5.is_err());
    assert!(res5.unwrap_err().contains("Track with ID"));

    // 6. Zero-duration clip from media
    let res6 = project.add_clip_from_media(track_id, media_id, Some(50), Some(50), Some(0));
    assert!(res6.is_ok(), "Zero-duration clip is mathematically valid");
    let c = res6.unwrap();
    assert_eq!(c.duration(), 0);

    // 7. Auto-placement (None timeline_in appends to track end)
    let c1 = project
        .add_clip_from_media(track_id, media_id, Some(0), Some(200), None)
        .unwrap();
    assert_eq!(c1.timeline_in, 0); // track had 0 duration
    assert_eq!(c1.timeline_out, 200);

    let c2 = project
        .add_clip_from_media(track_id, media_id, Some(0), Some(150), None)
        .unwrap();
    assert_eq!(c2.timeline_in, 200); // appended after c1
    assert_eq!(c2.timeline_out, 350);

    // 8. Auto-duration for Still Image
    let img = MediaItem::new(
        "/photo.png".to_string(),
        MediaType::Image,
        MediaMetadata::default(), // duration_pts = 0
    );
    let img_id = project.add_media(img);
    let c3 = project
        .add_clip_from_media(track_id, img_id, None, None, None)
        .unwrap();
    assert_eq!(c3.timeline_in, 350);
    // Image default duration is 300 pts (5s @ 60fps)
    assert_eq!(c3.duration(), 300);
    assert_eq!(c3.timeline_out, 650);
    assert_eq!(project.timeline.duration_pts, 650);

    // 9. Referential usage and removal
    assert!(project.is_media_used(media_id));
    assert!(project.is_media_used(img_id));

    // Removing media item from pool
    let removed_img = project.remove_media(img_id);
    assert!(removed_img.is_some());
    assert!(!project.media_pool.items.iter().any(|i| i.id == img_id));
    // Notice: timeline clips still retain the source_id (soft-deleted / offline reference)
    assert!(project.is_media_used(img_id));
    // But get_media now returns None
    assert!(project.get_media(img_id).is_none());
}

#[test]
fn test_adversarial_mediapool_duplicate_paths_and_fps_zero_timebase() {
    // 1. Divide by zero check in fps()
    let zero_den_meta = MediaMetadata {
        width: Some(1920),
        height: Some(1080),
        duration_pts: 100,
        duration_seconds: 1.66,
        timebase: Some(Rational { num: 60, den: 0 }),
        audio_channels: None,
        sample_rate: None,
        file_size_bytes: 500,
    };
    assert_eq!(zero_den_meta.fps(), None, "Denominator of 0 must return None without panicking");

    // 2. Duplicate file paths in MediaPool
    let mut pool = MediaPool::new();
    let path = "/media/same_file.mp4".to_string();
    let item1 = MediaItem::new(path.clone(), MediaType::Video, zero_den_meta.clone());
    let id1 = item1.id;
    let item2 = MediaItem::new(path.clone(), MediaType::Video, zero_den_meta);
    let id2 = item2.id;
    assert_ne!(id1, id2, "Generated IDs must be unique");

    pool.add(item1);
    pool.add(item2);
    assert_eq!(pool.len(), 2);

    // Both IDs are queryable
    assert_eq!(pool.get(id1).unwrap().id, id1);
    assert_eq!(pool.get(id2).unwrap().id, id2);

    // find_by_path returns first match
    assert_eq!(pool.find_by_path(&path).unwrap().id, id1);

    // Removing id1 leaves id2 intact, and find_by_path now finds id2
    let removed1 = pool.remove(id1).unwrap();
    assert_eq!(removed1.id, id1);
    assert_eq!(pool.len(), 1);
    assert_eq!(pool.find_by_path(&path).unwrap().id, id2);
}

#[test]
fn test_adversarial_max_i64_timeline_overflow_protection() {
    let mut timeline = Timeline::new(Rational { num: 60, den: 1 });
    let track_id = timeline.add_track(TrackKind::Video);

    // Saturating arithmetic under massive values
    let clip = Clip::new(Uuid::new_v4(), 0, i64::MAX, 100);
    assert_eq!(clip.timeline_out, i64::MAX);
    assert!(timeline.add_clip(track_id, clip).is_ok());
    assert_eq!(timeline.duration_pts, i64::MAX);

    // Recalculate duration caps cleanly
    timeline.recalculate_duration();
    assert_eq!(timeline.duration_pts, i64::MAX);
}

