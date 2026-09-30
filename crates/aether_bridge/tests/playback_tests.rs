use std::fs;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;
use uuid::Uuid;

use aether_bridge::api::{
    close_preview_session, create_preview_session, extract_single_frame, get_preview_state,
    init_engine, preview_pause, preview_play, preview_seek_pts, preview_seek_seconds,
};

/// Helper to create a unique temporary directory in std::env::temp_dir()
struct TestDir {
    path: PathBuf,
}

impl TestDir {
    fn new(suffix: &str) -> Self {
        let dir_name = format!("aether_playback_test_{}_{}", suffix, Uuid::new_v4());
        let path = std::env::temp_dir().join(dir_name);
        fs::create_dir_all(&path).expect("Failed to create test directory");
        Self { path }
    }

    fn file(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// Helper resolving sample mp4 test fixture path from aether_media
fn sample_mp4_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../aether_media/tests/fixtures/sample_64x64_10frames.mp4")
}

#[test]
fn test_init_engine() {
    init_engine();
}

// =========================================================================
// 1. Session Creation Tests
// =========================================================================

#[test]
fn test_create_preview_session_synthetic() {
    let uri = "synthetic://preview_test?width=128&height=72&fps=30&duration=2.0";
    let info = create_preview_session(uri.to_string()).expect("Failed to create synthetic session");

    assert!(!info.session_id.is_empty(), "Session ID must not be empty");
    assert!(Uuid::parse_str(&info.session_id).is_ok(), "Session ID must be a valid UUID");
    assert!(info.texture_id >= 1000, "Texture ID must be allocated >= 1000");
    assert_eq!(info.width, 128);
    assert_eq!(info.height, 72);
    assert_eq!(info.fps, 30.0);
    assert!((info.duration_seconds - 2.0).abs() < 0.05);
    assert!(info.duration_pts > 0);

    // Initial state check
    let state = get_preview_state(info.session_id.clone()).expect("Failed to get initial state");
    assert_eq!(state.session_id, info.session_id);
    assert!(!state.is_playing, "Session should not start playing automatically");
    assert_eq!(state.current_pts, 0);
    assert_eq!(state.current_seconds, 0.0);
    assert_eq!(state.duration_pts, info.duration_pts);
    assert!((state.duration_seconds - info.duration_seconds).abs() < 0.05);

    // Clean up
    close_preview_session(info.session_id).expect("Close session should succeed");
}

#[test]
fn test_create_preview_session_mp4_fixture() {
    let mp4 = sample_mp4_path();
    if !mp4.exists() {
        eprintln!("Skipping mp4 fixture test; file not found at {:?}", mp4);
        return;
    }

    let path_str = mp4.to_str().expect("Valid UTF-8 path").to_string();
    let info = create_preview_session(path_str).expect("Failed to create preview session for mp4");

    assert!(!info.session_id.is_empty());
    assert!(info.texture_id >= 1000);
    assert_eq!(info.width, 64);
    assert_eq!(info.height, 64);
    assert!(info.duration_seconds > 0.0);
    assert!(info.duration_pts > 0);

    let state = get_preview_state(info.session_id.clone()).expect("Get state");
    assert!(!state.is_playing);
    assert_eq!(state.current_pts, 0);

    close_preview_session(info.session_id).expect("Close session");
}

#[test]
fn test_create_preview_session_invalid_inputs() {
    // 1. Empty string
    let res = create_preview_session("".to_string());
    assert!(res.is_err());
    assert_eq!(res.unwrap_err(), "File path cannot be empty");

    // 2. Whitespace string
    let res = create_preview_session("   \t\n  ".to_string());
    assert!(res.is_err());
    assert_eq!(res.unwrap_err(), "File path cannot be empty");

    // 3. Nonexistent file
    let res = create_preview_session("/nonexistent/path/media_file_12345.mp4".to_string());
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("File does not exist"));

