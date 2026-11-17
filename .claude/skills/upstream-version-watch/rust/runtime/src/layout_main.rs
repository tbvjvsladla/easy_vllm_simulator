//! `analyze(keys)` and `main()` of `check_spec_layout.py`: the early check of a checkpoint's speculative-decoder tensor names against the chosen
//! method, and the command line around it. Exit status 0 (OK or SKIP), 8 (STOP), 3 (the model directory is missing), 2 (argparse).

use crate::layout_files::{
    isdir, load_index_keys, read_method_from_config, read_num_nextn, LayoutWorld,
};
use crate::layout_rules::{
    first_component, holds_dspark_marker, is_one_or_none, is_plain_component, match_spec_key,
    minus_one, text_of,
};
use crate::layout_sites::SiteId;
use crate::main_cmd::{argument_error, basename, exit_with, intern, split_usage};
use crate::pytext::py_str;
use crate::thrown::{Done, Thrown};
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{Opaque, PyDict, PyValue, Typed};
use recipe_runtime::cliparse::{
    format_usage, parse, terminal_width, Kind, Opt, Outcome, Spec, Value,
};
use recipe_runtime::escalation::Raised;
use std::collections::BTreeMap;

/// The program name of `sys.argv[0]` in the command line (every usage and error text says `check_spec_layout.py`).
pub const PROGRAM: &str = "check_spec_layout.sh";

fn option(
    flags: &'static [&'static str],
    dest: &'static str,
    kind: Kind,
    help: Option<&'static str>,
    required: bool,
) -> Opt {
    Opt {
        flags,
        dest,
        kind,
        help,
        required,
        group: None,
    }
}

/// The parser of `main` for the program name `prog`: `--model-dir` (required), `--method`, `--config`.
pub fn spec(prog: &str) -> Spec {
    Spec {
        prog: intern(prog),
        description: "",
        options: vec![
            option(&["-h", "--help"], "help", Kind::Help, None, false),
            option(
                &["--model-dir"],
                "model_dir",
                Kind::Text,
                Some("호스트에서 본 체크포인트 디렉터리"),
                true,
            ),
            option(
                &["--method"],
                "method",
                Kind::Text,
                Some("speculative method(미지정 시 --config 에서 읽음)"),
                false,
            ),
            option(
                &["--config"],
                "config",
                Kind::Text,
                Some("트리플렛 yaml (method 도출용)"),
                false,
            ),
        ],
        group_required: false,
        positional: None,
    }
}

/// `print(text)`: the text and then its newline, two writes, each of which can fail before the next step.
fn print(world: &mut LayoutWorld<'_>, text: &PyStr) -> Done<()> {
    world.host.out(text)?;
    world.host.out(&PyStr::new("\n"))
}

/// `print(text, file=sys.stderr)`.
fn print_error(world: &mut LayoutWorld<'_>, text: &PyStr) -> Done<()> {
    world.host.err(text)?;
    world.host.err(&PyStr::new("\n"))
}

fn line(parts: &[&PyStr]) -> PyStr {
    let mut text = PyStr::default();
    for part in parts {
        text.push_py(part);
    }
    text
}

fn literal(text: &str) -> PyStr {
    PyStr::new(text)
}

