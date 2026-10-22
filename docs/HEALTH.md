# Main's health

The Verifier's log of main (`claude/serene-clarke-rcgj62`). The coordinator
gates each merge on fmt, build, `extraction_core` tests and the 19 headless
scenarios; this page covers the slow half, run after each main push:

- **smoke**: one sim raid per valley and seed from a copied `fast` binary,
  `SIM_NO_JOB=1 RUST_BACKTRACE=1 <bin> --sim-run smoke --map M --seed S
  --sim-out DIR --headless`, four at a time. Pass = exits 0 with no panic.
- **suite**: `cargo test --workspace --no-fail-fast -j 4 -- --test-threads=4`
  (~25 min since headless apps run single-threaded), timed with
  `scripts/test_times.py` (fails a test past its budget).
- **gauges**: `cargo test --lib -- --ignored gauge_` (Performance's render
  gauges, one at a time; a failure there is Performance's).

The tiers are in `docs/TESTING.md`.

Never run sims and the suite at once, nor rebuild a binary sims are running
from: the container runs out of memory.

## Shas tested

| main | when (UTC) | smoke | suite | notes |
| - | - | - | - | - |
| 5955943 | 12:10 | 20/20 exit 0, 10 extracted (Marrow 0/4) | 925 passed, 0 failed, all within budget, 29 min | gauges 15/15 in 20 min: green |
| ce0f10b | 10:20 | 20/20 exit 0 (seeds 1-4), 9 extracted, no timeouts | 907 passed, 0 failed, all within budget, 31 min | gauges 14/14 in 17 min: the first fully green round |
| 7f23655 | 08:00 | 10/10 exit 0, 5 extracted (lowest yet) | 900 passed, 0 failed, 33 min wall; 1 over budget (soak walls 123.5 s) | gauges 13/14 in 16 min (busy moment: fix on Performance's branch) |
| 039a64a | 06:40 | 10/10 exit 0, 7 extracted; Sallow seed 1 timed out stuck | 879 passed, 0 failed, all within budget, 36 min wall (5630 test-s) | gauges: 13/14 in 18 min (busy moment's PostUpdate share) |
| 18ba914 | 04:30 | 10/10 clean, 8 extracted | 856 passed, 0 failed, all within budget; 4912 test-seconds (render gauges skipped by hand) | first round with single-threaded headless apps |
| 4d45a5f | 03:40 | | 846 tests minus gauges in 24 min: 1 FAILED | single-threaded headless apps |
| 24d1ef0 | 02:00 | | 842 of 861 timed, 0 FAILED; 2 h wall | the profile: render gauges 45% |
| 8e63945 | 22:00 | 10/10 clean, 7 extracted | rechecks: 5 of 8 pass | sims 2-3x faster single-threaded |
| eb0bcd4 | 20:00 | 11/11 clean: Marrow and Sallow seeds 1-4, Kettle, Cinder, Rime Fell seed 1; 7 extracted; stuck_block fired 3 times, no timeouts | 798 passed, 8 failed | |
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

None in the suite or gauges as of ce0f10b.

| test / run | det. / flaky | class | owner | status |
| - | - | - | - | - |
| Sim: Marrow Creek extracts worst all day (0/4 on 5955943); seeds 2 and 4 walk 566 m to depot_marrow along the southern road into a Walker + Drone patrol (x -75…+25, z -230…-265) and die at ~370 s with no loot | det. | `choose_site` gave a building 0.35 pull per loot spot without limit, so the Depot's underground complex outscored every site and a fresh raider marched up the Gate road, the robots' commute | Map design | fixed on claude/polish-map-design-2 (spots past 8 add no pull): seed 2 now extracts with 36 items, seed 4 dies later with 35; to confirm on main |

## Closed

| test / run | fixed in | by |
| - | - | - |
| the raider waded into live water (Cinder seed 4 now extracts); telemetry names live water | 5955943 / a1edafef | coordinator, Underground, Verifier |
| the busy-moment gauge (share limits under one thread), the Sallow pocket (escalation), Soak's walls over budget | ce0f10b | Performance, coordinator, Soak |
| `robots::mind_tests::a_crawling_walker_doesnt_flank` (a walker took the flank before its limp caught up) | 18ba914 (8b9fabb6) | Robot minds |
| the dirigible pod (a spotter counted against the director's budget, since bb5dbaf0), `heaps_settle` (a log pile taken for a floor), Rime Fell's bats and ducks | 24d1ef0 / 4d45a5f | Robot bodies, Soak, Wildlife |
| the suite took ~2 h at 4 threads | this branch (single-threaded headless apps) + Performance's gauge tier | Verifier, Performance |
| Rime Fell bats and ducks (no water birds above the snow line; the tarn is too small for a raft), Soak's launch test (an item down a site is fine), drone chirp, thrown bolt | 8e63945 | Wildlife, Soak, Robot minds |
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
