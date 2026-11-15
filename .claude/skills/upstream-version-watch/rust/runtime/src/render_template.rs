//! `render_dockerfile.py`, unit B: the template render (`--template`/`--resolved`, `--canonical-kind`/`--topology`) with the source-build key
//! guard, the build-ledger stanza, the Flag gate and the COPY-context directories. `--self-test` stays in Python (exit status 2, see
//! `render_dockerfile::run`).
//!
//! The ledger body (`_LEDGER_PY` of the original) is not run here: it is the byte-for-byte text of the original literal
//! (`render_ledger_body.txt`) with its two `__EASY_VLLM_LEDGER_*__` tokens replaced by `json.dumps` text.

use crate::gate_cli::{self, Values};
use crate::render_dockerfile::{json_string, key_error, os_error, py_repr_str, type_name, value_error, Raised, V};
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::PyValue;
use agent_runtime_support::pyyaml::safe_load;
use agent_runtime_support::pyjson;
use role_contract::manifest_gate::{evaluate_contract_value, GateFailure};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const IMAGE_NAME: &str = "easy-vllm";
const LEDGER_BODY: &str = include_str!("render_ledger_body.txt");

/// tripwire: the closed list of (NGC base, vLLM version) pairs a build and smoke have passed for (the original's `VALIDATED_SOURCE_BUILD_KEYS`).
const VALIDATED_SOURCE_BUILD_KEYS: [(&str, &str); 8] = [
    ("26.03-py3", "0.22.1"),
    ("26.05-py3", "0.23.0"),
    ("26.05-py3", "0.24.0"),
    ("26.05-py3", "0.25.1"),
    ("26.05-py3", "0.26.0"),
    ("26.07-py3", "0.27.0"),
    ("26.07-py3", "0.31.0"),
    ("26.07-py3", "0.32.0"),
];

/// One hand-written entry of `INHERITED_SOURCE_BUILD_KEYS` (a human adds a line at gate G1.6; the ledger is empty on purpose).
struct InheritEntry {
    key: (&'static str, &'static str),
    inherits: (&'static str, &'static str),
    attestation: &'static str,
    approved_by: &'static str,
    approved_kst: &'static str,
}
const INHERITED_SOURCE_BUILD_KEYS: &[InheritEntry] = &[];
const INHERIT_ATTESTATION_SUFFIX: &str = "#upstream_delta";

fn attr_error<T>(v: &V) -> Result<T, Raised> {
    Err(Raised { class: "AttributeError", message: format!("'{}' object has no attribute 'get'", type_name(v)) })
}

/// `v.get(k1, {}).get(k2, {})....get(last)`: an absent intermediate is an empty dict, a present non-dict one raises.
fn chain<'a>(v: &'a V, keys: &[&str]) -> Result<Option<&'a V>, Raised> {
    let mut cur = v;
    for (i, key) in keys.iter().enumerate() {
        if !matches!(cur, V::Dict(_)) {
            return attr_error(cur);
        }
        match cur.get(key) {
            None => return Ok(None),
            Some(next) => {
                if i + 1 == keys.len() {
                    return Ok(Some(next));
                }
                cur = next;
            }
        }
    }
    Ok(Some(cur))
}

fn str_or(v: Option<&V>, default: &str) -> String {
    v.map_or_else(|| default.to_string(), V::py_str)
}

/// `str(x or "")`
fn str_truthy(v: Option<&V>) -> String {
    match v {
        Some(x) if x.truthy() => x.py_str(),
        _ => String::new(),
    }
}

fn opt_repr(v: Option<&V>) -> String {
    v.map_or_else(|| "None".to_string(), V::py_repr)
}

fn echo_safe(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ").replace('"', "'").replace('\\', "/")
}

fn norm_ref(v: Option<&V>) -> String {
    let r = str_truthy(v);
    let r = r.trim();
    r.strip_prefix('v').unwrap_or(r).to_string()
}

fn py_list_repr(items: &[String]) -> String {
    format!("[{}]", items.iter().map(|s| py_repr_str(s)).collect::<Vec<_>>().join(", "))
}

fn is_int_one(v: Option<&V>) -> bool {
    match v {
        Some(V::Int(n)) => n == "1",
        Some(V::Bool(b)) => *b,
        Some(V::Float(f)) => *f == 1.0,
        _ => false,
    }
}

