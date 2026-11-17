//! Pure-gate checks of `manifest_gate` over values built by the common YAML loader: operator semantics of the owner on values it
//! does not control, the result key order, the uncaught-exception shapes (as data), and the branch helpers with a stub probe.
//! The byte-level replay of the actual ORIGINAL190 capture lives in the B program checks (`manifest_contract_cli`).

use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::PyValue;
use agent_runtime_support::pyyaml::{safe_load, safe_load_objects};
use role_contract::manifest_gate::{
    branch_topology_conflict, evaluate_contract, manifest_tp_value, resolve_topology, BranchProbe,
    Evaluation, GateFailure, CALL_MANIFEST_TP, ENUMERATE_NODES, GENEXPR_RSTRIP_ATTR, MAN_GET,
    NEXT_CALL, TP_INT, TP_LEN,
};

const OK: &str = "terraforming: {complete: true, branch_verified: true}\ntopology: single\ngpus_per_node: 1\nmodel_source: managed\nnodes: []\n";

fn load(yaml: &str) -> PyValue {
    safe_load(yaml.as_bytes(), &PyStr::new("fixture.yaml")).expect("fixture YAML")
}

fn eval(yaml: &str, topology: &str) -> Result<Evaluation, GateFailure> {
    evaluate_contract(&load(yaml), topology)
}

fn answered(yaml: &str, topology: &str) -> Evaluation {
    eval(yaml, topology).unwrap_or_else(|failure| panic!("an answer, got {failure:?}"))
}

fn reason(evaluation: &Evaluation) -> String {
    evaluation.reason().to_utf8().expect("UTF-8 reason")
}

fn raised(yaml: &str, topology: &str) -> role_contract::manifest_gate::Raised {
    match eval(yaml, topology) {
        Err(GateFailure::Raised(raised)) => raised,
        other => panic!("an unhandled exception, got {other:?}"),
    }
}

#[test]
fn a_valid_manifest_answers_in_the_owners_key_order() {
    let evaluation = answered(OK, "single");
    assert!(evaluation.flag());
    assert_eq!(evaluation.exit_code(), 0);
    let keys: Vec<String> = evaluation
        .res
        .iter()
        .map(|(key, _)| key.to_utf8().expect("key"))
        .collect();
    assert_eq!(
        keys,
        [
            "flag",
            "reason",
            "exit_code",
            "topology",
            "gpus_per_node",
            "nodes",
            "model_source",
            "nas_model_path",
            "custom_model_paths",
            "quant_model_path",
            "tiktoken_host_path",
            "manifest_tp",
            "node_warnings",
            "effective_model_source"
        ]
    );
}

#[test]
fn the_flag_is_identity_true_not_truthiness() {
    for complete in ["1", "\"true\"", "false"] {
        let yaml = OK.replace("complete: true", &format!("complete: {complete}"));
        let evaluation = answered(&yaml, "single");
        assert_eq!(evaluation.exit_code(), 4, "complete: {complete}");
        assert_eq!(
            reason(&evaluation),
            "terraforming.complete != true (Flag 미발급 — info-only)"
        );
    }
    let sub = format!("self_role: sub\n{OK}");
    assert_eq!(answered(&sub, "single").exit_code(), 4);
    let issued = sub.replace("complete: true,", "complete: true, issued_by: main,");
    assert!(answered(&issued, "single").flag());
}

#[test]
fn missing_fields_are_judged_by_truthiness_before_the_tp_error() {
    let evaluation = answered(
        &OK.replace("gpus_per_node: 1", "gpus_per_node: 0"),
        "single",
    );
    assert_eq!(evaluation.exit_code(), 5);
    assert_eq!(
        reason(&evaluation),
        "Flag true 이나 필수 HW필드 누락: gpus_per_node"
    );
    let evaluation = answered(
        &OK.replace("gpus_per_node: 1", "gpus_per_node: \"0\""),
        "single",
    );
    assert_eq!(evaluation.exit_code(), 5);
    assert_eq!(
        reason(&evaluation),
        "Flag true 이나 gpus_per_node 부재/비정수('0') — TP 는 HW 사실이지 기본값이 아니다"
    );
    let mismatch = answered(&OK.replace("topology: single", "topology: multi"), "single");
    assert_eq!(mismatch.exit_code(), 6);
}

