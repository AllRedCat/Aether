use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use aether_bridge::api::{
    close_preview_session, create_preview_session, extract_single_frame, get_preview_state,
    init_engine, preview_pause, preview_play, preview_seek_pts, preview_seek_seconds,
};

/// Simple fast PRNG (Xorshift64) for deterministic repeatable stress tests.
struct FastRng {
    state: u64,
}

impl FastRng {
    fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 0xdeadbeefcafe } else { seed },
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

// =========================================================================
// 1. Rapid Play/Pause Cycling (100+ rapid toggles)
// =========================================================================

#[test]
fn test_rapid_play_pause_100_cycles_sync() {
    init_engine();
    let uri = "synthetic://rapid_toggle?width=64&height=64&fps=30&duration=4.0";
    let info = create_preview_session(uri.to_string()).expect("Create session");

    let start = Instant::now();

    // 100 rapid toggles with 0 delay (synchronous hammer)
    for i in 0..100 {
        preview_play(info.session_id.clone()).unwrap_or_else(|e| panic!("Play failed at {}: {}", i, e));
        preview_pause(info.session_id.clone()).unwrap_or_else(|e| panic!("Pause failed at {}: {}", i, e));
    }

    let elapsed = start.elapsed();
    println!("100 rapid synchronous play/pause toggles completed in {:?}", elapsed);

    // Verify session state is cleanly paused
    let state = get_preview_state(info.session_id.clone()).expect("Get state");
    assert!(!state.is_playing, "Final state should be paused");

    // Another 50 rapid toggles with micro-sleeps (varying intervals)
    for i in 0..50 {
        preview_play(info.session_id.clone()).unwrap_or_else(|e| panic!("Play sleep failed at {}: {}", i, e));
        if i % 3 == 0 {
            thread::sleep(Duration::from_micros(500));
        } else if i % 3 == 1 {
            thread::sleep(Duration::from_millis(1));
        }
        preview_pause(info.session_id.clone()).unwrap_or_else(|e| panic!("Pause sleep failed at {}: {}", i, e));
    }

    // Verify session is still responsive: seek and play briefly
    let frame = preview_seek_seconds(info.session_id.clone(), 1.0)
        .expect("Seek after rapid toggles")
        .expect("Should return frame");
    assert_eq!(frame.width, 64);
    assert_eq!(frame.height, 64);

    let state_after_seek = get_preview_state(info.session_id.clone()).expect("State after seek");
    assert!((state_after_seek.current_seconds - 1.0).abs() < 0.1);

    close_preview_session(info.session_id).expect("Close session");
}

#[test]
fn test_rapid_play_pause_multithreaded_hammer() {
    init_engine();
    let uri = "synthetic://threaded_toggle?width=64&height=64&fps=30&duration=5.0";
    let info = create_preview_session(uri.to_string()).expect("Create session");
    let session_id = Arc::new(info.session_id);

    let num_threads = 4;
    let toggles_per_thread = 50; // Total 200 concurrent play/pause operations
    let completed_count = Arc::new(AtomicUsize::new(0));

    let mut handles = Vec::new();
    for thread_idx in 0..num_threads {
        let sid = Arc::clone(&session_id);
        let counter = Arc::clone(&completed_count);
        handles.push(thread::spawn(move || {
            let mut rng = FastRng::new(100 + thread_idx as u64);
            for _ in 0..toggles_per_thread {
                let _ = preview_play((*sid).clone());
                if rng.next_u64() % 4 == 0 {
                    thread::sleep(Duration::from_micros(200));
                }
                let _ = preview_pause((*sid).clone());
                counter.fetch_add(1, Ordering::SeqCst);
            }
        }));
    }

    for h in handles {
        h.join().expect("Worker thread panicked during play/pause hammer");
    }

    assert_eq!(
        completed_count.load(Ordering::SeqCst),
        num_threads * toggles_per_thread,
        "All concurrent toggles must complete without deadlock"
    );

    // Settle session to paused state
    let _ = preview_pause((*session_id).clone());
    let state = get_preview_state((*session_id).clone()).expect("Get state after multithreaded hammer");
    assert!(!state.is_playing);

    // Verify session remains functional
    let frame_opt = preview_seek_pts((*session_id).clone(), 15).expect("Seek after hammer");
    assert!(frame_opt.is_some());

    close_preview_session((*session_id).clone()).expect("Close session");
}

// =========================================================================
// 2. High-Frequency Scrub Seeking Across Boundaries
// =========================================================================

