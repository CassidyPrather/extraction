#!/usr/bin/env bash
# One headless scenario run three times from fresh data dirs: on one
# thread, on four (EXTRACTION_THREADS), and on four beside eight busy
# processes. The runs are compared: the same data (their `assets:` lines,
# `asset_identity`), the same play, line for line once timestamps,
# colours, wall-clock durations and the raids' file names are taken out,
# and the same saved career, byte for byte. Exits non-zero, with the
# diff, if they part.
#
#   scripts/determinism.sh [EXE] [-- SCENARIO ARGS...]
#
# EXE defaults to target/fast/extraction. The scenario defaults to the
# probe that showed a run depends on its data, not its machine (6
# November: one binary's first_fight went from 26/40 to 9/40 when its
# checkout's item boxes changed; on one data set, 1 thread and 4 match):
#   --scenario first_raid --personality reckless --encounter walker --map marrow_creek
# A run is given --data-dir and --save in its own fresh dir, already
# through the yard.
set -uo pipefail
exe=${1:-target/fast/extraction}
shift || true
[ "${1:-}" = "--" ] && shift
args=("$@")
if [ ${#args[@]} -eq 0 ]; then
  args=(--scenario first_raid --personality reckless --encounter walker --map marrow_creek)
fi
out=$(mktemp -d "${TMPDIR:-/tmp}/determinism.XXXXXX")
# Third, four threads again beside eight busy processes, for anything
# paced by the machine. And every run's save compared byte for byte: the
# same play can be written in another order. Progression's wanderer r14,
# 7 November: two bags of remains swapped in the save, because they went
# through a std HashMap, whose RandomState is new in every process. Any
# two runs of a raid leaving two or more bags would show it (fa596c46
# keys them in a BTreeMap); the default probe leaves none, so give a
# scenario that does to test that.
for t in 1 4 4load; do
  d="$out/t$t"
  mkdir -p "$d/data"
  printf 'yes\n' > "$d/data/trained"
  busy=()
  if [ "$t" = 4load ]; then
    for _ in 1 2 3 4 5 6 7 8; do yes > /dev/null & busy+=($!); done
  fi
  EXTRACTION_THREADS=${t%load} RUST_LOG=info timeout 1800 "$exe" --headless "${args[@]}" \
    --data-dir "$d/data" --save "$d/data/career.ron" > "$d/log.txt" 2>&1
  echo "threads $t: exit $?"
  [ ${#busy[@]} -gt 0 ] && kill "${busy[@]}" 2> /dev/null
  # Timestamps and colours, durations in wall time, and the raids' file
  # names (their stamps) differ run to run whatever the play.
  sed -E 's/\x1b\[[0-9;]*m//g; s/^[0-9T:.-]+Z +//;
          s/ in [0-9.]+(ms|µs|s)\b/ in T/g; s/[0-9.]+ ?(ms|µs)//g;
          s/at [0-9.]+ s \(frame/at T s (frame/g;
          s/raid[0-9]+-[0-9]+\.jsonl/raidN.jsonl/g;
          s#'"$out"'/t[0-9]+(load)?#OUT#g' "$d/log.txt" \
    | grep -vE "took|elapsed|fps|ms/frame|frame time" > "$d/norm.txt"
done
fail=0
a1=$(grep -m1 '^INFO .*assets: data' "$out/t1/norm.txt" | sed 's/.*assets: //')
for t in 4 4load; do
  a=$(grep -m1 '^INFO .*assets: data' "$out/t$t/norm.txt" | sed 's/.*assets: //')
  if [ -z "$a1" ] || [ "$a1" != "$a" ]; then
    echo "FAIL: 1 thread and $t didn't read the same data (or didn't say): ${a1:-none} / ${a:-none}"
    exit 2
  fi
done
echo "assets: $a1"
for t in 4 4load; do
  if diff -q "$out/t1/norm.txt" "$out/t$t/norm.txt" > /dev/null; then
    echo "play: 1 thread and $t the same ($(wc -l < "$out/t1/norm.txt") lines)"
  else
    echo "FAIL: play parted, 1 thread and $t:"
    diff "$out/t1/norm.txt" "$out/t$t/norm.txt" | head -40
    fail=1
  fi
done
# The save's bytes too: the same play can be written in another order.
s1=$(sha256sum < "$out/t1/data/career.ron" 2> /dev/null | cut -c1-16)
for t in 4 4load; do
  s=$(sha256sum < "$out/t$t/data/career.ron" 2> /dev/null | cut -c1-16)
  if [ -n "$s1" ] && [ "$s1" = "$s" ]; then
    echo "save: 1 thread and $t byte for byte ($s1)"
  else
    echo "FAIL: save bytes parted, 1 thread and $t: ${s1:-none} / ${s:-none}"
    diff <(tr ',' '\n' < "$out/t1/data/career.ron") <(tr ',' '\n' < "$out/t$t/data/career.ron") | head -20
    fail=1
  fi
done
if [ $fail = 0 ]; then
  echo "OK: the same data, play and save on 1 thread, 4, and 4 under load ($out)"
fi
exit $fail
