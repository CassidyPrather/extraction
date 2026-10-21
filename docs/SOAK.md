# Soak and fuzz

What breaks over long play and odd input, found before the playtester
finds it. Each finding names its fix and the test that keeps it fixed.

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
