# 성능 보고서 — `model-x` @ GPU-X · vLLM 0.0.0

> ⚠ **CARRY-FORWARD 재검증**: 합성 픽스처. 생성일 2026-01-01T00:00:00Z.

## 측정 구성 — bench_mode · 도구 · 반복 (기재 · 게이트 아님)

> 같은 모델·하드웨어라도 도구·반복·모드가 다르면 수치를 나란히 놓지 않는다.

| 키 | 값 |
|---|---|
| bench_mode | full |
| bench_mode_kind | full |
| bench_mode_source | runs[] 집계(판정점 level 1 완주 3 ≥ full 정의 3) |
| downgrade_reason | N/A |
| downgrade_reason_source | N/A |
| bench_tool | guidellm |
| bench_tool_version | 0.7.3 |
| bench_tool_version_source | measured(benchmarks.json metadata.guidellm_version) |
| repeats | 3 |
| repeats_source | declared(campaign.yaml budgets.repeats) |
| repeats_completed | 3 |
| dataset_kind | synthetic |
| dataset_source | N/A |
| dataset_id | n/a |
| dataset_revision |  |
| dataset_source_sha256 | N/A |
| dataset_column_map | {"prompt": "a \| b"} |
| dataset_preprocess_strategy | N/A |
| dataset_seed | 42 |
| dataset_duplicate_sampling | N/A |
| dataset_tokenizer | N/A |

## 판정 (표시만 — verdict_rule.py 결과)

| 항목 | 값 |
|---|---|
| verdict | **REFUTE** |

## lite 지표 (full ⊇ lite — 열 집합 중첩 보장)

> full 런이 lite 를 **포함해서 실행**한 결과다.

| 메트릭 | Main | Sub |
|---|---|---|
| gen tokens/sec (warm) [master] | 18.81 t/s | — |
| cold-start TTFT [master] | 2161 ms | — |

## 부하 스윕 곡선 (client-load · reload 없음)

> 단일 running serve 에 동시 요청 수만 변화(동시성 축).

| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50(ms) | ITL p50(ms) | 완료/실패 |
|---|---|---|---|---|---|---|
| 1 ★판정점 | 22.26 | 22.115934004263103 | 113.17138103744011 | 750.4868507385254 | 42.139861162971044 | 16/0 |
| 2 | 40.5 | 41.005 | N/A | 0.125 | - | 32/0 |
| 4 | 1_000 | 1e3 | 0.375 |  | 2.675 | 60/4 |

**⚠ 절삭된 부하 레벨(적응 상한 클램프 — 조용히 자르지 않음):**
- level 8 truncated: parse/measurement_ok=false
-   level 16 not attempted   

## 반복 축 · 재현 밴드 (full 정의 = lite ∪ GuideLLM × 반복 ≥3)

| 동시성 | 완주/요청 | decode t/s (run 순) | 재현 밴드 % | 밴드 출처 |
|---|---|---|---|---|
| 1 | 3/3 | 22.26 / 23.38 / 23.38 | 4.87 | measured(n=3) |

---
_결정론 렌더 = `render_report.py`(LLM 표저작 ✗)._
