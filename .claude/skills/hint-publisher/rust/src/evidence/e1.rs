//! evidence part 1 — port of `scripts/hintlib/evidence.py` lines 1–1912: constants, missing codes, the CellEvidence
//! shape, small tools, owner mirrors (incl. the completion_gate `CGate`), certificate/gate/authorize, publication
//! records, sweep binding, read-only docker probes, missing/measurement.
#![allow(non_snake_case, non_upper_case_globals, dead_code, unused_imports)]
use std::path::{Path, PathBuf};

use crate::core::{self, HintError, HR};
use crate::value::{Map, J};
use crate::{jd, jl};
use super::*;

/// Owners = their Rust launchers (plan_26111615 D4 · presence never a `.py`).
pub const REL_MANIFEST_CONTRACT: &str = ".claude/skills/terraforming_node/scripts/manifest_contract.sh";
pub const REL_CHECK_SMOKE_MODEL: &str = ".claude/skills/upstream-version-watch/scripts/check_smoke_model.sh";
pub const _GATE_REQUIRED_API: [&str; 10] = ["parse_flat_certificate", "certificate_run_key", "CERTIFICATE_FIELD_MAP", "STRONG_IDENTITY_FIELDS", "lexical_components", "manifest_rubric_contract", "PII_MACHINE_RAW_EXEMPTION_REASON", "certificate_requirement_authority", "certificate_waived", "CERTIFICATE_NON_ISSUING_AUTHORITIES"];
pub const NODE_AXIS: [&str; 3] = ["main", "sub", "cluster"];
pub const TOPOLOGIES: [&str; 2] = ["single", "multi"];
pub const RESERVED_CAMPAIGN_IDS: [&str; 2] = ["_template", "_bootstrap"];
pub const MODES: [&str; 2] = ["campaign", "publication-replay"];
pub const _PLE_KEYS: [&str; 2] = ["ple_layer_ids", "ple_embed_dim"];
pub const _WHEEL_URL_KEYS: [&str; 2] = ["VLLM_WHEEL_URL", "WHEEL_URL"];
/// hintlib/evidence.py:94 MISSING_CODES (python literal as JSON)
pub fn MISSING_CODES() -> &'static J {
    static CELL: std::sync::OnceLock<J> = std::sync::OnceLock::new();
    CELL.get_or_init(|| crate::value::loads("{\"HINT_MISSING_CERTIFICATE\": \"인증서 부재 — 인증서가 발행되는 판정(루브릭 권한 explicit ∧ PASS, 또는 판정·권한을 관측하지 못한 측정)인데 인증서가 없다. 비발행 판정(REFUTE · weak/explore PASS · lite-only)은 이 결손이 아니다 — 그때 판정 근거는 bench_report 이고 판정은 measurement.verdict 가 말한다. 인증서 발행은 adversarial-benchmark 의 책임이다.\", \"HINT_MISSING_BENCH_REPORT\": \"벤치 리포트 부재 — 동시성별 곡선을 실을 수 없다.\", \"HINT_MISSING_SWEEP_LEVELS\": \"부하 레벨이 1개 이하 — 부하 거동을 알 수 없다(경량 리포트는 동시성 곡선을 재지 않는다).\", \"HINT_MISSING_LITE\": \"lite 관측 부재.\", \"HINT_MISSING_SLAVE_ATTESTATION\": \"슬레이브 ABI attestation 부재 — 멀티에서 두 노드가 같은 것을 돌렸다는 증거가 성공 경로에 보존되지 않았다.\", \"HINT_MISSING_ENV_SHAPE_RERENDER\": \"측정 뒤 재생성된 토폴로지 env(`.env.interconnect`·`.env.cluster`)를 측정 당시 판본 렌더러로 다시 렌더하지 못했다(측정 전 렌더러 커밋 · manifest · 렌더러 거부 중 하나 — 사유는 compose 제외 행의 rerender_error). 형상 템플릿이 빠졌다 — 측정 당시 env 는 엔진 로그 관측 표가 대신한다.\", \"HINT_MISSING_ENV_SHAPE\": \"토폴로지 env 형상 부재 — compose 가 참조하는 env 파일(`.env.cluster`·`.env.interconnect` 등)이 출력 평면에 없어 형상 템플릿을 싣지 못했다. 수신자가 어떤 변수가 필요한지 모른다(artifacts 가 기재).\", \"HINT_MISSING_SUB_TRIPLET\": \"서브 트리플렛 부재 — 서브는 자기 것을 자율 저작하며 상향 회수는 **문서기반 only** 다(코드·설정 직접 회수 금지). 해소는 서브가 자기 트리플렛을 문서로 발행 → 메인이 재저작 → 그 문서를 발행 증거로 삼는 것이다.\", \"HINT_MISSING_TRIPLET\": \"트리플렛 부재(메인 워킹트리에 config/runner/env 3종이 없다).\", \"HINT_MISSING_PII_TERMS\": \"pii_terms.txt 부재 — 리터럴 스캔이 축소된 상태로 돌았다.\", \"HINT_MISSING_MEASURED_NODE\": \"인증서에 측정 노드 출처가 없다 — 어느 노드가 쟀는지 단정할 수 없다.\", \"BENCH_MODE_LITE\": \"full bench 정의(lite ∪ GuideLLM × 반복 ≥3)를 충족하지 않았다고 **기재된** 측정이다 — 선언된 lite-only 셀이거나 반복 불성립(기계 이벤트)으로 강등된 셀이다. 수치는 lite 스냅샷(또는 강등 셀의 대표 run 1회)이라 산포 추정치·인증서가 없다. 어느 쪽인지·강등 사유는 측정 구성 표(bench_mode_kind · downgrade_reason)가 말한다. bench_mode 를 읽지 못한 측정(미확정·미기재)에는 붙이지 않는다 — 모름을 lite 로 접으면 합성이다(그 사실은 카탈로그 bench_mode 칸이 말한다).\", \"HINT_MISSING_APPLIED_PATCH_FILE\": \"원장(또는 재구성)이 **적용됐다**고 말한 빌드 패치 스크립트가 출력 평면에 없다 — 적용 사실은 있으나 그 바이트를 실을 수 없다(01 적용 판정표에 `file_absent` 로 기재 · artifacts 가 기재).\", \"HINT_MISSING_BUILD_LEDGER\": \"빌드 원장(`/opt/easy-vllm/build_ledger.json`) 부재 — 도입 시점과 부재 원인은 미관측이다. 패치 적용 결과는 `applied_set.status: unobservable` 과 라벨된 재구성으로만 말한다(X9).\", \"HINT_MISSING_ATTESTATION_PARITY_ROWS\": \"노드 정합 attestation 파일은 있으나 대조 행이 없다 — v1 은 `checks` 가 비었고(소스빌드 트랙의 옛 스모크 판본은 대조 행을 쓰지 않았다), v2 는 `parity` 의 모든 축이 `unobservable` 이다. 두 노드의 vLLM SHA·torch·driver 일치는 관측되지 않았다.\", \"HINT_MISSING_CAMPAIGN_INSTANCE\": \"캠페인 인스턴스를 읽지 않았다(발행 기록 재생 — purge · 캠페인 밖 측정 · 보존됐으나 재생 모드라 읽지 않음 중 어느 쪽인지는 이 셀의 사유) — 셀 lockset·config·cell.status·journey·grounding 이 이 페이로드에 없다. 값의 지위와 여정 한 줄은 문서(testlog·devlog) 서사에서만 복원된다.\", \"HINT_MISSING_CELL_STATUS\": \"캠페인 셀 상태(`cell.status.json`) 부재 — 선언↔서빙 불일치 기록을 읽지 못한다.\", \"HINT_MISSING_LOCKSET\": \"셀 lockset 부재 — 서빙 노브의 출처(explorer 파생 vs 손작성)를 읽지 못한다.\", \"HINT_MISSING_CELL_CONFIG\": \"셀 config 부재 — 선언 축(declared_axes·target_gpu)을 읽지 못한다.\", \"HINT_MISSING_SWEEP\": \"이 측정의 스윕 디렉터리를 묶지 못했다 — `sweep_index.generated_utc` 가 인증서 `measured_utc` 와 같은 사본이 없다(셀 id 재사용으로 덮였을 수 있다 · 이름만으로 묶지 않는다).\", \"HINT_MISSING_SWEEP_RAW\": \"출력 평면의 스윕 디렉터리는 다른 측정으로 덮였고, 발행 기록의 simlog 사본(sweep_index·판정점 measured)만 묶었다 — 레벨별 원시·post_health·엔진 로그는 이 측정의 것이 아니다.\", \"HINT_MISSING_BUILD_IDENTITY\": \"측정 이미지(digest)를 로컬에서 관측하지 못했다 — 태그가 다른 이미지를 가리키거나 이미지가 지워졌다. 빌드 입력은 셀 env 선언과 인증서 지문으로만 말한다.\", \"HINT_MISSING_RESOLVE\": \"`output/<t>/resolved.json` 을 이 빌드에 묶지 못했다(부재 또는 다른 vLLM 버전의 해소값) — torch·NGC·CUDA 는 이미지 관측값만 싣는다.\", \"HINT_MISSING_SUB_RECIPE\": \"서브 레시피 미관측 — 서브 노드가 무엇으로 기동됐는지(`sub_recipe.json` · 서브 env 전달 파생)를 이 셀의 증거에서 읽지 못했다. 서브 기동 절차는 메인 쪽 기록만으로 말한다(artifacts 가 기재).\", \"HINT_MISSING_APPLIED_SET\": \"빌드 패치 적용 집합 미관측 — 어느 빌드 패치가 이 이미지에 실제로 적용됐는지(빌드 원장 · 재구성) 읽지 못했다. 실린 패치 파일은 '적용됨' 이 아니라 '후보' 다(artifacts 가 기재).\", \"HINT_MISSING_NATIVE_LAUNCH\": \"native 기동 기록 부재 — native 셀의 기동 절차(Ray head/worker · vllm serve 인자)를 기록한 실물이 증거에 없다. 기동 명령은 재구성이지 관측이 아니다(artifacts 가 기재).\"}").expect("constant"))
}
pub const DISTRIBUTED_MEASUREMENT_KEYS: [&str; 7] = ["bench_mode", "bench_mode_kind", "downgrade_reason", "bench_tool", "bench_tool_version", "repeats", "repeats_completed"];
pub const DATASET_KEYS: [&str; 10] = ["dataset_kind", "dataset_source", "dataset_id", "dataset_revision", "dataset_source_sha256", "dataset_column_map", "dataset_preprocess_strategy", "dataset_seed", "dataset_duplicate_sampling", "dataset_tokenizer"];
pub const DATASET_STATUS_RECORDED: &str = "recorded";
pub const DATASET_STATUS_INFERRED: &str = "inferred-synthetic";
pub const _CERT_MEASUREMENT_KEYS: [&str; 22] = ["benchmark_mode", "verdict", "decode_tps_conc1", "rubric_authority", "primary_source", "primary_tps", "floor_tps", "tolerance", "ratio_M_over_primary", "spec_on", "accept_len", "sweep_levels", "sweep_truncated", "lite_included", "lite_gen_tps_warm", "lite_gen_src", "lite_cold_ttft_ms", "lite_kv_gib", "lite_accept_len", "measured_utc", "bench_tool", "bench_tool_version"];
/// hintlib/evidence.py:674 _DOCKER_READ_ONLY (python literal as JSON)
pub fn _DOCKER_READ_ONLY() -> &'static J {
    static CELL: std::sync::OnceLock<J> = std::sync::OnceLock::new();
    CELL.get_or_init(|| crate::value::loads("[[\"image\", \"inspect\"], [\"history\"]]").expect("constant"))
}
pub const HISTORY_TRACK_MARK: &str = "docker history build-arg 관측";
pub const _OS_KEYS: [&str; 6] = ["os", "os_release", "os_name", "os_version", "kernel", "kernel_release"];
pub const _DECLARED_COPY: &str = "(manifest 선언 옮김)";
pub const MEASUREMENT_VERDICTS: [&str; 3] = ["PASS", "REFUTE", "OBSERVATION-ONLY"];
pub const OBSERVATION_ONLY_VERDICT: &str = "OBSERVATION-ONLY";
pub const _JUDGE_NO_BASELINE_VERDICT: &str = "NEEDS_RUBRIC";
/// hintlib/evidence.py:1561 _JUDGED_KEYS (python literal as JSON)
pub fn _JUDGED_KEYS() -> &'static J {
    static CELL: std::sync::OnceLock<J> = std::sync::OnceLock::new();
    CELL.get_or_init(|| crate::value::loads("[[\"verdict\", \"verdict\", \"verdict\", \"verdict\"], [\"decode_tps_conc1\", \"measured_decode_tps\", \"측정 decode t/s (동시성1)\", null], [\"rubric_authority\", \"rubric.authority\", \"루브릭 권한\", \"rubric_authority\"], [\"primary_tps\", \"rubric.primary\", \"루브릭 primary\", null], [\"primary_source\", \"rubric.source\", null, \"primary_source\"], [\"floor_tps\", \"rubric.floor\", \"floor (primary×(1−tol))\", \"floor_tps\"], [\"tolerance\", \"rubric.tolerance\", \"tolerance\", null], [\"ratio_M_over_primary\", \"rubric.ratio_M_over_primary\", \"ratio (M/primary)\", \"ratio_M_over_primary\"], [\"spec_on\", \"measured_spec_on\", null, null], [\"accept_len\", \"measured_accept_len\", null, null]]").expect("constant"))
}
pub const _CROSSCHECK_KEYS: [&str; 5] = ["verdict", "decode_tps_conc1", "rubric_authority", "floor_tps", "ratio_M_over_primary"];
pub const KV_CLAMP_KEY: &str = "kv-cache-memory-bytes";

/// `REL_MANIFEST_CONTRACT_RUNTIME` (what the launcher `REL_MANIFEST_CONTRACT` starts — kept for callers that stage a scratch
/// checkout). The launcher only names `manifest_contract_compat_owner.py` as a traceback frame path (the file need not exist ·
/// plan_26111615 D4), so it is not staged.
pub const REL_MANIFEST_CONTRACT_RUNTIME: [&str; 1] = [".claude/skills/adversarial-benchmark/rust/target/release/manifest_contract"];

crate::re_static!(_CELL_RE, r"^[A-Za-z0-9][A-Za-z0-9._-]*$");
crate::re_static!(_SHA40, r"^[0-9a-f]{40}$");
crate::re_static!(_UTC, r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z");
crate::re_static!(_REPORT_BORN, r"생성일\s+(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z)");
crate::re_static!(_HISTORY_ARGS, r"^(?:RUN\s+)?\|(\d+)\s+(.*)$");
crate::re_static!(
    _HF_URL,
    r"^(?:https?://)?(?:[^/@\s]+@)?(?:huggingface\.co|hf\.co)/([A-Za-z0-9][A-Za-z0-9._-]*/[A-Za-z0-9][A-Za-z0-9._-]*?)(?:\.git)?/?$"
);
pub const _UNOBSERVED: [&str; 7] = ["", "n/a", "na", "null", "none", "unknown", "?"];
pub const _TRUE: [&str; 4] = ["1", "true", "yes", "on"];
pub const _FALSE: [&str; 4] = ["0", "false", "no", "off"];
crate::re_static!(_INTERNAL_DOC_REF, r"코드맵|TL;DR");
crate::re_static!(_COMPOSE_BUILD_BLOCK, r"(?m)^([ \t]*)build:[ \t]*(?:#.*)?\n((?:\1[ \t]+\S.*\n?)+)");
crate::re_static!(_KERNEL_DRIVER, r"with kernel driver version (\d+(?:\.\d+)+)");
crate::re_static!(_DATASET_HOST_PATH_RE, r#"(?:^|(?<=[\s=:'"(\[{,]))/(?!app/)[^\s'"|,;)\]}]+"#);
crate::re_static!(_ABS_PATH_RE, r#"(?:^|[\s(=:'"])/(?:home|mnt|root|Users|srv|data)\b"#);

crate::re_static!(e1_digits_re, r"\d+");
crate::re_static!(e1_upper_key_re, r"[A-Z_][A-Z0-9_]*");
crate::re_static!(e1_compose_dockerfile_re, r"(?m)^\s*dockerfile:\s*([^#\n]*)");
crate::re_static!(e1_selector_re, r"\$\{BUILD_DOCKERFILE:?-([^}]*)\}");
crate::re_static!(e1_created_re, r"^(\d{4}-\d{2}-\d{2})T(\d{2}:\d{2}:\d{2})(?:\.\d+)?(Z|[+-]\d{2}:\d{2})$");
crate::re_static!(e1_project_re, r"vllm_(.+)_project");
crate::re_static!(e1_flat_line_re, r"^([A-Za-z0-9_]+):\s*(.*)$");
crate::re_static!(e1_env_line_re, r"([A-Za-z_][A-Za-z0-9_]*)=(.*)");
crate::re_static!(e1_ledger_arg_re, r"^ARG\s+([A-Za-z_][A-Za-z0-9_]*)", crate::pyre::M);
crate::re_static!(e1_ansi_re, "\\x1b\\[[0-9;]*[A-Za-z]");
crate::re_static!(e1_primary_re, r"^([0-9.]+)\s*t/s\s*\((.+)\)\s*$");
crate::re_static!(e1_tps_suffix_re, r"\s*t/s$");
crate::re_static!(e1_lead_digit_re, r"^[0-9]");

// ── private helpers (Python built-in semantics) ─────────────────────────────────────────────────────────────
fn py_isspace(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}
fn pstrip(s: &str) -> &str {
    s.trim_matches(py_isspace)
}
fn plower(s: &str) -> String {
    s.to_lowercase()
}
/// `str.splitlines()`.
fn splitlines(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut it = s.char_indices().peekable();
    while let Some((i, c)) = it.next() {
        if matches!(c, '\n' | '\r' | '\u{0b}' | '\u{0c}' | '\u{1c}' | '\u{1d}' | '\u{1e}' | '\u{85}' | '\u{2028}' | '\u{2029}') {
            out.push(&s[start..i]);
            let mut end = i + c.len_utf8();
            if c == '\r' {
                if let Some(&(_, '\n')) = it.peek() {
                    it.next();
                    end += 1;
                }
            }
            start = end;
        }
    }
    if start < s.len() {
        out.push(&s[start..]);
    }
    out
}
/// `Path.read_text(encoding="utf-8", errors="replace")`.
fn read_replace(p: &Path) -> Result<String, std::io::Error> {
    std::fs::read(p).map(|b| String::from_utf8_lossy(&b).into_owned())
}
/// `Path(s).name`.
fn pname(s: &str) -> String {
    let t = s.trim_end_matches('/');
    let last = t.rsplit('/').next().unwrap_or("");
    if last == "." {
        String::new()
    } else {
        last.to_string()
    }
}
/// `x or {}` then `.get(k)` (non-dict truthy values read as absent).
fn dget<'a>(v: &'a J, k: &str) -> &'a J {
    v.get(k).unwrap_or(&crate::value::NULL)
}
fn or_str(v: &J, default: &str) -> String {
    if v.truthy() {
        v.display()
    } else {
        default.to_string()
    }
}
fn py_tuple_repr(items: &[String]) -> String {
    let parts: Vec<String> = items.iter().map(|s| crate::value::py_repr(s)).collect();
    if parts.len() == 1 {
        format!("({},)", parts[0])
    } else {
        format!("({})", parts.join(", "))
    }
}
fn list_repr(items: &[&str]) -> String {
    let parts: Vec<String> = items.iter().map(|s| crate::value::py_repr(s)).collect();
    format!("[{}]", parts.join(", "))
}
fn in_set(s: &str, set: &[&str]) -> bool {
    set.iter().any(|x| *x == s)
}
fn j_in(v: &J, set: &[&str]) -> bool {
    matches!(v.as_str(), Some(s) if in_set(s, set))
}
fn jeq(v: &J, s: &str) -> bool {
    v.as_str() == Some(s)
}
/// Python `float(str)`; `None` = ValueError.
fn py_float(text: &str) -> Option<f64> {
    let t = pstrip(text);
    if t.is_empty() {
        return None;
    }
    let low = t.to_ascii_lowercase();
    let (sign, body) = match low.strip_prefix('-') {
        Some(b) => (-1.0, b),
        None => (1.0, low.strip_prefix('+').unwrap_or(&low)),
    };
    match body {
        "inf" | "infinity" => return Some(sign * f64::INFINITY),
        "nan" => return Some(f64::NAN),
        _ => {}
    }
    // underscores only between digits
    let b = body.as_bytes();
    for (i, &c) in b.iter().enumerate() {
        if c == b'_' && !(i > 0 && i + 1 < b.len() && b[i - 1].is_ascii_digit() && b[i + 1].is_ascii_digit()) {
            return None;
        }
        if !(c.is_ascii_digit() || matches!(c, b'.' | b'e' | b'+' | b'-' | b'_')) {
            return None;
        }
    }
    let clean: String = body.chars().filter(|c| *c != '_').collect();
    if !clean.bytes().any(|c| c.is_ascii_digit()) {
        return None;
    }
    clean.parse::<f64>().ok().map(|f| sign * f)
}
fn py_type_of_io(e: &std::io::Error) -> &'static str {
    match e.raw_os_error() {
        Some(2) => "FileNotFoundError",
        Some(13) | Some(1) => "PermissionError",
        Some(21) => "IsADirectoryError",
        Some(20) => "NotADirectoryError",
        _ => "OSError",
    }
}
fn utf8_err_text(e: &std::string::FromUtf8Error) -> String {
    let u = e.utf8_error();
    let pos = u.valid_up_to();
    let bytes = e.as_bytes();
    let b = bytes.get(pos).copied().unwrap_or(0);
    match u.error_len() {
        None => format!("'utf-8' codec can't decode byte 0x{b:02x} in position {pos}: unexpected end of data"),
        Some(_) if (0x80..0xc2).contains(&b) || b >= 0xf5 => {
            format!("'utf-8' codec can't decode byte 0x{b:02x} in position {pos}: invalid start byte")
        }
        Some(_) => format!("'utf-8' codec can't decode byte 0x{b:02x} in position {pos}: invalid continuation byte"),
    }
}
/// `Path.read_text(encoding="utf-8")` with the Python exception (type, text).
fn read_strict(p: &Path) -> Result<String, (String, String)> {
    let bytes = std::fs::read(p).map_err(|e| (py_type_of_io(&e).to_string(), core::io_err_text(&e, p)))?;
    String::from_utf8(bytes).map_err(|e| ("UnicodeDecodeError".to_string(), utf8_err_text(&e)))
}
/// `Path.resolve() != Path.absolute()` (a symlink somewhere on the path).
fn is_symlinked(p: &Path) -> bool {
    let abs = if p.is_absolute() { p.to_path_buf() } else { std::env::current_dir().unwrap_or_default().join(p) };
    core::resolve(p) != abs
}
fn path_of(v: &J) -> Option<PathBuf> {
    match v {
        J::Str(s) => Some(PathBuf::from(s)),
        J::Dict(m) => m.get("__path__").and_then(J::as_str).map(PathBuf::from),
        _ => None,
    }
}
fn ev_str(ev: &J, k: &str) -> Option<String> {
    ev.get(k).and_then(J::as_str).map(str::to_string)
}
fn m_get(m: &Map, k: &str) -> J {
    m.get(k).cloned().unwrap_or(J::Null)
}
/// The owner module of a Python import is present (its native launcher `<stem>.sh` or the source) — else the same
/// fail-closed answer as `core.load_owner_module`.
fn owner_present(repo: &Path, rel_py: &str) -> HR<()> {
    // presence = the owner's native launcher only (plan_26111615 D4 — a `.py` alone is not an owner).
    if core::owner_launcher(repo, rel_py).is_file() {
        return Ok(());
    }
    core::fail(
        "HINT_OWNER_MODULE_MISSING",
        format!("소유 모듈 부재: {rel_py}"),
        Some("대체 구현으로 계속하지 않는다 — 그 스킬이 설치된 체크아웃에서 실행한다."),
    )
}

