//! The three commands of `dataset_prep.py` (`prepare`, `slice`, `synthetic-sidecar`) on an argparse-like namespace of
//! dynamically typed values. Every statement of the original is kept in its order, because the order decides which
//! exception is raised, which files already changed and what was already printed.

use crate::dataset_prep::{
    child_path, column_mapper_arg, data_arg, dict_get, die, dir_digest, ends_with_newline,
    load_spec, path_name, path_text, pretty_document, read_json, target_rows, text_lines, Console,
    TargetRows, PREP_META, PREP_NAME, PROMPT_COUNT_COL,
};
use crate::dataset_random::PythonRandom;
use crate::guidellm::Failure;
use crate::pydyn::{subscript, Text};
use crate::pyfs;
use crate::pyjson::loads_text;
use crate::pynumber::{float_from_int, int_call};
use crate::pysort::py_eq;
use crate::pystr::PyStr;
use crate::pyvalue::{PyDict, PyValue};
use num_bigint::BigInt;
use num_traits::{Signed, ToPrimitive, Zero};
use ring::digest::{digest, SHA256};
use std::ffi::OsString;
use std::os::unix::process::ExitStatusExt;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// The `argparse.Namespace` a command receives: attributes by name, each any JSON-like value.
pub struct Namespace<'a>(pub &'a PyDict);

impl<'a> Namespace<'a> {
    /// `a.<name>`.
    fn attr(&self, name: &str) -> Result<&'a PyValue, Failure> {
        self.0.get(name).ok_or_else(|| {
            Failure::new(
                "AttributeError",
                &format!("'Namespace' object has no attribute '{name}'"),
            )
        })
    }

    /// `int(a.<name>)`.
    fn int(&self, name: &str) -> Result<BigInt, Failure> {
        Ok(int_call(self.attr(name)?.peel())?)
    }

    /// `Path(a.<name>)` as text.
    fn path(&self, name: &str) -> Result<PyStr, Failure> {
        path_text(self.attr(name)?)
    }
}

fn int_value(number: &BigInt) -> PyValue {
    PyValue::Int(number.clone())
}

/// `str.join` of the lines of a selection.
fn join_lines(lines: &[&PyValue]) -> PyStr {
    let mut text = PyStr::default();
    for line in lines {
        if let PyValue::Str(line) = line {
            text.push_py(line);
        }
    }
    text
}

// ── synthetic-sidecar ───────────────────────────────────────────────────────────

/// `cmd_synthetic_sidecar(a)`: the provenance of a synthetic input (nothing was preprocessed).
pub fn cmd_synthetic_sidecar(
    a: &Namespace<'_>,
    _console: &mut dyn Console,
) -> Result<i32, Failure> {
    let mut side = PyDict::new();
    side.set("dataset_kind", PyValue::str("synthetic"));
    side.set("input_len", int_value(&a.int("input_len")?));
    side.set("output_len", int_value(&a.int("output_len")?));
    side.set("duplicate_sampling", PyValue::None);
    side.set(
        "provenance",
        PyValue::str("declared(GuideLLM synthetic_text · 원본 없음)"),
    );
    let sidecar = a.path("sidecar")?;
    let document = pretty_document(&PyValue::Dict(side))?;
    pyfs::write_text(&sidecar, &document)?;
    Ok(0)
}

// ── slice ───────────────────────────────────────────────────────────────────────

fn fixed(text: &str) -> PyStr {
    PyStr::new(text)
}

