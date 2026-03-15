#!/usr/bin/env bash
# Generate a C NetHack state recording for differential testing.
set -euo pipefail

NETHACK_DIR="${NETHACK_DIR:-/Users/hz/Downloads/NetHack}"
BABEL_DIR="$(cd "$(dirname "$0")/.." && pwd)"
SEED="${1:-12345}"
SCENARIO="${2:-movement}"

echo "=== Generating C NetHack recording (seed=$SEED, scenario=$SCENARIO) ==="

# Ensure DIFF_TEST is enabled
if ! grep -q "^#define DIFF_TEST" "$NETHACK_DIR/include/config.h"; then
    echo "ERROR: DIFF_TEST not enabled in $NETHACK_DIR/include/config.h"
    echo "Uncomment '#define DIFF_TEST' and rebuild."
    exit 1
fi

# Build if needed
if [ ! -f "$NETHACK_DIR/playground/nethack" ]; then
    echo "Building C NetHack with DIFF_TEST..."
    cd "$NETHACK_DIR"
    cd sys/unix && sh setup.sh hints/macOS.370 && cd ../..
    make fetch-Lua 2>/dev/null || true
    make WANT_SOURCE_INSTALL=1 all
fi

# Configure wizard mode
sed -i '' 's/^WIZARDS=.*/WIZARDS=*/' "$NETHACK_DIR/playground/sysconf" 2>/dev/null || true

# Generate keystroke sequence based on scenario
case "$SCENARIO" in
    movement) KEYS="llllkkkkhhhhjjjj" ;;  # square loop
    rest)     KEYS="................" ;;  # 16 rests
    search)   KEYS="ssssssssssssssss" ;;  # 16 searches
    *)        KEYS="$SCENARIO" ;;          # custom sequence
esac

echo "Scenario: $SCENARIO ($((${#KEYS})) keystrokes)"

# Run with piped input
cd "$NETHACK_DIR/playground"
echo "$KEYS" | ./nethack -D -u DiffTest 2>/dev/null || true

# Copy output
mkdir -p "$BABEL_DIR/crates/engine/tests/fixtures"
if [ -f diff_test_output.jsonl ]; then
    cp diff_test_output.jsonl "$BABEL_DIR/crates/engine/tests/fixtures/c_recording_${SCENARIO}.jsonl"
    LINES=$(wc -l < diff_test_output.jsonl)
    echo "Recording captured: $LINES turns → fixtures/c_recording_${SCENARIO}.jsonl"
else
    echo "WARNING: No recording output generated"
fi
