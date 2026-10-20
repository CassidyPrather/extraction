# Map simulation

Bots play raids headless, much faster than real time, and write down what
happened; a script turns a batch of runs into heatmaps and a summary. It is
the tool for "does this map play well?": where people go, where they die,
what they loot, what nobody visits.

```bash
scripts/sim.sh marrow_creek 8              # build, play 8 raids, draw the report
scripts/sim.sh marrow_creek 12 --jobs 3    # 3 raids at a time
scripts/sim-report.sh marrow_creek         # redraw the report from existing runs
```

Telemetry lands in `target/sim/<map>/run000.jsonl ...`; the report in
`target/sim/<map>/report/` (`summary.md` and five PNGs).

## Running

`extraction --sim N` plays N raids, each in a fresh subprocess
(`--sim-run runNNN`), `--jobs J` at a time (default 2). Run *i* gets seed
`--seed + i` (so a different loot roll, anomaly layout and director) and
the personality `looter`, `hunter`, `cautious`, `reckless` in turn, unless
`--personality` picks one. The batch clears old `*.jsonl` from its output
directory first (`--sim-out DIR` to put it elsewhere).

| Flag | Meaning |
| - | - |
| `--sim N` | Play N raids. |
| `--jobs J` | Raids at once (each is one process, about one core). |
| `--seed S` | First run's seed. |
| `--personality P` | Every run plays as P. |
| `--sim-minutes M` | Raid minutes the bots plan for before heading out (default by personality, 10-14). Runs are cut off at M + 15 minutes of game time. |
| `--sim-out DIR` | Where telemetry goes (default `target/sim/<map>`). |
| `--sim-run NAME` | (What the batch passes to each child.) Play one raid, write `NAME.jsonl`, exit. |

A raid runs at about 3-5x real time per process with the `fast` profile
(`cargo build --profile fast`; four at once on a four-core machine), so a
batch of eight 10-15 minute raids takes 7-8 minutes of wall time. Nothing
is shortened for the sim: same tick rate, same truck timings, same robots.
`--sim-minutes` is the knob for shorter raids. Sims never touch the save (headless runs only
save with `--save`).

Environment knobs for debugging: `SIM_DEBUG=1` logs every plan the raider
makes as a `plan` event; `SIM_DUMP_PLANS=1` writes each building's indoor
grid as text to `<out>/plans/`; `SIM_NO_JOB=1` has the raider ignore
the driver's scout job, to compare with runs from before bots took jobs.

## The raiding bot

`src/sim/raider.rs`. A `Raider` brain sits on top of the scenario bot
(`src/bot`): the bot runs a task list (walk here, open that door, grab
this, stow it, shoot that, use the radio), and the raider writes the list.
It replans whenever the bot runs dry, and interrupts it when a robot comes
for it, an anomaly turns up on its route, or it's stuck. Everything goes
through the same `RigInput` and `HandAction`s a player uses, so a sim raid
is a raid a player could have played.

A raid, as the bot plays it:

1. **Kit up.** Step down out of the truck bed, pick up an AK, two
   magazines, a box of 5.45, the radio and two dressings from the ground by
   the insertion point (the dev cache plus a small sim kit), load, set semi.
2. **Loot.** Pick a site — a building with loot spots, an outdoor loot
   marker, an artifact field, a wreck it made — scored by what the
   personality likes, distance, and (as the clock runs) how far it is
   toward the rendezvous side. Walk there on the Zone's nav grid
   (`robots::walker::Nav`, the walkers' grid, with the anomalies it has
   seen blocked out). Indoors it uses a fine grid per building rasterised
   from the prefab geometry (`src/sim/indoor.rs`) and opens doors on the
   way. At the site it looks at what's lying there and takes the best
   things that fit, one at a time, into the backpack (long guns onto a
   free sling). Artifacts sit in anomalies: it walks to the edge, throws
   junk into a snare to set it off, then darts in.
3. **Fight.** A robot that's onto it and close enough to hit (or that's
   hitting it; for the reckless, any robot near enough) gets shot at: the
   AK off the sling, both hands on it, aimed bursts at the core (leading a
   moving target), reload from the pouches, rack when it clicks, a few
   steps sideways when its rounds keep hitting a wall. Afterwards it slings
   the gun, picks up the magazines it dropped and tops them up from its
   ammunition. It dresses wounds between fights, and mid-fight if it's
   bad. Out of ammunition it runs.
