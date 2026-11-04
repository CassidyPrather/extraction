# Robot minds: fairness notes

How fair a newcomer's fights with each machine are. Each machine is
measured by:
- how long its tell leads the first harm;
- whether someone who reads the tell can escape it or fight it.

The rule is that punishing stays and unfair goes.

## First when we resume: the melee machines (hound, crawler, heavy)

The brief: the melee machines under the same fairness lens as the
gunners. For each, the time from tell to first harm for a newcomer, and
whether someone who reads its tell can escape or fight it. Use the same
table format, with the cautious and lost newcomers, and only change what
the numbers say is unfair.

Measured, and the fixes are written and tested fail-first. They are not
gated and not on the branch. They are saved as
[`robot_minds/melee_fairness_wip.patch`](robot_minds/melee_fairness_wip.patch),
a diff against 43822017 (`git apply` it). To finish:
1. Apply the patch.
2. Run the three new tests and the neighbouring modules: robots::{mind_tests, senses, indoor, hound, tells_tests}, world::underground.
3. Re-run `first_fight_hound` and `first_fight_heavy`.
4. Gate, then report.

### What the patch holds

- **New first-fight encounters:** `hound` and `heavy` added to
  `onboarding::first_hour::ENCOUNTERS`. They're placed on their rounds,
  20 m and 25 m off the way. That's Onboarding's file: tell its owner. The
  crawler lives in indoor sites, so it's measured in a test (below).
- **Three fail-first tests in `robots::mind_tests`.** Each failed before
  its fix; none has been run after the fix yet:
  - `a_crawlers_charge_warns_as_long_as_a_walkers_aim`. Before the fix:
    "zapped 0.56 s after its charge, 0.88 s after it saw you".
  - `a_hound_loses_you_low_and_still`. Before the fix: "bitten 2 times
    more, low and still".
  - `nothing_comes_for_you_at_base`. Before the fix: "at base, it has
    you".
- **Three fixes:**
  - **Crawler:** `indoor::ZAP_CHARGE` 0.5 → `walker::AIM_TIME` (1.5 s).
    That's the gunners' bar, every round 1.5 s after its tell.
  - **Hound:** `senses::exposure`, the point-blank floor goes from a flat
    0.9 to `0.9 × posture × motion`. Within 4 m the cone, fog, grass, light
    and trees still hide nothing, but how low and how still you are now
    count. Every machine still sees you at point blank, except that a
    motion-sighted one loses you low and still:

    | Machine, you low and still at point blank | Exposure | Seen? (seen at 0.12) |
    | - | - | - |
    | hound | 0.099 | no |
    | drone | 0.149 | yes |
    | walker | 0.27 | yes |

    The driver says "Low behind it, and keep still. It only finds what
    moves", and with this a hound doesn't contradict it.
  - **Home:** `search::let_go_at_home` with `search::at_home`. While the
    truck is pulling out (`Extracting`) or you're `AtBase`:
    - `perceive` doesn't see players;
    - whatever had you gives up, as `forget_the_downed` does for
      someone who's gone down.
- **Not yet checked:** whether the point-blank change moves any timing
  test. The suspicion gain scales with exposure, so a still, crouched
  player near a walker is gained on at 0.27, not 0.9.

### The numbers so far

These are first-fight runs, one per newcomer. The machine is placed on
the first route to the Old farm. The numbers are before the fixes.

