#!/usr/bin/env python3
"""Lay out the "what's new" gallery from scripts/capture_new.sh's shots.

    scripts/capture_new.py <shots dir> <gallery dir>

Reads <shots>/turn (turntables), <shots>/after and <shots>/before (stills
by map and mood) and <shots>/reels (frame sequences). Writes JPEGs, MP4s
and index.html into <gallery>, replacing what was there.
"""

import html
import os
import shutil
import sys

from PIL import Image, ImageDraw

WIDTH = 1280

# What each subject is, for the captions.
TURNS = {
    "robots__spotter": "The spotter: a balloon on a tether, watching a road",
    "robots__minelayer": "The minelayer and its mine",
    "robots__mender": "The mender, which welds other machines back together",
    "robots__plough": "Heavy's plough (the model, its parts laid out)",
    "props__gramophone": "A gramophone",
    "props__upright_piano": "An upright piano",
}
VIEWS = {
    "n_minefield": "The minelayer's field at Marrow's village, each mine in its ring of turned earth",
    "n_wreck_lada": "A Lada wreck on the slope",
    "n_wreck_uaz": "A UAZ wreck on the slope",
    "n_far_band": "Sallow's far tree line against the sky in drizzle",
}
PAIRS = {
    "n_minefield": "The mines drew 3 m from their rings of earth, two on the road. Now each sits in its own ring.",
    "n_wreck_lada": "Wrecks stood level on a slope, wheels in the air or sunk, with ivy up their sides. Now they pitch and roll onto all four wheels, and no ivy.",
    "n_wreck_uaz": "The same, on the UAZ.",
    "n_far_band": "The sky was fogged only in a fixed band near the horizon, so the far trees stood pale against clear cloud like cards. Now the sky fogs as the scene does.",
}
# Orbits shot with --cine-preview (<shots>/cine/<name>/<shot>/fNNNNN.png):
# caption, and which of the frames to show.
CINE = {
    "plough": ("Rime Fell's plough on the pass road below Coldharbour, its plume up", (1, 2)),
    "plough_after": ("And after it has gone by: the packed lane it leaves (no prints for a while) and the bank it threw, loot buried in it", (1, 2)),
}
# Reels: caption, and the part of each frame to show (fractions of its
# width and height: the robots, without the player's hands at the bottom).
REELS = {
    "tells_dusk": ("Damage you can read at 24-36 m, at dusk: a walker on a shot leg, one with its eye shot dark, one with its core failing, and a drone with a rotor hit", (0.34, 0.34, 0.66, 0.52)),
    "tells_day": ("The same by day", (0.34, 0.34, 0.66, 0.52)),
    "snow_prints": ("A walker leaving prints in the lying snow at Rime Fell's col", (0.25, 0.2, 1.0, 0.75)),
}


