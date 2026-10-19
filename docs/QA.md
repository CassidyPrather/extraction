# QA: gauges, knobs and the tour

Screenshots catch problems only when a camera happens to point at them.
So every class of defect that has bitten this project gets a **gauge**:
a number computed from the data, with a threshold a test holds. The
screenshot **tour** and the reviewers who read it are for whatever the
gauges don't cover yet. When a reviewer finds something, the fix comes
with a gauge, so it stays found.

## The gauges

| Gauge | What it measures | Run | Holds the line at |
|---|---|---|---|
| Fighting faces (`src/qa.rs`) | Box faces in the same plane, facing the same way and overlapping, with different materials (z-fighting). A face hidden inside another piece doesn't count. | `cargo test --lib qa -- --nocapture` | 0 per prefab (overlaps under 0.005 m² are ignored) |
| Floating pieces (`src/qa.rs`) | A piece that doesn't rest on the ground or on another piece, isn't held from the side, and doesn't hang from above. | same | 0, except `anomaly_core` (it floats on purpose) |
| Foundation depth (`src/qa.rs`) | How far each model reaches below its footing, against what `map::prefab_foundation` claims. | same | the model must reach at least the claim |
| Building fit (`extraction_core::fit`) | For every padded placement on every map: ground through the floor, foundation exposed past what it covers, earth banked against a wall, water inside, and road under the floor. | `cargo test -p extraction_core fit -- --nocapture` | through ≤ 0.1 m, exposed ≤ 0.1 m past cover, banked ≤ 0.6 m, dry, roads only into halls and barns |
| Map design (`extraction_core::map_lint`) | The measurable rules of `docs/MAP_DESIGN.md` on each valley as built, on the walkers' own nav grid: everything reachable (and over ground under 25°), main routes' grade, pacing, cover, landmarks in sight and a second way, roads' grade and width, loot against danger, nothing wet or on a cliff, buildings clear of each other and the roads, anomalies off the roads, robot work sites, render load. `scripts/map-preview.sh <map>` draws the routes and findings. | `cargo test -p extraction_core map_lint -- --nocapture` | no findings on the three valleys; the Training Yard's are printed (only reach, footing and overlap hold) |
| On the rails (`fit::on_rails`) | Each boxcar's offset and angle from its rail line. | same | ≤ 0.3 m and ≤ 2°, or more than 30 m from any rail (abandoned) |
| Sprig atlas (`trees::atlas_tests`) | Spruce card rects against the atlas alpha. | `cargo test --lib sprig` | 5–45% opaque, clear of the bark strip and the stems |
| Doors and floors (`sim::indoor`) | A bot walks in through every door on every map; every floor and deck has a surface under it. | `cargo test --lib indoor` | every door, every floor |
| Bridges (`world::bridges`) | A bot walks over every bridge, along waypoints every 2 m. | `cargo test --lib bridges` | every bridge |
| Worn ground (`terrain::rain_wears_…`) | How far relief and erosion move the land, overall and under buildings. | `cargo test -p extraction_core rain_wears` | RMS over 0.15 m, nowhere over 6 m; buildings' ground moved under 0.04 m (median), 0.5 m at most |
| Rocks and cover (`rocks`, `cover`) | Counts per map and per patch, and that none lies on a road, in water or on a floor. | `cargo test -p extraction_core rocks cover` | 1,000–20,000 rocks a map; 600+ plants per 24 m patch |
| Render load (`render/stats.rs`) | Meshes, triangles and shadow casters a view draws. Also warns about LOD-ranged meshes at the world origin or without bounds. | `EXTRACTION_RENDER_STATS=1 scripts/shot.sh ...` | eyeballed against `docs/PLAYTEST.md`. The warning should never print. |
| New player's path (`camp::tests::a_new_player_…`) | First launch to the yard, skip, the valley with its camp and career, relaunch, back to the yard from camp, NEW CAREER (twice) and the yard again; each relaunch replayed in-process with its own profile folder (`--data-dir`). | `cargo test --lib a_new_player` | every step |
| Mix (`scripts/audio_audit.py --check`) | Every cue's loudness as heard (asset loudness by BS.1770, plus gain, distance falloff and muffle) against the world's levels compressed into the game's range; each signal's audible range over calm and downpour beds (a 1/3-octave masking model); true peaks; five offline scenes. See `docs/AUDIO.md`, "Checking the mix". | `python3 scripts/audio_audit.py --check` | no cue flagged: details and actions within 5 LU of the line, signals reach their ranges and stay under your gunshot, peaks under 0 dBTP (+3 for rare loud events) |
| Headset path (`scripts/xr_check.sh`) | The game against Monado's simulated headset and WMR controllers: session focus, bindings accepted, and a screenshot of the left eye (`--mirror`). | `MONADO=<prefix> scripts/xr_check.sh [map] [out.png]` | the session reaches FOCUSED, no binding is rejected, and the eye's view is eyeballed |
| Sim telemetry (`scripts/sim.sh`) | Bots raiding a map: who gets home, who dies, where they get stuck. | `scripts/sim.sh <map> N --jobs 3 --seed S` | see `docs/SIMULATION.md` |

