#!/usr/bin/env python3
"""Draws a top-down design preview of a map from `map-dump` output.

Usage:
    scripts/map_preview.py <dump-dir> <out.png> [--heat heat.f32] [--paths paths.json]
                           [--lint findings.json]

Hillshade and contours for the land, water, roads by kind, forests, region
outlines tinted by danger, markers with labels, a 50 m grid. Optional
overlays: a heatmap (raw f32 grid the same size as the heightmap) and
polyline paths (JSON list of {"points": [[x, z], ...], "color": "#rrggbb"}),
such as the map lint's main routes, and the lint's findings (JSON list of
{"rule": ..., "what": ..., "at": [x, z]}), ringed and numbered with a key.
`scripts/map-preview.sh` draws both from `cargo test -p extraction_core
map_lint` (`LINT_DUMP`).
"""

import argparse
import json
import os

import numpy as np
from PIL import Image, ImageDraw, ImageFont

ROAD_COLORS = {
    1: (70, 70, 74),  # Asphalt
    2: (150, 140, 120),  # Gravel
    3: (120, 95, 70),  # Dirt
    4: (60, 50, 45),  # Rail
    5: (140, 120, 90),  # Path
}
MARKER_STYLE = {
    "Insertion": ((80, 220, 120), "▲"),
    "Rendezvous": ((80, 200, 255), "◆"),
    "ShortExfil": ((200, 200, 80), "◇"),
    "RobotGate": ((255, 80, 80), "■"),
    "Loot": ((255, 200, 60), "•"),
    "Poi": ((255, 255, 255), "○"),
    "ArtifactField": ((200, 120, 255), "✶"),
    "Offering": ((255, 150, 220), "☉"),
    "WorkSite": ((255, 140, 60), "⚒"),
    "TruckStop": ((80, 220, 120), "▭"),
    "Sentry": ((255, 60, 60), "▼"),
    "Spotter": ((255, 170, 60), "◎"),
}
# A sentry's reach and eye (extraction_core::sentry SIGHT, FOV).
SENTRY_SIGHT = 55.0
SENTRY_FOV = 50.0
# A spotter's reach and the blind ring under it (extraction_core::spotter
# SIGHT, blind_radius(TETHER)).
SPOTTER_SIGHT = 85.0
SPOTTER_BLIND = 24.0 / np.tan(np.radians(50.0))
DANGER = [(120, 200, 120), (230, 210, 90), (240, 140, 60), (230, 60, 60)]


def load(dirname, name, n):
    return np.fromfile(os.path.join(dirname, name), dtype="<f4").reshape(n, n)


