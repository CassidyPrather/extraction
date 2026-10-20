# Underground sites

One underground site per valley: dark, tense, several rooms, explored by
torchlight, with the best loot behind the most risk.

| Valley | Site (`prefab`) | What it is | Rooms | Ways out |
| - | - | - | - | - |
| Marrow Creek | `bunker_marrow` | Civil-defence shelter under the village: blast door, gas lock, decontamination, a spine corridor, dormitory, mess, filter plant, generator, command post, armoury, medical post, escape tunnel | 11 | Entrance stair; escape stair 60 m north |
| Kettle Pass | `mine_kettle` | Mine gallery behind the mine office: shaft station, lamp room, main drive, crosscuts, a tall stope, explosives magazine, sump, refuge chamber, raise | 10 | Entrance incline; the raise's stair |
| Cinder Works | `tunnel_cinder` | Service tunnel and pump room under the factory hall: stair hall, two tunnels, pump room, valve chamber, switch room, store, cable vault, workshop, overflow chamber | 10 | Entrance stair; a stair south of the hall |

Plans: `docs/underground/<site>.svg` (regenerate with
`PLAN_DIR=$PWD/docs/underground cargo test -p extraction_core plans_draw`).
Gold dots are loot, red bars doors, blue stairs; `*` a lamp still on, `~`
a failing one, `x` a dead fitting.

## Layouts are data

Each site is `assets/underground/<site>.ron` (format and rules in
`crates/extraction_core/src/underground.rs`): rooms as rectangles (centre,
size, ceiling height, lamp, props with loot tables), corridors between
room centres through `via` points (every leg along x or z, at least 1.4 m
wide, doors at either end), and portals (a kiosk on the surface and a
stair down from it). Site space: the entrance kiosk's centre on the
ground, `+Z` the way its stair goes down. The layouts ship in the binary:
edit, rebuild, and the tests below tell you what broke.

