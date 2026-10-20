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
| `walker_limp.png` | A walker with a leg shot off: limping across on three, one foot up at a time, the dead leg dangling, the body sagging toward the gap | 0.2 s |
| `drone_wreck.png` | A drone killed in the air: it drops, sheds rotors and a panel, and smoulders where it lands | 0.2 s |
| `walker_wreck.png` | A walker killed in view: legs off at hip and knee, panels and turret thrown, the orb rolling to rest, scorched, smouldering | 0.17 s |

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
