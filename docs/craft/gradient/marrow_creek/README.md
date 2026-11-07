# Marrow: the place tells of itself

Where a walked way crosses from quiet ground (danger 0 or 1, or no
region) into hot ground (2 or 3), something of the place should be in
sight on the way up to the line: a wreck, a gate or booth, a sentry, an
anomaly or a sign. Landmark lamps don't count, because every landmark has
one. `map_gradient` finds the crossings: run
`MAP_ID=marrow_creek cargo test -p extraction_core probe_gradient -- --ignored --nocapture`.

Before, 7 of Marrow's 10 crossings had nothing of the place in sight, and
only The Hollow announced itself. After:

- **Pumping station** is cut down to the station and its yard. Its
  polygon had reached about 100 m out over the fields and the Forest
  path, a way home that was counted hot without being hot.
- **Radio mast**, the mast_relay job's two ways in: a guard booth on the
  east way, and the relay's jeep just inside the compound on the north
  one. A booth there stood on the slope, and its door step failed the
  legged machines' footing test (feet_gauge).
- **Bus depot** and **Rail yard**: a wrecked car by the way, short of the
  line.
- **The depot exit's landing** (the coordinator's note): two marker posts
  either side of the head of the way down, so a newcomer stepping out of
  the kiosk sees there is a way.

All 7 crossings are now told. The map lint is clean.

`before_after.jpg`: from the approaches at day, before and after.
