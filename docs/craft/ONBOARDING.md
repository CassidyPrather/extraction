# Onboarding: how I work

> **Where I stopped (2 Nov), and what's next.** On claude/polish-onboarding,
> tip 25a7055c, its gate under way at the pause. The batch: the flat
> reload keeps its only magazine (weapons/flat.rs, guarded in
> weapons::feel); "Load up from the crate" fills from loose rounds; a
> packed loaded gun is no spare ("swap them"); out of a building by its
> plan (the post office counter), and out of a shelter by its door,
> opened again; to the truck's steps round its body and square on to the
> ramp; drone or sentry warnings send those who'd listen under the
> nearest roof; a lock is run through only at the truck's tail; the
> Payday line claims nothing it doesn't know. Last runs: first_jobs
> cautious home 5/5, killed 0, stuck 1; reckless home 5/5, unready 0.
> first_fight_drone 2/5 home (reckless, one-handed).
> **Next:** (1) the Payday line through Progression's `CardCheck` once
> c9c5efff is on main: "that's the job paid" only when `would_pay`, else
> the card's own verdict line (Err("") says nothing). (2) cautious and
> lost in first_fight_drone: bleeding mid-flight, and whether a roof is in
> reach of the old farm at all (if none, open ground after a warning is
> meant to lose; say so in the scenario). (3) the hoarder in
> first_fight_broken and _drone fails by design; mark it so. (4) `Hurt.by`
> exception kinds once Legibility's b1d1821a is on main (saved table:
> fall, mine, anomaly, surge, heat/gas, water, cage, self, aegis, plough;
> cage and aegis have no tell). (5) The diagnostic info logs in
> bot::Ready and the unready note can go once nothing's being traced.

My own manual for teaching a newcomer this game: the yard, the first hour,
what arrives when. Not rules for tests; the tests follow from it. Short, and
rewritten as I learn.

## What to teach next

1. **What the next five minutes need, and nothing they don't.** The yard
   teaches the pistol before the radio and the radio before the pack,
   because the hands are full in that order. Anything that can wait for a
   raid waits.
2. **Taught where it's used.** A step lives at the place you'd do it: the
   dressing at the house's nail, the bolt at the shimmer. The kit for a
   lesson arrives with the lesson (the truck's crate shows only the first
   station's things; `stash_station`).
3. **One new thing at a time.** One verb, one item family, one danger per
   beat. If two land together, split them or move one.
4. **The newcomer's question first.** "What do I do with this?" is answered
   before "here's another thing". If a transcript shows someone holding two
   things looking for a pocket, the order is wrong, not the player.
5. **Breadth stays; order changes.** Nothing gets cut for being much. It
   comes later (`loot.ron`'s `from_raid`: families arrive raid by raid).

## Fun, not a chore

A step is a chore when it's a tick on a board. It's fun when the world
answers back:
- **Something reacts.** Run, and the rooks go up. Step in, and the rats
  bolt. The answer is the lesson: noise carries, rats mean nobody's been in.
- **The driver says why, once.** Lev's line comes after the moment, names
  what it means out there, and is never said twice.
- **It's the game's own system.** The rooks are the raid's flocks and the
  rats the raid's rats: what you learn holds out there.
- **Low cost, real stakes later.** You can't lose the yard, but what it shows
  you can kill you on job 2.

Check with the bots: the moment happens in the yard run (the bot's report
notes it), at the step that teaches it, not before.

## The budget: one thing at a time

- **Count it.** `EXTRACTION_ITEM_CENSUS=1` logs each item kind the first time
  one comes within 6 m; `=map` lists each building's kinds. Numbers, not
  impressions.
- **Yard:** at most ~3 kinds in sight before the station that uses them. Now
  3 at the start, then about one a station, 15 in all (was 13 at once, 19 in
  all).
- **First raid's houses:** a starter set (food, a dressing, the pistol's
  rounds, light, bolts, scrap). Marrow's houses: 17 kinds on raid 0 (was 52).
- **Enemies:** the first machine you meet shouldn't be able to kill you
  (the broken walker on job 2), and every hit comes after a tell.

## Reading a playtest transcript

Look for these, timestamped:
- **Dead time.** A gap with no step done and no line said: they're lost. What
  were they looking at?
- **Repeats.** The same line twice, the same step tried three times: the
  hint isn't landing, or the step can't be done as told.
- **Looking for something we never showed.** "Where's the pocket?" "Is
  there a menu?" Each is a missing beat.
- **Two things at once.** Both hands full; two lines over each other; a
  crate of thirteen things. That's where the order breaks.
- **What they said in their own words.** "Muddy", "bombarded": quote it in
  the commit, and measure it before changing anything.
- **Then the bots.** Make the newcomer personas do what the person did (the
  wanderer, the empty-handed, the hoarder), and fix until each gets paid.
