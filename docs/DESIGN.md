# Design

The living design document. The spec Cassidy wrote is the brief; this file
records what was decided, why, and what is still open. When code and this
file disagree, fix whichever one is wrong.

## Pillars

1. **Into the Radius' lightning in a bottle.** Slow, deliberate, tactile VR
   scavenging. Real-gun handling (manual mags, racking, maintenance), a
   backpack you physically reach into, fog you can't see through.
2. **The Zone feels busy, not scripted.** Robots arrive, do jobs and leave.
   They are not patrolling; they are *working*, and you are in the way.
3. **Nothing is bought.** Every item in the game was found, made from found
   things, or taken off a robot. Things also leave the world for good.
4. **No single currency.** Value is weight, bulk, attention, and usefulness —
   never a number on a price tag.
5. **Built to be simulated.** Anything a player does, a bot can do through
   the same interface, so maps can be play-tested by robots overnight.

## Decisions on the open questions in the brief

### Crossing the map to extract — yes, with the truck as the hinge

The truck drops you at one edge. To leave, you **radio the truck** and it
drives the perimeter road to a rendezvous point (RV) on the *other* side of
the map. Crossing is the raid; it is not tedium because:

- The map is ~650 m across, sized so a straight crossing takes ~6 minutes
  of VR walking and a scavenging crossing takes 30–45.
- The truck takes real time to get to the RV (it is a diegetic vehicle on a
  road), so you call it *before* you are done, which creates a planning beat
  and a "hold the RV" tension moment.
- A **short exfil** back at the insertion edge exists but costs fuel (a
  scavenged, consumable resource): the convenience is a resource sink.

### Map reset — the Surge

The Zone has a persistent state between raids and between sessions
(looted containers stay looted, the cache you hid stays where you hid it,
your remains lie where you fell). The whole career is one file,
`<data dir>/extraction/career.ron`: the stash, what you carry, your record
and the memorial are shared, and each valley keeps its own Zone. It's
saved when a raid ends, every minute, and on quit (`src/persist.rs`,
format in `extraction_core::save`); saves from before the career file
(one per map) are brought in automatically.

**Changing valleys**: a route card on the truck bed's right wall names
the next drive; press it to pick another valley. Ring the bell at base
with another valley on the card and the truck drives there through the
night: the career is saved, and the game opens again in the new valley.
The game always opens wherever the truck was headed (`--map` overrides).

**The logbook**, a card by the tailgate, writes up the last raid: how
long you were out, what you wrecked, what came home with you (artifacts
by name) or what the convoy took, under your record of raids, homecomings
and losses (`raid/logbook.rs`).

The **Zone clock** runs only while you are out, and carries over from
raid to raid. Every 60 minutes of Zone time — two or three raids — the
**Surge** comes: five minutes of warning (sky, radio), then two minutes
that kill anyone in the open and wear down anyone under a roof. The next
time you're back at the truck after it, the Zone resets: loose items and
remains are swept away and the loot is rolled afresh for the new cycle.
If the Surge is close when you're at base, pull the bell to sleep it out
in the cab instead of starting a raid you can't finish. Durations are
constants in `raid/mod.rs`, to tune.

Each cycle also brings its own **weather** (`world/weather.rs`, a pure
function of seed and cycle): drizzle (the reference look), a pea-souper,
a downpour, or a clearing spell. Weather is gameplay: visibility caps
how far robots see, and rain covers footsteps. It drives the sky, distance
and ground fog, the sun, GPU-animated rain streaks and the rain bed.

A new game starts with an empty stash except for a starter kit (a
pistol, a spare magazine, a box of rounds, the radio, a light, a
dressing, a tin of stew and a cleaning kit). `--new-game` starts over;
`--save PATH` saves elsewhere. Scenarios, screenshots and test scenes
never save.

### No cash, and the resource sinks

There is no shop and no currency. Things enter the world as loot, robot
parts, and artifacts. They leave the world through sinks:

| Sink | What it eats |
| - | - |
| **Maintenance** | Gun oil, cleaning patches, spare parts; guns degrade with use |
| **Fabrication** | The truck's bench disassembles robot parts/ammo into materials and re-packs rounds; every recipe loses material |
| **Fuel** | Optional truck services (short exfil, choosing your insertion point) burn scavenged fuel |
| **Offerings** | Weird interactables in the Zone accept items for boons, information, or calm |
| **Memorial convoy** | Your body and everything on it are hauled off-site unless you intercept the convoy |
| **Decay** | Batteries drain, food spoils (cosmetically), artifacts eventually go inert |
| **Truck space** | The stash is the truck's physical shelving; it is finite, so hoarding costs |

