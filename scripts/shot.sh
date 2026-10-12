#!/usr/bin/env bash
# Headless screenshot: scripts/shot.sh <out.png> <x,y,z,yaw,pitch> [frames] [extra args...]
# Renders with the software Vulkan driver under Xvfb; slow but needs no GPU.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
OUT="$1"; CAM="$2"; FRAMES="${3:-20}"; shift 3 || shift $#
BIN="${EXTRACTION_BIN:-./target/debug/extraction}"
WGPU_BACKEND=vulkan timeout 900 xvfb-run -a -s "-screen 0 1600x900x24" \
    "$BIN" --screenshot "$OUT" --frames "$FRAMES" --camera "$CAM" "$@" 2>&1 \
    | grep -E 'ERROR|panicked|Screenshot saved' || true
