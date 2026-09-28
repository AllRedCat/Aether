# Formal Handoff Report: FFI Bridge & Build Specification

**Role**: Survey Spec Miner (FFI Bridge & Build Spec Miner)  
**Agent Directory**: `/Users/gabrielgenaro/Developer/Pessoal/Aether/.agents/teamwork/survey_spec_miner_bridge`  
**Handoff Type**: Hard (Task complete)  
**Recipient**: Orchestrator (`fc902b32-5c5a-4c10-a43b-df36c14550c4`)  

---

## 1. Observation

1. **Undeclared Dependency in `aether_bridge`**:
   - `crates/aether_bridge/src/api.rs:2`: `use uuid::Uuid;`
   - `crates/aether_bridge/src/api.rs:6`: `id: Uuid::new_v4(),`
   - `crates/aether_bridge/Cargo.toml:6-10`:
     ```toml
     [dependencies]
     flutter_rust_bridge = "2.3.0"
     aether_core = { path = "../aether_core" }
     aether_render = { path = "../aether_render" }
     aether_media = { path = "../aether_media" }
     ```
     `uuid` is completely absent from `crates/aether_bridge/Cargo.toml`.
2. **Makefile and `make bridge` Codegen Target Path**:
   - `Makefile:6-7`:
     ```makefile
     bridge:
     	flutter_rust_bridge_codegen generate --rust-root crates/aether_bridge --flutter-root apps/aether_app
     ```
   - In `flutter_rust_bridge` v2, running without `--dart-output` generates Dart files to `<flutter-root>/lib/src/rust/`.
   - In `apps/aether_app/lib`:
     - Empty directory exists: `apps/aether_app/lib/src/bridge/`
     - `apps/aether_app/lib/main.dart:6`: `// import 'src/bridge/api.dart';`
3. **Execution of Required Acceptance Commands**:
   - `cargo test -p aether_core`:
     - Command: `cargo test -p aether_core`
     - Exit code: 127
     - Stderr: `zsh:1: command not found: cargo`
   - `cargo check -p aether_bridge`:
     - Command: `cargo check -p aether_bridge`
     - Exit code: 127
     - Stderr: `zsh:1: command not found: cargo`
   - `make bridge`:
     - Command: `make bridge`
     - Output:
       ```
       flutter_rust_bridge_codegen generate --rust-root crates/aether_bridge --flutter-root apps/aether_app
       make: flutter_rust_bridge_codegen: No such file or directory
       make: *** [bridge] Error 1
       ```
     - Exit code: 2
   - `flutter analyze`:
     - Command: `flutter analyze`
     - Exit code: 127
     - Stderr: `zsh:1: command not found: flutter`
4. **Host Machine Environment**:
   - `brew list` returned formulas and casks without `rust`, `cargo`, or `flutter`.
   - `brew info flutter` returned `Not installed`.
   - `brew info rust` returned `Not installed`.
   - `which mise asdf proto nix rustup dart fvm` returned `none of these`.
5. **Existing Bridge API & Dart Files**:
   - `crates/aether_bridge/src/api.rs:4-17` exports `create_timeline()` and `init_engine()`.
   - `apps/aether_app/lib/src/bridge` is currently empty (0 generated files).
   - No `frb_generated.rs` exists yet in `crates/aether_bridge/src/`.

---

## 2. Logic Chain

1. **Step 1 (From Observation 1)**: In Rust 2021 edition, crates cannot import symbols from undeclared dependencies even if sibling dependencies use them. Because `crates/aether_bridge/src/api.rs` has `use uuid::Uuid;` while `crates/aether_bridge/Cargo.toml` lacks `uuid`, compiling `aether_bridge` will fail with an unresolved import error. Therefore, adding `uuid = { version = "1.10", features = ["v4"] }` to `aether_bridge/Cargo.toml` is strictly necessary.
2. **Step 2 (From Observation 2)**: `make bridge` executes `flutter_rust_bridge_codegen generate --rust-root crates/aether_bridge --flutter-root apps/aether_app`. Because FRB v2 outputs to `lib/src/rust/` by default, running this command will place Dart code in `lib/src/rust/` rather than the empty directory `lib/src/bridge/` anticipated by `main.dart`. Therefore, either `--dart-output apps/aether_app/lib/src/bridge` must be added to the Makefile, or `apps/aether_app/lib/main.dart` must import from `src/rust/api.dart` and `src/rust/frb_generated.dart`.
3. **Step 3 (From Observation 3 & 4)**: All four acceptance criteria commands (`cargo test`, `cargo check`, `make bridge`, and `flutter analyze`) fail immediately with exit code 127/2 because `cargo`, `flutter`, and `flutter_rust_bridge_codegen` are not installed or in the host shell's PATH. Therefore, automated verification cannot execute until the host environment has Rust and Flutter SDKs configured.
4. **Step 4 (From Observation 5 & User Requirement R1/R2)**: To fulfill R1 and R2 cleanly, `aether_bridge/src/api.rs` must expose `add_clip_to_track` receiving `timeline: Timeline` and returning `Result<Timeline, String>`. In Dart, this maps to a pure value-based state transition in Riverpod: `state = await addClipToTrack(timeline: state, ...);`, keeping the Flutter UI reactive without requiring complex mutex-guarded handles or C pointer wrappers.

