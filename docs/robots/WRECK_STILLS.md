# Downed dirigible and wrecked sorter (#105 salvage)

2x (1920x1080, the bottom 100 px cut), from standing height by day, nothing
in hand.

- `wreck_dirigible.png`: a dirigible down at 16 m, side on (camera turned
  90°: a sinking ship keeps its own heading). Its cells lie slack and low on
  the grass; a gas valve shows on the crown, seated on the drape
  (`dirigible_look::down_point` + 0.12 m), not floating at the cells' taut
  radius.
- `wreck_sorter.png`: the sorter down at 3.5 m. Its carriage on the grass,
  the travel motor bolted on its top beside where the bridge ran, its arm's
  two pieces lying off its right. **Open:** the grab (`sorter_jaw`) doesn't
  show at the forearm's end in this render, and two orange-tipped jaw shapes
  sit on the carriage's top instead. The headless test seats one
  `sorter_jaw` on the forearm piece; the render differs, under review.

    sheet: robots:dirigible!dead,0,16,0   camera 92.9,0,131.9,90,-10
           robots:sorter!dead,0,3.5,0     camera 92.9,0,131.9,0,-35