#[test]
fn test_scrub_boundary_seeking_extremes() {
    init_engine();
    let uri = "synthetic://boundary_seek?width=64&height=64&fps=30&duration=2.0";
    let info = create_preview_session(uri.to_string()).expect("Create session");
    let duration = info.duration_pts;

    let pts_per_frame = if info.fps > 0.0 {
        ((info.timebase.num as f64 / (info.timebase.den as f64 * info.fps)).ceil() as i64).max(1)
    } else {
        2
    };

    // Test a wide spectrum of boundary and pathological PTS values
    let boundary_pts_cases = [
        // Extreme negative values
        i64::MIN,
        i64::MIN + 1,
        i64::MIN / 2,
        -1_000_000_000,
        -999_999,
        -100,
        -1,
        // Zero boundary
        0,
        // Within valid range
        1,
        duration / 2,
        duration - 1,
        duration,
        // Extreme positive values (beyond duration)
        duration + 1,
        duration + 10,
        duration + 1_000,
        1_000_000,
        i64::MAX / 2,
        i64::MAX - 1,
        i64::MAX,
    ];

    for &pts in &boundary_pts_cases {
        let result = preview_seek_pts(info.session_id.clone(), pts);
        assert!(
            result.is_ok(),
            "preview_seek_pts must succeed without error for PTS {}, got: {:?}",
            pts,
            result.err()
        );

        let state = get_preview_state(info.session_id.clone()).expect("Get state");
        if pts <= 0 {
            assert_eq!(
                state.current_pts, 0,
                "Negative or zero PTS {} must clamp to 0, got {}",
                pts, state.current_pts
            );
        } else if pts >= duration {
            assert_eq!(
                state.current_pts, duration,
                "Beyond-duration PTS {} must clamp to duration_pts {}, got {}",
                pts, duration, state.current_pts
            );
        } else {
            // Quantized to nearest decoded video frame
            assert!(
                (state.current_pts - pts).abs() <= pts_per_frame,
                "In-bounds PTS {} must be within {} PTS units of current_pts {}, got diff {}",
                pts,
                pts_per_frame,
                state.current_pts,
                (state.current_pts - pts).abs()
            );
        }
    }

    // Test fractional seconds boundary cases
    let boundary_sec_cases = [
        -1000.0,
        -1.0,
        -0.0001,
        0.0,
        info.duration_seconds / 2.0,
        info.duration_seconds,
        info.duration_seconds + 0.1,
        info.duration_seconds + 1000.0,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NAN,
    ];

    for &sec in &boundary_sec_cases {
        let result = preview_seek_seconds(info.session_id.clone(), sec);
        assert!(
            result.is_ok(),
            "preview_seek_seconds must not panic or error for sec {}, got: {:?}",
            sec,
            result.err()
        );

        let state = get_preview_state(info.session_id.clone()).expect("Get state");
        assert!(
            state.current_pts >= 0 && state.current_pts <= duration,
            "current_pts {} must remain clamped within [0, {}] for seconds {}",
            state.current_pts,
            duration,
            sec
        );
    }

    close_preview_session(info.session_id).expect("Close session");
}

#[test]
fn test_high_frequency_random_scrub_sequence() {
    init_engine();
    let uri = "synthetic://scrub_jitter?width=64&height=64&fps=30&duration=3.0";
    let info = create_preview_session(uri.to_string()).expect("Create session");
    let duration = info.duration_pts;

    let pts_per_frame = if info.fps > 0.0 {
        ((info.timebase.num as f64 / (info.timebase.den as f64 * info.fps)).ceil() as i64).max(1)
    } else {
        2
    };

    let mut rng = FastRng::new(0x1337c0de);
    let iterations = 1000;
    let start = Instant::now();

    for i in 0..iterations {
        // Generate random PTS ranging from well below 0 to well above duration
        let target_pts = rng.next_i64_range(-100, duration + 100);
        let frame_res = preview_seek_pts(info.session_id.clone(), target_pts);
        assert!(
            frame_res.is_ok(),
            "Iteration {} failed seeking to PTS {}",
            i,
            target_pts
        );

        let expected_clamped = target_pts.clamp(0, duration);
        let state = get_preview_state(info.session_id.clone()).expect("State query");
        assert!(
            (state.current_pts - expected_clamped).abs() <= pts_per_frame,
            "State current_pts mismatch on iteration {}: target_pts={}, clamped={}, actual={}",
            i,
            target_pts,
            expected_clamped,
            state.current_pts
        );

        if let Ok(Some(frame)) = frame_res {
            assert_eq!(frame.width, 64);
            assert_eq!(frame.height, 64);
            assert_eq!(frame.rgba_bytes.len(), 64 * 64 * 4);
            assert_eq!(frame.row_stride_bytes, 64 * 4);
        }
    }

    let elapsed = start.elapsed();
    println!(
        "{} random boundary scrubs completed in {:?} ({:.2} ms/seek)",
        iterations,
        elapsed,
        elapsed.as_secs_f64() * 1000.0 / iterations as f64
    );

    close_preview_session(info.session_id).expect("Close session");
}

