use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::timeline::Rational;

/// High-level categorization of media assets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MediaType {
    Video,
    Audio,
    Image,
}

impl std::fmt::Display for MediaType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MediaType::Video => write!(f, "Video"),
            MediaType::Audio => write!(f, "Audio"),
            MediaType::Image => write!(f, "Image"),
        }
    }
}

/// Technical metadata extracted from a media asset.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MediaMetadata {
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub duration_pts: i64,
    pub duration_seconds: f64,
    pub timebase: Option<Rational>,
    pub audio_channels: Option<u16>,
    pub sample_rate: Option<u32>,
    pub file_size_bytes: u64,
}

impl Default for MediaMetadata {
    fn default() -> Self {
        Self {
            width: None,
            height: None,
            duration_pts: 0,
            duration_seconds: 0.0,
            timebase: None,
            audio_channels: None,
            sample_rate: None,
            file_size_bytes: 0,
        }
    }
}

impl MediaMetadata {
    /// Returns the calculated frames-per-second (FPS) if a valid timebase is present.
    pub fn fps(&self) -> Option<f64> {
        self.timebase.and_then(|tb| {
            if tb.den != 0 {
                Some(tb.num as f64 / tb.den as f64)
            } else {
                None
            }
        })
    }
}

/// Represents an individual media file imported into the project asset pool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MediaItem {
    pub id: Uuid,
    pub file_path: String,
    pub file_name: String,
    pub media_type: MediaType,
    pub metadata: MediaMetadata,
}

impl MediaItem {
    /// Creates a new `MediaItem` with a generated UUID and extracts the filename from the path.
    pub fn new(file_path: String, media_type: MediaType, metadata: MediaMetadata) -> Self {
        let file_name = std::path::Path::new(&file_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        Self {
            id: Uuid::new_v4(),
            file_path,
            file_name,
            media_type,
            metadata,
        }
    }

    /// Creates a `MediaItem` with an explicit ID and file name (useful for testing or reconstruction).
    pub fn with_id(
        id: Uuid,
        file_path: String,
        file_name: String,
        media_type: MediaType,
        metadata: MediaMetadata,
    ) -> Self {
        Self {
            id,
            file_path,
            file_name,
            media_type,
            metadata,
        }
    }

    /// Convenience accessor for file name.
    pub fn name(&self) -> &str {
        &self.file_name
    }
}

/// Catalog holding all imported media assets for a project.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MediaPool {
    pub items: Vec<MediaItem>,
}

impl MediaPool {
    /// Creates a new, empty `MediaPool`.
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Adds a `MediaItem` to the pool and returns its `Uuid`.
    pub fn add(&mut self, item: MediaItem) -> Uuid {
        let id = item.id;
        self.items.push(item);
        id
    }

    /// Retrieves an immutable reference to a `MediaItem` by its `Uuid`.
    pub fn get(&self, id: Uuid) -> Option<&MediaItem> {
        self.items.iter().find(|i| i.id == id)
    }

    /// Retrieves a mutable reference to a `MediaItem` by its `Uuid`.
    pub fn get_mut(&mut self, id: Uuid) -> Option<&mut MediaItem> {
        self.items.iter_mut().find(|i| i.id == id)
    }

    /// Removes a `MediaItem` by its `Uuid` and returns it if found.
    pub fn remove(&mut self, id: Uuid) -> Option<MediaItem> {
        if let Some(pos) = self.items.iter().position(|i| i.id == id) {
            Some(self.items.remove(pos))
        } else {
            None
        }
    }

    /// Finds a `MediaItem` by its absolute file path.
    pub fn find_by_path(&self, path: &str) -> Option<&MediaItem> {
        self.items.iter().find(|i| i.file_path == path)
    }

