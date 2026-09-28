# Detailed Investigation Report: Dart-side Code Generation & FRB v2 Mirroring Remediation

**Explorer**: Explorer 2 (Milestone M1 Iteration 2 — Native Engine & Bridge Remediation)  
**Date**: 2026-09-28T03:45:00Z  
**Objective**: Investigate Dart-side code generation requirements, verify how flutter_rust_bridge v2 mirror declarations produce concrete Dart classes (`Timeline`, `Track`, `Clip`, `TrackKind`, `Rational`) with accessible fields, uncover hidden dependencies, and specify exact remediation instructions for Worker M1.

---

## 1. Executive Summary

During Milestone M1 Iteration 1, Gate 1 failed (REQUEST_CHANGES issued by Reviewers 1 & 2 and Challenger 2) because `crates/aether_bridge/src/api.rs` used a naive `pub use aether_core::timeline::{Clip, Rational, Timeline, TrackKind};` without FRB v2 mirror annotations (`#[flutter_rust_bridge::frb(mirror(...))]`) and omitted re-exporting `Track`.

Consequently:
1. `flutter_rust_bridge_codegen 2.3.0` generated `Timeline` and `TrackKind` as opaque pointers (`abstract class Timeline implements RustOpaqueInterface {}`) with **zero accessible fields or methods**.
2. Classes `Track`, `Clip`, and `Rational` were **never generated in Dart at all**.
3. Dart code could not access `timeline.tracks`, `track.clips`, `clip.timelineIn`, or `timeline.durationPts`.
4. Dart could not extract track IDs to invoke `addClipToTrack(...)`, rendering UI clip addition impossible and violating Acceptance Criterion AC5.

### Key Breakthrough Discovery: The Hidden `uuid` Feature Flag in `flutter_rust_bridge`
During empirical testing of the proposed mirror declarations, this investigation discovered a **critical compiler blocker that neither Reviewers 1 & 2 nor Challenger 2 foresaw**:
- When `Uuid` appears as a struct field in mirrored types (`Timeline.id`, `Track.id`, `Clip.id`, `Clip.source_id`), FRB v2 codegen emits `self.0.id.into_into_dart().into_dart()` in `crates/aether_bridge/src/frb_generated.rs`.
- The trait implementation `impl_into_into_dart_by_self!(uuid::Uuid)` inside `flutter_rust_bridge` is strictly gated behind `#[cfg(feature = "uuid")]`.
- Currently, `crates/aether_bridge/Cargo.toml` declares `flutter_rust_bridge = "=2.3.0"` **without the `"uuid"` feature**.
- If Worker M1 applies only the mirror structs in `api.rs`, running `cargo check -p aether_bridge` will fail with:
  ```
  error[E0599]: no method named `into_into_dart` found for struct `Uuid` in the current scope
     --> crates/aether_bridge/src/frb_generated.rs:425:23
      |
  425 |             self.0.id.into_into_dart().into_dart(),
      |                       ^^^^^^^^^^^^^^ method not found in `Uuid`
  ```
- **Remediation**: `crates/aether_bridge/Cargo.toml` **must** declare:
  ```toml
  flutter_rust_bridge = { version = "=2.3.0", features = ["uuid"] }
  ```

---

## 2. Analysis of Dart-Side Code Generation Requirements

### 2.1 Domain Model Requirements in Dart

Per `PROJECT.md` (Lines 87-91) and `ORIGINAL_REQUEST.md` (Requirement R2 & AC5):
- The Flutter application state tier (`TimelineState`) requires:
  - `timeline: Timeline?`
  - `totalClipCount: int`
  - `durationPts: int`
  - `tracks: List<Track>`
- The UI (`TimelineView`) requires direct access to:
  - `timeline.tracks` -> list of tracks
  - `track.clips` -> list of clips per track
  - `clip.timelineIn` and `clip.timelineOut` -> PTS positions
  - `timeline.durationPts` -> overall timeline length
  - `track.id` -> UUID to pass into `addClipToTrack(timeline: timeline, trackId: track.id, ...)`

### 2.2 Comparison: Current vs. Required Dart Output