/// `cmd_slice(a)`: this run's rows from the prepared dataset, and the sidecar that records them.
pub fn cmd_slice(a: &Namespace<'_>, console: &mut dyn Console) -> Result<i32, Failure> {
    let spec = load_spec(a.attr("spec")?, console)?;
    let cache = a.path("cache")?;
    let meta = read_json(&child_path(&cache, &fixed(PREP_META)))?;
    // Only the target length's rows are used, and that length is the distribution's mode: a tokenizer that counts a
    // special token makes the strategy's rows one longer than requested.
    let prepared = text_lines(&child_path(&cache, &fixed(PREP_NAME)))?;
    let mut rows: Vec<PyValue> = Vec::new();
    for line in &prepared.lines {
        if line.is_blank() {
            continue;
        }
        let count = dict_get(
            &loads_text(line).map_err(|e| crate::repeat_api_cli::load_failure(&e))?,
            PROMPT_COUNT_COL,
        )?;
        let mut kept = line.clone();
        if !ends_with_newline(&kept) {
            kept.push_str("\n");
        }
        rows.push(PyValue::Tuple(vec![count, PyValue::Str(kept)]));
    }
    if let Some(failure) = prepared.failure {
        return Err(failure);
    }
    let selected = target_rows(&PyValue::List(rows), &int_value(&a.int("input_len")?))?;
    let (count, run_index) = (a.int("count")?, a.int("run_index")?);
    if !count.is_positive() || run_index.is_negative() {
        return Err(die(
            console,
            2,
            &fixed("--count 는 양수 · --run-index 는 0 이상이다"),
        ));
    }
    if selected.pool.is_empty() {
        let message = Text::new()
            .lit("input_len=")
            .val(a.attr("input_len")?)
            .lit(" 목표 길이 행이 없다")
            .done();
        return Err(die(console, 3, &message));
    }
    let available = BigInt::from(selected.pool.len());
    let start = &run_index * &count;
    // The sweep offset: a salt (the sweep's start time) moves the first row, so re-measuring a configuration does not
    // hit the prefix cache with the same prompts. The same salt gives the same rows; runs of one sweep still do not
    // overlap. No salt means offset 0.
    let salt = a.attr("sweep_salt")?;
    let base = if salt.is_truthy() {
        let PyValue::Str(text) = salt.peel() else {
            return Err(Failure::new(
                "AttributeError",
                &format!(
                    "'{}' object has no attribute 'encode'",
                    salt.peel().type_name()
                ),
            ));
        };
        let encoded = pyfs::strict_utf8(text)
            .map_err(|failure| Failure::new("UnicodeEncodeError", &failure.message()))?;
        let hash = digest(&SHA256, &encoded);
        let first: [u8; 8] = hash.as_ref()[..8]
            .try_into()
            .expect("a digest has 32 bytes");
        BigInt::from(u64::from_be_bytes(first)) % &available
    } else {
        BigInt::zero()
    };
    let (chosen, duplicate, how): (Vec<&PyValue>, bool, PyStr) = if &start + &count <= available {
        let rows_used = count.to_usize().expect("a count within the pool fits");
        let first = (&base + &start)
            .to_usize()
            .expect("an offset within the pool fits");
        let pool_len = selected.pool.len();
        let chosen = (0..rows_used)
            .map(|offset| &selected.pool[(first + offset) % pool_len])
            .collect();
        let last = &start + &count - 1;
        let how = if base.is_zero() {
            PyStr::new(&format!(
                "contiguous(rows {start}..{last} · 다른 런과 겹치지 않음)"
            ))
        } else {
            Text::new()
                .lit(&format!(
                    "contiguous(offset {base} + rows {start}..{last} mod {available} · 스윕 salt "
                ))
                .val(salt)
                .lit(" · 다른 런과 겹치지 않음)")
                .done()
        };
        (chosen, false, how)
    } else {
        let seed = subscript(&spec, "seed")?;
        let label = Text::new().val(seed).lit(&format!(":{run_index}")).done();
        let mut generator = PythonRandom::from_text(&label)
            .map_err(|failure| Failure::new("UnicodeEncodeError", &failure.message()))?;
        let rows_used = count
            .to_usize()
            .ok_or_else(|| Failure::new("MemoryError", ""))?;
        let pool_len = u64::try_from(selected.pool.len()).expect("a pool length fits 64 bits");
        let mut chosen: Vec<&PyValue> = Vec::with_capacity(rows_used.min(1 << 20));
        for _ in 0..rows_used {
            let at = usize::try_from(generator.randbelow(pool_len)).expect("an index fits");
            chosen.push(&selected.pool[at]);
        }
        let needed = &start + &count;
        let how = Text::new()
            .lit("random_with_replacement(seed=")
            .val(seed)
            .lit(&format!(":{run_index} · 행 {available} < 필요 {needed})"))
            .done();
        (chosen, true, how)
    };
    let run = SliceRun {
        spec: &spec,
        meta: &meta,
        selected: &selected,
        run_index: &run_index,
        count: &count,
        base: &base,
    };
    finish_slice(a, console, &run, &chosen, duplicate, how)
}

