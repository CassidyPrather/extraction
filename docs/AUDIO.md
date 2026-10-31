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
| A jam: failure to eject or feed, a double feed (`Malfunctioned`) | `gun.malfunction`, a dull wrong clack | each its own pulse in the gun hand (`weapons::faults`: 0.55/30 ms, 0.12/15 ms, 0.85/120 ms); the old generic 0.5 pulse in both hands is gone | a generic pulse as well |
| Its fragments (and any slow round) landing | the impact by surface, softer the slower: full from 300 m/s, −12 dB at 75 m/s | — | every round at full |
| A step into a puddle (rain-wet ground, `world::puddles::puddle_at` > 0.5) | `step` from `footsteps/water` | — | the ground's set |
| A sentry (`robots::sentry::SentryEvent`) | its warning (`Chirp`, a second before it fires): `sentry.chirp`, the lock tone quicker and higher, levelled to carry; its shots (`robot.shot`); a servo as a decoy draws it (`sentry.turn`); its power going, a whine winding down and a clank; its head shot through, glass and a groan | — | silent: the warning was only the lens's red flare |
| A step in snow lying (`world::snow::Snow::lies` over 0.25: Kettle Pass's high ground) | `step` from `footsteps/gravel` for its crunch, until a recording of snow can be fetched | — | the ground's set |
| The sweep detector pointing at a loose stone (`anomaly::detector::StonePing`) | `detector.ping`: synthesised (`robot_voice::render_ping`), a soft sine at 1.76 kHz with its octave, sweeter than the click, played faster and higher the nearer the stone | — | silent |
| A wet aegis biting its carrier (`anomaly::costs::ArtifactBite`, shock) | `bite.shock`, a synthesised electric crackle at the chest (`render_crackle`) | a kick in both hands (0.7/60 ms) | silent |
| A shard's late glint (`ArtifactBite`, glint) | `bite.glint`, a faint glass tick | — | silent |
| A hot stone held (`anomaly::harvest::Burned`) | `burn.sizzle`, a hiss at the hand for as long as it burns, gone when you let go | a tick in that hand every half second | silent |
| Levering with the crowbar (`interact::tools::crowbar::Pried`) | `tool.pry`, a strained creak every 0.6 s, higher and louder the harder you pull; when it gives, it's a break (`destruction::Broke`, below) | Hands' strain pulses and knock (`interact::tools`) | silent |
| The bolt cutters biting through (`cutters::Cut`) | `tool.cut`, the snip, and `tool.cut_ping` | Hands' knock | silent |
| Standing in a Hush (`anomaly::hush`) | everything 18 dB down and every placed sound (your own steps and shots too) muffled to 350 Hz, over a second; after 4 s a low pressure tone rises (`hush.tone`), a warning well before it starts to hurt (8 s). A hushstone's 1.2 m: 4 dB and 3 kHz, all the time. Checked dull, not deaf: your footstep still clears the wind by +6.1 dB, a call at 2 m by +23 dB | — | nothing |
| A squib: a reloaded round with too little powder (`FireEvent::Squib`) | `gun.squib`: the rifle's shot low-passed at 450 Hz and slowed, a weak dull pop 10 LU under a real shot up close, nothing cycling | 0.2/30 ms in the gun hand (`weapons::faults`) | the generic malfunction clack |
| A bulge: a round fired into a squib's bullet (`Bulged`) | `gun.bulged` (the shot through 1.4 kHz: muffled, wrong) and `gun.bulged_ring`, the barrel ringing | 1.0/350 ms (`weapons::faults`), and it hurts you | — |
| Rodding a stuck bullet out (`Rodded`) | `gun.rod_scrape` down the bore and `gun.rod_tink` as it drops | 0.3/40 ms | — |
| A Mosin's bolt seized after the shot (`BoltStuck`), slapped free (`BoltFreed`) | `gun.bolt_stuck`, a dull clack through 1.8 kHz; `gun.bolt_freed`, a sharp metal smack, then the bolt opening | 0.6/80 ms, a grinding 0.3 tick while you lift against it; 0.4/20 ms | — |
| A worn SKS slam-firing (`SlamFire`) | the bolt's close (a tick before), the shot on top of it, and `gun.slam_snap`, the floating pin | the shot's kick | — |
| Racking the MP-133's pump back, and running it home (`Cocked`, `BoltClosed` on a pump action) | `gun.pump_back`, the fore-end's slide and the bolt unlocking; `gun.pump_fwd`, the slap forward as it locks up (`audio::gun_sounds`) | back: 0.25/20 ms; home: the bolt-home shape, fainter in the pumping hand | silent: a pump's rack matched no arm |
| A signal flare leaving the muzzle (`weapons::flares::Flare` appearing) | `flare.pop`, a soft dull pop (a pistol's report low-passed to 1.2 kHz); then `flare.burn`, a fizzing hiss on the star as it flies and lies burning, silent until it lights and fading with its glow over its last second (`audio_kit`) | the shot's kick | the shot only; the star burned silently |
| The RPK's bipod swung down or up (`weapons::armoury::Bipod`) | `bipod.swing`, a short metal creak as the legs start to move; `bipod.lock`, a clank as they lock down or fold home; at most one every 0.15 s per gun | — | silent |
| A bare hand striking something at 1.5 m/s or more (`interact::touch::HandStruck`) | a palm slap by what it met: `hand.slap_wood`, `hand.slap_metal`, `hand.slap_stone` (concrete), `hand.slap_glass` (a small knock, the break sped up), `hand.slap_skin` (a robot's soft parts, a dull thud), `hand.slap_wire` (a chain-link rattle); 12 dB down and dull at 1.5 m/s, the cue's level and full brightness at 6 m/s (`audio_kit::slap_db`) | that material's shape, by speed (`rig::shapes`) | felt, not heard |
| A held gun carried into a wall, a doorframe or a tree at 0.8 m/s or more (`weapons::carry::GunBumped`) | what it met as a hand would slap it, 3 dB under and duller, under `gun.bump`, the steel's own knock; 12 dB down at 0.8 m/s, the cue's level at 3 m/s (`audio_kit::bump_sounds`) | Weapon feel's, by material (gun hand, support hand at 0.6) | felt, not heard |
| A chemlight snapped, a hand flare struck (`world::glow::Lit`) | the chemlight: `glow.snap`, a plastic crack at 1.5x. The flare: `glow.strike`, the cap rasping across (a zip slowed to 0.7), then `glow.burn`, the burner's roar at 2.1x, a spitting sputter that rides on the flare wherever it's thrown and stops when it burns out | — | — |
| A hang-fire (`HangFire`: the hammer fell, the round hasn't gone) | `gun.dry_fire`, a dud's click and nothing more; the shot comes 0.3-1 s later as its own `Fired` | Weapon feel's | — |
| Out of battery (`OutOfBattery`: a hanging round going off with the action open) | `gun.out_of_battery`, the shot at 1.3x through 3 kHz, flat and harsh, between a squib and a shot, and `gun.burst_case`, the case clattering out | Weapon feel's, 1.0 (it hurts the hand) | — |
| A stiff action worked (`weapons::ActionGrind`, a cold bolt, pump or charging handle) | `gun.grind`, the metal handle's scrape at 1.6x, a dry grate under the hand, its level by `stiff` (20 dB down at the faintest), at most every 0.11 s | Weapon feel's ticks | — |
| The carrier running home after a shot (`CarrierHome`), an open bolt slamming forward before one (`OpenBoltSlam`) | `gun.carrier_home`, a small, quick bolt clack sped up 1.35x a beat behind the report; `gun.open_bolt`, a heavier one at 0.9x just ahead of it | Weapon feel's beats | silent |
| Something bulky dragged over the ground (`interact::carry::Dragged`: a car door, a panel, a wheel, a body, a robot's hull) | one loop per thing, following it: `drag.metal` (a sheet scraping through its ringing modes, catching and slipping), `drag.wood`, `drag.roll` (a wheel's rumble and tread), `drag.body` (cloth and limbs in tugs); 15 dB down and low at a crawl, the cue's level at `DRAG_SPEED`; gone 0.15 s after it stops. Stand-ins (`synth_foley`), heard about as far as robots hear the drag (`DRAG_HEARD`, the audit checks) | — | silent |
| A heavy thing landing (an `ItemImpact` over 8 kg: a thrown door, a wheel) | its drop sound, with `drop.heavy` under it, the ground taking the weight (earth at half speed), louder with mass and speed | — | — |
| A scope's elevation drum turned a detent (`weapons::zero::TurretClicked`) | `scope.click`, a small quick click | a tick in the turning hand (`weapons::zero`) | silent |
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
| A robot's mind turning: on its rounds, noticing you, going to look, searching, certain of you, losing you, calling the others, answering a call, hurt, dying (`robot_voice::Spoke`) | `voice.<mind>`: its kind's word for it, synthesised (see "What the machines say"); one cue per turn, never the same word twice within 4 s | a faint tick on the left wrist when one within 25 m turns certain (comfort `machine_tick`) | the servo whirrs, sweeps and a squelch; no hurt, dying or answering |
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

**Robot radio talk** (`machines/robot_voice`, the chatter your radio
picks up, synthesised in `scripts/synth_anomaly.py`, as
`docs/DESIGN.md` allows; what robots say aloud is
`src/robot_voice.rs`, below). They're
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

**Haptic shapes** (`src/rig/shapes.rs`). A few pulses in a pattern, for
things the hand should tell apart without looking. Each pulse is on the
scale above. An empty hand meeting something (`interact::touch`, the
physics hands) plays its material's shape, scaled 0.4-1.0 by how fast
the hand was going (full from 2 m/s). Pulses that would come out under
0.1 are dropped.

| Shape | What | Pulses (start s, amplitude, length s) | How it feels |
| --- | --- | --- | --- |
| `WOOD` | planks, logs, timber doors and crates, trees, wooden items | (0, 0.42, 0.03) | one dull thud |
| `METAL` | sheet and painted metal, robot armour, metal items | (0, 0.5, 0.012) (0.035, 0.25, 0.012) (0.07, 0.12, 0.012) | a sharp hit and a ring dying away |
| `GLASS` | glass, glass items | (0, 0.3, 0.01) (0.025, 0.3, 0.01) | two quick light ticks |
| `STONE` | brick, concrete, plaster, rock, the ground | (0, 0.6, 0.018) | hard and short, no give |
| `SKIN` | a robot's soft parts (anything but its armour) | (0, 0.18, 0.025) (0.03, 0.26, 0.035) | soft, gives, then holds |
| `WIRE` | fences, chain-link | four ticks 30 ms apart, 0.3 / 0.22 / 0.26 / 0.16 | a rattle |
| `BOLT_HOME` | a gun's bolt closing or locking | (0, 0.6, 0.015) (0.03, 0.3, 0.025) | a slam, then it settles |
| `MAG_SEAT` | a magazine seating | (0, 0.2, 0.02) (0.045, 0.55, 0.025) | the slide up the well, then the catch |
| `CHAMBER` | a round chambered or loaded | (0, 0.3, 0.012) (0.025, 0.2, 0.012) | a double tick |

The gun shapes play in the grip hand, and at half strength in a
supporting hand. A hand going 1.5 m/s or faster strikes rather than
touches. That writes `interact::touch::HandStruck { at, stuff, speed,
side }` for a slap sound by material.

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

### Finding a threat by ear

Hearing that a drone is near is not enough: you have to find it. The
localisation pass (`scripts/audio_localise.py`, run by the audit) takes
each threat's own sound: the drone's rotor, a walker's footfall, a
sentry's chirp, and the crawler's feet and its charging spark gun. It
plays one second of it, at the ranges where the threat matters, with the
distance muffle and any filter of its own, through the head-related
responses on a grid of bearings and elevations
(`audio::tests::localisation_grid` writes them). It then asks for three
cues:

- **Side**: a source at 90° must be either 3 dB louder in the near ear
  or at least 300 µs earlier there.
- **Front/back**: ahead must be at least 2 dB brighter than the mirror
  bearing behind (4-10 kHz over 0.5-2 kHz, at 0/180° and 30/150°).
  60/120° isn't asked: so near the ears' axis the responses themselves
  barely differ. The reference stops at 2 kHz because our responses are
  already 3 dB brighter ahead from 2 kHz up. Measured against 0.5-3 kHz,
  the drone's rotor, whose low band sits there, read as no brighter ahead
  than behind.
- **Above** (the drone): at least 2 dB brighter overhead than level.

It found two gaps, both fixed:

1. **Past 40 m everything was panned** (`BINAURAL_REACH`), and the panner
   gives a level difference only. A drone at 60 m was no more ahead than
   behind, nor above. Signals (threats) now keep the ears' filter to
   `SIGNAL_BINAURAL_REACH` (150 m, `audio::binaural_reach`). Details
   still pan past 40 m, which keeps the voices (`HRTF_VOICES`) for what
   matters.
2. **Flat mode (binaural off) had no front/back at all.** The panner now
   muffles a sound straight behind you to 5 kHz, easing in from the side
   (`audio::behind_cutoff`, set by `occlude`, with occlusion's own muffle
   taking the lower of the two). Flat, every threat now reads 9-18 dB
   brighter ahead. Nothing in flat mode can tell above from level.

In the headset every threat now separates side, front from back by 4-6
dB, and the drone's height by about 4 dB, at every range measured (to
100 m for the drone).

## One world, one hearing

What you hear muffled, a robot hears cut short, by the same number.
`src/hearing.rs` (`HearingPlugin`, headless) builds each building into a
small graph:

- **Rooms.** The floor is flooded with the doors shut, and whatever reaches
  the edge is outside. Walls are found at sill, ear and lintel height, as
  thin, long pieces (a table is thick both ways). Where a cell is walled
  below and open at ear height, it's a window.
- **What joins rooms.** A door: open, 3 dB; shut, 5 dB for wood or 8 dB for
  steel. A wall, by what it's built of (`touch::Stuff`): timber 6.9 dB
  (the robots' `OCCLUDED`, which anchors the rest), metal 8, brick or
  stone 10, a window 4.
- **Room to room.** The loss from every room to every other is solved
  once, and again only when one of the building's doors moves. Asking is
  two lookups and a table read.
- **Storeys, sites and the open.** Each storey up adds a floor (6.9 dB). A
  site underground reaches the surface by its way out or through the rock
  (14 dB), whichever is louder. Every site's ways out are its kiosks: the
  room inside each kiosk's door joins the outside through that door, by
  its state (or open, under an arch). Out in the open on both ends, a hill on
  the heightmap or anything `acoustics::blocked` catches (a compound wall,
  an outcrop, a house between) costs a wall.
- **One knob.** `WALL_SCALE` moves every wall and door together. It's Robot
  minds' to tune if indoor stealth swings.

Both ears ask it. `audio::occlude` sets a sound's gain and muffle from
`Hearing::path`. `robots::senses::perceive` scales a sound's reach by
`10^(db/20)` from `Hearing::heard`, which also says whether anything beyond
an open doorway was in the way (for a site's corridor walk).
`Hearing::smear(at)` is how much a room spreads a robot's guess of where a
sound came from: 1 in the open, up to 2.5 in a rock gallery, by the room's
size and how hard its walls are.

**The gauge** (`hearing::gauge::robots_hear_what_you_hear`, on Marrow
Creek) finds a real walker's hearing threshold: the quietest noise it
hears through the obstacle, against the same distance in the open. It
asserts that the walker's loss and the player's agree within a dB:

| Pair | Player | Walker |
| --- | --- | --- |
| A house, its door shut (heard by way of its window) | 4.0 dB | 4.0 dB |
| The same, the door open | 3.0 | 3.0 |
| The ranger tower's cabin, from the ground below | 4.0 | 4.0 |
| The bunker's deepest room, from the surface over it (its doors shut: the rock is quieter) | 14.0 | 14.0 |
| The bunker's way in (inside its kiosk), from the ground at its mouth, its steel door shut | 8.0 | 8.0 |
| The same, the door open | 3.0 | 3.0 |

## Hands on the machines

`src/audio_fight.rs` (`FightSoundsPlugin`) and `audio::hit_sounds`:

| What | Sound |
| --- | --- |
| A blow: a crowbar, a knife, a stock (an `Impact` slower than `BLOW_SPEED`, 40 m/s, which no round is) | `hit.blow`: what it struck, slowed (0.75-0.8) and duller than a round's hit, 12 dB down at `BLUNT_MIN` (2.5 m/s) to the cue's level at 10 m/s (`audio::blow_db`). Steel struck hard (over 6 m/s, a robot's panel) rings with `hit.blow_ring`, a clank under it. Until now a blow played a round's ping 12 dB down |
| A palm shoving a robot (`interact::fight::Shoved`) | `fight.shove`, the flat of the hand on its plate (metal at 0.7), and `fight.stagger`, its servos catching it; 12 dB down at an 8 N·s push to the cue's level at 40 |
| A robot fighting a grip (`Gripped`) | `fight.strain`, its servos whining against the hand, louder and higher the harder it pulls, every 0.35 s while it does |
| A robot kicking free (`robots::grip::KickedFree`) | `fight.kick`, a hard clank, and its servos snapping round |
| The crawler moving (it has no gait message: its speed is read from where it was) | `crawler.step`, many small metal feet (the walker's footfall at 2.2x), pattering every 0.11 s at a run, slower and softer at a creep; heard to 15 m (8 in a downpour) |
| The crawler's spark gun charging (`robots::indoor::CrawlerCharge`, the tell) | `crawler.charge`, the electric crackle at 0.8, a Signal |
| Its zap (`CrawlerZap`) | `crawler.zap`, a hard crackle snap at the emitter, and 6 dB down where it lands |

| A held robot torn out of the hand (`interact::fight::TornFree`, a drone thrusting free) | `fight.torn`, metal wrenched (a creak at 1.9x), and `fight.lurch`, the machine whooshing away |
| A round out through steel, a plank or a wall (`impacts::is_exit`) | the exit's crack, 6 dB under the entry, and `hit.spall`: bits clattering out 0.4 m past the hole (steel's drop at 1.7x, wood's knock at 1.4x, grit at 1.5x) |
| Winding a dressing round a limb (`first_aid::Winding`) | `aid.wrap`, a short rasp of the bandage at each quarter turn; the tear when it's on is `use.bandage` |
| Strapping a splint (`Strapping`) | `aid.strap`, a strap pulled tight (velcro) each half second, as the hand feels it; `aid.cinch` as it's on |
| A bottle's cap twisted (`Capped`) | `aid.cap`, a plastic tick each quarter turn, and `aid.cap_off`, the pop; the swallow is `use.drink` |
| A hand at an open wound (`wounds::Bloodied` coming on) | `aid.wet`, a wet squelch |

Until now the crawler made no sound except its voice.

## What the machines say

Every robot that has a mind speaks it (`src/robot_voice.rs`), so you can
read a machine's intent by ear: a small vocabulary, one word for each
turn of its mind, and **one grammar for every kind**. Learn the walker's
and the hound's makes sense.

| Mind | The word |
| --- | --- |
| on its rounds (within 70 m of someone, every 18-40 s) | two soft low notes, a little apart |
| noticed something | one note gliding up: a question |
| investigating | three short notes stepping up |
| searching | four short notes see-sawing: scanning |
| certain, engaging | two hard notes, high and falling: a statement |
| lost you, giving up | one long note sinking away |
| calling the others | three pulses rising to a held note |
| answering a call | the call's top note, then a step down: "heard" |
| flanking (working round to your side) | a quick low-high pair, twice, running on: "moving" |
| retreating (breaking off, badly hurt) | a strained note that stumbles down in two steps |
| hurt (a part hit it survives) | a stutter of rough notes, pitch thrown about |
| dying | one long fall, slowing and breaking up |

**A voice per kind**: the drone a quick thin whistle chopped by its
rotors; the walker a hollow mid voice; the grappler the walker's with a
metal ring; the dirigible a slow deep horn; the crawler fast insect
chirps; the hound a rough bark; the warden a low heavy walker; the heavy the
deepest of all, a brassy horn of a voice. Pylons
and sorters are mute. Every word is rendered at start-up, in the
background on the compute pool (never on a frame: rendering on first
use cost a 340 ms hitch when a walker first died), at the stream's
rate, deterministically from its kind, mind and variant (three of each),
and levelled to one loudness. It plays as an ordinary placed sound, so
it's heard through the head-related filter, muffled by walls and doors,
and sent to the room's reverb.

**Who says what** (`VoicePlugin`, headless): it reads Robot minds'
one hook, `robots::search::MindShift` (one per change of mind, never the
same state twice running; its own test holds that). A robot going back
to its rounds says so only within 70 m of someone. Each turn is said exactly once (tested). The two
nearest robots that heard a call answer it a beat apart, and a response
team answers as it sets out from wherever it starts. Calls and answers
carry about 200 m in calm (`RANGE_TARGETS`), so a call answered far off
is the warning that they're coming. The two agree: your radio, if you
carry one, crackles with their chatter when the director sends them
(`raid::chatter`), and their answer comes from where they start as they
set out, so the radio tells you first and your ears confirm it.

**One word at a time.** A robot whose mind turns several ways in a
moment (noticed, then calling) says each word when the last has
finished, in order; dying doesn't wait, and silences what was left.

**Listening** (`--headless --scenario listening`, in `all`): the bot
stands by the truck while a drone sets out 185 m away and a walker
comes up the road. Every word is logged with its distance, the wall it
came through (`acoustics::occlusion`) and whether you'd have heard it,
judged by `robot_voice::heard_to` (each word's audible range in calm,
measured by the audit, which flags drift over 25%; 6 dB of wall halves
it). It passes if both answers to the dispatch are heard and at least
four words in all. A run:

```
  0.0 s  walker    idle             46 m   +0.0 dB  -
  0.5 s  drone     answering       185 m   +0.0 dB  HEARD
  0.6 s  walker    answering        46 m   +0.0 dB  HEARD
  2.0 s  walker    investigating    43 m   +0.0 dB  HEARD
  2.5 s  walker    engaging         43 m   +0.0 dB  HEARD
  2.8 s  walker    calling          42 m   +0.0 dB  HEARD
  4.3 s  walker    noticed          41 m   +0.0 dB  HEARD
  5.4 s  drone     calling         148 m   +0.0 dB  HEARD
  6.4 s  drone     noticed         135 m   +0.0 dB  -
  6.8 s  drone     investigating   130 m   +0.0 dB  -
 11.9 s  drone     engaging         57 m   +0.0 dB  HEARD
```

(Times are when each word is spoken, one after another per robot. The
walker's "noticed" after "engaging" is the order its mind reports them
in: its suspicion is announced crossing `NOTICED` after it has already
turned to fight; raised with Robot minds.)

**The score steps aside** for them: a voice within 80 m ducks the score
12 dB for as long as it speaks (`SCORE_DUCK_DB`). The words that fall,
giving up and dying, land on the drones' register, and the voices are
the information; the score is only the mood.

**The audit** (`scripts/audio_voices.py`) measures every rendered word:

- each mind's word is within 6 LU across kinds (with `voice_trim`; now
  1-4 LU);
- with any kind saying any word 6 m off, your footstep over the wind
  still clears its masking (now +12 dB) and your rifle does too
  (+28 dB);
- the mix pass gives each `voice.*` cue its audible range (calls 248 m
  calm, 231 m in a downpour; the rest 50-115 m). The startle check keeps
  a call up close under your rifle with normal dynamics and within 2 LU
  of it with narrow;
- the score pass: with the score at full (ducked), every word keeps 90%
  of its range.

## The heavy machine

The heavy (`robots::heavy`, four metres of walker) is heard well before
it's seen (`heavy_sounds`, `heavy_loops`):

| What | Sound | Heard to (calm / downpour) |
| --- | --- | --- |
| A footfall (`HeavyAct::Step`) | `heavy.step`: a heavy explosion low-passed at 220 Hz and slowed to half, mostly under 100 Hz, a thud you feel; `heavy.foot`, its metal foot, over it up close. Its `WalkerStep` is skipped (it comes for the same foot) | 175 / 163 m |
| The arm rising, the 1.2 s tell (`Raise`) | `heavy.raise`, a slow servo, and `heavy.hiss`, the hydraulics | 132 / 87 m |
| The blow (`Slam`) | `heavy.slam`, a low boom through 600 Hz, `heavy.clank`, and stone if it hit | — |
| A door torn off, a bolt shot out, something loose, a plate down | `heavy.tear` (creak, strain, splintering), `heavy.bolt` (a ping), `heavy.loose`, `heavy.plate` | — |
| Steam vented, the stall's roar as it sinks, a well holding it, a cell pulled, the fall | `heavy.vent`, `heavy.stall`, `heavy.strain`, `heavy.cell`, a boom and a groan | — |
| Idle | `heavy.idle`: the hum, slowed to 0.55, on it | 201 m |
| Called out of the Gate (`heavy_mind::HeavyCalled`) | `heavy.horn`: synthesised (`robot_voice::render_horn`), two long low tones a fifth apart (D1, A1), 6 s, from the Gate | 705 / 658 m |

Its mind speaks the robots' grammar in its own voice, the deepest
(`VoiceKind::Heavy`, 65 Hz, a brassy saw through a 320 Hz formant, slow):
within 1.4-3.8 LU of the other kinds for every word. Within 25 m its
footfalls and blows are felt in both hands (Heavy's
`heavy_look::footfall_pulse`, from a tick to just under a knock).
Everything sudden is kept under your rifle up close (the horn 7.5 dB
down from where it first measured, for anyone standing at the Gate).

## Breaking things, and the birds

**Breaks** (`destruction`, `break_sounds`), each levelled at its
biggest so the audit sees the worst; the code only takes it down:

| What | Sound | You hear it / robots do (m) |
| --- | --- | --- |
| A step on broken glass (`glass::Crunch`) | `break.crunch`, a pane's break quick and short (a recording of glass underfoot when one can be fetched) | 27 / 22 |
| A pane (`Broke`, glass) | `break.glass`: a shop window lower and longer than a vent pane, quieter the smaller | 57 / 55 |
| Wood (`Broke`, wood) | `break.wood` and `break.splinter`; a door coming down thuds (`break.wood_big`) | 28 / 36 |
| A wall breached (`Broke` of 3 m²) | `break.breach`, masonry going, `break.rubble` pouring, and the boom | 46 / 40 |

**Flocks** (`wildlife::Flushed`, `flock_sounds`): wings and the birds'
calls by the flock's species (rooks as crows, ravens, jackdaws, a
heron's croak, choughs' ringing calls); a burst of calls when alarmed,
one softer call when it's only restless. **Animals** (`animal_sounds`):
a roe deer's hoarse bark, a chamois' whistle, a dog pack barking, a
dog's yelp, hooves bolting through the leaves, rats in the walls, the
thump of an animal shot.

**Stand-ins.** Freesound has been out of reach from this environment's
network, so the sounds of real things not yet in the bank are
synthesised (`src/synth_foley.rs`): small models of each (a bark is a
pitched, buzzy burst through a dog's formants; a drip a droplet's
rising chirp; a breath shaped noise through the mouth), deterministic,
rendered with the robots' words at start-up and audited like any set.
They are: the deer's bark, dogs' barks and yelp, rats, the chough, the
heron, the chamois, ducks, bats, your breath, drips and lapping. A
raft of mallards going up quacks and splashes across the water in its
run-up; bats at dusk and night are a quiet high flicker from the
nearest one hunting within 15 m. Each
yields to a recording of the same name the moment the bank has one
(`render_voices` skips a stand-in whose set exists). Layered from
recordings meanwhile: wings (cloth), hooves (leaves, quick), glass
underfoot (a pane's break, quick), snow (gravel), the scour outflow
(the creek, slowed).

**Breathing** (`breath_sounds`): winded (stamina under 0.6), a breath a
second, louder the more winded, muffled to 1 kHz and a third as loud
with a hand over your mouth, as `interact::gestures` lets robots hear
it. You hear a full one 7 m off, robots 6 m.

**Water** (`water_sounds`): any wide water (20 m and more: Sallow's
flooded arm, a lake) laps at the shore nearest you within 40 m; Sallow's
scours drip at the draw-off tower's foot.

**Checked** (`scripts/audio_voices.py`): every sound breaking things,
the birds and the heavy machine make, at its usual distance over the
wind, leaves your footstep clearing its masking (worst +12.8 dB, a pane
going) and a robot's call 30 m off clearing its (worst +8.9 dB, the
heavy's footfall); each break you hear about as far as robots do
(0.6-1.6x).

## Sallow Reservoir

- **The dam slaps back** (`acoustics::slapback`, `slapbacks`): a shot,
  a grenade, the heavy's footfall or blow, or a wall breached, with you
  and the sound both in front of one of the dam's faces (the downstream
  face, 22 m of masonry; the upstream wall, 21 m), comes back off it from
  the sound's mirror image behind the face, after the extra path at
  343 m/s, through 2.5 kHz. It is never occluded (it's behind the dam on
  purpose). A shot fired 100 m out comes back 0.58 s later, 11 LU under
  the shot and 5.5 dB over the wind; closer than ~10 m, the echo would
  fuse with the shot, so there is none (`SLAP_AFTER`, 60 ms).
- **The draw-off tower** (probed at its bridge from the crest): a hard
  room, a 2.2 s tail at -12 dB wet: a tall concrete shaft. **The crest**
  is open and dry, which is why the slapback matters. The dam's
  inspection gallery has no reverb yet: it isn't built (Underground's).
- **Wind** over the open bed is the mountain wind (`wind_mountain`,
  Fourth valley's sounds block); **the river** below the dam is the
  creek, which follows any valley's water.
- **Silt underfoot**: on the drained bed (`world::terrain::Drowned`,
  the same definition the ground's look uses: under the old water line,
  behind the dam) footsteps are `footsteps/mud`.
- **The scours**: the valves the Custodian opened still run, roaring out
  of the valve house at the dam's toe into the river (`OUTFLOWS`,
  `water.scour`, -26.5 LUFS at 20 m): the creek's recording slowed to
  0.75 and louder, until an outflow can be recorded. The spillway is
  dry (the reservoir is drained below its sill).
- **Drips and lapping**: the scours drip at the draw-off tower's foot;
  New Sallow's flooded street laps at its edge (`water_sounds`; both
  stand-ins until recordings can be fetched).

## Rime Fell

Nothing on Rime Fell runs. Every sound there is the wind working on the
derelict works. `world::fell_sounds::emitters` (Fifth valley's) says
where each one is, worked out from the map, and `audio::fell_sounds`
voices them:

- **The funicular's rope**: the slack rope creaks on its rollers at the
  roller nearest you within 60 m, every 2-6 s (`fell.rope`).
- **The bull wheel**: the top station's wheel, 8 m up, groans as the cut
  rope's end tugs at it (`fell.wheel`).
- **The mill's loose sheet**: a corrugated sheet on the headhouse,
  about 20 m up, bangs one to three times every 4-15 s. It is heard to
  160 m, across the settlement (`fell.sheet`).
- **The stopped car**: a door swings (the rope's creak, quicker) and
  bangs to about half the time (`fell.car`).
- **Wind through the works**: a loop in each mill bay, which the
  acoustics probe makes roomy, and along each avalanche gallery
  (`fell.wind`). The galleries' meltwater drips within 50 m.
- **The mill slaps back**: its front, 20 m wide and 9 m up, facing down
  the slope, is a reflector (`acoustics::reflectors`, "ore_mill"). A
  shot 50 m down the slope from it comes back about 0.3 s later.

## What your gun and your hits tell you (the playtest of 28 October)

**Your gun's state** (`scripts/audio_gunstate.py`): every cue that says
what the gun is doing (the slide locked back, the trigger falling on an
empty chamber or a hammer already down, the trigger blocked, the safety,
and a press with the action open) is played in the hand under a walker
suppressing at 20 m, your feet at a run, and both, and must clear the
masking by 6 dB. The slide-lock is the slide's half of a rack, the safety
a switch, the dead trigger a dulled dry-fire click. (The slide-lock lands
with your own last shot: the press after, `gun.action_open`, is the tell.)

**A robot struck** (`audio_machines::struck_sounds`, `RobotStruck`): by
what the hit did. Shrugged off, a thin tink; a wound, the impact and a
low thud; telling (a weak spot, a part giving way), a crunch and an
electrical crack, a sensor's glass breaking. At 30 m telling stands 11.7
LU over a shrug (wants 10), and a wound between them.

**A walker hunting you is never silent:** standing to look, its head
sweeps round (`walker.scan`), heard to 81 m.

## The machines' plan, heard

When machines share a plan on you (`robots::squad::SquadCall`), each
part chirps its own pattern at the machine that takes it, with a tick
under it to place it by (`audio_machines::squad_sounds`): holding you,
two low, level notes (`squad.pin`); going round to your side, up and
back down (`squad.flank`); making for your way out, a sharp fall
(`squad.cutoff`); a drone lighting the way in, a high trill
(`squad.spot`); a relay calling it in, long-short-long (`squad.call`).
None rises like a question (`device.notice`). Each is heard to the
chirp's reach, 40 m in calm and 20 in a downpour (`RANGE_TARGETS`), and
placed by ear (`audio_localise.py`). The radio's line is Robot minds',
heard as chatter. A relay reporting you (`robots::recall::Filed`) gives
three falling notes, "noted" (`relay.filed`), quicker and fuller the
more often it has.

The walker pinning you going over to suppressing fire while another
moves up (`robots::walker::Suppressing`) racks its feed, heavy and slow,
with a mode's snap (`walker.suppress`, `walker.suppress_mode`): unlike its
aim's clank and servo, and heard to 38 m in a downpour. It racks down
when it stops (`walker.suppress_off`). Its faster fire says the rest.

A machine the Zone does something to (`robots::hazards::HazardStruck`)
is heard at it, by what it does (`audio_machines::hazard_layers`): a
drone thrown by a lift, its rotors screaming as it tumbles; a walker
thrown, a servo's lurch and its foot scraping; dragged in a well or
stiffened by frost, its legs grinding; crushed at a well's heart, a metal
groan and a hit; dazzled, its sensor stuttering and its lock clicking
out; stunned, its servos winding down; hurt by what makes its own sound,
a short distress note. Deafened by a hush, nothing: the hush is heard.

A minelayer crawling grinds, its treads labouring, louder and quicker
the faster it goes, silent standing (`audio_machines::grind_sounds`,
`minelayer.grind`). Stopped over a spot to lay, its chute charges with a
hiss for the stop's two seconds before the mine drops with a chunk
(`hiss_sounds` on `Minelayer::laying_in`, `minelayer.hiss`, then
`mine.laid`). It's a breathy, low hiss, so a shower doesn't wash it out.
Both are heard as you near it, 30 m in calm and at most 75, but a
downpour hides them within 10 m (`RANGE_TARGETS`, `RANGE_MOST`). A
grappler's line on you strains at its winch the whole time it holds,
harder near the end (`grapple.winch`). Let go, it whips slack with a
clank (`grapple.slack`, `grapple.clank`).

## Underground's sites

The signature sites under the map (`world::underground`, `::workings`,
`::furnace`) are heard by `src/audio_underground.rs`
(`UndergroundSoundsPlugin`, client only), which reads their state and
messages. Every cue is an `ug.*` line in the mix, and the code plays the
set that line names. Loops (a basin draining, the cart, the crane, the
heat) are spawned when what makes them starts. They ease in and out, and
are dropped once they're silent. Per-entity timers forget whatever is
gone.

| What | Sound |
| --- | --- |
| A valve wheel turned (`DrainControl` opening) | `ug.valve`: the door's creak at 0.75, the rusted stem shrieking round |
| A hand pump's stroke (pressing a pump `DrainControl`) | `ug.pump`, the iron handle knocking, and `ug.pump_water`, a gout from the spout |
| A basin's level falling (`Levels`) | `ug.gurgle`: the creek slowed to 0.6 and through 900 Hz, water going down a pipe, from the basin's water; held 1.2 s after the level last fell, so pump strokes run together |
| A basin drained (`BasinDrained`) | `ug.drained`: a long glug (a splash at half speed) as the pipe sucks air |
| Breathing firedamp (`InGas`) | `ug.cough` (a stand-in: a fit of one to three coughs) or `ug.wheeze` (the breath sped up), every 1.6-3.2 s at the head |
| The canary (`Canary`) | `ug.canary` (a stand-in: a run of thin falling "tsip" notes at about 4 kHz). With no gas near, a calm run every 4-9 s, 10 dB down. With gas near, runs come quicker, louder and higher as `alarm` rises, to four a second in the gas. Dead (`life` 0), it is silent. Heard within 30 m |
| A pocket going up (`GasFlash`, `GasIgnited`) | `ug.whump`: the heavy grenade at 0.7 through 300 Hz, a muffled blast heard through rock, not a bang; and `ug.timbers`, the groan slowed, as the shock goes through the shoring |
| The mine cart (`MineCart`) | `ug.cart`: the truck's idle through 600 Hz, at 0.5-0.8 speed and up to full level with the cart's speed; `ug.cart_clack` at each rail joint (every 2.5 m); `ug.cart_stop`, a clank, when it stops hard at the rail's end |
| A roof coming down (`RoofFell`) | `ug.roof`, the rockfall, and `ug.roof_crash`, concrete landing slowed; 1.2-2 s later, `ug.dust`, the rockfall sped up and quiet: grit still trickling |
| Standing in the heat (`InHeat`) | `ug.sizzle`: the burner's roar at 1.8, louder the hotter, about the body; your own breath (`ug.heat_breath`) every 2.2 s once the heat is past a quarter |
| A damper opened (`Vented`) | `ug.damper`, a metal clang slowed to 0.55, in the flue over its lever, and `ug.flue`, a long whoosh: hot air going up |
| A zone cooling (its `Heat` share falling) | `ug.heat_tick`: a casing's tink slowed, at random points in the room, quick at first and slowing as it cools; it stops when the room has cooled. Heard within 35 m |
| The crane running (`Crane::dir` not 0) | `ug.crane`, the hum at 1.6 (the trolley's motor whining); `ug.chain` clinking at the hook every 0.25-0.5 s; `ug.crane_clank` as its brake lets go, and again at the end of its run |
| A vent's or the crane's lever (`VentLever`, `CraneLever`) | `ug.lever`, a heavy thunk under the press's click |
| A cage bell pull rung (`world::cage::CageRung`) | `ug.cage_bell`: the mine bell, three strokes 0.45 s apart, a Signal heard to the bell's 35 m (`BELL_HEARD`, the audit checks) |
| The cage starting to ride (its `wait` out, `to` set) | `ug.cage_strain`, the groan slowed: the winding engine taking the strain at the shaft's head; then `ug.cage_rope`, the hum at 1.3, the rope singing over the sheave, from the head (not the cage) while it rides, easing out at the stop |
| The cage at a stop (`CageArrived`) | `ug.cage_land`, a clank onto the keps, and `ug.cage_gate`, the latch |
| The cage come down on someone (`CageCrushed`) | `ug.cage_crush`, metal slowed to half: the iron floor's thud, and `ug.cage_body` |
| A step carried down Sallow's dam gallery (`world::echo::Echoed` with a player: upright, at a walk or faster) | `ug.echo`: a concrete footfall (the concrete impact, its variants even where the concrete steps' are not: `concrete_5` sits 5 LU under the rest at the set's peak, to be re-picked when the bank can be rebuilt) from the gallery's far end (`from`), late by the round trip (0.1-0.25 s), through 2.5 kHz and into the reverb. A creep sends nothing and is quiet; that's the lesson |
| The shelter's klaxon (`world::lockdown::Klaxon`, every 2.5 s for 20 s) | `ug.klaxon`, an electric klaxon (stand-in `alarm/klaxon`: a clipped buzz near 420 Hz on its motor's cam, through the horn's flare), from the hall's ceiling, a Signal heard well past the klaxon's 50 m. The tape's spoken line waits for a recording; the text says it |
| The shelter sealed (`Sealed`) | at each gas-lock door, `ug.seal_slam` (a door slammed, slowed to 0.7), then its two bolts shooting home (`ug.seal_bolt`, clanks 0.45 and 0.63 s later) |
| The cinder tunnel's water live (`world::live::Live` holds the site) | `ug.mains`, the hum slowed to 0.6, at the arcing, easing in and out with the power (the isolator pulled lets it die away); `ug.arc`, the crackle's snaps every 0.3-2 s |
| A jolt from the water (`Shocked`, every 0.5 s you stand in it) | `ug.shock`, a hard crackle, and `ug.jolt`, the body's shudder |
| The isolator thrown (`Isolated`, either way) | `ug.isolator`, a heavy switchgear clunk at the isolator |

Two stand-ins were added (`synth_foley`): `body/cough` (noise and a
rough voice at about 160 Hz through the throat's formants, sharp onset,
each cough weaker than the last) and `wildlife/canary`.

**The stair breathing out.** Each kiosk door is a tell for what's below
(`stair_breath`, by the site's prefab, `breath`): faint, from just
inside the door, full within 5 m, gone by 9 (`breath_heard`), and gone
again once your head is 2.5 m down the stair, where the site's own
sounds take over. The shelter: a slow drip, the air handling's dead hum
(`ug.breath_hum`), a far ventilator's blade ticking round. The drowned
works: running water, a pump's distant thud. The workings: a draught's
moan, drips, the timbers creaking; the deep workings add the firedamp's
faint hiss. The cinder tunnel: the mains' hum and a cable's buzz. The
furnace cellars: a warm roar and metal ticking. The dam gallery: water
seeping, every drop answered late and faint down its length. The winze:
wind down the drift and the cage's rope creaking. The robots' depot, occupied:
a gantry's hum, a conveyor's distant rattle, now and then a servo. Every layer is a
`ug.breath_*` line, its low-pass in `CUE_LOWPASS`.

**Audited** (`scripts/audio_audit.py`; `ug.gurgle`, `ug.whump` and
`ug.cart` are judged through their low-passes, `CUE_LOWPASS`). Every
detail and action is within 2.6 LU of the mix line; the worst is the
cart's rumble at +2.6 LU, then the heat's breath at -1.2 LU. Up close,
the loudest is the roof's crash, 2.7 LU under your rifle (5.7 with
narrow dynamics). The canary comes within 2.3 LU of the rifle with
narrow dynamics. The whump is heard at -28.8 LUFS from 10 m, 5.9 LU
under the rifle. The canary is heard 201 m away over the wind and 16 m
over a downpour (target 15 / 8). No `ug.*` cue is flagged.

## Storms

A thunderstorm (`Conditions::storm`, half of all downpours) is heard
by its lightning (`world::weather::Lightning`, `storm_sounds`): each
strike's thunder arrives `distance / 343` s after the flash
(`storm::thunder_delay`), from the strike's direction. Near (under
600 m): a sharp crack echoing the flash's strokes, then the long roll;
far: the low rumble only, through 1.5 kHz at 600 m darkening to 300 Hz
at 2.6 km. 6 dB quieter each doubling past 250 m, and up to 6 dB
quieter for a faint strike. The nearest, brightest strike is kept under
your rifle (the roll 2.1 LU, the crack 3.7 LU). In a storm the random
far thunder keeps quiet: every roll is a flash's.

In any downpour a wall of rain sweeps over every 10-20 s (the rain
sheets crossing the valley): the rain bed up 4 dB and back over 4 s.

## The night

After dark (`world::weather::Hour::Night`; half at dusk) the valley
sounds different (`night_ambience`, `ambient_oneshots`):

- **Insects** (`ambience/insects_night`: TRP's cricket chorus, Ontario,
  CC0) outside, at the level Wildlife's chorus gives
  (`wildlife::insects::Chorus`, crickets at dusk and night): silent
  within 60 m of a walker, hound or the dirigible and 120 m of a drone,
  knocked down by a shot, a flush or a bolt within 150 m, back over a
  minute, thinned by rain and wind. 12 dB down through walls, gone
  underground. The crickets stopping is often the first you know of a
  machine.
- **Wind in the leaves** (`ambience/rustle_leaves`, high-passed at
  250 Hz so it's the leaves and not the gusts on the microphone), louder
  the more overgrown it is round you: the ivy and weeds on the buildings
  within 40 m (`world::overgrowth::amount`, nearer counting more), or
  the forest's cover where there are none.
- **The Surge's thunder**: the far thunder comes more often at night
  (every 35-90 s), deeper (0.78 speed), a little louder
  (`thunder.surge`).
- **No birds.** They used to sing all night.

**The audit** (`scripts/audio_voices.py`): the insects clear the wind
by 24 dB at 8 kHz and the leaves by 19 dB at 16 kHz, so both are
heard. Over the night's beds every robot's word keeps 75% of its calm
range (now 87-100%) and your footstep still clears them (+6.7 dB). A
walker's step and a drone's rotor are warned of to the mix's own target
(60 m, 120 m): the crickets would hide the rotor past 71 m and the
step past 46 m, so Wildlife's hush radii (`HUSH_NEAR`, `HUSH_NEAR_DRONE`,
read by the audit, held at 60/120 by their test) cover the rest.

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
