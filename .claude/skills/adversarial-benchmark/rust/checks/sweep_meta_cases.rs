//! The sweep-index producer (`assemble_sweep_meta`) as a library and as the real program.
//!
//! * Frozen originals: 25 of the original full-IO cases (inputs, environment, whole standard output, standard error,
//!   status and the whole index) are run through the real program on clones of their inputs and compared byte for
//!   byte after the fixture root and a fresh clock reading are projected.
//! * Scenarios the originals do not reach (the sidecars, the dataset provenance, the lite block, the serve proof,
//!   integer and path semantics of the environment, the backends, partial writes) with expectations derived from the
//!   original program's text, run through the library with a private cell.
//! * The program itself: it asks the environment only for the variables it owns, runs with no `PATH` at all, ends with
//!   the original's status and diagnostics, and its launcher and start-up check behave as installed.
use std::path::{Path, PathBuf};
use sweep_stop::assemble_sweep_meta::{assemble, Ending, EnvSource, MapEnv, Produced};
use sweep_stop::pyjson::loads;
use sweep_stop::pystr::PyStr;
use sweep_stop::pyvalue::PyValue;

pub const PRODUCER: &str = env!("CARGO_BIN_EXE_assemble_sweep_meta");
pub const SKILL: &str = ".claude/skills/adversarial-benchmark";

/// The installation the checks read shared files from (the benchmark skill's fixtures and launchers).
pub fn closure_root() -> PathBuf {
    std::env::var_os("BENCH_VERIFY_REPO_ROOT")
        .or_else(|| std::env::var_os("EASY_VLLM_SOURCE_ROOT"))
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .ancestors()
                .nth(4)
                .expect("the crate sits four levels below the checkout")
                .to_path_buf()
        })
}

/// A scratch directory removed when dropped.
pub struct Scratch {
    pub root: PathBuf,
}