A map test can be pointed at one map with `EXTRACTION_TEST_MAP=<map>`.

## Knobs

- **Thresholds** are named constants beside each test (`THROUGH`,
  `EXPOSED`, `BANKED` and `DRIVE_IN` in `fit.rs`, `MAY_FLOAT` in `qa.rs`,
  and the 0.005 m² area in `fighting_faces`). Tighten one and see what
  falls out.
- **Footing**: `map::prefab_foundation` (how deep a model's plinth goes)
  and `map::prefab_lift` (how far above its lowest corner a building may
  sit). `Heightmap::footing` is the single rule that the terrain settle,
  the game and the Unity export all use. Rolling stock stands on its
  bogies (`terrain::BOGIE`) and pitches to the grade.
- **Ground**: `noise.relief` and `noise.erosion` per map (1 by default;
  0 turns either off). Droplet constants are at the top of
  `Heightmap::rain`. For a hillshade of each valley,
  `DUMP=dir cargo test -p extraction_core --release dump_heights --
  --ignored` writes the heights as raw f32. Rock density is `cell` and
  the chances in `rocks::scatter_rocks`; ground cover is `cell` and the
  chances in `cover::CoverMap::patch`, and its distances (`PATCH`,
  `SPAWN`, `NEAR_FADE`, `FADE`, `KEEP_FAR`) are in `world/cover.rs`.
- **Detail**: `--detail` (0.25 to 1) thins ground cover by rank, drops
  field stones and shortens the sun's shadows. Rock LOD bands are
  `NEAR_LOD`, `FAR_LOD`, `CULL`, `STONES_NEAR` and `STONES` in
  `world/rocks.rs`. Render load at four standing views after the ground
  work and the rock LODs: 387k to 535k triangles drawn, 108k to 360k in
  the sun's shadows (most of that the forest's foliage). Before the rock
  LODs, rocks alone were 65k to 225k drawn and 156k to 247k in shadows,
  and the terrain another 140k to 160k in shadows (it no longer casts).
- **Screenshot height**: `scripts/shot.sh` with y = 0 stands you on the
  ground (eyes at 1.7 m). Any other y is absolute. Until this was the
  rule, a y of 0 put the camera 0.3 m off the ground, and ground-level
  shots looked sparse and oversized because of it.
- **Tour**: `scripts/tour.sh <map> [out] [args]` takes the game's
  arguments. Try `--weather fog` or `--weather rain`, a `--size`, or
  `--frames` to let more settle before shooting.
- **Render**: `--weather` changes the fog cull. Tree LOD distances are
  `NEAR` (impostor cut), `NEAR_SPAWN`/`NEAR_DROP` (card streaming) and
  `FAR_LOD` in `world/trees.rs`; terrain LODs are `LODS` in
  `world/terrain.rs`.

