#!/usr/bin/env bash
# Remake the visual diff's cameras: resolve scripts/visual_diff/views.cine
# in each valley (--cine-dump) into scripts/visual_diff/<map>.stills. Do
# this when a valley changes under them (a marker or landmark moved), then
# commit the stills: the diff after that compares from the new places.
#   scripts/visual_diff_cameras.sh [maps...]
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
BIN="${EXTRACTION_BIN:-./target/fast/extraction}"
MAPS=("$@")
[ ${#MAPS[@]} -eq 0 ] && MAPS=(marrow_creek kettle_pass cinder_works sallow_reservoir rime_fell)
for MAP in "${MAPS[@]}"; do
    DATA="$(mktemp -d)"
    WGPU_BACKEND=vulkan timeout 600 xvfb-run -a -s "-screen 0 320x180x24" \
        "$BIN" --map "$MAP" --new-game --no-robots --data-dir "$DATA" --size 320x180 \
        --cine-dump "scripts/visual_diff/views.cine,scripts/visual_diff/$MAP.stills" 2>&1 \
        | grep -E 'cine-dump|panicked' || true
    rm -rf "$DATA"
done
