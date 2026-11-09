# The danger gradient: hot ground tells of itself

A newcomer walking from quiet ground into hot ground should be told before
they are in it, by the place itself, by day and after dark. "Quiet" is a
region of danger 0 or 1, or no region; "hot" is 2 or 3. This page is the
rule, the gauges that hold it, and how to place something when a gauge
fails. Per-valley notes: [marrow_creek](marrow_creek/README.md),
[kettle_cinder](kettle_cinder/README.md); Sallow and Rime are Fourth
valley's (`docs/FOURTH_VALLEY.md`).

All of the code is `crates/extraction_core/src/map_gradient.rs`.

## What is a crossing

`map_gradient::crossings` walks every routed way (routes to the
rendezvous, first jobs' ways there and home, contracts' ways) and notes
each point where it goes from quiet into hot. Two things keep the list to
real crossings:

- **A gap is no crossing.** A way must have crossed at least `QUIET_RUN`
  (20 m) of quiet ground before the line. A few metres of open ground
  between two hot places is walked hot to hot (Cinder's scout home from
  the works into the Works yard was 6 m).
- The approach is the way's last `APPROACH` (60 m) before the line,
  ending where it was last in hot ground.

## What tells, by day

A tell is something of the place: a sign, a wreck, a gate or guard booth,
a sentry, an anomaly, or a machine stake. It must stand in the hot ground
being entered, or within `BY_THE_LINE` (40 m) of the line. It counts only
if a newcomer finds it **in the frame, without a caption**:

- read from a point on the approach `SHORT` (8 m) to 60 m before the
  line;
- **ahead**, within `AHEAD` (35°) of the way's heading at that point (not
  over a shoulder: a tell beside the way short of the line reads from
  further back, where it is ahead);
- **big enough** for its distance: the square root of its height times
  its narrowest breadth, over the distance, at least `READ_ANGLE` (as
  `map_breadcrumbs` reads a cue, with no skyline allowance);
- **in sight**, over the ground and past the buildings between.

Landmarks' red lamps don't count by day: every landmark has one.

## What tells, after dark

`crossings_at_night` reads the same crossings with only what can be seen
at night:

- **a light of its own**, read as a glow (no size test): a sentry's
  searchlight to 45 m (`sentry::BEAM_REACH`), a steady anomaly
  (`GLOWING`: Burner, Snare, Well, Frost; the rest show only when
  something is thrown in), a robot gate's red lamp to 25 m, or a prop in
  `SELF_LIT` (the machine stake, 25 m);
- **an unlit tell with a lamp beside it**: a light to steer by
  (`map_night::lights`) within `LIT_BY` (8 m), read as by day but no
  further than `LIT_READ` (25 m);
