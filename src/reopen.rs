//! The game opening itself again (the truck drove to another valley, a new
//! career, a playtest tour's next station) without leaving the player's
//! terminal behind.
//!
//! A game that spawned its successor and exited handed the terminal back at
//! once, while the successor went on writing its log to that terminal. When
//! the successor closed there was no fresh prompt, and the process looked
//! hung (playtest 2026-11-22: "the process doesn't terminate when the window
//! closes", the yard opened by NEW CAREER).
//!
//! The process the player started now opens each successor and waits for
//! it. A successor that wants to open the game again doesn't spawn a game of
//! its own: it leaves the arguments in the handoff file named by
//! [`HANDOFF`] and exits, and the first process opens the next one. There
//! are never more than two processes, and the terminal comes back when the
//! last game closes, with that game's exit code.

use std::io;
use std::path::{Path, PathBuf};

/// Set on a game opened by a waiting first process: the file in which to
/// leave the arguments for the next one.
pub const HANDOFF: &str = "EXTRACTION_REOPEN_HANDOFF";

/// Where this process leaves its successor's arguments, if a first process
/// is waiting for it.
pub fn handoff() -> Option<PathBuf> {
    std::env::var_os(HANDOFF).map(PathBuf::from)
}

/// A handoff file for a first process (one per process, in the temp folder).
pub fn handoff_path() -> PathBuf {
    std::env::temp_dir().join(format!("extraction-reopen-{}", std::process::id()))
}

/// Leave the next game's arguments for the waiting first process.
pub fn leave(path: &Path, args: &[String]) -> io::Result<()> {
    std::fs::write(path, args.join("\0"))
}

/// The arguments a game left, if it left any; the file is removed.
pub fn take(path: &Path) -> Option<Vec<String>> {
    let text = std::fs::read_to_string(path).ok()?;
    let _ = std::fs::remove_file(path);
    Some(if text.is_empty() {
        Vec::new()
    } else {
        text.split('\0').map(str::to_owned).collect()
    })
}

/// The first process: open the game with `first`, wait for it, and open it
/// again for as long as each one leaves arguments in `handoff`. Returns the
/// last game's exit code. `open` runs one game to its end and returns its
/// exit code (`None`: ended by a signal).
pub fn supervise(
    first: Vec<String>,
    handoff: &Path,
    mut open: impl FnMut(&[String]) -> io::Result<Option<i32>>,
) -> u8 {
    let _ = std::fs::remove_file(handoff);
    let mut args = first;
    loop {
        let code = match open(&args) {
            Ok(code) => code.unwrap_or(1),
            Err(e) => {
                eprintln!("couldn't reopen the game: {e}");
                let _ = std::fs::remove_file(handoff);
                return 1;
            }
        };
        match take(handoff) {
            Some(next) => args = next,
            None => return code.clamp(0, 255) as u8,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "extraction-reopen-test-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&dir);
        dir
    }

    /// The first process waits for every game opened after it, in turn, and
    /// ends with the last one; the terminal isn't handed back while a game
    /// it started is still running.
    #[test]
    fn the_first_process_waits_for_each_reopened_game() {
        let path = scratch("chain");
        let mut opened: Vec<Vec<String>> = Vec::new();
        let code = supervise(vec!["--map".into(), "camp".into()], &path, |args| {
            opened.push(args.to_vec());
            // The yard asks for a valley, the valley for nothing more.
            match opened.len() {
                1 => leave(&path, &["--map".into(), "training_yard".into()])?,
                2 => leave(
                    &path,
                    &[
                        "--map".into(),
                        "marrow_creek".into(),
                        "--vignette".into(),
                        "0.3 x".into(),
                    ],
                )?,
                _ => {}
            }
            Ok(Some(if opened.len() == 3 { 3 } else { 0 }))
        });
        assert_eq!(code, 3, "the last game's exit code is the terminal's");
        assert_eq!(
            opened,
            vec![
                vec!["--map".to_owned(), "camp".to_owned()],
                vec!["--map".to_owned(), "training_yard".to_owned()],
                vec![
                    "--map".to_owned(),
                    "marrow_creek".to_owned(),
                    "--vignette".to_owned(),
                    "0.3 x".to_owned()
                ],
            ]
        );
        assert!(!path.exists(), "no handoff left behind");
    }

    /// A stale handoff from an earlier run isn't taken as a request.
    #[test]
    fn a_stale_handoff_is_not_a_request() {
        let path = scratch("stale");
        leave(&path, &["--map".into(), "kettle_pass".into()]).unwrap();
        let mut runs = 0;
        let code = supervise(vec![], &path, |_| {
            runs += 1;
            Ok(Some(0))
        });
        assert_eq!((runs, code), (1, 0));
    }

    /// A game that couldn't be started ends the first process, with an error.
    #[test]
    fn a_game_that_wont_start_ends_the_wait() {
        let path = scratch("fail");
        let code = supervise(vec![], &path, |_| Err(io::Error::other("no exe")));
        assert_eq!(code, 1);
    }
}
