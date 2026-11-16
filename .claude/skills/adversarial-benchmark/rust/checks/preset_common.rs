//! Shared helpers of the preset checks (included with `#[path]`; not a test target itself).

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use sweep_stop::pyjson::{decode_file_text, loads_text};
use sweep_stop::pyvalue::PyValue;

pub const OWNER_REL: &str = ".claude/skills/terraforming_node/scripts/campaign_init.py";
pub const SKILL_REL: &str = ".claude/skills/adversarial-benchmark";

pub fn data_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("checks")
        .join("data")
        .join("preset")
        .join(name)
}

pub fn load_json(path: &Path) -> PyValue {
    let bytes = std::fs::read(path)
        .unwrap_or_else(|e| panic!("NOT_RUN: cannot read {}: {e}", path.display()));
    let text = decode_file_text(&bytes).expect("reference files are UTF-8");
    loads_text(&text).unwrap_or_else(|e| panic!("{} is not JSON: {e}", path.display()))
}

pub fn field<'a>(value: &'a PyValue, name: &str) -> &'a PyValue {
    match value.peel() {
        PyValue::Dict(map) => map
            .get(name)
            .unwrap_or_else(|| panic!("missing field {name}")),
        other => panic!("not an object ({}) reading {name}", other.type_name()),
    }
}

pub fn text(value: &PyValue) -> String {
    match value.peel() {
        PyValue::Str(s) => s.to_utf8().expect("utf-8 text"),
        other => panic!("not a string: {}", other.type_name()),
    }
}

pub fn items(value: &PyValue) -> &[PyValue] {
    match value.peel() {
        PyValue::List(items) => items,
        other => panic!("not a list: {}", other.type_name()),
    }
}

/// The repository whose installed layout supplies the owner, the config and the adapter.
pub fn real_repo() -> PathBuf {
    let repo = std::env::var_os("BENCH_PRESET_REPO_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .ancestors()
                .nth(4)
                .expect("installed layout has a repository root")
                .to_path_buf()
        });
    assert!(
        repo.join(OWNER_REL).is_file(),
        "NOT_RUN: no campaign_init.py under {}",
        repo.display()
    );
    assert!(
        repo.join(SKILL_REL)
            .join("config/bench_bindings.yaml")
            .is_file(),
        "NOT_RUN: no preset config under {}",
        repo.display()
    );
    repo
}

/// A command with the native bindings of `repo` (root and skill directory); nothing from the
/// caller's environment names another installation.
pub fn command(bin: &str, repo: &Path) -> Command {
    let mut cmd = Command::new(bin);
    cmd.env_remove("SWEEP_ORACLE_DIR")
        .env_remove("REPEAT_AXIS_REPO_ROOT")
        .env("BENCH_PRESET_REPO_ROOT", repo)
        .env("BENCH_PRESET_SKILL_DIR", repo.join(SKILL_REL))
        .env("COLUMNS", "80")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cmd
}

pub fn run(mut cmd: Command, input: &[u8]) -> std::process::Output {
    use std::io::Write;
    let mut child = cmd.spawn().expect("spawn");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(input)
        .expect("feed stdin");
    child.wait_with_output().expect("wait")
}

pub fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("preset-check.{}.{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch");
    dir.canonicalize().expect("canonical scratch")
}

pub fn base64_decode(encoded: &str) -> Vec<u8> {
    let value = |c: u8| -> u32 {
        match c {
            b'A'..=b'Z' => u32::from(c - b'A'),
            b'a'..=b'z' => u32::from(c - b'a') + 26,
            b'0'..=b'9' => u32::from(c - b'0') + 52,
            b'+' => 62,
            b'/' => 63,
            _ => panic!("bad base64"),
        }
    };
    let mut out = Vec::new();
    let mut acc = 0u32;
    let mut bits = 0;
    for &c in encoded.as_bytes().iter().filter(|&&c| c != b'=') {
        acc = (acc << 6) | value(c);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((acc >> bits) & 0xFF) as u8);
        }
    }
    out
}

pub fn replace_bytes(data: &[u8], from: &[u8], to: &[u8]) -> Vec<u8> {
    if from.is_empty() {
        return data.to_vec();
    }
    let mut out = Vec::with_capacity(data.len());
    let mut at = 0;
    while at < data.len() {
        if data[at..].starts_with(from) {
            out.extend_from_slice(to);
            at += from.len();
        } else {
            out.push(data[at]);
            at += 1;
        }
    }
    out
}

/// relative path -> `None` for a directory, `Some(bytes)` for a file.
pub type Tree = BTreeMap<String, Option<Vec<u8>>>;

pub fn snapshot(root: &Path, normalize_root: bool) -> Tree {
    fn walk(dir: &Path, root: &Path, out: &mut Tree, normalize: bool) {
        let mut names: Vec<_> = std::fs::read_dir(dir)
            .expect("read dir")
            .map(|e| e.expect("entry").path())
            .collect();
        names.sort();
        for path in names {
            let rel = path
                .strip_prefix(root)
                .expect("under root")
                .to_string_lossy()
                .to_string();
            if path.is_dir() {
                out.insert(rel, None);
                walk(&path, root, out, normalize);
            } else {
                let raw = std::fs::read(&path).expect("read file");
                let bytes = if normalize {
                    replace_bytes(&raw, root.to_string_lossy().as_bytes(), b"@ROOT@")
                } else {
                    raw
                };
                out.insert(rel, Some(bytes));
            }
        }
    }
    let mut out = Tree::new();
    walk(root, root, &mut out, normalize_root);
    out
}

/// Stages a tree described by `{path: {kind, bytes_base64}}` entries (scratch literal replaced after decoding).
pub fn stage(root: &Path, entries: &PyValue) {
    let PyValue::Dict(map) = entries.peel() else {
        panic!("tree is an object")
    };
    for (rel, entry) in map.iter() {
        let path = root.join(rel.to_utf8().expect("utf-8 path"));
        if text(field(entry, "kind")) == "directory" {
            std::fs::create_dir_all(&path).expect("mkdir");
        } else {
            std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir parent");
            let raw = base64_decode(&text(field(entry, "bytes_base64")));
            std::fs::write(
                &path,
                replace_bytes(&raw, b"@ROOT@", root.to_string_lossy().as_bytes()),
            )
            .expect("write");
        }
    }
}

pub fn expected_tree(entries: &BTreeMap<String, PyValue>) -> Tree {
    entries
        .iter()
        .map(|(rel, entry)| {
            let bytes = (text(field(entry, "kind")) == "file")
                .then(|| base64_decode(&text(field(entry, "bytes_base64"))));
            (rel.clone(), bytes)
        })
        .collect()
}
