#!/usr/bin/env python3
"""Contact sheets for the visual lookbook (scripts/lookbook.sh).

    python3 scripts/lookbook_sheet.py target/lookbook/base/marrow_creek out.jpg
    python3 scripts/lookbook_sheet.py --compare BEFORE_DIR AFTER_DIR out.jpg

The first lays a valley's views out in a grid; the second puts each view's
before and after side by side, one pair a row, labelled.
"""

import sys
from pathlib import Path

from PIL import Image, ImageDraw

W = 600  # thumbnail width


def thumb(p: Path) -> Image.Image:
    im = Image.open(p).convert("RGB")
    return im.resize((W, round(im.height * W / im.width)), Image.LANCZOS)


def label(im: Image.Image, text: str) -> None:
    d = ImageDraw.Draw(im)
    d.rectangle([0, 0, 8 * len(text) + 10, 18], fill=(0, 0, 0))
    d.text((5, 3), text, fill=(255, 255, 255))


def sheet(dir_: Path, out: Path, cols: int = 3) -> None:
    shots = sorted(dir_.glob("*.png"))
    thumbs = [thumb(p) for p in shots]
    for t, p in zip(thumbs, shots):
        label(t, p.stem)
    h = thumbs[0].height
    rows = (len(thumbs) + cols - 1) // cols
    canvas = Image.new("RGB", (W * cols, h * rows))
    for i, t in enumerate(thumbs):
        canvas.paste(t, ((i % cols) * W, (i // cols) * h))
    canvas.save(out, quality=88)


def compare(before: Path, after: Path, out: Path) -> None:
    names = sorted(p.name for p in after.glob("*.png") if (before / p.name).exists())
    pairs = []
    for n in names:
        a, b = thumb(before / n), thumb(after / n)
        label(a, f"{Path(n).stem} before")
        label(b, f"{Path(n).stem} after")
        pairs.append((a, b))
    h = pairs[0][0].height
    canvas = Image.new("RGB", (W * 2, h * len(pairs)))
    for i, (a, b) in enumerate(pairs):
        canvas.paste(a, (0, i * h))
        canvas.paste(b, (W, i * h))
    canvas.save(out, quality=88)


if __name__ == "__main__":
    args = sys.argv[1:]
    if args and args[0] == "--compare":
        compare(Path(args[1]), Path(args[2]), Path(args[3]))
    else:
        sheet(Path(args[0]), Path(args[1]))
