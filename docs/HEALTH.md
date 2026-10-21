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
| eb0bcd4 | 20:00 | 11/11 clean: Marrow and Sallow seeds 1-4, Kettle, Cinder, Rime Fell seed 1; 7 extracted; stuck_block fired 3 times, no timeouts | full suite running | |
| bdb219a | 17:30 | | 781 passed, 10 failed (7 fixed by eb0bcd4) | |
| 7505d1e | 16:50 | | stopped at 615/790: 6 FAILED | |
| 5d2ad52 | 15:00 | 7/7 clean, five valleys (Rime Fell seeds 1-3 all extracted); Sallow seed 1 timed out stuck | stopped at 132 (main moved): `a_stormglass_is_a_roof_in_the_surge` FAILED | |
| 3037c2b | 13:40 | 8/8 clean | not run (main moved) | marrow_creek seed 2 still dies early (now to the sentry) |
| 881fc3b | 13:15 | not run | 727 passed, 7 failed (render gauges not run) | |
| 6789739 | 11:30 | (cbf9c3d's binary) | the 20 reported tests: 19 pass, birds fails; full suite running | |
| cbf9c3d | 10:40 | 8/8 clean | the 20 reported tests: 7 fail | both marrow_creek bots now die to the sentry |
| eb7b932 | 08:35 | 8/8 clean, every hit named | stopped at 339/681: round_trip, warmup, lev FAILED | marrow_creek seed 2's looter dies in the same Walker-Drone-Hound gauntlet at x -60..-110, z -257 on every sha, going for depot_marrow 570 m off (Map design) |
| 073c684 | 06:45 | 8/8 clean (seeds 1, 2) | stopped at 228/611 (main moved): 4 FAILED | the Sallow seed-1 bot was killed by its own wet aegis (15 × 5 HP), and telemetry called it "fall"; fixed on this branch |
| d0118ba | 05:30 | 8/8 clean (seeds 1, 2) | not run (main moved) | 7 of 8 raids died; the cinder_works looter dies to a Crawler at tunnel_cinder at ~3 min on both seeds |

## Open failures

As of eb0bcd4; det. = fails every time, alone too.

| test / run | det. / flaky | class | owner | status |
| - | - | - | - | - |
| `wildlife::bats::tests::bats_never_go_inside_anything_or_underground` ("rime_fell: no bats hunting Water") | det. | VALLEYS-wide gauges assume every valley has what the first four have | Wildlife (Fifth valley cc) | routed 19:50 |
| `wildlife::ducks::tests::flushed_ducks_fly_off_and_land_on_water_again` ("rime_fell: came down 24 m from the fright") | det. | same | Wildlife (Fifth valley cc) | routed 19:50 |
| `soak::physics::items_spawned_inside_things_are_not_launched`: now a walker_leg_actuator 0.2 m into a wall ends 5.1 m under the ground | det. | tunnelling | Soak | routed 19:50 |
| Sim: Marrow Creek, 0 extractions in 11 raids up to bdb219a (drones, the sentry, a crawler, a burner) | | balance | Map design / Heavy | better on eb0bcd4: 2 of 4 (seeds 1-4); Sallow 2 of 4 |

## Closed

| test / run | fixed in | by |
| - | - | - |
| underground ×5 on Rime Fell (no site), sentry `a_heavy_walks_into_its_fire` (the assert reads backwards: it fails when the sentry never fires), save fuzz's misfit magazine | eb0bcd4 | Underground, Heavy, Soak |
| sims not reproducible from their seed | f1c4fd4 (single-threaded sims) | Verifier |
| stormglass, destruction shutters, `no_prefab_fights_or_floats`, `heaps_settle` (failed on 7505d1e) | bdb219a | Anomalies, Destruction, World models, Soak |
| the wreck parts, crawlers' rounds, birds, Soak's lint and save fuzz, the storm bolt, the perf flake | by bdb219a | Destruction, Underground, Wildlife, Soak + owners, Visual design, Performance |
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
