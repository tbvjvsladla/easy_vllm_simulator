//! Python's argument binding as the numeric functions use it, checked where the cost of a call is decided: the supplied
//! arguments are the caller's own objects (Python binds the same object, it does not copy it), defaults are made for the call
//! and owned while the body runs, no call changes a nested input, and the order and the messages of the `TypeError`s are the
//! ones the saved original results fix (`recipe_v2_cases`, `recipe_formulas`). All 19 functions go through the same binding.

use agent_runtime_support::pyjson::loads;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use recipe_runtime::operations::{resolve, FUNCTIONS};

fn json(text: &str) -> PyValue {
    loads(text).expect("test JSON")
}

/// A parsed config with nested members the formulas never read: a deep structure is what a copying call would duplicate.
const PARSED: &str = r#"{"num_hidden_layers": 32, "num_key_value_heads": 8, "head_dim": 128, "max_position_embeddings": 131072, "num_params": 8000000000, "prequantized": false, "serve_bpw": 2.0, "extras": {"layers": [[1, 2, {"deep": [3, 4, [5, {"k": "v"}]]}], [6]], "notes": {"a": {"b": {"c": [true, null, 1.5]}}}}}"#;
const CANDIDATES: &str = r#"[{"id": "b", "quantization": "fp8", "max_model_len": 4096, "gpu_memory_utilization": 0.9, "meta": {"tags": ["x", ["y"]]}}, {"id": "a", "quantization": "none", "max_model_len": 8192, "gpu_memory_utilization": 0.5}]"#;

fn failure_text(operation: &str, args: &[PyValue], kwargs: &PyDict) -> (String, String) {
    let function = resolve(operation).expect("a known function");
    let failure = function.call(args, kwargs).expect_err("the call must fail");
    (
        failure.class.to_string(),
        failure.message.to_utf8().expect("text"),
    )
}

#[test]
fn there_are_19_functions_and_each_fits_the_inline_argument_storage() {
    assert_eq!(FUNCTIONS.len(), 19);
    for function in FUNCTIONS {
        assert!(
            function.params.len() <= 8,
            "{}.{} has {} parameters",
            function.module,
            function.name,
            function.params.len()
        );
        // Every function binds when it is given its required arguments, and `bind` (the owned adapter) agrees with the borrow.
        let args: Vec<PyValue> = function
            .params
            .iter()
            .filter(|param| param.default.is_none())
            .enumerate()
            .map(|(index, _)| PyValue::int(index as i64))
            .collect();
        let none = PyDict::new();
        let borrowed = function.bind_borrowed(&args, &none).expect("binds");
        let owned = function.bind(&args, &none).expect("binds");
        assert_eq!(owned.len(), function.params.len());
        for (index, value) in owned.iter().enumerate() {
            assert_eq!(&borrowed[index], value);
        }
    }
}

#[test]
fn supplied_arguments_are_the_callers_own_objects_not_copies() {
    let function = resolve("estimate_vram.estimate").expect("estimate");
    let parsed = json(PARSED);
    let candidate = match json(CANDIDATES) {
        PyValue::List(mut items) => items.remove(0),
        other => panic!("{}", other.repr()),
    };
    // Positionals: parsed, candidate, tp; keywords: budget_gib, safety_margin (in the order the caller wrote them).
    let args = vec![parsed, candidate, PyValue::int(2)];
    let mut kwargs = PyDict::new();
    kwargs.set("safety_margin", PyValue::Float(0.9));
    kwargs.set("budget_gib", PyValue::Float(80.0));
    let bound = function.bind_borrowed(&args, &kwargs).expect("binds");
    for (index, value) in args.iter().enumerate() {
        assert!(
            std::ptr::eq(&bound[index], value),
            "positional {index} was copied"
        );
    }
    let in_kwargs = |name: &str| kwargs.get(name).expect("a keyword");
    assert!(
        std::ptr::eq(&bound[3], in_kwargs("budget_gib")),
        "keyword was copied"
    );
    assert!(
        std::ptr::eq(&bound[4], in_kwargs("safety_margin")),
        "keyword was copied"
    );
    // The default for `kv_cache_dtype_bytes` is made for this call: equal to 2, and nobody else's object.
    assert_eq!(&bound[5], &PyValue::int(2));
    assert!(args.iter().all(|value| !std::ptr::eq(&bound[5], value)));
}

#[test]
fn defaults_apply_where_nothing_was_supplied_and_keywords_override_them() {
    let absolute = resolve("estimate_vram.estimate_absolute").expect("estimate_absolute");
    let parsed = json(PARSED);
    let candidate = match json(CANDIDATES) {
        PyValue::List(items) => items[0].clone(),
        other => panic!("{}", other.repr()),
    };
    let base = vec![parsed, candidate];
    let by_default = absolute.call(&base, &PyDict::new()).expect("defaults");
    // The explicit spelling of the same defaults gives the same value.
    let mut spelled = base.clone();
    spelled.extend([PyValue::None, PyValue::None, PyValue::int(2)]);
    assert_eq!(
        absolute.call(&spelled, &PyDict::new()).expect("explicit"),
        by_default
    );
    // A keyword for the last parameter replaces its default and leaves the two before it at theirs.
    let mut kwargs = PyDict::new();
    kwargs.set("kv_dtype_bytes", PyValue::int(1));
    let bound = absolute.bind_borrowed(&base, &kwargs).expect("binds");
    assert_eq!(
        (&bound[2], &bound[3], &bound[4]),
        (&PyValue::None, &PyValue::None, &PyValue::int(1))
    );
    let overridden = absolute.call(&base, &kwargs).expect("override");
    assert_ne!(
        overridden, by_default,
        "a 1-byte KV differs from a 2-byte KV"
    );
    // Two calls in a row do not share a default (each one made its own).
    assert_eq!(
        absolute.call(&base, &PyDict::new()).expect("again"),
        by_default
    );
}

