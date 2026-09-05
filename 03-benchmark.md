# 3. 벤치 결과 — 그리고 무엇과 비교할 수 있나

> 아래 수치는 인증서에서 **파싱만** 한 것이다. 합성하지 않았고, 재계산하지 않았다.
> 출처: `benchmark_26090603_gpt-oss-20b_GB10_0.18.0.yaml`

## 성능

| 항목 | 값 |
|---|---|
| `benchmark_mode` | full |
| `verdict` | PASS |
| `decode_tps_conc1` | 45.54 |
| `rubric_authority` | explore |
| `primary_source` | expected_achievable(roofline×MBU) |
| `primary_tps` | 19.63 |
| `floor_tps` | 16.69 |
| `tolerance` | 0.15 |
| `ratio_M_over_primary` | 2.32 |
| `spec_on` | false |
| `accept_len` | N/A |
| `sweep_levels` | 1,2,4,8,16 |
| `sweep_truncated` | N/A |
| `lite_included` | true |
| `lite_gen_tps_warm` | 45.54 |
| `lite_gen_src` | median_tpot |
| `lite_cold_ttft_ms` | 519.1 |
| `lite_kv_gib` | 56.0 |
| `measured_utc` | 2026-09-05T18:12:05Z |

## 측정 구성 — 무엇으로 쟀나(기재 · 게이트 아님)

> 도구·버전이 다르면 수치를 나란히 놓기 전에 조건부터 본다. 부재 키는 그 시점에 그 필드가
> 없었다는 뜻이다(합성하지 않는다).

| 항목 | 값 |
|---|---|
| `bench_tool` | vllm-bench-serve |
| `bench_tool_version` | N/A |
| `bench_tool_version_source` | declared(도구가 버전을 자기보고하지 않는다) |

## 강한 일치 키 — 하나라도 다르면 이 수치는 **무효**다

| 키 | 값 |
|---|---|
| `model` | gpt-oss-20b |
| `gpu_model` | NVIDIA GB10 |
| `vllm_version` | 0.18.0 |
| `quantization` | mxfp4 |
| `topology` | single |
| `tensor_parallel_size` | 1 |

## 소프트 지문 — 다르면 stale, 재측정 권고

| 키 | 값 |
|---|---|
| `driver_version` | 580.173.02 |
| `cuda_version` | 132 |
| `image_tag` | easy-vllm:0.18.0-cu130-aarch64-wheel |
| `image_digest` | sha256:d022edd3bfaf101e0c978930716da1b6845ebb16a72c1479eb9d764469568f44 |
| `max_model_len` | 131072 |
| `max_num_seqs` | 36 |
| `kv_cache_memory_bytes` | 60129542144 |
| `kv_cache_dtype` | fp8 |
| `gpu_memory_utilization` | 0.9 |
| `moe_backend` | marlin |
| `enforce_eager` | N/A |
| `ngc_base_tag` | N/A |

## like-with-like 한정자 (Agent)

**측정 조건.** GuideLLM 0.7.3 · 입력 1024 tok · 출력 256 tok · 프롬프트 16개 · warmup 2 ·
엔드포인트 `openai`(completions) · `ignore_eos=true` · 동시성 1/2/4/8/16 **전 구간 완주**
(`sweep_truncated: N/A`). 표의 `decode_tps_conc1` 은 동시성 1 값이다.

**비교할 수 있는 것**
- 같은 강한 일치 키(gpt-oss-20b · GB10 · 0.18.0 · mxfp4 · single · TP=1)를 가진 다른 셀. 이 캠페인의
  커널 축 8셀이 그러하며 전부 44.53~45.59 t/s 안에 있다.
- 같은 도구·같은 입출력 길이로 잰 다른 GB10 노드의 같은 레시피. 실제로 두 번째 노드가 45.28 t/s 를
  독립 측정했다(세 측정의 폭 0.7%).

**비교할 수 없는 것**
- ⚠ **다른 vLLM 버전의 hint 와 나란히 놓지 마라.** 같은 캠페인의 0.19.0 측정은 **모델(120b)과
  토폴로지(TP=2)까지 함께 달라서** 차이를 버전에 귀속시킬 수 없다.
- ⚠ **`--backend openai-chat` 으로 잰 harmony 수치와 비교 불가.** 그 경로는 `ignore_eos` 가 무력해
  TPOT 이 크게 왜곡된 전례가 있다.
- ⚠ **랜덤 데이터셋 하네스의 speculative-decoding 수치와 비교 불가.** 이 측정은 SD 없음
  (`spec_on: false`)이고, 랜덤 데이터셋은 SD 를 과소평가한다.
- ⚠ **KV 예산이 다르면 동시성 곡선이 통째로 다르다.** 이 측정은 `kv_cache_memory_bytes`
  60,129,542,144(56 GiB)에서 났다. 그 값은 **타겟 H100 80GiB 클램프의 파생값**이지 호스트 VRAM 이
  아니다 — 실제 H100 에서 재현하려면 같은 클램프를 선언해야 한다.

**호스트와 타겟이 다르다는 사실.** 측정 호스트는 GB10(통합메모리 128GB · aarch64 · sm_121a)이고
`target_gpu` 선언은 H100(80 GiB/카드)이다. 따라서 이 수치는 **"H100 에서 이만큼 난다"가 아니라
"H100 예산으로 클램프했을 때 GB10 에서 이만큼 났다"** 이다. 실카드 H100 의 대역폭·커널 가용성은
다르므로 이 hint 는 **용량 계획의 지도**로 쓰고 성능은 그 하드웨어에서 재측정하라.
