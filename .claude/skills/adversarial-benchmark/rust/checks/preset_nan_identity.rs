//! Which Python object a NaN is. CPython's `in`, dict lookup and container equality ask "same object?" before `==`,
//! so a NaN two places share is found again although `nan != nan`. The observations below were made on the original
//! interpreter (3.12.3, PyYAML 6.0.3) and frozen: `json.loads('[NaN,NaN]')` holds one object, shared with every other
//! JSON load and different from YAML's; YAML `.nan` is one object for the process (`SafeConstructor.nan_value`), while
//! `!!float nan` builds one per node (an alias reuses it); the numeric `==` of two NaNs stays false.
//!
//! The identity is the object itself (an `Arc` carried as `PyValue::Shared`), not a number inside the float, so these
//! checks need no count of loads or nodes: separate objects are separate allocations whatever their number.

use sweep_stop::pyjson::{loads, loads_objects};
use sweep_stop::pysort::{py_eq, same_or_eq};
use sweep_stop::pystr::PyStr;
use sweep_stop::pyvalue::{nan_object, same_nan_object, PyValue};
use sweep_stop::pyyaml::{safe_load, safe_load_text};

fn items(value: &PyValue) -> Vec<PyValue> {
    match value.peel() {
        PyValue::List(items) => items.clone(),
        other => panic!("not a list: {}", other.type_name()),
    }
}

fn yaml(text: &str) -> Vec<PyValue> {
    items(&safe_load_text(&PyStr::new(text), 1).expect("YAML loads"))
}

#[test]
fn json_nan_is_one_object_for_the_process() {
    let pair = items(&loads_objects("[NaN,NaN]").unwrap());
    let other = items(&loads_objects("[NaN]").unwrap());
    assert!(same_nan_object(&pair[0], &pair[1]) && same_nan_object(&pair[0], &other[0]));
    assert!(!py_eq(&pair[0], &pair[1]), "numeric == of NaN stays false");
    assert!(
        same_or_eq(&pair[0], &pair[1]),
        "`in` and container equality find the same object"
    );
    assert!(
        py_eq(
            &PyValue::List(vec![pair[0].clone()]),
            &PyValue::List(vec![pair[1].clone()])
        ),
        "[nan] == [nan] holds one object"
    );
    assert!(
        matches!(pair[0].peel(), PyValue::Float(number) if number.is_nan()),
        "it is a NaN to every reader"
    );
}

#[test]
fn the_plain_decoder_keeps_plain_floats_for_its_readers() {
    let plain = items(&loads("[NaN,NaN]").unwrap());
    assert!(
        matches!(plain[0], PyValue::Float(number) if number.is_nan()),
        "readers that match on Float see Float"
    );
    assert!(!same_nan_object(&plain[0], &plain[1]) && !same_or_eq(&plain[0], &plain[1]));
    let file_mode = items(&safe_load(b"[.nan]", &PyStr::new("m.yaml")).unwrap());
    assert!(
        matches!(file_mode[0], PyValue::Float(number) if number.is_nan()),
        "a YAML file load keeps plain floats too"
    );
}

#[test]
fn yaml_nan_origins_are_kept_apart() {
    let all = yaml("[.nan, .NaN, !!float nan, !!float nan, &n !!float nan, *n]");
    assert!(same_nan_object(&all[0], &all[1]), ".nan is one object");
    assert!(
        !same_nan_object(&all[2], &all[3]),
        "two `!!float nan` nodes are two objects"
    );
    assert!(
        same_nan_object(&all[4], &all[5]),
        "an alias reuses its node's object"
    );
    assert!(!same_nan_object(&all[0], &all[2]));
    let again = yaml("[.nan]");
    assert!(
        same_nan_object(&all[0], &again[0]),
        ".nan is the same object across loads"
    );
    let fresh = yaml("[!!float nan]");
    assert!(
        !same_nan_object(&all[2], &fresh[0]),
        "nodes of different documents are different objects"
    );
    let json = items(&loads_objects("[NaN]").unwrap());
    assert!(
        !same_nan_object(&json[0], &all[0]),
        "JSON's NaN is not YAML's"
    );
    assert!(!same_or_eq(&all[2], &all[3]) && !py_eq(&all[2], &all[2]));
}

#[test]
fn identity_is_the_object_not_a_number_so_it_cannot_collide() {
    // two objects stay two objects however many exist; a reference-copy of one stays that one
    let first = nan_object();
    let second = nan_object();
    assert!(same_nan_object(&first, &first.clone()) && same_or_eq(&first, &first.clone()));
    assert!(!same_nan_object(&first, &second) && !same_or_eq(&first, &second));
    let many: Vec<PyValue> = (0..2000).map(|_| nan_object()).collect();
    for (index, value) in many.iter().enumerate() {
        assert!(same_nan_object(value, &value.clone()));
        assert!(!same_nan_object(value, &many[(index + 1) % many.len()]));
    }
    // numeric equality is a separate question and stays false for every pair, including an object with itself
    assert!(!py_eq(&first, &first.clone()) && !py_eq(&first, &second));
}

#[test]
fn a_bare_float_nan_is_an_object_nobody_shares() {
    let bare = PyValue::Float(f64::NAN);
    assert!(!same_nan_object(&bare, &bare.clone()) && !same_or_eq(&bare, &bare.clone()));
    assert!(!py_eq(&PyValue::Float(f64::NAN), &PyValue::Float(f64::NAN)));
}