type Checks = Vec<(&'static str, bool, String)>;

/// `_inherit_eligibility`: (ok, checks, attestation)
fn inherit_eligibility<'a>(ngc_tag: &str, vllm_version: &str, entry: &InheritEntry, resolved: &'a V) -> (bool, Checks, Option<&'a V>) {
    let mut checks: Checks = Vec::new();
    let mut ck = |name: &'static str, ok: bool, detail: String| {
        checks.push((name, ok, detail));
    };
    let mut missing = Vec::new();
    for (name, empty) in [
        ("inherits", false),
        ("attestation", entry.attestation.is_empty()),
        ("approved_by", entry.approved_by.is_empty()),
        ("approved_kst", entry.approved_kst.is_empty()),
    ] {
        if empty {
            missing.push(name.to_string());
        }
    }
    ck("entry_shape", missing.is_empty(), format!("필수 필드 결손: {}", py_list_repr(&missing)));
    ck("base_shape", true, String::new());
    ck(
        "base_same_ngc",
        entry.inherits.0 == ngc_tag,
        format!("상속원 NGC({}) != 대상 NGC({ngc_tag}) — 출구①은 **같은 NGC 베이스** 위의 vLLM bump 만 덮는다", entry.inherits.0),
    );
    ck(
        "base_validated",
        VALIDATED_SOURCE_BUILD_KEYS.contains(&entry.inherits),
        format!("상속원 ('{}', '{}') 가 VALIDATED_SOURCE_BUILD_KEYS 에 없음 — 상속의 상속(체인) 금지", entry.inherits.0, entry.inherits.1),
    );
    let ptr = entry.attestation;
    ck(
        "attestation_pointer",
        ptr.ends_with(INHERIT_ATTESTATION_SUFFIX) && ptr.chars().count() > INHERIT_ATTESTATION_SUFFIX.chars().count(),
        format!("attestation 포인터가 '<경로>{INHERIT_ATTESTATION_SUFFIX}' 형식이 아님: {}", py_repr_str(ptr)),
    );
    let att = resolved.get("upstream_delta").filter(|a| matches!(a, V::Dict(d) if !d.is_empty()));
    let Some(att) = att else {
        ck(
            "attestation_present",
            false,
            "렌더 입력(resolved)에 'upstream_delta' 블록이 없다 — judge_version_delta.py --write-resolved 로 발행하지 않았거나 다른 resolved 를 렌더 중이다".into(),
        );
        return (false, checks, None);
    };
    ck("attestation_present", true, String::new());
    ck(
        "attestation_schema",
        is_int_one(att.get("schema_version")),
        format!("attestation schema_version={} 이 승인목록 [1] 밖", opt_repr(att.get("schema_version"))),
    );
    let prov = att.get("provenance");
    ck(
        "attestation_provenance",
        matches!(prov, Some(V::Str(s)) if s == "measured"),
        format!("provenance={} (measured 만 상속 근거가 된다 — 모의·dry-run 금지)", opt_repr(prov)),
    );
    let to_ok = norm_ref(att.get("to_ref")) == vllm_version;
    let from_ok = norm_ref(att.get("from_ref")) == entry.inherits.1;
    ck(
        "attestation_refs",
        to_ok && from_ok,
        format!(
            "attestation {}->{} 이 상속 주장 {}->{vllm_version} 와 불일치",
            opt_repr(att.get("from_ref")),
            opt_repr(att.get("to_ref")),
            entry.inherits.1
        ),
    );
    let verdict_of = |key: &str| -> Option<&V> { att.get(key).filter(|b| b.truthy()).and_then(|b| b.get("verdict")) };
    let ax_a = verdict_of("axis_A_build_input");
    ck("axis_A_no_impact", matches!(ax_a, Some(V::Str(s)) if s == "NO_IMPACT"), format!("axis_A={} (NO_IMPACT 필요)", opt_repr(ax_a)));
    let ax_b_block = att.get("axis_B_port_scope").filter(|b| b.truthy());
    let ax_b = ax_b_block.and_then(|b| b.get("verdict"));
    let srr = ax_b_block.and_then(|b| b.get("silent_revert_risk"));
    ck(
        "axis_B_no_silent_revert",
        matches!(srr, Some(V::List(l)) if l.is_empty()),
        format!(
            "axis_B.silent_revert_risk={} — 빈 리스트여야 한다(axis_B.verdict={}; WOULD_REVERT/UNDETERMINED 프로브가 남아 있거나 필드가 없다)",
            opt_repr(srr),
            opt_repr(ax_b)
        ),
    );
    let gv = att.get("verdict");
    ck(
        "global_verdict",
        matches!(gv, Some(V::Str(s)) if s == "NO_IMPACT" || s == "IMPACT"),
        format!("전역 verdict={} — UNDETERMINED/미지 는 상속 근거가 아니다(미판정이 무영향으로 세탁되는 것을 막는다)", opt_repr(gv)),
    );
    let unk = att.get("unknown");
    ck("unknown_empty", matches!(unk, Some(V::List(l)) if l.is_empty()), format!("unknown={} (비어 있어야 한다)", opt_repr(unk)));
    let _ = entry.key;
    (checks.iter().all(|(_, ok, _)| *ok), checks, Some(att))
}

