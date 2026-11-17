ARCADE — design proposal (Step 1, no code yet). Branch: claude/polish-arcade (no commits yet).

ENTRY
- `extraction --arcade [--data-dir DIR]`. One function a menu can call later: `arcade::enter(&mut LaunchOptions)` sets scene = SceneChoice::Arcade, map = "arcade_depot", robots on, director off. The camp board (camp.rs, cards TRAINING / NEW CAREER / ...) is the obvious future home: an ARCADE card that relaunches with `--arcade`, exactly as the yard's card does. Not in this slice.

WHAT A RUN IS
- Arena: "The Depot", a new small fenced map (assets/maps/arcade_depot.ron, ~200 m like the yard) built only from shipped prefabs: a brick house in the middle, a guard booth, two car wrecks and a boxcar for cover, log piles near the start. ~45 m across the fighting ground. Three machine gates (N, E, W) out at the fence, each with a way_lamp.
- Start: you stand at the bench (a table at the south edge): every range gun (weapons::range::GUNS) laid out with two loaded mags/clips each, a pistol, field dressings, a few grenades; an ammo crate that refills at each wave call. Pick what you want; the rest stays on the table for later.
- Waves, called by YOU: pull the bell cord at the bench. Nothing comes until you pull it (danger follows your action). After the pull, a 10 s warning: the board names what comes and from which gate ("WAVE 3 · DRONE + WALKER · EAST GATE"), that gate's lamp lights and a horn sounds from there. Machines then walk/fly in from outside the fence through that gate: you see them coming. The whole ladder is printed on the board: no hidden counter.
- Ladder v1: 1 walker / 2 walkers / walker + drone / hound / 2 walkers + drone / heavy / then loops with +1 machine each loop. Real spawn_walker / spawn_drone / spawn_hound / spawn_heavy with a Response job to the arena centre, so their minds run unchanged (search, flee, call).
- Score: per kill by kind (walker 100, drone 75, hound 150, heavy 400) + wave clear bonus; run time; best wave.
- End: you go down -> run over, summary on the board (waves, kills, accuracy per gun, time). RESTART card: instant in-place reset (wave machines and wrecks despawned, you healed and stood at the bench, table relaid). No reload of the world.
- Settings in the headset: the board's cards (RESTART, START AT WAVE 1/4 knob). Comfort cards as on the truck board.

WHAT IT BYPASSES
- Saves/career/progression/contracts/economy: SceneChoice::Arcade -> persist::career_path() already returns None for any scene != Raid, so nothing loads or saves the career; no persist edit needed. Raid phase forced to Out at start, Surge pushed out of reach, Director.enabled = false (no gunfire responses: the ladder is the only source of machines), truck parked off-arena, no contracts card.
- Downed: arcade catches the local Downed after raid::downed and keeps the run in the arena (no remains bag, no convoy, no logbook).
- Data: only <data dir>/arcade/. `--data-dir` respected; never touches career.ron.

REUSED UNCHANGED
- Guns, mags, ballistics, reload, stoppages, items, hands (interact), body slots, robots and their minds, robot audio/visuals, health, HUD/captions, bot driver (for the headless scenario).

PLAYTEST DATA (per run)
- <data dir>/arcade/run-<stamp>.jsonl: the sims' telemetry format (sim::telemetry SimLog: shots, hits by robot/part, damage taken by cause, death cause+finisher, kills, item uses), so scripts/sim-report.sh reads it as-is; plus arcade lines: wave_called / wave_arrived / wave_cleared, reload (gun, mid_fight: any wave machine alive and within 40 m or aware of you), stoppage (gun, kind), kill (robot, gun, ttk = first hit -> down).
- <data dir>/arcade/run-<stamp>.summary.txt: per gun shots/hits/accuracy/reloads/mid-fight reloads/stoppages; items used; per kind mean TTK; deaths and cause; waves; score; time.
- report.rs: the playtest zip gains `arcade/` (newest 4 runs). Small additive change.

FILES
New: src/arcade/{mod.rs (plugin, ArcadeRun, enter()), arena.rs (bench, table, board, cards, reset), waves.rs (ladder, gate warning, spawns), record.rs (jsonl + summary), scenario.rs (arcade_smoke)}, assets/maps/arcade_depot.ron, docs/ARCADE.md.
Shared, each flagged:
1. launch.rs: `--arcade` flag, SceneChoice::Arcade, arcade excluded from rolls_career_seed. Additive.
2. lib.rs: `pub mod arcade;` + ArcadePlugin in GamePlugin (its systems run_if scene == Arcade).
3. raid/route.rs: one-line exclusion so the truck never offers the arena as a destination (as it already does for the yard).
4. bot/scenarios.rs: `arcade_smoke` in NAMES (bot picks a gun, calls wave 1, kills the walker, asserts the record has shot/hit/kill/reload lines and no career file).
5. report.rs: include arcade/ runs.
6. main.rs: none expected (map_explicit skips choose_start).
Nothing in onboarding/, robots/, interact/, progression/, persist, HUD. Every test_maps() map test will also walk the new map (cost: some test time; it's held to the same lints).

TESTS
- `--headless --arcade --scenario arcade_smoke`, unit tests for the ladder and the summary, launch parse test. Proof normal play unchanged: scenario all + second_hour with fresh --data-dir, before/after.

Mock: 2x PNG of real prefabs laid out as the arena, from the bench at standing height (attached path below).
