# Aether Full-Stack Slice — Test Infrastructure Specification (TEST_INFRA.md)

## 1. Overview & 4-Tier Test Architecture

This document defines the formal test infrastructure, validation methodologies, and automated verification protocols for the **Aether Full-stack Slice** project.
Aether links a high-performance native core in Rust (`aether_core`), an FFI serialization bridge (`aether_bridge`), and a reactive Dart UI in Flutter (`aether_app` via Riverpod).

Testing follows a strict **4-Tier Methodology**:
- **Tier 1: Feature Coverage** (>= 5 test cases per feature across all 19 features in `PROJECT.md`): Validates primary happy paths, interface contracts, return types, and expected values against authoritative requirements.
- **Tier 2: Boundary & Corner Cases** (>= 5 test cases per feature across all 19 features): Validates system behavior at extremums, zero values, negative timestamps, missing UUIDs, empty collections, rapid repeated calls, and error propagation.
- **Tier 3: Cross-Feature Combinations**: Validates pairwise and multi-module interactions across the Rust Core, Bridge FFI, Riverpod state container, and Flutter Widget tree.
- **Tier 4: Real-World Application Scenarios**: Validates end-to-end user and system workflows simulating realistic editing operations and automated pipeline validation.

---

## 2. Authoritative Specification Sources

Expected outputs and assertions are derived directly from:
1. `ORIGINAL_REQUEST.md`: Acceptance criteria and requirements R1 (Rust native engine & PTS duration recalculation) and R2 (Flutter Riverpod state & UI clip count display).
2. `PROJECT.md`: Architectural specification, feature inventory (Features 1-19), interface contracts, and file boundaries.
3. Mathematical PTS & EDL Timebase properties:
   - Presentation Time Stamps (PTS): `duration_pts = max(clip.timeline_out)` across all tracks.
   - For any clip $c$, $c.\text{timeline\_out} = c.\text{timeline\_in} + (c.\text{source\_out} - c.\text{source\_in})$.
   - Invariant: $c.\text{source\_out} > c.\text{source\_in} \ge 0$, $c.\text{timeline\_in} \ge 0$.

---

## 3. Tier 1: Feature Coverage (>= 5 Test Cases per Feature)

### Group A: Rust Native Core (`aether_core`)

#### Feature 1: Domain Models & Trait Derivations
- **TC-T1-F01-01 (Timeline Struct Derivation)**: Verify `Timeline` implements `Debug`, `Clone`, and `PartialEq`.
  - *Input*: `Timeline { id, timebase: Rational { num: 60, den: 1 }, duration_pts: 0, tracks: vec![] }`.
  - *Expected*: `format!("{:?}", tl)` succeeds; `tl.clone() == tl` evaluates to `true`.
- **TC-T1-F01-02 (Track Struct Derivation)**: Verify `Track` contains `id: Uuid`, `kind: TrackKind`, `clips: Vec<Clip>` and supports `Clone` and `PartialEq`.
  - *Input*: Instantiate `Track` with `TrackKind::Video`.
  - *Expected*: Struct members are accessible and clone equality holds.
- **TC-T1-F01-03 (Clip Struct Fields)**: Verify `Clip` contains `id, source_id, source_in, source_out, timeline_in, timeline_out`.
  - *Input*: Instantiate `Clip` with explicit values.
  - *Expected*: All 6 fields are readable and equal to assigned inputs.
- **TC-T1-F01-04 (Rational Timebase Representation)**: Verify `Rational` handles standard NLE framerates (60/1, 30/1, 24/1, 24000/1001).
  - *Input*: `Rational { num: 24, den: 1 }`.
  - *Expected*: Struct retains exact numerator and denominator without floating-point degradation.
- **TC-T1-F01-05 (TrackKind Enum Variants)**: Verify `TrackKind` supports `Video`, `Audio`, and `Overlay`.
  - *Input*: Match across all 3 variants.
  - *Expected*: All variants construct cleanly and pattern matching is exhaustive.

#### Feature 2: Timeline Error Hierarchy
- **TC-T1-F02-01 (TrackNotFound Error)**: Trigger error when inserting a clip into a non-existent `track_id`.
  - *Input*: `timeline.add_clip(Uuid::nil(), clip)`.
  - *Expected*: Returns `Err(TimelineError::TrackNotFound(Uuid::nil()))`.
- **TC-T1-F02-02 (InvalidClipBounds Error on Inverted Source)**: Trigger error when `source_out <= source_in`.
  - *Input*: `Clip::new(source_id, 100, 50, 0)`.
  - *Expected*: Returns `Err(TimelineError::InvalidClipBounds)` or constructor validation error.
- **TC-T1-F02-03 (InvalidClipBounds on Negative Source In)**: Validate negative PTS protection.
  - *Input*: `Clip::new(source_id, -10, 50, 0)`.
  - *Expected*: Fails with `TimelineError::InvalidClipBounds`.
- **TC-T1-F02-04 (Error Trait Implementation)**: Verify `TimelineError` implements `std::fmt::Display` and `std::error::Error`.
  - *Input*: Format error using `{}` format specifier.
  - *Expected*: User-readable error message containing contextual details.
- **TC-T1-F02-05 (Error Serialization across FFI)**: Verify `TimelineError` maps to a clear string description for FFI `Result<Timeline, String>`.
  - *Input*: `format!("{}", TimelineError::TrackNotFound(id))`.
  - *Expected*: Non-empty descriptive string containing track UUID.

#### Feature 3: Clip Creation & Bounds
- **TC-T1-F03-01 (Standard Clip Instantiation)**: Test `Clip::new` with standard positive bounds.
  - *Input*: `Clip::new(source_id, 0, 120, 0)`.
  - *Expected*: `timeline_out == 120`, `duration == 120`.
- **TC-T1-F03-02 (Clip Offset In Timeline)**: Test clip placed at non-zero `timeline_in`.
  - *Input*: `Clip::new(source_id, 10, 70, 300)`.
  - *Expected*: `source duration = 60`, `timeline_out = 300 + 60 = 360`.
- **TC-T1-F03-03 (Unique Clip UUID)**: Test that distinct `Clip::new` invocations generate unique clip IDs.
  - *Input*: Create `clip1` and `clip2`.
  - *Expected*: `clip1.id != clip2.id`.
- **TC-T1-F03-04 (Preservation of Source ID)**: Test that `source_id` is faithfully preserved.
  - *Input*: Given `sid = Uuid::new_v4()`, create clip.
  - *Expected*: `clip.source_id == sid`.
- **TC-T1-F03-05 (Sub-clip Source Slicing)**: Test sub-clip extracted from middle of media asset.
  - *Input*: `source_in = 500`, `source_out = 800`, `timeline_in = 0`.
  - *Expected*: `timeline_out == 300`.

#### Feature 4: Track Clip Insertion
- **TC-T1-F04-01 (Append Clip to Empty Track)**: Insert single clip into fresh track.
  - *Input*: `track.add_clip(clip)`.
  - *Expected*: `track.clips.len() == 1`, `track.clips[0] == clip`.
