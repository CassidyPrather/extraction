#!/usr/bin/env python3
"""Reads back the render-based legibility gauge's shots (`--legibility DIR`).

    scripts/legibility.py DIR [--sheet OUT.png] [--crops CROPDIR]

Each shot shows one text panel from where it's read, at the headset's
pixels a degree (render::legibility). This crops the panel, blurs it a
little for the compositor's resample onto the headset's lenses, and reads
it with tesseract (or, without tesseract, a stroke/contrast proxy). Then it
scores what was read against what the panel says. A must-read panel that
reads back worse than MUST_SCORE, or an incidental one worse than
INCIDENTAL_SCORE, fails: the exit status is the number of failures.

OCR is harsher than a person (no context, no second look), so the scores
are a floor, not a target: a panel that tesseract reads, a person reads.
"""
import difflib
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile

from PIL import Image, ImageDraw, ImageFilter, ImageOps

MUST_SCORE = 0.75
INCIDENTAL_SCORE = 0.5
# The compositor resamples each eye's picture onto the lens-warped panel:
# about half a pixel of blur.
COMPOSITOR_BLUR = 0.5


def norm(s):
    """Upper case letters and digits, one space between words."""
    return re.sub(r"\s+", " ", re.sub(r"[^0-9A-Z]+", " ", s.upper())).strip()


def score(want, got):
    """How much of `want` came back (0..1): matching characters over the
    expected ones, so stray noise round a panel doesn't count against it."""
    a, b = norm(want), norm(got)
    if not a:
        return 1.0
    m = difflib.SequenceMatcher(None, a, b, autojunk=False)
    return sum(blk.size for blk in m.get_matching_blocks()) / len(a)


def crop(img, rect, pad=4):
    x, y, w, h = rect
    box = (max(0, int(x) - pad), max(0, int(y) - pad),
           min(img.width, int(x + w) + pad), min(img.height, int(y + h) + pad))
    return img.crop(box)


def ocr(im, dark):
    """Tesseract on the crop scaled up (it wants letters ~30 px tall)."""
    g = ImageOps.grayscale(im)
    if dark:
        g = ImageOps.invert(g)
    g = ImageOps.autocontrast(g, cutoff=1)
    g = g.resize((g.width * 4, g.height * 4), Image.LANCZOS)
    with tempfile.TemporaryDirectory() as d:
        p = os.path.join(d, "c.png")
        g.save(p)
        out = subprocess.run(["tesseract", p, "-", "--psm", "6", "-l", "eng"],
                             capture_output=True, text=True)
    return out.stdout


def proxy(im, dark):
    """Without tesseract: the share of the crop's contrast that survives a
    one-pixel blur (fine strokes lose it), as a stand-in score."""
    g = ImageOps.grayscale(im)
    a = list(g.getdata())
    b = list(g.filter(ImageFilter.GaussianBlur(1.0)).getdata())
    spread = (max(a) - min(a)) or 1
    keep = (max(b) - min(b)) / spread
    return f"(proxy {keep:.2})", keep


def main(argv):
    sheet = None
    crops_dir = None
    rest = []
    it = iter(argv)
    for a in it:
        if a == "--sheet":
            sheet = next(it)
        elif a == "--crops":
            crops_dir = next(it)
        else:
            rest.append(a)
    d = rest[0]
    shots = json.load(open(os.path.join(d, "legibility.json")))
    have_ocr = shutil.which("tesseract") is not None
    if not have_ocr:
        print("tesseract not found: using the stroke/contrast proxy")
    if crops_dir:
        os.makedirs(crops_dir, exist_ok=True)
    results = []
    for s in shots:
        path = os.path.join(d, s["file"])
        if not os.path.exists(path):
            continue
        im = crop(Image.open(path).convert("RGB"), s["rect"])
        seen = im.filter(ImageFilter.GaussianBlur(COMPOSITOR_BLUR))
        if have_ocr:
            got = ocr(seen, s["dark"])
            sc = score(s["text"], got)
        else:
            got, sc = proxy(seen, s["dark"])
        need = MUST_SCORE if s["need"] == "MustRead" else INCIDENTAL_SCORE
        ok = sc >= need
        results.append((s, im, got, sc, ok))
        if crops_dir:
            im.save(os.path.join(crops_dir, s["file"]))
        flag = "ok  " if ok else "FAIL"
        print(f"{flag} {s['file']:28} {s['deg']:.2f} deg (floor {s['floor']}) "
              f"read {sc:.2f}  {norm(s['text'])[:40]!r} -> {norm(got)[:40]!r}")
    fails = [r for r in results if not r[4]]
    print(f"{len(results)} panels, {len(fails)} unreadable")
    if sheet:
        contact(results, sheet)
    return len(fails)


def contact(results, out, cell=(360, 250), cols=4):
    """The crops at 2x (pixels kept square, so the eye's pixels show),
    labelled with source, size and the OCR score."""
    rows = (len(results) + cols - 1) // cols
    W, H = cell
    sheet = Image.new("RGB", (W * cols, H * rows), (30, 30, 30))
    d = ImageDraw.Draw(sheet)
    for i, (s, im, _, sc, ok) in enumerate(results):
        x, y = (i % cols) * W, (i // cols) * H
        k = min(2.0, (W - 8) / max(1, im.width), (H - 30) / max(1, im.height))
        big = im.resize((max(1, int(im.width * k)), max(1, int(im.height * k))), Image.NEAREST)
        sheet.paste(big, (x + 4, y + 4))
        col = (120, 220, 120) if ok else (240, 90, 80)
        d.text((x + 4, y + H - 22),
               f"{s['source']} {s['deg']:.2f}deg read {sc:.2f}", fill=col)
    sheet.save(out)
    print(out)


if __name__ == "__main__":
    sys.exit(min(main(sys.argv[1:]), 125))
