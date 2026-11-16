//! artifacts part 1 — port of `scripts/hintlib/artifacts.py` lines 1–1532: constants, small helpers, the owner modules
//! (slave_forward / render_dockerfile boundary), the probe-only docker runner, `_Ctx`, the recipe revision selected at
//! image build time, measurement time / post-measurement regeneration, the render-3-layer env re-render, the compose slot
//! revision, the plane/build selectors and the ledger observation.
//!
//! Conventions shared with the other parts (a2/a3) — `_Ctx` is owned here:
//! - `bytes` inside a `J` value (Python dicts that carry bytes: `_select_dockerfile`/`_select_copied`/`_select_tracked`/
//!   `_history_blobs` rows) are `{"__bytes__": "<hex>"}` (the bridge encoding) — build/read them with [`_bytes_j`]/[`_j_bytes`].
//! - `c.memo` keys are the Python key object as a `J` (a tuple = `J::List`, e.g. `jl!["applied", true]`) — use
//!   [`_Ctx::memo_get`]/[`_Ctx::memo_set`]/[`_Ctx::memo_has`].
//! - the docker runner is a [`DockerRunner`] trait object; `c.docker(&[...], cache)` mirrors `_Ctx.docker`.
//! - a render module handed through a `&J` parameter (`_env_shape(render_module=...)`): `J::Null` = the worktree renderer;
//!   `{"__render_module__": <rel>, "rev": <sha>}` = the measurement-time renderer revision whose bytes equal the worktree
//!   renderer (see `_rerender_at_measurement` — the only revision this port can execute).
#![allow(non_snake_case, non_upper_case_globals, dead_code, unused_imports)]
use std::cell::{Cell, RefCell};
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::core::{self, HintError, HR};
use crate::pyre::{self, Re};
use crate::value::{Map, J};
use crate::{jd, jl, re_static};
use super::*;

pub const SLOTS: [&str; 7] = ["triplet", "runtime_patch", "build_patch_pre", "build_patch_post", "build_recipe", "compose", "fork_pin"];
pub const LEDGER_IN_IMAGE: &str = "/opt/easy-vllm/build_ledger.json";
pub const LEDGER_REQUIREMENTS_IN_IMAGE: &str = "/tmp/requirements.txt";
pub const REL_ARCH_VARIANT_LEDGER: &str = ".claude/policies/arch_variant_ledger.json";
pub const REL_SMOKE_MULTI: &str = ".claude/skills/upstream-version-watch/scripts/multinode_serve_smoke.sh";
pub const REL_SMOKE_SINGLE: &str = ".claude/skills/upstream-version-watch/scripts/single_serve_up.sh";
/// hintlib/artifacts.py:114 PATCH_DIRS (python literal as JSON)
pub fn PATCH_DIRS() -> &'static J {
    static CELL: std::sync::OnceLock<J> = std::sync::OnceLock::new();
    CELL.get_or_init(|| crate::value::loads("{\"pre\": \"build_patches_src\", \"post\": \"build_patches\"}").expect("constant"))
}
/// `KNOWN_NON_PATCH` (frozenset · membership only).
pub const KNOWN_NON_PATCH: [&str; 2] = [".gitkeep", "PROVENANCE.json"];
pub const DOCKER_TIMEOUT_S: i64 = 60;

