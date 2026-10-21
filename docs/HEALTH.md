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
| 073c684 | 06:45 | 8/8 clean (seeds 1, 2) | stopped at 228/611 (main moved): 4 FAILED | the Sallow seed-1 bot was killed by its own wet aegis (15 × 5 HP), and telemetry called it "fall"; fixed on this branch |
| d0118ba | 05:30 | 8/8 clean (seeds 1, 2) | not run (main moved) | 7 of 8 raids died; the cinder_works looter dies to a Crawler at tunnel_cinder at ~3 min on both seeds |

## Open failures

| test / run | since | det. / flaky | class | owner | status |
| - | - | - | - | - | - |
| `persist::tests::a_heavys_wreck_keeps_until_the_surge` | 073c684 | ? | Surge despawn (`insert` on a despawned entity) | Weapon feel / Heavy | fixed on eb7b932 by 36e66c5 and optics.rs:189 (coordinator); to check |
| `persist::tests::surge_rerolls_the_zone_at_base` | 073c684 | ? | same | Weapon feel / Heavy | same |
| `render::legibility::tests::no_loot_lands_on_a_note` | 073c684 | ? | ? | Legibility / Story | to check on eb7b932 |
| `render::legibility::tests::nothing_lies_on_a_scenes_note` | 073c684 | ? | ? | Legibility / Story | to check on eb7b932 |

## Closed

| test / run | fixed in | by |
| - | - | - |

## Machine notes

- A fresh container may lack the system packages: `scripts/cloud-setup.sh`
  lists them (`mold`, `libudev-dev`, `libasound2-dev`, `mesa-vulkan-drivers`
  ...). Without `mold` every build fails at the first link.
