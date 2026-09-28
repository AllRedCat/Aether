use flutter_rust_bridge::frb;
pub use aether_core::timeline::{Clip, Rational, Timeline, Track, TrackKind};
use uuid::Uuid;

#[allow(dead_code)]
#[frb(mirror(Rational))]
pub struct _Rational {
    pub num: i32,
    pub den: i32,
}

#[allow(dead_code)]
#[frb(mirror(TrackKind))]
pub enum _TrackKind {
    Video,
    Audio,
    Overlay,
}

#[allow(dead_code)]
#[frb(mirror(Clip))]
pub struct _Clip {
    pub id: Uuid,
    pub source_id: Uuid,
    pub source_in: i64,
    pub source_out: i64,
    pub timeline_in: i64,
    pub timeline_out: i64,
}

#[allow(dead_code)]
#[frb(mirror(Track))]
pub struct _Track {
    pub id: Uuid,
    pub kind: TrackKind,
    pub clips: Vec<Clip>,
}

#[allow(dead_code)]
#[frb(mirror(Timeline))]
pub struct _Timeline {
    pub id: Uuid,
    pub timebase: Rational,
    pub duration_pts: i64,
    pub tracks: Vec<Track>,
}

pub fn init_engine() {
    flutter_rust_bridge::setup_default_user_utils();
    aether_render::init_render();
    aether_media::init_media();
}

pub fn create_timeline() -> Timeline {
    let mut timeline = Timeline::new(Rational { num: 60, den: 1 });
    timeline.add_track(TrackKind::Video);
    timeline
}

pub fn add_track(mut timeline: Timeline, kind: TrackKind) -> Timeline {
    timeline.add_track(kind);
    timeline
}

pub fn add_clip_to_track(
    mut timeline: Timeline,
    track_id: Uuid,
    source_id: Uuid,
    source_in: i64,
    source_out: i64,
    timeline_in: i64,
) -> Result<Timeline, String> {
    let clip = Clip::new(source_id, source_in, source_out, timeline_in);
    timeline
        .add_clip(track_id, clip)
        .map_err(|e| e.to_string())?;
    Ok(timeline)
}

