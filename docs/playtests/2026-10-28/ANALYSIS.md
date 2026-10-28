# Playtest log analysis: build 2798e450, 28 October, Beyond 2 + Index, SteamVR

Read from the logs in this folder: six sessions, their frame logs, the
crashes, the two Marrow Creek raids, and the career, settings and
`trained` files. Times are UTC as logged. Inferences are marked
**(inference)**. Several slow stretches may be the designer deliberately
checking hardware (PLAYTEST §1); the spoken transcript will settle those.

## 0. Headlines

1. **The whole night ran at a half or a third of the headset's rate, and
   the eye governor made it worse.** The Beyond 2 ran at 75 Hz (13.3 ms a
   frame). After load, 96.7% of yard frames and 99.7% of Marrow Creek
   frames were over 13.3 ms. Medians:
   - yard 26.5 ms;
   - Marrow Creek 27.8 ms;
   - the Marrow Creek forest 37.9 ms.

   The governor took the eyes from 60% to 100% before the headset was even
   showing the game, then stayed at 100% for all 142,251 frames after. It
   never stepped down, for two reasons:
   - it budgets against SteamVR's throttled `predicted_display_period`
     (26.7, 40 or 53.3 ms) instead of 13.3;
   - its GPU reading covers about one view, a median of 6.6–10 ms, against
     15–19 ms for all passes.
