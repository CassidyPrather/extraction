# Gloves

Before/after (`0_*`: the old box gloves left, the modelled ones right) and
the gloves first-person, from `--gallery fp:WHAT[,ZOOM[,TURN]]` (see
`dev::first_person_hands`): open, from the back, pointing at the yard's
push button (the index tip on `poke::FINGERTIP`), on the PM's grip from
behind and from the left, on an AK (grip and handguard), and an AK
magazine in the left fist. Rendered without the texture scans (flat
colours), so the canvas weave on the fabric doesn't show here.

```bash
scripts/shot.sh out.png 40,19.2,262,0,-25 30 --no-robots --gallery fp:pm_pistol,1.6,90
```

`6_trigger_pulled.jpg`: PM, AK, Mosin, TOZ from the left with the trigger
pulled (`fp:<gun>,2.2,90,1`); the index curls to each gun's own trigger
(`render::props::gun_index_curl`). On the Mosin and TOZ the trigger sits
5–6 cm below the index's line, which no curl reaches.
`6b_mosin_toz_grip_lowered_rejected.jpg`: Mosin and TOZ before (left) and
with their layout grip points lowered ~4 cm so the trigger lines up
(right): the fist then hangs under the stock's wrist. Not kept.
`7_handles.jpg`: the right hand working a handle (`fp:<gun>+handle`):
the AK's charging handle, the Mosin's bolt knob (from ahead and from the
left), the PM's slide. The handle sits inside the closed fist.
`8_tracked_fingers_monado.jpg`: the left eye under Monado's simulated
headset with `SIMULATED_HANDS=1` (Monado patched with
`scripts/monado-simulated-hands.patch`): the gloves' fingers follow the
runtime's `XR_EXT_hand_tracking` joints, each finger at its own curl.
`9_sleeves_monado.jpg`: the same headset view with forearms and sleeves
(`render::arms`: two-bone IK from a shoulder on the torso to the glove's
cuff, elbow down and out). `9b_sleeves_flat.jpg`: flat mode, whose own
hand pose turns the cuffs sideways, so the wrists bend hard there.
`10_body_shadow.jpg` (`--gallery above`, training yard, clear sky): the
body's shadow on the road, joined at the feet (the shadow on the grass is
the signpost's). `10b`: the same with the shadow proxy drawn white for the
record (it's on a layer only the sun sees). `10c`: before, a glove's
shadow floating on the grass.
`11_tracked_thumbs_monado.jpg`: tracked fingers with the thumbs following
the runtime's own thumb direction (here open, standing up beside the
index).
`12_physics_ghost_monado.jpg` (Monado, training yard, with
`SIMULATED_REACH=1.1,0.1,-0.6,4,4`): the right hand is pushed into the
truck's side wall. The glove and sleeve stop at the wall's face, and the
ghost glove's faint rim shows the real hand inside the wall, over the
workbench panel.
`13_ghost_fade_monado.jpg` (Monado, training yard,
`SIMULATED_REACH=0.88,0.1,-0.6,2,3` left, `1.1,0.1,-0.6,2,3` right): the
ghost fades with the gap. With the hand only just at the wall, the glove
rests on the rail and there is no ghost. Pushed deep in, the ghost's rim
shows inside the wall beyond the stopped glove.
`14_body_side.jpg` (`--gallery body+side`, with `+stoop`, `+crouch`,
`--seated`): the body under the head (`rig::stance`, drawn by
`render::body`), from 2 m to the right, in flat mode (whose hands are held
up). Standing, the knees soft. Bent over, the hips stay put and the chest
leans out over them, the pouches, harness and pack leaning with it while
the holster stays on the hip. Crouched, the knees forward over feet that
didn't move. Seated, the hips under the belt (the slots stay at their
standing heights, as the seated view is lifted to a standing eye), the
thighs out ahead.
`15_body_eye.jpg` (`--gallery body+down`, `+stoop`, `+crouch`): the same
from the eye. Looking down, the chest rig, the jacket and the sleeves;
the chest rig keeps the feet out of sight standing.
`16_carrying.jpg` (`--gallery carry`, `carry+side`, `drag+side`, `pack`,
flat mode with the hands posed by the gallery; `interact::carry`): a car
door taken off the ground by its two ends and held up across you, from the
eye and from beside; the same door taken by its near end in one hand and
drawn back, the far end on the ground, scraping; and the pack swung off
the left shoulder, hanging open in front of you for the right hand to
reach into.
`17_climbing.jpg` (`--gallery climb+behind`, flat mode with the hands
posed and their holds put on by the gallery; `interact::hand_climb`): a
wall 2 m high, from behind. Reaching up to its top, gripping its edge
(`Hold::Edge`), and pulled up until the head is over it; a moment later
you're over and down the far side.
`18_fight.jpg` (`--gallery fight`, `fight+side`; `interact::fight`): a
crawler a metre ahead, the crowbar raised in the right hand to strike
(swung fast, its claw end lands a blunt blow), and the left palm laid on
the machine (where a shove or a grip would start).
`19_treated.jpg` (`--gallery treat+side`, `treat+down`, flat mode;
`interact::wounds`, drawn by `render::wounds`): a body hurt and treated.
From beside: an open wound on the right thigh, where it was hit, and the
splint's two slats strapped along the right shin. From the eye: the left
glove turned palm up, bloody from touching a wound (it fades over a
minute), an open wound on the left forearm, and a dressing wound round
the right upper arm as a white band.
`20_bleeding.jpg` (28 October playtest, item 12; `--gallery treat+side`,
then `treat+down+wrist`): before and after a bound limb that still
bleeds. Left, before: every open wound drawn alike. Middle, after: the
bleeding thigh wound wet and brighter (a stopped one draws dark and dry).
Right, after: the right upper arm bound, the wrist raised; it reads BLEED
and pages through the line said when the dressing went on, "That one's
bound. You're still bleeding: the right thigh." (the old line was "The
bleeding stops.").
`21_body_and_thumb.jpg` (28 October playtest, transcript M1 and M2). Top,
looking down (`--gallery kit+down`): before (left, `--body`, as the
headset showed it whatever the setting) the sleeves, the jacket and the
harness's straps, placed by guessing from a head and two hands; after
(right, the default) the gloves and what you reach for on yourself (the
pouches, the compass), nothing the IK can't back up. Bottom, the PM in
the right glove from its left side (`fp:pm_pistol,1.6,90`): before, the
thumb hooked across and into the frame; after, it lies forward along the
frame under the slide.
`22_gloves.jpg` (28 October playtest, M2: the hands are the most-seen
model; `fp:open,2.4,90` and `fp:open,2.4,0`): top before, bottom after
the first pass on `art/hands/gloves.py`. Rounder fingers (16 sides, fuller
caps) and a finer palm; a leather patch over each fingertip; a moulded pad
over each knuckle; the leather a shade lighter than the olive nylon, so
palm and back read apart. Still to come: the back of the hand's panels
and stitching, the thumb's shape, and the cuff.
`23_glove_back.jpg` (`fp:open,2.4,270`, the back of the hand, the side
you see most): left the glove before this wave, right after its second
pass. A padded panel over the back of the hand with dark seams down its
sides, a moulded pad on each knuckle, leather fingertips. (The block at
the wrist is the display's housing, on each glove.)

`24_glove_cuff.jpg` (`kit+down`, the default view, the cuff end to the
eye as when reaching forward): left the third pass's first try, right
after. The dome meant to close the cuff was scaled about a point back in
the hand, so it ran in as a funnel and read as a hole; it now closes
over the arm's end in leather. The third pass also tapers the thumb.

`25_side_opening_pack.jpg` (`--gallery pack`, `pack+side`, flat mode):
the pack swung off the left shoulder and held in front. Before, it hung
with its straps to you and opened at the top, the lid thrown back at
you. Now it's a panel loader: it hangs with its outer face to you,
tipped up toward the eyes, and opens there. The cavity is lined a shade
off the canvas, the dressing shows inside, and the front panel (with the
big pocket on it) folds down from its bottom edge.

`26_hips_upright.jpg` (`--gallery body+down+side`, BODY on): standing
and looking down, from beside. The hips stay under the neck, and the
belt, harness and pack sit over the planted feet. Before, the drawn
spine ran up to a neck that followed the eyes forward when looking down,
and walking let the hips hang back by up to 19 cm, so the body leaned
(the 28 October playtest). The jacket isn't drawn here because it lies
between the eyes and the floor (`render::clear_eyes`).

`27_two_hand_pistol.jpg` (`fp:pm_pistol,1.6,0`, `,90`, `,300`): the PM
in both hands (28 October playtest, M2: "I can't use both hands on the
gun"). The left palm is cupped under the grip, drawn on the gun's cup
point as the right is drawn on the grip, not at its controller a few
centimetres off (`render::props` test
`a_palm_cupping_a_pistol_is_drawn_on_its_cup`). From the eye, from the
left, and from below right. Still rough: the left glove's fingertips
pass a little into the right hand's at the front of the grip.

`28_support_hands.jpg` (`fp:pm_pistol,1.6,90`, `,0`, `fp:ak74,1.2,300`):
the supporting hand on the PM and on the AK, before (top) and after
(bottom). Before, the glove on a support or cup took the gun's own turn,
which wraps a grip standing up through it. Under the PM's butt that made
a fist round nothing, back to the gun: sausage fingers and a blob palm,
cruder-looking than the glove passes though it was the same model. Beside
the AK's handguard it was the same fist. After, a supporting hand holds
from below, palm up (`render::props::support_pose`, tested on every gun):
the PM's butt sits in the palm with the fingers forward under the gun
hand's, and the AK's handguard lies in the palm with the fingers up its
far side. The display's housing is now 7 mm thick and only on the
display's hand, and the cuff's end is dark again.

`29_support_and_handles.jpg` (`fp:ak74,1.2,300,0,60`, `fp:ak74+handle`,
`fp:mosin+handle`, `fp:pm_pistol+handle`): the AK's handguard from
below, lying across the supporting palm with the fingers up its far
side (`support_pose`). Then a hand working each kind of handle, drawn on
the handle where it has got to (`handle_pose`, tested on every gun):
overhand, the palm down on it, the fingers forward of a knob so a pull
back hooks it, the fist along a pistol's slide. Before, a hand on a
handle was drawn at its controller, turned however the hand happened to
be, a few centimetres off the knob. (The gallery's fifth `fp` field,
TILT, tips a gun to show it from below.)
`30_nagant_grip.jpg` (`fp:nagant,2.2,90,1`; below `fp:nagant,3.0,90,0` and
`,1`): top, the Nagant's new grip model held at the old grip point (left:
the fist closes on the air ahead of the grip) and at the new one (right,
Gunsmith 29287b4: the fist on the grip). Below, the index released (along
the frame, off the trigger) and pulled: `gun_index_curl` reaches the blade,
but at 116 mm ahead of the hand (the other guns' 63-89 mm) only with a
near-straight finger (curl 0.17; theirs 0.35-0.77), its tip hidden behind
the blade from this side.
`31_magazine_catch.jpg` (`fp:ak74,2.4,-90` / `fp:ak74+catch,2.4,-90`;
`fp:pm_pistol,2.8,90,0,25` / `+catch`): left as held, right with the
magazine release held. The AK's paddle (the model's `mag_catch`, just
ahead of the guard) is the gun hand's index's: it straightens forward off
the trigger onto it (`render::props::catch_index_curl`). The PM's heel
catch is past any finger of the gun hand: the thumb of the palm cupped
under the butt rises onto it (`catch_thumb`). Let go, each eases back.
`32_crossbow_redraw.jpg` (`fp:crossbow,1.5,90` and `fp:crossbow+handle,1.5,90`;
left before, right Gunsmith's ef8141d): the shorter draw a leaf-spring
prod gives (the string's rest from z -0.33 to -0.22) and the front grip
110 mm back. Top, the support palm under the new front grip, up into it;
below, the fist closed on the string at its new rest. Both follow the
layout (`support_pose`, `handle_pose`): nothing of the hands' changed.
`33_held_free.jpg` (`fp:<item>,2.2` and `fp:<item>+palm,2.2` for the
first raid's starter loot, in the left hand, Marrow Creek's grey daylight;
each before, after, and after on the upturned palm):
before, a thing held free sat wherever the hand took it, and the glove
curled by its smallest size: the fingers closed through a tin or round
the air beside a magazine. After, the glove is seated on it
(`render::hold::seat`): the palm on it, each finger closed until it
touches, the thumb laid on a side. The gallery takes each thing as a
hand picks it up (`render::hold::taken`: its thinnest side to the palm). Small
things (a power cell, a bolt) lay on a hand still nearly open: see 34.
`34_small_gripped.jpg` (`fp:<item>+palm,2.2` and `fp:<item>,2.2`; left
the seat of 33, middle and right now): a small thing (`render::hold::small`:
a closed hand goes round it) must be gripped, the thumb against a finger
and the fingers on it curled at least `GRIPPED` on average, or it isn't
held: the fingers close round the power cell, the bolt, the box of
rounds, the magazine and the matches rather than lying flat under them.
`35_first_hour_eye.jpg` (the yard by day, as one headset eye sees it:
`EXTRACTION_FOV=100`, `--size 1000x1000`, `--camera 0,0,40,0,-25`,
`--gallery fp:read`, `fp:pm_mag`, `fp:field_dressing`, `fp:radio`,
`fp:radio+palm`): the first hour's hands. The job card held up to read
stood on the glove's fingertips like a tray (top left); a thin flat
thing is now held only pinched by an edge (`render::hold::thin`), and
the glove takes the card's bottom edge between thumb and fingers (top
middle). The crate's magazine, the dressing and the radio read right at
the eye. (`fp:read`, new, holds the job card up as `interact::read`
does; the `fp` gallery now stands a 0-height camera's eye 1.7 m up, as
the camera does: it had set the hands out round the feet.)
