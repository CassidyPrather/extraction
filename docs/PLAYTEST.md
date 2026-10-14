# Playtest guide

Everything here has been tested headless, with bots, in sims and in
software-rendered screenshots. **None of it has run in a headset yet.**
The first session is as much about finding what breaks on real hardware as
about whether it's fun, so the order below starts with the plumbing.

## Before you start

```bash
cargo run --release                 # VR if an OpenXR runtime is up, else flat
cargo run --release -- --flat       # flat, on the monitor
cargo run --release -- --new-game   # ignore the existing career
cargo run --release -- --map cinder_works   # straight into a valley
```

Keep a release build: the debug build is far too slow for a headset.
SteamVR, Meta Link/Air Link (as the OpenXR runtime) and WMR should all be
picked up; the controller bindings cover Quest Touch (and Touch Plus),
Index, Vive wands and WMR controllers.

Your data folder is `%APPDATA%\extraction` on Windows
(`~/.local/share/extraction` on Linux). In it:

| What | Where |
| - | - |
| The career (stash, kit, record, each valley) | `career.ron` |
| Comfort settings | `settings.ron` |
| This session's log (last ten kept) | `logs/session-*.log` |
| A crash report, if it crashes | `logs/crash-*.txt` |
| Every raid you play, as telemetry | `raids/<map>/raid*.jsonl` |
| Marks you made (below) | `marks/*.png` |

## Marks

**Something wrong, odd or great? Mark it.** Press **F9** on the keyboard,
or **click both thumbsticks together** (standing still) in VR. Your wrist
says "Mark 3 noted.", the log and the raid recording get the spot, and a
screenshot is saved. Afterwards you only need to remember the number.

## After a session

Send me (or keep for me) the newest `logs/session-*.log`, any `crash-*`,
the `raids/` folder and `marks/`. From the repository,

```bash
scripts/sim-report.sh marrow_creek "%APPDATA%/extraction/raids/marrow_creek"
```

draws the same report and heatmaps for your raids as the sims get: where
you went, what hurt you, where you died, and a list of your marks.

The session log also says, every ten seconds, how the frames went
(`frames: ... mean 9.8 ms, 1% 12.4 ms, worst 31.0 ms`). At 90 Hz every
frame needs to be under 11.1 ms; the 1% figure is the one that makes
people feel sick. If a spot is slow, `EXTRACTION_RENDER_STATS=1` prints
what the view draws (meshes and triangles, by what they belong to) a
moment after the game starts; `--camera` puts the view anywhere.

## Order of business

### 1. Does it work in the headset? (10 minutes)

- The view is in the headset, the right way up, the right scale (the
  cards on the truck bed's walls should be about eye height), and it
  follows your head.
- Both hands track. Grip picks things up; the index finger follows the
  trigger.
- Left stick walks (click to sprint), right stick snap-turns (or turns
  smoothly, see Comfort) and, pulled down, crouches.
- The controllers buzz: grabbing something ticks, a shot kicks the hand
  on the grip hardest.
- Your left wrist shows the Surge clock, your load and the latest message.
- Sound comes from the right places (turn your head near the creek).

If any of this fails, stop and send the log: everything else depends on it.

### 2. Comfort (5 minutes)

Press the cards on the truck bed's left wall: turning (snap 30°/45°,
smooth 90–180°/s) and vignette (off to 90%, darkening the edges while
the stick moves you). Note which you end up with, and anything that made
you queasy — climbing, the zipline, being knocked about, swimming.

### 3. The truck and the guns, at base (15 minutes)

- The driver's note (left wall) reads; the logbook and the driver's
  request card beside it read.
- Take the kit from the crate. Load the pistol: magazine out, rounds into
  it from the box, magazine in, rack the slide. Then the rifle and the
  shotgun if you have them. Does it feel like Into the Radius? Where does
  a hand fight you?
- Reach over a shoulder for the backpack; let go of it in front of you to
  open it. Stow things, take them out.
- The bench (right side): strip something, repack rounds.
- `--scene range` is a flat range with every gun on a table if you want
  to shoot without the raid around you.

### 4. A raid (as long as you like)

Step out of the truck: the clock starts and the recording begins. Things
worth noticing:

- **Fog and sight.** Can you see robots before they see you? Too much,
  too little?
- **Drones** chirp three times and flare red before they fire. Do you
  hear it in time to get behind something? (In the sims, bots that duck
  come home 70% of the time; bots that don't, 50%.)
- **The radio** you carry crackles when a response team is sent, and says
  whether it's coming your way.
- **Attention.** Pick up an artifact: your teeth ache, a ringing grows in
  your ears, the wrist tells you when it gets worse. Does that read?
- **Water** slows you; deep water floats you. Try the canal in Cinder
  Works.
- **Getting out.** Radio the truck from the far side, walk to the RV,
  climb the steps, pull the cord.
- **Dying** sends the convoy for your kit. The logbook writes it up
  either way.

### 5. Between raids

- Hand the driver what they ask for (stash it, press their card). Their
  cache shows on your field map as a red cross. Did you go and get it?
- Change valleys with the route card and the bell.

## Known rough edges

- **No headset has ever run this.** Expect the XR side to need fixes:
  bindings for your exact controllers, the floor height, hand offsets on
  the grips, haptic strength.
- Room-scale: your body follows your head around the playspace. Walk
  physically into a wall and the world is nudged back to keep you out of
  it; tell me if that feels bad.
- Hands are simple: no finger tracking beyond the trigger finger.
- Some models are still simple: a few props and buildings are close to
  plain boxes.
- Drones still decide most deaths in the sims; whether that's right is
  the main balance question for you.
- Sim bots sometimes wedge themselves on doors. A person shouldn't, but
  mark it if a door ever traps you.

## Handy switches

| Flag | What |
| - | - |
| `--flat` | Monitor and keyboard instead of the headset |
| `--map NAME` | `marrow_creek`, `kettle_pass`, `cinder_works` |
| `--new-game` | Ignore the saved career |
| `--surge-in SECONDS` | Bring the Surge close (try 330 for the warning) |
| `--weather NAME` | `drizzle`, `fog`, `downpour`, `clearing` |
| `--scene range` | The test range; `--scene truck` the truck alone |
| `--no-robots` | An empty Zone, for looking around |
| `--smooth-turn`, `--vignette 0.5` | Comfort, without the cards |
| `--no-record` | Don't record raids |
