//! Checks of `pyhash` (CPython 3.12.3 `str`/`tuple` hash and `frozenset` slot order): the saved observation of the installed interpreter for the hash
//! seeds 0 and 17 is replayed through the generic API - 64 values and 34 generator-built frozensets per seed - plus structural checks of what the
//! observation cannot name (equal-but-distinct keys, shared hashes, growth, first-equal retention).
//!
//! The case file (`cases.json`) and the two observations (`seed-0.json`, `seed-17.json`) are test data under `checks/data/pyhash_reference/`;
//! the production module holds no case, value or expected hash. These checks start no Python, process, socket or benchmark.

use agent_runtime_support::pyjson::loads;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use upstream_runtime::pyhash::{
    frozenset_iteration_order, hash_str, hash_tuple, HashSecret, PyHash,
};

const DATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/checks/data/pyhash_reference");

fn read(name: &str) -> PyValue {
    let path = format!("{DATA}/{name}");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    loads(&text).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn dict(value: &PyValue) -> &PyDict {
    match value {
        PyValue::Dict(map) => map,
        other => panic!("not an object: {}", other.repr()),
    }
}

fn list(value: &PyValue) -> &[PyValue] {
    match value {
        PyValue::List(items) => items,
        other => panic!("not an array: {}", other.repr()),
    }
}

fn field<'a>(row: &'a PyDict, name: &str) -> &'a PyValue {
    row.get(name)
        .unwrap_or_else(|| panic!("missing field {name}"))
}

fn text(value: &PyValue) -> String {
    match value {
        PyValue::Str(text) => text
            .to_utf8()
            .unwrap_or_else(|| panic!("not UTF-8 text: {}", value.repr())),
        other => panic!("not text: {}", other.repr()),
    }
}

/// A value the case file describes: `["str", text]` or `["tuple", [value, ...]]`.
#[derive(Clone, Debug, PartialEq)]
enum Described {
    Str(PyStr),
    Tuple(Vec<Described>),
}

fn described(value: &PyValue) -> Described {
    let parts = list(value);
    match (&parts[0], &parts[1]) {
        (PyValue::Str(kind), PyValue::Str(payload)) if kind.equals("str") => {
            Described::Str(payload.clone())
        }
        (PyValue::Str(kind), PyValue::List(items)) if kind.equals("tuple") => {
            Described::Tuple(items.iter().map(described).collect())
        }
        _ => panic!("unknown value description {}", value.repr()),
    }
}

fn hash_of(value: &Described, secret: &HashSecret) -> PyHash {
    match value {
        Described::Str(text) => hash_str(text, secret),
        Described::Tuple(items) => hash_tuple(
            &items
                .iter()
                .map(|item| hash_of(item, secret))
                .collect::<Vec<_>>(),
        ),
    }
}

/// The generator values of one saved frozenset case: id, key and its hash (the forced `__hash__` result when the case names one).
struct Key {
    id: String,
    key: Described,
    hash: PyHash,
}

fn keys_of(row: &PyDict, secret: &HashSecret) -> Vec<Key> {
    list(field(row, "items"))
        .iter()
        .map(|item| {
            let item = dict(item);
            let key = described(field(item, "key"));
            let hash = match item.get("forced_hash") {
                Some(forced) => PyHash::from_dunder_hash_result(
                    text(forced).parse().expect("forced hash is an i64"),
                ),
                None => hash_of(&key, secret),
            };
            Key {
                id: text(field(item, "id")),
                key,
                hash,
            }
        })
        .collect()
}

fn retained(keys: &[Key]) -> Vec<usize> {
    let items: Vec<(PyHash, &Described)> = keys.iter().map(|key| (key.hash, &key.key)).collect();
    frozenset_iteration_order(&items)
}

fn seed_of(observation: &PyValue) -> u32 {
    text(field(dict(observation), "seed"))
        .parse()
        .expect("seed")
}

fn replay_seed(observation_file: &str) -> (usize, usize) {
    let cases = read("cases.json");
    let observation = read(observation_file);
    let secret = HashSecret::from_seed(seed_of(&observation));
    let observed = dict(&observation);

    let (case_values, observed_values) = (
        list(field(dict(&cases), "values")),
        list(field(observed, "values")),
    );
    assert_eq!(case_values.len(), observed_values.len());
    for (case, seen) in case_values.iter().zip(observed_values) {
        let (case, seen) = (dict(case), dict(seen));
        assert_eq!(text(field(case, "id")), text(field(seen, "id")));
        let value = described(field(case, "value"));
        assert_eq!(
            hash_of(&value, &secret).value().to_string(),
            text(field(seen, "hash")),
            "{observation_file}: {}",
            text(field(case, "id"))
        );
    }

    let (case_sets, observed_sets) = (
        list(field(dict(&cases), "frozensets")),
        list(field(observed, "frozensets")),
    );
    assert_eq!(case_sets.len(), observed_sets.len());
    for (case, seen) in case_sets.iter().zip(observed_sets) {
        let (case, seen) = (dict(case), dict(seen));
        let id = text(field(case, "id"));
        assert_eq!(id, text(field(seen, "id")));
        let keys = keys_of(case, &secret);
        let order = retained(&keys);
        let ids: Vec<&str> = order.iter().map(|&index| keys[index].id.as_str()).collect();
        let expected_ids: Vec<String> = list(field(seen, "order")).iter().map(text).collect();
        assert_eq!(
            ids, expected_ids,
            "{observation_file}: iteration order of {id}"
        );
        let hashes: Vec<String> = order
            .iter()
            .map(|&index| keys[index].hash.value().to_string())
            .collect();
        let expected_hashes: Vec<String> = list(field(seen, "retained_key_hashes"))
            .iter()
            .map(text)
            .collect();
        assert_eq!(
            hashes, expected_hashes,
            "{observation_file}: retained key hashes of {id}"
        );
    }
    (observed_values.len(), observed_sets.len())
}

