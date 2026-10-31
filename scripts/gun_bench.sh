#!/usr/bin/env bash
# The gunsmith's bench, headless: scripts/gun_bench.sh <gun id> <out dir> [extra args...]
# Takes the review's standard set of shots (docs/craft/GUNSMITHING.md) under
# Xvfb with the software Vulkan driver, and keeps them as JPEGs. For one
# shot instead, give "-" for the dir and --screenshot FILE.png in the extra
# args: scripts/gun_bench.sh ak74 - --screenshot x.png --view left --dist 0.3
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
ID="$1"; OUT="$2"; shift 2
PROFILE="${BENCH_PROFILE:-fast}"
cargo build -q --profile "$PROFILE" -p gunsmith --example bench
BIN="target/$PROFILE/examples/bench"
ARGS=("$ID")
if [ "$OUT" != "-" ]; then ARGS+=(--shots "$OUT"); fi
WGPU_BACKEND=vulkan timeout 1800 xvfb-run -a -s "-screen 0 1600x900x24" \
    "$BIN" "${ARGS[@]}" "$@" 2>&1 | grep -E 'ERROR|panicked|bench:' || true
if [ "$OUT" != "-" ]; then
    python3 -I - "$OUT" <<'PY'
import glob, os, sys
from PIL import Image
for p in glob.glob(os.path.join(sys.argv[1], "*.png")):
    Image.open(p).convert("RGB").save(p[:-4] + ".jpg", quality=88)
    os.remove(p)
PY
fi