// ── CellEvidence ─────────────────────────────────────────────────────────────────────────────────────────────
/// The `CellEvidence` dataclass field names in declaration order.
pub const CELL_EVIDENCE_FIELDS: [&str; 36] = [
    "mode", "campaign_id", "cell", "node", "topology", "declaration", "cell_status", "lockset", "cell_config", "triplet",
    "sweep", "certificate", "certificate_path", "bench_report_path", "report_kind", "pointers", "attestation",
    "image_ledger", "build_identity", "resolve", "manifest", "publication", "lineage_seeds", "missing", "serve_proof",
    "plane", "native_install_path", "pip_freeze_path", "pip_freeze_paths", "cleanup_attestation_path",
    "cleanup_attestation", "native_launch_path", "native_launch_error", "phases", "sources", "repo",
];

/// `CellEvidence(mode=, campaign_id=, cell=, node=, topology=)` as a J dict: every field in declaration order with its default.
pub fn new_cell_evidence(mode: &str, campaign_id: Option<&str>, cell: &str, node: &str, topology: &str) -> J {
    let mut m = Map::new();
    for f in CELL_EVIDENCE_FIELDS {
        let v = match f {
            "mode" => J::from(mode),
            "campaign_id" => J::from(campaign_id),
            "cell" => J::from(cell),
            "node" => J::from(node),
            "topology" => J::from(topology),
            "triplet" | "build_identity" | "lineage_seeds" | "pip_freeze_paths" | "phases" | "sources" => J::dict(),
            "pointers" | "missing" => J::list(),
            _ => J::Null,
        };
        m.insert(f, v);
    }
    J::Dict(m)
}

// ── 작은 도구 ─────────────────────────────────────────────────────────────────────────────────────────────────
/// `Path(repo).resolve()`.
pub fn _repo(repo: &Path) -> HR<PathBuf> {
    Ok(core::resolve(repo))
}

pub fn _rel(repo: &Path, path: &Path) -> HR<String> {
    core::rel(repo, path)
}

/// Absent = `Null` · a broken file is `HINT_EVIDENCE_UNREADABLE` (never read as absent).
pub fn _read_json_opt(path: &Path, what: &str) -> HR<J> {
    if !path.exists() {
        return Ok(J::Null);
    }
    let fail = |ty: &str, e: &str| -> HR<J> {
        core::fail(
            "HINT_EVIDENCE_UNREADABLE",
            format!("{what} 를 읽을 수 없다({}): {ty}: {e}", pname(&core::posix(path))),
            Some("파일을 고치거나 지운다 — 깨진 증거를 부재로 읽지 않는다."),
        )
    };
    let text = match read_strict(path) {
        Ok(t) => t,
        Err((ty, e)) => return fail(&ty, &e),
    };
    match crate::value::loads(&text) {
        Ok(v) => Ok(v),
        Err(e) => fail("JSONDecodeError", &e),
    }
}

pub fn _unobserved(value: &J) -> HR<bool> {
    Ok(match value {
        J::Null => true,
        J::Str(s) => in_set(&plower(pstrip(s)), &_UNOBSERVED),
        _ => false,
    })
}
fn unobs(v: &J) -> bool {
    _unobserved(v).unwrap_or(false)
}

pub fn _norm_value(value: &J) -> HR<J> {
    if unobs(value) {
        return Ok(J::Null);
    }
    Ok(match value {
        J::Str(s) => J::Str(pstrip(s).to_string()),
        other => other.clone(),
    })
}

pub fn _as_int(value: &J) -> HR<Option<i64>> {
    Ok(match value {
        J::Int(i) => Some(*i),
        J::Str(s) if e1_digits_re().is_full(pstrip(s)) => pstrip(s).parse::<i64>().ok(),
        _ => None,
    })
}

pub fn _boolish(value: &J) -> HR<Option<bool>> {
    Ok(match value {
        J::Bool(b) => Some(*b),
        J::Str(s) => {
            let low = plower(pstrip(s));
            if in_set(&low, &_TRUE) {
                Some(true)
            } else if in_set(&low, &_FALSE) {
                Some(false)
            } else {
                None
            }
        }
        _ => None,
    })
}

pub fn _check_campaign_id(campaign_id: &J) -> HR<String> {
    let ok = matches!(campaign_id.as_str(), Some(s) if !s.is_empty() && !s.contains('/') && s != "." && s != "..");
    if !ok {
        return core::fail(
            "HINT_CAMPAIGN_ID_REQUIRED",
            format!("명시 캠페인 id 가 필요하다: {}", campaign_id.repr()),
            Some("`--campaign <camp-id>` 로 준다 — ACTIVE 포인터는 읽지 않는다(2026-09-07 Q6)."),
        );
    }
    let s = campaign_id.as_str().unwrap();
    if in_set(s, &RESERVED_CAMPAIGN_IDS) {
        return core::fail(
            "HINT_CAMPAIGN_ID_RESERVED",
            format!("{} 는 캠페인 인스턴스가 아니다(예약 이름).", campaign_id.repr()),
            Some("_bootstrap 은 캠페인 밖 대기실이고 _template 은 뼈대다 — 발행 대상 캠페인 id 를 준다."),
        );
    }
    Ok(s.to_string())
}

pub fn _check_cell(cell: &J) -> HR<String> {
    match cell.as_str() {
        Some(s) if _CELL_RE().is_match(s) => Ok(s.to_string()),
        _ => core::fail("HINT_CELL_INVALID", format!("셀 id 가 비었거나 경로 문자를 포함한다: {}", cell.repr()), None),
    }
}

pub fn _check_node(node: &J) -> HR<String> {
    match node.as_str() {
        Some(s) if in_set(s, &NODE_AXIS) => Ok(s.to_string()),
        _ => core::fail(
            "HINT_NODE_AXIS_INVALID",
            format!("노드 축은 {} 중 하나다: {}", list_repr(&NODE_AXIS), node.repr()),
            None,
        ),
    }
}

// ── 소유 모듈 적재 ─────────────────────────────────────────────────────────────────────────────────────────────
/// The completion gate's native launcher (`completion_gate.sh` next to the original path) — the gate that runs.
pub fn _cgate_launcher_rel() -> String {
    core::REL_COMPLETION_GATE.strip_suffix(".py").map(|s| format!("{s}.sh")).unwrap_or_else(|| core::REL_COMPLETION_GATE.to_string())
}

/// The Rust source of the gate the launcher runs (policy-gate), read from this checkout: the binary the launcher
/// starts is built from it, so its public API is the gate's API (plan_26111615 — the `.py` is a reference original).
pub const REL_COMPLETION_GATE_RS: &str = ".claude/rust/policy-gate/src/completion_gate.rs";

pub fn _cgate_rust_source() -> PathBuf {
    // The checkout this binary was built from (`<root>/.claude/skills/hint-publisher/rust`): fixture repositories that
    // redirect HINT_SKILL_DIR still run this checkout's gate binary, so its source is the API authority. A moved or
    // stripped checkout leaves the file absent = HINT_GATE_API_MISSING (fail closed, never a guess).
    let built = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = built.parent().and_then(Path::parent).and_then(Path::parent).and_then(Path::parent).map(Path::to_path_buf).unwrap_or_default();
    root.join(REL_COMPLETION_GATE_RS)
}

pub fn _cgate_path(repo: &Path) -> HR<PathBuf> {
    // The native launcher only (what `run_owner` starts · plan_26111615 D4: neither the canonical nor a colocated `.py` is an
    // owner). The message keeps the original wording (it names where the gate used to live).
    let launcher = repo.join(_cgate_launcher_rel());
    if launcher.is_file() {
        return Ok(launcher);
    }
    core::fail(
        "HINT_OWNER_MODULE_MISSING",
        format!("completion_gate 부재: {} · 동거 사본 completion_gate.py", core::REL_COMPLETION_GATE),
        Some("대체 구현으로 계속하지 않는다(2026-07-31 ModuleNotFoundError 선례) — 정책 런타임이 있는 체크아웃에서 실행한다."),
    )
}

/// The completion_gate in-process API this module uses.
/// OWNER-MIRROR: .claude/policies/runtime/completion_gate.py::{parse_flat_certificate, certificate_run_key,
/// CERTIFICATE_FIELD_MAP, STRONG_IDENTITY_FIELDS, lexical_components, manifest_rubric_contract,
/// PII_MACHINE_RAW_EXEMPTION_REASON, certificate_requirement_authority, certificate_waived,
/// CERTIFICATE_NON_ISSUING_AUTHORITIES, OBSERVATION_ONLY_VERDICT, OBSERVATION_ONLY_JUDGE_VERDICT,
/// ANNOTATED_NON_PASS_VERDICTS, MANIFEST_RUBRIC_SOURCES}. No Rust form of the gate exists; `cgate()` keeps the
/// presence check (gate file absent = `HINT_OWNER_MODULE_MISSING`) and checks that the file still defines the
/// required public names (`HINT_GATE_API_MISSING`), then answers with these mirrored rules.
#[derive(Clone, Debug)]
pub struct CGate {
    pub path: PathBuf,
}

