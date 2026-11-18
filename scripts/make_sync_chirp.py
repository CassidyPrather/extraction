#!/usr/bin/env python3
"""The playtest tour's sync chirp, made from scratch: a rising sweep, 1.2
to 4.8 kHz in 0.3 s, played as each station starts. Nothing in the game
sounds like it, so scripts/playtest_ingest.py finds it in a recording's
audio by cross-correlation, and lines the video up with the game's logs
to the frame. Writes assets/audio/ui/tour_sync.ogg; ingest reads the same
file as its template.

    python3 scripts/make_sync_chirp.py
"""
import os
import subprocess
import tempfile
import wave

import numpy as np

RATE = 44100
F0, F1, SECONDS = 1200.0, 4800.0, 0.3
OUT = os.path.join(os.path.dirname(__file__), "..", "assets", "audio", "ui")


def chirp():
    t = np.arange(int(RATE * SECONDS)) / RATE
    # Exponential sweep: equal time per octave, two octaves.
    k = np.log(F1 / F0) / SECONDS
    phase = 2 * np.pi * F0 * (np.exp(k * t) - 1) / k
    # 5 ms fades, so it doesn't click.
    env = np.minimum(1.0, np.minimum(t, SECONDS - t) / 0.005)
    return np.sin(phase) * env


def main():
    samples = chirp() * 0.8
    pcm = (samples * 32767).astype(np.int16)
    os.makedirs(OUT, exist_ok=True)
    with tempfile.NamedTemporaryFile(suffix=".wav", delete=False) as f:
        tmp = f.name
    with wave.open(tmp, "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(RATE)
        w.writeframes(pcm.tobytes())
    dest = os.path.join(OUT, "tour_sync.ogg")
    subprocess.run(["ffmpeg", "-y", "-loglevel", "error", "-i", tmp,
                    "-c:a", "libvorbis", "-q:a", "6", dest], check=True)
    os.remove(tmp)
    print(dest)


if __name__ == "__main__":
    main()
