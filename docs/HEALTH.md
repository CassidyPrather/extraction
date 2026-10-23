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

Note: rounds 18ba914 to 85c9d1f were first reported with 0 failed. `scripts/test_times.py` then exited non-zero only on budget, not on failed tests; the counts below are corrected from the recorded runs (cea9a92b fixed the script). 5955943 was the only clean one.

| main | when (UTC) | smoke | suite | notes |
| - | - | - | - | - |
| c495f9f | 02:30 | 32/32 exit 0, 18 extracted (Marrow 6/8, Sallow 5/8, Cinder 3/8, Kettle 2/4, Rime 2/4); Cinder seed 5 timed out 6.5 m under the ground at (64-72, -23..-30), 1000 s on to_rv | 5 FAILED (Lev k14, ducks, flock, two pane tests), 22 min | gauges 15/16 in 1898 s (first_contact load: this machine, 1.2-1.5x slower); BC7/BC5 copies baked |
| 191e4bd | 01:30 | not run (main moved) | 10 FAILED (7 since fixed by c495f9f), 23 min | gauges cut short |
| 536ba70 | 06:10 | 32/32 exit 0, no timeouts, 17 extracted (Marrow 7/8, Cinder 4/8, Kettle 2/4, Rime 2/4, Sallow 3/8; every death to drones) | 7 FAILED (the 6 below plus legibility's site signs), all within budget, 22 min | gauges 14/15 in 1328 s (first_contact: load and VRAM) |
| 4fd146c | 03:30 | | 9 FAILED (lev k14, walker_loses_you, lev gifts, ledge trap, walker-to-flushed-birds, ducks, flock, double feed, recall table), 30 min | |
| 823b12f | 01:00 | lost (container restart) | round 33's 6 by name: green | |
| 77ce6f5 | 20:40 | 32/32 exit 0, 17 extracted (Kettle 3/4, Rime 3/4, Cinder 4/8, Marrow 4/8, Sallow 3/8); Cinder 7 boards now; Cinder 3 and Marrow 1 still time out in pockets | 6 FAILED (a_search, spotter, cloak, DeviceNoticed sound, clearance, item boxes); cellar tests pass; 31 min | gauges 16/17 (hold_rate), 51 min |
| 98f85f0 | 17:40 | 32/32 exit 0, 3 timeouts (Cinder 7 can't board at Fire lookout; Cinder 3, Marrow 1 pockets); Kettle 4/4, Sallow 4/8, Marrow 3/8, Cinder 3/8, Rime 2/4 | 6 FAILED (cellars_cinder ×3, prefab clearance, item boxes, walker-looks-at-flock), all within budget, 30 min | gauges 16/16 in 22 min |
| 85c9d1f | 15:20 | 24/24 exit 0: Marrow 3/8 with the BIG_SITE cap (was 0/8); Cinder seed 2 died underground with damage {} | 1 FAILED (`prefabs_stay_inside_their_clearance`), 36 min; 1 over budget (every_drowned_house 301 s, its cut not yet merged) | gauges 15/15 in 21 min |
| 7b03261 | 13:40 | 24/24 exit 0 (seeds 1-4, Marrow 1-8): 11/16 outside Marrow, Marrow 0/8 | 1 FAILED (`prefabs_stay_inside_their_clearance`), 31 min; 1 over budget (every_drowned_house 294 s) | gauges 15/15 in 20 min |
| 5955943 | 12:10 | 20/20 exit 0, 10 extracted (Marrow 0/4) | 925 passed, 0 failed, all within budget, 29 min | gauges 15/15 in 20 min: green |
| ce0f10b | 10:20 | 20/20 exit 0 (seeds 1-4), 9 extracted, no timeouts | 3 FAILED (wounds ×2, melee `a_butt_finds_the_weak_points`), all within budget, 31 min | gauges 14/14 in 17 min (not green after all: see note) |
| 7f23655 | 08:00 | 10/10 exit 0, 5 extracted (lowest yet) | 2 FAILED (`no_daylight_in_the_rooms`, `entity_keyed_locals_forget_what_is_gone`), 33 min wall; 1 over budget (soak walls 123.5 s) | gauges 13/14 in 16 min (busy moment: fix on Performance's branch) |
| 039a64a | 06:40 | 10/10 exit 0, 7 extracted; Sallow seed 1 timed out stuck | 2 FAILED (`no_daylight_in_the_rooms`, `a_drone_held_by_its_frame_stays_in_the_hand`), all within budget, 36 min wall (5630 test-s) | gauges: 13/14 in 18 min (busy moment's PostUpdate share) |
| 18ba914 | 04:30 | 10/10 clean, 8 extracted | 1 FAILED (`no_daylight_in_the_rooms`; first reported as 0, see note), all within budget; 4912 test-seconds (render gauges skipped by hand) | first round with single-threaded headless apps |
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

As of c495f9f (round 39).

| test / run | class | owner | status |
| - | - | - | - |
| `destruction::tests::a_thrown_thing_breaks_a_pane_and_a_slow_one_doesnt`, `a_walker_walking_into_a_pane_breaks_it` | `open_pane` took the first pane in query order, which moved with the layouts | Destruction | fixed on claude/polish-destruction, not merged |
| `wildlife::ducks::...flushed_ducks_fly_off...`, `wildlife::flock::...birds_never_go_inside_walls...` (Marrow) | | Wildlife | fixed in 64423138 (tip 4e4ebb8e), not merged |
| `raid::epilogue::tests::lev_reaches_k14_...` (Lev stops at (-216, 15, -194)) | | Story | routed |
| `render::first_contact` gauge: load over in Marrow 6.48 > 6.0 s, Sallow 4.93 > 4.5 s | this machine (4 cores, 1.2-1.5x Performance's times, every valley alike) | Performance | gauges run with LOAD_SLACK=1.5 from round 40; budgets to be normalised |
| the gauge tier took 1898 s (target 1500): new `stress` every_valley_worst_minute 516 s, `vr_budget` every_valley 627 s (was 123) | | Performance | routed |
| Sim: Cinder seed 5 stuck 6.5 m under ground at (64-72, -23..-30) going to the RV | stuck_block can't get out | coordinator | routed |

## Closed

| test / run | fixed in | by |
| - | - | - |
| legibility site signs, Cinder water_tower ledge trap, recall table, walker_loses_you, crawler_hears_round_a_corner, what_a_machine_breaks, haptics button; first_contact VRAM (it was RGBA8 without etcpak) | c495f9f | Legibility, World models, Robot minds, Hands, Performance |
| lev gifts, walker-to-flushed-birds, double feed (failed on 4fd146c); Cinder 3 and Marrow 1 pocket timeouts; round 33's robot senses, hold_rate, DeviceNoticed, clearance, item boxes | 536ba70 | owners, coordinator |
| cellars_cinder ×3 (warm heat shut for every plan), Cinder 7 couldn't board at Fire lookout | 77ce6f5 | coordinator |
| sim telemetry dropped every unclaimed loss under 0.05 hp a tick (gas, heat, slow bleeding: a death of "nothing"); heat and gas now named | d5c2291e (round 32), 19e8be63 (round 33) | Verifier |
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
- Without `etcpak` (pip) no BC7 copies are baked and the game loads the
  RGBA8 textures, so `first_contact`'s load and VRAM read 2-4x high (round
  36). `cloud-setup.sh` installs it now; in an older container run
  `pip install etcpak && python3 scripts/bc7_textures.py`.
