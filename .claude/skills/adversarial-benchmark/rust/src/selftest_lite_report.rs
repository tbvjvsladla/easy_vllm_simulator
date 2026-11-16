//! `selftest_lite_report`: the isolated check of the **report plane** of the lite hint path (plan_26091407 §4.5 and
//! §7 O5). A Rust port of `selftest_lite_report.py`.
//!
//! A cell measured only by lite still produces a hint, so the lite run has to produce the document the hint binds
//! to. The deployed `lite_bench.sh` is run as byte-for-byte copies in an isolated git checkout and the check asks
//! whether its closing part publishes the light report and whether that report keeps the publisher's contract
//! (measurement configuration table, lite metric table, `mode: lite`). A unit test of the functions would not show
//! that the shell wiring stands on the real path.
//!
//! Isolation (an honest test-plane mock, never shaped like a measurement): `docker`, `curl` and `nvidia-smi` are shims
//! ahead of `PATH` that return fixture results instead of running `vllm bench serve`; the Terraforming flag gate is a
//! sentinel-marked stub in the copy's place (the gate is not the subject; its own regression guards it); no GPU, no
//! serving, no NAS, no network, and the real `docs/benchmark`, `campaigns` and tags are never touched.
//!
//! The native owners of the typed pieces (report identity and constants, naming, bench-mode vocabulary) are called as
//! this crate's own libraries. The owners of the other skills (hint reader, evidence publisher) are their Rust
//! libraries, reached through [`crate::owner_bridge`]; no Python is started.
//!
//! Cases: L1 default (no flag) = automatic handoff path, nothing published · L2 `--publish-report` · L3 idempotent
//! re-render · L4 publication failure does not lose the measurement · L5 no measurement time · L6 mode mix and missing
//! arguments · L7 a full sweep's lite leg does not publish · L8 naming key cross-check · L9 measurement configuration
//! table of the full report · L10 contract constants between the two skills.

use crate::bench_naming::{bench_filename, measurement_ts};
use crate::bench_verify_support::{
    dict_of, float, get_eq, get_eq_text, int, is_none, py_lstrip, py_str, py_strip, st,
    starts_with, tuple_text, BracketChecks, DetailStyle,
};
use crate::captured_process::ChildSettings;
use crate::classify_cell::{
    bench_mode_kind, BENCH_MODE_LITE, BENCH_MODE_RECORD_NAME, BENCH_MODE_SOURCE_DECLARED_PREFIX,
};
use crate::conformance::{py_equals, splitlines, Raised};
use crate::dataset_prep::Console;
use crate::owner_bridge::OwnerBridge;
use crate::pystr::{is_decimal_digit, PyStr};
use crate::pyvalue::{PyDict, PyValue};
use crate::render_report::{
    lite_identity, Environment, LITE_BENCH_TOOL, MEASUREMENT_CONFIG_KEYS, MEASUREMENT_CONFIG_TITLE,
};
use crate::selftest_sweep_meta::{engine_log_line, BuildMeta, SweepMeta};
use crate::verify_fixture::{
    chmod, contains, copy2, dumps, get, is_file, or_default, read_text, repo_root_unknown_message,
    run_checked_inherited, run_text_within, tail, unlink_if_present, which, words, write_str,
    write_text, TemporaryDirectory, TextRun, BENCHMARK_SCRIPTS,
};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The program's name in its messages.
pub const PROGRAM: &str = "selftest_lite_report";

/// The script directory of the benchmark skill, relative to a repository root (`AB` of the original).
pub(crate) const AB: &str = BENCHMARK_SCRIPTS;
/// The configuration the fixtures serve.
pub(crate) const CFG: &str = "fx-lite";
/// `SHIM_SENTINEL`: what the shims and the gate stub say they are.
const SHIM_SENTINEL: &str = "selftest-lite-report-shim";
const CHILD_SECONDS: u64 = 120;
const FULL_RENDER_SECONDS: u64 = 60;
const RELEASE_DIRECTORY: &str = ".claude/skills/adversarial-benchmark/rust/target/release";

const GATE_STUB: &str = ".claude/skills/terraforming_node/scripts/manifest_contract.py";
const GATE_LAUNCHER_STUB: &str = ".claude/skills/terraforming_node/scripts/manifest_contract.sh";
/// Test-plane launcher stub of the Flag gate (a shell program; nothing in the sandbox runs Python): the availability
/// check passes and every gate call passes. The callers also require the owner file beside it to exist
/// ([`GATE_STUB`] is only that presence marker; nothing executes it).
const GATE_LAUNCHER_TEXT: &str = "#!/bin/bash\n# selftest-lite-report-shim — 테라포밍 Flag 게이트 stub(이 시험의 대상이 아니다 · 실 게이트는 benchmark_info_only_gate 가 지킨다).\n[ \"${MANIFEST_CONTRACT_VALIDATE_ONLY:-}\" = 1 ] && exit 0\nexit 0\n";

/// Scripts copied as text (read as UTF-8, written back) in the original's order.
const COPIED_SCRIPTS: [&str; 14] = [
    "bench_shell.sh",
    "render_report.sh",
    "render_sweep_map.sh",
    "render_max_report.sh",
    "publish_benchmark_record.sh",
    "bench_doc_naming.sh",
    "bench_reporting_api.sh",
    "lite_bench.sh",
    "repeat_axis.sh",
    "repeat_axis_api.sh",
    "lite_metrics.sh",
    "lite_metrics_api.sh",
    "classify_cell.sh",
    "classify_cell_api.sh",
];

/// Release binaries copied byte for byte (with their permission bits): the two repeat-axis binaries first, then the
/// rest, as the original's order. `classify_cell` / `classify_cell_api` need their private sibling helper
/// `node_identity_resolve` in the same directory (the native node-identity lookup), so it is copied with them.
const COPIED_BINARIES: [&str; 11] = [
    "bench_shell",
    "repeat_axis",
    "repeat_axis_api",
    "lite_metrics",
    "lite_metrics_api",
    "classify_cell",
    "classify_cell_api",
    "node_identity_resolve",
    "render_report",
    "publish_benchmark_record",
    "bench_reporting_api",
];

