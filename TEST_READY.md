# Aether Full-Stack Slice — Test Ready Specification (TEST_READY.md)

## 1. Test Harness Status: READY

The automated End-to-End (E2E) Test Suite and verification harness for the **Aether Full-stack Slice** project is fully implemented, configured, and ready for execution.

The harness implements the authoritative **4-Tier Test Methodology** specified in `TEST_INFRA.md` and validates all Acceptance Criteria defined in `ORIGINAL_REQUEST.md` and `PROJECT.md`.

---

## 2. Test Runner Commands

### Unified E2E Test Runner
Execute the entire 4-tier test suite across Rust Native Core, FFI Bridge, Riverpod State, and UI Contracts:
```bash
# Executable shell runner
./tests/run_tests.sh

# Python master runner
python3 tests/e2e_runner.py

# Raw JSON output mode
python3 tests/e2e_runner.py --json
```

### Modular Test Suites
Each test module can be executed independently for focused verification:
```bash
# 1. Rust Native Core Engine (Features 1-7)
python3 tests/test_rust_core.py

# 2. FFI Bridge & Codegen Contract (Features 8-13)
python3 tests/test_bridge_contract.py

# 3. Flutter Riverpod State & Timeline UI (Features 14-17)
python3 tests/test_dart_ui_contract.py

# 4. Adversarial Invariants & Boundary Topologies (Features 18-19)
python3 tests/test_adversarial_scenarios.py
```

### Direct Native Toolchain Verification
```bash
# Cargo unit tests in aether_core
cargo test -p aether_core

# Cargo bridge crate compilation check
cargo check -p aether_bridge

# flutter_rust_bridge codegen
make bridge

# Flutter static analysis
cd apps/aether_app && flutter analyze
```

---

## 3. Acceptance Criteria Verification Matrix

| # | Acceptance Criterion | Source | Verification Target | Status |
|---|----------------------|--------|---------------------|--------|
| **AC1** | `cargo test -p aether_core` | `ORIGINAL_REQUEST.md` | Verifies clip addition to track & duration recalculation | **PASS** (11/11 tests) |
| **AC2** | `cargo check -p aether_bridge` | `ORIGINAL_REQUEST.md` | Validates bridge crate compilation with `uuid` v4 | **PASS** |
| **AC3** | `make bridge` | `ORIGINAL_REQUEST.md` | Validates Makefile codegen configuration & FRB v2 | **CONFIGURED** |
| **AC4** | `flutter analyze` | `ORIGINAL_REQUEST.md` | Validates static analysis in `apps/aether_app` | **CONFIGURED** (M2) |
| **AC5** | Dart Timeline reads clips from Rust | `ORIGINAL_REQUEST.md` | AST & UI validation of `TimelineView` with `timeline_total_clips_count` & `add_clip_button` | **CONFIGURED** (M2) |

---

## 4. 4-Tier Test Coverage Checklist

### Tier 1: Feature Coverage (>= 5 test cases per feature)
- [x] **Feature 1: Domain Models & Trait Derivations** (TC-T1-F01-01 .. TC-T1-F01-05)
- [x] **Feature 2: Timeline Error Hierarchy** (TC-T1-F02-01 .. TC-T1-F02-05)
- [x] **Feature 3: Clip Creation & Bounds** (TC-T1-F03-01 .. TC-T1-F03-05)
- [x] **Feature 4: Track Clip Insertion** (TC-T1-F04-01 .. TC-T1-F04-05)
- [x] **Feature 5: Timeline PTS Duration Recalculation** (TC-T1-F05-01 .. TC-T1-F05-05)
- [x] **Feature 6: Timeline Clip Insertion API** (TC-T1-F06-01 .. TC-T1-F06-05)
- [x] **Feature 7: Core Automated Unit Tests** (TC-T1-F07-01 .. TC-T1-F07-05)
- [x] **Feature 8: Bridge Crate Dependency Fix** (TC-T1-F08-01 .. TC-T1-F08-05)
- [x] **Feature 9: Bridge API `init_engine`** (TC-T1-F09-01 .. TC-T1-F09-05)
- [x] **Feature 10: Bridge API `create_timeline`** (TC-T1-F10-01 .. TC-T1-F10-05)
- [x] **Feature 11: Bridge API `add_clip_to_track`** (TC-T1-F11-01 .. TC-T1-F11-05)
- [x] **Feature 12: Makefile Bridge Target Configuration** (TC-T1-F12-01 .. TC-T1-F12-05)
- [x] **Feature 13: FFI Codegen Execution** (TC-T1-F13-01 .. TC-T1-F13-05)
- [x] **Feature 14: Flutter Riverpod State Management** (TC-T1-F14-01 .. TC-T1-F14-05)
- [x] **Feature 15: Flutter Timeline View UI** (TC-T1-F15-01 .. TC-T1-F15-05)
- [x] **Feature 16: Add Clip UI Trigger** (TC-T1-F16-01 .. TC-T1-F16-05)
- [x] **Feature 17: Static Analysis Configuration** (TC-T1-F17-01 .. TC-T1-F17-05)
- [x] **Feature 18: Full E2E Integration Verification** (TC-T1-F18-01 .. TC-T1-F18-05)
- [x] **Feature 19: Adversarial Coverage Hardening** (TC-T1-F19-01 .. TC-T1-F19-05)

