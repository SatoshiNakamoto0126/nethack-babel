#!/usr/bin/env bash
#
# install.sh -- build and install NetHack Babel
#
# Usage:
#   ./install.sh              # install to ~/.local/bin + ~/.config/nethack-babel/
#   ./install.sh --system     # install to /usr/local/bin + ~/.config/nethack-babel/
#   ./install.sh --prefix DIR # install to DIR/bin + ~/.config/nethack-babel/

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

# ---------------------------------------------------------------------------
# Parse arguments
# ---------------------------------------------------------------------------

INSTALL_MODE="user"  # user | system | prefix
PREFIX=""

while [ $# -gt 0 ]; do
    case "$1" in
        --system)
            INSTALL_MODE="system"
            shift
            ;;
        --prefix)
            INSTALL_MODE="prefix"
            PREFIX="$2"
            shift 2
            ;;
        --help|-h)
            echo "Usage: $0 [--system | --prefix DIR]"
            echo ""
            echo "  (default)      Install to ~/.local/bin and ~/.config/nethack-babel/"
            echo "  --system       Install to /usr/local/bin and ~/.config/nethack-babel/"
            echo "  --prefix DIR   Install to DIR/bin and ~/.config/nethack-babel/"
            exit 0
            ;;
        *)
            echo "Unknown option: $1" >&2
            exit 1
            ;;
    esac
done

# ---------------------------------------------------------------------------
# Determine install paths
# ---------------------------------------------------------------------------

case "$INSTALL_MODE" in
    user)
        BIN_DIR="$HOME/.local/bin"
        CONFIG_DIR="$HOME/.config/nethack-babel"
        NEEDS_SUDO=""
        ;;
    system)
        BIN_DIR="/usr/local/bin"
        CONFIG_DIR="$HOME/.config/nethack-babel"
        NEEDS_SUDO="sudo"
        ;;
    prefix)
        BIN_DIR="$PREFIX/bin"
        CONFIG_DIR="$HOME/.config/nethack-babel"
        NEEDS_SUDO=""
        ;;
esac

echo "==> Building nethack-babel in release mode..."
cargo build --release

BINARY="target/release/nethack-babel"

if [ ! -f "$BINARY" ]; then
    echo "Error: Release binary not found at $BINARY" >&2
    exit 1
fi

echo "==> Installing binary to $BIN_DIR..."
${NEEDS_SUDO:+$NEEDS_SUDO} mkdir -p "$BIN_DIR"
${NEEDS_SUDO:+$NEEDS_SUDO} cp "$BINARY" "$BIN_DIR/nethack-babel"
${NEEDS_SUDO:+$NEEDS_SUDO} chmod 755 "$BIN_DIR/nethack-babel"

echo "==> Setting up default config at $CONFIG_DIR..."
mkdir -p "$CONFIG_DIR"
if [ ! -f "$CONFIG_DIR/config.toml" ]; then
    cat > "$CONFIG_DIR/config.toml" << 'TOML'
# NetHack Babel configuration
# See README.md for all available options.

[game]
language = "en"
autopickup = true
autopickup_types = "$?!/="

[display]
map_colors = true
message_colors = true
buc_highlight = true
minimap = true
nerd_fonts = false

[sound]
enabled = true
volume = 75
TOML
    echo "    Created default config.toml"
else
    echo "    Config already exists, skipping"
fi

echo ""
echo "==> Installation complete!"
echo "    Binary:  $BIN_DIR/nethack-babel"
echo "    Config:  $CONFIG_DIR/config.toml"

# Check if BIN_DIR is in PATH
case ":$PATH:" in
    *":$BIN_DIR:"*) ;;
    *)
        echo ""
        echo "    NOTE: $BIN_DIR is not in your PATH."
        echo "    Add it with:  export PATH=\"$BIN_DIR:\$PATH\""
        ;;
esac
