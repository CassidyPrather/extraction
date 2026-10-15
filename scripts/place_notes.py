#!/usr/bin/env python3
"""Writes the notes, signs and terminals into the map RONs.

Each entry names a building by its placement (prefab and position), a spot
in the building's own frame (a table, a counter) and what it says; this
works out the world position, so moving a building in a map and re-running
keeps its notes on its table. The generated block sits between
`// notes:begin` and `// notes:end` in each map's markers.

    scripts/place_notes.py
"""
import math
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# Table and counter spots in each prefab's frame (x, z), from
# src/world/buildings.rs, and how high above the footing to look (the
# ranger tower's table is up in its cabin).
SPOTS = {
    "house_plank": [(-1.6 + 0.3, -0.7)],
    "farmhouse": [(-2.0 + 0.3, -0.8)],
    "house_brick": [(-2.0 + 0.3, -0.8)],
    "cabin_log": [(0.3, 0.55)],
    "shop": [(-1.6, -0.4)],
    "pump_station": [(5.5 + 0.3, -3.0)],
    "bus_depot": [(12.0 + 0.2, 6.5)],
    "mast_hut": [(0.8, 1.4)],
    "guard_booth": [(0.3, 0.6)],
    "ranger_tower": [(0.3, 0.6)],
    "factory_hall": [(-12.6 + 0.4, 7.0)],
}
REACH = {"ranger_tower": 8.5}


def world(at, rot, local):
    """The game's yaw (Quat::from_rotation_y(-rot)) applied to a local point."""
    r = math.radians(rot)
    lx, lz = local
    return (at[0] + lx * math.cos(r) - lz * math.sin(r), at[1] + lx * math.sin(r) + lz * math.cos(r))


def facing(at, frm):
    """Marker rot whose screen (local +Z) faces from `at` toward `frm`."""
    dx, dz = frm[0] - at[0], frm[1] - at[1]
    return math.degrees(math.atan2(-dx, dz))


def ron_str(s):
    return '"' + s.replace("\\", "\\\\").replace('"', '\\"').replace("\n", "\\n") + '"'


# (prefab, at, kind, text or pages, name). Notes lie on the building's
# table; terminals sit on it facing the room's middle.
NOTES = {
    "marrow_creek": [
        ("house_plank", (-18.0, 95.0), "Note", """Anna -

They said three days. The buses left at six. I stayed for the dog.

The white machines came out of the Gate the next morning and started
tidying. They took the bins. They took the bicycles. They took Mr Holm.

Keep still when the little flying one looks at you. It sees you move.""", "Village note"),
        ("house_brick", (-20.0, 65.0), "Note", """POST OFFICE - UNDELIVERED

To the station chief, Hollow survey post.
Your lockbox key for the Marrow depot is to stay with you. Do not leave
it at the post. The Authority will collect all three keys at the review.

- Quarantine Authority, valley office""", "Post office letter"),
        ("cabin_log", (-185.0, 190.0), "Note", """Walkers:

Don't waste rounds on the shell. The eye under the front plate, or the
knees. They go down like a felled tree.

The flyers are glass and wire. One good hit.

And never shoot near the well. It doesn't like it.

- J.""", "Foreman's note"),
        ("pump_station", (-200.0, 22.0), "Note", """The well in the village takes what
you give it. Food, scrap, the bright stones. Sometimes it gives back:
a calm hour, a whisper of where things are. Sometimes nothing.

Don't give it your gun.""", "Pump station note"),
        ("bus_depot", (25.0, -225.0), "Terminal", [
            """QUARANTINE AUTHORITY
CUSTODIAN NETWORK - MARROW

Depot GATE-M: ACTIVE
Mode: SWEEP
Units: survey, haul, build,
       recover

The Custodian keeps the valley
clean until the review.""",
            """DEPOT ACCESS

The depots answer only to the
Authority. Recall needs the
three valley keys, presented
together at any depot lock:

  KEY-M  Marrow  (Hollow post)
  KEY-K  Kettle  (quarry)
  KEY-C  Cinder  (the works)""",
            """REVIEW STATUS

Review date: (not set)
Authority staff on site: 0

Custodian continues SWEEP until
recalled.""",
        ], "Depot office terminal"),
        ("mast_hut", (-112.0, -160.0), "Terminal", [
            """RELAY LOG - MAST M1

Pylons extend the Custodian's
eyes: each one reports what
moves near it, and the depot
sends a team.

Pylon down = reports stop.""",
            """SWEEP SCHEDULE

Field discharge ("SURGE") every
cycle. Clears contamination.
Personnel: be under a roof or
out of the valley. Loose
material is reset.""",
        ], "Mast terminal"),
        ("ranger_tower", (-240.0, -138.0), "Note", """From up here you can see the Hollow
glow after dark. The survey team went in with the chief's lockbox and
came back without the chief. The box is still down there, where the
ground lifts. Bring a light. Don't run.""", "Ranger's note"),
    ],
    "kettle_pass": [
        ("house_brick", (-55.0, 60.0), "Note", """MINE OFFICE

The Authority man took the quarry blast-shelter key and locked the
Kettle depot key in the shelter "for safe keeping". Then the shelter
started to burn without fire. Nobody goes up to the quarry now.""", "Mine office note"),
        ("house_brick", (-8.0, 150.0), "Note", """Sunday. Nobody came to the chapel
but the machines. One of them stood at the back the whole service,
very still, and then cleaned the windows.

The pass road is the only way the trucks still come in.""", "Chapel note"),
        ("pump_station", (-200.0, -100.0), "Terminal", [
            """CUSTODIAN NETWORK - KETTLE

Depot GATE-K: ACTIVE
Mode: SWEEP

Recovered personnel are kept
at the depot pending review.""",
            """KEY-K custody: quarry blast
shelter, Kettle.

Note: field activity near the
quarry. Unshielded personnel
should not approach.""",
        ], "Pump station terminal"),
        ("mast_hut", (75.0, -210.0), "Terminal", [
            """RELAY LOG - MAST K2

Grapplers carry stock to the
work sites. Dirigibles drop
pods where walkers are short.

A recovered body goes to the
depot. Everything is kept.""",
        ], "Mast terminal"),
        ("cabin_log", (205.0, -205.0), "Note", """Lodge book.

Snow on the saddle again. The machines walked the fence line all
night, very slowly, as if counting the posts.

If you hear the hum, you're carrying too many of the bright stones. Put some down.""", "Lodge book"),
    ],
    "cinder_works": [
        ("factory_hall", (75.0, -20.0), "Terminal", [
            """WORKS FOREMAN - LOG

We fed the furnace what the
machines brought in. Scrap,
cars, furniture. They brought
more every day.""",
            """Then the fire got into the
valley and didn't go out. The
Authority took the works keys
and the depot key away.

KEY-C is in the strongbox at
the foot of the chimney.""",
        ], "Foreman's terminal"),
        ("bus_depot", (-20.0, -30.0), "Terminal", [
            """CUSTODIAN NETWORK - CINDER

Depot GATE-C: ACTIVE
Mode: SWEEP (heat damage)

Recall requires KEY-M, KEY-K,
KEY-C at any depot lock.""",
        ], "Garage terminal"),
        ("house_brick", (-110.0, 175.0), "Note", """The ash doesn't stop. It isn't from
the fire any more. It comes out of the ground near the slag heaps.
The works chimney is the best landmark: walk toward it in the fog and
you'll find the road.""", "Foreman's house note"),
        ("house_plank", (0.0, 150.0), "Note", """If you're reading this you came in on
the driver's truck. He's all right. He won't come past the fence.

Pull the cord in the back when you want out. Don't be late for the
Surge.""", "House note"),
    ],
}


