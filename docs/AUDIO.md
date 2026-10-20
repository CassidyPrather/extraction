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
| A grenade's pin pulled | `grenade.pin` (a click) and `grenade.scrape` (the ring coming off) | click in the hand on the ring | silent |
| Its spoon flying (thrown, or thumbed off to cook it) | `grenade.spoon`, a small metal ping | tick in the hand holding it down | silent |
| Its bang | `grenade.bang` (RGD-5, sharper) or `grenade.bang_heavy` (F-1), by catalog id; level with your rifle at 15 m and no louder closer | a hit, if it hurts you | silent |
| Its fragments (and any slow round) landing | the impact by surface, softer the slower: full from 300 m/s, −12 dB at 75 m/s | — | every round at full |
| A step into a puddle (rain-wet ground, `world::puddles::puddle_at` > 0.5) | `step` from `footsteps/water` | — | the ground's set |
| The ending (`raid::epilogue`) | the score falls to silence over ~6 s while it's active; the crates opening: `epilogue.creak`, a long low lid at hers, then five fainter round the stacks a beat apart; him at her crate: nothing added, the wind and room held down 9 dB for 10 s (`Hush`) | — | the score played on |
| A round meeting your armour (`weapons::armour::ArmourStruck`) | stopped: `armour.clang` and `armour.thump` through it; turned: `armour.ping`; through: the clang and then the wet `armour.thump`. The clang is levelled just under your rifle up close | a hit, if it hurts you | silent |
| Your kit as you move (`foley`): worn plates knocking, sling swivels, the pack's load shifting, water sloshing | `gear.plate`, `gear.swivel`, `gear.pack`, `gear.slosh` (low-passed: it's in a bottle), on a step, each louder the farther it carries (`gear_db`) and more often | — | silent |
| Robot chatter overheard on your radio | `chatter.squelch`, `chatter.static`, `chatter.voice`: their talk | — | servo chirps |
| Guns: shot, mechanisms, casings | `gun.*` | shot kicks the trigger hand hardest, the support hand less, by the gun's recoil (pistol 0.68, Mosin 0.96; less with a shouldered stock, `shot_kick`); the action clicks; a magazine seats with a knock | one kick for every gun |
| A round landing (`weapons::impacts::Impact`, classed once for the spray and the sound) | `hit.impact` by surface: concrete, wood, steel, earth; `hit.glass` (window glass, a robot's eye); `hit.water` (a slap, harder for faster rounds); `hit.body`; out through a thin wall, a second crack 6 dB quieter | — | re-classed from the Hit; water silent |
| A spent case landing (`weapons::CasingLanded`), a beat after the shot | `gun.casing` by floor: brass rings on steel and concrete, knocks on boards, ticks in grass, plips in water; a shotgun hull thunks; bounces 8 dB quieter | — | a ping at the port |
| A suppressed shot (`gadgets::Suppressed`) | `gun.shot_suppressed`: the shot through a 2.2 kHz low-pass, 5 LU under a bare one (about 118 dB at the ear, against 140); far off, the distant shot 14 dB down | as a shot | a bare shot |
| A malfunction (stovepipe, failure to feed, double feed) | `gun.malfunction`: the bolt's close, slowed to a dull, wrong clack | knock in the grip hand | the haptic only |
| A round into you | `hit.body`, a thump | both hands, harder for more | the ground's dirt spray |
| A fall, an anomaly, the Surge, bleeding | — | both hands, at most four times a second | nothing |
| The truck coming to the rendezvous | `truck.arrive`, up the road, as it's due | — | silent |
| The truck waiting | `truck.idle` at its engine | — | silent |
| The truck leaving, with you or without | `truck.depart` | — | silent |
| A robot losing a part | `robot.part`; glass for an eye (`robot.glass`); the frame groaning when it was the core (`robot.wreck`) | — | only the bullet's hit |
| A robot noticing you (its suspicion passing `robots::NOTICED`) | `robot.notice`: the whirr of its head snapping round, at most every 8 s per robot | — | silent |
| A robot going to look, searching, giving up, calling the others (`robots::search`) | `robot.investigate` (a whirr), `robot.search` (servo sweeps), `robot.lost` (a falling whine), `robot.callout` (a squelch) and `robot.voice` (its call); search sounds at most every 6 s per robot | — | silent |
| A walker's turret settling before its first shot; a grappler winding up | `walker.aim` + `walker.aim_lock` at the muzzle, `harpoon.windup`: the 0.6-0.7 s warning, heard 43-71 m through a downpour | — | silent |
| The convoy loading, setting off through the Gate | `convoy.loaded`, `convoy.delivered` | — | silent |
| A dirigible overhead | `dirigible.engine`: a slow throb (the rotor loop at 0.4x), quickening as it moves, gone when wrecked | — | silent |
| Drones, walkers, grapplers, pylons | rotor loop, `drone.lock`, `walker.step`, `walker.servo`, `harpoon.*`, `pylon.work` | — | same |
| A gravity well; a mirror fold (synthesised) | `anomaly.well`: sub tones beating, air drawn in in long breaths; `anomaly.mirror`: glassy partials drifting; `anomaly.blink` at each thing blinked (`mirrors::Blinked`): air rushing out where it was (`anomaly.blink_air`), a glassy pop where it lands; `anomaly.arc`: a snare's arc cracking where it strikes (`ArcStrike`); `anomaly.reveal`: a falling ping as a thrown thing touches one; `anomaly.crush`: metal giving, slowed, under a whump | — | placeholders |
| The anomaly detector (`anomaly::detector::DetectorClick`) | `detector.click`: a dry Geiger tick with a little case ring, at the hand when held, 8 dB down at the hip, brighter near an edge | — | silent |
| A round glancing off (`ballistics::Ricochet`) | `hit.ricochet`, a little along the way it left; only real glances whine now | — | a random 12% of hits |
| Underground power (`world::underground::SitePower`) | `power.breaker`: the lever clanking home; `power.pull` then `power.generator` running by the switch, per stage. The generator is synthesised until a recording can be had: firing at 27 Hz with its harmonics, the rate wandering a little, a rattle on each firing, the exhaust | — | silent |
| A door that won't open: power-locked, wedged | `door.locked` (a bolt's clank), `door.wedged` (a thud) | click | silent |
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

