# Capture and showcase

Tools for footage and stills that show the game off, and for sweeps that
find what looks wrong. They live in `src/capture/` and
`scripts/capture_*`. None of them runs unless asked for on the command
line, so they cost the shipped game nothing. They all render with the
software Vulkan driver under Xvfb, so they need no GPU. That is slow:
budget about a second a frame for a valley and about 0.3 s for the
sweep's studio.

| What | Run | Out |
| - | - | - |
| Cinematic flythroughs | `scripts/capture_cinematic.sh <map>` | `target/capture/<map>/`: frames, `<shot>.mp4`, `reel.mp4`, contact sheets |
| Press-kit stills | `scripts/capture_presskit.sh [maps]` | `target/presskit/index.html` to choose from; `--publish` to `docs/shots/showcase/` |
| Trailer from a bot's raid | `scripts/capture_trailer.sh <map>` | `target/trailer/<map>/trailer.mp4`, `trailer_sheet.jpg` |
| Looks-wrong sweep | `scripts/capture_sweep.sh [filter]` | `target/sweep/report.md`, `index.html` |
| Visual diff of a merge round | `scripts/visual_diff.sh [old new]` | `target/vdiff/diff/<old>_<new>/sheet.jpg`, `index.html`, `report.json` |
| What's new gallery | `scripts/capture_new.sh` | `docs/shots/capture/new/index.html` |
| Turntables | `TURN=36 scripts/capture_sweep.sh <filter> <out>` | `<out>/<light>/<subject>.mp4`, `_strip.jpg` |
| The cheap checks (sizes, furniture, z-fighting, camera paths) | `cargo test --lib capture -- --nocapture` | test failures and tallies |

Everything is deterministic: the same seed, map, hour, weather and path
file give the same frames. Time steps a fixed amount each frame,
however slow the renderer is.

Things that have bitten captures, and what now guards against them:

- **A stale binary.** A binary built before a merge, rendering shaders
  from after it, quit 121 frames into a shot. A haze shader's uniform had
  grown a field, and wgpu's validation rejected the pipeline when a Hush
  came into view. Every capture script now warns when the binary is older
  than `src/`, `crates/` or `assets/shaders/` (`scripts/capture_common.sh`).
- **The player dying off camera.** In a still capture (cinematics, stills,
  the sweep) the player stands idle where `--camera` put them. The Surge
  kills anyone in the open in seconds, and the raid then ends at base,
  where the Surge doesn't show. Still captures keep the local player's
  health topped up (`capture::keep_alive`). A trailer's bot takes its
  chances.
- **The world answering the studio.** At night, wildlife sends moths to
  any bright lamp, the sweep's included, and spawns more as soon as they
  go. The sweep cleared them as leftovers before each subject and never
  got past the first. Moths count as the scene's (`sweep::SCENE_MESHES`)
  and can show in a night shot near the lamp. Don't merge new shaders
  into the tree under a running capture either: it loads them from
  `assets/` as it goes.
- **An hour the game didn't know.** `--hour 23` once fell through to the
  raid's own hour, so a "night" shot came out in daylight, and the
  screenshot scripts show only errors, so nothing said so. A clock hour
  now parses (dawn from 5, day from 7, dusk from 18, night from 21). An
  unknown `--hour` or `--weather` stops the game with an ERROR line.
  `scripts/capture_check_dark.sh [map] [camera]` shoots a view at hours
  12 and 23 and fails unless the night is dark: mean luminance under 0.2
  and under half the day's. Marrow's gate road reads 0.38 by day and 0.13
  at night; with the bug, it read 0.38 for both.
- **Shooting before everything was in.** A cinematic's first still came
  at about frame 61, but "everything in" (every material and pipeline
  compiled, `render::timeline::Loaded`) comes at about frame 65 under
  lavapipe. Whether a pipeline was ready depended on the wall clock, so
  one build's Marrow dusk came out warm in one run and grey-green in the
  next. Captures now wait at their first pose for `Loaded`, for at most
  `cinematic::LOAD_WAIT` frames, and the sweep's warm-up waits the same
  way. Three runs of one build are now bit-identical.
