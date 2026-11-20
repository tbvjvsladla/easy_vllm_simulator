//! The license a downloaded package declares in its own manifest, read from the Cargo home that the node's builds
//! use (`<cargo home>/registry/src/<index dir>/<name>-<version>/Cargo.toml`). Nothing is downloaded here.

use std::path::Path;

/// What the downloaded manifest says about the license.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeclaredLicense {
    /// No unpacked source of that release in the Cargo home.
    SourceMissing,
    /// The `license` field of `[package]`.
    Expression(String),
    /// The manifest has no `license` field (only `license-file`, or nothing).
    NotDeclared,
}

/// The `license = "…"` value of the `[package]` table of a normalized (published) manifest.
fn package_license(manifest: &str) -> Option<String> {
    let mut in_package = false;
    for raw in manifest.lines() {
        let line = raw.trim();
        if line.starts_with('[') {
            in_package = line == "[package]";
            continue;
        }
        if !in_package {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim() == "license" {
            let value = value.trim();
            return Some(
                value
                    .strip_prefix('"')
                    .and_then(|v| v.strip_suffix('"'))
                    .unwrap_or(value)
                    .to_string(),
            );
        }
    }
    None
}

/// Look the release up in every index directory of `<cargo_home>/registry/src`. A manifest that exists but cannot
/// be read is an error, so that an unreadable file never passes as "missing".
pub fn declared_license(
    cargo_home: &Path,
    name: &str,
    version: &str,
) -> Result<DeclaredLicense, String> {
    let src = cargo_home.join("registry").join("src");
    let indexes = match std::fs::read_dir(&src) {
        Ok(dir) => dir,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(DeclaredLicense::SourceMissing)
        }
        Err(e) => return Err(format!("cannot list {}: {e}", src.display())),
    };
    let mut index_dirs: Vec<_> = indexes
        .map(|entry| {
            entry
                .map(|e| e.path())
                .map_err(|e| format!("cannot list {}: {e}", src.display()))
        })
        .collect::<Result<_, _>>()?;
    index_dirs.sort();
    for index in index_dirs {
        let manifest = index.join(format!("{name}-{version}")).join("Cargo.toml");
        match std::fs::read_to_string(&manifest) {
            Ok(text) => {
                return Ok(match package_license(&text) {
                    Some(expr) => DeclaredLicense::Expression(expr),
                    None => DeclaredLicense::NotDeclared,
                });
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(format!("cannot read {}: {e}", manifest.display())),
        }
    }
    Ok(DeclaredLicense::SourceMissing)
}

#[cfg(test)]
mod tests {
    use super::package_license;

    #[test]
    fn license_of_the_package_table_only() {
        let toml = "[package]\nname = \"x\"\nlicense = \"MIT OR Apache-2.0\"\n\n[dependencies.y]\nlicense = \"not this\"\n";
        assert_eq!(package_license(toml).as_deref(), Some("MIT OR Apache-2.0"));
        assert_eq!(
            package_license("[package]\nname = \"x\"\nlicense-file = \"LICENSE\"\n"),
            None
        );
    }
}
