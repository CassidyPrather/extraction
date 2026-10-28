#!/usr/bin/env python3
"""Was every near building really there to see?

    scripts/check_buildings.py <shots dir>...
    scripts/check_buildings.py --self-test

With `--check-buildings 1`, each still is shot again with each building
within 60 m in view hidden in turn (`capture::cinematic::probe_buildings`),
listed in `<still>.probes.json`. Hiding a building that's there to see
changes its patch of the screen; hiding one that draws nothing (Cinder's
blast furnace at dusk on main 782d9195, its meshes passing every culling
test) changes nothing. Prints one line a building, ERROR where under
MIN_CHANGED of its screen rectangle changed, and exits 1 if any did.

It judges only a building it could see. One that would fail is SKIPPED
instead, with the reason, never passed, where:
- it's a sliver at the frame's edge: its box on screen runs off an edge
  and is under SLIVER of the frame across (the camera up against Rime's
  guard booth, the gate behind it a strip down the right side);
- something nearer stands in the way: under MIN_SEEN of the points
  through its box in frame are in line of sight from the eye (`seen`,
  raycast in the game, `capture::cinematic::line_of_sight`);
- its patch is too dark or fogged to show anything: under MIN_CONTRAST
  levels between the 5th and 95th percentile of its box's brightness
  (Rime's insertion at night, black fog).
A building whose meshes were lost (the furnace) still stands where it
was placed, collider and all, in a frame with something to see, so it is
still judged and still fails: `--self-test` holds that, on the missing
furnace's own numbers.
"""

import glob
import json
import os
import sys

import numpy as np
from PIL import Image

# A building in view changes a good share of the box round it on screen
# (the box holds sky and ground too). Below this it isn't really there.
MIN_CHANGED = float(os.environ.get("MIN_CHANGED", "0.03"))
# How far a pixel must move to count, 0-1: three levels in 255. Stills
# are deterministic, so hiding a building nobody can see leaves its patch
# all but bit-identical. A dark building at night moves its pixels by only
# a few levels, so a coarser 0.04 (after a blur) once failed a hut that
# was there. Measured: the missing furnace 0.1% of its box; every building
# there at least 5.8% (Cinder's cellars at night).
PIXEL = 3 / 255
# Skips (above). A box this narrow, run off the frame's edge, is a sliver.
SLIVER = 0.15
# Measured, seed 7, five valleys at dusk and night: every building judged
# had 0.48 or more of its points in line of sight, the missing furnace 1.0.
MIN_SEEN = 0.25
# Levels, 0-255. Rime's black-fog log pile 3; the missing furnace's box 47
# at night (82 at dusk).
MIN_CONTRAST = 8


def changed_share(with_path, without_path, rect):
    a = Image.open(with_path).convert("RGB")
    b = Image.open(without_path).convert("RGB")
    w, h = a.size
    x0, y0, x1, y1 = rect
    box = (int(x0 * w), int(y0 * h), max(int(x1 * w), int(x0 * w) + 1), max(int(y1 * h), int(y0 * h) + 1))
    pa = np.asarray(a.crop(box), dtype=np.float32) / 255
    pb = np.asarray(b.crop(box), dtype=np.float32) / 255
    return float((np.abs(pa - pb).max(axis=2) > PIXEL).mean())


def luma(path, rect):
    img = Image.open(path).convert("L")
    w, h = img.size
    x0, y0, x1, y1 = rect
    box = (int(x0 * w), int(y0 * h), max(int(x1 * w), int(x0 * w) + 1), max(int(y1 * h), int(y0 * h) + 1))
    return np.asarray(img.crop(box), dtype=np.float32)


def skips(still, p):
    """Why a building couldn't be judged here; empty if it could."""
    why = []
    x0, y0, x1, y1 = p["rect"]
    edge = 0.002
    if ((x1 - x0 < SLIVER and (x0 <= edge or x1 >= 1 - edge))
            or (y1 - y0 < SLIVER and (y0 <= edge or y1 >= 1 - edge))):
        why.append(f"a sliver at the frame's edge ({x1 - x0:.0%} x {y1 - y0:.0%})")
    seen = p.get("seen")
    if seen is not None and seen < MIN_SEEN:
        why.append(f"behind something nearer ({seen:.0%} in line of sight)")
    lum = luma(still, p["rect"])
    contrast = float(np.percentile(lum, 95) - np.percentile(lum, 5))
    if contrast < MIN_CONTRAST:
        why.append(f"too dark or fogged to show (contrast {contrast:.0f} levels)")
    return why