impl Scratch {
    pub fn new(label: &str) -> Scratch {
        static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let serial = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let root = std::env::temp_dir().join(format!(
            "sweep-meta-check-{label}-{}-{serial}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("scratch directory");
        Scratch { root }
    }

    pub fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    pub fn write(&self, relative: &str, text: &str) {
        let path = self.path(relative);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("directories");
        std::fs::write(path, text).expect("write");
    }

    pub fn read(&self, relative: &str) -> String {
        std::fs::read_to_string(self.path(relative)).expect("read")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

pub const ENGINE_LINE: &str = "INFO 08-23 12:20:10 [core.py:99] Initializing a V1 LLM engine (v0.27.1.dev0+g4bdc8a788.d20260814) with config: model='/models/acme/Acme-7B', quantization=fp8, quantization_config=None, kv_cache_dtype=auto, seed=None\n";

/// One cell as the sweep driver leaves it: config yaml, env file, manifest, level 1 with an engine log and a
/// measurement, an empty truncation log and the measurement-shape record.
pub struct Cell {
    pub scratch: Scratch,
}

impl Cell {
    pub fn new(label: &str) -> Cell {
        let scratch = Scratch::new(label);
        scratch.write(
            "cfg.yaml",
            "model: /models/acme/Acme-7B\nmax-model-len: 4096\n",
        );
        scratch.write(".env.x", "IMAGE_TAG=easy-vllm:9.9.9-cell\n");
        scratch.write(
            "manifest.yaml",
            "gpu_model: NVIDIA GB10\ngpus_per_node: 1\ntopology: single\ndriver_version: 1.2.3\n",
        );
        scratch.write("sweep_x/level_01/engine_x.log", ENGINE_LINE);
        scratch.write(
            "sweep_x/level_01/measured.json",
            "{\"measurement_ok\": true}",
        );
        scratch.write("sweep_x/truncation.log", "");
        scratch.write("knobs.json", "{\"preset\": \"cell\"}");
        Cell { scratch }
    }

    pub fn path(&self, relative: &str) -> String {
        self.scratch.path(relative).to_string_lossy().into_owned()
    }

    pub fn env(&self) -> MapEnv {
        MapEnv::new()
            .with("CONFIG", "x")
            .with("TOPO", "single")
            .with("CFGYAML", &self.path("cfg.yaml"))
            .with("EF", &self.path(".env.x"))
            .with("MANIFEST", &self.path("manifest.yaml"))
            .with("SWEEPDIR", &self.path("sweep_x"))
            .with("COMPLETED", "1")
            .with("ILEN", "1024")
            .with("KNOB_RECORD", &self.path("knobs.json"))
    }

    pub fn index_path(&self) -> PathBuf {
        self.scratch.path("sweep_x/sweep_index.json")
    }

    /// The index text, or `None` when none was written.
    pub fn index_text(&self) -> Option<String> {
        std::fs::read_to_string(self.index_path()).ok()
    }

    pub fn index(&self) -> PyValue {
        loads(&self.index_text().expect("an index was written")).expect("the index is JSON")
    }
}

pub const FIXED_CLOCK: &str = "2031-02-03T04:05:06Z";

/// One run of the producer library with a fixed clock reading.
pub fn produce(env: &dyn EnvSource) -> Produced {
    assemble(env, &|| FIXED_CLOCK.to_string())
}

pub fn member(value: &PyValue, key: &str) -> PyValue {
    match value.peel() {
        PyValue::Dict(map) => map
            .get(key)
            .unwrap_or_else(|| panic!("no member {key}"))
            .clone(),
        other => panic!("not an object: {}", other.repr()),
    }
}

pub fn keys(value: &PyValue) -> Vec<String> {
    match value.peel() {
        PyValue::Dict(map) => map
            .iter()
            .map(|(key, _)| key.to_utf8().expect("key"))
            .collect(),
        other => panic!("not an object: {}", other.repr()),
    }
}

pub fn meta(index: &PyValue) -> PyValue {
    member(index, "meta")
}

pub fn text_of(value: &PyValue) -> String {
    match value.peel() {
        PyValue::Str(text) => text.to_utf8().expect("UTF-8"),
        other => panic!("not a string: {}", other.repr()),
    }
}

pub fn stderr_text(produced: &Produced) -> String {
    produced.stderr.to_utf8().expect("UTF-8")
}

pub fn done_line(produced: &Produced) -> String {
    match &produced.ending {
        Ending::Done(line) => line.to_utf8().expect("UTF-8"),
        other => panic!("the run did not end with the index line: {other:?}"),
    }
}

pub fn uncaught(produced: &Produced) -> (String, String) {
    match &produced.ending {
        Ending::Uncaught(failure) => (
            failure.class.to_string(),
            failure.message.to_utf8().expect("UTF-8"),
        ),
        other => panic!("the run did not end with an uncaught exception: {other:?}"),
    }
}

pub fn pystr(text: &str) -> PyStr {
    PyStr::new(text)
}

/// The D1 connection-refused lite fixture of the installation, with its result files made absolute and a stored
/// verdict: a raw lite record the producer's lite block can be built from.
pub fn lite_raw_from_fixture(cell: &Cell, verdict: &str) -> String {
    let fixture = closure_root()
        .join(SKILL)
        .join("fixtures/lite_raw_d1_connection_refused.json");
    let bytes = std::fs::read(&fixture)
        .unwrap_or_else(|error| panic!("NOT_RUN: {}: {error}", fixture.display()));
    let mut raw = loads(&String::from_utf8(bytes).expect("UTF-8")).expect("JSON");
    let directory = fixture.parent().expect("fixture directory").to_path_buf();
    if let PyValue::Dict(map) = &mut raw {
        for key in ["bench_cold_json", "bench_warm_json"] {
            let relative = text_of(map.get(key).expect("result file"));
            map.set(
                key,
                PyValue::str(&directory.join(relative).to_string_lossy()),
            );
        }
        map.set("lite_verdict", PyValue::str(verdict));
    }
    let path = cell.path("lite_raw.json");
    std::fs::write(&path, raw.dumps_ascii().expect("JSON")).expect("write");
    path
}

// ── frozen originals ──────────────────────────────────────────────────────────────

use std::process::Command;
use sweep_stop::assemble_sweep_meta::ENV_NAMES;
use sweep_stop::utc::{parse_utc, UtcInstant};

fn oracle_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("checks/data/sweep_meta")
}

fn copy_tree_detached(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("directory");
    for entry in std::fs::read_dir(from).expect("read directory") {
        let entry = entry.expect("entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("type").is_dir() {
            copy_tree_detached(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).expect("copy");
        }
    }
}

fn file_set(root: &Path) -> Vec<String> {
    fn walk(base: &Path, dir: &Path, out: &mut Vec<String>) {
        for entry in std::fs::read_dir(dir).expect("read directory") {
            let entry = entry.expect("entry");
            if entry.file_type().expect("type").is_dir() {
                walk(base, &entry.path(), out);
            } else {
                out.push(
                    entry
                        .path()
                        .strip_prefix(base)
                        .expect("inside")
                        .to_string_lossy()
                        .into_owned(),
                );
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out.sort();
    out
}

struct Frozen {
    name: String,
    env: Vec<(String, String)>,
    status: i32,
    stdout: String,
    stderr: String,
    index: Option<String>,
    fresh_clock: bool,
}

fn frozen_cases() -> Vec<Frozen> {
    let mut names: Vec<String> = std::fs::read_dir(oracle_dir())
        .expect("oracle data")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
        .into_iter()
        .map(|name| {
            let text = std::fs::read_to_string(oracle_dir().join(&name).join("expect.json"))
                .expect("expectation");
            let document = loads(&text).expect("JSON");
            let env = match member(&document, "env").peel() {
                PyValue::Dict(map) => map
                    .iter()
                    .map(|(key, value)| (key.to_utf8().expect("name"), text_of(value)))
                    .collect(),
                _ => panic!("env is an object"),
            };
            let index = match member(&document, "index").peel() {
                PyValue::None => None,
                other => Some(text_of(other)),
            };
            Frozen {
                status: i32::try_from(
                    member(&document, "status")
                        .as_plain_int()
                        .expect("status")
                        .to_string()
                        .parse::<i64>()
                        .expect("number"),
                )
                .expect("status fits"),
                stdout: text_of(&member(&document, "stdout")),
                stderr: text_of(&member(&document, "stderr")),
                fresh_clock: matches!(member(&document, "fresh_clock").peel(), PyValue::Bool(true)),
                name,
                env,
                index,
            }
        })
        .collect()
}

/// The native traceback marker line: the original's `File "<stdin>"` frame lines are source frames of a program that
/// no longer exists, and the frozen data has them removed.
const NATIVE_FRAME_NOTE: &str = "  (native: Python stack frames are not reproduced)\n";

fn clock_stamp(index: &str) -> (String, String) {
    let marker = "\"generated_utc\": \"";
    let at = index.find(marker).expect("generated_utc") + marker.len();
    let end = at + index[at..].find('"').expect("end of value");
    (
        index[at..end].to_string(),
        format!("{}<fresh>{}", &index[..at], &index[end..]),
    )
}

/// Runs one frozen case through the real program; returns what differs.
fn replay(case: &Frozen) -> Vec<String> {
    let scratch = Scratch::new("frozen");
    let node = scratch.path("node");
    copy_tree_detached(&oracle_dir().join(&case.name).join("input"), &node);
    let before = file_set(&node);
    let root = node.to_string_lossy().into_owned();
    let mut command = Command::new(PRODUCER);
    command.env_clear().current_dir(&scratch.root);
    for (name, value) in &case.env {
        command.env(name, value.replace("{ROOT}", &root));
    }
    let started = UtcInstant::now();
    let output = command.output().expect("the program starts");
    let finished = UtcInstant::now();
    let mut problems = Vec::new();
    let project = |bytes: &[u8]| String::from_utf8_lossy(bytes).replace(&root, "{ROOT}");
    if output.status.code() != Some(case.status) {
        problems.push(format!(
            "status {:?} != {}",
            output.status.code(),
            case.status
        ));
    }
    if project(&output.stdout) != case.stdout {
        problems.push(format!(
            "stdout {:?} != {:?}",
            project(&output.stdout),
            case.stdout
        ));
    }
    let stderr = project(&output.stderr).replace(NATIVE_FRAME_NOTE, "");
    if stderr != case.stderr {
        problems.push(format!("stderr {stderr:?} != {:?}", case.stderr));
    }
    let written = std::fs::read_to_string(node.join("sweep_x/sweep_index.json")).ok();
    match (&case.index, written) {
        (Some(expected), Some(written)) => {
            let mut written = written.replace(&root, "{ROOT}");
            if case.fresh_clock {
                let (stamp, projected) = clock_stamp(&written);
                match parse_utc(&PyValue::str(&stamp), "generated_utc") {
                    Ok(instant) => {
                        let first = parse_utc(&PyValue::str(&started.evaluated_text()), "start")
                            .expect("our own text");
                        let last = parse_utc(&PyValue::str(&finished.evaluated_text()), "end")
                            .expect("our own text");
                        if instant.whole_seconds_since(&first) < 0
                            || last.whole_seconds_since(&instant) < 0
                        {
                            problems.push(format!(
                                "generated_utc {stamp} is outside the call interval"
                            ));
                        }
                    }
                    Err(error) => {
                        problems.push(format!("generated_utc {stamp} is not UTC text: {error}"))
                    }
                }
                written = projected;
            }
            if &written != expected {
                let first_difference = written
                    .chars()
                    .zip(expected.chars())
                    .position(|(left, right)| left != right)
                    .unwrap_or(written.len().min(expected.len()));
                problems.push(format!("index differs from character {first_difference}"));
            }
            let mut expected_files = before.clone();
            expected_files.push("sweep_x/sweep_index.json".to_string());
            expected_files.sort();
            expected_files.dedup();
            if file_set(&node) != expected_files {
                problems
                    .push("the file set after the run is not the input plus the index".to_string());
            }
        }
        (Some(_), None) => problems.push("no index was written".to_string()),
        (None, written) => {
            let prior = std::fs::read_to_string(
                oracle_dir()
                    .join(&case.name)
                    .join("input/sweep_x/sweep_index.json"),
            )
            .ok();
            if written != prior {
                problems.push(
                    "an index was written or changed where the original wrote none".to_string(),
                );
            }
            if file_set(&node) != before {
                problems.push("the run changed the file set".to_string());
            }
        }
    }
    problems
}

#[test]
fn every_frozen_original_case_is_reproduced_byte_for_byte() {
    let cases = frozen_cases();
    assert_eq!(cases.len(), 25, "the frozen selection");
    let mut failures = Vec::new();
    for case in &cases {
        let problems = replay(case);
        if !problems.is_empty() {
            failures.push(format!("{}: {}", case.name, problems.join("; ")));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn the_frozen_selection_reaches_every_ending_of_the_original() {
    let cases = frozen_cases();
    let statuses: std::collections::BTreeSet<i32> = cases.iter().map(|case| case.status).collect();
    assert_eq!(statuses.into_iter().collect::<Vec<_>>(), [0, 1, 2]);
    assert!(
        cases
            .iter()
            .any(|case| !case.fresh_clock && case.index.is_some()),
        "a reassembly"
    );
    assert!(
        cases.iter().any(|case| case.index.is_none()
            && case.status == 1
            && case.stderr.contains("Traceback")),
        "an uncaught exception"
    );
    assert!(
        cases.iter().any(|case| case.index.is_none()
            && case.status == 1
            && !case.stderr.contains("Traceback")),
        "a refusal by SystemExit(message)"
    );
}

/// A source that remembers what it was asked for.
struct Recording<'a> {
    inner: MapEnv,
    asked: &'a std::cell::RefCell<std::collections::BTreeSet<String>>,
}

impl EnvSource for Recording<'_> {
    fn get(&self, name: &str) -> Option<PyStr> {
        self.asked.borrow_mut().insert(name.to_string());
        self.inner.get(name)
    }
}

#[test]
fn the_producer_reads_only_its_own_variables_and_every_one_of_them_matters() {
    let asked = std::cell::RefCell::new(std::collections::BTreeSet::new());
    for case in frozen_cases() {
        let scratch = Scratch::new("names");
        let node = scratch.path("node");
        copy_tree_detached(&oracle_dir().join(&case.name).join("input"), &node);
        let root = node.to_string_lossy().into_owned();
        let mut env = MapEnv::new();
        for (name, value) in &case.env {
            env = env.with(name, &value.replace("{ROOT}", &root));
        }
        let _ = produce(&Recording {
            inner: env,
            asked: &asked,
        });
    }
    // The scenarios below set the variables the frozen cases leave out; the union is what must equal the list.
    let mut asked = asked.into_inner();
    for name in [
        "IMAGE_DIGEST_ACTUAL",
        "SERVE_PLANE",
        "EASY_VLLM_VERSION",
        "REPEATS",
        "REPEATS_SOURCE",
        "REPEAT_KIND",
        "CLAMP_LEVEL",
    ] {
        asked.insert(name.to_string());
    }
    let listed: std::collections::BTreeSet<String> =
        ENV_NAMES.iter().map(ToString::to_string).collect();
    let unlisted: Vec<_> = asked.difference(&listed).collect();
    assert!(
        unlisted.is_empty(),
        "variables read but not listed: {unlisted:?}"
    );
    let unread: Vec<_> = listed.difference(&asked).collect();
    assert!(
        unread.is_empty(),
        "variables listed but never read: {unread:?}"
    );
}

// ── scenarios ─────────────────────────────────────────────────────────────────────

#[test]
fn a_fresh_assembly_writes_the_index_in_the_originals_layout() {
    let cell = Cell::new("layout");
    let produced = produce(&cell.env());
    assert!(done_line(&produced).ends_with("(완료 레벨 [1] · 절삭 0건)"));
    assert_eq!(
        keys(&cell.index()),
        [
            "config",
            "topology",
            "generated_utc",
            "reassembled_from_raw",
            "verdict_point_level",
            "meta",
            "lite",
            "levels",
            "repetition",
            "truncated",
            "input_len",
            "bench_knobs"
        ]
    );
    let text = cell.index_text().expect("index");
    assert!(text.starts_with("{\n  \"config\": \"x\",\n"));
    assert!(!text.ends_with('\n'), "json.dump writes no final newline");
    assert_eq!(
        text_of(&member(&cell.index(), "generated_utc")),
        FIXED_CLOCK
    );
    assert!(matches!(
        member(&cell.index(), "reassembled_from_raw").peel(),
        PyValue::None
    ));
}

#[test]
fn the_clock_is_read_once_for_a_fresh_measurement_and_never_for_a_reassembly() {
    let cell = Cell::new("clock");
    let reads = std::cell::Cell::new(0);
    let _ = assemble(&cell.env(), &|| {
        reads.set(reads.get() + 1);
        FIXED_CLOCK.to_string()
    });
    assert_eq!(reads.get(), 1);
    let produced = assemble(&cell.env().with("REASSEMBLE", "1"), &|| {
        panic!("a reassembly does not read the clock")
    });
    let _ = done_line(&produced);
    let index = cell.index();
    assert_eq!(text_of(&member(&index, "generated_utc")), FIXED_CLOCK);
    assert!(matches!(
        member(&index, "reassembled_from_raw").peel(),
        PyValue::Bool(true)
    ));
}

#[test]
fn the_sidecars_of_the_first_readable_level_become_the_tool_block() {
    let cell = Cell::new("sidecars");
    cell.scratch.write(
        "sweep_x/level_01/measured.json",
        "{\"measurement_ok\": true}",
    );
    cell.scratch.write(
        "sweep_x/level_02/measured.json",
        "{\"measurement_ok\": true, \"bench_tool\": \"guidellm\", \"bench_tool_version\": \"0.5.1\", \"bench_tool_version_source\": \"measured(pip)\", \"max_error_rate_declared\": 0.02, \"server_alive_at_bench_end\": true, \"boundary_exemption\": \"none\"}",
    );
    cell.scratch.write(
        "sweep_x/level_02/bench_tool_x.json",
        "{\"image_ref\": \"registry/tool:1\", \"observed_digest\": \"sha256:ab\", \"endpoint\": \"http://host:8000\"}",
    );
    let _ = done_line(&produce(&cell.env().with("COMPLETED", "2 1")));
    let index = cell.index();
    let meta = meta(&index);
    // level 1 is read first: its measured.json names no tool, so the default tool is the answer, and the readable
    // file ends the search (level 2 is not looked at for the tool fields)
    assert_eq!(text_of(&member(&meta, "bench_tool")), "vllm-bench-serve");
    assert_eq!(text_of(&member(&meta, "bench_tool_version")), "NA");
    assert_eq!(
        text_of(&member(&meta, "bench_tool_version_source")),
        "declared(도구가 버전을 자기보고하지 않는다)"
    );
    assert_eq!(text_of(&member(&meta, "bench_max_error_rate")), "NA");
    assert!(matches!(
        member(&meta, "server_alive_at_bench_end").peel(),
        PyValue::None
    ));
    // the per-run tool image sidecar is only on level 2: it is found because level 1 has none
    assert_eq!(
        text_of(&member(&meta, "bench_tool_image_ref")),
        "registry/tool:1"
    );
    assert_eq!(
        text_of(&member(&meta, "bench_tool_image_digest")),
        "sha256:ab"
    );
    assert_eq!(
        text_of(&member(&meta, "bench_tool_image_digest_source")),
        "measured(docker image inspect .RepoDigests)"
    );
    assert_eq!(
        text_of(&member(&meta, "bench_endpoint")),
        "http://host:8000"
    );
    // removing level 1's measurement makes level 2 the first readable one
    std::fs::remove_file(cell.scratch.path("sweep_x/level_01/measured.json")).expect("remove");
    let _ = done_line(&produce(&cell.env().with("COMPLETED", "2 1")));
    let meta = meta_of(&cell);
    assert_eq!(text_of(&member(&meta, "bench_tool")), "guidellm");
    assert_eq!(text_of(&member(&meta, "bench_tool_version")), "0.5.1");
    assert_eq!(
        text_of(&member(&meta, "bench_tool_version_source")),
        "measured(pip)"
    );
    assert_eq!(member(&meta, "bench_max_error_rate").repr(), "0.02");
    assert!(matches!(
        member(&meta, "server_alive_at_bench_end").peel(),
        PyValue::Bool(true)
    ));
    assert_eq!(text_of(&member(&meta, "boundary_exemption")), "none");
}

fn meta_of(cell: &Cell) -> PyValue {
    meta(&cell.index())
}

#[test]
fn the_dataset_provenance_counts_the_runs_and_sorts_the_column_map() {
    let cell = Cell::new("dataset");
    cell.scratch.write(
        "sweep_x/level_01/dataset_x.json",
        "{\"dataset_kind\": \"hf\", \"dataset_source\": \"hf://org/data\", \"dataset_id\": \"org/data\", \"dataset_revision\": \"r1\", \"source_sha256\": \"ab12\", \"preprocess_strategy\": \"cut\", \"seed\": 7, \"column_map\": {\"b\": \"x\", \"a\": \"y\"}, \"tokenizer\": \"tok\", \"duplicate_sampling\": false}",
    );
    cell.scratch.write(
        "sweep_x/level_01/run_02/dataset_x.json",
        "{\"dataset_kind\": \"hf\", \"duplicate_sampling\": true}",
    );
    cell.scratch.write(
        "sweep_x/level_01/other/dataset_x.json",
        "{\"dataset_kind\": \"ignored\"}",
    );
    let _ = done_line(&produce(&cell.env()));
    let meta = meta_of(&cell);
    let expected = [
        ("dataset_kind", "\"hf\""),
        ("dataset_source", "\"hf://org/data\""),
        ("dataset_id", "\"org/data\""),
        ("dataset_revision", "\"r1\""),
        ("dataset_source_sha256", "\"ab12\""),
        ("dataset_preprocess_strategy", "\"cut\""),
        ("dataset_seed", "7"),
        ("dataset_duplicate_sampling", "true"),
        ("dataset_tokenizer", "\"tok\""),
    ];
    for (key, json) in expected {
        assert_eq!(
            member(&meta, key)
                .dumps_unicode()
                .expect("JSON")
                .to_utf8()
                .expect("UTF-8"),
            json,
            "{key}"
        );
    }
    assert_eq!(
        text_of(&member(&meta, "dataset_column_map")),
        "{\"a\": \"y\", \"b\": \"x\"}"
    );
    assert_eq!(
        text_of(&member(&meta, "dataset_provenance")),
        "measured(런별 사이드카 dataset_x.json · 2 런)"
    );
    // the keys come in the original's order, after the tool block
    let order = keys(&meta);
    let at = order
        .iter()
        .position(|key| key == "dataset_kind")
        .expect("dataset block");
    assert_eq!(order[at - 1], "bench_endpoint");
    assert_eq!(order.last().map(String::as_str), Some("dataset_provenance"));
}

#[test]
fn a_synthetic_dataset_leaves_the_duplicate_sampling_flag_empty() {
    let cell = Cell::new("synthetic");
    cell.scratch.write(
        "sweep_x/level_01/dataset_x.json",
        "{\"dataset_kind\": \"synthetic\", \"duplicate_sampling\": true}",
    );
    let _ = done_line(&produce(&cell.env()));
    let meta = meta_of(&cell);
    assert!(matches!(
        member(&meta, "dataset_duplicate_sampling").peel(),
        PyValue::None
    ));
    assert!(matches!(
        member(&meta, "dataset_column_map").peel(),
        PyValue::None
    ));
}

#[test]
fn a_dataset_sidecar_that_is_empty_is_replaced_by_the_next_one_as_the_original_does() {
    let cell = Cell::new("dataset-empty");
    cell.scratch.write("sweep_x/level_01/dataset_x.json", "{}");
    cell.scratch.write(
        "sweep_x/level_01/run_01/dataset_x.json",
        "{\"dataset_kind\": \"hf\"}",
    );
    let _ = done_line(&produce(&cell.env()));
    let meta = meta_of(&cell);
    assert_eq!(text_of(&member(&meta, "dataset_kind")), "hf");
    assert_eq!(
        text_of(&member(&meta, "dataset_provenance")),
        "measured(런별 사이드카 dataset_x.json · 2 런)"
    );
}

#[test]
fn a_sidecar_that_is_not_an_object_ends_the_run_with_the_attribute_error_and_writes_nothing() {
    for (document, shown) in [
        ("[1]", "list"),
        ("null", "NoneType"),
        ("3", "int"),
        ("\"s\"", "str"),
    ] {
        let cell = Cell::new("not-object");
        cell.scratch
            .write("sweep_x/level_01/measured.json", document);
        let produced = produce(&cell.env());
        assert_eq!(
            uncaught(&produced),
            (
                "AttributeError".to_string(),
                format!("'{shown}' object has no attribute 'get'")
            ),
            "{document}"
        );
        assert!(cell.index_text().is_none());
    }
}

#[test]
fn a_native_serve_proof_names_the_image_only_when_it_passed_and_nothing_measured_it() {
    let cell = Cell::new("proof");
    cell.scratch.write(".env.x", "");
    cell.scratch.write(
        "serve_proof_x.json",
        "{\"plane\": \"native\", \"status\": \"PASS\", \"wheelhouse\": {\"source_image_tag\": \"  easy-vllm:7.1.2-src  \"}}",
    );
    let env = cell.env().with("SERVE_PLANE", "native");
    let _ = done_line(&produce(&env));
    let meta = meta_of(&cell);
    assert_eq!(text_of(&member(&meta, "image_tag")), "easy-vllm:7.1.2-src");
    assert_eq!(
        text_of(&member(&meta, "image_tag_source")),
        "measured(native serve proof · wheelhouse 원천 이미지)"
    );
    assert_eq!(text_of(&member(&meta, "vllm_version")), "7.1.2");
    // a measured image wins over the proof, a failed proof or another plane leaves the declaration
    let _ = done_line(&produce(
        &env.with("IMAGE_TAG_ACTUAL", "easy-vllm:measured"),
    ));
    assert_eq!(
        text_of(&member(&meta_of(&cell), "image_tag")),
        "easy-vllm:measured"
    );
    cell.scratch.write(
        "serve_proof_x.json",
        "{\"plane\": \"native\", \"status\": \"FAIL\", \"wheelhouse\": {\"source_image_tag\": \"easy-vllm:7.1.2-src\"}}",
    );
    let _ = done_line(&produce(&cell.env().with("SERVE_PLANE", "native")));
    let meta = meta_of(&cell);
    assert_eq!(text_of(&member(&meta, "image_tag")), "NA");
    assert_eq!(
        text_of(&member(&meta, "image_tag_source")),
        "declared(envfile)"
    );
    // a tag that is not text has no strip: the original ends with the attribute error
    cell.scratch.write(
        "serve_proof_x.json",
        "{\"plane\": \"native\", \"status\": \"PASS\", \"wheelhouse\": {\"source_image_tag\": 5}}",
    );
    let produced = produce(&cell.env().with("SERVE_PLANE", "native"));
    assert_eq!(
        uncaught(&produced),
        (
            "AttributeError".to_string(),
            "'int' object has no attribute 'strip'".to_string()
        )
    );
}

#[test]
fn the_measured_image_and_digest_are_inherited_by_a_reassembly_only_when_they_were_measured() {
    let cell = Cell::new("inherit-image");
    let env = cell
        .env()
        .with("IMAGE_TAG_ACTUAL", "easy-vllm:measured")
        .with("IMAGE_DIGEST_ACTUAL", "sha256:cafe");
    let _ = done_line(&produce(&env));
    let meta = meta_of(&cell);
    assert_eq!(text_of(&member(&meta, "image_digest")), "sha256:cafe");
    assert_eq!(
        text_of(&member(&meta, "image_digest_source")),
        "measured(docker inspect .Image)"
    );
    // docker is not asked again: a changed environment value is ignored, the recorded one is taken
    let _ = done_line(&produce(
        &env.with("IMAGE_TAG_ACTUAL", "easy-vllm:other")
            .with("REASSEMBLE", "1"),
    ));
    let meta = meta_of(&cell);
    assert_eq!(text_of(&member(&meta, "image_tag")), "easy-vllm:measured");
    assert_eq!(text_of(&member(&meta, "image_digest")), "sha256:cafe");
    // a recorded value whose source was not a measurement is not inherited as one
    let mut index = std::fs::read_to_string(cell.index_path()).expect("index");
    index = index.replace(
        "\"image_tag_source\": \"measured(docker inspect)\"",
        "\"image_tag_source\": \"declared(envfile)\"",
    );
    std::fs::write(cell.index_path(), index).expect("write");
    let _ = done_line(&produce(&cell.env().with("REASSEMBLE", "1")));
    let meta = meta_of(&cell);
    assert_eq!(text_of(&member(&meta, "image_tag")), "easy-vllm:9.9.9-cell");
    assert_eq!(
        text_of(&member(&meta, "image_tag_source")),
        "declared(envfile)"
    );
}

#[test]
fn a_reassembly_without_a_usable_prior_index_stops_and_leaves_the_prior_file_alone() {
    for prior in [
        None,
        Some("not json"),
        Some("[]"),
        Some("{\"generated_utc\": \"\"}"),
    ] {
        let cell = Cell::new("no-prior");
        if let Some(text) = prior {
            std::fs::write(cell.index_path(), text).expect("write");
        }
        let produced = produce(&cell.env().with("REASSEMBLE", "1"));
        assert_eq!(
            produced.ending,
            sweep_stop::assemble_sweep_meta::Ending::Reason(pystr(
                "[sweep_bench] --reassemble-only: 기존 index 의 generated_utc 부재 — 측정시각 승계 불가(날조 금지) → 중단"
            )),
            "{prior:?}"
        );
        assert_eq!(cell.index_text().as_deref(), prior);
    }
}

#[test]
fn the_levels_follow_python_integers_and_sort_numerically() {
    let cell = Cell::new("levels");
    for directory in ["level_02", "level_10", "level_-1"] {
        cell.scratch.write(
            &format!("sweep_x/{directory}/measured.json"),
            "{\"measurement_ok\": true}",
        );
    }
    let produced = produce(&cell.env().with("COMPLETED", "+10 1_0\t2 -1\u{3000}٣"));
    // 10, 10, 2, -1 and an Arabic-Indic digit three: the digit has no level directory but is still a level
    assert!(done_line(&produced).ends_with("(완료 레벨 [-1, 2, 3, 10, 10] · 절삭 0건)"));
    let levels = member(&cell.index(), "levels");
    let PyValue::List(entries) = levels.peel() else {
        panic!("levels")
    };
    let measured: Vec<bool> = entries
        .iter()
        .map(|entry| !matches!(member(entry, "measured").peel(), PyValue::None))
        .collect();
    assert_eq!(measured, [true, true, false, true, true]);
    assert!(text_of(&member(&entries[0], "bench_json")).ends_with("/sweep_x/level_-1/bench_x.json"));
    let broken = produce(&cell.env().with("COMPLETED", "1 x"));
    assert_eq!(
        uncaught(&broken),
        (
            "ValueError".to_string(),
            "invalid literal for int() with base 10: 'x'".to_string()
        )
    );
}

#[test]
fn the_tensor_parallel_size_comes_from_the_config_then_the_manifest_with_a_single_node_counted_once(
) {
    let cell = Cell::new("tp");
    let size = |cell: &Cell| text_of(&member(&meta_of(cell), "tensor_parallel_size"));
    let run = |cell: &Cell, env: MapEnv| {
        let _ = done_line(&produce(&env));
        size(cell)
    };
    assert_eq!(run(&cell, cell.env()), "1");
    cell.scratch.write(
        "cfg.yaml",
        "model: /m/A\ntensor-parallel-size: 4 # declared\n",
    );
    assert_eq!(run(&cell, cell.env()), "4");
    cell.scratch.write("cfg.yaml", "model: /m/A\n");
    cell.scratch.write(
        "manifest.yaml",
        "gpu_model: GB10\ngpus_per_node: 4\ntopology: single\nnodes:\n  - role: main\n  - role: sub\n",
    );
    assert_eq!(
        run(&cell, cell.env()),
        "4",
        "a control peer is not a tensor worker"
    );
    cell.scratch.write(
        "manifest.yaml",
        "gpu_model: GB10\ngpus_per_node: 4\ntopology: multi\nnodes:\n  - role: main\n  - role: sub\n",
    );
    assert_eq!(run(&cell, cell.env().with("TOPO", "multi")), "8");
    cell.scratch.write(
        "manifest.yaml",
        "gpu_model: GB10\ngpus_per_node: x\ntopology: multi\n  - role: a\n",
    );
    assert_eq!(
        run(&cell, cell.env().with("TOPO", "multi")),
        "1",
        "an unreadable count falls back to one"
    );
    cell.scratch.write("manifest.yaml", "gpu_model: GB10\n");
    assert_eq!(run(&cell, cell.env().with("TOPO", "single")), "1");
}

#[test]
fn the_model_axis_falls_back_loudly_and_the_measured_node_says_where_it_came_from() {
    let cell = Cell::new("model");
    cell.scratch.write("cfg.yaml", "max-model-len: 1\n");
    cell.scratch
        .write(".env.x", "SERVING_MODEL_NAME=Combo-Name\nVLLM_PLE_MMAP=1\n");
    let produced = produce(&cell.env());
    let _ = done_line(&produced);
    assert_eq!(
        stderr_text(&produced),
        "[sweep_bench] ⚠ config yaml 에 model 경로가 없어 강한키를 조합명으로 폴백했다 (model_source=fallback(serving_model_name)) — 인증서 모델 축이 조합명이 된다\n"
    );
    let meta = meta_of(&cell);
    assert_eq!(text_of(&member(&meta, "model")), "Combo-Name");
    assert_eq!(text_of(&member(&meta, "serving_config")), "Combo-Name");
    assert_eq!(text_of(&member(&meta, "ple_mode")), "mmap");
    assert_eq!(
        text_of(&member(&meta, "ple_mode_source")),
        "declared(envfile VLLM_PLE_MMAP=1)"
    );
    cell.scratch.write(".env.x", "");
    let _ = done_line(&produce(&cell.env()));
    let meta = meta_of(&cell);
    assert_eq!(text_of(&member(&meta, "model")), "x");
    assert_eq!(
        text_of(&member(&meta, "model_source")),
        "fallback(config_name)"
    );
    // a trailing slash and capitals: the basename, lower-cased
    cell.scratch
        .write("cfg.yaml", "model: /models/Acme/Big-MODEL//\n");
    let _ = done_line(&produce(&cell.env()));
    assert_eq!(text_of(&member(&meta_of(&cell), "model")), "big-model");
    for (role, node, source) in [
        ("self_role: sub", "sub", "derived(manifest.self_role=sub)"),
        (
            "self_role: main",
            "main",
            "derived(manifest.self_role=main)",
        ),
        (
            "self_role: observer",
            "main",
            "derived(manifest.self_role=observer)",
        ),
        ("x: y", "main", "defaulted(self_role absent)"),
    ] {
        cell.scratch
            .write("manifest.yaml", &format!("gpu_model: GB10\n{role}\n"));
        let _ = done_line(&produce(&cell.env()));
        let meta = meta_of(&cell);
        assert_eq!(text_of(&member(&meta, "measured_node")), node);
        assert_eq!(text_of(&member(&meta, "measured_node_source")), source);
    }
    let _ = done_line(&produce(&cell.env().with("TOPO", "multi")));
    assert_eq!(
        text_of(&member(&meta_of(&cell), "measured_node")),
        "cluster"
    );
}

#[test]
fn the_vllm_version_follows_the_documented_precedence() {
    let cell = Cell::new("version");
    let version = |env: MapEnv| {
        let _ = done_line(&produce(&env));
        text_of(&member(&meta_of(&cell), "vllm_version"))
    };
    assert_eq!(
        version(
            cell.env()
                .with("VLLM_VER", "1.2.3")
                .with("EASY_VLLM_VERSION", "4.5.6")
        ),
        "1.2.3"
    );
    assert_eq!(
        version(cell.env().with("EASY_VLLM_VERSION", "4.5.6")),
        "4.5.6"
    );
    assert_eq!(version(cell.env()), "9.9.9", "the declared image line");
    cell.scratch.write(".env.x", "");
    assert_eq!(version(cell.env()), "0.27.1", "the engine's own report");
    cell.scratch.write("sweep_x/level_01/engine_x.log", "");
    cell.scratch.write(
        "cfg.yaml",
        "# rendered for vLLM   3.2.1 images\nmodel: /m/A\n",
    );
    assert_eq!(version(cell.env()), "3.2.1", "the config comment");
    cell.scratch.write("cfg.yaml", "model: /m/A\n");
    assert_eq!(version(cell.env()), "NA");
}

#[test]
fn the_backends_are_measured_first_and_a_disagreement_is_said_on_standard_error() {
    let cell = Cell::new("backends");
    cell.scratch.write(
        "sweep_x/level_01/engine_x.log",
        &format!(
            "{ENGINE_LINE}INFO Using TRITON_ATTN attention backend out of potential backends: ['TRITON_ATTN']\nINFO [mxfp4.py:352] Using 'MARLIN' Mxfp4 MoE backend.\n"
        ),
    );
    cell.scratch.write(
        "cfg.yaml",
        "model: /m/A\nmoe-backend: triton\nquantization: none\n",
    );
    cell.scratch.write(
        "x.sh",
        "#!/bin/bash\nexport VLLM_ATTENTION_BACKEND=FLASHINFER\n",
    );
    let produced = produce(&cell.env());
    let _ = done_line(&produced);
    assert_eq!(
        stderr_text(&produced),
        "[sweep_bench] ⚠ 선언↔실측 불일치 — quantization:YES(measured=fp8 declared=none) · kv_cache_dtype:unknown · attention_backend:YES(measured=triton_attn declared=FLASHINFER) · moe_backend:YES(measured=marlin declared=triton)\n[sweep_bench] ⓘ 어텐션 축 무선택 — 후보 ['TRITON_ATTN'] (요청 FLASHINFER 는 실효 없음)\n"
    );
    let meta = meta_of(&cell);
    for (key, want) in [
        ("attention_backend", "triton_attn"),
        ("attention_backend_source", "measured(engine log)"),
        ("attention_backend_declared", "FLASHINFER"),
        ("moe_backend", "marlin"),
        ("moe_backend_declared", "triton"),
    ] {
        assert_eq!(text_of(&member(&meta, key)), want, "{key}");
    }
    assert_eq!(
        member(&meta, "attention_backend_candidates").repr(),
        "['TRITON_ATTN']"
    );
}

#[test]
fn the_lite_block_is_the_owners_figures_and_the_raw_records_own_verdict() {
    let cell = Cell::new("lite");
    let raw = lite_raw_from_fixture(&cell, "pass");
    let _ = done_line(&produce(&cell.env().with("LITE_RAW", &raw)));
    let lite = member(&cell.index(), "lite");
    assert_eq!(
        keys(&lite),
        [
            "raw_json",
            "lite_verdict",
            "gen_tps",
            "gen_src",
            "cold_ttft_ms",
            "kv_gib",
            "capacity",
            "accept_len",
            "accept_len_src",
            "table",
            "engine"
        ]
    );
    assert_eq!(text_of(&member(&lite, "raw_json")), raw);
    assert_eq!(text_of(&member(&lite, "lite_verdict")), "pass");
    assert!(
        text_of(&member(&lite, "table")).contains('|'),
        "the owner's rendered table"
    );
    // no stored verdict reads as none, and an absent file is no block at all
    let without = lite_raw_from_fixture(&cell, "unknown-word");
    let _ = done_line(&produce(&cell.env().with("LITE_RAW", &without)));
    assert!(matches!(
        member(&member(&cell.index(), "lite"), "lite_verdict").peel(),
        PyValue::None
    ));
    let _ = done_line(&produce(
        &cell.env().with("LITE_RAW", &cell.path("absent.json")),
    ));
    assert!(matches!(
        member(&cell.index(), "lite").peel(),
        PyValue::None
    ));
}

#[test]
fn a_lite_record_that_cannot_be_built_is_recorded_and_a_system_exit_of_the_owner_ends_the_run() {
    let cell = Cell::new("lite-errors");
    cell.scratch.write("lite_raw.json", "{\"broken\"");
    let _ = done_line(&produce(
        &cell.env().with("LITE_RAW", &cell.path("lite_raw.json")),
    ));
    let lite = member(&cell.index(), "lite");
    assert_eq!(keys(&lite), ["raw_json", "error"]);
    assert_eq!(
        text_of(&member(&lite, "error")),
        "lite_metrics 산정 실패: Expecting ':' delimiter: line 1 column 10 (char 9)"
    );
    // a record with no declared topology: the owner's SystemExit is not an Exception, so it ends the run
    cell.scratch.write("lite_raw.json", "{}");
    let _ = std::fs::remove_file(cell.index_path());
    let produced = produce(&cell.env().with("LITE_RAW", &cell.path("lite_raw.json")));
    match &produced.ending {
        sweep_stop::assemble_sweep_meta::Ending::Reason(reason) => assert!(
            reason
                .to_utf8()
                .expect("UTF-8")
                .starts_with("[lite_metrics] FAIL: topology 가 선언되지 않았다(None)"),
            "{}",
            reason.to_utf8().expect("UTF-8")
        ),
        other => panic!("{other:?}"),
    }
    assert!(cell.index_text().is_none());
}

#[test]
fn the_repeat_request_and_the_clamp_boundary_come_from_the_drivers_variables() {
    let cell = Cell::new("repeats");
    cell.scratch.write(
        "sweep_x/level_02/repeat_run.json",
        "{\"level\": 2, \"run\": 1, \"run_bench_rc\": 1}",
    );
    let env = cell
        .env()
        .with("REPEATS", "3")
        .with("REPEATS_SOURCE", "campaign declaration")
        .with("REPEAT_KIND", "warm-rerun")
        .with("CLAMP_LEVEL", "2");
    let _ = done_line(&produce(&env));
    let repetition = member(&cell.index(), "repetition");
    assert_eq!(member(&repetition, "requested").repr(), "3");
    assert_eq!(
        text_of(&member(&repetition, "requested_source")),
        "campaign declaration"
    );
    assert_eq!(text_of(&member(&repetition, "kind")), "warm-rerun");
    assert_eq!(
        member(&member(&repetition, "clamp_run"), "run_bench_rc").repr(),
        "1"
    );
    // a clamp record that is not an object is recorded as unreadable, with the reason
    cell.scratch
        .write("sweep_x/level_02/repeat_run.json", "[1]");
    let _ = done_line(&produce(&env));
    let clamp = member(&member(&cell.index(), "repetition"), "clamp_run");
    assert!(text_of(&member(&clamp, "unreadable"))
        .ends_with("/sweep_x/level_02/repeat_run.json: 객체가 아니다"));
    assert_eq!(member(&clamp, "level").repr(), "2");
    // a count that is not an integer is ignored; one that only looks like a digit to isdigit() is an error
    let _ = done_line(&produce(&cell.env().with("REPEATS", "x3")));
    assert!(matches!(
        member(&member(&cell.index(), "repetition"), "requested").peel(),
        PyValue::None
    ));
    let produced = produce(&cell.env().with("REPEATS", "²"));
    assert_eq!(
        uncaught(&produced),
        (
            "ValueError".to_string(),
            "invalid literal for int() with base 10: '²'".to_string()
        )
    );
    // a reassembly takes the request of the measurement time from the index, not from the variables
    let _ = done_line(&produce(&env));
    let _ = done_line(&produce(
        &cell.env().with("REASSEMBLE", "1").with("REPEATS", "9"),
    ));
    let repetition = member(&cell.index(), "repetition");
    assert_eq!(member(&repetition, "requested").repr(), "3");
}

#[test]
fn the_measurement_shape_is_read_from_the_record_and_inherited_by_a_reassembly() {
    let cell = Cell::new("knobs");
    let _ = done_line(&produce(&cell.env()));
    assert_eq!(
        member(&member(&cell.index(), "bench_knobs"), "preset").repr(),
        "'cell'"
    );
    let _ = done_line(&produce(
        &cell
            .env()
            .with("REASSEMBLE", "1")
            .with("KNOB_RECORD", "/nonexistent-record"),
    ));
    assert_eq!(
        member(&member(&cell.index(), "bench_knobs"), "preset").repr(),
        "'cell'"
    );
    // a prior index without the record: the absence is said in its own words
    let text = cell.index_text().expect("index");
    let start = text
        .find(",\n  \"bench_knobs\"")
        .expect("bench_knobs member");
    std::fs::write(cell.index_path(), format!("{}\n}}", &text[..start])).expect("write");
    let _ = done_line(&produce(&cell.env().with("REASSEMBLE", "1")));
    assert_eq!(
        text_of(&member(&member(&cell.index(), "bench_knobs"), "absent")),
        "측정 시점 index 에 bench_knobs 없음(프리셋 신설 전 측정 · plan_26101011)"
    );
    // an unset variable names itself as the reason
    let mut env = MapEnv::new();
    for (name, value) in cell.env().pairs() {
        if name != "KNOB_RECORD" {
            env = env.with(name, &value);
        }
    }
    let produced = produce(&env);
    assert_eq!(
        produced.ending,
        sweep_stop::assemble_sweep_meta::Ending::Reason(pystr(
            "[sweep_bench] bench_knobs 기록을 읽지 못했다('KNOB_RECORD') — 해소 기록 없는 index 를 내지 않는다"
        ))
    );
}

#[test]
fn a_missing_required_variable_is_the_key_error_of_the_original() {
    let cell = Cell::new("required");
    for name in [
        "CONFIG", "TOPO", "SWEEPDIR", "CFGYAML", "EF", "MANIFEST", "ILEN",
    ] {
        let mut env = MapEnv::new();
        for (own, value) in cell.env().pairs() {
            if own != name {
                env = env.with(own, &value);
            }
        }
        let produced = produce(&env);
        assert_eq!(
            uncaught(&produced),
            ("KeyError".to_string(), format!("'{name}'")),
            "{name}"
        );
        assert!(cell.index_text().is_none(), "{name}");
    }
}

#[test]
fn an_unwritable_index_is_the_open_error_and_an_unencodable_text_leaves_the_chunks_before_it() {
    let cell = Cell::new("write");
    std::fs::remove_dir_all(cell.scratch.path("sweep_x/level_01")).expect("remove");
    let produced = produce(&cell.env().with("COMPLETED", ""));
    assert_eq!(
        produced.ending,
        sweep_stop::assemble_sweep_meta::Ending::Done(pystr(&format!(
            "[sweep_bench] sweep_index.json → {} (완료 레벨 [] · 절삭 0건)",
            cell.path("sweep_x/sweep_index.json")
        )))
    );
    std::fs::remove_dir_all(cell.scratch.path("sweep_x")).expect("remove");
    let produced = produce(&cell.env().with("COMPLETED", ""));
    let (class, message) = uncaught(&produced);
    assert_eq!(class, "FileNotFoundError");
    assert_eq!(
        message,
        format!(
            "[Errno 2] No such file or directory: '{}'",
            cell.path("sweep_x/sweep_index.json")
        )
    );

    // a lone surrogate in a dict value: the file keeps everything before that string and nothing of it
    let cell = Cell::new("surrogate-value");
    cell.scratch.write(
        "sweep_x/level_01/measured.json",
        "{\"measurement_ok\": true, \"bench_tool\": \"a\\ud800b\"}",
    );
    let produced = produce(&cell.env());
    assert_eq!(
        uncaught(&produced),
        (
            "UnicodeEncodeError".to_string(),
            "'utf-8' codec can't encode character '\\ud800' in position 2: surrogates not allowed"
                .to_string()
        )
    );
    let partial = cell
        .index_text()
        .expect("the chunks before the failing one were written");
    assert!(partial.ends_with("    \"bench_tool\": "), "{partial:?}");
    assert!(partial.starts_with("{\n  \"config\": \"x\""));

    // in a list the chunk carries the separator before the string: the failing position counts it
    let cell = Cell::new("surrogate-list");
    cell.scratch.write(
        "sweep_x/level_01/measured.json",
        "{\"measurement_ok\": true, \"notes\": [\"x\", \"a\\ud800\"]}",
    );
    let produced = produce(&cell.env());
    assert_eq!(
        uncaught(&produced),
        (
            "UnicodeEncodeError".to_string(),
            "'utf-8' codec can't encode character '\\ud800' in position 14: surrogates not allowed"
                .to_string()
        )
    );
    assert!(cell.index_text().expect("partial").ends_with("\"x\""));
}

#[test]
fn a_prior_index_survives_every_failure_that_happens_before_the_file_is_opened_for_writing() {
    let cell = Cell::new("prior-survives");
    let _ = done_line(&produce(&cell.env()));
    let kept = cell.index_text().expect("index");
    let mut broken = MapEnv::new();
    for (name, value) in cell.env().pairs() {
        broken = broken.with(name, if name == "ILEN" { "bad" } else { &value });
    }
    let produced = produce(&broken);
    assert_eq!(uncaught(&produced).0, "ValueError");
    assert_eq!(cell.index_text().as_deref(), Some(kept.as_str()));
    let gone = cell.env().with("KNOB_RECORD", &cell.path("absent.json"));
    let _ = produce(&gone);
    assert_eq!(cell.index_text().as_deref(), Some(kept.as_str()));
}

#[test]
fn a_manifest_without_the_gpu_model_ends_with_status_two_and_the_original_message() {
    let cell = Cell::new("gpu");
    cell.scratch.write("manifest.yaml", "topology: single\n");
    let produced = produce(&cell.env());
    assert_eq!(
        produced.ending,
        sweep_stop::assemble_sweep_meta::Ending::Status(2)
    );
    assert_eq!(
        stderr_text(&produced),
        format!(
            "[sweep_bench] FAIL: manifest({}) 에 gpu_model 이 없다 — 인증서 강한 키 gpu 를 NA 로 발행하지 않는다. terraforming_node 스캔(서브면 scan_node.py --emit-sub-manifest)으로 채워라.\n",
            cell.path("manifest.yaml")
        )
    );
    assert!(cell.index_text().is_none());
}

// ── the program ───────────────────────────────────────────────────────────────────

fn run_program(arguments: &[&str], environment: &[(&str, String)]) -> std::process::Output {
    let mut command = Command::new(PRODUCER);
    command.args(arguments).env_clear();
    for (name, value) in environment {
        command.env(name, value);
    }
    command.output().expect("the program starts")
}

#[test]
fn the_program_runs_with_no_path_at_all_and_prints_the_index_line() {
    let cell = Cell::new("program");
    let env = cell.env();
    let output = run_program(&[], &env.pairs());
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8(output.stdout).expect("UTF-8"),
        format!(
            "[sweep_bench] sweep_index.json → {} (완료 레벨 [1] · 절삭 0건)\n",
            cell.path("sweep_x/sweep_index.json")
        )
    );
    assert!(output.stderr.is_empty());
    let stamp = text_of(&member(&cell.index(), "generated_utc"));
    assert!(
        parse_utc(&PyValue::str(&stamp), "generated_utc").is_ok(),
        "{stamp}"
    );
}

#[test]
fn an_uncaught_exception_ends_the_program_with_the_traceback_marker_class_and_message() {
    let cell = Cell::new("traceback");
    let env = cell.env();
    let environment: Vec<(&str, String)> = env
        .pairs()
        .into_iter()
        .map(|(name, value)| {
            (
                name,
                if name == "ILEN" {
                    "bad".to_string()
                } else {
                    value
                },
            )
        })
        .collect();
    let output = run_program(&[], &environment);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).expect("UTF-8"),
        "Traceback (most recent call last):\n  (native: Python stack frames are not reproduced)\nValueError: invalid literal for int() with base 10: 'bad'\n"
    );
    let missing = run_program(&[], &[]);
    assert_eq!(missing.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&missing.stderr).ends_with("KeyError: 'CONFIG'\n"));
}

#[test]
fn the_start_up_check_runs_the_real_producer_and_unknown_arguments_are_refused() {
    let output = run_program(&["--self-test"], &[]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("UTF-8");
    assert!(
        stdout.starts_with("[assemble_sweep_meta] self-test OK — "),
        "{stdout}"
    );
    assert!(output.stderr.is_empty());
    let refused = run_program(&["--bogus"], &[]);
    assert_eq!(refused.status.code(), Some(2));
    assert_eq!(
        String::from_utf8(refused.stderr).expect("UTF-8"),
        "usage: assemble_sweep_meta [--self-test]\nassemble_sweep_meta: error: unrecognized arguments: --bogus\n"
    );
}

fn launcher() -> PathBuf {
    closure_root()
        .join(SKILL)
        .join("scripts/assemble_sweep_meta.sh")
}

#[test]
fn the_installed_launcher_runs_the_program_it_is_pointed_at_and_refuses_a_missing_one_with_status_two(
) {
    let launcher = launcher();
    assert!(
        launcher.is_file(),
        "NOT_RUN: no installed launcher at {}",
        launcher.display()
    );
    let run = |binary: &str| {
        Command::new("bash")
            .arg(&launcher)
            .arg("--self-test")
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("ASSEMBLE_SWEEP_META_BIN", binary)
            .output()
            .expect("bash starts")
    };
    let ok = run(PRODUCER);
    assert_eq!(ok.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&ok.stdout).contains("self-test OK"));
    let scratch = Scratch::new("launcher");
    scratch.write("not_executable", "#!/bin/sh\n");
    for missing in [scratch.path("absent"), scratch.path("not_executable")] {
        let output = run(&missing.to_string_lossy());
        assert_eq!(output.status.code(), Some(2), "{}", missing.display());
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        assert!(
            stderr.contains("[assemble_sweep_meta] ERROR Rust 실행파일이 없다/실행 불가: "),
            "{stderr}"
        );
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn the_launcher_forwards_the_environment_unchanged_and_the_program_never_sees_a_python_path_dependency(
) {
    let cell = Cell::new("through-launcher");
    let mut command = Command::new("bash");
    command
        .arg(launcher())
        .env_clear()
        .env("ASSEMBLE_SWEEP_META_BIN", PRODUCER);
    for (name, value) in cell.env().pairs() {
        command.env(name, value);
    }
    let output = command.output().expect("bash starts");
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(cell.index_text().is_some());
    let binary = std::fs::read(PRODUCER).expect("program");
    let needle = b"python3";
    let mentions = binary
        .windows(needle.len())
        .filter(|window| *window == needle)
        .count();
    // the program may name python in its diagnostics; it must not run it: the run above had no PATH to find it by
    assert!(mentions < 20, "{mentions}");
}

// ── the word class and the read sequence ──────────────────────────────────────────

#[test]
fn a_combining_mark_that_python_does_not_call_a_word_character_is_none_in_every_word_boundary_expression(
) {
    let cell = Cell::new("word-class");
    // U+0345 (combining Greek ypogegrammeni) is alphabetic in Rust but neither `isalnum()` nor `\w` in CPython 3.12.3
    let mark = "\u{345}";
    cell.scratch.write(".env.x", "");
    cell.scratch.write(
        "sweep_x/level_01/engine_x.log",
        &format!("{mark}v8.9.10\nUsing MARLIN {mark} MoE backend\n"),
    );
    cell.scratch.write(
        "cfg.yaml",
        &format!("model: /m/A\nspeculative-config: num_speculative_tokens{mark}3\n"),
    );
    let _ = done_line(&produce(&cell.env()));
    let meta = meta_of(&cell);
    assert_eq!(
        text_of(&member(&meta, "vllm_build")),
        "8.9.10",
        "`\\b` before the v"
    );
    assert_eq!(
        text_of(&member(&meta, "moe_backend")),
        "NA",
        "`\\w+` cannot take the mark as a word"
    );
    assert_eq!(
        member(&meta, "spec_declared_k").repr(),
        "3",
        "`\\W+` takes the mark as a separator"
    );
}

#[test]
fn the_word_class_is_cpythons_isalnum_for_the_characters_that_differ_from_rusts() {
    use sweep_stop::sweep_meta_scan::engine_version;
    let version = |text: &str| engine_version(&text.chars().collect::<Vec<_>>());
    // word characters before the v hide the boundary; combining marks, spaces and punctuation do not
    for word_before in ["a", "9", "_", "é", "٣", "\u{2160}", "\u{b2}"] {
        assert_eq!(
            version(&format!("{word_before}v1.2.3")),
            None,
            "{word_before:?}"
        );
    }
    for not_word in ["\u{345}", "\u{300}", "\u{93e}", " ", "-", "(", "\u{200b}"] {
        assert_eq!(
            version(&format!("{not_word}v1.2.3")).as_deref(),
            Some("1.2.3"),
            "{not_word:?}"
        );
    }
}

/// The writer side of the next test, finite by construction: it opens the pipe once (non-blocking, retried only until a
/// deadline), writes the first document, replaces the symlink by a regular second document **before** closing the pipe,
/// and closes. If the producer never reaches the pipe it still releases a reader that may arrive late, until `done`.
fn serve_first_read_from_a_pipe_then_replace_it(
    pipe: &Path,
    link: &Path,
    first: &str,
    second: &str,
    done: &std::sync::atomic::AtomicBool,
) -> Result<(), String> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    use std::sync::atomic::Ordering;
    const NON_BLOCKING: i32 = 0o4000; // O_NONBLOCK on Linux
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let release = |reason: String| -> Result<(), String> {
        // opening a pipe for reading and writing never blocks and lets a reader that waits in `open` go on
        let end = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !done.load(Ordering::SeqCst) && std::time::Instant::now() < end {
            drop(
                std::fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .custom_flags(NON_BLOCKING)
                    .open(pipe),
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        Err(reason)
    };
    // `ENXIO` (no reader yet) is the only error that is retried; the producer's open of the symlink is the reader
    let mut writer = loop {
        match std::fs::OpenOptions::new()
            .write(true)
            .custom_flags(NON_BLOCKING)
            .open(pipe)
        {
            Ok(file) => break file,
            Err(error)
                if error.raw_os_error() == Some(6) && std::time::Instant::now() < deadline =>
            {
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            Err(error) => {
                return release(format!("the pipe was never opened for reading: {error}"))
            }
        }
    };
    let outcome = (|| -> Result<(), String> {
        writer
            .write_all(first.as_bytes())
            .map_err(|error| error.to_string())?;
        let next = link.with_extension("next");
        std::fs::write(&next, second).map_err(|error| error.to_string())?;
        std::fs::rename(&next, link).map_err(|error| error.to_string())
    })();
    drop(writer); // the reader's end of file comes only after the replacement is in place
    outcome
}

/// What this proves: the original reads the first readable `measured.json` once per loop and the three loops read it
/// separately. The first read is held on a pipe (document one, then end of file) while the link is replaced by a
/// regular file (document two) before that pipe is closed; every later read follows the regular file. A producer that
/// kept its first read for the server-alive block would report document one's `true`; the level record likewise comes
/// from the later read. It does not distinguish the second read from the third (their documents are the same file).
#[test]
fn a_second_read_of_the_first_readable_measurement_sees_what_replaced_it_after_the_first_read() {
    let cell = Cell::new("reads");
    let link = cell.scratch.path("sweep_x/level_01/measured.json");
    let pipe = cell.scratch.path("sweep_x/level_01/measured.pipe");
    std::fs::remove_file(&link).expect("remove");
    assert!(Command::new("mkfifo")
        .arg(&pipe)
        .status()
        .expect("mkfifo")
        .success());
    std::os::unix::fs::symlink(&pipe, &link).expect("link");
    let first = "{\"measurement_ok\": true, \"bench_tool\": \"first\", \"server_alive_at_bench_end\": true}";
    let second = "{\"measurement_ok\": true, \"bench_tool\": \"second\", \"server_alive_at_bench_end\": false, \"boundary_exemption\": \"second\"}";
    let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let writer = {
        let (pipe, link, done) = (pipe.clone(), link.clone(), done.clone());
        std::thread::spawn(move || {
            serve_first_read_from_a_pipe_then_replace_it(&pipe, &link, first, second, &done)
        })
    };
    let produced = produce(&cell.env());
    done.store(true, std::sync::atomic::Ordering::SeqCst);
    let served = writer.join().expect("the writer thread ends");
    assert_eq!(served, Ok(()), "the premise of the test held");
    let _ = done_line(&produced);
    let index = cell.index();
    let meta = meta(&index);
    assert_eq!(
        text_of(&member(&meta, "bench_tool")),
        "first",
        "the tool block took the first read"
    );
    assert!(
        matches!(
            member(&meta, "server_alive_at_bench_end").peel(),
            PyValue::Bool(false)
        ),
        "the server-alive block must take its own, later read"
    );
    assert_eq!(text_of(&member(&meta, "boundary_exemption")), "second");
    let levels = member(&index, "levels");
    let PyValue::List(entries) = levels.peel() else {
        panic!("levels")
    };
    assert_eq!(
        text_of(&member(&member(&entries[0], "measured"), "bench_tool")),
        "second"
    );
}

#[test]
fn model_and_measured_values_are_lowered_with_cpythons_str_lower() {
    let cell = Cell::new("lower");
    // model basename: final sigma in context, and a scalar Unicode 15 does not know stays as it is
    cell.scratch.write("cfg.yaml", "model: /m/\u{39f}\u{394}\u{3a5}\u{3a3}\u{3a3}\u{395}\u{3a5}\u{3a3}-\u{1c89}\nquantization: \u{130}X\n");
    cell.scratch.write(
        "sweep_x/level_01/engine_x.log",
        "Initializing a V1 LLM engine (v1.2.3) with config: quantization='\u{130}X', kv_cache_dtype=auto\n",
    );
    let _ = done_line(&produce(&cell.env()));
    let meta = meta_of(&cell);
    assert_eq!(
        text_of(&member(&meta, "model")),
        "\u{3bf}\u{3b4}\u{3c5}\u{3c3}\u{3c3}\u{3b5}\u{3c5}\u{3c2}-\u{1c89}"
    );
    // the one-to-many mapping: the measured token and the declaration lower to the same text, so no mismatch
    assert_eq!(text_of(&member(&meta, "quantization")), "i\u{307}x");
    assert_eq!(text_of(&member(&meta, "quantization_declared")), "\u{130}X");
    assert_eq!(text_of(&member(&meta, "quantization_mismatch")), "no");
}

#[test]
fn the_off_sentinel_of_a_speculative_declaration_is_lowered_with_cpythons_str_lower() {
    use sweep_stop::sweep_meta_scan::spec_declaration;
    let chars = |text: &str| text.chars().collect::<Vec<_>>();
    // `NONE` in capitals is the sentinel; a capital I with dot above lowers to two scalars and so is not one
    let off = spec_declaration(true, &chars("speculative-config: NONE\n"), &[]).expect("scan");
    assert_eq!(off.state, "off");
    let on =
        spec_declaration(true, &chars("speculative-config: NONE\u{130}\n"), &[]).expect("scan");
    assert_eq!(on.state, "on");
}
