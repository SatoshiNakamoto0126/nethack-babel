#!/usr/bin/env bash
# Generate a C NetHack state recording for differential testing.
# Requires: C NetHack built with DIFF_TEST enabled + WANT_SOURCE_INSTALL=1
# Uses Python pty to handle NetHack's interactive terminal (--More-- prompts).
#
# Usage:
#   ./scripts/generate_c_recording.sh [scenario] [num_turns]
#   ./scripts/generate_c_recording.sh [seed] [scenario] [num_turns]   (legacy)
#   scenario: rest|movement|search|<custom_keys>
#   num_turns: number of keystrokes (default 20)
set -euo pipefail

NETHACK_DIR="${NETHACK_DIR:-/Users/hz/Downloads/NetHack}"
BABEL_DIR="$(cd "$(dirname "$0")/.." && pwd)"
OUTPUT_NAME="${OUTPUT_NAME:-}"
LEGACY_SEED=""

if [ "$#" -ge 2 ] && [[ "$1" =~ ^[0-9]+$ ]] && ! [[ "$2" =~ ^[0-9]+$ ]]; then
    # Backward-compatible form: [seed] [scenario] [num_turns]
    LEGACY_SEED="$1"
    SCENARIO="$2"
    NUM_TURNS="${3:-20}"
else
    SCENARIO="${1:-rest}"
    NUM_TURNS="${2:-20}"
fi

if ! [[ "$NUM_TURNS" =~ ^[0-9]+$ ]]; then
    echo "ERROR: num_turns must be an integer, got: $NUM_TURNS"
    exit 1
fi

if [ -z "$OUTPUT_NAME" ]; then
    OUTPUT_NAME="$SCENARIO"
fi
SAFE_OUTPUT_NAME="$(printf '%s' "$OUTPUT_NAME" | tr -cd '[:alnum:]_.-')"
if [ -z "$SAFE_OUTPUT_NAME" ]; then
    SAFE_OUTPUT_NAME="recording"
fi

echo "=== Generating C NetHack recording (scenario=$SCENARIO, turns=$NUM_TURNS) ==="
if [ -n "$LEGACY_SEED" ]; then
    echo "Legacy seed argument provided ($LEGACY_SEED); C runtime seed control is not wired in this script."
fi

if [ ! -f "$NETHACK_DIR/playground/nethack" ]; then
    echo "ERROR: No binary at $NETHACK_DIR/playground/nethack"
    echo "Build: cd $NETHACK_DIR && make WANT_SOURCE_INSTALL=1 all"
    exit 1
fi

if ! strings "$NETHACK_DIR/playground/nethack" | grep -q "diff_test_output"; then
    echo "ERROR: Binary not built with DIFF_TEST."
    exit 1
fi

# Prepare
rm -rf "$NETHACK_DIR/playground/save/"; mkdir -p "$NETHACK_DIR/playground/save"
rm -f "$NETHACK_DIR/playground/diff_test_output.jsonl"
sed -i '' 's/^WIZARDS=.*/WIZARDS=*/' "$NETHACK_DIR/playground/sysconf" 2>/dev/null || true

# Generate keystroke sequence
case "$SCENARIO" in
    rest)      KEYS=$(printf '.%.0s' $(seq 1 "$NUM_TURNS")) ;;
    movement)  KEYS="llllkkkkhhhhjjjj" ;;
    search)    KEYS=$(printf 's%.0s' $(seq 1 "$NUM_TURNS")) ;;
    *)         KEYS="$SCENARIO" ;;
esac

echo "Keystrokes: ${#KEYS}"

# Run via Python pty (handles --More-- prompts)
python3 -c "
import pty, os, time, select, signal, fcntl, struct, termios

KEYS = '$KEYS'
NH = '$NETHACK_DIR'
master, slave = pty.openpty()
fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 24, 80, 0, 0))
pid = os.fork()
if pid == 0:
    os.close(master); os.setsid()
    os.dup2(slave, 0); os.dup2(slave, 1); os.dup2(slave, 2); os.close(slave)
    os.chdir(f'{NH}/playground')
    os.environ['HOME'] = f'{NH}/playground'
    os.environ['TERM'] = 'xterm-256color'
    os.environ['NETHACKOPTIONS'] = 'name:DiffTest,race:human,role:valkyrie,gender:female,align:neutral,!autopickup'
    os.execv('./nethack', ['nethack', '-D', '-u', 'DiffTest'])
else:
    os.close(slave)
    def drain(t=0.5):
        buf = b''
        while True:
            r,_,_ = select.select([master],[],[],t)
            if r:
                try: buf += os.read(master, 8192)
                except: break
            else: break
        return buf
    time.sleep(5); out = drain(3.0)
    while b'More' in out:
        os.write(master, b' '); time.sleep(0.5); out = drain(0.5)
    for key in KEYS:
        os.write(master, key.encode()); time.sleep(0.8)
        out = drain(0.3)
        while b'More' in out:
            os.write(master, b' '); time.sleep(0.3); out = drain(0.3)
    time.sleep(3)
    j = f'{NH}/playground/diff_test_output.jsonl'
    if os.path.exists(j) and os.path.getsize(j) > 0:
        print(f'Recorded {len(open(j).readlines())} turns ({os.path.getsize(j)} bytes)')
    else:
        print('FAILED: No recording')
    os.kill(pid, signal.SIGTERM); time.sleep(0.5)
    try: os.kill(pid, signal.SIGKILL)
    except: pass
    os.waitpid(pid, 0); os.close(master)
"

# Copy to fixtures
JSONL="$NETHACK_DIR/playground/diff_test_output.jsonl"
if [ -f "$JSONL" ] && [ -s "$JSONL" ]; then
    mkdir -p "$BABEL_DIR/crates/engine/tests/fixtures"
    cp "$JSONL" "$BABEL_DIR/crates/engine/tests/fixtures/c_recording_${SAFE_OUTPUT_NAME}.jsonl"
    echo "→ crates/engine/tests/fixtures/c_recording_${SAFE_OUTPUT_NAME}.jsonl"
else
    echo "ERROR: No recording to copy"
    exit 1
fi
