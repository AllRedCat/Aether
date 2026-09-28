# Investigation & Technical Remediation Report: FRB v2 Mirroring and 64-Bit Integer Mapping

**Milestone**: M1 Iteration 2 (Native Engine & Bridge Remediation)  
**Author**: Explorer 1 (`explorer_m1_r2_1`)  
**Target Recipient**: Worker M1 & Orchestrator  
**Status**: COMPLETE  

---

## 1. Executive Summary & Root Cause Analysis

During Milestone M1 Gate 1 review, Reviewers 1 & 2 and Challenger 2 rejected the M1 deliverables with `REQUEST_CHANGES`. While the native core logic in `crates/aether_core` passed all unit tests and dependency issues in `crates/aether_bridge/Cargo.toml` were resolved, the FFI bridge interface exhibited a critical architectural defect:

1. **Opaque Type Encapsulation**: Because `crates/aether_bridge/src/api.rs` simply imported types from `aether_core` via `pub use aether_core::timeline::{Clip, Rational, Timeline, TrackKind};` without declaring `flutter_rust_bridge` mirror attributes, FRB v2 treated external types as opaque Rust pointers (`RustOpaqueMoi<RustAutoOpaqueInner<Timeline>>`).
2. **Missing Dart Classes & Field Getters**: In the generated Dart code (`apps/aether_app/lib/src/bridge/api.dart`), `Timeline` and `TrackKind` were emitted as abstract classes implementing `RustOpaqueInterface` with **zero fields, zero getters, and zero methods**. `Track`, `Clip`, and `Rational` were **omitted entirely**.
3. **Missing Re-export of `Track`**: `Track` was not included in `api.rs` re-exports, preventing codegen from analyzing track structures.
4. **Move Semantics on Opaque Handles**: Passing `mut timeline: Timeline` across FFI marked the native pointer as moved (`move: true`). On an error (such as `TimelineError::TrackNotFound`), the native pointer was dropped, rendering the Dart instance permanently unusable.
5. **64-bit Integer Mapping**: Without `--type-64bit-int`, FRB v2 translated 64-bit timestamps to `PlatformInt64` instead of idiomatic Dart `int`, violating the interface contracts in `PROJECT.md`.

As a direct consequence, downstream Milestone M2 was completely blocked: Dart could not read `timeline.tracks`, could not inspect track UUIDs to call `addClipToTrack`, could not read `timeline.durationPts`, and could not fulfill Acceptance Criterion AC5 / AC33.

---

## 2. Investigation of FRB v2 Mirror Syntax

### 2.1 The FRB v2 External Type Mirror Mechanism

In `flutter_rust_bridge` v2, types residing in external workspace crates (like `aether_core`) cannot have FRB macro attributes injected directly into their source definitions by the bridge generator. By default, FRB wraps external types into `RustAutoOpaqueInner<T>`.

To serialize domain types transparently across the FFI boundary as plain Dart data classes, FRB v2 provides the **Mirroring** feature (`#[flutter_rust_bridge::frb(mirror(...))]`).
Mirroring requires two strict conditions:
1. **Public Re-export**: The target type must be publicly re-exported in the module processed by `flutter_rust_bridge_codegen` (`pub use aether_core::timeline::{...};`).
2. **Structural Placeholder**: A dummy struct or enum matching the fields and types of the target struct/enum must be declared in the bridge module and annotated with `#[frb(mirror(TargetType))]`.

At codegen time, FRB's AST parser inspects the mirror definition to generate matching Dart classes with public constructor parameters, field accessors, equality operators, and SSE/DCO serialization codecs. At compile time, the Rust compiler verifies that the placeholder definition matches the real struct.

### 2.2 Exact Mirror Definitions Required

The domain model in `crates/aether_core/src/timeline.rs` consists of 5 core types:
1. `Rational` (struct: `num: i32`, `den: i32`)
2. `TrackKind` (enum: `Video`, `Audio`, `Overlay`)
3. `Clip` (struct: `id: Uuid`, `source_id: Uuid`, `source_in: i64`, `source_out: i64`, `timeline_in: i64`, `timeline_out: i64`)
4. `Track` (struct: `id: Uuid`, `kind: TrackKind`, `clips: Vec<Clip>`)
5. `Timeline` (struct: `id: Uuid`, `timebase: Rational`, `duration_pts: i64`, `tracks: Vec<Track>`)

