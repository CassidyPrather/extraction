# Robot reading: a review procedure

`ITEM_READING.md`'s method, applied to the machines. In a fight a newcomer
has to know, from across a field: **what** it is, **which way it faces**,
and **whether it can still hurt them**. The eye and the tell matter most.

## 1. Render the line-up

In the game, with the game's own light and weather:

    scripts/shot.sh out.png 0,0,30,0,-1 60 --map training_yard --no-robots \
        --weather fog --hour 11 \
        --gallery robots:walker+drone+grappler+sentry+hound+heavy,YAW,DIST

- `--gallery robots:` (`src/dev.rs`) stands the machines side by side
  DIST ahead, each turned YAW from facing the camera, held where they
  stand (their minds would walk them; a heavy walked up to the camera).
  A walker-bodied machine faces by its `Walker::yaw`; a drone faces as it
  flies. `walker!broken` is a walker with its gun dead and a leg at 5%
  (`robots::reel`'s way of breaking one).
- **The eye's sharpness**: render at `--size 2234x1257` under an
  `xvfb-run -s "-screen 0 2240x1260x24"`: the flat camera's 75 degree
  vertical view then gives 14.3 px a degree, as the headset renders.
  `scripts/shot.sh`'s 1600 x 900 gives 10 px a degree, harsher than the
  headset. Crop; enlarge only by whole pixels.
- **Weather**: fog and drizzle, Marrow's reference weather
  (`ART_DIRECTION.md`). Drizzle's light comes and goes (a frame can render
  black or dim from one run to the next under the software renderer); fog
  is steady. Say which frames are a darker spell.
- **Distances**: 5, 15 and 30 m for a machine closing; 30 and 60 m for
  the family. **Facings**: towards you, side-on, away.
- The training yard's open track is the stage; its stations and the
  shimmer's sparkle stand behind the line (mind the sparkle: it can be
  read as a machine's).

## 2. Look, in this order

1. **60 m, grey**: what is each? Count the silhouettes you can name.
2. **30 m, colour**: which way does each face? Find the eye.
3. **Close (5 to 15 m)**: is it hurt, is it harmless? Find the tell.

## 3. What reads

- **Body shape first**: the families should differ in outline, not detail
  (a sphere on legs, a tall mast, a big four-legged frame, a low runner).
- **A face**: something only the front has, big enough for 30 m. A round
  body has no front unless it's given one.
- **The eye** is the face's centre, and should be the brightest thing on
  the machine when it's looking at you.
- **Harmless reads as broken, not as dark**: a dead part should change
  shape (a gun that droops, a leg that drags), not only go dark.
- **Marks and the eye tell the truth, not comfort**: a weak-point mark
  means "shoot here" and stays lit while the part's there (a 5% leg is
  still the best target); the eye reports awareness (a disarmed walker
  still sees you and calls the others). So "harmless" is carried by the
  dead part itself (Robot bodies' rule).

## 4. Reviews

### The machine family (1 November)

`docs/shots/robots/family_{fog,drizzle}_{colour,grey}_before.jpg`.

- **The walker and the grappler are twins** at 30 m and 60 m: white
  spheres on orange-kneed legs.
- **Their facing doesn't read**: a round body looks the same from the
  front and the back; the walker's eye is lost at 30 m in fog.
- **The heavy reads** (size, and its red eye lamps tell you it's facing
  you).
- **The sentry reads** as what it is (a tall mast), but not where it's
  looking.
- **The hound is nearly lost** at 60 m: small, low, grey on grey.
- **The drone is a speck** at both distances; its facing can't be told.

### The broken walker (1 November)

`docs/shots/robots/broken_walker_{fog,drizzle}_{colour,grey}_before.jpg`:
a healthy walker (left) beside the job-2 one (right, gun at 0 hp, a leg
at 5%), 5, 15 and 30 m.

- **Its gun goes dark but stays up**, where it was: a dark turret, not a
  broken one. Its orange ring goes out; at 15 m that's the only clear
  difference.
- **The smoke is faint**: a few flat grey puffs, lost against drizzle and
  fog past 15 m.
- **The failing leg looks whole** standing: its knee mark still lit, and
  the limp shows only in motion.
- **Its eye stays lit and faces you**, rightly (it still sees you): so
  the dead gun has to say "harmless" on its own, and today it doesn't.

### After the geometry (1 November)

`docs/shots/robots/*_after.jpg`, the same cameras (`--gallery robots:` now
holds a walker still the way `robots::reel` does: no mind, no path, its
yaw held; snapping its body back each frame had its drawn legs stride on
and its knee marks hang off them).

- **Walker: a dark brow** over the orb's upper front (`core`, clear of the
  eye: its orange ring keeps pale shell about it). Facing you: brow and
  red eye; away: a plain pale orb. Reads at 30 m; at 60 m only just. The
  crawler shares it, rightly (indoors it comes round a corner at you).
- **Grappler: a grapnel spar** standing behind the reel, three claws at
  its top (`harpoon.glb`). At 30 m a dark upright no walker has; at 60 m
  a hairline.
- **Hound: a tail mast** with a teal lamp (teal: the robots' own glow, off
  the "shoot here" orange). It lifts the hound off the grass at 30 m; at
  60 m it's still small. What's left is its size, which is gameplay
  (`HOUND_SIZE`), and its marks at range (Robot bodies).
- **The broken walker**: its gun's droop and the failing leg held up are
  Robot bodies'; the line-ups follow when they land. The grey discs over
  it are the smoke being replaced by a soft puff (`render::puff`, Weapon
  feel).

### The droop and leg lift (1 November)

`docs/shots/robots/broken_walker_{fog,drizzle}_{colour,grey}_droop.jpg`,
the same cameras on Robot bodies' droop and leg lift (f70e42fd), broken
at frame 4 and shot at 140; the fog sheets add a side-on pair.

- **The failing leg** reads side-on: its foot held clear of the ground.
- **The dead gun** points up and back, about 50 degrees, nose high: a
  dark slab standing on the orb (front-on, a black block where the
  turret was). It reads as changed, not as drooped. Sent to Robot
  bodies: the pitch looks sign-flipped.
- **The smoke** is still the grey discs (Weapon feel's puff to come).

### The slump (1 November)

`docs/shots/robots/broken_walker_{fog,drizzle}_{colour,grey}_slump.jpg`,
the same cameras on Robot bodies' fix (f539576b, on main 87afa914).

- **The dead gun slumps**: its nose down and swung aside, a dropped
  barrel beside the healthy walker's level turret, at 5 and 15 m in
  colour and grey. Side-on it hangs low on the orb. Harmless now reads
  from its shape, as the rule asks.
- **The failing leg** still holds its foot clear.
- **The smoke** is Weapon feel's soft puff now, not grey discs.

### The lamps through fog (1 November)

`docs/shots/robots/family_fog_{colour,grey}_lamps.jpg`, the family's
cameras on main 1123f737 (Robot bodies' eyes and marks: unfogged, and no
longer culled by the render budget's size fade).

- **60 m**: the walker's and grappler's orange knee marks, the walker's
  red eye when it faces you (none facing away), the heavy's eye lamps,
  the hound's marks low on the grass and the sentry's teal lamp all
  read, where the 60 m family was grey shapes before.
- **30 m**: the same, sharper; facing still reads from the walker's brow
  and eye.
