//! The 67 observations of the ORIGINAL resolver (62 whole `resolve` values or errors, 5 `check_dataset_runtime`
//! calls) in standard temporary repository trees: the real `campaign_init.py` is copied to the owner path when the
//! case has an owner, `@ROOT@` is the scratch root, and the tree must be byte-identical after the call.

#[path = "preset_common.rs"]
mod preset_common;

use preset_common::*;
use sweep_stop::pyjson::loads;
use sweep_stop::pyvalue::{PyDict, PyValue};

const API: &str = env!("CARGO_BIN_EXE_bench_preset_api");

#[test]
fn frozen_67_original_resolutions_match_whole_records_and_leave_the_tree() {
    let repo = real_repo();
    let cases = load_json(&data_path("resolution_cases.json"));
    assert_eq!(items(&cases).len(), 67);
    let mut failures = Vec::new();
    for case in items(&cases) {
        let id = text(field(case, "id"));
        let root = scratch(&id);
        if let PyValue::Dict(files) = field(case, "files").peel() {
            for (rel, encoded) in files.iter() {
                let path = root.join(rel.to_utf8().unwrap());
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                let raw = base64_decode(&text(encoded));
                std::fs::write(
                    &path,
                    replace_bytes(&raw, b"@ROOT@", root.to_string_lossy().as_bytes()),
                )
                .unwrap();
            }
        }
        if matches!(field(case, "owner_present").peel(), PyValue::Bool(true)) {
            let owner = root.join(OWNER_REL);
            std::fs::create_dir_all(owner.parent().unwrap()).unwrap();
            std::fs::copy(repo.join(OWNER_REL), &owner).unwrap();
        }
        let before = snapshot(&root, false);
        let arguments_json = field(case, "arguments")
            .dumps_ascii()
            .unwrap()
            .replace("@ROOT@", &root.to_string_lossy());
        let mut request = PyDict::new();
        request.set("id", field(case, "id").clone());
        request.set("operation", field(case, "operation").clone());
        request.set("arguments", loads(&arguments_json).unwrap());
        let mut cmd = command(API, &repo);
        cmd.arg("evaluate-jsonl")
            .env_remove("BENCH_PRESET_REPO_ROOT")
            .env_remove("BENCH_PRESET_SKILL_DIR");
        let out = run(
            cmd,
            format!("{}\n", PyValue::Dict(request).dumps_ascii().unwrap()).as_bytes(),
        );
        let answer = String::from_utf8(out.stdout)
            .unwrap()
            .replace(&root.to_string_lossy().to_string(), "@ROOT@");
        let got = loads(answer.trim()).expect("answer is JSON");
        let mut record = PyDict::new();
        record.set("id", field(case, "id").clone());
        let PyValue::Dict(want) = field(case, "expected").peel() else {
            panic!("expected object")
        };
        for (name, value) in want.iter() {
            record.insert(name.clone(), value.clone());
        }
        let same = got.dumps_ascii().unwrap() == PyValue::Dict(record).dumps_ascii().unwrap();
        if !same || before != snapshot(&root, false) {
            failures.push(id);
        }
        let _ = std::fs::remove_dir_all(&root);
    }
    assert!(failures.is_empty(), "differ: {failures:?}");
}