#### Precise Mirror Declarations for `crates/aether_bridge/src/api.rs`:

```rust
use flutter_rust_bridge::frb;
pub use aether_core::timeline::{Clip, Rational, Timeline, Track, TrackKind};
use uuid::Uuid;

#[allow(dead_code)]
#[frb(mirror(Rational))]
pub struct _Rational {
    pub num: i32,
    pub den: i32,
}

#[allow(dead_code)]
#[frb(mirror(TrackKind))]
pub enum _TrackKind {
    Video,
    Audio,
    Overlay,
}

#[allow(dead_code)]
#[frb(mirror(Clip))]
pub struct _Clip {
    pub id: Uuid,
    pub source_id: Uuid,
    pub source_in: i64,
    pub source_out: i64,
    pub timeline_in: i64,
    pub timeline_out: i64,
}

#[allow(dead_code)]
#[frb(mirror(Track))]
pub struct _Track {
    pub id: Uuid,
    pub kind: TrackKind,
    pub clips: Vec<Clip>,
}

#[allow(dead_code)]
#[frb(mirror(Timeline))]
pub struct _Timeline {
    pub id: Uuid,
    pub timebase: Rational,
    pub duration_pts: i64,
    pub tracks: Vec<Track>,
}
```

*Note on `#[allow(dead_code)]`*: Prefixing placeholders with `_` and adding `#[allow(dead_code)]` guarantees that `cargo check` and `cargo test` emit zero lint warnings during Rust compilation.

---

## 3. Evaluation of `--type-64bit-int` in `Makefile`

### 3.1 Comparison: `PlatformInt64` vs Dart `int`

By default, `flutter_rust_bridge_codegen` translates Rust `i64` and `u64` to `PlatformInt64` to preserve compatibility with JavaScript 53-bit integer limits on the web.
However:
1. **Target Architecture**: Aether is primarily a high-performance desktop Non-Linear Editor (macOS, Linux, Windows), where the 64-bit Dart VM natively represents `int` as a signed 64-bit two's complement integer (`-2^63` to `2^63 - 1`).
2. **Interface Specification Alignment**: In `PROJECT.md`, the interface contract specifies:
   - `add_clip_to_track({required Timeline timeline, required UuidValue trackId, required UuidValue sourceId, required int sourceIn, required int sourceOut, required int timelineIn}) -> Future<Timeline>`
   - `TimelineState`: `durationPts: int`, `totalClipCount: int`
3. **Developer Experience in Dart**: With `PlatformInt64`, UI widgets and Riverpod state require tedious wrapping (`PlatformInt64(120)`) or conversions. With `--type-64bit-int`, Dart uses native `int`, enabling direct math, formatting, and clean code.

### 3.2 Recommendation for `Makefile`

Adding `--type-64bit-int` to the `flutter_rust_bridge_codegen generate` invocation is **strictly appropriate, safe, and directly fulfills the project requirements**.

Furthermore, to address Reviewer 1's Minor Finding 3 (lack of default `all` target causing `make` to execute `setup:`), `all: bridge` should be added as the default rule.

Updated `Makefile`:
```makefile
.PHONY: all setup bridge

all: bridge

setup:
	cargo install flutter_rust_bridge_codegen --version 2.3.0

bridge:
	flutter_rust_bridge_codegen generate --type-64bit-int --rust-root crates/aether_bridge --rust-input crate::api --dart-root apps/aether_app --dart-output apps/aether_app/lib/src/bridge
```

---

## 4. Downstream Impact and Verification

### 4.1 Generated Dart Output Structure
When `make bridge` is run with the mirror definitions and `--type-64bit-int`, FRB v2 generates:

