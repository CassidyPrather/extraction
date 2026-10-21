# Armoury

The seven guns the game started with (PM, AK-74, Mosin, SKS, Nagant,
PPSh-41, the TOZ double) cover a pistol, an assault rifle, a bolt rifle,
a clip-fed carbine, a revolver, an open-bolt sub-machine gun and a
break-action shotgun. This page records which guns came next, why each
one earns its place, and what it took to put one in the game.

Each new gun is picked because it brings a **mechanism** the hands
haven't done yet or a **role** no gun fills, and because a scavenger in
a post-Soviet zone would plausibly find one: in a hunters' lodge, a
militia store, a signal box, a collapsed police post.

## The new guns

| Gun | id | Brings | Where it turns up |
| - | - | - | - |
| Baikal MP-133 pump shotgun | `mp133` | **Pump action**: a tube magazine thumbed full a shell at a time through the loading port under the receiver, the forend pumped back and forward to cycle, an action release to open it on a live round. Its fault is the short stroke. | farmhouses and hunters' places everywhere; commoner than the double in Marrow Creek |
| SVD Dragunov | `svd` | **The marksman's rifle**: semi-automatic 7.62x54R from a ten-round box, a side rail the PSO-1 clamps to, a cheek rest. The Mosin's reach without working a bolt between shots. | rare; Kettle Pass's long sightlines, militia stores |
| TOZ-8M | `toz8` | **Quiet and cheap**: a single-shot .22 bolt rifle. Robots hear it at about a third of a pistol's reach, and a box of fifty weighs nothing. Useless against plate, good for an eye, a joint, a rook. Rimfire misfires: lift the bolt and drop it to strike again. | common in Marrow Creek (hunting country) |
| SPSh-44 signal pistol | `flare_pistol` | **Light and a lure**: a 26 mm break-open flare gun. A white star lights the ground under it for eight seconds as it sinks; a red or green one is a signal. Robots look at a burning flare and go to where it falls; a flare to the face dazzles. It lights you too. | signal boxes, rail and works sites: Cinder Works, the reservoir |
| RPK-74 | `rpk74` | **Resting on cover**: a long, heavy-barrelled AK with a folding bipod. Swing the legs down and set them on a wall top, a sill or the ground, and the gun lies still and soaks up its recoil. Takes AK magazines, and its own 45. | rare; Cinder Works and the militia posts |

A sixth, a home-made crossbow (silent, bolts you pull back out of what
you shot), is the next candidate: it fits "nothing is bought" best of
all, and its retrievable projectile needs more new code than the rest.

## What a gun needs

A gun is in the game when it has every one of these. The list is the
checklist for the next one.

1. **Catalog** (`assets/data/items.ron`): the gun, its magazine or clip
   if any, its ammunition box and loose round, with real masses and
   dimensions matched to the model.
2. **Model** (`art/guns/<id>.py`, `art/mags/`, `art/items/`): one object
   per motion (`static`, `bolt*`, `turnbolt*`, `barrels*`, `trigger*`,
   `selector*`, `bipod*`), `pt_*` empties at the layout's points,
   `magwell`, sling swivels; `art/check.py` clean.
3. **Mechanism** (`extraction_core::firearm`): the action as a state
   machine with unit tests, its faults and their cures.
4. **Layout** (`src/weapons/layout.rs`): grip, support, muzzle, port,
   sight, magwell, handle and load port, matching the model's empties.
5. **Hands** (`src/weapons/mod.rs`): how a hand works it, with a headset
   test in `weapons::feel::vr_tests` that loads, cycles, fires and clears
   each fault through `RigInput`.
6. **Flat and bots** (`src/weapons/flat.rs`): reload, rack and mode
   macros, and a drill in `src/bot/scenarios.rs`.
7. **Feel and sound**: recoil and case size (`feel.rs`), the sling's
   swivels (`sling.rs`), the shot's cue (`audio.rs`), how far robots hear
   it (`report` in the catalog).
8. **Bench and loot**: re-pack and repair recipes
   (`extraction_core::fabrication`), loot entries by rarity and valley
   (`assets/data/loot.ron`), the dev cache.
9. **The range** (`weapons::range`): every gun fires at every material.
