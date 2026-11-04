#!/usr/bin/env bash
# The training yard's stations as a person standing at each sees them
# (eyes 1.7 m over the ground; in the truck, over its bed), into
# docs/shots/onboarding/. A fresh profile, so it's a first run; each
# shot starts the yard at its station (EXTRACTION_YARD_AT), clear weather.
#   EXTRACTION_BIN=target/fast/extraction scripts/onboarding_shots.sh [name...]
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
OUT=docs/shots/onboarding
mkdir -p "$OUT"
DATA=$(mktemp -d)
trap 'rm -rf "$DATA"' EXIT
# name  station  x,y,z,yaw,pitch  (y = 0: standing on the ground there).
# The station by its title (onboarding::STATIONS, any case, spaces as
# underscores), not its number: stations come and go, and by number the
# shots drifted onto blank boards.
SHOTS=(
  "truck_bed your_kit -0.3,15.77,95.4,180,-14"
  "settings settings -0.3,15.77,95.4,180,-14"
  "off_the_steps getting_about -0.6,0,94.3,345,-6"
  "getting_about getting_about -3.5,0,91.5,0,-6"
  "out_of_reach out_of_reach -3.4,0,86.8,345,4"
  "the_pen out_of_reach -6.3,0,82.2,355,-14"
  "the_beam out_of_reach 0.3,0,82.5,0,0"
  "range_post the_range -4.3,0,49.4,315,-6"
  "range_table run_it_dry -13.5,0,45.0,0,-14"
  "the_house the_house 4.2,0,27,90,-6"
  "first_aid first_aid 2,0,8,90,-6"
  "the_shimmer the_shimmer 2,0,-30,90,-6"
  "the_shimmer_beyond the_shimmer 3.5,0,-31.5,60,-12"
  "the_stone the_shimmer 9.2,0,-36,90,-28"
  "going_home going_home 4,0,-61.6,0,-6"
)
for s in "${SHOTS[@]}"; do
  read -r name station cam <<<"$s"
  if [ $# -gt 0 ] && [[ ! " $* " =~ " $name " ]]; then continue; fi
  # That station's lamp lit, the ones before it done.
  EXTRACTION_YARD_AT=$station scripts/shot.sh "$OUT/$name.png" "$cam" 40 \
    --map training_yard --data-dir "$DATA" --no-robots --weather clearing
  # Kept as JPEG: a dozen PNGs are 20 MB in the repository.
  ffmpeg -loglevel error -y -i "$OUT/$name.png" -q:v 3 "$OUT/$name.jpg" && rm "$OUT/$name.png"
done