#[test]
fn test_scrub_seeking_during_active_playback() {
    init_engine();
    let uri = "synthetic://seek_while_play?width=64&height=64&fps=30&duration=5.0";
    let info = create_preview_session(uri.to_string()).expect("Create session");
    let duration = info.duration_pts;

    // Start playback: ticker thread is actively decoding in background
    preview_play(info.session_id.clone()).expect("Start play");
    let state_play = get_preview_state(info.session_id.clone()).expect("Get play state");
    assert!(state_play.is_playing);

    let mut rng = FastRng::new(0x42);
    // Fire 60 rapid random seeks while background playback is running
    for i in 0..60 {
        let pts = rng.next_i64_range(-20, duration + 20);
        let _ = preview_seek_pts(info.session_id.clone(), pts);
        if i % 10 == 0 {
            thread::sleep(Duration::from_millis(5));
        }
    }

    // Pause playback
    preview_pause(info.session_id.clone()).expect("Pause");
    let state_paused = get_preview_state(info.session_id.clone()).expect("Get paused state");
    assert!(!state_paused.is_playing);

    // Verify session is still fully functional after interleaved play and seek
    let f = preview_seek_seconds(info.session_id.clone(), 0.5)
        .expect("Seek")
        .expect("Frame");
    assert_eq!(f.width, 64);

    close_preview_session(info.session_id).expect("Close session");
}

// =========================================================================
// 3. Concurrent Multiple Sessions
// =========================================================================

#[test]
fn test_concurrent_multiple_sessions_stress() {
    init_engine();
    let session_count = 20;
    let mut infos = Vec::new();
    let mut seen_session_ids = HashSet::new();
    let mut seen_texture_ids = HashSet::new();

    // 1. Create 20 concurrent sessions
    for i in 0..session_count {
        let uri = format!(
            "synthetic://multi_{}?width={}&height={}&fps=30&duration=3.0",
            i,
            32 + i * 2,
            32 + i * 2
        );
        let info = create_preview_session(uri).expect("Create session");

        assert!(
            seen_session_ids.insert(info.session_id.clone()),
            "Session IDs must be strictly unique: duplicate found: {}",
            info.session_id
        );
        assert!(
            seen_texture_ids.insert(info.texture_id),
            "Texture IDs must be strictly unique: duplicate found: {}",
            info.texture_id
        );

        infos.push(info);
    }

    assert_eq!(seen_session_ids.len(), session_count);
    assert_eq!(seen_texture_ids.len(), session_count);

    // 2. Multithreaded concurrent operations on all sessions
    let infos_arc = Arc::new(infos);
    let worker_count = 4;
    let mut handles = Vec::new();

    for worker_id in 0..worker_count {
        let all_infos = Arc::clone(&infos_arc);
        handles.push(thread::spawn(move || {
            let mut rng = FastRng::new(500 + worker_id as u64);
            // Each worker interacts with a partition of sessions
            for info in all_infos.iter() {
                let sid = info.session_id.clone();
                // Play
                let _ = preview_play(sid.clone());
                // Random seek
                let target = rng.next_i64_range(0, info.duration_pts);
                let _ = preview_seek_pts(sid.clone(), target);
                // Pause
                let _ = preview_pause(sid.clone());
                // Get state
                let state_res = get_preview_state(sid.clone());
                assert!(state_res.is_ok(), "Concurrent state query must succeed");
            }
        }));
    }

    for h in handles {
        h.join().expect("Worker thread panicked during concurrent session stress");
    }

    // 3. Verify session isolation: seek session 0, verify session 1 PTS unaffected
    let s0_id = infos_arc[0].session_id.clone();
    let s1_id = infos_arc[1].session_id.clone();

    let _ = preview_seek_pts(s0_id.clone(), 10);
    let _ = preview_seek_pts(s1_id.clone(), 50);

    let st0 = get_preview_state(s0_id.clone()).expect("Get s0 state");
    let st1 = get_preview_state(s1_id.clone()).expect("Get s1 state");

    assert_eq!(st0.current_pts, 10, "s0 PTS must not be overwritten by s1");
    assert_eq!(st1.current_pts, 50, "s1 PTS must not be overwritten by s0");

    // 4. Close all sessions cleanly
    for info in infos_arc.iter() {
        close_preview_session(info.session_id.clone()).expect("Close session");
    }

    // Verify all sessions are closed
    for info in infos_arc.iter() {
        assert!(
            get_preview_state(info.session_id.clone()).is_err(),
            "Session {} should no longer exist after close",
            info.session_id
        );
    }
}

