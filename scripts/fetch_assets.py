#!/usr/bin/env python3
"""Fetch CC0 texture sources and build the game's KTX2 textures.

    python3 scripts/fetch_assets.py              # fetch + build everything
    python3 scripts/fetch_assets.py terrain sky  # only some groups
    python3 scripts/fetch_assets.py --offline    # build from assets/.cache only
    python3 scripts/fetch_assets.py --force      # rebuild even if up to date
    python3 scripts/fetch_assets.py --credits    # print CREDITS.md rows

Inputs live in scripts/assets_manifest.json. On the first fetch the script
resolves each source to concrete URLs (Poly Haven / ambientCG APIs) and
writes url + sha256 + author back into the manifest; later runs download
from the recorded URLs and refuse files whose hash changed (pass
--update-hashes to accept a deliberate upstream change).

Downloads go to assets/.cache/ (gitignored). Outputs go to assets/textures/:

    terrain/{albedo,normal,arm}.ktx2          2D arrays, 7 layers, 1024, full mips
    materials/<name>/{albedo,normal,arm}.ktx2 single 2D, 1024, full mips
    foliage/<name>/{albedo,normal,arm}.ktx2   RGBA cards (+ albedo.png)
    sky/<name>.hdr                            HDRI, copied verbatim

KTX2 files are RGBA8 (R8G8B8A8_SRGB for albedo, _UNORM for the rest), with
every mip level generated here: albedo filtered in linear light, normals
renormalised with Toksvig-style roughness widening, foliage alpha scaled
to keep its coverage. The container is written by this script and then
zstd-supercompressed with KTX-Software's `ktx deflate` when `ktx` is on
PATH (or under /opt/ktx-software-*/bin); without it the files are left
uncompressed (about 3x larger) and a warning is printed.

Normal maps are OpenGL convention (+Y = up in the image, green = towards
the top row). ARM packs R = ambient occlusion, G = perceptual roughness,
B = metallic.

Only the Python standard library, numpy and Pillow are used.
"""

from __future__ import annotations

import argparse
import concurrent.futures
import glob
import hashlib
import io
import json
import os
import shutil
import struct
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
import zipfile
from pathlib import Path

import numpy as np
from PIL import Image

# Build stamps include this, the script's own hash, the output format and
# whether `ktx` was available, so any change rebuilds what it affects.
PIPELINE_VERSION = 4
OPTIONS = {"format": "rgba8"}

ROOT = Path(__file__).resolve().parent.parent
MANIFEST = ROOT / "scripts" / "assets_manifest.json"
CACHE = ROOT / "assets" / ".cache"
OUT = ROOT / "assets" / "textures"
STAMPS = CACHE / "build_stamps.json"
USER_AGENT = "extraction-asset-pipeline/1.0 (+https://github.com/CassidyPrather)"
GROUPS = ("terrain", "materials", "foliage", "sky")

VK_FORMAT_R8G8B8A8_UNORM = 37
VK_FORMAT_R8G8B8A8_SRGB = 43
ZSTD_LEVEL = 18
ALPHA_CUTOFF = 0.5

Image.MAX_IMAGE_PIXELS = 1 << 28


def log(msg: str) -> None:
    print(msg, flush=True)


# ---------------------------------------------------------------------------
# Networking and the cache
# ---------------------------------------------------------------------------


def http_get(url: str, tries: int = 4) -> bytes:
    req = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    for attempt in range(tries):
        try:
            with urllib.request.urlopen(req, timeout=120) as r:
                return r.read()
        except (urllib.error.URLError, TimeoutError, ConnectionError) as e:
            if attempt == tries - 1:
                raise RuntimeError(f"GET {url} failed: {e}") from e
            time.sleep(2 * (attempt + 1))
    raise AssertionError


def http_json(url: str):
    return json.loads(http_get(url))


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def provider_page(src: dict) -> str:
    if src["provider"] == "polyhaven":
        return f"https://polyhaven.com/a/{src['id']}"
    return f"https://ambientcg.com/a/{src['id']}"


def resolve_source(key: str, src: dict) -> None:
    """Fill in src['files'] (name -> url) and src['author'] from the APIs."""
    prov, aid, res = src["provider"], src["id"], src["res"]
    files: dict[str, dict] = {}
    if prov == "polyhaven":
        listing = http_json(f"https://api.polyhaven.com/files/{aid}")
        info = http_json(f"https://api.polyhaven.com/info/{aid}")
        src["author"] = ", ".join(info.get("authors", {}).keys()) or "Poly Haven"
        if src.get("kind") == "hdri":
            entry = listing["hdri"][res]["hdr"]
            files[Path(entry["url"]).name] = {"role": "hdri", "url": entry["url"]}
        else:
            maps = src.get("maps") or {"albedo": "Diffuse", "normal": "nor_gl", "arm": "arm"}
            for role, mkey in maps.items():
                fmt = "png" if role == "alpha" else "jpg"
                entry = listing[mkey][res][fmt]
                files[Path(entry["url"]).name] = {"role": role, "url": entry["url"]}
    elif prov == "ambientcg":
        name = f"{aid}_{res}.zip"
        files[name] = {"role": "zip", "url": f"https://ambientcg.com/get?file={name}"}
        src["author"] = "Lennart Demes (ambientCG)"
    else:
        raise ValueError(f"{key}: unknown provider {prov}")
    src["license"] = "CC0-1.0"
    src["page"] = provider_page(src)
    src["files"] = files


def cache_path(src: dict, fname: str) -> Path:
    return CACHE / src["provider"] / src["id"] / fname


def fetch_file(src: dict, fname: str, meta: dict, offline: bool, update: bool) -> bool:
    """Ensure one file is in the cache with the recorded hash. Returns True
    if the manifest entry changed."""
    path = cache_path(src, fname)
    want = meta.get("sha256")
    if path.exists():
        have = sha256_file(path)
        if want is None:
            meta["sha256"], meta["bytes"] = have, path.stat().st_size
            return True
        if have == want:
            return False
        log(f"  cache file {path} does not match the manifest hash; re-downloading")
    if offline:
        raise RuntimeError(f"--offline but {path} is missing or stale")
    data = http_get(meta["url"])
    got = sha256(data)
    changed = False
    if want is not None and got != want:
        if not update:
            raise RuntimeError(
                f"{meta['url']}: sha256 {got} != manifest {want}. Upstream changed; "
                "inspect it and rerun with --update-hashes to accept."
            )
        log(f"  accepting new hash for {fname}")
    if want != got:
        meta["sha256"], meta["bytes"], changed = got, len(data), True
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".part")
    tmp.write_bytes(data)
    tmp.replace(path)
    log(f"  fetched {src['provider']}/{src['id']}/{fname} ({len(data) / 1e6:.1f} MB)")
    return changed


