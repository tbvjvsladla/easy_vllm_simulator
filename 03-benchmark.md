# 3. 벤치 결과 — 그리고 무엇과 비교할 수 있나

> 아래 수치는 인증서에서 **파싱만** 한 것이다. 합성하지 않았고, 재계산하지 않았다.
> 출처: `benchmark_26090408_37_21_gpt-oss-120b_GB10_0.18.0.yaml`

## 성능

| 항목 | 값 |
|---|---|
| `benchmark_mode` | full |
| `verdict` | PASS |
| `decode_tps_conc1` | 34.42 |
| `rubric_authority` | weak |
| `primary_source` | expected_achievable(roofline×MBU) |
| `primary_tps` | 15.51 |
| `floor_tps` | 13.18 |
| `tolerance` | 0.15 |
| `ratio_M_over_primary` | 2.219 |
| `spec_on` | false |
| `accept_len` | N/A |
| `sweep_levels` | 1,2,4,8,16 |
| `sweep_truncated` | N/A |
| `lite_included` | true |
| `lite_gen_tps_warm` | 34.69 |
| `lite_gen_src` | median_tpot |
| `lite_cold_ttft_ms` | 69.6 |
| `lite_kv_gib` | 30.0 |
| `measured_utc` | 2026-09-03T12:34:31Z |

## 강한 일치 키 — 하나라도 다르면 이 수치는 **무효**다

| 키 | 값 |
|---|---|
| `model` | gpt-oss-120b |
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
| `kv_cache_memory_bytes` | 32212254720 |
| `kv_cache_dtype` | fp8 |
| `gpu_memory_utilization` | 0.9 |
| `moe_backend` | N/A |
| `enforce_eager` | N/A |
| `ngc_base_tag` | N/A |

## like-with-like 한정자 (Agent)

**측정 정의**: 판정점은 동시성 1 에서의 `1000/median_tpot` 이고, 스윕은 재기동 없이 클라이언트
동시성만 1/2/4/8/16 으로 올린 것이다(절삭 0 · 실패 0). ★ **반드시 `/v1/completions` 경로로 재라** —
harmony 계열은 chat 경로에서 `--ignore-eos` 가 무력화돼 같은 런에서 11.83 대 34.42(2.9배)로 갈렸다.
총 생성 토큰이 요청값과 일치하는지 확인하는 것이 그 검사다.

**나란히 놓을 수 있는 것**: 같은 저장소가 같은 방식으로 잰 같은 모델의 0.19.1 단일노드 수치
(34.55 t/s · 스윕 각 레벨 ±0.5 이내) — **두 버전은 이 토폴로지에서 사실상 같다**. 그리고 같은 모델의
TP=2 분산 수치(53.92 t/s · 1.57배). lite `lite_gen_tps_warm` 과 full `decode_tps_conc1` 은 같은
정의이므로 나란히 놓아도 된다(34.69 대 34.42).

**비교할 수 없는 것**: 공개 커뮤니티의 57~60 t/s 급 수치. 그 구성은 dense 층까지 MXFP4 로 내리고
다른 attention/GEMM 백엔드를 쓰므로 active 바이트가 우리의 절반 수준이고 **루프라인 상한 자체가
다르다**. 우리 구성의 상한은 44.31 t/s 이므로 60 t/s 는 물리적으로 도달 불가능하며, 문턱으로 쓰면
부당한 기각이 된다. 다른 엔진(SGLang·llama.cpp)의 수치도 같은 이유로 비교 대상이 아니다.

**MBU**: 34.42 / 44.31 = **77.7%** 로, 배치 1 에서 대역폭을 상당히 잘 쓰고 있다. 루브릭 primary 는
보수적 가정(realistic_fraction 0.35)의 `expected_achievable` 15.51 이라 ratio 가 2.219 로 크게 나온
것이지 **초과 달성이 아니다** — 문턱이 낮았던 것이다. 이 항목의 실질 여유는 22% 다.

**환경 한정**: 열 최고 SoC 89°C 로 경고선(90) **바로 아래**에서 완주했다. 열 여유가 없는 섀시나
주변 온도가 높은 환경에서는 같은 설정이 강등될 수 있다. 이 인증서는 `image_digest` 필드가
신설되기 **전**에 발행돼 이미지를 태그로만 지목한다 — 재현 시 트랙(wheel)과 vLLM 버전을 컨테이너
안에서 직접 확인하라.
