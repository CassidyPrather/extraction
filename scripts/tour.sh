#!/usr/bin/env bash
# QA tour: screenshots of every placement on a map (outside, and inside
# the buildings you walk into), plus tour.json saying what each shows.
#   scripts/tour.sh marrow_creek [out dir] [extra args...]
# Software-rendered under Xvfb: slow (a few seconds a view), needs no GPU.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
MAP="${1:-marrow_creek}"
OUT="${2:-target/tour/$MAP}"
shift $(( $# > 2 ? 2 : $# ))
BIN="${EXTRACTION_BIN:-./target/debug/extraction}"
mkdir -p "$OUT"
WGPU_BACKEND=vulkan timeout 7200 xvfb-run -a -s "-screen 0 1280x720x24" \
    "$BIN" --map "$MAP" --tour "$OUT" --frames 60 --weather clearing --size 1280x720 "$@" 2>&1 \
    | grep -E 'ERROR|panicked|tour:' || true
ls "$OUT"/*.png 2>/dev/null | wc -l
