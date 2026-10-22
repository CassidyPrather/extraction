//! The playtester's report: one action writes one zip to send back.
//! REPORT on the truck's board, or `extraction --report` (which needs no
//! window or headset, so it works after a crash), writes
//! `<data dir>/reports/report-<when>.zip` holding:
//! - `system.txt`: the build, the OS, the GPU and its driver, the OpenXR
//!   runtime and the eyes' resolution as it reported them (the newest
//!   session log's `report:` and `eyes:` lines);
//! - `frames-summary.txt`: per valley, CPU and GPU milliseconds a frame
//!   (p50, p99, p99.9, worst) from the frame logs, once the world was in;
//! - `load.txt`: each session's `load:` lines (`render::timeline`);
//! - `logs/`: the newest session logs and crash reports;
//! - `frames/`: the frame logs themselves (`frame cpu_ms gpu_ms`);
//! - `settings.ron`, and `raid/`: the last raid's telemetry.
//!
//! Private by construction: nothing but these files, and in their text the
//! home folder becomes `~`, the user's name `<user>` and the machine's name
//! `<host>`. The only hardware named is the GPU model and driver.
//! See docs/PLAYTEST.md, "Sending a report".

use std::path::{Path, PathBuf};

/// The newest this many session logs, crash reports and frame logs go in.
const KEEP: usize = 4;

/// Writes the report from `data` (the data folder) into
/// `data/reports/`, and returns where.
pub fn write_report(data: &Path) -> std::io::Result<PathBuf> {
    let scrub = Scrub::from_env();
    let mut zip = Zip::default();
    let logs = data.join("logs");
    let sessions = newest(&logs, "session-", ".log", KEEP);
    let crashes = newest(&logs, "crash-", ".txt", KEEP);
    let frames = newest(&logs, "frames-", ".txt", KEEP * 2);

    let mut readme = String::from(
        "Extraction playtest report. Send this whole zip back.\n\n\
         system.txt          build, OS, GPU, headset runtime, eye resolution\n\
         frames-summary.txt  frame times per valley (ms: p50, p99, p99.9, worst)\n\
         load.txt            how long each step of loading took\n\
         logs/               the newest session logs and crash reports\n\
         frames/             one line a frame: frame cpu_ms gpu_ms\n\
         settings.ron        your comfort settings\n\
         raid/               the last raid's recording\n\n\
         Your home folder, user name and computer name are replaced with\n\
         ~, <user> and <host> in everything here.\n",
    );
    readme += &format!("\nWritten by {}.\n", build());
    zip.add("README.txt", readme.as_bytes());
    zip.add("system.txt", system(&sessions, &scrub).as_bytes());
    zip.add(
        "frames-summary.txt",
        frames_summary(&frames, &scrub).as_bytes(),
    );
    let mut load = String::new();
    for s in &sessions {
        let text = read(s, &scrub);
        load += &format!("{}\n", name(s));
        for l in text.lines().filter_map(|l| l.split_once(" load: ")) {
            load += &format!("  {}\n", l.1);
        }
        load += "\n";
        zip.add(&format!("logs/{}", name(s)), text.as_bytes());
    }
    zip.add("load.txt", load.as_bytes());
    for c in &crashes {
        zip.add(&format!("logs/{}", name(c)), read(c, &scrub).as_bytes());
    }
    for f in &frames {
        zip.add(&format!("frames/{}", name(f)), read(f, &scrub).as_bytes());
    }
    let settings = data.join("settings.ron");
    if settings.exists() {
        zip.add("settings.ron", read(&settings, &scrub).as_bytes());
    }
    if let Some(raid) = last_raid(&data.join("raids")) {
        let map = raid
            .parent()
            .and_then(|p| p.file_name())
            .map_or("raid".into(), |m| m.to_string_lossy().to_string());
        zip.add(
            &format!("raid/{map}-{}", name(&raid)),
            read(&raid, &scrub).as_bytes(),
        );
    }

    let dir = data.join("reports");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("report-{}.zip", stamp()));
    std::fs::write(&path, zip.finish())?;
    Ok(path)
}

/// The build, as `git describe` had it.
pub fn build() -> &'static str {
    env!("GIT_DESCRIBE_VERSION")
}

/// The facts a report wants from this session, logged once at start as
/// `report: <what>: <value>` (the session log is where a later
/// `--report` finds them).
pub fn log_fact(what: &str, value: &str) {
    bevy::log::info!("report: {what}: {value}");
}

