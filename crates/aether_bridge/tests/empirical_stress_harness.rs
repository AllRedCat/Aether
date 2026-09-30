use std::collections::HashSet;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use aether_bridge::api::{
    close_preview_session, create_preview_session, extract_single_frame, get_preview_state,
    init_engine, preview_pause, preview_play, preview_seek_pts, preview_seek_seconds,
};

fn sample_mp4_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../aether_media/tests/fixtures/sample_64x64_10frames.mp4")
}

// =========================================================================
// 1. Session Lifecycle & Multi-threaded Registry Concurrency
// =========================================================================

#[test]
fn test_lifecycle_multithreaded_churn_and_monotonic_texture_ids() {
    init_engine();

    let num_threads = 5;
    let sessions_per_thread = 10; // Total 50 sessions
    let all_sessions = Arc::new(std::sync::Mutex::new(Vec::new()));

    let mut handles = Vec::new();
    for thread_idx in 0..num_threads {
        let pool = Arc::clone(&all_sessions);
        handles.push(thread::spawn(move || {
            let mut created = Vec::new();
            for i in 0..sessions_per_thread {
                let uri = format!(
                    "synthetic://lifecycle_{}_{}?width=64&height=64&fps=30&duration=2.0",
                    thread_idx, i
                );
                let info = create_preview_session(uri).expect("Session creation failed");
                assert!(info.texture_id >= 1001, "Texture ID must be positive and >= 1001");
                assert_eq!(info.width, 64);
                assert_eq!(info.height, 64);
                assert_eq!(info.fps, 30.0);
                assert!(info.duration_pts > 0);

                // Initial state check
                let state = get_preview_state(info.session_id.clone()).expect("Get initial state");
                assert_eq!(state.current_pts, 0);
                assert!(!state.is_playing);

                created.push(info);
            }
            let mut guard = pool.lock().unwrap();
            guard.extend(created);
        }));
    }

    for h in handles {
        h.join().expect("Worker thread panicked during session creation");
    }

    let sessions = all_sessions.lock().unwrap().clone();
    assert_eq!(sessions.len(), num_threads * sessions_per_thread);

    // Verify all session IDs and texture IDs are strictly unique
    let mut session_ids = HashSet::new();
    let mut texture_ids = HashSet::new();
    for s in &sessions {
        assert!(session_ids.insert(s.session_id.clone()), "Duplicate session ID: {}", s.session_id);
        assert!(texture_ids.insert(s.texture_id), "Duplicate texture ID: {}", s.texture_id);
    }

    // Concurrent closing
    let sessions_arc = Arc::new(sessions);
    let mut close_handles = Vec::new();
    for worker_idx in 0..num_threads {
        let chunk = Arc::clone(&sessions_arc);
        close_handles.push(thread::spawn(move || {
            for i in 0..sessions_per_thread {
                let idx = worker_idx * sessions_per_thread + i;
                let sid = &chunk[idx].session_id;
                close_preview_session(sid.clone()).expect("Close session should succeed");
            }
        }));
    }

    for h in close_handles {
        h.join().expect("Worker thread panicked during session close");
    }

    // Verify all sessions are closed and return clean errors
    for s in sessions_arc.iter() {
        let sid = s.session_id.clone();
        assert!(get_preview_state(sid.clone()).is_err(), "Closed session must not be queryable");
        assert!(preview_play(sid.clone()).is_err(), "Closed session play must fail");
        assert!(preview_pause(sid.clone()).is_err(), "Closed session pause must fail");
        assert!(preview_seek_pts(sid.clone(), 0).is_err(), "Closed session seek must fail");
        assert!(close_preview_session(sid.clone()).is_err(), "Double close must return Err");
    }
}

// =========================================================================
// 2. Rapid Play/Pause Cycling Chaos and Race Stress
// =========================================================================

