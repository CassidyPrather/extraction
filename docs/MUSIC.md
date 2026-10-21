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
