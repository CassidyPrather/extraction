#!/usr/bin/env python3
"""Fetch recorded sound sources and master them into game-ready Ogg Vorbis.

    python3 scripts/fetch_audio.py                 # fetch + master everything
    python3 scripts/fetch_audio.py --only footsteps/grass guns   # sets or whole categories
    python3 scripts/fetch_audio.py --inspect footsteps/grass  # PNG of how a set gets split
    python3 scripts/fetch_audio.py --check-licenses           # re-read licenses
    python3 scripts/fetch_audio.py --credits                  # CREDITS.md rows

Sources and sets live in scripts/audio_manifest.json. Downloads are cached
under assets/.cache/audio/ (gitignored) and verified against the sha256 in
the manifest; a source with no sha256 yet gets one written back on first
download (pinning). Mastered files land in assets/audio/<category>/ and an
index of every set is written to assets/audio/sounds.json.

Needs python3 + numpy and ffmpeg (with libvorbis) on PATH. --inspect also
needs Pillow. See docs/AUDIO.md for the mastering chain and how to add sounds.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import re
import shutil
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
import zipfile
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parent.parent
MANIFEST = ROOT / "scripts" / "audio_manifest.json"
CACHE = ROOT / "assets" / ".cache" / "audio"
OUT = ROOT / "assets" / "audio"
SR = 48000
USER_AGENT = "extraction-audio-fetch/1 (+https://github.com/CassidyPrather)"

# Envelope analysis resolution: 10 ms RMS window, 2.5 ms hop.
ENV_WIN = 0.010
ENV_HOP = 0.0025

DEFAULT_SPLIT = {
    "mode": "events",  # events | whole | segments
    "rel_db": -30.0,  # onset threshold below the loudest point of the input
    "floor_margin_db": 10.0,  # ...but at least this far above the noise floor
    "min_gap": 0.12,  # merge bursts closer than this (s)
    "min_len": 0.04,  # drop events shorter than this (s)
    "max_len": 1.5,  # hard cap per clip (s)
    "pre": 0.008,  # pre-roll kept before the onset (s)
    "tail_db": 40.0,  # clip ends when the envelope falls this far below its peak
    "drop_db": 10.0,  # drop events this much quieter than the median event
}


# --------------------------------------------------------------------------
# small utilities


def log(msg: str) -> None:
    print(msg, flush=True)


def sha256_of(path: Path) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def db(x: float) -> float:
    return 20.0 * math.log10(max(x, 1e-12))


def undb(x: float) -> float:
    return 10.0 ** (x / 20.0)


def need_tool(name: str) -> None:
    if shutil.which(name) is None:
        sys.exit(f"fetch_audio: '{name}' not found on PATH (apt-get install {name})")


# --------------------------------------------------------------------------
# fetching


def http_get(url: str, tries: int = 4) -> bytes:
    """GET with retries; Freesound answers 429/503 when hit too quickly."""
    req = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    for attempt in range(tries):
        try:
            with urllib.request.urlopen(req, timeout=60) as r:
                return r.read()
        except urllib.error.HTTPError as e:
            if e.code not in (429, 500, 502, 503, 504) or attempt == tries - 1:
                raise
        except urllib.error.URLError:
            if attempt == tries - 1:
                raise
        time.sleep(5 * 2 ** attempt)
    raise AssertionError("unreachable")


def source_path(sid: str, src: dict) -> Path:
    ext = Path(src["url"].split("?")[0]).suffix or ".bin"
    return CACHE / "src" / f"{sid}{ext}"


def fetch_source(sid: str, src: dict, offline: bool, repin: bool) -> tuple[Path, bool]:
    """Returns (path, manifest_changed)."""
    path = source_path(sid, src)
    changed = False
    if not path.exists():
        if offline:
            raise RuntimeError(f"{sid}: not cached and --offline given")
        log(f"  download {sid}  {src['url']}")
        data = http_get(src["url"])
        path.parent.mkdir(parents=True, exist_ok=True)
        tmp = path.with_suffix(path.suffix + ".part")
        tmp.write_bytes(data)
        tmp.replace(path)
    digest = sha256_of(path)
    want = src.get("sha256")
    if not want or repin:
        if want != digest:
            src["sha256"] = digest
            changed = True
    elif want != digest:
        raise RuntimeError(
            f"{sid}: sha256 mismatch\n  manifest {want}\n  file     {digest}\n"
            f"  ({path}) - upstream changed? Re-check the source and run with --repin."
        )
    return path, changed


def input_file(sid: str, src: dict, member: str | None, path: Path) -> Path:
    """Resolves an input to a decodable file (extracting zip members)."""
    if member is None:
        return path
    out = CACHE / "src" / sid / member
    if not out.exists():
        with zipfile.ZipFile(path) as z:
            out.parent.mkdir(parents=True, exist_ok=True)
            out.write_bytes(z.read(member))
    return out


# --------------------------------------------------------------------------
# decoding and analysis


def decode(path: Path, channels: int, highpass: float | None, lowpass: float | None,
           start: float | None, end: float | None, extra_af: str | None = None) -> np.ndarray:
    """Decodes to float32 [n, channels] at 48 kHz with the set's filters applied."""
    af = []
    if highpass:
        # Two 2-pole sections: a 24 dB/oct rumble filter.
        af += [f"highpass=f={highpass}:poles=2", f"highpass=f={highpass}:poles=2"]
    if lowpass:
        af += [f"lowpass=f={lowpass}:poles=2"]
    if extra_af:
        af.append(extra_af)
    cmd = ["ffmpeg", "-v", "error", "-nostdin"]
    if start:
        cmd += ["-ss", f"{start:.4f}"]
    if end:
        cmd += ["-to", f"{end:.4f}"]
    cmd += ["-i", str(path)]
    if af:
        cmd += ["-af", ",".join(af)]
    # Decode at the file's channel count; fold to mono ourselves.
    cmd += ["-f", "f32le", "-ar", str(SR), "-"]
    probe = subprocess.run(
        ["ffprobe", "-v", "error", "-select_streams", "a:0", "-show_entries",
         "stream=channels", "-of", "csv=p=0", str(path)],
        capture_output=True, text=True, check=True)
    src_ch = int(probe.stdout.strip().split(",")[0])
    raw = subprocess.run(cmd, capture_output=True, check=True).stdout
    x = np.frombuffer(raw, dtype=np.float32).reshape(-1, src_ch).astype(np.float64)
    return to_channels(x, channels)


