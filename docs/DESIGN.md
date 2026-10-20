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

**The logbook**, a card by the tailgate (next to the driver's request
card, see the sinks), writes up the last raid: how
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
| **The driver's requests** | A card by the tailgate asks for a few of one thing (robot eyes, scrap, stew); hand them over from the stash and they're gone |

The driver pays for a request the only way that keeps "nothing is bought"
true: by telling you where they hid a cache of their own, a pile of the
good stuff (ammunition, gadgets, medicine, now and then a gun) left at a
building well out in the Zone — "in the log cabin by the Hunting lodge."
It's pencilled on your field map as a red cross. You still have to go
and get it, and get it home; the Surge sweeps up a
cache nobody fetched. The requests are **jobs in tiers**: fetch a few
of something, recover a recorder from a building, or go and look at a
place, each tier sending you somewhere worse, and each tier done gets
something fitted to the truck (a rack for the stash, a vice for the bench,
an aerial, a medicine cabinet). One job a raid; press the logbook for the
career page (`raid/requests.rs`, `progression/`, `docs/PROGRESSION.md`).

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

**Robots sense like animals, not cameras** (`robots/senses.rs`, every
tunable named and documented there). Sight is an *exposure*: range, fog
(ground fog thickest of all), light, your posture (`Body::height`, so a
real crouch counts), the grass you're crouched in and whether you're
moving all multiply. Over a threshold it has you and can fire; under it, a
glimpse only makes it curious, never enough to shoot; crouched still in
grass in a fog, thirty metres off, you're nothing at all. Hearing: shots
carry by calibre (a pistol 150 m, the AK 220, a Mosin 250), footsteps by
what's underfoot (gravel and boards loud, grass soft), how fast and how
low you go, doors when you open them, and things that land; a wall or a
hill between cuts a sound's reach by more than half. A heard sound tells
a robot roughly where the *sound* was: a guess a fifth of the distance
out, never on the spot, so it comes to look nearby, not at you. A glimpse
of you moving is a guess the same way; a glimpse of you keeping still
places you nowhere new. Crouched still in grass, a robot makes you out
only within 4 m: shoot once from the grass at 40 m and keep still, and
the one that comes to look finds you about half the time.

**Light and dark.** Robot eyes follow the same light the player sees
(`Conditions::light_at`, Visual design's time of day): exposure scales by
its square root, so dusk leaves about 0.6 of the day's, a room by day
0.55, a full-moon night 0.28 and a black one 0.14. A torch lights you up wherever the sky doesn't, and its
beam pointed at a robot in the dark is seen from 45 m. A **suppressor**
carries a shot a third as far. Robots hear round open doorways and
through walls by the same rule the player's ears do (`acoustics`).

**Hounds hunt by ear** (`robots/hound_mind.rs`). Poor eyes tuned to
movement, the best ears in the Zone: a sound sends a hound running, the
last stretch it stalks low, then noses round. Seen, it comes round to
your side, crouches (the tell) and lunges to bite. The way to beat one is
the opposite of a drone: keep still and quiet. The director answers
gunfire with a pair of hounds a third of the time.

**Robots remember and search** (`robots/search.rs`). Each keeps a
last-known position and an uncertainty that grows at walking pace while
you're out of sight (it knows which way you went for a second, then it's
guessing). Suspicious, it **investigates**: walks over (a walker
carefully, a grappler creeping low) and looks round. Then it
**searches**: the doorways (it opens a shut door and looks in) and the
ground hard by walls and cover within its uncertainty (a drone comes down to a doorway and hangs outside it, looking in), a pause and a look
round at each. Nothing, and it **gives up**: back to its job, cut short,
or home if finding you was the job. Lose a robot mid-fight and it does
the same from where it last saw you. A robot that notices you calls the
others within 45 m to come and look (they don't open fire until they see
you themselves), and a radio you carry hears the chatter. Each step is a
message (`Noticed`, `Investigating`, `Searching`, `LostYou`, `CalledOut`)
for sound and the wrist.