4. **Get out.** When the clock says so, the pack is full or too heavy,
   it's hurt (sooner with an artifact on it: the robots keep coming),
   the artifacts it carries draw more attention than its nerve allows
   (`heat`: cautious 1.0, looter 1.6, hunter 2.2, reckless never), it's
   out of ammunition, or there's nothing left worth the walk. Hurt or
   carrying artifacts, it leaves wrecks alone: radio the truck when the walk to the nearest rendezvous is about
   as long as the truck's drive, walk there, wait, climb the rear steps
   into the bed and pull the bell. With the truck waiting close by, it
   stops fighting and runs for it.

Death is data: the run ends when the bot goes down.

### Personalities

| | Likes | Risks |
| - | - | - |
| `looter` | Buildings, everything that fits (heads home at 26 kg or a 90% full pack) | Fights what comes for it; routes round anomalies |
| `hunter` | Artifact fields (artifacts worth 3x) | Skips low-value loot; won't dive into an anomaly below 60 hp |
| `cautious` | Buildings; leaves early (10 min) and light (18 kg) | Slips away from robots that haven't engaged it; won't touch artifacts |
| `reckless` | A bit of everything, to the brim (34 kg) | Walks through anomalies, starts fights with anything within 60 m |

All of them hear a drone's lock-on chirp and, most of the time (cautious
nine times in ten, looter and hunter three in four, reckless three in
ten), step to the nearest spot out of its sight. Ducking is worth a lot:
the same Marrow Creek batch went from 50% home without it to 70% with.

Tune them in `Traits::named`.

### What it knows

What a player who'd studied the map would: where buildings, loot markers,
artifact fields and rendezvous are. What's actually on the shelves it finds
out when it arrives. It sees robots within 60 m and in line of sight, and
anomalies within 30 m.

## Telemetry format

One JSON object per line. Every line has `t` (seconds of game time since
the run started) and `ev`; the rest depends on `ev`. Positions are map
metres, `x` east, `z` south (as in the map files). Format version 1.

