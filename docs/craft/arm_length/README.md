# The most-carried guns at arm's length, in the game

The AK-74, the PM and the Mosin, checked against "What a gun must show at
arm's length in VR" (`docs/craft/GUNSMITHING.md`) on 4 November 2026, on
main 8d5c1105. Each was shot in the game under the sky light:

- **The yard**, noon, clearing: at 1 m from both sides, and at 30 cm.
- **The Rime Fell mill**, inside: at 30 cm.

The shots:

    scripts/shot.sh OUT.png 0,0,40,0,0 40 --map training_yard --gallery GUN,YAW,DIST --hour day --weather clearing --no-robots
    scripts/shot.sh OUT.png 140,91.85,38,0,0 90 --map rime_fell --gallery GUN,0,0.3 --hour day --weather clearing --no-robots

In each sheet: top left, 1 m from the right; top right, 1 m from the left;
bottom left, 30 cm in the yard; bottom right, 30 cm in the mill.

| AK-74 | PM | Mosin |
| - | - | - |
| ![](sheet_ak74.jpg) | ![](sheet_pm_pistol.jpg) | ![](sheet_mosin.jpg) |

## Findings, worst first

1. **The Mosin's tang stood clear of the wrist.** The receiver's rear tang
   ran at a shallower slope than the wrist's top, so its end stood 5 mm
   above the wood. At 30 cm it read as a rod over the wrist. Fixed: its
   underside now follows the wrist half a millimetre into the wood, as an
   inlet tang does, and the tang screw sits on it (`art/guns/mosin.py`).

   | Before (game, 30 cm) | After (bench) |
   | - | - |
   | ![](mosin_tang_before.jpg) | ![](mosin_tang_after.jpg) |

2. **The AK's handguards show a dark line between them.** At 30 cm a
   band of the gas tube shows. The two handguards are only 1.5 mm apart,
   and both edges are rounded, as the real ones are. It's the same seam
   settled on the RPK, so I've left it.

Checked and holding:

- **Silhouette at 1 m**, both sides: all three read as themselves.
  - The AK's plum grip and magazine.
  - The PM's slide over its wraparound grip.
  - The Mosin's long straight wrist, its box magazine and its bayonet.
- **Controls at 30 cm**:
  - **AK**: the selector lever, the charging handle and the magazine.
  - **PM**: the slide's serrations, the trigger in its guard, the
    magazine's base.
  - **Mosin**: the bolt handle's ball and the cocking knob.

  All are where the hand looks for them, each its own part.
- **Finish under the sky light**: wood, plum polymer, blued steel and
  Bakelite part from each other in the yard. In the mill they hold, a
  step darker.
- **Seams**: none open apart from the AK's handguard line (above).
