#!/usr/bin/env bash
# The worst minute of a raid in the game itself (`--stress`, see
# src/render/stress.rs), each valley in turn. Prints each valley's
# "stress:" lines: the CPU's p99 and worst, the GPU's per view, and what
# the GPU's worst frames spend per pass.
#   scripts/stress.sh [map...]
# By default one eye's work at a Beyond 2's size: a flat view EYE_PX
# square (1800: its 2980 recommended, at the default render scale).
# XR=1 runs it in Monado's simulated headset instead, both eyes at that
# headset's own size (640x720, the most it offers; the frame rate not
# held), for the stereo path. STRESS_FRAMES (default 900) and STRESS_WARM
# (90) shorten it for a software GPU, whose frames take seconds (the
# minute's blasts and breach come in the frames there are). Monado as
# scripts/xr_check.sh has it (MONADO, the OpenXR loader).
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
BIN="${EXTRACTION_BIN:-./target/fast/extraction}"
MAPS=("$@")
[ ${#MAPS[@]} -gt 0 ] || MAPS=(marrow_creek kettle_pass cinder_works sallow_reservoir rime_fell)
MONADO="${MONADO:-$HOME/monado}"
OUT=$(mktemp -d)
for MAP in "${MAPS[@]}"; do
    DATA=$(mktemp -d)
    LOG="$OUT/$MAP.log"
    if [ "${XR:-0}" != 1 ]; then
        PX="${EYE_PX:-1800}"
        WGPU_BACKEND=vulkan xvfb-run -a -s "-screen 0 ${PX}x${PX}x24" \
            "$BIN" --flat --map "$MAP" --data-dir "$DATA" --new-game --size "${PX}x${PX}" \
            --stress > "$LOG" 2>&1 || true
    else
        [ -x "$MONADO/bin/monado-service" ] || { echo "no monado-service under MONADO=$MONADO (see scripts/xr_check.sh)"; exit 2; }
        LOADER=$(ls /usr/lib/*/libopenxr_loader.so.1* 2>/dev/null | head -1)
        [ -n "$LOADER" ] || { echo "no OpenXR loader: apt install libopenxr-loader1"; exit 2; }
        RUN=$(mktemp -d /tmp/xr.XXXX)
        mkdir -p "$OUT/lib"
        ln -sf "$LOADER" "$OUT/lib/libopenxr_loader.so"
        cat > "$OUT/inner.sh" <<EOF
#!/usr/bin/env bash
export XDG_RUNTIME_DIR=$RUN
export XR_RUNTIME_JSON=$MONADO/share/openxr/1/openxr_monado.json
export LD_LIBRARY_PATH=$OUT/lib
export XRT_COMPOSITOR_NULL=1 SIMULATED_ENABLE=1 SIMULATED_LEFT=wmr SIMULATED_RIGHT=wmr
rm -f $OUT/stdin; mkfifo $OUT/stdin
$MONADO/bin/monado-service < $OUT/stdin > $OUT/monado.log 2>&1 &
MPID=\$!
exec 9> $OUT/stdin
sleep 4
WGPU_BACKEND=vulkan "$BIN" --map $MAP --data-dir $DATA \
    --new-game --mirror --hold-rate 0 --stress > $LOG 2>&1
kill \$MPID 2>/dev/null
EOF
        chmod +x "$OUT/inner.sh"
        xvfb-run -a -s "-screen 0 1600x900x24" "$OUT/inner.sh" || true
        rm -rf "${RUN:?}"
    fi
    rm -rf "${DATA:?}"
    echo "$MAP:"
    grep -E "stress: |eyes: [0-9]|panicked|No compatible view" "$LOG" | sed 's/^.*INFO[^ ]* //; s/^/    /' || echo "    no stress lines: see $LOG"
done
echo "logs: $OUT"