- **TC-T1-F04-02 (Sequential Clip Appends)**: Insert two non-overlapping clips in succession.
  - *Input*: Clip A (0..60), Clip B (60..120).
  - *Expected*: `track.clips.len() == 2`.
- **TC-T1-F04-03 (Track ID Preservation)**: Verify track ID remains unchanged after clip insertion.
  - *Input*: Check `track.id` before and after `add_clip`.
  - *Expected*: `before.id == after.id`.
- **TC-T1-F04-04 (Track Kind Preservation)**: Verify `track.kind` remains `TrackKind::Video`.
  - *Input*: Check kind after insertion.
  - *Expected*: Kind unchanged.
- **TC-T1-F04-05 (Clip Retrieval by Index)**: Verify clip data retrieved matches inserted clip.
  - *Input*: Inspect `track.clips[0]`.
  - *Expected*: All fields equal to original clip.

#### Feature 5: Timeline PTS Duration Recalculation
- **TC-T1-F05-01 (Empty Timeline Duration)**: Verify duration of empty timeline.
  - *Input*: Timeline with no clips.
  - *Expected*: `duration_pts == 0`.
- **TC-T1-F05-02 (Single Clip Recalculation)**: Add clip ending at PTS 180.
  - *Input*: `timeline_out == 180`.
  - *Expected*: `timeline.duration_pts == 180`.
- **TC-T1-F05-03 (Multiple Sequential Clips on Single Track)**: Add clip ending at 100, then clip ending at 250.
  - *Input*: Clips ending at 100 and 250.
  - *Expected*: `duration_pts == 250`.
- **TC-T1-F05-04 (Multi-Track Maximum PTS Calculation)**: Track 1 ends at PTS 300, Track 2 ends at PTS 450.
  - *Input*: Two tracks with different end PTS.
  - *Expected*: `duration_pts == 450` (`max(300, 450)`).
- **TC-T1-F05-05 (Non-Monotonic Clip Insertion)**: Insert clip ending at 500, then insert clip ending at 200.
  - *Input*: Shorter clip inserted after longer clip.
  - *Expected*: `duration_pts == 500`.

#### Feature 6: Timeline Clip Insertion API
- **TC-T1-F06-01 (Public API Success Path)**: Call `timeline.add_clip(track_id, clip)`.
  - *Input*: Valid track ID and valid clip.
  - *Expected*: Returns `Ok(())`, clip is added, duration is recalculated.
- **TC-T1-F06-02 (Track Not Found Guard)**: Call `timeline.add_clip` with random UUID.
  - *Input*: `track_id = Uuid::new_v4()` not in timeline.
  - *Expected*: Returns `Err(TimelineError::TrackNotFound(_))`.
- **TC-T1-F06-03 (Total Clip Count Increment)**: Track clip count before and after `add_clip`.
  - *Input*: Timeline with 0 clips, invoke `add_clip`.
  - *Expected*: Sum of clips across all tracks increments from 0 to 1.
- **TC-T1-F06-04 (Duration Updated in Place)**: Check `duration_pts` immediately after `add_clip`.
  - *Input*: Clip with `timeline_out = 240`.
  - *Expected*: `timeline.duration_pts == 240`.
- **TC-T1-F06-05 (Audio Track Clip Insertion)**: Call `timeline.add_clip` on an Audio track.
  - *Input*: `track_id` corresponding to `TrackKind::Audio`.
  - *Expected*: Returns `Ok(())`, clip added to audio track.

#### Feature 7: Core Automated Unit Tests
- **TC-T1-F07-01 (Cargo Test Execution)**: Execute `cargo test -p aether_core`.
  - *Input*: Workspace test runner invocation.
  - *Expected*: Exit code 0, test runner executes without panics.
- **TC-T1-F07-02 (Clip Addition Test Presence)**: Verify unit test specifically asserts clip addition.
  - *Input*: Cargo test output stream.
  - *Expected*: Test containing `clip` or `add_clip` reported as `ok`.
- **TC-T1-F07-03 (Duration Recalculation Test Presence)**: Verify unit test asserts `duration_pts`.
  - *Input*: Cargo test output stream.
  - *Expected*: Test asserting `duration_pts` reported as `ok`.
- **TC-T1-F07-04 (Zero Failure Assertion)**: Check failure count in test summary.
  - *Input*: `cargo test` summary line.
  - *Expected*: `0 failed; 0 errors`.
- **TC-T1-F07-05 (Doc-tests Execution)**: Verify doc-tests compile or report ok.
  - *Input*: Doc-test phase output.
  - *Expected*: `0 failed`.

---

### Group B: FFI Bridge & Tooling (`aether_bridge`)

#### Feature 8: Bridge Crate Dependency Fix
- **TC-T1-F08-01 (Uuid Dependency in Cargo.toml)**: Verify `crates/aether_bridge/Cargo.toml` contains `uuid`.
  - *Input*: Parse `crates/aether_bridge/Cargo.toml`.
  - *Expected*: `uuid` dependency declared with `v4` feature.
- **TC-T1-F08-02 (Crate Cargo Check)**: Execute `cargo check -p aether_bridge`.
  - *Input*: CLI execution.
  - *Expected*: Exit code 0, 0 compiler errors.
- **TC-T1-F08-03 (Direct Symbol Import)**: Verify `use uuid::Uuid;` in `aether_bridge/src/api.rs` compiles.
  - *Input*: Compiler analysis.
  - *Expected*: `Uuid` resolved cleanly.
- **TC-T1-F08-04 (Inter-crate Dependency Link)**: Verify `aether_core` dependency in `aether_bridge`.
  - *Input*: Inspect dependency path.
  - *Expected*: `aether_core = { path = "../aether_core" }` present.
- **TC-T1-F08-05 (Crate Type Declaration)**: Verify `crate-type = ["cdylib", "staticlib"]` in `Cargo.toml`.
  - *Input*: Inspect `[lib]` section.
  - *Expected*: Both `cdylib` and `staticlib` present.

#### Feature 9: Bridge API `init_engine`
- **TC-T1-F09-01 (Function Signature)**: Verify `pub fn init_engine()` exists in `aether_bridge/src/api.rs`.
  - *Input*: AST / Symbol check.
  - *Expected*: Symbol is public with `() -> ()` signature.
- **TC-T1-F09-02 (FRB User Utils Initialization)**: Verify call to `flutter_rust_bridge::setup_default_user_utils()`.
  - *Input*: Inspect function body.
  - *Expected*: FRB setup is invoked.
- **TC-T1-F09-03 (Render Engine Initialization)**: Verify call to `aether_render::init_render()`.
  - *Input*: Inspect function body.
  - *Expected*: Render engine initialization called.
- **TC-T1-F09-04 (Media Engine Initialization)**: Verify call to `aether_media::init_media()`.
  - *Input*: Inspect function body.
  - *Expected*: Media engine initialization called.
