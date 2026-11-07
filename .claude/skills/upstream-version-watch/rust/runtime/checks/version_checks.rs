//! Checks of the ported `resolve_torch_pin.norm_prefix` / `pick_bound` (R35 stage A): every one of the 220 saved original results (typed value, origin of an
//! original-input return, exception chain with constructor arguments, arguments after the call, streams) through the public service, the
//! source-derived behaviours the corpus does not name, the benchmark fixture loader and the normal program.
//!
//! The saved controls are read through the environment variable `R35_REFERENCE` (the directory that holds `api-001` ... `api-220`); the production
//! code contains no path, label or expected value. These checks start no benchmark and no Python.

use agent_runtime_support::pyjson::loads;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use recipe_runtime::observe::same_value;
use std::path::PathBuf;
use upstream_runtime::torch_pin::{norm_prefix, pick_bound};
use upstream_runtime::version_api::answer;

fn reference() -> PathBuf {
    PathBuf::from(
        std::env::var("R35_REFERENCE")
            .expect("R35_REFERENCE must name the saved reference directory (api-001 ... api-220)"),
    )
}

fn read(path: PathBuf) -> PyValue {
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    loads(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn dict(value: &PyValue) -> &PyDict {
    match value {
        PyValue::Dict(map) => map,
        other => panic!("not an object: {}", other.repr()),
    }
}

fn text_of(value: &PyValue) -> String {
    match value {
        PyValue::Str(text) => text.to_utf8().expect("text"),
        other => panic!("not text: {}", other.repr()),
    }
}

fn list(value: &PyValue) -> &[PyValue] {
    match value {
        PyValue::List(items) => items,
        other => panic!("not an array: {}", other.repr()),
    }
}

fn index_of(value: &PyValue) -> usize {
    match value {
        PyValue::Int(number) => usize::try_from(number.clone()).expect("index"),
        other => panic!("not an index: {}", other.repr()),
    }
}

/// The node of a DTO (`["list"|"tuple", [..]]`, `["dict", [[k, v], ..]]`) one step of a saved path leads to.
fn step_into<'a>(node: &'a mut PyValue, kind: &str, at: usize) -> &'a mut PyValue {
    let PyValue::List(parts) = node else {
        panic!("a DTO is an array")
    };
    let PyValue::List(members) = &mut parts[1] else {
        panic!("a DTO holds an array")
    };
    match kind {
        "item" => &mut members[at],
        "key" | "value" => {
            let PyValue::List(pair) = &mut members[at] else {
                panic!("a dict entry is a pair")
            };
            &mut pair[usize::from(kind == "value")]
        }
        other => panic!("unknown step {other}"),
    }
}

type Path = Vec<(String, Option<usize>)>;

fn steps_of(path: &PyValue) -> Path {
    list(path)
        .iter()
        .map(|step| {
            let step = list(step);
            (text_of(&step[0]), step.get(1).map(index_of))
        })
        .collect()
}

/// Gives every saved set argument its 3-tag input form: the order the original observed (its `set_item i` input nodes) as a permutation of the
/// sorted members. Nothing is re-sorted: the order comes from the saved input graph.
fn with_observed_set_order(args: &mut PyValue, kwargs: &mut PyValue, nodes: &[PyValue]) {
    let sets: Vec<Path> = nodes
        .iter()
        .filter_map(|node| {
            let node = dict(node);
            let PyValue::List(value) = node.get("value").expect("value") else {
                return None;
            };
            (text_of(&value[0]) == "set" && value.len() == 2)
                .then(|| steps_of(node.get("path").expect("path")))
        })
        .collect();
    for path in sets {
        let members: Vec<String> = {
            let node = nodes
                .iter()
                .find(|node| steps_of(dict(node).get("path").unwrap()) == path)
                .unwrap();
            let PyValue::List(parts) = dict(node).get("value").unwrap() else {
                unreachable!()
            };
            list(&parts[1])
                .iter()
                .map(|m| m.dumps_ascii_tight().expect("JSON"))
                .collect()
        };
        let mut observed: Vec<(usize, usize)> = Vec::new();
        for node in nodes {
            let steps = steps_of(dict(node).get("path").unwrap());
            if steps.len() == path.len() + 1
                && steps[..path.len()] == path[..]
                && steps[path.len()].0 == "set_item"
            {
                let at = steps[path.len()].1.unwrap();
                let shown = dict(node)
                    .get("value")
                    .unwrap()
                    .dumps_ascii_tight()
                    .unwrap();
                let member = members.iter().position(|m| *m == shown).expect("a member");
                observed.push((at, member));
            }
        }
        observed.sort();
        let order = PyValue::List(observed.iter().map(|&(_, m)| PyValue::int(m)).collect());
        let root = if path[0].0 == "args" {
            &mut *args
        } else {
            &mut *kwargs
        };
        let mut node = root;
        for (kind, at) in &path[1..] {
            node = step_into(node, kind, at.unwrap());
        }
        let PyValue::List(parts) = node else {
            unreachable!()
        };
        parts.push(order);
    }
}