**Every attack has a tell.** Drones chirp and settle 0.9 s; walkers'
turrets settle 0.7 s (`WalkerAim`); grapplers rear back 0.6 s before a
throw (`HarpoonWindup`). Tactics differ by kind: drones stand off at 20 m
and 8 m up, slide round to your flank when you aim at them and climb to
look over what you're behind; walkers move between pieces of cover in
range and push through doors; grapplers lie low and wait for you to come
within reach, then rush.

**Crawlers keep the dark** (`robots/indoor.rs`). Underground sites have
residents: small walkers that walk a round of the rooms on the site's own
grid (`LocalNav`), pausing to look round at each, and hunt with the same
senses and brain as any walker, with an 18 m spark gun that crackles 0.5 s
before each zap (`CrawlerCharge`). One that hears you close by keeps still
and low in the dark for a few seconds and lets you come to it. Below ground it's dark
(`senses::Lighting`): unlit, you're a tenth as visible, a shape at arm's
length. A lamp lights you near it; a torch lights you up, and its beam
pointed at a crawler is seen from as far as it carries. Sound follows the
corridors: round a corner it carries as far as the walk there, not cut as
if through rock. Sites register their grids, lamps, routes and crawler
counts (`indoor::IndoorSites`, `senses::Lighting`); the director puts the
crawlers on their rounds once.

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
the body and take it back through the Gate to the Custodian's depot, "pending
review" (`docs/LORE.md`). The Zone clock keeps
running, so you can suit up again from the truck and race to intercept the
convoy before it leaves the map. Everything the convoy delivers is gone for
good and counted on a memorial board in the truck.

### The camp, the yard, and words in the world

No menus, still. The title screen is a **camp** behind the parked truck:
a folding table, a lantern, a board with the game's name, your career,
and cards you press (training yard, new career, comfort, pack up). You
start a raid by walking out through the gate. Someone new starts in the
**training yard** instead (`assets/maps/training_yard.ron`), a fenced
practice valley where signs walk you through the kit; a card skips it,
the truck's cord finishes it. Nothing there is kept.

Text lives on things:
- signs on posts, and notes on tables and floors (`MarkerKind::Note`,
  `Sign`, placed by `scripts/place_notes.py` on the buildings' tables);
- terminals that page when pressed;
- the radio's own LCD, the wrist, the truck's cards.

The flat-screen HUD keeps only a crosshair, what you're looking at, and
the radio. Hold H to check yourself; F1 lists the keys.

### The capstone: recalling the machines

The terminals tell it. The machines are the Quarantine Authority's
**Custodian**, still sweeping the valleys for a review that never came,
and each Gate is one of its depots. Three **valley keys** recall them,
one per valley at its worst place, set together in the lock beside any
Gate. Carrying all three at once is the risk: the stash holds them
between raids, and going down loses them to the collector. After the
recall no machines come, in any valley; the Zone stays, the Surge too,
quiet to walk. It's an ending, not a stop: the career goes on.

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

### Weapon feel

How a gun handles is the game's main verb, so it is physical and fair,
and it moves the gun, never the head (`src/weapons/feel.rs`).

- **Recoil** is a kick of momentum per shot (`recoil` in `items.ron`,
  per gun and cartridge) shared with the mass behind the gun: muzzle
  rise about the grip, a little random yaw, a push back along the bore.
  The hands are a damped spring that brings it back. One hand is a
  soft spring with a wrist behind it; two hands stiffer with both arms;
  a stock within 30 cm of the head (shoulder pocket or cheek weld)
  stiffest, with the body behind it. Fast fire from one hand stacks up.
  Ten rounds of automatic from a still AK group at about 5° (RMS) in one
  hand, 1° in two, 0.3° shouldered (a test holds this ordering).
- **Weight**: a held gun follows the controller with a lag of
  `mass x LAG_*` (an AK: 40 ms in one hand, 17 ms in two, 13 ms
  shouldered; a pistol under 10 ms), smoothed in the player's space
  so walking and riding never trail it.
- **Strain**: a long gun held out in one hand droops (`DROOP` per kg m
  of moment about the wrist: an AK about 4°, a Mosin 7°) and sways
  slowly (`SWAY_*`, under 1°). Sprint fatigue adds up to `SWAY_TIRED`
  (0.35°) to any hold, shared by a second hand and a shoulder.
- **Bracing**: a gun resting on something solid (its handguard, body
  or magazine within 5 cm of a wall top, a sill, a rock) loses 85% of
  its sway and its droop, takes recoil on a stiffer spring with 8 kg of
  rest behind it, and follows with half the lag; a click in the hand
  says it took. Crouching (or a head under 1.2 m) is worth 40% less sway
  and a slightly quicker recovery, on top.
- **Malfunctions** come from wear and dirt (one in a thousand on a
  clean gun in good order, about one in twenty-five at half condition)
  and each has its own cure. A *stovepipe* (a case standing in the
  port, drawn on the gun): rack. A *failure to feed* (the magazine
  slipped; the trigger clicks): tap the magazine home with a palm, then
  rack. A *double feed* (a case and a round wedged together, drawn in the
  port): the bolt won't close or clear until the magazine is out, so
  drop it, rack, reload. The bench's repair and a cleaning kit make
  them rare again. Flat mode's rack key tap-racks, or strips, racks and
  reloads on a double feed.

### What a round does, grenades, blades

- **Through and off things** (`extraction_core::firearm_terminal`): a
  round's energy (its bullet's mass by cartridge, its speed now) against
  each material's cost per millimetre, thin things at their real
  thickness, a longer path at a slant. Pistols and buckshot stop in a
  plank wall; rifles go through planks, plaster and tin; only the Mosin
  through a log wall; brick, concrete, rock, earth and robot plate stop
  everything. Grazing a hard face (under about 20 degrees) mostly turns
  the round, keeping 60% of its speed. The range test
  (`weapons/range.rs`) fires every gun at every material to hold it.