def block(map_name, placements):
    lines = ["        // notes:begin (generated by scripts/place_notes.py)"]
    for prefab, at, kind, body, name in NOTES[map_name]:
        rot = next((r for (p, a, r) in placements if p == prefab and abs(a[0] - at[0]) < 0.01 and abs(a[1] - at[1]) < 0.01), None)
        if rot is None:
            sys.exit(f"{map_name}: no {prefab} at {at}")
        spot = SPOTS[prefab][0]
        x, z = world(at, rot, spot)
        y = REACH.get(prefab, 0.0)
        if kind == "Note":
            # Paragraphs are separated by blank lines; within one, lines
            # are joined (the game wraps them to the paper).
            body = "\n".join(" ".join(p.split()) for p in body.split("\n\n"))
            # Read from the room's middle side.
            face = facing((x, z), at) + 180.0
            k = f"Note(text: {ron_str(body)})"
        else:
            face = facing((x, z), at)
            pages = ", ".join(ron_str(p) for p in body)
            k = f"Terminal(pages: [{pages}])"
        y_part = f", y: {y:.1f}" if y else ""
        lines.append(f"        (kind: {k}, at: ({x:.2f}, {z:.2f}), rot: {face:.1f}{y_part}, name: {ron_str(name)}),")
    lines.append("        // notes:end")
    return "\n".join(lines)


def main():
    for map_name in NOTES:
        path = ROOT / "assets" / "maps" / f"{map_name}.ron"
        text = path.read_text()
        placements = [
            (m.group(1), (float(m.group(2)), float(m.group(3))), float(m.group(4)))
            for m in re.finditer(r'\(prefab: "(\w+)", at: \(([-\d.]+), ([-\d.]+)\), rot: ([-\d.]+)', text)
        ]
        new = block(map_name, placements)
        if "// notes:begin" in text:
            text = re.sub(r"        // notes:begin.*?// notes:end", lambda _: new, text, flags=re.S)
        else:
            i = text.index("    markers: [")
            j = text.index("\n    ],", i)
            text = text[:j] + "\n" + new + text[j:]
        path.write_text(text)
        print(f"{map_name}: {len(NOTES[map_name])} notes and terminals")


if __name__ == "__main__":
    main()