#[test]
fn a_call_changes_no_nested_input_and_a_result_can_feed_the_next_call() {
    let parsed = json(PARSED);
    let candidates = json(CANDIDATES);
    let (parsed_before, candidates_before) = (parsed.clone(), candidates.clone());
    let rank = resolve("rank_recipes.rank").expect("rank");
    let args = vec![
        parsed.clone(),
        candidates.clone(),
        PyValue::int(1),
        PyValue::Float(80.0),
        PyValue::Float(0.9),
    ];
    let mut kwargs = PyDict::new();
    kwargs.set("kv_bytes", PyValue::int(2));
    let ranked = rank.call(&args, &kwargs).expect("ranks");
    assert_eq!((&args[0], &args[1]), (&parsed_before, &candidates_before));
    // The result is an owned whole value; feeding it to the report leaves it, the parsed config and the rest untouched.
    let result_before = ranked.clone();
    let report = resolve("rank_recipes.render_report").expect("render_report");
    let args = vec![
        ranked,
        parsed,
        PyValue::Float(80.0),
        PyValue::Float(0.9),
        PyValue::int(1),
    ];
    let text = report.call(&args, &PyDict::new()).expect("renders");
    assert!(matches!(text, PyValue::Str(_)));
    assert_eq!((&args[0], &args[1]), (&result_before, &parsed_before));
    // The per-layer and per-token functions also only read the nested config.
    for operation in [
        "estimate_vram.kv_bearing_layers",
        "estimate_vram.kv_layers_source",
    ] {
        let function = resolve(operation).expect("function");
        let args = vec![args[1].clone()];
        function.call(&args, &PyDict::new()).expect("reads");
        assert_eq!(args[0], parsed_before, "{operation}");
    }
    let estimate = resolve("estimate_vram.estimate").expect("estimate");
    let candidate = match &candidates_before {
        PyValue::List(items) => items[0].clone(),
        _ => unreachable!(),
    };
    let args = vec![
        parsed_before.clone(),
        candidate.clone(),
        PyValue::int(2),
        PyValue::Float(80.0),
        PyValue::Float(0.9),
    ];
    let value = estimate.call(&args, &PyDict::new()).expect("estimates");
    assert_eq!((&args[0], &args[1]), (&parsed_before, &candidate));
    // The result does not alias the input: it is an owned whole value that outlives the borrowed arguments.
    drop(args);
    assert!(matches!(value, PyValue::Dict(_)));
}

#[test]
fn a_binding_failure_raises_in_cpythons_order_with_its_messages() {
    let seven: Vec<PyValue> = (1..=7).map(PyValue::int).collect();
    let kwargs = |entries: &[(&str, i64)]| {
        let mut dict = PyDict::new();
        for (name, value) in entries {
            dict.set(name, PyValue::int(*value));
        }
        dict
    };
    let type_error = |message: &str| ("TypeError".to_string(), message.to_string());
    // An unexpected keyword is raised before a repeated one and before surplus positionals.
    assert_eq!(
        failure_text(
            "estimate_vram.estimate",
            &seven,
            &kwargs(&[("x", 1), ("parsed", 1)])
        ),
        type_error("estimate() got an unexpected keyword argument 'x'")
    );
    // A repeated keyword is raised before surplus positionals.
    assert_eq!(
        failure_text("estimate_vram.estimate", &seven, &kwargs(&[("parsed", 1)])),
        type_error("estimate() got multiple values for argument 'parsed'")
    );
    assert_eq!(
        failure_text("estimate_vram.estimate", &seven, &PyDict::new()),
        type_error("estimate() takes from 5 to 6 positional arguments but 7 were given")
    );
    // Missing parameters are reported after the keywords were applied, in parameter order.
    assert_eq!(
        failure_text(
            "estimate_vram.estimate",
            &seven[..2],
            &kwargs(&[("safety_margin", 1)])
        ),
        type_error("estimate() missing 2 required positional arguments: 'tp' and 'budget_gib'")
    );
    assert_eq!(
        failure_text("quant_table.vllm_quant_bpw", &[], &PyDict::new()),
        type_error(
            "vllm_quant_bpw() missing 2 required positional arguments: 'quant' and 'serve_bpw'"
        )
    );
    assert_eq!(
        failure_text("quant_table.dtype_bpw", &seven[..2], &PyDict::new()),
        type_error("dtype_bpw() takes 1 positional argument but 2 were given")
    );
    // A failing call leaves its (borrowed) arguments as they were.
    let args = vec![json(PARSED)];
    let before = args.clone();
    let _ = resolve("estimate_vram.estimate")
        .expect("estimate")
        .call(&args, &PyDict::new());
    assert_eq!(args, before);
    // The owned adapter raises the same errors.
    let failure = resolve("estimate_vram.estimate")
        .expect("estimate")
        .bind(&seven, &PyDict::new())
        .expect_err("surplus");
    assert_eq!(
        failure.message.to_utf8().expect("text"),
        "estimate() takes from 5 to 6 positional arguments but 7 were given"
    );
}
