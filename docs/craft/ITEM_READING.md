# Item reading: a review procedure

> **Where Models stopped (2 November), and what's next.**
> Pushed and gated, on claude/polish-models: the crossbow bolt at 0.30 m
> (fd6c6cd5, riding with Gunsmith's tip: their in-flight bolt and
> crossbow.glb rebuild go with it). On main before that: the truck
> crate's radio face up; the house plans (farm, post office, plank B/C);
> the labels' shared `LIFT` and `art/check.py`'s item gap; the stencil
> glyphs and `art/check_glyphs.py`; the walk props (log stack, wrecks,
> stone ring); the robot line-ups.
> Next, when work resumes:
> - The guns' labels pick up the fixed glyphs when Gunsmith next builds
>   them; the depot's conveyor and cradle (a C in their codes) want their
>   owner's rebuild (rebuilt here they lost their rollers' baked AO).
> - ~~Lettering at 2 m in game~~: done 4 November (the crate reads; the
>   gate blocks' ivy kept off their lettering; below).
> - ~~The truck crate's later kinds at reading distance~~: done 4
>   November (the magazine on its side; the rounds out from under the map).
> - ~~The walk props past the first jobs~~: Map design's walk reviews
>   cover them and send model requests (Sallow's viewing bay, Rime's snow
>   poles); the first-job interiors in Kettle, Rime and Sallow are done
>   here (4 November, below).
> - ~~The first raid's loot in Marrow's first-job houses, in game
>   light~~: done 4 November (below, "The first raid's houses").
> - Cinder's Ash Row at raid 0 (the room cameras I had predate the map's
>   move; Ash Row is now at (-34.5, 108.2)).

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
- **How it rests is part of its silhouette**: a cell lies, a magazine
  stands. Set the model's rest pose and its catalog `dims` to the way the
  thing lies on a shelf, and check a dropped one comes to rest that way.
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

### After the first fixes (31 October)

`docs/shots/items/kit_crate_after.jpg`, `starter_set_after.jpg`, same
camera:

- **Field dressing**: pale, a red cross on top. Now the lightest thing in
  either set; no twin.
- **Crackers**: brown kraft paper. A warm box beside a white packet, food
  beside medicine.
- **Cleaning kit**: dark canvas roll, leather straps, steel showing.
- **Matches**: a blue label; no longer a second small pale box.
- **Scrap metal**: a bigger blue-painted plate with bright edges; it reads
  from 3 m.
