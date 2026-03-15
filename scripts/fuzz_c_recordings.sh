#!/usr/bin/env bash
# Generate random C NetHack recordings for fuzzing the differential test harness.
# Runs multiple games with different seeds and random keystroke sequences.
#
# Usage: ./scripts/fuzz_c_recordings.sh [num_runs] [turns_per_run]
set -euo pipefail

NUM_RUNS="${1:-10}"
TURNS="${2:-50}"
OUTDIR="crates/engine/tests/fixtures/fuzz"
mkdir -p "$OUTDIR"

# Valid movement keys + common actions
VALID_KEYS="hjklyubn.ssss"

echo "=== Fuzzing C NetHack ($NUM_RUNS runs × $TURNS turns) ==="

for run in $(seq 1 "$NUM_RUNS"); do
    SEED=$((run * 7919 + 42))  # Deterministic but varied seeds

    # Generate random keystroke sequence
    KEYS=""
    for i in $(seq 1 "$TURNS"); do
        IDX=$((RANDOM % ${#VALID_KEYS}))
        KEYS="${KEYS}${VALID_KEYS:$IDX:1}"
    done

    echo "Run $run/$NUM_RUNS: seed=$SEED, ${#KEYS} keystrokes"

    # Use the generate script
    bash scripts/generate_c_recording.sh "$SEED" "$KEYS" 2>/dev/null || true

    # Move output to fuzz directory
    if [ -f "crates/engine/tests/fixtures/c_recording_${KEYS}.jsonl" ]; then
        mv "crates/engine/tests/fixtures/c_recording_${KEYS}.jsonl" \
           "$OUTDIR/fuzz_seed_${SEED}.jsonl"
    fi
done

echo ""
echo "=== Fuzzing Complete ==="
echo "Generated recordings in $OUTDIR/"
ls -la "$OUTDIR/" 2>/dev/null | tail -20

echo ""
echo "Run differential tests: cargo test -p nethack-babel-engine --test differential"
