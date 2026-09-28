# Project: Aether Full-stack Slice

## Architecture
Aether is a cross-platform NLE (Non-Linear Editor) video editor with a high-performance native core in Rust and a reactive user interface in Flutter. The architecture separates concerns across three tiers:
1. **Core Domain Tier (`aether_core`)**: Pure Rust domain model implementing a non-destructive edit decision list / evaluation DAG. Models `Timeline`, `Track`, `Clip`, and `Rational` timebases. Calculates Presentation Time Stamps (PTS) and overall timeline duration.
2. **FFI Bridge Tier (`aether_bridge`)**: Exposes native core capabilities to Dart using `flutter_rust_bridge` (FRB v2). Provides value-based, stateless API endpoints (`create_timeline`, `add_clip_to_track`) with full mirror attributes (`#[frb(mirror(...))]`) and 64-bit integer mappings (`--type-64bit-int`) that serialize domain models cleanly as concrete Dart classes.
3. **Application & State Tier (`aether_app`)**: Flutter UI built on Material 3 and Riverpod 2.x. `TimelineNotifier` manages immutable timeline states received from Rust and issues asynchronous FFI commands. `TimelineView` observes the state, displaying track and clip cards, total clip counts, duration PTS, and an "Add Clip" action button.

```
+-------------------------------------------------------------+
|                 Flutter UI (apps/aether_app)                |
|  TimelineView (Clip Count, Duration PTS, Add Clip Button)   |
+------------------------------+------------------------------+
                               |
                               v
+-------------------------------------------------------------+
|              State Layer (Riverpod 2.x)                     |
|           TimelineNotifier / TimelineState                  |
+------------------------------+------------------------------+
                               |
                               v (Dart FFI calls)
+-------------------------------------------------------------+
|         FFI Bridge Layer (crates/aether_bridge)             |
|       flutter_rust_bridge v2 Codegen & Mirrored Structs     |
+------------------------------+------------------------------+
                               |
                               v (Rust native calls)
+-------------------------------------------------------------+
|            Rust Native Core (crates/aether_core)            |
|  Timeline DAG: Tracks, Clips, PTS & duration recalculation  |
+-------------------------------------------------------------+
```

## Feature Inventory
| # | Feature | Description | Milestone | Source |
|---|---------|-------------|-----------|--------|
| 1 | Domain Models & Trait Derivations | `Timeline`, `Track`, `Clip`, `Rational`, `TrackKind` with `PartialEq, Eq, Debug, Clone, Copy` | M1 | survey_explorer_rust [DONE] |
| 2 | Timeline Error Hierarchy | `TimelineError` enum handling `TrackNotFound`, `InvalidClipBounds`, `InvalidSourceBounds` | M1 | survey_explorer_rust [DONE] |
| 3 | Clip Creation & Bounds | `Clip::new` with automatic `timeline_out` computation from source duration | M1 | survey_explorer_rust [DONE] |
| 4 | Track Clip Insertion | `Track::add_clip` pushing clip into track layer | M1 | survey_explorer_rust [DONE] |
| 5 | Timeline PTS Duration Recalculation | `Timeline::recalculate_duration` calculating `max(clip.timeline_out)` across all tracks (clamped >= 0) | M1 | survey_explorer_rust / ORIGINAL_REQUEST R1 [DONE] |
| 6 | Timeline Clip Insertion API | `Timeline::add_clip` validating bounds, appending clip to track, and recalculating duration | M1 | survey_explorer_rust / ORIGINAL_REQUEST R1 [DONE] |
| 7 | Core Automated Unit Tests | 18 unit and adversarial integration tests validating `add_clip`, `duration_pts` calculation, and bounds | M1 | survey_explorer_rust / Acceptance Criteria [DONE] |
| 8 | Bridge Crate Dependency Fix | Added `uuid` and `flutter_rust_bridge` with `features = ["uuid"]` in `aether_bridge/Cargo.toml` | M1 | survey_spec_miner_bridge [DONE] |
| 9 | Bridge API `init_engine` | Initializes native render, media, and FRB user utils | M1 | survey_spec_miner_bridge [DONE] |
| 10 | Bridge API `create_timeline` | Instantiates initial timeline with default video track | M1 | survey_spec_miner_bridge [DONE] |
| 11 | Bridge API `add_clip_to_track` | FFI function taking timeline + clip parameters, mutating via core, returning `Result<Timeline, String>` | M1 | survey_spec_miner_bridge / ORIGINAL_REQUEST R1 [DONE] |
| 12 | Makefile Bridge Target Configuration | Configured `make bridge` with `--type-64bit-int`, `--rust-input crate::api`, and `all: bridge` | M1 | survey_spec_miner_bridge / Acceptance Criteria [DONE] |
| 13 | FFI Codegen Execution | Generated concrete Dart classes (`Timeline`, `Track`, `Clip`, etc.) with public getters | M1 | survey_spec_miner_bridge / Acceptance Criteria [DONE] |
| 14 | Flutter Riverpod State Management | `TimelineState` and `TimelineNotifier` consuming Rust Timeline via bridge | M2 | survey_explorer_flutter / ORIGINAL_REQUEST R2 [PLANNED] |
| 15 | Flutter Timeline View UI | `TimelineView` displaying total clip count, duration PTS, and track details | M2 | survey_explorer_flutter / ORIGINAL_REQUEST R2 [PLANNED] |
| 16 | Add Clip UI Trigger | Button with `Key('add_clip_button')` triggering `add_clip` FFI and updating clip count in UI | M2 | survey_explorer_flutter / ORIGINAL_REQUEST R2 [PLANNED] |
| 17 | Static Analysis Configuration | `analysis_options.yaml` in `apps/aether_app` ensuring clean `flutter analyze` | M2 | survey_explorer_flutter / Acceptance Criteria [PLANNED] |
| 18 | Full E2E Integration Verification | Automated verification of all acceptance criteria (`cargo test`, `cargo check`, `make bridge`, `flutter analyze`) | M3 | ORIGINAL_REQUEST Acceptance Criteria [PLANNED] |
| 19 | Adversarial Coverage Hardening | White-box stress tests, invalid bounds, edge cases verification | M3 | Project Pattern Phase 2 [PLANNED] |

