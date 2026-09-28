# Technical Specification & Survey Report: FFI Bridge, Build & Test Configuration

**Date**: 2026-09-28  
**Author**: Survey Spec Miner (FFI Bridge & Build Spec Miner)  
**Working Directory**: `/Users/gabrielgenaro/Developer/Pessoal/Aether/.agents/teamwork/survey_spec_miner_bridge`  
**Target Project**: `/Users/gabrielgenaro/Developer/Pessoal/Aether`  

---

## 1. Executive Summary

This report delivers a comprehensive technical survey and specification mining of the FFI bridge (`crates/aether_bridge`), code generation configuration (`Makefile`), Dart-side bindings (`apps/aether_app`), and build/test environments.

### Key Discoveries:
1. **Host Environment Status**: Neither `cargo` (Rust toolchain), `flutter` (Dart/Flutter SDK), nor `flutter_rust_bridge_codegen` is available in PATH in the host shell environment (all return exit code 127 / command not found). Execution of automated verification steps requires host-level SDK provisioning.
2. **Missing Dependency in `aether_bridge`**: `crates/aether_bridge/src/api.rs` uses `uuid::Uuid` directly (`use uuid::Uuid;`, `Uuid::new_v4()`), but `crates/aether_bridge/Cargo.toml` does NOT declare `uuid` as a dependency. In Rust 2021 edition, this will cause a compile-time failure upon `cargo check -p aether_bridge`.
3. **`make bridge` Target Path Discrepancy**:
   - `Makefile` executes `flutter_rust_bridge_codegen generate --rust-root crates/aether_bridge --flutter-root apps/aether_app`.
   - By default in `flutter_rust_bridge` v2, output Dart files are placed into `<flutter-root>/lib/src/rust/`.
   - However, the codebase contains an empty directory `apps/aether_app/lib/src/bridge/` and commented import in `main.dart` (`// import 'src/bridge/api.dart';`).
   - Resolution options: Either add `--dart-output apps/aether_app/lib/src/bridge` to `make bridge`, or standardize on `lib/src/rust/` and update Dart imports.
4. **Clean Functional FFI Contract**: A pure value-based FFI API where `Timeline` is passed and returned across the bridge aligns with Flutter Riverpod state immutability, avoids tricky Rust-side pointer lifecycle bugs, and satisfies all acceptance criteria.

---

## 2. Crate Inspection: `crates/aether_bridge`

### 2.1 Workspace & Dependencies
**Path**: `/Users/gabrielgenaro/Developer/Pessoal/Aether/crates/aether_bridge/Cargo.toml`

```toml
[package]
name = "aether_bridge"
version = "0.1.0"
edition = "2021"

[dependencies]
flutter_rust_bridge = "2.3.0"
aether_core = { path = "../aether_core" }
aether_render = { path = "../aether_render" }
aether_media = { path = "../aether_media" }

[lib]
crate-type = ["cdylib", "staticlib"]
```

### 2.2 Critical Dependency Defect
In `crates/aether_bridge/src/api.rs`:
```rust
use aether_core::timeline::{Rational, Timeline};
use uuid::Uuid; // Line 2

pub fn create_timeline() -> Timeline {
    Timeline {
        id: Uuid::new_v4(), // Line 6
...
```
- `uuid` is **NOT** listed in `crates/aether_bridge/Cargo.toml`.
- While `crates/aether_core` lists `uuid = { version = "1.10", features = ["v4"] }`, Rust 2021 does not permit transitive imports of undeclared crates.
- **Fix Required**: Add `uuid = { version = "1.10", features = ["v4"] }` to `crates/aether_bridge/Cargo.toml` or re-export `pub use uuid;` from `aether_core`.

### 2.3 Crate Configuration & Flutter Rust Bridge Version
- `flutter_rust_bridge = "2.3.0"`: Uses FRB v2.3.0 architecture.
- `crate-type = ["cdylib", "staticlib"]`: Properly configured to generate both dynamic library (`.dylib` / `.so` / `.dll`) and static library (`.a`) for native embedding across iOS, macOS, Android, and Desktop platforms.
- `crates/aether_bridge/src/lib.rs` currently contains only:
  ```rust
  pub mod api;
  ```
  Note: When FRB v2 codegen runs, it automatically creates `frb_generated.rs` (and platform support files) and expects/injects `mod frb_generated;` into `lib.rs`.

---

## 3. Makefile & Code Generation Analysis

### 3.1 Content of `Makefile`
**Path**: `/Users/gabrielgenaro/Developer/Pessoal/Aether/Makefile`

```makefile
.PHONY: all setup bridge

setup:
	cargo install flutter_rust_bridge_codegen

bridge:
	flutter_rust_bridge_codegen generate --rust-root crates/aether_bridge --flutter-root apps/aether_app
```