    /// Returns the number of items in the pool.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Returns true if the pool contains no items.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Clears all items from the pool.
    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// Returns an iterator over the media items.
    pub fn iter(&self) -> std::slice::Iter<'_, MediaItem> {
        self.items.iter()
    }

    /// Returns a mutable iterator over the media items.
    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, MediaItem> {
        self.items.iter_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::{Clip, Timeline, TrackKind};

    fn make_test_metadata(duration_seconds: f64) -> MediaMetadata {
        MediaMetadata {
            width: Some(1920),
            height: Some(1080),
            duration_seconds,
            duration_pts: (duration_seconds * 60.0).round() as i64,
            timebase: Some(Rational { num: 60, den: 1 }),
            audio_channels: Some(2),
            sample_rate: Some(48000),
            file_size_bytes: 1024 * 1024,
        }
    }

    #[test]
    fn test_media_type_display_and_equality() {
        assert_eq!(MediaType::Video.to_string(), "Video");
        assert_eq!(MediaType::Audio.to_string(), "Audio");
        assert_eq!(MediaType::Image.to_string(), "Image");
        assert_eq!(MediaType::Video, MediaType::Video);
        assert_ne!(MediaType::Video, MediaType::Audio);
    }

    #[test]
    fn test_media_metadata_default_and_fps() {
        let meta = MediaMetadata::default();
        assert_eq!(meta.duration_pts, 0);
        assert_eq!(meta.duration_seconds, 0.0);
        assert_eq!(meta.width, None);
        assert_eq!(meta.height, None);
        assert_eq!(meta.fps(), None);

        let video_meta = MediaMetadata {
            width: Some(1920),
            height: Some(1080),
            duration_pts: 3600,
            duration_seconds: 60.0,
            timebase: Some(Rational { num: 60, den: 1 }),
            audio_channels: Some(2),
            sample_rate: Some(48000),
            file_size_bytes: 1024 * 1024 * 50,
        };
        assert_eq!(video_meta.fps(), Some(60.0));
    }

    #[test]
    fn test_media_item_creation_and_filename_extraction() {
        let path = "/Users/test/videos/sample_clip.mp4".to_string();
        let item = MediaItem::new(
            path.clone(),
            MediaType::Video,
            MediaMetadata::default(),
        );

        assert_eq!(item.file_path, path);
        assert_eq!(item.file_name, "sample_clip.mp4");
        assert_eq!(item.name(), "sample_clip.mp4");
        assert_eq!(item.media_type, MediaType::Video);
    }

    #[test]
    fn test_media_pool_crud_operations() {
        let mut pool = MediaPool::new();
        assert_eq!(pool.len(), 0);
        assert!(pool.is_empty());

        let item1 = MediaItem::new(
            "/assets/clip1.mp4".to_string(),
            MediaType::Video,
            make_test_metadata(5.0),
        );
        let id1 = item1.id;
        let item2 = MediaItem::new(
            "/assets/audio.wav".to_string(),
            MediaType::Audio,
            make_test_metadata(3.0),
        );
        let id2 = item2.id;
        let item3 = MediaItem::new(
            "/assets/logo.png".to_string(),
            MediaType::Image,
            MediaMetadata {
                width: Some(512),
                height: Some(512),
                duration_seconds: 0.0,
                duration_pts: 0,
                timebase: None,
                audio_channels: None,
                sample_rate: None,
                file_size_bytes: 4096,
            },
        );
        let id3 = item3.id;

        // Add
        assert_eq!(pool.add(item1), id1);
        assert_eq!(pool.add(item2), id2);
        assert_eq!(pool.add(item3), id3);
        assert_eq!(pool.len(), 3);
        assert!(!pool.is_empty());

        // Get
        let fetched = pool.get(id1).expect("item1 should be in pool");
        assert_eq!(fetched.file_name, "clip1.mp4");
        assert_eq!(fetched.media_type, MediaType::Video);

        // Get non-existent
        assert!(pool.get(Uuid::new_v4()).is_none());

        // Find by path
        let by_path = pool.find_by_path("/assets/audio.wav").expect("audio should be found");
        assert_eq!(by_path.id, id2);
        assert!(pool.find_by_path("/nonexistent/path").is_none());

        // Get mut
        if let Some(mut_item) = pool.get_mut(id1) {
            mut_item.file_name = "renamed_clip1.mp4".to_string();
        }
        assert_eq!(pool.get(id1).unwrap().file_name, "renamed_clip1.mp4");

        // Remove
        let removed = pool.remove(id2).expect("should remove item2");
        assert_eq!(removed.id, id2);
        assert_eq!(pool.len(), 2);
        assert!(pool.get(id2).is_none());

        // Remove non-existent
        assert!(pool.remove(Uuid::new_v4()).is_none());
        assert_eq!(pool.len(), 2);

        // Clear
        pool.clear();
        assert_eq!(pool.len(), 0);
        assert!(pool.is_empty());
    }

    #[test]
    fn test_media_pool_serde_roundtrip() {
        let mut pool = MediaPool::new();
        let item = MediaItem::new(
            "/media/audio.wav".to_string(),
            MediaType::Audio,
            MediaMetadata {
                width: None,
                height: None,
                duration_pts: 2400,
                duration_seconds: 40.0,
                timebase: Some(Rational { num: 60, den: 1 }),
                audio_channels: Some(2),
                sample_rate: Some(44100),
                file_size_bytes: 2048,
            },
        );
        pool.add(item);

        let json = serde_json::to_string_pretty(&pool).expect("serialization should succeed");
        let deserialized: MediaPool = serde_json::from_str(&json).expect("deserialization should succeed");

        assert_eq!(pool, deserialized);
    }

    #[test]
    fn test_non_destructive_clip_linking_multiple_subclips() {
        let mut pool = MediaPool::new();
        let total_duration_seconds = 60.0;
        let total_duration_pts = 3600; // 60s @ 60fps

        let mut meta = make_test_metadata(total_duration_seconds);
        meta.duration_pts = total_duration_pts;

        let source_media = MediaItem::new(
            "/raw/master_interview_4k.mp4".to_string(),
            MediaType::Video,
            meta,
        );
        let source_id = pool.add(source_media);

        // Create timeline
        let mut timeline = Timeline::new(Rational { num: 60, den: 1 });
        let v1 = timeline.add_track(TrackKind::Video);
        let v2 = timeline.add_track(TrackKind::Video);

        // Clip 1: In 0s -> Out 10s (0..600 PTS), Timeline: 0..600 PTS
        let clip1 = Clip::new(source_id, 0, 600, 0);
        assert_eq!(clip1.duration(), 600);
        timeline.add_clip(v1, clip1).unwrap();

        // Clip 2: In 30s -> Out 45s (1800..2700 PTS), Timeline: 600..1500 PTS
        let clip2 = Clip::new(source_id, 1800, 2700, 600);
        assert_eq!(clip2.duration(), 900);
        timeline.add_clip(v1, clip2).unwrap();

        // Clip 3 (Cutaway on v2): In 50s -> Out 55s (3000..3300 PTS), Timeline: 200..500 PTS
        let clip3 = Clip::new(source_id, 3000, 3300, 200);
        assert_eq!(clip3.duration(), 300);
        timeline.add_clip(v2, clip3).unwrap();

        // Assert non-destructive properties:
        // 1. All clips link back to the exact same source_id
        assert_eq!(timeline.tracks[0].clips[0].source_id, source_id);
        assert_eq!(timeline.tracks[0].clips[1].source_id, source_id);
        assert_eq!(timeline.tracks[1].clips[0].source_id, source_id);

        // 2. Each clip has its own independent in/out coordinates
        assert_eq!(timeline.tracks[0].clips[0].source_in, 0);
        assert_eq!(timeline.tracks[0].clips[0].source_out, 600);
        assert_eq!(timeline.tracks[0].clips[1].source_in, 1800);
        assert_eq!(timeline.tracks[0].clips[1].source_out, 2700);

        // 3. The master MediaItem in MediaPool remains completely unaltered
        let master = pool.get(source_id).unwrap();
        assert_eq!(master.metadata.duration_pts, 3600);
        assert_eq!(master.metadata.duration_seconds, 60.0);

        // 4. Timeline duration is calculated from timeline_out coordinates
        assert_eq!(timeline.duration_pts, 1500); // clip2 ends at 1500
        assert_eq!(timeline.total_clip_count(), 3);
    }
}