/// `ap.parse_args()` over `sys.argv[1:]`: the values, or the `SystemExit` of help and of a usage error, with what argparse wrote.
fn parse_arguments(world: &mut LayoutWorld<'_>) -> Done<BTreeMap<&'static str, Value>> {
    let program = world
        .argv
        .first()
        .ok_or_else(|| Thrown::builtin("IndexError", "list index out of range"))?;
    let spec = spec(&basename(program));
    let (status, stdout, stderr) = match parse(&spec, &world.argv[1..], world.columns) {
        Outcome::Values(values) => return Ok(values),
        Outcome::Exit {
            status,
            stdout,
            stderr,
        } => (status, stdout, stderr),
    };
    let mut message = None;
    if !stdout.is_empty() {
        world.host.parser_help(&without_extra_blank_line(&stdout));
    }
    if !stderr.is_empty() {
        let usage = PyStr::new(&format_usage(&spec, terminal_width(world.columns)));
        let (head, rest) = split_usage(&usage, &stderr);
        world.host.parser_error(&head, &rest);
        message = Some(rest);
    }
    // an output write that failed with an exception argparse does not swallow stopped the parse there (see `resolve_build_track`)
    if let Some(mut failure) = world.host.take_parser_failure() {
        if failure.raised.cause.is_none() {
            if let Some(context) = message.as_ref().and_then(argument_error) {
                failure.raised.cause = Some(Box::new(context));
                failure.raised.via_context = true;
            }
        }
        return Err(failure);
    }
    let mut raised = Raised::new("SystemExit", "builtins", PyStr::new(&status.to_string()));
    raised.code = Some(Box::new(PyValue::int(status)));
    if let Some(context) = message.as_ref().and_then(argument_error) {
        raised.cause = Some(Box::new(context));
        raised.via_context = true;
    }
    Err(Thrown::plain(raised))
}

/// The common help formatter always puts a description paragraph after the usage: the usage line break, one empty line break around the (empty)
/// description paragraph and the blank line of the next section. This parser has no description, so argparse prints the usage, ONE blank line and
/// the options: the four consecutive line breaks that follow the usage are two.
fn without_extra_blank_line(help: &PyStr) -> PyStr {
    let points = help.code_points();
    let quadruple: Vec<u32> = "\n\n\n\n".chars().map(u32::from).collect();
    match points
        .windows(4)
        .position(|window| window == quadruple.as_slice())
    {
        Some(at) => {
            let mut fixed = points[..at + 2].to_vec();
            fixed.extend_from_slice(&points[at + 4..]);
            PyStr::from_code_points(fixed)
        }
        None => help.clone(),
    }
}

fn text_option(values: &BTreeMap<&'static str, Value>, key: &str) -> Option<PyStr> {
    match values.get(key) {
        Some(Value::Text(text)) => Some(text.clone()),
        _ => None,
    }
}

/// The names of the tensors that belong to the speculative decoder: per index the parts it has, in the order they first appear.
pub struct Layout {
    pub per_index: Vec<(Vec<u32>, Vec<Vec<u32>>)>,
    pub total: usize,
}

impl Layout {
    /// The indexes sorted as `sorted(per_index)` sorts the strings (by code point, lexically: `"10"` before `"2"`).
    pub fn sorted_indexes(&self) -> Vec<Vec<u32>> {
        let mut indexes: Vec<Vec<u32>> = self
            .per_index
            .iter()
            .map(|(index, _)| index.clone())
            .collect();
        indexes.sort();
        indexes
    }

    /// `set().union(*per_index.values())`, sorted.
    pub fn sorted_components(&self) -> Vec<Vec<u32>> {
        let mut all: Vec<Vec<u32>> = Vec::new();
        for (_, parts) in &self.per_index {
            for part in parts {
                if !all.contains(part) {
                    all.push(part.clone());
                }
            }
        }
        all.sort();
        all
    }
}

/// `for k in keys`: what the loop sees. A dict yields its keys (of any type), a string its characters, bytes their integers.
fn iterate_keys(keys: &PyValue) -> Done<Vec<PyValue>> {
    match keys.peel() {
        PyValue::List(items) | PyValue::Tuple(items) => Ok(items.clone()),
        PyValue::Dict(map) => map
            .keyed_entries()
            .map(|(other, key, _)| match other {
                None => Ok(PyValue::Str(key.clone())),
                Some(other) => match &other.value {
                    Some(value) => Ok((**value).clone()),
                    None => Err(Thrown::builtin(
                        "TypeError",
                        "a dict key without its value is outside the DTO profile",
                    )),
                },
            })
            .collect(),
        PyValue::Str(text) => Ok(text
            .code_points()
            .iter()
            .map(|&point| PyValue::Str(PyStr::from_code_points(vec![point])))
            .collect()),
        PyValue::Bytes(bytes) => Ok(bytes.iter().map(|&byte| PyValue::int(byte)).collect()),
        // a set yields its members in the order its caller observed (an input fact of the live set)
        PyValue::Opaque(opaque) if opaque.type_name == "set" => match &opaque.typed {
            Typed::Set(members) => Ok(members.clone()),
            _ => Err(Thrown::builtin(
                "TypeError",
                "a set without its members is outside the DTO profile",
            )),
        },
        other => Err(Thrown::builtin(
            "TypeError",
            &format!("'{}' object is not iterable", other.type_name()),
        )),
    }
}

/// `analyze(keys)`: the spec-decoder keys of `keys`, per index, and how many keys matched (a duplicate counts every time).
pub fn analyze_keys(keys: &PyValue) -> Done<Layout> {
    let mut layout = Layout {
        per_index: Vec::new(),
        total: 0,
    };
    for key in iterate_keys(keys)? {
        let points = match key.peel() {
            PyValue::Str(text) => text.code_points().to_vec(),
            PyValue::Bytes(_) => {
                return Err(Thrown::builtin(
                    "TypeError",
                    "cannot use a string pattern on a bytes-like object",
                ))
            }
            other => {
                return Err(Thrown::builtin(
                    "TypeError",
                    &format!(
                        "expected string or bytes-like object, got '{}'",
                        other.type_name()
                    ),
                ))
            }
        };
        let Some(found) = match_spec_key(&points) else {
            continue;
        };
        layout.total += 1;
        let index = found
            .index
            .unwrap_or_else(|| "block".chars().map(u32::from).collect());
        let component = first_component(&found.rest);
        match layout.per_index.iter_mut().find(|(own, _)| *own == index) {
            Some((_, parts)) => {
                if !parts.contains(&component) {
                    parts.push(component);
                }
            }
            None => layout.per_index.push((index, vec![component])),
        }
    }
    Ok(layout)
}

fn set_value(parts: &[Vec<u32>]) -> PyValue {
    let mut sorted: Vec<&Vec<u32>> = parts.iter().collect();
    sorted.sort();
    let members: Vec<PyValue> = sorted
        .iter()
        .map(|part| PyValue::Str(text_of(part)))
        .collect();
    let shown: Vec<String> = members.iter().map(PyValue::repr).collect();
    PyValue::Opaque(Opaque {
        type_name: "set",
        repr: if shown.is_empty() {
            "set()".to_string()
        } else {
            format!("{{{}}}", shown.join(", "))
        },
        truthy: !shown.is_empty(),
        typed: Typed::Set(members),
        identity: None,
    })
}

/// `analyze(keys)` as the value the original returns: `(per_index, total)` with `per_index` a `dict` of `set`s.
pub fn analyze(keys: &PyValue) -> Done<PyValue> {
    let layout = analyze_keys(keys)?;
    let mut per_index = PyDict::new();
    for (index, parts) in &layout.per_index {
        per_index.insert(text_of(index), set_value(parts));
    }
    Ok(PyValue::Tuple(vec![
        PyValue::Dict(per_index),
        PyValue::int(layout.total),
    ]))
}

fn list_text(points: &[Vec<u32>]) -> PyStr {
    PyValue::List(
        points
            .iter()
            .map(|each| PyValue::Str(text_of(each)))
            .collect(),
    )
    .display_text()
}

fn exit(code: i64) -> Done<PyValue> {
    Err(exit_with(code))
}

/// `main()`: every path ends in `sys.exit`, so the outcome is always the `SystemExit` (or the exception that stopped the command).
pub fn main(world: &mut LayoutWorld<'_>) -> Done<PyValue> {
    let values = parse_arguments(world)?;
    let model_dir = text_option(&values, "model_dir").unwrap_or_default();
    let method_argument = text_option(&values, "method").filter(|text| !text.is_empty());
    let config_argument = text_option(&values, "config").filter(|text| !text.is_empty());
    // `a.method or (read_method_from_config(a.config) if a.config else None)`
    let method = match (method_argument, config_argument) {
        (Some(method), _) => PyValue::Str(method),
        (None, Some(config)) => {
            let outcome = read_method_from_config(world, &PyValue::Str(config));
            world.through(SiteId::MainReadMethod, outcome)?
        }
        (None, None) => PyValue::None,
    };
    if !method.is_truthy() {
        print(
            world,
            &literal("[spec-layout] SKIP: speculative method 없음 — 검사 비대상"),
        )?;
        return exit(0);
    }

    if !isdir(world, &model_dir) {
        let message = line(&[
            &literal("[spec-layout] FAIL: 체크포인트 디렉터리 부재 — "),
            &model_dir,
        ]);
        print_error(world, &message)?;
        return exit(3);
    }

    let outcome = load_index_keys(world, &PyValue::Str(model_dir.clone()));
    let loaded = world.through(SiteId::MainLoadIndex, outcome)?;
    let PyValue::Tuple(pair) = &loaded else {
        unreachable!("load_index_keys returns a pair")
    };
    let (keys, source) = (&pair[0], py_str(&pair[1]));
    if matches!(keys, PyValue::None) {
        print(world, &line(&[&literal("[spec-layout] SKIP: "), &source]))?;
        return exit(0);
    }

    let layout = analyze_keys(keys)?;
    let method_text = py_str(&method);
    if layout.per_index.is_empty() {
        print(
            world,
            &line(&[
                &literal("[spec-layout] SKIP: spec-decoder 계열 키 0건(method="),
                &method_text,
                &literal(") — 드래프트 가중치를 별도 경로에서 받는 구성일 수 있다. 검사 생략"),
            ]),
        )?;
        return exit(0);
    }

    let components = layout.sorted_components();
    let dspark_hits: Vec<Vec<u32>> = components
        .iter()
        .filter(|component| holds_dspark_marker(component))
        .cloned()
        .collect();
    let outcome = read_num_nextn(world, &PyValue::Str(model_dir.clone()));
    let nextn = world.through(SiteId::MainReadNextn, outcome)?;
    let nextn_text = py_str(&nextn);
    let key_count = match keys {
        PyValue::List(items) => items.len(),
        _ => 0,
    };

    print(
        world,
        &line(&[
            &literal("[spec-layout] provenance=measured src="),
            &source,
            &literal(&format!(
                " 전체텐서={key_count} spec계열={} 인덱스=",
                layout.total
            )),
            &list_text(&layout.sorted_indexes()),
            &literal(" num_nextn_predict_layers="),
            &nextn_text,
        ]),
    )?;
    print(
        world,
        &line(&[&literal("[spec-layout] 부품집합="), &list_text(&components)]),
    )?;

    let is_method = |name: &str| matches!(method.peel(), PyValue::Str(text) if text.equals(name));

    if is_method("mtp") && !dspark_hits.is_empty() {
        let visible = if is_one_or_none(&nextn) {
            literal("mtp.0")
        } else {
            let minus = minus_one(&nextn).inspect_err(|_| {
                world.frames = vec![SiteId::MainNextnMinusOne];
            })?;
            line(&[&literal("mtp.0.."), &py_str(&minus)])
        };
        let message = line(&[
            &literal("[spec-layout] STOP: 체크포인트의 spec 가중치가 **plain MTP 구조가 아니다**.\n  DSpark 전용 부품 검출: "),
            &list_text(&dspark_hits),
            &literal("\n  stock 의 MTP 모듈에는 이 부품들을 담을 슬롯이 없다 → 로드 중 KeyError 로 죽는다.\n  (num_nextn_predict_layers="),
            &nextn_text,
            &literal(" 이므로 stock 이 보는 블록은 "),
            &visible,
            &literal(
                " 인데,\n   하필 그 블록이 DSpark 전용 텐서를 가장 많이 가진 블록이다.)\n  → `method: mtp` 는 이 체크포인트에 **구조적으로 부적용**이다. `method: dspark`\n     (또는 그 경로를 여는 이미지 변종)로 가라. 근거: testlog_26081419 §2.4·§2.5.\n  이 판정은 로드 13분이 아니라 index.json 키 스캔으로 나왔다.",
            ),
        ]);
        print_error(world, &message)?;
        return exit(8);
    }

    if is_method("dspark") && dspark_hits.is_empty() {
        let message = line(&[
            &literal("[spec-layout] STOP: `method: dspark` 인데 체크포인트에 DSpark 부품이 없다.\n  검출된 부품="),
            &list_text(&components),
            &literal("\n  → 드래프터가 자기 가중치를 못 찾는다. method 또는 체크포인트를 재확인하라."),
        ]);
        print_error(world, &message)?;
        return exit(8);
    }

    let unknown: Vec<Vec<u32>> = components
        .iter()
        .filter(|component| !is_plain_component(component) && !dspark_hits.contains(component))
        .cloned()
        .collect();
    if !unknown.is_empty() {
        let message = line(&[
            &literal("[spec-layout] ⚠ 미분류 부품(차단하지 않음 · tripwire 확장 후보): "),
            &list_text(&unknown),
        ]);
        print_error(world, &message)?;
    }

    print(
        world,
        &line(&[
            &literal("[spec-layout] OK: method="),
            &method_text,
            &literal(" ↔ 체크포인트 구조 정합(조기 차단 사유 없음). ⚠ 서빙 성공을 뜻하지 않는다 — 커널·디스패치 층은 스모크가 판정한다."),
        ]),
    )?;
    exit(0)
}
