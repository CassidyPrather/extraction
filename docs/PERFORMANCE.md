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

The gauges at every lookbook view of the four valleys (GPU-less client,
one eye), in clear weather (the furthest you see, so the most drawn;
`BUDGET_WEATHER=drizzle` for the reference look), worst view against
the budget:

| | Worst | Budget | Headroom |
| - | - | - | - |
| Draws | 1081 (Marrow Creek spruce) | 1200 | 10% |
| Triangles | 570k (Marrow Creek spruce) | 900k | 37% |
| Shadow draws | 706 (Training Yard aerial) | 900 | 22% |
| Shadow triangles | 247k (Cinder Works gate) | 700k | 65% |

At the start the worst views were 3316 draws and 970 shadow draws.

Not yet known, and only the headset can say: what MSAA 4x costs on the
3060 (`EXTRACTION_PERF=msaa=2`/`msaa=1` against the `gpu:` lines), and
the main schedule's and render world's real CPU time (the `cpu:` line;
under lavapipe both share four cores with the renderer and read 17-31
ms, which says nothing about the laptop).

Left on the list: interiors seen from
outside within 25 m (furniture behind walls; measured small, no class
in any view's top five); the truck's 47-94 shadow
casters; GPU occlusion culling (Bevy 0.19 has it, but it needs a depth
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
`budget::tests::a_busy_moment_holds_the_cpu_budget` stages a raid's busy
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
`warmup::tests::the_shelf_stocks_and_clears` holds the shelf's mechanics
(it stocks, the decoys go, nothing's left after).

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
