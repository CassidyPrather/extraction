#!/usr/bin/env python3
"""Press-kit stills: a contact sheet to choose from, and publishing the
chosen ones.

    scripts/capture_presskit.py [DIR]                 # DIR/index.html
    scripts/capture_presskit.py [DIR] --publish MAP/MOOD/VIEW ...

DIR (default target/presskit) holds <map>/<mood>/<view>.png from
scripts/capture_presskit.sh. The contact sheet ranks each mood's stills by a
rough eye: tonal range, colour, detail, and no great areas crushed to black
or blown to white. It's a sort order, not a judge; look at them.

--publish copies the named stills into docs/shots/showcase/ as JPEGs
(<map>_<mood>_<view>.jpg, 1600 wide) and redraws docs/shots/showcase/index.html
from everything there.
"""
import html
import os
import sys

import numpy as np
from PIL import Image

SHOWCASE = "docs/shots/showcase"
VALLEYS = {
    "marrow_creek": "Marrow Creek",
    "kettle_pass": "Kettle Pass",
    "cinder_works": "Cinder Works",
    "training_yard": "Training Yard",
}
MOODS = ["golden", "night", "rain", "surge", "day"]


def score(path):
    im = Image.open(path).convert("RGB")
    im.thumbnail((480, 480))
    a = np.asarray(im).astype(np.float32)
    lum = a @ np.array([0.2126, 0.7152, 0.0722], dtype=np.float32)
    p5, p95 = np.percentile(lum, [5, 95])
    rg, yb = a[..., 0] - a[..., 1], 0.5 * (a[..., 0] + a[..., 1]) - a[..., 2]
    colour = np.hypot(rg.std(), yb.std()) + 0.3 * np.hypot(rg.mean(), yb.mean())
    gy, gx = np.gradient(lum)
    detail = np.hypot(gx, gy).mean()
    crushed = (lum < 6).mean() + (lum > 250).mean()
    s = (p95 - p5) / 255 * 40 + colour / 4 + detail * 2 - crushed * 60
    return round(float(s), 1), {
        "range": round(float(p95 - p5)), "colour": round(float(colour)),
        "detail": round(float(detail), 1), "crushed": round(float(crushed), 3)}


def contact_sheet(root):
    css = ("body{background:#0d0d0d;color:#ddd;font:13px sans-serif;margin:16px}"
           ".row{display:flex;flex-wrap:wrap;gap:6px}.c{width:384px}"
           ".c img{width:384px;display:block}.c div{font-size:12px;color:#aaa}"
           "h2{margin:18px 0 6px}h3{margin:10px 0 4px;color:#fc9}")
    out = [f"<!doctype html><meta charset=utf-8><title>Press-kit stills</title><style>{css}</style>",
           "<h1>Press-kit stills</h1><p>Sorted best-first by a rough eye "
           "(tonal range, colour, detail, nothing crushed). Publish with "
           "<code>scripts/capture_presskit.py --publish MAP/MOOD/VIEW</code>.</p>"]
    for m in sorted(os.listdir(root)):
        md = os.path.join(root, m)
        if not os.path.isdir(md):
            continue
        out.append(f"<h2>{html.escape(VALLEYS.get(m, m))}</h2>")
        for mood in sorted(os.listdir(md), key=lambda x: MOODS.index(x) if x in MOODS else 99):
            d = os.path.join(md, mood)
            shots = sorted(f for f in os.listdir(d) if f.endswith(".png")) if os.path.isdir(d) else []
            if not shots:
                continue
            ranked = sorted(((score(os.path.join(d, f)), f) for f in shots), key=lambda x: -x[0][0])
            out.append(f"<h3>{mood}</h3><div class=row>")
            for (s, parts), f in ranked:
                rel = f"{m}/{mood}/{f}"
                out.append(f"<div class=c><a href='{rel}'><img loading=lazy src='{rel}'></a>"
                           f"<div>{m}/{mood}/{f[:-4]} &middot; {s} &middot; {parts}</div></div>")
            out.append("</div>")
    with open(os.path.join(root, "index.html"), "w") as f:
        f.write("\n".join(out))
    print(f"{root}/index.html")


def publish(root, names):
    os.makedirs(SHOWCASE, exist_ok=True)
    for n in names:
        m, mood, view = n.split("/")
        src = os.path.join(root, m, mood, view + ".png")
        im = Image.open(src).convert("RGB")
        if im.width > 1600:
            im = im.resize((1600, round(im.height * 1600 / im.width)), Image.LANCZOS)
        dst = os.path.join(SHOWCASE, f"{m}_{mood}_{view}.jpg")
        im.save(dst, quality=87, optimize=True, progressive=True)
        print(dst)
    showcase_index()


def showcase_index():
    files = sorted(f for f in os.listdir(SHOWCASE) if f.endswith(".jpg"))
    stills = [f for f in files if not f.startswith("sheet_")]
    css = ("body{background:#0d0d0d;color:#ddd;font:14px sans-serif;margin:24px;max-width:1700px}"
           "h1{font-weight:400}h2{margin:28px 0 8px;font-weight:400;color:#fc9}"
           ".row{display:flex;flex-wrap:wrap;gap:8px}.c{width:400px}"
           ".c img{width:400px;display:block}.c div{font-size:12px;color:#999;margin:2px 0 8px}"
           "a{color:#9cf}")
    out = [f"<!doctype html><meta charset=utf-8><title>Extraction showcase</title><style>{css}</style>",
           "<h1>Showcase stills</h1><p>Made with <code>scripts/capture_presskit.sh</code>; "
           "see <a href='../../CAPTURE.md'>docs/CAPTURE.md</a>.</p>"]
    for m, name in VALLEYS.items():
        mine = [f for f in stills if f.startswith(m + "_")]
        if not mine:
            continue
        out.append(f"<h2>{html.escape(name)}</h2><div class=row>")
        for f in sorted(mine, key=lambda f: (MOODS.index(f[len(m) + 1:].split("_")[0])
                                             if f[len(m) + 1:].split("_")[0] in MOODS else 99, f)):
            label = f[len(m) + 1:-4].replace("_", " ", 1)
            out.append(f"<div class=c><a href='{f}'><img loading=lazy src='{f}'></a><div>{html.escape(label)}</div></div>")
        out.append("</div>")
    others = [f for f in stills if not any(f.startswith(m + "_") for m in VALLEYS)]
    if others:
        out.append("<h2>Other</h2><div class=row>")
        for f in others:
            out.append(f"<div class=c><a href='{f}'><img loading=lazy src='{f}'></a><div>{html.escape(f[:-4])}</div></div>")
        out.append("</div>")
    with open(os.path.join(SHOWCASE, "index.html"), "w") as f:
        f.write("\n".join(out))
    print(f"{SHOWCASE}/index.html: {len(stills)} stills")


def main(argv):
    root = "target/presskit"
    rest = list(argv)
    if rest and not rest[0].startswith("--"):
        root = rest.pop(0)
    if rest and rest[0] == "--publish":
        publish(root, rest[1:])
    elif rest and rest[0] == "--index":
        showcase_index()
    else:
        contact_sheet(root)


if __name__ == "__main__":
    main(sys.argv[1:])
