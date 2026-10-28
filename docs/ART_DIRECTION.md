# Art direction

The look the visual changes are held to. `DESIGN.md` says what the game
is; this says what it should look like, and why. Check every visual
change against it, with before and after shots from the lookbook
(`scripts/lookbook.sh`, see "Checking" below).

## The one-line brief

A wet, overcast exclusion zone in late autumn: low cloud you could almost
touch, the far side of the valley half gone in the murk, and the land
read in layers of fog. Grounded and gloomy, but never murky to the point
of mud: you should always be able to tell grass from mud from rock, near
from far, and a robot from a tree.

## References (described, not copied)

- **Late-October Polesie or Carpathian foothills, mid-morning in
  drizzle.** Olive and straw grass, black-green spruce, birch trunks the
  brightest thing in the frame, wet asphalt darker than the soil.
- **Documentary photographs of abandoned Soviet-era villages under
  overcast.** Flat light, no hard shadows, colour mostly in the
  mid-values; the sky is the brightest thing and never pure white.
- **Tarkovsky's Zone (Stalker, 1979), its exteriors.** Damp, green-brown,
  a held breath. The colour is muted but present: not a grey filter.
- **A Scottish glen or the Tatras in low cloud** for Kettle Pass: cold
  rock, the tops cut off by cloud, a cooler, harder light.
- **A burnt pine plantation a year after the fire** for Cinder Works:
  charcoal trunks, ash-grey ground with rust and ochre, the air warm and
  brown with dust.

## Values

The headset shows everything big and close; values matter more than hue.

| Element | Value (sRGB, 0-255, as shot) | Note |
| - | - | - |
| Sky, overhead | 110-135 | Darker than the horizon: a low deck, never a white dome |
| Sky at the horizon / fog | 135-165 | The brightest broad area in the frame |
| Distant forest silhouettes | 95-125 | Read as layers: each ridge paler than the last |
| Meadow grass, near | 55-85 | Olive; never as dark as the forest floor |
| Forest floor | 35-60 | Dark, warm brown |
| Spruce foliage, near | 25-45 | The darkest natural thing |
| Birch bark | 150-190 | Brightest natural thing; anchors the eye |
| Wet asphalt | 45-65 | Darker than the verge |
| Robots (pale shell) | 150-200 | Above the background at every distance to 60 m |

**Nothing below about 18 in a lit area** (black reads as a hole in a
headset and smears on OLED/LCD). **Nothing pure white** except emissives.

## Colour

- **Palette**: olive and straw greens, umber and peat browns, slate and
  blue-grey, birch white; rust and dull ochre on metal; Cinder's ash grey,
  charcoal and a burnt sienna. Saturation stays in the mid-values; the
  brights and the darks are near-neutral.
- **Accents** are rare and mean something: anomaly orange, robot cyan,
  warning red and yellow on signs. Keep the environment out of those
  hues so they stay readable.
- **Temperature**: daylight through cloud is slightly cool (sky light),
  what little direct sun gets through slightly warm. Shadows go blue-grey,
  never black.

## Light

- **Overcast first.** The sky's ambient carries the scene; the sun is
  high, dim and diffuse, its shadows short and soft. Shadows are a hint
  of direction, not a pattern on the ground.
- **No screen-space effects per eye** unless shown to be cheap: no SSAO,
  SSR or volumetrics. Darkening and contact come from baked vertex AO,
  textures and the fog.

## Fog and air

- **Distance fog** is the horizon's colour, so the land dissolves into
  the sky at the far side of the valley (visibility is gameplay: robots'
  sight follows it). It should make silhouettes, not mush: the nearest
  tree line stays dark and readable to about 60 m in drizzle.
- **Ground fog** pools in the low ground and drifts in banks; it is
  lighter than the land behind it.
- **The sky** is a low stratocumulus deck: heavy darker bellies, paler
  seams, faster scud underneath. It moves, slowly.

## Night

Night is half the game, and it must be both beautiful and playable.

- **The moon is the light and the compass.** Its month is eight Surge
  cycles. It rides the sun's own path: east to west over the north. Full,
  it stands where the noon sun would. Waxing, it hangs toward the west;
  waning, it rises in the east. Round new moon it isn't up. Bright in a
  clear sky, it casts long, cool shadows that tell you which way you
  face.
- **Clear nights can be walked; overcast nights are black.** A clear
  full-moon night lets you cross a meadow without a torch. Fog or a
  downpour leaves only the grade's lifted black floor: the torch and the
  lamps are the only light.
- **The sky earns the dark.** The gaps between clouds are darker than
  the clouds themselves. Stars show in the gaps, with a Milky Way band;
  near the horizon they flicker. The moon silvers the cloud edges near
  it and the fog on its side.
