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

## How each one works

### MP-133 pump (`mp133`)

`ActionKind::Pump` in `extraction_core::firearm`. Shells go into the
four-shell tube through the loading port under the receiver, forend
forward (a loose shell let go there, or a box held there, one shell a
beat). The **supporting hand pumps it**: there's no separate handle, the
hand on the forend drives the action as it slides along the bore
(`work_handles`). Back, the stroke cocks the hammer, lets the next shell
out of the tube onto the carrier at 70% of the travel and throws out
what's in the chamber at 85%; forward, it rams the carrier's shell home.

- **The action lock**: pumped onto a live shell, the cocked hammer locks
  the forend shut until the shot or a dry click. A (the action release,
  by the trigger guard) lets it open: to unload, or to clear a live
  shell.
- **The short stroke**: back far enough to let a shell out but not to
  throw the hull, then forward: the shell runs into the case and the
  forend stops short (`Malfunction::DoubleFeed`, drawn in the port). The
  trigger does nothing. A full pump to the rear throws the hull and the
  next stroke feeds the shell. A worn extractor can slip off a fired hull
  on a full stroke and give the same jam (the gun's own dice, at the
  self-loaders' malfunction chance).
- Flat and bots: `R` thumbs the tube full and pumps one up; the rack key
  releases and pumps.

### SVD (`svd`)

A self-loader from a detachable ten-round box (`svd_mag`, family `svd`),
7.62x54R: the Mosin's round and its re-packed boxes. The dovetail mount
over the bore takes the PSO-1 (and the 3.5x scope, the side rail); the
model's rail is on the left of the receiver as on the real gun. Safe and
semi; the carrier holds open on the empty box. The faults are the
self-loaders' (stovepipe, failure to feed, double feed).

### TOZ-8M (`toz8`)

A turn-bolt with no magazine (`internal_capacity: 0`): the round goes
straight into the open chamber. Rimfire (`rimfire: true`): 3% more
misfires; the round stays in, and lifting the bolt and dropping it again
cocks the striker to strike again. `report: 0.4`: robots hear it at 40%
of what its calibre would carry (about 50 m), and the shot sounds like a
suppressed one. A .22 stops in tin and plate; it goes through glass. Its
ammunition can't be re-packed (rimfire cases can't be reloaded), so it
comes only as found: boxes of fifty, commonest in Marrow Creek.

### SPSh-44 signal pistol (`flare_pistol`)

A single-barrel break action (`26mm`). Its shots don't make bullets
(`flares::Launcher`): `weapons::flares` throws a **star** instead, which
climbs (about 85 m straight up), lights after 0.2 s, burns for 8 s and,
past the top of its arc, sinks at about 6 m/s; where it comes down it
lies burning.

- **White** (the standard load) lights the ground within 60 m of it: a
  bright point light, and `flares::Starlight`, which robots' sight reads
  as they read a lamp. You can see them; they can see you.
- **Red** and **green** (`Load::SignalRed`, `Load::SignalGreen`) light
  little but carry further.
- Every second it burns, robots within 160 m (220 m for a signal) are
  told where it is, as if they'd heard something there: they go and
  look. A flare is a lure.
- A star that strikes a robot burns it (20) and dazzles it
  (`robots::Stunned`, 3 s).
- The cartridges are told apart by touch in the dark: the white wad is
  smooth, the red has one bump, the green two.
- The bench (with the vice) packs two white stars from powder, brass and
  polymer (`pack_flare`). The signals come only as found.

### RPK-74 (`rpk74`)

The AK's mechanism with a long, heavy barrel; it takes AK magazines and
its own 45-round ones (`rpk_mag`, family `ak545`). Its **bipod**
(`armoury::Bipod`): take the legs with the other hand's trigger and push
them down (or up) to swing them out (or fold them); they swing about
the clamp's pivot (-1.279 rad, `BIPOD_DEPLOY`). Deployed, the feet are a
rest probe for `feel`'s bracing that counts the bare ground too: set the
feet on a wall top, a sill or the earth and the gun lies still, with
bracing's recoil and sway.

## Where they turn up

`assets/data/loot.ron`: all five are in `weapons_rare` (the pump about as
often as the double, the .22 and the flare gun a little less, the SVD
and the RPK as rare as the PPSh), their ammunition and magazines in
`ammo_common`. On top of that, **each valley has more of some**: a table
named `<name>@<valley>` stands in for `<name>` in that valley
(`LootTables::roll_in`, the valley's key from its map's name), and rolls
the common table plus a few things of its own:

| Valley | More of |
| - | - |
| Marrow Creek (hunting country) | the TOZ-8 and .22, the pump |
| Kettle Pass (long sightlines) | the SVD and its magazine |
| Cinder Works (guard posts, signal boxes) | the RPK and its magazine, the flare gun and white and red stars |
| Sallow Reservoir (the dam's keepers) | the flare gun and white and green stars, the pump |

## Checks

- Core: `firearm::tests` (the pump's feed, lock and short stroke; a
  single-shot; a rimfire misfire), `loot::tests::a_valley_has_its_own_tables`.
- Headset (`weapons::armoury::vr_tests`, `weapons::flares::vr_tests`):
  each gun loaded, cycled, fired and cleared by hand through `RigInput`.
- Bots (`--scenario`): `pump_drill`, `marksman_drill`, `smallbore_drill`,
  `lmg_drill`, `flare_drill`.
- The range (`weapons::range`) fires the four new bullet guns at every
  material.

## Shots

`docs/shots/armoury/`: each gun in the gallery (`--gallery <id>+open`
works the action and swings a bipod down; give it 60 frames so the
model has loaded):

```bash
EXTRACTION_BIN=target/fast/extraction scripts/shot.sh docs/shots/armoury/rpk74_open.png \
    40,19.2,262,0,0 60 --gallery rpk74+open,0,0.95 --data-dir /tmp/x --no-robots
```
