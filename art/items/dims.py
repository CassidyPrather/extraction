#!/usr/bin/env python3
"""Print each item's model bounds, the `dims` its row in
assets/data/items.ron should have; `--write` puts them there.

    python3 art/items/dims.py            # print the rows that are off
    python3 art/items/dims.py --write    # and update items.ron

Every item with a model and a row, guns aside (their layouts place them):
a row is off when a side differs from its model's by more than 1 mm
(`models::tests::every_item_box_is_its_model`). The models kept off
their box's middle (`_kit.KEEPS_ORIGIN`) are left for their owners.

Runs without Blender: it reads the .glb's vertices itself.
"""

import ast
import json
import os
import re
import struct
import sys

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..")


def kit_keys(name):
    """The keys of a dict in `_kit`, read from its source (no Blender here)."""
    src = open(os.path.join(ROOT, "art", "items", "_kit.py")).read()
    tree = ast.parse(src)
    for node in tree.body:
        if isinstance(node, ast.Assign) and getattr(node.targets[0], "id", "") == name:
            return [k.value for k in node.value.keys]
    return []


def scaled_ids():
    """The keys of `_kit.SCALE`."""
    return kit_keys("SCALE")


# A side may differ from its model's by this much (m): dims are written to
# the millimetre.
TOLERANCE = 0.001


def rows(ron):
    """Every row with dims: (id, dims, category)."""
    out = []
    for m in re.finditer(r'id: "([^"]+)",\s*dims: Some\(\(([^)]*)\)\)', ron):
        c = re.search(r"category: (\w+)", ron[m.end():m.end() + 2000])
        out.append((m.group(1), [float(x) for x in m.group(2).split(",")],
                    c.group(1) if c else None))
    return out


def model_path(item, category):
    folder = "mags" if category == "Magazine" else "items"
    return os.path.join(ROOT, "assets", "models", folder, f"{item}.glb")


def fmt(v):
    """To the millimetre; to a tenth under a centimetre (a round)."""
    return f"{round(v, 4) if v < 0.01 else round(v, 3)}"


def quat_rotate(q, v):
    x, y, z, w = q
    # v + 2 w (q x v) + 2 q x (q x v)
    cx = (y * v[2] - z * v[1], z * v[0] - x * v[2], x * v[1] - y * v[0])
    cx2 = (y * cx[2] - z * cx[1], z * cx[0] - x * cx[2], x * cx[1] - y * cx[0])
    return tuple(v[i] + 2 * w * cx[i] + 2 * cx2[i] for i in range(3))


def bounds(path):
    """Bounds of every mesh node's vertices (no `_lod`), through the node
    hierarchy (translation, rotation, scale)."""
    b = open(path, "rb").read()
    n = struct.unpack("<I", b[12:16])[0]
    j = json.loads(b[20:20 + n])
    at = 20 + (n + 3) // 4 * 4
    bin_ = b[at + 8:]
    lo, hi = [1e9] * 3, [-1e9] * 3

    def walk(i, xf):
        node = j["nodes"][i]
        t = node.get("translation", [0, 0, 0])
        r = node.get("rotation", [0, 0, 0, 1])
        s = node.get("scale", [1, 1, 1])

        def here(p):
            p = (p[0] * s[0], p[1] * s[1], p[2] * s[2])
            p = quat_rotate(r, p)
            return xf((p[0] + t[0], p[1] + t[1], p[2] + t[2]))

        if "mesh" in node and "_lod" not in node.get("name", ""):
            for prim in j["meshes"][node["mesh"]]["primitives"]:
                acc = j["accessors"][prim["attributes"]["POSITION"]]
                view = j["bufferViews"][acc["bufferView"]]
                off = view.get("byteOffset", 0) + acc.get("byteOffset", 0)
                stride = view.get("byteStride", 12)
                for k in range(acc["count"]):
                    p = here(struct.unpack_from("<3f", bin_, off + k * stride))
                    for a in range(3):
                        lo[a] = min(lo[a], p[a])
                        hi[a] = max(hi[a], p[a])
        for c in node.get("children", []):
            walk(c, here)

    for r in j["scenes"][j.get("scene", 0)]["nodes"]:
        walk(r, lambda p: p)
    return lo, hi


def main():
    write = "--write" in sys.argv
    ron_path = os.path.join(ROOT, "assets", "data", "items.ron")
    ron = open(ron_path).read()
    kept = set(kit_keys("KEEPS_ORIGIN"))
    for item, have, category in rows(ron):
        if category == "Firearm" or item in kept:
            continue
        path = model_path(item, category)
        if not os.path.exists(path):
            continue
        lo, hi = bounds(path)
        size = [hi[a] - lo[a] for a in range(3)]
        if all(abs(size[a] - have[a]) <= TOLERANCE for a in range(3)):
            continue
        off = tuple(round((hi[a] + lo[a]) / 2, 4) for a in range(3))
        dims = f"dims: Some(({fmt(size[0])}, {fmt(size[1])}, {fmt(size[2])}))"
        print(f"{item:<22} {dims}  was {tuple(have)}  (centre off by {off})")
        if not write:
            continue
        m = re.search(r'id: "%s",' % re.escape(item), ron)
        end = ron.index(")", ron.index("category", m.end()))
        row = re.sub(r"dims: Some\(\([^)]*\)\)", dims, ron[m.end():end], count=1)
        ron = ron[:m.end()] + row + ron[end:]
    if write:
        open(ron_path, "w").write(ron)


if __name__ == "__main__":
    main()
