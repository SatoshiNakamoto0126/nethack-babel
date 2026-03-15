#!/usr/bin/env bash
# End-to-end differential execution test pipeline.
set -euo pipefail

echo "=== Differential Execution Test Pipeline ==="

SEED="${1:-12345}"
SCENARIO="${2:-movement}"

echo "Phase 1: Generate C recording (seed=$SEED, scenario=$SCENARIO)..."
bash "$(dirname "$0")/generate_c_recording.sh" "$SEED" "$SCENARIO"

echo ""
echo "Phase 2: Run Rust differential tests..."
cargo test -p nethack-babel-engine --test differential -- --nocapture 2>&1

echo ""
echo "=== Pipeline Complete ==="
