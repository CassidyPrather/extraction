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


# The sheet in the map case and its lettering, in millimetres on the paper.
# You read it held up to your face (0.3 m): letters these sizes clear the
# must-read floor there (docs/LEGIBILITY.md; `render::legibility` reads
# these lines, so keep them as plain numbers).
MAP_MM = 280.0
MAP_PX = 1024.0
TITLE_MM = 8.5
NAME_MM = 6.4
LABEL_MM = 6.2
# Landmarks: anything this tall (m; map_lint::LANDMARK_HEIGHT, which a test
# holds this to) is drawn as a pencilled tower with its red lamp on top, this
# tall on the sheet (mm), and named, so the lamp you see on the skyline
# finds its spot on the paper.
LANDMARK_M = 12.0
LANDMARK_MM = 5.0
# What each landmark is called on the map (a test holds every landmark the
# maps place to a name here).
LANDMARK_NAMES = {
    "radio_mast": "mast",
    "water_tower": "water tower",
    "chimney": "chimney",
    "ranger_tower": "lookout",
    "robot_gate": "gate",
    "mill_race": "mill",
    "ropeway_station": "ropeway",
    "ropeway_pylon": "pylon",
    "chapel": "chapel",
    "steeple": "steeple",
    "headframe": "pithead",
    "blast_furnace": "furnace",
    "dam_face": "dam",
    "dam_face_end": "dam",
    "dam_back": "dam",
    "dam_back_end": "dam",
    "draw_off_tower": "intake tower",
    "sorting_gantry": "gantry",
    "ore_mill": "ore mill",
    "observatory": "observatory",
    "funicular_upper": "top station",
    "survey_tower": "survey beacon",
}
RUIN_STATES = ("_damaged", "_burnt", "_drowned")


def prefab_base(id):
    for r in RUIN_STATES:
        if id.endswith(r):
            return id[: -len(r)]
    return id


def landmark_name(prefab):
    b = prefab_base(prefab)
    return LANDMARK_NAMES.get(b, b.replace("_", " "))


def overlap(a, b):
    w = min(a[2], b[2]) - max(a[0], b[0])
    h = min(a[3], b[3]) - max(a[1], b[1])
    return max(0.0, w) * max(0.0, h)


def load(d, name, n):
    return np.fromfile(os.path.join(d, name), dtype="<f4").reshape(n, n)


