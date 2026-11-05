#!/usr/bin/env python3
"""Can the way home be read at night? Scores the frames
`scripts/night_route.sh` shoots along each first job's way home.

    scripts/night_route.py WALKS.jsonl FRAMES_DIR --case torch|dark [--sheet OUT.png] [--worst N]

Each stop is a line of WALKS.jsonl, from `night-walks`. Two cases:

- torch (shot with the hand torch): inside its pool, 6 to 16 m ahead, does
  the way stand out from the ground 2 m either side of it (worn track, or
  an edge: a wall, a kerb, a fence)? |way - beside| over beside's
  brightness, at each point; READ when `TORCH_POINTS` of them stand out by
  `STANDS_OUT`.
- dark (no torch, a newcomer's raid): is something lit within `CONE`
  degrees of the way ahead, a lamp, a beacon, a window? Lit is coloured
  and bright (`LIT`, `COLOURED`), which leaves out the stars. What stands
  against the sky isn't scored (every treeline breaks the skyline): judge
  that off the sheet.

LOST is a stop where you can't tell which way the way goes.
Prints one line a stop, then the LOST stretches (runs of LOST stops), and
with --sheet tiles the worst stops' frames, the way ahead marked.

Exit status: the number of LOST stops (at most 125).
"""
import json
import math
import os
import sys

from PIL import Image, ImageDraw

# The stills camera's vertical field of view (capture::cinematic::STILL_FOV).
FOV = 75.0
# How much the way must stand out from beside it, for a point to.
STANDS_OUT = 0.12
# Where night-walks samples the way ahead (m): night_walks.rs's AHEAD.
AHEAD_M = [6.0, 9.0, 12.0, 16.0, 20.0, 25.0, 30.0, 40.0]
# Below this the frame is black there: nothing to read either way.
FLOOR = 4.0
# Torch: the points inside the pool (m ahead), and how many must stand out.
POOL = (5.0, 17.0)
TORCH_POINTS = 2
# Dark: how far either side of the way ahead a light counts (degrees), and
# what's lit: this bright (0-255) and this coloured ((max-min)/max).
CONE = 30.0
LIT = 40
COLOURED = 0.35


def project(eye, yaw, pitch, p, w, h):
    y, t = math.radians(yaw), math.radians(pitch)
    f = (math.sin(y) * math.cos(t), math.sin(t), -math.cos(y) * math.cos(t))
    r = (math.cos(y), 0.0, math.sin(y))
    u = (r[1] * f[2] - r[2] * f[1], r[2] * f[0] - r[0] * f[2], r[0] * f[1] - r[1] * f[0])
    d = [p[i] - eye[i] for i in range(3)]
    z = sum(d[i] * f[i] for i in range(3))
    if z < 0.5:
        return None
    x = sum(d[i] * r[i] for i in range(3))
    v = sum(d[i] * u[i] for i in range(3))
    fy = (h / 2) / math.tan(math.radians(FOV) / 2)
    px, py = w / 2 + x / z * fy, h / 2 - v / z * fy
    if 2 <= px < w - 2 and 2 <= py < h - 2:
        return px, py
    return None


