# 성능 보고서 — `deepseek-v4-flash` @ NVIDIA GB10 · vLLM 0.24.0

> ⚠ **CARRY-FORWARD 재검증**: 이 보고서는 아래 '측정 환경' 한정 계측이다(그때-그 HW/config). 다른 세션·하드웨어·드라이버·이미지에서 이 수치를 그대로 신뢰하지 말 것 — 소비 전 재측정. 이 문서는 **inform-only**(판정 권한 없음 · verdict_rule 독점). 생성일 2026-07-15T10:30:00Z.

## 측정 구성 — bench_mode · 도구 · 반복 (기재 · 게이트 아님)

> 같은 모델·하드웨어라도 도구·반복·모드가 다르면 수치를 나란히 놓지 않는다. `bench_mode=lite` 는 full 정의(lite ∪ GuideLLM × 반복 ≥3)를 충족하지 않은 측정이다 — 선언된 lite-only 이거나 반복 불성립으로 강등된 셀이다(강등이면 `downgrade_reason`). 이 표는 기재이지 판정이 아니다.

| 키 | 값 |
|---|---|
| bench_mode | N/A |
| bench_mode_kind | undetermined |
| bench_mode_source | 미확정 — absent(bench_mode.json 부재 — 이 스윕을 분류한 기록이 없다) |
| downgrade_reason | N/A |
| downgrade_reason_source | N/A |
| bench_tool | N/A |
| bench_tool_version | N/A |
| bench_tool_version_source | N/A |
| repeats | N/A |
| repeats_source | absent(반복 축 기록 없음 — 신설 전 산출물) |
| repeats_completed | N/A |
| dataset_kind | N/A |
| dataset_source | N/A |
| dataset_id | N/A |
| dataset_revision | N/A |
| dataset_source_sha256 | N/A |
| dataset_column_map | N/A |
| dataset_preprocess_strategy | N/A |
| dataset_seed | N/A |
| dataset_duplicate_sampling | N/A |
| dataset_tokenizer | N/A |

## 판정 (표시만 — verdict_rule.py 결과)

| 항목 | 값 |
|---|---|
| verdict | **PASS** |
| 측정 decode t/s (동시성1) | 24.3 t/s |
| 루브릭 권한 | weak |
| 루브릭 primary | 25.0 t/s (E(external_reference)) |
| floor (primary×(1−tol)) | 21.25 t/s |
| ratio (M/primary) | 0.972 |
| tolerance | 0.15 |
| E-search 상태 | no |

## lite 지표 (full ⊇ lite — 열 집합 중첩 보장)

> ⚠ **lite 미포함** — 이 full 런은 lite 를 수집하지 못했다(열 집합 결손). lite 만 돈 다른 모델과 직접 비교하지 마라.

## 부하 스윕 곡선 (client-load · reload 없음)

> 단일 running serve 에 동시 요청 수만 변화(동시성 축). **동시성=1 행이 판정점**(verdict 재사용). request-rate=inf(각 레벨 포화 측정). config-space(batch×maxlen)는 이 스윕 밖(Max 모드/explorer 소관).

| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50(ms) | ITL p50(ms) | 완료/실패 |
|---|---|---|---|---|---|---|
| 1 ★판정점 | 24.3 | 23.8 | 190.0 | 210.0 | 41.1 | 16/0 |
| 2 | 23.1 | 44.9 | 360.0 | 260.0 | 43.3 | 16/0 |
| 4 | 21.0 | 82.5 | 660.0 | 380.0 | 47.6 | 16/0 |
| 8 | 17.4 | 132.0 | 1050.0 | 720.0 | 57.5 | 16/0 |

**⚠ 절삭된 부하 레벨(적응 상한 클램프 — 조용히 자르지 않음):**
- level 16 truncated: run_bench exit 3 (serve health-drop/게이트/bench 실패 — 상위 레벨 중단)

## 반복 축 · 재현 밴드 (full 정의 = lite ∪ GuideLLM × 반복 ≥3)

> full 정의는 레벨마다 같은 running serve 에 **반복 ≥3** 이다. 위 스윕 표와 판정점·인증서는 **대표 run(첫 완주 run) 하나**의 값이다(평균·합성 ✗). 재현 밴드는 **기재**다 — 판정 게이트는 불변이고 분산은 강등 사유가 아니다. 반복 불성립(판정점 run 실패 · 스윕이 멈춘 자리의 블랙박스 kill)만 셀을 lite 로 강등하며 확정은 `classify_cell.py` 가 한다. 포화 경계에서 스윕이 멈춘 것(적응 상한 클램프)은 강등이 아니다.

- bench_mode: 미확정 — absent(bench_mode.json 부재 — 이 스윕을 분류한 기록이 없다)

> ⚠ **반복 축 기록 없음** — 반복 축 신설(2026-09-14) 전 산출물이거나 재조립 원천에 runs[] 가 없다. 이 리포트의 수치는 단일 run 이며 **산포 추정치가 없다**(다른 도구·조건의 밴드를 빌려 쓰지 말 것).

## 루프라인 컨텍스트 (결정론 상한 — 의심 임계, SLA 아님)

| 지표 | 값 |
|---|---|
| R_fp (forward-pass/s 상한, 100% MBU) | 30.0 t/s |
| R_token (spec, accept_len×R_fp) | 45.0 t/s |
| expected_achievable (루프라인×MBU) | 15.75 t/s |
| spec(MTP) on (측정) | True (accept_len=1.85) |

## 측정 환경 스냅샷

| 키 | 값 |
|---|---|
| model | deepseek-v4-flash |
| model_source | N/A |
| serving_config | N/A |
| gpu_model | NVIDIA GB10 |
| vllm_version | 0.24.0 |
| quantization | N/A |
| quantization_source | N/A |
| topology | multi |
| tensor_parallel_size | 2 |
| driver_version | 580.159.03 |
| cuda_version | 132 |
| image_tag | easy-vllm:0.24.0-cu132-aarch64-source-sm12x |
| max_model_len | 524288 |
| max_num_seqs | 16 |
| kv_cache_memory_bytes | 17179869184 |
| kv_cache_dtype | auto |
| kv_cache_dtype_source | N/A |
| gpu_memory_utilization | 0.80 |
| moe_backend | humming |
| enforce_eager | true |
| model_path | /app/models/DeepSeek/DeepSeek-V4/DeepSeek-V4-Flash |
| 입력 길이(sweep) | 1024 |

---
_결정론 렌더 = `render_report.py`(LLM 표저작 ✗) · 참조 체인: bench JSON → 이 report → testlog(판정) → devlog(서사)._
