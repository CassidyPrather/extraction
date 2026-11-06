#!/usr/bin/env python3
"""Fail on a blank frame; lay the frames out on a contact sheet.

    scripts/frame_check.py [--sheet out.png] [--empty PREFIX]
                           [--lineup DIST,FOV] frame.png ...

A frame is blank when it's one flat colour with nothing in it, as a
camera under the ground, in a wall or looking into nothing shows: its
luminance spread (2nd to 98th percentile) under SPREAD, or its standard
deviation under STD, over the whole frame or its middle (where a gallery
puts what it shows). A view of a valley measures 38-71 and 9-17; the
blank frames of a camera 3.7 km under the ground measured 1 and 0.4.
With --empty, each frame is also held against PREFIX<weather>.png, the
same view with nothing put in it (the weather the word after `_` in the
frame's name): a frame whose middle differs from it by under SHOWN (mean
absolute difference, 0-255) didn't draw what it was meant to show. (A
line-up of machines 20-50 m off measures 0.2-8; the same view with
nothing drawn, 0.0.)
With --lineup, a frame is a `--gallery robots:` line-up DIST m ahead
under a FOV-degree (vertical) camera, its machines named in its file name
after the weather (`standing_clearing_walker-drone.png`), each MACHINE_GAP
apart (dev.rs: the larger of 3.5 m and 0.16 DIST): each machine's own
column is held against the empty view's: one with under PER_MACHINE of its
pixels changed by over CHANGED didn't draw.
Exits 1 if any frame is blank, shows nothing, or is missing.
"""

import sys
from PIL import Image, ImageDraw, ImageStat

SPREAD = 12
STD = 4.0
SHOWN = 0.1
# A machine's column: pixels changed by over CHANGED, at least PER_MACHINE
# of them. Measured at 960x540, a line-up 17 m off: a standing hound 194
# (259 in a downpour), a wrecked heavy 1488-1570; the empty view against
# itself 0, and a column with nothing in it 0-2 (the rain changes almost
# nothing over 12).
CHANGED = 12
PER_MACHINE = 40
# A wrecked flyer (a drone, a mender) is a small heap in the grass, an
# ember and its smoke (a wrecked mender 21, a drone 56): held to less.
SMALL_WRECK = {"drone": 15, "mender": 15}


def measure(im):
    g = im.convert("L").resize((256, 256))
    px = sorted(g.tobytes())
    n = len(px)
    return px[int(0.98 * n)] - px[int(0.02 * n)], ImageStat.Stat(g).stddev[0]


def check(path):
    try:
        im = Image.open(path).convert("RGB")
    except OSError as e:
        return None, f"unreadable: {e}"
    w, h = im.size
    whole = measure(im)
    middle = measure(im.crop((w // 4, h // 4, 3 * w // 4, 3 * h // 4)))
    blank = any(s < SPREAD or d < STD for s, d in (whole, middle))
    note = (
        f"spread {whole[0]:3d}/{middle[0]:3d}  std {whole[1]:4.1f}/{middle[1]:4.1f}"
        + ("  BLANK" if blank else "")
    )
    return im, note if not blank else note


def shown(im, empty):
    """How much the middle of `im` differs from `empty`'s (0-255)."""
    w, h = im.size
    box = (w // 6, h // 4, 5 * w // 6, 3 * h // 4)
    a = im.crop(box).convert("L").resize((240, 135))
    b = empty.resize(im.size).crop(box).convert("L").resize((240, 135))
    diff = [abs(x - y) for x, y in zip(a.tobytes(), b.tobytes())]
    return sum(diff) / len(diff)


def columns(im, empty, n, dist, fov):
    """Each of a line-up's `n` machines' columns: how many of its pixels
    differ from `empty`'s by over CHANGED (0-255), left to right as the
    line-up stands, over the band a machine 17 m off stands in."""
    import math

    w, h = im.size
    gap = max(3.5, 0.16 * dist)
    half_w = math.tan(math.radians(fov) / 2) * w / h
    a_all, b_all = im.convert("L"), empty.resize(im.size).convert("L")
    out = []
    for i in range(n):
        off = (i - (n - 1) / 2) * gap
        x = w / 2 + (off / dist) / half_w * (w / 2)
        box = (int(x - w * 0.04), int(h * 0.1), int(x + w * 0.04), int(h * 0.6))
        a, b = a_all.crop(box).tobytes(), b_all.crop(box).tobytes()
        out.append(sum(1 for p, q in zip(a, b) if abs(p - q) > CHANGED))
    return out


def sheet(frames, out, cols=4, cell=(400, 225)):
    rows = (len(frames) + cols - 1) // cols
    s = Image.new("RGB", (cols * cell[0], rows * (cell[1] + 18)), (20, 20, 20))
    d = ImageDraw.Draw(s)
    for i, (path, im, note) in enumerate(frames):
        x, y = (i % cols) * cell[0], (i // cols) * (cell[1] + 18)
        if im is not None:
            s.paste(im.resize(cell), (x, y + 18))
        label = path.rsplit("/", 1)[-1].removesuffix(".png")
        d.text((x + 4, y + 3), f"{label}  {note}", fill=(230, 230, 230))
    s.save(out)


def main(argv):
    out, empty, lineup = None, None, None
    while argv[:1] in (["--sheet"], ["--empty"], ["--lineup"]):
        if argv[0] == "--sheet":
            out = argv[1]
        elif argv[0] == "--empty":
            empty = argv[1]
        else:
            lineup = [float(v) for v in argv[1].split(",")]
        argv = argv[2:]
    frames, bad = [], 0
    for path in argv:
        im, note = check(path)
        blank = im is None or note.endswith("BLANK")
        if empty and im is not None and not blank:
            weather = path.rsplit("/", 1)[-1].split("_")[1]
            try:
                d = shown(im, Image.open(f"{empty}{weather}.png").convert("RGB"))
            except OSError as e:
                d, note = 0.0, f"{note}  no empty view: {e}"
            note = f"{note}  shown {d:.1f}"
            if d < SHOWN:
                note += "  NOTHING"
                blank = True
            if lineup and not blank:
                name = path.rsplit("/", 1)[-1].removesuffix(".png")
                kinds = name.split("_", 2)[2].split("-")
                empty_im = Image.open(f"{empty}{weather}.png").convert("RGB")
                cols = columns(im, empty_im, len(kinds), *lineup)
                note += "  " + " ".join(f"{k} {c}" for k, c in zip(kinds, cols))
                wrecked = name.startswith("wrecked_")
                need = lambda k: SMALL_WRECK.get(k, PER_MACHINE) if wrecked else PER_MACHINE
                missing = [k for k, c in zip(kinds, cols) if c < need(k)]
                if missing:
                    note += "  MISSING " + ",".join(missing)
                    blank = True
        bad += blank
        print(f"{'FAIL ' if blank else 'ok   '}  {path}  {note}")
        frames.append((path, im, note))
    if out and frames:
        sheet(frames, out)
        print(f"sheet: {out}")
    print(f"{len(frames) - bad}/{len(frames)} frames have something in them")
    return 1 if bad or not frames else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
