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

(To be written with the cold system: exposure up high and in the wind,
warmth indoors and by a stove.)

## Checks that keep it so

- `map_lint` holds it clean like the other valleys
  (`rime_fell_passes_the_map_lint`).
- `rime_fell_parses_and_builds`: the bench over the Bottoms, the col over
  the bench, the summit over the col, the mill's four terraces 4 m apart,
  the tarn wet, every RV dry and level.
- Every valley-wide gauge over `map::VALLEYS` (fit, budget, notes,
  wildlife, scenes, anomalies) runs it with the others.

## Status

Whitebox lint-clean but for the landmarks its own prefabs give it.
