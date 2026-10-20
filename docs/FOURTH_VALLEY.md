# Sallow Reservoir: the fourth valley

`assets/maps/sallow_reservoir.ron`. The Authority's water works: a
masonry dam across the narrows of a hill valley, the reservoir behind it
drained by the machines, and the Custodian's sorted stock laid out in rows
on the bed. It opens after the other three in a career (Progression owns
the unlock). Map design's rules (`docs/MAP_DESIGN.md`) and lint hold it
like the other valleys; this file says what it's for and where things are,
and why.

## How it plays unlike the other three

The other three are crossings of rolling ground: a creek, a pass, a canal,
woods to hide in. Sallow is a **wall, then a hole**.

- **The dam is the first decision.** The truck stops below a 21 m masonry
  wall that fills the view north. Nothing in Sallow is reached without
  getting over it, and the three ways over are three kinds of risk: the
  west road round the abutment (long, wooded, safe), the spillway at the
  east end (short, walled in, a standing anomaly in its throat), and the
  face stair and crest (a climb you see the whole valley from, and are seen
  from).
- **The bed is open on purpose.** Behind the dam the drained reservoir is
  a basin half a kilometre long with no trees in it: silt, the old river
  in its channel, the drowned village standing up out of the mud, and the
  Custodian's stock in rows. It's the fast way to two of the three
  rendezvous and the most exposed ground in the game. Cover comes in
  clumps (the rows of buses, the ruins, the stranded boulders) with
  measured open gaps between them, and the long sightlines are the point.
- **Vertical.** The dam's crest is 21 m over the valley below and 16 m over
  the bed; the draw-off tower stands another 16 m above the crest, joined to
  it by a footbridge; the catchwater runs along the east hillside at crest
  height. You look down on the bed from three sides before you go in.
