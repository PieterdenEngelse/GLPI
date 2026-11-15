//! Running scripts/db-restart.sh, and nothing else.
//!
//! A module of its own because the decisions are all in the script: which OS
//! this is, which unit to restart, whether the config file can be written at
//! all. None of that belongs in Rust - it is shell work, it differs per
//! operating system, and it is the kind of thing that has to be readable by
//! whoever is about to let it restart their database. This file only finds
//! the script, runs it, and hands back what it said.
//!
//! WHY THE OUTPUT IS RETURNED VERBATIM. On NixOS the "settings" mode cannot
//! do anything at all - /etc/my.cnf is a read-only store path - so what it
//! produces is instructions: the exact services.mysql.settings block to
//! paste, and the list of settings that need a restart in the first place.
//! Paraphrasing that in the UI would mean maintaining the same text twice,
//! and the script is the half that can be run by hand.
use std::path::PathBuf;

use serde::Serialize;

/// What a run said. `code` is the script's own exit status, and the UI shows
/// each one differently: 0 done, 2 misuse, 3 not possible on this OS, 4 the
/// attempt failed and was rolled back.
#[derive(Debug, Serialize)]
pub struct Run {
    pub code: i32,
    pub output: String,
    /// Which script ran, so the page can name it rather than implying the
    /// backend did this itself.
    pub script: String,
    /// True when DRY_RUN was set: nothing ran, the commands were printed.
    pub preview: bool,
}

/// Where the script is.
///
/// Beside the installed binary first, mirroring how the web bundle is found -
/// `just deploy` copies both - then the repo's own copy, which is what a
/// `cargo run` from the tree uses. GLPI_INV_DB_SCRIPT overrides for a
/// deployment that puts it elsewhere.
pub fn script_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("GLPI_INV_DB_SCRIPT") {
        let p = PathBuf::from(p);
        return p.is_file().then_some(p);
    }
    let beside = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|d| d.join("db-restart.sh")));
    if let Some(p) = beside {
        if p.is_file() {
            return Some(p);
        }
    }
    let in_repo = PathBuf::from("scripts/db-restart.sh");
    in_repo.is_file().then_some(in_repo)
}

/// Run it. `settings` is empty for the plain restart.
///
/// stdout and stderr are merged, because the script writes its reasoning to
/// stdout and the tools it calls write theirs to stderr - a reader needs them
/// interleaved, in the order they happened, not in two panes.
pub async fn run(mode: &str, settings: &[String], preview: bool) -> Result<Run, String> {
    let script = script_path().ok_or_else(|| {
        "scripts/db-restart.sh was not found beside the binary or in the working \
         directory. `just deploy` installs it; set GLPI_INV_DB_SCRIPT to point at \
         it instead."
            .to_string()
    })?;
    run_script(&script, mode, settings, preview).await
}

/// The same, with the script named outright.
///
/// Separate so the tests do not depend on the working directory: `cargo test`
/// runs with backend/ as the cwd, where the repo-relative fallback above does
/// not resolve, and a test that silently skipped for that reason would be
/// worse than no test.
async fn run_script(
    script: &std::path::Path,
    mode: &str,
    settings: &[String],
    preview: bool,
) -> Result<Run, String> {
    let mut cmd = tokio::process::Command::new(script);
    cmd.arg(mode);
    cmd.args(settings);
    if preview {
        cmd.env("DRY_RUN", "1");
    }
    // Merged in the child, so the ordering is the child's rather than this
    // process's reading order.
    cmd.stderr(std::process::Stdio::piped());
    cmd.stdout(std::process::Stdio::piped());

    let out = cmd
        .output()
        .await
        .map_err(|e| format!("could not run {}: {e}", script.display()))?;

    let mut output = String::from_utf8_lossy(&out.stdout).into_owned();
    let err = String::from_utf8_lossy(&out.stderr);
    if !err.trim().is_empty() {
        output.push_str(&err);
    }

    Ok(Run {
        // 128+signal if it was killed, which the script cannot report itself.
        code: out.status.code().unwrap_or(-1),
        output,
        script: script.display().to_string(),
        preview,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The repo's copy, found from this crate's own directory rather than
    /// from wherever the test runner happens to be.
    fn repo_script() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../scripts/db-restart.sh")
    }

    async fn run_repo(mode: &str, settings: &[String], preview: bool) -> Run {
        run_script(&repo_script(), mode, settings, preview)
            .await
            .expect("ran")
    }

    /// Proves the file is there and executable - a chmod lost in a commit
    /// fails here rather than at the first click.
    #[test]
    fn the_script_is_in_the_repo_and_runnable() {
        let p = repo_script();
        assert!(p.is_file(), "{} is missing", p.display());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&p).expect("stat").permissions().mode();
            assert!(mode & 0o111 != 0, "{} is not executable", p.display());
        }
    }

    /// Detection is the script's first job and the only one that is safe to
    /// run in a test: it reads /etc/os-release and prints a word.
    #[tokio::test]
    async fn detect_names_an_os() {
        let r = run_repo("detect", &[], false).await;
        assert_eq!(r.code, 0);
        assert!(
            ["nixos", "linux", "windows", "darwin"].contains(&r.output.trim()),
            "unexpected OS: {:?}",
            r.output
        );
    }

    /// A mode the script does not have must not be treated as a restart.
    #[tokio::test]
    async fn an_unknown_mode_is_misuse_not_a_restart() {
        let r = run_repo("wat", &[], false).await;
        assert_eq!(r.code, 2);
        assert!(r.output.contains("usage:"));
    }

    /// The settings the script refuses outright are refused before it looks
    /// at the OS, so this holds on any host the tests run on.
    #[tokio::test]
    async fn page_size_is_refused() {
        let r = run_repo("settings", &["innodb_page_size=32K".into()], true).await;
        assert_eq!(r.code, 2);
        assert!(r.output.contains("Refusing innodb_page_size"));
    }
}
