#!/usr/bin/env python3
"""Turn `mariadbd --help --verbose` into backend/src/glpi/innodb_help.rs.

    mariadbd --help --verbose | scripts/innodb-help.py > backend/src/glpi/innodb_help.rs

WHY GENERATE IT. The InnoDB panel puts an info button on every setting, and
the text behind it has to come from somewhere. Writing 135 descriptions here
would make them this app's claims about a server it does not ship, wrong in
whatever way the next MariaDB changes and with nothing to notice. The server's
own binary already holds the sentence; this copies it.

THE PARSE. Option blocks look like one of three shapes, and all three occur:

      --innodb-buffer-pool-size=#
                          The size of the memory buffer InnoDB uses to cache
                          data and indexes of its tables.
      --innodb-flush-sync Allow IO bursts at the checkpoints ignoring
                          io_capacity setting.
      --innodb-log-file-mmap[=#]
                          Whether to use memory-mapped I/O ...

So: a name, an optional `=#` or `[=#]`, then the description either on the
same line or on the following lines indented past column 20. Getting the
trailing-space case wrong is what makes a run report "described: 2 of 137" -
the regex has to allow whitespace after `=#` before the line ends.

WHAT IT DOES NOT DO. It never invents an entry. Two variables this server
reports - innodb_lru_flush_size and innodb_purge_rseg_truncate_frequency -
have no option help at all, so they are simply absent from the table and the
lookup answers None for them.
"""

import re
import sys
import textwrap

OPTION = re.compile(r"^  --([a-z0-9-]+)(=\S*|\[=\S*\])?\s*(\S.*)?$")
CONTINUATION = re.compile(r"^\s{20,}\S")


def parse(lines):
    """Option name (underscored) -> description, for innodb_* options only."""
    opts, name, buf = {}, None, []
    for line in lines:
        m = OPTION.match(line.rstrip("\n"))
        if m:
            if name:
                opts[name] = " ".join(buf).strip()
            name = m.group(1).replace("-", "_")
            buf = [m.group(3)] if m.group(3) else []
        elif name is not None and CONTINUATION.match(line):
            buf.append(line.strip())
    if name:
        opts[name] = " ".join(buf).strip()
    return {
        k: re.sub(r"\s+", " ", v)
        for k, v in opts.items()
        if k.startswith("innodb_") and v.strip()
    }


def literal(text, indent):
    """A Rust string literal, wrapped with \\ continuations past one line.

    The space before each backslash is load-bearing: Rust strips the newline
    and the next line's leading whitespace, so without it the words either
    side would be glued together.
    """
    text = text.replace("\\", "\\\\").replace('"', '\\"')
    room = 84 - len(indent)
    if len(text) <= room + 4:
        return f'"{text}"'
    parts = textwrap.wrap(text, width=room, break_long_words=False)
    out = '"' + parts[0] + " \\\n"
    for part in parts[1:-1]:
        out += indent + "     " + part + " \\\n"
    return out + indent + "     " + parts[-1] + '"'


HEADER = '''//! What each InnoDB setting is, in the server's own words.
//!
//! GENERATED, not written - by scripts/innodb-help.py, from
//! `mariadbd --help --verbose` on MariaDB 11.4.12. Every description below is
//! the server's own, reflowed and nothing else: no paraphrase, no manual, no
//! text of this app's. Same reason the engine page read
//! `information_schema.ENGINES.COMMENT` rather than quoting documentation - a
//! sentence written here would be this app's claim about a server it does not
//! ship, and would rot at the next upgrade with nothing to notice.
//!
//! To regenerate after a MariaDB upgrade:
//!
//! ```sh
//! mariadbd --help --verbose | scripts/innodb-help.py > backend/src/glpi/innodb_help.rs
//! ```
//!
//! THE COUNT DOES NOT MATCH THE PANEL'S, on purpose. This server reports 137
//! `innodb_%` system variables; the binary documents 165 `--innodb-*`
//! options. 135 names are in both. The other 30 here are information_schema
//! plugins - `innodb_trx`, `innodb_locks`, `innodb_cmp` and friends - which
//! are loadable plugins rather than settings, so nothing ever looks them up.
//! They are kept because filtering them would mean this file is no longer
//! simply what the command printed, and that property is the whole point.
//!
//! The two variables going the other way are the server's doing:
//! `innodb_lru_flush_size` and `innodb_purge_rseg_truncate_frequency` are
//! reported as variables but have no option help at all, so there is nothing
//! to quote. `innodb_help` answers None for them and the panel says as much
//! rather than inventing a sentence.

/// Variable name -> the server's own description. Sorted by name, which is
/// also the order `SHOW VARIABLES` returns them in.
pub const INNODB_HELP: [(&str, &str); {count}] = [
{body}
];

/// The server's description of a setting, if its own binary printed one.
pub fn innodb_help(name: &str) -> Option<&'static str> {
    INNODB_HELP
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, help)| *help)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A regeneration that goes wrong grows duplicates, and a duplicate here
    /// is invisible: the lookup finds the first and the second never shows.
    #[test]
    fn every_name_appears_once() {
        let mut seen = std::collections::HashSet::new();
        for (name, _) in INNODB_HELP {
            assert!(seen.insert(name), "{name} is listed twice");
        }
    }

    /// Sorted, so the diff after a MariaDB upgrade is readable: added and
    /// removed settings stay put instead of the whole file shifting.
    #[test]
    fn the_list_is_sorted() {
        let given: Vec<&str> = INNODB_HELP.iter().map(|(n, _)| *n).collect();
        let mut sorted = given.clone();
        sorted.sort_unstable();
        assert_eq!(given, sorted);
    }

    /// An empty entry would render an info button that opens on a blank
    /// panel, which is worse than having no button.
    #[test]
    fn no_entry_is_empty() {
        for (name, help) in INNODB_HELP {
            assert!(!help.trim().is_empty(), "{name} has no description");
        }
    }

    #[test]
    fn the_two_the_server_does_not_describe_are_absent() {
        assert_eq!(innodb_help("innodb_lru_flush_size"), None);
        assert_eq!(innodb_help("innodb_purge_rseg_truncate_frequency"), None);
        assert!(innodb_help("innodb_buffer_pool_size").is_some());
    }
}
'''


def main():
    opts = parse(sys.stdin)
    if len(opts) < 100:
        sys.exit(f"parsed only {len(opts)} innodb options - the help format moved")
    body = "\n".join(
        f'    (\n        "{name}",\n        {literal(opts[name], "        ")},\n    ),'
        for name in sorted(opts)
    )
    sys.stdout.write(
        HEADER.replace("{count}", str(len(opts)))
        .replace("{body}", body)
        .replace("{name} is listed twice", "{name} is listed twice")
    )


if __name__ == "__main__":
    main()
