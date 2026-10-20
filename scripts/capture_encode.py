#!/usr/bin/env python3
"""Turns a cinematic capture's frames into something to watch.

    scripts/capture_encode.py OUT_DIR [--fps 24] [--no-video]

For each shot in OUT_DIR/cine.json (or each sub-folder of f*.png frames):
<shot>_sheet.jpg, a contact sheet of 12 frames spread over the shot, and,
when ffmpeg is on the PATH, <shot>.mp4 (H.264, yuv420p, crf 18). With more
than one shot, reel.mp4 joins them with a short fade between.
"""
import json
import os
import shutil
import subprocess
import sys

from PIL import Image, ImageDraw


def frames_of(shot_dir):
    return sorted(
        os.path.join(shot_dir, f)
        for f in os.listdir(shot_dir)
        if f.startswith("f") and f.endswith(".png")
    )


def sheet(frames, out, cols=4, cells=12, width=1600):
    if not frames:
        return
    n = min(cells, len(frames))
    pick = [frames[round(i * (len(frames) - 1) / max(n - 1, 1))] for i in range(n)]
    first = Image.open(pick[0])
    w = (width - (cols - 1) * 4) // cols
    h = round(first.height * w / first.width)
    rows = (n + cols - 1) // cols
    img = Image.new("RGB", (cols * w + (cols - 1) * 4, rows * h + (rows - 1) * 4), (16, 16, 16))
    draw = ImageDraw.Draw(img)
    for i, f in enumerate(pick):
        cell = Image.open(f).convert("RGB").resize((w, h), Image.LANCZOS)
        x, y = (i % cols) * (w + 4), (i // cols) * (h + 4)
        img.paste(cell, (x, y))
        draw.text((x + 6, y + 4), os.path.basename(f), fill=(255, 220, 120))
    img.save(out, quality=88)


def encode(shot_dir, out, fps):
    subprocess.run(
        ["ffmpeg", "-y", "-loglevel", "error", "-framerate", str(fps),
         "-i", os.path.join(shot_dir, "f%05d.png"),
         "-c:v", "libx264", "-pix_fmt", "yuv420p", "-crf", "18",
         "-movflags", "+faststart", out],
        check=True,
    )


def reel(clips, out, fade=0.5):
    """Joins clips with a crossfade between each pair."""
    if len(clips) == 1:
        shutil.copy(clips[0], out)
        return
    durs = []
    for c in clips:
        r = subprocess.run(
            ["ffprobe", "-v", "error", "-show_entries", "format=duration",
             "-of", "default=nw=1:nk=1", c],
            capture_output=True, text=True, check=True)
        durs.append(float(r.stdout.strip()))
    args = ["ffmpeg", "-y", "-loglevel", "error"]
    for c in clips:
        args += ["-i", c]
    chain, last, offset = [], "[0:v]", 0.0
    for i in range(1, len(clips)):
        offset += durs[i - 1] - fade
        tag = f"[v{i}]"
        chain.append(f"{last}[{i}:v]xfade=transition=fade:duration={fade}:offset={offset:.3f}{tag}")
        last = tag
    args += ["-filter_complex", ";".join(chain), "-map", last,
             "-c:v", "libx264", "-pix_fmt", "yuv420p", "-crf", "18",
             "-movflags", "+faststart", out]
    subprocess.run(args, check=True)


def main(argv):
    out_dir = argv[0]
    fps = 24
    video = True
    it = iter(argv[1:])
    for a in it:
        if a == "--fps":
            fps = int(next(it))
        elif a == "--no-video":
            video = False
    manifest = os.path.join(out_dir, "cine.json")
    if os.path.exists(manifest):
        with open(manifest) as f:
            shots = [s["dir"] for s in json.load(f)]
    else:
        shots = sorted(
            d for d in os.listdir(out_dir)
            if os.path.isdir(os.path.join(out_dir, d)) and frames_of(os.path.join(out_dir, d))
        )
    has_ffmpeg = video and shutil.which("ffmpeg") is not None
    clips = []
    for s in shots:
        d = os.path.join(out_dir, s)
        frames = frames_of(d)
        if not frames:
            print(f"{s}: no frames")
            continue
        sheet(frames, os.path.join(out_dir, f"{s}_sheet.jpg"))
        line = f"{s}: {len(frames)} frames, {s}_sheet.jpg"
        # A preview's handful of stills isn't footage.
        if has_ffmpeg and len(frames) >= fps:
            clip = os.path.join(out_dir, f"{s}.mp4")
            encode(d, clip, fps)
            clips.append(clip)
            line += f", {s}.mp4"
        print(line)
    if len(clips) > 1:
        reel(clips, os.path.join(out_dir, "reel.mp4"))
        print(f"reel.mp4: {len(clips)} shots")
    elif not has_ffmpeg:
        print("no ffmpeg: frames and contact sheets only")


if __name__ == "__main__":
    main(sys.argv[1:])
