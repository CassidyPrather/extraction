# Progression

Why go on raid 10, or raid 30? The game already has raids, the truck as
home, a no-currency economy (loot feeds the bench, the kitchen and the
driver), a career across three valleys and an ending (recalling the
machines). This file is about the arc that holds them together: what the
player wants at each timescale, what the game offers, and how long it
takes. The rules live in `extraction_core::progression` and the game side
in `src/progression/`; the numbers below are their constants.

The identity stays: **nothing is bought, everything is physical, the truck
is home, and the words are on things** (cards, terminals, the wrist).
Progression is never a number you spend. It is jobs done for the driver,
and the driver fitting out the truck in return.

## What already pulls you forward

| Thing | Timescale | What it gives |
| - | - | - |
| Loot for the bench and the kitchen | one raid | rounds, repairs, EMP grenades, a meal |
| The driver's request (`raid/requests.rs`) | one raid | a cache of the good stuff, out in the Zone |
| Artifacts and gadgets | a few raids | odd powers; the attention they cost |
| The Surge (every ~60 Zone-minutes) | two or three raids | fresh loot, a new weather, caches swept |
| Danger tiers and region richness (`MAP_DESIGN.md`) | a few raids | better loot where it's worse to be |
| Three valleys | the career | new ground |
| The three keys and the recall (`capstone.rs`) | the career | the ending |

What was missing: between "one more request" and "the ending" there was
nothing to *work towards*. The request was the same size on raid 3 and raid
30, its cache was the same pile, and the truck never changed. Players need
a ladder they can see from the bottom.

## The loop at three timescales

### This raid (15-40 minutes): a job with a place

*Wants:* a reason to go *somewhere*, not just "out"; something to tell the
driver about when you get back.

*Offer:* **the driver's job** on the request card, one at a time. Three
kinds, each tied to the map:

- **Fetch** — "2 x Sensor eye: someone in town pays in diesel." As before:
  stash them, press the card. Fetches grow with the tier: tins and matches,
  then robot parts, then a gun pod or an artifact.
- **Recover** — "The surveyor's recorder: in the pump house by the Pumping
  station." The driver names a building in a region of the tier's danger;
  a field recorder (a real item, 1.1 kg) lies there. Bring it home in the
  stash and press the card. The Surge doesn't sweep it (it's put back),
  and if the convoy takes it the driver sighs and names another place.
- **Scout** — "Walk up to the Rail yard and look about. Tell me what's
  there." Stand within 15 m of the place: the wrist says "Noted. Now get it
  home." It only counts if you get home; going down forgets it.

The job's place is pencilled on the field map (a blue ring; the cache keeps
its red cross), so the map case tells you where this raid is going. The
pay is still the cache (unchanged: a pile of ammunition, gadgets, medicine,
now and then a gun, left out in the Zone for you to fetch). A finished job
is replaced by the next one straight away, so you read the next job before
you set off.

### This session (3-6 raids, about one Surge cycle): a tier

*Wants:* a visible milestone within an evening; the truck to feel more
like home.

*Offer:* **tiers of jobs.** Each tier is a few jobs at one level of
danger. Finish them and the driver fits something to the truck: you come
home and it's there, bolted in, with a line on the wrist. The card heads
each job with its tier and count ("ERRANDS · JOB 2 OF 3") and says what's
coming ("Do these and I'll fit a vice to the bench").

| Tier | Jobs | Where | Fitted at the end |
| - | - | - | - |
| 1 **Odd jobs** | 3 | danger 1 (the village, the farm) | **Stash rack**: a timber rack over the crate, the stash grows 200 → 300 L |
| 2 **Errands** | 3 | danger 1-2; robot parts | **Bench vice**: the bench learns six more recipes (side rail, weapon light, EMP trap, careful repair, door wedge, 7.62x39 for the SKS) |
| 3 **Runs** | 4 | danger 2-3 | **Whip aerial** on the cab: the truck radios a Surge warning at ten minutes instead of five, and robot chatter says how far |
| 4 **Far jobs** | 4 | another valley, danger 2-3 | **Medicine cabinet** on the front wall (a dressing in it each raid) and a **second rack**: 400 L |

The last job of the last tier is always the same, whatever valley you're
in: "There's a house by the well in the village. On the table. Bring me
what's there." It's Hana's letter (`hana_letter`, see `docs/LORE.md`), and
it ends the driver's ladder on his own story.

Then **standing jobs**: the driver keeps asking (any kind, any valley,
danger 2-3) and keeps paying in caches, for as long as you play.

Stash space is the upgrade that bites. The crate starts at 200 L (five
backpacks' worth); a career that hoards guns and robot parts fills it in
ten raids or so, and then every gun pod on the shelf is a choice. The
racks are the answer to "my stash is full", which is what a long-term
extraction player actually says.

### The career (30-60 raids): the valleys, then the keys

*Wants:* to have *been* somewhere; a record; an ending that means
something.

*Offer:* the tiers walk you up the danger ladder valley by valley: tier 1
keeps you near the insertion, tier 2 sends you past the first ridge, tier 3
into the danger-3 places (the Hollow, the quarry, the slag heaps), which
are exactly where the keys are. Tier 4 sends you to the other valleys.
By the time the driver has nothing left to fit, you have walked every
valley's worst place once, and the capstone's three keys are the only
thing left to do. The arc ends where the game's story does.

**The career page** is the logbook's second page (press the logbook to
turn it): raids by valley, the tier and job count, what's fitted and
what's next, the keys at home, the memorial's count. It's the one place
the whole arc is written down, in pencil, in the truck.

## Where it lives

