#!/usr/bin/env python3
"""Where does an anomaly start? From 5 m off its edge, at eye height, can
you see the line you mustn't cross?

For each kind, a view 5 m outside one of its anomalies (`anomaly-views
... edge`), shot with the anomaly dressed and without it
(EXTRACTION_HIDE_ANOMALIES), in clear day, fog, downpour and night. Along
five lines on the ground running from the near side in across its edge,
it measures what the anomaly changes at each step (the colour difference,
CIE ΔE, between with and without: a burner's red scorch on green grass
barely moves luminance), and asks two things of each line:

- the step: what it changes just inside the edge (0.2-0.8 m in) beats
  what it changes just outside (0.2-0.8 m out) by at least `MIN_STEP`;
- the place: the change crosses halfway between the two within
  `MAX_SLIP` of the true edge.

An edge reads when most of its lines pass. A kind fails when its edge
doesn't read in any condition: an anomaly you can walk into without
seeing where it starts is an invisible wall.

    cargo build --profile fast
    scripts/anomaly_edges.py                    # Marrow Creek
    scripts/anomaly_edges.py --map=rime_fell frost night
    scripts/anomaly_edges.py --measure          # the shots already taken
    scripts/anomaly_edges.py --fresh            # shoot again

Writes docs/shots/anomalies/edges[_<map>].png (a strip a kind) and .json.
"""
import concurrent.futures
import json
import math
import os
import subprocess
import sys

import numpy as np
from PIL import Image, ImageDraw

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
MAP = next(
    (a.split("=", 1)[1] for a in sys.argv if a.startswith("--map=")),
    "marrow_creek",
)
SUFFIX = "" if MAP == "marrow_creek" else f"_{MAP}"
OUT = os.path.join(ROOT, "docs", "shots", "anomalies", f"edges{SUFFIX}")
RAW = os.path.join(ROOT, "target", "edges", MAP)
BIN = os.path.join(ROOT, "target", "fast", "extraction")
W, H = 1600, 900
# The player-view camera's vertical field of view (`render::camera_bundle`).
FOV = math.radians(75.0)
# Metres off the edge the eye stands.
OFF = 5.0
CONDITIONS = {
    "clear": ["--weather", "clearing", "--hour", "day"],
    "fog": ["--weather", "fog"],
    "downpour": ["--weather", "downpour"],
    "night": ["--weather", "drizzle", "--hour", "night", "--moon", "new"],
}
# The edge's step, in ΔE (CIE76; about 2.3 is just noticeable): plainly
# different, side by side.
MIN_STEP = 8.0
# How far from the true edge the change may cross halfway, metres.
MAX_SLIP = 0.35
# Lines of five that must pass for the edge to read.
MIN_LINES = 3


def views() -> list:
    subprocess.run(
        ["cargo", "build", "-q", "--profile", "fast", "-p", "extraction_core",
         "--bin", "anomaly-views"],
        cwd=ROOT, check=True,
    )
    out = subprocess.run(
        [os.path.join(ROOT, "target", "fast", "anomaly-views"), MAP, "1", "0", str(OFF), "edge"],
        cwd=ROOT, check=True, capture_output=True, text=True,
    ).stdout
    return [json.loads(l) for l in out.splitlines()]


