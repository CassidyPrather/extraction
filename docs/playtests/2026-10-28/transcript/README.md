# Playtest handoff: 2026-10-27 headset session

Second headset session (Valve Index on SteamVR, build `0.1.0+g2798e450`, RTX 3080).
It covers the training yard, then Marrow Creek: two raids, and the tester died in both.
The tester talked through the whole session. Their closing summary (1:05:50) asks for:
**focus hard on models and map work, and on onboarding**. Onboarding should be fun and
show one or two things at a time, not a list of instructions.

## What's in here

| Path | What |
| - | - |
| `timeline.md` | **Start here.** Transcript, game-log events and screenshots on one video clock |
| `transcript.txt` / `transcript.json` | Speech only, timestamped (Whisper large-v3, not hand-corrected) |
| `screenshots/HMMSS_label.jpg` | 102 frames from the recording (1920x1080), named by video time. `log-*` ones are at logged events |
| `game-data/logs/` | Every log in `%APPDATA%\extraction\logs`, including four crash reports from *before* the recording |
| `game-data/raids/marrow_creek/raid00{1,2}-*.jsonl` | Telemetry for both raids |
| `game-data/career.ron`, `settings.ron`, `trained` | The data folder's state after the session |

**Clock:** video 0:00:00 = `06:24:59Z` in the logs (23:24:59 PDT). Sessions overlapping the video:
`session-1793168679.log` (training yard, starts 20 s before the video) and
`session-1793170661.log` (Marrow Creek, from 0:32:42). Raid 1 is 0:38:35 to 0:58:02 (died);
raid 2 is 0:59:27 to 1:05:31 (died). The tester didn't call any numbered marks, and there's no `marks/` folder.

## Findings by area (video time, then screenshot label)

### Onboarding and tutorial: the tester's top priority
- Too much text at once on entering the truck. Asked for a title screen, a settings menu, and onboarding that shows one thing at a time (0:01:03, 0:06:13, 0:15:43, 1:06:14).
- Text z-fighting on the truck boards (0:01:00 `text-z-fighting-truck-boards`). The tutorial sign blocks the crate (0:06:00). The board sits at an odd angle (0:11:55).
- "Take the radio": couldn't tell which item in the crate is the radio (0:08:45).
- "Next, the lit lamp" was unclear; the tester got stuck in the truck for about 2 min (0:10:28 to 0:12:45). It means "leave the truck".
- Pockets are never taught before the tester's hands are full (0:09:03).
- "Safety off" doesn't say how. The tester ejected the mag while trying it (0:16:44 to 0:17:55).
- Rounds-into-mag: the tester used the wrong gun and wrong ammo first. 7.62x25 rounds wouldn't come out, and a second tutorial gun appeared (0:19:31 to 0:20:47). A mysterious brown block on the board (0:19:32).
- The "throw a part in" target was unclear, and the tester thought it meant the fire (0:27:40 to 0:30:04).
- "Push the button" should come first (0:30:43).
- The truck just pops in when called; maybe drive it in with an animation (0:31:45).
- Loading into Marrow Creek: the tester sat in the SteamVR loading void with no transition and thought it had failed (0:32:42 to 0:33:09).
- No menu button found (0:06:28).

### Body, arms, hands
- The tester feels slightly tilted. Hips stay put while the legs move, so the body looks like it tilts (0:01:34 `player-tilted-hips-stay-put`).
- Arm IK "isn't good at all". Recommends **removing the arm model entirely**, and the chest straps too, to hide IK problems (0:01:57, 0:04:51, 0:36:16).
- Sleeves are back-face culled, so you can see through your own body (0:05:03).
- Body textures look wrong, and seeing your torso is "icky". BODY hidden looks marginally better (0:02:06, 0:03:22).
- Hands are what you look at most and need the most detail of anything in the game. Current gloves are placeholder quality (0:03:52 to 0:04:42).
- Bending over: some texture clips through the face and the floor can't be seen (0:12:17). This also made a magazine under the crate unfindable (0:34:43 to 0:35:11).
- The ghost hand separating from the glove was noticed and its purpose was unclear (0:12:08).
- **Leaning over a table pushes the play-space root back. "Never, ever do that."** Let the head clip, and use a vignette or fade when it's inside geometry (0:06:33 `leaning-over-table-pushes-playspace`).
- Torso yaw following head yaw may need tuning (0:09:43).
- The wrist's black housing renders in front of its screen when the arm is held to the face (0:26:48). The screen sometimes doesn't show as the wrist rotates (0:22:08). The tester never looked at the wrist during raids (0:58:24).
- Two hands on the pistol: wants finger poses for a two-handed grip (0:17:27), and "I can't use both hands on the gun" (0:55:01).

