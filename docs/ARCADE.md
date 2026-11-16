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
- **The bench** (`arcade::arena`), at the south edge, has four camp
  tables:
  - three tables of kit:
    - the AK-74 and the PPSh (each loaded) with two spare AK magazines;
    - the Mosin and the SKS with their clips;
    - the MP-133 with a box of shells, and the PM with two spare
      magazines and two dressings;
  - a fourth table with the board and two RGD-5s.
- **The board**: the sheet shows what is on, what comes next and through
  which gate, and the next four waves of the ladder. The cards are:
  - CALL WAVE;
  - RESTART;
  - START AT WAVE 1/4/7;
  - turning, vignette and sound, as on the truck's board.
- **Waves** (`arcade::waves`): nothing comes until you press CALL WAVE.
  The board names the machines and the gate. That gate's lamp burns red and
  the machines' siren sounds from it. Ten seconds later the machines come
  through it.
  - Each machine is spawned as the director sends one, as a response to
    where you stand, with its own mind.
  - The ladder is walker / 2 walkers / walker + drone / hound / 2 walkers
    + drone / heavy. It then goes round again with one machine more each
    time round.
  - The gate turns north, east, west in order. All of it is printed on
    the board before you call.
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
headless run with no `--data-dir`. The playtest report's zip carries the
newest four runs.

- `run-<stamp>-<n>.jsonl` holds the sims' telemetry lines
  (`docs/SIMULATION.md`), so `scripts/sim-report.sh` reads it like a raid.
  It also has the arcade's own lines:

  | `ev` | fields |
  | - | - |
  | `arcade_run` | start wave, run number, build |
  | `wave_called` | wave, machines, gate |
  | `wave_arrived`, `wave_cleared` | wave (score, time) |
  | `fired` | gun |
  | `reload` | gun, `mid_fight`, by magazine or by rounds |
  | `stoppage` | gun, kind, `mid_fight` |
  | `machine_down` | machine, wave, gun, time to kill, points |
  | `item_used`, `grenade` | item, `mid_fight` |
  | `run_over` | wave, score, time, waves cleared, down or restart, cause |

- `run-<stamp>-<n>.summary.txt` gives, per gun, shots, hits, accuracy,
  reloads, mid-fight reloads and stoppages. It also lists kills and mean
  time to kill by machine, items used, what took you down, waves, score
  and time.

A reload is **mid-fight** when a machine of the run is still standing
within 40 m of you, or is aware of you (sees you, or is after you with
suspicion of at least 0.5). The jsonl and the summary use the same rule
(`arcade::record::mid_fight`).

A hit is put down to the gun fired last, within 3 s. Rounds pushed in one
after another (a clip, a shotgun's shells) count as one reload.

## Tests

- `extraction --headless --scenario arcade_smoke` runs a bot through one
  run. It takes the AK, presses CALL WAVE, fights the walker, reloads from a
  spare and presses RESTART. It passes only if the run's jsonl has every
  beat, the summary names the AK and the walker, the tables are laid again,
  and no career was touched. It is in `--scenario all`.
- Unit tests (`cargo test -p extraction --lib arcade`) cover the ladder, the
  gates, the board's sheet, the kit's catalog ids, the summary and the
  mid-fight rule.
- `cargo test -p extraction_core arcade_map_lint` holds the ground to the
  yard's rules (reach, footing, overlap) and prints the rest.