def to_channels(x: np.ndarray, channels: int) -> np.ndarray:
    if x.shape[1] == channels:
        return x
    if channels == 1:
        if x.shape[1] == 2:
            l, r = x[:, 0], x[:, 1]
            mix = 0.5 * (l + r)
            # Guard against out-of-phase pairs cancelling in the fold.
            e_mix = float(np.mean(mix ** 2))
            e_ch = 0.5 * float(np.mean(l ** 2) + np.mean(r ** 2))
            if e_ch > 0 and e_mix < 0.35 * e_ch:
                return (l if np.mean(l ** 2) >= np.mean(r ** 2) else r)[:, None]
            return mix[:, None]
        return x.mean(axis=1, keepdims=True)
    if x.shape[1] == 1:
        return np.repeat(x, channels, axis=1)
    return x[:, :channels]


def envelope_db(mono: np.ndarray) -> np.ndarray:
    win = int(SR * ENV_WIN)
    hop = int(SR * ENV_HOP)
    sq = np.concatenate([[0.0], np.cumsum(mono.astype(np.float64) ** 2)])
    idx = np.arange(0, max(len(mono) - win, 0) + 1, hop)
    if len(idx) == 0:
        return np.array([db(float(np.sqrt(np.mean(mono ** 2))))])
    rms = np.sqrt(np.maximum((sq[idx + win] - sq[idx]) / win, 1e-24))
    return 20.0 * np.log10(rms)


