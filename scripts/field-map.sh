#!/usr/bin/env bash
# Draws the in-game paper map for a map: scripts/field-map.sh [map]
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
MAP="${1:-marrow_creek}"
DUMP="target/preview/${MAP}"
cargo run --quiet --release -p extraction_core --bin map-dump -- "assets/maps/${MAP}.ron" "$DUMP"
python3 scripts/field_map.py "$DUMP" "assets/textures/ui/${MAP}_field_map.jpg"
