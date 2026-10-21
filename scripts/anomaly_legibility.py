#!/usr/bin/env python3
"""Can you tell an anomaly is there from 20 m, in fog and in rain?

For each kind, one view at 20 m on Marrow Creek (the Surge's cycle-0
layout, seed 1, and the standing ones), shot three times per weather: with
the anomalies dressed, and twice without (EXTRACTION_HIDE_ANOMALIES; the
anomalies still work, they just aren't drawn). What the anomaly adds is
how much of the box round it changes between "with" and "without", over
what changes between the two "without" shots (rain, swaying grass: the
noise floor).

    cargo build --profile fast
    scripts/anomaly_legibility.py            # shoots, then measures
    scripts/anomaly_legibility.py --measure  # measures the shots already taken
    scripts/anomaly_legibility.py hush night # shoots just these (kinds, conditions)
    scripts/anomaly_legibility.py --fresh    # shoots again what's already shot

Writes a contact sheet, docs/shots/anomalies/legibility.png, and the
numbers, legibility.json beside it (the shots stay in target/legibility). Exits non-zero when a kind falls under its
floor.
"""
import concurrent.futures
import json
import os
import subprocess
import sys

import numpy as np
from PIL import Image, ImageDraw

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
OUT = os.path.join(ROOT, "docs", "shots", "anomalies", "legibility")
RAW = os.path.join(ROOT, "target", "legibility")
BIN = os.path.join(ROOT, "target", "fast", "extraction")

def targets() -> list:
    """For each kind, a camera 20 m off one of its anomalies on Marrow (seed
    1, cycle 0), on the side with a clear line: no rise, trunk or other
    anomaly between, off roads, on gentle ground (`anomaly::view_of`, via
    the anomaly-views tool), aimed at a metre over its centre."""
    subprocess.run(
        ["cargo", "build", "-q", "--profile", "fast", "-p", "extraction_core",
         "--bin", "anomaly-views"],
        cwd=ROOT, check=True,
    )
    out = subprocess.run(
        [os.path.join(ROOT, "target", "fast", "anomaly-views"), "marrow_creek", "1", "0", "20"],
        cwd=ROOT, check=True, capture_output=True, text=True,
    ).stdout
    return [
        (v["kind"].lower(), v["camera"])
        for v in map(json.loads, out.splitlines())
    ]


TARGETS = targets()
# Label: launch flags. Night is drizzle under a new moon, the darkest.
CONDITIONS = {
    "fog": ["--weather", "fog"],
    "downpour": ["--weather", "downpour"],
    "night": ["--weather", "drizzle", "--hour", "night", "--moon", "new"],
}
WEATHERS = list(CONDITIONS)

# The box the anomaly fills at 20 m, in a 1600x900 shot: about 7 m across
# and 5 m up from the ground.
BOX = (560, 260, 1040, 620)
# A pixel "changed" when its luminance moved by this much (of white): a
# few just-noticeable differences at the grey of fog.
STEP = 0.04
# What each kind must change, beyond the noise, as a share of the box. A
# mirror is meant to be almost nothing, a glint: it's held to less.
FLOOR = {"mirror": 0.01}
DEFAULT_FLOOR = 0.03


