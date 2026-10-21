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
| 2 **Errands** | 3 | danger 1-2; robot parts | **Bench vice**: the bench learns seven more recipes (side rail, weapon light, EMP trap, careful repair, door wedge, 7.62x39 for the SKS, 7.62x25 for the PPSh) |
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

## After the ladder (raid 30 and on)

By raid 30 the driver has fitted everything, the letter is home, and the
keys are the last thing the story asks. What keeps raid 40 from being
raid 30 again? Three things, all physical, none of them a number to spend:

**Deep jobs.** Past the ladder the driver's standing jobs only send you to
the worst places: danger 3, and the underground sites (the shelter, the
mine gallery, the service tunnel). They're the same three kinds (fetch,
recover, scout) but never easy, and the cache they pay is three piles
instead of two. Raid 40 is a raid to the Hollow or the quarry because the
card says so, not because the loot table does.

Half the deep fetches are one named **stone** (`progression::STONES`):
a featherstone, a hearth, an aegis knot, a lodestone or a mirror shard,
each asked for with a line that hints at its harvest trick ("A hearth.
Don't pick it up with your hands. Let it cool first.", see
`docs/ANOMALIES.md`), and now and then (15%) a stormglass, which means
staying out into the Surge. A stone sealed in its lead case in the stash
counts. After the epilogue the quiet jobs ask for none.

**The fourth valley.** Sallow Reservoir (`docs/FOURTH_VALLEY.md`) doesn't
come up on the route card until the career has raided Marrow Creek, Kettle
Pass and Cinder Works (`Progress::opens`). It has no key; the capstone
stays three.

**Your order.** With the ladder done the driver will take an order: a
second card under the job, pressed to cycle, says what you want his caches
to lean to: ROUNDS (ammunition), MEDICINE, GADGETS, or ANYTHING (as
before). His caches are still found, carried and got home; you choose what
kind of pile is waiting. It's the one choice the economy gives you over
what enters the world, and you earn it by finishing his jobs.

**Upkeep.** Fittings wear. Every six raids each one wants a little from
the stash, taken when you get home and said on the wrist ("Driver: took
two scrap for the rack"):

| Fitting | Upkeep, every 6 raids |
| - | - |
| Stash rack, wall shelf | 2 scrap metal each |
| Bench vice | 1 gun oil |
| Whip aerial | 1 electronics |
| Medicine cabinet | 1 field dressing |

If the stash hasn't got it, that fitting is **laid up** until it has (the
next time home with the parts): the rack's room shrinks back (what's on it
stays, nothing more goes in), the vice's recipes go, the aerial goes
quiet, the cabinet stops. The career page lists what's laid up and what it
wants. It starts six raids after each fitting, so it's a sink across the
middle of the career too, and it's the reason scrap and oil are worth a
pocket at raid 50. It's deliberately small: about one item a raid against
the bots' 3-4 of the kinds it asks for.

