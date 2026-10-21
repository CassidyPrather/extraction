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
