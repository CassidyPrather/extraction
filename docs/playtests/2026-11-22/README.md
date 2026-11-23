# Playtest, 2026-11-22 (Valve Index, SteamVR, RTX 3080, build 0.1.0+g5ccaabcc)

A short evening session, about 35 minutes. The tester set out to check two things:
**onboarding** (NEW CAREER, then the training yard) and **arcade mode** (`--arcade`).
The tester said the yard was "an improvement over every version so far" and that shooting
out a walker's camera was "so fun". Two bugs made both modes nearly unplayable, though:
board text that never draws, and a holster/pocket that won't give items back.

Times are `MM:SS` on the video clock (0:00 = 21:57:52 PST = 05:57:52Z). The game logs
match the speech to within a few seconds (see `timeline.md`). Screenshot names start
with the video time as `HMMSS`. The `*_ingame.jpg` files are the game's own mark
screenshots. Every other picture is a frame from the SteamVR mirror at the moment the
tester was talking about. A frame may sit a second or two off the exact moment.

> **In the repo:** `game-data/logs/` is `logs/`, `game-data/raids/training_yard/` is `raids/`,
> `game-data/arcade/` is `arcade/`, and `career.ron.bak` is `career-before.ron`. The full-size
> mark PNGs and `career.ron.old` aren't committed; the `*_ingame.jpg` frames show the marks.
> Who owns what is in [`ROUTING.md`](ROUTING.md).

## What's in here

| Path | What |
| - | - |
| `README.md` | Findings by area, with times and pictures (this file) |
| `timeline.md` | Speech, game-log events and screenshots merged on the video clock |
| `transcript.txt` / `transcript.json` | faster-whisper large-v3 transcript (`{start, end, text}` in seconds) |
| `screenshots/` | 47 video frames + 4 in-game mark screenshots |
| `game-data/logs/` | Today's 6 `session-*.log` and `frames-*.txt` |
| `game-data/raids/training_yard/yard-1795413723.jsonl` | Yard telemetry |
| `game-data/arcade/` | Arcade run telemetry, summary and `guns.json` |
| `game-data/marks/` | Marks 1-4, full-size PNG |
| `game-data/career.ron.bak`, `.old`, `settings.ron` | See "career" below. `career.ron` itself is gone (NEW CAREER) |

The game's processes, in video time:

| Log (`session-<epoch>`) | Video | What |
| - | - | - |
| `1795413400` | -1:12 → -0:25 | Old career, Marrow Creek (before recording started) |
| `1795413462` | -0:10 → 1:35 | Old career again |
| `1795413577` | 1:45 → 1:56 | Old career, closed after 11 s (relaunching with `--help`, per the speech) |
| `1795413682` | 3:30 → 4:09 | Camp → NEW CAREER |
| `1795413721` | 4:09 → 17:35 | **Training yard**, new career (the game relaunched itself) |
| `1795414583` | 18:31 → 35:18 | **Arcade** (`--arcade`) |

---

## 1. Blockers

### 1a. Board and card text doesn't render (yard and arcade)
- **4:51** "there is a sign with literally no text on it at all" → `00455_sign_no_text.jpg`
- **6:06** the SETTINGS card: "obviously not, because the text isn't rendering at all" → `00604_settings_button_no_text.jpg`
- **9:46** "some of the text is deciding to render now. A lot of it still isn't." The Out of Reach board rendered by mark 1 (`00952_mark1_ingame.jpg` shows its text). → `00948_text_partly_renders.jpg`
- **18:50** arcade: "none of the text is rendering again, so I have no idea how to operate any of this" → `01846_arcade_load_no_text.jpg`. The target board behind the Mosin at 31:38 is also blank (`03138_gun_clipped_to_face.jpg`).
- The log has no warning for this. The boards' text arrives late or never, possibly a font or text-texture load and the preload/warm-up path. Both runs reached `load: everything in` at about 14 s.
- The result: the tester pressed cards blind. In the yard (log, `1795413721`):
  - 6:00 pressed **SKIP**, which counted as `Button done`
  - 6:09 pressed **SETTINGS**
  - 6:12 pressed **START OVER**, which teleported them back ("Oh, I have teleported")

  The three cards sit about 0.35 m apart (x = -1.1 / -1.5 / -1.8). Spacing the cards out matters less than getting the text to draw.

