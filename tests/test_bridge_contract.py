#!/usr/bin/env python3
"""
Test Suite: FFI Bridge & Codegen Contract (aether_bridge & Makefile)
Validates Acceptance Criteria:
  - "O pacote compila com sucesso (`cargo check -p aether_bridge`)."
  - "O código FFI deve ser gerado sem erros usando o comando `make bridge` (flutter_rust_bridge_codegen)."
"""

import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

ROOT_DIR = Path(__file__).resolve().parent.parent
BRIDGE_DIR = ROOT_DIR / "crates" / "aether_bridge"
CARGO_TOML = BRIDGE_DIR / "Cargo.toml"
API_RS = BRIDGE_DIR / "src" / "api.rs"
MAKEFILE = ROOT_DIR / "Makefile"

def test_bridge_cargo_toml_dependencies():
    """Verify uuid dependency is declared in crates/aether_bridge/Cargo.toml."""
    if not CARGO_TOML.exists():
        return {
            "name": "Bridge Cargo.toml Dependencies",
            "status": "FAIL",
            "message": f"File not found: {CARGO_TOML}"
        }
        
    content = CARGO_TOML.read_text(encoding="utf-8")
    has_uuid = bool(re.search(r'uuid\s*=\s*\{[^}]*features\s*=\s*\[[^]]*"v4"', content) or 
                    re.search(r'uuid\s*=\s*"[^"]*"', content) or
                    re.search(r'uuid\s*=', content))
    
    if not has_uuid:
        return {
            "name": "Bridge Cargo.toml Dependencies",
            "status": "FAIL",
            "message": "Missing `uuid` dependency in crates/aether_bridge/Cargo.toml (required for Uuid::new_v4() in api.rs)"
        }
        
    return {
        "name": "Bridge Cargo.toml Dependencies",
        "status": "PASS",
        "message": "`uuid` dependency correctly declared in crates/aether_bridge/Cargo.toml."
    }

def test_cargo_check_aether_bridge():
    """Execute `cargo check -p aether_bridge` and verify exit code 0."""
    try:
        proc = subprocess.run(
            ["cargo", "check", "-p", "aether_bridge"],
            cwd=str(ROOT_DIR),
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            timeout=120
        )
    except FileNotFoundError:
        return {
            "name": "Cargo Check aether_bridge",
            "status": "FAIL",
            "message": "cargo binary not found in PATH"
        }
    except subprocess.TimeoutExpired:
        return {
            "name": "Cargo Check aether_bridge",
            "status": "FAIL",
            "message": "`cargo check -p aether_bridge` timed out after 120s"
        }

    stdout = proc.stdout
    stderr = proc.stderr
    output = stdout + "\n" + stderr

    if proc.returncode != 0:
        return {
            "name": "Cargo Check aether_bridge",
            "status": "FAIL",
            "exit_code": proc.returncode,
            "message": f"`cargo check -p aether_bridge` failed with exit code {proc.returncode}:\n{stderr or stdout}"
        }

    return {
        "name": "Cargo Check aether_bridge",
        "status": "PASS",
        "exit_code": proc.returncode,
        "message": "`cargo check -p aether_bridge` passed cleanly with exit code 0."
    }

def test_bridge_api_endpoints():
    """Verify that crates/aether_bridge/src/api.rs exports required FFI endpoints."""
    if not API_RS.exists():
        return {
            "name": "Bridge API Endpoints",
            "status": "FAIL",
            "message": f"File not found: {API_RS}"
        }
        
    content = API_RS.read_text(encoding="utf-8")
    required_endpoints = [
        ("init_engine", r"pub\s+fn\s+init_engine"),
        ("create_timeline", r"pub\s+fn\s+create_timeline"),
        ("add_clip_to_track", r"pub\s+fn\s+add_clip_to_track"),
    ]
    
    missing = []
    for label, pattern in required_endpoints:
        if not re.search(pattern, content):
            missing.append(label)
            
    if missing:
        return {
            "name": "Bridge API Endpoints",
            "status": "FAIL",
            "message": f"Missing required FFI API functions in aether_bridge/src/api.rs: {', '.join(missing)}"
        }
        
    return {
        "name": "Bridge API Endpoints",
        "status": "PASS",
        "message": "All required FFI endpoints (init_engine, create_timeline, add_clip_to_track) exported in api.rs."
    }

