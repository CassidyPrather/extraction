#!/usr/bin/env python3
"""Rain drumming on a roof over you, made from scratch: tin, wood, canvas
(`acoustics::rain_under` plays it; docs/AUDIO.md, "Rain under a roof").

There are no recordings of these in the CC0 libraries this repo could
reach, so they're modelled: rain on a roof is a dense Poisson stream of
drop impacts, each a short burst exciting the roof's modes.

- **Tin:** a few bright modes (1-4 kHz) ringing 80-150 ms, and a low boom
  from the panel.
- **Wood:** shingles and planks, darker (300-900 Hz) and short (~20 ms).
- **Canvas:** soft, low-passed thumps with almost no ring.

Each roof gets a light and a heavy loop (more and bigger drops); the game
blends them by the rain. A loop is the circular convolution of a drop
train with the roof's response, so it repeats seamlessly. Each is
levelled against the outdoor rain bed (`ambience/rain_light`), by the
offsets in `LEVEL`, which the audio audit checks.

Deterministic. Writes assets/audio/ambience/rain_{tin,wood,canvas}[_light].ogg
and their sounds.json entries.

    python3 scripts/make_rain_roofs.py
"""
import json
import subprocess
import sys
import tempfile
import wave
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "assets" / "audio" / "ambience"
SR = 48000
SECONDS = 20.0
SOURCE = "synth:scripts/make_rain_roofs.py"

# Loudness (integrated, LU) against the outdoor rain bed: a tin roof
# drums louder than the rain outside, a tent about as loud, a shingled
# roof well under.
LEVEL = {"tin": 3.0, "wood": -4.0, "canvas": 0.0}


def damped_modes(rng, n, freqs, decays, amps):
    t = np.arange(n) / SR
    out = np.zeros(n)
    for f, d, a in zip(freqs, decays, amps):
        out += a * np.sin(2 * np.pi * f * t + rng.uniform(0, 2 * np.pi)) * np.exp(-t / d)
    return out


def burst(rng, n, ms, lowpass_hz, order=1):
    """The drop's strike: a few ms of noise, softened (`order` poles)."""
    k = int(ms / 1000 * SR)
    x = np.zeros(n)
    x[:k] = rng.normal(size=k) * np.hanning(k)
    X = np.fft.rfft(x)
    f = np.fft.rfftfreq(n, 1 / SR)
    return np.fft.irfft(X / np.sqrt(1 + (f / lowpass_hz) ** 2) ** order, n)


def response(rng, roof, n=8192):
    """One patch of roof's response to a drop."""
    if roof == "tin":
        k = 12
        freqs = rng.uniform(1000, 4000, k)
        decays = rng.uniform(0.08, 0.15, k)
        amps = rng.uniform(0.3, 1.0, k)
        ring = damped_modes(rng, n, freqs, decays, amps)
        boom = damped_modes(rng, n, [rng.uniform(80, 140)], [0.06], [0.6])
        return np.convolve(burst(rng, 256, 1.0, 8000), ring + boom)[:n]
    if roof == "wood":
        k = 10
        freqs = rng.uniform(300, 900, k)
        decays = rng.uniform(0.012, 0.028, k)
        amps = rng.uniform(0.4, 1.0, k)
        ring = damped_modes(rng, n, freqs, decays, amps)
        return np.convolve(burst(rng, 256, 2.0, 2500, order=2), ring)[:n]
    # canvas: a soft thump, a breath of membrane at ~150 Hz.
    thump = burst(rng, n, 3.0, 1400, order=2) * 6
    membrane = damped_modes(rng, n, [rng.uniform(120, 180)], [0.01], [0.4])
    return thump + membrane


def drops(rng, n, rate, size_sigma):
    """A Poisson train of drop impacts, sizes log-normal."""
    count = rng.poisson(rate * n / SR)
    at = rng.integers(0, n, count)
    train = np.zeros(n)
    np.add.at(train, at, rng.lognormal(0.0, size_sigma, count))
    return train


