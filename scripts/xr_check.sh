#!/usr/bin/env bash
# Runs the game's headset path against Monado's simulated headset, headless:
#   scripts/xr_check.sh [map] [out.png] [extra game args...]
# It starts monado-service (simulated HMD and WMR controllers, null
# compositor), launches the game with --mirror, screenshots the left eye's
# view after a while, and fails if the OpenXR session never got focus or
# the runtime rejected a binding.
#
# Needs a recent Monado (Ubuntu 24.04's 21.0 overflows on llvmpipe's
# format list). Build one once and point MONADO at its prefix:
#   git clone --depth 1 https://gitlab.freedesktop.org/monado/monado.git
#   cmake -G Ninja -B build -DCMAKE_INSTALL_PREFIX=$HOME/monado \
#     -DXRT_HAVE_SDL2=OFF -DXRT_HAVE_OPENGL=OFF -DXRT_HAVE_OPENGL_GLX=OFF \
#     -DXRT_HAVE_OPENGLES=OFF -DXRT_HAVE_EGL=OFF -DBUILD_TESTING=OFF
#   ninja -C build install
# and have the OpenXR loader (libopenxr-loader1) installed.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
MAP="${1:-training_yard}"; OUT="${2:-target/xr_check/eye.png}"; shift 2 || shift $#
MONADO="${MONADO:-$HOME/monado}"
BIN="${EXTRACTION_BIN:-./target/debug/extraction}"
[ -x "$MONADO/bin/monado-service" ] || { echo "no monado-service under MONADO=$MONADO (see the top of this script)"; exit 2; }
LOADER=$(ls /usr/lib/*/libopenxr_loader.so.1* 2>/dev/null | head -1)
[ -n "$LOADER" ] || { echo "no OpenXR loader: apt install libopenxr-loader1"; exit 2; }

WORK=$(mktemp -d)
# Unix socket paths are short: the runtime dir can't live somewhere deep.
RUN=$(mktemp -d /tmp/xr.XXXX)
mkdir -p "$(dirname "$OUT")" "$WORK/lib"
ln -s "$LOADER" "$WORK/lib/libopenxr_loader.so"
cat > "$WORK/inner.sh" <<EOF
#!/usr/bin/env bash
export XDG_RUNTIME_DIR=$RUN
export XR_RUNTIME_JSON=$MONADO/share/openxr/1/openxr_monado.json
export LD_LIBRARY_PATH=$WORK/lib
export XRT_COMPOSITOR_NULL=1 SIMULATED_ENABLE=1 SIMULATED_LEFT=wmr SIMULATED_RIGHT=wmr
mkfifo $WORK/stdin
$MONADO/bin/monado-service < $WORK/stdin > $WORK/monado.log 2>&1 &
MPID=\$!
exec 9> $WORK/stdin
sleep 4
WGPU_BACKEND=vulkan timeout 900 $BIN --map $MAP --new-game --mirror --screenshot $OUT --frames \${FRAMES:-150} $* > $WORK/game.log 2>&1
echo "exit \$?" >> $WORK/game.log
kill \$MPID 2>/dev/null
EOF
chmod +x "$WORK/inner.sh"
xvfb-run -a -s "-screen 0 1600x900x24" "$WORK/inner.sh"

LOG="$WORK/game.log"
fail=0
grep -q "entered XR state FOCUSED" "$LOG" || { echo "FAIL: the session never got focus"; fail=1; }
if grep -E "Suggested Path Unsupported|PATH_UNSUPPORTED|Failed to initialize openxr|panicked" "$LOG"; then
    echo "FAIL: see above"; fail=1
fi
[ -f "$OUT" ] || { echo "FAIL: no screenshot"; fail=1; }
grep -E "Loaded OpenXR runtime|Using system|XrCamera resolution|entered XR state" "$LOG" | sed 's/^.*INFO[^ ]* //' | sort -u
echo "logs: $WORK (game.log, monado.log); eye: $OUT"
rm -rf "$RUN"
exit $fail
