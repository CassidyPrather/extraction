#!/usr/bin/env python3
"""Reads a looks-wrong sweep (`--sweep`, see src/capture/sweep.rs) and flags
what looks wrong.

    scripts/capture_sweep.py SWEEP_DIR

Per subject, from <light>/<subject>.png and the day's picture with the
subject hidden (`_empty`):

- fallback colours: magenta in the subject (an engine's missing texture);
- blank views: a view where the subject covers almost nothing;
- size: a model's size against its folder's range, a prefab's against the
  radius the map clears round it;
- blockouts left standing (a model that never loaded) and pieces whose
  boxes touch nothing (floating), from sweep.json.

Writes SWEEP_DIR/report.md (findings, worst first), SWEEP_DIR/index.html
(a contact sheet of every subject under every light, flagged ones
outlined, each linking to its pictures). Z-fighting isn't looked for here:
a still picture doesn't show it; `cargo test --lib capture::fighting`
finds it from the geometry.
"""
import html
import json
import os
import sys

import numpy as np
from PIL import Image

# What a model in each folder may measure (largest side, metres); the same
# table as `capture::checks::size_range`.
SIZE = {
    "guns": (0.15, 1.8), "mags": (0.04, 0.5), "items": (0.01, 1.6),
    "kit": (0.15, 1.0), "hands": (0.08, 0.4), "props": (0.03, 70.0),
    "robots": (0.3, 40.0), "vehicles": (0.3, 12.0),
}
# The subject is where the picture differs from the empty studio past this.
SUBJECT = 10


def load(path):
    if not os.path.exists(path):
        return None
    return np.asarray(Image.open(path).convert("RGB")).astype(np.int16)


def cells(img, grid):
    cols, rows = grid
    h, w = img.shape[0] // rows, img.shape[1] // cols
    return [img[r * h:(r + 1) * h, c * w:(c + 1) * w] for r in range(rows) for c in range(cols)]


def clumped(mask):
    """Changed pixels with at least two changed neighbours (of eight)."""
    m = mask.astype(np.int8)
    p = np.pad(m, 1)
    n = sum(
        p[1 + dy:p.shape[0] - 1 + dy, 1 + dx:p.shape[1] - 1 + dx]
        for dy in (-1, 0, 1) for dx in (-1, 0, 1) if dy or dx
    )
    return mask & (n >= 2)


def analyse(out, rec, grid):
    f = rec["file"]
    flags = []
    day = load(os.path.join(out, "day", f + ".png"))
    empty = load(os.path.join(out, "day", f + "_empty.png"))
    stats = {}
    if rec.get("failed"):
        flags.append(("error", "didn't load"))
    if day is None:
        flags.append(("error", "no day picture"))
        return flags, stats
    subject = None
    if empty is not None and empty.shape == day.shape:
        # Clumped, so a drifting cloud's edge doesn't count as subject.
        subject = clumped(np.abs(day - empty).max(axis=2) > SUBJECT)
        cover = [float(c.mean()) for c in cells(subject, grid)]
        stats["cover"] = cover
        if max(cover) < 0.002:
            flags.append(("error", "nothing visible in any view"))
        else:
            blank = [i for i, c in enumerate(cover) if c < 0.002]
            if blank:
                flags.append(("warn", f"almost nothing visible in views {blank}"))
        r, g, b = day[..., 0], day[..., 1], day[..., 2]
        magenta = subject & (r > 200) & (b > 200) & (g < 70)
        frac = magenta.sum() / max(subject.sum(), 1)
        stats["magenta"] = float(frac)
        if frac > 0.01:
            flags.append(("error", f"magenta (missing texture) on {frac:.0%} of it"))
    lo, hi = np.array(rec["lo"]), np.array(rec["hi"])
    size = hi - lo
    if rec["kind"] == "model":
        folder = rec["subject"].split("/")[0]
        a, b = SIZE.get(folder, (0.01, 60.0))
        if not a <= size.max() <= b:
            flags.append(("error", f"{size.max():.2f} m across, outside {a}-{b} m for {folder}"))
    elif rec["kind"] == "prefab":
        # How far it reaches from its footing against the circle the map
        # clears round it: past that it pokes into trees, rocks and its
        # neighbours. (Sites underground have nothing cleared.)
        clear = rec.get("clearance")
        if clear is not None:
            reach = float(np.hypot(max(abs(lo[0]), abs(hi[0])), max(abs(lo[2]), abs(hi[2]))))
            if reach > clear + 1.0:
                flags.append(("warn", f"reaches {reach:.1f} m from its footing, the map clears {clear:.1f} m"))
    if rec.get("blockouts"):
        flags.append(("error", f"{rec['blockouts']} blockout boxes still standing"))
    # animals/ hold pose sheets (each pose its own node, laid out side by
    # side, one shown at a time): their flying poses aren't floating.
    # Robots and the open rig are kits of parts the game places itself.
    # (`capture::checks::assembled_in_game`.)
    kit = rec["kind"] == "model" and (
        rec["subject"].split("/")[0] in ("animals", "robots") or rec["subject"] == "kit/rig_open"
    )
    if rec.get("floating") and not kit:
        names = sorted(set(rec["floating"]))
        flags.append(("warn", f"floating: {', '.join(names[:6])}" + (" ..." if len(names) > 6 else "")))
    return flags, stats


