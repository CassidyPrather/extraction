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
From a checkout, run `python scripts/fetch_assets.py` once first: the
textures aren't in git (without them everything is flat colour). The
packaged zip has them.

**Which headset runtime.** Windows has one *active* OpenXR runtime for
the whole machine, whichever installed or claimed it last (Meta's often
lingers after the Oculus app). The game uses that one. On a SteamVR
setup (Index, Bigscreen Beyond, Vive), start SteamVR first, then either
make it the active runtime (SteamVR → Settings → OpenXR → "Set SteamVR
as OpenXR runtime") or launch with `--steamvr`. If the active runtime
finds no headset and SteamVR is installed, the game reopens itself on
SteamVR once by itself. A flat window saying **NO HEADSET** means the
runtime it ended up on found none: is the headset on and tracking?
Controller bindings cover Index, Quest Touch (and Touch Plus), Vive
wands and WMR controllers.

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

## Recording a session

In the headset the game's own window stays grey: it draws nothing
unless you start with `--mirror`, which renders the left eye a third
time (a real cost on a laptop GPU). To record, capture SteamVR's mirror
instead (SteamVR menu → Display VR View, then an OBS window capture of
it): it shows what the headset already has, for free.

If you record video and talk over it, two habits make it easy to line
the recording up with the logs afterwards:

- **Say the mark out loud.** When you press a mark, say "mark 3" and a
  word on why. The wrist confirms the number; the log, the raid
  telemetry and `marks/` all carry it, so the video's timestamp, the
  spot on the map and the screenshot meet on that number.
- **Say when you start a raid and which valley.** Raid telemetry is one
  file per raid, named in order; "raid two, Kettle Pass" on the tape is
  enough to pair them.

Then hand over the recording (or a transcript of it) with the files
below, and the findings can be pulled out of both together.

## What's new to look at (since the first headset session)

The first session found the controls mostly dead and a lot of stooping.
What changed, and worth a comment each:

- **Controls.** SteamVR had handed the game a generic "simple
  controller" profile instead of the Index bindings, so most buttons did
  nothing. That fallback is gone. The log now names each hand's profile
  (`input: Right hand profile ...`) and the first use of every control
  (`input: Right trigger works`): if something's dead, the log says which.
- **Hands.** The hand points where your hand points, and the gloves sit
  the right way round. **Grip** takes hold of things (doors, cords,
  cards too). **Trigger**, with an empty hand, works a slide, bolt or
  charging handle on the gun in your other hand. The glove glows green
  and the controller ticks when something's in reach.
- **Out of reach: point and flick.** Point an empty hand at something up
  to 5 m off (a green ring shows what), squeeze the grip, flick your hand
  back: it hops into your hand. Open the hand and it lands at your feet.
- **Off the floor.** Your kit is in the stash crate, on a stand at
  waist height. Loot in the open sits on supply crates, and indoors it's on
  tables, shelves and crates. Floors still happen, as the exception.
- **The body.** Belt and holster stay at your hip when you bend over;
  they come down only if you crouch right down. Reach over your
  shoulder and grip to take the pack off; let go of a thing over your
  shoulder and it goes in the pack. Your own body no longer shades you.
- **Magazines** seat when their top meets the well, and the bell cord
  is easier to catch.
- **Frame rate.** The Beyond 2 ran at 20-30 fps. The **SHARPNESS** card on
  the truck wall sets the render scale (auto is about 60% on a Beyond 2;
  takes effect next launch), as does `--render-scale 0.6`. The log says
  the size each eye renders at (`eyes: ...`). Tell me how it looks
  and how it feels.

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
- Both hands track and point where your hands point. Grip picks things
  up; the trigger works slides and bolts (and fires a gun in hand).
  Point at something far off, squeeze and flick back: it comes to you.
- Left stick walks (click to sprint), right stick snap-turns (or turns
  smoothly, see Comfort) and, pulled down, crouches.
- The controllers buzz: grabbing something ticks, a shot kicks the hand
  on the grip hardest.
- Your left wrist shows the Surge clock, your load and the latest message.
- Sound comes from the right places (turn your head near the creek).

If any of this fails, stop and send the log: everything else depends on it.

### 1b. The yard and the camp (10 minutes)

- A first launch (no career yet) opens in the **training yard**: a small
  fenced valley with nothing hunting you and nothing kept. A gravel track
  runs north from the truck past signed stations (kit, pockets, loading
  and the targets, searching, staying alive, the machines, artifacts,
  calling the truck). Do the stations; mark a sign you couldn't read or
  that told you wrong. Pull the cord in the truck when you're done, or
  press the card by the tailgate to skip: either way the game reopens in
  Marrow Creek.
- Every later launch opens at the **camp**: you stand at a table behind
  the truck, a board on it with your career and cards to press (TRAINING
  YARD, NEW CAREER (press twice), the comfort cards, PACK UP to quit).
  To play, walk out through the gate. Is this a good first minute?

### 2. Comfort (5 minutes)

Press the cards on the camp board (or the truck bed's left wall): turning (snap 30°/45°,
smooth 90–180°/s), vignette (off to 90%, darkening the edges while
the stick moves you) and sound (full range, or narrow: the forest and
your hands up, gunfire down, for open speakers in a room that isn't
quiet). Note which you end up with, and anything that made
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
- The training yard (`--map training_yard`) has targets at 15, 30 and
  60 m if you want to shoot without the raid around you.

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

### 4b. The long game

Notes on tables and terminals in the buildings (press a terminal for its
next page) tell what the machines are and point to three depot keys, one
per valley, each in a strongbox with a blinking beacon at the valley's
worst place. Bring all three to the lock beside any robot Gate in one
raid and set them in its cradle. Mark whether the trail was findable and
whether the ending felt like one.

### 5. Between raids

- Hand the driver what they ask for (stash it, press their card). Their
  cache shows on your field map as a red cross. Did you go and get it?
- Change valleys with the route card and the bell.

## Known rough edges

- **No real headset has run this yet.** The headset path does run
  against Monado's simulated headset (`scripts/xr_check.sh`): the
  OpenXR session starts and gets focus, every binding is accepted, and
  the eyes see the game from the player's head with its fog and
  lighting. That check found and fixed three bugs that would have
  sunk the first session: the eyes were at the world origin, under
  the map; they had none of the game's fog or exposure; and their near
  plane cut off iron sights. What it can't check: your controllers'
  feel, the floor height, hand offsets on the grips, haptic strength.
- On Linux the game needs the OpenXR loader as `libopenxr_loader.so`
  (install the loader's dev package, or symlink the `.so.1`). On
  Windows it's built in. Without a runtime the game starts flat.
- Room-scale: your body follows your head around the playspace. Walk
  physically into a wall and the world is nudged back to keep you out of
  it; tell me if that feels bad.
- Fingers are posed, not tracked: they wrap what you hold (a pistol grip
  with the index along the frame until you squeeze, a handguard, a
  slide, a can, a round between finger and thumb), and empty-handed the
  grip closes the hand and the trigger the index. Mark a grip that looks
  wrong.
- Some models are still simple, the hall's lathes among them.
- The camp, the yard and the keys are new and have never been played
  through end to end by a person: expect rough edges in them first.
- To see the yard again after skipping it, delete `trained` from the
  data folder (or press NEW CAREER at camp). For a first launch without
  touching your own career, run with `--data-dir` pointing at an empty
  folder.
- Drones still decide most deaths in the sims; whether that's right is
  the main balance question for you.
- Sim bots sometimes wedge themselves on doors. A person shouldn't, but
  mark it if a door ever traps you.

## Handy switches

| Flag | What |
| - | - |
| `--flat` | Monitor and keyboard instead of the headset |
| `--data-dir DIR` | Keep the career, settings and logs in DIR instead of the usual data folder: a second player's profile, or a clean first launch |
| `--detail 0.5` | Less grass, fewer field stones, shorter shadows (0.25 to 1; 1 is the default). Try it if the frames log says the 1% figure is over 11 ms |
| `--mirror` | In the headset: show the left eye's view in the desktop window, for whoever is watching (costs a third render of the scene, so not by default) |
| `--map NAME` | `marrow_creek`, `kettle_pass`, `cinder_works`, `training_yard` |
| `--camp` | Set up the camp table even in a screenshot |
| `--new-game` | Ignore the saved career |
| `--surge-in SECONDS` | Bring the Surge close (try 330 for the warning) |
| `--weather NAME` | `drizzle`, `fog`, `downpour`, `clearing` |
| `--no-robots` | An empty Zone, for looking around |
| `--steamvr` | Use SteamVR's OpenXR runtime, whatever the active one is |
| `--render-scale 0.6` | Render each eye at this fraction of the headset's size (the SHARPNESS card) |
| `--smooth-turn`, `--vignette 0.5`, `--narrow-dynamics` | Comfort, without the cards |
| `--no-record` | Don't record raids |
