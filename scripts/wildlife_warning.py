#!/usr/bin/env python3
"""How often the valley warns you a machine is coming (docs/WILDLIFE.md,
"Warning"): the sim's `approach` events (`wildlife::warning`), added up.

    scripts/wildlife_warning.py target/sim/warning/*/

Each argument is a directory of runs (`*.jsonl`) for one valley. For each
valley, and each robot in it: how many approaches, the share warned at
least 5 s before the robot had you in sight, the median lead, and which
cues did it.
"""

import json
import statistics
import sys
from collections import Counter, defaultdict
from pathlib import Path


def approaches(directory):
    runs = 0
    out = []
    for path in sorted(Path(directory).glob("*.jsonl")):
        runs += 1
        for line in path.open():
            try:
                ev = json.loads(line)
            except json.JSONDecodeError:
                continue
            if ev.get("ev") == "approach" or ev.get("event") == "approach":
                out.append(ev)
    return runs, out


def row(name, evs):
    warned = [e for e in evs if e["warned"]]
    leads = [e["lead"] for e in warned]
    cues = Counter(c for e in warned for c in e["cues"])
    share = len(warned) / len(evs) if evs else 0.0
    lead = f"{statistics.median(leads):.0f}" if leads else "-"
    cue = ", ".join(f"{c} {n}" for c, n in cues.most_common()) or "-"
    return f"| {name} | {len(evs)} | {share:.0%} | {lead} | {cue} |"


def main():
    print("| Valley / robot | Approaches | Warned | Median lead (s) | Cues |")
    print("| - | - | - | - | - |")
    for d in sys.argv[1:]:
        runs, evs = approaches(d)
        name = Path(d).name or Path(d).parent.name
        print(row(f"**{name}** ({runs} raids)", evs))
        by = defaultdict(list)
        for e in evs:
            by[e["robot"]].append(e)
        for robot, es in sorted(by.items(), key=lambda kv: -len(kv[1])):
            print(row(f"  {robot}", es))


if __name__ == "__main__":
    main()