- **TC-T1-F09-05 (Idempotent Execution)**: Verify `init_engine()` can be called safely without panic.
  - *Input*: Call `init_engine()`.
  - *Expected*: Completes normally without panics.

#### Feature 10: Bridge API `create_timeline`
- **TC-T1-F10-01 (Function Signature & Return)**: Verify `pub fn create_timeline() -> Timeline`.
  - *Input*: Call `create_timeline()`.
  - *Expected*: Returns valid `Timeline` instance.
- **TC-T1-F10-02 (Initial Track Provisioning)**: Verify newly created timeline contains at least 1 default video track.
  - *Input*: Inspect `timeline.tracks`.
  - *Expected*: `timeline.tracks.len() >= 1`, with `tracks[0].kind == TrackKind::Video`.
- **TC-T1-F10-03 (Initial Duration Zero)**: Verify initial duration PTS.
  - *Input*: Inspect `timeline.duration_pts`.
  - *Expected*: `duration_pts == 0`.
- **TC-T1-F10-04 (Timebase Initialization)**: Verify standard initial timebase.
  - *Input*: Inspect `timeline.timebase`.
  - *Expected*: `Rational { num: 60, den: 1 }` or valid standard framerate.
- **TC-T1-F10-05 (Unique Timeline ID Generation)**: Call `create_timeline()` twice.
  - *Input*: Generate two timelines.
  - *Expected*: `tl1.id != tl2.id`.

#### Feature 11: Bridge API `add_clip_to_track`
- **TC-T1-F11-01 (FFI Mutation Success)**: Call `add_clip_to_track(timeline, track_id, source_id, 0, 60, 0)`.
  - *Input*: Existing timeline with valid `track_id`.
  - *Expected*: Returns `Ok(Timeline)` with 1 clip and `duration_pts == 60`.
- **TC-T1-F11-02 (Error Result on Missing Track)**: Call `add_clip_to_track` with invalid track ID.
  - *Input*: Random track UUID.
  - *Expected*: Returns `Err(String)` containing track not found diagnostic.
- **TC-T1-F11-03 (Error Result on Invalid Source Range)**: Call with `source_in >= source_out`.
  - *Input*: `source_in = 100`, `source_out = 50`.
  - *Expected*: Returns `Err(String)` indicating invalid bounds.
- **TC-T1-F11-04 (Preservation of Existing Clips)**: Add a second clip via bridge.
  - *Input*: Timeline already containing 1 clip; add another.
  - *Expected*: Resulting timeline has 2 clips.
- **TC-T1-F11-05 (Stateless Value Passing)**: Verify function takes ownership and returns mutated `Timeline` value.
  - *Input*: Rust signature `pub fn add_clip_to_track(mut timeline: Timeline, ...) -> Result<Timeline, String>`.
  - *Expected*: Pure value-based FFI serialization without persistent raw pointers.

#### Feature 12: Makefile Bridge Target Configuration
- **TC-T1-F12-01 (Makefile File Existence)**: Verify `Makefile` exists at project root.
  - *Input*: Test file presence.
  - *Expected*: File exists.
- **TC-T1-F12-02 (Phony Declaration)**: Verify `.PHONY: ... bridge` is declared.
  - *Input*: Inspect Makefile header.
  - *Expected*: `bridge` listed under `.PHONY`.
- **TC-T1-F12-03 (Target `bridge` Definition)**: Verify target rule `bridge:` is present.
  - *Input*: Inspect target rules.
  - *Expected*: Target executes `flutter_rust_bridge_codegen`.
- **TC-T1-F12-04 (Rust Root Parameter)**: Verify `--rust-root crates/aether_bridge` argument.
  - *Input*: Inspect command string.
  - *Expected*: Parameter accurately references bridge crate.
- **TC-T1-F12-05 (Flutter Root Parameter)**: Verify `--flutter-root apps/aether_app` argument.
  - *Input*: Inspect command string.
  - *Expected*: Parameter accurately references Flutter app.

#### Feature 13: FFI Codegen Execution
- **TC-T1-F13-01 (Execution of `make bridge`)**: Run `make bridge`.
  - *Input*: Trigger `make bridge`.
  - *Expected*: Codegen runs without syntax errors.
- **TC-T1-F13-02 (Rust Binding Generation)**: Verify generation of `frb_generated.rs` in `crates/aether_bridge/src/`.
  - *Input*: Check target file.
  - *Expected*: File generated and non-empty.
- **TC-T1-F13-03 (Dart Binding Generation)**: Verify generation of Dart bindings in Flutter app.
  - *Input*: Check `apps/aether_app/lib/src/rust/` or `lib/src/bridge/`.
  - *Expected*: Dart bridge files generated.
- **TC-T1-F13-04 (Export of Domain Models in Dart)**: Verify `Timeline`, `Track`, `Clip` generated in Dart.
  - *Input*: Inspect generated Dart models.
  - *Expected*: Classes exist with appropriate getters.
- **TC-T1-F13-05 (Export of Bridge Functions in Dart)**: Verify `createTimeline` and `addClipToTrack` available in Dart.
  - *Input*: Inspect generated Dart API.
  - *Expected*: Async functions generated with type signatures.

---

### Group C: Flutter App UI & Riverpod State (`apps/aether_app`)

#### Feature 14: Flutter Riverpod State Management
- **TC-T1-F14-01 (TimelineState Class Definition)**: Verify `TimelineState` class exists with `timeline`, `totalClipCount`, `durationPts`, `isLoading`, `errorMessage`.
  - *Input*: Source code inspection of `timeline_provider.dart`.
  - *Expected*: All required fields declared with immutable types.
- **TC-T1-F14-02 (TimelineNotifier Initialization)**: Verify `TimelineNotifier` initializes timeline state via bridge.
  - *Input*: Provider creation lifecycle.
  - *Expected*: Calls `createTimeline()` or populates initial state with default track.
- **TC-T1-F14-03 (Add Clip Method in Notifier)**: Verify `notifier.addClip()` method exists.
  - *Input*: Inspect notifier API.
  - *Expected*: Method invokes bridge FFI and updates state.
- **TC-T1-F14-04 (Total Clip Count Computation)**: Verify `totalClipCount` aggregates clips across all tracks.
  - *Input*: Timeline with track 1 (2 clips) and track 2 (1 clip).
  - *Expected*: `state.totalClipCount == 3`.
- **TC-T1-F14-05 (timelineProvider Declaration)**: Verify `timelineProvider` is exposed as `StateNotifierProvider`.
  - *Input*: Inspect top-level provider variable.
  - *Expected*: Provider exported and accessible by UI widgets.

#### Feature 15: Flutter Timeline View UI
- **TC-T1-F15-01 (TimelineView Widget Implementation)**: Verify `TimelineView` extends `ConsumerWidget`.
  - *Input*: Inspect `timeline_view.dart`.
  - *Expected*: Widget consumes Riverpod state via `WidgetRef`.
- **TC-T1-F15-02 (Clip Count Display Element)**: Verify widget displays total clip count text.
  - *Input*: State with `totalClipCount = 4`.
  - *Expected*: Widget tree contains Text node displaying "4" or "Clips: 4".
