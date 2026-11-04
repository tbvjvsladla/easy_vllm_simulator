//! `check_dataset_runtime` hashes the dataset source incrementally with the pinned SHA-256 crate: deterministic local
//! files around the 1 MiB read boundary and at 16 and 64 MiB (bytes 0..255 repeated) against digests computed
//! outside the program, and the refusals of the file checks in the original's order.

#[path = "preset_common.rs"]
mod preset_common;

use preset_common::*;
use sweep_stop::pyjson::loads;

const API: &str = env!("CARGO_BIN_EXE_bench_preset_api");

/// (size in bytes, sha256 of bytes `i % 256` for `i < size`), prepared with the interpreter's `hashlib` outside.
const DIGESTS: [(usize, &str); 7] = [
    (
        0,
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    ),
    (
        1,
        "6e340b9cffb37a989ca544e6bb780a2c78901d3fb33738768511a30617afa01d",
    ),
    (
        1048575,
        "d8222d77ac43a9391cb348ce3a3bb3c0221a4a666e907b5bc77de56a41f37848",
    ),
    (
        1048576,
        "fbbab289f7f94b25736c58be46a994c441fd02552cc6022352e3d86d2fab7c83",
    ),
    (
        1048577,
        "607deb6eccbc844880b9d7b523751a4cdba0452727b885c74264bfe1fb7843e2",
    ),
    (
        16777216,
        "341aacac661ccb210720bedaa9ead5d668fe5ea41a73532fc147c71e34040df1",
    ),
    (
        67108864,
        "281e519df3077b557c6b03f5da83c4e8d397219259615dd7c3308f89cae8f2a6",
    ),
];

fn check(values: &str) -> String {
    let request = format!("{{\"id\":\"c\",\"operation\":\"check_dataset_runtime\",\"arguments\":{{\"values\":{values}}}}}\n");
    let mut cmd = command(API, &real_repo());
    cmd.arg("evaluate-jsonl");
    let out = run(cmd, request.as_bytes());
    String::from_utf8(out.stdout).unwrap()
}

fn values(path: &str, sha: &str) -> String {
    format!("{{\"dataset_kind\":\"json_file\",\"dataset_source\":\"custom\",\"dataset_path\":{path:?},\"source_sha256\":{sha:?}}}")
}

#[test]
fn deterministic_files_hash_to_the_externally_computed_digests() {
    let dir = scratch("hash");
    for (size, digest) in DIGESTS {
        let file = dir.join(format!("source-{size}.bin"));
        let block: Vec<u8> = (0..=255u8).collect();
        let mut data = Vec::with_capacity(size);
        while data.len() < size {
            let take = (size - data.len()).min(block.len());
            data.extend_from_slice(&block[..take]);
        }
        std::fs::write(&file, &data).unwrap();
        let answer = loads(check(&values(&file.to_string_lossy(), digest)).trim()).unwrap();
        let value = field(&answer, "value");
        assert_eq!(text(field(&answer, "kind")), "value", "{answer:?}");
        assert_eq!(
            text(field(value, "checked")),
            format!("measured(sha256 일치 · {size} bytes)")
        );
        // a one-character change of the declaration is the original's refusal, naming both digests
        let wrong = format!(
            "{}{}",
            &digest[..63],
            if digest.ends_with('0') { '1' } else { '0' }
        );
        let refused = loads(check(&values(&file.to_string_lossy(), &wrong)).trim()).unwrap();
        assert_eq!(text(field(&refused, "type")), "PresetError");
        assert!(
            text(field(&refused, "message")).contains(&format!("(선언 {wrong} · 실측 {digest})"))
        );
    }
}

#[test]
fn file_checks_refuse_in_the_originals_order() {
    let dir = scratch("hash-refusals");
    let missing = dir.join("missing.bin");
    let answer = loads(check(&values(&missing.to_string_lossy(), "00")).trim()).unwrap();
    assert!(
        text(field(&answer, "message")).starts_with("dataset_path 가 없다(또는 파일이 아니다): ")
    );
    let answer = loads(check(&values(&dir.to_string_lossy(), "00")).trim()).unwrap();
    assert!(
        text(field(&answer, "message")).starts_with("dataset_path 가 없다(또는 파일이 아니다): "),
        "a directory is not a file"
    );
    let unreadable = dir.join("locked.bin");
    std::fs::write(&unreadable, b"x").unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&unreadable, std::fs::Permissions::from_mode(0o000)).unwrap();
    let answer = loads(check(&values(&unreadable.to_string_lossy(), "00")).trim()).unwrap();
    assert!(
        text(field(&answer, "message")).starts_with("dataset_path 를 읽을 수 없다: "),
        "{answer:?}"
    );
    let answer =
        loads(check("{\"dataset_kind\":\"json_file\",\"dataset_source\":\"ephemeral\"}").trim())
            .unwrap();
    assert!(text(field(&answer, "message"))
        .starts_with("dataset_source=ephemeral 은 이번 개정에서 경로가 없다"));
    let answer = loads(check("{\"dataset_kind\":\"synthetic\"}").trim()).unwrap();
    assert_eq!(
        text(field(field(&answer, "value"), "checked")),
        "not_applicable(synthetic)"
    );
    let answer =
        loads(check("{\"dataset_kind\":\"json_file\",\"dataset_source\":\"custom\"}").trim())
            .unwrap();
    assert_eq!(text(field(&answer, "type")), "KeyError");
}