/// What `cmd_slice` knows when it starts writing.
struct SliceRun<'a> {
    spec: &'a PyValue,
    meta: &'a PyValue,
    selected: &'a TargetRows,
    run_index: &'a BigInt,
    count: &'a BigInt,
    base: &'a BigInt,
}

/// The tail of `cmd_slice`: write the rows, then the sidecar, then report.
fn finish_slice(
    a: &Namespace<'_>,
    console: &mut dyn Console,
    run: &SliceRun<'_>,
    chosen: &[&PyValue],
    duplicate: bool,
    how: PyStr,
) -> Result<i32, Failure> {
    let SliceRun {
        spec,
        meta,
        selected,
        run_index,
        count,
        base,
    } = *run;
    let out = a.path("out")?;
    pyfs::write_text(&out, &join_lines(chosen))?;
    let mut side = PyDict::new();
    for key in [
        "dataset_kind",
        "dataset_source",
        "dataset_id",
        "dataset_path",
        "dataset_revision",
        "source_sha256",
        "column_map",
        "preprocess_strategy",
        "seed",
    ] {
        side.set(key, dict_get(spec, key)?);
    }
    // The dictionary display of the original, evaluated in its order (a failure leaves `side` unused, as there).
    side.set("input_len", int_value(&a.int("input_len")?));
    side.set("output_len", int_value(&a.int("output_len")?));
    side.set("run_index", int_value(run_index));
    side.set("rows_used", int_value(count));
    side.set("rows_available", PyValue::int(selected.pool.len()));
    side.set("rows_prepared", dict_get(meta, "rows")?);
    side.set("selection", PyValue::Str(how.clone()));
    side.set("prompt_tokens_count_target", selected.target.clone());
    let salt = a.attr("sweep_salt")?;
    side.set(
        "sweep_salt",
        if salt.is_truthy() {
            salt.clone()
        } else {
            PyValue::None
        },
    );
    side.set("sweep_offset", int_value(base));
    side.set("duplicate_sampling", PyValue::Bool(duplicate));
    side.set("tokenizer", a.attr("tokenizer_label")?.clone());
    side.set(
        "tokenizer_digest",
        subscript(subscript(meta, "key")?, "tokenizer_digest")?.clone(),
    );
    side.set(
        "provenance",
        PyValue::str("measured(run_bench 가 이 런에 넘긴 행 · dataset_prep slice)"),
    );
    let sidecar = a.path("sidecar")?;
    let document = pretty_document(&PyValue::Dict(side))?;
    pyfs::write_text(&sidecar, &document)?;
    let line = Text::new()
        .lit(&format!("[dataset_prep] 런 {run_index}: {count} 행 · "))
        .str(&how)
        .done();
    console.print(&line)?;
    Ok(0)
}

// ── prepare ─────────────────────────────────────────────────────────────────────

/// How the preprocessing container ended.
enum Finished {
    Exit(i64),
    TimedOut,
}

