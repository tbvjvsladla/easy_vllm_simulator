//! Pure numerics of `recipe.py simulate` (chunk 2 of the port): `_kv_dtype_bytes`, `_enrich_overhead`, `_kv_fit_tokens`,
//! `derive_batch`, `_resolve_clamp_kv`. Plain Rust types (no Python-value emulation): the caller reads the dict inputs and
//! passes numbers. Nothing here touches the disk or docker. Truncations follow Python's `int()` (toward zero).

use crate::estimate_vram::KV_BLOCK_ALIGN_BUFFER;

const GIB: f64 = 1073741824.0;

/// The 1-byte KV dtype spellings (`_KV_ONE_BYTE_DTYPES`).
const KV_ONE_BYTE_DTYPES: [&str; 3] = ["fp8", "fp8_e4m3", "fp8_e5m2"];

/// `_kv_dtype_bytes`: 1 for an fp8 family spelling (trimmed, lowercased), else 2.
pub fn kv_dtype_bytes(kv_cache_quant: Option<&str>) -> i64 {
    let q = kv_cache_quant.unwrap_or("").trim().to_lowercase();
    if KV_ONE_BYTE_DTYPES.contains(&q.as_str()) {
        1
    } else {
        2
    }
}

/// The measured `vllm_profile` fields the numerics read (GiB values and engine-reported tokens).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Profile {
    pub weights_gib: Option<f64>,
    pub kv_cache_gib: Option<f64>,
    pub kv_cache_tokens: Option<i64>,
    pub non_kv_overhead_gib: Option<f64>,
    pub gmu_trial: Option<f64>,
    pub device_total_gib: Option<f64>,
    pub gmu_used: Option<f64>,
    pub gmu_source: Option<String>,
}

/// `max_safe_kv_bytes(budget, gmu, weights, overhead)`: `int(budget*gmu*GIB) - int(weights) - int(overhead)`.
pub fn max_safe_kv_bytes(budget_gib: f64, gmu: f64, weights_bytes: f64, overhead_bytes: f64) -> i64 {
    (budget_gib * gmu * GIB) as i64 - weights_bytes as i64 - overhead_bytes as i64
}

/// `_enrich_overhead`: derive `non_kv_overhead_gib = gmu × device_total − weights − kv` in place when the log gave none
/// (skipped for an explicit-clamp trial, whose residual includes the deliberately unused pool).
pub fn enrich_overhead(p: &mut Profile, device_total_gib: f64, gmu_fallback: Option<f64>, kv_was_explicit: bool) {
    if p.non_kv_overhead_gib.is_some() || kv_was_explicit {
        return;
    }
    let from_log = p.gmu_trial.is_some();
    let gmu = p.gmu_trial.or(gmu_fallback);
    let (Some(gmu), Some(w), Some(kv)) = (gmu, p.weights_gib, p.kv_cache_gib) else { return };
    if device_total_gib == 0.0 {
        return;
    }
    let overhead = gmu * device_total_gib - w - kv;
    if overhead > 0.0 {
        p.non_kv_overhead_gib = Some(overhead);
        p.device_total_gib = Some(device_total_gib);
        p.gmu_used = Some(gmu);
        p.gmu_source = Some(
            if from_log { "log" } else { "known-input(candidate.gpu_memory_utilization)" }.to_string(),
        );
    }
}

