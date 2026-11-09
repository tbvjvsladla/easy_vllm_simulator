//! Shared identity and exception attributes at the bridge, on constructed requests (the saved Original107 rows are covered by
//! `scan_node_p1_original_cases`; there is no saved row for the exceptions below, so these check the plumbing's own contract and
//! are not an original-behaviour oracle): a caller object handed back by the business is an `argref` to the same numbered object, a
//! NaN stays one object however many times the caller used it, and a failed attribute read carries the real `name` and the real `obj`
//! (the caller's own object when it is one).

use agent_runtime_support::pyjson::loads_text;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use sweep_stop::scan_node_boundary::{FixedClock, StdFiles};
use sweep_stop::scan_node_bridge::{serve, Environment};

fn ask(request: &str) -> PyValue {
    let clock = FixedClock {
        hour: "2026110910".into(),
        provenance: "a literal of the check",
    };
    let answer = serve(
        request.as_bytes(),
        &Environment::default(),
        &StdFiles,
        &clock,
    )
    .expect("served");
    loads_text(&PyStr::new(&answer)).expect("JSON")
}

fn member<'a>(value: &'a PyValue, key: &str) -> &'a PyValue {
    match value.peel() {
        PyValue::Dict(map) => map.get(key).unwrap_or_else(|| panic!("no {key}")),
        _ => panic!("not an object"),
    }
}

fn dumped(value: &PyValue) -> String {
    value.dumps_ascii().expect("dump")
}

const NAN0: &str = r#"{"t":"float","repr":"nan","id":0}"#;

fn gate(bandwidth: &str, per_port: &str) -> PyValue {
    ask(&format!(
        r#"{{"protocol":"scan-node-bridge/1","op":"evaluate_gate","params":{{
        "declared":{{"t":"str","v":"multi"}},"ic_present":{{"t":"bool","v":true}},"peer_given":{{"t":"bool","v":true}},
        "peer_reachable":{{"t":"bool","v":true}},"bandwidth":{bandwidth},"bw_floor":{{"t":"float","repr":"180.0"}},
        "per_port_gbps":{per_port},"per_port_floor":{{"t":"float","repr":"100.0"}},"branch":{{"t":"none"}},"branch_topo":{{"t":"str","v":"multi"}},
        "mani_topo":{{"t":"none"}},"mani_path":{{"t":"str","v":"manifest.yaml"}},"peer_ip":{{"t":"none"}},"egress_self":{{"t":"none"}},
        "egress_peer":{{"t":"none"}},"model_env_ok":{{"t":"none"}},"model_env_reason":{{"t":"none"}},"manifest_read_error":{{"t":"none"}}}}}}"#
    ))
}

