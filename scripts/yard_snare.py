#!/usr/bin/env python3
"""The training yard's snare, charged and spent (what a thrown part does to
it), by day and at night by the player's torch: a sheet to see it by
without a headset.

    cargo build --profile fast
    scripts/yard_snare.py

Spent is held with EXTRACTION_SPENT_SNARES (every snare kept at no
charge). Writes docs/shots/anomalies/yard_snare.png.
"""
import concurrent.futures
import os
import subprocess

from PIL import Image, ImageDraw

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
BIN = os.path.join(ROOT, "target", "fast", "extraction")
RAW = os.path.join(ROOT, "target", "yard_snare")
OUT = os.path.join(ROOT, "docs", "shots", "anomalies", "yard_snare.png")
W, H = 1600, 900
# `onboarding::SNARE_AT` (9, -31.5), radius 1.8: 4 m off its edge, looking
# at it (-z), a little down.
CAMERA = "9,0,-25.7,0,-14"
HOURS = {
    "day": ["--weather", "clearing", "--hour", "day"],
    "night": ["--weather", "clearing", "--hour", "night", "--moon", "new", "--torch"],
}


def shot(job):
    hour, spent = job
    os.makedirs(RAW, exist_ok=True)
    out = os.path.join(RAW, f"{hour}_{'spent' if spent else 'charged'}.png")
    env = dict(os.environ, WGPU_BACKEND="vulkan")
    if spent:
        env["EXTRACTION_SPENT_SNARES"] = "1"
    data = out[:-4] + "_data"
    subprocess.run(
        ["timeout", "900", "xvfb-run", "-a", "-s", f"-screen 0 {W}x{H}x24", BIN,
         "--map", "training_yard", "--data-dir", data, "--no-robots", "--new-game",
         *HOURS[hour], "--screenshot", out, "--frames", "150", "--camera", CAMERA],
        env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    )
    subprocess.run(["rm", "-rf", data])
    return out


def main() -> int:
    jobs = [(h, s) for h in HOURS for s in (False, True)]
    with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
        shots = list(pool.map(shot, jobs))
    missing = [s for s in shots if not os.path.exists(s)]
    if missing:
        print("FAIL shots missing:", missing)
        return 1
    tw, th = 800, 450
    sheet = Image.new("RGB", (tw * 2 + 30, (th + 26) * len(HOURS) + 10), (18, 18, 18))
    d = ImageDraw.Draw(sheet)
    for (hour, spent), path in zip(jobs, shots):
        row, col = list(HOURS).index(hour), int(spent)
        x, y = 10 + col * (tw + 10), 10 + row * (th + 26)
        sheet.paste(Image.open(path).convert("RGB").resize((tw, th)), (x, y + 22))
        what = "spent: a part set it off" if spent else "charged"
        d.text((x, y + 4), f"Training yard snare, {hour}: {what}", fill=(230, 230, 230))
    sheet.save(OUT)
    print("sheet", os.path.relpath(OUT, ROOT))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
