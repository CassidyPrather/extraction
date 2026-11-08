# A machine answers a hit with its body

`strip.jpg`: a heavy at 15 m, as the player sees it. At rest, then held at
the peak of a PM (Makarov) round's shove, then a Mosin's
(`EXTRACTION_ROCK_SHOW=<momentum>`, the same axis and swing as a real
hit). In play it settles within about a second.

Struck, a heavy rocks, the plough jolts, and a sentry's head snaps
(`robots::rock`). How far depends on the round's momentum where it
lands, on a log scale (Weapon feel's numbers), so the guns differ rather
than just scale. It falls off with range, and buckshot's pellets are one
shove. A heavy's peak:

| Round | Momentum (kg m/s) | Rock |
|---|---|---|
| .22 | 0.9 | about 0.2° |
| PM | 1.9 | about 1.3° |
| SKS | 5.8 | about 2.8° |
| Mosin | 8.3 | about 3.5° |
| Twelve-bore | 12.6 | about 4.2° |

The shove tips it away from the round and rolls it about the round's
line, the struck side dipping (`rock::ROLL`). Tipped straight away, it
hardly moved in the shooter's view: the shove was all depth. Against
rest, in the heavy's box, the PM's frame changes 1548 px and the Mosin's
2672 (1080-line scale).

Only what's drawn moves: meshes' world transforms, after propagation.
Colliders, aim and every `Transform` stay as they were. A heavy's knees
ride its legs, and its hips and shoulder follow the hull.

A sentry struck by a round that does harm is also knocked off-line for
0.35 s (`extraction_core::sentry::KNOCKED`). Its next round waits, it
keeps its lock and its warning burst, and it's knocked at most once a
burst cycle, so fire slows it but never stops it.

`robots::hit_gauge` checks it for every part of every machine: a machine
that rocks has to turn at least 0.1° within 100 ms.