- **Lights you navigate by** get a halo, not bloom. The Depot's lamps,
  the camp's lantern and the truck's work lamp each have a tight core and
  a wider glow, which spreads more in damp air. A torch throws a soft
  cone of lit air with dust drifting in it; fog and rain show it most.
- **A coloured lamp stays its colour up close.** A red landmark and a
  green rendezvous beacon must read as red and green, not white. The
  tonemapper (AgX) runs a colour to white past about 1 in its brightest
  channel, and a green, which is nearly all luminance, at about a third
  of that. So every lamp's light is scaled down, hue kept, until it is
  under both caps as the tonemapper gets it, after the eyes' adaptation:
  - a halo, in `halo.wgsl` (`HUE_CAP`, `LUMA_CAP`);
  - a bulb, by putting `render::grade::ToTheEye` on it with the lamp's
    colour at any strength (`hue_safe`, `LAMP_CHANNEL_CAP`,
    `LAMP_LUMA_CAP`).
- **A landmark's lamp shows from below.** On a wide flat top, like a
  chimney's mouth, the lamp stands on a short pole (`lamp_lift`, at most
  1.5 m) so the rim doesn't hide it from the ground.
- Gauge: `scripts/lamp_check.sh [map]` shoots every landmark lamp and
  beacon at night from 50, 150 and 300 m, on up to six bearings each.
  A distance passes when some bearing sees the lamp brighter than its
  surroundings and in its own colour.
  - Open (2026-10-28):
    - Sallow, Kettle and Cinder pass.
    - Marrow fails once: lamp 8913 from 300 m, dim on all three
      bearings, not yet looked at.
    - Rime Fell and Training Yard haven't been run with the final gauge.
    - Next step: `scripts/lamp_check.sh marrow_creek` and look at that
      lamp's views (`target/lamps/<map>/wayfind.json`, its `*_a.png`),
      then run the two remaining valleys.
    - The beacon bulb on `ToTheEye` is Fourth valley's change
      (`render/rv_beacons.rs`); it lands on top of this one.
- **Eyes, not a camera.** Out of a lit room you begin to see again
  within a few seconds; the rest of your night vision comes over a
  minute. Standing by a lamp keeps your eyes from opening as far. The
  exposure never pumps.
- Gauges: `scripts/night_matrix.sh` shoots every valley's night views
  across a range of weathers and moon phases. It then checks the
  luminance:
  - the near ground must be distinct from the sky on a clear night;
  - overcast must be no brighter than clear;
  - no view may be a hole.

## Storms and snow

- **A storm is a downpour with thunder in it.** Half of all downpours
  are storms. Under the anvil it is darker, the wind is at its hardest,
  and the hollows fill in a minute or two.
