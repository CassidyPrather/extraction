# World models

Notes for the buildings, furniture and props (World models). The rule they
follow: what looks usable is usable. A handle that opens nothing, or a lever
that moves nothing, is a lie (`capture::checks::every_handle_is_in_reach`,
`world::hinge::every_container_opens_clear`).

## What opens

- Lockers (`locker#door`), counter cabinets (`counter_cabinet#door_l/_r`),
  counter drawers (`counter_drawers#drawer_*`), stoves
  (`stove#door_fire/_ash/_oven`): moving parts in the hand (`world::hinge`).
- Tool chests (`tool_chest#lid`): the lid lifts on its back hinge, by the
  bar along its front (`hinge::Frame::Lid`). Its hasp hangs open, no lock.
- Wheelie bins (`wheelie_bin#lid`): the lid lifts back over its handle on
  its rail, by the lip under its free edge (`hinge::Frame::SideLid`). They're
  empty.
- Coal bunkers (`coal_bunker#hatch`, a yard thing): the hatch at the foot of
  the front lifts out on its top edge, by the bar at its foot; coal behind
  it. A yard's things' moving parts are fitted as a building's furniture's
  are (`buildings::yard_models`, `hinge::fit_counters`).
- A locker's chest-height shelf and each counter's top drawer hold one real
  find in a socket (`buildings::Hold`, `items::loot::roll_holds`). Stoves hold
  nothing.
- A tool chest's tray, across its back half, holds one find from `tools`
  (a crowbar, the bolt cutters: what pays off the chains below). The loot
  spot it always had (its placement's table) lies on its floor, rolled as
  it was on its lid (`LootSpot::keyed`).
- Tables and workbench pedestals have plain fronts: no drawer, no pull.

## What doesn't, and shows it

Rust and shearing: nothing on them suggests a tool would help.

- `brake_drum`: its band rusted fast to the drum, the lever's pivot seized.
- `control_stand`: both levers snapped off above their quadrants.
- `filter_unit`: the crank handle sheared, its boss rusted solid.
- `hand_pump`: its handle rusted fast where it last came down, its pivot
  seized in a blister of rust.
- `rain_barrel`: its tap's key and spigot rusted fast in a blister of rust.
- `lever_frame`: the signal box's levers rusted in their quadrants, each
  clamped by its lever collar (the signalman's lock), rusted on.
- `mill_ore_bin`: its chute gate's lever rusted fast at its pivot, its knob
  gone.
- `wall_telephone`: its crank broken off at the arm, a stub on its
  spindle.
- `logbook`: the Morse key beside it snapped off at its pivot.

## Chain/padlock: future bolt-cutter targets

A padlocked chain promises the bolt cutters. These are kept for when the
cutters cut them; until then they can't be opened. What each would open:

| Prop | Locked | Cut, it would |
| - | - | - |
| `gate_valve` | its handwheel chained to the yoke, a padlock and a red lockout tag | free the wheel: turn the valve to open or shut its flow (a drain, a race, a cold-blast line) |
| `switch_cabinet` | its door padlocked on a hasp (its breaker's lever is outside it) | open the cabinet: its fuses, and whatever was left in it |
| `winch` | its drum chained to its sole and padlocked (its crank handles are sheared stubs) | free the drum: let its rope run out, lowering what hangs on it |
| `warning_door` | the booth's door padlocked on its hasp, under its warning | open the booth: whatever's kept behind its warning |
| `warning_gate` | its latch padlocked on its post | open the gate into what it fences off |

The chain and padlock are `art/props/_kit.py`'s `chain()` and `padlock()`.
