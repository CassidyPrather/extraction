#!/usr/bin/env bash
# First-sight pipelines gauge: no render pipeline may be built mid-raid.
# Each valley loads on the real renderer (lavapipe under Xvfb), the
# warm-up shelf runs its 900 frames (render::warmup), then everything a
# raid can bring out comes out in front of the camera (EXTRACTION_EVERYTHING:
# every robot kind and every item) and the raid's effects go off one by
# one (EXTRACTION_FIRSTS: a shot, impacts, a robot struck, a grenade, each
# bloom, a wreck). Every pipeline queued from then on is one a player
# would wait on at first sight: a judder in the headset. Any outside the
# allow-list below fails the valley.
#   scripts/pipelines.sh [maps...]    (default: every valley)
# Env: SIZE (default 256x256), OUT (default target/pipelines).
# Slow: lavapipe takes about a second a frame; run it with nothing else.
set -uo pipefail
cd "$(git rev-parse --show-toplevel)"
BIN="${EXTRACTION_BIN:-./target/fast/extraction}"
OUT="${OUT:-target/pipelines}"
SIZE="${SIZE:-256x256}"
OUT_AT=950    # everything out: after the shelf's 900 frames
FIRSTS_AT=1000
FRAMES="${FRAMES:-2700}"  # the effects (90 frames apart), the tells, the next cycle, and time to queue
MAPS=("$@")
[ ${#MAPS[@]} -eq 0 ] && MAPS=(marrow_creek kettle_pass cinder_works sallow_reservoir rime_fell)
# Built by the tool, not the game: the screenshot that ends the run.
ALLOW='screenshot-to-screen'
mkdir -p "$OUT"
fail=0
for MAP in "${MAPS[@]}"; do
    # The valley's first lookbook view, where its walk starts.
    CAM=$(grep -v '^#' "scripts/lookbook/$MAP.views" | awk 'NF{print $2; exit}')
    DATA="$(mktemp -d)"
    EXTRACTION_PIPELINES=1 EXTRACTION_EVERYTHING=$OUT_AT EXTRACTION_FIRSTS=$FIRSTS_AT \
    WGPU_BACKEND=vulkan timeout 7200 xvfb-run -a -s "-screen 0 ${SIZE}x24" \
        "$BIN" --map "$MAP" --data-dir "$DATA" --new-game --camera "$CAM" \
        --screenshot "$OUT/$MAP.png" --frames $FRAMES --size "$SIZE" \
        --weather drizzle > "$OUT/$MAP.log" 2>&1
    rm -rf "${DATA:?}"
    late=$(sed 's/\x1b\[[0-9;]*m//g' "$OUT/$MAP.log" \
        | awk -v from=$OUT_AT '/pipeline queued/ {
            for (i = 1; i <= NF; i++) if ($i == "frame") { f = $(i + 1); sub(":", "", f) }
            if (f + 0 >= from) { sub(/.*at frame /, ""); print } }' \
        | grep -v -E "$ALLOW")
    n=$(printf '%s' "$late" | grep -c . || true)
    # A run that died before its effects were through measured nothing.
    if ! grep -q 'first use: done' "$OUT/$MAP.log"; then
        echo "$MAP: the run didn't finish its effects (see $OUT/$MAP.log)"
        fail=1
        continue
    fi
    shelved=$(sed 's/\x1b\[[0-9;]*m//g' "$OUT/$MAP.log" | grep -o 'warm-up: [0-9]* kinds of thing shelved ([0-9]* entries)' | head -1)
    echo "$MAP: $n pipelines built at first sight ($shelved)"
    [ -n "$late" ] && printf '%s\n' "$late" | sed 's/^/    /' | cut -c1-400
    [ "$n" -eq 0 ] || fail=1
done
exit $fail