/// `os.fsencode(argument)` for one element of a command line.
fn exec_argument(value: &PyValue) -> Result<OsString, Failure> {
    match value.peel() {
        PyValue::Str(text) => pyfs::os_string(text).map_err(pyfs::FsFailure::into_failure),
        other => Err(Failure::new(
            "TypeError",
            &format!(
                "expected str, bytes or os.PathLike object, not {}",
                other.type_name()
            ),
        )),
    }
}

fn spawn_failure(error: &std::io::Error, program: &OsString) -> Failure {
    let name =
        PyStr::from_surrogateescape(std::os::unix::ffi::OsStrExt::as_bytes(program.as_os_str()));
    Failure::new(
        pyfs::os_error_class(error),
        &crate::cli::os_error_text(error, &name),
    )
}

/// `subprocess.run(command, timeout=int(a.timeout_s)).returncode`: the child shares this process's input and
/// output; after the timeout it is killed and reaped. The wait polls the way `Popen._wait` does.
fn run_command(command: &[PyValue], a: &Namespace<'_>) -> Result<Finished, Failure> {
    let timeout = a.int("timeout_s")?;
    let arguments: Vec<OsString> = command
        .iter()
        .map(exec_argument)
        .collect::<Result<_, _>>()?;
    let seconds = float_from_int(&timeout)?;
    let mut child = Command::new(&arguments[0])
        .args(&arguments[1..])
        .spawn()
        .map_err(|error| spawn_failure(&error, &arguments[0]))?;
    let deadline = if seconds <= 0.0 {
        Some(Instant::now())
    } else {
        Duration::try_from_secs_f64(seconds)
            .ok()
            .and_then(|span| Instant::now().checked_add(span))
    };
    let mut delay = 0.0005_f64;
    loop {
        if let Ok(Some(status)) = child.try_wait() {
            let code = status
                .code()
                .map(i64::from)
                .or_else(|| status.signal().map(|signal| -i64::from(signal)))
                .unwrap_or(-1);
            return Ok(Finished::Exit(code));
        }
        let remaining =
            deadline.map(|end| end.saturating_duration_since(Instant::now()).as_secs_f64());
        if remaining.is_some_and(|left| left <= 0.0) {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(Finished::TimedOut);
        }
        delay = (delay * 2.0).min(0.05);
        if let Some(left) = remaining {
            delay = delay.min(left);
        }
        std::thread::sleep(Duration::from_secs_f64(delay));
    }
}

/// `subprocess.run(["docker", "rm", "-f", name], capture_output=True)`: the container is removed whatever happened;
/// the return code is not looked at.
fn remove_container(name: &str) -> Result<(), Failure> {
    let program = OsString::from("docker");
    Command::new(&program)
        .args(["rm", "-f", name])
        .stdin(Stdio::inherit())
        .output()
        .map(|_| ())
        .map_err(|error| spawn_failure(&error, &program))
}

