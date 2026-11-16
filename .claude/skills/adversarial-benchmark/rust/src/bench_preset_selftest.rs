//! The deterministic rejection suite of the preset contract: a Rust port of `selftest_bench_preset.py`.
//!
//! It is the review-time enforcer of "values may be tuned, keys may not" and of "the grade says who opens a choice":
//! K (schema snapshot), V (loader rejections), R (resolution rejections and records) and D (dataset axis). The suite
//! runs the same functions the program runs ([`crate::bench_preset`]) against the real installed `config/` (the
//! default preset and the bindings), the real campaign schema and the real campaign owner, copied into a temporary
//! directory where the checks change files; nothing in the installation is touched. A missing installed file is a
//! failure of the suite, not a skipped check.

use crate::bench_preset::{
    self, dumps_sorted, load_bindings, load_preset, resolve, resolve_text, serving_caps,
    validate_preset, CallDepth, ResolveArguments, SkillDir, BINDINGS_REL, DATASET_KNOBS, KNOB_KEYS,
    KNOB_SPECS, OPTION_KEYS, TOP_KEYS,
};
use crate::bench_preset_cli::Runtime;
use crate::cli::TRACEBACK_HEADER;
use crate::guidellm::Failure;
use crate::pyfs;
use crate::pysort::py_eq;
use crate::pystr::PyStr;
use crate::pyvalue::{PyDict, PyValue};
use crate::repeat_axis::SCHEMA_REL;
use crate::repeat_owner::{ConfiguredCampaignOwner, RepoRoot};
use crate::FULL_REPEATS_MIN;
use ring::digest::{digest, SHA256};
use std::path::{Path, PathBuf};

/// What the suite printed and how it ended.
#[derive(Debug)]
pub struct SelfTestReport {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

// schema snapshot (tripwire: a change must be reviewed together with the preset and the documents)
const SNAP_TOP: [&str; 5] = ["schema_version", "bench", "tool", "reason_codes", "knobs"];
const SNAP_KNOB: [&str; 6] = [
    "owner",
    "kind",
    "default",
    "serving_cap",
    "options",
    "reasons",
];
const SNAP_KNOBS: [&str; 17] = [
    "levels",
    "input_len",
    "output_len",
    "num_prompts",
    "warmups",
    "repeats",
    "backend",
    "bench_budget_mib",
    "dataset_kind",
    "dataset_source",
    "dataset_id",
    "dataset_path",
    "dataset_revision",
    "source_sha256",
    "column_map",
    "preprocess_strategy",
    "seed",
];

// ── value helpers ───────────────────────────────────────────────────────────────

fn s(text: &str) -> PyValue {
    PyValue::str(text)
}

fn n(number: i64) -> PyValue {
    PyValue::int(number)
}

fn ints(numbers: &[i64]) -> PyValue {
    PyValue::List(numbers.iter().map(|&number| n(number)).collect())
}

fn dict(pairs: Vec<(&str, PyValue)>) -> PyValue {
    let mut map = PyDict::new();
    for (key, value) in pairs {
        map.set(key, value);
    }
    PyValue::Dict(map)
}

/// `value[path...]`, `None` when a step is missing (the suite only reads documents it knows are shaped).
fn at(value: &PyValue, path: &[&str]) -> PyValue {
    let mut current = value.clone();
    for key in path {
        current = match current.peel() {
            PyValue::Dict(map) => map.get(key).cloned().unwrap_or(PyValue::None),
            _ => PyValue::None,
        };
    }
    current
}

fn is(value: &PyValue, text: &str) -> bool {
    matches!(value.peel(), PyValue::Str(own) if own.equals(text))
}

fn is_int(value: &PyValue, number: i64) -> bool {
    py_eq(value, &n(number))
}

fn starts_with(value: &PyValue, prefix: &str) -> bool {
    match value.peel() {
        PyValue::Str(text) => text.to_utf8().is_some_and(|own| own.starts_with(prefix)),
        _ => false,
    }
}

fn contains_text(value: &PyValue, needle: &str) -> bool {
    match value.peel() {
        PyValue::Str(text) => text.to_utf8().is_some_and(|own| own.contains(needle)),
        _ => false,
    }
}

/// One step into a document.
#[derive(Clone, Copy)]
enum Seg<'a> {
    Key(&'a str),
}

/// What to do at the end of a path.
enum Edit {
    Set(&'static str, PyValue),
    Remove(&'static str),
    Append(PyValue),
    SetIndex(usize, PyValue),
}

/// A copy of `value` with `edit` applied at `path` (the suite's `copy.deepcopy` plus mutation).
fn edited(value: &PyValue, path: &[Seg<'_>], edit: &Edit) -> PyValue {
    match path.split_first() {
        None => match (value.peel(), edit) {
            (PyValue::Dict(map), Edit::Set(key, new)) => {
                let mut copy = map.clone();
                copy.set(key, new.clone());
                PyValue::Dict(copy)
            }
            (PyValue::Dict(map), Edit::Remove(key)) => {
                let mut copy = PyDict::new();
                for (name, entry) in map.iter() {
                    if !name.equals(key) {
                        copy.insert(name.clone(), entry.clone());
                    }
                }
                PyValue::Dict(copy)
            }
            (PyValue::List(items), Edit::Append(new)) => {
                let mut copy = items.clone();
                copy.push(new.clone());
                PyValue::List(copy)
            }
            (PyValue::List(items), Edit::SetIndex(index, new)) => {
                let mut copy = items.clone();
                copy[*index] = new.clone();
                PyValue::List(copy)
            }
            _ => value.clone(),
        },
        Some((Seg::Key(key), rest)) => match value.peel() {
            PyValue::Dict(map) => {
                let mut copy = PyDict::new();
                for (name, entry) in map.iter() {
                    let entry = if name.equals(key) {
                        edited(entry, rest, edit)
                    } else {
                        entry.clone()
                    };
                    copy.insert(name.clone(), entry);
                }
                PyValue::Dict(copy)
            }
            _ => value.clone(),
        },
    }
}

fn k<'a>(name: &'a str) -> Seg<'a> {
    Seg::Key(name)
}

fn same_keys(value: &PyValue, wanted: &[&str]) -> bool {
    match value.peel() {
        PyValue::Dict(map) => {
            map.len() == wanted.len() && wanted.iter().all(|name| map.get(name).is_some())
        }
        _ => false,
    }
}

// ── the suite ───────────────────────────────────────────────────────────────────

struct Suite {
    out: String,
    fails: Vec<String>,
}

impl Suite {
    fn ck(&mut self, label: &str, cond: bool, detail: &dyn Fn() -> String) {
        if cond {
            self.out.push_str(&format!("  [PASS] {label}\n"));
        } else {
            self.out
                .push_str(&format!("  [FAIL] {label} — {}\n", detail()));
            self.fails.push(label.to_string());
        }
    }
}

/// The failures that are the suite's own business: a `PresetError` is a rejection, anything else ends the suite
/// (the original lets it propagate).
fn rejected<T>(result: Result<T, Failure>) -> Result<bool, Failure> {
    match result {
        Ok(_) => Ok(false),
        Err(failure) if failure.class == bench_preset::PRESET_ERROR => Ok(true),
        Err(other) => Err(other),
    }
}

/// A scratch directory removed when the suite ends.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Result<Self, Failure> {
        let base = std::env::temp_dir();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos());
        let path = base.join(format!("bench-preset.{}.{nanos}", std::process::id()));
        std::fs::create_dir(&path).map_err(|error| Failure::new("OSError", &error.to_string()))?;
        Ok(Scratch(path))
    }

    fn text(&self, rel: &str) -> PyStr {
        use std::os::unix::ffi::OsStrExt;
        PyStr::from_surrogateescape(self.0.join(rel).as_os_str().as_bytes())
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn copy_tree(from: &Path, to: &Path) -> Result<(), Failure> {
    std::fs::create_dir_all(to).map_err(|error| Failure::new("OSError", &error.to_string()))?;
    let entries = std::fs::read_dir(from)
        .map_err(|error| Failure::new("OSError", &format!("{}: {error}", from.display())))?;
    for entry in entries {
        let entry = entry.map_err(|error| Failure::new("OSError", &error.to_string()))?;
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target).map_err(|error| {
                Failure::new("OSError", &format!("{}: {error}", entry.path().display()))
            })?;
        }
    }
    Ok(())
}

