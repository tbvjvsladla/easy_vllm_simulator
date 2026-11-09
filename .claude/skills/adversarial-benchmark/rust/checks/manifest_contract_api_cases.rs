//! The in-process API of `manifest_contract.py` through the native bridge, checked against the ONE accepted original API87 capture.
//!
//! The golden file `data/manifest_contract_api/api87.json` is a literal copy of the saved raw (the request graph `args_before`, the graph after the
//! call `args_after` and the observed outcome of each of the 87 rows; only the private fixtures root is a token). For every row this check builds
//! the declared fixture in a fresh private tree, hands the request graph to `serve` (the bridge entry point the executable uses) and compares the
//! answer with the observation. Nothing is generated from the new code: the expected value is the raw, the only derived step is the mechanical
//! assembly of the result PLAN (what the Python adapter does), which is applied to the raw's own `args_after`.
//!
//! Rows the bridge does not answer by design are checked for the CLASSIFICATION only (`main` legacy shapes go to the reviewed original in the
//! adapter), and the module-facts rows are checked by the static adapter check, not here. U2 (traceback frames) is not part of any row.

use agent_runtime_support::pyjson::loads_text;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use sweep_stop::manifest_contract::Environment;
use sweep_stop::manifest_contract_bridge::serve;
use sweep_stop::typed_wire::{
    field, hex_decode, int_of, items_of, str_value, text_of, wtf8_decode,
};

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("checks/data/manifest_contract_api")
}

fn parse(text: &str) -> PyValue {
    loads_text(&PyStr::new(text)).expect("JSON")
}

fn dump(value: &PyValue) -> String {
    value.dumps_ascii().expect("ASCII JSON")
}

fn object(entries: Vec<(&str, PyValue)>) -> PyValue {
    let mut dict = PyDict::new();
    for (key, value) in entries {
        dict.set(key, value);
    }
    PyValue::Dict(dict)
}

fn get<'a>(value: &'a PyValue, name: &str) -> Option<&'a PyValue> {
    match value.peel() {
        PyValue::Dict(map) => map.get(name),
        _ => None,
    }
}

fn tag(wire: &PyValue) -> String {
    get(wire, "t")
        .map(|t| text_of(t).expect("tag"))
        .unwrap_or_default()
}

fn wire_text(wire: &PyValue) -> String {
    let bytes =
        hex_decode(&text_of(field(wire, "utf8_surrogatepass_hex").expect("hex")).expect("text"))
            .expect("hex");
    let points = wtf8_decode(&bytes).expect("wtf8");
    PyStr::from_code_points(points)
        .to_utf8()
        .unwrap_or_default()
}

// ── comparison form: the observer writes a few fields the bridge has no use for ────────────────────────────

/// Drops what the observer adds beyond the carried facts (`hex` of a float, `tzinfo`/`fold` of a datetime, the `order` note of a set) and sorts
/// the members of a set the way the observer does (by the canonical encoding of each member).
fn normal(wire: &PyValue) -> PyValue {
    match wire.peel() {
        PyValue::Dict(map) => {
            let kind = tag(wire);
            let mut out = PyDict::new();
            for (key, value) in map.iter() {
                let name = key.to_utf8().unwrap_or_default();
                let skip = (kind == "float" && name == "hex")
                    || (kind == "datetime" && (name == "tzinfo" || name == "fold"))
                    || (kind == "set" && name == "order");
                if skip {
                    continue;
                }
                let value = if kind == "set" && name == "members" {
                    let mut members: Vec<PyValue> = items_of(value)
                        .expect("members")
                        .iter()
                        .map(normal)
                        .collect();
                    members.sort_by_key(dump);
                    PyValue::List(members)
                } else {
                    normal(value)
                };
                out.set(&name, value);
            }
            PyValue::Dict(out)
        }
        PyValue::List(items) => PyValue::List(items.iter().map(normal).collect()),
        other => other.clone(),
    }
}

/// Adds `offset` to every container id and ref id (the observer numbers the answer after the argument graph).
fn shift_ids(wire: &PyValue, offset: i64) -> PyValue {
    match wire.peel() {
        PyValue::Dict(map) => {
            let kind = tag(wire);
            let mut out = PyDict::new();
            for (key, value) in map.iter() {
                let name = key.to_utf8().unwrap_or_default();
                if name == "id"
                    && matches!(kind.as_str(), "list" | "dict" | "tuple" | "set" | "ref")
                {
                    out.set("id", PyValue::int(int_of(value).unwrap() + offset));
                } else {
                    out.set(&name, shift_ids(value, offset));
                }
            }
            PyValue::Dict(out)
        }
        PyValue::List(items) => {
            PyValue::List(items.iter().map(|item| shift_ids(item, offset)).collect())
        }
        other => other.clone(),
    }
}

