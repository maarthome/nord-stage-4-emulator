#!/usr/bin/env bash
# Builds the Nord Stage 4 LV2 plugin and installs the bundle into ~/.lv2 (or $LV2_DEST).
# Usage (from anywhere in the repo):  crates/nord-lv2/install-lv2.sh
# Needs Rust (rustup) and the X11 headers for the GUI:  sudo apt install libx11-dev
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DEST="${LV2_DEST:-$HOME/.lv2}/nord-stage4.lv2"
URI="https://github.com/ymatrix1/nord-stage-4-emulator#lv2"

if ! command -v cargo >/dev/null; then
    echo "cargo not found. Install Rust with rustup:" >&2
    echo "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh" >&2
    exit 1
fi

cd "$ROOT"
# Tune for the CPU it is built on (Pi 5 = Cortex-A76) unless RUSTFLAGS is already set.
export RUSTFLAGS="${RUSTFLAGS:--C target-cpu=native}"
cargo build --release -p nord-lv2 -p nord-lv2-ui

mkdir -p "$DEST"
cp crates/nord-lv2/bundle/nord-stage4.lv2/*.ttl "$DEST/"
cp target/release/libnord_lv2.so target/release/libnord_lv2_ui.so "$DEST/"
echo "Installed to $DEST"

if command -v lv2info >/dev/null; then
    lv2info "$URI" | head -n 5
else
    echo "Tip: sudo apt install lilv-utils, then: lv2info '$URI'"
fi
