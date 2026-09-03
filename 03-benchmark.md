# 3. 벤치 결과 — 그리고 무엇과 비교할 수 있나

> 아래 수치는 인증서에서 **파싱만** 한 것이다. 합성하지 않았고, 재계산하지 않았다.
> 출처: `benchmark_26090308_58_00_gpt-oss-20b_NVIDIA GB10_0.18.0.yaml`

## 성능

| 항목 | 값 |
|---|---|
| `benchmark_mode` | full |
| `verdict` | PASS |
| `decode_tps_conc1` | 46.51 |
| `rubric_authority` | weak |
| `primary_source` | expected_achievable(roofline×MBU) |
| `primary_tps` | 19.63 |
| `floor_tps` | 16.69 |
| `tolerance` | 0.15 |
| `ratio_M_over_primary` | 2.369 |
| `spec_on` | false |
| `accept_len` | N/A |
| `sweep_levels` | 1,2,4,8,16 |
| `sweep_truncated` | N/A |
| `lite_included` | true |
| `lite_gen_tps_warm` | 46.67 |
| `lite_gen_src` | median_tpot |
| `lite_cold_ttft_ms` | 109.7 |
| `lite_kv_gib` | 48.0 |
| `measured_utc` | 2026-09-02T23:50:16Z |

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
| `max_model_len` | 131072 |
| `max_num_seqs` | 16 |
| `kv_cache_memory_bytes` | 51539607552 |
| `kv_cache_dtype` | fp8 |
| `gpu_memory_utilization` | 0.9 |
| `moe_backend` | N/A |
| `enforce_eager` | N/A |
| `ngc_base_tag` | N/A |

## like-with-like 한정자 (Agent)

- **비교 가능**: 동시성=1, input_len=1024, output_len=256, `--backend openai`(/v1/completions,
  ignore-eos 유효) 조건으로 측정된 다른 gpt-oss-20b × GB10 × vLLM 0.18.0 결과. 스윕 곡선(동시성
  1/2/4/8/16 → 46.56/43.63/33.82/26.94/21.45 t/s)도 같은 input/output 길이 조건에서만 나란히 놓을 수
  있다.
- **비교 불가**: 커뮤니티에서 흔히 보이는 고동시성 처리량 수치(예: A100 환경의 "1000 concurrent
  requests" 벤치 ~9743 tok/s)는 이 표의 `decode_tps_conc1`(단일 스트림 디코드 지연 지표)과 다른 축을
  잰다 — 이번 검증에서 외부 레퍼런스(E)를 그 이유로 채택하지 않고 `e_search=empty`로 기록한 뒤
  roofline 기대치(19.63 t/s)로 판정했다(테스트로그 "Full 벤치마크" 절).
- **다른 vLLM 버전과 비교 시**: attention backend가 TRITON_ATTN으로 고정된 것은 0.18.0 시점의
  사실이다 — 다른 버전에서는 FLASHINFER 등 다른 백엔드가 유효해질 수 있고, 그러면 per-token KV
  비용과 decode t/s 둘 다 달라질 수 있다. `attention_backend` 소프트 지문이 다르면 재측정 없이
  이 수치를 인용하지 마라.
- **chat 엔드포인트 수치와 비교 불가**: `/v1/chat/completions`(harmony)로 측정한 gpt-oss 수치는
  ignore-eos가 무력화돼 완결 응답보다 짧게 끝나는 경향이 있다 — 이 표는 `/v1/completions`로만
  측정했으므로 chat 엔드포인트 기반 수치와 직접 비교하면 왜곡된 결론을 낸다.
