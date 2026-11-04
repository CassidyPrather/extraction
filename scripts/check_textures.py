#!/usr/bin/env python3
"""Are the generated textures all here? Exit 1, naming what's missing.

    scripts/check_textures.py

The textures scripts/fetch_assets.py makes are gitignored. A checkout
without them, or without a set added to the manifest since, draws those
surfaces in flat fallback colours, and every capture of it is wrong. The
game makes the same check before a capture or a screenshot
(`capture::assets`). This one is for the build scripts
(scripts/build_at.sh), whose old builds predate that check but take
their textures from this checkout.
"""

import json
import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def missing(textures, manifest):
    m = json.load(open(manifest))
    names = lambda v: [k for k in (v or {}) if not k.startswith("_")]
    want = []
    for s in names(m.get("materials", {}).get("sets")):
        want += [(f"materials/{s}/{x}.ktx2", "materials") for x in ("albedo", "normal", "arm")]
    for c in names(m.get("foliage", {}).get("cards")):
        want.append((f"foliage/{c}/albedo.ktx2", "foliage"))
    if m.get("terrain", {}).get("layers"):
        want += [(f"terrain/{x}.ktx2", "terrain") for x in ("albedo", "normal", "arm")]
    for s in (m.get("sky") or {}).values():
        if isinstance(s, dict) and s.get("out"):
            want.append((s["out"], "sky"))
    return [(f, g) for f, g in want if not os.path.exists(os.path.join(textures, f))]


def main():
    if os.environ.get("EXTRACTION_ALLOW_FALLBACK"):
        return 0
    gone = missing(os.path.join(ROOT, "assets", "textures"),
                   os.path.join(ROOT, "scripts", "assets_manifest.json"))
    if not gone:
        return 0
    groups = " ".join(sorted({g for _, g in gone}))
    print(f"ERROR {len(gone)} generated texture(s) missing, so shots would show flat fallback "
          f"colours: {', '.join(f for f, _ in gone[:12])}{', ...' if len(gone) > 12 else ''}. "
          f"Make them with `python3 scripts/fetch_assets.py {groups}` "
          f"(or EXTRACTION_ALLOW_FALLBACK=1 to shoot the fallback on purpose).", file=sys.stderr)
    return 1


if __name__ == "__main__":
    sys.exit(main())
