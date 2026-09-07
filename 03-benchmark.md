# 3. 벤치 결과 — 그리고 무엇과 비교할 수 있나

> 아래 수치는 인증서에서 **파싱만** 한 것이다. 합성하지 않았고, 재계산하지 않았다.
> 출처: `benchmark_26090705_gpt-oss-120b_GB10_0.19.0.yaml`

## 성능

| 항목 | 값 |
|---|---|
| `benchmark_mode` | full |
| `verdict` | PASS |
| `decode_tps_conc1` | 50.85 |
| `rubric_authority` | explore |
| `primary_source` | expected_achievable(roofline×MBU) |
| `primary_tps` | 30.99 |
| `floor_tps` | 26.34 |
| `tolerance` | 0.15 |
| `ratio_M_over_primary` | 1.641 |
| `spec_on` | false |
| `accept_len` | N/A |
| `sweep_levels` | 1,2,4,8 |
| `sweep_truncated` | level 16 truncated: parse/measurement_ok=false |
| `lite_included` | true |
| `lite_gen_tps_warm` | 53.03 |
| `lite_gen_src` | median_tpot |
| `lite_cold_ttft_ms` | 195.8 |
| `lite_kv_gib` | 54.68 |
| `measured_utc` | 2026-09-06T20:20:23Z |

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
| `kv_cache_memory_bytes` | 58712915968 |
| `kv_cache_dtype` | fp8 |
| `gpu_memory_utilization` | 0.9 |
| `moe_backend` | marlin |
| `enforce_eager` | N/A |
| `ngc_base_tag` | N/A |

## 부하 스윕 곡선 — 동시성별 (결정론 파싱 · 손저작 ✗)

> 단일 running serve 에 **동시 요청 수만** 바꿔 잰 곡선이다(reload 없음 · request-rate=inf).
> `동시성=1` 행이 인증서의 판정점이며, 나머지 행은 그 레시피가 **부하에서 어떻게 되는지**를 말한다.
> 출처: `bench_report_26090705_gpt-oss-120b_GB10_0.19.0.md` (bench_report) · 소수 2자리 표시 반올림 · 결측은 `N/A`(0 이 아니다).
> 이 표는 스크립트가 파싱해 렌더한다 — 손으로 옮긴 수치가 아니며 `--verify` 가 diff 0 을 요구한다.

| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50(ms) | ITL p50(ms) | 완료/실패 |
|---|---|---|---|---|---|---|
| 1 ★판정점 | 50.85 | 47.74 | 251.20 | 267.14 | 18.70 | 14/2 |
| 2 | 45.55 | 90.94 | 478.50 | 135.65 | 21.51 | 16/0 |
| 4 | 34.70 | 122.99 | 647.14 | 160.05 | 28.36 | 17/1 |
| 8 | 26.61 | 181.95 | 957.38 | 158.73 | 37.10 | 18/0 |

**⚠ 절삭된 부하 레벨(조용히 자르지 않는다):**
- level 16 truncated: parse/measurement_ok=false


## 결손 기재

> ⚠ 아래 정보가 **부재한 채로 발행**됐다. 부재는 실패가 아니라 **기록**이며, 이 태그를
> 소비할 때 그만큼을 모른 채 소비한다는 뜻이다(강행 발행 정책 · plan_26090616 Q7/Q8).

| 사유코드 | 뜻 |
|---|---|
| `HINT_MISSING_SLAVE_ATTESTATION` | 슬레이브 ABI attestation 부재 — 멀티에서 두 노드가 같은 것을 돌렸다는 증거가 성공 경로에 보존되지 않았다. |

## like-with-like 한정자 (Agent)

**비교 가능**: GB10 **2노드 TP=2 · Ray** · vLLM 0.19.0 · mxfp4 · `max_model_len 131072` ·
GuideLLM 0.7.3 · 입력 1024/출력 256 · `ignore_eos` on 인 판. §강한 일치 키가 하나라도 다르면 무효다.

**비교 불가 — 넷**:
1. **엔드포인트.** 이 판은 **chat**(`/v1/chat/completions`)으로 쟀고 16건 중 3건이 harmony 파서
   파손으로 깨졌다. gpt-oss 는 완결 엔드포인트(`/v1/completions`)로 재는 것이 옳다. 형제
   `gb10-sub-native`(완결, 18/18)와 **직접 비교하지 마라**.
2. **토폴로지.** 단일 노드 판(`gb10-main-native`)과는 노드 수도 TP 도 다르다. 같은 하드웨어라는
   이유로 나란히 놓으면 TP 이득과 모델 크기 차이가 뒤섞인다.
3. **부하 시간.** 동시성 곡선 1→50.85 · 2→45.55 · 4→34.70 · 8→26.61 이고 **레벨 16 은 절삭**이다 —
   측정 실패가 아니라 **서빙이 열로 죽었다**. 절삭된 곡선을 완주한 곡선과 나란히 놓지 마라.
4. **KV 예산의 타겟.** 형제 태그 `gb10x2-sim-h100` 은 H100 80GiB 이식 클램프(KV 25,841,106,944 B)
   판이다. 이 판은 그것을 걷어낸 네이티브 예산이며 KV 가 2.27배다. 두 태그를 나누는 축은 하드웨어가
   아니라 **클램프의 타겟 선택**이다 — 그리고 그 2.27배가 속도로는 +0.5% 밖에 안 됐다.

**결손**: 노드 정합 attestation(`attestation_b0-kvfp8-attnauto-moeauto.json`)이 없다. 두 노드가
같은 이미지 digest·같은 ABI 로 섰다는 기계 대조가 이 판에는 **부재**하며, 페이로드 `missing[]` 에
그대로 실린다. 재현자는 양 노드 빌드 뒤 digest 를 스스로 대조하라.