re_static!(_PATCH_NAME, r"^\d+-.*\.sh$");
re_static!(_MODEL_TRIGGER, r"^\s*#\s*model-trigger\s*:\s*(.+?)\s*$", pyre::M | pyre::I);
re_static!(_GATE_DEFAULT, r#"^\s*:\s*"?\$\{([A-Z][A-Z0-9_]*):=([^}]*)\}"?\s*$"#, pyre::M);
re_static!(_GATE_SKIP_IF, r#"^\s*if\s+\[\s*"?\$\{?([A-Z][A-Z0-9_]*)\}?"?\s*!=\s*"?1"?\s*\]\s*;\s*then\s*$"#);
/// `_PRE_GATE_OK` (tuple of compiled patterns, in order).
pub fn _PRE_GATE_OK() -> &'static [Re] {
    static CELL: std::sync::OnceLock<Vec<Re>> = std::sync::OnceLock::new();
    CELL.get_or_init(|| {
        vec![
            Re::new(r"^\s*set\s+-[A-Za-z]+(?:\s+[a-z]+)*\s*$", 0),
            Re::new(r#"^\s*[A-Za-z_][A-Za-z0-9_]*=(?:"[^"`]*"|'[^']*'|[^\s`;|&]*)\s*$"#, 0),
            Re::new(r#"^\s*:\s*"?\$\{[A-Za-z_][A-Za-z0-9_]*:=[^}]*\}"?\s*$"#, 0),
            Re::new(r"^\s*if\s+\[.*\]\s*;\s*then\s*$", 0),
            Re::new(r"^\s*fi\s*$", 0),
            Re::new(r"^\s*echo\s.*$", 0),
        ]
    })
}
re_static!(_HISTORY_RUN_ARGS, r"^RUN \|(\d+) (.*)$");
re_static!(_DOCKERFILE_COPY, r"^\s*(COPY|ADD)\s+(.+?)\s*$", pyre::M | pyre::I);
re_static!(_CONFIGS_REF, r"/app/configs/([A-Za-z0-9_.${}:-]+\.sh)");
re_static!(_SOURCED, r#"(?:^\s*|[;&|]\s*|\bthen\s+)(?:source|\.)\s+"?/app/configs/([A-Za-z0-9_.-]+\.sh)"#, pyre::M);
re_static!(_ENV_FILE_REF, r"(?<![\w/.-])envs/(\.env\.[A-Za-z0-9_.-]+)");
re_static!(_RUNNER_GPU_EQ, r#""\$\{GPU_COUNT\}"\s*=\s*"(\d+)""#);
re_static!(_RUNNER_GPU_RAY, r"(\d+)\.0/(\d+)\.0 GPU");
re_static!(_LOCAL_DATE, r"^(\d{4})(\d{2})(\d{2})-(\d{2})(\d{2})(\d{2})$");
pub const PROBE_ATTR: &str = "image_probe";
pub const COMMENT_ONLY_REQUIREMENT_NOTE: &str = "yaml 주석만 있음 — 관측된 실패가 있으면 declared-requirement";
pub const COMMENT_ONLY_TUNED_NOTE: &str = "yaml 주석만 있음(스윕 기록 없음) — 이 셀 계보에서 값을 바꿔 잰 기록이 있으면 tuned";
pub const POST_MEASUREMENT_REGENERATED: &str = "post-measurement-regenerated";
pub const ENV_OBSERVED_POINTER: &str = "01 §1.4 '측정 실행의 env 관측' 표 · PAYLOAD.json measurement_env_observed";
pub const LAYER_WINDOW: &str = "layer-window";
pub const CLOCK_MIXED: &str = "clock-mixed";
pub const HOST_START_MARK: &str = "호스트 date -u";
pub const RUNTIME_PATCH_ARMER: &str = "arm_patch.sh";
pub const REGENERATED_VALUE: &str = "<측정 후 재생성 — 측정 당시 값 미관측>";
re_static!(_PLACEHOLDER_VALUE, r"^<[^<>\s][^<>]*>$");
re_static!(_HEREDOC, r#"(?<!<)<<(-?)\s*(['"]?)([A-Za-z_][A-Za-z0-9_]*)\2"#);
re_static!(_RUN_FLAG, r"^--(?:mount|network|security)=\S+\s+");
re_static!(_SHELL_C, r"^(?:\S*/)?(?:ba|da)?sh\s+-c\s+");
re_static!(_SH_EXIT, r#"(?:^|[;&|{(\s'"])exit(?:\s+([^\s;&|)}#'"]+))?\s*(?=$|[;&|)}#'"])"#);
re_static!(_SH_SWALLOW, r"\|\|\s*(?:true|:)\s*(?=$|[;&)}#])");
/// `_SWALLOW_OK_CMDS` (frozenset · membership only).
pub const _SWALLOW_OK_CMDS: [&str; 4] = ["find", "rm", "rmdir", "unlink"];
re_static!(_SH_TRAP, r"(?:^|[;&|{(\s])trap\s");
re_static!(_PY_EXIT0, r"\b(?:sys\.exit|os\._exit|quit|exit)\(\s*(?:0\s*)?\)|\braise\s+SystemExit\s*(?:\(\s*(?:0\s*)?\))?\s*(?:$|;)");
re_static!(_SET_E, r"^\s*set\s+(?:-[A-Za-z]*e[A-Za-z]*\b|-o\s+errexit\b)");
re_static!(_SET_NO_E, r"^\s*set\s+(?:\+[A-Za-z]*e[A-Za-z]*\b|\+o\s+errexit\b)");
re_static!(_SKIP_WORD, r"(?i)\bskip");
re_static!(_SH_ASSIGN, r"^\s*(?:export\s+)?([A-Za-z_][A-Za-z0-9_]*)=(.*)$");
re_static!(_SH_VAR, r"\$\{([A-Za-z_][A-Za-z0-9_]*)\}|\$([A-Za-z_][A-Za-z0-9_]*)");
re_static!(_GREP_Q, r#"\bgrep\s+((?:-[A-Za-z]+\s+)+)(?:--\s+)?(?:'([^']*)'|"([^"$`\\]*)")\s+("[^"]*"|'[^']*'|[^\s;|&)]+)"#);
re_static!(_ASSERT_IN, r#"\bassert\s+(['"])(.+?)\1\s+in\s+[A-Za-z_]"#);
re_static!(_MODULE_AS, r"\bimport\s+([A-Za-z_][\w.]*)\s+as\s+([A-Za-z_]\w*)");
/// `_BRE_SPECIAL = set(".[]*^$\\")` (membership only).
pub const _BRE_SPECIAL: [char; 7] = ['.', '[', ']', '*', '^', '$', '\\'];
pub const _BENCH_JSON_KEYS: [&str; 4] = ["backend", "num_prompts", "max_concurrency", "date"];
pub const IDENTITY_KEY_AXIS: [&str; 14] = ["_HOST_IP", "_HOSTNAME", "_HOST", "_USER", "_IP", "_ADDR", "_ADDRESS", "_SSH", "_KEY", "_TOKEN", "_SECRET", "_PASSWORD", "_ACCOUNT", "_MAIL"];
/// `TUNING_KEYS` (frozenset · membership only).
pub const TUNING_KEYS: [&str; 13] = [
    "RAY_PORT", "MAX_JOBS", "PYTORCH_CUDA_ALLOC_CONF", "RAY_OBJECT_STORE_MEMORY", "VLLM_VERSION", "IMAGE_TAG", "TENSOR_PARALLEL_SIZE",
    "GPU_MEMORY_UTILIZATION", "MAX_MODEL_LEN", "MAX_NUM_SEQS", "KV_CACHE_DTYPE", "MOE_BACKEND", "ATTENTION_BACKEND",
];
pub const ENV_TIERS: [&str; 4] = ["env", "preset", "invariant", "unknown"];
/// hintlib/artifacts.py:221 _TIER_ENV_KINDS (python literal as JSON)
pub fn _TIER_ENV_KINDS() -> &'static J {
    static CELL: std::sync::OnceLock<J> = std::sync::OnceLock::new();
    CELL.get_or_init(|| crate::value::loads("{\".env.cluster\": \"cluster\", \".env.interconnect\": \"interconnect\"}").expect("constant"))
}
/// hintlib/artifacts.py:226 LOCKSET_SOURCE_KNOB (python literal as JSON)
pub fn LOCKSET_SOURCE_KNOB() -> &'static J {
    static CELL: std::sync::OnceLock<J> = std::sync::OnceLock::new();
    CELL.get_or_init(|| crate::value::loads("{\"batch_source\": \"max-num-seqs\", \"gmu_source\": \"gpu-memory-utilization\", \"kv_source\": \"kv-cache-memory-bytes\"}").expect("constant"))
}
/// hintlib/artifacts.py:229 _SOURCE_STATUS (python literal as JSON)
pub fn _SOURCE_STATUS() -> &'static J {
    static CELL: std::sync::OnceLock<J> = std::sync::OnceLock::new();
    CELL.get_or_init(|| crate::value::loads("{\"measured-clamp\": \"tuned\", \"kv-fit-measured\": \"tuned\", \"declared-requirement\": \"declared-requirement\", \"target_gmu\": \"declared-requirement\", \"hand\": \"inherited\", \"hand-lever\": \"inherited\"}").expect("constant"))
}
/// `VALUE_STATUS_EXCLUDED_KNOBS` (frozenset · membership only).
pub const VALUE_STATUS_EXCLUDED_KNOBS: [&str; 4] = ["host", "model", "port", "served-model-name"];
/// hintlib/artifacts.py:239 _COMMENT_STATUS (python literal as JSON)
pub fn _COMMENT_STATUS() -> &'static J {
    static CELL: std::sync::OnceLock<J> = std::sync::OnceLock::new();
    CELL.get_or_init(|| crate::value::loads("[[\"음성대조\", \"negative-control\"], [\"negative\", \"negative-control\"], [\"승계\", \"inherited\"], [\"필수\", \"declared-requirement\"], [\"요구\", \"declared-requirement\"], [\"수렴\", \"tuned\"], [\"측정\", \"tuned\"]]").expect("constant"))
}

// ── private Python-text helpers ──────────────────────────────────────────────────────────────
fn is_py_ws(c: char) -> bool {
    c.is_whitespace() || ('\x1c'..='\x1f').contains(&c)
}
fn py_strip(s: &str) -> &str {
    s.trim_matches(is_py_ws)
}
fn py_lstrip(s: &str) -> &str {
    s.trim_start_matches(is_py_ws)
}
fn py_rstrip(s: &str) -> &str {
    s.trim_end_matches(is_py_ws)
}
/// `str.split()` (no argument).
fn py_split(s: &str) -> Vec<&str> {
    s.split(is_py_ws).filter(|x| !x.is_empty()).collect()
}
/// `str.splitlines()`.
fn splitlines(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut it = text.char_indices().peekable();
    while let Some((i, c)) = it.next() {
        if matches!(c, '\n' | '\r' | '\x0b' | '\x0c' | '\x1c' | '\x1d' | '\x1e' | '\u{85}' | '\u{2028}' | '\u{2029}') {
            out.push(&text[start..i]);
            let mut next = i + c.len_utf8();
            if c == '\r' {
                if let Some(&(j, '\n')) = it.peek() {
                    it.next();
                    next = j + 1;
                }
            }
            start = next;
        }
    }
    if start < text.len() {
        out.push(&text[start..]);
    }
    out
}
fn head(s: &str, n: usize) -> String {
    core::head_chars(s, n)
}
fn lossy(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}
fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}
fn git_blob_sha(data: &[u8]) -> String {
    let mut buf = format!("blob {}\0", data.len()).into_bytes();
    buf.extend_from_slice(data);
    hex(ring::digest::digest(&ring::digest::SHA1_FOR_LEGACY_USE_ONLY, &buf).as_ref())
}
fn is_sha40(s: &str) -> bool {
    s.len() == 40 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn utc_of_epoch(t: i64) -> String {
    format!("{}Z", core::Civil::from_epoch(t).iso())
}
fn opt_disp(v: Option<&str>) -> String {
    v.map(str::to_string).unwrap_or_else(|| "None".to_string())
}
fn read_bytes(p: &Path) -> HR<Vec<u8>> {
    std::fs::read(p).map_err(|e| core::io_err(&e, p))
}
fn file_mode(p: &Path) -> HR<i64> {
    use std::os::unix::fs::PermissionsExt;
    let m = std::fs::metadata(p).map_err(|e| core::io_err(&e, p))?;
    Ok((m.permissions().mode() & 0o777) as i64)
}
fn num(v: &J) -> f64 {
    match v {
        J::Bool(b) => *b as i64 as f64,
        _ => v.as_f64().unwrap_or(0.0),
    }
}
fn dict_of(v: &J) -> Option<&Map> {
    v.as_dict()
}
fn set_all(m: &mut J, kv: Vec<(&str, J)>) {
    for (k, v) in kv {
        m.set(k, v);
    }
}
/// `PurePath(s).name`.
fn purepath_name(s: &str) -> String {
    s.split('/').filter(|x| !x.is_empty() && *x != ".").last().unwrap_or("").to_string()
}

/// `bytes` inside a `J` value: `{"__bytes__": "<hex>"}` (the bridge encoding).
pub fn _bytes_j(b: &[u8]) -> J {
    jd! {"__bytes__" => hex(b)}
}

/// The bytes of a `{"__bytes__": hex}` value (a plain string = its UTF-8 bytes); anything else = `None`.
pub fn _j_bytes(v: &J) -> Option<Vec<u8>> {
    match v {
        J::Str(s) => Some(s.as_bytes().to_vec()),
        J::Dict(m) => {
            let h = m.get("__bytes__")?.as_str()?;
            let hb = h.as_bytes();
            if hb.len() % 2 != 0 {
                return None;
            }
            let mut out = Vec::with_capacity(hb.len() / 2);
            for ch in hb.chunks(2) {
                out.push(u8::from_str_radix(std::str::from_utf8(ch).ok()?, 16).ok()?);
            }
            Some(out)
        }
        _ => None,
    }
}

/// `isinstance(ev, evidence.CellEvidence)` for a `J` evidence: the bridge form (`__class__`) or the dataclass field set.
pub fn a1_is_cell_evidence(ev: &J) -> bool {
    is_cell_evidence(ev)
}

fn is_cell_evidence(ev: &J) -> bool {
    let Some(m) = ev.as_dict() else { return false };
    if let Some(cls) = m.get("__class__").and_then(J::as_str) {
        return cls == "CellEvidence";
    }
    ["mode", "campaign_id", "cell", "node", "topology", "triplet", "build_identity", "lineage_seeds", "pointers", "missing", "phases"]
        .iter()
        .all(|k| m.contains_key(k))
}

// ── 작은 도우미 ──────────────────────────────────────────────────────────────────────────────
/// `_get(obj, name, default)`: dict → `.get` · other objects → `getattr` (a `J` non-dict has no attributes).
pub fn _get(obj: &J, name: &str, default: &J) -> HR<J> {
    Ok(match obj {
        J::Dict(m) => m.get(name).cloned().unwrap_or_else(|| default.clone()),
        _ => default.clone(),
    })
}
fn g(obj: &J, name: &str) -> J {
    _get(obj, name, &J::Null).unwrap_or(J::Null)
}

pub fn _read(path: &Path) -> HR<String> {
    match std::fs::read(path) {
        Ok(b) => Ok(lossy(&b)),
        Err(e) => core::fail(
            "HINT_ARTIFACT_READ_FAILED",
            format!("아티팩트 읽기 실패({}): {}", purepath_name(&core::posix(path)), core::io_err_text(&e, path)),
            None,
        ),
    }
}

pub fn _json_file(path: &Path) -> HR<Option<J>> {
    if !path.is_file() {
        return Ok(None);
    }
    let Ok(text) = core::read_text(path) else { return Ok(None) };
    match crate::value::loads(&text) {
        Ok(v) if v.is_dict() => Ok(Some(v)),
        _ => Ok(None),
    }
}

pub fn _topology(ev: &J) -> HR<String> {
    let decl = g(ev, "declaration");
    let control = decl.get("control_variables").cloned().unwrap_or(J::Null);
    let ident = g(ev, "identity");
    for cand in [g(ev, "topology"), control.get("topology").cloned().unwrap_or(J::Null), ident.get("topology").cloned().unwrap_or(J::Null)] {
        if let Some(s) = cand.as_str() {
            if s == "single" || s == "multi" {
                return Ok(s.to_string());
            }
        }
    }
    core::fail(
        "HINT_TOPOLOGY_UNDERIVABLE",
        "셀 증거에서 single|multi 토폴로지를 해소하지 못했다.",
        Some("evidence 가 manifest/캠페인 선언에서 topology 를 채워 넘긴다(브랜치로 추론 ✗)."),
    )
}

pub fn _cell(ev: &J) -> HR<String> {
    let a = g(ev, "cell");
    let v = if a.truthy() { a } else { g(ev, "cell_id") };
    let cell = if v.truthy() { v.display() } else { String::new() };
    if cell.is_empty() || cell.contains('/') || cell == "." || cell == ".." || cell.starts_with('.') {
        return core::fail("HINT_CELL_INVALID", format!("셀 id 가 비었거나 경로를 포함한다: {}", crate::value::py_repr(&cell)), None);
    }
    Ok(cell)
}

re_static!(utc_z_re, r"^(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2})(?:\.\d+)?(Z|[+-]\d{2}:\d{2})$");

/// `2026-09-09T23:24:35.288+09:00` · `…Z` → `YYYY-MM-DDTHH:MM:SSZ`. 해석 불가 = None.
pub fn _utc_z(raw: Option<&str>) -> HR<Option<String>> {
    let Some(raw) = raw.filter(|r| !r.is_empty()) else { return Ok(None) };
    let s = py_strip(raw);
    let Some(m) = utc_z_re().match_at_start(s) else { return Ok(None) };
    let base = pyre::group(&m, 1).unwrap_or("");
    let tz = pyre::group(&m, 2).unwrap_or("");
    if !base.is_ascii() || !tz.is_ascii() {
        return Ok(None); // fromisoformat accepts ASCII digits only
    }
    let n = |r: std::ops::Range<usize>| -> i64 { base[r].parse().unwrap_or(-1) };
    let (y, mo, d, h, mi, se) = (n(0..4), n(5..7), n(8..10), n(11..13), n(14..16), n(17..19));
    let dim = |y: i64, m: i64| -> i64 {
        match m {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
            2 => 28,
            _ => 0,
        }
    };
    if y < 1 || !(1..=12).contains(&mo) || d < 1 || d > dim(y, mo) || h > 23 || mi > 59 || se > 59 {
        return Ok(None);
    }
    let off = if tz == "Z" {
        0
    } else {
        let sign = if tz.starts_with('-') { -1 } else { 1 };
        let oh: i64 = tz[1..3].parse().unwrap_or(99);
        let om: i64 = tz[4..6].parse().unwrap_or(99);
        if oh > 23 || om > 59 {
            return Ok(None);
        }
        sign * (oh * 3600 + om * 60)
    };
    let c = core::Civil { year: y, month: mo as u32, day: d as u32, hour: h as u32, minute: mi as u32, second: se as u32 };
    let t = c.epoch() - off;
    let u = core::Civil::from_epoch(t);
    if u.year < 1 || u.year > 9999 {
        return Err(HintError::py("OverflowError", "date value out of range"));
    }
    Ok(Some(format!("{}Z", u.iso())))
}

pub fn _secs_between(a: Option<&str>, b: Option<&str>) -> HR<Option<i64>> {
    let (Some(a), Some(b)) = (a.filter(|x| !x.is_empty()), b.filter(|x| !x.is_empty())) else { return Ok(None) };
    let pb = match core::parse_utc(b) {
        Ok(v) => v,
        Err(e) if !e.is_py() => return Ok(None),
        Err(e) => return Err(e),
    };
    let pa = match core::parse_utc(a) {
        Ok(v) => v,
        Err(e) if !e.is_py() => return Ok(None),
        Err(e) => return Err(e),
    };
    Ok(Some(pb.epoch() - pa.epoch()))
}

pub fn _fmt_duration(secs: Option<i64>, bound: &str) -> HR<String> {
    let Some(secs) = secs else { return Ok("미관측".to_string()) };
    let a = secs.abs();
    let (mm, ss) = (a / 60, a % 60);
    let txt = if mm >= 60 { format!("{}시간 {}분 {}초", mm / 60, mm % 60, ss) } else { format!("{mm}분 {ss}초") };
    Ok(match bound {
        "exact" => txt,
        "upper" => format!("≤ {txt}(상한)"),
        CLOCK_MIXED => format!("≈ {txt}(두 끝 시계 다름 — 컨테이너 date · 호스트 UTC · 근사)"),
        LAYER_WINDOW => format!("층 창 {txt}(docker history 첫 층 → 끝 층 CreatedAt · 캐시 재사용 층 포함 — 빌드 소요 아님)"),
        _ => txt,
    })
}

// ── 교차 스킬 소유 모듈 ──────────────────────────────────────────────────────────────────────
/// `core.load_owner_module`'s presence check (the module itself is not executed — its rules are mirrored below).
fn owner_module_path(repo: &Path, rel_path: &str) -> HR<PathBuf> {
    let path = core::resolve(&repo.join(rel_path));
    if !path.is_file() {
        return core::fail(
            "HINT_OWNER_MODULE_MISSING",
            format!("소유 모듈 부재: {rel_path}"),
            Some("대체 구현으로 계속하지 않는다 — 그 스킬이 설치된 체크아웃에서 실행한다."),
        );
    }
    Ok(path)
}

/// upstream `slave_forward.py` (the module the Python loaded in-process). `extra` = test-injection values (unused by a1).
#[derive(Clone, Debug)]
pub struct _ForwardModule {
    pub path: PathBuf,
    pub extra: J,
}

fn forward_error(code: &str, message: String) -> HintError {
    HintError::py("ForwardError", format!("{code}: {message}"))
}

impl _ForwardModule {
    /// OWNER-MIRROR: .claude/skills/upstream-version-watch/scripts/slave_forward.py::read_env_first — first match of
    /// `KEY=value` per key (fullmatch `([A-Za-z_][A-Za-z0-9_]*)=(.*)` per line), raw value; an absent file = `{}`.
    pub fn read_env_first(&self, path: Option<&Path>) -> HR<J> {
        let mut out = Map::new();
        let Some(p) = path else { return Ok(J::Dict(out)) };
        if !p.is_file() {
            return Ok(J::Dict(out));
        }
        let text = lossy(&read_bytes(p)?);
        for raw in splitlines(&text) {
            let mut chars = raw.char_indices();
            match chars.next() {
                Some((_, c)) if c.is_ascii_alphabetic() || c == '_' => {}
                _ => continue,
            }
            let end = raw.char_indices().find(|&(_, c)| !(c.is_ascii_alphanumeric() || c == '_')).map_or(raw.len(), |(i, _)| i);
            if raw[end..].starts_with('=') && !raw[end + 1..].contains('\n') && !out.contains_key(&raw[..end]) {
                out.insert(raw[..end].to_string(), J::Str(raw[end + 1..].to_string()));
            }
        }
        Ok(J::Dict(out))
    }

    /// OWNER-MIRROR: .claude/skills/upstream-version-watch/scripts/slave_forward.py::compose_default — the default of the
    /// first `${KEY:-default}` / `${KEY-default}` outside comments (balanced braces on one line). `ForwardError` is
    /// raised as the Python exception type `ForwardError` (`str(e)` = `"<code>: <message>"`).
    pub fn compose_default(&self, compose_path: &Path, key: &str) -> HR<Option<String>> {
        if !compose_path.is_file() {
            return Err(forward_error("SF_COMPOSE_ABSENT", format!("compose 파일이 없다: {}", core::posix(compose_path))));
        }
        let text = core::read_text(compose_path).map_err(|e| HintError::py("UnicodeDecodeError", e))?;
        let k: Vec<char> = key.chars().collect();
        for raw in splitlines(&text) {
            let line: Vec<char> = sf_strip_comment(raw).chars().collect();
            let mut start = None;
            for i in 0..line.len() {
                if line[i] != '$' || (i > 0 && line[i - 1] == '$') || line.get(i + 1) != Some(&'{') {
                    continue;
                }
                let j = i + 2;
                if line.len() < j + k.len() || line[j..j + k.len()] != k[..] {
                    continue;
                }
                let mut e = j + k.len();
                if line.get(e) == Some(&':') {
                    e += 1;
                }
                if line.get(e) == Some(&'-') {
                    start = Some(e + 1);
                    break;
                }
            }
            let Some(i) = start else { continue };
            let mut depth = 1;
            for jx in i..line.len() {
                if line[jx] == '{' {
                    depth += 1;
                } else if line[jx] == '}' {
                    depth -= 1;
                    if depth == 0 {
                        return Ok(Some(line[i..jx].iter().collect()));
                    }
                }
            }
            return Err(forward_error(
                "SF_COMPOSE_SHAPE",
                format!("${{{key}:-…}} 의 괄호가 한 줄 안에서 닫히지 않는다: {}", crate::value::py_repr(py_strip(raw))),
            ));
        }
        Ok(None)
    }
}

/// OWNER-MIRROR: slave_forward.py::_strip_comment.
fn sf_strip_comment(line: &str) -> String {
    let (mut out, mut quote, mut prev) = (String::new(), None::<char>, ' ');
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
    py_rstrip(&out).to_string()
}

/// upstream `render_dockerfile.py` (in-process module of the Python). `rev` = a measurement-time revision (whose bytes equal
/// the worktree renderer — the only revision this port runs) · `extra` = test-injection values.
#[derive(Clone, Debug)]
pub struct _RenderModule {
    pub path: PathBuf,
    pub SHARED_ASSET_DIR: String,
    pub rev: Option<String>,
    pub extra: J,
}

re_static!(ledger_arg_re, r"^ARG\s+([A-Za-z_][A-Za-z0-9_]*)", pyre::M);

impl _RenderModule {
    /// OWNER-MIRROR: .claude/skills/upstream-version-watch/scripts/render_dockerfile.py::ledger_build_args — `^ARG <NAME>`
    /// names (declaration order · de-duplicated).
    pub fn ledger_build_args(&self, text: &str) -> HR<Vec<String>> {
        let mut out: Vec<String> = Vec::new();
        for n in ledger_arg_re().findall(text) {
            if !out.contains(&n) {
                out.push(n);
            }
        }
        Ok(out)
    }
}

/// `_forward_module(repo, module)`: the injected module, else the repository's `slave_forward.sh` launcher (absent = HINT_OWNER_MODULE_MISSING · never a `.py`).
pub fn _forward_module(repo: &Path, module: Option<&_ForwardModule>) -> HR<_ForwardModule> {
    if let Some(m) = module {
        return Ok(m.clone());
    }
    let path = owner_module_path(repo, core::REL_SLAVE_FORWARD)?;
    Ok(_ForwardModule { path, extra: J::Null })
}

/// `_render_module(repo, module)`: the injected module, else the repository's `render_dockerfile.sh` launcher (never a `.py`).
pub fn _render_module(repo: &Path, module: Option<&_RenderModule>) -> HR<_RenderModule> {
    if let Some(m) = module {
        return Ok(m.clone());
    }
    let path = owner_module_path(repo, core::REL_RENDER_DOCKERFILE)?;
    // SKILL_ROOT = abspath(dirname(__file__)/..) · SHARED_ASSET_DIR = SKILL_ROOT/assets/configs
    let skill_root = path.parent().and_then(Path::parent).map(Path::to_path_buf).unwrap_or_default();
    let shared = core::posix(&skill_root.join("assets").join("configs"));
    Ok(_RenderModule { path, SHARED_ASSET_DIR: shared, rev: None, extra: J::Null })
}

/// `_read_env(sf, path)`: slave_forward `read_env_first` values stripped.
pub fn _read_env(sf: &_ForwardModule, path: Option<&Path>) -> HR<J> {
    let Some(p) = path.filter(|p| p.is_file()) else { return Ok(J::dict()) };
    let env = sf.read_env_first(Some(p))?;
    let mut out = Map::new();
    for (k, v) in env.as_dict().map(|m| m.iter().collect::<Vec<_>>()).unwrap_or_default() {
        out.insert(k, J::Str(py_strip(&v.display()).to_string()));
    }
    Ok(J::Dict(out))
}

// ── docker 실행기 ────────────────────────────────────────────────────────────────────────────
/// Why a docker call produced no process (Python: FileNotFoundError/OSError → "docker-unavailable", TimeoutExpired → "timeout").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunFail {
    Unavailable,
    Timeout,
}

/// `DockerRunner = Callable[..., CompletedProcess]` plus its declared `image_probe` capability (`PROBE_ATTR`).
/// Any closure `Fn(&[String], u64) -> Result<core::Proc, RunFail>` is a runner that does not declare `image_probe`
/// (= Python's injected runner without the attribute → read-only).
pub trait DockerRunner {
    fn run(&self, argv: &[String], timeout: u64) -> Result<core::Proc, RunFail>;
    /// `getattr(runner, PROBE_ATTR, False) is True`.
    fn image_probe(&self) -> bool {
        false
    }
}

impl<F: Fn(&[String], u64) -> Result<core::Proc, RunFail>> DockerRunner for F {
    fn run(&self, argv: &[String], timeout: u64) -> Result<core::Proc, RunFail> {
        self(argv, timeout)
    }
}

/// `repr(CompletedProcess(argv, rc, stdout, stderr))`.
pub fn _proc_repr(argv: &[String], p: &core::Proc) -> String {
    format!(
        "CompletedProcess(args={}, returncode={}, stdout={}, stderr={})",
        J::from(argv.to_vec()).repr(),
        p.returncode,
        crate::value::py_repr(&p.stdout),
        crate::value::py_repr(&p.stderr)
    )
}

/// The injected real call of `_probe_only_runner(run=...)` (default = subprocess).
pub type RealRun = Rc<dyn Fn(&[String], u64) -> Result<core::Proc, RunFail>>;

/// `subprocess.run(argv, capture_output=True, text=True, timeout=timeout)`.
fn subprocess_run(argv: &[String], timeout: u64) -> Result<core::Proc, RunFail> {
    use std::io::Read;
    use std::process::{Command, Stdio};
    if argv.is_empty() {
        return Err(RunFail::Unavailable);
    }
    let mut child = Command::new(&argv[0])
        .args(&argv[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| RunFail::Unavailable)?;
    let mut so = child.stdout.take().unwrap();
    let mut se = child.stderr.take().unwrap();
    let ho = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = so.read_to_end(&mut b);
        b
    });
    let he = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = se.read_to_end(&mut b);
        b
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout);
    let status = loop {
        match child.try_wait() {
            Ok(Some(st)) => break st,
            Ok(None) => {
                if std::time::Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = ho.join();
                    let _ = he.join();
                    return Err(RunFail::Timeout);
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            Err(_) => return Err(RunFail::Unavailable),
        }
    };
    let stdout = ho.join().unwrap_or_default();
    let stderr = he.join().unwrap_or_default();
    use std::os::unix::process::ExitStatusExt;
    let rc = status.code().unwrap_or_else(|| -status.signal().unwrap_or(0));
    Ok(core::Proc { returncode: rc, stdout: lossy(&stdout), stderr: lossy(&stderr) })
}

/// `_probe_only_runner()`'s runner: `image inspect` · `history` · `create`(`--pull never` + `--network none`) · `cp`/`rm`
/// of containers it made · everything else refused with rc 125 (never called). Declares `image_probe`.
pub struct _ProbeOnlyRunner {
    made: RefCell<BTreeSet<String>>,
    real: Option<RealRun>,
}

re_static!(cid_re, r"[0-9a-f]{12,64}");

impl _ProbeOnlyRunner {
    fn refuse(_argv: &[String], why: &str) -> core::Proc {
        core::Proc { returncode: 125, stdout: String::new(), stderr: format!("hintlib.artifacts 기본 실행기(탐침 전용): {why}") }
    }
    fn call(&self, argv: &[String], timeout: u64) -> Result<core::Proc, RunFail> {
        match &self.real {
            Some(f) => f(argv, timeout),
            None => subprocess_run(argv, timeout),
        }
    }
}

impl DockerRunner for _ProbeOnlyRunner {
    fn run(&self, argv: &[String], timeout: u64) -> Result<core::Proc, RunFail> {
        let a: Vec<String> = argv.iter().skip(1).cloned().collect();
        let a0 = a.first().map(String::as_str);
        if (a.len() >= 2 && a[0] == "image" && a[1] == "inspect") || a0 == Some("history") {
            return self.call(argv, timeout);
        }
        if a0 == Some("create") {
            if _flag_value(&a, "--pull").ok().flatten().as_deref() != Some("never")
                || _flag_value(&a, "--network").ok().flatten().as_deref() != Some("none")
            {
                return Ok(Self::refuse(argv, "create 는 `--pull never --network none` 으로만(이미지를 받지 않고 네트워크 없이 · 시작 ✗)"));
            }
            let r = self.call(argv, timeout)?;
            let stripped = py_strip(&r.stdout).to_string();
            let lines = splitlines(&stripped);
            if r.returncode == 0 {
                if let Some(last) = lines.last() {
                    let id = py_strip(last);
                    if cid_re().is_full(id) {
                        self.made.borrow_mut().insert(id.to_string());
                    }
                }
            }
            return Ok(r);
        }
        if a0 == Some("cp") {
            let srcs: Vec<&String> = a[1..].iter().filter(|x| !x.starts_with('-')).collect();
            let cid = match srcs.first() {
                Some(s) if s.contains(':') => s.split(':').next().unwrap_or("").to_string(),
                _ => String::new(),
            };
            if !self.made.borrow().contains(&cid) {
                return Ok(Self::refuse(argv, "cp 는 이 실행기가 만든(시작하지 않은) 컨테이너에서 꺼내는 것만"));
            }
            return self.call(argv, timeout);
        }
        if a0 == Some("rm") {
            let ids: Vec<String> = a[1..].iter().filter(|x| !x.starts_with('-')).cloned().collect();
            if ids.is_empty() || ids.iter().any(|x| !self.made.borrow().contains(x)) {
                return Ok(Self::refuse(argv, "rm 은 이 실행기가 만든 컨테이너만"));
            }
            let r = self.call(argv, timeout)?;
            if r.returncode == 0 {
                let mut m = self.made.borrow_mut();
                for x in &ids {
                    m.remove(x);
                }
            }
            return Ok(r);
        }
        let first: Vec<&str> = a.iter().take(1).map(String::as_str).collect();
        Ok(Self::refuse(argv, &format!("`docker {}` 거부(컨테이너를 시작하거나 이미지·상태를 바꾸는 호출 ✗)", first.join(" "))))
    }
    fn image_probe(&self) -> bool {
        true
    }
}

/// `_probe_only_runner(run=None)` — the probe-only default runner (`run` = injected subprocess for checks).
pub fn _probe_only_runner(run: Option<RealRun>) -> HR<Rc<_ProbeOnlyRunner>> {
    Ok(Rc::new(_ProbeOnlyRunner { made: RefCell::new(BTreeSet::new()), real: run }))
}

/// OWNER-MIRROR: scripts/hint.py::_docker_read_only — `image inspect`·`history` real, everything else rc 125. No `image_probe`.
pub struct _DockerReadOnly {
    pub real: Option<RealRun>,
}

impl DockerRunner for _DockerReadOnly {
    fn run(&self, argv: &[String], timeout: u64) -> Result<core::Proc, RunFail> {
        let a: Vec<&String> = argv.iter().skip(1).collect();
        if (a.len() >= 2 && a[0] == "image" && a[1] == "inspect") || a.first().map(|s| s.as_str()) == Some("history") {
            return match &self.real {
                Some(f) => f(argv, timeout),
                None => subprocess_run(argv, timeout),
            };
        }
        Ok(core::Proc { returncode: 125, stdout: String::new(), stderr: "hint.py --docker-read-only: 이미지 실행(원장 탐침) 거부".into() })
    }
}

/// A runner handed through a `&J` parameter: `J::Null` = default (`None`); `{"__docker_runner__": "probe-only"|"read-only"}`
/// = the probe-only runner (as an injected runner) / hint.py's read-only runner. Anything else = TypeError.
pub fn _runner_from_j(runner: &J) -> HR<Option<Rc<dyn DockerRunner>>> {
    match runner {
        J::Null => Ok(None),
        J::Dict(m) => match m.get("__docker_runner__").and_then(J::as_str) {
            Some("probe-only") => Ok(Some(_probe_only_runner(None)? as Rc<dyn DockerRunner>)),
            Some("read-only") => Ok(Some(Rc::new(_DockerReadOnly { real: None }))),
            _ => Err(HintError::py("TypeError", "runner: expected a DockerRunner")),
        },
        _ => Err(HintError::py("TypeError", "runner: expected a DockerRunner")),
    }
}

pub fn _flag_value(args: &[String], flag: &str) -> HR<Option<String>> {
    let pre = format!("{flag}=");
    for (i, tok) in args.iter().enumerate() {
        if tok == flag && i + 1 < args.len() {
            return Ok(Some(args[i + 1].clone()));
        }
        if tok.starts_with(&pre) {
            return Ok(Some(tok.split_once('=').map(|x| x.1).unwrap_or("").to_string()));
        }
    }
    Ok(None)
}

// ── 문맥 ─────────────────────────────────────────────────────────────────────────────────────
/// `_Ctx` — one cell's observation context. Mutable members (runner, rd, compose_eff, the docker cache, memo) sit behind
/// `RefCell`/`Cell` so every function takes `&_Ctx` like the Python takes `c`.
pub struct _Ctx {
    pub repo: PathBuf,
    pub ev: J,
    pub runner: RefCell<Option<Rc<dyn DockerRunner>>>,
    pub sf: Option<_ForwardModule>,
    pub rd: RefCell<Option<_RenderModule>>,
    pub rd_loader: Option<_RenderModule>,
    pub topo: String,
    pub cell: String,
    pub node: String,
    pub out: PathBuf,
    /// `{"yaml": Path, "sh": Path, "env": Path}` in that order.
    pub triplet: Vec<(String, PathBuf)>,
    /// cell env (`dict[str, str]` · values `J::Str`).
    pub env: Map,
    pub compose_path: Option<PathBuf>,
    pub compose_eff: RefCell<Option<PathBuf>>,
    pub default_runner: Cell<bool>,
    pub _docker: RefCell<HashMap<Vec<String>, (String, Option<core::Proc>)>>,
    /// string keys: Python tuple keys = parts joined with `\u{1f}` (`str()` of each part) · a plain str key = itself.
    pub memo: RefCell<Map>,
    /// coordinator shape field (unused: the render module is cached in `rd`).
    pub rd_loaded: RefCell<bool>,
}

impl _Ctx {
    /// `_Ctx(repo=, ev=, runner=, sf=, rd=, rd_loader=, topo=, cell=, node=)` with the dataclass defaults for the rest.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        repo: &Path,
        ev: &J,
        runner: Option<Rc<dyn DockerRunner>>,
        sf: Option<_ForwardModule>,
        rd: Option<_RenderModule>,
        rd_loader: Option<_RenderModule>,
        topo: &str,
        cell: &str,
        node: &str,
    ) -> _Ctx {
        _Ctx {
            repo: repo.to_path_buf(),
            ev: ev.clone(),
            runner: RefCell::new(runner),
            sf,
            rd: RefCell::new(rd),
            rd_loader,
            topo: topo.to_string(),
            cell: cell.to_string(),
            node: node.to_string(),
            out: PathBuf::from("."),
            triplet: Vec::new(),
            env: Map::new(),
            compose_path: None,
            compose_eff: RefCell::new(None),
            default_runner: Cell::new(false),
            _docker: RefCell::new(HashMap::new()),
            memo: RefCell::new(Map::new()),
            rd_loaded: RefCell::new(false),
        }
    }

    /// `c.triplet[key]` (`yaml`|`sh`|`env`).
    pub fn trip(&self, key: &str) -> Option<&Path> {
        self.triplet.iter().find(|(k, _)| k == key).map(|(_, p)| p.as_path())
    }

    /// `c.sf` (a `None` sf raises the attribute error the Python would).
    pub fn sf(&self, attr: &str) -> HR<&_ForwardModule> {
        self.sf.as_ref().ok_or_else(|| HintError::py("AttributeError", format!("'NoneType' object has no attribute '{attr}'")))
    }

    pub fn memo_key(key: &J) -> String {
        match key {
            J::List(parts) => parts.iter().map(J::display).collect::<Vec<_>>().join("\u{1f}"),
            other => other.display(),
        }
    }
    pub fn memo_get(&self, key: &J) -> Option<J> {
        self.memo.borrow().get(&Self::memo_key(key)).cloned()
    }
    pub fn memo_has(&self, key: &J) -> bool {
        self.memo.borrow().contains_key(&Self::memo_key(key))
    }
    pub fn memo_set(&self, key: &J, v: J) {
        self.memo.borrow_mut().insert(Self::memo_key(key), v);
    }

    /// `c.render()`.
    pub fn render(&self) -> HR<_RenderModule> {
        if self.rd.borrow().is_none() {
            let m = _render_module(&self.repo, self.rd_loader.as_ref())?;
            *self.rd.borrow_mut() = Some(m);
        }
        Ok(self.rd.borrow().clone().unwrap())
    }

    /// `c.docker(*args, cache=...)` → (class, proc). class ∈ ok · rc=<n> · docker-unavailable · timeout.
    pub fn docker(&self, args: &[&str], cache: bool) -> (String, Option<core::Proc>) {
        let key: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        if cache {
            if let Some(r) = self._docker.borrow().get(&key) {
                return r.clone();
            }
        }
        if self.runner.borrow().is_none() {
            let r: Rc<dyn DockerRunner> = _probe_only_runner(None).expect("runner");
            *self.runner.borrow_mut() = Some(r);
            self.default_runner.set(true);
        }
        let run = self.runner.borrow().clone().unwrap();
        let mut argv = vec!["docker".to_string()];
        argv.extend(key.iter().cloned());
        let res = match run.run(&argv, DOCKER_TIMEOUT_S as u64) {
            Ok(p) => (if p.returncode == 0 { "ok".to_string() } else { format!("rc={}", p.returncode) }, Some(p)),
            Err(RunFail::Unavailable) => ("docker-unavailable".to_string(), None),
            Err(RunFail::Timeout) => ("timeout".to_string(), None),
        };
        if cache {
            self._docker.borrow_mut().insert(key, res.clone());
        }
        res
    }

    /// `getattr(c.runner, PROBE_ATTR, False) is True`.
    pub fn runner_probes(&self) -> bool {
        self.runner.borrow().as_ref().is_some_and(|r| r.image_probe())
    }
    /// Same as [`_Ctx::runner_probes`] (name requested by a2).
    pub fn runner_declares_probe(&self) -> bool {
        self.runner_probes()
    }
}

/// `_ctx(repo, ev, runner=, forward_module=, render_module=)` with `&J` hooks (`J::Null` = default · runner see
/// [`_runner_from_j`] · a non-null module hook is a TypeError — use [`_ctx_with`]).
pub fn _ctx(repo: &Path, ev: &J, runner: &J, forward_module: &J, render_module: &J) -> HR<_Ctx> {
    let r = _runner_from_j(runner)?;
    if !forward_module.is_null() || !render_module.is_null() {
        return Err(HintError::py("TypeError", "module hooks are typed in Rust — use _ctx_with"));
    }
    _ctx_with(repo, ev, r, None, None)
}

/// `_ctx` with typed hooks.
pub fn _ctx_with(
    repo: &Path,
    ev: &J,
    runner: Option<Rc<dyn DockerRunner>>,
    forward_module: Option<&_ForwardModule>,
    render_module: Option<&_RenderModule>,
) -> HR<_Ctx> {
    let repo = core::resolve(repo);
    let default_runner = runner.is_none();
    let runner: Rc<dyn DockerRunner> = match runner {
        Some(r) => r,
        None => _probe_only_runner(None)?,
    };
    let sf = _forward_module(&repo, forward_module)?;
    let mut c = _Ctx::new(&repo, ev, Some(runner), Some(sf), None, render_module.cloned(), "", "", "");
    c.default_runner.set(default_runner);
    c.topo = _topology(ev)?;
    c.cell = _cell(ev)?;
    let node = g(ev, "node");
    c.node = if node.truthy() { node.display() } else { String::new() };
    c.out = repo.join("output").join(&c.topo);
    let supplied = g(ev, "triplet");
    let conv = [
        ("yaml", format!("output/{}/configs/{}.yaml", c.topo, c.cell)),
        ("sh", format!("output/{}/configs/{}.sh", c.topo, c.cell)),
        ("env", format!("output/{}/envs/.env.{}", c.topo, c.cell)),
    ];
    for (key, dflt) in conv.iter() {
        let raw = supplied.get(key).cloned().unwrap_or(J::Null);
        let rawp = if raw.truthy() { raw.display() } else { dflt.clone() };
        let rel = core::rel(&repo, Path::new(&rawp))?;
        c.triplet.push((key.to_string(), repo.join(rel)));
    }
    c.env = _read_env(c.sf.as_ref().unwrap(), c.trip("env"))?.as_dict().cloned().unwrap_or_default();
    for name in ["docker-compose.yaml", "docker-compose.yml"] {
        if c.out.join(name).is_file() {
            c.compose_path = Some(c.out.join(name));
            break;
        }
    }
    Ok(c)
}

pub fn _image_ref(c: &_Ctx) -> HR<(Option<String>, String)> {
    let build = g(&c.ev, "build_identity");
    let digest = match build.as_dict() {
        Some(m) => {
            let d = m.get("image_digest").cloned().unwrap_or(J::Null);
            py_strip(&if d.truthy() { d.display() } else { String::new() }).to_string()
        }
        None => String::new(),
    };
    if !digest.is_empty() {
        return Ok((Some(digest), "measured image_digest(evidence.build_identity · 내용 권위)".to_string()));
    }
    let (tag, tag_src) = _image_tag(c)?;
    if let Some(t) = tag.filter(|t| !t.is_empty()) {
        return Ok((Some(t), format!("{tag_src}(측정 digest 미기재 — 태그로 관측 · 태그는 가변 포인터)")));
    }
    Ok((None, "unobserved(측정 digest·이미지 태그 모두 없음)".to_string()))
}

pub fn _image_created(c: &_Ctx) -> HR<Option<String>> {
    let (r, _) = _image_ref(c)?;
    let Some(r) = r else { return Ok(None) };
    let (cls, proc) = c.docker(&["image", "inspect", "--format", "{{.Created}}", &r], true);
    match (cls.as_str(), proc) {
        ("ok", Some(p)) => _utc_z(Some(py_strip(&p.stdout))),
        _ => Ok(None),
    }
}

fn warn_hm(hm: &J) -> Option<(String, String)> {
    let m = hm.as_dict()?;
    let matched = m.get("matched").map(num).unwrap_or(0.0);
    let total = m.get("total").map(num).unwrap_or(0.0);
    if matched < total {
        Some((hm["matched"].display(), hm["total"].display()))
    } else {
        None
    }
}

pub fn _recipe_vs_image(c: &_Ctx, rels: &[String], created: Option<&str>, selected: &J) -> HR<Vec<J>> {
    let mut out: Vec<J> = Vec::new();
    if !c.repo.join(".git").exists() {
        return Ok(out);
    }
    let sel_get = |rel: &str| -> J { selected.as_dict().and_then(|m| m.get(rel)).cloned().unwrap_or(J::Null) };
    for rel in rels {
        let s0 = sel_get(rel);
        if s0.is_dict() && s0["origin_path"].truthy() {
            let mut row = jd! {
                "path" => s0["origin_path"].clone(), "checkout_path" => rel.as_str(), "tracked" => true,
                "worktree_equals_head" => J::Null, "last_commit_utc" => s0["origin_commit_utc"].clone(),
                "last_commit_kind" => "실린 개정 커밋",
            };
            if let Some(cr) = created.filter(|x| !x.is_empty()) {
                row.set("image_created_utc", cr);
            }
            row.set("shipped", s0["shipped"].clone());
            row.set("selection_method", s0["method"].clone());
            let hm = s0["history_match"].clone();
            if !hm.is_null() {
                row.set("history_match", hm.clone());
            }
            if !s0["verified"].truthy() {
                if let Some((m, t)) = warn_hm(&hm) {
                    row.set(
                        "warning",
                        format!("실린 바이트({})가 측정 이미지 docker history 의 RUN·COPY 와 전수 일치하지 않는다({m}/{t})", s0["shipped"].display()),
                    );
                } else if s0["ledger_sha256_match"] == J::Bool(false) {
                    row.set("warning", "빌드 시 원장의 COPY 원천 sha256과 실린 바이트가 다르다(이미지 파일 직접 관측 아님)");
                } else if s0["image_bytes_differ"].truthy() {
                    row.set("warning", "이미지 안 COPY 대상 바이트가 실린 바이트와 다르다(일치하는 개정 없음)");
                }
            }
            out.push(row);
            continue;
        }
        let data = read_bytes(&c.repo.join(rel))?;
        let blob = git_blob_sha(&data);
        let ls = core::git_nocheck(&c.repo, &["-c", "core.quotePath=false", "ls-tree", "HEAD", "--", rel])?;
        let parts: Vec<String> = if ls.returncode == 0 { py_split(&ls.stdout).into_iter().map(str::to_string).collect() } else { vec![] };
        let head_blob = if parts.len() >= 3 && parts[1] == "blob" { Some(parts[2].clone()) } else { None };
        let weh = head_blob.as_ref().map(|h| *h == blob);
        let mut row = jd! {"path" => rel.as_str(), "tracked" => head_blob.is_some(), "worktree_equals_head" => J::from(weh)};
        if head_blob.is_some() {
            let lg = core::git_nocheck(&c.repo, &["-c", "log.showSignature=false", "log", "-1", "--format=%ct", "--", rel])?;
            let ts = if lg.returncode == 0 { py_strip(&lg.stdout).to_string() } else { String::new() };
            if !ts.is_empty() && ts.chars().all(|ch| ch.is_ascii_digit()) {
                if let Ok(t) = ts.parse::<i64>() {
                    row.set("last_commit_utc", utc_of_epoch(t));
                }
            }
        }
        if let Some(cr) = created.filter(|x| !x.is_empty()) {
            row.set("image_created_utc", cr);
            let late = row.get_str("last_commit_utc").is_some_and(|l| !l.is_empty() && l > cr);
            if weh == Some(false) || late {
                let s = format!(
                    "이미지 빌드 뒤 바뀐 레시피일 수 있다 — {}{}",
                    if weh == Some(false) { "작업트리 수정본(미커밋) " } else { "" },
                    if late { "마지막 커밋이 이미지 Created 뒤" } else { "" }
                );
                row.set("warning", py_strip(&s));
            }
        }
        let s = sel_get(rel);
        if s.is_dict() {
            row.set("shipped", s["shipped"].clone());
            row.set("selection_method", s["method"].clone());
            if !s["history_match"].is_null() {
                row.set("history_match", s["history_match"].clone());
            }
            let hm = s["history_match"].clone();
            let prev_tail = |row: &J| -> String {
                match row.get("warning") {
                    Some(p) if p.truthy() => format!(" · {}", p.display()),
                    _ => String::new(),
                }
            };
            if s["verified"].truthy() {
                let was = row.as_dict_mut().unwrap().remove("warning");
                if let Some(w) = was.filter(|w| w.truthy()) {
                    row.set(
                        "note",
                        format!("작업트리 경고({})는 실린 파일에 해당하지 않는다 — 실린 바이트 = {} · {}", w.display(), s["shipped"].display(), s["method"].display()),
                    );
                }
            } else if let Some((m, t)) = warn_hm(&hm) {
                let tail = prev_tail(&row);
                row.set("warning", format!("실린 바이트({})가 측정 이미지 docker history 의 RUN·COPY 와 전수 일치하지 않는다({m}/{t}){tail}", s["shipped"].display()));
            } else if s["ledger_sha256_match"] == J::Bool(false) {
                let tail = prev_tail(&row);
                row.set("warning", format!("빌드 시 원장의 COPY 원천 sha256과 실린 작업트리 바이트가 다르다(이미지 파일 직접 관측 아님){tail}"));
            } else if s["image_bytes_differ"].truthy() {
                let tail = prev_tail(&row);
                row.set("warning", format!("이미지 안 COPY 대상 바이트가 실린 작업트리 바이트와 다르다(일치하는 개정 없음){tail}"));
            }
        }
        out.push(row);
    }
    Ok(out)
}

// ── 이미지를 지은 레시피 개정 선택 ────────────────────────────────────────────────────────────
pub fn _ws(text: &str) -> HR<String> {
    Ok(py_split(text).join(" "))
}

re_static!(instr_re, r"^\s*([A-Za-z]+)\s+(.*)$");

pub fn _dockerfile_steps(text: &str) -> HR<Vec<String>> {
    let lines = splitlines(text);
    let skip = |l: &str| py_strip(l).is_empty() || py_lstrip(l).starts_with('#');
    let mut steps: Vec<String> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let raw = lines[i];
        i += 1;
        if skip(raw) {
            continue;
        }
        let mut buf = py_rstrip(raw).to_string();
        while buf.ends_with('\\') && i < lines.len() {
            buf.pop();
            while i < lines.len() && skip(lines[i]) {
                i += 1;
            }
            if i < lines.len() {
                buf.push_str(py_rstrip(lines[i]));
                i += 1;
            }
        }
        let Some(m) = instr_re().match_at_start(&buf) else { continue };
        let instr = pyre::group(&m, 1).unwrap_or("").to_uppercase();
        let mut body = pyre::group(&m, 2).unwrap_or("").to_string();
        if instr == "FROM" {
            steps.clear();
            continue;
        }
        if !matches!(instr.as_str(), "RUN" | "COPY" | "ADD") {
            continue;
        }
        let mut docs: Vec<String> = Vec::new();
        let hds: Vec<(bool, String)> =
            _HEREDOC().finditer(&body).map(|h| (pyre::group(&h, 1) == Some("-"), pyre::group(&h, 3).unwrap_or("").to_string())).collect();
        for (dash, delim) in hds {
            let mut chunk: Vec<String> = Vec::new();
            while i < lines.len() {
                let ln = lines[i];
                i += 1;
                let cmp = if dash { py_strip(ln) } else { py_rstrip(ln) };
                if cmp == delim {
                    break;
                }
                chunk.push(if dash { ln.trim_start_matches('\t').to_string() } else { ln.to_string() });
            }
            docs.push(format!("{}\n{}", chunk.join("\n"), delim));
        }
        if !docs.is_empty() {
            body = format!("{}\n{}", body, docs.join("\n"));
        }
        if instr == "RUN" {
            while let Some(fm) = _RUN_FLAG().match_at_start(&body) {
                let end = fm.get(0).unwrap().end();
                body = body[end..].to_string();
            }
            if py_lstrip(&body).starts_with('[') {
                if let Ok(J::List(items)) = crate::value::loads(&body) {
                    body = items.iter().map(J::display).collect::<Vec<_>>().join(" ");
                }
            }
        }
        steps.push(format!("{} {}", instr, _ws(&body)?));
    }
    Ok(steps)
}

re_static!(buildkit_tail_re, r"\s+# buildkit\s*$");
re_static!(run_args_re, r"^\|(\d+)\s");

pub fn _history_step(created_by: &str) -> HR<Option<String>> {
    let s = buildkit_tail_re().sub_literal(py_strip(created_by), "");
    let (hd, rest) = match s.split_once(' ') {
        Some((a, b)) => (a.to_string(), b.to_string()),
        None => (s.clone(), String::new()),
    };
    if hd == "RUN" {
        let mut rest = rest;
        if let Some(m) = run_args_re().match_at_start(&rest) {
            let n: usize = pyre::group(&m, 1).unwrap_or("0").parse().unwrap_or(usize::MAX);
            let end = m.get(0).unwrap().end();
            let tail: Vec<&str> = rest[end..].split(' ').collect();
            rest = tail.iter().skip(n).cloned().collect::<Vec<_>>().join(" ");
        }
        let lst = py_lstrip(&rest).to_string();
        let sub = match _SHELL_C().match_at_start(&lst) {
            Some(m) => lst[m.get(0).unwrap().end()..].to_string(),
            None => lst,
        };
        return Ok(Some(format!("RUN {}", _ws(&sub)?)));
    }
    if hd == "COPY" || hd == "ADD" {
        return Ok(Some(format!("{} {}", hd, _ws(&rest)?)));
    }
    Ok(None)
}

pub fn _history_layers(c: &_Ctx) -> HR<(Option<Vec<J>>, String)> {
    let key = J::from("layers");
    if let Some(v) = c.memo_get(&key) {
        let rows = v.at(0).as_list().cloned();
        return Ok((rows, v.at(1).display()));
    }
    let (image, image_src) = _image_ref(c)?;
    let mut res: (Option<Vec<J>>, String) = (None, format!("unobserved({image_src})"));
    if let Some(image) = image {
        let (cls, proc) = c.docker(&["history", "--no-trunc", "--format", "{{json .}}", &image], true);
        match (cls.as_str(), proc) {
            ("ok", Some(p)) => {
                let mut rows: Option<Vec<J>> = Some(Vec::new());
                for ln in splitlines(&p.stdout) {
                    if py_strip(ln).is_empty() {
                        continue;
                    }
                    let d = match crate::value::loads(ln) {
                        Ok(d) => d,
                        Err(_) => {
                            rows = None;
                            break;
                        }
                    };
                    if !d.is_dict() || !d["CreatedBy"].is_str() {
                        rows = None;
                        break;
                    }
                    let cu = _utc_z(d["CreatedAt"].as_str())?;
                    rows.as_mut().unwrap().push(jd! {"created_by" => d["CreatedBy"].clone(), "created_utc" => J::from(cu)});
                }
                res = match rows {
                    Some(r) if !r.is_empty() => (Some(r), format!("docker history --no-trunc --format '{{{{json .}}}}' {image}")),
                    _ => (None, "unobserved(docker history JSON 판독 불가 — 층 경계를 가르지 못한다)".to_string()),
                };
            }
            (cls, _) => res = (None, format!("unobserved(docker history {cls})")),
        }
    }
    c.memo_set(&key, jl![res.0.clone().map(J::List).unwrap_or(J::Null), res.1.clone()]);
    Ok(res)
}

pub fn _history_match(steps: &[String], window: &[String]) -> HR<J> {
    let mut pool: Vec<(String, i64)> = Vec::new();
    for w in window {
        match pool.iter_mut().find(|(k, _)| k == w) {
            Some(e) => e.1 += 1,
            None => pool.push((w.clone(), 1)),
        }
    }
    let mut matched = 0i64;
    for s in steps {
        if let Some(e) = pool.iter_mut().find(|(k, n)| k == s && *n > 0) {
            e.1 -= 1;
            matched += 1;
        }
    }
    let rest: i64 = pool.iter().map(|(_, n)| *n).sum();
    Ok(jd! {"matched" => matched, "total" => steps.len() as i64 + rest})
}

pub fn _git_rev_before(c: &_Ctx, rel: &str, created: Option<&str>) -> HR<Option<String>> {
    let Some(created) = created.filter(|x| !x.is_empty()) else { return Ok(None) };
    if !c.repo.join(".git").exists() {
        return Ok(None);
    }
    let before = format!("--before={created}");
    let lg = core::git_nocheck(&c.repo, &["-c", "log.showSignature=false", "log", "-1", &before, "--format=%H", "--", rel])?;
    let sha = if lg.returncode == 0 { py_strip(&lg.stdout).to_string() } else { String::new() };
    Ok(if is_sha40(&sha) { Some(sha) } else { None })
}

pub const _RECIPE_DIR_GLOB: &str = "output/*";

pub fn _history_blobs(c: &_Ctx, name: &str) -> HR<Vec<J>> {
    let key = jl!["history_blobs", name];
    if let Some(v) = c.memo_get(&key) {
        return Ok(v.items().to_vec());
    }
    let mut out: Vec<J> = Vec::new();
    if c.repo.join(".git").exists() {
        let spec = format!(":(glob){_RECIPE_DIR_GLOB}/{name}");
        let lg = core::git_nocheck(
            &c.repo,
            &["-c", "log.showSignature=false", "-c", "core.quotePath=false", "log", "--all", "--format=@%H %cI", "--name-only", "--", &spec],
        )?;
        let mut seen: BTreeSet<String> = BTreeSet::new();
        let (mut commit, mut utc): (Option<String>, String) = (None, String::new());
        let text = if lg.returncode == 0 { lg.stdout.clone() } else { String::new() };
        for line in splitlines(&text) {
            if let Some(r) = line.strip_prefix('@') {
                let (a, b) = r.split_once(' ').unwrap_or((r, ""));
                commit = Some(a.to_string());
                utc = b.to_string();
                continue;
            }
            let path = py_strip(line);
            let Some(cm) = commit.clone() else { continue };
            let ok = path.strip_prefix("output/").and_then(|r| r.split_once('/')).is_some_and(|(seg, rest)| !seg.is_empty() && rest == name);
            if path.is_empty() || !ok {
                continue;
            }
            let spec2 = format!("{cm}:{path}");
            let rp = core::git_nocheck(&c.repo, &["rev-parse", &spec2])?;
            let blob = if rp.returncode == 0 { py_strip(&rp.stdout).to_string() } else { String::new() };
            if !is_sha40(&blob) || seen.contains(&blob) {
                continue;
            }
            seen.insert(blob.clone());
            if let Some(b) = core::git_bytes(&c.repo, &["cat-file", "blob", &blob], false)? {
                let u = _utc_z(Some(&utc))?.unwrap_or_else(|| utc.clone());
                out.push(jd! {"commit" => cm.as_str(), "path" => path, "blob" => blob.as_str(), "bytes" => _bytes_j(&b), "utc" => u});
            }
        }
    }
    c.memo_set(&key, J::List(out.clone()));
    Ok(out)
}

pub fn _prefer_same_path(cands: &J, rel: &str) -> HR<Vec<J>> {
    let mut v: Vec<J> = cands.items().to_vec();
    v.sort_by_key(|x| x.get_str("path") != Some(rel));
    Ok(v)
}

pub fn _better(a: &J, b: &J) -> HR<bool> {
    let (am, at, bm, bt) = (num(&a["matched"]), num(&a["total"]), num(&b["matched"]), num(&b["total"]));
    Ok(at > 0.0 && (bt == 0.0 || am * bt > bm * at))
}

fn full_score(s: &J) -> bool {
    num(&s["total"]) != 0.0 && num(&s["matched"]) == num(&s["total"])
}

pub fn _select_dockerfile(c: &_Ctx, dockerfile: &str) -> HR<J> {
    let key = jl!["dockerfile", dockerfile];
    if let Some(v) = c.memo_get(&key) {
        return Ok(v);
    }
    let path = c.out.join(dockerfile);
    let wt = read_bytes(&path)?;
    let rel = core::rel(&c.repo, &path)?;
    let mut out = jd! {
        "path" => rel.as_str(), "bytes" => _bytes_j(&wt), "mode" => file_mode(&path)?, "shipped" => "worktree",
        "commit" => J::Null, "history_match" => J::Null, "candidates" => J::list(), "verified" => false,
    };
    let (layers, lsrc) = _history_layers(c)?;
    let created = _image_created(c)?;
    match layers {
        None => out.set("method", format!("worktree(이미지 history 미관측 — 개정 대조 불가 · {lsrc})")),
        Some(layers) => {
            let mut cands: Vec<(String, Vec<u8>)> = vec![("worktree".to_string(), wt.clone())];
            let rev = _git_rev_before(c, &rel, created.as_deref())?;
            let rev_bytes = match &rev {
                Some(r) => core::git_bytes(&c.repo, &["show", &format!("{r}:{rel}")], false)?,
                None => None,
            };
            if let (Some(r), Some(rb)) = (&rev, &rev_bytes) {
                if *rb != wt {
                    cands.push((r.clone(), rb.clone()));
                }
            }
            let mut steps: Vec<(String, Vec<String>)> = Vec::new();
            for (name, b) in &cands {
                steps.push((name.clone(), _dockerfile_steps(&lossy(b))?));
            }
            let union: BTreeSet<String> = steps.iter().flat_map(|(_, s)| s.iter().cloned()).collect();
            let mut hist: Vec<Option<String>> = Vec::new();
            for x in &layers {
                hist.push(_history_step(&x["created_by"].display())?);
            }
            let deepest: i64 = hist.iter().enumerate().filter(|(_, h)| h.as_ref().is_some_and(|h| !h.is_empty() && union.contains(h))).map(|(i, _)| i as i64).max().unwrap_or(-1);
            let upto = (deepest + 1) as usize;
            let window: Vec<String> = hist[..upto].iter().filter_map(|h| h.clone().filter(|x| !x.is_empty())).collect();
            let mut own: Vec<String> = layers[..upto].iter().filter(|x| x["created_utc"].truthy()).map(|x| x["created_utc"].display()).collect();
            own.sort();
            if !own.is_empty() {
                out.set(
                    "layer_span",
                    jd! {"oldest_utc" => own[0].as_str(), "newest_utc" => own[own.len() - 1].as_str(), "layers" => deepest + 1,
                         "source" => lsrc.as_str(), "note" => "우리 Dockerfile 층의 CreatedAt 범위 — 캐시 재사용 층은 이전 빌드 시각을 가진다(소요 아님)"},
                );
            }
            let mut scores: Vec<(String, J)> = Vec::new();
            for (name, st) in &steps {
                scores.push((name.clone(), _history_match(st, &window)?));
            }
            let score = |n: &str| scores.iter().find(|(k, _)| k == n).map(|(_, v)| v.clone()).unwrap_or(J::Null);
            let mut cl = Vec::new();
            for (name, _) in &cands {
                let mut m = jd! {"revision" => name.as_str()};
                for (k, v) in score(name).as_dict().unwrap().iter() {
                    m.set(k, v.clone());
                }
                cl.push(m);
            }
            out.set("candidates", J::List(cl));
            let w = score("worktree");
            let best: Option<String> = if cands.len() > 1 && _better(&score(&cands[1].0), &w)? { Some(cands[1].0.clone()) } else { None };
            let w_full = full_score(&w);
            let b_full = best.as_ref().is_some_and(|b| full_score(&score(b)));
            let floor = match &best {
                Some(b) => score(b),
                None => w.clone(),
            };
            let hit = if w_full || b_full { None } else { dockerfile_from_history_t(c, dockerfile, &rel, &hist, &union, &floor)? };
            let (wm, wtt) = (w["matched"].display(), w["total"].display());
            let created_s = opt_disp(created.as_deref());
            if w_full {
                set_all(&mut out, vec![
                    ("method", J::from("worktree(측정 이미지 docker history 의 RUN·COPY 와 전수 일치 · Dockerfile 전체 바이트는 미검증)")),
                    ("history_match", w.clone()),
                    ("verified", J::Bool(false)),
                ]);
            } else if best.is_some() && (b_full || hit.is_none()) {
                let bname = best.clone().unwrap();
                let b = cands.iter().find(|(n, _)| *n == bname).map(|(_, b)| b.clone()).unwrap();
                let sb = score(&bname);
                set_all(&mut out, vec![
                    ("bytes", _bytes_j(&b)),
                    ("shipped", J::from(format!("git:{bname}"))),
                    ("commit", J::from(bname.as_str())),
                    ("history_match", sb.clone()),
                    ("verified", J::Bool(false)),
                    ("method", J::from(format!(
                        "git log -1 --before={created_s} -- {rel} · docker history 대조(개정 {}/{} > 작업트리 {wm}/{wtt}) — 이 개정의 바이트를 싣는다(git show)",
                        sb["matched"].display(), sb["total"].display()
                    ))),
                ]);
            } else if let Some(hit) = hit {
                let full = hit["full"].truthy();
                let (hm, ht) = (hit["score"]["matched"].display(), hit["score"]["total"].display());
                let commit = hit["commit"].display();
                let hpath = hit["path"].display();
                let unmatched: Vec<String> = hit["unmatched"].items().iter().map(J::display).collect();
                let mut method = format!("git log --all -- {_RECIPE_DIR_GLOB}/{dockerfile} · {}:{hpath} 가 docker history ", head(&commit, 12));
                if full {
                    method.push_str(&format!("RUN·COPY 와 전수 일치({hm}/{ht}"));
                } else {
                    method.push_str(&format!("RUN·COPY 와 가장 잘 맞는 이력 개정({hm}/{ht} · 전수 일치 개정 없음 · 불일치 {}", unmatched.join(" | ")));
                }
                method.push_str(&format!(" · 작업트리 {wm}/{wtt})"));
                if hpath != rel {
                    method.push_str(&format!(" — 이 체크아웃 통로({rel}) 밖"));
                }
                let hutc = hit["utc"].clone();
                if let Some(cr) = created.as_deref().filter(|x| !x.is_empty()) {
                    if hutc.truthy() && hutc.display().as_str() > cr {
                        method.push_str(&format!(
                            " · 커밋 {} > 이미지 Created {cr}{}",
                            hutc.display(),
                            if full { "(빌드 뒤 커밋된 같은 단계)" } else { "(빌드 뒤 커밋 · 위 불일치 단계만큼 다르다)" }
                        ));
                    }
                }
                method.push_str(" — 이 개정의 바이트를 싣는다(git cat-file)");
                set_all(&mut out, vec![
                    ("bytes", hit["bytes"].clone()),
                    ("shipped", J::from(format!("git:{commit}"))),
                    ("commit", J::from(commit.as_str())),
                    ("history_match", hit["score"].clone()),
                    ("verified", J::Bool(false)),
                    ("origin_path", J::from(hpath.as_str())),
                    ("origin_commit_utc", hutc.clone()),
                    ("method", J::from(method)),
                ]);
            } else {
                let mut method = format!("worktree(어느 개정도 이미지 history 와 더 맞지 않는다 · 작업트리 {wm}/{wtt}");
                if rev.is_none() {
                    method.push_str(&format!(
                        " · 대조할 개정 없음({})",
                        if created.as_deref().filter(|x| !x.is_empty()).is_none() { "이미지 Created 미관측" } else { "이미지 Created 이전 커밋 부재 · 비추적" }
                    ));
                }
                method.push_str(&format!(" · 전 이력({_RECIPE_DIR_GLOB}/{dockerfile} · 모든 ref)에도 전수 일치 개정 없음"));
                method.push_str(" — recipe_vs_image 경고 유지)");
                set_all(&mut out, vec![("history_match", w.clone()), ("method", J::from(method))]);
            }
        }
    }
    let text = lossy(&_j_bytes(&out["bytes"]).unwrap_or_default());
    out.set("text", text);
    c.memo_set(&key, out.clone());
    Ok(out)
}

fn dockerfile_from_history_t(c: &_Ctx, dockerfile: &str, rel: &str, hist: &[Option<String>], union: &BTreeSet<String>, floor: &J) -> HR<Option<J>> {
    let mut scored: Vec<J> = Vec::new();
    for h in _history_blobs(c, dockerfile)? {
        let st = _dockerfile_steps(&lossy(&_j_bytes(&h["bytes"]).unwrap_or_default()))?;
        let mut u = union.clone();
        u.extend(st.iter().cloned());
        let deepest: i64 = hist.iter().enumerate().filter(|(_, x)| x.as_ref().is_some_and(|x| !x.is_empty() && u.contains(x))).map(|(i, _)| i as i64).max().unwrap_or(-1);
        let window: Vec<String> = hist[..(deepest + 1) as usize].iter().filter_map(|x| x.clone().filter(|y| !y.is_empty())).collect();
        let sc = _history_match(&st, &window)?;
        if num(&sc["total"]) == 0.0 {
            continue;
        }
        let mut pool = window.clone();
        let mut unmatched: Vec<J> = Vec::new();
        for x in &st {
            if let Some(p) = pool.iter().position(|y| y == x) {
                pool.remove(p);
            } else {
                unmatched.push(J::from(format!("레시피에만: {}", head(x, 120))));
            }
        }
        for x in &pool {
            unmatched.push(J::from(format!("이미지에만: {}", head(x, 120))));
        }
        let mut row = h.clone();
        let full = num(&sc["matched"]) == num(&sc["total"]);
        row.set("score", sc);
        row.set("full", full);
        row.set("unmatched", J::List(unmatched));
        scored.push(row);
    }
    let full: Vec<J> = scored.iter().filter(|x| x["full"].truthy()).cloned().collect();
    if !full.is_empty() {
        return Ok(_prefer_same_path(&J::List(full), rel)?.into_iter().next());
    }
    let mut better: Vec<J> = Vec::new();
    for x in &scored {
        if _better(&x["score"], floor)? {
            better.push(x.clone());
        }
    }
    if better.is_empty() {
        return Ok(None);
    }
    let ratio = |x: &J| num(&x["score"]["matched"]) / num(&x["score"]["total"]);
    let top = better.iter().map(ratio).fold(f64::NEG_INFINITY, f64::max);
    let tops: Vec<J> = better.into_iter().filter(|x| ratio(x) == top).collect();
    Ok(_prefer_same_path(&J::List(tops), rel)?.into_iter().next())
}

/// `_dockerfile_from_history(c, dockerfile, rel, hist, union, floor)` (`hist` = list of str|None · `union` = list/set of str).
pub fn _dockerfile_from_history(c: &_Ctx, dockerfile: &str, rel: &str, hist: &J, union: &J, floor: &J) -> HR<Option<J>> {
    let h: Vec<Option<String>> = hist.items().iter().map(|x| x.as_str().map(str::to_string)).collect();
    let u: BTreeSet<String> = crate::api::unwrap_seq(union).iter().filter_map(|x| x.as_str().map(str::to_string)).collect();
    dockerfile_from_history_t(c, dockerfile, rel, &h, &u, floor)
}

pub fn _copy_dests(text: &str) -> HR<J> {
    let mut out = Map::new();
    for m in _DOCKERFILE_COPY().finditer(text) {
        let g2 = pyre::group(&m, 2).unwrap_or("");
        let body = py_strip(g2.split('#').next().unwrap_or("")).to_string();
        if body.starts_with('[') {
            continue;
        }
        let toks: Vec<&str> = py_split(&body);
        if toks.iter().any(|t| t.starts_with("--from")) {
            continue;
        }
        let toks: Vec<&str> = toks.into_iter().filter(|t| !t.starts_with("--")).collect();
        if toks.len() != 2 || !toks[1].starts_with('/') {
            continue;
        }
        let src = toks[0].trim_end_matches('/');
        if !src.is_empty() && !src.contains("://") && !src.starts_with("git@") && !out.contains_key(src) {
            out.insert(src, toks[1]);
        }
    }
    Ok(J::Dict(out))
}

pub fn _dest_path(src: &str, dst: &str, src_is_dir: bool) -> HR<String> {
    if src_is_dir {
        let d = dst.trim_end_matches('/');
        return Ok(if d.is_empty() { "/".to_string() } else { d.to_string() });
    }
    Ok(if dst.ends_with('/') { format!("{}/{}", dst.trim_end_matches('/'), purepath_name(src)) } else { dst.to_string() })
}

/// `_select_copied(c, src_rel, path, baked, probe_note, ledger_sha256=)` — `baked` = `{"__bytes__"}` | `J::Null`.
pub fn _select_copied(c: &_Ctx, src_rel: &str, path: &Path, baked: &J, probe_note: &str, ledger_sha256: Option<&str>) -> HR<J> {
    let baked = _j_bytes(baked);
    let wt = read_bytes(path)?;
    let rel = core::rel(&c.repo, path)?;
    let pname = purepath_name(&core::posix(path));
    let mut out = jd! {
        "path" => rel.as_str(), "bytes" => _bytes_j(&wt), "mode" => file_mode(path)?, "shipped" => "worktree", "commit" => J::Null,
        "history_match" => J::Null, "verified" => false,
    };
    if baked.is_none() && ledger_sha256.is_some_and(|x| !x.is_empty()) {
        let ls = ledger_sha256.unwrap();
        out.set("ledger_sha256", ls);
        if core::sha256_bytes(&wt) == ls {
            set_all(&mut out, vec![
                ("method", J::from("worktree(빌드 원장 sha256 일치 — 쓰인 바이트 관측 확인 = 예)")),
                ("verified", J::Bool(true)),
                ("ledger_sha256_match", J::Bool(true)),
            ]);
            return Ok(out);
        }
        let created = _image_created(c)?;
        let rev = _git_rev_before(c, &rel, created.as_deref())?;
        let rev_bytes = match &rev {
            Some(r) => core::git_bytes(&c.repo, &["show", &format!("{r}:{rel}")], false)?,
            None => None,
        };
        if let Some(rb) = rev_bytes.as_ref().filter(|rb| core::sha256_bytes(rb) == ls) {
            let r = opt_disp(rev.as_deref());
            set_all(&mut out, vec![
                ("bytes", _bytes_j(rb)),
                ("shipped", J::from(format!("git:{r}"))),
                ("commit", J::from(rev.clone())),
                ("verified", J::Bool(true)),
                ("ledger_sha256_match", J::Bool(true)),
                ("method", J::from(format!("git log -1 --before={} -- {rel} · 빌드 원장 sha256 일치(작업트리는 다르다)", opt_disp(created.as_deref())))),
            ]);
            return Ok(out);
        }
        let hits: Vec<J> = _history_blobs(c, &pname)?
            .into_iter()
            .filter(|h| core::sha256_bytes(&_j_bytes(&h["bytes"]).unwrap_or_default()) == ls)
            .collect();
        if !hits.is_empty() {
            let h = _prefer_same_path(&J::List(hits), &rel)?.remove(0);
            let commit = h["commit"].display();
            let hpath = h["path"].display();
            let mut method = format!(
                "git log --all -- {_RECIPE_DIR_GLOB}/{pname} · {}:{hpath} · 빌드 원장 sha256 일치(작업트리·이미지 Created 이전 개정은 다르다)",
                head(&commit, 12)
            );
            if hpath != rel {
                method.push_str(&format!(" — 이 체크아웃 통로({rel}) 밖"));
            }
            if let Some(cr) = created.as_deref().filter(|x| !x.is_empty()) {
                if h["utc"].truthy() && h["utc"].display().as_str() > cr {
                    method.push_str(&format!(" · 커밋 {} > 이미지 Created {cr}(빌드 뒤 커밋된 같은 바이트)", h["utc"].display()));
                }
            }
            set_all(&mut out, vec![
                ("bytes", h["bytes"].clone()),
                ("shipped", J::from(format!("git:{commit}"))),
                ("commit", J::from(commit.as_str())),
                ("verified", J::Bool(true)),
                ("ledger_sha256_match", J::Bool(true)),
                ("origin_path", J::from(hpath.as_str())),
                ("origin_commit_utc", h["utc"].clone()),
                ("method", J::from(method)),
            ]);
            return Ok(out);
        }
        set_all(&mut out, vec![
            ("ledger_sha256_match", J::Bool(false)),
            ("method", J::from(format!(
                "worktree(빌드 원장 sha256 {} 과 작업트리·이미지 Created 이전 개정 · 전 이력({_RECIPE_DIR_GLOB}/{pname} · 모든 ref)이 모두 다르다 — 쓰인 바이트가 아니다 · recipe_vs_image 경고)",
                head(ls, 16)
            ))),
        ]);
        return Ok(out);
    }
    let Some(baked) = baked else {
        out.set("method", format!("worktree(이미지 안 COPY 대상 바이트 미관측 — {probe_note})"));
        return Ok(out);
    };
    if baked == wt {
        set_all(&mut out, vec![("method", J::from("worktree(image-probe: 이미지 안 COPY 대상 바이트와 sha256 일치)")), ("verified", J::Bool(true))]);
        return Ok(out);
    }
    let created = _image_created(c)?;
    let rev = _git_rev_before(c, &rel, created.as_deref())?;
    let rev_bytes = match &rev {
        Some(r) => core::git_bytes(&c.repo, &["show", &format!("{r}:{rel}")], false)?,
        None => None,
    };
    if let Some(rb) = rev_bytes.filter(|rb| *rb == baked) {
        set_all(&mut out, vec![
            ("bytes", _bytes_j(&rb)),
            ("shipped", J::from(format!("git:{}", opt_disp(rev.as_deref())))),
            ("commit", J::from(rev.clone())),
            ("verified", J::Bool(true)),
            ("method", J::from(format!("git log -1 --before={} -- {rel} · image-probe 바이트 일치(작업트리는 다르다)", opt_disp(created.as_deref())))),
        ]);
    } else {
        out.set(
            "method",
            format!(
                "worktree(이미지 안 바이트와 다르고 이미지 Created 이전 개정도 같지 않다 — recipe_vs_image 경고 · 이미지 sha256 {})",
                head(&core::sha256_bytes(&baked), 16)
            ),
        );
        out.set("image_bytes_differ", true);
    }
    let _ = src_rel;
    Ok(out)
}

// ── 측정 시각 · 측정 뒤 재생성 ────────────────────────────────────────────────────────────────
pub fn _measured_utc(c: &_Ctx) -> HR<(Option<String>, String)> {
    let cert = g(&c.ev, "certificate");
    if cert.is_dict() {
        if let Some(got) = _utc_z(cert["measured_utc"].as_str())? {
            return Ok((Some(got), "certificate.measured_utc".to_string()));
        }
    }
    let sweep = g(&c.ev, "sweep");
    if sweep.is_dict() {
        if let Some(got) = _utc_z(sweep["generated_utc"].as_str())? {
            return Ok((Some(got), "sweep_index.generated_utc".to_string()));
        }
    }
    let lite = _lite_measured_utc(&c.repo, &c.ev)?;
    if lite.0.as_deref().is_some_and(|x| !x.is_empty()) {
        return Ok(lite);
    }
    Ok((None, "unobserved(인증서 measured_utc · 스윕 generated_utc · lite 리포트 생성일 없음)".to_string()))
}

pub fn _lite_window(c: &_Ctx) -> HR<Option<J>> {
    if !is_cell_evidence(&c.ev) {
        return Ok(None);
    }
    crate::evidence::lite_window(&c.repo, &c.ev)
}

pub fn _lite_tool_args(c: &_Ctx) -> HR<Option<J>> {
    if !is_cell_evidence(&c.ev) {
        return Ok(None);
    }
    let key = jl!["lite_tool_args"];
    if !c.memo_has(&key) {
        let v = crate::evidence::lite_tool_args(&c.repo, &c.ev)?;
        c.memo_set(&key, v.unwrap_or(J::Null));
    }
    let v = c.memo_get(&key).unwrap_or(J::Null);
    Ok(if v.is_null() { None } else { Some(v) })
}

pub fn _lite_measured_utc(repo: &Path, ev: &J) -> HR<(Option<String>, String)> {
    if g(ev, "report_kind").as_str() != Some("lite") {
        return Ok((None, "lite 셀 아님".to_string()));
    }
    let rel = g(ev, "bench_report_path");
    let born = if rel.truthy() {
        let b = crate::evidence::_report_born_utc(repo, rel.as_str())?;
        _utc_z(b.as_deref())?
    } else {
        None
    };
    let Some(born) = born else { return Ok((None, "lite bench_report 생성일 미관측".to_string())) };
    Ok((Some(born), format!("lite bench_report 생성일({} 머리 · lite 측정 시작 — 부하 직전 lite_bench.sh MEASURED_UTC)", purepath_name(&rel.display()))))
}

pub fn _mtime_utc(path: &Path) -> HR<Option<String>> {
    let Ok(m) = std::fs::metadata(path) else { return Ok(None) };
    let Ok(t) = m.modified() else { return Ok(None) };
    let secs = match t.duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => {
            let us = (d.as_nanos() + 500) / 1000;
            (us / 1_000_000) as i64
        }
        Err(e) => {
            let d = e.duration();
            let us = (d.as_nanos() + 500) / 1000;
            -(((us + 999_999) / 1_000_000) as i64)
        }
    };
    Ok(Some(utc_of_epoch(secs)))
}