/// `_kv_fit_tokens`: tokens the deployed KV holds and the source sentence. `None` tokens carry the reason.
pub fn kv_fit_tokens(
    p: &Profile,
    budget: f64,
    deploy_gmu: f64,
    tp_divisor: i64,
    clamp_bytes: Option<i64>,
) -> (Option<i64>, String) {
    let tok = p.kv_cache_tokens.unwrap_or(0);
    if tok == 0 {
        return (None, "엔진 보고 kv_cache_tokens 부재(로그 키 미관측 · mock 프로파일에 없음)".to_string());
    }
    if clamp_bytes.is_some() {
        return (Some(tok), format!("engine kv_cache_tokens={} (배포 클램프 트라이얼 — 환산 없음)", tok));
    }
    let kv_gib = p.kv_cache_gib.unwrap_or(0.0);
    let (Some(w), Some(o)) = (p.weights_gib, p.non_kv_overhead_gib) else {
        return (None, "언클램프 트라이얼인데 kv_cache_gib·weights·overhead 실측 중 부재 — 배포 천장으로 환산 불가".to_string());
    };
    if kv_gib == 0.0 {
        return (None, "언클램프 트라이얼인데 kv_cache_gib·weights·overhead 실측 중 부재 — 배포 천장으로 환산 불가".to_string());
    }
    let (mut wb, mut ob) = (w * GIB, o * GIB);
    if tp_divisor > 1 {
        wb /= tp_divisor as f64;
        ob /= tp_divisor as f64;
    }
    let max_safe = max_safe_kv_bytes(budget, deploy_gmu, wb, ob);
    let per_token = kv_gib * GIB / tok as f64;
    if max_safe <= 0 {
        return (Some(0), format!("배포 KV 천장 max_safe={} ≤ 0 (weights+overhead 가 budget×deploy_gmu 를 채운다)", max_safe));
    }
    let mut tokens = (max_safe as f64 / (per_token * KV_BLOCK_ALIGN_BUFFER)).floor() as i64;
    while tokens > 0 && ((per_token * tokens as f64 * KV_BLOCK_ALIGN_BUFFER) as i64) > max_safe {
        tokens -= 1;
    }
    (
        Some(tokens),
        format!(
            "언클램프 측정 환산: 배포 KV 천장 {} B ÷ (per_token {:.1} B × 블록정렬 버퍼 {:.2}) — per_token = 측정 KV {} B ÷ engine kv_cache_tokens {}",
            max_safe,
            per_token,
            KV_BLOCK_ALIGN_BUFFER,
            (kv_gib * GIB) as i64,
            tok
        ),
    )
}

/// The `_batch_plan` fields `derive_batch` reads.
#[derive(Clone, Debug, Default)]
pub struct Plan {
    pub derive_mode: bool,
    pub requirement: Option<i64>,
    pub requirement_state: String,
    pub requirement_source: String,
    pub typical: Option<i64>,
    pub typical_source: String,
    pub prior_batch: Option<i64>,
}

/// The `derive_batch` record (the lockset `batch_derivation` body, minus the key order the writer owns).
#[derive(Clone, Debug, PartialEq)]
pub struct Derivation {
    pub applied: bool,
    pub kv_fit_length: Option<i64>,
    pub kv_fit_length_source: String,
    pub kv_fit_tokens: Option<i64>,
    pub kv_fit_tokens_source: String,
    pub kv_fit: Option<i64>,
    pub batch: Option<i64>,
    pub batch_source: Option<&'static str>,
    pub verified: Option<bool>,
    pub reason: String,
}