fn patch_guard(ngc_tag: &str, vllm: &str, resolved: &V, allow_unvalidated: bool) -> String {
    let key = (ngc_tag, vllm);
    if VALIDATED_SOURCE_BUILD_KEYS.contains(&key) {
        return format!(
            "RUN echo \"[guard] source-build key ({ngc_tag} x vLLM {vllm}): validated patch set ((ngc_base x vllm_version) keyed) — proceeding.\""
        );
    }
    if let Some(entry) = INHERITED_SOURCE_BUILD_KEYS.iter().find(|e| e.key == key) {
        let (ok, checks, att) = inherit_eligibility(ngc_tag, vllm, entry, resolved);
        let base_s = format!("('{}', '{}')", entry.inherits.0, entry.inherits.1);
        if let (true, Some(att)) = (ok, att) {
            let ax_c = att.get("axis_C_model_path").filter(|b| b.truthy()).and_then(|b| b.get("verdict"));
            let axa = att.get("axis_A_build_input").filter(|b| b.truthy()).and_then(|b| b.get("verdict"));
            let axb = att.get("axis_B_port_scope").filter(|b| b.truthy());
            let srr_len = match axb.and_then(|b| b.get("silent_revert_risk")) {
                Some(V::List(l)) => l.len(),
                _ => 0,
            };
            let unk_len = match att.get("unknown") {
                Some(V::List(l)) => l.len(),
                _ => 0,
            };
            let basis = format!(
                "axis_A={} axis_B={}(silent_revert_risk={srr_len}) unknown={unk_len}",
                axa.map_or("None".to_string(), V::py_str),
                axb.and_then(|b| b.get("verdict")).map_or("None".to_string(), V::py_str)
            );
            let es = |v: Option<&V>| echo_safe(&v.map_or("None".to_string(), V::py_str));
            return format!(
                "RUN echo \"[guard] source-build key ({ngc_tag} x vLLM {vllm}): INHERITED from {}.\" && \\\n    echo \"  -> basis: upstream_delta {} verdict={} {}\" && \\\n    echo \"  -> attestation: {} (judge_version_delta.py · provenance={} · delta_source={})\" && \\\n    echo \"  -> approved_by: {} @ {} (HITL 게이트 G1.6 — 이 항목은 사람이 손으로 등재했다. Judge 는 가드를 넓히지 않는다)\" && \\\n    echo \"  -> scope: 상속되는 것은 **빌드 키**(패치 셋 적용가능성)뿐이다. 기능 판정 아님 — axis_C={}.\" && \\\n    echo \"  -> arbiter = smoke (workflow S3 · HITL 게이트 ③). 상속은 스모크를 면제하지 않는다.\" && \\\n    echo \"  -> 출구① 계약: .claude/skills/upstream-version-watch/references/source-build.md §3.1\"",
                echo_safe(&base_s),
                echo_safe(&format!("{}->{}", opt_repr_py(att.get("from_ref")), opt_repr_py(att.get("to_ref")))),
                es(att.get("verdict")),
                echo_safe(&basis),
                echo_safe(entry.attestation),
                es(att.get("provenance")),
                es(att.get("delta_source")),
                echo_safe(entry.approved_by),
                echo_safe(entry.approved_kst),
                es(ax_c),
            );
        }
        let reasons: Vec<String> = checks.iter().filter(|(_, ok, _)| !ok).map(|(n, _, d)| format!("{n}: {d}")).collect();
        let reasons = if reasons.is_empty() { "unknown".to_string() } else { reasons.join("; ") };
        return format!(
            "RUN echo \"ERROR: source-build key ({ngc_tag} x vLLM {vllm}) declares an INHERITED_SOURCE_BUILD_KEYS entry, but the inheritance is NOT eligible.\" && \\\n    echo \"  -> refused: {}\" && \\\n    echo \"  -> 상속은 **증거로만** 성립한다 — 성립 조건 정본 = .claude/skills/upstream-version-watch/references/source-build.md §3.1. 선언은 증거가 아니다.\" && \\\n    echo \"  -> --allow-unvalidated 로 우회되지 않는다 — 이것은 미검증 키가 아니라 **원장 항목의 결함**이다(D3: 우회 말고 경로를 고친다).\" && \\\n    echo \"  -> fix: judge_version_delta.py 를 재실행해 resolved.json#upstream_delta 를 갱신하거나, INHERITED 항목을 지우고 출구②(--allow-unvalidated)/③(source-build.md §3)로 간다.\" && exit 1",
            echo_safe(&reasons)
        );
    }
    if allow_unvalidated {
        return format!(
            "RUN echo \"WARN: source-build key ({ngc_tag} x vLLM {vllm}) is UNVALIDATED — attempt-build under HITL override (--allow-unvalidated).\" && \\\n    echo \"  -> arbiter = smoke (workflow S3). REQUIRED: record this attempt in docs/testlog/.\" && \\\n    echo \"  -> on smoke PASS: codify the key into VALIDATED_SOURCE_BUILD_KEYS (.claude/skills/upstream-version-watch/references/source-build.md §3).\""
        );
    }
    format!(
        "RUN echo \"ERROR: source-build key ({ngc_tag} x vLLM {vllm}) has NO validated patch set.\" && \\\n    echo \"  -> this is UNVALIDATED, not IMPOSSIBLE — 미검증이지 불가 판정 아님(시도-우선 따름정리).\" && \\\n    echo \"  -> run .claude/skills/upstream-version-watch/references/source-build.md §3 HITL discovery loop, then graduate the verified\" && \\\n    echo \"     patches into Dockerfile.source-build.template ((ngc_base x vllm_version) guarded, P6 catalog).\" && \\\n    echo \"  -> or: human-approved attempt-build via render --allow-unvalidated (WARN + testlog 의무).\" && \\\n    echo \"  -> refusing to build on unvalidated determinism (plan rev3 §5).\" && exit 1"
    )
}

