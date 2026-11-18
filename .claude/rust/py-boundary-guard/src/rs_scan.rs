//! `.rs` mode of `inline_python_scan` (plan 26111805 c10 · request 26111719 A-4): tracked Rust sources must not launch
//! the Python interpreter outside the temporary allowlist `rs_python_allowlist.tsv`.
//!
//! The finder (`finder`) reads each tracked `.rs` with syn and reports every launch with its line, kind and normalized
//! call text. An allowlist row = `<repo path>\t<call text>\t<count>\t<cell>\t<reason>`: that file holds exactly
//! `count` launches with that call text, and `cell` is the campaign cell that removes them. There is no per-file
//! permission:
//! - a launch with no row, or beyond the count of its row, is a violation (`RS-VIOLATION`);
//! - a row with fewer launches than its count (none at all included) is stale (`RS-STALE`).
//!
//! Both fail the check. A Python started inside a container (`docker exec|run … python3`) is not a host or sub launch
//! (plan 26111805 D16: the container class), so the finder does not count it. Neither does it follow a program across
//! files or through struct fields; the strace count of the campaign (`pyexec0`) is the runtime net behind this static
//! one. A source syn cannot parse as a file (a fragment, a fixture) is read token by token for `Command::new("…")` and
//! named on stderr. A missing default allowlist reads as an empty one (every launch is a violation); an explicit
//! `--rs-allowlist` that cannot be read, a malformed row and a source that is not even Rust tokens are input errors
//! (fail-closed).

mod finder;
mod names;
mod tokens;

use std::collections::BTreeMap;
use std::path::Path;

pub const DEFAULT_RS_ALLOWLIST: &str = ".claude/rust/py-boundary-guard/rs_python_allowlist.tsv";

/// The stack of the scanning thread: syn and the visitor recurse along the nesting of the source.
const SCAN_STACK_BYTES: usize = 256 * 1024 * 1024;

struct Row {
    path: String,
    call: String,
    count: usize,
}

fn is_cell_name(text: &str) -> bool {
    !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn parse_allowlist(text: &str) -> Result<Vec<Row>, String> {
    let mut rows: Vec<Row> = Vec::new();
    for (no, raw) in text.lines().enumerate() {
        let line = raw.trim_end_matches('\r');
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        let at = |what: String| format!("rs allowlist line {}: {what}", no + 1);
        let fields: Vec<&str> = line.split('\t').collect();
        let [path, call, count, cell, reason] = fields[..] else {
            return Err(at(
                "expected <path>\\t<call>\\t<count>\\t<cell>\\t<reason>".to_string()
            ));
        };
        let (path, call, cell, reason) = (path.trim(), call.trim(), cell.trim(), reason.trim());
        if path.is_empty() || call.is_empty() || reason.is_empty() {
            return Err(at("empty path, call or reason".to_string()));
        }
        if path.starts_with('/') || path.split('/').any(|c| c == "..") || !path.ends_with(".rs") {
            return Err(at(format!("path must be a repo-relative .rs file: {path}")));
        }
        let count: usize = match count.trim().parse() {
            Ok(n) if n > 0 => n,
            _ => return Err(at(format!("count must be a positive integer: {count}"))),
        };
        if !is_cell_name(cell) {
            return Err(at(format!(
                "cell must be a campaign cell name ([a-z0-9-]+): {cell}"
            )));
        }
        if rows.iter().any(|row| row.path == path && row.call == call) {
            return Err(at(format!("duplicate row: {path}\t{call}")));
        }
        rows.push(Row {
            path: path.to_string(),
            call: call.to_string(),
            count,
        });
    }
    Ok(rows)
}

/// The verdict of the `.rs` mode.
pub struct RsReport {
    scanned: usize,
    allowed: usize,
    rows: usize,
    violations: Vec<String>,
    stale: Vec<String>,
}

impl RsReport {
    pub fn passed(&self) -> bool {
        self.violations.is_empty() && self.stale.is_empty()
    }

    /// Findings on stdout; on a pass one summary line.
    pub fn print(&self) {
        if self.passed() {
            println!(
                "[inline_python_scan] .rs mode: scanned {} files · {} interpreter launches within rs_python_allowlist.tsv ({} rows)",
                self.scanned, self.allowed, self.rows
            );
            return;
        }
        for violation in &self.violations {
            println!("RS-VIOLATION {violation}");
        }
        for stale in &self.stale {
            println!("RS-STALE {stale}");
        }
    }

    pub fn print_failure(&self) {
        eprintln!(
            "[inline_python_scan] FAIL .rs mode: {} launch(es) outside rs_python_allowlist.tsv · {} stale row(s). \
             하네스는 호스트·서브에서 Python 을 띄우지 않는다 — 남은 기동은 사유와 대체 cell 과 함께 임시 허용목록에만 둔다.",
            self.violations.len(),
            self.stale.len()
        );
    }
}

/// Tracked `.rs` sources of the scan: everything but the frozen data planes.
fn is_scanned_source(path: &str) -> bool {
    path.ends_with(".rs") && !crate::in_data_plane(path)
}

/// Scans the tracked `.rs` sources of `repo` against the allowlist (`None` = the default path, absent = empty).
pub fn scan(repo: &Path, tracked: &[String], allowlist: Option<&Path>) -> Result<RsReport, String> {
    let text = match allowlist {
        Some(path) => std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read rs allowlist {}: {e}", path.display()))?,
        None => match std::fs::read_to_string(repo.join(DEFAULT_RS_ALLOWLIST)) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => {
                return Err(format!(
                    "cannot read rs allowlist {DEFAULT_RS_ALLOWLIST}: {e}"
                ))
            }
        },
    };
    let rows = parse_allowlist(&text)?;
    let sources: Vec<(String, String)> = tracked
        .iter()
        .filter(|rel| is_scanned_source(rel))
        .filter_map(|rel| {
            std::fs::read(repo.join(rel))
                .ok()
                .map(|bytes| (rel.clone(), bytes))
        }) // tracked but absent: skipped
        .map(|(rel, bytes)| {
            String::from_utf8(bytes)
                .map(|text| (rel.clone(), text))
                .map_err(|_| format!("{rel}: not UTF-8"))
        })
        .collect::<Result<_, _>>()?;
    let found = std::thread::Builder::new()
        .stack_size(SCAN_STACK_BYTES)
        .spawn(move || find_all(&sources))
        .map_err(|e| format!("cannot start the .rs scan: {e}"))?
        .join()
        .map_err(|_| "the .rs scan panicked".to_string())??;
    Ok(judge(found, &rows))
}

