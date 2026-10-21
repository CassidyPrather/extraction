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
  corner.

## Results

### Last runs

The lost and hoarder rows are from this branch's head. The other three are from the round before its last two fixes: the hold that outlived its hand, and the bot's kit order.

| Newcomer | Yard | First raid |
| - | - | - |
| cautious | all 23 steps | home, 510 s |
| reckless | all 23 steps; walked into the shimmer for the stone | home, 196 s |
| lost | 22 of 23: back from its wrong turns, it couldn't reach the house's crate from the side it came in by (the bot's fault: the search step assumes the usual way in) | home, 313 s (set off the wrong way first) |
| hoarder | all 23 steps; one bot step failed (its own dressing, pocketed earlier, wasn't found to use) | home, 286 s |
| one-handed | all 23 steps | home, 289 s |

None was hurt, stuck or confused on the raid, bar the lost one's wrong turn. None met a machine: the farm is a danger-1 place, and the first job's road there is quiet. So the reckless one never got its fight, and the first raid's danger is still to be measured. A second job, or a later hour, is where that starts.

#### cautious

| t | What | Moment |
| - | - | - |
| 0 s | Learned | camp: "Your kit is in the crate in the truck" |
| 13 s | Learned | the job card: ODD JOBS - JOB 1 OF 3 "Go up to the old farm and look round. Tell me what's changed." From the drop: north-east, 190 m. Ringed on your map. |
| 18 s | Learned | on to Old farm |
| 34 s | Learned | Detector: Truck: "Hear it ticking? Faster's nearer. Sweep it slow." |
| 43 s | Learned | Anomaly: Truck: "Throw something in first. Air's wrong ahead." |
| 199 s | Learned | the place, noted: "Now get it home" |
| 510 s | Learned | in the truck and away |

#### reckless

| t | What | Moment |
| - | - | - |
| 0 s | Learned | camp: "Your kit is in the crate in the truck" |
| 7 s | Learned | the job card: ODD JOBS - JOB 1 OF 3 "Go up to the old farm and look round. Tell me what's changed." From the drop: north-east, 190 m. Ringed on your map. |
| 7 s | Learned | on to Old farm |
| 11 s | Learned | Detector: Truck: "Hear it ticking? Faster's nearer. Sweep it slow." |
| 14 s | Learned | Anomaly: Truck: "Throw something in first. Air's wrong ahead." |
| 69 s | Learned | the place, noted: "Now get it home" |
| 196 s | Learned | in the truck and away |

#### lost

| t | What | Moment |
| - | - | - |
| 0 s | Learned | camp: "Your kit is in the crate in the truck" |
| 9 s | Learned | the job card: ODD JOBS - JOB 1 OF 3 "Go up to the old farm and look round. Tell me what's changed." From the drop: north-east, 190 m. Ringed on your map. |
| 9 s | Confused | set off the wrong way |
| 9 s | Learned | on to Old farm |
| 138 s | Learned | the place, noted: "Now get it home" |
| 313 s | Learned | in the truck and away |

#### hoarder

| t | What | Moment |
| - | - | - |
| 0 s | Learned | camp: "Your kit is in the crate in the truck" |
| 10 s | Learned | the job card: ODD JOBS - JOB 1 OF 3 "Go up to the old farm and look round. Tell me what's changed." From the drop: north-east, 190 m. Ringed on your map. |
| 10 s | Learned | on to Old farm |
| 110 s | Learned | the place, noted: "Now get it home" |
| 286 s | Learned | in the truck and away |

#### one-handed

| t | What | Moment |
| - | - | - |
| 0 s | Learned | camp: "Your kit is in the crate in the truck" |
| 9 s | Learned | the job card: ODD JOBS - JOB 1 OF 3 "Go up to the old farm and look round. Tell me what's changed." From the drop: north-east, 190 m. Ringed on your map. |
| 9 s | Learned | on to Old farm |
| 18 s | Learned | Detector: Truck: "Hear it ticking? Faster's nearer. Sweep it slow." |
| 23 s | Learned | Anomaly: Truck: "Throw something in first. Air's wrong ahead." |
| 113 s | Learned | the place, noted: "Now get it home" |
| 289 s | Learned | in the truck and away |


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
