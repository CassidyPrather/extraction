# Underground sites

One underground site per valley: dark, tense, several rooms, explored by
torchlight, with the best loot behind the most risk.

| Valley | Site (`prefab`) | What it is | Rooms | Ways out |
| - | - | - | - | - |
| Marrow Creek | `bunker_marrow` | Civil-defence shelter under the village: blast door, gas lock, decontamination, a spine corridor, dormitory, mess, filter plant, generator, command post, armoury, medical post, escape tunnel | 11 | Entrance stair; escape stair 60 m north |
| Kettle Pass | `mine_kettle` | Mine gallery behind the mine office: shaft station, lamp room, main drive, crosscuts, a tall stope, explosives magazine, sump, refuge chamber, raise | 10 | Entrance incline; the raise's stair |
| Cinder Works | `tunnel_cinder` | Service tunnel and pump room under the factory hall: stair hall, two tunnels, pump room, valve chamber, switch room, store, cable vault, workshop, overflow chamber | 10 | Entrance stair; a stair south of the hall |
| Sallow Reservoir | `gallery_sallow` | The dam's inspection gallery: a landing under the toe, 44 m of gallery under the crest (seepage ankle deep), plumb-line chamber, weir chamber, gauge room, a passage under the bed to the draw-off tower | 6 | Entrance by the valve house; a stair up beside the tower on the bed |
| Rime Fell | `adit_rime` | Coldharbour's mine in the knoll over the ore mill: the stair foot by the adit house, the lamp room (breaker), the drift, a timbered stope, the powder magazine, the haulage south past a flooded sump to the ore bin; off the drift the shaft station and the winze's cage, down 12 m to the deep level (its station, the vein drive, the assay store) | 13 | Entrance beside the adit house; a stair up onto the mill's back apron |

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
  falls to 0.2%, the sun goes out and the fog's colour goes to black; the
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
  lift in Kettle's stope, a snare in the far corner of Cinder's overflow
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
  lie on the hall's crates and in its bins, filled with sacked kit to
  a reach under the rim (`LostKit`), for the taking
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

**The deep job, end to end** (`the_ledger_comes_out_of_the_depot`):
with Progression's Ledger job on the card the intake ledger lies in bay
K-14; a bot goes down the ramp, throws the three stages in turn, takes
the ledger into its pack, swims the cistern and climbs out by the hatch,
and `CarriedOutOfDepot` names the ledger. (Handing it in at the card is
Progression's own test.)

**Budget.** Everything below is in the `Below` group, drawn only from
inside or near the ramp and the hatch; five lamps (three on the power),
range-hidden.

**Shots** (`docs/shots/underground/d_*.jpg`, torch on, power off): the
ramp kiosk (`d_ramp`; still the shelter's kiosk, a wider shuttered one
is to come), the intake down the haul ramp (`d_intake`), the dark
sorting hall (`d_hall`), the stacks and bay K-14 (`d_stacks`,
`d_bay`), the flooded cistern (`d_cistern`), the service stair up
(`d_stair`) and the hatch (`d_hatch`).

## Signature sites

One more site per valley, each built on a mechanic the others don't
have. Layouts stay data (`assets/underground/*.ron`); each mechanic is
a field or two in the layout and one small system, tested headless by a
scenario that walks it end to end.

### Marrow Creek: the pump station (`pump_marrow`)

The village's sewage and storm pumping station, drowned since the
pumps stopped. **The water level is the puzzle.**

- *Levels, not floods.* A flood gets a `basin`: rooms and corridors
  that share one water level, a height (site y) rather than a depth.
  The level moves; every cell's water is `level - floor`, so a basin
  can be chest deep in its upper rooms and over your head in its sump.
- *Valves* (`valves: [(room, at, basin, to)]`): a wheel you turn (held
  grip, a few seconds) opens a gravity drain from one basin into a lower
  one, or out. A basin drains at a set rate until it reaches the valve's
  `to` level or the lower basin's. Valves are loud (Noise r20, a
  gurgle).
- *The hand pump* (`pump: (room, at, basin, rate)`): a crank you work
  (each press lifts a few litres; held, a steady rate) for the last
  basin, the sump, which has nowhere to drain to.
