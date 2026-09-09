# 3. 벤치 결과 — 그리고 무엇과 비교할 수 있나

> 아래 수치는 인증서에서 **파싱만** 한 것이다. 합성하지 않았고, 재계산하지 않았다.
> 출처: `benchmark_26090915_qwen3.8-27b_RTXPRO6000BlackwellServerEdition_0.28.0.yaml`

## 성능

| 항목 | 값 |
|---|---|
| `benchmark_mode` | full |
| `verdict` | PASS |
| `decode_tps_conc1` | 44.22 |
| `rubric_authority` | weak |
| `primary_source` | expected_achievable(roofline×MBU) |
| `primary_tps` | 11.29 |
| `floor_tps` | 9.6 |
| `tolerance` | 0.15 |
| `ratio_M_over_primary` | 3.917 |
| `spec_on` | false |
| `accept_len` | N/A |
| `sweep_levels` | 1,2,4,6 |
| `sweep_truncated` | N/A |
| `lite_included` | false |
| `lite_gen_tps_warm` | N/A |
| `lite_gen_src` | N/A |
| `lite_cold_ttft_ms` | N/A |
| `lite_kv_gib` | N/A |
| `measured_utc` | 2026-09-09T06:01:33Z |

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
| `quantization` | fp8 |
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
| `max_num_seqs` | 6 |
| `kv_cache_memory_bytes` | 54737327753 |
| `kv_cache_dtype` | fp8 |
| `gpu_memory_utilization` | 0.9 |
| `moe_backend` | N/A |
| `enforce_eager` | N/A |
| `ngc_base_tag` | N/A |

## 부하 스윕 곡선 — 동시성별 (결정론 파싱 · 손저작 ✗)

> 단일 running serve 에 **동시 요청 수만** 바꿔 잰 곡선이다(reload 없음 · request-rate=inf).
> `동시성=1` 행이 인증서의 판정점이며, 나머지 행은 그 레시피가 **부하에서 어떻게 되는지**를 말한다.
> 출처: `bench_report_26090915_qwen3.8-27b_RTXPRO6000BlackwellServerEdition_0.28.0.md` (bench_report) · 소수 2자리 표시 반올림 · 결측은 `N/A`(0 이 아니다).
> 이 표는 스크립트가 파싱해 렌더한다 — 손으로 옮긴 수치가 아니며 `--verify` 가 diff 0 을 요구한다.

| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50(ms) | ITL p50(ms) | 완료/실패 |
|---|---|---|---|---|---|---|
| 1 ★판정점 | 44.22 | 44.26 | 221.45 | 110.02 | 22.27 | 16/0 |
| 2 | 43.18 | 86.61 | 433.37 | 171.44 | 22.44 | 16/0 |
| 4 | 41.06 | 146.77 | 734.44 | 385.71 | 22.94 | 14/0 |
| 6 | 38.76 | 236.14 | 1181.60 | 570.89 | 23.65 | 18/0 |

_절삭된 레벨 없음(요청 전 레벨 완주)._


## 결손 기재

> ⚠ 아래 정보가 **부재한 채로 발행**됐다. 부재는 실패가 아니라 **기록**이며, 이 태그를
> 소비할 때 그만큼을 모른 채 소비한다는 뜻이다(강행 발행 정책 · plan_26090616 Q7/Q8).

| 사유코드 | 뜻 |
|---|---|
| `HINT_MISSING_PII_TERMS` | pii_terms.txt 부재 — 리터럴 스캔이 축소된 상태로 돌았다. |

## like-with-like 한정자 (Agent)

- **측정 조건**: input=1024 tok, output=256 tok(random synthetic), `--ignore-eos`(강제 256 토큰
  생성), `--backend openai`(완결 엔드포인트, chat 아님), warmup 2회 제외. 다른 input/output 길이·
  chat 엔드포인트·harmony 계열 파서를 쓰는 벤치와는 **직접 비교 불가**(TPOT 정의 자체가 갈린다).
- **동일 캠페인 내 자매 hint 와 비교**: `hint/0.28.0/qwen3.8-27b/rtxpro6000-1of2-262k-fp8w-kvnone`
  (셀 A, fp8가중치+KV무양자화)·`…-262k-bf16w-kvfp8`(셀 C, bf16가중치+fp8KV)와 **완전히 같은 측정
  스크립트·같은 GPU·같은 vLLM 버전**으로 쟀다 — 나란히 놓아도 안전하다. 셀 B가 최대동시(batch=6)
  집계처리량 1181.6 tok/s 로 셋 중 최고다(KV도 fp8라 동시접속 여유가 가장 큼).
- **다른 GPU/다른 vLLM 버전의 hint 와는 비교 금지**: 특히 `hint/…/qwen3.8-27b/gb10-*`(DGX Spark
  GB10, 통합메모리, 물리 대역폭이 이 GPU 의 ~1/17)와는 절대치를 나란히 놓지 마라 — 그쪽 캠페인
  승자가 18.69 t/s 인 것과 이쪽 44.22 t/s 는 하드웨어가 다른 결과이지 레시피 우열이 아니다.
- **YaRN 확장 컨텍스트(524288/1M)와 비교 불가**: 이 hint 는 네이티브 262144 컨텍스트 한정이다.
  YaRN 축은 이 캠페인에서 도구 한계(`--hf-overrides` 미지원)로 측정하지 못하고 보류됐다(런타임
  패치가 필요 — 02-narrative.md 미포함).
- **재현 시 필수 재확인**: `tensor_parallel_size=1`(GPU 1장, 2장 중 1장만) · 이 셀은
  `attention_backend=flashinfer` 로 실측(선언과 일치, `sweep_index.json` 확인)됐지만, 같은 env
  선언(`VLLM_ATTENTION_BACKEND=FLASHINFER`)을 쓴 자매 셀 A 는 `flash_attn` 으로 실측돼 선언과
  어긋났다 — vLLM 0.28.0 이 이 env var 를 "Unknown environment variable" 경고와 함께 사실상
  무시하고(구버전 이름일 가능성) 자체 휴리스틱으로 백엔드를 고르는 것으로 보인다. **재현 시
  이 env 변수를 신뢰하지 말고, 원하는 백엔드를 명시하는 vLLM 0.28.0 의 정확한 방법(CLI 플래그 등)
  을 먼저 확인하라** — 미확인 후속 과제로 남는다.