    // 4. Directory path
    let test_dir = TestDir::new("dir_check");
    let res = create_preview_session(test_dir.path.to_str().unwrap().to_string());
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("Path is not a regular file"));

    // 5. Zero-byte file
    let zero_file = test_dir.file("zero_byte.mp4");
    fs::write(&zero_file, b"").expect("Failed to write zero file");
    let res = create_preview_session(zero_file.to_str().unwrap().to_string());
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("File is empty (0 bytes)"));
}

// =========================================================================
// 2. Seeking Tests (preview_seek_pts, preview_seek_seconds)
// =========================================================================

#[test]
fn test_preview_seek_seconds_and_pts() {
    let uri = "synthetic://seek_test?width=64&height=64&fps=30&duration=3.0";
    let info = create_preview_session(uri.to_string()).expect("Create session");

    // Seek to 1.0 second
    let frame_opt = preview_seek_seconds(info.session_id.clone(), 1.0)
        .expect("Seek seconds should succeed");
    assert!(frame_opt.is_some(), "Seek should return a frame");
    let frame = frame_opt.unwrap();
    assert_eq!(frame.width, 64);
    assert_eq!(frame.height, 64);
    assert_eq!(frame.rgba_bytes.len(), 64 * 64 * 4);
    assert_eq!(frame.row_stride_bytes, 64 * 4);
    assert!(
        (frame.timestamp_seconds - 1.0).abs() < 0.1,
        "Expected frame near 1.0s, got: {}",
        frame.timestamp_seconds
    );

    // Verify state reflects seek
    let state = get_preview_state(info.session_id.clone()).expect("Get state");
    assert!((state.current_seconds - 1.0).abs() < 0.1);

    // Seek to PTS corresponding to ~2.0 seconds (e.g. 60 frames in 30fps or 120 pts in 60fps)
    let target_pts = (2.0 * info.fps).round() as i64;
    let frame_opt2 = preview_seek_pts(info.session_id.clone(), target_pts)
        .expect("Seek PTS should succeed");
    assert!(frame_opt2.is_some());
    let frame2 = frame_opt2.unwrap();
    assert_eq!(frame2.width, 64);
    assert_eq!(frame2.height, 64);
    assert_eq!(frame2.rgba_bytes.len(), 64 * 64 * 4);

    let state2 = get_preview_state(info.session_id.clone()).expect("Get state");
    assert!(state2.current_pts >= target_pts - 1);

    close_preview_session(info.session_id).expect("Close session");
}

#[test]
fn test_preview_seek_clamping_bounds() {
    let uri = "synthetic://bounds_test?width=64&height=64&fps=30&duration=2.0";
    let info = create_preview_session(uri.to_string()).expect("Create session");

    // 1. Negative PTS seek should clamp to 0
    let f_neg = preview_seek_pts(info.session_id.clone(), -1000)
        .expect("Negative seek should not fail");
    assert!(f_neg.is_some());
    let frame_neg = f_neg.unwrap();
    assert_eq!(frame_neg.pts, 0);
    assert_eq!(frame_neg.timestamp_seconds, 0.0);

    let state_neg = get_preview_state(info.session_id.clone()).expect("Get state");
    assert_eq!(state_neg.current_pts, 0);

    // 2. Negative seconds seek should clamp to 0.0
    let f_neg_sec = preview_seek_seconds(info.session_id.clone(), -5.0)
        .expect("Negative seconds seek should not fail");
    assert!(f_neg_sec.is_some());
    let frame_neg_sec = f_neg_sec.unwrap();
    assert_eq!(frame_neg_sec.pts, 0);

    // 3. Excessive PTS seek should clamp to duration_pts
    let _ = preview_seek_pts(info.session_id.clone(), 999999)
        .expect("Overshoot seek should clamp safely");
    let state_over = get_preview_state(info.session_id.clone()).expect("Get state");
    assert_eq!(state_over.current_pts, info.duration_pts);

    // 4. Excessive seconds seek should clamp to duration_seconds
    let _ = preview_seek_seconds(info.session_id.clone(), 9999.0)
        .expect("Overshoot seconds should clamp safely");
    let state_over_sec = get_preview_state(info.session_id.clone()).expect("Get state");
    assert_eq!(state_over_sec.current_pts, info.duration_pts);

    close_preview_session(info.session_id).expect("Close session");
}

