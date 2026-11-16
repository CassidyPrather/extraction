# Driver lines in view (#90): mock-up

Throwaway mock-up of the approved design. These are panels placed by hand
in the dev poses (`mock.patch` on `src/dev.rs`, not merged). There is no
follow, fade or reading time yet. All shots are 3200×1800 PNG (2×), with
the desktop's 75° vertical field of view.

| Shot | What it shows |
|---|---|
| `1_callout_day.png` | Item callout beside a held PM, daylight. Panel x-height 0.65° at its distance (about 0.5 m), 20 characters a line, 4 lines. A 1.5 mm leader runs from the grip to the panel's lower near corner. |
| `2_callout_night.png` | The same at night. |
| `3_caption_walk.png` | The view caption, walking (hands at rest): 1.4 m out, 12° below the eye line, x-height 0.9°, 26 characters a line. Speaker on its own line. |
| `4_caption_and_callout.png` | Both live, with the caption's urgent bar (orange-red, 12 mm). The view is pitched 18° down, as when looking at the gun. |

Findings for the real build:

- **Night:** the slate behind the text is lifted to grey. The panel is self-lit, and the night exposure brightens it along with the letters. The real panel's emission must follow the eye's adaptation (`render::grade`), so the slate stays near-black and the letters stay at reading contrast, never a glow.
- **Caption over the held gun (shot 4):** the caption and the callout keep clear of each other. But looking down at a raised pistol puts the caption over its muzzle. Rule: the caption keeps 3° clear of every held item's outline in view. It moves up toward the eye line (no higher than 6° below it), else down (no lower than 20° below).
- **Sight keep-out:** the shot of a gun at the sights with a callout 12° off its line didn't render. The mock panel spawns (logged) but doesn't draw in the `--gallery sights:` pose, cause not found. The keep-out will be shown from the real build, with its unit test.

Shots were made with `EXTRACTION_BIN=<mock build> scripts/shot.sh` at camera `40,19.2,262,0,-25` (`fp:pm_pistol`), `40,0,262,0,-4` (`fp:open`) and `40,19.2,262,0,-18`. Flags: `--no-robots --hour day|night --size 3200x1800`. Text came from `MOCK_CALLOUT`, `MOCK_CAPTION` and `MOCK_URGENT`.
