# Known red

What is currently failing on the integration branch (`claude/serene-clarke-rcgj62`), and who owns
the fix. The coordinator keeps this page.

Since 17 Nov, breakage doesn't block a merge (the user: "lint and test breakage shouldn't preclude
merges, but instead flag followup work"). Only two things block a merge: the game or the lib tests
don't compile, or a headless run crashes (a panic, a signal or a hang). Everything else that fails
lands here instead:
- fmt and warnings;
- a failing test;
- a scenario or contract failing;
- second_hour coming up short.

How it works:
- Every merge goes through the coordinator's fast gate: build, lib-test compile, the core and
  gunsmith tests, `--scenario all`, the 13 contracts and second_hour.
- The full lib suite runs afterwards on the pushed head, in its own worktree.
- Failures from either are added below with the commit they were first seen on and their owner.
- An owner's red item is their next job, ahead of new work. The owner of the code fixes it when two
  sessions collide.
- Before chasing a red test on your branch, look here first. If it's listed and isn't yours, it
  isn't yours.
- Fixed items move to the bottom section with the commit that fixed them.

## Open

| What | First seen on | Owner | Notes |
| - | - | - | - |
| first_jobs one-handed bot: 6 deaths on seeds 1-5 vs 2 on main (reckless was chance: 4 vs 4) | 9c1135f4 (in 1f40e45c) | Onboarding | re-measured on 6d425f1d (19 Nov): 1 death per seed, no common cause (logged as balance); seed 5 also has 2 HurtUnwarned, which are the anomaly discharge (row below), not a one-handed fault |
| Seed-5 one-handed first_jobs: two 22 HP snare hits (raid 4 at 64.8 s and 68.9 s; raid 5 at 68.3 s), Surge snare at (-10.85,47.45) r1.92, hand-reach branch. The bot's resting hand (chest height, 0.5 m above the heart) grazes 1-3 cm inside the rim while its body is 2.1 m out: a passing brush, not a reach. Ruling changed 19 Nov 12:00: a brush at the rim doesn't fire; a real reach (down for the stone, or in toward the heart) does. No second Player entity (Anomalies mixed two worlds' logs) | 6d425f1d (Onboarding re-measure) | Anomalies (reach margin + tests) | |
| `render::puff::tests::no_puff_is_a_sphere`: src/render/handle_glow.rs draws 1 sphere, not on the list | 3634c69 (Hands, lib372 on 780a1e7b) | Hands | the Mosin glow, being reworked into a soft halo anyway: list it with what it is, or stop it being a sphere |

## Logged, not failing a check

- **A wreck lying in a doorway or gateway is invisible to the raider's `Scene::wreck_out_at`** when the ray starts under a lintel, rail or leaf (bot-only). Owner: Onboarding.

- **one-handed first_jobs 4/5**, raid 4 (post_counter): the bot climbs out of the creek bed beside
  the Forest path footbridge deck at (-87.76, -35.05) and stops there. A bot-only stall: the gap is
  in 45bdef65's bridge-end approach. Owner: Onboarding.
- **seed-3 one-handed 4/5** (since 863a6150): the one-handed reload looks only in the pouches while
  the spare is in the pack. Owner: Onboarding, with #52/#53.

## Fixed

| What | Fixed by |
| - | - |
| `soak::ledges::tests::no_valley_has_a_ledge_trap` (rime ore mill's high sills, solid since a41e3ff) | f31200ce (World models: only sills within reach are solid), gate 373; confirm on lib373 |
| scenario `walker_hunt` (0 wrecked: bots' sights turned to an eye 32 mm off the head) | 2bc1856 (Hands: a bot sights from its head), green on gate 372 |
| `audio_machines::tests::the_dangers_are_heard` (MachineSpent) | 00ed3f59 (Arcade), green on lib371 (4f146f94: 1863/1) |
| `render::puff::tests::no_puff_is_a_sphere` (weak_points.rs) | 519754a7 (Robot bodies), green on lib371 |
| `robots::recall` every_robot_keyed_local_is_in_the_table (grip.rs) + `soak` entity_keyed_locals_forget_what_is_gone (arena.rs) | ce7a46e5 (Robot bodies), 0c9f8e34 (Arcade), green on lib371 |
| `soak::chaos::chaos_monkey_short` items off the map after a teleport (a held pack stayed behind, then hung 224 m off) | 6a8656d2 (Hands), green on lib364 (85daf8f6: 1847/0) |
| `robots::recall` + `soak::entity_keyed_locals_forget_what_is_gone` (sorter_look `waited`) | bcf4a1dc (Robot bodies), green on lib360 |
| `capture::checks::models_are_a_believable_size` items/plough_plate 3.08 m | 39c11074 (Heavy: two 1.54 m halves), green on lib357 |
| `asset_path_tests::game_code_never_reads_from_the_build_machine` (grips.rs CARGO_MANIFEST_DIR) | 81b48bb7 (World models), green on lib354 |
| first_jobs cautious yard WalkTo timeouts near gate 4 | 0ba2aadb (Onboarding) |
| `render::affordance` every_interactable (yard Bell pull, BenchLever, BenchKnob) | 4cf47a06 (Onboarding) |
| `sim::tests::a_bot_climbs_into_the_truck` | 4cf47a06 (Onboarding) |
| `sim::indoor::tests::every_door_lets_a_bot_in` | 4cf47a06 (Onboarding) |
| `weapons::impacts::plate_tests::every_range_plate_stops_and_marks_a_round` | 70521467 (Onboarding) |
| `sim::raider::run_tests::a_walk_goes_round_a_wreck_in_the_open` | feb39b12 (Onboarding) |
| `sim::indoor::tests::every_floor_and_deck_has_a_surface` (yard gates' ground floors) | fcffac03 (Map design) |
| `render::legibility::tests::every_panel_in_the_game_clears_its_floor` | 1744915e (Legibility) |
| `interact::read::tests::the_logbook_comes_up_to_be_read_and_goes_back` | ae762ae8 (Hands) |
| `capture::attach::tests::model_pieces_touch_what_holds_them` (field set pieces in the air) | 6b547021 (Progression) |
| `render::legibility::tests::every_model_with_letters_is_registered_and_legible` (MIX label) | edfd7666 (Legibility) |
| `onboarding::pickups` each_line_is_what_the_game_does, every_kind_rollable_before_raid_6 (cartridges, DP-3) | 9f512a16 (Onboarding) |
| `capture::attach::tests::model_pieces_touch_what_holds_them`: items/regen_injector, two 11x2x3 mm pieces at (±0.008, 0.010, 0) float 3 mm off the body | passes on 10957625 (Models fc5d9928) |
| warning: constant `LATER` is never used at src/onboarding/pickups.rs:95 | 0f34cacf (Onboarding) |
| warning: function `least_clear` is never used at src/world/hinge.rs:763 | e2a5ef5a (World models) |
| worst-view stereo draw gauge: Cinder s01_ahead (night, downpour, fight) 2552 of 2400 draws (106%); Cinder insertion_south_gate_2 97%, two fuel-exfil views ~96%, Marrow poi_radio_mast_0 94% — up ~650 draws since 10 Nov, in the smaller groups | draws: 8a42f400 (Performance) + 95c32584 (Arcade); five-valley table on 10957625 green |
| `capture::attach::tests::model_pieces_touch_what_holds_them`: items/hound_hip_servo fin 3 mm off; props/stove doors 2-14 mm off | Heavy 37a452bb, World models 73fa36b6; passes on d6bdf8ef (lib344) |
| `capture::fighting::tests::models_dont_fight_themselves`: walker grappler_core vs engineer_hull 0.13 m² | Models 705f16c9; passes on d6bdf8ef (lib344) |
| `models::tests::no_flat_model_material_is_paper_white`: "Baked" 1.00 | Arcade 35bc2962; passes on d6bdf8ef (lib344) |
| `onboarding::firsts` every_machine_has_its_first_contact_line, `robots::intent_roster` every_machine_is_on_the_intent_roster: the engineer | Robot bodies d40ba037; passes on d6bdf8ef (lib344) |
| `render::landmarks::tests::every_landmark_lamp_stands_on_its_building`: no art for ore_mill/observatory (locker#carcass.glb, stove#carcass.glb) | World models 313a9753; passes on d6bdf8ef (lib344) |
| `sim::indoor::tests::what_is_laid_on_a_spot_settles_in_reach`: 68 of 1395 laid things out of the plan, seed 4, ~20 m under ground | World models 313a9753 (test model was stale; nothing fell); passes on d6bdf8ef (lib344) |
| `soak::save_fuzz` relocating_keeps_every_item, every_item_is_saved_once..., a_poisoned_career_loads_whole...: save holds ~17-21 more items than the world | World models 0c655f2d (fuzz model was stale; ToHold gap closed); passes on d6bdf8ef (lib344) |
| `items::loot::hold_tests::holds_leave_every_other_roll_as_it_was`: rime_fell flashlight moved | World models 0c8b13f9 (compares rolled-at); passes on d6bdf8ef (lib344) |
| `soak::quiet` no_session_logs_a_warning, no_valley_logs_an_error_without_the_downloads: "Unknown vertex attribute COLOR_" (bevy_gltf) | Arcade 12c412e5 (COLOR_0 only); passes on d6bdf8ef (lib344) |
| warning: unused import `Stash` at src/vr_harness.rs:2590 | gone: no build warnings since gate 342b (10957625) |
| warning: unused `held` at src/anomaly/mod.rs:1284 | gone: no build warnings since gate 342b (10957625) |
| steam-bank gauge, Kettle inside 104% fragments | Anomalies 2c577d29 (20° cap) + Performance dd57552d (replay cap): worst 69% inside, ≤34% at 10 m |
| `vr_harness::wreck_reach::every_wrecks_salvage_is_seen_and_reached`: drone robot_cell (Live) unseen, out of reach (0.03 m up, under the hull) | Heavy acb939c3; passes on d0b692c0 (lib346) |
| `vr_harness::salvage::a_servo_pack_lifts_out_once_its_bolts_are_undone`: "a servo pack bolted on" (seat moved in #105 step 3) | Heavy acb939c3; passes on d0b692c0 (lib346) |
| sentry/spotter/plough/pylon item models off their boxes | Heavy fdedaa97; passes on c91aea7e (lib348) |
| `robots::sentry::tests::its_wreck_lies_in_reach` (counted every item) | Heavy 0d575202; passes on c91aea7e (lib348) |
| `soak::chaos::chaos_monkey_short` stim ejected at 59 m/s (grenade impulse on light bodies) | Hands e3e8abb2 (light-body floor in weapons/grenades.rs) |
| `perf::tests::no_collider_scale_drifts` rain_barrel scale | World models e567e778; passes on 6efac71d (lib351) |
