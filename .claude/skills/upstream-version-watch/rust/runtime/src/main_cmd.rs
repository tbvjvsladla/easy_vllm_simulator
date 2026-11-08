//! `main()` and `_self_test()` of `resolve_build_track.py`.

use crate::business::{map_torch_cuda_arch, require, resolve, sm_from_manifest};
use crate::manifest::{is_file, load_manifest, World};
use crate::thrown::{Done, Thrown};
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use recipe_runtime::cliparse::{
    format_usage, parse, terminal_width, Kind, Opt, Outcome, Spec, Value,
};
use recipe_runtime::escalation::Raised;

/// The program name of `sys.argv[0]` in the command line (every usage and error text says `resolve_build_track.py`).
pub const PROGRAM: &str = "resolve_build_track.py";

const TOPOLOGIES: &[&str] = &["single", "multi"];

fn option(
    flags: &'static [&'static str],
    dest: &'static str,
    kind: Kind,
    help: Option<&'static str>,
) -> Opt {
    Opt {
        flags,
        dest,
        kind,
        help,
        required: false,
        group: None,
    }
}

/// The parser of `main` for the program name `prog`.
pub fn spec(prog: &str) -> Spec {
    Spec {
        prog: intern(prog),
        description: "resolve_build_track.py — build_track 제안 + torch_cuda_arch (proposer; 스모크가 최종 중재)",
        options: vec![
            option(&["-h", "--help"], "help", Kind::Help, None),
            option(
                &["--torch-pin"],
                "torch_pin",
                Kind::Text,
                Some("resolve_torch_pin.py 의 torch_pin (예: 2.11.0)"),
            ),
            option(
                &["--sm-arch"],
                "sm_arch",
                Kind::Text,
                Some("GPU compute-capability (예: sm_121a | 121a | 12.1a). manifest 보다 우선"),
            ),
            option(
                &["--manifest"],
                "manifest",
                Kind::Text,
                Some("--sm-arch 미지정 시 여기서 compute-capability 읽음(기본: output/<--topology>/manifest.yaml)"),
            ),
            option(
                &["--topology"],
                "topology",
                Kind::Choice(TOPOLOGIES),
                Some("manifest 통로 파생(output/<t>/manifest.yaml) — 루트 manifest.yaml 은 없다(plan_26103120)"),
            ),
            option(
                &["--self-test"],
                "self_test",
                Kind::Flag,
                Some("내장 self-test"),
            ),
        ],
        group_required: false,
        positional: None,
    }
}

