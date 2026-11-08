//! The two command lines (`parse_model_config.py`, `crosscheck_model_card.py`) as functions from arguments to status and
//! stream text. Program names in usage and help are the original `.py` names. An exception nothing handles ends the run with the
//! whole `Class: message` block of Python's report on standard error (the interpreter's frames are not reproduced), status 1.

use crate::cli::Run;
use crate::cliparse::{
    exit_with, parse as parse_arguments, terminal_width, Kind, Opt, Outcome, Positional, Spec,
    Value,
};
use crate::model_card::{self, with_commas, Fetch, Origin, HF_API_BASE};
use crate::model_config::{self, DEFAULT_NAS_HOST_ROOT};
use crate::model_dyn::{is_os_error, is_value_error, Message};
use crate::model_selftest;
use crate::pyexpr::Res;
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use std::collections::BTreeMap;

/// What the program printed and ended with, built the way `print` and `sys.stderr.write` produce it.
#[derive(Default)]
pub struct Streams {
    pub stdout: PyStr,
    pub writes: Vec<PyStr>,
    pub stderr: PyStr,
}

impl Streams {
    /// `print(text)`: the text and then its newline, as two writes.
    pub fn print(&mut self, text: &PyStr) {
        self.stdout.push_py(text);
        self.stdout.push_str("\n");
        self.writes.push(text.clone());
        self.writes.push(PyStr::new("\n"));
    }

    /// `sys.stdout.write(text)`.
    pub fn write(&mut self, text: &PyStr) {
        self.stdout.push_py(text);
        self.writes.push(text.clone());
    }

    /// `print(text, file=sys.stderr)`.
    pub fn eprint(&mut self, text: &PyStr) {
        self.stderr.push_py(text);
        self.stderr.push_str("\n");
    }

    pub fn finish(self, status: i32) -> Run {
        Run {
            status,
            stdout: self.stdout,
            stderr: self.stderr,
            writes: self.writes,
        }
    }
}

/// The name Python prints for an exception class in a traceback.
fn qualified(class: &str) -> &str {
    match class {
        "JSONDecodeError" => "json.decoder.JSONDecodeError",
        "error" => "struct.error",
        "InvalidURL" => "http.client.InvalidURL",
        "IncompleteRead" => "http.client.IncompleteRead",
        other => other,
    }
}

/// The end of a run that raised an exception nothing handles: the complete final `Class: message` block of Python's report,
/// whatever the message holds (several lines included), qualified as the interpreter qualifies it. The interpreter's own
/// frames are not reproduced and no stand-in line is written for them; the rest of the report is the standard error as it is.
pub fn uncaught(mut streams: Streams, failure: &Failure) -> Run {
    streams.stderr.push_str(qualified(failure.class));
    if !failure.message.is_empty() {
        streams.stderr.push_str(": ");
        streams.stderr.push_py(&failure.message);
    }
    streams.stderr.push_str("\n");
    streams.finish(1)
}

fn opt(
    flags: &'static [&'static str],
    dest: &'static str,
    kind: Kind,
    help: Option<&'static str>,
) -> Opt {
    Opt {
        flags,
        dest,
        kind,
        help,
        required: false,
        group: None,
    }
}

fn exited(outcome: Outcome) -> Result<BTreeMap<&'static str, Value>, Run> {
    match outcome {
        Outcome::Values(values) => Ok(values),
        Outcome::Exit {
            status,
            stdout,
            stderr,
        } => {
            let writes = if stdout.is_empty() {
                Vec::new()
            } else {
                vec![stdout.clone()]
            };
            Err(Run {
                status,
                stdout,
                stderr,
                writes,
            })
        }
    }
}

fn text_value(values: &BTreeMap<&'static str, Value>, dest: &str) -> Option<PyStr> {
    match values.get(dest) {
        Some(Value::Text(text)) => Some(text.clone()),
        _ => None,
    }
}

fn flag(values: &BTreeMap<&'static str, Value>, dest: &str) -> bool {
    matches!(values.get(dest), Some(Value::Flag))
}

fn dumps(value: &PyValue) -> Res<PyStr> {
    value.dumps_pretty().map_err(|error| {
        let text = error.to_string();
        let prefix = format!("{}: ", error.python_type());
        Failure::new(
            error.python_type(),
            text.strip_prefix(&prefix).unwrap_or(&text),
        )
    })
}

