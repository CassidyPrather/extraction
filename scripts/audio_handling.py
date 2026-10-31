"""The gun-handling pass of scripts/audio_audit.py: is every thing your
hands do to a gun heard where you do it?

`weapons::stoppages::tests::every_gun_action_is_heard_as_itself` proves
each action (a magazine in or out, the action worked, the safety, a dry
fire, a stoppage and its clear) has a sound of its own. This proves it's
heard: each, in the hand, under the loudest beds it's played under (BEDS:
a downpour, the loudest steady machine (a heavy at 8 m), a
walker's suppressing fire at 20 m), in its loudest LISTEN s. A tell (a
stoppage, a dry loose) must clear the masking of the rest by CLEAR dB in
its best band, as `audio_gunstate`'s state cues must; routine handling,
heard because you're doing it, is flagged under ROUTINE_FLAG dB and warned
under CLEAR. A fault heard as several layers is heard if one of them
clears. The gun-state cues are `audio_gunstate`'s (under its FIGHTS), not
judged again here. Every `gun.*` cue in the mix is an action here, one of
those, or named in NOT_HANDLING, so a new one can't miss the pass.
"""
import audio_gunstate as gs
import audio_mix as am

# Action -> its layers (`audio::handling_sound`, `audio::fault_layers`).
ACTIONS = {
    "magazine in": ["gun.mag_insert"],
    "magazine half-seated": ["gun.mag_half"],
    "magazine out": ["gun.mag_release"],
    "magazine shaken out": ["gun.mag_drop"],
    "slide or bolt drawn back": ["gun.slide_back"],
    "slide racked home": ["gun.slide_rack"],
    "charging handle (AK)": ["gun.charging"],
    "bolt opened": ["gun.bolt_open"],
    "bolt closed": ["gun.bolt_close"],
    "carrier home after a shot": ["gun.carrier_home"],
    "open bolt slamming forward": ["gun.open_bolt"],
    "round chambered": ["gun.chamber"],
    "live round worked out": ["gun.casing"],
    "shotgun opened": ["gun.shotgun_open"],
    "shotgun closed": ["gun.shotgun_close"],
    "pump back": ["gun.pump_back"],
    "pump forward": ["gun.pump_fwd"],
    "shell in": ["gun.shell_insert"],
    "clip stripped in": ["gun.stripper_clip"],
    "Nagant gate opened": ["gun.gate_open"],
    "Nagant gate shut": ["gun.gate_shut"],
    "Nagant cylinder turned": ["gun.cylinder"],
    "crossbow string latched": ["gun.latch"],
    "crossbow string let go short": ["gun.string_slap"],
    "crossbow loosed dry": ["gun.dry_loose"],
    "stovepipe": ["gun.malfunction", "gun.stovepipe"],
    "failure to feed": ["gun.ftf"],
    "double feed": ["gun.double_feed", "gun.grind"],
    "short stroke": ["gun.short_stroke"],
    "dud": ["gun.dud"],
    "hang-fire": ["gun.dry_fire", "gun.hang_fizz"],
    "bore rodded": ["gun.rod_scrape", "gun.rod_tink"],
    "bolt stuck": ["gun.bolt_stuck"],
    "bolt freed": ["gun.bolt_freed", "gun.bolt_open"],
    "slam-fire's snap": ["gun.slam_snap"],
}
# Tells: what went wrong, or what to do. The rest is routine handling.
TELLS = {
    "stovepipe", "failure to feed", "double feed", "short stroke", "dud",
    "hang-fire", "bolt stuck", "bolt freed", "slam-fire's snap",
    "crossbow loosed dry",
}
ROUTINE_FLAG = 3.0  # dB: routine handling under this is flagged, under CLEAR warned
# Gun cues that aren't the hands' doing, or are shots (judged by the mix
# pass against the world's levels).
NOT_HANDLING = {
    "gun.shot", "gun.shot_suppressed", "gun.distant", "gun.squib",
    "gun.bulged", "gun.bulged_ring", "gun.out_of_battery", "gun.burst_case",
    # A gun knocked on a wall as you move: not an action.
    "gun.bump",
}
# Beds as scene layers: (cue, start s or "loop", distance m or None, extra
# dB, every s).
WIND = [("wind", "loop", None, 0.0, None)]
FIGHT = "walker suppressing at 20 m"
BEDS = {
    # The audit's storm (`audio_mix`'s range check).
    "downpour": WIND + [("rain", "loop", None, 0.0, None)],
    # The loudest steady machine. (A minelayer's grind close by would be
    # the small one, but it's synthesised in the game, not a file.)
    "heavy idling at 8 m": WIND + [("heavy.idle", "loop", 8.0, 0.0, None)],
    FIGHT: WIND + gs.FIGHTS[FIGHT],
}
# Masked, and not a gain's to fix (the sample or its filter is wrong for
# it; with Audio, the mix's owners): reported, not failed. One that
# clears is flagged until it's taken off.
KNOWN = {
    "Nagant cylinder turned": "guns/dry_fire at x1.6 and -10 dB: a quiet ratchet by design, 6 dB short in rain",
}
CLEAR = gs.CLEAR
LISTEN = gs.LISTEN
AT = 0.5  # s: the action lands with the bed's first shot, its worst