#[test]
fn gpus_per_node_coercion_follows_int_with_only_type_and_value_errors_caught() {
    for (raw, tp) in [
        ("\"2\"", "2"),
        ("2.9", "2"),
        ("true", "1"),
        ("\"1_0\"", "10"),
    ] {
        let evaluation = answered(
            &OK.replace("gpus_per_node: 1", &format!("gpus_per_node: {raw}")),
            "single",
        );
        assert!(evaluation.flag(), "gpus_per_node: {raw}");
        assert_eq!(
            evaluation
                .res
                .get("manifest_tp")
                .map(PyValue::repr)
                .as_deref(),
            Some(tp)
        );
    }
    let failure = raised(
        &OK.replace("gpus_per_node: 1", "gpus_per_node: .inf"),
        "single",
    );
    assert_eq!(failure.class, "OverflowError");
    assert_eq!(
        failure.message.to_utf8().unwrap(),
        "cannot convert float infinity to integer"
    );
    assert_eq!(failure.frames, vec![CALL_MANIFEST_TP, TP_INT]);
    assert_eq!(
        answered(
            &OK.replace("gpus_per_node: 1", "gpus_per_node: .nan"),
            "single"
        )
        .exit_code(),
        5
    );
}

#[test]
fn tp_multiplies_the_node_count_only_outside_single() {
    let two = OK
        .replace("topology: single", "topology: multi")
        .replace("nodes: []", "nodes: [{role: main}, {role: sub}]");
    let multi = answered(
        &two.replace("gpus_per_node: 1", "gpus_per_node: 2"),
        "multi",
    );
    assert_eq!(
        multi.res.get("manifest_tp").map(PyValue::repr).as_deref(),
        Some("4")
    );
    let single = answered(
        &OK.replace("nodes: []", "nodes: [{role: main}, {role: sub}]"),
        "single",
    );
    assert_eq!(
        single.res.get("manifest_tp").map(PyValue::repr).as_deref(),
        Some("1")
    );
    let empty = answered(&OK.replace("topology: single", "topology: multi"), "multi");
    assert_eq!(
        empty.res.get("manifest_tp").map(PyValue::repr).as_deref(),
        Some("None")
    );
    let failure = raised(
        &OK.replace("topology: single", "topology: multi")
            .replace("nodes: []", "nodes: 5"),
        "multi",
    );
    assert_eq!(failure.class, "TypeError");
    assert_eq!(
        failure.message.to_utf8().unwrap(),
        "object of type 'int' has no len()"
    );
    assert_eq!(failure.frames, vec![CALL_MANIFEST_TP, TP_LEN]);
    let man = match load(OK).peel() {
        PyValue::Dict(map) => map.clone(),
        _ => unreachable!(),
    };
    assert!(matches!(manifest_tp_value(&man, "single"), Ok(Ok(_))));
}

#[test]
fn values_that_are_not_mappings_end_in_the_owners_attribute_errors() {
    let failure = match evaluate_contract(&load("[a, b]"), "single") {
        Err(GateFailure::Raised(raised)) => raised,
        other => panic!("{other:?}"),
    };
    assert_eq!(failure.class, "AttributeError");
    assert_eq!(
        failure.message.to_utf8().unwrap(),
        "'list' object has no attribute 'get'"
    );
    assert_eq!(failure.frames, vec![MAN_GET]);
    let failure = raised(
        &OK.replace(
            "terraforming: {complete: true, branch_verified: true}",
            "terraforming: text",
        ),
        "single",
    );
    assert_eq!(
        failure.message.to_utf8().unwrap(),
        "'str' object has no attribute 'get'"
    );
    let failure = raised(&OK.replace("nodes: []", "nodes: 5"), "single");
    assert_eq!(
        failure.message.to_utf8().unwrap(),
        "'int' object is not iterable"
    );
    assert_eq!(failure.frames, vec![ENUMERATE_NODES]);
}

