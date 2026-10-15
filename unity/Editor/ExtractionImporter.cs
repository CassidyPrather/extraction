// ExtractionImporter.cs: builds a Unity scene from a `unity-export` folder.
//
// UNTESTED. This was written against the documented Unity 2022.3 editor API
// without a Unity install to compile or run it in. Expect to fix small
// things the first time it is opened in the editor. See unity/README.md.
//
// Put this file in any `Editor` folder of the Unity project, then use the
// menu: Extraction > Import Map Export...
//
// What it builds, under one root object:
// - A Terrain from heightmap.r16 and terrain.json, painted with one terrain
//   layer per splat_*.png. A TerrainLayer asset named after the layer
//   (`grass.terrainlayer`, ...) in the layer folder is used if it exists;
//   otherwise a flat-colour placeholder layer is made.
// - Tree instances from trees.json. Each species ("spruce", "pine",
//   "birch", "snag") uses the prefab of that name in the tree folder if
//   there is one, otherwise a generated placeholder tree.
// - Rocks from rocks.json: `boulder_<variant>.prefab` from the prefab
//   folder if there is one (about unit radius, flat base at y -0.55),
//   otherwise a squashed sphere. The big ones keep a collider.
// - Placements from placements.json. Each uses the prefab named after its
//   id (`barn.prefab`, ...) in the prefab folder if there is one,
//   otherwise a labelled cube of the prefab's footprint.
// - Empty marker objects from markers.json, and a flat water ribbon per
//   stream from water.json.
//
// Every position in the export is already in Unity's frame (x east, y up,
// z north). Unity world heights equal the game's heights.

using System;
using System.IO;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.SceneManagement;

public class ExtractionImporter : EditorWindow
{
    // Settings, kept between sessions in EditorPrefs.
    string exportFolder = "";
    string prefabFolder = "Assets/Extraction/Prefabs";
    string treeFolder = "Assets/Extraction/Trees";
    string layerFolder = "Assets/Extraction/TerrainLayers";
    string outputFolder = "Assets/Extraction/Generated";
    // Models brought in through glTFast or UnityGLTF are mirrored on X, not
    // Z, so they need half a turn to face the way they do in the game.
    // Use 0 for prefabs built natively in Unity facing +Z at map rot 0.
    float modelYawOffset = 180f;
    bool newScene = true;
    bool importTrees = true;
    bool importPlacements = true;
    bool importRocks = true;
    bool importMarkers = true;
    bool importWater = true;
    bool addFog = true;

    const string PrefsKey = "ExtractionImporter.";

    [MenuItem("Extraction/Import Map Export...")]
    static void Open()
    {
        GetWindow<ExtractionImporter>("Extraction Import");
    }

    void OnEnable()
    {
        exportFolder = EditorPrefs.GetString(PrefsKey + "export", exportFolder);
        prefabFolder = EditorPrefs.GetString(PrefsKey + "prefabs", prefabFolder);
        treeFolder = EditorPrefs.GetString(PrefsKey + "trees", treeFolder);
        layerFolder = EditorPrefs.GetString(PrefsKey + "layers", layerFolder);
        outputFolder = EditorPrefs.GetString(PrefsKey + "output", outputFolder);
        modelYawOffset = EditorPrefs.GetFloat(PrefsKey + "yawOffset", modelYawOffset);
    }

    void SavePrefs()
    {
        EditorPrefs.SetString(PrefsKey + "export", exportFolder);
        EditorPrefs.SetString(PrefsKey + "prefabs", prefabFolder);
        EditorPrefs.SetString(PrefsKey + "trees", treeFolder);
        EditorPrefs.SetString(PrefsKey + "layers", layerFolder);
        EditorPrefs.SetString(PrefsKey + "output", outputFolder);
        EditorPrefs.SetFloat(PrefsKey + "yawOffset", modelYawOffset);
    }