def fetch_sources(manifest: dict, keys: set[str], offline: bool, update: bool) -> bool:
    changed = False
    sources = manifest["sources"]
    for key in sorted(keys):
        src = sources[key]
        if "files" not in src:
            if offline:
                raise RuntimeError(f"{key} was never resolved; run once without --offline")
            log(f"resolving {key}")
            resolve_source(key, src)
            changed = True
    jobs = [(sources[k], fn, meta) for k in sorted(keys) for fn, meta in sources[k]["files"].items()]
    with concurrent.futures.ThreadPoolExecutor(6) as ex:
        futs = [ex.submit(fetch_file, s, fn, m, offline, update) for s, fn, m in jobs]
        for f in futs:
            changed |= f.result()
    return changed


# ---------------------------------------------------------------------------
# Loading source maps
# ---------------------------------------------------------------------------


def _decode(data: bytes) -> Image.Image:
    im = Image.open(io.BytesIO(data))
    im.load()
    return im


def to_float(im: Image.Image, channels: int) -> np.ndarray:
    """Image -> float32 array in 0..1 with the requested channel count."""
    if im.mode in ("I;16", "I;16B", "I"):
        a = np.asarray(im, dtype=np.float32) / 65535.0
        a = a[..., None]
    else:
        if channels == 1:
            im = im.convert("L") if im.mode not in ("L", "F") else im
        elif channels == 3:
            im = im.convert("RGB")
        else:
            im = im.convert("RGBA")
        a = np.asarray(im, dtype=np.float32) / 255.0
        if a.ndim == 2:
            a = a[..., None]
    if a.shape[2] != channels:
        a = np.repeat(a[..., :1], channels, axis=2) if channels > 1 else a[..., :1]
    return a


def load_source_maps(src: dict) -> dict[str, np.ndarray]:
    """Returns float32 maps: albedo (sRGB-encoded rgb), normal (xyz, -1..1,
    OpenGL), ao, rough, metal, and optionally alpha, height."""
    raw: dict[str, Image.Image] = {}
    if src["provider"] == "polyhaven":
        for fname, meta in src["files"].items():
            raw[meta["role"]] = _decode(cache_path(src, fname).read_bytes())
    else:
        (fname,) = src["files"].keys()
        suffixes = {
            "albedo": "_Color", "normal": "_NormalGL", "rough": "_Roughness",
            "ao": "_AmbientOcclusion", "metal": "_Metalness", "height": "_Displacement",
            "alpha": "_Opacity",
        }
        with zipfile.ZipFile(cache_path(src, fname)) as z:
            names = z.namelist()
            for role, suf in suffixes.items():
                hits = [n for n in names if Path(n).stem.endswith(suf) and n.lower().endswith((".jpg", ".png"))]
                if hits:
                    raw[role] = _decode(z.read(sorted(hits)[0]))
    out: dict[str, np.ndarray] = {}
    out["albedo"] = to_float(raw["albedo"], 3)
    nrm = to_float(raw["normal"], 3)
    bias = nrm[::8, ::8, :2].mean(axis=(0, 1))
    if np.all(np.abs(bias - 0.735) < 0.05):
        # Some uploads (e.g. Poly Haven rocks_ground_09) store the normal map
        # sRGB-encoded: flat reads 0.735 instead of 0.5. Undo the gamma.
        log(f"  {src['id']}: normal map looks sRGB-encoded (mean xy {bias.round(3)}); decoding")
        nrm = srgb_to_linear(nrm)
    out["normal"] = nrm * 2.0 - 1.0
    h, w = out["albedo"].shape[:2]
    if "arm" in raw:
        arm = to_float(raw["arm"], 3)
        out["ao"], out["rough"], out["metal"] = arm[..., 0], arm[..., 1], arm[..., 2]
    else:
        out["rough"] = to_float(raw["rough"], 1)[..., 0] if "rough" in raw else np.full((h, w), 0.8, np.float32)
        out["metal"] = to_float(raw["metal"], 1)[..., 0] if "metal" in raw else np.zeros((h, w), np.float32)
        if "ao" in raw:
            out["ao"] = to_float(raw["ao"], 1)[..., 0]
        elif "height" in raw:
            out["ao"] = ao_from_height(to_float(raw["height"], 1)[..., 0])
        else:
            out["ao"] = np.ones((h, w), np.float32)
    if "alpha" in raw:
        out["alpha"] = to_float(raw["alpha"], 1)[..., 0]
    if "height" in raw:
        out["height"] = to_float(raw["height"], 1)[..., 0]
    for k, v in out.items():
        if v.shape[:2] != (h, w):
            out[k] = resize(v, w, h, wrap=False)
    return out


# ---------------------------------------------------------------------------
# Image maths
# ---------------------------------------------------------------------------


def srgb_to_linear(c: np.ndarray) -> np.ndarray:
    return np.where(c <= 0.04045, c / 12.92, ((c + 0.055) / 1.055) ** 2.4).astype(np.float32)


def linear_to_srgb(c: np.ndarray) -> np.ndarray:
    c = np.clip(c, 0.0, 1.0)
    return np.where(c <= 0.0031308, c * 12.92, 1.055 * np.power(c, 1 / 2.4) - 0.055).astype(np.float32)


def _resize_plane(p: np.ndarray, w: int, h: int) -> np.ndarray:
    return np.asarray(Image.fromarray(p.astype(np.float32)).resize((w, h), Image.Resampling.LANCZOS))


def resize(a: np.ndarray, w: int, h: int, wrap: bool) -> np.ndarray:
    """Lanczos resize of a float map. With wrap=True the image is treated as
    tiling, so the result tiles seamlessly too."""
    if a.shape[1] == w and a.shape[0] == h:
        return a.astype(np.float32)
    planes = a[..., None] if a.ndim == 2 else a
    sh, sw = planes.shape[:2]
    out = []
    for c in range(planes.shape[2]):
        p = planes[..., c]
        if wrap:
            # Pad by a margin that maps to a whole number of output pixels.
            m_out = 8
            m_in_x = int(round(m_out * sw / w))
            m_in_y = int(round(m_out * sh / h))
            p = np.pad(p, ((m_in_y, m_in_y), (m_in_x, m_in_x)), mode="wrap")
            r = _resize_plane(p, w + 2 * m_out, h + 2 * m_out)[m_out:-m_out, m_out:-m_out]
        else:
            r = _resize_plane(p, w, h)
        out.append(r)
    r = np.stack(out, axis=-1)
    return r[..., 0] if a.ndim == 2 else r


def box2(a: np.ndarray) -> np.ndarray:
    """2x2 box downsample (exact for power-of-two tiles, keeps tiling)."""
    return 0.25 * (a[0::2, 0::2] + a[1::2, 0::2] + a[0::2, 1::2] + a[1::2, 1::2])


def blur_wrap(a: np.ndarray, sigma: float) -> np.ndarray:
    """Gaussian blur with wrap-around, via FFT."""
    h, w = a.shape
    fy = np.fft.fftfreq(h)[:, None]
    fx = np.fft.rfftfreq(w)[None, :]
    g = np.exp(-2 * (np.pi * sigma) ** 2 * (fx * fx + fy * fy))
    return np.fft.irfft2(np.fft.rfft2(a) * g, s=(h, w)).astype(np.float32)


