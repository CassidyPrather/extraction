"""Parts prised off vehicle wrecks (Destruction's `Strippable` yields):
carried off two-handed, propped up as cover, or broken down for scrap.
Writes assets/models/items/<id>.glb (item space: origin at the dims box's
centre, -Z the outer face).

- `bus_panel` (dims 0.9 x 0.6 x 0.03): a bus's side panel from under the
  windows, a steel sheet curving with the body, painted cream over a red
  waist stripe, rust along its foot, two rows of rivets
- `car_door` (dims 1.0 x 0.95 x 0.12): a saloon's front door, faded blue,
  its window frame empty but for a few shards, the handle and lock, the
  hinges along its front edge, the trim panel on the inside
- `car_bonnet` (dims 1.3 x 1.0 x 0.08): a bonnet or boot lid, the faded
  skin bowed over its bracing, rust along one edge
- `car_wheel` (dims 0.6 x 0.6 x 0.2): a road wheel on its flat tyre, the
  rim rusty, standing on its tread, its axle along Z

Materials: Cream, Red, Rusty, Steel, Blue, Black, Glass, Padding, Rubber, Dark.
"""

import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from _kit import item, wanted  # noqa: E402
from common import RX, RZ, T, Part, fillet  # noqa: E402


def bow(y, h, depth):
    """The sheet's outward bow (toward -Z) at height y on a panel h tall."""
    return -depth * math.cos(math.pi * y / h)


def sheet(p, y0, y1, x0, x1, h, depth, t, mat, n=6):
    """A band of the curved sheet between heights y0 and y1, t thick."""
    ys = [y0 + (y1 - y0) * i / n for i in range(n + 1)]
    front = [(bow(y, h, depth) - t / 2, y) for y in ys]
    back = [(bow(y, h, depth) + t / 2, y) for y in reversed(ys)]
    # axis 'x': the profile is (z, y), extruded across X.
    p.prism(front + back, x0, x1, mat, axis="x")


def bus_panel():
    p = Part("body")
    W, H, D, t = 0.9, 0.6, 0.012, 0.0025
    # Rust along the foot, the cream body, the red waist stripe near the top.
    bands = ((-0.3, -0.24, "Rusty"), (-0.24, 0.16, "Cream"), (0.16, 0.22, "Red"),
             (0.22, 0.3, "Cream"))
    for y0, y1, mat in bands:
        sheet(p, y0, y1, -W / 2, W / 2, H, D, t, mat, n=max(2, int((y1 - y0) / 0.05)))
    # Two rows of rivets where it was fixed to the frame, a few torn out.
    for y in (-0.27, 0.27):
        for k in range(15):
            if k in (3, 9, 12) and y > 0:
                continue
            x = -W / 2 + 0.03 + k * (W - 0.06) / 14
            z = bow(y, H, D) - t / 2
            p.sphere(0.005, (x, y, z), "Steel", segs=8, rings=3, scale=(1.0, 1.0, 0.5))
    return [p]


def car_door():
    p = Part("body")
    # The door shell: a skin bowed outward over the inner frame, the trim
    # panel on the inside, the belt line where the window starts.
    W, Hd = 1.0, 0.55
    yb = -0.475                       # the door's bottom
    yt = yb + Hd                      # the belt line
    skin = fillet([(-W / 2, yb), (W / 2, yb), (W / 2, yt), (-W / 2 + 0.05, yt),
                   (-W / 2, yt - 0.06)], [0.04, 0.04, 0.02, 0.02, 0.04], 3)
    p.prism(skin, -0.06, -0.035, "Blue", axis="z", bevel=0.008, segs=1)
    # (The body runs into the skin and the trim into the body, so no two
    # layers meet in one plane.)
    p.prism(skin, -0.04, 0.04, "Black", axis="z")
    p.prism(fillet([(-W / 2 + 0.04, yb + 0.03), (W / 2 - 0.04, yb + 0.03), (W / 2 - 0.04, yt - 0.03),
                    (-W / 2 + 0.06, yt - 0.03)], 0.03, 2), 0.035, 0.06, "Padding", axis="z",
            bevel=0.006, segs=1)
    # A crease along the skin, the handle and lock, rust at the sill.
    p.box((W - 0.06, 0.012, 0.004), (0.0, yb + 0.33, -0.0615), "Blue", segs=1)
    p.box((0.12, 0.025, 0.015), (W / 2 - 0.16, yt - 0.08, -0.068), "Steel", bevel=0.004, segs=1)
    p.cyl(0.009, -0.07, -0.06, "Steel", segs=10, xf=T(W / 2 - 0.06, yt - 0.08, 0.0))
    p.box((W - 0.1, 0.05, 0.004), (0.0, yb + 0.03, -0.0615), "Rusty", segs=1)
    # Hinges along the front (-X) edge.
    for y in (yb + 0.12, yt - 0.12):
        p.box((0.03, 0.06, 0.05), (-W / 2 - 0.012, y, 0.0), "Steel", bevel=0.004, segs=1)
    # The window frame: a channel up from the belt, empty, a few shards left
    # in its foot.
    fw = 0.025
    frame = [(-W / 2 + 0.07, yt), (-W / 2 + 0.07, yt + 0.33), (-W / 2 + 0.25, yt + 0.4),
             (W / 2 - 0.05, yt + 0.4), (W / 2 - 0.05, yt)]
    for k, ((x0, y0), (x1, y1)) in enumerate(zip(frame, frame[1:])):
        L = math.hypot(x1 - x0, y1 - y0)
        a = math.atan2(y1 - y0, x1 - x0)
        # (Alternate pieces 8 mm thinner, so where they overlap at the
        # corners their faces don't share a plane.)
        p.box((L + fw, fw, 0.03 - 0.008 * (k % 2)), (0.0, 0.0, 0.0), "Blue", bevel=0.004, segs=1,
              xf=T((x0 + x1) / 2, (y0 + y1) / 2, -0.02) @ RZ(a))
    # (Their feet down in the belt, where the glass stood.)
    for x, h, lean in ((-0.25, 0.07, 0.2), (-0.05, 0.12, -0.15), (0.2, 0.05, 0.1)):
        p.prism([(x - 0.03, yt - 0.01), (x + 0.035, yt - 0.01), (x + 0.005 + lean * h, yt + h)],
                -0.022, -0.018, "Glass", axis="z")
    return [p]


