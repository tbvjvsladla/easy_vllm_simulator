//! The binding of Python calls (`pycall`) against what CPython 3.12 does for a function with the same signature. The expected
//! messages were taken from CPython 3.12.3 for these definitions, one call of a small stdlib-only probe (the interpreter is used
//! only as the oracle of the language's own binding rules, never of any owner's function; the probe and its output are kept by
//! the worker, `oracle/pycall_probe.py`):
//!
//! ```python
//! def one(a): ...
//! def two(a, b=2, c=None): ...
//! def gen(p, r, n, root, port, s, force=False, topology="single", v=None, b=None, image=None): ...   # 11 / 5 defaults
//! def extra(candidate, **extra): ...
//! ```

use agent_runtime_support::pycall::{optional, required, Param, Signature};
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};

fn none() -> PyValue {
    PyValue::None
}

fn single() -> PyValue {
    PyValue::str("single")
}

fn flag() -> PyValue {
    PyValue::Bool(false)
}

fn two_default() -> PyValue {
    PyValue::int(2)
}

static ONE: Signature = Signature {
    name: "one",
    params: &[required("a")],
    rest: false,
};

static TWO: Signature = Signature {
    name: "two",
    params: &[
        required("a"),
        optional("b", two_default),
        optional("c", none),
    ],
    rest: false,
};

static GENERATE: Signature = Signature {
    name: "generate",
    params: &[
        required("parsed"),
        required("recipe"),
        required("name"),
        required("repo_root"),
        required("port"),
        required("served_model_name"),
        optional("force", flag),
        optional("topology", single),
        optional("vllm_version", none),
        optional("build_dockerfile", none),
        optional("image", none),
    ],
    rest: false,
};

static EXTRA: Signature = Signature {
    name: "recipe_from_candidate",
    params: &[required("candidate")],
    rest: true,
};

fn ints(values: &[i64]) -> Vec<PyValue> {
    values.iter().map(|&value| PyValue::int(value)).collect()
}

fn keywords(pairs: &[(&str, i64)]) -> PyDict {
    let mut dict = PyDict::new();
    for (key, value) in pairs {
        dict.set(key, PyValue::int(*value));
    }
    dict
}

fn error_of(signature: &Signature, args: &[PyValue], kwargs: &PyDict) -> String {
    match signature.bind(args, kwargs) {
        Ok(_) => panic!("the call was bound"),
        Err(failure) => {
            assert_eq!(failure.class, "TypeError");
            failure.message.to_utf8().expect("text")
        }
    }
}

#[test]
fn supplied_values_stay_where_the_caller_put_them_and_defaults_fill_the_rest() {
    let args = ints(&[10]);
    let kwargs = keywords(&[("c", 30)]);
    let bound = TWO.bind(&args, &kwargs).expect("binds");
    assert_eq!(bound[0], PyValue::int(10));
    assert_eq!(bound[1], PyValue::int(2));
    assert_eq!(bound[2], PyValue::int(30));
    assert!(bound.is_supplied(0) && !bound.is_supplied(1) && bound.is_supplied(2));
    // The value is the very one in the argument list (borrowed, not copied).
    assert!(std::ptr::eq(&bound[0], &args[0]));
    let all = ints(&[1, 2, 3]);
    let empty = PyDict::new();
    let none_given = TWO.bind(&all, &empty).expect("binds");
    assert!(none_given.is_supplied(2));
    assert_eq!(
        (&none_given[0], &none_given[1], &none_given[2]),
        (&PyValue::int(1), &PyValue::int(2), &PyValue::int(3))
    );
}

#[test]
fn eleven_parameters_with_five_trailing_defaults_bind() {
    let args = vec![
        PyValue::str("p"),
        PyValue::str("r"),
        PyValue::str("n"),
        PyValue::str("root"),
        PyValue::int(8080),
        PyValue::str("s"),
    ];
    let mut kwargs = PyDict::new();
    kwargs.set("image", PyValue::str("img"));
    kwargs.set("force", PyValue::int(1));
    let bound = GENERATE.bind(&args, &kwargs).expect("binds");
    assert_eq!(bound[6], PyValue::int(1));
    assert_eq!(bound[7], PyValue::str("single"));
    assert_eq!(bound[8], PyValue::None);
    assert_eq!(bound[9], PyValue::None);
    assert_eq!(bound[10], PyValue::str("img"));
    assert_eq!(bound[4], PyValue::int(8080));
}

