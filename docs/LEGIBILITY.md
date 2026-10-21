# Legibility

The first headset session (SteamVR, Bigscreen Beyond 2, at a reduced render
scale for frame rate) found small text very hard to read. This file sets
the floors every piece of text in the game is held to, says why, and says
how they're checked. The code is `src/render/legibility.rs`.

## The floors

Text is measured by its **smallest letters' height as an angle at the
eye, from where it's read**: the x-height when the text has lower case,
the cap height when it is all capitals (digits count as capitals).

| Kind | Floor | At 14 px/deg | What |
| - | - | - | - |
| Must read | **0.55°** | ~8 px | instructions, labels you act on, numbers you dial, names you navigate by |
| Incidental | **0.35°** | ~5 px | flavour: stampings, serials, the memorial's names, tins' labels |

and every stroke at least one eye pixel wide from where it's read.

"Where it's read" is a reading pose, set per source in the registry: the
wrist raised like a watch (0.4 m), a radio or map held up to the face
(0.3–0.35 m), a card or terminal within reach of its button (0.7–0.75 m,
a terminal looked down at, 35° off square), a note on a table leant over
(0.65 m, 20°), a signpost from a few steps (3 m). Text seen at a slant
counts its foreshortened height. Anything small you must read is
something you can hold and bring to your face, or reach.

## Why these numbers

**The display.** The Beyond 2 has 2560 x 2560 micro-OLED panels per eye
and Bigscreen quotes 32 px/deg at best, over about 108° x 96°. But the
panel is not what limits text: the render target is. Each eye renders to
a flat (rectilinear) image the runtime resamples onto the lenses, and a
flat image has its *fewest* pixels a degree in the middle, where you read.
The runtime asked for 2980 px across; our auto render scale aims for 1800
(`Comfort::AUTO_EYE_PX`, about 60%). Over a frustum about 96° across
(tan-span 2.22) that is

  1800 / 2.22 px per radian = **14.3 px/deg** in the middle of the view.

(The old rule assumed 20 px/deg; that is closer to 100% render scale.)
The session log's `eyes:` line says the real size each run.

**What 14 px/deg can show.** Hoffman et al. (2019), on a 13.8 px/deg
headset, found the legibility threshold for high-contrast letters at
45 arcmin of letter height and the comfortable size at 70 arcmin (as cited
by Kilpeläinen & Häkkinen 2023). Taking those letters as capitals, their
threshold is an x-height of about 0.55°. Solum (2019) recommends at least
0.8–0.9° (14–16 dmm) of type for VR text and 1.2–1.3° for comfortable
reading; with a 0.53 x-height font that is 0.45° x-height minimum and
0.65° comfortable. Google's VR guidance (I/O 2017) uses 1.375° type. A
headset resamples twice (render, then lens warp), so it needs more
pixels per letter than a monitor: on a monitor 5–6 px of x-height reads,
in a headset about 8. Comfortable reading wants about twice the
threshold (Kochurova et al. 2015).

So: **must read 0.55°** (the threshold, about 8 px of x-height at the
auto scale, at least Solum's minimum), and **incidental 0.35°** (about
5 px, the fewest a lower-case letter can be drawn in and stay distinct:
readable with effort or by leaning in). Bigger is better wherever it fits;
0.75° is a good target for anything read often.

The old floor, `MIN_EM_DEG = 0.6` for the *em*, was an x-height of 0.32°
(4.5 px): below even the incidental floor.

**Why x-height.** Lower-case letters are told apart by their x-height
part; a font's em includes ascenders, descenders and leading and says
little about how big letters look. The panels' font (Fira Mono, Bevy's
default) has an x-height of 0.528 em and caps of 0.69 em, so all-capital
text gets 30% more letter for the same em: short labels on LCDs and
cards are in capitals for that reason.

**Strokes and contrast.** The panels' font is Bevy's built-in Fira Mono
in its Medium weight (500), not Regular: at 8 px of x-height its stem is
about 1.5 px, which the lens resample softens but keeps. Bold (700) would
thicken stems by a third and start to close the counters of e, a and s at
that size; the render gauge reads every panel at Medium, so it stays. Light letters on dark
glow (the LCDs and CRTs are self-lit, so they read at night); dark
letters on paper need light on them: the paper is matte and lit by the
scene. Contrast on every panel style is above 7:1.

**Texels.** A panel's texture must give each letter enough texels (at
least 8 across an x-height) without being so fine that, unfiltered, it
sparkles from where it's read. Panels are drawn at 1000 px/m; see
"Filtering" below.

## The registry

Every source of text and fine symbol in the game is listed in
`src/render/legibility.rs`:

- **Panels** (`ScreenText`): each carries a `TextSource` component; each
  source has its reading pose and kind (`TextSource::reading`). A panel
  without one logs an error and fails the gauges.
- **Models with letters** (`art/**/*.py`, the stencil `text()` helper):
  each script is listed in `ART` with its reading pose. A script that
  draws letters and isn't listed fails the gauge; so does a `text()` call
  whose height isn't a plain number.