### Inventory: weight, bulk, and two big guns

- **Weight** (kg) is the slow constraint: carry mass drives encumbrance —
  walk speed, stamina, how loud you are, how fast your hands settle.
- **Bulk** is the backpack constraint: the backpack is a physical volume you
  reach into, with no collision inside it (as in ITR). It accepts items until
  their summed bulk hits its capacity. Big items are awkward to fish out
  because they crowd the volume.
- **Pockets** are fixed-size slots on a chest rig and belt: mag pouches, a
  pistol holster, small-item pockets. A slot accepts an item by *size class*.
- **Bulky weapons** (rifles, shotguns) can't go in the backpack at all. Two
  shoulder slings carry them, which caps you at two long guns.
- **Attention** is the artifact cost (below).

### Anomalies, artifacts and attention

**Anomalies** are laid out fresh each Surge cycle (a pure function of map,
seed and cycle, in `extraction_core::anomaly`): five to eight in each
artifact field, a few loose elsewhere, never on roads, water or in
buildings. They affect anything physical, so the classic test is to throw
something in first:

| Anomaly | What it does | Grows |
| - | - | - |
| **Lift** | Loose things float and circle; the longer something's in it, the sooner it pops and flings everything out. You get light on your feet, then thrown. | Featherstone |
| **Burner** | Heat with no fire: burns whoever stands in it, cooks robot parts. Glows orange through the fog. | Hearth |
| **Snare** | A knot of static: discharges into whatever touches it (robots too, through the same hit path as bullets), then recharges for a few seconds. | Aegis knot |

About a third of field anomalies hold an artifact at their heart each
cycle (rarely a glass eye instead). Artifacts are items: they persist in
saves like anything else and reset with the Surge.

**Artifacts** do odd, useful things while carried (`anomaly::artifacts`):
the featherstone takes 6 kg off your load (loose, it hovers at chest
height); the aegis knot turns the first hit and needs 90 s to re-knot; the
glass eye, held to your eye, shows robots through walls; the hearth will
power gadgets (with the gadget pass).

