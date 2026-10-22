#!/usr/bin/env python3
"""Compare two shot sets from scripts/visual_diff.sh.

    scripts/visual_diff.py <old shots> <new shots> <out dir> [--title T] [--threshold X]

Each <map>/<mood>/<view>.png in both is scored by how much it changed:
the larger of 1 - SSIM on luminance (structure, at half size) and the
share of pixels whose colour moved more than a little after a light blur
(a light going dark, a material turning magenta, a prop gone). Anything
scoring over the threshold goes on sheet.jpg (old | new | where it
changed) with the most changed first. report.json and index.html list
every pair.
"""

import argparse
import html
import json
import os
import sys

import numpy as np
from PIL import Image, ImageDraw, ImageFilter

# Two shots of one build differ by about 0.002 (see docs/CAPTURE.md);
# anything a person would call a change scores well over this.
THRESHOLD = float(os.environ.get("THRESHOLD", "0.02"))
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


def score(old, new):
    a = np.asarray(old.resize(TILE).convert("L"), dtype=np.float32) / 255
    b = np.asarray(new.resize(TILE).convert("L"), dtype=np.float32) / 255
    structure = 1.0 - ssim(a, b)
    ba = np.asarray(old.resize(TILE).filter(ImageFilter.GaussianBlur(2)), dtype=np.float32) / 255
    bb = np.asarray(new.resize(TILE).filter(ImageFilter.GaussianBlur(2)), dtype=np.float32) / 255
    moved = np.abs(ba - bb).max(axis=2)
    changed = moved > PIXEL
    return max(structure, float(changed.mean())), structure, float(changed.mean()), moved


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
    ap.add_argument("--threshold", type=float, default=THRESHOLD)
    a = ap.parse_args(argv)
    os.makedirs(a.out, exist_ok=True)

    def shots(root):
        found = {}
        for dirpath, _, files in os.walk(root):
            for f in files:
                if f.endswith(".png"):
                    rel = os.path.relpath(os.path.join(dirpath, f), root)
                    found[rel[:-4]] = os.path.join(dirpath, f)
        return found

    old, new = shots(a.old), shots(a.new)
    rows, missing = [], []
    for key in sorted(set(old) | set(new)):
        if key not in old or key not in new:
            missing.append((key, "only in new" if key in new else "only in old"))
            continue
        s, structure, share, moved = score(load(old[key]), load(new[key]))
        rows.append({"shot": key, "score": s, "structure": structure, "changed": share, "_moved": moved})
    rows.sort(key=lambda r: -r["score"])
    flagged = [r for r in rows if r["score"] > a.threshold]

    # The sheet: old | new | where, most changed first.
    label_h = 22
    w, h = TILE
    sheet = Image.new("RGB", (w * 3, max(1, len(flagged)) * (h + label_h)), (24, 24, 24))
    d = ImageDraw.Draw(sheet)
    for i, r in enumerate(flagged):
        y = i * (h + label_h)
        o, n = load(old[r["shot"]]), load(new[r["shot"]])
        d.text((6, y + 5), f"{i + 1}. {r['shot']}   score {r['score']:.3f} "
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
        "threshold": a.threshold,
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
                f"<p>{len(flagged)} of {len(rows)} shots changed past {a.threshold}.</p>"
                "<img src=sheet.jpg><table><tr><th>shot<th>score<th>structure<th>pixels moved")
        for r in rows:
            f.write(f"<tr><td>{html.escape(r['shot'])}<td>{r['score']:.3f}"
                    f"<td>{r['structure']:.3f}<td>{r['changed']:.1%}")
        f.write("</table>")
        for key, why in missing:
            f.write(f"<p>{html.escape(key)}: {why}</p>")
    print(f"{len(flagged)} of {len(rows)} shots changed past {a.threshold}; "
          f"{len(missing)} missing: {a.out}/sheet.jpg")


if __name__ == "__main__":
    main(sys.argv[1:])
