#!/usr/bin/env python3
"""Lays a gun's orthographic bench shot over a side photograph of the real
gun, both at the same scale, so what's the wrong size shows.

    scripts/gun_overlay.py BENCH.png PHOTO OUT.png --mm-per-px M \
        --photo-muzzle X,Y --photo-butt X,Y --length MM [--crop X0,Y0,X1,Y1]

BENCH.png comes from the bench with `--view level_left --ortho H`: muzzle
on the left, 1 px = H m / 800 (the window is 800 px high). The photo's
muzzle tip and butt (pixels, the gun's left side) scale it to the real
length; both are registered on the muzzle tip and the bore line through
it. The model is drawn in cyan where it is, the photo in grey beneath; a
model that runs past the real gun shows cyan on white.
"""
import sys

from PIL import Image, ImageChops, ImageOps


def arg(name, default=None):
    if name in sys.argv:
        return sys.argv[sys.argv.index(name) + 1]
    return default


def pair(s):
    return tuple(float(v) for v in s.split(","))


def main():
    bench, photo, out = sys.argv[1:4]
    mm_px = float(arg("--mm-per-px"))
    pm, pb = pair(arg("--photo-muzzle")), pair(arg("--photo-butt"))
    length = float(arg("--length"))
    b = Image.open(bench).convert("RGB")
    # The bench's backdrop is flat; anything else is the gun.
    bg = b.getpixel((2, 2))
    diff = ImageChops.difference(b, Image.new("RGB", b.size, bg)).convert("L")
    mask = diff.point(lambda v: 255 if v > 12 else 0)
    box = mask.getbbox()
    # The model's muzzle: the leftmost column; its bore line, the middle of
    # what's drawn in the first few millimetres of it.
    mx = box[0]
    col = [y for y in range(b.height) for x in range(mx, mx + int(4 / mm_px))
           if mask.getpixel((x, y))]
    my = (min(col) + max(col)) / 2
    # Photo to bench pixels.
    k = length / ((pb[0] - pm[0]) * mm_px)
    p = Image.open(photo).convert("L")
    p = p.resize((round(p.width * k), round(p.height * k)), Image.LANCZOS)
    ox, oy = mx - pm[0] * k, my - pm[1] * k
    if arg("--align") == "butt":
        # On the butt instead (the action compares, the barrel's error
        # shows all at the muzzle).
        ox = box[2] - pb[0] * k
    canvas = Image.new("L", b.size, 255)
    canvas.paste(p, (round(ox), round(oy)))
    base = ImageOps.autocontrast(canvas).convert("RGB")
    cyan = Image.new("RGB", b.size, (0, 190, 230))
    edge = mask.filter(__import__("PIL.ImageFilter", fromlist=["x"]).FIND_EDGES)
    over = Image.composite(Image.blend(base, cyan, 0.45), base, mask)
    over = Image.composite(Image.new("RGB", b.size, (0, 90, 160)), over, edge)
    c = arg("--crop")
    if c:
        over = over.crop(tuple(int(v) for v in c.split(",")))
    over.save(out)
    print(f"model muzzle at ({mx}, {my:.0f}); photo scaled x{k:.3f}")


if __name__ == "__main__":
    main()
