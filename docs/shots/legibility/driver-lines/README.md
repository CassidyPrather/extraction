# Driver lines in view (#90)

Every line is still on the wrist. A line told for the view (`Raid::tell`)
also shows in front of the eyes (`render::said`):

- **About a thing in hand** (`Show::Item`): a callout beside it.
- **To act on now** (`Show::View`, `Show::Urgent`): a caption in the lower
  view.

The Captions card cycles three settings: in view (the default), the wrist
only, off.

All shots are 3200×1800 PNG, made with the real systems in the dev poses
(`EXTRACTION_SAY`, see `dev::say_for_gallery`). The view is the desktop's
75° vertical field of view, so panels off to the side show its
perspective. The callout text is real: the yard's own steps
(`onboarding::Goal::how`).

| Shot | What it shows |
|---|---|
| `1_callout_day.png` | The yard's Rack step as a callout beside the PM in hand, headed with the catalog's name. |
| `2_callout_night.png` | The same at night: the slate stays near black, lit only as far as the eyes are open (`said::glow`), never a glow. |
| `3_caption_walk.png` | The truck's answer as the caption, walking, hands at rest. The speaker is on its own line. |
| `4_caption_and_callout.png` | Both live, the caption urgent (orange-red bar). A pistol raised to look at fills every height from the eye line to 20° below, so the caption slides aside off it and its glove. |
| `5_at_the_sights.png` | The PM at the eye. Its own callout has faded, the caption waits (time stood still, back when the gun comes down), and the magazine's callout in the other hand keeps 12° off the sight line. |

## Numbers

| | Callout | Caption |
|---|---|---|
| Distance | about 0.5 m (beside the thing) | 1.4 m, 12° below the eye line |
| Letter size | x-height 0.65° | x-height 0.9° |
| Measure | 20 characters a line | 26 characters a line |
| Follow | critically damped, ω 8 rad/s (about 0.13 s behind, no overshoot); turn τ 0.15 s; tremor under 1 cm ignored | soft lock: still within 18° of yaw and 10° of pitch, then comes round with τ 0.4 s |

Both clear the 0.55° floor for text that must be read
(`legibility::MUST_READ_DEG`).

**Reading time:** 1.5 s + 0.06 s a character, 3 to 9 s. Looked at
(within 10°), a line holds up to 4 s longer. It fades over 0.3 s. A new
line takes the place of the shown one after 1.5 s, except that an urgent
line is never replaced inside its reading time.

**Clear of what's held:** the caption keeps 3° clear of everything held
and of the gloves, by their outline in view:

1. it moves up to 6° below the eye line;
2. then down to 20° below;
3. then up to the eye line;
4. then aside, up to 20°;
5. if nothing clears, it takes the least crowded spot.

It also keeps clear of the callouts showing.

**At the sights:** a callout's centre keeps 12° off every held gun's sight
line. With a gun at the eye (`Handling::at_eye`), that gun's own callout
and the caption fade over 0.15 s.

`mockup/` holds the throwaway mock-up the design was approved from.
