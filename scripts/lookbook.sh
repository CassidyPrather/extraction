#!/usr/bin/env bash
# Visual lookbook: the fixed views in scripts/lookbook/<map>.views, shot in
# one run per valley so every change can be re-shot and compared.
#   scripts/lookbook.sh [maps...]          (default: all four)
# Env: OUT (default target/lookbook), WEATHER (default drizzle, the
# reference look), HOUR (dawn, day, dusk, night; default the cycle's,
# which is day), SIZE (default 1200x675), ONLY (view-name filter).
# Software-rendered under Xvfb: a few seconds a view.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
BIN="${EXTRACTION_BIN:-./target/fast/extraction}"
OUT="${OUT:-target/lookbook}"
WEATHER="${WEATHER:-drizzle}"
SIZE="${SIZE:-1200x675}"
MAPS=("$@")
[ ${#MAPS[@]} -eq 0 ] && MAPS=(marrow_creek kettle_pass cinder_works training_yard)
for MAP in "${MAPS[@]}"; do
    DIR="$OUT/$MAP"
    mkdir -p "$DIR"
    DATA="$(mktemp -d)"
    EXTRA=()
    [ -n "${ONLY:-}" ] && EXTRA+=(--tour-only "$ONLY")
    [ -n "${HOUR:-}" ] && EXTRA+=(--hour "$HOUR")
    WGPU_BACKEND=vulkan timeout 3600 xvfb-run -a -s "-screen 0 ${SIZE}x24" \
        "$BIN" --map "$MAP" --data-dir "$DATA" --new-game --tour "$DIR" \
        --tour-views "scripts/lookbook/$MAP.views" --frames 60 \
        --weather "$WEATHER" --size "$SIZE" "${EXTRA[@]}" 2>&1 \
        | grep -E 'ERROR|panicked|tour:' || true
    rm -rf "$DATA"
    echo "$MAP: $(ls "$DIR"/*.png 2>/dev/null | wc -l) views in $DIR"
done