pub fn _regenerated(c: &_Ctx, path: &Path) -> HR<Option<J>> {
    let (measured, msrc) = _measured_utc(c)?;
    let mt = _mtime_utc(path)?;
    if let (Some(me), Some(mt)) = (&measured, &mt) {
        if mt > me {
            return Ok(Some(jd! {"mtime_utc" => mt.as_str(), "measured_utc" => me.as_str(), "measured_source" => msrc}));
        }
    }
    Ok(None)
}

// ── 측정 뒤 재생성된 render 3층 env 의 측정 당시 판본 재렌더 ───────────────────────────────────
/// hintlib/artifacts.py:1044 RERENDER_FUNCS (python literal as JSON)
pub fn RERENDER_FUNCS() -> &'static J {
    static CELL: std::sync::OnceLock<J> = std::sync::OnceLock::new();
    CELL.get_or_init(|| crate::value::loads("{\"interconnect\": \"render_nccl_envfile\", \"cluster\": \"render_cluster_envfile\"}").expect("constant"))
}
re_static!(RERENDER_GENERATED_BY_RE, r"^renderer@[0-9a-f]{12}\Z");
pub const RERENDER_STATUS: &str = "rerendered-at-measurement-revision";

/// `_rerender_at_measurement(c, kind)`. Port boundary: the Python executed the measurement-time renderer **revision**
/// (`git show <rev>:render_dockerfile.py` loaded as a module). This port never starts Python: the revision identity is the
/// renderer's Rust source (`REL_RENDER_DOCKERFILE_SRC`); a measurement before any Rust revision (only the `.py` existed) =
/// RustPortBoundary (fail closed); it re-renders only when the Rust revision's bytes equal the worktree renderer (whose native port `render_dockerfile.sh --nccl-envfile|--cluster-envfile`
/// is the same function); any other revision = `{ok: False, why: "렌더러 <rev12> 적재 실패: …"}` (fail closed · reported).
pub fn _rerender_at_measurement(c: &_Ctx, kind: &str) -> HR<J> {
    let key = jl!["rerender", kind];
    if let Some(v) = c.memo_get(&key) {
        return Ok(v);
    }
    let no = |why: String| -> HR<J> {
        let v = jd! {"ok" => false, "why" => why};
        c.memo_set(&key, v.clone());
        Ok(v)
    };
    let (measured, msrc) = _measured_utc(c)?;
    // revision identity = the renderer's Rust source; the launcher (`render_dockerfile.sh`) is what runs (plan_26111615 D4).
    let rel = core::REL_RENDER_DOCKERFILE_SRC;
    let Some(fn_name) = RERENDER_FUNCS().get_str(kind).map(str::to_string) else {
        return no(format!("render 3층 env 가 아니다(kind={kind}) — 재렌더 대상 아님"));
    };
    let Some(measured) = measured.filter(|x| !x.is_empty()) else {
        return no("측정 시각 미관측 — 측정 당시 렌더러 판본을 고를 수 없다".to_string());
    };
    if !c.repo.join(".git").exists() {
        return no("git 저장소 아님 — 측정 당시 렌더러 판본을 꺼낼 수 없다".to_string());
    }
    let Some(rev) = _git_rev_before(c, rel, Some(&measured))? else {
        // Before the Rust port only the Python renderer existed: that revision is never run (no Python · fail closed, named).
        let pre = core::REL_RENDER_DOCKERFILE_PRE_PORT;
        if let Some(prev) = _git_rev_before(c, pre, Some(&measured))? {
            return no(format!(
                "렌더러 {} 적재 실패: RustPortBoundary: 측정 당시 렌더러는 Rust 이식 전 판본({pre})이다 — Python 렌더러는 실행하지 않는다(재렌더 ✗ · 측정 당시 {rel} 커밋 없음)",
                head(&prev, 12)
            ));
        }
        return no(format!("git log -1 --before={measured} -- {rel} 없음(측정 전 렌더러 커밋 없음)"));
    };
    let rev12 = head(&rev, 12);
    let Some(data) = core::git_bytes(&c.repo, &["show", &format!("{rev}:{rel}")], false)? else {
        return no(format!("git show {rev12}:{rel} 실패"));
    };
    let man = c.repo.join("output").join(&c.topo).join("manifest.yaml");
    if !man.is_file() {
        return no(format!("output/{}/manifest.yaml 부재 — 렌더 입력 없음", c.topo));
    }
    let worktree = std::fs::read(c.repo.join(rel)).ok();
    if worktree.as_deref() != Some(&data[..]) {
        return no(format!(
            "렌더러 {rev12} 적재 실패: RustPortBoundary: 측정 당시 렌더러 판본({rel})이 작업트리와 다르다 — 그 판본의 실행파일은 이 노드에 빌드돼 있지 않다. 바이트가 같을 때만 native 렌더러(render_dockerfile.sh)로 재렌더한다"
        ));
    }
    let flag = if kind == "interconnect" { "--nccl-envfile" } else { "--cluster-envfile" };
    let args = vec![flag.to_string(), "--manifest".to_string(), core::posix(&man)];
    let p = match core::run_owner(&c.repo, core::REL_RENDER_DOCKERFILE, &args, None, &[], false) {
        Ok(p) => p,
        Err(e) => return no(format!("렌더러 {rev12} 적재 실패: HintError: {}", e.message)),
    };
    if p.returncode != 0 {
        let last = p.stderr.lines().rev().map(py_strip).find(|l| !l.is_empty()).unwrap_or("").to_string();
        return no(format!("렌더러 {rev12}.{fn_name} 거부: {last}"));
    }
    let text = p.stdout;
    if py_strip(&text).is_empty() {
        return no(format!("렌더러 {rev12}.{fn_name} 가 빈 결과를 냈다"));
    }
    let mmt = _mtime_utc(&man)?;
    let mbasis = match &mmt {
        Some(m) if m.as_str() <= measured.as_str() => "manifest mtime ≤ 측정(측정 당시 입력으로 관측)".to_string(),
        _ => format!("manifest mtime {} > 측정 — 측정 뒤 바뀐 입력일 수 있다(비추적 · 측정 당시 manifest 미관측)", opt_disp(mmt.as_deref())),
    };
    let v = jd! {
        "ok" => true, "rev" => rev.as_str(), "rev12" => rev12.as_str(), "generated_by" => format!("renderer@{rev12}"), "text" => text,
        "module" => jd!{"__render_module__" => rel, "rev" => rev.as_str()}, "function" => fn_name.as_str(),
        "manifest" => format!("output/{}/manifest.yaml", c.topo), "manifest_mtime_utc" => J::from(mmt), "manifest_basis" => mbasis,
        "measured_utc" => measured.as_str(), "measured_source" => msrc,
        "method" => format!("git log -1 --before={measured} -- {rel} → {rev12} · git show · {fn_name}(load_manifest(output/{}/manifest.yaml))", c.topo),
    };
    c.memo_set(&key, v.clone());
    Ok(v)
}

