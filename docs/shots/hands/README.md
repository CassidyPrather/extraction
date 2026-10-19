# Gloves

Before/after (`0_*`: the old box gloves left, the modelled ones right) and
the gloves first-person, from `--gallery fp:WHAT[,ZOOM[,TURN]]` (see
`dev::first_person_hands`): open, from the back, pointing at the yard's
push button (the index tip on `poke::FINGERTIP`), on the PM's grip from
behind and from the left, on an AK (grip and handguard), and an AK
magazine in the left fist. Rendered without the texture scans (flat
colours), so the canvas weave on the fabric doesn't show here.

```bash
scripts/shot.sh out.png 40,19.2,262,0,-25 30 --no-robots --gallery fp:pm_pistol,1.6,90
```

`6_trigger_pulled.jpg`: PM, AK, Mosin, TOZ from the left with the trigger
pulled (`fp:<gun>,2.2,90,1`); the index curls to each gun's own trigger
(`render::props::gun_index_curl`). On the Mosin and TOZ the trigger sits
5–6 cm below the index's line, which no curl reaches.
`6b_mosin_toz_grip_lowered_rejected.jpg`: Mosin and TOZ before (left) and
with their layout grip points lowered ~4 cm so the trigger lines up
(right): the fist then hangs under the stock's wrist. Not kept.