### 3.2 Codegen Command Breakdown
Command: `flutter_rust_bridge_codegen generate --rust-root crates/aether_bridge --flutter-root apps/aether_app`

1. **`--rust-root crates/aether_bridge`**:
   - FRB v2 inspects `crates/aether_bridge/src/api.rs` (or `src/api/` submodules).
   - Generates Rust boilerplate in `crates/aether_bridge/src/frb_generated.rs`, `frb_generated.io.rs`, `frb_generated.web.rs`.
2. **`--flutter-root apps/aether_app`**:
   - FRB v2 inspects `apps/aether_app/pubspec.yaml` to verify `flutter_rust_bridge: ^2.3.0`.
   - **Default Target Output**: Without `--dart-output`, FRB v2 defaults to generating Dart files in:
     `apps/aether_app/lib/src/rust/`
     - `frb_generated.dart`
     - `frb_generated.io.dart`
     - `frb_generated.web.dart`
     - `api.dart` (or `api/mod.dart`)
3. **Discrepancy with Flutter Codebase**:
   - `apps/aether_app/lib/src/bridge` exists as an empty directory.
   - `apps/aether_app/lib/main.dart` lines 5-6:
     ```dart
     // Import the generated bridge when available:
     // import 'src/bridge/api.dart';
     ```
   - If `make bridge` is executed as-is, Dart files are generated into `lib/src/rust/`, meaning `import 'src/bridge/api.dart';` would fail.
   - **Two Path Solutions**:
     - *Solution A (Align Makefile to existing directory)*:
       Modify `make bridge` to:
       ```makefile
       flutter_rust_bridge_codegen generate --rust-root crates/aether_bridge --flutter-root apps/aether_app --dart-output apps/aether_app/lib/src/bridge
       ```
     - *Solution B (Adopt standard FRB v2 convention)*:
       Keep `Makefile` as-is, delete the unused `apps/aether_app/lib/src/bridge` directory, and update Dart imports to `import 'src/rust/api.dart';` and `import 'src/rust/frb_generated.dart';`.
4. **Codegen Version Alignment**:
   - `make setup` runs `cargo install flutter_rust_bridge_codegen`.
   - In production CI/CD, running unpinned `cargo install` might install a newer minor/patch version (e.g., 2.7.x) than `flutter_rust_bridge = "2.3.0"`. It is recommended to pin: `cargo install flutter_rust_bridge_codegen --version 2.3.0`.

---

## 4. Verification of Build & Test Commands

Every command specified in the acceptance criteria was executed in `/Users/gabrielgenaro/Developer/Pessoal/Aether`. The exact results are documented below:

| # | Command | Directory | Exit Code | Observed Output / Stderr | Status |
|---|---------|-----------|-----------|--------------------------|--------|
| 1 | `cargo test -p aether_core` | `/Users/gabrielgenaro/Developer/Pessoal/Aether` | 127 | `zsh:1: command not found: cargo` | Tooling missing on host |
| 2 | `cargo check -p aether_bridge` | `/Users/gabrielgenaro/Developer/Pessoal/Aether` | 127 | `zsh:1: command not found: cargo` | Tooling missing on host + code defect (`uuid` missing in Cargo.toml) |
| 3 | `make bridge` | `/Users/gabrielgenaro/Developer/Pessoal/Aether` | 2 | `flutter_rust_bridge_codegen generate --rust-root crates/aether_bridge --flutter-root apps/aether_app`<br>`make: flutter_rust_bridge_codegen: No such file or directory`<br>`make: *** [bridge] Error 1` | Codegen binary not installed |
| 4 | `flutter analyze` | `/Users/gabrielgenaro/Developer/Pessoal/Aether` | 127 | `zsh:1: command not found: flutter` | Tooling missing on host |
| 5 | `flutter analyze` | `/Users/gabrielgenaro/Developer/Pessoal/Aether/apps/aether_app` | 127 | `zsh:1: command not found: flutter` | Tooling missing on host |

### Host Environment Diagnosis:
- Homebrew formulae check (`brew list`): `rust`, `cargo`, and `flutter` are NOT installed via Homebrew.
- System PATH check: PATH contains Android SDK cmdline tools, OpenJDK 17, Ruby, Bun, Node v24, LM Studio, but neither `~/.cargo/bin` nor any Flutter SDK directory is in PATH.
- Zsh history inspection indicates previous Flutter usage (`brew install --cask flutter` followed by `brew remove flutter`).
- **Prerequisite for automated testing**: The host developer machine requires Rust (`rustup` / `cargo`) and Flutter SDK installed and added to PATH before automated test execution can succeed.

---

## 5. Interface Constraints & Serialization Specification

### 5.1 Domain Types Transfer Across FFI

