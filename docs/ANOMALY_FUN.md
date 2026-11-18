# Anomaly fun (#108): a proposal

The user: the tutorial's anomalies are a one-time puzzle ("how do I get
the stone out of the fire?"). Once solved, the kind stops being
interesting. They want (1) the taking to stay a puzzle with many answers,
solved once, and (2) the anomaly's ongoing effect, its place in the map,
and its ties to the world to be the lasting challenge.

So this proposal is in four parts:

- **(A)** more ways in for the one-time puzzle;
- **(B)** each kind as terrain, every raid;
- **(C)** the world acting on anomalies, and anomalies on each other;
- **(D)** one yard beat that shows (B).

Everything is ranked by fun per day at the end. Nothing here is built
yet: this is for the coordinator to decide.

Rules held throughout:

- Every new bite is perceivable before it lands (at least `MIN_TELL`;
  slow and strategic first).
- Danger follows what the player does and where they go.
- No combination may remove a kind's tell. A silent snare (a snare in a
  hush) is out, for that reason.

## What's already there, and nobody knows

Much of (B) exists on the machines' side (`robots::hazards`, its table
of what every hazard does to every machine). The player is never shown
it:

| Kind | Does to machines today | Does for or against you today |
|---|---|---|
| Lift | throws a drone when it pops; staggers and slows a walker | half gravity inside; the pop flings you 8.5 m/s up, 5 out (6 hp) |
| Burner | cooks robot parts; heats a heavy | burns you |
| Snare | discharges into them; a chasing hound runs straight across it (the lure) | strikes a hand or metal you hold |
| Well | drags and crushes walkers; bends rounds | bends rounds: a robot shooting at you across a well misses |
| Mirror | dazzles: a machine passing the fold loses you | blinks you radius + 3 m on |
| Hush | deafens a machine standing in it; a drone in one loses its lock | your shots and steps aren't heard |
| Frost | nothing (the hazard table has no Frost row) | chills and stiffens you |

Machines path round live anomalies unless they're chasing hard, in sight
of you. That chase rule is the lure, and it's the best toy already in the
game. The cheapest fun is teaching it: one driver line, one yard beat,
and a hazard tell the player can read on the machine (the `HazardStruck`
tells already exist).

## (A) Taking the stone: three or more ways in per kind

| Kind | Today | Cheap new ways in (days) |
|---|---|---|
| Lift (featherstone rides 3 m up) | throw junk in so it winds up, pops, and you catch the stone | **shoot it down**: a round knocks the stone out of the column (0.5); **weigh it**: heavy junk winds it faster (exists, teach it); **ride it**: stand in it at half gravity and jump for the stone, then take the pop (exists, teach it) |
| Burner (hearth, burns the hand) | shoot or flick it out, then let it cool or drop it in water | **rake it** out with the crowbar's reach (0.5); **a rimestone carried**, and the coals don't take you (exists, teach it); **wet gloves**: soaked through by rain or wading, you hold it twice as long (0.5) |
| Snare (aegis, strikes a reaching hand) | ground it with thrown metal, reach in while it recharges | **let a machine set it off**: lure a hound across, then reach in (exists, teach it); **rain grounds it**: in a downpour it fires into the wet ground on its own every few seconds, a window you can see (C, 1) |
| Well (lodestone at the crushing heart) | dash in and out | **feed it**: each thing it crushes sates it, and its pull drops for 3 s, a slack you can see in the grit (1); **hook it** with the grappler's line, Robot minds' #100 (1, shared) |
| Mirror (shard in the fold) | throw something through, reach in while it closes | **walk through backwards**: you come out facing it with the fold still closing, and reach back (exists, teach it); **let a machine cross it** while you wait (exists) |
| Hush (hushstone; the silence presses after 8 s) | in and out before 8 s | **fill it**: a live radio or ticking clicker dropped inside pushes the pressure back while it plays (0.5); **from outside**: knock the stone out with a thrown thing (check that it works) |
| Frost (rimestone; it chills, then bites) | a live hearth carried | **knock it out** with thrown junk (check); **go in warm**: straight from shelter, the chill starts from nothing (exists, teach it); **a burner beside it** (C, the steam pair) |

The ways marked "teach it" cost nothing to build. They need a line on a
field note or the radio, owned by Onboarding or Story.

## (B) Each kind as terrain, every raid

| Kind | As a tool | Against you | New work (days) |
|---|---|---|---|
| **Lift** | **A step.** At half gravity you clear a wall or reach a roof; the pop is a fling across a gap. Map design puts one lift a valley beside a vantage (a roof, a ledge, a ditch). Make the pop throw toward where you face, not straight out, so it can be aimed. | It drags loose loot away, and a bag you've dropped. | aimable pop, landing kept under fall damage (1.5); placement, Map design (1) |
| **Burner** | **A cooker.** Lure a walker into its edge (exists). A downed machine's parts left in it cook: brittle parts that break for scrap or better salvage, tying into #105. | **At night its glow lights you**: machines see you further near a burner. You can see why: you're standing in the light. | cooked salvage with #105's owner (1); the night glow in machines' sight, Robot minds (1.5) |
| **Snare** | **A fence.** Lure a hound across (exists). A line of snares across a pass holds machines off you. | Metal on you draws it from further (exists). | none: teach it |
| **Well** | **A shield.** Fight from behind one: rounds aimed at you bend away (exists). It also pulls a grappler's line off true (#100). | It drags loose things and walkers. Your rounds bend too. | the grappler line, Robot minds (1) |
| **Mirror** | **A route.** Blink across a fence, a stream or a gap. Map design puts each valley's mirror on a crossing. It also breaks pursuit, since a machine passing the fold loses you (exists). | You come out where you didn't choose. | placement, Map design (1) |
| **Hush** | **A nest.** Shoot from inside unheard, 8 s at a time (exists). A drone in one loses you (exists). | **Machines come through it silent.** Check that a hush eats their steps as it eats yours. If so, make what moves in it show: the hanging dust stirs, so the silence has a tell. | dust stir (0.5) |
| **Frost** | **A bridge.** A frost over water freezes it walkable, a crossing that isn't there without it (Sallow's reservoir arms, Rime's streams). **It slows machines too**: give the hazard table a Frost row (Hampered). | The chill. | ice collider and look (2); the hazard row, Robot minds (0.5); placement, Map design (0.5) |

