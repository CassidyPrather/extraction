#!/usr/bin/env python3
"""walk_stops.py <walk.json> <out.stills> <out_stops.json>: the stops of a
walk review (docs/craft/WALK_REVIEW.md) along its route.

The route is `map_openness::probe_walk`'s: the drop to the career's first
job, then on to the nearest rendezvous. A stop every 40 m, and at every
fork (two roads meeting by the route), door (within 12 m), crest (a high
point 1.5 m over the ground 30 m either side) and the job and the
rendezvous; stops closer than 12 m merge, the more particular kept. Each
gets two views at eye height: along the route, and to the side, at the
door if it's a door or else the nearest place, rendezvous or landmark
standing 40 degrees or more off the way ahead."""
import json, math, sys
d = json.load(open(sys.argv[1]))
route = [tuple(p) for p in d["to_job"]] + [tuple(p) for p in d["to_rv"]][1:]
job_i = len(d["to_job"]) - 1
# resample 1 m
pts, arc, marks = [], [], {}
acc = 0.0
for i, (a, b) in enumerate(zip(route, route[1:])):
    L = math.dist(a[:2], b[:2])
    n = max(1, int(L))
    for k in range(n):
        t = k / n
        pts.append((a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t))
        arc.append(acc + L * t)
    acc += L
    if i + 1 == job_i:
        marks["job"] = acc
pts.append(route[-1]); arc.append(acc)
total = acc
def at(s):
    k = min(range(len(arc)), key=lambda i: abs(arc[i] - s))
    return k
def yaw_to(a, b):
    return math.degrees(math.atan2(b[0] - a[0], -(b[1] - a[1]))) % 360
stops = []
def add(s, kind, note=""):
    stops.append({"s": s, "kind": kind, "note": note})
add(0.0, "drop")
s = 40.0
while s < total - 10:
    add(s, "pace"); s += 40.0
add(marks.get("job", total), "job", d["job"])
add(total, "rendezvous", d["rv"])
# junctions: points where two roads meet (within 4 m), near the route (8 m)
road_pts = [(r["name"], tuple(q)) for r in d["roads"] for q in r["points"][::3]]
seen = set()
for i in range(0, len(pts), 3):
    p = pts[i]
    names = {n for n, q in road_pts if math.dist(p[:2], q) < 6.0}
    if len(names) >= 2:
        key = tuple(sorted(names))
        if key not in seen:
            seen.add(key); add(arc[i], "fork", " / ".join(sorted(names)))
# doors within 12 m
for door in d["doors"]:
    f = door["face"]
    k = min(range(len(pts)), key=lambda i: math.dist(pts[i][:2], f))
    if math.dist(pts[k][:2], f) < 12.0:
        add(arc[k], "door", door["prefab"])
# crests: local max with 1.5 m prominence within 30 m
for i in range(len(pts)):
    lo = [j for j in range(len(pts)) if abs(arc[j] - arc[i]) <= 30]
    if pts[i][2] >= max(pts[j][2] for j in lo) and pts[i][2] - min(pts[j][2] for j in lo) >= 1.5 and 5 < arc[i] < total - 5:
        add(arc[i], "crest")
# merge within 12 m: keep the more specific
prio = {"drop": 0, "job": 0, "rendezvous": 0, "fork": 1, "door": 1, "crest": 2, "pace": 3}
stops.sort(key=lambda x: (x["s"], prio[x["kind"]]))
merged = []
for st in sorted(stops, key=lambda x: prio[x["kind"]]):
    if all(abs(st["s"] - m["s"]) >= 12 for m in merged):
        merged.append(st)
merged.sort(key=lambda x: x["s"])
# views
notable = [(p["name"], tuple(p["at"])) for p in d["places"]] + [(r["name"], tuple(r["at"])) for r in d["rvs"]]
LAND = ("chimney", "steeple", "chapel", "water_tower", "radio_mast", "ranger_tower", "headframe", "survey_tower", "blast_furnace", "church_ruin", "observatory")
notable += [(p["prefab"], tuple(p["at"])) for p in d["placements"] if p["prefab"] in LAND]
lines, out = [], []
for n, st in enumerate(merged, 1):
    k = at(st["s"]); p = pts[k]
    # An arrival (the job, the rendezvous) faces the way you came in; any
    # other stop, the way on.
    if st["kind"] in ("job", "rendezvous"):
        fwd = yaw_to(pts[at(max(0.0, st["s"] - 15))], p)
    else:
        ahead = pts[at(min(total, st["s"] + 15))]
        fwd = yaw_to(p, ahead) if math.dist(ahead[:2], p[:2]) > 1 else 0.0
    side, why = None, ""
    if st["kind"] == "door":
        door = min(d["doors"], key=lambda dd: math.dist(dd["face"], p[:2]))
        side, why = yaw_to(p, door["face"]), "the " + door["prefab"] + "'s door"
    else:
        cands = []
        for nm, q in notable:
            dd = math.dist(q, p[:2])
            if 15 < dd < 220:
                y = yaw_to(p, q); off = abs((y - fwd + 180) % 360 - 180)
                if off > 40:
                    cands.append((dd, y, nm))
        if cands:
            dd, side, why = min(cands); why = f"{why} ({dd:.0f} m)"
        else:
            side, why = (fwd + 90) % 360, "right"
    name = f"s{n:02d}"
    lines.append(f"{name}_ahead {p[0]:.1f},0,{p[1]:.1f},{fwd:.1f},-3")
    lines.append(f"{name}_side {p[0]:.1f},0,{p[1]:.1f},{side:.1f},-3")
    out.append({"n": n, "name": name, "s": round(st["s"]), "kind": st["kind"], "note": st["note"], "at": [round(p[0], 1), round(p[1], 1)], "h": round(p[2], 1), "fwd": round(fwd), "side": round(side), "side_what": why})
open(sys.argv[2], "w").write(f"# walk review: {d['map']} {d['job']} then {d['rv']}; name x,y,z,yaw,pitch.\n" + "\n".join(lines) + "\n")
json.dump(out, open(sys.argv[3], "w"), indent=1)
print(d["map"], f"{total:.0f} m,", len(out), "stops")
for o in out: print(o["name"], o["s"], o["kind"], o["note"], o["at"], o["h"], "side:", o["side_what"])
