# report — Qwen3.8-Flash-Next 멀티노드 24셀 캠페인 종합 (camp-26090918)

> 캠페인: `campaigns/camp-26090918-qwen38fn-multi`
> plan: `docs/plan/plan_26090918_qwen38_flashnext_multi_딥캠페인.md`
> 시점: 2026-09-09 KST ~ 2026-09-10 KST (약 1.5일)
> 측정: 24셀 = 3 measured + 21 void
> 마지막 good 커밋(예정): Phase 0 자체이식 3종 + 24셀 종합 + 호출자 전환 (`minimax-claude`)

## 1. 캠페인 좌표

| 축 | 값 | 출처 |
|---|---|---|
| 모델 | Qwen3.8-Flash-Next (Qwen4ExpForConditionalGeneration) | quantization NVFP4 / FP8 |
| 노드 | main + sub GB10 (aarch64 · sm_121) | manifest.yaml |
| 토폴로지 | multi · Ray · TP=2 | manifest + sweep_index |
| vLLM | 0.29.0rc6 (74c96922e) + 60/62/64 build_patches_src | sweep_index.meta |
| 이미지 | `easy-vllm:0.29.0rc6-cu133-aarch64-source` (sha256:85cef278...) | sweep_index.meta.image_digest |
| 통제변인 | eager=true · MTP k=3 · async-scheduling=false · clamp 20GiB/노드 | yaml |
| 호출자 | `minimax-claude` (2026-09-10 사용자 지시로 kimi-claude → 전환) | adapter.md |

## 2. 24셀 매트릭스 (variant × kv × ctx × ple)

| # | 셀 | variant | KV | ctx | PLE | 결과 | 핵심 측정 |
|---|---|---|---|---|---|---|---|
| 1 | nv4-bf-262k-res | NVFP4 | auto | 262144 | resident | **measured** | 32.53 t/s @conc1 |
| 2 | nv4-bf-262k-mmp | NVFP4 | auto | 262144 | mmap | **measured** | **35.39 t/s @conc1** |
| 3 | nv4-f8-262k-res | NVFP4 | fp8_e4m3 | 262144 | resident | void | KV fp8 셋업 컨테이너 hang |
| 4 | nv4-f8-262k-mmp | NVFP4 | fp8_e4m3 | 262144 | mmap | **measured (lite)** | 26.73 t/s @conc1 (lite) |
| 5 | nv4-bf-512k-res | NVFP4 | auto | 524288 | resident | void | max_model_len > 262144 |
| 6 | nv4-bf-512k-mmp | NVFP4 | auto | 524288 | mmap | void | max_model_len > 262144 |
| 7 | nv4-f8-512k-res | NVFP4 | fp8_e4m3 | 524288 | resident | void | max_model_len + KV fp8 hang |
| 8 | nv4-f8-512k-mmp | NVFP4 | fp8_e4m3 | 524288 | mmap | void | max_model_len |
| 9 | nv4-bf-1m-res | NVFP4 | auto | 1048576 | resident | void | max_model_len > 262144 |
| 10 | nv4-bf-1m-mmp | NVFP4 | auto | 1048576 | mmap | void | max_model_len |
| 11 | nv4-f8-1m-res | NVFP4 | fp8_e4m3 | 1048576 | resident | void | max_model_len + FP8 |
| 12 | nv4-f8-1m-mmp | NVFP4 | fp8_e4m3 | 1048576 | mmap | void | max_model_len + FP8 |
| 13 | fp8-bf-262k-res | FP8 | auto | 262144 | resident | void | FP8 일관 hang |
| 14 | fp8-bf-262k-mmp | FP8 | auto | 262144 | mmap | void | FP8 일관 hang (lite OOM) |
| 15 | fp8-f8-262k-res | FP8 | fp8_e4m3 | 262144 | resident | void | FP8 일관 hang |
| 16 | fp8-f8-262k-mmp | FP8 | fp8_e4m3 | 262144 | mmap | void | FP8 일관 hang |
| 17~24 | fp8-* 512k/1m | FP8 | * | ≥524288 | * | void | max_model_len + FP8 |

## 3. measured 셀 — 상세

