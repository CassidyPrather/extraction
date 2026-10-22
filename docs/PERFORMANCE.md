# Performance: the VR frame budget

The headset machine is a laptop RTX 3060 driving a Bigscreen Beyond 2:
about 2560x2560 per eye at 75-90 Hz. Its one session ran at 20-30 fps
before the render scale came in. A headset wants every frame inside
11.1 ms at 90 Hz, both eyes. This is what a view may cost, how to
measure it, and what has been cut to get there.

## Measuring

Three tools, from cheapest to most realistic:

- **The gauges** (`cargo test --lib budget`): the whole client with no
  window and no GPU, at every lookbook view of every valley, through the
  headset's eye frustum (100 degrees, square). Visibility, LOD ranges and
  the shadow cascades' culling all run on the CPU, so a view's load is
  counted exactly as the renderer would see it. They fail when a view is
  over the budget below, and print each view's load by what it belongs
  to.
- **The benchmark** (`scripts/bench.sh [maps]`): the real renderer under
  Xvfb (lavapipe, a CPU renderer), at the same views, with a second eye
  rendering beside the first as a headset does. One JSON line a view in
  `target/bench/<map>.jsonl`: draws, triangles, shadow draws and
  triangles, CPU ms (main schedule and physics, as if each frame ran one
  fixed tick), GPU ms per pass where timestamps exist, and wall time a
  frame. `python3 scripts/bench_report.py --compare BEFORE AFTER` pairs
  two runs up. Lavapipe is not a 3060: compare its times with each
  other, never with the headset's.