/// The `docker` shim: not docker, it never runs `vllm bench serve`. It remembers the `--num-prompts` of the bench call
/// and answers the result file with that many requests (cold 1, warm N).
const DOCKER_SHIM: &str = r##"#!/usr/bin/env bash
# @SENTINEL@ — docker 가 아니다(vllm bench serve 를 돌리지 않는다).
#   bench 호출의 --num-prompts 를 기억했다가 결과 파일을 그 요청 수로 돌려준다(cold 1 · warm N).
#   SHIM_BENCH_MODE: ok(기본) · refused(D1 — 연결 거부 · 결과 파일 없음) · http500(서버가 실패로 응답)
#   SHIM_CLIENT_HEALTH: 클라이언트 평면(컨테이너 안) health 응답(기본 200 · refused 는 unreachable)
case "$1" in
  ps) echo "cafe0001" ;;
  exec)
    shift; shift
    if [ "$1" = "cat" ]; then
      n="$(cat "$SHIM_DIR/last_n" 2>/dev/null || echo 1)"
      case "${SHIM_BENCH_MODE:-ok}" in
        http500|refused) printf '{"completed": 0, "failed": %s, "total_output_tokens": 0, "median_tpot_ms": null}' "$n" ;;
        *) printf '{"completed": %s, "failed": 0, "total_output_tokens": %s, "median_tpot_ms": 25.0, "median_ttft_ms": 118.0, "output_throughput": 39.0, "spec_decode_acceptance_length": 1.75}' "$n" "$((n*128))" ;;
      esac
      exit 0
    fi
    if [ "$1" = "python3" ]; then echo "${SHIM_CLIENT_HEALTH:-200}"; exit 0; fi
    n="$(printf '%s' "$*" | sed -n 's/.*--num-prompts \([0-9]*\).*/\1/p')"; echo "${n:-1}" > "$SHIM_DIR/last_n"
    if [ "${SHIM_BENCH_MODE:-ok}" = "refused" ]; then
      # 실물 모양(2026-09-29 라이브 음성 · vllm 0.18.0): 연결 거부에도 rc 0 + 결과 파일(completed 0 · failed N)
      echo "UserWarning: All requests failed. This is likely due to a misconfiguration on the benchmark arguments." >&2
      echo "aiohttp.client_exceptions.ClientConnectorError: Cannot connect to host localhost:8000 ssl:default [Connect call failed ('127.0.0.1', 8000)]" >&2
      exit 0
    fi
    exit 0 ;;
  logs) cat "$SHIM_DIR/engine.log" ;;
  *) exit 0 ;;
esac
"##;

/// The Terraforming flag gate owner's presence marker: not the gate and never executed (the launcher stub above
/// answers every call); the callers only test that the owner file exists.
const GATE_STUB_TEXT: &str = "# @SENTINEL@ — 테라포밍 Flag 게이트 소유자 자리표시(실행되지 않는다 · 호출은 manifest_contract.sh stub 이 받는다).\n";

