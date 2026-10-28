"""The gun-state pass of scripts/audio_audit.py: in a fight, can you hear
what your gun is telling you?

The playtest of 28 October (docs/playtests/2026-10-28/ANALYSIS.md, items 3
and 4) ended with a seated magazine and an empty chamber: the slide had
locked back on the last round and the player never heard it, nor the
trigger falling on nothing. Each cue that tells you the gun's state
(STATE) is played in the hand, under what a fight puts round it (FIGHTS):
a walker's suppressing fire at 20 m, your own feet at a run, and both.
It must clear the masking of the rest by CLEAR dB in its best band, by
the audit's masking model, at every shot's worst moment (the cue landing
with a shot).
"""
import audio_mix as am

# Cue -> what it tells you.
STATE = {
    # The slide locking back lands with your own last shot, so it can't be
    # heard over it: the press after is the tell (`gun.action_open`).
    "gun.bolt_catch": "slide or bolt locked back: empty, reload",
    "gun.action_open": "the trigger pressed with the action open (slide locked back)",
    "gun.dry_fire": "the hammer on an empty chamber",
    "gun.trigger_dead": "the trigger on a hammer already down",
    "gun.trigger_block": "the trigger blocked (safe, or a gate open)",
    "gun.safety": "the safety thrown",
}
# A fight, as scene layers besides the cue: (cue, start s, distance m or
# None, extra dB, every s).
SUPPRESS = 0.45  # robots::walker::SUPPRESS_EVERY's cadence
FIGHTS = {
    "walker suppressing at 20 m": [("robot.shot", 0.5, 20.0, 0.0, SUPPRESS)],
    "running": [("step", 0.5, None, 0.0, 0.33)],
    "both": [("robot.shot", 0.5, 20.0, 0.0, SUPPRESS), ("step", 0.6, None, 0.0, 0.33)],
}
CLEAR = 6.0     # dB over the masking: heard at once, not strained for
LISTEN = 0.15   # s of the cue judged: its click


def run(sets, out):
    mix = am.read_mix()
    names = {c["name"] for c in mix}
    lines = ["Your gun's state in a fight (each cue in the hand, landing with a shot; "
             f"wants {CLEAR:+.0f} dB over the masking):",
             f"    {'cue':18s} " + " ".join(f"{f:>26s}" for f in FIGHTS) + "  flags"]
    flags = 0
    for cue, what in STATE.items():
        if cue not in names or next(c for c in mix if c["name"] == cue)["set"] not in sets:
            lines.append(f"    {cue:18s} (not in the mix or the bank)")
            continue
        row, bad = [], []
        for fight, layers in FIGHTS.items():
            # The cue lands with the first shot (or footfall), its worst.
            scene = [("wind", "loop", None, 0.0, None)] + layers + [(cue, 0.5, None, 0.0, None)]
            total, parts = am.render(scene, sets, mix, seconds=1.5)
            sig = parts[f"{len(scene) - 1}:{cue}"]
            rest = total - sig
            # Through its own filter, as the game plays it (a dead trigger
            # is a dull click).
            if cue in am.CUE_LOWPASS:
                sig = am.lowpass(sig, am.CUE_LOWPASS[cue])
            m, band = am.margin(sig, rest, 0.5, LISTEN)
            row.append(f"{m:+6.1f} dB at {band:>5} Hz      ")
            if m < CLEAR:
                bad.append(f"{fight}: {m:+.1f}")
        flags += bool(bad)
        lines.append(f"    {cue:18s} " + " ".join(f"{r:>26s}" for r in row)
                     + ("  masked (" + "; ".join(bad) + ")" if bad else ""))
    return flags, lines
