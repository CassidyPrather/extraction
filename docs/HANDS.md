# Hands and body: where I stopped

Paused 2 November at the coordinator's word. Branch `claude/polish-hands`;
everything handed back is on main (last: 4a3c7175, main d600a153). Shots
of the work are in `docs/shots/hands/` (its README lists them).

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

## What's next

- Merge main when Gunsmith's crossbow (ef8141d) lands: the hands follow it
  by rule (`docs/shots/hands/32_crossbow_redraw.jpg`); nothing to change.
- Review Onboarding's flat-reload fix (2c16dce7) when it's on main; the VR
  release is already pinned (the dropped magazine is kept, rounds and all).
- Help Gunsmith with the SPSh-44's trigger reach if asked: it came in at
  curl 0.83 (cramped, 60 mm); the grip point 29 mm ahead of the backstrap
  should bring it near the PM's 76 mm, or a measured ceiling if the real
  reach is short.
- Kept in the notes, not started: a general rule for parked kinematic
  bodies (doors, lifts) going static, only if a gauge shows them keeping
  piles awake.

## Working here

- Gate: `cargo fmt --all --check`, `cargo test --profile fast`,
  `cargo test -p extraction_core`, the 41 scenarios
  (`--headless --scenario all`) and the 13 contracts; build with
  `CARGO_INCREMENTAL=0` (the incremental caches fill the disk).
- Fetch the textures first (`scripts/fetch_assets.py foliage materials sky
  terrain`): without them the quiet-log soak and the fragment budgets read
  wrong.
