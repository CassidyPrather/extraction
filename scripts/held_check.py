#!/usr/bin/env python3
"""What's in your hand, read against what's behind it.

    scripts/held_check.py LIST [--bin BINARY] [--out DIR] [--only a,b]

Each line of LIST is `name | game arguments`, with a `--gallery` putting
the thing in front of the camera (`pm_pistol,0,0.3`: the PM at 30 cm).
Each view is shot twice by the software renderer, with the gallery and
without it. Where the two differ is the thing. What's read is its own
median brightness (sRGB, of 255): a dark gun against a lit wall is a
black silhouette, its slide and grip unread, however much it stands out.
The yard at noon, where a gun reads, has the PM at 31. A view under its
floor (`--floor`, default 26, or ` | floor=N` at the line's end) fails;
the exit status is the number that failed. The Weber contrast against
the room behind (linear luminance, medians) is reported beside it. Shots
are kept in target/held/<name>{,_bare}.png.
"""

import argparse
import os
import shlex
import subprocess
import sys
import tempfile

import numpy as np
from PIL import Image

ROOT = subprocess.run(["git", "rev-parse", "--show-toplevel"], capture_output=True, text=True,
                      check=True).stdout.strip()
# Where the two shots differ by more than this (of 255, sRGB), and by more
# than MASK_SHARE of what's behind, is the thing. (The share keeps out a
# bright sky that came up with its clouds in one shot and not the other: the
# software renderer's sky loads late now and then.)
MASK_STEP = 10
MASK_SHARE = 0.25
# The least median brightness (sRGB, of 255) a thing in your hand reads at.
FLOOR = 26.0


def linear(a):
    a = a / 255.0
    return np.where(a <= 0.04045, a / 12.92, ((a + 0.055) / 1.055) ** 2.4)


def luma(png):
    rgb = np.asarray(Image.open(png).convert("RGB")).astype(float)
    return rgb, linear(rgb) @ np.array([0.2126, 0.7152, 0.0722])


def shoot(binary, args, png, size):
    env = dict(os.environ, WGPU_BACKEND="vulkan")
    while args and "=" in args[0] and not args[0].startswith("-"):
        k, _, v = args[0].partition("=")
        env[k] = v
        args = args[1:]
    data = tempfile.mkdtemp(prefix="held_")
    w, h = size.split("x")
    argv = ["timeout", "1800", "xvfb-run", "-a", "-s", f"-screen 0 {w}x{h}x24", binary,
            "--screenshot", png, "--size", size, "--new-game", "--seed", "7",
            "--data-dir", data] + args
    if "--frames" not in args:
        argv += ["--frames", "60"]
    subprocess.run(argv, env=env, cwd=ROOT, capture_output=True)
    return os.path.exists(png)


def without_gallery(args):
    if "--gallery" not in args:
        return args
    i = args.index("--gallery")
    return args[:i] + args[i + 2:]


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("list")
    ap.add_argument("--bin", default=os.path.join(ROOT, "target", "fast", "extraction"))
    ap.add_argument("--out", default=os.path.join(ROOT, "target", "held"))
    ap.add_argument("--only")
    ap.add_argument("--size", default="1280x720")
    ap.add_argument("--floor", type=float, default=FLOOR)
    ap.add_argument("--rescore", action="store_true", help="score the shots in --out again, without shooting")
    o = ap.parse_args()
    os.makedirs(o.out, exist_ok=True)
    failed = 0
    for raw in open(o.list):
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        fields = [f.strip() for f in line.split("|")]
        name, args = fields[0], shlex.split(fields[1])
        floor = o.floor
        if len(fields) > 2 and fields[2].startswith("floor="):
            floor = float(fields[2][6:])
        if o.only and name not in o.only.split(","):
            continue
        held, bare = os.path.join(o.out, name + ".png"), os.path.join(o.out, name + "_bare.png")
        shot = (os.path.exists(held) and os.path.exists(bare)) if o.rescore else (
            shoot(o.bin, args, held, o.size) and shoot(o.bin, without_gallery(args), bare, o.size))
        if not shot:
            print(f"MISSING {name}: a shot didn't come out")
            failed += 1
            continue
        (a, la), (b, lb) = luma(held), luma(bare)
        d = np.abs(a - b).max(axis=2)
        mask = (d > MASK_STEP) & (d > MASK_SHARE * b.max(axis=2))
        if mask.sum() < 200:
            print(f"MISSING {name}: the thing isn't in frame ({mask.sum()} px)")
            failed += 1
            continue
        li, lbg = float(np.median(la[mask])), float(np.median(lb[mask]))
        c = abs(li - lbg) / max(lbg, 1e-4)
        level = float(np.median(a.mean(axis=2)[mask]))
        ok = level >= floor
        failed += not ok
        print(f"{'ok  ' if ok else 'LOW '} {name}: the thing at {level:.0f} of 255 (floor {floor:.0f});"
              f" against the room behind, contrast {c:.2f}, over {int(mask.sum())} px")
    return failed


if __name__ == "__main__":
    sys.exit(main())
