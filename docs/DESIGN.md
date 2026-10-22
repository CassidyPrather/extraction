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
get a response team sent; a clatter just gets looked at. Keep shooting
and they send more: each reported shot heats things up a point (at most
one per 8 s, cooling a point every two and a half minutes), and from
2.5 a response brings two walkers as well, who pin and flank.

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
out, never on the spot, so it comes to look nearby, not at you. A lure
works once: a robot that searched a spot and found nothing ignores sounds
nobody made there (a tape left playing, a flare) for two minutes. A glimpse
of you moving is a guess the same way; a glimpse of you keeping still
places you nowhere new. Crouched still in grass, a robot makes you out
only within 4 m: shoot once from the grass at 40 m and keep still, and
the one that comes to look finds you about half the time.

**Light and dark.** Robot eyes follow the same light the player sees
(`Conditions::light_at`, Visual design's time of day): exposure scales by
its square root, so dusk leaves about 0.6 of the day's, a room by day
0.55, a full-moon night 0.28 and a black one 0.14. A torch lights you up wherever the sky doesn't, and its
beam pointed at a robot in the dark is seen from 45 m. A shot is heard
by its report and its crack (see "The shot as a sound" below). Robots hear round open doorways and
through walls by the same rule the player's ears do (`acoustics`). A
scope pointed into a low sun flashes, and robots see the flash further
than they see a body: it places a still sniper roughly, where keeping
still would otherwise have kept them unplaced.

**Tracks in the snow** (`robots/tracks.rs`). A robot that can't see you
and comes within sight of a fresh print of yours (12 m by day, less as
light and weather close in) reads the trail on, about 15 m along while
it's fresh, and goes to look there; arrived, it reads on. It walks your
trail to you unless the snow has filled it in: fresh snow is the cover.
A plough's fresh lane takes no prints, but a trail that stops at its
edge says you walked it: the robot goes on along the lane the way you
were heading (`robots/signs.rs`).

**They read what you set down** (`robots/devices.rs`). Every device you place gives something off: light, a hum, power draw (a drone's or a relay's sensors pick up a hungry device, 3 m a watt), and how far it bites. That alone sets what the machines do about it, from one table:
- what bites (an armed trap) is shot from outside its reach;
- what's hungry (10 W or more: a jammer, a cloak, a camera drone) is hunted and shot to scrap;
- the rest (a light, a laser) is come and looked at.

A machine that can't shoot reports what it can't deal with. The tell: it chirps and throws a scan beam at the device, then turns and goes to it.

**Their network, turned against them** (`robots/network.rs`). Pylons, sentries, spotters and the airship report what they see, and a team follows. Every report goes through the director's network, so it can be worked on:
- **Cut a pylon's cable** with bolt cutters and it goes silent. The silence is the tell: after 45 s a mender is sent, and once it has welded the torn cabinet whole the pylon is back on the network.
- **Splice a salvaged radio** into a pylon or a spotter's winch (let go of it against the cabinet): it reports contact at a named place at least 150 m from you, and the team goes there. Do it twice to the same relay and they stop believing it: none of its reports go out, and a walker comes to look it over.
- **A powered jammer** blocks every relay's reports within its radius, but a relay gone quiet in a jam is a sign too: after 40 s a team comes to roughly where the jammer is.

**The Zone remembers you** (`robots/memory.rs`, `zone_memory`). Each valley keeps a compact memory of your habits, saved with it:
- the 30 m cells you walk;
- the rendezvous you get out from;
- where you wreck its machines;
- the loot spots you clear.

It fades each raid to 0.6 and forgets under 0.1, so changing your habits pays off within a few raids. When you next step out, it leans, gently and capped:
- patrols pick places on your usual ways up to 1.5 times as often;
- the sentry nearest your favourite rendezvous turns its sweep toward it, by at most a quarter of its arc;
- up to two buildings you cleared are found with their doors shut, and once clearing one is a habit, one door is wedged if there's another way in;
- where you've wrecked them more than once lately, one of their kind is left watching (an unarmed walker that calls the others, or a drone).

The driver remarks on the strongest of these as you set off, and a radio you carry says when you're picked up on your usual way. One knob (`zone_memory::MEMORY`) scales all of it; a fresh career is untouched, the director's random draws included.

**The disturbed world is evidence** (`robots/disturbed.rs`). What you leave changed tells a patrol someone was here:
- a door you opened;
- wire or a padlock cut;
- crowbar marks;
- broken glass or a smashed crate;
- a wreck dragged from where it fell;
- a loot spot left bare.

Each is a sign with an age, in one registry. A robot that passes within sight of a fresh one (under 3 minutes) goes to look; an older one only makes it wary. Two fresh signs in a line are a route: it goes on past the newer, the way they run.

**What you handle gives you away** (`robots/signs.rs`). A gun knocked on
a doorframe, or let go so that it slaps back on its sling, is heard (up
to 15 m for a hard knock). A dragged load scrapes, and robots hear it
twice a second. A car door or panel in your hands makes you more to see,
and while it swings you aren't keeping still, however still you stand.
A crate you break is gone for them too: where it stood is walkable again
and no longer cover.

**Hounds hunt by ear** (`robots/hound_mind.rs`). Poor eyes tuned to
movement, the best ears in the Zone: a sound sends a hound running, the
last stretch it stalks low, then noses round. Seen, it comes round to
your side, crouches (the tell) and lunges to bite; a pair splits up, one
round each side. The way to beat one is
the opposite of a drone: keep still and quiet. The director answers
gunfire with a pair of hounds a third of the time.

**Walkers pin and flank** (`robots/walker.rs`). A walker fights from
cover in range. Two on one player pair up: the first pins you, the other
says so (`Mind::Flanking`) and works round to your side, about 75° off
the first's line, so cover that faces one doesn't face both. Three or
more take sides by turns. A walker shot most of the way through its core
(under a third left) breaks off: it says so (`Mind::Retreating`) and falls
back about 14 m to cover away from you, still shooting. Press it.

**The Sorter rings its hall** (`robots/sorter_mind.rs`). The Depot's
gantry can't leave its rails. It turns its head to a sound; seeing you,
it bangs the hall once (loud enough to wake what sleeps in the bays) and
throws a tote while you're within 14 m, after a wind-up that's its tell:
it throws where you stood then, so step aside. Lose it and it watches
where you went, then gropes round there with its arm, then goes back to
its sorting.

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
others within 45 m to come and look (a drone, up high, spots for them:
its call carries 120 m), and they don't open fire until they see you
themselves; a radio you carry hears the chatter. Each step is a
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
twelve loud seconds before tearing it off its hinges. Nor are thin walls
cover: a walker that loses you behind a plank wall or door, close by and
knowing roughly where, settles its turret (the same tell as an aimed
shot) and fires three rounds through it, once, if a round would get
through (`destruction::through`; brick and steel stop them). Moving off
from where it lost you beats it.

### The heavy: you should not be fighting this

The Custodian's demolition unit (`robots/heavy.rs`, its mind
`heavy_mind.rs`, its rules `extraction_core::heavy`): a pale hull the size
of a van on four long legs, nearly four metres to its back, a sensor brow
of three red lenses and one manipulator arm. It walks a little slower
than you; it doesn't hide, take cover or shoot. It comes straight at you,
and when you're in reach its arm rises over its hull, hissing (1.2 s, the
tell), and comes down where you stood (70 at the middle, less at the
edge, and you feel the blow land; whatever it lands on splinters).
Hiding indoors only brings it to the door: it wrenches the door off and
reaches in, and every few seconds tears a doorway-sized hole in the wall
between it and you (Destruction's `Breach`: wood, plaster and tin tear;
brick and concrete hold). Out of sight it searches
like any robot, patiently, then gives up and walks home.

