#!/usr/bin/env python3
"""
Empirical Challenger Test Suite for Milestone M1 (Native Engine & Bridge).
Tests boundary values, edge cases, PTS math, error handling, and the FFI bridge interface contract.
"""

import os
import re
import subprocess
import sys
from pathlib import Path

ROOT_DIR = Path(__file__).resolve().parent.parent

def test_rust_core_cargo_test():
    """Verify all unit and integration tests in aether_core pass cleanly."""
    proc = subprocess.run(
        ["cargo", "test", "-p", "aether_core"],
        cwd=str(ROOT_DIR),
        capture_output=True,
        text=True,
        timeout=120
    )
    if proc.returncode != 0:
        return {
            "name": "Rust Core Cargo Test",
            "status": "FAIL",
            "message": f"cargo test failed:\n{proc.stderr or proc.stdout}"
        }
    
    # Check that both unit tests and integration tests ran
    total_passed = sum(int(m) for m in re.findall(r"(\d+) passed", proc.stdout))
    total_failed = sum(int(m) for m in re.findall(r"(\d+) failed", proc.stdout))
    
    if total_failed > 0:
        return {
            "name": "Rust Core Cargo Test",
            "status": "FAIL",
            "message": f"cargo test reported {total_failed} failures."
        }
        
    return {
        "name": "Rust Core Cargo Test",
        "status": "PASS",
        "message": f"All tests passed ({total_passed} tests across unit and adversarial suites)."
    }

def test_boundary_zero_duration_clip():
    """Verify zero-duration clip behavior in Rust core."""
    # Test verified via crates/aether_core/tests/adversarial_suite.rs::test_adversarial_zero_length_clip
    proc = subprocess.run(
        ["cargo", "test", "-p", "aether_core", "--test", "adversarial_suite", "test_adversarial_zero_length_clip"],
        cwd=str(ROOT_DIR),
        capture_output=True,
        text=True,
        timeout=60
    )
    if proc.returncode != 0:
        return {
            "name": "Boundary: Zero-Duration Clip",
            "status": "FAIL",
            "message": f"Zero-duration test failed:\n{proc.stderr or proc.stdout}"
        }
    return {
        "name": "Boundary: Zero-Duration Clip",
        "status": "PASS",
        "message": "Zero-duration clip (source_in == source_out) is accepted and computes duration 0 as expected."
    }

def test_boundary_inverted_bounds_rejection():
    """Verify inverted source and timeline bounds are rejected."""
    proc = subprocess.run(
        ["cargo", "test", "-p", "aether_core", "--test", "adversarial_suite", "test_adversarial_inverted_bounds"],
        cwd=str(ROOT_DIR),
        capture_output=True,
        text=True,
        timeout=60
    )
    if proc.returncode != 0:
        return {
            "name": "Boundary: Inverted Bounds Rejection",
            "status": "FAIL",
            "message": f"Inverted bounds test failed:\n{proc.stderr or proc.stdout}"
        }
    return {
        "name": "Boundary: Inverted Bounds Rejection",
        "status": "PASS",
        "message": "Inverted source bounds (source_in > source_out) and timeline bounds (timeline_in > timeline_out) properly rejected."
    }

def test_boundary_negative_bounds_rejection():
    """Verify negative timeline and source bounds are rejected in aether_core."""
    proc = subprocess.run(
        ["cargo", "test", "-p", "aether_core", "--test", "adversarial_suite", "test_adversarial_negative_pts_behavior"],
        cwd=str(ROOT_DIR),
        capture_output=True,
        text=True,
        timeout=60
    )
    if proc.returncode != 0:
        return {
            "name": "Boundary: Negative Bounds Rejection",
            "status": "FAIL",
            "message": f"Negative bounds test failed:\n{proc.stderr or proc.stdout}"
        }
    
    # Also verify source code in timeline.rs contains the explicit negative check
    timeline_rs = ROOT_DIR / "crates" / "aether_core" / "src" / "timeline.rs"
    content = timeline_rs.read_text(encoding="utf-8")
    if "clip.timeline_in < 0 || clip.timeline_out < 0" not in content:
        return {
            "name": "Boundary: Negative Bounds Rejection",
            "status": "FAIL",
            "message": "Missing timeline_in/timeline_out negative bounds check in timeline.rs"
        }
    if "clip.source_in < 0 || clip.source_out < 0" not in content:
        return {
            "name": "Boundary: Negative Bounds Rejection",
            "status": "FAIL",
            "message": "Missing source_in/source_out negative bounds check in timeline.rs"
        }

    return {
        "name": "Boundary: Negative Bounds Rejection",
        "status": "PASS",
        "message": "Negative bounds (timeline_in < 0, timeline_out < 0, source_in < 0, source_out < 0) are strictly validated and rejected."
    }

def test_duration_clamping_non_negative():
    """Verify timeline recalculate_duration clamps to non-negative even with corrupt/negative clips."""
    proc = subprocess.run(
        ["cargo", "test", "-p", "aether_core", "--", "test_recalculate_duration_clamp_non_negative"],
        cwd=str(ROOT_DIR),
        capture_output=True,
        text=True,
        timeout=60
    )
    if proc.returncode != 0:
        return {
            "name": "PTS Math: Duration Non-Negative Clamping",
            "status": "FAIL",
            "message": f"Duration clamping test failed:\n{proc.stderr or proc.stdout}"
        }
    return {
        "name": "PTS Math: Duration Non-Negative Clamping",
        "status": "PASS",
        "message": "Timeline recalculate_duration clamps duration_pts to non-negative (>= 0)."
    }

