#!/usr/bin/env python3
"""Rebuild every procedural model: runs each script under art/guns,
art/mags, art/robots, art/vehicles and art/items in a fresh headless
Blender.

    python3 art/build.py              # everything
    python3 art/build.py ak74 drone   # only scripts whose name matches

Needs Blender 4.2+ on PATH (or BLENDER=/path/to/blender). Each script
writes its own .glb (some write several) under assets/models/.
"""

import os
import subprocess
import sys
import time

ART = os.path.dirname(os.path.abspath(__file__))
BLENDER = os.environ.get("BLENDER", "blender")


def scripts():
    for group in ("guns", "mags", "robots", "vehicles", "items"):
        d = os.path.join(ART, group)
        if not os.path.isdir(d):
            continue
        for name in sorted(os.listdir(d)):
            if name.endswith(".py") and not name.startswith("_"):
                yield os.path.join(d, name)


def main():
    want = sys.argv[1:]
    failed = []
    for path in scripts():
        name = os.path.splitext(os.path.basename(path))[0]
        if want and not any(w in name for w in want):
            continue
        t = time.time()
        res = subprocess.run([BLENDER, "-b", "--factory-startup", "--python-exit-code", "1",
                              "--python", path], capture_output=True, text=True)
        out = res.stdout + res.stderr
        lines = [ln for ln in out.splitlines() if "tris" in ln or ln.startswith("wrote")]
        ok = res.returncode == 0 and any(ln.startswith("wrote") for ln in lines)
        print(f"{'ok ' if ok else 'FAIL'} {name} ({time.time() - t:.1f} s)")
        for ln in lines:
            print("    " + ln.strip())
        if not ok:
            print(out[-3000:])
            failed.append(name)
    if failed:
        sys.exit(f"failed: {', '.join(failed)}")


if __name__ == "__main__":
    main()
