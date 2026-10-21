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
  it by its bridge; the catchwater runs along the east hillside at crest
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
- **The dam**: a crest strip at 52 m from x -200 to 140 along z 134-146,
  falling sheer to 30 downstream and 31 upstream, keyed into abutment
  shoulders at crest height either end. The crest is terrain, so walkers,
  bots and the lint walk it; its faces are thin placements, `absolute`:
  `dam_face` and `dam_face_end` downstream (the gap between them is the
  face stair's landing), `dam_back_end` and `dam_back` upstream (the gap is
  the slipway's). `src/world/dam.rs` builds them.
- **Ramps** the land can't give (the face stair, the spillway's steps, the
  catchwater's fall, the roads into New Sallow) are flights of terrace
  plateaus under the road, so the road's own grading evens them into a
  slope a body walks. Off the road they read as steps.
- **The bed**: 31-34 m by the dam rising to 41 at the head, the old river
  in a broad shallow channel down it (wadeable, its banks gentle enough to
  cross anywhere), bare ground up to the hillsides.
- **The upper arm** (north-west), behind the old causeway (48.6 m) at
  z -130: a flat bed at 38.6 under water standing at 47.6, New Sallow's
  lower street 0.7 m under it, its upper street dry at 49.4.
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

### Over the dam

Three ways, west to east, each a different risk:

- **The west road** leaves the river road by the keeper's cottages, runs
  west along the valley and climbs the west abutment's long shoulder to
  the crest's west end: the long, wooded, safe way. The west shore road
  goes on north from there.
- **The face stair** climbs the dam's downstream face from the valve house
  at its toe, 105 m of steps on an embankment against the wall, to a gap
  in the parapet a third of the way from the east end. You climb it in
  full view of the valley below, and come out on the crest in full view
  of the bed.
- **The spillway**, cut into the east abutment: a stepped concrete chute
  you climb from the filter house, walled in for most of its length, a
  Well standing in it beside the steps. At its head the catchwater path
  runs north along the east hillside above the shore road.

From the crest into the bed: the **slipway** (through a gap in the upstream
parapet, west of the middle), the **haul ramp** the machines cut down
from the crest's east end, and the **draw-off tower**'s stair: its bridge
leaves the crest's middle and its stair goes down inside it to a door on
the bed. The tower is for players; nothing paths through it.

### The three ways out

| RV | Where | The main route (the lint's) | The other way |
| - | - | - | - |
| **A. Sallow Head** | (-200, -258), the turning place above New Sallow | The west road, then the west shore road above the water line: long, wooded, safe | The face stair, the crest, down the slipway, across the bed's west side and over the causeway, round the arm's head by New Sallow's back lane |
| **B. Inflow bridge** | (30, -262), where the river comes in | The west road to the crest, down the slipway and up the old road through Old Sallow: the bed's length, open | The face stair, the crest east, the haul ramp down past the drowned cottage and up the sorting ground's east side |
| **C. Gauge house** | (236, -46), on the east hillside | The face stair, the crest east, the east shore road | The spillway's chute and the catchwater path along the hillside, above everything |

Each is a different raid's end: A is wet and slow but hidden, B fast and
naked, C high and watched. The truck takes two minutes; at B you wait
for it in the open beside the Custodian's ground.

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

## What the sims said, and what changed

Four batches of eight (`--sim 8 --map sallow_reservoir --jobs 2`):

| Batch | Out | Died | What it showed | Change |
| - | -: | -: | - | - |
| seed 1 | 4 | 0 (4 crashed) | A wrecked walker's torn-leg joints crashed avian; the bed's ruins hold next to nothing, so nobody went in | Robot bodies' joint fix (1aefd75); six caches on the bed; the silt fans off the walkers' track |
| seed 1 | 5 | 3 | Bots in Old Sallow now, but every raid west to New Sallow | A tagging shed on the sorting ground |
| seed 11 | 7 | 1 | Still west: a danger-2 street of full houses outweighed everything | New Sallow picked over (loot 0.9) |
| seed 21 | 6 | 2 | The shed looted (30 items); all three ways out used (Inflow bridge 4, Sallow Head 3, gauge house 1); both deaths in the danger-3 east under the Gate | |

Traffic and the last report: `docs/shots/fourth-valley/sims/`. Never
visited yet: the valve house and filter house, the draw-off tower (no
path goes through it, by design: it's for players), the gauge house and
the spillway. Progression's scout jobs are to send players to the first
two.

`--scenario crossing` (opt-in; `bot::crossing`) walks the lint's
shortest route, the face stair, the crest and the east shore road, to
the gauge house and out: 262 s.

## Waiting on others (at merge)

- Story's ten notes and terminals: run `python3 scripts/place_notes.py`
  once `claude/polish-story` and this branch are merged.
- Anomalies' Hush: `(kind: Anomaly(kind: "Hush", radius: 3.0), at:
  (-120.0, -5.0), name: "Old Sallow hush")` once their sixth kind is in.
- World models' drowned houses and dressed stone (claude/polish-world-models,
  ae803ac and 2bd5791): Old Sallow's four `*_damaged` ruins and the
  drowned cottage become `*_drowned`, Old Sallow's church tower becomes
  `chapel_drowned` (00e2ac6: the nave stripped and silted, the tower
  standing; it needs room for its 10 x 23 m footprint by the font and the
  mirror), and the dam's `Mat::Concrete`
  faces and tower `Mat::DressedStone` (then `python3
  scripts/fetch_assets.py materials`). Lint, fit and the sims again after.
- Progression's gate (`LATE_VALLEYS`), Visual design's fog and grade
  (0ff156b), Audio's slapback off the dam face, Wildlife's perches
  (they need `dam::PREFABS` or `buildings::prefab(id).is_some()`),
  World models' masonry material and drowned ruins, Underground's
  inspection gallery.

## Status

Whitebox lint clean, sims iterated, the dam, tower and stock built
(`src/world/dam.rs`, `art/props/dam.py`). Next: World models' drowned
ruins and masonry in place of the stand-ins, a pass through the valley
in fog with screenshots, and Performance's gauge on it.
