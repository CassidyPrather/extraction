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
- **`--camera` places the player.** That's what a screenshot wants, but a
  trailer's bot must start in the truck, as a headless sim's does. The
  trailer takes no `--camera`; it spawns its own chase camera instead.

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