### 1b. Things put in the holster or a pouch can't be taken back out → the yard soft-locks
- **13:05** holstering works and the hip glow is "delightful" → `01308_hip_holster_glow.jpg`
- **13:26** the pistol turns **white** while in the holster and sits sideways. The tester would prefer it barrel-down. → `01328_pistol_white_in_holster.jpg`, `01416_cannot_draw_from_holster.jpg` (holster/pouch ghost spheres, pistol icon)
- **13:58–14:57** "I have no idea how to pull something out of my pocket … I think I might actually have soft-locked myself in the tutorial … your pocket code has a bug … I'm completely stuck in this tutorial."
- Log: `PutAway done` at 13:21, and no range goal after it (TakeMagazine, load, seat, rack, fire never fire).
- The tester walked into the burner anomaly to end the yard (death at 15:54).
- **20:53** arcade, the same with a magazine: "just like last time, it's not coming out if I put it in the pocket" → `02055_mag_stuck_in_pocket.jpg`
- **27:16** "I will put the pistol in my holster never to be seen again"

### 1c. The game process doesn't exit when the window closes
- **17:31–18:19** the window closed, the log says `load: closed (world dropped) at 806.35 s` (17:35), but the process kept running for over a minute and needed Ctrl-C. See the end of `session-1795413721.log`.

### 1d. NEW CAREER relaunches the game, and the headset drops to SteamVR's loading screen
- **4:07–4:27** "did the game crash? No, I just got kicked out to the SteamVR loading screen while it was loading … the load-in is abrupt and some things load in faster than others, making it very uncomfortable. You need a loading transitory system."
- Log: NEW CAREER's "SURE? AGAIN" fired **twice** 106 ms apart (`camp: on to training_yard (new career)` ×2 at 06:01:59.52 / .62). The process closed and a new one started 0.4 s later. The yard log shows the loading view held until 14.5 s.
- Also **3:42–3:58**: the tester couldn't find NEW CAREER at first and restarted with `--help` to look for a flag. "There was a new career button in the main menu anyway, that I just didn't see."

---

## 2. Onboarding (training yard)

- Overall: "every version of the onboarding I have seen so far has been an improvement over the last right so I'm not demoralized". There's "a sense of clarity and direction here that's way better than last time".
- **5:28** still "barraged with a lot at once in front of me. My line of sight is completely unbroken throughout this entire tutorial area." **16:51** "consider visually breaking it up more so it's not just a flat meadow with some stuff". → `01712_mark4_ingame.jpg` (mark 4, looking back over the yard)
- **6:20** nothing says to move on to the next lamp after a station: "maybe a glow trail along the floor".
- **10:10** far grab: "why is this so hard to target? There's a very narrow band here" → `01012_fargrab_targeting.jpg`. The first thing caught was a **cleaning kit**, which fits no pocket. **10:52** suggestion: hide the rig's pocket UI until the yard teaches pockets. "Does the first item they pick up matter?"
- **11:14** "Why is there a plank here in the middle of the road? Was that supposed to be a crouching tutorial?" → `01115_plank_in_road.jpg`
- **12:33** at the range the instructions are on the sign, not the hand: "I can't see the tutorial text anymore because rather than being an annotation on my hand it's on the sign, I now have to walk back … this is a class of UX problem." → `01238_range_text_on_sign.jpg`
- **13:44** looking straight down at the holster, "my body is spinning wildly". The body IK turns while the head pitches down. → `01346_body_spinning_looking_down.jpg`
- **15:48** walked into the burner to die: "I'm not seeing that damage feedback we discussed." Log: "Back out. That shimmer burns." (15:39), grey-out at 15:54. → `01550_no_damage_feedback.jpg`
- **16:04** respawned at the **opposite end of the yard** in the truck ("The old farm's still waiting. Marrow Creek"; 2 items left in remains). → `01608_respawn_far_end.jpg`
- **16:18** the radio at the end of the yard can be picked up. It "should be the fixed radio transceiver" → `01624_radio_pickup_item.jpg`. **16:29** a floating orb in the distance → `01632_floating_orb.jpg`.
- **16:44** request: "a real solid flow chart of what the player can do in the onboarding section".
- Marks 1–3 (9:52–9:54) came right after the driver's "Anything odd … click both sticks" (9:46), which suggests a test of the gesture. Three in 2.7 s could also mean the both-sticks click re-fires. The tester said nothing about them.

