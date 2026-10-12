# Porting to VRChat

> **Status: parked.** Written early, while the groundwork was cheap to lay.
> The port is weeks away: the game stays in Bevy until the single-player
> design has settled through real playtesting. When it's time, this file is
> the handoff for whoever works on a machine with Unity and a GPU: read it,
> run `unity-export` on each map, and start with the prototypes listed under
> the top risks (pickups and manual gun handling first).

Phase 2 of the plan: the game as a VRChat world (Unity + VRChat SDK3
Worlds + UdonSharp). This file is the plan. It records what the platform
allows today, maps each system in this game to its VRChat equivalent, and
says plainly what survives, what has to be redesigned and what may not be
feasible. Facts were checked in October 2026, with links inline. Recheck
the numbers before relying on them: VRChat changes them often.

The tooling that already exists is in [`unity/README.md`](../unity/README.md):
`unity-export` writes a map out for Unity and an editor script builds the
scene from it.

## The platform, in numbers

**Unity version.** VRChat content must be built with **Unity 2022.3.22f1**
exactly ([current Unity version](https://creators.vrchat.com/sdk/upgrade/current-unity-version/)).
A Unity 6 client (6000.0.60f1) is in open beta, but uploads from Unity 6
are rejected. It keeps the built-in render pipeline, and moving to URP is
"not yet" scheduled ([developer update, 25 June 2026](https://ask.vrchat.com/t/developer-update-25-june-2026/48618)).
Target 2022.3.22f1 and the **built-in pipeline**.

**Scripting.** Use UdonSharp, a C# subset compiled to the Udon VM. Udon
"can take on the order of **200x to 1000x** longer" than ordinary C#, and
"even about 40 loop iterations" in `Update` can show up in the frame rate
([UdonSharp performance tips](https://udonsharp.docs.vrchat.com/random-tips-&-performance-pointers)).
The language has no `List<T>`, generic classes, interfaces or method
overloads. It has arrays and the DataList and DataDictionary containers
([UdonSharp](https://creators.vrchat.com/worlds/udon/udonsharp/)). Methods
that mutate a struct (for example `Vector3.Normalize()`) do not change the
original. Soba, the planned faster successor, is still unreleased
([feedback board](https://feedback.vrchat.com/udon/p/soba)), so plan for
today's Udon. JSON can be parsed at runtime into DataDictionary and
DataList ([VRCJson](https://creators.vrchat.com/worlds/udon/data-containers/vrcjson)).
That gives a route for the item and loot tables.

**Networking** ([network specs](https://creators.vrchat.com/worlds/udon/networking/network-details),
[events](https://creators.vrchat.com/worlds/udon/networking/events),
[ownership](https://creators.vrchat.com/worlds/udon/networking/ownership)):
- Each player can send about **11 KB/s** of Udon data. VRChat's own figure
  for network events is 8–10 KB/s in practice.
- Continuous sync carries about **200 bytes** per serialisation. Manual sync
  carries about 280 KB, but it is rate-limited by size.
- A network event takes at most 8 parameters (16 KB in total). Each event
  fires at most 5 times a second by default, configurable from 1 to 100.
  The whole world gets about 100 events a second.
- Only an object's **owner** can write its synced variables. The instance
  master owns any object that has never been claimed. When the owner
  leaves, VRChat reassigns ownership, and nobody can predict who the next
  master will be.
- VRC Object Sync gets expensive at scale. Community reports describe
  frame-time trouble at around 200 synced objects, and one world that
  synced well only after it cut 150 Object Syncs to 30
  ([feedback](https://feedback.vrchat.com/sdk-bug-reports/p/vrc-object-sync-major-performance-issues-with-a-real-solution)).

**Persistence.** A world gets **100 KB of PlayerData and 100 KB of
PlayerObject data per player**, stored compressed. Both are per player.
There is **no world-global persistence**, and players can wipe their own
data ([persistence](https://creators.vrchat.com/worlds/udon/persistence/)).

**Pickups.** A [VRC Pickup](https://creators.vrchat.com/worlds/components/vrc_pickup)
needs a Rigidbody and a Collider. Its options:
- **Orientation** is Any, Gun or Grip, with Exact Gun and Exact Grip
  transforms for the held pose.
- **Auto Hold** keeps the item in the hand after the grab.
- **Use** is the trigger while the item is held.

Remote players do not see a pickup move unless it has VRC Object Sync.
Each pickup is held by one hand, and there is no built-in two-handed hold.

**Players.** Udon can read the head and hand tracking data. It can set
walk, run and strafe speed, jump impulse and gravity, push the player with
`SetVelocity`, and teleport them
([player forces](https://creators.vrchat.com/worlds/udon/players/player-forces)).
Avatar height is player-controlled by default, from 0.2 to 5 m eye height.
A world can take control or set its own min and max, and scaling never
changes the player's collision
([avatar scaling](https://creators.vrchat.com/worlds/udon/players/player-avatar-scaling)).
Input arrives as `InputUse`, `InputGrab` and `InputJump`, with the hand
that pressed it ([input events](https://creators.vrchat.com/worlds/udon/input-events)).
NavMeshAgent works, with the default agent type only
([AI navigation](https://creators.vrchat.com/worlds/udon/ai-navigation)).
User layers 22–31 survive upload. Use numeric layer masks, not names
([layers](https://creators.vrchat.com/worlds/layers)).

**Size and performance.**
- **PC.** "Try to keep your worlds under 200MB." A world going public
  should hold 45 FPS for a single VR user at spawn, and every shader must
  support single-pass stereo
  ([publishing tips](https://creators.vrchat.com/worlds/submitting-a-world-to-be-made-public/)).
  The hard PC cap is not documented. Creators report about 1.2 GB
  ([feedback](https://feedback.vrchat.com/feature-requests/p/increase-world-upload-limit-for-pc)).
- **Android and Quest** ([Android optimisation](https://creators.vrchat.com/platforms/android/quest-content-optimization)):
  - **100 MB** after compression is a hard limit.
  - The whole world gets about **250k triangles**.
  - Post-processing is off.
  - Baked lighting is "essential".
  - Textures should be at most 1k.
  - Avoid transparency, because alpha fill rate is the main cost.
- **Unity trees on Quest.** Built-in billboard trees have caused severe
  frame drops on Quest
  ([report](https://feedback.vrchat.com/bug-reports/p/1121-unity-2019-trees-causing-extreme-fps-drops-on-quest-2)).

## What this means overall

VRChat owns the body, the hands, locomotion and the network. This game's
deepest seam, the **rig** (a `RigInput` that a headset, the keyboard or a
bot writes), has no equivalent there. The player is VRChat's, and Udon can
only read it and nudge it. Everything built on "physical hands" has to be
rebuilt on pickups and hand tracking.

Udon is slow, and every piece of shared state needs an owner. The
simulation therefore has to shrink. Use fewer robots, think less often,
lean on native Unity components (NavMeshAgent, Animator, ParticleSystem,
physics) instead of Udon loops, and sync compact state rather than
transforms.

A VRChat world is multiplayer whether you want it or not: a private
instance is "single-player", but the instance master still owns the world
state. **Phase 2 cannot ignore phase 3.** Design every synced system for
co-op from the start, even if public instances come later.

The good news is that the rules are already engine-free.
`extraction_core` holds the maps, terrain, forest, items, inventory
arithmetic, firearm state machines, loot, anomaly layouts, recipes and
cooking graph. All of it is plain data and small state machines that port
to UdonSharp nearly line for line (arrays instead of `Vec`, ints instead
of enums with payloads), or that can be baked to JSON or assets by tools
like `unity-export`. Bevy stays the place where the game is designed and
simulated. The VRChat build is a re-implementation of the client against
the same rules.

## System by system

Verdicts: **intact** (ports with little change), **redesign** (the idea
survives, the mechanism changes) and **at risk** (may not be feasible at
this game's quality; prototype before committing).

### Rig and hands: redesign

Today a `RigInput` drives the hands, and held items follow on Avian
joints. Weight changes how quickly the hands settle, and flat mode and
bots play scripted hand macros.

In VRChat the avatar, IK and locomotion are VRChat's. Held things are
VRC Pickups snapped to the hand. Udon reads hand poses (`GetTrackingData`)
and grip and trigger input per hand, so "reach for the slide with your
free hand" becomes: when the off hand grips near a handle, Udon follows
that hand along the mechanism's axis. Encumbrance survives as walk, run
and jump settings, but hand sway from weight does not.

Desktop players have no second hand, so they need the equivalent of this
game's flat-mode macros: rack, reload and swap bound to keys. Bots do not
port, and the simulations stay in Bevy.

Lock avatar scale to world control, or clamp it (say 1.5–1.9 m eye
height). Otherwise a 0.3 m avatar walks under robot sightlines and a 5 m
one reaches over walls.

### Guns with manual mechanisms: at risk

This is the pillar most likely to suffer, so prototype it first.
- **The state machine is intact.** The magazine, chamber, slide or bolt,
  safety, durability and malfunctions in `extraction_core::firearm` port
  as plain UdonSharp.
- **Guns and magazines are pickups.** The gun uses Orientation Gun with an
  Exact Gun pose. A magazine seats when it enters a trigger volume at the
  magwell. That much is routine in VRChat worlds.
- **Slides, bolts and charging handles have no pickup to hang on.**
  Nested pickups on a held pickup misbehave. Use the off-hand approach
  above: on `InputGrab` near the handle, project the hand's position onto
  the handle's travel each frame, and run the state machine on the
  travel. Ten lines in Bevy become fiddly here.
- **The two-handed hold is the open question.** VRChat attaches the
  pickup to one hand. Turning a held gun to point from grip to off hand
  means overriding a held pickup's transform after VRChat sets it.
  Creators do this, but it is not a documented feature. If it fails,
  rifles become one-handed, with a "brace" bonus to stability when the
  off hand is near the handguard.
- **Ballistics are fine.** A few raycasts per shot, stepped over frames
  for drop, cost little in Udon. Udon cannot map a raycast hit to a
  player directly
  ([forum](https://ask.vrchat.com/t/how-to-get-playerapi-using-raycast-or-similar-to-raycast/41313)),
  which matters only for PvP.
- **Sync.** The shooter owns the gun. Shots go out as a network event
  (origin, direction, seed), and every client draws its own tracers.
  Damage to robots goes to the robot's owner.

### Inventory, backpack and weight: redesign

The arithmetic in `extraction_core::inventory` (slots, size classes, bulk,
weight) is intact. The body is not:
- **Chest rig and belt sockets** become trigger volumes that Udon moves
  every frame to the player's head or chest. Bone positions vary by
  avatar, so anchor to head tracking, as `body.rs` already does with its
  lazy torso.
- **The backpack** becomes a pickup-able object that follows the player.
  Dropping an item into its volume disables the item and records it as an
  id in an array. That part is easy.
- **Weight** drives `SetWalkSpeed` and `SetRunSpeed`.

The real problem is **sync volume**. Every loose item that others can see
move needs Object Sync, and that degrades at a few hundred. Rules:
- Items in a backpack or pocket are not world objects. They are bytes in
  the owner's synced inventory.
- Loot spawns from pools only when a container is opened.
- Dropped items sleep, so that nobody pays for their sync when they are
  not moving.

### Robots, AI and physics: at risk at today's scale

There are five robot types. Their physics, leg IK, part damage,
perception and the director run every frame in Rust. In Udon, at 200–1000x
the cost, that budget is gone.
- **The director** runs on one owner (the master), time-sliced: a job
  posted every few seconds rather than evaluated every frame.
- **Movement** uses NavMeshAgent for walkers, grapplers and the collector,
  baked from the exported terrain. Drones and the dirigible are simple
  steering, or animation along authored paths.
- **Leg IK** gives way to baked walk cycles on an Animator driven by
  speed. Part damage survives as separate colliders per part, with health
  in ints.
- **Perception** casts a few rays per robot per second, spread across
  frames.
- **Sync.** The owner syncs a compact state (position, heading, state and
  target as a few bytes) at a low rate, and every client interpolates. Do
  not put Object Sync on robot rigidbodies.
- **The budget is unknown until measured.** Build one walker and one
  drone, then measure frame time with 5, 10 and 20 active on PC and Quest
  before designing encounters. Expect a cap well below the Bevy game's.
- **Grappler tethers that drag the player** are feasible with
  `SetVelocity` on the victim's client.
- **The bot brains, the scenario tests and the map sims don't port.**
  They are the design tool, and they stay in Bevy.

### Anomalies and artifacts: intact, with care

The layouts (`extraction_core::anomaly`) are a pure function of map, seed
and cycle. They either port to UdonSharp or get baked by the exporter as
a table of layouts per cycle. The effects:
- **Burner and snare** are trigger zones with a particle glow.
- **Lift** sets low gravity and an upward impulse on the player
  (`SetGravityStrength`, `SetVelocity`). Floating loose objects are owned
  rigidbodies, which is fine with a few at a time.
- **Artifacts** are pickups whose effect applies while held or in your
  inventory.
- **Attention** is a counter on the director's owner, fed by events from
  each player.

### Gadgets: mostly intact, two at risk

- **Intact:** the compass, the map case (a texture), EMP grenades and
  traps, the cloak, deployable cover and the lead-lined case.
- **Climbing kit and zipline** are known VRChat patterns: Udon moves the
  player with `SetVelocity` or teleports along a cable or rope. Feasible.
- **Lights** are realtime spot lights. A few work on PC. On Quest even one
  is costly, so use emissive cones and a light cookie instead.
- **The scope (picture-in-picture)** and **the camera drone's feed** each
  need a camera rendering to a texture. That is acceptable on PC at low
  resolution and **at risk on Quest**. Udon has no documented way to zoom
  the player's own camera, so a reticle-only "scope" with no
  magnification may be all Quest gets.
- **Robots seen through walls** (the glass eye, drone spotting) needs a
  custom x-ray shader, ported from `xray.wgsl`. That is feasible.

### The truck base: intact, re-staged

- **The truck** is a scene object. Driving to a rendezvous is an animation
  along the perimeter road's polyline, which `roads.json` already exports.
- **Riding** uses a [VRC Station](https://udonsharp.docs.vrchat.com/using-vrc-stations)
  with "Immobilize for Vehicle".
- **The bench, the camp kitchen and the cooking graph** port as data and
  state machines.
- **The stash shelves** are sockets. Contents load from the owner's
  PlayerData and sync to others as bytes.
- **In co-op,** each player sees their own stash in a personal locker, or
  the truck shows the owner's stash to everyone. Decide that early.

### Persistence and the Surge: redesign

Today the whole Zone persists between sessions: looted containers, hidden
caches, remains, the Zone clock. VRChat has no world-global storage, and
an instance's state dies when its last player leaves.
- **The stash, your kit and the memorial board persist per player** in
  PlayerData. Use a compact binary or base64 string of item ids,
  conditions and rounds, not this game's RON save. 100 KB compressed holds
  hundreds of items easily.
- **The Zone lives as long as its instance.** That is a natural fit for
  the Surge: an instance is a Surge cycle. The cycle's weather and anomaly
  layout come from the instance's seed, and leaving the instance means the
  Zone has reset.
- **Personal Zone state:** your remains and a hidden cache or two can be
  stored in *your* PlayerData as "items at (x, z)" and re-spawned when you
  join an instance. That is feasible for a few small caches, but they are
  not shared.
- **The Zone clock** can be per player (time spent in the Zone) or per
  instance (time since the instance opened). Per instance is simpler and
  co-op-friendly.

### The memorial convoy: intact

The convoy is a NavMesh collector that drives out through the Gate with a
body container. In co-op it becomes better, because a friend can
intercept it. Whatever it delivers is deleted from the dead player's
PlayerData.

### Audio: redesign for the synthesised half

- **Recorded sounds port directly:** footsteps per surface, impacts, gun
  mechanisms and ambience beds. Use AudioSources with VRC Spatial Audio
  Source.
- **Footsteps** have to be played by Udon for the local player, from
  velocity and the surface underneath. The exported splat maps can be
  sampled to find the surface.
- **Synthesised sounds have no clear path.** Today `synth.rs` makes the
  anomalies and robot voices. Realtime DSP from Udon is not documented
  (`OnAudioFilterRead` reportedly does nothing:
  [feedback](https://feedback.vrchat.com/udon/p/issue-on-implementing-custom-dsp-algorithms-with-onaudiofilterread)).
  Pre-render variation sets to clips with the existing synth and treat
  them as recordings.

### Fog, rain and shaders: redesign; volumetrics at risk

All six WGSL shaders (sky, terrain, water, rain, near-fade, x-ray) have to
be rewritten as built-in-pipeline ShaderLab/HLSL with single-pass
instanced stereo.
- **Distance fog** is Unity's built-in exponential fog. It is cheap,
  works on Quest, and is this game's look. It is also the performance
  ally: a short far-clip behind the fog culls most of the forest.
- **Height fog** must go into every material's shader. PC can use a
  post-process volume. Quest has no post-processing.
- **Rain** becomes a ParticleSystem near the camera. Quest's alpha
  fill-rate rule means sparse, thin streaks.
- **Volumetric light shafts** are PC only, if at all.
- **Weather per cycle** (`world/weather.rs`) is a pure function: port it,
  or bake it into the export.

### Maps: intact (this is the part already started)

`unity-export` writes the terrain (a 1025² RAW), splat maps, every tree
(the same scatter the game uses, now in `extraction_core::forest`), and
the placements, markers, regions, roads and water in Unity's coordinates.
`ExtractionImporter.cs` builds a scene from them. What remains:
- **Buildings are generated in Bevy code** (`src/world/buildings.rs`), not
  glTF, so Unity has nothing to import. The cheapest path is a small tool
  in the Bevy crate that writes each building to glTF, with loot spots and
  doors as named nodes. The alternative is remodelling them in Blender
  next to the other models in `art/`.
- **Trees are procedural meshes too.** Make a few low-poly mesh trees per
  species with LODGroups, not Unity billboards. Marrow Creek has 3,472
  trees and Kettle Pass 1,942. On Quest's ~250k-triangle budget that
  means fog, a short draw distance and aggressive LODs.
- **Terrain layers:** the export paints seven. On Quest merge them to
  four (the export's `quest_hint` says how).
- **Lighting:** bake lighting and occlusion. The map is 640 m square,
  which is fine for Unity terrain and for floating-point precision.

## Summary

| Survives intact | Needs redesign | At risk |
| - | - | - |
| Map layout, terrain, roads, water, forest scatter | The rig and hands (VRChat owns them) | Two-handed gun handling and manual actions at this game's feel |
| Items, loot tables, recipes, cooking graph | Backpack and body sockets | Robot count and behaviour at today's scale (Udon CPU) |
| Firearm and inventory state machines | Persistence: the Zone becomes per instance; the stash moves to PlayerData | Scope picture-in-picture and drone feed on Quest |
| Anomaly layouts, weather per cycle, offerings | Shaders: fog, rain, sky, water, x-ray | Volumetric light on any platform; heavy fog layers on Quest |
| Memorial convoy, truck, bench, kitchen | Synthesised audio becomes pre-rendered clips | Hundreds of synced loose items |
| | Desktop players need key macros | Shared, persistent Zone state across sessions (no platform support) |

## Suggested order

1. **Walk the map.** Run `unity-export` and the importer, use blockout
   buildings, and test with Build & Test on PC and a Quest. Measure frame
   time with the forest and fog. (This is the first check of the
   performance budget.)
2. **One gun.** The PM pistol: magazine, slide and the off-hand mechanism
   in UdonSharp, plus the firearm state machine. Judge the feel before
   anything else.
3. **The body.** Backpack, pockets and weight, with items as bytes.
4. **One robot type, then two,** NavMesh-driven and master-owned. Measure
   Udon time at 5, 10 and 20 active.
5. **Persistence.** The stash in PlayerData, and the instance-scoped
   Surge.
6. **Then content:** the remaining guns, gadgets and robots, and the
   second map.

Each step gives a yes or no on one risk above. If step 2 or 4 fails,
rethink the port before building more.
