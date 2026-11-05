#!/usr/bin/env python3
"""Reads a `--torchlit` capture (`src/capture/torchlit.rs`) and says how
each item stands off its floor by torchlight.

Each item's pixels are where its shot differs from its empty floor's (the
torch is at the eye, so its shadow hides behind it). From them:

- body: how far its colour is from the floor's it covers (the same pixels
  in the empty shot), as CIE76 delta E between their mean Lab colours:
  under about 10 it's the floor's colour, 20 or more stands off it;
- rim: the same across its outline only (a light rim on a dark thing
  reads even when its body matches the floor);
- contrast: |item - floor| / the brighter, in linear luminance (lightness
  alone, for the record);
- size: the side of a square of its pixels, in degrees at the eye;
- colour: its mean hue and chroma (CIE Lab a*, b*), its lightness L*.

A frame whose floor, outside the item, differs from its empty floor's is
flagged as a bad shot (rendered wrong) and left out.

Twins are items that are near one another in size, outline (the aspect of
its pixels' spread) and colour, seen from 2 m on wood, and aren't the
same kind of thing by name.

    scripts/torchlit.py CAPTURE_DIR [--json out.json] [--sheet out.jpg N]
        [--compare OTHER_CAPTURE_DIR] [--ids a,b,c]

`--sheet` makes a contact sheet of the worst N (both eyes, each floor);
`--compare` puts another capture's shots of the same items beside each
(before / after).
"""

import argparse
import json
import math
import os
import sys

import numpy as np
from PIL import Image, ImageDraw

# How far a pixel must differ from the empty floor to be the item's (0-255).
DIFF = 14
# The share of the floor outside the item that may differ before the shot
# counts as rendered wrong.
BAD_OUTSIDE = 0.02
# What reads: below these an item is flagged.
LOW_DE = 15.0
SMALL_DEG = 0.7


def lin(a):
    a = a / 255.0
    return np.where(a <= 0.04045, a / 12.92, ((a + 0.055) / 1.055) ** 2.4)


def lum(rgb):
    l = lin(rgb.astype(np.float32))
    return 0.2126 * l[..., 0] + 0.7152 * l[..., 1] + 0.0722 * l[..., 2]


def lab(rgb_mean):
    """CIE Lab of a mean sRGB colour (0-255)."""
    r, g, b = lin(np.array(rgb_mean, dtype=np.float32))
    x = (0.4124 * r + 0.3576 * g + 0.1805 * b) / 0.9505
    y = 0.2126 * r + 0.7152 * g + 0.0722 * b
    z = (0.0193 * r + 0.1192 * g + 0.9505 * b) / 1.089
    f = lambda t: t ** (1 / 3) if t > 0.008856 else 7.787 * t + 16 / 116
    fx, fy, fz = f(x), f(y), f(z)
    return 116 * fy - 16, 500 * (fx - fy), 200 * (fy - fz)


def lab_px(rgb):
    """Mean CIE Lab over pixels (N x 3, sRGB 0-255)."""
    l = lin(rgb.astype(np.float32))
    x = (0.4124 * l[:, 0] + 0.3576 * l[:, 1] + 0.1805 * l[:, 2]) / 0.9505
    y = 0.2126 * l[:, 0] + 0.7152 * l[:, 1] + 0.0722 * l[:, 2]
    z = (0.0193 * l[:, 0] + 0.1192 * l[:, 1] + 0.9505 * l[:, 2]) / 1.089
    f = lambda t: np.where(t > 0.008856, np.cbrt(t), 7.787 * t + 16 / 116)
    fx, fy, fz = f(x), f(y), f(z)
    return np.array([(116 * fy - 16).mean(), (500 * (fx - fy)).mean(), (200 * (fy - fz)).mean()])


def delta_e(a, b):
    return float(np.linalg.norm(a - b))


def erode(m):
    e = m.copy()
    e[1:, :] &= m[:-1, :]
    e[:-1, :] &= m[1:, :]
    e[:, 1:] &= m[:, :-1]
    e[:, :-1] &= m[:, 1:]
    return e


def dilate(m):
    d = m.copy()
    d[1:, :] |= m[:-1, :]
    d[:-1, :] |= m[1:, :]
    d[:, 1:] |= m[:, :-1]
    d[:, :-1] |= m[:, 1:]
    return d