// =========================================================================
// 3. Playback Controls (preview_play, preview_pause, get_preview_state)
// =========================================================================

#[test]
fn test_preview_play_pause_and_state() {
    let uri = "synthetic://playback_test?width=64&height=64&fps=30&duration=3.0";
    let info = create_preview_session(uri.to_string()).expect("Create session");

    // Initial state is paused
    let s0 = get_preview_state(info.session_id.clone()).expect("Get state");
    assert!(!s0.is_playing);
    assert_eq!(s0.current_pts, 0);

    // Start playback
    preview_play(info.session_id.clone()).expect("preview_play should succeed");
    let s_play = get_preview_state(info.session_id.clone()).expect("Get state");
    assert!(s_play.is_playing);

    // Wait briefly for background ticker thread to advance frames
    thread::sleep(Duration::from_millis(150));

    let s_adv = get_preview_state(info.session_id.clone()).expect("Get state");
    assert!(
        s_adv.current_pts > 0,
        "Playback should have advanced PTS beyond 0, got: {}",
        s_adv.current_pts
    );

    // Pause playback
    preview_pause(info.session_id.clone()).expect("preview_pause should succeed");
    let s_paused = get_preview_state(info.session_id.clone()).expect("Get state");
    assert!(!s_paused.is_playing);
    let paused_pts = s_paused.current_pts;

    // Wait to ensure ticker is stopped and PTS does not change
    thread::sleep(Duration::from_millis(100));
    let s_still_paused = get_preview_state(info.session_id.clone()).expect("Get state");
    assert!(!s_still_paused.is_playing);
    assert_eq!(
        s_still_paused.current_pts, paused_pts,
        "PTS should not advance while paused"
    );

    // Resume playback
    preview_play(info.session_id.clone()).expect("Resume play should succeed");
    let s_resumed = get_preview_state(info.session_id.clone()).expect("Get state");
    assert!(s_resumed.is_playing);

    // Pause again and clean up
    preview_pause(info.session_id.clone()).expect("Pause");
    close_preview_session(info.session_id).expect("Close");
}

#[test]
fn test_preview_play_pause_idempotency() {
    let uri = "synthetic://idempotent?width=64&height=64&fps=30&duration=1.0";
    let info = create_preview_session(uri.to_string()).expect("Create session");

    // Calling play twice in a row
    preview_play(info.session_id.clone()).expect("Play 1");
    preview_play(info.session_id.clone()).expect("Play 2 (idempotent)");
    let state = get_preview_state(info.session_id.clone()).expect("Get state");
    assert!(state.is_playing);

    // Calling pause twice in a row
    preview_pause(info.session_id.clone()).expect("Pause 1");
    preview_pause(info.session_id.clone()).expect("Pause 2 (idempotent)");
    let state2 = get_preview_state(info.session_id.clone()).expect("Get state");
    assert!(!state2.is_playing);

    close_preview_session(info.session_id).expect("Close");
}

#[test]
fn test_preview_playback_eos_and_restart() {
    // Very short duration: 0.15s @ 30fps = ~4-5 frames
    let uri = "synthetic://eos_test?width=64&height=64&fps=30&duration=0.15";
    let info = create_preview_session(uri.to_string()).expect("Create session");

    preview_play(info.session_id.clone()).expect("Play");

    // Poll until playback reaches EOS and stops automatically
    let mut stopped = false;
    for _ in 0..40 {
        thread::sleep(Duration::from_millis(25));
        let state = get_preview_state(info.session_id.clone()).expect("Get state");
        if !state.is_playing && state.current_pts >= info.duration_pts {
            stopped = true;
            break;
        }
    }
    assert!(stopped, "Playback should automatically stop upon reaching EOS");

    // Calling play again at EOS should restart from PTS 0
    preview_play(info.session_id.clone()).expect("Restart play from EOS");
    let state_restart = get_preview_state(info.session_id.clone()).expect("Get state");
    assert!(state_restart.is_playing);

    preview_pause(info.session_id.clone()).expect("Pause");
    close_preview_session(info.session_id).expect("Close");
}

