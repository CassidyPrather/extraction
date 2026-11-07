#!/usr/bin/env bash
# One headless scenario run twice from fresh data dirs, on one thread and on
# four (EXTRACTION_THREADS), and the two runs compared: the same data
# (their `assets:` lines, `asset_identity`) and the same play, line for
# line, once timestamps, colours, wall-clock durations and the raids'
# file names are taken out. Exits non-zero, with the diff, if they part.
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
for t in 1 4; do
  d="$out/t$t"
  mkdir -p "$d/data"
  printf 'yes\n' > "$d/data/trained"
  EXTRACTION_THREADS=$t RUST_LOG=info timeout 1800 "$exe" --headless "${args[@]}" \
    --data-dir "$d/data" --save "$d/data/career.ron" > "$d/log.txt" 2>&1
  echo "threads $t: exit $?"
  # Timestamps and colours, durations in wall time, and the raids' file
  # names (their stamps) differ run to run whatever the play.
  sed -E 's/\x1b\[[0-9;]*m//g; s/^[0-9T:.-]+Z +//;
          s/ in [0-9.]+(ms|µs|s)\b/ in T/g; s/[0-9.]+ ?(ms|µs)//g;
          s/at [0-9.]+ s \(frame/at T s (frame/g;
          s/raid[0-9]+-[0-9]+\.jsonl/raidN.jsonl/g;
          s#'"$out"'/t[0-9]#OUT#g' "$d/log.txt" \
    | grep -vE "took|elapsed|fps|ms/frame|frame time" > "$d/norm.txt"
done
a1=$(grep -m1 '^INFO .*assets: data' "$out/t1/norm.txt" | sed 's/.*assets: //')
a4=$(grep -m1 '^INFO .*assets: data' "$out/t4/norm.txt" | sed 's/.*assets: //')
echo "assets (1 thread): ${a1:-none}"
echo "assets (4 threads): ${a4:-none}"
if [ -z "$a1" ] || [ "$a1" != "$a4" ]; then
  echo "FAIL: the two runs didn't read the same data (or didn't say)"
  exit 2
fi
if diff -q "$out/t1/norm.txt" "$out/t4/norm.txt" > /dev/null; then
  echo "OK: the same play on 1 thread and 4 ($(wc -l < "$out/t1/norm.txt") lines; $out)"
  exit 0
fi
echo "FAIL: 1 thread and 4 parted ($out):"
diff "$out/t1/norm.txt" "$out/t4/norm.txt" | head -40
exit 1
