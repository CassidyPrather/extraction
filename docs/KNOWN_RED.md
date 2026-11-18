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
| first_jobs one-handed bot: 6 deaths on seeds 1-5 vs 2 on main (reckless was chance: 4 vs 4) | 9c1135f4 (in 1f40e45c) | Onboarding | game cause fixed (8bb2a19a); bot cause #96 (one-handed reload only from a pouch; spare packed) — first fix didn't trigger, under investigation |
| warning: unused import `Stash` at src/vr_harness.rs:2590 | aa4fccff | vr_harness's owner (to confirm) | |
| warning: unused `held` at src/anomaly/mod.rs:1284 | (Legibility's report, 18 Nov) | Anomalies | |
| `capture::attach::tests::model_pieces_touch_what_holds_them`: items/regen_injector, two 11x2x3 mm pieces at (±0.008, 0.010, 0) float 3 mm off the body | bca2242a (lib338) | Models | the injector model's grip tabs |
| warning: constant `LATER` is never used at src/onboarding/pickups.rs:95 | 950cadec (gate 339) | Onboarding | |
| warning: function `least_clear` is never used at src/world/hinge.rs:763 | 950cadec (gate 339) | World models | |
| worst-view stereo draw gauge: Cinder s01_ahead (night, downpour, fight) 2552 of 2400 draws (106%); Cinder insertion_south_gate_2 97%, two fuel-exfil views ~96%, Marrow poi_radio_mast_0 94% — up ~650 draws since 10 Nov, in the smaller groups | 9d495af1 | Performance (finding the group), then that group's owner | cuts only in views ≥85% |

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
| first_jobs cautious yard WalkTo timeouts near gate 4 | 0ba2aadb (Onboarding) |
| `render::affordance` every_interactable (yard Bell pull, BenchLever, BenchKnob) | 4cf47a06 (Onboarding) |
| `sim::tests::a_bot_climbs_into_the_truck` | 4cf47a06 (Onboarding) |
| `sim::indoor::tests::every_door_lets_a_bot_in` | 4cf47a06 (Onboarding) |
| `weapons::impacts::plate_tests::every_range_plate_stops_and_marks_a_round` | 70521467 (Onboarding) |
| `sim::raider::run_tests::a_walk_goes_round_a_wreck_in_the_open` | feb39b12 (Onboarding) |
| `sim::indoor::tests::every_floor_and_deck_has_a_surface` (yard gates' ground floors) | fcffac03 (Map design) |
| `audio_machines::tests::the_dangers_are_heard` (Recall) | 88c44a5e (Legibility) |
| `render::legibility::tests::every_panel_in_the_game_clears_its_floor` | 1744915e (Legibility) |
| `interact::read::tests::the_logbook_comes_up_to_be_read_and_goes_back` | ae762ae8 (Hands) |
| `soak::chaos::chaos_monkey_short` | passes on 5f9a23a7 (lib332) |
| `capture::attach::tests::model_pieces_touch_what_holds_them` (field set pieces in the air) | 6b547021 (Progression) |
| `render::legibility::tests::every_model_with_letters_is_registered_and_legible` (MIX label) | edfd7666 (Legibility) |
| `onboarding::pickups` each_line_is_what_the_game_does, every_kind_rollable_before_raid_6 (cartridges, DP-3) | 9f512a16 (Onboarding) |
| `audio_machines::tests::the_dangers_are_heard` (StimCue), `items::loot` nothing_turns_up_before_its_raid / a_first_raids_small_things_lie_in_sight, `models` every_item_box_is_its_model / no_item_falls_back_to_the_placeholder, `capture::scale` items_are_drawn_near_their_real_size | pass on bca2242a (lib338): Hands 80d205c3, Models 911c7098 |