    void OnGUI()
    {
        EditorGUILayout.LabelField("Export folder (written by unity-export)", EditorStyles.boldLabel);
        EditorGUILayout.BeginHorizontal();
        exportFolder = EditorGUILayout.TextField(exportFolder);
        if (GUILayout.Button("Browse", GUILayout.Width(70)))
        {
            string picked = EditorUtility.OpenFolderPanel("Export folder", exportFolder, "");
            if (!string.IsNullOrEmpty(picked)) exportFolder = picked;
        }
        EditorGUILayout.EndHorizontal();

        EditorGUILayout.Space();
        EditorGUILayout.LabelField("Project folders", EditorStyles.boldLabel);
        prefabFolder = EditorGUILayout.TextField("Building prefabs", prefabFolder);
        treeFolder = EditorGUILayout.TextField("Tree prefabs", treeFolder);
        layerFolder = EditorGUILayout.TextField("Terrain layers", layerFolder);
        outputFolder = EditorGUILayout.TextField("Generated assets", outputFolder);
        modelYawOffset = EditorGUILayout.FloatField("Model yaw offset", modelYawOffset);

        EditorGUILayout.Space();
        newScene = EditorGUILayout.Toggle("Build in a new scene", newScene);
        importTrees = EditorGUILayout.Toggle("Trees", importTrees);
        importPlacements = EditorGUILayout.Toggle("Placements", importPlacements);
        importRocks = EditorGUILayout.Toggle("Rocks", importRocks);
        importMarkers = EditorGUILayout.Toggle("Markers", importMarkers);
        importWater = EditorGUILayout.Toggle("Water", importWater);
        addFog = EditorGUILayout.Toggle("Distance fog", addFog);

        EditorGUILayout.Space();
        if (GUILayout.Button("Import", GUILayout.Height(30)))
        {
            SavePrefs();
            try
            {
                Import();
            }
            catch (Exception e)
            {
                Debug.LogException(e);
                EditorUtility.DisplayDialog("Extraction import failed", e.Message, "OK");
            }
            finally
            {
                EditorUtility.ClearProgressBar();
            }
        }
    }

    // ---- JSON shapes (only the fields the importer needs; JsonUtility
    // ignores the rest, and it can't read nested arrays, so none are used).

    [Serializable] public class TerrainJson
    {
        public string map;
        public float size_m;
        public TerrainBox terrain;
        public HeightmapInfo heightmap;
        public AlphamapInfo alphamap;
    }
    [Serializable] public class TerrainBox { public float[] position; public float[] size; }
    [Serializable] public class HeightmapInfo
    {
        public string file;
        public int resolution;
        public float min_height_m;
        public float max_height_m;
    }
    [Serializable] public class AlphamapInfo { public int resolution; public LayerInfo[] layers; }
    [Serializable] public class LayerInfo { public int index; public string name; public string file; }

    [Serializable] public class TreesJson { public int count; public TreeJson[] trees; }
    [Serializable] public class TreeJson
    {
        public string species;
        public int variant;
        public float[] pos;
        public float yaw_deg;
        public float scale;
    }

    [Serializable] public class RocksJson { public int count; public float solid_radius_m; public RockJson[] rocks; }
    [Serializable] public class RockJson
    {
        public int variant;
        public float[] pos;
        public float yaw_deg;
        public float[] scale;
    }

    [Serializable] public class PlacementsJson { public PlacementJson[] placements; }
    [Serializable] public class PlacementJson
    {
        public string prefab;
        public string tag;
        public float[] pos;
        public float yaw_deg;
        public float[] footprint;
    }

    [Serializable] public class MarkersJson { public MarkerJson[] markers; }
    [Serializable] public class MarkerJson
    {
        public string kind;
        public string loot_table;
        public string name;
        public float[] pos;
        public float yaw_deg;
    }

    [Serializable] public class WaterJson { public WaterBody[] water; }
    [Serializable] public class WaterBody { public string name; public float width_m; public float[] centreline_flat; }

    static T ReadJson<T>(string folder, string file)
    {
        string path = Path.Combine(folder, file);
        if (!File.Exists(path)) throw new FileNotFoundException("Missing " + path);
        return JsonUtility.FromJson<T>(File.ReadAllText(path));
    }

    static Vector3 V3(float[] a)
    {
        return new Vector3(a[0], a[1], a[2]);
    }

    // ---- The import.

