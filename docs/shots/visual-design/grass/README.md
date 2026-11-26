# Grass: square tussocks and the 3 m repeat (playtest 22 Nov)

These stills cover two items from the playtest, both in
`assets/shaders/terrain.wgsl`. Map design diagnosed them (`2cd0bb1` on
`claude/polish-map-design-2`, `docs/shots/map-design/ground/`).

1. **Square tiles** (00638, "cube lighting texture"). The grass layer's
   tussock shading was value noise on the world's axis-aligned lattice
   (0.59 m and 0.28 m cells), sharpened by `smoothstep(0.38, 0.58)` into
   hollows darkened to 0.7×. That drew blotches with square,
   axis-parallel edges. It now uses gradient noise (`gnoise`) on lattices
   turned 37° and 71° and offset, so no octave lines up with another or
   with the world's axes. The whole domain is bent by a 3 m warp, and the
   threshold is softer (`0.3..0.7`). The tussocks stay about a metre
   across and keep the same share of crown to hollow (the 0.15 m grain
   uses gradient noise too). All of it is low frequency, per the
   VR-shimmer ruling.
2. **Grass repeat** (00920). Layer 0 was one 3 m tile, hex-tiled. Up a
   long slope at a grazing angle, its clumps recurred every 3–5 m. A
   second variant of the same scan now covers soft-edged patches about
   10 m across: 1.62× the size, turned 23°, its hex vertices seeded
   apart. A pixel samples only the variant(s) its mask needs, so the
   extra taps fall only on the patches' edges and on the variant's own
   ground.

The bare-earth and puddle `vnoise` taps are unchanged. The puddle mask
must stay in step with `world::puddles::mask`.

| File | View |
| - | - |
| `tussock_mask_top_down.png` | The tussock mask from above, 12 m square: old (left), new (right) |
| `yard_close_{before,after}.png`, `yard_close_before_after.jpg`, `yard_close_crop_before_after.jpg` | Yard, open grass, looking down |
| `yard_rim_slope_{before,after}.png`, `yard_rim_slope_before_after.jpg` | Yard, the perimeter fence and the rim slope behind it (as 00920) |
| `valley_slope_{before,after}.png`, `valley_slope_before_after.jpg` | Marrow Creek, a long grass slope at a grazing angle |

In each `_before_after.jpg`, "before" is on top. All stills are 3200×1800
(2×), from eye height, at `--hour day --weather clearing --no-robots
--new-game --no-captions --frames 40`, with cameras:

- yard close: `--map training_yard`, `-62,0,40,280,-32`
- yard rim slope: `--map training_yard`, `-90,0,44,275,3`
- valley slope: `--map marrow_creek`, `-20,0,-100,280,-2`

"Before" used the same binary with `EXTRACTION_ASSET_ROOT` pointed at a
checkout of the previous commit's `assets/`; only the shader differs.
Both valley shots were taken at the 180 s limit, before every material
was in (`screenshot: taken after 180 s with the world not all in`): the
tufts are missing and some trees are half-dressed in both. The ground,
which is what's compared, is the same in both shots.
