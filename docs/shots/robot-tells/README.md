# A failing machine at fighting range

`close.jpg` is the one to look at: each machine cropped from its frame
and enlarged (1.2x at 10 m, 2x at 20 m, 2.7x at 30 m), intact | failing,
the same box for both (its centres picked by hand from the frames).
`sheet.jpg` is the whole frames: a row a kind and range (heavy, spotter, minelayer, sorter at
10, 20 and 30 m), each alone, from the same view and frame: intact |
failing, every part held at failing with nothing fired
(`EXTRACTION_HIT_SHOW=failing`, `scripts/tells_range.sh`). A spotter's row
looks up to its balloon, some 23 m up.

Darkening alone (`robots::wear`) read as muddy, so a failing machine now
has two more cues:

- **Soot.** It sends up a thread of dark soot from the top of its most
  nearly gone part (`fx::sputter`; the higher part on a tie), 1.4 a
  second, never smaller at the eye than 0.4° (`fx::wisp_floor`). It is
  dark because pale smoke on an overcast sky was a big puff no one saw.
  It uses the shared puff batch, so it adds no mesh, draw or shadow.
- **Guttering.** What's lit of it (its eye, its lamps) drops out for
  0.06-0.22 s at irregular gaps of 0.15-1.1 s and comes back no brighter
  (`wear::gutter`). It reads as broken, not as charging: a machine's
  warnings (a lock, an aim, a chirp) brighten, and this never does. A
  still catches a light out about one frame in five, so the sheet shows
  it only sometimes.

The measure, for each pair, is the pixels that changed by more than 12
levels in the machine's column, at the 1080-line view's scale, against
the 33 px of a 0.4° square:

| kind      | 10 m   | 20 m   | 30 m  |
|-----------|--------|--------|-------|
| heavy     | 43 224 | 10 140 | 6 088 |
| spotter   | 10 136 |  6 536 | 2 960 |
| minelayer |  4 580 |  1 312 | 1 472 |
| sorter    | 10 600 |  3 160 | 2 140 |

Every pair clears 33 px many times over. But these counts are total
change: darkening, soot, gutter, and whatever the machine's pose or sway
did between the two runs (the heavy's 10 m count is mostly a leg set
differently; a spotter turns on its tether). The sheet is the evidence;
the counts only show nothing is under the bar.

What the renders found and fixed: soot let out of a part's middle rose
inside a spotter's 2.8 m envelope and faded unseen, and on a tie its winch
in the grass sent it. Soot now leaves the top of the part, which also
lifts every machine's thread clear into the sky.

What reads, by the close sheet:

- **Heavy:** a column of soot and a darker body at all three ranges.
- **Spotter:** soot over the envelope at 20 and 30 m. At 10 m you are
  nearly under it, and the envelope hides most of its top.
- **Minelayer:** a column of soot at all three ranges. Its pale panels
  darken only a little: the soot is what tells.
- **Sorter:** a darkened arm at 10 m, and a darker gantry with its
  carriage band dimmed at 20 m. Its soot now leaves the carriage's
  side, 0.9 m out on the eye's side (`fx`, `SORTER_SOOT_OUT`), so it
  climbs beside the bridge and under a hall's ceiling, not into them.
  At 10 and 20 m that gives a small smudge beside the light cone and at
  the beam. The thread is at its 0.4° floor there and sits against the
  houses, not open sky, so the sorter's darker arm and band read before
  its soot. At 30 m (shot before the offset), soot over the carriage.

Weakest: the sorter's soot at 10-20 m (its darkening carries it), and a
spotter right overhead.

## Against a dark backdrop

`dark.jpg`: the minelayer at 10 m in front of the dark house, the case
the bar is about: of the two cues, at least one has to read.

- **Facing:** soot over the roof line; with the gutter held out
  (`EXTRACTION_GUTTER=out`, a still of a light that has dropped out),
  its striped hopper goes nearly dark.
- **Turned away:** its eye is hidden behind the hull, but the hopper's
  lit stripes face every way and gutter with it. The soot reads here
  against the mist at the house's edge.

The case where neither would read: a machine whose lit pieces all face
away, close in front of a backdrop tall enough to hide the whole climb of
its soot (a wall, a treeline). None of these four machines is lit on one
side only, but the soot alone can't promise that. The cheapest fix, not
made: an ember at the soot's root, a small orange puff on the batch
(`fx`'s `embers` material). That's one more puff batch while a failing
machine smokes in view, about 2 stereo draws.
