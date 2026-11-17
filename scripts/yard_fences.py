# Fence runs for the training yard's bays: each line (axis-aligned) from a
# to b, with 4 m gates centred at the given points, cut into 6 m runs and
# 2 m fillers. Prints RON placements.
LINES = [
    # (comment, axis, fixed, lo, hi, [(gate_centre, n)])
    ("Bay 1's back (south) fence, inside the yard's own", "z", 100, -34, 10, []),
    ("Bays 1-2, west", "x", -34, 74, 100, []),
    ("Bays 1-2, east", "x", 10, 74, 100, []),
    ("Bay 1 | bay 2", "z", 84, -34, 10, [(0, 1)]),
    ("Bay 2 | the range", "z", 74, -34, 10, [(0, 2)]),
    ("The range and its lane, west", "x", -24, -24, 74, []),
    ("The range, east", "x", 10, 38, 74, []),
    ("The range | bay 4 (the house)", "z", 38, -6, 36, [(0, 3)]),
    ("The lane | bays 4-6", "x", -6, -48, 38, []),
    ("The lane's far end", "z", -24, -24, -6, []),
    ("Bay 4, east", "x", 36, 12, 38, []),
    ("Bay 4 | bay 5 (first aid)", "z", 12, -6, 36, [(0, 4)]),
    ("Bay 5, east", "x", 30, -16, 12, []),
    ("Bay 5 | bay 6 (the shimmer)", "z", -16, -6, 30, [(0, 5)]),
    ("Bay 6, east", "x", 20, -48, -16, []),
    ("Bay 6 | bay 7 (the extract)", "z", -48, -10, 20, [(0, 6)]),
    ("Bay 7, west", "x", -10, -82, -48, []),
    ("Bay 7, east", "x", 14, -82, -48, []),
    ("Bay 7, north", "z", -82, -10, 14, []),
]

def pieces(lo, hi):
    out = []
    x = lo
    while hi - x >= 6 - 1e-6:
        out.append(("yard_fence", x + 3)); x += 6
    while hi - x >= 2 - 1e-6:
        out.append(("yard_fence_2", x + 1)); x += 2
    assert abs(hi - x) < 1e-6, (lo, hi, x)
    return out

# Runs on ground that rises over 0.3 m along 6 m: in 2 m fillers.
SPLIT = {(10, 71), (14, -67)}
lines = []
total = 0
for comment, axis, fixed, lo, hi, gates in LINES:
    lines.append("        // %s." % comment)
    segs, x = [], lo
    for c, n in sorted(gates):
        segs.append((x, c - 2)); x = c + 2
    segs.append((x, hi))
    items = []
    for a, b in segs:
        if b - a > 1e-6:
            items += pieces(a, b)
    for c, n in gates:
        items.append(("yard_gate", c, n))
    items.sort(key=lambda it: it[1])
    split = []
    for it in items:
        at = (it[1], fixed) if axis == "z" else (fixed, it[1])
        if it[0] == "yard_fence" and at in SPLIT:
            split += [("yard_fence_2", it[1] - 2), ("yard_fence_2", it[1]), ("yard_fence_2", it[1] + 2)]
        else:
            split.append(it)
    for it in split:
        pf, s = it[0], it[1]
        at = (s, fixed) if axis == "z" else (fixed, s)
        rot = 0.0 if axis == "z" else 90.0
        tag = ', tag: "gate_%d"' % it[2] if pf == "yard_gate" else ""
        lines.append('        (prefab: "%s", at: (%.1f, %.1f), rot: %.1f%s),' % (pf, at[0], at[1], rot, tag))
        total += 1
print("\n".join(lines))
import sys; print("count", total, file=sys.stderr)
