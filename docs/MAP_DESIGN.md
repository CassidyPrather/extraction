# Map design

How a valley in this game is laid out, and the numbers that hold it to
that. The game is slow, tense and physical: you walk every metre in a
headset, you loot by hand, you leave by calling a truck to the far side,
and robots are working the valley while you do. A map is designed, not
generated (`assets/maps/*.ron`, format in `crates/extraction_core/src/map.rs`).

Every rule below that can be measured is measured by the **map lint**
(`crates/extraction_core/src/map_lint.rs`). The name in brackets after a
rule is its constant there; tighten one and see what falls out.

```bash
cargo test -p extraction_core map_lint -- --nocapture   # every valley, every finding
scripts/map-preview.sh kettle_pass                      # top-down, routes and findings drawn on
scripts/sim.sh kettle_pass 8 --jobs 3 --seed 1          # then watch bots play it
```

The three valleys must lint clean. The Training Yard is linted too and
must have no unreachable, wet or overlapping things; its other findings
are printed for the onboarding work, which owns it.

## What the lint stands on

- **The walkers' navigation grid** (`map_nav::zone_grid`): 2 m cells,
  blocked by ground over 32°, water deeper than 0.9 m, every placement's
  footprint (anything thinner than a cell, a garden fence, widened to
  half a cell so no path slips through it), tree trunks and big rocks;
  forest costs more, roads less;
  bridge decks are lanes. The robots and the sim bots walk on this very
  grid, so "reachable" means what they find.
- **The comfort grid**: the same, blocked over 25° (`COMFORT_SLOPE`).
- **Main routes**: the cheapest walk from the insertion to each
  rendezvous over ground no steeper than 16° (`ROUTE_GRADE`), as a player
  taking the easy way would, sampled every 2 m. There must be one. They lean on roads (a
  road cell costs a third of grass), as players do. Routes are what a
  first-timer walks; the rules about pacing, cover and landmarks are
  rules about them.
- **Landmarks**: structures 12 m tall or more (`LANDMARK_HEIGHT`): masts,
  chimneys, towers, the robot Gate, Sallow's dam. Trees are 10-20 m; a
  landmark has to stand over them or on high ground. The lint sees them
  as a player does (`map_landmarks`): a sight line goes to the top of a
  tall part where the game builds it (on its settled ground plus `y`, or
  at `y` if `absolute`), a long wall's top every 20 m along it
  (`TOP_STEP`), a chapel at its bell tower, a blast furnace at its stack
  and its chimney; one part in sight is enough. Pieces of one kind within
  30 m of each other are one landmark (`JOIN`): the dam's four faces are
  the dam. A kind is what it looks like from afar: a chapel's tower is a
  steeple, a drowned chapel a chapel. Heights are `map::prefab_footprint`'s;
  a prefab missing there is 3 m tall and never a landmark, so list every
  tall one.

## Landmarks and orientation

In a headset you navigate by what you see, and the weather takes that
away: drizzle leaves you 200 m (`CLEAR_SIGHT`), a pea-souper 150 m
(`FOG_SIGHT`). Drizzle was 320 m until Legibility's wayfinding renders
(docs/LEGIBILITY.md, "Wayfinding") stood at the insertion and every
rendezvous and looked: past about 200 m a structure keeps 3-7% of its
contrast, which is not there to steer by. The compass and the field map help, but the land should do
most of the work.

- From most of every main route a landmark is in sight over the land in
  ordinary weather: at least 70% of the way (`LANDMARK_SHARE`), never
  more than 200 m blind (`BLIND_GAP`).
- Each landmark is one of a kind within sight: two of a kind closer than
  320 m (`UNIQUE_SIGHT`) and you can't tell which is which. That stays
  wider than drizzle sight: a twin that's barely there past 200 m still
  reads as "the mast" and sends you the wrong way. Masts, chimneys,
  towers: one of each in any view. A ropeway's pylons are a line to
  follow, not twins (`SERIES`). The lint also measures, per route, how
  much of it has two of a kind in one view (within `CLEAR_SIGHT`)
  however far apart they are: none now. Marrow Creek's two ranger
  towers, 366 m apart, were both in sight from 20-41% of each route at
  320 m; at 200 m never at once.
- Every region you go to has something of its own: a structure at least
  6 m tall standing in it (any corner of its footprint or any top inside
  the region: a dam whose middle is over the line still counts), or a
  landmark's top within fog range of its middle (`REGION_MARK`). A crater
  or a pit is a place, but you can't see it until you're in it.
- Put landmarks on high ground or at the far end of long views (the
  mast on its hill, the chimney over a flat valley, the tower on a
  summit), and give each valley one that's visible from nearly
  everywhere. Water runs downhill and roads follow valleys: the creek and
  the road are landmarks too, and a map should let you find one.