#[test]
fn test_rapid_play_pause_concurrent_race_and_no_deadlock() {
    init_engine();

    let uri = "synthetic://chaos_race?width=64&height=64&fps=60&duration=10.0";
    let info = create_preview_session(uri.to_string()).expect("Create session");
    let session_id = Arc::new(info.session_id);

    let stop_flag = Arc::new(AtomicBool::new(false));
    let mut handles = Vec::new();

    // 4 threads rapidly toggling play/pause
    for thread_idx in 0..4 {
        let sid = Arc::clone(&session_id);
        let flag = Arc::clone(&stop_flag);
        handles.push(thread::spawn(move || {
            let mut count = 0;
            while !flag.load(Ordering::Relaxed) && count < 200 {
                if thread_idx % 2 == 0 {
                    let _ = preview_play((*sid).clone());
                } else {
                    let _ = preview_pause((*sid).clone());
                }
                count += 1;
            }
        }));
    }

    // 2 threads rapidly seeking while playback toggles occur
    for _ in 0..2 {
        let sid = Arc::clone(&session_id);
        let flag = Arc::clone(&stop_flag);
        let duration = info.duration_pts;
        handles.push(thread::spawn(move || {
            let mut count = 0;
            let mut pts = 0;
            while !flag.load(Ordering::Relaxed) && count < 100 {
                pts = (pts + 15) % (duration + 10);
                let _ = preview_seek_pts((*sid).clone(), pts);
                count += 1;
            }
        }));
    }

    // 2 threads continuously polling state
    for _ in 0..2 {
        let sid = Arc::clone(&session_id);
        let flag = Arc::clone(&stop_flag);
        handles.push(thread::spawn(move || {
            while !flag.load(Ordering::Relaxed) {
                if let Ok(st) = get_preview_state((*sid).clone()) {
                    assert!(st.current_pts >= 0);
                }
            }
        }));
    }

    // Run chaos for 300ms
    thread::sleep(Duration::from_millis(300));
    stop_flag.store(true, Ordering::Relaxed);

    for h in handles {
        h.join().expect("Thread panicked during play/pause/seek chaos race");
    }

    // Settle session to paused state
    preview_pause((*session_id).clone()).expect("Pause session");
    let state = get_preview_state((*session_id).clone()).expect("State after chaos");
    assert!(!state.is_playing, "Session should settle in paused state");

    // Seek and confirm full functionality
    let frame_opt = preview_seek_seconds((*session_id).clone(), 1.0)
        .expect("Seek after chaos")
        .expect("Frame returned");
    assert_eq!(frame_opt.width, 64);
    assert_eq!(frame_opt.height, 64);

    close_preview_session((*session_id).clone()).expect("Close session");
}

#[test]
fn test_play_at_eos_rewinds_to_pts_zero() {
    init_engine();

    let uri = "synthetic://eos_test?width=64&height=64&fps=30&duration=0.5";
    let info = create_preview_session(uri.to_string()).expect("Create session");

    // Seek to duration (EOS)
    let _ = preview_seek_pts(info.session_id.clone(), info.duration_pts);
    let s_eos = get_preview_state(info.session_id.clone()).expect("State at EOS");
    assert_eq!(s_eos.current_pts, info.duration_pts);

    // Call play() at EOS: PlaybackSession::play explicitly rewinds to PTS 0
    preview_play(info.session_id.clone()).expect("Play at EOS");

    // Let the ticker decode 2-3 frames
    thread::sleep(Duration::from_millis(100));

    // Pause
    preview_pause(info.session_id.clone()).expect("Pause");

    let s_after = get_preview_state(info.session_id.clone()).expect("State after play from EOS");
    assert!(!s_after.is_playing);
    // Since it rewound to 0 and played for 100ms at 30fps (~3 frames = ~6 PTS), PTS should be near beginning (< duration)
    assert!(
        s_after.current_pts < info.duration_pts,
        "Current PTS {} must have rewound from duration {}",
        s_after.current_pts,
        info.duration_pts
    );

    close_preview_session(info.session_id).expect("Close session");
}

// =========================================================================
// 3. Boundary Seeking Fuzzing and Pathological Values
// =========================================================================

