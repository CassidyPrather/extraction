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

## The bot's own faults, fixed on the way

These made a newcomer look stuck when a person wouldn't have been:
- walking out of the truck bed through its side rather than over the
  tailgate;
- forgetting a line that had gone from the log before it looked ("noted",
  the truck's reply);
- a step to the side round an obstacle that itself got stuck, and counted
  as a failure;
- the cautious newcomer never running, even when the yard's board said to.

## Results

### First run (before the fixes above)

| Newcomer | Yard | First raid |
| - | - | - |
| cautious | all 23 steps; but the first time, burned to death by the stone in silence | stuck leaving the truck bed; the truck's reply misread |
| reckless | all 23 steps (walked into the shimmer for the stone) | to the farm and noted; at the rail yard the truck came down on top of it |
| lost | all 23 steps, with walks that stalled coming back from the wrong posts | set off the wrong way, into the camp's edge, and stayed stuck |
| hoarder | all 23 steps; a step failed with its hands full of what it had picked up | home, with two stalled walks |
| one-handed | all 23 steps | home, with five stalled walks (the truck bed, the spot) |
