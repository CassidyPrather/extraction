# Tower stairs, before and after

Onboarding's two-lane switchback (`world::buildings::tower_legs`, 058ce693):
the ranger and survey towers' flights alternate between lanes at x 2.5 and
x 3.45, so each has headroom over the one below and the cab is reachable.
Map design's side of it: the outer lane is a wall past the legs' footprint
(`map::prefab_walls`, 63e27b40).

Marrow, the valley's reference weather, 2560x1440 (`towers.stills`; before
from the binary at f0e5aa60, after at 63e27b40, same maps):

- `marrow_drop_*`: the arrival frame (rot 53); the ranger tower's lamp on
  the right reads the same.
- `marrow_ranger_stairs_*`: the ranger tower (90.8, 144.1) from 24 m on its
  stair side.
- `marrow_survey_stairs_*`: the survey tower (35.3, 14.6) from 28 m on its
  stair side.

The silhouette and the landmark read (cab, lamp, mast) are unchanged; the
stair is a little wider on its own side and zigzags in two lanes.