fn same(label: &str, got: &PyValue, want: &PyValue, failures: &mut Vec<String>) {
    let (a, b) = (dump(&normal(got)), dump(&normal(want)));
    if a != b {
        failures.push(format!(
            "{label}\n  got  {}\n  want {}",
            &a[..a.len().min(600)],
            &b[..b.len().min(600)]
        ));
    }
}

// ── the observed graph as data ─────────────────────────────────────────────────────────────────────────────

fn max_id(wire: &PyValue) -> i64 {
    let mut best = get(wire, "id").and_then(|id| int_of(id).ok()).unwrap_or(0);
    if let PyValue::Dict(map) = wire.peel() {
        for (_, value) in map.iter() {
            best = best.max(max_id(value));
        }
    } else if let PyValue::List(items) = wire.peel() {
        for item in items {
            best = best.max(max_id(item));
        }
    }
    best
}

fn find_id(wire: &PyValue, id: i64) -> Option<&PyValue> {
    if tag(wire) != "ref" && get(wire, "id").and_then(|v| int_of(v).ok()) == Some(id) {
        return Some(wire);
    }
    match wire.peel() {
        PyValue::Dict(map) => map.iter().find_map(|(_, value)| find_id(value, id)),
        PyValue::List(items) => items.iter().find_map(|item| find_id(item, id)),
        _ => None,
    }
}

fn resolve<'a>(roots: &'a [PyValue], wire: &'a PyValue) -> &'a PyValue {
    if tag(wire) == "ref" {
        let id = int_of(field(wire, "id").expect("id")).expect("int");
        return roots
            .iter()
            .find_map(|root| find_id(root, id))
            .expect("a ref points into the argument graph");
    }
    wire
}

fn ref_or_inline(wire: &PyValue) -> PyValue {
    match (tag(wire).as_str(), get(wire, "id")) {
        ("list" | "dict" | "tuple" | "set", Some(id)) => {
            object(vec![("t", PyValue::str("ref")), ("id", id.clone())])
        }
        _ => wire.clone(),
    }
}

fn dict_get<'a>(roots: &'a [PyValue], dict: &'a PyValue, key: &str) -> Option<&'a PyValue> {
    let dict = resolve(roots, dict);
    if tag(dict) != "dict" {
        return None;
    }
    items_of(field(dict, "pairs").ok()?)
        .ok()?
        .iter()
        .find_map(|pair| {
            let pair = items_of(pair).ok()?;
            (tag(&pair[0]) == "str" && wire_text(&pair[0]) == key).then(|| &pair[1])
        })
}

/// Python's dictionary-key identity of a hashable wire.
fn key_identity(wire: &PyValue) -> String {
    let wire = normal(wire);
    match tag(&wire).as_str() {
        "str" => format!("s:{}", wire_text(&wire)),
        "none" => "none".into(),
        "bool" => format!(
            "n:{}",
            u8::from(matches!(
                field(&wire, "v").map(PyValue::peel),
                Ok(PyValue::Bool(true))
            ))
        ),
        "int" => format!(
            "n:{}{}",
            if text_of(field(&wire, "sign").unwrap()).unwrap() == "-" {
                "-"
            } else {
                ""
            },
            text_of(field(&wire, "hex").unwrap())
                .unwrap()
                .trim_start_matches('0')
        ),
        "float" => match get(&wire, "nan_id") {
            Some(id) => format!("nan:{}", dump(id)),
            None => format!("f:{}", text_of(field(&wire, "repr").unwrap()).unwrap()),
        },
        other => format!("{other}:{}", dump(&wire)),
    }
}

fn renumber(wire: &PyValue, next: &mut i64) -> PyValue {
    match wire.peel() {
        PyValue::Dict(map) => {
            let kind = tag(wire);
            let mut out = PyDict::new();
            for (key, value) in map.iter() {
                let name = key.to_utf8().unwrap_or_default();
                if name == "id" && matches!(kind.as_str(), "list" | "dict" | "tuple" | "set") {
                    out.set("id", PyValue::int(*next));
                    *next += 1;
                } else {
                    out.set(&name, renumber(value, next));
                }
            }
            PyValue::Dict(out)
        }
        PyValue::List(items) => {
            PyValue::List(items.iter().map(|item| renumber(item, next)).collect())
        }
        other => other.clone(),
    }
}