    void Import()
    {
        if (!Directory.Exists(exportFolder))
            throw new DirectoryNotFoundException("Export folder not found: " + exportFolder);
        TerrainJson tj = ReadJson<TerrainJson>(exportFolder, "terrain.json");
        string mapId = Safe(tj.map);

        if (newScene)
        {
            if (!EditorSceneManager.SaveCurrentModifiedScenesIfUserWantsTo()) return;
            EditorSceneManager.NewScene(NewSceneSetup.DefaultGameObjects, NewSceneMode.Single);
        }
        EnsureFolder(outputFolder);

        var root = new GameObject("Extraction - " + tj.map);

        EditorUtility.DisplayProgressBar("Extraction import", "Terrain heights", 0.1f);
        TerrainData td = BuildTerrainData(tj);
        EditorUtility.DisplayProgressBar("Extraction import", "Terrain layers", 0.3f);
        PaintSplats(tj, td);

        if (importTrees)
        {
            EditorUtility.DisplayProgressBar("Extraction import", "Trees", 0.5f);
            PlantTrees(tj, td);
        }

        string tdPath = outputFolder + "/" + mapId + "_terrain.asset";
        AssetDatabase.DeleteAsset(tdPath);
        AssetDatabase.CreateAsset(td, tdPath);

        GameObject terrainGo = Terrain.CreateTerrainGameObject(td);
        terrainGo.name = "Terrain";
        terrainGo.transform.SetParent(root.transform, false);
        terrainGo.transform.position = V3(tj.terrain.position);
        Terrain terrain = terrainGo.GetComponent<Terrain>();
        terrain.drawInstanced = true;
        terrain.treeDistance = 400f;
        terrain.treeBillboardDistance = 80f;
        terrain.heightmapPixelError = 5f;

        if (importPlacements)
        {
            EditorUtility.DisplayProgressBar("Extraction import", "Placements", 0.7f);
            PlacePrefabs(root.transform);
        }
        if (importRocks)
        {
            EditorUtility.DisplayProgressBar("Extraction import", "Rocks", 0.75f);
            PlaceRocks(root.transform);
        }
        if (importMarkers)
        {
            EditorUtility.DisplayProgressBar("Extraction import", "Markers", 0.8f);
            PlaceMarkers(root.transform);
        }
        if (importWater)
        {
            EditorUtility.DisplayProgressBar("Extraction import", "Water", 0.9f);
            BuildWater(root.transform, mapId);
        }
        if (addFog)
        {
            // The game's look: grey, close, always there. Tune to taste.
            RenderSettings.fog = true;
            RenderSettings.fogMode = FogMode.ExponentialSquared;
            RenderSettings.fogColor = new Color(0.55f, 0.58f, 0.6f);
            RenderSettings.fogDensity = 0.012f;
        }

        AssetDatabase.SaveAssets();
        if (newScene)
        {
            string scenePath = outputFolder + "/" + mapId + ".unity";
            EditorSceneManager.SaveScene(SceneManager.GetActiveScene(), scenePath);
        }
        Selection.activeGameObject = root;
        Debug.Log("Extraction: imported " + tj.map + " from " + exportFolder);
    }

    TerrainData BuildTerrainData(TerrainJson tj)
    {
        int res = tj.heightmap.resolution;
        byte[] raw = File.ReadAllBytes(Path.Combine(exportFolder, tj.heightmap.file));
        if (raw.Length != res * res * 2)
            throw new Exception("heightmap.r16 is " + raw.Length + " bytes; expected " + (res * res * 2));

        var td = new TerrainData();
        // Resolution first: setting it afterwards rescales the size.
        td.heightmapResolution = res;
        td.size = V3(tj.terrain.size);

        // Rows run south to north and columns west to east, which is
        // SetHeights' [row (z), column (x)] order. Little-endian u16.
        var heights = new float[res, res];
        for (int row = 0; row < res; row++)
        {
            for (int col = 0; col < res; col++)
            {
                int i = (row * res + col) * 2;
                int v = raw[i] | (raw[i + 1] << 8);
                heights[row, col] = v / 65535f;
            }
        }
        td.SetHeights(0, 0, heights);
        return td;
    }

