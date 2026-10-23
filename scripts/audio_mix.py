"""The mix and scene passes of scripts/audio_audit.py (see its docstring).

The mix comes from src/audio_mix.rs (`cue!` lines). A cue's level as heard
is its asset's loudness plus its playback gain plus the inverse-distance
falloff at its usual distance (Firewheel's default: full within 5 m, then
-6 dB per doubling). That is set against:

- a line through the cues' real-world levels (`spl`), fitted robustly over
  the details and actions: its slope is how hard the game compresses the
  world's dynamics, and the cues far off it are the ones to look at;
- the beds, which signals must clear (by `SIGNAL_OVER_BED`) and footsteps
  sit just above;
- gunfire, which nothing but the Surge should be louder than.

Scenes are mixed offline from the assets as the game plays them, and
measured for loudness, peaks and masking: for each signal in a scene, the
margin by which it stands out of everything else in the 1/3-octave band
where it's strongest (a simple masking model: the masker's band level,
spread to neighbouring bands at -25 dB/band below and -12 dB/band above,
less 6 dB). And every signal's audible range over the calm and the
downpour beds, from the same model with the distance falloff.
"""

import json
import math
import re
from pathlib import Path

import numpy as np

import audio_audit as aa

ROOT = Path(__file__).resolve().parent.parent

# Rules of thumb, as numbers.
SIGNAL_OVER_BED = 10.0   # LU an unplaced signal should clear the loudest bed by
# How far off a signal must still be heard, metres, over the calm and the
# downpour beds (by the masking model): a threat's warning reaches about as
# far as the threat does.
RANGE_TARGETS = {
    "ug.cage_bell": (35, 20), "ug.klaxon": (50, 30), "crawler.step": (15, 8), "anomaly.frost": (4, 2),
    "drone.rotor": (120, 40), "drone.lock": (60, 40), "walker.step": (60, 30),
    "door.blow": (60, 25), "door.broken": (80, 30), "anomaly.snare": (15, 10),
    "anomaly.burner": (20, 12), "anomaly.lift": (15, 10),
    "anomaly.well": (15, 10), "anomaly.mirror": (15, 10), "anomaly.reveal": (15, 8),
    "detector.click": (3, 2), "gun.dry_fire": (5, 1), "gun.malfunction": (5, 1),
    "radio.squelch": (3, 2), "chatter.squelch": (3, 2), "chatter.static": (3, 2), "chatter.voice": (3, 2),
    "power.generator": (40, 20), "ug.canary": (15, 8),
    # the machines' plan (squad::CHIRP_REACH 40 m) and a relay filing you
    "squad.pin": (40, 20), "squad.flank": (40, 20), "squad.cutoff": (40, 20),
    "squad.spot": (40, 20), "squad.call": (40, 20), "relay.filed": (30, 15),
    "hand.refused": (2, 1), "hit.glance": (2, 1), "offering": (10, 5),
    "truck.arrive": (60, 15), "walker.aim": (60, 30),
    "voice.idle": (20, 8), "voice.noticed": (30, 15), "voice.investigating": (30, 15),
    "voice.searching": (25, 10), "voice.engaging": (40, 20), "voice.lost": (25, 10),
    "voice.calling": (200, 80), "voice.answering": (200, 80), "voice.hurt": (30, 15),
    "voice.dying": (40, 20), "voice.flanking": (30, 15), "voice.retreating": (30, 15),
    "heavy.step": (150, 60), "heavy.raise": (40, 20), "heavy.idle": (40, 15),
    "heavy.horn": (500, 200), "sentry.chirp": (60, 30),
    "walker.aim_lock": (40, 20), "harpoon.windup": (30, 15), "dirigible.engine": (200, 80), "truck.idle": (150, 50), "hit.body": (2, 1),
}
STEP_OVER_BED = (2.0, 10.0)  # footsteps: audible over the wind, not on top
OFF_LINE = 5.0           # LU from the fitted line before a cue is flagged
# Comfort in a headset: nothing sudden may hit harder up close than your own
# rifle does with the normal dynamics, whichever dynamics are on (a
# one-shot's loudest variant, at the nearest it plays; set pieces at their
# usual distance). Narrow dynamics is for a noisy room, which takes the edge
# off a little: there, 2 LU over.
STARTLE_OVER_SHOT = 0.0
STARTLE_OVER_SHOT_NARROW = 2.0
GEAR_HEARD = (0.6, 1.6)  # a kit rattle's audible range over its robot reach
MASK_OFFSET = 6.0        # dB below the masker's spread level that still hides
REF_DISTANCE = 5.0       # Firewheel's default reference distance