- **TC-T1-F15-03 (Total Clip Count Key)**: Verify widget includes `Key('timeline_total_clips_count')`.
  - *Input*: Search widget tree for key.
  - *Expected*: Key exists on count display widget.
- **TC-T1-F15-04 (Duration PTS Display)**: Verify duration PTS is displayed in UI.
  - *Input*: State with `durationPts = 120`.
  - *Expected*: Text node displaying duration PTS.
- **TC-T1-F15-05 (Track / Clip Lane Rendering)**: Verify list of tracks and clips is rendered.
  - *Input*: State with 1 track and 1 clip.
  - *Expected*: ListView/Column renders track container and clip card.

#### Feature 16: Add Clip UI Trigger
- **TC-T1-F16-01 (Add Clip Button Key)**: Verify UI contains button with `Key('add_clip_button')`.
  - *Input*: Inspect widget tree.
  - *Expected*: Button with key `add_clip_button` found.
- **TC-T1-F16-02 (Button Action Trigger)**: Verify clicking button calls `ref.read(timelineProvider.notifier).addClip()`.
  - *Input*: Inspect button `onPressed` callback.
  - *Expected*: Invokes notifier `addClip`.
- **TC-T1-F16-03 (Reactive State Update)**: Verify UI updates immediately when new clip is added.
  - *Input*: Simulate button trigger.
  - *Expected*: Clip count widget rebuilds with incremented value.
- **TC-T1-F16-04 (Visual Feedback / Loading State)**: Verify UI handles asynchronous loading during FFI call.
  - *Input*: Set `isLoading = true`.
  - *Expected*: Button or UI shows loading indicator or disabled state.
- **TC-T1-F16-05 (Error Snackbar / Banner Display)**: Verify UI surfaces error if FFI call fails.
  - *Input*: State with non-null `errorMessage`.
  - *Expected*: Error banner or text visible.

#### Feature 17: Static Analysis Configuration
- **TC-T1-F17-01 (Analysis Options File Existence)**: Verify `apps/aether_app/analysis_options.yaml` exists.
  - *Input*: Test file presence.
  - *Expected*: File exists.
- **TC-T1-F17-02 (Flutter Lints Inclusion)**: Verify `include: package:flutter_lints/flutter.yaml`.
  - *Input*: Parse yaml content.
  - *Expected*: Standard Flutter lints included.
- **TC-T1-F17-03 (Analyzer Rules Configuration)**: Verify analyzer is configured for strict type checking.
  - *Input*: Inspect analyzer configuration block.
  - *Expected*: Strict lints enabled without conflicting overrides.
- **TC-T1-F17-04 (Static Analysis Run)**: Run `flutter analyze` on `apps/aether_app`.
  - *Input*: Execute analyzer command.
  - *Expected*: 0 static errors, 0 syntax errors.
- **TC-T1-F17-05 (No Deprecated API Usage)**: Verify Dart files adhere to modern Flutter 3.x patterns.
  - *Input*: Static AST scan of all `.dart` files.
  - *Expected*: 0 deprecated API usages found.

---

### Group D: Integration & Verification

#### Feature 18: Full E2E Integration Verification
- **TC-T1-F18-01 (Acceptance Check: cargo test -p aether_core)**: Automated test passes without error.
  - *Input*: Execute runner step.
  - *Expected*: Exit code 0.
- **TC-T1-F18-02 (Acceptance Check: cargo check -p aether_bridge)**: Automated bridge check compiles cleanly.
  - *Input*: Execute runner step.
  - *Expected*: Exit code 0.
- **TC-T1-F18-03 (Acceptance Check: make bridge)**: Codegen target executes without failure.
  - *Input*: Execute runner step.
  - *Expected*: Exit code 0 or successfully generated bindings.
- **TC-T1-F18-04 (Acceptance Check: flutter analyze)**: Flutter static analysis reports 0 issues.
  - *Input*: Execute runner step.
  - *Expected*: Clean static analysis report.
- **TC-T1-F18-05 (Acceptance Check: Dart/Timeline Screen Verification)**: Automated verification that Dart Timeline reads clip list from Rust and displays count.
  - *Input*: Execute verification script on Dart UI files.
  - *Expected*: Validates connection between bridge model, Riverpod notifier, and UI keys.

#### Feature 19: Adversarial Coverage Hardening
- **TC-T1-F19-01 (Rapid Sequential Ingest Stress)**: Simulate 100 rapid sequential clip insertions.
  - *Input*: Loop adding 100 clips.
  - *Expected*: Final clip count is 100, PTS equals cumulative duration, 0 memory leaks.
- **TC-T1-F19-02 (Zero Duration Clip Prevention)**: Attempt to insert clip with `source_in == source_out`.
  - *Input*: `source_in = 60`, `source_out = 60`.
  - *Expected*: Fails with validation error.
- **TC-T1-F19-03 (Large PTS Bounds Handling)**: Test insertion with `i64::MAX / 2` bounds.
  - *Input*: Massive PTS timestamps.
  - *Expected*: Handles 64-bit integer values without overflow or truncation.
- **TC-T1-F19-04 (Multi-Track Concurrent Offsets)**: Insert clips across 10 distinct video and audio tracks at varying offsets.
  - *Input*: Complex EDL structure.
  - *Expected*: Global `duration_pts` accurately matches the single highest `timeline_out`.
- **TC-T1-F19-05 (Corrupted FFI Input Recovery)**: Send invalid JSON/binary payload across FFI boundary.
  - *Input*: Null or malformed bridge call.
  - *Expected*: FFI gracefully rejects without native process segfault.

---

## 4. Tier 2: Boundary & Corner Cases (>= 5 Test Cases per Feature)

### Group A: Rust Native Core (`aether_core`)

#### Feature 1: Domain Models & Trait Derivations
- **TC-T2-F01-01 (Zero Denominator Protection in Rational)**: Test `Rational` with `den == 0`.
  - *Input*: `Rational { num: 30, den: 0 }`.
  - *Expected*: Guarded or flagged as invalid timebase.
- **TC-T2-F01-02 (Negative Framerate Numerator in Rational)**: Test `Rational` with `num < 0`.
  - *Input*: `Rational { num: -30, den: 1 }`.
  - *Expected*: Rejected by timebase validator.
- **TC-T2-F01-03 (Empty Track Vector Clone Equality)**: Test cloning a `Timeline` with zero tracks.
  - *Input*: `Timeline { tracks: vec![], ... }`.
  - *Expected*: Clone equality evaluates to `true`.
- **TC-T2-F01-04 (Nil UUID Handling)**: Test struct behavior when instantiated with `Uuid::nil()`.
  - *Input*: `id = Uuid::nil()`.
  - *Expected*: Handled without crash; distinguished from non-nil UUIDs.