- `extraction_core::progression`: tiers, upgrades, contracts and how
  they're rolled from a map (`roll_contract`, deterministic in the career
  seed and the job number), and the stash capacity per upgrade. No Bevy;
  unit-tested. The bench's vice recipes are `fabrication::VICE_RECIPES`.
- `src/progression/`: the `Progression` resource, fitting upgrades (the
  stash's capacity, the bench's tier, the aerial's warning, the cabinet),
  the recorder's placement, scouting, the career page.
- One job is handed in per raid ("One thing at a time. After your next
  run."): a full stash can't clear fetches back to back at base.
- `src/progression/lines.rs`: every word the driver says about his jobs,
  apart from the logic (Story writes them), with a test holding the wrist
  lines to three lines of 26.
- `src/raid/requests.rs`: the card now shows the job; pressing it checks
  whichever kind it is.
- `src/persist.rs` / `extraction_core::save`: `ProgressSave` on the career
  (`#[serde(default)]`, so older saves load). An older career is credited
  with the requests it already met, so a veteran starts on the right tier.
- Truck props (`src/render/upgrades.rs`): the racks, the vice, the aerial,
  the cabinet, modelled in `art/props/truck_kit.py` (`models/props/<id>.glb`)
  over blockouts.
- The job's place is a blue ring on the field map (`gadgets/visuals.rs`),
  the cache a red cross.

## The first job

Fixed, so the training yard can hand it over: **Scout** the danger-1 point
of interest nearest the insertion (Marrow Creek: the Old farm; Kettle Pass:
the Logging camp; Cinder Works: Ash Row). "Walk up to the Old farm and have
a look round. Come back and tell me." The yard's last board says the card
in the truck has your first job. It teaches the
whole loop in one raid: go somewhere, call the truck, get home, press the
card, and the cache it pays shows up as a red cross on the map.

## Pacing

How many raids each tier takes, at the sim bots' rates (see "Measuring it"
below for how the numbers were made).

Measured on 36 sim raids on the merged polish build (12 in each valley,
seed 21, bots of every personality: 53% home, 9 minutes a raid). `python3
scripts/progression_pacing.py all` walks 3000 careers through the tiers
on those raids:

| Tier | Jobs | Danger | Raids for the tier (median) | Career so far: median (1 in 10 take) |
| - | - | - | - | - |
| 1 Odd jobs | 3 | 1-1 | 7 | 7 (12) |
| 2 Errands | 3 | 1-2 | 8 | 15 (22) |
| 3 Runs | 4 | 2-3 | 11 | 26 (36) |
| 4 Far jobs | 4 | 2-3 (another valley) | 7 | 33 (44) |

By valley, the whole ladder takes a median of 34 raids in Marrow Creek, 39
in Kettle Pass and 31 in Cinder Works. An earlier run, before the robots
and guns of this polish round were merged, had bots home 79% of the time
and the ladder at 23 raids (Cinder 36 then, from fetches the valley can't
give: its bots bring home no food or robot parts; fetches of "robot parts,
any" and a three-raid swap brought it to 32).

What it says:

- **The first evening has a fitting in it.** Tier 1 is about seven raids:
  the stash rack comes in the first or second session.
- **Getting home is the pace.** Every job but a fetch from the stash needs
  the raid to end at the truck; only half of the bots' raids do now, and a
  quarter reach a danger-3 place and get home:

| Danger reached and home | Share of raids |
| - | - |
| 0 | 53% |
| 1 | 53% |
| 2 | 53% |
| 3 | 25% |

- **Robot parts are the bottleneck of fetches.** The bots bring home a
  robot part (any kind) about once in twelve raids (they fight what comes
  for them and rarely strip the wreck). A fetch for one used to be the way
  a career could stall for good, so a fetch may take any robot part, and
  the driver gives up on an unmet fetch after three raids
  (`FETCH_PATIENCE`). A player who hunts robots meets them far sooner.
- **The ladder ends around raid 33 (1 in 10: past 44)**: fifteen hours or
  so at a player's pace. Then the standing jobs and the keys: by then tier
  3 has walked you through the danger-3 places where the keys lie, and
  the last job (Hana's letter) has closed the driver's story.

The bots don't take jobs, and their raids are short and timid next to a
player's, so read the numbers as the shape of the arc, not its minutes.

## Measuring it

`scripts/progression_pacing.py <map|all> [runs-dir]` reads the sims'
telemetry (each run's `end` line: outcome, loot ids, the job, and its
`pos` trail through the regions) and walks a few thousand careers through
the tiers, raid by raid, on those raids: every raid home adds its loot to
the stash, so a fetch is often met from what's piled up.

Sim bots take jobs: each sim run is handed a scout job of one of the first
three tiers (the run's seed picks which), and the raider treats the job's
place as a site worth a long walk (while it's fit for it). The `end` line
says whether the place was seen and the bot got home, and the pacing
script uses that, by the place's danger, as a place job's odds per raid
(falling back on "reached that danger and got home" for runs from before).
Fetches aren't played: the bots don't hunt items to order.

## What's cut

- **Reputation or standing as a number.** Tier progress is a count of jobs
  on a card, not a meter; nothing is spent.
- **A trader.** The driver never sells; the caches are found, not given.
- **Skill trees and perks on the player.** The truck improves, not you;
  your hands are your skill.
- **Bench tiers beyond one upgrade.** One vice is a clear step; a ladder
  of benches is a shop by another name.
- **Choosing jobs from a list.** One job at a time keeps the card readable
  in a headset and the decision on the map, not on a menu.
- **Bots taking jobs in the sims.** Pacing is computed from their loot and
  routes instead.