CUE = re.compile(
    r'cue!\("([^"]+)",\s*"([^"]+)",\s*(-?[\d.]+),\s*(-?[\d.]+),\s*(-?[\d.]+),\s*(\w+)'
    r'(?:,\s*(-?[\d.]+))?\)')


def read_mix():
    text = (ROOT / "src" / "audio_mix.rs").read_text()
    full = float(re.search(r"MUFFLE_FULL: f32 = ([\d.]+)", text).group(1))
    return [
        dict(name=n, set=s, db=float(db), spl=float(spl), dist=float(d), role=r,
             reach=float(reach or REF_DISTANCE), muffle_full=full)
        for n, s, db, spl, d, r, reach in CUE.findall(text)
    ]


def falloff(dist, reach=REF_DISTANCE):
    """dB at `dist` metres (0 means unspatialised)."""
    if dist <= 0:
        return 0.0
    return 20 * math.log10(min(1.0, reach / dist))


def cutoff(dist, reach, full):
    """Firewheel's distance muffle: a one-pole low-pass whose cutoff falls
    from 20 kHz at `reach` to 20 Hz at `full` (factor 1.9)."""
    if dist <= reach or dist <= 0:
        return 20480.0
    t = min(1.0, (dist - reach) / (full - reach))
    return 20.0 + 20460.0 * (1.0 - t ** (1 / 1.9))


def muffle_db(fc):
    """The one-pole's attenuation per 1/3-octave band, dB."""
    f = np.array(aa.THIRDS, dtype=float)
    return -10 * np.log10(1 + (f / fc) ** 2)


# Filters the game puts on a cue's own chain (src/audio.rs), Hz: the
# suppressed shot's low-pass, water muffled in a bottle. Judged by what
# they take out of the asset's spectrum.
CUE_LOWPASS = {"gun.shot_suppressed": 2200.0, "gear.slosh": 900.0, "gun.squib": 450.0,
               "gun.bulged": 1400.0, "gun.out_of_battery": 3000.0, "ug.echo": 2500.0, "gun.bolt_stuck": 1800.0, "heavy.step": 220.0,
               "heavy.slam": 600.0, "heavy.stall": 2500.0, "echo.slap": 2500.0,
               "thunder.crack": 8000.0, "thunder.roll": 4000.0,
               # src/audio_underground.rs: water down a pipe, firedamp
               # through rock, a cart's iron wheels.
               "ug.gurgle": 900.0, "ug.whump": 300.0, "ug.cart": 600.0,
               # the kiosk doors' breath (`stair_breath`), one cut-off a cue.
               "ug.breath_hum": 400.0, "ug.breath_water": 1000.0,
               "ug.breath_draught": 650.0, "ug.breath_hiss": 3500.0,
               "ug.breath_buzz": 2500.0, "ug.breath_roar": 500.0,
               "ug.breath_wind": 1000.0}


def own_filter(cue_name):
    """A cue's own low-pass, per 1/3-octave band, dB (0 for none)."""
    fc = CUE_LOWPASS.get(cue_name)
    return muffle_db(fc) if fc else 0.0


def filter_loss(cue, sets):
    fc = CUE_LOWPASS.get(cue["name"])
    if not fc:
        return 0.0
    bands = np.array(sets[cue["set"]]["bands"], dtype=float)
    p = 10 ** (bands / 10)
    return float(10 * np.log10((p * 10 ** (muffle_db(fc) / 10)).sum() / p.sum()))


def heard(cue, sets):
    a = sets[cue["set"]]
    base = a["loudness"] + filter_loss(cue, sets)
    return base + cue["db"] + falloff(cue["dist"], cue["reach"])


def theil_sen(x, y):
    slopes = [(y[j] - y[i]) / (x[j] - x[i])
              for i in range(len(x)) for j in range(i + 1, len(x)) if x[j] != x[i]]
    k = float(np.median(slopes))
    a = float(np.median([yi - k * xi for xi, yi in zip(x, y)]))
    return k, a


