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
| Heard | a machine turns to look for you | get low, keep still |
| Lost you | it gives up | stay low a while |
| Drone lock | the chirp before it fires | get behind something |
| Anomaly | within 4 m of one's edge | throw something in first |
| Underground | below ground in a site | torch on, go slow |
| Surge | the sky turns (5 min warning) | come home |
| Lamed | a walker loses a leg | it can't chase you now |
| Pack full | something won't go in | leave the worst of it |
| Worn gun | a gun in hand under half condition | it'll jam; the bench, when you're back |
| First death | waking in the truck | what came back (the crate, counted) and what didn't (what you carried, counted, where you fell); beat their convoy there |
| Convoy took it / kit recovered | the first time each way | gone through the Gate / that's how you get it back |
| Note pinned | first note read (Story's board) | he keeps copies, board in the back |

Other systems can add their own moment with `MeetFirst(First::...)`.

## The bot's career: `--scenario second_hour`

A bot plays three raids in Marrow Creek, plus the start of a fourth, in
about two minutes of wall clock (headless). The scenario sets each moment
up the way the drills set things up:
- a rifle worn to 0.2;
- a walker about its own business that hears a shot;
- a drone;
- the Surge's clock wound on;
- health run out.

Then the bot does what Lev's line says, and the run checks that it
worked. The run passes when every moment was met and all but one answer
worked.

The scenario isn't in `--scenario all`; run it by name. It also shortens
the truck's drive to 10 seconds and clears its own robots after their
moments.

Last run (merged main, 113 s):

| Moment | Raid | Bot did | Worked |
| - | - | - | - |
| Anomaly | 1 | go round it (a burner) | yes |
| Worn | 1 | carry on; mend it at the bench later | yes |
| Jam (failure to eject) | 1 | rack it | yes |
| Pack full | 1 | leave the worst of it | yes |
| Surge | 1 | call the truck, go home | yes |
| Heard | 2 | move off low, then keep still | yes |
| Jam (failure to feed) | 2 | rack it | yes |
| Drone lock | 2 | fight it (two hands, a sound rifle) | **no**: it died |
| First death | 2 | go back for it | yes |
| Kit recovered | 3 | went back for it before the convoy | yes |

### What it found

- **"Get low and keep still" fails after a gunshot.** A walker that heard
  your shot comes to where the shot was. A bot that kept still there was
  found and killed. Moving off 20 m low, *then* keeping still, lost it.
  The line should say to move: a wording question for Story.
- **Fighting a drone that has already locked on loses.** That held even
  with a sound rifle, both hands on it, and the drone 40 m off. Lev's
  "get behind something" is the right advice. The run doesn't yet try
  cover, so that row tests the wrong answer: next.
- **Waking in the truck after going down restarted the raid.** It said
  "Out of the truck..." while you were in the bed, and counted a raid.
  `leave_base` read stale global poses. Fixed, with a test.
- **The bot couldn't reach the cord at the ranger tower.** The bench's
  corner at waist height blocks a straight line from the bed's centre to
  the cord. A person can lean, but the gap between the crate and the
  bench is narrower than the body. Worth checking in a headset.
