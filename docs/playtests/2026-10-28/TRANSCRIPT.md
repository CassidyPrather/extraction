# The 28 October playtest, spoken: classes to fix

Source: `transcript/` (narrated video, 1 h 07 min, build 2798e450). It's the same session as the logs in `ANALYSIS.md`.
- `transcript/README.md` lists every finding with its video time.
- `transcript/timeline.md` puts speech, log events and screenshots on one clock.
- `transcript/screenshots/HMMSS_label.jpg` are frames from the video, named by video time.

This file groups those findings into **classes**. A brief fixes a class (every instance of it, with a test that would catch the next one), not the one instance the tester happened to hit.

**Overnight fixes.** The night of 28 October fixed ANALYSIS.md's top 15 from the logs alone. Several transcript items overlap with those. **Every owner checks their items on current main first** and records any that are already fixed, rather than redoing them.

## The designer's direction (1:05:50)

> Focus really hard on models and map work … and especially onboarding, super super important … making the onboarding fun and not just following a list of instructions, and presenting you with one thing at a time. One or two things at a time.

**What the tester says works:**
- the walker's footsteps ("scary as hell, which is a good thing");
- the walker falling apart ("utterly delightful");
- the dirigible ("one of the coolest things I've ever seen");
- blinking lights through fog, the smokestacks, perching birds;
- scrounging junk in houses, the inside of houses, smoke particles;
- backpack sounds, gun sounds, hand poses ("definitely on the right track");
- haptics for heat damage, the response squads' time pressure.

**What to set aside:**
- **cooking**: "can very much be sidelined";
- **the truck as a concept**: "let's not spend too much time on this truck concept for now";
- **features** in general: the tester wants the basics to work first.

## Tier 0: blockers. "The backpack doesn't work"

The tester cut the session short because of these (0:53:10): "the fact that the backpack doesn't work and a couple of the basic QOL issues are making this really rough for me to want to have an extended play test".

| Class | Instances (video time) | Owner |
|---|---|---|
| **B1. Stowed items move or vanish.** Something in a pack, pocket or crate must stay exactly where it was put, with the pose it was put in, through every move, pickup, snap and scene change. | Bullets, radio and cassette gone from the pack (0:19:58, 0:23:46, 0:31:22). Items snap upright (0:14:59). Dropped above the crate, they fall through it (0:10:53). A mag falls under the crate where it can't be seen (0:34:55). | Soak (QA) + Hands |
| **B2. A grab takes the wrong thing.** A grab picks the thing the hand is reaching for, never a part seated inside it. | Grabbing the pistol takes its magazine, 4 times (0:07:38, 0:10:04, 0:21:15, 0:34:43). Passing the gun between hands fails (0:21:09). | Weapon feel |
| **B3. A grab doesn't land in the hand.** | Far-grab stops about a foot short (0:07:57, 0:14:47). | Hands |
| **B4. The body moves the player.** Head, hands or a held pack must never move the play space or the rig root. | Leaning over a table pushes the play space back: "never, ever do that" (0:06:33). The pack flips on a wrist turn (0:02:47) and snaps onto the player on a step (0:23:36). It's too easy to pull out (0:05:15). | Hands |
| **B5. Unexplained speed loss.** | Slow, unable to run, from 0:21:51 to 0:32:20. 2.8 kg carried, no wound logged. Normal again in Marrow Creek. | Progression |
| **B6. Career lost between the yard and the valley.** | The log says `saving to career.ron`, then 1 s later `no career at … starting fresh`. "None of the material from the tutorial has carried over" (0:33:15). | Progression |

## Tier 1: onboarding (designer's #1)

| Class | Instances | Owner |
|---|---|---|
| **O1. Too much at once.** The yard is a list of instructions. It should be one or two things at a time, and fun. | The truck's boards are a wall of text (0:01:03). "Let the player have some fun with the onboarding" (1:06:27). A fixed sign blocks the crate (0:06:00). There's no title screen, settings or menu button (0:06:28). | Onboarding |
| **O2. Instructions that name what the player can't identify.** | "The radio": which one? (0:08:45). "The lit lamp" means leave the truck; stuck about 2 min (0:10:28). "Safety off" without how (0:16:44). The wrong gun and ammo first, a second gun (0:19:31). A brown block on the board (0:19:32). "Throw a part in": which target? (0:27:40). "Push the button" should come first (0:30:43). The dressing doesn't read as one (0:27:20). | Onboarding |
| **O3. Skills needed before they're taught.** | Pockets, while both hands are full (0:09:03). | Onboarding |
| **O4. Transitions with no feedback.** | The SteamVR loading void, thought to have failed (0:32:42). The truck pops in (0:31:45). The death teleport: "no idea how or why" (0:58:07). | Performance (load), Progression (death, truck) |

## Tier 1: models (designer's #2). Hands first

