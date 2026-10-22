#!/usr/bin/env python3
"""Turntables from `--sweep-turn` frames.

    scripts/capture_turntable.py <out dir> [fps]

For each <out>/<light>/<subject>/tNNN.png sequence, writes
<out>/<light>/<subject>.mp4 (H.264, yuv420p, one turn, plays as a loop)
and <out>/<light>/<subject>_strip.jpg (six frames side by side).
Without ffmpeg you get the strips only.
"""

import os
import shutil
import subprocess
import sys

from PIL import Image


def strip(frames, path, n=6, height=240):
    pick = [frames[i * len(frames) // n] for i in range(n)]
    ims = [Image.open(f).convert("RGB") for f in pick]
    ims = [im.resize((im.width * height // im.height, height)) for im in ims]
    sheet = Image.new("RGB", (sum(im.width for im in ims), height))
    x = 0
    for im in ims:
        sheet.paste(im, (x, 0))
        x += im.width
    sheet.save(path, quality=88)


def main(argv):
    out = argv[0]
    fps = int(argv[1]) if len(argv) > 1 else 12
    ffmpeg = shutil.which("ffmpeg")
    made = 0
    for light in sorted(os.listdir(out)):
        ldir = os.path.join(out, light)
        if not os.path.isdir(ldir):
            continue
        for subject in sorted(os.listdir(ldir)):
            sdir = os.path.join(ldir, subject)
            if not os.path.isdir(sdir):
                continue
            frames = sorted(
                os.path.join(sdir, f) for f in os.listdir(sdir) if f.startswith("t") and f.endswith(".png")
            )
            if len(frames) < 2:
                continue
            strip(frames, os.path.join(ldir, f"{subject}_strip.jpg"))
            if ffmpeg:
                subprocess.run(
                    [ffmpeg, "-y", "-loglevel", "error", "-framerate", str(fps),
                     "-i", os.path.join(sdir, "t%03d.png"),
                     "-vf", "scale=trunc(iw/2)*2:trunc(ih/2)*2",
                     "-c:v", "libx264", "-pix_fmt", "yuv420p", "-crf", "20",
                     "-movflags", "+faststart",
                     os.path.join(ldir, f"{subject}.mp4")],
                    check=True,
                )
            made += 1
    print(f"{made} turntables in {out}")


if __name__ == "__main__":
    main(sys.argv[1:])
