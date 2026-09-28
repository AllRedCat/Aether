# Challenger 2 Handoff Report: Milestone M1 (Native Engine & Bridge)

**Verdict**: **REQUEST_CHANGES**

---

## 1. Observation

### Observation 1.1: Rust Core Test Execution
- Executed: `cargo test -p aether_core`
- Output:
  ```
  running 11 tests
  test timeline::tests::test_add_clip_to_track_and_recalculate_duration ... ok
  test timeline::tests::test_add_clip_track_not_found ... ok
  test timeline::tests::test_invalid_source_bounds ... ok
  test timeline::tests::test_invalid_clip_bounds ... ok
  test timeline::tests::test_new_with_default_tracks ... ok
  test timeline::tests::test_add_multiple_clips_recalculates_max_pts ... ok
  test timeline::tests::test_recalculate_duration_empty_tracks ... ok
  test timeline::tests::test_staggered_clips_with_gaps_across_tracks ... ok
  test timeline::tests::test_track_with_id_and_duration ... ok
  test timeline::tests::test_timeline_error_display ... ok
  test timeline::tests::test_zero_duration_clip ... ok

  test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
  ```
- Exit code: `0`.
- Mathematical and arithmetic boundary behavior in `crates/aether_core/src/timeline.rs`:
  - `Clip::new` uses `saturating_sub` and `saturating_add`, preventing panic under integer overflow.
  - `Timeline::add_clip` validates `source_in <= source_out` and `timeline_in <= timeline_out`.
  - `Timeline::recalculate_duration` computes `max(timeline_out)` across all tracks, correctly handling empty timelines with `unwrap_or(0)`.

### Observation 1.2: Bridge Cargo Compilation & Makefile Codegen
- Executed: `cargo check -p aether_bridge`
  - Exit code: `0`.
- Executed: `make bridge`
  - Exit code: `0`. Generated files in `apps/aether_app/lib/src/bridge/` and `crates/aether_bridge/src/frb_generated.rs`.
- Executed Python suites:
  - `python3 tests/test_rust_core.py`: Exit code `0` (PASS).
  - `python3 tests/test_bridge_contract.py`: Exit code `0` (PASS).
  - `python3 tests/test_adversarial_scenarios.py`: Exit code `0` (PASS).

### Observation 1.3: FFI Boundary Generates Opaque Types Without Field Access (CRITICAL)
- File inspected: `apps/aether_app/lib/src/bridge/api.dart` (lines 34-39 verbatim):
  ```dart
  // Rust type: RustOpaqueMoi<flutter_rust_bridge::for_generated::RustAutoOpaqueInner<Timeline>>
  abstract class Timeline implements RustOpaqueInterface {}

  // Rust type: RustOpaqueMoi<flutter_rust_bridge::for_generated::RustAutoOpaqueInner<TrackKind>>
  abstract class TrackKind implements RustOpaqueInterface {}
  ```
- In `crates/aether_bridge/src/api.rs`, types are imported via `pub use aether_core::timeline::{Clip, Rational, Timeline, TrackKind};` without mirroring or field mapping.
- Because `Timeline` originates from an external crate (`aether_core`), `flutter_rust_bridge` v2 automatically wraps it in `RustAutoOpaqueInner<Timeline>`.
- In `apps/aether_app/lib/src/bridge/api.dart`, `Timeline` has **zero public fields, zero getters, and zero methods**. `Track` and `Clip` are not exposed to Dart at all.

### Observation 1.4: Empirical Failure in Dart Field Access
- Tested field access on `Timeline` in Dart:
  ```dart
  import 'package:aether_app/src/bridge/api.dart';

  void test(Timeline t) {
    print(t.durationPts);
    print(t.tracks);
  }
  ```
- Executed `dart analyze` on this usage:
  ```
  error - The getter 'durationPts' isn't defined for the type 'Timeline'. - undefined_getter
  error - The getter 'tracks' isn't defined for the type 'Timeline'. - undefined_getter
  2 issues found.
  ```
  Exit code: `3`.

### Observation 1.5: Breakdown of Clip Addition Workflow
- `create_timeline()` in `crates/aether_bridge/src/api.rs` (lines 10-14):
  ```rust
  pub fn create_timeline() -> Timeline {
      let mut timeline = Timeline::new(Rational { num: 60, den: 1 });
      timeline.add_track(TrackKind::Video);
      timeline
  }
  ```
- The initial Video track receives a randomly generated `Uuid::new_v4()`.
- Because `Timeline` is opaque in Dart, Dart cannot read `timeline.tracks` or query the UUID of that track.
- When calling `addClipToTrack`:
  ```dart
  Future<Timeline> addClipToTrack({
      required Timeline timeline,
      required UuidValue trackId,
      ...
  })
  ```
  Dart has **no valid `trackId`** to provide. Passing any dummy or random UUID causes `Timeline::add_clip` to return `Err(TimelineError::TrackNotFound)`, failing the entire user workflow.

### Observation 1.6: 64-bit Integer FFI Mapping
- In `apps/aether_app/lib/src/bridge/api.dart` lines 23-25:
  ```dart
  required PlatformInt64 sourceIn,
  required PlatformInt64 sourceOut,
  required PlatformInt64 timelineIn
  ```
  The bridge parameters use `PlatformInt64` rather than Dart `int` because `--type-64bit-int` was omitted from the `make bridge` command in `Makefile`.

---

## 2. Logic Chain

