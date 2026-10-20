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
  footprint, tree trunks and big rocks; forest costs more, roads less;
  bridge decks are lanes. The robots and the sim bots walk on this very
  grid, so "reachable" means what they find.
- **The comfort grid**: the same, blocked over 25° (`COMFORT_SLOPE`).
- **Main routes**: the cheapest walk from the insertion to each
  rendezvous over ground no steeper than 16° (`ROUTE_GRADE`), as a player
  taking the easy way would, sampled every 2 m. There must be one. They lean on roads (a
  road cell costs a third of grass), as players do. Routes are what a
  first-timer walks; the rules about pacing, cover and landmarks are
  rules about them.
- **Landmarks**: placements 12 m tall or more (`LANDMARK_HEIGHT`): masts,
  chimneys, towers, the robot Gate. Trees are 10-20 m; a landmark has to
  stand over them or on high ground.

## Landmarks and orientation

In a headset you navigate by what you see, and the weather takes that
away: drizzle leaves you 320 m (`CLEAR_SIGHT`), a pea-souper 150 m
(`FOG_SIGHT`). The compass and the field map help, but the land should do
most of the work.

- From most of every main route a landmark is in sight over the land in
  ordinary weather: at least 70% of the way (`LANDMARK_SHARE`), never
  more than 200 m blind (`BLIND_GAP`).
- Each landmark is one of a kind within sight: two of the same prefab
  closer than `CLEAR_SIGHT` and you can't tell which is which. Masts,
  chimneys, towers: one of each in any view.
- Every region you go to has something of its own: a structure at least
  6 m tall (`REGION_MARK`), or a landmark within fog range of its middle.
  A crater or a pit is a place, but you can't see it until you're in it.
- Put landmarks on high ground or at the far end of long views (the
  mast on its hill, the chimney over a flat valley, the tower on a
  summit), and give each valley one that's visible from nearly
  everywhere. Water runs downhill and roads follow valleys: the creek and
  the road are landmarks too, and a map should let you find one.
- The rendezvous should be where a landmark is (a tower, a depot, a
  pass), so "head for the mast" is a plan.

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
multiplied by `Region::richness`. So a rich region off the road is worth
a bot's detour in proportion to what it pays; a picked-over village near
the gate is worth less. If a place you meant to matter is never visited
in the sims, check its richness and its loot spots before its routes.

## Indoor and outdoor rhythm

Buildings are where the hands-on looting is, and a roof is cover from
drones and the Surge. A raid should alternate: a walk, a search, a walk.

- Along every main route, a building you can walk into within 60 m at
  least every 320 m (`RHYTHM_GAP`). Enterable prefabs are `ENTERABLE`.
- Mix the scale: a village of small houses, a hall you can get lost in,
  a lone hut. Big buildings belong to dangerous places (they're slow to
  clear and robots come while you do).
- Leave open ground between groups: the outdoor stretch is where you
  look around, listen, and decide.

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

## Performance: what's in view

Lavapipe screenshots and a 90 Hz headset both pay for every building in
view. Buildings are the expensive part (dozens of pieces, shadow
casters); trees are instanced with impostors, rocks have LODs.

- At most 16 placements within 150 m of any point on a main route or any
  point of interest (`VIEW_MAX`, `VIEW_RANGE`). A village is a dozen
  buildings, not forty.
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
