"""The voices pass of scripts/audio_audit.py: what the robots say
(src/robot_voice.rs) against the mix.

`cargo test robot_voice::tests::renders_for_the_audit` renders every
utterance of every kind, levelled as the game levels them, to
target/audio_audit/voices/<kind>_<mind>_<variant>.wav; `add_sets` runs it
when the renders are missing or older than the synth, and adds them to
the audit's sets as `voice/<kind>/<mind>` (the mix table's `voice.*` cues
name the walker's; the other kinds play at the cue's gain plus their
`voice_trim`). This checks:

- **The kinds balance.** For each mind, every kind's heard loudness (with
  its trim) is within `KIND_SPREAD` LU of the others: a hound's warning
  isn't a whisper beside a walker's.
- **They don't hide footsteps or gunshots.** With the loudest of each
  kind's words close by (`NEAR` m, every word in turn), your own footstep
  over the wind and your rifle still clear the masking of everything else
  by the masking model.
- **The listening log agrees.** `robot_voice::heard_to`, which the
  headless listening scenario judges each word by, is within
  `HEARD_DRIFT` of each word's audible range in calm measured here.
- **The night doesn't hide them.** Over the night's beds (the wind,
  the insects at full, the leaves), each word keeps `NIGHT_KEPT` of its
  range in calm, and your own footstep still clears the masking. A
  walker's step and a drone's rotor are warned of to the mix's own
  target (`RANGE_TARGETS`): by the sound itself, or, where the crickets
  would hide it, by the crickets falling silent (`INSECTS_HUSH_*`).
- **Calls carry.** `voice.calling` and `voice.answering` are heard to
  `RANGE_TARGETS` (200 m in calm) by the mix pass, like any signal: a call
  answered that far off is the warning that a response team is coming.
"""
import re
import subprocess
from pathlib import Path

import numpy as np

import audio_audit as aa
import audio_mix as am

ROOT = Path(__file__).resolve().parent.parent
KINDS = ["drone", "walker", "grappler", "dirigible", "crawler", "hound", "warden"]
MINDS = ["idle", "noticed", "investigating", "searching", "engaging", "lost",
         "calling", "answering", "hurt", "dying"]
KIND_SPREAD = 6.0   # LU between kinds saying the same thing
NEAR = 6.0          # m: a robot this close, talking over your footsteps
HEARD_DRIFT = 0.25  # robot_voice::heard_to against the measured range
NIGHT_KEPT = 0.75   # share of a warning's calm range kept over the night beds
NIGHT_BED = [("wind", 0.0), ("insects", 0.0), ("rustle", 0.0)]


def renders_dir(out):
    return out / "voices"


def add_sets(sets, out):
    """Render (if stale) and measure the voices into `sets`."""
    d = renders_dir(out)
    synth = ROOT / "src" / "robot_voice.rs"
    wavs = sorted(d.glob("*.wav")) if d.exists() else []
    if not wavs or min(p.stat().st_mtime for p in wavs) < synth.stat().st_mtime:
        print("voices: rendering (cargo test robot_voice::tests::renders_for_the_audit)")
        subprocess.run(["cargo", "test", "--profile", "fast", "--lib", "--",
                        "robot_voice::tests::renders_for_the_audit"],
                       cwd=ROOT, check=True, capture_output=True)
        wavs = sorted(d.glob("*.wav"))
    for k in KINDS:
        for m in MINDS:
            files = [aa.measure(p, "oneshot") for p in sorted(d.glob(f"{k}_{m}_*.wav"))]
            if not files:
                continue
            loud = [f["momentary_max"] for f in files]
            sets[f"voice/{k}/{m}"] = {
                "type": "oneshot",
                "variants": len(files),
                "loudness": round(float(np.mean(loud)), 1),
                "spread": round(float(max(loud) - min(loud)), 1),
                "true_peak": max(f["true_peak"] for f in files),
                "crest": round(float(np.mean([f["crest"] for f in files])), 1),
                "onset_ms": max(f["onset_ms"] for f in files),
                "clipped_runs": sum(f["clipped_runs"] for f in files),
                "dc": max(abs(f["dc"]) for f in files),
                "channels": [1],
                "bands": [round(float(v), 1)
                          for v in np.mean([f["bands"] for f in files], axis=0)],
                "files": files,
            }


def trims():
    """`voice_trim` from src/audio.rs, dB by kind."""
    text = (ROOT / "src" / "audio.rs").read_text()
    body = text[text.index("pub fn voice_trim"):]
    body = body[:body.index("\n}\n")]
    out = {k: 0.0 for k in KINDS}
    for names, db in re.findall(r"((?:K::\w+\s*\|?\s*)+)=>\s*(-?[\d.]+)", body):
        for n in re.findall(r"K::(\w+)", names):
            out[n.lower()] = float(db)
    return out


def hush_radii():
    """`INSECTS_HUSH_*` from src/audio.rs, m."""
    text = (ROOT / "src" / "audio.rs").read_text()
    return {k: float(v) for k, v in re.findall(r"INSECTS_HUSH_(\w+): f32 = ([\d.]+)", text)}