#[test]
fn the_failures_are_cpythons_in_cpythons_order() {
    let nothing = PyDict::new();
    // (messages from CPython 3.12.3)
    assert_eq!(
        error_of(&ONE, &[], &nothing),
        "one() missing 1 required positional argument: 'a'"
    );
    assert_eq!(
        error_of(&GENERATE, &[], &nothing),
        "generate() missing 6 required positional arguments: 'parsed', 'recipe', 'name', 'repo_root', 'port', and 'served_model_name'"
    );
    assert_eq!(
        error_of(&GENERATE, &ints(&[1, 2]), &nothing),
        "generate() missing 4 required positional arguments: 'name', 'repo_root', 'port', and 'served_model_name'"
    );
    assert_eq!(
        error_of(&TWO, &[], &nothing),
        "two() missing 1 required positional argument: 'a'"
    );
    assert_eq!(
        error_of(&ONE, &ints(&[1, 2]), &nothing),
        "one() takes 1 positional argument but 2 were given"
    );
    assert_eq!(
        error_of(&TWO, &ints(&[1, 2, 3, 4]), &nothing),
        "two() takes from 1 to 3 positional arguments but 4 were given"
    );
    assert_eq!(
        error_of(&GENERATE, &ints(&[0; 12]), &nothing),
        "generate() takes from 6 to 11 positional arguments but 12 were given"
    );
    assert_eq!(
        error_of(&ONE, &ints(&[1]), &keywords(&[("a", 2)])),
        "one() got multiple values for argument 'a'"
    );
    assert_eq!(
        error_of(&ONE, &ints(&[1]), &keywords(&[("z", 2)])),
        "one() got an unexpected keyword argument 'z'"
    );
    // A keyword error is raised before the count of positionals and before the missing ones.
    assert_eq!(
        error_of(&ONE, &ints(&[1, 2]), &keywords(&[("z", 2)])),
        "one() got an unexpected keyword argument 'z'"
    );
    assert_eq!(
        error_of(&TWO, &[], &keywords(&[("z", 2)])),
        "two() got an unexpected keyword argument 'z'"
    );
    // The first offending keyword in the caller's order decides.
    assert_eq!(
        error_of(&TWO, &ints(&[1]), &keywords(&[("z", 1), ("a", 2)])),
        "two() got an unexpected keyword argument 'z'"
    );
    assert_eq!(
        error_of(&TWO, &ints(&[1]), &keywords(&[("a", 1), ("z", 2)])),
        "two() got multiple values for argument 'a'"
    );
    // A key that is not an identifier is shown by its repr.
    let mut odd = PyDict::new();
    odd.insert(PyStr::new("a b"), PyValue::None);
    assert_eq!(
        error_of(&ONE, &[], &odd),
        "one() got an unexpected keyword argument 'a b'"
    );
}

#[test]
fn a_function_with_rest_keeps_the_keywords_no_parameter_takes_in_order() {
    let args = vec![PyValue::str("candidate")];
    let mut kwargs = PyDict::new();
    kwargs.set("zeta", PyValue::int(1));
    kwargs.set("alpha", PyValue::int(2));
    let bound = EXTRA.bind(&args, &kwargs).expect("binds");
    let rest: Vec<String> = bound
        .rest()
        .iter()
        .map(|(key, value)| format!("{}={}", key.to_utf8().expect("text"), value.repr()))
        .collect();
    assert_eq!(rest, vec!["zeta=1", "alpha=2"]);
    // The parameter's own name is not collected; given twice it is the multiple-values failure.
    let mut named = PyDict::new();
    named.set("candidate", PyValue::int(9));
    named.set("other", PyValue::int(1));
    let bound = EXTRA.bind(&[], &named).expect("binds");
    assert_eq!(bound[0], PyValue::int(9));
    assert_eq!(bound.rest().len(), 1);
    assert_eq!(
        error_of(&EXTRA, &args, &keywords(&[("candidate", 1)])),
        "recipe_from_candidate() got multiple values for argument 'candidate'"
    );
    assert_eq!(
        error_of(&EXTRA, &[], &PyDict::new()),
        "recipe_from_candidate() missing 1 required positional argument: 'candidate'"
    );
    assert_eq!(
        error_of(&EXTRA, &ints(&[1, 2]), &PyDict::new()),
        "recipe_from_candidate() takes 1 positional argument but 2 were given"
    );
}

#[test]
fn names_of_the_unused_params_type_is_irrelevant() {
    // `Param` is plain data: a table of them can be a constant.
    const TABLE: [Param; 2] = [required("x"), optional("y", none)];
    assert_eq!(TABLE[0].name, "x");
    assert!(TABLE[1].default.is_some());
}

/// CPython puts the key's own text between single quotes (no `repr` escaping) in the unexpected-keyword message; checked for
/// keys with a quote, a newline, a tab, a backslash, a double quote, Unicode, a lone surrogate, a lone quote and the empty
/// key. The expected texts are those of CPython 3.12.3 from one stdlib-only probe (the worker's `oracle/keyword_probe.py`).
#[test]
fn an_unexpected_keyword_is_named_by_its_raw_text_between_single_quotes() {
    let cases: [(Vec<u32>, &str); 9] = [
        ("q'uote".chars().map(u32::from).collect(), "q'uote"),
        ("a\nb".chars().map(u32::from).collect(), "a\nb"),
        ("한글".chars().map(u32::from).collect(), "한글"),
        (
            "back\\slash".chars().map(u32::from).collect(),
            "back\\slash",
        ),
        ("dq\"x".chars().map(u32::from).collect(), "dq\"x"),
        (vec![0xDCFF], ""),
        ("tab\there".chars().map(u32::from).collect(), "tab\there"),
        ("'".chars().map(u32::from).collect(), "'"),
        (vec![], ""),
    ];
    for (points, shown) in cases {
        let mut kwargs = PyDict::new();
        kwargs.insert(PyStr::from_code_points(points.clone()), PyValue::int(1));
        let failure = ONE.bind(&[], &kwargs).err().expect("refused");
        assert_eq!(failure.class, "TypeError");
        let mut want: Vec<u32> = "one() got an unexpected keyword argument '"
            .chars()
            .map(u32::from)
            .collect();
        want.extend(&points);
        want.push(u32::from('\''));
        assert_eq!(failure.message.code_points(), want.as_slice(), "{points:?}");
        if !shown.is_empty() {
            assert!(failure.message.to_utf8().expect("text").contains(shown));
        }
    }
}
