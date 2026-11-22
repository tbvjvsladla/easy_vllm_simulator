//! Checks of the `role-contract` library with its own fixtures (no saved case, no shell): the Python behaviours of the owner functions that the
//! frozen cases do not all reach, and the loader over a real file.

use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use role_contract::load::{load_yaml, LoadFailure};
use role_contract::*;

fn s(text: &str) -> PyValue {
    PyValue::str(text)
}
fn dict(pairs: Vec<(&str, PyValue)>) -> PyValue {
    let mut d = PyDict::new();
    for (k, v) in pairs {
        d.set(k, v);
    }
    PyValue::Dict(d)
}
fn value_of(result: Result<Verdict, ContractViolation>) -> PyValue {
    result.expect("a verdict").value
}

#[test]
fn equality_follows_python_across_the_numeric_tower_and_a_none_role_matches_nodes_without_one() {
    assert!(py_eq(&PyValue::Bool(false), &PyValue::int(0)));
    assert!(py_eq(&PyValue::int(1), &PyValue::Float(1.0)));
    assert!(!py_eq(&PyValue::str("0"), &PyValue::int(0)));
    // `False == 0`: a node whose role is False is the one asked for with role 0
    let nodes = PyValue::List(vec![
        dict(vec![("role", PyValue::Bool(false))]),
        dict(vec![("role", s("sub"))]),
    ]);
    assert_eq!(
        value_of(resolve_rank(&s("multi"), &nodes, &PyValue::int(0))),
        PyValue::int(0)
    );
    // `None == None`: with role None a node WITHOUT the key matches
    let nodes = PyValue::List(vec![
        dict(vec![("host", s("h"))]),
        dict(vec![("role", s("sub"))]),
    ]);
    assert_eq!(
        value_of(resolve_rank(&s("multi"), &nodes, &PyValue::None)),
        PyValue::int(0)
    );
    // a non-dict element never matches; a non-list nodes is "nodes[] is empty"
    let odd = PyValue::List(vec![s("main"), PyValue::int(3)]);
    assert_eq!(
        resolve_rank(&s("multi"), &odd, &s("main"))
            .unwrap_err()
            .code,
        "RANK_UNRESOLVED"
    );
    assert_eq!(
        resolve_rank(&s("multi"), &s("main"), &s("main"))
            .unwrap_err()
            .code,
        "RANK_UNRESOLVED"
    );
}

#[test]
fn topology_truthiness_and_the_closed_vocabulary_are_python_not_string_tests() {
    // a falsy override falls back to the manifest; a truthy invalid one is itself the unknown topology
    let doc = dict(vec![
        ("topology", s("single")),
        ("nodes", PyValue::List(vec![])),
    ]);
    assert_eq!(
        evaluate_manifest(&doc, &PyValue::str(""), &s("sub")).topology(),
        &s("single")
    );
    assert_eq!(
        evaluate_manifest(&doc, &PyValue::int(0), &s("sub")).topology(),
        &s("single")
    );
    let invalid = evaluate_manifest(&doc, &PyValue::Bool(true), &s("sub"));
    assert_eq!(invalid.violations()[0].code, "TOPOLOGY_UNKNOWN");
    assert_eq!(
        invalid.delivery_plane().source,
        PyStr::new("fail-closed:unevaluated")
    );
    // a non-dict document is an empty one
    let e = evaluate_manifest(&PyValue::int(7), &PyValue::None, &s("sub"));
    assert_eq!(e.violations()[0].code, "TOPOLOGY_UNKNOWN");
    // the message names the caller's value with its Python repr
    let violation = assert_topology(&PyValue::List(vec![s("single")])).unwrap_err();
    assert!(violation
        .message
        .to_utf8()
        .unwrap()
        .starts_with("topology=['single'] 는 선언 어휘 ['single', 'multi'] 밖이다"));
    assert_eq!(violation.field, Some("topology"));
}

