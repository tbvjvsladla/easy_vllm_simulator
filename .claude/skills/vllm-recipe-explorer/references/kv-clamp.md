# 절대 KV 클램프 · 이식형 타겟-GPU 예산 · MoE 백엔드 (조건부 reference — Phase 2 의 핵심)

> spine step 8(trial-loop)의 **산정식·측정 정본·백엔드 함정**. 헌법 `policy:KV_ABSOLUTE_CLAMP_PORTABILITY`
> 의 스킬-측 상세. 여기 수치·공식은 **측정이 정본이고 공식은 상한**이라는 원칙 아래서만 유효하다.

## 1. 절대 KV 클램프

vLLM 인자 **`--kv-cache-memory-bytes`**(GPU당 KV 바이트, 정수)로 KV 캐시를 **절대값으로 고정**한다.
이 인자를 주면 vLLM 은 KV 프로파일링을 건너뛰므로 `gpu-memory-utilization` 은 **KV 사이징에는 쓰이지 않는다**
(분율이 아니라 절대 바이트가 제어). gmu 가 남는 자리는 아래 둘째 항목이다.

```text
Phase 2 총 VRAM = weights + non_kv_overhead + kv_cache_memory_bytes     ← gmu로 나누지 않음
클램프 천장     = kv_cache_memory_bytes ≤ budget_gib × deploy_gmu × GiB − weights/TP − overhead/TP
검증 게이트     = (weights + overhead) ≤ budget_gib × gate_margin × GiB          ← sim_classify(트라이얼 분류)
```

- **승수는 역할이 둘이다**(2026-09-14 · plan_26091407 §4.3 · 사용자 결정 Q2·Q9): `deploy_gmu` = `target_gpu.target_gmu`
  (배포 yaml `gpu-memory-utilization` 이자 클램프 천장 승수 — safe_gmu 의 정본이며 신설 필드가 아니다) ·
  `gate_margin` = `config.safety_margin`(트라이얼 검증 게이트 승수 · 되먹임 `safety_margin_threshold`). 종전에는
  `recipe.py` 의 `margin` 변수 하나가 넷을 겸했고, 타겟 흐름에서는 target_gmu 가 그 자리로 들어가 게이트 승수까지 바꿨다.
  수렴 lockset 의 `gmu_roles` 가 두 값과 출처를 나란히 싣는다.
- **Phase 1 공식과 다름**: Phase 1 은 `(…)/gmu` 로 나눴다. Phase 2 는 절대값 합이다. 혼동 금지.
- **최종 recipe 는 gpu-memory-utilization + kv-cache-memory-bytes 를 함께 emit 한다 (KV 절대클램프 따름정리, E2E 실증)**:
  KV 는 측정된 절대 클램프가 제어하며 이게 **이식성**을 준다(gmu-derived KV 는 호스트 VRAM 차이로 비이식).
  **단 gmu 도 필수** — vLLM 소스(`gpu_worker.determine_available_memory`·`utils.request_memory`)상 클램프를 주면 KV
  프로파일링 전체를 건너뛰고, gmu 는 **기동 전 `free ≥ ceil(total×gmu)` 검사**에만 관여한다. GB10 등 통합메모리
  (free/total≈0.91)는 OS ~11GiB 점유로 기본 `0.92` 가 그 검사에서 막힌다(`Free memory < desired GPU memory utilization`)
  → **통합메모리 gmu ≤ `0.90` 명시 필수**(값은 `deploy_gmu` = `target_gpu.target_gmu`). ∴ gmu=기동 전 free 검사, clamp=KV 사이징·이식성.
