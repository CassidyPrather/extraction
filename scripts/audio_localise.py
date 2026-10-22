"""The localisation pass of scripts/audio_audit.py: can you find a threat
by ear?

For each threat's sound (THREATS), at the ranges it matters, this plays
the sound itself (its first second, muffled by distance as firewheel does
and through any filter of its own) through what the game would play it
through at that range, and measures the cues the ears use:

- **Side:** a source at 90 degrees, the near ear louder (interaural level,
  0.5-10 kHz) or first (interaural time, by cross-correlation). Either
  will do: a low thud has little level difference but a clear delay.
- **Front/back:** the pinna shades sound from behind, so ahead is
  brighter: the 4-10 kHz band over 0.5-2 kHz, ahead against the mirror
  bearing behind (0/180 and 30/150), the worse pair. (Not 60/120: so near
  the ears' axis the responses themselves barely differ, -0.9 dB for
  white noise, and listeners confuse them too.) The reference stops at
  2 kHz, below the pinna's own work: from 2 kHz on our responses are
  already 3 dB brighter ahead, so a sound whose low band sits there (a
  drone's rotor) would hide its front/back difference in its reference.
  Only readable if the sound has a high band left at that range.
- **Up** (what flies): brighter from above than level ahead.

Within its binaural reach (`audio::binaural_reach`: Signals reach
further than the rest) a placed sound in the headset goes through the
head-related filter, measured here with the responses on a grid
(`cargo test audio::tests::localisation_grid` writes
target/audio_audit/hrir_grid.json). Past it, and with binaural off, it is
panned (firewheel's SpatialBasicNode): a level difference only, and
behind you darkened (`audio::BEHIND_CUTOFF`, set by `occlude`) so that
front and back still differ; nothing tells above from level. Each threat
is measured in the headset at each range, then flat (binaural off) at the
nearest, where only side and front/back are asked of it.
"""
import json
import math
import re

import numpy as np

import audio_audit as aa
import audio_mix as am

ROOT = aa.ROOT

# Threat -> (its layers as (cue, playback speed), as the game plays them
# together; ranges measured, m; flies).
THREATS = {
    "drone.rotor": ((("drone.rotor", 1.0),), (10, 30, 60, 100), True),
    "walker.step": ((("walker.step", 1.0),), (10, 30, 60), False),
    "sentry.chirp": ((("sentry.chirp", 1.0),), (10, 30, 60), False),
    "crawler.step": ((("crawler.step", 2.2),), (5, 10, 15), False),
    "crawler.charge": ((("crawler.charge", 0.8),), (5, 10, 15), False),
}
ILD_MIN = 3.0            # dB at 90 degrees, or...
ITD_MIN = 300e-6         # ...s at 90 degrees
FRONT_BACK_MIN = 2.0     # dB of tilt, ahead over behind, the worse pair
FRONT_BACK_PAIRS = (0, 30)
UP_MIN = 2.0             # dB of tilt, above over level
HIGH_BAND_MIN = -35.0    # dB, the 4-10 kHz band against 0.5-3 kHz: any less
                         # and there's nothing for the pinna to shade
PAN_THRESHOLD = 0.6      # SpatialBasicNode's default


def consts():
    text = (ROOT / "src" / "audio.rs").read_text()
    get = lambda name, default: float(m.group(1)) if (
        m := re.search(rf"const {name}: f32 = ([\d.]+)", text)) else default
    return dict(reach=get("BINAURAL_REACH", 40.0),
                signal_reach=get("SIGNAL_BINAURAL_REACH", get("BINAURAL_REACH", 40.0)),
                behind=get("BEHIND_CUTOFF", 20480.0))


def band(x, lo, hi):
    spec = np.abs(np.fft.rfft(x, axis=0)) ** 2
    f = np.fft.rfftfreq(len(x), 1 / aa.SR)
    sel = (f >= lo) & (f < hi)
    return 10 * np.log10(spec[sel].mean() + 1e-30)


def tilt(x):
    return band(x, 4000, 10000) - band(x, 500, 2000)


def conv(x, h):
    n = len(x) + len(h) - 1
    k = 1 << (n - 1).bit_length()
    return np.fft.irfft(np.fft.rfft(x, k) * np.fft.rfft(h, k), k)[:n]


def itd(l, r):
    """Seconds the left ear lags the right (the cross-correlation's peak,
    within a head's +-1 ms)."""
    m = int(0.001 * aa.SR)
    k = 1 << (len(l) + len(r)).bit_length()
    c = np.fft.irfft(np.fft.rfft(l, k) * np.conj(np.fft.rfft(r, k)), k)
    lags = np.concatenate([c[-m:], c[: m + 1]])
    return (np.argmax(lags) - m) / aa.SR