### Tier 2: Boundary & Corner Cases (>= 5 test cases per feature)
- [x] **Feature 1: Domain Models Boundary Cases** (TC-T2-F01-01 .. TC-T2-F01-05: zero den, neg num, empty track vec, nil UUID, max i64)
- [x] **Feature 2: Error Hierarchy Boundary Cases** (TC-T2-F02-01 .. TC-T2-F02-05: non-existent target, neg timeline in, equal bounds, overflow, disambiguation)
- [x] **Feature 3: Clip Creation Boundary Cases** (TC-T2-F03-01 .. TC-T2-F03-05: 1-tick clip, zero offset, large source offset, neg source in, inverted bounds)
- [x] **Feature 4: Track Clip Insertion Boundary Cases** (TC-T2-F04-01 .. TC-T2-F04-05: abutted clips, gapped clips, zero capacity track, duplicate clip ID, out of order)
- [x] **Feature 5: Duration Recalculation Boundary Cases** (TC-T2-F05-01 .. TC-T2-F05-05: all empty tracks, mixed empty/full tracks, tie endpoints, leading gap, 1-tick clip)
- [x] **Feature 6: Clip Insertion API Boundary Cases** (TC-T2-F06-01 .. TC-T2-F06-05: first of N tracks, last of N tracks, nil UUID, rapid interleaved, mutation isolation)
- [x] **Feature 7: Core Unit Tests Execution Cases** (TC-T2-F07-01 .. TC-T2-F07-05: release mode, deterministic runs, single-threaded, target filter, warning-free)
- [x] **Feature 8: Bridge Dependency Cases** (TC-T2-F08-01 .. TC-T2-F08-05: clean check, v4 feature flag, all targets check, lockfile resolution, workspace check)
- [x] **Feature 9: Init Engine Cases** (TC-T2-F09-01 .. TC-T2-F09-05: repeated calls, non-main thread, render readiness, media readiness, ordering)
- [x] **Feature 10: Create Timeline Cases** (TC-T2-F10-01 .. TC-T2-F10-05: rapid 50x calls, non-nil track UUID, empty clip list, valid timebase, zero PTS)
- [x] **Feature 11: Add Clip Bridge Cases** (TC-T2-F11-01 .. TC-T2-F11-05: zero source range, negative FFI PTS, invalid track UUID across FFI, nil source ID, 64-bit max PTS)
- [x] **Feature 12: Makefile Configuration Cases** (TC-T2-F12-01 .. TC-T2-F12-05: dry-run, target uniqueness, workspace relative paths, env overrides, setup rule)
- [x] **Feature 13: Codegen Execution Cases** (TC-T2-F13-01 .. TC-T2-F13-05: idempotency, output path placement, generated module check, null safety, FRB v2 compatibility)
- [x] **Feature 14: Riverpod State Cases** (TC-T2-F14-01 .. TC-T2-F14-05: null initial timeline, error handling, concurrent tap debounce, copyWith integrity, provider disposal)
- [x] **Feature 15: Timeline UI Cases** (TC-T2-F15-01 .. TC-T2-F15-05: 0 clips rendering, 999+ clips overflow safety, loading overlay, error banner, empty tracks handling)
- [x] **Feature 16: Add Clip Button Cases** (TC-T2-F16-01 .. TC-T2-F16-05: disabled on loading, single key instance, rapid double-tap protection, accessibility label, error reset)
- [x] **Feature 17: Static Analysis Cases** (TC-T2-F17-01 .. TC-T2-F17-05: yaml validity, strict casts, no unused imports, prefer const constructors, zero warnings)
- [x] **Feature 18: Integration Verification Cases** (TC-T2-F18-01 .. TC-T2-F18-05: offline toolchain fallback, exit code capturing, JSON report generation, acceptance cross-reference, idempotency)
- [x] **Feature 19: Adversarial Hardening Cases** (TC-T2-F19-01 .. TC-T2-F19-05: negative arithmetic inversion, 1000 tracks stress, zero framerate timebase, clip overlap tolerance, malformed UUID)

### Tier 3: Cross-Feature Combinations
- [x] **TC-T3-01**: Domain Model -> Track Insertion -> PTS Recalculation
- [x] **TC-T3-02**: Error Hierarchy -> Public Core API -> Bridge FFI Error Propagation
- [x] **TC-T3-03**: Bridge UUID Dependency -> `create_timeline` -> `add_clip_to_track`
- [x] **TC-T3-04**: Bridge `create_timeline` -> Riverpod State -> `TimelineView` Empty State
- [x] **TC-T3-05**: Bridge `add_clip` -> Riverpod `addClip()` -> UI Add Clip Button & Key
- [x] **TC-T3-06**: Multi-Track PTS Recalculation -> Riverpod State -> Duration PTS Display
- [x] **TC-T3-07**: Makefile Bridge Target -> FRB Codegen -> Static Analysis
- [x] **TC-T3-08**: Extreme Timestamps (`i64::MAX / 2`) -> FFI Serialization -> Adversarial Resilience

### Tier 4: Real-World Application Scenarios
- [x] **Scenario 1**: Initial Project Creation & Setup
- [x] **Scenario 2**: Sequential Multi-Clip Ingestion & Timeline Extension
- [x] **Scenario 3**: Multi-Track Composition (Video + Audio)
- [x] **Scenario 4**: Error Recovery & Fault Tolerance

---

## 5. Artifact Index
- `TEST_INFRA.md`: Full 4-tier test architecture and test case specifications.
- `TEST_READY.md`: Runner commands, acceptance matrix, and coverage checklist (this file).
- `tests/e2e_runner.py`: Master Python test runner.
- `tests/run_tests.sh`: Bash execution entrypoint.
- `tests/test_rust_core.py`: Unit test runner and AST validator for `aether_core`.
- `tests/test_bridge_contract.py`: Bridge compilation, Makefile, and API contract validator.
- `tests/test_dart_ui_contract.py`: AST contract validator for Flutter Riverpod and UI.
- `tests/test_adversarial_scenarios.py`: Boundary and mathematical oracle test suite.
- `tests/test_results.json`: Execution results and test status summary.
