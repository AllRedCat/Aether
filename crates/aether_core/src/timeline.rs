use uuid::Uuid;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rational {
    pub num: i32,
    pub den: i32,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackKind {
    Video,
    Audio,
    Overlay,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Clip {
    pub id: Uuid,
    pub source_id: Uuid,
    pub source_in: i64,
    pub source_out: i64,
    pub timeline_in: i64,
    pub timeline_out: i64,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Track {
    pub id: Uuid,
    pub kind: TrackKind,
    pub clips: Vec<Clip>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Timeline {
    pub id: Uuid,
    pub timebase: Rational,
    pub duration_pts: i64,
    pub tracks: Vec<Track>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum TimelineError {
    TrackNotFound(Uuid),
    InvalidClipBounds { timeline_in: i64, timeline_out: i64 },
    InvalidSourceBounds { source_in: i64, source_out: i64 },
}

impl std::fmt::Display for TimelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TrackNotFound(id) => write!(f, "Track with ID {} not found", id),
            Self::InvalidClipBounds {
                timeline_in,
                timeline_out,
            } => {
                write!(
                    f,
                    "Invalid clip timeline bounds: timeline_in ({}) > timeline_out ({})",
                    timeline_in, timeline_out
                )
            }
            Self::InvalidSourceBounds {
                source_in,
                source_out,
            } => {
                write!(
                    f,
                    "Invalid source bounds: source_in ({}) > source_out ({})",
                    source_in, source_out
                )
            }
        }
    }
}

impl std::error::Error for TimelineError {}

impl Clip {
    pub fn new(source_id: Uuid, source_in: i64, source_out: i64, timeline_in: i64) -> Self {
        let duration = source_out.saturating_sub(source_in);
        Self {
            id: Uuid::new_v4(),
            source_id,
            source_in,
            source_out,
            timeline_in,
            timeline_out: timeline_in.saturating_add(duration),
        }
    }

    pub fn with_id(
        id: Uuid,
        source_id: Uuid,
        source_in: i64,
        source_out: i64,
        timeline_in: i64,
        timeline_out: i64,
    ) -> Self {
        Self {
            id,
            source_id,
            source_in,
            source_out,
            timeline_in,
            timeline_out,
        }
    }

    pub fn duration(&self) -> i64 {
        self.timeline_out.saturating_sub(self.timeline_in)
    }
}

impl Track {
    pub fn new(kind: TrackKind) -> Self {
        Self {
            id: Uuid::new_v4(),
            kind,
            clips: Vec::new(),
        }
    }

    pub fn with_id(id: Uuid, kind: TrackKind) -> Self {
        Self {
            id,
            kind,
            clips: Vec::new(),
        }
    }

    pub fn duration_pts(&self) -> i64 {
        self.clips.iter().map(|c| c.timeline_out).max().unwrap_or(0)
    }

    pub fn add_clip(&mut self, clip: Clip) {
        self.clips.push(clip);
    }
}

impl Timeline {
    pub fn new(timebase: Rational) -> Self {
        Self {
            id: Uuid::new_v4(),
            timebase,
            duration_pts: 0,
            tracks: Vec::new(),
        }
    }

    pub fn new_with_default_tracks(timebase: Rational) -> Self {
        Self {
            id: Uuid::new_v4(),
            timebase,
            duration_pts: 0,
            tracks: vec![Track::new(TrackKind::Video), Track::new(TrackKind::Audio)],
        }
    }

    pub fn add_track(&mut self, kind: TrackKind) -> Uuid {
        let track = Track::new(kind);
        let id = track.id;
        self.tracks.push(track);
        id
    }

    pub fn recalculate_duration(&mut self) {
        self.duration_pts = self
            .tracks
            .iter()
            .flat_map(|track| track.clips.iter())
            .map(|clip| clip.timeline_out)
            .max()
            .unwrap_or(0)
            .max(0);
    }

    pub fn add_clip(&mut self, track_id: Uuid, clip: Clip) -> Result<(), TimelineError> {
        if clip.source_out < clip.source_in {
            return Err(TimelineError::InvalidSourceBounds {
                source_in: clip.source_in,
                source_out: clip.source_out,
            });
        }
        if clip.timeline_out < clip.timeline_in {
            return Err(TimelineError::InvalidClipBounds {
                timeline_in: clip.timeline_in,
                timeline_out: clip.timeline_out,
            });
        }
        if clip.timeline_in < 0 || clip.timeline_out < 0 {
            return Err(TimelineError::InvalidClipBounds {
                timeline_in: clip.timeline_in,
                timeline_out: clip.timeline_out,
            });
        }
        if clip.source_in < 0 || clip.source_out < 0 {
            return Err(TimelineError::InvalidSourceBounds {
                source_in: clip.source_in,
                source_out: clip.source_out,
            });
        }
        let track = self
            .tracks
            .iter_mut()
            .find(|t| t.id == track_id)
            .ok_or(TimelineError::TrackNotFound(track_id))?;
        track.add_clip(clip);
        self.recalculate_duration();
        Ok(())
    }

    pub fn total_clip_count(&self) -> usize {
        self.tracks.iter().map(|t| t.clips.len()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_clip_to_track_and_recalculate_duration() {
        let mut timeline = Timeline::new(Rational { num: 60, den: 1 });
        let track_id = timeline.add_track(TrackKind::Video);
        assert_eq!(timeline.duration_pts, 0);
        assert_eq!(timeline.total_clip_count(), 0);

        let source_id = Uuid::new_v4();
        let clip = Clip::new(source_id, 0, 120, 0);
        assert_eq!(clip.duration(), 120);
        assert_eq!(clip.timeline_in, 0);
        assert_eq!(clip.timeline_out, 120);

        let result = timeline.add_clip(track_id, clip);
        assert!(result.is_ok());

        assert_eq!(timeline.tracks[0].clips.len(), 1);
        assert_eq!(timeline.duration_pts, 120);
        assert_eq!(timeline.total_clip_count(), 1);
        assert_eq!(timeline.tracks[0].duration_pts(), 120);
    }

    #[test]
    fn test_add_multiple_clips_recalculates_max_pts() {
        let mut timeline = Timeline::new(Rational { num: 60, den: 1 });
        let video_track_id = timeline.add_track(TrackKind::Video);
        let audio_track_id = timeline.add_track(TrackKind::Audio);

        let clip1 = Clip::new(Uuid::new_v4(), 0, 60, 0); // timeline_in = 0, timeline_out = 60
        timeline.add_clip(video_track_id, clip1).unwrap();
        assert_eq!(timeline.duration_pts, 60);

        let clip2 = Clip::new(Uuid::new_v4(), 0, 180, 50); // timeline_in = 50, timeline_out = 230
        timeline.add_clip(audio_track_id, clip2).unwrap();
        assert_eq!(timeline.duration_pts, 230);

        // Add shorter clip after clip2 on video track
        let clip3 = Clip::new(Uuid::new_v4(), 0, 30, 70); // timeline_in = 70, timeline_out = 100
        timeline.add_clip(video_track_id, clip3).unwrap();
        assert_eq!(timeline.duration_pts, 230);
        assert_eq!(timeline.total_clip_count(), 3);
    }

    #[test]
    fn test_add_clip_track_not_found() {
        let mut timeline = Timeline::new(Rational { num: 60, den: 1 });
        let clip = Clip::new(Uuid::new_v4(), 0, 60, 0);
        let non_existent_id = Uuid::new_v4();
        let result = timeline.add_clip(non_existent_id, clip);
        assert_eq!(result, Err(TimelineError::TrackNotFound(non_existent_id)));
    }

    #[test]
    fn test_invalid_source_bounds() {
        let mut timeline = Timeline::new(Rational { num: 60, den: 1 });
        let track_id = timeline.add_track(TrackKind::Video);

        let clip = Clip::with_id(Uuid::new_v4(), Uuid::new_v4(), 100, 50, 0, 50);
        let result = timeline.add_clip(track_id, clip);
        assert_eq!(
            result,
            Err(TimelineError::InvalidSourceBounds {
                source_in: 100,
                source_out: 50,
            })
        );
    }

    #[test]
    fn test_invalid_clip_bounds() {
        let mut timeline = Timeline::new(Rational { num: 60, den: 1 });
        let track_id = timeline.add_track(TrackKind::Video);

        let clip = Clip::with_id(Uuid::new_v4(), Uuid::new_v4(), 0, 50, 100, 50);
        let result = timeline.add_clip(track_id, clip);
        assert_eq!(
            result,
            Err(TimelineError::InvalidClipBounds {
                timeline_in: 100,
                timeline_out: 50,
            })
        );
    }

    #[test]
    fn test_new_with_default_tracks() {
        let timeline = Timeline::new_with_default_tracks(Rational { num: 60, den: 1 });
        assert_eq!(timeline.duration_pts, 0);
        assert_eq!(timeline.tracks.len(), 2);
        assert_eq!(timeline.tracks[0].kind, TrackKind::Video);
        assert_eq!(timeline.tracks[1].kind, TrackKind::Audio);
        assert_eq!(timeline.total_clip_count(), 0);
    }

    #[test]
    fn test_recalculate_duration_empty_tracks() {
        let mut timeline = Timeline::new(Rational { num: 60, den: 1 });
        timeline.add_track(TrackKind::Video);
        timeline.recalculate_duration();
        assert_eq!(timeline.duration_pts, 0);
    }

    #[test]
    fn test_staggered_clips_with_gaps_across_tracks() {
        let mut timeline = Timeline::new(Rational { num: 60, den: 1 });
        let video_track = timeline.add_track(TrackKind::Video);
        let audio_track = timeline.add_track(TrackKind::Audio);

        // Track 1: clip from 0 to 50
        let c1 = Clip::new(Uuid::new_v4(), 0, 50, 0);
        timeline.add_clip(video_track, c1).unwrap();
        assert_eq!(timeline.duration_pts, 50);

        // Track 2: clip starting after gap at 100 with duration 100 -> ends at 200
        let c2 = Clip::new(Uuid::new_v4(), 0, 100, 100);
        timeline.add_clip(audio_track, c2).unwrap();
        assert_eq!(timeline.duration_pts, 200);
    }

    #[test]
    fn test_zero_duration_clip() {
        let mut timeline = Timeline::new(Rational { num: 60, den: 1 });
        let track_id = timeline.add_track(TrackKind::Video);

        let clip = Clip::new(Uuid::new_v4(), 10, 10, 25);
        assert_eq!(clip.duration(), 0);
        assert_eq!(clip.timeline_in, 25);
        assert_eq!(clip.timeline_out, 25);

        assert!(timeline.add_clip(track_id, clip).is_ok());
        assert_eq!(timeline.duration_pts, 25);
    }

    #[test]
    fn test_track_with_id_and_duration() {
        let custom_id = Uuid::new_v4();
        let mut track = Track::with_id(custom_id, TrackKind::Overlay);
        assert_eq!(track.id, custom_id);
        assert_eq!(track.kind, TrackKind::Overlay);
        assert_eq!(track.duration_pts(), 0);

        let clip = Clip::new(Uuid::new_v4(), 0, 80, 20);
        track.add_clip(clip);
        assert_eq!(track.duration_pts(), 100);
    }

    #[test]
    fn test_timeline_error_display() {
        let track_id = Uuid::new_v4();
        let err1 = TimelineError::TrackNotFound(track_id);
        assert!(err1.to_string().contains(&track_id.to_string()));

        let err2 = TimelineError::InvalidClipBounds {
            timeline_in: 100,
            timeline_out: 50,
        };
        assert!(err2.to_string().contains("100"));
        assert!(err2.to_string().contains("50"));

        let err3 = TimelineError::InvalidSourceBounds {
            source_in: 80,
            source_out: 40,
        };
        assert!(err3.to_string().contains("80"));
        assert!(err3.to_string().contains("40"));
    }

    #[test]
    fn test_recalculate_duration_clamp_non_negative() {
        let mut timeline = Timeline::new(Rational { num: 60, den: 1 });
        let _track_id = timeline.add_track(TrackKind::Video);

        // Manually push a clip with negative timeline_out
        let negative_clip = Clip::with_id(
            Uuid::new_v4(),
            Uuid::new_v4(),
            0,
            10,
            -50,
            -40,
        );
        timeline.tracks[0].add_clip(negative_clip);
        timeline.recalculate_duration();
        assert_eq!(timeline.duration_pts, 0);
    }
}


