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

## The Depot

The place the convoys go. The lore says each Gate is a depot, the
machines keep everything they take "pending review", and only the three
keys end it. The Depot makes that physical: the valleys' things, sorted
and stacked under the Gate, your own lost kit among them.

**Where.** `depot_marrow`, placed at (-230, -234) rot 0 on Marrow Creek,
in the Gate's danger zone 29 m from the Gate itself (scouted for rock
cover and a kiosk clear of buildings: `SCOUT=marrow_creek:depot_marrow:
-230:-230:60`). Four levels stepping down, 7 m to 19 m under the yard,
about 60 x 120 m (plan: `docs/underground/depot_marrow.svg`).

**The way in** is the intake ramp at the entrance kiosk. It is open: the
Depot is a destination from rank 4 (Progression's deep jobs draw it as
any site with a layout, the first after Hana's letter a fixed one), and
Lev's line points there (Story). The shutter that opens behind a haul
convoy after a Surge stays a proposal.

**The way out** is the service stair from the pump room, 23 m up to a
hatch north of the Depot at (-246, -119): only the last power stage
opens it (`portal1` on stage 3), and raiders never leave by a held exit.

| Level | Floor | Rooms | Power |
| - | - | - | - |
| 1 Intake | -7 m | `intake_bay` (the ramp's foot), `guard_post` | stage 1: the guard post's breaker opens the haul ramp's door into the hall and lights the hall |
| 2 Sorting | -11 m | `sorting_hall` (24 x 16, 6 m high: rows of bins), `tag_office` (a staff stair round from the guard post) | stage 2: the office's board opens the stacks and lights them |
| 3 The kept | -15 m | `kept_stacks` (shelves both walls, crates down the middle), `bay_k14` (the Mareks' crates and Jan Holm's) | - |
| 4 Cistern | -19 m | `pump_gallery` (flooded 1.2 m: swim), `pump_room` (the best shelf) | stage 3: the pump room's breaker opens the hatch |

Each switch is dead until the one before ("Nothing upstream."), and
each is loud. Corridors between levels are stepped (a tread per half
metre, 0.19 m); where one turns, its treads stay one apart across the
corner.

**What's there.**
- *Your lost kit.* The latest 16 of `Memorial::lost`, by catalog name,
  lie on the hall's crates and pallets (`LostKit`), for the taking
  back (`the_lost_kit_lies_in_the_sorting_hall`).
- *The kept.* Bay K-14's crates are closed props: Hana's long crate
  with her basket, the small one for the dog, Jan Holm's. Story places
  their tags, the tag office terminal and the review docket as scenes.
  Progression puts the `intake_ledger` in the bay while its job is on.
- *Danger.* Crawlers on the three dry levels (`rounds`: the intake's
  round, two in the hall, one in the stacks), shut in behind the power
  until it lets them out; none in the cistern (too deep). Something
  heavier in the hall waits on Robot minds.

**Hooks.** `DepotEntered { player }` each time a player goes down into
it; `CarriedOutOfDepot { player, item }` for each thing they come back
up with that they didn't take in (`the_depot_knows_who_came_and_what_
they_took`).

**Tested.** The layout check (stages reached in order, steps, floods);
the headset throws its switches out of turn (nothing) and then in turn;
a bot walks to each switch and throws it, takes the deepest loot (over
the swim) and leaves by the hatch; daylight and crawler tests take every
site on a map.

**Budget.** Everything below is in the `Below` group, drawn only from
inside or near the ramp and the hatch; five lamps (three on the power),
range-hidden.

**Shots** (`docs/shots/underground/d_*.jpg`, torch on, power off): the
ramp kiosk (`d_ramp`; still the shelter's kiosk, a wider shuttered one
is to come), the intake down the haul ramp (`d_intake`), the dark
sorting hall (`d_hall`), the stacks and bay K-14 (`d_stacks`,
`d_bay`), the flooded cistern (`d_cistern`), the service stair up
(`d_stair`) and the hatch (`d_hatch`).
