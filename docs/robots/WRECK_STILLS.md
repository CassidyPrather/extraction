# Downed dirigible and wrecked sorter (#105 salvage)

2x (1920x1080, the bottom 100 px cut), from standing height by day, nothing
in hand.

- `wreck_dirigible.png`: a dirigible down at 16 m, side on (camera turned
  90°: a sinking ship keeps its own heading). Its cells lie slack and low on
  the grass; a gas valve shows on the crown, seated on the drape
  (`dirigible_look::down_point` + 0.12 m), not floating at the cells' taut
  radius.
- `wreck_sorter.png`: the sorter down at 4 m. Its carriage on the grass,
  the travel motor bolted on its top beside where the bridge ran; its arm
  folded back under it as it came down one end first. Its grab, its wrist
  under the carriage out of sight and reach, tore off in the fall and lies
  clear off the carriage's right, its cut leads up (`sorter_jaw`,
  `salvage::torn_clear`). **Still open, Robot bodies':** the carriage
  hovers a little over the grass, and its cyan strip is still lit.

    sheet: robots:dirigible!dead,0,16,0   camera 92.9,0,131.9,90,-10
           robots:sorter!dead,0,4,0       camera 92.9,0,131.9,0,-32