/// The program names the parsers were built for: `argparse` takes `os.path.basename(sys.argv[0])`, and a [`Spec`] holds a `'static` name,
/// so every distinct name is kept once (a process meets a handful).
pub(crate) fn intern(name: &str) -> &'static str {
    use std::sync::{Mutex, OnceLock};
    static NAMES: OnceLock<Mutex<Vec<&'static str>>> = OnceLock::new();
    let mut names = NAMES
        .get_or_init(|| Mutex::new(Vec::new()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(held) = names.iter().find(|held| **held == name) {
        return held;
    }
    let leaked: &'static str = Box::leak(name.to_string().into_boxed_str());
    names.push(leaked);
    leaked
}

/// `os.path.basename(path)`: what follows the last `/`.
pub(crate) fn basename(path: &PyStr) -> String {
    let shown = path.to_utf8().unwrap_or_else(|| {
        String::from_utf8_lossy(&path.to_surrogateescape_bytes().unwrap_or_default()).into_owned()
    });
    shown.rsplit('/').next().unwrap_or("").to_string()
}

/// `print(text)`: the text and then its newline, two writes, each of which can fail before the next step.
fn print(world: &mut World<'_>, text: &PyStr) -> Done<()> {
    world.host.out(text)?;
    world.host.out(&PyStr::new("\n"))
}

pub(crate) fn exit_with(code: i64) -> Box<Thrown> {
    let mut raised = Raised::new("SystemExit", "builtins", PyStr::new(&code.to_string()));
    raised.code = Some(Box::new(PyValue::int(code)));
    Thrown::plain(raised)
}

/// The `ArgumentError` behind a usage error whose message names an argument; the errors `parse_args` itself reports (`unrecognized arguments`)
/// are not raised from one.
pub(crate) fn argument_error(message: &PyStr) -> Option<Raised> {
    let shown = message.to_utf8()?;
    let detail = shown.trim_end_matches('\n').rsplit_once(": error: ")?.1;
    detail
        .starts_with("argument ")
        .then(|| Raised::new("ArgumentError", "argparse", PyStr::new(detail)))
}

fn text_value(values: &std::collections::BTreeMap<&'static str, Value>, key: &str) -> PyValue {
    match values.get(key) {
        Some(Value::Text(text)) => PyValue::Str(text.clone()),
        _ => PyValue::None,
    }
}

/// `ap.parse_args()` over `sys.argv[1:]`: the values, or the `SystemExit` of help and of a usage error, with what argparse wrote.
fn parse_arguments(world: &mut World<'_>) -> Done<std::collections::BTreeMap<&'static str, Value>> {
    let program = world
        .argv
        .first()
        .ok_or_else(|| Thrown::builtin("IndexError", "list index out of range"))?;
    let spec = spec(&basename(program));
    let outcome = parse(&spec, &world.argv[1..], world.columns);
    let (status, stdout, stderr) = match outcome {
        Outcome::Values(values) => return Ok(values),
        Outcome::Exit {
            status,
            stdout,
            stderr,
        } => (status, stdout, stderr),
    };
    let mut message = None;
    if !stdout.is_empty() {
        world.host.parser_help(&stdout);
    }
    if !stderr.is_empty() {
        let usage = PyStr::new(&format_usage(&spec, terminal_width(world.columns)));
        let (head, rest) = split_usage(&usage, &stderr);
        world.host.parser_error(&head, &rest);
        message = Some(rest);
    }
    // A parser output write that failed with an exception argparse does not swallow stopped the parse there: that exception is the outcome,
    // in place of the `SystemExit`; raised inside the `except ArgumentError` handler it has that error as its implicit context.
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

/// The usage the parser put first in `report`, and the rest; no usage at the start means no usage part.
pub(crate) fn split_usage(usage: &PyStr, report: &PyStr) -> (PyStr, PyStr) {
    let (head, all) = (usage.code_points(), report.code_points());
    if all.starts_with(head) {
        (
            usage.clone(),
            PyStr::from_code_points(all[head.len()..].to_vec()),
        )
    } else {
        (PyStr::default(), report.clone())
    }
}

/// `_self_test()`: the same assertions through the same functions, then the one line it prints.
pub fn self_test(world: &mut World<'_>) -> Done<PyValue> {
    let none = PyValue::None;
    let decision = |pin: &str, arch: &PyValue| -> Done<String> {
        let resolved = resolve(&PyValue::str(pin), arch)?;
        let PyValue::Dict(map) = resolved else {
            unreachable!("resolve returns a dict")
        };
        let Some(PyValue::Dict(track)) = map.get("build_track") else {
            unreachable!("a build_track")
        };
        Ok(match track.get("decision") {
            Some(PyValue::Str(text)) => text.to_utf8().unwrap_or_default(),
            _ => String::new(),
        })
    };
    let check =
        |condition: bool, message: &str| require(&PyValue::Bool(condition), &PyValue::str(message));
    check(
        decision("2.11.0", &none)? == "source-build",
        "2.11 source build",
    )?;
    check(
        decision("2.11.0a0+a6c236b", &none)? == "source-build",
        "2.11 alpha source build",
    )?;
    check(
        decision("2.12.0", &none)? == "source-build",
        "2.12 source build",
    )?;
    check(decision("2.10.0", &none)? == "wheel", "2.10 wheel")?;
    check(decision("2.9.1", &none)? == "wheel", "2.9 wheel")?;
    let resolved = resolve(&PyValue::str("2.11.0"), &none)?;
    let arbiter = match &resolved {
        PyValue::Dict(map) => match map.get("build_track") {
            Some(PyValue::Dict(track)) => track.get("arbiter").cloned(),
            _ => None,
        },
        _ => None,
    };
    check(
        matches!(arbiter, Some(PyValue::Str(ref text)) if text.equals("smoke")),
        "smoke arbiter",
    )?;
    let maps = |arg: PyValue, want: Option<&str>, message: &str| -> Done<PyValue> {
        let got = map_torch_cuda_arch(&arg)?;
        let same = match (&got, want) {
            (PyValue::Str(text), Some(want)) => text.equals(want),
            (PyValue::None, None) => true,
            _ => false,
        };
        check(same, message)
    };
    maps(PyValue::str("sm_121a"), Some("12.1a"), "sm_121a mapping")?;
    maps(PyValue::str("121a"), Some("12.1a"), "121a mapping")?;
    maps(PyValue::str("sm_90a"), Some("9.0a"), "sm_90a mapping")?;
    maps(PyValue::str("89"), Some("8.9"), "89 mapping")?;
    maps(PyValue::str("12.1a"), Some("12.1a"), "12.1a mapping")?;
    maps(
        PyValue::str("12.0"),
        Some("12.0"),
        "12.0(RTX 5090 · nvidia-smi compute_cap) mapping",
    )?;
    maps(PyValue::None, None, "None arch mapping")?;
    check(
        decision("garbage", &none)? == "source-build",
        "invalid version fail-closed",
    )?;
    print(
        world,
        &PyStr::new("[build_track] self-test OK — track 제안(wheel/source) + torch_cuda_arch 결정론 매핑 정상"),
    )?;
    Ok(PyValue::None)
}

/// `main()`: reads `sys.argv[1:]`; the proposal is printed as JSON (`ensure_ascii=False`, indent 2).
pub fn main(world: &mut World<'_>) -> Done<PyValue> {
    let values = parse_arguments(world)?;
    if values.contains_key("self_test") {
        self_test(world)?;
        return Ok(PyValue::None);
    }
    let torch_pin = text_value(&values, "torch_pin");
    if !torch_pin.is_truthy() {
        let line =
            PyStr::new("{\"error\": \"--torch-pin 필요 (resolve_torch_pin.py 의 torch_pin)\"}");
        world.host.err(&line)?;
        world.host.err(&PyStr::new("\n"))?;
        return Err(exit_with(2));
    }
    let mut sm_arch = text_value(&values, "sm_arch");
    let mut sm_source = PyValue::str("--sm-arch");
    if !sm_arch.is_truthy() {
        let manifest_option = text_value(&values, "manifest");
        let mpath = if manifest_option.is_truthy() {
            manifest_option
        } else if let Some(Value::Text(topology)) = values.get("topology") {
            let mut path = PyStr::new("output/");
            path.push_py(topology);
            path.push_str("/manifest.yaml");
            PyValue::Str(path)
        } else {
            PyValue::str("manifest.yaml")
        };
        let PyValue::Str(path_text) = &mpath else {
            unreachable!("a manifest path is text")
        };
        let manifest = if is_file(world, path_text) {
            load_manifest(world, &mpath)?
        } else {
            PyValue::Dict(PyDict::new())
        };
        let PyValue::Tuple(pair) = sm_from_manifest(&manifest)? else {
            unreachable!("sm_from_manifest returns a pair")
        };
        sm_arch = pair[0].clone();
        sm_source = match &pair[1] {
            PyValue::Str(key) => {
                let mut source = PyStr::new("manifest[");
                source.push_py(key);
                source.push_str("]");
                PyValue::Str(source)
            }
            _ => PyValue::None,
        };
    }
    let PyValue::Dict(mut out) = resolve(&torch_pin, &sm_arch)? else {
        unreachable!("resolve returns a dict")
    };
    let Some(PyValue::Dict(mut source_build)) = out.get("source_build").cloned() else {
        unreachable!("a source_build")
    };
    source_build.set("sm_arch_input", sm_arch);
    source_build.set("sm_arch_source", sm_source);
    if matches!(source_build.get("torch_cuda_arch"), Some(PyValue::None)) {
        source_build.set(
            "note",
            PyValue::str("torch_cuda_arch 미해소 — --sm-arch 또는 manifest compute_capability 필요 (source-build 트랙이면 TORCH_CUDA_ARCH_LIST 빈값 → 렌더 fail-loud)"),
        );
    }
    out.set("source_build", PyValue::Dict(source_build));
    let shown = PyValue::Dict(out)
        .dumps_pretty()
        .map_err(|error| Thrown::builtin(error.python_type(), &error.to_string()))?;
    print(world, &shown)?;
    Ok(PyValue::None)
}