def font(size):
    for path in [
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
    ]:
        if os.path.exists(path):
            return ImageFont.truetype(path, size)
    return ImageFont.load_default()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("dump")
    ap.add_argument("out")
    ap.add_argument("--px", type=int, default=1400)
    ap.add_argument("--heat")
    ap.add_argument("--paths")
    ap.add_argument("--lint")
    ap.add_argument("--sight", help="map-dump's sight.json: shade ground with no landmark in sight")
    ap.add_argument("--title")
    args = ap.parse_args()
    heat = None
    if args.heat:
        meta = json.load(open(os.path.join(args.dump, "map.json")))
        heat = np.fromfile(args.heat, dtype="<f4").reshape(meta["n"], meta["n"])
    img, _, px = render(args.dump, args.px, heat=heat, title=args.title)
    d = ImageDraw.Draw(img)
    if args.sight and os.path.exists(args.sight):
        # Red where no landmark shows within fog range; deeper where none
        # shows even in ordinary weather.
        sg = json.load(open(args.sight))
        ov = Image.new("RGBA", img.size, (0, 0, 0, 0))
        od = ImageDraw.Draw(ov)
        half = sg["step"] / 2
        for k, d in enumerate(sg["nearest"]):
            if d is None:
                continue
            x = sg["origin"] + (k % sg["n"]) * sg["step"]
            z = sg["origin"] + (k // sg["n"]) * sg["step"]
            if d < 0:
                col = (220, 30, 30, 120)
            elif d > sg["fog"]:
                col = (240, 120, 40, 70)
            else:
                continue
            a_, b_ = px((x - half, z - half)), px((x + half, z + half))
            od.rectangle([a_, b_], fill=col)
        img = Image.alpha_composite(img.convert("RGBA"), ov).convert("RGB")
        d = ImageDraw.Draw(img)
    if args.paths and os.path.exists(args.paths):
        for path in json.load(open(args.paths)):
            pts = [px(p) for p in path["points"]]
            if len(pts) > 1:
                d.line(pts, fill=path.get("color", "#40e0ff"), width=max(2, args.px // 400))
    if args.lint and os.path.exists(args.lint):
        findings = json.load(open(args.lint))
        f_small = font(max(11, args.px // 110))
        for i, f in enumerate(findings):
            x, y = px(f["at"])
            r = max(8, args.px // 90)
            d.ellipse([x - r, y - r, x + r, y + r], outline=(255, 60, 60), width=2)
            d.text((x + r, y - r), str(i + 1), fill=(255, 90, 90), font=f_small)
        y0 = 40
        for i, f in enumerate(findings[:40]):
            d.text((12, y0), f'{i + 1}. {f["rule"]}: {f["what"]}', fill=(255, 120, 120), font=f_small)
            y0 += f_small.size + 3
    img.save(args.out)
    print(args.out)


def render(dump, px_size=1400, heat=None, title=None, dim=0.0):
    """The preview as a PIL RGB image, plus the map metadata and a function
    from map (x, z) to image pixels. `heat` is an optional n x n array
    (heightmap grid) drawn as a heatmap; `dim` darkens the base map (0..1)
    so overlays read."""

    class Args:
        pass

    args = Args()
    args.dump = dump
    args.px = px_size
    args.title = title
    meta = json.load(open(os.path.join(args.dump, "map.json")))
    n = meta["n"]
    h = load(args.dump, "height.f32", n)
    water = load(args.dump, "water.f32", n)
    roads = load(args.dump, "roads.f32", n)
    wet = load(args.dump, "wet.f32", n)

    # Hillshade from the north-west.
    gz, gx = np.gradient(h, meta["cell"])
    az, alt = np.radians(315), np.radians(40)
    slope = np.arctan(np.hypot(gx, gz))
    aspect = np.arctan2(-gx, gz)
    shade = np.sin(alt) * np.cos(slope) + np.cos(alt) * np.sin(slope) * np.cos(az - aspect)
    shade = np.clip(shade, 0, 1)

    # Hypsometric tint: dark wet lowland green to grey-brown high ground.
    lo, hi = np.percentile(h, 1), np.percentile(h, 99)
    t = np.clip((h - lo) / max(hi - lo, 1e-3), 0, 1)[..., None]
    low = np.array([62, 78, 52], dtype=float)
    high = np.array([120, 112, 96], dtype=float)
    rgb = low * (1 - t) + high * t
    rgb *= (0.45 + 0.75 * shade)[..., None]
    # Mud where wet.
    mud = np.array([70, 58, 44], dtype=float)
    rgb = rgb * (1 - 0.5 * wet[..., None]) + mud * 0.5 * wet[..., None] * (0.6 + 0.4 * shade[..., None])
    # Roads.
    kind = (roads // 1000).astype(int)
    weight = (roads % 1000) / 255.0
    for k, col in ROAD_COLORS.items():
        m = (kind == k)[..., None] * weight[..., None]
        rgb = rgb * (1 - m) + np.array(col, dtype=float) * m
    # Water.
    wmask = ~np.isnan(water) & (water > h)
    depth = np.where(wmask, water - h, 0)
    wcol = np.array([40, 70, 90], dtype=float)
    a = np.clip(0.55 + depth, 0, 0.95)[..., None] * wmask[..., None]
    rgb = rgb * (1 - a) + wcol * a
    # Contours every 5 m.
    cont = (np.floor(h / 5.0) != np.floor(np.roll(h, 1, 0) / 5.0)) | (
        np.floor(h / 5.0) != np.floor(np.roll(h, 1, 1) / 5.0)
    )
    rgb[cont] *= 0.75

    rgb *= 1.0 - dim
    if heat is not None:
        hv = heat / max(heat.max(), 1e-6)
        hc = np.stack([np.clip(hv * 3, 0, 1), np.clip(hv * 3 - 1, 0, 1), np.clip(hv * 3 - 2, 0, 1)], -1) * 255
        ha = np.clip(hv * 1.5, 0, 0.85)[..., None]
        rgb = rgb * (1 - ha) + hc * ha

    img = Image.fromarray(np.clip(rgb, 0, 255).astype(np.uint8), "RGB")
    img = img.resize((args.px, args.px), Image.BILINEAR)
    s = args.px / meta["size"]
    o = meta["origin"]

    def px(p):
        return ((p[0] - o) * s, (p[1] - o) * s)

    overlay = Image.new("RGBA", img.size, (0, 0, 0, 0))
    d = ImageDraw.Draw(overlay)
    # Forests: translucent green fill and outline.
    for f in meta["forests"]:
        poly = [px(p) for p in f["polygon"]]
        d.polygon(poly, fill=(20, 60, 25, 70), outline=(40, 110, 50, 200))
    # Regions.
    for r in meta["regions"]:
        poly = [px(p) for p in r["polygon"]]
        c = DANGER[min(r["danger"], 3)]
        d.polygon(poly, outline=c + (180,))
    # Grid.
    step = 50
    half = meta["size"] / 2
    for v in np.arange(-half, half + 1, step):
        a_ = px((v, -half))
        b_ = px((v, half))
        d.line([a_, b_], fill=(255, 255, 255, 25))
        a_ = px((-half, v))
        b_ = px((half, v))
        d.line([a_, b_], fill=(255, 255, 255, 25))
    img = Image.alpha_composite(img.convert("RGBA"), overlay)
    d = ImageDraw.Draw(img)
    f_small = font(max(11, args.px // 110))
    f_big = font(max(14, args.px // 60))
    # Trees and blocking rocks, as `map-dump` scattered them.
    tpath = os.path.join(args.dump, "trees.f32")
    if os.path.exists(tpath):
        for x, z in np.fromfile(tpath, dtype="<f4").reshape(-1, 2):
            tx, ty = px((x, z))
            d.ellipse([tx - 1.5, ty - 1.5, tx + 1.5, ty + 1.5], fill=(90, 160, 80))
    rpath = os.path.join(args.dump, "rocks.f32")
    if os.path.exists(rpath):
        for x, z, r in np.fromfile(rpath, dtype="<f4").reshape(-1, 3):
            rx, ry = px((x, z))
            rr = max(1.0, r * s)
            d.ellipse([rx - rr, ry - rr, rx + rr, ry + rr], fill=(150, 150, 140))
    # Bridges.
    for b in meta["bridges"]:
        d.line([px(b[0]), px(b[1])], fill=(200, 190, 170), width=max(3, int(6 * s)))
    # Placements: footprint (to scale, turned as placed) and name.
    for p in meta["placements"]:
        x, y = px(p["at"])
        if "size" in p:
            # As the nav grid and the lint turn it.
            hw, hd = p["size"][0] / 2, p["size"][2] / 2
            yaw = np.radians(p["rot"])
            c, s_ = np.cos(yaw), np.sin(yaw)
            corners = [
                px((p["at"][0] + lx * c - lz * s_, p["at"][1] + lx * s_ + lz * c))
                for lx, lz in [(-hw, -hd), (hw, -hd), (hw, hd), (-hw, hd)]
            ]
            d.polygon(corners, fill=(30, 30, 30), outline=(230, 230, 230))
        else:
            d.rectangle([x - 4, y - 4, x + 4, y + 4], fill=(30, 30, 30), outline=(230, 230, 230))
        d.text((x + 6, y - 6), p["prefab"], fill=(220, 220, 220), font=f_small)
    # Sentries: the ground they can ever look at, and the cable to the box.
    for m in meta["markers"]:
        if m["kind"] != "Sentry":
            continue
        half = m["arc"] / 2 + SENTRY_FOV / 2
        wedge = [px(m["at"])]
        for k in range(25):
            b = np.radians(m["rot"] - half + 2 * half * k / 24)
            wedge.append(px((m["at"][0] + np.sin(b) * SENTRY_SIGHT, m["at"][1] - np.cos(b) * SENTRY_SIGHT)))
        d.polygon(wedge, outline=(255, 60, 60))
        d.line([px(m["at"]), px(m["power"])], fill=(255, 220, 60), width=2)
        bx, by = px(m["power"])
        d.rectangle([bx - 3, by - 3, bx + 3, by + 3], fill=(255, 220, 60))
    # Spotters: how far they see, and the blind ring under the balloon.
    for m in meta["markers"]:
        if m["kind"] != "Spotter":
            continue
        x, y = px(m["at"])
        for r, w in ((SPOTTER_SIGHT, 2), (SPOTTER_BLIND, 1)):
            d.ellipse([x - r * s, y - r * s, x + r * s, y + r * s], outline=(255, 170, 60), width=w)
    # The minelayer's mines.
    for at in meta.get("mines", []):
        x, y = px(at)
        d.ellipse([x - 3, y - 3, x + 3, y + 3], fill=(255, 40, 40), outline=(0, 0, 0))
    # Markers.
    for m in meta["markers"]:
        col, glyph = MARKER_STYLE.get(m["kind"], ((255, 255, 255), "?"))
        x, y = px(m["at"])
        d.text((x - 6, y - 9), glyph, fill=col, font=f_big)
        if m["name"] and m["kind"] not in ("Poi",):
            d.text((x + 9, y + 2), m["name"], fill=col, font=f_small)
    # Region labels.
    for r in meta["regions"]:
        xs = [px(p)[0] for p in r["polygon"]]
        ys = [px(p)[1] for p in r["polygon"]]
        c = DANGER[min(r["danger"], 3)]
        d.text((min(xs) + 4, min(ys) + 2), f'{r["name"]} [{r["danger"]}]', fill=c, font=f_small)
    # Title, scale bar, north arrow.
    title = args.title or meta["name"]
    d.text((12, 10), title, fill=(255, 255, 255), font=f_big)
    bar = 100 * s
    y0 = args.px - 24
    d.line([(12, y0), (12 + bar, y0)], fill=(255, 255, 255), width=3)
    d.text((12, y0 - 18), "100 m", fill=(255, 255, 255), font=f_small)
    d.text((args.px - 30, 10), "N↑", fill=(255, 255, 255), font=f_big)
    return img.convert("RGB"), meta, px


if __name__ == "__main__":
    main()