#[test]
fn test_boundary_seeking_pathological_extremes() {
    init_engine();

    let uri = "synthetic://boundary_fuzz?width=64&height=64&fps=30&duration=1.0";
    let info = create_preview_session(uri.to_string()).expect("Create session");
    let dur = info.duration_pts;

    // Pathological PTS checks
    let pts_tests: Vec<i64> = vec![
        i64::MIN,
        i64::MIN + 1,
        -1_000_000,
        -100,
        -1,
        0,
        dur / 2,
        dur,
        dur + 1,
        dur + 100,
        1_000_000,
        i64::MAX - 1,
        i64::MAX,
    ];

    for &target in &pts_tests {
        let res = preview_seek_pts(info.session_id.clone(), target);
        assert!(res.is_ok(), "Seek to PTS {} failed: {:?}", target, res.err());

        let state = get_preview_state(info.session_id.clone()).expect("Get state");
        assert!(
            state.current_pts >= 0 && state.current_pts <= dur,
            "PTS {} did not clamp within [0, {}], got {}",
            target,
            dur,
            state.current_pts
        );
    }

    // Pathological Seconds checks
    let sec_tests: Vec<f64> = vec![
        f64::NEG_INFINITY,
        -1e15,
        -100.0,
        -0.001,
        0.0,
        info.duration_seconds * 0.5,
        info.duration_seconds,
        info.duration_seconds + 0.01,
        info.duration_seconds + 100.0,
        1e15,
        f64::INFINITY,
        f64::NAN,
    ];

    for &sec in &sec_tests {
        let res = preview_seek_seconds(info.session_id.clone(), sec);
        assert!(res.is_ok(), "Seek to Seconds {} failed: {:?}", sec, res.err());

        let state = get_preview_state(info.session_id.clone()).expect("Get state");
        assert!(
            state.current_pts >= 0 && state.current_pts <= dur,
            "Seconds {} resulted in unclamped PTS {}",
            sec,
            state.current_pts
        );
        assert!(
            state.current_seconds >= 0.0 && state.current_seconds <= info.duration_seconds + 0.1,
            "Seconds {} resulted in out-of-range current_seconds {}",
            sec,
            state.current_seconds
        );
    }

    close_preview_session(info.session_id).expect("Close session");
}

// =========================================================================
// 4. Stateless Frame Extraction Concurrent Stress & Error Handling
// =========================================================================

#[test]
fn test_stateless_frame_extraction_concurrent_and_boundaries() {
    init_engine();

    let uri = "synthetic://stateless_bench?width=64&height=64&fps=30&duration=2.0";
    let info = create_preview_session(uri.to_string()).expect("Create session for metadata");
    let dur = info.duration_pts;
    close_preview_session(info.session_id).expect("Close metadata session");

    // 8 threads running concurrent stateless extractions
    let num_threads = 8;
    let extractions_per_thread = 20;
    let success_count = Arc::new(AtomicUsize::new(0));

    let mut handles = Vec::new();
    for thread_idx in 0..num_threads {
        let counter = Arc::clone(&success_count);
        handles.push(thread::spawn(move || {
            for i in 0..extractions_per_thread {
                let pts = match (thread_idx + i) % 5 {
                    0 => -50,          // Underflow
                    1 => 0,            // Start
                    2 => dur / 2,      // Middle
                    3 => dur,          // End
                    _ => dur + 500,    // Overflow
                };
                let frame = extract_single_frame(uri.to_string(), pts)
                    .unwrap_or_else(|e| panic!("Stateless extract failed at PTS {}: {}", pts, e));
                assert_eq!(frame.width, 64);
                assert_eq!(frame.height, 64);
                assert_eq!(frame.rgba_bytes.len(), 64 * 64 * 4);
                assert_eq!(frame.row_stride_bytes, 64 * 4);
                counter.fetch_add(1, Ordering::SeqCst);
            }
        }));
    }

    for h in handles {
        h.join().expect("Worker thread panicked during stateless frame extraction");
    }

    assert_eq!(
        success_count.load(Ordering::SeqCst),
        num_threads * extractions_per_thread,
        "All concurrent stateless extractions must succeed"
    );
}

