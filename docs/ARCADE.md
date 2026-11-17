# Arcade

A sandbox for item mechanics, tactics and UI, without the strategy
layer, and a place to collect playtest data.

```bash
extraction --arcade [--data-dir DIR] [--flat] [--weather W] [--hour H]
```

A menu can open it by calling `arcade::enter(&mut LaunchOptions)` and
relaunching, as `--arcade` does. The camp board is the natural place for an
ARCADE card, next to TRAINING YARD.

## A run

- **The ground** (`assets/maps/arcade.ron`): a clearing about 50 m across
  inside a tight fence. Its cover is built from the valleys' own pieces: a
  brick house, a guard booth, two car wrecks, a boxcar and log piles.
- **The bench** (`arcade::arena`), at the south edge, has five camp
  tables:
  - three tables of kit:
    - the AK-74 and the PPSh (each loaded) with two spare AK magazines;
    - the Mosin and the SKS with their clips;
    - the MP-133 with a box of shells, and the PM with two spare
      magazines and two dressings;
  - a fourth table with the board and two RGD-5s;
  - a fifth, the other side, with the gun board (below).
- **The board**: the sheet shows what is on, what comes next and through
  which gate, and the next four waves of the ladder. The cards are:
  - CALL WAVE;
  - RESTART;
  - START AT WAVE 1/4/7;
  - turning, vignette and sound, as on the truck's board.
- **Waves** (`arcade::waves`): nothing comes until you press CALL WAVE.
  The board names the machines and the gate. That gate's lamp burns red until the wave is cleared, and
  the machines' siren sounds from it. Ten seconds later the machines come
  through it.
  - Each machine is spawned as the director sends one, as a response to
    where you stand, with its own mind.
  - The ladder is walker / 2 walkers / walker + drone / hound / 2 walkers
    + drone / heavy. It then goes round again with one machine more each
    time round.
  - The gate turns north, east, west in order. All of it is printed on
    the board before you call.
- **The range** (`arcade::range`), east of the bench: a firing line and
  steel plates hung at 10, 25 and 50 m. A struck plate rings (the game's
  own steel impact) and swings back on its chains as hard as the round hit
  it. It's there to judge a gun's recoil, sights and sound with nothing
  shooting back. Shots at it between waves are marked as the range's.
- **The gun board**: the gun card, each gun fired this run with its
  shots, hits and accuracy, shots and hits to kill, median time to kill,
  median reload, and accuracy by distance (fight and range), each against
  that gun's best before this run (kept across sessions in
  `<arcade dir>/guns.json`). It's live through a run and holds the last
  run's until a gun fires in the next. After a run it asks, gun by gun,
  "how did it feel?": cards 1 (poor) to 5 (great), or SKIP. One press
  each, in the headset.
- **Score**: a walker is 100, a drone 75, a hound 150 and a heavy 400. A
  cleared wave adds 50 × its number.
- **Down**: the run is over. You are up again at once, the ground is
  quiet, and the board carries the run's summary. RESTART drops what you
  carry, sweeps the ground, heals you, stands you at the bench and lays the
  tables again. A call after the end does the same.

## What it leaves out

There is no career, contracts, economy, Surge, director or truck runs.

An arcade session's scene is `SceneChoice::Arcade`. The save code already
saves nothing for a scene other than a raid, so no career is read or
written. The raid stays "out" with its clock held at zero, so the Surge
never comes. The director stays home (`robots` off), so gunfire brings no
one. The raid's answer to going down (the convoy, the remains) is turned
back the same frame.

Guns, ballistics, reloads, stoppages, items, hands, robots and their minds,
health, sound, the HUD and captions are the game's own, unchanged.

## What it records

Each run is written to `<data dir>/arcade/`, or to `target/arcade/` for a
headless run with no `--data-dir`. A run's record opens when the bench is
laid (the range before the first call is in it) and closes when you go
down or press RESTART. The playtest report's zip carries the newest four
runs.