/// `derive_batch`: `max_num_seqs = min(concurrency_requirement, KV_fit@L)`; see the original for the per-state behavior.
pub fn derive_batch(plan: &Plan, max_model_len: Option<i64>, fit_tokens: Option<i64>, fit_source: &str) -> Derivation {
    let length = plan.typical.or(max_model_len);
    let mut rec = Derivation {
        applied: plan.derive_mode,
        kv_fit_length: length,
        kv_fit_length_source: if plan.typical.is_some() {
            plan.typical_source.clone()
        } else {
            format!(
                "max_model_len={} 대체 — {} · 최악 길이로 보수 산정",
                max_model_len.map(|v| v.to_string()).unwrap_or_else(|| "None".to_string()),
                plan.typical_source
            )
        },
        kv_fit_tokens: fit_tokens,
        kv_fit_tokens_source: fit_source.to_string(),
        kv_fit: None,
        batch: None,
        batch_source: None,
        verified: None,
        reason: String::new(),
    };
    let req = plan.requirement;
    let len = length.unwrap_or(0);
    if fit_tokens.is_none() || len == 0 {
        if let Some(r) = req {
            rec.batch = Some(r);
            rec.batch_source = Some("declared-requirement");
            rec.verified = Some(false);
            rec.reason = format!("KV-fit 산출 불가({}) — 선언 요구 {} 를 **검증 없이** 채택", fit_source, r);
        } else {
            rec.reason = format!(
                "KV-fit 산출 불가({}) ∧ 동시성 요구 {} — batch 미정(max-num-seqs 미emit · vLLM 기본 admission 상한)",
                fit_source, plan.requirement_state
            );
        }
        return rec;
    }
    let fit = fit_tokens.unwrap() / len;
    rec.kv_fit = Some(fit);
    let Some(r) = req else {
        rec.reason = format!(
            "동시성 요구 {} → batch 를 산출하지 않는다(max-num-seqs 미emit · vLLM 기본 admission 상한 · 클램프는 max_model_len 1건 기준). KV-fit {}(@L={})은 참고값으로만 기재한다 — 요구가 있으면 declared_axes.concurrency_requirement 로 선언하라",
            plan.requirement_state, fit, len
        );
        return rec;
    };
    let floor_note = if fit >= 1 {
        ""
    } else {
        " · KV-fit 0(대표 요청 1건도 배포 KV 에 들지 않는다 — 1 로 두고 클램프 산정이 판정한다)"
    };
    if r <= fit {
        rec.batch = Some(r);
        rec.batch_source = Some("declared-requirement");
        rec.reason = format!("요구 {} ≤ KV-fit {}(@L={}) → 요구 채택", r, fit, len);
    } else {
        rec.batch = Some(fit.max(1));
        rec.batch_source = Some("kv-fit-measured");
        rec.reason = format!(
            "요구 {} > KV-fit {}(@L={}) → KV-fit 으로 낮춤. 요구를 지키려면 context·KV dtype·예산을 재조정해 다시 돈다(loop-until-done){}",
            r, fit, len, floor_note
        );
    }
    rec
}