/// `f"{x}"` of a value (the `str()` form)
fn opt_repr_py(v: Option<&V>) -> String {
    v.map_or_else(|| "None".to_string(), V::py_str)
}

fn compact_cuda(cuda: &str) -> String {
    if cuda.contains('.') {
        let mut parts = cuda.split('.');
        let a = parts.next().unwrap_or("");
        let b = parts.next().unwrap_or("");
        format!("{a}{b}")
    } else {
        cuda.to_string()
    }
}

/// `build_context`: the ordered placeholder map.
fn build_context(manifest: &V, resolved: &V, allow_unvalidated: bool) -> Result<Vec<(&'static str, String)>, Raised> {
    if !matches!(resolved, V::Dict(_)) {
        return Err(Raised { class: "TypeError", message: "resolved is not a mapping".into() });
    }
    let Some(vllm) = resolved.get("vllm_version") else { return key_error("vllm_version".into()) };
    let vllm = vllm.py_str();
    if !matches!(manifest, V::Dict(_)) {
        return Err(Raised { class: "TypeError", message: "manifest is not a mapping".into() });
    }
    let Some(arch) = manifest.get("cpu_arch") else { return key_error("cpu_arch".into()) };
    let arch = arch.py_str();
    let torch_pin = str_or(chain(resolved, &["torch", "pin"])?, "");
    let ngc_tag = str_or(chain(resolved, &["ngc_base", "tag"])?, "");
    let decision = str_or(chain(resolved, &["build_track", "decision"])?, "");
    let source = decision.contains("source");
    let track = if source { "source" } else { "wheel" };
    let wheel_cuda = str_truthy(chain(resolved, &["wheel", "cuda"])?);
    let wheel_manylinux = str_truthy(chain(resolved, &["wheel", "manylinux"])?);
    let cuda;
    let dockerfile;
    if source {
        let c = compact_cuda(&str_or(chain(resolved, &["ngc_base", "cuda_version"])?, ""));
        cuda = if c.is_empty() { "src".to_string() } else { c };
        dockerfile = "Dockerfile.source-build";
        let a = chain(resolved, &["source_build", "torch_cuda_arch"])?;
        let bad = match a {
            None | Some(V::None) => true,
            Some(x) => matches!(x.py_str().trim(), "" | "None" | "null"),
        };
        if bad {
            return Err(Raised {
                class: "SystemExit",
                message: format!(
                    "[render] FAIL: source-build 인데 resolved.source_build.torch_cuda_arch 미해소({}) — resolve_build_track.py 를 --sm-arch 또는 manifest compute_capability(terraforming_node 스캔)로 다시 돌려라. 다른 GPU 의 arch 를 기본값으로 쓰지 않는다.",
                    opt_repr(a)
                ),
            });
        }
    } else {
        cuda = wheel_cuda.clone();
        dockerfile = "Dockerfile";
    }
    let image_tag = format!("{vllm}-cu{cuda}-{arch}-{track}");
    let torch_cuda_arch = str_or(chain(resolved, &["source_build", "torch_cuda_arch"])?, "");
    let nas = str_or(manifest.get("nas_model_path"), "");
    Ok(vec![
        ("VLLM_VERSION", vllm.clone()),
        ("CPU_ARCH", arch),
        ("NGC_TAG", ngc_tag.clone()),
        ("TORCH_PIN", torch_pin),
        ("VLLM_REF", format!("v{vllm}")),
        ("TORCH_CUDA_ARCH", torch_cuda_arch),
        ("NAS_MODEL_PATH", nas),
        ("IMAGE_NAME", IMAGE_NAME.to_string()),
        ("IMAGE_TAG", image_tag),
        ("DOCKERFILE", dockerfile.to_string()),
        ("SOURCE_BUILD_PATCH_GUARD", patch_guard(&ngc_tag, &vllm, resolved, allow_unvalidated)),
        ("CUDA_VERSION", wheel_cuda),
        ("VLLM_MANYLINUX", wheel_manylinux),
    ])
}