The core types in `aether_core::timeline` map to Dart as follows:

| Rust Type | Dart Type (`package:uuid` / FRB v2) | Serialization / Representation | Notes |
|-----------|-------------------------------------|--------------------------------|-------|
| `uuid::Uuid` | `UuidValue` (or `String`) | 16-byte binary UUID / string representation | Supported natively in FRB v2 SSE codec; `uuid: ^4.4.0` is present in `pubspec.yaml`. |
| `i64` | `PlatformInt64` / `int` | Signed 64-bit integer | Standard native Dart int on 64-bit architectures. |
| `i32` | `int` | Signed 32-bit integer | Maps directly to Dart `int`. |
| `Vec<T>` | `List<T>` | Sequential array/list | Direct mapping. |
| `Rational` | `class Rational { int num; int den; }` | Generated struct | Holds framerate / PTS timebase. |
| `TrackKind` | `enum TrackKind { video, audio, overlay }` | Generated Dart enum | Discriminant serialization. |
| `Clip` | `class Clip { UuidValue id; UuidValue sourceId; int sourceIn; int sourceOut; int timelineIn; int timelineOut; }` | Generated class with fields | Leaf node of timeline DAG. |
| `Track` | `class Track { UuidValue id; TrackKind kind; List<Clip> clips; }` | Generated class with fields | Container of clips for a layer. |
| `Timeline` | `class Timeline { UuidValue id; Rational timebase; int durationPts; List<Track> tracks; }` | Generated class with fields | Root container representing the timeline DAG. |

### 5.2 API Signatures for `crates/aether_bridge/src/api.rs`

To satisfy Requirements R1 and R2, the following public API must be exposed from `crates/aether_bridge/src/api.rs`:

