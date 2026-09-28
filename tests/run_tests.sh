#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

export PATH="${HOME}/.cargo/bin:${PATH}"

echo "Starting Aether E2E Test Suite from ${ROOT_DIR}..."
python3 "${SCRIPT_DIR}/e2e_runner.py" "$@"
