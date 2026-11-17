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
