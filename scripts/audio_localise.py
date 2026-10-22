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
  brighter: the 4-10 kHz band over 0.5-3 kHz, ahead against the mirror
  bearing behind (0/180, 30/150, 60/120), the worst pair. Only readable if
  the sound has a high band left at that range.
- **Up** (what flies): brighter from above than level ahead.

Within its binaural reach (`audio::binaural_reach`: Signals reach
further than the rest) a placed sound in the headset goes through the
head-related filter, measured here with the responses on a grid
(`cargo test audio::tests::localisation_grid` writes
target/audio_audit/hrir_grid.json). Past it, and with binaural off, it is
panned (firewheel's SpatialBasicNode): a level difference only, and
behind you darkened (`audio::BEHIND_CUTOFF`, set by `occlude`) so that
front and back still differ; nothing tells above from level.
"""
import json
import math
import re

import numpy as np

import audio_audit as aa
import audio_mix as am

ROOT = aa.ROOT

# Threat cue -> (ranges measured, m; flies).
THREATS = {
    "drone.rotor": ((10, 30, 60, 100), True),
    "walker.step": ((10, 30, 60), False),
    "sentry.chirp": ((10, 30, 60), False),
    "crawler.step": ((5, 10, 15), False),
    "crawler.charge": ((5, 10, 15), False),
}
ILD_MIN = 3.0            # dB at 90 degrees, or...
ITD_MIN = 300e-6         # ...s at 90 degrees
FRONT_BACK_MIN = 2.0     # dB of tilt, ahead over behind, the worst pair
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
    return band(x, 4000, 10000) - band(x, 500, 3000)


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


def source(c, d, sets):
    x = am.samples(c["set"], sets)[: aa.SR, 0].astype(np.float64)
    fc = min(am.cutoff(d, c["reach"], c["muffle_full"]), am.CUE_LOWPASS.get(c["name"], 20480.0))
    return am.lowpass(x[:, None], fc)[:, 0]


def binaural_cues(x, grid, flies):
    ears = lambda k: (conv(x, grid[k]["left"]), conv(x, grid[k]["right"]))
    l, r = ears("az90_el0")
    ild = band(r, 500, 10000) - band(l, 500, 10000)
    t = itd(l, r)
    both = lambda k: tilt(np.concatenate(ears(k)))
    fb = min(both(f"az{a}_el0") - both(f"az{180 - a}_el0") for a in (0, 30, 60))
    up = min(both(f"az0_el{e}") for e in (60, 85)) - both("az0_el0") if flies else None
    return ild, t, fb, up


def panned_cues(x, behind):
    # Equal-power pan, held to the threshold, for a source at 90 degrees.
    pan = 0.5 + 0.5 * PAN_THRESHOLD
    ild = 20 * math.log10(math.sin(pan * math.pi / 2) / math.cos(pan * math.pi / 2))
    fb = tilt(x) - tilt(am.lowpass(x[:, None], behind)[:, 0]) if behind < 20000 else 0.0
    return ild, 0.0, fb, 0.0


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
    for name, (ranges, flies) in THREATS.items():
        c = next((c for c in mix if c["name"] == name), None)
        if c is None or c["set"] not in sets:
            lines.append(f"    {name:15s} (not in the mix or the bank)")
            continue
        reach = k["signal_reach"] if c["role"] == "Signal" else k["reach"]
        bad = False
        for d in ranges:
            x = source(c, d, sets)
            high = tilt(x)
            binaural = d < reach
            ild, t, fb, up = binaural_cues(x, grid, flies) if binaural else panned_cues(x, k["behind"])
            f = []
            if ild < ILD_MIN and t < ITD_MIN:
                f.append("side unclear")
            if high < HIGH_BAND_MIN:
                f.append(f"no high band ({high:.0f} dB)")
            elif fb < FRONT_BACK_MIN:
                f.append(f"front/back {fb:.1f} dB")
            if flies and (up or 0.0) < UP_MIN:
                f.append("above unclear" if binaural else "panned: no above")
            bad |= bool(f)
            results.append(dict(cue=name, range=d, path="binaural" if binaural else "panned",
                                ild=round(ild, 1), itd_us=round(t * 1e6), front_back=round(fb, 1),
                                up=None if up is None else round(up, 1), high=round(high, 1),
                                flags=f))
            lines.append(f"    {name:15s} {d:4d} {'binaural' if binaural else 'panned':9s} "
                         f"{ild:5.1f} {t * 1e6:6.0f} {fb:10.1f} "
                         f"{'' if up is None else f'{up:5.1f}':>5s}  " + "; ".join(f))
        flagged += bad
    (out / "localise.json").write_text(json.dumps(results, indent=1))
    return flagged, lines