- `run-<stamp>-<n>.jsonl` holds the sims' telemetry lines
  (`docs/SIMULATION.md`), so `scripts/sim-report.sh` reads it like a raid.
  It also has the arcade's own lines:

  | `ev` | fields |
  | - | - |
  | `arcade_run` | start wave, run number, build |
  | `wave_called` | wave, machines, gate |
  | `wave_arrived`, `wave_cleared` | wave (score, time) |
  | `picked_up` | gun, its order this run |
  | `fired` | gun, `range`, band, distance to the target, `mid_fight` |
  | `gun_hit` | gun, target (machine or plate), part, damage, distance, `range` |
  | `reload` | gun, `mid_fight`, by magazine or by rounds |
  | `reload_done` | gun, cost (last shot to next), time (magazine out to next shot), `mid_fight` |
  | `stoppage` | gun, kind, `mid_fight` |
  | `stoppage_cleared` | gun, seconds to the next shot |
  | `machine_down` | machine, wave, gun, time to kill, shots, hits, distance, points |
  | `item_used`, `grenade` | item, `mid_fight` |
  | `run_over` | wave, score, time, waves cleared, down or restart, cause, guns used |
  | `feel` | gun, 1 to 5 (added after the run, as answered) |

- `run-<stamp>-<n>.summary.txt` gives, per gun:
  - shots, hits, accuracy, reloads, mid-fight reloads and stoppages;
  - TO KILL: kills, shots and hits a kill, median time to kill;
  - ACCURACY: by distance in a fight (under 15, 15 to 30, over 30 m) and
    at each plate;
  - STRUCK: strikes and mean damage by the part struck;
  - RELOAD: median cost (last shot to next), mid-fight, and magazine out
    to next shot;
  - HANDLING: stoppages per 100 rounds and median time to clear, median
    time to a string's first hit and hits in its first three shots, and
    the gaps between shots (median and middle half).

  It also lists kills by machine (median time to kill, shots and hits a
  kill), guns in the order picked up, items used, what took you down,
  waves, score and time, and the FEEL answers once given.

A reload is **mid-fight** when a machine of the run is still standing
within 40 m of you, or is aware of you (sees you, or is after you with
suspicion of at least 0.5). The jsonl and the summary use the same rule
(`arcade::record::mid_fight`).

The numbers' rules (`arcade::record`, `arcade::stats`):

- A hit is a shot that struck: put down to the shot fired last, within
  3 s, once. A shotgun's pellets that land are one hit; each pellet is a
  strike in STRUCK.
- A fight's shot is banded by the distance to the nearest machine
  standing. Between waves, a shot whose line passes within 1.5 m of a
  plate is the range's, at that plate.
- Shots to kill are the fight's shots since the wave came in or the last
  machine went down; hits to kill are the shots that struck that machine.
- A string is a gun's shots with no gap over 3 s (a reload starts a new
  one).
- Rounds pushed in one after another (a clip, a shotgun's shells) count as
  one reload.

## Tests

- `extraction --headless --scenario arcade_smoke` runs a bot through one
  run. It takes the AK, shoots the 10 and 25 m plates, presses CALL WAVE,
  fights the walker, reloads from a spare, fires again, checks the gun card
  after the wave, presses RESTART and answers how the AK felt. It passes
  only if the run's jsonl has every beat and gun-feel line (range shots and
  hits, hits with their distance, the kill's shots and distance, the
  reload's time, the feel answer), the summary has every section, the
  tables are laid again, and no career was touched. It is in
  `--scenario all`.
- Unit tests (`cargo test -p extraction --lib arcade`) cover the ladder, the
  gates, the board's sheet, the kit's catalog ids, the summary and gun card
  numbers and bests, the plate a shot was aimed at, the feel answers in
  both files, a plate's hang and swing, the lane inside the fence, and the
  mid-fight rule.
- `cargo test -p extraction_core arcade_map_lint` holds the ground to the
  yard's rules (reach, footing, overlap) and prints the rest.
