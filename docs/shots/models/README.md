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
| `m4_crude_props.jpg` (top: before, bottom: now) | The crudest props seen up close, by triangles per metre (M4) | The rug's stripes are uneven braided rag with a knotted fringe. The broom's head is a bundle of birch twigs splaying out under two bindings, with a ferrule on the handle. The hide frame carries a fox's pelt (head, leg flaps, belly line, a white-tipped brush) over a crosspiece. The tool chest has iron corner caps, strap hinges, an iron band round its lid, bail handles on lugs and a padlock hung open on its hasp. |

## M3, instance by instance

Checked against the merged branch (`claude/serene-clarke-rcgj62` at
b15461c) on 30 October.

| Instance | State | Held by |
| - | - | - |
| Mag through the gun (0:08:31) | Fixed | `seated::magazines_sit_in_their_guns`; `m3_pm_magwell.jpg`, `m3_pm_in_game.jpg` |
| Z-fight on the mag top (0:20:16) | Fixed | The same test: no fighting faces between a magazine and its gun (down to 0.5 cm²); its KNOWN list is empty |
| House wall gap (0:22:50, 0:41:47) | Fixed | `attach::buildings_have_no_cracks_and_doors_hang_in_their_jambs`: every prefab crack-free, BUILDINGS_KNOWN empty |
| Window latch floats (0:22:34) | Fixed | `attach::model_pieces_touch_what_holds_them`; `m3_window_latch.jpg` |
| Door hinge off the building (0:41:50) | Fixed | The cracks test's hinge check (`attach::hinges`); `m3_door_hinge.jpg` |
| Truck sunk, mud flaps detached (0:12:57, 0:13:02) | Fixed | `footing::placed_props_stand_on_what_is_under_them` (the wheels at every stop, no truck line left in KNOWN), `the_trucks_steps_reach_the_ground_at_every_stop`, the island test; `m3_truck_rear.jpg` |
| Stool floats, table legs in the air (0:33:38) | Fixed | The footing test (the camp); `m3_camp.jpg` |
| Button out of its panel's back (0:30:47) | Fixed | `render::props::button_tests` |
| Cassette z-fight (0:39:21) | Fixed | `radio::places` (no deck laid on its own tape), `seated::mounted_things_sit_in_their_mounts` (the tape inside its well) |
| Holster flap pops (0:35:26), pocket fronts pop (0:35:45) | Fixed | `attach::rig_flaps_lie_against_their_pieces` |
| Battery through the flashlight (0:11:13) | Fixed | `seated::mounted_things_sit_in_their_mounts`; `m3_flashlight_cell.jpg` |
| Model seam (0:44:00) | Not found | The shot looks down a road at a car wreck and the barriers. Every wreck's nodes pass the island test and the barriers have no cracks. Nothing in it is a known defect. |
| Walker through a wall (0:57:47) | Not a model | The walker's body went into a wall while it steered. That's its movement against the building's colliders (Robot minds and bodies), not a model. |
| Dirigible basket buried (1:01:50) | Fixed | The cargo pod hung 0.15 m up on its cable, its skids 0.3 m into the ground. Now `dirigible::pod_rest` stands it on the highest ground under it, on the cable, when the ship sinks and in the dev spawn; `a_lowered_pod_stands_on_the_ground`. |
| Broken crate's "googly eyes" (0:52:37) | Fixed | By Destruction |
