#!/usr/bin/env bash
# Far trees after a slow load: a valley loaded the way a slow headset
# loads it, every frame held EXTRACTION_FRAME_PAD_MS longer (default 90)
# and the GPU not ready for EXTRACTION_GPU_HOLD_S (default 120: longer
# than the old retries lasted, as Marrow Creek's 21.5 s of pipelines was
# on a headset at 90 ms a frame), then its impostor captures counted from
# the log (`world::impostors`). Every variant must be captured; none may
# stay empty.
#   scripts/impostor_check.sh [map] [out.png] [pad ms] [hold s]
# Exit status: the number of captures that stayed empty (or 1 if none ran).
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
MAP="${1:-marrow_creek}"
OUT="${2:-target/impostors/$MAP.png}"
PAD="${3:-90}"
HOLD="${4:-120}"
# Small, so frames come about as fast as a headset's did and the old
# retries (counted in frames) run out inside the hold.
SIZE="${IMPOSTOR_SIZE:-480x270}"
BIN="${EXTRACTION_BIN:-./target/fast/extraction}"
DATA="$(mktemp -d)"
LOG="${OUT%.png}.log"
mkdir -p "$(dirname "$OUT")"
# Up the valley from the gate, over the far forest (lookbook m01_gate).
CAM="${IMPOSTOR_CAM:-40,0,255,0,-2}"
EXTRACTION_FRAME_PAD_MS="$PAD" EXTRACTION_GPU_HOLD_S="$HOLD" WGPU_BACKEND=vulkan timeout 3600 xvfb-run -a -s "-screen 0 1280x720x24" \
    "$BIN" --map "$MAP" --data-dir "$DATA" --new-game --screenshot "$OUT" --frames 900 \
    --camera "$CAM" --weather clearing --hour day --no-robots --size "$SIZE" >"$LOG" 2>&1 || true
rm -rf "$DATA"
captured=$(grep -c "impostor captured" "$LOG" || true)
empty=$(grep -c "impostor capture stayed empty" "$LOG" || true)
saved=$(grep -c "Screenshot saved" "$LOG" || true)
echo "impostors: $captured captured, $empty stayed empty (frames padded ${PAD} ms, GPU held ${HOLD} s); shot ${OUT} (${saved} saved)"
if [ "$captured" -eq 0 ]; then exit 1; fi
exit "$empty"