- **Textures with letters**: the field map (`scripts/field_map.py`, which
  states its lettering in millimetres on the sheet).
- **Fine symbols without letters** (scope reticle, compass needle,
  memorial tallies, rear-sight ticks, the PSO's drum ticks): listed below;
  none carries text to read.

### Inventory

| Source | Where | Kind | Read from |
| - | - | - | - |
| Wrist readout | `render/comfort.rs` | must | 0.4 m, wrist raised |
| Radio screen | `render/comfort.rs` | must | 0.35 m, held up |
| Bench display | `raid/bench.rs` | must | 0.9 m |
| Notes | `world/notes.rs`, `world/scenes.rs`, `raid/board.rs` | must | 0.65 m, 20° |
| Signposts | `world/notes.rs` | must | 3 m |
| Terminals | `world/notes.rs`, `world/scenes.rs` | must | 0.7 m, 35° |
| Driver's note | `render/memorial.rs` | must | 0.8 m |
| Memorial list | `render/memorial.rs` | incidental | 0.8 m |
| Camp title, sheet | `camp.rs` | incidental, must | 1.2 m, 1.0 m |
| Cards (camp, comfort, route, yard) | `camp.rs`, `raid/comfort_board.rs`, `raid/route.rs`, `onboarding` | must | 0.75 m |
| Logbook, request, order cards | `raid/logbook.rs`, `raid/requests.rs`, `progression` | must | 0.75 m |
| Cork board: pinned notes, columns | `raid/board.rs` | must | 0.6 m, 0.8 m |
| Kitchen labels | `raid/kitchen.rs` | must | 0.8 m |
| Depot lock, epilogue | `capstone.rs` | must | 0.8 m |
| Yard boards, training list | `onboarding` | must | 2 m |
| The yard's first board (in the truck) | `onboarding` | must | 1.2 m |
| Gun, magazine stampings | `art/guns`, `art/mags` | incidental | 0.35 m, in hand |
| Ammo boxes | `art/items/ammo.py` | must | 0.35 m, in hand |
| Keys | `art/items/curios.py` | must | 0.35 m, in hand |
| Tins, grenades, materials | `art/items` | incidental | 0.35 m, in hand |
| Truck, bench, stash crate lettering | `art/vehicles` | incidental | 0.7–1.5 m |
| Depot stencils | `art/props/depot.py` | incidental | 5 m |
| Field map | `scripts/field_map.py` | must | 0.3 m, held up |
| Flat-mode HUD | `hud.rs` | (monitor only) | — |

## The checks

1. **Unit gauges** (`cargo test legibility`): every panel in every valley,
   the yard, the camp and the truck, as fitted to its words, clears its
   floor from its reading pose; the wrist and radio at their fullest;
   every model's stencil letters and strokes; the field map's lettering
   and texels. Other workstreams' own tests (the yard's boards at every
   stage, the cork board's every card, the longest request and logbook
   page, every note in every valley) use the same `check_panel`.
2. **Render gauge** (`scripts/legibility.sh [map] [out] [--camp]`): the
   game renders every panel from its reading pose with a field of view
   that gives 14.3 px/deg (the auto render scale), the crop is blurred by
   half a pixel for the compositor's resample, and tesseract reads it
   back. A must-read panel that reads back under 75% of its characters,
   or an incidental one under 50%, fails. Without tesseract a
   stroke/contrast proxy stands in. Contact sheets of the crops are in
   `docs/shots/legibility/`.

OCR is harsher than a reader (no context, no second look): a panel OCR
reads, a person reads.

### Fine symbols (no letters)

| Symbol | Size where seen | Verdict |
| - | - | - |
| Scope reticle (PU, and the PSO) | 256 texels over the lens's 26°: a texel is ~1.5 eye px; lines 2 texels, the post 6 | clear |
| Scope picture | 384 px over 26°: ~15 px/deg, the eye's own | matches the eye |
| Compass | red needle 4 x 38 mm on a cream face, in hand | clear; no letters to read |
| Rear-sight ticks, PSO drum ticks | 1–2 mm enamel, gun at the face | incidental |
| Memorial tallies | 7 x 75 mm chalk strokes on the front wall | clear |

## Filtering

Panel textures are drawn once and kept. A texture seen with more texels
than eye pixels for it (a sign from up the track, a note at a slant)
sparkles as the head moves unless it has mipmaps. Each panel's picture is
read back once drawn and shown with a full mip chain (averaged in linear
light) and 16x anisotropic filtering (`render::screen::mipped`); while it
redraws, the live render target shows.

## Changes

What the first pass found and changed:

- **The floor** was an em of 0.6° assuming 20 px/deg: an x-height of
  0.32°, 4.5 px at the auto render scale's real 14 px/deg. It's now the
  x-height floors above, over every text source.
- **Wrist and radio** were the worst (0.21° and 0.23° x-height; OCR read
  nothing). Both now show capitals, a few words a line, and page a long
  message every 3 s instead of shrinking it; the wrist panel is as big as
  its housing (78 x 48 mm). 0.59° and 0.6°.
