#!/usr/bin/env bash
# Runs the game's headset path against Monado's simulated headset, headless:
#   scripts/xr_check.sh [map] [out.png] [extra game args...]
# It starts monado-service (simulated HMD and WMR controllers, null
# compositor), launches the game, screenshots the window after a while,
# and fails if the OpenXR session never got focus, the runtime rejected a
# binding, or the game didn't end cleanly. It runs twice: with --mirror
# (the window shows the left eye: out.png) and without (the window gets
# its card: out-window.png), the way players launch it. A window left
# with no camera once the session began hung the GPU on a playtest.
# MIRROR=1 or MIRROR=0 runs the one.
#
# Needs a recent Monado (Ubuntu 24.04's 21.0 overflows on llvmpipe's
# format list). Build one once and point MONADO at its prefix:
#   git clone --depth 1 https://gitlab.freedesktop.org/monado/monado.git
#   cmake -G Ninja -B build -DCMAKE_INSTALL_PREFIX=$HOME/monado \
#     -DXRT_HAVE_SDL2=OFF -DXRT_HAVE_OPENGL=OFF -DXRT_HAVE_OPENGL_GLX=OFF \
#     -DXRT_HAVE_OPENGLES=OFF -DXRT_HAVE_EGL=OFF -DBUILD_TESTING=OFF
#   ninja -C build install
# and have the OpenXR loader (libopenxr-loader1) installed.
#
# SIMULATED_HANDS=1 checks finger tracking too: it needs Monado with
# scripts/monado-simulated-hands.patch applied (simulated controllers
# report XR_EXT_hand_tracking joints, fingers cycling open and shut), and
# fails unless the game logs that it saw tracked fingers.
#
# SIMULATED_REACH="dx,dy,dz,delay,secs" (same patch) moves the right
# controller by (dx, dy, dz) metres, starting `delay` seconds in and taking
# `secs`: a hand pushed into a wall, to see the physics hands' ghost glove.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
MAP="${1:-training_yard}"; OUT="${2:-target/xr_check/eye.png}"; shift 2 || shift $#
MONADO="${MONADO:-$HOME/monado}"
BIN="${EXTRACTION_BIN:-./target/debug/extraction}"
[ -x "$MONADO/bin/monado-service" ] || { echo "no monado-service under MONADO=$MONADO (see the top of this script)"; exit 2; }
LOADER=$(ls /usr/lib/*/libopenxr_loader.so.1* 2>/dev/null | head -1)
[ -n "$LOADER" ] || { echo "no OpenXR loader: apt install libopenxr-loader1"; exit 2; }

run_one() {
local MIR="$1" SHOT="$2"
WORK=$(mktemp -d)
# Unix socket paths are short: the runtime dir can't live somewhere deep.
RUN=$(mktemp -d /tmp/xr.XXXX)
mkdir -p "$(dirname "$SHOT")" "$WORK/lib"
ln -s "$LOADER" "$WORK/lib/libopenxr_loader.so"
cat > "$WORK/inner.sh" <<EOF
#!/usr/bin/env bash
export XDG_RUNTIME_DIR=$RUN
export XR_RUNTIME_JSON=$MONADO/share/openxr/1/openxr_monado.json
export LD_LIBRARY_PATH=$WORK/lib
export XRT_COMPOSITOR_NULL=1 SIMULATED_ENABLE=1 SIMULATED_LEFT=wmr SIMULATED_RIGHT=wmr
export SIMULATED_HANDS=${SIMULATED_HANDS:-0}
${SIMULATED_REACH:+export SIMULATED_REACH=$SIMULATED_REACH}
mkfifo $WORK/stdin
$MONADO/bin/monado-service < $WORK/stdin > $WORK/monado.log 2>&1 &
MPID=\$!
exec 9> $WORK/stdin
sleep 4
WGPU_BACKEND=vulkan timeout 900 $BIN --map $MAP --new-game $MIR --screenshot $SHOT --frames \${FRAMES:-150} $ARGS > $WORK/game.log 2>&1
echo "exit \$?" >> $WORK/game.log
kill \$MPID 2>/dev/null
EOF
chmod +x "$WORK/inner.sh"
xvfb-run -a -s "-screen 0 1600x900x24" "$WORK/inner.sh"

LOG="$WORK/game.log"
echo "== $MIR"
grep -q "entered XR state FOCUSED" "$LOG" || { echo "FAIL: the session never got focus"; fail=1; }
if grep -E "Suggested Path Unsupported|PATH_UNSUPPORTED|Failed to initialize openxr|panicked|Couldn't get swap chain texture|GPU stopped responding" "$LOG"; then
    echo "FAIL: see above"; fail=1
fi
[ -f "$SHOT" ] || { echo "FAIL: no screenshot"; fail=1; }
grep -q "loading view: lifted" "$LOG" || { echo "FAIL: the loading view never lifted (the world never shown)"; fail=1; }
grep -q "^exit 0" "$LOG" || { echo "FAIL: the game didn't end cleanly ($(grep '^exit' "$LOG"))"; fail=1; }
if [ "$MIR" = --headset ]; then
    grep -q "window: a card" "$LOG" || { echo "FAIL: no card on the window"; fail=1; }
fi
if [ "${SIMULATED_HANDS:-0}" = 1 ]; then
    grep -q "fingers tracked" "$LOG" || { echo "FAIL: no tracked fingers"; fail=1; }
fi
grep -E "Loaded OpenXR runtime|Using system|XrCamera resolution|entered XR state|fingers tracked|Handtracking|hand_tracking|loading view" "$LOG" | sed 's/^.*INFO[^ ]* //' | sort -u
echo "logs: $WORK (game.log, monado.log); window: $SHOT"
rm -rf "${RUN:?}"
}
ARGS="$*"
fail=0
case "${MIRROR:-both}" in
    1) run_one --mirror "$OUT" ;;
    0) run_one --headset "${OUT%.png}-window.png" ;;
    *) run_one --mirror "$OUT"; run_one --headset "${OUT%.png}-window.png" ;;
esac
exit $fail
