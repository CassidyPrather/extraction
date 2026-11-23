# Downed dirigible and wrecked sorter (#105 salvage)

2x (1920x1080, the bottom 100 px cut), from standing height by day, nothing
in hand.

- `wreck_dirigible.png`: a dirigible down at 16 m, side on (camera turned
  90°: a sinking ship keeps its own heading). Its cells lie slack and low on
  the grass; a gas valve shows on the crown, seated on the drape
  (`dirigible_look::down_point` + 0.12 m), not floating at the cells' taut
  radius.
- `wreck_sorter.png`: the sorter down at 4 m. Its carriage tipped on the
  grass, the travel motor bolted on its top; its head seated against the
  carriage's front and its servo pack on the near flank
  (`salvage::seat_sorter_jaw`, Heavy 2854e961). Its arm lies out along the
  grass, and its grab, the wrist landing in reach, stays seated on the
  fallen forearm. Nothing lit: the strip is dark blue once the sorter is
  dead (`sorter_look::go_dark`), and the carriage rests on the grass
  rather than hovering over it.

    sheet: robots:dirigible!dead,0,16,0   camera 92.9,0,131.9,90,-10
           robots:sorter!dead,0,4,0       camera 92.9,0,131.9,0,-32
