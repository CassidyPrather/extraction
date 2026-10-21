"""Lev, rigged for Story's walk: one node per part, each with its origin on
the joint it turns about (as the robots' legs). Writes
assets/models/props/lev.glb. (lev_body.glb, the standing figure, is
art/props/lev.py.)

A man in his fifties, 1.76 m to the top of his cap: a hip-length quilted
work jacket (Wool, as Hands' lev_hand sleeve), wool trousers, a knitted
cap, the brick scarf, worn boots, a canvas bag on his left hip and a torch
in his left hand. Prop space: origin on the ground between his feet,
facing -Z, +X his right. The nodes are flat (no hierarchy): Story chains
them, each rotating about its origin, in this rest pose:

    node          origin (joint)          child joint it reaches
    pelvis        (0, 0.95, 0)            hips (+-0.09, 0.92, 0), waist
    spine         (0, 1.05, 0) waist      chest (0, 1.22, 0)
    chest         (0, 1.22, 0)            shoulders (+-0.2, 1.42, 0), neck
    head          (0, 1.49, 0) neck
    upper_arm_l/r (+-0.2, 1.42, 0)        elbow (+-0.215, 1.13, 0)  0.29 m
    forearm_l     (-0.215, 1.13, 0)       wrist (-0.22, 0.88, 0)    0.25 m
    hand_l        (-0.22, 0.88, 0) wrist  (holds the torch)
    thigh_l/r     (+-0.09, 0.92, 0) hip   knee (+-0.095, 0.49, 0)   0.43 m
    shin_l/r      (+-0.095, 0.49, 0)      ankle (+-0.1, 0.08, 0)    0.41 m
    foot_l/r      (+-0.1, 0.08, 0) ankle
    bag           (-0.19, 1.02, -0.01)    the strap's end on his left hip

The right forearm and hand are Hands' lev_hand (art/hands/lev_hand.py): it
includes the forearm and its sleeve, its origin at the palm 0.304 m from
the elbow along the forearm, which runs along its +Z. The right sleeve
here ends at the elbow, 0.053 across, over the end of lev_hand's.

`pt_torch` is the torch's lens (the light), in the rest pose: it moves
with hand_l. Every node has `<name>_lod1` and `<name>_lod2` stand-ins with
the same origin.

Materials: Wool (jacket), DarkWool (trousers, cap), Brick (scarf), Skin,
Leather (boots), Black, Dark, Ticking, Canvas, Olive, Lamp.
"""

import math
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from common import RX, S, T, Part, empty, export, make, report, reset  # noqa: E402
from mathutils import Vector  # noqa: E402

UP = RX(-math.pi / 2)
SEGS = (20, 12, 6)            # round things' sides at each LOD


def ring(c, rx, rz, n, dz=0.0):
    cx, cy, cz = c
    return [Vector((cx + rx * math.cos(2 * math.pi * k / n), cy,
                    cz + dz + rz * math.sin(2 * math.pi * k / n))) for k in range(n)]


def trunk(p, stations, mat, n):
    """A loft of ellipses up the body: (y, rx, rz, dz)."""
    p.loft([ring((0.0, y, 0.0), rx, rz, n, dz) for y, rx, rz, dz in stations], mat)


def strap(p, pts, w, t, nrm, mat):
    """A flat strap along points, its flat face toward `nrm`."""
    rings = []
    for i, c in enumerate(pts):
        tng = (pts[min(i + 1, len(pts) - 1)] - pts[max(i - 1, 0)]).normalized()
        across = tng.cross(nrm).normalized()
        out = across.cross(tng).normalized()
        rings.append([c - across * w / 2 - out * t / 2, c + across * w / 2 - out * t / 2,
                      c + across * w / 2 + out * t / 2, c - across * w / 2 + out * t / 2])
    p.loft(rings, mat)


