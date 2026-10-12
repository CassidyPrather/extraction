#!/usr/bin/env python3
"""Synthesises the sounds of things that don't exist (the Surge, anomalies).

Recorded sounds cover the real world (see fetch_audio.py); these are the
exceptions the audio direction allows. Deterministic: rerunning produces
identical files. Writes Ogg Vorbis via ffmpeg into assets/audio/anomaly/
and prints the sounds.json entries to merge.

    python3 scripts/synth_anomaly.py
"""
import json
import pathlib
import subprocess
import tempfile
import wave

import numpy as np

SR = 48000
ROOT = pathlib.Path(__file__).resolve().parent.parent
OUT = ROOT / "assets" / "audio" / "anomaly"


def lowpass(x, cutoff):
    # One-pole, run forwards and backwards for zero phase.
    a = np.exp(-2 * np.pi * cutoff / SR)
    y = np.empty_like(x)
    acc = 0.0
    for i, v in enumerate(x):
        acc = (1 - a) * v + a * acc
        y[i] = acc
    acc = 0.0
    for i in range(len(y) - 1, -1, -1):
        acc = (1 - a) * y[i] + a * acc
        y[i] = acc
    return y


def loop_crossfade(x, fade):
    """Makes `x` loop seamlessly by folding its tail over its head."""
    n = int(fade * SR)
    head, body, tail = x[:n], x[n:-n], x[-n:]
    t = np.linspace(0, 1, n, endpoint=False)
    mixed = tail * np.cos(t * np.pi / 2) + head * np.sin(t * np.pi / 2)
    return np.concatenate([mixed, body, tail[:0]])[: len(x) - n]


def write(name, stereo, quality=4):
    OUT.mkdir(parents=True, exist_ok=True)
    peak = np.max(np.abs(stereo))
    stereo = stereo / peak * 0.89
    pcm = (stereo * 32767).astype("<i2")
    with tempfile.NamedTemporaryFile(suffix=".wav", delete=False) as f:
        tmp = f.name
    with wave.open(tmp, "wb") as w:
        w.setnchannels(2)
        w.setsampwidth(2)
        w.setframerate(SR)
        w.writeframes(pcm.T.copy().tobytes())
    path = OUT / f"{name}.ogg"
    subprocess.run(
        ["ffmpeg", "-y", "-loglevel", "error", "-i", tmp, "-c:a", "libvorbis",
         "-q:a", str(quality), "-fflags", "+bitexact", "-flags:a", "+bitexact",
         "-map_metadata", "-1", str(path)],
        check=True,
    )
    pathlib.Path(tmp).unlink()
    return path, stereo.shape[1] / SR


def surge_drone(seed=7, seconds=34.0):
    """A low, beating, breathing roar: sub sines a few Hz apart, filtered
    brown noise swelling slowly, a metallic overtone that wanders."""
    rng = np.random.default_rng(seed)
    t = np.arange(int(seconds * SR)) / SR
    out = []
    for ch in range(2):
        detune = 0.35 * ch
        sub = (np.sin(2 * np.pi * (37 + detune) * t)
               + 0.8 * np.sin(2 * np.pi * (41.5 - detune) * t)
               + 0.35 * np.sin(2 * np.pi * 74.3 * t + 0.6 * np.sin(2 * np.pi * 0.07 * t)))
        brown = np.cumsum(rng.normal(size=t.size))
        brown -= lowpass(brown, 2.0)
        rumble = lowpass(brown, 180.0)
        rumble /= np.max(np.abs(rumble))
        swell = 0.65 + 0.35 * np.sin(2 * np.pi * t / 8.5 + ch)
        whine = 0.08 * np.sin(2 * np.pi * (520 + 40 * np.sin(2 * np.pi * 0.05 * t)) * t)
        whine *= 0.5 + 0.5 * np.sin(2 * np.pi * 0.13 * t + 2 * ch)
        x = (0.55 * sub + 0.9 * rumble) * swell + whine
        out.append(loop_crossfade(x, 2.0))
    return np.stack(out)


def surge_crack(seed, seconds=3.5):
    """An electrical crack with a rolling, ringing tail."""
    rng = np.random.default_rng(seed)
    n = int(seconds * SR)
    t = np.arange(n) / SR
    out = []
    for ch in range(2):
        burst = rng.normal(size=n) * np.exp(-t * (38 + 6 * ch))
        # Crackle: sparse impulses through the first half second.
        crackle = (rng.random(n) < 0.004 * np.exp(-t * 5)) * rng.normal(size=n) * 3
        zap = np.sin(2 * np.pi * (900 * np.exp(-t * 7) + 60) * t) * np.exp(-t * 9)
        tail = lowpass(rng.normal(size=n), 260) * np.exp(-t * 1.6) * 6
        x = burst + crackle + 0.6 * zap + tail
        out.append(x)
    return np.stack(out)


