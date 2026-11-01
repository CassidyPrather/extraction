# Prop reading: a review procedure

`ITEM_READING.md`'s method applied to the world a newcomer walks past.
Map design's walk reviews (`docs/craft/walks/*/WALK.md`) name the props
the first walks pass at arm's length: sheds, woodpiles, fences, the Ash
Row houses, a car wreck. A prop passes when, at 5 m, it reads as the
thing it is, and at 1 m (in reach, or as cover) it holds up rather than
showing a blockout: one primitive repeated, even and clean.

## 1. Shoot it where it stands

In the game, at the walk's spot, in the valley's reference weather:

    scripts/prop_pose.py AT_X AT_Z ROT LX LZ [TX TZ [PITCH]]   # prints the --camera
    xvfb-run -s "-screen 0 2240x1260x24" target/fast/extraction \
        --screenshot out.png --frames 60 --size 2234x1257 \
        --camera X,0,Z,YAW,PITCH --map VALLEY --no-robots \
        [--weather drizzle] --hour 11

- Take the placement from the valley's map (`at`, `rot`). Stand at a
  point in the prefab's own frame (front -Z, doors per
  `map::prefab_entrances`), turned by `rot` the way
  `buildings::placement_transform` turns it (`Quat::from_rotation_y(-rot)`).
  `--camera`'s yaw turns clockwise from -Z, so the yaw that looks along
  (dx, dz) is `atan2(dx, -dz)`.
- 2234 x 1257 under a 2240 x 1260 screen: the headset's 14.3 px a degree
  (`ROBOT_READING.md`).
- **Distances**: 5 m (the whole prop, as you walk by) and 1 to 1.5 m
  (in reach: a wall you lean on, a pile you crouch behind). **Facings**:
  the way the walk passes it, and its end or back if the walk sees that.
- **Weather**: Marrow in drizzle (`--weather drizzle`), Cinder in its own
  ash haze (no `--weather`).

## 2. What fails

- **One primitive repeated**: ten identical cylinders, a row of the same
  box. Real stacks vary: girth, length, where each one ends.
- **No ends**: a stack of logs or pipes is read by its ends; a side view
  of even tubes is a striped wall.
- **Print colours on a whole object**: the flat, saturated colours made
  for labels (`Blue`, `Red`, `Yellow`) read as a toy on a car or a door.
  Big painted surfaces take a scan's relief and wear (`Paint`,
  `CarBlue`: `src/models.rs`).
- **Growing into the road**: a reworked prop must keep inside its
  footprint (`map::prefab_footprint`), which the map's lints and the
  clutter keep clear; a test holds the free-standing ones to it
  (`buildings::tests::solid_pieces_stay_in_the_footprint`).

## 3. Reviews

### The first walks' props (1 November)

`docs/shots/props/walk_props_before.jpg`: Marrow's field shed (front,
back, a wall at 1.5 m), a log pile and a garden fence; Cinder's Ash Row
(the damaged plank and brick houses, the burnt plank house, the shop), a
shed, a log pile and the blue wreck by the works yard.

- **Read well**: the houses (brick and plank, shutters, steps, their
  damage), the shop, the sheds (plank walls, a door, ivy) and the garden
  fence (pickets, brick posts). At 1.5 m their walls hold up.
- **The burnt plank house** is a near-black box at 5 m: it reads as burnt,
  and it's the darkest thing in the Row; its shape carries it. Left as is.
- **The log pile is the worst, and it's on both walks** (Marrow's verge,
  the woodpile in Ash Row, beside the works hall): ten identical 7 m
  tubes in three even rows. At 5 m a striped block; at arm's length a
  wall of bark.
- **The blue wreck is a toy**: the whole car in the flat print blue, one
  rust decal on the bonnet. At 5 m and in reach it's a clean plastic car.
- Not shot yet: the grey concrete block on Marrow's crest (s09).

### After the fixes (1 November)

`docs/shots/props/*_{before,after}.jpg`, the same cameras.

- **Log pile** (`buildings::log_pile`): stacked four, three, two, one,
  each log its own girth (0.16 to 0.24 m) and length (5.4 to 6.9 m),
  ends staggered; its sawn ends show, a ring of bark round each; stakes
  hold the bottom row. At 5 m a stack of felled trunks; end-on
  (`marrow_logs_end1_after.jpg`) a face of cut ends. Inside its
  footprint.
- **Wreck** (`art/vehicles/wrecks.py`, and its doors and bonnet in
  `art/items/wreck_parts.py`, so a panel prised off still matches):
  `CarBlue`, a faded grey-blue on the painted-metal scan's relief and
  wear. At 5 m a weathered car; in reach, chalky paint worn through at
  the edges.
- **The lorry wreck's footprint** was 6.4 m long for a 7.0 m body (the
  footprint test found it); now 7.0 m. The valleys' lints pass.