def detect_events(x: np.ndarray, p: dict) -> list[dict]:
    """Finds separate hits in a multi-hit recording.

    Returns dicts with start/end (seconds) and peak (dB of the envelope)."""
    mono = x.mean(axis=1)
    env = envelope_db(mono)
    hop = ENV_HOP
    floor = float(np.percentile(env, 10))
    top = float(env.max())
    thr = max(top + p["rel_db"], floor + p["floor_margin_db"])
    above = env > thr
    # Runs of above-threshold frames, merging short gaps.
    runs = []
    i, n = 0, len(env)
    gap = int(p["min_gap"] / hop)
    while i < n:
        if not above[i]:
            i += 1
            continue
        j = i
        last = i
        while j < n and (above[j] or j - last <= gap):
            if above[j]:
                last = j
            j += 1
        runs.append((i, last))
        i = last + 1
    events = []
    quiet = floor + 6.0
    for k, (a, b) in enumerate(runs):
        pk = float(env[a:b + 1].max())
        # Walk back to where the attack leaves the floor (at most 60 ms).
        s = a
        while s > 0 and env[s - 1] > quiet and (a - s) * hop < 0.06:
            s -= 1
        # Walk forward through the decay.
        stop = max(quiet, pk - p["tail_db"])
        e = b
        limit = runs[k + 1][0] if k + 1 < len(runs) else n
        while e + 1 < limit and env[e + 1] > stop:
            e += 1
        start = max(0.0, s * hop - p["pre"])
        end = min((e + 1) * hop + ENV_WIN, len(mono) / SR)
        if k + 1 < len(runs):
            end = min(end, runs[k + 1][0] * hop - 0.005)
        truncated = end - start > p["max_len"]
        end = min(end, start + p["max_len"])
        if end - start < p["min_len"]:
            continue
        seg = x[int(start * SR):int(end * SR)]
        clipped = bool(np.max(np.abs(seg)) >= 0.999) if len(seg) else False
        events.append({"start": start, "end": end, "peak": pk,
                       "truncated": truncated, "clipped": clipped})
    if events:
        med = float(np.median([e["peak"] for e in events]))
        for e in events:
            e["weak"] = e["peak"] < med - p["drop_db"]
            score = -abs(e["peak"] - med)
            if e["truncated"]:
                score -= 6.0
            if e["clipped"]:
                score -= 12.0
            if e["weak"]:
                score -= 100.0
            e["score"] = score
    return events


# --------------------------------------------------------------------------
# finishing


def trim(x: np.ndarray, head_db: float = -30.0, tail_db: float = -55.0) -> np.ndarray:
    """Cuts silence relative to the clip's own peak."""
    mono = np.abs(x).max(axis=1)
    pk = mono.max()
    if pk <= 0:
        return x
    loud = np.nonzero(mono > pk * undb(head_db))[0]
    a = max(0, int(loud[0]) - int(0.005 * SR)) if len(loud) else 0
    env = envelope_db(x.mean(axis=1)[a:])
    env_pk = env.max()
    keep = np.nonzero(env > env_pk + tail_db)[0]
    b = a + (int((keep[-1]) * ENV_HOP * SR + ENV_WIN * SR) if len(keep) else len(x))
    return x[a:min(b, len(x))]