#[test]
fn per_node_model_source_errors_warnings_and_keys() {
    let bad = OK.replace(
        "nodes: []",
        "nodes: [{role: main, model_source: 123}, {role: sub, model_source: bad}]",
    );
    let evaluation = answered(&bad, "single");
    assert_eq!(evaluation.exit_code(), 5);
    assert_eq!(
        reason(&evaluation),
        "per-node model_source 오류: nodes[0](main).model_source=123; nodes[1](sub).model_source='bad' (기대 managed|ephemeral|custom)"
    );
    let multi = OK.replace("topology: single", "topology: multi").replace(
        "nodes: []",
        "nodes: [{role: main}, {role: sub, model_source: managed}]",
    );
    let evaluation = answered(&multi, "multi");
    assert!(evaluation.flag());
    assert_eq!(
        evaluation.res.get("node_warnings").map(PyValue::repr).as_deref(),
        Some("['nodes[1](sub).model_source override 는 single-node sub-control 한정 — multi 는 무시(단일 정책)']")
    );
    let unnamed = answered(
        &OK.replace(
            "nodes: []",
            "nodes: [{model_source: custom}, {role: sub}, 7]",
        ),
        "single",
    );
    assert_eq!(
        unnamed
            .res
            .get("effective_model_source")
            .map(PyValue::repr)
            .as_deref(),
        Some("{'node0': 'custom', 'sub': 'managed'}")
    );
}

#[test]
fn the_tiktoken_warning_and_the_generator_exceptions() {
    let nodes =
        "nodes: [{role: main, work_dir: /fixture/proj}, {role: sub, work_dir: /fixture/proj}]";
    let warned = answered(
        &format!("{OK}tiktoken_host_path: /fixture/proj/tk\n").replace("nodes: []", nodes),
        "single",
    );
    assert_eq!(
        warned
            .res
            .get("node_warnings")
            .map(|value| value.repr().contains("노드 작업경로(/fixture/proj)")),
        Some(true)
    );
    let outside = answered(
        &format!("{OK}tiktoken_host_path: /mnt/shared/tk\n").replace("nodes: []", nodes),
        "single",
    );
    assert_eq!(
        outside
            .res
            .get("node_warnings")
            .map(PyValue::repr)
            .as_deref(),
        Some("[]")
    );
    let failure = raised(
        &format!("{OK}tiktoken_host_path: /fixture/proj/tk\n").replace(
            "nodes: []",
            "nodes: [{role: main, work_dir: 5}, {role: sub}]",
        ),
        "single",
    );
    assert_eq!(
        failure.message.to_utf8().unwrap(),
        "'int' object has no attribute 'rstrip'"
    );
    assert_eq!(failure.frames, vec![NEXT_CALL, GENEXPR_RSTRIP_ATTR]);
}

