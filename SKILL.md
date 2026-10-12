---
name: extraction
description: Cassidy's VR extraction shooter — Bevy 0.19, OpenXR, Avian; engine-free core crate; bot-driven scenario tests
---

# extraction

Read `docs/DESIGN.md` first: it records the design decisions and roadmap.

## Repo map

- `crates/extraction_core` — no Bevy. Items (`assets/data/items.ron`),
  inventory rules, firearm state machines, loot tables, map format +
  heightmap builder, deterministic noise/RNG, the cooking graph. Put rules
  here; they test in milliseconds.
- `src/rig` — every player is a head and two hands fed by a driver writing
  `RigInput` in `PreUpdate` (`RigDriverSystems`): `flat.rs`, `xr.rs`
  (feature `xr`, default on), and the bot (`src/bot`).
- `src/interact` — grab points, held-item velocity tracking, sockets, body
  slots, backpack. `HandAction` messages are the common currency for VR,
  flat and bots.
- `src/weapons` — gun layouts (named points + blockout parts), mechanism
  driving, ballistics, flat macros (`GunMacro` component, also used by bots).
- `src/world` — map, heightfield collider, terrain/sky/water/trees visuals.
- `src/bot` — bot brain (`Task` list) and `scenarios.rs` integration tests.

## Rules of the house

- Logic vs. visuals: `GamePlugin` must run headless (no `Assets<Mesh>` or
  materials in logic systems); visuals attach in `ClientPlugin` systems
  reacting to `Added<...>`. `--headless` will panic if you break this.
- Every asset gets a `CREDITS.md` row at intake. CC0 first.
- Clippy at default level; CI does not fail on warnings (deliberate).
- Parry heightfields are `heights_zx` column-major: through Avian's
  `Vec<Vec<_>>` the outer index is X. A test guards it.

## Commands

```bash
cargo test --workspace
cargo run -- --headless --scenario all       # must print OK
scripts/shot.sh target/shots/x.png 40,30,250,10,-8 20   # look at it
scripts/map-preview.sh                       # map design review
```

Screenshots render with lavapipe under Xvfb (`mesa-vulkan-drivers`): ~2 s
per frame, so keep `--frames` small.
