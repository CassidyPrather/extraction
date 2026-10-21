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
(0.3–0.35 m), a card within reach of its button (0.75 m), a note on a
table leant over (0.9 m, 25° off square), a signpost from a few steps
(3 m). Anything small you must read is something you can hold and bring
to your face.

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

**Strokes and contrast.** At 8 px of x-height a regular weight's stem is
1–1.5 px, and the lens resample halves its contrast. Light letters on dark
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
| Notes | `world/notes.rs`, `world/scenes.rs`, `raid/board.rs` | must | 0.9 m, 25° |
| Signposts | `world/notes.rs` | must | 3 m |
| Terminals | `world/notes.rs`, `world/scenes.rs` | must | 0.8 m |
| Driver's note | `render/memorial.rs` | must | 0.8 m |
| Memorial list | `render/memorial.rs` | incidental | 0.8 m |
| Camp title, sheet | `camp.rs` | incidental, must | 1.2 m, 1.0 m |
| Cards (camp, comfort, route, yard) | `camp.rs`, `raid/comfort_board.rs`, `raid/route.rs`, `onboarding` | must | 0.75 m |
| Logbook, request, order cards | `raid/logbook.rs`, `raid/requests.rs`, `progression` | must | 0.75 m |
| Cork board: pinned notes, columns | `raid/board.rs` | must | 0.6 m, 0.8 m |
| Kitchen labels | `raid/kitchen.rs` | must | 0.8 m |
| Depot lock, epilogue | `capstone.rs` | must | 0.8 m |
| Yard boards, training list | `onboarding` | must | 2 m |
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

## Filtering

Panel textures are rendered once and kept. Seen from farther than they're
read, a texture with more texels than eye pixels sparkles as the head
moves unless it has mipmaps. See "Changes" for what's done about it.

## Changes

(See the commit history; summarised here as the work lands.)
