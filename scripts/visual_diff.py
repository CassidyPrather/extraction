#!/usr/bin/env python3
"""Compare two shot sets from scripts/visual_diff.sh.

    scripts/visual_diff.py <old shots> <new shots> <out dir> [--title T] [--noise FILE]
    scripts/visual_diff.py --calibrate <shots> <same build's shots again> <noise.json>

Each <map>/<mood>/<view>.png in both is measured two ways: 1 - SSIM on
luminance (structure, at half size), and the share of pixels whose colour
moved more than a little after a light blur (a light going dark, a
material turning magenta, a prop gone). A shot changed if either passes
its threshold. Its score is how many times over the larger one is, and
the sheet (old | new | where it changed) puts the highest first (the top
SHEET_MAX). report.json and index.html list every pair.

The thresholds are low, so a small lamp going dark counts (a 12 px patch
at 960 px is 0.08% of the frame). A shot that differs from itself (falling
snow) is allowed twice its own noise, from noise.json: a calibration, the
same build shot twice (--calibrate).
"""

import argparse
import html
import json
import os
import sys

import numpy as np
from PIL import Image, ImageDraw, ImageFilter

# The same build shot twice: 96 of 100 shots came out identical; the rest
# (Rime Fell by day, its falling snow) differ by up to 0.027 structure and
# 0.15% of pixels, and get their own allowance from noise.json.
STRUCTURE = float(os.environ.get("STRUCTURE", "0.005"))
PIXELS = float(os.environ.get("PIXELS", "0.0005"))
SHEET_MAX = 40
# How far a pixel's colour may move (0-1, per channel, after a 2 px blur)
# before it counts as changed.
PIXEL = 0.08
TILE = (480, 270)


def load(path):
    return Image.open(path).convert("RGB")


def box(a, r):
    """Mean over a (2r+1)^2 box, edges clamped."""
    p = np.pad(a, r + 1, mode="edge")
    c = p.cumsum(0).cumsum(1)
    k = 2 * r + 1
    s = c[k:, k:] - c[:-k, k:] - c[k:, :-k] + c[:-k, :-k]
    return s[: a.shape[0], : a.shape[1]] / (k * k)


def ssim(a, b, r=3):
    c1, c2 = 0.01**2, 0.03**2
    ma, mb = box(a, r), box(b, r)
    va = box(a * a, r) - ma * ma
    vb = box(b * b, r) - mb * mb
    cov = box(a * b, r) - ma * mb
    s = ((2 * ma * mb + c1) * (2 * cov + c2)) / ((ma * ma + mb * mb + c1) * (va + vb + c2))
    return float(s.mean())


def score(old, new, allow=(STRUCTURE, PIXELS)):
    a = np.asarray(old.resize(TILE).convert("L"), dtype=np.float32) / 255
    b = np.asarray(new.resize(TILE).convert("L"), dtype=np.float32) / 255
    structure = 1.0 - ssim(a, b)
    ba = np.asarray(old.resize(TILE).filter(ImageFilter.GaussianBlur(2)), dtype=np.float32) / 255
    bb = np.asarray(new.resize(TILE).filter(ImageFilter.GaussianBlur(2)), dtype=np.float32) / 255
    moved = np.abs(ba - bb).max(axis=2)
    share = float((moved > PIXEL).mean())
    return max(structure / allow[0], share / allow[1]), structure, share, moved


def heat(new, moved):
    """The new shot greyed, with where it changed in red."""
    base = np.asarray(new.resize(TILE).convert("L").convert("RGB"), dtype=np.float32) * 0.5
    m = np.clip(moved / 0.3, 0, 1)[..., None]
    out = base * (1 - m) + np.array([255, 40, 40], dtype=np.float32) * m
    return Image.fromarray(out.astype(np.uint8))


