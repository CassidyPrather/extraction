#!/usr/bin/env python3
"""Bakes a BC7 copy of every RGBA8 KTX2 texture under assets/textures,
and a BC5 copy of every normal map.

    scripts/bc7_textures.py [--force] [paths...]

Each `<name>.ktx2` (R8G8B8A8, every mip level, 2D or array) gets a
`<name>.bc7.ktx2` beside it: the same levels and layers, BC7 (sRGB if the
source was), a quarter of the bytes. The GPU samples BC7 as it is, so
nothing is decoded as it loads, and it takes a quarter of the memory.
The game loads the BC7 copy where the GPU has BC formats and the copy
exists (`render::textures`), the original otherwise. A texture that fails
to convert keeps only its original. Needs `pip install etcpak numpy`.

Normal maps (`normal.ktx2`) get `normal.bc5.ktx2` instead: their fine
detail doesn't fit BC7's 4x4 blocks (26-29 dB, about 5 degrees off), and
BC5 keeps X (red) and Y (green) at 8 bits a texel each, a channel apiece
and half the bytes of RGBA8; the shaders rebuild Z (Bevy's own does,
`TWO_COMPONENT_NORMAL_MAP`, as does terrain.wgsl).
See docs/PERFORMANCE.md, "Textures".
"""
import struct
import sys
from pathlib import Path

import numpy as np

try:
    import etcpak
except ImportError:
    print("bc7_textures.py: no etcpak (pip install etcpak): no BC7 copies made")
    sys.exit(1)

ROOT = Path(__file__).resolve().parent.parent / "assets" / "textures"
MAGIC = b"\xabKTX 20\xbb\r\n\x1a\n"
RGBA8 = {37: False, 43: True}  # VK_FORMAT_R8G8B8A8_UNORM / _SRGB
BC7_UNORM, BC7_SRGB = 145, 146
BC5_UNORM = 141


def read_ktx2(path: Path):
    """(srgb, width, height, layers or 0, levels[level][layer] = bytes)."""
    d = path.read_bytes()
    if d[:12] != MAGIC:
        raise ValueError("not a KTX2")
    vk, _ts, w, h, _d, layers, faces, n_levels, scs = struct.unpack("<9I", d[12:48])
    if vk not in RGBA8 or scs != 0 or faces != 1:
        raise ValueError(f"not plain RGBA8 (vkFormat {vk}, supercompression {scs})")
    n_layers = max(layers, 1)
    levels = []
    for lvl in range(n_levels):
        off, length, _ = struct.unpack("<3Q", d[80 + 24 * lvl:80 + 24 * (lvl + 1)])
        blob = d[off:off + length]
        per = len(blob) // n_layers
        levels.append([blob[i * per:(i + 1) * per] for i in range(n_layers)])
    return RGBA8[vk], w, h, layers, levels


def params(srgb: bool):
    """Colour weighted as the eye sees it; data (roughness, AO, metal)
    every channel alike."""
    p = etcpak.BC7CompressBlockParams()
    if not srgb:
        p.init_linear_weights()
        p.m_perceptual = False
    return p


def bc7(rgba: bytes, w: int, h: int, srgb: bool) -> bytes:
    """One level of one layer, padded to whole 4x4 blocks (edge texels
    repeated, so a small mip's blocks don't pull in black)."""
    a = np.frombuffer(rgba, dtype=np.uint8).reshape(h, w, 4)
    pw, ph = -w % 4, -h % 4
    if pw or ph:
        a = np.pad(a, ((0, ph), (0, pw), (0, 0)), mode="edge")
    a = np.ascontiguousarray(a)
    return etcpak.compress_bc7(a.tobytes(), a.shape[1], a.shape[0], params(srgb))


def bc5(rgba: bytes, w: int, h: int, _srgb: bool) -> bytes:
    """One level of one layer as BC5: red and green, each a channel of
    its own (padded as `bc7`)."""
    a = np.frombuffer(rgba, dtype=np.uint8).reshape(h, w, 4)
    pw, ph = -w % 4, -h % 4
    if pw or ph:
        a = np.pad(a, ((0, ph), (0, pw), (0, 0)), mode="edge")
    a = np.ascontiguousarray(a)
    return etcpak.compress_bc5(a.tobytes(), a.shape[1], a.shape[0])


