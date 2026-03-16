#!/usr/bin/env bash
# End-to-end differential execution test pipeline.
set -euo pipefail

echo "=== Differential Execution Test Pipeline ==="

if [ "$#" -ge 2 ] && [[ "$1" =~ ^[0-9]+$ ]] && ! [[ "${2:-}" =~ ^[0-9]+$ ]]; then
    # Legacy: diff_test.sh [seed] [scenario] [turns]
    SEED="$1"
    SCENARIO="${2:-movement}"
    TURNS="${3:-20}"
else
    # Preferred: diff_test.sh [scenario] [turns]
    SEED="12345"
    SCENARIO="${1:-movement}"
    TURNS="${2:-20}"
fi

echo "Phase 1: Generate C recording (scenario=$SCENARIO, turns=$TURNS; legacy-seed=$SEED)..."
bash "$(dirname "$0")/generate_c_recording.sh" "$SEED" "$SCENARIO" "$TURNS"

echo ""
echo "Phase 2: Run Rust differential tests..."
cargo test -p nethack-babel-engine --test differential -- --nocapture 2>&1

echo ""
echo "=== Pipeline Complete ==="
