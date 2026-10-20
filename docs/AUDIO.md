# Audio assets

Physical-world sound is recorded, not synthesised: footsteps, guns, impacts,
props, ambience and machines all come from field and foley recordings in CC0
libraries. `scripts/fetch_audio.py` downloads them, masters them and writes
game-ready Ogg Vorbis files under `assets/audio/`. Synthesis stays reserved
for things that do not exist (anomalies, robot voices), per
[DESIGN.md](DESIGN.md#audio-direction). Every source has a row in
`CREDITS.md`.

## Running it

```bash
python3 scripts/fetch_audio.py                        # fetch (~80 MB, first run) + master everything
python3 scripts/fetch_audio.py --only guns footsteps/mud   # categories and/or sets
python3 scripts/fetch_audio.py --offline              # build from assets/.cache only
python3 scripts/fetch_audio.py --fetch-only           # download + verify hashes, no mastering
python3 scripts/fetch_audio.py --inspect guns/mag_insert   # PNG of how a set is split
python3 scripts/fetch_audio.py --check-licenses       # re-read each freesound page's license
python3 scripts/fetch_audio.py --credits              # print CREDITS.md rows
python3 scripts/fetch_audio.py --repin                # accept changed upstream hashes
```

Needs `python3` with `numpy` (and `Pillow` for `--inspect`), and `ffmpeg`
built with libvorbis. `scripts/cloud-setup.sh` installs all three
(`apt-get install ffmpeg python3-numpy python3-pil`).

Mastering everything takes one to two minutes. The first run also
downloads about 80 MB of sources into `assets/.cache/audio/`; some Freesound
previews come off a cold CDN at under 100 KB/s, so allow a quarter of an
hour.

The mastered `.ogg` files (about 12 MB) are meant to be committed, so
building the game needs none of this. Run the script only to add or change
sounds.

## What is there

`assets/audio/sounds.json` is the index the game should read: one entry per
set (`"footsteps/grass"`), with its type, channel count, whether it loops,
and the variant files with their lengths. Pick a random variant per play,
and avoid repeating the last one.

| Category | Sets | Notes |
| --- | --- | --- |
| `footsteps/` | grass, gravel, mud, concrete, wood, metal, water, leaves | 8 variants each, mono. Mostly hiking/mountain boots. |
| `guns/` shots | pistol_shot, rifle_shot, bolt_rifle_shot, shotgun_shot, distant_shot | Real firearms: 9 mm and .380 pistols, AK-pattern (Saiga 7.62x39 and an AK47 at a range), Sauer 404 and a 6.5x55 bolt rifle, Mossberg 500 and a 20-gauge. `distant_shot` is stereo with the echo tail. |
| `guns/` mechanisms | mag_insert, mag_release, slide_rack, charging_handle, bolt_open, bolt_close, stripper_clip, shotgun_open, shotgun_close, shell_insert, dry_fire, safety, casing | Glock 17 and Remington 870 foley, a Romanian AK, a Mosin-Nagant and a Remington 700 bolt, a break-action 12-gauge, brass dropped on tile and concrete. |
| `impacts/` | metal, wood, dirt, concrete, ricochet | Target-side recordings of real hits on steel; rock and stick hits standing in for wood, dirt and stone. |
| `physical/` | drop_metal, drop_wood, drop_plastic, drop_cardboard, zip, cloth, velcro, door_creak, door_open, door_close, glass_break | |
| `physical/` handling | handle_metal, handle_wood, handle_plastic, button, switch, whoosh, catch, bell, chime | Light knocks of metal, wood and plastic for taking things up (Kenney); an arcade push button; a flashlight's switch; a bamboo stick and a shirt swung for the far grab; a ball caught; a struck bell for the truck; a glass tapped for the step-done ding. |
| `physical/` body | eat, drink, bandage, body_hit | An apple bitten, crackers crunched; gulps of water; tape and cloth torn; a punch for a round into you. |
| `ambience/` | wind_conifers, wind_mountain, wind_works, rain_light, creek, rural, room_tone (stereo loops); thunder_distant (stereo one-shots); crow, raven, jackdaw, rockfall (mono one-shots) | Loops are 15–45 s, 2 variants each. Each valley picks its wind, birds and far-off sounds in its map file (`sounds:`). |
| `machines/` | servo, clank, groan (one-shots); drone_rotor, hum, radio_static (mono loops); radio_squelch (one-shots); siren (stereo, 60 s) | The siren is a recorded air-raid siren, for the Surge warning. |
| `machines/` truck | truck_idle (mono loop); truck_arrive, truck_depart (mono beds) | Two diesel trucks idling; a small truck coming up the road; an old truck pulling away (a 1992 cassette recording from the Volga) and a large one grinding into gear. |

## Sources

What the script can reach without accounts or keys (checked 2026-10):

| Source | Status | License |
| --- | --- | --- |
| **Freesound** (freesound.org) | **Used, the main source.** The search pages scrape fine and the HQ previews at `cdn.freesound.org/previews/<id/1000>/<id>_<user>-hq.ogg` download without a key: Ogg Vorbis, ~192 kbps, 44.1 kHz stereo. The script only takes sounds whose page links CC0, and `--check-licenses` re-verifies that. | CC0 only |
| **Kenney** (kenney.nl) Impact Sounds, RPG Audio | **Used** for short impacts, cloth and doors. Direct zip downloads. | CC0 |
| OpenGameArt | Reachable and scrapable; nothing taken yet (Freesound covered the list). Good fallback for CC0 packs. | per item |
| Wikimedia Commons | API open (send a User-Agent or it rate-limits with 429). Few useful SFX; most audio is CC-BY-SA. Not used. | mostly BY-SA |
| Internet Archive | Reachable, but uploader-asserted licenses are unreliable (a Disney gunshot is tagged public domain). Not used. | unreliable |
| NASA | API open, public domain, but the audio is podcasts and launches. Not used. | PD |
| Sonniss GDC bundles | **Excluded.** Cloudflare blocks the site from here, and the license allows use in products but not redistributing the raw files, which a public repo would do. | royalty-free, no redistribution |
| Pixabay | **Excluded.** Returns 403 here, and its content license forbids redistributing files standalone. | Pixabay license |
| BBC Sound Effects | **Excluded.** Non-commercial. | RemArc, non-commercial |

**A Freesound API key would buy one thing: originals.** With a key plus a
user OAuth2 login, `apiv2/sounds/<id>/download/` returns the uploaded
WAV/FLAC instead of the lossy preview. The previews are good enough for game
SFX after mastering (they are re-encoded to Vorbis anyway), but anything
heard up close and long — the ambience beds, the siren — would gain a little
from lossless sources. If you get a key, add a `download` step that uses it
for `fs_*` sources and re-pin the hashes; the rest of the pipeline is
unchanged.

CC-BY is acceptable if a sound is clearly attributable (record author and
URL in the manifest and CREDITS.md), but nothing in the current set needs
it. The build helper refused a Freesound pack that turned out to be CC-BY
3.0 (Pjkasinski3's M4 sounds); the substitutes are CC0. No non-commercial or
no-redistribution licenses, ever: the repository is public under AGPL.

## The mastering chain

All processing happens at 48 kHz in float, then encodes once:

1. **Decode and filter** (ffmpeg). Resample to 48 kHz. High-pass at the
   set's `highpass` frequency with a 24 dB/oct slope to remove handling
   noise, wind and traffic rumble: 30 Hz for gunshots, 60 Hz for
   footsteps, 80–200 Hz for small mechanical clicks and radio. Optional
   `lowpass` and a raw ffmpeg `af` chain per input.
2. **Channel fold.** Positional sounds become mono. Stereo sources are
   averaged; if the average loses more than about two-thirds of the energy
   (an out-of-phase pair), the louder channel is used instead.
3. **Split.** Multi-hit recordings are cut into separate events
   (`split.mode = "events"`):
   - A 10 ms RMS envelope (2.5 ms hop) is computed.
   - Onset threshold: `rel_db` below the loudest point (default -30 dB), but
     at least `floor_margin_db` (10 dB) above the noise floor, taken as the
     10th percentile of the envelope.
   - Bursts closer than `min_gap` merge into one event, so a heel–toe step
     or a rack-and-release stays together.
   - Each event extends back through its attack (up to 60 ms, plus `pre`
     pre-roll) and forward through its decay until the envelope is
     `tail_db` below the event's peak, the next event starts, or `max_len`
     is reached.
   - Events more than `drop_db` (10 dB) quieter than the median event are
     dropped as partial or background hits. The rest are ranked by how
     close they are to the median level, with penalties for clipping and
     truncation.
   - `pick: [i, ...]` takes specific events by index (read them off the
     `--inspect` plot), `take: n` caps how many come from one input, and
     `split.mode = "segments"` takes explicit `[start, end]` times instead.
   A set draws round-robin across its inputs until it has `count`
   variants, so every recording is represented.
4. **Trim.** Leading audio quieter than -30 dB below the clip's peak is
   cut, keeping 5 ms before the first loud sample. The tail is cut where
   the envelope falls 55 dB below the peak.
5. **Fades.** 2 ms raised-cosine fade-in (keeps the transient), and a
   fade-out of 15 % of the clip's length, between 30 ms and 250 ms but no
   more than a third of a very short click.
6. **Level.**
   - One-shots: peak-normalised to -1 dBFS (sample peak).
   - Loops and beds: integrated loudness (EBU R128, measured by ffmpeg's
     `ebur128`) to -16 LUFS for ambience beds, -18 to -20 LUFS for machine
     loops, -20 LUFS for thunder, then a lookahead peak limiter at -1 dBFS
     if anything pokes over.
   These are asset levels, not mix levels: the game sets the playback
   volume per sound. Normalising everything means a variant never jumps
   out of its set.
7. **Loops.** A loop takes `length + xfade` seconds and folds the overhang
   back over the head with an equal-power crossfade, so the end flows into
   the start. Tonal loops (`tonal: true`: mains hum, rotors) would partly
   cancel under that, so their loop length is nudged by up to 25 ms to the
   best phase match and they use an equal-gain crossfade.
8. **Encode.** Ogg Vorbis, `-q:a 4` (about 110–130 kbps stereo), 48 kHz,
   metadata stripped. Mono for positional sounds, stereo for ambience beds,
   distant gunshots, thunder and the siren.

Lossy previews in, lossy out: the double encode is inaudible at these
bitrates for this kind of material, and the sources are better recordings
than anything that could be synthesised. Expect some clipping baked into
close gunshot recordings — that is what a handheld recorder does next to a
rifle, and it reads as loud.

## Adding a sound

1. Find a recording. On Freesound, filter by license "Creative Commons 0".
   Prefer field or foley recordings of the real thing (read the
   description: many "gun" sounds are airsoft or layered designs), with a
   low noise floor and the hits separated by silence.
2. Add a source to `scripts/audio_manifest.json`:

   ```json
   "fs_123456": {
     "url": "https://cdn.freesound.org/previews/123/123456_7890-hq.ogg",
     "page": "https://freesound.org/s/123456/",
     "title": "Boots on gravel",
     "author": "someone",
     "license": "CC0-1.0"
   }
   ```

   The preview URL is `previews/<id // 1000>/<id>_<uploader user id>-hq.ogg`;
   the user id is in the sound page's `data-mp3` attribute. Leave `sha256`
   out — the first fetch pins it.
3. Add or extend a set:

   ```json
   {"category": "footsteps", "name": "gravel", "type": "oneshot", "count": 8,
    "highpass": 60, "split": {"max_len": 0.8, "min_gap": 0.15, "tail_db": 45},
    "inputs": [{"source": "fs_123456", "end": 12},
               {"source": "kenney_impact", "member": "Audio/footstep_concrete_000.ogg",
                "split": {"mode": "whole"}}]}
   ```

   Set fields: `type` (`oneshot`, `loop`, `bed` for a long non-looping
   sound), `channels`, `count`, `highpass`, `lowpass`, `normalize`
   (`{"peak": -1}` or `{"lufs": -16, "peak": -1}`), `fade_in`, `fade_out`,
   `quality`, `split` defaults, and for loops `xfade` and `tonal`. Input
   fields: `source`, `member` (path inside a zip source), `start`/`end` or
   `length` (seconds), `split` overrides, `pick`, `take`, `highpass`,
   `gain_db`.
4. Run `python3 scripts/fetch_audio.py --inspect <category>/<name>`. It
   writes `assets/.cache/audio/inspect/<category>_<name>.png`: the
   envelope of each input with the detected events boxed and numbered
   (red = dropped as weak). Adjust `split`, `start`/`end`, or `pick` until
   the boxes sit on the hits you want.
5. Build it (`--only <category>/<name>`) and listen. Then add a
   `CREDITS.md` row per source (`--credits` prints them).

A set's old files are deleted before it is rebuilt, so shrinking `count`
leaves no stale variants behind.

## What you hear and feel

Every interaction and event in the game, the sound it makes (cue names
from `src/audio_mix.rs`) and the pulse your hands feel
(`src/rig/haptics.rs`). Audited 2026-10 by reading every message the game
writes against what `audio.rs` and `haptics.rs` listen to; the "was" column
is what it did before.

| What | Sound | Haptic | Was |
| --- | --- | --- | --- |
| Hand in reach of something; pointing at something far | — | tick in that hand | same |
| Taking something up | `hand.take`: a knock by what it's made of (`items::impacts::material_of`): metal, wood, plastic, or cloth for soft things; louder and lower the heavier it is (`weight_tone`) | by weight: a tin a tick, a pistol a click, a rifle a knock (`grab_pulse`) | cloth rustle for everything; one tick for everything |
| Letting go of it | `hand.release`: the same, lighter | tick | silent, nothing |
| Into a pouch or holster | `hand.socket` (cloth) | click in the hand that put it there | rustle, nothing |
| Onto a rail or cradle | `hand.attach` | click | click, nothing |
| Into your pack | `hand.stow` (zip) | click | zip, nothing |
| Into a crate, the stash, a tray, the pod | `hand.place`: the thing set down | click | the backpack's zip |
| Pack opened or shut | `pack.open`, `pack.close` (zip) | — | silent |
| Pack full, refused | `hand.refused` | knock | same |
| Push button beside a card | `button.in` as it clicks home, `button.out` as it comes back | click in the finger that pushed, tick coming out | silent; a click in both hands |
| Card, knob or kitchen tool pressed with the grip | `button.in` | click in the nearest hand | silent; both hands |
| Bench lever | `press.lever` | click | silent |
| The bell cord (truck, training yard) | `truck.bell` | click | silent |
| Far grab: the flick | `far.whoosh`, flying with the thing | click | silent |
| Far grab: the catch | `far.catch`, a slap into the palm, then `hand.take` | the grab's pulse plus a bit | `hand.grab` |
| Doors | `door.open`, `door.close` | click (grip press) | same, both hands |
| Eating, drinking | `use.eat`, `use.drink` (by the food's tag) | — | silent |
| Field dressing | `use.bandage` | — | silent |
| A light, the injector, a cloak, arming a charge | `use.switch` | — | silent |
| The radio | `radio.squelch`, `radio.static` | — | same |
| Guns: shot, mechanisms, casings | `gun.*` | shot kicks the trigger hand hardest, the support hand less, by the gun's recoil (pistol 0.68, Mosin 0.96; less with a shouldered stock, `shot_kick`); the action clicks; a magazine seats with a knock | one kick for every gun |
| A malfunction (stovepipe, failure to feed, double feed) | `gun.malfunction`: the bolt's close, slowed to a dull, wrong clack | knock in the grip hand | the haptic only |
| A round into you | `hit.body`, a thump | both hands, harder for more | the ground's dirt spray |
| A fall, an anomaly, the Surge, bleeding | — | both hands, at most four times a second | nothing |
| The truck coming to the rendezvous | `truck.arrive`, up the road, as it's due | — | silent |
| The truck waiting | `truck.idle` at its engine | — | silent |
| The truck leaving, with you or without | `truck.depart` | — | silent |
| A robot losing a part | `robot.part`; glass for an eye (`robot.glass`); the frame groaning when it was the core (`robot.wreck`) | — | only the bullet's hit |
| A robot noticing you (its suspicion passing `robots::NOTICED`) | `robot.notice`: the whirr of its head snapping round, at most every 8 s per robot | — | silent |
| The convoy loading, setting off through the Gate | `convoy.loaded`, `convoy.delivered` | — | silent |
| A dirigible overhead | `dirigible.engine`: a slow throb (the rotor loop at 0.4x), quickening as it moves, gone when wrecked | — | silent |
| Drones, walkers, grapplers, pylons | rotor loop, `drone.lock`, `walker.step`, `walker.servo`, `harpoon.*`, `pylon.work` | — | same |
| Anomalies, the Surge, attention | hums, pops, cracks, the siren, the ringing | — | same |
| Weather | rain bed with the rain; distant thunder | — | same |
| Landing from a jump or a drop | `land`: both feet on whatever's underfoot, harder the faster you fell | — | silent |
| The grapnel thrown, biting, coiled | `grapnel.throw`, `grapnel.hook`, `grapnel.coil` | — | silent |
| A cover deployed, folded | `cover.deploy`, `cover.fold` | — | silent |
| The camera drone spun up, marking a robot, landing | `camera_drone.on` (switch), `camera_drone.spot` (a click in your ear), `camera_drone.land` | — | rotor loop only |
| A zipline rigged, packed | `zip.rig` (the anchor biting), `zip.pack` | — | silent |
| Kitchen work: chopping, grinding, kneading and stirring, a dish done | `kitchen.chop`, `kitchen.grind`, `kitchen.work` per stroke; `kitchen.done` (`raid::kitchen::Cooking`) | — | silent |
| The bench finishing a job | `bench.done`, a small high bell | — | silent |
| An onboarding step done | `step.done`, a light ding | a soft pulse in both hands | (new) |

**For onboarding.** Write `crate::rig::haptics::StepDone { at: None }`
(or call `crate::rig::haptics::step_done(&mut commands, None)`) when a
step is done. The message is registered in the game plugin, so writing
it headless is fine; it does nothing there. Give `at` a position to
place the ding in the world instead of in your head.

**Still open.** Searching robots and the walker's and grappler's
attack tells wait on the robot-minds branch. The audit measures the dirigible's engine
at normal speed, so its spectrum (and audible range) is placed higher
than it plays. The truck
doesn't move between stops (it is moved while you're away), so its
sounds are placed where it would be.

**Haptics.** One scale, so a pulse means the same thing wherever it comes
from: a tick (0.1-0.15) is "this is here", a click (0.25-0.35) "that
happened", a knock (0.4-0.6) "that took force", and only a shot or a hit
goes past 0.6. Index controllers barely register under about 0.1 or
10 ms, so nothing is below either. Pulses on one controller in the same
frame merge (strongest, longest), because the runtime plays only the
last one it is given: a shot was being cut short by its own casing.

## Listening in the headset

The numbers place each sound; only ears say whether it's the right one.
Everything added in the 2026-10 polish was chosen from recordings'
descriptions and envelope plots and levelled by measurement, never heard.
At the next headset session, with the normal dynamics and then the
narrow ones (comfort board), go through these and note what's wrong in
words ("too bright", "sounds like plastic", "late"):

1. **Taking things up.** A rifle, a pistol, a tin, a wooden crate slat,
   a dressing, a radio. Does each sound like what it is, and does the
   rifle sound heavier than the tin? Letting go: heard, but not fussy?
   Does the haptic match: a tin a tick, a rifle a knock?
2. **Push buttons** beside the cards. Does the click land when the
   finger feels it bottom out, and is the release click heard?
3. **Far grab.** Point, squeeze, flick: does the whoosh travel with the
   thing, and does the catch read as a catch rather than a slap?
4. **Crates and the stash.** Things put in sound set down, not zipped.
5. **The bell cord** (truck and yard). Bell or gong? Too loud in the
   truck's bed?
6. **Onboarding.** The step ding and the station chime: pleasant, clear
   over wind, distinct from each other and from the bell?
7. **Eating, drinking, the field dressing, a flashlight's switch.**
8. **The truck.** Call it: is the approach heard about when it appears,
   does the idle sit right (not droning for five minutes), does it pull
   away when you extract?
9. **Being hit.** The thump of a round into you, and the shake in both
   hands for a fall and a burner: felt, not painful.
10. **Robots.** A drone's lock-on and a walker's steps from close by:
    alarming but not painful (they were turned down up close). A robot
    noticing you: is the whirr readable as "it saw me"?
11. **A jam.** Does `gun.malfunction` read as "wrong", distinct from dry
    fire?
12. **Overall.** Anything that made you flinch, or that you didn't hear
    at all. The audit's "Loudest up close" list in `target/audio_audit/
    mix.txt` names the loudest suspects.

## Checking the mix

Ears are fickle, so the mix is also judged by numbers.
`python3 scripts/audio_audit.py` writes `target/audio_audit/` (`mix.txt`
to read, `assets.json` and `mix.json` for tools); `--check` exits 1 if
any cue is flagged, which makes it a gauge (`docs/QA.md`). The mix
itself is data: every sound the game plays is a cue in
`src/audio_mix.rs` with its playback gain, its reach (where its distance
falloff starts) and what it is for. `audio.rs` reads its gains from
there and the audit reads the same lines, so what is judged is what
plays.

**What's measured.** Every asset's loudness by ITU-R BS.1770, the
K-weighting behind LUFS. One-shots are measured as momentary-max
loudness (the loudest 400 ms; a short click counts the silence a meter
would see, which is how short sounds are heard). Loops and beds are
measured as gated integrated loudness. Also true peak (4x oversampled),
crest factor, DC offset, onset delay, clipped runs, loop seams and a
1/3-octave spectrum.

**How a cue is judged.** Its level as heard is the asset's loudness plus
its gain plus the falloff at its usual distance. That falloff follows
Firewheel's model: inverse distance from the cue's reach, and a one-pole
low-pass that closes with distance (`audio_mix::MUFFLE_FULL`). Then:

- **Realism, compressed.** Every cue carries a rough real-world level
  (`spl`: a rifle at the shooter's ear about 140 dB(A), a footstep at
  the walker's 55). A robust (Theil-Sen) line through the handling
  sounds and actions gives the game's compression of the world's
  dynamics, currently about 6:1. A detail or action more than 5 LU off
  that line is flagged: it is louder or quieter than its kind.
- **Roles.** Beds set the floor, and footsteps sit 2-10 LU over the wind.
  Signals are the sounds that carry information: a drone's rotor and
  lock-on, a walker's footfall, an anomaly, the radio, your own dry
  fire. Realism doesn't decide those; audible range does (below). No
  signal may be louder than your own gunshot.
- **Audible range.** For each placed signal, the farthest it is still
  heard over the calm bed (wind) and over a downpour (wind and full
  rain). The masking model is simple: the masker's 1/3-octave levels,
  spread to neighbouring bands at -25 dB per band below and -12 dB per
  band above, less 6 dB. A signal is heard where it beats that threshold
  in any band, after the falloff and the muffle. Targets
  (`RANGE_TARGETS`) say how far each warning must reach, about as far
  as the threat does: the drone's rotor 120 m calm and 40 m in a
  downpour, a walker's footfall 60 and 30.
- **Loop edges.** Loops the game only keeps near you (anomaly hums,
  started inside 40 m) must start below hearing, or they switch on as
  you walk up. They fade from full at 25 m to -40 dB at 40 m; the audit
  reports each one's margin over the wind where it starts, and flags
  any above 0.
- **Peaks.** A cue whose asset true peak plus its gain goes over 0 dBTP
  is flagged. Loud, rare actions and events may lean on the master
  limiter (bevy_seedling's default graph ends in one) up to +3.
- **Startle.** In a headset nothing should hit painfully. Each one-shot's
  loudest variant, at the nearest it plays (set pieces at their usual
  distance), may be no louder than your own rifle up close, with either
  dynamics (narrow dynamics, for a noisy room, 2 LU more). Loops and
  beds swell rather than strike, and aren't judged here. `mix.txt`
  lists the ten loudest.
- **Scenes.** Five scenes are mixed offline from the assets as the game
  plays them: a quiet walk, a downpour, a firefight in the rain,
  indoors, the Surge. Each is measured for integrated loudness, peaks,
  and how far each signal in it clears everything else.
- **A raid.** The scenes weighted by how much of a raid each takes
  (fighting measured from the bot raids, 7%; a fifth of raids rain
  hard) give a raid's integrated loudness and loudness range (EBU Tech
  3342). The reference is ASWG-R001, the game-audio loudness practice
  from Sony's Audio Standards Working Group: -24 LUFS +-2 over
  representative play on console and PC. The check fails outside it. At
  the time of writing a raid measures -24.9 LUFS with a 16 LU range: the
  quiet walk sits 15 LU under a firefight, which is the tension the game
  wants, but on a headset's open speakers in a noisy room the forest
  may be lost under the room. So the comfort cards offer narrow
  dynamics (`audio_mix::NARROW`): beds +6 dB, details +5, signals +5
  (with the beds, so warnings keep their reach), actions and events -3.
  The audit measures it too: -23.6 LUFS with a 9.3 LU range, about what
  night modes aim for.

**What the first run found** (and what changed):

- The drone's lock-on (a synthesised tone, crest factor 3.6 dB) played
  11 LU louder than your own rifle, from 20 m away, and carried 750 m
  through a downpour. The offering chime and the lift's pop were 14 LU
  over your rifle. All three are now under it; the lock-on still clears
  a firefight by 5 dB at 20 m.
- Variants within a set differed by up to 16 LU: one-shots had been
  peak-normalised, and a peak says little about loudness. The mastering
  now pulls each set's variants to within about 2 LU of the set median,
  keeping 30% of the natural spread (`match_variants` in
  `fetch_audio.py`). It also leaves each set's loudest peak at -1.5 dBFS,
  so the Vorbis encode stays under -1 dBTP; before, encodes reached
  +2.4 dBTP.
- Firewheel's default distance muffle closes to 20 Hz at 200 m, which
  silenced everything farther off, the Surge siren at the Gate included.
  The muffle now closes at 900 m, and the siren has a 200 m reach: it is
  heard across the valley.
- A robot's gunfire 30 m away sat 15 LU under its kind; it now has a
  20 m reach. Your own dry fire was masked entirely by a downpour; it is
  now heard to 9 m. A walker battering a door could not be heard in
  rain; it now carries 30 m. Ricochets and impacts came up 4-6 dB.
- The firefight scene peaked at +2.4 dBFS (204 samples over full scale,
  caught by the limiter); it now peaks at +0.4 dBFS (5 samples). Overall
  the game now runs from about -32 LUFS on a quiet walk to -17 LUFS in a
  firefight, with your rifle at -23 LUFS momentary.

**What the startle check found** (2026-10). Several sounds that usually
play far off were louder than your rifle when they happened near you:
the drone's lock-on chirp by 8 LU (16 with narrow dynamics), a pylon at
work, a walker's servos and a door giving way by 1-4 LU, the EMP's pop
by 6. Their gains came down and their reach went up by the same amount,
so each is as loud as before from where it usually plays and only the
near ones are tamed. Signals that sit at your hand (dry fire, the
radio's squelch, a glance) came down 2-3 dB, since narrow dynamics lifts
them 5. A walker battering a door is as loud up close as at its 20 m
(it's inside its reach), so it keeps its level and the narrow allowance.

**Changing the mix.** Edit a cue's gain or reach in `src/audio_mix.rs`
and re-run the audit (a few seconds; assets are re-measured only when
they change, or with `--fresh`). To move the targets, change the
constants at the top of `scripts/audio_mix.py`.
