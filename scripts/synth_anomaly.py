#!/usr/bin/env python3
"""Synthesises the sounds of things that don't exist (the Surge, anomalies,
robot voices).

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


def write(name, stereo, quality=4, out=OUT):
    out.mkdir(parents=True, exist_ok=True)
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
    path = out / f"{name}.ogg"
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


def bowl(seed=15, seconds=7.0):
    """A struck singing bowl, far too deep: the offering's answer."""
    t = np.arange(int(seconds * SR)) / SR
    out = []
    for ch in range(2):
        x = np.zeros_like(t)
        for k, (f, a, d) in enumerate([(92, 1.0, 0.5), (251, 0.6, 0.8), (473, 0.35, 1.2), (761, 0.2, 1.8)]):
            beat = 1 + 0.004 * np.sin(2 * np.pi * (0.7 + 0.3 * ch) * t)
            x += a * np.sin(2 * np.pi * f * beat * t + k) * np.exp(-t * d)
        out.append(x * (1 - np.exp(-t * 60)))
    return np.stack(out)


def loopset(entries, name, data, kind="loop"):
    path, secs = write(name, data)
    entries[f"anomaly/{name}"] = {
        "type": kind, "channels": 2, "loop": kind == "loop",
        "files": [{"file": str(path.relative_to(ROOT / "assets")), "seconds": round(secs, 3),
                   "source": "synth:scripts/synth_anomaly.py"}],
    }


def attention_ring(seed=23, seconds=12.0):
    """Being watched: a thin ringing in the ears that beats against itself,
    and a slow pressure in the jaw, like teeth aching."""
    rng = np.random.default_rng(seed)
    t = np.arange(int(seconds * SR)) / SR
    out = []
    for ch in range(2):
        f = 2350 + 4.5 * ch
        ring = (np.sin(2 * np.pi * f * t) + 0.7 * np.sin(2 * np.pi * (f + 3.0) * t))
        ring *= 0.5 + 0.5 * np.sin(2 * np.pi * t / 6.0 + ch) ** 2
        pressure = np.sin(2 * np.pi * 48 * t) * (0.5 + 0.5 * np.sin(2 * np.pi * t / 3.0) ** 4)
        n = rng.normal(size=t.size)
        air = lowpass(n, 3000) - lowpass(n, 1200)
        x = 0.06 * ring + 0.7 * pressure + 0.25 * air / np.max(np.abs(air))
        out.append(loop_crossfade(x, 1.5))
    return np.stack(out)


def drone_lock(seed, seconds=0.85):
    """A drone settling its gun on you: three quick electronic chirps,
    rising, the last held — a sound that means *move*."""
    rng = np.random.default_rng(seed)
    n = int(seconds * SR)
    t = np.arange(n) / SR
    base = 1250 + 60 * (seed % 3)
    x = np.zeros(n)
    for k, (start, length) in enumerate([(0.0, 0.07), (0.16, 0.07), (0.32, 0.45)]):
        a, b = int(start * SR), int((start + length) * SR)
        tt = t[a:b] - start
        f = base * (1.0 + 0.18 * k) * (1.0 + 0.04 * tt / length)
        env = np.minimum(1.0, tt / 0.006) * np.minimum(1.0, (length - tt) / 0.02)
        tone = np.sign(np.sin(2 * np.pi * np.cumsum(f) / SR)) * 0.35 + np.sin(2 * np.pi * np.cumsum(f) / SR)
        x[a:b] += tone * env
    x = lowpass(x, 5500) + 0.01 * rng.normal(size=n)
    return np.stack([x, x])


def well_drone(seed=31, seconds=14.0):
    """A gravity knot: a pressure in the ears, sub tones a few Hz apart
    beating slowly, and air being drawn in, in long breaths."""
    rng = np.random.default_rng(seed)
    t = np.arange(int(seconds * SR)) / SR
    out = []
    for ch in range(2):
        sub = (np.sin(2 * np.pi * 27 * t) + 0.8 * np.sin(2 * np.pi * (29.5 + 0.3 * ch) * t)
               + 0.4 * np.sin(2 * np.pi * 41 * t))
        n = rng.normal(size=t.size)
        draw = lowpass(n, 260) - lowpass(n, 60)
        # Breathing in: swells that rise slowly and stop short.
        phase = (t / 3.5 + 0.3 * ch) % 1.0
        breath = phase ** 2 * (1.0 - np.clip((phase - 0.92) / 0.08, 0, 1))
        x = 0.8 * sub * (0.7 + 0.3 * np.sin(2 * np.pi * t / 7.0)) + 2.5 * draw * breath
        out.append(loop_crossfade(x, 1.2))
    return np.stack(out)


