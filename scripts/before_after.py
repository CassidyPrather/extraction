#!/usr/bin/env python3
"""Before and after, side by side: one contact sheet of fixes you can see.

    scripts/before_after.py docs/playtests/2026-10-28/shots.txt
    scripts/before_after.py LIST --before SHA --after SHA --only a,b --jobs 1

LIST names the two builds and the shots, one a line:

    before 2798e450
    after  origin/claude/serene-clarke-rcgj62
    # name | owner | what to look for | game arguments [| before's arguments]
    range_board | Onboarding | The range's board ... | --map training_yard --camera -4.2,0,49.3,315,-5

Each build is made once (scripts/build_at.sh) and each shot taken with
both, at `size` (1280x720 unless the list says), by `--screenshot` after
`--frames` frames (30 unless the arguments give them). Every run gets a
fresh `--data-dir` and `--new-game`, and `--seed 7` unless the arguments
give one. Arguments may start with NAME=value words, set in the game's
environment (`EXTRACTION_YARD_AT=5`). A fifth field, if there, is used by
the before build instead: for a shot whose staging only exists since the
fix. `--picture FILE` (a path in the repo) instead of game arguments puts
a picture taken elsewhere in that half, for a fix the rig can't stage;
say so in what to look for. `--crop x0,y0,x1,y1` (pixels) crops both
halves on the sheet, for something small in frame; the game never sees
it. Shots are kept in target/before_after/<sha>/ and taken again only
when their arguments change. `--try BINARY` shoots the list with your
own build only, into target/before_after/try/, to stage a line.

`--dolly X,Y,Z,YAW,PITCH:X,Y,Z[:SECONDS[:FPS]]` in place of `--camera`
shoots motion: the camera walks from the first point to the second (Y 0:
eyes 1.7 m over the ground, as `--camera`), looking along YAW and PITCH
all the way, at walking pace (1.4 m/s) unless SECONDS says, every frame
saved at FPS (default 10) by `--cinematic` from one launch: an LOD pop or
a flicker shows between frames, not in any one. On the sheet each half is
a strip of DOLLY_STRIP frames down the walk (`--crop` crops each frame);
the page plays the two walks side by side (`<name>_pair.mp4`, with
ffmpeg), and every frame is kept in target/before_after/<sha>/<name>/.

Out: target/before_after/<before>_<after>/sheet.jpg and index.html, a row
a shot: its name, owner and what to look for over the before and the after.
A shot that didn't come out is a grey tile with the end of its log, never
left off. `--out DIR` puts them there instead (and copies the pictures).
"""

import argparse
import concurrent.futures
import hashlib
import html
import os
import shlex
import shutil
import subprocess
import sys
import tempfile
import textwrap

from PIL import Image, ImageDraw, ImageFont

ROOT = subprocess.run(["git", "rev-parse", "--show-toplevel"], capture_output=True, text=True,
                      check=True).stdout.strip()
BASE = os.path.join(ROOT, "target", "before_after")


class Shot:
    def __init__(self, name, owner, caption, args, before_args):
        self.name, self.owner, self.caption = name, owner, caption
        self.args, self.crop = take_crop(args)
        self.before_args, before_crop = take_crop(before_args)
        self.crop = self.crop or before_crop
        self.dolly = "--dolly" in self.args


def take_crop(args):
    """(args without `--crop x0,y0,x1,y1`, the box or None): the sheet's,
    not the game's."""
    if "--crop" not in args:
        return args, None
    i = args.index("--crop")
    box = tuple(int(v) for v in args[i + 1].split(","))
    return args[:i] + args[i + 2:], box


# Frames down a dolly's walk on the sheet; walking pace (m/s) and frame
# rate when the line doesn't say.
DOLLY_STRIP = 6
WALK = 1.4
DOLLY_FPS = 10