def dfd_bc5() -> bytes:
    """A Basic Data Format Descriptor for BC5: two 64-bit samples per 4x4
    block, red then green, linear."""
    block = struct.pack(
        "<IHH4B4B8B",
        0,
        2,
        24 + 2 * 16,
        132,  # colour model: BC5
        1,
        1,  # transfer: linear
        0,
        3, 3, 0, 0,
        16, 0, 0, 0, 0, 0, 0, 0,
    )
    red = struct.pack("<HBBIII", 0, 63, 0, 0, 0, 0xFFFFFFFF)
    green = struct.pack("<HBBIII", 64, 63, 1, 0, 0, 0xFFFFFFFF)
    body = block + red + green
    return struct.pack("<I", 4 + len(body)) + body


def dfd_bc7(srgb: bool) -> bytes:
    """A Basic Data Format Descriptor for BC7: one 128-bit sample per 4x4
    block."""
    block = struct.pack(
        "<IHH4B4B8B",
        0,  # vendor (Khronos), descriptor type (basic)
        2,  # version
        24 + 16,  # block size: header and one sample
        134,  # colour model: BC7
        1,  # primaries: BT.709
        2 if srgb else 1,  # transfer: sRGB or linear
        0,  # flags: straight alpha
        3, 3, 0, 0,  # texel block: 4x4 (each dimension less one)
        16, 0, 0, 0, 0, 0, 0, 0,  # bytes per plane
    )
    sample = struct.pack("<HBBIII", 0, 127, 0, 0, 0, 0xFFFFFFFF)
    body = block + sample
    return struct.pack("<I", 4 + len(body)) + body


def write_ktx2(path: Path, srgb: bool, w: int, h: int, layers: int, levels,
               normal: bool = False) -> None:
    n_levels = len(levels)
    dfd = dfd_bc5() if normal else dfd_bc7(srgb)
    vk = BC5_UNORM if normal else BC7_SRGB if srgb else BC7_UNORM
    kvd_text = b"KTXwriter\0extraction bc7_textures.py\0"
    kvd = struct.pack("<I", len(kvd_text)) + kvd_text
    kvd += b"\0" * (-len(kvd) % 4)
    header_len = 12 + 4 * 9 + 4 * 4 + 8 * 2
    dfd_off = header_len + 24 * n_levels
    kvd_off = dfd_off + len(dfd)
    data_off = kvd_off + len(kvd)
    data_off += -data_off % 16
    blobs = [b"".join(layer) for layer in levels]
    offsets = [0] * n_levels
    pos = data_off
    # Smallest first, each level aligned to the block size.
    for lvl in reversed(range(n_levels)):
        pos += -pos % 16
        offsets[lvl] = pos
        pos += len(blobs[lvl])
    with open(path, "wb") as f:
        f.write(MAGIC)
        f.write(struct.pack("<9I", vk, 1, w, h, 0, layers, 1, n_levels, 0))
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


def convert(src: Path, force: bool) -> str:
    normal = src.name == "normal.ktx2"
    out = src.with_name(src.name[: -len(".ktx2")] + (".bc5.ktx2" if normal else ".bc7.ktx2"))
    if out.exists() and not force and out.stat().st_mtime >= src.stat().st_mtime:
        return "up to date"
    srgb, w, h, layers, levels = read_ktx2(src)
    packed = []
    for lvl, per_layer in enumerate(levels):
        lw, lh = max(w >> lvl, 1), max(h >> lvl, 1)
        packed.append([(bc5 if normal else bc7)(rgba, lw, lh, srgb) for rgba in per_layer])
    write_ktx2(out, srgb, w, h, layers, packed, normal)
    return f"{src.stat().st_size >> 20} MB > {out.stat().st_size >> 20} MB"


def main() -> int:
    force = "--force" in sys.argv
    paths = [Path(p).resolve() for p in sys.argv[1:] if not p.startswith("--")]
    if not paths:
        paths = sorted(
            p
            for p in ROOT.rglob("*.ktx2")
            if not p.name.endswith((".bc7.ktx2", ".bc5.ktx2"))
        )
    failed = 0
    for p in paths:
        try:
            print(f"{p.relative_to(ROOT.parent.parent)}: {convert(p, force)}", flush=True)
        except Exception as e:  # noqa: BLE001 - one texture's failure keeps its original
            failed += 1
            print(f"{p}: kept as it is ({e})", flush=True)
    print(f"{len(paths) - failed} converted or current, {failed} kept as they were")
    return 0


if __name__ == "__main__":
    sys.exit(main())
