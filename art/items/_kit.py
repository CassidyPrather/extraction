"""Shared bits for the item scripts: each item is its own .glb under
assets/models/items/<item id>.glb, built in item space -- metres, origin at
the centre of the item's `dims` box from assets/data/items.ron (or a cube
of side cbrt(bulk / 1000) when it has none), +Y up, -Z forward (where a
light shines or a scope looks)."""

import math
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from common import RX, RY, T, export, fillet, make, report, reset  # noqa: E402

UP = RX(-math.pi / 2)       # lathe/cyl axis (Z) -> +Y
ALONG_X = RY(math.pi / 2)   # lathe/cyl axis (Z) -> +X


def cube(bulk):
    """Side of the default box for an item without dims."""
    return (bulk / 1000.0) ** (1.0 / 3.0)


def item(item_id, build):
    """Build one item in a fresh scene and write its .glb. `build` returns
    the Parts to make."""
    reset()
    parts = build()
    for p in parts:
        make(p)
    print(f"{item_id}:")
    report()
    export(f"assets/models/items/{item_id}.glb")


def wanted(item_id):
    """Scripts can be run for just some items: `-- id1 id2`."""
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    return not argv or item_id in argv


def cell_bay(p, r_tube, mat, y=0.0, bay=(0.0095, 0.0305), wall=0.003):
    """A power cell bay: two cheeks either side of an open vertical bore
    round the cell's seat at z 0.02, the body's tube left out between
    `bay` (a z range)."""
    z0, z1 = bay
    h = r_tube * 2 + 0.002
    for sx in (-1, 1):
        prof = fillet([(z0 - 0.003, y - h / 2), (z1 + 0.003, y - h / 2), (z1 + 0.003, y + h / 2),
                       (z0 - 0.003, y + h / 2)], 0.003, 2)
        a, b = sorted((sx * 0.0103, sx * (0.0103 + wall)))
        p.prism(prof, a, b, mat, axis="x", bevel=0.001, segs=1)
    # Spring contacts top and bottom, seen through the bore.
    p.box((0.016, 0.002, 0.004), (0.0, y + h / 2 - 0.001, z0 - 0.001), "Steel")
    p.box((0.016, 0.002, 0.004), (0.0, y - h / 2 + 0.001, z1 + 0.001), "Steel")