**When it comes.** Not on a timer. The director sends one when the whole
Zone has been looking at you (attention at its worst, "the whole Zone is
looking at you", held for 40 s, not in a raid's first six minutes), or at
once when someone stands within 60 m of a Gate carrying a valley key. One
at a time, fifteen minutes between one going and the next, at most two a
raid, none with the director off (the recall, the yard) or while a
night visit is on its way. A horn sounds
from the Gate, the radio hears it, and it walks out twenty seconds later
toward where you are. A pure step function, so pacing is a test
(`heavies_keep_their_distance_over_a_raid`: a quiet raid never meets one,
a hoarder meets two, a long while apart, a recall run meets one at the
Gate).

**Taking it apart.** There's no health bar. The hull stops everything;
what can be hurt is small and particular:

| Piece | Where | What it takes | What it does |
| - | - | - | - |
| Bolts | four on each plate, ringed amber | one ordinary round each, or a good butt stroke | on two, the plate hangs loose and a hand can tear it off; on none it drops. A crowbar at a plate's edge (two hands, 200 N·m) tears one out each time it gives |
| Plates | the hatch on its back, one down each flank | nothing | cover the cell and the vents: a covered weak point takes nothing, blasts and anomalies included |
| Knees | out in the open, lit cyan | ~5 rifle rounds, 3 from a Mosin | one gone and it limps; two and it's down for good |
| Vents | behind the flank plates | 120 | shed its heat: each lost, it runs hotter |
| Cell | behind the hatch, low on its back | 200 | shot through, it's dead and the cell with it; pulled out by hand, it's dead and the cell is yours |
| Arm, eye | out front | 300, 80 | no more blows; blind, it hunts by ear |

**Heat.** Walking and its blows heat it; its vents shed it. With both it
never overheats; with one it holds out a long fight; with none it stalls
within half a minute of chasing you. Past the mark it **stalls**: down on
its knees for ten seconds, and the hatch over its cell blows its bolts to
vent. Its strips and vents run from cyan to orange as it heats: a hot
heavy reads as one before it stalls.

**The prepared player's tools.** Standing, its cell is out of reach (2.6 m
up). It has to be brought down to you:
- an **EMP** drops it to its knees for 7 s (a walker gets 12);
- a **burner** heats it fast (a stall in about five seconds, vents or
  not) and cooks its bolts;
