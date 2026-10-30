//! A capture shoots the game as it's meant to look, or not at all.
//!
//! The textures `scripts/fetch_assets.py` makes (materials, foliage
//! cards, terrain, sky) are gitignored. A checkout that never ran it,
//! or ran it before a set was added to the manifest, draws those surfaces
//! in their flat fallback colours: guns with no gun metal, Weapon feel's
//! shots one night. The game runs fine like that, which is the trouble.
//! So a capture run (`--cinematic`, `--sweep`, `--trailer`) or a
//! `--screenshot` checks every output the manifest promises first, and
//! stops with the list and the command that makes them.
//! `EXTRACTION_ALLOW_FALLBACK=1` shoots anyway, for a shot of the fallback
//! itself.

use std::path::Path;

/// Every generated file the manifest promises that isn't on disk, as a
/// path under `assets/textures/`, with its group (the `fetch_assets.py`
/// argument that makes it). Empty without a manifest (a packaged build).
pub fn missing(textures: &Path, manifest: &Path) -> Vec<(String, &'static str)> {
    let Ok(text) = std::fs::read_to_string(manifest) else {
        return Vec::new();
    };
    let Ok(m) = serde_json::from_str::<serde_json::Value>(&text) else {
        return vec![(format!("{} (unreadable)", manifest.display()), "")];
    };
    let mut want: Vec<(String, &'static str)> = Vec::new();
    let names = |v: &serde_json::Value| -> Vec<String> {
        v.as_object()
            .map(|o| o.keys().filter(|k| !k.starts_with('_')).cloned().collect())
            .unwrap_or_default()
    };
    for set in names(&m["materials"]["sets"]) {
        for map in ["albedo", "normal", "arm"] {
            want.push((format!("materials/{set}/{map}.ktx2"), "materials"));
        }
    }
    for card in names(&m["foliage"]["cards"]) {
        want.push((format!("foliage/{card}/albedo.ktx2"), "foliage"));
    }
    if m["terrain"]["layers"]
        .as_array()
        .is_some_and(|l| !l.is_empty())
    {
        for map in ["albedo", "normal", "arm"] {
            want.push((format!("terrain/{map}.ktx2"), "terrain"));
        }
    }
    if let Some(sky) = m["sky"].as_object() {
        for s in sky.values() {
            if let Some(out) = s["out"].as_str() {
                want.push((out.to_owned(), "sky"));
            }
        }
    }
    want.retain(|(f, _)| !textures.join(f).exists());
    want
}

/// Err with what to do, when a capture or screenshot would shoot fallbacks.
pub fn ready_for(opts: &crate::launch::LaunchOptions) -> Result<(), String> {
    if !(opts.capture.active() || opts.screenshot.is_some())
        || std::env::var_os("EXTRACTION_ALLOW_FALLBACK").is_some()
    {
        return Ok(());
    }
    let root = crate::asset_root_path();
    let gone = missing(
        &root.join("textures"),
        &root.join("../scripts/assets_manifest.json"),
    );
    if gone.is_empty() {
        return Ok(());
    }
    let mut groups: Vec<&str> = gone
        .iter()
        .map(|(_, g)| *g)
        .filter(|g| !g.is_empty())
        .collect();
    groups.sort();
    groups.dedup();
    let list: Vec<&str> = gone.iter().take(12).map(|(f, _)| f.as_str()).collect();
    Err(format!(
        "{} generated texture(s) missing, so this shot would show flat fallback colours: {}{}. \
         Make them with `python3 scripts/fetch_assets.py {}` (or set \
         EXTRACTION_ALLOW_FALLBACK=1 to shoot the fallback on purpose).",
        gone.len(),
        list.join(", "),
        if gone.len() > list.len() { ", ..." } else { "" },
        groups.join(" "),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A set in the manifest with no files on disk is named, with the
    /// group that makes it; one on disk isn't.
    #[test]
    fn a_missing_material_is_named() {
        let dir = std::env::temp_dir().join(format!("fallback_check_{}", std::process::id()));
        let tex = dir.join("textures");
        for map in ["albedo", "normal", "arm"] {
            let f = tex.join(format!("materials/gun_metal/{map}.ktx2"));
            std::fs::create_dir_all(f.parent().unwrap()).unwrap();
            std::fs::write(f, b"").unwrap();
        }
        let manifest = dir.join("manifest.json");
        std::fs::write(
            &manifest,
            r#"{"materials": {"_about": "", "sets": {"gun_metal": {}, "polymer_plum": {}}},
                "foliage": {"cards": {}}, "terrain": {"layers": []}, "sky": {}}"#,
        )
        .unwrap();
        let gone = missing(&tex, &manifest);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(
            gone.iter()
                .map(|(f, g)| format!("{g}:{f}"))
                .collect::<Vec<_>>(),
            [
                "materials:materials/polymer_plum/albedo.ktx2",
                "materials:materials/polymer_plum/normal.ktx2",
                "materials:materials/polymer_plum/arm.ktx2",
            ]
        );
    }

    /// The real manifest reads, and names something for each group, so a
    /// change to its shape can't quietly switch the check off.
    #[test]
    fn the_manifest_promises_every_group() {
        let root = crate::asset_root_path();
        let empty = std::env::temp_dir().join("fallback_check_nothing_here");
        let gone = missing(&empty, &root.join("../scripts/assets_manifest.json"));
        for group in ["materials", "foliage", "terrain", "sky"] {
            assert!(
                gone.iter().any(|(_, g)| *g == group),
                "the manifest promises nothing in {group}: has its shape changed?"
            );
        }
    }
}
