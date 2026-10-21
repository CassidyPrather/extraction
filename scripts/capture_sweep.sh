#!/usr/bin/env bash
# The looks-wrong sweep: every model and prefab alone in a studio, eight
# angles under three lights, then scripts/capture_sweep.py flags z-fighting,
# fallback colours, blank views, odd sizes, blockouts and floating pieces.
#   scripts/capture_sweep.sh [filter] [out dir] [extra game args...]
# filter: all (default), models, prefabs, or text in a subject's name
# (props/bed, prefab:barn; commas for several). Env: LIGHTS (day,dusk,night).
# Software-rendered under Xvfb: about ten seconds a subject a light.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
FILTER="${1:-all}"
OUT="${2:-target/sweep}"
shift $(( $# > 2 ? 2 : $# ))
BIN="${EXTRACTION_BIN:-./target/fast/extraction}"
SIZE=1280x640
mkdir -p "$OUT"
DATA="$(mktemp -d)"
trap 'rm -rf "$DATA"' EXIT
EXTRA=()
[ -n "${LIGHTS:-}" ] && EXTRA+=(--sweep-lights "$LIGHTS")
WGPU_BACKEND=vulkan timeout 36000 xvfb-run -a -s "-screen 0 ${SIZE}x24" \
    "$BIN" --map training_yard --sweep "$FILTER" --capture-out "$OUT" \
    --size "$SIZE" --frames 30 --data-dir "$DATA" --new-game --no-robots \
    "${EXTRA[@]}" "$@" 2>&1 | grep -E 'ERROR|panicked|sweep:' || true
python3 scripts/capture_sweep.py "$OUT"
