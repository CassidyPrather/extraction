#!/usr/bin/env bash
# Can the way home be read at night? Shoots each first job's way home in a
# valley (`night-walks`: a stop every 50 m, eye height, looking along it)
# at new moon in clear air and again in a downpour, then scores each stop
# (`scripts/night_route.py`): does the way ahead stand out from the ground
# beside it? Writes <out>/<weather>/report.txt and worst.png.
#   scripts/night_route.sh [map] [out dir]
# Exit status: the number of stops, both weathers, where it can't be read.
set -uo pipefail
cd "$(git rev-parse --show-toplevel)"
MAP="${1:-sallow_reservoir}"
OUT="${2:-target/night_route/$MAP}"
BIN="${EXTRACTION_BIN:-./target/fast/extraction}"
mkdir -p "$OUT"
cargo run -q --profile fast -p extraction_core --bin night-walks -- "$MAP" > "$OUT/walks.jsonl"
python3 -c "import json,sys; [print(json.loads(l)['still']) for l in open(sys.argv[1])]" \
    "$OUT/walks.jsonl" > "$OUT/walks.stills"
lost=0
for weather in clearing downpour; do
    mkdir -p "$OUT/$weather"
    CINE="$OUT/walks.stills" HOUR=night WEATHER="$weather" EXTRACTION_BIN="$BIN" \
        scripts/capture_cinematic.sh "$MAP" "$OUT/$weather" --moon new > "$OUT/$weather/capture.log" 2>&1
    python3 scripts/night_route.py "$OUT/walks.jsonl" "$OUT/$weather" \
        --sheet "$OUT/$weather/worst.png" > "$OUT/$weather/report.txt"
    n=$?
    echo "$MAP $weather: $(tail -1 "$OUT/$weather/report.txt")"
    lost=$((lost + n))
done
exit $((lost > 125 ? 125 : lost))
