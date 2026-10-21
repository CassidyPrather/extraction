# Main's health

The Verifier's log of main (`claude/serene-clarke-rcgj62`). The coordinator
gates each merge on fmt, build, `extraction_core` tests and the 19 headless
scenarios; this page covers the slow half, run after each main push:

- **smoke**: one sim raid per valley and seed from a copied `fast` binary,
  `SIM_NO_JOB=1 RUST_BACKTRACE=1 <bin> --sim-run smoke --map M --seed S
  --sim-out DIR --headless`, four at a time. Pass = exits 0 with no panic.
- **suite**: `cargo test --workspace --no-fail-fast -j 4 -- --test-threads=4
  --skip render::budget --skip render::legibility`, then those two groups
  alone with `--test-threads=2`: they render in a debug build at about
  4 fps and are load-sensitive by nature. All in one go, the suite takes
  over an hour on four cores.

Never run sims and the suite at once, nor rebuild a binary sims are running
from: the container runs out of memory.

## Shas tested

| main | when (UTC) | smoke | suite | notes |
| - | - | - | - | - |
| 5d2ad52 | 15:00 | 7/7 clean, five valleys (Rime Fell seeds 1-3 all extracted); Sallow seed 1 timed out stuck | stopped at 132 (main moved): `a_stormglass_is_a_roof_in_the_surge` FAILED | |
| 3037c2b | 13:40 | 8/8 clean | not run (main moved) | marrow_creek seed 2 still dies early (now to the sentry) |
| 881fc3b | 13:15 | not run | 727 passed, 7 failed (render gauges not run) | |
| 6789739 | 11:30 | (cbf9c3d's binary) | the 20 reported tests: 19 pass, birds fails; full suite running | |
| cbf9c3d | 10:40 | 8/8 clean | the 20 reported tests: 7 fail | both marrow_creek bots now die to the sentry |
| eb7b932 | 08:35 | 8/8 clean, every hit named | stopped at 339/681: round_trip, warmup, lev FAILED | marrow_creek seed 2's looter dies in the same Walker-Drone-Hound gauntlet at x -60..-110, z -257 on every sha, going for depot_marrow 570 m off (Map design) |
| 073c684 | 06:45 | 8/8 clean (seeds 1, 2) | stopped at 228/611 (main moved): 4 FAILED | the Sallow seed-1 bot was killed by its own wet aegis (15 × 5 HP), and telemetry called it "fall"; fixed on this branch |
| d0118ba | 05:30 | 8/8 clean (seeds 1, 2) | not run (main moved) | 7 of 8 raids died; the cinder_works looter dies to a Crawler at tunnel_cinder at ~3 min on both seeds |

## Open failures

As of 881fc3b's full suite (727 passed, 7 failed). Det. = fails every time, alone too.

| test / run | det. / flaky | class | owner | status |
| - | - | - | - | - |
| `destruction::tests::the_wrecks_come_apart_where_kit_modelled_them` (wreck_lorry 8 parts, 7 expected) | det. | model vs. table drift, since b08309d3 (kit-models) | Destruction | they're fixing |
| `world::underground::tests::crawlers_walk_their_rounds_in_the_dark` (Sallow: a crawler out of the site at (-9.1, 26.8, 141.3)) | det. | | Underground | they're fixing |
| `wildlife::flock::tests::birds_never_go_inside_walls_or_ground_and_settle_again` | det. | birds pinned on new tall geometry (the 56 m furnace stack, Sallow's dam) | Wildlife | fixed on claude/polish-wildlife 3c521d79 |
| `soak::tests::entity_keyed_locals_forget_what_is_gone`: touch.rs:170 `last`, radio/visuals.rs:222-223 `arm` `crank`, destruction/breach.rs:535 `geos`, world/snow.rs:277 `last` | det. | entity-keyed Locals never pruned (a leak in a long session) | coordinator, Radio, Destruction, Visual design, Audio | each fixed on its branch; Soak listed touch.rs as bounded and tightened the lint (any `.remove(` no longer counts as pruning), which found a real leak in anomaly/harvest.rs |
| `soak::save_fuzz::a_poisoned_career_loads_whole_and_in_the_world` (3 gone, 2 expected) | det. | a cassette player and its cell went "nowhere" together | Soak | fixed on claude/polish-soak |
| `world::storm::tests::a_bolt_comes_down_to_where_it_struck` | det. | | Visual design | fixed on main by e16f7954 (after 881fc3b); to confirm |
| Sim: Sallow seed 1's raider stuck 15 min at (-246, -157), 3 m short of a waypoint (-249, -155) on the way to Sallow Head; timed out | **intermittent**: same binary and seed extracted twice | the raider never gave up an unreachable waypoint (fixed on main, c2cdcf2: three sticks within 4 m block the cell and replan). Sims weren't reproducible: with the default thread pool, the same seed diverges by t=24 s; with `EXTRACTION_THREADS=1`, 438 events over 380 s are identical, so the source is multi-threaded execution (system-order ambiguity or Avian's parallel solver), not dt or RNG | coordinator (src/sim) | trap fixed; suggested sims default to one pool thread |
| `anomaly::surge::tests::a_stormglass_is_a_roof_in_the_surge` | ? | | Anomalies | to confirm on 7505d1e |
| `perf::tests::headless_ticks_hold_their_floor` (62.7 units > 60 at 4 threads) | **flaky**: passes alone 2/2 | load-sensitive gauge: reference and busy tick sampled at different moments | Performance | fixed on claude/polish-performance 5e2a955 (per-tick ratio to a reference timed right after, lower quartile: 21-26 units alone and loaded) |

## Closed

| test / run | fixed in | by |
| - | - | - |
| `persist::tests::save_and_restore_round_trip` (radio's tapes doubled by each load) | 881fc3b | Radio |
| `animals_can_be_shot_and_their_carcasses_collected` | 6789739 | Wildlife |
| `nothing_lies_on_a_scenes_note`, `sim::indoor` ×2 (Sallow's drowned prefabs), `animals_can_be_shot` | 6789739 | Story, World models, Wildlife |
| `render::warmup::tests::every_material_is_shelved` (storm's Sheet/BoltMaterial, from 337a20a6) | this branch | Verifier (two shelve lines) |
| `persist::tests::a_heavys_wreck_keeps_until_the_surge`, `surge_rerolls_the_zone_at_base` | eb7b932 | Weapon feel, Heavy |
| `raid::lev::tests::home_with_a_stone_he_says_so`, `no_loot_lands_on_a_note`, capture fighting, contacts churn, destruction boards, `a_thrown_bolt_*`: pass on cbf9c3d | | |
| sim telemetry called an aegis shock "fall" | 47e8772 | Verifier |

## Machine notes

- A fresh container may lack the system packages: `scripts/cloud-setup.sh`
  lists them (`mold`, `libudev-dev`, `libasound2-dev`, `mesa-vulkan-drivers`
  ...). Without `mold` every build fails at the first link.