#[test]
fn a_nan_the_caller_used_twice_is_one_object_and_comes_back_as_that_object() {
    // `bandwidth` is the numbered NaN 0; `per_port_gbps` is a repeat visit of the same object
    let answer = gate(NAN0, r#"{"t":"ref","id":0}"#);
    assert_eq!(member(&answer, "status").peel(), &PyValue::str("returned"));
    let value = member(&answer, "value");
    let assertion = &match member(value, "v").peel() {
        PyValue::List(parts) => parts[0].clone(),
        _ => panic!("tuple"),
    };
    let pairs = match member(assertion, "v").peel() {
        PyValue::List(pairs) => pairs.clone(),
        _ => panic!("pairs"),
    };
    let per_port = pairs
        .iter()
        .find_map(|pair| match pair.peel() {
            PyValue::List(kv) if dumped(&kv[0]).contains("per_port_gbps") => Some(kv[1].clone()),
            _ => None,
        })
        .expect("per_port_gbps");
    assert_eq!(dumped(&per_port), r#"{"t": "argref", "id": 0}"#);
}

#[test]
fn a_nan_that_is_not_returned_is_not_planned() {
    // `bandwidth` NaN, `per_port_gbps` None: nothing of the caller's comes back, so no argref appears anywhere in the answer
    let answer = gate(NAN0, r#"{"t":"none"}"#);
    assert!(!dumped(&answer).contains("argref"), "{}", dumped(&answer));
}

#[test]
fn a_returned_caller_container_is_an_argref_and_a_fresh_one_is_not() {
    let answer = ask(
        r#"{"protocol":"scan-node-bridge/1","op":"build_sub_manifest","params":{
        "result":{"t":"dict","id":0,"v":[]},"peer_hw":{"t":"dict","id":1,"v":[]},
        "main_manifest":{"t":"dict","id":2,"v":[[{"t":"str","v":"custom_model_paths"},{"t":"dict","id":3,"v":[[{"t":"str","v":"a"},{"t":"str","v":"/x"}]]}]]},
        "scanned_at":{"t":"str","v":"2026110910"},"egress_peer":{"t":"none"}}}"#,
    );
    let text = dumped(&answer);
    // the same dict object as `main_manifest["custom_model_paths"]` (id 3) is what the original returns under that key
    assert!(
        text.contains(r#"[{"t": "str", "v": "custom_model_paths"}, {"t": "argref", "id": 3}]"#),
        "{text}"
    );
    // `nodes` is built fresh by `[dict(n) for n in ...]`: not a caller object
    assert!(
        text.contains(r#"[{"t": "str", "v": "nodes"}, {"t": "list", "v": []}]"#),
        "{text}"
    );
}

#[test]
fn a_failed_attribute_read_carries_the_real_name_and_object() {
    // `peer.get(f)` on a caller's list: `obj` is that very list (numbered 1), `name` is `get`
    let answer = ask(
        r#"{"protocol":"scan-node-bridge/1","op":"assert_homogeneity","params":{
        "local":{"t":"dict","id":0,"v":[]},"peer":{"t":"list","id":1,"v":[{"t":"int","hex":"+1"}]}}}"#,
    );
    assert_eq!(member(&answer, "status").peel(), &PyValue::str("raised"));
    let exc = member(&answer, "exc");
    assert_eq!(member(exc, "cls").peel(), &PyValue::str("AttributeError"));
    assert_eq!(
        dumped(member(exc, "ctor")),
        r#"[{"t": "str", "v": "'list' object has no attribute 'get'"}]"#
    );
    assert_eq!(
        dumped(member(exc, "ctor_kwargs")),
        r#"{"name": {"t": "str", "v": "get"}, "obj": {"t": "argref", "id": 1}}"#
    );
}

#[test]
fn an_attribute_error_on_a_plain_value_carries_the_value() {
    let answer = ask(
        r#"{"protocol":"scan-node-bridge/1","op":"parse_gpu_memory_line","params":{"line":{"t":"int","hex":"+5"}}}"#,
    );
    let exc = member(&answer, "exc");
    assert_eq!(
        dumped(member(exc, "ctor_kwargs")),
        r#"{"name": {"t": "str", "v": "split"}, "obj": {"t": "int", "hex": "+5"}}"#
    );
}

#[test]
fn the_wire_refuses_what_it_cannot_honour() {
    for bad in [
        // an id on a singleton (bool): it carries no identity
        r#"{"protocol":"scan-node-bridge/1","op":"valid_host","params":{"value":{"t":"bool","v":true,"id":0}}}"#,
        // a ref to an object not defined before it (a cycle or a forward reference)
        r#"{"protocol":"scan-node-bridge/1","op":"valid_host","params":{"value":{"t":"ref","id":7}}}"#,
    ] {
        let clock = FixedClock {
            hour: String::new(),
            provenance: "unused",
        };
        assert!(
            serve(bad.as_bytes(), &Environment::default(), &StdFiles, &clock).is_err(),
            "{bad}"
        );
    }
    let _ = PyDict::new();
}

#[test]
fn a_numbered_scalar_is_the_callers_own_object_in_the_plan() {
    // `emission_blockers("text")`: the failed `.get` names the caller's own string (numbered 0) as `obj`
    let answer = ask(
        r#"{"protocol":"scan-node-bridge/1","op":"emission_blockers","params":{"result":{"t":"str","v":"text","id":0}}}"#,
    );
    let exc = member(&answer, "exc");
    assert_eq!(
        dumped(member(exc, "ctor_kwargs")),
        r#"{"name": {"t": "str", "v": "get"}, "obj": {"t": "argref", "id": 0}}"#
    );
    // and the scalar the business returns unchanged is the caller's object too
    let answer = ask(
        r#"{"protocol":"scan-node-bridge/1","op":"build_sub_manifest","params":{"result":{"t":"dict","id":0,"v":[]},
        "peer_hw":{"t":"dict","id":1,"v":[]},"main_manifest":{"t":"dict","id":2,"v":[]},
        "scanned_at":{"t":"str","v":"2026110910","id":3},"egress_peer":{"t":"none"}}}"#,
    );
    assert!(
        dumped(&answer).contains(r#"[{"t": "str", "v": "scanned_at"}, {"t": "argref", "id": 3}]"#),
        "{}",
        dumped(&answer)
    );
}
