//! The harness's own packages: a closed list of the 16 harness crates (the campaign's `crates.tsv`), each with the
//! tracked `Cargo.toml` that declares it and the `[package]` name and version that manifest declares now.
//!
//! A source-less package in a lock is exempt from the crates.io rules only when its name and version are one of
//! these entries **and** the tracked manifest at that entry's path declares exactly that name and version. Any other
//! source-less package is a `path-unknown` finding: a vendored copy (with or without a tracked lock of its own), a
//! 17th crate, a harness crate whose version moved without this list, a harness manifest that is not tracked.
//!
//! The list is a tripwire (a closed list that forces a review on change, `.claude/rules/workflow.md` §결정론 규율):
//! adding a harness crate or raising one's version changes `HARNESS_CRATES` in the same commit, or the check fails.

use std::collections::BTreeSet;
use std::path::Path;

use crate::lockfile::string_field;

/// `(tracked manifest, [package] name, [package] version)` of the 16 harness crates.
pub const HARNESS_CRATES: [(&str, &str, &str); 16] = [
    (".claude/rust/parity/Cargo.toml", "parity", "0.1.0"),
    (
        ".claude/rust/policy-audit/Cargo.toml",
        "policy-audit",
        "0.1.0",
    ),
    (
        ".claude/rust/policy-distribution/Cargo.toml",
        "policy-distribution",
        "0.1.0",
    ),
    (
        ".claude/rust/policy-gate/Cargo.toml",
        "policy-gate",
        "0.1.0",
    ),
    (
        ".claude/rust/policy-predicates/Cargo.toml",
        "policy-predicates",
        "0.1.0",
    ),
    (
        ".claude/rust/policy-publisher/Cargo.toml",
        "policy-publisher",
        "0.1.0",
    ),
    (
        ".claude/rust/policy-selftest/Cargo.toml",
        "policy-selftest",
        "0.1.0",
    ),
    (
        ".claude/rust/py-boundary-guard/Cargo.toml",
        "py-boundary-guard",
        "0.1.0",
    ),
    (
        ".claude/rust/role-contract/Cargo.toml",
        "role-contract",
        "0.1.0",
    ),
    (
        ".claude/rust/runtime-support/Cargo.toml",
        "agent-runtime-support",
        "0.1.0",
    ),
    (
        ".claude/skills/adversarial-benchmark/rust/Cargo.toml",
        "sweep-stop",
        "0.1.0",
    ),
    (
        ".claude/skills/hint-publisher/rust/Cargo.toml",
        "hint-publisher",
        "0.1.0",
    ),
    (
        ".claude/skills/terraforming_node/rust/Cargo.toml",
        "terraforming-runtime",
        "0.1.0",
    ),
    (
        ".claude/skills/upstream-version-watch/rust/runtime/Cargo.toml",
        "upstream-runtime",
        "0.1.0",
    ),
    (
        ".claude/skills/vllm-recipe-explorer/rust/runtime/Cargo.toml",
        "recipe-runtime",
        "0.1.0",
    ),
    (
        ".claude/skills/wiki-desk/rust/Cargo.toml",
        "wiki-desk-oracle",
        "0.1.0",
    ),
];

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

/// `(name, version)` of every `HARNESS_CRATES` entry whose manifest is tracked and declares exactly that name and
/// version. An entry whose manifest is untracked or declares something else gives nothing (its packages are then
/// `path-unknown`); a tracked entry manifest that cannot be read or has no readable name and version is an error.
pub fn packages(repo: &Path, manifests: &[String]) -> Result<BTreeSet<(String, String)>, String> {
    let mut found = BTreeSet::new();
    for (manifest, name, version) in HARNESS_CRATES {
        if !manifests.iter().any(|m| m == manifest) {
            continue;
        }
        let path = repo.join(manifest);
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("cannot read manifest {}: {e}", path.display()))?;
        let declared = name_and_version(&text).map_err(|e| format!("{manifest}: {e}"))?;
        if declared.0 == name && declared.1 == version {
            found.insert(declared);
        }
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::{name_and_version, HARNESS_CRATES};
    use std::collections::BTreeSet;
    use std::path::Path;

    #[test]
    fn package_name_and_version_only() {
        let toml = "[package]\nname = \"parity\"\nversion = \"0.1.0\"\n\n[dependencies]\nsyn = { version = \"3\" }\n";
        assert_eq!(
            name_and_version(toml).unwrap(),
            ("parity".to_string(), "0.1.0".to_string())
        );
        assert!(name_and_version("[package]\nname = \"x\"\nversion.workspace = true\n").is_err());
        assert!(
            name_and_version("[package]\nname = \"x\"\nname = \"y\"\nversion = \"1\"\n").is_err()
        );
        assert!(name_and_version("[workspace]\nmembers = [\"a\"]\n").is_err());
    }

    #[test]
    fn the_list_has_sixteen_distinct_crates() {
        let paths: BTreeSet<&str> = HARNESS_CRATES.iter().map(|(m, _, _)| *m).collect();
        let names: BTreeSet<&str> = HARNESS_CRATES.iter().map(|(_, n, _)| *n).collect();
        assert_eq!((paths.len(), names.len()), (16, 16));
        assert!(paths.iter().all(|m| m.ends_with("/Cargo.toml")));
    }

    /// The tripwire: every entry equals what the manifest in this checkout declares (the repository root is three
    /// levels above this crate).
    #[test]
    fn the_list_equals_the_manifests_of_this_checkout() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        for (manifest, name, version) in HARNESS_CRATES {
            let text = std::fs::read_to_string(root.join(manifest))
                .unwrap_or_else(|e| panic!("{manifest}: {e}"));
            assert_eq!(
                name_and_version(&text).unwrap(),
                (name.to_string(), version.to_string()),
                "{manifest}: change HARNESS_CRATES in the same commit"
            );
        }
    }
}