def fade(x: np.ndarray, fade_in: float, fade_out: float) -> np.ndarray:
    x = x.copy()
    n = len(x)
    fi = min(int(fade_in * SR), n // 4)
    fo = min(int(fade_out * SR), n // 2)
    if fi > 0:
        x[:fi] *= (0.5 - 0.5 * np.cos(np.linspace(0, math.pi, fi)))[:, None]
    if fo > 0:
        x[n - fo:] *= (0.5 + 0.5 * np.cos(np.linspace(0, math.pi, fo)))[:, None]
    return x


def loudness_lufs(x: np.ndarray) -> float:
    """Integrated loudness via ffmpeg's EBU R128 meter."""
    with tempfile.TemporaryDirectory() as td:
        wav = Path(td) / "m.wav"
        write_wav(wav, x)
        r = subprocess.run(["ffmpeg", "-nostats", "-nostdin", "-i", str(wav), "-af",
                            "ebur128=framelog=quiet", "-f", "null", "-"],
                           capture_output=True, text=True)
    m = re.findall(r"I:\s+(-?[\d.]+|-inf) LUFS", r.stderr)
    if not m or m[-1] == "-inf":
        return -70.0
    return float(m[-1])


def limit(x: np.ndarray, ceiling_db: float) -> np.ndarray:
    """Lookahead peak limiter (1 ms blocks, 5 ms lookahead, 60 ms release)."""
    ceil = undb(ceiling_db)
    if np.abs(x).max() <= ceil:
        return x
    blk = int(0.001 * SR)
    n = len(x)
    nb = -(-n // blk)
    pad = np.zeros((nb * blk, x.shape[1]))
    pad[:n] = x
    peaks = np.abs(pad).reshape(nb, blk, -1).max(axis=(1, 2))
    look = 5
    held = peaks.copy()
    for k in range(1, look + 1):
        held[:-k] = np.maximum(held[:-k], peaks[k:])
        held[k:] = np.maximum(held[k:], peaks[:-k])
    gain = np.minimum(1.0, ceil / np.maximum(held, 1e-12))
    # Smooth (release) without letting gain rise above the needed value.
    rel = 60
    kern = np.ones(rel) / rel
    smooth = np.convolve(np.concatenate([np.full(rel, gain[0]), gain]), kern, "same")[rel:]
    gain = np.minimum(gain, smooth)
    g = np.interp(np.arange(nb * blk), np.arange(nb) * blk + blk / 2, gain)[:n]
    y = x * g[:, None]
    return np.clip(y, -ceil, ceil)


def normalize(x: np.ndarray, norm: dict) -> np.ndarray:
    if "lufs" in norm:
        cur = loudness_lufs(x)
        x = x * undb(norm["lufs"] - cur)
        return limit(x, norm.get("peak", -1.0))
    pk = np.abs(x).max()
    return x * (undb(norm.get("peak", -1.0)) / pk) if pk > 0 else x


def make_loop(x: np.ndarray, length: float, xfade: float, tonal: bool = False) -> np.ndarray:
    """Takes length+xfade seconds and folds the overhang over the head, so
    the result loops without a seam.

    Noise-like beds use an equal-power crossfade. Tonal loops (hum, motors)
    would partly cancel under that, so with tonal=True the loop length is
    nudged by up to 25 ms to where the overhang best lines up in phase with
    the head, and the crossfade is linear (equal-gain)."""
    n = int(length * SR)
    f = int(xfade * SR)
    if tonal:
        w = int(0.025 * SR)
        if len(x) < n + f + w:
            raise RuntimeError(f"loop needs {length + xfade + 0.025:.2f}s, input has {len(x) / SR:.2f}s")
        head = x[:f].mean(axis=1)
        best, best_d = -2.0, 0
        for d in range(-w, w + 1):
            seg = x[n + d:n + d + f].mean(axis=1)
            c = float(np.dot(seg, head) / (np.linalg.norm(seg) * np.linalg.norm(head) + 1e-12))
            if c > best:
                best, best_d = c, d
        n += best_d
    if len(x) < n + f:
        raise RuntimeError(f"loop needs {length + xfade:.1f}s, input has {len(x) / SR:.1f}s")
    body = x[:n].copy()
    tail = x[n:n + f]
    if tonal:
        a = np.linspace(0, 1, f)[:, None]
        body[:f] = body[:f] * a + tail * (1 - a)
    else:
        t = np.linspace(0, math.pi / 2, f)[:, None]
        body[:f] = body[:f] * np.sin(t) + tail * np.cos(t)
    return body


def write_wav(path: Path, x: np.ndarray) -> None:
    import wave
    pcm = (np.clip(x, -1, 1) * 32767.0).round().astype("<i2")
    with wave.open(str(path), "wb") as w:
        w.setnchannels(x.shape[1])
        w.setsampwidth(2)
        w.setframerate(SR)
        w.writeframes(pcm.tobytes())


def encode_ogg(path: Path, x: np.ndarray, quality: float) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory() as td:
        wav = Path(td) / "e.wav"
        # 24-bit-ish precision is not needed; 16-bit PCM is plenty before Vorbis.
        write_wav(wav, x)
        subprocess.run(["ffmpeg", "-v", "error", "-nostdin", "-y", "-i", str(wav),
                        "-c:a", "libvorbis", "-q:a", str(quality), "-ar", str(SR),
                        "-map_metadata", "-1", "-fflags", "+bitexact",
                        "-flags:a", "+bitexact", str(path)], check=True)


# --------------------------------------------------------------------------
# sets


def set_id(st: dict) -> str:
    return f"{st['category']}/{st['name']}"


def set_defaults(st: dict) -> dict:
    kind = st.get("type", "oneshot")
    d = {
        "oneshot": {"channels": 1, "normalize": {"peak": -1.0}, "fade_in": 0.002,
                    "fade_out": 0.03, "quality": 4, "highpass": 40},
        "loop": {"channels": 2, "normalize": {"lufs": -16.0, "peak": -1.0}, "xfade": 2.0,
                 "quality": 4, "highpass": 30},
        "bed": {"channels": 2, "normalize": {"lufs": -16.0, "peak": -1.0}, "fade_in": 0.05,
                "fade_out": 0.5, "quality": 4, "highpass": 30},
    }[kind]
    out = dict(d)
    out.update({k: v for k, v in st.items() if k != "inputs"})
    out["inputs"] = st["inputs"]
    out["type"] = kind
    return out


def candidates_for_input(st: dict, inp: dict, path: Path) -> list[np.ndarray]:
    x = decode(path, st["channels"], inp.get("highpass", st.get("highpass")),
               inp.get("lowpass", st.get("lowpass")), inp.get("start"), inp.get("end"),
               inp.get("af", st.get("af")))
    if inp.get("gain_db"):
        x = x * undb(inp["gain_db"])
    split = dict(DEFAULT_SPLIT)
    split.update(st.get("split", {}))
    split.update(inp.get("split", {}))
    mode = split["mode"]
    if mode == "whole":
        return [x]
    if mode == "segments":
        return [x[int(a * SR):int(b * SR)] for a, b in split["segments"]]
    events = detect_events(x, split)
    if "pick" in inp:
        chosen = [events[i] for i in inp["pick"] if i < len(events)]
    else:
        good = [e for e in events if not e["weak"]]
        good.sort(key=lambda e: -e["score"])
        chosen = sorted(good[:inp.get("take", len(good))], key=lambda e: e["start"])
    return [x[int(e["start"] * SR):int(e["end"] * SR)] for e in chosen]


def finish_oneshot(st: dict, clip: np.ndarray) -> np.ndarray:
    clip = trim(clip, st.get("head_db", -30.0), st.get("tail_trim_db", -55.0))
    dur = len(clip) / SR
    fo = st.get("fade_out", 0.03)
    # Longer clips get longer fades, but a short click keeps most of its body.
    fo = min(max(fo, 0.15 * dur), 0.25, 0.35 * dur)
    clip = fade(clip, st.get("fade_in", 0.002), fo)
    return normalize(clip, st["normalize"])


def build_set(st: dict, srcs: dict, paths: dict) -> list[tuple[Path, np.ndarray, str]]:
    """Returns (output path, audio, source id) per variant."""
    st = set_defaults(st)
    outs = []
    if st["type"] in ("loop", "bed"):
        for k, inp in enumerate(st["inputs"]):
            path = input_file(inp["source"], srcs[inp["source"]], inp.get("member"),
                              paths[inp["source"]])
            start = inp.get("start", 0.0)
            length = inp.get("length", st.get("length", 30.0))
            if st["type"] == "loop":
                end = start + length + st["xfade"] + 0.15
            else:
                end = start + length
            x = decode(path, st["channels"], inp.get("highpass", st.get("highpass")),
                       inp.get("lowpass", st.get("lowpass")), start, end,
                       inp.get("af", st.get("af")))
            if st["type"] == "loop":
                x = make_loop(x, length, st["xfade"], st.get("tonal", False))
            else:
                x = fade(x, st["fade_in"], st["fade_out"])
            x = normalize(x, st["normalize"])
            outs.append((x, inp["source"]))
    else:
        per_input = []
        for inp in st["inputs"]:
            path = input_file(inp["source"], srcs[inp["source"]], inp.get("member"),
                              paths[inp["source"]])
            per_input.append([(c, inp["source"]) for c in candidates_for_input(st, inp, path)])
        # Round-robin across inputs so every source is represented.
        want = st.get("count", sum(len(c) for c in per_input))
        picked = []
        k = 0
        while len(picked) < want and any(per_input):
            lst = per_input[k % len(per_input)]
            if lst:
                picked.append(lst.pop(0))
            k += 1
        outs = [(finish_oneshot(st, c), sid) for c, sid in picked]
    name = st["name"]
    return [(OUT / st["category"] / f"{name}_{i + 1}.ogg", x, sid)
            for i, (x, sid) in enumerate(outs)]


# --------------------------------------------------------------------------
# inspection


def inspect(st: dict, srcs: dict, paths: dict, dest: Path) -> None:
    from PIL import Image, ImageDraw

    st = set_defaults(st)
    rows = []
    for inp in st["inputs"]:
        path = input_file(inp["source"], srcs[inp["source"]], inp.get("member"),
                          paths[inp["source"]])
        x = decode(path, 1, inp.get("highpass", st.get("highpass")),
                   inp.get("lowpass", st.get("lowpass")), inp.get("start"), inp.get("end"))
        split = dict(DEFAULT_SPLIT)
        split.update(st.get("split", {}))
        split.update(inp.get("split", {}))
        ev = detect_events(x, split) if split["mode"] == "events" else []
        rows.append((inp, x, ev, split))
    W, H = 1800, 220
    img = Image.new("RGB", (W, H * len(rows)), "white")
    d = ImageDraw.Draw(img)
    for r, (inp, x, ev, split) in enumerate(rows):
        y0 = r * H
        env = envelope_db(x[:, 0])
        dur = len(x) / SR
        sx = lambda t: int(t / dur * (W - 20)) + 10
        sy = lambda v: y0 + 20 + int((0 - max(v, -80)) / 80 * (H - 40))
        for s in range(int(dur) + 1):
            d.line([(sx(s), y0 + 15), (sx(s), y0 + H - 15)], fill=(225, 225, 225))
            if s % 5 == 0:
                d.text((sx(s) + 2, y0 + H - 14), f"{s}s", fill=(120, 120, 120))
        if split["mode"] == "segments":
            ev = [{"start": a, "end": b, "weak": False} for a, b in split["segments"]]
        for k, e in enumerate(ev):
            col = (255, 220, 220) if e.get("weak") else (210, 235, 210)
            d.rectangle([sx(e["start"]), y0 + 15, sx(e["end"]), y0 + H - 15], fill=col)
            d.text((sx(e["start"]) + 1, y0 + 16), str(k), fill=(0, 0, 160))
        pts = [(sx(i * ENV_HOP), sy(v)) for i, v in enumerate(env)]
        d.line(pts, fill=(30, 30, 30))
        d.text((10, y0 + 2), f"{inp['source']} {inp.get('member', '')} "
               f"[{inp.get('start', 0)}..{inp.get('end', '')}] events={len(ev)}", fill=(0, 0, 0))
        d.line([(0, y0 + H - 1), (W, y0 + H - 1)], fill=(0, 0, 0))
    dest.parent.mkdir(parents=True, exist_ok=True)
    img.save(dest)
    for r, (inp, x, ev, split) in enumerate(rows):
        log(f"{inp['source']}: " + "  ".join(
            f"{k}:{e['start']:.2f}-{e['end']:.2f}/{e['peak']:.0f}{'w' if e['weak'] else ''}"
            for k, e in enumerate(ev)))
    log(f"wrote {dest}")


# --------------------------------------------------------------------------
# licenses and credits


def check_license(sid: str, src: dict) -> str:
    page = src.get("page", "")
    if "freesound.org" not in page:
        return "skip (not freesound; checked by hand at intake)"
    html = http_get(page).decode("utf-8", "replace")
    lic = set(re.findall(r'href="(https?://creativecommons\.org/[^"]+)"', html))
    if any("publicdomain/zero" in l for l in lic):
        return "CC0 ok" if src["license"] == "CC0-1.0" else f"page says CC0, manifest says {src['license']}"
    return f"MISMATCH: page links {sorted(lic)}"


def credits_rows(man: dict, date: str) -> list[str]:
    """One CREDITS.md row per (set, source) pair that a built file uses."""
    names = {"CC0-1.0": "CC0 1.0", "CC-BY-3.0": "CC BY 3.0", "CC-BY-4.0": "CC BY 4.0"}
    index_path = OUT / "sounds.json"
    index = json.loads(index_path.read_text()) if index_path.exists() else {}
    rows = []
    for st in man["sets"]:
        seen = []
        for inp in st["inputs"]:
            if inp["source"] not in seen:
                seen.append(inp["source"])
        built = index.get(set_id(st))
        if built:
            # Only credit recordings that ended up in a shipped file.
            used = {f.get("source") for f in built["files"]}
            seen = [sid for sid in seen if sid in used]
        for sid in seen:
            src = man["sources"][sid]
            glob = f"`assets/audio/{set_id(st)}_*.ogg`"
            where = "Freesound" if "freesound.org" in src["page"] else "Kenney"
            what = f"{where} \"{src.get('title', sid)}\""
            if where == "Kenney":
                members = sorted({Path(i["member"]).name for i in st["inputs"]
                                  if i["source"] == sid and "member" in i})
                what += " (" + ", ".join(members) + ")"
            rows.append(f"| {glob} | {what} — <{src['page']}> | {src['author']} | "
                        f"{names.get(src['license'], src['license'])} | {date} |")
    return rows


# --------------------------------------------------------------------------


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--only", nargs="*", help="set ids (footsteps/grass) or categories to build")
    ap.add_argument("--offline", action="store_true", help="use cached sources only")
    ap.add_argument("--repin", action="store_true", help="accept changed source hashes")
    ap.add_argument("--fetch-only", action="store_true", help="download and verify, do not master")
    ap.add_argument("--inspect", metavar="SET", help="render the split of a set (category/name) to a PNG")
    ap.add_argument("--check-licenses", action="store_true", help="re-read freesound license links")
    ap.add_argument("--credits", metavar="DATE", nargs="?", const="", help="print CREDITS.md rows")
    args = ap.parse_args()

    man = json.loads(MANIFEST.read_text())
    srcs = man["sources"]
    sets = man["sets"]
    names = [set_id(s) for s in sets]
    if len(set(names)) != len(names):
        sys.exit("duplicate set ids (category/name) in manifest")

    if args.credits is not None:
        import datetime
        date = args.credits or datetime.date.today().strftime("%Y-%m")
        print("\n".join(credits_rows(man, date)))
        return 0

    if args.check_licenses:
        bad = 0
        for sid, src in srcs.items():
            try:
                res = check_license(sid, src)
            except Exception as e:  # noqa: BLE001
                res = f"MISMATCH? could not read the page: {e}"
            time.sleep(1.0)  # be polite; Freesound rate-limits page views
            bad += res.startswith("MISMATCH")
            log(f"{sid:28} {res}")
        return 1 if bad else 0

    need_tool("ffmpeg")
    need_tool("ffprobe")
    if args.inspect:
        sets = [s for s in sets if set_id(s) == args.inspect]
        if not sets:
            sys.exit(f"no set named {args.inspect}")
    elif args.only:
        want = lambda s: set_id(s) in args.only or s["category"] in args.only
        unknown = [o for o in args.only if o not in names and not any(s["category"] == o for s in sets)]
        if unknown:
            sys.exit(f"unknown sets/categories: {unknown}")
        sets = [s for s in sets if want(s)]

    used = []
    for st in sets:
        for inp in st["inputs"]:
            if inp["source"] not in srcs:
                sys.exit(f"{st['name']}: unknown source {inp['source']}")
            if inp["source"] not in used:
                used.append(inp["source"])

    paths = {}
    dirty = False
    failed = []
    for sid in used:
        try:
            paths[sid], changed = fetch_source(sid, srcs[sid], args.offline, args.repin)
            dirty |= changed
        except Exception as e:  # noqa: BLE001 - report and carry on
            failed.append(sid)
            log(f"  FAILED {sid}: {e}")
    if dirty:
        MANIFEST.write_text(json.dumps(man, indent=2, ensure_ascii=False) + "\n")
        log("pinned new sha256s in the manifest")
    if args.fetch_only:
        return 1 if failed else 0

    if args.inspect:
        inspect(sets[0], srcs, paths, CACHE / "inspect" / f"{args.inspect.replace('/', '_')}.png")
        return 0

    index_path = OUT / "sounds.json"
    index = json.loads(index_path.read_text()) if index_path.exists() else {}
    for st in sets:
        if any(inp["source"] in failed for inp in st["inputs"]):
            log(f"skip {set_id(st)} (missing source)")
            continue
        log(f"master {set_id(st)}")
        try:
            outs = build_set(st, srcs, paths)
        except Exception as e:  # noqa: BLE001 - one bad set should not stop the rest
            log(f"  FAILED {set_id(st)}: {e}")
            failed.append(set_id(st))
            continue
        cat_dir = OUT / st["category"]
        if cat_dir.exists():
            # Remove stale variants left over from a bigger previous build.
            for old in cat_dir.glob(f"{st['name']}_*.ogg"):
                if re.fullmatch(re.escape(st["name"]) + r"_\d+\.ogg", old.name):
                    old.unlink()
        files = []
        for path, x, sid in outs:
            encode_ogg(path, x, set_defaults(st)["quality"])
            files.append({"file": str(path.relative_to(ROOT / "assets")),
                          "seconds": round(len(x) / SR, 3), "source": sid})
        d = set_defaults(st)
        index[set_id(st)] = {
            "type": d["type"], "channels": d["channels"],
            "loop": d["type"] == "loop", "files": files,
        }
        log(f"  -> {len(files)} file(s) in {cat_dir.relative_to(ROOT)}")
    index = {k: index[k] for k in sorted(index) if k in names}
    index_path.parent.mkdir(parents=True, exist_ok=True)
    index_path.write_text(json.dumps(index, indent=2) + "\n")
    if failed:
        log(f"\n{len(failed)} source(s)/set(s) failed: {', '.join(failed)}")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
