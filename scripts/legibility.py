#!/usr/bin/env python3
"""Reads back the render-based legibility gauge's shots (`--legibility DIR`).

    scripts/legibility.py DIR [--sheet OUT.png] [--crops CROPDIR]
    scripts/legibility.py --compare OUT.png BEFORE AFTER [...] [--sources A,B]

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


def flatten(im, dark):
    """Dark letters on white: the panel's own ground (a lamp's glow across
    it, shade) divided out, then cut to the words with a margin, as a
    reader's eye adapts to the page and finds the words on it."""
    g = ImageOps.grayscale(im)
    if dark:
        g = ImageOps.invert(g)
    # The ground: the lightest nearby, smoothed.
    k = max(3, (min(g.width, g.height) // 8) | 1)
    ground = g.filter(ImageFilter.MaxFilter(k)).filter(ImageFilter.GaussianBlur(k))
    px = [min(255, int(255 * a / max(b, 1))) for a, b in zip(g.get_flattened_data(), ground.get_flattened_data())]
    g.putdata(px)
    g = ImageOps.autocontrast(g, cutoff=1)
    ink = g.point(lambda v: 255 if v < 140 else 0)
    box = ink.getbbox()
    if box:
        m = max(4, (box[3] - box[1]) // 4)
        g = g.crop((max(0, box[0] - m), max(0, box[1] - m),
                    min(g.width, box[2] + m), min(g.height, box[3] + m)))
    return g


def ocr(im, dark, want):
    """Tesseract on the crop scaled up (it wants letters ~30 px tall): as a
    block, a line and sparse words, keeping the best reading (a lone word
    on a big card trips the block reading)."""
    g = flatten(im, dark)
    g = g.resize((g.width * 4, g.height * 4), Image.LANCZOS)
    best = ""
    with tempfile.TemporaryDirectory() as d:
        p = os.path.join(d, "c.png")
        g.save(p)
        for psm in ("6", "11", "7"):
            out = subprocess.run(["tesseract", p, "-", "--psm", psm, "-l", "eng"],
                                 capture_output=True, text=True).stdout
            if score(want, out) > score(want, best):
                best = out
    return best


def proxy(im, dark):
    """Without tesseract: the share of the crop's contrast that survives a
    one-pixel blur (fine strokes lose it), as a stand-in score."""
    g = ImageOps.grayscale(im)
    a = list(g.getdata())
    b = list(g.filter(ImageFilter.GaussianBlur(1.0)).getdata())
    spread = (max(a) - min(a)) or 1
    keep = (max(b) - min(b)) / spread
    return f"(proxy {keep:.2})", keep


def compare(pairs, out, sources=None):
    """Before and after, side by side, a row a source: `pairs` is a list of
    (before dir, after dir); each row is the first shot of a source in
    each, at 2x with square pixels, labelled with its size and score."""
    rows = []
    for before, after in pairs:
        b = {s["source"]: s for s in reversed(json.load(open(os.path.join(before, "legibility.json"))))}
        a = {s["source"]: s for s in reversed(json.load(open(os.path.join(after, "legibility.json"))))}
        for src in sources or sorted(set(b) & set(a)):
            if src in b and src in a and (b[src], a[src]) not in rows:
                rows.append(((before, b[src]), (after, a[src])))
    W, H = 520, 300
    sheet = Image.new("RGB", (W * 2, H * len(rows) + 24), (30, 30, 30))
    d = ImageDraw.Draw(sheet)
    d.text((8, 6), "before", fill=(230, 230, 230))
    d.text((W + 8, 6), "after", fill=(230, 230, 230))
    for r, pair in enumerate(rows):
        for c, (dir_, s) in enumerate(pair):
            whole = Image.open(os.path.join(dir_, s["file"])).convert("RGB")
            im = crop(whole, s["rect"])
            inner = crop(whole, s["rect"], pad=-max(2, int(0.03 * min(s["rect"][2:]))))
            seen = inner.filter(ImageFilter.GaussianBlur(COMPOSITOR_BLUR))
            sc = score(s["text"], ocr(seen, s["dark"], s["text"])) if shutil.which("tesseract") else 0.0
            k = min(2.0, (W - 8) / max(1, im.width), (H - 30) / max(1, im.height))
            big = im.resize((max(1, int(im.width * k)), max(1, int(im.height * k))), Image.NEAREST)
            x, y = c * W, 24 + r * H
            sheet.paste(big, (x + 4, y + 4))
            ok = sc >= (MUST_SCORE if s["need"] == "MustRead" else INCIDENTAL_SCORE)
            d.text((x + 4, y + H - 22),
                   f"{s['source']}: x-height {s['deg']:.2f} deg at {s['at']} m (floor {s['floor']}), OCR {sc:.2f}",
                   fill=(120, 220, 120) if ok else (240, 90, 80))
    sheet.save(out)
    print(out)


def main(argv):
    if argv and argv[0] == "--compare":
        # --compare OUT BEFORE AFTER [BEFORE AFTER ...] [--sources A,B,...]
        sources = None
        if "--sources" in argv:
            i = argv.index("--sources")
            sources = argv[i + 1].split(",")
            argv = argv[:i] + argv[i + 2:]
        dirs = argv[2:]
        compare(list(zip(dirs[::2], dirs[1::2])), argv[1], sources)
        return 0
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
        whole = Image.open(path).convert("RGB")
        # What the sheet shows: the panel and a little round it. What's
        # read: just inside its edges, so nothing beside it is taken for
        # letters.
        im = crop(whole, s["rect"])
        inner = crop(whole, s["rect"], pad=-max(2, int(0.03 * min(s["rect"][2:]))))
        seen = inner.filter(ImageFilter.GaussianBlur(COMPOSITOR_BLUR))
        if have_ocr:
            got = ocr(seen, s["dark"], s["text"])
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