// ── placeholder scan ───────────────────────────────────────────────────────────────────────────────

/// Match `{{\s*<inner>\s*}}` at `i`; returns the end index.
fn match_placeholder(chars: &[char], i: usize, inner: &dyn Fn(&[char], usize) -> Option<usize>) -> Option<usize> {
    if chars.get(i) != Some(&'{') || chars.get(i + 1) != Some(&'{') {
        return None;
    }
    let mut j = i + 2;
    while j < chars.len() && chars[j].is_whitespace() {
        j += 1;
    }
    j = inner(chars, j)?;
    while j < chars.len() && chars[j].is_whitespace() {
        j += 1;
    }
    if chars.get(j) == Some(&'}') && chars.get(j + 1) == Some(&'}') {
        Some(j + 2)
    } else {
        None
    }
}

fn replace_key(text: &str, key: &str, value: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let key_chars: Vec<char> = key.chars().collect();
    let inner = |c: &[char], j: usize| -> Option<usize> {
        if c.len() >= j + key_chars.len() && c[j..j + key_chars.len()] == key_chars[..] {
            Some(j + key_chars.len())
        } else {
            None
        }
    };
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        if let Some(end) = match_placeholder(&chars, i, &inner) {
            out.push_str(value);
            i = end;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

fn leftovers(text: &str) -> BTreeSet<String> {
    let chars: Vec<char> = text.chars().collect();
    let inner = |c: &[char], j: usize| -> Option<usize> {
        let mut k = j;
        while k < c.len() && (c[k].is_ascii_uppercase() || c[k].is_ascii_digit() || c[k] == '_') {
            k += 1;
        }
        // `[A-Z0-9_]+` is greedy but the closing `\s*}}` cannot start with such a character: no backtracking needed
        if k > j { Some(k) } else { None }
    };
    let mut found = BTreeSet::new();
    let mut i = 0;
    while i < chars.len() {
        if let Some(end) = match_placeholder(&chars, i, &inner) {
            found.insert(chars[i..end].iter().collect());
            i = end;
        } else {
            i += 1;
        }
    }
    found
}

fn substitute(text: &str, context: &[(&'static str, String)]) -> Result<String, Raised> {
    let mut text = text.to_string();
    for (key, val) in context {
        text = replace_key(&text, key, val);
    }
    let left = leftovers(&text);
    if !left.is_empty() {
        let items: Vec<String> = left.into_iter().collect();
        return value_error(format!("미치환 플레이스홀더 잔존(컨텍스트 누락): {}", py_list_repr(&items)));
    }
    Ok(text)
}

// ── build ledger stanza ────────────────────────────────────────────────────────────────────────────

const LEDGER_STANZA_HEAD: &str = "# ═════════════════════════════════════════════════════════════════════════════\n\
# [빌드 원장] /opt/easy-vllm/build_ledger.json — 이미지 자기서술(render_dockerfile.py 단일 소유 주입)\n\
#   build-arg 목록 = 이 템플릿의 `^ARG` 줄에서 렌더 시 파생 · 분류 = 상태 줄 > 로그 토큰 > 종료코드.\n\
#   **마지막 RUN** 이어야 한다 — 이 레이어만 바뀌면 `_C` 컴파일 캐시가 보존된다.\n\
#   `import vllm._C` 금지(빌드 시 libcuda 없음) · 베이스 사실은 파일에서 읽는다(ARG 가 ENV 를 가린다).\n\
# ═════════════════════════════════════════════════════════════════════════════\n";

/// `re.findall(r"^ARG\s+([A-Za-z_][A-Za-z0-9_]*)", text, re.M)`, de-duplicated in order.
fn ledger_build_args(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let line_start = i == 0 || chars[i - 1] == '\n';
        if line_start && chars[i..].starts_with(&['A', 'R', 'G']) {
            let mut j = i + 3;
            let ws_start = j;
            while j < chars.len() && chars[j].is_whitespace() {
                j += 1;
            }
            if j > ws_start && j < chars.len() && (chars[j].is_ascii_alphabetic() || chars[j] == '_') {
                let mut k = j;
                while k < chars.len() && (chars[k].is_ascii_alphanumeric() || chars[k] == '_') {
                    k += 1;
                }
                let name: String = chars[j..k].iter().collect();
                if !out.contains(&name) {
                    out.push(name);
                }
                i = k;
                continue;
            }
        }
        i += 1;
    }
    out
}

fn json_list(items: &[String]) -> String {
    format!("[{}]", items.iter().map(|s| json_string(s)).collect::<Vec<_>>().join(", "))
}

fn ledger_dockerfile_name(template_path: &str) -> String {
    let name = Path::new(template_path).file_name().map_or_else(|| template_path.to_string(), |n| n.to_string_lossy().into_owned());
    name.strip_suffix(".template").map_or(name.clone(), str::to_string)
}

fn build_ledger_stanza(build_args: &[String], dockerfile: &str) -> String {
    let body = LEDGER_BODY
        .replace("__EASY_VLLM_LEDGER_ARGS__", &json_list(build_args))
        .replace("__EASY_VLLM_LEDGER_DOCKERFILE__", &json_string(dockerfile));
    format!("{LEDGER_STANZA_HEAD}RUN python3 - <<'PY'\n{body}PY")
}

fn json_load_error(e: &pyjson::LoadError) -> Raised {
    let class = match e {
        pyjson::LoadError::Decode { .. } => "JSONDecodeError",
        pyjson::LoadError::Encoding(_) => "UnicodeDecodeError",
        pyjson::LoadError::TooDeep { .. } => "RecursionError",
        _ => "ValueError",
    };
    Raised { class, message: e.to_string() }
}

fn read_text(path: &str) -> Result<String, Raised> {
    let bytes = std::fs::read(path).map_err(|e| os_error("open", &e, Path::new(path)))?;
    let text = pyjson::decode_file_text(&bytes).map_err(|e| json_load_error(&e))?;
    Ok(text.to_utf8().unwrap_or_else(|| String::from_utf8_lossy(&text.to_backslashreplace_bytes()).into_owned()))
}

fn load_json(path: &str) -> Result<V, Raised> {
    let text = read_text(path)?;
    let value = pyjson::loads(&text).map_err(|e| json_load_error(&e))?;
    Ok(V::from_py(&value))
}

fn render(template_path: &str, manifest: &V, resolution: &V, allow_unvalidated: bool) -> Result<String, Raised> {
    let text = read_text(template_path)?;
    let mut ctx = build_context(manifest, resolution, allow_unvalidated)?;
    let stanza = build_ledger_stanza(&ledger_build_args(&text), &ledger_dockerfile_name(template_path));
    ctx.push(("BUILD_LEDGER_STANZA", stanza));
    let out = substitute(&text, &ctx)?;
    if text.contains("vllm-${VLLM_VERSION}+cu${CUDA_VERSION}") && ctx.iter().any(|(k, v)| *k == "CUDA_VERSION" && v.is_empty()) {
        return value_error("wheel 트랙인데 resolved.wheel.cuda 부재 → wheel URL 불완전".into());
    }
    Ok(out)
}

// ── canonical (shared) render ──────────────────────────────────────────────────────────────────────

fn shared_template(topology: &str, kind: &str) -> Option<&'static str> {
    match (topology, kind) {
        ("single" | "multi", "dockerfile") => Some("Dockerfile.template"),
        ("single" | "multi", "source-build") => Some("Dockerfile.source-build.template"),
        ("single", "compose") => Some("docker-compose.single.template.yaml"),
        ("multi", "compose") => Some("docker-compose.multi.template.yaml"),
        _ => None,
    }
}

fn kind_track(kind: &str) -> Option<&'static str> {
    match kind {
        "dockerfile" => Some("wheel"),
        "source-build" => Some("source-build"),
        _ => None,
    }
}

fn load_shared_resolution(skill_root: &Path, track: Option<&str>) -> Result<V, Raised> {
    let path = skill_root.join("assets").join("current-production-resolution.json");
    let path_s = path.to_string_lossy().into_owned();
    let unreadable = |e: Raised| Raised { class: "ValueError", message: format!("shared production resolution unreadable: {}", e.message) };
    let mut value = load_json(&path_s).map_err(unreadable)?;
    if !matches!(value, V::Dict(_)) {
        return value_error("shared production resolution is not an object".into());
    }
    if let Some(track) = track {
        let default_track = value.get("default_track").cloned();
        if !matches!(&default_track, Some(V::Str(s)) if s == track) {
            let picked = match value.get("tracks") {
                Some(t @ V::Dict(_)) => t.get(track).filter(|v| matches!(v, V::Dict(_))).cloned(),
                _ => None,
            };
            match picked {
                Some(p) => value = p,
                None => {
                    return value_error(format!(
                        "shared production resolution lacks tracks.{track} (default_track={})",
                        opt_repr(default_track.as_ref())
                    ))
                }
            }
        }
    }
    if !str_truthy_present(chain(&value, &["ngc_base", "tag"])?) {
        return value_error(format!(
            "shared production resolution lacks ngc_base.tag{}",
            track.map_or(String::new(), |t| format!(" for track {}", py_repr_str(t)))
        ));
    }
    Ok(value)
}

fn str_truthy_present(v: Option<&V>) -> bool {
    v.is_some_and(V::truthy)
}

fn render_shared(skill_root: &Path, kind: &str, topology: &str, manifest: &V, allow_unvalidated: bool) -> Result<String, Raised> {
    let Some(name) = shared_template(topology, kind) else {
        return value_error(format!("unknown topology/artifact kind: {}/{}", py_repr_str(topology), py_repr_str(kind)));
    };
    let path = skill_root.join("templates").join(name);
    if !path.is_file() {
        return Err(Raised { class: "FileNotFoundError", message: format!("shared template missing: {}", path.display()) });
    }
    let resolution = load_shared_resolution(skill_root, kind_track(kind))?;
    render(&path.to_string_lossy(), manifest, &resolution, allow_unvalidated)
}

// ── Flag gate · COPY context dirs ──────────────────────────────────────────────────────────────────

fn load_manifest_py(path: &str) -> Result<PyValue, Raised> {
    let bytes = std::fs::read(path).map_err(|e| os_error("open", &e, Path::new(path)))?;
    match safe_load(&bytes, &PyStr::new(path)) {
        Ok(v) => Ok(v),
        Err(error) => Err(Raised { class: error.class, message: error.message.to_utf8().unwrap_or_default() }),
    }
}

/// `_require_terraform_flag`: `Some(status)` ends the program, `None` lets the render go on.
fn require_terraform_flag(manifest_path: &str) -> Result<Option<i32>, Raised> {
    if !Path::new(manifest_path).is_file() {
        gate_cli::write_err("[render] FAIL: manifest 부재 — 테라포밍 미완(fail-closed).\n");
        return Ok(Some(4));
    }
    let fail_closed = |message: String| {
        gate_cli::write_err(&format!("[render] FAIL: manifest readiness 판정 불가(fail-closed): {message}\n"));
        Ok(Some(4))
    };
    let man = match load_manifest_py(manifest_path) {
        Ok(v) => if v.is_truthy() { v } else { PyValue::Dict(Default::default()) },
        Err(e) if matches!(e.class, "OSError" | "FileNotFoundError" | "PermissionError" | "ValueError") => return fail_closed(e.message),
        Err(e) => return Err(e),
    };
    let topology = match man.peel() {
        PyValue::Dict(d) => d.get("topology").cloned().unwrap_or(PyValue::None),
        _ => return attr_error(&V::from_py(&man)),
    };
    match evaluate_contract_value(&man, &topology) {
        Ok(ev) => {
            if ev.flag() {
                return Ok(None);
            }
            let reason = ev.reason().to_utf8().unwrap_or_default();
            gate_cli::write_err(&format!("[render] FAIL: manifest/Flag readiness 거부 — {reason}\n"));
            let code = ev.exit_code();
            Ok(Some(if code == 0 { 4 } else { code }))
        }
        Err(GateFailure::Raised(r)) if r.class == "ValueError" || r.class == "OSError" => fail_closed(r.message.to_utf8().unwrap_or_default()),
        Err(GateFailure::Raised(r)) => Err(Raised { class: r.class, message: r.message.to_utf8().unwrap_or_default() }),
        Err(GateFailure::Unsupported { boundary, detail }) => fail_closed(format!("{boundary}: {detail}")),
    }
}

/// `^\s*COPY\s+(?:--\S+\s+)*([A-Za-z0-9._-]+)/\s+\S` (re.M): the directory names COPY'd from the build context.
fn copy_context_dirs(rendered: &str) -> BTreeSet<String> {
    let c: Vec<char> = rendered.chars().collect();
    let ws = |mut i: usize| -> usize {
        while i < c.len() && c[i].is_whitespace() {
            i += 1;
        }
        i
    };
    let is_ident = |ch: char| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-');
    // after the options: ident+ '/' \s+ \S
    fn try_at(c: &[char], q: usize, is_ident: &dyn Fn(char) -> bool) -> Option<(String, usize)> {
        // option first (greedy), then the directory form
        if c.get(q) == Some(&'-') && c.get(q + 1) == Some(&'-') {
            let mut j = q + 2;
            let s = j;
            while j < c.len() && !c[j].is_whitespace() {
                j += 1;
            }
            if j > s {
                let k0 = j;
                let mut k = k0;
                while k < c.len() && c[k].is_whitespace() {
                    k += 1;
                }
                if k > k0 {
                    if let Some(r) = try_at(c, k, is_ident) {
                        return Some(r);
                    }
                }
            }
        }
        let mut j = q;
        while j < c.len() && is_ident(c[j]) {
            j += 1;
        }
        if j > q && c.get(j) == Some(&'/') {
            let mut k = j + 1;
            let s = k;
            while k < c.len() && c[k].is_whitespace() {
                k += 1;
            }
            if k > s && k < c.len() && !c[k].is_whitespace() {
                return Some((c[q..j].iter().collect(), k + 1));
            }
        }
        None
    }
    let mut found = BTreeSet::new();
    let mut i = 0;
    while i < c.len() {
        if i == 0 || c[i - 1] == '\n' {
            let j = ws(i);
            if c[j..].starts_with(&['C', 'O', 'P', 'Y']) {
                let k0 = j + 4;
                let k = ws(k0);
                if k > k0 {
                    if let Some((dir, end)) = try_at(&c, k, &is_ident) {
                        found.insert(dir);
                        i = end;
                        continue;
                    }
                }
            }
        }
        i += 1;
    }
    found
}

fn ensure_copy_context_dirs(rendered: &str, out_path: &str) -> Result<(), Raised> {
    let abs: PathBuf = if Path::new(out_path).is_absolute() {
        PathBuf::from(out_path)
    } else {
        std::env::current_dir().unwrap_or_default().join(out_path)
    };
    let base = abs.parent().map(Path::to_path_buf).unwrap_or_default();
    for d in copy_context_dirs(rendered) {
        let target = base.join(&d);
        if !target.is_dir() {
            std::fs::create_dir_all(&target).map_err(|e| os_error("makedirs", &e, &target))?;
            gate_cli::write_err(&format!("[render] 빌드 컨텍스트 디렉터리 생성 → {d}/ (COPY 대상 · 비추적 슬롯이라 clone 에 부재)\n"));
        }
        let keep = target.join(".gitkeep");
        if !keep.exists() {
            std::fs::write(&keep, b"").map_err(|e| os_error("open", &e, &keep))?;
            gate_cli::write_err(&format!("[render] 뼈대 선언 → {d}/.gitkeep\n"));
        }
    }
    Ok(())
}

fn non_empty(values: &Values, key: &str) -> Option<String> {
    gate_cli::text(values, key).filter(|s| !s.is_empty())
}

/// `--canonical-kind` / `--template`: `None` when neither is given (the caller falls through to the self-test message).
pub fn run_render(values: &Values, scripts: &Path, manifest_path: &str, allow_unvalidated: bool) -> Result<Option<i32>, Raised> {
    let kind = non_empty(values, "canonical_kind");
    let template = non_empty(values, "template");
    if kind.is_none() && template.is_none() {
        return Ok(None);
    }
    if let Some(status) = require_terraform_flag(manifest_path)? {
        return Ok(Some(status));
    }
    let skill_root = scripts.parent().map(Path::to_path_buf).unwrap_or_else(|| scripts.to_path_buf());
    let load_manifest = || -> Result<V, Raised> {
        let v = V::from_py(&load_manifest_py(manifest_path)?);
        Ok(if v.truthy() { v } else { V::Dict(Vec::new()) })
    };
    let out = if let Some(kind) = kind {
        let topology = non_empty(values, "topology");
        if topology.is_none() || template.is_some() || non_empty(values, "resolved").is_some() {
            gate_cli::write_err("[render] FAIL: --canonical-kind requires --topology and forbids --template/--resolved\n");
            return Ok(Some(2));
        }
        render_shared(&skill_root, &kind, &topology.unwrap_or_default(), &load_manifest()?, allow_unvalidated)?
    } else {
        let manifest = load_manifest()?;
        let resolved = load_json(&non_empty(values, "resolved").unwrap_or_else(|| "resolved.json".into()))?;
        render(&template.unwrap_or_default(), &manifest, &resolved, allow_unvalidated)?
    };
    match non_empty(values, "out") {
        Some(path) => {
            std::fs::write(&path, out.as_bytes()).map_err(|e| os_error("open", &e, Path::new(&path)))?;
            gate_cli::write_err(&format!("[render] deterministic render → {path} ({} bytes)\n", out.chars().count()));
            ensure_copy_context_dirs(&out, &path)?;
        }
        None => gate_cli::write_out(&out),
    }
    Ok(Some(0))
}
