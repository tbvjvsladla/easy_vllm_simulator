# 3. 벤치 결과 — 그리고 무엇과 비교할 수 있나

> 아래 수치는 인증서에서 **파싱만** 한 것이다. 합성하지 않았고, 재계산하지 않았다.
> 출처: `benchmark_26090915_09_18_qwen3.8-27b_RTXPRO6000BlackwellServerEdition_0.28.0.yaml`

## 성능

| 항목 | 값 |
|---|---|
| `benchmark_mode` | full |
| `verdict` | PASS |
| `decode_tps_conc1` | 26.83 |
| `rubric_authority` | weak |
| `primary_source` | expected_achievable(roofline×MBU) |
| `primary_tps` | 11.29 |
| `floor_tps` | 9.6 |
| `tolerance` | 0.15 |
| `ratio_M_over_primary` | 2.376 |
| `spec_on` | false |
| `accept_len` | N/A |
| `sweep_levels` | 1,2,3 |
| `sweep_truncated` | N/A |
| `lite_included` | false |
| `lite_gen_tps_warm` | N/A |
| `lite_gen_src` | N/A |
| `lite_cold_ttft_ms` | N/A |
| `lite_kv_gib` | N/A |
| `measured_utc` | 2026-09-09T06:09:18Z |

## 측정 구성 — 무엇으로 쟀나(기재 · 게이트 아님)

> 도구·버전이 다르면 수치를 나란히 놓기 전에 조건부터 본다. 부재 키는 그 시점에 그 필드가
> 없었다는 뜻이다(합성하지 않는다).

| 항목 | 값 |
|---|---|
| `bench_tool` | guidellm |
| `bench_tool_version` | 0.7.3 |
| `bench_tool_version_source` | measured(benchmarks.json metadata.guidellm_version) |

## 강한 일치 키 — 하나라도 다르면 이 수치는 **무효**다

| 키 | 값 |
|---|---|
| `model` | qwen3.8-27b |
| `gpu_model` | NVIDIA RTX PRO 6000 Blackwell Server Edition |
| `vllm_version` | 0.28.0 |
| `quantization` | none |
| `topology` | single |
| `tensor_parallel_size` | 1 |

## 소프트 지문 — 다르면 stale, 재측정 권고

| 키 | 값 |
|---|---|
| `driver_version` | 595.71.05 |
| `cuda_version` | 132 |
| `image_tag` | vllm-src-022:clean |
| `image_digest` | N/A |
| `max_model_len` | 262144 |
| `max_num_seqs` | 3 |
| `kv_cache_memory_bytes` | 27386562564 |
| `kv_cache_dtype` | fp8 |
| `gpu_memory_utilization` | 0.9 |
| `moe_backend` | N/A |
| `enforce_eager` | N/A |
| `ngc_base_tag` | N/A |

## 부하 스윕 곡선 — 동시성별 (결정론 파싱 · 손저작 ✗)

> 단일 running serve 에 **동시 요청 수만** 바꿔 잰 곡선이다(reload 없음 · request-rate=inf).
> `동시성=1` 행이 인증서의 판정점이며, 나머지 행은 그 레시피가 **부하에서 어떻게 되는지**를 말한다.
> 출처: `bench_report_26090915_09_18_qwen3.8-27b_RTXPRO6000BlackwellServerEdition_0.28.0.md` (bench_report) · 소수 2자리 표시 반올림 · 결측은 `N/A`(0 이 아니다).
> 이 표는 스크립트가 파싱해 렌더한다 — 손으로 옮긴 수치가 아니며 `--verify` 가 diff 0 을 요구한다.

| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50(ms) | ITL p50(ms) | 완료/실패 |
|---|---|---|---|---|---|---|
| 1 ★판정점 | 26.83 | 26.86 | 134.38 | 197.26 | 36.65 | 16/0 |
| 2 | 24.31 | 48.77 | 244.04 | 311.72 | 39.83 | 16/0 |
| 3 | 23.70 | 71.44 | 357.47 | 499.74 | 40.40 | 18/0 |

_절삭된 레벨 없음(요청 전 레벨 완주)._


## 결손 기재

_결손 없음 — 이 절이 요구하는 증거가 모두 도착했다._

## like-with-like 한정자 (Agent)

- **측정 조건**: input=1024 tok, output=256 tok(random synthetic), `--ignore-eos`, `--backend
  openai`(완결 엔드포인트), warmup 2회 제외 — 자매 hint(A/B)와 완전히 동일.
- **동일 캠페인 자매 hint와 비교 가능**: `…qfp8-len262144-kvauto`(셀 A, fp8가중치/KV무양자화)·
  `…qfp8-len262144-kvfp8`(셀 B, fp8가중치/fp8KV, 이 캠페인 승자)와 같은 스크립트·GPU·vLLM
  버전. 단일스트림 속도(26.83)는 A(43.91)·B(44.22) 대비 약 40% 낮다 — **차이는 가중치
  양자화 유무(bf16 vs fp8)에서 난다**, KV 양자화(이 셀도 fp8 로 A·B의 절충)가 아니라. 동시접속
  한계(batch=3)는 A와 동일 — KV 무양자화 A(batch=3, KV 2배)와 이 셀(batch=3, bf16 가중치 2배)이
  다른 원인으로 같은 batch 상한에 도달했다는 점에 유의.
- **다른 GPU/버전의 hint와 비교 금지**: GB10 통합메모리 계열 등(대역폭 규모가 다름).
- **재현 시 필수 재확인**: `tensor_parallel_size=1` 필수(자매 hint 참조). 이 셀의 루프라인
  판정은 `roofline.py` 의 가중치양자화 미인지 결함(02-narrative.md §되풀이하지 말 것 참조)을
  피하려 `--e-search empty`(외부 레퍼런스 미사용)로 재판정된 값이다 — 외부 커뮤니티 수치와
  직접 대조하지 마라(A/B와 동일한 재판정 사유).
