"""The mender: a small repair drone. A flat octagonal body on four ducted
rotors, a gas bottle slung on either flank, a camera eye up front and a
welding arm folded forward and down under the nose, a torch at its end.
Writes assets/models/robots/mender.glb.

Robot space as the drone's (src/robots/drone.rs): metres, +Y up, -Z
forward. The rotors follow drone.py: one duct (`rotor`) and one
propeller (`blades`) modelled at rotor 0's corner, (+X, +Z), and the game
turns copies to the other three corners (-X +Z, -X -Z, +X -Z) and spins
the blades about their own origin. Objects and their origins (each the
spawn offset of its part):

- `core`    (0, 0, 0)             the body, 0.55 x 0.22 x 0.55: shell over a dark
                                  belly, the arms out to the ducts, the gas
                                  bottles (0.72 across them), the shoulder's
                                  clevis, a beacon on top
- `eye`     (0, 0, -0.27)         the camera on its front face, lens -Z
- `arm`     (0, -0.12, -0.15)     the welding arm from its shoulder hinge
                                  (across X) under the body: an upper arm forward
                                  to the elbow (0, -0.17, -0.37), a forearm down to
                                  the wrist (0, -0.28, -0.43), its hose
- `torch`   (0, -0.38, -0.45)     the torch, from the wrist to its tip (the
                                  origin), the tip lit
- `rotor`   (0.42, 0.06, 0.42)    duct and motor of rotor 0, duct r 0.16
- `blades`  (0.42, 0.08, 0.42)    its propeller, r 0.13, spun by the game

Every object has `<name>_lod1` and `<name>_lod2` stand-ins (fewer sides,
no small detail).
"""

import math
import os
import sys

from mathutils import Vector

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from common import Part, RX, RY, T, export, fillet, make, report, reset

UP = RX(-math.pi / 2)  # cyl axis Z -> Y
SIDE = RY(math.pi / 2)  # cyl axis Z -> X
SEGS = (24, 12, 6)

ARM = 0.42
ROTOR = (ARM, 0.06, ARM)
BLADES = (ARM, 0.08, ARM)
EYE = (0.0, 0.0, -0.27)
SHOULDER = (0.0, -0.12, -0.15)
ELBOW = (0.0, -0.17, -0.37)
WRIST = (0.0, -0.28, -0.43)
TIP = (0.0, -0.38, -0.45)
HALF, CORNER = 0.275, 0.08  # the body's plan: an octagon


def suffix(lod):
    return "" if lod == 0 else f"_lod{lod}"


def octagon(h, c):
    return [(h, -h + c), (h, h - c), (h - c, h), (-h + c, h), (-h, h - c), (-h, -h + c),
            (-h + c, -h), (h - c, -h)]


def along(a, b):
    """A transform putting +Z from a towards b, origin at a."""
    a, b = Vector(a), Vector(b)
    rot = (b - a).to_track_quat("Z", "Y").to_matrix().to_4x4()
    return T(*a) @ rot, (b - a).length


def rod(p, a, b, r, mat, segs=8):
    xf, ln = along(a, b)
    p.cyl(r, 0.0, ln, mat, segs=segs, xf=xf)


def beam(p, a, b, w, h, mat, bevel=0.0):
    """A box beam from a to b, w across X and h deep."""
    xf, ln = along(a, b)
    p.box((w, h, ln), (0.0, 0.0, ln / 2), mat, bevel=bevel, segs=1, xf=xf)


