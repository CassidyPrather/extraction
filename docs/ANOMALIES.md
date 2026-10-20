# Anomalies

What lies in the Zone, how to find it before it finds you, and what it
looks like. Code: `src/anomaly/`, rules in
`crates/extraction_core/src/anomaly.rs`, shaders
`assets/shaders/anomaly_*.wgsl`.

## The kinds

| Kind | Where | What it does | Tell | Grows |
| - | - | - | - | - |
| Lift | the Surge scatters them | floats and swirls anything loose; the longer something's in it, the sooner it pops and flings it all out | circling leaves; as it winds up, leaves and junk slide in along the ground from twice its radius | featherstone |
| Burner | the Surge | burns whoever stands in it, cooks robot parts | coal-red glow, embers, smoke, heat shimmer over the patch; a flare when something steps in | hearth |
| Snare | the Surge | discharges into whatever touches it (robots too), then recharges; metal in a hand draws the arc from 0.8 m further off | blue arcs and sparks, a crackling light; arcs reach out to what they strike | aegis knot |
| Well | standing, one a valley | drags loose things and walkers toward a heart 1.3 m up that crushes (30/s); bends rounds that pass close | a lens in the air that turns the world over, grit spiralling in, a violet throb | lodestone |
| Mirror | standing, one a valley | blinks whatever crosses it out the far side, radius + 3 m on, still moving; 1.5 s to fold back up | almost nothing: a shimmer at waist height, a glint now and then | mirror shard |

Wells and mirrors don't move between cycles: the maps place them
(`MarkerKind::Anomaly`), so they become landmarks you learn. The director
can still bloom any kind in your way.

### Wells and bullets

The lens is `K / (d² + s²)` toward the heart, summed along each tick's
segment (a rifle round crosses a well's reach in a tick or two). A rifle
round a metre off the heart turns a few degrees; a pistol round, at half
the speed, about four times as much. Shooting across a well at a robot
behind it misses. The pull reaches three radii; a walker can still walk
out from twice the radius, not from the heart.

### Mirrors

It sends you out the far side of where you came in. If that spot is in
water, a building or another anomaly's heart, it swings round a sixth of a
turn at a time until it finds open ground. Its shard hangs in the fold
unmoved: throw something in, then reach in while it's still closing.

## The Surge

Through the five minutes' warning every anomaly stirs: it swells (up to
30% wider), charges and recharges up to three times as fast, burns
hotter, and flares on its own beat, harder as the Surge nears. At 240,
170, 110, 60 and 25 seconds out a new anomaly blooms round whoever is
still out: 10 to 25 m off, to the side of the way they're heading (50 to
130 degrees off it), never on or by a road, and clear of the others. Each
spends its first four seconds forming, lit up and harmless, so you see it
open before it can hurt you. Keep to the road and you get through.
Rules: `extraction_core::anomaly::surge_*`; game side `src/anomaly/surge.rs`.

## Finding them

- **Throw something in.** A bolt (junk, found in tool boxes and
  workshops), a nut, a tin. The moment a loose thing first crosses into an
  anomaly it shows itself for six seconds: its light, particles and haze
  surge, whatever kind it is.
- **The detector.** Carried anywhere on you it clicks for the nearest
  anomaly's edge: about 1.5 s apart at 25 m, a rattle at the edge.
  Held, it listens the way its antenna points (one behind you sounds half
  as far again), so sweep it about. Its lamp blinks with each click.

## Artifacts: what they give, what they cost, how to get them

Every stone does something for you while you carry it, and costs you
something physical; nothing shows a number. Each draws the Zone's
attention too (the hum). Sealed in a lead-lined case it does neither:
carry it safe, or carry it live.

| Stone | From | Gives | Costs | Getting it out |
| - | - | - | - | - |
| Featherstone | lift | takes 6 kg off your load | it wants home: lifts within 15 m wind up toward it and pop | it rides the column out of reach; throw something in, stand clear, catch it as the pop throws it |
| Hearth | burner | power gadgets plugged into it never run down | you run hot: your wind comes back at half the pace | it's hot in the coals and burns the hand that holds it; shoot or flick it out, let it cool 20 s or drop it in water |
| Aegis knot | snare | the first hit glances, then 90 s to knot back up | it holds a charge: wet (soaked ground, wading) it shocks you every 8–15 s, with a crack robots hear | the snare ignores its own stone but strikes a hand reaching in; ground it with thrown metal and reach in while it recharges |
| Lodestone | well | rounds fired at you curve aside | five kilograms more on your back | it hangs at the crushing heart; dash in and out |
| Mirror shard | mirror | use it to step up to 4 m ahead, once a minute | every 20–40 s its late reflection glints, and robots within 35 m hear where you are | it hangs in the fold; throw something through and reach in while it closes (1.5 s) |
| Glass eye | rare, any field | held to your eye, walls go thin | strong attention | as its anomaly's stone |
| Stormglass | only the Surge's own blooms (about one a Surge) | the Surge passes over you as if you were under a roof | the whole Zone knows where you are (attention 3/s) | as its bloom's kind, once it has finished forming |

Rules and numbers: `extraction_core::anomaly` (costs, harvesting);
game side `src/anomaly/costs.rs`, `harvest.rs`, `artifacts.rs`.
Scenarios: `harvest_snare`, `harvest_lift`, `harvest_burner`,
`harvest_well`, `harvest_mirror` (in `--scenario all`).

**Harvest band** (`scripts/harvest_band.py <sim dir>`): over a batch of
`hunter` sim raids, at most 20% of deliberate dives into anomalies may
end in death, and at least 30% must bring a stone out. Last measured (9
hunter raids on Marrow Creek, 10 minutes each): 20 dives, 1 death in a
snare (5%), 12 stones (60%).

## Looks, and what they cost

- Particles are moved in the vertex shader: each system is one mesh of
  quads, one draw however many there are. A lift used to be 65 draws (one
  a leaf); now it's four (mark, leaves, dust, haze).
- Refraction samples the view transmission texture (a StandardMaterial
  with `specular_transmission` puts the haze in Bevy's transmissive pass),
  bent through a noise-wobbled normal; Bevy's own blurred fetch turned
  depth-rejected taps into black specks at the cards' edges. The copy
  behind it happens only while a haze is in view (within 55 m).
- Particles and snare arcs thicken with distance (glowing ones most, up
  to 2.5×), so a spark or an ember keeps a pixel or two at 20 m in rain
  instead of flickering out; glowing particles use a soft round dot.
- One unshadowed light each. A kind's flicker (`visuals::flicker`)
  drives its light, its particles' brightness and its haze's glow
  together.

Shots: `docs/shots/anomalies/` (`before_*` are the old looks).
