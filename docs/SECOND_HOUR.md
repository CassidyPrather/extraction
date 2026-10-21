# The second hour

The training yard teaches the first raid. The **first-time moments**
(`src/onboarding/firsts.rs`) teach the next twenty. The first time you
meet something the yard doesn't cover, Lev says one short line on the
radio with what to do. He says each line once per career, and the career
saves which ones he's said (`firsts` in `career.ron`). The lines are
spaced five seconds apart, because the wrist only shows the newest one.
Each line fits the wrist (3 lines of 26 characters) and uses no game words.

| Moment | When | He says |
| - | - | - |
| Jam (each kind) | a malfunction in your gun | its remedy: rack / slap the mag home, rack / mag out, rack, mag in |
| Heard | a machine turns to look for you | move off quiet, then go still |
| Lost you | it gives up | stay low a while |
| Drone lock | the chirp before it fires | get behind something |
| Anomaly | within 4 m of one's edge | throw something in first |
| Underground | below ground in a site | torch on, go slow |
| Surge | the sky turns (5 min warning) | come home |
| Lamed | a walker loses a leg | it can't follow; get out of its sight |
| Pack full | something won't go in | leave the worst of it |
| Worn gun | a gun in hand under half condition | it'll jam; the bench, when you're back |
| Bleeding | hit, and bleeding (faster than 0.1 HP a second) | dressing on it now, before you drop |
| Detector | the detector ticking fast near you | faster means nearer; sweep it slow |
| Revealed | a thrown thing shows one up | that's how you find them; bolts are cheap |
| Fold | through a fold, somewhere else | stop; look before you step |
| Well | in a well's pull | back out the way you came, now |
| First death | waking in the truck | what came back (the crate, counted) and what didn't (what you carried, counted, where you fell); beat their convoy there |
| Convoy took it / kit recovered | the first time each way | gone through the Gate / that's how you get it back |
| Note pinned | first note read (Story's board) | he keeps copies, board in the back |

Other systems can add their own moment with `MeetFirst(First::...)`.

## The bot's career: `--scenario second_hour`

A bot plays three raids in Marrow Creek, plus the start of a fourth, in
about four minutes of wall clock (headless). The scenario sets each moment
up the way the drills set things up:
- a rifle worn to 0.2;
- a walker about its own business, about 85 m off, that hears a shot
  fired crouched (nearer, by day, it sees who fired, even crouched);
- a drone, met by a house in the village (where most of the Zone's
  cover is);
- a torch from the crate, its cell in, for the village bunker's stair;
- a walker at work in the open;
- the Surge's clock wound on;
- health run out.

Then the bot does what Lev's line says, and the run checks that it
worked. The run passes when every moment was met and all but one answer
worked.

The scenario isn't in `--scenario all`; run it by name. It also shortens
the truck's drive to 10 seconds and clears its own robots after their
moments.

Last run (260 s, the merged tree with the bracing fix):

| Moment | Raid | Bot did | Worked |
| - | - | - | - |
| Revealed | 1 | (a thrown thing showed one up) | yes |
| Anomaly | 1 | go round it (a burner) | yes |
| Worn | 1 | carry on; mend it at the bench later | yes |
| Jam (failure to eject) | 1 | rack it | yes |
| Pack full | 1 | leave the worst of it | yes |
| Surge | 1 | call the truck, go home | yes |
| Heard | 2 | move off low, then keep still | yes |
| Jam (failure to feed) | 2 | rack it | yes |
| Drone lock | 2 | get behind something (indoors, beside the door) | yes |
| Bleeding | 2 | dressing on it (and every wound after) | yes |
| Lost you | 2 | stay low a while | yes |
| Underground | 2 | torch on, go slow (down the stair, 25 s at the foot) | yes |
| Lamed | 2 | walk away from it, out of its sight (behind the kiosk) | yes |
| First death | 3 | go back for it | yes |
| Kit recovered | 4 | went back for it before the convoy | yes |

### Per mode: seated, one-handed, left-handed

The same career is replayed with each access setting (`--seated`,
`--one-handed`, `--left-handed`). The bot plays the way that setting
makes the player play:
- **Seated:** its real eye is at 1.2 m. The rig lifts it to a standing
  eye, and it reaches no lower than a seated hand. Anything lower takes
  the crouch button.
- **One-handed:** everything is done with the gun hand. It reloads from a
  pouch, racks with the second button, and switches the fire mode with a
  stick click. It holsters the gun to free the hand, and draws it back.
- **Left-handed:** mirrored. The holster and mount are on the other hip,
  the watch is on the right wrist, and the sticks are swapped.

RESULTS_TABLE

Where a mode failed, the game was usually wrong for that mode, and the fix
went into the game for that kind of player:
- **Seated: the crouch didn't reach the ground.** Crouching seated took
  the eye to 0.9 m, so a hand beside the chair reached the floor only by
  leaning, and missed kit that had settled on a slope below the feet. The
  crouch now goes to 0.7 m. A hand held at knee height reaches the ground
  (`seated_lift_and_crouch_to_the_floor`).
- **Left-handed: the aim was right-hand only.** The aim hint read the
  right hand's gun, so a left-handed player's sights were wrong. It now
  reads whichever hand holds the grip.
- **One-handed: a magazine had to go in by hand, and there is no other
  hand.** An empty gun held to a pouched magazine now takes it straight
  into the well. Pouches are filled first with magazines, and pockets
  with everything else, so a magazine is where the well can reach it. The
  second button racks the gun (there is no hand for the slide), and a gun
  rested on a surface counts as held in two hands.
- **One-handed: the bot's own bookkeeping.** These were the bot's faults,
  not the game's:
  - a gun picked up for the pack became its "last gun";
  - a button press timed from the clock of the idle wait it replaced
    never came, so the fire selector stuck on auto;
  - a torch put away to open a door was never taken back out.
  All are fixed.
- **One-handed, lamed walker: the kiosk has to stay between you.** A
  one-handed shot at a leg takes longer, so the walker had seen the bot.
  It limped round the kiosk to look, and found it. Hiding now moves round
  the kiosk while the walker looks, keeping the kiosk between.

### What it found

- **A held trigger keeps a rifle firing.** When the driver's line cut in
  on a shot, the bot walked off with the trigger still held, and the
  rifle kept firing: it gave away every move after being heard or
  laming a walker. That was the run's own bug: the trigger is now let go
  whenever the run cuts in.

- **"Get low and keep still" fails after a gunshot.** A walker that heard
  your shot comes to where the shot was. A bot that kept still there was
  found and killed. Moving off 20 m low, *then* keeping still, lost it.
  Lev's line now says to move off quiet, then go still.
- **Fighting a drone that has already locked on loses.** That held even
  with a sound rifle, both hands on it, and the drone 40 m off.
- **Cover beats it, if it's the right cover.** In through the door and
  one step to the side, out of the doorway's line: the drone hovered at
  the door, searched round the house for about 40 s, gave up and left,
  and the bot walked out unhurt ("Lost you" came too). Two kinds of
  cover failed first:
  - a shed: the drone hovers at the door and looks straight in;
  - deep in the house, across the far room: the drone searches indoors,
    came in and found it.
  Lev's "get behind something" is right; "inside, beside the door" would
  be more exact, but it doesn't fit his line.
- **Nothing told you that you were bleeding.** The drone's first burst
  lands before you're in cover, and the wound bleeds until dressed. The
  only sign was "BLEED" on the wrist's status line, and the bot (like a
  player who missed it) bled out later, waiting for the truck. Now Lev
  says it once ("You're bleeding. Dressing on it now, before you drop."),
  the moment the yard's first-aid station was for.
- **A lamed walker can't chase you, but it can still shoot.** Walking off
  in the open, as the old line ("it can't chase you now") suggested, got
  the bot shot. Walking off out of its sight (behind the bunker's kiosk)
  worked; it limped a few metres to look, and no further. Lev now says
  so: "You've lamed it. It can't follow. Get out of its sight."
- **A torch and a slow walk down the bunker stair was fine** for 25 s at
  the foot. (The test `underground_a_crawler_sees_you_by_your_torch` says what happens
  further in.)
- **Waking in the truck after going down restarted the raid.** It said
  "Out of the truck..." while you were in the bed, and counted a raid.
  `leave_base` read stale global poses. Fixed, with a test.
- **The bot couldn't reach the cord at the ranger tower.** It caught the
  bench's waist-high corner on a straight line from the back of the bed.
  A person isn't blocked: walking at the cord from where you start in the
  bed brings the body to 0.21 m of its toggle, well inside an arm
  (`the_cord_is_in_reach_from_the_bed`). The bot's route was the
  problem, so it now goes round the middle of the bed.