```rust
use aether_core::timeline::{Clip, Rational, Timeline, TrackKind};
use uuid::Uuid;

/// Initializes native subsystems (render, media, frb user utils).
pub fn init_engine() {
    flutter_rust_bridge::setup_default_user_utils();
    aether_render::init_render();
    aether_media::init_media();
}

/// Creates a new timeline with a default video track ready to accept clips.
pub fn create_timeline() -> Timeline {
    let mut timeline = Timeline {
        id: Uuid::new_v4(),
        timebase: Rational { num: 60, den: 1 },
        duration_pts: 0,
        tracks: vec![],
    };
    // Ensure at least one default video track is present so clips can be added immediately
    timeline.add_track(TrackKind::Video);
    timeline
}

/// Adds a new track to an existing timeline.
pub fn add_track(mut timeline: Timeline, kind: TrackKind) -> Timeline {
    timeline.add_track(kind);
    timeline
}

/// Adds a clip to a specific track in the timeline and recalculates timeline duration.
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

### 5.3 Error Handling & Boundaries
- Rust functions return `Result<Timeline, String>`.
- In Dart, FRB converts `Err(String)` into an asynchronous Dart exception (e.g. `AnyhowException`).
- The Riverpod state notifier wraps FFI calls with `try / catch`, maintaining UI stability and enabling descriptive error feedback (e.g., if a non-existent track ID is provided).

### 5.4 State Flow Architecture with Flutter Riverpod
1. **App Initialization**:
   - `main()` in `apps/aether_app/lib/main.dart`:
     ```dart
     WidgetsFlutterBinding.ensureInitialized();
     await RustLib.init();
     initEngine();
     runApp(const ProviderScope(child: AetherApp()));
     ```
2. **Timeline Provider (`timeline_provider.dart`)**:
   - State: `AsyncValue<Timeline>` or `Timeline?`.
   - On provider init: calls `createTimeline()` via bridge.
   - `addClip()` method:
     - Identifies target track ID (e.g., `state.tracks.first.id`).
     - Calls `addClipToTrack(timeline: state, trackId: ..., ...)` via bridge.
     - Sets `state = updatedTimeline`.
3. **UI Display (`TimelineView`)**:
   - Listens to `timelineProvider`.
   - Reads `timeline.tracks` and calculates `totalClips = timeline.tracks.fold(0, (acc, t) => acc + t.clips.length)`.
   - Displays current clip count and timeline duration (`durationPts`).
   - Button: "Add Clip" triggers `ref.read(timelineProvider.notifier).addClip()`.

---

## 6. Features Discovered & Probed

## Features Discovered
| # | Category | Feature | Description | Inputs | Outputs | Error Behavior | Discovered Via |
|---|----------|---------|-------------|--------|---------|----------------|----------------|
| 1 | FFI Bridge | `init_engine()` | Initializes FRB default utils, native renderer, and native media decoders | None | `()` / `void` | Infallible | Source inspection of `crates/aether_bridge/src/api.rs:13-17` |
| 2 | FFI Bridge | `create_timeline()` | Creates initial timeline with default timebase (60/1 fps) | None | `Timeline` | Infallible | Source inspection of `crates/aether_bridge/src/api.rs:4-11` |
| 3 | FFI Bridge | `add_clip_to_track` | Adds clip to target track and recalculates duration PTS | `timeline: Timeline`, `track_id: Uuid`, `source_id: Uuid`, `source_in: i64`, `source_out: i64`, `timeline_in: i64` | `Result<Timeline, String>` | Returns `Err("Track with ID ... not found")` or `Err("Invalid clip bounds")` | Required for R1 / R2 acceptance criteria |
| 4 | FFI Bridge | `add_track` | Appends a new track (video/audio/overlay) to timeline | `timeline: Timeline`, `kind: TrackKind` | `Timeline` | Infallible | Required for multi-track timeline construction |
| 5 | Codegen | `make bridge` | Invokes `flutter_rust_bridge_codegen` to produce Dart-Rust bindings | `--rust-root`, `--flutter-root` | Generated Rust and Dart glue code | Fails if codegen binary not installed (exit code 2) | Source inspection of `Makefile:6-7` |
| 6 | Codegen | `make setup` | Installs `flutter_rust_bridge_codegen` CLI via cargo | None | Binary installed in `~/.cargo/bin` | Fails if `cargo` is missing (exit code 127) | Source inspection of `Makefile:3-4` |
| 7 | Core Model | `Timeline.duration_pts` recalculation | Aggregates max `timeline_out` across all tracks and clips | `&mut self` | `()` (updates `self.duration_pts`) | Infallible (defaults to 0 if empty) | Required by acceptance criteria R1 |
| 8 | Core Model | `Clip` PTS bounds validation | Enforces `timeline_out >= timeline_in` and `source_out >= source_in` | Start and end PTS values | `Result<(), TimelineError>` | Returns `TimelineError::InvalidClipBounds` | Logic invariant |
| 9 | Core Model | Track kind discrimination | Distinguishes Video, Audio, and Overlay tracks | `TrackKind` enum | Enum variant | Infallible | Source inspection of `crates/aether_core/src/timeline.rs:17-22` |

## Edge Cases
| # | Feature | Input | Observed / Expected Behavior |
|---|---------|-------|------------------------------|
| 1 | `add_clip_to_track` | Empty timeline with zero tracks | Returns error `TrackNotFound` if no track exists with given UUID. `create_timeline()` should initialize at least 1 track. |
| 2 | `add_clip_to_track` | Non-existent `track_id` | Returns `TimelineError::TrackNotFound(id)`, bridge maps to `Err(String)`, Dart catches without crash. |
| 3 | `add_clip_to_track` | Zero-duration clip (`source_in == source_out`) | Valid slice (duration 0). `duration_pts` equals `timeline_in`. |
| 4 | `add_clip_to_track` | Inverted bounds (`source_out < source_in`) | Fails validation with `InvalidClipBounds` error. |
| 5 | `duration_pts` recalculation | Multiple tracks with overlapping time spans | Returns the maximum `timeline_out` across all tracks. |
| 6 | `duration_pts` recalculation | Staggered clips with gaps between clips | Correctly reflects maximum end point regardless of gaps. |
| 7 | `make bridge` | Target folder discrepancy | FRB v2 outputs to `lib/src/rust` by default; if `lib/src/bridge` is desired, `--dart-output` parameter is mandatory. |
| 8 | `cargo check -p aether_bridge` | Missing `uuid` dependency | Fails compilation on `use uuid::Uuid;` until `uuid` is declared in `aether_bridge/Cargo.toml`. |

---

## 7. Recommendations for Implementation Phase

1. **Fix `crates/aether_bridge/Cargo.toml`**:
   Add `uuid = { version = "1.10", features = ["v4"] }` to ensure `cargo check -p aether_bridge` succeeds.
2. **Resolve Dart Output Target Directory**:
   Either configure `make bridge` with `--dart-output apps/aether_app/lib/src/bridge` OR standardize the project on `apps/aether_app/lib/src/rust/` and remove the empty `lib/src/bridge` folder.
3. **Pin FRB Codegen in Makefile**:
   Update `make setup` to `cargo install flutter_rust_bridge_codegen --version 2.3.0` to avoid mismatch between CLI tool and runtime library.
4. **Implement Stateless Value-based FFI Functions**:
   Implement `create_timeline` with a pre-configured video track, and `add_clip_to_track(mut timeline: Timeline, ...) -> Result<Timeline, String>`. This seamlessly integrates with Riverpod.
5. **Host Tooling Prerequisite**:
   Ensure `cargo` and `flutter` are installed on the execution environment so that `cargo test -p aether_core`, `cargo check -p aether_bridge`, and `flutter analyze` can be executed automatically.