```dart
class Timeline {
  final UuidValue id;
  final Rational timebase;
  final int durationPts;
  final List<Track> tracks;
  const Timeline({
    required this.id,
    required this.timebase,
    required this.durationPts,
    required this.tracks,
  });
  // ... operators, copyWith, hash
}

class Track {
  final UuidValue id;
  final TrackKind kind;
  final List<Clip> clips;
  const Track({
    required this.id,
    required this.kind,
    required this.clips,
  });
}

class Clip {
  final UuidValue id;
  final UuidValue sourceId;
  final int sourceIn;
  final int sourceOut;
  final int timelineIn;
  final int timelineOut;
  const Clip({
    required this.id,
    required this.sourceId,
    required this.sourceIn,
    required this.sourceOut,
    required this.timelineIn,
    required this.timelineOut,
  });
}

enum TrackKind {
  video,
  audio,
  overlay,
}

class Rational {
  final int num;
  final int den;
  const Rational({
    required this.num,
    required this.den,
  });
}
```

### 4.2 Resolution of Downstream Milestones
- **Field Access**: Dart can immediately access `timeline.tracks`, `timeline.durationPts`, and `track.clips`.
- **Track ID Discovery**: When `createTimeline()` returns, Dart can inspect `timeline.tracks.first.id` and pass that valid UUID to `addClipToTrack`.
- **Move Semantics Disarmed**: Because domain models are now value structs, Dart retains immutable copies of `Timeline`. If an FFI operation fails, the original Dart object remains intact and accessible.

---

## 5. Defensive Hardening: `recalculate_duration`

Reviewer 2 identified an edge case where unexpected negative timestamps could theoretically yield a negative `duration_pts`. Clamping `duration_pts` to `.max(0)` guarantees that timeline duration is always non-negative:

In `crates/aether_core/src/timeline.rs`:
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

## 6. Actionable Implementation Instructions for Worker M1

Worker M1 must apply the following exact modifications:

### Step 1: Update `crates/aether_bridge/src/api.rs`
Replace the entire file with:

```rust
use flutter_rust_bridge::frb;
pub use aether_core::timeline::{Clip, Rational, Timeline, Track, TrackKind};
use uuid::Uuid;

#[allow(dead_code)]
#[frb(mirror(Rational))]
pub struct _Rational {
    pub num: i32,
    pub den: i32,
}

#[allow(dead_code)]
#[frb(mirror(TrackKind))]
pub enum _TrackKind {
    Video,
    Audio,
    Overlay,
}

#[allow(dead_code)]
#[frb(mirror(Clip))]
pub struct _Clip {
    pub id: Uuid,
    pub source_id: Uuid,
    pub source_in: i64,
    pub source_out: i64,
    pub timeline_in: i64,
    pub timeline_out: i64,
}

#[allow(dead_code)]
#[frb(mirror(Track))]
pub struct _Track {
    pub id: Uuid,
    pub kind: TrackKind,
    pub clips: Vec<Clip>,
}

#[allow(dead_code)]
#[frb(mirror(Timeline))]
pub struct _Timeline {
    pub id: Uuid,
    pub timebase: Rational,
    pub duration_pts: i64,
    pub tracks: Vec<Track>,
}

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

### Step 2: Update `Makefile`
Update `Makefile` to include `all: bridge` and `--type-64bit-int`:

```makefile
.PHONY: all setup bridge

all: bridge

setup:
	cargo install flutter_rust_bridge_codegen --version 2.3.0

bridge:
	flutter_rust_bridge_codegen generate --type-64bit-int --rust-root crates/aether_bridge --rust-input crate::api --dart-root apps/aether_app --dart-output apps/aether_app/lib/src/bridge
```

### Step 3: (Defensive) Update `crates/aether_core/src/timeline.rs`
Update `recalculate_duration` around line 166:
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

### Step 4: Execute Codegen and Verification Commands
Run the full verification suite:
```bash
make bridge
cargo check -p aether_bridge
cargo test -p aether_core
cargo test --workspace
flutter analyze apps/aether_app
python3 tests/test_bridge_contract.py
python3 tests/test_rust_core.py
python3 tests/test_challenger_adversarial.py
```

All commands must exit with code `0`.