fn path_str(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn raised_io(error: &std::io::Error, path: &Path) -> Raised {
    Raised::os_error(&error_clone(error), &crate::verify_fixture::path_text(path))
}

fn error_clone(error: &std::io::Error) -> std::io::Error {
    match error.raw_os_error() {
        Some(code) => std::io::Error::from_raw_os_error(code),
        None => std::io::Error::new(error.kind(), error.to_string()),
    }
}

/// The isolated checkout of the lite programs (also the sandbox `selftest_lite_gate` runs in): the deployed copies,
/// the shims and the git checkout around them.
pub(crate) struct Sandbox {
    temp: TemporaryDirectory,
    root: PathBuf,
    shim_dir: PathBuf,
    /// The child environment beyond this process's own: names set (in order) and the one removed.
    environment: Vec<(OsString, OsString)>,
}

impl Sandbox {
    /// `temporary_prefix` is the name the calling program gives its temporary directory (the lite gate program
    /// runs in this sandbox under its own).
    pub(crate) fn build(
        repo_root: &Path,
        scripts_dir: &Path,
        temporary_prefix: &str,
    ) -> Result<Sandbox, Raised> {
        let temp = TemporaryDirectory::create(temporary_prefix)?;
        let root = temp.path().join("repo");
        let bin_dir = temp.path().join("bin");
        let shim_dir = temp.path().join("shimdata");
        for name in COPIED_SCRIPTS {
            write_str(
                &root.join(AB).join(name),
                &read_text(&scripts_dir.join(name))?,
            )?;
        }
        for name in COPIED_BINARIES {
            let relative = format!("{RELEASE_DIRECTORY}/{name}");
            let destination = root.join(&relative);
            if let Some(parent) = destination.parent() {
                std::fs::create_dir_all(parent).map_err(|error| raised_io(&error, parent))?;
            }
            copy2(&repo_root.join(&relative), &destination)?;
        }
        write_str(
            &root.join(GATE_STUB),
            &GATE_STUB_TEXT.replace("@SENTINEL@", SHIM_SENTINEL),
        )?;
        chmod(&root.join(GATE_STUB), 0o755)?;
        write_str(&root.join(GATE_LAUNCHER_STUB), GATE_LAUNCHER_TEXT)?;
        chmod(&root.join(GATE_LAUNCHER_STUB), 0o755)?;
        let shim = |name: &str, body: &str| -> Result<(), Raised> {
            write_str(&bin_dir.join(name), body)?;
            chmod(&bin_dir.join(name), 0o755)
        };
        shim("docker", &DOCKER_SHIM.replace("@SENTINEL@", SHIM_SENTINEL))?;
        shim("curl", "#!/bin/sh\nprintf 200\n")?;
        shim("nvidia-smi", "#!/bin/sh\necho '[N/A], [N/A]'\n")?;
        // The main RAM read (/proc/meminfo) is real: the five metrics are filled from the test host.
        write_str(
            &shim_dir.join("engine.log"),
            "INFO GPU KV cache size: 154,192 tokens\n\
INFO Available KV cache memory: 20.5 GiB\n\
INFO Model weights take 12.5 GiB\n",
        )?;
        run_checked_inherited(&[
            OsString::from("git"),
            OsString::from("init"),
            OsString::from("-q"),
            root.clone().into_os_string(),
        ])?;
        let mut sandbox = Sandbox {
            temp,
            root,
            shim_dir,
            environment: Vec::new(),
        };
        let mut search = bin_dir.into_os_string();
        search.push(":");
        search.push(std::env::var_os("PATH").unwrap_or_default());
        sandbox.environment.push((OsString::from("PATH"), search));
        sandbox.environment.push((
            OsString::from("SHIM_DIR"),
            sandbox.shim_dir.clone().into_os_string(),
        ));
        sandbox.environment.push((
            OsString::from("PYTHONDONTWRITEBYTECODE"),
            OsString::from("1"),
        ));
        sandbox.manifest(true)?;
        write_str(
            &sandbox
                .root
                .join(format!("output/single/configs/{CFG}.yaml")),
            "model: /models/fx-org/FX-Lite-7B\nmax-model-len: 8192\n",
        )?;
        write_str(
            &sandbox.root.join(format!("output/single/envs/.env.{CFG}")),
            &format!(
                "SERVING_PORT=18080\nSERVING_MODEL_NAME={CFG}\nCONTAINER_NAME=fx-ctr\n\
IMAGE_TAG=easy-vllm:9.8.7-cu130-wheel\n"
            ),
        )?;
        Ok(sandbox)
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    /// `sb.env.update(NAME=value)`.
    pub(crate) fn set_env(&mut self, name: &str, value: &str) {
        let name = OsString::from(name);
        match self
            .environment
            .iter_mut()
            .find(|(known, _)| *known == name)
        {
            Some(entry) => entry.1 = OsString::from(value),
            None => self.environment.push((name, OsString::from(value))),
        }
    }

    /// Removes the directory and everything in it (what leaving the `with` block does).
    pub(crate) fn close(self) -> Result<(), Raised> {
        self.temp.close()
    }

    pub(crate) fn manifest(&self, with_gpu: bool) -> Result<(), Raised> {
        let gpu = if with_gpu {
            "gpu_model: \"NVIDIA GB10\"\n"
        } else {
            ""
        };
        write_str(
            &self.root.join("output/single/manifest.yaml"),
            &format!("topology: single\ngpus_per_node: 1\n{gpu}"),
        )
    }

    fn settings(&self) -> ChildSettings {
        ChildSettings {
            current_dir: Some(self.root.clone()),
            env_remove: vec![OsString::from("EASY_VLLM_VERSION")],
            env_set: self.environment.clone(),
            ..ChildSettings::default()
        }
    }

    /// `subprocess.run(argv, cwd=root, env=self.env, capture_output=True, text=True, timeout=120)`.
    pub(crate) fn run(&self, argv: Vec<OsString>) -> Result<TextRun, Raised> {
        run_text_within(&argv, &self.settings(), CHILD_SECONDS)
    }

    pub(crate) fn script(&self, name: &str) -> OsString {
        self.root.join(AB).join(name).into_os_string()
    }

    /// `bash lite_bench.sh <cfg> --topology single --backend openai <extra>`.
    pub(crate) fn lite(&self, extra: &[&str]) -> Result<TextRun, Raised> {
        let mut argv = vec![
            OsString::from("bash"),
            self.script("lite_bench.sh"),
            OsString::from(CFG),
            OsString::from("--topology"),
            OsString::from("single"),
            OsString::from("--backend"),
            OsString::from("openai"),
        ];
        argv.extend(words(extra));
        self.run(argv)
    }

    /// `bash render_report.sh <argv>`.
    fn render(&self, arguments: &[&str]) -> Result<TextRun, Raised> {
        let mut argv = vec![OsString::from("bash"), self.script("render_report.sh")];
        argv.extend(words(arguments));
        self.run(argv)
    }

    fn matching_names(&self, prefix: &str, suffix: &str) -> Vec<String> {
        let directory = self.root.join("docs").join("benchmark");
        let Ok(listing) = std::fs::read_dir(&directory) else {
            return Vec::new();
        };
        let mut names: Vec<String> = listing
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| {
                name.len() >= prefix.len() + suffix.len()
                    && name.starts_with(prefix)
                    && name.ends_with(suffix)
            })
            .collect();
        names.sort();
        names
    }

    pub(crate) fn reports(&self) -> Vec<String> {
        self.matching_names("bench_report_", ".md")
    }

    pub(crate) fn certs(&self) -> Vec<String> {
        self.matching_names("benchmark_", ".yaml")
    }

    /// The raw of the **last run**: α and sweep legs write under `benchlog`, β (`--publish-report`) under
    /// `benchlog/lite_publish/<config>_<UTC>/` (so α never overwrites β's raw). The most recently written wins (the
    /// judge writes in place, so the modification time is that run's).
    pub(crate) fn raw_path(&self) -> PathBuf {
        use std::os::unix::fs::MetadataExt;
        let base = self.root.join("output/single/benchlog");
        let file_name = format!("lite_raw_{CFG}.json");
        let mut candidates = vec![base.join(&file_name)];
        if let Ok(listing) = std::fs::read_dir(base.join("lite_publish")) {
            for entry in listing.filter_map(Result::ok) {
                candidates.push(entry.path().join(&file_name));
            }
        }
        let mut best: Option<(i128, PathBuf)> = None;
        for candidate in candidates {
            let Ok(metadata) = std::fs::metadata(&candidate) else {
                continue;
            };
            if !metadata.is_file() {
                continue;
            }
            let stamp =
                i128::from(metadata.mtime()) * 1_000_000_000 + i128::from(metadata.mtime_nsec());
            if best.as_ref().is_none_or(|(known, _)| stamp > *known) {
                best = Some((stamp, candidate));
            }
        }
        best.map_or_else(|| base.join(&file_name), |(_, path)| path)
    }
}

/// `^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$`: `\d` is any Unicode decimal digit (the project's own digit table), and
/// `$` also accepts one trailing newline.
fn measured_utc_shape(text: &str) -> bool {
    let body = text.strip_suffix('\n').unwrap_or(text);
    let shape = [
        'd', 'd', 'd', 'd', '-', 'd', 'd', '-', 'd', 'd', 'T', 'd', 'd', ':', 'd', 'd', ':', 'd',
        'd', 'Z',
    ];
    let characters: Vec<char> = body.chars().collect();
    characters.len() == shape.len()
        && characters
            .iter()
            .zip(shape)
            .all(|(character, kind)| match kind {
                'd' => is_decimal_digit(u32::from(*character)),
                other => *character == other,
            })
}

fn load_raw(path: &Path) -> Result<PyValue, Raised> {
    crate::verify_fixture::load_json(path)
}

fn names_value(names: &[String]) -> PyValue {
    PyValue::List(names.iter().map(|name| st(name)).collect())
}

fn is_dict(value: &PyValue) -> bool {
    matches!(value.peel(), PyValue::Dict(_))
}

/// `(value or {}).get("bench_mode")`.
fn mode_of(config: &PyValue) -> Result<PyValue, Raised> {
    get(
        &or_default(config.clone(), PyValue::Dict(PyDict::new())),
        "bench_mode",
    )
}

/// `raw.get(key, default)`: the default only for an absent key (an explicit null stays `None`).
fn get_or(value: &PyValue, key: &str, default: &str) -> Result<PyValue, Raised> {
    match value.peel() {
        PyValue::Dict(map) => Ok(map.get(key).cloned().unwrap_or_else(|| st(default))),
        _ => get(value, key),
    }
}

/// `Path(text)`: the empty text is the current directory.
fn path_of(text_path: &str) -> PathBuf {
    if text_path.is_empty() {
        PathBuf::from(".")
    } else {
        PathBuf::from(text_path)
    }
}

fn parent_of(text_path: &str) -> PathBuf {
    let parent = Path::new(text_path)
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default();
    if parent.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        parent
    }
}