pub fn _rerender_reason(rr: &J) -> HR<String> {
    Ok(format!("측정 뒤 재생성된 실행 파일 대신 측정 당시 판본 렌더러 {} 로 같은 manifest 를 다시 렌더한 형상(실행 바이트 아님)", rr["rev12"].display()))
}

pub fn _shape_regenerated(c: &_Ctx, p: &Path, name: &str, kind: &str, regen: &J, derived_out: &J) -> HR<J> {
    let _ = derived_out; // a `&J` cannot be appended to — callers that need the derived rows use `_shape_regenerated_d`
    _shape_regenerated_d(c, p, name, kind, regen, None)
}

/// `_shape_regenerated` with the `derived_out` out-list (forwarded to a2's `_env_shape_d`).
pub fn _shape_regenerated_d(c: &_Ctx, p: &Path, name: &str, kind: &str, regen: &J, derived_out: Option<&mut Vec<J>>) -> HR<J> {
    let _ = p;
    let rr = _rerender_at_measurement(c, kind)?;
    if !rr["ok"].truthy() {
        return Ok(jd! {"ok" => false, "why" => rr["why"].clone()});
    }
    let td = core::TempDir::new("hint-rerender-");
    let tp = td.path().join(name);
    std::fs::write(&tp, rr["text"].display().as_bytes()).map_err(|e| core::io_err(&e, &tp))?;
    let mut rerender = rr.clone();
    rerender.set("mtime_utc", regen["mtime_utc"].clone());
    rerender.set("file", name);
    let measured = regen["measured_utc"].as_str().map(str::to_string);
    let (text, rule) = _env_shape_d(&c.repo, &tp, kind, &rr["module"], &J::Null, &J::Null, measured.as_deref(), derived_out, &rerender)?;
    drop(td);
    Ok(jd! {"ok" => true, "text" => text, "rule" => rule, "rr" => rr})
}

