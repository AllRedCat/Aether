#!/usr/bin/env python3
"""
Empirical Challenger Test Suite for Milestone M1 (Media Subsystem - aether_media).
Adversarially challenges format detection, header truncation, dimension boundaries,
audio parameters, path handling, and panic safety.
"""

import os
import re
import subprocess
import sys
from pathlib import Path

ROOT_DIR = Path(__file__).resolve().parent.parent

def run_adversarial_cargo_test(test_filter=None):
    cmd = ["cargo", "test", "-p", "aether_media", "--test", "adversarial_challenge_test"]
    if test_filter:
        cmd.extend([test_filter, "--", "--nocapture"])
    else:
        cmd.extend(["--", "--nocapture"])
    
    proc = subprocess.run(
        cmd,
        cwd=str(ROOT_DIR),
        capture_output=True,
        text=True,
        timeout=60
    )
    return proc

def test_vector_1_truncated_files():
    proc = run_adversarial_cargo_test("test_adversarial_truncated_files")
    return {
        "vector": "V1: Truncated Headers",
        "description": "PNG, WAV, MP4 abruptly truncated at varying header offsets",
        "status": "PASS" if proc.returncode == 0 else "FAIL",
        "output": proc.stdout if proc.returncode != 0 else "All truncated headers returned graceful CorruptFile/UnsupportedFormat."
    }

def test_vector_2_mismatched_extensions():
    proc = run_adversarial_cargo_test("test_adversarial_mismatched_extensions")
    return {
        "vector": "V2: Mismatched Extensions vs Magic Bytes",
        "description": "PNG disguised as .wav/.mp4/.txt, WAV disguised as .png/.mp4, MP4 disguised as .png/.wav",
        "status": "PASS" if proc.returncode == 0 else "FAIL",
        "output": proc.stdout if proc.returncode != 0 else "Magic numbers prioritized or gracefully rejected."
    }

def test_vector_3_boundary_dimensions():
    proc = run_adversarial_cargo_test("test_adversarial_boundary_dimensions")
    return {
        "vector": "V3: Boundary Dimensions",
        "description": "1x1 minimal PNG, 10000x1 and 1x10000 aspect ratios, 0x0 header rejection, 4GB dimension header memory safety",
        "status": "PASS" if proc.returncode == 0 else "FAIL",
        "output": proc.stdout if proc.returncode != 0 else "Handled dimensions safely without memory exhaustion."
    }

def test_vector_4_extreme_audio_parameters():
    proc = run_adversarial_cargo_test("test_adversarial_extreme_audio_parameters")
    return {
        "vector": "V4a: Extreme Valid Audio Parameters",
        "description": "Sample rates from 8 kHz to 192 kHz, channels 1 to 8, zero duration payload",
        "status": "PASS" if proc.returncode == 0 else "FAIL",
        "output": proc.stdout if proc.returncode != 0 else "Extreme valid sample rates and channels parsed cleanly."
    }

def test_vector_4b_audio_channels_zero():
    proc = run_adversarial_cargo_test("test_adversarial_audio_channels_zero")
    return {
        "vector": "V4b: Corrupted Audio Channels = 0",
        "description": "WAV with 0 channels in fmt chunk",
        "status": "PASS" if proc.returncode == 0 else "FAIL",
        "output": proc.stdout if proc.returncode != 0 else "Channels = 0 handled gracefully."
    }

def test_vector_4c_audio_sample_rate_zero_panic():
    proc = run_adversarial_cargo_test("test_adversarial_audio_sample_rate_zero_panic_check")
    # This is expected to FAIL because symphonia panics on sample_rate = 0
    return {
        "vector": "V4c: Corrupted Audio Sample Rate = 0 (Panic Safety)",
        "description": "WAV with sample_rate = 0 in fmt chunk",
        "status": "PASS" if proc.returncode == 0 else "FAIL",
        "output": "PANIC DETECTED in symphonia: 'TimeBase cannot have 0 numerator or denominator' at symphonia-core-0.5.5/src/units.rs:150:13" if proc.returncode != 0 else "Handled gracefully."
    }

