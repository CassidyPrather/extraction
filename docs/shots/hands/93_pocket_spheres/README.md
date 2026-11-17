# Pocket spheres: a proposal (#93)

There's no code for this yet. These pictures come from a throwaway
prototype that isn't on any branch. They were rendered on the software
renderer at 3200×1800 and cropped, with no other changes, as lossless
PNGs. The gallery's view is `--gallery kit`: the standard kit is put on
the body (a PM pistol in the holster, two AK magazines and a PM magazine
in pouches, a Mosin clip in the left hip pocket, a compass on the mount).
The eye looks down at the belt from the chest.

The body is off, which is the default, so the pouches float. The pack is
hidden in these pictures: in this staged view it hung open in front of
the chest and covered the pouches.

| Picture | What it shows |
| - | - |
| `1_far.png` | Something held (a PM magazine, right hand, out in front). Every slot that would take it shows a faint hollow sphere: here, the one empty pouch. Every slot with something in it shows a faint sphere with a small white silhouette of what's in it, turned to the eye. Slots that would never take it, like the sling, vest and head, show nothing. |
| `2_near.png` | The hand about 25 cm from the empty pouch. Its sphere has grown and brightened a little, in proportion to the distance. |
| `3_hover.png` | The magazine inside the pouch's catch radius: it would go in if let go. The sphere is bright, a third larger, and pulses gently. |
| `4_full.png` | The hand at a full pouch, the one with the PM magazine. That sphere stays faint and shows its silhouette, with no hover: letting go there wouldn't stow. |
| `5_holster.png` | A PM pistol held at an empty holster: the holster's sphere is in hover. |
| `7_lefty.png` | The same, left-handed: the holster and mount are on the left side (`body::handed_slots`), and the spheres go with them. |
| `8_onehand_load.png` | One-handed mode: the pistol put to the pouch with a PM magazine. One-handed, touching a gun to a magazine on your body loads it (`weapons::flat::touch_to_combine`). That pouch shows the hover look, because something will happen there. |