def measure(shot_path, empty, px_per_deg):
    shot = np.asarray(Image.open(shot_path).convert("RGB")).astype(np.int16)
    diff = np.abs(shot - empty).max(axis=2)
    raw = diff > DIFF
    # Opened: lone noisy pixels out, the item's body kept.
    mask = dilate(erode(raw))
    n = int(mask.sum())
    outside = raw & ~dilate(dilate(dilate(mask)))
    bad = outside.mean() > BAD_OUTSIDE or n == 0
    if n == 0:
        return {"bad": True, "px": 0}
    li = lum(shot[mask]).mean()
    lf = lum(empty[mask]).mean()
    contrast = abs(li - lf) / max(li, lf, 1e-6)
    body = delta_e(lab_px(shot[mask]), lab_px(empty[mask]))
    rim = mask & ~erode(erode(mask))
    edge = delta_e(lab_px(shot[rim]), lab_px(empty[rim])) if rim.sum() > 0 else body
    ys, xs = np.nonzero(mask)
    cov = np.cov(np.vstack([xs, ys])) if n > 2 else np.eye(2)
    ev = np.sort(np.linalg.eigvalsh(cov))
    aspect = math.sqrt(max(ev[1], 1e-6) / max(ev[0], 1e-6))
    L, a, b = lab(shot[mask].mean(axis=0))
    return {
        "bad": bool(bad),
        "px": n,
        "deg": math.sqrt(n) / px_per_deg,
        "contrast": float(contrast),
        "body": float(body),
        "edge": float(edge),
        "item_lum": float(li),
        "floor_lum": float(lf),
        "aspect": float(aspect),
        "L": float(L),
        "a": float(a),
        "b": float(b),
    }


def analyse(root):
    meta = json.load(open(os.path.join(root, "torchlit.json")))
    ppd = meta["px_per_deg"]
    items = [i for i in meta["items"] if not i.get("failed")]
    out = {}
    for floor in meta["floors"]:
        for eye in [e["name"] for e in meta["eyes"]]:
            d = os.path.join(root, floor, eye)
            empty_p = os.path.join(d, "_empty.png")
            if not os.path.exists(empty_p):
                continue
            empty = np.asarray(Image.open(empty_p).convert("RGB")).astype(np.int16)
            for it in items:
                p = os.path.join(d, it["id"] + ".png")
                if not os.path.exists(p):
                    continue
                m = measure(p, empty, ppd)
                out.setdefault(it["id"], {"item": it, "views": {}})["views"][f"{floor}/{eye}"] = m
    return meta, out


def score(entry):
    """The worst of an item's good views: its lowest contrast and edge, its
    smallest size from 2 m."""
    views = [v for v in entry["views"].values() if not v["bad"]]
    if not views:
        return None
    worst_c = min(v["body"] for v in views)
    worst_e = min(v["edge"] for v in views)
    far = [v["deg"] for k, v in entry["views"].items() if k.endswith("/2m") and not v["bad"]]
    size = min(far) if far else min(v["deg"] for v in views)
    # A thing that matches the floor in body but stands out at its rim
    # still reads: the better of the two counts, the worse warns.
    reads = max(worst_c, worst_e)
    return {"contrast": worst_c, "edge": worst_e, "reads": reads, "deg_2m": size,
            "worst_view": min(((max(v["body"], v["edge"]), k) for k, v in entry["views"].items() if not v["bad"]))[1]}


def stem(i):
    """What kind of thing an item is, by its id: its first word (`ammo`,
    `round`, `tape`), so a box of rounds isn't a twin of another box of
    rounds."""
    return i.split("_")[0]


def twins(out, n=20):
    pts = []
    for k, e in out.items():
        v = e["views"].get("wood/2m")
        if not v or v["bad"]:
            continue
        pts.append((k, v))
    pairs = []
    for i in range(len(pts)):
        for j in range(i + 1, len(pts)):
            (ka, a), (kb, b) = pts[i], pts[j]
            if stem(ka) == stem(kb):
                continue
            size = abs(math.log(a["px"] / b["px"]))
            shape = abs(math.log(a["aspect"] / b["aspect"]))
            colour = math.dist((a["L"], a["a"], a["b"]), (b["L"], b["a"], b["b"])) / 25.0
            d = size + shape + colour
            pairs.append((d, ka, kb, size, shape, colour))
    pairs.sort()
    return pairs[:n]


