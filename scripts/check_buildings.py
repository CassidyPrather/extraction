#!/usr/bin/env python3
"""Was every near building really there to see?

    scripts/check_buildings.py <shots dir>...

With `--check-buildings 1`, each still is shot again with each building
within 60 m in view hidden in turn (`capture::cinematic::probe_buildings`),
listed in `<still>.probes.json`. Hiding a building that's there to see
changes its patch of the screen; hiding one that draws nothing (Cinder's
blast furnace at dusk on main 782d9195, its meshes passing every culling
test) changes nothing. Prints one line a building, ERROR where under
MIN_CHANGED of its screen rectangle changed, and exits 1 if any did.
"""

import glob
import json
import os
import sys

import numpy as np
from PIL import Image, ImageFilter

# A building in view changes a good share of the box round it on screen
# (the box holds sky and ground too). Below this it isn't really there.
MIN_CHANGED = float(os.environ.get("MIN_CHANGED", "0.03"))
# How far a pixel must move (0-1, after a 1 px blur) to count.
PIXEL = 0.04


def changed_share(with_path, without_path, rect):
    a = Image.open(with_path).convert("RGB").filter(ImageFilter.GaussianBlur(1))
    b = Image.open(without_path).convert("RGB").filter(ImageFilter.GaussianBlur(1))
    w, h = a.size
    x0, y0, x1, y1 = rect
    box = (int(x0 * w), int(y0 * h), max(int(x1 * w), int(x0 * w) + 1), max(int(y1 * h), int(y0 * h) + 1))
    pa = np.asarray(a.crop(box), dtype=np.float32) / 255
    pb = np.asarray(b.crop(box), dtype=np.float32) / 255
    return float((np.abs(pa - pb).max(axis=2) > PIXEL).mean())


def main(dirs):
    bad = checked = 0
    for d in dirs:
        for probes in sorted(glob.glob(os.path.join(d, "**", "*.probes.json"), recursive=True)):
            folder = os.path.dirname(probes)
            still = os.path.join(folder, os.path.basename(probes)[: -len(".probes.json")] + ".png")
            for p in json.load(open(probes)):
                without = os.path.join(folder, p["file"])
                if not (os.path.exists(still) and os.path.exists(without)):
                    continue
                share = changed_share(still, without, p["rect"])
                checked += 1
                where = os.path.relpath(still)
                if share < MIN_CHANGED:
                    bad += 1
                    print(f"ERROR {where}: {p['prefab']} isn't there to see "
                          f"(hiding it changed {share:.1%} of its box)")
                else:
                    print(f"ok {where}: {p['prefab']} ({share:.0%} of its box)")
    print(f"buildings: {checked} checked, {bad} not there to see")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
