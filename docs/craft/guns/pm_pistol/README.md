# PM pistol: fidelity review

The pistol a newcomer holds first. Reviewed on the bench
(`docs/craft/GUNSMITHING.md`) on 31 October 2026.

## Reference

- **Numbers**: 161 mm long, 126.75 mm high, 29.4 mm wide, a 93.5 mm
  barrel, a 130 mm sight line, 730 g with the magazine empty: the Soviet
  manual's figures as Wikipedia (Russian and English) carries them. In
  `gunsmith::spec`.
- **No photograph yet.** The free side views (Wikimedia Commons) couldn't
  be fetched from this workshop, so this review measures against the
  numbers only. The next pass should lay the model over a side view, as
  the AK's review does: the grip's shape and the slide's sections need it.
- **What I know of it**: a blowback 9x18; the slide's rounded nose and
  domed top; the frame's front ends short of the slide's; the safety on
  the slide's left rear, a lever whose tip the thumb pushes down to safe
  (it decocks the hammer and blocks it); the big hinged trigger guard
  that pulls down to strip it; the slide stop on the left above the
  trigger; the heel magazine catch, drawn back by the thumb; one
  wrap-around bakelite grip, checkered, the star in a circle at its top,
  held by one screw up the back; the lanyard ring at the left of the
  heel; a spur hammer showing behind the slide.

## Shots

Same commands before and after (`scripts/gun_bench.sh pm_pistol <dir>`):
`shots/before/`, `shots/after/`.

| | Before | After |
| - | - | - |
| Right, 30 cm | ![](shots/before/right_30cm.jpg) | ![](shots/after/right_30cm.jpg) |
| Left, 30 cm | ![](shots/before/left_30cm.jpg) | ![](shots/after/left_30cm.jpg) |
| Three-quarter, 30 cm | ![](shots/before/threeq_30cm.jpg) | ![](shots/after/threeq_30cm.jpg) |
| Shooter's eye, 30 cm | ![](shots/before/eye_30cm.jpg) | ![](shots/after/eye_30cm.jpg) |
| Slide back | ![](shots/before/bolt_back_30cm.jpg) | ![](shots/after/bolt_back_30cm.jpg) |
| Exploded | ![](shots/before/exploded_1m.jpg) | ![](shots/after/exploded_1m.jpg) |

## Sections

Cut across the bore at the nose and the ejection port, and across the grip
at its top and low down (`art/sections.py`, 10 mm grid), in a second pass
after the coordinator saw the PM still read as a box at 30 cm. Before: the
slide 28 mm wide with a flat top between two small rounds; the grip a box
with 3.5 mm corners. After: the slide 25.4 mm, arched nearly across; the
grip a rounded oblong (7 mm rounds), its heel rounded. (The first pass
scored the sections 2; seen cut, they were a 1, and the before total is
re-scored to match.)

| Before | After |
| - | - |
| ![](sections_before.png) | ![](sections_after.png) |

## Measured

| | Real | Before | After |
| - | - | - | - |
| Overall length (the drawn gun) | 161 | 175 (+9%) | 165 (+2%) |
| Barrel | 93.5 | 102.5 (+10%) | 94 |
| Sight line | 130 | 141.5 (+9%) | 130 |
| Height (the drawn gun, magazine in) | 126.75 | 138 (+9%) | 138 (+9%) |
| Width (the drawn gun) | 29.4 | 35 | 34 |
| Slide's width | about 25 | 28 | 25.4 |

Everything above the grip was 9% long along the bore. The grip, which
holds the magazine, was left as it was; the frame blends between them
through the top of the grip (`ABOVE` in `art/guns/pm_pistol.py`). The
height and width stay over: what makes them up (the magazine's floor
under the grip, the safety lever and lanyard ring at the sides) needs a
photograph to settle.

## Scores

0 wrong or missing, 1 a person who knows the gun winces, 2 right at a
glance and wrong at 30 cm, 3 right at 30 cm. Sound and feel aren't
scored here (weapon feel owns them).

| Row | Before | After | Notes |
| - | - | - | - |
| Silhouette | 1 | 2 | the nose now rounds over, the heel rounds |
| Proportion | 1 | 2 | length, barrel and sight line right; height and width 9% and 20% over |
| Sections | 1 | 2 | the slide arched, the grip a rounded oblong; the frame's front strap still square-edged |
| Parts present | 2 | 2 | no red dot under the safety; the slide stop's shape is a plain tab |
| Parts move | 2 | 2 | the heel catch is its own part, the trigger turns on its pin; the guard can't swing down |
| Controls | 2 | 2 | |
| Finish | 2 | 2 | as the AK's: the gun-metal scan reads as a casting at 30 cm |
| Wear | 2 | 2 | |
| Edges and making | 2 | 2 | |
| Markings | 2 | 2 | serial and year on the left of the slide |
| **Total (of 30)** | **17** | **20** | |

## What's still wrong, worst first

1. **Height and width over** (see above): the magazine's floor below the
   grip and the grip's thickness; a photograph would settle both.
2. **The frame's front strap and trigger guard** are square-edged steel
   under rounded bakelite.
3. **The trigger guard can't swing down**, so the gun can't be stripped
   as the real one is. It's welded into `static_frame`.
4. **No red dot** on the frame under the safety, which shows when it's
   off safe: the one thing the eye checks on a PM.
5. **The finish** at 30 cm (shared with every gun).

## What changed

- `art/guns/pm_pistol.py`: everything above the grip 8.3% shorter along
  the bore; the slide's nose rounded over (9 mm) and under (4 mm), the
  front sight bedded into it; the heel magazine catch its own object
  (`mag_catch_heel`, `Moves::HeelCatch`); the grip's share of the
  magazine well cut wider than the frame's so they no longer z-fight
  (`art/check.py` failed on it).
- Second pass: the slide 25.4 mm wide (it was 28) and arched across its
  top; the grip's edges rounded 7 mm, its outline's corners eased; the
  trigger's origin on its pin.
- `src/weapons/layout.rs` (`pm()`): the muzzle, port, sight, slide grab
  and load port moved with the model.