**Robot voices** (`machines/robot_voice`, synthesised in
`scripts/synth_anomaly.py`, as `docs/DESIGN.md` allows). They're
machine syllables: a buzz at 85-130 Hz through shifting vowel formants,
clipped short, in bursts of three to seven. Then comes the radio's
chain, baked into the asset: band-limited to 300-3400 Hz, crushed to
6 bits held at 8 kHz, softened above 5 kHz by the speaker, and a
squelch tail. They play as placed sounds, so a robot calling the
others from behind a wall is occluded and reverberated like everything
else. The audit measures the dirigible's engine
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

## Acoustics

Spaces sound like their size (`src/acoustics.rs`).

**The space.** A few times a second, seventeen rays go out from your
head: twelve round the horizon, four raised 40 degrees, one straight
up. What they meet, and where, gives:
- how enclosed the place is: the share of rays that meet something
  within 40 m;
- its size: the mean distance to what they meet; misses are openings;
- its longest way round: a corridor's length, a hall's far end;
- what its surfaces are made of, by majority: metal, bare concrete or
  rock, plaster and brick, wood, soft (canvas, bark, earth);
- the roof overhead.

That makes it open, sheltered (an alley, a yard), a room, a hall, or
underground (the dark sites).

**The reverb.** One reverb bus (bevy_seedling's Freeverb, through its
`effects` feature). Every placed sound sends to it at full level,
except recordings with their own space baked in: the far-off shot's
echo, the siren and the Surge's cracks send 30%. The space sets the
bus:
- **Decay** follows Sabine's formula. For a box the mean free path is
  4V/S, so RT60 is about 0.04 × path / absorption. Absorption counts a
  room's contents, not only its walls: metal 0.05, bare concrete 0.07,
  plaster and brick 0.15, wood 0.2, soft 0.45. Open sides shorten it,
  and tunnels count their longest way round. Above ground, long rays
  have usually left through a window, so they don't count.
