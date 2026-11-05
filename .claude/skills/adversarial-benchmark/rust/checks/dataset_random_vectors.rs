//! The generator behind the random-replacement selection: draws of CPython's own `random.Random(str).randrange(n)`
//! for seeds of every shape (non-ASCII, a leading NUL, very long, the `seed:index` form `slice` uses) and bounds from
//! 1 to 2**64 - 1 (one-word, word-boundary and two-word draws, small bounds that reject most candidates).

#[path = "dataset_common.rs"]
mod common;

use common::*;
use sweep_stop::dataset_random::PythonRandom;
use sweep_stop::pystr::PyStr;
use sweep_stop::pyvalue::PyValue;

#[test]
fn seeded_draws_equal_the_standard_library() {
    let vectors = list(read_json("prng_vectors.json"));
    assert_eq!(vectors.len(), 285);
    let mut failures = Vec::new();
    for vector in &vectors {
        let vector = dict(vector);
        let seed = text(field(vector, "seed"));
        let PyValue::Int(bound) = field(vector, "bound") else {
            panic!("bound is an integer")
        };
        let bound = u64::try_from(bound.clone()).expect("bound fits 64 bits");
        let mut generator = PythonRandom::from_text(&PyStr::new(&seed)).expect("encodable seed");
        let drawn: Vec<String> = (0..40)
            .map(|_| generator.randbelow(bound).to_string())
            .collect();
        let wanted: Vec<String> = list(field(vector, "draws").clone())
            .iter()
            .map(|draw| match draw {
                PyValue::Int(number) => number.to_string(),
                other => panic!("draw {}", other.repr()),
            })
            .collect();
        if drawn != wanted {
            failures.push(format!("{seed:?} bound {bound}"));
        }
    }
    assert!(failures.is_empty(), "differing sequences {failures:?}");
}

#[test]
fn a_seed_with_a_lone_surrogate_cannot_be_encoded() {
    let seed =
        PyStr::from_code_points(vec![u32::from('a'), 0xD800, u32::from(':'), u32::from('1')]);
    let failure = PythonRandom::from_text(&seed).err().expect("refused");
    assert_eq!(
        failure.message(),
        "'utf-8' codec can't encode character '\\ud800' in position 1: surrogates not allowed"
    );
}
