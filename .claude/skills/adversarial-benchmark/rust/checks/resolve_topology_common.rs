//! Helpers of the topology-resolver checks (included with `#[path]`; not a test target): frozen-case loading, scratch trees in
//! the shape the original-only references use (files, directories, symbolic links), and the comparisons.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use sweep_stop::pyfs::os_string;
use sweep_stop::pyjson::{decode_file_text, loads_text};
use sweep_stop::pystr::PyStr;
use sweep_stop::pyvalue::{PyDict, PyValue};

pub fn data(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("checks")
        .join("data")
        .join("resolve_topology")
        .join(name)
}

pub fn read_json(name: &str) -> PyValue {
    let bytes = std::fs::read(data(name)).unwrap_or_else(|e| panic!("NOT_RUN: {name}: {e}"));
    loads_text(&decode_file_text(&bytes).expect("UTF-8")).expect("JSON")
}

pub fn read_jsonl(name: &str) -> Vec<PyDict> {
    let text =
        std::fs::read_to_string(data(name)).unwrap_or_else(|e| panic!("NOT_RUN: {name}: {e}"));
    text.lines()
        .map(
            |line| match sweep_stop::pyjson::loads(line).expect("JSON line") {
                PyValue::Dict(row) => row,
                other => panic!("not an object: {}", other.repr()),
            },
        )
        .collect()
}

pub fn list(value: PyValue) -> Vec<PyValue> {
    match value {
        PyValue::List(items) => items,
        other => panic!("not an array: {}", other.repr()),
    }
}

pub fn dict(value: &PyValue) -> &PyDict {
    match value {
        PyValue::Dict(map) => map,
        other => panic!("not an object: {}", other.repr()),
    }
}

pub fn text(value: &PyValue) -> String {
    match value {
        PyValue::Str(text) => text.to_utf8().expect("UTF-8"),
        other => panic!("not a string: {}", other.repr()),
    }
}

pub fn field<'a>(map: &'a PyDict, key: &str) -> &'a PyValue {
    map.get(key).unwrap_or_else(|| panic!("missing {key}"))
}

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn base64_decode(input: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let (mut bits, mut count) = (0u32, 0);
    for byte in input.bytes().filter(|b| *b != b'=') {
        let value = ALPHABET.iter().position(|a| *a == byte).expect("base64") as u32;
        bits = (bits << 6) | value;
        count += 6;
        if count >= 8 {
            count -= 8;
            out.push((bits >> count) as u8);
            bits &= (1 << count) - 1;
        }
    }
    out
}

pub fn replace_bytes(data: &[u8], from: &[u8], to: &[u8]) -> Vec<u8> {
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

/// One entry of a recorded tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Entry {
    Directory,
    File(Vec<u8>),
    Link(Vec<u8>),
}

pub type Tree = BTreeMap<Vec<u32>, Entry>;

fn name_of(key: &PyStr) -> OsString {
    os_string(key).unwrap_or_else(|_| panic!("a tree name must be encodable"))
}

/// A scratch tree under the system temporary directory, removed when dropped.
pub struct Scratch {
    pub root: PathBuf,
}