#[test]
fn saved_observation_of_seed_0_is_reproduced() {
    assert_eq!(replay_seed("seed-0.json"), (64, 34));
}

#[test]
fn saved_observation_of_seed_17_is_reproduced() {
    assert_eq!(replay_seed("seed-17.json"), (64, 34));
}

#[test]
fn the_observations_are_not_vacuous() {
    // A secret of another seed must disagree with the saved hashes, and the saved sets must be large enough to have grown.
    let cases = read("cases.json");
    let observation = read("seed-0.json");
    let wrong = HashSecret::from_seed(17);
    let disagreeing = list(field(dict(&cases), "values"))
        .iter()
        .zip(list(field(dict(&observation), "values")))
        .filter(|(case, seen)| {
            let value = described(field(dict(case), "value"));
            hash_of(&value, &wrong).value().to_string() != text(field(dict(seen), "hash"))
        })
        .count();
    assert!(
        disagreeing >= 60,
        "only {disagreeing} of 64 hashes depend on the seed"
    );
    let largest = list(field(dict(&cases), "frozensets"))
        .iter()
        .map(|row| list(field(dict(row), "items")).len())
        .max()
        .unwrap();
    assert!(largest >= 128);
}

#[test]
fn seed_zero_is_the_zero_secret_and_other_seeds_are_not() {
    assert_eq!(HashSecret::from_seed(0), HashSecret::from_seed(0));
    assert_ne!(HashSecret::from_seed(0), HashSecret::from_seed(17));
    assert_ne!(HashSecret::from_seed(17), HashSecret::from_seed(18));
    assert_eq!(
        hash_str(&PyStr::new(""), &HashSecret::from_seed(17)).value(),
        0
    );
}

#[test]
fn a_dunder_hash_of_minus_one_reads_as_minus_two() {
    assert_eq!(PyHash::from_dunder_hash_result(-1).value(), -2);
    assert_eq!(PyHash::from_dunder_hash_result(-2).value(), -2);
    assert_eq!(PyHash::from_dunder_hash_result(i64::MIN).value(), i64::MIN);
    assert_eq!(PyHash::from_dunder_hash_result(i64::MAX).value(), i64::MAX);
}

#[test]
fn equal_hashes_of_different_keys_stay_distinct_and_keep_input_order_of_first_arrival() {
    let hashed = |hash: i64, name: &str| (PyHash::from_dunder_hash_result(hash), name.to_string());
    let items = [
        hashed(5, "a"),
        hashed(5, "b"),
        hashed(5, "a"),
        hashed(5, "c"),
        hashed(6, "b"),
    ];
    let order = frozenset_iteration_order(&items);
    let names: Vec<&str> = order.iter().map(|&index| items[index].1.as_str()).collect();
    // `("b", hash 6)` is a different key from `("b", hash 5)` only by hash; equal hash and equal text is the same key, so the later "a" is dropped.
    assert_eq!(order.len(), 4);
    assert!(
        order.contains(&0)
            && order.contains(&1)
            && order.contains(&3)
            && order.contains(&4)
            && !order.contains(&2)
    );
    assert_eq!(names.iter().filter(|name| **name == "a").count(), 1);
}

#[test]
fn equal_text_with_different_hashes_is_not_merged() {
    // Equality is consulted only for equal hashes: two equal values stored under different hashes are two entries (CPython's contract that equal
    // objects hash equally is the caller's).
    let items = [
        (PyHash::from_dunder_hash_result(1), "same"),
        (PyHash::from_dunder_hash_result(2), "same"),
    ];
    assert_eq!(frozenset_iteration_order(&items).len(), 2);
}

#[test]
fn growth_keeps_every_distinct_item_exactly_once() {
    for count in [0usize, 1, 5, 6, 12, 13, 100, 1000] {
        let items: Vec<(PyHash, usize)> = (0..count)
            .map(|n| {
                (
                    PyHash::from_dunder_hash_result(
                        (n as i64).wrapping_mul(0x9E37_79B9_7F4A_7C15u64 as i64),
                    ),
                    n,
                )
            })
            .collect();
        let mut order = frozenset_iteration_order(&items);
        order.sort_unstable();
        assert_eq!(order, (0..count).collect::<Vec<_>>(), "count {count}");
    }
}