**Attention** is what they cost. Each carried artifact emits attention
per second (data: `attention` in `items.ron`), and your gunfire adds a
small share that fades in half a minute (gunfire already gets a team
sent to where it was heard; when noise counted for more, every fight
bought three more and the sims showed nobody surviving the spiral). The
director turns attention into budget and spends it: every 90
attention-seconds buys a response team sent to where you *are* (not
where you were heard), and while you carry artifacts, anomalies
**bloom** across your path ahead of you (temporary, at most three).
There is no number on screen: you feel it. A ringing rises in your ears
past the first prickle (synthesised, `anomaly/attention`), and each step
worse is said once on your wrist ("your teeth ache", "the air hums
around you", "the whole Zone is looking at you"); the flat HUD names the
feeling. A **lead-lined case** (2.4 kg, found among gadgets) seals one
artifact away: sealed, it draws no attention and does nothing for you —
no aegis, no hearth, no lift. Carry it shut and open it when it counts.

### Robots have jobs

The director doesn't spawn patrols. It posts **jobs**, and robots arrive
diegetically (out of the robot Gate at the map edge, dropped by dirigible, or
flown in) to do them, then leave:

- *Survey*: drones fly to a POI, scan it, fly out.
- *Logistics* (built): every six to ten minutes a dirigible comes in over
  the map edge, crosses to a work site and lowers a cargo pod full of
  robot stock on a cable, while two grapplers walk out from the Gate to
  stand guard. A few minutes later the pod is reeled back up — with
  whatever you didn't take — and the airship leaves. The dirigible is
  unarmed but sees a long way and reports what it sees (a response team
  follows); puncture two of its three gas cells and it sinks to a wreck
  full of salvage, dropping the pod where it hangs. Grapplers don't shoot:
  they spear you with a tether that slows you and drags you toward them.
- *Construction* (built): a pylon kit appears at a work site and two
  walkers walk out from the Gate to it. They put up a relay pylon (a mast
  climbs out of the cabinet over forty seconds, a sensor dish on top), run
  it a few minutes and pack it up again. While it runs it's a sentry: it
  sees 85 m all round from the top of its mast, and what it sees it
  reports (a response team follows). Shoot the dish and it's blind; wreck
  the cabinet and it spills salvage (electronics, cells, a sensor) — with
  its builders standing next to it. `--gallery pylon` shows one.
- *Response*: a team is sent to the last known position of a disturbance.
  They are *late* — it takes them time to get there — so noise has a cost
  you pay a few minutes later.
- *Collection*: the memorial convoy (below).

**Robots hear things land.** A loose item that hits something makes a
noise that carries with how hard and how heavy (metal rings, cloth
thuds): drop a pan beside a drone and it knows; throw a tin can down the
street and a walker goes to look where it landed, not where you are
(`items/impacts.rs`). Only gunfire and what the dirigible and pylons see
get a response team sent; a clatter just gets looked at.

**The radio listens.** Whenever a response team is sent — gunfire heard,
the airship or a pylon reporting you, the Zone's attention coming due —
the robots talk about it, and a radio you carry picks it up: a squelch
and a burst of static with something mechanical chirping through it,
and a line on your wrist saying whether it's about somewhere close by or
which way off it is (`raid/chatter.rs`). It's your warning to move.

**Drones warn before they fire.** Each time a drone gets its gun on you
it settles first: three rising chirps and a red flare under its nose,
0.9 s, then the burst (`robots/drone.rs`). Duck out of sight for a moment
and it keeps the lock; stay out for longer and it has to settle again.

**Buildings are not a refuge.** A robot hunting someone indoors goes to
the building's doors (`robots/doors.rs`). A drone hovers low outside a
doorway that stands open and looks in (it hangs back from a shut one,
waiting); a walker walks up to a shut door and opens it. A **door wedge**
set down against the foot of a closed door jams it — for you too, until
you pick the wedge up — and a walker that finds one batters the door for
twelve loud seconds before tearing it off its hinges.

### Death and the memorial convoy

When you go down, a collection robot is dispatched from the Gate to recover
the body and drive it out to a government facility. The Zone clock keeps
running, so you can suit up again from the truck and race to intercept the
convoy before it leaves the map. Everything the convoy delivers is gone for
good and counted on a memorial board in the truck.

### Offerings

The well in the village and the stone ring on the eastern hill take what
you leave in them (`anomaly/offering.rs`). Leave it, step back, and it's
gone; each place answers once a raid. Food buys **calm** (the director's
attention account is wiped and pending response teams are called off),
curios buy **information** (a voice gives the distance, bearing and place
of the nearest artifact this cycle), and an artifact buys **time** (the
Surge holds off ten minutes). Anything else is simply taken. Curios exist
for this: it's what an old wristwatch is *for* in a world with no shops.

### Cooking

A camp kitchen stands beside the truck at base (`raid/kitchen.rs`), built
on the cooking graph lifted from the prototypes: a board, a bowl, a pot, a
pan, a tin oven and a crock. Food items carry their graph food in a
`food:` tag; every food the graph can make exists as an item. Purely for
the ritual of it: there is no hunger.

### Weapons: real guns first

Recognisable, historically-documented service weapons first, because the
handling is the fun and friends watching can talk about them. The
abstractions (receiver, feed, chamber, durability) are written so a later
non-firearm pass replaces models and numbers, not systems. Starting set:

- A Makarov-pattern pistol (8-round single stack, slide + safety + decocker)
- An AK-pattern rifle (30-round mag, side charging handle, safety lever, rails via a side mount)
- A Mosin-pattern bolt rifle (5-round internal mag, stripper clips)
- A double-barrel break-action shotgun (two shells, ejectors)

Robot weapons can be carried home but are awkward to hold and their ammo
must be broken down at the bench and re-packed into compatible rounds.

### Gadgets and attachments

Things go where they physically fit (`gadgets/mod.rs`): the AK and the
Mosin have a receiver dovetail that takes the 3.5x scope or a side-mount
rail; the rail takes a weapon light or a laser; lights and lasers each
take a power cell; every gun but the double has a muzzle for the artifact
adapter, whose cradle takes any artifact. Bring a part to its mount and
let go, as with magazines. Use on a gun switches everything on it.

- **Power**: cells drain while a device is on (a light is 6 W, so a cell
  is about an hour and a half of torch); a carried hearth powers them for
  nothing. Cells come from decanting robot cells at the bench.
- **Optics**: the scope moves the sight line to its eyepiece and
  magnifies when your eye is on it: in the headset, a picture-in-picture
  lens (with a PU reticle in the scope); flat mode zooms the camera.
- **Artifact adapter**: featherstone rounds fly flat, hearth rounds hit
  half again as hard, aegis rounds knock robots senseless for two
  seconds, glass-eye rounds bend toward robots.
- **EMP**: arm a grenade (3.5 s fuse) or a trap (bites the first robot
  within 4 m once it's set down). Robots within 9 m lose senses, rotors
  and legs for 12 s: drones drop, walkers sag.
- **Compass and map case**: the needle finds north (-Z); the map is a
  hand-drawn sheet generated from the map file (`scripts/field-map.sh`).

- **Cloak and cover**: a cloak generator, switched on and set down,
  hides everyone within 3 m from robot eyes (15 W: a cell runs it for
  about forty minutes); deployable cover unfolds into a wall where you
  stand and folds back up with a grab.
- **Camera drone**: use it to spin it up and let go. It climbs to about
  22 m and circles where it left your hand; every robot it has a clear
  line to (90 m, less in fog and rain) is *spotted* — drawn through
  walls, as the glass eye draws them, for 40 s after it last saw them.
  Its feed shows on a screen above your left wrist. Use it again to
  call it home, or let the battery do it: 90 s of flight, then it comes
  back, drops at your feet, and recharges itself over a few minutes.
- **Lead-lined case**: see artifacts, above.
- **Climbing kit**: use it to shake out the line, then throw the hook up
  onto a roof. If it lands at least 2.2 m up it bites, and the rope drops
  from the nearest edge on your side. Grab the rope and pull down to go
  up, hand over hand (or push the stick); at the top you haul yourself
  over. Let go and you fall. Pick the hook back up to coil the line.

- **Zipline kit**: stand somewhere high, look where you want to go and
  use it. The cable shoots out and, if it bites 8 to 60 m off and
  downhill (no steeper than 45°), runs there from a post at your feet.
  Grab it and you ride, gathering speed to 9 m/s, and drop off the end.
  Pick the kit up off its post to reel the cable in.
- **Door wedge**: see robots and doors, above.

### IFF, PvP, and networking

Out of scope until the single-player game is excellent. The architecture
keeps the door open: all gameplay state is in ECS components that are
separate from visuals, players are driven by an input struct (not the
keyboard or a headset directly), and the simulation runs headless.

## Architecture

### Crates

- `crates/extraction_core` — no Bevy. Rules and data: items, inventory
  arithmetic, firearm mechanisms as state machines, the cooking graph (lifted
  whole from `game-prototypes`), the economy. Unit tests run in milliseconds.
- `extraction` (root) — the Bevy game as a library of plugins, plus the
  client binary and the headless simulation binary.

### The rig

Every player — VR, flat, or bot — is a **rig**: a head and two hands. Each
frame something writes a `RigInput` (target head/hand poses, grip, trigger,
buttons, locomotion stick). Everything downstream — grabbing, guns, the
backpack, locomotion — reads only the rig. Three drivers write it:

- **XR** (`--features xr`, the default): OpenXR via `bevy_mod_openxr`.
- **Flat**: mouse and keyboard, for development and for people without a
  headset. Hands hover in front of the camera; scripted hand *macros*
  (reload, rack, swap) play the VR motions out so flat mode exercises the
  same interactions.
- **Bot**: the simulation's AI player, which uses the same macros.

This one seam buys testability, flat-mode play, map simulation, and later,
network replication (a rig input is the obvious thing to send).

### Logic vs. visuals

Gameplay entities are spawned with gameplay components only (physics,
health, item state). Separate *visual* systems react to them and attach
meshes, materials, sounds and particles. The headless simulation simply does
not add the visual plugins. This is also the shape a dedicated server wants.

### Physics

[Avian](https://github.com/Jondolf/avian) 3D: ECS-native, deterministic
enough, joints for physical hands and robot limbs.

### Maps

Hand-crafted, authored as text (`assets/maps/*.ron`): terrain features
(hills, valleys, the creek bed, road flattening), roads as splines,
placements of building and prop prefabs, loot spots, robot sites, RV points.
The game builds terrain, roads and vegetation from it at load time, and a
top-down preview renderer draws the layout for design review. Prefabs are
modelled by Blender Python scripts in `art/`, exported as glTF with
collision and socket markers as custom properties. Enemy spawns, jobs and
specific loot roll from pools; the places themselves are designed.
Two maps so far. **Marrow Creek**: a river valley, the creek its spine,
three ways across it and every rendezvous on the far side. **Kettle
Pass**: a valley that climbs 30 m from the south gate to a saddle
between two summits; the pass road switchbacks up the middle in plain
view, and cutting across means climbing. Its rendezvous are the pass
itself, the mine yard in the west and the hunting lodge on the east
shoulder; its artifacts grow in the quarry pit and on the mine's spoil
heap. Each map keeps its own Zone; the stash and kit travel with you
(see "Changing valleys" above).

Risk pays: each region's loot spots roll `0.8 + 0.2 × danger` times
(fractions are a chance of one more), unless the map sets `loot` for the
region. The village by the insertion point is set to 0.6 — picked over —
after the sims showed bots filling their packs there and never going
further.

### Simulation

`extraction --sim N` (or `scripts/sim.sh <map> N`) plays N raids headless,
each in its own process, at several times real speed. The player is a bot
with a **raider** brain (`src/sim`) on top of the scenario bot's task list:
it kits up, picks loot sites by personality (looter, artifact hunter,
cautious, reckless), walks there on the robots' nav grid (and, indoors, a
fine grid per building, opening doors), takes what's worth taking, fights
robots that come for it, routes round anomalies (or doesn't), radios the
truck, walks to the rendezvous and pulls the bell — all through `RigInput`
and `HandAction`s, like a player. Each run writes JSON-lines telemetry
(position every second, loot, shots and hits, fights, damage by cause,
anomalies, death, the outcome) to `target/sim/<map>/`;
`scripts/sim_report.py` draws traffic, death, engagement, loot and damage
heatmaps over the map preview and writes a summary (extraction rate, raid
times, causes of death, most-looted and never-visited places). That is the
tool for "does this map play well?" questions. Details and the telemetry
format: `docs/SIMULATION.md`.

## Art direction

Grounded, gloomy temperate: wet spruce and birch forest, overcast sky, low
sun that never quite breaks through, rain-darkened concrete and rusted steel,
and fog in layers — distance fog, ground-hugging height fog, and volumetric
light shafts for flashlights. PBR throughout: CC0 scanned materials from
Poly Haven and ambientCG, fetched by `scripts/fetch_assets.py` (credited in
`CREDITS.md`), and custom shaders where scans don't cut it (wetness, moss
blending on terrain, fog). Robots are the exception to grime: sleek, pale,
panelled sci-fi forms with a few emissive accents, so they read instantly
against the murk. Procedural animation everywhere it can go: robot legs by
IK, rotors and tilt by physics, hands by springs.

## Audio direction

Recorded, mastered sounds for the physical world — footsteps per surface,
impacts, gun mechanisms, ambience beds — from CC0 libraries, processed by a
mastering script (trim, normalise loudness, fade, variation sets).
Synthesis is reserved for things that don't exist (anomalies, robot voices).
Spatialised via `bevy_seedling`.

## Lints

Clippy runs at its default level and CI does not fail on warnings. Bevy
style trips `too_many_arguments` and `type_complexity`, so those are off.
This is deliberate (see the brief): strict lint regimes cost more than they
catch for agent-written code. Revisit if that research changes.

## Roadmap

1. **Foundation** — done: workspace, CI, rig + drivers, headless runner,
   screenshots, client smoke test.
2. **World and core loop** — done: map, terrain, layered fog and weather,
   locomotion, grabbing, backpack and pockets, weight, raid lifecycle,
   truck, saves.
3. **Weapons** — done: the four guns (modelled), ballistics, durability,
   maintenance, attachments and optics.
4. **Robots** — done: drone, walker, grappler, dirigible, collector; parts
   damage, IK, jobs (survey, watch, response, logistics, collection).
5. **Economy and systems** — done: sinks (bench, offerings, convoy, fuel,
   decay of cells), artifacts and attention, anomalies, gadgets, cooking,
   the Surge.
6. **Polish** — in progress. Done: models for the truck, airship, pod,
   harpoon launcher, gadgets and artifacts; map simulation with telemetry
   and heatmaps (see `SIMULATION.md`); forest and building batching for VR;
   the lead-lined case, the camera drone, the climbing and zipline kits.
   A second map, Kettle Pass. Loot that rewards risk, after the sims
   showed the far side of Marrow Creek going unvisited (it no longer
   does). Gunfire no longer snowballs into endless response teams (the
   sims went from 30% to 60% extraction, hunters from none of 11 to
   about half); the radio overhears responses, and attention is felt.
   Still open: drones still decide most deaths; playtesting in a
   headset.
7. *(stop)* — VRChat port and multiplayer are later phases. The port's
   plan, and what of this game survives it, is in `VRCHAT_PORT.md`; the
   map already exports to Unity (`unity/README.md`).