### Grabbing, guns, items
- **Grabbing the pistol takes the magazine out of it.** This happened repeatedly (0:07:38, 0:10:04, 0:21:15, 0:34:43). Make the seated mag not grabbable; use a release button.
- Holding mag-release could hold the mag half-out for the other hand instead of dropping it (0:19:01).
- Far-grab (point and flick) lands the item about a foot in front of the hand, not in it (0:07:57, 0:14:47).
- The thumb clips through the pistol, and the mag clips through the bottom of the gun (0:08:14, 0:08:31). Z-fighting on the top of the magazine (0:20:16).
- Items dropped above the crate fall through it (0:10:53). An item fell under the crate (0:34:55).
- The battery goes sideways through the flashlight, and the flashlight won't turn on (0:11:13). It fits in no pocket (0:37:08).
- Loading the mag from the ammo box fills it all at once. The tester wants one round at a time, and loading is too fussy to line up (0:20:22 to 0:21:03).
- Passing the gun between hands is unreliable, probably because of the grabbable magazine (0:21:09).
- The target plate is shot straight through. No hit feedback; tracers might help (0:18:13 to 0:18:42). Aim is hard in general (0:55:18).
- The green "in reach" glow around the gun hand is distracting (0:52:55). The far-grab ring is a "hula hoop" that flops around; the tester suggests billboarding it to the camera (0:05:22, 0:09:32).
- Props are too small: matchbox, grenade, dressing, carrots (0:51:54, 0:25:25). The dressing doesn't read as a dressing (0:27:20).
- Throwing in VR is hard (0:28:55, 0:57:39).
- Chest-rig pockets: items clip sideways into them, and the front pops out when something's inside. Suggests real enclosed pockets or a belt abstraction (0:09:25, 0:35:45). The holster's leather flap pops out (0:35:26).

### Backpack: "the backpack doesn't work", the biggest QOL blocker
- It's too easy to pull out by accident (0:05:15). It flips out when the wrist rotates (0:02:47). It snaps down onto the player when they move while it's out (0:23:36).
- **Items inside move or vanish.** Bullets, the radio and the cassette disappeared (0:19:58, 0:23:46, 0:31:22).
- Opening from the top is awkward. The tester wants it to open from the side, facing you, with a visible cavity (0:02:54, 0:15:16).
- Items put in at an angle snap upright; they should keep their pose (0:14:59).
- The backpack sounds are good (0:02:38).

### Movement
- **During the yard the tester became slow and couldn't run** (0:21:51 to 0:26:38, still at 0:32:20). Their pack weighed 2.8 kg. The log shows no wound, weight or limp message around then (the dressing-station wound is later, at 0:27:21). Speed was normal again in Marrow Creek (0:41:20). **Cause unknown; check what the yard does to speed.**
- Doors: one jitters and then won't move, then flings open (0:22:58 to 0:23:16). Generally hard to open (0:49:46). Windows can't swing (0:22:27). Drawers don't open (0:25:04).
- Couldn't climb the dirigible's rope; the tester would love to ride it up (1:01:38 to 1:02:17).

