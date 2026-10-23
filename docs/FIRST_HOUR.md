# The first hour

Five newcomers play a player's first hour. Each knows only what the game
shows them, and each writes down where it was confused, stuck, hurt
without warning, or killed. Where one of them came to grief, the fix went
into the game: its signposting, the driver's lines, the yard's stations,
and how safe things are. The bot was not taught round the problem.

```
extraction --headless --scenario first_hour             # all five
extraction --headless --scenario first_hour_cautious    # one
```

Each newcomer plays two sessions, as a new player's first hour runs.
1. **`first_yard`:** the training yard, played by the yard's own bot with
   the newcomer's habits.
2. **`first_raid`:** after the cord, the game opens again in Marrow Creek
   at the camp (the title screen behind the truck). The bot keeps the
   career the yard left behind.

Both sessions keep their files in `target/first_hour/<name>/`: the career,
and the yard's recording in `raids/`. The scenarios are opt-in, not part
of `all`: together they take about an hour of wall time.

A new player never sees the camp before the yard. With no career,
`persist::choose_start` opens the yard. The camp first appears when the
yard's cord brings them to the Zone, with "Your kit is in the crate in the
truck. Walk out through the gate to start."

## The newcomers

| Newcomer | Plays |
| - | - |
| cautious | walks, stops at each board to read it, creeps once out in the Zone |
| reckless | runs everywhere, walks into the shimmer for the stone, takes fights |
| lost | goes to the wrong post first at each station; heads off the wrong way on the raid |
| hoarder | picks up everything lying about, takes the whole crate |
| one-handed | plays with HANDS set to one |

## What it acts on

The raid director (`onboarding::first_hour::direct_raid`) acts on what the
game shows:
- the camp board;
- the crate in the truck;
- the job card, read for a place name; the map sheet has the names
  written where the places are;
- the driver's lines (`Raid::log`), including the first-time moments;
- the truck when it's in sight.

It doesn't know the rendezvous names: the map's rings only say RV. Where
it has to guess, it says so, and the guess is written down as Confused.

Kinds of moment:
- **Learned:** something shown or said that it used.
- **Confused:** it had to guess.
- **Stuck:** a step it couldn't do.
- **Hurt:** with a warning in the last half minute.
- **HurtUnwarned:** hurt with nothing said first.
- **Killed:** it went down.

A newcomer passes when it gets home with nothing stuck, unwarned or
killed.

## Found, and fixed in the game

- **The yard's stone burned the cautious newcomer to death.** A stone
  pulled from a burner stays hot for 20 s and burns the hand that holds
  it, about 10 HP a second as it starts to cool. The yard's bot stows it
  at once. The cautious newcomer held it while walking to read the next
  board, and was dead in 12 s with not a word said. (Its recording called
  the damage "fall".) Now a hand lets go of something that hot after
  1.5 s, and the game says "Too hot to hold: you drop it. Let it cool, or
  pocket it." (`anomaly::harvest`, test `a_hot_stone_is_let_go_of`.)
- **The yard's radio said machines were coming.** A shot on the range set
  the robot director's response going, and the radio said "Machine talk
  on the radio, loud and close: coming here." Nothing hunts you in the
  yard, and nothing came. The yard has no machine talk now
  (`raid::chatter`).
- **The job card named a place, but not where it is.** "Go up to the Old
  farm and look round." The Old farm is 190 m north-east of where the
  truck drops you. It has nothing tall enough to see from the gate, and
  the yard never shows the map. The card now adds "From the drop:
  north-east, 190 m. Ringed on your map." (`raid::requests`.)
- **The truck named a rendezvous the map doesn't.** "Heading for the
  Ranger tower", but the field map labels every rendezvous just "RV".
  The truck now says which way and how far from you: "Copy. The ranger
  tower, north-west, 480 m. Two minutes." It does so again if you call
  while it waits. (`raid::lines::way`.)

- **The truck came down on top of a newcomer at the rendezvous.** Told
  "the rail yard", the reckless newcomer went and stood on the spot. The
  truck arrives by appearing where it parks, and it appeared around them,
  inside the bed's walls: they couldn't move, board or ring for five
  minutes. Now it pulls up past them, short of them, or beside them
  (`raid::truck::rv_pose_clear`, test
  `the_truck_pulls_up_clear_of_whoever_waits_on_the_spot`).