- **Grenades** (RGD-5, F-1): the other hand's trigger pulls the ring; the
  gripping hand holds the spoon, safe while it holds; let go (or B to
  cook) and the spoon flies, 3.6-3.8 s. A pinned grenade never goes off.
  Fragments are real rounds (cover stops them, concrete turns them), the
  blast hurts and shoves what it can see, falling off within 5 m; robots
  hear it 160 m off.
- **Blades**: a knife, the Mosin's fixed bayonet and the SKS's folding
  one (A). Harm from the point's speed along the blade or across the
  edge, nothing below about 1.5 m/s; eyes take double, joints half again,
  a robot's core and plate a fraction.
- **Armour** (`weapons/armour.rs`): a plate carrier goes on when you let
  it go at your chest, a helmet at your head, and a steel plate in either
  of the vest's pockets (the pocket decides front or back). A round that
  lands where a plate or the helmet covers goes through `armour_strike`:
  a stop leaves a bruise (5% of the damage) and wears the plate, a
  ricochet does nothing, and a round that goes through does harm in
  proportion to the speed it keeps. A 6 mm plate stops pistol rounds and buckshot. A
  new one dents under a rifle round, and a worn one lets it through.
  Blasts and blades go round plates.
- **Loads** (`extraction_core::ammo`): rifle calibres come as ball, AP
  and tracer, and 12 gauge as buckshot or slug. Each round keeps its load
  from the box through the magazine and the chamber to the shot. Loading
  by hand is how you choose: touch rounds to a magazine and they go on
  top, last in first out. A box can come packed with every fourth round a
  tracer, and that order carries into the magazine.
  - AP: half the cost through hard plate (armour steel, sheet, a helmet),
    so it goes through a worn plate where ball stops. It does 1.6 times
    ball's harm to a robot's armour plate and 0.85 to soft targets.
  - Tracers (`weapons/tracers.rs`): a red-orange streak from 8 m out to
    800 m, a pixel or two wide however far off, and fog doesn't hide it.
    At most 32 are drawn.
  - A slug: one 32 g ball with 0.8 of the buckshot's summed harm, and a
    rifle's group.

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
- **Zeroing** (`weapons/zero.rs`): the PSO-1 (100 to 1000 m) and the PU
  (to 1300 m) have an elevation drum. Take it with the other hand's
  trigger and turn your wrist, clockwise from above for more range, one
  click per 100 m. The zero comes from the game's own ballistics (the
  gun's muzzle velocity, the scope's height over the bore), so at the set
  range the round goes where the reticle is. Nearer it strikes high,
  farther low: under a 300 m zero, a robot at 400 m needs holdover (the
  PSO's lower chevrons are a thousandth apart). The PSO's rangefinder
  reads range off a 1.7 m man between its baseline and curve.
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
**Water** is in the way, not scenery: wading drags at you (down to 40%
of your pace at chest depth, and no sprinting), and deeper than that you
float with your head out and paddle slowly, kit and all — no jumping
out of it (`player.rs`). A creek or canal is a crossing to plan.
Three maps so far. **Marrow Creek**: a river valley, the creek its spine,
three ways across it and every rendezvous on the far side. **Kettle
Pass**: a valley that climbs 30 m from the south gate to a saddle
between two summits; the pass road switchbacks up the middle in plain
view, and cutting across means climbing. Its rendezvous are the pass
itself, the mine yard in the west and the hunting lodge on the east
shoulder; its artifacts grow in the quarry pit and on the mine's spoil
heap. **Cinder Works**: a flat, burnt industrial valley cut in two by a
canal with one bridge. The works sit in the middle under a 38 m chimney
you can steer by when the fog allows; the north-west burned and stands
dead (long sightlines, nothing low to hide behind), and the artifacts are
on the slag heaps in the north-east, under the robots' Gate, and in the
ash pit to the west. The air is its own: a warm ash-brown haze over the
sky and fog, and ash still drifting down when it isn't raining (maps
can set `haze` and `ash`), and it sounds of wind through scaffolding,
metal groaning in the halls, jackdaws. Kettle Pass has gusts over bare
rock, ravens and scree letting go up the slopes; maps pick these with
`sounds`. Its rendezvous are the fire lookout in the dead
wood, the north sidings and the works yard at the railway's east end.
Each map keeps its own Zone; the stash and kit travel with you
(see "Changing valleys" above).