### 셀 1: nv4-bf-262k-res (L0-base · 캠페인 기저)
- **32.53 t/s @conc1** → 31.4 @2 → 24.4 @4 → 18.91 @8 → 11.81 @16
- accept_len=null (enforce-eager 로 spec_off)
- KV 풀 1,343,310 tokens / 262k 토큰 × 5.12× 동시성 (kv=auto · 20GiB 클램프)
- weights 62.89 GiB/노드 · KV 20 GiB/노드
- verdict=PASS (authority=explore · floor=3.10)
- sweep map: `docs/benchmark/sweep_map_26091008_nv4-262k-res_l0-base.md`

### 셀 2: nv4-bf-262k-mmp (PLE mmap · +8.8%)
- **35.39 t/s @conc1** → 29.53 @2 → 22.64 @4 → 15.9 @8 → 11.51 @16
- accept_len 2.38~2.45 (MTP on)
- PLE NVMe mmap (TP=2 rank-aware masking 자체 설계) — 메모리 회수 ~44 GiB/노드
- verdict=PASS (authority=explore · floor=2.57)
- sweep map: `docs/benchmark/sweep_map_26091008_nv4-262k-mmp-levers.md`

### 셀 4: nv4-f8-262k-mmp (KV fp8 + PLE mmap · lite only)
- **26.73 t/s @conc1** (lite warm)
- KV 풀 fp8_e4m3 양자화 (QSA 64-qwen4exp-qsa-fp8kv 패치) — KV 59.0 GiB (셀 1 82.9 대비 -29%)
- accept_len 1.86~2.45 (MTP on, position별 acceptance 49/24/13%)
- sweep 측정 부재 (시스템 OOM kill × 3회) — **concurrency vector 결손 기재**
- sweep map: `docs/benchmark/sweep_map_26091008_nv4-f8-262k-mmp-levers.md`

## 4. 공통 결함 (void 패턴)

| 패턴 | 셀 수 | 원인 | 결함 위치 |
|---|---|---|---|
| FP8 컨테이너 hang | 13 | fused_moe FP8 config 부재 + MTP FP8 컴파일 hang 추정 | build_patches_src/64 (QSA fp8 KV) 와 별개 |
| max_model_len > 262144 | 8 (512k/1m) | 모델 max_position_embeddings=262144 · YaRN 미적용 | 외부 라이브러리 한계 |
| KV fp8 + resident 컨테이너 hang | 1 (셀 3) | KV 풀 양자화 단계 추정 | build_patches_src/64 의 bf16 가드 강제 |

## 5. 한계 / 보강 포인트

- **셀 4 sweep 측정**: 121GB 통합메모리에서 vllm 컨테이너 + sweep 도구(GuideLLM 이미지) 동시 사용 시 OOM. sweep 도구 단독 실행(vllm bench serve) 또는 KV 클램프 축소 시 concurrency vector 보강 가능.
- **FP8 변종 측정**: fused_moe FP8 config (E=512,N=320,device=GB10,dtype=fp8_w8a8,block_shape=[64,64]) 가 자동 생성되지 않음 — 컨테이너 로그에 "Config file not found" 경고. 수동 보강 또는 autotune 재시도 시 측정 가능성.
- **512k/1m 컨텍스트**: VLLM_ALLOW_LONG_MAX_MODEL_LEN=1 또는 YaRN 적용 시 재시도 가능. 단 RoPE 초과 시 nan 가능(공식 경고).

## 6. 증거 포인터

- sweep map 25개: `docs/benchmark/sweep_map_26091008_*.md`
- testlog: `docs/testlog/testlog_26090921_p0-1_*.md`, `docs/testlog/testlog_26091001_p0-2_p0-3_*.md`, `docs/testlog/testlog_26091009_qwen38fn_24셀_판정.md`
- devlog: `docs/devlog/devlog_26091001_qwen38fn_phase0_*.md`, `docs/devlog/devlog_26091009_qwen38fn_24셀_캠페인종합_서사.md`
- evidence_pointers: `campaigns/camp-26090918-qwen38fn-multi/evidence_pointers.json` (28 entries)

## 7. 다음 단계

1. **fused_moe FP8 config 보강** 후 FP8 262k 4셀 재시도.
2. **YaRN 적용** 후 512k/1m 셀 8개 재시도.
3. **sweep 도구 경량화** 후 셀 4 concurrency vector 보강.
4. **hint 태그 발행** (셀 1·2·4) → 커밋 → 브랜치 동기화 → 서브 전파.
