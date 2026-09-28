# Reviewer 2 Handoff & Adversarial Report: Milestone M1 (Native Engine & Bridge)

## Review Summary

**Verdict**: **REQUEST_CHANGES**

Milestone M1 made commendable progress on the native Rust engine in `crates/aether_core` (robust domain logic, duration recalculation, bounds checking, and 11 green unit tests) and fixed the missing `uuid` dependency in `crates/aether_bridge/Cargo.toml`.

However, an interface and serialization defect exists at the FFI bridge layer:
Because `crates/aether_bridge/src/api.rs` does not mirror external types via `#[frb(mirror(...))]` and omits re-exporting `Track`, `flutter_rust_bridge_codegen` generates `Timeline` and `TrackKind` as opaque pointers (`RustOpaqueMoi<RustAutoOpaqueInner<Timeline>>`) with **zero accessible fields or methods in Dart**, and **completely omits generating `Track`, `Clip`, and `Rational` classes**.

This directly breaks the interface contract in `PROJECT.md` and makes it impossible for Milestone M2 to satisfy Acceptance Criterion 33:
> *"O código Dart da tela de Timeline deve comprovadamente ler a lista de faixas/clipes recebida do Rust via FFI e exibir a quantidade correta em tela."*

---

## 1. Observation

1. **Independent Verification Commands Executed**:
   - `cargo test -p aether_core`:
     ```text
     running 11 tests
     test timeline::tests::test_invalid_clip_bounds ... ok
     test timeline::tests::test_add_multiple_clips_recalculates_max_pts ... ok
     test timeline::tests::test_add_clip_track_not_found ... ok
     test timeline::tests::test_add_clip_to_track_and_recalculate_duration ... ok
     test timeline::tests::test_invalid_source_bounds ... ok
     test timeline::tests::test_new_with_default_tracks ... ok
     test timeline::tests::test_recalculate_duration_empty_tracks ... ok
     test timeline::tests::test_staggered_clips_with_gaps_across_tracks ... ok
     test timeline::tests::test_timeline_error_display ... ok
     test timeline::tests::test_track_with_id_and_duration ... ok
     test timeline::tests::test_zero_duration_clip ... ok

     test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     ```
   - `cargo check -p aether_bridge`: Exited with code 0 (`Finished dev profile [unoptimized + debuginfo] target(s) in 0.06s`).
   - `make bridge`: Exited with code 0 (`Done!`).
   - `cargo test --workspace`: Exited with code 0 (all 4 workspace crates passed).
   - `flutter analyze apps/aether_app`: Exited with code 0 (`No issues found!`).

2. **Source Code Inspection of `crates/aether_bridge/src/api.rs`**:
   Lines 1-3:
   ```rust
   pub use aether_core::timeline::{Clip, Rational, Timeline, TrackKind};
   use uuid::Uuid;
   ```
   - `Track` is not re-exported.
   - No `#[frb(mirror(...))]` attributes are declared for any `aether_core` types.

3. **Generated Dart FFI Output in `apps/aether_app/lib/src/bridge/api.dart`**:
   Lines 10-39:
   ```dart
   Future<void> initEngine() => RustLib.instance.api.crateApiInitEngine();

   Future<Timeline> createTimeline() =>
       RustLib.instance.api.crateApiCreateTimeline();

   Future<Timeline> addTrack(
           {required Timeline timeline, required TrackKind kind}) =>
       RustLib.instance.api.crateApiAddTrack(timeline: timeline, kind: kind);

   Future<Timeline> addClipToTrack(
           {required Timeline timeline,
           required UuidValue trackId,
           required UuidValue sourceId,
           required PlatformInt64 sourceIn,
           required PlatformInt64 sourceOut,
           required PlatformInt64 timelineIn}) =>
       RustLib.instance.api.crateApiAddClipToTrack(
           timeline: timeline,
           trackId: trackId,
           sourceId: sourceId,
           sourceIn: sourceIn,
           sourceOut: sourceOut,
           timelineIn: timelineIn);

   // Rust type: RustOpaqueMoi<flutter_rust_bridge::for_generated::RustAutoOpaqueInner<Timeline>>
   abstract class Timeline implements RustOpaqueInterface {}

   // Rust type: RustOpaqueMoi<flutter_rust_bridge::for_generated::RustAutoOpaqueInner<TrackKind>>
   abstract class TrackKind implements RustOpaqueInterface {}
   ```
   Direct observations:
   - `Timeline` is an abstract class with zero properties or methods. There is no `timeline.tracks`, no `timeline.durationPts`, no `timeline.id`.
   - `TrackKind` is an opaque pointer with zero enum variants exposed to Dart.
   - `Track`, `Clip`, and `Rational` are not generated anywhere in Dart.