- **Wet.** The upper arm, held back by the old causeway when the bed was
  drained, is still full: New Sallow's lower street is under 0.5-1.4 m of
  water (wading slows you, the robots don't follow you in deep water), its
  upper street dry.

## What the place is (for Story)

Story writes the words; this is the place they're about.

- **Old Sallow** was a village drowned when the water board built the dam,
  sixty-odd years before the Lift. Its church tower, its bridge and its
  house walls stand in the bed now the water's gone.
- **New Sallow** was built for the people moved out, on the upper arm's
  slope above the top water line. It was evacuated with the others (it's
  in the district); half of it is flooded now.
- **The water works**: the dam, the draw-off tower, the valve house at the
  dam's toe, the filter house and keeper's cottages below the dam, the
  catchwater and its gauge house on the east hillside. The reservoir fed
  the three valleys' taps.
- **The drain**: in the second winter the Custodian opened the scour
  valves. "Collect everything" meant what was under the water too. The bed
  came up out of the reservoir over a month; the upper arm, behind the old
  causeway, didn't drain.
- **The stock ground**: with the depots full, the Custodian uses the bed as
  an open store: rows of buses (Marrow depot's buses, numbered, among
  them; bus four isn't there, Lev drove it out), bicycles in racks, wheelie
  bins in ranks, tagged "pending review". It's the depot under the Marrow
  Gate ("the sorting hall's bins, the bicycles, the buses") with no roof.
- **Its Gate** stands on the north-east shore: an overflow depot under the
  Marrow depot's authority, so there is no fourth key. The recall reaches
  it like every other.

## The map

640 m on a side, the fence 38 m in, as the others. North is -z.

```
    -280          -140            0             140          280   x
-280 +---------------------------------------------------------------+
     | Sallow Head (RV A)       Inflow bridge (RV B)       THE GATE   |
     |   New Sallow (flooded)       \  river in            silt fans  |
     |    the upper arm (water)      \                      (field)   |
-140 |  ==== the old causeway ====   the sorting ground  .  catchwater|
     | west          Old Sallow (ruins, the church tower) . gauge house|
     | shore            (field: churchyard)              .  (RV C)    |
   0 | road       old river channel through the bed       . east shore|
     |                                                    .           |
     |  slipway / fishing club    draw-off tower          .           |
 140 |  #################### THE DAM (crest road) ###########|spillway|
     |  west road        valve house (toe)                  |  chute  |
     | (switchbacks)  keeper's cottages      filter house   |         |
 280 |                      SOUTH GATE (insertion)                    |
     +---------------------------------------------------------------+
```

### The land

- **Below the dam** (z 160-282), the valley floor at 31 m, the river (the
  compensation flow out of the valve house) running south to the fence.
- **The dam**: a crest strip at 52 m from x -185 to 135 along z 140,
  falling sheer to 31 downstream and 36 upstream; its faces are the
  `dam_wall` prefab, absolute. The west end meets the west hillside, the
  east end the spillway.
- **The bed**: 36 m by the dam rising to 42 at the head, the old river
  channel meandering down it (shallow water, wadeable), banks to the high
  water mark at about 51 m: a grey band of bare ground under the
  treeline all round.
- **The upper arm** (north-west), behind the old causeway at z -130: water
  standing at 45 m over New Sallow's lower street.
- **Hillsides** west and east rise to 60-70 m, wooded above the high water
  mark.

### Landmarks visible from the truck

From the south gate, looking north: the dam's wall across the whole view,
its parapet and the two crest turrets; the draw-off tower standing up
behind the middle of it; the radio mast on the east hill behind the
spillway. Once over the dam: the church tower of Old Sallow in the middle
of the bed, the Gate on the far shore, New Sallow's water tower over the
upper arm. Each is one of a kind; the tower, the church tower and the mast
are all over 18 m.

| Landmark | Prefab | Where | Steer by it for |
| - | - | - | - |
| The dam | `dam_wall` | across the south | home, the truck |
| Draw-off tower | `draw_off_tower` | bed, by the dam | the crest, the bed's south end |
| Church tower | `steeple` | Old Sallow | the middle of the bed |
| Radio mast | `radio_mast` | east hill | the catchwater, RV C |
| Water tower | `water_tower` | New Sallow | the upper arm, RV A |
| The Gate | `robot_gate` | north-east shore | stay away |

### Routes and the three ways out

| RV | Where | The cheap (exposed) way | The other way |
| - | - | - | - |
| **A. Sallow Head** | (-200, -245), the road above New Sallow | West road over the abutment, then the west shore road above the water line: long, wooded | Down the slipway, across the bed's west side, over the causeway and through New Sallow's flooded street |
| **B. Inflow bridge** | (40, -262), where the river comes in | Straight up the bed: the crest, the slipway, the old road past Old Sallow and the stock rows. Fastest, open, the Custodian at work beside you | The west shore road, then the causeway and the bed's head |
| **C. Gauge house** | (245, -60), on the east hillside | The spillway chute and the east shore road | The crest east, the catchwater path along the hillside at crest height: a view over all of the bed, and seen from all of it |

Each is a different raid's end: A is wet and slow but hidden, B fast and
naked, C high and watched. The truck coming takes two minutes; at B
you wait for it in the open beside the Custodian's ground.

### Sightlines

- The bed is a deliberate sightline 500 m long: from the crest you see the
  Gate on a clear day. In drizzle (320 m) the bed's far half is grey; in
  a pea-souper (150 m) you cross it by the church tower and the channel.
- The catchwater and the crest look down into the bed: the high ground
  sees you before you see it, and so do the pylons the walkers raise on
  the bed.
- Inside New Sallow and in the west shore woods the views are short.

### Danger

| Region | Danger | Why |
| - | - | - |
| South gate, below the dam | 0 | picked over (`loot: 0.4`) |
| Keeper's row, filter house, the spillway | 1 | |
| The dam and the draw-off tower | 1 | seen, but the machines rarely come |
| West shore woods | 1 | the slow, safe way |
| Old Sallow | 2 | in the open bed; artifact field in the churchyard |
| New Sallow (flooded) | 2 | |
| The catchwater | 2 | |
| The sorting ground | 3 | the Custodian's stock, its work sites; rich |
| Silt fans and the Gate | 3 | the second artifact field, under the Gate |

### Hazards

- **The spillway**: a walled concrete chute, 12 m wide, no way out of it
  for 90 m, a standing anomaly in its throat you pass within a few metres
  of. The quick way up to the east side.
- **The bed's channel**: wading water, slow and loud.
- **New Sallow's lower street**: deep in places (swim), houses standing in
  water.
- Loose anomalies fall on the bed like anywhere else: it's open ground,
  you'll see the shimmer.

## Checks that keep it so

- `map_lint` holds it clean like the other valleys
  (`sallow_reservoir_passes_the_map_lint`).
- `extraction_core::map` tests: the bed is dry, the upper arm wet, the
  crest well above both, every RV on dry level ground.
- The fit gauge and the frame-budget gauge run it with the others.
- The headless scenario `sallow` drives there and extracts.

## Status

Design (this file), then a whitebox, sims, and dressing. See the commits.
