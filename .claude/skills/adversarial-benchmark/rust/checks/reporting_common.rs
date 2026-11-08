//! Helpers of the reporting checks (included with `#[path]`; not a test target): frozen-case loading, scratch
//! trees in the shape the original-only references use, and the comparisons.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use sweep_stop::pyjson::{decode_file_text, loads_text};
use sweep_stop::pyvalue::{PyDict, PyValue};

pub fn data(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("checks")
        .join("data")
        .join("reporting")
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

pub fn base64_encode(bytes: &[u8]) -> String {
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for (i, shift) in [18, 12, 6, 0].iter().enumerate() {
            if i <= chunk.len() {
                out.push(ALPHABET[((n >> shift) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

fn replace_bytes(data: &[u8], from: &[u8], to: &[u8]) -> Vec<u8> {
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

/// A scratch tree under the system temporary directory, removed when dropped.
pub struct Scratch {
    pub root: PathBuf,
}

impl Scratch {
    pub fn new(label: &str) -> Scratch {
        static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let id = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let root = std::env::temp_dir().join(format!(
            "reporting-check-{}-{id}-{label}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).expect("scratch root");
        Scratch { root }
    }

    pub fn text(&self) -> String {
        self.root.to_string_lossy().into_owned()
    }

    /// Builds the tree of a frozen case; `@ROOT@` inside files and link targets becomes this root, and a file with
    /// `from_data` takes its bytes from that check-data file.
    pub fn populate(&self, tree: &PyValue) {
        for (name, item) in dict(tree).iter() {
            let path = self.root.join(text(&PyValue::Str(name.clone())));
            let item = dict(item);
            match text(field(item, "kind")).as_str() {
                "directory" => std::fs::create_dir_all(&path).expect("directory"),
                "file" => {
                    std::fs::create_dir_all(path.parent().expect("parent")).expect("parent");
                    let bytes = match item.get("from_data") {
                        Some(file) => std::fs::read(data(&text(file))).expect("shared fixture"),
                        None => replace_bytes(
                            &base64_decode(&text(field(item, "bytes_base64"))),
                            b"@ROOT@",
                            self.text().as_bytes(),
                        ),
                    };
                    std::fs::write(&path, bytes).expect("file");
                }
                other => panic!("unsupported fixture kind {other}"),
            }
        }
    }

    /// The tree as the references record it (path -> directory | file bytes with the root written as `@ROOT@`).
    pub fn snapshot(&self) -> BTreeMap<String, Option<Vec<u8>>> {
        fn walk(base: &Path, dir: &Path, out: &mut BTreeMap<String, Option<Vec<u8>>>, root: &str) {
            for entry in std::fs::read_dir(dir).expect("read_dir") {
                let path = entry.expect("entry").path();
                let name = path
                    .strip_prefix(base)
                    .expect("inside")
                    .to_string_lossy()
                    .into_owned();
                if path.is_dir() {
                    out.insert(name, None);
                    walk(base, &path, out, root);
                } else {
                    let bytes = std::fs::read(&path).expect("file");
                    out.insert(
                        name,
                        Some(replace_bytes(&bytes, root.as_bytes(), b"@ROOT@")),
                    );
                }
            }
        }
        let mut out = BTreeMap::new();
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
pub fn expected_tree(tree: &PyValue) -> BTreeMap<String, Option<Vec<u8>>> {
    dict(tree)
        .iter()
        .map(|(name, item)| {
            let item = dict(item);
            let bytes = match text(field(item, "kind")).as_str() {
                "directory" => None,
                _ => Some(match item.get("from_data") {
                    Some(file) => std::fs::read(data(&text(file))).expect("shared fixture"),
                    None => base64_decode(&text(field(item, "bytes_base64"))),
                }),
            };
            (name.to_utf8().expect("UTF-8"), bytes)
        })
        .collect()
}

pub fn rebase(value: &PyValue, from: &str, to: &str) -> PyValue {
    match value {
        PyValue::Str(text) => PyValue::str(&text.to_utf8().expect("UTF-8").replace(from, to)),
        PyValue::List(items) => PyValue::List(items.iter().map(|i| rebase(i, from, to)).collect()),
        PyValue::Dict(map) => {
            let mut out = PyDict::new();
            for (key, item) in map.iter() {
                out.insert(
                    sweep_stop::pystr::PyStr::new(&key.to_utf8().expect("UTF-8").replace(from, to)),
                    rebase(item, from, to),
                );
            }
            PyValue::Dict(out)
        }
        other => other.clone(),
    }
}

/// What the established traceback projection keeps: everything before the traceback and its final line.
pub fn traceback_projection(stderr: &str) -> String {
    match stderr.find("Traceback (most recent call last):\n") {
        Some(at) => {
            let last = stderr.trim_end().lines().last().unwrap_or("");
            format!("{}TRACEBACK\n{last}\n", &stderr[..at])
        }
        None => stderr.to_string(),
    }
}

/// `json.dumps(value)` as the comparison key of a value (NaN-safe, order-preserving).
pub fn key(value: &PyValue) -> String {
    value.dumps_ascii().unwrap_or_else(|e| format!("<{e}>"))
}