/// Logs this session's facts for the report once the GPU is known.
pub struct ReportPlugin;

impl bevy::prelude::Plugin for ReportPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        app.add_systems(bevy::prelude::Update, log_facts);
    }
}

fn log_facts(
    adapter: Option<bevy::prelude::Res<bevy::render::renderer::RenderAdapterInfo>>,
    opts: Option<bevy::prelude::Res<crate::launch::LaunchOptions>>,
    mut done: bevy::prelude::Local<bool>,
) {
    if *done {
        return;
    }
    *done = true;
    log_fact("build", build());
    if let Some(o) = opts {
        log_fact("valley", &o.map);
    }
    if let Some(a) = adapter {
        let a = &a.0;
        log_fact(
            "gpu",
            &format!(
                "{} ({:?}, driver {} {})",
                a.name, a.backend, a.driver, a.driver_info
            ),
        );
    }
}

fn system(sessions: &[PathBuf], scrub: &Scrub) -> String {
    let mut out = format!(
        "build: {}\nos: {} {}\n",
        build(),
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    // The newest session that got as far as saying what it ran on.
    for s in sessions {
        let text = read(s, scrub);
        let facts: Vec<String> = text
            .lines()
            .filter_map(|l| {
                l.split_once("report: ")
                    .map(|(_, f)| f.trim().to_string())
                    .or_else(|| {
                        l.split_once(" eyes: ")
                            .map(|(_, f)| format!("eyes: {}", f.trim()))
                    })
            })
            .collect();
        if facts.is_empty() {
            continue;
        }
        out += &format!("\nfrom {}:\n", name(s));
        for f in facts {
            out += &format!("{f}\n");
        }
        break;
    }
    out
}

/// Per frame log (a valley each): CPU and GPU milliseconds a frame from
/// the frame the world was in (`# world in`; every frame if it never
/// was), as `scripts/frames.py` gives them.
pub fn frames_summary(frames: &[PathBuf], scrub: &Scrub) -> String {
    let mut out = String::from("frame times in ms, once the world was in\n\n");
    for f in frames {
        let text = read(f, scrub);
        let map = text
            .lines()
            .find_map(|l| l.strip_prefix("# map "))
            .unwrap_or("?")
            .to_string();
        let from = text.lines().position(|l| l == "# world in").unwrap_or(0);
        let (mut cpu, mut gpu, mut eyes) = (Vec::new(), Vec::new(), Vec::new());
        for l in text.lines().skip(from) {
            let mut it = l.split_whitespace();
            let (Some(_), Some(c), Some(g)) = (it.next(), it.next(), it.next()) else {
                continue;
            };
            let (Ok(c), Ok(g)) = (c.parse::<f64>(), g.parse::<f64>()) else {
                continue;
            };
            cpu.push(c);
            if g >= 0.0 {
                gpu.push(g);
            }
            // The eyes' width as a share of the runtime's (a headset's).
            if let Some(e) = it.next().and_then(|e| e.parse::<f64>().ok()) {
                if e >= 0.0 {
                    eyes.push(e);
                }
            }
        }
        out += &format!("{map} ({})\n", name(f));
        for (what, v) in [("cpu", &mut cpu), ("gpu", &mut gpu)] {
            if v.is_empty() {
                out += &format!("  {what}: no frames\n");
                continue;
            }
            v.sort_by(f64::total_cmp);
            let pct = |q: f64| v[((v.len() as f64 * q) as usize).min(v.len() - 1)];
            out += &format!(
                "  {what}: {} frames, p50 {:.1}, p99 {:.1}, p99.9 {:.1}, worst {:.1}\n",
                v.len(),
                pct(0.5),
                pct(0.99),
                pct(0.999),
                v[v.len() - 1]
            );
        }
        if !eyes.is_empty() {
            eyes.sort_by(f64::total_cmp);
            out += &format!(
                "  eye scale (share of the runtime's width): least {:.2}, p50 {:.2}, most {:.2}\n",
                eyes[0],
                eyes[eyes.len() / 2],
                eyes[eyes.len() - 1]
            );
        }
        out += "\n";
    }
    out
}

/// The newest raid recording under `raids/<map>/`.
fn last_raid(raids: &Path) -> Option<PathBuf> {
    let mut all = Vec::new();
    for map in std::fs::read_dir(raids).ok()?.flatten() {
        for f in std::fs::read_dir(map.path())
            .into_iter()
            .flatten()
            .flatten()
        {
            let p = f.path();
            if p.extension().is_some_and(|x| x == "jsonl") {
                let when = f.metadata().and_then(|m| m.modified()).ok();
                all.push((when, p));
            }
        }
    }
    all.sort();
    all.pop().map(|(_, p)| p)
}

/// The newest `n` files in `dir` named `<prefix>…<suffix>` (their names
/// carry the time, so by name).
fn newest(dir: &Path, prefix: &str, suffix: &str, n: usize) -> Vec<PathBuf> {
    let mut all: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            let n = name(p);
            n.starts_with(prefix) && n.ends_with(suffix)
        })
        .collect();
    all.sort();
    all.reverse();
    all.truncate(n);
    all
}