def render(roof, heavy, seed):
    rng = np.random.default_rng(seed)
    n = int(SECONDS * SR)
    rate = {"tin": 260, "wood": 320, "canvas": 360}[roof] * (2.4 if heavy else 1.0)
    # Canvas gives under a big drop: its thumps vary less.
    sigma = (0.7 if heavy else 0.5) * (0.6 if roof == "canvas" else 1.0)
    channels = []
    for ch in range(2):
        out = np.zeros(n)
        # Many patches of roof (each drop lands on one: every sheet and
        # shingle rings a little differently), so no one resonance sings.
        patches = 16
        for patch in range(patches):
            h = np.zeros(n)
            r = response(rng, roof)
            h[: len(r)] = r
            train = drops(rng, n, rate / patches, sigma)
            out += np.fft.irfft(np.fft.rfft(train) * np.fft.rfft(h), n)
        # The rain beyond the roof, a hiss under it all.
        hiss = rng.normal(size=n)
        H = np.fft.rfft(hiss)
        f = np.fft.rfftfreq(n, 1 / SR)
        hiss = np.fft.irfft(H / np.sqrt(1 + (f / 3000) ** 2) / np.sqrt(1 + (200 / np.maximum(f, 1)) ** 2), n)
        # (Under canvas the drops' thumps are soft, so less hiss over them.)
        hiss_level = 0.12 if roof == "canvas" else 0.25
        out = out / np.sqrt(np.mean(out ** 2)) + hiss_level * hiss / np.sqrt(np.mean(hiss ** 2))
        channels.append(out)
    return np.stack(channels, axis=1)


def loudness(x):
    sys.path.insert(0, str(ROOT / "scripts"))
    import audio_audit as aa
    return aa.integrated(aa.k_weight(x))


def write(name, x):
    OUT.mkdir(parents=True, exist_ok=True)
    pcm = (np.clip(x, -1, 1) * 32767).astype("<i2")
    with tempfile.NamedTemporaryFile(suffix=".wav", delete=False) as f:
        tmp = f.name
    with wave.open(tmp, "wb") as w:
        w.setnchannels(2)
        w.setsampwidth(2)
        w.setframerate(SR)
        w.writeframes(pcm.tobytes())
    path = OUT / f"{name}.ogg"
    subprocess.run(["ffmpeg", "-y", "-loglevel", "error", "-i", tmp, "-c:a", "libvorbis",
                    "-q:a", "4", "-fflags", "+bitexact", "-flags:a", "+bitexact",
                    "-map_metadata", "-1", str(path)], check=True)
    Path(tmp).unlink()
    return path


def main():
    sys.path.insert(0, str(ROOT / "scripts"))
    import audio_audit as aa
    rain = aa.decode(ROOT / "assets" / "audio" / "ambience" / "rain_light_1.ogg")
    reference = loudness(rain)
    index_path = ROOT / "assets" / "audio" / "sounds.json"
    index = json.loads(index_path.read_text())
    for i, roof in enumerate(LEVEL):
        for heavy in (True, False):
            name = f"rain_{roof}" + ("" if heavy else "_light")
            x = render(roof, heavy, 900 + 10 * i + heavy)
            # Light rain on the same roof plays 6 LU under the heavy.
            target = reference + LEVEL[roof] - (0 if heavy else 6.0)
            # Level it, then let the mastering chain's limiter take the
            # biggest drops' peaks (-1 dBFS), and level it again.
            from fetch_audio import limit
            for _ in range(3):
                x = x * 10 ** ((target - loudness(x)) / 20)
                x = limit(x, -1.5)
            path = write(name, x)
            index[f"ambience/{name}"] = {
                "type": "loop", "channels": 2, "loop": True,
                "files": [{"file": str(path.relative_to(ROOT / "assets")),
                           "seconds": SECONDS, "source": SOURCE}],
            }
            print(f"{name}: {loudness(x):.1f} LUFS (rain outside {reference:.1f})")
    index_path.write_text(json.dumps(dict(sorted(index.items())), indent=2) + "\n")


if __name__ == "__main__":
    main()