def judge(still, without, p):
    """("ok"|"ERROR"|"SKIPPED", the share changed, why skipped)."""
    share = changed_share(still, without, p["rect"])
    if share >= MIN_CHANGED:
        return "ok", share, []
    why = skips(still, p)
    return ("SKIPPED" if why else "ERROR"), share, why


def main(dirs):
    bad = checked = skipped = 0
    for d in dirs:
        for probes in sorted(glob.glob(os.path.join(d, "**", "*.probes.json"), recursive=True)):
            folder = os.path.dirname(probes)
            still = os.path.join(folder, os.path.basename(probes)[: -len(".probes.json")] + ".png")
            for p in json.load(open(probes)):
                without = os.path.join(folder, p["file"])
                if not (os.path.exists(still) and os.path.exists(without)):
                    continue
                verdict, share, why = judge(still, without, p)
                checked += 1
                where = os.path.relpath(still)
                if verdict == "ERROR":
                    bad += 1
                    print(f"ERROR {where}: {p['prefab']} isn't there to see "
                          f"(hiding it changed {share:.1%} of its box)")
                elif verdict == "SKIPPED":
                    skipped += 1
                    print(f"SKIPPED {where}: {p['prefab']} not judged: {'; '.join(why)} "
                          f"(hiding it changed {share:.1%} of its box)")
                else:
                    print(f"ok {where}: {p['prefab']} ({share:.0%} of its box)")
    print(f"buildings: {checked} checked, {bad} not there to see, {skipped} skipped")
    return 1 if bad else 0


def self_test():
    """Each skip skips, and the missing furnace still fails."""
    import tempfile

    rng = np.random.default_rng(7)
    # A dusk valley: texture enough to show a building (the missing
    # furnace's box: 5th-95th percentile 47 levels apart at night).
    lit = (rng.normal(60, 15, (54, 96, 3))).clip(0, 255).astype(np.uint8)
    # Black fog: everything within a level or two.
    fog = (26 + rng.integers(0, 3, (54, 96, 3))).astype(np.uint8)
    cases = [
        # Cinder's blast furnace, its meshes left at the origin: the whole
        # frame its box, in line of sight, hiding it changing nothing.
        ("furnace", lit, lit, {"rect": [0, 0, 1, 1], "seen": 1.0}, "ERROR"),
        # The same, with no line-of-sight reading (an older capture).
        ("furnace_old", lit, lit, {"rect": [0, 0, 1, 1]}, "ERROR"),
        # Rime's gate: a strip down the right edge.
        ("gate", lit, lit, {"rect": [0.891, 0, 1, 1], "seen": None}, "SKIPPED"),
        # Behind a nearer building.
        ("behind", lit, lit, {"rect": [0.3, 0.3, 0.6, 0.6], "seen": 0.1}, "SKIPPED"),
        # Rime's log pile in black fog.
        ("fog", fog, fog, {"rect": [0.37, 0.41, 0.51, 0.49], "seen": 1.0}, "SKIPPED"),
        # A narrow building mid-frame (a mast) is no sliver.
        ("mast", lit, lit, {"rect": [0.49, 0.17, 0.51, 0.85], "seen": 1.0}, "ERROR"),
    ]
    there = lit.copy()
    there[10:40, 20:70] = 200
    cases.append(("there", there, lit, {"rect": [0.2, 0.2, 0.75, 0.75], "seen": 1.0}, "ok"))
    # A sliver that's there still passes: skips never hide a pass.
    cases.append(("there_sliver", there, lit, {"rect": [0.0, 0.2, 0.3, 0.3], "seen": None}, "ok"))
    wrong = []
    with tempfile.TemporaryDirectory() as d:
        for name, with_, without, p, want in cases:
            a, b = os.path.join(d, name + ".png"), os.path.join(d, name + "_without.png")
            Image.fromarray(with_).save(a)
            Image.fromarray(without).save(b)
            got, share, why = judge(a, b, p)
            if got != want:
                wrong.append(f"{name}: {got}, want {want} ({share:.1%}; {why})")
    for w in wrong:
        print("FAIL", w)
    print(f"self-test: {len(cases) - len(wrong)} of {len(cases)} right")
    return 1 if wrong else 0


if __name__ == "__main__":
    if sys.argv[1:] == ["--self-test"]:
        sys.exit(self_test())
    sys.exit(main(sys.argv[1:]))
