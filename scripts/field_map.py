#!/usr/bin/env python3
"""Draws the in-game paper map (the map case) from a map dump: ink contours
on stained paper, roads, the creek, forests as tree marks, buildings, and
hand-lettered names. Not a design tool (that's map_preview.py): this is the
thing your character drew.

    scripts/field-map.sh [map]   # dumps the map, then runs this
    python3 scripts/field_map.py <dump-dir> <out.jpg> [--px 1024]
"""
import argparse
import json
import math
import os

import numpy as np
from PIL import Image, ImageDraw, ImageFilter, ImageFont


def load(d, name, n):
    return np.fromfile(os.path.join(d, name), dtype="<f4").reshape(n, n)


def font(size, italic=True):
    for p in [
        "/usr/share/fonts/truetype/dejavu/DejaVuSerif-Italic.ttf" if italic else "",
        "/usr/share/fonts/truetype/dejavu/DejaVuSerif.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    ]:
        if p and os.path.exists(p):
            return ImageFont.truetype(p, size)
    return ImageFont.load_default()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("dump")
    ap.add_argument("out")
    ap.add_argument("--px", type=int, default=1024)
    args = ap.parse_args()
    meta = json.load(open(os.path.join(args.dump, "map.json")))
    n = meta["n"]
    h = load(args.dump, "height.f32", n)
    water = load(args.dump, "water.f32", n)
    roads = load(args.dump, "roads.f32", n)
    rng = np.random.default_rng(3)

    # Paper: warm, blotchy, darker toward the edges.
    N = args.px
    yy, xx = np.mgrid[0:N, 0:N] / N
    blot = rng.normal(size=(N // 16, N // 16))
    blot = np.array(Image.fromarray(((blot - blot.min()) / np.ptp(blot) * 255).astype(np.uint8)).resize((N, N), Image.BICUBIC)) / 255.0
    edge = np.clip(np.minimum.reduce([xx, yy, 1 - xx, 1 - yy]) * 6, 0, 1)
    paper = np.array([214, 200, 168], float) * (0.86 + 0.1 * blot[..., None]) * (0.8 + 0.2 * edge[..., None])
    paper += rng.normal(scale=5, size=(N, N, 1))

    def grid_to_px(a):
        return np.array(Image.fromarray(a.astype(np.float32)).resize((N, N), Image.BILINEAR))

    hp = grid_to_px(h)
    ink = np.zeros((N, N))
    # Contours every 5 m, every fifth darker.
    for step, weight in [(5.0, 0.35), (25.0, 0.75)]:
        b = np.floor(hp / step)
        c = (b != np.roll(b, 1, 0)) | (b != np.roll(b, 1, 1))
        ink = np.maximum(ink, c * weight)
    brown = np.array([110, 72, 40], float)
    rgb = paper * (1 - ink[..., None] * 0.8) + brown * ink[..., None] * 0.8
    # Water: a blue-grey wash with a darker edge.
    wp = grid_to_px(np.nan_to_num(water, nan=-1e9))
    wet = wp > hp
    wash = np.array([120, 140, 150], float)
    rgb[wet] = rgb[wet] * 0.45 + wash * 0.55
    shore = wet ^ np.roll(wet, 1, 0) | wet ^ np.roll(wet, 1, 1)
    rgb[shore] = np.array([60, 75, 85], float)
    # Roads: double ink lines; tracks dashed.
    rp = grid_to_px(roads)
    kind = (rp // 1000).astype(int)
    on = (rp % 1000) > 120
    inner = on & (np.roll(on, 2, 0) & np.roll(on, -2, 0) & np.roll(on, 2, 1) & np.roll(on, -2, 1))
    outline = on & ~inner
    dash = ((xx * N // 6 + yy * N // 6).astype(int) % 2 == 0)
    dark = np.array([40, 34, 30], float)
    paved = (kind == 1) | (kind == 2) | (kind == 4)
    rgb[outline & paved] = dark
    rgb[outline & ~paved & dash] = dark
    rgb[inner & (kind == 1)] = rgb[inner & (kind == 1)] * 0.9

    img = Image.fromarray(np.clip(rgb, 0, 255).astype(np.uint8), "RGB")
    d = ImageDraw.Draw(img)
    s = N / meta["size"]
    o = meta["origin"]

    def px(p):
        return ((p[0] - o) * s, (p[1] - o) * s)

    # Forests: little tree marks scattered in the polygons.
    from PIL import ImagePath  # noqa: F401

    def inside(poly, p):
        x, y = p
        c = False
        j = len(poly) - 1
        for i in range(len(poly)):
            xi, yi = poly[i]
            xj, yj = poly[j]
            if (yi > y) != (yj > y) and x < (xj - xi) * (y - yi) / (yj - yi + 1e-9) + xi:
                c = not c
            j = i
        return c

    tree = (60, 72, 50)
    for f in meta["forests"]:
        poly = f["polygon"]
        xs = [p[0] for p in poly]
        zs = [p[1] for p in poly]
        for _ in range(int((max(xs) - min(xs)) * (max(zs) - min(zs)) / 220)):
            p = (rng.uniform(min(xs), max(xs)), rng.uniform(min(zs), max(zs)))
            if not inside(poly, p):
                continue
            x, y = px(p)
            r = rng.uniform(2.2, 3.4) * N / 1024
            d.line([(x, y + r), (x, y + r * 2.2)], fill=tree, width=1)
            d.polygon([(x - r, y + r), (x + r, y + r), (x, y - r * 1.6)], outline=tree)
    # Buildings: filled squares with a little rotation.
    for p in meta["placements"]:
        if p["prefab"] in ("fence", "quarantine_fence"):
            continue
        x, y = px(p["at"])
        r = 4.0 * N / 1024
        a = math.radians(p.get("rot", 0))
        pts = [
            (x + (dx * math.cos(a) - dy * math.sin(a)) * r, y + (dx * math.sin(a) + dy * math.cos(a)) * r)
            for dx, dy in [(-1, -0.7), (1, -0.7), (1, 0.7), (-1, 0.7)]
        ]
        d.polygon(pts, fill=(52, 44, 38))
    # Names, in pencil.
    f_med = font(max(14, N // 52))
    f_small = font(max(11, N // 75))
    pencil = (55, 50, 48)
    for r in meta["regions"]:
        xs = [px(p)[0] for p in r["polygon"]]
        ys = [px(p)[1] for p in r["polygon"]]
        cx, cy = sum(xs) / len(xs), sum(ys) / len(ys)
        w = d.textlength(r["name"], font=f_med)
        d.text((cx - w / 2, cy - 8), r["name"], fill=pencil, font=f_med)
    for m in meta["markers"]:
        x, y = px(m["at"])
        if m["kind"] == "Rendezvous":
            d.ellipse([x - 7, y - 7, x + 7, y + 7], outline=(140, 30, 25), width=2)
            d.text((x + 9, y + 4), "RV", fill=(140, 30, 25), font=f_small)
        elif m["kind"] == "RobotGate":
            d.line([(x - 7, y - 7), (x + 7, y + 7)], fill=(140, 30, 25), width=3)
            d.line([(x - 7, y + 7), (x + 7, y - 7)], fill=(140, 30, 25), width=3)
            d.text((x - 20, y + 10), "keep away", fill=(140, 30, 25), font=f_small)
        elif m["kind"] == "ArtifactField":
            d.text((x - 6, y + 12), "? ? ?", fill=(90, 40, 110), font=f_med)
        elif m["kind"] == "Insertion":
            d.text((x + 10, y + 10), "truck drops here", fill=pencil, font=f_small)
    # North arrow and a pencilled scale.
    d.text((N - 50, 18), "N", fill=pencil, font=f_med)
    d.line([(N - 42, 44), (N - 42, 90)], fill=pencil, width=2)
    d.polygon([(N - 48, 50), (N - 36, 50), (N - 42, 40)], fill=pencil)
    bar = 100 * s
    d.line([(24, N - 30), (24 + bar, N - 30)], fill=pencil, width=2)
    d.text((24, N - 52), "100 m (paced)", fill=pencil, font=f_small)
    d.text((24, 16), meta["name"], fill=pencil, font=font(max(18, N // 36)))
    img = img.filter(ImageFilter.GaussianBlur(0.4))
    img.save(args.out, quality=86)
    print(args.out)


if __name__ == "__main__":
    main()