def main(argv):
    out = argv[0]
    with open(os.path.join(out, "sweep.json")) as f:
        sweep = json.load(f)
    grid = tuple(sweep["grid"])
    lights = sweep["lights"]
    results = []
    for rec in sweep["subjects"]:
        flags, stats = analyse(out, rec, grid)
        results.append((rec, flags, stats))
    rank = {"error": 0, "warn": 1}
    flagged = sorted(
        (r for r in results if r[1]),
        key=lambda r: (min(rank[s] for s, _ in r[1]), r[0]["subject"]),
    )
    with open(os.path.join(out, "report.md"), "w") as f:
        f.write(f"# Looks-wrong sweep\n\n{len(results)} subjects under {', '.join(lights)}; "
                f"{len(flagged)} flagged.\n\n")
        f.write("| Subject | Severity | Finding |\n| - | - | - |\n")
        for rec, flags, _ in flagged:
            for sev, msg in flags:
                f.write(f"| `{rec['subject']}` | {sev} | {msg} |\n")
    # The contact sheet.
    css = ("body{background:#111;color:#ddd;font:13px sans-serif;margin:16px}"
           "h1{font-size:18px}.grid{display:flex;flex-wrap:wrap;gap:8px}"
           ".s{width:330px;background:#1b1b1b;padding:4px;border:2px solid #1b1b1b}"
           ".s.error{border-color:#d33}.s.warn{border-color:#d93}"
           ".s img{width:320px;display:block}.s .n{font-weight:bold;margin:2px 0}"
           ".s ul{margin:2px 0 0 16px;padding:0;color:#f99}"
           "nav a{color:#9cf;margin-right:12px}")
    with open(os.path.join(out, "index.html"), "w") as f:
        f.write(f"<!doctype html><meta charset=utf-8><title>Sweep</title><style>{css}</style>")
        f.write(f"<h1>Looks-wrong sweep: {len(results)} subjects, {len(flagged)} flagged</h1><nav>")
        for l in lights:
            f.write(f"<a href='#{l}'>{l}</a>")
        f.write("<a href='report.md'>report.md</a></nav>")
        order = flagged + [r for r in results if not r[1]]
        for l in lights:
            f.write(f"<h2 id='{l}'>{l}</h2><div class=grid>")
            for rec, flags, _ in order:
                sev = min((rank[s] for s, _ in flags), default=2)
                cls = ["error", "warn", ""][sev]
                img = f"{l}/{rec['file']}.png"
                f.write(f"<div class='s {cls}'><div class=n>{html.escape(rec['subject'])}</div>"
                        f"<a href='{img}'><img loading=lazy src='{img}'></a>")
                if flags and l == "day":
                    f.write("<ul>" + "".join(f"<li>{html.escape(m)}</li>" for _, m in flags) + "</ul>")
                f.write("</div>")
            f.write("</div>")
    print(f"{len(results)} subjects, {len(flagged)} flagged: {out}/report.md, {out}/index.html")
    for rec, flags, _ in flagged:
        for sev, msg in flags:
            print(f"  {sev:5} {rec['subject']}: {msg}")


if __name__ == "__main__":
    main(sys.argv[1:])
