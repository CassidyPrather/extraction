# Extraction

A VR extraction shooter in the spirit of Into the Radius: scavenge a
quarantined, fog-bound valley, keep out of the robots' way, and get across
it to the truck. Nothing can be bought; everything is found.

Rust + [Bevy](https://bevy.org/) 0.19, OpenXR via
[bevy_mod_openxr](https://github.com/awtterpip/bevy_oxr), physics by
[Avian](https://github.com/Jondolf/avian). Runs in a headset when an OpenXR
runtime is present (SteamVR, Meta Link, WMR, Monado), flat on a monitor
otherwise.

Design decisions and the roadmap: [docs/DESIGN.md](docs/DESIGN.md). Before a
playtest: [docs/PLAYTEST.md](docs/PLAYTEST.md).

## Playing

```bash
cargo run --release            # VR if a runtime is up, otherwise flat
cargo run --release -- --flat  # always flat
```

Flat controls: WASD move, mouse look (click to capture, Esc to release),
Shift sprint, Space jump, C crouch. E grab what the crosshair is on, F put
the second hand on the gun, Q drop (hold to throw), G stow, Tab open the
backpack, 1-0 draw from a body slot. LMB fire, RMB aim, R reload, V rack /
work the bolt / break the shotgun, X selector, B magazine release, C combine
the two held items (load a magazine from a box, clean a gun).

VR: grip grabs, trigger fires; A/X magazine release (breaks a shotgun),
B/Y slide stop; left stick walks (click to sprint), right stick snap-turns
and (down) crouches, right stick click works the selector. Reach for a
slide, charging handle or bolt with your free hand to work it; reach over a
shoulder for the backpack and let go of it in front of you to open it.
Look at your left wrist for the Surge clock, your load and the radio.
The controllers kick when you fire, click as you work the action, thunk
as a magazine seats, and shake when you're hit. Hold
a scope or binoculars to your eye to look through them.

Comfort: press the cards on the truck bed's left wall (turning: snap 30°
or 45°, smooth 90-180°/s; vignette: off to 90%, darkening the edges
while the stick moves you; sound: full or narrow range). Or
`--smooth-turn` (or `--snap-turn`), `--vignette 0..1`,
`--narrow-dynamics`, or put them in
`~/.local/share/extraction/settings.ron` (Windows: `%APPDATA%\extraction`):
`(smooth_turn: true, turn_speed: 120.0, snap_angle: 30.0, vignette: 0.6, narrow_dynamics: false)`.

The game saves itself (the stash, what you carry, and each valley's Zone
as you left it) to `career.ron` in the same folder; `--new-game` starts
over.

Listen. A drone that has you chirps three times before it fires, so get
behind something. A radio on you crackles with robot chatter when a
response team is sent, and says whether it's coming your way. Artifacts
draw attention you feel rather than see: a ringing in your ears that
grows. The logbook by the tailgate writes up each raid; beside it the driver
asks for things, and pays by telling you where their cache is.

Three valleys so far: **Marrow Creek**, a foggy river valley; **Kettle
Pass**, a mining valley that climbs to a high pass; and **Cinder Works**,
a burnt-out works on a canal. To change valleys,
press the route card on the truck bed's right wall and ring the bell; the
truck drives through the night and the game reopens there. `--map NAME`
opens a valley directly.

## Development

Needs the Rust toolchain pinned in `rust-toolchain.toml`. On Linux also
`libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev`, and optionally
`mold` + `clang` (configured in `.cargo/config.toml`; delete that block if
you don't have them). `scripts/cloud-setup.sh` installs everything.

```bash
cargo build                                        # first build is long (Bevy)
cargo test --workspace                             # unit tests (core rules are instant)
cargo run -- --headless --scenario all             # bot-driven integration tests
cargo run -- --scenario pistol_drill               # ...watch one in a window
scripts/map-preview.sh                             # top-down map design preview
scripts/sim.sh marrow_creek 8 --jobs 3             # bots play 8 raids; report + heatmaps
scripts/shot.sh out.png X,Y,Z,YAW,PITCH 20         # headless screenshot (Xvfb + software Vulkan)
cargo clippy --workspace --all-targets             # default lints; warnings don't fail CI
```

Assets: CC0 textures and sounds are fetched and processed by
`scripts/fetch_assets.py` and `scripts/fetch_audio.py`; the game runs
without them (procedural stand-ins). Every asset is credited in
[CREDITS.md](CREDITS.md).

### Layout

- `crates/extraction_core` — engine-free rules: items, inventory, firearm
  mechanisms, loot, maps and terrain, navigation, anomaly layouts, the
  bench's recipes, the save format, the cooking graph. Fast unit tests.
- `src/` — the Bevy game. `GamePlugin` is the logic and runs headless;
  `ClientPlugin` is everything you see and touch.
  - `rig/` — the head-and-two-hands abstraction, its drivers (flat, XR,
    bot) and comfort settings
  - `interact/` — grabbing, holding, sockets, the body rig and backpack
  - `weapons/` — guns, ballistics, flat-mode handling macros
  - `gadgets/` — mounts and attachments, power cells, optics, EMP, cover,
    the cloak
  - `robots/` — drones, walkers, grapplers, the dirigible, the convoy's
    collector, perception and the director
  - `anomaly/` — anomalies, artifacts and attention, offerings
  - `raid/` — the truck, the raid clock and the Surge, the bench, the camp
    kitchen, the route card, the logbook, the driver's requests, radio
    chatter
  - `persist.rs` — the career save: stash, kit, record, and each valley's
    Zone
  - `world/` — map loading, terrain, sky, fog, weather and rain, water,
    forests, buildings
  - `render/` — materials, height fog, in-world text panels, props
  - `models.rs` — glTF models built by the Blender scripts in `art/`
  - `bot/` — bot brains and the scenario tests
  - `sim/` — map simulation: bot raiders that play whole raids and write
    telemetry ([docs/SIMULATION.md](docs/SIMULATION.md))
- `assets/` — data (`data/items.ron`, `data/loot.ron`), maps, shaders,
  models, sounds.
- `art/` — Blender Python scripts for every model (`python3 art/build.py`).
- `unity/` — groundwork for the VRChat port: the map importer for Unity
  (fed by `cargo run -p extraction_core --bin unity-export`). The plan is
  [docs/VRCHAT_PORT.md](docs/VRCHAT_PORT.md).
- `scripts/` — asset fetching, sound synthesis, map previews and the
  hand-drawn map, screenshots.

## License

AGPL-3.0-or-later. Third-party assets are listed in [CREDITS.md](CREDITS.md).