| Machine | Newcomer | Noticed → first tell | Tell → first harm | Harm | Outcome |
| - | - | - | - | - | - |
| hound | cautious | 6.0 s | 0.8 s (coil) | 16 a bite, every ~3.5 s | down, 6 bites |
| hound | lost | 8.9 s | 1.7 s | as above | down, 6 bites |
| hound | hoarder | 4.4 s | 0.8 s | as above | down |
| hound | one-handed | 28.6 s | 0.6 s | as above | down; it got low and kept still, as told, and was bitten 5 more times |
| hound | reckless | – | never hit | – | home: it fought and lamed it |
| heavy | cautious | 27.5 s | 1.4 s (arm rising) | 57, then 70 (two blows kill) | down |
| heavy | lost | 29.9 s | 24 s (it ran, then stopped) | 60, 70 | down |
| heavy | hoarder | 17.5 s | 100 s | 70, 70 | got home and was paid, then the heavy that had followed it slammed it dead by the truck |
| heavy | one-handed | 28.1 s | 1.4 s | 43, then 58 a minute later | down |
| heavy | reckless | – | (a drone's 9) | – | home |

The crawler is measured in a test: in its view at 12 m, by daylight.

| Machine | Sees you → charge (the tell) | Charge → zap | Harm |
| - | - | - | - |
| crawler | 0.32 s | 0.56 s | 12 a zap, every 2.1 s, out to 18 m |

What the numbers say:
- **Crawler: unfair.** Its zap lands 0.56 s after the crackle, under a
  reaction time. It's the walker's old 0.7 s aim, which was ruled unfair
  and fixed for the gunners.
- **Hound: unfair, in one place.** The noticing gives warning: it stalks,
  4–29 s before the first coil. Its coil before the bite (0.6–0.8 s) is
  short but fits a lunge. What isn't fair is that within 4 m, the
  point-blank rule sees through keeping still. So the driver's advice can't
  work once it's on you, and it bites you to death where you lie. Fighting
  works: the reckless newcomer lamed it and got home.
- **Heavy: fair in the fight, unfair at home.**
  - The fight: the 1.2 s raise, then the slam 0.2 s later, is a tell you
    can read. From the raise, someone sprinting covers 3.2 m after a 0.7 s
    reaction, and the slam's radius is 1.7 m.
  - Escaping: it's slow (hunt pace 1.3), so walking away works.
  - What's unfair: nothing stopped a hunt that followed you home.

## The gunners: tell to first round (done, in main)

Measured first, with no changes:

| Machine | Noticed → 1st round | Tell → 1st round | Range at 1st round | Home |
| - | - | - | - | - |
| walker | 2.1–3.0 s | **0.7 s** (aim settle) | 39–52 m | 1/5 |
| drone | 3.0–7.3 s (median 3.7) | **1.5 s** (lock chirp; once 4.1) | 29–52 m | 0/5 |
| sentry | 1.4–1.6 s | **1.6 s** (chirp) | 24–45 m | 1/5 |
| broken walker | shoves only | – | – | 5/5 |
| hush / night | – | – | – | 5/5 each |

Does backing off or taking cover save you? Almost never, before the fix:

| Reaction | vs walker | vs drone | vs sentry |
| - | - | - | - |
| low and still / cover (cautious, one-handed) | 0/2 | 0/2 | 0/2 |
| runs (lost) | 0/1 | 0/1 | 0/1 |
| carries on (hoarder) | 0/1 | 0/1 | 0/1 |
| reckless | home | down (took it on) | home |

What changed:
- A walker's every round now comes a full 1.5 s after its aim tell, and
  it re-tells when it finds you again.
- A drone's first burst after a lock warns and hurts no one. It breaks
  off after 3 locks without a hit, or after 60 s. It doesn't lock on to,
  or fire at, anyone under a roof.
- The sentry chirps 2.5 s before its first round, and that first burst
  warns.

The tests are `tells_tests::every_gunner_warns_before_it_hurts` and the
drone and sentry tests.

After the change, in the first fights (lead is the first tell to the
first hit, in s):

| Machine | Home before → after | Tell → first hit, before → after |
| - | - | - |
| walker | 1/5 → 1/5 | cautious 5.2 → 10.5, lost 10.6 → 16.6, hoarder **0.8 → 3.8**, one-handed 5.2 → 4.9 |
| drone | 1/5 → 2/5 | cautious 13.2 → **123.5, now home**; reckless 40 → 42, home |
| sentry | 2/5 → 2/5 | cautious 3.4 → **68, now home** |

Over five-job careers (`first_jobs_<newcomer>`), downs went 28 → 4. The
cautious newcomer went 8 → 0.

The broken walker no longer stands on someone crouched: `walker::keep_clear`,
and its shove reaches from its body's edge. Before, a one-handed newcomer
was pressed under the ground and fell forever. The test is
`broken::tests::it_never_stands_on_someone_crouched`.