- **Brightness:** metal rings, plaster and wood dull it.
- **Wet level:** off in the open and in a forest (trunks and earth
  don't echo), -24 to -18 dB sheltered, -16 to -12 in a room, -13 to
  -9 in a hall, -11 to -8 underground.

Changes ease over half a second, so walking through a door is a
crossfade.

| Place (2026-10, three maps) | Places | RT60 | Wet | A rifle shot's direct over reverb |
| --- | --- | --- | --- | --- |
| Field, forest | 10 | (dry) | off | dry |
| Timber shed or cabin | 18 | 0.5 s (Freeverb's shortest) | -12 dB | +7 dB |
| House, shop (plaster, brick) | 27 | 0.8 s | -12 dB | +6 dB |
| Concrete room | 6 | 1.2 s | -12 dB | +5 dB |
| Barn (wood hall) | 2 | 1.9 s | -9 dB | 0 dB |
| Tin shed, depot (metal) | 9 | 2.3-3.5 s | -12 to -10 dB | +2 to -2 dB |
| Factory hall (brick, Cinder Works) | 2 | 3.1 s | -11 dB | 0 dB |
| Underground | 31 | 1.8 s | -8 dB | -1 dB |

**Occlusion.** Each placed sound is checked as it starts, and loops
every quarter second (at most 24 checks a frame), by the rule robots
hear by (`robots::senses`):
- **Through a wall or over a hill:** a sound carries `OCCLUDED`
  (0.45) of its reach. Under inverse-distance falloff that is 6.9 dB
  quieter, and it's muffled to 700 Hz.
- **Round the edge of an open doorway** (both legs clear): 3 dB
  quieter, muffled to 3 kHz.
- **The ray test:** a path counts as blocked only if both the line from
  just above the source and the line `HEARD_OVER` (1.2 m) above it are
  cut. Robots test only the raised line, which through a doorway runs
  into the lintel; they've been told.

So a walker's footfall behind a wall is still heard, just duller, about
where a robot would hear yours.

**Underground**, the wind dies away over a few seconds as you go
down the stairs, and the rain with it.

**Rain under a roof.** Rain follows where you stand:
- **In the open:** all of it.
- **Under a roof:** quieter and duller. Tin and wood are 12-14 dB down
  and below about 1 kHz; canvas loses less.
- **Under a porch:** a little duller.
- **Underground:** none.

A roof's own drumming plays over that, blended by the rain's strength
from a light loop to a heavy one (more and bigger drops). The loops are
`ambience/rain_tin`, `rain_wood` and `rain_canvas`, plus `_light`
versions.

They're synthesised (`scripts/make_rain_roofs.py`), an exception to
the recorded-world rule. No CC0 recordings were reachable: Freesound
refused this container, and OpenGameArt and Commons had none. Rain on
a roof is a dense Poisson stream of drop impacts, each a short burst
exciting the roof's modes, over 16 patches of roof so no one resonance
sings:
- **Tin:** bright modes at 1-4 kHz, ringing 80-150 ms, and a low boom
  from the panel.
- **Wood:** 300-900 Hz, about 20 ms.
- **Canvas:** soft, low-passed thumps.

Each loop is a circular convolution of the drop train with the roof's
response, so it repeats seamlessly. It's levelled against the outdoor
rain bed: tin +3 LU, canvas 0, wood -4, light loops 6 LU under. The
audit checks those levels to within 1.5 LU, and that tin rings
brightest ([spectra](shots/audio/rain_roofs.png)). Replace them with
recordings when they can be had.

**Measuring it.** `cargo test acoustics::tests::every_place_is_probed_for_the_audit`
probes every building and underground room on the three maps and writes
`target/audio_audit/acoustics.json`. `audio_audit.py` then renders each
kind of place's reverb with a port of the game's Freeverb
(`scripts/audio_acoustics.py`) and checks it:
- the decay against what the probe asked for, within 15%;
- the direct-to-reverberant ratio of a shot and a footstep;
- that a room's tail adds at most 3 LU to a shot's loudness, for
  comfort;
- that outdoors is dry.

The first run found Freeverb's damping shortens every tail to 0.86 of
its combs' decay, so `freeverb_room` asks for that much more
(`FREEVERB_DAMPED`). Decays and tail spectra:
[decay](shots/audio/acoustics_decay.png),
[spectra](shots/audio/acoustics_spectra.png).

## Your kit, heard

Every step, whatever you carry rattles a little (`src/foley.rs`, which
runs headless), and robots hear it:

| Part | Robots hear it to, at a walk |
| --- | --- |
| A plate worn in the vest (`weapons::armour::Worn`) knocking | 5 m per plate (two: √2 × as far) |
| A slung gun's swivels | 3 m per gun |
| The pack's load shifting | 0.6 m per kg over 2 kg, to 8 m |
| Water sloshing | 2.5 m per bottle |

A run carries farther (in proportion to pace) and a crouch 0.6 as far,
as a footstep does. The parts add by power into one `robots::Noise`
from you, so a loaded runner is heard coming where a light one creeps
by. Even the heaviest kit at a run carries less than one and a half
times a run on gravel; the footstep stays the loudest thing you do.

Each part plays its cue at a level by its reach (`gear_db`: twice as
far, 6 dB up). **The audit check:** at its cue's gain a part is one
robots hear to 5 m (`GEAR_REACH_REF`), and by the audit's masking
model your ears should hear it over the wind to within 0.6–1.6× that.
Now: plate 4 m, swivels 5 m, pack 4 m, slosh 5 m. So what robots hear
you can hear too, and nothing you carry gives you away silently.

## Binaural in the headset

In the headset, placed sounds go through a head-related filter
(firewheel's `HrtfNode`): each ear hears the sound as a dummy head's ear
would from that direction. That gives the delay between the ears, the
level difference, and the pinna's colouring, which is what tells ahead
from behind and above from level. Panning gives only the level
difference, so behind sounds like ahead.

- **The responses** are SADIE II's KEMAR (University of York, Apache-2.0;
  `assets/audio/hrir/NOTICE`). They were measured at 48 kHz, and
  `scripts/audio_hrir.py` writes them on a 266-direction sphere in the
  listener's frame. The node also embeds IRCAM subjects, which localise
  about as well (the audit prints one beside ours), but the IRCAM Listen
  data states no licence.
- **What goes binaural** (`audio::place`): a placed sound within
  `BINAURAL_REACH` (40 m), while fewer than `HRTF_VOICES` (24) are
  playing; each costs a convolution. Farther sounds, and sounds past the
  cap, are panned: at that distance the muffle and the reverb say more
  than the ears' filter does. Both chains share the cue's falloff, distance
  muffle, occlusion (`occlude`) and reverb send, so which chain a sound
  takes changes its direction cues, not how it fades.
- **When:** in the headset only, and only when the comfort setting
  `binaural` is on (on by default; `--no-binaural` turns it off). Flat
  mode always pans, because on speakers a head filter just colours.
- **FFT size.** The node convolves 128-sample slices with 512-point FFTs.
  The sphere is zero-padded to 385 taps so that slice + taps − 1 = 512
  points. A length that factors badly makes rustfft use an algorithm
  that needs more scratch than the node allocates, and the audio thread
  panics. A test (`binaural_tells_front_from_back`) guards this.

**The audit check** (`scripts/audio_binaural.py`): the test above renders
a click from eight directions through the `hrtf` crate (the node's own
convolution) and writes `target/audio_audit/binaural.json`. The audit
measures the cues on that output:

| Cue | Measure | Wants | KEMAR | IRCAM 1040 |
| --- | --- | --- | --- | --- |
| Side level | near ear over far, 0.5–10 kHz | ≥ 6 dB | 15 dB | 12 dB |
| Side arrival | cross-correlation lag | ≥ 400 µs | 729 µs | 562 µs |
| Front/back | 4–10 kHz over 0.5–3 kHz, ahead minus behind | ≥ 2.5 dB | 4.0 dB | 6.3 dB |
| Up | the same, above minus ahead | ≥ 2.0 dB | 3.7 dB | 2.7 dB |

## The score

A generative ambient layer (`src/audio_score.rs`) that isn't there until
it is. While nothing is wrong it is silent: its node outputs exact zeros
and costs nothing. Then it rises:

- with the attention on you (`anomaly::Attention`, the same total the
  attention ringing follows), to 0.75 at most;
- to 0.5 while a response team is on its way to within 150 m of you
  (`Director::responses`);
- to 1 with the Surge (`anomaly::surge::Agitation`).

It rises over about ten seconds and falls over twenty-five. At the
truck (waiting within 12 m, extracting, at base) and when you're down,
it falls away over six. Off with the comfort setting `score`
(`--no-score`).

**What it is**, synthesised in the node one sample at a time from
`intensity` alone, and deterministic for a seed:

- long low drones on D2 (73 Hz, with harmonics so a headset's drivers
  carry it): the root and a slow-beating partner, then the octave, the
  fifth, the twelfth, and past 0.6 the minor second for unease, each
  breathing on its own 17–43 s swell;
- sparse fragments: one to three notes of D minor pentatonic (E flat
  past 0.7), swelling in over 1–2.5 s and ringing out over 4–7 s. None
  below 0.2; at 1, about one every 12 s. Nothing in it starts sharply:
  a test holds every 50 ms within 6 dB of the half second before.

**The audit check** (`scripts/audio_score.py`): `cargo test
audio_score::tests::renders_for_the_audit` renders 90 s at intensities
0.25, 0.5, 0.75 and 1, at the in-game gain (`audio_mix::SCORE_DB`). The
check holds that:

| What | Wants | Now |
| --- | --- | --- |
| Loudest 400 ms, any intensity | ≥ 3 LU under the wind bed | 3.2 LU under, at full |
| Heard at all at full | clears the wind's masking | by 15 dB |
| Every placed robot sound's audible range, score at full | ≥ 90 % of without, over the wind and over the Surge | unchanged for all 16 |

The masking uses the score's loudest 3 s in each band, so a fragment
ringing counts, not the average.

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
12. **Rooms.** Walk from the field into a cabin, a tin shed and down
    into a bunker, clapping or firing a shot in each: does each sound
    its size? A walker behind a wall: muffled but readable? Rain from
    under a roof?
13. **Where things are.** With binaural on (the default), close your
    eyes by a running generator or the truck and turn round: does it stay
    put, and is behind clearly behind? A drone overhead: above? Then
    with `--no-binaural`: is the difference worth its cost?
14. **The score.** Carry an artifact until the ringing starts, then
    wait for the Surge (`--surge-in 60`): does something low come up
    under the world without you noticing when it started? Does it make
    the moment tenser, or just busier? Can you still pick out a walker's
    steps and a drone? Does it fall away at the truck? Then try
    `--no-score`.
15. **Overall.** Anything that made you flinch, or that you didn't hear
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
