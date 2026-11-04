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

## Hand-worked mechanisms: flick, steady, let go short

The crossbow's lesson as a class rule: every hand-worked handle on every
gun, three strokes, held by
`weapons::strokes::every_hand_worked_mechanism_completes_as_its_kind_should`
(a flick is past the end in three ticks; a steady pull past the end over
half a second; short is half way, then let go).

| Handle (guns) | A flick completes | A steady pull completes | Let go short |
| - | - | - | - |
| Slide (PM) | Yes: racked hard is how it's worked | Yes, and chambers | Springs home; nothing chambered |
| Charging handle, closed bolt (AK, RPK, SKS, SVD) | Yes, as a slide | Yes, and chambers | Springs home; nothing chambered |
| Charging handle, open bolt (PPSh) | Yes: catches on the sear | Yes | Rides home, not caught |
| Turn bolt (Mosin, TOZ-8) | Yes: a bolt thrown fast is still worked | Yes | Stays where left (no spring) |
| Pump (MP-133) | Yes | Yes | Stays where left (no spring) |
| Break action (TOZ double, flare pistol) | Yes: a smart push opens it | Yes | Stays shut short of 6 cm |
| String (crossbow) | No: the prod's too heavy (`DRAW_WEIGHT`) | Yes: catches on the latch | Slaps home, not caught |

The Nagant has no handle (a loading gate), so it isn't in the table.

Every gun passes as it stands; nothing needed fixing beyond the crossbow
(ae8ccd8f). The test bites: with `DRAW_WEIGHT` at 0 the crossbow's flick
catches on the latch and it fails. A closed-bolt stroke that completes must
also chamber a round, and one that doesn't must not.

## The shots

- `models/`: the gun model passes (see `models/README.md`).
- `plate_*`: the range plate before (no colliders: rounds through, no
  mark) and after. The plate gallery staging is newer than the fix, so the
  before is the current build with the plate's and its centre's colliders
  turned off.
- `mag_*`: the PM's magazine seated, and held out by the release.
- `soft_puff_*`: every puff (smoke, dust, gas, powder) was an alpha-blended
  sphere, a flat grey disc with a hard edge; now one soft puff,
  `render::puff`, turned to face the eye. Before (left, or top) is
  39708bc5, the code before the change with the impact gallery's clock;
  after is aabbb3f5. Each pair is from the same camera:
  - `concrete` and `concrete_zoom` (3x): `--gallery impact` into the
    yard's concrete road block (`--camera 5.2,0,68,180,-11 --frames 75`,
    a quarter of a second after the rounds land). Hard white discs, then
    soft dust.
  - `dirt`: the same into the road (`0,0,40,0,-20`). Low contrast both
    ways: brown dust on brown gravel.
  - `broken_walker_5m` and `_15m`: Models' line-up
    (`--gallery robots:walker+walker!broken,0,D`, fog, 11:00). The smoke
    over the broken walker was a stack of grey discs; now it's soft wisps.
    How many show is the stream's moment.
  - `wreck_reel` (`reel:wreck,16,5`, 14:00): a smouldering wreck's soot
    is a soft column, not a stack of circles, and not a black ball.
  - `hazards_reel` (`reel:hazards,12,5`): the hazard clouds (frost
    shown) are soft hazes.
- `impact_burst_*`: up close a round's dust read as soft balls left
  hanging, not a hit. Now masonry and soil throw a burst of five small
  puffs out of the face (3 m/s, braked and blooming), then three that
  linger. Before (left) is 219dc6da, after this change; top row frame 63,
  just after the rounds land, bottom frame 75, a quarter of a second on
  (`--gallery impact`, as above). On concrete the dust now bursts from
  the marks; on dirt it's still brown on brown.
- `safety_*`, `jam_*`, `eyebox_*`, and the rest: earlier waves.

## Where I stopped (2 November, paused for usage)

On `claude/polish-weapon-feel` at 6ed7ee78, gated green (lib 1329, core
317, 41/41 scenarios, 13/13 contracts), for the coordinator's final gate:

- **Tracers light from the muzzle** (6ed7ee78): `Projectile.origin`; a
  tracer landing inside about 22 m now streaks. Any `Projectile { .. }`
  literal on another branch needs `origin`.
- **The Nagant's own kick** (dc0699c6): `climb 0.56, push 2.2`, so
  Gunsmith can raise its hold to (0, 0.014, 0.027); green at both holds.
- **On main already:** soft puffs (`render::puff`), the impact burst, the
  10 degree near-puff cap under Performance's stereo gauge, the shared
  analog press (`rig::pressed`), steady aim, the gun-sound audits.

What's next, when work resumes:

- (Done: Gunsmith's raised Nagant hold is on main since 5c2422d, and
  `every_gun_has_its_own_kick_and_stays_in_hand` passes there.)
- `audio_handling.py`'s KNOWN holds only the Nagant's cylinder (Audio's
  to fix); it flags when it clears.
- If lit puffs show black pixels beyond Robot bodies' gun column, or on a
  GPU, make every puff unlit in `PuffAssets::soften` (a class change,
  with before/after shots).
- The hazard cloud has 13% headroom on the stereo budget (56.4M of
  64.8M); a later effect that eats it should get a per-view cap on near
  puffs, not a smaller angle.
