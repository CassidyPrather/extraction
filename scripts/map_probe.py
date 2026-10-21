#!/usr/bin/env python3
"""Ask a built map what it is where: ground height, slope, water.

    scripts/map_probe.py <dump-dir> [name=]x,z ...
    scripts/map_probe.py <dump-dir> --slope x0,z0,x1,z1 out.png

`<dump-dir>` is what `map-dump` writes (scripts/map-preview.sh makes it
under target/preview/<map>). The first form prints each point's height,
slope in degrees and water depth; the second draws the slope over a box
the way the lint's grids see it: dark green walkable, olive over 12
degrees, yellow over 16 (no main route), orange over 25 (not
comfortable), red over 32 (walkers stop); roads under 16 in blue. Most
of a valley's grade and choice findings are easiest read off it.
"""
import os
import sys

import numpy as np


def load(d, name):
    a = np.fromfile(os.path.join(d, name), dtype="<f4")
    n = int(round(len(a) ** 0.5))
    return a.reshape(n, n)


def main():
    if len(sys.argv) < 3:
        sys.exit(__doc__)
    d = sys.argv[1]
    h, w, s = load(d, "height.f32"), load(d, "water.f32"), load(d, "slope.f32")
    half = (h.shape[0] - 1) / 2
    if sys.argv[2] == "--slope":
        from PIL import Image

        x0, z0, x1, z1 = map(float, sys.argv[3].split(","))
        r = load(d, "roads.f32")
        img = np.zeros(s.shape + (3,), np.uint8)
        img[...] = (30, 70, 30)
        for lim, col in [(12, (150, 150, 0)), (16, (255, 200, 0)), (25, (255, 80, 0)),
                         (32, (255, 0, 0))]:
            img[s > lim] = col
        img[(r > 0) & (s <= 16)] = (120, 120, 255)
        c = img[int(z0 + half):int(z1 + half), int(x0 + half):int(x1 + half)]
        k = max(1, int(800 / max(c.shape[:2])))
        Image.fromarray(c).resize((c.shape[1] * k, c.shape[0] * k), Image.NEAREST).save(sys.argv[4])
        return
    for a in sys.argv[2:]:
        name, _, xz = a.rpartition("=")
        x, z = map(float, xz.split(","))
        i, j = int(round(x + half)), int(round(z + half))
        wet = ""
        if not np.isnan(w[j, i]) and w[j, i] > h[j, i]:
            wet = f"water {w[j, i]:.2f} (depth {w[j, i] - h[j, i]:.2f})"
        print(f"{name:14} ({x:6.0f},{z:6.0f}) h {h[j, i]:6.2f} slope {s[j, i]:4.1f} {wet}")


if __name__ == "__main__":
    main()