| Class | Instances | Owner |
|---|---|---|
| **M1. Show only what the body can back up.** Hide what IK can't place. | "Get rid of the arm model entirely … hide the fact that the IK is garbage" (0:04:51). Remove the chest straps (0:36:16). Sleeves are back-face culled (0:05:03). Hips stay put, so the body looks tilted (0:01:34). The face clips when bending, and the floor can't be seen (0:12:17, 0:34:50). The ghost hand separates (0:12:08). | Hands |
| **M2. The hands are the most-seen model.** | "Very high amount of detail … more than anything else" (0:03:52). Thumb through the pistol (0:08:14). No two-hand grip pose (0:17:27, 0:55:01). | Hands |
| **M3. Model correctness.** Seams, gaps, z-fighting, floating or detached parts, parts through parts, things sunk in the ground or hanging in the air. | Mag through the gun (0:08:31). Z-fight on the mag top (0:20:16). House wall gap (0:22:50, 0:41:47). Window latch floats (0:22:34). Door hinge off the building (0:41:50). Truck sunk, mud flaps detached (0:12:57). Stool floats, table legs in the air (0:33:38). Button out of its panel's back (0:30:47). Cassette z-fight (0:39:21). Holster flap pops (0:35:26). Pocket fronts pop (0:35:45). Battery through the flashlight (0:11:13). Model seam (0:44:00). Walker through a wall (0:57:47). Dirigible basket buried (1:01:50). Broken crate's floating "googly eyes" (0:52:37). | Model correctness (new) |
| **M4. Props at the wrong scale or too crude.** | Carrots, matchbox, grenade and dressing too small (0:25:25, 0:51:54). Capsule pillow (0:24:17). Handsaw (0:42:31). The white-box cooking table (0:13:36, so remove it). | Model correctness (new) |
| **M5. Foliage LOD.** | Leaf pop "ugly as sin" (0:33:58). Leaves shrink when close (0:43:10). Lamp and trees flicker on a steady pulse at the Marrow spawn (0:33:15). | Visual design |
| **M6. Fog and sky consistency.** | The red building ignores fog (0:41:28). The horizon is abrupt (0:13:11). Fog brightness changes indoors (0:23:27). God-ray asterisk on the floor (0:26:00). Blue puddle under the electric anomaly (0:28:29). | Visual design (+ Anomalies for the puddle) |
| **M7. Ground texture.** | Seams where leaf-litter meets grass and at path edges (0:30:20). One grass texture copy-pasted across the field (1:00:37). Triplanar cliffs and terrain kits suggested (0:48:39). | Visual design |

## Tier 1: map (designer's #2)

| Class | Instances | Owner |
|---|---|---|
| **P1. Flat and open, things too far apart.** | "A couple of points in an otherwise empty plane" (0:45:50). Too far apart (0:54:09). Break up sight lines. Enemies see you from too far (0:54:22). Hot zones separated by quiet gaps, so one fight doesn't pull every robot (1:01:02). | Map design |

## Tier 2: readability of fights and controls

| Class | Instances | Owner |
|---|---|---|
| **F1. Is a hit landing?** | Plates shot straight through, no feedback (0:18:13). Can't tell when being shot: wants a damage vignette (1:03:25). | Weapon feel (plates), Legibility (vignette) |
| **F2. Targets VR can hit.** | Walker leg joints too small; weak points should be big and orange (1:04:22). "The drone is way over tuned … too fast for somebody in VR to possibly have a shot" (1:05:24). | Robot bodies (weak points), Robot minds (drone) |
| **F3. What is this machine doing to me?** | Minelayer: no audio cue, unclear role, glowing mines too obvious (0:47:13, 0:53:43). Grappler pulls the player back and forth with no clear goal (1:03:28). | Robot minds + Audio |
| **F4. Diegetic UI hidden by its own housing.** | The watch body renders over its screen (0:26:48). The screen vanishes as the wrist turns (0:22:08). UI covers the cassette buttons (0:40:08). Text z-fights on the truck boards (0:01:00). The note has a pure white background (0:38:57). | Legibility |
| **F5. Physical controls that don't move.** | A door jitters, sticks, then flings open (0:22:58). Doors hard to open generally (0:49:46). Windows can't swing (0:22:27). Drawers don't open (0:25:04). | Hands (hand-path) |
| **F6. Grab affordances that distract.** | The far-grab ring is a floppy "hula hoop"; billboard it (0:05:22, 0:09:32). A green in-reach glow around the gun hand (0:52:55, 0:02:30). | Hands |
| **F7. Mag handling.** | Holding release could hold the mag half-out (0:19:01). The ammo box fills a mag at once; wants one round at a time and more generous alignment (0:20:22). | Weapon feel |

## Tier 3: audio

| Class | Instances | Owner |
|---|---|---|
| **A1. Indoor or outdoor rule.** | Indoor rain plays outside under a roof (0:38:45, 0:49:38). | Audio |
| **A2. Tone.** | Cassette music "on the nose", spoils the vibe (0:40:41, 0:41:07). Footsteps maybe heard from too far (0:58:24). Birds want wing sounds (0:41:35). | Audio |

## Not this round

These are features the tester liked the idea of. They're in the notes, but nobody is building them now:
- climbing the dirigible's rope;
- dirigible searchlights;
- firing a walker's gun from its wreck;
- grabbing and pushing robot parts;
- a handsaw that saws;
- one-round-at-a-time mixed loading (beyond F7's generosity);
- bird takeoff and landing animation;
- truck drive-in animation.

## Logs only

- **Four crashes before the recording** (22:57–23:21 local), owned by Performance:
  - 2 × `wgpu-hal vulkan/swapchain/native.rs:359`;
  - `gpu_readback.rs:364 Failed to map buffer`;
  - `cluster/gpu.rs:1156 PoisonError`.
- `hand profile none` for the first 21 s, until the Index profile binds.
- "Entity despawned" warnings: fixed overnight (ANALYSIS #11).