- The rendezvous should be where a landmark is (a tower, a depot, a
  pass), so "head for the mast" is a plan: from every rendezvous a
  landmark is in sight within 200 m (`RV_LANDMARK`). Within fog range
  (150 m) is the aim, and measured: five of the twelve rendezvous miss it
  (Marrow's Bus depot, Kettle's Hunting lodge, Cinder's North sidings and
  Works yard, Sallow's Inflow bridge), between 151 and 183 m off.
- Only structures count. The heavy machine has no ground of its own (it
  walks out of the Gate to where you are and goes home), so its mark is
  the Gate; staged scenes are furniture at points of interest; the pylons
  the walkers raise come and go. None of them is something to steer by.
- Signs face the way you walk up to them: a Sign marker within 25 m of
  a road or path turns its board within 60° of the road 10 m back
  toward the insertion (or toward the insertion itself, away from
  roads). The board stands at eye height by construction. This is
  checked by `render::legibility::tests::every_sign_faces_its_approach`,
  not by the map lint.

What each valley steers by, as measured (the lint prints, per route, the
share in sight in drizzle and in fog, the longest blind stretch, and each
landmark's share and where along the route it shows):

| Valley | Landmarks | Routes: in sight (in fog), longest blind |
| - | - | - |
| Marrow Creek | mill race, radio mast, water tower, two ranger towers (the survey post, the RV's), the Gate | 91-93% (58-72%), 48 m |
| Kettle Pass | headframe, ropeway station and its pylons, chapel, radio mast, ranger tower, chimney, the Gate | 100% (95-100%), 0 m |
| Cinder Works | blast furnace, chimney, ranger tower, the Gate | 100% (80-89%), 2 m |
| Sallow Reservoir | the dam, draw-off tower, church tower (steeple), water tower, radio mast, ranger tower, sorting gantry, the Gate | 93-100% (90-100%), 50 m |

Every valley reads in drizzle at 200 m, without a landmark added or
moved: only Marrow Creek lost anything when drizzle came down from
320 m (its routes from 100% to 91-93%, its longest blind stretch from
0 to 48 m, all three just south of the creek before the mill race and
the survey post's tower come into view). Kettle Pass and Cinder Works
are tall and close enough together that nothing changed; Sallow's Sallow
Head route went from 48 to 50 m blind. Fog is where they differ: Marrow Creek's
routes are blind in fog for a third of the way (south of the creek
nothing tall stands within 150 m: the insertion sees nothing in fog, the
mill race is 235 m off), and `map_sight`'s hero-site search says where a
tower would help most.

## Routes and choices

The raid is a crossing: in at one edge, out by the truck on the other.
A good map makes the crossing a decision.

- Every rendezvous is at least 350 m from the insertion (`RV_FAR`), and
  the rendezvous are at least 180 m apart (`RV_APART`), so choosing one
  is choosing a side.
- The truck appears on the rendezvous marker and parks with its bed to
  the map centre: nothing is placed within 8 m of it (`TRUCK_ROOM`), and
  the ground there is no steeper than 12° (`RV_LEVEL`).
- To every rendezvous there are **two ways**: block a 60 m wide corridor
  along the main route (all but its first and last 90 m) and there is
  still a comfortable walk there, at most 1.6 times as long
  (`CHOICE_APART`, `CHOICE_ENDS`, `CHOICE_DETOUR`). A creek or a canal
  needs at least two crossings that matter (a bridge in plain view, a
  footbridge in the trees, a ford).
- **The first ten minutes are a choice too**: from the insertion, at
  least two places with loot (a point of interest with loot or a walk-in
  building within 40 m, `CHOICE_LOOT`) are within a 450 m comfortable
  walk (`FIRST_REACH`), each walk with cover all the way (`COVER_GAP`),
  their bearings at least 50° apart (`CHOICE_ANGLE`) and their walks
  parting (no more than half the shorter within 30 m of the other,
  `CHOICE_SHARED`). Not one corridor out of the drop. Now: Marrow 112°
  (Logging camp or Old farm), Kettle 146°, Cinder 88°, Sallow 122°,
  Rime 134°.
- **The first jobs have cover within a four-second sprint all the way**:
  for the first job of each kind that sends you into the valley (the
  first contract step that scouts, brings or plants, and the ladder's
  letter and ledger: `map_lint::first_jobs`), every way there (the easy
  way, straight across, and by any place within 1.3 times the direct
  walk, `FIRST_DETOUR`) has cover within 18 m of every step
  (`FIRST_RUN`: four seconds at a sprint), but for a run of 10 m
  (`FIRST_OPEN`). Cover is any building or prop, a boulder or three
  trunks. The ways home from each to its two nearest rendezvous are
  measured against the main routes' cover, `COVER_GAP` within
  `COVER_REACH` (Onboarding's newcomers fell going home west from the
  Village past the pumping station; laden and warned, not a sprint);
  measured until Sallow's drained bed has its two props, then held. Three of Onboarding's four newcomers on Marrow's mast relay
  died to a drone 7-17 s after its lock in the open past the Village.
  Across the open, cars left on the verges and log piles by the way
  (light props: no view load). And roofs on the way, for a drone: the
  measure gives the farthest any first job walks from a door under a
  roof (Marrow's mast relay: 97 m, by the gate; the pumping station's
  tool shed, the rangers' store and the road-menders' hut past the
  Village).
- **The way out is a choice too**: from every rich place (a point of
  interest or artifact field in a danger-2 region or worse), at least
  two rendezvous are a sensible walk away: within 300 m of the nearest
  (`OUT_EXTRA`) or 500 m (`OUT_REACH`), whichever is further, the
  short way over comfortable ground (a hunted player cuts across, not
  round by the road). Both walks have cover (`COVER_GAP`) and part
  within 150 m (`OUT_PART`). And the last 100 m to each rendezvous
  (`LAST_STRETCH`), where you wait laden for the truck, has no open run
  over 40 m (`LAST_OPEN`) and no more than a quarter of it in a
  spotter's sight (`LAST_SPOTTED`).
- **Waiting for the truck under a drone's eye is survivable**: a
  rendezvous within a survey drone's sight of the point of interest it
  orbits (the orbit, 14 m, plus 90 m: `SURVEY_SIGHT`) has a roof or
  solid cover a body can wait behind (a walk-in building or a kiosk, or
  anything 1.2 m high, `WAIT_HEIGHT`, but a garden fence) within 15 m
  of where the truck stops (`WAIT_COVER`). Don't move the rendezvous: a
  tense wait under a landmark is the design. Put the shelter in the
  place's story (the bus shelter at the depot, the platelayers' hut at
  the rail yard, the lodge's woodshed), 10-13 m off, on the side away
  from the rendezvous beacon (`map_landmarks::beacon_at`: 7 m off the
  truck's side), clear of the truck (`TRUCK_ROOM`) and its tailgate.
- Routes differ in kind, not just in line: the road (fast, open, seen),
  the woods (slow, hidden), the high ground (a climb, a view). The cheap
  way should be the exposed one.
- Robots come from their Gate. Put it on a far edge, at least 300 m from
  the insertion (`GATE_FROM_INSERTION`), so one side of the valley is
  theirs and every route has a near side and a far side.

## Sightlines and cover

Robots see 60-85 m (drones, pylons further), and the player is slow.
Open ground is a choice you make knowingly, not something a route makes
for you.

- Along every main route there's cover within 25 m (`COVER_REACH`, a
  five-second dash when a drone chirps its lock): a building, a boulder
  you can crouch behind (0.6 m across, 0.8 m tall: `BOULDER_RADIUS`,
  `BOULDER_HEIGHT`) or a stand of at least three trees (`STAND`). The
  longest stretch without it is 60 m (`COVER_GAP`), about 25 s at a walk.
  The road can still be the exposed way: cover off to the side, not over
  it.
- A copse narrower than about 30 m comes out thin: trees thin out within
  12 m of a forest's edge and keep 2.5 m off a road's shoulder
  (`forest::scatter_trees`). Draw a roadside stand right up to the road;
  the road keeps itself clear. The preview dots in every tree and
  blocking rock.
- Long sightlines belong somewhere specific: across a field you can skirt,
  down a road you can leave, through a burnt wood. Say on purpose where
  they are (Cinder Works' dead wood has them, and nothing low to hide
  behind).
- Cover comes in clumps, with a gap you choose to cross between them:
  copses, walls, log piles, rock fields, the next house. A bend in a road
  is cover from the far end of it.

## Points of interest, density and pacing

A point of interest (a `Poi` marker) is somewhere worth the walk: a
group of buildings, a site, a hazard with artifacts. The valley is about
560 m inside the fence.

- No two points of interest closer than 60 m (`POI_MIN`): closer, they're
  one place. None more than 190 m from its nearest neighbour
  (`POI_MAX`): further, the walk between is dead time.
- Along a main route, never more than 220 m (`PACING_GAP`, about a
  minute and a half) without something to find within 60 m
  (`NEAR_ROUTE`): a point of interest, a building, a loot spot, an
  offering, an artifact field.
- A road, track or footpath leads to every point of interest: one within
  30 m (`FUNNEL`) of it. Hazards you approach across country (an artifact
  field within 40 m, `HAZARD_NEAR`) are the exception. Paths are the
  paths of least resistance: people (and the sim's bots, on the cheaper
  road cells) follow them to the door.
- Small things fill the gaps: a shed by a field, a boxcar on a siding, a
  hut on a switchback. They're each one or two loot spots and a roof.
- The truck takes about two minutes to come; a scavenging crossing takes
  30-45 minutes. That's 6-10 stops, so a valley wants 8-12 points of
  interest plus small sites.

## Loot value against risk

Risk pays, and the map says where the risk is.

- Each region's loot richness (`Region::richness`: rolls per spot, by
  default `0.8 + 0.2 × danger`) rises with danger: the mean per danger
  tier goes up tier by tier, and no region is as rich as one two tiers
  more dangerous.
- The insertion is on danger-0 ground. Regions of danger 2 and up are at
  least 220 m from it (`DANGER_FAR`).
- Artifact fields lie in danger-2 regions or worse (`FIELD_DANGER`).
- Picked-over places near the insertion (the village by the gate) are
  set below 1 (`loot: Some(0.6)`): the sims showed bots filling their
  packs there and never going further.
- The best loot is where you'd least like to be: under the robots'
  Gate, in the anomaly fields, in the hall in the middle of the works.

### What the bots value

The sim's raiders (`src/sim/raider.rs`, `choose_site`) pick where to go by
what a site holds (loot spots; artifact fields for hunters), how far it
is, the rendezvous side late in the raid, and the region's loot
richness, as a player reading the field map would: a site's pull is
multiplied by `Region::richness`. Each raider also sets out for one
region, chosen at the start weighted by richness (any region with loot
and danger above 0). Sites near that region's middle score up to four
times higher, so far quarters get tried and not only the cluster
nearest the gate. So a rich region off the road is worth
a bot's detour in proportion to what it pays; a picked-over village near
the gate is worth less. If a place you meant to matter is never visited
in the sims, check its richness and its loot spots before its routes.
Offerings and work sites are never bot targets, so "never visited" for
them says nothing about the map.

### How the bots move

Robots see a crouched body in grass and fog far less well, and hear a
crouched creep a third as far (`robots/senses.rs`), so the raiders move
like careful players near them (`raider.rs`, `sneak`). They **crouch**,
and so creep, within `Traits::sneak_range` of a robot they can see that
isn't fighting them (cautious 45 m, looter and hunter 35 m, reckless
never), or near where a robot is searching. They **freeze** 2–4 s when a
robot that's investigating or searching is in sight within 30 m. Any
but the reckless, with junk in its pack, **throws it** about 15 m off to
one side when a walker within 45 m is about but not onto it (at work, or
already looking for someone: the sims showed that's when bots meet
walkers), so the walker goes to look there instead. With `SIM_STALK=1` the cautious play it properly: they creep, crouched,
round behind an unaware walker within 70 m (by way of a point 25 m out
behind it), to 10 m off, and lob the junk 5 m past it. So far the walkers
spot most of them crossing open ground on the way in; they'd need routes
through cover to get there. None of this
happens in a fight, with the truck waiting, or when hurrying home hurt.
Telemetry: `crouch` (seconds crouched), `freeze`, `distract`. Cover and
grass along a route now matter to the sims as they do to a player: a
route through open road beside a work site gets bots seen; one through
long grass behind a wall doesn't.

## Indoor and outdoor rhythm

Buildings are where the hands-on looting is, and a roof is cover from
drones and the Surge. A raid should alternate: a walk, a search, a walk.

- Along every main route, a building you can walk into within 60 m at
  least every 320 m (`RHYTHM_GAP`). Enterable prefabs are `ENTERABLE`;
  an underground kiosk's stair down counts too.
- Mix the scale: a village of small houses, a hall you can get lost in,
  a lone hut. Big buildings belong to dangerous places (they're slow to
  clear and robots come while you do).
- Leave open ground between groups: the outdoor stretch is where you
  look around, listen, and decide.

## The Surge

Five minutes' warning, then at a minute the radio says "One minute. Get
out, or get under something." (`raid::update`), and for two minutes the
Surge kills anyone with open sky over their head (the game casts a ray
25 m up from the head). A roof is any walk-in building (`ENTERABLE`)
with its roof on, anything with a door you walk through (a boxcar, the
signal box, the chapel) and every underground kiosk; not a burnt house
(open behind its front) or a drowned one (`map_surge::roofed`). Walks
are measured on the comfort grid from each door, 1.5 m out from it
(`DOOR_STEP`); an open shed with no doors (the sawmill) from anywhere
round it.

- From every point of every main route, a roof within a minute's walk
  once the call comes: 144 m (`ROUTE_ROOF`, `SURGE_CALL` × `WALK`). A
  first-timer on the road hears it and walks in.
- From everywhere comfortable inside the fence, a roof within a minute
  at the best pace a body keeps up: 234 m (`ANYWHERE_ROOF` = `dash(60)`:
  sprinting whenever stamina allows, 12 s of it fresh and 3 parts in 5
  after, not the 276 m a minute's sprint would be). A patch out of reach
  is a finding at its farthest cell (`PATCH_MIN` and up).
- Why a minute, not the five minutes' warning: a five-minute walk is
  720 m, and no ground in any valley is that far from a roof (the
  farthest is 223 m), so a rule at five minutes says nothing. The minute
  after the call is when it's decided.
- The Surge's anomalies, over 8 seeds × 4 cycles (`SURGE_SEEDS`,
  `SURGE_CYCLES`), and the standing ones, keep their reach
  (`AnomalyKind::danger_radius`) 2 m off every wall (`WALL_CLEAR`: a
  building's footprint, a kiosk's with its apron) and out of the 4 m in
  front of every door (`DOOR_STRIP`; `map_surge::anomaly_room`). The
  layout and the run-time blooms keep to it by construction
  (`anomaly::clear_of_buildings`); a snare once opened 2 m from the
  factory hall's wall.
- `SURGE=1 scripts/map-preview.sh <map>` shades the ground over a
  minute's walk from a roof, brighter the farther.

## Anomalies

Anomalies are hazards in the land, laid out fresh each Surge
(`anomaly::layout`): fields of 5-8 around each `ArtifactField` marker,
spread 28 m (`FIELD_SPREAD`), and a handful loose anywhere.

- None reaches onto a road or a path (over `ANOMALY_SEEDS` layouts):
  the road is the safe, exposed way, and a shimmer across it is a trap you
  can't see coming in VR. (`anomaly::placeable` keeps them a radius and a
  margin clear.)
- An artifact field's middle is at least 35 m from every main route
  (`FIELD_CLEAR`): you go to it, it doesn't come to you.
- Fields sit in terrain that says "something's wrong here": a crater,
  a pit, a heap, dead trees. Give the approach a view down into it.
- Loose anomalies can land on a cross-country route; that's what they're
  for. Watch the ground.

## Sentries

A sentry (`MarkerKind::Sentry { arc, power }`, the machine in
`robots::sentry`, its rules in `extraction_core::sentry`) is a turret on
a 4.6 m mast over something that matters. It is built to be outwitted:
it sweeps a fixed arc, is blind behind, chirps a second before it fires,
and dies when its cable is cut. The map's job is to make each one a
puzzle, not a wall. `marker.rot` is the middle of its arc as
`sentry::bearing` measures (0 toward -Z, 90 toward +X); `power` is its
junction box.

- **A field, not a wall**: its head sees at least 60% (`FIELD_SHARE`)
  of the walkable ground its sweep can reach within 55 m (`sentry::SIGHT`),
  through the land and the buildings (trees don't stop rounds).
- **It guards something**: a Gate, a key, a point of interest, a work
  site, loot or an underground site within 40 m (`GUARD_REACH`).
- **A way round**: a walk from the insertion that never enters its field
  gets within 8 m of the mast (`WAY_ROUND`): behind it. Put its back to
  a fence, a wall or a slope with a way along it.
- **The box on that unseen ground**, 8–25 m from the mast (`BOX_NEAR`,
  `BOX_FAR`), inside the fence: finding the cable and following it is
  the puzzle.
- **Never over the way in or out**: not the insertion, not a rendezvous,
  its tailgate or the approach to it.
- **Seen before it sees you**: where a main route walks into its field,
  its head is in sight 25 m back along the route (`WARN_RUN`).

Each valley has one over its Gate (Marrow's covers the Depot mouth too,
25 m off) and one over the pit or shelter where its key lies, facing the
way down. To site a new one, `SCOUT_SENTRY=<map>:<x>:<z> cargo test -p
extraction_core scout_sentry -- --ignored --nocapture` tries spots
round a point and prints the ones that pass, facing the way people come
first. The preview draws each one's sweep (red) and cable (yellow).

## Spotters

A spotter (`MarkerKind::Spotter`, the machine in `robots::spotter`, its
rules in `extraction_core::spotter`) is a balloon on a 24 m tether over a
winch at `marker.at` (`rot` is unused). It is an alarm, not a gun: its
level eye (100° wide) turns a full circle every 40 s and sees 85 m; held
in view for 3.5 s it calls a drone onto you. Straight under it, within
about 20 m of the winch (`spotter::blind_radius`), it is blind, and
cutting the tether at the winch sends it off downwind. A map uses one to
make you use the ground's cover, or time its turn, crossing open ground.
The lint (`Rule::Spotter`, `map_spotter`) sees from the envelope to a
standing body through the land, the buildings and the kiosks (not trees:
its field is an upper bound).

- **The winch on good ground**: walkable, dry, no steeper than 25°
  (`COMFORT_SLOPE`), inside the fence.
- **It watches something**: a key, a point of interest, a work site,
  loot or an underground site within 85 m.
- **A way in under it**: a walk from the insertion to within 3 m of the
  winch (`WAY_IN`) crossing no more of the ground it sees than you walk
  while its eye is turned away: 69 m (`WAY_SEEN`, 2.4 m/s for the 29 s
  of a turn its view isn't on you). On flat open ground the whole ring is
  less than that; it bites where the land rises round the winch and there
  is no blind ring. (A sentry's way round never enters its field; a
  spotter in the open has no such way, by design.)
- **Never over the way in or out**: not the insertion, not a rendezvous,
  its tailgate or the approach to it.
- **Not stacked on a sentry**: the ground it sees covers at most half of
  any sentry's field (`STACK_SHARE`). A spotter calling drones onto a
  turret's ground is the pairing the sims found deadly in Marrow. Measured
  across the valleys, a winch within about 40 m of a sentry sees 55-98% of
  its field, one 60 m off 36-46%, and the placed ones 0%.

The lint also measures, for reading against the sims: the main routes in
its sight (metres, and the longest seen run), the work sites within its
sight (a pylon there watches the same ground and calls too), and the
survey drones' orbits and lanes it sees. Place one over open ground a
main route crosses on the way to a key site, where no sentry or pylon
already watches. Marrow's is on the road north to the bus depot and the
rail yard, Kettle's where the three ways north part, and Cinder's on the
west way to the fire lookout across the ash pit (the east ways have the
chimney's sentry). To site one, `SCOUT_SPOTTER=<map>:<x>:<z> cargo test
-p extraction_core scout_spotter -- --ignored --nocapture` tries winches
round a point and prints the ones that pass, most main route in sight
first, with their measures. The preview draws its 85 m sight and its
blind ring (orange).

## Mines

Once a raid the minelayer (`robots::minelayer`, rules in
`extraction_core::minelayer`) crawls out of the Gate and lays a field of
four pressure mines at each of two points of interest (`FIELDS`): the
`Poi` markers on walkable ground nearest the insertion, the very nearest
skipped. It lays them across the way it came, from a hopper of six, and
never within 3 m of a door strip, 0.5 m of a road or 2 m of a building
(`minelayer::spot_ok`). So which markers are `Poi`s, and where they
stand, decides where the mines go. The lint (`Rule::Mines`, `map_mines`)
works the fields out as the game does and holds them:

- **Not on a main route**: no mine within its plate and a boot (0.82 m,
  `ON_ROUTE`) of a main route's line. A field is a find, not a coin toss.
- **Not in a doorway**: no mine in a door's strip or a kiosk's.
- **A site with room for a field**: at least half its four mines pass
  `spot_ok`. A `Poi` marker at a building's door or inside its 2 m has
  room for none; stand it in the yard in front.

The measures line lists each field, its mines, how many the hopper had
left for it and the nearest main route. A `Poi` inside a building isn't
walkable and isn't a site at all, so Marrow, whose points of interest
are nearly all in buildings, gets one field (the Village). The preview
dots the mines red.

## Robot patrol space

Robots don't patrol at random: they walk out of the Gate to work sites
(`WorkSite` markers) to build pylons and guard cargo pods, survey points
of interest, and answer noise.

- Walkers can get from the Gate to every work site on their own grid.
- Work sites are at least 150 m from the insertion (`WORK_FROM_INSERTION`)
  and 40 m from any rendezvous (`WORK_FROM_RV`).
- A work site has an open yard: at least 70% of the ground within 16 m
  walkable (`WORK_YARD`, `WORK_OPEN`). Pylons see 85 m from the top of
  their mast; put sites where that view covers a route you might take,
  with cover to approach under.
- Between the Gate and the work sites lies the robots' country: open
  enough for walkers (they path over 32° at most and around every trunk),
  crossed by the routes rather than running along them.

## VR comfort

You walk with a stick and turn with your body, often for an hour. What's
fine on a monitor makes people sick or tired in a headset.

- **Slopes**: everything that matters is reachable without walking up
  ground steeper than 25° (`COMFORT_SLOPE`). Main routes climb no more
  than 16° over any 10 m (`ROUTE_GRADE`, `GRADE_RUN`), bridge decks
  counted at the deck. The body controller climbs far steeper; that's
  for scrambling on purpose, not for the way home.
- **Roads**: asphalt and gravel no steeper than 14% over 10 m (a steep
  mountain road), tracks and
  footpaths 22%, rail beds 6% (a mine line's grade) (`ROAD_GRADE`, `TRACK_GRADE`, `RAIL_GRADE`),
  from 12 m inside the fence (`FENCE_MARGIN`: roads climb the rim to it). Switchback up a hill rather than straight at it.
- **Road ends**: where a road stops you walk on, so the ground meets it:
  within 0.6 m three metres on (`END_STEP`, `END_REACH`), and no rise
  over 0.35 m in any half metre for eight (`LEDGE`, `END_WALK`). A graded
  road cut into a slope otherwise ends at a bank you can't step up (the
  sims found one at the foot of Cinder Works' slag heap, and no bot ever
  reached the field above it). Roads that end on another road or at a
  door are exempt. End a road where it's at grade, or run it on to a door.
- **Steps**: the body steps up 0.4 m (`player.rs` `STEP_HEIGHT`). Stairs
  and kerbs are the prefabs' job (the building fit gauge in
  `extraction_core::fit` holds plinths to the ground); the map must not
  leave a building's door above a drop.
- **Doors**: in front of every outside door (`map::prefab_entrances`),
  a strip as wide as the opening and 0.3 m either side, from the wall
  out past its steps and at least 4 m (`DOOR_SHOULDER`, `DOOR_REACH`), is
  clear: no other building or site kiosk, trunk, solid rock, supply
  crate, sign, offering or standing anomaly, and no water. Past the
  steps the ground meets them within a step (the steps reach down to
  the building's footing less its lift; `DOOR_STEP`, the body's 0.4 m)
  and is no steeper than `COMFORT_SLOPE`, and the walkers' grid is open
  there and reached from the insertion. The lint prints how many doors
  a walker, which steps only 0.28 m (`WALKER_STEP`), couldn't step to.
  A booth or hut at the fence turns its door away from the rim's rise.
  A road ending within 5 m of a door's strip runs on to it
  (`DOOR_ROAD_END`). Boxcars are a climb on purpose: their strip is
  kept clear but their ground isn't held. The forest and rock scatter
  keep off door strips (`map_doors::clear_of_doors`). Staged scenes
  (`assets/data/scenes.ron`) are the game's and aren't checked.
- **No pockets**: the walkers' grid judges each 2 m cell by the slope at
  its middle, so a bank between two middles is invisible to it. Every
  step between walkable neighbours is walked every 25 cm (bridges at
  their decks); one that climbs or drops more than a step (0.4 m,
  `BODY_STEP`) over ground steeper than the body stands on (49.5°,
  `BODY_SLOPE`, the game's `walk_over`) is a false step, and ground the
  insertion reaches only over false steps is a pocket (`map_pockets`,
  `POCKET_MIN` 16 m²). None in the five valleys; the measure prints each
  valley's false steps.
- **No forced jumps**: nothing that matters is only reached by jumping,
  climbing or swimming. Water deeper than 0.9 m is a wall on the nav
  grid, and the lint's reach is on that grid.
- **Narrow passages**: footpaths at least 1.4 m wide, roads 3 m
  (`PATH_WIDTH`, `ROAD_WIDTH`). Two buildings either touch, or leave a
  gap too narrow to try (under 0.6 m, `SEALED`), or leave at least 1.5 m
  (`SQUEEZE`): nothing you'd have to turn sideways for with a rifle on
  your back.
- **No crawling**: nothing that matters needs crouching to get through.
  Doorways and decks are the prefabs' (the sim's door test walks every
  door).
- **No overlap**: buildings don't overlap each other or sit on a road
  (except those built to be driven into or over: halls, the barn, the
  sawmill, gates, boxcars on rails); a road may end at a building's door.
- **Footing**: nothing you go to (rendezvous, loot, offerings, keys,
  work sites, notes, signs) is in water or on ground steeper than 25°,
  and no placement stands in water.
- **Kiosks are placements.** An underground site's placement is its
  entrance kiosk and apron (`underground::FOOTPRINT`); its exit kiosks
  stand on the surface too, tens of metres off (`underground::kiosks`).
  The lint holds every kiosk to the placement rules (`map_lint::solids`,
  the entrance counted once): no building on its apron, no squeeze
  between its walls and a building's (the apron is ground you walk
  over), not on a road (a road may end at its door), no crate within
  `CRATE_ROOM`, no truck within `TRUCK_ROOM`; an exit's levelled pad not
  in water, and the ground 2 m out from every kiosk's door
  (`KIOSK_STEP`, its -Z end) neither water nor steeper than 25°. A kiosk
  counts in the view load, as cover, as a way indoors for the rhythm and
  the Surge, and its door as somewhere you must be able to walk to.

## Gardens and fences

A house with a garden fence round its back is a house someone lived in,
and its planks are something to shoot through or break down
(`fence_garden`: four breakable plank panels between five posts, an
8.4 m run, footprint 8.5 x 0.2).

- A garden is behind or beside its house, never on the door side (the
  door strip keeps it off anyway), and squared to the house: each run is
  turned with the house's `rot`.
- Two runs and the house's wall make an L with its fourth side open: you
  walk in, and so do the walkers. The paddocks behind the farmhouses are
  two runs deep and two wide. A gate in a run wants 3.5 m to show on the
  walkers' 2 m grid, which narrows every gap by its 0.6 m clearance both
  sides; to them a 1.5 m gap is mostly a wall.
- Runs touch their house (0.1 m off it) and each other, or stand at
  least `SQUEEZE` off, like any placement.
- Off roads and paths (`Overlap` finds a run on one), and off the main
  routes: no route within 0.5 m of a run (`FENCE_CLEAR`). The grid blocks
  a run only where a cell's middle falls within 0.7 m of it, so a route
  could otherwise be drawn through one.
- A fence isn't levelled for (`prefab_wants_pad`): a pad for a run that
  meets a house's back wall reached under the house and lifted its door
  off its steps. It stands on the lowest ground under it, and the ground
  may rise at most 0.3 m along it (`FENCE_RISE`), so put gardens on the
  flat: Kettle's Miners' Row stands on the kettle's sides and has none.

Marrow Creek has five gardens (four village houses, the foreman's cabin)
and the farm's paddock, 14 runs; Kettle Pass four (the gatekeeper's and
weir cottages, the camp cabin, the lodge's second cabin) and the low
farm's paddock, 12 runs.

## Wrecks

Cars left where they stopped (`wreck_lada` 1.8 x 4.3, `wreck_uaz`
1.9 x 4.1, `wreck_lorry` 2.6 x 6.4; front to local -Z): no way in, no
loot, a body to crouch behind, and doors, bonnet, boot and wheels that
come off (Destruction's breakables). They're cover, and they say what
happened on the roads out.

- **On the verge, never the road.** Just off the carriageway, angled as
  if pulled over or slewed; `Overlap` finds one on a road (they aren't
  among the prefabs a road may run into). The road stays the exposed
  way: a wreck beside it is cover a dash off the tarmac, not a wall
  across it. Wider than a nav cell with its clearance, a wreck is walked
  round, so no `FENCE_CLEAR`-style check is needed for routes.
- **Where the long open stretches are.** The cover measure counts them
  like any placement; put them where a route's longest open stretch runs
  along a road.
- **On the ground as it is.** A wreck isn't levelled for
  (`prefab_wants_pad`) and the ground round it isn't settled the way
  it is round a building (`Heightmap::erode`): it stands level on the
  lowest of its corners and middle, and the ground under it may be at
  most 0.3 m over or under that (`WRECK_RISE`), a wheel sunk to its hub.
  Road verges cut into a hillside are often steeper than that; slide
  the wreck along to where the verge is flat.
- Like any placement: off door strips, out of the truck's room at a
  rendezvous, `SQUEEZE` or `SEALED` from buildings and each other, out
  of water; the trees and rocks keep their distance (`prefab_clearance`).
  The Surge's anomalies keep off them by construction.
- They're light props in the view load (`VIEW_LIGHT`).
- Keep them out of the bot scenarios' ground: the insertion and the 60 m
  of road north of it (`src/bot/scenarios.rs` sets targets and robots
  there).

Marrow Creek has five (the queue for the gate, three run into each
other on the district road's west verge; one pulled over north of the
logging-road turn; the farm's jeep in its yard), Kettle Pass four (Lorry
12 on the inside of the switchbacks' third bend, as its driver's note
says; two on the climb to the pass; the low farm's car), Cinder Works
four (a works lorry in the yard between the garage and the hall; one
turned back from the gate; the woodward's jeep on the lookout track; one
on the works road out north). Cover within 25 m along the routes, before
and after: Marrow's routes 95/96/93% with 30/30/22 m open at most, now
100/100/97% and 0/2/20 m (the district road between the logging-road
turn and the village was the gap); Cinder's North sidings route 93% and
30 m, now 98% and 12 m (the lorry in the works yard); Kettle's pass
route 98% and 10 m, now 99% and 4 m.

## Terraces

A plateau with a `height` is ground the map declares: a terrace, a
ledge, a dam's crest, a stepped street. Fitting a building in
(`terrain::settle`: flat under its walls, then a ramp of 0.5 m per metre
back up to the ground, out to 8 m) never digs below a declared terrace
the building doesn't stand on. A road's fill on top of one it may still
trim. On its own terrace a building settles as anywhere else, so put it
wholly on the terrace or wholly off it: one that hangs off the edge
stands on its lowest corner and lowers the terrace to it. `map_lint`
builds the land with and without settling and finds any declared ground
taken more than 0.5 m below its level (`TERRACE_CUT`). Sallow's valve
house dug the dam-face stair down to a stuck cluster before this.

## Performance: what's in view

Lavapipe screenshots and a 90 Hz headset both pay for every building in
view. Buildings are the expensive part (dozens of pieces, shadow
casters); trees are instanced with impostors, rocks have LODs.

- At most 16 placements within 150 m of any point on a main route or any
  point of interest (`VIEW_MAX`, `VIEW_RANGE`), an underground site's
  exit kiosks among them. A village is a dozen buildings, not forty.
  Light props (`VIEW_LIGHT`: the garden fence, nine boxes a run, no roof;
  a car wreck, one body and its dozen small parts) aren't counted; the
  measure prints how many are in the worst view on their own (12 in
  Marrow's, 7 in Kettle's, 3 in Cinder's).
- Spread groups out so no view takes in two big ones at once.
- `EXTRACTION_RENDER_STATS=1 scripts/shot.sh ...` reports what a view
  actually draws (`docs/QA.md`, render load).

## Working on a map

1. Lay out the land first: the spine (a creek, a canal, a climb), the
   high ground, the hollows. Then the roads, following the land.
2. Place the points of interest against the rules above, then the
   regions and their danger.
3. `scripts/map-preview.sh <map>` and read the findings drawn on it;
   `cargo test -p extraction_core map_lint -- --nocapture` for the
   measurements.
4. `cargo test -p extraction_core fit` (buildings meet the ground) and
   check `src/bot/scenarios.rs` before moving anything a scenario uses.
5. `scripts/sim.sh <map> 8 --jobs 3 --seed 1` before and after, and
   compare `target/sim/<map>/report/`: where bots go, die, get stuck, and
   what nobody visits.
6. Look at it: `scripts/shot.sh`, standing on the route, in fog
   (`--weather fog`).

What the lint can't judge (is this place interesting, does the approach
feel tense, does the view reveal the next goal) is what the sims and your
own eyes are for.