- **A building drawn where nobody can see it.** On main, Cinder Works'
  blast-furnace group sometimes drew nothing, its lamp hanging over an
  empty field. Its meshes passed every culling test. They were children
  that `dress_buildings` adds to a Building already placed, and Bevy's
  static-transform optimisation never propagated to them: their
  GlobalTransform stayed `[0,0,0]`, under the map. Whether it hit
  depended on the build (system order) and on the roll.
  `EXTRACTION_NO_STATIC_TRANSFORMS=1` switches the optimisation off in a
  capture, as a diagnostic. `EXTRACTION_PROBE_DEBUG=<prefab>` prints each
  mesh of a building with its GlobalTransform, LOD range and visibility.
  The guard is `--check-buildings 1`: after each still, every building
  within 60 m in view is hidden in turn and the view shot again
  (`<still>__without_<i>_<prefab>.png`, listed in
  `<still>.probes.json`). `scripts/check_buildings.py` fails a building
  whose hiding moved under 3% of its on-screen box by three levels or
  more. On one build across five valleys, dusk and night: the missing
  furnace scored 0.1%, and every building that was there scored at least
  5.8%, including a dark hut at night. The visual diff runs it on every
  shot and prints `BUILDINGS PASS` or `BUILDINGS MISSING`.

  It judges only what it could see. A building that would fail is
  `SKIPPED`, with the reason, never passed, when:
  - its box is a sliver run off the frame's edge;
  - something nearer blocks it: each probe raycasts from the eye through
    its footprint box (`seen` in the probes file);
  - its patch is black fog.
  Rime on 844e46d8 flagged three of those: the gate beside the guard
  booth the camera is up against, at dusk and night, and a log pile in
  black fog at night. A lost building still stands where it was placed,
  collider and all, so it is still judged. `--self-test` (run by
  `cargo test`) holds that on the missing furnace's own numbers.

  The game-side fix is `transform_guard`: each frame, everything attached
  marks every ancestor's tree, between `mark_dirty_trees` and the
  propagation, so the optimisation stays on (turning it off globally
  cost about 0.8 ms p99 a frame, up to 20 ms on catch-up frames, per
  Performance). `the_game_never_leaves_children_unplaced` holds the
  game's own app to the optimisation being off or the guard being on,
  and places a child attached to a still parent. On one binary, seed 7,
  `EXTRACTION_NO_TRANSFORM_GUARD=1` lost the furnace at dusk and night,
  and the default drew all 18 buildings checked.
- **`--camera` places the player.** That's what a screenshot wants, but a
  trailer's bot must start in the truck, as a headless sim's does. The
  trailer takes no `--camera`; it spawns its own chase camera instead.

## Textures first

Materials, foliage cards, terrain and sky are made by
`scripts/fetch_assets.py` and gitignored. Without them (or without a set
added to the manifest since) the game draws flat fallback colours: guns
with no gun metal. So a capture (`--cinematic`, `--sweep`, `--trailer`)
or a `--screenshot` checks first, against `scripts/assets_manifest.json`,
and stops with the missing files and the command that makes them
(`capture::assets`). `scripts/build_at.sh`, and so the visual diff and
the before/after rig, checks the same way with `scripts/check_textures.py`
before it builds, because an old build predates the game's check but
takes its textures from this checkout. `EXTRACTION_ALLOW_FALLBACK=1`
shoots the fallback on purpose.

## Cinematics: `--cinematic FILE`

The free camera flies each shot's spline. The spline is a centripetal
Catmull-Rom curve through its key points: it has no loops or cusps where
key points bunch up. The camera walks it at an even speed by arc length,
and speeds up and slows down on sine ramps (`ease`). Every frame is saved
as `<capture-out>/<shot>/f00000.png`, and `cine.json` lists what was shot.
Time advances exactly 1/fps per saved frame. While frames are being read
back, time is paused, so rain, foliage and the sky move at their real
speed in the footage.

```text
# scripts/capture_paths/marrow_creek.cine
shot mc_aerial dur=14 ease=0.3 weather=clearing
  at 40,~40,310  look 30,~5,180       # ~h: h metres over the ground
  at 35,~55,250  look 15,~5,110
shot mc_sawmill gen=orbit target=sawmill turns=0.3 dur=12
```

A shot takes these settings:

| Key | Meaning |
| - | - |
| `dur` | Length in seconds (default 8). |
| `ease` | Share of the time spent speeding up and slowing down at each end (default 0.25). |
| `fov` | Vertical field of view in degrees (default 55). |
| `pitch` | Down-tilt when the shot has no `look` points (default 6). |
| `hour`, `weather` | As `--hour` and `--weather`, for this shot only. |
| `settle` | Frames to settle at the first pose before recording (default 40). |
| `gen` | Generate the key points: `orbit target=T [radius height turns start look]`, `road index=I [from to height side]`, `river index=I ...`, `rise target=T [from dist low high look]`. T is a prefab (its first placement), a marker's name, or `x,z`. |