impl CGate {
    pub const CERTIFICATE_FIELD_MAP: [(&'static str, &'static str); 6] = [
        ("model", "model"),
        ("gpu", "gpu_model"),
        ("vllm", "vllm_version"),
        ("quant", "quantization"),
        ("topology", "topology"),
        ("tp", "tensor_parallel_size"),
    ];
    pub const STRONG_IDENTITY_FIELDS: [&'static str; 6] = ["model", "gpu", "vllm", "quant", "topology", "tp"];
    pub const CERTIFICATE_RUN_KEY_FIELDS: [&'static str; 7] =
        ["model", "gpu_model", "vllm_version", "quantization", "topology", "tensor_parallel_size", "measured_utc"];
    pub const RUBRIC_AUTHORITIES: [&'static str; 3] = ["weak", "explicit", "explore"];
    pub const OBSERVATION_ONLY_VERDICT: &'static str = "OBSERVATION-ONLY";
    pub const OBSERVATION_ONLY_JUDGE_VERDICT: &'static str = "NEEDS_RUBRIC";
    pub const ANNOTATED_NON_PASS_VERDICTS: [&'static str; 2] = ["REFUTE", "FAIL"];
    pub const MANIFEST_RUBRIC_SOURCES: [&'static str; 1] = ["verdict_json"];
    pub const PII_MACHINE_RAW_EXEMPTION_REASON: &'static str = "machine-raw(docs.md §PII 판정 대상 밖)";
    pub const CERTIFICATE_ISSUING_AUTHORITIES: [&'static str; 1] = ["explicit"];
    pub const CERTIFICATE_NON_ISSUING_AUTHORITIES: [&'static str; 2] = ["weak", "explore"];

    pub fn field_map(&self, f: &str) -> &'static str {
        Self::CERTIFICATE_FIELD_MAP.iter().find(|(k, _)| *k == f).map(|(_, v)| *v).unwrap_or("")
    }

    /// `parse_flat_certificate(text) -> (fields, ok)`.
    pub fn parse_flat_certificate(&self, text: &str) -> (J, bool) {
        let mut fields = Map::new();
        for line in splitlines(text) {
            let stripped = pstrip(line);
            if stripped.is_empty() || stripped.starts_with('#') {
                continue;
            }
            if line != stripped {
                return (J::dict(), false);
            }
            let Some(m) = e1_flat_line_re().match_at_start(line) else { return (J::dict(), false) };
            let key = crate::pyre::group(&m, 1).unwrap_or("").to_string();
            let mut value = pstrip(crate::pyre::group(&m, 2).unwrap_or("")).to_string();
            let cs: Vec<char> = value.chars().collect();
            if cs.len() >= 2 && cs[0] == '"' && cs[cs.len() - 1] == '"' {
                value = cs[1..cs.len() - 1].iter().collect::<String>().replace("\\\"", "\"");
            }
            if fields.contains_key(&key) {
                return (J::dict(), false);
            }
            fields.insert(key, value);
        }
        (J::Dict(fields), true)
    }

    /// `certificate_run_key(cert) -> tuple | None`.
    pub fn certificate_run_key(&self, cert: &J) -> Option<Vec<String>> {
        let mut out = Vec::new();
        for k in Self::CERTIFICATE_RUN_KEY_FIELDS {
            let v = cert.get(k)?;
            if v.is_null() {
                return None;
            }
            let s = pstrip(&v.display()).to_string();
            if s.is_empty() {
                return None;
            }
            out.push(s);
        }
        Some(out)
    }

    /// `lexical_components(manifest_dir, repo_root, rel_path) -> (status, parts | None)` (pure path normalization).
    pub fn lexical_components(&self, manifest_dir: &Path, repo_root: &Path, rel_path: &str) -> (String, Option<Vec<String>>) {
        let joined = manifest_dir.join(rel_path);
        // os.path.normpath
        let s = core::posix(&joined);
        let abs = s.starts_with('/');
        let mut parts: Vec<&str> = Vec::new();
        for c in s.split('/') {
            match c {
                "" | "." => {}
                ".." => {
                    if parts.last().map_or(false, |p| *p != "..") {
                        parts.pop();
                    } else if !abs {
                        parts.push("..");
                    }
                }
                other => parts.push(other),
            }
        }
        let lexical = PathBuf::from(if abs { format!("/{}", parts.join("/")) } else { parts.join("/") });
        match lexical.strip_prefix(repo_root) {
            Ok(r) => (
                "ok".into(),
                Some(r.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).filter(|p| p != "." && !p.is_empty()).collect()),
            ),
            Err(_) => ("repo_escape".into(), None),
        }
    }

    fn certificate_number(raw: &J) -> Option<f64> {
        if raw.is_null() {
            return None;
        }
        let text = pstrip(&raw.display()).to_string();
        if text.is_empty() || text.to_uppercase() == "N/A" {
            return None;
        }
        let v = py_float(&text)?;
        if v.is_nan() || v.is_infinite() {
            return None;
        }
        Some(v)
    }

    /// `manifest_rubric_contract(benchmark) -> dict` (problems = list of [code, message]).
    pub fn manifest_rubric_contract(&self, benchmark: &J) -> J {
        let mut out = crate::jd! {"declared" => false, "ok" => false, "authority" => J::Null, "floor_tps" => J::Null,
            "ratio_M_over_primary" => J::Null, "primary_source" => J::Null, "problems" => J::list()};
        if !benchmark.is_dict() {
            return out;
        }
        let raw_authority = benchmark.get_str("rubric_authority").map(|s| pstrip(s).to_string()).unwrap_or_default();
        if raw_authority.is_empty() || raw_authority.to_uppercase() == "N/A" {
            return out;
        }
        out.set("declared", true);
        let mut problems: Vec<J> = Vec::new();
        if in_set(&raw_authority, &Self::RUBRIC_AUTHORITIES) {
            out.set("authority", raw_authority.clone());
        } else {
            problems.push(crate::jl![
                "MANIFEST_RUBRIC_AUTHORITY_UNKNOWN",
                format!(
                    "benchmark.rubric_authority={} is not one of {} -- an unknown rubric authority cannot be reasoned about, fail closed",
                    crate::value::py_repr(&raw_authority),
                    list_repr(&Self::RUBRIC_AUTHORITIES)
                )
            ]);
        }
        let raw_source = benchmark.get_str("rubric_source").map(|s| pstrip(s).to_string()).unwrap_or_default();
        if !in_set(&raw_source, &Self::MANIFEST_RUBRIC_SOURCES) {
            problems.push(crate::jl![
                "MANIFEST_RUBRIC_PROVENANCE_MISSING",
                format!(
                    "benchmark.rubric_source={} is not one of {} -- 출처 표시가 없는 rubric 값은 판정기 산출물 파생분과 손저작을 데이터에서 구분할 수 없다(헌법 §결정론 규율). 승격을 열지 않는다",
                    benchmark["rubric_source"].repr(),
                    list_repr(&Self::MANIFEST_RUBRIC_SOURCES)
                )
            ]);
        }
        let floor = Self::certificate_number(&benchmark["floor_tps"]);
        out.set("floor_tps", J::from(floor));
        if floor.map_or(true, |f| f <= 0.0) {
            problems.push(crate::jl![
                "MANIFEST_RUBRIC_FLOOR_INVALID",
                format!(
                    "benchmark.floor_tps={} is not a finite positive number -- floor<=0 means the rubric imposed NO threshold (공허 PASS): every measurement passes. Not promotable in ANY authority.",
                    benchmark["floor_tps"].repr()
                )
            ]);
        }
        let ratio = Self::certificate_number(&benchmark["ratio_M_over_primary"]);
        out.set("ratio_M_over_primary", J::from(ratio));
        if ratio.is_none() {
            problems.push(crate::jl![
                "MANIFEST_RUBRIC_RATIO_MISSING",
                format!(
                    "benchmark.ratio_M_over_primary={} is not a finite number -- the measurement-vs-rubric indicator is absent, so the verdict cannot be audited",
                    benchmark["ratio_M_over_primary"].repr()
                )
            ]);
        }
        let ps = benchmark.get_str("primary_source").map(|s| pstrip(s).to_string()).unwrap_or_default();
        out.set("primary_source", if ps.is_empty() { J::Null } else { J::from(ps.clone()) });
        if ps.is_empty() || ps.to_uppercase() == "N/A" {
            problems.push(crate::jl![
                "MANIFEST_RUBRIC_SOURCE_MISSING",
                format!(
                    "benchmark.primary_source={} is absent -- which rubric slot won cannot be established (surfaced, never matched as a string: the numeric floor/ratio contract above is the gate)",
                    benchmark["primary_source"].repr()
                )
            ]);
        }
        out.set("ok", problems.is_empty());
        out.set("problems", J::List(problems));
        out
    }

    /// `certificate_requirement_authority(benchmark) -> str | None`.
    pub fn certificate_requirement_authority(&self, benchmark: &J) -> Option<String> {
        let a = benchmark.get_str("rubric_authority")?;
        let s = benchmark.get_str("rubric_source")?;
        let (a, s) = (pstrip(a), pstrip(s));
        if !in_set(s, &Self::MANIFEST_RUBRIC_SOURCES) || !in_set(a, &Self::RUBRIC_AUTHORITIES) {
            return None;
        }
        Some(a.to_string())
    }

    /// `certificate_waived(task_class, verdict, rubric_authority) -> bool`.
    pub fn certificate_waived(&self, task_class: &str, verdict: &J, rubric_authority: &J) -> bool {
        task_class == "full_benchmark" && jeq(verdict, "PASS") && j_in(rubric_authority, &Self::CERTIFICATE_NON_ISSUING_AUTHORITIES)
    }
}

/// `cgate(repo)` — the completion_gate API (OWNER-MIRROR, see [`CGate`]). Gate absent = `HINT_OWNER_MODULE_MISSING`;
/// a gate file that no longer defines a required public name = `HINT_GATE_API_MISSING`.
pub fn cgate(repo: &Path) -> HR<CGate> {
    let path = _cgate_path(&core::resolve(repo))?;
    // A launcher is judged by the Rust gate source it runs; a `.py` copy by its own text (same required names).
    let native = path.extension().map_or(false, |e| e == "sh");
    let api_path = if native { _cgate_rust_source() } else { path.clone() };
    let text = read_replace(&api_path).unwrap_or_default();
    let lines = splitlines(&text);
    let absent: Vec<&str> = _GATE_REQUIRED_API
        .iter()
        .copied()
        .filter(|a| {
            !lines.iter().any(|l| {
                if native {
                    ["pub fn ", "pub const ", "pub static "].iter().any(|k| {
                        l.strip_prefix(k).and_then(|r| r.strip_prefix(*a)).map_or(false, |r| r.starts_with('(') || r.starts_with(':') || r.starts_with('<'))
                    })
                } else {
                    l.strip_prefix(&format!("def {a}")).map_or(false, |r| r.starts_with('('))
                        || l.strip_prefix(*a).map_or(false, |r| r.trim_start().starts_with('='))
                }
            })
        })
        .collect();
    if !absent.is_empty() {
        return core::fail(
            "HINT_GATE_API_MISSING",
            format!("completion_gate 에 공개 API {} 가 없다({}).", list_repr(&absent), pname(&core::posix(&api_path))),
            Some("동거 사본이 낡았을 수 있다 — 정본(policy-gate `completion_gate.rs` · 런처 `completion_gate.sh`)으로 맞춘다."),
        );
    }
    Ok(CGate { path })
}

/// `_validator(repo)` — owner present check (campaign_template_validator; callers use its native launcher).
pub fn _validator(repo: &Path) -> HR<J> {
    owner_present(repo, core::REL_CAMPAIGN_VALIDATOR).map(|_| J::Null)
}

/// `_rbs(repo)` — render_bench_section is ported in this crate (`crate::render_bench_section`).
pub fn _rbs(repo: &Path) -> HR<J> {
    owner_present(repo, core::REL_RENDER_BENCH_SECTION).map(|_| J::Null)
}

/// `_smoke_model(repo)` — owner present check (check_smoke_model; callers use its native launcher).
pub fn _smoke_model(repo: &Path) -> HR<J> {
    owner_present(repo, REL_CHECK_SMOKE_MODEL).map(|_| J::Null)
}

/// `_slave_forward(repo)` — owner present check; the two rules used here are mirrored below.
pub fn _slave_forward(repo: &Path) -> HR<J> {
    owner_present(repo, core::REL_SLAVE_FORWARD).map(|_| J::Null)
}

/// OWNER-MIRROR: .claude/skills/upstream-version-watch/scripts/slave_forward.py::read_env_first
fn sf_read_env_first(path: &Path) -> J {
    let mut out = Map::new();
    if !path.is_file() {
        return J::Dict(out);
    }
    let text = read_replace(path).unwrap_or_default();
    for raw in splitlines(&text) {
        if let Some(m) = e1_env_line_re().fullmatch(raw) {
            let k = crate::pyre::group(&m, 1).unwrap_or("");
            if !out.contains_key(k) {
                out.insert(k, crate::pyre::group(&m, 2).unwrap_or(""));
            }
        }
    }
    J::Dict(out)
}

/// OWNER-MIRROR: slave_forward.py::_strip_comment
fn sf_strip_comment(line: &str) -> String {
    let mut out = String::new();
    let mut quote: Option<char> = None;
    let mut prev = ' ';
    for ch in line.chars() {
        if let Some(q) = quote {
            out.push(ch);
            if ch == q {
                quote = None;
            }
        } else if ch == '"' || ch == '\'' {
            quote = Some(ch);
            out.push(ch);
        } else if ch == '#' && (prev == ' ' || prev == '\t') {
            break;
        } else {
            out.push(ch);
        }
        prev = ch;
    }
    out.trim_end_matches(py_isspace).to_string()
}

/// OWNER-MIRROR: slave_forward.py::compose_default (ForwardError = `py:ForwardError` "<code>: <message>").
fn sf_compose_default(path: &Path, key: &str) -> HR<Option<String>> {
    if !path.is_file() {
        return Err(HintError::py("ForwardError", format!("SF_COMPOSE_ABSENT: compose 파일이 없다: {}", core::posix(path))));
    }
    let text = match read_strict(path) {
        Ok(t) => t,
        Err((ty, e)) => return Err(HintError::py(&ty, e)),
    };
    let head = crate::pyre::Re::new(&format!(r"(?<!\$)\$\{{{}:?-", fancy_regex::escape(key)), 0);
    for raw in splitlines(&text) {
        let line = sf_strip_comment(raw);
        let Some(m) = head.search(&line) else { continue };
        let i = m.get(0).unwrap().end();
        let mut depth = 1;
        for (j, c) in line[i..].char_indices() {
            if c == '{' {
                depth += 1;
            } else if c == '}' {
                depth -= 1;
                if depth == 0 {
                    return Ok(Some(line[i..i + j].to_string()));
                }
            }
        }
        return Err(HintError::py(
            "ForwardError",
            format!("SF_COMPOSE_SHAPE: ${{{key}:-…}} 의 괄호가 한 줄 안에서 닫히지 않는다: {}", crate::value::py_repr(pstrip(raw))),
        ));
    }
    Ok(None)
}

/// YAML through the owner loader's rules (`manifest_contract._load_manifest` = PyYAML safe_load → `value::yaml_loads`).
pub fn _owner_yaml(repo: &Path, path: &Path, what: &str) -> HR<J> {
    owner_present(repo, REL_MANIFEST_CONTRACT)?;
    let text = match read_strict(path) {
        Ok(t) => t,
        Err((ty, e)) => {
            return core::fail("HINT_EVIDENCE_UNREADABLE", format!("{what} YAML 파손({}): {ty}: {e}", pname(&core::posix(path))), None)
        }
    };
    crate::value::yaml_loads(&text).or_else(|e| {
        core::fail("HINT_EVIDENCE_UNREADABLE", format!("{what} YAML 파손({}): YAMLError: {e}", pname(&core::posix(path))), None)
    })
}

pub fn _manifest(repo: &Path, topology: &str) -> HR<(Option<J>, String)> {
    let p = repo.join("output").join(topology).join("manifest.yaml");
    if !p.is_file() {
        return Ok((None, format!("output/{topology}/manifest.yaml 부재")));
    }
    let doc = _owner_yaml(repo, &p, "manifest")?;
    if !doc.is_dict() {
        return core::fail("HINT_EVIDENCE_UNREADABLE", format!("output/{topology}/manifest.yaml 최상위가 사전이 아니다."), None);
    }
    Ok((Some(doc), format!("output/{topology}/manifest.yaml(manifest_contract 로더)")))
}

// ── 인증서 · 게이트 ──────────────────────────────────────────────────────────────────────────────────────────
/// flat certificate → {key: str} (the gate's parser, mirrored).
pub fn parse_certificate(repo: &Path, path: &J) -> HR<J> {
    let repo = core::resolve(repo);
    let p = path_of(path).unwrap_or_default();
    let rel = _rel(&repo, &p)?;
    let f = repo.join(&rel);
    if !f.is_file() {
        return core::fail("HINT_CERTIFICATE_ABSENT", format!("인증서 부재: {rel}"), None);
    }
    let text = match read_strict(&f) {
        Ok(t) => t,
        Err((ty, _)) => return core::fail("HINT_CERTIFICATE_UNREADABLE", format!("인증서를 읽을 수 없다({rel}): {ty}"), None),
    };
    let (fields, ok) = cgate(&repo)?.parse_flat_certificate(&text);
    if !ok {
        return core::fail(
            "HINT_CERTIFICATE_UNPARSEABLE",
            format!("flat 인증서 형식이 아니다(들여쓰기·중복 키·잘못된 행): {rel}"),
            Some("결정론 발행기(publish_benchmark_record.py)가 쓴 원본을 바인딩한다 — 손으로 고친 인증서를 읽지 않는다."),
        );
    }
    if !fields.truthy() {
        return core::fail(
            "HINT_CERTIFICATE_UNPARSEABLE",
            format!("인증서가 비었다(주석뿐): {rel}"),
            Some("빈 인증서로 페이로드를 짓지 않는다 — 벤치마커가 발행한 원본을 바인딩한다."),
        );
    }
    Ok(fields)
}

pub fn _identity_from_certificate(repo: &Path, cert: &J) -> HR<J> {
    let g = cgate(repo)?;
    let mut out = Map::new();
    for f in CGate::STRONG_IDENTITY_FIELDS {
        out.insert(f, _norm_value(&cert[g.field_map(f)])?);
    }
    let tp = _as_int(&m_get(&out, "tp"))?;
    out.insert("tp", J::from(tp));
    Ok(J::Dict(out))
}

/// `completion_gate.py authorize --mode promotion` once, through the gate's native launcher (`completion_gate.sh`).
/// Launcher absent / not runnable = `HINT_GATE_SUBPROCESS_UNAVAILABLE` (exit 2) · stdout not JSON =
/// `HINT_GATE_OUTPUT_UNREADABLE` (exit 2) · denied = `HINT_GATE_DENIED` (message = gate stdout).
pub fn authorize(repo: &Path, manifest_path: &J, action: &str) -> HR<J> {
    let repo = core::resolve(repo);
    let mp = path_of(manifest_path).unwrap_or_default();
    let manifest = if mp.is_absolute() { mp } else { repo.join(mp) };
    let _gate_path = _cgate_path(&repo)?;
    let args: Vec<String> = vec![
        "authorize".into(),
        "--manifest".into(),
        core::posix(&manifest),
        "--mode".into(),
        "promotion".into(),
        "--action".into(),
        action.into(),
        "--repo-root".into(),
        core::posix(&repo),
    ];
    let env = vec![("PYTHONDONTWRITEBYTECODE".to_string(), "1".to_string())];
    let proc = match core::run_owner(&repo, core::REL_COMPLETION_GATE, &args, None, &env, false) {
        Ok(p) => p,
        Err(e) => {
            return core::fail_exit(
                "HINT_GATE_SUBPROCESS_UNAVAILABLE",
                format!("completion_gate.py authorize 를 실행할 수 없다: {}", e.message),
                None,
                2,
            )
        }
    };
    let parsed = match crate::value::loads(&proc.stdout) {
        Ok(v) => v,
        Err(_) => {
            return core::fail_exit(
                "HINT_GATE_OUTPUT_UNREADABLE",
                format!(
                    "completion_gate.py authorize (exit={}) 의 stdout 이 JSON 이 아니다; stderr={}",
                    proc.returncode,
                    crate::value::py_repr(&core::head_chars(pstrip(&proc.stderr), 500))
                ),
                None,
                2,
            )
        }
    };
    if proc.returncode == 0 && parsed.is_dict() && parsed.get("allowed") == Some(&J::Bool(true)) {
        return Ok(parsed);
    }
    let msg = if proc.stdout.ends_with('\n') { proc.stdout.clone() } else { format!("{}\n", proc.stdout) };
    core::fail_exit(
        "HINT_GATE_DENIED",
        msg,
        Some("게이트 JSON 의 reason_codes 를 해소한다 — 우회하지 않는다(D3)."),
        if proc.returncode != 0 { proc.returncode } else { 1 },
    )
}

fn default_repo() -> PathBuf {
    let s = core::skill_dir();
    s.ancestors().nth(3).map(Path::to_path_buf).unwrap_or(s)
}

/// The single decider of what opens a bench_report binding when there is no certificate.
pub fn no_cert_binding_source(manifest: &J, repo: Option<&Path>) -> HR<Option<String>> {
    if !manifest.is_dict() {
        return Ok(None);
    }
    let benchmark = match manifest.get("benchmark") {
        Some(b @ J::Dict(_)) => b.clone(),
        _ => J::dict(),
    };
    if jeq(&manifest["task_class"], "hint_map_only") {
        return Ok(Some("hint_map_only".into()));
    }
    let gate = cgate(&repo.map(Path::to_path_buf).unwrap_or_else(default_repo))?;
    let verdict = benchmark["verdict"].clone();
    if jeq(&verdict, CGate::OBSERVATION_ONLY_VERDICT) {
        let sourced = j_in(&benchmark["rubric_source"], &CGate::MANIFEST_RUBRIC_SOURCES);
        let claimed = ["floor_tps", "ratio_M_over_primary", "primary_source"]
            .iter()
            .any(|k| !matches!(benchmark.get(k), None | Some(J::Null)) && !jeq(&benchmark[*k], ""));
        return Ok(if sourced && !claimed { Some("observation_only".into()) } else { None });
    }
    let rubric = gate.manifest_rubric_contract(&benchmark);
    if !(rubric["declared"].truthy() && rubric["ok"].truthy()) {
        return Ok(None);
    }
    let Some(authority) = gate.certificate_requirement_authority(&benchmark) else { return Ok(None) };
    if j_in(&verdict, &CGate::ANNOTATED_NON_PASS_VERDICTS) {
        return Ok(Some(authority));
    }
    if jeq(&verdict, "PASS") && in_set(&authority, &CGate::CERTIFICATE_NON_ISSUING_AUTHORITIES) {
        return Ok(Some(authority));
    }
    Ok(None)
}

pub fn binding_artifact_path(manifest: &J, repo: Option<&Path>) -> HR<Option<String>> {
    let ev = match manifest.get("evidence") {
        Some(e @ J::Dict(_)) => e.clone(),
        _ => J::dict(),
    };
    let path = &ev["certificate"]["path"];
    if path.truthy() {
        return Ok(Some(path.display()));
    }
    if no_cert_binding_source(manifest, repo)?.map_or(false, |s| !s.is_empty()) {
        let p = &ev["bench_report"]["path"];
        return Ok(if p.is_null() { None } else { Some(p.display()) });
    }
    Ok(None)
}

pub fn promotion_binding_problems(manifest: &J, tag: &str, topology: &str, anchor: &str) -> HR<Vec<(String, String)>> {
    let Some(pt) = manifest.get("promotion_target").filter(|v| v.is_dict()) else {
        return Ok(vec![(
            "HINT_PROMOTION_TARGET_MISSING".into(),
            "work-manifest 에 promotion_target 이 없다 — `set_promotion_target` 뒤 finalize 가 방출한다".into(),
        )]);
    };
    let mut out = Vec::new();
    if !jeq(&pt["kind"], "hint") {
        out.push(("HINT_PROMOTION_TARGET_KIND".into(), format!("promotion_target.kind={} ≠ 'hint'", pt["kind"].repr())));
    }
    for (key, want, code) in [
        ("tag", tag, "HINT_PROMOTION_TARGET_TAG_MISMATCH"),
        ("topology", topology, "HINT_PROMOTION_TARGET_TOPOLOGY_MISMATCH"),
        ("anchor", anchor, "HINT_PROMOTION_TARGET_ANCHOR_MISMATCH"),
    ] {
        if !jeq(&pt[key], want) {
            out.push((
                code.into(),
                format!("promotion_target.{key}={} ≠ 봉인 대상 {}", pt[key].repr(), crate::value::py_repr(want)),
            ));
        }
    }
    match manifest.get("identity").filter(|v| v.is_dict()) {
        None => out.push(("HINT_IDENTITY_ABSENT".into(), "work-manifest 에 identity 가 없다".into())),
        Some(ident) => match topology_label(ident) {
            Err(e) if !e.is_py() => out.push((e.code.clone(), e.message.clone())),
            Err(e) => return Err(e),
            Ok(label) => {
                if label != topology {
                    out.push((
                        "HINT_IDENTITY_TOPOLOGY_LABEL_MISMATCH".into(),
                        format!(
                            "identity 파생 라벨 {} ≠ promotion_target.topology {}(X7)",
                            crate::value::py_repr(&label),
                            crate::value::py_repr(topology)
                        ),
                    ))
                }
            }
        },
    }
    Ok(out)
}

// ── 발행 기록 ────────────────────────────────────────────────────────────────────────────────────────────────
pub fn publication_records(repo: &Path) -> HR<Vec<J>> {
    let d = core::resolve(repo).join(core::REL_EVIDENCE_DIR);
    let mut out = Vec::new();
    if !d.is_dir() {
        return Ok(out);
    }
    let mut names: Vec<String> = std::fs::read_dir(&d)
        .map_err(|e| core::io_err(&e, &d))?
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".json"))
        .collect();
    names.sort();
    for name in names {
        if name.ends_with(".work-manifest.json") {
            continue;
        }
        let stem = name[..name.len() - ".json".len()].to_string();
        let f = d.join(&name);
        let data = match read_strict(&f) {
            Err((ty, e)) => {
                out.push(crate::jd! {"_id" => stem, "_unreadable" => format!("{ty}: {e}")});
                continue;
            }
            Ok(t) => match crate::value::loads(&t) {
                Err(e) => {
                    out.push(crate::jd! {"_id" => stem, "_unreadable" => format!("JSONDecodeError: {e}")});
                    continue;
                }
                Ok(v) => v,
            },
        };
        match data {
            J::Dict(mut m) => {
                m.insert("_id", stem);
                out.push(J::Dict(m));
            }
            other => out.push(crate::jd! {"_id" => stem, "_unreadable" => format!("최상위가 객체가 아니다({})", other.type_name())}),
        }
    }
    Ok(out)
}

// ── 스윕 ─────────────────────────────────────────────────────────────────────────────────────────────────────
pub fn _sweep_doc(repo: &Path, directory: &Path, binding: &str) -> HR<Option<J>> {
    let idx = _read_json_opt(&directory.join("sweep_index.json"), "sweep_index.json")?;
    if !idx.is_dict() {
        return Ok(None);
    }
    let levels = if idx["levels"].is_list() { idx["levels"].clone() } else { J::list() };
    let dir = _rel(repo, directory)?;
    let lite = if idx["lite"].is_dict() { idx["lite"].clone() } else { J::Null };
    let gen = m_get(idx.as_dict().unwrap(), "generated_utc");
    let verdict = _read_json_opt(&directory.join("verdict.json"), "verdict.json")?;
    let roofline = _read_json_opt(&directory.join("roofline.json"), "roofline.json")?;
    let bench_mode = _read_json_opt(&directory.join("bench_mode.json"), "bench_mode.json")?;
    Ok(Some(crate::jd! {"dir" => dir, "index" => idx, "generated_utc" => gen, "levels" => levels, "lite" => lite,
        "verdict" => verdict, "roofline" => roofline, "bench_mode" => bench_mode, "binding" => binding}))
}

fn sweep_name(idx: &J) -> J {
    let meta = &idx["meta"];
    let n = if meta.truthy() { meta["config_name"].clone() } else { J::Null };
    if n.truthy() {
        n
    } else {
        idx["config"].clone()
    }
}

pub fn _output_sweeps(repo: &Path, topology: &str, cell: Option<&str>) -> HR<Vec<J>> {
    let base = repo.join("output").join(topology).join("benchlog");
    let mut names: Vec<String> = match std::fs::read_dir(&base) {
        Ok(rd) => rd.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.starts_with("sweep_")).collect(),
        Err(_) => Vec::new(),
    };
    names.sort();
    let mut docs = Vec::new();
    for n in names {
        let directory = base.join(&n);
        if !directory.is_dir() || is_symlinked(&directory) {
            continue;
        }
        let own = cell.map_or(true, |c| n == format!("sweep_{c}"));
        let p = directory.join("sweep_index.json");
        let idx = if own { _read_json_opt(&p, "sweep_index.json")? } else { _read_json_opt(&p, "sweep_index.json").unwrap_or(J::Null) };
        if !idx.is_dict() {
            continue;
        }
        let name = sweep_name(&idx);
        match cell {
            None => {
                if !matches!(name.as_str(), Some(s) if _CELL_RE().is_full(s)) {
                    continue;
                }
            }
            Some(c) => {
                if !jeq(&name, c) && !(name.is_null() && n == format!("sweep_{c}")) {
                    continue;
                }
            }
        }
        let gen = idx["generated_utc"].clone();
        docs.push(crate::jd! {"dir" => _rel(repo, &directory)?, "index" => idx, "generated_utc" => gen});
    }
    Ok(docs)
}

/// Bind an official pointer or a unique config/time match, never a name alone.
pub fn _bind_sweep(
    repo: &Path,
    topology: &str,
    cell: &str,
    measured_utc: Option<&str>,
    simlog_dirs: &J,
    sweep_dir: &J,
    sweep_config: &J,
    sweep_generated_utc: &J,
) -> HR<(Option<J>, Vec<String>, String)> {
    let out_docs: Vec<J>;
    if !sweep_dir.is_null() {
        let base = repo.join("output").join(topology).join("benchlog");
        if !matches!(sweep_generated_utc.as_str(), Some(s) if _UTC().is_full(s)) {
            return core::fail("HINT_SWEEP_POINTER_INVALID", "measurement.sweep_dir requires its producer sweep_generated_utc", None);
        }
        let sd = sweep_dir.as_str().unwrap_or("");
        if sd.is_empty() || Path::new(sd).is_absolute() || sd.split('/').any(|p| p == "..") {
            return core::fail("HINT_SWEEP_POINTER_INVALID", "measurement.sweep_dir must be a repository-relative output path", None);
        }
        let directory = repo.join(sd);
        let dname = directory.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        if directory.parent() != Some(base.as_path()) || !dname.starts_with("sweep_") || is_symlinked(&directory) {
            return core::fail("HINT_SWEEP_POINTER_INVALID", "measurement.sweep_dir escapes the topology benchlog or uses a symlink", None);
        }
        let doc = if directory.is_dir() { _sweep_doc(repo, &directory, "output")? } else { None };
        if let Some(d) = &doc {
            let name = sweep_name(&d["index"]);
            let want = if sweep_config.truthy() { sweep_config.clone() } else { J::from(cell) };
            if name != want || d["generated_utc"] != *sweep_generated_utc || measured_utc.map_or(false, |m| !jeq(&d["generated_utc"], m)) {
                return core::fail("HINT_SWEEP_POINTER_MISMATCH", "measurement.sweep_dir disagrees with the cell or measurement UTC", None);
            }
        }
        out_docs = doc.into_iter().collect();
    } else {
        out_docs = _output_sweeps(repo, topology, Some(cell))?;
    }
    let Some(mu) = measured_utc else {
        if out_docs.is_empty() {
            return Ok((None, vec![], "스윕 디렉터리 없음(조인 키도 없음 — lite 셀이면 정상)".into()));
        }
        return Ok((
            None,
            vec!["HINT_MISSING_SWEEP".into()],
            format!("{} 실재하나 조인 키(인증서 measured_utc · 리포트 생성일)가 없어 묶지 않았다", out_docs[0]["dir"].display()),
        ));
    };
    let matches: Vec<&J> = out_docs.iter().filter(|d| jeq(&d["generated_utc"], mu)).collect();
    if matches.len() > 1 {
        return core::fail("HINT_SWEEP_AMBIGUOUS", format!("same cell and measurement UTC match {} output runs", matches.len()), None);
    }
    if let Some(m) = matches.first() {
        let od = _sweep_doc(repo, &repo.join(m["dir"].display()), "output")?;
        let od = od.unwrap_or(J::Null);
        let msg = format!("{} (generated_utc = measured_utc {mu})", od["dir"].display());
        return Ok((Some(od), vec![], msg));
    }
    let refused = if !out_docs.is_empty() {
        format!("output/{topology}/benchlog의 셀 {cell} 스윕에 measured_utc={} 일치가 없음", crate::value::py_repr(mu))
    } else {
        let sd = if sweep_dir.truthy() { sweep_dir.display() } else { format!("output/{topology}/benchlog/sweep_{cell}") };
        format!("{sd} 부재")
    };
    if !sweep_dir.is_null() {
        return Ok((None, vec!["HINT_MISSING_SWEEP".into()], format!("공식 measurement.sweep_dir를 묶지 않음 — {refused}")));
    }
    for sd in crate::api::unwrap_seq(simlog_dirs).iter() {
        let Some(p) = path_of(sd) else { continue };
        let sdp = repo.join(p);
        if sdp.is_dir() {
            if let Some(cp) = _sweep_doc(repo, &sdp, "simlog-copy")? {
                if jeq(&cp["generated_utc"], mu) {
                    let msg = format!("{} (simlog 사본 · {refused})", cp["dir"].display());
                    return Ok((Some(cp), vec!["HINT_MISSING_SWEEP_RAW".into()], msg));
                }
            }
        }
    }
    Ok((None, vec!["HINT_MISSING_SWEEP".into()], format!("묶지 않음 — {refused}")))
}

pub fn _internal_doc_refs(text: &str) -> HR<Vec<String>> {
    Ok(_INTERNAL_DOC_REF().findall(text))
}

// ── docker (읽기 전용) ───────────────────────────────────────────────────────────────────────────────────────
/// A docker runner hook (test injection): argv (starting with `docker`) → process, or `Err` (= OSError/timeout).
pub type DockerRunner<'a> = &'a dyn Fn(&[String]) -> Result<core::Proc, String>;

fn default_docker(argv: &[String]) -> Result<core::Proc, String> {
    core::run_capture(argv, None, &[], &[], None).map(|o| core::Proc {
        returncode: o.returncode,
        stdout: String::from_utf8_lossy(&o.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&o.stderr).into_owned(),
    })
}

/// `_docker(runner, *args)` with the default runner (the J hook is the production `None`).
pub fn _docker(runner: &J, args: &[String]) -> HR<Option<String>> {
    let _ = runner;
    _docker_r(None, args)
}

/// `_docker` with a typed runner hook. Only read-only `image inspect` / `history` are allowed.
pub fn _docker_r(runner: Option<DockerRunner>, args: &[String]) -> HR<Option<String>> {
    let ro = (args.len() >= 2 && args[0] == "image" && args[1] == "inspect") || (!args.is_empty() && args[0] == "history");
    if !ro {
        let head: Vec<String> = args.iter().take(2).cloned().collect();
        return core::fail("HINT_DOCKER_WRITE_REFUSED", format!("evidence 는 docker 읽기만 부른다: {}", py_tuple_repr(&head)), None);
    }
    let mut argv = vec!["docker".to_string()];
    argv.extend(args.iter().cloned());
    let r = match runner {
        Some(f) => f(&argv),
        None => default_docker(&argv),
    };
    match r {
        Ok(p) if p.returncode == 0 => Ok(Some(p.stdout)),
        _ => Ok(None),
    }
}

pub fn _history_build_args(text: Option<&str>) -> HR<(J, J)> {
    let mut seen: Vec<(String, Vec<String>)> = Vec::new();
    for line in splitlines(text.unwrap_or("")) {
        let Some(m) = _HISTORY_ARGS().match_at_start(pstrip(line)) else { continue };
        let n: usize = crate::pyre::group(&m, 1).unwrap_or("0").parse().unwrap_or(usize::MAX);
        for tok in crate::pyre::group(&m, 2).unwrap_or("").split(' ').take(n) {
            if let Some((k, v)) = tok.split_once('=') {
                if e1_upper_key_re().is_full(k) {
                    let i = match seen.iter().position(|(a, _)| a == k) {
                        Some(i) => i,
                        None => {
                            seen.push((k.to_string(), Vec::new()));
                            seen.len() - 1
                        }
                    };
                    if !seen[i].1.iter().any(|x| x == v) {
                        seen[i].1.push(v.to_string());
                    }
                }
            }
        }
    }
    seen.sort_by(|a, b| a.0.cmp(&b.0));
    let mut args = Map::new();
    let mut conflicts = Map::new();
    for (k, v) in &seen {
        args.insert(k.clone(), v[0].clone());
        if v.len() > 1 {
            conflicts.insert(k.clone(), J::from(v.clone()));
        }
    }
    Ok((J::Dict(args), J::Dict(conflicts)))
}

pub fn _image_facts(ref_: &str, runner: &J) -> HR<Option<J>> {
    let _ = runner;
    _image_facts_r(ref_, None)
}

pub fn _image_facts_r(ref_: &str, runner: Option<DockerRunner>) -> HR<Option<J>> {
    let Some(raw) = _docker_r(runner, &["image".into(), "inspect".into(), ref_.into()])? else { return Ok(None) };
    let Ok(doc) = crate::value::loads(&raw) else { return Ok(None) };
    let doc = match &doc {
        J::List(l) if !l.is_empty() => l[0].clone(),
        _ => doc,
    };
    if !doc.is_dict() {
        return Ok(None);
    }
    let cfg = if doc["Config"].is_dict() { doc["Config"].clone() } else { J::dict() };
    let mut env = Map::new();
    for item in cfg["Env"].items() {
        if let Some((k, v)) = item.as_str().and_then(|s| s.split_once('=')) {
            if matches!(k, "NVIDIA_PYTORCH_VERSION" | "PYTORCH_VERSION" | "CUDA_VERSION") {
                env.insert(k, v);
            }
        }
    }
    let labels = if cfg["Labels"].is_dict() { cfg["Labels"].clone() } else { J::dict() };
    let history = _docker_r(
        runner,
        &["history".into(), "--no-trunc".into(), "--format".into(), "{{.CreatedBy}}".into(), ref_.into()],
    )?;
    let (args, conflicts) = _history_build_args(history.as_deref())?;
    Ok(Some(crate::jd! {"id" => doc["Id"].clone(), "created" => doc["Created"].clone(), "architecture" => doc["Architecture"].clone(),
        "env" => J::Dict(env), "compose_project" => labels["com.docker.compose.project"].clone(), "history_build_args" => args,
        "history_build_args_conflicts" => conflicts, "history_observed" => history.is_some()}))
}

/// The cell env's first KEY=VALUE (OWNER-MIRROR of `slave_forward.read_env_first`).
pub fn _env_first(repo: &Path, rel_path: Option<&str>) -> HR<J> {
    let Some(rel) = rel_path.filter(|s| !s.is_empty()) else { return Ok(J::dict()) };
    _slave_forward(repo)?;
    Ok(sf_read_env_first(&repo.join(rel)))
}

/// Our Dockerfile's own `^ARG` names (OWNER-MIRROR of `render_dockerfile.ledger_build_args`) as a sorted list
/// (Python frozenset) · `None` = cannot judge membership.
pub fn _dockerfile_args(repo: &Path, topology: &str, dockerfile: Option<&str>) -> HR<(Option<J>, String)> {
    let Some(df) = dockerfile.filter(|d| !d.is_empty() && !d.contains('/')) else {
        return Ok((None, "쓰인 Dockerfile 선택자 없음 — history build-arg 소속 판정 불가(싣지 않는다)".into()));
    };
    let p = repo.join("output").join(topology).join(df);
    if !p.is_file() {
        return Ok((None, format!("output/{topology}/{df} 부재 — history build-arg 소속 판정 불가(싣지 않는다)")));
    }
    owner_present(repo, core::REL_RENDER_DOCKERFILE)?;
    let text = read_replace(&p).map_err(|e| core::io_err(&e, &p))?;
    let names = crate::value::sorted_strs(e1_ledger_arg_re().findall(&text).into_iter().collect::<std::collections::BTreeSet<_>>());
    Ok((Some(J::from(names)), format!("output/{topology}/{df} 의 ^ARG(render_dockerfile.ledger_build_args)")))
}

/// compose's first `build:` block `dockerfile:` → ("literal", file) · ("selector", default | None) · (None, None).
pub fn compose_build_dockerfile(compose_path: &J) -> HR<(Option<String>, Option<String>)> {
    let Some(p) = path_of(compose_path) else { return Ok((None, None)) };
    let text = match read_strict(&p) {
        Ok(t) => t,
        Err((ty, e)) if ty == "UnicodeDecodeError" => return Err(HintError::py(&ty, e)),
        Err(_) => return Ok((None, None)),
    };
    let block = _COMPOSE_BUILD_BLOCK().search(&text).and_then(|m| crate::pyre::group(&m, 2).map(str::to_string));
    let raw = block
        .as_deref()
        .and_then(|b| e1_compose_dockerfile_re().search(b).and_then(|m| crate::pyre::group(&m, 1).map(str::to_string)))
        .map(|g| pstrip(&g).trim_matches(|c| c == '\'' || c == '"').to_string())
        .unwrap_or_default();
    if raw.is_empty() {
        return Ok((None, None));
    }
    if raw.contains('$') {
        let d = e1_selector_re().fullmatch(&raw).and_then(|m| crate::pyre::group(&m, 1).map(str::to_string)).filter(|s| !s.is_empty());
        return Ok((Some("selector".into()), d));
    }
    Ok((Some("literal".into()), Some(raw)))
}

pub fn _history_track_dockerfile(raw_args: &J) -> HR<(Option<String>, String)> {
    let has = |k: &str| raw_args.as_dict().map_or(false, |m| m.contains_key(k));
    let (r, v) = (has("VLLM_REF"), has("VLLM_VERSION"));
    if r && !v {
        return Ok((
            Some("Dockerfile.source-build".into()),
            format!("{HISTORY_TRACK_MARK}(VLLM_REF={} · VLLM_VERSION 없음 → 소스빌드 Dockerfile)", raw_args["VLLM_REF"].display()),
        ));
    }
    if v && !r {
        return Ok((
            Some("Dockerfile".into()),
            format!("{HISTORY_TRACK_MARK}(VLLM_VERSION={} · VLLM_REF 없음 → wheel Dockerfile)", raw_args["VLLM_VERSION"].display()),
        ));
    }
    Ok((
        None,
        if r {
            "history 에 VLLM_REF · VLLM_VERSION 이 둘 다 있다 — 트랙 판정 불가".into()
        } else {
            "history 에 VLLM_REF · VLLM_VERSION 모두 없음(또는 history 미관측) — 트랙 판정 불가".into()
        },
    ))
}

pub fn _build_identity(repo: &Path, topology: &str, env: &J, env_rel: Option<&str>, measured: &J, manifest: &J, runner: &J) -> HR<(J, Vec<String>)> {
    let _ = runner;
    _build_identity_r(repo, topology, env, env_rel, measured, manifest, None)
}

fn src_str(src: &Map, k: &str) -> String {
    src.get(k).map(J::display).unwrap_or_default()
}

/// `_build_identity` with a typed docker runner hook.
pub fn _build_identity_r(
    repo: &Path,
    topology: &str,
    env: &J,
    env_rel: Option<&str>,
    measured: &J,
    manifest: &J,
    runner: Option<DockerRunner>,
) -> HR<(J, Vec<String>)> {
    let mut src = Map::new();
    let mut missing: Vec<String> = Vec::new();
    let env_src = match env_rel.filter(|s| !s.is_empty()) {
        Some(r) => format!("셀 env({r})"),
        None => "셀 env 부재".to_string(),
    };
    let image_tag = if env["IMAGE_TAG"].truthy() { env["IMAGE_TAG"].clone() } else { measured["image_tag"].clone() };
    src.insert(
        "image_tag",
        if env["IMAGE_TAG"].truthy() {
            J::from(format!("{env_src} IMAGE_TAG"))
        } else {
            measured.get("image_tag_source").cloned().unwrap_or_else(|| J::from("미관측"))
        },
    );
    let digest: Option<String> = match measured["image_digest"].as_str() {
        Some(s) if !matches!(pstrip(s), "" | "NA" | "N/A") => Some(s.to_string()),
        _ => None,
    };
    src.insert("image_digest", measured.get("image_digest_source").cloned().unwrap_or_else(|| J::from("미측정")));
    let mut facts: Option<J> = None;
    if let Some(d) = digest.as_deref() {
        facts = _image_facts_r(d, runner)?;
        if facts.is_none() && image_tag.truthy() {
            let tag_facts = _image_facts_r(&image_tag.display(), runner)?;
            if let Some(tf) = tag_facts {
                if !jeq(&tf["id"], d) {
                    src.insert(
                        "image_observation",
                        format!(
                            "로컬 태그 {} 는 측정 digest 와 다른 이미지({}…)를 가리킨다 — 그 이미지의 history 를 빌드 정체로 쓰지 않는다",
                            image_tag.display(),
                            core::head_chars(&tf["id"].display(), 19)
                        ),
                    );
                }
            }
        } else if facts.is_some() {
            src.insert("image_observation", format!("docker image inspect/history {}…(측정 digest)", core::head_chars(d, 19)));
        }
    } else if image_tag.truthy() {
        facts = _image_facts_r(&image_tag.display(), runner)?;
        if facts.is_some() {
            src.insert("image_observation", format!("docker image inspect/history {}(측정 digest 미기재 — 태그로 관측)", image_tag.display()));
        }
    }
    if facts.is_none() {
        missing.push("HINT_MISSING_BUILD_IDENTITY".into());
        if !src.contains_key("image_observation") {
            src.insert("image_observation", "관측 불가(이미지 부재·docker 미가용·태그 이동)");
        }
    }
    let fj = facts.clone().unwrap_or_else(J::dict);
    let docker_plane = env["IMAGE_TAG"].truthy() || env["BUILD_DOCKERFILE"].truthy() || digest.is_some() || facts.is_some();
    let mut dockerfile: J = if env["BUILD_DOCKERFILE"].truthy() { env["BUILD_DOCKERFILE"].clone() } else { J::Null };
    if dockerfile.truthy() {
        src.insert("dockerfile", format!("{env_src} BUILD_DOCKERFILE"));
    } else if docker_plane {
        let compose = repo.join("output").join(topology).join("docker-compose.yaml");
        dockerfile = if compose.is_file() {
            _slave_forward(repo)?;
            J::from(sf_compose_default(&compose, "BUILD_DOCKERFILE")?)
        } else {
            J::Null
        };
        let (lit_kind, lit) =
            if !dockerfile.truthy() { compose_build_dockerfile(&J::from(core::posix(&compose)))? } else { (None, None) };
        let lit_s = lit.clone().unwrap_or_else(|| "None".into());
        let lit_txt = if lit_kind.as_deref() == Some("literal") {
            Some(format!(
                "output/{topology}/docker-compose.yaml 리터럴 build.dockerfile: {lit_s}(이 파일만 단독 적용하고 override 가 없을 때 {lit_s} 선택 · 과거 빌드에 이 compose 또는 override 를 사용했는지는 미관측)"
            ))
        } else {
            None
        };
        src.insert(
            "dockerfile",
            if dockerfile.truthy() {
                format!(
                    "output/{topology}/docker-compose.yaml 기본값 ${{BUILD_DOCKERFILE:-{}}}(셀 env 선택자 없음 · 현재 compose 기준)",
                    dockerfile.display()
                )
            } else if let Some(t) = &lit_txt {
                format!("셀 env 선택자 없음 · {t}")
            } else {
                "선택자 없음(셀 env·compose 기본값 모두 부재)".to_string()
            },
        );
        let hb = &fj["history_build_args"];
        let hb = if hb.truthy() { hb.clone() } else { J::dict() };
        let (obs_df, obs_note) = _history_track_dockerfile(&hb)?;
        match &obs_df {
            Some(o) if !jeq(&dockerfile, o) => {
                let cur = src_str(&src, "dockerfile");
                let tail = if dockerfile.truthy() {
                    format!(" — {cur} 은 이 이미지 관측과 모순 · 관측이 이긴다")
                } else {
                    format!(" — {cur}")
                        + &(if lit_txt.is_some() && lit.as_deref() != Some(o.as_str()) {
                            format!(" · ⚠ compose 리터럴 {lit_s} ≠ 관측 {o}(이미지 빌드에 사용된 compose 또는 override 여부는 미관측)")
                        } else {
                            String::new()
                        })
                };
                src.insert("dockerfile", obs_note.clone() + &tail);
                dockerfile = J::from(o.clone());
            }
            _ if !dockerfile.truthy() => {
                let cur = src_str(&src, "dockerfile");
                src.insert(
                    "dockerfile",
                    format!("{cur} · {obs_note}") + if facts.is_some() { "" } else { " · 이미지 탐침 결과 없음(docker 미가용 · 이미지 부재)" },
                );
            }
            _ => {}
        }
    } else {
        src.insert("dockerfile", "docker 평면 신호 없음(IMAGE_TAG·선택자·측정 이미지 부재)");
    }
    let raw_args = if fj["history_build_args"].truthy() { fj["history_build_args"].clone() } else { J::dict() };
    let df_s = if dockerfile.truthy() { Some(dockerfile.display()) } else { None };
    let (own, own_src) = _dockerfile_args(repo, topology, df_s.as_deref())?;
    src.insert("history_build_args", own_src);
    let own_has = |k: &str| own.as_ref().map_or(false, |o| o.items().iter().any(|x| jeq(x, k)));
    let mut hargs = Map::new();
    if let Some(m) = raw_args.as_dict() {
        for (k, v) in m.iter() {
            if own_has(k) {
                hargs.insert(k, v.clone());
            }
        }
    }
    let mut hconf = Map::new();
    if let Some(m) = fj["history_build_args_conflicts"].as_dict() {
        for (k, v) in m.iter() {
            if own_has(k) {
                hconf.insert(k, v.clone());
            }
        }
    }
    let track: Option<&str> = if jeq(&dockerfile, "Dockerfile") {
        Some("wheel")
    } else if dockerfile.truthy() {
        Some("source-build")
    } else {
        None
    };
    let track_src = if dockerfile.truthy() {
        format!("선택자 {}", dockerfile.display())
            + &(if src_str(&src, "dockerfile").contains(HISTORY_TRACK_MARK) { format!("({HISTORY_TRACK_MARK})") } else { String::new() })
    } else if docker_plane {
        format!("선택자 미관측 — 트랙 미정({})", src_str(&src, "dockerfile"))
    } else {
        "docker 평면 신호 없음 — 트랙 미정(native 여부는 artifacts.plane_of 가 관측 신호로 판정)".to_string()
    };
    src.insert("track", track_src);
    let mut conflicts = Map::new();
    for key in ["VLLM_REF", "VLLM_REPO", "VLLM_VERSION"] {
        if let Some(h) = hargs.get(key) {
            let e = &env[key];
            if !e.is_null() && !jeq(e, "") && e != h {
                conflicts.insert(key, crate::jl![format!("env:{}", e.display()), format!("history:{}", h.display())]);
            }
        }
    }
    let track_key = match track {
        Some("wheel") => Some("VLLM_VERSION"),
        Some("source-build") => Some("VLLM_REF"),
        _ => None,
    };
    if let Some(tk) = track_key {
        if raw_args.truthy() && own.is_some() && own_has(tk) && !raw_args.as_dict().map_or(false, |m| m.contains_key(tk)) {
            conflicts.insert(
                "BUILD_DOCKERFILE",
                crate::jl![format!("env:{}", dockerfile.display()), format!("history:{tk} build-arg 없음(다른 Dockerfile 로 지은 이미지)")],
            );
        }
    }
    let fenv = if fj["env"].truthy() { fj["env"].clone() } else { J::dict() };
    let pick = |key: &str| -> (J, String) {
        if let Some(h) = hargs.get(key) {
            return (if h.truthy() { h.clone() } else { J::Null }, format!("docker history build-arg {key}"));
        }
        if env[key].truthy() {
            return (env[key].clone(), format!("{env_src} {key}"));
        }
        (J::Null, format!("{key} 미관측"))
    };
    let (vllm_ref, s1) = pick("VLLM_REF");
    src.insert("vllm_ref", s1);
    let (vllm_repo, s2) = pick("VLLM_REPO");
    src.insert("vllm_repo", s2);
    let (vllm_version, s3) = pick("VLLM_VERSION");
    src.insert("vllm_version", s3);
    let arch = &fj["architecture"];
    let cpu_arch = if arch.truthy() { arch.clone() } else { manifest["cpu_arch"].clone() };
    let wheel_url = _WHEEL_URL_KEYS.iter().find(|k| env[**k].truthy()).map(|k| env[*k].clone()).unwrap_or(J::Null);
    let mut out = crate::jd! {
        "track" => J::from(track), "dockerfile" => dockerfile.clone(), "image_tag" => image_tag.clone(),
        "image_digest" => J::from(digest.clone()), "vllm_repo" => vllm_repo, "vllm_ref" => vllm_ref,
        "vllm_version" => vllm_version, "vllm_sha" => J::Null, "torch" => fenv["PYTORCH_VERSION"].clone(),
        "cuda" => fenv["CUDA_VERSION"].clone(), "ngc" => fenv["NVIDIA_PYTORCH_VERSION"].clone(), "cpu_arch" => cpu_arch.clone(),
        "created" => fj["created"].clone(), "compose_project" => fj["compose_project"].clone(),
        "history_build_args" => J::Dict(hargs.clone()), "history_build_args_conflicts" => J::Dict(hconf),
        "env_history_conflicts" => J::Dict(conflicts), "wheel_url" => wheel_url,
    };
    for (k, what) in [("torch", "PYTORCH_VERSION"), ("cuda", "CUDA_VERSION"), ("ngc", "NVIDIA_PYTORCH_VERSION")] {
        src.insert(k, if out[k].truthy() { format!("docker image inspect Config.Env {what}") } else { "이미지 관측 불가".to_string() });
    }
    src.insert(
        "cpu_arch",
        if arch.truthy() {
            "docker image inspect Architecture"
        } else if cpu_arch.truthy() {
            "manifest cpu_arch"
        } else {
            "미관측"
        },
    );
    let it_src = src.get("image_tag").cloned().unwrap_or(J::Null);
    let it_txt = if it_src.truthy() { it_src.display() } else { String::new() };
    out.set("native_wheelhouse_source", !env["IMAGE_TAG"].truthy() && it_txt.contains("native serve proof"));
    out.set("source", J::Dict(src));
    Ok((out, missing))
}

/// Host fact observed per node by the attestation: (value, source, {node: value}).
pub fn _attested_host(att: &J, key: &str, timing_note: Option<&str>) -> HR<(Option<String>, Option<String>, J)> {
    if !att.is_dict() {
        return Ok((None, None, J::dict()));
    }
    let path = or_str(&att["_path"], "attestation");
    let row = if att["parity"].is_dict() { att["parity"][key].clone() } else { J::Null };
    let mut by_node: Vec<(String, String)> = Vec::new();
    let mut where_ = String::new();
    if row.is_dict() {
        for n in ["main", "sub"] {
            if !unobs(&row[n]) {
                by_node.push((n.to_string(), pstrip(&row[n].display()).to_string()));
            }
        }
        where_ = format!("attestation parity.{key}({path} · 노드별 관측 · verdict={})", row["verdict"].display());
    }
    if by_node.is_empty() {
        if let Some(nodes) = att["nodes"].as_dict() {
            let mut items: Vec<(&str, &J)> = nodes.iter().collect();
            items.sort_by(|a, b| a.0.cmp(b.0));
            for (n, d) in items {
                let v = &d[key];
                if d.is_dict() && !unobs(v) && matches!(v, J::Str(_) | J::Int(_) | J::Float(_) | J::Bool(_)) {
                    by_node.push((n.to_string(), pstrip(&v.display()).to_string()));
                }
            }
            where_ = format!("attestation nodes.<n>.{key}({path} · 노드별 관측)");
        }
    }
    if by_node.is_empty() {
        return Ok((None, None, J::dict()));
    }
    let vals = crate::value::sorted_strs(by_node.iter().map(|(_, v)| v.clone()).collect::<std::collections::BTreeSet<_>>());
    let main = by_node.iter().find(|(n, _)| n == "main").map(|(_, v)| v.clone()).filter(|v| !v.is_empty());
    let val = main.unwrap_or_else(|| vals[0].clone());
    let mut sorted = by_node.clone();
    sorted.sort();
    if vals.len() > 1 {
        let parts: Vec<String> = sorted.iter().map(|(n, v)| format!("{n}={v}")).collect();
        where_ += &format!(" — ⚠ 노드별 상이({} · 값 = main)", parts.join(" · "));
    } else {
        let ns: Vec<&str> = sorted.iter().map(|(n, _)| n.as_str()).collect();
        where_ += &format!(" — {} 일치", ns.join("="));
    }
    if !timing_note.map_or(false, |t| t.contains(SAME_BOOT_MARK) || t.contains(WEAK_SAME_BOOT_MARK)) {
        where_ += " · 이 측정 실행과 같은 실행인지 미검증(같은 호스트의 관측)";
    }
    if let Some(t) = timing_note.filter(|t| !t.is_empty()) {
        where_ += &format!(" · {t}");
    }
    let mut bn = Map::new();
    for (n, v) in by_node {
        bn.insert(n, v);
    }
    Ok((Some(val), Some(where_), J::Dict(bn)))
}

/// (value, source) of the measurement engine logs' `kernel driver version` banner when all agree, else (None, None).
pub fn _engine_log_driver(repo: &Path, ev: &J) -> HR<(Option<String>, Option<String>)> {
    let logs = match _measured_engine_logs(repo, ev) {
        Ok(r) => r.0,
        Err(e) if !e.is_py() => return Ok((None, None)),
        Err(e) => return Err(e),
    };
    let mut vals: Vec<(String, String)> = Vec::new();
    for lg in &logs {
        for (i, line) in _log_lines(lg)?.iter().enumerate() {
            let clean = e1_ansi_re().sub_literal(line, "");
            if let Some(m) = _KERNEL_DRIVER().search(&clean) {
                let v = crate::pyre::group(&m, 1).unwrap_or("").to_string();
                if !vals.iter().any(|(a, _)| *a == v) {
                    vals.push((v, format!("{}:{}", _rel(repo, lg)?, i + 1)));
                }
                break;
            }
        }
    }
    if vals.len() != 1 {
        return Ok((None, None));
    }
    let (v, where_) = vals.remove(0);
    Ok((
        Some(v),
        Some(format!(
            "측정 엔진 로그 기동 배너 'kernel driver version'({where_} · 컨테이너가 보고한 호스트 커널 드라이버 · 이 측정의 엔진 로그 {}개 중 그 줄이 있는 로그 모두 같은 값)",
            logs.len()
        )),
    ))
}

/// Put the measurement host facts `driver`·`os` (with sources) into `ev.build_identity` (mutates `ev`).
pub fn _host_facts(ev: &mut J, repo: Option<&Path>) -> HR<()> {
    if !ev["build_identity"].is_dict() {
        ev.set("build_identity", J::dict());
    }
    {
        let b = ev.get_mut("build_identity").and_then(J::as_dict_mut).unwrap();
        if !b.contains_key("source") {
            b.insert("source", J::dict());
        }
    }
    let evc = ev.clone();
    let mut b = evc["build_identity"].as_dict().cloned().unwrap_or_default();
    let mut src = b.get("source").and_then(J::as_dict).cloned().unwrap_or_default();
    let cert = if evc["certificate"].truthy() { evc["certificate"].clone() } else { J::dict() };
    let meta = {
        let m = &evc["sweep"]["index"]["meta"];
        if m.truthy() { m.clone() } else { J::dict() }
    };
    let man = if evc["manifest"].truthy() { evc["manifest"].clone() } else { J::dict() };
    let man_where = or_str(&evc["sources"]["manifest"], "manifest");
    let measured_src = |doc: &J| -> bool {
        let v = &doc["driver_version_source"];
        let s = if v.truthy() { v.display() } else { String::new() };
        pstrip(&s).starts_with("measured")
    };
    let timing = _attestation_timing_note(&evc)?;
    let (obs_v, mut obs_src, mut by_node) = _attested_host(&evc["attestation"], "driver", timing.as_deref())?;
    let att_cfg = evc["attestation"]["config"].clone();
    let plane_native = jeq(&evc["plane"], "native");
    let other_run = obs_v.as_deref().map_or(false, |v| !v.is_empty())
        && (plane_native || (!att_cfg.is_null() && !jeq(&att_cfg, "") && att_cfg != evc["cell"]));
    let mut layers: Vec<(String, String, bool)> = Vec::new();
    if obs_v.as_deref().map_or(false, |v| !v.is_empty()) && !other_run {
        layers.push((obs_v.clone().unwrap(), obs_src.clone().unwrap_or_default(), true));
    } else if let Some(r) = repo {
        let (lv, lsrc) = _engine_log_driver(&core::resolve(r), &evc)?;
        if let Some(lv) = lv.filter(|s| !s.is_empty()) {
            layers.push((lv.clone(), lsrc.clone().unwrap_or_default(), true));
            if j_in(&evc["node"], &["main", "sub"]) {
                by_node = crate::jd! {evc["node"].display() => lv};
                obs_src = lsrc;
            }
        }
    }
    let cert_name = pname(&or_str(&evc["certificate_path"], ""));
    let docs = [
        (meta.clone(), format!("sweep_index.meta.driver_version({} · 측정 시 조립", evc["sweep"]["dir"].display())),
        (cert.clone(), format!("인증서 driver_version({cert_name} · 스윕 meta 의 옮김")),
    ];
    for (doc, where_) in &docs {
        let v = if doc.is_dict() { _norm_value(&doc["driver_version"])? } else { J::Null };
        if v.truthy() {
            let obs = measured_src(doc);
            let tail = if obs {
                format!(" · {})", doc["driver_version_source"].display())
            } else {
                format!(" — sweep_bench.sh 가 manifest 에서 읽는다) {_DECLARED_COPY}")
            };
            layers.push((v.display(), format!("{where_}{tail}"), obs));
        }
    }
    let mv = if man.is_dict() { _norm_value(&man["driver_version"])? } else { J::Null };
    if mv.truthy() {
        layers.push((mv.display(), format!("{man_where} driver_version(스캔 시점 선언 — 측정 시각 값 미보장)"), false));
    }
    b.insert("driver", J::Null);
    src.insert("driver", "미관측(attestation·인증서·스윕 meta·manifest 에 driver 없음)");
    if let Some(l) = layers.first() {
        b.insert("driver", l.0.clone());
        src.insert("driver", l.1.clone());
    }
    b.remove("driver_by_node");
    src.insert("driver_by_node", "이 측정에 결속된 노드별 구조화 관측 입력 없음(다른 시점 관측의 부재를 뜻하지 않음)");
    if by_node.truthy() && !other_run {
        b.insert("driver_by_node", by_node.clone());
        src.insert("driver_by_node", J::from(obs_src.clone()));
    }
    if other_run {
        let ov = obs_v.clone().unwrap_or_default();
        let who = (if plane_native { "원천 셀" } else { "다른 run" }).to_string() + &format!("({})", att_cfg.display());
        let mut pairs: Vec<(String, String)> =
            by_node.as_dict().map(|m| m.iter().map(|(k, v)| (k.to_string(), v.display())).collect()).unwrap_or_default();
        pairs.sort();
        let nodes = pairs.iter().map(|(n, v)| format!("{n}={v}")).collect::<Vec<_>>().join(" · ");
        let note = format!(
            "{who} attestation 관측 {ov}({nodes} · 이 run 아님 · 같은 호스트 — 관측 > 선언 규칙은 같은 run 의 관측에만 적용 · {})",
            obs_src.clone().map(J::from).unwrap_or(J::Null).display()
        );
        if b.get("driver").map_or(true, J::is_null) {
            b.insert("driver", ov.clone());
            src.insert("driver", format!("{note} — 이 run 의 관측·선언 모두 없어 그 값을 옮긴다(범위 = 다른 run)"));
        } else {
            let mut s = src_str(&src, "driver") + &format!(" · {note}");
            let d = m_get(&b, "driver");
            if !jeq(&d, &ov) {
                s += &format!(" · ⚠ 선언 {} 과 다르다(이 run 에서 관측되지 않았다 — 판정 ✗)", d.display());
            }
            src.insert("driver", s);
        }
    }
    let observed = layers.iter().find(|l| l.2).cloned();
    let declared = layers.iter().find(|l| !l.2).cloned();
    b.remove("driver_conflict");
    b.remove("driver_check");
    if let (Some(o), Some(d)) = (&observed, &declared) {
        if o.0 != d.0 {
            b.insert(
                "driver_conflict",
                crate::jd! {"observed" => o.0.clone(), "declared" => d.0.clone(),
                "sources" => crate::jd!{"observed" => o.1.clone(), "declared" => d.1.clone()}},
            );
            let s = src_str(&src, "driver") + &format!(" · ⚠ 선언 {} 과 다르다(driver_conflict)", d.0);
            src.insert("driver", s);
        }
    }
    if observed.is_some() || declared.is_some() {
        let verdict = match (&observed, &declared) {
            (Some(o), Some(d)) if o.0 != d.0 => "conflict",
            (Some(_), Some(_)) => "agree",
            _ => "unpaired",
        };
        b.insert(
            "driver_check",
            crate::jd! {"verdict" => verdict, "observed" => J::from(observed.as_ref().map(|o| o.0.clone())),
            "declared" => J::from(declared.as_ref().map(|d| d.0.clone())),
            "sources" => crate::jd!{"observed" => J::from(observed.as_ref().map(|o| o.1.clone())),
                                    "declared" => J::from(declared.as_ref().map(|d| d.1.clone()))},
            "observation_scope" => "이 측정에 결속된 구조화 드라이버 관측 입력"},
        );
    }
    b.insert("os", J::Null);
    src.insert("os", format!("미관측(attestation·인증서·스윕 meta·manifest 에 {} 키 없음 — 지어내지 않는다)", list_repr(&_OS_KEYS)));
    let mut done = false;
    for k in _OS_KEYS {
        let (v, where_, _bn) = _attested_host(&evc["attestation"], k, None)?;
        if let Some(v) = v.filter(|s| !s.is_empty()) {
            b.insert("os", v);
            src.insert("os", format!("{} {k}", where_.unwrap_or_default()));
            done = true;
            break;
        }
    }
    if !done {
        for (doc, where_) in [(&meta, "sweep_index.meta".to_string()), (&cert, "인증서".to_string()), (&man, man_where.clone())] {
            let hit = _OS_KEYS.iter().find(|k| {
                doc.is_dict() && !unobs(&doc[**k]) && matches!(doc[**k], J::Str(_) | J::Int(_) | J::Float(_) | J::Bool(_))
            });
            if let Some(k) = hit {
                b.insert("os", pstrip(&doc[*k].display()).to_string());
                src.insert("os", format!("{where_} {k}"));
                break;
            }
        }
    }
    b.insert("source", J::Dict(src));
    ev.set("build_identity", J::Dict(b));
    Ok(())
}

pub fn _local_image_digest(repo: &Path, ev: &mut J, runner: &J) -> HR<()> {
    let _ = runner;
    _local_image_digest_r(repo, ev, None)
}

/// `_local_image_digest` with a typed docker runner hook (mutates `ev.build_identity.image_digest_local`).
pub fn _local_image_digest_r(repo: &Path, ev: &mut J, runner: Option<DockerRunner>) -> HR<()> {
    let Some(b) = ev.get_mut("build_identity").and_then(J::as_dict_mut) else { return Ok(()) };
    b.remove("image_digest_local");
    let tag = m_get(b, "image_tag");
    if m_get(b, "image_digest").truthy() || !matches!(tag.as_str(), Some(t) if !pstrip(t).is_empty()) {
        return Ok(());
    }
    let tag = tag.as_str().unwrap().to_string();
    let facts = _image_facts_r(&tag, runner)?;
    let set = |ev: &mut J, v: J| {
        if let Some(b) = ev.get_mut("build_identity").and_then(J::as_dict_mut) {
            b.insert("image_digest_local", v);
        }
    };
    let Some(facts) = facts.filter(|f| f["id"].truthy()) else {
        set(ev, crate::jd! {"id" => J::Null, "source" => format!("docker image inspect {tag} 관측 불가(이미지 부재 · docker 미가용)")});
        return Ok(());
    };
    let created = _utc_z_soft(&facts["created"])?;
    let measured = _measured_key(repo, ev)?;
    let iid = facts["id"].display();
    let att_note = _node_image_id_note(&ev["attestation"], &iid)?;
    let who = if jeq(&ev["plane"], "native") { "원천(native wheelhouse 재포장 원천) " } else { "" };
    let base = format!(
        "docker image inspect {tag} .Id · .Created {}(현 로컬 {who}이미지 관측 · 태그는 가변 포인터){att_note}",
        created.clone().filter(|s| !s.is_empty()).unwrap_or_else(|| "미관측".into())
    );
    let c = created.clone().filter(|s| !s.is_empty());
    let m = measured.clone().filter(|s| !s.is_empty());
    let v = match (&c, &m) {
        (Some(cv), Some(mv)) if cv < mv => crate::jd! {"id" => iid, "created" => J::from(created.clone()), "same_build" => true,
            "source" => format!("{base} · Created < 측정 {mv} → 측정 전 빌드 — 같은 빌드로 판정 가능(그 사이 재태깅은 관측 밖)")},
        _ => {
            let why = match (&c, &m) {
                (Some(cv), Some(mv)) => format!("Created {cv} ≥ 측정 {mv} → 측정 뒤 빌드 — 측정 이미지 아님"),
                _ => "Created 또는 측정 시각 미관측 — 측정 전후 판정 불가".to_string(),
            };
            crate::jd! {"id" => iid, "created" => J::from(created.clone()), "same_build" => false, "source" => format!("{base} · {why}")}
        }
    };
    set(ev, v);
    Ok(())
}

pub fn _node_image_id_note(att: &J, iid: &str) -> HR<String> {
    let nodes = if att.is_dict() { att["nodes"].clone() } else { J::Null };
    let mut by: Vec<(String, String)> = Vec::new();
    if let Some(m) = nodes.as_dict() {
        let mut items: Vec<(&str, &J)> = m.iter().collect();
        items.sort_by(|a, b| a.0.cmp(b.0));
        for (n, d) in items {
            if d.is_dict() && d["image_id"].truthy() {
                let v = d["image_id"].display();
                if let Some(slot) = by.iter_mut().find(|(k, _)| k == n) {
                    slot.1 = v;
                } else {
                    by.push((n.to_string(), v));
                }
            }
        }
    }
    if by.is_empty() {
        return Ok(String::new());
    }
    let parts: Vec<String> = by
        .iter()
        .map(|(n, v)| format!("{n} {}", if v == iid { "같다".to_string() } else { format!("다르다({}…)", core::head_chars(v, 19)) }))
        .collect();
    let mut note = format!(" · 묶인 attestation 노드별 image_id({}): {}", att["_path"].display(), parts.join(" · "));
    let distinct: std::collections::BTreeSet<&String> = by.iter().map(|(_, v)| v).collect();
    if distinct.len() > 1 {
        note += " — 노드별 로컬 빌드라 digest 가 노드마다 다른 것은 정상(요건은 동일 ABI 이지 동일 digest 가 아니다 · 이 행의 Id 는 메인 로컬 관측)";
    }
    Ok(note)
}

/// docker `.Created` → `YYYY-MM-DDTHH:MM:SSZ` | None.
pub fn _utc_z_soft(v: &J) -> HR<Option<String>> {
    let Some(s) = v.as_str().map(pstrip).filter(|s| !s.is_empty()) else { return Ok(None) };
    let Some(m) = e1_created_re().match_at_start(s) else { return Ok(None) };
    let (d, t, z) = (crate::pyre::group(&m, 1).unwrap(), crate::pyre::group(&m, 2).unwrap(), crate::pyre::group(&m, 3).unwrap());
    if z == "Z" {
        return Ok(Some(format!("{d}T{t}Z")));
    }
    let Some(c) = core::parse_civil(&format!("{d}T{t}")) else { return Ok(None) };
    let sign: i64 = if z.starts_with('-') { -1 } else { 1 };
    let (oh, om): (i64, i64) = (z[1..3].parse().unwrap_or(99), z[4..6].parse().unwrap_or(99));
    if oh >= 24 || om >= 60 {
        return Ok(None);
    }
    let utc = core::Civil::from_epoch(c.epoch() - sign * (oh * 3600 + om * 60));
    Ok(Some(format!("{}Z", utc.iso())))
}

pub fn _bind_resolve(repo: &Path, topology: &str, build: &J) -> HR<(Option<J>, String)> {
    let p = repo.join("output").join(topology).join("resolved.json");
    let doc = _read_json_opt(&p, "resolved.json")?;
    if !doc.is_dict() {
        return Ok((None, format!("output/{topology}/resolved.json 부재")));
    }
    let r = &build["vllm_ref"];
    let ref_ = plower(pstrip(&if r.truthy() { r.display() } else { String::new() }));
    let rel = crate::naming::release_of(&J::from(ref_.clone()))?;
    let want: Option<String> = match rel.filter(|s| !s.is_empty()) {
        Some(r) => Some(r),
        None if build["vllm_version"].truthy() => Some(build["vllm_version"].display().trim_start_matches('v').to_string()),
        None => None,
    };
    let gv = &doc["vllm_version"];
    let got = (if gv.truthy() { gv.display() } else { String::new() }).trim_start_matches('v').to_string();
    let delta = if doc["upstream_delta"].is_dict() { doc["upstream_delta"].clone() } else { J::dict() };
    let ts = &delta["to_sha"];
    let to_sha = plower(&if ts.truthy() { ts.display() } else { String::new() });
    let by_sha = _SHA40().is_match(&ref_) && to_sha == ref_;
    if !by_sha && (want.as_deref().map_or(true, str::is_empty) || Some(got.as_str()) != want.as_deref()) {
        let ts12 = core::head_chars(&to_sha, 12);
        let ts_r = if ts12.is_empty() { "None".to_string() } else { crate::value::py_repr(&ts12) };
        let w = want.clone().filter(|s| !s.is_empty()).or_else(|| Some(core::head_chars(&ref_, 12)).filter(|s| !s.is_empty()));
        let w_r = w.map(|s| crate::value::py_repr(&s)).unwrap_or_else(|| "None".into());
        return Ok((
            None,
            format!(
                "output/{topology}/resolved.json vllm_version={}·to_sha={ts_r} ≠ 빌드 입력 {w_r} — 다른 해소값이라 묶지 않는다",
                crate::value::py_repr(&got)
            ),
        ));
    }
    let sub = |k: &str| if doc[k].is_dict() { doc[k].clone() } else { J::dict() };
    let (torch, ngc, bt, sb) = (sub("torch"), sub("ngc_base"), sub("build_track"), sub("source_build"));
    let source = if by_sha {
        format!("output/{topology}/resolved.json(upstream_delta.to_sha = 빌드 입력 커밋 핀)")
    } else {
        format!("output/{topology}/resolved.json(vllm_version={got} = 빌드 입력)")
    };
    Ok((
        Some(crate::jd! {"vllm_version" => doc["vllm_version"].clone(), "torch_pin" => torch["pin"].clone(), "ngc_tag" => ngc["tag"].clone(),
            "ngc_image" => ngc["image"].clone(), "ngc_cuda" => ngc["cuda_version"].clone(), "ngc_torch_build" => ngc["build_version"].clone(),
            "build_track" => bt["decision"].clone(), "torch_cuda_arch" => sb["torch_cuda_arch"].clone(),
            "to_ref" => delta["to_ref"].clone(), "to_sha" => if _SHA40().is_match(&to_sha) { J::from(to_sha.clone()) } else { J::Null },
            "from_ref" => delta["from_ref"].clone(), "generated_kst" => delta["generated_kst"].clone(), "source" => source}),
        format!("output/{topology}/resolved.json"),
    ))
}

pub fn _bind_attestation(repo: &Path, topology: &str, node: &str, cell: &str, build: &J) -> HR<(Option<J>, Vec<String>, String)> {
    if topology != "multi" || node != "cluster" {
        return Ok((None, vec![], format!("{topology}/{node} — 노드 정합 attestation 대상 아님(cluster 축 전용)")));
    }
    let bench = repo.join("output").join(topology).join("benchlog");
    let mut names: Vec<(String, String, &str)> = vec![(format!("attestation_{cell}.json"), "셀 이름".into(), "cell-name")];
    let project = if build["compose_project"].truthy() { build["compose_project"].display() } else { String::new() };
    if let Some(pm) = e1_project_re().fullmatch(&project) {
        let g = crate::pyre::group(&pm, 1).unwrap_or("");
        if g != cell {
            names.push((format!("attestation_{g}.json"), format!("이미지 compose 라벨 {project}"), "compose-label"));
        }
    }
    let mut refused: Vec<String> = Vec::new();
    for (name, why, bound_by) in &names {
        let doc = _read_json_opt(&bench.join(name), "attestation")?;
        if !doc.is_dict() {
            continue;
        }
        if doc["image_tag"].truthy() && build["image_tag"].truthy() && doc["image_tag"] != build["image_tag"] {
            refused.push(format!("{name}: image_tag {} ≠ {}", doc["image_tag"].repr(), build["image_tag"].repr()));
            continue;
        }
        let digest = &build["image_digest"];
        let ids: Vec<String> = match doc["nodes"].as_dict() {
            Some(m) => crate::value::sorted_strs(
                m.values().filter(|n| n.is_dict() && n["image_id"].truthy()).map(|n| n["image_id"].display()).collect::<std::collections::BTreeSet<_>>(),
            ),
            None => vec![],
        };
        if digest.truthy() && !ids.is_empty() && !ids.contains(&digest.display()) {
            refused.push(format!(
                "{name}: 노드 image_id {} 에 측정 digest {}… 가 없다",
                J::from(ids.clone()).repr(),
                core::head_chars(&digest.display(), 19)
            ));
            continue;
        }
        let mut att = doc.as_dict().cloned().unwrap();
        let p = _rel(repo, &bench.join(name))?;
        att.insert("_path", p.clone());
        let s = format!("{p} ({why})");
        att.insert("_source", s.clone());
        att.insert("_bound_by", *bound_by);
        let miss = if _attestation_has_rows(&doc)? { vec![] } else { vec!["HINT_MISSING_ATTESTATION_PARITY_ROWS".to_string()] };
        return Ok((Some(J::Dict(att)), miss, s));
    }
    let listed: Vec<&str> = names.iter().map(|(n, _, _)| n.as_str()).collect();
    let mut msg = format!("attestation 부재({})", listed.join(", "));
    if !refused.is_empty() {
        msg += &format!(" · 다른 이미지라 묶지 않음: {}", J::from(refused).repr());
    }
    Ok((None, vec!["HINT_MISSING_SLAVE_ATTESTATION".into()], msg))
}

pub fn _attestation_has_rows(doc: &J) -> HR<bool> {
    if matches!(&doc["checks"], J::List(l) if !l.is_empty()) {
        return Ok(true);
    }
    if let Some(p) = doc["parity"].as_dict() {
        return Ok(p.values().any(|r| r.is_dict() && j_in(&r["verdict"], &["equal", "differ"])));
    }
    Ok(false)
}

pub fn _triplet(repo: &Path, topology: &str, cell: &str) -> HR<J> {
    let mut out = Map::new();
    for (k, v) in [
        ("yaml", format!("output/{topology}/configs/{cell}.yaml")),
        ("sh", format!("output/{topology}/configs/{cell}.sh")),
        ("env", format!("output/{topology}/envs/.env.{cell}")),
    ] {
        if repo.join(&v).is_file() {
            out.insert(k, v);
        }
    }
    Ok(J::Dict(out))
}

pub fn _serve_proof(repo: &Path, topology: &str, cell: &str) -> HR<(Option<J>, Option<String>)> {
    let p = repo.join("output").join(topology).join("benchlog").join(format!("serve_proof_{cell}.json"));
    let doc = _read_json_opt(&p, "serve_proof")?;
    if doc.is_dict() {
        let r = _rel(repo, &p)?;
        Ok((Some(doc), Some(r)))
    } else {
        Ok((None, None))
    }
}

pub fn _preserved_native_file(repo: &Path, raw: &J, field: &str) -> HR<(String, PathBuf)> {
    let Some(r) = raw.as_str().filter(|s| !pstrip(s).is_empty()) else {
        return core::fail(
            "HINT_NATIVE_EVIDENCE_MISSING",
            format!("native serve proof의 {field} 가 없다."),
            Some("native producer가 run-root 삭제 전에 보존한 repo-relative evidence 경로를 기록해야 한다."),
        );
    };
    let rel = pstrip(r).to_string();
    if Path::new(&rel).is_absolute() {
        return core::fail(
            "HINT_NATIVE_EVIDENCE_PATH_ABSOLUTE",
            format!("native serve proof의 {field} 는 repo-relative 여야 한다: {}", crate::value::py_repr(&rel)),
            None,
        );
    }
    // pathlib `repo / rel` (`.` components collapse — the path text is part of the messages)
    let path: PathBuf = repo
        .components()
        .chain(Path::new(&rel).components())
        .filter(|c| *c != std::path::Component::CurDir)
        .collect();
    let path = if path.as_os_str().is_empty() { PathBuf::from(".") } else { path };
    let clean = core::rel(repo, &path)?;
    let is_link = std::fs::symlink_metadata(&path).map(|m| m.file_type().is_symlink()).unwrap_or(false);
    if !path.is_file() || is_link {
        return core::fail("HINT_NATIVE_EVIDENCE_UNREADABLE", format!("native serve proof의 {field} 가 regular file이 아니다: {clean}"), None);
    }
    Ok((clean, path))
}

pub fn _native_cleanup_attestation(repo: &Path, proof: &J) -> HR<(String, J)> {
    let (rel, path) = _preserved_native_file(repo, &proof["cleanup_attestation_path"], "cleanup_attestation_path")?;
    let doc = _read_json_opt(&path, "native cleanup attestation")?;
    if !doc.is_dict() {
        return core::fail("HINT_NATIVE_CLEANUP_UNOBSERVED", format!("native cleanup attestation이 JSON object가 아니다: {rel}"), None);
    }
    if !jeq(&doc["kind"], "native_multinode_cleanup_attestation") || !jeq(&doc["plane"], "native") {
        return core::fail(
            "HINT_NATIVE_CLEANUP_INVALID",
            format!("native cleanup attestation 정체가 맞지 않다: kind={}, plane={}", doc["kind"].repr(), doc["plane"].repr()),
            None,
        );
    }
    let empty = |v: &J| matches!(v, J::List(l) if l.is_empty());
    if !jeq(&doc["status"], "PASS") || !empty(&doc["errors"]) || doc["evidence_preserved"] != J::Bool(true) {
        return core::fail(
            "HINT_NATIVE_CLEANUP_NONPASS",
            "native cleanup attestation이 PASS·errors=[]·evidence_preserved=true를 모두 만족하지 않는다.",
            Some("cleanup 실패·unknown은 native hint 발행을 차단한다."),
        );
    }
    let ok_nodes = doc["nodes"].as_dict().map_or(false, |m| m.len() == 2 && m.contains_key("main") && m.contains_key("sub"));
    if !ok_nodes {
        return core::fail("HINT_NATIVE_CLEANUP_INVALID", "native cleanup attestation의 nodes는 정확히 main·sub여야 한다.", None);
    }
    for name in ["main", "sub"] {
        let n = &doc["nodes"][name];
        if !n.is_dict()
            || n["root_absent"] != J::Bool(true)
            || !empty(&n["owned_processes"])
            || !empty(&n["owned_gpu_processes"])
            || !empty(&n["unknown"])
        {
            return core::fail(
                "HINT_NATIVE_CLEANUP_NONPASS",
                format!("native cleanup attestation nodes.{name}가 완전한 cleanup PASS가 아니다."),
                Some("root_absent=true, owned_processes=[], owned_gpu_processes=[], unknown=[]이 모두 필요하다."),
            );
        }
    }
    if proof["run_id"] != doc["run_id"] {
        return core::fail("HINT_NATIVE_CLEANUP_RUN_MISMATCH", "serve proof와 cleanup attestation의 run_id가 다르다.", None);
    }
    Ok((rel, doc))
}

pub fn _native_producer_evidence(repo: &Path, topology: &str, cell: &str, proof: &J, proof_rel: Option<&str>) -> HR<J> {
    if !proof.is_dict() || !jeq(&proof["plane"], "native") {
        return Ok(J::dict());
    }
    if topology != "multi" || !jeq(&proof["kind"], "native_multinode_serve_proof") || !jeq(&proof["status"], "PASS") {
        return core::fail("HINT_NATIVE_PROOF_INVALID", "native serve proof의 topology/kind/status가 producer 계약과 맞지 않는다.", None);
    }
    let pc = match proof.get("cell") {
        Some(v) => v.clone(),
        None => proof["config"].clone(),
    };
    if !jeq(&pc, cell) {
        return core::fail("HINT_NATIVE_PROOF_CELL_MISMATCH", format!("native serve proof cell/config가 {}와 다르다.", crate::value::py_repr(cell)), None);
    }
    let d = |v: &J| if v.is_dict() { v.clone() } else { J::dict() };
    let health = d(&proof["health"]);
    let endpoints = d(&proof["endpoints"]);
    let h_ep = d(&endpoints["health"]);
    let i_ep = d(&endpoints["inference"]);
    let inference = d(&proof["inference"]);
    if health["ok"] != J::Bool(true)
        || h_ep["http_status"].display() != "200"
        || inference["ok"] != J::Bool(true)
        || i_ep["http_status"].display() != "200"
        || _as_int(&inference["completion_text_len"])?.unwrap_or(0) < 1
    {
        return core::fail("HINT_NATIVE_PROOF_UNOBSERVED", "native serve proof에 health 200 + inference 200 + completion_text_len>0 관측이 없다.", None);
    }
    let (install_rel, _) = _preserved_native_file(repo, &proof["native_install_path"], "native_install_path")?;
    let freezes = &proof["pip_freeze_paths"];
    if !freezes.as_dict().map_or(false, |m| m.len() == 2 && m.contains_key("main") && m.contains_key("sub")) {
        return core::fail("HINT_NATIVE_EVIDENCE_MISSING", "native serve proof의 pip_freeze_paths는 정확히 main·sub 보존 경로여야 한다.", None);
    }
    let (freeze_rel, _) = _preserved_native_file(repo, &freezes["main"], "pip_freeze_paths.main")?;
    let (sub_rel, _) = _preserved_native_file(repo, &freezes["sub"], "pip_freeze_paths.sub")?;
    let (cleanup_rel, cleanup) = _native_cleanup_attestation(repo, proof)?;
    let launch_raw = &proof["native_launch_path"];
    let launch_rel = if launch_raw.is_null() || jeq(launch_raw, "") {
        None
    } else {
        Some(_preserved_native_file(repo, launch_raw, "native_launch_path")?.0)
    };
    let launch_err = match proof["native_launch_error"].as_str() {
        Some(e) if !pstrip(e).is_empty() && launch_rel.is_none() => Some(core::tail_chars(pstrip(e), 400)),
        _ => None,
    };
    Ok(crate::jd! {"plane" => "native", "native_install_path" => install_rel, "pip_freeze_path" => freeze_rel.clone(),
        "pip_freeze_paths" => crate::jd!{"main" => freeze_rel, "sub" => sub_rel},
        "cleanup_attestation_path" => cleanup_rel, "cleanup_attestation" => cleanup,
        "native_launch_path" => J::from(launch_rel), "native_launch_error" => J::from(launch_err),
        "source" => proof_rel.filter(|s| !s.is_empty()).unwrap_or("serve_proof")})
}

pub fn _measured_meta(ev_sweep: &J, cert: &J) -> HR<J> {
    let meta = &ev_sweep["index"]["meta"];
    let mut out = Map::new();
    for key in ["image_tag", "image_digest"] {
        if !unobs(&meta[key]) {
            out.insert(key, meta[key].clone());
            out.insert(format!("{key}_source"), format!("sweep_index.meta.{key}({})", or_str(&meta[&*format!("{key}_source")], "출처 미표기")));
        } else if cert.truthy() && !unobs(&cert[key]) {
            out.insert(key, cert[key].clone());
            out.insert(format!("{key}_source"), format!("인증서 {key}"));
        }
    }
    Ok(J::Dict(out))
}

pub fn _report_born_utc(repo: &Path, rel_path: Option<&str>) -> HR<Option<String>> {
    let Some(rel) = rel_path.filter(|s| !s.is_empty()) else { return Ok(None) };
    let p = repo.join(rel);
    if !p.is_file() {
        return Ok(None);
    }
    let text = read_replace(&p).map_err(|e| core::io_err(&e, &p))?;
    let head: Vec<&str> = splitlines(&text).into_iter().take(12).collect();
    Ok(_REPORT_BORN().search(&head.join("\n")).and_then(|m| crate::pyre::group(&m, 1).map(str::to_string)))
}

pub fn _require_report_joins_certificate(repo: &Path, report_rel: Option<&str>, cert: &J) -> HR<()> {
    let Some(rr) = report_rel.filter(|s| !s.is_empty()) else { return Ok(()) };
    if !cert.truthy() {
        return Ok(());
    }
    let born = _report_born_utc(repo, Some(rr))?;
    let measured = _norm_value(&cert["measured_utc"])?;
    if let Some(b) = born.filter(|s| !s.is_empty()) {
        if measured.truthy() && !jeq(&measured, &b) {
            return core::fail(
                "HINT_BENCH_REPORT_CERT_MISMATCH",
                format!("리포트 {rr} 생성일 {b} ≠ 인증서 measured_utc {} — 다른 측정이다.", measured.display()),
                Some("같은 측정의 리포트(인증서 이름 쌍)를 가리키도록 캠페인 포인터를 정리한다 — 발행기가 둘을 섞지 않는다."),
            );
        }
    }
    Ok(())
}

pub fn _report_twin(cert_rel: Option<&str>) -> HR<Option<String>> {
    let Some(c) = cert_rel.filter(|s| !s.is_empty()) else { return Ok(None) };
    let name = pname(c);
    if !(name.starts_with("benchmark_") && name.ends_with(".yaml")) || name.len() < "benchmark_".len() + ".yaml".len() {
        return Ok(None);
    }
    let mid = &name["benchmark_".len()..name.len() - ".yaml".len()];
    let parent = Path::new(c).parent().map(core::posix).unwrap_or_default();
    let twin = format!("bench_report_{mid}.md");
    Ok(Some(if parent.is_empty() {
        twin
    } else if parent.ends_with('/') {
        format!("{parent}{twin}")
    } else {
        format!("{parent}/{twin}")
    }))
}

fn table_rows(repo: &Path, rel_path: Option<&str>, heading: &str) -> HR<Option<Vec<Vec<String>>>> {
    let Some(rel) = rel_path.filter(|s| !s.is_empty()) else { return Ok(None) };
    let p = repo.join(rel);
    if !p.is_file() {
        return Ok(None);
    }
    let text = read_replace(&p).map_err(|e| core::io_err(&e, &p))?;
    let mut rows = Vec::new();
    let mut on = false;
    for ln in splitlines(&text) {
        let s = pstrip(ln);
        if s.starts_with("## ") {
            on = s.starts_with(heading);
            continue;
        }
        if on && s.starts_with('|') {
            rows.push(s.trim_matches('|').split('|').map(|c| pstrip(c).to_string()).collect());
        }
    }
    Ok(Some(rows))
}

pub fn _lite_snapshot(repo: &Path, rel_path: Option<&str>) -> HR<J> {
    let mut out = Map::new();
    for cells in table_rows(repo, rel_path, "## 측정 환경 스냅샷")?.unwrap_or_default() {
        if cells.len() == 2 && cells[0] != "키" && !cells[0].is_empty() && !cells[0].chars().all(|c| c == '-') {
            out.insert(cells[0].clone(), cells[1].clone());
        }
    }
    Ok(J::Dict(out))
}

// ── 결손 · 측정 ──────────────────────────────────────────────────────────────────────────────────────────────
pub fn _report_missing(repo: &Path, cert: &J, report_rel: Option<&str>) -> HR<(Vec<String>, Option<String>)> {
    let mut missing: Vec<String> = Vec::new();
    let mut kind = None;
    _rbs(repo)?;
    let text = match report_rel.filter(|s| !s.is_empty()) {
        Some(r) if repo.join(r).is_file() => Some(read_replace(&repo.join(r)).map_err(|e| core::io_err(&e, &repo.join(r)))?),
        _ => None,
    };
    match &text {
        None => missing.push("HINT_MISSING_BENCH_REPORT".into()),
        Some(t) if crate::render_bench_section::is_lite_report(t)? => {
            kind = Some("lite".to_string());
            missing.push("HINT_MISSING_SWEEP_LEVELS".into());
        }
        Some(t) => {
            kind = Some("full".to_string());
            match crate::render_bench_section::parse_report(t) {
                Ok(r) => {
                    if r["levels"].len() <= 1 {
                        missing.push("HINT_MISSING_SWEEP_LEVELS".into());
                    }
                }
                Err(e) if e.code == "py:BenchSectionFailure" => missing.push("HINT_MISSING_BENCH_REPORT".into()),
                Err(e) => return Err(e),
            }
        }
    }
    if cert.truthy() {
        let li = &cert["lite_included"];
        let li_s = if li.is_null() && cert.get("lite_included").is_none() { String::new() } else { li.display() };
        let wt = &cert["lite_gen_tps_warm"];
        let wt_s = if cert.get("lite_gen_tps_warm").is_none() { String::new() } else { wt.display() };
        if plower(pstrip(&li_s)) != "true" || !e1_lead_digit_re().is_match(pstrip(&wt_s)) {
            missing.push("HINT_MISSING_LITE".into());
        }
        if unobs(&cert["measured_node"]) {
            missing.push("HINT_MISSING_MEASURED_NODE".into());
        }
    } else {
        missing.push("HINT_MISSING_CERTIFICATE".into());
        if let Some(t) = &text {
            if !crate::render_bench_section::lite_metrics_present(t)? {
                missing.push("HINT_MISSING_LITE".into());
            }
        }
    }
    Ok((missing, kind))
}

fn report_text(repo: &Path, rep: &J) -> HR<Option<String>> {
    match rep.as_str().filter(|s| !s.is_empty()) {
        Some(r) if repo.join(r).is_file() => Ok(Some(read_replace(&repo.join(r)).map_err(|e| core::io_err(&e, &repo.join(r)))?)),
        _ => Ok(None),
    }
}

fn opt_name(rep: &J) -> String {
    if rep.truthy() {
        pname(&rep.display())
    } else {
        "None".into()
    }
}

pub fn measurement_config(repo: &Path, ev: &J) -> HR<J> {
    let repo = core::resolve(repo);
    _rbs(&repo)?;
    let mut out = Map::new();
    for k in DISTRIBUTED_MEASUREMENT_KEYS {
        out.insert(k, J::Null);
    }
    let mut sources: Vec<String> = Vec::new();
    let rep = &ev["bench_report_path"];
    let name = opt_name(rep);
    let text = report_text(&repo, rep)?;
    if let Some(t) = &text {
        let mc = match crate::render_bench_section::parse_measurement_config(t) {
            Ok(v) => v,
            Err(e) if e.code == "py:BenchSectionFailure" => {
                sources.push(format!("unparseable(bench_report({name}) 측정 구성 표 — 파싱 실패)"));
                None
            }
            Err(e) => return Err(e),
        };
        if let Some(mc) = mc.filter(J::is_dict) {
            for k in DISTRIBUTED_MEASUREMENT_KEYS {
                if !mc[k].is_null() {
                    out.insert(k, mc[k].clone());
                }
            }
            sources.push(format!("bench_report({name})"));
        }
    }
    let cert = if ev["certificate"].truthy() { ev["certificate"].clone() } else { J::dict() };
    let mut filled = false;
    for (k, ck) in [("bench_mode", "benchmark_mode"), ("bench_tool", "bench_tool"), ("bench_tool_version", "bench_tool_version")] {
        let v = &cert[ck];
        if m_get(&out, k).is_null() && !unobs(v) {
            out.insert(k, v.clone());
            filled = true;
        }
    }
    if filled {
        sources.push(format!("certificate({})", pname(&or_str(&ev["certificate_path"], ""))));
    }
    if m_get(&out, "bench_mode").is_null() {
        if let Some(t) = &text {
            if crate::render_bench_section::is_lite_report(t)? {
                out.insert("bench_mode", "lite");
                sources.push(format!("bench_report({name}) 헤더 mode: lite"));
            }
        }
    }
    let rec_mode = &ev["publication"]["record"]["benchmark"]["mode"];
    if m_get(&out, "bench_mode").is_null() && jeq(rec_mode, "lite") {
        out.insert("bench_mode", "lite");
        sources.push("publication record(benchmark.mode=lite)".into());
    }
    out.insert("source", if sources.is_empty() { "absent(측정 구성 표·인증서 모두 없다 — 미기재)".to_string() } else { sources.join(" + ") });
    Ok(J::Dict(out))
}

pub fn _sanitize_dataset_value(v: &J) -> HR<J> {
    let J::Str(s) = v else { return Ok(v.clone()) };
    Ok(J::Str(_DATASET_HOST_PATH_RE().sub_fn(s, |m| {
        let whole = m.get(0).unwrap().as_str().trim_end_matches('/');
        let base = whole.rsplit('/').next().unwrap_or("");
        if base.is_empty() {
            "<abs-path>".to_string()
        } else {
            format!("<abs-path>/{base}")
        }
    })))
}

pub fn dataset_provenance(repo: &Path, ev: &J) -> HR<J> {
    let repo = core::resolve(repo);
    _rbs(&repo)?;
    let mut out = Map::new();
    for k in DATASET_KEYS {
        out.insert(k, J::Null);
    }
    let mut sources: Vec<String> = Vec::new();
    let rep = &ev["bench_report_path"];
    let name = opt_name(rep);
    if let Some(t) = report_text(&repo, rep)? {
        let mc = match crate::render_bench_section::parse_measurement_config(&t) {
            Ok(v) => v,
            Err(e) if e.code == "py:BenchSectionFailure" => {
                sources.push(format!("unparseable(bench_report({name}) 측정 구성 표 — 파싱 실패)"));
                None
            }
            Err(e) => return Err(e),
        };
        if let Some(mc) = mc.filter(|m| m.is_dict() && DATASET_KEYS.iter().any(|k| !m[*k].is_null())) {
            for k in DATASET_KEYS {
                if !mc[k].is_null() {
                    out.insert(k, mc[k].clone());
                }
            }
            sources.push(format!("bench_report({name}) 측정 구성 표"));
        }
    }
    let cert = if ev["certificate"].truthy() { ev["certificate"].clone() } else { J::dict() };
    let mut filled = false;
    for k in DATASET_KEYS {
        if m_get(&out, k).is_null() && !unobs(&cert[k]) {
            out.insert(k, cert[k].clone());
            filled = true;
        }
    }
    if filled {
        sources.push(format!("certificate({}) 소프트 지문", pname(&or_str(&ev["certificate_path"], ""))));
    }
    let prov = &cert["dataset_provenance"];
    out.insert("dataset_provenance", if unobs(prov) { J::Null } else { prov.clone() });
    if DATASET_KEYS.iter().any(|k| !m_get(&out, k).is_null()) {
        out.insert("status", DATASET_STATUS_RECORDED);
        out.insert("source", sources.join(" + "));
    } else {
        out.insert("dataset_kind", "synthetic");
        out.insert("status", DATASET_STATUS_INFERRED);
        let tail = if sources.is_empty() { String::new() } else { format!(" · 판독: {}", sources.join(" + ")) };
        out.insert(
            "source",
            format!("inferred(부재 = synthetic — 리포트 측정 구성 표 · 인증서 어디에도 dataset_* 기재가 없다 · 데이터셋 축 도입(2026-10-16) 전 측정으로 읽는다 · 소급 ✗){tail}"),
        );
    }
    let mut res = Map::new();
    for (k, v) in out.iter() {
        res.insert(k, _sanitize_dataset_value(v)?);
    }
    Ok(J::Dict(res))
}

pub fn _surface(v: &J) -> HR<Option<String>> {
    Ok(match v {
        J::Null => None,
        J::Str(s) if unobs(v) => {
            let _ = s;
            None
        }
        J::Bool(b) => Some(if *b { "true" } else { "false" }.into()),
        J::Int(_) | J::Float(_) => Some(v.repr()),
        other => Some(pstrip(&other.display()).to_string()).filter(|s| !s.is_empty()),
    })
}

pub fn _dig(doc: &J, dotted: &str) -> HR<J> {
    let mut cur = doc.clone();
    for part in dotted.split('.') {
        if !cur.is_dict() {
            return Ok(J::Null);
        }
        cur = cur[part].clone();
    }
    Ok(cur)
}

pub fn _report_verdict_table(repo: &Path, rel_path: Option<&str>) -> HR<J> {
    let mut out = Map::new();
    for cells in table_rows(repo, rel_path, "## 판정")?.unwrap_or_default() {
        if cells.len() == 2 && cells[0] != "항목" && !cells[0].is_empty() && !cells[0].chars().all(|c| c == '-') {
            let v = pstrip(&cells[1].replace("**", "")).to_string();
            if cells[0] == "루브릭 primary" {
                if let Some(m) = e1_primary_re().match_at_start(&v) {
                    out.insert("루브릭 primary", crate::pyre::group(&m, 1).unwrap_or(""));
                    out.insert("_primary_source", crate::pyre::group(&m, 2).unwrap_or(""));
                    continue;
                }
            }
            out.insert(cells[0].clone(), e1_tps_suffix_re().sub_literal(&v, ""));
        }
    }
    Ok(J::Dict(out))
}

pub fn _lite_report_numbers(repo: &Path, rel_path: Option<&str>) -> HR<J> {
    let mut out = Map::new();
    let rows = [("gen tokens/sec (warm) [master]", "gen_tps", "t/s"), ("cold-start TTFT [master]", "cold_ttft_ms", "ms")];
    for cells in table_rows(repo, rel_path, "## lite 지표")?.unwrap_or_default() {
        if cells.len() >= 2 {
            if let Some((_, key, unit)) = rows.iter().find(|r| r.0 == cells[0]) {
                let re = crate::pyre::Re::new(&format!(r"([0-9]+(?:\.[0-9]+)?)\s*{}", fancy_regex::escape(unit)), 0);
                if let Some(m) = re.fullmatch(&cells[1]) {
                    if let Ok(f) = crate::pyre::group(&m, 1).unwrap_or("").parse::<f64>() {
                        out.insert(*key, f);
                    }
                }
            }
        }
    }
    Ok(J::Dict(out))
}

fn judged_keys() -> Vec<(String, String, Option<String>, Option<String>)> {
    _JUDGED_KEYS()
        .items()
        .iter()
        .map(|t| {
            let o = |i: usize| t.at(i).as_str().map(str::to_string);
            (o(0).unwrap(), o(1).unwrap(), o(2), o(3))
        })
        .collect()
}

pub fn _judged(repo: &Path, ev: &J) -> HR<(J, J, Vec<String>)> {
    let mut vals = Map::new();
    let mut srcs = Map::new();
    let mut reasons: Vec<String> = Vec::new();
    let sw = &ev["sweep"];
    let vj = &sw["verdict"];
    if vj.is_dict() {
        let where_ = format!("{}/verdict.json({} · verdict_rule 출력)", sw["dir"].display(), sw["binding"].display());
        for (key, path, _, _) in judged_keys() {
            if let Some(v) = _surface(&_dig(vj, &path)?)? {
                if !vals.contains_key(&key) {
                    vals.insert(key.clone(), v);
                    srcs.insert(key, format!("{where_} {path}"));
                }
            }
        }
        reasons = vj["reasons"]
            .items()
            .iter()
            .filter_map(|r| r.as_str())
            .filter(|r| !_ABS_PATH_RE().is_search(r))
            .map(str::to_string)
            .collect();
    }
    let brp = ev["bench_report_path"].as_str().map(str::to_string);
    let table = _report_verdict_table(repo, brp.as_deref())?;
    if table.truthy() {
        let where_ = format!("bench_report({}) `## 판정` 표", pname(&or_str(&ev["bench_report_path"], "")));
        for (key, _, item, _) in judged_keys() {
            let Some(item) = item else { continue };
            if let Some(v) = _surface(&table[&*item])? {
                if !vals.contains_key(&key) {
                    vals.insert(key.clone(), v);
                    srcs.insert(key, format!("{where_} {item}"));
                }
            }
        }
        if !vals.contains_key("primary_source") && table["_primary_source"].truthy() {
            vals.insert("primary_source", table["_primary_source"].clone());
            srcs.insert("primary_source", format!("{where_} 루브릭 primary(괄호)"));
        }
    }
    let bm = &ev["publication"]["record"]["benchmark"];
    if bm.is_dict() && bm.truthy() {
        let where_ = format!(
            "발행 기록 benchmark({} · rubric_source={})",
            ev["publication"]["record_path"].display(),
            bm["rubric_source"].display()
        );
        for (key, _, _, rk) in judged_keys() {
            let Some(rk) = rk else { continue };
            if let Some(v) = _surface(&bm[&*rk])? {
                if !vals.contains_key(&key) {
                    vals.insert(key.clone(), v);
                    srcs.insert(key, format!("{where_} {rk}"));
                }
            }
        }
    }
    if let Some(v) = vals.get("verdict").cloned() {
        let up = v.display().to_uppercase();
        vals.insert("verdict", up.clone());
        if up == _JUDGE_NO_BASELINE_VERDICT {
            vals.insert("verdict", OBSERVATION_ONLY_VERDICT);
            let s = m_get(&srcs, "verdict").display();
            srcs.insert(
                "verdict",
                format!("{s} ({_JUDGE_NO_BASELINE_VERDICT} → {OBSERVATION_ONLY_VERDICT} · 기준 c·E·expected 부재)"),
            );
        }
    }
    Ok((J::Dict(vals), J::Dict(srcs), reasons))
}

pub fn _certificate_not_due(repo: &Path, ev: &J) -> HR<Option<String>> {
    if jeq(&ev["report_kind"], "lite") {
        return Ok(Some("lite-only 셀 — 인증서는 full 측정에서만 나온다(판정 OBSERVATION-ONLY · 결손 아님)".into()));
    }
    let (jv, js, _r) = _judged(repo, ev)?;
    let verdict = jv["verdict"].clone();
    let mut authority = jv["rubric_authority"].clone();
    let asrc = if js["rubric_authority"].truthy() { js["rubric_authority"].display() } else { String::new() };
    let rec_bm = &ev["publication"]["record"]["benchmark"];
    if asrc.starts_with("발행 기록") && !jeq(&rec_bm["rubric_source"], "verdict_json") {
        authority = J::Null;
    }
    if j_in(&verdict, &["REFUTE", "FAIL"]) {
        return Ok(Some(format!(
            "{} — 인증서는 PASS 전용이다(판정 {} · 결손 아님 · 판정 근거 = bench_report)",
            verdict.display(),
            js["verdict"].display()
        )));
    }
    if jeq(&verdict, OBSERVATION_ONLY_VERDICT) {
        return Ok(Some(format!(
            "OBSERVATION-ONLY — 기준(c·E·expected)이 없어 판정을 주장하지 않는다(판정 {} · 결손 아님 · 계측 근거 = bench_report)",
            js["verdict"].display()
        )));
    }
    if jeq(&verdict, "PASS") {
        let _g = cgate(repo)?;
        if j_in(&authority, &CGate::CERTIFICATE_NON_ISSUING_AUTHORITIES) {
            return Ok(Some(format!(
                "PASS ∧ 비발행 권한 {}({asrc}) — judge_bench 는 explicit ∧ PASS 만 인증서를 낸다(결손 아님 · 판정 근거 = bench_report)",
                authority.display()
            )));
        }
    }
    Ok(None)
}

pub fn _num_equal(a: &str, b: &str) -> HR<bool> {
    let (Some(fa), Some(fb)) = (py_float(a), py_float(b)) else {
        return Ok(plower(pstrip(a)) == plower(pstrip(b)));
    };
    let decimals = |s: &str| -> i32 {
        let s = pstrip(s);
        if s.contains('.') && !s.to_lowercase().contains('e') {
            s.split_once('.').unwrap().1.chars().count() as i32
        } else {
            0
        }
    };
    let tol = 0.5 * 10f64.powi(-decimals(a).min(decimals(b))) + 1e-9;
    Ok((fa - fb).abs() <= tol)
}

pub fn cert_is_lite(cert: &J) -> HR<bool> {
    Ok(cert.is_dict() && {
        let v = &cert["benchmark_mode"];
        pstrip(&if v.truthy() { v.display() } else { String::new() }) == "lite"
    })
}

pub fn measurement(ev: &J) -> HR<J> {
    let repo = core::resolve(Path::new(&ev_str(ev, "repo").filter(|s| !s.is_empty()).unwrap_or_else(|| ".".into())));
    let mut out = Map::new();
    let mut srcs: Vec<String> = Vec::new();
    let mut per = Map::new();
    let cert = if ev["certificate"].truthy() { ev["certificate"].clone() } else { J::dict() };
    let cname = format!("certificate({})", pname(&or_str(&ev["certificate_path"], "")));
    let lite_cert = cert_is_lite(&cert)?;
    for k in _CERT_MEASUREMENT_KEYS {
        if let Some(v) = cert.get(k) {
            if !(lite_cert && k == "verdict") {
                out.insert(k, v.clone());
                per.insert(k, format!("{cname} {k}"));
            }
        }
    }
    if cert.truthy() {
        srcs.push(cname.clone());
    }
    let (jv, js, reasons) = _judged(&repo, ev)?;
    if cert.truthy() {
        let mut bad: Vec<String> = Vec::new();
        for k in _CROSSCHECK_KEYS {
            if let Some(j) = jv.get(k) {
                if lite_cert && k == "verdict" {
                    continue;
                }
                if !unobs(&cert[k]) && !_num_equal(&cert[k].display(), &j.display())? {
                    bad.push(format!("{k}: 인증서={} {}={}", cert[k].repr(), js[k].display(), j.repr()));
                }
            }
        }
        if !bad.is_empty() {
            return core::fail(
                "HINT_MEASUREMENT_VERDICT_MISMATCH",
                format!("인증서와 같은 측정의 판정 원천이 다른 값을 말한다: {}", J::from(bad).repr()),
                Some("인증서가 이 측정(스윕 generated_utc = measured_utc)의 판정에서 나왔는지 확인한다 — 발행기가 둘 중 하나를 고르지 않는다."),
            );
        }
    }
    for (k, v) in jv.as_dict().unwrap().iter() {
        if !out.contains_key(k) || unobs(&m_get(&out, k)) {
            out.insert(k, v.clone());
            per.insert(k, js[k].clone());
        }
    }
    if jv.truthy() && !cert.truthy() {
        let kinds: std::collections::BTreeSet<&str> = js
            .as_dict()
            .unwrap()
            .values()
            .map(|v| {
                let s = v.display();
                if s.contains("verdict.json") {
                    "verdict.json"
                } else if s.contains("`## 판정` 표") {
                    "bench_report 판정 표"
                } else {
                    "발행 기록 benchmark"
                }
            })
            .collect();
        srcs.push(format!("판정 원천({})", kinds.into_iter().collect::<Vec<_>>().join(" · ")));
    }
    if !reasons.is_empty() && jeq(&m_get(&out, "verdict"), "REFUTE") {
        out.insert("verdict_reasons", J::from(reasons.clone()));
        per.insert("verdict_reasons", js.get("verdict").cloned().unwrap_or_else(|| J::from("verdict.json reasons")));
    }
    let mc = measurement_config(&repo, ev)?;
    if unobs(&m_get(&out, "benchmark_mode")) && mc["bench_mode"].truthy() {
        out.insert("benchmark_mode", mc["bench_mode"].clone());
        per.insert("benchmark_mode", format!("measurement_config.bench_mode({})", mc["source"].display()));
    }
    let report_lite = jeq(&ev["report_kind"], "lite");
    if unobs(&m_get(&out, "verdict")) {
        if jeq(&m_get(&out, "benchmark_mode"), "lite") || report_lite {
            out.insert("verdict", "OBSERVATION-ONLY");
            per.insert(
                "verdict",
                format!(
                    "lite-only 셀 — verdict_rule 에 투입되지 않았다(경량 리포트 `inform-only` · 판정 원천 없음 · benchmark_mode={} · report_kind={})",
                    m_get(&out, "benchmark_mode").display(),
                    ev["report_kind"].display()
                ),
            );
        } else {
            out.insert("verdict", J::Null);
            per.insert("verdict", "미관측(인증서·스윕 verdict.json·리포트 `## 판정` 표·발행 기록 benchmark 모두 없다 — 지어내지 않는다)");
        }
    } else if !j_in(&m_get(&out, "verdict"), &MEASUREMENT_VERDICTS) {
        return core::fail(
            "HINT_MEASUREMENT_VERDICT_UNKNOWN",
            format!(
                "판정 원천의 verdict {} 가 ('PASS', 'REFUTE', 'OBSERVATION-ONLY') 밖이다({}).",
                m_get(&out, "verdict").repr(),
                m_get(&per, "verdict").display()
            ),
            Some("verdict_rule 어휘가 바뀌었으면 evidence.MEASUREMENT_VERDICTS 를 개정한다(tripwire — 모르는 판정을 통과로 접지 않는다)."),
        );
    }
    let is_obs = jeq(&m_get(&out, "verdict"), OBSERVATION_ONLY_VERDICT);
    let lite_obs = is_obs && (jeq(&m_get(&out, "benchmark_mode"), "lite") || report_lite);
    if !out.contains_key("accept_len") && lite_obs {
        let lw = lite_window(&repo, ev)?.unwrap_or(J::Null);
        let legs = &lw["legs"];
        let mut got: Vec<(&str, J)> = Vec::new();
        for leg in ["warm", "cold"] {
            if legs.get(leg).is_some() {
                let v = legs[leg]["doc"]["spec_decode_acceptance_length"].clone();
                if v.is_number() {
                    got.push((leg, v));
                }
            }
        }
        if !got.is_empty() {
            let leg = if got.iter().any(|g| g.0 == "warm") { "warm" } else { "cold" };
            let val = got.iter().find(|g| g.0 == leg).unwrap().1.clone();
            out.insert("accept_len", val.display());
            let cold = got.iter().find(|g| g.0 == "cold").map(|g| g.1.display());
            per.insert(
                "accept_len",
                format!("{} spec_decode_acceptance_length({leg} 레그 · 원문)", legs[leg]["path"].display())
                    + &(if leg == "warm" && cold.is_some() { format!(" · cold {}", cold.unwrap()) } else { String::new() })
                    + " — lite 관측(판정 입력 아님 · lite-only 셀은 verdict_rule 에 투입되지 않았다)",
            );
        } else if !unobs(&cert["lite_accept_len"]) {
            out.insert("accept_len", cert["lite_accept_len"].display());
            per.insert(
                "accept_len",
                format!("{cname} lite_accept_len(") + &or_str(&cert["lite_accept_len_src"], "출처 칸 없음")
                    + ") — lite 관측(판정 입력 아님 · lite-only 셀은 verdict_rule 에 투입되지 않았다)",
            );
        }
    }
    if !out.contains_key("accept_len") && !lite_obs {
        let sw_ = &ev["sweep"];
        let vj = &sw_["verdict"];
        let ax = if vj["rubric"].is_dict() { vj["rubric"]["spec_axis"].clone() } else { J::Null };
        if vj.is_dict() && vj.get("measured_accept_len") == Some(&J::Null) && ax.is_dict() {
            out.insert("accept_len", J::Null);
            let missing_s = plower(&ax["missing"].display());
            per.insert(
                "accept_len",
                format!(
                    "{}/verdict.json measured_accept_len = null(측정값 없음) · rubric.spec_axis: declared={} · accept_len={} · accept_len_source={} · missing={missing_s}",
                    sw_["dir"].display(),
                    ax["declared"].display(),
                    ax["accept_len"].display(),
                    ax["accept_len_source"].display()
                ) + if jeq(&ax["accept_len_source"], "declared-absent") && ax["missing"] == J::Bool(false) {
                    " — spec 미선언이라 루브릭이 1.0 을 쓴다(선언값 · 측정 아님 · 결손 아님)"
                } else {
                    ""
                },
            );
        }
    }
    for k in ["decode_tps_conc1", "floor_tps", "ratio_M_over_primary", "rubric_authority", "accept_len"] {
        if !out.contains_key(k) {
            out.insert(k, J::Null);
            per.insert(
                k,
                if lite_obs {
                    "lite-only 셀 — 동시성 1 스윕·루브릭이 없다(lite 수치는 `lite`)"
                } else if is_obs && (k == "floor_tps" || k == "ratio_M_over_primary") {
                    "기준 부재 full 셀 — 루브릭이 서지 않았다(관측 전용 · 판정 원천 NEEDS_RUBRIC)"
                } else {
                    "미관측(판정 원천에 이 칸이 없다)"
                },
            );
        }
    }
    let sw = if ev["sweep"].truthy() { ev["sweep"].clone() } else { J::dict() };
    let mut levels: Vec<J> = Vec::new();
    for lv in sw["levels"].items() {
        if !lv.is_dict() {
            continue;
        }
        let m = if lv["measured"].is_dict() { lv["measured"].clone() } else { J::dict() };
        let mut row = crate::jd! {"level" => lv["level"].clone(), "status" => lv["status"].clone()};
        for k in [
            "decode_tps", "decode_tps_mean", "output_throughput", "ttft_ms_median", "itl_ms_median", "tpot_ms_median", "accept_len",
            "completed", "failed", "max_concurrency", "measurement_ok",
        ] {
            if let Some(v) = m.get(k) {
                row.set(k, v.clone());
            }
        }
        levels.push(row);
    }
    let swdesc = format!("sweep_index({} · {})", sw["dir"].display(), sw["binding"].display());
    if !levels.is_empty() {
        out.insert("levels", J::List(levels));
        out.insert("verdict_point_level", sw["index"]["verdict_point_level"].clone());
        per.insert("levels", swdesc.clone());
        per.insert("verdict_point_level", swdesc.clone());
    }
    let lite = if sw["lite"].truthy() { sw["lite"].clone() } else { J::dict() };
    let mut lite_nums = Map::new();
    for k in ["gen_tps", "gen_src", "cold_ttft_ms", "kv_gib"] {
        if let Some(v) = lite.get(k) {
            lite_nums.insert(k, v.clone());
        }
    }
    if let Some(eng) = lite["engine"].as_dict() {
        for (k, v) in eng.iter() {
            if v.is_number() || v.is_bool() {
                lite_nums.insert(format!("engine_{k}"), v.clone());
            }
        }
    }
    let brname = pname(&or_str(&ev["bench_report_path"], ""));
    if !lite_nums.is_empty() {
        out.insert("lite", J::Dict(lite_nums));
        per.insert("lite", format!("sweep_index.lite({})", sw["dir"].display()));
    } else if report_lite {
        let brp = ev["bench_report_path"].as_str().map(str::to_string);
        let ln = _lite_report_numbers(&repo, brp.as_deref())?;
        if ln.truthy() {
            out.insert("lite", ln);
            per.insert("lite", format!("bench_report({brname}) `## lite 지표` 표 master 열(단위 벗김 · 파싱만)"));
            srcs.push(format!("bench_report({brname} · lite)"));
        }
    }
    if sw.truthy() {
        out.insert("generated_utc", sw["generated_utc"].clone());
        srcs.push(swdesc.clone());
    }
    if unobs(&m_get(&out, "measured_utc")) {
        let mu = if sw["generated_utc"].truthy() {
            Some(sw["generated_utc"].clone())
        } else {
            let brp = ev["bench_report_path"].as_str().map(str::to_string);
            _report_born_utc(&repo, brp.as_deref())?.filter(|s| !s.is_empty()).map(J::from)
        };
        if let Some(mu) = mu {
            out.insert("measured_utc", mu);
            per.insert(
                "measured_utc",
                if sw["generated_utc"].truthy() {
                    format!("sweep_index.generated_utc({})", sw["dir"].display())
                } else {
                    format!("bench_report({brname}) 머리 `생성일`")
                },
            );
        }
    }
    out.insert("source", if srcs.is_empty() { "absent(인증서·스윕 모두 없다)".to_string() } else { srcs.join(" + ") });
    let mut outj = J::Dict(out);
    let mut perj = J::Dict(per);
    _label_kv_declared(&repo, ev, &mut outj, &mut perj)?;
    outj.set("sources", perj);
    Ok(outj)
}

fn py_float_str(v: &J) -> String {
    match v {
        J::Float(f) => crate::value::J::Float(*f).repr(),
        other => other.display(),
    }
}

pub fn _kv_declared_note(v: &J, clamp: &J, rel: &str) -> HR<Option<String>> {
    let s = py_float_str(v);
    let Some(first) = s.split(py_isspace).find(|t| !t.is_empty()) else { return Ok(None) };
    let Some(got) = py_float(first) else { return Ok(None) };
    let c = clamp.as_f64().unwrap_or(0.0);
    let decl_gib = c / (1024f64 * 1024.0 * 1024.0);
    if (got - decl_gib).abs() < 0.05 + 1e-9 {
        let ci = match clamp {
            J::Int(i) => *i,
            _ => c.trunc() as i64,
        };
        return Ok(Some(format!(
            " — 서빙 yaml `{KV_CLAMP_KEY}: {ci}`({decl_gib:.1} GiB)의 **선언 클램프를 엔진이 되읽은 값**(측정 아님 · {rel})"
        )));
    }
    Ok(Some(format!(" — 서빙 yaml 선언 {decl_gib:.1} GiB 와 다르다(엔진 보고값 · {rel})")))
}

/// Label lite KV readbacks of a declared `kv-cache-memory-bytes` clamp (mutates `per`).
pub fn _label_kv_declared(repo: &Path, ev: &J, out: &mut J, per: &mut J) -> HR<()> {
    let Some(rel) = ev["triplet"]["yaml"].as_str().filter(|s| !s.is_empty()).map(str::to_string) else { return Ok(()) };
    if !repo.join(&rel).is_file() {
        return Ok(());
    }
    let y = _owner_yaml(repo, &repo.join(&rel), "서빙 yaml")?;
    let clamp = if y.is_dict() { y[KV_CLAMP_KEY].clone() } else { J::Null };
    if !clamp.is_number() || clamp.as_f64().unwrap() <= 0.0 {
        return Ok(());
    }
    if let Some(v) = out.get("lite_kv_gib").cloned() {
        if let Some(n) = _kv_declared_note(&v, &clamp, &rel)? {
            let base = or_str(&per["lite_kv_gib"], "출처 미기재");
            per.set("lite_kv_gib", base + &n);
        }
    }
    let lt = out["lite"].clone();
    for sk in ["kv_gib", "engine_kv_gib"] {
        if let Some(v) = lt.get(sk) {
            if let Some(n) = _kv_declared_note(v, &clamp, &rel)? {
                let base = or_str(&per["lite"], "출처 미기재");
                per.set(format!("lite.{sk}"), base + &n);
            }
        }
    }
    Ok(())
}