---

## 3. Caveats

1. **Host Tooling Availability**: The commands `cargo`, `flutter`, and `flutter_rust_bridge_codegen` were not available on the execution runner during this survey phase. Verification of compilation and test execution was determined via deep static code analysis and authoritative tool documentation.
2. **Codegen Generation Determinism**: In `flutter_rust_bridge` v2, running codegen for the first time will generate `crates/aether_bridge/src/frb_generated.rs` and inject `mod frb_generated;` into `crates/aether_bridge/src/lib.rs`. The builder agent should verify that `crates/aether_bridge/src/lib.rs` contains `mod frb_generated;` if codegen does not auto-inject it.
3. **No Code Was Modified**: Following the strict specification miner role, this agent did not modify any files in `crates/` or `apps/`.

---

## 4. Conclusion

1. **Bridge Architectural Design**:
   - The FFI interface should be stateless and value-based:
     - `create_timeline() -> Timeline` (pre-populated with 1 video track).
     - `add_clip_to_track(mut timeline: Timeline, track_id: Uuid, source_id: Uuid, source_in: i64, source_out: i64, timeline_in: i64) -> Result<Timeline, String>`.
     - `init_engine()`.
2. **Necessary Fixes Before Codegen & Build**:
   - Add `uuid = { version = "1.10", features = ["v4"] }` to `crates/aether_bridge/Cargo.toml`.
   - Update `Makefile` target `bridge` to include `--dart-output apps/aether_app/lib/src/bridge` (or update Dart imports to `package:aether_app/src/rust/`).
   - Add unit tests in `crates/aether_core/src/timeline.rs` to satisfy the `cargo test -p aether_core` criterion.
3. **Flutter Riverpod Integration**:
   - Create `timeline_provider.dart` managing `Timeline` state using Riverpod.
   - Update `TimelineView` with a button to invoke `addClip` and a widget displaying total clip count across tracks.

---

## 5. Verification Method

### 5.1 Verification Commands
Once the Rust and Flutter SDKs are present in the environment:
1. **Verify Rust Core Tests**:
   ```bash
   cargo test -p aether_core
   ```
   *Expected outcome*: 0 test failures, passes clip addition and duration PTS recalculation tests.
2. **Verify Bridge Crate Compilation**:
   ```bash
   cargo check -p aether_bridge
   ```
   *Expected outcome*: Compiles with exit code 0 without undeclared crate errors.
3. **Verify Codegen**:
   ```bash
   make bridge
   ```
   *Expected outcome*: Generates `frb_generated.rs` in `crates/aether_bridge/src/` and Dart bindings in `apps/aether_app/lib/src/bridge/` (or `lib/src/rust/`) with exit code 0.
4. **Verify Flutter Static Analysis**:
   ```bash
   cd apps/aether_app && flutter analyze
   ```
   *Expected outcome*: `No issues found!`.

### 5.2 Files to Inspect
- `/Users/gabrielgenaro/Developer/Pessoal/Aether/crates/aether_bridge/Cargo.toml`
- `/Users/gabrielgenaro/Developer/Pessoal/Aether/crates/aether_bridge/src/api.rs`
- `/Users/gabrielgenaro/Developer/Pessoal/Aether/Makefile`
- `/Users/gabrielgenaro/Developer/Pessoal/Aether/apps/aether_app/lib/main.dart`
- `/Users/gabrielgenaro/Developer/Pessoal/Aether/apps/aether_app/lib/src/features/timeline/timeline_view.dart`
