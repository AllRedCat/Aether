# Execution Plan — Milestone M2 (Flutter App & Riverpod Integration & UI)

## Objectives
1. State Management in `apps/aether_app`: Riverpod Notifier/Provider consuming Rust Timeline state via FFI bridge.
2. Flutter UI: `TimelineView` displaying track/clip list, total clip count, duration PTS, and an "Add Clip" button.
3. Verification:
   - `cargo test -p aether_core` passing with clip addition & duration recalculation tests.
   - `cargo check -p aether_bridge` compiling cleanly.
   - `make bridge` generating code cleanly without errors.
   - `flutter analyze` passing with 0 errors/warnings.
   - Automated widget / integration tests verifying UI displays clip count accurately from Rust Timeline state.
4. Gate Evaluation: 2 Reviewers, 2 Challengers, 1 Forensic Auditor.
5. Final Reporting to Sentinel.

## Step-by-Step Milestones & Workflow
- [x] **Step 1: Orchestrator Initialization & Setup**
  - Setup DISPATCH.md, BRIEFING.md, plan.md, progress.md.
  - Launch recurring heartbeat cron (`*/10 * * * *`).

- [ ] **Step 2: Milestone M2 Exploration (3 Parallel Explorers)**
  - `explorer_m2_1`: Investigate Flutter Riverpod architecture, dependencies (`pubspec.yaml`), state management design for `TimelineNotifier` and `TimelineState`.
  - `explorer_m2_2`: Investigate FFI Bridge Dart API (`api.dart`, initialization lifecycle, asynchronous `createTimeline()`, `addClipToTrack()`, parameter conversions).
  - `explorer_m2_3`: Investigate UI layout in `timeline_view.dart`, keys (`add_clip_button`, `timeline_total_clips_count`), and widget testing setup (`flutter test`).

- [ ] **Step 3: Implementation Dispatch (Worker)**
  - Synthesize findings from Explorers 1, 2, and 3.
  - Dispatch Worker (`teamwork_preview_worker`) with exclusive write ownership:
    - `apps/aether_app/pubspec.yaml`
    - `apps/aether_app/analysis_options.yaml`
    - `apps/aether_app/lib/main.dart`
    - `apps/aether_app/lib/src/features/timeline/timeline_provider.dart`
    - `apps/aether_app/lib/src/features/timeline/timeline_view.dart`
    - `apps/aether_app/test/timeline_view_test.dart`
  - Worker verifies:
    - `cargo test -p aether_core`
    - `cargo check -p aether_bridge`
    - `make bridge`
    - `flutter analyze apps/aether_app`
    - `flutter test`

- [ ] **Step 4: Gate Evaluation Dispatch**
  - 2 Reviewers (`teamwork_preview_reviewer`): Code quality, riverpod best practices, widget tree structure, error handling, strict static analysis.
  - 2 Challengers (`teamwork_preview_challenger`): Stress tests, mock/real bridge interaction, rapid button clicking, state mutation tests.
  - 1 Forensic Auditor (`teamwork_preview_auditor`): Authentic FFI calls, no hardcoded UI counts, genuine state propagation.

- [ ] **Step 5: Gate Status Evaluation & Verification**
  - Synthesize reports into `GATE_STATUS.md`.
  - Pass criteria: Clean audit (binary veto), both Reviewers APPROVE, Challengers confirm correctness, tests pass.

- [ ] **Step 6: Final Milestone & Acceptance Verification**
  - Run full acceptance verification suite (`e2e_runner.py`).

- [ ] **Step 7: Final Completion Report to Sentinel**
  - Synthesize all evidence and hand off to parent sentinel.
