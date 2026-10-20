#!/usr/bin/env bash
# Time-of-day matrix: one lookbook view per valley at dawn, day, dusk and
# night, in clear (a clearing spell) and rain (a downpour), laid out per
# valley (hours across, weathers down) into docs/shots/visual-design/.
#   scripts/hours_matrix.sh [maps...]   (default: all four)
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
declare -A VIEW=(
    [marrow_creek]=m06_edge
    [kettle_pass]=k08_pass
    [cinder_works]=c06_wood_edge
    [training_yard]=t04_east
)
MAPS=("$@")
[ ${#MAPS[@]} -eq 0 ] && MAPS=(marrow_creek kettle_pass cinder_works training_yard)
for MAP in "${MAPS[@]}"; do
    for HOUR in dawn day dusk night; do
        for W in clearing downpour; do
            HOUR=$HOUR WEATHER=$W ONLY="${VIEW[$MAP]}" OUT="target/hours/$HOUR-$W" \
                scripts/lookbook.sh "$MAP" >/dev/null
        done
    done
    python3 - "$MAP" "${VIEW[$MAP]}" <<'PY'
import sys
from PIL import Image, ImageDraw
m, v = sys.argv[1], sys.argv[2]
W = 300
cells = []
for w in ["clearing", "downpour"]:
    for h in ["dawn", "day", "dusk", "night"]:
        im = Image.open(f"target/hours/{h}-{w}/{m}/{v}.png").convert("RGB")
        im = im.resize((W, round(im.height * W / im.width)), Image.LANCZOS)
        d = ImageDraw.Draw(im)
        t = f"{h} / {'clear' if w == 'clearing' else 'rain'}"
        d.rectangle([0, 0, 7 * len(t) + 8, 15], fill=0)
        d.text((4, 2), t, fill=(255, 255, 255))
        cells.append(im)
h = cells[0].height
out = Image.new("RGB", (W * 4, h * 2))
for i, c in enumerate(cells):
    out.paste(c, ((i % 4) * W, (i // 4) * h))
out.save(f"docs/shots/visual-design/hours_{m}.jpg", quality=85)
PY
    echo "$MAP done"
done
