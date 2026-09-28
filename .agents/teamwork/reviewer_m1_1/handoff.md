# Milestone M1 Review Report & Adversarial Challenge

**Verdict**: **REQUEST_CHANGES**  
**Reviewer Role**: Reviewer 1 & Adversarial Critic  
**Target Milestone**: M1 (Native Engine & Bridge)  
**Worker Reviewed**: `worker_m1_1`  

---

## 1. Observation

### 1.1 Automated Verification Commands
We independently executed all verification commands in the workspace:
1. `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p aether_core`:
   ```
   running 11 tests
   test timeline::tests::test_add_clip_to_track_and_recalculate_duration ... ok
   test timeline::tests::test_add_clip_track_not_found ... ok
   test timeline::tests::test_invalid_clip_bounds ... ok
   test timeline::tests::test_invalid_source_bounds ... ok
   test timeline::tests::test_new_with_default_tracks ... ok
   test timeline::tests::test_add_multiple_clips_recalculates_max_pts ... ok
   test timeline::tests::test_recalculate_duration_empty_tracks ... ok
   test timeline::tests::test_timeline_error_display ... ok
   test timeline::tests::test_staggered_clips_with_gaps_across_tracks ... ok
   test timeline::tests::test_track_with_id_and_duration ... ok
   test timeline::tests::test_zero_duration_clip ... ok

   test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   ```
2. `export PATH="$HOME/.cargo/bin:$PATH" && cargo check -p aether_bridge`:
   - Exit code: 0
   - Finished dev profile in 0.12s.
3. `export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:$PATH" && make bridge`:
   - Exit code: 0
   - Generated bridge bindings into `apps/aether_app/lib/src/bridge/` and `crates/aether_bridge/src/frb_generated.rs` with `Done!`.
4. `export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:$PATH" && cargo test --workspace`:
   - Exit code: 0 across all 4 crates (`aether_core`, `aether_bridge`, `aether_media`, `aether_render`).
5. `export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:$PATH" && flutter analyze apps/aether_app`:
   - Exit code: 0 ("No issues found!").

### 1.2 Inspection of Implementation Files
- **`crates/aether_core/src/timeline.rs`**:
  - Implements `TimelineError` (`TrackNotFound`, `InvalidClipBounds`, `InvalidSourceBounds`) with `std::fmt::Display` and `std::error::Error`.
  - Derives `Debug, Clone, PartialEq, Eq` on all models; derives `Copy` on `Rational` and `TrackKind`.
  - `Clip::new` computes `duration = source_out.saturating_sub(source_in)` and `timeline_out = timeline_in.saturating_add(duration)`.
  - `Timeline::recalculate_duration` computes `max(clip.timeline_out)` across all tracks via `tracks.iter().flat_map(|track| track.clips.iter()).map(|clip| clip.timeline_out).max().unwrap_or(0)`.
  - `Timeline::add_clip` validates `source_in <= source_out` and `timeline_in <= timeline_out`, verifies track existence, appends the clip, and calls `recalculate_duration()`.
  - Contains 11 comprehensive unit tests verifying zero durations, gaps, multi-tracks, errors, and bounds.
- **`crates/aether_bridge/Cargo.toml`**:
  - Added `uuid = { version = "1.10", features = ["v4"] }`.
- **`crates/aether_bridge/src/api.rs`**:
  - Contains:
    ```rust
    pub use aether_core::timeline::{Clip, Rational, Timeline, TrackKind};
    use uuid::Uuid;

    pub fn init_engine() {
        flutter_rust_bridge::setup_default_user_utils();
        aether_render::init_render();
        aether_media::init_media();
    }

    pub fn create_timeline() -> Timeline {
        let mut timeline = Timeline::new(Rational { num: 60, den: 1 });
        timeline.add_track(TrackKind::Video);
        timeline
    }

    pub fn add_track(mut timeline: Timeline, kind: TrackKind) -> Timeline {
        timeline.add_track(kind);
        timeline
    }

    pub fn add_clip_to_track(
        mut timeline: Timeline,
        track_id: Uuid,
        source_id: Uuid,
        source_in: i64,
        source_out: i64,
        timeline_in: i64,
    ) -> Result<Timeline, String> {
        let clip = Clip::new(source_id, source_in, source_out, timeline_in);
        timeline
            .add_clip(track_id, clip)
            .map_err(|e| e.to_string())?;
        Ok(timeline)
    }
    ```
