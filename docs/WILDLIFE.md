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

## Deer and dogs

One larger animal a valley (`wildlife::herd`): two to four roe deer in
Marrow Creek and Kettle Pass, three or four stray dogs in Cinder Works.
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

## Fish

Trout in the creeks (`wildlife::fish`): the water near you is cut into
14 m cells, and about half the cells with water 0.45 m deep have a shoal
of two to five, always the same for the seed. They hold nose into the
current between the bed and the surface; someone on the bank within 6 m
(11 m wading or moving fast), a walker in the water, or a shot within
40 m and they dart off downstream (3.5 m/s) and hide, drifting back after
12 s. Fish there means nobody's been by. They never leave water 0.3 m
deep. Drawn within 30 m (`art/animals/fish.py`, 118 triangles).

## Moths

At dusk and at night (not in heavy rain), seven moths flit round each of
the nearest six lit lamps within 35 m of you: any point light, house
lamps, lanterns, the camp's (`wildlife::small_look`; visual only).

## The chorus

`wildlife::insects::Chorus` is how loud the small life round you is, 0
to 1, for sound to play: crickets at dusk and night, flies and small
birds by day. Machines hush it (silent within 25 m, untouched beyond
70 m); a shot or blast, a flock going up or a herd bolting within 120 m
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
- The sim logs `flush` (`cause`, `birds`, `alarm`, `x`, `z`) and
  `bird_alarm` (a robot sent to look by an alarm: `robot`, `x`, `z`,
  `dist`). `EXTRACTION_WILDLIFE=off` runs without animals, to compare.

## Pacing

Pending: a batch of sims with and without.