def shot(name: str, cam: str, weather: str, hide: bool, n: int) -> str:
    os.makedirs(RAW, exist_ok=True)
    out = os.path.join(RAW, f"{name}_{weather}_{'off' if hide else 'on'}{n}.png")
    if os.path.exists(out) and "--fresh" not in sys.argv:
        return out
    data = os.path.join(RAW, f"data_{name}_{weather}_{int(hide)}{n}")
    env = dict(os.environ, WGPU_BACKEND="vulkan")
    if hide:
        env["EXTRACTION_HIDE_ANOMALIES"] = "1"
    cmd = [
        "timeout", "900", "xvfb-run", "-a", "-s", "-screen 0 1600x900x24",
        BIN, "--map", "marrow_creek", "--data-dir", data, "--no-robots",
        "--new-game", *CONDITIONS[weather], "--screenshot", out,
        "--frames", "150", "--camera", cam,
    ]
    subprocess.run(cmd, env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    return out


def luma(path: str) -> np.ndarray:
    a = np.asarray(Image.open(path).convert("RGB"), dtype=np.float32) / 255.0
    x0, y0, x1, y1 = BOX
    a = a[y0:y1, x0:x1]
    return a @ np.array([0.2126, 0.7152, 0.0722], dtype=np.float32)


def measure(name: str, weather: str) -> dict:
    on = luma(os.path.join(RAW, f"{name}_{weather}_on0.png"))
    off = luma(os.path.join(RAW, f"{name}_{weather}_off0.png"))
    off2 = luma(os.path.join(RAW, f"{name}_{weather}_off1.png"))
    changed = float((np.abs(on - off) > STEP).mean())
    noise = float((np.abs(off2 - off) > STEP).mean())
    floor = FLOOR.get(name, DEFAULT_FLOOR)
    return {
        "kind": name,
        "weather": weather,
        "changed": round(changed, 4),
        "noise": round(noise, 4),
        "over_noise": round(changed - noise, 4),
        "floor": floor,
        "ok": changed - noise >= floor,
    }


def sheet(results: list) -> None:
    tiles = []
    for r in results:
        src = os.path.join(RAW, f"{r['kind']}_{r['weather']}_on0.png")
        im = Image.open(src).convert("RGB")
        x0, y0, x1, y1 = BOX
        pad = 80
        t = im.crop((x0 - pad, y0 - pad, x1 + pad, y1 + pad)).resize((320, 260))
        d = ImageDraw.Draw(t)
        mark = "ok" if r["ok"] else "LOW"
        d.rectangle((0, 0, 320, 18), fill=(0, 0, 0))
        d.text(
            (4, 3),
            f"{r['kind']} {r['weather']}: {r['over_noise']:.1%} {mark}",
            fill=(255, 255, 255),
        )
        tiles.append(t)
    # A row a kind, a column a condition.
    names = [n for n, _ in TARGETS]
    out = Image.new("RGB", (320 * len(WEATHERS), 260 * len(names)), (20, 20, 20))
    for r, t in zip(results, tiles):
        col, row = WEATHERS.index(r["weather"]), names.index(r["kind"])
        out.paste(t, (col * 320, row * 260))
    out.save(os.path.join(ROOT, "docs", "shots", "anomalies", "legibility.png"))


def main() -> int:
    words = [a for a in sys.argv[1:] if not a.startswith("--")]
    kinds = [n for n, _ in TARGETS if n in words] or [n for n, _ in TARGETS]
    conds = [w for w in WEATHERS if w in words] or WEATHERS
    if "--measure" not in sys.argv:
        # Shots kept from a run with other cameras are no use: start over.
        cams = os.path.join(RAW, "cameras.json")
        if os.path.exists(cams) and json.load(open(cams)) != [list(t) for t in TARGETS]:
            import shutil
            shutil.rmtree(RAW)
        os.makedirs(RAW, exist_ok=True)
        with open(cams, "w") as f:
            json.dump(TARGETS, f)
        jobs = [
            (name, cam, w, hide, n)
            for name, cam in TARGETS
            if name in kinds
            for w in conds
            for hide, n in [(False, 0), (True, 0), (True, 1)]
        ]
        with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
            for out in pool.map(lambda j: shot(*j), jobs):
                print("shot", os.path.relpath(out, ROOT), flush=True)
    results = [
        measure(name, w)
        for name, _ in TARGETS
        for w in WEATHERS
        if os.path.exists(os.path.join(RAW, f"{name}_{w}_off1.png"))
    ]
    sheet(results)
    with open(OUT + ".json", "w") as f:
        json.dump(results, f, indent=1)
    for r in results:
        print(
            f"{r['kind']:7} {r['weather']:9} changed {r['changed']:6.1%}"
            f"  noise {r['noise']:6.1%}  over {r['over_noise']:6.1%}"
            f"  floor {r['floor']:.0%}  {'ok' if r['ok'] else 'LOW'}"
        )
    low = [r for r in results if not r["ok"]]
    print("anomaly legibility OK" if not low else f"{len(low)} under their floor")
    return 1 if low else 0


if __name__ == "__main__":
    sys.exit(main())
