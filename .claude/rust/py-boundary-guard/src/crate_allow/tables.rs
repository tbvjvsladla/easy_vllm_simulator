//! The two human-maintained tables the check reads: the crate allowlist (TSV) and the crate notice (a Markdown table).

/// One approved crate release. `license` is the manifest's SPDX expression as written in the allowlist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllowedCrate {
    pub name: String,
    pub version: String,
    pub license: String,
    /// 1-based line in the allowlist file, for messages.
    pub line: usize,
}

/// Columns the allowlist must have (FORMAT.md 「crate_allowlist.tsv」 requires `crate` and `version`; the check also
/// needs `license`). Further columns are for the human reader and are not interpreted here.
const REQUIRED_COLUMNS: [&str; 3] = ["crate", "version", "license"];

/// Parse `crate_allowlist.tsv`: `#` lines and blank lines are comments, the first other line is the header, every
/// later line is one row with exactly the header's number of tab-separated fields. A row with an empty crate,
/// version or license, and a second row for the same `crate@version`, are input errors.
pub fn parse_allowlist(text: &str) -> Result<Vec<AllowedCrate>, String> {
    let mut lines = text
        .lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.trim_end_matches('\r')))
        .filter(|(_, l)| !l.trim().is_empty() && !l.starts_with('#'));
    let Some((_, header)) = lines.next() else {
        return Err("allowlist has no header line".to_string());
    };
    let columns: Vec<&str> = header.split('\t').collect();
    let index_of = |name: &str| columns.iter().position(|c| *c == name);
    let mut required = [0usize; 3];
    for (slot, name) in required.iter_mut().zip(REQUIRED_COLUMNS) {
        *slot = index_of(name).ok_or_else(|| {
            format!("allowlist header lacks the column '{name}' (header: {header})")
        })?;
    }
    let [crate_col, version_col, license_col] = required;
    let mut rows: Vec<AllowedCrate> = Vec::new();
    for (line, raw) in lines {
        let fields: Vec<&str> = raw.split('\t').collect();
        if fields.len() != columns.len() {
            return Err(format!(
                "allowlist line {line}: {} fields, the header has {}",
                fields.len(),
                columns.len()
            ));
        }
        let (name, version, license) =
            (fields[crate_col], fields[version_col], fields[license_col]);
        if name.is_empty() || version.is_empty() || license.is_empty() {
            return Err(format!(
                "allowlist line {line}: crate, version and license must not be empty"
            ));
        }
        if let Some(first) = rows.iter().find(|r| r.name == name && r.version == version) {
            return Err(format!(
                "allowlist line {line}: {name}@{version} is already listed at line {}",
                first.line
            ));
        }
        rows.push(AllowedCrate {
            name: name.to_string(),
            version: version.to_string(),
            license: license.to_string(),
            line,
        });
    }
    Ok(rows)
}

/// One row of the crate notice table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoticeEntry {
    pub name: String,
    pub version: String,
    pub license: String,
}

fn cells(row: &str) -> Vec<String> {
    let inner = row.trim().trim_start_matches('|').trim_end_matches('|');
    inner
        .split('|')
        .map(|c| c.trim().trim_matches('`').to_string())
        .collect()
}

/// Rows of every Markdown table in the notice whose header names the cells `crate`, `version` and `license` (in any
/// order, among other cells). Prose and other tables are ignored. A table row with fewer cells than its header is
/// an input error.
pub fn parse_notice(text: &str) -> Result<Vec<NoticeEntry>, String> {
    let mut entries = Vec::new();
    let mut columns: Option<(usize, [usize; 3])> = None; // (header width, [crate, version, license])
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if !line.starts_with('|') {
            columns = None;
            continue;
        }
        let row = cells(line);
        match columns {
            None => {
                let at = |n: &str| row.iter().position(|c| c == n);
                if let (Some(c), Some(v), Some(l)) = (at("crate"), at("version"), at("license")) {
                    columns = Some((row.len(), [c, v, l]));
                }
            }
            Some((width, [c, v, l])) => {
                if row
                    .iter()
                    .all(|cell| !cell.is_empty() && cell.chars().all(|ch| matches!(ch, '-' | ':')))
                {
                    continue; // the |---|---| separator
                }
                if row.len() < width {
                    return Err(format!(
                        "notice line {}: {} cells, the table header has {width}",
                        i + 1,
                        row.len()
                    ));
                }
                entries.push(NoticeEntry {
                    name: row[c].clone(),
                    version: row[v].clone(),
                    license: row[l].clone(),
                });
            }
        }
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::{parse_allowlist, parse_notice};

    #[test]
    fn allowlist_rows_after_comments_and_header() {
        let rows = parse_allowlist(
            "# note\ncrate\tversion\tlicense\tuse\nsyn\t3.0.9\tMIT OR Apache-2.0\tscan\n",
        )
        .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(
            (
                rows[0].name.as_str(),
                rows[0].version.as_str(),
                rows[0].line
            ),
            ("syn", "3.0.9", 3)
        );
    }

    #[test]
    fn allowlist_input_errors() {
        assert!(
            parse_allowlist("crate\tversion\n").is_err(),
            "no license column"
        );
        assert!(
            parse_allowlist("crate\tversion\tlicense\nsyn\t3.0.9\n").is_err(),
            "short row"
        );
        assert!(
            parse_allowlist("crate\tversion\tlicense\nsyn\t3.0.9\t\n").is_err(),
            "empty license"
        );
        assert!(
            parse_allowlist("crate\tversion\tlicense\nsyn\t3.0.9\tMIT\nsyn\t3.0.9\tMIT\n").is_err(),
            "duplicate"
        );
        assert!(parse_allowlist("# only comments\n").is_err(), "no header");
    }

    #[test]
    fn notice_rows_of_the_crate_table_only() {
        let md = "# Notice\n\n| a | b |\n|---|---|\n| x | y |\n\n| crate | version | license | used by |\n|---|---|---|---|\n| `syn` | 3.0.9 | MIT OR Apache-2.0 | guard |\n";
        let rows = parse_notice(md).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(
            (rows[0].name.as_str(), rows[0].license.as_str()),
            ("syn", "MIT OR Apache-2.0")
        );
    }
}