A generated shot that would fly through a tree's crown, or whose view a
trunk would block, is tried again wider, tighter and higher, and the
clearest version is kept (`cinematic::DODGES`). A shot with written key
points is flown exactly as written. The test
`capture::cinematic::tests::path_files_fly_clear_of_trees` checks every
shot in `scripts/capture_paths/` for trees. The run also logs a warning for
any shot that goes into trees.

`scripts/capture_cinematic.sh <map> [out] [game args]` takes these
environment variables: `CINE` (the path file), `ONLY` (a filter on shot
names), `PREVIEW=N` (N stills a shot instead of every frame, to check
framing in a minute or two), `FPS`, `SIZE`, `HOUR`, `WEATHER`, `SUN` and
`SEED`. After the run, `scripts/capture_encode.py` draws a contact sheet
for each shot. If ffmpeg is installed, it also encodes each shot (H.264,
yuv420p, crf 18) and joins the shots into `reel.mp4` with half-second
crossfades. Without ffmpeg you get the frames and the contact sheets.

## Press-kit stills

`scripts/capture_presskit.sh` shoots the views in
`scripts/capture_paths/<map>.stills` in each mood. The views use the
`--tour-views` format, `name x,y,z,yaw,pitch`, where `~h` means h metres
over the ground. It shoots through `--cinematic`: a `.stills` file gives
one still per line, saved as `<capture-out>/<name>.png`. Time still steps
one frame at a time. That matters for the Surge: a real-time tour takes
long enough that the Surge rages, passes and resets the Zone partway
through, so a tour's "Surge" stills mostly show a calm day.

| Mood | Settings |
| - | - |
| golden | dusk, clearing, the sun 7° up |
| night | night, clearing, by the free camera's torch (`EXTRACTION_TORCH=1`) |
| rain | a downpour by day |
| surge | the Surge raging (`--surge-in -40`), drizzle |
| day | the reference look: drizzle by day |

`scripts/capture_presskit.py` draws `target/presskit/index.html`. It sorts
each mood's stills by a rough eye: tonal range, colour, detail, and
nothing crushed to black or blown to white. Treat that as a sort order,
not a verdict, and look at the stills yourself.
`scripts/capture_presskit.py --publish marrow_creek/golden/m03_meadow ...`
copies the chosen stills to `docs/shots/showcase/` as 1600-wide JPEGs and
redraws `docs/shots/showcase/index.html`.

## Trailer: `--trailer SECONDS`

`scripts/capture_trailer.sh <map>` has one sim bot (`--sim-run`, a
`hunter` by default) play a raid, rendered. Time steps exactly as in a
headless sim: one 1/90 s fixed step a frame. Two cameras follow the bot:

- a chase camera eased in behind and above its right shoulder, pulled in
  where a wall or the ground is in the way;
- a drone's-eye view circling 18 m up and 12 m out (higher over a crown, and used only when it can see the bot).

The player has no body a camera draws: in the headset you see your gloves
and sleeves. The body's shadow comes from a proxy of capsules on a layer
only the sun sees (`render::arms::SHADOW_ONLY`). The trailer's cameras see
that layer too, so the bot shows as a figure, dressed in drab. For the
first 6 s the cameras render without saving, so the valley's detail has
streamed in before the first kept frame.

After that, the cameras render only while something is worth seeing: leaving the
truck, a robot within 30 m, a fight, an anomaly, the truck called and the
ride out, going down, and six seconds of walking every 75 s. Frames in
between cost little, and a frame is saved every 1/30 s of raid time. The
raid's own telemetry (`trailer.jsonl`, the sim format) is written next to
`trailer.json`, which lists the clips. The cut is decided while filming
(`trailer::camera_for`), and only that camera renders. A fight cuts back
and forth every 2.5 s, starting close. Company arriving is seen from above
first. Walking, the truck and the ride out are seen from above. Everything
else is close. A camera starts rendering a moment before its first saved
frame. `scripts/capture_trailer.py` trims clips to 9 s, captions them with
what's happening and when, adds a title card and an end card, and encodes
the result.

## The looks-wrong sweep: `--sweep FILTER`

