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
| Forest impostors merged over 3x3 chunks | `world::trees` | About 250 fewer draws a forested view (Marrow Creek with the posts: gate 1267 > 851, spruce 1346 > 927) |

Shared tools for the next ones (in `render::budget`):

- `LodReach(m)` on an entity: every mesh under it, models that arrive
  later included, fades out by `m`.
- `ShadowReach(m)` on a mesh: it casts the sun's shadow only within `m`
  (a `VisibilityRange` can't do this: the cascades use the camera's).
- Items get a range from their catalogue size by themselves.

## Where it stands

The gauges at every lookbook view of the four valleys (GPU-less client,
one eye), worst view of each against the budget:

| | Worst | Budget | Headroom |
| - | - | - | - |
| Draws | 994 (Marrow Creek spruce) | 1200 | 17% |
| Triangles | 529k (Marrow Creek spruce) | 900k | 41% |
| Shadow draws | 731 (Training Yard aerial) | 900 | 19% |
| Shadow triangles | 268k (Cinder Works gate) | 700k | 62% |

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
