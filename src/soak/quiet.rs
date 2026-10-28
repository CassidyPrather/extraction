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

/// Whether a class is on the allow-list.
pub fn allowed(level: &str, target: &str, message: &str) -> bool {
    ALLOWED
        .iter()
        .any(|(l, t, m, _)| *l == level && *t == target && message.contains(m))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The sessions: (name, what they play).
    const SESSIONS: &[&str] = &["yard", "marrow_creek", "kettle_pass", "client_yard"];
    /// Game seconds each plays.
    const SECS: f32 = 45.0;

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
            other => panic!("no session {other}"),
        };
        app.finish();
        app.cleanup();
        for _ in 0..(SECS * 90.0) as usize {
            app.update();
        }
        let _ = std::fs::remove_dir_all(&out);
        println!("QUIET SESSION DONE {which}");
    }

    /// No session logs a warning or an error that isn't on the
    /// allow-list (each listed with why).
    #[test]
    fn no_session_logs_a_warning() {
        let exe = std::env::current_exe().unwrap();
        let children: Vec<(&str, std::process::Child)> = SESSIONS
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
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped())
                    .spawn()
                    .expect("the session");
                (*s, child)
            })
            .collect();
        let mut found: std::collections::BTreeMap<(String, String, String), (usize, Vec<&str>)> =
            Default::default();
        for (s, child) in children {
            let out = child.wait_with_output().unwrap();
            let text = String::from_utf8_lossy(&out.stdout).into_owned()
                + &String::from_utf8_lossy(&out.stderr);
            assert!(
                text.contains(&format!("QUIET SESSION DONE {s}")),
                "the {s} session didn't finish:\n{}",
                text.lines().rev().take(40).collect::<Vec<_>>().join("\n")
            );
            for line in text.lines() {
                if let Some(c) = class(line) {
                    if !allowed(&c.0, &c.1, &c.2) {
                        let e = found.entry(c).or_default();
                        e.0 += 1;
                        if !e.1.contains(&s) {
                            e.1.push(s);
                        }
                    }
                }
            }
        }
        let report: Vec<String> = found
            .iter()
            .map(|((l, t, m), (n, s))| format!("{n}x {l} {t}: {m} (in {})", s.join(", ")))
            .collect();
        assert!(
            report.is_empty(),
            "warnings and errors not on soak::quiet::ALLOWED (fix them, or list them there \
             with why):\n{}",
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
    }
}
