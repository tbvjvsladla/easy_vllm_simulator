# 절대 KV 클램프 · 이식형 타겟-GPU 예산 · MoE 백엔드 (조건부 reference — Phase 2 의 핵심)

> spine step 8(trial-loop)의 **산정식·측정 정본·백엔드 함정**. 헌법 `policy:KV_ABSOLUTE_CLAMP_PORTABILITY`
> 의 스킬-측 상세. 여기 수치·공식은 **측정이 정본이고 공식은 상한**이라는 원칙 아래서만 유효하다.

## 1. 절대 KV 클램프

vLLM 인자 **`--kv-cache-memory-bytes`**(GPU당 KV 바이트, 정수)로 KV 캐시를 **절대값으로 고정**한다.
이 인자를 주면 **`gpu-memory-utilization` 은 무시**된다(분율이 아니라 절대 바이트가 제어).

```text
Phase 2 총 VRAM = weights + non_kv_overhead + kv_cache_memory_bytes     ← gmu로 나누지 않음
검증 게이트     = (weights + overhead + kv_bytes) ≤ budget_gib × safety_margin × GiB
```

- **Phase 1 공식과 다름**: Phase 1 은 `(…)/gmu` 로 나눴다. Phase 2 는 절대값 합이다. 혼동 금지.
- **최종 recipe 는 gpu-memory-utilization + kv-cache-memory-bytes 를 함께 emit 한다 (KV 절대클램프 따름정리, E2E 실증)**:
  KV 는 측정된 절대 클램프가 제어하며 이게 **이식성**을 준다(gmu-derived KV 는 호스트 VRAM 차이로 비이식).
  **단 gmu 도 필수** — vLLM 은 클램프 설정 시 gmu 를 *KV 사이징*에만 무시(`config/cache.py`)하고, **startup free-memory
  검증(`free ≥ gmu×total`)+총-cap 엔 여전히 사용**. GB10 등 통합메모리(free/total≈0.91)는 OS ~11GiB 점유로 기본 `0.92` 가
  startup OOM(`Free memory < desired GPU memory utilization`) → **통합메모리 gmu ≤ `0.90`(=`safety_margin`) 명시 필수**.
  ∴ gmu=startup/총-cap 게이트, clamp=KV 사이징·이식성.
- 이식성 = 선언된 절대 필요량(weights+overhead+kv) 이상 GPU 서 동일 구동(작은 GPU 자동맞춤 ✗, GPU당 값이라 TP 의존).

## 2. 타겟-GPU 이식형 예산 (host≠target — `plan_26070809_47_07`)

사용자가 산출 host 와 **다른** 타겟 GPU 를 명시하면(config `target_gpu` 블록) Phase-1(estimate/generate) 의 gmu-only
종료를 **거부**하고 Phase-2 측정경로를 강제한다(target_gpu 미정의 시 기존 host 흐름 완전 보존 — 회귀 0).

```yaml
target_gpu:                    # (선택) 있으면 이식형 타겟 예산 경로 활성
  gpu_model: "NVIDIA RTX PRO 6000"   # references.md §4 테이블 역룩업 키(per-card VRAM 해소)
  per_card_vram_gib: 96              # (선택) 테이블 미등재/오버라이드 시 사용자 명시
  cards_per_node: 1                  # 기본 1. N장은 명시.
  target_gmu: 0.90                   # 기본 0.90. 통합메모리 타겟은 gmu≤0.90 하드클램프.
```

**per-GPU 산정식**(동질 클러스터, TP=`cards_per_node`×node_count — manifest `len(nodes)`):

```
kv_clamp_perGPU = per_card_VRAM_bytes(gpu_model, references.md §4 역룩업) × target_gmu
                  − weights_total/TP − overhead_total/TP
```

`weights_total`·`overhead_total` = host 측정값(GPU-불변 기하량 — overhead 는 런타임 성질 일부 포함해 정량보증은 아니고
target-margin 쿠션). **1→N 외삽 금지** — 멀티노드 타겟은 양노드 실측(측정 TP == 타겟 TP 강제). 통합메모리 타겟만
`target_gmu>0.90` 거부·0.90 하향+HITL, discrete 는 in-scope. batch 는 이 산정식이 아니라 measured `max_concurrency` 로만
(formula 금지). 트리플렛은 host↔target **동일 attention backend** 를 이전 전제로 헤더에 명시.

## 3. per-token KV — 공식 vs 측정

- `per_token_kv_bytes = 2 × num_hidden_layers × num_key_value_heads × head_dim × kv_dtype_bytes`
  (kv_dtype_bytes: KV quant 없으면 2, `fp8` 이면 1).
- `required_kv = per_token_kv_bytes × max_model_len × batch`, `max_safe_kv = int(budget×margin×GiB) − weights − overhead`.
- **측정 per-token KV 가 정본 — 공식은 거의 항상 과대추정(upper bound)**: 위 공식은 **full-attention 가정**이라
  sliding-window/GQA/hybrid attention(현대 모델 대다수)에서 per-token 을 **과대추정** → feasible batch 를 **과소추정**한다.
  0.23.0 듀얼모델 E2E 직접측정(`testlog_26062422`): **gemma-4** 공식 393KB vs 실측 ~50KB(@32768) = ~8× 과대 ·
  **gpt-oss-20b** 공식 48KB vs 실측 **26KB**(@32768) = **1.9× 과대**.
  ⚠ **"full-attention 이라 공식≈측정" 가정 금지** — gpt-oss-20b 는 GQA/sliding 이라 공식과 어긋난다.
  → **near-max batch 는 어떤 모델이든 측정으로만 산정**한다: Phase-2 trial-loop 또는 serve 로그의 `kv_cache_tokens`/`max_concurrency`.
  공식 기반 batch 는 위험 — 과소추정하면 OOM, 과대추정하면(대다수) 용량 낭비(gpt-oss formula batch 46 = 실측 near-max 86 의 53%).
  측정 정본 우선순위: trial 로그 per-token(`Available KV cache memory`/`kv_cache_tokens`) > 공식 폴백(`recipe.py _resolve_clamp_kv`).
- **절대 클램프 실측 절차(최소 2-트라이얼)**: ① **trial1 측정**(`kv_cache_memory_bytes=null`, 언클램프) → 로그에서 free_kv 실측 →
  ② `--kv-cache-memory-bytes` 로 환산 → ③ **trial2 클램프 검증**. 언클램프 통과만으로는 수렴이 아니다.
- **Phase-1.5 — serve KV-log 경량 측정**(전체 trial-loop 불요): 절대클램프(공식 상한 또는 보수값)로 **1회 serve** →
  `docker logs` 에서 `reserved … GiB … kv_cache_memory_bytes` + `GPU KV cache … N tokens` + `max_concurrency=X` grep →
  **실측 per-token = clamp_bytes ÷ kv_cache_tokens** · **near-max batch = floor(max_concurrency)**. clamp 유지·batch 만 상향.
  (E2E gpt-oss: clamp 69GiB→2,825,636 tokens→max_concurrency 86.23→**batch 86**, 공식 46 의 ~2배). 로그 키명은 vLLM 버전 따라
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
