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
    cx = w // 2

    # A side's own colour: a strip `a`..`b` metres from the edge (the
    # frame's centre; under 0 to the left), the middle rows.
    def side(a, b):
        x0, x1 = sorted((int(cx + a * ppm), int(cx + b * ppm)))
        cs = [px[x, y] for y in range(h // 4, 3 * h // 4, 4) for x in range(max(x0, 0), min(x1, w), 2)]
        return tuple(median([c[i] for c in cs]) for i in range(3))

    # The grass: the greener of the two far strips (a narrow road's far
    # side is grass again). The road: the side, and the colour, of the
    # column within 2.5 m of the edge least like the grass (where the
    # edge wanders, a fixed strip a metre in can be verge, not road).
    greenness = lambda c: c[1] - c[0]
    def column(x):
        col = [px[x, y] for y in range(h // 4, 3 * h // 4, 4)]
        return tuple(median([q[i] for q in col]) for i in range(3))

    # The greenest column 3.5-6.5 m out either side (a strip there can be
    # a narrow road's other verge).
    far = [
        column(int(cx + s * k))
        for s in (-1, 1)
        for k in range(int(3.5 * ppm), int(6.5 * ppm), max(int(0.1 * ppm), 1))
        if 0 <= int(cx + s * k) < w
    ]
    grass = max(far, key=greenness)
    best = None
    for s in (-1, 1):
        for k in range(int(0.3 * ppm), int(2.5 * ppm), max(int(0.1 * ppm), 1)):
            x = int(cx + s * k)
            if not 0 <= x < w:
                continue
            c = column(x)
            d = dist(c, grass)
            if best is None or d > best[0]:
                best = (d, s, c)
    _, s, road = best
    if dist(road, grass) < 40:
        return None
    # Each row from 2.5 m in on the road to 6.5 m out on the grass.
    xs = [int(cx + s * 2.5 * ppm - s * k) for k in range(int(9.0 * ppm))]
    xs = [x for x in xs if 0 <= x < w]
    # Each band of rows (0.6 m of road) as one profile, each column's
    # median over the band: the grass tufts' dark blades are a few pixels
    # here and there, and a single row read them as road.
    band = max(int(0.6 * ppm), 8)
    widths, mids = [], []
    for y0 in range(0, h - band + 1, band // 2):
        # Per column: how much like the road it is, and how much like the
        # grass, each on its own (a verge's own colour, a gravel lip's
        # yellow, is neither: nearer one or the other, it still isn't it).
        span = dist(road, grass)
        t, rn, gn = [], [], []
        for x in xs:
            ct, cr, cg = [], [], []
            for y in range(y0, y0 + band, 2):
                c = px[x, y]
                dr, dg = dist(c, road), dist(c, grass)
                ct.append(dg / (dr + dg + 1e-6))
                cr.append(1.0 - dr / span)
                cg.append(1.0 - dg / span)
            t.append(median(ct))
            rn.append(median(cr))
            gn.append(median(cg))
        nt = len(t)
        # Out from the road: where it stops being 90% road, and where it
        # is first 90% grass (a patch further out isn't this edge's).
        a = next((x for x in range(nt) if rn[x] < 0.9), None)
        b = next((x for x in range(a or 0, nt) if gn[x] >= 0.9), None)
        m = next((x for x in range(nt) if t[x] < 0.5), None)
        if a is None or b is None or m is None or b < a:
            continue
        # A profile clipped by its window tells nothing.
        if a == 0 or b == nt - 1:
            continue
        widths.append((b - a) / ppm)
        mids.append((y0 + band // 2, m))
    if len(mids) < 6:
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