def take_dolly(args):
    """(args without `--dolly SPEC`, the walk or None): (from x,y,z, to
    x,y,z, yaw, pitch, seconds, fps)."""
    if "--dolly" not in args:
        return args, None
    i = args.index("--dolly")
    parts = args[i + 1].split(":")
    a = [float(v) for v in parts[0].split(",")]
    b = [float(v) for v in parts[1].split(",")]
    if len(a) != 5 or len(b) != 3:
        raise ValueError("--dolly wants X,Y,Z,YAW,PITCH:X,Y,Z[:SECONDS[:FPS]]")
    dist = sum((p - q) ** 2 for p, q in zip(a[:3], b)) ** 0.5
    secs = float(parts[2]) if len(parts) > 2 and parts[2] else max(dist / WALK, 1.0)
    fps = int(parts[3]) if len(parts) > 3 and parts[3] else DOLLY_FPS
    return args[:i] + args[i + 2:], (a[:3], b, a[3], a[4], secs, fps)


def cine(walk):
    """The walk as a one-shot `.cine` (`capture::cinematic`): written key
    points, an even pace (no ease), the look held 60 m ahead."""
    import math
    a, b, yaw, pitch, secs, _ = walk
    yaw, pitch = math.radians(yaw), math.radians(pitch)
    # `--camera`'s yaw (`dev.rs`): -Z turned by -YAW about +Y.
    d = (math.sin(yaw) * math.cos(pitch), math.sin(pitch), -math.cos(yaw) * math.cos(pitch))

    def key(p):
        if p[1] == 0:
            at = f"{p[0]:.2f},~1.7,{p[2]:.2f}"
            look = f"{p[0] + d[0] * 60:.2f},~{1.7 + d[1] * 60:.2f},{p[2] + d[2] * 60:.2f}"
        else:
            at = f"{p[0]:.2f},{p[1]:.2f},{p[2]:.2f}"
            look = f"{p[0] + d[0] * 60:.2f},{p[1] + d[1] * 60:.2f},{p[2] + d[2] * 60:.2f}"
        return f"  at {at}  look {look}\n"

    return f"shot dolly dur={secs:.2f} ease=0\n" + key(a) + key(b)


def parse(path):
    """(settings, shots) from a list file; errors name the line."""
    settings = {"size": "1280x720"}
    shots, errors = [], []
    for n, raw in enumerate(open(path), 1):
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        if "|" not in line:
            key, _, value = line.partition(" ")
            if key in ("before", "after", "size"):
                settings[key] = value.strip()
            else:
                errors.append(f"{path}:{n}: not a setting or a shot: {line}")
            continue
        fields = [f.strip() for f in line.split("|")]
        if len(fields) not in (4, 5) or not fields[0] or not fields[3]:
            errors.append(f"{path}:{n}: want name | owner | what to look for | arguments "
                          f"[| before's arguments]")
            continue
        name = fields[0]
        if not name.replace("_", "").replace("-", "").isalnum():
            errors.append(f"{path}:{n}: name {name!r}: letters, digits, _ and - only")
            continue
        if any(s.name == name for s in shots):
            errors.append(f"{path}:{n}: {name} twice")
            continue
        try:
            args = shlex.split(fields[3])
            before = shlex.split(fields[4]) if len(fields) == 5 and fields[4] else args
            for a in (args, before):
                take_dolly(take_crop(a)[0])
                box = take_crop(a)[1]
                if box is not None and (len(box) != 4 or box[0] >= box[2] or box[1] >= box[3]):
                    raise ValueError("--crop wants x0,y0,x1,y1 in pixels")
        except (ValueError, IndexError) as e:
            errors.append(f"{path}:{n}: {e}")
            continue
        shots.append(Shot(name, fields[1], fields[2], args, before))
    for key in ("before", "after"):
        if key not in settings:
            errors.append(f"{path}: no `{key} <sha>` line")
    return settings, shots, errors


def sha8(ref):
    out = subprocess.run(["git", "rev-parse", "--short=8", ref], capture_output=True, text=True,
                         cwd=ROOT)
    if out.returncode != 0:
        sys.exit(f"no such commit: {ref}")
    return out.stdout.strip()


def build(sha):
    out = subprocess.run([os.path.join(ROOT, "scripts", "build_at.sh"), sha], stdout=subprocess.PIPE,
                         cwd=ROOT)
    if out.returncode != 0:
        sys.exit(f"build of {sha} failed")
    return out.stdout.decode().strip()


