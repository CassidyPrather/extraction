#!/usr/bin/env python3
"""Draw the sections art/sections.py cut (OUT.png.json) into OUT.png:
each station a panel, a 10 mm grid, each object's outline in its own
shade, the panel's name above it.

    scripts/draw_sections.py OUT.png
"""
import json
import sys

from PIL import Image, ImageDraw

SHADES = [(20, 20, 20), (180, 60, 40), (40, 90, 170), (60, 140, 60), (150, 110, 30),
          (120, 50, 140), (90, 90, 90)]


def main():
    out = sys.argv[1]
    d = json.load(open(out + ".json"))
    w, h, px = d["w"], d["h"], d["px"]
    pad = 30
    sheet = Image.new("RGB", ((w + pad) * len(d["cuts"]) + pad, h + 2 * pad), "white")
    draw = ImageDraw.Draw(sheet)
    for k, cut in enumerate(d["cuts"]):
        ox, oy = pad + k * (w + pad), pad
        # 10 mm grid.
        step = px * 0.01
        x = 0.0
        while x <= w:
            draw.line([(ox + x, oy), (ox + x, oy + h)], fill=(225, 225, 225))
            x += step
        y = 0.0
        while y <= h:
            draw.line([(ox, oy + y), (ox + w, oy + y)], fill=(225, 225, 225))
            y += step
        draw.rectangle([ox, oy, ox + w, oy + h], outline=(160, 160, 160))
        for i, a, b in cut:
            p = (ox + (a[0] - d["x0"]) * px, oy + (d["y1"] - a[1]) * px)
            q = (ox + (b[0] - d["x0"]) * px, oy + (d["y1"] - b[1]) * px)
            draw.line([p, q], fill=SHADES[i % len(SHADES)], width=2)
        draw.text((ox, 8), d["names"][k] + "  (grid 10 mm)", fill=(0, 0, 0))
    sheet.save(out)


if __name__ == "__main__":
    main()
