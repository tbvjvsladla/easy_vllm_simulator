# 3. 벤치 결과 — 그리고 무엇과 비교할 수 있나

> 아래 수치는 인증서에서 **파싱만** 한 것이다. 합성하지 않았고, 재계산하지 않았다.
> 출처: `benchmark_26092112_qwen3-4b_H10080GBHBM3_0.27.1.yaml`

## 성능

| 항목 | 값 |
|---|---|
| `benchmark_mode` | full |
| `verdict` | PASS |
| `decode_tps_conc1` | 219.67 |
| `rubric_authority` | weak |
| `primary_source` | E(external_reference) |
| `primary_tps` | 214.0 |
| `floor_tps` | 181.9 |
| `tolerance` | 0.15 |
| `ratio_M_over_primary` | 1.026 |
| `spec_on` | false |
| `accept_len` | N/A |
| `sweep_levels` | 1,2,4,8 |
| `sweep_truncated` | N/A |
| `lite_included` | true |
| `lite_gen_tps_warm` | 235.71 |
| `lite_gen_src` | median_tpot |
| `lite_cold_ttft_ms` | 61.0 |
| `lite_kv_gib` | 12.55 |
| `measured_utc` | 2026-09-21T03:16:30Z |

## 측정 구성 — 무엇으로 쟀나(기재 · 게이트 아님)

> 도구·버전이 다르면 수치를 나란히 놓기 전에 조건부터 본다. 부재 키는 그 시점에 그 필드가
> 없었다는 뜻이다(합성하지 않는다).

| 항목 | 값 |
|---|---|
| `bench_mode` | full |
| `bench_mode_kind` | full |
| `도구` | guidellm 0.7.3 |
| `반복 N` | 3 |
| `판정점 완주` | 3 |
| `downgrade_reason` | 미기재 |
| `측정 구성 출처` | bench_report(bench_report_26092112_qwen3-4b_H10080GBHBM3_0.27.1.md) |
| `bench_tool` | guidellm |
| `bench_tool_version` | 0.7.3 |
| `bench_tool_version_source` | measured(benchmarks.json metadata.guidellm_version) |

## 강한 일치 키 — 하나라도 다르면 이 수치는 **무효**다

| 키 | 값 |
|---|---|
| `model` | qwen3-4b |
| `gpu_model` | NVIDIA H100 80GB HBM3 |
| `vllm_version` | 0.27.1 |
| `quantization` | none |
| `topology` | single |
| `tensor_parallel_size` | 1 |

## 소프트 지문 — 다르면 stale, 재측정 권고

| 키 | 값 |
|---|---|
| `driver_version` | 595.71.05 |
| `cuda_version` | 132 |
| `image_tag` | N/A |
| `image_digest` | N/A |
| `max_model_len` | 32768 |
| `max_num_seqs` | 8 |
| `kv_cache_memory_bytes` | N/A |
| `kv_cache_dtype` | auto |
| `gpu_memory_utilization` | 0.28 |
| `moe_backend` | N/A |
| `enforce_eager` | N/A |
| `ngc_base_tag` | N/A |

## 부하 스윕 곡선 — 동시성별 (결정론 파싱 · 손저작 ✗)

> 단일 running serve 에 **동시 요청 수만** 바꿔 잰 곡선이다(reload 없음 · request-rate=inf).
> `동시성=1` 행이 인증서의 판정점이며, 나머지 행은 그 레시피가 **부하에서 어떻게 되는지**를 말한다.
> 출처: `bench_report_26092112_qwen3-4b_H10080GBHBM3_0.27.1.md` (bench_report) · 소수 2자리 표시 반올림 · 결측은 `N/A`(0 이 아니다).
> 이 표는 스크립트가 파싱해 렌더한다 — 손으로 옮긴 수치가 아니며 `--verify` 가 diff 0 을 요구한다.

| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50(ms) | ITL p50(ms) | 완료/실패 |
|---|---|---|---|---|---|---|
| 1 ★판정점 | 219.67 | 219.45 | 1098.12 | 51.16 | 4.37 | 16/0 |
| 2 | 219.42 | 436.51 | 2184.23 | 24.54 | 4.48 | 16/0 |
| 4 | 213.09 | 645.63 | 3230.67 | 27.26 | 4.60 | 15/0 |
| 8 | 206.48 | 1127.17 | 5640.26 | 26.60 | 4.76 | 16/0 |

_절삭된 레벨 없음(요청 전 레벨 완주)._


## 결손 기재

_결손 없음 — 이 절이 요구하는 증거가 모두 도착했다._

## like-with-like 한정자 (Agent)

**이 수치와 비교 가능한 것**

- **같은 측정 조건**의 수치만 나란히 놓을 수 있다: 입력 1,024 / 출력 256 토큰 · 반복 3 ·
  동시성 1·2·4·8 · 요청 포맷은 **완결 엔드포인트** · `--ignore-eos` · 요청마다 고유 프롬프트
  (프리픽스 캐시 이득 배제). 이 중 하나라도 다르면 같은 모델·같은 하드웨어라도 배수로 갈린다.
- 정본 지표는 **스트림당 decode tok/s**(= 1000 / median TPOT)다. 집계 처리량(총 tok/s)과
  혼동하면 안 된다 — 동시성이 오르면 전자는 내려가고 후자는 올라간다.

**비교할 수 없는 것**

- **다른 컨텍스트 길이**. 이 측정은 단문(1K 입력)이다. 압축 KV 의 이득과 대가는 컨텍스트가 길수록
  커지므로, 10K·30K 입력 수치와 직접 견주면 안 된다.
- **다른 엔진 버전의 hint**. 이 조합은 엔진 버전이 **정확히** 맞아야 성립한다(플러그인이 그 버전
  내부에 결합한다). 버전이 다른 태그의 수치는 조건이 같은지 확인하기 전에는 비교 불가다.
- **다른 하드웨어**. 절대 처리량은 이 GPU·이 드라이버 한정이다. 이월하려면 재측정하라.
- **워밍업을 상쇄하지 않은 단일 run**. 이 장비의 동시성 1 재현 밴드는 약 2.8% 인데 그 폭은 무작위
  산포가 아니라 **첫 run 이 낮고 이후 안착하는 계단**이다. 단일 run 끼리 견주면 그 계단이 차이로
  오독된다 — run 번호를 짝지어 비교하라.

**게재 성격**

- 이 절은 **관측 게재**이지 권고나 기준선이 아니다. 여기 적힌 수치를 SLA·합격선으로 승격하지 마라.