struct Saved {
    id: String,
    function: String,
    args: PyValue,
    kwargs: PyValue,
    original: PyValue,
}

fn saved(number: usize) -> Saved {
    let dir = reference().join(format!("api-{number:03}"));
    let request = read(dir.join("request.json"));
    let original = read(dir.join("original.json"));
    let request = dict(&request);
    let mut args = request.get("args_dto").expect("args").clone();
    let mut kwargs = request.get("kwargs_dto").expect("kwargs").clone();
    with_observed_set_order(
        &mut args,
        &mut kwargs,
        list(dict(&original).get("input_nodes").expect("nodes")),
    );
    Saved {
        id: text_of(request.get("id").unwrap()),
        function: text_of(request.get("function").unwrap()),
        args,
        kwargs,
        original,
    }
}

fn request_line(case: &Saved) -> PyStr {
    let mut row = PyDict::new();
    row.set("id", PyValue::str(&case.id));
    row.set(
        "operation",
        PyValue::str(&format!("resolve_torch_pin.{}", case.function)),
    );
    row.set("args_dto", case.args.clone());
    row.set("kwargs_dto", case.kwargs.clone());
    PyValue::Dict(row).dumps_unicode().expect("a request line")
}

fn differences(case: &Saved, got: &PyValue) -> Vec<String> {
    let original = dict(&case.original);
    let got = dict(got);
    let mut problems = Vec::new();
    let mut check = |what: &str, want: &PyValue, have: Option<&PyValue>| match have {
        Some(have) if same_value(want, have) => {}
        Some(have) => problems.push(format!(
            "{}: {what}: want {} got {}",
            case.id,
            want.repr().chars().take(200).collect::<String>(),
            have.repr().chars().take(200).collect::<String>()
        )),
        None => problems.push(format!("{}: {what}: missing", case.id)),
    };
    check("id", &PyValue::str(&case.id), got.get("id"));
    check("kind", original.get("kind").unwrap(), got.get("kind"));
    if text_of(original.get("kind").unwrap()) == "value" {
        check("value", original.get("value").unwrap(), got.get("value"));
        let aliases = list(original.get("return_aliases").unwrap());
        assert!(
            aliases.len() <= 1,
            "{}: more than one alias is not modelled",
            case.id
        );
        let want = aliases.first().cloned().unwrap_or(PyValue::None);
        check("input_ref", &want, got.get("input_ref"));
    } else {
        for key in ["type", "module", "message", "exception_chain"] {
            check(key, original.get(key).unwrap(), got.get(key));
        }
    }
    check("stdout", original.get("stdout").unwrap(), got.get("stdout"));
    check("stderr", original.get("stderr").unwrap(), got.get("stderr"));
    let after = dict(original.get("after").unwrap());
    check(
        "args_after",
        after.get("args").unwrap(),
        got.get("args_after"),
    );
    check(
        "kwargs_after",
        after.get("kwargs").unwrap(),
        got.get("kwargs_after"),
    );
    problems
}

