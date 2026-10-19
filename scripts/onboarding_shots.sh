#!/usr/bin/env bash
# The training yard's stations as a person standing at each sees them
# (eyes 1.7 m over the ground; in the truck, over its bed), into
# docs/shots/onboarding/. A fresh profile, so it's a first run.
#   EXTRACTION_BIN=target/fast/extraction scripts/onboarding_shots.sh [name...]
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
OUT=docs/shots/onboarding
mkdir -p "$OUT"
DATA=$(mktemp -d)
trap 'rm -rf "$DATA"' EXIT
# name  x,y,z,yaw,pitch  (y = 0: standing on the ground there)
SHOTS=(
  "1_truck_bed -0.3,15.77,95.4,180,-14"
  "1b_off_the_steps -0.6,0,94.3,345,-6"
  "2_put_it_away 3.3,0,89.6,0,-6"
  "3_out_of_reach -3.4,0,86.8,345,4"
  "4_range_post -14.3,0,48.6,0,-6"
  "4b_range_table -21,0,43.4,0,-14"
  "5_the_house 4.2,0,27,90,-6"
  "6_first_aid 2,0,8,90,-6"
  "7_the_shimmer 2,0,-30,90,-6"
  "7b_the_shimmer_beyond 2.5,0,-30.5,115,-10"
  "8_buttons -1.2,0,-50,270,-8"
  "9_going_home 4,0,-61.6,0,-6"
)
for s in "${SHOTS[@]}"; do
  name=${s%% *}; cam=${s#* }
  if [ $# -gt 0 ] && [[ ! " $* " =~ " $name " ]]; then continue; fi
  scripts/shot.sh "$OUT/$name.png" "$cam" 40 --map training_yard --data-dir "$DATA" --no-robots
done