def font(size, italic=False):
    # Upright: an italic's slanted, thin strokes are the first thing to go
    # at a headset's few pixels a letter.
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
        if p["prefab"] in ("fence", "quarantine_fence") or p["size"][1] >= LANDMARK_M:
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
    mm = N / MAP_MM
    f_med = font(round(NAME_MM * mm))
    f_small = font(round(LABEL_MM * mm))
    pencil = (55, 50, 48)
    # A paper-coloured halo round the letters: names stay clear where they
    # cross a road, a contour or the forest's marks.
    HALO = {"stroke_width": max(1, round(0.5 * mm)), "stroke_fill": (214, 200, 168)}
    red = (140, 30, 25)
    # Everything lettered or marked claims its box on the sheet; labels then
    # take the first free spot near what they name, so none sits on another.
    taken = []
    margin = 6

    def box_of(xy, text, f):
        b = d.textbbox(xy, text, font=f, stroke_width=HALO["stroke_width"])
        return (b[0] - 2, b[1] - 2, b[2] + 2, b[3] + 2)

    def place(text, f, spots):
        best = None
        for xy in spots:
            b = box_of(xy, text, f)
            off = max(0, margin - b[0]) + max(0, margin - b[1]) + max(0, b[2] - N + margin) + max(0, b[3] - N + margin)
            cost = sum(overlap(b, t) for t in taken) + off * 100
            if best is None or cost < best[0]:
                best = (cost, xy, b)
            if cost == 0:
                break
        taken.append(best[2])
        return best[1]

    labels = []  # (xy, text, font, colour), drawn last

    # Fixed marks first: the title, the north arrow, the scale, the
    # rendezvous rings, the gate's cross, the landmarks' towers.
    f_title = font(round(TITLE_MM * mm))
    taken.append(box_of((24, 16), meta["name"], f_title))
    taken.append((N - 56, 12, N - 24, 96))
    taken.append((18, N - 40 - LABEL_MM * mm * 1.6, 30 + 100 * s, N - 24))
    gh = LANDMARK_MM * mm
    marks = [p for p in meta["placements"] if p["size"][1] >= LANDMARK_M]
    for p in marks:
        x, y = px(p["at"])
        taken.append((x - gh * 0.4, y - gh * 0.85, x + gh * 0.4, y + gh * 0.4))
    for m in meta["markers"]:
        x, y = px(m["at"])
        if m["kind"] in ("Rendezvous", "RobotGate"):
            r = 9 if m["kind"] == "Rendezvous" else 13
            taken.append((x - r, y - r, x + r, y + r))

    def around(x, y, w, h, gap):
        # Right, left, above, below, then the corners, then further out.
        out = []
        for k in (1.0, 1.8, 2.6):
            g = gap * k
            out += [
                (x + g, y - h / 2),
                (x - g - w, y - h / 2),
                (x - w / 2, y - g - h),
                (x - w / 2, y + g),
                (x + g, y - g - h),
                (x - g - w, y - g - h),
                (x + g, y + g),
                (x - g - w, y + g),
            ]
        return out

    # Marker labels: they must stay clear.
    for m in meta["markers"]:
        x, y = px(m["at"])
        if m["kind"] == "Rendezvous":
            t = "RV"
            w = d.textlength(t, font=f_small)
            spots = [(x + 9, y + 4 - LABEL_MM * mm / 2)] + around(x, y, w, LABEL_MM * mm, 10)
            labels.append((place(t, f_small, spots), t, f_small, red))
        elif m["kind"] == "RobotGate":
            t = "keep away"
            w = d.textlength(t, font=f_small)
            spots = [(x - w / 2, y + 10)] + around(x, y, w, LABEL_MM * mm, 12)
            labels.append((place(t, f_small, spots), t, f_small, red))
        elif m["kind"] == "ArtifactField":
            t = "? ? ?"
            w = d.textlength(t, font=f_med)
            spots = [(x - 6, y + 12)] + around(x, y, w, NAME_MM * mm, 14)
            labels.append((place(t, f_med, spots), t, f_med, (90, 40, 110)))
        elif m["kind"] == "Insertion":
            t = "truck drops here"
            w = d.textlength(t, font=f_small)
            spots = [(x + 10, y + 10)] + around(x, y, w, LABEL_MM * mm, 10)
            labels.append((place(t, f_small, spots), t, f_small, pencil))

    # Landmark names. A landmark built in pieces (the dam's four faces) is
    # named once, at the piece nearest its middle.
    named = []
    groups = []
    for p in marks:
        name = landmark_name(p["prefab"])
        near = [g for g in groups if g[0] == name and any(math.dist(p["at"], q["at"]) < 130 for q in g[1])]
        merged = (name, [p] + [q for g in near for q in g[1]])
        groups = [g for g in groups if g not in near] + [merged]
    for name, ps in groups:
        cx = sum(q["at"][0] for q in ps) / len(ps)
        cz = sum(q["at"][1] for q in ps) / len(ps)
        named.append((name, min(ps, key=lambda q: math.dist(q["at"], (cx, cz)))))
    for name, p in named:
        x, y = px(p["at"])
        y -= gh * 0.25
        w = d.textlength(name, font=f_small)
        labels.append((place(name, f_small, around(x, y, w, LABEL_MM * mm, gh * 0.55)), name, f_small, pencil))

    # Region names: at the middle, or nudged off what's already there.
    for r in meta["regions"]:
        xs = [px(p)[0] for p in r["polygon"]]
        ys = [px(p)[1] for p in r["polygon"]]
        cx, cy = sum(xs) / len(xs), sum(ys) / len(ys)
        w = d.textlength(r["name"], font=f_med)
        hh = NAME_MM * mm
        spots = [
            (cx - w / 2 + dx * w * 0.35, cy - hh / 2 + dy * hh * 0.8)
            for dy, dx in sorted(
                ((dy, dx) for dy in range(-4, 5) for dx in (-1, 0, 1)),
                key=lambda t: abs(t[0]) + abs(t[1]) * 1.5,
            )
        ]
        labels.append((place(r["name"], f_med, spots), r["name"], f_med, pencil))

    # Draw: the marks, then every label over them.
    for m in meta["markers"]:
        x, y = px(m["at"])
        if m["kind"] == "Rendezvous":
            d.ellipse([x - 7, y - 7, x + 7, y + 7], outline=red, width=2)
        elif m["kind"] == "RobotGate":
            # Under the gate's own tower, if it has one: a cross to keep off.
            d.line([(x - 11, y - 11), (x + 11, y + 11)], fill=red, width=3)
            d.line([(x - 11, y + 11), (x + 11, y - 11)], fill=red, width=3)
    paper_ink = HALO["stroke_fill"]
    for p in marks:
        x, y = px(p["at"])
        # A filled tower standing on its spot, its red lamp on top.
        top, base, half = y - gh * 0.6, y + gh * 0.4, gh * 0.26
        tri = [(x - half, base), (x + half, base), (x, top)]
        d.polygon(tri, fill=paper_ink, outline=paper_ink, width=max(1, round(0.5 * mm)))
        d.polygon(tri, fill=(40, 34, 30))
        lr = max(2.0, 0.7 * mm)
        d.ellipse([x - lr - 1, top - lr - 1, x + lr + 1, top + lr + 1], fill=paper_ink)
        d.ellipse([x - lr, top - lr, x + lr, top + lr], fill=(190, 30, 20))
    for xy, t, f, c in labels:
        d.text(xy, t, fill=c, font=f, **HALO)
    # North arrow and a pencilled scale.
    d.text((N - 50, 18), "N", fill=pencil, font=f_med, **HALO)
    d.line([(N - 42, 44), (N - 42, 90)], fill=pencil, width=2)
    d.polygon([(N - 48, 50), (N - 36, 50), (N - 42, 40)], fill=pencil)
    bar = 100 * s
    d.line([(24, N - 30), (24 + bar, N - 30)], fill=pencil, width=2)
    d.text((24, N - 36 - LABEL_MM * mm * 1.2), "100 m (paced)", fill=pencil, font=f_small, **HALO)
    d.text((24, 16), meta["name"], fill=pencil, font=f_title, **HALO)
    img = img.filter(ImageFilter.GaussianBlur(0.4))
    img.save(args.out, quality=86)
    print(args.out)


if __name__ == "__main__":
    main()
