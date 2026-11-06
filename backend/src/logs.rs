//! Reading two of GLPI's own log files off disk.
//!
//! This is a FOURTH way into GLPI, and the smallest one deliberately. The
//! other three reach GLPI through its APIs or its database; this reads files
//! GLPI wrote, behind GLPI's back. That is a real cost and the reason the
//! surface is kept to an allowlist of two names rather than a path parameter.
//!
//! Why these two of the ten GLPI checks it can write
//! (`LogsWriteAccess::LOG_FILES`), both verified by finding the code that
//! writes them rather than inferred from the filename:
//!
//!   - `access-errors.log` - `AccessLogHandler::isHandling` returns false for
//!     anything that is not an `AccessDeniedHttpException`, so this file is
//!     *only* refusals. That is exactly what an agent gets when inventory is
//!     disabled or a right is missing, which makes it the most on-subject log
//!     GLPI keeps.
//!   - `php-errors.log` - `ErrorLogHandler`, GLPI's errors and uncaught
//!     exceptions. Where a 500 from either API lands, and in GLPI 12 where
//!     SQL failures surface too; there is no separate sql-errors.log.
//!
//! `api.log` was the first choice here and was WRONG. It is written from
//! exactly one place, `Glpi/Api/API.php:2406` in the LEGACY v1 API, by
//! `logEndpointUsage()` - and only when the calling API client record has
//! `dolog_method = DOLOG_LOGS`, which is not the default
//! (`dolog_method tinyint NOT NULL DEFAULT '0'` = `DOLOG_DISABLED`). It also
//! records *successful* endpoint usage, not refusals. Three reasons it could
//! not do the job it was picked for.
//!
//! The remaining eight are mail, cron, events and webhooks. GLPI's own viewer
//! is a better place to read them.
//!
//! Why not through GLPI instead, all checked in the 12.0.1 source:
//!   - `system_logs`, `LogViewer` and `LogParser` have ZERO occurrences under
//!     src/Glpi/Api/. Neither v2.3 nor v1 exposes logs at all.
//!   - `ajax/logviewer.php` returns HTML, not JSON, and requires the
//!     `system_logs` right - which would have to be added to this app's
//!     service account, widening a stolen token from "agents and computers"
//!     to "every log on the server".
//!   - `front/logs.php` additionally calls
//!     `LogViewer::checkReAuthenticationOrRedirect()`. GLPI demands a fresh
//!     password for log access on purpose; a bearer token cannot satisfy it
//!     and should not try.
//!
//! Reading the files needs the directory to be readable by this process,
//! which it is not by default: GLPI_LOG_DIR is `/var/lib/glpi/files/_log`
//! and `/var/lib/glpi/files` is `0750 glpi:glpi`, while this backend runs as
//! an ordinary user. Hence `State::Unreadable`, which carries the OS error
//! rather than pretending the file is empty.
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use serde::Serialize;

/// The only names this module will open. Not a prefix check and not a
/// sanitised path parameter: an allowlist, so no request can describe a file
/// outside it however it is spelled.
pub const ALLOWED: [&str; 2] = ["access-errors.log", "php-errors.log"];

/// How much of the tail to read. Bounded in BYTES, not lines: a single log
/// line can be a whole stack trace, so a line budget is not a memory budget.
const TAIL_BYTES: u64 = 256 * 1024;

/// How many lines to return from within that window.
const TAIL_LINES: usize = 300;

#[derive(Serialize)]
#[serde(rename_all = "snake_case", tag = "state", content = "detail")]
pub enum State {
    /// Read, and it had content.
    Ok,
    /// The file is not there. GLPI creates each log on first write, so this
    /// is the normal state for a log with nothing to say - distinct from
    /// Unreadable, and the distinction is the point of this enum.
    Absent,
    /// Present and zero-length.
    Empty,
    /// Could not be opened or read. Carries the OS error, because
    /// "permission denied" and "no such file" lead to different fixes.
    Unreadable(String),
}

#[derive(Serialize)]
pub struct Tail {
    pub file: String,
    #[serde(flatten)]
    pub state: State,
    pub lines: Vec<String>,
    /// Total size on disk, so the UI can say the tail is a tail.
    pub bytes: u64,
    /// True when bytes exceeded the window and earlier content was skipped.
    pub truncated: bool,
}

/// One file found in the directory. Metadata only - listing what exists is a
/// different privilege from reading what it says, and the UI needs the first
/// for every log and the second for two of them.
#[derive(Serialize)]
pub struct Entry {
    pub file: String,
    pub bytes: u64,
    /// Whether `tail` will serve this one. Sent rather than inferred, so the
    /// UI never has to keep its own copy of ALLOWED.
    pub readable: bool,
}

pub struct LogDir {
    root: PathBuf,
}