/// `_resolve_clamp_kv` result: the clamp bytes, or the structural failure (`vram_infeasible` note), plus the note.
#[derive(Clone, Debug, PartialEq)]
pub struct Clamp {
    pub kv: Option<i64>,
    /// `(failure_class, note)` of a structural infeasibility.
    pub fail: Option<(&'static str, String)>,
    pub note: String,
}

/// `_resolve_clamp_kv`: `kv = min(required, max_safe)`. `formula_required(tokens)` is `required_kv_bytes(parsed, tokens, 1,
/// dtype_bytes)` — only consulted when the profile has no measured per-token KV (`kv_cache_gib` and `kv_cache_tokens`).
pub fn resolve_clamp_kv(
    p: &Profile,
    max_model_len: i64,
    batch: Option<i64>,
    budget: f64,
    deploy_gmu: f64,
    tp_divisor: i64,
    request_tokens: Option<i64>,
    formula_required: &dyn Fn(i64) -> i64,
) -> Clamp {
    let (Some(w), Some(o)) = (p.weights_gib, p.non_kv_overhead_gib) else {
        return Clamp { kv: None, fail: None, note: "weights/overhead 실측 부재.".to_string() };
    };
    let (mut wb, mut ob) = (w * GIB, o * GIB);
    if tp_divisor > 1 {
        wb /= tp_divisor as f64;
        ob /= tp_divisor as f64;
    }
    let batch = batch.filter(|b| *b != 0).unwrap_or(1);
    let req_tokens = request_tokens.filter(|t| *t != 0).map(|t| max_model_len.max(t * batch));
    let required: i64 = match (p.kv_cache_gib.filter(|v| *v != 0.0), p.kv_cache_tokens.filter(|v| *v != 0)) {
        (Some(g), Some(t)) => {
            let per_token = g * GIB / t as f64;
            match req_tokens {
                None => (per_token * max_model_len as f64 * batch as f64 * KV_BLOCK_ALIGN_BUFFER) as i64,
                Some(rt) => (per_token * rt as f64 * KV_BLOCK_ALIGN_BUFFER) as i64,
            }
        }
        _ => match req_tokens {
            None => formula_required(max_model_len * batch),
            Some(rt) => formula_required(rt),
        },
    };
    let basis = match (req_tokens, request_tokens) {
        (Some(rt), Some(l)) => format!("max(max_model_len={}, L={}×batch={})={} tokens", max_model_len, l, batch, rt),
        _ => format!("max_model_len={}×batch={}", max_model_len, batch),
    };
    let max_safe = max_safe_kv_bytes(budget, deploy_gmu, wb, ob);
    if max_safe <= 0 {
        return Clamp {
            kv: None,
            fail: Some(("vram_infeasible", "weights+overhead(실측) 만으로 천장 초과 → KV 조정 불가(HITL).".to_string())),
            note: String::new(),
        };
    }
    if required > max_safe {
        return Clamp {
            kv: None,
            fail: Some((
                "vram_infeasible",
                format!(
                    "required_kv={} > max_safe_kv={} (실측 기반 · {}) → max_model_len·batch 를 KV 로 만족 불가(HITL).",
                    required, max_safe, basis
                ),
            )),
            note: String::new(),
        };
    }
    let kv = required.min(max_safe);
    Clamp { kv: Some(kv), fail: None, note: format!("kv=min(required={}, max_safe={})={}. ({})", required, max_safe, kv, basis) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dtype_bytes() {
        assert_eq!(kv_dtype_bytes(Some(" FP8_E5M2 ")), 1);
        assert_eq!(kv_dtype_bytes(Some("bf16")), 2);
        assert_eq!(kv_dtype_bytes(None), 2);
    }

    #[test]
    fn derive_requirement_wins_and_lowers() {
        let plan = Plan { derive_mode: true, requirement: Some(16), requirement_state: "declared".into(), typical: Some(1000), ..Default::default() };
        let r = derive_batch(&plan, Some(4096), Some(83_000), "s");
        assert_eq!((r.kv_fit, r.batch, r.batch_source), (Some(83), Some(16), Some("declared-requirement")));
        let r = derive_batch(&plan, Some(4096), Some(8_000), "s");
        assert_eq!((r.kv_fit, r.batch, r.batch_source), (Some(8), Some(8), Some("kv-fit-measured")));
        let r = derive_batch(&plan, Some(4096), Some(500), "s");
        assert_eq!(r.batch, Some(1));
    }

    #[test]
    fn clamp_measured_and_infeasible() {
        let p = Profile { weights_gib: Some(10.0), non_kv_overhead_gib: Some(2.0), kv_cache_gib: Some(1.0), kv_cache_tokens: Some(1000), ..Default::default() };
        let c = resolve_clamp_kv(&p, 2000, Some(2), 100.0, 0.9, 1, None, &|_| 0);
        let per = GIB / 1000.0;
        assert_eq!(c.kv, Some((per * 2000.0 * 2.0 * 1.02) as i64));
        let c = resolve_clamp_kv(&p, 2000, Some(2), 12.0, 0.9, 1, None, &|_| 0);
        assert_eq!(c.fail.as_ref().map(|f| f.0), Some("vram_infeasible"));
    }

    #[test]
    fn enrich_skips_explicit_and_derives() {
        let mut p = Profile { weights_gib: Some(10.0), kv_cache_gib: Some(5.0), ..Default::default() };
        enrich_overhead(&mut p, 100.0, Some(0.5), true);
        assert_eq!(p.non_kv_overhead_gib, None);
        enrich_overhead(&mut p, 100.0, Some(0.5), false);
        assert_eq!(p.non_kv_overhead_gib, Some(35.0));
    }

    #[test]
    fn fit_tokens_clamp_trial_passthrough() {
        let p = Profile { kv_cache_tokens: Some(777), ..Default::default() };
        assert_eq!(kv_fit_tokens(&p, 1.0, 0.9, 1, Some(5)).0, Some(777));
        assert_eq!(kv_fit_tokens(&Profile::default(), 1.0, 0.9, 1, None).0, None);
    }
}
