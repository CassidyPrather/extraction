# Stairs hop: probe and fix design

Playtest 2026-11-22: climbing box stairs hops and lunges.

## What the probe found

The probe is `probe_stairs_hop.rs`. It runs six steps, each with a 0.17 m rise and a 0.3 m tread, against a ramp through the same nosings. The player walks at 2.4 m/s at 90 Hz.

- **Box flight:**
  - The body stalls about 0.3 s at the first riser, with a 7 cm bob.
  - `try_step` then lifts it a whole riser in one tick.
  - Each riser also throws the body forward by one capsule radius (0.28 m), so it climbs the flight at about 8.7 m/s.
- **Ramp:** the climb is smooth, with no more than 1.5 cm rise per tick.

## The fix

There's no draft diff. The edit to `src/player.rs` was refused before one was written.

1. **Step up on first contact.** Step up as soon as the capsule first touches a riser no higher than `STEP_HEIGHT`. Allow a 0.2 s grace after contact, so a riser touched mid-stride still counts and the body doesn't stall at it.
2. **Ease the eye.** Let the body snap up, but move the camera to the new height through an `eye_ease` on `Body`, carried by `RigInput.lifted`, so no single frame rises more than about 3 cm. Don't hide ramps under stairs.
3. **Remove the lunge.** Measure the forward throw per riser and remove it. A step-up should keep the horizontal position it had before the step.

## The test

Make the probe a real test. It should assert that the per-tick rise is no more than 0.03 m both up and down the box flight. It should also check that the climb's average speed stays near `WALK_SPEED`.
