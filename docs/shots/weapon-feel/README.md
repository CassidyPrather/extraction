# Weapon feel: shots, and the 28 October playtest walked

## B2, F1 and F7, instance by instance

The rows of `docs/playtests/2026-10-28/TRANSCRIPT.md` this workstream
owns, each instance against the merged branch (`claude/serene-clarke-rcgj62`
merged at 0d1c303f, 31 October), with the test that holds it; every test
named passes there.

| Instance | State | Held by |
| - | - | - |
| **B2.** Grabbing the pistol takes its magazine (0:07:38, 0:10:04, 0:21:15) | Fixed | `weapons::reload::grab::a_grab_takes_the_gun_never_the_magazine_in_it`: every gun with a magazine, from six approaches, held in the other hand and lying on a table; a part seated in a held or lying thing is never what a grab takes (`interact::find_grab`). On a pistol a hand under the grip cups it; on a long gun a deliberate pull at the foot takes the magazine. |
| **B2.** The same at 0:34:43 | Fixed | The same test. (At 0:34:43 the magazine then fell under the crate where it couldn't be seen: B1's, Soak and Hands.) |
| **B2.** Passing the gun between hands fails (0:21:09) | Fixed | The same test's hand-to-hand pass for every gun: the other hand takes the gun, never its magazine. |
| **F1.** The plate shot straight through, no feedback (0:18:13 to 0:18:42) | Fixed | `weapons::impacts::plate_tests::every_range_plate_stops_and_marks_a_round`: every range plate in the yard stops a round with a metal hit (spark, ring) and a mark, nothing past it, and a hit in the painted centre marks the face you see. The plate and its centre had no colliders. `plate_before_after.png`. |
| **F1.** "Tracers might help" (0:18:13) | Already there | The rifle cartridges have tracer loads (`weapons::tracers`); the pistol's 9x18 has none, as in life, so the plate's mark is the answer there. |
| Aim is hard in general (0:55:18) | Fixed, and helped | What the guns did wrong is fixed: rounds leave on the iron sights' zero (`weapons::zero::tests::every_gun_hits_where_its_iron_sights_point`), and every gun's eye line runs over its rear sight onto its front (`capture::sightline`; ten guns' sights were off it). At 0:55:01 the tester couldn't get a second hand on the PM (B2, fixed above). Holding steady is now helped as in life: a second hand, a stock at the shoulder or a rest damps hand tremor (Comfort "steady aim", on; nothing moves the muzzle toward anything). `weapons::feel::vr_tests::a_steadying_hold_shrinks_a_tremoring_hands_group_at_25_m`: 8-12 Hz tremor, RMS group at 25 m off/on: PM cupped 9.9/3.7 cm, AK two hands 7.0/3.4, AK shouldered 7.9/2.5, one hand 9.4/9.4. `a_steadied_gun_still_swings_where_its_hands_turn_it`: a swing settles in 0.062 s either way. |
| **F7.** Holding the release could hold the mag half out (0:19:01) | Fixed | `weapons::reload::grab::the_release_held_hangs_the_magazine_out_for_the_other_hand`: held, the release hangs the magazine 3.5 cm out for the other hand; let go, it falls. `mag_seated_vs_presented_pm.png`. |
| **F7.** The ammo box fills a magazine all at once (0:20:22) | Fixed | `weapons::reload::grab::a_box_at_the_lips_loads_a_round_at_a_time`: a round each 0.15 s while box and magazine touch. (Flat mode's key and the bots still fill at once.) |
| **F7.** Loading is too fussy to line up (0:20:22 to 0:21:03) | Fixed | The same test, with the box at the magazine's lips: the touch is measured from the nearer of the magazine's middle and its lips, 16 cm. |
| The range's "shoot it empty" could stall after a short fill (found on the way) | Fixed | `vr_harness::onboarding::station_the_range`: a squeeze on a slide already back counts. |

## The shots

- `models/`: the gun model passes (see `models/README.md`).
- `plate_*`: the range plate before (no colliders: rounds through, no
  mark) and after. The plate gallery staging is newer than the fix, so the
  before is the current build with the plate's and its centre's colliders
  turned off.
- `mag_*`: the PM's magazine seated, and held out by the release.
- `safety_*`, `jam_*`, `eyebox_*`, and the rest: earlier waves.