- *Order matters*: the inlet chamber drains into the screen room,
  which only drains once its own valve (behind the inlet's water) is
  reached; the sump needs the pump. The best loot (the operator's
  locker, a weapons cache) is in the sump, under 2.4 m of water at the
  start: swim for it (cold, slow, loud) or pump it down.
- *Wading and swimming* as on the surface and in the floods; the water
  surface follows the level (Visual design's water look).
  Sound carries over water: noise made in or over a basin carries half
  as far again (Audio).
- *Residents*: crawlers that keep to the water's edge (Robot minds):
  their round is recomputed as the level drops, along the cells just
  dry.

### Kettle Pass: the deep mine (`deep_kettle`)

Below the existing mine, the old workings: **darkness and bad air**.

- *Bad air* (`gas: [(room or link, kind)]`): firedamp pockets in the
  low headings. A pocket hisses (Audio, looped, near it) and fills its
  cells; standing in it slowly hurts and blurs. A spark ignites it: a
  gunshot fired in or into it, a torch switched on in it, a grenade.
  Ignition is an explosion through the pocket's cells (the grenades'
  blast, scaled by the pocket's size) and then the air's clear.
- *The canary* (`canary` item, Kit models): a caged detector you carry;
  it chirps and its light goes amber, then red, as gas nears, and goes
  quiet (dead) after long enough in it.
- *The mine cart* (`carts: [(rail id, at)]`): a tub on a rail
  (`rails: [[points]]`, along the corridors' legs). Push it (grab and
  walk) or ride it (grab its rim and press: you're carried, like a
  zipline grip) down the haulage; let it run into a doorway and it
  blocks it (crawlers can't pass a blocked cell).
- *Shoring* (`shoring: [(link, at)]`): timber sets that hold up a bad
  roof. Shoot one out (two hits) and the roof comes down behind it,
  rubble that blocks the corridor for good (a way closed behind you,
  or ahead of a crawler).
- *Light identity*: no emergency lamps at all; only your torch, the
  canary's glow and the gas's faint shimmer.

### Cinder Works: the furnace cellars (`cellars_cinder`)

Under the furnace hall: flues, ash pits and the charging crane.

- *Heat* (`heat: [(room or link, level)]`): hot passages hurt as a
  Burner's edge does; over a level they're impassable. *Vents*
  (`vents: [(room, at, heat zones)]`): open one (a lever) and its
  passages cool over half a minute. Burner anomalies breed here: the
  cycle's anomaly spawn favours hot cells.
- *The crane* (`crane: (rail from, rail to, hook height, pit)`): a
  travelling crane over the ash pit. A lever runs it along its rail;
  its hook carries what hangs on it (loot crates hooked by the cellar's
  workers) and you, if you grip the hook's chain (a moving rope grip).
  The pit's floor is far below and hot: the crane is the way across.
- *Light identity*: no lamps; the glow of the hot passages, dull red,
  fading as they're vented.

### Each site

- A guard behaviour that suits it (Robot minds): edge-walkers at the
  pump station; crawlers that avoid gas and are lured into it by noise
  in the mine; in the cellars, crawlers that keep to the cool side and
  wait for you at the vents.
- A story beat placed with Story, a deep-job hook for Progression
  (`underground_sites` already counts any site with a layout).
- Tests: the layout validator (basins, valves, rails, gas, heat checked
  for sense), a headless scenario per site (drain the station; cross
  the gas; vent the cellar and ride the crane), a sim smoke (bots reach
  and leave each), and Performance's budget underground.
- Entrances: proposed to Map design with the report (scouted for rock
  cover and a clear kiosk, as before).

Built in that order: the pump station first (it extends the floods
already there), then the mine, then the cellars.

### As built

| Site | Placed | Mechanic | Systems | End-to-end test |
| - | - | - | - | - |
| `pump_marrow` | Marrow (-173, 3), outfall (-173, 58) | three basins: inlet (valve in a dry gallery), screens (valve under the inlet's water), sump (hand pump only) | `underground::Levels`, `DrainControl`, `BasinDrained`, `site_water` reads live levels, basin surfaces follow | `the_pump_station_drains` |
| `deep_kettle` | Kettle (-54, 106), incline (-54, 173) | firedamp in the old heading (and its corridor) and a dip; the canary; a cart on the haulage rail; shoring between heading and refuge | `world::workings`: `Gas`, `InGas`, `GasIgnited`, `Canary`, `MineCart`, `Shoring`, `Fall`, `RoofFell` | `crossing_the_gas_with_the_canary`, `the_cart_rolls_and_the_roof_falls` |
| `cellars_cinder` | Cinder (-154, 168), exit (-178, 142) | the flue scorching till vented; the hot burner gallery (Burners); the crane over the ash pit to the far ledge | `world::furnace`: `Heat`, `VentLever`, `Vented`, `InHeat`, `Crane`, `CraneLever`; pit faces; pits and scorching cells walls to bots | `venting_the_cellar_and_riding_the_crane` |

| `gallery_sallow` | Sallow (-6, 172) turned 180, exit (-31, 102.5) | the gallery carries sound (below); a third way through the dam under cover (Fourth valley's ask); seepage ankle deep the length of the gallery | `world::echo`: `Echoed`; a room's `carries`; the layout's `basins` (a still one) | `the_gallery_carries_sound_its_length`, `a_walk_carries_and_a_creep_does_not`; scenario `gallery_echo` |
| `bunker_marrow` | Marrow (-10, 70), escape stair 60 m north | its lockdown (above): the generator started, a klaxon, and the gas lock sealed | `world::lockdown`: `Lockdown`, `KlaxonLamp`, `Klaxon`, `Sealed`; the layout's `lockdown`; `underground::SEALED` | `the_shelter_seals_and_the_tunnel_stays_open`, `a_sealed_shelter_can_be_left`; scenario `shelter_lockdown` |
| `tunnel_cinder` | Cinder (73, -2) turned 180 | its live water (above): the overflow chamber live on the vault's breaker till the switch room's isolator cuts it | `world::live`: `Live`, `Isolator`, `Arcing`, `Shocked`, `Isolated`; the layout's `live`; `Works::live` | `the_overflow_is_live_till_isolated`, `crawlers_keep_out_of_live_water`; scenario `live_water` |
| `adit_rime` | Rime Fell (160, -55) turned 180, exit (121, -2) on the mill's back apron | the winze's cage, down to the deep level (below); the one warm place on the fell (any roof stops the Cold's chill, `world::cold`); a flooded sump on the loop; the lamp room's breaker lights the drift and opens the magazine | `world::cage`: `Cage`, `CageBell`, `GateLamp`, `CageRung`, `CageArrived`, `CageCrushed`; the layout's `cage`, `floods` and `power` | `riding_the_cage_and_standing_under_it`, `nothing_walks_into_the_winze`; scenario `cage_ride` |

### One of its own: the adit's cage

Each site gets one mechanic met nowhere else. The adit's is the winze's
cage (`world::cage`, the layout's `cage`: a `pit` room for the shaft, the
room at its top beside it, the bells). The deep level (the station at
the winze's foot, the vein drive, the assay store with the best of the
adit's loot) is reached by it, or the long way: the old incline, stepped
up 12 m from the vein drive to the drift's far end, the deep level's
second way out (see "The way out under pressure").

- *Riding.* A bell pull at the shaft station, one at the foot, one in
  the cage. A landing's bell calls the cage there, or sends it from
  there; the cage's own sends it to the other stop. Whoever stands in it
  rides (12 m at 1.5 m/s).
- *The tell.* A bell rung (heard 35 m, a Noise), the cage waits 2 s
  before it moves, the landings' lamps flashing amber; a landing's lamp
  is green with the cage there and red over the open shaft. (The bell,
  the winding engine and the rope are Audio's, asked for: `CageRung`,
  `Cage`.)
- *The hurt.* The open shaft is a 12 m drop (the fall hurts). Stand in
  the shaft's foot as it comes down and it comes down on you (70, and a
  bleed) and stops there, its rope slack; its own bell takes it back up.
- *Bots.* The shaft is a pit: the raiders' routes and the crawlers'
  grid stop at its edge, top and foot (`nothing_walks_into_the_winze`);
  raiders walk to the deep level by the incline.

Shots: the cage at the shaft station, its lamp green
(`docs/shots/underground/r_cage.jpg`); up the open winze from its foot to
the cage's floor 12 m overhead (`r_winze.jpg`).

### One of its own: the gallery that carries

The dam gallery's is sound (`world::echo`, a room's `carries`): 44 m of
bare concrete under the crest carries every sound made in it its whole
length and a little over (48 m).

- *What carries.* Any noise made in it (a shot, a tin knocked over, a
  thrown bolt landing, a valve), and your steps there upright at a walk
  or faster. Crouched, or slower than 1 m/s, they don't.
- *The tell.* Your first carried step: your steps come back to you off
  the far end (told; `Echoed` for the ear, Audio's echo).
- *The hurt.* The gallery's crawler hears you from the far end and
  comes, with a fair idea where; and the gallery is the only way to the
  gauge room's breaker and the tower stair. Turned round, a thrown tin
  at one end draws it the length of the gallery while you go by the
  other.
- *Bots.* Nothing new: a raider's steps carry like yours.

Shot: the gallery's length, the crawler at its far end, as it carries
(`docs/shots/underground/s_gallery.jpg`).

### One of its own: the shelter's lockdown

The civil-defence shelter's (`bunker_marrow`) programme is still set
(`world::lockdown`, the layout's `lockdown`): start its generator and,
after the lights, it seals itself.

- *The tell.* A klaxon in the hall every 2.5 s (heard 50 m: a Noise,
  `Klaxon` for the ear), an amber lamp turning on the hall's ceiling, and
  a voice off a tape: "Attack warning. Shelter sealing in 20 seconds."
- *The hurt.* Twenty seconds on, the gas lock's two doors swing to and
  bolt for good (`PowerLocked` at stage `SEALED`; a hand on one is told
  so). The way you came in is shut, and the klaxon has brought the two
  crawlers. Out by the escape tunnel past the medical post, or chest
  deep through the drain from the filter plant to the gas lock's stair.
- *Always a way out.* The layout's check: sealed, every room still
  reaches `out` (the escape tunnel), and a way out leaves from it
  (`a_sealed_shelter_can_be_left`); in the game, a raider's route from
  the hall to the tunnel goes through no sealed door
  (`the_shelter_seals_and_the_tunnel_stays_open`); scenario
  `shelter_lockdown` starts the generator, waits it out and walks out.
- *Bots.* A raider routes round a sealed door, and leaves by a door
  that opens (`PowerLocked` doors were already so).

Shot: the hall with the klaxon sounding, its amber lamp sweeping the
wall and a crawler coming to it (`docs/shots/underground/m_klaxon.jpg`; `EXTRACTION_LOCKDOWN=1` sets a
shelter sounding from the start, for shots).

### One of its own: the service tunnel's live water

The service tunnel's (`tunnel_cinder`) overflow chamber is flooded 0.7 m
deep and lit off the cable vault's breaker, which also unbolts its door.
Its lamps hang on their cables down into the water (`world::live`, the
layout's `live`): throw the breaker and the way in opens onto live water.

- *The tell.* Blue arcing snapping over the water (`Arcing`), its hum
  (Audio's, asked for), and at its door you're told: "The water ahead
  hums, and blue light snaps over it: it's live."
- *The hurt.* Standing in it, a jolt of 22 every half second
  (`Shocked`): four or five and you're down.
- *The way.* The isolator on the switch room's wall cuts the chamber's
  circuit (`Isolator`, `Isolated`), and the water's dead; pull it again
  and it's live (with a crawler wading, say).
- *Bots.* The crawlers' grid shuts the chamber's water while it's live
  (`Works::live`, `block_works`) and opens it isolated
  (`crawlers_keep_out_of_live_water`); `the_overflow_is_live_till_isolated`;
  scenario `live_water` throws the breaker, pulls the isolator and wades in
  unhurt.

Shot: the overflow chamber's water live, cold blue under the arcing,
its Snare over it (`docs/shots/underground/c_live.jpg`; `EXTRACTION_LIVE=1` makes it live
from the start, for shots).

### The way out under pressure

From every loot spot, how long the way out takes at a walk (2.4 m/s,
wading and stairs slower, a shaft by the cage: its ride and a wait for
it), to whichever kiosk's nearest: calm, and *pressed*, with a crawler
behind in the way it came (the first corridor, room or stair out of the
spot's room on the calm way shut). `underground::ways_out`; the rule,
`no_loot_spot_is_a_long_dead_end`: a spot with no second way lies within
60 s (`DEAD_END_MAX`) of a way out. `--nocapture` prints every spot.

| Site | Spots | Deepest (calm) | Its pressed way | Dead-end spots (longest) |
|---|---|---|---|---|
| `bunker_marrow` | 13 | armoury 16.8 s | none | 5 (16.8 s) |
| `mine_kettle` | 9 | magazine 15.5 s | none | 3 (15.5 s) |
| `tunnel_cinder` | 10 | cable vault 16.2 s | none | 1 (16.2 s) |
| `depot_marrow` | 21 | kept stacks 29.5 s | 45.2 s | 0 |
| `pump_marrow` | 10 | screen room 30.6 s | 30.9 s | 4 (10.8 s) |
| `deep_kettle` | 10 | old heading 18.1 s | 20.8 s | 5 (16.7 s) |
| `cellars_cinder` | 8 | far ledge 44.8 s | none | 4 (44.8 s) |
| `gallery_sallow` | 10 | gauge room 22.7 s | none | 4 (22.7 s) |
| `adit_rime` | 13 | assay store 44.8 s | none | 7 (44.8 s) |

A dead-end spot is one in a room with the one door: a crawler in it and
there's no other way. None is over the minute. The two longest:

- *The adit's deep level* was out by the cage alone: a crawler waiting at
  the shaft station and everything below was a trap 37-45 s deep. It has
  a second way now, the old incline (`drift` to `vein_drive`, a door at
  the drift's end), stepped 12 m up 50 m of drift from the vein drive to
  the main drift's far end: the level station's pressed way is 45.6 s
  where it was none. The assay store is still one door off the vein
  drive, 45 s out. Shot: up the incline's steps from its foot, by
  torchlight (`docs/shots/underground/r_incline.jpg`).
- *The cellars' far ledge* is out only by the crane across the pit, 45 s;
  but nothing follows across it (the crawler's round keeps to the near
  side), so it's a refuge, not a trap. Left as it is.

### Worth going down: the entrances

Every site says from the surface what's down there: over each kiosk's
door (bar one with a front of its own, the Depot's) a stencilled sign,
the layout's `sign`, names the place and what to mind there ("DEEP
WORKINGS - FIREDAMP / NO NAKED LIGHTS", "DANGER - LIVE CABLES", "CAGE TO
LOWER LEVEL"), and a dim warm lamp (`DoorLamp`) shows the way down from
across the valley at night. Audio's asked for the stair breathing out
what's below. The skipped sites got a reward worth the walk: three more
loot spots each in the pump station and the deep workings, the best
deepest. Shots: `docs/shots/underground/sign_adit.jpg`,
`sign_bunker.jpg`.

Models (Kit's): the cellars' crane bridge and hook block, the chain
scaled to the drop (`docs/shots/underground/c_crane.jpg`); the cast-iron
lever box on every vent and crane lever, a vent's arm swinging down as
it opens (`c_vent.jpg`); the Depot ramp kiosk's Custodian front, from a
portal's `facade` (`d_front.jpg`). No canary model yet: the canary is
still the item's own.

Shots of the gallery (`docs/shots/underground/s_*.jpg`, torch on): down
its 44 m with the seepage underfoot and a crawler at the far end
(`s_gallery`), the gauge room under its lamp (`s_gauge`), and the exit
kiosk on the bed beside the draw-off tower (`s_tower`).

Shots of the adit (`docs/shots/underground/r_*.jpg`): the drift by
torchlight, a tub on the rail (`r_drift`), and the exit kiosk on the
mill's back apron, the snow on the knoll behind it (`r_out`).

Layout data added for them: `basins`, `drains`, `gas`, `rails`, `carts`,
`shoring`, `items`, `heat`, `vents`, `crane`, a room's `pit`, a
layout's `lamp_colour`, a round's `heavy` and `dormant`; each checked by
`check` and drawn on the plans (`docs/underground/*.svg`). Light: the
pump station's lamps cold green-white; the mine has none (the canary's
glow, the gas's flash); the cellars' only light is the heat's dull red,
fading as they're vented.
