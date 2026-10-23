# Anomalies

What lies in the Zone, how to find it before it finds you, and what it
looks like. Code: `src/anomaly/`, rules in
`crates/extraction_core/src/anomaly.rs`, shaders
`assets/shaders/anomaly_*.wgsl`.

## The kinds

| Kind | Where | What it does | Tell | Grows |
| - | - | - | - | - |
| Lift | the Surge scatters them | floats and swirls anything loose; the longer something's in it, the sooner it pops and flings it all out | circling leaves; as it winds up, leaves and junk slide in along the ground from twice its radius | featherstone |
| Burner | the Surge | burns whoever stands in it, cooks robot parts | coal-red glow, embers, smoke, heat shimmer over the patch; a flare when something steps in | hearth |
| Snare | the Surge | discharges into whatever touches it (robots too), then recharges; metal in a hand draws the arc from 0.8 m further off | blue arcs and sparks, a crackling light; arcs reach out to what they strike | aegis knot |
| Well | standing, one a valley | drags loose things and walkers toward a heart 1.3 m up that crushes (30/s); bends rounds that pass close | a lens in the air that turns the world over, grit spiralling in, a violet throb | lodestone |
| Mirror | standing, one a valley | blinks whatever crosses it out the far side, radius + 3 m on, still moving; 1.5 s to fold back up | almost nothing: a shimmer at waist height, a glint now and then | mirror shard |
| Hush | standing, one a valley | eats every sound made in it: a shot, a step, your stones' hum never leave it; detectors inside are mute; past 8 s the silence presses (3 hp/s) | almost nothing: pale dust hanging dead still | hushstone |
| Frost | standing, Rime Fell's own (the col, the scree) | still cold that steals heat: it feeds the body's chill (the valley's, `world::cold`), 0.3/s at its edge to 0.8/s at its heart, so a second or two in and you're frozen through and shivering for minutes after; in it you stiffen (up to 70% of your pace, too stiff to run from 0.4), you're told at 0.4, and frozen through it bites (8 hp/s) | a white rime on the ground, motes hanging and glinting in a cold blue light, the air over it faintly wavering | rimestone |

Wells, mirrors, hushes and frosts don't move between cycles: the maps place them
(`MarkerKind::Anomaly`), so they become landmarks you learn. The director
can still bloom any kind in your way.

### Placing standing anomalies (and any marker)

Every map marker clears trees for 8 m around it (`forest::scatter_trees`).
Each scatter cell now draws from its own seeded stream. So a new marker,
road or building removes only the trees in its own cells, and every other
tree stays exactly where it was. Before this, the scatter drew from one
stream per forest: one cleared cell shifted every later tree in that
forest. That once put a tree in the camera's face on a budget view
(Kettle's quarry view, overdraw 4.3 to 10.3).
Test: `forest::a_new_marker_clears_only_its_own_trees`. Rerun the
map's `*_holds_the_budget` test anyway when you add one.

### Migration

The Surge's own anomalies (lifts, burners, snares) aren't scattered afresh
every cycle. Every 6th cycle (`SURGE_EPOCH`) the Surge scatters them anew.
On each cycle in between, each one creeps up to 7 m (`MIGRATE_STEP`) down
the slope it sits on, with a little wobble. It stops short of a road,
water, a building or another anomaly, and once on the flat of a low it
only stirs. So a field you learned last cycle is still there, a little
further down. Over a run they gather in the valley's hollows and along
its stream banks. On Marrow with seed 3, 16 of 17 moved over five
cycles, and they ended 3.6 m lower on average.

Each one that moved leaves a **drag scar**: torn turf smeared across the
ground from where it sat to where it sits now. So the trail points to it.
Shots, cycle 0 then cycle 1 (the Hollow's lift):
`migration_cycle0.png`, `migration_cycle1.png`. Every cycle still grows
its own stones. Rules: `anomaly::layout` and `migrate`.

### Wells and bullets

The lens is `K / (d² + s²)` toward the heart, summed along each tick's
segment (a rifle round crosses a well's reach in a tick or two). A rifle
round a metre off the heart turns a few degrees; a pistol round, at half
the speed, about four times as much. Shooting across a well at a robot
behind it misses. The pull reaches three radii; a walker can still walk
out from twice the radius, not from the heart.

### Frosts

A frost's cold is the valley's cold: it's the one chill a body has (Fifth
valley's `world::cold::Chill`), so a frost on top of an hour on the col is
worse than either, and you warm it off the same way, under a roof or going
down. Its stiffness is its own and only while you're in it: out, it leaves
you in a couple of seconds. You're told before it bites, and you can always
crawl out. The hearth and the rimestone are each other's way in: a hearth
keeps a frost off, a rimestone keeps a burner off. Code:
`src/anomaly/frost.rs`.

### Mirrors

It sends you out the far side of where you came in. If that spot is in
water, a building or another anomaly's heart, it swings round a sixth of a
turn at a time until it finds open ground. Its shard hangs in the fold
unmoved: throw something in, then reach in while it's still closing.

## The Surge

Through the five minutes' warning every anomaly stirs: it swells (up to
30% wider), charges and recharges up to three times as fast, burns
hotter, and flares on its own beat, harder as the Surge nears. At 240,
170, 110, 60 and 25 seconds out a new anomaly blooms round whoever is
still out: 10 to 25 m off, to the side of the way they're heading (50 to
130 degrees off it), never on or by a road, and clear of the others. Each
spends its first four seconds forming, lit up and harmless, so you see it
open before it can hurt you. Keep to the road and you get through.
Rules: `extraction_core::anomaly::surge_*`; game side `src/anomaly/surge.rs`.

## Finding them

- **Throw something in.** A bolt (junk, found in tool boxes and
  workshops), a nut, a tin. The moment a loose thing first crosses into an
  anomaly it shows itself for six seconds: its light, particles and haze
  surge, whatever kind it is.
- **Detectors**, cheapest first (`DetectorTier`):
  - **Clicker** (`detector`): carried anywhere on you it clicks for the
    nearest anomaly's edge, about 1.5 s apart at 25 m, a rattle at the
    edge. Held, it favours the way its antenna points.
  - **Sweep** (`detector_sweep`, rarer): held, it hears only what lies in
    a 25° cone ahead, so you sweep to find it; and a second, higher
    voice (`StonePing`, its green lamp) sings for a loose stone in the
    cone within 20 m, quicker the nearer. Worn, it's a plain clicker.
  - **Bear** (`detector_bear`, rarest): held up, its round screen shows
    the stones within 20 m as bright dots and the anomalies within 25 m
    as dim ones sized by their reach, the way you face. It still clicks.
  Shot: `docs/shots/anomalies/detectors.png`.

## Artifacts: what they give, what they cost, how to get them

Every stone does something for you while you carry it, and costs you
something physical; nothing shows a number. Each draws the Zone's
attention too (the hum). Sealed in a lead-lined case it does neither:
carry it safe, or carry it live.

| Stone | From | Gives | Costs | Getting it out |
| - | - | - | - | - |
| Featherstone | lift | takes 6 kg off your load | it wants home: lifts within 15 m wind up toward it and pop | it rides the column out of reach; throw something in, stand clear, catch it as the pop throws it |
| Hearth | burner | power gadgets plugged into it never run down | you run hot: your wind comes back at half the pace | it's hot in the coals and burns the hand that holds it; shoot or flick it out, let it cool 20 s or drop it in water |
| Aegis knot | snare | the first hit glances, then 90 s to knot back up | it holds a charge: wet (soaked ground, wading) it shocks you every 8–15 s, with a crack robots hear | the snare ignores its own stone but strikes a hand reaching in; ground it with thrown metal and reach in while it recharges |
| Lodestone | well | rounds fired at you curve aside | five kilograms more on your back | it hangs at the crushing heart; dash in and out |
| Mirror shard | mirror | use it to step up to 4 m ahead, once a minute | every 20–40 s its late reflection glints, and robots within 35 m hear where you are | it hangs in the fold; throw something through and reach in while it closes (1.5 s) |
| Glass eye | rare, any field | held to your eye, walls go thin | strong attention | as its anomaly's stone |
| Hushstone | hush | a small Hush round you (1.2 m): your steps and shots aren't heard | every detector you carry is mute; attention 1.5/s | it lies in the dead air: in and out before 8 s |
| Rimestone | frost | a burner's coals don't take you | a little chill (0.15) never leaves you: a little stiff, always | the frost lies round it: carry a live hearth (it chills you a quarter as fast and never past 0.3, and the frost never bites), walk in and out; without one, leave it |
| Stormglass | only the Surge's own blooms (about one a Surge) | the Surge passes over you as if you were under a roof | the whole Zone knows where you are (attention 3/s) | as its bloom's kind, once it has finished forming |

Rules and numbers: `extraction_core::anomaly` (costs, harvesting);
game side `src/anomaly/costs.rs`, `harvest.rs`, `artifacts.rs`.
Scenarios: `harvest_snare`, `harvest_lift`, `harvest_burner`,
`harvest_well`, `harvest_mirror`, `harvest_hush`, `harvest_frost` (in
`--scenario all`).

**Harvest band** (`scripts/harvest_band.py <sim dir>`): over a batch of
`hunter` sim raids, at most 20% of deliberate dives into anomalies may
end in death, and at least 30% must bring a stone out. Per kind, it also
reports stones taken a dive and health lost a dive, and the stones that
raids which extracted brought home.

Sim raiders harvest two ways (`sim::harvest`):

- **Skilled** (the default): each kind's way, as in the harvest
  scenarios. They bring four bolts.
- **Novice** (`SIM_HARVEST=novice`): walk in, grab, walk out.

Last measured, 6 hunter raids a valley over all four valleys:

| | Dives | Stones | Health lost a dive | Anomaly deaths |
|---|---|---|---|---|
| Novice | 42 | 17 (40%) | 18 | 1 |
| Skilled | 37 | 16 (43%) | 9 | 0 |

**Anomalies in a hunter's raid.** A hunter used to go only where the
loot is (buildings, spots, the Surge's artifact fields), so it met a
standing anomaly only by passing one. Now it knows the valley's standing
anomalies (`SiteKind::Anomaly`, from the map's markers), and goes to
their edge to see whether one has grown a stone this cycle
(`SIM_NO_ANOMALY_SITES=1` for the old way). The band now also counts the
raids that dive at all: at least half must (`MIN_DIVING_RAIDS`).

Free play, 6 hunter raids a valley (12 minutes), raids that dive at
least once, and what anomalies cost a dive:

| Valley | Before | Skilled | hp a dive | Novice | hp a dive |
|---|---|---|---|---|---|
| Marrow Creek | 33% | 50% | 3 | 67% | 11 |
| Kettle Pass | 33% | 67% | 7 | 50% | 7 |
| Cinder Works | 83% | 83% | 9 | 100% | 16 |
| Sallow Reservoir | 83% | 83% | 7 | 100% | 14 |
| Rime Fell | 100% | 100% | 7 | 67% | 12 |

No raid died in an anomaly. Most dives are still the Surge's kinds by
the fields (burners, snares, lifts): a standing anomaly has a stone
about one cycle in three, and in 60 raids hunters took a lodestone and a
hushstone from them. They walked up to the fell's frosts and left them:
a skilled raider goes into a frost only with a hearth on it, which it
has to have won from a burner earlier in the raid. So the head-on
measures stand beside free play:

The frosts sit high on Rime Fell (the col, the scree), and the sims'
raiders never get up to one in a 10 or 25 minute raid (18 and 8 hunter
raids on the seeds that grow rimestones: no dive). So the Frost's band is
measured head on (`sim::harvest` test): the same dive in to a 3 m frost's
stone and out costs a novice 17 hp (frozen through, bitten), and with a
live hearth 0 (chill held at 0.3). A skilled raider without a hearth
leaves it.

Skill halves what anomalies cost and takes the deaths out, and it gets
into wells and wins more lifts. It doesn't yet win more stones overall.
A skilled raider's shots don't knock a hearth out of the coals in the
sims, so it gets none, and leaves the stone rather than reach in.

Two fixes the bands needed on the way:

- A bot could grab anything at arm's length at any height. Novices
  plucked a lift's stone riding 3 m up, which no hand can, and out-
  harvested the skilled. Bots now reach no higher than `STAND_REACH`
  (2.2 m) over their feet.
- A wet aegis killed raiders who couldn't get dry in a downpour. Its
  shocks now stop at `AEGIS_FLOOR` (20 hp).

## Looks, and what they cost

- Particles are moved in the vertex shader: each system is one mesh of
  quads, one draw however many there are. A lift used to be 65 draws (one
  a leaf); now it's four (mark, leaves, dust, haze).
- Refraction samples the view transmission texture (a StandardMaterial
  with `specular_transmission` puts the haze in Bevy's transmissive pass),
  bent through a noise-wobbled normal; Bevy's own blurred fetch turned
  depth-rejected taps into black specks at the cards' edges. The copy
  behind it happens only while a haze is in view (within 55 m).
- Particles thicken with distance (glowing ones most, up to 2.5×), so
  a spark or an ember keeps a pixel or two at 20 m in rain instead of
  flickering out. Glowing particles use a soft round dot. A snare's arcs
  are never thinner than about 0.15° from the eye.
- One unshadowed light each. A kind's flicker (`visuals::flicker`)
  drives its light, its particles' brightness and its haze's glow
  together.

Shots: `docs/shots/anomalies/` (`before_*` are the old looks).

### Readable from 20 m: the gauge

`scripts/anomaly_legibility.py` takes the shots. For each kind, one
anomaly on Marrow is viewed from 20 m, in pea-souper fog, in a downpour
and on a moonless night. Each view is shot three times:

- once with the anomaly's looks;
- twice without them (`EXTRACTION_HIDE_ANOMALIES`: the anomaly still
  works, it just isn't dressed).

The score is the share of the box around the anomaly that changes
between "with" and "without", minus what changes between the two
"without" shots (rain and swaying grass: the noise floor). Each kind must
change at least 3% of the box. A mirror only has to manage 1%, as it is
meant to be almost nothing.

The cameras come from `anomaly::view_of` through the `anomaly-views`
tool. Of sixteen bearings, it takes the gentlest ground whose line to the
anomaly is clear of rises, trunks and other anomalies, and off roads and
water. So the views follow the layout when it moves.

The gauge writes a contact sheet, `legibility.png`, and its numbers in
`legibility.json`. The first run is kept as `legibility_before.*`.

The floor is what you see, not a share of the box: at least 2 square
degrees changed, about a head at 10 m (1 for a mirror). That's at the
shots' 12 px/deg (75° is the vertical field of view). The first cut held
each kind to 3% of the box, which is 36 deg², and marked a plainly seen
burner LOW. The views keep their line of sight off trunks, rocks and
anything the map places (`sight_blockers`). A boulder once hid the
mirror.

Where it stands: every kind clears its floor in all three conditions in
every valley, 90 views of 90 (`--map=NAME`; sheets
`legibility.png` for Marrow Creek and `legibility_<map>.png` for the
rest). The table gives each kind's weakest view across the valleys, in
deg²:

| Kind | Weakest | Where |
|---|---|---|
| Lift | 44 | Rime Fell, night |
| Burner | 12 | Cinder Works, downpour (on ash) |
| Snare | 7 | Sallow Reservoir, fog |
| Well | 19 | Rime Fell, downpour |
| Mirror | 2.2 | Rime Fell, fog (on snow) |
| Hush | 3.1 | Sallow Reservoir, night |
| Frost | 9.3 | Rime Fell, fog |

(Mirror, Hush and Frost as measured once their rims were gone; see
below.)

What the valleys taught us, beyond Marrow:

- **Snow takes added light.** The mirror's glint added light, and on
  Rime Fell it changed almost nothing (0.4 deg² at night). Its glint now
  steps away from what's behind it, darker over light and lighter over
  dark. Inverting the background wouldn't work, because mid-grey fog
  inverts to itself.
- **Nothing outlines an anomaly on the screen.** The Hush, the mirror
  and the frost each had a rim: a bright or stepped-away band round the
  edge of the haze card. It carried them through fog and snow, but it
  was a circle on the glass, not a thing in the world. The rims are
  gone; the haze fades out well inside its card; and the gauge fails any
  anomaly whose change round its centre is a ring (`OUTLINE_MAX`: the old
  frost rim scored 0.56, a lift's swirl 0.22). Without them every kind
  still clears its floor, on what's in the world: the frost's rime and
  glinting motes, the Hush's dim dead air and its edge of settled dust
  (ragged, not a drawn line), the mirror's glint.
- **A view needs to clear walls by their faces, not their middles.**
  On Kettle Pass the Hush's first view was a brick wall.

The first runs, re-scored the same way, had the lift at 0 and the snare
at 1.2 in fog on Marrow, and the mirror at 0 at night. A lift's column of
air is now a shade darker, and that is what carries it through
pea-souper fog, where its leaves don't.

What it taught us:

- **Fog takes the Zone's things as if half as far off.** Anomaly
  particles are fogged at `FOG_REACH` = 0.5 of their distance. At full
  fog a lift's leaves were gone at 20 m.
- **Luminance, not hue, carries in grey weather.** The Hush dims what
  you see through it, and so does the well's lens at its heart. Both are
  applied after fog, so fog can't wash them out. Colour drain did almost
  nothing in grey weather, and colour-blind eyes miss it anyway.
- **In the dark there's nothing to dim or brighten.** A Hush at night
  is its pale dust, and only just (3-6 deg² in three valleys).
- **Motion reads from the corner of the eye.** The mirror carries a
  glint that crosses it every 4 s, each mirror on its own beat.

### First contact: what it is and where it starts, at 5 m

A first-time player meets an anomaly on foot in VR, with nothing on the
field map. What tells them it's there is the 20 m gauge above; what it
does, and where it starts, has to be in the world too, never an invisible
wall. So:

- **Each kind does something you can see to the ground and the grass
  in it, out to its edge and no further** (`shaders/sway.wgsl` takes the
  four anomalies nearest the eye, `world::cover::AnomalyGrass`):

  | Kind | The grass | The ground |
  |---|---|---|
  | Lift | torn up and gone | scoured to raw rusty subsoil |
  | Burner | burnt to the ground | scorched, coals glowing in patches |
  | Snare | standing on end, quivering | fused dark and glassy, still faintly charged (blue) |
  | Well | laid flat toward its heart | pressed dark and hard |
  | Mirror | wiped flat | glazed like still water, giving back the sky |
  | Hush | dead still, whatever the wind; pressed into its dust | pale settled dust, and its edge a ragged line of it |
  | Frost | stiff, bowed under the rime | hoar, faintly blue |

  What it does to you comes on top of that: leaves circling, embers and
  heat shimmer, arcs, grit spiralling in, a glint, dust hanging dead still,
  glinting crystals and your own breath.

- **The mark runs to the edge.** Every mark used to share the particles'
  soft falloff, solid to half its radius and gone by 0.8: a burner's
  scorch ended 0.9 m inside where it burns. Marks have their own texture
  now, solid to 0.93 of the edge and gone by 1.02, its edge wavering a
  few centimetres, mottled in colour and glow so it reads as ground, not
  a painted disc.

- **It holds in murk and dark.** Marks are fogged as if half as far off,
  as particles are (`MarkMaterial`, `shaders/anomaly_mark.wgsl`), and the
  lift's, mirror's and hush's hold a faint light of their own, too faint
  to see by day.

**The edge gauge** (`scripts/anomaly_edges.py [--map=NAME]`): for each
kind, a view from 5 m off its edge at eye height, in clear day, fog,
downpour and a moonless night (by the player's torch: every stash has
one; `--torch` hangs its beam on the shot's camera). Along five lines on
the ground across the edge it measures the colour difference (CIE ΔE)
the anomaly makes, smoothed over three 10 cm steps, and asks that what it
changes 0.3-1 m inside beats what it changes 0.3-1 m outside by 8 ΔE,
and that the change crosses halfway within 35 cm of the true edge. Three
lines of five must pass. (It samples a strip one row tall: at 5 m half a
metre of ground is a few rows, and a square window straddled the edge.)

Marrow Creek, before and after:

| | Edges that read (of 28) |
|---|---|
| First run | 17 |
| Marks to the edge, grass, half-reach fog, torch at eye, faint light | 27 |

The one that doesn't, the lift in a downpour, is plain to the eye (a
rusty patch against wet grass) but the standing grass just outside it
hides the strip inside its edge from 5 m, and both are dark. Sheets:
`docs/shots/anomalies/edges*.png`.