/// What the adapter does with the plan, on the observer's wire: the result dictionary assembled from the ORIGINAL argument objects.
fn assemble(args_after: &[PyValue], plan: &PyValue) -> PyValue {
    let man = &args_after[0];
    let mut next = args_after.iter().map(max_id).max().unwrap_or(0) + 1;
    let res_id = next;
    next += 1;
    let mut pairs: Vec<PyValue> = Vec::new();
    let mut res_values: Vec<(String, PyValue)> = Vec::new();
    for entry in items_of(plan).expect("plan") {
        let key = text_of(field(entry, "key").unwrap()).unwrap();
        let from = text_of(field(entry, "from").unwrap()).unwrap();
        let value = match from.as_str() {
            "input" => {
                let name = text_of(field(entry, "name").unwrap()).unwrap();
                dict_get(args_after, man, &name)
                    .map_or_else(|| object(vec![("t", PyValue::str("none"))]), ref_or_inline)
            }
            "value" => renumber(field(entry, "v").unwrap(), &mut next),
            "effective_map" => {
                let map_id = next;
                next += 1;
                let nodes_wire = res_values
                    .iter()
                    .find(|(k, _)| k == "nodes")
                    .map(|(_, v)| v.clone())
                    .expect("nodes precede the map");
                let nodes_resolved = resolve(args_after, &nodes_wire).clone();
                let top = res_values
                    .iter()
                    .find(|(k, _)| k == "model_source")
                    .map(|(_, v)| v.clone())
                    .expect("model_source precedes the map");
                let mut entries: Vec<(String, PyValue, PyValue)> = Vec::new();
                for item in items_of(field(entry, "entries").unwrap()).unwrap() {
                    let at = int_of(field(item, "node").unwrap()).unwrap() as usize;
                    let node = resolve(
                        args_after,
                        &items_of(field(&nodes_resolved, "v").unwrap()).unwrap()[at],
                    )
                    .clone();
                    let role = match field(item, "role").unwrap().peel() {
                        PyValue::Str(_) => dict_get(args_after, &node, "role")
                            .map(ref_or_inline)
                            .expect("role"),
                        other => str_value(
                            &text_of(field(&PyValue::clone(&other.clone()), "text").unwrap())
                                .unwrap(),
                        ),
                    };
                    let value =
                        if text_of(field(item, "value").unwrap()).unwrap() == "node_model_source" {
                            ref_or_inline(
                                dict_get(args_after, &node, "model_source").expect("model_source"),
                            )
                        } else {
                            top.clone()
                        };
                    let identity = key_identity(&role);
                    match entries.iter_mut().find(|(known, _, _)| *known == identity) {
                        Some(existing) => existing.2 = value,
                        None => entries.push((identity, role, value)),
                    }
                }
                object(vec![
                    ("t", PyValue::str("dict")),
                    ("id", PyValue::int(map_id)),
                    (
                        "pairs",
                        PyValue::List(
                            entries
                                .into_iter()
                                .map(|(_, k, v)| PyValue::List(vec![k, v]))
                                .collect(),
                        ),
                    ),
                ])
            }
            other => panic!("plan source {other}"),
        };
        res_values.push((key.clone(), value.clone()));
        pairs.push(PyValue::List(vec![str_value(&key), value]));
    }
    object(vec![
        ("t", PyValue::str("dict")),
        ("id", PyValue::int(res_id)),
        ("pairs", PyValue::List(pairs)),
    ])
}

// ── fixtures ───────────────────────────────────────────────────────────────────────────────────────────────

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_AUTHOR_NAME", "x")
        .env("GIT_AUTHOR_EMAIL", "x@x")
        .env("GIT_COMMITTER_NAME", "x")
        .env("GIT_COMMITTER_EMAIL", "x@x")
        .output()
        .expect("git");
    assert!(
        status.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&status.stderr)
    );
}

fn make_groups(root: &Path) {
    for (group, branch, born) in [
        ("single", "single-node", true),
        ("multi", "multi-node", true),
        ("other", "fixture-other", true),
        ("detached", "single-node", true),
        ("unborn", "single-node", false),
        ("nogit", "", false),
    ] {
        let dir = root.join(group);
        std::fs::create_dir_all(dir.join("cases")).unwrap();
        if group == "nogit" {
            continue;
        }
        git(&dir, &["init", "-q", "-b", branch]);
        if born {
            git(&dir, &["commit", "-q", "--allow-empty", "-m", "fixture"]);
        }
        if group == "detached" {
            git(&dir, &["checkout", "-q", "--detach"]);
        }
    }
    std::fs::create_dir_all(root.join("_nogit_bin")).unwrap();
}

