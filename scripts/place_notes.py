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
    "house_plank_damaged": [(-1.6 + 0.3, -0.7)],
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
    "chapel": [(0.3, 14.0 * 0.5 - 1.6)],
    # Sallow's (src/world/dam.rs): the tagging desk under the gantry, and
    # the table in the draw-off tower's valve room at the bridge floor.
    "sorting_gantry": [(-6.0, 0.0)],
    "draw_off_tower": [(-1.0, 1.9)],
}
REACH = {"ranger_tower": 8.5, "draw_off_tower": 20.0}


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
# table; terminals sit on it facing the room's middle. Who wrote what, and
# why, is in docs/LORE.md; src/world/story.rs checks that every place a
# note names exists.
NOTES = {
    "marrow_creek": [
        ("guard_booth", (47.8, 285.0), "Note", """SOUTH GATE - DUTY LOG, 14 OCT

0600. Buses out of the depot, 9 of 9.

0655. Bus 4 back, empty. Driver asks to go in for his mother. Refused.

0710. Driver still here. Refused again. Escorted to the road.

1300. A dog barking in the village all afternoon.

2200. Lights by the well. Not ours.""", "Gate log"),
        ("house_plank", (-18.0, 95.0), "Note", """Anna -

They said three days. That was three weeks ago. Lev drove the four and
wouldn't look at me, so I stayed for the dog.

The white machines came out of the new Gate this morning and started
tidying. They took the bins. They took the bicycles. They took Mr Holm.

Keep still when the little flying one looks at you. It sees you move.

Tell Lev I'm not angry. I've written to him as well, for when he comes.

- Mum""", "Village note"),
        ("house_brick", (-20.0, 65.0), "Note", """POST OFFICE - UNDELIVERED

To O. Vey, station chief, Hollow survey post.

Your lockbox holds the depot key for Marrow (KEY-M). It is to stay with
you. Do not leave it at the post. The Authority will collect all three
keys at the review.

P. Rus, Inspector
Quarantine Authority, valley office, 20 Nov.""", "Post office letter"),
        ("shop", (-16.0, 35.0), "Note", """CREDIT BOOK

Holm - bread, paraffin. 3.40

Sedlak - batteries x6, lamp oil. 5.10

Marek H. - bread, dog biscuits x4, tea. 2.80

Brabec - nails, tea. 1.95

Vey (survey) - batteries x12, chalk. Paid.

13 Oct. Shut till further notice. Pay me when you're back.""", "Credit book"),
        ("cabin_log", (-185.0, 190.0), "Note", """Walkers:

Don't waste rounds on the shell. The eye under the front plate, or the
knees. They go down like a felled tree.

The flyers are glass and wire. One good hit.

And never shoot near the well. It doesn't like it.

- J., logging camp""", "Foreman's note"),
        ("pump_station", (-200.0, 22.0), "Note", """PUMP LOG - O. Brabec

The Authority says the Surge is theirs: "scheduled discharge". It was
here first. 9 Oct, before any machine, the sky went the colour of a
bruise and every needle in this station lay down.

The well in the village takes what you give it. Sometimes it gives back:
a quiet hour, a word about where the stones are.

My grandmother fed it bread. It was slower then.

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
            """KEPT PENDING REVIEW

Buses                  9
Bicycles             212
Refuse bins          340
Domestic animals      58
Personnel             31

Condition: kept.""",
            """DEPOT ACCESS

The depots answer only to the
Authority. Recall needs the
three valley keys, presented
together at any depot lock:

  KEY-M  Marrow  (Hollow post)
  KEY-K  Kettle  (quarry)
  KEY-C  Cinder  (the works)""",
            """REVIEW STATUS

Review date: 1 APR (postponed)
Review date: (not set)
Authority staff on site: 0
Last login: P.RUS (Cinder)

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
material is reset.

Schedule source: OBSERVED.""",
        ], "Mast terminal"),
        ("ranger_tower", (101.0, -2.0), "Note", """SURVEY POST - HOLLOW

21 Nov. Ground up 41 cm since spring. The glow is brighter the nights
before a discharge; I can predict them now. Nobody at the Authority
wants to hear it.

The inspector's letter never came, but I know what the lockbox is for.
Nobody else is going to keep it.

Going down at first light to read the centre stakes. Taking the box.

- O.V.""", "Survey log"),
        ("ranger_tower", (-240.0, -138.0), "Note", """From up here you can see the Hollow
glow after dark. The survey team went in with the chief's lockbox and
came back without the chief. The box is still down there, where the
ground lifts. Bring a light. Don't run.

Lamp to Kettle tower, 2100: no answer from Emil three nights now.

- M. Sedlak""", "Ranger's note"),
        ("cabin_log", (-145.0, 190.0), "Note", """Crew tally, on the wall by the bed:

Mon 14 logs   Tue 16   Wed 15
Thu - buses

There's a tin of beans under the bed for whoever's next. Wedge the door
at night with the split log, the latch is no good.

- Tomas, J.'s crew""", "Cabin tally"),
    ],
    "kettle_pass": [
        ("guard_booth", (127.8, 285.0), "Note", """SOUTH GATE, KETTLE - DUTY LOG

3 Jan. Snow. Nothing in, nothing out.

The machines walked the fence again at 0200. Counting posts, Novak
says. He's been up here too long.

12 Feb. Smoke over the hill from Cinder. Orders: none.

1 Apr. Review postponed. Post to stand down. Leave the stove.""", "Gate log"),
        ("house_brick", (-55.0, 60.0), "Note", """MINE OFFICE

Inspector Rus took Karel's key to the quarry blast shelter and locked
the Kettle depot key inside "for safe keeping". He signed for it in our
book, as if that made it ours.

A week later the shelter started to burn without fire. Nobody goes up
to the quarry now.

- Irena, clerk""", "Mine office note"),
        ("house_plank", (-30.0, 72.0), "Note", """K. Dusek, shot-firer.

Shelter: poured concrete, 30 cm. Steel door, one lock. My key, until
the inspector took it.

Since November the air in the pit shimmers like over a stove, and
nothing is burning. Bright stones on the benches, warm. My meter clicks
at them.

I have fired four hundred shots up there. I know what a shelter is for.
That one is keeping something in.

Vey came up from Marrow to look. He went white.""", "Shot-firer's note"),
        ("house_plank", (76.0, 222.0), "Note", """Last train out, 15 Oct, 1140.
Eleven wagons, people on the roofs.

I closed the crossing gates behind it like always. Nobody told me to
stop.

Every morning since, I close them at 1140.

The machines wait for the gates now. They stand at the barrier, very
patient, till I lift it.""", "Crossing keeper's note"),
        ("chapel", (-8.0, 150.0), "Note", """Sunday. Nobody came to the chapel
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
Signed: P.RUS

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
            """LINK STATUS

M1  MARROW          OK
C3  CINDER          DEGRADED
AUTHORITY UPLINK    NO CARRIER
  since 1 APR (3851 days)

Holding reports for uplink.""",
        ], "Mast terminal"),
        ("ranger_tower", (-48.0, -233.0), "Note", """KETTLE TOWER

Marta's lamp from Marrow, 2100, three long: all well. Answered.

The quarry glows now too, like her Hollow. Lower, redder.

The walkers come up the pass road at night with their arms full, and go
down empty.

The lamp battery is nearly gone. If I stop answering, Marta, it's the
battery.

- Emil""", "Ranger's log"),
        ("house_plank", (-99.0, 154.0), "Note", """WEIR BOOK - KETTLE BECK

Level 0.62. Clear.

Level 0.60. Clear. The machines took the eel trap.

Level 0.61. The water tastes of pennies the day after a Surge, then it
doesn't.

Level 0.58. Nobody to send these to. Writing them anyway.""", "Weir book"),
        ("mast_hut", (-40.0, -128.0), "Note", """Lorry 12, 15 Oct.

Stuck on the third bend with forty from Miners' Row in the back. Walked
them over the pass on foot, the old ones on the soldiers' shoulders.

Left the lorry on the bend with the keys in. The machines had it by
morning.""", "Lorry driver's note"),
        ("pump_station", (125.0, -42.0), "Note", """QUARRY PUMPS

Pit dry at 0600. Pumps one and two running.

Since November the pit fills again every night, and the water comes up
warm.

Turned the pumps off. Whatever is down there, it wants to be under
water. Let it.""", "Quarry pumps note"),
        ("cabin_log", (205.0, -205.0), "Note", """Lodge book.

Snow on the saddle again. The machines walked the fence line all
night, very slowly, as if counting the posts.

If you hear the hum, you're carrying too many of the bright stones. Put some down.

- Fiala""", "Lodge book"),
        ("cabin_log", (119.0, -138.0), "Note", """The kiln wants four days' watching, so it's getting four days.

They say the buses are the last. Fine. If it's cold when you come, it went
out without me and I walked down after all.

Bed's dry. My name's on the frame so you know whose it was.

- Mirek Hajek, burner""", "Burner's note"),
    ],
    "cinder_works": [
        ("guard_booth", (-102.2, 285.0), "Note", """SOUTH GATE, CINDER - DUTY LOG

19 Feb. Fire in the north-west wood since the 12th. The works still
burning loads; machines in from the Gate every hour.

26 Feb. Wind turned. Ash on everything. The rations taste of it.

1 Apr. Review postponed. We stand down. The inspector stays: says he'll
wait for them in the works garage.""", "Gate log"),
        ("factory_hall", (75.0, -20.0), "Terminal", [
            """WORKS LOG - R. KRAL

Authority contract: burn all
material the machines deliver.

Nov: cars, bedding, furniture.
Dec: more. Toys. A piano.
Jan: we stopped asking.""",
            """12 Feb. Furnace three blew
back into the yard. The wood
caught by evening.

The machines kept delivering
all through the fire.""",
            """The inspector locked the
Cinder depot key in the
strongbox at the foot of the
chimney, and kept our keys.

KEY-C is down there still.
He sits in the garage with
the radio, waiting.""",
        ], "Foreman's terminal"),
        ("bus_depot", (-20.0, -30.0), "Terminal", [
            """CUSTODIAN NETWORK - CINDER

Depot GATE-C: ACTIVE
Mode: SWEEP (heat damage)

Recall requires KEY-M, KEY-K,
KEY-C at any depot lock.""",
            """SESSION LOG

P.RUS   status       ACTIVE
P.RUS   review date  NOT SET
P.RUS   review date  NOT SET
P.RUS   review date  NOT SET
P.RUS   recall       KEYS 0/3

No session since 9 JUN.""",
        ], "Garage terminal"),
        ("house_brick", (-110.0, 175.0), "Note", """Ruben says the ash is from the fire.
The fire was out by March. I sweep the step every morning, and every
morning there's more, and it isn't coming down. It's coming up, out of
the cracks, worst by the slag heaps.

The chimney is the best landmark: walk toward it in the fog and you'll
find the road.

- V.""", "Foreman's house note"),
        ("house_plank_damaged", (0.0, 150.0), "Note", """Came in on the driver's truck, same as
you, I expect. He's all right. He won't come past the fence. I asked
him why once. He said he tried, the first day, and they turned him back,
and now he's used to it.

Pull the cord in the back when you want out. Don't be late for the
Surge.

He keeps a count of us. Don't add to it.""", "House note"),
        ("ranger_tower", (-178.0, -168.0), "Note", """FIRE LOOKOUT - CINDER, 12 FEB

1540. Black smoke, works yard.

1610. Flame in the north-west wood. Phoned the works. No answer.

1700. Wind south-east. Lamp to Marrow and Kettle: three short.

2300. Machines walking into the fire with their arms full, and out
empty.

Marta answered. Kettle didn't.""", "Lookout log"),
        ("pump_station", (-172.0, 32.0), "Note", """SLUICE HOUSE - ASH PIT

Works order: drain the pit to the canal.

The pit won't drain. The ash floats. It drifts against the current, back
toward the slag heaps.

Sluice shut. I'm not opening it again.""", "Sluice house note"),
        ("mast_hut", (95.0, -238.0), "Terminal", [
            """RELAY LOG - MAST C3

Heat damage: housing, cabling.
Pylon reports: degraded.
Repair requested: 1 FEB.
Repair requested: 1 MAR.
Repair requested: 1 APR.
...

Requests held for uplink.""",
        ], "Mast terminal"),
        ("cabin_log", (-75.0, -159.0), "Note", """Woodward's book, last page.

The works' smoke came down the valley at noon and didn't lift. I've wet
the roof and the woodpile and I'm stopping here.

Matches by the stove. The door sticks, kick it at the
bottom, then wedge it.

- T. Vesely, woodward""", "Woodward's book"),
    ],
    # The fourth valley (claude/polish-fourth-valley): skipped until its
    # map is in.
    "sallow_reservoir": [
        ("guard_booth", (32.8, 285.0), "Note", """SOUTH GATE, SALLOW - DUTY LOG

15 Oct. New Sallow out on the Marrow buses, second run. The old ones
slow to board. One says they were moved for the water once already and
once was enough.

16 Oct. The keeper won't leave the works. Water board says let him,
someone has to mind the dam.

2 Nov. Something built on the north-east shore overnight. Not ours. Not
the water board's.""", "Sallow gate log"),
        ("house_brick", (-58.0, 206.0), "Note", """Whoever has the cottage now -

I was seven when the water board moved us up the hill. We watched the
water come over the bridge a hand a day.

The second winter the machines opened the scours, and it went down a
hand a day. On the twentieth day the church came out. Our house is the
third from the bridge.

Don't stand in the churchyard after rain. Things come up there that
weren't buried.

- A. Pech, keeper""", "Keeper's note"),
        ("pump_station", (12.0, 172.0), "Terminal", [
            "SALLOW WORKS - VALVE HOUSE\nVALVE LOG\n\nDRAW-OFF 1   SHUT    14 OCT\nDRAW-OFF 2   SHUT    14 OCT\nSUPPLY       NO DEMAND\n\nSCOUR 1      OPEN     2 DEC\nSCOUR 2      OPEN     2 DEC\nOPERATOR     NOT RECOGNISED",
            "LEVEL ABOVE BED (M)\n\n 1 DEC   31.4\n15 DEC   16.2\n29 DEC    4.1\n 5 JAN    0.6\n\nUPPER ARM   NO CHANGE\nCAUSEWAY    HOLDING",
        ], "Valve log"),
        ("pump_station", (100.0, 210.0), "Terminal", [
            "SALLOW FILTER HOUSE\nDISTRICT SUPPLY (M3/DAY)\n\n         13 OCT  15 OCT  17 OCT\nMARROW    1840     210       0\nKETTLE    2210    1960       0\nCINDER    3930    3880       0",
            "CINDER WORKS\n\nDEMAND FROM    3 NOV\nFOR   FURNACE COOLING\nCEASED        26 FEB\n\nNO OTHER DEMAND.",
        ], "Supply terminal"),
        ("pump_station", (245.0, -70.0), "Note", """CATCHWATER GAUGE - read Sundays

Sun. 0.42. Sun. 0.40. Sun. 0.41.

Sun. 0.12. They've cut it at the top. They want the bed dry and it's
dry.

Sun. 0. Legs bad. Not coming up the hill again. A.P.""", "Gauge book"),
        ("cabin_log", (-160.0, 112.0), "Note", """SALLOW ANGLING CLUB

Season closed until further notice, by order of the Quarantine
Authority.

(in pencil) Pike in the upper arm are the size of dogs now. Something's
feeding them. Don't trail your hand.""", "Club notice"),
        ("mast_hut", (-122.0, -112.0), "Note", """CAUSEWAY TOLL

Foot - free. Cart - 2. Motor - 5.
New Sallow residents free (show card).

CLOSED 14 OCT.

(chalk) Culverts under the road are silted shut. The bed goes down,
the arm comes up. Lower street's gone.""", "Toll board"),
        ("sorting_gantry", (100.0, -82.0), "Terminal", [
            "OVERFLOW STORE - SALLOW\nUNDER    DEPOT MARROW\nSTATUS   PENDING REVIEW\n\nBUSES              14\nBICYCLES          212\nREFUSE BINS       380\nPRAMS              31\n\nREVIEW DATE   (NOT SET)",
            "BUSES - DEPOT MARROW\n\nNO. 1-3     KEPT\nNO. 4       NOT RECOVERED\nNO. 5-9     KEPT",
            "BUSES - OTHER DEPOTS\n\nKETTLE         3   KEPT\nCINDER         3   KEPT\n\nSEARCH CONTINUES:\nMARROW NO. 4",
        ], "Stock terminal"),
        ("draw_off_tower", (-20.0, 102.5), "Note", """TOWER BOOK

14 Oct. Both draw-offs shut, by order. Nobody downstream to drink it.

2 Dec. The scours are opening by themselves. The wheels won't turn
back. I've cut the chain off the big one and left it here.

Somebody may want it shut again one day. - A.P.""", "Tower book"),
        ("shop", (-180.0, -222.0), "Note", """We were moved up here for the water, and now we're moved for this.
Mrs Hlavac says she won't go twice. I've left her the shop key. Let her
take what she needs.

- J. Rybar, shop

(another hand) She went in the end, in the spring, on her own feet.
The shop's anyone's now. - A.P.""", "Shop note"),
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
        if not path.exists():
            print(f"{map_name}: no map yet, skipped")
            continue
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