**The ground** is more than the author drew. On top of the map's noise
and features every valley gets natural relief (`noise.relief`, a share
of the amplitude): knolls and hummocks in patches, with sharper crests
than hollows. Then it rains (`noise.erosion`): droplets run downhill,
picking up soil where they speed up and dropping it where they slow,
which cuts gullies down every slope and fans silt out at their feet.
Round buildings and along streams the land stays as laid out, so pads,
fords and bridges still meet it. **Rocks** (`extraction_core::rocks`)
gather on steep ground and in slow-noise rock fields, with the odd lone
erratic boulder to steer by and a sprinkling of field stones everywhere
else; the big ones block the way for you, bullets and the walkers' nav.
**Ground cover** (`extraction_core::cover`) is streamed round the
player in 16 m patches: grass tufts as real blades, thick in meadows and
thin between, sedge on wet ground, and ferns under the trees. Nothing
of it collides. Rocks and cover are deterministic per map, so the game,
the sims and the Unity export (`rocks.json`) see the same ones.

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
   harpoon launcher, gadgets and artifacts; map simulation with
   telemetry and heatmaps (see `SIMULATION.md`); forest and building
   batching for VR; the lead-lined case, the camera drone, the climbing
   and zipline kits. A second map, Kettle Pass, and a third, Cinder
   Works. Loot that rewards risk, after the sims showed the far side of
   Marrow Creek going unvisited (it no longer does). Gunfire no longer
   snowballs into endless response teams (the sims went from 30% to 60%
   extraction, hunters from none of 11 to about half); the radio
   overhears responses, and attention is felt. Drones chirp before they
   fire; the driver asks for things and pays in caches; a logbook writes
   up each raid. Still open: drones still decide most deaths;
   playtesting in a headset.
7. *(stop)* — VRChat port and multiplayer are later phases. The port's
   plan, and what of this game survives it, is in `VRCHAT_PORT.md`; the
   map already exports to Unity (`unity/README.md`).