def normalize(v: np.ndarray) -> np.ndarray:
    n = np.linalg.norm(v, axis=-1, keepdims=True)
    return (v / np.maximum(n, 1e-8)).astype(np.float32)


def normal_from_height(hgt: np.ndarray, strength: float) -> np.ndarray:
    """OpenGL-convention tangent normal from a tiling height map (pixels)."""
    hx = 0.5 * (np.roll(hgt, -1, axis=1) - np.roll(hgt, 1, axis=1))
    hy = 0.5 * (np.roll(hgt, -1, axis=0) - np.roll(hgt, 1, axis=0))  # +y = down the image
    n = np.stack([-hx * strength, hy * strength, np.ones_like(hgt)], axis=-1)
    return normalize(n)


def ao_from_height(hgt: np.ndarray, radius: float = 6.0, strength: float = 4.0) -> np.ndarray:
    cav = np.maximum(blur_wrap(hgt, radius) - hgt, 0.0)
    return np.clip(1.0 - strength * cav, 0.35, 1.0).astype(np.float32)


def periodic_noise(size: int, rng: np.random.Generator, beta: float = 2.0,
                   sx: float = 1.0, sy: float = 1.0, fmin: float = 1.0) -> np.ndarray:
    """Tileable 1/f^beta noise in 0..1. sx/sy stretch features along x/y."""
    white = rng.standard_normal((size, size))
    fy = np.fft.fftfreq(size)[:, None] * size / sy
    fx = np.fft.rfftfreq(size)[None, :] * size / sx
    f = np.sqrt(fx * fx + fy * fy)
    amp = np.where(f >= fmin, 1.0 / np.maximum(f, 1e-6) ** (beta / 2), 0.0)
    n = np.fft.irfft2(np.fft.rfft2(white) * amp, s=(size, size))
    n = (n - n.min()) / max(n.max() - n.min(), 1e-8)
    return n.astype(np.float32)


# ---------------------------------------------------------------------------
# Materials (in-memory form: float maps at the output size)
# ---------------------------------------------------------------------------


def make_material(maps: dict, size: int, repeat: int = 1, wrap: bool = True) -> dict:
    out = {}
    for k, v in maps.items():
        if repeat > 1:
            v = np.tile(v, (repeat, repeat) + (1,) * (v.ndim - 2))
        out[k] = resize(v, size, size, wrap=wrap)
    out["albedo"] = np.clip(out["albedo"], 0, 1)
    out["normal"] = normalize(out["normal"])
    for k in ("ao", "rough", "metal", "alpha"):
        if k in out:
            out[k] = np.clip(out[k], 0, 1)
    return out


def grade(mat: dict, cfg: dict) -> None:
    lin = srgb_to_linear(mat["albedo"])
    lum = lin @ np.array([0.2126, 0.7152, 0.0722], np.float32)
    if "recolor" in cfg:
        target = srgb_to_linear(np.array(cfg["recolor"], np.float32) / 255.0)
        rel = lum / max(float(lum.mean()), 1e-6)
        lin = target[None, None, :] * rel[..., None]
    if "saturation" in cfg:
        s = float(cfg["saturation"])
        lin = lum[..., None] + s * (lin - lum[..., None])
    if "tint" in cfg:
        lin = lin * np.array(cfg["tint"], np.float32)[None, None, :]
    if "brightness" in cfg:
        lin = lin * float(cfg["brightness"])
    mat["albedo"] = linear_to_srgb(lin)
    if "roughness" in cfg:
        r = cfg["roughness"]
        rough = mat["rough"] * r.get("scale", 1.0) + r.get("offset", 0.0)
        mat["rough"] = np.clip(rough, r.get("min", 0.0), r.get("max", 1.0)).astype(np.float32)
    if "metallic" in cfg:
        mat["metal"] = np.full_like(mat["rough"], float(cfg["metallic"]))


# ---- procedural materials -------------------------------------------------


def gen_birch_bark(size: int, seed: int) -> dict:
    """White birch bark. Trunk axis runs along V (image vertical), so the
    lenticels are horizontal dashes."""
    rng = np.random.default_rng(seed)
    # Base tone: cream white with grey-pink mottling, faint horizontal banding.
    mottle = periodic_noise(size, rng, beta=2.6, fmin=2)
    band = periodic_noise(size, rng, beta=2.2, sx=8.0, sy=0.6, fmin=1)
    fine = periodic_noise(size, rng, beta=1.2, sx=3.0, sy=0.5, fmin=8)
    base = np.array([0.60, 0.58, 0.53], np.float32)  # linear: cream white
    grey = np.array([0.30, 0.29, 0.27], np.float32)
    pink = np.array([0.50, 0.40, 0.33], np.float32)
    t = np.clip((mottle - 0.55) * 2.0, 0, 1)[..., None] * 0.7
    col = base * (1 - t) + grey * t
    p = np.clip((band - 0.6) * 3.0, 0, 1)[..., None] * 0.3
    col = col * (1 - p) + pink * p
    col *= (0.97 + 0.05 * fine)[..., None]
    height = 0.5 + 0.15 * (fine - 0.5) + 0.1 * (band - 0.5)

    # Lenticels: short dark horizontal dashes, sunk into the bark.
    yy, xx = np.mgrid[0:size, 0:size].astype(np.float32)
    lent = np.zeros((size, size), np.float32)
    n_lent = int(size * size / 11000)
    for _ in range(n_lent):
        cx, cy = rng.uniform(0, size), rng.uniform(0, size)
        ln = rng.uniform(0.008, 0.04) * size
        th = rng.uniform(1.2, 3.0) * size / 1024
        x0, x1 = int(cx - ln), int(cx + ln) + 1
        y0, y1 = int(cy - 4 * th), int(cy + 4 * th) + 1
        xs = np.arange(x0, x1) % size
        ys = np.arange(y0, y1) % size
        dx = (np.arange(x0, x1) - cx)[None, :] / ln
        dy = (np.arange(y0, y1) - cy)[:, None] / th
        d = dx * dx + dy * dy
        v = np.clip(1.0 - d, 0, 1) ** 0.3 * rng.uniform(0.75, 1.0)
        lent[np.ix_(ys, xs)] = np.maximum(lent[np.ix_(ys, xs)], v)
    # Black fissured patches (old bark plates / branch scars).
    blot = periodic_noise(size, rng, beta=3.0, sx=2.5, sy=1.0, fmin=2)
    crack = periodic_noise(size, rng, beta=1.6, sx=1.0, sy=4.0, fmin=6)
    patch = np.clip((blot - 0.68) * 6.0, 0, 1)
    patch = np.clip(patch * (0.6 + 0.8 * crack), 0, 1)
    dark = np.array([0.025, 0.022, 0.02], np.float32)
    lent_col = np.array([0.05, 0.042, 0.038], np.float32)
    col = col * (1 - lent[..., None]) + lent_col * lent[..., None]
    col = col * (1 - patch[..., None]) + dark * (0.7 + 0.6 * crack[..., None]) * patch[..., None]
    height = height - 0.35 * lent + patch * (0.25 * crack - 0.05)

    albedo = linear_to_srgb(col)
    rough = np.clip(0.62 + 0.25 * patch + 0.1 * lent + 0.06 * (mottle - 0.5), 0, 1)
    return {
        "albedo": albedo,
        "normal": normal_from_height(height * size / 64.0, 1.0),
        "ao": ao_from_height(height, radius=3.0, strength=3.0),
        "rough": rough.astype(np.float32),
        "metal": np.zeros((size, size), np.float32),
    }