#[test]
fn declared_sub_mode_types_and_whitespace_follow_the_source() {
    let non_string = resolve_sub_mode(&s("single"), &PyValue::Bool(false)).unwrap_err();
    assert_eq!(non_string.code, "SUB_MODE_UNKNOWN");
    assert!(non_string
        .message
        .to_utf8()
        .unwrap()
        .contains("(got False)"));
    // surrounding whitespace is stripped before the vocabulary test
    assert_eq!(
        value_of(resolve_sub_mode(&s("multi"), &s("  ray-worker \n"))),
        s("ray-worker")
    );
    // the helper functions are not re-validated as a full topology: delivery and tool plane take any known sub_mode under any valid topology
    assert_eq!(
        value_of(delivery_plane(&s("single"), &s("ray-worker"))),
        s("active")
    );
    assert_eq!(
        value_of(tool_plane(&s("multi"), &s("a2a-agent"))),
        PyValue::List(
            [
                "vllm-recipe-explorer",
                "adversarial-benchmark",
                "upstream-version-watch",
                "hint-publisher",
                "wiki-desk"
            ]
            .iter()
            .map(|x| s(x))
            .collect()
        )
    );
    assert_eq!(
        value_of(sub_self_publishes(&s("multi"), &s("a2a-agent"))),
        PyValue::Bool(true)
    );
}

#[test]
fn path_join_follows_posixpath_for_absolute_parts_empty_repo_and_bad_types() {
    let join = |repo: PyValue, topology: PyValue| {
        manifest_path_for(&repo, &topology).map(|p| p.to_utf8().unwrap())
    };
    assert_eq!(
        join(s("/r"), s("single")).unwrap(),
        "/r/output/single/manifest.yaml"
    );
    assert_eq!(
        join(s("/r/"), s("multi")).unwrap(),
        "/r/output/multi/manifest.yaml"
    );
    assert_eq!(
        join(s(""), s("single")).unwrap(),
        "output/single/manifest.yaml"
    );
    // an absolute component restarts the path
    assert_eq!(join(s("/r"), s("/abs")).unwrap(), "/abs/manifest.yaml");
    assert_eq!(
        join(PyValue::None, s("single")).unwrap_err(),
        PathJoinError::NotPathLike("NoneType")
    );
    assert_eq!(
        join(s("/r"), PyValue::Bool(false)).unwrap_err().message(),
        "join() argument must be str, bytes, or os.PathLike object, not 'bool'"
    );
    assert_eq!(
        join(PyValue::Bytes(b"/r".to_vec()), s("single")).unwrap_err(),
        PathJoinError::MixedStrAndBytes
    );
}

