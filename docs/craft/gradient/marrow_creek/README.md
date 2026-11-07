# Marrow: the place tells of itself

Where a walked way crosses from quiet ground (danger 0 or 1, or no
region) into hot ground (2 or 3), something of the place should be in
sight on the way up to the line: a wreck, a gate or booth, a sentry, an
anomaly or a sign. Landmark lamps don't count, because every landmark has
one. `map_gradient` finds the crossings: run
`MAP_ID=marrow_creek cargo test -p extraction_core probe_gradient -- --ignored --nocapture`.

Before, 7 of Marrow's 10 crossings had nothing of the place in sight, and
only The Hollow announced itself. After:

- **Pumping station**: its polygon reaches about 100 m out over the
  fields and the Forest path, a way home that is counted hot without
  being hot. Cutting it to the station and its yard is agreed but held:
  it moved the director's survey ground, and a Hush test that counts any
  response then failed. It comes back with Robot minds' fix to that test.
  Until then its three crossings (out in the fields) stay untold.
- **Radio mast**, the mast_relay job's two ways in: a guard booth on the
  east way, and the relay's jeep on the north one, in the way's mouth
  where the cut ends, so it stands dark against the trees from 20 m
  short. (Further in, on the skyline, it couldn't be found in the frame
  without a caption. A booth on the slope failed the legged machines'
  footing test.) **Held:** the jeep changed the plan Onboarding's bot
  drew in second_hour, and the new plan walked it under the Forest path
  footbridge to reach the deck from below, where it stuck. It comes back
  once that fix (Onboarding, 5a91610e) is on main. Until then the north
  way is untold, and row 2's after frame shows the jeep as it will stand.

A tell counts only if it can be found in the approach frame without a
caption. `map_gradient` holds this: the tell must look big enough for
its distance (as `map_breadcrumbs` reads a cue, with no skyline allowance),
seen from the last 8 to 30 m before the line.
- **Bus depot** and **Rail yard**: a wrecked car by the way, short of the
  line.
- **The depot exit's landing** (the coordinator's note): two marker posts
  either side of the head of the way down, so a newcomer stepping out of
  the kiosk sees there is a way.

With the shrink held, Marrow's other crossings are all told. The map lint is clean.

`before_after.jpg`: from the approaches at day, before and after.