fn name(p: &Path) -> String {
    p.file_name()
        .map_or(String::new(), |n| n.to_string_lossy().to_string())
}

fn read(p: &Path, scrub: &Scrub) -> String {
    scrub.text(&String::from_utf8_lossy(
        &std::fs::read(p).unwrap_or_default(),
    ))
}

fn stamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// What mustn't leave the machine: the home folder, the user's name and
/// the machine's name, each replaced wherever it appears.
pub struct Scrub {
    /// (what, with what, whole words only): a short machine name is often
    /// inside other words ("vm" in "llvmpipe").
    pairs: Vec<(String, &'static str, bool)>,
}

impl Scrub {
    pub fn from_env() -> Self {
        let var = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
        let host = var("HOSTNAME").or_else(|| var("COMPUTERNAME")).or_else(|| {
            std::fs::read_to_string("/etc/hostname")
                .ok()
                .map(|h| h.trim().to_string())
        });
        Self::new(
            var("HOME").or_else(|| var("USERPROFILE")),
            var("USER").or_else(|| var("USERNAME")),
            host,
        )
    }

    pub fn new(home: Option<String>, user: Option<String>, host: Option<String>) -> Self {
        let mut pairs = Vec::new();
        // The home folder first (it holds the user's name), both slashes.
        if let Some(h) = home.filter(|h| h.len() > 1) {
            pairs.push((h.replace('\\', "/"), "~", false));
            pairs.push((h.replace('/', "\\"), "~", false));
        }
        if let Some(u) = user.filter(|u| u.len() > 1) {
            pairs.push((u, "<user>", true));
        }
        if let Some(h) = host.filter(|h| h.len() > 1) {
            pairs.push((h, "<host>", true));
        }
        // Longest first: a machine named after its user stays `<host>`.
        pairs.sort_by_key(|(from, ..)| std::cmp::Reverse(from.len()));
        Self { pairs }
    }

    pub fn text(&self, s: &str) -> String {
        let mut s = s.to_string();
        for (from, to, word) in &self.pairs {
            s = if *word {
                replace_words(&s, from, to)
            } else {
                s.replace(from.as_str(), to)
            };
        }
        s
    }
}

/// `s` with `from` replaced where it stands as a word of its own (no
/// letter or digit either side).
fn replace_words(s: &str, from: &str, to: &str) -> String {
    let word = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric());
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find(from) {
        let before = rest[..i].chars().next_back();
        let after = rest[i + from.len()..].chars().next();
        out += &rest[..i];
        if word(before) || word(after) {
            out += from;
        } else {
            out += to;
        }
        rest = &rest[i + from.len()..];
    }
    out + rest
}

/// A zip archive, written whole (deflated entries, no extras).
#[derive(Default)]
struct Zip {
    data: Vec<u8>,
    central: Vec<u8>,
    count: u16,
}

