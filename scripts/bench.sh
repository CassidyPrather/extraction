#!/usr/bin/env bash
# Frame-budget benchmark: the lookbook's fixed views (scripts/lookbook/),
# each through the headset's eye frustum with a second eye rendering
# beside it, as a headset does. Writes one JSON line a view (draws,
# triangles, shadow-cascade draws and triangles, CPU main/physics ms, GPU
# ms per pass where timestamps exist) to $OUT/<map>.jsonl and prints a
# table against the budget (src/render/budget.rs, docs/PERFORMANCE.md).
#   scripts/bench.sh [maps...]          (default: all four)
# Env: OUT (default target/bench), SIZE (one eye, default 1024x1024),
# HOLD (frames a view, default 12), WEATHER (default drizzle), HOUR (dawn,
# day, dusk, night; default the cycle's), ONLY.
# Under Xvfb this is lavapipe, a CPU renderer: compare GPU times only
# with each other, never with the headset's.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
BIN="${EXTRACTION_BIN:-./target/fast/extraction}"
OUT="${OUT:-target/bench}"
SIZE="${SIZE:-1024x1024}"
WEATHER="${WEATHER:-drizzle}"
MAPS=("$@")
[ ${#MAPS[@]} -eq 0 ] && MAPS=(marrow_creek kettle_pass cinder_works training_yard)
mkdir -p "$OUT"
for MAP in "${MAPS[@]}"; do
    DATA="$(mktemp -d)"
    SHOTS="$OUT/$MAP"
    mkdir -p "$SHOTS"
    rm -f "$OUT/$MAP.jsonl"
    EXTRA=()
    [ -n "${ONLY:-}" ] && EXTRA+=(--tour-only "$ONLY")
    [ -n "${HOUR:-}" ] && EXTRA+=(--hour "$HOUR")
    EXTRACTION_BENCH="$OUT/$MAP.jsonl" EXTRACTION_BENCH_HOLD="${HOLD:-12}" \
    WGPU_BACKEND=vulkan timeout 3600 xvfb-run -a -s "-screen 0 ${SIZE}x24" \
        "$BIN" --map "$MAP" --data-dir "$DATA" --new-game --tour "$SHOTS" \
        --tour-views "scripts/lookbook/$MAP.views" --frames 60 \
        --weather "$WEATHER" --size "$SIZE" --no-robots "${EXTRA[@]}" 2>&1 \
        | grep -E 'ERROR|panicked|bench ' || true
    rm -rf "$DATA"
done
python3 scripts/bench_report.py "${MAPS[@]/#/$OUT/}"