def sheet(root, ids, out_path, compare=None, meta=None):
    floors = meta["floors"]
    eyes = [e["name"] for e in meta["eyes"]]
    cell = 150
    cols = [(f, e) for f in floors for e in eyes]
    roots = [root] + ([compare] if compare else [])
    w = 170 + cell * len(cols) * len(roots)
    h = 22 + cell * len(ids)
    img = Image.new("RGB", (w, h), (0, 0, 0))
    d = ImageDraw.Draw(img)
    for c, (f, e) in enumerate(cols):
        for r_i, r in enumerate(roots):
            x = 170 + (c * len(roots) + r_i) * cell
            label = f"{f} {e}" + (" before" if compare and r_i == 0 else " after" if compare else "")
            d.text((x + 3, 4), label, fill=(255, 255, 255))
    for r, i in enumerate(ids):
        y = 22 + r * cell
        d.text((4, y + 4), i, fill=(255, 255, 255))
        for c, (f, e) in enumerate(cols):
            for r_i, rt in enumerate(roots):
                p = os.path.join(rt, f, e, i + ".png")
                if not os.path.exists(p):
                    continue
                im = Image.open(p).convert("RGB")
                # The middle, where the item lies: its share of the view
                # grown so a small thing shows.
                W, H = im.size
                k = 0.5 if e == "arm" else 0.22
                box = (int(W * (0.5 - k / 2)), int(H * (0.5 - k / 2)), int(W * (0.5 + k / 2)), int(H * (0.5 + k / 2)))
                im = im.crop(box).resize((cell - 2, cell - 2))
                x = 170 + (c * len(roots) + r_i) * cell
                img.paste(im, (x, y))
    img.save(out_path, quality=88)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("root")
    ap.add_argument("--json")
    ap.add_argument("--sheet")
    ap.add_argument("--worst", type=int, default=20)
    ap.add_argument("--compare")
    ap.add_argument("--ids")
    a = ap.parse_args()
    root = os.path.join(a.root, "torchlit") if os.path.isdir(os.path.join(a.root, "torchlit")) else a.root
    meta, out = analyse(root)
    scored = []
    bad_shots = []
    for k, e in out.items():
        for v_k, v in e["views"].items():
            if v["bad"]:
                bad_shots.append(f"{k} {v_k}")
        s = score(e)
        if s:
            e["score"] = s
            scored.append((s["reads"], k, s))
    scored.sort()
    print(f"{len(scored)} items, {len(bad_shots)} bad shots")
    for b in bad_shots:
        print("  bad shot:", b)
    print("\nWorst by torchlight (delta E: the better of body and rim, at its worst floor and eye):")
    for reads, k, s in scored[: max(a.worst, 30)]:
        flag = []
        if s["contrast"] < LOW_DE:
            flag.append("body")
        if s["edge"] < LOW_DE:
            flag.append("rim")
        if s["deg_2m"] < SMALL_DEG:
            flag.append("small")
        print(f"  {k:<26} reads {reads:5.1f}  body {s['contrast']:5.1f}  rim {s['edge']:5.1f}  "
              f"{s['deg_2m']:.2f} deg at 2 m  worst {s['worst_view']:<14} {' '.join(flag)}")
    flagged = [k for r, k, s in scored if s["reads"] < LOW_DE or s["deg_2m"] < SMALL_DEG]
    print(f"\n{len(flagged)} items under delta E {LOW_DE} or {SMALL_DEG} deg")
    tw = twins(out)
    print("\nNearest twins from 2 m on wood (size + outline + colour):")
    for d, ka, kb, sz, sh, co in tw:
        print(f"  {d:.2f}  {ka} ~ {kb}  (size {sz:.2f} outline {sh:.2f} colour {co:.2f})")
    if a.json:
        json.dump({"items": out, "twins": tw, "flagged": flagged}, open(a.json, "w"), indent=1)
    if a.sheet:
        ids = a.ids.split(",") if a.ids else [k for _, k, _ in scored[: a.worst]]
        sheet(root, ids, a.sheet, compare=(os.path.join(a.compare, "torchlit") if a.compare and os.path.isdir(os.path.join(a.compare, "torchlit")) else a.compare), meta=meta)
        print("sheet:", a.sheet)


if __name__ == "__main__":
    sys.exit(main())
