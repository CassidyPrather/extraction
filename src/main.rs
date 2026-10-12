//! The game client. See `extraction::launch` for the command line.

use bevy::prelude::*;

fn main() -> AppExit {
    let opts = extraction::LaunchOptions::from_args();
    // `--scenario all` runs each scenario in its own fresh process, so each
    // starts from an untouched world.
    if opts.scenario.as_deref() == Some("all") {
        return run_all_scenarios();
    }
    extraction::run(opts)
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
