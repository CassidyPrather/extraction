"""Every stencil glyph told apart from every other, at the size a prop's
lettering is read (docs/craft/PROP_READING.md: the shared font's В was
the 8's strokes, Н read as K):

    blender -b --python art/check_glyphs.py

Each glyph is drawn as the props draw it (`art/props/_kit.py`'s
`stencil_strokes`), rasterised 24 px high (a 9 cm letter at 2 m in the
headset), and every pair's masks compared: two glyphs that overlap by
more than TWIN of their union are twins. Pairs drawn the same on
purpose (a Cyrillic letter on its Latin lookalike; 0 and O) are listed
in SAME. Exits 1 on a twin.
"""

import math
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "props"))
import _kit  # noqa: E402

TWIN = 0.8
PX = 24
# Drawn alike on purpose: what reads the same in print. И and Й differ
# by Й's breve alone, as in print: a diacritic is too little of a letter
# for masks to weigh, so the breve is drawn a full cup (`Й`).
SAME = {frozenset("ИЙ")}


def mask(ch):
    h = 1.0
    strokes = _kit.stencil_strokes(ch, h, align="left")
    # Up to a breve's height over the letter (Й).
    w, ht = int(PX * 0.9) + 2, int(PX * 1.3) + 2
    s = PX / h
    on = set()
    for x in range(w):
        for y in range(ht):
            px, py = (x - 1) / s, (y - 1) / s - h / 2
            for cx, cy, L, sw, a in strokes:
                dx, dy = px - cx, py - cy
                u = dx * math.cos(a) + dy * math.sin(a)
                v = -dx * math.sin(a) + dy * math.cos(a)
                if abs(u) <= L / 2 and abs(v) <= sw / 2:
                    on.add((x, y))
                    break
    return on


glyphs, lookalike = _kit._glyphs()
chars = sorted(set(glyphs) | set(lookalike))
masks = {c: mask(c) for c in chars}
twins = []
for i, a in enumerate(chars):
    for b in chars[i + 1:]:
        if lookalike.get(a) == b or lookalike.get(b) == a or frozenset((a, b)) in SAME:
            continue
        if lookalike.get(a, a) == lookalike.get(b, b):
            continue
        ma, mb = masks[a], masks[b]
        if not ma or not mb:
            continue
        iou = len(ma & mb) / len(ma | mb)
        if iou > TWIN:
            twins.append((iou, a, b))
near = sorted(((len(masks[a] & masks[b]) / max(len(masks[a] | masks[b]), 1), a, b) for a, b in (("H", "K"), ("П", "Л"), ("Ш", "Ц"), ("3", "З"), ("0", "О")) if a in masks and b in masks), reverse=True)
for iou, a, b in near:
    print(f"asked: {a} {b} ({iou:.2f})")
for iou, a, b in sorted(twins, reverse=True):
    print(f"twins: {a} {b} ({iou:.2f})")
print(f"{len(chars)} glyphs, {len(twins)} twin pairs")
sys.exit(1 if twins else 0)
