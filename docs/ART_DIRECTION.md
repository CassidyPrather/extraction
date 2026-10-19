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

## Per valley

| Valley | Mood | Air | Grade |
| - | - | - | - |
| **Marrow Creek** (the reference) | Wet, green-brown, lived-in, river mist | Neutral cool grey | Saturation back, slight contrast |
| **Kettle Pass** | Cold, high, rock and pine, wind | Blue-grey haze, cloud on the tops | Cooler, a touch harder, less green |
| **Cinder Works** | Burnt, industrial, dry under the rain, ash | Warm ash-brown haze, ash falling | Warm and drained |
| **Training Yard** | Practice: a clear, kind version of Marrow Creek | Neutral | A shade brighter |

## Legibility and comfort (VR)

- Nothing flickers or crawls: thin things (grass blades, wires, fences,
  far branches) are alpha-to-coverage or faded, never 1-pixel aliased
  lines; procedural detail in shaders fades out before it would alias.
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

Per-pixel work is estimated from the shaders (the software renderer
can't time a GPU); geometry is from `EXTRACTION_RENDER_STATS=1`.

| Change | Where | Cost |
| - | - | - |
| Per-valley grade, shadow lift | tonemapping pass | None: the pass already runs |
| Sky deck | sky pixels only | 2D value noise, ~8 octaves plus 2 warp taps; cheaper than the old 10 3D octaves |
| Water reflection | water pixels | A few ALU; the PBR specular it replaces is dropped |
| Terrain macro colour, burn | every terrain pixel | 2 value-noise taps (~10 ALU), against 9 texture taps per layer already |
| Cloud on the tops | every lit pixel, Kettle Pass only | A second closed-form fog integral (~10 ALU); compiled out elsewhere |
| Alpha to coverage on foliage | foliage | Same pass and cost as the alpha test it replaces; shadow casters cut at 0.05 |
| Grass widening | grass vertices | ~10 ALU a vertex; no new geometry |
| Fuller pine crowns | pine near LOD | Densest pine view: tris drawn 321k to 332k, shadow 261k to 287k, draw calls unchanged |
| Weather wetness, sky weight | uniforms | None |
| Charred trees, tuft spread | vertex colour, geometry layout | None |
