# Hands and body: where I stopped

Resumed 4 November. Branch `claude/polish-hands`, main 3a1602ef merged
(e880868a). Shots of the work are in `docs/shots/hands/` (its README
lists them).

## Landed this round

- Gloves on what they hold: support, cup and handle poses by rule from each
  gun's layout; the index fitted to each gun's trigger
  (`render::props::gun_index_curl`), held to a healthy pull by
  `a_pulled_index_meets_each_guns_trigger` (curl 0.3 to 0.8, the Nagant's
  measured floor 0.25 for its real long reach).
- The magazine catch: the release held, the finger that reaches the catch
  works it (the AK's paddle by the index, the PM's heel by the cupping
  thumb, on its face, not inside the gun).
- Body slots fitted by bounds; a refused stow stays in the hand.
- The truck: static parked; riders and loose kit in the bed go with it;
  giving up with you aboard extracts you.
- Items lying still go to sleep (`items::rest`).
- The ground holds a body up whatever leans on it (`player::under_the_ground`).
- `--camera X,0,Z` stands its eye over the floor the player lands on.
- Flat reload with no spare (review of Onboarding's 2c16dce7): the only
  magazine never leaves the well (1cc661a5); a clearing still strips it.
- Gunsmith's crossbow is on main; the hands follow it by rule
  (`docs/shots/hands/32_crossbow_redraw.jpg`).
- `perf::gauge_items_fall_asleep` green on main: 0 items awake on every map.
- Things held free (food, tools, valuables, ammo, loose magazines) are held
  by the glove, not a fist wherever the hand took them (`render::hold`):
  the open hand pressed onto the thing's box, each finger closed to a
  touch, the thumb aimed onto it (raised to lie on a flat side), the
  glove turned only where the hand's turn can't close on it. Held by
  `every_free_held_kind_is_held_by_the_glove` over every such kind in the
  table, 240 ways each (sink at most 4 mm, thumb and two fingers within
  3 mm, palm on it or thumb against a finger; a small thing gripped, the
  fingers closed round it); `STILL_FAILING` lists what isn't yet and can
  only shrink (empty: the vest's last hold went with the press centred on
  its nearest point). `docs/shots/hands/33_held_free.jpg` is the first
  raid's loot before and after, `34_small_gripped.jpg` the small things.

## What's next

- Help Gunsmith with the SPSh-44's trigger reach if asked: it came in at
  curl 0.83 (cramped, 60 mm); the grip point 29 mm ahead of the backstrap
  should bring it near the PM's 76 mm, or a measured ceiling if the real
  reach is short.
- Kept in the notes, not started: a general rule for parked kinematic
  bodies (doors, lifts) going static, only if a gauge shows them keeping
  piles awake.

## Working here

- Gate: `cargo fmt --all --check`, `cargo test --profile fast`,
  `cargo build --profile fast` (the test run doesn't relink the binary the
  scenarios run),
  `cargo test -p extraction_core`, the 41 scenarios
  (`--headless --scenario all`) and the 13 contracts; build with
  `CARGO_INCREMENTAL=0` (the incremental caches fill the disk).
- Fetch the textures first (`scripts/fetch_assets.py foliage materials sky
  terrain`): without them the quiet-log soak and the fragment budgets read
  wrong.
