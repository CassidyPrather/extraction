#!/usr/bin/env bash
# Cinematic footage: flies the shots in scripts/capture_paths/<map>.cine (or
# CINE) through a valley, saves every frame, then encodes each shot to
# <out>/<shot>.mp4 (and the lot to <out>/reel.mp4) with ffmpeg if there is
# one; always draws <out>/<shot>_sheet.jpg, a contact sheet of the shot.
#   scripts/capture_cinematic.sh marrow_creek [out dir] [extra game args...]
# Env: CINE (path file), ONLY (shot-name filter), PREVIEW (N stills a shot
# instead of every frame), FPS (default 24), SIZE (default 1280x720),
# HOUR, WEATHER, SUN (sun elevation at dawn/dusk), SEED (default 1).
# Software-rendered under Xvfb: a few seconds a frame. A 12 s shot at
# 24 fps is 288 frames, about 15 minutes; PREVIEW=6 frames it in one.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
MAP="${1:-marrow_creek}"
OUT="${2:-target/capture/$MAP}"
shift $(( $# > 2 ? 2 : $# ))
BIN="${EXTRACTION_BIN:-./target/fast/extraction}"
. scripts/capture_common.sh
capture_check_bin "$BIN"
CINE="${CINE:-scripts/capture_paths/$MAP.cine}"
SIZE="${SIZE:-1280x720}"
FPS="${FPS:-24}"
ARGS=(--map "$MAP" --cinematic "$CINE" --capture-out "$OUT" --fps "$FPS"
      --size "$SIZE" --frames 60 --seed "${SEED:-1}" --no-robots)
[ -n "${ONLY:-}" ] && ARGS+=(--cine-only "$ONLY")
[ -n "${PREVIEW:-}" ] && ARGS+=(--cine-preview "$PREVIEW")
[ -n "${HOUR:-}" ] && ARGS+=(--hour "$HOUR")
[ -n "${WEATHER:-}" ] && ARGS+=(--weather "$WEATHER")
[ -n "${SUN:-}" ] && ARGS+=(--sun-elevation "$SUN")
mkdir -p "$OUT"
DATA="$(mktemp -d)"
trap 'rm -rf "$DATA"' EXIT
WGPU_BACKEND=vulkan timeout 36000 xvfb-run -a -s "-screen 0 ${SIZE}x24" \
    "$BIN" "${ARGS[@]}" --data-dir "$DATA" --new-game "$@" 2>&1 \
    | grep -E 'ERROR|panicked|cinematic:' || true
python3 scripts/capture_encode.py "$OUT" --fps "$FPS"
