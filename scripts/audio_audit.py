#!/usr/bin/env python3
"""Measures the game's sound, so the mix can be judged by numbers as well as ears.

    python3 scripts/audio_audit.py                 # assets + mix + scenes -> target/audio_audit/
    python3 scripts/audio_audit.py --assets-only   # just the per-file measurements

Three passes (see docs/AUDIO.md, "Checking the mix"):

1. Assets. Every file in assets/audio/sounds.json: loudness by ITU-R
   BS.1770 (K-weighted; momentary-max over 400 ms for one-shots, gated
   integrated for loops), true peak (4x oversampled), crest factor, DC
   offset, onset delay, clipped runs, loop seams, and a 1/3-octave
   spectrum. Each set is summarised, and variants that jump out of their
   set are flagged.
2. Mix. The playback gains the game uses (src/audio/mix.rs, the `MIX`
   table) applied to the assets: each cue's loudness as heard at its
   reference distance, against a target from its real-world sound level
   compressed into the game's range, and against its priority (cues that
   carry information must clear the ambience beds).
3. Scenes. Mixes rendered offline from the assets as the game would play
   them (gains, distances, the inverse-distance falloff), measured for
   loudness, peak and masking: can the drone's tell be heard over heavy
   rain and a gunfight?

Needs python3 with numpy, and ffmpeg (to decode).
"""

import argparse
import json
import math
import re
import subprocess
import sys
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parent.parent
AUDIO = ROOT / "assets"
SR = 48000

# ---------------------------------------------------------------- decoding


def decode(path):
    """Float32 samples, shape (n, channels), at 48 kHz."""
    out = subprocess.run(
        ["ffmpeg", "-v", "error", "-i", str(path), "-f", "f32le", "-ar", str(SR), "-"],
        capture_output=True,
        check=True,
    ).stdout
    probe = subprocess.run(
        ["ffprobe", "-v", "error", "-select_streams", "a:0", "-show_entries",
         "stream=channels", "-of", "csv=p=0", str(path)],
        capture_output=True, text=True, check=True,
    ).stdout.strip()
    ch = int(probe or 1)
    x = np.frombuffer(out, dtype=np.float32)
    return x.reshape(-1, ch).astype(np.float64)


# ------------------------------------------------------- BS.1770 weighting

# The K-weighting pre-filter at 48 kHz (ITU-R BS.1770-4, table 1 and 2):
# a high shelf (head acoustics) and a high-pass (RLB).
_SHELF = ([1.53512485958697, -2.69169618940638, 1.19839281085285],
          [1.0, -1.69065929318241, 0.73248077421585])
_RLB = ([1.0, -2.0, 1.0], [1.0, -1.99004745483398, 0.99007225036621])


def _biquad_mag2(b, a, f):
    z = np.exp(-2j * np.pi * f / SR)
    num = b[0] + b[1] * z + b[2] * z * z
    den = a[0] + a[1] * z + a[2] * z * z
    return np.abs(num / den) ** 2