- **관측과 기전을 가른다**(2026-09-14 정정 · plan_26091407 F5): 종전 이 절은 gmu 가 "startup 검증 + **총 cap**" 이라고
  한 문장의 기전으로 적었다. 총량 cap·할당자 cap 을 거는 코드는 소스에 **없다**. 반면 같은 셀에서 gmu 0.85→0.80 이
  5,562MiB 의 여유를 연 **관측**(`perf_26091305`)은 사실이다 — gmu 가 총량 cap **처럼 작용한** 관측은 남기고 기전은
  미확정으로 둔다. 생성 yaml 의 주석도 같은 분리로 적는다(`gen_recipe_set.py` · "총량 cap 은 관측된 작용"). 그래서 서빙
  스모크의 `budget_preflight --declared-gmu` 는 예상 vLLM 몫(gmu × MemTotal)·잔차를 **기재만** 하고 게이트로 쓰지 않는다
  (overhead(gmu) 함수형은 기재가 쌓인 뒤 별도 결정 · plan §9 R4).
  ⚠ **헌법층 미결**: 정책 `KV_ABSOLUTE_CLAMP_PORTABILITY.C2` 문장(`.claude/policies/registry.yaml` — "startup free-memory gate
  and total cap")과 그 술어(`claim_predicates.py` C2 — 생성 주석 줄에 `startup free-memory 게이트`·`cap` 을 요구)는 이 단계에서
  고치지 않았다(plan §1 범위 밖 · 기초레이어). 위 문장은 C2 를 부정하지 않고 그 "total cap" 을 관측으로 한정한다. C2 문장의
  관측/기전 분리와 술어 앵커 강화는 사람 결정(HITL) 후속이다.
- 이식성 = 선언된 절대 필요량(weights+overhead+kv) 이상 GPU 서 동일 구동(작은 GPU 자동맞춤 ✗, GPU당 값이라 TP 의존).

## 2. 타겟-GPU 이식형 예산 (host≠target — `plan_26070809_47_07`)

사용자가 산출 host 와 **다른** 타겟 GPU 를 명시하면(config `target_gpu` 블록) Phase-1(estimate/generate) 의 gmu-only
종료를 **거부**하고 Phase-2 측정경로를 강제한다(target_gpu 미정의 시 Phase-1 은 기존 host 흐름 그대로).

**host 흐름도 같은 해소 경로를 탄다**(2026-09-14 · plan_26091407 §4.3): `simulate` 는 target_gpu 가 없으면 manifest 에서
블록을 채운다 — `gpu_model` = manifest · `per_card_vram` = 통합메모리(references.md 헤더)면 `test_device_total_gib` 선언 >
`/proc/meminfo` 실측, discrete 는 선언 > references.md §4 역룩업(둘 다 없으면 exit 5) · `target_gmu` = `safety_margin`
**명시값 승계**(host 흐름에는 target_gmu 칸이 없다 — lockset `gmu_source=hand` 와 stderr 로 승계 사실을 표시 · 미선언이면
exit 5) · 통합메모리 0.90 하드클램프 유지 · `tp_divisor=1`(host == target). **선언 예산 `vram_budget_gb` 가 그 per_card 와
5% 넘게 다르면 simulate 는 exit 5 로 멈춘다** — 두 권위(선언 carve-out 과 노드 사실) 중 어느 쪽을 클램프 천장·검증 게이트에 쓸지
추측하지 않는다(실측이 선언을 조용히 덮으면 24 GiB 로 선언한 config 가 노드 전체 크기의 클램프를 rc 0 으로 낸다). simulate 의
carve-out(가상 예산)은 `target_gpu` 블록으로 선언하고, 노드 전체가 예산이면 `vram_budget_gb` 를 맞춘다. 허용오차 안이면 정합
사실이 `gmu_roles.budget_source` 에 적힌다. host 흐름에서 배포 gmu 만 따로 선언하는 칸은 아직 없다 — 두 역할을 다른 값으로 두려면
`target_gpu` 블록(이 노드 `gpu_model` 포함)을 선언한다.

```yaml
target_gpu:                    # (선택) 있으면 이식형 타겟 예산 경로 활성
  gpu_model: "NVIDIA RTX PRO 6000"   # references.md §4 테이블 역룩업 키(per-card VRAM 해소)
  per_card_vram_gib: 96              # (선택) 테이블 미등재/오버라이드 시 사용자 명시
  cards_per_node: 1                  # 기본 1. N장은 명시.
  target_gmu: 0.90                   # 필수(기본값 없음 — 미선언 exit 5). 배포 gmu(deploy_gmu)의 정본. 통합메모리 타겟은 ≤0.90 하드클램프.
```

**per-GPU 산정식**(동질 클러스터, TP=`cards_per_node`×node_count — manifest `len(nodes)`):

```
kv_clamp_perGPU = per_card_VRAM_bytes(gpu_model, references.md §4 역룩업) × target_gmu
                  − weights_total/TP − overhead_total/TP
```

`weights_total`·`overhead_total` = host 측정값(GPU-불변 기하량 — overhead 는 런타임 성질 일부 포함해 정량보증은 아니고
target-margin 쿠션). **1→N 외삽 금지** — 멀티노드 타겟은 양노드 실측(측정 TP == 타겟 TP 강제). 통합메모리 타겟만
`target_gmu>0.90` 거부·0.90 하향+HITL, discrete 는 in-scope. batch 는 이 산정식이 아니라 §3 의 max-num-seqs 산식
(선언 요구 + 엔진 보고 KV 토큰의 KV-fit)으로 정한다(공식 per-token batch 금지). 트리플렛은 host↔target **동일 attention
backend** 를 이전 전제로 헤더에 명시.

## 3. per-token KV — 공식 vs 측정

- `per_token_kv_bytes = 2 × num_hidden_layers × num_key_value_heads × head_dim × kv_dtype_bytes`
  (kv_dtype_bytes: KV quant 없으면 2, `fp8` 이면 1).
- `required_kv = per_token_kv_bytes × max_model_len × batch`(손레버 batch · 구 lockset · batch 미정이면 1), batch 를 아래 산식으로
  정했으면 `per_token × max(max_model_len, L × batch) × 블록정렬 버퍼` — `max_safe_kv = int(budget × deploy_gmu × GiB) − weights − overhead`.
  클램프 값은 **required** 이고 max_safe 는 실행 가능 경계다(아래 "free 변수 결정" — required > max_safe 면 vram_infeasible 이라
  `min(required, max_safe)` 는 늘 required 다). plan §4.2 "required_kv 는 산출된 batch 로 재계산" 의 토큰 기준을 L 로 둔 이유:
  KV_fit@L 로 정한 batch 에 최악 길이 × batch 를 요구하면 산식이 자기 산출을 거부한다(L < max_model_len 이면 거의 항상
  vram_infeasible). `max(…, max_model_len)` 은 최악 길이 요청 1건이 KV 에 드는지를 보는 vLLM 기동 검사를 함께 담는다.
  `typical_request_tokens` 가 없으면 L = max_model_len 이라 종전 식과 같다.
- **측정 per-token KV 가 정본 — 공식은 거의 항상 과대추정(upper bound)**: 위 공식은 **full-attention 가정**이라
  sliding-window/GQA/hybrid attention(현대 모델 대다수)에서 per-token 을 **과대추정** → feasible batch 를 **과소추정**한다.
  0.23.0 듀얼모델 E2E 직접측정(`testlog_26062422`): **gemma-4** 공식 393KB vs 실측 ~50KB(@32768) = ~8× 과대 ·
  **gpt-oss-20b** 공식 48KB vs 실측 **26KB**(@32768) = **1.9× 과대**.
  ⚠ **"full-attention 이라 공식≈측정" 가정 금지** — gpt-oss-20b 는 GQA/sliding 이라 공식과 어긋난다.
  → **batch 는 어떤 모델이든 측정 토큰으로만 산정**한다: Phase-2 trial-loop 의 엔진 보고 `kv_cache_tokens`.
  공식 기반 batch 는 위험 — 과소추정하면 OOM, 과대추정하면(대다수) 용량 낭비(gpt-oss formula batch 46 = 실측 KV-fit 86 의 53%).
  측정 정본 우선순위: trial 로그 per-token(`Available KV cache memory`/`kv_cache_tokens`) > 공식 폴백(`recipe.py _resolve_clamp_kv`).
- **max-num-seqs 산식**(2026-09-14 · plan_26091407 §4.2 · 사용자 결정 Q5 — 종전 "near-max = floor(max_concurrency)" 정의를 대체):

  ```text
  max_num_seqs = min(concurrency_requirement, KV_fit@typical_request_tokens)
  KV_fit@L     = floor(kv_fit_tokens ÷ L)          kv_fit_tokens = 엔진 보고 `GPU KV cache size` 토큰(실측)
  ```

  입력은 셀 config `declared_axes.concurrency_requirement`·`typical_request_tokens`(선언 계층)이고 산출은 lockset `batch`·
  `batch_source`. 요구가 KV-fit 을 넘으면 KV-fit 으로 낮추고 사유를 적는다(요구를 지키려면 context·KV dtype·예산을 재조정해
  다시 돈다 — README loop-until-done). 선언 상태별: **요구 null/부재 → batch 미정**(max-num-seqs 미emit · vLLM 기본 admission
  상한 · 클램프는 max_model_len 1건 기준 · KV-fit 은 참고값 `batch_derivation.kv_fit` 으로만 기재) · L null/부재 → `max_model_len`
  으로 보수 산정(대체 사실 기재) · KV-fit 산출 불가 ∧ 요구 선언 → 요구를 **검증 없이** 채택(`verified=false`).
  `batch_source` ∈ {`declared-requirement`, `kv-fit-measured`, `hand-lever`} · batch 미정이면 null.
  요구 없음을 near-max 로 채우지 않는 이유(2026-09-14 리뷰 교정): 그 batch 의 required 는 배포 천장 전체라 선언 없는 config
  (캠페인 밖 기본 경로) 모두가 천장 클램프를 받는다 — 통합메모리에서는 run_trial 의 호스트 바닥 캡이 **클램프 트라이얼에는
  걸리지 않아** 워치독 사살 구성이 된다. 엔진 `max_concurrency`(최악 길이)도 plan §4.2 가 **기재**로 둔 보수 하한이지 산출값이
  아니다. 동시성을 원하면 `concurrency_requirement` 를 선언한다(요구 > KV-fit 이면 클램프가 천장에 닿을 수 있다 — 선언한 요구다).
  - 엔진 `max_concurrency` 는 **max_model_len 최악 길이** 기준이라 L 로 센 KV-fit 보다 작거나 같다 — **보수 하한으로 기재**만
    한다(`batch_derivation.engine_max_concurrency` · 산식 입력 ✗). L = max_model_len 이면 두 값이 같다.
  - vLLM 에서 `max_num_seqs` 는 admission 상한이고 KV 부족은 선점·재큐잉이다(오류 ✗) — KV-fit 초과는 불법이 아니라
    대기로 나타난다. 무릎(동시성 대비 처리량 포화)·열벽은 explorer 가 재지 않는다: `adversarial-benchmark` 스윕·노드
    블랙박스가 재서 `escalation_candidates`/hint input 으로 넘긴다(트리플렛 직접 쓰기 ✗). `max-num-batched-tokens` 모델링은
    범위 밖이다(후속).
  - 손레버: lockset 이 batch 를 들고 있고 `batch_source` 가 null·`hand-lever` 면 explorer 는 **덮어쓰지 않고** 같은 산식이
    냈을 값을 `derived_batch_would_be` 로만 기재한다(표시+대조 · Q1). 직전 explorer 산출(`declared-requirement`·
    `kv-fit-measured` batch, `measured-clamp` 클램프)은 승계하지 않고 비운 뒤 다시 잰다(carry-forward ✗).
- **절대 클램프 실측 절차(2-위상 · 최소 2-트라이얼)**: ① **trial1 언클램프 측정**(`kv_cache_memory_bytes=null`) → 로그에서
  weights·overhead·KV 바이트·`kv_cache_tokens`·`max_concurrency` 실측(`parse_vllm_log`) → ② **batch 산출**(위 산식 · 언클램프
  토큰은 배포 천장으로 환산: `max_safe ÷ (per_token × 블록정렬 버퍼)`) → ③ 그 batch 로 `required_kv` 를 재계산해
  `--kv-cache-memory-bytes` 산정 → ④ **trial2 클램프+batch 고정 재검증** — `sim_classify` 가 클램프 트라이얼의 엔진 보고
  토큰으로 KV-fit 을 다시 재고, batch 가 넘으면 `adjust_target=batch` 로 낮춰 한 번 더 돈다(클램프는 다시 산정하지 않는다 —
  batch↔클램프가 서로를 입력으로 삼는 순환을 **위상 분리**로 끊는다). 언클램프 통과만으로는 수렴이 아니다.
  - **첫 트라이얼이 vram_oom 이면**(워치독 SIGKILL·CUDA OOM — batch 산출 전) 루프가 max_model_len 1건 기준 **잠정 클램프**를
    잡는다. 그 트라이얼이 통과하면 위상 1 은 잠정 클램프의 엔진 토큰을 그대로 쓰지 않고 **배포 천장으로 환산**해 KV-fit 을 재고,
    산출 batch 로 required 를 재계산해 클램프를 다시 잡은 뒤 재검증한다(잠정 클램프의 작은 토큰으로 요구를 깎지 않는다). batch 를
    산출하지 않았으면(요구 없음) 잠정 클램프가 곧 최종 기준이라 그대로 수렴한다. 입력 lockset 의 **사람 클램프**는 배포값이므로
    그 엔진 토큰을 환산 없이 쓴다(`kv_source=hand`).
  - **cap**: 측정 → 클램프 검증 → 위상 2 재검증까지 기본 3칸이 든다. 첫 트라이얼 OOM·위상 2 하향이 겹치면 한 칸이 더 든다 —
    마지막 칸에서 batch 를 낮추면 HITL 요약 note 가 `cap(N) 소진 — 위상 2 …` 로 사유를 말한다(`--cap` 을 늘려 다시 돈다).
  수렴하면 `recipe.py simulate` 가 `provenance=explorer-phase2`·`batch_source`·`gmu_source`·`kv_source`·
  `trial_provenance`(measured|mock|dry-run)를 **기계 각인**하고, 서빙 yaml 에 `max-num-seqs` = 산출 batch 를 emit 한다. 각인이
  닿는 자리: `--lockset-out <경로>` · 미지정이면 `--candidate` 가 캠페인 셀 lockset(`campaigns/<id>/cells/<cell>/lockset.json`)일 때
  그 파일 · 그 밖이면 `run_summary.json` 의 lockset 칸뿐이다. `campaign_init --cell-set` 은 lockset 의 `trial_provenance` 와
  각인 여부(`lockset_stamp` = machine|self-declared|hand)를 cell.status 로 옮긴다(기재).
- **Phase-1.5 — serve KV-log 경량 측정**(전체 trial-loop 불요): 절대클램프(공식 상한 또는 보수값)로 **1회 serve** →
  `docker logs` 에서 `reserved … GiB … kv_cache_memory_bytes` + `GPU KV cache … N tokens` + `max_concurrency=X` grep →
  **실측 per-token = clamp_bytes ÷ kv_cache_tokens** · **KV-fit@L = floor(kv_cache_tokens ÷ L)**(L = max_model_len 이면
  floor(max_concurrency) 와 같다 — 보수 하한). batch 는 위 산식 `min(요구, KV-fit)` 이다. clamp 유지·batch 만 조정.
  (E2E gpt-oss @L=max_model_len 32768: clamp 69GiB→2,825,636 tokens→max_concurrency 86.23→**KV-fit 86**, 공식 46 의 ~2배). 로그 키명은 vLLM 버전 따라
  변할 수 있어 **fail-soft**(라인 부재 시 null→HITL 또는 Phase-2 폴백).
- **overhead 실측 vs 유도(gotcha)**: consolidated 라인(`model weights take …; non_torch …; reserved for KV Cache …`)이 있으면
  직접 산출. **없는 빌드(예 0.22.2 NGC)는** `Model loading took X GiB memory`(weights)·`Available KV cache memory`(kv)·
  `--gpu-memory-utilization=X`(gmu)만 있으므로 overhead 를 **유도**: `overhead = gmu_trial × test_device_total − weights − kv_available`
  (`recipe.py _enrich_overhead`).
- 루프의 free 변수 결정: **`kv_bytes = min(required_kv, max_safe_kv)`**. `required > safe` 면 max_model_len·batch 를 KV 로
  만족 불가 → **`vram_infeasible`(HITL)**.

## 4. 사전적재 인코딩 자산 (gpt-oss harmony/tiktoken)

gpt-oss류의 harmony/tiktoken o200k 인코딩은 이미지에 번들되지 않아 런타임 fetch 가 필요한데 **read-only 마운트 불변식상
런타임 fetch 는 금지**다(사전적재 원칙). → NAS 에 flat `tiktoken_cache/` 를 사전적재하고 컨테이너에 `/encodings:ro` 로 마운트
(compose 호스트경로 변수 `TIKTOKEN_HOST_PATH`)한 뒤, **정본 env `TIKTOKEN_ENCODINGS_BASE=/encodings` ·
`TIKTOKEN_RS_CACHE_DIR=/encodings`**(+`TIKTOKEN_ENABLED`)를 둘 다 마운트 경로로 가리킨다. 구식 `TIKTOKEN_ENCODINGS_PATH` 는 폐기 —
정본 동기화 대상 3곳(`docker-compose.template.yaml` · `config.example.yaml` · `run_trial.py`)을 맞춘다.

## 5. MoE 백엔드 on sm_121a(GB10/Blackwell) 따름정리

> 헌법 §모델별 서빙전략 독립 따름정리의 실현(carry-forward 금지). 올바른 moe-backend 는 **(모델×quant×하드웨어)에 종속**한다 —
> 한 모델의 교훈은 그 조합에 context-bounded, 전역 금지/전역 신뢰 ✗. 맥락이 갈리면 양쪽 다 참 → 조합마다 oracle/소스 독해로 재확립.

- **bf16 MoE**(예 Qwen3-Next-80B): 기본 `moe_backend=auto` 는 **flashinfer_cutlass** 를 고른다 → 그 CUTLASS MoE 커널이
  sm_121a용 prebuilt 부재 → 런타임 nvcc JIT(수십 커널)가 **고병렬=OOM-kill / 저병렬(MAX_JOBS↓)=단일커널 30분+ stall** 로
  둘 다 막힌다. → **`--moe-backend triton`** 명시(in-process Triton fused MoE, nvcc 불요)로 회피. Ray 분산이면 master serve
  에만 줘도 엔진config 가 slave 워커로 전파된다. 근거: 멀티노드 0.23.0 E2E combo③(testlog_26062501).
- **NVFP4(W4A4) MoE**(예 Qwen3.5-122B-A10B-NVFP4): **triton 은 미지원** — `--moe-backend triton` 을 주면 엔진 init 에서
  `ValueError: moe_backend='triton' is not supported for NvFP4 MoE` 로 즉사한다(supported = cutlass/flashinfer_*/marlin/emulation).
  → **플래그를 생략**하고 `moe_backend=auto` 의 vLLM NVFP4 oracle 선택에 위임하면 **FLASHINFER_CUTLASS**(NvFp4 변종,
  sm_121a prebuilt 존재)를 골라 서빙된다(122B-NVFP4 2노드 serve PASS — testlog_26062614 §2).
- **arch-walled 환경에선 `auto` 자체를 무비판 신뢰 ✗ (carry-forward 금지의 핵심)**: arch-wall 에선 `auto` 폴백이
  **MARLIN-repack → 통합메모리 OOM(호스트 하드다운)** 일 수 있다. ∴ MXFP4 대형 MoE(예 DeepSeek-V4 on sm_121)는 비-repack 경로
  **`--moe-backend humming` 을 oracle 독해로 명시**한다(`auto` 위임 ✗). 근거 = `testlog_26062823`.
- 값은 `MoEBackend` Literal(config/kernel.py) 참조.

## 6. 모델구동 런타임 패치 (`<model>_patch.py`)

**런타임-코드 불일치**(Python processor/config shim)는 이 스킬이 유도한다 — 참조-그라운디드로
`output/<topology>/configs/<model>_patch.py`(휘발·비추적)를 생성하고 `arm_patch.sh` 가 serve_runner 에서 자동 arm 한다.
헌법 `policy:RUNTIME_PATCH_NO_CARRY_FORWARD` — 이전 모델 패치를 다음 모델로 들고 가지 않는다.
**native lib/커널 의존은 여기가 아니라** `upstream-version-watch` 빌드평면(`build_patches/`)이 소유한다.