Every `.glb` under `assets/models/` and every generated prefab (buildings
and underground sites) is placed alone in a studio 900 m over the valley:
a grey floor 4 mm below the subject, under the valley's own sky. Eight
cameras shoot it at once in a 4 × 2 grid: eight azimuths, at 14° and 38°
elevation alternately. This happens under three lights:

- the clearing day;
- dusk;
- night, with a lamp to one side.

A building stands at its footing, its foundation under the floor. An
underground site sits on the floor like a model; seen from outside, it
shows only its entrances and fittings, since its walls face in. The
cameras see through eight times the weather's haze. Each picture is saved
as `<light>/<subject>.png`. FILTER is `all`,
`models`, `prefabs`, or text that a subject's name must contain
(`props/bed,prefab:barn`). Set `LIGHTS=day` to shoot only the day.

The day is shot a second time with the subject hidden (`_empty`), which
tells the subject apart from the studio. Time stands still while a subject
is photographed.

**Z-fighting can't be seen in a still**, so the sweep doesn't look for
it. An earlier version shot the day twice with the near plane moved,
expecting depth rounding to flip where surfaces fight. A probe with
sheets lying exactly on a block's face showed that it doesn't: they draw
solidly in a still, and the near-plane pair differed only at edges and in
the sky. Z-fighting is found from the geometry instead (below).

`scripts/capture_sweep.py` flags:

- **missing textures**: magenta on the subject;
- **blank views**: the subject covers almost nothing in one or more views;
- **odd size**: a model's largest side against its folder's range;
- **blockouts left standing**: a model that never loaded;
- **floating pieces**: meshes whose boxes touch no other mesh and not the
  floor. The test is generous: it never flags a piece that touches
  something, but it can miss a piece that floats close to another.

It writes `report.md` (worst first) and `index.html` (every subject under
every light, flagged ones outlined). `--sweep probe` adds a test piece
that fights on purpose: a block with sheets lying on and 1 mm above its
top face.

### The checks that run as tests (`capture::checks`, `capture::fighting`)

- **Fighting faces in assembled buildings** (`fighting`): each prefab as
  the game draws it close up. That is its blockout boxes, less those its
  furniture and fittings replace, plus those models where they are placed.
  Every pair of triangles from different sources is checked: in the same
  plane (within 0.5 mm), facing the same way, overlapping by more than
  4 cm², and in different materials. An overlap doesn't count if it is
  buried in a solid box just in front of it, or lies below where anyone
  looks (`qa`'s burial depth, taken at the overlap rather than at a face's
  centre). `qa`'s gauge covers boxes against boxes; this one adds models
  against boxes and models against each other. Underground sites are
  held too. Earth lies outside their boxes, so a face there counts only
  with somewhere to stand in front of it (`floor_under`). `KNOWN` lists
  fights waiting on their owners. It's empty: Models: world and
  Underground fixed the first round. A new fight fails the test.
- **Every model against itself** (`fighting`): its nodes against each
  other, as `art/check.py` checks in Blender, here in `cargo test` too.

- **Inside its clearance**: how far each surface prefab reaches from its
  footing, as drawn, against the radius the map clears round it
  (`prefab_clearance`). Past that radius it stands in the trees and rocks
  the map put just outside. This is measured on the geometry: the
  sweep's bounding boxes overstate a roof's corners by a couple of
  metres. Ropeways are left out, since their cables run on to the next
  pylon.
- **Believable size**: every model's bounds against its folder's range
  (`checks::size_range`). This catches a model exported in centimetres, or
  a gun 4 m long.
- **Furniture where its boxes were**: a building's furniture models
  replace its blockout boxes (`PropDef::pieces`). Each group must stand on
  its boxes' floor (within 3 cm) and be within half to twice their size.
  Otherwise it floats, sinks or swamps the room once the boxes are gone.
  Outside, a model may reach deeper than its boxes (more foundation) but
  must not stop short of them, which leaves a gap under it on a slope.
  Rubble lies however it fell. Known findings are in `KNOWN`.
