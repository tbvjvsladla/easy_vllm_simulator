//! Helpers of the conformance-checker checks (included with `#[path]`; not a test target): frozen-case loading, scratch trees
//! (files, directories, symbolic links), a private installation copied from the installed sources, and the
//! environment that points the copied launchers at the binaries built from this crate.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use sweep_stop::conformance::Context;
use sweep_stop::pyjson::{decode_file_text, loads_text};
use sweep_stop::pystr::PyStr;
use sweep_stop::pyvalue::{PyDict, PyValue};

pub fn data(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("checks")
        .join("data")
        .join("conformance")
        .join(name)
}

pub fn read_json(name: &str) -> PyValue {
    let bytes = std::fs::read(data(name)).unwrap_or_else(|e| panic!("NOT_RUN: {name}: {e}"));
    loads_text(&decode_file_text(&bytes).expect("UTF-8")).expect("JSON")
}

pub fn read_text(name: &str) -> String {
    std::fs::read_to_string(data(name)).unwrap_or_else(|e| panic!("NOT_RUN: {name}: {e}"))
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
    map.get(key).unwrap_or_else(|| panic!("no field {key}"))
}

pub fn hex_bytes(hex: &str) -> Vec<u8> {
    (0..hex.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&hex[at..at + 2], 16).expect("hex"))
        .collect()
}

/// A scratch directory removed when dropped.
pub struct Tree {
    pub root: PathBuf,
}

impl Tree {
    pub fn new(label: &str) -> Tree {
        static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let serial = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let root = std::env::temp_dir().join(format!(
            "conformance-check-{label}-{}-{serial}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("scratch directory");
        Tree { root }
    }

    pub fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    pub fn write(&self, relative: &str, bytes: &[u8]) {
        let path = self.path(relative);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("directories");
        std::fs::write(path, bytes).expect("write");
    }

    /// The `before` map of a frozen case: `{name: {kind: file|directory|symlink, hex|target}}`.
    pub fn materialize(&self, before: &PyDict) {
        for (name, spec) in before.iter() {
            let name = name.to_utf8().expect("name");
            let spec = dict(spec);
            match text(field(spec, "kind")).as_str() {
                "file" => self.write(&name, &hex_bytes(&text(field(spec, "hex")))),
                "directory" => std::fs::create_dir_all(self.path(&name)).expect("directory"),
                "symlink" => {
                    std::os::unix::fs::symlink(text(field(spec, "target")), self.path(&name))
                        .expect("symlink")
                }
                other => panic!("unknown kind {other}"),
            }
        }
    }

    pub fn context(&self) -> Context {
        Context::from_path(&PyStr::from_surrogateescape(
            self.root.as_os_str().as_bytes(),
        ))
        .expect("context")
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// The scripts directory of the installation whose sources the checks read (environment `CONFORMANCE_SCRIPTS_DIR`).
pub fn installed_scripts_dir() -> PathBuf {
    PathBuf::from(
        std::env::var_os("CONFORMANCE_SCRIPTS_DIR").unwrap_or_else(|| {
            panic!("NOT_RUN: CONFORMANCE_SCRIPTS_DIR names no installed scripts directory")
        }),
    )
}

fn copy_files(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("directory");
    for entry in
        std::fs::read_dir(from).unwrap_or_else(|e| panic!("NOT_RUN: {}: {e}", from.display()))
    {
        let entry = entry.expect("entry");
        if entry.file_type().expect("type").is_file() {
            std::fs::copy(entry.path(), to.join(entry.file_name())).expect("copy");
        }
    }
}

/// A private installation: the scripts and fixtures of the installed skill, the three classifier sources the
/// checker reads and the two files of the neighbouring skills, in the layout `<root>/.claude/skills/...`.
/// Returns the scripts directory.
pub fn install_copy(tree: &Tree) -> PathBuf {
    let live_scripts = installed_scripts_dir();
    let live_skill = live_scripts.parent().expect("skill directory");
    let live_skills = live_skill.parent().expect("skills directory");
    let skill = tree.path(".claude/skills/adversarial-benchmark");
    copy_files(&live_scripts, &skill.join("scripts"));
    copy_files(&live_skill.join("fixtures"), &skill.join("fixtures"));
    for name in [
        "classify_cell.rs",
        "classify_cell_cli.rs",
        "argv.rs",
        "assemble_sweep_meta.rs",
    ] {
        let source = live_skill.join("rust/src").join(name);
        std::fs::create_dir_all(skill.join("rust/src")).expect("directory");
        std::fs::copy(&source, skill.join("rust/src").join(name))
            .unwrap_or_else(|e| panic!("NOT_RUN: {}: {e}", source.display()));
    }
    for relative in [
        "terraforming_node/scripts/campaign_init.py",
        "vllm-recipe-explorer/recipe.py",
    ] {
        let target = tree.path(".claude/skills").join(relative);
        std::fs::create_dir_all(target.parent().expect("parent")).expect("directory");
        std::fs::copy(live_skills.join(relative), target)
            .unwrap_or_else(|e| panic!("NOT_RUN: {relative}: {e}"));
    }
    skill.join("scripts")
}

/// The overrides the launchers honour, naming the owner binaries built from this crate (the three the caller names
/// and the sweep-index producer).
pub fn owner_overrides(
    bench_reporting: &str,
    publish: &str,
    lite_metrics: &str,
) -> BTreeMap<&'static str, String> {
    BTreeMap::from([
        ("BENCH_REPORTING_API_BIN", bench_reporting.to_string()),
        ("PUBLISH_BENCHMARK_RECORD_BIN", publish.to_string()),
        ("LITE_METRICS_API_BIN", lite_metrics.to_string()),
        (
            "ASSEMBLE_SWEEP_META_BIN",
            env!("CARGO_BIN_EXE_assemble_sweep_meta").to_string(),
        ),
    ])
}

/// Applies the overrides to a command.
pub fn with_overrides(
    command: &mut std::process::Command,
    bench_reporting: &str,
    publish: &str,
    lite_metrics: &str,
) {
    for (name, value) in owner_overrides(bench_reporting, publish, lite_metrics) {
        command.env(name, value);
    }
}

/// A frozen case's materialised result as the checks compare it: kind, value or (type, message), stdout.
pub fn expected_matches(record: &PyDict, expected: &PyDict) -> Result<(), String> {
    let kind = text(field(expected, "kind"));
    if text(field(record, "kind")) != kind {
        return Err(format!(
            "kind: {} != {kind}",
            record.get("kind").map_or("?".into(), PyValue::repr)
        ));
    }
    if kind == "value" {
        if field(record, "value") != field(expected, "value") {
            return Err(format!(
                "value {} != {}",
                field(record, "value").repr(),
                field(expected, "value").repr()
            ));
        }
    } else {
        for key in ["type", "message"] {
            if field(record, key) != field(expected, key) {
                return Err(format!(
                    "{key} {} != {}",
                    field(record, key).repr(),
                    field(expected, key).repr()
                ));
            }
        }
    }
    match (record.get("stdout"), expected.get("stdout")) {
        (Some(own), Some(frozen)) if own == frozen => Ok(()),
        (None, None) => Ok(()),
        (own, frozen) => Err(format!(
            "stdout {:?} != {:?}",
            own.map(PyValue::repr),
            frozen.map(PyValue::repr)
        )),
    }
}