#[test]
fn the_saved_first_equal_case_keeps_the_earlier_spelling() {
    let cases = read("cases.json");
    let row = list(field(dict(&cases), "frozensets"))
        .iter()
        .find(|row| text(field(dict(row), "id")) == "set-first-equal-retention")
        .expect("case");
    let keys = keys_of(dict(row), &HashSecret::from_seed(0));
    let kept: Vec<&str> = retained(&keys)
        .iter()
        .map(|&index| keys[index].id.as_str())
        .collect();
    assert!(kept.contains(&"first") && !kept.contains(&"equal-later"));
    assert!(kept.contains(&"third") && !kept.contains(&"third-equal"));
    assert!(kept.contains(&"other"));
}

#[test]
fn collision_cases_keep_all_forty_eight_distinct_keys() {
    let cases = read("cases.json");
    let mut seen = 0;
    for row in list(field(dict(&cases), "frozensets")) {
        if !text(field(dict(row), "id")).starts_with("set-collision-") {
            continue;
        }
        let keys = keys_of(dict(row), &HashSecret::from_seed(0));
        assert_eq!(keys.len(), 48);
        assert!(
            keys.iter().all(|key| key.hash == keys[0].hash),
            "one forced hash per collision case"
        );
        assert_eq!(retained(&keys).len(), 48);
        seen += 1;
    }
    assert_eq!(seen, 7);
}

mod entropy_selection {
    use agent_runtime_support::pystr::PyStr;
    use upstream_runtime::pyhash::{
        hash_str, os_entropy, parse_hash_seed, HashSecret, HashSeedSetting,
    };

    #[test]
    fn the_environment_selects_the_documented_setting() {
        assert_eq!(parse_hash_seed(None), Ok(HashSeedSetting::Random));
        assert_eq!(parse_hash_seed(Some(b"")), Ok(HashSeedSetting::Random));
        assert_eq!(
            parse_hash_seed(Some(b"random")),
            Ok(HashSeedSetting::Random)
        );
        assert_eq!(parse_hash_seed(Some(b"0")), Ok(HashSeedSetting::Fixed(0)));
        assert_eq!(parse_hash_seed(Some(b"17")), Ok(HashSeedSetting::Fixed(17)));
        assert_eq!(
            parse_hash_seed(Some(b"4294967295")),
            Ok(HashSeedSetting::Fixed(u32::MAX))
        );
        for refused in [&b"4294967296"[..], b"-1", b" 1", b"abc", b"1.5", b"RANDOM"] {
            assert!(parse_hash_seed(Some(refused)).is_err(), "{refused:?}");
        }
    }

    #[test]
    fn a_fixed_seed_never_reads_entropy_and_matches_the_explicit_profiles() {
        let mut calls = 0;
        let mut entropy = |_: &mut [u8]| {
            calls += 1;
            Ok(())
        };
        let seventeen = HashSecret::for_setting(HashSeedSetting::Fixed(17), &mut entropy).unwrap();
        let zero = HashSecret::for_setting(HashSeedSetting::Fixed(0), &mut entropy).unwrap();
        assert_eq!(calls, 0);
        assert_eq!(seventeen, HashSecret::from_seed(17));
        assert_eq!(zero, HashSecret::from_secret_bytes(&[0; 24]));
    }

    #[test]
    fn a_random_start_fills_the_whole_secret_from_the_entropy_once() {
        let mut calls = 0;
        let mut sizes = Vec::new();
        let mut entropy = |buffer: &mut [u8]| {
            calls += 1;
            sizes.push(buffer.len());
            for (position, byte) in buffer.iter_mut().enumerate() {
                *byte = position as u8 + 1;
            }
            Ok(())
        };
        let secret = HashSecret::for_setting(HashSeedSetting::Random, &mut entropy).unwrap();
        assert_eq!((calls, sizes), (1, vec![24]));
        let mut bytes = [0u8; 24];
        for (position, byte) in bytes.iter_mut().enumerate() {
            *byte = position as u8 + 1;
        }
        assert_eq!(secret, HashSecret::from_secret_bytes(&bytes));
        // different entropy, different `str` hashes (a randomized interpreter orders its frozensets differently)
        let other = HashSecret::from_secret_bytes(&[9; 24]);
        let text = PyStr::new("torch");
        assert_ne!(hash_str(&text, &secret), hash_str(&text, &other));
    }

    #[test]
    fn an_entropy_failure_is_an_error_not_a_fixed_secret() {
        let mut entropy = |_: &mut [u8]| Err(std::io::Error::other("no entropy"));
        assert!(HashSecret::for_setting(HashSeedSetting::Random, &mut entropy).is_err());
    }

    #[test]
    fn the_operating_system_entropy_differs_between_reads() {
        let (mut first, mut second) = ([0u8; 24], [0u8; 24]);
        os_entropy(&mut first).unwrap();
        os_entropy(&mut second).unwrap();
        assert_ne!(first, second);
    }
}
