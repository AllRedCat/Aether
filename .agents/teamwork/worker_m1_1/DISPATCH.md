## 2026-09-28T03:17:20Z

You are the Worker for Milestone M1 (Native Engine & Bridge).
Your Working Directory: /Users/gabrielgenaro/Developer/Pessoal/Aether/.agents/teamwork/worker_m1_1
Original User Request: /Users/gabrielgenaro/Developer/Pessoal/Aether/.agents/teamwork/ORIGINAL_REQUEST.md
Project Scope: /Users/gabrielgenaro/Developer/Pessoal/Aether/PROJECT.md
Survey Reports to read:
- /Users/gabrielgenaro/Developer/Pessoal/Aether/.agents/teamwork/survey_explorer_rust/report.md
- /Users/gabrielgenaro/Developer/Pessoal/Aether/.agents/teamwork/survey_spec_miner_bridge/report.md

MANDATORY INTEGRITY WARNING:
DO NOT CHEAT. All implementations must be genuine. DO NOT hardcode test results, create dummy/facade implementations, or circumvent the intended task. A teamwork_preview_auditor will independently verify your work. Integrity violations WILL be detected and your work WILL be rejected.

Your Exclusive Write Ownership:
- `crates/aether_core/src/timeline.rs`
- `crates/aether_core/src/lib.rs`
- `crates/aether_bridge/Cargo.toml`
- `crates/aether_bridge/src/api.rs`
- `crates/aether_bridge/src/lib.rs`
- `Makefile`

Instructions:
1. Read ORIGINAL_REQUEST.md, PROJECT.md, and the two survey reports.
2. In `crates/aether_core/src/timeline.rs`:
   - Derive `PartialEq, Eq` for all structs/enums. Derive `Copy` for `Rational` and `TrackKind`.
   - Implement `TimelineError` (`TrackNotFound`, `InvalidClipBounds`, `InvalidSourceBounds`) with `Display` and `std::error::Error`.
   - Implement methods on `Clip` (`new`, `with_id`, `duration`).
   - Implement methods on `Track` (`new`, `with_id`, `duration_pts`, `add_clip`).
   - Implement methods on `Timeline` (`new`, `new_with_default_tracks`, `add_track`, `recalculate_duration`, `add_clip`, `total_clip_count`).
   - Ensure `recalculate_duration` computes `max(clip.timeline_out)` across all tracks (0 if empty).
   - Implement unit tests covering `add_clip`, `recalculate_duration` (single and multiple tracks/clips), invalid bounds, and non-existent track ID.
3. In `crates/aether_bridge/Cargo.toml`:
   - Add `uuid = { version = "1.10", features = ["v4"] }`.
4. In `crates/aether_bridge/src/api.rs`:
   - Implement `init_engine()`.
   - Implement `create_timeline() -> Timeline` (with default video track).
   - Implement `add_track(timeline: Timeline, kind: TrackKind) -> Timeline`.
   - Implement `add_clip_to_track(timeline: Timeline, track_id: Uuid, source_id: Uuid, source_in: i64, source_out: i64, timeline_in: i64) -> Result<Timeline, String>`.
5. Tooling & Verification:
   - Check where `cargo` is installed (e.g. check `~/.cargo/bin/cargo`, `/opt/homebrew/bin/cargo`, etc.).
   - Execute verification: `cargo test -p aether_core`, `cargo check -p aether_bridge`, and test `make bridge`.
   - Document commands, exit codes, and outputs in your report.
6. Write full handoff report to `/Users/gabrielgenaro/Developer/Pessoal/Aether/.agents/teamwork/worker_m1_1/handoff.md`.
7. Send message to orchestrator (conversation ID fc902b32-5c5a-4c10-a43b-df36c14550c4). Maintain progress.md in your directory.