// ── parse_model_config ──────────────────────────────────────────────────────────────────────────

fn parse_spec() -> Spec {
    Spec {
        prog: "parse_model_config.py",
        description: "config.json 결정론 파서 (vllm-recipe-explorer)",
        options: vec![
            opt(&["-h", "--help"], "help", Kind::Help, None),
            opt(
                &["--self-test"],
                "self_test",
                Kind::Flag,
                Some("하이브리드 KV·MXFP4 헤더/교차검증 회귀(모델/하드웨어/네트워크 불요)"),
            ),
            opt(
                &["--nas-root"],
                "nas_root",
                Kind::Text,
                Some("/app/models 매핑 호스트 NAS 루트 (기본 /mnt/models)"),
            ),
            opt(&["--json"], "json", Kind::Flag, Some("JSON 으로 출력")),
        ],
        group_required: false,
        positional: Some(Positional {
            dest: "path",
            help: Some("모델 경로 (/app/models/<Org>/<Name> 또는 호스트 경로)"),
        }),
    }
}

/// `parse_model_config.py ...`: `_main(argv)`.
pub fn parse_main(args: &[PyStr], columns: Option<&str>) -> Run {
    let spec = parse_spec();
    let values = match exited(parse_arguments(&spec, args, columns)) {
        Ok(values) => values,
        Err(run) => return run,
    };
    if flag(&values, "self_test") {
        return model_selftest::parse_self_test();
    }
    let path = text_value(&values, "path").filter(|path| !path.is_empty());
    let Some(path) = path else {
        let outcome = exit_with(
            &spec,
            terminal_width(columns),
            "path 는 필수다(--self-test 는 예외)",
        );
        return exited(outcome).expect_err("an error exit");
    };
    let mut streams = Streams::default();
    let nas_root =
        text_value(&values, "nas_root").unwrap_or_else(|| PyStr::new(DEFAULT_NAS_HOST_ROOT));
    let result = model_config::parse(
        &PyValue::Str(path),
        &PyValue::Str(nas_root),
        &PyValue::str("/app/models"),
    );
    let result = match result {
        Ok(result) => result,
        Err(failure) if failure.class == "FileNotFoundError" => {
            streams.eprint(
                &Message::new()
                    .lit("[parse_model_config] 중단: ")
                    .text(&failure.message)
                    .done(),
            );
            return streams.finish(2);
        }
        Err(failure) if is_value_error(failure.class) => {
            streams.eprint(
                &Message::new()
                    .lit("[parse_model_config] 중단(해소 실패): ")
                    .text(&failure.message)
                    .done(),
            );
            return streams.finish(2);
        }
        Err(failure) => return uncaught(streams, &failure),
    };
    let report = PyValue::Dict(result);
    if flag(&values, "json") {
        match dumps(&report) {
            Ok(text) => streams.print(&text),
            Err(failure) => return uncaught(streams, &failure),
        }
    } else if let PyValue::Dict(result) = &report {
        for (key, value) in result.iter() {
            if key.equals("warnings") {
                continue;
            }
            let mut line = key.clone();
            while line.len() < 28 {
                line.push_str(" ");
            }
            line.push_str(" ");
            line.push_py(&value.display_text());
            streams.print(&line);
        }
        if let Some(PyValue::List(warnings)) = result.get("warnings") {
            if !warnings.is_empty() {
                streams.print(&PyStr::new("--- warnings ---"));
                for warning in warnings {
                    streams.print(&Message::new().lit("  ! ").val(warning).done());
                }
            }
        }
    }
    streams.finish(0)
}

// ── crosscheck_model_card ───────────────────────────────────────────────────────────────────────

fn crosscheck_spec() -> Spec {
    Spec {
        prog: "crosscheck_model_card.py",
        description: "HF 원본 모델카드(번들 README) 교차검증 — config 단독파싱 함정 차단(결정론).",
        options: vec![
            opt(&["-h", "--help"], "help", Kind::Help, None),
            opt(
                &["--hf-repo-id"],
                "hf_repo_id",
                Kind::Text,
                Some("HF repo id(예 google/gemma-3-1b-it) — managed: 로컬 실측 이중검증. --ephemeral-estimate 와 함께 쓰면 그게 곧 repo_id."),
            ),
            opt(
                &["--ephemeral-estimate"],
                "ephemeral_estimate",
                Kind::Flag,
                Some("로컬 디렉토리 없이(다운로드 전) --hf-repo-id 만으로 외부 파라미터 총계 사전추정."),
            ),
            opt(&["--json"], "json", Kind::Flag, Some("JSON 출력")),
        ],
        group_required: false,
        positional: Some(Positional {
            dest: "path",
            help: Some("모델 디렉토리(호스트 경로 또는 /app/models/<Org>/<Name>) — managed 모드."),
        }),
    }
}

