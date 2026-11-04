//! The crate carries no developer path: sources, checks, notices, manifests and fixtures name only package-relative
//! locations. (The needles are assembled at run time so this file does not contain them itself.)

use std::path::Path;

fn scan(dir: &Path, needles: &[String], hits: &mut Vec<String>) {
    for entry in std::fs::read_dir(dir).expect("read_dir") {
        let path = entry.expect("entry").path();
        if path.is_dir() {
            if path
                .file_name()
                .is_some_and(|n| n == "target" || n == "data")
            {
                if path.file_name().is_some_and(|n| n == "data") {
                    scan(&path, needles, hits);
                }
                continue;
            }
            scan(&path, needles, hits);
        } else if let Ok(text) = std::fs::read_to_string(&path) {
            for needle in needles {
                if text.contains(needle.as_str()) {
                    hits.push(format!("{} contains {needle}", path.display()));
                }
            }
        }
    }
}

#[test]
fn no_developer_path_is_packaged() {
    let needles = [
        format!("/{}/", "home"),
        format!("{}_{}", "val", "agent"),
        format!("real-{}-", "worker"),
        format!("{}-{}", "worker", "checks"),
    ];
    let mut hits = Vec::new();
    scan(Path::new(env!("CARGO_MANIFEST_DIR")), &needles, &mut hits);
    assert!(hits.is_empty(), "{hits:#?}");
}

#[test]
fn no_scratch_named_directory_is_tracked() {
    fn walk(dir: &Path, found: &mut Vec<String>) {
        for entry in std::fs::read_dir(dir).expect("read_dir") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();
                if name == "target" {
                    continue;
                }
                if ["temp", "tmp", "test", "tests"].contains(&name.as_str()) {
                    found.push(path.display().to_string());
                }
                walk(&path, found);
            }
        }
    }
    let mut found = Vec::new();
    walk(Path::new(env!("CARGO_MANIFEST_DIR")), &mut found);
    assert!(found.is_empty(), "{found:?}");
}
