#!/usr/bin/env python3
"""Print each rescaled item's model bounds (`_kit.SCALE`), the `dims` its
row in assets/data/items.ron should have; `--write` puts them there.

    python3 art/items/dims.py            # print
    python3 art/items/dims.py --write    # and update items.ron

Runs without Blender: it reads the .glb's vertices itself.
"""

import ast
import json
import os
import re
import struct
import sys

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..")


def scaled_ids():
    """The keys of `_kit.SCALE`, read from its source (no Blender here)."""
    src = open(os.path.join(ROOT, "art", "items", "_kit.py")).read()
    tree = ast.parse(src)
    for node in tree.body:
        if isinstance(node, ast.Assign) and getattr(node.targets[0], "id", "") == "SCALE":
            return [k.value for k in node.value.keys]
    return []


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
    for item in scaled_ids():
        lo, hi = bounds(os.path.join(ROOT, "assets", "models", "items", f"{item}.glb"))
        size = tuple(round(hi[a] - lo[a], 3) for a in range(3))
        off = tuple(round((hi[a] + lo[a]) / 2, 4) for a in range(3))
        dims = f"dims: Some(({size[0]}, {size[1]}, {size[2]}))"
        print(f"{item:<16} {dims}  (centre off by {off})")
        if not write:
            continue
        m = re.search(r'id: "%s",' % re.escape(item), ron)
        if not m:
            print(f"  {item}: no row")
            continue
        end = ron.index(")", ron.index("category", m.end()))
        row = ron[m.end():end]
        if "dims:" in row:
            row2 = re.sub(r"dims: Some\(\([^)]*\)\)", dims, row, count=1)
        else:
            row2 = " " + dims + "," + row
        ron = ron[:m.end()] + row2 + ron[end:]
    if write:
        open(ron_path, "w").write(ron)


if __name__ == "__main__":
    main()
