//! Dict key provenance (finding R3-K1): a JSON/YAML string is a string whatever characters it holds. The old encoding
//! stored YAML's non-string keys (`true:`, `1:`) as private-looking text, so a user string spelled like that text was
//! taken for such a key (`{"\u0000yaml-key:num:1\u0000True": 5}` came out as `{"true": 5}`). Keys now carry their kind
//! as a type. `key_encoding_cases.json` holds what the ORIGINAL printed for: the six reference CLI observations and four
//! PyYAML `repr` observations frozen before the fix, the root's probe and the independent run (with its negative
//! control), and ten more captures (marker-looking strings of several shapes, a lone surrogate, nested objects).

#[path = "common.rs"]
mod common;

use common::{command, data_path, dict, list, load_json_file, text_of};
use std::process::Stdio;
use sweep_stop::pyvalue::PyValue;

fn int_of(value: Option<&PyValue>) -> i32 {
    match value {
        Some(PyValue::Int(number)) => number.to_string().parse().expect("small integer"),
        other => panic!("expected an integer, got {other:?}"),
    }
}

/// A traceback is compared by what precedes it and its last line.
fn projected(text: &str) -> String {
    const TB: &str = "Traceback (most recent call last):\n";
    match text.split_once(TB) {
        None => text.to_string(),
        Some((head, tail)) => format!(
            "{head}[traceback] {}",
            tail.trim_end_matches('\n')
                .rsplit('\n')
                .next()
                .unwrap_or("")
        ),
    }
}

#[test]
fn keys_keep_the_kind_the_original_gives_them() {
    let fixture = load_json_file(&data_path("roofline_verdict/key_encoding_cases.json"));
    let fixture = dict(&fixture);
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("key_encoding_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch");
    let PyValue::Dict(files) = fixture.get("files").expect("files") else {
        panic!("files is not an object");
    };
    for (name, content) in files.iter() {
        let PyValue::Str(content) = content else {
            panic!("file content is not text")
        };
        // JSON inputs hold lone surrogates as escapes: write the text as it is (UTF-8 where encodable)
        std::fs::write(
            dir.join(name.to_utf8().expect("file name")),
            content
                .to_utf8()
                .expect("fixture files are text")
                .as_bytes(),
        )
        .expect("input file");
    }
    let cases = list(fixture.get("cases").expect("cases").clone());
    assert!(cases.len() >= 23, "only {} key cases", cases.len());
    let mut failures = Vec::new();
    for case in &cases {
        let case = dict(case);
        let id = text_of(case.get("id"));
        let argv: Vec<String> = list(case.get("argv").expect("argv").clone())
            .iter()
            .map(|arg| text_of(Some(arg)).replace("{DIR}", dir.to_str().unwrap()))
            .collect();
        let bin = match text_of(case.get("tool")).as_str() {
            "verdict_rule" => env!("CARGO_BIN_EXE_verdict_rule"),
            "roofline" => env!("CARGO_BIN_EXE_roofline"),
            other => panic!("unknown tool {other}"),
        };
        let output = command(bin, None, &argv)
            .stdin(Stdio::null())
            .output()
            .expect("run");
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = projected(&String::from_utf8_lossy(&output.stderr));
        let exit = output.status.code().unwrap_or(-1);
        if exit != int_of(case.get("exit"))
            || stdout != text_of(case.get("stdout"))
            || stderr != projected(&text_of(case.get("stderr")))
        {
            failures.push(format!(
                "{id}: exit {exit} (original {}), stdout {:?}, stderr {:?}",
                int_of(case.get("exit")),
                &stdout[..stdout.len().min(160)],
                &stderr[..stderr.len().min(120)]
            ));
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        failures.is_empty(),
        "{} of {} key cases differ:\n{}",
        failures.len(),
        cases.len(),
        failures.join("\n")
    );
}