def test_pts_overflow_saturation():
    """Verify saturation math prevents panics on extreme timestamps near i64::MAX."""
    proc = subprocess.run(
        ["cargo", "test", "-p", "aether_core", "--test", "adversarial_suite", "test_adversarial_integer_overflow_saturation"],
        cwd=str(ROOT_DIR),
        capture_output=True,
        text=True,
        timeout=60
    )
    if proc.returncode != 0:
        return {
            "name": "PTS Math: Integer Overflow Saturation",
            "status": "FAIL",
            "message": f"Overflow test failed:\n{proc.stderr or proc.stdout}"
        }
    return {
        "name": "PTS Math: Integer Overflow Saturation",
        "status": "PASS",
        "message": "Saturating arithmetic prevents panic under extreme timestamps (i64::MAX)."
    }

def test_multi_track_out_of_order_pts():
    """Verify PTS recalculation is accurate across tracks regardless of insertion order."""
    proc = subprocess.run(
        ["cargo", "test", "-p", "aether_core", "--test", "adversarial_suite", "test_adversarial_out_of_order_and_multi_track_pts"],
        cwd=str(ROOT_DIR),
        capture_output=True,
        text=True,
        timeout=60
    )
    if proc.returncode != 0:
        return {
            "name": "PTS Math: Multi-Track Out-of-Order Recalculation",
            "status": "FAIL",
            "message": f"Multi-track test failed:\n{proc.stderr or proc.stdout}"
        }
    return {
        "name": "PTS Math: Multi-Track Out-of-Order Recalculation",
        "status": "PASS",
        "message": "Multi-track PTS calculation accurately tracks max(timeline_out) with interleaved, staggered, and out-of-order clips."
    }

def test_bridge_compilation_and_codegen():
    """Verify aether_bridge compilation and make bridge codegen."""
    proc_check = subprocess.run(
        ["cargo", "check", "-p", "aether_bridge"],
        cwd=str(ROOT_DIR),
        capture_output=True,
        text=True,
        timeout=120
    )
    if proc_check.returncode != 0:
        return {
            "name": "Bridge Compilation & Codegen",
            "status": "FAIL",
            "message": f"cargo check -p aether_bridge failed:\n{proc_check.stderr}"
        }

    proc_bridge = subprocess.run(
        ["make", "bridge"],
        cwd=str(ROOT_DIR),
        capture_output=True,
        text=True,
        timeout=120
    )
    if proc_bridge.returncode != 0:
        return {
            "name": "Bridge Compilation & Codegen",
            "status": "FAIL",
            "message": f"make bridge failed:\n{proc_bridge.stderr}"
        }

    return {
        "name": "Bridge Compilation & Codegen",
        "status": "PASS",
        "message": "cargo check -p aether_bridge and make bridge codegen both succeed with exit code 0."
    }

def test_ffi_contract_analysis():
    """Analyze the generated Dart bridge contract and highlight architectural caveats."""
    dart_api = ROOT_DIR / "apps" / "aether_app" / "lib" / "src" / "bridge" / "api.dart"
    if not dart_api.exists():
        return {
            "name": "FFI Contract Analysis",
            "status": "FAIL",
            "message": f"Generated api.dart not found at {dart_api}"
        }
    
    content = dart_api.read_text(encoding="utf-8")
    
    # Check whether Timeline is RustOpaque
    is_opaque = "abstract class Timeline implements RustOpaqueInterface" in content
    has_track_model = "class Track" in content
    has_clip_model = "class Clip" in content
    
    caveats = []
    if is_opaque:
        caveats.append("Timeline is generated as an opaque type (RustOpaqueInterface) without direct field access.")
    if not has_track_model:
        caveats.append("Track struct is not mirrored/exported in Dart api.dart.")
    if not has_clip_model:
        caveats.append("Clip struct is not mirrored/exported in Dart api.dart.")
        
    return {
        "name": "FFI Contract Analysis",
        "status": "PASS",
        "opaque": is_opaque,
        "caveats": caveats,
        "message": f"Bridge FFI contract analyzed. Timeline is opaque: {is_opaque}. Caveats: {'; '.join(caveats) if caveats else 'None'}."
    }

def run_all():
    print("=== Empirical Challenger M1 Test Suite ===")
    tests = [
        test_rust_core_cargo_test(),
        test_boundary_zero_duration_clip(),
        test_boundary_inverted_bounds_rejection(),
        test_boundary_negative_bounds_rejection(),
        test_duration_clamping_non_negative(),
        test_pts_overflow_saturation(),
        test_multi_track_out_of_order_pts(),
        test_bridge_compilation_and_codegen(),
        test_ffi_contract_analysis(),
    ]
    for t in tests:
        color = "\033[92m" if t["status"] == "PASS" else "\033[91m"
        reset = "\033[0m"
        print(f"[{color}{t['status']}{reset}] {t['name']}: {t['message']}")
    return tests

if __name__ == "__main__":
    results = run_all()
    if any(r["status"] == "FAIL" for r in results):
        sys.exit(1)
    sys.exit(0)
