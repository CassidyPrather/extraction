# Who fixes what (playtest 2026-11-22)

Section numbers point into [`README.md`](README.md). The owner of the code fixes it; a
session that finds the cause lies in another's code says so to the coordinator, not
patches around it. Nothing merges to main until it passes the gate.

## P0: blockers (both modes nearly unplayable)

| # | What | Owner | Lead |
| - | - | - | - |
| 1a | Board and card text never draws, or draws late (yard SKIP / SETTINGS / START OVER, the arcade boards) | Legibility | No warning in the log. Text arriving late suggests the font or text texture and the warm-up/preload path; Performance helps if it is warm-up |
| 1b | Holster and pouches don't give things back: the pistol (yard soft-lock after `PutAway done`) and a magazine (arcade) | Hands | `src/interact/pockets.rs`; the pocket spheres (#93, 18da3bed) and the pack changes (6a8656d2) are recent. Also the pistol turns white and lies sideways in the holster; the tester wants it barrel down |
| 1c | The process keeps running after the window closes (`world dropped` at 17:35, Ctrl-C a minute later) | Performance | End of `logs/session-1795413721.log` |
| 1d / 4 | One press fires twice: NEW CAREER's "SURE? AGAIN" twice in 106 ms, the feedback board's rating twice in 78 ms, plus three ratings lit by one hover | Onboarding (camp), Arcade (feedback board) | One shared debounce for pressed cards, if they share code. NEW CAREER also relaunches the process and SteamVR drops to its loading screen: the tester asks for a loading transition |

## P1: the strongest complaints

| What | Owner |
| - | - |
| Bringing a rifle near the face snaps it to the middle of the view, so neither eye can use the sights (31:39-32:47) | Hands |
| Magazines don't seat unless placed exactly; "generous contacts" (19:27, 25:42), probably on every gun | Hands |
| Mosin: the trigger is dead until the rifle changes hands after the clip (29:06-30:46) | Hands |
| No damage feedback in the burner (15:48) | Legibility |
| The driver's first-time lines (Heard, Aimed, ComingToLook, LostYou, Anomaly, safety) fire mid-fight in the arcade: "Lev is so heavy-handed" | Arcade, with Onboarding |
| Range instructions sit on the sign, not the hand; the tester has to walk back to read them (12:33) | Onboarding |
| Yard CPU render world is 10 ms a frame (1% 20 ms), the bottleneck at about 50 fps; arcade about 70 fps | Performance |

## P2: the rest, by owner

- **Hands**: wrist seam; thumb through the pistol, index finger too far forward; left thumb a stub; Mosin grip; the pistol trigger may be backwards; catching a tossed gun to change grip should be more forgiving; slap-to-lock when A is held from a previous action; grenade throw still hard; far grab's target band is very narrow (10:10); the body spins when looking straight down (13:44); a magazine set on a windowsill fell through it (24:46; World models if it is the sill's collider).
- **Onboarding**: the yard's sightline is unbroken, so everything shows at once (break it up); nothing leads from one station's lamp to the next (a glow trail along the floor?); the first far grab catches a cleaning kit that fits no pocket (hide pocket hints until pockets are taught?); the plank in the road; respawn at the far end of the yard; the end radio can be picked up but should be the fixed transceiver; the tester asks for a flow chart of what a player can do in the yard; marks 1-3 three times in 2.7 s (does the both-sticks click re-fire?).
- **Arcade**: the X on the ground (22:47); a robot left blinded for long could self-destruct; the blinded walker wandered off the map.
- **Robot bodies / Heavy**: the haptics say a hand has the walker's plate or leg, but nothing comes away (22:38); the walker's dropped plate can't be reached or picked up (23:39); the joints and weak points are plain coloured spheres; the walker's voice is too quiet.
- **Robot minds** (once pushed): the second walker flanking looked deliberate, so make it so; it hung back out of range.
- **World models**: z-fighting on a fence post; the train car's roof vanishes when climbing onto it (Performance if it is culling); grass shows under a shack instead of its foundation; foliage pokes into cupboards (Map design if it is placement).
- **Map design**: stairs don't climb, the player hops (check every stair in the game); the ground's square tiles light and blend wrong, and the grass repeats visibly (`00638`, `00920`).
- **Models**: the backpack's bottom and flap are long and clunky, there are see-through holes on both sides, and the side pouches look decorative only.
- **Performance**: tree LOD pops; a trunk shows far but vanishes up close; narrow silhouettes with big flat triangles.
- **Anomalies**: too stark against the ground at a distance; a low-res patch under the lightning anomaly; the lightning looks blocky and too fast only at a distance (an LOD?).
- **Progression**: loading the old career put back 281 items that "fell out of the world", logged about 100 `Entity despawned` errors, and called chemlight / hand_flare unknown items. Removed items must keep old saves loading through aliases.

## Praised (keep)

The yard as a whole ("an improvement over every version so far"), the hands and their finger animations on the slide, the hip-holster glow, shooting out a walker's camera, distant anomalies, cupboards opening and closing, the yard's small shed and explorable corners, tutorial text when the pistol is held straight up.
