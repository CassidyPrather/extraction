# Soak and fuzz

What breaks over long play and odd input, found before the playtester
finds it. Each finding names its fix and the test that keeps it fixed.

## Long runs

`extraction soak` plays a long run headless and takes a census every few
seconds (`src/soak/census.rs`): entities, rigid bodies and how many
dynamic ones are awake, contact pairs, items, robots (by kind, job and
what they're doing), sound voices, meshes, images and materials,
resident memory, frame time and the size of the save. At the end the gauge
skips the first fifth (the world settling in), cuts the rest in thirds,
and fails any measure whose least in the last third is still above its
most in the first, past a slack. Things that come and go (a fight, a
Surge) overlap themselves and pass; things that only pile up don't. It
also names the kinds of entity that grew (by name, or the nearest named
ancestor's).

```bash
cargo build --profile fast
target/fast/extraction soak --map marrow_creek --minutes 60          # raid after raid
target/fast/extraction soak --map kettle_pass --minutes 60 --career  # the sim's raider
target/fast/extraction soak --map marrow_creek --minutes 20 --client # with visuals and sound
```

| Flag | Meaning |
| - | - |
| `--minutes N` | Game minutes to play (90 frames a second, as fast as it goes: about 1.5x real time headless, 0.3x with `--client`). |
| `--raid-minutes R` | Without `--career`: minutes from stepping out to the Surge (default 8). |
| `--career` | The sim's raiding bot plays (`src/sim/raider.rs`), raid after raid in one process: kit up, loot, fight, home or down, a fresh kit, again. Its telemetry goes beside the census file. |
| `--client` | The whole client too (`ClientPlugin` and sound) with no window and no GPU: meshes, materials and voices pile up here as they would in the headset. |
| `--every S` | Seconds between censuses (default 20). |
| `--seed S`, `--map M`, `--out FILE` | As usual; the census goes to `target/soak/<map>-<seed>.jsonl`, a JSON line each. |

Without `--career` the driver (`soak::drive::Raids`) walks the player out
of the pull-in and leaves them in the Zone with the Surge's clock wound
on. The director's robots come, the weather turns, the Surge (or a robot)
puts them down; they wake in the truck, the Zone resets and out they go
again. It exits 1 if the gauge finds a leak. In the suite,
`soak::drive::tests::each_zone_reset_puts_the_world_back` plays three
such cycles and holds each reset to the size of the one before.

### What the long runs found

- **Robot wrecks stayed all session.** A robot brought down kept its root
  entity, parts, colliders and meshes until the game was closed: only the
  heavies' wrecks were swept by the Surge. The Surge now sweeps every
  wreck (`robots::wreck::clear_wrecks`; test
  `soak::tests::the_surge_sweeps_wrecks_away`). Within a cycle wrecks are
  salvage and stay; a career run on Kettle Pass had 18 by half an hour.
- **Maps keyed by entities that never forgot one.** An audit of every
  map or set keyed by `Entity` that lives across frames found a dozen
  never pruned: robot voices' per-robot state, call-out and line
  cooldowns, wreck loot timers, door, grapnel and camera drone states in
  sound and robot hearing (a broken door comes back as a new entity after
  the Surge), held-gun buttons, the bayonet, the injector trigger, the
  onboarding's dressings, and the haptics' in-hand map (food eaten from the
  hand never sends `Released`). Each now forgets what's gone. Gauge:
  `soak::tests::entity_keyed_locals_forget_what_is_gone` fails on any
  system `Local` map or set keyed by `Entity` that never prunes, unless
  it's listed as bounded (keyed by hand, by building) with why.
- **With no sound device, every sound stayed forever.** A one-shot
  despawns itself when it finishes, but with no output device (none
  plugged in, or the headset's audio gone mid-session) nothing finishes:
  the client run had 11,000 live voices after four minutes and 26,000
  after seven, a few dozen more each second. One-shots now retire after
  90 s (the longest sound is a minute) and at most 256 live at once, oldest
  first (`audio::retire_voices`; test
  `soak::tests::voices_that_never_finish_are_retired`).
- **The convoy isn't counted against the director's budget.** Each death
  sends a collector walker and an escort drone from the Gate whatever the
  number of robots out. They do walk home and go, but a run of quick
  deaths stacks them: the client run, dying about once a minute, had 20
  collectors walking at once. Left as is (a player dies once a raid, not
  once a minute); noted for Robot minds.
- Headless, 30 minutes of raids on Marrow Creek (11 raids) and 45 minutes
  of the raider's career (with the wreck and map fixes): no leaks, memory
  flat at 185-210 MB, frame time flat.

## Save and load

Checks: `cargo test -p extraction_core save::` (the file fuzzer, in
milliseconds) and `cargo test --lib soak::save_fuzz` (in-game, about a
minute).

- **The file fuzzer** (`save::tests::mangled_saves_never_panic_and_mend_to_sound_ones`)
  takes a career with something in every field and cuts it short at every
  7th byte, flips bytes (1,500 rounds), deletes each line in turn, swaps
  every number for poison (`NaN`, `inf`, `1e39`, `-1`, `u32::MAX` and
  past it) and nests brackets 100,000 deep. The reader never panicked.
  Whatever still parses is mended to finite numbers and writes back out
  readably.
- **Poison a parser lets through.** RON reads `NaN` and `inf`, so a save
  could carry an item at infinity, a NaN Zone clock (the Surge would
  never come) or `raids: 4294967295` (the next `+= 1` panics in a debug
  build). `Career::sanitize` / `SaveGame::sanitize` mend or drop each, and
  loading always calls them
  (`poison_values_are_mended_and_reported`). A magazine saved with more
  rounds than it holds is clamped when it loads.
- **A damaged career was silently replaced.** A file that wouldn't parse
  was renamed `.bad` and a fresh career started, with only a log line to
  say so. Now every write keeps the save before it as `career.ron.bak`. A
  damaged career is put aside, as `career.ron.bad`, or `.bad2` and up so
  that a second never buries the first. The backup is loaded instead, and
  the radio and wrist tell the player
  (`a_damaged_career_goes_back_to_the_last_good_save`,
  `a_damaged_career_with_no_backup_is_kept_and_reported`).
- **A newer build's career was overwritten.** Opening an older build
  treated a newer save as damaged, then the next autosave wrote a fresh
  career over it. The loader now reads the version on its own, since a
  newer layout may not parse. A newer build's career is left alone, the
  session saves nothing, and the player is told
  (`a_newer_builds_career_is_never_overwritten`,
  `a_newer_career_is_known_by_its_version`).
- **Reading the career moved files.** Choosing the start valley read the
  career, and on a damaged one renamed it before the game had loaded it.
  Reading is now pure: only `load_save` puts a damaged file aside, and it
  does it once.
- **Things that no longer fit went to the world's origin.** On restore,
  an inserted item that didn't seat was left loose at `(0, 0, 0)`: a
  magazine whose gun changed family, a mount that's gone. So was anything
  with no stash to go into, and a second item for a body slot that was
  already taken. Each now goes with its owner (same bag) or to the
  player's feet (`a_poisoned_career_loads_whole_and_in_the_world`). Unknown
  catalog ids are skipped with a warning. Items lying nowhere (NaN, or a
  world away) are dropped and counted.
- **Awkward moments**
  (`every_item_is_saved_once_through_a_raids_awkward_moments`). Every frame
  of a scripted raid takes the world apart. The raid holds a pistol and a
  spare, pulls the magazine mid-reload, stows six things from the stash,
  drops one item and throws another hard at the ground, sits out the
  Surge, and goes down. Each frame must show:
  - every item in exactly one place, from both the item's side and the
    container's;
  - no container listing a despawned entity;
  - each gun's well and mechanism agreeing about the magazine;
  - the snapshot holding each saved item exactly once.

  At each moment the save is written and loaded into a fresh game, which
  must hold the same items by id and the same live rounds. It found one
  bug:
- **The safehouse's leftovers came back on relaunch, not after the Surge.**
  The Surge sweeps everything loose, the last people's tin under the
  Marrow Creek cabin's bed with it. The tin was laid again only when the
  cabin was first found, at startup. After a Surge in a session it was
  missing, and the next launch conjured it from nowhere (one more item
  after a save and load). The Surge reset now lays the leftovers for the
  new cycle (`safehouse::relay_leftovers`). Found by the round trip after
  going down, which ran after a Surge.
- **Saved past the Surge's coming.** The game opens at base, where the
  Surge has been and gone, so the Zone re-rolls on load. That is by
  design, and what you own comes through whole. Quitting mid-raid also
  puts you back at base with everything you carried. That is a design
  question (a save-quit escape), not a bug, and is left to the
  coordinator.
- **Relocating** (`relocating_keeps_every_item`): the career is saved
  pointed at the new valley, and the kit arrives whole.

## Chaos monkey

`src/soak/chaos.rs`: a seeded bot at the headset's controls. It writes
poses, grip, trigger and buttons into `RigInput` through the VR harness
(`vr_harness::Vr`), so what the hand finds, reach and collision are all in
play, and does random but plausible things: walks, sprints, jumps and
turns; teleports to doors, ladders, the water's edge, items or anywhere
inside the fence; grabs whatever grab point is in reach (items, doors,
ladders, a held gun's slide, by grip or trigger); waves both hands
through walls and floors; throws at 1-40 m/s, often at the floor; drops;
stows into pouches, the holster and the pack (sometimes dropping the
magazine first: holstering mid-reload); takes things back out; pulls the
pack over the shoulder; pulls triggers and presses the magazine release
and slide stop; pulls the magazine with the other hand while still
firing; brings a held magazine to the other hand's gun; sits, kneels and
crouches; flips comfort settings (seated, left-handed, one-handed,
smooth turn, climb teleport, finger grab) mid-raid; pokes the truck's
buttons; climbs ladders hand over hand and lets go halfway; spawns a few
random catalog items to play with; and fakes tracking glitches (a hand
jumping up to 60 m for a frame or two, holding something or not).

After every action it checks:

- nothing has a non-finite `Transform` or velocity;
- no loose item is off the map, more than 0.6 m under the ground (or an
  underground site's floor), or flying faster than 45 m/s;
- an item is in one place at a time: a hand's `holding` and the item's
  `Held` agree, nothing is both held and socketed or stowed, every socket
  and its occupant's `InSocket` agree, every pack's list and its items'
  `InBackpack` agree, and no socket or pack lists a despawned item;
- guns, magazines, tools and artifacts are never despawned (food and
  ammunition may be used up);
- a hand that opened lets go within 20 frames;
- the player stays over the ground, and doesn't go nowhere on six walks
  in a row.

Each finding prints the last two dozen actions. `chaos_monkey_short`
(three seeds, 30 s each, yard and Marrow Creek with robots) is in the
suite. The long run:

```bash
CHAOS_SEEDS=1-20 CHAOS_SECS=600 CHAOS_MAP=all cargo test --lib \
    chaos_monkey_long -- --ignored --nocapture
```

`CHAOS_MAP` takes a map, a comma list or `all`; `CHAOS_ROBOTS=1` turns
robots on; `CHAOS_TRACE=<entity index>` prints that entity every frame
and anything moving faster than 50 m/s. A debug build runs about real
time (a two-minute seed in 35-75 s).

### What it found

Run on every valley, seeds 1-6 for two minutes each without robots and
11-14 with them (about 40 game-minutes of monkey):

- **Thrown and dropped items left at hundreds of m/s** (up to 1,800 m/s,
  off the map) after a tracking jump or a teleport of the playspace with
  something in hand: a hand's velocity is measured in the world, so a
  jump of the hand or the body is a throw. Fixed with the release cap
  (`interact::MAX_THROW`, Physics edge cases below); the monkey's
  glitch action keeps it covered.
- **Curios spawned inside the slag heaps' hill** at Cinder Works and fell
  through it: the anomaly prefab sits on its lowest corner over a
  hilltop, 0.5 m under the ground at its centre, where its loot spot is.
  Every building loot spot is now put on or over the ground
  (`buildings::loot_spot_at`); gauge
  `every_loot_spot_is_over_the_ground` checks every spot on every map.
- **A pack listed an EMP grenade that had gone off inside it.** Nothing
  took a despawned item out of its pack's list (or its socket). Observers
  on `Despawn` now do, whatever despawned it; test
  `a_despawned_item_leaves_its_pack_and_pouch`.
- **A cleaning rod went off like a shell**, three times in ~70 runs
  (280-450 m/s): a 1 cm by 60 cm rod (`dims` 0.008 x 0.008 x 0.6, the
  collider floored at 1 cm) with almost no inertia about its length.
  Open: wants a thicker collider floor for items (2 cm) and a speed cap
  on loose items like the robots' bodies have (Physics edge cases).
- **Items lying in tunnels the underground rasters don't know**: things
  dropped in Marrow's civil-defence bunker (at (-8, 82), 6 m down) and
  the Depot's intake ramp (at (-245, -152), 21 m down) lay still on a
  floor where `underground::floor_under` says there is only rock. Anything
  that uses it to tell "in a site" from "through the ground" (the lost
  items' rescue, the monkey's own check) gets those wrong. Open, for
  Underground.
- Small items (rounds, ammunition boxes, a pistol) still go through the
  terrain now and then at 5-15 m/s, mostly dropped from a hand pushed
  into the ground; the lost items' rescue brings them back. Open:
  Physics edge cases.

## Physics edge cases

`src/soak/physics.rs` throws, heaps, buries and shoves things on Marrow
Creek and watches every tick. Its rule for tunnelling: a body's middle
never touches a surface (resting on one, it stays this side), so a
middle whose path between two ticks passes through anything static has
gone through it (`crossings`, `watch`). Final positions alone misled:
items and the player that slid along a wall into a doorway, or skipped
over a log pile, looked as if they'd gone through.

| Test | What | Holds |
| - | - | - |
| `thrown_items_never_tunnel` | every catalog item at 10, 25 and 50 m/s at 60 outside walls (upright from 0.6 to 1.6 m, clear air in front), 60 spots of open ground and 30 of water | no NaN, nothing still flying at 20 m/s after 1.5 s, at most `THROUGH_ALLOWED` (5) of ~1,600 through anything |
| `long_things_thrown_into_the_ground_stay_on_it` | rod, crowbar, rifles, knife, a round, spinning at 15 rad/s, into the ground at every angle | at most 2 of 96 through it |
| `heaps_settle` | 60 items dropped in a column on three buildings' floors and the steepest hillside near the insertion | none under the floor, nothing thrown out faster than 8 m/s (12 on the hill), at most 3 still moving after 8 s |
| `items_spawned_inside_things_are_not_launched` | items half and deep into walls, half into the ground, ten in one spot, in the truck | pushed out under `LAUNCH` (12 m/s; ten in one spot reach 10), never left under the ground |
| `the_player_cant_be_pushed_through_walls` | sprint into 24 walls for 3 s, straight and glancing (0.6 rad either way), then walk 1.5 m on into them in the room | the capsule's middle never passes through anything |
| `items_under_the_world_come_back` | an item shoved 6 m under the ground, one spawned 5 m under, one on an underground site's floor | back where it last rested; up on the ground; left where it is |
| `a_tracking_jump_doesnt_launch_what_you_let_go` | the hand's tracking jumps 3 m in a frame while holding something, then lets go | it leaves at no more than `MAX_THROW` |

### What it found

- **A tracking glitch fired what you held at 80 m/s.** A hand's velocity
  is smoothed from its pose each frame; a pose that jumps (tracking lost
  and found, the playspace moved) gave a release of 80 m/s from one 3 m
  jump. Release is capped at `interact::MAX_THROW` (25 m/s, a hard
  overarm throw; spin at 40 rad/s), and so is the velocity a grab
  starts with. (`src/interact/mod.rs`, the coordinator's.)
- **Thin things went under the ground.** A cleaning rod (8 mm) or a loose
  round thrown at the terrain slipped under its heightfield a third to a
  half of the time, even at 10 m/s, and fell for ever: under a
  heightfield there's nothing to push it back up. Swept CCD (on for
  items) didn't stop it; a thicker collider helped (24 in 48 rounds down
  to 2) but not for rods, and would float small things off the ground.
- **Small fast things went through plank walls.** One 5.45 or 7.62
  round in four thrown at a shed's or a house's wall at 50 m/s.
- **A rod struck end-on blew up.** Its inertia along its length is
  tiny: it spun up to 360 rad/s on impact and once left at 2,100 m/s.
  Items now cap their speed at 60 m/s and spin at 60 rad/s
  (`items::physical_bundle`; nothing the game does moves one faster).
- **Whatever left the world stayed gone and kept falling.** The save
  skipped items below -50 m, but the item was simulated, falling, for the
  rest of the session.

The fix for the last three is `src/items/lost.rs`, three backstops
after each physics step:

1. `no_tunnelling`: a loose item that moved more than 5 cm in a tick,
   as fast as its speed says (a teleport isn't a flight), has its
   middle's path ray-cast against static colliders; through one, it goes
   back to where it struck, off the face by its half thickness, with a
   little bounce and half its slide.
2. `ground_skin`: a moving item more than 15 cm under the terrain (and
   not down an underground site) is lifted back onto it.
3. `recover_lost_items` (4 times a second): an item more than 1 m under
   the ground and not over a site's floor (under its ceiling), or not
   finite, goes back to where it last lay at rest, or onto the ground
   over where it is.

Throws through something: 47 of ~1,600 before, 4 after.

### Open

- A water tower's face (Marrow Creek, about (231, 21, -194)): a round and
  an artifact shard thrown at it end up behind it. Not looked into: the
  tower's colliders are Models: world's.
- A sliver (an old photo, a letter) thrown at the ground at 25-50 m/s
  now and then passes the terrain's surface for a tick before
  `ground_skin` has it back up; a cleaning rod thrown end-on, spinning,
  once in a hundred.
- Ten items spawned in exactly one spot fly apart at up to 10 m/s
  (Avian pushes each overlapping pair apart at up to 4 m/s). Loot spreads
  what it spawns, so the game doesn't do this.
- Doors (kinematic) aren't in `no_tunnelling`'s static set; nothing
  showed going through one.