- a **snare**'s discharge knocks it down a moment and heats it;
- a **well** takes its feet: it's held, dragged toward the heart, and the
  heart crushes its knees until it falls. Robots don't path round
  anomalies, so lure it across one.
Then get behind it, shoot the hatch's bolts or wait for the stall to blow
them, and pull the cell. Or don't fight it: break its line of sight, keep
quiet, and let it give up.

**Its wreck stays.** It sinks onto its folded legs where it fell and stays
there, not broken up or counted against the debris budget: a landmark. Its
plates come loose in the fall (a hand can have them) and whole knees and
its claw drop beside it. Salvage that matters: the **heavy power cell**
(2400 Wh, twenty robot cells; only if pulled), **heavy armour plate**,
**heavy knee actuator**, **manipulator claw**.

**Felt and heard.** Each footfall is a `HeavyAct::Step` event (for its sound,
heard far) and a pulse in both hands within 25 m, a knock close by fading
to a tick; never a kick, which is for shots and hits. Every act it makes
(`HeavyAct`: the arm's rise and blow, a door torn off, a bolt, a plate
off, a vent out, a stall, a well's grip, the cell pulled, the fall) is an
event for audio, the wrist and the radio.

**Checks.** The rules in `extraction_core::heavy` (weak points, plates,
heat, the call); the body in headless tests (it walks, bolts drop plates
and open what's behind, two knees bring it down and it stays, EMP,
burner, well); a hand pulling its cell through the headset path
(`a_hand_pulls_the_cell`); the `heavy_well` scenario (a bot that stands
behind a well while a heavy comes for it); `--gallery
heavy[:open|:hot|:down]` and `docs/shots/heavy/`.

### The sentry: learn it, then walk past it

A turret head on a 4.6 m mast at the Gates, the Depot's mouths and key
pits (`robots/sentry.rs`, its rules `extraction_core::sentry`; placed as
`MarkerKind::Sentry` by Map design, held to `map_sentry`'s lint). It is
built to be got round, not shot down:

- **Its sweep is learnable.** End to end at 24°/s with a pause at each end;
  **behind it is blind**: its head can't turn past its arc.
- **It warns**: a chirp, then a second before the first round of a burst;
  out of sight two seconds and it has to chirp again.
- **It's drawn**: a thrown can's clatter, a hot artifact or a mirror shard
  flying past turns it for five seconds.
- **It runs on a cable** to a junction box behind it: bolt cutters through
  any stretch, or the box wrecked, and it's dark. An EMP blinds it; the
  recall cuts the depots off from it. A heavy lured into its arc takes its
  rounds. At night its searchlight sweeps with it.
- Wrecked (its head shot through), it hangs off its mast, a landmark.

Checks: the rules' tests; the body's (sweep, chirp before fire, never
behind, a cut dark, real bolt cutters on its cable, a noise draws it, a
heavy in its fire); `sentry_cable` (a bot gets behind one, cuts the cable,
walks through its arc unhurt; without the cut it's shot down).

### The spotter, the minelayer and the mender

Three more of the Custodian's machines, none armed, each with a tell, a
weakness, salvage and a scenario.

**The spotter** (`robots/spotter.rs`, `extraction_core::spotter`): an
observation balloon 24 m up on a tether, a camera under it turning a slow
circle (40 s a turn, a 100° view). It needs 3.5 s of you in view for a fix;
its beacon blinks faster and goes red as the fix builds, and a full fix
sounds a klaxon that brings robots near it to look and queues a response
to where you stood. *Weakness:* straight under it is blind (about 20 m
round the winch): get to the winch and cut the tether, or wreck the winch,
and it rises and drifts off downwind. *Salvage:* hole the envelope and it
sinks to the ground: a gas valve, fabric, its eye if whole. Placed by
`MarkerKind::Spotter`. Scenario `spotter_tether` (no call);
`spotter_control` walks out under its watch and is called.

**The minelayer** (`robots/minelayer.rs`, `extraction_core::minelayer`):
a slow tracked crawler that sets out from the gate five minutes into a
raid and lays a staggered line of four pressure mines across the way at
two points of interest. *Tell:* the grind of its tracks, the chunk of its
chute; each mine blinks red every two seconds in a ring of turned earth.
A mine arms after 4 s, clicks under 25 kg and goes off 0.35 s later.
*Disarm* by cutting its lead with bolt cutters or holding a hand on its
prong for 2.5 s: either way a safe `pressure_mine` to sell; or shoot it
from out of its 4 m reach. *Weakness:* its striped hopper: hole it and the
mines in it cook off with it. Seen, it turns tail and drops a mine
behind it. *Salvage:* its cell, a servo, and the mines left in a whole
hopper. Scenario `minefield` (spot, cut, go round, unhurt);
`minefield_control` walks straight through and sets one off.

**The mender** (`robots/mender.rs`, `extraction_core::mender`): a small
quadcopter with a welding arm, sent from the gate when the valley's
machines are hurt (120 hp missing; two a raid). It picks the worst hurt
near at hand and welds back 9 hp a second, only what's still there.
*Tell:* the arc's blue-white flicker, seen a valley off. *Weakness:* it
hangs still and low beside the part it welds; shoot its arm off and it
can mend nothing. *Salvage:* as a drone, and a kit of spare parts.
Scenario `mender_weld` (shoot it off a hurt pylon);
`mender_control` lets it weld the pylon whole.

### The plough: Rime Fell's snow machine

A machine that only makes sense in snow (`robots/plough.rs`, its rules
`extraction_core::plough`): a tracked rotary snow-blower that keeps
Rime Fell's Pass road open for the Custodian's convoys. It runs the road
up from its shed and back, on a beat (about 10 minutes), only while snow
lies; it never leaves the road and carries no gun.

- **The tell.** Its amber beacon, its auger's roar, and a plume of snow
  arcing out of its chute that you see across the valley. Anyone in its
  lane ahead gets its horn about 8 s before it arrives: it doesn't stop.
- **The lane.** Behind it the road is packed and clean (`Lanes`): it
  takes no prints while it's fresh (25 min dry, 6 in a blizzard), so it's
  the one way through the snow the print-reading machines can't follow,
  and it's in full view, and the plough comes back.
- **The bank.** What lies in its way, loot and all, goes out of its chute
  into a bank on the downhill side, buried. A hand held in the bank
  (1.2 s) digs it out.
- **The weakness.** Anything of 1.5 kg or more (a crowbar, a rifle, a
  fuel can) in its auger jams it for 30 s: the engine hatch springs and
  its power cell rides up out of its well, to be shot. Its body shrugs
  off rounds. An EMP stalls it.
- **The salvage.** Its fuel, its auger's scrap, a servo.

Checks: the rules' tests (its beat, the horn's lead, the lane, the
bank side); the body's (the horn then the auger, a tin buried and dug
out by a hand, a crowbar's jam); `plough_jam` (a tin buried, a crowbar
laid in its lane jams it, the bot shoots the cell); `--gallery
plough[:after]` and `docs/shots/plough/` (before and after it passes).

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
- **Each gun its own kick** (`shot_of`, `kick` in `items.ron`). The climb
  goes with the bore's height over where the gun bears (the butt
  shouldered, else the grip): a straight stock turns climb into push. A
  gun's traits scale that: `climb`, `push`, `walk` (which way the muzzle
  drifts) and `shove` (seconds the shot's push is spread over: a snap
  or a shove). A self-loader's carrier adds a little more kick as it hits
  the back of its travel (0.3 of the cycle) and a dip as it slams home
  (0.75). An open bolt slams home first and dips the muzzle 8 ms before
  the round goes. Each of these is a tick in the gun hand, and the slam
  home is a sound (the `CarrierHome` and `OpenBoltSlam` events). A pump
  or bolt worked against its stops jolts the gun, and on its deployed
  bipod the RPK hops and drops back past its line before it settles.
  - The SVD is a straight hard push (it climbs half as much as an SKS,
    with twice the push).
  - The PPSh climbs and walks left, chattering at 900 rpm.
  - The AK's brake keeps its climb low; it walks a little right.
  - The Mosin snaps up.
  - The TOZ double climbs hard; the MP-133 shoves for 28 ms.
  - The Nagant flips off its high bore; the PM is snappy.
  - The flare pistol is a long, slow shove.
  - The TOZ-8 .22 barely moves.