fn all_or_fail(problems: Vec<String>) {
    assert!(
        problems.is_empty(),
        "{} differences, first ones:\n{}",
        problems.len(),
        problems
            .iter()
            .take(12)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

mod saved_cases {
    use super::*;

    #[test]
    fn the_220_saved_original_results_agree_through_the_public_service() {
        let mut problems = Vec::new();
        for number in 1..=220 {
            let case = saved(number);
            let got = answer(&request_line(&case));
            problems.extend(differences(&case, &got));
        }
        all_or_fail(problems);
    }

    #[test]
    fn the_saved_corpus_has_the_scope_that_was_frozen() {
        let (mut norm, mut pick) = (0, 0);
        for number in 1..=220 {
            match saved(number).function.as_str() {
                "norm_prefix" => norm += 1,
                "pick_bound" => pick += 1,
                other => panic!("{other}"),
            }
        }
        assert_eq!((norm, pick), (102, 118));
    }
}

mod source_contract {
    use super::*;

    fn text(value: &str) -> PyValue {
        PyValue::str(value)
    }

    fn norm(value: PyValue) -> Result<PyValue, String> {
        norm_prefix(&[value], &PyDict::new())
            .map(|returned| returned.value)
            .map_err(|raised| raised.rows[0].message.to_utf8().unwrap_or_default())
    }

    fn bound(items: &[&str], newest: bool) -> String {
        let versions = PyValue::List(items.iter().map(|item| text(item)).collect());
        match pick_bound(&[versions, PyValue::Bool(newest)], &PyDict::new()) {
            Ok(returned) => match returned.value {
                PyValue::Str(shown) => shown.to_utf8().unwrap_or_default(),
                other => other.repr(),
            },
            Err(raised) => format!(
                "raised {}",
                raised.rows[0].message.to_utf8().unwrap_or_default()
            ),
        }
    }

    #[test]
    fn the_whole_release_is_kept_without_padding_truncation_or_leading_zeros() {
        for (given, want) in [
            ("1", "1"),
            ("1.0", "1.0"),
            ("1.0.0.0", "1.0.0.0"),
            ("007.010.0", "7.10.0"),
            ("2.11.0a0+a6c236b", "2.11.0"),
            ("v1!2.3.4.5.6.7.8.9", "2.3.4.5.6.7.8.9"),
        ] {
            assert!(
                same_value(&norm(text(given)).unwrap().clone(), &text(want)),
                "{given}"
            );
        }
    }

    #[test]
    fn only_invalid_version_gives_the_original_back_and_everything_else_propagates() {
        assert!(same_value(
            &norm(text("not a version")).unwrap(),
            &text("not a version")
        ));
        // ASCII digits only: an Arabic-Indic digit is not a digit of the pattern
        assert!(same_value(
            &norm(text("\u{663}.1")).unwrap(),
            &text("\u{663}.1")
        ));
        // a non-text is the TypeError of the pattern's `search`
        assert_eq!(
            norm(PyValue::int(3)).unwrap_err(),
            "expected string or bytes-like object, got 'int'"
        );
        assert_eq!(
            norm(PyValue::Bytes(b"1.0".to_vec())).unwrap_err(),
            "cannot use a string pattern on a bytes-like object"
        );
    }

    #[test]
    fn re_ignorecase_letters_and_unicode_space_are_the_patterns_not_ascii_lowercase() {
        // `ſ` matches the `s` of `post` and `İ` the `i` of `preview`; neither is rewritten to the ordinary label
        assert!(same_value(
            &norm(text("1.2.po\u{17f}t3")).unwrap(),
            &text("1.2")
        ));
        assert!(same_value(
            &norm(text("1.2prev\u{130}ew4")).unwrap(),
            &text("1.2")
        ));
        assert!(same_value(
            &norm(text("1.2+k\u{212a}\u{131}")).unwrap(),
            &text("1.2")
        ));
        // an unusual pre label is compared as text: `previ\u{307}ew` < `rc`, so this is below 1.0rc1 and above 1.0a1
        assert_eq!(
            bound(&["1.0rc1", "1.0prev\u{130}ew1", "1.0a1"], false),
            "1.0a1"
        );
        assert_eq!(
            bound(&["1.0rc1", "1.0prev\u{130}ew1", "1.0a1"], true),
            "1.0rc1"
        );
        assert_eq!(
            bound(&["1.0rc1", "1.0prev\u{130}ew1"], false),
            "1.0prev\u{130}ew1"
        );
        // a line feed is whitespace and anchors; NUL and a lone surrogate do not match
        assert!(same_value(
            &norm(text(" \u{85}1.5\n")).unwrap(),
            &text("1.5")
        ));
        assert!(same_value(
            &norm(text("1.5\u{0}")).unwrap(),
            &text("1.5\u{0}")
        ));
    }

    #[test]
    fn the_sort_is_the_stable_pep_440_order_and_the_raw_element_is_returned() {
        let ascending = [
            "1.0.dev1",
            "1.0a1",
            "1.0a2.dev1",
            "1.0a2",
            "1.0b1",
            "1.0rc1",
            "1.0",
            "1.0+abc",
            "1.0+abc.5",
            "1.0+5",
            "1.0.post1.dev1",
            "1.0.post1",
            "1!0.1",
        ];
        let mut shuffled = ascending.to_vec();
        shuffled.reverse();
        assert_eq!(bound(&shuffled, true), "1!0.1");
        assert_eq!(bound(&shuffled, false), "1.0.dev1");
        // equal versions keep the input order: the oldest is the first, the newest the LAST of the equal ones, and the raw spelling comes back
        assert_eq!(bound(&["1.0", "1.0.0", "01.00"], false), "1.0");
        assert_eq!(bound(&["1.0", "1.0.0", "01.00"], true), "01.00");
        // the first element that is not a version ends the sort and `versions[0]` comes back, whatever the others are
        assert_eq!(bound(&["2.0", "bad", "1.0"], true), "2.0");
        // a failing integer conversion is not InvalidVersion: no fallback
        let long = "9".repeat(4301);
        assert_eq!(bound(&["bad", &format!("1.{long}")], true), "bad");
        assert!(bound(&[&format!("1.{long}"), "bad"], true)
            .starts_with("raised Exceeds the limit (4300 digits)"));
    }
}

mod fixture_and_program {
    use super::*;
    use std::io::Write;

    /// The saved results of api-001..044 as one norm_prefix row and api-220 as the pick_bound row, the way the Root controller builds them.
    fn fixture_rows() -> PyValue {
        let call = |number: usize| {
            let case = saved(number);
            let original = dict(&case.original);
            let aliases = list(original.get("return_aliases").unwrap());
            let mut expected = PyDict::new();
            expected.set("value", original.get("value").unwrap().clone());
            expected.set(
                "input_ref",
                aliases.first().cloned().unwrap_or(PyValue::None),
            );
            let mut row = PyDict::new();
            row.set("args_dto", case.args);
            row.set("kwargs_dto", case.kwargs);
            row.set("expected", PyValue::Dict(expected));
            PyValue::Dict(row)
        };
        let row = |metric: &str, operation: &str, calls: Vec<PyValue>, iterations: usize| {
            let mut row = PyDict::new();
            row.set("metric", PyValue::str(metric));
            row.set("operation", PyValue::str(operation));
            row.set("calls", PyValue::List(calls));
            row.set("iterations", PyValue::int(iterations));
            PyValue::Dict(row)
        };
        PyValue::List(vec![
            row(
                "norm_prefix_batch44",
                "resolve_torch_pin.norm_prefix",
                (1..=44).map(call).collect(),
                20,
            ),
            row(
                "pick_bound_256",
                "resolve_torch_pin.pick_bound",
                vec![call(220)],
                5,
            ),
        ])
    }

    fn fixture() -> String {
        fixture_rows().dumps_unicode().unwrap().to_utf8().unwrap()
    }

    /// The fixture text after `change` was applied to call `call_at` of row `row_at`, with that row's iterations made absurd (10^12): if any timer ran
    /// before the row was refused, the check below would not return.
    fn fixture_changed(row_at: usize, call_at: usize, change: impl FnOnce(&mut PyDict)) -> String {
        let mut rows = fixture_rows();
        let PyValue::List(listed) = &mut rows else {
            unreachable!()
        };
        let PyValue::Dict(row) = &mut listed[row_at] else {
            unreachable!()
        };
        row.set("iterations", PyValue::int(1_000_000_000_000u64));
        let Some(PyValue::List(calls)) = row.get_mut("calls") else {
            unreachable!()
        };
        let PyValue::Dict(call) = &mut calls[call_at] else {
            unreachable!()
        };
        change(call);
        rows.dumps_unicode().unwrap().to_utf8().unwrap()
    }

    fn change_expected(call: &mut PyDict, member: &str, to: PyValue) {
        let Some(PyValue::Dict(expected)) = call.get_mut("expected") else {
            unreachable!()
        };
        expected.set(member, to);
    }

    fn refusal(text: &str) -> String {
        let rows = upstream_runtime::version_bench::load(text).expect("the rows decode");
        upstream_runtime::version_bench::run_rows(&rows, 11, 200)
            .expect_err("the row must be refused before any timer")
            .line()
            .to_utf8()
            .unwrap()
    }

    #[test]
    fn the_frozen_fixture_decodes_once_into_45_calls_and_9735_timed_helper_calls() {
        let rows = upstream_runtime::version_bench::load(&fixture()).expect("the rows decode");
        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows.iter()
                .map(|row| row.metric.as_str())
                .collect::<Vec<_>>(),
            ["norm_prefix_batch44", "pick_bound_256"]
        );
        let rounds = 11;
        // source-derived counts: one preflight call per fixture call, and calls x iterations x rounds timed helper calls
        let preflight: usize = rows.iter().map(|row| row.calls()).sum();
        let timed: usize = rows
            .iter()
            .map(|row| row.calls() * row.iterations_or(200) * rounds)
            .sum();
        assert_eq!((preflight, timed), (45, 44 * 20 * 11 + 5 * 11));
        assert_eq!(timed, 9735);
    }

    #[test]
    fn the_preflight_accepts_every_saved_original_value_and_origin_once_each() {
        let rows = upstream_runtime::version_bench::load(&fixture()).expect("the rows decode");
        upstream_runtime::version_bench::preflight(&rows).expect("the saved results agree");
    }

    #[test]
    fn the_roots_pinned_fixture_decodes_and_preflights_without_a_timer() {
        // the file Root built only from the saved originals (`R35_BENCH_FIXTURE`); decoding and the 45 preflight calls, no measuring loop
        let path = std::env::var("R35_BENCH_FIXTURE")
            .expect("R35_BENCH_FIXTURE must name the pinned benchmark fixture");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
        let rows = upstream_runtime::version_bench::load(&text).expect("the rows decode");
        assert_eq!(
            rows.iter()
                .map(|row| (row.metric.as_str(), row.calls(), row.iterations_or(0)))
                .collect::<Vec<_>>(),
            [("norm_prefix_batch44", 44, 20), ("pick_bound_256", 1, 5)]
        );
        upstream_runtime::version_bench::preflight(&rows).expect("the saved results agree");
    }

    #[test]
    fn a_wrong_value_a_wrong_origin_or_a_raising_call_is_refused_before_any_timer() {
        // a different expected value (norm_prefix call 3 of 44)
        let text = fixture_changed(0, 3, |call| {
            change_expected(
                call,
                "value",
                PyValue::List(vec![PyValue::str("str"), PyValue::str("wrong")]),
            );
        });
        let line = refusal(&text);
        assert!(
            line.contains("norm_prefix_batch44 call 3: the value differs"),
            "{line}"
        );
        // a different expected origin: a null one made a path, and a path made null (the pick_bound row)
        let path = PyValue::List(vec![PyValue::List(vec![PyValue::str("args")])]);
        let text = fixture_changed(0, 0, |call| change_expected(call, "input_ref", path));
        let line = refusal(&text);
        assert!(
            line.contains("norm_prefix_batch44 call 0: the origin differs"),
            "{line}"
        );
        let text = fixture_changed(1, 0, |call| {
            change_expected(call, "input_ref", PyValue::None)
        });
        let line = refusal(&text);
        assert!(
            line.contains("pick_bound_256 call 0: the origin differs"),
            "{line}"
        );
        // a call that raises (no argument for `v`) is a refusal too, not a value
        let text = fixture_changed(0, 5, |call| {
            call.set(
                "args_dto",
                PyValue::List(vec![PyValue::str("list"), PyValue::List(vec![])]),
            );
        });
        let line = refusal(&text);
        assert!(line.contains("call 5 fails before measurement"), "{line}");
    }

    #[test]
    fn the_timed_region_runs_the_raw_helpers_only_with_no_projection_comparison_or_second_preflight(
    ) {
        let source = include_str!("../src/version_bench.rs");
        let begin_marker = "            // TIMED-PATH-BEGIN\n";
        let end_marker = "            // TIMED-PATH-END\n";
        assert_eq!(source.matches(begin_marker).count(), 1);
        assert_eq!(source.matches(end_marker).count(), 1);
        let begin = source.find(begin_marker).unwrap() + begin_marker.len();
        let end = source.find(end_marker).unwrap();
        let timed = &source[begin..end];
        // what is timed: the business function on every call, the raw returns collected and dropped
        for needed in ["row.function.call(", "Vec<Returned>", "black_box(results)"] {
            assert!(timed.contains(needed), "the timed region lost {needed}");
        }
        // what is not: encoding to a DTO, the origin path as a value, response dictionaries, JSON, expected values, comparisons, a preflight
        for forbidden in [
            "typed_result",
            "encode",
            "to_value",
            "PyDict",
            "PyValue",
            "dumps",
            "loads",
            "expected",
            "same_value",
            "preflight",
            "format!",
            "repr(",
        ] {
            assert!(
                !timed.contains(forbidden),
                "the timed region contains {forbidden}"
            );
        }
        // the measuring function runs no preflight of its own; the single one is in `run_rows`, before it
        let measure =
            &source[source.find("fn measure(").unwrap()..source.find("pub fn run_rows(").unwrap()];
        assert!(!measure.contains("preflight("));
        let run_rows = &source[source.find("pub fn run_rows(").unwrap()..];
        assert!(
            run_rows.find("preflight(rows)?").unwrap() < run_rows.find("measure(rows").unwrap()
        );
        assert_eq!(source.matches("preflight(rows)?").count(), 1);
    }

    #[test]
    fn the_normal_bench_program_reports_a_tiny_run_and_refuses_a_wrong_row_without_a_report() {
        // TEST-INTERNAL benchmark children (not official): exactly one measuring process (one round, one iteration of one call) and one refused row
        // process that never reaches a timer.
        let dir = PathBuf::from(
            std::env::var("TMPDIR").expect("TMPDIR must name the private temporary directory"),
        )
        .join(format!("version_checks_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let tiny = |wrong: bool| {
            let mut rows = fixture_rows();
            let PyValue::List(listed) = &mut rows else {
                unreachable!()
            };
            listed.truncate(1);
            let PyValue::Dict(row) = &mut listed[0] else {
                unreachable!()
            };
            row.set("iterations", PyValue::int(1));
            let Some(PyValue::List(calls)) = row.get_mut("calls") else {
                unreachable!()
            };
            calls.truncate(1);
            if wrong {
                let PyValue::Dict(call) = &mut calls[0] else {
                    unreachable!()
                };
                change_expected(call, "value", PyValue::None);
            }
            rows.dumps_unicode().unwrap().to_utf8().unwrap()
        };
        let run = |name: &str, text: String| {
            let path = dir.join(name);
            std::fs::write(&path, text).unwrap();
            std::process::Command::new(env!("CARGO_BIN_EXE_upstream_version_bench"))
                .args(["--fixture", path.to_str().unwrap(), "--rounds", "1"])
                .output()
                .expect("the program runs")
        };
        let good = run("good.json", tiny(false));
        assert!(
            good.status.success(),
            "{}",
            String::from_utf8_lossy(&good.stderr)
        );
        let report = String::from_utf8(good.stdout).unwrap();
        let lines: Vec<&str> = report.lines().collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "{\"rounds\": 1, \"iterations\": 200}");
        assert!(lines[1].contains("\"metric\": \"norm_prefix_batch44\", \"unit\": \"ns_per_call\""));
        assert!(lines[1].ends_with("\"round_count\": 1, \"iterations\": 1}"));
        let bad = run("bad.json", tiny(true));
        assert_eq!(bad.status.code(), Some(1));
        assert!(bad.stdout.is_empty());
        assert!(String::from_utf8_lossy(&bad.stderr)
            .contains("the value differs from the saved original result"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_normal_program_answers_jsonl_on_standard_input() {
        let case = saved(45);
        let mut line = request_line(&case).to_utf8().unwrap();
        line.push('\n');
        let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_upstream_version_api"))
            .arg("evaluate-jsonl")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("the program starts");
        child
            .stdin
            .take()
            .unwrap()
            .write_all(line.as_bytes())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success());
        let record = loads(&String::from_utf8(output.stdout).unwrap()).unwrap();
        assert!(differences(&case, &record).is_empty());
        assert!(output.stderr.is_empty());
    }

    #[test]
    fn unknown_members_operations_and_modules_are_refused_with_request_errors() {
        let refused = |line: &str| {
            let row = answer(&PyStr::new(line));
            let row = dict(&row);
            (
                text_of(row.get("kind").unwrap()),
                row.get("type").map(text_of),
            )
        };
        assert_eq!(
            refused(
                "{\"id\": 1, \"operation\": \"resolve_torch_pin.norm_prefix\", \"globals\": {}}"
            ),
            ("exception".to_string(), Some("RequestError".to_string()))
        );
        assert_eq!(
            refused("{\"id\": 1, \"operation\": \"resolve_torch_pin.main\"}"),
            ("exception".to_string(), Some("AttributeError".to_string()))
        );
        assert_eq!(
            refused("{\"id\": 1, \"operation\": \"other.norm_prefix\"}"),
            (
                "exception".to_string(),
                Some("ModuleNotFoundError".to_string())
            )
        );
    }
}
