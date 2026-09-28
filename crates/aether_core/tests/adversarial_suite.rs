use aether_core::timeline::{Clip, Rational, Timeline, TimelineError, TrackKind};
use uuid::Uuid;

#[test]
fn test_adversarial_empty_timeline_and_empty_tracks() {
    let mut tl = Timeline::new(Rational { num: 60, den: 1 });
    assert_eq!(tl.duration_pts, 0);
    assert_eq!(tl.total_clip_count(), 0);

    // Add empty tracks
    let t1 = tl.add_track(TrackKind::Video);
    let _t2 = tl.add_track(TrackKind::Audio);
    let _t3 = tl.add_track(TrackKind::Overlay);
    tl.recalculate_duration();
    assert_eq!(tl.duration_pts, 0);
    assert_eq!(tl.total_clip_count(), 0);

    // Add clip to nonexistent track
    let random_id = Uuid::new_v4();
    let clip = Clip::new(Uuid::new_v4(), 0, 100, 0);
    let res = tl.add_clip(random_id, clip);
    assert_eq!(res, Err(TimelineError::TrackNotFound(random_id)));
    assert_eq!(tl.duration_pts, 0);
    assert_eq!(tl.total_clip_count(), 0);

    // Add clip to t1
    let clip1 = Clip::new(Uuid::new_v4(), 0, 100, 0);
    assert!(tl.add_clip(t1, clip1).is_ok());
    assert_eq!(tl.duration_pts, 100);
    assert_eq!(tl.total_clip_count(), 1);

    // t2 is still empty
    assert_eq!(tl.tracks[1].clips.len(), 0);
    assert_eq!(tl.tracks[1].duration_pts(), 0);
}

#[test]
fn test_adversarial_zero_length_clip() {
    let mut tl = Timeline::new(Rational { num: 60, den: 1 });
    let track_id = tl.add_track(TrackKind::Video);

    // Zero-length source: source_in == source_out
    let clip = Clip::new(Uuid::new_v4(), 50, 50, 200);
    assert_eq!(clip.duration(), 0);
    assert_eq!(clip.timeline_in, 200);
    assert_eq!(clip.timeline_out, 200);

    let res = tl.add_clip(track_id, clip);
    // Empirical observation of whether zero-duration is accepted:
    assert!(res.is_ok());
    assert_eq!(tl.duration_pts, 200);
}

#[test]
fn test_adversarial_inverted_bounds() {
    let mut tl = Timeline::new(Rational { num: 60, den: 1 });
    let track_id = tl.add_track(TrackKind::Video);

    // Case 1: source_out < source_in
    let clip1 = Clip::new(Uuid::new_v4(), 100, 50, 0);
    let res1 = tl.add_clip(track_id, clip1);
    assert_eq!(
        res1,
        Err(TimelineError::InvalidSourceBounds {
            source_in: 100,
            source_out: 50
        })
    );

    // Case 2: timeline_out < timeline_in via with_id
    let clip2 = Clip::with_id(Uuid::new_v4(), Uuid::new_v4(), 0, 100, 500, 400);
    let res2 = tl.add_clip(track_id, clip2);
    assert_eq!(
        res2,
        Err(TimelineError::InvalidClipBounds {
            timeline_in: 500,
            timeline_out: 400
        })
    );

    // Existing duration not corrupted by rejected clips
    assert_eq!(tl.duration_pts, 0);
    assert_eq!(tl.total_clip_count(), 0);
}

#[test]
fn test_adversarial_out_of_order_and_multi_track_pts() {
    let mut tl = Timeline::new(Rational { num: 30, den: 1 });
    let video_track = tl.add_track(TrackKind::Video);
    let audio_track = tl.add_track(TrackKind::Audio);

    // Add clip late in time on audio track
    let a1 = Clip::new(Uuid::new_v4(), 0, 300, 1000); // 1000..1300
    tl.add_clip(audio_track, a1).unwrap();
    assert_eq!(tl.duration_pts, 1300);

    // Add earlier clip on video track
    let v1 = Clip::new(Uuid::new_v4(), 0, 200, 0); // 0..200
    tl.add_clip(video_track, v1).unwrap();
    assert_eq!(tl.duration_pts, 1300); // max still 1300

    // Add overlapping clip on video track beyond audio track
    let v2 = Clip::new(Uuid::new_v4(), 0, 500, 1000); // 1000..1500
    tl.add_clip(video_track, v2).unwrap();
    assert_eq!(tl.duration_pts, 1500); // max now 1500

    // Add middle clip on audio track
    let a2 = Clip::new(Uuid::new_v4(), 0, 100, 500); // 500..600
    tl.add_clip(audio_track, a2).unwrap();
    assert_eq!(tl.duration_pts, 1500);
    assert_eq!(tl.total_clip_count(), 4);
}

#[test]
fn test_adversarial_integer_overflow_saturation() {
    let mut tl = Timeline::new(Rational { num: 60, den: 1 });
    let track_id = tl.add_track(TrackKind::Video);

    // Huge bounds near i64::MAX
    let source_id = Uuid::new_v4();
    let clip = Clip::new(source_id, 0, i64::MAX, 100);
    // saturating_add should cap at i64::MAX without panicking
    assert_eq!(clip.timeline_out, i64::MAX);

    let res = tl.add_clip(track_id, clip);
    assert!(res.is_ok());
    assert_eq!(tl.duration_pts, i64::MAX);
}

#[test]
fn test_adversarial_negative_pts_behavior() {
    let mut tl = Timeline::new(Rational { num: 60, den: 1 });
    let track_id = tl.add_track(TrackKind::Video);

    // Negative timeline_in with positive duration
    let clip = Clip::new(Uuid::new_v4(), 0, 50, -100);
    assert_eq!(clip.timeline_in, -100);
    assert_eq!(clip.timeline_out, -50);

    // Empirical check: does add_clip reject or accept negative timestamps?
    let res = tl.add_clip(track_id, clip);
    // Currently, add_clip does NOT check negative bounds, so it accepts it
    if res.is_ok() {
        // If accepted, duration_pts becomes -50!
        assert_eq!(tl.duration_pts, -50);
    }
}
