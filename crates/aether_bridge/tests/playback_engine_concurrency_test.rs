use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use aether_bridge::api::{
    close_preview_session, create_preview_session, get_preview_state, init_engine,
    preview_pause, preview_play, preview_seek_pts,
};
use aether_bridge::session::SessionRegistry;

/// Simple fast PRNG (Xorshift64) for repeatable concurrency stress tests.
struct FastRng {
    state: u64,
}

impl FastRng {
    fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 0xbadc0ffeedead } else { seed },
        }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    fn next_i64_range(&mut self, min: i64, max: i64) -> i64 {
        if min >= max {
            return min;
        }
        let range = (max - min) as u64;
        min + (self.next_u64() % range) as i64
    }
}

fn sample_mp4_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../aether_media/tests/fixtures/sample_64x64_10frames.mp4")
}

#[test]
fn test_continuous_playback_lookahead_buffer_and_transport() {
    init_engine();

    let uri = "synthetic://lookahead_test?width=64&height=64&fps=30&duration=3.0";
    let info = create_preview_session(uri.to_string()).expect("Create session");

    let session = SessionRegistry::global()
        .get_session(&info.session_id)
        .expect("Retrieve session Arc");

    // Initially paused, buffer begins filling up to capacity (up to 16 frames)
    thread::sleep(Duration::from_millis(50));
    assert!(
        session.lookahead_buffer.len() > 0,
        "Lookahead buffer should pre-fill in background while paused, got len: {}",
        session.lookahead_buffer.len()
    );

    // Play continuously for 200ms
    preview_play(info.session_id.clone()).expect("Play");
    thread::sleep(Duration::from_millis(200));

    let state = get_preview_state(info.session_id.clone()).expect("State query");
    assert!(state.is_playing);
    assert!(state.current_pts > 0, "Playback should advance PTS");

    // Pause cleanly freezes playhead
    preview_pause(info.session_id.clone()).expect("Pause");
    let state_paused = get_preview_state(info.session_id.clone()).expect("State query");
    assert!(!state_paused.is_playing);
    let paused_pts = state_paused.current_pts;

    // Buffer remains populated and ready for immediate resumption
    thread::sleep(Duration::from_millis(50));
    let state_check = get_preview_state(info.session_id.clone()).expect("State query");
    assert_eq!(state_check.current_pts, paused_pts);

    close_preview_session(info.session_id).expect("Close session");
}

#[test]
fn test_concurrency_continuous_playback_and_rapid_random_seeks() {
    init_engine();

    let uri = "synthetic://concurrency_seek?width=64&height=64&fps=60&duration=10.0";
    let info = create_preview_session(uri.to_string()).expect("Create session");
    let session_id = Arc::new(info.session_id);
    let duration = info.duration_pts;

    // Start continuous playback
    preview_play((*session_id).clone()).expect("Start continuous playback");

    let stop_flag = Arc::new(AtomicBool::new(false));
    let seek_counter = Arc::new(AtomicUsize::new(0));

    // Spawn 4 concurrent worker threads executing consecutive random seeks
    let num_seek_threads = 4;
    let mut handles = Vec::new();

    for thread_idx in 0..num_seek_threads {
        let sid = Arc::clone(&session_id);
        let stop = Arc::clone(&stop_flag);
        let counter = Arc::clone(&seek_counter);
        let seed = 0xabcdef + (thread_idx as u64) * 1000;

        handles.push(thread::spawn(move || {
            let mut rng = FastRng::new(seed);
            while !stop.load(Ordering::Relaxed) {
                let target_pts = rng.next_i64_range(-20, duration + 20);
                let res = preview_seek_pts((*sid).clone(), target_pts);
                assert!(res.is_ok(), "Seek to PTS {} must succeed", target_pts);
                counter.fetch_add(1, Ordering::Relaxed);
                thread::sleep(Duration::from_millis(2));
            }
        }));
    }

    // Spawn 2 concurrent state observer threads
    for _ in 0..2 {
        let sid = Arc::clone(&session_id);
        let stop = Arc::clone(&stop_flag);
        handles.push(thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                if let Ok(st) = get_preview_state((*sid).clone()) {
                    assert!(st.current_pts >= 0);
                    assert!(st.current_pts <= duration);
                }
                thread::sleep(Duration::from_millis(5));
            }
        }));
    }

    // Let the concurrency test run under load for 400ms
    thread::sleep(Duration::from_millis(400));
    stop_flag.store(true, Ordering::Relaxed);

    for h in handles {
        h.join().expect("Worker thread panicked during concurrency stress");
    }

    let completed_seeks = seek_counter.load(Ordering::Relaxed);
    assert!(
        completed_seeks > 50,
        "Expected at least 50 concurrent seeks, completed: {}",
        completed_seeks
    );

    // Pause and verify stable shutdown
    preview_pause((*session_id).clone()).expect("Pause");
    let final_state = get_preview_state((*session_id).clone()).expect("Final state");
    assert!(!final_state.is_playing);

    close_preview_session((*session_id).clone()).expect("Close session");
}

#[test]
fn test_mp4_fixture_lookahead_and_gop_keyframe_seeking() {
    init_engine();

    let mp4 = sample_mp4_path();
    if !mp4.exists() {
        eprintln!("Skipping MP4 fixture test: fixture not found at {:?}", mp4);
        return;
    }
    let path_str = mp4.to_str().unwrap().to_string();

    let info = create_preview_session(path_str).expect("Create MP4 session");
    let session = SessionRegistry::global()
        .get_session(&info.session_id)
        .expect("Retrieve session");

    // Verify GOP index was loaded or built for real MP4
    assert!(
        session.frame_index.is_some(),
        "Video MP4 session should have an indexed FrameIndex"
    );

    // Play continuously
    preview_play(info.session_id.clone()).expect("Play MP4");
    thread::sleep(Duration::from_millis(100));

    // Rapidly seek to multiple arbitrary PTS points
    let seek_targets = [0, 2, 4, 1, 6, 8, info.duration_pts];
    for &pts in &seek_targets {
        let frame_opt = preview_seek_pts(info.session_id.clone(), pts).expect("MP4 seek PTS");
        if let Some(frame) = frame_opt {
            assert_eq!(frame.width, info.width);
            assert_eq!(frame.height, info.height);
            assert_eq!(frame.rgba_bytes.len(), (info.width * info.height * 4) as usize);
        }
    }

    preview_pause(info.session_id.clone()).expect("Pause MP4");
    close_preview_session(info.session_id).expect("Close MP4 session");
}
