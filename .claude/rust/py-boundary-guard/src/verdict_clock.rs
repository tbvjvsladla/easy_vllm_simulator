//! The clock reads of tracked harness sources and their verdict against `verdict_clock_allowlist.tsv`.
//!
//! An allowlist row = `<repo path>\t<read text>\t<count>\t<class>\t<reason>`: that file holds exactly `count` reads
//! with that text. There is no per-file permission:
//! - a read with no row, or beyond the count of its row, is a violation (`CLOCK-VIOLATION`);
//! - a row with fewer reads than its count (none at all included) is stale (`CLOCK-STALE`).
//!
//! `class` is `runtime-fact` (the runtime measures real elapsed time or an outside fact: timeouts, watchdogs, leases,
//! certificate validity), `record-only` (a time stamped into a record, a log or a name, never into a verdict),
//! or `not-time` (a name or a never-executed string that matches a rule but reads no time).
//!
//! Every tracked `.rs`, `.ts` and shell source is read (a shell source: `.sh`, `.bash`, or any other name whose
//! shebang interpreter is a shell), test fixtures that tests run included. Two frozen data planes are not: the parity
//! records `.claude/rust/parity/data/` (recorded outputs, never executed) and this lint's own planted reads
//! `.claude/rust/py-boundary-guard/checks/data/verdict_clock/` (checked by `--self-test`).

pub mod rust_reads;
pub mod selftest;
pub mod shell_reads;
pub mod ts_reads;

use std::collections::BTreeMap;

pub const DEFAULT_ALLOWLIST: &str = ".claude/rust/py-boundary-guard/verdict_clock_allowlist.tsv";
pub const CLASSES: [&str; 3] = ["runtime-fact", "record-only", "not-time"];