- **The headset's session log.** Every ten seconds: `frames:` (mean, 1%,
  worst), `cpu:` (main schedule and physics a frame) and `gpu:` (per
  pass, both eyes; the headset's device has timestamp queries).

`EXTRACTION_RENDER_STATS=1` prints the first view's load once, as
before. `EXTRACTION_PERF="msaa=2,shadows=0,cascades=1,shadow_map=1024,
shadow_far=60"` changes render settings at launch, so their cost can be
timed against each other (here with the bench, in the headset with the
`gpu:` lines) without a rebuild.

What lavapipe can't time: Bevy 0.19 has no GPU timestamps on the shadow
passes. Time them as a difference (`shadows=0`).

## Overdraw

`render::overdraw` (in the gauges) rasterises everything an eye draws on
the CPU at 192x192, depth test off, back faces culled where the material
culls them, and counts layers a pixel: the fragments shaded, which is
what fill and MSAA cost scale with. An alpha-tested card counts its
whole polygon: a discarded fragment has still been shaded. It prints the
mean, p95 and max, and the classes (tree cards, impostors, terrain,
ground cover, or what a mesh belongs to) that make up the mean. It's an
upper bound (no early depth rejection), but it ranks the classes right.

Rain and ash streaks are placed by their vertex shader in a box round
the eye, so the gauge replays them at the worst weather wherever its own
is (a downpour, or full ashfall in a burnt valley) and adds them to every
view. They come to about 0.01 layers a pixel: 2500 streaks 6 mm wide are
very little area, even counting the few right at the eye.

Open views sit at 1-2.5 layers (terrain about 0.8, the sky 1); woods
are where it piles up: the Marrow Creek spruce view was 16.3 (cards
6.95, impostors 6.45), pines 9.3, Kettle Pass's quarry 7.0.

## Budget

Per eye, per view (`render::budget::BUDGET`). Both eyes draw it all,
each with its own shadow cascades, so the frame is twice this.

| What | Per eye | Why |
| - | - | - |
| Mesh draws (main pass) | 1200 | CPU: see below |
| Triangles (main pass) | 900k | GPU: see below |
| Shadow draws (sun, all cascades) | 900 | |
| Shadow triangles | 700k | |
| Shadowed point/spot lights | 2 | Each is another view (six for a point light) |
| Overdraw, mean layers a pixel | 10 | Worst view 7.3 (Marrow's spruce); 11.5 for a while, see "Gauge margins" |
| Overdraw, 95th percentile | 35 | |
| CPU main schedule | 6 ms | |
| Physics within it | 2 ms | |

How the numbers were reached (assumptions in italics):

- **The GPU.** A laptop RTX 3060 is about 10-13 TFLOPS and 336 GB/s
  (*80-115 W parts; about 75-85% of the desktop card in games*). Its
  three GPCs set up at most about 3 triangles a clock, about 4.5 G/s at
  1.5 GHz; *real throughput with vertex shading, attribute fetch and
  culling is a quarter of that or less*, about 1 G/s, or 11M in an
  11.1 ms frame if it did nothing else. *Geometry should take no more
  than a quarter of the frame*: about 3M triangles a frame across all
  passes, so per eye 0.9M main and 0.7M into the cascades (depth-only,
  cheaper a triangle).
- **Pixels.** At render scale 1.0 (2560x2560 a eye, 13.1 Mpx a frame)
  with 4x MSAA in HDR (Rgba16Float), the colour and depth samples alone
  are 48 bytes a pixel written, before overdraw and the resolve. At
  336 GB/s an 11.1 ms frame moves at most 3.7 GB: 285 bytes a pixel.
  Foliage overdraw of 2-3 leaves little; see "MSAA" below.
- **The CPU.** *Bevy 0.19's render world is retained*: a static mesh that
  doesn't change costs little to extract or queue, but each one still
  goes through visibility (frustum and range tests) for every view
  (two eyes, two cascades each) and the GPU's per-instance culling.
  *About 1-2 microseconds a visible mesh a view on a laptop CPU*: 1200
  draws and 900 shadow draws an eye are about 4200 mesh-views, 4-8 ms of
  render-world time, which runs beside the main schedule and has to fit
  in the frame on its own.
- **Main schedule.** The render world runs in parallel with it; *the
  OpenXR frame wait and the compositor want a few ms*. 6 ms leaves
  room; physics, the dearest single system set, 2 ms of it.

## What costs what

The benchmark at the nine `m0*` Marrow Creek views, 1024x1024 an eye,
both eyes, drizzle, as software frame time (lavapipe: relative only; it
exaggerates per-sample and per-pixel work against a GPU):

| Setting (`EXTRACTION_PERF`) | Village (m02) | Spruce (m05) | Pines (m08) | Note |
| - | - | - | - | - |
| As shipped | 1690 ms | 3296 | 2412 | |
| `shadows=0` | 1474 (-13%) | 2000 (-39%) | 1702 (-29%) | The forest's card shadows are the dearest thing in a wood |
| `msaa=1` (MSAA off) | 904 (-46%) | 2242 (-32%) | 1800 (-25%) | Software MSAA is dear; a GPU's is cheaper, but 4x at 13 Mpx HDR is real bandwidth |
| `cascades=1,shadow_far=40` | 1579 (-7%) | 2886 (-12%) | 2167 (-10%) | |
| `shadow_map=1024` | 1578 (-7%) | 2688 (-18%) | 2171 (-10%) | |
| Eyes at 724x724 (half the pixels) | 1120 (-34%) | 2340 (-29%) | 1720 (-29%) | What's left is geometry and fixed cost |

`msaa=2` doesn't run under lavapipe (no 2x for the HDR target); try it
in the headset.

## Cuts

Each landed as its own commit with the numbers; per eye, before > after.

| Cut | Where | What it bought |
| - | - | - |
| Fence wire merged per 16 m cell, fades by 40-50 m, no shadow | `world::structures` | 1000-1500 fewer draws in Marrow Creek views (gate 3291 > 1831) |
| Items fade at 300x their size (15-60 m; artifacts excepted) | `budget::range_items` | Draws -15..-30%, triangles -10..-30%, village shadow triangles 128k > 56k |
| Forest cards cast the sun's shadow only within 30 m (`ShadowReach`) | `world::trees`, `budget::shadow_reach` | Wood shadow draws -30..-45%; software frame spruce -15%, pines -11% |
| Fence posts and plates fade by 90 m (`LodReach`) | `world::structures`, `budget::range_items` | Kettle Pass draws -20..-30% (gate 1491 > 1146) |
| Buildings past 120 m: one vertex-coloured stand-in of the shell | `render::lod`, `world::buildings` | Gate 851 > 700, meadow 871 > 699, spruce 927 > 788 draws |
| Sun shadow map 1024 a cascade (Bevy's default 2048) | `budget::SHADOW_MAP` | Software frame spruce -14%, pines -12%; looks the same, a touch softer |
| Near trunk chunks cast the sun's shadow only within 60 m (`ShadowReach`) | `world::trees` | Shadow triangles spruce 232k > 153k, pines 193k > 121k, edge 111k > 59k |
| Doors fade over 120-150 m, after the stand-in takes over | `budget::reach_doors` | Draws -6..-54, triangles -1..-3% |
| Forest far LOD (130 m on) one vertex-coloured mesh a chunk | `world::trees`, `render::lod` | Clear weather: draws -30..-130 a view (spruce 956 > 831) |
| Foliage cards drawn as the 8-sided outline round their leaves, not the quad | `render::cards`, `world::trees` | Card layers -23..-27%; spruce view overdraw 16.3 > 14.2, pines 9.3 > 8.3. Costs triangles: an 8-gon is 6 where a quad was 2 (spruce view 581k > 781k), cheap against ~1.7 layers of shaded fragments a pixel |
| Impostors of near trees collapse in the vertex shader, not discarded per fragment | `assets/shaders/impostor.wgsl` | Spruce view overdraw 14.2 > 10.0, summit 8.7 > 5.8, Training Yard belt 6.2 > 4.0 |
| Impostors one eye-facing quad a tree, not three crossed | `world::impostors`, `impostor.wgsl` | Impostor layers -48%; open views -15..-25% (Marrow meadow 1.97 > 1.51, Kettle gate 2.06 > 1.63), edge p95 43 > 28 |
| Forest impostors merged over 3x3 chunks | `world::trees` | About 250 fewer draws a forested view (Marrow Creek with the posts: gate 1267 > 851, spruce 1346 > 927) |

Shared tools for the next ones (in `render::budget`):

- `LodReach(m)` on an entity: every mesh under it, models that arrive
  later included, fades out by `m`.
- `ShadowReach(m)` on a mesh: it casts the sun's shadow only within `m`
  (a `VisibilityRange` can't do this: the cascades use the camera's).
- Items get a range from their catalogue size by themselves.

## Where it stands

Without the game's own textures (`scripts/fetch_assets.py` not run) the
trees are an untextured fallback with no cards and no impostors. A
playtest has run from a bare checkout, so it holds the budget too: it
draws no near crowns (their material dissolves within 74 m, where cards
would be, so they were over a million triangles unseen), switches to the
far LOD at 75 m, and a pine's or birch's far crown is one coarse blob.
Worst fallback view: 549k triangles. The gauges print which path they
measured (`foliage fetched` / `FALLBACK`), and so does the session log
(`forest: foliage ...`).

The gauges at every lookbook view of the five valleys (GPU-less client,
one eye, fetched foliage), in clear weather (the furthest you see, so
the most drawn; `BUDGET_WEATHER=drizzle` for the reference look), worst
view against the budget, on main with everything merged (wildlife, ivy,
decals, the fourth valley, the underground sites):

| | Worst | Budget | Headroom |
| - | - | - | - |
| Draws | 998 (Marrow Creek aerial) | 1200 | 17% |
| Triangles | 515k (Marrow Creek spruce) | 900k | 43% |
| Shadow draws | 509 (Training Yard aerial) | 900 | 43% |
| Shadow triangles | 422k > (small casters) under 330k | 700k | 50%+ |
| Overdraw, mean | 7.3 (Marrow Creek spruce) | 10 | 27% |

At the start the worst views were 3316 draws and 970 shadow draws.

Not yet known, and only the headset can say: what MSAA 4x costs on the
3060 (`EXTRACTION_PERF=msaa=2`/`msaa=1` against the `gpu:` lines), and
the main schedule's and render world's real CPU time (the `cpu:` line;
under lavapipe both share four cores with the renderer and read 17-31
ms, which says nothing about the laptop).

Left on the list: interiors seen from
outside within 25 m (furniture behind walls; measured small, no class
in any view's top five); the hands' 80 draws in every view (each finger
a mesh; Hands are merging them); GPU occlusion culling (Bevy 0.19 has it, but it needs a depth
prepass, a second geometry pass per eye: not worth it at these counts).

## Stereo: what the second eye costs, and multiview

Each eye is its own Bevy camera (bevy_mod_openxr gives each a view of
one layer of the 2-layer swapchain image), so everything a view does is
done twice. Measured with `EYES=1` against both (`scripts/bench.sh`,
Marrow Creek m01-m09, 128x128 so pixels don't drown the rest, software
renderer):

| | One eye | Both | The second eye |
| - | - | - | - |
| Render world, per-mesh work (`rw ms`) | 12-16 ms | 17-24 ms | +40-55%: about a third of it is per view (queue, sort, batch, the view's own passes); extraction and asset prep are shared |
| Main schedule | 19-27 ms | 22-29 ms | Within noise: visibility per camera is cheap |
| GPU (software) | 150-270 ms | 310-520 ms | x2: every pass, the sun's cascades included, runs per eye |

(Under lavapipe the CPU columns share four cores with the renderer:
read them as proportions.)

**Single-pass stereo (multiview) isn't reachable without forking
Bevy.** wgpu 29 has it (`Features::MULTIVIEW`, a render pass's
`multiview_mask`, `@builtin(view_index)`), Vulkan (bevy_mod_openxr's
default backend) has it in core, and the OpenXR swapchain is already a
2-layer array image. But Bevy 0.19 builds every render pipeline with
`multiview_mask: None` in its pipeline cache (with a "TODO: expose the
rest" beside it), every pass node (opaque, transparent, prepass, shadow,
tonemapping, upscaling, transmission) opens its pass with `None`, a
camera's `ViewTarget` is one 2D texture (and its MSAA and HDR
intermediates with it), and every PBR shader reads one `view` uniform
(`mesh_view_bindings.wgsl`). Multiview would take patches to
bevy_render, bevy_core_pipeline and bevy_pbr (view uniforms as a
2-array indexed by `view_index`, array view targets, every node's
mask), our ten custom shaders, and a one-camera mode in
bevy_mod_openxr: a fork to carry through every Bevy upgrade.

What it would buy, from the numbers above: the per-view third of the
render world's CPU (on the laptop, a few ms a frame) and the second set
of draw submissions. On the GPU, vertex work for both eyes in one pass
only where the driver shares it (not guaranteed), pixels not at all,
and the shadow cascades not at all (Bevy draws cascades per view, and
multiview passes wouldn't change that). Recommendation: not now. If
the headset's `cpu:` line shows the render world over budget, the
cheaper levers are fewer entities a view (the draw budget) first; watch
upstream Bevy for multiview, and take it when it lands rather than fork.

## CPU: a busy raid moment

`render::cpu` splits every frame by schedule (marker schedules between
the main and fixed schedules, so no tracing build is needed;
`EXTRACTION_CPU=1` logs the split every ten seconds). The gauge
`budget::tests::gauge_a_busy_moment_holds_the_cpu_budget` stages a raid's busy
moment in Marrow Creek: the whole client without a GPU, the player beside
an anomaly in a downpour, ten robots (six drones, four walkers) hunting
and shooting, one fixed tick a frame, on one thread so each system's time
is its own work and the frame's is the total. It holds each schedule to a
share of the frame (`budget::CPU_SHARE`: PostUpdate 50%, Update 40%,
physics 35%, ...) and the frame to 160 units of a fixed reference
workload (three times today's): shares survive a busy machine, wall
milliseconds don't (beside the rest of the suite they more than double).
`busy_moments` (ignored) prints all three valleys. The moment is staged
by `perf::fight`: two guns in the player's hands (a rifle and a pistol,
taken through `HandAction::Grab`, so held guns run their handling every
tick) and the ten robots.

`perf::tests::headless_ticks_hold_their_floor` stages the same fight in
the headless game (`GamePlugin` alone) and holds its tick under 60
reference units: a slowdown anywhere in the simulation fails there, where
the shares only notice one schedule outgrowing the rest. A healthy busy
tick is about 21 units (9 ms, ~107 ticks a second, dev build); with
Weapon feel's three world shape queries per held gun per tick it was 139
(61 ms, 16 a second), and in the client FixedUpdate went from 12% of the
frame to 75% (`weapons::feel::handle_guns`, 66 ms a frame). Both gauges
fail on that until its fix lands.

To name systems: `cargo test --lib system_profile --features "bevy/trace
bevy/debug" -- --ignored --nocapture` (a one-off Bevy rebuild with
per-system spans; a tracing layer totals them by system).

What it found: Bevy's `calculate_bounds` was 18 ms of a 40 ms frame,
re-bounding all ground cover (4.6M vertices) every frame because the
cover's sway material is rewritten every frame (wind, and the player's
feet for the grass to lean from), which marks every mesh using it
changed. The cover now carries `NoAutoAabb` (its bounds are exact from
the start): 40 > 20 ms, PostUpdate 68% > 37% of the frame. Today, single
threaded and dev-built, the moment is about 20-23 ms: PostUpdate 8
(visibility 2.2, transforms 1.3), Update 5, physics 3.7, game logic 2.7;
past the top few, a long tail of systems under 0.1 ms each.

## Hitches: the frames after one-off events

Averages hold; a headset feels the single long frame. `render::spikes`
sets off the raid's heavy one-off events in the busy moment above (the
same staged fight, `budget::tests::busy_scene`) and times every frame for
a second after each: a grenade (300 fragments), a walker and a drone
wrecking, each kind of Surge bloom, the Depot's residents coming out, the
truck's homecoming; and every frame of a valley loading.
`render::spikes::tests::hitches_hold_their_ceiling` holds each event's
worst frame to 3x the moment's median frame and loading's frames after
the second to 10x (ratios in the same run). It's wall-clock, so it isn't
in the default suite: on a busy machine (or a busy container) one long
frame from anything else reads as a hitch, and it failed other sessions'
suites that way. `scripts/hitches.sh` runs it per valley, in a process of
its own (Bevy's task pools are the process's), best of three per event,
and `scripts/bench.sh` calls it (`HITCHES=0` to skip). With
`SPIKE_PROFILE=1` and `--features "bevy/trace bevy/debug"` it names the
systems in each worst frame. The suite holds the hitches it found by
things that don't depend on the clock (`render::spikes::guards`): the
collider trees are optimised in place, and a walker wrecked mid-stride on
Kettle Pass's slope keeps its debris under 400 contact points a tick and
25 m/s (uncapped: 1900 points, 1400 m/s).

Marrow Creek, one thread, the moment's median 18 ms:

| event | before | after |
|---|---|---|
| grenade (300 fragments; ~170 land the first tick) | 28 ms | 30 ms |
| walker / drone wreck | 31 / 32 ms | 28 / 28 ms |
| blooms (Lift, Burner, Snare, Well, Mirror) | 29-35 ms | 24-36 ms |
| Depot residents (7 robots) | 36 ms | 33 ms |
| truck homecoming | **78 ms** | 27-31 ms |
| loading, after the first frame | **p99 147 ms, worst 978** | p99 65 ms, worst 215 (frame 2) |

Most events sit inside the moment's own spread (p99 28 ms): fragments are
cheap, wrecks and blooms add a few entities. The two hitches were:

- **The collider trees.** Avian optimises its trees as an async task and
  blocks on it at the step's end. The trees are small (Marrow Creek: 505
  static proxies, 311 dynamic, 82 kinematic), but the task queues behind
  the forest and grass chunk builds on the same pool, so the frame waits
  for those: 33-114 ms on `block_on_optimize_trees` while chunks stream,
  worst when the truck brings the player home. `GamePlugin` now optimises
  them in place (`ColliderTreeOptimization::use_async_tasks = false`):
  microseconds at this size.
- **Dressing buildings.** `dress_buildings` baked each building's ambient
  occlusion on the main thread: the Depot's 1348 pieces 440 ms, the
  bunker's 250, every house 20-50, each plank house the same bake again.
  It's baked once a prefab on the async pool now (`buildings::Bake`).

- **Flung debris.** In Kettle Pass a wrecked walker's leg, caught
  between its joints and the slope, was thrown at up to 680 m/s and
  1500 rad/s; avian's speculative contacts then reach metres ahead, and
  the leg touched 15,220 ground-triangle contact points in one tick (20
  ms of contact solving, 54 ms frame, 3.0-3.4x the median in four runs).
  Debris is now capped at 25 m/s and 40 rad/s (it's thrown at a few
  m/s): 22 ms, 1.6x. Why the joints fling it is the Robots workstream's
  to find.

Kettle Pass and Cinder Works hold the same ceilings (`SPIKE_MAP`): their
events are all 1.2-2.6x the median, loading's worst after the second
frame 79 and 102 ms.

The first frame (Startup: building the Zone, terrain and rock meshes,
6.7 s on one thread) runs before the OpenXR session is even created
(`bevy_mod_xr` runs `XrFirst` ahead of everything but the startup
schedules, and the session was only asked for in that frame's
PreUpdate), so the runtime shows its own loader. What did reach the
headset was the rest of the load: frames of 60-100 ms for about a second
while buildings dressed and the forest's impostors were taken. So the
session now starts only when the load has settled
(`rig::xr::handle_session`, `bevy_mod_xr`'s own handling switched off):
no building still waiting on its props, the forest built, every impostor
captured; or after 20 s whatever the load's doing, so a stuck stream
can't leave the player in the runtime's loader. `rig::xr::tests` hold
both. (`scripts/xr_check.sh` needs more `FRAMES` for it: the screenshot
now comes after the load, not during it.)

### First use: pipelines

Bevy builds a pipeline the first time a mesh needs one, on the async
compute pool: the render thread doesn't wait, but the mesh isn't drawn
until it's ready. On a headset that's a muzzle flash that never shows (it
lives 35 ms) or a bloom that opens a beat late. `EXTRACTION_PIPELINES=1`
logs each pipeline as it's queued and ready; `EXTRACTION_FIRSTS=<frame>`
sets off, in front of the camera, a shot, impacts on every surface, a
robot struck, a grenade, each bloom and a wreck. On lavapipe:

    D=$(mktemp -d); EXTRACTION_PIPELINES=1 EXTRACTION_FIRSTS=1000 \
    WGPU_BACKEND=vulkan xvfb-run -a -s "-screen 0 1024x1024x24" \
    ./target/fast/extraction --map marrow_creek --data-dir $D --new-game \
    --screenshot out.png --frames 2100 --camera 170,0,230,315,-2 \
    --weather downpour --size 1024x1024 --no-robots

Before the warm-up, ten pipelines were first built in the raid, each a
frame or more of the thing missing (lavapipe took 0.7-4 s apiece; a
headset's driver tens of milliseconds): the muzzle flash's additive
material at the first shot, three at the first Lift bloom (its particles,
its haze, its ground mark), one each at the first Burner and Snare, and
three more as a gun and a walker first came into view.

`render::warmup` now keeps a shelf stocked for the first 900 frames of a
session (ten seconds in the truck; `EXTRACTION_WARMUP=<frames>` to change,
`0` to leave it out): one entry for every distinct material key, mesh
layout, visibility-range dither and shadow casting among the things in
the world, plus every effect material nothing wears yet, each drawn 30 cm
in front of the eye at a millionth of its size, with bounds of its own.
It restocks every half second (a material isn't drawn until its textures
are in, and things stream in), and for the first restocks it holds a
decoy anomaly of each kind 5 km under the map, since a valley's cycle may
not hold every kind the Surge can open. Marrow Creek shelves 64 kinds of
thing; every pipeline the raid's firsts need is then built in the first
200 frames, and none at first use (the same run: a shot, impacts on every
surface, a robot struck, a grenade, each bloom, a wreck: 0 of 10).
`warmup::tests::gauge_the_shelf_stocks_and_clears` holds the shelf's mechanics
(it stocks, the decoys go, nothing's left after).

The shelf draws nothing. Each entry wears its layout's *point twin*: the
mesh's first vertex three times, one triangle with no area whatever the
vertex shader does, which still builds the pipeline. Shrinking the real
mesh wasn't enough. Rain and the storm's sheets and bolts place their
vertices in the world themselves, and their shelf copies drew full size
round the eye on every restock.

In a headset the shelf hangs from one eye and keeps it. There are two XR
cameras, and the order a query names them in can change. The shelf
used to start over each time it did, leaving its decoys and their lights
behind, and so it never finished. In Monado's simulated headset (training
yard, standing still) the point lights went from 7 to 61, and the opaque
pass from about 1 s to 9-15 s a frame on llvmpipe. Fixed, the lights go
7, 13, 7 and opaque holds at 0.4-1.3 s.
`warmup::tests::gauge_two_eyes_one_shelf` holds that, and
`EXTRACTION_COUNTS=1` logs the world's entity, light and mesh counts every
ten seconds, and what grew, for anything else that piles up.

Neither change shows: `docs/shots/performance/hitches_village.png` is the
village at frame 600 (lavapipe, clear day) with the shelf stocked and the
buildings dressed from the background bake, and the same frame with the
old in-place bake and no shelf is identical to the pixel.

## Gauge margins

The gauges here judged other sessions' work, and some sat on their limits
or flaked; one pass made them say what they measured and fail only on
something real (main at 43940f8, fetched foliage):

- **Counted once the view stops changing.** Each view used to be counted
  30 frames after the camera moved, wherever the streaming had got to:
  Marrow Creek's aerial view read 1193-1201 draws run to run (and over
  1200 in another session's suite). Now it's counted when draws and
  triangles have held still for three checks ten frames apart: 1195
  every time.
- **Which scene.** With the fetched foliage textures the forest is cards
  and impostors (what ships); without them, a lighter fallback (Marrow's
  aerial: 818 draws instead of 1195). Both are held; every message names
  the one measured, and the fallback says it isn't the shipped scene.
- **A floor, not a measure.** "A view counted almost nothing" was more
  than 50 draws, and the Training Yard's emptiest views draw 48-90 by path
  and schedule. It's 20 now.
- **Kettle Pass's quarry, overdraw 10.27 > 10.** Not the new ivy: the
  forest reshuffled with Map design's new region, and the view's camera
  now stands 1.3 m from a spruce with three more within 9 m (on the old
  map, one pine within 10 m). Tree cards are 6.1 of its layers, and card
  quads within 2 m of the eye 2.4 of those (within 1 m, 1.3). Standing
  under a spruce clump is a fair worst case, so the limit is re-based to
  11.5 (12% over it). The cut that would earn 10 back: collapse foliage
  cards within about 1.5 m of the eye in their vertex shader, as the
  impostors' near cut does (in VR those are flat sheets in your face);
  it needs each card's centre in a vertex attribute, and the tree shader
  is Visual design's. Done, with their OK: each card vertex carries its
  card's centre (`trees::ATTRIBUTE_CARD_CENTRE`, at location 8 through
  `TreeSway`'s specialize), and `tree_sway.wgsl` shrinks the card to it
  smoothly, whole from 2 m and gone by 1.2 m (`trees::card_scale`); the
  trunks, the dark core and the cards' shadows stay, and the overdraw
  gauge models it. Kettle's quarry on main: 10.27 > 8.64 (tree cards 6.07
  > 4.43). `EXTRACTION_CARD_SHRINK=0` leaves cards whole, to compare.
  `docs/shots/performance/card_shrink_k05.png` (lavapipe, left before,
  right after) also shows what the view had become: its camera stands
  inside that spruce's dark core, which is drawn from inside; the view
  wants moving a few metres, and the core seen from within is a look for
  World models and Visual design.
- **Where each valley stands** (worst share of any limit in any view):
  Marrow 1.00 (aerial, 1195 of 1200 draws), Kettle 0.89 (quarry overdraw,
  of 11.5), Training Yard 0.90 (aerial shadow draws), Cinder 0.86 (gate
  shadows). The hands are 80 draws in every view (each finger a mesh);
  merged, they'd give every view 6% back, Marrow's aerial included.
- **Forest detail around the impostor rigs.** The forest streamed near
  trees (cards, cores) round every 3D camera, the impostors' capture rigs
  included: rigs 4 km down under x = 0-480, which in the gauges' GPU-less
  harness never get their picture back and stay. Near trees stood round
  them (by horizontal distance), and the views near them counted those.
  In the game the rigs go once their picture's read back, a few frames
  into the load, so only the gauges were wrong; the forest now streams
  for the cameras that don't draw into an image. Marrow Creek: aerial
  1150 > 1003 draws, spruce 1097 > 844, summit 715 > 460, creek 502 >
  249 (on main, where wildlife and the new scatter took the aerial to
  1211, expect about 1064).
- The hitch gauge's wall-clock test is out of the default suite
  (`scripts/hitches.sh`), its finds held by counts (see "Hitches").

## Load time

The startup frame (before the headset's session starts) was 4.7-6.2 s on
this 4-core container, and longer on one thread. `render::spikes::tests::
startup_frames` (ignored, wall-clock) times it per valley on all cores.

- The terrain's shaping pass (noise, relief, the distance from every cell
  to every ridge and channel spline) was most of `Heightmap::build`, on
  one thread. Each cell's rise depends only on where it is, so rows go to
  every core (bit-identical heights, hashed): `Heightmap::build` 3.1-4.5 s
  > 1.5-2.5 s.
- The terrain chunks' meshes and the rock chunks' meshes (with their
  tangents) are built on the compute pool, each chunk on its own, and
  spawned after: the startup frame 5.4 > 4.4 s (Marrow Creek), 6.2 > 5.1
  (Kettle Pass), 5.1 > 4.1 (Cinder Works), 4.7 > 3.9 (Sallow Reservoir).
- Every cell still asked every segment of each ridge, channel and stream
  how near it was, and a creek across the valley is hundreds of segments.
  `geom::NearLine` keeps a line's segments in 8 m buckets and answers as
  `polyline_closest` does, to the bit, within the reach that matters (the
  five heightmaps hash the same): `Heightmap::build` 1.3-2.5 s > 0.5-0.8 s.

### Waiting, step by step

`render::timeline` logs `load: <what> at <s> s` once for each thing the
player waits on, from `main`'s first line: plugins building, start-up
(zone, terrain, nav, props), the first frame rendered, the preload landed,
the forest streamed, the buildings dressed, the warm-up shelf done, and on
the way out, the exit asked and the world dropped (a valley change waits
through the close too, then the next launch). Each line carries its frame
number: a step that waits on frames takes as many anywhere, one that
waits on work doesn't. Ask a playtester for those lines.

### Ready to look at

Bevy draws nothing for a thing whose material isn't prepared (its
textures still loading) or whose pipeline is still compiling. On the
software renderer the ground's and buildings' pipelines took 18 s, so a
screenshot at a fixed frame 30 had trees floating over no ground.
`timeline::WorldReady` is the one answer to "is the world in": the
buildings dressed, the forest built with its impostors taken, and the
render world clear of unprepared materials and compiling pipelines for 4
frames in a row (`Unprepared`, counted once a frame's draws are queued; a
material that just readied queues its pipelines a frame or two later).
The headset's session waits on it (or 20 s), `--screenshot` waits on its
materials and buildings after `--frames` (or 180 s, with a warning), and
"everything in" includes it.

## First contact

A hitch the player meets only the first time: a model, material,
effect, sound or building's inside first seen, or the first fight.

### The CPU

`render::first_contact` walks each valley's tour, then a fight, twice,
in a new game per run. The tour is every placement from outside and in,
the notes, signs and terminals, and the open ground. The second walk is
the steady state.

A stop counts as a first contact when its first-pass worst frame is over
1.2x its steady self (and 3 ms) in every run of three. The harness's
frames jitter by more than a small hitch, so one run decides nothing.
Each contact is named with what came in round it:

- the assets that loaded;
- what was spawned, by name;
- with `FIRST_PROFILE=1` and `--features "bevy/trace bevy/debug"`, the
  systems that took the time.

`gauge_no_first_sight_stutter` fails a valley over 1.2x in every run.

On all five valleys no stop was a first contact in every run. One was
in two runs of three: Rime Fell's compressor terminal, 80 ms first
against 42 ms steady, which didn't recur in a third. No asset loads
mid-walk (the preload's doing), and no named system over a few ms. What
was left was the harness's own jitter.

### The GPU

A pipeline is built per material, mesh layout and kind of view, the
first time something needing it is drawn. `EXTRACTION_PIPELINES=1` logs
each, and `scripts/xr_check.sh` with it shows what a headset meets. Two
causes, both fixed:

- **The eyes' kind of view.** The headset's eyes have a custom
  projection, a kind of view of its own. Warmed on the flat camera, every
  pipeline was built again as the session began: 58 on Monado's yard, in
  the player's first seconds in the headset. Now, while the session
  waits on the load, a proxy eye carries the warm-up shelf instead
  (`warmup::ProxyEye`): the eyes' projection, look and samples, drawing
  into a small image under the world. After the session, 2 are built
  where 58 were:
  - a skinned X-ray (the shelf leaves skinned meshes alone);
  - a terrain variant.

  The `--mirror` window's are still built then. It's a view of its own
  too, and only for whoever is watching.
- **What streamed in last.** The shelf restocks every 45 frames. On
  Marrow's tour, 28 pipelines were queued 8 frames after the world
  counted as in, by the restock after it. Once the world settles, the
  shelf now restocks at once, and `timeline::WorldReady` waits for that
  restock's pipelines too (`warmup::Warmed`). That gate covers the
  session, `--screenshot` and "everything in".

  After the world is in, the tour now builds none (the screenshot's own
  aside). On lavapipe, "everything in" comes at frame 65, not 41: the
  compiling moved out of play and into the load.

### A map change's wait

`gauge_map_load_seconds` times each valley's load on the CPU, from its
app being built to:

- the preload landed;
- the buildings dressed;
- the warm-up shelf's settled restock out (`WorldReady`'s GPU share
  aside).

| valley | before | after |
| - | - | - |
| Marrow Creek | 9.6 s to the buildings (frame 24); warm only at the shelf's end | 7.7 s, all in (frame 18) |
| Kettle Pass | 8.5 s to the buildings | 7.2 s |
| Cinder Works | 7.0 s | 6.6 s |
| Sallow Reservoir | 5.9 s | 6.2 s (noise) |
| Rime Fell | 7.7 s | 8.7 s |

Rime Fell is now the slowest, and the preload decides it.

- **Buildings while loading.** Buildings are dressed 3 a frame so play
  doesn't hitch. While the valley loads no one's playing, so it's 24
  (`timeline::Loaded` marks the end of the load).
- **The settled restock.** The warm-up's settled restock waits on the
  forest built and the buildings dressed, not the impostors' pictures.
- **The IO pool.** Models and textures load on the IO pool. It's now
  three quarters of the cores' worth (2 to 8), on top of the cores, so
  the compute pool keeps every thread (`crate::task_pools`). On this
  4-core container every core is busy loading anyway, so it bought little
  here (Marrow 8.6 s > 7.7 s). A laptop with more cores should gain more.

What's left is the 213 models decoding. The relaunch around it is 0.9 s
to close and about 0.2 s to start again.

## Holding the frame rate

`render::hold_rate` keeps the headset at its refresh rate by rendering
more or less of each eye. The swapchain is made at the runtime's
recommended size. Each frame, the eye cameras' viewports and the
projection layer's image rects are set to the governed size, and the
compositor scales that part to the display. No swapchain is made again.
`bevy_mod_openxr`'s projection layer is replaced by `RectLayer`, the same
layer but with the rect. Both read the frame's extracted `EyeRect`, so the
cameras and the rect always agree.

The governor (`Governor`) steps every 4 frames, from the GPU's measured
time (timestamp queries, the top-level passes summed) against the
display's period:

- **Down** when the second-worst of the last 8 frames runs past 85% of the
  period, by the square root of the overrun (time goes with the pixels),
  at most 20% a step. A single long frame doesn't count: a pipeline built
  or a hitch on the CPU isn't the pixels' doing.
- **Up** 3% a step, at most once every half second after a drop, and only
  if the 90th percentile of the last 45 frames would still fit 80% of
  the target at the larger size. The band between up and down is the
  hysteresis that keeps it from pumping.
- **Floor:** the narrowest eye that keeps must-read text at 7.5 px of
  x-height (`floor_eye_px`, about 1735 px across, from Legibility's
  constants).
- **Cap:** the runtime's recommended size.
- **Start:** the SHARPNESS size. With HOLD FRAME RATE off it stays there.
- **No GPU times** (no timestamp queries): it holds where it is.

The frame log's fourth column is the eyes' width as a share of the
runtime's recommended width. The report's `frames-summary.txt` gives its
least, median and most per valley. The session log says, every ten
seconds, the size, the GPU's time and the period it was held to.

What it buys:

| Run | Eyes | Eyes' GPU time a frame |
| - | - | - |
| Monado's simulated headset, lavapipe | 640x720 | 370-400 ms |
| Monado's simulated headset, lavapipe | 320x360 (a quarter of the pixels) | 220 ms (45% less) |

The opaque pass shrinks with the viewport. Tonemapping and MSAA
writeback still cover the whole target, but they're small.

`gauge_busy_moments_drop_the_scale_and_it_comes_back` runs the hitch
gauge's forced events (the response team, the Surge and the rest) through
the governor. Each frame's measured load is taken as the GPU's, with a
modelled GPU (a fifth fixed, the rest the pixels'). Wherever frames ran
late in a run, the scale came down within a step, and it was back within
2.3 s:

- the Surge: 0.62 > 0.57;
- the response team: 0.58 > 0.56;
- the Depot's residents: 0.62 > 0.55.

That shows the steps on the real moments' shapes. What a headset's GPU
spends is the frame log's to say.

For a simulated runtime (whose period follows the game's own pace),
`EXTRACTION_FRAME_BUDGET_MS` stands in for the period and
`EXTRACTION_EYE_FLOOR_PX` for the floor.

## Empty meshes

Bevy skips the GPU room for a mesh with no vertex data (or no indices)
but still copies it in, logging "Use-after-free: attempted to copy element
data for an unallocated key" each time the mesh is built or changed. A
mesh made from a list is empty whenever its list is: the footprints, the
machine prints and the lightning bolt, rebuilt as they change, logged
18 or more a shot. `render::sound_meshes::never_empty` gives such a mesh
one zero-area triangle, which draws nothing; build any mesh from a list
that can be empty through it. In tests, an empty mesh added or changed
fails the run (`no_empty_meshes`, in every client test), and
`no_empty_mesh_reaches_the_gpu` settles Marrow Creek in a storm.

## VRAM

`render::vram` counts what a valley holds on the GPU from the assets and
views, without a GPU:

- textures (format, size, mips, layers);
- meshes (vertex and index buffers);
- the sun's cascades (per eye) and shadowed lights' maps;
- both eyes' targets, with 4x MSAA: the multisampled HDR colour and
  depth, the resolve, the tonemapped image, and the swapchain's.

The eye targets are counted at the runtime's full size, 2980² for the
playtest's Beyond 2. Holding the frame rate (`render::hold_rate`), they're
made that big.

`gauge_every_valley_holds_its_vram` holds each valley under 4.5 GB (the
3060 laptop's 6 GB, less the driver's, the compositor's and SteamVR's
share) and prints the biggest texture folders.

| valley | total | textures | meshes | shadows | eye targets |
|---|---|---|---|---|---|
| Marrow Creek | 2.2 GB | 546 MB | 558 MB | 16 MB | 1084 MB |
| Kettle Pass | 2.2 GB | 542 | 585 | 16 | 1084 |
| Cinder Works | 2.1 GB | 532 | 465 | 16 | 1084 |
| Sallow Reservoir | 2.0 GB | 515 | 358 | 16 | 1084 |
| Rime Fell | 2.2 GB | 501 | 603 | 16 | 1084 |

At the old fixed 1800² the eye targets were 396 MB, and each valley 1.3-1.5
GB. Under half the ceiling either way: nothing to stream or cut yet.

## Small things' shadows

At the gates half the sun's shadow-pass meshes were smaller than 10 cm
(Cinder Works' gate: 275 under 5 cm, 146 more under 10, against 333
bigger): the truck's buttons and fittings, door furniture, tins and
boxes of rounds. Past a few metres such a shadow is a texel or two of the
cascade. `budget::small_shadows` gives every mesh under 12 cm (its
largest side, world size) a `ShadowReach` when its bounds arrive: under
5 cm it casts within 3 m of the eye, under 12 cm within 6 m (with
`ShadowReach`'s 2 m give either way). `EXTRACTION_SMALL_SHADOWS=0` leaves
them all casting, to compare. Sun shadow draws, before > after:

| view | before | after |
|---|---|---|
| Cinder Works gate | 852 | 412 |
| Sallow Reservoir gate | 824 | 393 |
| Marrow Creek gate | 612 | 315 |
| Kettle Pass gate | 501 | 269 |
| Training Yard aerial | 743 | 509 |
| Marrow Creek aerial | 501 | 366 |

`ShadowReach` (and the forest's near detail) now take their eyes from
`render::eyes`: the cameras not drawing into an image, so an impostor
capture's rig isn't one.

`docs/shots/performance/small_shadows_truck.png` is the player's first
view (the truck at base, clear day, lavapipe) with the rule; without it
the frame is identical to the pixel (the buttons, the vise, the tins are
within reach and still cast), as is the Marrow gate view.

## Small things' range

Items fade where they'd be a few pixels (`item_range`). Other small
things had no range. Marrow Creek's aerial view went from 998 to 1208
draws in one merge, with the top five kinds unchanged. `BUDGET_LIST=1`
lists every kind a view draws, which is how the tail showed up: 96 piano
parts (keys, hammers), 39 of the player's body and kit parts, 35 depot
fittings, 18 terminals, 18 notes, 15 supply crates, 7 keys. None were
items. (The guns in that view are items, on the gate's truck within
their 60 m range, and rightly drawn.)

`render::budget::small_ranges` gives any StandardMaterial mesh under 1 m
(largest side, world) that isn't part of an item or under a `LodReach` a
range of 200 times its size, 20 to 200 m: a 10 cm fitting goes at 20 m,
about three pixels a side on the headset. The aerial view now draws 945.
The gate, village and aerial shots with and without it
(`EXTRACTION_SMALL_RANGES=0`) differ only in the grass's sway.

## Guns at rest

A gun is one entity per glTF primitive. Its `static` body is already
one primitive per material, joined in Blender. Then come each moving
part (bolt, selector, trigger, hammer), a seated magazine's own, and
three for every round in a drum or clip. An AK-74 with its magazine is
about 19 draws, a PPSh with its drum 55, a Nagant 49. The parts move only
in hand.

`render::gun_rest`: a gun that isn't held or steadied, isn't under the
Player, and whose meshes have stayed the same for 30 frames is drawn as
its shown leaves merged in the pose it was left in. They're grouped by
material and by how each part casts the sun's shadow (merged together, a
drum's rounds would cast at any distance; `small_shadows` keeps them to
the eye's few metres). Merged guns are children of the gun, so they move
and fade with it, and guns left alike share them. A hand taking the
gun, or anything under it put on, taken off or gone, brings its parts
back exactly. The muzzle crown (fouling, nicks) and a scope's vignette
stay live. `EXTRACTION_GUN_REST=0` leaves every gun as its parts.

Marrow Creek's start has 15 guns lying near the gate: 165 leaf meshes
drawn as 125. Gate shadow draws went Marrow 391 to 361, Rime Fell 483 to
445, Sallow 513 to 468, with shadow triangles unchanged. The aerial view
went 954 to 919. The gate views' own draws don't move: those guns are
behind the cameras. `docs/shots/performance/guns_at_rest.png` shows
those guns with it off (top) and on (bottom); only the grass sway
differs. Tests: `gun_rest::tests::gauge_guns_lying_about_rest` and
`a_resting_gun_wakes`.

## The headset's two eyes: what each costs

In the XR path each eye is its own Bevy camera (`bevy_mod_openxr`
spawns `XrCamera(0)` and `XrCamera(1)`, one swapchain layer each, no
multiview). Everything a view does happens twice:
- frustum culling (`check_visibility`);
- light clustering;
- the main passes;
- tonemapping and upscaling;
- the sun's cascades, both building them from the eye's frustum and the
  caster culling and shadow pass per cascade.

Bevy's `prepare_lights` gives directional cascades a new shadow view per
camera, by design ("cascades are unique to each view"). Spot and point
shadows are shared across cameras. `--mirror` adds a third full camera,
and it's off by default.

Measured on lavapipe (Marrow Creek's village, 1024² an eye, 20-frame
holds, `scripts/bench.sh`'s second eye):

| | frame ms | GPU ms (passes) | render-world CPU ms |
| - | - | - | - |
| one eye | 1153 | 3536 | 18.9 |
| two eyes | 2217 | 6704 | 21.9 |
| two eyes, `shadows=0` | 1957 | 6042 | 19.3 |
| two eyes, `msaa=1` | 1167 | 3470 | 21.4 |
| two eyes, `cascades=1` | 2186 | 6665 | 20.8 |

- **The second eye is a whole eye's pixels.** The GPU time goes ×1.9 and
  render-world CPU +16%. That's what stereo costs; it isn't waste.
- **The sun's shadows** (both eyes' passes and the sampling in the
  opaque pass) are about 12% of the frame. Sharing the cascades between
  the eyes would save only the second eye's shadow passes, a part of
  that. It means vendoring and patching `bevy_pbr::prepare_lights`, the
  only place a directional light's shadow views are made per camera.
  Not worth a fork for single-digit percent. One cascade instead of two
  saves 1-2%.
- **4× MSAA at the eye's full size** is the biggest knob: off, the frame
  halves on lavapipe. Each eye carries a 4-sample HDR target
  (`camera_look`), and the swapchain is single-sampled.
  - On a headset GPU the share is smaller than in software rendering,
    but it's the one per-eye cost here that isn't the picture itself.
  - With a Beyond 2's pixel density, `msaa=2` (or off) may read the
    same.
  - `EXTRACTION_PERF="msaa=2"` is how to try it on the playtester's
    laptop; the session log's `gpu:` lines show the result.
- **The eyes stay matched.** `render::vr_budget` holds every view
  through both eyes to two eyes' budget and to shaded fragments at the
  game's auto eye size (1800², `BUDGET_EYE_PX` to change it). It also
  fails if the eyes see more than 10% apart, or if a third view draws
  the world. The fragments are overdraw × eye pixels. Marrow's worst is
  the spruce view: 45.8M a frame against 64.8M.

## First sight and the raid's one-offs

A single long frame is worse in a headset than a steady few milliseconds
over budget. The hitch work hunts the frames that run long once.

- **Models load at start, not at first sight.**
  - Spawning everything a raid can bring out after Marrow Creek had
    loaded (a robot of each mid-raid kind, one of every item) used to
    load 117 files mid-raid: 106 item and magazine models and 8 robot
    models.
  - `render::first_sight::PreloadPlugin` asks for every model under
    `models/items`, `mags`, `guns` and `robots` at start (340 paths).
    `EXTRACTION_PRELOAD=0` turns it off.
  - `first_sight::tests::nothing_new_is_asked_for_mid_raid` (in the
    suite) fails on any model asked for after start; without the preload
    it counts 114. `gauge_nothing_loads_mid_raid` (gauge tier) waits
    for the start's loads to land, then fails on any model or texture
    loaded from disk.
- **The shelf builds their pipelines too.**
  - `warmup::shelve_models` puts the preloaded models' primitives on the
    warm-up shelf, with the materials they'll wear.
  - On lavapipe, with everything brought out at frame 1100
    (`EXTRACTION_EVERYTHING=1100 EXTRACTION_PIPELINES=1`), one pipeline
    was built at first sight (a prepass, likely a skinned robot's).
  - Neither side's worst frame was that frame: CPU p99.9 1495 ms, GPU
    p99.9 1162 ms, against p50s of 1179 and 989.
- **Per-frame CPU and GPU.** `EXTRACTION_FRAMELOG=<path>` writes a line a
  frame (frame, cpu_ms, gpu_ms). `scripts/frames.py` gives each side's
  p50, p99, p99.9 and worst frame.
- **The hitch gauge's events** (`render::spikes`, `scripts/hitches.sh`):
  - the grenade, the wrecks and each bloom;
  - the Depot's residents;
  - a response team at the gate, a dirigible lowering its pod, a supply
    crate bursting, and the Surge's every bloom at once;
  - the homecoming.

  On Marrow Creek every event but the homecoming is within 2.2x of the
  busy moment's median (ceiling 3x).
- **Resting guns are merged off the main thread.** Deciding whether to
  merge a gun had built the merge, and a declined gun was asked again
  every half-second (a 240 ms frame). Now:
  - the check is cheap;
  - a declined gun is remembered;
  - new merges build on the async pool.
- **The Surge's reset at base goes a step a frame** (`persist::ZoneReset`).
  At once it was 37 ms of work and more:
  - the sweep 18 ms;
  - the mending 9;
  - the loot 7.

  Now the sweep runs 40 items a frame, then the wrecks, the mending,
  the anomalies, the loot, and the loot let fall 40 bodies a frame. A
  save waits for the end, and leaving mid-reset finishes it first.
  The homecoming went from 89-119 ms to about 60 ms (4x). The rest is
  single steps (the mending, the loot roll's spawn) that would need
  splitting inside `destruction::surge` and `loot::roll_zone_loot`.
