"""The score pass of scripts/audio_audit.py: the generative ambient layer
(src/audio_score.rs) against the mix.

`cargo test audio_score::tests::renders_for_the_audit` renders the score
at its in-game gain, 90 s at each of four intensities, to
target/audio_audit/score_<intensity>.wav. This checks it:

- **Under the wind.** The score's loudest 400 ms, at any intensity, is
  at least `UNDER_BED` LU under the wind bed's loudness: an undercurrent
  you notice, never a soundtrack you hear over the world.
- **Heard at all.** At full intensity it clears the wind's masking
  somewhere (else it's just CPU).
- **Never hides a robot.** Every placed robot sound (`robot.*`,
  `drone.*`, `walker.*`) is heard as far with the score at full as
  without it, to within `RANGE_KEPT`, over the wind and over the Surge's
  roar: by the audit's masking model, taking the score's loudest 3 s in
  each band (a fragment ringing, not the average).
"""
import math

import numpy as np

import audio_audit as aa
import audio_mix as am

UNDER_BED = 3.0         # LU the score's loudest moment stays under the wind
RANGE_KEPT = 0.9        # share of a robot sound's audible range kept with the score
ROBOTS = ("robot.", "drone.", "walker.")


def worst_bands(x, block=3.0):
    """Per third-octave band, the loudest any `block` seconds gets."""
    n = int(block * aa.SR)
    levels = [am.band_levels(x[i:i + n]) for i in range(0, len(x) - n + 1, n // 2)]
    return np.max(levels, axis=0)


def heard_to(cue, sets, mix, masker_bands):
    """`audio_mix.audible_range`, over a masker given as band levels."""
    c = next(c for c in mix if c["name"] == cue)
    x = am.samples(c["set"], sets)
    base = am.band_levels(x[: int(min(len(x), 1.0 * aa.SR))]) + c["db"] + am.own_filter(cue)
    t = am.masked_threshold(masker_bands)
    best = 0.0
    for d in np.geomspace(1, 1000, 100):
        heard = (base + am.falloff(d, c["reach"]) + am.fade_db(cue, d)
                 + am.muffle_db(am.cutoff(d, c["reach"], c["muffle_full"])))
        if np.max(heard - t) > 0:
            best = d
    return round(float(best))


def power_sum(*bands):
    return 10 * np.log10(sum(10 ** (b / 10) for b in bands))


def run(sets, out):
    renders = sorted(out.glob("score_*.wav"))
    if not renders:
        print("score: no target/audio_audit/score_*.wav (run cargo test "
              "audio_score::tests::renders_for_the_audit); skipped")
        return 0, []
    mix = am.read_mix()
    wind, _ = am.render([("wind", "loop", None, 0.0, None)], sets, mix, seconds=30.0)
    surge, _ = am.render([("wind", "loop", None, 0.0, None),
                          ("surge.drone", "loop", None, 0.0, None)], sets, mix, seconds=30.0)
    wind_lufs = aa.integrated(aa.k_weight(wind))
    wind_bands = worst_bands(wind)
    surge_bands = worst_bands(surge)
    flags = 0
    lines = [f"Score (src/audio_score.rs; the wind bed is {wind_lufs:.1f} LUFS):",
             f"    {'intensity':>9s} {'integrated':>10s} {'loudest':>8s} {'under wind':>10s} "
             f"{'clears wind':>11s}  flags"]
    full = None
    for path in renders:
        i = float(path.stem.split("_")[1])
        x = aa.decode(path)
        y = aa.k_weight(x)
        integrated = aa.integrated(y)
        loudest = float(aa.block_loudness(y).max())
        bands = worst_bands(x)
        t = am.masked_threshold(wind_bands)
        clears = float(np.max(bands - t))
        f = []
        if wind_lufs - loudest < UNDER_BED:
            f.append(f"loudest {loudest:.1f} LUFS, only {wind_lufs - loudest:.1f} LU under the wind")
        if i == 1.0:
            full = bands
            if clears <= 0:
                f.append("not heard over the wind at full")
        flags += bool(f)
        lines.append(f"    {i:9.2f} {integrated:10.1f} {loudest:8.1f} {wind_lufs - loudest:10.1f} "
                     f"{clears:+11.1f}  " + "; ".join(f))
    if full is None:
        return flags, lines
    lines.append(f"  Robots heard to (m), without / with the score at full (keeps {RANGE_KEPT:.0%}):")
    for c in mix:
        if not c["name"].startswith(ROBOTS) or c["dist"] <= 0:
            continue
        r = []
        f = []
        for bed_name, bed in (("wind", wind_bands), ("surge", surge_bands)):
            alone = heard_to(c["name"], sets, mix, bed)
            scored = heard_to(c["name"], sets, mix, power_sum(bed, full))
            r.append(f"{alone:4d} / {scored:4d}")
            if alone and scored < RANGE_KEPT * alone:
                f.append(f"over the {bed_name}, {scored} m of {alone}")
        flags += bool(f)
        lines.append(f"    {c['name']:20s} wind {r[0]}   surge {r[1]}  "
                     + ("masked: " + "; ".join(f) if f else ""))
    return flags, lines
