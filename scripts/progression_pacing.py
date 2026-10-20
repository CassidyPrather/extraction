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
    # The ladder, then the standing (deep) jobs after it.
    body = src[src.index("pub const TIERS"):src.index("/// The stash crate's room")]
    out = []
    for t in re.finditer(
        r'name: "([^"]+)",\s*jobs: (\d+),\s*danger: \((\d), (\d)\),\s*far: (\w+),\s*kinds: &\[([^\]]*)\],\s*fits: &\[([^\]]*)\]',
        body,
    ):
        kinds = re.findall(r"JobKind::(\w+)", t.group(6))
        names = {"StashRack": "stash_rack", "BenchVice": "bench_vice", "Aerial": "aerial",
                 "MedCabinet": "med_cabinet", "StashRack2": "stash_rack_2"}
        fits = [names[u] for u in re.findall(r"Upgrade::(\w+)", t.group(7))]
        out.append(dict(name=t.group(1), jobs=int(t.group(2)), danger=(int(t.group(3)), int(t.group(4))),
                        far=t.group(5) == "true", kinds=kinds, fits=fits))
    wants_src = src[src.index("pub const WANTS"):src.index("pub const ANY_PARTS")]
    wants = []
    for tier in wants_src.split("&[\n")[1:]:
        wants.append([(m[0] or "robot_parts", int(m[1]), int(m[2]))
                      for m in re.findall(r'(?:"([a-z_]+)"|ANY_PARTS),\s*(\d+),\s*(\d+),', tier)])
    return out[:-1], wants, out[-1]


PRIOR = 4
# Careers are walked to this many raids; "after the ladder" counts from LADDER_TO.
HORIZON = 60
LADDER_TO = 30

PATIENCE = int(re.search(r"FETCH_PATIENCE: u32 = (\d+)", open(os.path.join(
    ROOT, "crates/extraction_core/src/progression.rs")).read()).group(1))


PARTS = re.findall(r'"([a-z_]+)"', re.search(r"pub const PARTS: &\[&str\] = &\[([^\]]*)\]", open(
    os.path.join(ROOT, "crates/extraction_core/src/progression.rs")).read()).group(1))


def upkeep():
    """Each fitting's upkeep, as `Upgrade::upkeep` and `UPKEEP_EVERY` have it."""
    src = open(os.path.join(ROOT, "crates/extraction_core/src/progression.rs")).read()
    every = int(re.search(r"UPKEEP_EVERY: u32 = (\d+)", src).group(1))
    id_fn = src[src.index("pub fn id(self)"):]
    ids = dict(re.findall(r"Upgrade::(\w+) => \"([a-z_0-9]+)\",\n", id_fn[:id_fn.index("\n    }\n")]))
    body = src[src.index("pub fn upkeep(self)"):]
    body = body[:body.index("\n    }\n")]
    out = {}
    for arms, parts in re.findall(r"((?:Upgrade::\w+\s*\|?\s*)+)=> &\[([^\]]*)\]", body):
        p = [(i, int(n)) for i, n in re.findall(r'\("([a-z_]+)",\s*(\d+)\)', parts)]
        for u in re.findall(r"Upgrade::(\w+)", arms):
            out[ids[u]] = p
    return every, out


UPKEEP_EVERY, UPKEEP = upkeep()


def takes(want, item):
    return want == item or (want == "robot_parts" and item in PARTS)