def shot(v: dict, cond: str, hide: bool) -> str:
    os.makedirs(RAW, exist_ok=True)
    name = v["kind"].lower()
    out = os.path.join(RAW, f"{name}_{cond}_{'off' if hide else 'on'}.png")
    if os.path.exists(out) and "--fresh" not in sys.argv:
        return out
    data = os.path.join(RAW, f"data_{name}_{cond}_{int(hide)}")
    env = dict(os.environ, WGPU_BACKEND="vulkan")
    if hide:
        env["EXTRACTION_HIDE_ANOMALIES"] = "1"
    cmd = [
        "timeout", "900", "xvfb-run", "-a", "-s", f"-screen 0 {W}x{H}x24",
        BIN, "--map", MAP, "--data-dir", data, "--no-robots",
        "--new-game", *CONDITIONS[cond], "--screenshot", out,
        "--frames", "150", "--camera", v["camera"],
    ]
    subprocess.run(cmd, env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    return out


def project(v: dict, p) -> tuple:
    """A world point to pixels in the shot, or None behind the eye."""
    yaw, pitch = math.radians(v["yaw"]), math.radians(v["pitch"])
    d = np.array(p, dtype=np.float64) - np.array(v["eye"], dtype=np.float64)
    # Bevy's YXZ Euler: R = Ry(-yaw) Rx(pitch); view = R^T d.
    cy, sy = math.cos(-yaw), math.sin(-yaw)
    cp, sp = math.cos(pitch), math.sin(pitch)
    # Undo the yaw (about Y), then the pitch (about X).
    x1 = cy * d[0] - sy * d[2]
    z1 = sy * d[0] + cy * d[2]
    y1 = d[1]
    y2 = cp * y1 + sp * z1
    z2 = -sp * y1 + cp * z1
    if z2 >= -0.05:
        return None
    f = (H / 2) / math.tan(FOV / 2)
    return (W / 2 + f * x1 / -z2, H / 2 - f * y2 / -z2)


def lab(path: str) -> np.ndarray:
    """A shot in CIE L*a*b* (D65)."""
    c = np.asarray(Image.open(path).convert("RGB"), dtype=np.float32) / 255.0
    c = np.where(c <= 0.04045, c / 12.92, ((c + 0.055) / 1.055) ** 2.4)
    xyz = c @ np.array(
        [[0.4124, 0.2126, 0.0193], [0.3576, 0.7152, 0.1192], [0.1805, 0.0722, 0.9505]],
        dtype=np.float32,
    )
    xyz /= np.array([0.95047, 1.0, 1.08883], dtype=np.float32)
    f = np.where(xyz > 0.008856, np.cbrt(xyz), 7.787 * xyz + 16 / 116)
    return np.stack(
        [116 * f[..., 1] - 16, 500 * (f[..., 0] - f[..., 1]), 200 * (f[..., 1] - f[..., 2])],
        axis=-1,
    )


def measure(v: dict, cond: str) -> dict:
    name = v["kind"].lower()
    diff = np.sqrt(
        (
            (lab(os.path.join(RAW, f"{name}_{cond}_on.png"))
             - lab(os.path.join(RAW, f"{name}_{cond}_off.png"))) ** 2
        ).sum(-1)
    )
    lines = []
    for probe in v["probes"]:
        prof = []
        for d, p in probe:
            uv = project(v, p)
            if uv is None or not (2 <= uv[0] < W - 2 and 2 <= uv[1] < H - 2):
                continue
            x, y = int(uv[0]), int(uv[1])
            prof.append((d, float(diff[y - 2:y + 3, x - 2:x + 3].mean())))
        inside = [c for d, c in prof if -0.8 <= d <= -0.2]
        outside = [c for d, c in prof if 0.2 <= d <= 0.8]
        if not inside or not outside:
            lines.append({"step": 0.0, "slip": None, "ok": False})
            continue
        i, o = float(np.mean(inside)), float(np.mean(outside))
        half = (i + o) / 2
        # Where, walking out from the inside, the change first falls below
        # halfway.
        cross = next(
            (d for d, c in sorted(prof) if d >= -1.2 and c < half and d > -0.8),
            None,
        )
        slip = None if cross is None else abs(cross)
        ok = (i - o) >= MIN_STEP and slip is not None and slip <= MAX_SLIP
        lines.append({"step": round(i - o, 1), "slip": slip, "ok": ok})
    passing = sum(l["ok"] for l in lines)
    return {
        "kind": name,
        "condition": cond,
        "lines": lines,
        "passing": passing,
        "ok": passing >= MIN_LINES,
    }


def sheet(vs: list, results: list) -> None:
    conds = list(CONDITIONS)
    tiles = Image.new("RGB", (400 * len(conds), 260 * len(vs)), (20, 20, 20))
    for row, v in enumerate(vs):
        name = v["kind"].lower()
        for col, cond in enumerate(conds):
            src = os.path.join(RAW, f"{name}_{cond}_on.png")
            if not os.path.exists(src):
                continue
            im = Image.open(src).convert("RGB")
            d = ImageDraw.Draw(im)
            for probe in v["probes"]:
                pts = [project(v, p) for _, p in probe]
                pts = [p for p in pts if p is not None]
                if len(pts) > 1:
                    d.line(pts, fill=(255, 0, 255), width=1)
                mid = project(v, probe[len(probe) // 2][1])
                if mid:
                    d.ellipse((mid[0] - 3, mid[1] - 3, mid[0] + 3, mid[1] + 3), outline=(255, 255, 0))
            r = next(x for x in results if x["kind"] == name and x["condition"] == cond)
            t = im.resize((400, 225))
            td = ImageDraw.Draw(t)
            td.rectangle((0, 0, 400, 16), fill=(0, 0, 0))
            td.text(
                (4, 2),
                f"{name} {cond}: {r['passing']}/5 lines {'ok' if r['ok'] else 'NO EDGE'}",
                fill=(255, 255, 255),
            )
            tiles.paste(t, (col * 400, row * 260))
    tiles.save(OUT + ".png")


def main() -> int:
    vs = views()
    words = [a for a in sys.argv[1:] if not a.startswith("--")]
    kinds = [v for v in vs if v["kind"].lower() in words] or vs
    conds = [c for c in CONDITIONS if c in words] or list(CONDITIONS)
    if "--measure" not in sys.argv:
        jobs = [(v, c, h) for v in kinds for c in conds for h in (False, True)]
        with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
            for out in pool.map(lambda j: shot(*j), jobs):
                print("shot", os.path.relpath(out, ROOT), flush=True)
    results = [
        measure(v, c)
        for v in vs
        for c in CONDITIONS
        if os.path.exists(os.path.join(RAW, f"{v['kind'].lower()}_{c}_off.png"))
        and os.path.exists(os.path.join(RAW, f"{v['kind'].lower()}_{c}_on.png"))
    ]
    if not results:
        print("FAIL no shots")
        return 2
    sheet([v for v in vs if any(r["kind"] == v["kind"].lower() for r in results)], results)
    with open(OUT + ".json", "w") as f:
        json.dump(results, f, indent=1)
    for r in results:
        steps = " ".join(
            f"{l['step']:+.0f}/" + ("-" if l["slip"] is None else f"{l['slip']:.1f}")
            for l in r["lines"]
        )
        print(f"{r['kind']:7} {r['condition']:9} {r['passing']}/5  {steps}  {'ok' if r['ok'] else 'NO EDGE'}")
    low = [r for r in results if not r["ok"]]
    print("anomaly edges OK" if not low else f"{len(low)} edges don't read")
    return 1 if low else 0


if __name__ == "__main__":
    sys.exit(main())