`underground::check` holds every layout to: rooms reached from the
entrance, every stair landing in a room or corridor within 4 m of its
foot, no stair over a room (the bots' indoor grid is one level), no prop
in the rock, corridors along the axes. Each site needs a loop (more joins
than rooms) and a second portal.

The layout is rasterised on a 0.25 m grid (`Raster`): the game builds its
walls on the rock side of every edge between open cells and rock (so a
1.5 m corridor is 1.5 m clear), the bots' grid is the raster, and the plan
views draw it.

## On the map

A map places a site as an ordinary placement (`(prefab: "bunker_marrow",
at: (..), rot: ..)`). Its footprint is the entrance kiosk with its apron
(`underground::FOOTPRINT`, wired into `map::prefab_footprint`); the rooms
lie at the layout's `floor` below the kiosk's footing. Exit kiosks are not
placements: the heightmap levels the ground under each to the layout's
`y` (`Heightmap::underground`), and the walkers' grid keeps off them
(`underground::exit_footprints`).

- `every_site_is_under_rock`: at least 0.8 m of ground over every ceiling
  past the kiosks, on every valley; each exit's ground within 0.3 m of the
  layout's `y`.
- Moving a site: `SCOUT=<map>:<site>:<x>:<z>:<radius> cargo test -p
  extraction_core scout -- --ignored --nocapture` lists spots with cover,
  clear kiosks and the exit's height step; `where_sites` prints the rooms
  in world space (for screenshots).

## Holes in the ground

The terrain has no holes except over the stairwells: `Heightmap::holes`
marks every cell whose centre is within 0.75 m of a kiosk's stairwell.
The heightfield collider removes those cells (parry's cell status), and
the terrain mesh leaves them out (`terrain.rs`, `holed`). The kiosk's
walls and its 1.5 m concrete apron cover the ragged edge;
`holes_only_under_kiosks` checks that no hole reaches past the apron and
that the whole stairwell is open.

## Light

- **No daylight below**: every room and corridor is a closed box (walls,
  ceiling slabs, doors). `no_daylight_in_the_rooms` casts rays every way
  from every room (doors shut) and none gets out.
- **Going dark** (`go_dark`, client): below ground the global ambient
  falls to 1.5%, the sun goes out and the fog's colour goes to black; the
  eyes adapt 2.5 stops (`Exposure`). On the surface everything is as the
  sky left it. Rain and ash aren't drawn round an underground camera.
- **Emergency lamps**: rooms with `lamp: Steady` or `Flicker` get an
  unshadowed point light (range room size + 4 m) and a glowing lens;
  failing ones stutter every 7-10 s. Budget: at most `LAMP_BUDGET` (6) a
  site, two or three in practice; lamps further than 45 m from the eye are
  hidden. No lamp casts shadows: the player's flashlight (a shadowed spot)
  is the only shadow caster underground.
- `EXTRACTION_TORCH=1 scripts/shot.sh ...` gives the screenshot camera a
  torch like the flashlight gadget's.

## Hazards and props

- **Anomalies** live in the layouts (`anomalies: [(kind, room, at,
  radius)]`) and are laid out again each cycle with the valley's
  (`underground::spawn_anomalies`, called from
  `anomaly::spawn_cycle_anomalies`): a live snare by Marrow's generator, a
  lift in Kettle's stope, a snare guarding the crate in Cinder's overflow
  chamber. `check` keeps each inside its room and clear of the doorways:
  you go round them, you're never made to walk through one.
- **Crawlers** (Robot minds' `robots::indoor`): each site's layout names
  a patrol round (room ids in walking order) and how many crawlers walk
  it (Marrow 2, Kettle 1, Cinder 2). `register_residents` hands Robot
  minds the site's plan grid (the ground round the kiosks and deep water
  taken out: they stay below and dry), its floors and the round
  (`IndoorSites`), and tells every robot where it's dark and where the
  lamps are (`senses::Lighting`): in the dark a crawler sees you only by
  lamplight or your torch.
- **Props**: the layout's vocabulary (`PropKind`) is built from the
  houses' furniture where it fits (shelves, lockers, crates, tables,
  workbenches, beds as bunks, barrels, pallets, counters, chairs), plus
  `art/props/bunker.py`: `filter_unit` (Machine), `switch_cabinet`
  (Cabinet), `mine_tub` (Cart) and `bulkhead_lamp` (every lamp fitting).
  Rubble is boxes for now. `check` keeps props a metre clear of every
  corridor mouth and stair foot.

## Power

Each site has a breaker board or a generator somewhere awkward (layout
`power`): Marrow's generator beside the live snare in the generator room,
Kettle's breaker in the wet sump, Cinder's at the back of the dead-end
cable vault. Until it's thrown, the doors it holds (the armoury, the
explosives magazine, the overflow chamber: the best loot) won't open
(`PowerLocked`), and the `Lamp::Powered` rooms are dark. Throwing it (a
pressable, through the hands) unbolts them, lights those lamps (and adds
them to the robots' `Lighting`), and makes a noise the crawlers come to;
a generator keeps humming. `a_hand_on_the_switch_powers_the_site`
tests it through the headset harness. Sim raiders whose way to an item
runs through a held door throw the switch first (a `power` event in the
telemetry); cautious ones leave it.

The power comes in stages (`power` is a list): each stage's switch is
dead ("Nothing upstream.") until the one before is on, each holds its
own doors (a room's, or `portal<N>`: an exit stair's, kiosk and foot)
and lights its own rooms (a room's `stage`). The layout check floods
the plan with each stage's doors and every later one's shut and wants
the switch reached (`power_stages_are_reached_in_order`); the headset
test throws a late switch first, then each in turn. Raiders throw the
switches in order up to the stage a door needs, and never leave by an
exit still held.

## Water

Floods are layout data too (a room or a corridor, and a depth): a drain
from Marrow's gas lock to the filter plant (1.0 m), Kettle's drowned old
drive from the lamp room to the west crosscut (1.35 m: you float) and
its wet sump, Cinder's culvert from the stair hall to the pump room
(1.1 m) and its flooded overflow chamber. The corridors are short cuts:
slower (wading drags, the bots' grid costs it) and louder (splashing is a
robot `Noise`), and the crawlers won't follow into water deeper than 0.9 m
(`DEEP`). Underground the player reads the site's water
(`site_water`), never the ground's water above.

## What it costs to draw

Nothing culls by the rock, so a site under a village would be drawn in
every view of it. A site's geometry more than 1 m below its kiosks is
merged apart (`Below`) and shown only while the eye is underground or
within 18 m of a kiosk; door leaves below are drawn within 25 m (a
kiosk's within 50 m), lamp lenses within 25 m. Marrow's budget views
(`render::budget`) hold the 1200-draw line with the shelter in.

## Bots

A site is a `Building`, so the sim's indoor planning sees it:
`sim::indoor::plan_site` builds one grid with a floor height per cell
(rooms and corridors at the floor, stairs at their treads, the ground
round each kiosk), and `BuildingPlan::levels` makes `contains`, `in_grid`
and `reachable_height` level-aware (loot in a house above the shelter is
not the shelter's). The entries are the stands outside every kiosk door, so
a raider can leave by the exit. The generic reach and door tests cover the
sites; `a_bot_goes_down_and_out_the_other_way` walks a bot from the
entrance to the furthest loot and out by the other kiosk on every valley.

## The Depot (design, in progress)

The place the convoys go. The lore says each Gate is a depot, the
machines keep everything they take "pending review", and only the three
keys end it. The Depot makes that physical: the valleys' things, sorted
and stacked under the Gate, your own lost kit among them.

**Where.** Under the Marrow Gate (-250, -255), in the robots' corner of
the valley where it began; Hana's dog was carried "to the Gate", and so
was she. One depot is built (Marrow's), deeper and bigger than the three
sites: four levels stepping down, about 60 x 45 m, 7 m to 20 m below
the yard.

**The way in: follow the haul.** The Depot's intake is a wide ramp in
the Gate's yard behind a steel shutter. After every Surge the machines
bring in what it swept: a haul convoy walks down the ramp and the
shutter stands open for a few minutes; you go in behind it, or wait for
the next. (In the recall's epilogue it stands open for good, the
machines asleep in their bays.) Lev tells you about it on the radio at
rank 4, after her letter ("she says they keep everything"), and
Progression's late jobs send you there; Story places what you find.
*Proposed to Progression and Story; theirs to decide.*

**The way out** is the service stair from the bottom level, up to a
hatch in the rim woods: only the last power stage opens it.

**Levels** (each a set of rooms on its own floor; stairs between them;
no level over another, so one plan grid still serves the bots):

| Level | Floor | Rooms | Power |
| - | - | - | - |
| 1 Intake | -7 m | the ramp's foot, a receiving bay where the haulers set down, a guard post | stage 1: the intake breaker, by the bay |
| 2 Sorting | -11 m | the sorting hall (24 x 16, 6 m high): rows of bins by kind, a gantry; the tag office (a terminal: "KEPT FOR REVIEW") | stage 2: the hall's control desk, opens the stacks |
| 3 The kept | -15 m | long stacks by valley and house: crates with names on; one says MAREK | lights only on stage 2 |
| 4 Cistern | -19 m | the pump gallery, flooded (1.2 m), the old pumps; the service stair's foot | stage 3: the pump room's breaker opens the stair out |

**What's there.**
- *Your lost kit.* Everything the memorial convoy carried off
  (`Memorial::lost`) lies in the sorting hall's bins, tagged with the
  raid it was taken on. You can take it back.
- *The valleys' things.* Bicycles, the bins, bus four's destination
  board, the chapel's candlesticks: what the notes say was taken.
- *The kept.* Rusty's lead and whatever Story decides of Hana's in the
  MAREK crate, and the Holms'.
- *Danger.* Crawlers on every level, and on the sorting floor something
  heavier (Robot minds and bodies). Every power stage is louder than
  the last: the generator wakes the hall.

**Budget.** Everything below the yard is in the `Below` group (drawn
only from inside or near the ramp and the hatch); at most 8 unshadowed
lamps, range-hidden; the bins and stacks are instanced props or merged
boxes. Checked with Performance's gauges once built.

**Built in stages:** per-room floors and stairs between levels; the
layout and its placement; power in stages; the flooded level; the bins
and the kept; robots; the way in. Each with its tests and shots.