fn build_row_fixture(root: &Path, row: &PyValue, opaque: &mut Vec<PathBuf>) -> PathBuf {
    let fixture = field(row, "fixture").unwrap();
    let group = root.join(text_of(field(fixture, "repo_group").unwrap()).unwrap());
    let case_dir = group.join(text_of(field(fixture, "case_dir").unwrap()).unwrap());
    std::fs::create_dir_all(&case_dir).unwrap();
    for file in items_of(field(fixture, "files").unwrap()).unwrap() {
        let path = case_dir.join(text_of(field(file, "path").unwrap()).unwrap());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        if let Some(link) = get(file, "link") {
            std::os::unix::fs::symlink(text_of(link).unwrap(), &path).unwrap();
            continue;
        }
        let data = text_of(field(file, "data").unwrap()).unwrap();
        std::fs::write(&path, std::fs::read(data_dir().join(&data)).unwrap()).unwrap();
        let mode = u32::from_str_radix(&text_of(field(file, "mode").unwrap()).unwrap(), 8).unwrap();
        std::fs::set_permissions(&path, std::os::unix::fs::PermissionsExt::from_mode(mode))
            .unwrap();
        if get(file, "opaque_mode").is_some() {
            opaque.push(path);
        }
    }
    case_dir
}

fn untoken(wire: &PyValue, root: &Path) -> PyValue {
    match wire.peel() {
        PyValue::Dict(map) => {
            let mut out = PyDict::new();
            for (key, value) in map.iter() {
                let name = key.to_utf8().unwrap_or_default();
                if name == "utf8_surrogatepass_hex" {
                    let text = wire_text(wire).replace("<FIX>", root.to_str().unwrap());
                    out.set(
                        &name,
                        PyValue::str(&text.bytes().map(|b| format!("{b:02x}")).collect::<String>()),
                    );
                } else if name == "codepoints" {
                    out.set(
                        &name,
                        PyValue::int(
                            PyStr::new(&wire_text(wire).replace("<FIX>", root.to_str().unwrap()))
                                .len() as i64,
                        ),
                    );
                } else {
                    out.set(&name, untoken(value, root));
                }
            }
            PyValue::Dict(out)
        }
        PyValue::List(items) => {
            PyValue::List(items.iter().map(|item| untoken(item, root)).collect())
        }
        other => other.clone(),
    }
}

fn untoken_hex(hex: &str, root: &Path) -> String {
    let text = String::from_utf8(hex_decode(hex).unwrap())
        .unwrap()
        .replace("<FIX>", root.to_str().unwrap());
    text.bytes().map(|b| format!("{b:02x}")).collect()
}

static CWD_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn is_root() -> bool {
    rustix::process::geteuid().is_root()
}

fn environment() -> Environment {
    Environment {
        text_variables: vec![
            ("LC_ALL".to_string(), b"C.UTF-8".to_vec()),
            ("LANG".to_string(), b"C.UTF-8".to_vec()),
        ],
        ..Environment::default()
    }
}

fn ask(op: &str, args: &[PyValue]) -> PyValue {
    let request = object(vec![
        ("v", PyValue::int(1)),
        ("op", PyValue::str(op)),
        ("args", PyValue::List(args.to_vec())),
    ]);
    let answer = serve(dump(&request).as_bytes(), &environment()).expect("transport");
    get(&parse(&answer), "answer").expect("answer").clone()
}

fn status(answer: &PyValue) -> String {
    text_of(field(answer, "status").unwrap()).unwrap()
}

fn class_of(observed: &PyValue) -> String {
    let cls = text_of(field(observed, "cls").unwrap()).unwrap();
    cls.rsplit('.').next().unwrap().to_string()
}

/// The observed exception's constructor arguments as wires.
fn observed_args(exception: &PyValue) -> Vec<PyValue> {
    items_of(field(exception, "args").unwrap())
        .unwrap()
        .to_vec()
}

fn text_arg(wire: &PyValue) -> Option<String> {
    (tag(wire) == "str").then(|| wire_text(wire))
}

fn walk_steps(roots: &[PyValue], path: &PyValue) -> PyValue {
    let mut current: Option<PyValue> = None;
    for step in items_of(path).unwrap() {
        current = Some(if let Some(arg) = get(step, "arg") {
            roots[int_of(arg).unwrap() as usize].clone()
        } else if let Some(key) = get(step, "key") {
            dict_get(roots, current.as_ref().unwrap(), &text_of(key).unwrap())
                .expect("key on the path")
                .clone()
        } else {
            let list = resolve(roots, current.as_ref().unwrap()).clone();
            items_of(field(&list, "v").unwrap()).unwrap()
                [int_of(get(step, "index").unwrap()).unwrap() as usize]
                .clone()
        });
    }
    ref_or_inline(resolve(roots, &current.expect("a path")))
}