// ── compose 슬롯 추적 파일의 측정 당시 개정 ──────────────────────────────────────────────────
pub const _TRACKED_KEYS: [&str; 13] = ["path", "shipped", "commit", "method", "basis", "verified", "executed", "measured_utc", "mtime_utc", "commits_after_measurement", "cross_check", "history_match", "warning"];

pub fn _select_tracked(c: &_Ctx, path: &Path) -> HR<J> {
    let rel = _repo_rel(c, path)?;
    let key = jl!["tracked", rel.clone().unwrap_or_else(|| core::posix(path))];
    if let Some(v) = c.memo_get(&key) {
        return Ok(v);
    }
    let wt = read_bytes(path)?;
    let (measured, msrc) = _measured_utc(c)?;
    let mt = _mtime_utc(path)?;
    let pname = purepath_name(&core::posix(path));
    let mut out = jd! {
        "path" => rel.clone().unwrap_or_else(|| format!("<저장소 밖>/{pname}")), "bytes" => _bytes_j(&wt), "mode" => file_mode(path)?,
        "shipped" => "worktree", "commit" => J::Null, "basis" => "unobserved", "verified" => false,
        "measured_utc" => J::from(measured.clone()), "mtime_utc" => J::from(mt.clone()),
        "commits_after_measurement" => J::list(), "cross_check" => J::list(), "history_match" => J::Null,
    };
    let mt_s = opt_disp(mt.as_deref());
    let newer = measured.is_some() && (mt.is_none() || mt.as_deref().unwrap() > measured.as_deref().unwrap());
    match &measured {
        None => out.set("method", format!("worktree(측정 시각 미관측 — 측정 당시 개정 대조 불가 · {msrc})")),
        Some(measured) if rel.is_none() || !c.repo.join(".git").exists() => {
            let why = if rel.is_none() { "저장소 밖 파일" } else { "git 저장소 아님" };
            if newer {
                out.set("method", format!("worktree({why} — 측정 당시 개정 대조 불가 · mtime {mt_s} > 측정 {measured}({msrc}))"));
                out.set("warning", format!("측정({measured}) 뒤 mtime {mt_s} — 측정 전 개정을 찾을 수 없다({why}) · 실린 작업트리 바이트가 측정 당시 바이트라는 보장 없음"));
            } else {
                set_all(&mut out, vec![
                    ("basis", J::from("mtime≤measured")),
                    ("verified", J::Bool(false)),
                    ("method", J::from(format!("worktree(mtime {mt_s} ≤ 측정 {measured}({msrc}) — 측정 당시 바이트 후보 · {why} · 실행 바이트 미검증)"))),
                ]);
            }
        }
        Some(measured) => {
            let rel = rel.clone().unwrap();
            let rev = _git_rev_before(c, &rel, Some(measured))?;
            let rev_bytes = match &rev {
                Some(r) => core::git_bytes(&c.repo, &["show", &format!("{r}:{rel}")], false)?,
                None => None,
            };
            let after_flag = format!("--after={measured}");
            let lg = core::git_nocheck(&c.repo, &["-c", "log.showSignature=false", "log", &after_flag, "--format=%H", "--", &rel])?;
            let after: Vec<String> =
                if lg.returncode == 0 { py_split(&lg.stdout).into_iter().filter(|s| is_sha40(s)).map(str::to_string).collect() } else { vec![] };
            let ls = core::git_nocheck(&c.repo, &["-c", "core.quotePath=false", "ls-tree", "HEAD", "--", &rel])?;
            let parts: Vec<String> = if ls.returncode == 0 { py_split(&ls.stdout).into_iter().map(str::to_string).collect() } else { vec![] };
            let head_blob = if parts.len() >= 3 && parts[1] == "blob" { Some(parts[2].clone()) } else { None };
            let wt_blob = git_blob_sha(&wt);
            let mut c0 = format!("측정 뒤 커밋 {}", after.len());
            if !after.is_empty() {
                c0.push_str(&format!("({})", after.iter().take(5).map(|s| head(s, 12)).collect::<Vec<_>>().join(", ")));
            }
            let c1 = if head_blob.as_deref() == Some(wt_blob.as_str()) {
                "작업트리 = HEAD"
            } else if head_blob.is_some() {
                "작업트리 수정본(HEAD 대비 미커밋)"
            } else {
                "HEAD 에 없음(비추적)"
            };
            let cross = vec![c0, c1.to_string()];
            let cross_j = J::from(cross.clone());
            out.set("commits_after_measurement", J::from(after.iter().map(|s| head(s, 12)).collect::<Vec<_>>()));
            out.set("cross_check", cross_j);
            let hd = format!("git log -1 --before={measured} -- {rel}");
            let crossj = cross.join(" · ");
            if !newer {
                set_all(&mut out, vec![("basis", J::from("mtime≤measured")), ("verified", J::Bool(false))]);
                match (&rev, &rev_bytes) {
                    (Some(r), Some(rb)) if *rb == wt => set_all(&mut out, vec![
                        ("commit", J::from(r.as_str())),
                        ("method", J::from(format!("worktree(mtime {mt_s} ≤ 측정 {measured}({msrc}) · 바이트 = 측정 전 마지막 커밋 {})", head(r, 12)))),
                    ]),
                    (Some(r), _) => out.set(
                        "method",
                        format!("worktree(mtime {mt_s} ≤ 측정 {measured}({msrc}) · 측정 전 마지막 커밋 {} 와 다르다 — 측정 당시 바이트는 미검증)", head(r, 12)),
                    ),
                    (None, _) => out.set("method", format!("worktree(mtime {mt_s} ≤ 측정 {measured}({msrc}) · 측정 전 커밋 없음)")),
                }
            } else if let (Some(r), Some(rb)) = (&rev, &rev_bytes) {
                if *rb == wt {
                    set_all(&mut out, vec![
                        ("commit", J::from(r.as_str())),
                        ("basis", J::from("git-rev-before-measurement")),
                        ("method", J::from(format!("worktree({hd} → {} · 작업트리 mtime {mt_s} > 측정({msrc})이지만 바이트가 그 개정과 같다 · {crossj})", head(r, 12)))),
                    ]);
                } else {
                    set_all(&mut out, vec![
                        ("bytes", _bytes_j(rb)),
                        ("shipped", J::from(format!("git:{r}"))),
                        ("commit", J::from(r.as_str())),
                        ("basis", J::from("git-rev-before-measurement")),
                        ("method", J::from(format!(
                            "{hd} → {} · 작업트리 mtime {mt_s} > 측정 {measured}({msrc}) · {crossj} — 측정 전 마지막 커밋의 바이트를 싣는다(git show · 측정 당시 미커밋 수정이 있었는지는 관측 불가)",
                            head(r, 12)
                        ))),
                    ]);
                }
            } else {
                out.set("method", format!("worktree({hd} 없음(측정 전 커밋 없음 · 비추적 또는 측정 뒤 첫 커밋) · mtime {mt_s} > 측정 {measured}({msrc}) · {crossj})"));
                out.set("warning", format!("측정({measured}) 뒤 mtime {mt_s} · 측정 전 개정 없음 — 실린 작업트리 바이트가 측정 당시 바이트라는 보장 없음"));
            }
        }
    }
    c.memo_set(&key, out.clone());
    Ok(out)
}