**Measured** (the same 54 job-taking sim raids, careers walked on to raid
60 by `scripts/progression_pacing.py`): past the ladder a deep job is done
about **every 2.6 raids** (the ladder's last tier took about one every 2),
so raid 40 is a run to a danger-3 place or underground most times, for
three piles of what you ordered. Upkeep at the bots' loot rates:

| Part | Home per raid | Upkeep per raid (all fitted) | Share of income |
| - | - | - | - |
| scrap metal | 1.56 | 0.67 | 43% |
| gun oil | 1.00 | 0.17 | 17% |
| electronics | 0.72 | 0.17 | 23% |
| field dressing | 1.54 | 0.17 | 11% |

It never lays a fitting up for a career that keeps what it finds (0% of
raids, every fitting); what it does is take nearly half the scrap, which
the bench also wants for rounds and spare parts. That's the intended
size: a pull on what you carry home and what you strip at the bench, not
a tax that shuts the truck down. A player who strips everything for rounds
will see the rack lie up, and that's the decision it's there to make.

Cut from this: a reputation number (the order card is what reputation
would have bought); per-fitting repairs at the bench (upkeep is taken, not
crafted, so it never becomes a menu); rotating "seasons".

## Safehouses: staying out overnight

Raids were in and out. Into the Radius's best tension is the long walk
with a safe room in the middle, so each valley has one cabin you can
claim and sleep in (`extraction_core::safehouse`, `progression::safehouse`):

| Valley | Cabin | Where |
| - | - | - |
| Marrow Creek | Logging camp, the south cabin (-145, 190) | danger 1, 200 m in, the west side's halfway house |
| Kettle Pass | Charcoal burners' hut (119, -138) | danger 2, 410 m in, the climb's last stop |
| Cinder Works | Woodward's cabin (-75, -159) | danger 2, 430 m in, in the Cinder wood |

**Claiming** is three physical things, in any order: **clear** it (be
inside with no machine within 30 m), **bar** it (a door wedge set at its
door, the same wedge that stops a walker), **light its stove** (press it
with matches on you; one is used). Then its bed is yours, a crate of its
own appears (60 L, saved with the valley, not with the truck's stash),
and a radio on the table.

**Sleeping**: lie on the bed with the door wedged. The view fades; a night
passes (15 minutes of Zone clock, so a real bite out of the hour between
Surges, and refused if the Surge would come before morning); the hour
moves on one step (dusk to night, night to dawn: "Grey light under the
door."); a night behind a wedged door cools the Zone's attention. If you
lie down loud (the second feeling or worse), the radio warns you in the
dark and something comes to the door before light: two walkers from the
Gate (Robot minds' `NightVisit`) that batter the wedge down and search the
cabin for up to ten minutes.

**The truck** waits one night if you radio it from the safehouse (press
the radio); untold, it leaves the rendezvous while you sleep.

**Expeditions**: past the ladder, once you've slept out, half the recover
jobs are expeditions to the valley's deepest place ("Two days' walk. Sleep
at the cabin on the way."). The recorder only counts if the raid that
brought it home had a night out; the logbook records nights out.

**The cost**: a safehouse can be found. Come home from a loud raid (the
third feeling, "the air hums around you", or worse) and the machines have
followed your trail to it: the door's broken, the crate tipped out over
the floor, and you'll find it like that next time. The Surge ends a claim
(and empties the crate: the Zone takes what it's given).

**Leftovers**: each cabin's table has a note from its last people
(Story's), and what the note promises is there, each Surge cycle until you
clear the cabin: the logging crew's tin under the Marrow bed, the
woodward's matches by his stove in the Cinder wood.

Cut from this: hunger (there still is none; food is the kitchen's and the
offerings'), a sleep timer or a menu (you lie down; a night is a night),
more than one safehouse a valley.

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

Measured on 54 sim raids on the merged polish build (6 in each valley for
each of seeds 50, 56 and 62; bots of every personality, each handed a
scout job: 74% home, 10 minutes a raid). `python3
scripts/progression_pacing.py all <dir>` walks 3000 careers through the
tiers on those raids:

| Tier | Jobs | Danger | Raids for the tier (median) | Career so far: median (1 in 10 take) |
| - | - | - | - | - |
| 1 Odd jobs | 3 | 1-1 | 6 | 6 (10) |
| 2 Errands | 3 | 1-2 | 6 | 12 (19) |
| 3 Runs | 4 | 2-3 | 10 | 22 (31) |
| 4 Far jobs | 4 | 2-3 (another valley) | 9 | 31 (41) |

**The range:** by seed, the whole ladder takes a median of 29, 36 and 30
raids; by valley, 30 in Marrow Creek, 29 in Kettle Pass and 37 in Cinder
Works (the slowest: an industrial valley with little food to fetch, so its
fetches more often run out the driver's patience). Read the ladder as
**about 30 raids, 25 to 40**. Single seeds of a dozen raids swing much
more than that (an early run on one seed said 23, another 33), which is
why the table pools three.

The bots that took scout jobs saw the place and got home 56% of the time
at danger 1, 52% at danger 2 and 2 times in 4 at danger 3, against 74% for
raids in general: a job pulls you somewhere you'd not have gone, and that
costs. Danger 3 has few samples; the script leans on how often raids
reached danger 3 and got home (28%) until there are more.

By play style the job comes out level: cautious bots saw the place and got
home 5 times in 9, looters 10 in 18, hunters 8 in 18 (their artifacts draw
response teams), reckless bots 6 in 9 (they get there fast, and get away
with it on a short trip). Robot minds' sims say careful play all but ends
drone deaths; a job is a short trip, which suits the bold, and the deep
jobs past the ladder are where that stops being true.

What it says:

- **The first evening has a fitting in it.** Tier 1 is about six raids:
  the stash rack comes in the first or second session.
- **Getting home is the pace.** Every job but a fetch from the stash needs
  the raid to end at the truck, and a quarter of raids at best reach a
  danger-3 place and get home:

| Danger reached and home | Share of raids |
| - | - |
| 0 | 74% |
| 1 | 74% |
| 2 | 74% |
| 3 | 28% |

- **Robot parts are the bottleneck of fetches.** The bots bring home a
  robot part (any kind) about once in five raids, a given part far less
  (they fight what comes for them and rarely strip the wreck). A fetch for
  one used to be the way a career could stall for good, so a fetch may take
  any robot part, and the driver gives up on an unmet fetch after three
  raids (`FETCH_PATIENCE`). A player who hunts robots meets them far sooner.
- **The ladder ends around raid 30**: twelve to fifteen hours at a
  player's pace. Then the standing jobs and the keys: by then tier 3 has
  walked you through the danger-3 places where the keys lie, nights have
  joined the Surge's table (`Progress::nights`), and the last job (Hana's
  letter) has closed the driver's story.

The bots' raids are short and timid next to a player's, so read the
numbers as the shape of the arc, not its minutes.

## The economy pass (pre-hound)

What fills and drains the stash, from job-taking sims across the three
valleys, bots with and without late armour (`SIM_ARMOUR=late`: vest,
plate and helmet from tier 3). Measured on 6a429e9, the build before the
hounds, with Robot bodies' wreck-joint fix (1aefd75) cherry-picked for the
"after" runs; `python3 scripts/economy_report.py <dirs>` and
`scripts/progression_pacing.py all <dir>`. Seeds 80 and 90, 8 raids a seed
a valley an arm. Read the two sets side by side with care: before the fix
the physics crash took 40 of 96 raids, mostly ones with a fight in them,
so "before" is a calmer sample (80% home) than "after" (65%).

**Does the ladder still take about 30 raids?** Yes: a median of 27 (1 in
10 take 35) before, 32 (41) after, the difference being the home rate of
the sample, not the loot.

**The three worst findings, and what changed:**

| Finding | Before (per raid) | Change | After (per raid) |
| - | - | - | - |
| Spare durables pile up with nothing to eat them (multitool, flashlight, compass, knife, door wedge) | 2.00 | lower weights in `tools` and `gadgets`; two bench strips: two multitools to spare parts, two flashlights to a board | 1.26 (the wedge, now the safehouse's, kept at 0.4) |
| Brass never comes home and powder hardly, so the vice's seven re-packs never run | brass 0, powder 0.04 | brass in `ammo_common` (spent cases) and `workshop`, powder in `workshop` | brass 0.31, powder 0.17: a box re-packed every 6-12 raids |
| Fetches of servos and gun pods stall the middle of the ladder | servo 0.04 (23-31 raids a fetch), gun pod 0.02 (31) | the odd servo in `workshop`; gun pods 0.3 to 0.5 on walker wrecks | servo 0.22 (7-10 raids), gun pod 0.11 (10) |

The bolt (1.2-1.5 a raid) looks like a pile-up but is thrown at anomalies.
Food fetches (stew, tea) still take 16-20 raids at the bots' rates, but the
bots value food least and don't hunt to order; a player on a "something
hot" fetch looks in kitchens. Upkeep takes 45-62% of the scrap that comes
home and never lays a fitting up.

**Is the late game trivial?** Not before the hounds. Home rates by stage
(after): odd jobs 50-58%, errands 58-67%, runs 58-67%, deep jobs 67%
unarmoured and 92% armoured. Armour makes the deep jobs safe for a bot
that has it, and drones are most of the deaths at every stage. Rerun this
with the hounds (and the heavy) on before tuning armour.

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