## 3. Hands, holding and guns

- **0:54 / 19:57** the hands "look so much better", and "I love the finger animations" on the slide. Issues:
  - **1:02** an ugly seam where the wrist meets the hand
  - **20:06** "pancaking" around the thumb on the pistol; the thumb goes straight through the pistol; the index finger is too far forward → `02012_thumb_through_pistol.jpg`
  - **31:19** "my left thumb is a stub" → `03122_left_thumb_stub.jpg`
  - **30:52** "the grip for the Mosin is off, to say the least. Clearly not all the hand poses are done."
- **20:19** "I think the trigger on the pistol might be backwards" (the tester isn't sure).
- **19:27** a magazine won't seat: "it's connecting to the bottom of the pistol … way too restrictive of a contact … my IRL hands are blocked by big VR controllers". It probably affects every gun. **25:42** "generous contacts, generous contacts". → `01928_magazine_wont_seat.jpg`
- **19:16** tossing a gun and catching it to change grip is hard; it could be more forgiving.
- **20:25** tutorial text appeared when the pistol was held straight up: "that's cool though, I like that" → `02028_tutorial_text_held_up.jpg`
- **14:32** suggestion: a slightly smaller player body might make handling things easier.
- **Mosin, 29:06–30:46**: the clip didn't go in until the tester found the bolt rotation ("tricky to do in VR, but I can"). Five rounds load at once. Then the trigger did nothing until they **switched hands**: "maybe the control state machine was messed up". → `02915_mosin_clip.jpg`, `03035_mosin_trigger_dead.jpg`
- **31:39–31:59, the strongest complaint in the arcade**: bringing the rifle near the face snaps it to the middle of the view: "don't clip the gun to my face … it's clipping it to the middle of my camera so neither my left eye or my right eye can actually be using [the sights]". **32:47** "with this weird auto clip to the front camera thing it's just not doing it for me". → `03138_gun_clipped_to_face.jpg`
- **33:55** holding A while loading a magazine left over from a previous action gave "a weird slap-to-lock situation" → `03358_slap_to_lock.jpg`
- **35:01** "Throwing grenades is still difficult" → `03502_grenade_throw.jpg`
- **24:46** a magazine set on a windowsill **fell through it** and disappeared: "that would be horrible if that happened to a player" → `02448_mag_through_windowsill.jpg`

## 4. Arcade mode

Run summary (`game-data/arcade/run-1795414586-1.summary.txt`):
- wave 1 cleared, down on wave 2 to a Walker, score 50, 5:50
- PM: 15 shots, 6 hits (40%), 4 reloads, median reload 24.6 s
- feel rated 4/5

- **21:16** "those things are noisy". **22:29–22:38** shooting out the walker's camera: "Oh, that's so fun." → `02232_shot_out_camera.jpg`
- **21:24** "Lev, not now, not the time!" **24:07** "Lev is so heavy-handed, my goodness." The driver's onboarding firsts kept firing mid-fight: Heard, Aimed, ComingToLook, LostYou, Anomaly, and the safety reminder (log, 21:22–24:01).
- **22:38–23:33** tried to pull the blinded walker apart by hand. The haptics said they were grabbing, but nothing came off: "can't get my fingers under this plate or on its legs". Suggestion: a robot left alone that long should self-destruct. The walker then "wandered off the map". **24:26** the tester doesn't think hand-dismantling needs to work for new players, but the haptics made it feel as if it should.
- **22:47** "Why is there an X on the ground?" → `02248_x_on_ground.jpg`. (A driver-cache cross? The job is `Fetch water_bottle`.)
- **23:39** the walker dropped a metal plate that couldn't be reached or picked up → `02341_dropped_plate_unreachable.jpg`
- **22:58** the walker's voice lines (if any) are too quiet.
- **25:00** wave 2's second walker flanked: "it would be cool if they were [doing it on purpose]". **25:07** too far to aim at, and it hung back.
- **26:25–27:04** legs smoking after shots at the joint, then dead. "The joints are just colored spheres right now and the weak points are colored sphere indicators, which I guess is fine for a white-box test." → `02630_smoking_legs.jpg`, `02706_joints_coloured_spheres.jpg`
- **27:26** climbing onto the train car, its **roof disappears** → `02728_train_car_roof_vanishes.jpg`. An "ominous train horn" sounded nearby.
- **27:48** "These stairs do not climb. I kind of hop up and down them." "We need to look at everywhere there are stairs in the game." → `02750_stairs_hop.jpg`
- **28:06** opening and closing cupboards is good. Foliage clips into them a little. → `02810_cupboard_foliage_clip.jpg`
- **28:18** looking down at a shack from the side shows the grass under it instead of the foundation top → `02825_grass_under_shack.jpg`
- **32:18–32:33** the feedback board: "not even functional enough to use … I pushed the button and it took three feedback options … no debounce". → `02826_feedback_board.jpg`, `03236_feedback_triple_press.jpg`
  - The log shows three `read` events for "4 good / 3 fine / 5 great" within 45 ms at 32:24, then one `press` "4 good" followed 78 ms later by a second `press` with an empty label (32:31). Only `pm_pistol felt 4/5` was recorded.
  - So the triple was probably the hover/read highlight lighting three ratings, plus one real double-press. NEW CAREER double-fired the same way (1d).

## 5. Looks

- **6:35** "a very strange cube lighting texture on the ground everywhere in this field … it's definitely a bug" → `00638_ground_cube_lighting.jpg`. **9:16** "still some square tiles where the texture blending is completely wrong", and the grass texture repeats visibly ("the reuse of the asset is too heavily leaned upon") → `00920_grass_tile_blend.jpg`
- **8:43** z-fighting on a fence post → `00846_fence_post_zfight.jpg`
- **7:07** backpack: the bottom and flap shown in full are "long and clunky"; there are see-through holes on either side; the side pouches look decorative → `00710_backpack_holes_flap.jpg`
- **7:37** distant trees haven't improved. LODs visibly pop. A tree's trunk shows at the far LOD but goes invisible up close → `00752_tree_lod_trunk.jpg`. Very narrow silhouettes and big flat filled triangles ("like the tail of a 3D-printed dragon") → `00808_tree_silhouettes.jpg`
- **11:19** anomalies seen in the distance are "a huge step up … I like this a lot", but too stark against the environment, especially on the floor → `01130_anomalies_distance_stark.jpg`. **15:18** the low-res patch on the ground under the lightning anomaly → `01522_anomaly_floor_lowres.jpg`
- **12:13 / 15:18** the lightning anomaly looks blocky and too fast **at a distance only**. Up close it's fine, probably a rendering/LOD bug. → `01216_lightning_blocky_far.jpg`
- **8:16** liked the small shed and the explorable bits of the yard.

## 6. Logs and performance

- **Frame rate**: neither mode held 90 Hz. Eyes were 1800² (60% of 2980²), with hold-rate on.
  - **Yard**: about 20 ms a frame (50 fps), 1% about 28 ms. GPU about 6.5 ms (`main_opaque_pass_3d` about 5.3). The **CPU render world is 10 ms (1% 20 ms)**, the bottleneck, beside main at 6.2 ms.
  - **Arcade**: about 14 ms (70 fps), 1% about 17 ms. GPU 6–8 ms. CPU main about 7 ms, render world about 5.5 ms.
- **Old career** (sessions `1795413400` / `1795413462`, before NEW CAREER):
  - 281 `item … fell out of the world; put back` warnings per load
  - about 100 `Entity despawned` command errors
  - `save: unknown item "chemlight"` / `"hand_flare"` (8 per load)
  - `marrow_creek: 2 item value(s) mended`
  - one `cpal … StreamInvalidated`
  - None of these recur after the new career.
- `Failed to download render statistics buffer` at every shutdown (harmless?).
- **Career**: NEW CAREER replaced the save. `career.ron` no longer exists; the old one is `career.ron.bak` / `.old`. The yard was played on the fresh career and the arcade doesn't touch it.
- The 15 MB `2026-11-22 21-57-01.mkv` just before this recording is a false start and isn't included.
