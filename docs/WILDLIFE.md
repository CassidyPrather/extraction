# Wildlife

The valleys' life besides the machines: atmosphere, and information. A
hunter reads a wood by what the animals do; so can you, and so can the
machines. Code in `src/wildlife/`, models in `art/animals/`.

## Rooks, ravens, jackdaws

Each valley has a few flocks (`wildlife::flock`): rooks in Marrow Creek
and the yard, ravens in Kettle Pass (fewer, bigger), jackdaws in Cinder
Works (small, greyer). A flock sits along the top of a building, a mast,
a water tower or a ropeway pylon and goes up all at once when something
comes too close:

| What | How close |
| - | - |
| You, standing still | 6 m |
| You, moving | 6 m + 6.5 m per m/s: walking 15 m, sprinting 38 m |
| You, crouched | 0.6 of that (a crouched creep gets to within 6 m) |
| A walker or hound | 22 m |
| A drone | 30 m |
| The airship | 70 m |
| A shot or a blast | 0.6 of how far it carries (a rifle: 130 m) |

For a minute and a half after a fright they're warier (1.3 times as far);
at night they roost (0.6 times as far) and don't move on of their own
accord. Left alone, a flock moves on now and then anyway (every 3 to 9
minutes), unhurried.

Flushed, the birds nearest the fright go first, burst up off the roof,
wheel overhead for 10 to 22 seconds, then fly to another building 60 to
280 m away that has room for them all and nothing within 45 m of it, and
come down one by one. What that tells you:

- **A flock going up over there**: something is there. Black specks
  wheeling above a roof show from hundreds of metres in clear air.
- **A flock that won't come down where it was headed**: something is
  there too (they go round again and pick another).
- **The Gate's rooks**: machines coming out put them up.

What it tells the machines: a flock *you* flush is heard by robots within
60 m (`flock::ALARM_REACH`), as a noise from you at the flock (6 m over
the roof, where the birds are). A robot that hears it comes to look where
the birds were, not where you are. At most once a minute a flock
(`ALARM_EVERY`). Flushed by a machine or a shot, they raise no alarm: the
machines don't care what scares birds, only what scares them. It goes
through the robots' ordinary hearing (`robots::Noise`), so walls and
hills and rain count as for any sound; `wildlife::Flushed` is the message
for anything else that wants to know (sound, the robots' minds).

### Perches

`wildlife::perches` finds them from the buildings' own pieces (the
`world::buildings::prefab` geometry the colliders are made from): a ray
straight down every 0.5 m over each building, solved against every piece
(boxes and cylinders), keeping spots at the top (within 1.4 m of the
highest), flat enough, at least 2.4 m off the ground, with room for a
bird's body and solid under its feet. Nothing is ever above a perch.
Underground prefabs are skipped. About 30 sites and 1200 perches a
valley, in 45 ms at startup. The same pieces are `perches::Obstacles`,
which birds in flight keep out of.

### Flight

Boids round a goal: a point wheeling round a circle over the roof, then
the next site. Each bird keeps its own place in the flock, apart from its
neighbours, with them, at least 8 m off the ground, and steers up from
anything built ahead of it; anything that still gets it inside a piece or
under the ground puts it back and lifts it out. Landing, it comes in from
above, slowing, onto a perch of its own.

### Looks

`art/animals/rook.py` (`assets/models/animals/rook.glb`): sitting, the
wingbeat (up, mid, down), gliding, and far stand-ins for sitting and
flying. 328 triangles near, 72-84 far. The game swaps poses rather than
skinning: each pose's primitives are merged into one vertex-coloured mesh,
so a bird is one entity and one draw, and every bird in the same pose
batches with the others. One material a species (a raven glossier, a
jackdaw greyer). The wingbeat runs 3.2 to 6 beats a second by how hard it
works; turning, it banks. Past 45 m the far stand-in, past 380 m nothing.
No shadows.

### Set pieces

Story's staged scenes (`world::scenes`: a row of kept buses over Sallow's
empty bay four, a yard of luggage at Marrow's depot) are perches too,
added as they're staged (`perches::scene`), as low as 18 cm (crows on the
cases), and their pieces are obstacles in flight. Flocks pick a scene six
times as often as a building.

### Rime Fell