- **the place's own red lamp**: a landmark's lamp counts after dark only
  when the landmark stands in the hot ground being entered or within
  `OWN_LAMP` (25 m) of the line, and only within `OWN_LAMP_READ` (100 m)
  of the reading point. Further off it is skyline: the lamps far away are
  how a walker finds home, and if every distant red light meant danger
  the warning would mean nothing (the coordinator's ruling).

### `SELF_LIT`: a light the game really lights

A prop counts as its own light only when it is in `SELF_LIT`. Whoever
lights a prop in the renderer adds it there, with a render test holding
the two together, as `map_night::LAMP_SEATS` is held. The machine stake
went in with Visual design's lit amber lens (render test in their tip),
not before: the gauge must not count a lamp the game doesn't light.

`SELF_LIT` is apart from `map_night::lights`: a stake says the ground is
the machines', it is not a light to steer home by.

## The machine stakes

`machine_marker` (Models' stake, Visual design's amber lens, a slow pulse
of at most 0.5 Hz, emissive with no point light, so it doesn't light the
ground) is the fix where nothing else tells at night. It reads from any
side; `rot` only turns its foot fins. It may stand on slopes up to
`MAX_SLOPE` (32°, `map_lint::footing_slope`); other unlevelled props keep
`COMFORT_SLOPE`.

Two uses:

- **Crossings**: one stake per place, shared where two ways converge on
  the same line (Cinder's North sidings takes one for both its ways).
- **The Gate's line**: a valley key carried within 60 m of the robot gate
  for 6 s calls a heavy. Stakes stand 52-68 m from the Gate so that every
  walkable spot of the 60 m circle (5 m samples, the walkers' grid, inside
  the fence) is within 25 m of one (Heavy's `gate_line_is_staked`,
  `src/robots/heavy_mind.rs`; `map_heavy::unmarked_line` lists what is
  unmarked).

## The gauges

| Gauge | Where | Holds |
| - | - | - |
| `every_quiet_to_hot_crossing_is_told_by_its_place` | `src/robots/pressure.rs` (Robot minds) | every crossing told by day |
| `every_quiet_to_hot_crossing_is_told_at_night` | `map_gradient` | every crossing told after dark |
| `every_sentry_reads_before_it_sees_you` | `map_gradient` | where a way first enters a sentry's sight, its lamp was read first: ahead within 35°, in sight; by day to 55 m (its marker), and at dawn, dusk and night against its sight then (37, 33, 22 m) with its searchlight to 45 m |
| `gate_line_is_staked` | `src/robots/heavy_mind.rs` (Heavy) | every Gate's 60 m line staked |

The two crossing gauges carry a `GAPS` list for crossings known to be
untold, each named with whose it is. They fail on an untold crossing not
listed, and on a listed one that is now told: take it off in the same
commit as the fix. Both lists are empty.

## Placing a tell

The probes print what tells each crossing and where to put something:

```
MAP_ID=<valley> cargo test -p extraction_core probe_gradient -- --ignored --nocapture
NIGHT=1 MAP_ID=<valley> cargo test -p extraction_core probe_gradient -- --ignored --nocapture
```

Each crossing line ends with a spot `BACK` (15 m) short of the line and
two spots 4 m either side of the way. They are suggestions, not
placements. What went wrong placing the wrecks and stakes so far:

- **Roads.** Several suggested spots stood on a road or track: keep clear
  of its width plus shoulder (the map lint's Overlap finding).
- **Footing.** The ground under a wreck must be within 0.3 m of its foot,
  and an unlevelled prop must be on gentle enough ground. Scan a few
  metres round for a level spot rather than accept the first.
- **The ground of a neighbour.** A stake 4 m from a log pile changed the
  ground under the pile (`fit::buildings_meet_the_ground`).
- **The way itself moves.** A prop beside a way can move the way. Cinder's
  first works wreck pushed the way home from Ash Row over by about 5 m and
  its first stop lost its lamp at night (Fourth valley's
  `every_first_way_home_reads_at_night`). A stake shifted Rime's barracks
  way 7 m. Run the night tests after any placement near a way.
  `probe_way_shift` (ignored, `map_gradient`) prints how far each way
  moves with a valley's wrecks taken out; it is a probe, not a rule (a
  shift is harmless unless something reading the way breaks, and the
  gauges run on the ways as walked).
- **Raids.** On Marrow, a wreck placed by the Radio mast's north way
  changed second_hour's raid enough to walk the bot into a bridge railing
  (Onboarding's planner fix). Run second_hour after a Marrow placement.

Then the map lint (`every_valley_passes_the_map_lint`), the extraction_core
suite, and the full gate with second_hour.

## Seen at night

`night_before_after.jpg`: three crossing stakes (Marrow's Bus depot
route, Kettle mine from Miners' Row, Cinder's North sidings) and one of
Marrow's Gate-ring stakes, at night in rain, from 10-18 m short on the
way, before (the same build, the stakes taken out) and after. The stake
reads as a small amber point where nothing marked the ground before: a
mark you find, not a beacon, as meant (emissive, no point light, read to
25 m). Kettle's sits among a few other lights on the skyline there.
