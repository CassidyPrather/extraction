"""The music pass of scripts/audio_audit.py: the synthesised music and
broadcasts (src/radio, extraction_core::music) against the mix.

`cargo test radio::sound::tests::renders_for_the_audit` renders each
synthesised set the mix table names (`music/tape`, `music/station`, ...)
as the game plays it, before the cue's gain, to
target/audio_audit/music/<set>.wav; `add_sets` runs it when the renders
are missing or older than the code that makes them, measures them like
the asset pass measures files, and adds them to the audit's sets so the
mix pass judges `tape.music`, `radio.station` and the rest like any cue:
their level against the handling line, their audible range, and what
they mask.

It also checks, for itself:

- **Music doesn't drown a robot near you.** A tape playing in your hand
  (0.6 m) is no more than `TAPE_OVER_STEP` LU over a walker's step at
  10 m. A lure you carry must not make you deaf.
- **The radio sits under the chatter.** A station at full is at least
  `CHATTER_OVER` LU under the robots' chatter on the same set: their
  traffic breaks in over whatever you're listening to.
"""
import subprocess
from pathlib import Path

import numpy as np

import audio_audit as aa

ROOT = Path(__file__).resolve().parent.parent
SETS = ["music/tape", "music/motor", "music/station", "music/static",
        "music/gramophone", "music/piano", "music/organ"]
LOOPS = {"music/motor", "music/static", "music/station"}
SOURCES = [ROOT / "src" / "radio", ROOT / "crates" / "extraction_core" / "src" / "music"]
TAPE_OVER_STEP = 12.0
CHATTER_OVER = 3.0


def renders_dir(out):
    return out / "music"


def stamp(paths):
    return max(p.stat().st_mtime for d in paths for p in d.rglob("*.rs"))


def add_sets(sets, out):
    """Render (if stale) and measure the music into `sets`."""
    d = renders_dir(out)
    wavs = sorted(d.glob("*.wav")) if d.exists() else []
    if not wavs or min(p.stat().st_mtime for p in wavs) < stamp(SOURCES):
        print("music: rendering (cargo test radio::sound::tests::renders_for_the_audit)")
        subprocess.run(["cargo", "test", "--profile", "fast", "--lib", "--",
                        "radio::sound::tests::renders_for_the_audit"],
                       cwd=ROOT, check=True, capture_output=True)
    for name in SETS:
        p = d / (name.replace("/", "_") + ".wav")
        if not p.exists():
            continue
        kind = "loop" if name in LOOPS else "bed"
        f = aa.measure(p, kind)
        sets[name] = {
            "type": kind,
            "variants": 1,
            "loudness": f["integrated"],
            "spread": 0.0,
            "true_peak": f["true_peak"],
            "crest": f["crest"],
            "onset_ms": f["onset_ms"],
            "clipped_runs": f["clipped_runs"],
            "dc": abs(f["dc"]),
            "channels": [f["channels"]],
            "bands": f["bands"],
            "files": [f],
        }


def run(sets, out):
    """The music's own checks; returns the flagged lines."""
    import audio_mix as am
    flags = []
    cues = {c["name"]: c for c in am.read_mix()}
    lines = ["", "Music (src/radio)"]

    def heard(cue, dist):
        c = cues[cue]
        s = sets.get(c["set"])
        if s is None:
            return None
        return s["loudness"] + c["db"] + am.falloff(dist, c["reach"])

    tape = heard("tape.music", 0.6)
    step = heard("walker.step", 10.0) if "walker.step" in cues else None
    if tape is not None and step is not None:
        over = tape - step
        lines.append(f"  a tape in hand over a walker's step at 10 m: {over:+.1f} LU")
        if over > TAPE_OVER_STEP:
            flags.append(f"tape.music {over:+.1f} LU over a walker's step at 10 m")
    station = heard("radio.station", 0.6)
    chatter = heard("chatter.voice", 0.6)
    if station is not None and chatter is not None:
        gap = chatter - station
        lines.append(f"  chatter over a station at full: {gap:+.1f} LU")
        if gap < CHATTER_OVER:
            flags.append(f"radio.station within {CHATTER_OVER} LU of chatter.voice ({gap:+.1f})")
    for f in flags:
        lines.append("  FLAG " + f)
    (out / "music.txt").write_text("\n".join(lines) + "\n")
    print("\n".join(lines))
    return flags