def build(lod):
    n = SEGS[lod]
    full = lod == 0
    bev = 0.012 if lod < 2 else 0.0
    out = []

    # --- Core -------------------------------------------------------------------------
    core = Part("core" + suffix(lod))
    core.prism(octagon(HALF, CORNER), -0.07, 0.11, "Shell", axis="y", bevel=bev,
               segs=2 if full else 1)
    core.prism(octagon(HALF - 0.02, CORNER), -0.11, -0.06, "Dark", axis="y",
               bevel=0.008 if lod < 2 else 0.0, segs=1)
    # Arms out to the ducts on the diagonals.
    sec = fillet([(-0.03, -0.018), (0.03, -0.018), (0.025, 0.016), (-0.025, 0.016)],
                 0.008 if lod < 2 else 0.0, 2)
    for sx, sz in ((1, 1), (-1, 1), (-1, -1), (1, -1)):
        yaw = math.atan2(sx, sz)
        core.prism(sec, 0.3, 0.462, "Shell", axis="z", bevel=0.005 if lod < 2 else 0.0, segs=1,
                   xf=RY(yaw) @ T(0, 0.04, 0))
        if lod < 2:
            core.cyl(0.036, 0.3, 0.32, "Dark", segs=max(8, n), xf=RY(yaw) @ T(0, 0.04, 0))
    # The gas bottles, one each flank, strapped on.
    bottle = [(0.0, -0.2), (0.035, -0.196), (0.055, -0.18), (0.06, -0.15), (0.06, 0.14),
              (0.052, 0.17), (0.035, 0.185), (0.0, 0.188)]
    for sx, mat in ((-1, "Red"), (1, "Blue")):
        bx = sx * 0.31
        core.lathe(bottle, mat, segs=n, xf=T(bx, -0.02, 0))
        if lod < 2:
            # Valve and its guard ring at the back.
            core.cyl(0.016, 0.18, 0.225, "Steel", segs=max(6, n // 2), xf=T(bx, -0.02, 0))
            core.tube(0.032, 0.026, 0.205, 0.215, "Dark", segs=max(8, n // 2), xf=T(bx, -0.02, 0))
            for z in (-0.1, 0.1):
                core.tube(0.065, 0.055, z - 0.015, z + 0.015, "Dark", segs=n, xf=T(bx, -0.02, 0))
    if lod < 2:
        # The shoulder's clevis under the nose.
        for sx in (-1, 1):
            core.box((0.012, 0.07, 0.07), (sx * 0.062, -0.12, SHOULDER[2]), "Dark", segs=1)
        # A spine along the top, an amber beacon on it.
        core.box((0.07, 0.016, 0.36), (0.0, 0.115, 0.02), "Dark", bevel=0.004, segs=1)
        core.lathe([(0.0, 0.0), (0.03, 0.0), (0.026, 0.012), (0.015, 0.02), (0.0, 0.023)],
                   "Beacon", segs=max(8, n), xf=T(0, 0.122, 0.12) @ UP)
    if full:
        # Hazard stripes either side of the spine, a lit strip each side
        # of the eye.
        for sx in (-1, 1):
            core.box((0.08, 0.006, 0.03), (sx * 0.1, 0.112, -0.17), "Warn", segs=1)
            core.box((0.06, 0.016, 0.006), (sx * 0.14, 0.02, -0.277), "Glow", segs=1)
    out.append(core)

    # --- Eye --------------------------------------------------------------------------
    ex, ey, ez = EYE
    eye = Part("eye" + suffix(lod), origin=EYE)
    aim = T(ex, ey, 0) @ RY(math.pi)
    eye.cyl(0.055, 0.25, 0.292, "Dark", segs=max(8, n), xf=aim, chamfer=0.006 if lod < 2 else 0.0)
    eye.cyl(0.038, 0.288, 0.298, "Eye", segs=max(8, n), xf=aim)
    if lod < 2:
        eye.box((0.13, 0.014, 0.04), (ex, ey + 0.064, -0.29), "Dark", segs=1)
    out.append(eye)

    # --- Arm --------------------------------------------------------------------------
    arm = Part("arm" + suffix(lod), origin=SHOULDER)
    jn = max(8, n)
    arm.cyl(0.035, -0.05, 0.05, "Dark", segs=jn, xf=T(*SHOULDER) @ SIDE)
    beam(arm, SHOULDER, ELBOW, 0.05, 0.04, "Steel", bevel=0.006 if lod < 2 else 0.0)
    arm.cyl(0.03, -0.045, 0.045, "Dark", segs=jn, xf=T(*ELBOW) @ SIDE)
    beam(arm, ELBOW, WRIST, 0.044, 0.036, "Steel", bevel=0.006 if lod < 2 else 0.0)
    arm.cyl(0.026, -0.04, 0.04, "Dark", segs=jn, xf=T(*WRIST) @ SIDE)
    if lod < 2:
        # A ram along the upper arm's underside.
        s, e = Vector(SHOULDER), Vector(ELBOW)
        d = (e - s).normalized()
        down = Vector((0, -1, 0))
        off = (down - d * down.dot(d)).normalized() * 0.032
        rod(arm, s + d * 0.04 + off, s + d * 0.15 + off, 0.012, "Dark")
        rod(arm, s + d * 0.13 + off, s + d * 0.2 + off, 0.007, "Steel", segs=6)
    if full:
        # The gas hose down its side to the torch, a hazard band on it.
        pts = [(0.035, -0.12, -0.16), (0.04, -0.15, -0.3), (0.035, -0.19, -0.38),
               (0.03, -0.27, -0.43)]
        for a, b in zip(pts, pts[1:]):
            rod(arm, a, b, 0.007, "Rubber", segs=6)
        xf, ln = along(ELBOW, WRIST)
        arm.box((0.048, 0.04, 0.03), (0.0, 0.0, ln * 0.5), "Warn", segs=1, xf=xf)
    out.append(arm)

    # --- Torch ------------------------------------------------------------------------
    torch = Part("torch" + suffix(lod), origin=TIP)
    xf, ln = along(WRIST, TIP)
    torch.lathe([(0.0, -0.01), (0.02, -0.01), (0.022, 0.0), (0.022, 0.05), (0.016, 0.06),
                 (0.0, 0.06)], "Dark", segs=jn, xf=xf)
    torch.lathe([(0.0, 0.055), (0.012, 0.055), (0.01, 0.075), (0.0065, ln - 0.008),
                 (0.0, ln - 0.008)], "Steel", segs=jn, xf=xf)
    torch.lathe([(0.0, ln - 0.01), (0.0065, ln - 0.01), (0.005, ln - 0.003), (0.0, ln)],
                "Glow", segs=jn, xf=xf)
    if lod < 2:
        # A shield ring round the nozzle.
        torch.tube(0.026, 0.01, 0.06, 0.066, "Steel", segs=jn, xf=xf)
    out.append(torch)

    # --- Rotor duct (rotor 0) ----------------------------------------------------------
    rotor = Part("rotor" + suffix(lod), origin=ROTOR)
    at = T(*ROTOR)
    dn = (48, 24, 12)[lod]
    rotor.lathe([(0.14, -0.025), (0.16, -0.025), (0.161, 0.015), (0.155, 0.032), (0.146, 0.036),
                 (0.14, 0.03)], "Shell", segs=dn, xf=at @ UP, closed=True)
    rotor.lathe([(0.14, -0.035), (0.152, -0.037), (0.16, -0.025), (0.14, -0.025)], "Dark",
                segs=dn, xf=at @ UP, closed=True)
    rotor.lathe([(0.0, -0.03), (0.02, -0.028), (0.03, -0.012), (0.03, 0.01), (0.024, 0.016),
                 (0.0, 0.017)], "Dark", segs=max(8, n), xf=at @ UP)
    for k in range(3):
        a = math.radians(30 + 120 * k)
        rotor.box((0.118, 0.008, 0.012), (0.085, -0.016, 0.0), "Dark", segs=1, xf=at @ RY(a))
    if lod < 2:
        rotor.lathe([(0.1605, -0.012), (0.166, -0.012), (0.166, 0.012), (0.1605, 0.012)], "Glow",
                    segs=dn, xf=at @ UP, closed=True)
    out.append(rotor)

    # --- Propeller ---------------------------------------------------------------------
    blades = Part("blades" + suffix(lod), origin=BLADES)
    bat = T(*BLADES)
    blades.cyl(0.014, -0.006, 0.006, "Rotor", segs=max(8, n), xf=bat @ UP,
               chamfer=0.003 if lod < 2 else 0.0)
    blade = fillet([(0.01, -0.008), (0.128, -0.012), (0.131, 0.0), (0.124, 0.009), (0.01, 0.01)],
                   [0.0, 0.004, 0.004, 0.004, 0.0], 2 if lod < 2 else 1)
    for k in range(3):
        blades.prism(blade, -0.0015, 0.0015, "Rotor", axis="y",
                     bevel=0.0008 if lod < 2 else 0.0, segs=1,
                     xf=bat @ RY(math.radians(120 * k)))
    out.append(blades)
    return out


reset()
for lod in (0, 1, 2):
    for p in build(lod):
        make(p)
report()
export("assets/models/robots/mender.glb")
