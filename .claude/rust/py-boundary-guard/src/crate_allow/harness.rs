//! The harness's own packages: the `[package]` name and version of every tracked `Cargo.toml` that sits beside a
//! tracked `Cargo.lock` (the crate whose build that lock pins — the 16 crates of the campaign's `crates.tsv`). A
//! source-less package in a lock is exempt from the crates.io rules only when it is one of these; any other path
//! package (a vendored copy, a crate without its own tracked lock) is a `path-unknown` finding. Nothing here is a
//! hand-written list: the set follows the tracked manifests.

use std::collections::BTreeSet;
use std::path::Path;

use crate::lockfile::string_field;

/// The `[package]` `name` and `version` of a repository manifest. Both must be basic strings and appear once; a
/// missing, repeated or other value (`version.workspace = true` included) is an error.
pub fn name_and_version(manifest: &str) -> Result<(String, String), String> {
    let mut in_package = false;
    let (mut name, mut version) = (None, None);
    for (index, raw) in manifest.lines().enumerate() {
        let line = raw.trim();
        if line.starts_with('[') {
            in_package = line == "[package]";
            continue;
        }
        if !in_package {
            continue;
        }
        let slot = match line.split_once('=').map(|(k, _)| k.trim()) {
            Some("name") => &mut name,
            Some("version") => &mut version,
            _ => continue,
        };
        let Some((key, value)) = string_field(line) else {
            return Err(format!(
                "line {}: the [package] value is not a basic string",
                index + 1
            ));
        };
        if slot.is_some() {
            return Err(format!("line {}: a second [package] {key}", index + 1));
        }
        *slot = Some(value);
    }
    match (name, version) {
        (Some(n), Some(v)) => Ok((n, v)),
        _ => Err("no [package] name or no [package] version".to_string()),
    }
}

/// `dir/Cargo.lock` for `dir/Cargo.toml` (`Cargo.lock` at the top).
fn sibling_lock(manifest: &str) -> String {
    match manifest.rsplit_once('/') {
        Some((dir, _)) => format!("{dir}/Cargo.lock"),
        None => "Cargo.lock".to_string(),
    }
}

/// `(name, version)` of every tracked manifest beside a tracked lock. An unreadable such manifest is an error.
pub fn packages(
    repo: &Path,
    locks: &[String],
    manifests: &[String],
) -> Result<BTreeSet<(String, String)>, String> {
    let mut found = BTreeSet::new();
    for manifest in manifests
        .iter()
        .filter(|m| locks.contains(&sibling_lock(m)))
    {
        let path = repo.join(manifest);
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("cannot read manifest {}: {e}", path.display()))?;
        found.insert(name_and_version(&text).map_err(|e| format!("{manifest}: {e}"))?);
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::{name_and_version, sibling_lock};

    #[test]
    fn package_name_and_version_only() {
        let toml = "[package]\nname = \"parity\"\nversion = \"0.1.0\"\n\n[dependencies]\nsyn = { version = \"3\" }\n";
        assert_eq!(
            name_and_version(toml).unwrap(),
            ("parity".to_string(), "0.1.0".to_string())
        );
        assert!(name_and_version("[package]\nname = \"x\"\nversion.workspace = true\n").is_err());
        assert!(name_and_version("[package]\nname = \"x\"\nname = \"y\"\nversion = \"1\"\n").is_err());
        assert!(name_and_version("[workspace]\nmembers = [\"a\"]\n").is_err());
    }

    #[test]
    fn the_lock_beside_a_manifest() {
        assert_eq!(sibling_lock("a/b/Cargo.toml"), "a/b/Cargo.lock");
        assert_eq!(sibling_lock("Cargo.toml"), "Cargo.lock");
    }
}
