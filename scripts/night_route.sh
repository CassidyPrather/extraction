#!/usr/bin/env bash
# Can the way home be read at night? Shoots each first job's way home in a
# valley (`night-walks`: a stop every 50 m, eye height, looking along it)
# at new moon, in clear air and in a downpour, with no torch and with the
# hand torch, then scores each stop (`scripts/night_route.py`): with no
# torch, is something lit ahead; with one, does the way stand out in its
# pool? Writes <out>/<case>_<weather>/report.txt and worst.png.
#   scripts/night_route.sh [map] [out dir]   (CASES="torch" for one case)
# Exit status: the number of stops, all four, where it can't be read.
set -uo pipefail
cd "$(git rev-parse --show-toplevel)"
MAP="${1:-sallow_reservoir}"
OUT="${2:-target/night_route/$MAP}"
BIN="${EXTRACTION_BIN:-./target/fast/extraction}"
mkdir -p "$OUT"
cargo run -q --profile fast -p extraction_core --bin night-walks -- "$MAP" > "$OUT/walks.jsonl"
python3 -c "import json,sys; [print(json.loads(l)['still']) for l in open(sys.argv[1])]" \
    "$OUT/walks.jsonl" > "$OUT/walks.stills"
# The torch case looks lower: a hand torch is held toward the ground
# ahead (TORCH_PITCH degrees), not where the eye looks, and the torch
# follows the camera.
TORCH_PITCH="${TORCH_PITCH:--15}"
python3 - "$OUT/walks.jsonl" "$TORCH_PITCH" > "$OUT/walks_torch.jsonl" <<'PY'
import json, sys
for line in open(sys.argv[1]):
    s = json.loads(line)
    s["pitch"] = float(sys.argv[2])
    name, pose = s["still"].split(" ", 1)
    v = pose.split(",")
    v[4] = sys.argv[2]
    s["still"] = name + " " + ",".join(v)
    print(json.dumps(s))
PY
python3 -c "import json,sys; [print(json.loads(l)['still']) for l in open(sys.argv[1])]" \
    "$OUT/walks_torch.jsonl" > "$OUT/walks_torch.stills"
lost=0
for case in ${CASES:-dark torch}; do
    flag=""
    walks="$OUT/walks.jsonl"
    stills="$OUT/walks.stills"
    if [ "$case" = torch ]; then
        flag="--torch"
        walks="$OUT/walks_torch.jsonl"
        stills="$OUT/walks_torch.stills"
    fi
    for weather in clearing downpour; do
        dir="$OUT/${case}_$weather"
        mkdir -p "$dir"
        CINE="$stills" HOUR=night WEATHER="$weather" EXTRACTION_BIN="$BIN" \
            scripts/capture_cinematic.sh "$MAP" "$dir" --moon new $flag > "$dir/capture.log" 2>&1
        python3 scripts/night_route.py "$walks" "$dir" --case "$case" \
            --sheet "$dir/worst.png" > "$dir/report.txt"
        n=$?
        echo "$MAP $case $weather: $(tail -1 "$dir/report.txt")"
        lost=$((lost + n))
    done
done
exit $((lost > 125 ? 125 : lost))
