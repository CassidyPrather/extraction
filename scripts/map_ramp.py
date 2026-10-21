#!/usr/bin/env python3
"""Print a flight of terrace plateaus that make a ramp the land can't give.

Roads in a map follow the terrain (graded over their `smoothing`), so a
road can't climb a cliff by itself. Lay this flight under it and the
road's own grading evens the steps into a slope a body walks; off the
road they read as steps (Sallow Reservoir's face stair, spillway and the
roads into New Sallow, docs/FOURTH_VALLEY.md).

    scripts/map_ramp.py NAME H0 H1 STEP WIDTH x,z x,z ...

From height H0 at the polyline's start to H1 at its end, one terrace
every STEP metres of arc, WIDTH wide. Paste the output into the map's
`features`, after any plateau it must override (later plateaus win).
Start H0 at the ground's height where the road meets it (scripts/
map_probe.py), give the road at least the flight's width plus its
shoulders, and a `smoothing` of about the step length: a route needs
every 2 m nav cell under 16 degrees.
"""
import math
import sys


def main():
    if len(sys.argv) < 8:
        sys.exit(__doc__)
    name, h0, h1, step, width = sys.argv[1], *map(float, sys.argv[2:6])
    pts = [tuple(map(float, a.split(","))) for a in sys.argv[6:]]
    seg = list(zip(pts, pts[1:]))
    lens = [math.dist(a, b) for a, b in seg]
    total = sum(lens)
    n = max(1, round(total / step))

    def at(s):
        for (a, b), l in zip(seg, lens):
            if s <= l or (a, b) == seg[-1]:
                t = min(s / l, 1.0)
                d = ((b[0] - a[0]) / l, (b[1] - a[1]) / l)
                return (a[0] + d[0] * l * t, a[1] + d[1] * l * t), d
            s -= l

    print(f"        // {name}: {n} terraces, {h0:.1f} to {h1:.1f} over {total:.0f} m.")
    w = width / 2
    for k in range(n):
        (p0, d0), (p1, d1) = at(total * k / n), at(total * (k + 1) / n)
        h = h0 + (h1 - h0) * (k + 1) / n
        side = lambda p, d, s: (p[0] - d[1] * w * s, p[1] + d[0] * w * s)
        # Overlap the next terrace by half a metre so no gap opens.
        p1 = (p1[0] + d1[0] * 0.5, p1[1] + d1[1] * 0.5)
        poly = [side(p0, d0, 1), side(p1, d1, 1), side(p1, d1, -1), side(p0, d0, -1)]
        s = ", ".join(f"({x:.1f}, {z:.1f})" for x, z in poly)
        print(f"        Plateau(polygon: [{s}], height: Some({h:.2f}), blend: 2.5),")


if __name__ == "__main__":
    main()
