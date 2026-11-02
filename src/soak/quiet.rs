//! A quiet log: a few sessions played headless (the yard, raids in two
//! valleys, and the client in the yard with no GPU), and any WARN or ERROR
//! line whose class isn't on [`ALLOWED`] fails. One night's headset log
//! carried 354 "Entity despawned" errors, and a real problem among them
//! would have gone unread.
//!
//! Each session runs in a child process (this test binary again, running
//! [`tests::quiet_session`] alone), so its log is its own: Bevy's logger and
//! task pools are the process's. A class is the line's level, target and
//! message with numbers and entity ids taken out.

/// Warnings and errors a session may log, each with why: (level, target,
/// a piece of the message, why).
pub const ALLOWED: &[(&str, &str, &str, &str)] = &[
    // The client session has no GPU and no sound card: what the harness
    // lacks, not what the game does.
    (
        "WARN",
        "bevy_render::texture",
        "CompressedImageFormatSupport resource not found",
        "no GPU in the client session",
    ),
    (
        "WARN",
        "bevy_gltf",
        "CompressedImageFormatSupport resource not found",
        "no GPU in the client session",
    ),
    (
        "WARN",
        "bevy_asset::server::loaders",
        "Duplicate AssetLoader registered",
        "no GPU in the client session: the image and glTF loaders register twice without one",
    ),
    (
        "WARN",
        "bevy_gizmos_render",
        "RenderApp was not detected",
        "no GPU in the client session",
    ),
    (
        "ERROR",
        "bevy_render::extract_resource",
        "Render app did not exist",
        "no GPU in the client session",
    ),
    (
        "WARN",
        "bevy_ecs::error::handler",
        "Failed to get audio device config",
        "no sound card in the client session",
    ),
    (
        "ERROR",
        "bevy_seedling::edge",
        "no Firewheel node found",
        "no sound card in the client session: the audio graph is never built",
    ),
    // Once, at the start, before there's a camera to measure shadows
    // from: in the headset the left eye takes it once the eyes exist
    // (`rig::xr::dress_eyes`); with no window here, never. Every session
    // of the first headset night had it once too.
    (
        "WARN",
        "bevy_light",
        "no entity has `ShadowLodOrigin`",
        "once, before the eyes exist (the left eye carries the origin)",
    ),
];

/// A log line's class: (level, target, message with numbers and entity
/// ids taken out), if it's a WARN or an ERROR.
pub fn class(line: &str) -> Option<(String, String, String)> {
    // Colours off.
    let mut plain = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            for d in chars.by_ref() {
                if d.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            plain.push(c);
        }
    }
    let mut words = plain.split_whitespace();
    let mut level = None;
    for w in words.by_ref() {
        if w == "WARN" || w == "ERROR" {
            level = Some(w.to_owned());
            break;
        }
    }
    let level = level?;
    let target = words.next()?.trim_end_matches(':').to_owned();
    let rest: Vec<&str> = words.collect();
    let message: String = rest
        .join(" ")
        .split(|c: char| c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("#");
    Some((level, target, message))
}

/// Allowed only once the session is closing (a line with one of
/// [`EXITING`] came before it): teardown, not a fault.
pub const EXIT_ONLY: &[(&str, &str, &str, &str)] = &[(
    "WARN",
    "bevy_render::diagnostic::internal",
    "Failed to download render statistics buffer",
    "a buffer map still pending as the app tears down (Performance: only ever after an exit)",
)];

/// What a session's log says as it closes.
pub const EXITING: &[&str] = &["exit asked", "No windows are open"];