def heard_to():
    """`robot_voice::heard_to`, m by mind."""
    text = (ROOT / "src" / "robot_voice.rs").read_text()
    body = text[text.index("pub fn heard_to"):]
    body = body[:body.index("\n}\n")]
    out = {}
    for names, m in re.findall(r"((?:Utterance::\w+\s*\|?\s*)+)=>\s*([\d.]+)", body):
        for n in re.findall(r"Utterance::(\w+)", names):
            out[n.lower()] = float(m)
    return out


def run(sets, out):
    if "voice/walker/calling" not in sets:
        return 0, ["Voices: no renders; skipped"]
    mix = am.read_mix()
    trim = trims()
    flags = 0
    lines = ["Voices (src/robot_voice.rs): heard loudness by kind, LUFS momentary max "
             "at the cue's distance (spread wants <= %.0f LU)" % KIND_SPREAD,
             "    %-14s" % "mind" + "".join(f"{k:>10s}" for k in KINDS) + "  spread  flags"]
    for m in MINDS:
        cue = next(c for c in mix if c["name"] == f"voice.{m}")
        row = []
        for k in KINDS:
            s = sets[f"voice/{k}/{m}"]
            row.append(s["loudness"] + cue["db"] + trim[k] + am.falloff(cue["dist"], cue["reach"]))
        spread = max(row) - min(row)
        f = []
        if spread > KIND_SPREAD:
            lo, hi = KINDS[int(np.argmin(row))], KINDS[int(np.argmax(row))]
            f.append(f"{hi} {spread:.1f} LU over {lo}")
            flags += 1
        lines.append(f"    {m:14s}" + "".join(f"{v:10.1f}" for v in row)
                     + f"  {spread:6.1f}  {'; '.join(f)}")
    # The listening scenario's ranges against the measured ones.
    table = heard_to()
    lines.append(f"  Heard to in calm (m): measured / robot_voice::heard_to "
                 f"(within {HEARD_DRIFT:.0%}):")
    row = []
    for m in MINDS:
        got = am.audible_range(f"voice.{m}", sets, mix, [("wind", 0.0)])
        want = table.get(m, 0.0)
        bad = not want or abs(got - want) > HEARD_DRIFT * want
        flags += bad
        row.append(f"{m} {got}/{want:g}" + (" (!)" if bad else ""))
    lines.append("    " + ", ".join(row))
    # The night's beds against the warnings and your footstep.
    if all(f"ambience/{b}" in sets for b in ("insects_night", "rustle_leaves")):
        calm = [("wind", 0.0)]
        row = []
        for name in [f"voice.{m}" for m in MINDS]:
            day = am.audible_range(name, sets, mix, calm)
            night = am.audible_range(name, sets, mix, NIGHT_BED)
            bad = day and night < NIGHT_KEPT * day
            flags += bool(bad)
            row.append(f"{name} {night}/{day}" + (" (!)" if bad else ""))
        # A machine's own sound against the crickets: where they'd hide it,
        # they've already gone quiet (the hush), so the warning reaches
        # the further of the two, and must reach the mix's own target.
        hush = hush_radii()
        for name, kind in (("walker.step", "BIG"), ("drone.rotor", "DRONE")):
            night = am.audible_range(name, sets, mix, NIGHT_BED)
            want = am.RANGE_TARGETS[name][0]
            reach = max(night, hush[kind])
            bad = reach < want
            flags += bad
            row.append(f"{name} {night} (hush {hush[kind]:g}) wants {want}" + (" (!)" if bad else ""))
        step = am.audible_margin("step", sets, mix, NIGHT_BED, 1.7)
        flags += step <= 0
        lines.append(f"  At night (wind, insects, leaves), heard to (m) night/calm, "
                     f"keeps {NIGHT_KEPT:.0%}:")
        for i in range(0, len(row), 4):
            lines.append("    " + ", ".join(row[i:i + 4]))
        lines.append(f"    your footstep clears the night by {step:+.1f} dB"
                     + ("" if step > 0 else "  (masked)"))
    # Masking: each kind's every word at NEAR m, against your footstep
    # (over the wind) and your shot.
    worst = {}
    for target, every in (("step", 0.5), ("gun.shot", None)):
        for k in KINDS:
            for m in MINDS:
                talk = dict(next(c for c in mix if c["name"] == f"voice.{m}"),
                            name=f"_voice", set=f"voice/{k}/{m}",
                            db=next(c for c in mix if c["name"] == f"voice.{m}")["db"] + trim[k])
                mix2 = [c for c in mix if c["name"] != "_voice"] + [talk]
                scene = [("wind", "loop", None, 0.0, None),
                         ("_voice", 0.5, NEAR, 0.0, 0.6),
                         (target, 0.5, None, 0.0, every)]
                total, parts = am.render(scene, sets, mix2, seconds=3.0)
                sig = parts[f"2:{target}"]
                margin, band = am.margin(sig, total - sig, 0.5, 0.4)
                if target not in worst or margin < worst[target][0]:
                    worst[target] = (margin, k, m, band)
    lines.append(f"  With a robot talking {NEAR:.0f} m off (its every word, every kind), "
                 "the worst case still clears its masking by:")
    for target, (margin, k, m, band) in worst.items():
        f = "" if margin > 0 else "  masked"
        if margin <= 0:
            flags += 1
        lines.append(f"    {target:10s} {margin:+5.1f} dB at {band} Hz (the {k} {m}){f}")
    return flags, lines