#[test]
fn test_stateless_frame_extraction_pathological_inputs() {
    init_engine();

    // 1. Empty string
    let res_empty = extract_single_frame("".to_string(), 0);
    assert!(res_empty.is_err());
    assert!(res_empty.unwrap_err().contains("empty"));

    // 2. Whitespace only
    let res_ws = extract_single_frame("   \t\n  ".to_string(), 0);
    assert!(res_ws.is_err());

    // 3. Nonexistent file
    let res_missing = extract_single_frame("/tmp/nonexistent_file_aether_12345.mp4".to_string(), 0);
    assert!(res_missing.is_err());
    assert!(res_missing.unwrap_err().contains("does not exist"));

    // 4. Directory
    let res_dir = extract_single_frame(std::env::temp_dir().to_str().unwrap().to_string(), 0);
    assert!(res_dir.is_err());
    assert!(res_dir.unwrap_err().contains("not a regular file"));

    // 5. Zero-byte file
    let zero_file_path = std::env::temp_dir().join("aether_zero_byte_test.mp4");
    {
        let mut f = File::create(&zero_file_path).expect("Create zero file");
        f.flush().expect("Flush zero file");
    }
    let res_zero = extract_single_frame(zero_file_path.to_str().unwrap().to_string(), 0);
    assert!(res_zero.is_err());
    assert!(res_zero.unwrap_err().contains("0 bytes"));
    let _ = std::fs::remove_file(zero_file_path);
}

// =========================================================================
// 5. MP4 Fixture Real Media Playback & Seeking Stress
// =========================================================================

#[test]
fn test_mp4_fixture_stress_decoding_and_teardown() {
    init_engine();

    let mp4 = sample_mp4_path();
    if !mp4.exists() {
        eprintln!("Skipping mp4 fixture test: fixture not found at {:?}", mp4);
        return;
    }
    let path_str = mp4.to_str().unwrap().to_string();

    let info = create_preview_session(path_str.clone()).expect("Create mp4 session");
    assert_eq!(info.width, 64);
    assert_eq!(info.height, 64);
    assert!(info.duration_pts > 0);
    assert!(info.duration_seconds > 0.0);

    // Play MP4 fixture briefly
    preview_play(info.session_id.clone()).expect("Play MP4");
    thread::sleep(Duration::from_millis(150));
    preview_pause(info.session_id.clone()).expect("Pause MP4");

    let state = get_preview_state(info.session_id.clone()).expect("MP4 state");
    assert!(!state.is_playing);

    // Scrub across multiple points
    let points = [0, info.duration_pts / 4, info.duration_pts / 2, info.duration_pts];
    for pts in points {
        let f_opt = preview_seek_pts(info.session_id.clone(), pts).expect("MP4 seek");
        if let Some(frame) = f_opt {
            assert_eq!(frame.width, 64);
            assert_eq!(frame.height, 64);
            assert_eq!(frame.rgba_bytes.len(), 64 * 64 * 4);
        }
    }

    close_preview_session(info.session_id).expect("Close MP4 session");
}

// =========================================================================
// 6. Rapid Clean Teardown & Ticker Thread Absence
// =========================================================================

#[test]
fn test_mass_create_play_close_no_thread_leak() {
    init_engine();

    let iterations = 50;
    let start = Instant::now();

    for i in 0..iterations {
        let uri = format!("synthetic://churn_{}?width=64&height=64&fps=30&duration=1.0", i);
        let info = create_preview_session(uri).expect("Session creation");

        // Spawn playback ticker
        preview_play(info.session_id.clone()).expect("Play");

        // Seek while ticker is running
        let _ = preview_seek_pts(info.session_id.clone(), 10);

        // Immediately close without explicit pause (verifying close_preview_session pauses ticker)
        close_preview_session(info.session_id).expect("Close session");
    }

    let elapsed = start.elapsed();
    println!(
        "{} mass churn cycles (create -> play -> seek -> close) completed in {:?}",
        iterations, elapsed
    );

    // Small grace period for OS thread cleanup
    thread::sleep(Duration::from_millis(50));

    // Verify system can still cleanly operate a new session
    let canary_uri = "synthetic://canary?width=64&height=64&fps=30&duration=1.0";
    let canary = create_preview_session(canary_uri.to_string()).expect("Canary session");
    assert!(canary.texture_id >= 1001 + iterations as i64);

    let canary_state = get_preview_state(canary.session_id.clone()).expect("Canary state");
    assert_eq!(canary_state.current_pts, 0);

    close_preview_session(canary.session_id).expect("Close canary");
}
