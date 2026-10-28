//! Embed a `git describe`-based version at compile time.

use std::env;
use std::process::Command;

fn main() {
    // Rebuild when the checked-out commit or the tags change: HEAD, the
    // branch it points at, and the packed refs a pull or gc may move it to.
    // Not `.git/index`: `git describe --dirty` refreshes the index itself,
    // so watching it made every build stale the next (cargo run after cargo
    // build recompiled the whole game). A build's "-dirty" is as of when
    // the commit last changed.
    println!("cargo::rerun-if-changed=.git/HEAD");
    println!("cargo::rerun-if-changed=.git/packed-refs");
    println!("cargo::rerun-if-changed=.git/refs/tags");
    if let Some(branch) = std::fs::read_to_string(".git/HEAD")
        .ok()
        .and_then(|head| head.strip_prefix("ref: ").map(|r| r.trim().to_string()))
    {
        println!("cargo::rerun-if-changed=.git/{branch}");
    }

    let pkg_version = env::var("CARGO_PKG_VERSION").expect("cargo always sets CARGO_PKG_VERSION");
    let version = git_describe().map_or_else(
        || pkg_version.clone(),
        |describe| {
            let describe = describe.strip_prefix('v').unwrap_or(&describe);
            if describe.contains('.') {
                // Tag-based: "1.2.3" or "1.2.3-4-gabc1234[-dirty]"
                describe.to_string()
            } else {
                format!("{pkg_version}+g{describe}")
            }
        },
    );
    println!("cargo::rustc-env=GIT_DESCRIBE_VERSION={version}");
}

fn git_describe() -> Option<String> {
    let output = Command::new("git")
        .args(["describe", "--tags", "--always", "--dirty"])
        .output()
        .ok()
        .filter(|output| output.status.success())?;
    let describe = String::from_utf8(output.stdout).ok()?;
    let describe = describe.trim();
    if describe.is_empty() {
        None
    } else {
        Some(describe.to_string())
    }
}
