use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use uuid::Uuid;

use crate::media::{MediaItem, MediaPool};
use crate::timeline::{Clip, Rational, Timeline};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub project_path: String, // Absolute path to project folder
    pub file_path: String,    // Absolute path to .aether file
    pub timeline: Timeline,
    #[serde(default)]
    pub media_pool: MediaPool,
}

impl Project {
    /// Creates a new non-destructive project.
    /// `base_dir` is the standard documents folder for the platform.
    pub fn create_new(name: &str, base_dir: &str) -> Result<Self, String> {
        let project_id = Uuid::new_v4().to_string();

        let safe_name = name.replace(' ', "_").to_lowercase();

        let project_dir = Path::new(base_dir).join(&safe_name);
        if !project_dir.exists() {
            fs::create_dir_all(&project_dir).map_err(|e| format!("Erro ao criar pasta: {}", e))?;
        }

        let file_path = project_dir.join(format!("{}.aether", safe_name));

        let project = Project {
            id: project_id,
            name: name.to_string(),
            project_path: project_dir.to_string_lossy().to_string(),
            file_path: file_path.to_string_lossy().to_string(),
            timeline: Timeline::new(Rational { num: 60, den: 1 }),
            media_pool: MediaPool::new(),
        };

        // Initial save to disk
        project.save()?;

        Ok(project)
    }

    /// Saves the current project state (Timeline, MediaPool, Clips) to disk as JSON.
    pub fn save(&self) -> Result<(), String> {
        let json_data = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Falha ao converter projeto para JSON: {}", e))?;

        fs::write(&self.file_path, json_data)
            .map_err(|e| format!("Falha ao escrever arquivo no disco: {}", e))?;

        Ok(())
    }

    /// Loads an existing project from disk from its .aether file path.
    pub fn load(file_path: &str) -> Result<Self, String> {
        let json_data = fs::read_to_string(file_path)
            .map_err(|e| format!("Falha ao ler o arquivo .aether: {}", e))?;

        let project: Project = serde_json::from_str(&json_data)
            .map_err(|e| format!("Falha ao decodificar projeto corrompido: {}", e))?;

        Ok(project)
    }

    /// Convenience helper to add a media item to the project's media pool.
    pub fn add_media(&mut self, item: MediaItem) -> Uuid {
        self.media_pool.add(item)
    }

    /// Convenience helper to get a media item from the project's media pool.
    pub fn get_media(&self, id: Uuid) -> Option<&MediaItem> {
        self.media_pool.get(id)
    }

    /// Convenience helper to remove a media item from the project's media pool.
    pub fn remove_media(&mut self, id: Uuid) -> Option<MediaItem> {
        self.media_pool.remove(id)
    }

    /// Checks if a media item is currently referenced by any clip on any timeline track.
    pub fn is_media_used(&self, media_id: Uuid) -> bool {
        self.timeline
            .tracks
            .iter()
            .any(|track| track.clips.iter().any(|clip| clip.source_id == media_id))
    }

