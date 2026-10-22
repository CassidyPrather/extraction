# Robot bodies: contact sheets

Frame sequences of robots moving and breaking, from `--gallery reel:WHAT`
(`src/robots/reel.rs`), tiled by `scripts/contact_sheet.py`. Time steps a
fixed 1/30 s a frame, so these are true intervals however slow the
software renderer is.

| Sheet | What | Interval |
| - | - | - |
| `walk.png` | A walker crossing the road: diagonal pairs stepping, the orb bobbing and leaning | 0.1 s |
| `drone_bank.png` | A drone flying a figure of eight: banking through the turns, nose leading | 0.2 s |
| `crawler_walk.png` | A crawler (the walker body at 0.6, for low ceilings underground) crossing: the same gait at its own scale | 0.13 s |
| `hound_run.png` | The hound's body (no mind yet, nothing spawns it): turning as it sets off and trotting across at nearly 4 m/s in diagonal pairs | 0.1 s |
| `sorter_at_work.png` | The Sorter (in the open for the camera; it lives in the Depot's sorting hall): the bridge running along its rails, the carriage across it, the arm reaching down and shifting orange totes, the festoon cable swinging | 1 s |
| `crawler_zap.png` | A crawler's spark gun: the blue charge glow and crackle for half a second, then the arc | 0.1 s |
| `shell_marks.png` | Rounds into a walker's front: its low armour plate takes them (sparks, marks) until it comes off, then the core behind scorches; the eye stutters when it's hit | 0.33 s |
| `eyes_by_hour.png` | Day, dusk and night (`--hour`): three walkers calm, suspicious and seeing you (left to right) and a drone; eyes glow by mood, more so the darker it is, and shells dim their own glow with the light | still |
| `eyes_night_glow.png` | Night: the eye glow is an HDR emissive now, and opens up after dark (core about 40 at rest), so calm, suspicious and seeing read as red, hot and near-white points. `reel:eyesfar` puts walkers at 30, 60 and 80 m for range checks | still |
| `eyes_halo_by_hour.png` | Day, dusk, night with `world::halos::Halo` on each eye (calm, suspicious, seeing you): after dark the shell gives up its own glow and the eye's halo is the tell | still |
| `eyes_halo_range.png` | `reel:eyesfar` at night, clear (top) and fog (bottom), zoomed: calm walkers at 30, 60 and 80 m and one that sees you | still |
| `feet_on_rubble.png` | `reel:rubble`: a walker crosses a heap of broken blocks on Destruction's debris layer; each foot comes down on a block's top, not through it | sheet |
| `knockdown.png` | `reel:knockdown`: one heavy round into a walker's side knocks it down, tipped the way it was hit, legs splayed; it lies 1.6 s, then plants its feet and rises on them | sheet |
| `crawl.png` | `reel:crawl`: two legs shot off, a walker no longer falls apart; belly down, it drags itself on the two it has left, the dead ones trailing | sheet |
| `damaged.png` | `reel:damaged` by day and dusk: plates on their last bolt hang from a corner and swing as it walks, the core at a fifth smokes and sparks, and its eye gutters (dusk row) | sheet |
| `rime_frost.png` | Rime Fell's weathering: shells iced cold blue down their sides and under, snow white on their tops, rime on the dark metal's tops (vertex colours, no new draws) | still |
| `rime_snow_walk.png` | Rime Fell's col in falling snow: an iced walker crossing lying snow (on the steep, icy ground beyond, its feet creep and it stumbles: `feet_slip_on_ice_slopes`) | sheet |
| `drone_lamps.png` | `reel:drone` at night in a downpour (top) and fog (bottom): after dark a drone hunts with its searchlight lit, down and ahead, a shaft through the rain and fog (`world::beams`) and a pool on the ground | sheet |
| `snow_prints.png` | Rime Fell's col: a walker leaves a trail of foot pads in the lying snow (`world::snow::Tracks`, the machines' own ring) | sheet |
| `damage_at_range.png` | `reel:tellsbefore` (top) and `reel:tells` (bottom) at dusk, 24-36 m: whole, then a walker on a shot leg (limping across), one with its eye shot dark, one with its core failing (scorched, smoking, guttering), and a drone with a rotor hit (listing, sinking, trailing smoke); wound effects draw larger with distance (`fx::at_range`) | sheet |
| `limp_and_eye_30m.png` | `reel:dying,24,4` at 1 pm, clearing, 28-32 m (crop): top two rows, the right walker's eye shot out (dark, its last red flare at the socket, then out for good; frames 2-9); bottom two, the left one on a leg at 0.3 limping across, hitching and listing onto its bad side as weight comes on it (frames 14-21) | 0.13 s |
| `salvage_seated.png` | `reel:salvage,16,10` at 1 pm, a walker brought down 2.4 m off (crop): what came through is seated on the wreck to be taken off by hand (`robots::salvage`): the servo packs at its hips (pale, on dark bolts), the cell on top (vented box), the gun pod where the turret was (left); the cables and bolts go mid-reel as the tools would take them. The hand work itself is in the headset tests (`vr_harness::salvage`) | 0.33 s |
| `pylon_cable.png` | `reel:pylon,2,10` at 1 pm (crop): a relay pylon's cable (Robot minds' `network::PylonCable`, cut with bolt cutters to silence it) down the cabinet's side into its junction box | still |
| `weathering.png` | The same walker in the training yard (clean), Marrow Creek (rust streaks, moss on top) and Cinder Works (sooted, heat-blued metal) | still |
| `walker_limp.png` | A walker with a leg shot off: limping across on three, one foot up at a time, the dead leg dangling, the body sagging toward the gap | 0.2 s |
| `drone_wreck.png` | A drone killed in the air: a rotor flies off, it corkscrews down on its fading lift trailing smoke, hits in a burst of sparks, sheds its panel and gun pod, and smoulders | 0.17 s |
| `walker_wreck.png` | A walker killed in view: visor, turret and panels thrown, its legs (jointed at hip and knee) skid out and fold as it drops, two tear off at the hip as it lands, scorched and smouldering | 0.13 s |

To make one again (about 2 s a frame under lavapipe):

```bash
cargo build --profile fast
WGPU_BACKEND=vulkan xvfb-run -a -s "-screen 0 1600x900x24" target/fast/extraction \
    --screenshot /tmp/reel/wreck.png --frames 99999 --camera 40,0,262,0,-12 \
    --no-robots --data-dir /tmp/reel-data --gallery reel:wreck,16,5 --size 960x540
python3 scripts/contact_sheet.py docs/shots/robot-bodies/walker_wreck.png 4 \
    /tmp/reel/wreck_[0-9]*.png --crop 230,150,500,300 --scale 0.6
```

Other reels: `turn`, `limp` (a leg shot off), `hurt` (a leg at a fifth),
`drone`, `dronewreck`.