## The tour and its reviewers

`scripts/tour.sh <map>` screenshots every placement into `target/tour/<map>`:

- an outside three-quarter view;
- for anything you can enter, a view from a back corner across the
  room (the ranger tower's is from its cabin).

`tour.json` says what each file shows and where the camera stood. It is
software-rendered, so allow a few seconds per view.

Reviewing is a fan-out: one agent per map reads every image with
`tour.json` and reports defects, most severe first, each with a file, a
place in the frame and a confidence. The prompt asks for:

- odd or broken models;
- z-fighting;
- clipping (props through walls, terrain through floors);
- floating objects, and buildings hovering or buried;
- wrong-scale, black or missing textures, and inside-out faces;
- terrain trenches, spikes or holes beside buildings;
- camera-placement problems, listed separately.

Low fidelity, aliasing and fog are to be ignored. Reviewers don't edit.
Their reports are triaged: a defect a gauge could have caught gets that
gauge before it's fixed.

## What the gauges were built from

The first tour (October 2026) and the reviewers' reports turned up the
following. Each line gives the finding, its cause and fix, and the gauge
that now holds it.

- **Tree crowns floating with no trunk, map-wide.** Merged forest meshes
  sat at the world origin, and the GPU's LOD dither measures from a
  mesh's origin. Near trunks faded with the camera's distance from the
  map centre, and terrain picked its LOD the same way. Fixed by
  `render::centred`. Gauge: the render probe's warning.
- **Brown "boards" on spruces.** The sprig UV rects reached into the
  atlas's opaque bark strip. Gauge: the sprig atlas test.
- **Rain and ash falling indoors.** The particle box knew nothing of
  roofs. The nearest roofs are now passed to the shader (`rain.rs`
  `Shelters`).
- **Guard booths dug into the hillside.** A building sat on its lowest
  corner beside a road cut. Fixed with the footing lift. Gauge: building
  fit, `banked`.
- **Ground through the mast hut floor; a house in the beck; a shed on
  the track.** Placement errors. Gauge: building fit, `through`, `wet`
  and `road`.
- **Boxcars buried in embankments and off their rails.** Gauge: on the
  rails.
- **Flat brown beams and furniture.** `Mat::Timber` had no texture.
- **Doors invisible from inside.** A plank door in a plank wall, with no
  jambs.
- **Bare halls, the bus as a box, placeholder sawmill and boxcar.**
  Models rebuilt. The geometry gauges keep the new pieces honest.

Since that pass, the robot gate has been rebuilt and the anomaly's
ground glow is a disc draped over the terrain. Still open:

- (Fixed since: the tour's cameras stand clear of trees and rocks and
  keep a clear sight of what they look at.)

Then the ground itself: the terrain texture visibly repeated, and the
land between buildings read as one smooth sheet. The texture is now
hex-tiled, and the ground has natural relief, erosion, rocks and ground
cover (see DESIGN, "Maps"). Gauges: worn ground; rocks and cover.

The new ground was checked three ways. A ground tour (`g###_*` views:
boulders, gullies, forest floor, verges, banks, pads) reviewed per
valley found rocks seated and cover clear of roads, floors and water;
it also found steep banks rendered as a glossy grey sheet (the rock
scan's gloss caught the sky), now matt, toned and cut with rills; wet
grass and boulders glinting the same way (only roads and mud go glossy
now); Cinder Works' ground still green (the terrain now knows a valley
is burnt and goes to ash); grass planted past the map's edge, hanging
in the air (now held to the land, with a test); and building pads cut
into hills meeting them in a straight crease (now a rounded shoulder). Bot
raids on all three valleys (6 each) got home more often than before
the new ground (13 of 18, against 7), and none of their stuck spots is
by a rock: they are inside houses, reaching for loot past furniture,
and on the Kettle bridge, where a route steps off the deck into the
railing (bots only; the bridge test walks its centreline).