/// The program. `verbose` is `-v` among the arguments.
#[allow(clippy::too_many_lines)]
pub fn main(
    repo_root: Option<&Path>,
    verbose: bool,
    console: &mut dyn Console,
) -> Result<i32, Raised> {
    let mut checks = BracketChecks::new(console, DetailStyle::TailedWithDash);
    if which("git").is_none() {
        checks.write_error(&format!(
            "[{PROGRAM}] FAIL git 이 없다 — 격리 저장소를 만들 수 없다(부재를 통과로 접지 않는다)\n"
        ));
        return Ok(1);
    }
    let Some(repo_root) = repo_root else {
        checks.write_error(&format!("{}\n", repo_root_unknown_message(PROGRAM)));
        return Ok(2);
    };
    let canonical = std::fs::canonicalize(repo_root).unwrap_or_else(|_| repo_root.to_path_buf());
    let scripts_dir = canonical.join(AB);
    // The hint reader, the evidence publisher and the completion gate are Rust libraries (no interpreter, no `.py`).
    let mut bridge = OwnerBridge::start();

    let sandbox = Sandbox::build(&canonical, &scripts_dir, "lite-report-selftest.")?;
    let outcome = run_sandbox_cases(&mut checks, &mut bridge, &sandbox, &scripts_dir, verbose);
    sandbox.close()?;
    outcome?;
    run_remaining_cases(&mut checks, &mut bridge, &scripts_dir)?;

    if !checks.failures().is_empty() {
        let line = format!(
            "[selftest_lite_report] FAIL {} 건: {}\n",
            checks.failures().len(),
            checks.failure_list_repr()
        );
        checks.write_error(&line);
        return Ok(1);
    }
    checks.print(
        "[selftest_lite_report] PASS — L1~L10(경량 리포트 발행 · 측정 구성 표 · 음성대조 · 명명 교차검증)",
    )?;
    Ok(0)
}