fn check_row(row: &PyValue, root: &Path, case_dir: &Path, failures: &mut Vec<String>) {
    let id = text_of(field(row, "id").unwrap()).unwrap();
    let op = text_of(field(row, "operation").unwrap()).unwrap();
    let expected = field(row, "expected").unwrap();
    // an `os.PathLike` argument travels as its `os.fspath()` text (what the adapter's path marshalling does)
    let args: Vec<PyValue> = items_of(field(row, "args").unwrap())
        .unwrap()
        .iter()
        .map(|wire| untoken(wire, root))
        .map(|wire| {
            if tag(&wire) == "object"
                && text_of(field(&wire, "type").unwrap()).unwrap() == "pathlib.PosixPath"
            {
                let repr = wire_text(field(&wire, "repr").unwrap());
                str_value(
                    repr.trim_start_matches("PosixPath('")
                        .trim_end_matches("')"),
                )
            } else {
                wire
            }
        })
        .collect();
    let args_after: Vec<PyValue> =
        items_of(get(expected, "args_after").unwrap_or(&PyValue::List(vec![])))
            .unwrap()
            .iter()
            .map(|wire| untoken(wire, root))
            .collect();
    let want_status = text_of(field(expected, "status").unwrap()).unwrap();
    let setup = field(row, "setup").unwrap();
    let saved: Vec<(String, Option<String>)> = ["PATH", "GIT_CEILING_DIRECTORIES"]
        .iter()
        .map(|k| (k.to_string(), std::env::var(k).ok()))
        .collect();
    for (key, value) in match get(setup, "env").map(PyValue::peel) {
        Some(PyValue::Dict(map)) => map
            .iter()
            .map(|(k, v)| {
                (
                    k.to_utf8().unwrap(),
                    text_of(v)
                        .unwrap()
                        .replace("$R48_FIXTURES", root.to_str().unwrap()),
                )
            })
            .collect::<Vec<_>>(),
        _ => vec![],
    } {
        std::env::set_var(key, value);
    }
    std::env::set_current_dir(case_dir).unwrap();
    let label = |what: &str| format!("{id} {op}: {what}");
    match op.as_str() {
        "evaluate_contract"
        | "manifest_tp"
        | "effective_model_source"
        | "resolve_topology"
        | "_load_manifest" => {
            let bridge_op = if op == "_load_manifest" {
                "load_manifest"
            } else {
                op.as_str()
            };
            let answer = ask(bridge_op, &args);
            let got = status(&answer);
            match want_status.as_str() {
                "RETURNED" => {
                    if got != "returned" {
                        failures.push(label(&format!(
                            "bridge answered {} instead of returned: {}",
                            got,
                            &dump(&answer)[..dump(&answer).len().min(300)]
                        )));
                    } else {
                        let want = field(expected, "return").unwrap();
                        match op.as_str() {
                            "evaluate_contract" => same(
                                &label("result"),
                                &assemble(&args_after, field(&answer, "plan").unwrap()),
                                want,
                                failures,
                            ),
                            "manifest_tp" | "_load_manifest" => same(
                                &label("value"),
                                &shift_ids(
                                    field(&answer, "value").unwrap(),
                                    args_after.iter().map(max_id).max().unwrap_or(0),
                                ),
                                want,
                                failures,
                            ),
                            "effective_model_source" => {
                                let which = text_of(field(&answer, "which").unwrap()).unwrap();
                                let chosen = if which == "node_model_source" {
                                    ref_or_inline(
                                        dict_get(&args_after, &args_after[0], "model_source")
                                            .expect("model_source"),
                                    )
                                } else {
                                    ref_or_inline(&args_after[1])
                                };
                                same(&label("selection"), &chosen, want, failures);
                            }
                            _ => {
                                let which = text_of(field(&answer, "which").unwrap()).unwrap();
                                let chosen = if which == "explicit" {
                                    ref_or_inline(&args_after[1])
                                } else {
                                    field(&answer, "value").unwrap().clone()
                                };
                                same(&label("topology"), &chosen, want, failures);
                            }
                        }
                    }
                }
                "RAISED" => {
                    let exception = field(expected, "exception").unwrap();
                    if got != "raised" {
                        failures.push(label(&format!("bridge answered {got} instead of raised")));
                    } else {
                        let want_class = class_of(exception);
                        let got_class = text_of(field(&answer, "cls").unwrap()).unwrap();
                        let args_want = observed_args(exception);
                        let kind = text_of(field(&answer, "kind").unwrap()).unwrap();
                        // `OSError(errno, strerror, filename)` is resolved by Python itself to the subclass of the errno
                        let expected_class = if kind == "os" {
                            match int_of(field(&answer, "errno").unwrap()).unwrap() {
                                2 => "FileNotFoundError",
                                13 => "PermissionError",
                                17 => "FileExistsError",
                                20 => "NotADirectoryError",
                                21 => "IsADirectoryError",
                                _ => "OSError",
                            }
                        } else {
                            got_class.as_str()
                        };
                        if expected_class != want_class {
                            failures
                                .push(label(&format!("class {expected_class} != {want_class}")));
                        }
                        match kind.as_str() {
                            "plain" => {
                                let message = wire_text(field(&answer, "message").unwrap());
                                if text_arg(&args_want[0]).as_deref() != Some(message.as_str()) {
                                    failures.push(label(&format!(
                                        "message {message:?} != {:?}",
                                        args_want[0]
                                    )));
                                }
                                if let Some(name) = get(&answer, "name")
                                    .filter(|n| !matches!(n.peel(), PyValue::None))
                                {
                                    let attrs = field(exception, "attrs").unwrap();
                                    same(
                                        &label("name attr"),
                                        &str_value(&text_of(name).unwrap()),
                                        field(attrs, "name").unwrap(),
                                        failures,
                                    );
                                    let obj =
                                        walk_steps(&args_after, field(&answer, "obj").unwrap());
                                    same(
                                        &label("obj attr"),
                                        &obj,
                                        field(attrs, "obj").unwrap(),
                                        failures,
                                    );
                                }
                            }
                            "os" => {
                                same(
                                    &label("errno"),
                                    &object(vec![
                                        ("t", PyValue::str("int")),
                                        ("sign", PyValue::str("+")),
                                        (
                                            "hex",
                                            PyValue::str(&format!(
                                                "{:x}",
                                                int_of(field(&answer, "errno").unwrap()).unwrap()
                                            )),
                                        ),
                                    ]),
                                    &args_want[0],
                                    failures,
                                );
                                same(
                                    &label("strerror"),
                                    field(&answer, "strerror").unwrap(),
                                    &args_want[1],
                                    failures,
                                );
                                same(
                                    &label("filename attr"),
                                    field(&answer, "filename").unwrap(),
                                    &untoken(
                                        field(field(exception, "attrs").unwrap(), "filename")
                                            .unwrap(),
                                        root,
                                    ),
                                    failures,
                                );
                            }
                            "decode" => {
                                same(
                                    &label("object"),
                                    &object(vec![
                                        ("t", PyValue::str("bytes")),
                                        ("hex", field(&answer, "object").unwrap().clone()),
                                    ]),
                                    &args_want[1],
                                    failures,
                                );
                                for (i, name) in ["start", "end"].iter().enumerate() {
                                    same(
                                        &label(name),
                                        &object(vec![
                                            ("t", PyValue::str("int")),
                                            ("sign", PyValue::str("+")),
                                            (
                                                "hex",
                                                PyValue::str(&format!(
                                                    "{:x}",
                                                    int_of(field(&answer, name).unwrap()).unwrap()
                                                )),
                                            ),
                                        ]),
                                        &args_want[2 + i],
                                        failures,
                                    );
                                }
                                same(
                                    &label("reason"),
                                    &str_value(
                                        &text_of(field(&answer, "reason").unwrap()).unwrap(),
                                    ),
                                    &args_want[4],
                                    failures,
                                );
                                same(
                                    &label("encoding"),
                                    &str_value(
                                        &text_of(field(&answer, "encoding").unwrap()).unwrap(),
                                    ),
                                    &args_want[0],
                                    failures,
                                );
                            }
                            "yaml_reader" => {
                                same(
                                    &label("name"),
                                    field(&answer, "source_name").unwrap(),
                                    &untoken(&args_want[0], root),
                                    failures,
                                );
                                for (i, name) in ["position", "character"].iter().enumerate() {
                                    same(
                                        &label(name),
                                        &object(vec![
                                            ("t", PyValue::str("int")),
                                            ("sign", PyValue::str("+")),
                                            (
                                                "hex",
                                                PyValue::str(&format!(
                                                    "{:x}",
                                                    int_of(field(&answer, name).unwrap()).unwrap()
                                                )),
                                            ),
                                        ]),
                                        &args_want[1 + i],
                                        failures,
                                    );
                                }
                                same(
                                    &label("encoding"),
                                    &str_value(
                                        &text_of(field(&answer, "encoding").unwrap()).unwrap(),
                                    ),
                                    &args_want[3],
                                    failures,
                                );
                                same(
                                    &label("reason"),
                                    &str_value(
                                        &text_of(field(&answer, "reason").unwrap()).unwrap(),
                                    ),
                                    &args_want[4],
                                    failures,
                                );
                            }
                            "yaml_marked" => {
                                // object identity: the observer writes the second mark as a `ref` exactly when PyYAML passed ONE Mark object
                                let observed_shared = tag(field(
                                    field(exception, "yaml_marks").unwrap(),
                                    "problem_mark",
                                )
                                .unwrap())
                                    == "ref";
                                let answered_shared = matches!(
                                    field(&answer, "marks_shared").unwrap().peel(),
                                    PyValue::Bool(true)
                                );
                                // the number of positional arguments of the original construction (R48 v5 measured: LM10..LM13 observed four, never five)
                                let arity = usize::try_from(
                                    int_of(field(&answer, "args_arity").unwrap()).unwrap(),
                                )
                                .unwrap();
                                if arity != args_want.len() {
                                    failures.push(label(&format!(
                                        "exception.args length: answered {arity}, observed {}",
                                        args_want.len()
                                    )));
                                }
                                if observed_shared != answered_shared {
                                    failures.push(label(&format!("marks shared: observed {observed_shared}, answered {answered_shared}")));
                                }
                                for (i, name) in
                                    ["context", "context_mark", "problem", "problem_mark", "note"]
                                        .iter()
                                        .enumerate()
                                {
                                    let got_field = field(&answer, name).unwrap();
                                    // the marks first appear (in full) in `yaml_marks`; `args` then holds refs to them
                                    let want_arg = if name.ends_with("mark") {
                                        untoken(
                                            field(field(exception, "yaml_marks").unwrap(), name)
                                                .unwrap(),
                                            root,
                                        )
                                    } else {
                                        args_want.get(i).map_or_else(
                                            || object(vec![("t", PyValue::str("none"))]),
                                            |wire| untoken(wire, root),
                                        )
                                    };
                                    if matches!(got_field.peel(), PyValue::None) {
                                        same(
                                            &label(name),
                                            &object(vec![("t", PyValue::str("none"))]),
                                            &want_arg,
                                            failures,
                                        );
                                    } else if name.ends_with("mark") {
                                        let fields = field(&want_arg, "fields").unwrap();
                                        let want_mark: Vec<String> = items_of(fields)
                                            .unwrap()
                                            .iter()
                                            .map(|pair| dump(&items_of(pair).unwrap()[1]))
                                            .collect();
                                        let ints = |k: &str| {
                                            object(vec![
                                                ("t", PyValue::str("int")),
                                                ("sign", PyValue::str("+")),
                                                (
                                                    "hex",
                                                    PyValue::str(&format!(
                                                        "{:x}",
                                                        int_of(field(got_field, k).unwrap())
                                                            .unwrap()
                                                    )),
                                                ),
                                            ])
                                        };
                                        let buffer = field(got_field, "buffer").unwrap();
                                        let pointer = if matches!(
                                            field(got_field, "pointer").unwrap().peel(),
                                            PyValue::None
                                        ) {
                                            object(vec![("t", PyValue::str("none"))])
                                        } else {
                                            ints("pointer")
                                        };
                                        let got_mark = [
                                            dump(&untoken(field(got_field, "name").unwrap(), root)),
                                            dump(&ints("index")),
                                            dump(&ints("line")),
                                            dump(&ints("column")),
                                            dump(&if matches!(buffer.peel(), PyValue::None) {
                                                object(vec![("t", PyValue::str("none"))])
                                            } else {
                                                buffer.clone()
                                            }),
                                            dump(&pointer),
                                        ];
                                        if got_mark.to_vec() != want_mark {
                                            failures.push(label(&format!(
                                                "{name}: {got_mark:?} != {want_mark:?}"
                                            )));
                                        }
                                    } else {
                                        same(&label(name), got_field, &want_arg, failures);
                                    }
                                }
                            }
                            other => failures.push(label(&format!("unmodelled kind {other}"))),
                        }
                    }
                }
                other => failures.push(label(&format!("unexpected observed status {other}"))),
            }
        }
        "main" => {
            let argv_text: Vec<String> = items_of(field(&args[0], "v").unwrap())
                .unwrap()
                .iter()
                .map(wire_text)
                .collect();
            let classified = ask("classify_main", &args);
            let which = text_of(field(&classified, "which").unwrap()).unwrap();
            if want_status == "SYSTEM_EXIT" || argv_text.iter().any(|a| a == "--self-test") {
                if which != "legacy" {
                    failures.push(label(&format!("{argv_text:?} must be classified legacy (the reviewed original answers), got {which}")));
                }
            } else if which != "native" {
                failures.push(label(&format!(
                    "{argv_text:?} must be classified native, got {which}"
                )));
            } else {
                let answer = ask("main", &args);
                if status(&answer) != "returned" {
                    failures.push(label(&format!("main answered {}", status(&answer))));
                } else {
                    let want_rc = i64::from_str_radix(
                        &text_of(field(field(expected, "return").unwrap(), "hex").unwrap())
                            .unwrap(),
                        16,
                    )
                    .unwrap();
                    if int_of(field(&answer, "rc").unwrap()).unwrap() != want_rc {
                        failures.push(label(&format!(
                            "rc {} != {want_rc}",
                            dump(field(&answer, "rc").unwrap())
                        )));
                    }
                    let captured = field(expected, "captured").unwrap();
                    for (name, got_field) in [
                        ("stdout", field(&answer, "stdout").unwrap()),
                        ("stderr", field(&answer, "stderr").unwrap()),
                    ] {
                        let want_hex =
                            untoken_hex(&text_of(field(captured, name).unwrap()).unwrap(), root);
                        let mut text = if matches!(got_field.peel(), PyValue::None) {
                            String::new()
                        } else {
                            wire_text(got_field)
                        };
                        if name == "stdout" && !matches!(got_field.peel(), PyValue::None) {
                            text.push('\n');
                        }
                        let got_hex: String = text.bytes().map(|b| format!("{b:02x}")).collect();
                        if got_hex != want_hex {
                            failures.push(label(&format!(
                                "{name}\n  got  {text:?}\n  want {:?}",
                                String::from_utf8_lossy(&hex_decode(&want_hex).unwrap())
                            )));
                        }
                    }
                }
            }
        }
        // module facts and import effects belong to the adapter file: checked statically
        "module_facts" | "import_effects" => {}
        other => failures.push(label(&format!("unknown operation {other}"))),
    }
    for (key, value) in saved {
        match value {
            Some(value) => std::env::set_var(key, value),
            None => std::env::remove_var(key),
        }
    }
}

