# Rounds landing on every machine

`sheet.jpg`: each kind alone, close, every part shot down to failing
(`robots::wear`'s look), and a round landing in each part as the frame is
taken (`scripts/hit_gauge.sh`, `EXTRACTION_HIT_SHOW`). One frame per kind
alongside.

The gauge behind it is `robots::hit_gauge`
(`cargo test --lib hit_gauge -- --ignored --nocapture`). It shoots every
part of every machine in a line-up, two rounds each, as `ballistics` would
write a hit. Then it counts:

- **within 100 ms (9 frames):** the effect at the point and the sounds
  placed there (their `audio_mix` cues);
- **once the part is failing:** whether its drawn pieces look it (changed
  or added, marks aside);
- **for each machine:** how many of its meshes the player's camera would
  draw.

Last run: 114/114 parts answer a hit. Every effect and sound shows on
frame 1 or 2 (≤ 22 ms), and all 14 kinds draw.

What a hit does, by part:

- **Seen.** `weapons::impacts` sprays sparks, flakes, glass or gas by part
  (`robot_recipe`). `fx::strike_flashes` adds a scuff for a shrug, an
  orange spit for a wound, and a white flash for a hit that tells.
  `fx::shell_marks` leaves a pock and a hole, except on a gas cell.
- **Heard.** `audio_machines::struck_layers` (Audio's):
  - armour rings (`hit.ring`), a lens breaks like glass, a gas cell
    thumps like cloth, and a joint clanks;
  - a weak spot that tells gets a crunch, a crack and a deep thud.
  - Every layer clears the gun's own report by 6 dB or more at 20 m.
- **Worn.** `robots::wear`:
  - hurt, a part's paint dulls and darkens;
  - failing, it goes sooty, a plate is knocked about 5° askew, and a
    lens cracks where it was last hit.
  - Pieces drawn away from their part (`wear::DrawnFor`: leg segments, a
    heavy's limbs) and a machine's loose pieces (a heavy's hull and
    plates, a sentry's housing) wear with the part nearest them.