def mirror_shimmer(seed=32, seconds=10.0):
    """A fold in space: a glassy shimmer of high partials drifting against
    each other, with a fine sparkle."""
    rng = np.random.default_rng(seed)
    t = np.arange(int(seconds * SR)) / SR
    out = []
    for ch in range(2):
        x = np.zeros_like(t)
        for k, f in enumerate([1870.0, 2810.0, 3730.0, 4690.0, 5600.0]):
            drift = 1.0 + 0.004 * np.sin(2 * np.pi * t / (3.0 + k) + ch + k)
            amp = 0.5 + 0.5 * np.sin(2 * np.pi * t / (2.2 + 0.7 * k) + 1.3 * k + ch) ** 2
            x += amp * np.sin(2 * np.pi * np.cumsum(f * drift) / SR) / (1 + k)
        n = rng.normal(size=t.size)
        sparkle = (n - lowpass(n, 6000)) * (rng.random(t.size) < 0.002)
        out.append(loop_crossfade(0.35 * x + 2.0 * lowpass(sparkle, 9000), 1.0))
    return np.stack(out)


def mirror_blink(seed, seconds=0.7):
    """Something blinked across a few metres: air rushing in backwards,
    then a hard glassy pop."""
    rng = np.random.default_rng(seed)
    n = int(seconds * SR)
    t = np.arange(n) / SR
    noise = rng.normal(size=n)
    rush = lowpass(noise, 4000) - lowpass(noise, 400)
    swell = np.clip(t / 0.28, 0, 1) ** 3 * (t < 0.28)
    pop_t = np.clip(t - 0.28, 0, None)
    pop = (np.sin(2 * np.pi * (3200 * pop_t - 2200 * pop_t ** 2)) * np.exp(-pop_t / 0.03)
           * (t >= 0.28))
    thump = np.sin(2 * np.pi * 70 * pop_t) * np.exp(-pop_t / 0.08) * (t >= 0.28)
    x = 1.2 * rush * swell + pop + 0.8 * thump
    return np.stack([x, x])


def reveal(seed, seconds=1.2):
    """A thrown thing touching an anomaly: a bright, falling glassy ping."""
    n = int(seconds * SR)
    t = np.arange(n) / SR
    f = 2600 + 300 * seed
    x = (np.sin(2 * np.pi * np.cumsum(f * (1 - 0.15 * t / seconds)) / SR)
         + 0.5 * np.sin(2 * np.pi * np.cumsum(2.76 * f * (1 - 0.15 * t / seconds)) / SR))
    x *= np.exp(-t / 0.25) * np.minimum(1.0, t / 0.002)
    return np.stack([x, x])


def detector_click(seed, seconds=0.06):
    """The anomaly detector's tick: a dry Geiger-counter click, a
    snap with a little body (its case ringing, so it carries over rain
    without a needle-sharp peak)."""
    rng = np.random.default_rng(seed)
    n = int(seconds * SR)
    t = np.arange(n) / SR
    snap = rng.normal(size=n) * np.exp(-t / 0.0012)
    body = np.sin(2 * np.pi * (1100 + 90 * seed) * t) * np.exp(-t / 0.012)
    x = 0.6 * snap + body
    return np.stack([x, x])


def robot_voice(seed, syllables=None):
    """A robot talking to the others over its radio: machine syllables (a
    buzz through shifting formants, clipped short), the way a cheap
    speech chip would say them, then the radio's chain: band-limited to
    300-3400 Hz, crushed to 6 bits at 8 kHz, a squelch tail."""
    rng = np.random.default_rng(seed)
    k = syllables or int(rng.integers(3, 8))
    parts = []
    vowels = [(300, 870, 2240), (530, 1840, 2480), (270, 2290, 3010), (730, 1090, 2440),
              (570, 840, 2410), (440, 1020, 2240)]
    f0 = rng.uniform(85, 130)
    for _ in range(k):
        dur = rng.uniform(0.05, 0.13)
        n = int(dur * SR)
        t = np.arange(n) / SR
        f = f0 * (1 + 0.08 * rng.normal()) * (1 + 0.15 * t / dur * rng.choice([-1, 1]))
        phase = np.cumsum(f) / SR
        buzz = 2 * (phase % 1.0) - 1.0
        v = vowels[int(rng.integers(len(vowels)))]
        y = np.zeros(n)
        for fc, bw in zip(v, (80, 120, 160)):
            # A two-pole resonator per formant.
            r = np.exp(-np.pi * bw / SR)
            a1, a2 = -2 * r * np.cos(2 * np.pi * fc / SR), r * r
            out = np.zeros(n)
            y1 = y2 = 0.0
            for i in range(n):
                y0 = buzz[i] - a1 * y1 - a2 * y2
                out[i] = y0
                y2, y1 = y1, y0
            y += out / np.max(np.abs(out) + 1e-9)
        env = np.minimum(1, t / 0.006) * np.minimum(1, (dur - t) / 0.01)
        parts.append(y * env)
        parts.append(np.zeros(int(rng.uniform(0.01, 0.05) * SR)))
    x = np.concatenate(parts)
    # The radio: band-limited, then crushed (6 bits, held at 8 kHz).
    x = lowpass(x, 3400) - lowpass(x, 300)
    x = x / np.max(np.abs(x))
    hold = SR // 8000
    x = np.repeat(x[::hold], hold)[: len(x)]
    x = np.round(x * 31) / 31
    # The speaker after it takes the crush's harshest images off.
    x = lowpass(x, 5000)
    tail = rng.normal(size=int(0.12 * SR)) * np.exp(-np.arange(int(0.12 * SR)) / SR / 0.03) * 0.3
    x = np.concatenate([x, lowpass(tail, 3000) - lowpass(tail, 500)])
    return np.stack([x, x])