/// What the cross-check command line depends on: how `parse_model_config.parse(path)` is run and how the external total
/// is fetched. The real program uses the real ones; a self-test supplies stand-ins.
pub struct Dependencies<'a> {
    pub parse: &'a dyn Fn(&PyValue) -> Res<PyDict>,
    pub fetch: Fetch<'a>,
}

/// `crosscheck_model_card.py ...`: `main()`.
pub fn crosscheck_main(
    args: &[PyStr],
    columns: Option<&str>,
    dependencies: &Dependencies<'_>,
) -> Run {
    let spec = crosscheck_spec();
    let values = match exited(parse_arguments(&spec, args, columns)) {
        Ok(values) => values,
        Err(run) => return run,
    };
    let mut streams = Streams::default();
    let as_json = flag(&values, "json");
    let repo = text_value(&values, "hf_repo_id");
    let repo_value = repo.clone().map_or(PyValue::None, PyValue::Str);
    if flag(&values, "ephemeral_estimate") {
        if !repo.as_ref().is_some_and(|repo| !repo.is_empty()) {
            streams.eprint(&PyStr::new(
                "[crosscheck] STOP: --ephemeral-estimate 는 --hf-repo-id 필수",
            ));
            return streams.finish(3);
        }
        let report = match model_card::crosscheck_external_vram(
            &PyValue::None,
            &repo_value,
            &PyValue::Float(0.05),
            dependencies.fetch,
        ) {
            Ok(report) => report,
            Err(failure) => return uncaught(streams, &failure),
        };
        let verdict = report.get("verdict").cloned().unwrap_or(PyValue::None);
        let verdict_is =
            |text: &str| matches!(verdict.peel(), PyValue::Str(own) if own.equals(text));
        if as_json {
            match dumps(&PyValue::Dict(report.clone())) {
                Ok(text) => streams.print(&text),
                Err(failure) => return uncaught(streams, &failure),
            }
        } else {
            streams.print(
                &Message::new()
                    .lit("# ephemeral 사전추정: ")
                    .val(&repo_value)
                    .lit(" → ")
                    .val(&verdict)
                    .done(),
            );
            if verdict_is("OK") {
                let total =
                    match with_commas(report.get("external_total").unwrap_or(&PyValue::None)) {
                        Ok(total) => total,
                        Err(failure) => return uncaught(streams, &failure),
                    };
                streams.print(
                    &Message::new()
                        .lit("  파라미터 총계: ")
                        .text(&total)
                        .lit(" · 예상 가중치: ")
                        .val(
                            report
                                .get("external_native_weight_gib_est")
                                .unwrap_or(&PyValue::None),
                        )
                        .lit(" GiB(dtype 혼합 시 근사)")
                        .done(),
                );
            } else {
                streams.print(
                    &Message::new()
                        .lit("  조회 실패: ")
                        .val(report.get("reason").unwrap_or(&PyValue::None))
                        .done(),
                );
            }
        }
        return streams.finish(if verdict_is("UNAVAILABLE") { 4 } else { 0 });
    }
    let path = text_value(&values, "path").filter(|path| !path.is_empty());
    let Some(path) = path else {
        streams.eprint(&PyStr::new(
            "[crosscheck] STOP: managed 모드는 모델 디렉토리 경로 필수(또는 --ephemeral-estimate)",
        ));
        return streams.finish(3);
    };
    let path_value = PyValue::Str(path);
    let (mut local_num_params, mut local_parse_error) = (PyValue::None, PyValue::None);
    if repo.as_ref().is_some_and(|repo| !repo.is_empty()) {
        match (dependencies.parse)(&path_value) {
            Ok(parsed) => {
                local_num_params = parsed.get("num_params").cloned().unwrap_or(PyValue::None)
            }
            Err(failure) => {
                local_parse_error = PyValue::Str(failure.message.clone());
                streams.eprint(
                    &Message::new()
                        .lit("[crosscheck] STOP: num_params 재측정 실패(")
                        .text(&failure.message)
                        .lit(") — managed 대조 불성립")
                        .done(),
                );
            }
        }
    }
    let report = match model_card::crosscheck(
        &path_value,
        &repo_value,
        &local_num_params,
        &local_parse_error,
        dependencies.fetch,
    ) {
        Ok(report) => report,
        Err(failure) if failure.class == "FileNotFoundError" => {
            streams.eprint(
                &Message::new()
                    .lit("[crosscheck] STOP: ")
                    .text(&failure.message)
                    .done(),
            );
            return streams.finish(3);
        }
        Err(failure) => return uncaught(streams, &failure),
    };
    let report = PyValue::Dict(report);
    let rendered = if as_json {
        dumps(&report)
    } else {
        model_card::human(&report)
    };
    match rendered {
        Ok(text) => streams.print(&text),
        Err(failure) => return uncaught(streams, &failure),
    }
    let mismatch =
        matches!(report_verdict(&report).peel(), PyValue::Str(own) if own.equals("MISMATCH"));
    streams.finish(if mismatch { 2 } else { 0 })
}