## Milestones
| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| 1 | M1: Native Engine & Bridge (R1) | Implement `aether_core` clip addition & PTS recalculation, 18 unit/adversarial tests, `aether_bridge` FFI mirrored methods, and `make bridge` codegen | none | DONE |
| 2 | M2: Flutter Application & State (R2) | Implement `TimelineNotifier`, `TimelineState`, `TimelineView` UI with clip count and Add Clip button, and `analysis_options.yaml` | M1 | IN_PROGRESS |
| 3 | M3: Final Milestone (E2E & Hardening) | Phase 1: Pass 100% E2E tests and acceptance criteria; Phase 2: Adversarial coverage hardening | M2 | PLANNED |

## Parallel Tracks
- **Track 1: Implementation Track** (M1 [DONE] -> M2 [IN_PROGRESS] -> M3 [PLANNED])
- **Track 2: E2E Testing Track** (Requirement-driven test harness, Tiers 1-4, `TEST_INFRA.md` & `TEST_READY.md` [PUBLISHED & READY])

## Interface Contracts

### `aether_core` ↔ `aether_bridge`
- `Timeline::new(timebase: Rational) -> Timeline`
- `Timeline::new_with_default_tracks(timebase: Rational) -> Timeline`
- `Timeline::add_track(&mut self, kind: TrackKind) -> Uuid`
- `Timeline::add_clip(&mut self, track_id: Uuid, clip: Clip) -> Result<(), TimelineError>`
- `Timeline::recalculate_duration(&mut self)`
- `Timeline.duration_pts: i64`
- `Clip::new(source_id: Uuid, source_in: i64, source_out: i64, timeline_in: i64) -> Clip`

### `aether_bridge` ↔ `aether_app` (FFI Boundary - Mirrored)
- `init_engine() -> Future<void>`
- `create_timeline() -> Future<Timeline>`
- `add_track({required Timeline timeline, required TrackKind kind}) -> Future<Timeline>`
- `add_clip_to_track({required Timeline timeline, required UuidValue trackId, required UuidValue sourceId, required int sourceIn, required int sourceOut, required int timelineIn}) -> Future<Timeline>`
- Concrete Dart Classes:
  - `class Timeline { final UuidValue id; final Rational timebase; final int durationPts; final List<Track> tracks; }`
  - `class Track { final UuidValue id; final TrackKind kind; final List<Clip> clips; }`
  - `class Clip { final UuidValue id; final UuidValue sourceId; final int sourceIn; final int sourceOut; final int timelineIn; final int timelineOut; }`
  - `enum TrackKind { video, audio, overlay; }`
  - `class Rational { final int num; final int den; }`

### `aether_app` Riverpod ↔ UI
- `timelineProvider`: `StateNotifierProvider<TimelineNotifier, TimelineState>`
- `TimelineState`:
  - `timeline: Timeline?`
  - `totalClipCount: int`
  - `durationPts: int`
  - `tracks: List<Track>`
  - `isLoading: bool`
  - `errorMessage: String?`
- UI Keys:
  - `Key('timeline_total_clips_count')`: Displays total clip count text.
  - `Key('add_clip_button')`: Button invoking `ref.read(timelineProvider.notifier).addClip()`.

## Code Layout & File Boundaries
- **Implementation Track M1** [DONE]:
  - `crates/aether_core/src/timeline.rs`
  - `crates/aether_core/src/lib.rs`
  - `crates/aether_bridge/Cargo.toml`
  - `crates/aether_bridge/src/api.rs`
  - `crates/aether_bridge/src/lib.rs`
  - `Makefile`
- **Implementation Track M2** [IN_PROGRESS]:
  - `apps/aether_app/pubspec.yaml`
  - `apps/aether_app/analysis_options.yaml` (Exclusive to M2 Worker)
  - `apps/aether_app/lib/main.dart` (Exclusive to M2 Worker)
  - `apps/aether_app/lib/src/features/timeline/timeline_provider.dart` (Exclusive to M2 Worker)
  - `apps/aether_app/lib/src/features/timeline/timeline_view.dart` (Exclusive to M2 Worker)
- **E2E Testing Track** [READY]:
  - `TEST_INFRA.md`
  - `TEST_READY.md`
  - `tests/` and `apps/aether_app/test/`