- **Generated Dart Output `apps/aether_app/lib/src/bridge/api.dart`**:
  - Generated lines 34-38 verbatim:
    ```dart
    // Rust type: RustOpaqueMoi<flutter_rust_bridge::for_generated::RustAutoOpaqueInner<Timeline>>
    abstract class Timeline implements RustOpaqueInterface {}

    // Rust type: RustOpaqueMoi<flutter_rust_bridge::for_generated::RustAutoOpaqueInner<TrackKind>>
    abstract class TrackKind implements RustOpaqueInterface {}
    ```
  - `Timeline` has **zero** fields, **zero** getters, and **zero** methods.
  - `Track` and `Clip` are **not generated at all** in Dart.
  - `addClipToTrack` requires `required UuidValue trackId`, but Dart has no way to read track IDs from `Timeline`.
- **`apps/aether_app/lib/src/bridge/frb_generated.dart`**:
  - Lines 425-430:
    ```dart
    void sse_encode_Auto_Owned_RustOpaque_flutter_rust_bridgefor_generatedRustAutoOpaqueInnerTimeline(
        Timeline self, SseSerializer serializer) {
      sse_encode_usize(
          (self as TimelineImpl).frbInternalSseEncode(move: true), serializer);
    }
    ```
  - Uses `move: true`, which transfers ownership and disposes the Dart handle on invocation.
- **`Makefile`**:
  - Line 1: `.PHONY: all setup bridge`
  - Defines `setup:` and `bridge:`. Lacks an `all:` rule. Executing bare `make` invokes `setup:`.

---

## 2. Logic Chain

1. **Integrity Check**:
   - We verified that `crates/aether_core/src/timeline.rs` contains no hardcoded test outputs, no facade stubs, and no bypassed logic. The PTS calculation, vector mutations, error types, and 11 unit tests are genuine, robust, and mathematically sound.
2. **Interface Breakdown at the FFI Boundary**:
   - In `PROJECT.md` Section 2: *"FFI Bridge Tier (`aether_bridge`): Exposes native core capabilities to Dart using `flutter_rust_bridge` (FRB v2). Provides value-based, stateless API endpoints (`create_timeline`, `add_clip_to_track`) that serialize domain models cleanly across the native boundary."*
   - In `PROJECT.md` Section `aether_app Riverpod ↔ UI`: `TimelineState` requires fields `timeline: Timeline?`, `totalClipCount: int`, `durationPts: int`, `tracks: List<Track>`.
   - In `ORIGINAL_REQUEST.md` Acceptance Criteria: *"(Verificação por Agente Juiz) O código Dart da tela de Timeline deve comprovadamente ler a lista de faixas/clipes recebida do Rust via FFI e exibir a quantidade correta em tela."*
   - However, because `crates/aether_bridge/src/api.rs` simply re-exports external types via `pub use aether_core::timeline::{Clip, Rational, Timeline, TrackKind};` without FRB v2 mirror annotations (`#[flutter_rust_bridge::frb(mirror(...))]`), FRB v2 auto-infers `Timeline` and `TrackKind` as opaque pointers (`RustOpaqueMoi<RustAutoOpaqueInner<...>>`).
   - Consequently:
     - Dart cannot access `timeline.durationPts` or `timeline.tracks`.
     - `Track` and `Clip` classes do not exist in Dart.
     - Dart has no way to discover the `track_id` generated during `create_timeline()`.
     - Dart cannot call `addClipToTrack` without guessing a UUID, which causes Rust to return `TrackNotFound`.
     - M2 cannot implement `TimelineState` or `TimelineView` with the required clip count and track data, causing Acceptance Criterion 3 to fail.
3. **Adversarial Error Handling & Move Semantics**:
   - Because `Timeline` is an opaque type passed by value (`mut timeline: Timeline`), FRB generates `move: true` in Dart serialization.
   - If `add_clip_to_track` fails (e.g. `InvalidClipBounds` or `TrackNotFound`), Rust drops `timeline` and returns `Err(String)`.
   - In Dart, the opaque pointer was already marked as moved. If Dart's Riverpod notifier catches the error and retains `state.timeline`, any future call using that instance will fail due to a disposed/invalid native pointer.
   - Mirroring domain models as transparent value types solves this entirely: structs are serialized by value, preserving the caller's immutable state in Dart upon failure.