fn report_verdict(report: &PyValue) -> PyValue {
    match report {
        PyValue::Dict(map) => map.get("verdict").cloned().unwrap_or(PyValue::None),
        _ => PyValue::None,
    }
}

/// The real dependencies: `parse_model_config.parse(path)` with its defaults and the HF lookup at the default base.
pub fn crosscheck_main_real(args: &[PyStr], columns: Option<&str>) -> Run {
    let parse = |path: &PyValue| {
        model_config::parse(
            path,
            &PyValue::str(DEFAULT_NAS_HOST_ROOT),
            &PyValue::str("/app/models"),
        )
    };
    let fetch = |repo: &PyValue| {
        model_card::fetch_hf_safetensors_total(
            repo,
            &PyValue::Float(8.0),
            &Origin {
                api_base: HF_API_BASE,
            },
        )
    };
    crosscheck_main(
        args,
        columns,
        &Dependencies {
            parse: &parse,
            fetch: &fetch,
        },
    )
}

/// An operating-system or value failure the model scripts catch by class.
pub fn is_caught_class(class: &str) -> bool {
    is_os_error(class) || is_value_error(class)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(class: &'static str, message: &str) -> (i32, String, String) {
        let run = uncaught(Streams::default(), &Failure::new(class, message));
        (
            run.status,
            run.stdout.to_utf8().expect("text"),
            run.stderr.to_utf8().expect("text"),
        )
    }

    #[test]
    fn the_report_of_an_unhandled_exception_is_only_its_final_qualified_block() {
        // The message is kept whole, several lines included, and nothing but the block is written.
        assert_eq!(
            report("ValueError", "first line\n  second line\nthird"),
            (
                1,
                String::new(),
                "ValueError: first line\n  second line\nthird\n".to_string()
            )
        );
        // An empty message prints the class alone, as the interpreter does.
        assert_eq!(
            report("KeyboardInterrupt", ""),
            (1, String::new(), "KeyboardInterrupt\n".to_string())
        );
        // Classes the interpreter qualifies by module.
        assert_eq!(
            report(
                "JSONDecodeError",
                "Expecting value: line 1 column 1 (char 0)"
            )
            .2,
            "json.decoder.JSONDecodeError: Expecting value: line 1 column 1 (char 0)\n"
        );
        assert_eq!(
            report("error", "unpack requires a buffer of 8 bytes").2,
            "struct.error: unpack requires a buffer of 8 bytes\n"
        );
        assert_eq!(report("InvalidURL", "x").2, "http.client.InvalidURL: x\n");
        assert_eq!(
            report("IncompleteRead", "IncompleteRead(1 bytes read)").2,
            "http.client.IncompleteRead: IncompleteRead(1 bytes read)\n"
        );
        // Builtin classes keep their bare names.
        assert_eq!(
            report("AttributeError", "'list' object has no attribute 'get'").2,
            "AttributeError: 'list' object has no attribute 'get'\n"
        );
        // No header and no stand-in line for the frames.
        assert!(!report("OverflowError", "x").2.contains("Traceback"));
    }
}