## (C) The world on anomalies, and anomalies on each other

| What | Effect, and its tell | Days |
|---|---|---|
| Rain, on snares | They fire into the wet ground on their own every few seconds: flashes you see coming. A window to grab the aegis, but also arcs where there were none. | 1 |
| Rain, on hearths | A hot hearth cools in 8 s, not 20. It steams as it cools. | 0.5 |
| Snow (Rime), on burners | A dark melt ring round each burner, seen from far off: in the snow, burners give themselves away. | 0.5 |
| Wind, on lifts | The column leans downwind. What it throws drifts, and its leaves show which way. | 1 |
| Night | Burners and snares light the ground round them, for you and against you (see Burner above). | (in B) |
| The Surge | Fields already creep downhill each cycle and re-scatter every sixth. Make it show: drag scars (exist), and a radio line after a re-scatter. | 0.5 |
| **Pair: burner and frost** | Where their reaches meet, **steam**: a cloud that blocks sight, yours and the machines'. Something to cross unseen, or something that hides a hound. | 2 |
| **Pair: snare and water** | A snare reaching water **charges it**: wading within a few metres shocks you. Crackling ripples show how far it reaches. | 1.5 |
| **Pair: lift over a well** | A lift's pop throws what it holds into the well's pull. It's chaos, and costs a lot to make readable. | not now |
| **Pair: snare in a hush** | It would remove the snare's crackle, so its tell. | **never** |

## (D) The yard: one beat where an anomaly helps

The yard teaches taking the stone (A). After the snare and the stone,
add one beat where an anomaly is useful:

**A lift as a step.** A lift sits beside the yard's container, with the
next station's lamp on its roof. You can't climb up. In the lift you're
light: jump and you're on the roof. The driver: "Light in there, isn't
it? Use that."

This needs a lift placed by Map design and Onboarding, and the aimable
pop from (B). It costs 1 day once the lift work is in.

The alternative, with nothing new to build, is a scrap walker on the
yard's rail that rolls into the sparks: the lure, shown. It's weaker,
because you watch it rather than do it.

## Ranked by fun per day

| # | What | Days (mine) | Others |
|---|---|---|---|
| 1 | Teach what exists: the lure, the well as a shield, mirror and hush against machines, the "teach it" ways in | 0 | Onboarding and Story, about 1 day of lines |
| 2 | Lift as a step and an aimed fling, plus the yard beat | 1.5 | Map design 1 (a lift by a vantage in each valley and in the yard); Onboarding 0.5 |
| 3 | Cheap ways in: shoot the lift's stone down, rake the hearth, wet gloves, fill the hush, feed the well | 2.5 | none |
| 4 | Rain on snares and hearths | 1.5 | Audio (the ground-strike crack) |
| 5 | Mirror as a route | 0 | Map design 1 |
| 6 | Hush dust that shows what moves in it | 0.5 | Audio check |
| 7 | Frost as a bridge, and a Frost row for machines | 2 | Map design 0.5; Robot minds 0.5 |
| 8 | Snare charging water | 1.5 | Audio |
| 9 | Steam where a burner meets a frost | 2 | Robot minds (sight blocked by steam) |
| 10 | Cooked salvage | 0.5 | #105's owner 1 |
| 11 | A burner's glow lighting you at night | 0.5 | Robot minds 1.5 |
| 12 | Snow melt round burners; wind on lifts | 1.5 | none |

1–3 are about 4 days of mine and make the biggest difference. All of
it, 1–12, is about 14 days of mine and about 8 of others'.

## Frame budget

- No new lights. Glow uses the marks' emissive (as today), and night
  sight reads the anomaly's light from its kind and charge, not from a
  render.
- Steam and charged-water ripples reuse the anomaly particle pool, with
  no new per-anomaly allocations. Steam is one volume a pair, and pairs
  are rare by placement.
- Machines' sight through steam: one sphere test a sight check, against
  the few steam volumes.
- An ice bridge is one flat collider. Nothing new runs every frame
  beyond what each kind already ticks.

## Owners

- **Robot minds:** the Frost row, steam blocking sight, night glow in
  sight, the grappler line in a well.
- **Map design:** a lift by a vantage, mirrors on crossings, frosts over
  water (Rime, Sallow), the yard's lift.
- **Onboarding and Story:** the lines that teach what exists, and the
  yard beat.
- **Audio:** the snare's ground-strike, the charged water, hush checks.
- **#105's owner:** what a cooked part is worth.
