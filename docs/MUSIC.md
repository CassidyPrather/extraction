# Music

All music in the game is played by something in the valleys and synthesised in `extraction_core::music`; no recordings, so the only licences are the tunes' own. Tunes are written in Music Macro Language in `music/tunes.rs`, played by the instruments in `music/synth.rs`, and coloured by tape, radio or gramophone in `music/media.rs`.

## The tunes

This table is the licence manifest. A tune goes in only if it is unambiguously public domain: traditional, or by a composer who died more than 100 years ago (or, for Satie, more than 70 years ago with the work first published before 1929). We wrote the harmonisations of the hymns and all the accompaniments for the game; they are not copied from any hymnal or edition.

| id | title | composer / origin, date | why it's public domain | instrument(s) |
|---|---|---|---|---|
| `minuet_g` | Minuet in G | Christian Petzold (1677–1733), BWV Anh. 114, c. 1725 | composer d. 1733 | piano |
| `minuet_g_learner` | Minuet in G (practice) | as above; first half played twice, slowly, with mistakes | composer d. 1733 | piano |
| `fur_elise` | Für Elise | Ludwig van Beethoven (1770–1827), Bagatelle WoO 59, 1810 | composer d. 1827 | piano |
| `bratre_kubo` | Bratře Kubo (Frère Jacques) | French traditional round, 18th c. | traditional | piano |
| `gymnopedie` | Gymnopédie No. 1 (opening) | Erik Satie (1866–1925), 1888 | published 1888 (US); composer d. 1925, so 70 years after death ended in 1995 (EU) | piano |
| `ragtime` | The Entertainer (intro and A strain) | Scott Joplin (1868–1917), 1902 | published 1902; composer d. 1917 | piano |
| `brahms_lullaby` | Wiegenlied | Johannes Brahms (1833–1897), Op. 49 No. 4, 1868 | composer d. 1897 | hum |
| `largo` | Largo theme ("Goin' Home") | Antonín Dvořák (1841–1904), Symphony No. 9, 1893 | composer d. 1904 (the theme only, not the 1922 song words) | hum |
| `old_hundredth` | Old Hundredth | Louis Bourgeois (c. 1510–c. 1560), Genevan Psalter, 1551 | 16th c. | harmonium (4 parts) |
| `silent_night` | Stille Nacht | Franz Xaver Gruber (1787–1863), 1818 | composer d. 1863 | harmonium (4 parts) |
| `hymn_to_joy` | Hymn to Joy | Ludwig van Beethoven, Symphony No. 9 finale theme, 1824 | composer d. 1827 | organ (4 parts) |
| `waltz_folk` | O du lieber Augustin | Viennese folk song, c. 1800 | traditional | accordion, bass |
| `polka` | Hänschen klein | German folk tune, 19th c. (words F. Wiedemann, 1860) | traditional | accordion, bass |
| `greensleeves` | Greensleeves | English traditional, late 16th c. | traditional | guitar, hum |
| `bells_rounds` | Rounds, Plain Hunt on six, call changes | English change ringing, 17th c. | a traditional ringing method, not a composition | bells |
| `westminster` | Westminster Quarters (the hour) | attrib. William Crotch (1775–1847), 1793 | composer d. 1847 | bells |
| `carillon` | Old Hundredth (carillon) | Louis Bourgeois, 1551 | 16th c. | bells |
| `music_box` | Ah ! vous dirai-je, maman | French folk melody, first printed 1761 | traditional | music box |
| `dance_band` | Oh! Susanna | Stephen Foster (1826–1864), 1848 | composer d. 1864 | accordion, piano, bass |

We deliberately left out:

- "Beer Barrel Polka" / "Škoda lásky" (Vejvoda, 1927; Vejvoda d. 1988): still in copyright.
- Any modern arrangement, edition or recording.

## The things that play it

Everything below is `src/radio` (headless logic in `RadioPlugin`, sound in
`radio::sound`, motion in `radio::visuals`); the rules are engine-free in
`extraction_core::music`.

**The cassette player** (`cassette_player`, a portable with four piano keys
on top). Hold it in one hand and push the keys with the other forefinger
(trigger off, hand empty); flat or bot, the use button is play/stop.

- **Stop**, then **stop** again: the door springs open (the eject).
- A tape goes in **open edge down**; the face toward you through the
  window is the side that plays. Upside down it won't go: it drops out in
  front (`DeckWhat::WrongWay`). The tape keeps how far it's wound, so
  side B is side A backwards.
- **Push the open door** shut (its inside face is a push button).
- **Play** runs to the end of the side and stops; **rewind** and **fast
  forward** wind twelve times as fast. The reels turn in the window.
- A **power cell** in the back runs it: 3 W playing, 5 W winding (a full
  cell is a little over three hours of music). Under a fifth of a charge
  the motor sags: the music goes flat and wobbles; then it stops.
- The players laid in Hana's house and the gate booth come with a cell
  half run down. Elsewhere they're among the gadgets; tapes among curios.

