#!/usr/bin/env python3
"""Can the way home be read at night? Scores the frames
`scripts/night_route.sh` shoots along each first job's way home.

    scripts/night_route.py WALKS.jsonl FRAMES_DIR [--sheet OUT.png] [--worst N]

For each stop (a line of WALKS.jsonl, from `night-walks`), finds in its
frame the ground the way runs over 6 to 40 m ahead and the ground 4 m
either side of it, and asks whether the way stands out from what's beside
it: |way - beside| over beside's brightness, at each point in the frame.
A stop is READ when at least `ENOUGH` of its points stand out by
`STANDS_OUT`; LOST when not (you can't tell which way the way goes).
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
# How much the way must stand out from beside it, and at how many of its
# points in frame, for a stop to read.
STANDS_OUT = 0.12
ENOUGH = 0.4
# Below this the frame is black there: nothing to read either way.
FLOOR = 4.0


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


def score(stop, im):
    w, h = im.size
    out = []
    for a, l, r in zip(stop["ahead"], stop["left"], stop["right"]):
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
    sheet, worst_n, rest = None, 12, []
    it = iter(argv)
    for a in it:
        if a == "--sheet":
            sheet = next(it)
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
        pts = score(s, im)
        read = sum(1 for c, _ in pts if c >= STANDS_OUT)
        share = read / len(pts) if pts else 0.0
        ok = len(pts) >= 3 and share >= ENOUGH
        med = sorted(c for c, _ in pts)[len(pts) // 2] if pts else 0.0
        results.append((s, ok, share, med, pts, f))
        print(
            f"{'READ' if ok else 'LOST'}  {s['name']}  {s['walk']}  stop {s['stop']}: "
            f"{read}/{len(pts)} points stand out (median {med:.2f})  at {s['eye'][0]:.0f},{s['eye'][2]:.0f}"
        )
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
