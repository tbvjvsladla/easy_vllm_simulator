# 3. 벤치 결과 — 그리고 무엇과 비교할 수 있나

> 아래 수치는 인증서에서 **파싱만** 한 것이다. 합성하지 않았고, 재계산하지 않았다.
> 출처: `benchmark_26091223_qwen3.8-flash-next-nvfp4_GB10_0.29.0.yaml`

## 성능

| 항목 | 값 |
|---|---|
| `benchmark_mode` | full |
| `verdict` | PASS |
| `decode_tps_conc1` | 46.7 |
| `rubric_authority` | explore |
| `primary_source` | E(external_reference) |
| `primary_tps` | 53.7 |
| `floor_tps` | 45.65 |
| `tolerance` | 0.15 |
| `ratio_M_over_primary` | 0.87 |
| `spec_on` | true |
| `accept_len` | 2.481818181818182 |
| `sweep_levels` | 1,2,4,8,16 |
| `sweep_truncated` | N/A |
| `lite_included` | true |
| `lite_gen_tps_warm` | 36.9 |
| `lite_gen_src` | median_tpot |
| `lite_cold_ttft_ms` | 1743.2 |
| `lite_kv_gib` | 8.0 |
| `measured_utc` | 2026-09-12T14:27:16Z |

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
| `model` | qwen3.8-flash-next-nvfp4 |
| `gpu_model` | NVIDIA GB10 |
| `vllm_version` | 0.29.0 |
| `quantization` | N/A |
| `topology` | multi |
| `tensor_parallel_size` | 2 |

## 소프트 지문 — 다르면 stale, 재측정 권고

| 키 | 값 |
|---|---|
| `driver_version` | 580.173.02 |
| `cuda_version` | 132 |
| `image_tag` | easy-vllm:0.29.0rc6-cu133-aarch64-source |
| `image_digest` | sha256:85cef27827c3afcdd39721ccafe623fb5457d11978e927edc216c1bf3fede931 |
| `max_model_len` | 262144 |
| `max_num_seqs` | 8 |
| `kv_cache_memory_bytes` | 8589934592 |
| `kv_cache_dtype` | auto |
| `gpu_memory_utilization` | 0.80 |
| `moe_backend` | flashinfer_cutlass |
| `enforce_eager` | true |
| `ngc_base_tag` | N/A |

## 부하 스윕 곡선 — 동시성별 (결정론 파싱 · 손저작 ✗)

> 단일 running serve 에 **동시 요청 수만** 바꿔 잰 곡선이다(reload 없음 · request-rate=inf).
> `동시성=1` 행이 인증서의 판정점이며, 나머지 행은 그 레시피가 **부하에서 어떻게 되는지**를 말한다.
> 출처: `bench_report_26091223_qwen3.8-flash-next-nvfp4_GB10_0.29.0.md` (bench_report) · 소수 2자리 표시 반올림 · 결측은 `N/A`(0 이 아니다).
> 이 표는 스크립트가 파싱해 렌더한다 — 손으로 옮긴 수치가 아니며 `--verify` 가 diff 0 을 요구한다.

| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50(ms) | ITL p50(ms) | 완료/실패 |
|---|---|---|---|---|---|---|
| 1 ★판정점 | 46.70 | 39.61 | 198.04 | 475.99 | 57.57 | 16/0 |
| 2 | 40.72 | 71.93 | 359.64 | 571.87 | 64.15 | 16/0 |
| 4 | 29.83 | 98.23 | 491.13 | 909.54 | 82.91 | 16/0 |
| 8 | 22.87 | 125.99 | 629.95 | 1344.00 | 108.77 | 16/0 |
| 16 | 21.99 | 125.20 | 625.98 | 8249.65 | 107.38 | 16/0 |

_절삭된 레벨 없음(요청 전 레벨 완주)._


## 결손 기재

> ⚠ 아래 정보가 **부재한 채로 발행**됐다. 부재는 실패가 아니라 **기록**이며, 이 태그를
> 소비할 때 그만큼을 모른 채 소비한다는 뜻이다(강행 발행 정책 · plan_26090616 Q7/Q8).

| 사유코드 | 뜻 |
|---|---|
| `HINT_MISSING_SLAVE_ATTESTATION` | 슬레이브 ABI attestation 부재 — 멀티에서 두 노드가 같은 것을 돌렸다는 증거가 성공 경로에 보존되지 않았다. |

## like-with-like 한정자 (Agent)

**비교 가능**: 같은 캠페인의 mmap 자매셀(`nv4-bf-262k-mmp`, 38.81 t/s)과는 **직접 비교된다** —
같은 이미지 digest · 같은 노드 · 같은 측정 사양(`vllm bench serve` · in 1024 / out 256 / n 16 /
warmup 2 폐기 · `/v1/completions`)에서 잰 값이고, 갈린 축은 PLE 모드와 그에 딸린 네 설정뿐이다.

**비교 가능(한정)**: 외부 레퍼런스의 DGX Spark 2노드 SPEED 프로필(median 53.7 t/s)과는 같은
체크포인트(`nvidia/Qwen3.8-Flash-Next-NVFP4`) · 같은 노드 수 · 같은 PLE resident · 같은 MTP3 라
**축이 맞는다**. 다만 그쪽은 **KV dtype 이 fp8** 이고 이쪽은 `auto`(bf16)이며 context 도 다르므로,
0.87 이라는 비율은 "같은 조건에서의 열세"가 아니라 **KV dtype 축이 섞인 값**으로 읽어야 한다.

**비교 불가**:
- 외부 **1노드** 수치(32.5 median)와 — 노드 수가 다르면 per-node 대역폭 분모가 달라진다.
- 외부 **4노드**(40.5) 및 게시물 제목의 "64 tok/s" 와 — 후자는 실측이 아니다(실제 피크 63.7).
- **다른 vLLM 버전**의 hint 와 — 이 모델은 0.29.x 계열에서 아키텍처 지원이 움직이는 중이다.
- 동시성 8 초과 구간의 집계 처리량과 — 이 레시피는 `max-num-seqs: 8` 이라 conc16 이 conc8 에서
  포화한다(125.99 → 125.20). **레시피의 물리 한계가 아니라 선언한 상한**이며, 여유
  (MemAvail +5,952 MiB)를 생각하면 올릴 여지가 있다.
