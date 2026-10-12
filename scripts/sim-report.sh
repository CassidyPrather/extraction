#!/usr/bin/env bash
# Draws heatmaps and a summary from a map's sim runs:
#   scripts/sim-report.sh [map] [runs-dir]
# Output: <runs-dir>/report/ (default target/sim/<map>/report/).
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
MAP="${1:-marrow_creek}"
RUNS="${2:-target/sim/${MAP}}"
DUMP="target/preview/${MAP}"
cargo run --quiet --release -p extraction_core --bin map-dump -- "assets/maps/${MAP}.ron" "$DUMP"
python3 scripts/sim_report.py "$RUNS" "$DUMP"