impl Zip {
    fn add(&mut self, name: &str, body: &[u8]) {
        let crc = crc32(body);
        let packed = miniz_oxide::deflate::compress_to_vec(body, 6);
        let offset = self.data.len() as u32;
        let header = |sig: u32, central: bool| {
            let mut h = Vec::new();
            h.extend_from_slice(&sig.to_le_bytes());
            if central {
                h.extend_from_slice(&20u16.to_le_bytes()); // made by
            }
            h.extend_from_slice(&20u16.to_le_bytes()); // needed: 2.0
            h.extend_from_slice(&0x0800u16.to_le_bytes()); // UTF-8 names
            h.extend_from_slice(&8u16.to_le_bytes()); // deflate
            h.extend_from_slice(&0u16.to_le_bytes()); // time
            h.extend_from_slice(&0x21u16.to_le_bytes()); // date: 1980-01-01
            h.extend_from_slice(&crc.to_le_bytes());
            h.extend_from_slice(&(packed.len() as u32).to_le_bytes());
            h.extend_from_slice(&(body.len() as u32).to_le_bytes());
            h.extend_from_slice(&(name.len() as u16).to_le_bytes());
            h.extend_from_slice(&0u16.to_le_bytes()); // extra
            if central {
                h.extend_from_slice(&0u16.to_le_bytes()); // comment
                h.extend_from_slice(&0u16.to_le_bytes()); // disk
                h.extend_from_slice(&0u16.to_le_bytes()); // internal attrs
                h.extend_from_slice(&0u32.to_le_bytes()); // external attrs
                h.extend_from_slice(&offset.to_le_bytes());
            }
            h.extend_from_slice(name.as_bytes());
            h
        };
        self.data.extend(header(0x0403_4b50, false));
        self.data.extend(&packed);
        self.central.extend(header(0x0201_4b50, true));
        self.count += 1;
    }

    fn finish(mut self) -> Vec<u8> {
        let at = self.data.len() as u32;
        let size = self.central.len() as u32;
        self.data.extend(&self.central);
        let mut end = Vec::new();
        end.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
        end.extend_from_slice(&[0; 4]); // this disk, the central's disk
        end.extend_from_slice(&self.count.to_le_bytes());
        end.extend_from_slice(&self.count.to_le_bytes());
        end.extend_from_slice(&size.to_le_bytes());
        end.extend_from_slice(&at.to_le_bytes());
        end.extend_from_slice(&0u16.to_le_bytes());
        self.data.extend(end);
        self.data
    }
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &b in bytes {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xEDB8_8320 & (crc & 1).wrapping_neg());
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The entries of a zip this module wrote: (name, contents).
    fn unzip(bytes: &[u8]) -> Vec<(String, String)> {
        let mut out = Vec::new();
        let mut i = 0;
        let u16_at = |i: usize| u16::from_le_bytes([bytes[i], bytes[i + 1]]) as usize;
        let u32_at = |i: usize| u32::from_le_bytes(bytes[i..i + 4].try_into().unwrap());
        while u32_at(i) == 0x0403_4b50 {
            let crc = u32_at(i + 14);
            let packed = u32_at(i + 18) as usize;
            let size = u32_at(i + 22) as usize;
            let n = u16_at(i + 26);
            let extra = u16_at(i + 28);
            let name = String::from_utf8(bytes[i + 30..i + 30 + n].to_vec()).unwrap();
            let start = i + 30 + n + extra;
            let body =
                miniz_oxide::inflate::decompress_to_vec(&bytes[start..start + packed]).unwrap();
            assert_eq!(body.len(), size, "{name}: size");
            assert_eq!(crc32(&body), crc, "{name}: crc");
            out.push((name, String::from_utf8(body).unwrap()));
            i = start + packed;
        }
        assert_eq!(u32_at(i), 0x0201_4b50, "the central directory follows");
        out
    }

