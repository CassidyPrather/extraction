#!/usr/bin/env python3
"""Aggregates map-simulation runs into heatmaps and a summary.

Usage:
    scripts/sim_report.py <runs-dir> <map-dump-dir> [--out DIR] [--px 1400]

<runs-dir> holds the `*.jsonl` telemetry `extraction --sim N` writes
(target/sim/<map>/); <map-dump-dir> is `map-dump` output (scripts/sim-report.sh
makes both). Writes to <runs-dir>/report/ unless --out says otherwise:

    traffic.png      where bots walked (time spent), with each run's path
    deaths.png       where bots died, and of what
    engagements.png  fights: bot and robot positions, joined, by outcome
    loot.png         where loot was picked up
    damage.png       where bots got hurt, weighted by damage
    summary.md       extraction rate, raid times, causes of death, most-looted
                     places, places nobody went

The telemetry format is documented in docs/SIMULATION.md.
"""

import argparse
import collections
import glob
import json
import math
import os
import statistics
import sys

import numpy as np
from PIL import Image, ImageDraw

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from map_preview import font, render  # noqa: E402

# One hue per layer (sequential: transparent -> full), validated reference
# steps for a dark surface; outcomes take the first three categorical slots.
HEAT = {
    "traffic": (237, 161, 0),  # yellow
    "loot": (25, 158, 112),  # aqua
    "damage": (217, 89, 38),  # orange
    "deaths": (230, 103, 103),  # red
    "engagements": (213, 81, 129),  # magenta
}
OUTCOME = {
    "extracted": (57, 135, 229),  # blue
    "died": (217, 89, 38),  # orange
    "timeout": (25, 158, 112),  # aqua
}
TEXT = (240, 239, 236)
MUTED = (195, 194, 183)


def load_runs(d):
    runs = []
    for path in sorted(glob.glob(os.path.join(d, "*.jsonl"))):
        evs = []
        with open(path) as f:
            for line in f:
                line = line.strip()
                if line:
                    try:
                        evs.append(json.loads(line))
                    except json.JSONDecodeError:
                        pass
        if not evs:
            continue
        start = next((e for e in evs if e["ev"] == "start"), {})
        end = next((e for e in evs if e["ev"] == "end"), None)
        runs.append(
            {
                "name": os.path.splitext(os.path.basename(path))[0],
                "start": start,
                "end": end,
                "events": evs,
                "outcome": end["outcome"] if end else "unfinished",
            }
        )
    return runs


def blur(a, sigma):
    """Separable Gaussian blur, numpy only."""
    if sigma <= 0:
        return a
    r = int(3 * sigma)
    k = np.exp(-0.5 * (np.arange(-r, r + 1) / sigma) ** 2)
    k /= k.sum()
    a = np.apply_along_axis(lambda m: np.convolve(m, k, mode="same"), 0, a)
    return np.apply_along_axis(lambda m: np.convolve(m, k, mode="same"), 1, a)


def density(points, meta, px_size, sigma_m, weights=None):
    """Points (x, z) in map metres -> blurred density at image resolution."""
    g = np.zeros((px_size, px_size), dtype=np.float64)
    s = px_size / meta["size"]
    o = meta["origin"]
    for i, (x, z) in enumerate(points):
        u, v = int((x - o) * s), int((z - o) * s)
        if 0 <= u < px_size and 0 <= v < px_size:
            g[v, u] += 1.0 if weights is None else weights[i]
    return blur(g, sigma_m * s)


def overlay(img, dens, color, gamma=0.6, max_alpha=0.85):
    """Blends a density over the image as one hue, transparent at zero."""
    if dens.max() <= 0:
        return img
    v = (dens / dens.max()) ** gamma
    a = np.clip(v * max_alpha, 0, max_alpha)[..., None]
    base = np.asarray(img, dtype=np.float64)
    col = np.array(color, dtype=np.float64)
    # Brighten toward white at the peaks so the ramp runs light-to-strong
    # on the dark map.
    top = col + (255 - col) * np.clip(v - 0.7, 0, 1)[..., None] / 0.3 * 0.5
    out = base * (1 - a) + top * a
    return Image.fromarray(np.clip(out, 0, 255).astype(np.uint8), "RGB")