    void PaintSplats(TerrainJson tj, TerrainData td)
    {
        LayerInfo[] infos = tj.alphamap.layers;
        int res = tj.alphamap.resolution;
        var layers = new TerrainLayer[infos.Length];
        for (int l = 0; l < infos.Length; l++) layers[l] = TerrainLayerFor(infos[l].name);
        td.alphamapResolution = res;
        td.terrainLayers = layers;

        var maps = new float[res, res, infos.Length];
        for (int l = 0; l < infos.Length; l++)
        {
            byte[] png = File.ReadAllBytes(Path.Combine(exportFolder, infos[l].file));
            var tex = new Texture2D(2, 2);
            if (!tex.LoadImage(png)) throw new Exception("Could not read " + infos[l].file);
            if (tex.width != res || tex.height != res)
                throw new Exception(infos[l].file + " is " + tex.width + "x" + tex.height + "; expected " + res);
            // A greyscale PNG may load as R8, Alpha8 or RGB(A); read the
            // channel that carries the value.
            bool alpha = tex.format == TextureFormat.Alpha8;
            Color32[] px = tex.GetPixels32();
            // GetPixels32 starts at the bottom row, which is the PNG's last
            // row: the south edge, alphamap row 0.
            for (int y = 0; y < res; y++)
                for (int x = 0; x < res; x++)
                {
                    Color32 c = px[y * res + x];
                    maps[y, x, l] = (alpha ? c.a : c.r) / 255f;
                }
            DestroyImmediate(tex);
        }
        // Rounding to bytes leaves the sums a hair off 1; normalise.
        for (int y = 0; y < res; y++)
            for (int x = 0; x < res; x++)
            {
                float sum = 0f;
                for (int l = 0; l < infos.Length; l++) sum += maps[y, x, l];
                if (sum <= 0f) { maps[y, x, 0] = 1f; continue; }
                for (int l = 0; l < infos.Length; l++) maps[y, x, l] /= sum;
            }
        td.SetAlphamaps(0, 0, maps);
    }

    TerrainLayer TerrainLayerFor(string name)
    {
        string path = layerFolder + "/" + name + ".terrainlayer";
        var existing = AssetDatabase.LoadAssetAtPath<TerrainLayer>(path);
        if (existing != null) return existing;

        // A flat-colour stand-in, saved so the scene keeps its reference.
        EnsureFolder(layerFolder);
        var tex = new Texture2D(4, 4, TextureFormat.RGBA32, false);
        Color c = PlaceholderColour(name);
        var px = new Color[16];
        for (int i = 0; i < px.Length; i++) px[i] = c;
        tex.SetPixels(px);
        tex.Apply();
        string texPath = layerFolder + "/" + name + "_placeholder.asset";
        AssetDatabase.DeleteAsset(texPath);
        AssetDatabase.CreateAsset(tex, texPath);

        var layer = new TerrainLayer();
        layer.diffuseTexture = tex;
        layer.tileSize = new Vector2(4f, 4f);
        AssetDatabase.CreateAsset(layer, path);
        return layer;
    }

    static Color PlaceholderColour(string name)
    {
        switch (name)
        {
            case "grass": return new Color(0.36f, 0.45f, 0.25f);
            case "forest": return new Color(0.2f, 0.22f, 0.14f);
            case "dirt": return new Color(0.42f, 0.33f, 0.22f);
            case "gravel": return new Color(0.5f, 0.5f, 0.47f);
            case "asphalt": return new Color(0.16f, 0.16f, 0.17f);
            case "rock": return new Color(0.45f, 0.44f, 0.42f);
            case "mud": return new Color(0.25f, 0.19f, 0.13f);
            default: return Color.magenta;
        }
    }

