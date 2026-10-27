# Testing: the tiers

Four tiers, fastest first. Run the first two before you push; the
Verifier runs the rest on every main push (`docs/HEALTH.md`).

| Tier | Command | Time (4 cores) | What |
| - | - | - | - |
| Core | `cargo test -p extraction_core` | ~2 min | the engine-free rules |
| Scenarios | `cargo run --profile fast -- --headless --scenario all` | a few min | the bot plays each scenario in a fresh process |
| Suite | `cargo test --workspace -- --test-threads=4` | ~25 min | every test not `#[ignore]`d |
| Gauges | `cargo test --lib -- --ignored gauge_` | ~12 min (at 2 test threads), target 25 | the render budgets, stereo budgets, warm-up shelf, resting guns, first sight, the worst minute: whole valleys through the client (below) |
| Smoke | `SIM_NO_JOB=1 target/fast/extraction --sim-run smoke --map M --seed S --sim-out DIR --headless` | ~5 min a raid | a bot's whole raid in each valley, from a copied binary, four at a time |

Run the suite and sims apart, never at once, and never rebuild a binary
sims are running from: the container runs out of memory.

## The gauge tier's time

Each gauge builds whole valleys through the GPU-less client. Those whose
verdict is a time (the busy moment's CPU, `first_contact`'s loads,
`stress`'s CPU p99, `first_sight`'s window for a load to land) take
`budget::tests::heavy()` and run alone. Those that count (draws,
triangles, overdraw, meshes, shelf entries, frames) take `counted()`
and run beside each other, never beside a timed one.

On this 4-core container, each run alone, in seconds:

| gauge | verdict | before (844e46d) | now |
| - | - | - | - |
| stress, every valley (450 frames) | time | 359 | 185 |
| first_contact, every valley | time | 341 | 152 |
| vr_budget, every valley | count | 133 | 134 |
| budget: a busy moment | time | 123 | 46 |
| budget: Marrow, storm, Kettle, snow, Cinder, Sallow, Rime | count | 320 | 175 |
| first_sight: nothing loads mid-raid | time | 37 | 37 |
| trees: near detail after a jump | count | 37 | 15 |
| vr_budget, Marrow | count | 28 | 29 |
| warm-up: shelf, two eyes; resting guns | count | 43 | 48 |
| all, one after another | | 1421 | 819 |

The tier at 2 test threads with `LOAD_SLACK=1.5`: 1378 s before, 716 s
now. Most of the cut is `settle`: its triangle count never forgot a
dropped mesh, so the far-off flocks' merged mesh, replaced every 6
frames, kept every valley "arriving" for the whole 3000-frame cap. The
VRAM it counts after settling is the same to the MB.

The target is 25 minutes (1500 s) on 4 cores. The timed gauges run
alone whatever the core count (about 420 s here), and the counting
ones share what's left (about 400 s alone here). On n cores, expect
roughly 420 + 400 x 2/n s, before the container's own speed (the
Verifier's runs about 1.2-1.5x this one). A tier over 1500 s on 4 cores
is a regression to find, not a slack to add.

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
