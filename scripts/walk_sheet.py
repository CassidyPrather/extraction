#!/usr/bin/env python3
"""walk_sheet.py <walk-dir>: a walk review's contact sheet and page
(docs/craft/WALK_REVIEW.md) from the stops, the shots and the answers.

<walk-dir> holds `stops.json` (scripts/walk_stops.py), the shots as
`<stop>_ahead.jpg` and `<stop>_side.jpg`, and `answers.json`:
{"title": ..., "beats": [[stop, beat], ...] (optional), "stops": {"s01":
{"vista": ..., "discover": ..., "for": ..., "beat": ...}}, "notes": [...],
"fixes": [{"title": ..., "what": ..., "pairs": [[before, after], ...]}]}
(the fixes' shots as paths in <walk-dir>, the before from the walk).
Writes `sheet.jpg` (a row a stop: the two views and the answers) and
`WALK.md` (the same, as a page, with the beat chart)."""

import json
import os
import sys
import textwrap

from PIL import Image, ImageDraw, ImageFont

d = sys.argv[1]
stops = json.load(open(os.path.join(d, "stops.json")))
ans = json.load(open(os.path.join(d, "answers.json")))


def font(size):
    for f in ("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",):
        if os.path.exists(f):
            return ImageFont.truetype(f, size)
    return ImageFont.load_default()


TW, TH, TXT = 480, 270, 760
rows = []
for s in stops:
    a = ans["stops"].get(s["name"], {})
    row = Image.new("RGB", (TW * 2 + TXT, TH), (24, 24, 24))
    for i, view in enumerate(("ahead", "side")):
        p = os.path.join(d, f"{s['name']}_{view}.jpg")
        if os.path.exists(p):
            row.paste(Image.open(p).convert("RGB").resize((TW, TH)), (i * TW, 0))
    g = ImageDraw.Draw(row)
    x, y = TW * 2 + 14, 10
    head = f"{s['name']}  {s['s']} m  {s['kind']}{(': ' + s['note']) if s['note'] else ''}"
    g.text((x, y), head, font=font(20), fill=(255, 220, 140))
    y += 28
    beat = a.get("beat", "")
    if beat:
        g.text((x, y), beat.upper(), font=font(16), fill=(140, 200, 255))
        y += 22
    for k, label in (("vista", "Vista"), ("discover", "Draws"), ("for", "For")):
        for i, line in enumerate(textwrap.wrap(f"{label}: {a.get(k, '')}", 78)):
            g.text((x, y), line, font=font(14), fill=(230, 230, 230))
            y += 17
        y += 3
    rows.append(row)
sheet = Image.new("RGB", (TW * 2 + TXT, TH * len(rows)), (0, 0, 0))
for i, r in enumerate(rows):
    sheet.paste(r, (0, i * TH))
sheet.save(os.path.join(d, "sheet.jpg"), quality=80)

md = [f"# {ans['title']}", ""]
md += ans.get("intro", [])
md += ["", "## Beats", "", "```mermaid", "flowchart LR"]
prev = None
for s in stops:
    a = ans["stops"].get(s["name"], {})
    node = f'{s["name"]}["{s["name"]} {s["s"]} m<br/>{a.get("beat", s["kind"])}"]'
    md.append(f"    {node}" if prev is None else f"    {prev} --> {node}")
    prev = s["name"]
md += ["```", "", "## Stops", ""]
for s in stops:
    a = ans["stops"].get(s["name"], {})
    md += [
        f"### {s['name']}: {s['s']} m, {s['kind']}{(' (' + s['note'] + ')') if s['note'] else ''}",
        "",
        f"At ({s['at'][0]}, {s['at'][1]}), {s['h']} m; looking {s['fwd']} deg along the way, and {s['side']} deg to {s['side_what']}.",
        "",
        f"![{s['name']} ahead]({s['name']}_ahead.jpg) ![{s['name']} side]({s['name']}_side.jpg)",
        "",
        f"- **Vista.** {a.get('vista', '')}",
        f"- **Draws the eye.** {a.get('discover', '')}",
        f"- **For.** {a.get('beat', '')}: {a.get('for', '')}",
        "",
    ]
md += ["## What's wrong", ""] + [f"- {n}" for n in ans.get("notes", [])] + [""]
if ans.get("fixes"):
    md += ["## Fixes", "", "Before on the left, after on the right.", ""]
    for f in ans["fixes"]:
        md += [f"### {f['title']}", "", f["what"], ""]
        md += [f"![before]({b}) ![after]({a})" for b, a in f["pairs"]] + [""]
open(os.path.join(d, "WALK.md"), "w").write("\n".join(md))
print(os.path.join(d, "sheet.jpg"), len(rows), "stops")
