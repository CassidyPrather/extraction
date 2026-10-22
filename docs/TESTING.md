# Testing: the tiers

Four tiers, fastest first. Run the first two before you push; the
Verifier runs the rest on every main push (`docs/HEALTH.md`).

| Tier | Command | Time (4 cores) | What |
| - | - | - | - |
| Core | `cargo test -p extraction_core` | ~2 min | the engine-free rules |
| Scenarios | `cargo run --profile fast -- --headless --scenario all` | a few min | the bot plays each scenario in a fresh process |
| Suite | `cargo test --workspace -- --test-threads=4` | ~25 min | every test not `#[ignore]`d |
| Gauges | `cargo test --lib -- --ignored gauge_` | ~1 h | the render budgets, stereo budgets, warm-up shelf, resting guns, first sight: whole valleys through the client, one at a time |
| Smoke | `SIM_NO_JOB=1 target/fast/extraction --sim-run smoke --map M --seed S --sim-out DIR --headless` | ~5 min a raid | a bot's whole raid in each valley, from a copied binary, four at a time |

Run the suite and sims apart, never at once, and never rebuild a binary
sims are running from: the container runs out of memory.

## Keeping the suite fast

- Every headless app runs single-threaded (`single_threaded` in
  `src/lib.rs`): four tests at a time each get a core, instead of four
  multi-threaded executors fighting over all of them (two to three times
  faster, and deterministic).
- A gauge that has to load whole valleys through the client is named
  `gauge_*` and `#[ignore = "gauge"]`; one representative stays in the
  suite (`training_yard_holds_the_budget`).
- `scripts/test_times.py` runs the suite with libtest's own timing and
  fails any test past 120 s (at four test threads) that isn't listed in
  `scripts/slow_tests.txt` with its own budget and why. Cut a slow test
  (fewer frames, a small arena instead of a valley) before listing it.