- **No outside images**: a model carries its own textures.
- **Held together**: every part of a model (a node's triangles in one
  material) touches another part or its lowest point's level. Otherwise
  it hangs in the air: a nail over its loop, a board beside a wall
  (`checks::loose_parts`, the sweep's floating test run on the glb).
  Animals (pose sheets), robots and the open rig are left out: they're
  kits whose parts the game places itself. `LOOSE_KNOWN` lists those
  waiting on their owners.

Item models are already held to their collider boxes by
`models::tests::every_item_model_fits_its_box`, and model materials to the
game's names by `every_model_material_is_mapped`.

## What's new: `scripts/capture_new.sh`

Shoots a gallery of what changed since the last pass into
`docs/shots/capture/new/`:

- **Turntables** of new models, by day and by night. `--sweep-turn N` puts
  one full-window camera in the sweep's studio and steps it N times round
  each subject. `scripts/capture_turntable.py` makes a looping mp4 and a
  strip of each.
- **Stills** from `scripts/capture_paths/new/<map>.stills`, by day and by
  night. Marrow's are shot with `--gallery minefields`.
- **Reels** from Robot bodies' `--gallery reel:` with `--hide-hands 1`, so
  the gloves stay out of frame. They're cropped to the robots.
- **Before/after pairs** of fixes you can see. A pair is the same view from
  the same tree with only the fix taken out, so nothing else differs. To
  build the before binary:

```bash
git worktree add /tmp/before HEAD
cd /tmp/before
git show <fix> -- <file> | git apply -R   # each fix's hunks
# Generated textures are gitignored. Without them a worktree draws its
# trees as blobs and its grass flat green:
for d in foliage materials sky terrain; do
    ln -s "$OLDPWD/assets/textures/$d" assets/textures/$d
done
ln -s "$OLDPWD/assets/.cache" assets/.cache
CARGO_TARGET_DIR=/tmp/before/target cargo build --profile fast
cd - && BEFORE_BIN=/tmp/before/target/fast/extraction scripts/capture_new.sh
```

  The binary reads its assets from its own checkout, so a reverted shader
  goes with it. The current pairs revert three fixes:
  - `08dbe5cd`: the mine drawn at its modelling offset.
  - `78a71777` and `e50ce87d`: wrecks placed level, and ivy on vehicles.
    This was done by having `Heightmap::wheels` return None and
    `prefab_is_vehicle` return false.
  - `30519ac`: the sky's fixed 8° fog band, as the sky shader drew it before.

Edit the subject lists in `scripts/capture_new.sh` and the captions in
`scripts/capture_new.py` for the next pass.

## Before and after: `scripts/before_after.py LIST`

A contact sheet of fixes you can see, each shot by the build before the
fix and the build after it, side by side. The list file names the two
builds and the shots:
`docs/playtests/2026-10-28/shots.txt` is the playtest's. Its header says
how to write a line: name, owner, what to look for, and the game's own
arguments. A fifth field gives the before build different arguments.
The owners keep their lines up to date as their fixes land.

```
scripts/before_after.py docs/playtests/2026-10-28/shots.txt      # the sheet
scripts/before_after.py LIST --only range_board                  # one shot
scripts/before_after.py LIST --try target/fast/extraction        # stage your line on your build
scripts/before_after.py LIST --check                             # read the list only
```

- **Builds:** `scripts/build_at.sh`, the same builds the visual diff uses
  (`target/vdiff/bin/<sha>/`).
- **Shots:** each is `--screenshot` after `--frames`, under Xvfb on the
  software renderer, with a fresh `--data-dir` and `--new-game`. They are
  kept in `target/before_after/<sha>/` with their logs, and shot again
  only when their arguments change.
- **Output:** `target/before_after/<before>_<after>/sheet.jpg` and
  `index.html`. A shot that didn't come out is a grey tile with why
  (timed out, killed, the panic), never left off.
- **One at a time** (`--jobs 1`) by default: three games and a build at
  once ran out of memory.
- **Pictures taken elsewhere:** `--picture FILE` in place of the arguments, for a fix the rig cannot stage. Marrow Creek's impostors were one: the software renderer's frames are too slow for the old build to give up on them.
- **Small in frame:** `--crop x0,y0,x1,y1`, in pixels, crops both
  halves on the sheet only (the walkers at 15 m).
- **In motion:** `--dolly X,Y,Z,YAW,PITCH:X,Y,Z[:SECONDS[:FPS]]` in place
  of `--camera`. The camera walks a straight line between the two points,
  looking along YAW and PITCH the whole way. Y 0 means eyes 1.7 m over the
  ground, as with `--camera`. It moves at walking pace (1.4 m/s) unless
  SECONDS is given, and saves a frame every 1/FPS s (default 10). Both
  builds shoot the walk in one launch each: the rig writes a one-shot
  `.cine` and runs `--cinematic`, which builds from before the tool have
  too. On the sheet each half shows six frames down the walk; `--crop`
  crops each frame. `index.html` plays the two walks side by side
  (`<name>_pair.mp4`, with ffmpeg). Every frame is kept in
  `target/before_after/<sha>/<name>/dolly/`. Use it for what shows only
  between frames: an LOD pop, a flicker, a crossfade (`treeline_walk`).
  The first frame settles for `--frames` (default 60) at its pose.
- **Slow frames:** the software renderer takes seconds a frame. Give a
  big valley 150 frames or so; a run has 30 minutes. A bug that needs many
  fast frames (a give-up counted in frames) won't show here.

## Visual diff: `scripts/visual_diff.sh [old_sha new_sha]`

The same cameras in every valley, shot by two builds at day, dusk, night
and fog, compared for what changed. It's meant to catch the change nobody
meant: the anomaly ring, a material breaking, a light going dark, a prop
vanishing. Run it after each merge round. With no shas it compares main's
tip with the end of the round before: the commit under main's latest
cluster of three or more merges within ten minutes.

- **Cameras.** `scripts/visual_diff/<map>.stills`, committed: an
  insertion, the tallest landmark, a furnished interior, an anomaly and an
  RV, in each of the five valleys. They're resolved from
  `scripts/visual_diff/views.cine`, which names its targets in words every
  valley answers to (`kind:Insertion`, `tallest`, `interior`). The
  `--cine-dump` flag resolves them; `scripts/visual_diff_cameras.sh`
  remakes them when a valley changes under them. Because they're written
  out, a build from before the target words still shoots from the same
  places.
- **Builds.** Each sha is built once, in a worktree at
  `target/vdiff/wt` with a shared `target/vdiff/build`. The first build
  took 17 min and the next 6. Each is kept as `target/vdiff/bin/<sha>/`
  next to a copy of its assets, which links in the generated textures
  (gitignored, so a worktree lacks them). Shots go to
  `target/vdiff/shots/<sha>/` at 960×540 with seed 7. Both are reused
  next round and pruned to the newest three, about 250 MB a sha.
- **The measure** (`scripts/visual_diff.py`) is 1 − SSIM on luminance, and
  the share of pixels whose colour moved after a light blur. A shot
  changed if structure passes 0.005 or more than 0.05% of pixels moved.
  That catches a small lamp going dark, about 0.08% of the frame. The
  same build shot twice gave identical pictures for 96 of the 100 shots.
  The other four are Rime Fell by day, its falling snow, and each gets
  twice its own measured noise from `scripts/visual_diff/noise.json`.
  Remake that file with `scripts/visual_diff.py --calibrate a b
  scripts/visual_diff/noise.json` from two runs of one build.
- **`--quick`**, for right after a gate: the landmark view of each valley
  (its sky, lights and lamps) at dusk and night, 10 shots. It takes about
  6 min of shooting a side, plus about 6.5 min to build a new sha
  incrementally. It ends, as the full sweep does, with one line: `PASS`,
  or `CHANGED` with the count and the sheet's path. Shots are kept a file
  at a time, so quick and full runs share them. `MAPS`, `MOODS` and
  `VIEWS` narrow a run to one shot for a bisect.
- **Lamps with a character** (`render::landmarks`) blink. A still's last
  two settle frames are held with the clock stopped
  (`cinematic::STILL_HOLD`), and a paused clock lights every lamp, as
  Fourth valley set it up. So a still always shows its lamps lit, and one
  build shoots the same picture every run. Builds from before this (round
  37 and earlier) still catch whatever phase their frame lands on, so a
  lamp-only change against one of them is noise.
- **The sheet** shows the 40 most changed shots, old | new | where it
  changed (red over the new, greyed), ranked by how many times over its
  threshold each is. `index.html` lists every shot's numbers.

The first round, `77ce6f57` to `823b12f3`, changed 21 of 100 shots
(`docs/shots/capture/vdiff/`):
- every anomaly view, from Anomalies taking the haze's rim ring out;
- Marrow Creek's dusk light shafts, from a warm haze to pale beams, with
  no change to the shafts' own code. This was the capture's fault, not the
  game's: stills were taken before every pipeline had compiled (see
  "Shooting before everything was in" above). The warm-up's timing only
  decided which run lost the race. `marrow_dusk_warmup_on_off.jpg` shows
  that race, not a warm-up bug.
