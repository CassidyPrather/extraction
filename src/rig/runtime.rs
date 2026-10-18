//! Which OpenXR runtime the game talks to.
//!
//! Windows keeps one "active" OpenXR runtime for the whole machine (Meta's,
//! SteamVR's, WMR's), whichever installed or claimed it last, and the game
//! gets that one. `--steamvr` points this launch at SteamVR's instead,
//! through the loader's standard `XR_RUNTIME_JSON`; if the active runtime
//! finds no headset and SteamVR is installed, the game reopens itself on
//! SteamVR once (`xr::headset_missing`).

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// What `steamvr_env` decided, for the log once it's up.
pub static NOTE: OnceLock<String> = OnceLock::new();

/// SteamVR's runtime manifest, if SteamVR is installed in any Steam
/// library this machine knows about.
pub fn steamvr_runtime() -> Option<PathBuf> {
    let file = if cfg!(windows) {
        "steamxr_win64.json"
    } else {
        "steamxr_linux64.json"
    };
    steam_roots()
        .iter()
        .flat_map(|root| libraries(root))
        .map(|lib| lib.join("steamapps/common/SteamVR").join(file))
        .find(|p| p.exists())
}

/// SteamVR's manifest when this launch should switch to it: the loader
/// isn't already pointed somewhere (`XR_RUNTIME_JSON`) and SteamVR is
/// installed. Says what it decided in [`NOTE`] otherwise.
pub fn steamvr_env() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("XR_RUNTIME_JSON") {
        let _ = NOTE.set(format!(
            "OpenXR runtime from XR_RUNTIME_JSON: {}",
            p.to_string_lossy()
        ));
        return None;
    }
    let found = steamvr_runtime();
    if found.is_none() {
        let _ = NOTE.set(
            "--steamvr: SteamVR's OpenXR runtime not found in any Steam library; \
             using the active runtime"
                .into(),
        );
    }
    found
}

/// This program again, on the OpenXR runtime `manifest` (the loader reads
/// `XR_RUNTIME_JSON`; setting it on the child keeps this process's own
/// environment untouched), with `--steamvr` so it doesn't switch again.
pub fn relaunch_on(manifest: &Path) -> std::io::Result<std::process::Command> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if !args.iter().any(|a| a == "--steamvr") {
        args.push("--steamvr".into());
    }
    let mut cmd = std::process::Command::new(std::env::current_exe()?);
    cmd.args(args).env("XR_RUNTIME_JSON", manifest);
    Ok(cmd)
}

fn steam_roots() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if cfg!(windows) {
        for var in ["ProgramFiles(x86)", "ProgramFiles"] {
            if let Some(p) = std::env::var_os(var) {
                out.push(PathBuf::from(p).join("Steam"));
            }
        }
    } else if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        out.push(home.join(".steam/steam"));
        out.push(home.join(".local/share/Steam"));
    }
    out
}

/// A Steam install's libraries: itself, and every library its
/// `libraryfolders.vdf` lists (games can live on other drives).
fn libraries(root: &Path) -> Vec<PathBuf> {
    let mut out = vec![root.to_path_buf()];
    if let Ok(text) = std::fs::read_to_string(root.join("steamapps/libraryfolders.vdf")) {
        out.extend(library_paths(&text));
    }
    out
}

fn library_paths(vdf: &str) -> Vec<PathBuf> {
    vdf.lines()
        .filter_map(|l| l.trim().strip_prefix("\"path\""))
        .map(|rest| PathBuf::from(rest.trim().trim_matches('"').replace("\\\\", "\\")))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steam_library_folders_are_read() {
        let vdf = r#""libraryfolders"
{
	"0"
	{
		"path"		"C:\\Program Files (x86)\\Steam"
		"label"		""
	}
	"1"
	{
		"path"		"D:\\SteamLibrary"
	}
}"#;
        assert_eq!(
            library_paths(vdf),
            vec![
                PathBuf::from(r"C:\Program Files (x86)\Steam"),
                PathBuf::from(r"D:\SteamLibrary")
            ]
        );
    }
}