### World, art, rendering
- Truck: sunk into the soil, mud flaps detached (0:12:57 `truck-sunk-into-soil`). The tester questions the whole truck concept and wants shelves and a cabinet for notes (0:12:50, 0:59:02).
- The horizon is abrupt: make the fog more gradual or add a textured fog wall (0:13:11).
- **Tree and leaf LOD pop is "ugly as sin"** (0:33:58). Leaves shrink away up close (0:43:10). On the Marrow Creek spawn, the table lamp and tree geometry flicker on a steady pulse (0:33:15).
- Spawn table: the stool floats, the crate clips into the ground, and the table has two legs in the air (0:33:38).
- The red building renders through the fog (0:41:28). A door hinge sits off the side of the building (0:41:50). There's a gap in the house wall (0:22:50). The window latch floats when the window is open (0:22:34).
- God rays: a visible asterisk or cross pattern on the floor, though there's "a lot of potential" (0:26:00).
- Ground texture seams where leaf-litter meets grass and along the path (0:30:20). The grass is the same copy-pasted texture across the field (1:00:37).
- A blue puddle (glow?) under the electric anomaly (0:28:29). The pushed button sticks out the back of its panel (0:30:47).
- Fog brightness changes when going indoors (0:23:27). Indoor rain sound plays outside under a roof (0:38:45, 0:49:38).
- Distant birds animate at a low frame rate (0:48:25). Birds need takeoff and landing animations and flapping sounds (0:41:35).
- The walker clips through a wall (0:57:47).
- Something is under a floor, or an invisible item can't be picked up (0:42:14).
- White-box cooking props on the table: the cooking idea can be sidelined (0:13:36, 1:06:41).
- Capsule-shaped pillow (0:24:17). Cassette player: z-fighting, and the UI covers its buttons (0:39:21, 0:40:08). The cassette music is "a bit on the nose" and spoils the vibe (0:40:41, 0:41:07).
- The map board is "super cool" but can't be taken along (0:37:56). The tester wants to take notes home too (0:58:52).
- Asset direction: the tester suggests a reference-photo research agent for buildings (0:25:52), terrain packs plus triplanar textures for cliffs (0:48:39 to 0:49:11), and better rigged models or asset packs. They call 3D model quality the project's biggest hurdle (0:44:29 to 0:45:01).

### Map, enemies, pacing
- The map is flat and open, and things are too far apart. The tester wants broken sight lines, a tighter layout, and quiet gaps between hot zones so one gunfight doesn't pull every robot in (0:45:50, 0:54:09 to 0:54:45, 1:01:02 to 1:01:28).
- Footsteps are scary in the right way ("unambiguously a good thing"), though maybe audible from too far (0:50:09 to 0:50:42, 0:58:24).
- Minelayer: cute, but no audio cue and unclear what it is. The glowing mines are too obvious. The tester rode on top of it (0:47:13 to 0:48:05, 0:53:43).
- Walker leg joints are too small to hit in VR: make weak points big and orange or yellow. The glowing orange circle reads as "shoot here" (1:04:22 to 1:04:58).
- The walker breaking apart is "utterly delightful". The tester tried to pick up its gun and fire it, and wants two-hand handling of the wreck (0:55:52 to 0:56:49). Wants to grab and push robot parts (1:03:47 to 1:04:10).
- A walker grappled the tester and pulled them back and forth, and its purpose was unclear (1:03:28).
- Damage needs a vignette, since the tester couldn't tell they were being shot (1:03:25). The haptics for heat damage were good (0:29:34).
- **The flying drone is "way over tuned", too fast to hit in VR** (1:05:24 to 1:05:43, killed the tester).
- Response squads add good time pressure (0:57:32).
- Death teleport back to the truck was confusing ("no idea how or why") (0:58:07).
- Loved: the dirigible ("one of the coolest things I've ever seen"; suggests searchlights), blinking lights in the fog, smokestacks, birds perching, scrounging junk, the inside of the houses, the smoke particles (0:59:16 to 1:00:33, 0:33:58, 0:16:20, 0:25:12).

## From the logs only

- `session-1793170661.log` at 06:57:42 says `persist: saving to ...career.ron`, then at 06:57:43 `no career at ...career.ron; starting fresh`, then `loaded save: 16 items`. The tester also said "new career, none of the material from the tutorial has carried over" (0:33:15). Worth checking the save and load ordering on the yard → valley handoff.
- Four crash reports from 22:57 to 23:21 local, before this recording: two `wgpu-hal vulkan/swapchain/native.rs:359` panics, one `bevy_render gpu_readback.rs:364 Failed to map buffer`, and one `bevy_pbr cluster/gpu.rs:1156 PoisonError`. `next_crash.zip` is included as found.
- The first ~35 s show `hand profile none` until the Index profile binds at 0:00:21. After that, every control logs as working.
- Lots of `Entity despawned` command warnings in both sessions (benign-looking, but noisy).
