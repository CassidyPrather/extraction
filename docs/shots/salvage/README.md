# A wreck half stripped (#105's proposal)

`half.png`, 2x (1920x1080): a walker wreck 3.2 m off, by day, with half of what
`robots::salvage` seats on it taken off and laid before it:

- taken: its power cell (by hand, once it has stopped arcing), a knee
  actuator (spanner, 3 bolts) and an armour plate (crowbar);
- still on it: the gun pod on its two cables (bolt cutters), the other knee
  actuator and two plates.

Made with the dev-only gallery word `walker!half` (`dev.rs`, `strip_half`):

    data=$(mktemp -d); EXTRACTION_WARMUP=0 WGPU_BACKEND=vulkan \
      xvfb-run -a -s "-screen 0 1920x1080x24" ./target/fast/extraction \
      --map marrow_creek --data-dir "$data" --new-game --no-robots \
      --weather clearing --hour day --camera 92.9,0,131.9,0,-24 \
      --gallery robots:walker!half,0,3.2 \
      --screenshot half.png --frames 700 --size 1920x1080

# A walker's parts, side by side

`walker_parts.png`: everything a walker wreck gives up, laid in a row 1.5 m
off by day, cropped from a 1920x1080 frame at its own pixels: its eye (a
lens with a red iris), its power cell (a chunky brick with a deep orange
band), a knee actuator (a long banded ram), an armour plate (flat, scorched)
and its gun pod. Under each, the hand or tool it comes off with. Each reads
as its own thing by shape and colour before you pick it up (#105).

    data=$(mktemp -d); EXTRACTION_WARMUP=0 WGPU_BACKEND=vulkan \
      xvfb-run -a -s "-screen 0 1920x1080x24" ./target/fast/extraction \
      --map marrow_creek --data-dir "$data" --new-game --no-robots \
      --weather clearing --hour day --camera 92.9,0,131.9,0,-30 \
      --gallery items:walker_eye+robot_cell+walker_leg_actuator+robot_plate+robot_gunpod,0,1.5,0.7 \
      --screenshot row.png --frames 300 --size 1920x1080

# The walker family, wrecked (playtest sheet)

Each wreck seen from standing height by day, 2x (1920x1080, the bottom 100 px
cut off where the player's own helmet shows), with what `robots::salvage`
seats on it still on, and one of its parts held up in the left hand:

- `wreck_walker.png`: a walker at 3.2 m, its eye in hand. Its knee actuators
  sit on top of its fallen legs, bolts facing up for the spanner.
- `wreck_grappler.png`: a grappler at 4.2 m, its cell in hand. The yellow
  line drum and the harpoon launcher are still seated on its back.
- `wreck_crawler.png`: a crawler at 2.6 m, turned to face you, a leg servo
  in hand.
- `wreck_hound.png`: a hound at 2.8 m (it fell toward you, lower left), a
  hip servo in hand (half behind the hand). Its cell, with the orange band,
  sits on its back, and its head lies loose at the bottom left.

Each is one gallery word, `WHO!dead,TURN,DIST,0,hold=ITEM`. The robot is
killed 90 frames in, after it is posed, so its pieces fall and are seated
the way they are in a raid:

    data=$(mktemp -d); EXTRACTION_WARMUP=0 WGPU_BACKEND=vulkan \
      xvfb-run -a -s "-screen 0 1920x1080x24" ./target/fast/extraction \
      --map marrow_creek --data-dir "$data" --new-game --no-robots \
      --weather clearing --hour day --camera 92.9,0,131.9,0,-24 \
      --gallery "robots:walker!dead,0,3.2,0,hold=walker_eye+up" \
      --screenshot wreck_walker.png --frames 700 --size 1920x1080

The others: `grappler!dead,0,4.2,0,hold=robot_cell`,
`crawler!dead,180,2.6,0,hold=crawler_leg_servo`,
`hound!dead,0,2.8,0,hold=hound_hip_servo`.