def mix_pass(sets, mix):
    for c in mix:
        c["heard"] = round(heard(c, sets), 1)
    fit = [c for c in mix if c["role"] in ("Detail", "Action")]
    k, a = theil_sen([c["spl"] for c in fit], [c["heard"] for c in fit])
    beds = {c["name"]: c["heard"] for c in mix if c["role"] == "Bed"}
    loudest_bed = max(beds["wind"], beds["rain"])
    gunshot = next(c["heard"] for c in mix if c["name"] == "gun.shot")
    for c in mix:
        c["line"] = round(a + k * c["spl"], 1)
        c["off_line"] = round(c["heard"] - c["line"], 1)
        flags = []
        if c["role"] in ("Detail", "Action") and abs(c["off_line"]) > OFF_LINE:
            flags.append(f"{c['off_line']:+.0f} LU off the line")
        if c["role"] == "Signal":
            # Placed signals are judged by how far off they're heard (see
            # `ranges`); an unplaced one by how far it clears the beds.
            if c["dist"] <= 0 and c["heard"] < loudest_bed + SIGNAL_OVER_BED:
                flags.append(f"only {c['heard'] - loudest_bed:+.0f} LU over the rain bed")
            if c["heard"] > gunshot + 3:
                flags.append(f"{c['heard'] - gunshot:+.0f} LU over your own gunshot")
        if c["name"] == "step":
            over = c["heard"] - beds["wind"]
            if not STEP_OVER_BED[0] <= over <= STEP_OVER_BED[1]:
                flags.append(f"{over:+.0f} LU over the wind")
        s = sets[c["set"]]
        if s["spread"] > 6 and s["type"] == "oneshot":
            flags.append(f"variants spread {s['spread']:.0f} LU")
        # Loud, rare events may lean on the master limiter; nothing else.
        if s["true_peak"] + c["db"] > (3.0 if c["role"] in ("Event", "Action") else 0.0):
            flags.append(f"true peak {s['true_peak'] + c['db']:+.1f} dBTP at its gain")
        c["flags"] = flags
    return dict(slope=round(k, 3), intercept=round(a, 1), beds=beds, gunshot=gunshot), mix


# ---------------------------------------------------------------- scenes

_cache = {}


def samples(set_name, sets, i=0):
    files = sets[set_name]["files"]
    key = (set_name, i % len(files))
    if key not in _cache:
        x = aa.decode(aa.AUDIO / files[i % len(files)]["file"])
        _cache[key] = x if x.shape[1] == 2 else np.repeat(x, 2, axis=1)
    return _cache[key]


def gain_of(cue_name, mix, extra=0.0, dist=None):
    c = next(c for c in mix if c["name"] == cue_name)
    d = c["dist"] if dist is None else dist
    return 10 ** ((c["db"] + extra + falloff(d, c["reach"])) / 20), c["set"]


def lowpass(x, fc):
    """One-pole low-pass magnitude, applied in the frequency domain."""
    if fc >= 20000:
        return x
    X = np.fft.rfft(x, axis=0)
    f = np.fft.rfftfreq(len(x), 1 / aa.SR)
    return np.fft.irfft(X / np.sqrt(1 + (f / fc) ** 2)[:, None], len(x), axis=0)


def render(scene, sets, mix, seconds=20.0):
    """A scene: (cue, start seconds or 'loop', distance or None, extra dB,
    every seconds). Returns the whole mix and each part separately."""
    n = int(seconds * aa.SR)
    parts = {}
    for i, (cue, start, dist, extra, every) in enumerate(scene):
        g, set_name = gain_of(cue, mix, extra, dist)
        c = next(c for c in mix if c["name"] == cue)
        d = c["dist"] if dist is None else dist
        fc = cutoff(d, c["reach"], c["muffle_full"])
        buf = np.zeros((n, 2))
        if start == "loop":
            x = samples(set_name, sets)
            reps = int(math.ceil(n / len(x)))
            buf += np.tile(x, (reps, 1))[:n] * g
        else:
            t, k = start, 0
            while t < seconds:
                x = samples(set_name, sets, k)
                s0 = int(t * aa.SR)
                m = min(len(x), n - s0)
                buf[s0:s0 + m] += x[:m] * g
                k += 1
                if not every:
                    break
                t += every
        parts[f"{i}:{cue}"] = lowpass(buf, fc)
    total = sum(parts.values())
    return total, parts


def band_levels(x):
    return np.array(aa.third_octaves(x.mean(axis=1)))