The high fell has alpine choughs (the rook, smaller and blacker, flocks of
8 to 16) on its ore mill, observatory and funicular, and chamois (the
deer's ways, a little quicker, a dark coat) on the highest open ground. A
herd's home needs no forest edge in a valley without one.

### The heron

Sallow Reservoir has rooks on the dam's parapet, the draw-off tower and the
gantry (perches come from any generated prefab), and a grey heron standing
in its shallows (`perches::wading`: water 8 to 32 cm deep, gathered in
24 m cells, the bed under its feet; `art/animals/heron.py`, in the rook's
pose names). It goes up from 1.6 times as far as a rook, with slow deep
wingbeats, and only ever comes down in the shallows. On a bare reservoir
bed, a heron lifting off the water is the tell from the slipway.

Flying birds keep 3 m over the top of anything built within 4 m of them
(`flock::OVER`), landing too until they're nearly down: a flock came to a
roof past Cinder Works' blast furnace and the stragglers were caught
going in and out of its top.

## Deer and dogs

One larger animal a valley (`wildlife::herd`): two to four roe deer in
Marrow Creek, Kettle Pass and on Sallow Reservoir's bed, three or four stray dogs in Cinder Works.
They live on the walkers' nav grid (`robots::walker::Nav`), so never in
a building, a rock, a trunk or deep water; a herd's home is open ground
at a forest's edge at least 150 m from the insertion point.

**Deer** graze, heads down, wandering a little, looking up now and then.
Something coming and they look up and turn to watch it; come on and they
bolt into the trees and settle somewhere else:

| What | They look up | They run |
| - | - | - |
| You | 20 m standing, 20 + 16 m per m/s moving (walking 42 m), up to 80 | at 0.6 of that, or after 5 s of you still coming |
| You, crouched | 0.55 of that (a creep at 0.5 m/s: 15 m) | likewise |
| A walker or hound / drone / the airship | 50 / 45 / 90 m | at 0.6 of that |
| A shot or a blast | | within 0.7 of its reach |

A run is flat out (10 m/s) for 8 s, then a trot, until whatever it was
is 110 m behind (or 25 s), the way most open and away, favouring cover.
The roe's bark and the bolt are `herd::Startled` (for sound).

**Dogs** keep their distance from people: one comes within 40 m and they
trot off to keep 28 m from them, watching; within 34 m they bark at you,
and a bark is heard by robots within 70 m as a noise from you at the
dogs (once every 45 s a pack). They fear the robot hounds above all: a
hound within 45 m and they run; one it catches (1.4 m) is killed and lies
where it fell. A dog a hound is chasing is a hound coming.

**Looks** (`wildlife::herd_look`): a procedural gait in the house style
of `robots::anim`. A walk (each foot in turn, 68% of the cycle down), a
trot (diagonal pairs) and a bound (fronts, then hinds, the body arching),
by speed with some hysteresis; each foot stays planted until its turn,
then swings in an arc to just ahead of the hip; two-bone legs reach it
(front knees forward, hind hocks back). The body rides on the legs,
bobbing with the gait and pitched with the slope; the head goes down to
graze and up to look, turned toward what it's watching. Models in parts
(`art/animals/deer.py`, `dog.py`: body, head on its neck pivot, upper and
lower legs, a far stand-in past 90 m), 0.7k triangles an animal near,
ten draws. Body and head cast shadows, legs don't.

## Shot, and kept

Every bird of a flock, deer and dog has a body (`wildlife::bodies`; not
yet the ducks, bats or fish): a
collider that follows it, in the robots' layer and colliding with
nothing, so bullets, fragments and blasts find it and nobody bumps into
it. A hit kills. A bird drops off its perch or out of the sky and lies on
whatever's under it; a deer or dog falls where it stood. Every death, a
shot or a hound's catch, leaves a `Carcass` entity: the Custodian keeps
everything, "animals included" (`docs/LORE.md`), and a robot that takes
the carcass (despawns it) takes the body with it. `bodies::Killed` says
so, and the sim logs `animal_killed`. The shot itself still flushes every
flock and herd in earshot.

## Fish

Trout in the creeks (`wildlife::fish`): the water near you is cut into
14 m cells, and about half the cells with water 0.45 m deep have a shoal
of two to five, always the same for the seed. They hold nose into the
current between the bed and the surface; someone on the bank within 6 m
(11 m wading or moving fast), a walker in the water, or a shot within
40 m and they dart off downstream (3.5 m/s) and hide, drifting back after
12 s. Fish there means nobody's been by. They never leave water 0.3 m
deep. Drawn within 30 m (`art/animals/fish.py`, 118 triangles).

## Ducks

Mallards on open water (`wildlife::ducks`): a tell across water. The
valley's water is sampled every 2 m for places a raft can sit: at least
0.25 m deep (enough to float), with deep water at least 3 m all round
and nothing built within 3 m (bridges, the dam). Every valley has some,
and gets one or two rafts of three to nine, drakes and hens, at places
fixed by the map and the seed, the roomiest water likelier (the second
at least 120 m from the first): Sallow Reservoir's upper arm (round
(-180, -170), surface 47.6) nearly always has one, Marrow Creek, Kettle
Pass and Cinder Works their creeks' wider stretches. None of the four
valleys goes without; the Training Yard has no water.

