//! `adversarial_stress` prompt and payload bytes against values captured once from the original Python
//! (`random.Random(seed).choice` over CPython's MT19937, `json.dumps` of `_payload`): `checks/data/stress_*.json`.

use agent_runtime_support::pyjson;
use terraforming_runtime::{py, stress};

fn data(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("checks/data")
            .join(name),
    )
    .unwrap()
}

#[test]
fn filler_matches_original() {
    let want = pyjson::loads(&data("stress_filler.json")).unwrap();
    for (key, tokens, seed) in [
        ("f1", 100i64, 1u64),
        ("f7920", 10, 7920),
        ("big", 3, (1u64 << 40) + 5),
        ("zero", -5, 0),
    ] {
        assert_eq!(
            py::get_str(&want, key).unwrap_or_default(),
            stress::filler(tokens, seed),
            "{key}"
        );
    }
}

#[test]
fn payload_matches_original() {
    let p = stress::Params {
        model: "m".into(),
        prompt_tokens: 8,
        max_tokens: 4,
        multimodal: true,
    };
    assert_eq!(
        data("stress_payload.json").trim_end(),
        stress::payload(&p, 3).dumps_ascii().unwrap()
    );
}
