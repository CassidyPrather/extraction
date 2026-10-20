"""The acoustics pass of scripts/audio_audit.py: what the rooms do to sound.

`cargo test acoustics::tests::every_place_is_probed_for_the_audit` probes
every building and underground room on the three maps from a head's height
(src/acoustics.rs) and writes target/audio_audit/acoustics.json: each
place's kind, surface, and the reverb the game sets there (wet level,
decay, damping). This pass renders that reverb offline with the same
Freeverb the game runs (firewheel-nodes' port: eight damped combs and four
all-passes a channel, wet only, width 0.8) and measures it:

- **Decay.** RT60 of the reverb's impulse response (Schroeder backward
  integration, T20 x 3) against what the probe asked for. Freeverb can't
  go shorter than about 0.6 s undamped (0.52 s with the damping), so
  short rooms are judged against that.
- **Wet against dry.** A rifle shot and a footstep through the room as the
  game plays them (dry plus the bus at its wet level): the
  direct-to-reverberant ratio, and how much the tail adds to the shot's
  momentary loudness (in a headset a shot mustn't get louder for being
  indoors by more than `TAIL_ADDS_MAX`).
- **Dry outside.** Fields and forests must stay dry.

It draws the decays and the tails' spectra into docs/shots/audio/.
"""

import json
import math
from pathlib import Path

import numpy as np

import audio_audit as aa

ROOT = Path(__file__).resolve().parent.parent
SHOTS = ROOT / "docs" / "shots" / "audio"

SR = aa.SR
COMB = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617]
ALLPASS = [556, 441, 341, 225]
SPREAD = 23
FIXED_GAIN, SCALE_WET, SCALE_DAMP, SCALE_ROOM, OFFSET_ROOM = 0.015, 3.0, 0.4, 0.28, 0.7
WIDTH = 0.8
FREEVERB_COMB = 0.0312          # src/acoustics.rs, the combs' mean delay (s)
FREEVERB_DAMPED = 1.16          # src/acoustics.rs, the damping's shortening, measured here

DECAY_OFF = 0.15                # measured RT60 more than this share off -> flag
TAIL_ADDS_MAX = 3.0             # LU a room's tail may add to a shot up close
OPEN_DRR_MIN = 30.0             # dB: outdoors is dry

CATEGORICAL = ["#2a78d6", "#eb6834", "#1baf7a", "#eda100", "#e87ba4", "#008300",
               "#6250d6", "#e34948"]
SURFACE, INK, MUTED, GRID = "#fcfcfb", "#1f1e1c", "#6b6a63", "#e4e3dd"


def freeverb_room(rt60):
    """src/acoustics.rs `freeverb_room`."""
    feedback = 10 ** (-3.0 * FREEVERB_COMB / max(rt60 * FREEVERB_DAMPED, 0.05))
    return min(1.0, max(0.0, (feedback - 0.7) / 0.28))


def impulse_response(room, damping, seconds):
    """Freeverb's stereo response to a centred unit impulse (both inputs 1),
    wet only, at SR."""
    n = int(seconds * SR)
    fb = room * SCALE_ROOM + OFFSET_ROOM
    damp = damping * SCALE_DAMP
    scale = lambda d: int(d * SR / 44100)
    x = np.zeros(n)
    x[0] = 2.0 * FIXED_GAIN
    outs = []
    for spread in (0, SPREAD):
        acc = np.zeros(n)
        for d0 in COMB:
            d = scale(d0 + spread)
            buf = np.zeros(d)
            state = 0.0
            out = np.empty(n)
            i = 0
            for t in range(n):
                o = buf[i]
                state = o * (1 - damp) + state * damp
                buf[i] = x[t] + state * fb
                out[t] = o
                i += 1
                if i == d:
                    i = 0
            acc += out
        for d0 in ALLPASS:
            d = scale(d0 + spread)
            buf = np.zeros(d)
            out = np.empty(n)
            i = 0
            for t in range(n):
                delayed = buf[i]
                out[t] = -acc[t] + delayed
                buf[i] = acc[t] + delayed * 0.5
                i += 1
                if i == d:
                    i = 0
            acc = out
        outs.append(acc)
    wet = SCALE_WET
    g1, g2 = wet * (WIDTH / 2 + 0.5), wet * ((1 - WIDTH) / 2)
    left = outs[0] * g1 + outs[1] * g2
    right = outs[1] * g1 + outs[0] * g2
    return np.stack([left, right], axis=1)


def schroeder_db(ir):
    e = (ir ** 2).sum(axis=1)
    edc = np.cumsum(e[::-1])[::-1]
    return 10 * np.log10(np.maximum(edc / max(edc[0], 1e-30), 1e-12))


def rt60_of(ir):
    """T20 (the -5 to -25 dB stretch of the decay), extrapolated to 60 dB."""
    edc = schroeder_db(ir)
    t = np.arange(len(edc)) / SR
    sel = (edc <= -5) & (edc >= -25)
    if sel.sum() < 10:
        return float("nan")
    k, _ = np.polyfit(t[sel], edc[sel], 1)
    return float(-60.0 / k) if k < 0 else float("inf")


