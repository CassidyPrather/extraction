# Rime Fell: the fifth valley

`assets/maps/rime_fell.ron`. A high, cold valley at the treeline's edge:
Coldharbour, a mining settlement on a glacial bench, the funicular that
climbed to it, the ore mill stepped down the slope under the mine, the
weather and radio observatory on the north-west summit, and the avalanche
galleries over the switchback road. It opens late in a career
(Progression owns the unlock). Map design's rules (`docs/MAP_DESIGN.md`)
and lint hold it like the other valleys; this file says what it's for,
where things are, and why.

## How it plays unlike the other four

Marrow, Kettle and Cinder are crossings of rolling ground with woods to
hide in; Sallow is a wall and then a hole. Rime Fell is **a climb with
nothing to hide behind**.

- **Up, all the way.** The truck stops at 51 m; the bench is at 93, the
  col at 136, the summit at 152. Every way out is uphill, and every way
  back to the truck is a long look down at where you've been. You see the
  whole valley below you, and the whole valley above sees you.
- **No trees.** The last stunted spruce are in the Bottoms; above the
  switchbacks' second leg nothing grows. Cover is rock: boulders on the
  scree, outcrops on the steep ground, the next building. The lint's cover
  rule holds by the boulders alone. Long sightlines are the whole upper
  valley; short ones are only indoors.
- **Two ways up the first rise, two kinds of risk.** The **funicular**
  goes straight up the middle, its service stair beside the track: the
  quick way, 200 m of steps in full view of everything. The **pass road**
  switchbacks up the west slope, three times the length, under the
  **avalanche galleries** where the chutes cross it: dark concrete roofs
  you can shelter under and be trapped under.
- **Tight inside.** The **ore mill** is the close-quarters fight the open
  valley isn't: four levels stepped down the knoll, 4 m apart, joined by
  steep stairs and a ladder, crowded with the crusher, the ball mill, the
  flotation cells and the concentrate shed; aisles a metre wide. It is the
  richest building in the valley and the hardest to clear.
- **Vertical.** Ladders in the mill and the observatory; the observatory's
  roof deck over the whole valley; the top station's winding house; the
  galleries' roofs climbable from the slope above. The zipline kit was
  made for this valley: the slopes are long, steep and bare.
