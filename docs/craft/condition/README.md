# Condition you can see

Does a gun's state read on the gun, at 30 cm and at 1 m in the game's
light, so that cleaning it at the bench is visibly worth doing? This
covers the first eight guns a player meets, on 4 November 2026.

## The states

Each gun is shown with `--gallery <gun>+<state>`, in five states:

| State | Flag | What the gun holds |
| - | - | - |
| clean | (none) | condition 1, no fouling, no grime |
| fouled | `+foul` | fouling 1 |
| grimy | `+grime` | grime 1 |
| worn | `+worn` | condition 0.45 |
| near failure | `+failing` | condition 0.08 |

The shots (`scripts/shot.sh`), each state in three places:

- the yard at 30 cm: `--map training_yard --gallery G+S,0,0.3`, camera
  `0,0,40,0,0`;
- the Rime Fell mill at 30 cm: `--map rime_fell`, camera
  `140,91.85,38,0,0`;
- the yard at 1 m: `--gallery G+S,0,1.0`.

In each sheet, the rows are those three places; the columns are clean,
fouled, grimy, worn and near failure.

## What was wrong, worst first

1. **A held gun kept the look it had when taken.**
   - **Why:** taking a gun in hand swaps its materials for marked copies,
     made once from what it then wore, so the room's light lifts it
     (`render::held_light`). The weathering then tinted its own
     materials, which no longer showed.
   - **What it meant:** cleaned, wiped or worn in the hand, a gun showed
     no change until it was put down. At 30 cm (the gallery stands in
     for a held gun), the PM read the same in every state.
   - **Fixed:** every gun now gets its own materials from the start, so
     the hand marks those, and each change also tints the copy shown.

   ![Before: the PM at 30 cm, the same in every state](pm_before_held_fix.jpg)

2. **Fouling showed only in the bore.** Burnt powder now lays a grey film
   on the metal and takes its sheen. A darker tint alone didn't read on
   near-black bluing, which shows mostly reflected sky. The film is
   metal-only, so a fouled gun's grip stays as it was. Grime, the
   weather's brown, covers the grip too.

   ![Clean, fouled, grimy, worn: the PM at 30 cm in the yard](pm_fouled_film.jpg)

3. **Wear rusted everything, wood and Bakelite included, and only
   faintly.** Wear now follows the finish, by the material's name
   (`weather::ages`):
   - **Steel** rusts: Steel, Blued, the AK's lacquered Stamped, Black,
     Iron.
   - **Aluminium** wears through its anodising to bare grey: the
     MP-133's receiver.
   - **Wood, Bakelite, polymer and rubber** fade and scuff paler.

   The lacquered and anodised finishes are 0 metallic, as wood is, so
   only the name tells them apart.

4. **The rust didn't line up with the handling.** Weapon feel's one hard
   line is condition 0.6 (`DRY_FROM`): there the action starts to drag
   in the hand, and jams are about 1 in 40. Rust now starts at 0.8, is
   plain by 0.6, and goes through by 0.35, the second line (+6% jams, 3x
   the spread). The eye warns before the hand does (`rust_shown`, tested).

## Per gun

**Reads** means a player tells the state from clean without a side by
side. For fouled, a side by side helps at 1 m. Up close the grey film
reads, and in the hand, where it matters, the gun is at 30 cm.

| Gun | Fouled | Grimy | Worn | Near failure | Sheet |
| - | - | - | - | - | - |
| PM | slide and frame grey, matte; Bakelite grip unchanged: reads at 30 cm | all dull grey-brown, grip too: reads | metal rust-brown, grip faded: reads, also at 1 m | rust through: reads (close to worn) | ![](sheet_pm_pistol.jpg) |
| TOZ-8M | receiver and barrel grey: reads at 30 cm | metal dull, stock darker: reads | receiver and barrel rust: reads | reads | ![](sheet_toz8.jpg) |
| IZh-43 | barrels grey; the bright action matte: reads | action and stock dull: reads | the action goes rust-brown: reads strongly | reads | ![](sheet_toz_double.jpg) |
| MP-133 | the alloy receiver takes the grey film: reads | dull: reads | receiver worn to bare pale grey, steel bolt rusts in the port: reads | reads | ![](sheet_mp133.jpg) |
| SPSh-44 | grey: reads at 30 cm | grip greys: reads | rust-brown, grip pale: reads | reads | ![](sheet_flare_pistol.jpg) |
| Mosin | receiver and barrel grey: reads at 30 cm | dull: reads | rust along the receiver and bands: reads | reads | ![](sheet_mosin.jpg) |
| SKS | grey: reads at 30 cm | dull: reads | rust: reads | reads | ![](sheet_sks.jpg) |
| AK-74 | the lacquered receiver greys; plum polymer unchanged: reads | dull: reads | receiver rust-brown; polymer faded: reads | reads | ![](sheet_ak74.jpg) |

## Still open

1. **Near failure looks only a little worse than worn.** By 0.45 the
   rust is already 85% of the way. A further cue for the last stretch,
   pitting or bare metal at the edges, needs patterned wear: a mask or
   texture, not a whole-material tint. It's worth doing if near failure
   matters to play. Weapon feel's softer 0.35 line is where it would
   start.
2. **Fouling builds evenly over the metal.** Real fouling gathers at the
   port and muzzle first. That also needs patterned wear.

## When a newcomer meets it

From Progression:
- **The kit PM** starts at condition 0.75: faint early rust, no fouling.
- **Fouling** comes from firing, so the newcomer's first fouled gun is
  their own PM, after the yard's range and the first fights.
- **Found guns** used to come up clean however worn they were. Now
  (Progression's 0fd9f25c) a found gun below condition 0.6 comes up
  grimy and fouled, scaled so no find jams more than 10% before
  cleaning. Found PMs, the kit, gifts, the armoury and sealed crates
  stay clean.

So the first find past raid 3 can look filthy, and the kit takes the
grime and the grey film straight off. Rust stays until spare parts.
