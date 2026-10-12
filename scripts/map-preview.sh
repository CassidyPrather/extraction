#!/usr/bin/env bash
# Renders a top-down design preview of a map: scripts/map-preview.sh [map] [out.png]
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
MAP="${1:-marrow_creek}"
OUT="${2:-target/preview/${MAP}.png}"
DUMP="target/preview/${MAP}"
mkdir -p "$(dirname "$OUT")"
cargo run --quiet --release -p extraction_core --bin map-dump -- "assets/maps/${MAP}.ron" "$DUMP"
python3 scripts/map_preview.py "$DUMP" "$OUT"
