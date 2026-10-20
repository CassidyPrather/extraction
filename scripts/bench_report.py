#!/usr/bin/env python3
"""Table of a frame-budget benchmark (scripts/bench.sh): one row a view,
per eye, with what's over the budget marked.
    python3 scripts/bench_report.py target/bench/marrow_creek [...]
    python3 scripts/bench_report.py --compare BEFORE_DIR AFTER_DIR
"""
import json
import sys
from pathlib import Path


def load(path):
    p = Path(path)
    if p.suffix != ".jsonl":
        p = p.with_suffix(".jsonl")
    if not p.exists():
        return []
    return [json.loads(l) for l in p.read_text().splitlines() if l.strip()]


def gpu_total(row):
    # Top-level passes make the frame (nested ones are inside them).
    return sum(v for k, v in row.get("gpu_ms", {}).items() if "/" not in k)


COLS = [
    ("draws", lambda r: r["draws"]),
    ("tris k", lambda r: r["tris"] / 1000),
    ("sh draws", lambda r: r["shadow_draws"]),
    ("sh tris k", lambda r: r["shadow_tris"] / 1000),
    ("cpu ms", lambda r: r["cpu_main_ms"]),
    ("phys ms", lambda r: r["cpu_physics_ms"]),
    ("gpu ms", gpu_total),
    ("frame ms", lambda r: r.get("frame_ms", 0)),
]


def table(rows):
    head = f"{'view':<28}" + "".join(f"{c:>10}" for c, _ in COLS)
    print(head)
    for r in rows:
        line = f"{r['map'][:6] + ' ' + r['view'][:21]:<28}"
        line += "".join(f"{f(r):>10.1f}" for _, f in COLS)
        if r.get("over"):
            line += "  OVER: " + ", ".join(r["over"])
        print(line)


def compare(before, after):
    b = {(r["map"], r["view"]): r for r in before}
    print(f"{'view':<28}" + "".join(f"{c:>18}" for c, _ in COLS))
    for r in after:
        o = b.get((r["map"], r["view"]))
        if not o:
            continue
        line = f"{r['map'][:6] + ' ' + r['view'][:21]:<28}"
        for _, f in COLS:
            line += f"{f'{f(o):.0f} > {f(r):.0f}':>18}"
        print(line)


def main():
    args = sys.argv[1:]
    if args and args[0] == "--compare":
        bdir, adir = Path(args[1]), Path(args[2])
        before = [r for p in sorted(bdir.glob("*.jsonl")) for r in load(p)]
        after = [r for p in sorted(adir.glob("*.jsonl")) for r in load(p)]
        compare(before, after)
        return
    rows = [r for a in args for r in load(a)]
    table(rows)
    over = [r for r in rows if r.get("over")]
    print(f"{len(rows)} views, {len(over)} over the budget")


if __name__ == "__main__":
    main()