// =========================================================================
// 4. Resource Cleanup: Loop Creation and Closure (No Leaks)
// =========================================================================

#[test]
fn test_resource_cleanup_loop_no_leak() {
    init_engine();
    let iterations = 100;
    let start = Instant::now();

    for i in 0..iterations {
        let uri = format!(
            "synthetic://cleanup_loop_{}?width=64&height=64&fps=30&duration=1.0",
            i
        );
        let info = create_preview_session(uri).expect("Session creation failed in cleanup loop");

        // Verify active
        let s1 = get_preview_state(info.session_id.clone()).expect("Initial state");
        assert_eq!(s1.current_pts, 0);

        // Play briefly to spawn ticker thread
        preview_play(info.session_id.clone()).expect("Play in loop");

        // Seek while playing
        let _ = preview_seek_seconds(info.session_id.clone(), 0.5);

        // Pause
        preview_pause(info.session_id.clone()).expect("Pause in loop");

        // Close session
        close_preview_session(info.session_id.clone()).expect("Close in loop");

        // Assert session is immediately inaccessible from registry
        let get_closed = get_preview_state(info.session_id.clone());
        assert!(
            get_closed.is_err(),
            "Closed session {} must be removed from SessionRegistry",
            info.session_id
        );
    }

    let elapsed = start.elapsed();
    println!(
        "{} session create-play-seek-pause-close cycles completed in {:?} ({:.2} ms/cycle)",
        iterations,
        elapsed,
        elapsed.as_secs_f64() * 1000.0 / iterations as f64
    );

    // Sleep briefly to ensure all background OS threads have exited cleanly
    thread::sleep(Duration::from_millis(50));

    // Verify a fresh session can be created and operated after 100 cycles
    let final_uri = "synthetic://final_check?width=64&height=64&fps=30&duration=1.0";
    let final_info = create_preview_session(final_uri.to_string()).expect("Final session creation");
    assert!(final_info.texture_id >= 1000 + iterations as i64);

    let final_frame = preview_seek_seconds(final_info.session_id.clone(), 0.25)
        .expect("Final seek")
        .expect("Final frame");
    assert_eq!(final_frame.width, 64);

    close_preview_session(final_info.session_id).expect("Final close");
}

// =========================================================================
// 5. MP4 Fixture Real Media Stress
// =========================================================================

#[test]
fn test_mp4_fixture_rapid_play_pause_and_boundary_scrub() {
    init_engine();
    let mp4 = sample_mp4_path();
    if !mp4.exists() {
        eprintln!("Skipping mp4 stress test; fixture not found");
        return;
    }
    let path_str = mp4.to_str().unwrap().to_string();

    let info = create_preview_session(path_str.clone()).expect("Create mp4 session");
    let duration = info.duration_pts;

    // 50 rapid play/pause toggles on real MP4 decoder
    for _ in 0..50 {
        let _ = preview_play(info.session_id.clone());
        let _ = preview_pause(info.session_id.clone());
    }

    let state = get_preview_state(info.session_id.clone()).expect("Get state");
    assert!(!state.is_playing);

    // Boundary scrubs on real MP4
    let _ = preview_seek_pts(info.session_id.clone(), -50);
    let s_neg = get_preview_state(info.session_id.clone()).unwrap();
    assert_eq!(s_neg.current_pts, 0);

    let _ = preview_seek_pts(info.session_id.clone(), duration + 100);
    let s_over = get_preview_state(info.session_id.clone()).unwrap();
    assert_eq!(s_over.current_pts, duration);

    // Stateless single frame extractions on real MP4
    for pts in [-10, 0, duration / 2, duration, duration + 50] {
        let f = extract_single_frame(path_str.clone(), pts);
        assert!(f.is_ok(), "Stateless extract on MP4 for PTS {} should succeed, got: {:?}", pts, f.err());
        let frame = f.unwrap();
        assert_eq!(frame.width, 64);
        assert_eq!(frame.height, 64);
    }

    close_preview_session(info.session_id).expect("Close mp4 session");
}
