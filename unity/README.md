# Unity (VRChat port) tooling

Groundwork for phase 2, the VRChat world. The plan and the platform's
limits are in [`docs/VRCHAT_PORT.md`](../docs/VRCHAT_PORT.md). This
folder holds the Unity side of the map pipeline. Nothing here has been run
in Unity yet, because there is no Unity licence in this environment.
`Editor/ExtractionImporter.cs` was type-checked against stubs of the
documented API, not against Unity itself.

## The workflow

```text
assets/maps/<map>.ron ──unity-export──▶ <out>/ (RAW, PNGs, JSON) ──ExtractionImporter──▶ Unity scene
assets/models/*.glb ──glTFast or UnityGLTF──▶ prefabs in the project ─────────┘ (by name)
```

### 1. Export a map (Rust, no Bevy needed)

```bash
cargo run --release -p extraction_core --bin unity-export -- assets/maps/marrow_creek.ron target/unity/marrow_creek
cargo run --release -p extraction_core --bin unity-export -- assets/maps/kettle_pass.ron  target/unity/kettle_pass
```

Each takes a couple of seconds and writes:

| File | What |
| - | - |
| `terrain.json` | Map size, the Terrain's position and size, the height range used to normalise the RAW, the file layouts and the coordinate conversion |
| `heightmap.r16` | 1025 × 1025 unsigned 16-bit little-endian, no header. Rows run south to north, columns west to east |
| `splat_<layer>.png` | 1024² greyscale weights for grass, forest, dirt, gravel, asphalt, rock and mud (they sum to 255). North is at the top of the image |
| `water_mask.png` | 1024², 255 under water |
| `trees.json` | Every tree the game places: species, variant, position, yaw, scale (Marrow Creek 3,472, Kettle Pass 1,942) |
| `placements.json` | Prefab id, tag, position on the ground, yaw, footprint (width, height, depth) |
| `markers.json` | Insertion, rendezvous, gate, loot, POIs and so on, with names and loot tables |
| `regions.json` | Danger regions (polygons, danger, loot richness) and the forest polygons |
| `roads.json` | Each road's built centreline (every 2 m, at its graded height), kind, width and bridge spans |
| `water.json` | Each stream's centreline at the game's water level |

Every position is already in **Unity's frame**. Fields prefixed `game_`
are in the game's frame.

### 2. Set up the Unity project

1. Install **Unity 2022.3.22f1** through the VRChat Creator Companion and
   create a **Worlds** project. Use that exact version: VRChat rejects
   content built with other versions.
2. Copy `unity/Editor/ExtractionImporter.cs` into `Assets/Editor/` (any
   folder named `Editor` works).
3. Optionally, put real assets where the importer looks for them. Anything
   missing gets a placeholder.
   - `Assets/Extraction/TerrainLayers/<layer>.terrainlayer` for `grass`,
     `forest`, `dirt`, `gravel`, `asphalt`, `rock` and `mud`. The CC0
     textures `scripts/fetch_assets.py` downloads make good layers.
   - `Assets/Extraction/Trees/<species>.prefab` for `spruce`, `pine`,
     `birch` and `snag`. A tree prefab needs a MeshRenderer or an LODGroup
     on its root.
   - `Assets/Extraction/Prefabs/<prefab id>.prefab` for each building,
     named as in the map (`barn`, `house_plank`, `pump_station` and so
     on). Without one, the importer places a grey box of the right
     footprint with the id above it.

### 3. Import

**Extraction > Import Map Export...**, pick the export folder (it can be
outside the project) and press **Import**. It builds the scene:
- a Terrain with its height and splat layers,
- the trees as terrain tree instances,
- buildings or blockouts,
- empty marker objects,
- a water ribbon per stream,
- grey exponential fog.

It saves the scene and generated assets in `Assets/Extraction/Generated/`.
Re-importing replaces the generated assets.

## Coordinates

The game is right-handed, with **+z south**. Unity is left-handed, and the
export makes **+z north**. The conversion is one axis flip:

```text
unity = (game.x, game.y, -game.z)
```

- A map placement's `rot` (degrees clockwise from north) is already
  Unity's Y Euler angle.
- A yaw in the game's radians (counter-clockwise, as in Bevy) becomes
  `-yaw`.
- The Terrain sits at `(-size/2, min_height, -size/2)`, so Unity heights
  equal game heights.

`extraction_core::unity` holds these conversions, and its tests pin them
down.

**Models from glTF need half a turn.** glTFast negates **X** when it
converts glTF's right-handed frame
([source](https://github.com/atteneder/glTFast/blob/main/Runtime/Scripts/Jobs.cs)),
and UnityGLTF does the same
([issue](https://github.com/KhronosGroup/UnityGLTF/issues/257)). The
export flips **Z**. The two differ by 180° about Y, so a model of this
game's imported from glTF needs `yaw + 180` to face as it does in the
game. That is the importer's **Model yaw offset** (default 180). Set it to
0 for prefabs built natively in Unity.

## Where the glTF models come in

`assets/models/` holds the guns, magazines, items, robots, truck and stash
crate as `.glb`, all built by the Blender scripts in `art/`. Import them
with either package:
- **glTFast** (`com.unity.cloud.gltfast`) is fast and imports `.glb` files
  dropped into `Assets/` as assets.
- **UnityGLTF** (`org.khronos.unitygltf`) imports and exports both ways,
  which is handy when changes need to go back to Blender.

In either case, the import is **editor-time only**. VRChat strips scripts
it does not allow from uploaded worlds, so turn each imported model into a
plain prefab (MeshFilter, MeshRenderer, materials). Then add the VRChat
pieces by hand: VRC Pickup, Rigidbody, colliders, UdonSharp behaviours.
Move the materials to Standard on PC, or to a mobile shader for Quest.

The **buildings** are not glTF. The game generates them in code
(`src/world/buildings.rs`), so until a glTF dump of those meshes exists,
or the buildings are remodelled in `art/`, they import as blockouts. The
**trees** are procedural in the game too, so the tree prefabs are new
work.

## Notes for Quest

- Seven terrain layers is too many on Android. Merge forest into grass,
  mud into dirt and asphalt into gravel (`terrain.json` has a
  `quest_hint`).
- Use mesh trees with LODs, not Unity's billboards.
- Keep the distance fog dense, and cut the camera's far plane just behind
  it.
- Android worlds have a hard 100 MB limit and a budget of about 250k
  triangles.
