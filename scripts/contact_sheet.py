#!/usr/bin/env python3
"""Tiles a frame sequence into one contact sheet, numbered.

    scripts/contact_sheet.py OUT.png COLS FRAME [FRAME ...] [--crop X,Y,W,H] [--scale S]

Frames are read in the order given; --crop cuts the same box out of each
(before --scale), so a small subject fills its cell.
"""
import sys

from PIL import Image, ImageDraw


def main(argv):
    crop = None
    scale = 1.0
    rest = []
    it = iter(argv)
    for a in it:
        if a == "--crop":
            crop = tuple(int(v) for v in next(it).split(","))
        elif a == "--scale":
            scale = float(next(it))
        else:
            rest.append(a)
    out, cols, frames = rest[0], int(rest[1]), rest[2:]
    cells = []
    for f in frames:
        im = Image.open(f).convert("RGB")
        if crop:
            x, y, w, h = crop
            im = im.crop((x, y, x + w, y + h))
        if scale != 1.0:
            im = im.resize((int(im.width * scale), int(im.height * scale)), Image.LANCZOS)
        cells.append(im)
    w, h = cells[0].size
    rows = (len(cells) + cols - 1) // cols
    sheet = Image.new("RGB", (cols * w + (cols - 1) * 4, rows * h + (rows - 1) * 4), (20, 20, 20))
    draw = ImageDraw.Draw(sheet)
    for i, im in enumerate(cells):
        x, y = (i % cols) * (w + 4), (i // cols) * (h + 4)
        sheet.paste(im, (x, y))
        draw.text((x + 6, y + 4), str(i), fill=(255, 220, 120))
    sheet.save(out)


if __name__ == "__main__":
    main(sys.argv[1:])