---

## 3. Caveats

- The core Rust logic in `crates/aether_core/src/timeline.rs` is high quality and completely ready.
- Homebrew environment and Cargo toolchain setup performed by worker was verified and functional.
- The requested changes are strictly localized to `crates/aether_bridge/src/api.rs` (adding mirror declarations for FRB v2) and regenerating bindings with `make bridge`, along with a minor Makefile target fix.

---

## 4. Conclusion & Findings

### Findings

#### [Critical Finding 1] FFI Bridge generates Opaque `Timeline` and omits `Track`/`Clip` models, breaking Riverpod contract and Acceptance Criterion 3
- **Where**: `crates/aether_bridge/src/api.rs` (lines 1-35) & `apps/aether_app/lib/src/bridge/api.dart` (lines 34-39).
- **Why**: External types from `aether_core` are treated as opaque pointers by FRB v2 unless mirrored with `#[flutter_rust_bridge::frb(mirror(...))]`. Dart cannot read `durationPts`, `tracks`, or `totalClipCount`, and cannot obtain `track_id` to pass to `addClipToTrack`.
- **Suggestion**:
  In `crates/aether_bridge/src/api.rs`:
  Add mirror placeholder structs for all domain models:
  ```rust
  pub use aether_core::timeline::{Clip, Rational, Timeline, Track, TrackKind};

  #[flutter_rust_bridge::frb(mirror(Rational))]
  pub struct _Rational {
      pub num: i32,
      pub den: i32,
  }

  #[flutter_rust_bridge::frb(mirror(TrackKind))]
  pub enum _TrackKind {
      Video,
      Audio,
      Overlay,
  }

  #[flutter_rust_bridge::frb(mirror(Clip))]
  pub struct _Clip {
      pub id: Uuid,
      pub source_id: Uuid,
      pub source_in: i64,
      pub source_out: i64,
      pub timeline_in: i64,
      pub timeline_out: i64,
  }

  #[flutter_rust_bridge::frb(mirror(Track))]
  pub struct _Track {
      pub id: Uuid,
      pub kind: TrackKind,
      pub clips: Vec<Clip>,
  }

  #[flutter_rust_bridge::frb(mirror(Timeline))]
  pub struct _Timeline {
      pub id: Uuid,
      pub timebase: Rational,
      pub duration_pts: i64,
      pub tracks: Vec<Track>,
  }
  ```
  Then execute `make bridge`. This generates proper Dart classes with accessible fields `id`, `timebase`, `durationPts`, `tracks` (`List<Track>`), and `clips` (`List<Clip>`).

#### [Major Finding 2] Opaque Pointer Invalidation on FFI Error
- **Where**: `crates/aether_bridge/src/api.rs` (lines 21-34) & `apps/aether_app/lib/src/bridge/frb_generated.dart` (lines 425-430).
- **Why**: Taking `mut timeline: Timeline` by value on an opaque pointer consumes the Dart wrapper (`move: true`). On error (`Err(String)`), the native struct is dropped, leaving Dart with a disposed handle.
- **Suggestion**: Resolved automatically when Finding 1 is addressed, as mirrored value structs do not carry native pointer lifecycle constraints.

#### [Minor Finding 3] Makefile missing explicit `all` default target
- **Where**: `Makefile` (line 1).
- **Why**: `all` is declared in `.PHONY`, but no `all:` target is defined, making `setup:` the default target when running `make`.
- **Suggestion**: Add `all: bridge` above `setup:` in `Makefile`.

---

## 5. Verification Method

To verify the remediated solution:
1. Apply the mirror declarations in `crates/aether_bridge/src/api.rs`.
2. Run `make bridge`.
3. Inspect `apps/aether_app/lib/src/bridge/api.dart` and verify:
   - `Timeline` is a class with `final PlatformInt64 durationPts` (or `int`), `final List<Track> tracks`.
   - `Track` is a class with `final UuidValue id`, `final TrackKind kind`, `final List<Clip> clips`.
   - `TrackKind` is an enum (`TrackKind.video`, `TrackKind.audio`, `TrackKind.overlay`).
4. Run:
   ```bash
   cargo test -p aether_core
   cargo check -p aether_bridge
   flutter analyze apps/aether_app
   ```
   *Expected outcome*: All tests pass, zero compile errors, and zero static analysis warnings.
