# Walk review

The map lint measures: how far you're seen, how steep a path climbs, how
long you walk with nothing to stop at. It can't tell you whether a walk
is any good. This is the other half: stand on the route a newcomer takes
and judge it the way a level designer would, stop by stop. It's a
procedure, for whoever has the maps next. Bend it where it's wrong, and
write down what you learned here.

## The walk

The first job's route: from the drop to the career's first job, then on
to the nearest rendezvous. That's the first raid a newcomer has in the
valley, and the one they'll judge it by.

1. **The route.** `MAP_ID=<valley> WALK_OUT=<file>.json cargo test --release
   -p extraction_core probe_walk -- --ignored` writes it out
   (`map_openness::probe_walk`). It's the easy way on comfortable ground (no
   slope over 25 degrees) that the lint's first-job checks take, as a
   line with heights, plus what stands along it: doors, places,
   rendezvous, placements and roads.
2. **The stops.** `scripts/walk_stops.py <route>.json <walk>.stills
   <stops>.json` puts a stop every 40 m, and at every fork, door, crest,
   the job and the rendezvous. Each stop gets two views at eye height:
   along the way, and to the side, at the door if it's a door or the
   nearest place or landmark standing off the way ahead. Add stops by hand
   where you'd stop yourself; a walk is not a sample.
3. **The shots.** `CINE=<walk>.stills scripts/capture_cinematic.sh <valley>
   <out>` renders them, a still a view (`docs/CAPTURE.md`). Use the
   valley's reference weather (the default). A shot that shows nothing
   because of fog is an answer, not a failed shot.
4. **The answers.** Look at each pair and answer three questions, a
   sentence or two each, in `answers.json` (format in
   `scripts/walk_sheet.py`):
   - **Vista.** What does it look like from here, and is it worth
     looking at? Name what's in the frame, not what's on the map.
   - **Draws the eye.** What could be discovered from here: what pulls
     your look, or your feet, off the way? A door, a light, a shape on a
     ridge, a sound you can't see the source of.
   - **For.** What is this moment for in the experience: calm, tension,
     reveal, choice, relief, threshold, or nothing (say so)? One word as
     the beat, then why.
5. **The sheet.** `scripts/walk_sheet.py <walk-dir>` lays the stops out
   as a contact sheet (`sheet.jpg`) and a page (`WALK.md`) with a beat chart:
   the walk as a flow of moments, not metres.
6. **What's wrong.** Read the beats in order and name:
   - **dead stretches**, where nothing changes for long enough that you'd
     stop looking;
   - **muddy views**, where the frame has no subject, or the subject is
     in fog, behind a bush, or side-on;
   - **moments that don't know what they're for**: a reveal nobody set
     up, a choice with only one way, tension with nothing to fear.
7. **Fix the worst three.** Each fix gets a before and after from the
   same stop, the same view. The lint stays green, but passing it is not
   the goal.

The walks live in `docs/craft/walks/<valley>/`: the stops, the shots,
the answers, the sheet, the page and the fixes.

## Judging

- A stop is judged from the shots, not from the map. If the farm is
  20 m off but you can't see it, it isn't there.
- Answer as a newcomer: you don't know where loot is, or that the tower
  is a ranger tower. You know the job's name and the way the truck faced.
- A good walk alternates. Calm, then something to look at; a threshold,
  then a reveal; a choice, then relief. Three calms in a row is a dead
  stretch even if each is pretty.
- The way ahead should always have a subject: a roof, a mast, a gap in
  the trees. A view down an empty road is fine once, as a breath.
- The side view is where discovery lives: what's worth stepping off the
  route for. If every side view is trees, the walk is a corridor.

## What I've learned

- The stop picker's side view picks the nearest named thing off the way
  ahead, which is often a building you're beside. Look at the frame, not
  the label.
- An arrival faces the way you came in, not the way on: the first walks'
  rendezvous looked due north into the trees, because a route's end has
  no way on, and that read as a dead end that wasn't there.
- A muddy arrival is often fixed by moving the point, not the scenery: the
  place's best frame tends to sit 10 to 20 m short of where its marker was
  dropped. Look for it in the stops before the arrival.
- Find out what's in a bad frame before moving it. Marrow's crest "concrete
  block" was a standing stone of an offering's ring that the route walked
  through; nothing in the map's prefabs listed it.
- A rendezvous keeps 8 m clear of buildings for the truck
  (`map_lint::TRUCK_ROOM`), and its beacon stands 7 m to one side
  (`map_landmarks::beacon_at`), so an arrival never faces the lamp, and
  the place you crossed to get there is behind you. Give the arrival
  something ahead instead: the truck's own track on out through the
  fence (roads make the fence's gates) says "the way out" plainly.
- A walk's fixes move its route: a job or rendezvous moved, the arrival
  stop moves with it. Show the new arrival beside the old, and say so.