// =========================================================================
// 4. Stateless Frame Extraction (extract_single_frame)
// =========================================================================

#[test]
fn test_extract_single_frame_synthetic() {
    let uri = "synthetic://extract_test?width=80&height=60&fps=30&duration=2.0";

    // Extract at PTS 0
    let f0 = extract_single_frame(uri.to_string(), 0).expect("Extract at PTS 0");
    assert_eq!(f0.width, 80);
    assert_eq!(f0.height, 60);
    assert_eq!(f0.rgba_bytes.len(), 80 * 60 * 4);
    assert_eq!(f0.row_stride_bytes, 80 * 4);
    assert_eq!(f0.pts, 0);

    // Extract at PTS 30 (~1.0s)
    let f1 = extract_single_frame(uri.to_string(), 30).expect("Extract at PTS 30");
    assert_eq!(f1.width, 80);
    assert_eq!(f1.height, 60);
    assert_eq!(f1.rgba_bytes.len(), 80 * 60 * 4);
    assert!(f1.timestamp_seconds > 0.0);

    // Negative PTS should clamp to 0
    let f_neg = extract_single_frame(uri.to_string(), -500).expect("Extract negative PTS");
    assert_eq!(f_neg.pts, 0);

    // Excessive PTS should clamp to duration_pts
    let f_over = extract_single_frame(uri.to_string(), 999999).expect("Extract overshoot PTS");
    assert_eq!(f_over.width, 80);
    assert_eq!(f_over.height, 60);
}

#[test]
fn test_extract_single_frame_mp4_fixture() {
    let mp4 = sample_mp4_path();
    if !mp4.exists() {
        eprintln!("Skipping mp4 extract test; fixture not found");
        return;
    }

    let path_str = mp4.to_str().unwrap().to_string();
    let frame = extract_single_frame(path_str, 0).expect("Extract frame from mp4 fixture");
    assert_eq!(frame.width, 64);
    assert_eq!(frame.height, 64);
    assert_eq!(frame.rgba_bytes.len(), 64 * 64 * 4);
    assert_eq!(frame.row_stride_bytes, 64 * 4);
}

#[test]
fn test_extract_single_frame_errors() {
    // 1. Empty string
    let res = extract_single_frame("".to_string(), 0);
    assert!(res.is_err());
    assert_eq!(res.unwrap_err(), "File path cannot be empty");

    // 2. Whitespace string
    let res = extract_single_frame("   \n ".to_string(), 0);
    assert!(res.is_err());
    assert_eq!(res.unwrap_err(), "File path cannot be empty");

    // 3. Nonexistent file
    let res = extract_single_frame("/nonexistent/video_extract_123.mp4".to_string(), 0);
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("File does not exist"));

    // 4. Directory path
    let test_dir = TestDir::new("extract_dir");
    let res = extract_single_frame(test_dir.path.to_str().unwrap().to_string(), 0);
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("Path is not a regular file"));

    // 5. Zero-byte file
    let zero_file = test_dir.file("zero_extract.mp4");
    fs::write(&zero_file, b"").unwrap();
    let res = extract_single_frame(zero_file.to_str().unwrap().to_string(), 0);
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("File is empty (0 bytes)"));
}

// =========================================================================
// 5. Session Lifecycle & Teardown (close_preview_session)
// =========================================================================