def command(binary, args, png, size):
    """(environment, argv) for one shot."""
    env = dict(os.environ, WGPU_BACKEND="vulkan")
    while args and "=" in args[0] and not args[0].startswith("-"):
        k, _, v = args[0].partition("=")
        env[k] = v
        args = args[1:]
    argv = [binary, "--screenshot", png, "--size", size, "--new-game"]
    if "--frames" not in args:
        argv += ["--frames", "30"]
    if "--seed" not in args:
        argv += ["--seed", "7"]
    w, h = size.split("x")
    return env, ["timeout", "1800", "xvfb-run", "-a", "-s", f"-screen 0 {w}x{h}x24"] + argv + args


def shoot(binary, sha, shot, args, size):
    """The shot's picture, or None and why not."""
    if args and args[0] == "--picture":
        # Taken elsewhere (a fix the rig can't stage): the file, as given.
        png = os.path.join(ROOT, args[1]) if len(args) > 1 else ""
        return (png, "") if os.path.exists(png) else (None, f"no such picture: {png}")
    folder = os.path.join(BASE, sha)
    os.makedirs(folder, exist_ok=True)
    png = os.path.join(folder, shot.name + ".png")
    key = hashlib.sha1(repr((args, size)).encode()).hexdigest()
    stamp = png + ".args"
    if os.path.exists(png) and os.path.exists(stamp) and open(stamp).read() == key:
        return png, ""
    for f in (png, stamp):
        if os.path.exists(f):
            os.remove(f)
    data = tempfile.mkdtemp(prefix="before_after_")
    args, walk = take_dolly(take_crop(list(args))[0])
    if walk:
        frames = os.path.join(folder, shot.name)
        shutil.rmtree(frames, ignore_errors=True)
        os.makedirs(frames)
        path = os.path.join(frames, "dolly.cine")
        with open(path, "w") as f:
            f.write(cine(walk))
        # (`--frames`: the first shot's settle at its first pose. No
        # `--screenshot` below, so no plain shot.)
        args = [a for a in args]
        if "--frames" not in args:
            args += ["--frames", "60"]
        args += ["--cinematic", path, "--capture-out", frames, "--fps", str(walk[5])]
    env, argv = command(binary, args, png, size)
    if walk:
        i = argv.index("--screenshot")
        argv = argv[:i] + argv[i + 2:]
    argv += ["--data-dir", data]
    run = subprocess.run(argv, env=env, cwd=ROOT, capture_output=True, text=True)
    shutil.rmtree(data, ignore_errors=True)
    log = (run.stdout + run.stderr).splitlines()
    with open(os.path.join(folder, shot.name + ".log"), "w") as f:
        f.write(" ".join(shlex.quote(a) for a in argv) + "\n\n" + "\n".join(log))
    if walk:
        strip(os.path.join(frames, "dolly"), png, shot.crop, walk[5])
    if not os.path.exists(png):
        why = [l for l in log if "panicked" in l or "ERROR" in l] or log[-3:] or ["no output"]
        code = {124: "timed out", 137: "killed (out of memory?)"}.get(run.returncode,
                                                                       f"exit {run.returncode}")
        return None, f"{code}: " + " / ".join(why[-3:])
    with open(stamp, "w") as f:
        f.write(key)
    return png, ""


