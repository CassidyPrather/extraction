#!/usr/bin/env bash
# Night lookbook: each valley's night views (scripts/lookbook/night/) under
# a spread of weathers and moon phases, then each shot's luminance
# histogram (scripts/night_histogram.py), so night's light levels are a
# gauge and not a guess.
#   scripts/night_matrix.sh [maps...]          (default: all with views)
# Env: OUT (default target/night), CASES (weather:moon pairs), SIZE.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
BIN="${EXTRACTION_BIN:-./target/fast/extraction}"
OUT="${OUT:-target/night}"
SIZE="${SIZE:-1200x675}"
CASES="${CASES:-clearing:full clearing:quarter clearing:new drizzle:full fog:full downpour:new}"
MAPS=("$@")
if [ ${#MAPS[@]} -eq 0 ]; then
    for f in scripts/lookbook/night/*.views; do MAPS+=("$(basename "$f" .views)"); done
fi
for MAP in "${MAPS[@]}"; do
    for CASE in $CASES; do
        W="${CASE%%:*}"
        M="${CASE##*:}"
        DIR="$OUT/$MAP/${W}_$M"
        mkdir -p "$DIR"
        DATA="$(mktemp -d)"
        WGPU_BACKEND=vulkan timeout 3600 xvfb-run -a -s "-screen 0 ${SIZE}x24" \
            "$BIN" --map "$MAP" --data-dir "$DATA" --new-game --tour "$DIR" \
            --tour-views "scripts/lookbook/night/$MAP.views" --frames 60 \
            --weather "$W" --hour night --moon "$M" --size "$SIZE" 2>&1 \
            | grep -E 'ERROR|panicked' || true
        rm -rf "$DATA"
    done
done
python3 scripts/night_histogram.py "$OUT" --check
