#!/usr/bin/env python3
"""The training yard's chimes, made from scratch (no samples): a small
hand bell struck once for a step done, twice rising for a station done.
Writes assets/audio/ui/chime_step.ogg and chime_station.ogg.

    python3 scripts/make_chimes.py
"""
import os
import subprocess
import tempfile
import wave

import numpy as np

RATE = 44100
OUT = os.path.join(os.path.dirname(__file__), "..", "assets", "audio", "ui")


def bell(f0, seconds, t0=0.0, total=None):
    """A struck bell: inharmonic partials, each dying at its own rate."""
    total = total or seconds
    t = np.arange(int(RATE * total)) / RATE - t0
    out = np.zeros_like(t)
    on = t >= 0
    for ratio, amp, decay in [(1.0, 1.0, 3.2), (2.0, 0.45, 5.0), (2.76, 0.32, 7.0),
                              (5.4, 0.12, 11.0), (8.9, 0.05, 16.0)]:
        out[on] += amp * np.sin(2 * np.pi * f0 * ratio * t[on]) * np.exp(-decay * t[on])
    # A soft strike: 3 ms attack.
    out[on] *= np.clip(t[on] / 0.003, 0.0, 1.0)
    return out


def write(name, samples):
    samples = samples / np.max(np.abs(samples)) * 0.7
    pcm = (samples * 32767).astype(np.int16)
    with tempfile.NamedTemporaryFile(suffix=".wav", delete=False) as f:
        tmp = f.name
    with wave.open(tmp, "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(RATE)
        w.writeframes(pcm.tobytes())
    dest = os.path.join(OUT, name)
    subprocess.run(["ffmpeg", "-y", "-loglevel", "error", "-i", tmp,
                    "-c:a", "libvorbis", "-q:a", "5", dest], check=True)
    os.remove(tmp)
    print(dest)


os.makedirs(OUT, exist_ok=True)
write("chime_step.ogg", bell(1318.5, 0.7))
write("chime_station.ogg", bell(987.8, 1.2, total=1.2) + bell(1318.5, 1.0, t0=0.18, total=1.2))
