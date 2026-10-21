#!/usr/bin/env python3
"""Cuts a trailer from a filmed raid (`--trailer`, src/capture/trailer.rs).

    scripts/capture_trailer.py OUT_DIR [--max-clip 9] [--no-video]

OUT_DIR holds chase/ and drone/ frames (each frame from whichever camera
the director cut to as it filmed), trailer.json (the clips: frames, raid
time, why) and trailer.jsonl (the raid's own telemetry). Each clip is
trimmed to --max-clip seconds, captioned with what's happening and when,
and set between a title card and an end card. Writes
OUT_DIR/trailer.mp4 when ffmpeg is there, and always OUT_DIR/edit/ (the
edited frames) and OUT_DIR/trailer_sheet.jpg.
"""
import json
import os
import shutil
import subprocess
import sys

from PIL import Image, ImageDraw, ImageFont

CAPTION = {
    "leaving": "Out of the truck",
    "walk": "On foot",
    "robot": "Company",
    "fight": "Contact",
    "anomaly": "An anomaly",
    "truck": "Calling the truck",
    "extract": "Extraction",
    "down": "Down",
}
VALLEYS = {"marrow_creek": "Marrow Creek", "kettle_pass": "Kettle Pass",
           "cinder_works": "Cinder Works", "training_yard": "Training Yard"}


def font(size):
    for f in ["/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
              "/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf"]:
        if os.path.exists(f):
            return ImageFont.truetype(f, size)
    return ImageFont.load_default()


def card(size, title, sub):
    img = Image.new("RGB", size, (8, 9, 10))
    d = ImageDraw.Draw(img)
    big, small = font(size[1] // 9), font(size[1] // 28)
    w = d.textlength(title, font=big)
    d.text(((size[0] - w) / 2, size[1] * 0.38), title, font=big, fill=(232, 226, 210))
    w = d.textlength(sub, font=small)
    d.text(((size[0] - w) / 2, size[1] * 0.58), sub, font=small, fill=(170, 160, 140))
    return img


def main(argv):
    out = argv[0]
    max_clip = 9.0
    video = "--no-video" not in argv
    if "--max-clip" in argv:
        max_clip = float(argv[argv.index("--max-clip") + 1])
    with open(os.path.join(out, "trailer.json")) as f:
        t = json.load(f)
    fps = int(t["fps"])
    edit = os.path.join(out, "edit")
    shutil.rmtree(edit, ignore_errors=True)
    os.makedirs(edit)
    frames = []
    for c in t["clips"]:
        n = min(c["last"] - c["first"], int(max_clip * fps))
        for i in range(n):
            # Whichever camera filmed it (the director cuts as it films).
            for cam in ("chase", "drone"):
                src = os.path.join(out, cam, f"f{c['first'] + i:05d}.png")
                if os.path.exists(src):
                    frames.append((src, c, c["start"] + i / fps))
                    break
    if not frames:
        print("no frames")
        return
    size = Image.open(frames[0][0]).size
    valley = VALLEYS.get(t.get("map", ""), t.get("map", ""))
    n = 0

    def put(img):
        nonlocal n
        img.save(os.path.join(edit, f"e{n:05d}.jpg"), quality=92)
        n += 1

    title = card(size, "EXTRACTION", f"{valley} - one raid, as a bot played it")
    for _ in range(int(2.5 * fps)):
        put(title)
    cap = font(size[1] // 30)
    for src, c, when in frames:
        img = Image.open(src).convert("RGB")
        d = ImageDraw.Draw(img)
        text = f"{CAPTION.get(c['reason'], c['reason'])}   {int(when // 60)}:{int(when % 60):02d}"
        d.text((size[0] * 0.04 + 2, size[1] * 0.9 + 2), text, font=cap, fill=(0, 0, 0))
        d.text((size[0] * 0.04, size[1] * 0.9), text, font=cap, fill=(240, 232, 214))
        put(img)
    end = card(size, "EXTRACTION", f"seed {t.get('seed')} - {t['raid_seconds'] / 60:.0f} minutes in the Zone")
    for _ in range(int(2.5 * fps)):
        put(end)
    # The sheet: sixteen frames through the cut.
    picks = [os.path.join(edit, f"e{round(i * (n - 1) / 15):05d}.jpg") for i in range(16)]
    cols, w = 4, 400
    h = round(size[1] * w / size[0])
    sheet = Image.new("RGB", (cols * w, 4 * h), (0, 0, 0))
    for i, p in enumerate(picks):
        sheet.paste(Image.open(p).resize((w, h)), ((i % cols) * w, (i // cols) * h))
    sheet.save(os.path.join(out, "trailer_sheet.jpg"), quality=88)
    line = f"{len(t['clips'])} clips, {n} frames ({n / fps:.0f} s), {out}/trailer_sheet.jpg"
    if video and shutil.which("ffmpeg"):
        secs = n / fps
        subprocess.run(
            ["ffmpeg", "-y", "-loglevel", "error", "-framerate", str(fps),
             "-i", os.path.join(edit, "e%05d.jpg"),
             "-vf", f"fade=in:0:{fps // 2},fade=out:st={secs - 0.6:.2f}:d=0.6",
             "-c:v", "libx264", "-pix_fmt", "yuv420p", "-crf", "19",
             "-movflags", "+faststart", os.path.join(out, "trailer.mp4")],
            check=True)
        line += f", {out}/trailer.mp4"
    print(line)


if __name__ == "__main__":
    main(sys.argv[1:])
