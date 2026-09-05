# 3. 벤치 결과 — 그리고 무엇과 비교할 수 있나

> 아래 수치는 인증서에서 **파싱만** 한 것이다. 합성하지 않았고, 재계산하지 않았다.
> 출처: `benchmark_26090602_gpt-oss-120b_GB10_0.19.0.yaml`

## 성능

| 항목 | 값 |
|---|---|
| `benchmark_mode` | full |
| `verdict` | PASS |
| `decode_tps_conc1` | 50.58 |
| `rubric_authority` | explore |
| `primary_source` | expected_achievable(roofline×MBU) |
| `primary_tps` | 30.99 |
| `floor_tps` | 26.34 |
| `tolerance` | 0.15 |
| `ratio_M_over_primary` | 1.632 |
| `spec_on` | false |
| `accept_len` | N/A |
| `sweep_levels` | 1 |
| `sweep_truncated` | level 2 truncated: parse/measurement_ok=false |
| `lite_included` | true |
| `lite_gen_tps_warm` | 52.52 |
| `lite_gen_src` | median_tpot |
| `lite_cold_ttft_ms` | 247.0 |
| `lite_kv_gib` | 24.07 |
| `measured_utc` | 2026-09-05T17:49:27Z |

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
| `model` | gpt-oss-120b |
| `gpu_model` | NVIDIA GB10 |
| `vllm_version` | 0.19.0 |
| `quantization` | mxfp4 |
| `topology` | multi |
| `tensor_parallel_size` | 2 |

## 소프트 지문 — 다르면 stale, 재측정 권고

| 키 | 값 |
|---|---|
| `driver_version` | 580.173.02 |
| `cuda_version` | 132 |
| `image_tag` | easy-vllm:0.19.0-cu130-aarch64-wheel |
| `image_digest` | sha256:a0249f3a416864b3c35e5a59a2149473e5e2593ceffda6da8115684c0c6a609e |
| `max_model_len` | 131072 |
| `max_num_seqs` | 20 |
| `kv_cache_memory_bytes` | 25841106944 |
| `kv_cache_dtype` | fp8 |
| `gpu_memory_utilization` | 0.80 |
| `moe_backend` | marlin |
| `enforce_eager` | N/A |
| `ngc_base_tag` | N/A |

## like-with-like 한정자 (Agent)

**측정 조건.** GuideLLM 0.7.3 · 입력 1024 tok · 출력 256 tok · 프롬프트 16개 · warmup 2 ·
엔드포인트 `openai`(completions) · `ignore_eos=true`. 판정점은 **동시성 1** 이다.

**★ 이 측정은 절삭됐다.** `sweep_truncated` 가 `level 2 truncated: parse/measurement_ok=false` 다 —
동시성 2 이상은 이 인증서에 없다. 원인은 성능이 아니라 **간헐적 스트리밍 절단**이다(서사 §7).
동시성 곡선이 필요하면 같은 캠페인의 다른 셀을 보라: KV `auto` 셀이 1/2/4/8/16 전 구간을 오류 0으로
완주했고 50.26 / 44.61 / 33.40 / 23.92 / 12.40 t/s 였다. **다만 그 셀은 KV dtype 이 다르므로
이 인증서의 강한 키와 일치하지 않는다** — 참고값이지 이 레시피의 곡선이 아니다.

**비교할 수 있는 것**
- 같은 강한 일치 키(gpt-oss-120b · GB10 · 0.19.0 · mxfp4 · multi · TP=2)를 가진 다른 셀.
- 같은 도구·같은 입출력 길이로 잰 같은 레시피의 재측정.

**비교할 수 없는 것**
- ⚠ **단일노드 수치와 나란히 놓지 마라.** TP=2 는 가중치를 쪼개 노드당 대역폭 요구를 바꾼다.
  이 캠페인의 단일노드 측정은 **모델(20b)과 버전(0.18.0)까지 다르다.**
- ⚠ **다른 vLLM 버전의 hint 와 비교 불가.** 0.19.1 은 애초에 이 토폴로지로 뜨지 않는다.
- ⚠ **`--backend openai-chat` 으로 잰 harmony 수치와 비교 불가**(`ignore_eos` 무력화 → TPOT 왜곡).
- ⚠ **KV 예산이 다르면 곡선이 통째로 다르다.** 이 측정의 노드당 KV 는 24.07 GiB 이며, 그 값은
  **타겟 H100 80GiB 클램프의 파생값**이지 호스트 VRAM 이 아니다.

**호스트와 타겟이 다르다.** 측정 호스트는 GB10×2(통합메모리 · aarch64 · sm_121a · RoCE)이고
`target_gpu` 선언은 H100 이다. 이 수치는 **"H100 2노드에서 이만큼 난다"가 아니라 "H100 예산으로
클램프했을 때 GB10 2노드에서 이만큼 났다"** 이다. 인터커넥트가 다르면 TP 통신 비용이 달라지므로,
용량 계획의 지도로 쓰고 성능은 그 하드웨어에서 재측정하라.