def car_bonnet():
    p = Part("body")
    # A bonnet (or boot lid) wrenched off: the faded skin bowed over its
    # dark inner bracing, the hinge brackets torn at one edge, rust
    # blotches eaten in. Its outer face is -Z.
    W, H, D, t = 1.3, 1.0, 0.02, 0.003
    for y0, y1, mat in ((-0.5, -0.42, "Rusty"), (-0.42, 0.5, "Blue")):
        sheet(p, y0, y1, -W / 2 + 0.02, W / 2 - 0.02, H, D, t, mat, n=max(2, int((y1 - y0) / 0.08)))
    # The bracing: an X of pressed ribs and a rim behind the skin, the
    # X's arms out into the rim's corners, their fronts against the skin
    # toward its ends (behind it where it bows out, never through it).
    # (The two arms of the X a little apart in depth, so they don't share
    # faces where they cross.)
    for a, front, d in ((0.62, -0.005, 0.04), (-0.62, -0.0045, 0.045)):
        p.box((1.45, 0.05, d), (0.0, 0.0, 0.0), "Dark", segs=1,
              xf=T(0.0, 0.0, front + d / 2) @ RZ(a))
    for y in (-0.44, 0.44):
        p.box((W - 0.1, 0.05, 0.02), (0.0, y, 0.016), "Dark", segs=1)
    for x in (-W / 2 + 0.07, W / 2 - 0.07):
        p.box((0.05, H - 0.14, 0.02), (x, 0.0, 0.021), "Dark", segs=1)
    # Hinge brackets along the top edge, one torn and bent.
    for x, bend in ((-0.45, 0.0), (0.45, 0.5)):
        p.box((0.04, 0.12, 0.012), (0.0, 0.0, 0.0), "Steel", segs=1,
              xf=T(x, 0.46, 0.028) @ RX(bend))
    # Two creases pressed along it.
    for x in (-0.3, 0.3):
        p.box((0.012, H - 0.12, 0.006), (x, 0.0, bow(0.0, H, D) - t / 2 - 0.001), "Blue", segs=1)
    return [p]


def car_wheel():
    p = Part("body")
    # A road wheel off a wreck, standing on its tread: the flat-spotted
    # tyre round a rusty pressed-steel rim, its hub cap gone, the stud
    # holes showing.
    r, w, rim = 0.29, 0.17, 0.19
    p.lathe([(rim - 0.01, -w / 2), (r - 0.02, -w / 2), (r, -w / 2 + 0.03), (r, w / 2 - 0.03),
             (r - 0.02, w / 2), (rim - 0.01, w / 2)], "Rubber", segs=28, closed=True)
    # Tread blocks across the crown.
    for k in range(28):
        a = 2 * math.pi * (k + 0.5) / 28
        p.box((0.022, 0.012, w - 0.07), (0.0, 0.0, 0.0), "Rubber", segs=1,
              xf=T(r * math.cos(a), r * math.sin(a), 0.0) @ RZ(a + math.pi / 2))
    p.lathe([(0.0, -0.035), (0.08, -0.035), (0.12, -0.05), (rim, -0.05), (rim, 0.06),
             (0.12, 0.04), (0.08, 0.025), (0.0, 0.025)], "Rusty", segs=24)
    for k in range(4):
        a = math.pi / 2 * k + math.pi / 4
        p.cyl(0.011, -0.04, -0.034, "Dark", segs=8, xf=T(0.055 * math.cos(a), 0.055 * math.sin(a),
                                                          0.0))
    p.cyl(0.03, -0.04, 0.03, "Dark", segs=12)
    return [p]


if __name__ == "__main__":
    for item_id, build in (("bus_panel", bus_panel), ("car_door", car_door),
                           ("car_wheel", car_wheel), ("car_bonnet", car_bonnet)):
        if wanted(item_id):
            item(item_id, build)