#[allow(clippy::too_many_lines)]
fn run_sandbox_cases(
    checks: &mut BracketChecks<'_>,
    bridge: &mut OwnerBridge,
    sandbox: &Sandbox,
    _scripts_dir: &Path,
    verbose: bool,
) -> Result<(), Raised> {
    let docs = sandbox.root().join("docs/benchmark");

    // ── L1 default = the automatic handoff path (no publication) ────────────────────────
    let run = sandbox.lite(&[])?;
    if verbose {
        let line = format!("{} {}", tail(&run.stdout, 1500), tail(&run.stderr, 1500));
        checks.print(&line)?;
    }
    let raw_path = sandbox.raw_path();
    let raw = if is_file(&raw_path)? {
        load_raw(&raw_path)?
    } else {
        dict_of(vec![])
    };
    let cond = run.status == 0
        && sandbox.reports().is_empty()
        && sandbox.certs().is_empty()
        && run.stdout.contains("미발행");
    checks.check(
        "L1 기본(플래그 없음) → exit 0 · 리포트 미발행 · 미발행을 이름으로 알린다",
        cond,
        || tuple_text(vec![int(run.status), st(&run.stderr)]),
    )?;
    let mut inputs_exist = true;
    for key in ["config_yaml", "env_file", "manifest"] {
        let named = py_str(&get_or(&raw, key, "")?);
        if !is_file(&path_of(&named))? {
            inputs_exist = false;
            break;
        }
    }
    let measured_utc = py_str(&get_or(&raw, "measured_utc", "")?);
    let cond =
        measured_utc_shape(&measured_utc) && get_eq_text(&raw, "config_name", CFG)? && inputs_exist;
    checks.check(
        "L1 raw 에 측정시각(UTC 초)·명명 입력(config_name·config_yaml·env_file·manifest)이 실린다",
        cond,
        || raw.repr(),
    )?;

    // ── L2 --publish-report ─────────────────────────────────────────────────────────────
    let run = sandbox.lite(&["--publish-report"])?;
    let raw = load_raw(&sandbox.raw_path())?;
    let names = sandbox.reports();
    let report_text = if names.len() == 1 {
        read_text(&docs.join(&names[0]))?
    } else {
        String::new()
    };
    checks.check(
        "L2 --publish-report → exit 0 · 경량 리포트 정확히 1건",
        run.status == 0 && names.len() == 1,
        || tuple_text(vec![int(run.status), names_value(&names), st(&run.stderr)]),
    )?;
    let empty_environment = PyValue::Dict(PyDict::new());
    let identity = if raw.is_truthy() {
        lite_identity(&raw, &Environment::Mapping(&empty_environment))
            .map_err(Raised::from_failure)?
    } else {
        dict_of(vec![])
    };
    let cond = !names.is_empty()
        && {
            let expected = bench_filename(
                &st("bench_report"),
                &identity,
                &get(&raw, "measured_utc")?,
                &PyValue::None,
                &PyValue::None,
            )
            .map_err(Raised::from_failure)?;
            expected.equals(&names[0])
        }
        && bridge.publisher_name_matches(checks.console(), "bench_report", &names[0])?;
    checks.check(
        "L2 이름 = 명명 SSOT(doc_naming.bench_filename) = publisher 정규식(변경 없음)",
        cond,
        || tuple_text(vec![names_value(&names), identity.clone()]),
    )?;
    let cond = get_eq_text(&identity, "model", "fx-lite-7b")?
        && get_eq_text(&identity, "gpu_key", "GB10")?
        && get_eq_text(&identity, "vllm_version", "9.8.7")?
        && get_eq_text(
            &identity,
            "vllm_version_source",
            "derived(envfile IMAGE_TAG)",
        )?;
    checks.check(
        "L2 명명 키가 체크포인트·manifest·이미지 태그에서 파생된다(출처 표시)",
        cond,
        || identity.repr(),
    )?;
    let measurement_config = if report_text.is_empty() {
        PyValue::None
    } else {
        bridge.parse_measurement_config(checks.console(), &report_text)?
    };
    let cond = !report_text.is_empty()
        && bridge.is_lite_report(checks.console(), &report_text)?
        && is_dict(&measurement_config)
        && get_eq_text(&measurement_config, "bench_mode", "lite")?
        && get_eq_text(&measurement_config, "bench_mode_kind", "declared-lite")?
        && get_eq_text(&measurement_config, "bench_tool", "vllm-bench-serve")?
        && get_eq(&measurement_config, "repeats", &int(1))?
        && is_none(&get(&measurement_config, "downgrade_reason")?)
        && starts_with(
            &st(&py_str(&get_or(
                &measurement_config,
                "bench_mode_source",
                "",
            )?)),
            BENCH_MODE_SOURCE_DECLARED_PREFIX,
        )?;
    checks.check(
        "L2 헤더 mode: lite · 측정 구성 표 = 선언된 lite(bench_mode·kind·도구·반복 1·강등 사유 없음)",
        cond,
        || measurement_config.repr(),
    )?;
    let cond = !report_text.is_empty()
        && bridge.lite_metrics_present(checks.console(), &report_text)?
        && report_text.contains("| gen tokens/sec (warm) | 40.00 t/s |")
        && {
            let rendered = bridge.render_lite(
                checks.console(),
                &report_text,
                names.first().map_or("x", String::as_str),
            )?;
            let title = bridge.reader_constant(checks.console(), "LITE_SECTION_TITLE")?;
            starts_with(&rendered, &py_str(&title))?
        };
    checks.check(
        "L2 lite 지표 표가 실측값으로 실린다(HINT_MISSING_LITE 술어 참) · hint 파서가 lite 절을 렌더한다",
        cond,
        || report_text.chars().take(1500).collect::<String>(),
    )?;
    let accepted = if report_text.is_empty() {
        false
    } else {
        let config = bridge.publisher_measurement_config(checks.console(), &report_text)?;
        let mode = mode_of(&config)?;
        let lite_mode = bridge.publisher_constant(checks.console(), "LITE_BENCH_MODE")?;
        py_equals(&mode, &lite_mode)
    };
    checks.check(
        "L2 publish-lite-report 수용 술어가 참이다(발행기가 표를 파싱해 lite 리포트만 바인딩한다)",
        accepted,
        String::new,
    )?;
    let named_time = measurement_ts(&st(&report_text)).map_err(Raised::from_failure)?;
    let cond = match (&named_time, get(&raw, "measured_utc")?.peel()) {
        (Some(left), PyValue::Str(right)) => left == right,
        (None, PyValue::None) => true,
        _ => false,
    };
    checks.check(
        "L2 본문 측정시각 = raw 측정시각(같은 측정 판정 근거 · doc_naming.measurement_ts)",
        cond,
        || {
            named_time.as_ref().map_or("None".to_string(), |value| {
                value.to_utf8().unwrap_or_default()
            })
        },
    )?;
    let sweep_header = bridge.reader_constant(checks.console(), "REPORT_HEADER")?;
    checks.check(
        "L2 스윕 표가 없다(동시성 곡선을 재지 않았다 — 만들어 넣지 않는다)",
        !contains(&st(&report_text), &py_str(&sweep_header))?,
        String::new,
    )?;
    // factcheck F5 (2026-10-06): the warm leg's accept length is carried under one name by the report and the lite
    // certificate.
    let certs = sandbox.certs();
    let certificate_text = if certs.len() == 1 {
        read_text(&docs.join(&certs[0]))?
    } else {
        String::new()
    };
    let cond = report_text.contains("| lite_accept_len | 1.75 |")
        && report_text.contains("| lite_accept_len_src | warm.spec_decode_acceptance_length |")
        && certificate_text.contains("\nlite_accept_len: 1.75\n")
        && certificate_text.contains("lite_accept_len_src: warm.spec_decode_acceptance_length");
    checks.check(
        "L2 F5 lite_accept_len = warm 레그 spec_decode_acceptance_length 원문 — 리포트 표 · lite 인증서(출처 칸 병기)",
        cond,
        || tuple_text(vec![names_value(&certs), st(&tail(&certificate_text, 700))]),
    )?;
    // factcheck F1 (2026-10-06): the head of the engine log is captured beside the tail; the raw names path and size.
    let head_text = py_str(&get_or(&raw, "engine_log_head", "")?);
    let head_path_buffer = path_of(&head_text);
    let head_path = head_path_buffer.as_path();
    let engine_log_text = py_str(&get(&raw, "engine_log")?);
    let cond = head_path
        .file_name()
        .is_some_and(|name| name == format!("lite_engine_head_{CFG}.log").as_str())
        && is_file(head_path)?
        && parent_of(&head_text) == parent_of(&engine_log_text)
        && get_eq(&raw, "engine_log_head_lines", &int(600))?;
    checks.check(
        "L2 F1 엔진 로그 머리 캡처 lite_engine_head_<config>.log 가 raw engine_log 옆에 남고 raw 가 가리킨다",
        cond,
        || raw.repr(),
    )?;

    // ── L3 idempotent re-render ─────────────────────────────────────────────────────────
    let run3 = sandbox.render(&[
        "--lite-only",
        "--lite-raw-json",
        &path_str(&sandbox.raw_path()),
    ])?;
    let cond = run3.status == 0
        && sandbox.reports() == names
        && py_strip(&run3.stdout).ends_with(names.first().map_or("?", String::as_str));
    checks.check(
        "L3 같은 raw 재렌더 → 같은 이름 덮어쓰기(파일 1건 유지)",
        cond,
        || {
            tuple_text(vec![
                int(run3.status),
                st(&run3.stderr),
                names_value(&sandbox.reports()),
            ])
        },
    )?;

    // ── L4 a failed publication does not lose the measurement ────────────────────────────
    std::fs::remove_dir_all(&docs).map_err(|error| raised_io(&error, &docs))?;
    sandbox.manifest(false)?;
    let run4 = sandbox.lite(&["--publish-report"])?;
    let cond = run4.status == 5
        && sandbox.reports().is_empty()
        && is_file(&sandbox.raw_path())?
        && run4.stderr.contains("gpu_model");
    checks.check(
        "★L4 manifest gpu_model 부재 + --publish-report → exit 5 · 리포트 0 · raw 는 남는다 · 사유를 말한다",
        cond,
        || tuple_text(vec![int(run4.status), st(&tail(&run4.stderr, 600))]),
    )?;
    let run4b = sandbox.lite(&[])?;
    checks.check(
        "★L4 같은 조건의 자동 핸드오프 경로(플래그 없음)는 exit 0 — 발행 실패가 lite 자체를 실패로 만들지 않는다",
        run4b.status == 0 && sandbox.reports().is_empty(),
        || tuple_text(vec![int(run4b.status), st(&tail(&run4b.stderr, 400))]),
    )?;
    sandbox.manifest(true)?;

    // ── L5 no measurement time ──────────────────────────────────────────────────────────
    let without_time =
        crate::verify_fixture::without_key(load_raw(&sandbox.raw_path())?, "measured_utc")?;
    write_text(&sandbox.root().join("bad_raw.json"), &dumps(&without_time)?)?;
    let run5 = sandbox.render(&[
        "--lite-only",
        "--lite-raw-json",
        &path_str(&sandbox.root().join("bad_raw.json")),
    ])?;
    let cond =
        run5.status == 2 && sandbox.reports().is_empty() && run5.stderr.contains("measured_utc");
    checks.check(
        "★L5 raw 측정시각 부재 → exit 2 · 리포트 0(측정시각을 날조하지 않는다)",
        cond,
        || tuple_text(vec![int(run5.status), st(&run5.stderr)]),
    )?;

    // ── L6 mode mix and missing arguments ───────────────────────────────────────────────
    let raw_text = path_str(&sandbox.raw_path());
    let run6 = sandbox.render(&[
        "--lite-only",
        "--lite-raw-json",
        &raw_text,
        "--sweep-index",
        "x.json",
    ])?;
    let run6b = sandbox.render(&["--sweep-index", "x.json"])?;
    let run6c = sandbox.render(&[
        "--lite-raw-json",
        &raw_text,
        "--sweep-index",
        "x.json",
        "--verdict-json",
        "v.json",
    ])?;
    let cond =
        run6.status == 2 && run6b.status == 2 && run6c.status == 2 && sandbox.reports().is_empty();
    checks.check(
        "★L6 --lite-only 에 스윕 인자 혼합 · full 인자 누락 · full 에 lite raw 혼합 → 전부 exit 2 · 리포트 0",
        cond,
        || tuple_text(vec![int(run6.status), int(run6b.status), int(run6c.status)]),
    )?;
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn run_remaining_cases(
    checks: &mut BracketChecks<'_>,
    bridge: &mut OwnerBridge,
    scripts_dir: &Path,
) -> Result<(), Raised> {
    // ── L7 a full sweep's lite leg does not publish ─────────────────────────────────────
    let sweep_text = read_text(&scripts_dir.join("sweep_bench.sh"))?;
    let lite_calls: Vec<String> = splitlines(&PyStr::new(&sweep_text))
        .iter()
        .map(|line| line.to_utf8().unwrap_or_default())
        .filter(|line| line.contains("lite_bench.sh") && py_lstrip(line).starts_with("bash "))
        .collect();
    let cond = lite_calls.len() == 1 && !lite_calls[0].contains("--publish-report");
    checks.check(
        "★L7 sweep_bench 의 lite 레그 호출은 1줄이고 --publish-report 를 넘기지 않는다(full 리포트 stem 보호)",
        cond,
        || names_value(&lite_calls).repr(),
    )?;
    let lite_text = read_text(&scripts_dir.join("lite_bench.sh"))?;
    let cond = lite_text.contains("PUBLISH_REPORT=0")
        && lite_text.contains("if [ \"$PUBLISH_REPORT\" = \"1\" ]")
        && lite_text.contains("--lite-only --lite-raw-json");
    checks.check(
        "L7 lite_bench 는 플래그가 있을 때만 render_report --lite-only 를 부른다(기본 PUBLISH_REPORT=0)",
        cond,
        String::new,
    )?;

    // ── L8 naming key cross-check (embedded sweep program, executed, against lite_identity) ──
    let meta_source = SweepMeta::load(scripts_dir)?;
    let empty_environment = PyValue::Dict(PyDict::new());
    for (label, engine) in [
        ("엔진 로그 있음", Some(("fp8", "fp8"))),
        ("엔진 로그 없음", None),
    ] {
        let arguments = BuildMeta {
            engine,
            ..BuildMeta::default()
        };
        let meta = meta_source.build_meta(checks.console(), &arguments)?;
        let directory = TemporaryDirectory::create("lite-identity.")?;
        let identity = (|| -> Result<PyValue, Raised> {
            let base = directory.path();
            write_str(
                &base.join("cfg.yaml"),
                "model: /models/acme/Acme-7B\nmax-model-len: 262144\n",
            )?;
            write_str(
                &base.join(".env.x"),
                "IMAGE_TAG=easy-vllm:9.9.9-test\nSERVING_MODEL_NAME=Acme-7B\n",
            )?;
            write_str(
                &base.join("manifest.yaml"),
                "gpu_model: NVIDIA GB10\ngpus_per_node: 1\ntopology: single\n",
            )?;
            let log = if engine.is_some() {
                engine_log_line("fp8", "fp8")
            } else {
                String::new()
            };
            write_str(&base.join("engine.log"), &log)?;
            let raw = dict_of(vec![
                ("config_name", st("x")),
                ("config_yaml", st(&path_str(&base.join("cfg.yaml")))),
                ("env_file", st(&path_str(&base.join(".env.x")))),
                ("manifest", st(&path_str(&base.join("manifest.yaml")))),
                ("engine_log", st(&path_str(&base.join("engine.log")))),
                ("topology", st("single")),
            ]);
            lite_identity(&raw, &Environment::Mapping(&empty_environment))
                .map_err(Raised::from_failure)
        })();
        directory.close()?;
        let identity = identity?;
        let keys = ["model", "model_source", "gpu_key", "vllm_version"];
        let mut same = true;
        for key in keys {
            if !py_equals(&get(&meta, key)?, &get(&identity, key)?) {
                same = false;
                break;
            }
        }
        checks.check(
            &format!(
                "L8 명명 키 교차검증({label}) — sweep_bench 조립 heredoc 과 lite_identity 가 같은 model·gpu_key·vllm_version"
            ),
            same,
            || -> Result<String, Raised> {
                let pick = |document: &PyValue| -> Result<PyValue, Raised> {
                    Ok(dict_of(
                        keys.iter()
                            .map(|key| Ok((*key, get(document, key)?)))
                            .collect::<Result<Vec<_>, Raised>>()?,
                    ))
                };
                Ok(tuple_text(vec![pick(&meta)?, pick(&identity)?])) },
        )?;
    }

    // ── L9 the measurement configuration table of the full report ───────────────────────
    let directory = TemporaryDirectory::create("lite-full-mc.")?;
    let outcome = full_report_cases(checks, bridge, scripts_dir, directory.path());
    directory.close()?;
    outcome?;

    // ── L10 the contract constants between the two skills ───────────────────────────────
    let title = bridge.reader_constant(checks.console(), "MEASUREMENT_CONFIG_TITLE")?;
    let cond = py_equals(&title, &st(MEASUREMENT_CONFIG_TITLE)) && {
        let keys = bridge.reader_constant(checks.console(), "MEASUREMENT_CONFIG_KEYS")?;
        py_equals(
            &PyValue::Tuple(match keys.peel() {
                PyValue::List(items) | PyValue::Tuple(items) => items.clone(),
                _ => Vec::new(),
            }),
            &PyValue::Tuple(MEASUREMENT_CONFIG_KEYS.iter().map(|key| st(key)).collect()),
        ) && matches!(keys.peel(), PyValue::List(_) | PyValue::Tuple(_))
    };
    checks.check(
        "L10 측정 구성 표 제목·키가 writer(render_report)와 reader(render_bench_section)에서 같다",
        cond,
        String::new,
    )?;
    // The default tool token of the sweep assembly is read from what the deployed producer really writes for a level
    // whose measurement names no tool (the fixture's `measured.json` does not).
    let default_tool = get(
        &meta_source.build_meta(checks.console(), &BuildMeta::default())?,
        "bench_tool",
    )?;
    checks.check(
        "L10 lite 도구 토큰이 sweep_bench 조립부의 도구 기본 토큰과 같은 철자다",
        py_equals(&default_tool, &st(LITE_BENCH_TOOL)),
        String::new,
    )?;
    let lite_mode = bridge.publisher_constant(checks.console(), "LITE_BENCH_MODE")?;
    checks.check(
        "L10 publish-lite-report 가 받는 bench_mode 토큰 = classify_cell 어휘의 lite",
        py_equals(&lite_mode, &st(BENCH_MODE_LITE)),
        String::new,
    )?;
    Ok(())
}

/// The full sweep fixture of L9: the index, the verdict and the judging record that decide the table.
fn full_report_cases(
    checks: &mut BracketChecks<'_>,
    bridge: &mut OwnerBridge,
    scripts_dir: &Path,
    base: &Path,
) -> Result<(), Raised> {
    let generated = "2026-09-14T01:00:00Z";
    let index = dict_of(vec![
        ("generated_utc", st(generated)),
        ("verdict_point_level", int(1)),
        (
            "meta",
            dict_of(vec![
                ("model", st("m")),
                ("gpu_model", st("NVIDIA GB10")),
                ("vllm_version", st("0.0.1")),
                ("bench_tool", st("guidellm")),
                ("bench_tool_version", st("0.7.3")),
                ("bench_tool_version_source", st("measured(benchmarks.json)")),
            ]),
        ),
        (
            "levels",
            PyValue::List(vec![dict_of(vec![
                ("level", int(1)),
                ("status", st("ok")),
                (
                    "measured",
                    dict_of(vec![
                        ("decode_tps", float(40.0)),
                        ("completed", int(4)),
                        ("failed", int(0)),
                    ]),
                ),
            ])]),
        ),
        (
            "repetition",
            dict_of(vec![
                ("requested", int(3)),
                ("requested_source", st("campaign(budgets.repeats)")),
                ("kind", st("warm-rerun")),
                ("completed_by_level", dict_of(vec![("1", int(2))])),
            ]),
        ),
        ("truncated", PyValue::List(vec![])),
    ]);
    write_text(&base.join("sweep_index.json"), &dumps(&index)?)?;
    write_text(
        &base.join("verdict.json"),
        &dumps(&dict_of(vec![
            ("verdict", st("PASS")),
            ("rubric", dict_of(vec![])),
        ]))?,
    )?;

    let full_render = |checks: &mut BracketChecks<'_>,
                       bridge: &mut OwnerBridge,
                       record: Option<PyValue>|
     -> Result<(TextRun, PyValue), Raised> {
        let record_path = base.join(BENCH_MODE_RECORD_NAME);
        match record {
            None => unlink_if_present(&record_path)?,
            Some(PyValue::Dict(map)) => {
                let mut document = map.clone();
                document.set("sweep_index_generated_utc", st(generated));
                write_text(&record_path, &dumps(&PyValue::Dict(document))?)?;
            }
            Some(_) => unreachable!("the L9 records are objects"),
        }
        let command: Vec<OsString> = vec![
            "bash".into(),
            scripts_dir.join("render_report.sh").into_os_string(),
            "--sweep-index".into(),
            base.join("sweep_index.json").into_os_string(),
            "--verdict-json".into(),
            base.join("verdict.json").into_os_string(),
            "--stdout".into(),
        ];
        let run = run_text_within(&command, &ChildSettings::default(), FULL_RENDER_SECONDS)?;
        let config = if run.status == 0 {
            bridge.parse_measurement_config(checks.console(), &run.stdout)?
        } else {
            PyValue::None
        };
        Ok((run, config))
    };

    let downgraded = dict_of(vec![
        ("bench_mode", st("lite")),
        ("bench_mode_source", st("runs[] 집계(판정점 끊김)")),
        ("downgrade_reason", st("run_failed")),
        ("downgrade_reason_source", st("runs[] run 3")),
        ("downgrade_correlation", st("not_scanned")),
    ]);
    let (run, down) = full_render(checks, bridge, Some(downgraded))?;
    let cond = is_dict(&down)
        && get_eq_text(&down, "bench_mode", "lite")?
        && get_eq_text(&down, "bench_mode_kind", "downgraded-lite")?
        && get_eq_text(&down, "downgrade_reason", "run_failed")?
        && get_eq(&down, "repeats", &int(3))?
        && get_eq(&down, "repeats_completed", &int(2))?
        && get_eq_text(&down, "bench_tool", "guidellm")?
        && get_eq_text(&down, "bench_tool_version", "0.7.3")?;
    checks.check(
        "L9 강등 기록 → 표 bench_mode=lite · kind=downgraded-lite · 사유 run_failed · 요청 반복 3 · 판정점 완주 2 · 도구",
        cond,
        || tuple_text(vec![st(&run.stderr), down.clone()]),
    )?;
    let lite_token = |bridge: &mut OwnerBridge, checks: &mut BracketChecks<'_>| {
        bridge.publisher_constant(checks.console(), "LITE_BENCH_MODE")
    };
    let first = bridge.publisher_measurement_config(checks.console(), &run.stdout)?;
    let cond = py_equals(&mode_of(&first)?, &lite_token(bridge, checks)?)
        && {
            let second = bridge.publisher_measurement_config(checks.console(), &run.stdout)?;
            bench_mode_kind(&second) == "downgraded-lite"
        }
        && py_equals(
            &bridge.parse_report_level_count(checks.console(), &run.stdout)?,
            &int(1),
        )
        && !bridge.is_lite_report(checks.console(), &run.stdout)?;
    checks.check(
        "L9 강등 셀의 full 모양 리포트는 발행기 표 판독에서 lite · downgraded-lite 이고(같은 lite 통로) 스윕 표 형식 계약은 그대로다",
        cond,
        String::new,
    )?;
    let full = dict_of(vec![
        ("bench_mode", st("full")),
        ("bench_mode_source", st("runs[] 집계(판정점 완주 3)")),
        ("downgrade_reason", PyValue::None),
        ("downgrade_reason_source", PyValue::None),
        ("downgrade_correlation", st("not_applicable")),
    ]);
    let (run, full_config) = full_render(checks, bridge, Some(full))?;
    let cond = is_dict(&full_config)
        && get_eq_text(&full_config, "bench_mode", "full")?
        && get_eq_text(&full_config, "bench_mode_kind", "full")?
        && {
            let config = bridge.publisher_measurement_config(checks.console(), &run.stdout)?;
            let lite = lite_token(bridge, checks)?;
            !py_equals(&mode_of(&config)?, &lite)
        };
    checks.check(
        "★L9 음성대조 full 기록 → bench_mode=full · 발행기 표 판독도 lite 가 아니다(full 리포트를 lite 로 바인딩할 수 없다)",
        cond,
        || full_config.repr(),
    )?;
    let (run, none_config) = full_render(checks, bridge, None)?;
    let cond = run.status == 0
        && is_dict(&none_config)
        && is_none(&get(&none_config, "bench_mode")?)
        && get_eq_text(&none_config, "bench_mode_kind", "undetermined")?
        && starts_with(
            &st(&py_str(&get_or(&none_config, "bench_mode_source", "")?)),
            "미확정",
        )?;
    checks.check(
        "★L9 판정 기록 부재 → 리포트는 발행 · bench_mode 미확정(N/A)으로 적는다(full·lite 로 접지 않는다)",
        cond,
        || tuple_text(vec![int(run.status), none_config.clone()]),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{measured_utc_shape, COPIED_BINARIES};

    #[test]
    fn the_measurement_time_shape_accepts_unicode_decimal_digits_and_one_trailing_newline() {
        assert!(measured_utc_shape("2026-11-05T09:01:07Z"));
        assert!(measured_utc_shape("2026-11-05T09:01:07Z\n"));
        // `\d` of a Python `str` pattern matches Arabic-Indic digits too.
        assert!(measured_utc_shape(
            "\u{662}\u{660}\u{662}\u{666}-11-05T09:01:07Z"
        ));
        assert!(!measured_utc_shape("2026-11-05T09:01:07Z\n\n"));
        assert!(!measured_utc_shape("2026-11-05 09:01:07Z"));
        assert!(!measured_utc_shape("2026-11-05T09:01:07z"));
    }

    #[test]
    fn the_classifier_binaries_are_copied_with_their_private_node_identity_helper() {
        // dependency closure: whenever a classifier binary is mirrored, its sibling helper is mirrored too
        let helper = crate::classify_cell_owner::HELPER_NAME;
        for caller in ["classify_cell", "classify_cell_api"] {
            assert!(COPIED_BINARIES.contains(&caller), "{caller}");
        }
        assert_eq!(
            COPIED_BINARIES
                .iter()
                .filter(|name| **name == helper)
                .count(),
            1
        );
    }
}