def layer(c, speed, d, sets):
    """One cue's sound at `d` m: a second of it at its playback speed,
    muffled by distance and its own filter, at its level and falloff."""
    x = am.samples(c["set"], sets)[:, 0].astype(np.float64)
    n = min(aa.SR, int(len(x) / speed))
    x = np.interp(np.arange(n) * speed, np.arange(len(x)), x)
    fc = min(am.cutoff(d, c["reach"], c["muffle_full"]), am.CUE_LOWPASS.get(c["name"], 20480.0))
    g = 10 ** ((c["db"] + am.falloff(d, c["reach"])) / 20)
    return am.lowpass(x[:, None], fc)[:, 0] * g


def source(layers, d, sets):
    parts = [layer(c, speed, d, sets) for c, speed in layers]
    n = max(len(p) for p in parts)
    return sum(np.pad(p, (0, n - len(p))) for p in parts)


def binaural_cues(x, grid, flies):
    ears = lambda k: (conv(x, grid[k]["left"]), conv(x, grid[k]["right"]))
    l, r = ears("az90_el0")
    ild = band(r, 500, 10000) - band(l, 500, 10000)
    t = itd(l, r)
    both = lambda k: tilt(np.concatenate(ears(k)))
    fb = min(both(f"az{a}_el0") - both(f"az{180 - a}_el0") for a in FRONT_BACK_PAIRS)
    up = min(both(f"az0_el{e}") for e in (60, 85)) - both("az0_el0") if flies else None
    return ild, t, fb, up


def panned_cues(x, behind, flies):
    # Equal-power pan, held to the threshold, for a source at 90 degrees;
    # behind, the panner's shade (`audio::behind_cutoff`); nothing for up.
    pan = 0.5 + 0.5 * PAN_THRESHOLD
    ild = 20 * math.log10(math.sin(pan * math.pi / 2) / math.cos(pan * math.pi / 2))
    fb = tilt(x) - tilt(am.lowpass(x[:, None], behind)[:, 0]) if behind < 20000 else 0.0
    return ild, 0.0, fb, 0.0 if flies else None


def run(sets, out):
    path = out / "hrir_grid.json"
    if not path.exists():
        print("localisation: no target/audio_audit/hrir_grid.json (run cargo test "
              "audio::tests::localisation_grid); skipped")
        return 0, []
    grid = {k: {e: np.asarray(v[e]) for e in ("left", "right")}
            for k, v in json.loads(path.read_text())["grid"].items()}
    k = consts()
    mix = am.read_mix()
    lines = ["Finding a threat by ear (its own sound, at range, through what plays it):",
             f"    {'threat':15s} {'m':>4s} {'path':9s} {'ILD':>5s} {'ITD us':>6s} "
             f"{'front/back':>10s} {'up':>5s}  flags"]
    flagged = 0
    results = []
    for name, (cues, ranges, flies) in THREATS.items():
        layers = [(next((c for c in mix if c["name"] == n), None), sp) for n, sp in cues]
        if any(c is None or c["set"] not in sets for c, _ in layers):
            lines.append(f"    {name:15s} (not in the mix or the bank)")
            continue
        c = layers[0][0]
        reach = k["signal_reach"] if c["role"] == "Signal" else k["reach"]
        bad = False
        # In the headset at each range; then flat (binaural off: the
        # panner at every range), at the nearest.
        for d, flat in [(d, False) for d in ranges] + [(ranges[0], True)]:
            x = source(layers, d, sets)
            high = tilt(x)
            binaural = d < reach and not flat
            ild, t, fb, up = (binaural_cues(x, grid, flies) if binaural
                              else panned_cues(x, k["behind"], flies))
            f = []
            if ild < ILD_MIN and t < ITD_MIN:
                f.append("side unclear")
            if high < HIGH_BAND_MIN:
                f.append(f"no high band ({high:.0f} dB)")
            elif fb < FRONT_BACK_MIN:
                f.append(f"front/back {fb:.1f} dB")
            if flies and binaural and up < UP_MIN:
                f.append("above unclear")
            if flies and not binaural and not flat:
                f.append("panned in the headset: no above")
            bad |= bool(f)
            path = "binaural" if binaural else ("flat" if flat else "panned")
            results.append(dict(cue=name, range=d, path=path,
                                ild=round(ild, 1), itd_us=round(t * 1e6), front_back=round(fb, 1),
                                up=None if up is None else round(up, 1), high=round(high, 1),
                                flags=f))
            lines.append(f"    {name:15s} {d:4d} {path:9s} "
                         f"{ild:5.1f} {t * 1e6:6.0f} {fb:10.1f} "
                         f"{'' if up is None else f'{up:5.1f}':>5s}  " + "; ".join(f))
        flagged += bad
    (out / "localise.json").write_text(json.dumps(results, indent=1))
    return flagged, lines
