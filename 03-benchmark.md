# 3. 벤치 결과 — 그리고 무엇과 비교할 수 있나

> 아래 수치는 인증서에서 **파싱만** 한 것이다. 합성하지 않았고, 재계산하지 않았다.
> 출처: `benchmark_26090408_gpt-oss-120b_GB10_0.18.0.yaml`

## 성능

| 항목 | 값 |
|---|---|
| `benchmark_mode` | full |
| `verdict` | PASS |
| `decode_tps_conc1` | 53.92 |
| `rubric_authority` | weak |
| `primary_source` | expected_achievable(roofline×MBU) |
| `primary_tps` | 30.99 |
| `floor_tps` | 26.34 |
| `tolerance` | 0.15 |
| `ratio_M_over_primary` | 1.74 |
| `spec_on` | false |
| `accept_len` | N/A |
| `sweep_levels` | 1,2,4,8,16 |
| `sweep_truncated` | N/A |
| `lite_included` | true |
| `lite_gen_tps_warm` | 54.01 |
| `lite_gen_src` | median_tpot |
| `lite_cold_ttft_ms` | 178.5 |
| `lite_kv_gib` | 15.0 |
| `measured_utc` | 2026-09-03T22:18:44Z |

## 강한 일치 키 — 하나라도 다르면 이 수치는 **무효**다

| 키 | 값 |
|---|---|
| `model` | gpt-oss-120b |
| `gpu_model` | NVIDIA GB10 |
| `vllm_version` | 0.18.0 |
| `quantization` | mxfp4 |
| `topology` | multi |
| `tensor_parallel_size` | 2 |

## 소프트 지문 — 다르면 stale, 재측정 권고

| 키 | 값 |
|---|---|
| `driver_version` | 580.173.02 |
| `cuda_version` | 132 |
| `image_tag` | easy-vllm:0.18.0-cu130-aarch64-wheel |
| `image_digest` | sha256:d022edd3bfaf101e0c978930716da1b6845ebb16a72c1479eb9d764469568f44 |
| `max_model_len` | 131072 |
| `max_num_seqs` | 16 |
| `kv_cache_memory_bytes` | 16106127360 |
| `kv_cache_dtype` | fp8 |
| `gpu_memory_utilization` | 0.80 |
| `moe_backend` | N/A |
| `enforce_eager` | N/A |
| `ngc_base_tag` | N/A |

## like-with-like 한정자 (Agent)

**측정 정의**: 판정점은 동시성 1 에서의 `1000 / median_tpot` 이고, 스윕은 재기동 없이 클라이언트
동시성만 1/2/4/8/16 으로 올린 것이다(절삭 0 · 실패 0). 이 모델은 harmony 계열이라 **`/v1/chat/completions`
로 재면 `--ignore-eos` 가 무력화돼 TPOT 이 왜곡된다** — 반드시 `/v1/completions` 경로로 재고, 생성된
총 토큰 수가 요청값과 정확히 일치하는지 확인하라. 우리도 이 배선이 한 곳에서 빠져 lite 수치가 2.9 배
낮게 나온 적이 있다.

**나란히 놓을 수 있는 것**: 같은 저장소가 같은 방식으로 잰 `gpt-oss-20b` 의 같은 버전·같은 토폴로지
수치(69.36 t/s), 그리고 같은 모델의 단일노드 수치(34.42 t/s). 후자와의 비율 1.57 배가 이 항목의 핵심
정보다. lite 의 `lite_gen_tps_warm` 과 full 의 `decode_tps_conc1` 은 **같은 정의**를 쓰므로 나란히 놓아도
된다(이 런에서 54.01 대 53.92 로 0.2% 이내).

**비교할 수 없는 것**: 공개 커뮤니티에서 보이는 같은 모델·같은 GPU 의 73~77 t/s 급 수치는 **양자화
범위와 커널 백엔드가 다르다** — dense 층까지 MXFP4 로 내리고 다른 attention/GEMM 백엔드를 쓰면 노드당
읽는 바이트가 우리의 절반이 되어 루프라인 상한 자체가 올라간다. 그런 수치를 문턱으로 쓰면 부당한 기각이
된다. **같은 모델·같은 하드웨어라도 구성이 다르면 물리 상한이 다르다** — like-with-like 의 축은
모델과 하드웨어만이 아니다. 그래서 이 판정의 외부 레퍼런스는 `empty` 로 기록됐고 루프라인만으로
판정했다(음성 정직).

**MBU 를 절대 성능과 혼동하지 마라**: TP=2 는 노드당 읽는 바이트를 절반으로 만들어 루프라인 상한을
두 배로 올린다(44.31 → 88.55 t/s). 그래서 **대역폭 활용률은 77.9% 에서 60.9% 로 떨어지는데 절대
성능은 34.42 에서 53.92 로 오른다.** 두 숫자가 반대 방향으로 움직이는 것은 모순이 아니다. 통신은
병목이 아니었다(집단 연산 추정 0.01 ms 미만 · 인터커넥트 합산 196.88 Gb/s).

**환경 한정**: 열은 최고 73°C 로 경고선 아래였고 강등 없이 full 로 완주했다. 열 여유가 없는 섀시에서는
같은 설정이라도 스윕 뒷부분이 달라질 수 있다.