pub fn _repo_rel(c: &_Ctx, path: &Path) -> HR<Option<String>> {
    let first = if path.is_absolute() { path.to_path_buf() } else { c.repo.join(path) };
    let second = core::resolve(&first);
    for cand in [first, second] {
        if let Ok(r) = cand.strip_prefix(&c.repo) {
            let s = core::posix(r);
            return Ok(Some(if s.is_empty() { ".".to_string() } else { s }));
        }
    }
    Ok(None)
}

pub fn _tracked_row(sel: &J) -> HR<J> {
    let mut m = Map::new();
    for k in _TRACKED_KEYS {
        if let Some(v) = sel.get(k) {
            m.insert(k, v.clone());
        }
    }
    Ok(J::Dict(m))
}

pub fn _compose_text(c: &_Ctx) -> HR<String> {
    let Some(p) = &c.compose_path else { return Ok(String::new()) };
    let sel = _select_tracked(c, p)?;
    Ok(lossy(&_j_bytes(&sel["bytes"]).unwrap_or_default()))
}

pub fn _compose_file(c: &_Ctx) -> HR<Option<PathBuf>> {
    Ok(c.compose_eff.borrow().clone().or_else(|| c.compose_path.clone()))
}

pub fn _renderer_revision(c: &_Ctx) -> HR<Option<J>> {
    // the renderer's revision = its Rust source (the launcher only starts the binary built from it · plan_26111615 D4)
    let p = c.repo.join(core::REL_RENDER_DOCKERFILE_SRC);
    if p.is_file() {
        Ok(Some(_select_tracked(c, &p)?))
    } else {
        Ok(None)
    }
}