    void PlantTrees(TerrainJson tj, TerrainData td)
    {
        TreesJson trees = ReadJson<TreesJson>(exportFolder, "trees.json");
        // One prototype per species, in first-seen order.
        var species = new System.Collections.Generic.List<string>();
        foreach (TreeJson t in trees.trees)
            if (!species.Contains(t.species)) species.Add(t.species);

        var protos = new TreePrototype[species.Count];
        for (int s = 0; s < species.Count; s++)
            protos[s] = new TreePrototype { prefab = TreePrefabFor(species[s]) };
        td.treePrototypes = protos;

        Vector3 origin = V3(tj.terrain.position);
        Vector3 size = td.size;
        var instances = new TreeInstance[trees.trees.Length];
        for (int i = 0; i < trees.trees.Length; i++)
        {
            TreeJson t = trees.trees[i];
            Vector3 p = V3(t.pos) - origin;
            instances[i] = new TreeInstance
            {
                // Terrain-local and normalised to 0..1.
                position = new Vector3(p.x / size.x, p.y / size.y, p.z / size.z),
                rotation = t.yaw_deg * Mathf.Deg2Rad,
                widthScale = t.scale,
                heightScale = t.scale,
                color = Color.white,
                lightmapColor = Color.white,
                prototypeIndex = species.IndexOf(t.species),
            };
        }
        // Snap to the surface; the game sinks trunks 0.15 m, which is noise
        // at this scale.
        td.SetTreeInstances(instances, true);
        td.RefreshPrototypes();
    }

    GameObject TreePrefabFor(string species)
    {
        var prefab = AssetDatabase.LoadAssetAtPath<GameObject>(treeFolder + "/" + species + ".prefab");
        if (prefab != null) return prefab;
        string path = outputFolder + "/placeholder_tree_" + species + ".prefab";
        prefab = AssetDatabase.LoadAssetAtPath<GameObject>(path);
        if (prefab != null) return prefab;

        // Terrain trees want a renderer or an LODGroup on the root, and
        // take their collider from the root too.
        var go = new GameObject("tree_" + species);
        bool birch = species == "birch";
        bool snag = species == "snag";
        float height = birch ? 12f : 16f;
        GameObject trunk = Primitive(PrimitiveType.Cylinder, go.transform,
            new Vector3(0f, height * 0.5f, 0f), new Vector3(0.35f, height * 0.5f, 0.35f),
            snag ? new Color(0.55f, 0.53f, 0.5f) : birch ? new Color(0.85f, 0.83f, 0.78f) : new Color(0.3f, 0.22f, 0.16f));
        Renderer[] renderers;
        if (snag)
        {
            renderers = new[] { trunk.GetComponent<Renderer>() };
        }
        else
        {
            GameObject crown = Primitive(PrimitiveType.Capsule, go.transform,
                new Vector3(0f, height * 0.62f, 0f),
                birch ? new Vector3(4f, 4f, 4f) : new Vector3(4.5f, height * 0.4f, 4.5f),
                birch ? new Color(0.45f, 0.55f, 0.3f) : new Color(0.16f, 0.25f, 0.17f));
            renderers = new[] { trunk.GetComponent<Renderer>(), crown.GetComponent<Renderer>() };
        }
        var lod = go.AddComponent<LODGroup>();
        lod.SetLODs(new[] { new LOD(0.01f, renderers) });
        var col = go.AddComponent<CapsuleCollider>();
        col.radius = 0.3f;
        col.height = 4f;
        col.center = new Vector3(0f, 2f, 0f);
        GameObject saved = PrefabUtility.SaveAsPrefabAsset(go, path);
        DestroyImmediate(go);
        return saved;
    }

    // A primitive with its collider stripped and its own saved material.
    GameObject Primitive(PrimitiveType type, Transform parent, Vector3 pos, Vector3 scale, Color colour)
    {
        GameObject g = GameObject.CreatePrimitive(type);
        DestroyImmediate(g.GetComponent<Collider>());
        g.transform.SetParent(parent, false);
        g.transform.localPosition = pos;
        g.transform.localScale = scale;
        g.GetComponent<Renderer>().sharedMaterial = ColourMaterial(colour);
        return g;
    }

    Material ColourMaterial(Color colour)
    {
        string path = outputFolder + "/mat_" + ColorUtility.ToHtmlStringRGB(colour) + ".mat";
        var mat = AssetDatabase.LoadAssetAtPath<Material>(path);
        if (mat != null) return mat;
        mat = new Material(Shader.Find("Standard"));
        mat.color = colour;
        mat.enableInstancing = true;
        AssetDatabase.CreateAsset(mat, path);
        return mat;
    }