def convolve(x, ir):
    """Mono `x` through the stereo `ir`, by FFT."""
    n = len(x) + len(ir) - 1
    size = 1 << (n - 1).bit_length()
    X = np.fft.rfft(x, size)
    return np.stack([np.fft.irfft(X * np.fft.rfft(ir[:, c], size), size)[:n]
                     for c in range(2)], axis=1)


def momentary_max(y):
    return float(aa.block_loudness(aa.k_weight(y), 0.4, 0.1).max())


def representatives(rows):
    """One place per (kind, surface): the one with the median decay."""
    groups = {}
    for r in rows:
        groups.setdefault((r["kind"], r["surface"]), []).append(r)
    reps = []
    for key, g in sorted(groups.items()):
        g = sorted(g, key=lambda r: r["rt60"])
        rep = dict(g[len(g) // 2])
        rep["count"] = len(g)
        rep["places"] = sorted({f"{r['map']}:{r['place'].split('/')[0]}" for r in g})[:6]
        reps.append(rep)
    return reps


def run(sets, out):
    path = out / "acoustics.json"
    if not path.exists():
        print("acoustics: no target/audio_audit/acoustics.json (run cargo test "
              "acoustics::tests::every_place_is_probed_for_the_audit); skipped")
        return 0, []
    rows = json.loads(path.read_text())
    shot = aa.decode(aa.AUDIO / sets["guns/rifle_shot"]["files"][0]["file"]).mean(axis=1)
    step = aa.decode(aa.AUDIO / sets["footsteps/concrete"]["files"][0]["file"]).mean(axis=1)
    dry_shot = np.stack([shot, shot], axis=1)
    shot_loud = momentary_max(dry_shot)
    results, flags = [], 0
    for rep in representatives(rows):
        wet_db = rep["wet_db"]
        r = dict(kind=rep["kind"], surface=rep["surface"], count=rep["count"],
                 places=rep["places"], want_rt60=round(rep["rt60"], 2), wet_db=round(wet_db, 1),
                 flags=[])
        if wet_db <= -59:
            r.update(rt60=None, drr_shot=None, tail_adds=0.0)
            if rep["kind"] != "Open":
                pass
        else:
            room = freeverb_room(rep["rt60"])
            ir = impulse_response(room, rep["damping"], min(6.0, 2.0 * rep["rt60"] + 1.0))
            measured = rt60_of(ir)
            g = 10 ** (wet_db / 20)
            tail = convolve(shot, ir) * g
            both = np.zeros((max(len(tail), len(dry_shot)), 2))
            both[:len(dry_shot)] += dry_shot
            both[:len(tail)] += tail
            drr = 10 * math.log10((dry_shot ** 2).sum() / max((tail ** 2).sum(), 1e-30))
            step_tail = convolve(step, ir) * g
            drr_step = 10 * math.log10(2 * (step ** 2).sum() / max((step_tail ** 2).sum(), 1e-30))
            adds = momentary_max(both) - shot_loud
            r.update(room=round(room, 3), rt60=round(measured, 2), drr_shot=round(drr, 1),
                     drr_step=round(drr_step, 1), tail_adds=round(adds, 1),
                     decay=schroeder_db(ir)[:: SR // 100].round(1).tolist(),
                     # The tail past its first 100 ms, where damping shows.
                     bands=[round(b, 1) for b in aa.third_octaves(ir[SR // 10:, 0])])
            floor = max(rep["rt60"], 0.6 / FREEVERB_DAMPED)
            if not math.isfinite(measured) or abs(measured / floor - 1) > DECAY_OFF:
                r["flags"].append(f"decays in {measured:.2f} s, wants {floor:.2f}")
            if adds > TAIL_ADDS_MAX:
                r["flags"].append(f"tail adds {adds:+.1f} LU to a shot")
        if rep["kind"] == "Open" and wet_db > -59:
            r["flags"].append("outdoors but wet")
        flags += bool(r["flags"])
        results.append(r)
    (out / "acoustics_measured.json").write_text(json.dumps(results, indent=1))
    lines = ["Acoustics by place (probed from a head, reverb rendered with the game's Freeverb):",
             f"    {'kind':12s} {'surface':8s} {'n':>3s} {'wet dB':>7s} {'RT60 want':>9s} "
             f"{'got':>5s} {'DRR shot':>8s} {'DRR step':>8s} {'adds LU':>7s}  flags"]
    for r in results:
        f = lambda v, w, p=1: f"{v:{w}.{p}f}" if isinstance(v, (int, float)) else f"{'-':>{w}s}"
        lines.append(f"    {r['kind']:12s} {r['surface']:8s} {r['count']:3d} {f(r['wet_db'], 7)} "
                     f"{f(r['want_rt60'], 9, 2)} {f(r.get('rt60'), 5, 2)} {f(r.get('drr_shot'), 8)} "
                     f"{f(r.get('drr_step'), 8)} {f(r.get('tail_adds'), 7)}  " + "; ".join(r["flags"]))
    try:
        draw(results)
        lines.append(f"    (plots: {SHOTS.relative_to(ROOT)}/acoustics_decay.png, acoustics_spectra.png)")
    except ImportError:
        lines.append("    (no Pillow: plots skipped)")
    return flags, lines


# ------------------------------------------------------------------ plots


def _font(size):
    from PIL import ImageFont
    for p in ("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
              "/usr/share/fonts/dejavu/DejaVuSans.ttf"):
        if Path(p).exists():
            return ImageFont.truetype(p, size)
    return ImageFont.load_default()


def _chart(title, subtitle, xlabel, ylabel, xr, yr, series, xticks, yticks, xfmt, path):
    """A line chart on one axis: `series` is [(label, colour, xs, ys)],
    named in a legend at the right."""
    from PIL import Image, ImageDraw
    W, H = 1100, 640
    L, R, T, B = 80, 230, 90, 70
    img = Image.new("RGB", (W, H), SURFACE)
    d = ImageDraw.Draw(img)
    ft, fs, fl = _font(22), _font(15), _font(14)
    d.text((L, 22), title, fill=INK, font=ft)
    d.text((L, 54), subtitle, fill=MUTED, font=fs)
    px = lambda x: L + (x - xr[0]) / (xr[1] - xr[0]) * (W - L - R)
    py = lambda y: T + (yr[1] - y) / (yr[1] - yr[0]) * (H - T - B)
    for y in yticks:
        d.line([(L, py(y)), (W - R, py(y))], fill=GRID, width=1)
        d.text((L - 10, py(y)), f"{y:g}", fill=MUTED, font=fl, anchor="rm")
    for x in xticks:
        d.text((px(x), H - B + 12), xfmt(x), fill=MUTED, font=fl, anchor="mt")
    d.line([(L, H - B), (W - R, H - B)], fill=MUTED, width=1)
    d.text(((L + W - R) / 2, H - 24), xlabel, fill=MUTED, font=fs, anchor="mm")
    d.text((L - 60, T - 26), ylabel, fill=MUTED, font=fs)
    for label, colour, xs, ys in series:
        pts = [(px(x), py(y)) for x, y in zip(xs, ys)
               if xr[0] <= x <= xr[1] and yr[0] <= y <= yr[1]]
        if len(pts) > 1:
            d.line(pts, fill=colour, width=2, joint="curve")
    # More than four lines: a legend, in the order given (the caller orders
    # it as the lines read), rather than labels on the lines.
    for i, (label, colour, _, _) in enumerate(series):
        y = T + 6 + i * 24
        d.line([(W - R + 12, y), (W - R + 32, y)], fill=colour, width=3)
        d.text((W - R + 40, y), label, fill=INK, font=fl, anchor="lm")
    SHOTS.mkdir(parents=True, exist_ok=True)
    img.save(path)


def draw(results):
    wet = [r for r in results if r.get("decay")]
    wet.sort(key=lambda r: -(r.get("rt60") or 0))
    series = []
    for i, r in enumerate(wet[: len(CATEGORICAL)]):
        ys = [y for y in r["decay"] if y > -60.5]
        xs = [k / 100 for k in range(len(ys))]
        label = f"{r['kind'].lower()}, {r['surface'].lower()} ({r['rt60']:.1f} s)"
        series.append((label, CATEGORICAL[i], xs, ys))
    xmax = max(max(s[2]) for s in series)
    _chart("Reverb decay by place",
           "Energy left in the tail, Schroeder integration of the game's Freeverb; one line "
           "per kind of place, its median room",
           "seconds after the sound", "dB", (0, min(xmax, 6)), (-60, 0),
           series, [0, 1, 2, 3, 4, 5, 6], [0, -10, -20, -30, -40, -50, -60],
           lambda x: f"{x:g} s", SHOTS / "acoustics_decay.png")
    centres = aa.THIRDS
    series = []
    for i, r in enumerate(wet[: len(CATEGORICAL)]):
        b = np.array(r["bands"])
        b = b - b.max()
        xs = [math.log10(f) for f in centres]
        series.append((f"{r['kind'].lower()}, {r['surface'].lower()}", CATEGORICAL[i], xs, b.tolist()))
    _chart("Reverb tail spectrum by place",
           "1/3-octave level of each place's reverb tail (after its first 100 ms), "
           "normalised to its loudest band",
           "Hz", "dB", (math.log10(50), math.log10(16000)), (-40, 0),
           series, [math.log10(f) for f in (63, 125, 250, 500, 1000, 2000, 4000, 8000, 16000)],
           [0, -10, -20, -30, -40], lambda x: f"{10 ** x:,.0f}", SHOTS / "acoustics_spectra.png")