def legend(img, title, rows, note=None):
    d = ImageDraw.Draw(img)
    f = font(max(13, img.size[0] // 95))
    fb = font(max(16, img.size[0] // 70))
    lines = len(rows) + (2 if note else 1)
    h = 12 + lines * (f.size + 8) + fb.size
    x0, y0 = 12, 46
    w = max(260, max(len(r[1]) for r in rows) * f.size * 0.6 + 50) if rows else 260
    d.rectangle([x0, y0, x0 + w, y0 + h], fill=(20, 20, 19))
    d.text((x0 + 10, y0 + 6), title, fill=TEXT, font=fb)
    y = y0 + 12 + fb.size
    for col, label in rows:
        d.rectangle([x0 + 10, y + 3, x0 + 24, y + 3 + f.size - 4], fill=col)
        d.text((x0 + 32, y), label, fill=TEXT, font=f)
        y += f.size + 8
    if note:
        d.text((x0 + 10, y), note, fill=MUTED, font=f)


def evs(runs, kind):
    for r in runs:
        for e in r["events"]:
            if e["ev"] == kind:
                yield r, e


def paths(runs):
    out = []
    for r in runs:
        pts = [(e["x"], e["z"]) for e in r["events"] if e["ev"] == "pos"]
        out.append((r, pts))
    return out


def draw_paths(img, runs, px, width=2, alpha=150):
    over = Image.new("RGBA", img.size, (0, 0, 0, 0))
    d = ImageDraw.Draw(over)
    for r, pts in paths(runs):
        if len(pts) < 2:
            continue
        col = OUTCOME.get(r["outcome"], MUTED)
        # Break at teleports (the truck ride home).
        seg = [px(pts[0])]
        for a, b in zip(pts, pts[1:]):
            if math.dist(a, b) > 30:
                if len(seg) > 1:
                    d.line(seg, fill=col + (alpha,), width=width)
                seg = []
            seg.append(px(b))
        if len(seg) > 1:
            d.line(seg, fill=col + (alpha,), width=width)
        # Where it ended: a cross for a death, a dot for anything else.
        x, y = px(pts[-1])
        if r["outcome"] == "died":
            d.line([x - 6, y - 6, x + 6, y + 6], fill=col + (255,), width=3)
            d.line([x - 6, y + 6, x + 6, y - 6], fill=col + (255,), width=3)
        else:
            d.ellipse([x - 6, y - 6, x + 6, y + 6], fill=col + (255,), outline=(20, 20, 19, 255), width=2)
    return Image.alpha_composite(img.convert("RGBA"), over).convert("RGB")


def outcome_rows(runs):
    c = collections.Counter(r["outcome"] for r in runs)
    return [(OUTCOME.get(k, MUTED), f"{k}: {v}") for k, v in sorted(c.items())]


def label(img, xy, text, color=TEXT):
    d = ImageDraw.Draw(img)
    f = font(max(12, img.size[0] // 110))
    x, y = xy
    d.text((x + 7, y - 7), text, fill=color, font=f, stroke_width=2, stroke_fill=(15, 15, 15))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("runs")
    ap.add_argument("dump")
    ap.add_argument("--out")
    ap.add_argument("--px", type=int, default=1400)
    args = ap.parse_args()
    out = args.out or os.path.join(args.runs, "report")
    os.makedirs(out, exist_ok=True)
    runs = load_runs(args.runs)
    if not runs:
        sys.exit(f"no *.jsonl runs in {args.runs}")
    mapname = runs[0]["start"].get("map", "map")
    n = len(runs)

    def base(title):
        img, meta, px = render(args.dump, args.px, title=f"{mapname}: {title} ({n} runs)", dim=0.35)
        return img, meta, px

    # Traffic: seconds spent per place, plus paths.
    img, meta, px = base("traffic")
    pos = [(e["x"], e["z"]) for _, e in evs(runs, "pos")]
    img = overlay(img, density(pos, meta, args.px, 6.0), HEAT["traffic"], gamma=0.45)
    img = draw_paths(img, runs, px, width=1, alpha=110)
    legend(img, "Traffic", [(HEAT["traffic"], "time spent (1 sample/s)")] + outcome_rows(runs),
           "lines: each run's path; x: died there, dot: ended there")
    img.save(os.path.join(out, "traffic.png"))

    # Deaths.
    img, meta, px = base("deaths")
    deaths = [e for _, e in evs(runs, "death")]
    img = overlay(img, density([(e["x"], e["z"]) for e in deaths], meta, args.px, 15.0),
                  HEAT["deaths"], gamma=0.8, max_alpha=0.6)
    d = ImageDraw.Draw(img)
    for e in deaths:
        x, y = px((e["x"], e["z"]))
        d.line([x - 7, y - 7, x + 7, y + 7], fill=HEAT["deaths"], width=3)
        d.line([x - 7, y + 7, x + 7, y - 7], fill=HEAT["deaths"], width=3)
        label(img, (x, y), e.get("cause", "?"))
    causes = collections.Counter(e.get("cause", "?") for e in deaths)
    legend(img, "Deaths", [(HEAT["deaths"], f"{c}: {k}") for c, k in causes.most_common()]
           or [(MUTED, "none")])
    img.save(os.path.join(out, "deaths.png"))

    # Engagements: bot position -> robot position, coloured by outcome.
    img, meta, px = base("engagements")
    eng = []
    for r in runs:
        open_ = None
        for e in r["events"]:
            if e["ev"] == "engage":
                open_ = e
            elif e["ev"] == "engage_end" and open_:
                eng.append((open_, e["outcome"]))
                open_ = None
        if open_:
            eng.append((open_, "unfinished"))
    img = overlay(img, density([(e["x"], e["z"]) for e, _ in eng], meta, args.px, 12.0),
                  HEAT["engagements"], gamma=0.7, max_alpha=0.55)
    d = ImageDraw.Draw(img)
    ecol = {"wrecked": OUTCOME["extracted"], "died": OUTCOME["died"]}
    for e, oc in eng:
        a, b = px((e["x"], e["z"])), px((e["rx"], e["rz"]))
        col = ecol.get(oc, MUTED)
        d.line([a, b], fill=col, width=2)
        d.ellipse([a[0] - 4, a[1] - 4, a[0] + 4, a[1] + 4], fill=col)
        g = "■" if e["robot"] == "Walker" else "▲"
        d.text((b[0] - 6, b[1] - 9), g, fill=col, font=font(16))
    oc = collections.Counter(o for _, o in eng)
    legend(img, "Engagements",
           [(ecol.get(k, MUTED), f"{k}: {v}") for k, v in oc.most_common()] or [(MUTED, "none")],
           "dot: bot, ▲ drone / ■ walker: robot")
    img.save(os.path.join(out, "engagements.png"))

    # Loot pickups (where the item lay).
    img, meta, px = base("loot pickups")
    loot = [e for _, e in evs(runs, "loot")]
    img = overlay(img, density([(e.get("ix", e["x"]), e.get("iz", e["z"])) for e in loot], meta, args.px, 8.0),
                  HEAT["loot"], gamma=0.6)
    by_site = collections.Counter(e["site"] for e in loot)
    legend(img, "Loot picked up", [(HEAT["loot"], f"{len(loot)} items")] +
           [(MUTED, f"{k}: {v}") for k, v in by_site.most_common(8)])
    img.save(os.path.join(out, "loot.png"))

    # Damage taken, weighted.
    img, meta, px = base("damage taken")
    dmg = [e for _, e in evs(runs, "damage")]
    img = overlay(img, density([(e["x"], e["z"]) for e in dmg], meta, args.px, 8.0,
                               weights=[e["amount"] for e in dmg]), HEAT["damage"], gamma=0.6)
    by_cause = collections.Counter()
    for e in dmg:
        by_cause[e["cause"]] += e["amount"]
    legend(img, "Damage taken", [(HEAT["damage"], f"{k}: {v:.0f} hp") for k, v in by_cause.most_common()]
           or [(MUTED, "none")])
    img.save(os.path.join(out, "damage.png"))

    write_summary(out, runs, meta, mapname)
    print(out)


def write_summary(out, runs, meta, mapname):
    n = len(runs)
    ends = [r["end"] for r in runs if r["end"]]
    oc = collections.Counter(r["outcome"] for r in runs)
    ext = [e for e in ends if e["outcome"] == "extracted"]
    died = [e for e in ends if e["outcome"] == "died"]

    def mean(xs):
        return statistics.mean(xs) if xs else float("nan")

    def mins(s):
        return f"{s / 60:.1f} min" if s == s else "-"

    L = [f"# Simulation report: {mapname}", ""]
    L += [f"{n} runs. Personalities: " + ", ".join(
        f"{k} {v}" for k, v in collections.Counter(r["start"].get("personality", "?") for r in runs).items())]
    L += ["", "## Outcomes", ""]
    L += ["| Outcome | Runs | Share |", "| - | -: | -: |"]
    for k, v in oc.most_common():
        L.append(f"| {k} | {v} | {100 * v / n:.0f}% |")
    L += ["", f"- Extraction rate: **{100 * len(ext) / n:.0f}%**"]
    L += [f"- Mean raid time, extracted: {mins(mean([e['raid_time'] for e in ext]))}; "
          f"died: {mins(mean([e['raid_time'] for e in died]))}"]
    L += [f"- Mean distance walked: {mean([e['distance'] for e in ends]):.0f} m"]
    L += [f"- Mean items carried out (extracted runs): {mean([e['loot_count'] for e in ext]):.1f}"]
    shots = sum(e["shots"] for e in ends)
    hits = sum(e["hits"] for e in ends)
    L += [f"- Shots fired: {shots}, hits: {hits}" + (f" ({100 * hits / shots:.0f}%)" if shots else "")]
    kills = collections.Counter()
    for e in ends:
        kills.update(e.get("kills", {}))
    L += [f"- Robots wrecked: " + (", ".join(f"{k} {v}" for k, v in kills.items()) or "none")]
    reasons = collections.Counter(e.get("extract_reason") or "-" for e in ends)
    L += [f"- Why bots headed out: " + ", ".join(f"{k} {v}" for k, v in reasons.most_common())]

    L += ["", "## By personality", "", "| Personality | Runs | Extracted | Died | Items (mean) | Raid time (mean) |",
          "| - | -: | -: | -: | -: | -: |"]
    byp = collections.defaultdict(list)
    for r in runs:
        byp[r["start"].get("personality", "?")].append(r)
    for p, rs in sorted(byp.items()):
        e = [r["end"] for r in rs if r["end"]]
        L.append(f"| {p} | {len(rs)} | {sum(x['outcome'] == 'extracted' for x in e)} | "
                 f"{sum(x['outcome'] == 'died' for x in e)} | {mean([x['loot_count'] for x in e]):.1f} | "
                 f"{mins(mean([x['raid_time'] for x in e]))} |")

    L += ["", "## Deaths by cause", ""]
    causes = collections.Counter(e.get("death_cause") or "?" for e in died)
    L += [f"- {k}: {v}" for k, v in causes.most_common()] or ["- none"]
    dmg = collections.Counter()
    for e in ends:
        for k, v in e.get("damage", {}).items():
            dmg[k] += v
    L += ["", "Damage taken, all runs: " + (", ".join(f"{k} {v:.0f} hp" for k, v in dmg.most_common()) or "none")]

    L += ["", "## Engagements", ""]
    eng = collections.Counter()
    for r in runs:
        for e in r["events"]:
            if e["ev"] == "engage_end":
                eng[(e["robot"], e["outcome"])] += 1
    L += [f"- {k[0]} -> {k[1]}: {v}" for k, v in eng.most_common()] or ["- none"]

    L += ["", "## Most-looted places", ""]
    sites = collections.Counter()
    items = collections.Counter()
    for r in runs:
        for e in r["events"]:
            if e["ev"] == "loot":
                sites[e["site"]] += 1
                items[e["item"]] += 1
    L += [f"- {k}: {v} items" for k, v in sites.most_common(10)] or ["- none"]
    L += ["", "Most-taken items: " + (", ".join(f"{k} {v}" for k, v in items.most_common(12)) or "none")]

    # Places nobody went: markers and buildings no position sample came
    # within 15 m of.
    pts = np.array([(e["x"], e["z"]) for r in runs for e in r["events"] if e["ev"] == "pos"] or [(1e9, 1e9)])

    def visited(p, r=15.0):
        return bool((np.hypot(pts[:, 0] - p[0], pts[:, 1] - p[1]) < r).any())

    L += ["", "## Never visited", "", "Within 15 m, by any run:", ""]
    unvisited = []
    for m in meta["markers"]:
        if m["kind"] in ("Poi", "Loot", "ArtifactField", "Rendezvous", "Offering", "WorkSite") and not visited(m["at"]):
            unvisited.append(f"{m['kind']} {m['name'] or ''} at ({m['at'][0]:.0f}, {m['at'][1]:.0f})")
    for p in meta["placements"]:
        if not visited(p["at"], 20.0):
            unvisited.append(f"building {p['prefab']} at ({p['at'][0]:.0f}, {p['at'][1]:.0f})")
    L += [f"- {u}" for u in unvisited] or ["- everything was visited"]
    # Coverage: share of the map's 25 m squares any bot passed through.
    size, o = meta["size"], meta["origin"]
    cells = int(size // 25)
    seen = set()
    for x, z in pts:
        seen.add((int((x - o) // 25), int((z - o) // 25)))
    L += ["", f"Coverage: bots passed through {100 * len(seen) / (cells * cells):.0f}% of the map's 25 m squares."]

    L += ["", "## Runs", "", "| Run | Personality | Seed | Outcome | Raid time | Items | Shots/hits | Distance |",
          "| - | - | -: | - | -: | -: | -: | -: |"]
    for r in runs:
        e = r["end"] or {}
        L.append(f"| {r['name']} | {r['start'].get('personality', '?')} | {r['start'].get('seed', '?')} | "
                 f"{r['outcome']}{' (' + str(e.get('death_cause')) + ')' if r['outcome'] == 'died' else ''} | "
                 f"{mins(e.get('raid_time', float('nan')))} | {e.get('loot_count', '-')} | "
                 f"{e.get('shots', '-')}/{e.get('hits', '-')} | {e.get('distance', '-')} m |")
    L += ["", "Heatmaps: traffic.png, deaths.png, engagements.png, loot.png, damage.png."]
    with open(os.path.join(out, "summary.md"), "w") as f:
        f.write("\n".join(L) + "\n")


if __name__ == "__main__":
    main()