- **A cupped pistol** (`layout::cup`): a handgun has a place under its
  grip for the other palm. Cupped there, it counts as two hands: a stiffer
  spring and more mass behind it. It steadies the gun without aiming it,
  so the gun hand still points it. A PM's kick drops from 313 to 82 mrad.
  Shot: `docs/shots/weapon-feel/pistol_cupped.png`.
- **Held out or tucked in**: away from the chest a gun's weight tells in
  its sway. `SWAY_REACH` is 1.6 mrad per kg at arm's length (65 cm from
  the chest), falling to nothing tucked in at 30 cm. A second hand halves
  it, and a stock against the shoulder bears it.
- **The sling** (`weapons/carry.rs`): a gun you've slung once stays on
  its sling. Let it go anywhere, not thrown, and the sling catches it back
  where it hangs instead of dropping it. Thrown, or put in a bag or someone
  else's hands, it's off the sling.
- **Bumps**: a held gun carried into a wall, a doorframe or a tree is
  stopped by it. The muzzle is knocked back off it, the hands feel what it
  hit by material, and a hard knock (0.8 m/s or more) is heard
  (`GunBumped`). A gun's collider is its own shape, out to the muzzle.
- **Weather and dirt** (`weapons/weather.rs`, `Firearm::grime` and
  `::stiff`): every gun and scope takes the air where it is (its
  carrier's chill, or its own roof).
  - *Cold* soaks in over minutes (a roof warms it out faster) and stiffens
    the oil. A stiff action cycles slower (11 rounds a second on automatic
    to 7) and jams more.
  - *Freezing*: a bolt-action left out cold and wet for 90 s freezes shut.
    Slap the handle up to break it, or bring it in to thaw.
  - *Fog*: a scope cold from outside brought under a roof fogs white
    (`pso_fogged.png`). A glove across the lens clears it, and it comes
    back slowly until the glass warms.
  - *Sting*: taking hold of deep-cold metal stings the hand.
  - *Grime* comes from rain and melting snow on an uncovered gun, a
    dunking, and ash drifting down in the dry. It jams the action and
    makes each shot foul it faster. A wet gun shines; a grimy one goes
    dark and dull (`ak_clean_vs_grimy_wet.png`).
  - *The cure*: clear the jam by hand, then run a glove along the
    receiver. Each 12 cm stroke takes off a fifth of the grime and a
    quarter of the wet. The cleaning kit takes the rest, fouling and all.
  - *The gauge* (core): jams per 100 rounds are 0.08 clean, 4.2 cold,
    5.9 grimy and 9.7 both, back to 1.3 after four strokes of a wipe.
- **Stiff actions in the hand** (`weapons::drag`): in cold oil a bolt,
  pump or charging handle drags. It follows the hand no faster than
  3 m/s divided by (1 + 3 × stiffness), and doesn't move at all unless the
  hand pulls at 0.35 m/s × stiffness; a timid hand slides on it. It
  grinds in the hand as it moves (`ActionGrind` for Audio). A cold
  bolt brought back short and pushed home has short-stroked
  (`Malfunction::ShortStroke`): the trigger's dead until it's worked
  fully. Gauge: a 10 cm stroke at 2 m/s takes 0.06 s warm, 0.14 s cold.
- **Ammunition condition** (`Load::Surplus`, `Load::Damp`,
  `Firearm::ammo_cold`): old surplus lots and rounds that got wet (ball's
  plain primers; AP, tracers, slugs and stars are sealed) misfire and
  hang-fire. So does a round as cold as the gun.
  - *Wetting*: a box, magazine or gun soaked past 0.6 turns a round damp
    every 45 s.
  - *A hang-fire* is a click, then the shot 0.3 to 1 s later from wherever
    the muzzle is by then. Open the action on it first and it goes off out
    of battery: it hurts the hand and the gun.
  - *Restoring*: the bench's "Re-pack a box's old or damp rounds" (a box
    and powder) makes them good ball again.
  - *The gauge*, misfires and hang-fires per 100 rounds:

    | Rounds | Misfires | Hang-fires |
    |---|---|---|
    | fresh | 0.17 | 0 |
    | surplus | 3.2 | 3.3 |
    | damp | 18.8 | 7.9 |
    | cold-soaked | 4.4 | 5.5 |
    | scavenged re-pack | 4.75 | 0 |

    The re-pack's trouble is the squib. The longest hang was 1.0 s.