def limb(p, pts, radii, mat, n):
    pts = [Vector(q) for q in pts]
    rings = []
    for i, c in enumerate(pts):
        t = (pts[min(i + 1, len(pts) - 1)] - pts[max(i - 1, 0)]).normalized()
        a = Vector((1, 0, 0)) if abs(t.x) < 0.9 else Vector((0, 0, 1))
        u = t.cross(a).normalized()
        v = t.cross(u)
        r = radii[i]
        rings.append([c + (u * math.cos(2 * math.pi * k / n) + v * math.sin(2 * math.pi * k / n))
                      * r for k in range(n)])
    p.loft(rings, mat)


HIP, KNEE, ANKLE = 0.92, 0.49, 0.08
SHOULDER, ELBOW, WRIST = (0.2, 1.42), (0.215, 1.13), (0.22, 0.88)


def build(lod):
    n = SEGS[lod]
    full = lod == 0
    parts = {}

    def part(name, origin):
        p = Part(name + ("" if lod == 0 else f"_lod{lod}"), origin=origin)
        parts[name] = p
        return p

    # --- Pelvis: the jacket's skirt over the hips, the trousers' seat.
    p = part("pelvis", (0.0, 0.95, 0.0))
    trunk(p, [(0.79, 0.215, 0.15, 0.0), (0.9, 0.205, 0.142, 0.0), (1.0, 0.195, 0.135, 0.0),
              (1.08, 0.19, 0.13, 0.0)], "Wool", n + 4)
    trunk(p, [(0.8, 0.17, 0.11, 0.0), (0.95, 0.17, 0.11, 0.0)], "DarkWool", n)
    if full:
        for y in (0.83, 0.91, 0.99):
            trunk(p, [(y - 0.003, 0.2155 - (y - 0.79) * 0.2, 0.151 - (y - 0.79) * 0.15, 0.0),
                      (y + 0.003, 0.2155 - (y - 0.79) * 0.2, 0.151 - (y - 0.79) * 0.15, 0.0)],
                  "DarkWool", n + 4)
        for y in (0.86, 0.98):
            p.sphere(0.01, (0.03, y, -0.152 + (y - 0.79) * 0.1), "Black", segs=8, rings=4)

    # --- Spine: the jacket from the waist up under the chest.
    p = part("spine", (0.0, 1.05, 0.0))
    trunk(p, [(1.03, 0.192, 0.132, 0.0), (1.13, 0.195, 0.132, 0.0), (1.25, 0.2, 0.135, -0.005)],
          "Wool", n + 4)
    if full:
        for y in (1.07, 1.15):
            trunk(p, [(y - 0.003, 0.197, 0.137, 0.0), (y + 0.003, 0.197, 0.137, 0.0)],
                  "DarkWool", n + 4)
        p.sphere(0.01, (0.03, 1.1, -0.133), "Black", segs=8, rings=4)

    # --- Chest: up to the shoulders and collar; the scarf; the bag's strap
    # across from the left hip to the right shoulder.
    p = part("chest", (0.0, 1.22, 0.0))
    # (Its bottom tucks inside the spine's top, so the two never coincide.)
    trunk(p, [(1.2, 0.188, 0.124, -0.005), (1.27, 0.2, 0.135, -0.005),
              (1.32, 0.205, 0.13, -0.005), (1.41, 0.2, 0.115, 0.0),
              (1.46, 0.13, 0.09, 0.005), (1.49, 0.08, 0.07, 0.01)], "Wool", n + 4)
    p.lathe([(0.07, 1.45), (0.082, 1.45), (0.086, 1.52), (0.078, 1.52)], "Wool", segs=n,
            xf=S(1.0, 1.0, 0.9) @ UP, closed=True)
    p.lathe([(0.06, 1.47), (0.088, 1.48), (0.09, 1.52), (0.06, 1.53)], "Brick", segs=n,
            xf=S(1.0, 1.0, 0.9) @ UP, closed=True)
    if full:
        p.box((0.065, 0.24, 0.012), (0.0, 0.0, 0.0), "Brick", bevel=0.004, segs=1,
              xf=T(-0.05, 1.36, -0.128) @ RX(-0.08))
        for y in (1.25, 1.33):
            trunk(p, [(y - 0.003, 0.207, 0.135, -0.005), (y + 0.003, 0.207, 0.135, -0.005)],
                  "DarkWool", n + 4)
        p.sphere(0.01, (0.03, 1.27, -0.136), "Black", segs=8, rings=4)
        p.sphere(0.01, (0.03, 1.37, -0.128), "Black", segs=8, rings=4)
    # The strap: from the right shoulder down across the chest to the left
    # hip, and the same across the back, lying on the jacket.
    for side in (-1, 1):
        pts = []
        for x, y in ((0.12, 1.445), (0.04, 1.34), (-0.06, 1.22), (-0.15, 1.1), (-0.19, 1.04)):
            rx, rz = (0.2, 0.135) if y < 1.38 else (0.19, 0.11)
            z = side * (rz * math.sqrt(max(0.0, 1.0 - (x / rx) ** 2)) + 0.007)
            pts.append(Vector((x, y, z)))
        strap(p, pts, 0.035, 0.004, Vector((0, 0, side)), "Canvas")

    # --- Head: neck, head, cap.
    p = part("head", (0.0, 1.49, 0.0))
    p.cyl(0.05, 1.49, 1.57, "Skin", segs=n, xf=UP)
    p.sphere(0.1, (0.0, 1.645, -0.005), "Skin", segs=max(8, n), rings=max(5, n // 2),
             scale=(0.82, 1.05, 0.95))
    if full:
        p.box((0.024, 0.04, 0.03), (0.0, 1.635, -0.1), "Skin", bevel=0.01, segs=2)
        for sx in (-1, 1):
            p.sphere(0.022, (sx * 0.083, 1.645, 0.0), "Skin", segs=8, rings=4,
                     scale=(0.4, 1.2, 0.8))
            p.sphere(0.009, (sx * 0.032, 1.665, -0.088), "Dark", segs=8, rings=4,
                     scale=(1.0, 0.6, 0.5))
        p.box((0.05, 0.012, 0.01), (0.0, 1.608, -0.096), "Ticking", bevel=0.004, segs=1)
    # The knitted cap: a dome pulled down to the ears, its brim turned up.
    p.lathe([(0.0, 1.78), (0.06, 1.772), (0.092, 1.745), (0.104, 1.7), (0.104, 1.665),
             (0.0, 1.665)], "DarkWool", segs=max(8, n), xf=T(0, 0, -0.005) @ S(0.86, 1, 1.0) @ UP)
    p.lathe([(0.101, 1.66), (0.11, 1.662), (0.11, 1.7), (0.101, 1.702)], "DarkWool",
            segs=max(8, n), xf=T(0, 0, -0.005) @ S(0.86, 1, 1.0) @ UP, closed=True)

    # --- Arms (Wool sleeves). The right ends open at the elbow over
    # lev_hand's sleeve.
    for side, sx in (("l", -1), ("r", 1)):
        sh = (sx * SHOULDER[0], SHOULDER[1], 0.0)
        el = (sx * ELBOW[0], ELBOW[1], 0.0)
        p = part(f"upper_arm_{side}", sh)
        p.sphere(0.07, (sx * 0.19, 1.405, 0.0), "Wool", segs=max(8, n), rings=max(4, n // 2),
                 scale=(1.0, 0.75, 1.0))
        limb(p, [sh, (sx * 0.208, 1.28, 0.0), el], [0.064, 0.06, 0.053], "Wool", n)
    p = part("forearm_l", (-ELBOW[0], ELBOW[1], 0.0))
    limb(p, [(-ELBOW[0], ELBOW[1] + 0.02, 0.0), (-0.218, 1.0, -0.004),
             (-WRIST[0], WRIST[1] + 0.02, 0.0)], [0.044, 0.05, 0.046], "Wool", n)
    if full:
        limb(p, [(-WRIST[0], WRIST[1] + 0.05, 0.0), (-WRIST[0], WRIST[1] + 0.015, 0.0)],
             [0.049, 0.049], "DarkWool", n)

    # --- Left hand, round the torch: the fist, the thumb over it; the torch
    # pointing ahead and down, its lens lit.
    wx, wy = -WRIST[0], WRIST[1]
    p = part("hand_l", (wx, wy, 0.0))
    p.box((0.04, 0.085, 0.085), (wx, wy - 0.05, -0.005), "Skin", bevel=0.016, segs=2 if full else 1)
    if full:
        p.box((0.022, 0.045, 0.024), (wx + 0.012, wy - 0.035, -0.05), "Skin", bevel=0.009, segs=2)
    torch = T(wx, wy - 0.06, -0.01) @ RX(-math.pi / 2 + 0.35)   # +Z of the torch: forward, down
    p.cyl(0.017, -0.07, 0.07, "Olive", segs=n, xf=torch, chamfer=0.003 if full else 0.0)
    p.lathe([(0.017, 0.07), (0.026, 0.1), (0.026, 0.115), (0.0, 0.115)], "Olive", segs=n,
            xf=torch)
    p.cyl(0.022, 0.115, 0.118, "Lamp", segs=n, xf=torch)
    lens = torch @ Vector((0.0, 0.0, 0.118))
    if full:
        p.box((0.008, 0.012, 0.02), (0.0, 0.019, 0.02), "Black", bevel=0.002, segs=1, xf=torch)

    # --- Legs: trousers, boots.
    for side, sx in (("l", -1), ("r", 1)):
        hip = (sx * 0.09, HIP, 0.0)
        knee = (sx * 0.095, KNEE, 0.0)
        ankle = (sx * 0.1, ANKLE, 0.0)
        p = part(f"thigh_{side}", hip)
        limb(p, [(sx * 0.09, HIP + 0.04, 0.0), (sx * 0.093, 0.7, -0.005),
                 (sx * 0.095, KNEE - 0.02, 0.0)], [0.078, 0.07, 0.06], "DarkWool", n)
        # (Down into the boot top, the trousers over it: no gap at the ankle.)
        p = part(f"shin_{side}", knee)
        limb(p, [(sx * 0.095, KNEE + 0.03, 0.0), (sx * 0.097, 0.3, 0.01),
                 (sx * 0.1, ANKLE + 0.02, 0.0)], [0.05, 0.058, 0.054], "DarkWool", n)
        p = part(f"foot_{side}", ankle)
        p.box((0.1, 0.11, 0.27), (sx * 0.1, 0.062, -0.045), "Leather", bevel=0.03 if full else 0.0,
              segs=2 if full else 1)
        if full:
            p.box((0.104, 0.02, 0.28), (sx * 0.1, 0.01, -0.045), "Black", bevel=0.006, segs=1)

    # --- The bag on the left hip: a canvas satchel, its flap and buckles.
    p = part("bag", (-0.19, 1.02, -0.01))
    p.box((0.06, 0.2, 0.26), (-0.24, 0.9, 0.0), "Canvas", bevel=0.02 if full else 0.0,
          segs=2 if full else 1)
    if full:
        p.box((0.066, 0.1, 0.27), (-0.24, 0.96, 0.0), "Canvas", bevel=0.01, segs=1)
        for z in (-0.07, 0.07):
            p.box((0.07, 0.06, 0.02), (-0.24, 0.92, z), "Leather", segs=1)
            p.box((0.072, 0.012, 0.016), (-0.24, 0.9, z), "Steel", segs=1)
    p.box((0.02, 0.1, 0.035), (-0.205, 1.0, 0.0), "Canvas", segs=1)
    return parts, tuple(lens)


def lev():
    reset()
    lens = None
    for lod in (0, 1, 2):
        parts, l_ = build(lod)
        lens = lens or l_
        for p in parts.values():
            make(p)
    empty("pt_torch", lens)
    print("lev:")
    report()
    export("assets/models/props/lev.glb")


if __name__ == "__main__":
    lev()