def generator(seed=60, seconds=8.0):
    """A small diesel generator running, close to (a stand-in until a
    recording can be had): its firing at about 27 Hz with the harmonics a
    single cylinder's pulses make, the rate wandering a little, a
    mechanical rattle riding each firing, and the exhaust's breath."""
    rng = np.random.default_rng(seed)
    n = int(seconds * SR)
    t = np.arange(n) / SR
    out = []
    for ch in range(2):
        jitter = 1.0 + 0.012 * lowpass(rng.normal(size=n), 3.0) / 0.05
        f = 27.0 * jitter
        phase = np.cumsum(f) / SR
        # Firing pulses: a sharp rise and a decay each cycle.
        cyc = phase % 1.0
        pulse = np.exp(-cyc / 0.12) * (1 + 0.15 * rng.normal(size=n))
        thump = lowpass(pulse - pulse.mean(), 400)
        # The rattle: bursts of bright noise riding each firing.
        n_hi = rng.normal(size=n)
        rattle = (n_hi - lowpass(n_hi, 1800)) * np.exp(-cyc / 0.04)
        exhaust = lowpass(rng.normal(size=n), 250) - lowpass(rng.normal(size=n), 60)
        x = 1.0 * thump / np.max(np.abs(thump)) + 0.25 * rattle / np.max(np.abs(rattle)) \
            + 0.4 * exhaust / np.max(np.abs(exhaust))
        out.append(loop_crossfade(x, 0.5))
    return np.stack(out)


def oneshots(entries, key, out, make, seeds):
    files = []
    name = key.split("/")[1]
    for i, seed in enumerate(seeds):
        path, secs = write(f"{name}_{i + 1}", make(seed), out=out)
        files.append({"file": str(path.relative_to(ROOT / "assets")), "seconds": round(secs, 3),
                      "source": "synth:scripts/synth_anomaly.py"})
    entries[key] = {"type": "oneshot", "channels": 2, "loop": False, "files": files}


def main():
    entries = {}
    loopset(entries, "lift_hum", lift_hum())
    loopset(entries, "burner_roar", burner_roar())
    loopset(entries, "snare_buzz", snare_buzz())
    loopset(entries, "attention", attention_ring())
    loopset(entries, "lift_pop", lift_pop(), kind="oneshot")
    loopset(entries, "offering", bowl(), kind="oneshot")
    loopset(entries, "well_drone", well_drone())
    loopset(entries, "mirror_shimmer", mirror_shimmer())
    oneshots(entries, "anomaly/mirror_blink", OUT, mirror_blink, [41, 42, 43])
    oneshots(entries, "anomaly/reveal", OUT, reveal, [0, 1, 2])
    path, secs = write("generator", generator(), out=ROOT / "assets" / "audio" / "machines")
    entries["machines/generator"] = {
        "type": "loop", "channels": 2, "loop": True,
        "files": [{"file": str(path.relative_to(ROOT / "assets")), "seconds": round(secs, 3),
                   "source": "synth:scripts/synth_anomaly.py"}],
    }
    oneshots(entries, "machines/robot_voice", ROOT / "assets" / "audio" / "machines",
             robot_voice, [50, 51, 52, 53, 54, 55, 56, 57])
    oneshots(entries, "machines/detector_click", ROOT / "assets" / "audio" / "machines",
             detector_click, [0, 1, 2, 3])
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

    files = []
    robots = ROOT / "assets" / "audio" / "machines"
    for i in range(2):
        path, secs = write(f"drone_lock_{i + 1}", drone_lock(i), out=robots)
        files.append({"file": str(path.relative_to(ROOT / "assets")), "seconds": round(secs, 3),
                      "source": "synth:scripts/synth_anomaly.py"})
    entries["machines/drone_lock"] = {"type": "oneshot", "channels": 2, "loop": False, "files": files}

    sounds = ROOT / "assets" / "audio" / "sounds.json"
    data = json.loads(sounds.read_text())
    data.update(entries)
    sounds.write_text(json.dumps(dict(sorted(data.items())), indent=2) + "\n")
    print("wrote", ", ".join(entries))


if __name__ == "__main__":
    main()
