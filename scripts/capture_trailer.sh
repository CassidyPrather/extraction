#!/usr/bin/env bash
# A trailer cut from a real run: a sim bot plays one raid, rendered, while a
# chase camera and a drone film the moments worth seeing; then
# scripts/capture_trailer.py cuts them together.
#   scripts/capture_trailer.sh [map] [out dir] [extra game args...]
# Env: SEED (default 3), PERSONALITY (default hunter: it looks for fights),
# MINUTES (raid minutes it plans for, default 6), BUDGET (seconds of footage
# at most, default 75), SIZE (default 1280x720).
# The raid runs at the sim's own fixed step; frames that aren't filmed cost
# little, the filmed ones a few seconds each: about 2-4 hours for 75 s.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
MAP="${1:-marrow_creek}"
OUT="${2:-target/trailer/$MAP}"
shift $(( $# > 2 ? 2 : $# ))
BIN="${EXTRACTION_BIN:-./target/fast/extraction}"
. scripts/capture_common.sh
capture_check_bin "$BIN"
SIZE="${SIZE:-1280x720}"
mkdir -p "$OUT"
rm -rf "$OUT/chase" "$OUT/drone"
DATA="$(mktemp -d)"
trap 'rm -rf "$DATA"' EXIT
WGPU_BACKEND=vulkan timeout 36000 xvfb-run -a -s "-screen 0 ${SIZE}x24" \
    "$BIN" --map "$MAP" --data-dir "$DATA" --sim-run trailer --sim-out "$OUT" \
    --seed "${SEED:-3}" --personality "${PERSONALITY:-hunter}" \
    --sim-minutes "${MINUTES:-6}" --trailer "${BUDGET:-75}" --capture-out "$OUT" \
    --size "$SIZE" "$@" 2>&1 | grep -E 'ERROR|panicked|trailer:|sim:' || true
python3 scripts/capture_trailer.py "$OUT"