#[test]
fn the_87_original_api_rows_are_answered_by_the_bridge_as_the_saved_original_raw() {
    let _cwd = CWD_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let golden = parse(&std::fs::read_to_string(data_dir().join("api87.json")).unwrap());
    let rows = items_of(field(&golden, "rows").unwrap()).unwrap();
    assert_eq!(rows.len(), 87, "the accepted capture has 87 rows");
    let root = std::env::temp_dir().join(format!("manifest-contract-api-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    make_groups(&root);
    let mut opaque = Vec::new();
    let cases: Vec<PathBuf> = rows
        .iter()
        .map(|row| build_row_fixture(&root, row, &mut opaque))
        .collect();
    let restricted = !is_root();
    for path in &opaque {
        std::fs::set_permissions(path, std::os::unix::fs::PermissionsExt::from_mode(0o000))
            .unwrap();
    }
    let before = std::env::current_dir().unwrap();
    let (mut failures, mut checked, mut skipped): (Vec<String>, usize, HashSet<String>) =
        (Vec::new(), 0, HashSet::new());
    for (row, case_dir) in rows.iter().zip(&cases) {
        let id = text_of(field(row, "id").unwrap()).unwrap();
        if !restricted && !opaque.is_empty() && id == "LM20" {
            skipped.insert(id);
            continue;
        }
        check_row(row, &root, case_dir, &mut failures);
        checked += 1;
    }
    std::env::set_current_dir(before).unwrap();
    for path in &opaque {
        let _ = std::fs::set_permissions(path, std::os::unix::fs::PermissionsExt::from_mode(0o644));
    }
    let _ = std::fs::remove_dir_all(&root);
    assert!(
        failures.is_empty(),
        "{} failure(s) over {checked} checked rows (skipped {skipped:?}):\n{}",
        failures.len(),
        failures.join("\n")
    );
    assert_eq!(checked + skipped.len(), 87);
}

#[test]
fn the_comparison_can_fail_a_different_answer_is_reported() {
    // negative control: the comparison used above is not a tautology
    let golden = parse(&std::fs::read_to_string(data_dir().join("api87.json")).unwrap());
    let rows = items_of(field(&golden, "rows").unwrap()).unwrap();
    let want = field(field(&rows[0], "expected").unwrap(), "return").unwrap();
    let mut failures = Vec::new();
    same("identical", want, want, &mut failures);
    assert!(failures.is_empty());
    let changed = parse(&dump(want).replace("\"v\": true", "\"v\": false"));
    same("changed", &changed, want, &mut failures);
    assert_eq!(failures.len(), 1, "a changed bool must be reported");
}