pub fn _renderer_drift_lines(rsel: &J) -> HR<Vec<String>> {
    if !rsel.truthy() {
        return Ok(vec![]);
    }
    let shipped = rsel["shipped"].clone();
    let shipped_s = if shipped.truthy() { shipped.display() } else { String::new() };
    if !shipped_s.starts_with("git:") {
        return Ok(vec![]);
    }
    let rev12 = head(&rsel["commit"].display(), 12);
    let after: Vec<String> = if rsel["commits_after_measurement"].truthy() { rsel["commits_after_measurement"].items().iter().map(J::display).collect() } else { vec![] };
    let path = rsel["path"].display();
    let mut l0 = format!("# ⚠ 측정 당시 렌더러 ≠ 작업트리 렌더러: {path} 측정 전 마지막 커밋 {rev12} · 측정 뒤 커밋 {}건", after.len());
    if !after.is_empty() {
        l0.push_str(&format!("({}{})", after.iter().take(5).cloned().collect::<Vec<_>>().join(", "), if after.len() > 5 { " …" } else { "" }));
    }
    l0.push_str(" — 아래 명령을 지금 돌리면 측정 당시와 다른 env 형상·러너가 나올 수 있다");
    Ok(vec![l0, format!("# 측정 당시 env 값 = {ENV_OBSERVED_POINTER}(측정 엔진 로그 관측) · 측정 당시 렌더러 원문 = git show {rev12}:{path}")])
}

// ── 평면 · 빌드 선택자 ───────────────────────────────────────────────────────────────────────
fn env_truthy(c: &_Ctx, k: &str) -> bool {
    c.env.get(k).is_some_and(J::truthy)
}

pub fn _plane(c: &_Ctx) -> HR<(String, String)> {
    let p = g(&c.ev, "plane");
    let mut declared = py_strip(&if p.truthy() { p.display() } else { String::new() }).to_lowercase();
    if declared == "container" {
        declared = "docker".to_string();
    }
    let build = g(&c.ev, "build_identity");
    let mut observed: Option<(&str, &str)> = None;
    if env_truthy(c, "IMAGE_TAG") || env_truthy(c, "BUILD_DOCKERFILE") {
        observed = Some(("docker", "cell-env(IMAGE_TAG|BUILD_DOCKERFILE)"));
    } else if build.is_dict() && (build["image_tag"].truthy() || build["dockerfile"].truthy()) && !build["native_wheelhouse_source"].truthy() {
        observed = Some(("docker", "evidence.build_identity(image_tag|dockerfile)"));
    }
    if !declared.is_empty() && declared != "docker" && declared != "native" {
        return core::fail("HINT_PLANE_UNKNOWN", format!("평면 어휘 밖: {}", crate::value::py_repr(&declared)), Some("docker|native(container=docker) 중 하나."));
    }
    if let Some((o, src)) = observed {
        if !declared.is_empty() && o != declared {
            return core::fail(
                "HINT_PLANE_MISMATCH",
                format!("선언 plane={declared} 와 관측 plane={o}({src}) 가 다르다."),
                Some("셀 env 의 이미지 선택자와 evidence 의 plane 중 어느 쪽이 틀렸는지 확인한다."),
            );
        }
        return Ok((o.to_string(), src.to_string()));
    }
    if !declared.is_empty() {
        return Ok((declared, "evidence.plane(declared)".to_string()));
    }
    core::fail(
        "HINT_PLANE_UNDERIVABLE",
        "실행 평면을 파생할 신호가 없다(셀 env 에 IMAGE_TAG·BUILD_DOCKERFILE 없음 · evidence.plane 없음).",
        Some("docker 셀이면 셀 env 에 이미지 선택자(IMAGE_TAG·BUILD_DOCKERFILE)를 둔다. native 셀이면 native serve proof(plane=native · native_multinode_serve.py 산출)가 있어야 evidence 가 `plane` 을 넘긴다(2026-09-23 plan_26092311 배선). '신호 없음 = native' 로 추측하지 않는다."),
    )
}

pub fn plane_of(repo: &Path, ev: &J, forward_module: &J) -> HR<String> {
    Ok(plane_and_source(repo, ev, forward_module)?.0)
}

pub fn plane_and_source(repo: &Path, ev: &J, forward_module: &J) -> HR<(String, String)> {
    _plane(&_ctx(repo, ev, &J::Null, forward_module, &J::Null)?)
}

pub fn _dockerfile_args(c: &_Ctx, text: &str) -> HR<Vec<String>> {
    let rd = c.render()?;
    let mut v: Vec<String> = rd.ledger_build_args(text)?.into_iter().collect::<BTreeSet<_>>().into_iter().collect();
    v.sort();
    Ok(v)
}

