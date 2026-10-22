#!/usr/bin/env python3
"""Checks a batch of sim raids for harvesting that is risky but not
murderous: a share of deliberate dives into anomalies end in death, and a
share of them bring a stone out; and that anomalies are part of a raid at
all: a share of the raids dive at least once.

    scripts/sim.sh marrow_creek 9 --personality hunter --jobs 3 --sim-minutes 10
    scripts/harvest_band.py target/sim/marrow_creek

Exits non-zero when out of band. Deaths count against harvesting when
their cause is an anomaly kind.
"""
import collections
import glob
import json
import os
import sys

KINDS = {"Lift", "Burner", "Snare", "Well", "Mirror", "Hush", "Frost"}
# Each kind's own stone.
STONE = {
    "Lift": "art_featherstone",
    "Burner": "art_hearth",
    "Snare": "art_aegis",
    "Well": "art_lodestone",
    "Mirror": "art_shard",
    "Hush": "art_hushstone",
    "Frost": "art_rimestone",
}
# Deaths in anomalies per deliberate dive: some, never most.
MAX_DEATHS_PER_DIVE = 0.2
# Stones brought out per deliberate dive.
MIN_STONES_PER_DIVE = 0.3
# Hunter raids that dive into at least one anomaly: anomalies are part of a
# hunter's raid, not something it meets by chance.
MIN_DIVING_RAIDS = 0.5


def main(path: str) -> int:
    runs = sorted(glob.glob(os.path.join(path, "*.jsonl")))
    if not runs:
        print(f"no runs in {path}")
        return 2
    dives = 0
    out = collections.Counter()
    hurt = collections.Counter()
    by_kind = collections.Counter()
    deaths = collections.Counter()
    stones = collections.Counter()
    diving = 0
    for r in runs:
        dived = False
        with open(r) as f:
            for line in f:
                e = json.loads(line)
                ev = e.get("ev") or e.get("event")
                if ev == "anomaly_enter" and e.get("deliberate"):
                    dives += 1
                    dived = True
                    by_kind[e.get("kind", "?")] += 1
                elif ev == "death":
                    deaths[e.get("cause", "?")] += 1
                elif ev == "damage" and e.get("cause") in KINDS:
                    hurt[e["cause"]] += float(e.get("amount", 0.0))
                elif ev == "loot" and str(e.get("item", "")).startswith("art_"):
                    stones[e["item"]] += 1
                elif ev == "end" and e.get("outcome") == "extracted":
                    for item in e.get("loot") or []:
                        if str(item).startswith("art_"):
                            out[item] += 1
        diving += dived
    harvest_deaths = sum(n for c, n in deaths.items() if c in KINDS)
    taken = sum(stones.values())
    per_dive = harvest_deaths / dives if dives else 0.0
    yield_ = taken / dives if dives else 0.0
    share = diving / len(runs)
    print(f"{len(runs)} runs, {dives} deliberate dives")
    print(f"raids that dive at least once: {diving} ({share:.0%})")
    print(f"deaths in anomalies: {harvest_deaths} ({per_dive:.0%} a dive); all deaths {dict(deaths)}")
    print(f"stones taken: {taken} ({yield_:.0%} a dive) {dict(stones)}")
    print(f"stones brought out (raids that extracted): {sum(out.values())} {dict(out)}")
    lost = sum(hurt.values())
    print(f"health lost in anomalies: {lost:.0f} ({lost / max(dives, 1):.0f} hp a dive)")
    for kind in sorted(KINDS):
        if by_kind[kind] or deaths[kind]:
            got = stones[STONE[kind]]
            print(
                f"  {kind:7} {by_kind[kind]:3} dives, {deaths[kind]:2} deaths,"
                f" {got:2} stones taken ({got / max(by_kind[kind], 1):.0%}),"
                f" {hurt[kind] / max(by_kind[kind], 1):4.0f} hp lost a dive"
            )
    ok = True
    if dives == 0:
        print("FAIL no deliberate dives: nobody tried")
        ok = False
    if share < MIN_DIVING_RAIDS:
        print(f"FAIL fewer than {MIN_DIVING_RAIDS:.0%} of raids dive at all")
        ok = False
    if per_dive > MAX_DEATHS_PER_DIVE:
        print(f"FAIL more than {MAX_DEATHS_PER_DIVE:.0%} of dives kill")
        ok = False
    if dives and yield_ < MIN_STONES_PER_DIVE:
        print(f"FAIL fewer than {MIN_STONES_PER_DIVE:.0%} of dives bring a stone out")
        ok = False
    print("harvest band OK" if ok else "harvest band out of range")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/sim/marrow_creek"))
