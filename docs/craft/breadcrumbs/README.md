# Breadcrumbs

Every reward off the ways a raid walks has to be cued from them: its
building's silhouette, its own shape (an artifact field's sparks, an
offering's figures), a lamp, a trail leaving the way, a sign. The rule
and its numbers are in `docs/MAP_DESIGN.md`, "Breadcrumbs";
`map_breadcrumbs` measures it and
`every_reward_is_cued_from_the_walked_ways` holds it.
`cargo test --release -p extraction_core probe_breadcrumbs -- --ignored --nocapture`
prints every reward and its cue (`MAP_ID` for one valley).

## The audit (5 November)

222 rewards in the five valleys. The first pass of the gauge found 18
with no cue from the walked ways. Each was shot from the nearest point
of a walked way, looking at it (`uncued_before.jpg`).

| Valley | Uncued | The worst, and what the still showed |
|---|---|---|
| Marrow | 4 | the tag office desk, 19 m into the trees: nothing. The well and its two scenes, 18-23 m off, behind a house: a wall (but the well's figures are seen from another way, 24 m off). |
| Kettle | 5 | the cairn, 58 m off, 17 m back from a ridge's lip: a bare hill. The farm workshop's cache, 22 m off in a field: grass. The shot-firer's store, 14 m off by the mine: grass. The quarry watch and the mine stores: a field you go to anyway, a shed in view. |
| Cinder | 1 | the ash pit, 39 m off: its sparks, plain. |
| Sallow | 6 | the stock pallet, 56 m off in the bus depot's yard: the yard's buses and shed. The first-aid boxes, the churchyard, bay four, the satchel, the drowned cottage: a shed, sparks, a gate, a low ruin, all in view. |
| Rime | 2 | the crown hole, 37 m off: its sparks in the pit. The miners' shrine, 13 m off: its figures before the chapel's lamp. |

Most of the 18 were the gauge's, not the map's: artifact fields and
offerings cue themselves, and a reward near a field is found by going
to it. The gauge counts those now, and its sight stops at buildings.

## The fixes

Five needed a cue, each the smallest that fits the place
(`fixes_before_after.jpg`, the same frame before and after):

- **Kettle, the cairn**: moved 8 m east to the ridge's lip, where its
  figures stand on the skyline from the way below.
- **Kettle, the shot-firer's store**: its magazine hut, the cache behind
  it.
- **Kettle, the farm workshop**: its woodpile, the cache by it (a light
  prop: the farm's view was at its building limit).
- **Marrow, the tag office desk**: a worn path off the way into the
  trees to it.
- **Sallow, the stock pallet**: a lorry left in the depot yard by it. At
  56 m it barely reads; the yard itself (its buses and shed) is the cue
  you go by.