- **TC-T2-F01-05 (Max i64 PTS Representation in Clip)**: Test `Clip` with `source_out = i64::MAX`.
  - *Input*: Large boundary timestamp.
  - *Expected*: Stored accurately in 64-bit signed integer.

#### Feature 2: Timeline Error Hierarchy
- **TC-T2-F02-01 (Non-Existent Target Track among Existing Tracks)**: Timeline has 5 tracks; request target is a 6th non-existent UUID.
  - *Input*: 5 valid tracks, 1 invalid target.
  - *Expected*: Returns `TimelineError::TrackNotFound` with exact requested UUID.
- **TC-T2-F02-02 (Negative Timeline In Timestamp)**: Clip with `timeline_in < 0`.
  - *Input*: `timeline_in = -1`.
  - *Expected*: Returns `TimelineError::InvalidClipBounds`.
- **TC-T2-F02-03 (Equal Source In and Source Out)**: Clip with `source_in == source_out`.
  - *Input*: `source_in = 50`, `source_out = 50`.
  - *Expected*: Returns `TimelineError::InvalidClipBounds`.
- **TC-T2-F02-04 (Integer Overflow on Timeline Out)**: Clip with `timeline_in = i64::MAX - 10` and `duration = 20`.
  - *Input*: Bounds that would cause arithmetic overflow.
  - *Expected*: Returns `TimelineError::InvalidClipBounds` (checked arithmetic).
- **TC-T2-F02-05 (Multiple Error Disambiguation)**: Verify error enum discriminants are distinct in pattern matching.
  - *Input*: Match between `TrackNotFound` and `InvalidClipBounds`.
  - *Expected*: Distinct branches executed.

#### Feature 3: Clip Creation & Bounds
- **TC-T2-F03-01 (Minimal Valid Duration Clip)**: 1-tick duration clip (`source_in = 0`, `source_out = 1`).
  - *Input*: `source_in = 0, source_out = 1, timeline_in = 0`.
  - *Expected*: `timeline_out == 1`, valid clip.
- **TC-T2-F03-02 (Zero Timeline In Offset)**: Clip starting at timeline zero.
  - *Input*: `timeline_in = 0`.
  - *Expected*: `timeline_in == 0`, `timeline_out == source_out - source_in`.
- **TC-T2-F03-03 (Large Source In Offset)**: Clip starting far into source file.
  - *Input*: `source_in = 1,000,000`, `source_out = 1,000,060`.
  - *Expected*: `duration == 60`, `timeline_out == timeline_in + 60`.
- **TC-T2-F03-04 (Negative Source In Boundary Check)**: `source_in = -100`.
  - *Input*: Negative source start.
  - *Expected*: Rejected by constructor.
- **TC-T2-F03-05 (Source Out Before Source In)**: `source_in = 200, source_out = 100`.
  - *Input*: Inverted source boundaries.
  - *Expected*: Rejected by constructor.

