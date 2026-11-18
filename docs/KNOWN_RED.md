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
| first_jobs A/B after the gated yard: the cautious bot's WalkTo times out near gate 4 at (12.4, 18) and (0, 18); the reckless bot dies in raids 3 and 5, the one-handed bot in raids 3 and 4 (main was all home). Raids diverge from raid 3: a real fault, or a knock-on of a different yard run? | 9c1135f4 (in 1f40e45c) | Onboarding | seed sweep before calling it chance |
| `audio_machines::tests::the_dangers_are_heard`: "no sound for: [Recall]" (#107's First::Recall has no sound or excuse) | aa4fccff | Legibility | |
| `render::legibility::tests::every_panel_in_the_game_clears_its_floor` (legibility.rs:770) | aa4fccff | Legibility | |
| `render::affordance::tests::every_interactable_is_a_known_kind_with_true_cues`: training_yard Bell pull, BenchLever, BenchKnob show the ring but hover can't find them | aa4fccff | Onboarding | yard bays (#89) |
| `sim::tests::a_bot_climbs_into_the_truck` (sim/mod.rs:486, "the bot is in the bed") | aa4fccff | Onboarding | the yard hides the truck until it's called |
| `sim::indoor::tests::every_door_lets_a_bot_in` (indoor.rs:1960) | aa4fccff | Onboarding | likely shut YardGate leaves |
| `weapons::impacts::plate_tests::every_range_plate_stops_and_marks_a_round`: yard targets no longer met along their plate normal | aa4fccff | Onboarding | traced by Arcade to the #89 bays (4181723) |
| `sim::raider::run_tests::a_walk_goes_round_a_wreck_in_the_open`: the test's "open ground" walk now crosses a bay fence and detours through gate_3 | aa4fccff | Onboarding | traced by Robot bodies; move the walk to open ground |
| `sim::indoor::tests::every_floor_and_deck_has_a_surface` (indoor.rs:1654, "nothing built underfoot") | aa4fccff | Map design | likely a new village building, the well sweep or a vantage deck |
| `soak::chaos::chaos_monkey_short` (chaos.rs:1285, findings) | aa4fccff | untriaged (coordinator) | rerun cut short |
| `interact::read::tests::the_logbook_comes_up_to_be_read_and_goes_back` | aa4fccff | Hands | fixed in ae762ae8 (in 1b98cfef); confirm on the next lib run |

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
