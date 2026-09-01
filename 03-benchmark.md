# 3. 벤치 결과 — 그리고 무엇과 비교할 수 있나

> 아래 수치는 인증서에서 **파싱만** 한 것이다. 합성하지 않았고, 재계산하지 않았다.
> 출처: `benchmark_26090117_17_21_gpt-oss-120b_NVIDIA GB10_0.19.1.yaml`

## 성능

| 항목 | 값 |
|---|---|
| `benchmark_mode` | full |
| `verdict` | PASS |
| `decode_tps_conc1` | 34.55 |
| `rubric_authority` | weak |
| `primary_source` | E(external_reference) |
| `primary_tps` | 33.53 |
| `floor_tps` | 28.5 |
| `tolerance` | 0.15 |
| `ratio_M_over_primary` | 1.03 |
| `spec_on` | false |
| `accept_len` | N/A |
| `sweep_levels` | 1,2,4,8,16 |
| `sweep_truncated` | N/A |
| `lite_included` | true |
| `lite_gen_tps_warm` | 34.52 |
| `lite_gen_src` | median_tpot |
| `lite_cold_ttft_ms` | 70.0 |
| `lite_kv_gib` | 30.0 |
| `measured_utc` | 2026-09-01T08:02:35Z |

## 강한 일치 키 — 하나라도 다르면 이 수치는 **무효**다

| 키 | 값 |
|---|---|
| `model` | gpt-oss-120b |
| `gpu_model` | NVIDIA GB10 |
| `vllm_version` | 0.19.1 |
| `quantization` | mxfp4 |
| `topology` | single |
| `tensor_parallel_size` | 1 |

## 소프트 지문 — 다르면 stale, 재측정 권고

| 키 | 값 |
|---|---|
| `driver_version` | 580.173.02 |
| `cuda_version` | 132 |
| `image_tag` | easy-vllm:0.19.1-cu130-aarch64-wheel |
| `max_model_len` | 131072 |
| `max_num_seqs` | 16 |
| `kv_cache_memory_bytes` | 32212254720 |
| `kv_cache_dtype` | fp8 |
| `gpu_memory_utilization` | 0.9 |
| `moe_backend` | N/A |
| `enforce_eager` | N/A |
| `ngc_base_tag` | N/A |

## like-with-like 한정자 (Agent)

### 이 수치의 측정 조건

동시성 **1** · 입력 길이 **2048** · 랜덤 데이터셋 · `ignore-eos` 가 실제로 먹는 엔드포인트
(`/v1/completions`). 정본 지표는 `decode_tps_conc1 = 1000/median_tpot` 이며 엔진 로그와
교차검증 ratio **0.99** 로 일치한다.

### 비교해도 되는 것

- **같은 하드웨어·같은 체크포인트·동시성 1 이 명시된 단일 스트림 수치.** 판정에 쓴 E=33.53 이
  그런 값이고, 우리 M=34.55 와 ratio 1.03 으로 사실상 동률이다. 즉 **이 조합의 정상 성능대는
  33~35 t/s** 라고 보면 된다.
- **부하 스윕은 같은 실행에서 나온 값끼리** 비교하라 — 절삭 0건이므로 레벨 간 비교는 유효하다.
  동시성 16 에서 총 출력 **151.32 t/s** 로 개별 처리량을 총 처리량과 맞바꾼다.

### 비교하면 안 되는 것

- **⚠ `hint/0.26.0/gpt-oss-120b/gb10-single`(11.24 t/s)과 직접 비교하지 마라.** 입력 길이가
  다르다(2048 vs 1024). 그 hint 자신이 이 모델의 컨텍스트 민감도가 크다고 경고한다. 따라서
  **"0.19.1 이 3배 빠르다"는 미확정**이며, 확정하려면 같은 입력 길이로 재측정해야 한다.
  이번에는 측정하지 않았다 — **모르는 것을 모른다고 적는다.**
- **60 t/s 류의 공개 수치와 비교하지 마라.** 그것은 높은 동시성에서의 **총합**이지 단일 스트림이
  아니다. 이 하드웨어의 배치1 절대 상한은 아래 검산으로 44.31 t/s 다.
- **다른 엔드포인트로 측정된 수치와 비교하지 마라.** 이 모델(harmony)은 chat 엔드포인트에서
  `ignore-eos` 가 무력해 생성이 조기 종료되고, TPOT 이 **3.3배** 부풀어 측정된다.

### 스스로 검산하는 법 (권장)

성능 주장을 받을 때 이 한 줄을 먼저 계산하라:

```
E × active_bytes  ≤  하드웨어 대역폭
```

이 조합의 active 는 배치1 디코드 기준 **5.738 GiB/token** 이다. 스펙 대역폭을 이 값으로 나눈
**44.31 t/s** 가 절대 상한이고, 우리 34.55 는 그 **77.9%**(MBU)다. 상한을 넘는 E 를 제시하는
비교 대상은 측정 조건이 다른 것이지 우리가 느린 것이 아니다.

**이 검산은 게이트가 하지 않는다** — 1차 판정에서 물리적으로 불가능한 E 가 그대로 통과해
REFUTE 가 났고, 사람이 손으로 검산해서 잡았다. 소비자도 같은 검산을 해야 한다.
