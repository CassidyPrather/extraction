#!/usr/bin/env bash
# Plays a batch of bot raids on a map and draws the report:
#   scripts/sim.sh [map] [runs] [extra extraction args...]
# e.g. scripts/sim.sh marrow_creek 8 --jobs 3 --sim-minutes 8
# Telemetry: target/sim/<map>/*.jsonl; report: target/sim/<map>/report/.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
MAP="${1:-marrow_creek}"
RUNS="${2:-8}"
shift $(( $# > 2 ? 2 : $# ))
cargo build --quiet --profile fast --bin extraction
TARGET="${CARGO_TARGET_DIR:-target}"
"$TARGET/fast/extraction" --sim "$RUNS" --map "$MAP" "$@"
scripts/sim-report.sh "$MAP"