They paddle slowly about their place (0.3 m/s), keeping together by a
few boids rules on the surface, and now and then one up-ends to dabble,
tail to the sky. Someone within 25 m and they swim off, slowly
(0.8 m/s), along the water away from them, keeping their distance;
within 12 m, or running (3.5 m/s) within 25 m, or a shot within 0.6 of
its reach, and they go up: a run of a few metres splashing along the
surface, then low and fast (15 m/s, 3 m over water, 12 m over land and
over anything built) to other open water 70 to 420 m off and 60 m from
whatever scared them, more often onward than back past it, and down
with a skid. Crouched you get 30% nearer; after a fright they're 30%
warier for two minutes. Machines scare them the same. A raft a player
flushes sends the flocks' `Flushed` (its `flock` is the raft's entity,
`birds` its ducks; look for `ducks::Raft` to tell), with `alarm` at
most once a minute a raft, so the robots hear it the way they hear
rooks. On the water a duck is always at the surface of water deep
enough and outside anything built; in the air never under the ground or
the water.

Drawn like the birds (`wildlife::duck_look`, `art/animals/duck.py`):
one merged, vertex-coloured mesh a pose, drakes (green head, white
collar, chestnut breast, grey flanks) and hens (browns) from the same
model, a splash in the run-up's and the skid's poses, far stand-ins
past 40 m and the whole raft in `visuals::FarParts` past 60 m.

## Moths

At dusk and at night (not in heavy rain), seven moths flit round each of
the nearest six lit lamps within 35 m of you: any point light, house
lamps, lanterns, the camp's (`wildlife::small_look`; visual only).

## Bats

At dusk and at night (not in heavy rain), small groups of bats, three to
eight, hunt in loose, erratic loops over the water, along the outside of
the woods' edges and round the buildings, 3 to 15 m up
(`wildlife::bats`). By day they roost in the buildings, out of sight. As
with the fish, the valley near you is cut into 40 m cells and about half
the cells with water, a wood's edge or a building in them have a group,
always the same for the seed; they're kept within 90 m of you. Each bat
flies at 6 to 9 m/s and swerves every quarter to whole second, turning
hard. None is ever inside or under anything built, or lower than 2 m
over the ground or the water. What they tell you:

- **Bats veer off a light**: they keep 9 m from a lamp (the houses'
  lamps, any other lit near you) and out of a torch's beam (30 m long, a
  cone 40 degrees across, with a margin): your torch on the creek clears
  a lane through them, and a hole in their loops where you'd expect none
  is a light, or was.
- **A shot scatters them** (as far off as it flushes a flock): they're
  off flat out for three seconds, then back to hunting.
- **A machine within 40 m of where they hunt sends them off**: gone
  after five seconds, and back only a minute after nothing's been near.
  No bats over a stretch of water at dusk, when the next one along has
  them, says something's been there.

Drawn within 60 m (`art/animals/bat.py`: wings up and down, swapped ten
times a second, 232 triangles; a 64-triangle stand-in past 25 m), a
little over life size so they read against the dusk.

## The chorus

`wildlife::insects::Chorus` is how loud the small life round you is, 0
to 1, for sound to play: crickets at dusk and night, flies and small
birds by day. Machines hush it (silent within 60 m of a walker, hound or
the airship and 120 m of a drone, Audio's masking radii, then a ramp out
to 1.6 times that); a shot or blast, a flock going up or a herd bolting
within 150 m
drops it to almost nothing, and it comes back over a minute; rain and
wind thin it. When the insects stop, something is close.

## Checks

- `wildlife::perches::tests`: rays against boxes and cylinders; every
  valley has at least 8 sites and 150 perches, none inside or under
  anything built, none below 2.4 m.
- `wildlife::flock::tests`: flush distance by pace and crouch; walking,
  sprinting and creeping straight at a flock in every valley, it goes up
  within a tick's walk of its radius; machines and shots flush it; eight
  minutes of someone tramping between flocks and shooting, per valley,
  with no bird ever inside anything built or under the ground, none
  faster than it flies, and every flock down on a roof again after;
  determinism.
- `wildlife::app_tests`: in the whole game, a walker 35 m off with its
  back to you walks over to where you flushed the birds; standing still
  14 m off, nothing. No bird is ever inside any collider (buildings,
  bridges, trees, rocks, fences, the ground) by the physics' own point
  queries while you chase a flock round.
- `wildlife::herd::tests`: every valley has a home for its herd; walking
  at deer they look up within a tick of the walk's alert radius and bolt
  before you're at their flight distance (a crouched creep gets within
  18 m); six minutes of being chased at a run and shot at, per valley,
  never puts an animal anywhere it can't stand or off the ground, and
  they get away; dogs keep more than 10 m from you and bark once; a hound
  takes a dog; determinism. `herd_look::tests`: knees reach the foot and
  bend the right way; gaits follow speed and keep feet down.