def masked_threshold(m):
    """Per band: the level a signal must beat to be heard over masker `m`
    (band levels, dB), with spreading to neighbouring bands."""
    n = len(m)
    out = np.full(n, -200.0)
    for b in range(n):
        acc = 0.0
        for j in range(n):
            spread = (b - j) * -12.0 if b > j else (j - b) * -25.0
            acc += 10 ** ((m[j] + spread) / 10)
        out[b] = 10 * math.log10(acc + 1e-30) - MASK_OFFSET
    return out


def margin(signal, rest, start, length):
    """How far the signal clears the masking threshold of everything else,
    in its best band, while it plays."""
    s0, s1 = int(start * aa.SR), int((start + length) * aa.SR)
    s = band_levels(signal[s0:s1])
    m = band_levels(rest[s0:s1])
    t = masked_threshold(m)
    i = int(np.argmax(s - t))
    return round(float(s[i] - t[i]), 1), aa.THIRDS[i]


SCENES = {
    # Walking through a still forest: wind, footsteps, a crow out in the fog.
    "quiet_walk": [
        ("wind", "loop", None, 0.0, None),
        ("step", 0.3, None, -3.0, 0.55),
        ("birds", 6.0, 45.0, 0.0, None),
        ("drone.rotor", "loop", 60.0, 0.0, None),
    ],
    # A downpour: can you hear your own feet, and a drone coming?
    "downpour": [
        ("wind", "loop", None, 0.0, None),
        ("rain", "loop", None, 0.0, None),
        ("step", 0.3, None, -3.0, 0.55),
        ("drone.rotor", "loop", 30.0, 0.0, None),
        ("drone.lock", 8.0, 30.0, 0.0, None),
    ],
    # A firefight in the rain: your rifle, the drone's gun, its lock-on, a
    # walker coming, impacts.
    "firefight": [
        ("wind", "loop", None, 0.0, None),
        ("rain", "loop", None, -6.0, None),
        ("gun.shot", 2.0, None, 0.0, 0.25),
        ("robot.shot", 3.1, None, 0.0, 0.9),
        ("hit.impact", 2.05, None, 0.0, 0.5),
        ("drone.rotor", "loop", 20.0, 0.0, None),
        ("drone.lock", 9.0, 20.0, 0.0, None),
        ("walker.step", 0.5, 25.0, 0.0, 0.7),
    ],
    # Indoors with the door shut: room tone, a door, a walker outside.
    "indoors": [
        ("room", "loop", None, 0.0, None),
        ("wind", "loop", None, -14.0, None),
        ("door.close", 2.0, None, 0.0, None),
        ("walker.step", 0.5, 25.0, 0.0, 0.7),
        ("gun.mag_insert", 6.0, None, 0.0, None),
    ],
    # The Surge: the roar, cracks overhead, the siren at the Gate.
    "surge": [
        ("wind", "loop", None, 0.0, None),
        ("surge.drone", "loop", None, -3.0, None),
        ("surge.crack", 4.0, None, 0.0, 5.0),
        ("surge.siren", 1.0, None, 0.0, None),
    ],
}

# The signals whose margin each scene checks: (part index, start, length).
PROBES = {
    "quiet_walk": [(1, 0.3, 0.3), (3, 0.0, 4.0)],
    "downpour": [(2, 0.3, 0.3), (3, 0.0, 4.0), (4, 8.0, 0.5)],
    "firefight": [(6, 9.0, 0.5), (7, 0.5, 0.4), (5, 0.0, 1.5)],
    "indoors": [(3, 0.5, 0.4), (4, 6.0, 0.3)],
    "surge": [(3, 1.0, 4.0)],
}


# Share of a raid spent in each scene. Fighting is measured from the bot
# raids (engagements were 6.7% of 164 bot-minutes); a fifth of raids
# draw a downpour; indoors and the Surge are estimates.
SESSION = {"quiet_walk": 0.60, "downpour": 0.15, "indoors": 0.15,
           "firefight": 0.07, "surge": 0.03}
# ASWG-R001 (the game-audio loudness practice from Sony's Audio Standards
# Working Group): -24 LUFS +-2 integrated over representative play on
# console/PC, true peaks at most -1 dBTP.
SESSION_TARGET = (-24.0, 2.0)