/// Launches by (path, call text): their lines and kinds, in file order.
type Found = (
    usize,
    BTreeMap<(String, String), Vec<(usize, &'static str)>>,
);

fn find_all(sources: &[(String, String)]) -> Result<Found, String> {
    let mut found: BTreeMap<(String, String), Vec<(usize, &'static str)>> = BTreeMap::new();
    for (rel, text) in sources {
        let (launches, reading) = finder::find_launches(text).map_err(|e| {
            format!(
                "cannot read {rel} as Rust tokens ({e}); the .rs mode reads every tracked source"
            )
        })?;
        if reading == finder::Reading::Tokens {
            eprintln!("[inline_python_scan] note .rs mode: {rel} does not parse as a Rust file; read token by token for Command::new(\"…\")");
        }
        for launch in launches {
            found
                .entry((rel.clone(), launch.call))
                .or_default()
                .push((launch.line, launch.kind.label()));
        }
    }
    Ok((sources.len(), found))
}

fn judge((scanned, found): Found, rows: &[Row]) -> RsReport {
    let mut report = RsReport {
        scanned,
        allowed: 0,
        rows: rows.len(),
        violations: Vec::new(),
        stale: Vec::new(),
    };
    let mut violations: Vec<(&str, usize, String)> = Vec::new();
    for ((path, call), sites) in &found {
        let listed = rows
            .iter()
            .find(|row| &row.path == path && &row.call == call)
            .map_or(0, |row| row.count);
        if sites.len() <= listed {
            report.allowed += sites.len();
            continue;
        }
        for (line, kind) in sites {
            let beyond = if listed == 0 {
                String::new()
            } else {
                format!(" (row allows {listed}, found {})", sites.len())
            };
            violations.push((path, *line, format!("{path}:{line}: {kind} {call}{beyond}")));
        }
    }
    violations.sort();
    report.violations = violations.into_iter().map(|(_, _, text)| text).collect();
    for row in rows {
        let present = found
            .get(&(row.path.clone(), row.call.clone()))
            .map_or(0, Vec::len);
        if present < row.count {
            report.stale.push(format!(
                "{}\t{}: listed {}, found {present}",
                row.path, row.call, row.count
            ));
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_are_checked_field_by_field() {
        let good = "# c\na/x.rs\tCommand::new(\"python3\")\t2\tc34-recipe-runtime\toracle\n";
        assert_eq!(parse_allowlist(good).map(|rows| rows.len()), Ok(1));
        for bad in [
            "a/x.rs\tCommand::new(\"python3\")\t2\tc34\n",
            "a/x.rs\tCommand::new(\"python3\")\t0\tc34\twhy\n",
            "a/x.rs\tCommand::new(\"python3\")\ttwo\tc34\twhy\n",
            "a/x.rs\tCommand::new(\"python3\")\t1\tC34 cell\twhy\n",
            "/a/x.rs\tCommand::new(\"python3\")\t1\tc34\twhy\n",
            "a/x.sh\tCommand::new(\"python3\")\t1\tc34\twhy\n",
            "a/x.rs\t \t1\tc34\twhy\n",
            "a/x.rs\tc\t1\tc34\twhy\na/x.rs\tc\t1\tc34\twhy\n",
        ] {
            assert!(parse_allowlist(bad).is_err(), "{bad:?}");
        }
    }
}