- **Cold.** The wind never stops up here (`ambience/wind_mountain`), the
  snow lies on the north faces all year, and when the weather turns it
  snows (Visual design's snow and storms). See "Cold" below.

## What the place is (for Story)

Story writes the words; this is the place they're about.

- **Coldharbour** was a company settlement for the Rime Fell mine: lead,
  zinc and a little silver from the knoll east of the bench. Miners'
  houses along one street, the mine office, the canteen, a chapel, the
  top station. Snowed in four months a year; the funicular was its only
  way down in winter. Evacuated with the district.
- **The funicular** (built with the mill) carried men, food and the
  concentrate sacks: one car up while the other came down, passing at the
  loop half way. One car is stopped at the loop now; the other is at the
  top station, its rope cut.
- **The ore mill**: crushed ore from the adit fell through it by gravity,
  level by level, to the concentrate shed at the bottom: the crusher at
  the top, then the ball mill, then the flotation cells, then the
  thickener and the sacks.
- **The observatory**: the weather service's high station and a relay for
  the district's radio, manned year round by two people. It kept
  reporting after the evacuation. Its log is the best record of what came
  over the col.
- **The galleries**: concrete roofs over the pass road where the chutes
  cross it. The roadmen kept the road open with a plough and the grit
  store at the bottom hairpin.
- **The Custodian** comes in by the cirque in the north-east, past the
  tarn. It wants the mine: the concentrate, the rails, the rope. Its
  walkers raise pylons on the upper slopes where they can see the bench.
- **The crown hole**: the old workings under the upper slopes fell in the
  first winter after the Lift; the ground over them sank into a pit.
  Things come up out of it.

## The map

640 m on a side, the fence 38 m in, as the others. North is -z.

```
    -280          -140            0             140          280   x
-280 +---------------------------------------------------------------+
     |  OBSERVATORY        SADDLE (RV A)       THE COL (RV B)  GATE   |
     |  (summit, 152 m)    ridge track ----- col hut      cirque      |
     |                     bivouac               avalanche   the tarn |
-140 |  west slopes   miners'   crown hole       watch       (field)  |
     |                path     (field)   snow survey                  |
     |                            upper slopes (scree, snow)  TIP (RV C)|
   0 |  chapel  COLDHARBOUR (bench, 93 m)  top station   adit  compr. |
     |          street ---------------------------  THE MILL (stepped)|
     |                                         |                      |
 140 |  galleries  switchbacks      hairpin hut  funicular             |
     |  (pass road, 3 legs)                     |  (stair + track)    |
     |                                    valley station              |
 280 |   roadmen's hut    THE BOTTOMS  SOUTH GATE (insertion, 51 m)    |
     +---------------------------------------------------------------+
```

### The land

- A 15% tilt north under everything, a scoop for the Bottoms, high bare
  walls east and west, the summit hill in the north-west, the cirque's
  back wall in the north-east with the tarn's hollow under it.
- **The bench** (Coldharbour, 93 m) is a plateau with a 30 m blend: its
  edges are the steepest open ground near the settlement.
- **The mill's terraces**: four plateaus, 90/94/98/102 m, stepping up the
  knoll's south face (z 52 to -12), one per level of the mill.
- **The crown hole**: a 6 m pit on the upper slopes; the artifact field.
- **Avalanche chutes**: two shallow channels off the west wall, across the
  switchbacks; the galleries sit where they cross the road.

### Landmarks

| Landmark | Prefab | Where | Steer by it for |
| - | - | - | - |
| The ore mill | `ore_mill` | the knoll, east of the bench | the bench, the tip road |
| The observatory | `observatory` | north-west summit | the saddle (RV A) |
| Top station | `funicular_upper` | the bench's east end | the funicular, Coldharbour |
| Avalanche watch | `ranger_tower` | the upper slopes | the col (RV B) |
| The chapel | `chapel` | Coldharbour's west end | the switchbacks' top |
| The Gate | `robot_gate` | the cirque | stay away |

### The three ways out

| RV | Where | Main route (the lint's) | The other way |
| - | - | - | - |
| **A. Observatory saddle** | (-115, -264) | the funicular stair, the street, the col track, the ridge track west | the switchbacks, the miners' path over the west slopes past the bivouac |
| **B. The col** | (70, -262) | the funicular stair, the col track over the upper slopes | the switchbacks, the street, the ridge track |
| **C. Tip road** | (238, -28) | the funicular stair, the street, round the knoll by the mill road | the mill steps past the mill's levels and over the adit |

A is far and high, past the observatory; B is the straight climb over
open snow; C is short but goes past the mill.

### Danger

| Region | Danger | Why |
| - | - | - |
| The Bottoms | 0 | picked over (`loot: 0.5`) |
| The switchbacks, the funicular | 1 | |
| West slopes | 1 | the miners' path: slow, out of the way |
| Coldharbour | 2 | |
| The upper slopes, the col, the summit | 2 | open, the walkers' pylons see the bench |
| The tip | 2 | |
| The mill | 3 | rich (`loot: 1.7`), tight, slow to clear |
| The cirque | 3 | the Gate and the tarn's field |

### Hazards

- **The galleries**: dark, 44-50 m long, the road the only way through.
- **The funicular stair**: 200 m of steps with nowhere to go but up or
  down.
- **The crown hole**: an artifact field in a pit on the upper slopes, a
  Well at its lip.
- **The tarn**: the second field under the Gate, a Mirror on its shore.
- **The galleries' Hush** on the slope between the switchbacks' legs.

## Cold

There are no meters in this game, so the cold is in the body
(`extraction_core::cold`, `src/world/cold.rs`). A valley says how cold it
is (`cold: 1.0` in the map; every other valley is 0).

- Out in the open the air bites by the valley's cold, your height over
  the insertion (nothing in the Bottoms, full from 85 m up: the col and
  the summit), and the weather: a clear spell is kinder; fog, rain and
  above all falling snow (Visual design's `Snow`) are worse; night bites
  harder. On the col in ordinary weather you're chilled through in about
  six minutes.
- A roof over your head (a ray up from the head finds anything solid:
  a house, the mill, a gallery's roof) stops it and warms you back in
  under a minute.
- A chilled body shivers: its stamina is capped (down to 45% frozen
  through), so sprints are short and, through the guns' tired sway, the
  aim shakes; every few seconds a shiver runs through both hands.
- Your breath shows in the cold air (the snow's, `world::snow`).

So the summit and the col are places you go to and leave, and the
buildings on the way are where you stop: the galleries, the bivouac,
the col hut, the snow survey hut.

## Structures

| Prefab | Where it's built | What it is |
| - | - | - |
| `ore_mill` | `src/world/fell.rs` | four bays stepped 4 m up the knoll (floors 0.15/4.15/8.15/12.15), stairs between, a ladder up the ore bin; 9 loot spots; the bots' indoor plan knows each bay's floor |
| `observatory` | `fell.rs` | stone block, porch, instrument room with the radio desk, ladders to the store and the roof deck, dome, mast to 19.7 m |
| `funicular_lower`, `funicular_upper` | `fell.rs` | the stations beside the line; the top one's bull wheel in its housing |
| `funicular_car` | `fell.rs` | a stepped car on the 20% grade, stopped at the passing loop |
| the funicular's track and stair | `src/world/roadworks.rs` | laid along the road named `Funicular`: metre gauge, rope on rollers, the passing loop, the service stair and its rail (visual only; you walk the graded path) |
| the avalanche galleries | `roadworks.rs` | `galleries: [(at, length)]` on a road: wall on the uphill side, roof on columns over the road, snow and icicles, a drift up onto the roof; solid |

## Checks that keep it so

- `map_lint` holds it clean like the other valleys
  (`rime_fell_passes_the_map_lint`).
- `rime_fell_parses_and_builds`: the bench over the Bottoms, the col over
  the bench, the summit over the col, the mill's four terraces 4 m apart,
  the tarn wet, every RV dry and level.
- Every valley-wide gauge over `map::VALLEYS` (fit, budget, notes,
  wildlife, scenes, anomalies) runs it with the others.

## What the sims said, and what changed

Three batches of eight (`--sim 8 --map rime_fell`); summaries and
traffic in `docs/shots/fifth-valley/sims/`.

| Batch | Out | Died | What it showed | Change |
| - | -: | -: | - | - |
| seed 1 | 7 | 1 | All east: the funicular, the lorry road, the mill (63 items), the tip road. Nobody west of the street | The summit pays like its danger (loot 1.65), the west slopes a little over theirs |
| seed 11 | 4 | 4 | Still east; the deaths on the open upper slopes and at the tarn | The miners' barracks on the west slopes |
| seed 1 | 4 | 4 | The saddle used (3 of 4 out), the observatory looted; the switchbacks still only the long way round | |

Never visited yet: the switchbacks' huts and the galleries (the
funicular is always cheaper, by design), the barracks, the compressor
house. The open upper slopes kill: drones and walkers see you a long way
off up there.

## Frame budget

Performance measured it on main (f4916fa): nothing to trim. Draws peak at
388 of 1200 (Coldharbour's street), triangles at 419k of 900k, overdraw
at most 2.47 (inside the mill). The tightest number is the gate view's
sun-shadow triangles, 532k of 700k, most of it the guns at the truck.
The scree draws about 140 merged rocks a view (35-40k triangles); the
galleries, the funicular and the mill never make a view's top five. First
frame in 4.3 s, the quickest valley; busy-moment hitches like the others.

## Status

Lint-clean under every rule on main (doors, Surge, landmarks), the fit
gauge, the anomalies' and wildlife's per-valley tests; `--scenario
crossing --map rime_fell` walks to the tip road and extracts (219 s).
Shots: `scripts/lookbook.sh rime_fell` (twelve views), the prefabs and
road works in `docs/shots/fifth-valley/`.

Waiting on others: Story's notes (`place_notes.py`, after the merge),
Progression's contract that opens the fell after Sallow, World models'
models for the mill, observatory and stations, Audio's recordings
(high wind, choughs, avalanche, the rope, a loose sheet on the mill),
Visual design's grass under snow.
