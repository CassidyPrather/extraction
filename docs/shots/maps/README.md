# The valleys from above, with the #94 extract plan

One overhead per valley, north up. Each is a real render of the game at
main 93dc0c43: a straight-down camera over the valley's centre, 2048 ×
2048 px, at midday in clearing weather. The plan is drawn on top, with
the legend and a scale bar in the panel underneath.

| Valley | File | BUSY | CALM |
|---|---|---|---|
| Marrow Creek | `marrow_creek_extracts.png` | Ranger tower | Rail yard |
| Kettle Pass | `kettle_pass_extracts.png` | Mine yard | Hunting lodge |
| Cinder Works | `cinder_works_extracts.png` | North sidings | Fire lookout |
| Sallow Reservoir | `sallow_reservoir_extracts.png` | none (the user's call) | all three |
| Rime Fell | `rime_fell_extracts.png` | Tip road | The saddle |
| Training yard | `training_yard_extracts.png` | today's layout, before #89 | |

## What's drawn

- **The plan:** Progression's final #94 list. It has the pads and short
  exfils as the maps place them today. The transceiver sets and the
  alternates are Map design's picks.
- **PAD and SET:** each rendezvous, and its transceiver on the shed wall
  facing the pad. The shed is the one already there for the RV
  wait-cover rule. At the Fire lookout, which has no shed, the set is at
  the ranger tower's foot.
- **SX:** the south gate's short exfil, with its set on the guard booth's
  wall.
- **BUSY:** red dashed lines run from the busy pad to its four nearest
  machines (work sites, sentries, spotters, the Gate), each labelled with
  its straight-line distance.
- **ALT and VP:** each flare-opened stop, its vantage, and a sight line
  between them.
  - The sight line is checked against the ground only. Trees and
    buildings are not counted. The eye height is 12 m on a tower or mast,
    the crest on Sallow's dam (53 m), and 1.7 m elsewhere. Every line
    drawn is clear of the ground.
  - Kettle's quarry-rim vantage was chosen as the highest ground 35-90 m
    from the stop with a clear line to it: (186, -34).
  - Rime's observatory stop is marked TRACK NEEDED, because no road
    reaches it.
- **Context:** the truck's road in, the valley's named places, the work
  sites, the sentry pylons, and the radio masts and mast huts.

## How they were made

Each overhead is a `.cine` shot with one still, `fov` 30-42°, from
460-907 m up. They were rendered by `scripts/capture_cinematic.sh` with
`PREVIEW=1 SIZE=2048x2048`.

That needs two local, capture-only edits, which were not committed. The
terrain is culled past the weather's visibility (`world::terrain::
fog_cull`), and the hill valleys carry a cloud cap (`world::sky`, the
height fog's `cap_density`). Both were switched off under an environment
variable for the capture.

The annotations are drawn with PIL. Each point is projected through the
same camera at the map's real ground height, from a `map-dump`
heightmap. Each render's levels are stretched (1st to 99th percentile)
to take out the haze of a kilometre of air; the pixels are otherwise
the game's.

The scale bar holds at the valley's mean ground. The view is a
perspective one, so ground above that mean reads a little bigger.
