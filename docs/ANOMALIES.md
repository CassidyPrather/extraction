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

## Finding them

- **Throw something in.** A bolt (junk, found in tool boxes and
  workshops), a nut, a tin. The moment a loose thing first crosses into an
  anomaly it shows itself for six seconds: its light, particles and haze
  surge, whatever kind it is.
- **The detector.** Carried anywhere on you it clicks for the nearest
  anomaly's edge: about 1.5 s apart at 25 m, a rattle at the edge.
  Held, it listens the way its antenna points (one behind you sounds half
  as far again), so sweep it about. Its lamp blinks with each click.

## Artifacts

- **Lodestone**: rounds fired at whoever carries it curve aside (a small
  well turned inside out).
- **Mirror shard**: held and used, it steps you up to 4 m the way you
  face, short of walls and water, once a minute.

## Looks, and what they cost

- Particles are moved in the vertex shader: each system is one mesh of
  quads, one draw however many there are. A lift used to be 65 draws (one
  a leaf); now it's four (mark, leaves, dust, haze).
- Refraction samples the view transmission texture (a StandardMaterial
  with `specular_transmission` puts the haze in Bevy's transmissive pass),
  bent through a noise-wobbled normal; Bevy's own blurred fetch turned
  depth-rejected taps into black specks at the cards' edges. The copy
  behind it happens only while a haze is in view (within 55 m).
- One unshadowed light each. A kind's flicker (`visuals::flicker`)
  drives its light, its particles' brightness and its haze's glow
  together.

Shots: `docs/shots/anomalies/` (`before_*` are the old looks).