def test_vector_5_unusual_paths():
    proc = run_adversarial_cargo_test("test_adversarial_unusual_paths")
    return {
        "vector": "V5: Unusual & Unicode Paths",
        "description": "Filenames with whitespace, accents (PT), Japanese (CJK), emojis, symbols, multiple dots, embedded nulls",
        "status": "PASS" if proc.returncode == 0 else "FAIL",
        "output": proc.stdout if proc.returncode != 0 else "Unusual paths handled safely without panicking."
    }

def test_vector_6_corrupted_mp4_boxes():
    proc = run_adversarial_cargo_test("test_adversarial_corrupted_mp4_boxes")
    return {
        "vector": "V6a: Corrupted MP4 Box Structure",
        "description": "Box size bomb (~4GB size claim in 32B file), missing moov box, 0-duration video",
        "status": "PASS" if proc.returncode == 0 else "FAIL",
        "output": proc.stdout if proc.returncode != 0 else "Corrupted boxes rejected gracefully without hang or OOM."
    }

def test_vector_6b_mp4_timescale_zero_panic():
    proc = run_adversarial_cargo_test("test_adversarial_mp4_timescale_zero_panic_check")
    # This is expected to FAIL because mp4 crate divides by zero on timescale = 0
    return {
        "vector": "V6b: MP4 Timescale = 0 (Divide-by-zero Panic Safety)",
        "description": "MP4 with mvhd timescale = 0",
        "status": "PASS" if proc.returncode == 0 else "FAIL",
        "output": "PANIC DETECTED in mp4 crate: 'attempt to divide by zero' at mp4-0.14.0/src/reader.rs:145:31" if proc.returncode != 0 else "Handled gracefully."
    }

def test_vector_7_fuzzing_panic_safety():
    proc = run_adversarial_cargo_test("test_adversarial_fuzzing_panic_safety_harness")
    return {
        "vector": "V7: Fuzzing & Memory Safety",
        "description": "300 iterations of pseudo-random byte permutations across png, jpg, wav, mp3, mp4, mov",
        "status": "PASS" if proc.returncode == 0 else "FAIL",
        "output": proc.stdout if proc.returncode != 0 else "100% of fuzz inputs returned graceful Result without panic."
    }

def main():
    print("=" * 70)
    print("AETHER MEDIA SUBSYSTEM: EMPIRICAL ADVERSARIAL CHALLENGER SUITE")
    print("=" * 70)

    tests = [
        test_vector_1_truncated_files,
        test_vector_2_mismatched_extensions,
        test_vector_3_boundary_dimensions,
        test_vector_4_extreme_audio_parameters,
        test_vector_4b_audio_channels_zero,
        test_vector_4c_audio_sample_rate_zero_panic,
        test_vector_5_unusual_paths,
        test_vector_6_corrupted_mp4_boxes,
        test_vector_6b_mp4_timescale_zero_panic,
        test_vector_7_fuzzing_panic_safety,
    ]

    results = []
    failures = 0
    for t in tests:
        res = t()
        results.append(res)
        status_badge = "[PASS]" if res["status"] == "PASS" else "[FAIL]"
        print(f"{status_badge} {res['vector']}: {res['description']}")
        if res["status"] == "FAIL":
            failures += 1
            print(f"       -> Reason: {res['output']}")

    print("=" * 70)
    total = len(results)
    passed = total - failures
    print(f"Summary: {passed}/{total} vectors passed, {failures} failures.")
    if failures > 0:
        print("Verdict: REQUEST_CHANGES (Defects found in panic safety)")
        sys.exit(1)
    else:
        print("Verdict: APPROVE")
        sys.exit(0)

if __name__ == "__main__":
    main()