impl Scratch {
    pub fn new(label: &str) -> Scratch {
        static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let id = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let root = std::env::temp_dir().join(format!(
            "resolve-topology-check-{}-{id}-{label}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).expect("scratch root");
        // The owner fixtures compare the `--repo` they receive with their working directory (the real path).
        let root = root.canonicalize().expect("canonical scratch root");
        Scratch { root }
    }

    pub fn text(&self) -> String {
        self.root.to_string_lossy().into_owned()
    }

    /// Builds the tree of a frozen case; `@ROOT@` inside files and link targets becomes this root (the recorded
    /// bytes carry no mode: every file is plain data, the owner fixtures are run by an interpreter).
    pub fn populate(&self, tree: &PyValue) {
        for (name, item) in dict(tree).iter() {
            let path = self.root.join(name_of(name));
            let item = dict(item);
            match text(field(item, "kind")).as_str() {
                "directory" => std::fs::create_dir_all(&path).expect("directory"),
                "file" => {
                    std::fs::create_dir_all(path.parent().expect("parent")).expect("parent");
                    let bytes = replace_bytes(
                        &base64_decode(&text(field(item, "bytes_base64"))),
                        b"@ROOT@",
                        self.text().as_bytes(),
                    );
                    std::fs::write(&path, bytes).expect("file");
                }
                "symlink" => {
                    let target = text(field(item, "target")).replace("@ROOT@", &self.text());
                    std::fs::create_dir_all(path.parent().expect("parent")).expect("parent");
                    std::os::unix::fs::symlink(target, &path).expect("symlink");
                }
                other => panic!("unsupported fixture kind {other}"),
            }
        }
    }

    /// The tree as the references record it (the root written as `@ROOT@`).
    pub fn snapshot(&self) -> Tree {
        fn walk(base: &Path, dir: &Path, out: &mut Tree, root: &str) {
            for entry in std::fs::read_dir(dir).expect("read_dir") {
                let path = entry.expect("entry").path();
                let relative = path.strip_prefix(base).expect("inside");
                let name = PyStr::from_surrogateescape(relative.as_os_str().as_bytes())
                    .code_points()
                    .to_vec();
                let kind = std::fs::symlink_metadata(&path)
                    .expect("metadata")
                    .file_type();
                if kind.is_symlink() {
                    let target = std::fs::read_link(&path)
                        .expect("link")
                        .into_os_string()
                        .into_vec();
                    out.insert(
                        name,
                        Entry::Link(replace_bytes(&target, root.as_bytes(), b"@ROOT@")),
                    );
                } else if kind.is_dir() {
                    out.insert(name, Entry::Directory);
                    walk(base, &path, out, root);
                } else {
                    let bytes = std::fs::read(&path).expect("file");
                    out.insert(
                        name,
                        Entry::File(replace_bytes(&bytes, root.as_bytes(), b"@ROOT@")),
                    );
                }
            }
        }
        let mut out = Tree::new();
        walk(&self.root, &self.root, &mut out, &self.text());
        out
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// The tree a frozen case expects, in the same shape as [`Scratch::snapshot`].
pub fn expected_tree(tree: &PyValue) -> Tree {
    dict(tree)
        .iter()
        .map(|(name, item)| {
            let item = dict(item);
            let entry = match text(field(item, "kind")).as_str() {
                "directory" => Entry::Directory,
                "symlink" => Entry::Link(text(field(item, "target")).into_bytes()),
                _ => Entry::File(base64_decode(&text(field(item, "bytes_base64")))),
            };
            (name.code_points().to_vec(), entry)
        })
        .collect()
}

/// The `after` tree of a case: `"same"` stands for the `before` tree.
pub fn after_tree(case: &PyDict) -> Tree {
    match field(case, "after") {
        PyValue::Str(_) => expected_tree(field(case, "before")),
        other => expected_tree(other),
    }
}

pub fn rebase(value: &PyValue, from: &str, to: &str) -> PyValue {
    match value {
        PyValue::Str(text) => PyValue::str(&text.to_utf8().expect("UTF-8").replace(from, to)),
        PyValue::List(items) => PyValue::List(items.iter().map(|i| rebase(i, from, to)).collect()),
        PyValue::Dict(map) => {
            let mut out = PyDict::new();
            for (key, item) in map.iter() {
                out.insert(
                    PyStr::new(&key.to_utf8().expect("UTF-8").replace(from, to)),
                    rebase(item, from, to),
                );
            }
            PyValue::Dict(out)
        }
        other => other.clone(),
    }
}

/// `json.dumps(value)` as the comparison key of a value (NaN-safe, order-preserving).
pub fn key(value: &PyValue) -> String {
    value.dumps_ascii().unwrap_or_else(|e| format!("<{e}>"))
}

/// The check-only stand-in of the native parity helper for the frozen resolve cases (`RESOLVE_TOPOLOGY_PARITY_BIN`): it
/// answers the declared `owner-response.json` of its working directory (stdout and stderr bytes as hex, the exit status) and
/// records its argv in `owner-calls.json`, exactly as the frozen cases' former owner fixture (a Python program) did. It holds
/// no verdict logic: the resolver's interpretation of the answer is what the cases check. Written once per test process.
pub fn stub_helper() -> String {
    use std::os::unix::fs::PermissionsExt;
    static STUB: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    STUB.get_or_init(|| {
        let dir = std::env::temp_dir().join(format!("resolve-topology-stub-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("stub directory");
        let path = dir.join("topology_parity_stub");
        let body = r#"#!/bin/bash
# CHECK-ONLY stand-in of the native parity helper (never installed): declared answer + argv record, no verdict logic.
set -u
here=$(pwd -P)
resp=$(<owner-response.json)
field() { local v=${resp#*\"$1\": \"}; printf '%s' "${v%%\"*}"; }
rc=${resp#*\"rc\": }; rc=${rc%%[,\}]*}
exact=false
if [ "$#" -eq 5 ] && [ "$1" = evaluate ] && [ "$2" = --repo ] && [ "$3" = "$here" ] && [ "$4" = --format ] && [ "$5" = json ]; then exact=true; fi
words=''
for a in "$@"; do [ "$a" = "$here" ] && a='@ROOT@'; a=${a//\\/\\\\}; a=${a//\"/\\\"}; words+="${words:+, }\"$a\""; done
printf '{"argv": [%s], "exact_argv": %s}' "$words" "$exact" > owner-calls.json
hex() { local h=$1 out=''; while [ -n "$h" ]; do out+="\\x${h:0:2}"; h=${h:2}; done; printf '%b' "$out"; }
hex "$(field stdout_hex)"
hex "$(field stderr_hex)" >&2
exit "$rc"
"#;
        let staged = dir.join("topology_parity_stub.tmp");
        std::fs::write(&staged, body).expect("stub");
        std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755)).expect("mode");
        std::fs::rename(&staged, &path).expect("stub in place");
        path.to_string_lossy().into_owned()
    })
    .clone()
}

/// The helper a frozen case's scratch tree is judged by: the stand-in when the case declares an owner answer
/// (`owner-response.json`), else none (the real native helper next to the binary under test).
pub fn helper_for(scratch: &Scratch) -> Option<String> {
    scratch.root.join("owner-response.json").is_file().then(stub_helper)
}

/// The scratch root placeholder of the frozen cases replaced.
pub fn fill(value: &PyValue, scratch: &Scratch) -> PyValue {
    rebase(value, "@ROOT@", &scratch.text())
}