def session_loudness(blocks):
    """BS.1770 gated loudness and EBU 3342 loudness range of a raid, from
    each scene's 400 ms and 3 s blocks weighted by its share of play."""
    def gated(b, w, rel_db):
        keep = b > -70
        b, w = b[keep], w[keep]
        e = lambda b, w: 10 * np.log10(np.sum(w * 10 ** (b / 10)) / np.sum(w))
        rel = e(b, w) + rel_db
        return b[b > rel], w[b > rel], e
    m = np.concatenate([blocks[n][0] for n in SESSION])
    mw = np.concatenate([np.full(len(blocks[n][0]), SESSION[n] / len(blocks[n][0]))
                         for n in SESSION])
    b, w, e = gated(m, mw, -10)
    integrated = e(b, w)
    s = np.concatenate([blocks[n][1] for n in SESSION])
    sw = np.concatenate([np.full(len(blocks[n][1]), SESSION[n] / len(blocks[n][1]))
                         for n in SESSION])
    b, w, _ = gated(s, sw, -20)
    order = np.argsort(b)
    cw = np.cumsum(w[order]) / np.sum(w)
    lo, hi = b[order][np.searchsorted(cw, 0.10)], b[order][np.searchsorted(cw, 0.95)]
    return round(float(integrated), 1), round(float(hi - lo), 1)


def scene_pass(sets, mix):
    out = {}
    blocks = {}
    for name, scene in SCENES.items():
        total, parts = render(scene, sets, mix)
        y = aa.k_weight(total)
        blocks[name] = (aa.block_loudness(y, 0.4, 0.1), aa.block_loudness(y, 3.0, 1.0))
        r = {
            "integrated": round(aa.integrated(y), 1),
            "momentary_max": round(float(aa.block_loudness(y).max()), 1),
            "sample_peak": round(20 * math.log10(max(np.abs(total).max(), 1e-9)), 1),
            "clipped": int(np.sum(np.abs(total) > 1.0)),
            "probes": [],
        }
        keys = list(parts)
        for idx, start, length in PROBES.get(name, []):
            key = keys[idx]
            rest = total - parts[key]
            m, band = margin(parts[key], rest, start, length)
            r["probes"].append({"cue": key.split(":", 1)[1], "margin_db": m, "band_hz": band})
        out[name] = r
    out["_session"] = session_loudness(blocks)
    return out


def loop_fades():
    """Loops the game only keeps near you (anomaly hums): cue -> (fade
    start, keep distance), read from audio.rs. Inside the fade start they
    play at full level; they ease to -40 dB at the keep distance, where
    they start and stop."""
    text = (ROOT / "src" / "audio.rs").read_text()
    near = float(re.search(r"ANOMALY_FADE_NEAR: f32 = ([\d.]+)", text).group(1))
    keep = float(re.search(r"ANOMALY_KEEP: f32 = ([\d.]+)", text).group(1))
    return {n: (near, keep) for n in ("anomaly.lift", "anomaly.burner", "anomaly.snare",
                                         "anomaly.well", "anomaly.mirror")}


def fade_db(name, d):
    f = loop_fades().get(name)
    if not f:
        return 0.0
    t = min(1.0, max(0.0, (d - f[0]) / (f[1] - f[0])))
    return -40.0 * t * t


def audible_margin(cue_name, sets, mix, bed, d):
    """How far over the bed's masking a cue is at `d` metres, dB."""
    c = next(c for c in mix if c["name"] == cue_name)
    x = samples(c["set"], sets)
    base = band_levels(x[: int(min(len(x), 1.0 * aa.SR))]) + c["db"] + own_filter(cue_name)
    bed_parts = []
    for b, extra in bed:
        g, s = gain_of(b, mix, extra)
        y = samples(s, sets)
        bed_parts.append(y[: aa.SR * 4] * g)
    t = masked_threshold(band_levels(sum(bed_parts)))
    heard = (base + falloff(d, c["reach"]) + fade_db(cue_name, d)
             + muffle_db(cutoff(d, c["reach"], c["muffle_full"])))
    return float(np.max(heard - t))


def audible_range(cue_name, sets, mix, bed):
    """The farthest a signal is still heard over a bed (by the masking
    model, with the distance falloff, muffle and any keep-near fade),
    metres, to 1 km."""
    c = next(c for c in mix if c["name"] == cue_name)
    x = samples(c["set"], sets)
    base = band_levels(x[: int(min(len(x), 1.0 * aa.SR))]) + c["db"] + own_filter(cue_name)
    reach, full = c["reach"], c["muffle_full"]
    bed_parts = []
    for b, extra in bed:
        g, s = gain_of(b, mix, extra)
        y = samples(s, sets)
        bed_parts.append(y[: aa.SR * 4] * g)
    m = band_levels(sum(bed_parts))
    t = masked_threshold(m)
    best = 0.0
    for d in np.geomspace(1, 1000, 100):
        heard_bands = (base + falloff(d, reach) + fade_db(cue_name, d)
                       + muffle_db(cutoff(d, reach, full)))
        if np.max(heard_bands - t) > 0:
            best = d
    return round(float(best))