fn write(path: &Path, text: &str) -> Result<(), Failure> {
    std::fs::write(path, text).map_err(|error| Failure::new("OSError", &error.to_string()))
}

fn to_path(text: &PyStr) -> PathBuf {
    use std::os::unix::ffi::OsStringExt;
    PathBuf::from(std::ffi::OsString::from_vec(
        text.to_surrogateescape_bytes().unwrap_or_default(),
    ))
}

/// The last line of the traceback of an exception that ends the suite. The suite reaches the preset program as the
/// module `bench_preset` (the original imports it, whether run standalone or through `--self-test`), so a
/// `PresetError` that escapes it is shown with that module name; every other class is shown as it is.
fn uncaught_line(failure: &Failure) -> PyStr {
    if failure.class != bench_preset::PRESET_ERROR {
        return failure.line();
    }
    let mut line = PyStr::new("bench_preset.");
    line.push_py(&failure.line());
    line
}

/// Runs the suite against the installation the runtime names.
pub fn run_self_test(runtime: &Runtime) -> SelfTestReport {
    let mut suite = Suite {
        out: String::new(),
        fails: Vec::new(),
    };
    match scenarios(&mut suite, runtime) {
        Ok(()) => {}
        Err(failure) => {
            return SelfTestReport {
                stdout: suite.out,
                stderr: format!(
                    "{TRACEBACK_HEADER}{}\n",
                    uncaught_line(&failure).to_utf8().unwrap_or_default()
                ),
                exit_code: 1,
            }
        }
    }
    if !suite.fails.is_empty() {
        let labels = PyValue::List(suite.fails.iter().map(|label| s(label)).collect()).repr();
        return SelfTestReport {
            stdout: suite.out,
            stderr: format!(
                "[selftest_bench_preset] FAIL {}건: {labels}\n",
                suite.fails.len()
            ),
            exit_code: 1,
        };
    }
    suite.out.push_str(
        "[selftest_bench_preset] OK — K1~K4 · V1~V15 · R1~R18(+R5b·R5c·R11e) · D1~D11 전부 통과\n",
    );
    SelfTestReport {
        stdout: suite.out,
        stderr: String::new(),
        exit_code: 0,
    }
}

/// One `resolve` call of the suite: the defaults of the original's `res()` helper (the backend choice every call
/// needs) with the caller's changes.
struct Call<'a> {
    cli: Vec<(&'a str, &'a str)>,
    reasons: Vec<(&'a str, &'a str)>,
    serving: Option<&'a PyValue>,
    tool: &'a str,
    cli_pins: Vec<(&'a str, &'a str)>,
    pinned_by: Option<&'a str>,
    approved_by: Option<&'a str>,
    repeats_source: Option<&'a str>,
}

impl<'a> Default for Call<'a> {
    fn default() -> Self {
        Call {
            cli: Vec::new(),
            reasons: Vec::new(),
            serving: None,
            tool: "vllm",
            cli_pins: Vec::new(),
            pinned_by: None,
            approved_by: None,
            repeats_source: None,
        }
    }
}

struct Resolved {
    values: PyDict,
    block: PyValue,
}

