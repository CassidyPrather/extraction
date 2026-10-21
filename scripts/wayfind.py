#!/usr/bin/env python3
"""Reads back the wayfinding gauge's shots (`--wayfind DIR`).

    scripts/wayfind.py DIR [--sheet OUT.png]

Each landmark was shot from each vantage (the truck's stop, every
rendezvous) twice, with it and without it, at the headset's pixels a
degree. What differs is the landmark as the eye gets it. A landmark you
can steer by covers at least MIN_AREA eye pixels (about a third of a
degree square: a mast against the sky) at a Weber contrast of at least
MIN_CONTRAST against what's behind it once fog and dark have had their
way; or, a light, at least LAMP_AREA pixels at LAMP_CONTRAST. Every vantage needs at least one. The exit status is the number of
vantages with none.

Time is frozen while a view is shot, the difference is taken only inside
the landmark's box in the picture, and only its largest blob counts.
"""
import json
import os
import sys

import numpy as np
from PIL import Image, ImageDraw, ImageFilter

MIN_AREA = 25
MIN_CONTRAST = 0.08
# A light: a few pixels, but standing out of the dark or the fog.
LAMP_AREA = 4
LAMP_CONTRAST = 0.4


def lum(path):
    a = np.asarray(Image.open(path).convert("RGB"), dtype=np.float32) / 255.0
    lin = np.where(a <= 0.04045, a / 12.92, ((a + 0.055) / 1.055) ** 2.4)
    return lin @ np.array([0.2126, 0.7152, 0.0722], dtype=np.float32)


def blob(mask):
    """The largest 4-connected blob of `mask`."""
    h, w = mask.shape
    seen = np.zeros_like(mask, dtype=bool)
    best = None
    for y0, x0 in zip(*np.nonzero(mask)):
        if seen[y0, x0]:
            continue
        stack = [(y0, x0)]
        seen[y0, x0] = True
        pts = []
        while stack:
            y, x = stack.pop()
            pts.append((y, x))
            for dy, dx in ((1, 0), (-1, 0), (0, 1), (0, -1)):
                ny, nx = y + dy, x + dx
                if 0 <= ny < h and 0 <= nx < w and mask[ny, nx] and not seen[ny, nx]:
                    seen[ny, nx] = True
                    stack.append((ny, nx))
        if best is None or len(pts) > len(best):
            best = pts
    out = np.zeros_like(mask, dtype=bool)
    if best:
        ys, xs = zip(*best)
        out[list(ys), list(xs)] = True
    return out


def v_rect(v):
    r = v.get("rect") or [0, 0, 0, 0]
    # A little slack round the box for the cross-faded far stand-in.
    return (r[0] - 4, r[1] - 4, r[2] + 8, r[3] + 8)


def measure(d, v):
    a = lum(os.path.join(d, v["with"]))
    b = lum(os.path.join(d, v["without"]))
    diff = np.abs(a - b)
    # Blur the difference a little: a streak of rain is a pixel wide.
    soft = np.asarray(Image.fromarray((np.clip(diff * 4, 0, 1) * 255).astype(np.uint8))
                      .filter(ImageFilter.BoxBlur(1)), dtype=np.float32) / 255.0 / 4
    # Only where the landmark's box falls: anything else that differs isn't it.
    x, y, w, h = (int(round(v)) for v in v_rect(v))
    inside = np.zeros_like(soft, dtype=bool)
    inside[max(0, y):y + h, max(0, x):x + w] = True
    mask = blob((soft > 0.004) & inside)
    area = int(mask.sum())
    if area == 0:
        return 0, 0.0, 0, None
    c = diff[mask] / np.maximum(b[mask], 0.01)
    contrast = float(np.median(c))
    # Its lamp, if it shows: the pixels well above the structure's own.
    lamp = int((c >= LAMP_CONTRAST).sum())
    ys, xs = np.nonzero(mask)
    return area, contrast, lamp, (int(xs.min()), int(ys.min()), int(xs.max()) + 1, int(ys.max()) + 1)


