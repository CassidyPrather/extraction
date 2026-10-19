# Polish round: brief for parallel sessions

Several Claude sessions work on this repo at once, each on its own branch
in its own container, each owning one workstream. The coordinating session
(on `claude/serene-clarke-rcgj62`) merges the branches. Read this first,
then `SKILL.md`, `docs/DESIGN.md` and `docs/PLAYTEST.md`.

## Where the game is

It's a VR extraction shooter (Bevy 0.19, OpenXR, Avian). It has had one
real headset session, on SteamVR with a Bigscreen Beyond 2 and Index
controllers. That session found broken controls, a lot of stooping, an
unclear tutorial, unreadable signs, placeholder-looking items, z-fighting
and odd trees. Most of that is fixed; `docs/PLAYTEST.md` ("What's new")
lists the fixes. The next headset session is a while off, so this round is
for polish. The playtester's standing rule: **where there's smoke there's
fire.** Fix the class of a problem and add the check that keeps it fixed,
not just the instance.

## House rules

- Commit messages end with these two lines, exactly:

  ```
  Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
  Claude-Session: <your own session URL>
  ```

  Never put model names or identifiers in code, comments, docs or commit
  messages.
- A pre-commit hook needs `cargo fmt --all`. Push only to your own branch.
  Never open a pull request.
- Commit small and often, each commit a coherent step with a message that
  says why. Push after each commit: the container can be reclaimed.
- The game must still run headless: logic in `GamePlugin`, visuals in
  `ClientPlugin` systems reacting to `Added<..>` (see `SKILL.md`).
- No `env!("CARGO_MANIFEST_DIR")` in game code; use `asset_root_path()`.
  A test guards this.
- Every new external asset gets a `CREDITS.md` row; CC0 first. Prefer
  procedural (Blender scripts in `art/`, Rust geometry) to downloads.

## Verifying

- **Before you finish:** `cargo test --workspace` and
  `cargo run --profile fast -- --headless --scenario all` (must print
  `all 7 scenarios OK`). While working, run only the tests you touched;
  the full suite takes about 7 minutes.
- **Looking at things:**
  - `cargo build --profile fast`, then
    `xvfb-run -a target/fast/extraction --map <map> --data-dir <tmp>
    --no-robots --screenshot out.png --frames 120 --camera X,Y,Z,YAW,PITCH`,
    or `scripts/shot.sh`.
  - Camera yaw: 0 looks toward -Z, 90 toward +X, 270 toward -X.
  - Look at your screenshots: a change you haven't seen isn't done.
  - Without a fresh `--data-dir` you'll get somebody's career.
  - The truck sits at the map's `TruckStop` marker, not at `Insertion`.
- **Models:** `blender -b --factory-startup --python art/check.py --
  assets/models/<dir>` must report every model clean: no z-fighting
  pairs, no floating pieces. `CHECK_DETAIL=1` prints the faces.
  - A bevelled `prism` (`_slab` in `art/common.py`) folds its sides when a
    profile corner's fillet radius is smaller than the bevel. Keep fillets
    at least as large as the bevel.
  - Never let a `prism` outline cross itself.
  - Keep decals and bands at least 4–5 mm proud of the surface under them.
  - `art/preview.py` renders a model on its own.
- **VR interactions:** test them through the headset path in
  `src/vr_harness.rs` (poses and grip/trigger values into `RigInput`),
  not by sending `HandAction`s directly. Show that each new test fails
  without your change.
- **Machine limits:** about 15 GB of RAM and a few GB of free disk. Don't
  run a release build and the full test suite at once (it has restarted a
  container). `target/` is large: clear `target/debug/incremental` if the
  disk fills.

## Who owns what

Stay inside your area. If you need a change in someone else's area, keep it
minimal (a hook, a registration line), say so in your report, and avoid
reformatting their code.

| Workstream | Owns |
| - | - |
| Onboarding | `assets/maps/training_yard.ron`; `src/camp.rs`; tutorial/onboarding systems (new modules welcome, e.g. `src/onboarding/`); signs' wording; the truck's first-run experience; first-run docs in `PLAYTEST.md` |
| Models: world | `art/props/` (new), building and furniture visuals (`src/world/buildings.rs` visual parts, `src/render/props.rs` furniture), fences, gates, signposts, the camp; new `art/` scripts for world props |
| Models: kit | `art/guns`, `art/mags`, `art/robots`, `art/vehicles`, `art/items`, and the Rust that places their nodes (`src/weapons` layouts, `src/robots/visuals.rs`) only where a model change needs it |
| Visual design | `src/world/sky.rs`, `src/render/terrain_material.rs` + `assets/shaders/`, `src/world/terrain.rs` (visual), `src/world/trees.rs`, `src/render/materials.rs`, ground cover, rocks visuals, weather, post-processing, colour grading, per-valley atmosphere |
| Map design | `assets/maps/{marrow_creek,kettle_pass,cinder_works}.ron`, map validation and lint (`crates/extraction_core/src/map*.rs`, new modules), `scripts/map_preview.py`, a map design guide in `docs/` |
| Audio | `src/audio.rs`, `src/audio_mix.rs`, `assets/audio/` (+ `sounds.json`), `scripts/audio_*.py`, `scripts/fetch_audio.py`, `docs/AUDIO.md`; haptics' feel (`src/rig/haptics.rs`) |

The coordinator handles the player body, locomotion and interaction
(`src/interact`, `src/player.rs`, `src/rig`) and merges everything.

## Handing back

When you finish (or reach a natural milestone after hours of work), push,
then send the coordinator a short report with the
`mcp__claude-code-remote__send_message` tool to session
`session_01Jh513kJMQ8gf9L4fAucvBe`:

- what changed and why;
- what you checked and how (tests, screenshot paths in the repo if you
  committed any under `docs/shots/`);
- what you'd do next;
- anything you touched outside your area.

Then keep going on the next most valuable thing in your workstream, unless
the coordinator tells you otherwise.