| Element | Current Output (`apps/aether_app/lib/src/bridge/api.dart`) | Required Output with FRB v2 Mirrors & `--type-64bit-int` |
|---|---|---|
| `Timeline` | `abstract class Timeline implements RustOpaqueInterface {}` (0 fields) | `class Timeline { final UuidValue id; final Rational timebase; final int durationPts; final List<Track> tracks; }` |
| `Track` | **Not generated** | `class Track { final UuidValue id; final TrackKind kind; final List<Clip> clips; }` |
| `Clip` | **Not generated** | `class Clip { final UuidValue id; final UuidValue sourceId; final int sourceIn; final int sourceOut; final int timelineIn; final int timelineOut; }` |
| `TrackKind` | `abstract class TrackKind implements RustOpaqueInterface {}` | `enum TrackKind { video, audio, overlay; }` |
| `Rational` | **Not generated** | `class Rational { final int num; final int den; }` |
| 64-bit integer types | `PlatformInt64` | `int` (clean Dart native integer) |
| Value equality | Reference identity only | Built-in `==` and `hashCode` overrides |

### 2.3 The 64-Bit Integer Mapping (`--type-64bit-int`)

Without `--type-64bit-int`, `i64` in Rust translates to `PlatformInt64` in Dart. This forces Dart consumers to convert between `PlatformInt64` and `int`, breaking simple property accesses like `clip.timelineIn` in UI rendering.
Adding `--type-64bit-int` to `flutter_rust_bridge_codegen generate` in `Makefile` translates all `i64` to Dart `int`, which is natively 64-bit on Flutter desktop and mobile targets.

---

## 3. Empirical Verification of Mirror Declarations

In an isolated verification sandbox, we tested the proposed mirror configuration against `flutter_rust_bridge_codegen 2.3.0`.

### 3.1 Tested Configuration
1. **`crates/aether_bridge/Cargo.toml`**:
   ```toml
   [package]
   name = "aether_bridge"
   version = "0.1.0"
   edition = "2021"

   [dependencies]
   flutter_rust_bridge = { version = "=2.3.0", features = ["uuid"] }
   uuid = { version = "1.10", features = ["v4"] }
   aether_core = { path = "../aether_core" }
   aether_render = { path = "../aether_render" }
   aether_media = { path = "../aether_media" }

   [lib]
   crate-type = ["cdylib", "staticlib"]
   ```

2. **`crates/aether_bridge/src/api.rs`**:
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

3. **`Makefile`**:
   ```makefile
   .PHONY: all setup bridge

   all: bridge

   setup:
   	cargo install flutter_rust_bridge_codegen --version 2.3.0

   bridge:
   	flutter_rust_bridge_codegen generate --type-64bit-int --rust-root crates/aether_bridge --rust-input crate::api --dart-root apps/aether_app --dart-output apps/aether_app/lib/src/bridge
   ```

### 3.2 Resulting Generated Dart Code (`apps/aether_app/lib/src/bridge/api.dart`)
`flutter_rust_bridge_codegen` produced the following concrete classes:
```dart
// Generated by flutter_rust_bridge@2.3.0
import 'frb_generated.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';
import 'package:uuid/uuid.dart';

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
        required int sourceIn,
        required int sourceOut,
        required int timelineIn}) =>
    RustLib.instance.api.crateApiAddClipToTrack(
        timeline: timeline,
        trackId: trackId,
        sourceId: sourceId,
        sourceIn: sourceIn,
        sourceOut: sourceOut,
        timelineIn: timelineIn);

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

  @override
  int get hashCode =>
      id.hashCode ^
      sourceId.hashCode ^
      sourceIn.hashCode ^
      sourceOut.hashCode ^
      timelineIn.hashCode ^
      timelineOut.hashCode;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is Clip &&
          runtimeType == other.runtimeType &&
          id == other.id &&
          sourceId == other.sourceId &&
          sourceIn == other.sourceIn &&
          sourceOut == other.sourceOut &&
          timelineIn == other.timelineIn &&
          timelineOut == other.timelineOut;
}

class Rational {
  final int num;
  final int den;

  const Rational({
    required this.num,
    required this.den,
  });

  @override
  int get hashCode => num.hashCode ^ den.hashCode;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is Rational &&
          runtimeType == other.runtimeType &&
          num == other.num &&
          den == other.den;
}

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

  @override
  int get hashCode =>
      id.hashCode ^ timebase.hashCode ^ durationPts.hashCode ^ tracks.hashCode;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is Timeline &&
          runtimeType == other.runtimeType &&
          id == other.id &&
          timebase == other.timebase &&
          durationPts == other.durationPts &&
          tracks == other.tracks;
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

  @override
  int get hashCode => id.hashCode ^ kind.hashCode ^ clips.hashCode;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is Track &&
          runtimeType == other.runtimeType &&
          id == other.id &&
          kind == other.kind &&
          clips == other.clips;
}

enum TrackKind {
  video,
  audio,
  overlay,
  ;
}
```

