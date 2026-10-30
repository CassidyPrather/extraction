# Model correctness: before and after

The 28 October playtest's M3 and M4 instances, each with what changed. Above
or left is before, below or right is after. Model previews are Cycles
renders (`art/preview.py`), and in-game shots are `scripts/shot.sh`. The
lints that hold each class are in `docs/ASSETS.md` ("Model correctness").

| Shot | Instance | What changed |
| - | - | - |
| `m3_pm_magwell.jpg` | The magazine stood through the pistol's grip (0:08:31) | The PM had no well: frame and grip were solid round the magazine. A well is now cut along the `magwell` seat, and the seat is 3 mm lower, so the lips sit under the slide. |
| `m3_flashlight_cell.jpg` | The cell crossed the flashlight sideways (0:11:13) | The cell stood across the tube through an open bore. It now lies along the tube in an open cradle on top. The weapon light and the laser got the same change. |
| `m4_hacksaw.jpg` | "Super primitive" hacksaw (0:42:31) | Three rods and a strip became a bent flat-tube frame, a shaped grip, a toothed blade on its pins and a wing nut. |
| `m4_pillow.jpg` | A capsule for a pillow (0:24:17) | The squashed sphere became a stuffed pillow, puffed in the middle, pinched to its corners and piped at the seam. |
| `m3_window_latch.jpg` | The latch floated by an open window (0:22:34) | The broken sash's open casement left its latch where it had shut. Each latch is now on its own casement. The island lint (`attach`) holds this. |
| `m3_door_hinge.jpg` | The door's hinge stood off the building (0:41:50) | The strap hinges' knuckles were 34 mm in front of the axis the game swings the leaf about. They're on it now, each linked to its strap. |
| `m4_small_props.jpg` | Carrots, dressing, grenade and matchbox too small (0:25:25, 0:51:54) | Same camera distance before and after. The carrots are now 2.9x (they were 0.36x a real bunch), the dressing 3.0 x 1.6 x 2.0 (the IPP-1's proportions), the grenades 1.2x and the matchbox 1.25x. 33 items in all; the table is `src/capture/scale.rs`. |
| `m3_camp.jpg` (left: the playtest, right: now) | The stool floated, the crate went into the ground, the table had legs in the air (0:33:38) | The stool and box are separate props, each tilted to the ground under its feet. The table stands on the lowest ground under its legs, and the camp picks the evenest spot. |
| `m3_truck_rear.jpg` (left: the playtest, right: now) | The mud flaps hung off nothing (0:13:02) | Each flap hangs from an arm off the frame. The rear ladder is its own node and hangs plumb, and a plank ramp reaches the ground behind. The truck stands on its wheels only on near-level stops; the yard's and the valleys' stops fall 1 to 1.4 m, so it's still settled low there (see `docs/ASSETS.md`). |
| `m3_pm_in_game.jpg` (left: the playtest, right: now) | The magazine through the pistol (0:08:31) | In the game, the seated magazine sits in its new well, its base plate flush under the grip. |
