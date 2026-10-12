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
your remains lie where you fell). It is saved to
`<data dir>/extraction/<map>.ron` when a raid ends, every minute, and on
quit (`src/persist.rs`, format in `extraction_core::save`).

The **Zone clock** runs only while you are out, and carries over from
raid to raid. Every 60 minutes of Zone time — two or three raids — the
**Surge** comes: five minutes of warning (sky, radio), then two minutes
that kill anyone in the open and wear down anyone under a roof. The next
time you're back at the truck after it, the Zone resets: loose items and
remains are swept away and the loot is rolled afresh for the new cycle.
If the Surge is close when you're at base, pull the bell to sleep it out
in the cab instead of starting a raid you can't finish. Durations are
constants in `raid/mod.rs`, to tune.

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

### Artifacts and attention

Carried artifacts emit **attention**. The Zone director reads the total
attention near a player and spends it: anomalous hazards bloom along your
path, response teams get dispatched to your position, the dirigibles reroute.
Gunshots add short-lived attention too. Artifacts are the high-value loot
*because* they make the raid harder the moment you pick them up. Container
gadgets (a lead-lined case) can damp attention at a weight cost.

### Robots have jobs

The director doesn't spawn patrols. It posts **jobs**, and robots arrive
diegetically (out of the robot Gate at the map edge, dropped by dirigible, or
flown in) to do them, then leave:

- *Survey*: drones fly to a POI, scan it, fly out.
- *Logistics*: a dirigible ferries a cargo pod between robot structures;
  grappler bots anchor and reel it.
- *Construction*: walkers assemble a relay pylon at a site, operate it a while,
  pack it up again.
- *Response*: a team is sent to the last known position of a disturbance.
  They are *late* — it takes them time to get there — so noise has a cost
  you pay a few minutes later.
- *Collection*: the memorial convoy (below).

### Death and the memorial convoy

When you go down, a collection robot is dispatched from the Gate to recover
the body and drive it out to a government facility. The Zone clock keeps
running, so you can suit up again from the truck and race to intercept the
convoy before it leaves the map. Everything the convoy delivers is gone for
good and counted on a memorial board in the truck.

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

### Simulation

`extraction-sim` runs the game headless at many times real speed with bot
players and writes telemetry (paths taken, engagements, deaths, loot found,
time-to-extract) to `target/sim/`, with heatmaps drawn over the map preview.
That is the tool for "does this map play well?" questions.

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

1. **Foundation** — workspace, CI, rig + drivers, headless runner, screenshots.
2. **World and core loop** — map, terrain, fog and lighting, locomotion,
   grabbing, backpack and pockets, weight, raid lifecycle, truck.
3. **Weapons** — the four guns, ballistics, durability, maintenance, rails.
4. **Robots** — drone, orb walker, grappler, dirigible; parts damage, IK, jobs.
5. **Economy and systems** — sinks, artifacts and attention, gadgets, food,
   death convoy, the Surge.
6. **Polish** — art pass, recorded audio, simulation tooling, VR comfort.
7. *(stop)* — VRChat port and multiplayer are later phases.