2. **The first spoken line in the headset was the flat-screen one** ("Look
   at it, press E."), in all six launches. It fires 1.5 s in, before the XR
   session exists, and stayed the newest wrist message for 8.5 minutes.
3. **The first death came with a loaded pistol in hand.** The player
   emptied the magazine at a walker, dropped it and seated the spare, but
   never released or racked the slide. The remains show `chambers: [None]`
   with an 8-round magazine in. The yard never makes you reload from
   slide-lock; that appears only as a tip after the range is done.
4. **The first hour's goal never happened.** The first job ("Old farm,
   north-east, 190 m") wasn't reached in two raids, and both ended in
   death. The map case was left in the truck. The second raid left the
   truck carrying nothing.
5. **The yard took 32 min 19 s,** against 108–674 s for the bots. The slow
   stretches:
   - the kit, 8.5 min (partly hardware checks);
   - "Getting about", 5.1 min;
   - the range, 5.6 min, with "Rounds into the mag" done last;
   - the dressing, 3.9 min.
6. **Jank with known code:**
   - Marrow Creek's far trees never drew: impostor capture gave up during
     the slow load.
   - A 50–67 ms hitch about 0.2 s after almost every text change.
   - 354 "Entity despawned" warnings, from the readback observers.
   - The second raid's telemetry has no positions and no damage: a
     `Local<Tracker>` carries over between raids.

## 1. Timeline

| # | Session | Time | Map | Outcome |
|---|---|---|---|---|
| 1–3 | 1793167048, 1793167339, 1793168474 | 05:57–06:21, seconds each | Yard | `DeviceLost` 3.6–12 s after the XR session (no camera on the window; fixed separately) |
| 4 | 1793168601 | 06:23, 24 s | Yard | First with `--mirror`; closed by the player |
| 5 | 1793168679 | 06:24:40–06:57:41 | **Training Yard** | All 23 goals; cord pulled; on to Marrow Creek |
| 6 | 1793170661 | 06:57:41–07:32:46 | **Marrow Creek** | 5.5 min at base, raid 1 (19.5 min, killed by a walker), raid 2 (6.1 min, killed by a drone), 2 min at base |

`settings.ron`: everything is default except `body: false`. Marks: none.

### Training Yard

- Greeting at 06:24:44.6, 0.26 s before `XR session created`.
- XR state: VISIBLE at 06:25:06, FOCUSED at 06:25:20.6. That's 36 s before
  the game had focus.
- First use of each control:

  | Control | First used |
  |---|---|
  | Grips | 06:25:20 |
  | Right trigger | 06:25:25 |
  | Sticks | 06:28:13 |
  | All face buttons | within 0.5 s at 06:31:24 (the PLAYTEST §1 control check, **(inference)**) |
  | Left trigger | 06:32:54 |
- Out of the truck at 06:39:42, 14 min 21 s after focus.
- The cord at 06:57:40, then a relaunch into Marrow Creek.

### Marrow Creek

- **Load:** the window stalled for about 8 s ("every model loaded at
  10.57 s"). All 9 impostor captures stayed empty. "Everything in at
  21.50 s". XR focused at 06:58:03.
- **At base:** 5 min 31 s. Reading the camp board and the job card isn't
  logged.

**Raid 1** (07:03:34–07:23:01, 19.5 min, 1,154 m walked). Times are raid
seconds.

| t | Where | What |
|---|---|---|
| 0–150 | South gatehouse | Read "Gate log" (`NotePinned`); loot |
| 178–262 | Field shed (22, 224) | Flushed the rooks |
| 300–460 | Car wrecks | The spare 16-round `ammo_9x18` box was left at (24.0, 192.6) |
| 484 | (42.7, 148.6) | **Closest to the job: 137 m from the Old farm** |
| 515 | | "Something heard you"; the player turned **west**, away from the farm |
| 611–856 | Logging camp | Cabin, "Cabin tally", Sawmill lock-up |
| 978–1015 | The Village | Walker warned 16.8 s ahead; "It's aiming at you" 4.7 s before its first hit |

The fight:
- 8 shots, a whole PM magazine: 5 hits (1 on the weapon, 4 on the core),
  3 rounds into a house.
- "You bind the wound. The bleeding stops.", yet bleeding went on for 19 s
  (about 4 HP).
- "Lost you", then 2 minutes hiding. The empty magazine was dropped at
  t=1112 and the spare seated; the slide was never released.
- The walker came back to 14 m. The `ComingToLook` line never fired, and
  there was 3.5 s of warning before 3 hits in 12.3 s. No shots fired: the
  chamber was empty. Down at t=1167.
- Remains: `pm_pistol` with an inserted 8-round magazine and
  `chambers: [None]`.

**Raid 2** (6 min): the player left with **nothing** (`loot: []`); the
spare radio stayed on the truck's ledge. The dirigible spotted them
twice, attention bought a response team, and a drone lock killed them
16 s later in open ground. The telemetry has no `pos` or `damage` events.

**Career at the end:** raids 2, extractions 0, deaths 2; the job is
`seen: false`.

## 2. Onboarding

### 2.1 The yard step by step (from focus)

| Goal (station) | + from focus | Notes |
|---|---|---|
| Wrist (7) | 0:32 | Ticked by a glance in the truck, 27 min before First aid asks for it |
| TakePistol (1) | 7:21 | |
| TakeRadio (1) | 8:29 | **Station 1: 8:29** |
| PutAway (3) | 9:03 | Done before station 2 |
| Crouch (2) | 11:01 | |
| Turn (2) | 11:13 | Stick used since 06:28; only counts once station 2 is current |
| *(cord pulled)* | 12:06 | "Not done yet. Pull again to go." |
| Run (2) | 13:36 | **Station 2: 5:06** |
| Pack (3) | 13:54 | |
| DropMag, SeatMag (5) | 13:58 / 14:03 | With the kit pistol by the truck |
| FarGrab (4) | 14:25 | |
| Rack (5) | 16:23 | |
| Safety (5) | 17:29 | |
| HitPlate (5) | 17:51 | |
| FillMag (5) | 20:01 | The station's first item, done **last** (**station 5: 5:36**) |
| OpenDoor (6) | 22:37 | |
| Search (6) | 23:08 | |
| Dressing (7) | 27:00 | **3:51 for one dressing** |
| Artifact (8) | 28:59 | |
| Probe (8) | 29:39 | After the stone, against the intended order |
| Button (9) | 30:24 | |
| CallTruck (10) | 31:15 | "Two minutes. Keep low.", then the truck is there 10 s later |
| PullCord (10) | 32:19 | |

The newcomer bots in FIRST_HOUR.md take 108–674 s. The yard isn't
recorded positionally.

### 2.2 Lines that misled or repeated

- **The flat greeting in VR.** `announce()`
  (`src/onboarding/mod.rs:1443–1446`) picks `GREET_FLAT` or `GREET_VR`
  from `RigDriver` at an age of 1.5 s. But `RigDriver` only becomes `Xr` in
  `make_tracking_root` (`src/rig/xr.rs:780–789`), which runs 3–19 s later.
  So "press E" was the wrist's text from 06:24:44 to 06:33:12.
- **"Safety's on. Click the stick on the gun hand." three times:** at
  06:33:12 in the truck, at 06:42:36, and at 06:45:52, three minutes after
  Safety was ticked (probably the range's own pistol, **(inference)**). The
  safety state doesn't read.
- **"Next: the lamp by the pen." twice in 28 s.**
- **An early cord pull during "Getting about".**

### 2.3 Where the time went

- **Station 1:** hardware checks, plus "press E" on the wrist.
- **Station 2:**
  - Turn only counts once the station is current.
  - The board says "squat for real (the stick only slows you)", the code
    accepts the stick (`stick_down`, `mod.rs:1237`), and PLAYTEST.md says
    "right stick down, or squat". They disagree.
- **Station 5:** the kit pistol comes loaded, so the station's first
  steps can be skipped. Box-to-magazine came last.
- **Station 7:** PLAYTEST.md still says "hold it, press A"; the board says
  "wind it round the other forearm".
- **Reloading from slide-lock is never practised.** The range's goals never
  run the gun dry. The only mention is a tip after the station is done:
  "Empty, the slide stays back: a new mag, then B (flat: V) lets it go."

### 2.4 The first raid against FIRST_HOUR

| | Bots (FIRST_HOUR) | The player |
|---|---|---|
| Time at base | 7–13 s to the job card | 5.5 min |
| Went to the job | all 5 | no; closest 137 m, then went west after "Heard" |
| Carried the map | (bots read the sheet) | no: the map case was left in the truck |
| Met a machine | none on job 1 | a walker at minute 16; a drone at minute 6 of raid 2 |
| Outcome | home in 136–367 s, paid | 2 deaths, 0 extractions |
| Raid 2 | | left with 0 items |

Nothing re-prompted the job during 26 minutes in the Zone. Everything
FIRST_HOUR built for the first walk (sheds and log piles on the way to
the farm, survey drones avoiding it, the free short exfil from the gate)
assumes the player goes to the farm.

**Not logged:** the job card, the camp board and comfort or settings cards
being read or pressed. The yard isn't recorded, and no marks were made.

## 3. Jank

| Class | Count | Meaning |
|---|---|---|
| `Entity despawned` command errors | 354 (29 at each launch, 106 and 130 in play) | The readback observers despawn unconditionally (`render/screen.rs:422`, `world/impostors.rs:226`). `Readback` repeats every frame until it's removed, and `World::trigger` doesn't flush commands between triggers, so two completions in one frame despawn twice. Pairs follow text changes. |
| `impostor capture stayed empty` | 9 (Marrow Creek) | `world/impostors.rs:212–249` counts its retries in frames. Marrow Creek's load ran at about 90 ms a frame, and its pipelines were ready only at frame 432; the captures gave up at frame 182. **The far trees were missing all session.** |
| `Camera order ambiguities` | 1 a `--mirror` session | The flat camera and the Mirror overlap at XR start (`rig/xr.rs:747–789`) |
| `DeviceLost` and its family | sessions 1–3 | No camera on the window (being fixed) |

**Hitches on text changes.** 9 of 9 late-yard goal ticks were followed
0.2 s later by a 50–67 ms frame. 56% of yard hitches fall near text
events, against 4% by chance. Each redraw spawns a `Camera2d` and a UI tree
(`render/screen.rs:70–113`) and, on readback, builds an sRGB mip chain on
the main thread (`mipped`, `screen.rs:460–523`). A tick redraws several
panels.

**Telemetry.** `record` keeps `Local<Tracker>` (`sim/telemetry.rs:135`)
across raids. `next_sample` carried over from raid 1 (about 1,168 s), so
raid 2 logged no positions and never flushed its damage. `last_hp`,
`expect`, `inside` and `recent_air` carry over the same way. `heals` and
`engagements` are counted for bots only (`sim/raider.rs:1302/1438`).
`blocked` mixes stopped rounds with blows from a hand or tool.

## 4. Performance

| | p50 | p90 | p99 | >13.3 ms | >26.7 ms | >50 ms |
|---|---|---|---|---|---|---|
| Yard (77,214 frames) | 26.5 | 28.6 | 40.9 | 96.7% | 44.2% | 25 |
| Marrow Creek (66,670 frames) | 27.8 | 40.7 | 43.6 | 99.7% | 67.4% | 66 |

The worst places:
- the Marrow Creek walk to the logging camp, p50 37.9 ms (about 25 fps);
- on to the Village, p50 39.2 ms;
- the yard's burner, p50 33.1 ms.

**The governor:**
- It ramped from 60% to 100% in 12 s, ×1.03 every 0.64 s, before XR was
  VISIBLE, on GPU readings of 0.9–1.4 ms taken while the eyes probably
  weren't being drawn.
- Not one frame ran below 100% after that.
- **Its period:** `period = predicted_display_period`
  (`render/hold_rate.rs:233–238`). The yard reported 26.7 ms in 162 of 197
  readings, and Marrow Creek 26.7 or 40 ms in 205 of 207. Falling behind
  raised the budget.
- **Its GPU reading:** `newest_gpu_ms` (`render/budget.rs`) takes the
  latest measurement per pass path, about one view of three. The 10-s
  `gpu:` line, which sums every measurement, reads 15 ms (yard) and
  19.4 ms (Marrow Creek), up to 28.4.

**GPU or CPU?** Mostly GPU or compositor:
- frame time correlates r = 0.82–0.87 with GPU passes and 0.87–0.96 with
  the render-world thread;
- Marrow Creek's main thread is 12.1 ms (1%: 15.1), over the governor's own
  11.3 ms target, with physics at 2.5 ms.

The `--mirror` view is a third render.

## 5. The top 15

| # | Problem | Owner |
|---|---|---|
| 1 | The governor locks at 100%. It budgets on the throttled period, undercounts the GPU, and ramps before VISIBLE. | performance |
| 2 | 25–37 fps all night; the forest is the worst; Marrow Creek's main thread is 12.1 ms. | performance |
| 3 | The headset's first line is "press E". | onboarding |
| 4 | Reloading from slide-lock is never practised, and it killed the player. Needs a station step, a cue, and haptics or audio on slide-lock. | onboarding, weapons |
| 5 | The first job is never attempted (map left behind, wandered off). Needs a re-prompt or nudge when heading away, and the map in hand. | onboarding, progression |
| 6 | The yard takes 32 min; steps come out of order; the board, the code and PLAYTEST.md disagree. | onboarding |
| 7 | Leaving the truck with nothing. Needs a tailgate check for a radio and a weapon. | onboarding, progression |
| 8 | Marrow Creek's far trees are missing (impostor capture counts frames, not readiness). | visual, performance |
| 9 | A 50–67 ms hitch after every text change. | UI, performance |
| 10 | Telemetry loses positions and damage after a session's first raid. | QA tooling |
| 11 | 354 "Entity despawned" warnings. | render, QA |
| 12 | "The bleeding stops." when it doesn't. | hands, health |
| 13 | The walker fight is hard to read: core hits do little and show it poorly; `ComingToLook` never fired; 3.5 s of warning before the burst. | robots, audio |
| 14 | Driver and caption polish: "Two minutes, keep low" for a 10-s truck; repeats; a note named as a place; "Safety's on" ×3. | onboarding, UI |
| 15 | Observability: card presses aren't logged; the yard isn't recorded; nobody found the mark. | QA tooling, onboarding |
