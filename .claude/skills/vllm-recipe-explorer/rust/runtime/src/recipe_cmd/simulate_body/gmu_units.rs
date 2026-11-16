//! The U layer (function units, U0-U9) of `scripts/selftest_gmu_roles_batch.py`, over the native owners themselves: the retired
//! self-test loaded `recipe.py`, `sim_classify.py` and `estimate_vram.py` in process and called their private functions; here the
//! same checks call the Rust functions those programs became (`_gpu_roles` → [`gpu_roles`], `derive_batch` / `_kv_fit_tokens` /
//! `_resolve_clamp_kv` → `simulate_math`, `sim_classify.classify` → `classifier::classify`, `_stamp` / `_input_source` /
//! `_declared_axis` / `_batch_plan` / `_is_campaign_cell_lockset` / `_lockset_vocab` → this module's parent). The module sits under
//! the owner so it reaches the private functions directly; nothing is re-implemented.
//!
//! Adapted on purpose (the owner changed shape, the question did not):
//! - the Rust roles function takes the checkout root for the manifest **and** the GPU specification table, so each unit root also
//!   carries `references.md` (the Python original read the table from its own module's repository);
//! - U9: the Rust vocabulary owner is the native campaign validator read as text, not a Python module that is imported — a
//!   "load exception" is therefore an absent owner file, and a shape defect is a missing `KNOB_SOURCES` table; each still has to stop
//!   with exit 5 naming the cause (no panic), and a contract-shaped owner still loads.

use super::super::{declared_axis_tp, gpu_roles};
use super::{batch_plan, declared_axis, is_campaign_cell_lockset, lockset_vocab, VOCAB_REL};
use crate::classifier;
use crate::estimate_vram::KV_BLOCK_ALIGN_BUFFER;
use crate::gmu_roles_selftest::Report;
use crate::model_cli::Streams;
use crate::simulate_math::{self as sm, Plan, Profile};
use agent_runtime_support::pyjson::loads_text;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use std::path::Path;

const GIB: f64 = 1073741824.0;
const BUFFER: f64 = 1.02;
const MAX_LEN: i64 = 32768;
const L_TYP: i64 = 8192;
const SAFETY: f64 = 0.90;
const MANIFEST: &str = "topology: single\ngpus_per_node: 1\ngpu_model: \"NVIDIA GB10\"\nmodel_source: managed\nterraforming:\n  complete: true\n  branch_verified: true\n";
const TARGET: &str = r#"{"gpu_model": "NVIDIA RTX PRO 6000", "per_card_vram_gib": 96, "target_gmu": 0.85, "cards_per_node": 1}"#;
const REFERENCES_REL: &str = ".claude/skills/wiki-desk/reference/references.md";

fn j(text: &str) -> PyValue {
    loads_text(&PyStr::new(text)).expect("a fixture JSON text")
}

fn root_of(path: &Path) -> PyStr {
    PyStr::new(&path.to_string_lossy())
}

fn key(v: &PyValue, k: &str) -> PyValue {
    match v.peel() {
        PyValue::Dict(d) => d.get(k).cloned().unwrap_or(PyValue::None),
        _ => PyValue::None,
    }
}

fn is_text(v: &PyValue, want: &str) -> bool {
    matches!(v.peel(), PyValue::Str(s) if s.to_utf8().as_deref() == Some(want))
}

fn is_int(v: &PyValue, want: i64) -> bool {
    matches!(v.peel(), PyValue::Int(n) if i64::try_from(n.clone()).ok() == Some(want))
}

fn none(v: &PyValue) -> bool {
    matches!(v.peel(), PyValue::None)
}

fn shown(v: &PyValue) -> String {
    v.dumps_ascii().unwrap_or_else(|_| "?".to_string())
}


fn profile(weights: f64, overhead: f64, kv_gib: f64, tokens: i64) -> Profile {
    Profile {
        weights_gib: Some(weights),
        non_kv_overhead_gib: Some(overhead),
        kv_cache_gib: Some(kv_gib),
        kv_cache_tokens: Some(tokens),
        ..Profile::default()
    }
}

fn no_formula(_: i64) -> i64 {
    0
}

