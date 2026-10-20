#!/usr/bin/env python3
"""Builds the head-related impulse responses the headset's binaural audio
uses (`audio.rs`, firewheel-ircam-hrtf's HrtfNode), from the SADIE II
database's KEMAR dummy head (University of York, Apache-2.0; see
assets/audio/hrir/NOTICE).

Why not the IRCAM subjects the node embeds: the IRCAM Listen pages offer
the data for download but state no licence; SADIE's is explicit, and it
was measured at the game's 48 kHz (no resampling at load). The sphere is
written in bevy_seedling's listener frame (+X right, +Y up, -Z forward);
`scripts/audio_binaural.py` checks the cues on rendered output, with
IRCAM alongside for comparison.

The sphere: rings every 15 degrees of elevation (and the poles), 24
directions round each, triangulated ring to ring; each vertex takes the
nearest of SADIE's 8802 measured directions. 48 kHz, 256 taps (zero-padded to 385, see TAPS), in the
`hrtf` crate's HRIR format:

    "HRIR" u32 rate, u32 length, u32 vertices, u32 indices,
    u32 indices..., then per vertex f32 x, y, z, left[length], right[length]

    python3 scripts/audio_hrir.py        # needs h5py (pip install h5py)
"""
import hashlib
import struct
import sys
import urllib.request
import zipfile
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parent.parent
CACHE = ROOT / "assets" / ".cache" / "hrir"
OUT = ROOT / "assets" / "audio" / "hrir" / "sadie_d2_kemar.bin"
URL = "https://zenodo.org/records/12092466/files/D2_HRIR_SOFA.zip?download=1"
MEMBER = "D2_HRIR_SOFA/D2_48K_24bit_256tap_FIR_SOFA.sofa"
ELEVATIONS = [-75, -60, -45, -30, -15, 0, 15, 30, 45, 60, 75]
AROUND = 24
# The node convolves 128-sample slices by FFT, of slice + taps - 1 points,
# with a scratch buffer only that long: a length rustfft can't factor into
# small radices (Bluestein, Rader) needs more and panics on the audio
# thread. SADIE's 256 taps, zero-padded to 385, make it 512.
SLICE = 128
TAPS = 385


def fetch():
    CACHE.mkdir(parents=True, exist_ok=True)
    z = CACHE / "D2_HRIR_SOFA.zip"
    if not z.exists():
        print(f"download {URL}")
        req = urllib.request.Request(URL, headers={"User-Agent": "extraction-audio-fetch/1"})
        z.write_bytes(urllib.request.urlopen(req, timeout=300).read())
    digest = hashlib.sha256(z.read_bytes()).hexdigest()
    pin = Path(__file__).with_suffix(".sha256")
    if pin.exists() and pin.read_text().strip() != digest:
        sys.exit(f"{z}: sha256 {digest} != pinned {pin.read_text().strip()}")
    pin.write_text(digest + "\n")
    sofa = CACHE / Path(MEMBER).name
    if not sofa.exists():
        with zipfile.ZipFile(z) as f:
            sofa.write_bytes(f.read(MEMBER))
    return sofa


def direction(az_deg, el_deg):
    """SOFA's spherical (azimuth anticlockwise from the front, elevation
    up) to bevy_seedling's listener frame: +X right, +Y up, -Z forward."""
    az, el = np.radians(az_deg), np.radians(el_deg)
    return np.stack([-np.sin(az) * np.cos(el), np.sin(el), -np.cos(az) * np.cos(el)], axis=-1)


def mesh():
    """Vertices (azimuth, elevation) and triangles: a top and a bottom
    pole, rings between."""
    verts = [(0.0, -90.0)]
    for el in ELEVATIONS:
        verts += [(k * 360.0 / AROUND, float(el)) for k in range(AROUND)]
    verts.append((0.0, 90.0))
    ring = lambda r, k: 1 + r * AROUND + k % AROUND
    faces = []
    for k in range(AROUND):
        faces.append((0, ring(0, k + 1), ring(0, k)))
    for r in range(len(ELEVATIONS) - 1):
        for k in range(AROUND):
            a, b = ring(r, k), ring(r, k + 1)
            c, d = ring(r + 1, k), ring(r + 1, k + 1)
            faces += [(a, b, d), (a, d, c)]
    top = len(verts) - 1
    for k in range(AROUND):
        faces.append((top, ring(len(ELEVATIONS) - 1, k), ring(len(ELEVATIONS) - 1, k + 1)))
    return np.array(verts), np.array(faces, dtype="<u4")


def main():
    import h5py
    f = h5py.File(fetch(), "r")
    rate = int(f["Data.SamplingRate"][0])
    pos = f["SourcePosition"][:]
    ir = f["Data.IR"][:].astype(np.float32)
    ir = np.pad(ir, ((0, 0), (0, 0), (0, TAPS - ir.shape[2])))
    assert (SLICE + TAPS - 1) & (SLICE + TAPS - 2) == 0
    measured = direction(pos[:, 0], pos[:, 1])
    verts, faces = mesh()
    dirs = direction(verts[:, 0], verts[:, 1])
    nearest = np.argmax(dirs @ measured.T, axis=1)
    worst = np.degrees(np.arccos(np.clip((dirs * measured[nearest]).sum(1), -1, 1))).max()
    out = bytearray(b"HRIR")
    out += struct.pack("<4I", rate, ir.shape[2], len(verts), faces.size)
    out += faces.tobytes()
    for d, m in zip(dirs, nearest):
        out += struct.pack("<3f", *d.astype(np.float32))
        out += ir[m, 0].astype("<f4").tobytes() + ir[m, 1].astype("<f4").tobytes()
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_bytes(bytes(out))
    print(f"{OUT.relative_to(ROOT)}: {len(verts)} directions, {len(faces)} faces, {rate} Hz, "
          f"{ir.shape[2]} taps; nearest measurement at most {worst:.1f} deg off")


if __name__ == "__main__":
    main()
