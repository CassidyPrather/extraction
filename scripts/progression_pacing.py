#!/usr/bin/env python3
"""How many raids each tier of the driver's jobs takes, at the sim bots' rates.

    scripts/progression_pacing.py [map] [runs-dir]

Reads the sims' telemetry (target/sim/<map>/*.jsonl): which regions each
raid walked through (`pos` lines against the map's region polygons), how
it ended, and what came home (`end`.loot when extracted). From those, per
job kind (see crates/extraction_core/src/progression.rs, whose TIERS and
WANTS this parses):

- a fetch of n x item takes max(1, n / items-home-per-raid) raids;
- a scout or recover at danger d takes 1 / P(a raid reaches a region of
  danger d and gets home) raids;

one job a raid at most (the driver's rule). A tier is its jobs, its kinds
drawn evenly. Prints a Markdown table.
"""
import glob
import json
import math
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def regions(map_name):
    text = open(os.path.join(ROOT, "assets", "maps", map_name + ".ron")).read()
    block = text[text.index("regions:"):]
    out = []
    for m in re.finditer(r'\(name: "([^"]+)", danger: (\d)[^\n]*?polygon: \[([^\]]*)\]', block):
        pts = [tuple(map(float, p)) for p in re.findall(r"\(([-\d.]+), ([-\d.]+)\)", m.group(3))]
        out.append((m.group(1), int(m.group(2)), pts))
    return out


def inside(poly, x, z):
    hit = False
    j = len(poly) - 1
    for i in range(len(poly)):
        xi, zi = poly[i]
        xj, zj = poly[j]
        if (zi > z) != (zj > z) and x < (xj - xi) * (z - zi) / (zj - zi + 1e-9) + xi:
            hit = not hit
        j = i
    return hit


def region_of(regs, x, z):
    for name, danger, poly in regs:
        if inside(poly, x, z):
            return name, danger
    return None, None


def tiers():
    src = open(os.path.join(ROOT, "crates/extraction_core/src/progression.rs")).read()
    body = src[src.index("pub const TIERS"):src.index("pub const STANDING")]
    out = []
    for t in re.finditer(
        r'name: "([^"]+)",\s*jobs: (\d+),\s*danger: \((\d), (\d)\),\s*far: (\w+),\s*kinds: &\[([^\]]*)\]',
        body,
    ):
        kinds = re.findall(r"JobKind::(\w+)", t.group(6))
        out.append(dict(name=t.group(1), jobs=int(t.group(2)), danger=(int(t.group(3)), int(t.group(4))),
                        far=t.group(5) == "true", kinds=kinds))
    wants_src = src[src.index("pub const WANTS"):src.index("pub fn why")]
    wants = []
    for tier in wants_src.split("&[\n")[1:]:
        wants.append([(m[0], int(m[1]), int(m[2]))
                      for m in re.findall(r'"([a-z_]+)",\s*(\d+),\s*(\d+),', tier)])
    return out, wants


def main():
    map_name = sys.argv[1] if len(sys.argv) > 1 else "marrow_creek"
    runs_dir = sys.argv[2] if len(sys.argv) > 2 else os.path.join(ROOT, "target", "sim", map_name)
    regs = regions(map_name)
    runs = []
    for path in sorted(glob.glob(os.path.join(runs_dir, "*.jsonl"))):
        visited, end, personality = set(), None, "?"
        for line in open(path):
            try:
                e = json.loads(line)
            except ValueError:
                continue
            if e["ev"] == "start":
                personality = e.get("personality", "?")
            elif e["ev"] == "pos":
                name, danger = region_of(regs, e["x"], e["z"])
                if name:
                    visited.add((name, danger))
            elif e["ev"] == "end":
                end = e
        if end:
            runs.append(dict(visited=visited, home=end["outcome"] == "extracted",
                             loot=end.get("loot", []) if end["outcome"] == "extracted" else [],
                             minutes=end.get("raid_time", 0) / 60.0, personality=personality))
    if not runs:
        sys.exit(f"no runs in {runs_dir}: scripts/sim.sh {map_name} 8 first")
    n = len(runs)
    home = sum(r["home"] for r in runs) / n

    def reach(lo, hi):
        return sum(r["home"] and any(lo <= d <= hi for _, d in r["visited"]) for r in runs) / n

    def rate(item):
        return sum(r["loot"].count(item) for r in runs) / n

    ts, wants = tiers()
    print(f"{map_name}: {n} sim raids, {home:.0%} home, "
          f"{sum(r['minutes'] for r in runs) / n:.0f} min a raid on average\n")
    print("| Danger reached and home | Share of raids |\n| - | - |")
    for d in range(4):
        print(f"| {d} | {reach(d, d):.0%} |")
    print()
    print("| Fetch (tier) | Home per raid | Raids for the job |\n| - | - | - |")
    fetch_cost = []
    for ti, tier_wants in enumerate(wants):
        costs = []
        for item, lo, hi in tier_wants:
            r = rate(item)
            need = (lo + hi) / 2
            c = max(1.0, need / r) if r > 0 else math.inf
            costs.append(c)
            print(f"| {item} x{need:g} ({ti + 1}) | {r:.2f} | {c:.1f} |")
        fin = [c for c in costs if math.isfinite(c)]
        fetch_cost.append(sum(fin) / len(fin) if fin else math.inf)
    print()
    print("| Tier | Jobs | Danger | Raids a job | Raids for the tier | Career so far |\n| - | - | - | - | - | - |")
    total = 0.0
    for ti, t in enumerate(ts):
        lo, hi = t["danger"]
        p = reach(lo, hi)
        place = 1 / p if p > 0 else math.inf
        kinds = []
        for k in t["kinds"]:
            kinds.append(fetch_cost[min(ti, len(fetch_cost) - 1)] if k == "Fetch" else place)
        per = sum(kinds) / len(kinds)
        raids = per * t["jobs"]
        total += raids
        where = f"{lo}-{hi}" + (" (another valley)" if t["far"] else "")
        print(f"| {ti + 1} {t['name']} | {t['jobs']} | {where} | {per:.1f} | {raids:.0f} | {total:.0f} |")


if __name__ == "__main__":
    main()