    void PlacePrefabs(Transform root)
    {
        PlacementsJson pj = ReadJson<PlacementsJson>(exportFolder, "placements.json");
        var parent = new GameObject("Placements").transform;
        parent.SetParent(root, false);
        Material blockout = ColourMaterial(new Color(0.6f, 0.58f, 0.55f));
        Font font = Resources.GetBuiltinResource<Font>("LegacyRuntime.ttf");
        foreach (PlacementJson p in pj.placements)
        {
            string label = string.IsNullOrEmpty(p.tag) ? p.prefab : p.prefab + " (" + p.tag + ")";
            Vector3 pos = V3(p.pos);
            var prefab = AssetDatabase.LoadAssetAtPath<GameObject>(prefabFolder + "/" + p.prefab + ".prefab");
            if (prefab != null)
            {
                var go = (GameObject)PrefabUtility.InstantiatePrefab(prefab, parent);
                go.name = label;
                go.transform.position = pos;
                go.transform.rotation = Quaternion.Euler(0f, p.yaw_deg + modelYawOffset, 0f);
                continue;
            }
            // Fallback: a box of the footprint (width x, height, depth z),
            // standing on the ground, with its id floating above it.
            Vector3 f = V3(p.footprint);
            var box = GameObject.CreatePrimitive(PrimitiveType.Cube);
            box.name = label + " [blockout]";
            box.transform.SetParent(parent, false);
            box.transform.position = pos + Vector3.up * (f.y * 0.5f);
            box.transform.rotation = Quaternion.Euler(0f, p.yaw_deg, 0f);
            box.transform.localScale = f;
            box.GetComponent<Renderer>().sharedMaterial = blockout;
            box.isStatic = true;

            // A sibling, not a child: the box's scale would stretch it.
            var text = new GameObject(label + " [label]");
            text.transform.SetParent(parent, false);
            text.transform.position = pos + Vector3.up * (f.y + 1f);
            text.transform.rotation = Quaternion.Euler(0f, p.yaw_deg, 0f);
            var tm = text.AddComponent<TextMesh>();
            tm.text = label;
            tm.anchor = TextAnchor.LowerCenter;
            tm.alignment = TextAlignment.Center;
            tm.characterSize = 0.25f;
            tm.fontSize = 48;
            if (font != null)
            {
                tm.font = font;
                text.GetComponent<MeshRenderer>().sharedMaterial = font.material;
            }
        }
    }

    void PlaceRocks(Transform root)
    {
        RocksJson rj = ReadJson<RocksJson>(exportFolder, "rocks.json");
        var parent = new GameObject("Rocks").transform;
        parent.SetParent(root, false);
        Material grey = ColourMaterial(new Color(0.45f, 0.45f, 0.42f));
        foreach (RockJson r in rj.rocks)
        {
            Vector3 scale = V3(r.scale);
            var prefab = AssetDatabase.LoadAssetAtPath<GameObject>(prefabFolder + "/boulder_" + r.variant + ".prefab");
            GameObject go;
            if (prefab != null)
            {
                go = (GameObject)PrefabUtility.InstantiatePrefab(prefab, parent);
            }
            else
            {
                go = GameObject.CreatePrimitive(PrimitiveType.Sphere);
                go.transform.SetParent(parent, false);
                go.GetComponent<Renderer>().sharedMaterial = grey;
                // A sphere's radius is 0.5: double the scale to match.
                scale *= 2f;
            }
            go.name = "Rock " + r.variant;
            go.transform.position = V3(r.pos);
            go.transform.rotation = Quaternion.Euler(0f, r.yaw_deg, 0f);
            go.transform.localScale = scale;
            go.isStatic = true;
            // Small stones are stepped over.
            if (r.scale[0] < rj.solid_radius_m)
            {
                var col = go.GetComponent<Collider>();
                if (col != null) DestroyImmediate(col);
            }
        }
    }

    void PlaceMarkers(Transform root)
    {
        MarkersJson mj = ReadJson<MarkersJson>(exportFolder, "markers.json");
        var parent = new GameObject("Markers").transform;
        parent.SetParent(root, false);
        foreach (MarkerJson m in mj.markers)
        {
            string label = m.kind;
            if (!string.IsNullOrEmpty(m.loot_table)) label += "[" + m.loot_table + "]";
            if (!string.IsNullOrEmpty(m.name)) label += ": " + m.name;
            var go = new GameObject(label);
            go.transform.SetParent(parent, false);
            go.transform.position = V3(m.pos);
            go.transform.rotation = Quaternion.Euler(0f, m.yaw_deg, 0f);
        }
    }