#### Feature 4: Track Clip Insertion
- **TC-T2-F04-01 (Clip Starting Exactly at Previous Clip's Out)**: Seamless abutted clips.
  - *Input*: Clip 1 (0..100), Clip 2 (100..200).
  - *Expected*: Both clips retained; track clip count is 2.
- **TC-T2-F04-02 (Clip Starting After a Gap)**: Gap between clips.
  - *Input*: Clip 1 (0..100), Clip 2 (300..400).
  - *Expected*: Both clips retained; gap between 100 and 300 preserved.
- **TC-T2-F04-03 (Zero-Duration Track Capacity)**: Track initialized with zero initial capacity.
  - *Input*: `Vec::new()`.
  - *Expected*: Dynamic growth operates seamlessly.
- **TC-T2-F04-04 (Inserting Clip with Same UUID)**: Attempt to insert duplicate clip reference.
  - *Input*: Same clip ID inserted twice.
  - *Expected*: Handled cleanly by track management.
- **TC-T2-F04-05 (Inserting Clips Out of Chronological Order)**: Clip 2 inserted at PTS 50, then Clip 1 inserted at PTS 10.
  - *Input*: Out of order insertion.
  - *Expected*: Track records clips; recalculate duration resolves correct max PTS.

#### Feature 5: Timeline PTS Duration Recalculation
- **TC-T2-F05-01 (All Tracks Empty Duration)**: Multiple tracks, all with 0 clips.
  - *Input*: 3 tracks, 0 clips.
  - *Expected*: `duration_pts == 0`.
- **TC-T2-F05-02 (Single Track with 0 Clips, Another with 1 Clip)**: Verify empty tracks do not corrupt max calculation.
  - *Input*: Video track (0 clips), Audio track (clip ending at 300).
  - *Expected*: `duration_pts == 300`.
- **TC-T2-F05-03 (Identical End Points Across Tracks)**: Track 1 ends at 600, Track 2 ends at 600.
  - *Input*: Ties in max calculation.
  - *Expected*: `duration_pts == 600`.
- **TC-T2-F05-04 (Clips with Gap Before First Clip)**: Clip starts at PTS 1000 and ends at 1500.
  - *Input*: Timeline starts with empty silence.
  - *Expected*: `duration_pts == 1500`.
- **TC-T2-F05-05 (Single 1-tick Clip Recalculation)**: Clip with duration 1 tick at PTS 0.
  - *Input*: Clip (0..1).
  - *Expected*: `duration_pts == 1`.

#### Feature 6: Timeline Clip Insertion API
- **TC-T2-F06-01 (Insert into Track 1 of 5)**: Multiple tracks, target is the first track.
  - *Input*: Target track index 0.
  - *Expected*: Clip placed strictly in track 0.
- **TC-T2-F06-02 (Insert into Track 5 of 5)**: Multiple tracks, target is the last track.
  - *Input*: Target track index 4.
  - *Expected*: Clip placed strictly in track 4.
- **TC-T2-F06-03 (Insert with Nil Track UUID)**: Target track is `Uuid::nil()`.
  - *Input*: Nil UUID.
  - *Expected*: Returns `TimelineError::TrackNotFound(Uuid::nil())`.
- **TC-T2-F06-04 (Rapid Interleaved Insertions Across Tracks)**: Alternating clip insertions between Video and Audio tracks.
  - *Input*: 10 alternating insertions.
  - *Expected*: Accurate count and duration across both tracks.
- **TC-T2-F06-05 (Validation Failure Prevents Duration Mutation)**: Invalid clip bounds rejected without altering existing duration.
  - *Input*: Timeline with duration 100; reject invalid clip.
  - *Expected*: `duration_pts` remains 100.

#### Feature 7: Core Automated Unit Tests
- **TC-T2-F07-01 (Test Execution in Release Mode)**: Run `cargo test --release -p aether_core`.
  - *Input*: Optimized test execution.
  - *Expected*: All tests pass without optimization bugs.
- **TC-T2-F07-02 (Deterministic Execution Across Runs)**: Run `cargo test -p aether_core` 3 times in succession.
  - *Input*: Repeated runs.
  - *Expected*: Results identical across all iterations.
- **TC-T2-F07-03 (Single Threaded Execution)**: Run `cargo test -p aether_core -- --test-threads=1`.
  - *Input*: Sequential execution.
  - *Expected*: 0 thread-concurrency flakes.
- **TC-T2-F07-04 (Individual Test Target Execution)**: Run specific test `cargo test -p aether_core test_add_clip`.
  - *Input*: Targeted test filter.
  - *Expected*: Target test filters and passes.
- **TC-T2-F07-05 (Warning Free Compilation in Tests)**: Verify test code produces zero rustc warnings.
  - *Input*: Compiler diagnostics check.
  - *Expected*: 0 compiler warnings.

---

### Group B: FFI Bridge & Tooling (`aether_bridge`)

#### Feature 8: Bridge Crate Dependency Fix
- **TC-T2-F08-01 (Clean Cargo Check Output)**: Run `cargo check -p aether_bridge` without cached artifacts.
  - *Input*: Fresh check.
  - *Expected*: Exit code 0.
- **TC-T2-F08-02 (Dependency Feature Flags)**: Check that `uuid` includes feature `v4`.
  - *Input*: Inspect `Cargo.toml`.
  - *Expected*: `features = ["v4"]` is enabled.
- **TC-T2-F08-03 (Check with All Features)**: Run `cargo check -p aether_bridge --all-targets`.
  - *Input*: Check all targets including benches and examples.
  - *Expected*: Exit code 0.
- **TC-T2-F08-04 (Transitive Dependency Conflict Check)**: Verify no duplicate or conflicting `uuid` crate versions in `Cargo.lock`.
  - *Input*: Inspect workspace resolver.
  - *Expected*: Unified `uuid` crate resolution.
- **TC-T2-F08-05 (Workspace Level Check)**: Run `cargo check --workspace`.
  - *Input*: Check all crates.
  - *Expected*: Clean compilation across all workspace members.

#### Feature 9: Bridge API `init_engine`
- **TC-T2-F09-01 (Multiple Calls to `init_engine`)**: Call `init_engine()` multiple times consecutively.
  - *Input*: 3 consecutive calls.
  - *Expected*: No panic or race conditions.
- **TC-T2-F09-02 (Call `init_engine` from Non-Main Thread)**: Execute `init_engine` on background worker thread.
  - *Input*: Background thread spawn.
  - *Expected*: Safely completes.
- **TC-T2-F09-03 (Render Engine Readiness Check)**: Verify render engine state after `init_engine`.
  - *Input*: Query render subsystem.
  - *Expected*: Subsystem is initialized.
- **TC-T2-F09-04 (Media Engine Readiness Check)**: Verify media engine state after `init_engine`.
  - *Input*: Query media subsystem.
  - *Expected*: Subsystem is initialized.
- **TC-T2-F09-05 (Init Engine Prior to Timeline Operations)**: Verify operations proceed normally after init.
  - *Input*: `init_engine()` followed by `create_timeline()`.
  - *Expected*: Successful initialization and timeline creation.

#### Feature 10: Bridge API `create_timeline`
- **TC-T2-F10-01 (Rapid Successive Invocations)**: Call `create_timeline()` 50 times in a loop.
  - *Input*: 50 calls.
  - *Expected*: 50 distinct timelines with unique UUIDs.
- **TC-T2-F10-02 (Default Track UUID Non-Nil)**: Verify default video track has a valid non-nil UUID.
  - *Input*: Inspect `tl.tracks[0].id`.
  - *Expected*: `track.id != Uuid::nil()`.
- **TC-T2-F10-03 (Initial Track Clips Vector Empty)**: Verify `tracks[0].clips.is_empty()`.
  - *Input*: Inspect track clips.
  - *Expected*: Length is 0.
- **TC-T2-F10-04 (Timebase Denominator Non-Zero)**: Verify denominator is strictly non-zero.
  - *Input*: `tl.timebase.den`.
  - *Expected*: `den > 0` (e.g. 1).
- **TC-T2-F10-05 (Duration PTS Exactly Zero)**: Verify initial duration is zero.
  - *Input*: `tl.duration_pts`.
  - *Expected*: `== 0`.

#### Feature 11: Bridge API `add_clip_to_track`
- **TC-T2-F11-01 (Zero-Length Source Boundary via Bridge)**: Call bridge with `source_in = 10, source_out = 10`.
  - *Input*: Invalid source range.
  - *Expected*: Returns `Err(String)` containing descriptive error message.
- **TC-T2-F11-02 (Negative Timestamp Across FFI)**: Call bridge with `timeline_in = -50`.
  - *Input*: Negative timeline offset.
  - *Expected*: Returns `Err(String)`.
- **TC-T2-F11-03 (Invalid Track ID Across FFI)**: Call bridge with non-existent track UUID.
  - *Input*: Unmatched track UUID.
  - *Expected*: Returns `Err(String)` mentioning track not found.
- **TC-T2-F11-04 (Nil Source ID via Bridge)**: Call bridge with `source_id = Uuid::nil()`.
  - *Input*: Nil source UUID.
  - *Expected*: Accepted if bounds valid, or rejected if nil source disallowed; deterministic response.
- **TC-T2-F11-05 (Max Value Timestamps via Bridge)**: Call bridge with large 64-bit integer timestamps.
  - *Input*: `source_out = 1_000_000_000`.
  - *Expected*: Accurately serializes and calculates duration without 32-bit truncation.

#### Feature 12: Makefile Bridge Target Configuration
- **TC-T2-F12-01 (Make Target Execution in Clean Workspace)**: Run `make -n bridge` (dry run).
  - *Input*: `make -n bridge`.
  - *Expected*: Prints expected codegen command line.
- **TC-T2-F12-02 (Target Name Resolution)**: Ensure no target conflicts in Makefile.
  - *Input*: Query Makefile rules.
  - *Expected*: `bridge` target uniquely defined.
- **TC-T2-F12-03 (Path Relative Resolution)**: Verify paths in Makefile resolve from workspace root.
  - *Input*: Execute from repository root.
  - *Expected*: Relative paths `crates/aether_bridge` and `apps/aether_app` correctly match filesystem.
- **TC-T2-F12-04 (Environment Variable Overrides)**: Test make invocation with custom options.
  - *Input*: Run make with standard flags.
  - *Expected*: Makefile executes without syntax errors.
- **TC-T2-F12-05 (Setup Target Presence)**: Verify `setup` target in Makefile.
  - *Input*: Inspect Makefile.
  - *Expected*: `setup` rule present for toolchain dependencies.

#### Feature 13: FFI Codegen Execution
- **TC-T2-F13-01 (Repeated Codegen Idempotency)**: Run codegen twice in succession.
  - *Input*: Successive codegen invocations.
  - *Expected*: Second run produces identical generated output without churn.
- **TC-T2-F13-02 (Dart Output Directory Placement)**: Verify generated Dart files match app import conventions.
  - *Input*: Check generated Dart file paths.
  - *Expected*: Files reside in configured output location (`lib/src/rust` or `lib/src/bridge`).
- **TC-T2-F13-03 (Rust Generated Module Binding)**: Verify `mod frb_generated;` compiles inside `crates/aether_bridge/src/lib.rs`.
  - *Input*: `cargo check -p aether_bridge`.
  - *Expected*: Clean compilation of generated module.
- **TC-T2-F13-04 (Dart Null Safety Compliance)**: Verify generated Dart code uses sound null safety.
  - *Input*: Dart code inspection.
  - *Expected*: No unsound null safety directives.
- **TC-T2-F13-05 (FRB Version Compatibility)**: Verify FRB v2 features (e.g. `RustLib.init()`) are appropriately supported.
  - *Input*: Inspect generated initialization routines.
  - *Expected*: Compatible with FRB v2.3.0.

---

### Group C: Flutter App UI & Riverpod State (`apps/aether_app`)

#### Feature 14: Flutter Riverpod State Management
- **TC-T2-F14-01 (Initial State Null Timeline Handling)**: State before timeline is loaded (`timeline == null`).
  - *Input*: Initial state initialization.
  - *Expected*: `totalClipCount == 0`, `durationPts == 0`, `isLoading == true` or `tracks == []`.
- **TC-T2-F14-02 (Error State Handling in Notifier)**: Bridge call throws exception.
  - *Input*: Simulate bridge error in `addClip()`.
  - *Expected*: `state.errorMessage != null`, `isLoading == false`, previous clips retained.
- **TC-T2-F14-03 (Concurrent Add Clip Prevention)**: Rapid multiple taps on Add Clip before FFI returns.
  - *Input*: 2 calls to `addClip()` while `isLoading == true`.
  - *Expected*: Ignored or queued safely without state corruption.
- **TC-T2-F14-04 (Immutable State CopyWith Integrity)**: Verify `TimelineState.copyWith` returns new instance.
  - *Input*: `state.copyWith(totalClipCount: 5)`.
  - *Expected*: New instance has count 5, other fields preserved.
- **TC-T2-F14-05 (Disposed Provider Guard)**: Verify notifier handles disposal gracefully.
  - *Input*: Unmount ProviderScope.
  - *Expected*: No unhandled memory leaks or orphan listeners.

#### Feature 15: Flutter Timeline View UI
- **TC-T2-F15-01 (Zero Clip State Rendering)**: UI rendering when `totalClipCount == 0`.
  - *Input*: Timeline with 0 clips.
  - *Expected*: Displays "0" in clip count widget without crashing.
- **TC-T2-F15-02 (Large Clip Count Rendering)**: UI rendering with 999+ clips.
  - *Input*: `totalClipCount = 9999`.
  - *Expected*: Number fits in UI container without layout overflow.
- **TC-T2-F15-03 (Loading State Overlay)**: UI rendering when `isLoading == true`.
  - *Input*: State has `isLoading = true`.
  - *Expected*: Shows CircularProgressIndicator or disabled button.
- **TC-T2-F15-04 (Error Banner Rendering)**: UI rendering when `errorMessage` is set.
  - *Input*: State has `errorMessage = "Track not found"`.
  - *Expected*: Error message displayed to user.
- **TC-T2-F15-05 (Empty Tracks List Handling)**: UI rendering when `tracks.isEmpty`.
  - *Input*: `tracks = []`.
  - *Expected*: Empty state message or placeholder rendered gracefully.

#### Feature 16: Add Clip UI Trigger
- **TC-T2-F16-01 (Disabled Button State during Loading)**: Check button `onPressed` is null when loading.
  - *Input*: `state.isLoading == true`.
  - *Expected*: Button is disabled to prevent duplicate requests.
- **TC-T2-F16-02 (Button Key Uniqueness)**: Check that `Key('add_clip_button')` appears exactly once in the widget tree.
  - *Input*: Search widget tree for key.
  - *Expected*: Exactly 1 matching widget found.
- **TC-T2-F16-03 (Rapid Double Tap Simulation)**: Simulate two tap events within 10ms.
  - *Input*: Consecutive tap events.
  - *Expected*: Only 1 FFI request dispatched while loading.
- **TC-T2-F16-04 (Button Accessible Label)**: Check accessibility semantics of Add Clip button.
  - *Input*: Inspect Semantics widget.
  - *Expected*: Button has meaningful tooltip or accessible label.
- **TC-T2-F16-05 (UI State Reset on Error Dismissal)**: Dismiss error banner and re-enable Add Clip button.
  - *Input*: Reset error message.
  - *Expected*: Button re-enabled.

#### Feature 17: Static Analysis Configuration
- **TC-T2-F17-01 (Missing Include Fallback)**: Test behavior if flutter_lints is resolved in pub cache.
  - *Input*: Verify analysis config syntax.
  - *Expected*: Syntax is valid YAML.
- **TC-T2-F17-02 (Strict Casts Rule)**: Check analyzer options for strict-casts.
  - *Input*: Inspect `language: strict-casts`.
  - *Expected*: Configured or compatible with flutter_lints.
- **TC-T2-F17-03 (Unused Import Rule Enforcement)**: Ensure no unused imports in any Flutter file.
  - *Input*: AST check for imports.
  - *Expected*: 0 unused imports.
- **TC-T2-F17-04 (Constant Constructor Lint Enforcement)**: Ensure `const` constructors used wherever possible.
  - *Input*: AST check for widget instantiations.
  - *Expected*: `prefer_const_constructors` followed.
- **TC-T2-F17-05 (Zero Warning Static Analysis)**: Run static analysis.
  - *Input*: Analyze all dart files in `lib/`.
  - *Expected*: Clean exit with 0 errors and 0 warnings.

---

### Group D: Integration & Verification

#### Feature 18: Full E2E Integration Verification
- **TC-T2-F18-01 (E2E Runner Handles Offline Toolchain Gracefully)**: E2E runner reports specific actionable missing tool diagnostics.
  - *Input*: Execute runner when optional tools missing.
  - *Expected*: Actionable error code and diagnosis.
- **TC-T2-F18-02 (E2E Runner Verifies Exit Codes Reliably)**: Runner captures non-zero exit codes accurately.
  - *Input*: Injected failure command.
  - *Expected*: Runner flags step failure and aggregates report.
- **TC-T2-F18-03 (E2E Runner JSON Report Generation)**: Runner produces structured machine-readable report.
  - *Input*: Runner execution.
  - *Expected*: JSON output with timestamp, pass/fail status, and steps.
- **TC-T2-F18-04 (Acceptance Criteria Cross-Reference)**: All criteria mapped in runner output.
  - *Input*: Runner test suite.
  - *Expected*: Exactly matches ORIGINAL_REQUEST acceptance criteria.
- **TC-T2-F18-05 (Idempotent Test Suite Execution)**: Runner can be executed multiple times without stale cache pollution.
  - *Input*: Successive runs.
  - *Expected*: Clean pass/fail reporting.

#### Feature 19: Adversarial Coverage Hardening
- **TC-T2-F19-01 (Negative PTS Arithmetic Inversion Stress)**: Inverted time ranges (`timeline_in = 100`, `duration = -50`).
  - *Input*: Negative duration.
  - *Expected*: Handled by safety guards; rejected cleanly.
- **TC-T2-F19-02 (Extreme Track Count Stress)**: Create timeline with 1,000 empty tracks.
  - *Input*: 1,000 tracks added.
  - *Expected*: Memory footprint remains stable, duration is 0.
- **TC-T2-F19-03 (Zero Framerate Timebase Stress)**: Compute PTS conversion with 0 fps timebase.
  - *Input*: Rational { num: 0, den: 1 }.
  - *Expected*: Protected from division-by-zero panics.
- **TC-T2-F19-04 (Clip Overlap Conflict Tolerance)**: Insert overlapping clips on same track.
  - *Input*: Non-destructive model handles overlapping clip segments.
  - *Expected*: `duration_pts` equals furthest extent; clips preserved in track layer.
- **TC-T2-F19-05 (Invalid UUID String Conversion across FFI)**: Pass malformed UUID string to bridge.
  - *Input*: Malformed string `"not-a-uuid"`.
  - *Expected*: Bridge catches error and returns `Err(String)` without crashing.

---

## 5. Tier 3: Cross-Feature Combinations (Pairwise & Multi-Module Interactions)

| Test ID | Combined Features | Scenario Description | Expected Outcome |
|---------|-------------------|----------------------|------------------|
| **TC-T3-01** | F01 + F04 + F05 | Model domain -> Track insertion -> PTS recalculation on native Core | Adding clip to track triggers duration recalculation; `duration_pts == clip.timeline_out`. |
| **TC-T3-02** | F02 + F06 + F11 | Error hierarchy -> Public core API -> Bridge FFI error propagation | Non-existent track ID passed through FFI results in `Err(String)` describing track not found; state is not mutated. |
| **TC-T3-03** | F08 + F10 + F11 | Bridge UUID fix -> `create_timeline` -> `add_clip_to_track` | Bridge compiles with `uuid`; `create_timeline` returns timeline with 1 video track; `add_clip_to_track` succeeds on that track. |
| **TC-T3-04** | F10 + F14 + F15 | Bridge `create_timeline` -> Riverpod state -> `TimelineView` | App startup calls `create_timeline`; Riverpod state populated with 1 track; UI renders empty track lane and "0" clip count. |
| **TC-T3-05** | F11 + F14 + F16 | Bridge `add_clip` -> Riverpod `addClip()` -> UI Add Clip Button | User clicks button with `Key('add_clip_button')`; notifier executes FFI call; UI clip count widget with `Key('timeline_total_clips_count')` increments. |
| **TC-T3-06** | F05 + F14 + F15 | Multi-track PTS recalculation -> Riverpod state -> Duration PTS display | Multiple clips added across Video and Audio tracks; UI displays max PTS matching longest track. |
| **TC-T3-07** | F12 + F13 + F17 | Makefile bridge target -> FRB codegen -> Static analysis | `make bridge` generates Dart bindings; `flutter analyze` runs without unresolved imports or type errors. |
| **TC-T3-08** | F06 + F11 + F19 | Boundary bounds -> FFI serialization -> Adversarial error resilience | Extreme timestamps (`i64::MAX / 2`) sent across FFI; bridge validates bounds and updates duration without 32-bit truncation. |

---

## 6. Tier 4: Real-World Application Scenarios (Comprehensive Workflows)

### Scenario 1: Initial Project Creation & Setup
- **Workflow**:
  1. User launches Aether application (`AetherApp`).
  2. Engine initializes native subsystems via `init_engine()`.
  3. App instantiates project timeline via `create_timeline()`.
  4. Riverpod `timelineProvider` receives immutable timeline with default video track.
  5. UI renders `TimelineView` displaying 1 empty video track lane, 0 clips, and duration 0 PTS.
- **Verification Criteria**:
  - `TimelineState.timeline != null`.
  - `TimelineState.totalClipCount == 0`.
  - `TimelineState.durationPts == 0`.
  - `Key('timeline_total_clips_count')` widget shows "0".

### Scenario 2: Sequential Multi-Clip Ingestion & Timeline Extension
- **Workflow**:
  1. User taps "Add Clip" (`Key('add_clip_button')`).
  2. First clip (PTS 0 to 120, duration 120) is added to Video Track 1.
  3. Timeline recalculates `duration_pts = 120`.
  4. UI updates `timeline_total_clips_count` to "1".
  5. User taps "Add Clip" a second time.
  6. Second clip (PTS 120 to 300, duration 180) is appended to Video Track 1.
  7. Timeline recalculates `duration_pts = 300`.
  8. UI updates `timeline_total_clips_count` to "2".
- **Verification Criteria**:
  - `totalClipCount == 2`.
  - `duration_pts == 300`.
  - Clip list in UI displays 2 cards.

### Scenario 3: Multi-Track Composition (Video + Audio)
- **Workflow**:
  1. Timeline contains Video Track 1 and Audio Track 1.
  2. Video clip of duration 200 PTS added to Video Track (ends at 200).
  3. Audio clip of duration 350 PTS added to Audio Track (ends at 350).
  4. Engine recalculates `duration_pts = max(200, 350) = 350`.
  5. Riverpod state synchronizes with Flutter UI.
- **Verification Criteria**:
  - Both tracks contain their respective clips.
  - `totalClipCount == 2`.
  - `duration_pts == 350`.
  - UI correctly displays aggregated clip count across all track types.

### Scenario 4: Error Recovery & Fault Tolerance
- **Workflow**:
  1. Application attempts to add a clip with invalid bounds (e.g. `source_in >= source_out`).
  2. Core rejects the operation returning `TimelineError::InvalidClipBounds`.
  3. Bridge propagates error as `Err("Invalid clip bounds: source_in >= source_out")`.
  4. Riverpod `TimelineNotifier` catches error without crashing, sets `errorMessage`, and resets `isLoading = false`.
  5. UI displays error notice; total clip count and existing timeline state remain unmodified.
- **Verification Criteria**:
  - No application crash or native panic.
  - Previous timeline clips and duration remain intact.
  - UI indicates error gracefully and allows user to retry.

---

## 7. Test Suite Layout & Implementation Strategy

```
tests/
├── e2e_runner.py                # Main automated E2E test runner executing all acceptance checks
├── run_tests.sh                 # Unified shell script runner for CLI execution
├── test_rust_core.py            # Automated runner & validator for `cargo test -p aether_core`
├── test_bridge_contract.py      # Automated runner & validator for `cargo check -p aether_bridge` and `make bridge`
├── test_dart_ui_contract.py     # Automated AST & contract validator for Flutter Riverpod & Timeline UI
└── test_adversarial_scenarios.py # Adversarial stress tests (bounds, overflow, multi-track, rapid ops)
```

The test runner produces structured terminal output, detailed failure diagnostics, and a comprehensive JSON summary artifact (`tests/test_results.json`) for reporting to Sentinel and the Orchestrator.