- **Power cell**: dark with a copper band, and now lying on its side (its
  model along Z; every bay's seat turned to match: `gadgets`'
  `CELL_ALONG` and `CELL_UPRIGHT`, the radio deck's). A low round shape
  with a bright end beside the magazine's tall dark one: they read apart
  at 3 m in grey too. A dropped cell comes to rest lying down
  (`vr_harness::tests::a_dropped_cell_comes_to_rest_lying_down`).
- **Bolt** (Anomalies', `src/anomaly/visuals.rs`): orange flagging tape
  knotted round it, tails trailing. At 3 m an orange fleck in colour;
  in grey it's still tiny. It's found by colour, which is what a thrown
  bolt in grass needs; its size stays as it is.
- **Detector** (Anomalies'): a signal-yellow face plate on the survey
  sets. No longer the radio's twin, in colour or grey.

Sheets re-shot on main 2ee65c62 (the same camera, `art/lineup.py` now
building the bolt's and detector's new looks as the game does).

### The shared lettering font (1 November)

`docs/shots/items/water_bottle_label_before_after.png` (in hand, pixels
x5): the font's В was the 8's strokes, so the water bottle read "8ОДА".
The font's twins are fixed (`art/check_glyphs.py`, run with
`art/check.py`); the labelled items rebuilt (only those whose geometry
changed: the ammo boxes, tins, packets, tapes and records). At in-hand
sharpness a 1.3 cm label is still soft: labels stay bonus legibility,
never the only difference. Not rebuilt: the depot's conveyor and
cradle (rebuilt here they lost their rollers' baked AO; their only
change is the C in their codes) and the guns (Gunsmith's, mid-edit).
The rice bag's and sugar box's label z-fighting predates this.

### Prints that stand off their faces (1 November)

`art/check.py` flagged label z-fighting on the rice bag and the sugar box
(the coordinator: check the class). No item shared a lift: each script
laid its prints, labels and bands 0.2 to 0.8 mm off the face under them,
and the ones with a big overlap fought (the rice bag's print 0.05 mm off
its label; the sugar box's bands 0.2 mm; the spare parts' lid label; the
splint's straps' undersides in the slats'; the car bonnet's brace arms
0.5 mm apart where they cross). Now a print, label or band stands
`art/items/_kit.py`'s `LIFT` (1.2 mm) off what it's on, and `art/check.py`
judges items at 1 mm (held 0.3 to 3 m from the eye; props stay at 3 mm).
All 192 item models check clean.

### The truck's crate, as a newcomer opens it (1 November)

The crate shows only what the first stations use (`onboarding::
stash_station`): the PM pistol (its magazine in it), the radio, and at
the first station the cleaning kit. In game, from the truck's bed, about
a metre away, under the room's light (`render::grade` `ROOM_ADAPT`):

- **The pistol**: laid on its side already (stood on its grip it was a
  stick); black and bakelite, its ring round it. Reads.
- **The radio stood up**: from above an olive stub and an aerial on the
  crate's dark floor, its face (grille, tuning window, lamp) unseen. Now
  it lies on its back, its face up (`persist::fresh_save`'s `turn`, as
  the pistol's): `docs/shots/items/truck_crate_radio_before_after.png`.
- **The cleaning kit**: a dark canvas roll, leather straps, brass rod
  ends: distinct on the boards (`truck_crate_first_three.png`, the kit
  shown early for the shot).

**How it rests is part of its silhouette**, again, and so is which face is
up: an item in a crate is seen from above.

### The truck's crate, all its kinds (4 November)

`docs/shots/items/truck_crate_all_before_after.jpg`: the crate as it
stands after the yard (every station's kit unfolded; shown so for the
shot, not in code), from the truck's bed at about a metre, the room's
light. Twelve kinds; ten read at a glance: the stew's lid, the radio
face up, the map case, the bolts' orange tape, the torch, the cleaning
roll, the detector's yellow face, the dressing's red cross, the compass
dial, the pistol. Two didn't:

- **The spare PM magazine stood up**: a dark sliver from above (its feed
  lips). It lies on its side now, its length and the top round showing.
- **The 9x18 rounds weren't there at all**: laid out under the map case
  (`fresh_save` had them a hand apart). Now out in the open between the
  torch and the dressing, the card box's print and its brass showing.

A crate's layout is part of reading its items: nothing under anything,
and each lying with its telling face up.

## The first raid's houses (4 November)

What a newcomer meets on their first raid, in game light (drizzle,
11:00), from 1.2 m: the eye standing in the room, turned to each loot
spot as it would walk up to it. The roll is a career's raid 0 (a fresh
`--save`; a scenario or a screenshot without a save rolls every
family). The houses roll 12 kinds then: ammo_9x18, bolt, canned_stew,
cleaning_kit, crackers, field_dressing, gun_oil, matches, pm_mag,
power_cell, tea_tin, water_bottle. Progression's stagger
(claude/polish-progression) keeps raid 1 the same: the PM family is the
only gun family until raid 3.

![farmhouse table](../shots/items/raid0_marrow_farmhouse_0.jpg)
![farmhouse kitchen](../shots/items/raid0_marrow_farmhouse_2.jpg)
![plank C shelf](../shots/items/raid0_marrow_plank_c_0.jpg)

- **The food reads.** The tea tin's red ЧАЙ on yellow, the stew's dark
  tin with its white band, the crackers' box, the water's ВОДА: each
  is its kind at 1.2 m, and none is mistaken for the table's own
  dishes (the plates and cups are white and plain; the loot is
  labelled).
- **The small things are specks, true to size.** The 9x18 box (4 x 3 x
  7 cm), the matches, the bolt and the power cell are a few pixels at
  1.2 m on a shelf. They are their real sizes, and the hands
  read them when picked up; nothing to fix in the models. If the
  first raid wants them found sooner, that's how loot is shown (a
  glint, a place they're kept together), not their size.
- **Nothing lies under anything.** The rolled loot stood clear of the
  shelves' books and the tables' dishes in every view.
- **Fixed: the canteen was a plastic bottle.** Named, drunk and scaled
  as a canteen (Hands' rename for the twist-cap drink), it was a clear
  PET bottle labelled ВОДА. Now it is an aluminium flask, its lower half
  in a canvas cover, under a black screw cap; the cap's place and axis
  are the old ones, so the twist handle still sits on it. Two lessons:
  a cover to the shoulders made it a dark olive lump on a dark table
  (the bare metal is what's seen), and the library canvas, laid a metre
  to a repeat, wove 5 mm cells on a 7 cm flask, a knitted net (fixed
  for every model in canvas, below):
  ![canteen, before and after](../shots/items/canteen_before_after.jpg)
- **Fixed: the dress form read as a giant blue bottle** (plank C's
  seamstress, beside the shelf the newcomer loots). It tapered straight
  up to a narrow neck, and the dress covered it to 1.4 m. Now the
  shoulders are square and wider than the waist, the neck carries a
  wooden cap, and the dress is pinned up only to the bust:
  ![dress form, before and after](../shots/props/dress_form_before_after.jpg)

### Ash Row at raid 0 (4 November)

Cinder's first houses, the same rig (raid 0, drizzle, 11:00, 1.2 m):
the plank house's table (a 9x18 box), its crate by the bed (two PM
magazines and a box), the shop's counter (tea, a glass), its shelves
(crackers, stew, a dressing with its red cross). The same reading as
Marrow's: the food and the dressing read, the PM family is small and
dark on a dark crate top, true to size (Progression's placement rule
puts first-raid small kinds on lit tops). The stool's yellow seat seen
from above is a disc; it is a stool.

**Fixed: canvas read as a knitted net, everywhere in reach.** Looking
down at the plank house's crate, the player's own rig pouches at the
belt were a quilted check: the canvas scan, laid a metre to a repeat,
weaves 5 mm cells. The gloves (six repeats a metre) and the rag cloth
(four) had already been scaled; plain `Canvas` hadn't. It is four
repeats a metre now (`models.rs`), for the rig, the sacks, the bags,
the bedroll and the canteen's cover alike; the canteen's own scale went
back to one.
![rig pouches, before and after](../shots/items/canvas_weave_rig_before_after.jpg)

### Lettering at 2 m (4 November)

Face-on from 2 m, standing, in drizzle at 11:00.

- **A supply crate reads.** Marrow's pump keeper's store: ТАРА over
  17-6, cream on the olive boards, every glyph its own.
  ![supply crate at 2 m](../shots/props/supply_crate_2m.jpg)
- **Fixed: the gate blocks' lettering was under ivy, in every valley.**
  The quarantine gate's blocks carry ФБС 24.3.6 on the face the walk
  out to the truck meets, which is their shaded north, the face ivy
  favours most. Marrow's, Kettle's and Cinder's were all sheets of
  leaves. `overgrowth::lettered` names a prefab's lettered faces and
  the ivy keeps off them; the rest of the gate grows as before
  (`ivy_keeps_off_lettering`). Kettle above, Cinder below:
  ![gate blocks, before and after](../shots/props/gate_block_ivy_before_after.jpg)

### The farmhouse with Progression's placement rule (4 November)

Progression's rule (claude/polish-progression, d525af94): on a
career's first two raids a Tiny or Small kind rolled at a spot out of
0.75–1.6 m moves to one in reach in the same building; a Tiny one
needs a flat top or the 1.25 shelf (the 0.80 shelf sits under a board,
shaded and seen edge-on). The same cameras at raid 0, main left, the
rule right (the roll differs; where each kind lands is the point):
![farmhouse, before and after the rule](../shots/items/raid0_farmhouse_placement_before_after.jpg)
The kitchen worktop now carries a matchbox and a dressing, lit; the
low shelf holds Small kinds (crackers, a dressing) only:
![the tops, close](../shots/items/raid0_farmhouse_placement_tops.jpg)

**Fixed: the canteen read black indoors.** From the door, two canteens
on the table were near-black bottles: their aluminium was `Zinc`,
0.9 metallic, a mirror of the dark room. They are `Aluminium` now, an
army flask's oxidised matte (lighter, half as metallic, rougher):
![canteen, zinc and aluminium](../shots/items/canteen_aluminium_before_after.jpg)

### The first jobs in Kettle, Rime and Sallow at raid 0 (4 November)

The same rig as Marrow's and Ash Row's (raid 0 from a fresh save,
drizzle, 11:00, 1.2 m from each loot spot), in the buildings of each
valley's first job: Kettle's Gatekeeper's cottage and its shed, Rime's
funicular valley station, Sallow's valve house. Main carries
Progression's placement rule now.
![first jobs, Kettle, Rime and Sallow](../shots/items/raid0_first_jobs_kettle_rime_sallow.jpg)

- **The placement rule holds outside Marrow.** Kettle's cottage keeps
  its food, canteens and tea on the table and the counter; the shed's
  bolt and power cell lie on the 1.25 shelf, the gun oil (Small) on the
  0.80 one; Rime's station has a power cell on its table, Sallow's valve
  house its kit on the table by the terminal and on the bench. No small
  thing on a low shaded shelf.
- **The models read.** The canteen in aluminium is a light flask on the
  counter, the tea tin and crackers their own, the terminal's green
  screen the brightest thing in the valve house. Nothing to fix.
- Still true everywhere: the 9x18 box, the bolt and a power cell are a
  few pixels at 1.2 m, true to size; the rule puts them where they are
  seen, which is the whole of it.

### Artifacts up close (4 November)

The Zone's most valuable finds, each lying 0.55 m off on the training
yard's road, fog at 11:00 (`--gallery art_<id>,30,0.55`). Left to right,
top to bottom: featherstone, hearth, glass eye; aegis, lodestone,
hushstone; stormglass, shard, rimestone:
![all nine](../shots/items/artifacts_all_before.jpg)

- **Read, and look valuable:** the hearth (black with glowing cracks),
  the aegis (a copper tangle), the shard (a blade of not-quite-glass),
  the featherstone (a dark pebble ringed in lilac), the glass eye (a
  marble with a highlight).
- **The hushstone** is what it says, a matte grey pebble: the one
  artifact you could take for a stone, by design.
- **Fixed (`anomaly/visuals.rs`, with Anomalies' go-ahead):**
  - *Stormglass and rimestone were twins*: two pale blue balls the same
    size. The stormglass is a clear bead with a dark cloud and a spark in
    it; the rimestone is pale stone furred with short white frost.
  - *The lodestone was a smooth violet ball*, not "a fist of dark iron".
    It is knobbled iron now, rough and barely metallic, with iron
    filings standing off its poles, their bright tips catching the light.
  - Each look inside its item's box; the bristles are one merged mesh per
    kind, built once and shared (fields hold several stones). Lessons:
    a lump's normals must be taken before its non-uniform scale (baked
    after, the lodestone went flat black); half-metal dark iron in fog
    renders black too.
  Before (top), after at 0.55 m (middle) and at 3 m (bottom):
  ![lodestone, stormglass, rimestone](../shots/items/artifacts_before_after.jpg)
- **At 3 m every artifact is a speck**, before as after: small things on
  stony ground. Their fields' light is what finds them.

### Every kind, and no placeholder (5 November)

**No item falls back to the placeholder.** The walker's plate was the
item placeholder's plain slab (`ROBOT_READING`), so a test now walks the
whole table: `models::tests::no_item_falls_back_to_the_placeholder`. A
firearm loads from `models/guns`, a magazine from `models/mags`, the
anomaly items are drawn by `anomaly::visuals` (`OWN_LOOKS`); every other
kind must have `models/items/<id>.glb`. It found 19 more slabs, now
modelled: the first-hour and contract things (the Zorki camera, Lev's
relay box, Holm's watch, Hajek's tin, the scour key, the pump log, the
deputy's lamp, the charging manifest, the intake ledger, Hana's letter,
in `art/items/story_items.py`), three pieces of kit (the spanner, the
work boots, the can suppressor) and six ammo lots (the subsonic rounds
and boxes, black over green tips and a blue band; the surplus lots, dated
and banded rust or tar).

**Every kind at 1.2 m**, rows of eight sorted by the first raid a kind
can appear on (from `loot.ron`; "-" for kinds no table rolls: kit, the
jobs' things, the cooked and the crafted, guns and their parts). Blender
line-ups (`art/lineup.py`, 1.2 m): the in-game rows (`--gallery
items:a+b+c`) were taken too, but the training yard's stations walk into
some frames and a cloak generator blacked one out, so the near-twin hunt
is done on the line-ups:
![every kind 1](../shots/items/every_kind_0.jpg)
![every kind 2](../shots/items/every_kind_1.jpg)
![every kind 3](../shots/items/every_kind_2.jpg)

- **Fixed: six cassettes were one cassette.** The tapes you find (for
  Anna, the harvest hymns, the lodge dance, the duty mix, our piano, the
  Sallow bells) were all the blank: grey shell, cream label, red band.
  Each has its own shell, label and band now and its title in block
  letters (`art/items/music.py`):
  ![tapes, before and after](../shots/items/tapes_before_after.png)
- **Near-twins left as they are:** a calibre's boxes (5.45 ball, AP,
  tracer, T-4, subsonic, surplus; the 54R's) share the packet and differ
  by band and tip colour, as real lots do; the depot keys differ by their
  stripe (yellow, red, cream) and their scorch and ash; the loose rounds
  are brass specks at 1.2 m by nature.


### By torchlight (6 November)

Most looting is done indoors in the dark by the hand torch, and the
user once said items felt muddy. `--torchlit all` (`capture::torchlit`)
lays every item with a model (230) on three floors in a closed dark
room: wood, concrete, and a burnt ruin's charred planks. The game's torch
beam is held at the eye and aimed at the item. Each is seen from an
arm's length (0.65 m, 55 degrees down) and from 2 m (40 degrees), at the
headset's pixels a degree. `scripts/torchlit.py` takes the item's pixels
from a second shot with the room hidden, and measures it in its own shot
against the floor round it:

- body: the median of its pixels' CIE delta E from the floor's colour;
- rim: its outline against the floor just outside, the upper quartile.

Under about 10 an item is the floor's colour. Run it per floor and read
the runs together:

    for f in wood concrete ruin; do
      target/fast/extraction --size 800x800 --map training_yard --no-robots \
        --torchlit $f --capture-out target/torch_$f
    done
    scripts/torchlit.py target/torch_wood target/torch_concrete target/torch_ruin \
      --json torch.json --sheet worst.jpg --worst 20

![The 20 worst by torchlight, each floor and eye](../shots/items/torchlit_worst20.jpg)

Three findings, by cause:

1. **At arm's length the torch washes the frame out.** The floor goes
   nearly white, even the charred planks, and black scopes, binoculars
   and the suppressor render a pale silver. Every item loses contrast
   and colour this way, and it is the likeliest cause of "muddy". It is
   the eye's exposure with a torch close to a surface (`render::grade`,
   `torch_light`), not any item: Visual design's.

   ![At arm's length and at 2 m: the same items, the floor blown out up close](../shots/items/torchlit_washout.jpg)

2. **Dark metal on a dark floor goes black from 2 m.** 15 of the worst
   20 are gun finishes on the charred floor: Blued, Black (blackened
   steel) and Steel. Examples are the PBS, both scopes, the weapon
   light, the laser, the binoculars, the PM and SVD magazines, the
   spanner, the Mosin's bolt and the multitool. A metal shows only its
   specular reflection, and with the torch at the eye and the item lying
   flat, little of it faces the eye. The finishes are Gunsmith's
   (`crates/gunsmith/src/finish.rs`). A trial on ten of them (the material
   swapped in the game for one run, nothing committed), delta E on the
   charred floor from 2 m:

   | item | as is | Black as a satin black paint (metallic 0, sRGB 0.06, roughness 0.45) |
   |---|---|---|
   | weapon light | 5.8 | 11.8 |
   | laser module | 5.9 | 10.3 |
   | binoculars | 5.9 | 10.7 |
   | side rail | 8.1 | 12.3 |
   | PBS | 4.4 | 11.1 |

   It also helped on concrete, and at arm's length (weapon light 19 to
   31). The same trial with bright Steel rougher (0.55) made the steel
   items worse (spanner 7.0 to 5.5, Mosin bolt 7.7 to 5.3): leave Steel as
   it is. A lighter, rougher blackened steel (metallic, sRGB 0.3, 0.6)
   helped about half as much as the paint did.

   ![Charred floor from 2 m: as it is (top), Black as satin paint (below)](../shots/items/torchlit_black_satin_trial.jpg)

3. **Small and pale.** The cooking's chopped vegetables are a pale board
   10 cm across with crumbs on it, lost on lit wood up close. A grenade's
   pin is 0.16 degrees from 2 m, a speck by nature. The pale card ammo
   packets sit at 9 to 12 on lit concrete at arm's length, which is the
   wash-out again.

Near-twins (size, outline and colour from 2 m on wood): most read apart
by shape once looked at. The two that don't, from 2 m, are the RGD-5 and
the F1 (both olive eggs; the F1's ribs show only up close), and the
laser module and the side rail (two small black blocks).

#### Fixed: the boards, the laser, the F1 (6 November)

The coordinator took the three that are the models' own. Measured again
with `scripts/torchlit.py`, the worst of each item's views:

- **The cooking boards** (chopped carrot, onion, tomato; the dough's)
  are oiled dark now, not scrubbed pale: worst 6.2-10.9, now 18.5-18.7.
  On lit wood up close, 6-7 is now 30, and on concrete 14 is now 40.
- **The laser module** has a wide red lens standing in its bezel and the
  red warning label on top of its front block: on the charred floor from
  2 m, 5.9 is now 11.1. On the twin measure it's now as far from the
  side rail (3.02) as unrelated things are (0.27 before).
- **The F1**'s cast segments are deeper (3.4 mm proud, not 2) with wider
  grooves between, so it reads as a grid of blocks from 2 m and not the
  RGD-5's smooth egg. The twin measure, which sees only size, outline and
  mean colour, can't tell them apart either way; the picture can.

![Before (top) and after (below), each in its worst light](../shots/items/torchlit_fixes_before_after.jpg)

#### The PU and the PSO-1 (6 November)

Once Gunsmith made the PSO-1 Black, the two scopes came near twins from
2 m. The real PU is a slim plain tube, its drums on a round saddle; it
had the PSO-1's squared housing here. It has the round saddle now, and
the PSO-1's brow shield sweeps wide and high over its eyecup. Twin
distance from 2 m (size + outline + colour, these models' finishes):
wood 0.54 → 0.65, charred floor 0.58 → 0.72, the rise in their outlines.
With Gunsmith's Black PSO-1 merged in, 2 m: wood 1.57 (Gunsmith measured
1.33 before the brow shield), charred floor 1.34; the PSO-1 reads 33.7 on
wood and 11.6 on charred floor.

#### Pale, not paper-white (6 November)

On Visual design's settled-eye sheet the flour sack was a blank white
blob at arm's length and from 2 m. Measured with Visual design's
settled-eye `--torchlit` applied locally (not merged), nothing clips,
before or after: the wash-out was exposure, and their fix took it. So
Paper and Enamel keep their albedo. A flat model material is held to
enamel's (linear luminance 0.78, `no_flat_model_material_is_paper_white`),
so nothing new comes in whiter.

What the sack lacked was surface and shape. Slumped into a lumpy ball,
it still read as a balloon at 2 m, its tie as a white cross. It stands
now as a filled sack does (`pillow_sack` in `art/items/food.py`). It is
a squat pillow lofted from rounded-rectangle rings, with near-flat broad
faces and a square base settled wide with dog-ear corners. Side tucks
and shoulder creases are geometry, and folds gather into the neck and
flare into a puff of the same cloth. Three wraps of dark twine and two
tails tie it off. Its stencil (МУКА, 1 СОРТ) and red stripes are on
both broad faces. Each line sits where the cloth stands furthest out
across its width, so no letter sinks (on the lumpy ball 1 СОРТ read
"1 ПРТ"). The material is `Sacking`, unbleached linen with the canvas
scan's weave.

By the settled-eye torch it reads 36.1 against the floors (36.6
before), with no pixel clipped. Detail (L* spread, worst view) is 15.0
(8.9 before), and it is larger from 2 m (5.1° to 6.2°: a pillow's broad
face, not a ball's). The other pale items (the hana letter, the rice
bag, the crackers) already carry a seal, grains or docking, and are
unchanged. The weave is too fine to read from 2 m. The stencil, the
stripes, the twine and the outline read.

![Flour sack by torchlight: the original, 0ce75fc9's slumped ball, and
now; wood and concrete at arm's length, wood and charred floor from 2 m
(enlarged)](../shots/items/torchlit_flour_before_after.jpg)

#### An item's box is its model (6 November)

An item's `dims` row is its box: its collider, its fit in a pocket or
pouch, and where it's grabbed. 78 rows had drifted from their models as
the models were redrawn. Among them, the vest was 3 cm too wide, the
work boots 7 cm too wide, and the Mosin bolt 4 cm too long. The robot's
belt of rounds had a box twice its height, and the electronics' model
stuck out of the bottom of its box.

`models::tests::every_item_box_is_its_model` holds every item with a
model and a row to it. Each side must be within a millimetre of the
model's, and the model must sit in the box's middle, within 2 mm. A
failure names each item, its row and its model.
`python3 art/items/dims.py --write` sizes the rows, and the kit centres
a model when it's built (`_kit.recentred`, its pivots moving with it).

Some items are checked differently:

- Guns are left out, because their layouts place them.
- Magazines and attachments seat by their origin, so only their size is
  held.
- Thirteen models stay off their middle until the code that places
  things in their space moves with them (`KEEPS_ORIGIN`, which names
  each one's code). These are the bolt cutters' grips and jaws, the
  crowbar's grip and claw, the hacksaw's blade, the cassette deck's
  well and buttons, and the gadgets' cell mounts and visuals.

Sixteen models were centred, and four `kit_cavities::ORIGIN_OFFSET`
entries that made up for them went away.

![Item boxes before (red) and after (green), over their models: vest,
work boots, robot ammo belt, Mosin bolt, electronics, crossbow bolt
bundle, car bonnet, scour key](../shots/items/item_boxes_before_after.png)