/// `cmd_prepare(a)`: runs `guidellm preprocess dataset` in an offline container unless the cache already holds the
/// result for exactly this source, tokenizer, lengths, strategy, seed, columns and image.
pub fn cmd_prepare(a: &Namespace<'_>, console: &mut dyn Console) -> Result<i32, Failure> {
    let spec = load_spec(a.attr("spec")?, console)?;
    let kind = subscript(&spec, "dataset_kind")?;
    if py_eq(kind, &PyValue::str("synthetic")) {
        return Err(die(
            console,
            2,
            &fixed("synthetic 은 전처리가 없다(호출부 결함)"),
        ));
    }
    let raw = path_text(subscript(&spec, "dataset_path")?)?;
    let tok = a.path("tokstage")?;
    let cache = a.path("cache")?;
    if !pyfs::is_file(&raw)? {
        return Err(die(
            console,
            2,
            &Text::new().lit("원본이 없다: ").str(&raw).done(),
        ));
    }
    if !pyfs::is_dir(&tok)? {
        let message = Text::new()
            .lit("토크나이저 스테이징 폴더가 없다: ")
            .str(&tok)
            .done();
        return Err(die(console, 2, &message));
    }
    // The key holds every input of the result. The tokenizer folder is hashed file by file in full each time.
    let mut key = PyDict::new();
    key.set("source_sha256", subscript(&spec, "source_sha256")?.clone());
    key.set("dataset_kind", kind.clone());
    key.set("column_map", subscript(&spec, "column_map")?.clone());
    key.set(
        "preprocess_strategy",
        subscript(&spec, "preprocess_strategy")?.clone(),
    );
    key.set("seed", subscript(&spec, "seed")?.clone());
    key.set("input_len", int_value(&a.int("input_len")?));
    key.set("output_len", int_value(&a.int("output_len")?));
    key.set(
        "tokenizer_digest",
        PyValue::str(&dir_digest(&PyValue::Str(tok.clone()))?),
    );
    key.set("image", a.attr("image")?.clone());
    let meta_path = child_path(&cache, &fixed(PREP_META));
    let out_path = child_path(&cache, &fixed(PREP_NAME));
    if pyfs::is_file(&meta_path)? && pyfs::is_file(&out_path)? {
        match read_json(&meta_path) {
            Ok(meta) => {
                if py_eq(&dict_get(&meta, "key")?, &PyValue::Dict(key.clone())) {
                    let line = Text::new()
                        .lit("[dataset_prep] 캐시 재사용 — ")
                        .str(&out_path)
                        .done();
                    console.print(&line)?;
                    return Ok(0);
                }
            }
            // `except ValueError: pass` - an unreadable file is an `OSError` and is not caught.
            Err(failure)
                if matches!(
                    failure.class,
                    "ValueError" | "UnicodeDecodeError" | "UnicodeEncodeError" | "JSONDecodeError"
                ) => {}
            Err(failure) => return Err(failure),
        }
    }
    pyfs::mkdir_parents_exist_ok(&cache)?;
    for path in [&meta_path, &out_path] {
        // `unlink(missing_ok=True)`
        if let Err(failure) = pyfs::unlink(path) {
            if failure.class != "FileNotFoundError" {
                return Err(failure);
            }
        }
    }
    let name = format!("guidellm-preprocess-{}", std::process::id());
    let (uid, gid) = (
        rustix::process::getuid().as_raw(),
        rustix::process::getgid().as_raw(),
    );
    let text = |parts: Text| PyValue::Str(parts.done());
    let budget = a.attr("budget_mib")?;
    let memory = |_: ()| text(Text::new().val(budget).lit("m"));
    let mut command: Vec<PyValue> = Vec::new();
    for item in ["docker", "run", "--rm", "--name"] {
        command.push(PyValue::str(item));
    }
    command.push(PyValue::str(&name));
    for item in ["--network", "none", "--user"] {
        command.push(PyValue::str(item));
    }
    command.push(PyValue::str(&format!("{uid}:{gid}")));
    command.push(PyValue::str("--memory"));
    command.push(memory(()));
    command.push(PyValue::str("--memory-swap"));
    command.push(memory(()));
    for item in [
        "-e",
        "HOME=/tmp",
        "-e",
        "XDG_CACHE_HOME=/tmp/.cache",
        "-e",
        "HF_HUB_OFFLINE=1",
        "-e",
        "TRANSFORMERS_OFFLINE=1",
        "-v",
    ] {
        command.push(PyValue::str(item));
    }
    command.push(text(Text::new().str(&pyfs::parent(&raw)).lit(":/data:ro")));
    command.push(PyValue::str("-v"));
    command.push(text(Text::new().str(&tok).lit(":/tok:ro")));
    command.push(PyValue::str("-v"));
    command.push(text(Text::new().str(&cache).lit(":/out")));
    command.push(PyValue::str("--entrypoint"));
    command.push(PyValue::str("guidellm"));
    command.push(a.attr("image")?.clone());
    command.push(PyValue::str("preprocess"));
    command.push(PyValue::str("dataset"));
    let data_path = Text::new().lit("/data/").str(&path_name(&raw)).done();
    command.push(PyValue::Str(data_arg(kind, &PyValue::Str(data_path))?));
    command.push(PyValue::str(&format!("/out/{PREP_NAME}")));
    command.push(PyValue::str("--tokenizer"));
    command.push(PyValue::str("kind=huggingface_auto,model=/tok"));
    command.push(PyValue::str("--strategy"));
    let strategy = subscript(&spec, "preprocess_strategy")?;
    let (input_len, output_len) = (a.int("input_len")?, a.int("output_len")?);
    command.push(text(Text::new().lit("kind=").val(strategy).lit(&format!(
        ",prompt_tokens={input_len},output_tokens={output_len}"
    ))));
    command.push(PyValue::str("--data-column-mapper"));
    command.push(PyValue::Str(column_mapper_arg(
        &spec,
        &PyValue::Bool(false),
    )?));
    command.push(PyValue::str("--seed"));
    let seed = int_call(subscript(&spec, "seed")?.peel())?;
    command.push(PyValue::str(&format!("kind=static,value={seed}")));
    let banner = Text::new()
        .lit("[dataset_prep] 전처리(오프라인 · ")
        .val(a.attr("image")?)
        .lit(" · budget ")
        .val(budget)
        .lit("MiB) → ")
        .str(&out_path)
        .done();
    console.print(&banner)?;
    // try / except TimeoutExpired / finally, in the original's order: the container is removed after a timeout and
    // again in the `finally`, and a failure of the last removal replaces whatever was pending.
    let mut pending: Result<i64, Failure> = match run_command(&command, a) {
        Ok(Finished::Exit(code)) => Ok(code),
        Ok(Finished::TimedOut) => match remove_container(&name) {
            Ok(()) => {
                let message = Text::new()
                    .lit("전처리가 ")
                    .val(a.attr("timeout_s")?)
                    .lit("s 안에 끝나지 않았다 — 측정을 시작하지 않는다")
                    .done();
                Err(die(console, 3, &message))
            }
            Err(failure) => Err(failure),
        },
        Err(failure) => Err(failure),
    };
    if let Err(failure) = remove_container(&name) {
        pending = Err(failure);
    }
    let code = pending?;
    if code != 0 || !pyfs::is_file(&out_path)? {
        let message = Text::new()
            .lit(&format!(
                "guidellm preprocess 실패(rc={code}) — 합성 입력으로 대체하지 않는다"
            ))
            .done();
        return Err(die(console, 3, &message));
    }
    let produced = text_lines(&out_path)?;
    let (mut rows, mut exact) = (0_u64, 0_u64);
    for line in &produced.lines {
        if line.is_blank() {
            continue;
        }
        rows += 1;
        let count = dict_get(
            &loads_text(line).map_err(|error| crate::repeat_api_cli::load_failure(&error))?,
            PROMPT_COUNT_COL,
        )?;
        if py_eq(&count, &int_value(&a.int("input_len")?)) {
            exact += 1;
        }
    }
    if let Some(failure) = produced.failure {
        return Err(failure);
    }
    if rows == 0 {
        return Err(die(
            console,
            3,
            &Text::new()
                .lit("전처리 결과가 비었다: ")
                .str(&out_path)
                .done(),
        ));
    }
    let mut meta = PyDict::new();
    meta.set("key", PyValue::Dict(key));
    meta.set("rows", PyValue::int(rows));
    meta.set("rows_exact_input_len", PyValue::int(exact));
    meta.set(
        "provenance",
        PyValue::str("measured(guidellm preprocess dataset · 오프라인 컨테이너)"),
    );
    let document = pretty_document(&PyValue::Dict(meta))?;
    pyfs::write_text(&meta_path, &document)?;
    console.print(&PyStr::new(&format!(
        "[dataset_prep] 전처리 완료 — 행 {rows} (input_len 정확 {exact})"
    )))?;
    Ok(0)
}