4. **Interface Contract in `PROJECT.md` & `ORIGINAL_REQUEST.md`**:
   - `PROJECT.md` Line 6: *"Provides value-based, stateless API endpoints (`create_timeline`, `add_clip_to_track`) that serialize domain models cleanly across the native boundary."*
   - `PROJECT.md` Lines 87-91:
     ```markdown
     - TimelineState:
       - timeline: Timeline?
       - totalClipCount: int
       - durationPts: int
       - tracks: List<Track>
     ```
   - `ORIGINAL_REQUEST.md` Line 33: Acceptance Criterion:
     *(Verificação por Agente Juiz) O código Dart da tela de Timeline deve comprovadamente ler a lista de faixas/clipes recebida do Rust via FFI e exibir a quantidade correta em tela.*

---

## 2. Logic Chain

1. **Premise 1**: Acceptance Criterion 33 and `PROJECT.md` Milestone M2 contract require Dart code in `apps/aether_app` to inspect the `Timeline` object received from Rust, accessing its `tracks` list (`List<Track>`), each track's `clips` list (`List<Clip>`), and `duration_pts`.
2. **Premise 2**: In `flutter_rust_bridge` v2, types defined in an external crate (`aether_core`) that are simply re-exported in `crates/aether_bridge/src/api.rs` default to `RustAutoOpaque` unless they are explicitly mirrored via `#[frb(mirror(...))]` or converted to bridge DTOs.
3. **Premise 3**: In `crates/aether_bridge/src/api.rs`, `Track` is not re-exported, and neither `Timeline`, `Track`, `Clip`, `Rational`, nor `TrackKind` have `#[frb(mirror(...))]` annotations.
4. **Premise 4**: Consequently, `make bridge` generated `Timeline` and `TrackKind` as empty opaque references (`RustOpaqueMoi`), while omitting `Track`, `Clip`, and `Rational` completely from Dart code.
5. **Conclusion**: Any Dart code in M2 attempting `timeline.tracks`, `timeline.durationPts`, or iterating over `track.clips` will immediately fail to compile. This blocks Milestone M2 and fails Acceptance Criterion 33. Because `crates/aether_bridge` is strictly within M1's file boundary, this defect must be resolved in M1 before handoff.

---

## 3. Findings

### [Critical] Finding 1: Unmirrored Domain Types Create Opaque Dart Facade & Missing Dart Classes

- **What**: `aether_bridge` does not declare FRB mirror annotations (`#[frb(mirror(...))]`) for `Timeline`, `Track`, `Clip`, `Rational`, and `TrackKind`, and omits re-exporting `Track`.
- **Where**: `crates/aether_bridge/src/api.rs:1`
- **Why**: Flutter Rust Bridge treats all five types as opaque or omits them entirely. In Dart, `Timeline` is generated with 0 fields and 0 methods, and `Track`, `Clip`, and `Rational` are non-existent. Milestone M2 cannot access `timeline.tracks`, `track.clips`, `timeline.durationPts`, or count clips in Dart.
- **Suggestion**:
  1. In `crates/aether_bridge/src/api.rs`:
     - Re-export `Track`: `pub use aether_core::timeline::{Clip, Rational, Timeline, Track, TrackKind};`
     - Import `use flutter_rust_bridge::frb;`
     - Define mirrored placeholder structs:
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
  2. Re-run `make bridge`.
  3. Verify that `apps/aether_app/lib/src/bridge/api.dart` contains concrete classes:
     `class Timeline`, `class Track`, `class Clip`, `class Rational`, `enum TrackKind` with all member fields.

---

### [Minor] Finding 2: `recalculate_duration` Potential Negative Duration on Edge-Case Clips

- **What**: In `crates/aether_core/src/timeline.rs:166-174`, `recalculate_duration` does not clamp to zero.
- **Where**: `crates/aether_core/src/timeline.rs:166-174`
- **Why**: If a clip has negative timeline positions (or edge-case calculation yielding negative `timeline_out`), `unwrap_or(0)` will not prevent `duration_pts` from becoming negative.
- **Suggestion**:
  Clamp duration to `>= 0`:
  ```rust
  pub fn recalculate_duration(&mut self) {
      self.duration_pts = self
          .tracks
          .iter()
          .flat_map(|track| track.clips.iter())
          .map(|clip| clip.timeline_out)
          .max()
          .unwrap_or(0)
          .max(0);
  }
  ```

