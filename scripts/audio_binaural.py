"""The binaural pass of scripts/audio_audit.py: does the headset's
head-related filter tell where a sound is?

`cargo test audio::tests::binaural_tells_front_from_back` renders a click
from eight directions through the `hrtf` crate (the convolution the
game's HrtfNode runs), with our sphere (SADIE II's KEMAR,
scripts/audio_hrir.py) and the node's own IRCAM subject, and writes
target/audio_audit/binaural.json. This measures the cues the ears use on
that rendered output:

- **Left/right:** the near ear louder (interaural level, 0.5-10 kHz).
- **Front/back:** the pinna shades sound from behind, so ahead is
  brighter: the 4-10 kHz band over 0.5-3 kHz ("tilt"), front over back.
- **Up:** brighter again from above than ahead.
- **Arrival:** the near ear hears it first (interaural time, by
  cross-correlation), a few hundred microseconds for a side source.

Only our sphere is checked; IRCAM is printed alongside for comparison.
"""
import json

import numpy as np

SR = 48000
ILD_MIN = 6.0           # dB, a source at the side
ITD_MIN = 400e-6        # s, a source at the side (a head ~ 650 us at most)
FRONT_BACK_MIN = 2.5    # dB of tilt, front over back
UP_MIN = 2.0            # dB of tilt, above over ahead
CHECKED = "sadie_d2_kemar"


def band(h, lo, hi):
    spec = np.abs(np.fft.rfft(h, 4096)) ** 2
    f = np.fft.rfftfreq(4096, 1 / SR)
    return 10 * np.log10(spec[(f >= lo) & (f < hi)].mean() + 1e-30)


def tilt(d):
    t = lambda h: band(h, 4000, 10000) - band(h, 500, 3000)
    return (t(d["left"]) + t(d["right"])) / 2


def itd(d):
    """Seconds the right ear lags the left (cross-correlation peak)."""
    l, r = np.asarray(d["left"]), np.asarray(d["right"])
    c = np.correlate(r, l, "full")
    return (np.argmax(c) - (len(l) - 1)) / SR


def cues(dirs):
    side = dirs["left"]
    return dict(
        ild=band(side["left"], 500, 10000) - band(side["right"], 500, 10000),
        ild_right=band(dirs["right"]["right"], 500, 10000) - band(dirs["right"]["left"], 500, 10000),
        itd=itd(side),
        front_back=tilt(dirs["front"]) - tilt(dirs["back"]),
        up=tilt(dirs["up"]) - tilt(dirs["front"]),
        tilts={k: round(tilt(v), 1) for k, v in dirs.items()},
    )


def run(out):
    path = out / "binaural.json"
    if not path.exists():
        print("binaural: no target/audio_audit/binaural.json (run cargo test "
              "audio::tests::binaural_tells_front_from_back); skipped")
        return 0, []
    data = json.loads(path.read_text())["subjects"]
    lines = ["Binaural (a click through the head-related filter, from each side):",
             f"    {'subject':16s} {'ILD dB':>7s} {'ILD R':>6s} {'ITD us':>7s} {'front/back':>10s} "
             f"{'up':>5s}  flags"]
    flags = 0
    results = {}
    for name, dirs in data.items():
        c = cues(dirs)
        f = []
        if name == CHECKED:
            if min(c["ild"], c["ild_right"]) < ILD_MIN:
                f.append(f"side level {min(c['ild'], c['ild_right']):.1f} dB < {ILD_MIN}")
            if c["itd"] < ITD_MIN:
                f.append(f"left arrives {c['itd'] * 1e6:.0f} us early < {ITD_MIN * 1e6:.0f}")
            if c["front_back"] < FRONT_BACK_MIN:
                f.append(f"front over back {c['front_back']:.1f} dB < {FRONT_BACK_MIN}")
            if c["up"] < UP_MIN:
                f.append(f"above over ahead {c['up']:.1f} dB < {UP_MIN}")
            flags += bool(f)
        results[name] = dict(c, flags=f)
        lines.append(f"    {name:16s} {c['ild']:7.1f} {c['ild_right']:6.1f} {c['itd'] * 1e6:7.0f} "
                     f"{c['front_back']:10.1f} {c['up']:5.1f}  " + ("; ".join(f) if name == CHECKED
                                                                     else "(for comparison)"))
    lines.append("    tilt (4-10 kHz over 0.5-3 kHz, dB) by direction: " + "; ".join(
        f"{n}: " + ", ".join(f"{k} {v:+.1f}" for k, v in r["tilts"].items()) for n, r in results.items()))
    (out / "binaural_measured.json").write_text(json.dumps(results, indent=1))
    return flags, lines
