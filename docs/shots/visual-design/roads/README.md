# Road edges: before and after

Each sheet shows one road edge in one valley, before (main, left) and after (right), by day. The frames are cropped round the edge and enlarged 2× with nearest-neighbour, and saved as uncompressed PNG, since JPEG hid these seams. The four rows are:

1. normal render, standing at eye height on the verge;
2. normal render, top-down from 6 m;
3. `--terrain-debug layers`, standing;
4. `--terrain-debug layers`, top-down.

In the debug view every ground layer is one flat colour, unlit and unfogged:

| layer | colour |
|---|---|
| grass | green |
| forest floor | brown |
| mud | lilac |
| rock | grey |
| gravel | yellow |
| asphalt | black |
| dirt | orange |

The weights and cover are the same as in the normal render.

## What was wrong

The road paint was a 1 m ramp on the 2 m vertex grid, interpolated per vertex, so each edge was at most one triangle wide and ran along the triangles. The height-aware cover then sharpened it into a sub-metre line.

## The fix (paint only)

Road widths, heights, collision, nav, footstep surfaces and `hm.roads` are unchanged.

- **Vertex road channel.** It now carries each vertex's signed distance to the nearest road edge, from the roads' own centrelines (`world::terrain::RoadEdges`). A distance is nearly linear across a cell, so interpolated it is the true distance at every pixel. This needs no texture tap.
- **Edge per pixel.** `terrain.wgsl` builds the edge from that distance. It wanders over metres, frays over decimetres and meanders over tens of metres.
- **Verges by road kind:**
  - asphalt: a gravel lip, then a thin worn verge;
  - gravel and ballast: a dirt verge;
  - dirt: a trodden, muddy verge.
- **Road-layer cover.** A road layer's cover is its weight. Its stones carry it over the grass only in between full and none. The old height offset never let asphalt cover fully.
- **Junctions.** Where a lane meets a road, the interpolated road kind blends its two neighbouring kinds. Rounding it drew a hard-edged patch of gravel at every junction.

## The gauge

`scripts/road_edges.py`, run on the debug top-down frames, measures two things across each 0.6 m band of edge:

- **width:** the metres from 90% road to 90% grass;
- **wander:** how far the 50% line strays from a straight fit over the frame's 9 m.

| measure | floor |
|---|---|
| width, gravel, dirt and rail verges | 1.5 m |
| width, asphalt lip | 1.0 m |
| wander (below this is flagged "ruled") | 0.10 m |

Before the fix, 27 of the 29 edges it could measure fail. After, 2 of 28 fail.

| valley | edge | before (main): width / wander | after |
|---|---|---|---|
| marrow_creek | 0_asphalt | 0.87 m / 0.35 **FAIL** | 2.10 m / 0.33 |
| marrow_creek | 1_asphalt | 0.74 m / 0.12 **FAIL** | 1.70 m / 0.13 |
| marrow_creek | 2_dirt | 0.67 m / 0.16 **FAIL** | 2.24 m / 0.39 |
| marrow_creek | 3_dirt | 0.63 m / 0.13 **FAIL** | 1.97 m / 0.12 |
| marrow_creek | 4_gravel | 0.67 m / 0.14 **FAIL** | 2.60 m / 0.54 |
| marrow_creek | 5_gravel | 1.11 m / 0.51 **FAIL** | 3.34 m / 0.89 |
| kettle_pass | 0_asphalt | 1.21 m / 0.20 | 2.29 m / 0.18 |
| kettle_pass | 1_asphalt | not measured | not measured |
| kettle_pass | 2_dirt | 0.77 m / 0.04 ruled **FAIL** | 2.24 m / 0.13 |
| kettle_pass | 3_dirt | 0.79 m / 0.61 **FAIL** | 2.11 m / 0.58 |
| kettle_pass | 4_gravel | 0.65 m / 0.10 **FAIL** | 2.34 m / 0.21 |
| kettle_pass | 5_gravel | 0.65 m / 0.19 **FAIL** | 2.31 m / 0.15 |
| cinder_works | 0_asphalt | 0.73 m / 0.15 **FAIL** | 1.48 m / 0.11 |
| cinder_works | 1_asphalt | 0.75 m / 0.14 **FAIL** | 1.83 m / 0.27 |
| cinder_works | 2_dirt | 0.60 m / 0.17 **FAIL** | 2.25 m / 0.13 |
| cinder_works | 3_gravel | 0.55 m / 0.11 **FAIL** | 2.15 m / 0.17 |
| cinder_works | 4_gravel | 0.65 m / 0.14 **FAIL** | 2.37 m / 0.39 |
| cinder_works | 5_rail | 0.77 m / 0.10 ruled **FAIL** | 1.61 m / 0.12 |
| sallow_reservoir | 0_asphalt | 0.51 m / 0.03 ruled **FAIL** | 1.51 m / 0.13 |
| sallow_reservoir | 1_asphalt | 0.49 m / 0.02 ruled **FAIL** | 1.87 m / 0.08 ruled **FAIL** |
| sallow_reservoir | 2_dirt | 0.79 m / 0.05 ruled **FAIL** | 1.52 m / 0.17 |
| sallow_reservoir | 3_dirt | 0.73 m / 0.06 ruled **FAIL** | 1.46 m / 0.20 **FAIL** |
| sallow_reservoir | 4_gravel | 0.70 m / 0.08 ruled **FAIL** | 1.80 m / 0.12 |
| sallow_reservoir | 5_gravel | 4.50 m / 1.20 | 4.19 m / 0.87 |
| rime_fell | 0_asphalt | 0.65 m / 0.13 **FAIL** | 2.00 m / 0.12 |
| rime_fell | 1_asphalt | 0.65 m / 0.14 **FAIL** | 2.28 m / 0.18 |
| rime_fell | 2_dirt | 0.63 m / 0.11 **FAIL** | 2.10 m / 0.12 |
| rime_fell | 3_dirt | 0.79 m / 1.45 **FAIL** | not measured |
| rime_fell | 4_gravel | 0.64 m / 0.17 **FAIL** | 2.58 m / 0.19 |
| rime_fell | 5_gravel | 0.61 m / 0.13 **FAIL** | 1.97 m / 0.24 |

Both edges still under a floor are at Sallow Reservoir:

- **Crest road, edge 1:** 1.87 m wide, but its 50% line strays only 0.08 m. The road runs dead straight along the dam, and the meander doesn't bend it quite enough over 9 m.
- **Back lane, edge 3:** 1.46 m against the 1.5 m floor. This dirt lane runs over the drowned reservoir bed, so beyond its verge is silt, not grass. Its trodden verge is the same mud as the silt, so it adds no width there.

Both read soft in their sheets.

"Not measured" means the gauge found no road-to-grass edge to measure in that frame. Sallow 5 reads over 4 m in both: its frame takes in more than the one edge.

**Cost.** Performance measured cd79b418 against main: Cinder's arrival view was −3% (noise) and Kettle's s23_side −12%. Draws and triangles are identical. Ground with no road near now skips the road blend.
