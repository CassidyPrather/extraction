#!/usr/bin/env bash
# Hitch gauge: the raid's one-off events and a valley loading, every frame
# timed, each event's worst frame held to a ceiling against the busy
# moment's median (render::spikes, docs/PERFORMANCE.md "Hitches"). Best of
# RUNS (default 3) per event, one valley at a time. Wall-clock: run it with
# nothing else busy on the machine.
#   scripts/hitches.sh [maps...]       (default: the three raid valleys)
set -uo pipefail
cd "$(git rev-parse --show-toplevel)"
MAPS=("$@")
[ ${#MAPS[@]} -eq 0 ] && MAPS=(marrow_creek kettle_pass cinder_works sallow_reservoir)
fail=0
for MAP in "${MAPS[@]}"; do
    echo "== $MAP"
    SPIKE_MAP="$MAP" SPIKE_RUNS="${RUNS:-3}" cargo test --profile fast --lib \
        render::spikes::tests::hitches_hold_their_ceiling -- --exact --ignored \
        --nocapture 2>&1 | grep -E ': p99|moment p50|load:|over in run|ceiling|test result'
    [ "${PIPESTATUS[0]}" -eq 0 ] || { echo "!! $MAP over its ceiling"; fail=1; }
done
exit $fail
