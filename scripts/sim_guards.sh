#!/usr/bin/env bash
# Sim guards: named replays whose telemetry must keep saying what it does.
# Each runs one smoke raid (`SIM_NO_JOB=1`, as the Verifier's smoke does)
# and checks a line of it; a failing guard prints why and the script exits
# non-zero. Runs are deterministic per build, so a guard that flips is the
# build's doing.
#
#   scripts/sim_guards.sh [path/to/extraction]   (default target/fast/extraction)
set -uo pipefail

BIN="${1:-target/fast/extraction}"
OUT="$(mktemp -d)"
trap 'rm -rf "$OUT"' EXIT
FAILED=0

run() { # map seed -> jsonl path
    local dir="$OUT/$1-$2"
    mkdir -p "$dir"
    SIM_NO_JOB=1 timeout 1500 "$BIN" --sim-run smoke --map "$1" --seed "$2" \
        --sim-out "$dir" --headless >/dev/null 2>&1
    echo "$dir/smoke.jsonl"
}

guard() { # name ok why
    if [ "$2" = 1 ]; then
        echo "PASS $1"
    else
        echo "FAIL $1: $3"
        FAILED=1
    fi
}

# Sallow seed 5: the raider's first crawler, a dead part in front of its
# core, is wrecked without emptying the gun into it (31 October: 59 rounds
# into the dead emitter, out of ammunition at 102 s, dead at 278 s;
# `bot::HITS_TO_SHIFT`).
f="$(run sallow_reservoir 5)"
first="$(grep '"engage_end"' "$f" | grep '"robot":"Crawler"' | head -1)"
ok=0
if echo "$first" | grep -q '"outcome":"wrecked"'; then
    shots="$(echo "$first" | grep -o '"shots":[0-9]*' | cut -d: -f2)"
    [ "${shots:-99}" -lt 30 ] && ok=1
fi
guard "sallow_reservoir seed 5: first crawler wrecked, under a magazine" "$ok" "${first:-no crawler fight}"

exit $FAILED
