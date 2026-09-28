# Orchestration Plan — Aether Full-stack Slice

## Objective
Deliver the full-stack slice of Aether connecting Flutter (Riverpod) to Rust (aether_core + aether_bridge) for adding a clip to a track in timeline, recalculating duration_pts, exposing via flutter_rust_bridge, and updating TimelineView.

## Phases
1. **Phase 0: Survey & Specification**
   - Survey 1: Rust Core (`aether_core` structure, timeline, track, clip, DAG, PTS recalculation, tests).
   - Survey 2: Flutter App (`aether_app` structure, riverpod state management, timeline view UI, widgets).
   - Survey 3: FFI Bridge & Tooling (`aether_bridge`, flutter_rust_bridge codegen, Makefile, CI/tests).
   - Merge findings into `PROJECT.md` Feature Inventory & Architecture.

2. **Phase 1: Milestone M1 — Rust Core & Bridge**
   - Implement `add_clip` logic in `aether_core` with `duration_pts` recalculation.
   - Add unit tests in `aether_core` verifying clip addition and `duration_pts`.
   - Expose `add_clip` and timeline query functions in `aether_bridge` via `flutter_rust_bridge`.
   - Verify `cargo test -p aether_core`, `cargo check -p aether_bridge`, and `make bridge`.
   - Gate verification: Explorer -> Worker -> Reviewer -> Challenger -> Auditor.

3. **Phase 2: Milestone M2 — Flutter App & Riverpod Integration**
   - Implement Riverpod TimelineProvider consuming Rust Timeline state.
   - Build UI action (button/trigger) to call FFI `add_clip` and update `TimelineView`.
   - Verify `flutter analyze` passes cleanly.
   - Gate verification: Explorer -> Worker -> Reviewer -> Challenger -> Auditor.

4. **Phase 3: Milestone M3 — Full Verification & Acceptance Checks**
   - Execute all acceptance criteria.
   - Validate end-to-end flow and UI rendering.
   - Produce final human-facing report to Sentinel.
