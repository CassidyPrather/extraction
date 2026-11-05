#!/usr/bin/env python3
"""Fail on a blank frame; lay the frames out on a contact sheet.

    scripts/frame_check.py [--sheet out.png] [--empty PREFIX] frame.png ...

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
Exits 1 if any frame is blank, shows nothing, or is missing.
"""

import sys
from PIL import Image, ImageDraw, ImageStat

SPREAD = 12
STD = 4.0
SHOWN = 0.1


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
    out, empty = None, None
    while argv[:1] in (["--sheet"], ["--empty"]):
        if argv[0] == "--sheet":
            out = argv[1]
        else:
            empty = argv[1]
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
