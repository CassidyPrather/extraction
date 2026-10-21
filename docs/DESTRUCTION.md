# Destruction

The world answers back. Glass breaks, wood splinters and gives way, rounds
go through what's thin, and what's broken stays broken until the Surge.
Code: `src/destruction/`, with the geometry in
`crates/extraction_core/src/fracture.rs`.

## What breaks, and how

| Thing | Breaks from | How much | Leaves |
| - | - | - | - |
| Window pane (every `window_sash` pane, four to a sash) | a round or fragment crossing it; anything thrown, swung or punched through it at 2.5 m/s or more; a robot's body against it; a grenade within 7 m that can see it | at once | teeth in the frame, shards on the floor |
| Plank door | rounds (at half their harm: they hole the boards, they don't split them), blows and blasts (full harm); a crowbar at its latch edge; a walker battering a wedged door | 260 (a Mosin's round is 90, a knife's thrust 40) | six boards' splinters; the doorway stands open |
| Supply crate (in the open, under loot) | rounds, blows, blasts, a crowbar in its lid | 110 (four pistol rounds) | slats, and a roll of its loot table spilled out on top of what was on its lid |
| Crate in a building (`Builder::crate_box`) | the same | 110, scaled by size | the same (its loot spot's table) |

Steel doors don't break, and rounds stop in them (their leaf is plate to a
round). Glass costs a round nothing to go through, so glass has no
collider: robots see through it, and rounds go on. Panes are logic
entities (`glass::Pane`) checked by sweeps:

- **Rounds** (`sweep_rounds`): each projectile's step this tick, after it's
  fired and before it flies, clipped at the first thing it would stop in
  or turn off. A round through one window may go on through the window
  opposite.
- **Movers** (`sweep_movers`): items (loose, held or thrown) and hands,
  by how far they moved since last tick.
- **Robots**: a few points up the body, as wide as the robot.
- **Blasts**: a grenade's `Exploded` event.

A pane breaks round where it was struck (`fracture::shatter`: a Voronoi
cut with its seeds crowded at the impact); border cells are cut with a
slanted line and the frame keeps the border side as a tooth. A plank
splits along its grain (`fracture::splinter`). Both are deterministic:
the same pane struck the same way breaks the same way, and a restored
pane shows the teeth it had (struck at its middle).

Building crates are taken out of their building's compound collider and
merged mesh (`wood::crate_pieces`/`crate_props`, read by
`buildings::spawn_buildings` and `dress_buildings`) and stand as their
own entities (`wood::BuildingCrate`), drawn by their `crate_slat` /
`crate_army` model. An underground site's crates stay part of the site.

## Breaking things from elsewhere (`destruction::hooks`)

- A **blow** is a `Hit` with `mass_g: 0` (a butt stroke, a blade, a
  thrown thing, a blast) on the thing's collider. Wood takes it at full
  weight. Glass needs nothing: anything moving through a pane breaks it.
- **`Smash { at, radius, damage, push }`**: something heavy through a
  place (a machine's arm): panes within reach break, wood takes the blow.
- **`Pryable` / `Pried { target, torque, by }`**: a crowbar levering at a
  seam. `torque >= needs` held for `PRY_HOLD` (0.4 s) and it gives: a
  plank door (350 N·m, at its latch edge) is wrenched off, a crate's lid
  (120 N·m) pops and it spills. Letting go starts it over.
- **`Breach { at, out }`**: a doorway-sized hole (1.1 m wide, floor to
  2 m, clamped to the wall segment) torn in the wall at `at` (within
  0.3 m of its middle), its boards or plaster flying along `out`. Planks,
  logs, timber, plaster and sheet tear; concrete and brick don't. See
  "Holes in walls" below.
- **`Cuttable` / `Cut`**: bolt cutters; nothing carries `Cuttable` yet.
- Despawning a plank door (a walker tearing it off) is breaking it: the
  splinters, the save and the Surge follow from the removal.

## Holes in walls

A building is one compound collider and a few merged meshes, so a hole
is cut in its prefab: the wall box the breach lands in becomes the boxes
round the hole (over it, and either side, added at the end so props'
piece ranges hold; `breach::split`). The collider and surfaces are
rebuilt at once (`buildings::body`); the client drops the building's
merged meshes and builds them again (`buildings::redress`;
`dress_buildings` cuts the holes too). Holes are kept by their centres in
the world (`breach::Breaches`), saved as `ZoneSave::breaches`, cut again
when a restored building appears, and mended by the Surge. Robots' nav
grids don't know about holes yet.

`EXTRACTION_BREACH=1` on a `--screenshot` run tears one where the view
meets a wall, twenty frames before the capture (`breach.png`).

## Broken by the years

World models' decay swaps some sashes to `window_sash_broken` (all of a
ruin's). `glass::weathered_sashes(prefab, at)` reads their decision
(`world::decay::weathered_sashes`, one bool per sash in `panes_of`
order): their panes start broken with
`Break { weathered: true }`, draw nothing (the model has the shards),
can't break again, and the Surge leaves them be.

## Would it get through?

`destruction::through::would_reach(spatial, struck, from, to, mass_g,
speed, skip, is_target)` walks a line through what's between by the
rounds' own rules, without firing: for a robot deciding whether to shoot
through a wall at what it heard (offered to Robot minds).

## Rounds through wood

Ballistics already prices a material per millimetre
(`firearm_terminal`); destruction gives the things it adds a material:

- a door's leaf is `Planks` (or `DarkMetal` for a steel door) in its
  `Surfaces`, so a 7.62 round goes through a plank door and a 9 mm stops
  in it, and hits on doors read (and sound) as wood, not soil;
- a supply crate is `Planks`.

So shooting through a door at a walker you hear behind it works
(`door_shot`, below), and so does a robot's round coming the other way.

## Debris

Pieces are real dynamic bodies (`debris::Debris`): a thin box per shard or
splinter, on the `Layer::Debris` layer, colliding with the world only (not
you, robots, items or each other), so a window's worth costs a few floor
contacts and no query finds them. They sleep when they settle and clear
after `DEBRIS_LIFE` (10 s), shrinking away in their last half second.

Budgets (gauged by `a_debris_storm_stays_in_budget_and_despawns_cleanly`
and `a_shop_front_shot_out_stays_in_budget`):

- `DEBRIS_CAP` (96) alive at once, the oldest cleared first;
- `PER_BREAK` (18) from one break, the biggest pieces; crumbs are left to
  the impact effects' glitter;
- `DebrisPool { peak, spawned, culled }` is the gauge.

No joints and no disabled bodies, ever: a piece can be despawned at any
moment (mid-flight, asleep, the tick it was thrown) without leaving the
solver an island to trip on.

## Drawing it

One mesh per building draws all its panes whole (`Mat::Glass`), and one its
broken panes' teeth in a paler, glinting shard material, rebuilt when a
pane changes. Under each broken pane a scatter of small flat shards lies
on the floor (or the ground) on the side the glass fell (`Break::fell`;
a restored pane's fell in), a few millimetres proud: what tells a window
was broken after the flying pieces are cleared away. `buildings::dress_buildings` leaves the sash models' glass
and the window glass boxes to the panes (`glass::window_glass`,
`glass::drawn_by_panes`). Flying pieces share eight unit shard shapes,
each stretched to its piece's size, and one material a kind (glass, or
the wood's), so a cap's worth of pieces batches into a few draws; the
mesh is on a child entity, so shrinking it never touches the body.

## What stays broken

`destruction::Broken` holds every broken thing by where it stood, to the
centimetre (`destruction::key`), and is saved as `ZoneSave::broken`. On a
restored Zone, panes come up broken, doors and crates come down quietly
(`wood::Quiet`: no splinters, no noise). The Surge (`persist::reset_zone`
calls `destruction::surge`) clears it: panes whole, broken doors hung again
(`buildings::spawn_door`, and the robots' doorway index patched to the new
door), crates rebuilt where they stood.

Supply crates are part of the place, not the loot: they spawn on their own
(`loot::spawn_supply_crates`) whether the Zone was rolled or restored, and
the Surge rebuilds rather than adds them.

## Glass underfoot

A step in a broken pane's glass (below it, within its width, up to 0.9 m
out on the side it fell) crunches (`glass::Crunch`, played by audio) and
is heard by robots 22 m off (`CRUNCH_HEARD`), further than a step on
boards: a window you smashed to get in is an alarm you leave yourself.

## Noise and sound

Every break is a `destruction::Broke { at, stuff, size }`. Robots hear it
as a `robots::Noise` (`noise_radius`: a pane 25 m and more for a big one, a
door 36 m); `audio::break_sounds` plays it (glass by size, wood with a
door's crash).

## Checks

- `cargo test --lib destruction` — a round through a pane, a round going
  away from one, a thrown bolt (and a slow one that doesn't break it), a
  walker shouldering into a window, rifle rounds through a plank door
  until it gives way, a pistol round stopping in one, blows breaking a
  door, a crate shot open spilling loot, a save round trip and the Surge,
  the debris storm (half despawned mid-flight, no panic, never past the
  cap), a whole valley's windows shot out at once, a building's crate
  broken and rebuilt by the Surge, a `Smash`, a crowbar's pull (too
  weak, too short, then enough), a step in broken glass, and a breach
  torn, saved, reloaded and mended (plus `breach::tests` on the split);
  through the headset path (`vr_harness`): a knife in a tracked hand
  eased into a pane (holds) and driven through it (breaks), and stabbed
  into a supply crate until it gives.
- `cargo test -p extraction_core fracture` — cells tile the pane, shards
  are convex and crowd the impact, teeth hang from the frame, boards split
  into long pieces.
- `extraction --headless --scenario door_shot` — a bot with a Mosin, a
  walker behind a shut plank door, one round: it has to go through the
  boards to hit (the scenario fails with the door's material taken away).

## Looking at it

`EXTRACTION_SHOOT=N` on a `--screenshot` run fires a rifle round from the
`--camera` along its view on each of the last N frames before the capture:

```bash
EXTRACTION_SHOOT=7 EXTRACTION_BIN=target/fast/extraction \
  scripts/shot.sh out.png -11.0,0,95.25,270,2 40 --map marrow_creek \
  --data-dir /tmp/x --no-robots      # the door_shot house's door, shot down
```

A save with broken windows for a still: `SHOT_SAVE=/tmp/s/career.ron cargo
test --lib stage_broken_house -- --ignored`, then `--save /tmp/s/career.ron`.

Shots in `docs/shots/destruction/`: `teeth.png` (a window shot out, the
teeth left in its sashes), `door_down.png` (a plank door shot down, its
splinters in the doorway), `litter.png` (glass glittering on the ground
under a window broken from inside), `breach.png` (a hole torn in a plank
house's back wall, its boards on the ground).

## Next

- Wooden fences, shutters and boarded windows, once World models has
  them as pieces of their own.
- Blunt, edge and point blows weighed differently against wood, once a
  `Hit` says which (asked of Weapon feel).
- Robots firing at a heard position through thin walls
  (`firearm_terminal` already says what a round gets through).

## Merge notes

- Weapon feel's `Hit::blow` (`Option<melee::Blow>`): add `blow: None` to
  the `Hit`s built in `destruction::tests` (they're blows: `Some(Blunt)`)
  and `hooks::smash` (`Some(Blunt)`), and weigh wood by it in
  `wood::take_hits`: rounds on doors `ROUND_ON_DOOR`, `Point` 0.25,
  `Edge` 0.5, `Blunt`/blasts 1.0.