def lift_hum(seed=11, seconds=16.0):
    """A slow, breathing whoosh with a low pressure throb."""
    rng = np.random.default_rng(seed)
    t = np.arange(int(seconds * SR)) / SR
    out = []
    for ch in range(2):
        n = rng.normal(size=t.size)
        air = lowpass(n, 900) - lowpass(n, 120)
        breath = 0.55 + 0.45 * np.sin(2 * np.pi * t / 4.0 + ch * 0.7) ** 2
        throb = 0.4 * np.sin(2 * np.pi * 29 * t) * (0.6 + 0.4 * np.sin(2 * np.pi * t / 2.0))
        out.append(loop_crossfade(air * breath * 3.0 + throb, 1.0))
    return np.stack(out)


def burner_roar(seed=12, seconds=12.0):
    """A low furnace roar with pops."""
    rng = np.random.default_rng(seed)
    t = np.arange(int(seconds * SR)) / SR
    out = []
    for ch in range(2):
        n = rng.normal(size=t.size)
        roar = lowpass(n, 350) * (0.8 + 0.2 * np.sin(2 * np.pi * 0.3 * t + ch))
        pops = (rng.random(t.size) < 0.0006) * rng.normal(size=t.size) * 4
        pops = pops - lowpass(pops, 1500)
        out.append(loop_crossfade(roar * 4.0 + pops, 1.0))
    return np.stack(out)


def snare_buzz(seed=13, seconds=10.0):
    """Mains-hum buzz with dry crackles, like a fault in a substation."""
    rng = np.random.default_rng(seed)
    t = np.arange(int(seconds * SR)) / SR
    out = []
    for ch in range(2):
        hum = sum(np.sin(2 * np.pi * 50 * k * t) / k for k in (1, 2, 3, 5, 7)) * 0.3
        hum *= 0.7 + 0.3 * np.sin(2 * np.pi * 0.21 * t + ch)
        crack = (rng.random(t.size) < 0.002) * rng.normal(size=t.size) * 5
        crack = crack - lowpass(crack, 2500)
        out.append(loop_crossfade(hum + crack, 0.5))
    return np.stack(out)


def lift_pop(seed=14, seconds=2.5):
    """A deep whump as a lift throws everything out."""
    rng = np.random.default_rng(seed)
    t = np.arange(int(seconds * SR)) / SR
    out = []
    for ch in range(2):
        f = 120 * np.exp(-t * 3) + 30
        thump = np.sin(2 * np.pi * np.cumsum(f) / SR) * np.exp(-t * 3.5)
        gust = lowpass(rng.normal(size=t.size), 600) * np.exp(-t * 2.2) * 5
        out.append(thump + gust)
    return np.stack(out)


def loopset(entries, name, data, kind="loop"):
    path, secs = write(name, data)
    entries[f"anomaly/{name}"] = {
        "type": kind, "channels": 2, "loop": kind == "loop",
        "files": [{"file": str(path.relative_to(ROOT / "assets")), "seconds": round(secs, 3),
                   "source": "synth:scripts/synth_anomaly.py"}],
    }


def main():
    entries = {}
    loopset(entries, "lift_hum", lift_hum())
    loopset(entries, "burner_roar", burner_roar())
    loopset(entries, "snare_buzz", snare_buzz())
    loopset(entries, "lift_pop", lift_pop(), kind="oneshot")
    path, secs = write("surge_drone", surge_drone())
    entries["anomaly/surge_drone"] = {
        "type": "bed", "channels": 2, "loop": True,
        "files": [{"file": str(path.relative_to(ROOT / "assets")), "seconds": round(secs, 3),
                   "source": "synth:scripts/synth_anomaly.py"}],
    }
    files = []
    for i in range(3):
        path, secs = write(f"surge_crack_{i + 1}", surge_crack(100 + i))
        files.append({"file": str(path.relative_to(ROOT / "assets")), "seconds": round(secs, 3),
                      "source": "synth:scripts/synth_anomaly.py"})
    entries["anomaly/surge_crack"] = {"type": "oneshot", "channels": 2, "loop": False, "files": files}

    sounds = ROOT / "assets" / "audio" / "sounds.json"
    data = json.loads(sounds.read_text())
    data.update(entries)
    sounds.write_text(json.dumps(dict(sorted(data.items())), indent=2) + "\n")
    print("wrote", ", ".join(entries))


if __name__ == "__main__":
    main()