def k_weight(x):
    """K-weighted signal. Loudness only needs the energy, so the filter's
    magnitude is applied in the frequency domain (zero-phase)."""
    n = len(x)
    pad = int(2 ** math.ceil(math.log2(n + SR // 2)))
    f = np.fft.rfftfreq(pad, 1 / SR)
    g = np.sqrt(_biquad_mag2(*_SHELF, f) * _biquad_mag2(*_RLB, f))
    X = np.fft.rfft(x, pad, axis=0)
    return np.fft.irfft(X * g[:, None], pad, axis=0)[:n]


def _lkfs(ms):
    return -0.691 + 10 * np.log10(np.maximum(ms, 1e-12))


def block_loudness(y, block=0.4, hop=0.1):
    """Loudness of each `block`-second window every `hop` seconds of a
    K-weighted signal (summing channels' mean squares, weights 1). Clips
    shorter than a block are measured with the silence a meter would see."""
    w, h = int(block * SR), int(hop * SR)
    if len(y) < w:
        y = np.vstack([y, np.zeros((w - len(y), y.shape[1]))])
    e = np.cumsum(np.vstack([np.zeros((1, y.shape[1])), y * y]), axis=0)
    starts = np.arange(0, len(y) - w + 1, h)
    ms = (e[starts + w] - e[starts]).sum(axis=1) / w
    return _lkfs(ms)


def integrated(y):
    """Gated integrated loudness (BS.1770: absolute -70, relative -10)."""
    blocks = block_loudness(y, 0.4, 0.1)
    blocks = blocks[blocks > -70]
    if len(blocks) == 0:
        return -70.0
    mean = lambda b: _lkfs(np.mean(10 ** ((b + 0.691) / 10)))
    rel = mean(blocks) - 10
    gated = blocks[blocks > rel]
    return float(mean(gated)) if len(gated) else float(mean(blocks))


def true_peak(x):
    """dBTP: 4x oversampled by band-limited interpolation."""
    n = len(x)
    X = np.fft.rfft(x, axis=0)
    up = np.fft.irfft(X, 4 * n, axis=0) * 4
    return 20 * np.log10(max(np.abs(up).max(), 1e-9))


THIRDS = [25, 31.5, 40, 50, 63, 80, 100, 125, 160, 200, 250, 315, 400, 500, 630, 800, 1000,
          1250, 1600, 2000, 2500, 3150, 4000, 5000, 6300, 8000, 10000, 12500, 16000, 20000]


def third_octaves(x):
    """Band levels (dB, relative) of a mono signal in 1/3-octave bands."""
    n = len(x)
    if n == 0:
        return [-120.0] * len(THIRDS)
    P = np.abs(np.fft.rfft(x * np.hanning(n))) ** 2 / n
    f = np.fft.rfftfreq(n, 1 / SR)
    out = []
    for fc in THIRDS:
        lo, hi = fc / 2 ** (1 / 6), fc * 2 ** (1 / 6)
        out.append(10 * np.log10(P[(f >= lo) & (f < hi)].sum() + 1e-12))
    return out


# ----------------------------------------------------------- asset pass


def measure(path, kind):
    x = decode(path)
    mono = x.mean(axis=1)
    y = k_weight(x)
    m = block_loudness(y, 0.4, 0.1)
    peak = float(np.abs(x).max())
    i_loud = m.argmax()
    lo = int(i_loud * 0.1 * SR)
    window = mono[lo:lo + int(0.4 * SR)]
    rms_window = math.sqrt(float(np.mean(window ** 2))) if len(window) else 0.0
    onset_at = np.argmax(np.abs(mono) > peak * 0.1) / SR if peak > 0 else 0.0
    clipped = int(np.sum(np.convolve(np.abs(mono) >= 0.998, np.ones(3), "valid") >= 3))
    r = {
        "file": str(path.relative_to(AUDIO)) if path.is_relative_to(AUDIO) else str(path),
        "seconds": round(len(x) / SR, 3),
        "channels": x.shape[1],
        "momentary_max": round(float(m.max()), 1),
        "short_term_max": round(float(block_loudness(y, 3.0, 0.1).max()), 1) if len(x) > 3 * SR else None,
        "integrated": round(integrated(y), 1),
        "sample_peak": round(20 * math.log10(max(peak, 1e-9)), 1),
        "true_peak": round(true_peak(x), 1),
        "crest": round(20 * math.log10(max(peak, 1e-9) / max(rms_window, 1e-9)), 1),
        "dc": round(float(np.mean(mono)), 5),
        "onset_ms": round(onset_at * 1000, 1),
        "clipped_runs": clipped,
        "bands": [round(b, 1) for b in third_octaves(window)],
    }
    if kind == "loop":
        # The seam: the last 20 ms against the first 20 ms, and the jump
        # between the last sample and the first, in the local RMS.
        k = int(0.02 * SR)
        a, b = mono[-k:], mono[:k]
        rms = lambda s: math.sqrt(float(np.mean(s ** 2))) + 1e-9
        r["seam_level_db"] = round(20 * math.log10(rms(a) / rms(b)), 1)
        r["seam_jump"] = round(abs(float(mono[-1] - mono[0])) / rms(np.concatenate([a, b])), 2)
    return r


def asset_pass(bank, out):
    sets = {}
    for name, s in sorted(bank.items()):
        kind = s.get("type", "oneshot")
        files = [measure(AUDIO / f["file"], kind) for f in s["files"]]
        loud = [f["integrated"] if kind in ("loop", "bed") else f["momentary_max"] for f in files]
        sets[name] = {
            "type": kind,
            "variants": len(files),
            "loudness": round(float(np.mean(loud)), 1),
            "spread": round(float(max(loud) - min(loud)), 1),
            "true_peak": max(f["true_peak"] for f in files),
            "crest": round(float(np.mean([f["crest"] for f in files])), 1),
            "onset_ms": max(f["onset_ms"] for f in files),
            "clipped_runs": sum(f["clipped_runs"] for f in files),
            "dc": max(abs(f["dc"]) for f in files),
            "channels": sorted({f["channels"] for f in files}),
            "bands": [round(float(v), 1) for v in np.mean([f["bands"] for f in files], axis=0)],
            "files": files,
        }
        if kind == "loop":
            sets[name]["seam_jump"] = max(f.get("seam_jump", 0) for f in files)
            sets[name]["seam_level_db"] = max(abs(f.get("seam_level_db", 0)) for f in files)
        print(f"  {name:32s} {kind:7s} {sets[name]['loudness']:6.1f} LUFS  "
              f"spread {sets[name]['spread']:4.1f}  TP {sets[name]['true_peak']:5.1f}  "
              f"crest {sets[name]['crest']:4.1f}", file=sys.stderr)
    (out / "assets.json").write_text(json.dumps(sets, indent=1))
    return sets


# ----------------------------------------------------------------- main


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--assets-only", action="store_true")
    ap.add_argument("--fresh", action="store_true", help="re-measure the assets")
    ap.add_argument("--check", action="store_true",
                    help="exit 1 if any cue is flagged (a gauge for the mix)")
    ap.add_argument("--out", default=str(ROOT / "target" / "audio_audit"))
    args = ap.parse_args()
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    bank = json.loads((AUDIO / "audio" / "sounds.json").read_text())
    cache = out / "assets.json"
    newest = max(p.stat().st_mtime for p in (AUDIO / "audio").rglob("*.ogg"))
    if cache.exists() and cache.stat().st_mtime > newest and not args.fresh:
        sets = json.loads(cache.read_text())
    else:
        sets = asset_pass(bank, out)
    if args.assets_only:
        return
    import audio_voices  # noqa: E402  (what the robots say)
    audio_voices.add_sets(sets, out)
    import audio_mix  # noqa: E402  (the mix and scene passes)
    flagged = audio_mix.run(sets, out)
    import audio_acoustics  # noqa: E402  (what the rooms do)
    room_flags, lines = audio_acoustics.run(sets, out)
    if lines:
        text = "\n".join(lines)
        print("\n" + text)
        with open(out / "mix.txt", "a") as f:
            f.write("\n\n" + text + "\n")
    flagged += room_flags
    import audio_binaural  # noqa: E402  (where the headset puts sounds)
    ear_flags, ear_lines = audio_binaural.run(out)
    if ear_lines:
        text = "\n".join(ear_lines)
        print("\n" + text)
        with open(out / "mix.txt", "a") as f:
            f.write("\n" + text + "\n")
    flagged += ear_flags
    import audio_score  # noqa: E402  (the score against the world)
    score_flags, score_lines = audio_score.run(sets, out)
    if score_lines:
        text = "\n".join(score_lines)
        print("\n" + text)
        with open(out / "mix.txt", "a") as f:
            f.write("\n" + text + "\n")
    flagged += score_flags
    voice_flags, voice_lines = audio_voices.run(sets, out)
    if voice_lines:
        text = "\n".join(voice_lines)
        print("\n" + text)
        with open(out / "mix.txt", "a") as f:
            f.write("\n" + text + "\n")
    flagged += voice_flags
    if args.check and flagged:
        sys.exit(f"{flagged} cue(s) flagged: see {out / 'mix.txt'}")


if __name__ == "__main__":
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    main()