impl Resolved {
    fn value(&self, name: &str) -> PyValue {
        self.values.get(name).cloned().unwrap_or(PyValue::None)
    }

    fn knob(&self, name: &str, field: &str) -> PyValue {
        at(&self.block, &["knobs", name, field])
    }
}

fn text_dict(pairs: &[(&str, &str)]) -> PyValue {
    dict(pairs.iter().map(|(key, value)| (*key, s(value))).collect())
}

fn scenarios(suite: &mut Suite, runtime: &Runtime) -> Result<(), Failure> {
    let loaded = load_preset(&PyValue::None, &runtime.skill_dir, CallDepth::API)?;
    let base = loaded.document.clone();
    let bindings = loaded.bindings.clone();
    let none_detail = || String::new();

    // K — the schema is fixed
    suite.ck(
        "K1 닫힌 키 표 = 스냅샷",
        TOP_KEYS == SNAP_TOP
            && KNOB_KEYS == SNAP_KNOB
            && KNOB_SPECS
                .iter()
                .map(|spec| spec.name)
                .eq(SNAP_KNOBS.iter().copied()),
        &none_detail,
    );
    let knobs_doc = at(&base, &["knobs"]);
    let all_knobs_fixed = match knobs_doc.peel() {
        PyValue::Dict(map) => map.iter().all(|(_, knob)| same_keys(knob, &SNAP_KNOB)),
        _ => false,
    };
    suite.ck(
        "K2 배포 default_preset 통과 · 키 집합이 표와 정확히 같다",
        same_keys(&base, &SNAP_TOP) && same_keys(&knobs_doc, &SNAP_KNOBS) && all_knobs_fixed,
        &none_detail,
    );
    suite.ck(
        "K3 바인딩 full-bench → 존재하는 도구 폴더",
        py_eq(&bindings, &dict(vec![("full-bench", s("guide-llm"))])),
        &|| bindings.repr(),
    );
    let schema_path = pyfs::join_text(runtime.repo_root.get()?, &PyStr::new(SCHEMA_REL));
    let schema_bytes = pyfs::read_bytes(&schema_path)?;
    let schema_text = crate::pyjson::decode_file_text(&schema_bytes)
        .map_err(|error| Failure::new("UnicodeDecodeError", &error.to_string()))?;
    let schema = crate::pyjson::loads_text(&schema_text)
        .map_err(|error| Failure::new("JSONDecodeError", &error.to_string()))?;
    let pn = at(
        &schema,
        &[
            "definitions",
            "budgets",
            "properties",
            "bench_pins",
            "propertyNames",
            "enum",
        ],
    );
    let wanted: Vec<&str> = KNOB_SPECS
        .iter()
        .map(|spec| spec.name)
        .filter(|name| *name != "repeats")
        .collect();
    let pn_ok = match pn.peel() {
        PyValue::List(items) => {
            items.len() == wanted.len()
                && wanted
                    .iter()
                    .all(|name| items.iter().any(|item| is(item, name)))
        }
        _ => false,
    };
    suite.ck(
        "K4 campaign.schema budgets.bench_pins 키 = knob 표 − repeats(반복 수 핀은 budgets.repeats 하나)",
        pn_ok,
        &|| pn.repr(),
    );

    // V — the loader rejects
    let viol = |path: &[Seg<'_>], edit: Edit| -> Result<bool, Failure> {
        let changed = edited(&base, path, &edit);
        Ok(!validate_preset(&changed, &bindings)?.is_empty())
    };
    suite.ck(
        "V1 미지 최상위 키(꺼 둔 vllm-bench 블록) 거부",
        viol(&[], Edit::Set("vllm_bench", dict(vec![("burst_n", n(3))])))?,
        &none_detail,
    );
    suite.ck(
        "V2 knob 누락 거부",
        viol(&[k("knobs")], Edit::Remove("warmups"))?,
        &none_detail,
    );
    suite.ck(
        "V3 knob 미지 키 거부",
        viol(&[k("knobs"), k("levels")], Edit::Set("note", s("x")))?,
        &none_detail,
    );
    suite.ck(
        "V4 기본값이 열거 밖 거부",
        viol(&[k("knobs"), k("input_len")], Edit::Set("default", n(1000)))?,
        &none_detail,
    );
    suite.ck(
        "V5 미정의 근거 코드 참조 거부",
        viol(
            &[k("knobs"), k("input_len"), k("reasons")],
            Edit::Append(s("vibes")),
        )?,
        &none_detail,
    );
    suite.ck(
        "V6 user knob 비기본 값이 auto 면 거부",
        viol(
            &[k("knobs"), k("output_len"), k("options")],
            Edit::SetIndex(0, dict(vec![("value", n(128)), ("grade", s("auto"))])),
        )?,
        &none_detail,
    );
    suite.ck(
        "V7 repeats 선택지 < full 하한 거부",
        viol(
            &[k("knobs"), k("repeats"), k("options")],
            Edit::Append(dict(vec![
                ("value", n(i64::from(FULL_REPEATS_MIN) - 1)),
                ("grade", s("escalate")),
            ])),
        )?,
        &none_detail,
    );
    suite.ck(
        "V8 기본값 없음 knob(backend)에 값 거부",
        viol(
            &[k("knobs"), k("backend")],
            Edit::Set("default", s("openai")),
        )?,
        &none_detail,
    );
    suite.ck(
        "V9 bench 불일치 거부",
        viol(&[], Edit::Set("bench", s("lite-bench")))?,
        &none_detail,
    );
    suite.ck(
        "V10 기본값이 confirm 이면 거부",
        viol(
            &[k("knobs"), k("input_len")],
            Edit::Set("default", n(16384)),
        )?,
        &none_detail,
    );
    suite.ck(
        "V11 잘못된 등급 이름 거부",
        viol(
            &[k("knobs"), k("levels"), k("options")],
            Edit::SetIndex(0, dict(vec![("value", n(1)), ("grade", s("maybe"))])),
        )?,
        &none_detail,
    );
    suite.ck(
        "V12 levels 기본값에 판정점 1 없음 거부",
        viol(
            &[k("knobs"), k("levels")],
            Edit::Set("default", ints(&[2, 4])),
        )?,
        &none_detail,
    );
    suite.ck(
        "V13 허용되지 않은 상한 필터 거부",
        viol(
            &[k("knobs"), k("output_len")],
            Edit::Set("serving_cap", s(bench_preset::CAP_LE_MAX_NUM_SEQS)),
        )?,
        &none_detail,
    );

    let scratch = Scratch::new()?;
    // an isolated skill directory (copies of the bindings and presets): the installation is not touched
    let skill = scratch.text("skill");
    let skill_path = to_path(&skill);
    let installed_config = to_path(&pyfs::join(runtime.skill_dir.get()?, "config"));
    copy_tree(&installed_config, &skill_path.join("config"))?;
    let bindings_file = skill_path.join(BINDINGS_REL);
    let isolated = SkillDir::given(skill.clone());
    write(
        &bindings_file,
        "full-bench: guide-llm\nlite-bench: vllm-bench\n",
    )?;
    suite.ck(
        "V14 바인딩 키 추가 거부",
        rejected(load_bindings(&isolated, CallDepth::API))?,
        &none_detail,
    );
    write(&bindings_file, "full-bench: no-such-tool\n")?;
    suite.ck(
        "V15 바인딩이 없는 폴더를 가리키면 거부",
        rejected(load_bindings(&isolated, CallDepth::API))?,
        &none_detail,
    );
    write(&bindings_file, "full-bench: guide-llm\n")?;
    let custom = edited(
        &base,
        &[k("knobs"), k("input_len")],
        &Edit::Set("default", n(8192)),
    );
    let long_ctx = skill_path.join("config/guide-llm/long_ctx.yaml");
    // JSON is YAML: the preset is written as the JSON text of the document
    write(
        &long_ctx,
        &custom
            .dumps_ascii()
            .map_err(|e| Failure::new(e.python_type(), &e.to_string()))?,
    )?;
    let named = load_preset(&s("long_ctx"), &isolated, CallDepth::API)?;
    suite.ck(
        "R14a 커스텀 프리셋 이름으로 선택 · 같은 검증 통과",
        named
            .path
            .to_utf8()
            .is_some_and(|path| path.ends_with("/long_ctx.yaml"))
            && is_int(
                &at(&named.document, &["knobs", "input_len", "default"]),
                8192,
            ),
        &none_detail,
    );
    let long_ctx_text = {
        use std::os::unix::ffi::OsStrExt;
        PyStr::from_surrogateescape(long_ctx.as_os_str().as_bytes())
    };
    let by_path = load_preset(
        &PyValue::Str(long_ctx_text.clone()),
        &isolated,
        CallDepth::API,
    )?;
    suite.ck(
        "R14b 커스텀 프리셋 경로로 선택",
        by_path.path == resolve_text(&long_ctx_text),
        &none_detail,
    );
    let bad = edited(
        &custom,
        &[k("knobs"), k("input_len")],
        &Edit::Set("extra", n(1)),
    );
    write(
        &skill_path.join("config/guide-llm/bad.yaml"),
        &bad.dumps_ascii()
            .map_err(|e| Failure::new(e.python_type(), &e.to_string()))?,
    )?;
    suite.ck(
        "R14c 계약 위반 커스텀 프리셋은 선택되지 않는다",
        rejected(load_preset(&s("bad"), &isolated, CallDepth::API))?,
        &none_detail,
    );
    suite.ck(
        "R14d 없는 프리셋 이름 거부",
        rejected(load_preset(&s("nope"), &isolated, CallDepth::API))?,
        &none_detail,
    );

    // an isolated repository: the campaign owner copy; campaign pins are changed between the checks
    let root = scratch.text("repo");
    let root_path = to_path(&root);
    // the campaign owner is its native launcher (the retired campaign_init.py may be absent)
    let owner_copy = root_path.join(crate::repeat_owner::CAMPAIGN_LAUNCHER_REL);
    std::fs::create_dir_all(owner_copy.parent().expect("owner path has a parent"))
        .map_err(|error| Failure::new("OSError", &error.to_string()))?;
    let installed_owner = pyfs::join_text(runtime.repo_root.get()?, &PyStr::new(crate::repeat_owner::CAMPAIGN_LAUNCHER_REL));
    std::fs::copy(to_path(&installed_owner), &owner_copy).map_err(|error| {
        Failure::new("OSError", &format!("{}: {error}", installed_owner.repr()))
    })?;
    let camps = root_path.join("campaigns");
    std::fs::create_dir(&camps).map_err(|error| Failure::new("OSError", &error.to_string()))?;
    let repo_root = RepoRoot::given(root.clone());
    let owner: &ConfiguredCampaignOwner = &runtime.owner;

    let set_campaign = |budgets: Option<PyValue>| -> Result<(), Failure> {
        let Some(budgets) = budgets else {
            let _ = std::fs::remove_file(camps.join("ACTIVE"));
            return Ok(());
        };
        std::fs::create_dir_all(camps.join("camp-x"))
            .map_err(|error| Failure::new("OSError", &error.to_string()))?;
        let document = dict(vec![("id", s("camp-x")), ("budgets", budgets)]);
        write(
            &camps.join("camp-x/campaign.yaml"),
            &document
                .dumps_ascii()
                .map_err(|e| Failure::new(e.python_type(), &e.to_string()))?,
        )?;
        write(&camps.join("ACTIVE"), "camp-x\n")
    };

    let serv = dict(vec![
        ("max_num_seqs", n(2)),
        ("max_model_len", n(4096)),
        ("source", s("declared(fixture)")),
    ]);
    let with_model_len = |len: i64| {
        dict(vec![
            ("max_num_seqs", n(2)),
            ("max_model_len", n(len)),
            ("source", s("declared(fixture)")),
        ])
    };
    let res_doc = |doc: &PyValue,
                   name: &str,
                   call: &Call<'_>,
                   dry_run: bool,
                   cli_base: &[(&str, &str)],
                   reasons_base: &[(&str, &str)]|
     -> Result<Resolved, Failure> {
        let mut cli: Vec<(&str, &str)> = cli_base.to_vec();
        for (key, value) in &call.cli {
            cli.retain(|(existing, _)| existing != key);
            cli.push((key, value));
        }
        let mut reasons: Vec<(&str, &str)> = reasons_base.to_vec();
        for (key, value) in &call.reasons {
            reasons.retain(|(existing, _)| existing != key);
            reasons.push((key, value));
        }
        let cli = text_dict(&cli);
        let reasons = text_dict(&reasons);
        let pins = text_dict(&call.cli_pins);
        let optional = |text: Option<&str>| text.map_or(PyValue::None, s);
        let (pinned_by, approved_by, repeats_source) = (
            optional(call.pinned_by),
            optional(call.approved_by),
            optional(call.repeats_source),
        );
        let preset_name = s(name);
        let tool = s(call.tool);
        let serving = call.serving.cloned().unwrap_or_else(|| serv.clone());
        let no_campaign = PyValue::None;
        let dry = PyValue::Bool(dry_run);
        let outcome = resolve(
            &ResolveArguments {
                preset_doc: doc,
                preset_name: &preset_name,
                tool: &tool,
                cli: &cli,
                cli_pins: &pins,
                pinned_by: &pinned_by,
                knob_reasons: &reasons,
                approved_by: &approved_by,
                campaign_id: &no_campaign,
                serving: &serving,
                repo_root: &repo_root,
                repeats_source: &repeats_source,
                dry_run: &dry,
            },
            owner,
        )?;
        Ok(Resolved {
            values: outcome.values,
            block: PyValue::Dict(outcome.block),
        })
    };
    let need: &[(&str, &str)] = &[("backend", "openai")];
    let rs: &[(&str, &str)] = &[("backend", "harmony_completion")];
    let res = |call: &Call<'_>| res_doc(&base, "default_preset", call, false, need, rs);

    set_campaign(None)?;
    let r = res(&Call::default())?;
    let filtered: Vec<PyValue> = match r.knob("levels", "filtered_out").peel() {
        PyValue::List(items) => items.iter().map(|item| at(item, &["value"])).collect(),
        _ => Vec::new(),
    };
    suite.ck(
        "R1 기본값 + 상한 필터: max-num-seqs 2 → levels [1,2] · 4·8·16 제외 기록",
        py_eq(&r.value("levels"), &ints(&[1, 2]))
            && py_eq(&PyValue::List(filtered), &ints(&[4, 8, 16]))
            && is(&r.knob("levels", "source"), "preset")
            && is(&r.knob("levels", "reason"), "preset_default"),
        &|| r.knob("levels", "source").repr(),
    );
    suite.ck(
        "R1b 반복 수 = 프리셋 기본 3 · repeat_axis 출처",
        is_int(&r.value("repeats"), 3)
            && is(&r.knob("repeats", "source"), "preset")
            && starts_with(
                &r.knob("repeats", "repeats_source"),
                "defaulted(preset default_preset",
            ),
        &|| r.knob("repeats", "repeats_source").repr(),
    );
    suite.ck(
        "R2 Agent 비기본 선택에 근거 코드 없음 → 거부",
        rejected(res(&Call {
            cli: vec![("input_len", "2048")],
            ..Call::default()
        }))?,
        &none_detail,
    );
    let r = res(&Call {
        cli: vec![("input_len", "2048")],
        reasons: vec![("input_len", "serving_context")],
        ..Call::default()
    })?;
    suite.ck(
        "R2b 근거 코드 동반 Agent 선택 → source agent · reason 기록",
        is_int(&r.value("input_len"), 2048)
            && is(&r.knob("input_len", "source"), "agent")
            && is(&r.knob("input_len", "reason"), "serving_context"),
        &none_detail,
    );
    let big = with_model_len(65536);
    suite.ck(
        "R3 confirm(16384)을 승인 없이 Agent 가 고름 → 거부",
        rejected(res(&Call {
            cli: vec![("input_len", "16384")],
            reasons: vec![("input_len", "serving_context")],
            serving: Some(&big),
            ..Call::default()
        }))?,
        &none_detail,
    );
    let r = res(&Call {
        cli: vec![("input_len", "16384")],
        reasons: vec![("input_len", "serving_context")],
        serving: Some(&big),
        approved_by: Some("사용자(plan 승인 팝업 전사) — \"승인\""),
        ..Call::default()
    })?;
    suite.ck(
        "R3b confirm + 사전 승인 전사 → 통과 · approved_by 기록",
        is_int(&r.value("input_len"), 16384)
            && contains_text(&r.knob("input_len", "approved_by"), "승인"),
        &none_detail,
    );
    suite.ck(
        "R4 escalate(32768)를 Agent 가 고름 → 거부(승인 전사가 있어도)",
        rejected(res(&Call {
            cli: vec![("input_len", "32768")],
            reasons: vec![("input_len", "serving_context")],
            serving: Some(&big),
            approved_by: Some("x"),
            ..Call::default()
        }))?,
        &none_detail,
    );
    let r = res(&Call {
        serving: Some(&big),
        cli_pins: vec![("input_len", "32768")],
        pinned_by: Some("사용자 — \"32k 로\""),
        ..Call::default()
    })?;
    suite.ck(
        "R4b escalate 를 사람 핀(--pinned-by)이 연다 · reason human_pin",
        is_int(&r.value("input_len"), 32768)
            && is(&r.knob("input_len", "source"), "cli_pin")
            && is(&r.knob("input_len", "reason"), "human_pin"),
        &none_detail,
    );
    suite.ck(
        "R5 열거 밖 값은 사람 핀이어도 거부",
        rejected(res(&Call {
            cli_pins: vec![("input_len", "3000")],
            pinned_by: Some("사용자"),
            ..Call::default()
        }))?,
        &none_detail,
    );
    suite.ck(
        "R5b 귀속 없는 --pin 거부",
        rejected(res(&Call {
            cli_pins: vec![("input_len", "2048")],
            ..Call::default()
        }))?,
        &none_detail,
    );
    suite.ck(
        "R5c Agent 값과 사람 핀이 다르면 거부",
        rejected(res(&Call {
            cli: vec![("input_len", "1024")],
            reasons: vec![("input_len", "prior_cell_shape")],
            cli_pins: vec![("input_len", "2048")],
            pinned_by: Some("사용자"),
            ..Call::default()
        }))?,
        &none_detail,
    );
    suite.ck(
        "R15 user knob(output_len 512)을 Agent 가 고름 → 거부(escalate)",
        rejected(res(&Call {
            cli: vec![("output_len", "512")],
            reasons: vec![("output_len", "preset_default")],
            ..Call::default()
        }))?,
        &none_detail,
    );
    set_campaign(Some(dict(vec![(
        "bench_pins",
        dict(vec![("input_len", n(2048))]),
    )])))?;
    let r = res(&Call::default())?;
    suite.ck(
        "R6 캠페인 핀 → source campaign · reason human_pin",
        is_int(&r.value("input_len"), 2048)
            && is(&r.knob("input_len", "source"), "campaign")
            && is(&at(&r.block, &["campaign"]), "camp-x"),
        &none_detail,
    );
    suite.ck(
        "R6b Agent 선택이 캠페인 핀과 다르면 거부(조용히 덮어쓰지 않는다)",
        rejected(res(&Call {
            cli: vec![("input_len", "1024")],
            reasons: vec![("input_len", "prior_cell_shape")],
            ..Call::default()
        }))?,
        &none_detail,
    );
    let mid = with_model_len(8192);
    let r = res(&Call {
        serving: Some(&mid),
        cli_pins: vec![("input_len", "4096")],
        pinned_by: Some("사용자 — \"4k\""),
        ..Call::default()
    })?;
    suite.ck(
        "R7 CLI 사람 핀 > 캠페인 핀 · 덮은 값 기록",
        is_int(&r.value("input_len"), 4096)
            && is_int(&r.knob("input_len", "overrode_campaign_pin"), 2048),
        &none_detail,
    );
    set_campaign(Some(dict(vec![(
        "bench_pins",
        dict(vec![("repeats", n(4))]),
    )])))?;
    suite.ck(
        "R8 bench_pins.repeats(두 번째 자리) 거부",
        rejected(res(&Call::default()))?,
        &none_detail,
    );
    set_campaign(Some(dict(vec![(
        "bench_pins",
        dict(vec![("no_such", n(1))]),
    )])))?;
    suite.ck(
        "R8b bench_pins 미지 knob 거부",
        rejected(res(&Call::default()))?,
        &none_detail,
    );
    set_campaign(Some(dict(vec![("repeats", n(5))])))?;
    let r = res(&Call::default())?;
    suite.ck(
        "R11 캠페인 budgets.repeats 5 → source campaign(escalate 를 사람 선언이 연다)",
        is_int(&r.value("repeats"), 5) && is(&r.knob("repeats", "source"), "campaign"),
        &none_detail,
    );
    set_campaign(None)?;
    suite.ck(
        "R11b 반복 수 4 를 귀속 없이 → Agent 선택 · escalate 거부",
        rejected(res(&Call {
            cli: vec![("repeats", "4")],
            ..Call::default()
        }))?,
        &none_detail,
    );
    let r = res(&Call {
        cli: vec![("repeats", "4")],
        repeats_source: Some("sweep state declared_budget.repeats"),
        ..Call::default()
    })?;
    suite.ck(
        "R11c --repeats-source 귀속 → cli_pin",
        is_int(&r.value("repeats"), 4) && is(&r.knob("repeats", "source"), "cli_pin"),
        &none_detail,
    );
    suite.ck(
        "R11d 반복 수 2 → full 정의 위반 거부",
        rejected(res(&Call {
            cli: vec![("repeats", "2")],
            repeats_source: Some("x"),
            ..Call::default()
        }))?,
        &none_detail,
    );
    suite.ck(
        "R9 backend 부재 → 거부(기본값 없음)",
        rejected(res_doc(
            &base,
            "default_preset",
            &Call::default(),
            false,
            &[],
            &[],
        ))?,
        &none_detail,
    );
    let r = res(&Call {
        cli_pins: vec![("repeats", "4")],
        pinned_by: Some("사용자 — \"4회\""),
        ..Call::default()
    })?;
    suite.ck(
        "R11e 반복 수 사람 핀(--pin repeats=4) → cli_pin",
        is_int(&r.value("repeats"), 4) && is(&r.knob("repeats", "source"), "cli_pin"),
        &none_detail,
    );
    suite.ck(
        "R9b guidellm 인데 예산 부재 → 거부",
        rejected(res(&Call {
            tool: "guidellm",
            ..Call::default()
        }))?,
        &none_detail,
    );
    let r = res(&Call {
        tool: "vllm",
        ..Call::default()
    })?;
    suite.ck(
        "R9c vllm 도구는 예산 not_applicable 로 기록",
        matches!(r.value("bench_budget_mib").peel(), PyValue::None)
            && is(&r.knob("bench_budget_mib", "source"), "not_applicable"),
        &none_detail,
    );
    suite.ck(
        "R9d 예산 16384(confirm)를 승인 없이 → 거부",
        rejected(res(&Call {
            cli: vec![("bench_budget_mib", "16384")],
            reasons: vec![("bench_budget_mib", "host_budget")],
            tool: "guidellm",
            ..Call::default()
        }))?,
        &none_detail,
    );
    suite.ck(
        "R10 input+output > max-model-len → 거부",
        rejected(res(&Call {
            cli: vec![("input_len", "4096")],
            reasons: vec![("input_len", "serving_context")],
            ..Call::default()
        }))?,
        &none_detail,
    );
    suite.ck(
        "R12 Agent 선택이 아닌 knob 에 --knob-reason → 거부",
        rejected(res(&Call {
            reasons: vec![("warmups", "preset_default")],
            ..Call::default()
        }))?,
        &none_detail,
    );
    suite.ck(
        "R12b 다른 knob 의 근거 코드를 쓰면 거부",
        rejected(res(&Call {
            cli: vec![("input_len", "2048")],
            reasons: vec![("input_len", "host_budget")],
            ..Call::default()
        }))?,
        &none_detail,
    );
    let missing_serving = scratch.text("nope.yaml");
    suite.ck(
        "R13 실측에서 서빙 yaml 부재 → 거부",
        rejected(serving_caps(
            &PyValue::Str(missing_serving.clone()),
            &PyValue::Bool(false),
        ))?,
        &none_detail,
    );
    let caps = serving_caps(&PyValue::Str(missing_serving), &PyValue::Bool(true))?;
    suite.ck(
        "R13b dry-run 은 unavailable 로 기록",
        matches!(at(&caps, &["max_num_seqs"]).peel(), PyValue::None)
            && starts_with(&at(&caps, &["source"]), "unavailable("),
        &none_detail,
    );
    let nolim = dict(vec![
        ("max_num_seqs", PyValue::None),
        ("max_model_len", PyValue::None),
        ("source", s("declared(fixture · 선언 없음)")),
    ]);
    let r = res(&Call {
        serving: Some(&nolim),
        ..Call::default()
    })?;
    suite.ck(
        "R16 서빙 yaml 에 한계 선언이 없으면 필터는 not_applied 로 기록(값을 만들지 않는다)",
        py_eq(&r.value("levels"), &ints(&[1, 2, 4, 8, 16]))
            && starts_with(&r.knob("levels", "serving_cap_applied"), "not_applied"),
        &none_detail,
    );
    let r = res(&Call {
        cli: vec![("levels", "2,4")],
        reasons: vec![("levels", "serving_concurrency")],
        serving: Some(&nolim),
        ..Call::default()
    })?;
    suite.ck(
        "R17 판정점 1 은 Agent 가 빼도 포함된다",
        py_eq(&r.value("levels"), &ints(&[1, 2, 4]))
            && matches!(
                r.knob("levels", "verdict_point_added").peel(),
                PyValue::Bool(true)
            ),
        &none_detail,
    );
    suite.ck(
        "R18 levels 32(confirm)를 승인 없이 → 거부",
        rejected(res(&Call {
            cli: vec![("levels", "1,32")],
            reasons: vec![("levels", "serving_concurrency")],
            serving: Some(&nolim),
            ..Call::default()
        }))?,
        &none_detail,
    );

    // D — the dataset axis: synthetic by default, a real dataset is a user preset, the verdict is unchanged
    let r = res(&Call {
        serving: Some(&nolim),
        ..Call::default()
    })?;
    let others_null = DATASET_KNOBS
        .iter()
        .filter(|name| **name != "dataset_kind")
        .all(|name| matches!(r.value(name).peel(), PyValue::None));
    suite.ck(
        "D1 기본 프리셋 = synthetic · 나머지 null · 기록 블록에 dataset 이 실린다",
        is(&r.value("dataset_kind"), "synthetic")
            && others_null
            && is(&at(&r.block, &["dataset", "dataset_kind"]), "synthetic"),
        &none_detail,
    );
    let raw = scratch.text("raw.parquet");
    std::fs::write(to_path(&raw), b"cnn-fixture")
        .map_err(|error| Failure::new("OSError", &error.to_string()))?;
    let digest = digest(&SHA256, b"cnn-fixture");
    let good_sha: String = digest
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let dataset_values = |over: &[(&str, PyValue)]| -> Vec<(&'static str, PyValue)> {
        let mut values: Vec<(&'static str, PyValue)> = vec![
            ("dataset_kind", s("parquet_file")),
            ("dataset_source", s("managed")),
            ("dataset_id", s("abisee/cnn_dailymail 3.0.0 test")),
            ("dataset_path", PyValue::Str(raw.clone())),
            ("dataset_revision", s(&"a".repeat(40))),
            ("source_sha256", s(&good_sha)),
            ("column_map", dict(vec![("text_column", s("article"))])),
            ("preprocess_strategy", s("concatenate")),
            ("seed", n(42)),
        ];
        for (name, value) in over {
            for entry in values.iter_mut() {
                if entry.0 == *name {
                    entry.1 = value.clone();
                }
            }
        }
        values
    };
    let dset_preset = |over: &[(&str, PyValue)]| -> PyValue {
        let mut doc = base.clone();
        for (name, value) in dataset_values(over) {
            doc = edited(
                &doc,
                &[k("knobs"), k(name)],
                &Edit::Set("default", value.clone()),
            );
            doc = edited(
                &doc,
                &[k("knobs"), k(name)],
                &Edit::Set(
                    "options",
                    PyValue::List(vec![dict(vec![("value", value), ("grade", s("auto"))])]),
                ),
            );
        }
        doc
    };
    let cnn = dset_preset(&[]);
    let cnn_problems = validate_preset(&cnn, &bindings)?;
    suite.ck(
        "D2 사용자 프리셋(CNN 모양)이 같은 검증을 통과한다",
        cnn_problems.is_empty(),
        &|| PyValue::List(cnn_problems.iter().cloned().map(PyValue::Str).collect()).repr(),
    );
    let invalid = |doc: PyValue| -> Result<bool, Failure> {
        Ok(!validate_preset(&doc, &bindings)?.is_empty())
    };
    suite.ck(
        "D3 상대경로 dataset_path 거부",
        invalid(dset_preset(&[("dataset_path", s("data/x.parquet"))]))?,
        &none_detail,
    );
    suite.ck(
        "D4 40자 hex 가 아닌 revision 거부",
        invalid(dset_preset(&[("dataset_revision", s("main"))]))?,
        &none_detail,
    );
    suite.ck(
        "D5 닫힌 목록 밖 dataset_kind 거부",
        invalid(dset_preset(&[("dataset_kind", s("huggingface"))]))?,
        &none_detail,
    );
    suite.ck(
        "D6 교차 규칙: 실제 데이터셋인데 sha256 이 null → 거부",
        invalid(dset_preset(&[("source_sha256", PyValue::None)]))?,
        &none_detail,
    );
    suite.ck(
        "D6b 교차 규칙: synthetic 인데 경로가 있다 → 거부",
        invalid(dset_preset(&[
            ("dataset_kind", s("synthetic")),
            ("dataset_source", PyValue::None),
            ("dataset_id", PyValue::None),
            ("dataset_revision", PyValue::None),
            ("source_sha256", PyValue::None),
            ("column_map", PyValue::None),
            ("preprocess_strategy", PyValue::None),
            ("seed", PyValue::None),
        ]))?,
        &none_detail,
    );
    suite.ck(
        "D6c column_map 은 text_column 하나만",
        invalid(dset_preset(&[(
            "column_map",
            dict(vec![("text_column", s("a")), ("x", s("b"))]),
        )]))?,
        &none_detail,
    );
    let dataset_cli: &[(&str, &str)] = &[("backend", "openai"), ("bench_budget_mib", "4096")];
    let dataset_reasons: &[(&str, &str)] = &[
        ("backend", "harmony_completion"),
        ("bench_budget_mib", "host_budget"),
    ];
    let res_d = |doc: &PyValue, tool: &'static str, dry_run: bool| {
        res_doc(
            doc,
            "cnn_preset",
            &Call {
                tool,
                serving: Some(&nolim),
                ..Call::default()
            },
            dry_run,
            dataset_cli,
            dataset_reasons,
        )
    };
    let r = res_d(&cnn, "guidellm", false)?;
    suite.ck(
        "D7 측정 시점: 원본 실재 · sha256 일치 → 통과하고 관측을 기록",
        is(&r.value("dataset_kind"), "parquet_file")
            && starts_with(
                &at(&r.block, &["dataset", "runtime_check", "checked"]),
                "measured(",
            ),
        &|| at(&r.block, &["dataset", "runtime_check"]).repr(),
    );
    suite.ck(
        "D8 --tool vllm + 실제 데이터셋 → 거부(조용히 synthetic 으로 재지 않는다)",
        rejected(res_d(&cnn, "vllm", false))?,
        &none_detail,
    );
    suite.ck(
        "D9 sha256 불일치 → 거부",
        rejected(res_d(
            &dset_preset(&[("source_sha256", s(&"b".repeat(64)))]),
            "guidellm",
            false,
        ))?,
        &none_detail,
    );
    let missing_dataset = PyValue::Str(scratch.text("missing.parquet"));
    suite.ck(
        "D9b 원본 부재 → 거부",
        rejected(res_d(
            &dset_preset(&[("dataset_path", missing_dataset.clone())]),
            "guidellm",
            false,
        ))?,
        &none_detail,
    );
    let r = res_d(
        &dset_preset(&[("dataset_path", missing_dataset)]),
        "guidellm",
        true,
    )?;
    suite.ck(
        "D9c dry-run 은 원본 검사를 건너뛰고 그 사실을 기록",
        starts_with(
            &at(&r.block, &["dataset", "runtime_check", "checked"]),
            "skipped(",
        ),
        &none_detail,
    );
    suite.ck(
        "D10 ephemeral 은 이번 개정에서 경로가 없다 → 거부",
        rejected(res_d(
            &dset_preset(&[("dataset_source", s("ephemeral"))]),
            "guidellm",
            false,
        ))?,
        &none_detail,
    );
    let non_default_escalate = ["dataset_kind", "dataset_source", "preprocess_strategy"]
        .iter()
        .all(|name| {
            let knob = at(&base, &["knobs", name]);
            let default = at(&knob, &["default"]);
            match at(&knob, &["options"]).peel() {
                PyValue::List(options) => options
                    .iter()
                    .filter(|option| !py_eq(&at(option, &["value"]), &default))
                    .all(|option| is(&at(option, &["grade"]), "escalate")),
                _ => false,
            }
        });
    let pins_only = DATASET_KNOBS
        .iter()
        .all(|name| bench_preset::knob_spec(name).is_some_and(|spec| spec.flag.is_none()));
    suite.ck(
        "D11 기본 프리셋의 비기본 데이터셋 값은 escalate(Agent 가 못 연다) · 데이터셋 knob 은 스윕 플래그가 없다",
        non_default_escalate && pins_only,
        &none_detail,
    );
    let _ = (dumps_sorted, OPTION_KEYS);
    Ok(())
}
