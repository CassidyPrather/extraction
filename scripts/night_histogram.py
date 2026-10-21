"""Luminance gauges for the night shots (scripts/night_matrix.sh).

For each shot: the mean and the 10th/50th/90th percentile of luma (0-255,
Rec. 709), the share of near-black pixels (luma < 8: a hole in a
headset), and a ten-bin histogram. For views with sky and ground, the
ground's near band (the bottom fifth of the frame, roughly 3-10 m ahead
at eye height) against the sky (the top fifth): the brief wants the
ground there told from the sky on a clear night.

    python3 scripts/night_histogram.py target/night [--check]

With --check it fails (exit 1) unless, in every valley: on a clear night
with a moon up the near ground and the sky differ by at least GROUND_SKY
luma in each view that has both; under fog and downpour each view's mean
is no brighter than the clear full-moon night's (within 2 luma: a forest floor is at the black floor either way); and no view is a hole (more than
HOLE of its pixels near black). And storms (`storm`, `stormflash`: a
strike's light held): a flash lights the near ground at least FLASH_LIFT
luma over the dark storm's, the sky brighter too, without blowing more
than BLOWN of the frame out (luma 250 or more); the dark storm is no
brighter than the clear full-moon night.
"""
import sys
from pathlib import Path

from PIL import Image


def luma(im):
    return Image.open(im).convert("RGB").convert("L", (0.2126, 0.7152, 0.0722, 0))


def stats(px):
    px = sorted(px)
    n = len(px)
    pct = lambda q: px[min(n - 1, int(q * n))]
    hist = [0] * 10
    for v in px:
        hist[min(9, v * 10 // 256)] += 1
    return {
        "mean": sum(px) / n,
        "p10": pct(0.1),
        "p50": pct(0.5),
        "p90": pct(0.9),
        "black": sum(1 for v in px if v < 8) / n,
        "blown": sum(1 for v in px if v >= 250) / n,
        "hist": [round(h * 100 / n) for h in hist],
    }


GROUND_SKY = 12
# Views under trees: the sky's a few gaps, so no ground-against-sky gauge.
FOREST = ("spruce", "pines", "wood", "belt")
# Views looking up at the sky (the moon's): no near ground in the frame.
SKYWARD = ("moon",)
HOLE = 0.25
FLASH_LIFT = 25
BLOWN = 0.15


def main(root, check=False):
    root = Path(root)
    seen = {}
    for shot in sorted(root.glob("*/*/*.png")):
        im = luma(shot)
        w, h = im.size
        s = stats(list(im.get_flattened_data()))
        sky = stats(list(im.crop((0, 0, w, h // 5)).get_flattened_data()))
        ground = stats(list(im.crop((0, h * 4 // 5, w, h)).get_flattened_data()))
        rel = shot.relative_to(root)
        seen[rel] = (s, sky, ground)
        print(
            f"{str(rel):46} mean {s['mean']:5.1f} p10 {s['p10']:3} p50 {s['p50']:3} "
            f"p90 {s['p90']:3} black {s['black']*100:4.1f}%  sky {sky['p50']:3} "
            f"ground {ground['p50']:3}  hist {s['hist']}"
        )


    if check:
        sys.exit(1 if failures(seen) else 0)


def failures(seen):
    bad = []
    for rel, (s, sky, ground) in seen.items():
        valley, case, view = rel.parts
        weather, moon, *hour = case.split("_")
        hour = hour[0] if hour else "night"
        if s["black"] > HOLE:
            bad.append(f"{rel}: {s['black']*100:.0f}% near black")
        if weather == "clearing" and moon != "new" and not any(f in view for f in FOREST + SKYWARD):
            if abs(sky["p50"] - ground["p50"]) < GROUND_SKY:
                bad.append(f"{rel}: ground {ground['p50']} too like the sky {sky['p50']}")
        if weather == "stormflash":
            dark = seen.get(Path(valley, case.replace("stormflash", "storm"), view))
            if dark:
                d_s, d_sky, d_ground = dark
                # Under trees the canopy shades the ground from it (the
                # flash casts shadows): the frame as a whole lifts.
                if any(f in view for f in FOREST):
                    if s["mean"] < d_s["mean"] + FLASH_LIFT:
                        bad.append(f"{rel}: the flash lit the wood to {s['mean']:.0f} from {d_s['mean']:.0f}")
                elif ground["p50"] < d_ground["p50"] + FLASH_LIFT:
                    bad.append(f"{rel}: the flash lit the ground to {ground['p50']} from {d_ground['p50']}")
                if sky["p50"] <= d_sky["p50"]:
                    bad.append(f"{rel}: the flash left the sky at {sky['p50']}")
            if s["blown"] > BLOWN:
                bad.append(f"{rel}: {s['blown']*100:.0f}% blown out")
        if weather in ("fog", "downpour") or (weather == "storm" and hour == "night"):
            clear = seen.get(Path(valley, "clearing_full", view))
            if clear and s["mean"] > clear[0]["mean"] + 2.0:
                bad.append(f"{rel}: overcast {s['mean']:.1f} not darker than clear {clear[0]['mean']:.1f}")
    for b in bad:
        print("FAIL", b)
    print(f"{len(bad)} gauge failures" if bad else "night gauges: all pass")
    return bad


if __name__ == "__main__":
    args = [a for a in sys.argv[1:] if a != "--check"]
    main(args[0] if args else "target/night", "--check" in sys.argv)