def lum(im, at):
    x, y = int(at[0]), int(at[1])
    vals = []
    for dy in (-1, 0, 1):
        for dx in (-1, 0, 1):
            r, g, b = im.getpixel((x + dx, y + dy))[:3]
            vals.append(0.2126 * r + 0.7152 * g + 0.0722 * b)
    vals.sort()
    return vals[len(vals) // 2]


def score_dark(stop, im):
    """The lit pixels within CONE of the way ahead (the frame's middle,
    the camera looking along the way), as (brightness, x, y)."""
    w, h = im.size
    fy = (h / 2) / math.tan(math.radians(FOV) / 2)
    half = fy * math.tan(math.radians(CONE))
    x0, x1 = int(max(0, w / 2 - half)), int(min(w, w / 2 + half))
    px = im.load()
    lit = []
    for y in range(0, h, 2):
        for x in range(x0, x1, 2):
            r, g, b = px[x, y][:3]
            m = max(r, g, b)
            if m >= LIT and (m - min(r, g, b)) / m >= COLOURED:
                lit.append((m, x, y))
    return lit


def score(stop, im):
    w, h = im.size
    out = []
    for i, (a, l, r) in enumerate(zip(stop["ahead"], stop["near_left"], stop["near_right"])):
        if not POOL[0] <= AHEAD_M[i] <= POOL[1]:
            continue
        pa = project(stop["eye"], stop["yaw"], stop["pitch"], a, w, h)
        pl = project(stop["eye"], stop["yaw"], stop["pitch"], l, w, h)
        pr = project(stop["eye"], stop["yaw"], stop["pitch"], r, w, h)
        if not pa or not (pl or pr):
            continue
        t = lum(im, pa)
        sides = [lum(im, q) for q in (pl, pr) if q]
        s = sum(sides) / len(sides)
        if max(t, s) < FLOOR:
            c = 0.0
        else:
            c = abs(t - s) / max(s, FLOOR)
        out.append((c, pa))
    return out


def main(argv):
    sheet, worst_n, case, rest = None, 12, "torch", []
    it = iter(argv)
    for a in it:
        if a == "--sheet":
            sheet = next(it)
        elif a == "--case":
            case = next(it)
        elif a == "--worst":
            worst_n = int(next(it))
        else:
            rest.append(a)
    walks, frames = rest
    stops = [json.loads(l) for l in open(walks) if l.strip()]
    results = []
    for s in stops:
        f = os.path.join(frames, s["name"] + ".png")
        if not os.path.exists(f):
            print(f"MISSING {s['name']}")
            continue
        im = Image.open(f).convert("RGB")
        if case == "dark":
            lit = score_dark(s, im)
            ok = len(lit) > 0
            share = 1.0 if ok else 0.0
            med = max((m for m, _, _ in lit), default=0)
            pts = [(1.0, (x, y)) for _, x, y in lit[:40]]
            print(
                f"{'READ' if ok else 'LOST'}  {s['name']}  {s['walk']}  stop {s['stop']}: "
                f"{len(lit)} lit points within {CONE:.0f} deg (brightest {med})  at {s['eye'][0]:.0f},{s['eye'][2]:.0f}"
            )
        else:
            pts = score(s, im)
            read = sum(1 for c, _ in pts if c >= STANDS_OUT)
            ok = read >= TORCH_POINTS
            share = read / len(pts) if pts else 0.0
            med = sorted(c for c, _ in pts)[len(pts) // 2] if pts else 0.0
            print(
                f"{'READ' if ok else 'LOST'}  {s['name']}  {s['walk']}  stop {s['stop']}: "
                f"{read}/{len(pts)} points in the pool stand out (median {med:.2f})  at {s['eye'][0]:.0f},{s['eye'][2]:.0f}"
            )
        results.append((s, ok, share, med, pts, f))
    lost = [r for r in results if not r[1]]
    # Stretches: runs of LOST stops along one walk.
    runs, cur = [], []
    for r in results:
        if not r[1]:
            if cur and cur[-1][0]["walk"] == r[0]["walk"] and cur[-1][0]["stop"] + 1 == r[0]["stop"]:
                cur.append(r)
            else:
                if cur:
                    runs.append(cur)
                cur = [r]
    if cur:
        runs.append(cur)
    for run in runs:
        a, b = run[0][0], run[-1][0]
        print(
            f"stretch: {a['walk']} stops {a['stop']}-{b['stop']} "
            f"({len(run) * 50} m from {a['eye'][0]:.0f},{a['eye'][2]:.0f})"
        )
    print(f"{len(lost)} of {len(results)} stops where the way home can't be read")
    if sheet and results:
        worst = sorted(results, key=lambda r: (r[2], r[3]))[:worst_n]
        cells = []
        for s, ok, share, med, pts, f in worst:
            im = Image.open(f).convert("RGB")
            d = ImageDraw.Draw(im)
            for c, (x, y) in pts:
                col = (60, 255, 90) if c >= STANDS_OUT else (255, 60, 60)
                d.ellipse((x - 3, y - 3, x + 3, y + 3), outline=col)
            d.text((6, 6), f"{s['name']} {share:.0%}", fill=(255, 255, 0))
            cells.append(im)
        cw, ch = cells[0].size
        cols = 4
        rows = (len(cells) + cols - 1) // cols
        out = Image.new("RGB", (cw * cols, ch * rows))
        for i, c in enumerate(cells):
            out.paste(c, ((i % cols) * cw, (i // cols) * ch))
        out.save(sheet)
    return min(len(lost), 125)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