---

## 4. Adversarial Challenges & Stress Tests

### Challenge 1: Dart FFI Inspection Stress Test (Contract Breach)
- **Assumption Challenged**: The generated FFI bindings allow Flutter UI to inspect the `Timeline` data structure and its tracks/clips.
- **Attack Scenario**: Write a Dart snippet consuming `createTimeline()`:
  ```dart
  final timeline = await createTimeline();
  final tracks = timeline.tracks;
  final duration = timeline.durationPts;
  ```
- **Blast Radius**: Build failure in Flutter (`The getter 'tracks' isn't defined for the type 'Timeline'`). M2 cannot be completed.
- **Result**: **FAIL** (Confirmed flaw).

### Challenge 2: Integrity & Cheating Audit
- **Checklist**:
  - Hardcoded test results or expected outputs embedded in source code: **None found**.
  - Dummy or facade implementations with no real logic: **None found in Rust engine; the FFI opacity is an FRB configuration gap, not intentional deception**.
  - Shortcuts bypassing core logic: **None found**.
  - Fabricated verification outputs: **None found**.
  - Self-certifying claims: **None found; commands re-executed and verified**.
- **Result**: **PASS** (No integrity violation).

---

## 5. Verified Claims

| Claim | Upstream Stated | Independently Verified | Status |
|---|---|---|---|
| `cargo test -p aether_core` | 11 passed | 11 passed in 0.00s | **PASS** |
| `cargo check -p aether_bridge` | Compiles cleanly | Exits 0 in 0.06s | **PASS** |
| `make bridge` | Generates without error | Exits 0, outputs `Done!` | **PASS (Syntactic only)** |
| `cargo test --workspace` | All passed | Exits 0 | **PASS** |
| `flutter analyze apps/aether_app` | Clean | Clean (0 issues) | **PASS** |
| FFI Serializability into Dart Models | Value-based serialization | Fails: generated as opaque pointers | **FAIL (Finding 1)** |

---

## 6. Caveats

- No caveats. The root cause and fix were verified against `flutter_rust_bridge` v2 official specifications.

---

## 7. Conclusion

Milestone M1 cannot be approved in its current state because the bridge generates opaque handles rather than translatable domain models with accessible fields, blocking Milestone M2 and violating the interface contracts.

**Verdict**: **REQUEST_CHANGES**

### Required Action Items for Worker M1:
1. In `crates/aether_bridge/src/api.rs`:
   - Add `use flutter_rust_bridge::frb;`
   - Re-export `Track`: `pub use aether_core::timeline::{Clip, Rational, Timeline, Track, TrackKind};`
   - Define `#[frb(mirror(...))]` structs for `Rational`, `TrackKind`, `Clip`, `Track`, and `Timeline`.
2. Run `make bridge` to regenerate Dart bindings in `apps/aether_app/lib/src/bridge/`.
3. Inspect `apps/aether_app/lib/src/bridge/api.dart` and confirm `class Timeline`, `class Track`, `class Clip`, `class Rational`, and `enum TrackKind` are generated with all fields accessible in Dart.
4. (Optional but recommended) In `crates/aether_core/src/timeline.rs`, clamp `duration_pts` in `recalculate_duration` with `.max(0)`.
5. Run `cargo test -p aether_core`, `cargo check -p aether_bridge`, `make bridge`, and `flutter analyze apps/aether_app`.

---

## 8. Verification Method

To verify the required fix:
1. In `crates/aether_bridge/src/api.rs`, add the mirroring blocks and re-export `Track`.
2. Run:
   ```bash
   make bridge
   ```
3. Inspect `apps/aether_app/lib/src/bridge/api.dart`:
   - Check that `class Timeline` has `final UuidValue id;`, `final Rational timebase;`, `final PlatformInt64 durationPts;`, `final List<Track> tracks;`.
   - Check that `class Track` and `class Clip` exist with their respective fields.
4. Run:
   ```bash
   cargo test -p aether_core
   cargo check -p aether_bridge
   flutter analyze apps/aether_app
   ```
   All commands must exit with 0.
