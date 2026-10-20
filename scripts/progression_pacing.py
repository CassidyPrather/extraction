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

The tier table walks a few thousand careers raid by raid: every raid home
adds that raid's loot (drawn from a real sim raid) to the stash, so a fetch
is often met from what's piled up; a scout or recover needs the raid to
reach a region of the tier's danger and get home. Several maps' runs can be
pooled: `progression_pacing.py all`.
"""
import glob
import json
import math
import os
import random
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


PATIENCE = int(re.search(r"FETCH_PATIENCE: u32 = (\d+)", open(os.path.join(
    ROOT, "crates/extraction_core/src/progression.rs")).read()).group(1))


def main():
    map_name = sys.argv[1] if len(sys.argv) > 1 else "marrow_creek"
    maps = ["marrow_creek", "kettle_pass", "cinder_works"] if map_name == "all" else [map_name]
    paths = []
    for m in maps:
        runs_dir = sys.argv[2] if len(sys.argv) > 2 else os.path.join(ROOT, "target", "sim", m)
        paths += [(m, p) for p in sorted(glob.glob(os.path.join(runs_dir, "*.jsonl")))]
    regs_by = {m: regions(m) for m in maps}
    runs = []
    for m, path in paths:
        regs = regs_by[m]
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
        sys.exit(f"no runs for {map_name}: scripts/sim.sh <map> 8 first")
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
    careers = []
    rng = random.Random(1)
    for _ in range(3000):
        stock = {}
        raids, at = 0, []
        for ti, t in enumerate(ts):
            lo, hi = t["danger"]
            for _job in range(t["jobs"]):
                def draw():
                    want = rng.choice(wants[min(ti, len(wants) - 1)])
                    return rng.choice(t["kinds"]), want, rng.randint(want[1], want[2])
                kind, want, n = draw()
                since = 0
                while True:
                    if kind == "Fetch" and since >= PATIENCE:
                        # The driver gives up waiting and asks for something else.
                        kind, want, n = draw()
                        since = 0
                    raids += 1
                    since += 1
                    r = rng.choice(runs)
                    if r["home"]:
                        for item in r["loot"]:
                            stock[item] = stock.get(item, 0) + 1
                    if kind == "Fetch" and stock.get(want[0], 0) >= n:
                        stock[want[0]] -= n
                        break
                    if kind != "Fetch" and r["home"] and any(lo <= d <= hi for _, d in r["visited"]):
                        break
                    if raids > 400:
                        break
            at.append(raids)
        careers.append(at)
    print("| Tier | Jobs | Danger | Raids for the tier (median) | Career so far: median (1 in 10 take) |\n| - | - | - | - | - |")
    prev = 0
    for ti, t in enumerate(ts):
        ends = sorted(c[ti] for c in careers)
        med = ends[len(ends) // 2]
        slow = ends[len(ends) * 9 // 10]
        lo, hi = t["danger"]
        where = f"{lo}-{hi}" + (" (another valley)" if t["far"] else "")
        print(f"| {ti + 1} {t['name']} | {t['jobs']} | {where} | {med - prev} | {med} ({slow}) |")
        prev = med


if __name__ == "__main__":
    main()
