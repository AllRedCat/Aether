## 2026-09-28T03:46:00Z
You are the Worker for Milestone M1 Iteration 2 (Native Engine & Bridge Remediation).
Your Working Directory: /Users/gabrielgenaro/Developer/Pessoal/Aether/.agents/teamwork/worker_m1_2
Original User Request: /Users/gabrielgenaro/Developer/Pessoal/Aether/.agents/teamwork/ORIGINAL_REQUEST.md
Project Scope: /Users/gabrielgenaro/Developer/Pessoal/Aether/PROJECT.md
Survey & Investigation Reports to read:
- /Users/gabrielgenaro/Developer/Pessoal/Aether/.agents/teamwork/explorer_m1_r2_1/report.md
- /Users/gabrielgenaro/Developer/Pessoal/Aether/.agents/teamwork/explorer_m1_r2_2/report.md
- /Users/gabrielgenaro/Developer/Pessoal/Aether/.agents/teamwork/explorer_m1_r2_3/report.md

MANDATORY INTEGRITY WARNING:
DO NOT CHEAT. All implementations must be genuine. DO NOT hardcode test results, create dummy/facade implementations, or circumvent the intended task. A teamwork_preview_auditor will independently verify your work. Integrity violations WILL be detected and your work WILL be rejected.

Your Exclusive Write Ownership:
- `crates/aether_core/src/timeline.rs`
- `crates/aether_bridge/Cargo.toml`
- `crates/aether_bridge/src/api.rs`
- `Makefile`

Instructions:
1. Read the reports above.
2. In `crates/aether_bridge/Cargo.toml`:
   - Set `flutter_rust_bridge = { version = "=2.3.0", features = ["uuid"] }`.
   - Ensure `uuid = { version = "1.10", features = ["v4"] }` is declared.
3. In `crates/aether_bridge/src/api.rs`:
   - Add `use flutter_rust_bridge::frb;`
   - Re-export `Track`: `pub use aether_core::timeline::{Clip, Rational, Timeline, Track, TrackKind};`
   - Add mirror structs `_Rational`, `_TrackKind`, `_Clip`, `_Track`, `_Timeline` with `#[frb(mirror(...))]`.
4. In `crates/aether_core/src/timeline.rs`:
   - In `recalculate_duration`, append `.max(0)` to clamp duration non-negative.
5. In `Makefile`:
   - Add `all: bridge`.
   - Add `--type-64bit-int` to the `bridge` recipe:
     `flutter_rust_bridge_codegen generate --type-64bit-int --rust-root crates/aether_bridge --rust-input crate::api --dart-root apps/aether_app --dart-output apps/aether_app/lib/src/bridge`
6. Execute and verify:
   - Run `make bridge`.
   - Verify `apps/aether_app/lib/src/bridge/api.dart` contains concrete classes `Timeline`, `Track`, `Clip`, `Rational`, `enum TrackKind` with standard `int` timestamp types and public getters.
   - Run `cargo test -p aether_core`.
   - Run `cargo check -p aether_bridge`.
   - Run `flutter analyze apps/aether_app`.
   - Run `python3 tests/test_rust_core.py` and `python3 tests/test_bridge_contract.py`.
7. Write full handoff report to `/Users/gabrielgenaro/Developer/Pessoal/Aether/.agents/teamwork/worker_m1_2/handoff.md`.
8. Send message to orchestrator (conversation ID fc902b32-5c5a-4c10-a43b-df36c14550c4). Maintain progress.md.