| `ev` | Fields | When |
| - | - | - |
| `start` | `run`, `map`, `seed`, `personality`, `minutes`, `format`, `sites`, `buildings` | First line. |
| `phase` | `phase` (`at_base`, `out`, `truck_called`, `truck_waiting`, `extracting`, `down`), `clock` | Raid phase changes. `clock` is the raid clock. |
| `pos` | `x`, `z`, `hp`, `kg` (carried), `stage` (`kit_up`, `raid`, `extract`), `doing` (`travel`, `search`, `take`, `fight`, `to_rv`, `board`, `idle`) | Every second. |
| `kit` | `gun`, `rounds`, `radio` | Kitted up. |
| `goal` | `site`, `kind` (`building`, `spot`, `field`, `wreck`), `x`, `z`, `dist` | Chose a site. |
| `loot` | `item` (catalog id), `site`, `x`, `z` (bot), `ix`, `iz` (where the item lay) | An item went into the pack or a slot. |
| `drop` | `why`, `bulky` | Something didn't fit and went back down. |
| `give_up` | `site` | Couldn't reach a site. |
| `arrive` | `site`, `items`, `artifacts` | Reached a site: loose items lying there (artifacts among them). |
| `stuck` | `x`, `z`, `doing`, `task` | Walked nowhere for 6 s; it sidesteps and replans. |
| `fetch_gun` | `dist` | Its gun got left behind (a held long gun snags on door frames); it went back for it. |
| `engage` | `robot` (`Drone`, `Walker`), `dist`, `noticed`, `x`, `z` (bot), `rx`, `rz` (robot) | A fight starts. |
| `engage_end` | `robot`, `outcome` (`wrecked`, `lost`, `gone`, `out_of_range`, `out_of_ammo`, `broke_off`, `took_cover`, `died`), `shots`, `hits`, `secs`, `x`, `z` | A fight ends. |
| `shot` | `x`, `z` | The bot fired. |
| `hit` | `robot`, `part`, `damage` | A bot round hit a robot part. |
| `blocked` | `what` (the collider's name: `Building shop`, `World edge`, `Terrain collider`...), `x`, `z`, `y` | A bot round hit something else. |
| `reload`, `rack`, `refill`, `reposition` | `reload`: `spare`, `cover` | Gun handling (a reload with `cover` steps out of the robot's sight first, then back); stepping aside when rounds hit a wall. |
| `evade`, `flee` | `robot`, `dist` | Backing off (cautious) or running (no ammunition). |
| `damage` | `cause`, `amount`, `x`, `z`, `hp` | Damage taken, summed per cause per second. Causes: `Drone`, `Walker` (their rounds), `Burner`, `Snare`, `Lift` (standing in one), `surge`, `bleeding`, `fall`. |
| `heal` | `item`, `hp`, `bleeding` | Used a dressing or injector. |
| `anomaly_enter` | `kind`, `x`, `z`, `bloom`, `deliberate` | Walked into an anomaly (`deliberate`: diving for an artifact). |
| `anomaly` | `kind`, `act` (`Pop`, `Discharge`, `Flare`, `Bloom`), `x`, `z`, `dist` | An anomaly went off within 10 m. |
| `duck` | `dist`, `x`, `z` | A drone chirped its lock: off to a spot out of its sight `dist` m away. |
| `crouch` | `secs` | Stood up after crouching (creeping) near robots for `secs` (see `docs/MAP_DESIGN.md`, "How the bots move"). |
| `freeze` | `secs`, `x`, `z` | Froze, crouched and still, while a robot searched close by. |
| `distract` | `x`, `z`, `walker_dist` | Threw junk past a calm walker to send it looking the other way. |
| `cover` | `why` (`hurt`), `dist`, `hp` | Hurt and still being hit: broke off the fight into cover `dist` m away (the fight ends `took_cover`). |
| `decide_extract` | `why` (`time`, `full`, `hurt`, `heat`, `no_ammo`, `done`), `clock` | Heading out. |
| `radio` | `rv`, `x`, `z`, `walk_secs` | Called the truck. |
| `extract` | `x`, `z` | Pulled the bell with the truck waiting: out. |
| `death` | `cause` (what did the most harm in the last 90 s, bleeding aside), `finisher` (what dealt the last point), `x`, `z`, `doing` | Went down. |
| `end` | `outcome` (`extracted`, `died`, `timeout`), `raid_time`, `distance`, `loot` (ids), `loot_count`, `shots`, `hits`, `blocked`, `engagements`, `kills` (by kind), `damage` (by cause), `anomaly_entries`, `heals`, `extract_reason`, `death_cause`, `x`, `z`, `job` (the driver's scout job the bot was handed: `place`, `danger`, `seen`; see `docs/PROGRESSION.md`) | Last line. |

## The report

`scripts/sim_report.py <runs-dir> <map-dump-dir>` (`scripts/sim-report.sh`
runs `map-dump` for you) draws over the map preview:

- `traffic.png` — time spent per place (one sample a second), each run's
  path coloured by outcome, a ring where it ended.
- `deaths.png` — where bots went down, labelled with the cause.
- `engagements.png` — each fight: the bot (dot) joined to the robot
  (▲ drone, ■ walker), coloured by outcome.
- `loot.png` — where picked-up items lay, with the most-looted sites.
- `damage.png` — where bots got hurt, weighted by damage.
- `summary.md` — extraction rate, raid times, deaths by cause, damage by
  source, engagements, most-looted places and items, markers and buildings
  no run came within 15 m of, map coverage, and a line per run.

## Tests and CI

`cargo test` (so CI) covers the pieces: a sim raid's first minute kits up,
picks a goal and sets off, with telemetry to show for it
(`sim::tests::a_raider_kits_up_and_sets_off`, ~20 s in a debug build); a
bot climbs into the truck at base and at every rendezvous; a bot routed by
a building plan walks in through the shop door; and
`every_door_lets_a_bot_in` walks into every outside door. These, and a
check that every rendezvous can be walked to from the insertion, run on
every valley in `assets/maps` (`EXTRACTION_TEST_MAP=kettle_pass` for
just one). A full raid is
minutes of wall time, so CI doesn't play one; run `scripts/sim.sh` before
and after map changes.

The sim has already paid for itself. Its bots found:

- that nobody could step up a kerb or a stair (the step-up never took);
- that the truck's chassis rails walled off its rear steps, and a truck
  parked on a slope left its steps in the air;
- that gunfire bought response teams twice over, so every fight
  snowballed into three or four more;
- that a bot heading home from a back room walked at the wall instead of
  out the door, and one wedged in a barn corner routed out *through* the
  wall (the nearest walkable cell was on the other side);
- that nobody ever crossed Cinder Works' canal: the bridge was laid in
  the navigation grid as cells touching only at their corners, which the
  pathfinder won't step between;
- that a bot leaving the Cinder Works bus depot's office stopped in the
  hall and walked into its wall for the rest of the raid: a hall whose
  bays have no doors counts as "outside" for its doors (bots now take
  the shortest way all the way out of the footprint);
- that a third of a batch's stuck bots were at one end of the Marrow
  village bridge: the cut under a deck began a cell before it, a ditch
  with the deck's edge for a step out (now `a_bot_walks_over_every_bridge`
  checks every bridge both ways), and that routes over a bridge ran
  along the bank beneath it and, on a curving footbridge, cut the
  curve into the rails (decks are lanes in the nav grid now).