def margin(layers, bed, sets, mix):
    """The best of an action's layers over the bed's masking (each layer
    heard through its own filter, as the game plays it)."""
    scene = bed + [(c, AT, None, 0.0, None) for c in layers]
    total, parts = am.render(scene, sets, mix, seconds=1.5)
    best = None
    for i, cue in enumerate(layers):
        sig = parts[f"{len(bed) + i}:{cue}"]
        rest = total - sig
        if cue in am.CUE_LOWPASS:
            sig = am.lowpass(sig, am.CUE_LOWPASS[cue])
        # Its best LISTEN s in its first second: the click is heard when
        # it comes, not at the cue's start (a fore-end's slide before it).
        m = max(am.margin(sig, rest, AT + k * 0.05, LISTEN) for k in range(18))
        if best is None or m[0] > best[0]:
            best = m
    return best


def run(sets, out):
    mix = am.read_mix()
    by_name = {c["name"]: c for c in mix}
    lines = ["Every gun action in the hand, under the loudest beds it's played under "
             f"(wants {CLEAR:+.0f} dB over the masking in its best band):",
             f"    {'action':28s} " + " ".join(f"{b:>27s}" for b in BEDS) + "  flags"]
    flags = 0
    named = {c for layers in ACTIONS.values() for c in layers} | set(gs.STATE)
    for c in sorted(n for n in by_name if n.startswith("gun.")):
        if c not in named and c not in NOT_HANDLING:
            flags += 1
            lines.append(f"    {c}: a gun cue not an action here, a state cue nor NOT_HANDLING")
    for action, layers in ACTIONS.items():
        missing = [c for c in layers if c not in by_name or by_name[c]["set"] not in sets]
        if missing:
            flags += 1
            lines.append(f"    {action:28s} {', '.join(missing)} not in the mix or the bank")
            continue
        row, bad, soft = [], [], []
        floor = CLEAR if action in TELLS else ROUTINE_FLAG
        for bed_name, bed in BEDS.items():
            m, band = margin(layers, bed, sets, mix)
            row.append(f"{m:+6.1f} dB at {band:>5} Hz       ")
            if m < floor:
                bad.append(f"{bed_name}: {m:+.1f}")
            elif m < CLEAR:
                soft.append(f"{bed_name}: {m:+.1f}")
        known = KNOWN.get(action)
        if known and bad:
            note = "  known (" + "; ".join(bad) + "): " + known
        elif known:
            flags += 1
            note = "  clears now: take it off KNOWN"
        else:
            flags += bool(bad)
            note = ("  masked (" + "; ".join(bad) + ")" if bad
                    else "  faint (" + "; ".join(soft) + ")" if soft else "")
        lines.append(f"    {action:28s} " + " ".join(f"{r:>27s}" for r in row) + note)
    return flags, lines


if __name__ == "__main__":
    # Alone, on the audit's cached asset measures (run the audit once).
    import json
    import sys
    out = am.ROOT / "target" / "audio_audit"
    sets = json.loads((out / "assets.json").read_text())
    n, text = run(sets, out)
    print("\n".join(text))
    sys.exit(1 if n else 0)