struct Lane(Option<&'static str>);

impl BranchProbe for Lane {
    fn branch(&mut self, _repo: &[u8]) -> Option<PyStr> {
        self.0.map(PyStr::new)
    }
}

#[test]
fn branch_helpers_follow_the_owner() {
    assert_eq!(
        resolve_topology(&mut Lane(Some("single-node")), b"/r", None).as_deref(),
        Some("single")
    );
    assert_eq!(
        resolve_topology(&mut Lane(Some("multi-node")), b"/r", None).as_deref(),
        Some("multi")
    );
    assert_eq!(resolve_topology(&mut Lane(Some("HEAD")), b"/r", None), None);
    assert_eq!(resolve_topology(&mut Lane(None), b"/r", None), None);
    assert_eq!(
        resolve_topology(&mut Lane(Some("single-node")), b"/r", Some("multi")).as_deref(),
        Some("multi")
    );
    let conflict = branch_topology_conflict(&mut Lane(Some("single-node")), b"/r", Some("multi"))
        .expect("conflict");
    assert_eq!(
        conflict.to_utf8().unwrap(),
        "요청 topology='multi' 이 체크아웃 브랜치가 고르는 통로 'single' 와 다르다 — 브랜치를 바꾸거나 요청을 바꿔라(자동 교정하지 않는다). 정본 술어: topology_parity.sh"
    );
    assert!(
        branch_topology_conflict(&mut Lane(Some("single-node")), b"/r", Some("single")).is_none()
    );
    assert!(
        branch_topology_conflict(&mut Lane(Some("fixture-other")), b"/r", Some("multi")).is_none()
    );
    assert!(branch_topology_conflict(&mut Lane(Some("single-node")), b"/r", None).is_none());
}

fn effective_keys(nodes: &str) -> String {
    let evaluation = answered(
        &OK.replace("nodes: []", &format!("nodes: {nodes}")),
        "single",
    );
    evaluation
        .res
        .get("effective_model_source")
        .map(PyValue::repr)
        .expect("effective_model_source")
}

#[test]
fn role_keys_follow_python_dictionary_semantics() {
    // 1, True and 1.0 are one key (the first key object and position, the last value); "1" is another key.
    assert_eq!(
        effective_keys(
            "[{role: 1}, {role: true, model_source: custom}, {role: 2.5}, {role: \"1\"}]"
        ),
        "{1: 'custom', 2.5: 'managed', '1': 'managed'}"
    );
    // A falsy role falls back to node<i> (the index counts every node).
    assert_eq!(
        effective_keys("[{role: null}, {role: false}, {role: 0}, {role: \"\"}, 7, {role: true}]"),
        "{'node0': 'managed', 'node1': 'managed', 'node2': 'managed', 'node3': 'managed', True: 'managed'}"
    );
    // A date is a valid key (only json.dumps refuses it later).
    assert!(effective_keys("[{role: 2026-11-08}]").starts_with("{datetime.date(2026, 11, 8): "));
}

#[test]
fn an_unhashable_role_is_the_owners_type_error() {
    for (role, name) in [("[a, b]", "list"), ("{a: 1}", "dict")] {
        let failure = raised(
            &OK.replace("nodes: []", &format!("nodes: [{{role: {role}}}]")),
            "single",
        );
        assert_eq!(failure.class, "TypeError");
        assert_eq!(
            failure.message.to_utf8().unwrap(),
            format!("unhashable type: '{name}'")
        );
        assert_eq!(
            failure.frames,
            vec![role_contract::manifest_gate::EFFECTIVE_KEY]
        );
    }
}

#[test]
fn str_of_a_date_or_timestamp_role_is_not_its_repr() {
    let date = OK.replace(
        "nodes: []",
        "nodes: [{role: 2026-11-08, model_source: bad}]",
    );
    assert_eq!(
        reason(&answered(&date, "single")),
        "per-node model_source 오류: nodes[0](2026-11-08).model_source='bad' (기대 managed|ephemeral|custom)"
    );
    let stamp = OK.replace(
        "nodes: []",
        "nodes: [{role: 2026-11-08 12:30:00, model_source: bad}]",
    );
    assert_eq!(
        reason(&answered(&stamp, "single")),
        "per-node model_source 오류: nodes[0](2026-11-08 12:30:00).model_source='bad' (기대 managed|ephemeral|custom)"
    );
}

#[test]
fn nan_role_keys_follow_object_identity() {
    // `.nan` is one shared object, an aliased `!!float nan` is one object, a separate `!!float nan` is another: three keys
    // (captured Y07: values custom, ephemeral, managed).
    let yaml = OK.replace(
        "nodes: []",
        "nodes: [{role: .nan}, {role: .nan, model_source: custom}, {role: &n !!float nan}, {role: *n, model_source: ephemeral}, {role: !!float nan}]",
    );
    let man =
        safe_load_objects(yaml.as_bytes(), &PyStr::new("fixture.yaml")).expect("fixture YAML");
    let evaluation = evaluate_contract(&man, "single").expect("an answer");
    assert_eq!(
        evaluation
            .res
            .get("effective_model_source")
            .map(PyValue::repr)
            .as_deref(),
        Some("{nan: 'custom', nan: 'ephemeral', nan: 'managed'}")
    );
}