**The tapes** (`music::tapes::TAPES`, one item each, owners and labels
held to `docs/LORE.md`): Hana's "for Anna - practice" (Anna at the upright,
stumbling and starting again; Hana humming), the sexton's HARVEST HYMNS
(harmonium), Fiala's "Lodge dance (off the wireless)", a soldier's DUTY MIX
2 (it runs out mid-song), Vlasta's "Our piano. Ruben's birthday." (the
piano that went into the furnace), Pech's "Old Sallow bells - from Dad's
wire". Each lies by its owner's note on a table or shelf, never the floor
(`radio::places`, tested).

**The radio's knob** (the field radio's `dial` node). Hold the radio, take
the knob with the other hand and turn your wrist: about 40 kHz a radian,
clockwise up the band. Let go and take it again to keep turning. All the
way down it clicks off (it starts off). What's on the dial is
`music::stations`:

| kHz | Station | Heard |
| - | - | - |
| 702 | The Authority's evacuation loop (attention tone, a machine voice, Story's eight lines) | everywhere, best in Marrow; it outlasts the recall: nobody turned it off |
| 909 | A station over the hills: dance music, faint and fading | high ground; on the Kettle saddle and over the reservoir |
| 1080 | Radio Marrow, off the air: static with the old station surfacing (a bar of a tune, the pips, a chime) | Marrow, under its mast |
| 1215 | A woman reading figures | from the second rank on, from high ground |
| 1413 | The Custodian's numbers, in Morse: new figures every raid day | everywhere; silent after the recall |
| 1560 | The machines' traffic | where the chatter is |

Between stations, static; underground, only static. A little off a
station the set whistles, lower as you close on it and gone when you're
on it: you can find a station by ear. The robots' chatter
breaks in whatever it's tuned to, as before. A radio in a pouch or the
pack plays nothing aloud.

**The upright piano** (`radio::piano`, `art/props/music.py`): each pointing
forefinger is a finger on the keys; a key sounds as it bottoms out, as
loud as it went down fast, and is damped as you lift. It stands in Hana's
front room, against the partition between the doorways: Anna's piano.

**The harmonium** is the same keyboard (the art script builds both from
one key layout, and `piano::Keyboard` holds the code to it), five octaves
of reeds that sound while held. One stands by the altar in the Kettle
chapel (the sexton's), one in the silt of Old Sallow's drowned church,
open to the sky, facing the doorway.

The standing instruments' spots (`radio::places::FIXTURES`) were chosen by
looking: a floor, a wall at the back, room in front. They're props, not
items, so the save never keeps them.

**The gramophone** (`radio::gramophone`): a 78 on the platter, push the
crank round (each push winds about twenty seconds), push the arm over.
As the spring runs down the record slows and sags flat; wind it while it
plays to keep it going. One stands on the floor of Fiala's lodge against
the log wall, with a waltz by the lodge book; three 78s turn up among
curios.

**Robots hear all of it.** While it plays, a tape writes a 26 m noise
every 3 s (past the sleepers' wake radius), a gramophone 24 m, a piano
key struck hard 30 m, a radio out loud 7-14 m. Set a tape playing on a
table and walk away: it's a lure. It also tells them where you are if
it's in your hand. It works once: a robot that searched a spot and gave
up ignores sourceless sounds near it for two minutes (Robot minds'
habituation, `robots::senses::BORED_FOR`), so a tape left running draws
a walker, not a queue of them.

**The tapes and players are laid with the loot** (`items::loot::
roll_zone_loot` calls `radio::places::lay_tapes`): with every fresh roll,
never on a restore. Laid at every load, they doubled with each save
(`persist::tests::every_valley_loads_back_to_the_same_items` holds every
valley to it now).

**The score steps aside**: `radio::MusicNear` is true while any of these
plays within 15 m of you, and the score is silent under it: one musical
layer at a time.

## Checking it

- `cargo test -p extraction_core music`: the tracker, every instrument in
  tune, decay, the media, the deck's rules, every tape's takes, stations,
  Morse, and every tune parsing, aligned and in its instruments' range.
- `cargo test --lib radio::`: the cassette player through the headset's
  path (`radio::tests`: a cell, stop twice, a tape the right way and the
  wrong way, the door pushed shut, play, robots hearing it), the piano's
  keys against the model's nodes, every placement on a surface.
- The audio audit: `radio::sound::tests::renders_for_the_audit` renders
  each synthesised set to `target/audio_audit/music/`, and
  `scripts/audio_music.py` measures them into the mix pass and checks that
  a tape in your hand stays within 12 LU of a walker's step at 10 m and a
  station sits at least 3 LU under the chatter.

## Listening in the headset

1. Load Hana's tape the wrong way, then the right way. Is it clear why it
   wouldn't go? Does the door shutting feel like a door?
2. Side A: does Anna's stumble sound like a child, not a glitch? Side B:
   is the humming a person?
3. Let a cell run low (`--data-dir` with a drained cell): is the sag
   audible and sad rather than broken?
4. Turn the radio's knob across the band: can you find each station with
   a glove? Is the static between them unpleasant enough to keep turning?
5. The evacuation loop: is the voice readable through the AM band?
6. The piano: two fingers, a tune. Does a hard key sound harder?
7. The gramophone running down: does it slow like a spring?
8. Set a tape playing on a table in a house and hide: do walkers come?
