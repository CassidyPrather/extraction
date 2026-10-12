//! The game client. See `extraction::launch` for the command line.

use bevy::prelude::*;

fn main() -> AppExit {
    let opts = extraction::LaunchOptions::from_args();
    // `--scenario all` runs each scenario in its own fresh process, so each
    // starts from an untouched world.
    if opts.scenario.as_deref() == Some("all") {
        return run_all_scenarios();
    }
    if let Some(n) = opts.sim {
        return run_sim_batch(&opts, n);
    }
    extraction::run(opts)
}

/// `--sim N`: N bot raids, each in a fresh process (`--sim-run`), a few at
/// a time, with seeds counting up from `--seed` and personalities taking
/// turns (unless `--personality` picks one).
fn run_sim_batch(opts: &extraction::LaunchOptions, n: u32) -> AppExit {
    use extraction::sim::raider::Traits;
    let exe = std::env::current_exe().expect("own executable");
    let dir = opts
        .sim_out
        .clone()
        .unwrap_or_else(|| std::path::PathBuf::from("target/sim").join(&opts.map));
    // The batch owns its directory: old runs would muddle the report.
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.flatten() {
            if e.path().extension().is_some_and(|x| x == "jsonl") {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    let started = std::time::Instant::now();
    let mut queue: std::collections::VecDeque<u32> = (0..n).collect();
    let mut running: Vec<(u32, std::process::Child)> = Vec::new();
    let mut failed = 0;
    while !queue.is_empty() || !running.is_empty() {
        while running.len() < opts.jobs && !queue.is_empty() {
            let i = queue.pop_front().unwrap();
            let seed = opts.seed + i as u64;
            let personality = opts
                .personality
                .clone()
                .unwrap_or_else(|| Traits::NAMES[i as usize % Traits::NAMES.len()].to_string());
            let mut cmd = std::process::Command::new(&exe);
            cmd.args(["--sim-run", &format!("run{i:03}")])
                .args(["--map", &opts.map])
                .args(["--seed", &seed.to_string()])
                .args(["--personality", &personality])
                .arg("--sim-out")
                .arg(&dir);
            if let Some(m) = opts.sim_minutes {
                cmd.args(["--sim-minutes", &m.to_string()]);
            }
            if std::env::var_os("RUST_LOG").is_none() {
                cmd.env("RUST_LOG", "warn");
            }
            match cmd.spawn() {
                Ok(child) => running.push((i, child)),
                Err(e) => {
                    eprintln!("sim run {i}: {e}");
                    failed += 1;
                }
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
        running.retain_mut(|(i, child)| match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    eprintln!("sim run {i} failed: {status}");
                    failed += 1;
                }
                false
            }
            Ok(None) => true,
            Err(_) => false,
        });
    }
    println!(
        "sim: {} raids on {} in {:.0} s -> {} (report: scripts/sim-report.sh {})",
        n,
        opts.map,
        started.elapsed().as_secs_f32(),
        dir.display(),
        opts.map
    );
    if failed == 0 {
        AppExit::Success
    } else {
        AppExit::error()
    }
}

fn run_all_scenarios() -> AppExit {
    let exe = std::env::current_exe().expect("own executable");
    let passthrough: Vec<String> = std::env::args()
        .skip(1)
        .scan(false, |skip, a| {
            let keep = !*skip && a != "--scenario";
            *skip = a == "--scenario";
            Some(keep.then_some(a))
        })
        .flatten()
        .collect();
    let mut failed = Vec::new();
    for name in extraction::bot::scenarios::NAMES {
        let status = std::process::Command::new(&exe)
            .args(&passthrough)
            .args(["--scenario", name])
            .status();
        if !status.is_ok_and(|s| s.success()) {
            failed.push(*name);
        }
    }
    if failed.is_empty() {
        println!(
            "all {} scenarios OK",
            extraction::bot::scenarios::NAMES.len()
        );
        AppExit::Success
    } else {
        println!("FAILED scenarios: {failed:?}");
        AppExit::error()
    }
}