    void BuildWater(Transform root, string mapId)
    {
        WaterJson wj = ReadJson<WaterJson>(exportFolder, "water.json");
        var parent = new GameObject("Water").transform;
        parent.SetParent(root, false);
        Material mat = ColourMaterial(new Color(0.16f, 0.2f, 0.2f));
        mat.SetFloat("_Glossiness", 0.9f);
        EditorUtility.SetDirty(mat);
        for (int w = 0; w < wj.water.Length; w++)
        {
            WaterBody body = wj.water[w];
            int n = body.centreline_flat.Length / 3;
            if (n < 2) continue;
            // A ribbon along the centreline, a little wider than the stream
            // so its edges tuck under the banks (as the game's water does).
            float half = body.width_m * 0.5f + 2f;
            var verts = new Vector3[n * 2];
            var uvs = new Vector2[n * 2];
            var tris = new int[(n - 1) * 6];
            float along = 0f;
            for (int i = 0; i < n; i++)
            {
                Vector3 p = Point(body.centreline_flat, i);
                Vector3 a = Point(body.centreline_flat, Mathf.Max(i - 1, 0));
                Vector3 b = Point(body.centreline_flat, Mathf.Min(i + 1, n - 1));
                Vector3 dir = b - a;
                dir.y = 0f;
                dir = dir.sqrMagnitude > 1e-6f ? dir.normalized : Vector3.forward;
                // Right of travel, looking down in Unity's left-handed frame.
                var right = new Vector3(dir.z, 0f, -dir.x);
                if (i > 0) along += Vector3.Distance(p, Point(body.centreline_flat, i - 1));
                verts[i * 2] = p - right * half;
                verts[i * 2 + 1] = p + right * half;
                uvs[i * 2] = new Vector2(0f, along / 4f);
                uvs[i * 2 + 1] = new Vector2(1f, along / 4f);
                if (i < n - 1)
                {
                    int k = i * 6;
                    int v = i * 2;
                    // Clockwise seen from above, so the face points up.
                    tris[k] = v; tris[k + 1] = v + 2; tris[k + 2] = v + 1;
                    tris[k + 3] = v + 1; tris[k + 4] = v + 2; tris[k + 5] = v + 3;
                }
            }
            var mesh = new Mesh { name = "water_" + w };
            mesh.vertices = verts;
            mesh.uv = uvs;
            mesh.triangles = tris;
            mesh.RecalculateNormals();
            mesh.RecalculateBounds();
            string meshPath = outputFolder + "/" + mapId + "_water_" + w + ".asset";
            AssetDatabase.DeleteAsset(meshPath);
            AssetDatabase.CreateAsset(mesh, meshPath);

            var go = new GameObject(string.IsNullOrEmpty(body.name) ? "water " + w : body.name);
            go.transform.SetParent(parent, false);
            go.AddComponent<MeshFilter>().sharedMesh = mesh;
            go.AddComponent<MeshRenderer>().sharedMaterial = mat;
            go.isStatic = true;
        }
    }

    static Vector3 Point(float[] flat, int i)
    {
        return new Vector3(flat[i * 3], flat[i * 3 + 1], flat[i * 3 + 2]);
    }

    // ---- Helpers.

    static string Safe(string name)
    {
        if (string.IsNullOrEmpty(name)) return "map";
        foreach (char c in Path.GetInvalidFileNameChars()) name = name.Replace(c, '_');
        return name.Replace(' ', '_').ToLowerInvariant();
    }

    // Creates an Assets/... folder and its parents.
    static void EnsureFolder(string path)
    {
        path = path.TrimEnd('/');
        if (AssetDatabase.IsValidFolder(path)) return;
        int slash = path.LastIndexOf('/');
        if (slash <= 0) throw new Exception("Folders must be inside Assets/: " + path);
        string parent = path.Substring(0, slash);
        EnsureFolder(parent);
        AssetDatabase.CreateFolder(parent, path.Substring(slash + 1));
    }
}