- **Signatures**: a test, `weapons/signature.rs`, records each gun's
  muzzle path. Each is held as a player would: two hands (a handgun
  cupped), shouldered where there's a stock. The trigger
  is held for half a second on its most automatic setting. The gauge:
  - Each pair of guns must differ in peak rise and its timing, peak push,
    the climb and walk left at the end, and shots, by at least 8.
  - Each must stay under 0.14 rad of climb with two hands. A cup must at
    least halve a handgun's kick in one hand.
  - A single shot must be back within a third of its peak by the end.
  - A new gun in the catalog is measured with the rest.
  - `cargo test every_gun_has_its_own -- --nocapture` prints the table.
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
  reloads on a double feed. Each one feels different in the gun hand: a
  stovepipe jolts, a failure to feed is a dead click, a double feed
  grinds (`weapons/faults.rs`).
- **Faults by type**:
  - *Squib*: the bench's re-packed rounds now and then have too little
    powder. You get a pop and no kick, the action doesn't move, and the
    bullet stays in the bore. Rack the case out and push a cleaning rod
    10 cm down the bore from the muzzle to knock the bullet out. Fire on
    it instead and the barrel bulges: a violent jolt, the gun keeps 30%
    of its condition, and the hand is hurt.
  - *Sticky bolt*: a filthy Mosin's bolt can stick shut after a shot.
    Lifting the handle only grinds until you slap it up hard with a
    palm.
  - *Slam-fire*: a worn, dirty SKS's floating pin can fire the round the
    bolt slams home on, trigger untouched and even on safe, sometimes
    round after round. Cleaning and repair are the cure.
- **Wear you can see** (`weapons/wear.rs`): the bore at the muzzle goes
  from bright steel to soot as the gun fouls, and the crown picks up
  nicks as it wears. The spread opens with both. In the headset, bring a
  cleaning kit, oil or spare parts to the receiver in your other hand.
  Each stroke of the cleaning rod down the bore and back scrubs some
  fouling too.

