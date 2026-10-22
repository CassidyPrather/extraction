#!/usr/bin/env bash
# Night lamps: every landmark's lamp (`render::landmarks`) shot after dark
# from 150 m and 300 m, with and without it, then checked brighter than
# what's round it (`scripts/wayfind.py --lamps`).
#   scripts/lamp_check.sh [map] [out dir] [extra game args, e.g. --weather fog]
# Defaults to the darkest case: clear, new moon. Later args win.
# Exit status: the number of lamp views that fail.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
MAP="${1:-sallow_reservoir}"
OUT="${2:-target/lamps/$MAP}"
shift $(( $# > 2 ? 2 : $# ))
BIN="${EXTRACTION_BIN:-./target/fast/extraction}"
DATA="$(mktemp -d)"
rm -rf "$OUT"
mkdir -p "$OUT"
WGPU_BACKEND=vulkan timeout 3600 xvfb-run -a -s "-screen 0 640x480x24" \
    "$BIN" --map "$MAP" --data-dir "$DATA" --wayfind-lamps "$OUT" --frames 60 \
    --hour night --weather clearing --moon new --size 640x480 "$@" 2>&1 \
    | grep -E 'ERROR|panicked|wayfind:' || true
rm -rf "$DATA"
python3 scripts/wayfind.py "$OUT" --lamps
