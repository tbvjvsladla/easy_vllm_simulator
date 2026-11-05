//! The original's `--self-test` (T1..T7): the length-target rule, the JSON-object arguments and the sweep offset of
//! `slice`, each printed as `  [PASS] label` / `  [FAIL] label`.

use crate::dataset_commands::{cmd_slice, Namespace};
use crate::dataset_prep::{
    column_mapper_arg, data_arg, target_rows, Console, Recorder, OUTPUT_COUNT_COL, PREP_META,
    PREP_NAME, PROMPT_COUNT_COL,
};
use crate::guidellm::Failure;
use crate::pyjson::loads;
use crate::pysort::py_eq;
use crate::pystr::PyStr;
use crate::pyvalue::{PyDict, PyValue};
use std::path::PathBuf;

fn pair(count: i64, line: &str) -> PyValue {
    PyValue::Tuple(vec![PyValue::int(count), PyValue::str(line)])
}

fn repeated(count: i64, line: &str, times: usize) -> Vec<PyValue> {
    (0..times).map(|_| pair(count, line)).collect()
}

fn dict(entries: &[(&str, PyValue)]) -> PyValue {
    let mut map = PyDict::new();
    for (key, value) in entries {
        map.set(key, value.clone());
    }
    PyValue::Dict(map)
}

fn parsed(text: &PyStr) -> PyValue {
    loads(&text.to_utf8().expect("ASCII arguments")).expect("the arguments are JSON")
}

fn member<'a>(value: &'a PyValue, key: &str) -> Option<&'a PyValue> {
    match value {
        PyValue::Dict(map) => map.get(key),
        _ => None,
    }
}

/// A scratch folder that disappears with it (`tempfile.TemporaryDirectory`).
struct ScratchDir(PathBuf);

