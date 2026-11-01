"""Compose a line-up review sheet (docs/craft/ITEM_READING.md): the 1 m and
3 m renders from art/lineup.py, colour and grey, each cropped round the
shelf and the 3 m ones enlarged 3x by whole pixels only.

    python3 art/lineup_sheet.py DIR NAME "title" "id,id,..." OUT

reads DIR/NAME_{1,3}m_{colour,grey}.png and writes OUT.
"""
import sys
from PIL import Image, ImageDraw
S, name, title, out_path = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[5]
labels = sys.argv[4].split(",")
rows = []
for d, box, k in [(1, (150, 230, 1280, 430), 1), (3, (480, 270, 950, 340), 3)]:
    for g in ("colour", "grey"):
        im = Image.open(f"{S}/{name}_{d}m_{g}.png").convert("RGB").crop(box)
        im = im.resize((im.width * k, im.height * k), Image.NEAREST)
        rows.append((f"{d} m, {g}" + (f" (pixels x{k})" if k > 1 else ""), im))
w = max(im.width for _, im in rows)
h = sum(im.height + 18 for _, im in rows) + 40
out = Image.new("RGB", (w, h), (30, 30, 30))
d = ImageDraw.Draw(out)
d.text((8, 4), title, fill=(255, 255, 255))
d.text((8, 20), "left to right: " + ", ".join(labels), fill=(200, 200, 200))
y = 40
for t, im in rows:
    d.text((8, y), t, fill=(220, 220, 150))
    out.paste(im, (0, y + 16))
    y += im.height + 18
out.save(out_path, quality=88)
print(out.size)
