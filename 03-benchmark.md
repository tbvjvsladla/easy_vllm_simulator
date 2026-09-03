# 3. 벤치 결과 — 그리고 무엇과 비교할 수 있나

> 아래 수치는 인증서에서 **파싱만** 한 것이다. 합성하지 않았고, 재계산하지 않았다.
> 출처: `benchmark_26090408_14_02_gpt-oss-20b_GB10_0.18.0.yaml`

## 성능

| 항목 | 값 |
|---|---|
| `benchmark_mode` | full |
| `verdict` | PASS |
| `decode_tps_conc1` | 69.36 |
| `rubric_authority` | weak |
| `primary_source` | expected_achievable(roofline×MBU) |
| `primary_tps` | 39.24 |
| `floor_tps` | 33.35 |
| `tolerance` | 0.15 |
| `ratio_M_over_primary` | 1.768 |
| `spec_on` | false |
| `accept_len` | N/A |
| `sweep_levels` | 1,2,4,8,16 |
| `sweep_truncated` | N/A |
| `lite_included` | true |
| `lite_gen_tps_warm` | 69.58 |
| `lite_gen_src` | median_tpot |
| `lite_cold_ttft_ms` | 89.9 |
| `lite_kv_gib` | 5.0 |
| `measured_utc` | 2026-09-03T22:41:45Z |

## 강한 일치 키 — 하나라도 다르면 이 수치는 **무효**다

| 키 | 값 |
|---|---|
| `model` | gpt-oss-20b |
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
| `max_model_len` | 4096 |
| `max_num_seqs` | 32 |
| `kv_cache_memory_bytes` | 5368709120 |
| `kv_cache_dtype` | fp8 |
| `gpu_memory_utilization` | 0.80 |
| `moe_backend` | N/A |
| `enforce_eager` | N/A |
| `ngc_base_tag` | N/A |

## like-with-like 한정자 (Agent)

**측정 정의**: 판정점은 동시성 1 에서의 `1000 / median_tpot` 이고, 스윕은 재기동 없이 클라이언트
동시성만 1/2/4/8/16 으로 올린 것이다(절삭 0 · 실패 0). 이 모델은 harmony 계열이라 **`/v1/chat/completions`
로 재면 `--ignore-eos` 가 무력화돼 TPOT 이 왜곡된다** — `/v1/completions` 경로로 재고, 생성된 총
토큰 수가 요청값과 정확히 일치하는지 확인하라.

**★ 같은 모델의 단일노드 수치와 나란히 놓을 때의 한정자**: 우리의 단일노드 측정(48.58 t/s)은
`max-model-len 131072` 구성이고 이 측정은 **4096** 이다. 동시성 1 의 디코드 속도는 대부분 가중치
읽기에 지배되므로 두 값을 비율(1.43 배)로 읽는 것은 성립하지만, **KV 점유·최대 동시 컨텍스트·긴
입력에서의 TTFT 는 비교 대상이 아니다.** 컨텍스트를 늘린 뒤에는 이 스윕을 다시 재라.

**나란히 놓을 수 있는 것**: 같은 저장소가 같은 방식으로 잰 같은 버전·같은 토폴로지의 `gpt-oss-120b`
수치(53.92 t/s). lite 의 `lite_gen_tps_warm` 과 full 의 `decode_tps_conc1` 은 **같은 정의**를 쓰므로
나란히 놓아도 된다(이 런에서 69.58 대 69.36 으로 0.4% 이내).

**비교할 수 없는 것**: 다른 엔진(다른 추론 서버)의 수치, 그리고 양자화 범위나 커널 백엔드가 다른
구성. dense 층까지 양자화하면 노드당 읽는 바이트가 줄어 **루프라인 상한 자체가 올라간다** — 그런
수치를 문턱으로 쓰면 부당한 기각이 된다. 이 판정의 외부 레퍼런스는 like-with-like 를 못 찾아
`empty` 로 기록됐고 루프라인만으로 판정했다(음성 정직).

**MBU 와 절대 성능은 반대로 움직일 수 있다**: TP=2 는 노드당 읽는 바이트를 절반으로 만들어 루프라인
상한을 두 배로 올린다(56.1 → 112.13 t/s). 그래서 대역폭 활용률은 86.6% 에서 61.9% 로 떨어지는데
절대 성능은 48.58 에서 69.36 으로 오른다. 모순이 아니다. 통신은 병목이 아니었다(인터커넥트 합산
196.88 Gb/s).

**환경 한정**: 열은 경고선 아래였고 강등 없이 full 로 완주했다. 열 여유가 없는 섀시에서는 같은
설정이라도 스윕 뒷부분이 달라질 수 있다.
