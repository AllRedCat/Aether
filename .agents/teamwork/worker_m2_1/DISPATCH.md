# Dispatch: worker_m2_1

**Archetype**: teamwork_preview_worker
**Working Directory**: /Users/gabrielgenaro/Developer/Pessoal/Aether/.agents/teamwork/worker_m2_1
**Milestone**: M2 (Flutter App & Riverpod Integration & UI)

## Context & Inputs
- Authoritative User Request: `/Users/gabrielgenaro/Developer/Pessoal/Aether/.agents/teamwork/ORIGINAL_REQUEST.md`
- Project Architecture & Contracts: `/Users/gabrielgenaro/Developer/Pessoal/Aether/PROJECT.md`
- Explorer Reports:
  - `/Users/gabrielgenaro/Developer/Pessoal/Aether/.agents/teamwork/explorer_m2_1/analysis.md`
  - `/Users/gabrielgenaro/Developer/Pessoal/Aether/.agents/teamwork/explorer_m2_2/analysis.md`
  - `/Users/gabrielgenaro/Developer/Pessoal/Aether/.agents/teamwork/explorer_m2_3/analysis.md`

## Exclusive Write Ownership
You have EXCLUSIVE write ownership of:
1. `apps/aether_app/analysis_options.yaml`
2. `apps/aether_app/lib/src/features/timeline/timeline_provider.dart`
3. `apps/aether_app/lib/src/features/timeline/timeline_view.dart`
4. `apps/aether_app/lib/main.dart`
5. `apps/aether_app/test/timeline_widget_test.dart` (and any related test under `apps/aether_app/test/`)

DO NOT modify files outside your write ownership.

## MANDATORY INTEGRITY WARNING
DO NOT CHEAT. All implementations must be genuine. DO NOT hardcode test results, create dummy/facade implementations, or circumvent the intended task. A teamwork_preview_auditor will independently verify your work. Integrity violations WILL be detected and your work WILL be rejected.

## Objectives
1. Create `apps/aether_app/analysis_options.yaml`:
   - Include `package:flutter_lints/flutter.yaml`.
   - Exclude analyzer for `lib/src/bridge/**`.
2. Create `apps/aether_app/lib/src/features/timeline/timeline_provider.dart`:
   - Implement `TimelineState` and `TimelineNotifier extends StateNotifier<TimelineState>`.
   - Ensure `totalClipCount`, `durationPts`, `tracks`, `timeline`, `isLoading`, `errorMessage` are present.
   - Implement `initTimeline()` and `addClip(...)`.
   - Export `final timelineProvider = StateNotifierProvider<TimelineNotifier, TimelineState>((ref) => TimelineNotifier());`.
   - Adhere strictly to the AST requirements in `tests/test_dart_ui_contract.py`.
3. Implement `apps/aether_app/lib/src/features/timeline/timeline_view.dart`:
   - `ConsumerWidget` watching `timelineProvider`.
   - Render `Key('timeline_total_clips_count')` showing `state.totalClipCount`.
   - Render `Key('add_clip_button')` invoking `notifier.addClip()`.
   - Render track list and clips demonstrably reading from the Rust timeline state.
4. Update `apps/aether_app/lib/main.dart`:
   - Ensure `WidgetsFlutterBinding.ensureInitialized()` is called.
   - Initialize Rust bridge safely (`RustLib.init()` and `initEngine()` with try-catch for headless test resilience).
   - Wrap with `ProviderScope`.
5. Create/update widget tests in `apps/aether_app/test/`:
   - Validate that `TimelineView` displays clip count from state and updates when clips are added.
6. Verify all Acceptance Criteria:
   - `cargo test -p aether_core`
   - `cargo check -p aether_bridge`
   - `make bridge`
   - `flutter analyze apps/aether_app`
   - `flutter test` (in `apps/aether_app`)
   - `python3 tests/test_dart_ui_contract.py`
   - `python3 tests/e2e_runner.py`

## Completion Criteria
- Write full implementation report and test output to `/Users/gabrielgenaro/Developer/Pessoal/Aether/.agents/teamwork/worker_m2_1/handoff.md`.
- Send completion message to parent.

## 2026-09-28T11:42:24Z
You are worker_m2_1.
Your working directory: /Users/gabrielgenaro/Developer/Pessoal/Aether/.agents/teamwork/worker_m2_1

MANDATORY INTEGRITY WARNING:
DO NOT CHEAT. All implementations must be genuine. DO NOT hardcode test results, create dummy/facade implementations, or circumvent the intended task. A teamwork_preview_auditor will independently verify your work. Integrity violations WILL be detected and your work WILL be rejected.

Read /Users/gabrielgenaro/Developer/Pessoal/Aether/.agents/teamwork/ORIGINAL_REQUEST.md before starting work. Do NOT summarize or filter it.
Read /Users/gabrielgenaro/Developer/Pessoal/Aether/PROJECT.md.
Read /Users/gabrielgenaro/Developer/Pessoal/Aether/.agents/teamwork/worker_m2_1/DISPATCH.md.
Read the findings and reference code in:
- /Users/gabrielgenaro/Developer/Pessoal/Aether/.agents/teamwork/explorer_m2_1/analysis.md
- /Users/gabrielgenaro/Developer/Pessoal/Aether/.agents/teamwork/explorer_m2_2/analysis.md
- /Users/gabrielgenaro/Developer/Pessoal/Aether/.agents/teamwork/explorer_m2_3/analysis.md

Exclusive Write Ownership:
1. `apps/aether_app/analysis_options.yaml`
2. `apps/aether_app/lib/src/features/timeline/timeline_provider.dart`
3. `apps/aether_app/lib/src/features/timeline/timeline_view.dart`
4. `apps/aether_app/lib/main.dart`
5. `apps/aether_app/test/timeline_widget_test.dart` (or related test files in `apps/aether_app/test/`)

Execute the implementation:
1. Implement `analysis_options.yaml` with standard Flutter lints and exclude bridge.
2. Implement `timeline_provider.dart` with `TimelineState` and `TimelineNotifier extends StateNotifier<TimelineState>` satisfying all contracts.
3. Implement `timeline_view.dart` watching `timelineProvider`, with `Key('timeline_total_clips_count')` and `Key('add_clip_button')` invoking `addClip()`.
4. Update `main.dart` to initialize bindings and `ProviderScope`.
5. Implement unit/widget tests in `apps/aether_app/test/`.
6. Run all verification commands:
   - `cargo test -p aether_core`
   - `cargo check -p aether_bridge`
   - `make bridge`
   - `flutter analyze apps/aether_app`
   - `flutter test apps/aether_app`
   - `python3 tests/test_dart_ui_contract.py`
   - `python3 tests/e2e_runner.py`

Write a comprehensive report to `/Users/gabrielgenaro/Developer/Pessoal/Aether/.agents/teamwork/worker_m2_1/handoff.md` and send a message when complete.