#[test]
fn test_close_preview_session_lifecycle() {
    let uri = "synthetic://lifecycle?width=64&height=64&fps=30&duration=1.0";
    let info = create_preview_session(uri.to_string()).expect("Create session");

    // Verify session is active
    let state = get_preview_state(info.session_id.clone()).expect("Get state");
    assert_eq!(state.session_id, info.session_id);

    // Close session
    close_preview_session(info.session_id.clone()).expect("Close session should succeed");

    // Subsequent calls with the closed session_id must fail
    let err_msg = format!("Session with ID '{}' not found", info.session_id);

    let res_state = get_preview_state(info.session_id.clone());
    assert_eq!(res_state.unwrap_err(), err_msg);

    let res_play = preview_play(info.session_id.clone());
    assert_eq!(res_play.unwrap_err(), err_msg);

    let res_pause = preview_pause(info.session_id.clone());
    assert_eq!(res_pause.unwrap_err(), err_msg);

    let res_seek_pts = preview_seek_pts(info.session_id.clone(), 10);
    assert_eq!(res_seek_pts.unwrap_err(), err_msg);

    let res_seek_sec = preview_seek_seconds(info.session_id.clone(), 0.5);
    assert_eq!(res_seek_sec.unwrap_err(), err_msg);

    let res_close_again = close_preview_session(info.session_id.clone());
    assert_eq!(res_close_again.unwrap_err(), err_msg);
}

#[test]
fn test_multiple_concurrent_sessions_isolation() {
    let uri1 = "synthetic://s1?width=64&height=64&fps=30&duration=2.0";
    let uri2 = "synthetic://s2?width=128&height=72&fps=24&duration=3.0";
    let uri3 = "synthetic://s3?width=80&height=60&fps=60&duration=1.0";

    let info1 = create_preview_session(uri1.to_string()).expect("Create s1");
    let info2 = create_preview_session(uri2.to_string()).expect("Create s2");
    let info3 = create_preview_session(uri3.to_string()).expect("Create s3");

    // All session IDs must be unique
    assert_ne!(info1.session_id, info2.session_id);
    assert_ne!(info2.session_id, info3.session_id);
    assert_ne!(info1.session_id, info3.session_id);

    // All texture IDs must be strictly unique
    assert_ne!(info1.texture_id, info2.texture_id);
    assert_ne!(info2.texture_id, info3.texture_id);
    assert_ne!(info1.texture_id, info3.texture_id);

    // Seek each session to different positions independently
    let f1 = preview_seek_seconds(info1.session_id.clone(), 1.0).unwrap().unwrap();
    let f2 = preview_seek_seconds(info2.session_id.clone(), 2.0).unwrap().unwrap();
    let f3 = preview_seek_seconds(info3.session_id.clone(), 0.5).unwrap().unwrap();

    assert_eq!(f1.width, 64);
    assert_eq!(f2.width, 128);
    assert_eq!(f3.width, 80);

    let s1 = get_preview_state(info1.session_id.clone()).unwrap();
    let s2 = get_preview_state(info2.session_id.clone()).unwrap();
    let s3 = get_preview_state(info3.session_id.clone()).unwrap();

    assert!((s1.current_seconds - 1.0).abs() < 0.1);
    assert!((s2.current_seconds - 2.0).abs() < 0.1);
    assert!((s3.current_seconds - 0.5).abs() < 0.1);

    // Close in arbitrary order
    close_preview_session(info2.session_id.clone()).unwrap();
    // s1 and s3 remain valid
    assert!(get_preview_state(info1.session_id.clone()).is_ok());
    assert!(get_preview_state(info3.session_id.clone()).is_ok());
    // s2 is closed
    assert!(get_preview_state(info2.session_id.clone()).is_err());

    close_preview_session(info1.session_id).unwrap();
    close_preview_session(info3.session_id).unwrap();
}

#[test]
fn test_nonexistent_session_operations() {
    let dummy_id = Uuid::new_v4().to_string();
    let expected_err = format!("Session with ID '{}' not found", dummy_id);

    assert_eq!(get_preview_state(dummy_id.clone()).unwrap_err(), expected_err);
    assert_eq!(preview_play(dummy_id.clone()).unwrap_err(), expected_err);
    assert_eq!(preview_pause(dummy_id.clone()).unwrap_err(), expected_err);
    assert_eq!(preview_seek_pts(dummy_id.clone(), 0).unwrap_err(), expected_err);
    assert_eq!(preview_seek_seconds(dummy_id.clone(), 0.0).unwrap_err(), expected_err);
    assert_eq!(close_preview_session(dummy_id.clone()).unwrap_err(), expected_err);
}