/// Whether a class is on the allow-list (`exiting`: the session has said
/// it's closing).
pub fn allowed(level: &str, target: &str, message: &str, exiting: bool) -> bool {
    let on = |list: &[(&str, &str, &str, &str)]| {
        list.iter()
            .any(|(l, t, m, _)| *l == level && *t == target && message.contains(m))
    };
    on(ALLOWED) || (exiting && on(EXIT_ONLY))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The sessions: (name, what they play).
    const SESSIONS: &[&str] = &["yard", "marrow_creek", "kettle_pass", "client_yard"];
    /// Game seconds each plays.
    const SECS: f32 = 45.0;
    /// Game seconds a session without the downloads plays: the loads are
    /// in its first seconds.
    const BARE_SECS: f32 = 15.0;

    /// One session, alone in its own process (the parent below runs it).
    #[test]
    #[ignore = "a child of no_session_logs_a_warning"]
    fn quiet_session() {
        let Ok(which) = std::env::var("QUIET_SESSION") else {
            return;
        };
        let out = std::env::temp_dir().join(format!("extraction-quiet-{}", std::process::id()));
        let mut app = match which.as_str() {
            // The yard's own bot, through the stations.
            "yard" => crate::headless_app(
                crate::LaunchOptions {
                    robots: false,
                    map: crate::camp::TRAINING_YARD.into(),
                    map_explicit: true,
                    scenario: Some(crate::onboarding::bot::SCENARIO.into()),
                    ..Default::default()
                },
                true,
            ),
            // The sims' raider, robots and all.
            "marrow_creek" | "kettle_pass" => crate::headless_app(
                crate::LaunchOptions {
                    map: which.clone(),
                    map_explicit: true,
                    sim_run: Some("quiet".into()),
                    sim_out: Some(out.clone()),
                    ..Default::default()
                },
                true,
            ),
            // The whole client, with no window and no GPU.
            "client_yard" => {
                let opts = crate::LaunchOptions {
                    robots: false,
                    ..super::super::drive::options(crate::camp::TRAINING_YARD, 1, None)
                };
                super::super::drive::client_app(opts)
            }
            // The whole client in a valley, robots and all.
            s if s.starts_with("client_") => {
                let map = &s["client_".len()..];
                super::super::drive::client_app(super::super::drive::options(map, 1, None))
            }
            other => panic!("no session {other}"),
        };
        app.finish();
        app.cleanup();
        let secs = std::env::var("QUIET_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(SECS);
        for _ in 0..(secs * 90.0) as usize {
            app.update();
        }
        let _ = std::fs::remove_dir_all(&out);
        println!("QUIET SESSION DONE {which}");
    }

    /// What `sessions` log that isn't on the allow-list, a line a class
    /// (empty: quiet), each played in its own child process with `env`,
    /// `at_once` at a time.
    fn unallowed(sessions: &[&str], env: &[(&str, &str)], at_once: usize) -> Vec<String> {
        let exe = std::env::current_exe().unwrap();
        let mut found: std::collections::BTreeMap<(String, String, String), (usize, Vec<&str>)> =
            Default::default();
        for batch in sessions.chunks(at_once) {
            let children: Vec<(&str, std::process::Child)> = batch
                .iter()
                .map(|s| {
                    let child = std::process::Command::new(&exe)
                        .args([
                            "--ignored",
                            "--exact",
                            "soak::quiet::tests::quiet_session",
                            "--nocapture",
                            "--test-threads=1",
                        ])
                        .env("QUIET_SESSION", s)
                        .env("RUST_LOG", "warn")
                        .env("EXTRACTION_THREADS", "1")
                        .envs(env.iter().copied())
                        .stdout(std::process::Stdio::piped())
                        .stderr(std::process::Stdio::piped())
                        .spawn()
                        .expect("the session");
                    (*s, child)
                })
                .collect();
            for (s, child) in children {
                let out = child.wait_with_output().unwrap();
                let text = String::from_utf8_lossy(&out.stdout).into_owned()
                    + &String::from_utf8_lossy(&out.stderr);
                assert!(
                    text.contains(&format!("QUIET SESSION DONE {s}")),
                    "the {s} session didn't finish:\n{}",
                    text.lines().rev().take(40).collect::<Vec<_>>().join("\n")
                );
                let mut exiting = false;
                for line in text.lines() {
                    exiting |= EXITING.iter().any(|e| line.contains(e));
                    if let Some(c) = class(line) {
                        if !allowed(&c.0, &c.1, &c.2, exiting) {
                            let e = found.entry(c).or_default();
                            e.0 += 1;
                            if !e.1.contains(&s) {
                                e.1.push(s);
                            }
                        }
                    }
                }
            }
        }
        found
            .iter()
            .map(|((l, t, m), (n, s))| format!("{n}x {l} {t}: {m} (in {})", s.join(", ")))
            .collect()
    }

    /// No session logs a warning or an error that isn't on the
    /// allow-list (each listed with why).
    #[test]
    fn no_session_logs_a_warning() {
        let report = unallowed(SESSIONS, &[], SESSIONS.len());
        assert!(
            report.is_empty(),
            "warnings and errors not on soak::quiet::ALLOWED (fix them, or list them there \
             with why):\n{}",
            report.join("\n")
        );
    }

    /// `dir/assets` as a checkout that never ran `scripts/fetch_assets.py`:
    /// everything in `assets/` linked in but the folders it makes
    /// (`capture::assets::FETCHED`), and `scripts/` beside it.
    fn bare_checkout(dir: &std::path::Path) -> std::path::PathBuf {
        use std::os::unix::fs::symlink;
        let real = crate::asset_root_path();
        let _ = std::fs::remove_dir_all(dir);
        let assets = dir.join("assets");
        std::fs::create_dir_all(assets.join("textures")).unwrap();
        symlink(real.join("../scripts"), dir.join("scripts")).unwrap();
        for e in std::fs::read_dir(&real).unwrap().flatten() {
            let name = e.file_name();
            if name == "textures" || name == ".cache" {
                continue;
            }
            symlink(e.path(), assets.join(&name)).unwrap();
        }
        for e in std::fs::read_dir(real.join("textures")).unwrap().flatten() {
            let name = e.file_name();
            if crate::capture::assets::FETCHED.iter().any(|f| name == *f) {
                continue;
            }
            symlink(e.path(), assets.join("textures").join(&name)).unwrap();
        }
        assets
    }

    /// A checkout without the downloads (the textures
    /// `scripts/fetch_assets.py` makes) plays every valley and the yard
    /// in the whole client with no warning or error off the allow-list:
    /// every load of a downloaded file checks it's there and falls back
    /// (a missing file's load is an ERROR, and once failed `soak::quiet` on
    /// every machine without them).
    #[test]
    fn no_valley_logs_an_error_without_the_downloads() {
        let dir = std::env::temp_dir().join(format!("extraction-bare-{}", std::process::id()));
        let assets = bare_checkout(&dir);
        assert!(
            !crate::capture::assets::missing(
                &assets.join("textures"),
                &assets.join("../scripts/assets_manifest.json")
            )
            .is_empty(),
            "the bare checkout still has the downloads"
        );
        let mut sessions: Vec<String> = vec!["client_yard".into()];
        sessions.extend(
            extraction_core::map::VALLEYS
                .iter()
                .map(|(m, _)| format!("client_{m}")),
        );
        let sessions: Vec<&str> = sessions.iter().map(String::as_str).collect();
        let secs = BARE_SECS.to_string();
        let root = assets.to_string_lossy().into_owned();
        let report = unallowed(
            &sessions,
            &[("EXTRACTION_ASSET_ROOT", &root), ("QUIET_SECS", &secs)],
            3,
        );
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            report.is_empty(),
            "without the downloads, warnings and errors (guard the load with an exists() \
             check and a fallback, as world::trees does):\n{}",
            report.join("\n")
        );
    }

    #[test]
    fn classes_drop_numbers_and_colours() {
        let line = "\u{1b}[2m2026-10-28T06:24:44.6Z\u{1b}[0m \u{1b}[33m WARN\u{1b}[0m \
                    \u{1b}[2mbevy_ecs::error::handler\u{1b}[0m\u{1b}[2m:\u{1b}[0m Entity \
                    despawned: The entity with ID 412v7 is invalid";
        let (l, t, m) = class(line).unwrap();
        assert_eq!(l, "WARN");
        assert_eq!(t, "bevy_ecs::error::handler");
        assert_eq!(m, "Entity despawned: The entity with ID #v# is invalid");
        assert!(class("2026 INFO extraction: fine").is_none());
        // Teardown's warning passes only after the exit.
        let stats = "WARN bevy_render::diagnostic::internal: Failed to download render \
                     statistics buffer: Error occurred";
        let (l, t, m) = class(stats).unwrap();
        assert!(!allowed(&l, &t, &m, false));
        assert!(allowed(&l, &t, &m, true));
    }
}