impl LogDir {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Every `*.log` in the directory, with its size.
    ///
    /// Enumerated rather than hardcoded, and that is the point: GLPI does not
    /// define a fixed set either. `LogParser::__construct` takes
    /// `GLPI_LOG_DIR` and `scandir`s it against `/^(.+)\.log$/`, so the real
    /// set is whatever has had something to say. An earlier version of this
    /// module listed eight names taken from the GLPI source; the install had
    /// fifteen files, because `Toolbox::logInFile()` is called with computed
    /// channel names in several places and plugins can add their own. A list
    /// derived from source can only ever be a lower bound.
    pub fn list(&self) -> Result<Vec<Entry>, String> {
        let mut out = Vec::new();
        for entry in std::fs::read_dir(&self.root).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if !name.ends_with(".log") {
                continue;
            }
            // A directory named *.log is not a log. Skipped rather than
            // listed with a size of zero, which would read as an empty file.
            let Ok(meta) = entry.metadata() else { continue };
            if !meta.is_file() {
                continue;
            }
            out.push(Entry {
                bytes: meta.len(),
                readable: ALLOWED.contains(&name.as_str()),
                file: name,
            });
        }
        // Name order, not directory order: readdir order is arbitrary and the
        // tab strip must not reshuffle itself between requests.
        out.sort_by(|a, b| a.file.cmp(&b.file));
        Ok(out)
    }

    /// Read the tail of one allowed log.
    ///
    /// Never fails: every outcome is a `State` the UI can render, because an
    /// unreadable log is a fact about the deployment rather than an error in
    /// this request.
    pub fn tail(&self, file: &str) -> Tail {
        if !ALLOWED.contains(&file) {
            return Tail {
                file: file.to_string(),
                state: State::Unreadable("not an allowed log name".into()),
                lines: Vec::new(),
                bytes: 0,
                truncated: false,
            };
        }

        let path = self.root.join(file);
        match read_tail(&path) {
            Ok((lines, bytes, truncated)) => {
                let state = if bytes == 0 { State::Empty } else { State::Ok };
                Tail {
                    file: file.to_string(),
                    state,
                    lines,
                    bytes,
                    truncated,
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Tail {
                file: file.to_string(),
                state: State::Absent,
                lines: Vec::new(),
                bytes: 0,
                truncated: false,
            },
            Err(e) => Tail {
                file: file.to_string(),
                state: State::Unreadable(e.to_string()),
                lines: Vec::new(),
                bytes: 0,
                truncated: false,
            },
        }
    }
}

/// Seek to the end and read back a bounded window.
///
/// Reading the whole file would be simpler and is wrong: php-errors.log on a
/// busy install reaches hundreds of megabytes, and this process would hold
/// all of it to show the last screen.
fn read_tail(path: &Path) -> std::io::Result<(Vec<String>, u64, bool)> {
    let mut f = std::fs::File::open(path)?;
    let bytes = f.metadata()?.len();

    if bytes == 0 {
        return Ok((Vec::new(), 0, false));
    }

    let window = bytes.min(TAIL_BYTES);
    let truncated = window < bytes;
    f.seek(SeekFrom::Start(bytes - window))?;

    let mut buf = Vec::with_capacity(window as usize);
    f.take(window).read_to_end(&mut buf)?;

    // Lossy on purpose: a log can hold a byte sequence that is not UTF-8 -
    // a mangled filename, a truncated multi-byte character at the window
    // edge - and refusing to show the whole tail over one bad byte would be
    // the wrong trade for a diagnostic view.
    let text = String::from_utf8_lossy(&buf);

    let mut lines: Vec<&str> = text.lines().collect();

    // The first line is a fragment whenever the window started mid-file.
    if truncated && !lines.is_empty() {
        lines.remove(0);
    }

    let start = lines.len().saturating_sub(TAIL_LINES);
    let lines = lines[start..].iter().map(|s| s.to_string()).collect();

    Ok((lines, bytes, truncated))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, name: &str, body: &str) {
        std::fs::write(dir.join(name), body).unwrap();
    }

    /// Per-test directory, not one shared per process. Tests run in parallel
    /// and two of these write the same filename, so a shared directory makes
    /// them race each other.
    fn tmpdir(test: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("glpi-inv-logs-{}-{test}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn allowlist_refuses_anything_else() {
        let d = LogDir::new("/nonexistent");
        // The traversal attempt matters more than the name: it must be
        // refused by the allowlist before any path is built.
        for name in ["api.log", "../config/config_db.php", "/etc/shadow"] {
            assert!(
                matches!(d.tail(name).state, State::Unreadable(_)),
                "{name} should not be openable"
            );
        }
    }

    #[test]
    fn absent_is_not_empty() {
        let d = tmpdir("absent");
        assert!(matches!(
            LogDir::new(&d).tail("php-errors.log").state,
            State::Absent
        ));

        write(&d, "php-errors.log", "");
        assert!(matches!(
            LogDir::new(&d).tail("php-errors.log").state,
            State::Empty
        ));
    }

    #[test]
    fn tail_caps_lines_and_reports_truncation() {
        let d = tmpdir("truncation");
        // Must exceed TAIL_BYTES, and these lines are ~11 bytes each - 20_000
        // of them is only ~218 KB, which is UNDER the 256 KB window and was
        // the first version of this test asserting the wrong thing.
        let body: String = (0..40_000).map(|i| format!("line {i}\n")).collect();
        write(&d, "access-errors.log", &body);

        let t = LogDir::new(&d).tail("access-errors.log");
        assert!(matches!(t.state, State::Ok));
        assert!(t.truncated, "a file larger than the window is truncated");
        assert_eq!(t.lines.len(), TAIL_LINES);
        assert_eq!(t.bytes, body.len() as u64);

        // The last line of the file must survive: a tail that loses the most
        // recent line is worse than no tail at all.
        assert_eq!(t.lines.last().unwrap(), "line 39999");
    }

    #[test]
    fn short_file_is_not_truncated_and_keeps_its_first_line() {
        let d = tmpdir("short");
        write(&d, "php-errors.log", "first\nsecond\nthird\n");

        let t = LogDir::new(&d).tail("php-errors.log");
        assert!(!t.truncated);
        // The partial-line drop must NOT apply here - there is no partial
        // line when the window covers the whole file.
        assert_eq!(t.lines, vec!["first", "second", "third"]);
    }
}
