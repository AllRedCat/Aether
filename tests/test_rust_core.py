#!/usr/bin/env python3
"""
Test Suite: Rust Core Engine (aether_core)
Validates Acceptance Criterion:
  "O comando `cargo test -p aether_core` deve passar sem falhas, contendo ao menos um teste unitário
   que valide se um `Clip` foi adicionado com sucesso a uma `Track` e se o tamanho da Timeline (`duration_pts`)
   foi recalculado corretamente."
"""

import os
import re
import subprocess
import sys
from pathlib import Path

ROOT_DIR = Path(__file__).resolve().parent.parent
AETHER_CORE_DIR = ROOT_DIR / "crates" / "aether_core"
TIMELINE_RS = AETHER_CORE_DIR / "src" / "timeline.rs"

def test_timeline_source_definitions():
    """Verify core domain struct definitions in timeline.rs."""
    if not TIMELINE_RS.exists():
        return {
            "name": "Rust Core Domain Definitions",
            "status": "FAIL",
            "message": f"File not found: {TIMELINE_RS}"
        }
    
    content = TIMELINE_RS.read_text(encoding="utf-8")
    required_symbols = [
        ("struct Timeline", r"pub\s+struct\s+Timeline"),
        ("struct Track", r"pub\s+struct\s+Track"),
        ("struct Clip", r"pub\s+struct\s+Clip"),
        ("struct Rational", r"pub\s+struct\s+Rational"),
        ("enum TrackKind", r"pub\s+enum\s+TrackKind"),
        ("duration_pts field", r"duration_pts\s*:\s*i64"),
    ]
    
    missing = []
    for label, pattern in required_symbols:
        if not re.search(pattern, content):
            missing.append(label)
            
    if missing:
        return {
            "name": "Rust Core Domain Definitions",
            "status": "FAIL",
            "message": f"Missing domain definitions in timeline.rs: {', '.join(missing)}"
        }
        
    return {
        "name": "Rust Core Domain Definitions",
        "status": "PASS",
        "message": "All required domain models (Timeline, Track, Clip, Rational, TrackKind, duration_pts) defined."
    }

def test_cargo_test_execution():
    """Execute `cargo test -p aether_core` and analyze results."""
    try:
        proc = subprocess.run(
            ["cargo", "test", "-p", "aether_core"],
            cwd=str(ROOT_DIR),
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            timeout=120
        )
    except FileNotFoundError:
        return {
            "name": "Cargo Test aether_core Execution",
            "status": "FAIL",
            "message": "cargo binary not found in PATH"
        }
    except subprocess.TimeoutExpired:
        return {
            "name": "Cargo Test aether_core Execution",
            "status": "FAIL",
            "message": "cargo test -p aether_core timed out after 120s"
        }

    stdout = proc.stdout
    stderr = proc.stderr
    output = stdout + "\n" + stderr

    if proc.returncode != 0:
        return {
            "name": "Cargo Test aether_core Execution",
            "status": "FAIL",
            "exit_code": proc.returncode,
            "message": f"`cargo test -p aether_core` failed with exit code {proc.returncode}:\n{stderr or stdout}"
        }

    # Extract test pass counts
    test_result_matches = re.findall(r"test result: ok\.\s+(\d+)\s+passed;\s+(\d+)\s+failed;", output)
    total_passed = sum(int(m[0]) for m in test_result_matches)
    total_failed = sum(int(m[1]) for m in test_result_matches)

    return {
        "name": "Cargo Test aether_core Execution",
        "status": "PASS" if total_failed == 0 else "FAIL",
        "exit_code": proc.returncode,
        "total_passed": total_passed,
        "total_failed": total_failed,
        "output": output,
        "message": f"`cargo test -p aether_core` passed ({total_passed} tests passed, {total_failed} failed)."
    }

def test_unit_test_content_requirements():
    """Verify that tests explicitly validate clip insertion and duration_pts recalculation."""
    if not TIMELINE_RS.exists():
        return {
            "name": "Rust Core Unit Test Requirements",
            "status": "FAIL",
            "message": f"File not found: {TIMELINE_RS}"
        }
        
    content = TIMELINE_RS.read_text(encoding="utf-8")
    
    # Check for unit test module
    has_test_mod = bool(re.search(r"#\[cfg\(test\)\]", content))
    has_clip_test = bool(re.search(r"fn\s+test.*add.*clip", content, re.IGNORECASE) or 
                        re.search(r"add_clip", content))
    has_duration_check = bool(re.search(r"duration_pts", content) and re.search(r"assert", content))
    
    if not has_test_mod:
        return {
            "name": "Rust Core Unit Test Requirements",
            "status": "FAIL",
            "message": "No #[cfg(test)] module found in timeline.rs"
        }
        
    if not (has_clip_test and has_duration_check):
        return {
            "name": "Rust Core Unit Test Requirements",
            "status": "FAIL",
            "message": f"Unit tests in timeline.rs missing explicit clip addition and duration_pts verification (clip test: {has_clip_test}, duration assert: {has_duration_check})"
        }
        
    return {
        "name": "Rust Core Unit Test Requirements",
        "status": "PASS",
        "message": "Unit tests in timeline.rs cover clip insertion and duration_pts recalculation."
    }

def run_all():
    print("=== Running Rust Core Tests ===")
    results = [
        test_timeline_source_definitions(),
        test_unit_test_content_requirements(),
        test_cargo_test_execution(),
    ]
    for res in results:
        status_color = "\033[92m" if res["status"] == "PASS" else "\033[91m"
        reset_color = "\033[0m"
        print(f"[{status_color}{res['status']}{reset_color}] {res['name']}: {res['message']}")
    return results

if __name__ == "__main__":
    results = run_all()
    if any(r["status"] == "FAIL" for r in results):
        sys.exit(1)
    sys.exit(0)