# --afford: an interactable from 1.5 m stands out from what it's on.
AFFORD_AREA = 60
AFFORD_CONTRAST = 0.15


def main(argv):
    afford = "--afford" in argv
    argv = [a for a in argv if a != "--afford"]
    sheet = None
    if "--sheet" in argv:
        i = argv.index("--sheet")
        sheet = argv[i + 1]
        argv = argv[:i] + argv[i + 2:]
    d = argv[0]
    views = json.load(open(os.path.join(d, "wayfind.json")))
    by_from = {}
    rows = []
    sky = {}
    for v in views:
        if v["from"] == "sky":
            p = os.path.join(d, v["with"])
            if os.path.exists(p):
                L = lum(p)
                sky[v["landmark"]] = float(L[: L.shape[0] // 2].mean())
            continue
        if not (os.path.exists(os.path.join(d, v["with"])) and os.path.exists(os.path.join(d, v["without"]))):
            continue
        area, contrast, lamp, box = measure(d, v)
        if afford:
            ok = (area >= AFFORD_AREA and contrast >= AFFORD_CONTRAST) or lamp >= LAMP_AREA
        else:
            ok = (area >= MIN_AREA and contrast >= MIN_CONTRAST) or lamp >= LAMP_AREA
        by_from.setdefault(v["from"], []).append((v, ok, area, contrast))
        rows.append((v, ok, area, contrast, box))
        print(f"{'ok  ' if ok else 'no  '} {v['from']:28} {v['landmark']:22} {v['distance']:5.0f} m  "
              f"{area:6d} px  contrast {contrast:.2f}  lamp {lamp} px")
    if len(sky) == 2:
        a, b = sky.get("sky toward the light", 0.0), sky.get("sky away from it", 0.0)
        k = a / max(b, 1e-6)
        print(f"sky: toward the sun or moon {k:.2f}x as bright as away"
              f" ({'a compass' if k >= 1.1 else 'no help'})")
    if afford:
        lost = [(v, a, c) for v, ok, a, _l, c, _b in [(r[0], r[1], r[2], 0, r[3], r[4]) for r in rows] if not ok]
        for v, a, c in lost:
            print(f"doesn't stand out: {v['from']}: {v['landmark']} ({a} px, contrast {c:.2f})")
        print(f"{len(rows)} things, {len(lost)} don't stand out")
        if sheet:
            contact(d, rows, sheet)
        return len(lost)
    blind = [f for f, vs in by_from.items() if not any(ok for _, ok, _, _ in vs)]
    for f in sorted(by_from):
        seen = [v["landmark"] for v, ok, _, _ in by_from[f] if ok]
        print(f"{f}: {len(seen)} landmark(s) to steer by: {', '.join(seen) or 'NONE'}")
    if sheet:
        contact(d, rows, sheet)
    return len(blind)


def contact(d, rows, out, cols=4, cell=(320, 260)):
    W, H = cell
    n = len(rows)
    img = Image.new("RGB", (W * cols, H * ((n + cols - 1) // cols)), (30, 30, 30))
    dr = ImageDraw.Draw(img)
    for i, (v, ok, area, contrast, box) in enumerate(rows):
        im = Image.open(os.path.join(d, v["with"])).convert("RGB")
        im.thumbnail((W - 8, H - 30))
        x, y = (i % cols) * W, (i // cols) * H
        img.paste(im, (x + 4, y + 4))
        if box:
            k = im.width / Image.open(os.path.join(d, v["with"])).width
            dr.rectangle([x + 4 + box[0] * k, y + 4 + box[1] * k, x + 4 + box[2] * k, y + 4 + box[3] * k],
                         outline=(120, 220, 120) if ok else (240, 90, 80))
        dr.text((x + 4, y + H - 22), f"{v['from'][:14]} > {v['landmark'][:14]} {v['distance']:.0f}m c{contrast:.2f}",
                fill=(120, 220, 120) if ok else (240, 90, 80))
    img.save(out)
    print(out)


if __name__ == "__main__":
    sys.exit(min(main(sys.argv[1:]), 125))