def run(sets, out):
    mix = read_mix()
    summary, mix = mix_pass(sets, mix)
    scenes = scene_pass(sets, mix)
    session, lra = scenes.pop("_session")
    # The same raid with the comfort board's narrow dynamics.
    text = (ROOT / "src" / "audio_mix.rs").read_text()
    lift = {r: float(v) for r, v in re.findall(r"\(Role::(\w+), (-?[\d.]+)\)", text)}
    narrow_mix = [dict(c, db=c["db"] + lift.get(c["role"], 0.0)) for c in mix]
    narrow_scenes = scene_pass(sets, narrow_mix)
    n_session, n_lra = narrow_scenes.pop("_session")
    calm = [("wind", 0.0)]
    storm = [("wind", 0.0), ("rain", 0.0)]
    ranges = {
        c["name"]: {"calm": audible_range(c["name"], sets, mix, calm),
                    "downpour": audible_range(c["name"], sets, mix, storm)}
        for c in mix if c["role"] == "Signal" and c["dist"] > 0
    }
    for n, r in ranges.items():
        want = RANGE_TARGETS.get(n)
        r["want"] = want
        if want and (r["calm"] < want[0] or r["downpour"] < want[1]):
            next(c for c in mix if c["name"] == n)["flags"].append(
                f"heard to {r['calm']}/{r['downpour']} m, wants {want[0]}/{want[1]}")
    # Your kit (src/foley.rs): at its cue's gain a part is one robots hear
    # to GEAR_REACH_REF; your ears should hear it about as far.
    gear_ref = float(re.search(r"GEAR_REACH_REF: f32 = ([\d.]+)",
                               (ROOT / "src" / "audio.rs").read_text()).group(1))
    gear = {}
    for c in mix:
        if not c["name"].startswith("gear."):
            continue
        heard = audible_range(c["name"], sets, mix, calm)
        gear[c["name"]] = dict(heard=heard, robots=gear_ref)
        if not GEAR_HEARD[0] <= heard / gear_ref <= GEAR_HEARD[1]:
            c["flags"].append(f"heard to {heard} m, robots to {gear_ref:g} m")
    # A drag (src/interact/carry.rs) at a full haul: robots hear it to
    # DRAG_HEARD; your ears should hear it about as far.
    drag_ref = float(re.search(r"DRAG_HEARD: f32 = ([\d.]+)",
                               (ROOT / "src" / "interact" / "carry.rs").read_text()).group(1))
    for c in mix:
        if not c["name"].startswith("drag."):
            continue
        heard = audible_range(c["name"], sets, mix, calm)
        gear[c["name"]] = dict(heard=heard, robots=drag_ref)
        if not GEAR_HEARD[0] <= heard / drag_ref <= GEAR_HEARD[1]:
            c["flags"].append(f"heard to {heard} m, robots to {drag_ref:g} m")
    # Startle: the loudest each sudden sound gets, up close.
    def up_close(c, extra=0.0):
        loud = max(f["momentary_max"] for f in sets[c["set"]]["files"]) + filter_loss(c, sets)
        near = falloff(c["dist"], c["reach"]) if c["role"] == "Event" else 0.0
        return loud + c["db"] + near + extra
    shot = next(c for c in mix if c["name"] == "gun.shot")
    startle = []
    for c in mix:
        if sets[c["set"]]["type"] != "oneshot":
            continue
        over = up_close(c) - up_close(shot)
        over_narrow = up_close(c, lift.get(c["role"], 0.0)) - up_close(shot)
        startle.append(dict(cue=c["name"], up_close=round(up_close(c), 1),
                            over_shot=round(over, 1), over_shot_narrow=round(over_narrow, 1)))
        if c["name"] != "gun.shot" and (over > STARTLE_OVER_SHOT
                                        or over_narrow > STARTLE_OVER_SHOT_NARROW):
            c["flags"].append(f"startles: {over:+.1f} / {over_narrow:+.1f} LU (narrow) "
                              "over your rifle up close")
    startle.sort(key=lambda r: -max(r["over_shot"], r["over_shot_narrow"]))
    # A loop that starts audibly switches on as you walk up to it.
    edges = {}
    for n, (_, keep) in loop_fades().items():
        m = audible_margin(n, sets, mix, calm, keep)
        edges[n] = dict(keep=keep, margin_db=round(m, 1))
        if m > 0:
            next(c for c in mix if c["name"] == n)["flags"].append(
                f"switches on at {keep:.0f} m, {m:+.1f} dB over the wind")
    report = dict(summary=summary, cues=mix, scenes=scenes, ranges=ranges, edges=edges, gear=gear,
                  startle=startle,
                  session=dict(integrated=session, lra=lra, shares=SESSION),
                  narrow=dict(integrated=n_session, lra=n_lra,
                              scenes={n: r["integrated"] for n, r in narrow_scenes.items()}))
    (out / "mix.json").write_text(json.dumps(report, indent=1))
    lines = [f"Mix line: heard = {summary['intercept']} + {summary['slope']} x SPL "
             f"(the world's dynamics compressed {1 / max(summary['slope'], 1e-3):.1f}:1)",
             f"Beds: {summary['beds']}; your gunshot heard at {summary['gunshot']} LUFS", ""]
    lines.append(f"{'cue':20s} {'role':7s} {'heard':>6s} {'line':>6s}  flags")
    for c in sorted(mix, key=lambda c: -abs(c["off_line"]) if c["flags"] else 0):
        lines.append(f"{c['name']:20s} {c['role']:7s} {c['heard']:6.1f} {c['line']:6.1f}  "
                     + "; ".join(c["flags"]))
    lines.append("")
    for name, r in scenes.items():
        lines.append(f"scene {name}: {r['integrated']} LUFS integrated, momentary max "
                     f"{r['momentary_max']}, peak {r['sample_peak']} dBFS"
                     + (f", {r['clipped']} samples over full scale" if r["clipped"] else ""))
        for p in r["probes"]:
            lines.append(f"    {p['cue']:16s} clears the rest by {p['margin_db']:+.1f} dB "
                         f"(at {p['band_hz']} Hz)")
    lo, hi = SESSION_TARGET[0] - SESSION_TARGET[1], SESSION_TARGET[0] + SESSION_TARGET[1]
    lines.append(f"a raid ({', '.join(f'{n} {w:.0%}' for n, w in SESSION.items())}): "
                 f"{session} LUFS integrated, loudness range {lra} LU "
                 f"(ASWG-R001 asks {lo:.0f}..{hi:.0f})")
    lines.append(f"  with narrow dynamics: {n_session} LUFS, loudness range {n_lra} LU; "
                 + ", ".join(f"{n} {r['integrated']}" for n, r in narrow_scenes.items()))
    lines.append("")
    lines.append("Audible range (m), over the wind / over a downpour:")
    for n, r in ranges.items():
        want = f"   (wants {r['want'][0]} / {r['want'][1]})" if r.get("want") else ""
        lines.append(f"    {n:20s} {r['calm']:4d} / {r['downpour']:4d}{want}")
    lines.append("")
    lines.append(f"Your kit, heard over the wind against robots' reach "
                 f"(wants {GEAR_HEARD[0]}..{GEAR_HEARD[1]}x):")
    for n, g in gear.items():
        lines.append(f"    {n:20s} {g['heard']:4d} m, robots {g['robots']:g} m")
    lines.append("")
    lines.append("Loudest up close (momentary LUFS; over your rifle, normal / narrow):")
    for r in startle[:10]:
        lines.append(f"    {r['cue']:20s} {r['up_close']:6.1f}   {r['over_shot']:+5.1f} / "
                     f"{r['over_shot_narrow']:+5.1f}")
    lines.append("")
    lines.append("Loops kept near you, where they start (over the wind):")
    for n, e in edges.items():
        lines.append(f"    {n:20s} at {e['keep']:.0f} m, {e['margin_db']:+.1f} dB")
    text = "\n".join(lines)
    (out / "mix.txt").write_text(text)
    print(text)
    off = abs(session - SESSION_TARGET[0]) > SESSION_TARGET[1]
    if off:
        print(f"a raid's loudness, {session} LUFS, is outside ASWG-R001's window")
    return sum(1 for c in mix if c["flags"]) + off
