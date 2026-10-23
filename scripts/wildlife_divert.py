#!/usr/bin/env python3
"""Does using the wildlife get you past the machines? (docs/WILDLIFE.md,
"Using them"): the sim raider's `watch` events (`sim::raider::watch`), a
diverting run (`SIM_DIVERT=1`) against a plain one on the same seeds.

    scripts/wildlife_divert.py target/sim/divert/{on,off}

Each argument holds a directory per valley of runs (`*.jsonl`). A watch is
a calm walker within 75 m with a clear line to the bot; it's `unseen` if the
walker never got onto it in the next 30 s. Per valley and arm: watches,
the share unseen, and (diverting) how many threw into the wildlife.
"""

import json
import sys
from collections import defaultdict
from pathlib import Path


def watches(arm):
    by = defaultdict(list)
    throws = defaultdict(lambda: defaultdict(int))
    for valley in sorted(Path(arm).iterdir()):
        if not valley.is_dir():
            continue
        for path in valley.glob("*.jsonl"):
            for line in path.open():
                try:
                    ev = json.loads(line)
                except json.JSONDecodeError:
                    continue
                if ev.get("ev") == "watch":
                    by[valley.name].append(ev)
                elif ev.get("ev") == "distract":
                    throws[valley.name][ev.get("wild") or "ground"] += 1
    return by, throws


def main():
    arms = [(Path(a).name, *watches(a)) for a in sys.argv[1:]]
    valleys = sorted({v for _, by, _ in arms for v in by})
    print("| Valley | Arm | Watched | Unseen | Into the wildlife | Throws (flock / herd / ground) |")
    print("| - | - | - | - | - | - |")
    for v in valleys:
        for name, by, throws in arms:
            evs = by.get(v, [])
            unseen = sum(e["outcome"] == "unseen" for e in evs)
            wild = sum(e["wild"] for e in evs)
            share = f"{unseen / len(evs):.0%}" if evs else "-"
            t = throws.get(v, {})
            print(f"| {v} | {name} | {len(evs)} | {share} | {wild} | "
                  f"{t.get('flock', 0)} / {t.get('herd', 0)} / {t.get('ground', 0)} |")


if __name__ == "__main__":
    main()