- **Lightning lights the whole valley for a few frames.** Two to four
  strokes flicker over about a third of a second. The light comes from
  where it struck, so shadows swing across the ground for the flash
  (the sun's cascades run only for those frames). The cloud lights from
  inside, the bellies most, and so do the fog and the horizon. Strikes
  come every 6 to 24 seconds, mostly far off. The thunder follows by
  the distance: about three seconds a kilometre (Audio).
- **Rain you can see crossing the valley.** In a downpour, veils of
  heavier rain drift across 40 to 200 m out with the wind: ragged, in
  columns, thinning upward. They gust in a storm.
- **First snow, at altitude.** In Kettle Pass, on one cold cycle in
  three after the first couple, snow starts 20 to 40 minutes into the
  raid. Above about 30 m (the pass, the lodge, the tops) it comes down
  as flakes and lies over about nine minutes; the valley floor stays
  wet. It lies on what faces up and is open to the sky: roofs and
  ground, not walls or under eaves. It comes thin and in patches first,
  the grass through it. Trees take it on their tops only, so the green
  shows under the load.
- **Snow keeps your tracks.** Every step through it leaves a print for
  up to twelve minutes, three in falling snow: a tell on you, and one
  the machines can read (`world::snow::Tracks`). Your breath shows in
  the cold.
- Gauges: `scripts/night_matrix.sh` also shoots storms dark and in a
  held flash, by night and by day. A flash must light the near ground
  well above the dark storm's, brighten the sky, and blow out no more
  than a sixth of the frame. The budget test has a storm and a snow
  case.

## Per valley

| Valley | Mood | Air | Grade |
| - | - | - | - |
| **Marrow Creek** (the reference) | Wet, green-brown, lived-in, river mist | Neutral cool grey | Saturation back, slight contrast |
| **Kettle Pass** | Cold, high, rock and pine, wind | Blue-grey haze, cloud on the tops | Cooler, a touch harder, less green |
| **Cinder Works** | Burnt, industrial, dry under the rain, ash | Warm ash-brown haze, ash falling | Warm and drained |
| **Training Yard** | Practice: a clear, kind version of Marrow Creek | Neutral | A shade brighter |

## Legibility and comfort (VR)

- Nothing flickers or crawls: thin things (grass blades, wires, fences,
  far branches) are widened to hold a pixel, faded, or alpha to coverage
  where the headset can afford it, never 1-pixel aliased lines;
  procedural detail in shaders fades out before it would alias.
- Robots stand out from everything behind them at 30-60 m in drizzle:
  pale against the dark forest, dark-edged against the sky.
- The darkest areas the player stands in (forest floor under spruce)
  must still show texture.

## Frame budget

The headset machine is a laptop RTX 3060 at about 1800x1800 per eye.
Every change states what it costs: draw calls and triangles
(`EXTRACTION_RENDER_STATS=1`) and, for shaders, the work per pixel. The
grade is free (it rides the tonemapping pass); the sky shader runs only
on uncovered sky; the terrain shader is the one to watch.

## Checking

- `scripts/lookbook.sh [maps...]` shoots the fixed views in
  `scripts/lookbook/<map>.views` into `target/lookbook/` (about two
  minutes a valley, software-rendered).
- `python3 scripts/lookbook_sheet.py --compare BEFORE AFTER out.jpg`
  pairs them up. Keep a change only if it is clearly better there.
- `EXTRACTION_LOOK="sat=1.2,contrast=1.1,temp=0.01"` tries a grade
  without rebuilding (keys in `src/render/grade.rs`).
- Before and after shots of the bigger changes are in
  `docs/shots/visual-design/`.

## What the current look costs

Measured A/B against the commit before this work, with the session log's
`gpu:` line under the software renderer (relative only: lavapipe is not a
3060), at three views (meadow edge, spruce interior, pine slope): with
everything below, the opaque pass is within noise of the base (spruce
interior 850 vs 840 ms a software frame). Geometry is from
`EXTRACTION_RENDER_STATS=1`.

| Change | Where | Cost |
| - | - | - |
| Per-valley grade, shadow lift | tonemapping pass | None: the pass already runs |
| Sky deck | sky pixels only | 2D value noise, ~8 octaves plus 2 warp taps; cheaper than the old 10 3D octaves |
| Water reflection | water pixels | A few ALU; the PBR specular it replaces is dropped |
| Terrain macro colour, burn | every terrain pixel | 2 value-noise taps (~10 ALU), against 9 texture taps per layer already |
| Cloud on the tops | every lit pixel, Kettle Pass only | A second closed-form fog integral (~10 ALU); compiled out elsewhere |
| Alpha to coverage on foliage | foliage | **Off by default** (`EXTRACTION_A2C=1`): it cost the software opaque pass +40-70% wherever foliage filled the view. Time it on the headset machine before turning it on |
| Grass widening | grass vertices | ~10 ALU a vertex; no new geometry |
| Fuller pine crowns | pine near LOD | Densest pine view: tris drawn 321k to 332k, shadow 261k to 287k, draw calls unchanged |
| Weather wetness, sky weight | uniforms | None |
| Charred trees, tuft spread | vertex colour, geometry layout | None |

### Marrow Creek's forest (2026-10-28)

The playtest's walk to the logging camp ran at p50 37.9 ms a frame. Its
`gpu:` lines (both eyes and the mirror, on an RTX 3080) put the opaque
pass at 10-22 ms, roughly 5-7 ms an eye. That session's far trees were
missing (their impostor captures gave up), so with them back the forest
costs more than this.

Layers a pixel by kind, from `gauge_marrow_creek_holds_the_budget`
(1800 px an eye, the governor's settling point):

| View | Mean | Near tree cards | Far impostors |
| - | - | - | - |
| m05 spruce (inside the forest) | 7.2 | 4.4 | 1.0 |
| m13 pump hall | 5.7 | 3.7 | 1.4 |
| m06 forest edge | 4.8 | 1.9 | 1.7 |
| m08 pines | 3.7 | 1.4 | 1.1 |
| open views (gate, meadow, creek, rail) | 1.0-1.8 | 0.4 or less | 0.5 or less |

In the forest, the cost is the near, alpha-tested cards, not the far
forest: the impostors stay under 1.7 layers even at the edge.

Open, next steps:

- Time the near cards on the headset machine with and without the
  spruce's dark core drawn first: an opaque core in front should let
  early depth reject the cards behind it.
- Thin the inner cards of the near crowns past about 30 m, where the
  core already reads as dense.
- Check whether the impostors' far LOD can take the band from 68 m in
  to about 50 m without a visible swap.

Each needs a before/after on the headset (lavapipe times aren't a GPU's)
and Performance's governor numbers alongside.