pub const DATA_PLANES: [&str; 2] = [".claude/rust/parity/data/", ".claude/rust/py-boundary-guard/checks/data/verdict_clock/"];
const PERIOD_UNITS: [&str; 3] = ["DAYS", "WEEKS", "MONTHS"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finding {
    pub line: usize,
    pub text: String,
}

pub fn is_period_name(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    PERIOD_UNITS.iter().any(|unit| upper.ends_with(&format!("_{unit}")) && upper.len() > unit.len() + 1)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    Rust,
    Shell,
    TypeScript,
}

pub fn in_data_plane(path: &str) -> bool {
    DATA_PLANES.iter().any(|plane| path.starts_with(plane))
}

/// The surface a tracked path has by its name alone; `None` leaves the shebang to decide.
pub fn surface_by_name(path: &str) -> Option<Surface> {
    let name = path.rsplit('/').next().unwrap_or(path);
    if name.ends_with(".rs") {
        Some(Surface::Rust)
    } else if name.ends_with(".ts") {
        Some(Surface::TypeScript)
    } else if name.ends_with(".sh") || name.ends_with(".bash") {
        Some(Surface::Shell)
    } else {
        None
    }
}

/// The first line is `#!<interpreter> …` and the interpreter (through `env`, its options and assignments skipped)
/// is a shell (its name ends in `sh`).
pub fn is_shell_script(head: &[u8]) -> bool {
    let Some(first) = head.split(|b| *b == b'\n').next().and_then(|l| l.strip_prefix(b"#!")) else { return false };
    let first = String::from_utf8_lossy(first.strip_suffix(b"\r").unwrap_or(first)).into_owned();
    let mut words = first.split_whitespace();
    let Some(mut interpreter) = words.next().map(|w| w.rsplit('/').next().unwrap_or(w)) else { return false };
    if interpreter == "env" {
        let mut skip_value = false;
        interpreter = "";
        for word in words {
            if std::mem::take(&mut skip_value) || word.contains('=') {
                continue;
            }
            if let Some(option) = word.strip_prefix('-') {
                skip_value = matches!(option, "u" | "-unset" | "C" | "-chdir");
                continue;
            }
            interpreter = word.rsplit('/').next().unwrap_or(word);
            break;
        }
    }
    interpreter.ends_with("sh")
}

pub fn find(surface: Surface, source: &str) -> Result<Vec<Finding>, String> {
    match surface {
        Surface::Rust => rust_reads::find(source),
        Surface::TypeScript => Ok(ts_reads::find(source)),
        Surface::Shell => Ok(source
            .lines()
            .enumerate()
            .flat_map(|(index, line)| {
                shell_reads::line_reads(line)
                    .into_iter()
                    .map(move |text| Finding { line: index + 1, text })
            })
            .collect()),
    }
}

pub struct Row {
    pub path: String,
    pub text: String,
    pub count: usize,
}

pub fn parse_allowlist(text: &str) -> Result<Vec<Row>, String> {
    let mut rows: Vec<Row> = Vec::new();
    for (no, raw) in text.lines().enumerate() {
        let line = raw.trim_end_matches('\r');
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        let at = |what: String| format!("allowlist line {}: {what}", no + 1);
        let fields: Vec<&str> = line.split('\t').collect();
        let [path, read, count, class, reason] = fields[..] else {
            return Err(at("expected <path>\\t<text>\\t<count>\\t<class>\\t<reason>".to_string()));
        };
        if path.is_empty() || read.is_empty() || reason.trim().is_empty() {
            return Err(at("empty path, text or reason".to_string()));
        }
        if path.starts_with('/') || path.split('/').any(|c| c == "..") {
            return Err(at(format!("path must be repo-relative: {path}")));
        }
        let count: usize = match count.parse() {
            Ok(n) if n > 0 => n,
            _ => return Err(at(format!("count must be a positive integer: {count}"))),
        };
        if !CLASSES.contains(&class) {
            return Err(at(format!("class must be one of {CLASSES:?}: {class}")));
        }
        if rows.iter().any(|row| row.path == path && row.text == read) {
            return Err(at(format!("duplicate row: {path}\t{read}")));
        }
        rows.push(Row { path: path.to_string(), text: read.to_string(), count });
    }
    Ok(rows)
}

pub type Found = BTreeMap<(String, String), Vec<usize>>;

pub struct Verdict {
    pub allowed: usize,
    pub rows: usize,
    pub violations: Vec<String>,
    pub stale: Vec<String>,
}

impl Verdict {
    pub fn passed(&self) -> bool {
        self.violations.is_empty() && self.stale.is_empty()
    }
}

pub fn judge(found: &Found, rows: &[Row]) -> Verdict {
    let mut verdict = Verdict { allowed: 0, rows: rows.len(), violations: Vec::new(), stale: Vec::new() };
    let mut violations: Vec<(&str, usize, String)> = Vec::new();
    for ((path, text), lines) in found {
        let listed = rows
            .iter()
            .find(|row| &row.path == path && &row.text == text)
            .map_or(0, |row| row.count);
        if lines.len() <= listed {
            verdict.allowed += lines.len();
            continue;
        }
        let beyond = if listed == 0 { String::new() } else { format!(" (row allows {listed}, found {})", lines.len()) };
        for line in lines {
            violations.push((path, *line, format!("{path}:{line}\t{text}{beyond}")));
        }
    }
    violations.sort();
    verdict.violations = violations.into_iter().map(|(_, _, text)| text).collect();
    for row in rows {
        let present = found.get(&(row.path.clone(), row.text.clone())).map_or(0, Vec::len);
        if present < row.count {
            verdict.stale.push(format!("{}\t{}: listed {}, found {present}", row.path, row.text, row.count));
        }
    }
    verdict
}

#[cfg(test)]
mod tests {
    use super::*;

    fn surface(path: &str, head: &[u8]) -> Option<Surface> {
        if in_data_plane(path) {
            return None;
        }
        surface_by_name(path).or_else(|| is_shell_script(head).then_some(Surface::Shell))
    }

    #[test]
    fn rows_are_checked_field_by_field() {
        let good = "# c\na/x.rs\tInstant::now()\t2\truntime-fact\tCK-001 watchdog\n";
        assert_eq!(parse_allowlist(good).map(|rows| rows.len()), Ok(1));
        for bad in [
            "a/x.rs\tInstant::now()\t2\truntime-fact\n",
            "a/x.rs\tInstant::now()\t0\truntime-fact\twhy\n",
            "a/x.rs\tInstant::now()\ttwo\truntime-fact\twhy\n",
            "a/x.rs\tInstant::now()\t1\tbomb\twhy\n",
            "a/x.rs\tInstant::now()\t1\ttransitional\twhy\n",
            "/a/x.rs\tInstant::now()\t1\truntime-fact\twhy\n",
            "a/../x.rs\tInstant::now()\t1\truntime-fact\twhy\n",
            "a/x.rs\t\t1\truntime-fact\twhy\n",
            "a/x.rs\tt\t1\trecord-only\twhy\na/x.rs\tt\t1\trecord-only\twhy\n",
        ] {
            assert!(parse_allowlist(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn surfaces() {
        assert_eq!(surface("a/src/x.rs", b""), Some(Surface::Rust));
        assert_eq!(surface("a/checks/data/cli/X/case.sh", b""), Some(Surface::Shell));
        assert_eq!(surface(".claude/rust/parity/data/S1/x.sh", b""), None);
        assert_eq!(surface(".claude/rust/py-boundary-guard/checks/data/verdict_clock/planted.rs", b""), None);
        assert_eq!(surface("x/.claude/rust/parity/data/S1/x.sh", b""), Some(Surface::Shell));
        assert_eq!(surface("s/run.sh", b""), Some(Surface::Shell));
        assert_eq!(surface(".claude/hooks/pre-commit", b"#!/bin/bash\nset -e\n"), Some(Surface::Shell));
        assert_eq!(surface("w/src/cli.ts", b""), Some(Surface::TypeScript));
        assert_eq!(surface("README.md", b""), None);
    }

    #[test]
    fn shebangs_name_their_interpreter() {
        for shell in [
            &b"#!/bin/bash\n"[..],
            b"#!/bin/bash -e\n",
            b"#!/usr/bin/env bash\r\n",
            b"#!/usr/bin/env -S bash -eu\n",
            b"#!/usr/bin/env -u X LC_ALL=C dash\n",
            b"#!/bin/sh",
            b"#!/usr/bin/zsh -f\n",
        ] {
            assert!(is_shell_script(shell), "{:?}", String::from_utf8_lossy(shell));
        }
        for other in [&b"#!/usr/bin/env python3\n"[..], b"#!/usr/bin/env -S node --x\n", b"# bash\n", b"echo\n"] {
            assert!(!is_shell_script(other), "{:?}", String::from_utf8_lossy(other));
        }
        assert_eq!(surface("s/tool.v2", b"#!/usr/bin/env bash\n"), Some(Surface::Shell));
    }
}