def main(argv):
    ap = argparse.ArgumentParser()
    ap.add_argument("old")
    ap.add_argument("new")
    ap.add_argument("out")
    ap.add_argument("--title", default="")
    ap.add_argument("--noise", default=os.path.join(os.path.dirname(__file__), "visual_diff", "noise.json"))
    ap.add_argument("--calibrate", action="store_true")
    ap.add_argument("--maps", default="", help="only these maps (space-separated)")
    ap.add_argument("--moods", default="", help="only these moods")
    ap.add_argument("--views", default="", help="only these views")
    a = ap.parse_args(argv)

    def shots(root):
        found = {}
        for dirpath, _, files in os.walk(root):
            for f in files:
                if f.endswith(".png"):
                    rel = os.path.relpath(os.path.join(dirpath, f), root)
                    found[rel[:-4]] = os.path.join(dirpath, f)
        return found

    def keep(key):
        parts = key.split("/")
        # The building probes (`<view>__without_...`) aren't views.
        if len(parts) != 3 or "__" in parts[2]:
            return False
        for want, part in zip((a.maps, a.moods, a.views), parts):
            if want and part not in want.split():
                return False
        return True

    old = {k: v for k, v in shots(a.old).items() if keep(k)}
    new = {k: v for k, v in shots(a.new).items() if keep(k)}
    if a.calibrate:
        # a.out is the noise file: each shot's own difference, where any.
        noise = {}
        for key in sorted(set(old) & set(new)):
            _, structure, share, _ = score(load(old[key]), load(new[key]))
            if structure > 0 or share > 0:
                noise[key] = {"structure": round(structure, 5), "pixels": round(share, 5)}
        with open(a.out, "w") as f:
            json.dump(noise, f, indent=1, sort_keys=True)
        print(f"{len(noise)} of {len(set(old) & set(new))} shots differ from themselves: {a.out}")
        return
    os.makedirs(a.out, exist_ok=True)
    noise = {}
    if os.path.exists(a.noise):
        with open(a.noise) as f:
            noise = json.load(f)
    rows, missing = [], []
    for key in sorted(set(old) | set(new)):
        if key not in old or key not in new:
            missing.append((key, "only in new" if key in new else "only in old"))
            continue
        n = noise.get(key, {})
        allow = (max(STRUCTURE, 2 * n.get("structure", 0)), max(PIXELS, 2 * n.get("pixels", 0)))
        s, structure, share, moved = score(load(old[key]), load(new[key]), allow)
        rows.append({"shot": key, "score": s, "structure": structure, "changed": share, "_moved": moved})
    rows.sort(key=lambda r: -r["score"])
    flagged = [r for r in rows if r["score"] > 1.0]
    shown = flagged[:SHEET_MAX]

    # The sheet: old | new | where, most changed first.
    label_h = 22
    w, h = TILE
    sheet = Image.new("RGB", (w * 3, max(1, len(shown)) * (h + label_h)), (24, 24, 24))
    d = ImageDraw.Draw(sheet)
    for i, r in enumerate(shown):
        y = i * (h + label_h)
        o, n = load(old[r["shot"]]), load(new[r["shot"]])
        d.text((6, y + 5), f"{i + 1}. {r['shot']}   {r['score']:.1f}x the threshold "
               f"(structure {r['structure']:.3f}, {r['changed']:.1%} of pixels moved)",
               fill=(255, 255, 255))
        sheet.paste(o.resize(TILE), (0, y + label_h))
        sheet.paste(n.resize(TILE), (w, y + label_h))
        sheet.paste(heat(n, r["_moved"]), (2 * w, y + label_h))
    if not flagged:
        d.text((10, 10), "No shot changed past the threshold.", fill=(255, 255, 255))
    sheet.save(os.path.join(a.out, "sheet.jpg"), quality=85)

    report = {
        "title": a.title,
        "thresholds": {"structure": STRUCTURE, "pixels": PIXELS},
        "compared": len(rows),
        "changed": len(flagged),
        "missing": missing,
        "shots": [{k: v for k, v in r.items() if not k.startswith("_")} for r in rows],
    }
    with open(os.path.join(a.out, "report.json"), "w") as f:
        json.dump(report, f, indent=1)
    with open(os.path.join(a.out, "index.html"), "w") as f:
        f.write("<!doctype html><meta charset=utf-8><title>Visual diff</title>"
                "<style>body{font:14px system-ui;background:#f6f6f4;color:#1d1d1b;margin:16px}"
                "@media (prefers-color-scheme:dark){body{background:#161615;color:#eee}}"
                "td{padding:2px 8px}img{max-width:100%}</style>")
        f.write(f"<h1>Visual diff {html.escape(a.title)}</h1>"
                f"<p>{len(flagged)} of {len(rows)} shots changed (structure over {STRUCTURE} "
                f"or over {PIXELS:.1%} of pixels moved).</p>"
                "<img src=sheet.jpg><table><tr><th>shot<th>times over<th>structure<th>pixels moved")
        for r in rows:
            f.write(f"<tr><td>{html.escape(r['shot'])}<td>{r['score']:.2f}"
                    f"<td>{r['structure']:.3f}<td>{r['changed']:.1%}")
        f.write("</table>")
        for key, why in missing:
            f.write(f"<p>{html.escape(key)}: {why}</p>")
    # One line for a gate: PASS, or CHANGED and where to look.
    sheet = os.path.join(a.out, "sheet.jpg")
    if flagged or missing:
        print(f"CHANGED {a.title}: {len(flagged)} of {len(rows)} shots changed, "
              f"{len(missing)} missing; sheet {sheet}")
    else:
        print(f"PASS {a.title}: none of {len(rows)} shots changed; sheet {sheet}")


if __name__ == "__main__":
    main(sys.argv[1:])