#[test]
fn the_loader_reads_the_file_like_a_text_file_and_reports_marked_errors_as_data() {
    let dir = std::env::temp_dir().join(format!("role-contract-checks-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("m.yaml");
    std::fs::write(&path, "topology: multi\nnodes: [\n").unwrap();
    let name = PyValue::str(&path.to_string_lossy());
    match load_yaml(&name) {
        Err(LoadFailure::Yaml(error)) => {
            assert_eq!(error.class, "ParserError");
            assert!(
                matches!(&error.detail, agent_runtime_support::pyyaml::YamlDetail::Marked(detail) if matches!(&detail.problem_mark, Some(mark) if mark.index == 25 && mark.line == 2 && mark.column == 0))
            );
        }
        other => panic!("{other:?}"),
    }
    // the document is not serialised: an unused 617-digit key loads, and the evaluation does not look at it
    let big = "9".repeat(617);
    std::fs::write(
        &path,
        format!("{big}: 1\ntopology: single\nnodes:\n  - role: main\n  - role: sub\n"),
    )
    .unwrap();
    let doc = load_yaml(&name).expect("the unused big key loads");
    let evaluation = evaluate_manifest(&doc, &PyValue::None, &s("sub"));
    assert!(evaluation.violations().is_empty());
    assert_eq!(evaluation.delivery_plane().value, s("dormant"));
    assert!(matches!(
        load_yaml(&PyValue::None),
        Err(LoadFailure::NotPathLike("NoneType"))
    ));
    assert!(matches!(
        load_yaml(&PyValue::str(&dir.to_string_lossy())),
        Err(LoadFailure::Os { .. })
    ));
    std::fs::remove_dir_all(dir).unwrap();
}

fn trace(violation: &ContractViolation) -> Vec<(&'static str, u32, &'static str)> {
    violation
        .frames()
        .iter()
        .map(|f| (f.file, f.line, f.function))
        .collect()
}

#[test]
fn a_violation_keeps_the_helper_calls_it_passed_through_in_source_order() {
    let file = OWNER_FILE;
    // the raise itself
    assert_eq!(
        trace(&assert_topology(&PyValue::Bool(false)).unwrap_err()),
        vec![(file, 118, "assert_topology")]
    );
    // a function that validates the topology first: its call line, then the raise inside `assert_topology`
    assert_eq!(
        trace(&resolve_sub_mode(&s("auto"), &PyValue::None).unwrap_err()),
        vec![
            (file, 128, "resolve_sub_mode"),
            (file, 118, "assert_topology")
        ]
    );
    assert_eq!(
        trace(&resolve_rank(&s("auto"), &PyValue::None, &s("sub")).unwrap_err())[0],
        (file, 162, "resolve_rank")
    );
    assert_eq!(
        trace(&identity_authority(&s("auto")).unwrap_err())[0],
        (file, 196, "identity_authority")
    );
    assert_eq!(
        trace(&delivery_plane(&s("auto"), &s("x")).unwrap_err())[0],
        (file, 208, "delivery_plane")
    );
    // a raise of the function's own, after the topology was accepted
    assert_eq!(
        trace(&delivery_plane(&s("single"), &s("x")).unwrap_err()),
        vec![(file, 210, "delivery_plane")]
    );
    assert_eq!(
        trace(&resolve_sub_mode(&s("single"), &s("ray-worker")).unwrap_err()),
        vec![(file, 144, "resolve_sub_mode")]
    );
    // the helper `sub_self_publishes` calls `tool_plane`: its own call line, then the callee's raise (and, for a bad topology, the callee's own assert chain)
    assert_eq!(
        trace(&sub_self_publishes(&s("single"), &s("x")).unwrap_err()),
        vec![(file, 269, "sub_self_publishes"), (file, 253, "tool_plane")]
    );
    assert_eq!(
        trace(&sub_self_publishes(&s("auto"), &PyValue::None).unwrap_err()),
        vec![
            (file, 267, "sub_self_publishes"),
            (file, 118, "assert_topology")
        ]
    );
}

#[test]
fn path_join_and_loader_failures_carry_the_frozen_and_owner_frames() {
    let join = manifest_path_for(&s("/r"), &PyValue::Bool(false)).unwrap_err();
    let frames: Vec<_> = join
        .frames()
        .iter()
        .map(|f| (f.file, f.line, f.function))
        .collect();
    assert_eq!(
        frames,
        vec![
            (OWNER_FILE, 110, "manifest_path_for"),
            ("<frozen posixpath>", 90, "join"),
            ("<frozen genericpath>", 164, "_check_arg_types")
        ]
    );
    let mixed = manifest_path_for(&PyValue::Bytes(b"/r".to_vec()), &s("single")).unwrap_err();
    assert_eq!(mixed.frames()[2].line, 167);
    assert!(
        matches!(load_yaml(&PyValue::None), Err(failure) if failure.frames().unwrap()[0].line == 105)
    );
    let dir = std::env::temp_dir().join(format!("role-contract-frames-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("m.yaml");
    // a scanner error has no mapped traceback: the loader says so instead of guessing
    std::fs::write(&path, "a: @\n").unwrap();
    let failure = load_yaml(&PyValue::str(&path.to_string_lossy())).unwrap_err();
    assert!(failure.frames().is_none());
    // a parser error is mapped from the owner's `yaml.safe_load` line down to the raise
    std::fs::write(&path, "nodes: [\n").unwrap();
    let failure = load_yaml(&PyValue::str(&path.to_string_lossy())).unwrap_err();
    let frames = failure.frames().unwrap();
    assert_eq!((frames[0].function, frames[0].line), ("load_yaml", 106));
    assert_eq!(
        (frames[1].file, frames[1].line, frames[1].function),
        ("yaml/__init__.py", 125, "safe_load")
    );
    let last = frames.last().unwrap();
    assert_eq!(
        (last.file, last.line, last.function),
        ("yaml/parser.py", 369, "parse_node")
    );
    std::fs::remove_dir_all(dir).unwrap();
}

fn big(digits: &str) -> PyValue {
    PyValue::int(
        agent_runtime_support::pynumber::int_from_str(&PyStr::new(digits)).expect("decimal digits"),
    )
}

#[test]
fn numeric_equality_is_exact_for_large_integers_and_integer_float_pairs() {
    // 2**53 = 9007199254740992: the first integer a double cannot tell from its successor. Expectations are the mathematical integers.
    let (a, b) = (
        PyValue::int(9_007_199_254_740_992i64),
        PyValue::int(9_007_199_254_740_993i64),
    );
    assert!(py_eq(&a, &a.clone()));
    assert!(
        !py_eq(&a, &b),
        "adjacent integers above 2**53 are different numbers"
    );
    assert!(
        py_eq(&a, &PyValue::Float(9_007_199_254_740_992.0)),
        "2**53 is exactly a double"
    );
    assert!(
        !py_eq(&b, &PyValue::Float(9_007_199_254_740_992.0)),
        "2**53 + 1 is not that double"
    );
    assert!(
        !py_eq(&b, &PyValue::Float(9_007_199_254_740_994.0)),
        "nor the next double"
    );
    // far beyond i64: 2**100 is exactly a double, 2**100 + 1 is not equal to it; two distinct 400-digit integers differ
    let two_100 = PyValue::int(1u128 << 100);
    assert!(py_eq(&two_100, &PyValue::Float(2f64.powi(100))));
    assert!(!py_eq(
        &PyValue::int((1u128 << 100) + 1),
        &PyValue::Float(2f64.powi(100))
    ));
    assert!(!py_eq(
        &big(&format!("1{}", "0".repeat(400))),
        &big(&format!("1{}1", "0".repeat(399)))
    ));
    assert!(py_eq(
        &big(&format!("1{}", "0".repeat(400))),
        &big(&format!("1{}", "0".repeat(400)))
    ));
    // 10**30 is not the double 1e30 (which is 1000000000000000019884624838656)
    assert!(!py_eq(
        &big(&format!("1{}", "0".repeat(30))),
        &PyValue::Float(1e30)
    ));
    // an integer too large for any double equals neither a finite double nor infinity
    let huge = big(&format!("1{}", "0".repeat(400)));
    assert!(!py_eq(&huge, &PyValue::Float(f64::MAX)));
    assert!(!py_eq(&huge, &PyValue::Float(f64::INFINITY)));
}

#[test]
fn bool_zero_one_nan_and_infinity_follow_the_python_numbers() {
    assert!(py_eq(&PyValue::Bool(false), &PyValue::int(0)));
    assert!(py_eq(&PyValue::Bool(true), &PyValue::int(1)));
    assert!(py_eq(&PyValue::Bool(true), &PyValue::Float(1.0)));
    assert!(py_eq(&PyValue::Bool(false), &PyValue::Float(-0.0)));
    assert!(!py_eq(&PyValue::Bool(true), &PyValue::int(2)));
    assert!(!py_eq(&PyValue::Bool(false), &PyValue::Float(f64::NAN)));
    // a NaN is equal to nothing, itself included
    assert!(!py_eq(&PyValue::Float(f64::NAN), &PyValue::Float(f64::NAN)));
    assert!(!py_eq(&PyValue::Float(f64::NAN), &PyValue::int(0)));
    assert!(py_eq(
        &PyValue::Float(f64::INFINITY),
        &PyValue::Float(f64::INFINITY)
    ));
    assert!(!py_eq(
        &PyValue::Float(f64::INFINITY),
        &PyValue::Float(f64::NEG_INFINITY)
    ));
    assert!(!py_eq(
        &PyValue::Float(f64::INFINITY),
        &PyValue::int(i64::MAX)
    ));
    // containers keep the same exact rule inside, and a number never equals text
    assert!(!py_eq(
        &PyValue::List(vec![PyValue::int(9_007_199_254_740_992i64)]),
        &PyValue::List(vec![PyValue::int(9_007_199_254_740_993i64)])
    ));
    assert!(py_eq(
        &PyValue::List(vec![PyValue::Bool(true)]),
        &PyValue::List(vec![PyValue::Float(1.0)])
    ));
    assert!(!py_eq(&PyValue::int(1), &PyValue::str("1")));
}

#[test]
fn distinct_large_numeric_roles_are_not_the_same_role_in_the_rank_search() {
    let node = |role: PyValue| dict(vec![("role", role)]);
    let first = PyValue::int(9_007_199_254_740_992i64);
    let second = PyValue::int(9_007_199_254_740_993i64);
    let nodes = PyValue::List(vec![node(first.clone()), node(second.clone())]);
    // exactly one node has role 2**53 + 1: its index, not an ambiguity between two nodes a double would merge
    assert_eq!(
        value_of(resolve_rank(&s("multi"), &nodes, &second)),
        PyValue::int(1)
    );
    assert_eq!(
        value_of(resolve_rank(&s("multi"), &nodes, &first)),
        PyValue::int(0)
    );
    // the float 2**53 is exactly the first role only
    assert_eq!(
        value_of(resolve_rank(
            &s("multi"),
            &nodes,
            &PyValue::Float(9_007_199_254_740_992.0)
        )),
        PyValue::int(0)
    );
    // a role equal to neither is not found
    let missing =
        resolve_rank(&s("multi"), &nodes, &PyValue::int(9_007_199_254_740_994i64)).unwrap_err();
    assert_eq!(missing.code, "RANK_UNRESOLVED");
    // False == 0 stays: the frozen api-039 relation (role 0 finds a node whose role is False)
    let nodes = PyValue::List(vec![node(PyValue::Bool(false)), node(s("sub"))]);
    assert_eq!(
        value_of(resolve_rank(&s("multi"), &nodes, &PyValue::int(0))),
        PyValue::int(0)
    );
}

/// plan_26112012 #2: `envs/.env` is seen by lstat only and narrowed to `mode & 0o600`, never widened; a link, a directory or a failed
/// lstat is `unknown` and never corrected; the record carries the fixed relative path and octal modes, never the content.
#[test]
fn the_env_file_is_narrowed_to_owner_read_write_by_its_mode_alone() {
    use role_contract::env_file_mode::{narrow, narrowing_value, observe, EnvFileState, NarrowAction, ENV_FILE_REL};
    use std::os::unix::fs::PermissionsExt;
    let root = std::env::temp_dir().join(format!("rc-env-mode-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("envs")).unwrap();
    let file = root.join(ENV_FILE_REL);
    let content = "HF_TOKEN=hf_CHECK_VALUE_31\nGITHUB_TOKEN=ghp_CHECK_VALUE_31\n";
    let place = |mode: u32| {
        std::fs::write(&file, content).unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(mode)).unwrap();
    };
    let mode_now = || std::fs::symlink_metadata(&file).unwrap().permissions().mode() & 0o7777;
    for (from, to, action) in [
        (0o664, 0o600, NarrowAction::Narrowed),
        (0o640, 0o600, NarrowAction::Narrowed),
        (0o700, 0o600, NarrowAction::Narrowed),
        (0o644, 0o600, NarrowAction::Narrowed),
        (0o600, 0o600, NarrowAction::Unchanged),
        (0o400, 0o400, NarrowAction::Unchanged),
    ] {
        place(from);
        assert_eq!(observe(&root).is_wide(), from & !0o600 != 0, "{from:o}");
        let n = narrow(&root);
        assert_eq!((n.before, n.after, n.action), (EnvFileState::Regular(from), EnvFileState::Regular(to), action), "{from:o}");
        assert_eq!(mode_now(), to, "{from:o}");
        let text = narrowing_value(&n).repr();
        assert!(text.contains(&format!("'before': '{from:o}'")) && text.contains(&format!("'after': '{to:o}'")), "{text}");
        assert!(!text.contains("HF_TOKEN") && !text.contains("CHECK_VALUE") && !text.contains(&root.display().to_string()), "{text}");
    }
    std::fs::remove_file(&file).unwrap();
    assert_eq!(observe(&root), EnvFileState::Absent);
    assert_eq!(narrow(&root).action, NarrowAction::Absent);
    let target = root.join("target.env");
    std::fs::write(&target, content).unwrap();
    std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o664)).unwrap();
    std::os::unix::fs::symlink(&target, &file).unwrap();
    assert_eq!(observe(&root), EnvFileState::Unknown);
    let n = narrow(&root);
    assert_eq!(n.action, NarrowAction::Unknown);
    assert_eq!(std::fs::metadata(&target).unwrap().permissions().mode() & 0o7777, 0o664, "a link is never followed");
    std::fs::remove_file(&file).unwrap();
    std::fs::create_dir(&file).unwrap();
    assert_eq!((observe(&root).status(), narrow(&root).action), ("unknown", NarrowAction::Unknown));
    let _ = std::fs::remove_dir_all(&root);
}

/// plan_26112012 #2b: the manifest-pair verdict over loaded manifests (no file read here): one sub each, both fields equal.
#[test]
fn the_sub_workspace_is_shared_only_when_one_sub_has_the_same_node_id_and_work_dir_in_both_manifests() {
    let doc = |subs: Vec<(&str, &str)>| {
        let mut nodes = vec![dict(vec![("role", s("main"))])];
        for (id, dir) in subs {
            nodes.push(dict(vec![("role", s("sub")), ("node_id", s(id)), ("work_dir", s(dir)), ("host", s(id))]));
        }
        dict(vec![("nodes", PyValue::List(nodes))])
    };
    let value = |a: &PyValue, b: &PyValue| sub_workspace_shared(Some(a), Some(b)).value;
    let one = doc(vec![("n1", "/w")]);
    assert_eq!(value(&one, &one), s("shared"));
    assert_eq!(value(&one, &doc(vec![("n2", "/w")])), s("distinct"));
    assert_eq!(value(&one, &doc(vec![("n1", "/w2")])), s("distinct"));
    assert_eq!(value(&one, &doc(vec![("n1", "/w"), ("n3", "/w3")])), s("unknown"), "two subs: not one workspace");
    assert_eq!(value(&one, &doc(vec![])), s("unknown"));
    assert_eq!(value(&one, &s("not a mapping")), s("unknown"));
    assert_eq!(sub_workspace_shared(None, Some(&one)).value, s("unknown"));
    let verdict = sub_workspace_shared(Some(&one), Some(&one));
    assert!(verdict.source.equals(SUB_WORKSPACE_SHARED_SOURCE));
    assert!(!verdict.to_value().repr().contains("/w"));
}
