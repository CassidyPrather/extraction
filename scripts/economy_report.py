#!/usr/bin/env python3
"""What fills and drains the stash, and how the late game kills, from sims.

    scripts/economy_report.py <runs-dir> [<runs-dir> ...]

Each runs dir holds sim telemetry (`*.jsonl`, any depth). Reads each run's
`end` line: what came home (`loot`, when extracted), how it ended
(`outcome`, `death_cause`), and the driver's job it had (`job`: tier, and
`armour` when run with SIM_ARMOUR). Prints Markdown:

- income per raid by item, beside what drains it (bench recipes, upkeep,
  the driver's fetches) parsed from the Rust, and the items that pile up
  with nothing to eat them;
- loot-table items that never came home;
- deaths by cause, by ladder stage, armoured or not.
"""
import collections
import glob
import json
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


# What the game eats besides the bench, upkeep and fetches, by category:
# cooking (`raid/kitchen.rs`), offerings (`anomaly/offering.rs`), the
# artifact adapter, and things used up in the hand (`weapons/flat.rs`).
CATEGORY_SINKS = {
    "Food": ["kitchen", "offering:calm"],
    "Curio": ["offering:information"],
    "Artifact": ["offering:time", "carried"],
}
USED = {
    "cleaning_kit": ["wears out cleaning guns"],
    "gun_oil": ["wears out oiling guns"],
    "spare_parts": ["field repair"],
    "field_dressing": ["bandaging"],
    "regen_injector": ["healing"],
    "power_cell": ["drains in lights, lasers, cloaks"],
    "emp_grenade": ["thrown"],
    "emp_trap": ["set"],
    "rgd5": ["thrown"],
    "f1": ["thrown"],
    "matches": ["kitchen"],
    "fuel_can": ["short exfil"],
    "bolt": ["thrown at anomalies"],
}


def runs(dirs):
    out = []
    for d in dirs:
        for path in glob.glob(os.path.join(d, "**", "*.jsonl"), recursive=True):
            end, personality = None, "?"
            for line in open(path):
                try:
                    e = json.loads(line)
                except ValueError:
                    continue
                if e["ev"] == "start":
                    personality = e.get("personality", "?")
                elif e["ev"] == "end":
                    end = e
            if end:
                end["personality"] = personality
                out.append(end)
    return out


def sinks():
    """Item id -> what eats it: recipes, upkeep, fetches (from the Rust)."""
    eats = collections.defaultdict(set)
    fab = open(os.path.join(ROOT, "crates/extraction_core/src/fabrication.rs")).read()
    for rid, needs in re.findall(r'id: "([a-z_0-9]+)",[^}]*?needs: &\[(.*?)\],\s*makes', fab, re.S):
        for item in re.findall(r'Need::Item\("([a-z_0-9]+)"\)', needs):
            eats[item].add(f"bench:{rid}")
        if "AmmoBox" in needs:
            eats["<ammo box>"].add(f"bench:{rid}")
    prog = open(os.path.join(ROOT, "crates/extraction_core/src/progression.rs")).read()
    body = prog[prog.index("pub fn upkeep(self)"):]
    body = body[:body.index("\n    }\n")]
    for item in re.findall(r'\("([a-z_]+)",\s*\d+\)', body):
        eats[item].add("upkeep")
    wants = prog[prog.index("pub const WANTS"):prog.index("pub const ANY_PARTS")]
    for item in re.findall(r'"([a-z_0-9]+)",\s*\d+,\s*\d+,', wants):
        eats[item].add("fetch")
    parts = re.search(r"pub const PARTS: &\[&str\] = &\[([^\]]*)\]", prog).group(1)
    for item in re.findall(r'"([a-z_0-9]+)"', parts):
        eats[item].add("fetch (any part)")
    return eats


def catalog():
    """Item id -> category, from items.ron."""
    text = open(os.path.join(ROOT, "assets/data/items.ron")).read()
    out = {}
    for m in re.finditer(r'id: "([a-z_0-9]+)".*?category: (\w+)', text, re.S):
        out.setdefault(m.group(1), m.group(2))
    return out


def loot_items():
    text = open(os.path.join(ROOT, "assets/data/loot.ron")).read()
    return set(re.findall(r'Item\(id: "([a-z_0-9]+)"', text))


def main():
    dirs = sys.argv[1:] or [os.path.join(ROOT, "target", "sim")]
    rs = runs(dirs)
    if not rs:
        sys.exit("no runs")
    n = len(rs)
    home = [r for r in rs if r["outcome"] == "extracted"]
    cat = catalog()
    eats = sinks()
    income = collections.Counter(i for r in home for i in r.get("loot", []))
    print(f"{n} sim raids, {len(home)} home ({len(home) / n:.0%}).\n")

    print("### Income per raid, and what eats it\n")
    print("| Item | Category | Home per raid | Eaten by |\n| - | - | - | - |")
    for item, c in income.most_common():
        sink = sorted(eats.get(item, ()))
        if cat.get(item) == "Ammo" and item.startswith("ammo_"):
            sink.append("guns; bench:pull_box")
        sink += CATEGORY_SINKS.get(cat.get(item), [])
        sink += USED.get(item, [])
        print(f"| {item} | {cat.get(item, '?')} | {c / n:.2f} | {', '.join(sink) or '**nothing**'} |")
    print()
    by_cat = collections.Counter()
    for item, c in income.items():
        by_cat[cat.get(item, "?")] += c
    print("| Category | Home per raid |\n| - | - |")
    for k, c in by_cat.most_common():
        print(f"| {k} | {c / n:.2f} |")
    print()
    never = sorted(i for i in loot_items() if income[i] == 0)
    print(f"Loot-table items that never came home in {n} raids: {', '.join(never) or 'none'}.\n")

    print("### Deaths by stage\n")
    stage = lambda r: (r.get("job") or {}).get("tier")
    names = {0: "1 Odd jobs", 1: "2 Errands", 2: "3 Runs", 3: "4 Far jobs", 4: "Deep jobs"}
    print("| Stage | Armour | Raids | Home | Deaths by cause |\n| - | - | - | - | - |")
    groups = collections.defaultdict(list)
    for r in rs:
        groups[(stage(r), (r.get("job") or {}).get("armour") or "none")].append(r)
    for (t, arm), g in sorted(groups.items(), key=lambda kv: (kv[0][0] is None, kv[0])):
        dead = collections.Counter(r.get("death_cause") for r in g if r["outcome"] == "died")
        h = sum(r["outcome"] == "extracted" for r in g)
        armoured = "yes" if arm == "late" and t is not None and t >= 2 else "no"
        print(f"| {names.get(t, '-')} | {armoured} | {len(g)} | {h / len(g):.0%} | "
              f"{', '.join(f'{k} {v}' for k, v in dead.most_common()) or '-'} |")


if __name__ == "__main__":
    main()