def test_makefile_bridge_target():
    """Verify Makefile syntax and parameters for target `bridge`."""
    if not MAKEFILE.exists():
        return {
            "name": "Makefile Bridge Target",
            "status": "FAIL",
            "message": f"File not found: {MAKEFILE}"
        }
        
    content = MAKEFILE.read_text(encoding="utf-8")
    has_bridge_rule = bool(re.search(r"^bridge\s*:", content, re.MULTILINE))
    has_codegen_call = bool(re.search(r"flutter_rust_bridge_codegen", content))
    has_rust_root = bool(re.search(r"--rust-root\s+crates/aether_bridge", content))
    has_flutter_or_dart_root = bool(re.search(r"--(flutter|dart)-root\s+apps/aether_app", content))
    
    if not (has_bridge_rule and has_codegen_call):
        return {
            "name": "Makefile Bridge Target",
            "status": "FAIL",
            "message": "Makefile missing `bridge` rule calling `flutter_rust_bridge_codegen`"
        }
        
    if not (has_rust_root and has_flutter_or_dart_root):
        return {
            "name": "Makefile Bridge Target",
            "status": "FAIL",
            "message": "Makefile bridge target missing correct --rust-root or --dart-root/--flutter-root parameters"
        }
        
    return {
        "name": "Makefile Bridge Target",
        "status": "PASS",
        "message": "Makefile `bridge` target properly configured with rust-root and dart/flutter-root."
    }

def test_make_bridge_dry_run():
    """Execute `make -n bridge` to verify rule parses cleanly in make."""
    try:
        proc = subprocess.run(
            ["make", "-n", "bridge"],
            cwd=str(ROOT_DIR),
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            timeout=10
        )
    except FileNotFoundError:
        return {
            "name": "Make Bridge Target Dry-Run",
            "status": "FAIL",
            "message": "make binary not found in PATH"
        }

    if proc.returncode != 0:
        return {
            "name": "Make Bridge Target Dry-Run",
            "status": "FAIL",
            "message": f"`make -n bridge` failed: {proc.stderr}"
        }

    return {
        "name": "Make Bridge Target Dry-Run",
        "status": "PASS",
        "message": f"`make -n bridge` dry-run parsed command: {proc.stdout.strip()}"
    }

def test_make_bridge_execution():
    """Execute `make bridge` or check flutter_rust_bridge_codegen availability."""
    codegen_bin = shutil.which("flutter_rust_bridge_codegen")
    if not codegen_bin:
        return {
            "name": "Make Bridge Execution",
            "status": "SKIP",
            "message": "flutter_rust_bridge_codegen not installed in PATH (prerequisite tool for codegen execution; syntax verified via dry-run)."
        }

    proc = subprocess.run(
        ["make", "bridge"],
        cwd=str(ROOT_DIR),
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        timeout=180
    )
    if proc.returncode != 0:
        return {
            "name": "Make Bridge Execution",
            "status": "FAIL",
            "exit_code": proc.returncode,
            "message": f"`make bridge` failed with exit code {proc.returncode}:\n{proc.stderr}"
        }

    return {
        "name": "Make Bridge Execution",
        "status": "PASS",
        "message": "`make bridge` executed successfully, generating bindings."
    }

def run_all():
    print("=== Running Bridge Contract Tests ===")
    results = [
        test_bridge_cargo_toml_dependencies(),
        test_bridge_api_endpoints(),
        test_makefile_bridge_target(),
        test_make_bridge_dry_run(),
        test_cargo_check_aether_bridge(),
        test_make_bridge_execution(),
    ]
    for res in results:
        if res["status"] == "PASS":
            color = "\033[92m"
        elif res["status"] == "SKIP":
            color = "\033[93m"
        else:
            color = "\033[91m"
        reset_color = "\033[0m"
        print(f"[{color}{res['status']}{reset_color}] {res['name']}: {res['message']}")
    return results

if __name__ == "__main__":
    results = run_all()
    if any(r["status"] == "FAIL" for r in results):
        sys.exit(1)
    sys.exit(0)