### What a round does, grenades, blades

- **Through and off things** (`extraction_core::firearm_terminal`): a
  round's energy (its bullet's mass by cartridge, its speed now) against
  each material's cost per millimetre, thin things at their real
  thickness, a longer path at a slant. Pistols and buckshot stop in a
  plank wall; rifles go through planks, plaster and tin; only the Mosin
  through a log wall; brick, concrete, rock, earth and robot plate stop
  everything. Grazing a hard face (under about 20 degrees) mostly turns
  the round, keeping 60% of its speed, with a whine and a streak of
  sparks. The range test (`weapons/range.rs`) fires every gun, and the
  robots' guns, at every material to hold it.
  - Hollow things cost their walls whatever their depth, plus a little
    per millimetre for what's inside. A sheet-steel box thicker than 4 cm
    (a car's door, its body, a bus, a locker) is a shell: two skins of
    0.8 mm sheet, about 440 J for a door. A car door stops pistols,
    revolvers, buckshot and a .22, and nothing else. A walker's, a
    sentry's or a drone's round comes through it, and a door carried as
    a shield is the same door. Broadside, a whole car (door, body, door)
    stops everything but the SKS, the Mosin, the SVD, a slug and a
    heavy's gun. An angled door is thicker: a drone's round that comes
    through square on stops in it at 45 degrees.
  - Robots: a drone's every part and any robot's eye, rotor or gas cell
    is a thin housing, two 1 mm walls of plate with its works between.
    Rifle rounds go in one side and out the other, on to whatever is
    behind; pistol and SMG rounds lodge. Walkers' cores, legs and guns,
    and all armour plates, are solid plate.
  - Robots' rounds are 8 g through the same projectiles, and a walker's
    blind fire reasons with the same costs (`destruction::through`).
  - Out the far side, the round throws spall on along its line: a cone
    of hot sparks and flakes from steel, long splinters from boards, dust
    and grit from masonry, a fan of shards from glass.
  - The gauge (`cargo test -p extraction_core the_penetration_gauge --
    --nocapture`) prints what each round has left, in joules, after each
    thing, square on:

    |              | .22 | Nagant | PM | buck | PPSh | AK | SKS | slug | Mosin | drone | walker | sentry | heavy |
    |---|---|---|---|---|---|---|---|---|---|---|---|---|---|
    | tin wall     | - | 59 | 103 | 80 | 460 | 1177 | 1934 | 2110 | 3391 | 378 | 610 | 1240 | 1760 |
    | plank wall   | - | - | - | - | 330 | 1047 | 1804 | 1980 | 3261 | 248 | 480 | 1110 | 1630 |
    | plaster wall | - | - | - | - | - | 657 | 1414 | 1590 | 2871 | - | 90 | 720 | 1240 |
    | car door     | - | - | - | - | 223 | 940 | 1697 | 1874 | 3155 | 141 | 373 | 1003 | 1523 |
    | whole car    | - | - | - | - | - | - | 384 | 561 | 1842 | - | - | - | 210 |
    | drone shell  | - | - | - | - | - | 538 | 1295 | 1471 | 2752 | - | - | 601 | 1121 |
    | log wall, tree | - | - | - | - | - | - | - | - | 511 | - | - | - | - |
    | robot plate, brick, concrete | - | - | - | - | - | - | - | - | - | - | - | - | - |
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
- **Butt strokes**: any long gun's butt is a blunt weapon. Driven or
  swung butt-first, its harm comes from the speed into what it hits
  (nothing below 2.5 m/s) and the gun's weight. Joints and optics feel it
  most. Parts made to be beaten or cut (a heavy's bolts) carry their own
  multiplier (`WeakPoint`). Every blow's `Hit` says what kind it was
  (point, edge or blunt), so a door gives to a butt before a knife.
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

### The shot as a sound

(`extraction_core::report`, `weapons/report.rs`, `weapons/reverb.rs`)

- **The report**: the muzzle blast, heard all round the gun. It's
  reckoned in decibels against the AK-74's, which carries 220 m; reach
  doubles every 12 dB.
  - A heavier powder charge is louder.
  - A barrel too short to burn its charge is louder still, 4 dB per
    halving.
  - The PPSh's jacket is a compensator.
  - What's on the muzzle takes decibels off.
- **The crack**: a round faster than sound cracks along its flight. It's
  heard only near the path (17 m either side for an AK round, 28 m for
  the Mosin), and from the path. Each shot makes up to six cracks, 40 m
  apart from 20 m out, before its report, and a gun cracks at most every
  0.25 s.
  - A robot that hears the report takes its word for where you are.
  - A robot downrange that hears only the crack goes to look along the
    line, away from you.
- **Suppressors**: the PBS takes 18 dB off and lasts about 1500 rounds. A
  can packed at the bench (any food tin and a piece of scrap, in the
  vice) takes 14 dB off and lasts about 40.
  - As either wears, it takes off less (the square root of its
    condition).
  - Either one fouls the gun twice as fast.
  - A suppressor doesn't stop the crack.
- **Subsonic rounds** (7U1 for 5.45, 7.62 US for 7.62x39; green tips):
  a heavy bullet at 300 m/s on a lighter charge. They make no crack and
  a slightly softer report.
  - Under the PBS, an AK is heard about 68 m off.
  - They drop fast, stop in a plank wall, and hit soft: harm goes by the
    square root of their energy against ball's.
- **Echo**: Audio's `hearing::Hearing::smear(at)` says how much a place
  blurs where a sound seems to come from, for robots' ears. Indoors it
  comes from the room. Outdoors it comes from `weapons::reverb::valley_smear`:
  1 in the open, up to 2.2 in a valley whose sides rise 6 m within 70 m,
  more when the sides face each other.
- **The gauges**:
  - `cargo test -p extraction_core the_hearing_gauge -- --nocapture`:
    every gun's report bare, subsonic, and under each suppressor new and
    at 10%.
  - `the_crack_gauge`: each round's crack reach.

  |            | ball | subsonic | PBS | PBS at 10% | can | can at 10% |
  |---|---|---|---|---|---|---|
  | PM         | 141 | - | 50 | 101 | 63 | 109 |
  | Nagant     | 130 | - | 46 | 94 | 58 | 101 |
  | TOZ-8 .22  | 94 | - | 40 | 68 | 42 | 73 |
  | PPSh       | 181 | - | 64 | 130 | 81 | 140 |
  | MP-133     | 237 | - | 84 | 170 | 105 | 183 |
  | TOZ double | 211 | - | 75 | 152 | 94 | 163 |
  | SKS        | 209 | 184 | 74 | 151 | 93 | 162 |
  | AK-74      | 220 | 194 | 78 | 158 | 98 | 170 |
  | RPK-74     | 196 | 172 | 69 | 141 | 87 | 152 |
  | SVD        | 254 | - | 90 | 183 | 113 | 197 |
  | Mosin      | 241 | - | 85 | 173 | 107 | 186 |

  (Metres, in still air. Weather and walls cut these further.)

### A gun's state, read in the hand

(`weapons/legibility.rs`)

No screen tells you a gun's state; the gun does, as its real counterpart
would. A table of each gun's real tells, held against the models and the
mechanism by `every_gun_shows_what_the_real_one_does`:

- **Cocked**:
  - an exposed hammer back (PM, Nagant, flare pistol);
  - a turn-bolt's cocking piece standing 12 mm back off the bolt (Mosin,
    TOZ-8);
  - the double's two indicator pins standing 3 mm proud, each over its
    cocked lock;
  - the PPSh's bolt held back;
  - a crossbow's string drawn.
  - The AK, RPK and MP-133 show nothing: look in the port.
- **Loaded**: with the action open (a press check), the round in the
  chamber is there to see. A fired case sits a little proud on the
  extractor. A revolver's cylinder shows its rounds, and a clip shows the
  rounds standing in it.
- **Empty**: the slide or carrier held open on the last round where the
  real gun does it (PM, SKS, SVD). An empty box magazine shows its
  follower between the lips.
- **How full**: a magazine weighs what its rounds weigh (an AK's is 0.23 kg
  empty, 0.56 kg full), so you heft it. The PM magazine's round-count
  holes show brass up to how full it is.
- **Condition**: below 0.8 the finish wears toward rust and bare steel, and
  goes rougher. The crown nicks and the bore soots (`wear`). A worn action
  (below 0.6), or a fouled one, drags in the hand as cold oil does
  (`Firearm::action_drag`).
- **Made into**: a cut barrel, a stock off, a bolt out, a suppressor or a
  scope are all on the model as they are.

### Gunsmithing at the vice

(`extraction_core::gunsmith`, `weapons/gunsmith.rs`)

A gun let go of at the bench vice's jaws is clamped there, with its
receiver in the jaws and the barrel out over the bench. Taken up, it's
free again. What's done to a gun is its build, which is saved with it and
worked out afresh from the gun as made whenever it changes.

- **Cut the barrel** (the Mosin, the SKS, the TOZ double, the MP-133): draw
  the hacksaw back and forth across the barrel where you want it to end.
  About 1.2 m of blade over one spot cuts a rifle's barrel; a double's two
  take 2 m. The end drops off as scrap.
  - The muzzle comes back by what was cut, and a suppressor's mount
    follows it.
  - The bullet leaves slower, because less barrel means less push.
  - It groups wider: the sight line is shorter and the front sight is
    gone.
  - It flashes more and is heard further (`report`'s 4 dB per halving).
  - It's lighter and shorter.
  - A cut doesn't grow back, and the bayonet's lug goes with the muzzle.
- **Take the stock off** (those, and the AK and the RPK; not the SVD, whose
  thumbhole stock is its grip): with
  the gun clamped, its stock screw takes the spanner. Two turns, and the
  stock lies on the bench as an item.
  - With nothing to put to the shoulder, the kick climbs about 1.7 times
    as much and it groups about 1.3 times as wide.
  - It's lighter and shorter.
  - Let the stock go behind the grip and it's back on.
- **Swap parts** between guns of one model: a Mosin's bolt, drawn back to
  its stop in the vice and on past it, comes out into the hand as an item
  with its own condition. Let it go into any Mosin's open receiver and it
  goes in.
  - A gun's condition is its parts': the bolt is 35% of it.
  - Each part's own condition is kept as its difference from the gun's,
    so a good bolt raises a worn rifle by its share, and swapping back
    puts both guns as they were.
  - With its bolt out, a gun neither cycles nor fires.
- **What it looks like**: every gun that can be worked has its barrel (and
  forend) and its butt as model nodes of their own. A test holds it
  (`every_workable_gun_shows_its_work`). A cut barrel is drawn shortened
  back toward the breech (the double's hinged pair too), the butt is gone
  when the stock's off, and the bolt is gone when it's out. A stock or
  bolt off a gun is drawn from that same piece of the gun's model.
- **Where the things come from**: the hacksaw comes with the vice and
  turns up in toolboxes and workshops. Workshops sometimes hold someone's
  half-finished work: a loose stock or a Mosin bolt. A test holds that the
  vice's tools and parts, and the subsonic rounds and the can, can all be
  found, given, made or taken off a gun
  (`what_the_vice_needs_and_makes_can_be_had`).
- **For the ear**: each step writes `weapons::gunsmith::Worked` (clamped,
  saw stroke, cut, screw turn, stock off or on, bolt out or in). Audio
  voices them.
- **The gauge**: `cargo test -p extraction_core the_gunsmith_gauge --
  --nocapture`

  |                 | barrel mm | m/s | heard m | MOA | flash | kg | m | climb |
  |---|---|---|---|---|---|---|---|---|
  | Mosin           | 730 | 865 | 241 | 1.5 | 1.0 | 4.00 | 1.23 | 1.00 |
  | Mosin, no stock | 730 | 865 | 241 | 1.9 | 1.0 | 3.25 | 0.93 | 1.89 |
  | Mosin obrez     | 250 | 653 | 320 | 4.7 | 4.5 | 2.58 | 0.45 | 2.12 |
  | TOZ double      | 720 | 400 | 211 | 150 | 1.0 | 3.20 | 1.12 | 1.00 |
  | sawn-off        | 300 | 336 | 282 | 423 | 3.4 | 1.98 | 0.40 | 2.16 |
  | SKS             | 520 | 735 | 209 | 3.0 | 1.0 | 3.85 | 1.05 | 1.00 |
  | SKS cut         | 300 | 662 | 251 | 5.5 | 2.3 | 3.59 | 0.83 | 1.04 |
  | MP-133 cut      | 350 | 367 | 268 | 237 | 1.8 | 3.14 | 0.97 | 1.04 |
  | AK-74           | 415 | 900 | 220 | 4.0 | 1.0 | 3.30 | 0.94 | 1.00 |
  | AK-74, no stock | 415 | 900 | 220 | 5.2 | 1.0 | 2.95 | 0.69 | 1.80 |

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
- **Looking through it** (`weapons/optics.rs`):
  - *Eye box*: a scope shows its picture only with your eye about 7 cm
    behind the eyepiece and on its axis. Drift a few millimetres off and
    a black edge closes in from the side you moved away from; lean back
    or crowd it and the picture shrinks to black.
  - *Parallax*: off the axis, the picture slides against the reticle, so
    the reticle on a target only holds true with your eye centred.
  - *Glint*: at dawn and dusk, a scope facing the low sun flashes back
    along its line. A robot it points at, out to 400 m, sees the flash
    like a torch beam in the dark. Don't glass a valley into the sun.
  - *Iron sights* line up at the cheek: bring a gun without a scope up
    with your eye just behind the rear sight, and it settles (at most
    about 3 degrees, about the front post) so your eye, the rear sight
    and the post are in line. Your eye is on the target, so the front
    post goes soft as it comes up: a dark, soft-edged disc about 12 mm
    across that fades in round the post. It's an approximation of focus,
    not a depth-of-field pass. Comfort setting `post_blur`, or
    `--no-post-blur`, turns it off. Shots are in `docs/shots/weapon-feel/`
    (`eyebox_*.png`, `iron_sights_at_the_cheek.png` and
    `post_blur_on_off_zoom.png`). The blur barely shows, since the rear
    notch hides most of the post.
- **Zeroing** (`weapons/zero.rs`): the PSO-1 (100 to 1000 m) and the PU
  (to 1300 m) have an elevation drum. Take it with the other hand's
  trigger and turn your wrist, clockwise from above for more range, one
  click per 100 m (in flat mode, `]` and `[`). The zero comes from the game's own ballistics (the
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
