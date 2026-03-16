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

# Valid movement keys + common safe actions.
VALID_KEYS="hjklyubn.s"
SUCCESS=0
FAIL=0

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

    OUT_BASENAME="fuzz_seed_${SEED}"
    OUTPUT_NAME="$OUT_BASENAME" bash scripts/generate_c_recording.sh "$KEYS" "$TURNS" \
        >/dev/null 2>&1 || true

    # Move output to fuzz directory.
    SRC="crates/engine/tests/fixtures/c_recording_${OUT_BASENAME}.jsonl"
    if [ -f "$SRC" ]; then
        mv "$SRC" "$OUTDIR/${OUT_BASENAME}.jsonl"
        SUCCESS=$((SUCCESS + 1))
    else
        FAIL=$((FAIL + 1))
    fi
done

echo ""
echo "=== Fuzzing Complete ==="
echo "Generated recordings in $OUTDIR/ (success=$SUCCESS, failed=$FAIL)"
ls -la "$OUTDIR/" 2>/dev/null | tail -20

echo ""
echo "Run differential tests: cargo test -p nethack-babel-engine --test differential"
