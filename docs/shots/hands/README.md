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
