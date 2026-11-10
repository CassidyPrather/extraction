#!/usr/bin/env python3
"""How soft the road edges are: the gauge for `--terrain-debug layers`.

Each view is a top-down debug frame over a road's edge, 6 m up, the road
running up the frame (`scripts/road_edges.sh` shoots them from the maps'
roads). Every ground layer is one flat colour, so across each pixel row
the road gives way to the grass plainly. Per row:

- width: metres from the last 90%-road pixel to the first 90%-grass one
  (the road's own lip, its verge, the grass taking it back), and
- wander: how far the 50% line strays from a straight fit over the frame's
  9 m of road (a seam ruled along the terrain's triangles hardly strays).

A road fails if its median width is under its floor (gravel, dirt and
ballast verges `VERGE`; asphalt's crumbling lip `LIP`), or if its edge
runs ruler-straight (wander under `WANDER`).

    scripts/road_edges.py DIR [DIR...]     (each DIR: top_*.png from a tour)

Exit status 1 if any road fails.
"""
import math
import sys
from pathlib import Path

from PIL import Image

# Floors (m): a verge reads as worn ground, not a line, over this much.
VERGE = 1.5
LIP = 1.0
# The 50% line's least RMS stray from straight (m) over the frame.
WANDER = 0.10
# The views: 6 m over the ground, a 75 degree vertical field of view.
HEIGHT = 6.0
VFOV = 75.0


def px_per_m(h_px):
    return (h_px / 2) / math.tan(math.radians(VFOV / 2)) / HEIGHT


def dist(a, b):
    return math.sqrt(sum((x - y) ** 2 for x, y in zip(a, b)))


def median(v):
    v = sorted(v)
    return v[len(v) // 2] if v else float("nan")


def measure(png):
    im = Image.open(png).convert("RGB")
    w, h = im.size
    px = im.load()
    ppm = px_per_m(h)
    # The two sides' own colours: a strip at each side, the middle rows.
    def side(x0, x1):
        cs = [px[x, y] for y in range(h // 4, 3 * h // 4, 4) for x in range(x0, x1, 2)]
        return tuple(median([c[i] for c in cs]) for i in range(3))

    left, right = side(0, w // 12), side(w - w // 12, w)
    # The grass is the green side; the road the other.
    greenness = lambda c: c[1] - c[0]
    road_left = greenness(left) < greenness(right)
    road, grass = (left, right) if road_left else (right, left)
    if dist(road, grass) < 40:
        return None
    widths, mids = [], []
    for y in range(0, h, 4):
        t = []
        for x in range(w):
            c = px[x, y]
            dr, dg = dist(c, road), dist(c, grass)
            t.append(dg / (dr + dg + 1e-6))  # 1 road, 0 grass
        if not road_left:
            t = t[::-1]
        # From the road side: where it stops being 90% road; from the
        # grass side: where it stops being 90% grass.
        a = next((x for x in range(w) if t[x] < 0.9), None)
        b = next((x for x in range(w - 1, -1, -1) if t[x] > 0.1), None)
        m = next((x for x in range(w) if t[x] < 0.5), None)
        if a is None or b is None or m is None or b < a:
            continue
        # A row clipped by the frame tells nothing.
        if a == 0 or b == w - 1:
            continue
        widths.append((b - a) / ppm)
        mids.append((y, m))
    if len(mids) < 20:
        return None
    # The 50% line's stray from a straight fit.
    n = len(mids)
    sy = sum(y for y, _ in mids)
    sx = sum(x for _, x in mids)
    syy = sum(y * y for y, _ in mids)
    syx = sum(y * x for y, x in mids)
    k = (n * syx - sy * sx) / max(n * syy - sy * sy, 1e-6)
    c = (sx - k * sy) / n
    rms = math.sqrt(sum((x - (k * y + c)) ** 2 for y, x in mids) / n) / ppm
    return median(widths), rms, len(widths)


def main(dirs):
    bad = 0
    for d in dirs:
        for png in sorted(Path(d).glob("top_*.png")):
            kind = png.stem.split("_")[-1]
            r = measure(png)
            if r is None:
                print(f"--   {d}/{png.name}: no road edge to measure")
                continue
            width, rms, rows = r
            floor = LIP if kind == "asphalt" else VERGE
            fails = []
            if width < floor:
                fails.append(f"width {width:.2f} m < {floor}")
            if rms < WANDER:
                fails.append(f"ruled (wander {rms:.2f} m < {WANDER})")
            status = "FAIL" if fails else "ok  "
            bad += bool(fails)
            print(
                f"{status} {Path(d).name}/{png.stem}: width {width:.2f} m, "
                f"wander {rms:.2f} m ({rows} rows){'  ' + '; '.join(fails) if fails else ''}"
            )
    print(f"{bad} road edge(s) under the floor")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