pub fn _dockerfile_copies(text: &str) -> HR<Vec<String>> {
    let mut out: Vec<String> = Vec::new();
    for m in _DOCKERFILE_COPY().finditer(text) {
        let is_add = pyre::group(&m, 1).unwrap_or("").to_uppercase() == "ADD";
        let g2 = pyre::group(&m, 2).unwrap_or("");
        let body = py_strip(g2.split('#').next().unwrap_or("")).to_string();
        let toks: Vec<String> = if body.starts_with('[') {
            match crate::value::loads(&body) {
                Ok(J::List(items)) => items.iter().map(J::display).collect(),
                Ok(other) => other.items().iter().map(J::display).collect(),
                Err(_) => return core::fail("HINT_DOCKERFILE_COPY_UNPARSED", format!("COPY JSON 형식을 읽지 못했다: {}", head(&body, 80)), None),
            }
        } else {
            py_split(&body).into_iter().map(str::to_string).collect()
        };
        if toks.iter().any(|t| t.starts_with("--from")) {
            continue;
        }
        let toks: Vec<String> = toks.into_iter().filter(|t| !t.starts_with("--")).collect();
        let n = toks.len().saturating_sub(1);
        for src in &toks[..n] {
            if is_add && (src.contains("://") || src.starts_with("git@")) {
                continue;
            }
            let s = src.trim_end_matches('/').to_string();
            if !s.is_empty() && !out.contains(&s) {
                out.push(s);
            }
        }
    }
    Ok(out)
}

pub fn _image_tag(c: &_Ctx) -> HR<(Option<String>, String)> {
    if env_truthy(c, "IMAGE_TAG") {
        return Ok((Some(c.env.get("IMAGE_TAG").map(J::display).unwrap_or_default()), "cell-env:IMAGE_TAG".to_string()));
    }
    let build = g(&c.ev, "build_identity");
    if build.is_dict() && build["image_tag"].truthy() {
        return Ok((Some(build["image_tag"].display()), "evidence.build_identity.image_tag".to_string()));
    }
    if let Some(cp) = &c.compose_path {
        if let Some(d) = c.sf("compose_default")?.compose_default(cp, "IMAGE_TAG")?.filter(|d| !d.is_empty()) {
            return Ok((Some(d), "compose-default:IMAGE_TAG".to_string()));
        }
    }
    Ok((None, "unobserved".to_string()))
}

pub fn _build_selection(c: &_Ctx, ledger: &J) -> HR<J> {
    let sel_ledger = if ledger.is_dict() && ledger["dockerfile"].truthy() { ledger["dockerfile"].display() } else { String::new() };
    let sel_env = c.env.get("BUILD_DOCKERFILE").map(J::display).unwrap_or_default();
    let compose_df = match &c.compose_path {
        Some(cp) => c.sf("compose_default")?.compose_default(cp, "BUILD_DOCKERFILE")?,
        None => None,
    };
    if !sel_ledger.is_empty() && !sel_env.is_empty() && sel_ledger != sel_env {
        return core::fail(
            "HINT_BUILD_SELECTOR_MISMATCH",
            format!(
                "이미지 원장 dockerfile={} 와 셀 env BUILD_DOCKERFILE={} 가 다르다.",
                crate::value::py_repr(&sel_ledger),
                crate::value::py_repr(&sel_env)
            ),
            Some("셀 env 가 가리키는 레시피로 이미지를 다시 짓거나 셀 env 를 원장에 맞춘다(태그는 가변 포인터)."),
        );
    }
    let bi = {
        let b = g(&c.ev, "build_identity");
        if b.truthy() { b } else { J::dict() }
    };
    let src = if bi.is_dict() && bi["source"].truthy() { bi["source"].clone() } else { J::dict() };
    let bi_src = if src["dockerfile"].truthy() { src["dockerfile"].display() } else { String::new() };
    let sel_hist = if bi_src.contains(crate::evidence::HISTORY_TRACK_MARK) && bi["dockerfile"].truthy() { bi["dockerfile"].display() } else { String::new() };
    let image_hist_src = format!("image-history:{bi_src}");
    let mut chosen: Option<(String, String)> = None;
    for (val, s) in [
        (sel_ledger.clone(), "build-ledger.dockerfile".to_string()),
        (sel_env.clone(), "cell-env:BUILD_DOCKERFILE".to_string()),
        (sel_hist, image_hist_src),
        (compose_df.clone().unwrap_or_default(), "compose-default:BUILD_DOCKERFILE".to_string()),
    ] {
        if !val.is_empty() {
            chosen = Some((val, s));
            break;
        }
    }
    let Some((dockerfile, df_source)) = chosen else {
        return core::fail(
            "HINT_BUILD_SELECTOR_UNDERIVABLE",
            format!(
                "쓰인 Dockerfile 을 파생하지 못했다(원장·셀 env·이미지 history build-arg 관측·compose 기본값 모두 없음 · 관측 기록: {}).",
                if bi_src.is_empty() { "미관측" } else { &bi_src }
            ),
            Some("셀 env 에 BUILD_DOCKERFILE 을 적거나 compose 의 `dockerfile: ${BUILD_DOCKERFILE:-…}` 기본값을 확인한다 · 이미지 탐침(docker history)이 가능한 곳에서 다시 실행한다."),
        );
    };
    if dockerfile.contains('/') || dockerfile.starts_with('.') {
        return core::fail(
            "HINT_BUILD_SELECTOR_INVALID",
            format!("Dockerfile 선택자는 빌드 컨텍스트 안 파일 이름이어야 한다: {}", crate::value::py_repr(&dockerfile)),
            None,
        );
    }
    let (image, image_source) = _image_tag(c)?;
    let track = if dockerfile == "Dockerfile" { "wheel" } else { "source-build" };
    if ledger.is_dict() && ledger["track"].truthy() && ledger["track"] != J::from(track) {
        return core::fail(
            "HINT_BUILD_TRACK_MISMATCH",
            format!("원장 track={} 와 Dockerfile 파생 track={} 가 다르다.", ledger["track"].repr(), crate::value::py_repr(track)),
            None,
        );
    }
    Ok(jd! {
        "dockerfile" => dockerfile.as_str(), "dockerfile_source" => df_source, "track" => track,
        "image_tag" => J::from(image.filter(|x| !x.is_empty())), "image_tag_source" => image_source,
    })
}

// ── 원장 관측 ───────────────────────────────────────────────────────────────────────────────
/// hintlib/artifacts.py:1387 _ROLE_KEYS (python literal as JSON)
pub fn _ROLE_KEYS() -> &'static J {
    static CELL: std::sync::OnceLock<J> = std::sync::OnceLock::new();
    CELL.get_or_init(|| crate::value::loads("{\"main\": \"main\", \"master\": \"main\", \"sub\": \"sub\", \"slave\": \"sub\"}").expect("constant"))
}

pub fn _ledgers_in_attestation(att: &J) -> HR<(J, i64)> {
    let mut out = Map::new();
    let mut dropped = 0i64;
    if let Some(nodes) = att.get("nodes").and_then(J::as_dict) {
        let mut items: Vec<(&str, &J)> = nodes.iter().collect();
        items.sort_by(|a, b| a.0.cmp(b.0));
        for (n, doc) in items {
            let Some(role) = _ROLE_KEYS().get_str(n) else {
                dropped += 1;
                continue;
            };
            if doc.is_dict() && doc["build_ledger"].is_dict() && !out.contains_key(role) {
                out.insert(role, doc["build_ledger"].clone());
            }
        }
    }
    Ok((J::Dict(out), dropped))
}

pub fn _attestation_ref(c: &_Ctx) -> HR<String> {
    let att = g(&c.ev, "attestation");
    if att.is_dict() {
        if let Some(p) = att["_path"].as_str().map(py_strip).filter(|p| !p.is_empty()) {
            return Ok(p.to_string());
        }
        if let Some(s) = att["_source"].as_str().filter(|s| !py_strip(s).is_empty()) {
            return Ok(s.split(' ').next().unwrap_or("").to_string());
        }
    }
    let seeds = g(&c.ev, "lineage_seeds");
    if let Some(r) = seeds.get("attestation").and_then(J::as_str).filter(|r| !r.is_empty()) {
        return Ok(r.to_string());
    }
    Ok(format!("output/{}/benchlog/attestation_{}.json{}", c.topo, c.cell, if att.is_dict() { "" } else { "(부재)" }))
}

pub fn _ledger_vllm_view(c: &_Ctx) -> HR<Option<J>> {
    let Some((applied, _sel, ledger, _per)) = applied_memo(c, true).or_else(|| applied_memo(c, false)) else { return Ok(None) };
    let ledger = ledger.unwrap_or(J::Null);
    if !ledger.is_dict() || !ledger["vllm"].is_dict() {
        return Ok(None);
    }
    let mut v = Map::new();
    for (k, x) in ledger["vllm"].as_dict().unwrap().iter() {
        if matches!(k, "dist_version" | "git_sha" | "describe") {
            v.insert(k, x.clone());
        }
    }
    let src = if applied.truthy() { applied.get("source").cloned().unwrap_or(J::Null) } else { J::Null };
    Ok(Some(jd! {"vllm" => J::Dict(v), "source" => src, "provenance" => ledger.get("provenance").cloned().unwrap_or(J::Null)}))
}

re_static!(digest_re, r"sha256:[0-9a-f]{64}");

/// `_observe_ledger(c)` → (main ledger, source, per-node ledgers, probe records).
pub fn _observe_ledger(c: &_Ctx) -> HR<(Option<J>, Option<String>, J, Vec<J>)> {
    let mut probes: Vec<J> = Vec::new();
    let att = g(&c.ev, "attestation");
    let mut per_node = J::dict();
    let r#ref = _attestation_ref(c)?;
    if att.is_dict() {
        let (pn, dropped) = _ledgers_in_attestation(&att)?;
        per_node = pn;
        let mut row = jd! {"tier" => "attestation", "ref" => r#ref.as_str(),
            "result" => if per_node.truthy() { "observed" } else { "no-ledger(nodes.{main,sub}.build_ledger 없음 — 옛 이미지 또는 v1)" }};
        if dropped != 0 {
            row.set("ignored_node_keys", dropped);
        }
        probes.push(row);
    } else {
        probes.push(jd! {"tier" => "attestation", "ref" => r#ref.as_str(), "result" => "absent"});
    }
    let want = if c.node == "sub" { "sub" } else { "main" };
    if let Some(l) = per_node.get(want) {
        return Ok((Some(l.clone()), Some(format!("attestation:{}#{want}", r#ref)), per_node.clone(), probes));
    }
    if per_node.truthy() {
        let mut roles: Vec<String> = per_node.as_dict().unwrap().keys().map(str::to_string).collect();
        roles.sort();
        let last = probes.last_mut().unwrap();
        last.set("result", format!("partial(원장 역할 {} — {want} 원장 없음 · 대리하지 않는다)", J::from(roles).repr()));
    }
    if c.node == "sub" {
        let bi = g(&c.ev, "build_identity");
        let d = bi.get("image_digest").cloned().unwrap_or(J::Null);
        let digest = if d.truthy() { d.display() } else { String::new() };
        if _topology(&c.ev)? != "single" || !_local_image_owner(&c.repo, &c.ev)? || !digest_re().is_full(&digest) {
            probes.push(jd! {"tier" => "local-image", "result" => "skipped(node=sub — 로컬 소유·측정 digest 미확인 · 문서기반 회수만)"});
            return Ok((None, None, per_node, probes));
        }
    } else if c.node == "cluster" && !_local_image_owner(&c.repo, &c.ev)? {
        probes.push(jd! {"tier" => "local-image", "result" => "skipped(cluster — 로컬 main 소유 미확인)"});
        return Ok((None, None, per_node, probes));
    } else if c.node != "sub" && c.node != "cluster" && !_local_image_owner(&c.repo, &c.ev)? {
        probes.push(jd! {"tier" => "local-image", "result" => "skipped(로컬 노드 소유 미확인)"});
        return Ok((None, None, per_node, probes));
    }
    let (image, image_src) = _image_ref(c)?;
    let Some(image) = image else {
        probes.push(jd! {"tier" => "local-image", "result" => format!("skipped({image_src})")});
        return Ok((None, None, per_node, probes));
    };
    let (cls, _) = c.docker(&["image", "inspect", "--format", "{{.Id}}", &image], true);
    if cls != "ok" {
        probes.push(jd! {"tier" => "local-image", "image" => image.as_str(), "image_source" => image_src.as_str(),
            "result" => if cls.starts_with("rc=") { "image-absent-local(측정 이미지가 로컬에 없다 — 태그로 대체하지 않는다)".to_string() } else { cls.clone() }});
        return Ok((None, None, per_node, probes));
    }
    let who = if c.node == "sub" { "sub" } else { "main" };
    let source = format!("docker-image({who}-local):{image}:{LEDGER_IN_IMAGE} · 빌드 시 원장(후속 이미지 층은 별도 관측)");
    if _probe_allowed(c)?.0 {
        let (data, got) = _cp_from_image(c, &image, LEDGER_IN_IMAGE)?;
        let data = data.and_then(|d| _j_bytes(&d));
        let Some(data) = data else {
            let rm_note = got.split_once(" · ").map(|x| x.1.to_string()).unwrap_or_default();
            let mut result = if got.starts_with("absent") { format!("no-ledger(이미지에 {LEDGER_IN_IMAGE} 없음 — docker cp)") } else { format!("docker-cp-failed({got})") };
            if !rm_note.is_empty() && got.starts_with("absent") {
                result.push_str(&format!(" · ⚠ {rm_note}"));
            }
            probes.push(jd! {"tier" => "local-image", "image" => image.as_str(), "image_source" => image_src.as_str(), "method" => "docker create+cp(시작 ✗)", "result" => result});
            return Ok((None, None, per_node, probes));
        };
        let doc = String::from_utf8(data).ok().and_then(|t| crate::value::loads(&t).ok());
        let Some(doc) = doc.filter(J::is_dict) else {
            return core::fail(
                "HINT_BUILD_LEDGER_UNREADABLE",
                format!("이미지 원장이 JSON 객체가 아니다: {image}:{LEDGER_IN_IMAGE}"),
                Some("upstream 빌드 원장 스탠자(source-build.md §4.1)를 확인한다."),
            );
        };
        probes.push(jd! {"tier" => "local-image", "image" => image.as_str(), "image_source" => image_src.as_str(), "method" => "docker create+cp(시작 ✗)",
            "result" => if got == "ok" { "observed".to_string() } else { format!("observed({got})") }});
        return Ok((Some(doc), Some(source), per_node, probes));
    }
    if c.node == "sub" {
        probes.push(jd! {"tier" => "local-image", "image" => image.as_str(), "image_source" => image_src.as_str(),
            "result" => "skipped(local sub requires create/cp/rm probe capability; no image execution)"});
        return Ok((None, None, per_node, probes));
    }
    let (cls, proc) = c.docker(&["run", "--rm", "--network", "none", "--pull", "never", "--entrypoint", "cat", &image, LEDGER_IN_IMAGE], true);
    let proc = match (cls.as_str(), proc) {
        ("ok", Some(p)) => p,
        (cls, _) => {
            let failed = matches!(cls, "rc=125" | "rc=126" | "rc=127") || !cls.starts_with("rc=");
            probes.push(jd! {"tier" => "local-image", "image" => image.as_str(), "image_source" => image_src.as_str(),
                "result" => if failed { format!("docker-run-failed({cls})") } else { format!("no-ledger(cat {cls})") }});
            return Ok((None, None, per_node, probes));
        }
    };
    let Some(doc) = crate::value::loads(&proc.stdout).ok().filter(J::is_dict) else {
        return core::fail(
            "HINT_BUILD_LEDGER_UNREADABLE",
            format!("이미지 원장이 JSON 객체가 아니다: {image}:{LEDGER_IN_IMAGE}"),
            Some("upstream 빌드 원장 스탠자(source-build.md §4.1)를 확인한다."),
        );
    };
    probes.push(jd! {"tier" => "local-image", "image" => image.as_str(), "image_source" => image_src.as_str(), "result" => "observed"});
    Ok((Some(doc), Some(source), per_node, probes))
}