    /// Places a clip onto a timeline track referencing a media item in the pool.
    /// If `source_in` is None, defaults to 0.
    /// If `source_out` is None, defaults to `media.metadata.duration_pts` (or 300 for image/0-duration).
    /// If `timeline_in` is None, appends to the track end.
    pub fn add_clip_from_media(
        &mut self,
        track_id: Uuid,
        media_id: Uuid,
        source_in: Option<i64>,
        source_out: Option<i64>,
        timeline_in: Option<i64>,
    ) -> Result<Clip, String> {
        let media = self
            .media_pool
            .get(media_id)
            .ok_or_else(|| format!("MediaItem with ID {} not found in MediaPool", media_id))?;

        let in_pts = source_in.unwrap_or(0);
        let out_pts = match source_out {
            Some(out) => out,
            None => {
                if media.metadata.duration_pts > 0 {
                    media.metadata.duration_pts
                } else {
                    // Default duration for still images / zero-duration media: 5s @ 60fps = 300 pts
                    300
                }
            }
        };

        let t_in = match timeline_in {
            Some(t) => t,
            None => {
                let track = self
                    .timeline
                    .tracks
                    .iter()
                    .find(|t| t.id == track_id)
                    .ok_or_else(|| format!("Track with ID {} not found", track_id))?;
                track.duration_pts()
            }
        };

        let clip = Clip::new(media_id, in_pts, out_pts, t_in);
        self.timeline
            .add_clip(track_id, clip.clone())
            .map_err(|e| e.to_string())?;
        Ok(clip)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::{MediaItem, MediaMetadata, MediaType};
    use crate::timeline::TrackKind;

    #[test]
    fn test_create_new_and_save_load_lifecycle() {
        let tmp_base = std::env::temp_dir().join(format!("aether_test_{}", Uuid::new_v4()));
        let base_dir_str = tmp_base.to_str().unwrap();

        let mut project = Project::create_new("My Vacation", base_dir_str).expect("create_new should succeed");
        assert_eq!(project.name, "My Vacation");
        assert!(project.media_pool.is_empty());
        assert_eq!(project.timeline.tracks.len(), 0);

        // Add a media item
        let media_item = MediaItem::new(
            "/videos/drone_shot.mp4".to_string(),
            MediaType::Video,
            MediaMetadata {
                width: Some(3840),
                height: Some(2160),
                duration_pts: 600,
                duration_seconds: 10.0,
                timebase: Some(Rational { num: 60, den: 1 }),
                audio_channels: Some(2),
                sample_rate: Some(48000),
                file_size_bytes: 1024 * 1024 * 100,
            },
        );
        let media_id = project.add_media(media_item);

        // Add a track and clip from media
        let track_id = project.timeline.add_track(TrackKind::Video);
        let clip = project.add_clip_from_media(track_id, media_id, Some(0), Some(300), Some(0))
            .expect("should add clip from media");

        assert_eq!(clip.source_id, media_id);
        assert_eq!(clip.duration(), 300);
        assert!(project.is_media_used(media_id));

        // Save
        project.save().expect("save should succeed");

        // Load
        let loaded = Project::load(&project.file_path).expect("load should succeed");
        assert_eq!(loaded.id, project.id);
        assert_eq!(loaded.name, project.name);
        assert_eq!(loaded.media_pool.len(), 1);
        assert_eq!(loaded.media_pool.items[0].id, media_id);
        assert_eq!(loaded.timeline.tracks.len(), 1);
        assert_eq!(loaded.timeline.tracks[0].clips.len(), 1);
        assert_eq!(loaded.timeline.tracks[0].clips[0].source_id, media_id);
        assert_eq!(loaded, project);

        // Cleanup
        let _ = fs::remove_dir_all(&tmp_base);
    }

    #[test]
    fn test_backwards_compatibility_without_media_pool_key() {
        let legacy_json = r#"{
            "id": "11111111-1111-1111-1111-111111111111",
            "name": "Legacy Project",
            "project_path": "/legacy/path",
            "file_path": "/legacy/path/legacy.aether",
            "timeline": {
                "id": "22222222-2222-2222-2222-222222222222",
                "timebase": { "num": 60, "den": 1 },
                "duration_pts": 120,
                "tracks": [
                    {
                        "id": "33333333-3333-3333-3333-333333333333",
                        "kind": "Video",
                        "clips": [
                            {
                                "id": "44444444-4444-4444-4444-444444444444",
                                "source_id": "55555555-5555-5555-5555-555555555555",
                                "source_in": 0,
                                "source_out": 120,
                                "timeline_in": 0,
                                "timeline_out": 120
                            }
                        ]
                    }
                ]
            }
        }"#;

        let project: Project = serde_json::from_str(legacy_json).expect("legacy JSON should deserialize cleanly");
        assert_eq!(project.name, "Legacy Project");
        assert_eq!(project.media_pool.len(), 0);
        assert!(project.media_pool.is_empty());
        assert_eq!(project.timeline.tracks.len(), 1);
        assert_eq!(project.timeline.tracks[0].clips.len(), 1);
        assert_eq!(project.timeline.duration_pts, 120);
    }

    #[test]
    fn test_non_destructive_multiple_clips_from_single_media() {
        let mut project = Project {
            id: Uuid::new_v4().to_string(),
            name: "NonDestructive".to_string(),
            project_path: "/dummy".to_string(),
            file_path: "/dummy/file.aether".to_string(),
            timeline: Timeline::new_with_default_tracks(Rational { num: 60, den: 1 }),
            media_pool: MediaPool::new(),
        };

        let media = MediaItem::new(
            "/media/long_take.mp4".to_string(),
            MediaType::Video,
            MediaMetadata {
                width: Some(1920),
                height: Some(1080),
                duration_pts: 1200,
                duration_seconds: 20.0,
                timebase: Some(Rational { num: 60, den: 1 }),
                audio_channels: Some(2),
                sample_rate: Some(48000),
                file_size_bytes: 50 * 1024 * 1024,
            },
        );
        let media_id = project.add_media(media);
        let video_track_id = project.timeline.tracks[0].id;

        // Clip A: sub-clip 0..200 at timeline 0
        let clip_a = project.add_clip_from_media(video_track_id, media_id, Some(0), Some(200), Some(0)).unwrap();
        // Clip B: sub-clip 800..1100 at timeline 200
        let clip_b = project.add_clip_from_media(video_track_id, media_id, Some(800), Some(1100), Some(200)).unwrap();

        assert_eq!(clip_a.source_id, media_id);
        assert_eq!(clip_b.source_id, media_id);
        assert_eq!(clip_a.duration(), 200);
        assert_eq!(clip_b.duration(), 300);
        assert_eq!(project.timeline.duration_pts, 500);

        // Verify media source is untouched
        let pool_item = project.get_media(media_id).unwrap();
        assert_eq!(pool_item.metadata.duration_pts, 1200);

        // Check is_media_used
        assert!(project.is_media_used(media_id));
        assert!(!project.is_media_used(Uuid::new_v4()));

        // Remove media
        let removed = project.remove_media(media_id);
        assert_eq!(removed.unwrap().id, media_id);
        assert!(project.get_media(media_id).is_none());
    }
}