def jpeg(src, dst, width=WIDTH):
    im = Image.open(src).convert("RGB")
    if im.width > width:
        im = im.resize((width, im.height * width // im.width))
    im.save(dst, quality=86)


def labelled_pair(before, after, dst, width=WIDTH):
    a = Image.open(before).convert("RGB")
    b = Image.open(after).convert("RGB")
    w = width // 2
    a = a.resize((w, a.height * w // a.width))
    b = b.resize((w, b.height * w // b.width))
    sheet = Image.new("RGB", (width, max(a.height, b.height)))
    sheet.paste(a, (0, 0))
    sheet.paste(b, (w, 0))
    d = ImageDraw.Draw(sheet)
    for x, text in ((8, "BEFORE"), (w + 8, "AFTER")):
        d.rectangle((x - 4, 6, x + 62, 24), fill=(0, 0, 0))
        d.text((x, 9), text, fill=(255, 255, 255))
    sheet.save(dst, quality=86)


def reel_sheet(frames, dst, crop=None, cols=3, width=WIDTH):
    w = width // cols
    ims = [Image.open(f).convert("RGB") for f in frames]
    if crop:
        ims = [
            im.crop((int(crop[0] * im.width), int(crop[1] * im.height),
                     int(crop[2] * im.width), int(crop[3] * im.height)))
            for im in ims
        ]
    ims = [im.resize((w, im.height * w // im.width)) for im in ims]
    rows = (len(ims) + cols - 1) // cols
    h = ims[0].height
    sheet = Image.new("RGB", (w * cols, h * rows))
    for i, im in enumerate(ims):
        sheet.paste(im, ((i % cols) * w, (i // cols) * h))
    sheet.save(dst, quality=86)


def main(argv):
    shots, gallery = argv[0], argv[1]
    if os.path.isdir(gallery):
        shutil.rmtree(gallery)
    os.makedirs(gallery)
    sections = []

    # Turntables.
    items = []
    for light in ("day", "night"):
        ldir = os.path.join(shots, "turn", light)
        if not os.path.isdir(ldir):
            continue
        for subject in sorted(TURNS):
            mp4 = os.path.join(ldir, f"{subject}.mp4")
            first = os.path.join(ldir, subject, "t000.png")
            if not os.path.exists(first):
                continue
            base = f"turn_{subject}_{light}"
            jpeg(first, os.path.join(gallery, base + ".jpg"), 960)
            video = None
            if os.path.exists(mp4):
                video = base + ".mp4"
                shutil.copy(mp4, os.path.join(gallery, video))
            items.append((f"{TURNS[subject]} ({light})", base + ".jpg", video))
    if items:
        sections.append(("New machines and props", "Turntables, by day and by night.", items))

    # Stills by day and night, and the before/after pairs.
    after = os.path.join(shots, "after")
    before = os.path.join(shots, "before")
    rime, new_views, pairs = [], [], []
    for m in sorted(os.listdir(after)) if os.path.isdir(after) else []:
        for mood in ("day", "clear", "night"):
            mdir = os.path.join(after, m, mood)
            if not os.path.isdir(mdir):
                continue
            for f in sorted(os.listdir(mdir)):
                if not f.endswith(".png"):
                    continue
                view = f[:-4]
                base = f"{m}_{view}_{mood}"
                jpeg(os.path.join(mdir, f), os.path.join(gallery, base + ".jpg"))
                if m == "rime_fell":
                    rime.append((f"{view[4:].replace('_', ' ')} ({mood})", base + ".jpg", None))
                else:
                    caption = VIEWS.get(view, view)
                    new_views.append((f"{caption}: {m.replace('_', ' ')} ({mood})", base + ".jpg", None))
                b = os.path.join(before, m, mood, f)
                if os.path.exists(b):
                    name = f"pair_{m}_{view}.jpg"
                    labelled_pair(b, os.path.join(mdir, f), os.path.join(gallery, name))
                    pairs.append((f"{PAIRS.get(view, view)} ({m.replace('_', ' ')})", name, None))
    if pairs:
        sections.append(("Before and after", "The same view from the same build with the fix taken out (left) and from main (right).", pairs))
    if rime:
        sections.append(("Rime Fell", "The fifth valley's landmarks from its lookbook views.", rime))
    if new_views:
        sections.append(("In the valleys", "", new_views))

    # Orbits.
    items = []
    cdir = os.path.join(shots, "cine")
    for name, (caption, pick) in CINE.items():
        frames = sorted(
            os.path.join(root, f)
            for root, _, fs in os.walk(os.path.join(cdir, name))
            for f in fs
            if f.endswith(".png")
        )
        for k in pick:
            if k < len(frames):
                out = f"cine_{name}_{k}.jpg"
                jpeg(frames[k], os.path.join(gallery, out))
                items.append((caption, out, None))
    if items:
        sections.append(("The plough", "Heavy's plough, in the game now.", items))

    # Reels.
    items = []
    rdir = os.path.join(shots, "reels")
    for name in sorted(os.listdir(rdir)) if os.path.isdir(rdir) else []:
        frames = sorted(
            os.path.join(rdir, name, f) for f in os.listdir(os.path.join(rdir, name)) if f.startswith("f_")
        )
        if not frames:
            continue
        caption, crop = REELS.get(name, (name, None))
        reel_sheet(frames, os.path.join(gallery, f"reel_{name}.jpg"), crop)
        items.append((caption, f"reel_{name}.jpg", None))
    if items:
        sections.append(("Machines in motion", "Frame sequences, read left to right, top to bottom.", items))

    with open(os.path.join(gallery, "index.html"), "w") as f:
        f.write(
            "<!doctype html><meta charset=utf-8><meta name=viewport content='width=device-width'>"
            "<title>What's new</title><style>"
            ":root{--bg:#f6f6f4;--fg:#1d1d1b;--muted:#666}"
            "@media (prefers-color-scheme:dark){:root{--bg:#161615;--fg:#eee;--muted:#aaa}}"
            "body{background:var(--bg);color:var(--fg);font:15px/1.45 system-ui,sans-serif;margin:0 auto;max-width:1300px;padding:16px}"
            "h2{margin-top:2em}p.note{color:var(--muted)}"
            ".grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(300px,1fr));gap:14px}"
            ".pairs{grid-template-columns:1fr}"
            "figure{margin:0}img,video{width:100%;height:auto;display:block;border-radius:4px}"
            "figcaption{font-size:13px;color:var(--muted);margin-top:4px}</style>"
            "<h1>What's new</h1><p class=note>Made by <code>scripts/capture_new.sh</code>; see docs/CAPTURE.md.</p>"
        )
        for title, note, items in sections:
            cls = "grid pairs" if title.startswith("Before") else "grid"
            f.write(f"<h2>{html.escape(title)}</h2>")
            if note:
                f.write(f"<p class=note>{html.escape(note)}</p>")
            f.write(f"<div class='{cls}'>")
            for caption, img, video in items:
                if video:
                    body = f"<video src='{video}' poster='{img}' loop muted autoplay playsinline></video>"
                else:
                    body = f"<a href='{img}'><img src='{img}' loading=lazy alt=''></a>"
                f.write(f"<figure>{body}<figcaption>{html.escape(caption)}</figcaption></figure>")
            f.write("</div>")
    print(f"{sum(len(s[2]) for s in sections)} items in {gallery}/index.html")


if __name__ == "__main__":
    main(sys.argv[1:])
