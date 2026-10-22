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
    scripts/anomaly_legibility.py --noise    # and a second shot without, for the noise floor
    scripts/anomaly_legibility.py --map=rime_fell  # another valley

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
# The valley: `--map NAME` (Marrow Creek by default). Its sheet and numbers
# are legibility.* for Marrow, legibility_<map>.* for the others.
MAP = next(
    (a.split("=", 1)[1] for a in sys.argv if a.startswith("--map=")),
    "marrow_creek",
)
SUFFIX = "" if MAP == "marrow_creek" else f"_{MAP}"
OUT = os.path.join(ROOT, "docs", "shots", "anomalies", f"legibility{SUFFIX}")
RAW = os.path.join(ROOT, "target", "legibility", MAP)
BIN = os.path.join(ROOT, "target", "fast", "extraction")

def targets() -> list:
    """For each kind, a camera 20 m off one of its anomalies in the valley
    (seed 1, cycle 0), on the side with a clear line: no rise, trunk or other
    anomaly between, off roads, on gentle ground (`anomaly::view_of`, via
    the anomaly-views tool), aimed at a metre over its centre."""
    subprocess.run(
        ["cargo", "build", "-q", "--profile", "fast", "-p", "extraction_core",
         "--bin", "anomaly-views"],
        cwd=ROOT, check=True,
    )
    out = subprocess.run(
        [os.path.join(ROOT, "target", "fast", "anomaly-views"), MAP, "1", "0", "20"],
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
# A second shot without, for the noise floor. Shots of the same view come
# out the same to the pixel (fixed steps), so it's off unless asked for.
NOISE = "--noise" in sys.argv

# The box the anomaly fills at 20 m, in a 1600x900 shot: about 7 m across
# and 5 m up from the ground.
BOX = (560, 260, 1040, 620)
# A pixel "changed" when its luminance moved by this much (of white): a
# few just-noticeable differences at the grey of fog.
STEP = 0.04
# Pixels a degree in the shots: Bevy's 75 degrees is the vertical field
# of view, over 900 rows.
PX_PER_DEG = 900 / 75
# What each kind must change, beyond the noise, in square degrees: about a
# head at 10 m, which you'd notice. A mirror is meant to be almost nothing,
# a glint: it's held to less.
FLOOR = {"mirror": 1.0}
DEFAULT_FLOOR = 2.0


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
        BIN, "--map", MAP, "--data-dir", data, "--no-robots",
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
    changed = float((np.abs(on - off) > STEP).mean())
    noise = 0.0
    second = os.path.join(RAW, f"{name}_{weather}_off1.png")
    if os.path.exists(second):
        noise = float((np.abs(luma(second) - off) > STEP).mean())
    floor = FLOOR.get(name, DEFAULT_FLOOR)
    # The box's share, in square degrees.
    x0, y0, x1, y1 = BOX
    area = (x1 - x0) * (y1 - y0) / PX_PER_DEG**2
    seen = (changed - noise) * area
    return {
        "kind": name,
        "weather": weather,
        "changed": round(changed, 4),
        "noise": round(noise, 4),
        "over_noise": round(changed - noise, 4),
        "deg2": round(seen, 2),
        "floor": floor,
        "ok": seen >= floor,
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
            f"{r['kind']} {r['weather']}: {r['deg2']:.1f} deg2 {mark}",
            fill=(255, 255, 255),
        )
        tiles.append(t)
    # A row a kind, a column a condition.
    names = [n for n, _ in TARGETS]
    out = Image.new("RGB", (320 * len(WEATHERS), 260 * len(names)), (20, 20, 20))
    for r, t in zip(results, tiles):
        col, row = WEATHERS.index(r["weather"]), names.index(r["kind"])
        out.paste(t, (col * 320, row * 260))
    out.save(OUT + ".png")


def main() -> int:
    words = [a for a in sys.argv[1:] if not a.startswith("--")]
    kinds = [n for n, _ in TARGETS if n in words] or [n for n, _ in TARGETS]
    conds = [w for w in WEATHERS if w in words] or WEATHERS
    if "--measure" not in sys.argv:
        # A kind's shots kept from a run with another camera for it are no
        # use: those go; the rest stay.
        cams = os.path.join(RAW, "cameras.json")
        os.makedirs(RAW, exist_ok=True)
        if os.path.exists(cams):
            old = dict(map(tuple, json.load(open(cams))))
            for name, cam in TARGETS:
                if old.get(name) != cam:
                    for f in os.listdir(RAW):
                        if f.startswith(f"{name}_") and f.endswith(".png"):
                            os.remove(os.path.join(RAW, f))
        with open(cams, "w") as f:
            json.dump(TARGETS, f)
        jobs = [
            (name, cam, w, hide, n)
            for name, cam in TARGETS
            if name in kinds
            for w in conds
            for hide, n in [(False, 0), (True, 0)] + ([(True, 1)] if NOISE else [])
        ]
        with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
            for out in pool.map(lambda j: shot(*j), jobs):
                print("shot", os.path.relpath(out, ROOT), flush=True)
    missing = [
        f"{name} {w}"
        for name, _ in TARGETS
        if name in kinds
        for w in conds
        for k in ("on0", "off0")
        if not os.path.exists(os.path.join(RAW, f"{name}_{w}_{k}.png"))
    ]
    if missing:
        print("FAIL shots missing (the render failed?):", sorted(set(missing)))
        return 2
    results = [
        measure(name, w)
        for name, _ in TARGETS
        for w in WEATHERS
        if os.path.exists(os.path.join(RAW, f"{name}_{w}_off0.png"))
    ]
    sheet(results)
    with open(OUT + ".json", "w") as f:
        json.dump(results, f, indent=1)
    for r in results:
        print(
            f"{r['kind']:7} {r['weather']:9} changed {r['changed']:6.1%}"
            f"  noise {r['noise']:6.1%}  seen {r['deg2']:5.1f} deg2"
            f"  floor {r['floor']:.1f}  {'ok' if r['ok'] else 'LOW'}"
        )
    low = [r for r in results if not r["ok"]]
    print("anomaly legibility OK" if not low else f"{len(low)} under their floor")
    return 1 if low else 0


if __name__ == "__main__":
    sys.exit(main())
