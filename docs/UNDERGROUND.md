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
