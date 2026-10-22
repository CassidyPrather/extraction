#!/usr/bin/env bash
# A career of sim raids on one valley, one after another, the valley's
# memory of the raider (`robots::memory`) carried from raid to raid.
#
#   scripts/sim-career.sh MAP HABIT SEED RAIDS OUT [memory|blank] [FROM]
#
# HABIT is `habit` (same region, same rendezvous every raid) or `varier`
# (takes them in turn); `blank` starts every raid from a blank memory (as
# `zone_memory::MEMORY` at 0 would). Each raid's telemetry goes to
# OUT/<map>-<habit>-<mode>-s<seed>-r<raid>.jsonl, the memory beside it;
# prints one line a raid: the run and its outcome. FROM resumes a career
# at that raid, keeping the memory it had.
set -uo pipefail
map=$1 habit=$2 seed=$3 raids=$4 out=$5 mode=${6:-memory} from=${7:-0}
exe=${EXE:-target/fast/extraction}
mkdir -p "$out"
mem="$out/$map-$habit-$mode-s$seed.memory.ron"
if [ "$from" -eq 0 ]; then
  rm -f "$mem"
fi
for ((i = from; i < raids; i++)); do
  run="$map-$habit-$mode-s$seed-r$(printf %02d "$i")"
  vars=(SIM_HABIT="$habit" SIM_RAID="$i" SIM_CAREER_SEED="$seed" RUST_LOG=warn)
  if [ "$mode" = memory ]; then
    vars+=(SIM_MEMORY_FILE="$mem")
  fi
  env "${vars[@]}" "$exe" --sim-run "$run" --map "$map" --seed $((seed * 100 + i)) \
    --personality looter --sim-out "$out" > /dev/null 2>&1
  outcome=$(grep '"ev":"end"' "$out/$run.jsonl" 2>/dev/null | grep -o '"outcome":"[a-z]*"' | cut -d'"' -f4)
  echo "$run ${outcome:-none}"
done
