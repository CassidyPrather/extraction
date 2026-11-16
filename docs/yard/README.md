# The training yard in bays (#89): Map design's part

The plan is `docs/YARD_SHAPE.md` (Onboarding). This is the map's half of
it, ready to land with Onboarding's gates code.

## What's in main now

- **Prefabs** (`src/world/yard.rs`, footprints in `extraction_core::map`):
  - `yard_fence`, a 6 m run, and `yard_fence_2`, a 2 m filler. Both are
    timber posts every 2 m with three rails, 1.3 m high, solid all through.
    Nothing in them breaks, so the range lane's fence stops rounds and
    stays up. No ivy grows on them.
  - `yard_gate`: two posts with a 3 m leaf hung on the west post, opening
    north, filling a 4 m gap in the line.
- **The gate interface:**
  - The leaf is a plain `buildings::Door` carrying
    `yard::YardGate { index }`, numbered from its map tag `gate_N`.
  - Set `Door.open` to open or shut it. `Door.angle` goes 0 (shut) to 1
    (open) over the swing.
  - Hands can't move it: the press and grip components are taken off the
    leaf, and `physics_hands::push_doors` skips it.
  - All gates start shut.
- **Map rules:**
  - `map::is_fence` covers both fence pieces for the lint and footing.
  - The nav grid blocks the fence runs on either side of a gate, never
    the gate itself, since the static grid can't know whether the leaf is
    open.
  - The overlap lint lets the yard's own pieces meet.

## What lands with Onboarding's code

`training_yard_bays.ron` is the yard map in bays. Copy it over
`assets/maps/training_yard.ron` in the commit that brings in the gates
system. With today's onboarding the two VR yard tests fail on it:
`station_going_home` and `the_driver_names_the_next_station_once`
expect the truck at the start. The core test `training_yard_bays_map_lint`
holds it to the yard lint (reach, footing, overlap) until it lands.

| Bay | Station(s) | x | z |
|---|---|---|---|
| 1 | START, GETTING ABOUT | -34..10 | 84..100 |
| 2 | OUT OF REACH | -34..10 | 74..84 |
| 3 | THE RANGE, RUN IT DRY | -24..10, and the lane -24..-6 | 38..74, lane -24..38 |
| 4 | THE HOUSE | -6..36 | 12..38 |
| 5 | FIRST AID (the shed at (24, -12)) | -6..30 | -16..12 |
| 6 | THE SHIMMER | -6..20 | -48..-16 |
| 7 | THE EXTRACT | -10..14 | -82..-48 |

- **Gates:**
  - Gates 1 to 6 sit on the track at z 84, 74, 38, 12, -16 and -48, all
    centred on x 0.
  - A shut leaf spans x -1.5..1.5.
  - Gate *k* opens when bay *k*'s station is done or skipped.
- **Start and truck:**
  - Insertion is at (0, 96), facing north.
  - TruckStop is at (0, -74) in bay 7, by the North post (0, -72), its
    log pile at (5.6, -74). The truck isn't shown until the extract.
- **Also changed in the map:**
  - The Yard track runs straight up x 0 and is 3 m wide, the width of a
    gate's leaf.
  - The quarantine barrier and its apron are gone; gate 2 replaces them.
  - The bank behind the targets is gone; the lane's end fence stops
    rounds now.
  - The rim is 18 m wide, so the bays stand on level ground.
  - The machines sign moved into bay 6.
- **Regenerating:** the fence runs come from `scripts/yard_fences.py`.

## Pictures (2560×1440, standing height)

- `bay1_start.png`: the first frame from the start, facing north. Gate 1
  is shut ahead, and the next bay shows over the fence.
- `gate1_shut.png`: gate 1, shut across the track.
- `gate2_open.png`: gate 2 standing open, swung north. For this capture
  only, it was opened by a temporary edit, not committed.

The leaf draws with the generic door's grey panel. A timber leaf would be
a Models item.
