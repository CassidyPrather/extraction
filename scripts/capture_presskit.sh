#!/usr/bin/env bash
# Press-kit stills: the views in scripts/capture_paths/<map>.stills, shot in
# each mood, into target/presskit/<map>/<mood>/; then
# scripts/capture_presskit.py draws target/presskit/index.html to choose
# from (and `--publish` copies the chosen into docs/shots/showcase/).
#   scripts/capture_presskit.sh [maps...]        (default: the three valleys)
# Env: MOODS (default "golden night rain surge"), SIZE (default 1600x900),
# ONLY (view-name filter), OUT (default target/presskit), SEED (default 1).
# Moods: golden (dusk, clearing, the sun 7 degrees up), night (clearing,
# by torchlight), rain (a downpour by day), surge (the Surge raging,
# drizzle), day (the reference look: drizzle by day).
# Shot through --cinematic, which steps time a frame at a time, so the
# Surge (gone in minutes of real time) rages in every still.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
BIN="${EXTRACTION_BIN:-./target/fast/extraction}"
OUT="${OUT:-target/presskit}"
SIZE="${SIZE:-1600x900}"
MAPS=("$@")
[ ${#MAPS[@]} -eq 0 ] && MAPS=(marrow_creek kettle_pass cinder_works)
for MAP in "${MAPS[@]}"; do
    for MOOD in ${MOODS:-golden night rain surge}; do
        TORCH=()
        case "$MOOD" in
            golden) ARGS=(--hour dusk --weather clearing --sun-elevation 7) ;;
            night) ARGS=(--hour night --weather clearing); TORCH=(env EXTRACTION_TORCH=1) ;;
            rain) ARGS=(--hour day --weather downpour) ;;
            surge) ARGS=(--hour day --weather drizzle --surge-in -40) ;;
            day) ARGS=(--hour day --weather drizzle) ;;
            *) echo "unknown mood $MOOD"; exit 1 ;;
        esac
        DIR="$OUT/$MAP/$MOOD"
        mkdir -p "$DIR"
        DATA="$(mktemp -d)"
        EXTRA=()
        [ -n "${ONLY:-}" ] && EXTRA+=(--cine-only "$ONLY")
        WGPU_BACKEND=vulkan timeout 3600 "${TORCH[@]}" xvfb-run -a -s "-screen 0 ${SIZE}x24" \
            "$BIN" --map "$MAP" --data-dir "$DATA" --new-game --no-robots \
            --seed "${SEED:-1}" --cinematic "scripts/capture_paths/$MAP.stills" \
            --capture-out "$DIR" --frames 60 \
            --size "$SIZE" "${ARGS[@]}" "${EXTRA[@]}" 2>&1 \
            | grep -E 'panicked|cinematic: done' || true
        rm -rf "$DATA"
        echo "$MAP $MOOD: $(ls "$DIR"/*.png 2>/dev/null | wc -l) stills in $DIR"
    done
done
python3 scripts/capture_presskit.py "$OUT"
