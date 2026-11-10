# Vistas: the first ten minutes, before and after

Marrow, Kettle and Cinder, from the truck to the first job and on to the
rendezvous (walk review, `docs/craft/WALK_REVIEW.md`). Before is on the
left or top, after on the right or bottom.

- `kettle_truck.jpg`: the first frame off the truck at Kettle. A ride cut
  through the Gate woods puts the Logging camp's shed and mast in it
  (cleared, not added).
- `marrow_truck.jpg` (centre crop, 2x): Marrow's first frame. Six marker
  posts every 12 m down the farm track lead the eye to the field shed and
  the farm. The farm itself stays faint in the drizzle; the gain is modest.
- `kettle_fingerposts.jpg`: the Pass road's three forks, each from where
  the walker reads its post. Story's wording names places. The road turns
  about 80 degrees at every fork, so the posts say left and right, never
  ahead. A signpost board reads to 6.5-6.9 m (`sign_reach`), so each post
  stands where the way passes 5-6 m short of it, its face turned there.
- `arrivals.jpg`: Marrow's Rail yard (top) and Cinder's Works yard
  (bottom). The places were fine; the review's last stop stood on the
  point and looked past it. `scripts/walk_stops.py` now frames every
  arrival from 20 m out, looking at the point. Review tooling only; the
  game is unchanged.
- `kettle_fingerposts_night.jpg`: the same forks after dark, before and
  after. A board needs a light near it: a way lamp 2.5-3 m from the
  Quarry and Mine posts, on the side the board faces, its arm over the
  board (behind the face, 0.6 m back, the board was a silhouette). The
  Lodge fork (bottom) has none: the charcoal hut's window stands dead
  ahead down the lodge branch, and a warm lamp there would stand in the
  frame of the lodge track's crossing stake for whoever walks home.
- `cinder_first_frame.jpg`: Cinder's first frame off the truck, before
  and after, by day (top) and at night (bottom). Nothing at Ash Row read:
  its houses are low and the blast furnace stands 60 degrees off the way.
  Models' works gas holder stands at the top of the Row, 116 m out and
  4 degrees off the way, lower in the gate's view than the works chimney
  (Story: the chimney stays the valley's landmark). Its red landmark lamp
  lights after dark. `map_vista::every_first_frame_holds_somewhere_to_go`
  holds every valley's first frame to this, with no gaps.
- `first_frames_night.jpg`: each valley's first frame off the truck after
  dark, along the way's heading. `every_first_frame_holds_somewhere_to_go`
  asks for a yard lamp, a lit window or a landmark's red lamp ahead (not a
  stake or a warning: they say danger). Marrow (the ranger tower's lamp),
  Kettle (the camp chimney's) and Cinder (the gas holder's) show plainly.
  Sallow's two yard lamps at about 100 m and Rime's survey tower lamp pass
  the gauge but are faint in this still, in rain.
- `arrivals_day.jpg`, `arrivals_night.jpg`: each valley's first frame off
  the truck at its insertion marker's `rot`, the way the player spawns
  facing (Marrow 53, Kettle 58, Cinder 21, Sallow and Rime as set by
  Fourth valley), by day and after dark in rain. These replace the
  way-heading frames above as the ones the gauge judges.

## What the first-frame gauge asks, and what it doesn't

`map_vista::every_first_frame_holds_somewhere_to_go` asks that the frame the
player arrives facing (the Insertion marker's `rot`, 35 degrees either
side) holds somewhere to go: by day, the first job's buildings or a
landmark, in sight and big enough; after dark, a yard lamp, a lit window or
a landmark's red lamp within `NIGHT_READ` (90 m). It does not catch a first
job hidden behind trees while something else stands in the frame: the
first frame's spruce crowns are cones, wide at their lowest boughs and
narrow at the top, so a chimney top over a wood reads (Kettle's frame
before its ride cut reads that way, the sawmill still hidden).

Each valley's marker faces the way on: Marrow 53, Kettle 58, Cinder 21
(turned with this gauge), Rime 56.5 (Fourth valley), Sallow 0.

### Night read, measured

Glow over its background in rendered night stills of each first frame
(the sum of each pixel's excess over the ring's median, more than 8 levels,
within 6 px of the light's projected spot), clear and in rain:

| Light | Distance | Clear | Rain |
| - | - | - | - |
| Marrow, ranger tower lamp | 69 m | 1750 | 2112 |
| Kettle, camp chimney lamp | 89 m | 1238 | 1173 |
| Cinder, gas holder lamp | 110 m | 402 | 485 |
| Sallow, yard lamp | 96 m | 507 | 791 |
| Sallow, yard lamp | 106 m | 174 | 240 |
| Rime, survey tower lamp | 128 m | 191 | 259 |

Kettle's chimney lamp is the faintest that reads (the coordinator's limit).
The glow falls off with distance; rain changed it little (it darkens what is
round a lamp as much as the lamp), so `NIGHT_READ` is the same in both.
Each valley's first frame now holds a yard lamp 40 m up the way: Cinder's
up the works road (Map design), Sallow's and Rime's from Fourth valley
(b9f2fe32). Both GAPS lists are empty.
