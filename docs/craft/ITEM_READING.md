# Item reading: a review procedure

Every item must be told apart from every other, and its family known,
from its shape and colour alone: at 1 m on a shelf (in reach) and at
3 m across a room, under the light most rooms in the Zone have. The
designer's first session: "bombarded with too many item types at once…
it felt muddy"; the yard's kit crate was "13 look-alike items".

This is the procedure. It's a living document: when a review teaches
something, write it here.

## 1. Render the line-up

`art/lineup.py` puts a family's items side by side on a shelf and renders
them as the headset sees them:

    blender -b --python art/lineup.py -- out.png 1 colour -- a.glb b.glb proc:bolt ...
    blender -b --python art/lineup.py -- out.png 1 grey   -- ...
    blender -b --python art/lineup.py -- out.png 3 colour -- ...
    blender -b --python art/lineup.py -- out.png 3 grey   -- ...

- **The eye's sharpness**: a 100° view on 1430 px, 14.3 px a degree, what
  the Beyond 2 renders in the middle of the view at the game's auto
  render scale (`docs/LEGIBILITY.md`). An item 10 cm across is about
  80 px at 1 m and 27 px at 3 m. Never judge a render scaled up smoothly:
  crop it, and enlarge only by whole pixels (nearest neighbour), so what
  you see is what the eye gets.
- **The light**: a room lit through a window to its left by overcast
  daylight (a large, soft, cool panel) and its own dim fill. No lamp, no
  rim light. An item has to read under the light most rooms have, not a
  studio's.
- **The shelf**: worn grey-brown board against a dull wall. Items stand
  as their models rest.
- **Grey**: the same render in greyscale. It tests silhouette and value
  without colour (and stands in for a dim room, and for colour-blind
  eyes). If two items only differ by hue, grey shows it.
- **Items built in code** (the bolt, the detector: `src/anomaly/visuals.rs`)
  are rebuilt from the same primitives with `proc:<id>`. Keep the two in
  step when either changes.
- **The sheet** (`art/lineup_sheet.py`): 1 m colour and grey cropped
  round the shelf, 3 m colour and grey cropped and enlarged 3x by whole
  pixels, the ids listed left to right. Sheets go in `docs/shots/items/`,
  before and after with the same camera.

Render the family together, and then the set a player meets together (a
crate's contents, a first house's shelves): confusion happens across
families as much as within them.

## 2. Look, in this order

1. **3 m, grey, first.** Squint. Count the shapes you can tell apart.
   Anything that's a smudge, or a twin of its neighbour, fails here.
2. **3 m, colour.** Does colour split what grey couldn't? Can you name
   each item's family from across the room?
3. **1 m, grey.** At reach, each item's silhouette should say what it is
   before its label does: a tin is a cylinder with a rim, a radio is tall
   with an antenna, a dressing is a soft packet.
4. **1 m, colour.** Labels, marks and the one distinguishing feature.
   Labels are bonus legibility at 1 m, never the only difference.

Write down, for the set:

- **pairs a newcomer would confuse** (at either distance, in either
  mode);
- **items that read as nothing** (a smudge, or so small or dark against
  the shelf it disappears);
- **items in the wrong family's colours** (a medical thing in tool
  colours).

## 3. What makes a silhouette distinct

- **The outline's big shape first**: tall, long, flat, round, boxy. Two
  items with the same proportions are twins at 3 m, whatever their
  detail. Change the proportion before adding detail.
- **One feature that breaks the outline**: an antenna, a handle, a cap,
  a strap, a lid. It must be big enough to survive 3 m: about 1.5 cm
  across (3 px) at the least, more if thin.
- **Value contrast against the shelf and the item's own parts**: a dark
  item on a dark shelf vanishes; a light band on a dark body reads from
  across the room.
- **How it rests**: an item that stands upright reads differently from
  one that lies flat. Use it.
- **Small things need a flag**: anything under about 6 cm needs a bright
  or high-value feature to be found at all (a coloured cap, tape, a tag).

## 4. Colour and value, by family

The Zone's palette is muted (`docs/ART_DIRECTION.md`); each family gets
its own corner of it, so a glance says what kind of thing it is.

| Family | Language | Value | Its mark |
| - | - | - | - |
| **Food** | warm: bare tinplate, paper labels in faded warm colours (ochre, brick, cream), brown card and paper packets | mid to light | the tin's rim, a label band |
| **Medical** | off-white and cream, light grey; a red cross or band | light: the lightest family | red on white |
| **Ammo** | ochre and brown card boxes, printed; brass rounds | mid, warm | brass, print |
| **Magazines** | blued steel, near black; red-brown bakelite | dark | the follower, the lips |
| **Tools** | bare steel with painted handles (red, orange, blue), dark canvas or leather rolls | mixed, metallic highlights | a coloured handle |
| **Electronics and gadgets** | olive drab and dark grey military kit; a pale dial or a lit lamp | mid to dark | a dial, an antenna, a lamp |
| **Power** | dark bodies with a bright metal or coloured end | dark with a light end | the end cap |
| **Anomaly kit** | signal colours: the bolt's flagging tape, the detector's dial | — | tape, lamp |
| **Curios** | personal and odd: saturated or patterned, old paper, glass | any | whatever makes it someone's |

Within a family, items are told apart by **shape**; across families by
**colour first, shape second**.

## 5. Measure what can be measured

A check is only worth having if it's honest. Bounds and mean colour can
flag twins (two items with near-identical proportions, size and average
colour), but they can't tell a dressing from a tin of the same size and
colour. The pictures are the real test; a check only catches a
regression on what a picture already found.

## 6. Reviews

### The kit crate (31 October)

`docs/shots/items/kit_crate_before.jpg`. Its fifteen kinds as the yard
shows them (`persist::fresh_save`, `onboarding::stash_station`), less
the pistol (the Gunsmith's): flashlight, power cell, PM mag, 9x18
rounds, radio, field dressing, canned stew, cleaning kit, spare parts,
map case, compass, detector, bolt, the hearth stone.

- **Muddy olive.** Six of fourteen are olive or sage (flashlight, radio,
  field dressing, cleaning kit, the map case's frame, detector): colour
  says nothing about family.
- **Confused pairs**: the field dressing and the cleaning kit (sage,
  rounded, a pale label: two pale-green lumps at 3 m); the power cell and
  the PM mag (thin dark uprights at 3 m); the detector and the radio
  (olive boxes with knobs: told apart only by the radio's height).
- **Reads as nothing**: the bolt (a 6 cm dark stick, gone at 3 m, and
  it's thrown and has to be found again).
- **Reads well**: the canned stew, the map case, the compass, the radio,
  the spare parts (black and steel), the hearth stone (it glows).

### The first-job houses' starter set (31 October, Onboarding's draft)

`docs/shots/items/starter_set_before.jpg`: canned stew, crackers, water
bottle, field dressing, splint, 9x18 rounds, power cell, matches, bolt,
scrap metal.

- **Food in medical colours**: the crackers are a sage box, the field
  dressing a sage packet: at 3 m a pale-green box beside a pale-green
  pillow, food and medicine the same.
- **Twins**: the 9x18 rounds and the matches, two small pale boxes.
- **Reads as nothing**: the bolt again, and the scrap metal (a small dark
  crumple), which is meant to be the first thing worth bringing home.
- **Barely there**: the power cell, a thin upright found by its glint.
- **Reads well**: the canned stew, the water bottle (tall, white, a red
  cap), the splint (its ladder shape).