def gen_robot_shell(size: int, seed: int) -> dict:
    """Clean off-white painted panels: big flat plates, recessed seams, a few
    fasteners, orange-peel micro texture, faint grime in the seams."""
    rng = np.random.default_rng(seed)
    h = np.zeros((size, size), np.float32)
    seam = np.zeros((size, size), np.float32)
    # Panel grid: rows of different heights, columns offset per row.
    rows = [0]
    while rows[-1] < size:
        rows.append(rows[-1] + int(rng.choice([size // 4, size // 2, size // 4])))
    rows = [r for r in rows if r < size]
    yy, xx = np.mgrid[0:size, 0:size]
    sw = max(2, size // 400)
    for i, r in enumerate(rows):
        r1 = rows[i + 1] if i + 1 < len(rows) else size
        seam[np.abs(yy - r) <= sw] = 1.0
        n_cols = int(rng.choice([1, 2, 2, 3]))
        off = int(rng.uniform(0, size))
        for c in range(n_cols):
            x = (off + c * size // n_cols) % size
            band = (yy >= r) & (yy < r1)
            seam[band & (np.abs(((xx - x + size // 2) % size) - size // 2) <= sw)] = 1.0
    seam = np.clip(blur_wrap(seam, 1.0) * 1.4, 0, 1)
    # Fasteners near seams.
    bolts = np.zeros((size, size), np.float32)
    sy, sx = np.nonzero(seam > 0.9)
    for _ in range(18):
        j = rng.integers(len(sy))
        cy, cx = sy[j] + rng.choice([-1, 1]) * size // 60, sx[j] + rng.choice([-1, 1]) * size // 60
        d = np.hypot(((yy - cy + size // 2) % size) - size // 2, ((xx - cx + size // 2) % size) - size // 2)
        rad = size / 160
        bolts = np.maximum(bolts, np.clip(1.0 - (d / rad) ** 2, 0, 1))
    peel = periodic_noise(size, rng, beta=1.0, fmin=size / 16)
    broad = periodic_noise(size, rng, beta=3.0, fmin=1)
    smudge = periodic_noise(size, rng, beta=2.4, fmin=3)
    h = -0.8 * seam + 0.35 * bolts + 0.02 * (peel - 0.5) + 0.05 * (broad - 0.5)
    base = np.array([0.72, 0.71, 0.67], np.float32)  # linear: off-white
    col = base[None, None, :] * (0.96 + 0.06 * broad[..., None])
    grime = np.clip(blur_wrap(seam, 4.0) * 1.5, 0, 1) * (0.4 + 0.6 * smudge)
    col = col * (1 - 0.45 * grime[..., None])
    col = col * (1 - 0.15 * bolts[..., None])
    rough = 0.32 + 0.12 * smudge + 0.25 * grime - 0.05 * bolts
    return {
        "albedo": linear_to_srgb(col),
        "normal": normal_from_height(h * size / 200.0, 1.0),
        "ao": np.clip(1.0 - 0.55 * blur_wrap(seam, 2.0), 0, 1).astype(np.float32),
        "rough": np.clip(rough, 0, 1).astype(np.float32),
        "metal": np.zeros((size, size), np.float32),
    }


# ---- foliage --------------------------------------------------------------


def dilate_colors(mat: dict, iterations: int = 48) -> None:
    """Bleed albedo/normal/arm out of the leaves into transparent texels so
    mips and bilinear filtering never pull in junk from outside the cut."""
    mask = (mat["alpha"] >= 0.02).astype(np.float32)
    planes = {
        "albedo": mat["albedo"],
        "normal": mat["normal"],
        "arm": np.stack([mat["ao"], mat["rough"], mat["metal"]], axis=-1),
    }
    acc = {k: v * mask[..., None] for k, v in planes.items()}
    w = mask.copy()
    known = mask > 0
    for _ in range(iterations):
        if known.all():
            break
        nw = np.zeros_like(w)
        nacc = {k: np.zeros_like(v) for k, v in acc.items()}
        for dy, dx in ((0, 1), (0, -1), (1, 0), (-1, 0)):
            sw = np.roll(w, (dy, dx), axis=(0, 1))
            nw += sw
            for k in acc:
                nacc[k] += np.roll(acc[k], (dy, dx), axis=(0, 1))
        grow = (~known) & (nw > 0)
        for k in acc:
            acc[k][grow] = nacc[k][grow] / nw[grow][:, None]
        w[grow] = 1.0
        known |= grow
    for k, v in planes.items():
        filled = np.where(known[..., None], acc[k], acc[k][known].mean(axis=0) if known.any() else v)
        filled = np.where(mask[..., None] > 0, v, filled)
        planes[k] = filled.astype(np.float32)
    mat["albedo"] = planes["albedo"]
    mat["normal"] = normalize(planes["normal"])
    mat["ao"], mat["rough"], mat["metal"] = (planes["arm"][..., i] for i in range(3))


def extract_leaves(maps: dict, rotate_to_up: float) -> list[dict]:
    """Cut each leaf out of an ambientCG leaf-set atlas (connected regions of
    the opacity map). Returns RGBA-ish crops, rotated so the tip points up."""
    alpha = maps["alpha"]
    small = np.asarray(Image.fromarray((alpha * 255).astype(np.uint8)).resize((256, 256), Image.Resampling.BILINEAR)) > 128
    seen = np.zeros_like(small)
    boxes = []
    for y0 in range(256):
        for x0 in range(256):
            if small[y0, x0] and not seen[y0, x0]:
                stack = [(y0, x0)]
                seen[y0, x0] = True
                ys, xs = [], []
                while stack:
                    y, x = stack.pop()
                    ys.append(y)
                    xs.append(x)
                    for ny, nx in ((y + 1, x), (y - 1, x), (y, x + 1), (y, x - 1)):
                        if 0 <= ny < 256 and 0 <= nx < 256 and small[ny, nx] and not seen[ny, nx]:
                            seen[ny, nx] = True
                            stack.append((ny, nx))
                if len(ys) > 400:  # a real leaf, not a speck
                    boxes.append((min(ys), max(ys) + 1, min(xs), max(xs) + 1, len(ys)))
    s = alpha.shape[0] / 256
    leaves = []
    for y0, y1, x0, x1, _ in boxes:
        Y0, Y1 = max(int((y0 - 2) * s), 0), min(int((y1 + 2) * s), alpha.shape[0])
        X0, X1 = max(int((x0 - 2) * s), 0), min(int((x1 + 2) * s), alpha.shape[1])
        crop = {k: maps[k][Y0:Y1, X0:X1] for k in ("albedo", "normal", "rough", "alpha")}
        # Keep only the main component inside the crop.
        leaves.append(rotate_leaf(crop, rotate_to_up))
    return leaves


def rotate_leaf(leaf: dict, deg: float, scale: float = 1.0) -> dict:
    """Rotate (counter-clockwise, degrees) and scale a leaf crop, rotating
    the normal vectors with it."""
    h, w = leaf["alpha"].shape
    nw, nh = max(1, int(round(w * scale))), max(1, int(round(h * scale)))

    def tf(p: np.ndarray) -> np.ndarray:
        im = Image.fromarray(p.astype(np.float32))
        if (nw, nh) != (w, h):
            im = im.resize((nw, nh), Image.Resampling.LANCZOS)
        if deg:
            im = im.rotate(deg, resample=Image.Resampling.BICUBIC, expand=True)
        return np.asarray(im)

    out = {}
    for k in ("albedo", "normal"):
        out[k] = np.stack([tf(leaf[k][..., c]) for c in range(3)], axis=-1)
    out["rough"] = tf(leaf["rough"])
    out["alpha"] = np.clip(tf(leaf["alpha"]), 0, 1)
    if deg:
        t = np.radians(deg)
        x, y = out["normal"][..., 0].copy(), out["normal"][..., 1].copy()
        out["normal"][..., 0] = x * np.cos(t) - y * np.sin(t)
        out["normal"][..., 1] = x * np.sin(t) + y * np.cos(t)
    return out


def gen_birch_cluster(size: int, seed: int, leaf_maps: list[dict]) -> dict:
    """Composite a drooping birch twig cluster card from scanned leaves."""
    rng = np.random.default_rng(seed)
    S = size * 2  # supersample, downsampled at the end
    albedo = np.zeros((S, S, 3), np.float32)
    normal = np.zeros((S, S, 3), np.float32)
    normal[..., 2] = 1.0
    rough = np.full((S, S), 0.7, np.float32)
    alpha = np.zeros((S, S), np.float32)
    ao = np.ones((S, S), np.float32)
    yy, xx = np.mgrid[0:S, 0:S]

    # LeafSet014 leaves point up; LeafSet007 leaves point right.
    pools = [extract_leaves(leaf_maps[0], 0.0), extract_leaves(leaf_maps[1], 90.0)]

    def paste(lf: dict, cx: float, cy: float, shade: float) -> None:
        a = lf["alpha"]
        h, w = a.shape
        x0, y0 = int(cx - w / 2), int(cy - h / 2)
        xa, ya = max(x0, 0), max(y0, 0)
        xb, yb = min(x0 + w, S), min(y0 + h, S)
        if xa >= xb or ya >= yb:
            return
        sl = (slice(ya, yb), slice(xa, xb))
        ls = (slice(ya - y0, yb - y0), slice(xa - x0, xb - x0))
        aa = a[ls]
        albedo[sl] = albedo[sl] * (1 - aa[..., None]) + lf["albedo_lin"][ls] * shade * aa[..., None]
        normal[sl] = normal[sl] * (1 - aa[..., None]) + lf["normal"][ls] * aa[..., None]
        rough[sl] = rough[sl] * (1 - aa) + lf["rough"][ls] * aa
        ao[sl] = ao[sl] * (1 - aa) + shade * aa
        alpha[sl] = alpha[sl] + aa * (1 - alpha[sl])

    def draw_twig(points: np.ndarray, w0: float, w1: float) -> None:
        bark = srgb_to_linear(np.array([0.30, 0.20, 0.16], np.float32))
        n = len(points)
        for i in range(n - 1):
            p0, p1 = points[i], points[i + 1]
            wdt = w0 + (w1 - w0) * i / (n - 1)
            xmin, xmax = int(min(p0[0], p1[0]) - wdt - 2), int(max(p0[0], p1[0]) + wdt + 2)
            ymin, ymax = int(min(p0[1], p1[1]) - wdt - 2), int(max(p0[1], p1[1]) + wdt + 2)
            xmin, ymin, xmax, ymax = max(xmin, 0), max(ymin, 0), min(xmax, S), min(ymax, S)
            if xmin >= xmax or ymin >= ymax:
                continue
            X, Y = xx[ymin:ymax, xmin:xmax], yy[ymin:ymax, xmin:xmax]
            d = p1 - p0
            L2 = max(float(d @ d), 1e-6)
            t = np.clip(((X - p0[0]) * d[0] + (Y - p0[1]) * d[1]) / L2, 0, 1)
            px, py = p0[0] + t * d[0], p0[1] + t * d[1]
            dist = np.hypot(X - px, Y - py)
            a = np.clip(wdt - dist + 0.5, 0, 1)
            # Cylinder normal across the twig.
            side = ((X - px) * -d[1] + (Y - py) * d[0]) / (np.sqrt(L2) * max(wdt, 1e-3))
            side = np.clip(side, -1, 1)
            perp = np.array([-d[1], d[0]]) / np.sqrt(L2)  # image coords
            nx, ny = side * perp[0], -side * perp[1]  # to GL (+y up)
            nz = np.sqrt(np.clip(1 - side * side, 0, 1))
            sl = (slice(ymin, ymax), slice(xmin, xmax))
            sh = (0.6 + 0.4 * nz)[..., None]
            albedo[sl] = albedo[sl] * (1 - a[..., None]) + bark * sh * a[..., None]
            nrm = np.stack([nx, ny, nz], -1)
            normal[sl] = normal[sl] * (1 - a[..., None]) + nrm * a[..., None]
            rough[sl] = rough[sl] * (1 - a) + 0.8 * a
            alpha[sl] = np.maximum(alpha[sl], a)

    def curve(p0, direction, length, droop, steps=24):
        pts = [np.array(p0, np.float32)]
        ang = np.arctan2(direction[1], direction[0])
        for _ in range(steps):
            ang += droop / steps
            pts.append(pts[-1] + length / steps * np.array([np.cos(ang), np.sin(ang)], np.float32))
        return np.array(pts)

    # Main twig enters at the top centre and droops down-right; side twigs
    # hang off it. Image coords: +y is down.
    main = curve((S * 0.5, S * 0.02), (0.15, 1.0), S * 0.95, 0.5)
    twigs = [(main, S * 0.006, S * 0.0025)]
    for t in np.linspace(0.18, 0.8, 5):
        i = int(t * (len(main) - 1))
        side = 1 if len(twigs) % 2 else -1
        d = main[min(i + 1, len(main) - 1)] - main[i]
        ang = np.arctan2(d[1], d[0]) + side * rng.uniform(0.6, 1.0)
        sub = curve(main[i], (np.cos(ang), np.sin(ang)), S * rng.uniform(0.22, 0.38), -side * 0.4)
        twigs.append((sub, S * 0.0035, S * 0.0018))

    # Leaves first, back to front, twigs on top so stems read.
    leaf_jobs = []
    for pts, _, _ in twigs:
        n = len(pts)
        for j in range(2, n, 2):
            d = pts[j] - pts[j - 1]
            base_ang = np.degrees(np.arctan2(-d[1], d[0]))  # GL-style angle (y up)
            for side in (-1, 1):
                if rng.random() < 0.3:
                    continue
                ang = base_ang + side * rng.uniform(25, 60) - 90  # leaf tip direction vs up
                pool = pools[0] if rng.random() < 0.55 else pools[1]
                lf = pool[rng.integers(len(pool))]
                target = S * rng.uniform(0.075, 0.11)
                scale = target / max(lf["alpha"].shape)
                leaf_jobs.append((rng.random(), lf, ang, scale, pts[j], rng))
    leaf_jobs.sort(key=lambda j: j[0])
    for depth, lf, ang, scale, at, _ in leaf_jobs:
        r = rotate_leaf({k: lf[k] for k in ("albedo", "normal", "rough", "alpha")}, ang, scale)
        lin = srgb_to_linear(np.clip(r["albedo"], 0, 1))
        lum = lin @ np.array([0.2126, 0.7152, 0.0722], np.float32)
        # Autumn: push some green leaves towards yellow-olive.
        if rng.random() < 0.5:
            yellow = srgb_to_linear(np.array([0.62, 0.55, 0.18], np.float32))
            k = rng.uniform(0.2, 0.6)
            lin = lin * (1 - k) + yellow * (lum / 0.25)[..., None] * k
        # Scans are lit brighter than a leaf's albedo; tone them down and
        # mute the saturation for the overcast palette.
        lin = (lum[..., None] + 0.8 * (lin - lum[..., None])) * 0.55
        r["albedo_lin"] = lin
        # Offset the leaf so its petiole (bottom of the unrotated crop) sits on the twig.
        t = np.radians(ang)
        half = lf["alpha"].shape[0] * scale * 0.45
        cx = at[0] - np.sin(t) * half
        cy = at[1] - np.cos(t) * half
        paste(r, cx, cy, 0.75 + 0.25 * depth)
    for pts, w0, w1 in twigs:
        draw_twig(pts, w0, w1)

    # Colours were composited over black: un-premultiply, then bleed them
    # outwards before downsampling so edges don't pick up a dark fringe.
    albedo = np.where(alpha[..., None] > 1e-3, albedo / np.maximum(alpha, 1e-3)[..., None], 0.0)
    mat = {
        "albedo": linear_to_srgb(albedo),
        "normal": normalize(normal),
        "rough": rough,
        "ao": ao,
        "metal": np.zeros((S, S), np.float32),
        "alpha": alpha,
    }
    dilate_colors(mat, iterations=8)
    return make_material(mat, size, wrap=False)


# ---------------------------------------------------------------------------
# Mip chains
# ---------------------------------------------------------------------------


def toksvig(rough: np.ndarray, ft: np.ndarray) -> np.ndarray:
    """Widen roughness where averaged normals got shorter (specular AA)."""
    a = rough.astype(np.float64) ** 2
    var = np.clip((1.0 - ft) / np.maximum(ft, 1e-4), 0.0, 1.0)
    a2 = np.clip(a * a + var, 0.0, 1.0)
    return (a2 ** 0.25).astype(np.float32)


def scale_alpha_to_coverage(alpha: np.ndarray, coverage: float) -> np.ndarray:
    lo, hi = 0.0, 8.0
    for _ in range(24):
        mid = 0.5 * (lo + hi)
        if float((np.clip(alpha * mid, 0, 1) >= ALPHA_CUTOFF).mean()) < coverage:
            lo = mid
        else:
            hi = mid
    return np.clip(alpha * hi, 0, 1)


def to_u8(a: np.ndarray) -> np.ndarray:
    return np.clip(np.round(a * 255.0), 0, 255).astype(np.uint8)


def build_mips(mat: dict) -> dict[str, list[bytes]]:
    """Returns {'albedo': [lvl0 bytes, ...], 'normal': [...], 'arm': [...]}
    with RGBA8 data for every level down to 1x1."""
    size = mat["albedo"].shape[0]
    assert mat["albedo"].shape[:2] == (size, size) and size & (size - 1) == 0
    lin = srgb_to_linear(mat["albedo"])
    has_alpha = "alpha" in mat
    alpha = mat["alpha"] if has_alpha else np.ones((size, size), np.float32)
    coverage = float((alpha >= ALPHA_CUTOFF).mean())
    nvec = normalize(mat["normal"])
    ao, rough, metal = mat["ao"], mat["rough"], mat["metal"]
    out = {"albedo": [], "normal": [], "arm": []}
    level = 0
    alpha_lvl = alpha
    while True:
        ft = np.linalg.norm(nvec, axis=-1)
        n = nvec / np.maximum(ft, 1e-8)[..., None]
        r = toksvig(rough, ft) if level else rough
        a_out = alpha_lvl if level == 0 or not has_alpha else scale_alpha_to_coverage(alpha_lvl, coverage)
        out["albedo"].append(np.dstack([to_u8(linear_to_srgb(lin)), to_u8(a_out)]).tobytes())
        out["normal"].append(np.dstack([to_u8(n * 0.5 + 0.5), np.full(ft.shape, 255, np.uint8)]).tobytes())
        out["arm"].append(np.dstack([to_u8(ao), to_u8(r), to_u8(metal), np.full(ft.shape, 255, np.uint8)]).tobytes())
        if lin.shape[0] == 1:
            break
        if has_alpha:
            # Alpha-weighted colour average so transparent texels don't bleed.
            w = alpha_lvl[..., None] + 1e-4
            lin = box2(lin * w) / box2(w)
        else:
            lin = box2(lin)
        alpha_lvl = box2(alpha_lvl)
        nvec = box2(nvec)
        ao, rough, metal = box2(ao), box2(rough), box2(metal)
        level += 1
    return out


# ---------------------------------------------------------------------------
# KTX2
# ---------------------------------------------------------------------------


def dfd_rgba8(srgb: bool) -> bytes:
    samples = b""
    for i, ch in enumerate((0, 1, 2, 15)):
        ctype = ch | (0x10 if (srgb and ch == 15) else 0)  # sRGB alpha is linear
        samples += struct.pack("<HBB4BII", i * 8, 7, ctype, 0, 0, 0, 0, 0, 255)
    block_size = 24 + len(samples)
    block = struct.pack(
        "<IHH4B4B8B",
        0,  # vendorId 0 (Khronos), descriptorType 0 (basic)
        2,  # versionNumber
        block_size,
        1,  # colorModel RGBSDA
        1,  # colorPrimaries BT709/sRGB
        2 if srgb else 1,  # transferFunction sRGB / linear
        0,  # flags: straight alpha
        0, 0, 0, 0,  # texelBlockDimension (1x1x1x1, stored minus one)
        4, 0, 0, 0, 0, 0, 0, 0,  # bytesPlane
    ) + samples
    return struct.pack("<I", 4 + len(block)) + block


def kv_entry(key: str, value: bytes) -> bytes:
    kv = key.encode() + b"\0" + value
    data = struct.pack("<I", len(kv)) + kv
    return data + b"\0" * (-len(data) % 4)


def write_ktx2(path: Path, size: int, levels: list[list[bytes]], srgb: bool, array: bool) -> None:
    """levels[level][layer] = RGBA8 bytes. Writes an uncompressed KTX2."""
    n_levels = len(levels)
    n_layers = len(levels[0])
    dfd = dfd_rgba8(srgb)
    kvd = kv_entry("KTXwriter", b"extraction fetch_assets.py\0")
    header_len = 12 + 4 * 9 + 4 * 4 + 8 * 2
    index_len = 24 * n_levels
    dfd_off = header_len + index_len
    kvd_off = dfd_off + len(dfd)
    data_off = kvd_off + len(kvd)
    data_off += -data_off % 16
    # Level data is stored smallest first.
    blobs = [b"".join(layers) for layers in levels]
    offsets = [0] * n_levels
    pos = data_off
    for lvl in reversed(range(n_levels)):
        pos += -pos % 4
        offsets[lvl] = pos
        pos += len(blobs[lvl])
    with open(path, "wb") as f:
        f.write(b"\xabKTX 20\xbb\r\n\x1a\n")
        f.write(struct.pack(
            "<9I",
            VK_FORMAT_R8G8B8A8_SRGB if srgb else VK_FORMAT_R8G8B8A8_UNORM,
            1, size, size, 0,
            n_layers if array else 0,
            1, n_levels, 0,
        ))
        f.write(struct.pack("<4I2Q", dfd_off, len(dfd), kvd_off, len(kvd), 0, 0))
        for lvl in range(n_levels):
            f.write(struct.pack("<3Q", offsets[lvl], len(blobs[lvl]), len(blobs[lvl])))
        f.write(dfd)
        f.write(kvd)
        f.write(b"\0" * (data_off - kvd_off - len(kvd)))
        cur = data_off
        for lvl in reversed(range(n_levels)):
            f.write(b"\0" * (offsets[lvl] - cur))
            f.write(blobs[lvl])
            cur = offsets[lvl] + len(blobs[lvl])


def find_ktx() -> str | None:
    exe = shutil.which("ktx")
    if exe:
        return exe
    hits = sorted(glob.glob("/opt/ktx-software-*/bin/ktx"))
    return hits[-1] if hits else None


def emit_ktx2(path: Path, size: int, levels: list[list[bytes]], srgb: bool, array: bool,
              normal_map: bool = False) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    ktx = find_ktx()
    with tempfile.TemporaryDirectory(dir=CACHE) as td:
        raw = Path(td) / "raw.ktx2"
        write_ktx2(raw, size, levels, srgb, array)
        if OPTIONS["format"] == "bc7":
            # Opt-in GPU compression: encode UASTC (keeps our mips), then
            # transcode to BC7 + zstd. Desktop GPUs only (no BC on Quest).
            if not ktx:
                raise RuntimeError("--format bc7 needs KTX-Software's `ktx`")
            uastc = Path(td) / "uastc.ktx2"
            packed = Path(td) / "bc7.ktx2"
            rdo = ["--uastc-rdo", "--uastc-rdo-l", "0.5" if normal_map else "1.0"]
            subprocess.run([ktx, "encode", "--codec", "uastc", "--uastc-quality", "2", *rdo,
                            str(raw), str(uastc)], check=True, stdout=subprocess.DEVNULL)
            subprocess.run([ktx, "transcode", "--target", "bc7", "--zstd", str(ZSTD_LEVEL),
                            str(uastc), str(packed)], check=True)
        elif ktx:
            packed = Path(td) / "zstd.ktx2"
            subprocess.run([ktx, "deflate", "--zstd", str(ZSTD_LEVEL), str(raw), str(packed)], check=True)
        if ktx:
            check = subprocess.run([ktx, "validate", str(packed)], capture_output=True, text=True)
            if check.returncode != 0:
                raise RuntimeError(f"ktx validate failed for {path}:\n{check.stdout}{check.stderr}")
            shutil.move(str(packed), path)
        else:
            shutil.move(str(raw), path)


def emit_set(dirpath: Path, chains: list[dict[str, list[bytes]]], size: int, array: bool) -> None:
    """chains: one build_mips() result per layer."""
    for kind, srgb in (("albedo", True), ("normal", False), ("arm", False)):
        n_levels = len(chains[0][kind])
        levels = [[c[kind][lvl] for c in chains] for lvl in range(n_levels)]
        emit_ktx2(dirpath / f"{kind}.ktx2", size, levels, srgb, array, normal_map=kind == "normal")


# ---------------------------------------------------------------------------
# Build steps
# ---------------------------------------------------------------------------


def build_env() -> dict:
    return {"v": PIPELINE_VERSION, "script": sha256_file(Path(__file__)),
            "ktx": bool(find_ktx()), "format": OPTIONS["format"]}


def stamp_key(cfg: dict, manifest: dict) -> str:
    src_keys = [cfg["source"]] if "source" in cfg else []
    src_keys += cfg.get("leaf_sources", [])
    hashes = {k: {fn: m.get("sha256") for fn, m in manifest["sources"][k]["files"].items()} for k in src_keys}
    blob = json.dumps({"cfg": cfg, "src": hashes, "env": build_env()}, sort_keys=True)
    return sha256(blob.encode())


def up_to_date(stamps: dict, name: str, key: str, outputs: list[Path]) -> bool:
    return stamps.get(name) == key and all(p.exists() for p in outputs)


def material_from_cfg(cfg: dict, manifest: dict, size: int) -> dict:
    if "generate" in cfg:
        gen = {"birch_bark": gen_birch_bark, "robot_shell": gen_robot_shell}[cfg["generate"]]
        mat = make_material(gen(size * 2, cfg.get("seed", 0)), size)
    else:
        maps = load_source_maps(manifest["sources"][cfg["source"]])
        mat = make_material(maps, size, repeat=cfg.get("repeat", 1))
        mat.pop("height", None)
    grade(mat, cfg)
    return mat


def describe(name: str, mat: dict) -> str:
    lin = srgb_to_linear(mat["albedo"]).reshape(-1, 3).mean(axis=0)
    return (f"{name:24s} albedo(lin) {lin[0]:.3f} {lin[1]:.3f} {lin[2]:.3f}  "
            f"rough {mat['rough'].mean():.2f}  ao {mat['ao'].mean():.2f}  metal {mat['metal'].mean():.2f}")


def build_terrain(manifest: dict, stamps: dict, force: bool) -> None:
    t = manifest["terrain"]
    size = t["size"]
    outdir = OUT / "terrain"
    outputs = [outdir / f"{k}.ktx2" for k in ("albedo", "normal", "arm")]
    key = sha256(json.dumps({
        "t": t, "env": build_env(),
        "src": {l["source"]: {fn: m.get("sha256") for fn, m in manifest["sources"][l["source"]]["files"].items()}
                for l in t["layers"]},
    }, sort_keys=True).encode())
    if not force and up_to_date(stamps, "terrain", key, outputs):
        log("terrain: up to date")
        return
    expected = ["grass", "forest_floor", "mud", "rock", "gravel", "asphalt", "dirt_road"]
    names = [l["name"] for l in t["layers"]]
    if names != expected:
        raise RuntimeError(f"terrain layers must be exactly {expected} (the shader indexes them), got {names}")
    chains = []
    for layer in t["layers"]:
        mat = material_from_cfg(layer, manifest, size)
        mat["metal"] = np.zeros_like(mat["metal"])  # terrain is never metallic
        log("  " + describe(f"terrain/{layer['name']}", mat))
        chains.append(build_mips(mat))
    emit_set(outdir, chains, size, array=True)
    stamps["terrain"] = key
    log(f"terrain: wrote {', '.join(str(p.relative_to(ROOT)) for p in outputs)}")


def build_materials(manifest: dict, stamps: dict, force: bool) -> None:
    m = manifest["materials"]
    size = m["size"]
    for name, cfg in m["sets"].items():
        outdir = OUT / "materials" / name
        outputs = [outdir / f"{k}.ktx2" for k in ("albedo", "normal", "arm")]
        key = stamp_key(cfg | {"size": size}, manifest)
        if not force and up_to_date(stamps, f"materials/{name}", key, outputs):
            log(f"materials/{name}: up to date")
            continue
        mat = material_from_cfg(cfg, manifest, size)
        log("  " + describe(f"materials/{name}", mat))
        emit_set(outdir, [build_mips(mat)], size, array=False)
        stamps[f"materials/{name}"] = key
        log(f"materials/{name}: written")


def build_foliage(manifest: dict, stamps: dict, force: bool) -> None:
    f = manifest["foliage"]
    size = f["size"]
    for name, cfg in f["cards"].items():
        outdir = OUT / "foliage" / name
        outputs = [outdir / f"{k}.ktx2" for k in ("albedo", "normal", "arm")] + [outdir / "albedo.png"]
        key = stamp_key(cfg | {"size": size}, manifest)
        if not force and up_to_date(stamps, f"foliage/{name}", key, outputs):
            log(f"foliage/{name}: up to date")
            continue
        if cfg.get("generate") == "birch_cluster":
            leaf_maps = [load_source_maps(manifest["sources"][k]) for k in cfg["leaf_sources"]]
            mat = gen_birch_cluster(size, cfg.get("seed", 0), leaf_maps)
        else:
            maps = load_source_maps(manifest["sources"][cfg["source"]])
            maps.pop("height", None)
            mat = make_material(maps, size, wrap=False)
        dilate_colors(mat)
        log("  " + describe(f"foliage/{name}", mat) + f"  coverage {(mat['alpha'] >= ALPHA_CUTOFF).mean():.2f}")
        chain = build_mips(mat)
        emit_set(outdir, [chain], size, array=False)
        rgba = np.frombuffer(chain["albedo"][0], np.uint8).reshape(size, size, 4)
        Image.fromarray(rgba, "RGBA").save(outdir / "albedo.png", optimize=True)
        stamps[f"foliage/{name}"] = key
        log(f"foliage/{name}: written")


def build_sky(manifest: dict, stamps: dict, force: bool) -> None:
    for name, cfg in manifest["sky"].items():
        src = manifest["sources"][cfg["source"]]
        (fname,) = src["files"].keys()
        dst = OUT / cfg["out"]
        if not force and dst.exists() and sha256_file(dst) == src["files"][fname]["sha256"]:
            log(f"sky/{name}: up to date")
            continue
        dst.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(cache_path(src, fname), dst)
        log(f"sky/{name}: copied to {dst.relative_to(ROOT)}")


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------


def sources_for(manifest: dict, groups: list[str]) -> set[str]:
    keys: set[str] = set()
    if "terrain" in groups:
        keys |= {l["source"] for l in manifest["terrain"]["layers"]}
    if "materials" in groups:
        keys |= {c["source"] for c in manifest["materials"]["sets"].values() if "source" in c}
    if "foliage" in groups:
        for c in manifest["foliage"]["cards"].values():
            keys |= {c["source"]} if "source" in c else set()
            keys |= set(c.get("leaf_sources", []))
    if "sky" in groups:
        keys |= {c["source"] for c in manifest["sky"].values()}
    return keys


def credits_rows(manifest: dict) -> str:
    today = time.strftime("%Y-%m")
    rows = []
    used = sources_for(manifest, list(GROUPS))
    for key in sorted(used):
        src = manifest["sources"][key]
        if "files" not in src:
            continue
        rows.append(f"| {key} | {src['page']} | {src.get('author', '?')} | CC0 1.0 | {today} |")
    return "\n".join(rows)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("groups", nargs="*", metavar="GROUP", help=f"subset to build: {', '.join(GROUPS)} (default: all)")
    ap.add_argument("--offline", action="store_true", help="never touch the network")
    ap.add_argument("--force", action="store_true", help="rebuild outputs even if up to date")
    ap.add_argument("--update-hashes", action="store_true", help="accept changed upstream files")
    ap.add_argument("--fetch-only", action="store_true", help="download and verify, build nothing")
    ap.add_argument("--credits", action="store_true", help="print CREDITS.md rows and exit")
    ap.add_argument("--format", choices=("rgba8", "bc7"), default="rgba8",
                    help="rgba8 (default, loads everywhere) or bc7 (4x less VRAM, desktop GPUs only)")
    args = ap.parse_args()
    groups = args.groups or list(GROUPS)
    OPTIONS["format"] = args.format
    bad = [g for g in groups if g not in GROUPS]
    if bad:
        ap.error(f"unknown group(s) {bad}; choose from {list(GROUPS)}")

    manifest = json.loads(MANIFEST.read_text())
    if args.credits:
        print(credits_rows(manifest))
        return 0
    if not find_ktx():
        log("warning: `ktx` (KTX-Software) not found; KTX2 files will be written without zstd "
            "supercompression. See docs/ASSETS.md.")
    CACHE.mkdir(parents=True, exist_ok=True)
    try:
        changed = fetch_sources(manifest, sources_for(manifest, groups), args.offline, args.update_hashes)
    finally:
        # Persist whatever was resolved/hashed, even if a later step failed.
        new_text = json.dumps(manifest, indent=2) + "\n"
        if new_text != MANIFEST.read_text():
            MANIFEST.write_text(new_text)
            log(f"updated {MANIFEST.relative_to(ROOT)}")
    del changed
    if args.fetch_only:
        return 0

    stamps = json.loads(STAMPS.read_text()) if STAMPS.exists() else {}
    builders = {"terrain": build_terrain, "materials": build_materials, "foliage": build_foliage, "sky": build_sky}
    try:
        for g in groups:
            builders[g](manifest, stamps, args.force)
    finally:
        STAMPS.write_text(json.dumps(stamps, indent=2, sort_keys=True) + "\n")
    # BC7 copies of the colour and data maps (`scripts/bc7_textures.py`):
    # the game loads them where the GPU takes BC, the originals elsewhere.
    # Without etcpak they're skipped, and the originals load everywhere.
    bake = Path(__file__).with_name("bc7_textures.py")
    if subprocess.run([sys.executable, str(bake)], check=False).returncode != 0:
        log("BC7 copies not made (pip install etcpak): the RGBA8 textures load instead")
    return 0


if __name__ == "__main__":
    sys.exit(main())