- **A hand kept hold of a stone it had let go of.** The hoarder picked
  up one thing after another. The hot stone it had taken first stayed
  marked held by a hand that had moved on to something else. Dropping it
  did nothing, and it burned the hoarder to death. Now anything held by a
  hand that has moved on drops (`interact::drop_orphaned_holds`). A stone
  just dropped too hot can't be taken again for 3 s
  (`anomaly::harvest::Scalded`).

## The bot's own faults, fixed on the way

These made a newcomer look stuck when a person wouldn't have been:
- walking out of the truck bed through its side rather than over the
  tailgate;
- forgetting a line that had gone from the log before it looked ("noted",
  the truck's reply);
- a step to the side round an obstacle that itself got stuck, and counted
  as a failure;
- the cautious newcomer never running, even when the yard's board said to;
- walking into the quarantine fence instead of out through the gate the
  camp board names;
- walking onto the rendezvous spot after the truck had parked on it;
- a hoarder taking the crate in any order, so its radio went to the
  bottom of the pack, out of reach;
- stepping aside when lining up close to something indoors, into a
  corner;
- going into the practice house at a slant from wherever it had been (the
  lost one, back from the wrong post), and sticking on the jamb: it now
  goes round to the door and in square;
- the hoarder pocketing the dressing it was carrying to the first-aid
  post, by grabbing with the hand that held it: it now takes things with
  a free hand and leaves alone what the station is about to use.

## Results

### Last runs

All five from this branch's head, yard and first raid both passing. The first job's run home is now free and short: back to the gate the newcomer went out by (the card says "Call me from the gate after"), not the long walk to the rail yard's rendezvous, where in the first fights a survey drone met every newcomer (see the first fight, below).

| Newcomer | Yard | First raid |
| - | - | - |
| cautious | all 23 steps, 329 s, reading every board | home from the gate, 367 s |
| reckless | all 23 steps, 108 s; walked into the shimmer for the stone | home from the gate, 136 s |
| lost | all 23 steps, 674 s, the long way round each station | home from the gate, 246 s (set off the wrong way first) |
| hoarder | all 23 steps, 163 s, pockets full | home from the gate, 218 s |
| one-handed | all 23 steps, 161 s | home from the gate, 217 s |

None was hurt, stuck or confused on the raid, bar the lost one's wrong turn. None met a machine. Out through the gate, the road to the farm passes no anomaly, so the detector's and the thrown bolt's first words (which the old way out heard about 30 s in) now wait for the second job. The yard has already taught both.

#### cautious

| t | What | Moment |
| - | - | - |
| 0 s | Learned | camp: "Your kit is in the crate in the truck" |
| 13 s | Learned | the job card: ODD JOBS - JOB 1 OF 3 "Go up to the old farm and look round. Tell me what's changed." From the drop: north-east, 190 m. Ringed on your map. Call me from the gate after. |
| 18 s | Learned | on to Old farm |
| 194 s | Learned | the place, noted: "Now get it home" |
| 194 s | Learned | back to the gate, as the card said |
| 352 s | Learned | the truck came back to the gate |
| 367 s | Learned | in the truck and away |

#### reckless

| t | What | Moment |
| - | - | - |
| 0 s | Learned | camp: "Your kit is in the crate in the truck" |
| 7 s | Learned | the job card: ODD JOBS - JOB 1 OF 3 "Go up to the old farm and look round. Tell me what's changed." From the drop: north-east, 190 m. Ringed on your map. Call me from the gate after. |
| 7 s | Learned | on to Old farm |
| 67 s | Learned | the place, noted: "Now get it home" |
| 67 s | Learned | back to the gate, as the card said |
| 126 s | Learned | the truck came back to the gate |
| 136 s | Learned | in the truck and away |

#### lost

| t | What | Moment |
| - | - | - |
| 0 s | Learned | camp: "Your kit is in the crate in the truck" |
| 9 s | Learned | the job card: ODD JOBS - JOB 1 OF 3 "Go up to the old farm and look round. Tell me what's changed." From the drop: north-east, 190 m. Ringed on your map. Call me from the gate after. |
| 9 s | Confused | set off the wrong way |
| 9 s | Learned | on to Old farm |
| 140 s | Learned | the place, noted: "Now get it home" |
| 140 s | Learned | back to the gate, as the card said |
| 231 s | Learned | the truck came back to the gate |
| 246 s | Learned | in the truck and away |

#### hoarder

| t | What | Moment |
| - | - | - |
| 0 s | Learned | camp: "Your kit is in the crate in the truck" |
| 10 s | Learned | the job card: ODD JOBS - JOB 1 OF 3 "Go up to the old farm and look round. Tell me what's changed." From the drop: north-east, 190 m. Ringed on your map. Call me from the gate after. |
| 10 s | Learned | on to Old farm |
| 110 s | Learned | the place, noted: "Now get it home" |
| 110 s | Learned | back to the gate, as the card said |
| 203 s | Learned | the truck came back to the gate |
| 218 s | Learned | in the truck and away |

#### one-handed

| t | What | Moment |
| - | - | - |
| 0 s | Learned | camp: "Your kit is in the crate in the truck" |
| 9 s | Learned | the job card: ODD JOBS - JOB 1 OF 3 "Go up to the old farm and look round. Tell me what's changed." From the drop: north-east, 190 m. Ringed on your map. Call me from the gate after. |
| 9 s | Learned | on to Old farm |
| 110 s | Learned | the place, noted: "Now get it home" |
| 110 s | Learned | back to the gate, as the card said |
| 202 s | Learned | the truck came back to the gate |
| 217 s | Learned | in the truck and away |


### First run (before the fixes above)

| Newcomer | Yard | First raid |
| - | - | - |
| cautious | all 23 steps; but the first time, burned to death by the stone in silence | stuck leaving the truck bed; the truck's reply misread |
| reckless | all 23 steps (walked into the shimmer for the stone) | to the farm and noted; at the rail yard the truck came down on top of it |
| lost | all 23 steps, with walks that stalled coming back from the wrong posts | set off the wrong way, into the camp's edge, and stayed stuck |
| hoarder | all 23 steps; a step failed with its hands full of what it had picked up | home, with two stalled walks |
| one-handed | all 23 steps | home, with five stalled walks (the truck bed, the spot) |

# The first fight

```
extraction --headless --scenario first_fight          # every machine
extraction --headless --scenario first_fight_walker   # one
```

The scenario puts one machine on each newcomer's first route to the Old
farm, a little short of halfway, and watches its tells and the first hit.
A newcomer met no machine at all on that route in the first-hour runs, so
the machine is placed by hand; a natural one would come on the second or
third job. Each run starts from a fresh career that has done the yard.

| Machine | Placed | Tells watched |
| - | - | - |
| walker | on Watch, 25 m off the route | Noticed (eye brightens), WalkerAim (clank, servo; 0.7 s to the shot) |
| drone | on Watch, 30 m off, 22 m up | Noticed, DroneLock (the chirp; 0.9 s to the burst) |
| hush | across the route, 7 m | InHush (the sound goes dull; tone at 4 s, harm from 8 s) |
| sentry | 22 m off, sweeping 140° across the route | SentryEvent::Chirp (lens flare, and from Audio's round a chirp sound; 1 s to the shot) |
| night | a walker as above, at `--hour night` | as the walker |

(A real night visit comes to a safehouse you sleep in, which a first raid
doesn't have, so "night" is the walker met in the dark.)

At its first tell each newcomer does what it would:
- **reckless:** draws and fights;
- **lost:** runs, anywhere;
- **hoarder:** carries on;
- **cautious** and **one-handed:** get low, out of its line, and keep
  still.

A hit is **warned** when one of the machine's tells came in the 15 s
before it.

## First runs: the walker and the sentry

| Machine | Newcomer | Tell to first hit | Outcome |
| - | - | - | - |
| walker | cautious | 0.8 s | down |
| walker | reckless | 3.0 s | down |
| walker | lost | 4.1 s | down |
| walker | hoarder | 4.1 s | down |
| walker | one-handed | 20.8 s | down |
| sentry | cautious | 28 s (it hid from the first chirp) | down later |
| sentry | reckless | 1.3 s | down |
| sentry | lost | 7.1 s, from Noticed | down |
| sentry | hoarder | 4.4 s | down |
| sentry | one-handed | 1.3 s | down |

Every newcomer died to its first machine.

**The walker can fire 0.7 s after it first sees you.** These come in the
same frame:
- `Noticed`, its eye brightening;
- `WalkerAim`, its gun settling;
- the driver's Heard line.

The shot follows 0.7 s later, which is under a newcomer's reaction time.
The cautious newcomer moved on the tell and was hit 0.8 s later, in the
open with nowhere to break line of sight. With Robot minds: a beat between
first noticing someone and the first aim.

**Death loops at the truck.** A newcomer who goes down wakes in the back
of the truck at base. The machines the fight drew in kept shooting there:
a drone answering the walker's call was at the base and hit them within a
second of waking, five times over. With Robot minds: waking up should
drop every machine's hold on you, or the parked truck at base should be
out of bounds.

**The sentry's chirp made no sound.** Its only tell was a red flare on
the lens, easily missed by someone new looking elsewhere. Audio has given
it a chirp that carries about 60 m. With Audio, too: the Hush's rising
tone came at 8 s, the moment its harm starts. It now comes at 4 s.

(In these first runs the bot kept walking its old route after going down,
into the fight again. It stops now.)

## First runs: the drone, the hush and the night

| Machine | Newcomer | Tell to first hit | Outcome |
| - | - | - | - |
| drone | cautious | 4.2 s, from Noticed | down 28 s later |
| drone | reckless | 5.1 s | down 15 s later |
| drone | lost | 5.2 s | down 13 s later |
| drone | hoarder | 5.1 s | down 16 s later |
| drone | one-handed | 5.3 s | down 16 s later |
| hush | all five | not hurt by it: each walked through inside its 8 s grace | |
| night | cautious | never noticed: a walker in the dark at 25 m didn't see a crouched newcomer | |

**The drone is told in time, but there's nowhere to go.** Every newcomer
had four or five seconds between the drone noticing them and its first
burst. Lev's "Get behind something" came with the lock. The Old farm road
is open field, though: nothing to get behind within a sprint, and the
drone fights from 20 m out and up high. The tell is fine; the place is
the problem. A first drone should meet you where there's cover, or a
newcomer should be told before setting out that the open is where drones
find you. (Robot minds' 1.5 s look before the lock adds to the warning,
not to the cover.)

**The hush didn't hurt anyone.** Placed across the road, each newcomer
walked through it inside its 8 s grace without knowing it was there. The
tone now rises at 4 s (Audio), and Lev's new line says "Out of there,
now", so someone who stops inside is told.

**At night, the walker never saw the cautious newcomer.** The machines'
sight falls to 0.4 at night, and a crouched newcomer 25 m off wasn't
seen. That's the design (night is for sneaking); the reckless, who runs
upright, is the one to watch in the rerun.

(These runs' later deaths, long after each hush, and most of the night
runs' stalls, were the bot's. It called the truck on whichever radio it
found first, often one lying elsewhere in the valley, and walked off after
it. It uses its own now.)

Fixed since these runs, to be measured in the rerun:
- **Robot minds:** a walker or drone spends 1.5 s looking at you before
  its aim or lock begins, so the first round is no sooner than 2.2 s
  (walker) or 2.4 s (drone). Going down makes every machine let you go.
- **Audio:** the sentry chirps, and the hush's tone rises at 4 s.
- **Here:** Lev warns of a walker's aim, a sentry's chirp and a hush, and
  a line about danger now is said 1.5 s after the last, not 5 s.

## Second runs: with the fixes

With Robot minds' look-before-shooting and let-go, Audio's sentry chirp and
earlier hush tone, and the Driver's new lines:

| Machine | Tell to first hit (all five) | Outcome |
| - | - | - |
| walker | 2.2 s at least (was 0.8 s), the Driver's "Break its line" with the aim | every newcomer down, 15–40 s later |
| drone | 3–13 s, "Run across it, to cover" with the lock | every newcomer down, 10–35 s later |
| sentry | 3.4 s at least, now with its chirp | every newcomer down |
| hush | no harm: the tone (4 s) and the line come before its 8 s | |
| night | the walker didn't see any of them | all five home |

**No newcomer was hit without a tell first.** Every first hit came at
least 2.2 s after a tell the newcomer could see or hear, and the death
loop at the truck is gone.

**Warned isn't enough on open ground.** The newcomers who do as they're
told ran for the far side of the nearest building, or hard across the
machine's line where there was none. They still went down to a walker or
a drone on the Old farm road: it's open fields, with nothing within a
sprint. That's the first machine's placement, not its tell.

**The first job sent newcomers to a drone.** In the plain first hour, with
no machine put there, the robot director's survey drones pick a named
place to fly to. One picked the Old farm, the first job's place. On its
first raid the cautious newcomer arrived as the drone did, and died.
Survey drones now leave a career's first job place alone, as they already
leave the rendezvous points (`robots::director::survey_places`, test
`a_first_job_place_is_left_alone`). The first raid is meant to teach the
way there and back; its first fight comes later, on the second or third
job.

**Then a drone met them on the way home.** With the farm left alone,
the next one came on the long walk from the farm to the rail yard's
rendezvous, 480 m of open ground, while the newcomer was carrying the
first find home. The first job's run home is now the truck's: call from
the gate you went out by (the job card says so, and the truck's "First
run's on me. Get in the back." says why it's free), and it comes back
there. Later jobs pay for the short run with a can of fuel as before
(`raid::radio_calls`, tests `the_first_jobs_short_run_is_on_the_house`
and `the_short_exfil_burns_a_can_of_fuel`). In the plain first hour, all
five newcomers now go to the farm, back to the gate and home without
meeting a machine.

## Jobs 2 and 3: the first natural fight

With the first job safe, the question is whether the first real fight
still comes, and fairly, by job 2. It does: every newcomer met a machine
on job 2, a drone over the Village or a walker on the way. Every one was
told before the first hit: "Something heard you", then the lock and "Run
across it, to cover", 4 to 25 s ahead.

Found, and fixed in the game:
- **Nothing said how a job is paid.** The newcomers rode home and the
  job sat unpaid into the next session, so job 2 never came. Home from
  the Zone with a job on the card, the driver now says "Home. Now press
  the card: that's the job paid." (once a career, `firsts::payday`).
- **A drone sees down through trees and round walls.** Robot sight is a
  ray stopped only by solid things, and trees only have trunks. Against
  a drone the only cover is a roof. The drone line goes on: "Under a
  roof's best. It sees down through trees."
- **A machine that lost you comes to where it saw you.** Behind a log
  pile, the cautious newcomer was lost to the walker, sat still, and the
  walker walked 25 s to that pile. Moving away was worse: a moving shape
  over low cover is glimpsed, and each glimpse moves the machine's guess
  (Robot minds). New, once a career, when a machine that was shooting at
  you goes to look: "Low behind it, and keep still. It only finds what
  moves." (`First::ComingToLook`).
- **The driver's cache line read "out past the The Hollow".**

With Robot minds and Map design, on their branches:
- Map design: log piles every ~25 m from the drop to the Old farm, and a
  roofed shed on each first walk (drop to the Old farm, drop to the
  Village).
- Robot minds: a walker's aim spreads on a target sprinting across its
  line; out of its sight it settles its aim again (with the tell) before
  it fires; a drone on its way home fights 8 s at most, and not at all
  once you're under a roof; and trees hide you from afar.

The bot's own faults, fixed on the way: it read the job's place off the
cache line under it (and went to The Hollow, sentry country, not the
Village); it sat in a shed's doorway, in a look through it; and it walked
out of the shed through the wall.

### Job 2, natural, all three branches together

| Newcomer | What came | Warned | What it did | Outcome |
| - | - | - | - | - |
| cautious | nothing near the Village | | | home |
| reckless | a machine twice | yes | took it on | home |
| lost | a drone over the Village | "heard you" 50 s, lock 6 s before the hit | off on its own | down |
| hoarder | the same drone | 1 s, then the lock 9 s before the hit | carried on (no dressing on it) | down |
| one-handed | the same drone | "heard you", then the lock | in at the shop door, off to the side | home, not hit |

Before the roofs and the drone's 8 s burst, the one-handed newcomer
went down to this drone in 57 s.

### Seeded fights, all three branches together (the machine put on the road)

| Newcomer | Walker: tell → hit | dead after | Drone: tell → hit | dead after |
| - | - | - | - | - |
| cautious | 2.2 s | 23 s | 62.7 s (in the shed) | 72 s |
| reckless | 11.3 s | 46 s | 11.3 s | 40 s |
| lost | 4.4 s | 32 s | 13.6 s | 158 s |
| hoarder | 6.6 s | 11 s | 3.9 s | 23 s |
| one-handed | 34.0 s | 7 s | 7.8 s | 97 s |

Before this round a walker killed a newcomer 8 s after the first hit; a
drone, 10 to 35 s. A machine put right on the road a newcomer walks still
wins in the end. That is the seeded test, harder than the game's own
placement, which on job 2 sent three of five home.

Still open: the bot can't do a fetch job (find the watch, put it in the
crate), so the record stops at job 2. And the lost and the hoarder do as
their habits say, not as they're told; a person like them would die the
same way.

# Every job, start to finish

The record shouldn't stop at a job the newcomer can't do. Every kind of job
on the board has a way for the newcomer bot (`first_hour::errand`, an
exhaustive match, so a new kind won't build without one):

| Kind | The errand | Live |
| - | - | - |
| Scout | go and look; the driver says when it's seen | all five, job 1 |
| Bring (a contract's thing) | find it (indoors: the rooms' plan, doors opened), pocket it, home, into the stash crate by hand, press the card | four of five, job 2 (the post office watch) |
| Fetch | the same, for n of a thing anywhere | four of five, job 3 (3 matches) |
| Plant | take it out (off the hatch's ledge, or the crate), leave it there, home, press | the cautious one, job 4 (the mast relay) |
| Recover | as Bring | seeded (`--job recover`): found in the plank house's back room, crated, paid |
| Letter | as Bring | seeded: found, crated, paid |
| Ledger | as Bring | seeded: down under the Gate; a drone and a machine below, warned each time; down |
| Photograph | camera off the ledge, out to where the card says one's working, the head kept on it, press from as far off as the camera takes, back off | the picture taken (Cinder Works, with Robot minds' surface walker); on the way home a walker heard it, aimed, warned, and shot it |

`every_job_kind_has_a_way` fails when a kind has no way, or when its card
doesn't name what the newcomer goes for: what the bot is after, a player
reads. `first_jobs` (or `first_jobs_<name>`) runs the yard and five jobs on
one career; `--job KIND` seeds a raid's job, for the kinds the ladder seldom
deals.

## Found, and fixed in the game

- **No radio, no way home, ever.** A newcomer who fell with the radio on
  them lost it with the rest; on every raid after there was no calling
  the truck. Home with no radio anywhere, the driver now passes his spare
  through the hatch: "No radio on you. Take my spare, off the ledge."
  (`firsts::spare_radio`, test `no_radio_home_gets_the_spare`).
- **The same for a job's kit.** A contract's relay box (or the camera)
  lost at a fall left the job undoable: "Lost the relay box? Here's
  another, off the ledge."

## The bot's own faults, fixed on the way

Out of a building by its door; round buildings in the way, not into their
walls; through the rooms by the building's plan, and by the door when the
plan threads a gap the body can't; crouched for the crate; past a find it
can't get at; "carried" meaning on it, not in the pack it left where it
fell; the radio out of its pack; every machine tell acted on, not only the
first one explained; what the driver passes through after the pay taken
off the ledge; the job's own thing taken out of the crate.

## Five jobs, by newcomer

| Newcomer | Job 1 (scout) | Job 2 (bring) | Job 3 (fetch) | Job 4 (plant) | Job 5 |
| - | - | - | - | - | - |
| cautious | paid | paid | paid | paid | down (a drone) |
| hoarder | paid | the watch found; down before home | down | down | down |
| lost | paid | the watch found; down before home | down | down | down |
| one-handed | paid | paid | paid | down: a drone by the pumping station | down |
| reckless | paid | paid | paid | the relay left; down on the way home | down |

Every hit after job 1 came after a tell: the drone's lock, a walker's aim.
The lost and the hoarder do as their habits say (off on its own, carry
on), and die as a person like them would. The rest are the drones. On
job 4, the mast relay, three of four were caught by one between the drop
and the mast (at the Village, the pumping station, the mast's own road),
each 7 to 17 s after its lock; only the cautious one, creeping, got there
and back.

Then the mast relay's walk, for a first contract: a newcomer warned by
the chirp had a roof only where the Village's houses are; past them,
nothing for 250 m. Map design put three roofed sheds on the ways to the
mast (the pumping station's tool shed, the rangers' store, the
road-menders' hut: none of the walk more than 97 m from a roof), and the
first plant job's card now ends "Sheds on the way: in, if anything
chirps." On that map, the one-handed newcomer, locked on by a drone on the
way, went in, came on, left the box and got home paid; the reckless one
left it with a machine already shooting at it, as it does, and fell
there.

The photograph: at a raid's start every walker on Cinder Works was a
crawler at work underground, and the first up on the surface came 17
minutes in, at the newcomer, gun up. Now, with a photograph on the card,
Robot minds keep one of the kind it names (a plain walker; not a crawler,
a hound or the heavy) at work on the surface 120 to 360 m from the drop
(`robots::subjects`), and the card says where it's been heard ("He's heard
a walker working by the chimney. From the drop: ..."). The newcomer took
its picture 3.5 minutes in, from as far off as the camera takes; a walker
caught it on the way home, warned.

# Machines working together (Robot minds' squad drill)

Robot minds' `squad_drill` (claude/polish-robot-minds 64802ac2,
`src/robots/squad_drill.rs`, 24 seeds an arm): on Marrow's south field a
newcomer with a radio jogs for the nearest rendezvous, cover to cover,
crouched in cover; a squad (three walkers, or two and a drone) finds it at
45 to 55 m. The listener acts only on what's said: a chirp within 40 m, a
radio line 1.5 s after it shows. A cut-off sends it to another rendezvous;
a flanker's post counts as a threat when it picks cover.

| Newcomer | Machines each alone | Machines working together |
| - | - | - |
| deaf | 17% get away | 8% |
| listening | 17% | 17% (alive a median 23 s, the deaf 15 s) |

Three walkers alone, working together: the listener 27%, the deaf 13%.
Working together halves a deaf newcomer's chances; listening wins them
back. The numbers are small, and a drone in the open kills nearly everyone
whatever they hear. The line a listener did most with was the cut-off's,
which names a rendezvous; the flank line gives a compass way ("round to
your east"), as the job card does ("north-east, 190 m").

## Five jobs again, on the map with the roofs

On Map design's da285bed (sheds on the ways to the mast, cover home from
the Village, the Village's garden shed kept), my head, one seed each:

| Newcomer | Job 1 (scout) | Job 2 (bring) | Job 3 (fetch) | Job 4 (the mast relay) | Job 5 |
| - | - | - | - | - | - |
| cautious | paid | down | the bot couldn't reach the crate | paid | the bot stuck at a door |
| hoarder | paid | paid | paid | down | down |
| lost | paid | down | down | down | down |
| one-handed | paid | paid | paid | the box left, home, paid | down |
| reckless | paid | paid | paid | the box left; down | down |

Every one of the 75 hits after job 1 came after a tell. Run to run,
with one seed each, this moves a lot: a run in between, when a Village
shed had made way for a hut out west, sent job 3's nearest matches out
past the pumping station and three newcomers died fetching them (Map
design put the shed back). The lost one, by the shop, was warned every
time (a drone's lock, 15 s ahead) and went in at the door; the bot stood
it in the doorway's line, which the rooms' plan now puts right.