1. **Acceptance Criteria & Specification Violation**:
   - `ORIGINAL_REQUEST.md` requires:
     > "(Verificação por Agente Juiz) O código Dart da tela de Timeline deve comprovadamente ler a lista de faixas/clipes recebida do Rust via FFI e exibir a quantidade correta em tela."
   - `PROJECT.md` specifies the state model:
     > `TimelineState`: `timeline: Timeline?`, `totalClipCount: int`, `durationPts: int`, `tracks: List<Track>`
   - Based on Observation 1.3 and 1.4, `Timeline` in Dart is an opaque pointer (`RustOpaqueInterface`) containing zero properties. Dart cannot read `durationPts`, cannot read `tracks`, and cannot read `clips`.
   - Therefore, the bridge in its current form cannot satisfy the core acceptance criteria.

2. **Functional Block on Milestone M2**:
   - Milestone M2 requires a Riverpod provider to consume the Timeline state and invoke `addClipToTrack`.
   - Per Observation 1.5, Dart cannot access the UUID of the tracks created inside Rust. Dart cannot supply a valid `trackId` to `addClipToTrack`. Any attempt to call `addClipToTrack` results in `TrackNotFound`.
   - Therefore, the implementation blocks M2 progress and is incomplete across the FFI boundary.

3. **Empirical Proof of Resolution via Mirroring**:
   - In a controlled test harness, adding `#[flutter_rust_bridge::frb(mirror(...))]` structs for `Rational`, `TrackKind`, `Clip`, `Track`, and `Timeline` in `crates/aether_bridge/src/api.rs`, combined with `--type-64bit-int` in `Makefile`, resulted in `make bridge` generating:
     ```dart
     class Timeline {
       final UuidValue id;
       final Rational timebase;
       final int durationPts;
       final List<Track> tracks;
       ...
     }
     class Track {
       final UuidValue id;
       final TrackKind kind;
       final List<Clip> clips;
     }
     class Clip {
       final UuidValue id;
       final UuidValue sourceId;
       final int sourceIn;
       final int sourceOut;
       final int timelineIn;
       final int timelineOut;
     }
     ```
   - All properties (`durationPts`, `tracks`, `clips`, `id`) become fully accessible to Dart, `dart analyze` passes with 0 errors, and Dart can inspect `timeline.tracks.first.id` to call `addClipToTrack`.

---

## 3. Caveats

- **No Caveats**. The empirical evidence was demonstrated both on the current generated bridge files and verified using the `flutter_rust_bridge_codegen` compiler.

---

## 4. Conclusion

**Verdict: REQUEST_CHANGES**

The native core logic (`aether_core`) is well-designed, mathematically sound, and passes all 11 unit tests. However, the FFI bridge (`aether_bridge`) suffers from a critical serialization defect: domain types are not mirrored, causing `flutter_rust_bridge` to generate opaque handles rather than concrete Dart data models. This breaks downstream Dart consumer compatibility, violates Acceptance Criterion AC5, and makes adding clips impossible from the UI.

### Actionable Required Changes for Worker M1:

1. **Update `crates/aether_bridge/src/api.rs`**:
   Import `flutter_rust_bridge::frb` and declare mirror structs for `Rational`, `TrackKind`, `Clip`, `Track`, and `Timeline`:
   ```rust
   use flutter_rust_bridge::frb;
   pub use aether_core::timeline::{Clip, Rational, Timeline, Track, TrackKind};
   use uuid::Uuid;

   #[frb(mirror(Rational))]
   pub struct _Rational {
       pub num: i32,
       pub den: i32,
   }

   #[frb(mirror(TrackKind))]
   pub enum _TrackKind {
       Video,
       Audio,
       Overlay,
   }

   #[frb(mirror(Clip))]
   pub struct _Clip {
       pub id: Uuid,
       pub source_id: Uuid,
       pub source_in: i64,
       pub source_out: i64,
       pub timeline_in: i64,
       pub timeline_out: i64,
   }

   #[frb(mirror(Track))]
   pub struct _Track {
       pub id: Uuid,
       pub kind: TrackKind,
       pub clips: Vec<Clip>,
   }

   #[frb(mirror(Timeline))]
   pub struct _Timeline {
       pub id: Uuid,
       pub timebase: Rational,
       pub duration_pts: i64,
       pub tracks: Vec<Track>,
   }
   ```

2. **Update `Makefile`**:
   Add `--type-64bit-int` to the `bridge` target so that 64-bit timestamps map cleanly to native Dart `int`:
   ```makefile
   bridge:
   	flutter_rust_bridge_codegen generate --type-64bit-int --rust-root crates/aether_bridge --rust-input crate::api --dart-root apps/aether_app --dart-output apps/aether_app/lib/src/bridge
   ```

3. **Re-run `make bridge`**:
   Regenerate bridge bindings so `Timeline`, `Track`, `Clip`, `Rational`, and `TrackKind` are emitted as full Dart classes with public fields.

---

## 5. Verification Method

To verify the fix once implemented:
1. Run `cargo test -p aether_core` (Expect: 11 passed, exit code 0).
2. Run `cargo check -p aether_bridge` (Expect: compiles cleanly, exit code 0).
3. Run `make bridge` (Expect: generates concrete Dart classes `Timeline`, `Track`, `Clip` in `apps/aether_app/lib/src/bridge/api.dart`).
4. Verify Dart field access:
   ```bash
   python3 -c '
   from pathlib import Path
   content = Path("apps/aether_app/lib/src/bridge/api.dart").read_text()
   assert "class Timeline {" in content, "Timeline must be a concrete class"
   assert "final List<Track> tracks;" in content, "Timeline must expose tracks"
   assert "final int durationPts;" in content, "Timeline must expose durationPts"
   assert "class Track {" in content, "Track must be a concrete class"
   assert "class Clip {" in content, "Clip must be a concrete class"
   print("Verification PASSED: Domain models correctly mirrored to Dart.")
   '
   ```
5. Run `flutter analyze apps/aether_app` (Expect: 0 issues found).
