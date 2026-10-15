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
| On the rails (`fit::on_rails`) | Each boxcar's offset and angle from its rail line. | same | ≤ 0.3 m and ≤ 2°, or more than 30 m from any rail (abandoned) |
| Sprig atlas (`trees::atlas_tests`) | Spruce card rects against the atlas alpha. | `cargo test --lib sprig` | 5–45% opaque, clear of the bark strip and the stems |
| Doors and floors (`sim::indoor`) | A bot walks in through every door on every map; every floor and deck has a surface under it. | `cargo test --lib indoor` | every door, every floor |
| Bridges (`world::bridges`) | A bot walks over every bridge, along waypoints every 2 m. | `cargo test --lib bridges` | every bridge |
| Render load (`render/stats.rs`) | Meshes, triangles and shadow casters a view draws. Also warns about LOD-ranged meshes at the world origin or without bounds. | `EXTRACTION_RENDER_STATS=1 scripts/shot.sh ...` | eyeballed against `docs/PLAYTEST.md`. The warning should never print. |
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

Open items from that pass:

- The robot gate is still a plain white arch.
- The anomaly's ground glow is a hard square.
- The tour's outside camera can stand inside a tree.