def main():
    map_name = sys.argv[1] if len(sys.argv) > 1 else "marrow_creek"
    maps = ["marrow_creek", "kettle_pass", "cinder_works"] if map_name == "all" else [map_name]
    paths = []
    for m in maps:
        if len(sys.argv) > 2:
            # A runs dir; with `all`, a dir holding one folder per map
            # (and they may hold a folder per batch).
            runs_dir = os.path.join(sys.argv[2], m) if map_name == "all" else sys.argv[2]
        else:
            runs_dir = os.path.join(ROOT, "target", "sim", m)
        paths += [(m, p) for p in sorted(glob.glob(os.path.join(runs_dir, "*.jsonl"))
                                          + glob.glob(os.path.join(runs_dir, "*", "*.jsonl")))]
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
            runs.append(dict(visited=visited, home=end["outcome"] == "extracted", job=end.get("job"),
                             loot=end.get("loot", []) if end["outcome"] == "extracted" else [],
                             minutes=end.get("raid_time", 0) / 60.0, personality=personality))
    if not runs:
        sys.exit(f"no runs for {map_name}: scripts/sim.sh <map> 8 first")
    n = len(runs)
    home = sum(r["home"] for r in runs) / n

    def reach(lo, hi):
        return sum(r["home"] and any(lo <= d <= hi for _, d in r["visited"]) for r in runs) / n

    def income(item):
        return sum(r["loot"].count(item) for r in runs) / len(runs)

    def rate(item):
        return sum(sum(takes(item, i) for i in r["loot"]) for r in runs) / n

    # Runs where the bot had the driver's scout job (the game hands sim bots
    # one): how often it saw the place and got home, by the place's danger.
    jobs = [r for r in runs if r["job"]]
    job_rate = {}
    if jobs:
        print("| Scout job, danger | Bots given it | Seen and home |\n| - | - | - |")
        for d in range(4):
            js = [r for r in jobs if r["job"].get("danger") == d]
            if js:
                done = sum(r["home"] and r["job"]["seen"] for r in js)
                # A few runs say little: lean on "reached that danger and
                # got home" as a prior worth PRIOR runs.
                job_rate[d] = (done + PRIOR * reach(d, d)) / (len(js) + PRIOR)
                print(f"| {d} | {len(js)} | {done / len(js):.0%} (used: {job_rate[d]:.0%}) |")
        print()

    def place_odds(lo, hi):
        """A place job's chance per raid: what bots that had the job did, else
        the share of raids that reached that danger and got home."""
        known = [job_rate[d] for d in range(lo, hi + 1) if d in job_rate]
        if known:
            return sum(known) / len(known)
        return reach(lo, hi)

    ts, wants, STANDING = tiers()
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
    fits = [t["fits"] for t in ts]
    careers, after = [], []
    rng = random.Random(1)
    for _ in range(3000):
        stock = {}
        raids, at = 0, []
        due, laid = {}, {}   # fitting -> raid due; fitting -> raids laid up (30-60)
        jobs_after = 0

        def homecoming():
            """A raid's end: its loot in the stash, then upkeep."""
            for f in [f for tf in fits[:len(at)] for f in tf]:
                if f not in due:
                    due[f] = raids + UPKEEP_EVERY
                    continue
                if raids >= due[f]:
                    parts = UPKEEP[f]
                    if all(stock.get(i, 0) >= n for i, n in parts):
                        for i, n in parts:
                            stock[i] -= n
                        due[f] = raids + UPKEEP_EVERY
                    elif raids > LADDER_TO:
                        laid[f] = laid.get(f, 0) + 1

        def job(ti, t):
            nonlocal raids
            lo, hi = t["danger"]

            def draw():
                want = rng.choice(wants[min(ti, len(wants) - 1)])
                return rng.choice(t["kinds"]), want, rng.randint(want[1], want[2])
            kind, want, n = draw()
            since = 0
            while raids < HORIZON:
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
                homecoming()
                have = sum(c for i, c in stock.items() if takes(want[0], i))
                if kind == "Fetch" and have >= n:
                    for i in list(stock):
                        while n and stock[i] and takes(want[0], i):
                            stock[i] -= 1
                            n -= 1
                    return True
                if kind != "Fetch" and rng.random() < place_odds(lo, hi):
                    return True
            return False

        for ti, t in enumerate(ts):
            for _job in range(t["jobs"]):
                job(ti, t)
            at.append(raids)
        careers.append(at)
        # Past the ladder: deep jobs until HORIZON raids.
        start = raids
        while raids < HORIZON and job(len(ts), STANDING):
            if raids > LADDER_TO:
                jobs_after += 1
        span = max(1, HORIZON - max(start, LADDER_TO))
        after.append((jobs_after, span, {f: laid.get(f, 0) / span for f in UPKEEP}))
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
    print()
    done = [a for a in after if a[1] > 1]
    if done:
        rate = sum(a[0] for a in done) / sum(a[1] for a in done)
        print(f"After the ladder (raids {LADDER_TO}-{HORIZON}): a deep job every "
              f"{1 / rate if rate else float('inf'):.1f} raids.\n")
        print("| Fitting | Upkeep every %d raids | Share of raids laid up |\n| - | - | - |" % UPKEEP_EVERY)
        for f, parts in UPKEEP.items():
            share = sum(a[2][f] for a in done) / len(done)
            what = ", ".join(f"{n} {i}" for i, n in parts)
            print(f"| {f} | {what} | {share:.0%} |")
        print()
        print("| Part | Home per raid | Upkeep per raid (all fitted) | Share of income |\n| - | - | - | - |")
        need = {}
        for parts in UPKEEP.values():
            for i, n in parts:
                need[i] = need.get(i, 0) + n / UPKEEP_EVERY
        for i, per in need.items():
            got = income(i)
            print(f"| {i} | {got:.2f} | {per:.2f} | {per / got if got else float('inf'):.0%} |")


if __name__ == "__main__":
    main()