### 3.3 Verification of Empirical Test Execution
We executed four layers of verification on the remediated structure:
1. `cargo check -p aether_bridge`: Exited with code `0`.
2. `cargo test --workspace`: Exited with code `0` across all crates.
3. `flutter analyze apps/aether_app`: Exited with code `0` ("No issues found!").
4. `flutter test` testing field access on `timeline.tracks`, `track.clips`, `clip.timelineIn`, `timeline.durationPts`: Exited with code `0` ("All tests passed!").

---

## 4. Remediation Action Plan for Worker M1

Worker M1 must perform the following actions:

### Action 1: Update `crates/aether_bridge/Cargo.toml`
Add `features = ["uuid"]` to `flutter_rust_bridge`:
```toml
[package]
name = "aether_bridge"
version = "0.1.0"
edition = "2021"

[dependencies]
flutter_rust_bridge = { version = "=2.3.0", features = ["uuid"] }
uuid = { version = "1.10", features = ["v4"] }
aether_core = { path = "../aether_core" }
aether_render = { path = "../aether_render" }
aether_media = { path = "../aether_media" }

[lib]
crate-type = ["cdylib", "staticlib"]
```

### Action 2: Update `crates/aether_bridge/src/api.rs`
Add `#[frb(mirror(...))]` structs and re-export `Track`:
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

### Action 3: Update `Makefile`
Add `all: bridge` default target and `--type-64bit-int` to `bridge`:
```makefile
.PHONY: all setup bridge

all: bridge

setup:
	cargo install flutter_rust_bridge_codegen --version 2.3.0

bridge:
	flutter_rust_bridge_codegen generate --type-64bit-int --rust-root crates/aether_bridge --rust-input crate::api --dart-root apps/aether_app --dart-output apps/aether_app/lib/src/bridge
```

### Action 4: Run `make bridge`
Regenerate bindings.

---

## 5. Verification Protocol for Worker M1

Worker M1 must execute the following commands in order and ensure all pass with exit code `0`:

1. **Codegen Generation**:
   ```bash
   make bridge
   ```
   *Expected*: `Done!` with exit code 0.

2. **Cargo Compilation Checks**:
   ```bash
   cargo check -p aether_bridge
   cargo test -p aether_core
   cargo test --workspace
   ```
   *Expected*: Exit code 0 for all three commands.

3. **Dart Static Analysis**:
   ```bash
   flutter analyze apps/aether_app
   ```
   *Expected*: `No issues found!` with exit code 0.

4. **Dart Model & Field Access Verification**:
   ```bash
   python3 -c '
   from pathlib import Path
   content = Path("apps/aether_app/lib/src/bridge/api.dart").read_text()
   assert "class Timeline {" in content, "Missing class Timeline"
   assert "class Track {" in content, "Missing class Track"
   assert "class Clip {" in content, "Missing class Clip"
   assert "enum TrackKind {" in content, "Missing enum TrackKind"
   assert "class Rational {" in content, "Missing class Rational"
   assert "final List<Track> tracks;" in content, "Missing tracks field"
   assert "final int durationPts;" in content, "Missing durationPts field"
   assert "final List<Clip> clips;" in content, "Missing clips field"
   assert "final int timelineIn;" in content, "Missing timelineIn field"
   print("SUCCESS: All Dart models and fields verified!")
   '
   ```
   *Expected*: `SUCCESS: All Dart models and fields verified!`

5. **E2E Test Runner**:
   ```bash
   python3 tests/e2e_runner.py
   ```
   *Expected*: All test suites pass.