def strip(frames, png, crop, fps):
    """A dolly's frames as one picture (DOLLY_STRIP of them down the walk,
    each cropped), and as `<name>.mp4` beside it with ffmpeg."""
    if not os.path.isdir(frames):
        return
    names = sorted(f for f in os.listdir(frames) if f.endswith(".png"))
    if not names:
        return
    n = min(DOLLY_STRIP, len(names))
    pick = [names[round(i * (len(names) - 1) / max(n - 1, 1))] for i in range(n)]
    imgs = [Image.open(os.path.join(frames, f)).convert("RGB") for f in pick]
    if crop:
        imgs = [im.crop(crop) for im in imgs]
    w, h = imgs[0].size
    out = Image.new("RGB", (w, (h + 4) * n - 4), (24, 24, 24))
    d = ImageDraw.Draw(out)
    for i, (f, im) in enumerate(zip(pick, imgs)):
        out.paste(im, (0, i * (h + 4)))
        d.text((8, i * (h + 4) + 6), f"frame {int(f[1:6])}", fill=(255, 230, 120), font=font(max(14, h // 24)))
    out.save(png)
    if shutil.which("ffmpeg"):
        subprocess.run(["ffmpeg", "-y", "-loglevel", "error", "-framerate", str(fps), "-i",
                        os.path.join(frames, "f%05d.png"), "-c:v", "libx264", "-pix_fmt", "yuv420p",
                        "-crf", "20", png[:-4] + ".mp4"])


def pair(b, a, out, name):
    """The two walks side by side, `<name>_pair.mp4` in `out`, or None."""
    vb, va = b[:-4] + ".mp4", a[:-4] + ".mp4"
    if not (shutil.which("ffmpeg") and os.path.exists(vb) and os.path.exists(va)):
        return None
    dst = os.path.join(out, f"{name}_pair.mp4")
    run = subprocess.run(["ffmpeg", "-y", "-loglevel", "error", "-i", vb, "-i", va, "-filter_complex",
                          "[0:v]pad=iw+8:ih[l];[l][1:v]hstack", "-c:v", "libx264", "-pix_fmt",
                          "yuv420p", "-crf", "20", dst])
    return dst if run.returncode == 0 else None


def font(size):
    for f in ("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
              "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf"):
        if os.path.exists(f):
            return ImageFont.truetype(f, size)
    return ImageFont.load_default()


def tile(png, why, w, h, crop=None):
    if png:
        img = Image.open(png).convert("RGB")
        if crop:
            img = img.crop(crop)
        return img.resize((w, int(img.height * w / img.width)))
    img = Image.new("RGB", (w, h), (70, 70, 70))
    d = ImageDraw.Draw(img)
    d.text((16, 16), "no picture", fill=(240, 240, 240), font=font(28))
    d.text((16, 60), "\n".join(textwrap.wrap(why, 70)[:12]), fill=(220, 220, 220), font=font(14))
    return img


def sheet(rows, title, out):
    """rows: (shot, (png, why) before, (png, why) after)."""
    W, GAP, HEAD = 960, 12, 74
    H = W * 9 // 16
    big, small = font(26), font(17)
    tiles = [(s, tile(*b, W, H, None if s.dolly else s.crop),
              tile(*a, W, H, None if s.dolly else s.crop)) for s, b, a in rows]
    height = 70 + sum(HEAD + max(b.height, a.height) + 2 * GAP for _, b, a in tiles)
    img = Image.new("RGB", (2 * W + 3 * GAP, height), (24, 24, 24))
    d = ImageDraw.Draw(img)
    d.text((GAP, 18), title, fill=(255, 255, 255), font=big)
    y = 70
    for s, b, a in tiles:
        d.text((GAP, y), f"{s.name}  ({s.owner})", fill=(255, 220, 140), font=big)
        d.text((GAP, y + 36), textwrap.shorten(s.caption, 220), fill=(225, 225, 225), font=small)
        y += HEAD
        img.paste(b, (GAP, y))
        img.paste(a, (2 * GAP + W, y))
        for x, label in ((GAP, "before"), (2 * GAP + W, "after")):
            d.rectangle((x, y, x + 90, y + 30), fill=(0, 0, 0))
            d.text((x + 8, y + 4), label, fill=(255, 255, 255), font=small)
        y += max(b.height, a.height) + 2 * GAP
    os.makedirs(out, exist_ok=True)
    img.save(os.path.join(out, "sheet.jpg"), quality=88)


def page(rows, title, out, copy):
    def pic(sha, side, png, why):
        if not png:
            return f'<div class="none">no picture: {html.escape(why)}</div>'
        src = png
        if copy:
            src = f"{side}_{os.path.basename(png)}"
            shutil.copy(png, os.path.join(out, src))
        else:
            src = os.path.relpath(png, out)
        return f'<a href="{html.escape(src)}"><img src="{html.escape(src)}" loading="lazy"></a>'

    body = []
    for s, b, a in rows:
        video = ""
        if s.dolly and b[0] and a[0]:
            mp4 = pair(b[0], a[0], out, s.name)
            if mp4:
                video = (f'<video src="{html.escape(os.path.basename(mp4))}" controls loop muted '
                         f'autoplay playsinline></video><p>before | after, the walk as shot</p>')
        body.append(f"<h2>{html.escape(s.name)} <small>{html.escape(s.owner)}</small></h2>"
                    f"<p>{html.escape(s.caption)}</p>{video}<div class=pair>"
                    f"<figure>{pic('', 'before', *b)}<figcaption>before</figcaption></figure>"
                    f"<figure>{pic('', 'after', *a)}<figcaption>after</figcaption></figure></div>")
    with open(os.path.join(out, "index.html"), "w") as f:
        f.write(f"""<!doctype html><meta charset=utf-8><title>{html.escape(title)}</title>
<style>body{{background:#181818;color:#eee;font:15px sans-serif;margin:16px}}
h2{{color:#ffdc8c;margin:28px 0 4px}} small{{color:#aaa;font-weight:normal}}
.pair{{display:grid;grid-template-columns:1fr 1fr;gap:10px}} img{{width:100%}}
figure{{margin:0}} video{{width:100%}} .none{{background:#464646;padding:20px;min-height:120px}}</style>
<h1>{html.escape(title)}</h1>{''.join(body)}""")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("list")
    ap.add_argument("--before")
    ap.add_argument("--after")
    ap.add_argument("--only", help="shot names, comma-separated")
    ap.add_argument("--jobs", type=int, default=1)
    ap.add_argument("--out")
    ap.add_argument("--check", action="store_true", help="read the list and stop")
    ap.add_argument("--try", dest="try_bin", metavar="BINARY",
                    help="shoot the after half only, with this binary (your own build), "
                         "into target/before_after/try/")
    o = ap.parse_args()
    settings, shots, errors = parse(o.list)
    for e in errors:
        print("ERROR", e)
    if errors:
        return 1
    if o.only:
        want = set(o.only.split(","))
        shots = [s for s in shots if s.name in want]
    if o.check:
        print(f"{o.list}: {len(shots)} shots, {settings['before']} -> {settings['after']}")
        return 0
    if o.try_bin:
        binary = os.path.abspath(o.try_bin)
        with concurrent.futures.ThreadPoolExecutor(max(1, o.jobs)) as pool:
            done = list(pool.map(lambda s: shoot(binary, "try", s, s.args, settings["size"]), shots))
        for s, (png, why) in zip(shots, done):
            print(f"{'ok' if png else 'MISSING'} {s.name}: {png or why}")
        return 0 if all(png for png, _ in done) else 1
    ref_b = o.before or settings["before"]
    ref_a = o.after or settings["after"]
    if ref_a.startswith("origin/"):
        subprocess.run(["git", "fetch", "-q", "origin", ref_a[len("origin/"):]], cwd=ROOT)
    before, after = sha8(ref_b), sha8(ref_a)
    bins = {before: build(before), after: build(after)}
    jobs = [(sha, s, s.before_args if sha == before else s.args) for s in shots for sha in (before, after)]
    with concurrent.futures.ThreadPoolExecutor(max(1, o.jobs)) as pool:
        done = list(pool.map(lambda j: shoot(bins[j[0]], j[0], j[1], j[2], settings["size"]), jobs))
    rows = [(s, done[2 * i], done[2 * i + 1]) for i, s in enumerate(shots)]
    for s, b, a in rows:
        state = "ok" if b[0] and a[0] else "MISSING"
        print(f"{state} {s.name}: before {'ok' if b[0] else b[1]}; after {'ok' if a[0] else a[1]}")
    out = o.out or os.path.join(BASE, f"{before}_{after}")
    title = f"Before {before}, after {after}"
    sheet(rows, title, out)
    os.makedirs(out, exist_ok=True)
    page(rows, title, out, copy=bool(o.out))
    print(f"sheet: {os.path.join(out, 'sheet.jpg')}")
    return 0 if all(b[0] and a[0] for _, b, a in rows) else 1


if __name__ == "__main__":
    sys.exit(main())
