#!/usr/bin/env python3
"""
Master E2E Acceptance Test Runner for Aether Full-Stack Slice
Orchestrates and aggregates validation across all Acceptance Criteria and 4-tier test specifications:
  - cargo test -p aether_core
  - cargo check -p aether_bridge
  - make bridge
  - flutter analyze
  - Dart / Timeline screen reading clip list/count from Rust via FFI
"""

import argparse
import json
import os
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

# Add tests directory to import path
TESTS_DIR = Path(__file__).resolve().parent
sys.path.insert(0, str(TESTS_DIR))

import test_rust_core
import test_bridge_contract
import test_dart_ui_contract
import test_adversarial_scenarios

RESULTS_JSON = TESTS_DIR / "test_results.json"

def run_e2e_suite(verbose=False):
    start_time = time.time()
    timestamp = datetime.now(timezone.utc).isoformat()
    
    print("\n" + "=" * 70)
    print("  AETHER FULL-STACK SLICE — E2E ACCEPTANCE TEST RUNNER")
    print(f"  Timestamp: {timestamp}")
    print("=" * 70 + "\n")
    
    suites = [
        ("Tier 1/2: Rust Native Core Engine", test_rust_core.run_all),
        ("Tier 1/2: FFI Bridge & Codegen Contract", test_bridge_contract.run_all),
        ("Tier 1/2: Flutter Riverpod State & Timeline UI", test_dart_ui_contract.run_all),
        ("Tier 2/3: Adversarial Bounds & PTS Invariants", test_adversarial_scenarios.run_all),
    ]
    
    aggregated_results = []
    total_passed = 0
    total_failed = 0
    total_skipped = 0
    
    for suite_name, runner_fn in suites:
        print(f"\n>>> Executing Suite: {suite_name}")
        results = runner_fn()
        for res in results:
            res["suite"] = suite_name
            aggregated_results.append(res)
            if res["status"] == "PASS":
                total_passed += 1
            elif res["status"] == "SKIP":
                total_skipped += 1
            else:
                total_failed += 1

    elapsed = time.time() - start_time
    
    # Map to ORIGINAL_REQUEST.md Acceptance Criteria
    acceptance_criteria = {
        "AC1: cargo test -p aether_core": any(
            r["name"] == "Cargo Test aether_core Execution" and r["status"] == "PASS" 
            for r in aggregated_results
        ),
        "AC2: cargo check -p aether_bridge": any(
            r["name"] == "Cargo Check aether_bridge" and r["status"] == "PASS" 
            for r in aggregated_results
        ),
        "AC3: make bridge configuration & execution": any(
            r["name"] == "Makefile Bridge Target" and r["status"] == "PASS"
            for r in aggregated_results
        ),
        "AC4: flutter analyze (clean static analysis)": any(
            "Flutter Static Analysis" in r["name"] and r["status"] == "PASS"
            for r in aggregated_results
        ),
        "AC5: Dart Timeline screen reading clips from Rust": any(
            r["name"] == "TimelineView UI & Rust State Display" and r["status"] == "PASS"
            for r in aggregated_results
        )
    }

    report = {
        "timestamp": timestamp,
        "elapsed_seconds": round(elapsed, 3),
        "summary": {
            "total_checks": len(aggregated_results),
            "passed": total_passed,
            "failed": total_failed,
            "skipped": total_skipped,
            "success": (total_failed == 0)
        },
        "acceptance_criteria": acceptance_criteria,
        "results": aggregated_results
    }
    
    # Save results to JSON file
    RESULTS_JSON.write_text(json.dumps(report, indent=2), encoding="utf-8")
    
    print("\n" + "=" * 70)
    print("  ACCEPTANCE CRITERIA STATUS SUMMARY")
    print("=" * 70)
    for ac, passed in acceptance_criteria.items():
        status_label = "\033[92m[PASSED]\033[0m" if passed else "\033[91m[PENDING/FAILED]\033[0m"
        print(f"  {status_label} {ac}")
        
    print("\n" + "-" * 70)
    summary_color = "\033[92m" if total_failed == 0 else "\033[91m"
    print(f"  Total Checks: {len(aggregated_results)} | Passed: {total_passed} | Failed: {total_failed} | Skipped: {total_skipped}")
    print(f"  Execution Time: {round(elapsed, 2)}s")
    print(f"  Full Results JSON: {RESULTS_JSON}")
    print(f"  Overall Status: {summary_color}{'PASSED' if total_failed == 0 else 'ACTION REQUIRED'}\033[0m")
    print("=" * 70 + "\n")
    
    return report

def main():
    parser = argparse.ArgumentParser(description="Aether E2E Acceptance Test Runner")
    parser.add_argument("--verbose", action="store_true", help="Print verbose error traces")
    parser.add_argument("--json", action="store_true", help="Output only raw JSON result")
    args = parser.parse_args()
    
    report = run_e2e_suite(verbose=args.verbose)
    if args.json:
        print(json.dumps(report, indent=2))
        
    # Non-zero exit code if failures detected
    if report["summary"]["failed"] > 0:
        sys.exit(1)
    sys.exit(0)

if __name__ == "__main__":
    main()