    #[test]
    fn crc_is_the_zip_one() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    /// A data folder as a playtest leaves it gives one zip with everything
    /// asked for, the frame times summed per valley from the world being
    /// in, and no home folder, user or machine name anywhere in it.
    #[test]
    fn a_report_holds_what_was_asked_and_nothing_private() {
        let data = std::env::temp_dir().join(format!("extraction-report-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&data);
        let logs = data.join("logs");
        std::fs::create_dir_all(&logs).unwrap();
        let home = "/home/kestrel";
        std::fs::write(
            logs.join("session-100.log"),
            format!(
                "INFO extraction: report: gpu: NVIDIA GeForce RTX 3060 Laptop GPU (Vulkan, driver NVIDIA 550.1)\n\
                 INFO extraction: report: runtime: SteamVR/OpenXR 2.7.4\n\
                 INFO extraction::rig::xr: eyes: 1800x1872 (78% of the runtime's 2312x2400)\n\
                 INFO extraction::render::timeline: load: startup done (zone, terrain, nav, props) at 1.40 s (frame 0)\n\
                 INFO extraction: assets from {home}/games/extraction/assets on kestrel-laptop\n"
            ),
        )
        .unwrap();
        std::fs::write(
            logs.join("crash-100.txt"),
            format!("panicked at {home}/x.rs"),
        )
        .unwrap();
        let mut frames =
            String::from("# map marrow_creek\n1 900.000 -1.000\n2 500.000 -1.000\n# world in\n");
        for f in 3..=1002 {
            let eye = if (500..520).contains(&f) { 0.55 } else { 0.70 };
            frames += &format!("{f} {} {} {eye}\n", if f == 500 { 40.0 } else { 10.0 }, 8.0);
        }
        std::fs::write(logs.join("frames-100-marrow_creek.txt"), frames).unwrap();
        std::fs::write(data.join("settings.ron"), "(msaa: 4)").unwrap();
        let raids = data.join("raids").join("marrow_creek");
        std::fs::create_dir_all(&raids).unwrap();
        std::fs::write(raids.join("200.jsonl"), "{\"t\":0}\n").unwrap();

        let scrub = Scrub::new(
            Some(home.into()),
            Some("kestrel".into()),
            Some("kestrel-laptop".into()),
        );
        // Built as `write_report` builds it, with this machine's names.
        let path = {
            let mut zip = Zip::default();
            let sessions = newest(&logs, "session-", ".log", KEEP);
            zip.add("system.txt", system(&sessions, &scrub).as_bytes());
            let frames = newest(&logs, "frames-", ".txt", KEEP);
            zip.add(
                "frames-summary.txt",
                frames_summary(&frames, &scrub).as_bytes(),
            );
            for s in &sessions {
                zip.add(&format!("logs/{}", name(s)), read(s, &scrub).as_bytes());
            }
            zip.add(
                "logs/crash-100.txt",
                read(&logs.join("crash-100.txt"), &scrub).as_bytes(),
            );
            let p = data.join("t.zip");
            std::fs::write(&p, zip.finish()).unwrap();
            p
        };
        let entries = unzip(&std::fs::read(&path).unwrap());
        let get = |n: &str| {
            entries
                .iter()
                .find(|(e, _)| e == n)
                .unwrap_or_else(|| panic!("no {n}"))
                .1
                .clone()
        };
        let system = get("system.txt");
        assert!(system.contains("build: "));
        assert!(system.contains("gpu: NVIDIA GeForce RTX 3060 Laptop GPU"));
        assert!(system.contains("runtime: SteamVR/OpenXR 2.7.4"));
        assert!(system.contains("eyes: 1800x1872"), "{system}");
        let summary = get("frames-summary.txt");
        assert!(summary.contains("marrow_creek"), "{summary}");
        assert!(
            summary.contains("cpu: 1000 frames, p50 10.0, p99 10.0, p99.9 40.0, worst 40.0"),
            "load frames left out: {summary}"
        );
        assert!(summary.contains("gpu: 1000 frames, p50 8.0"), "{summary}");
        assert!(
            summary.contains(
                "eye scale (share of the runtime's width): least 0.55, p50 0.70, most 0.70"
            ),
            "{summary}"
        );
        for (n, body) in &entries {
            for private in ["kestrel", home] {
                assert!(!body.contains(private), "{n} holds {private}: {body}");
            }
        }
        assert!(get("logs/session-100.log").contains("~/games/extraction/assets on <host>"));
        // A short machine name inside another word stays put.
        let vm = Scrub::new(None, None, Some("vm".into()));
        assert_eq!(vm.text("llvmpipe on vm."), "llvmpipe on <host>.");

        // And the real writer puts everything in the reports folder.
        let real = write_report(&data).unwrap();
        assert!(real.starts_with(data.join("reports")));
        let names: Vec<String> = unzip(&std::fs::read(&real).unwrap())
            .into_iter()
            .map(|(n, _)| n)
            .collect();
        for want in [
            "README.txt",
            "system.txt",
            "frames-summary.txt",
            "load.txt",
            "logs/session-100.log",
            "logs/crash-100.txt",
            "frames/frames-100-marrow_creek.txt",
            "settings.ron",
            "raid/marrow_creek-200.jsonl",
        ] {
            assert!(names.iter().any(|n| n == want), "no {want} in {names:?}");
        }
        let _ = std::fs::remove_dir_all(&data);
    }
}
