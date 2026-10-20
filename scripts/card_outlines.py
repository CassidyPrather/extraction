#!/usr/bin/env python3
"""Outlines for the foliage cards: for each atlas rect the cards use, the
smallest 8-sided polygon (an 8-DOP: eight fixed directions) round every
texel with any alpha, padded a little, in rect-local coordinates (s across
u0..u1, t down v0..v1). A card drawn as this polygon instead of its quad
shades a quarter fewer fragments for the same picture (see
docs/PERFORMANCE.md). Prints the Rust table for src/render/cards.rs;
`render::cards::tests` holds the table to the atlases.

    python3 scripts/card_outlines.py
"""
import math
import sys
from pathlib import Path

import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
# (name in the Rust table, atlas, rect [u0, v0, u1, v1]).
RECTS = [
    ("SPRIG_0", "spruce_branch", [0.300, 0.385, 0.656, 0.768]),
    ("SPRIG_1", "spruce_branch", [0.660, 0.446, 0.935, 0.765]),
    ("SPRIG_2", "spruce_branch", [0.183, 0.041, 0.444, 0.326]),
    ("SPRIG_3", "spruce_branch", [0.646, 0.034, 0.944, 0.375]),
    ("BIRCH", "birch_leaves", [0.21, 0.0, 0.81, 1.0]),
]
PAD = 0.02
DIRECTIONS = 8


def clip(poly, n, d):
    """Keeps the part of a convex polygon where n.p <= d."""
    out = []
    for i, a in enumerate(poly):
        b = poly[(i + 1) % len(poly)]
        da = n[0] * a[0] + n[1] * a[1] - d
        db = n[0] * b[0] + n[1] * b[1] - d
        if da <= 0:
            out.append(a)
        if (da < 0) != (db < 0) and da != db:
            t = da / (da - db)
            out.append((a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t))
    return out


def outline(atlas, rect):
    im = np.asarray(Image.open(ROOT / f"assets/textures/foliage/{atlas}/albedo.png").convert("RGBA"))
    h, w = im.shape[:2]
    x0, y0 = int(rect[0] * w), int(rect[1] * h)
    x1, y1 = int(round(rect[2] * w)), int(round(rect[3] * h))
    ys, xs = np.nonzero(im[y0:y1, x0:x1, 3] > 0)
    rw, rh = x1 - x0, y1 - y0
    # Every corner of every texel with alpha.
    cx = np.concatenate([xs, xs + 1, xs, xs + 1]) / rw
    cy = np.concatenate([ys, ys, ys + 1, ys + 1]) / rh
    poly = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)]
    for i in range(DIRECTIONS):
        a = 2 * math.pi * i / DIRECTIONS
        n = (math.cos(a), math.sin(a))
        d = float(np.max(n[0] * cx + n[1] * cy)) + PAD
        poly = clip(poly, n, d)
    out = []
    for p in poly:
        p = (min(max(p[0], 0.0), 1.0), min(max(p[1], 0.0), 1.0))
        if not out or math.dist(out[-1], p) > 1e-3:
            out.append(p)
    if len(out) > 1 and math.dist(out[0], out[-1]) < 1e-3:
        out.pop()
    area = 0.5 * abs(sum(out[i][0] * out[(i + 1) % len(out)][1]
                         - out[(i + 1) % len(out)][0] * out[i][1] for i in range(len(out))))
    return out, area


def main():
    for name, atlas, rect in RECTS:
        poly, area = outline(atlas, rect)
        pts = ", ".join(f"[{x:.3f}, {y:.3f}]" for x, y in poly)
        print(f"/// {atlas} {rect}: {area:.0%} of the rect.")
        print(f"pub const {name}: &[[f32; 2]] = &[{pts}];")


if __name__ == "__main__":
    sys.exit(main())
