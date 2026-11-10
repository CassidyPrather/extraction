# Marrow: the place tells of itself

Where a walked way crosses from quiet ground (danger 0 or 1, or no
region) into hot ground (2 or 3), something of the place should be in
sight on the way up to the line: a wreck, a gate or booth, a sentry, an
anomaly or a sign. Landmark lamps don't count, because every landmark has
one. `map_gradient` finds the crossings: run
`MAP_ID=marrow_creek cargo test -p extraction_core probe_gradient -- --ignored --nocapture`.

Before, 7 of Marrow's 10 crossings had nothing of the place in sight, and
only The Hollow announced itself. After:

- **Pumping station**: its polygon reached about 100 m out over the
  fields and the Forest path, a way home that was counted hot without
  being hot. It is cut to the station and its yard (after Robot minds'
  fix to a Hush test that counted any response), and its three crossings
  out in the fields are gone.
- **Radio mast**, the mast_relay job's two ways in: a guard booth on the
  east way, and the relay's jeep on the north one, in the way's mouth
  where the cut ends, so it stands dark against the trees from 20 m
  short. (Further in, on the skyline, it couldn't be found in the frame
  without a caption. A booth on the slope failed the legged machines'
  footing test.) It was held a while: with it placed, Onboarding's bot
  drew another plan in second_hour that walked it under the Forest path
  footbridge to reach the deck from below. The bot's planner now goes on
  and off a deck only by its ground ends (Onboarding), but the jeep was
  held again: with it placed, second_hour's raid changed and walked the
  bot onto the District road bridge's north end and then west through its
  railing, toward (-0.7, -12.5). Onboarding's leg-wise fix (a detour's
  leg that crosses a deck goes round the nearer end) brought it back.

A tell counts only if it can be found in the approach frame without a
caption. `map_gradient` holds this: the tell must look big enough for
its distance (as `map_breadcrumbs` reads a cue, with no skyline allowance),
ahead of the walker, from the last 60 m before the line. The full rule,
and the night one, are in [the gradient's README](../README.md).
- **Bus depot** and **Rail yard**: a wrecked car by the way, short of the
  line.
- **The depot exit's landing** (the coordinator's note): two marker posts
  either side of the head of the way down, so a newcomer stepping out of
  the kiosk sees there is a way.

Every Marrow crossing is told, by day and, with the machine stakes at the
Bus depot, the Rail yard and the Radio mast's east way, after dark. The
map lint is clean.

`before_after.jpg`: from the approaches at day, before and after.