- `wildlife::fish::tests`: every valley has shoals; walking to the bank,
  wading in and shooting, no fish leaves deep water or the space between
  bed and surface, and they dart.
- `wildlife::bats::tests`: out only at dusk and at night (and in the
  whole game: bats on Marrow Creek's creek at dusk and night, none by
  day); four minutes per valley of someone walking round the groups with
  a torch on them, shooting now and then, and a walker coming by: no bat
  ever inside or under anything built or below 2 m, none faster than
  9 m/s, and they scatter, leave and come back; with a torch pointed
  across their beat they keep further from its beam than in the dark;
  determinism.
- `wildlife::ducks::tests`: every valley has open water and one or two
  rafts on it (Sallow Reservoir's upper arm for nearly every seed); four
  minutes paddling, alone and with someone pacing 18 m off, keep every
  duck at the surface of water deep enough, together, and dabbling;
  walking at them they swim off inside 25 m and go up at 12 m, running
  at 25 m, crouched not at 10 m, a walker at 11 m and a shot 120 m off
  too; flushed again and again they never go inside anything built or
  under the ground or the water, land on open water at least 48 m from
  the fright, and raise the alarm at most once a minute; determinism.
- `wildlife::app_tests::animals_can_be_shot_and_their_carcasses_collected`:
  a ray down the ballistics' own filter finds a deer's body; a hit kills
  it and leaves a carcass; collected, the body and its collider go; a
  bird shot off its perch lies on what's under it.
- `perches::scene_perches_are_on_top_of_things`, `flock::empty_tests`
  (a map with nothing built has no flocks), `doc_tests` (this file stays
  whole: a bad edit once wrote one paragraph into it thirteen thousand
  times).
- The gauges (`render::budget`): everything far off is one mesh
  (`visuals::FarParts`: flocks and rafts of ducks past 60 m, deer and dogs past 90 m), so
  the animals cost a view one draw beyond the near ones.
- The sim logs `flush` (`cause`, `birds`, `alarm`, `x`, `z`) and
  `bird_alarm` (a robot sent to look by an alarm: `robot`, `x`, `z`,
  `dist`). `EXTRACTION_WILDLIFE=off` runs without animals, to compare;
  `=calm`, nothing scares them; `=restless`, flocks move on a second in
  (both for screenshots).

## Shots

`docs/shots/wildlife/`: `rooks_on_roof.png` (a row along a ridge, Marrow
Creek), `flock_up.png` (the same house a few seconds after they went up),
`deer_grazing.png`, `dogs.png` (Cinder Works), `heron.png` (Sallow
Reservoir's shallows), `bats.png` (dusk over Marrow Creek's creek,
looking downstream from -130, 8),
`ducks.png` (a raft on Sallow Reservoir's upper
arm from its west bank, 12 m off, `EXTRACTION_WILDLIFE=calm`; two
dabbling) and `ducks_close.png` (the same frame cropped, 5x).

## Pacing

Bot raids with animals and without (`EXTRACTION_WILDLIFE=off`), the same
seeds, four a valley:

| Valley | Animals | Extracted | Flushes | Alarms | Robots sent | Engagements | Mean raid (s) |
| - | - | - | - | - | - | - | - |
| Marrow Creek | with | 2 of 4 | 55 | 8 | 0 | 28 | 779 |
| | without | 3 of 4 | | | | 19 | 756 |
| Kettle Pass | with | 3 of 4 | 43 | 5 | 0 | 19 | 497 |
| | without | 2 of 4 | | | | 13 | 502 |
| Cinder Works | with | 1 of 4 | 33 | 2 | 0 | 26 | 462 |
| | without | 1 of 4 | | | | 13 | 449 |
| Sallow Reservoir | with | 1 of 4 | 39 | 8 | 0 | 21 | 442 |
| | without | 2 of 4 | | | | 31 | 525 |

Extraction 7 of 16 with, 8 of 16 without; raid times the same. Bots set
off a flock every minute or so and raise the alarm once or twice a raid,
but no robot was ever sent by birds: a robot has to be within 60 m of the
flock you put up, and a careful raider isn't. Engagements went up in
three valleys and down in one, so Cinder Works (26 against 13) was run
again at eight raids each way, logging the dogs' barks too:

| Cinder Works, 8 raids | Extracted | Engagements | Flushes | Alarms (birds, barks) | Robots sent |
| - | - | - | - | - | - |
| with animals | 4 of 8 | 49 | 87 | 8, 2 | 0 |
| without | 3 of 8 | 62 | | | |

The difference was noise. The animals don't change how raids go for a
careful bot; they change what a player can read, and what a careless one
pays for.