- **Panel styles** may set bigger type: an LCD's few lines fill its
  screen, a paper sign's short text is as big as a ten-line page's.
- **Notes** on a sheet a little over A4 (0.26 x 0.36 m); the logbook card
  0.36 x 0.30 m, so its career page fits.
- **Indoors went black.** The room's dimming of the ambient and the
  underground's darkening ran in no set order; once you'd been near an
  underground site the ambient drained to nothing within a few rooms,
  for the rest of the raid (`render::grade`, `world::underground`). The
  dimming now runs after; a test holds every ambient writer to an order.
- **Glare.** LCD and CRT panels were glossy; the bench display caught the
  canopy lamp's highlight over its words. They're matte now.
- **Shimmer.** Panels have mipmaps and anisotropic filtering (above).
- **Field map** lettering is sized in millimetres for reading held up
  (names 6.4 mm, labels 6.2 mm, was 3.6–5.2 mm), upright instead of italic,
  with a paper halo where it crosses roads and contours.
- **Reading close.** Notes (on tables, in scenes, the driver's note,
  the memorial list, the epilogue), the logbook and the request card can
  be taken up: squeeze on one and it comes into your hand facing your
  eyes; bring it to your face; open the hand and it goes back
  (`interact::read`). The logbook got a push button for its pages; the
  request card already had one. Control cards (comfort, route, camp)
  still press on a squeeze: their labels read from arm's length.
- **Seen, passed on:** the left sleeve's hem can cover the wrist panel's
  elbow end (Hands); a loot item lies across the Marrow pump-station note
  (Story/Map design).

Before and after, as the render gauge saw them: `docs/shots/legibility/`.

## Affordances

Can someone new tell what can be taken, pressed, pulled, turned or
climbed? `src/render/affordance.rs` lists every kind of interactable with
the cues it gives (a contrasting colour, a light, a shape you know, the
ring round it when an empty hand is near or points at it, the
controller's tick and click), and tests hold every kind to:

- at least two cues, at least one seen before you touch it;
- the part you take hold of at least 1° across from 1.5 m (about 14 eye
  pixels: 2.6 cm);
- its claims true in the game (a kind that says the ring shows is where
  the hand's hover finds it);
- every interactable in every valley, the yard and the camp a known kind
  (a new `Pressable` doesn't compile without one; a new grabbable thing
  fails the test).

What it found and what changed:

- **The near hover lit only the glove**, a faint green, and the thing
  itself never showed it could be taken. The ring that marks a far-grab
  target now also marks what an empty hand would take where it is
  (`render::props::far_cue`).
- **The furnace's levers** (and the underground's breakers, valves,
  pumps and mine tubs) wore the bench lever's 16 cm black stub: every
  run-lever got it. The bench keeps its own; the furnace's levers are
  wall levers you can see across a room (yellow plate, dark arm, red
  knob); fixtures with a look of their own get no stub.
- **The truck's bell pull**'s toggle was the colour of its rope: it's
  red now.
- **Climbing ropes** were 2.2 cm and tan (0.84° from 1.5 m, lost in rock
  and trees): 3.2 cm and climbing-rope orange.
- Doors, the folding shield and mine tubs are on the world layer and
  get no ring; their shapes carry them (handles, a tub on rails).

## Wayfinding

Can you find your way without a HUD? `--wayfind DIR`
(`scripts/wayfind.sh MAP [OUT] [--weather fog] [--hour night]`) stands
where you get your bearings (where the truck sets you down, every
rendezvous), turns to each landmark in range (a placement at least
`map_lint::LANDMARK_HEIGHT` tall, within `CLEAR_SIGHT`) at the headset's
pixels a degree, and shoots it with and without the landmark, time
frozen. What differs, inside the landmark's box in the picture, is the
landmark as the eye gets it through fog, trees and dark. It counts if it
covers 25 pixels at a contrast of 0.08, or its lamp shows (4 pixels at
0.4). Every vantage needs one.

Marrow Creek, first run:

| | drizzle | fog | night |
| - | - | - | - |
| Vantages with a landmark | 4 of 4, two of them barely (0.08–0.10) | 1 of 4 | 2 of 4 (two lamps the models already had) |

Landmarks past about 200 m in drizzle, and all of them at night, had a
few percent of contrast left: the map lint's line of sight says they're
there, the fog and the dark say otherwise. So:

- **Lamps on the landmarks** (`render::landmarks`): everything tall
  enough to steer by carries a steady red lamp at its highest point, as
  masts and chimneys do (red and steady: the drones' beacons are amber
  and blink). A lamp a few pixels across outshines the fog and the dark.
  With them (first at 0.22 m, since enlarged to 0.35 m): drizzle 4 of 4,
  night 4 of 4. Fog stays hard past 150 m,
  which is what fog is for (`map_lint::FOG_SIGHT`).
- **The compass needle** was red from end to end: nothing said which end
  was north. Red to the north, dark steel to the south.
- **The field map** reads (above); its routes are the map's own roads.