fn clamp(p: &Profile, max_len: i64, batch: i64, request_tokens: Option<i64>) -> sm::Clamp {
    sm::resolve_clamp_kv(p, max_len, Some(batch), 96.0, 0.85, 1, request_tokens, &no_formula)
}

fn fail_class(c: &sm::Clamp) -> Option<&'static str> {
    c.fail.as_ref().map(|f| f.0)
}

/// Runs U0-U9 with fixture roots below `tmp`; `repo` is the checkout whose `references.md` and vocabulary owner the units read.
#[allow(clippy::too_many_lines)]
pub fn unit_layer(tmp: &Path, repo: &Path) -> Report {
    let mut r = Report::default();
    r.ck(
        "U0 픽스처 버퍼 상수가 정본(estimate_vram.KV_BLOCK_ALIGN_BUFFER)과 같다(기대값 계산이 거울로 늦지 않게)",
        KV_BLOCK_ALIGN_BUFFER == BUFFER,
        || format!("정본={KV_BLOCK_ALIGN_BUFFER}"),
    );
    let froot = tmp.join("unit_root");
    let droot = tmp.join("unit_root_discrete");
    let discrete = MANIFEST.replace("NVIDIA GB10", "NVIDIA RTX PRO 6000");
    for (root, man) in [(&froot, MANIFEST.to_string()), (&droot, discrete)] {
        for topo in ["single", "multi"] {
            let dir = root.join("output").join(topo);
            std::fs::create_dir_all(&dir).expect("a unit root");
            std::fs::write(dir.join("manifest.yaml"), &man).expect("a unit manifest");
        }
        let refs = root.join(REFERENCES_REL);
        std::fs::create_dir_all(refs.parent().expect("a parent")).expect("the references directory");
        std::fs::copy(repo.join(REFERENCES_REL), &refs).expect("the GPU specification table");
    }
    let (f, d) = (root_of(&froot), root_of(&droot));
    let roles = |cfg: &str, root: &PyStr| {
        let mut streams = Streams::default();
        gpu_roles(&mut streams, &j(cfg), 1, root)
    };
    let cfg_target = format!(r#"{{"target_gpu": {TARGET}, "safety_margin": {SAFETY}}}"#);

    // U1 — target flow
    match roles(&cfg_target, &f) {
        Ok(t) => {
            r.ck("U1 target 흐름: deploy_gmu == target_gpu.target_gmu(0.85)", t.deploy_gmu == 0.85, || format!("{}", t.deploy_gmu));
            r.ck(
                "U1 target 흐름: gate_margin == config.safety_margin(0.90) — 둘이 한 변수로 접히지 않는다",
                t.gate_margin == SAFETY && t.gate_margin != t.deploy_gmu,
                || format!("{} {}", t.gate_margin, t.deploy_gmu),
            );
            r.ck(
                "U1 target 흐름: lockset gmu_source=target_gmu · 예산 = per_card_vram",
                t.gmu_source == "target_gmu" && t.budget_gib == 96.0 && t.flow == "target",
                || format!("{} {} {}", t.gmu_source, t.budget_gib, t.flow),
            );
        }
        Err(e) => r.ck("U1 target 흐름", false, || format!("stop {}: {:?}", e.code, e.text.to_utf8())),
    }

    // U2 — host flow
    match roles(r#"{"safety_margin": 0.88, "test_device_total_gib": 100}"#, &f) {
        Ok(h) => {
            r.ck(
                "U2 host 흐름: gpu_model=manifest · per_card=통합메모리 선언 total · deploy_gmu=safety_margin 승계",
                h.flow == "host" && h.gpu_model == "NVIDIA GB10" && h.budget_gib == 100.0 && h.deploy_gmu == 0.88 && h.gate_margin == 0.88,
                || format!("{} {} {} {} {}", h.flow, h.gpu_model, h.budget_gib, h.deploy_gmu, h.gate_margin),
            );
            r.ck(
                "U2 host 흐름: 승계는 정본 칸이 아니므로 gmu_source=hand + 출처 문장이 승계를 말한다(침묵 폴백 ✗)",
                h.gmu_source == "hand" && h.deploy_gmu_source.contains("safety_margin 승계"),
                || format!("{} {}", h.gmu_source, h.deploy_gmu_source),
            );
        }
        Err(e) => r.ck("U2 host 흐름", false, || format!("stop {}: {:?}", e.code, e.text.to_utf8())),
    }
    match roles(r#"{"safety_margin": 0.95, "test_device_total_gib": 100}"#, &f) {
        Ok(hc) => r.ck(
            "U2 host 흐름 통합메모리 0.90 하드클램프 유지(deploy 만 · 게이트 승수는 선언 그대로)",
            hc.deploy_gmu == 0.90 && hc.gate_margin == 0.95 && hc.deploy_gmu_source.contains("하드클램프"),
            || format!("{} {} {}", hc.deploy_gmu, hc.gate_margin, hc.deploy_gmu_source),
        ),
        Err(e) => r.ck("U2 host 흐름 하드클램프", false, || format!("stop {}", e.code)),
    }
    match (roles(r#"{"safety_margin": 0.9, "test_device_total_gib": 100}"#, &d), roles(r#"{"safety_margin": 0.9}"#, &d)) {
        (Ok(hd), Ok(hl)) => r.ck(
            "U2 host 흐름 discrete: 선언 test_device_total_gib(100) > references.md 역룩업(96) · 선언 없으면 역룩업",
            hd.budget_gib == 100.0 && hd.budget_source.contains("declared") && hl.budget_gib == 96.0 && hl.budget_source.contains("역룩업"),
            || format!("[{} {}] [{} {}]", hd.budget_gib, hd.budget_source, hl.budget_gib, hl.budget_source),
        ),
        _ => r.ck("U2 host 흐름 discrete", false, || "stopped".to_string()),
    }
    let conflict = roles(r#"{"safety_margin": 0.9, "test_device_total_gib": 100, "vram_budget_gb": 24}"#, &f);
    r.ck(
        "U2 host 흐름: 선언 예산 vram_budget_gb=24 ↔ 노드 per_card 100 충돌 → exit 5(실측이 선언을 조용히 덮지 않는다)",
        matches!(&conflict, Err(e) if e.code == 5 && e.text.to_utf8().unwrap_or_default().contains("target_gpu 블록")),
        || format!("died={} code={:?}", conflict.is_err(), conflict.as_ref().err().map(|e| e.code)),
    );
    match roles(r#"{"safety_margin": 0.9, "test_device_total_gib": 100, "vram_budget_gb": 98}"#, &f) {
        Ok(hm) => r.ck(
            "U2 음성대조: 허용오차 안(98 vs 100)이면 진행하고 정합 사실을 예산 출처에 적는다",
            hm.budget_gib == 100.0 && hm.budget_source.contains("vram_budget_gb=98"),
            || format!("{} {}", hm.budget_gib, hm.budget_source),
        ),
        Err(e) => r.ck("U2 음성대조", false, || format!("stop {}", e.code)),
    }

    // U3 — fail-loud negative controls
    let no_gmu = r#"{"gpu_model": "NVIDIA RTX PRO 6000", "per_card_vram_gib": 96, "cards_per_node": 1}"#;
    let u3a = roles(&format!(r#"{{"target_gpu": {no_gmu}, "safety_margin": {SAFETY}}}"#), &f);
    r.ck(
        "U3 target_gpu.target_gmu 미선언 → exit 5(종전 기본 0.90 으로 메우지 않는다)",
        matches!(&u3a, Err(e) if e.code == 5),
        || format!("died={} code={:?}", u3a.is_err(), u3a.as_ref().err().map(|e| e.code)),
    );
    let u3b = roles(r#"{"test_device_total_gib": 100}"#, &f);
    r.ck(
        "U3 host 흐름 safety_margin 도 미선언 → exit 5(배포 gmu 를 기본값으로 쓰지 않는다)",
        matches!(&u3b, Err(e) if e.code == 5),
        || format!("died={} code={:?}", u3b.is_err(), u3b.as_ref().err().map(|e| e.code)),
    );

    // U4 — derive_batch
    let plan = |req: Option<i64>, typ: Option<i64>| Plan {
        derive_mode: true,
        requirement: req,
        requirement_state: if req.is_some() { "declared" } else { "null" }.to_string(),
        requirement_source: "fx".to_string(),
        typical: typ,
        typical_source: "fx".to_string(),
        prior_batch: None,
    };
    let dv = sm::derive_batch(&plan(Some(16), Some(1000)), Some(MAX_LEN), Some(20000), "fx");
    r.ck(
        "U4 요구 16 ≤ KV-fit 20 → batch 16 · declared-requirement",
        dv.batch == Some(16) && dv.kv_fit == Some(20) && dv.batch_source == Some("declared-requirement"),
        || format!("{dv:?}"),
    );
    let dv = sm::derive_batch(&plan(Some(20), Some(1000)), Some(MAX_LEN), Some(20000), "fx");
    r.ck(
        "U4 경계: 요구 20 == KV-fit 20 → 요구 채택(declared-requirement · 낮추지 않는다)",
        dv.batch == Some(20) && dv.batch_source == Some("declared-requirement"),
        || format!("{dv:?}"),
    );
    let dv = sm::derive_batch(&plan(Some(21), Some(1000)), Some(MAX_LEN), Some(20000), "fx");
    r.ck(
        "U4 경계 +1: 요구 21 > KV-fit 20 → batch 20 · kv-fit-measured",
        dv.batch == Some(20) && dv.batch_source == Some("kv-fit-measured"),
        || format!("{dv:?}"),
    );
    let dv = sm::derive_batch(&plan(Some(64), Some(1000)), Some(MAX_LEN), Some(20000), "fx");
    r.ck(
        "U4 요구 64 > KV-fit 20 → batch 20 · kv-fit-measured · 낮춘 사유 기재",
        dv.batch == Some(20) && dv.batch_source == Some("kv-fit-measured") && dv.reason.contains("낮춤"),
        || format!("{dv:?}"),
    );
    let dv = sm::derive_batch(&plan(None, None), Some(MAX_LEN), Some(700000), "fx");
    r.ck(
        "U4 요구 없음(null · L 부재) → batch 미정 · batch_source 없음 · KV-fit 은 참고값(L=max_model_len 대체 기재)",
        dv.batch.is_none()
            && dv.batch_source.is_none()
            && dv.kv_fit == Some(700000 / MAX_LEN)
            && dv.kv_fit_length == Some(MAX_LEN)
            && dv.kv_fit_length_source.contains("대체")
            && dv.reason.contains("참고값"),
        || format!("{dv:?}"),
    );
    let dv = sm::derive_batch(&plan(Some(8), Some(1000)), Some(MAX_LEN), None, "엔진 토큰 부재");
    r.ck(
        "U4 KV-fit 산출 불가 ∧ 요구 선언 → batch=요구 · verified=False(검증 없이 채택을 적는다)",
        dv.batch == Some(8) && dv.batch_source == Some("declared-requirement") && dv.verified == Some(false),
        || format!("{dv:?}"),
    );
    let dv = sm::derive_batch(&plan(None, None), Some(MAX_LEN), None, "엔진 토큰 부재");
    r.ck(
        "U4 KV-fit 산출 불가 ∧ 요구 없음 → batch 미정(max-num-seqs 를 지어내지 않는다)",
        dv.batch.is_none() && dv.batch_source.is_none(),
        || format!("{dv:?}"),
    );

    // U5 — the classifier's KV-fit re-verification
    let trial = |cand: &str| {
        j(&format!(
            r#"{{"load_ok": true, "functional": {{"passed": true}}, "vllm_profile": {{"weights_gib": 10.0, "non_kv_overhead_gib": 2.0, "kv_cache_gib": 20.0, "kv_cache_tokens": 10000}}, "candidate": {cand}}}"#
        ))
    };
    let classify = |t: &PyValue, typical: PyValue| {
        classifier::classify(t, &PyValue::int(96), &PyValue::Float(SAFETY), &typical).map(PyValue::Dict)
    };
    let clamp20 = format!("{}", 20 * (GIB as i64));
    let over = trial(&format!(r#"{{"batch": 16, "kv_cache_memory_bytes": {clamp20}}}"#));
    match classify(&over, PyValue::int(1000)) {
        Ok(v) => r.ck(
            "U5 클램프 트라이얼 batch 16 > 엔진 KV-fit 10 → failure_class=none · adjust_target=batch · kv_fit=10",
            is_text(&key(&v, "failure_class"), "none") && is_text(&key(&v, "adjust_target"), "batch") && is_int(&key(&v, "kv_fit"), 10),
            || shown(&v),
        ),
        Err(e) => r.ck("U5 classify", false, || format!("{:?}", e.message.to_utf8())),
    }
    let check_of = |v: &PyValue, field: &str| key(&key(v, "kv_fit_check"), field);
    match classify(&trial(&format!(r#"{{"batch": 10, "kv_cache_memory_bytes": {clamp20}}}"#)), PyValue::int(1000)) {
        Ok(v) => r.ck(
            "U5 경계: batch 10 == 엔진 KV-fit 10 → 넘지 않는다(adjust 없음 · checked)",
            none(&key(&v, "adjust_target"))
                && matches!(check_of(&v, "checked").peel(), PyValue::Bool(true))
                && matches!(check_of(&v, "exceeds").peel(), PyValue::Bool(false)),
            || shown(&v),
        ),
        Err(e) => r.ck("U5 경계", false, || format!("{:?}", e.message.to_utf8())),
    }
    match classify(&trial(&format!(r#"{{"batch": 11, "kv_cache_memory_bytes": {clamp20}}}"#)), PyValue::int(1000)) {
        Ok(v) => r.ck("U5 경계 +1: batch 11 > KV-fit 10 → adjust_target=batch", is_text(&key(&v, "adjust_target"), "batch"), || shown(&v)),
        Err(e) => r.ck("U5 경계 +1", false, || format!("{:?}", e.message.to_utf8())),
    }
    match classify(&over, PyValue::None) {
        Ok(v) => r.ck(
            "U5 비회귀: typical 미전달(기존 호출부) → 종전 결과(adjust 없음 · kv_fit_check 없음)",
            none(&key(&v, "adjust_target")) && !matches!(v.peel(), PyValue::Dict(dd) if dd.contains_key("kv_fit_check")),
            || shown(&v),
        ),
        Err(e) => r.ck("U5 비회귀", false, || format!("{:?}", e.message.to_utf8())),
    }
    match classify(&trial(r#"{"batch": 16, "kv_cache_memory_bytes": null}"#), PyValue::int(1000)) {
        Ok(v) => r.ck(
            "U5 언클램프 측정 트라이얼은 재검증 대상 아님(측정 풀 토큰은 배포 KV 가 아니다)",
            none(&key(&v, "adjust_target")) && matches!(check_of(&v, "checked").peel(), PyValue::Bool(false)),
            || shown(&v),
        ),
        Err(e) => r.ck("U5 언클램프", false, || format!("{:?}", e.message.to_utf8())),
    }

    // U6 — clamp formula boundaries
    let prof = profile(10.0, 2.0, 70.0, 700000);
    let (tokens, _src) = sm::kv_fit_tokens(&prof, 96.0, 0.85, 1, None);
    let tokens = tokens.unwrap_or(0);
    let res = clamp(&prof, 1, tokens, Some(1));
    r.ck(
        "U6 경계: batch = KV-fit 토큰(L=1)이 정확히 천장에 닿아도 클램프 산식이 받아들인다(버퍼 일관)",
        res.fail.is_none() && res.kv.is_some(),
        || format!("{res:?}"),
    );
    let per_token = 70.0 * GIB / 700000.0;
    let max_safe = (96.0 * 0.85 * GIB) as i64 - (10.0 * GIB) as i64 - (2.0 * GIB) as i64;
    let naive = (max_safe as f64 / per_token).floor() as i64;
    let res = clamp(&prof, 1, naive, Some(1));
    r.ck(
        &format!("U6 음성대조: 버퍼 없이 센 KV-fit({naive})을 batch 로 쓰면 같은 산식이 vram_infeasible 로 거부한다"),
        naive > tokens && res.kv.is_none() && fail_class(&res) == Some("vram_infeasible"),
        || format!("{res:?}"),
    );
    let res = clamp(&prof, 4096, 4, Some(1000));
    r.ck(
        "U6 required 토큰 = max(max_model_len, L×batch) — 최악 길이 1건이 대표 길이 batch 보다 크면 그쪽",
        res.note.contains("max(max_model_len=4096, L=1000×batch=4)=4096"),
        || res.note.clone(),
    );
    let res = clamp(&prof, MAX_LEN, 16, Some(L_TYP));
    let want = (per_token * (MAX_LEN.max(L_TYP * 16)) as f64 * BUFFER) as i64;
    r.ck(
        "U6 산출 batch 의 클램프 = required(L×batch 토큰 × per_token × 버퍼) · 천장(max_safe)이 아니다(천장은 실행 가능 경계)",
        res.kv == Some(want)
            && res.kv.is_some_and(|kv| kv < max_safe)
            && res.note.contains(&format!("max(max_model_len={MAX_LEN}, L={L_TYP}×batch=16)={} tokens", L_TYP * 16)),
        || format!("kv={:?} want={want} max_safe={max_safe} note={}", res.kv, res.note),
    );
    let res = clamp(&prof, MAX_LEN, 16, None);
    r.ck(
        "U6 비회귀: request_tokens 미전달(손레버·구 lockset) → 종전 식 max_model_len × batch",
        res.kv == Some((per_token * MAX_LEN as f64 * 16.0 * BUFFER) as i64) && res.note.contains(&format!("max_model_len={MAX_LEN}×batch=16")),
        || res.note.clone(),
    );

    // U7 — the two multipliers answer different questions
    let heavy = j(r#"{"load_ok": true, "functional": {"passed": true}, "candidate": {}, "vllm_profile": {"weights_gib": 70.0, "non_kv_overhead_gib": 14.0, "kv_cache_gib": 1.0, "kv_cache_tokens": 1000}}"#);
    let v = classify(&heavy, PyValue::None);
    let res = sm::resolve_clamp_kv(&profile(70.0, 14.0, 1.0, 1000), 1000, Some(1), 96.0, 0.85, 1, None, &no_formula);
    r.ck(
        "U7 weights+overhead 84 GiB: 게이트(96×gate_margin 0.90=86.4) 통과 ∧ 클램프 천장(96×deploy_gmu 0.85=81.6) 초과",
        matches!(&v, Ok(v) if is_text(&key(v, "failure_class"), "none")) && fail_class(&res) == Some("vram_infeasible"),
        || format!("classify={:?} clamp={res:?}", v.as_ref().map(shown).ok()),
    );

    // U8 — stamp vocabulary · declared axis shapes · lockset path
    let repo_root = root_of(&std::fs::canonicalize(repo).unwrap_or_else(|_| repo.to_path_buf()));
    match lockset_vocab(&repo_root) {
        Ok(vocab) => {
            let stamped = vocab.stamp("batch_source", Some("bogus"));
            r.ck("U8 explorer 각인 값 어휘 밖 → exit 5(내부 결함)", matches!(&stamped, Err(e) if e.code == 5), || {
                format!("died={} code={:?}", stamped.is_err(), stamped.as_ref().err().map(|e| e.code))
            });
            let (val, why) = vocab.input_source("batch_source", &PyValue::str("bogus"));
            r.ck(
                "U8 음성대조: 입력 lockset 의 어휘 밖 출처는 멈추지 않고 사유로 돌려준다(P6 와 같은 등급 — 기재)",
                val.is_none() && why.as_deref().is_some_and(|w| w.contains("어휘 밖")),
                || format!("val={val:?} why={why:?}"),
            );
            let (val, why) = vocab.input_source("kv_source", &PyValue::str("measured-clamp"));
            r.ck("U8 어휘 안 입력 출처는 그대로 읽힌다", val.as_deref() == Some("measured-clamp") && why.is_none(), || {
                format!("val={val:?} why={why:?}")
            });
            let states: Vec<&str> = [r#""2""#, "2.0", r#""해당 없음""#, "0", "true"]
                .iter()
                .map(|v| declared_axis_tp(&j(&format!(r#"{{"declared_axes": {{"tp": {v}}}}}"#))).1)
                .collect();
            r.ck(
                "U8 declared_axes.tp 모양 결함(문자열·실수·0·bool)은 멈추지 않고 malformed 로 돌려준다(기재 항목)",
                states == ["malformed"; 5],
                || format!("{states:?}"),
            );
            let strict = declared_axis(&j(r#"{"declared_axes": {"concurrency_requirement": "16"}}"#), "concurrency_requirement");
            r.ck(
                "U8 음성대조: 산식 입력(concurrency_requirement='16')의 모양 결함은 exit 5(거짓 선언을 부재로 접지 않는다)",
                matches!(&strict, Err(e) if e.code == 5),
                || format!("died={} code={:?}", strict.is_err(), strict.as_ref().err().map(|e| e.code)),
            );
            let mut cand = match j(r#"{"batch": "<<FILL>>", "max_model_len": 1024}"#).peel() {
                PyValue::Dict(dd) => dd.clone(),
                _ => PyDict::new(),
            };
            let planned = batch_plan(&mut cand, &j("{}"), &vocab);
            r.ck(
                "U8 lockset batch='<<FILL>>' → traceback 이 아니라 exit 5 안내",
                matches!(&planned, Err(e) if e.code == 5),
                || format!("died={} code={:?}", planned.is_err(), planned.as_ref().err().map(|e| e.code)),
            );
        }
        Err(e) => r.ck("U8 어휘 소유자 적재", false, || format!("{:?}", e.text.to_utf8())),
    }
    let rr = repo_root.to_utf8().unwrap_or_default();
    let shapes: Vec<bool> = [
        "campaigns/camp-x/cells/c1/lockset.json",
        "campaigns/_template/cells/_cell/lockset.json",
        "campaigns/camp-x/cells/c1/config.yaml",
        "lockset.json",
        "campaigns/camp-x/lockset.json",
    ]
    .iter()
    .map(|rel| is_campaign_cell_lockset(&repo_root, &PyStr::new(&format!("{rr}/{rel}"))))
    .collect();
    r.ck(
        "U8 캠페인 셀 lockset 경로 판정: 인스턴스 셀만 참(뼈대 _template·다른 파일·루트·깊이 다름은 거짓)",
        shapes == [true, false, false, false, false],
        || format!("{shapes:?}"),
    );

    // U9 — a vocabulary owner that is present but malformed (or absent): exit 5 naming the cause, never a panic
    let vroot = tmp.join("vocab_root");
    let vfile = vroot.join(VOCAB_REL);
    std::fs::create_dir_all(vfile.parent().expect("a parent")).expect("the vocabulary directory");
    let vr = root_of(&vroot);
    let good = "pub const LOCKSET_PROVENANCE: [&str; 2] = [\"explorer-phase2\", \"hand-authored\"];\npub const KNOB_SOURCES: [(&str, &[&str]); 3] = [\n    (\"batch_source\", &[\"declared-requirement\"]),\n];\n";
    let cases: [(&str, Option<&str>, &str); 3] = [
        ("어휘 상수 없음(옛 판)", Some("pub const KNOB_SOURCES_OLD: u8 = 0;\n"), "LOCKSET_PROVENANCE"),
        ("소유자 파일 부재(적재 불가)", None, "소유자가 없다"),
        (
            "모양 불일치(노브 표 없음)",
            Some("pub const LOCKSET_PROVENANCE: [&str; 2] = [\"explorer-phase2\", \"hand-authored\"];\npub const KNOB_SOURCES: u8 = 3;\n"),
            "KNOB_SOURCES",
        ),
    ];
    for (label, body, want) in cases {
        match body {
            Some(text) => std::fs::write(&vfile, text).expect("a vocabulary owner"),
            None => {
                let _ = std::fs::remove_file(&vfile);
            }
        }
        let got = lockset_vocab(&vr);
        let err = got.as_ref().err().map(|e| e.text.to_utf8().unwrap_or_default()).unwrap_or_default();
        r.ck(
            &format!("U9 lockset 어휘 소유자 {label} → traceback 이 아니라 exit 5 · 원인({want})을 말한다"),
            matches!(&got, Err(e) if e.code == 5) && err.contains(want),
            || format!("died={} code={:?} err={err:?}", got.is_err(), got.as_ref().err().map(|e| e.code)),
        );
    }
    std::fs::write(&vfile, good).expect("a contract-shaped owner");
    let got = lockset_vocab(&vr);
    r.ck(
        "U9 음성대조: 계약 모양의 소유자는 그대로 적재된다(가드가 전부를 막으면 가드가 아니다)",
        matches!(&got, Ok(v) if v.provenance == ["explorer-phase2", "hand-authored"]
            && v.knobs.get("batch_source").map(Vec::as_slice) == Some(&["declared-requirement".to_string()][..])
            && v.knobs.len() == 1),
        || format!("{:?}", got.as_ref().map(|v| (v.provenance.clone(), v.knobs.clone())).map_err(|e| e.code)),
    );
    r
}