impl ScratchDir {
    fn create() -> Result<Self, Failure> {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos());
        let path =
            std::env::temp_dir().join(format!("dataset_prep_{}_{stamp}", std::process::id()));
        std::fs::create_dir(&path).map_err(|error| {
            Failure::new(
                crate::pyfs::os_error_class(&error),
                &crate::cli::os_error_text(&error, &PyStr::new(&path.display().to_string())),
            )
        })?;
        Ok(ScratchDir(path))
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// `_self_test()`: the exit status, 0 when every check passed.
pub fn self_test(console: &mut dyn Console) -> Result<i32, Failure> {
    let mut failures: Vec<String> = Vec::new();
    let mut check =
        |label: &str, condition: bool, console: &mut dyn Console| -> Result<(), Failure> {
            console.print(&PyStr::new(&format!(
                "  [{}] {label}",
                if condition { "PASS" } else { "FAIL" }
            )))?;
            if !condition {
                failures.push(label.to_string());
            }
            Ok(())
        };
    let length = |value: i64| PyValue::int(value);

    let mut rows = repeated(1025, "a", 5);
    rows.extend(repeated(1024, "b", 2));
    rows.extend(repeated(300, "c", 1));
    let found = target_rows(&PyValue::List(rows), &length(1024))?;
    check(
        "T1 BOS 를 세는 토크나이저(1025 최빈) → 목표 1025 · 5 행",
        py_eq(&found.target, &length(1025)) && found.pool.len() == 5,
        console,
    )?;
    let mut rows = repeated(1024, "a", 4);
    rows.extend(repeated(1022, "z", 1));
    let found = target_rows(&PyValue::List(rows), &length(1024))?;
    check(
        "T2 최빈 1024 → 꼬리 조각(1022) 제외",
        py_eq(&found.target, &length(1024))
            && py_eq(
                &PyValue::List(found.pool),
                &PyValue::List(vec![PyValue::str("a"); 4]),
            ),
        console,
    )?;
    let found = target_rows(&PyValue::List(repeated(512, "a", 4)), &length(1024))?;
    check(
        "T3 목표가 input_len 에서 크게 벗어나면 빈 목록(호출부 fail-closed)",
        found.pool.is_empty(),
        console,
    )?;
    let mapper = parsed(&column_mapper_arg(
        &dict(&[(
            "column_map",
            dict(&[("text_column", PyValue::str("article"))]),
        )]),
        &PyValue::Bool(true),
    )?);
    let data = parsed(&data_arg(
        &PyValue::str("json_file"),
        &PyValue::str("/out/x.jsonl"),
    )?);
    let expected_mappings = dict(&[
        ("text_column", PyValue::str("article")),
        ("prompt_tokens_count_column", PyValue::str(PROMPT_COUNT_COL)),
        ("output_tokens_count_column", PyValue::str(OUTPUT_COUNT_COL)),
    ]);
    let split = dict(&[("split", PyValue::str("train"))]);
    check(
        "T4 데이터·매퍼 인자는 JSON 객체(쉼표 분할 회피)",
        member(&mapper, "column_mappings").is_some_and(|found| py_eq(found, &expected_mappings))
            && member(&data, "load_kwargs").is_some_and(|found| py_eq(found, &split)),
        console,
    )?;

    // T5..T7: the sweep offset of `slice`, run against a prepared dataset of 100 identical-length rows.
    let scratch = ScratchDir::create()?;
    let folder = scratch.0.clone();
    let write = |name: &str, text: String| {
        std::fs::write(folder.join(name), text).map_err(|error| {
            Failure::new(
                crate::pyfs::os_error_class(&error),
                &crate::cli::os_error_text(&error, &PyStr::new(name)),
            )
        })
    };
    write(
        "spec.json",
        "{\"dataset_kind\": \"parquet_file\", \"seed\": 42}".to_string(),
    )?;
    write(
        PREP_META,
        "{\"rows\": 100, \"key\": {\"tokenizer_digest\": \"x\"}}".to_string(),
    )?;
    write(
        PREP_NAME,
        (0..100)
            .map(|id| format!("{{\"{PROMPT_COUNT_COL}\": 1024, \"id\": {id}}}\n"))
            .collect(),
    )?;
    let text_of = |name: &str| PyValue::str(&folder.join(name).display().to_string());
    let slice = |index: i64, salt: &str| -> Result<Vec<i64>, Failure> {
        let mut namespace = PyDict::new();
        namespace.set("spec", text_of("spec.json"));
        namespace.set("cache", PyValue::str(&folder.display().to_string()));
        namespace.set("run_index", PyValue::int(index));
        namespace.set("count", PyValue::int(10));
        namespace.set("input_len", PyValue::int(1024));
        namespace.set("output_len", PyValue::int(256));
        namespace.set("out", text_of("o.jsonl"));
        namespace.set("sidecar", text_of("s.json"));
        namespace.set("tokenizer_label", PyValue::str("t"));
        namespace.set("sweep_salt", PyValue::str(salt));
        cmd_slice(&Namespace(&namespace), &mut Recorder::default())?;
        let written = std::fs::read_to_string(folder.join("o.jsonl")).unwrap_or_default();
        Ok(written
            .lines()
            .filter_map(|line| loads(line).ok())
            .filter_map(|row| member(&row, "id").and_then(|id| id.as_plain_int().cloned()))
            .filter_map(|id| i64::try_from(id).ok())
            .collect())
    };
    check(
        "T5 salt 부재 = 종전 거동(런 0 = 행 0..9)",
        slice(0, "")? == (0..10).collect::<Vec<i64>>(),
        console,
    )?;
    let first = slice(0, "2026-10-17T00:00:00Z")?;
    let second = slice(1, "2026-10-17T00:00:00Z")?;
    let repeat = slice(0, "2026-10-17T00:00:00Z")?;
    check(
        "T6 같은 salt 의 런끼리 겹치지 않고 재현된다",
        !first.iter().any(|id| second.contains(id)) && first == repeat,
        console,
    )?;
    check(
        "T7 다른 salt(다른 스윕)는 다른 행에서 시작한다(prefix cache 재적중 회피)",
        first != slice(0, "2026-10-17T01:00:00Z")?,
        console,
    )?;
    drop(scratch);
    if !failures.is_empty() {
        let list = PyValue::List(failures.iter().map(|label| PyValue::str(label)).collect());
        console.error(&PyStr::new(&format!(
            "[dataset_prep] self-test FAIL {}\n",
            list.repr()
        )));
        return Ok(1);
    }
    console.print(&PyStr::new("[dataset_prep] self-test PASS — T1~T7"))?;
    Ok(0)
}
