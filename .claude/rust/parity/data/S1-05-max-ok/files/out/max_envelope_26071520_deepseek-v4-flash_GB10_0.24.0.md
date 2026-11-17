# Max 모드 — HW 안전-최대 컨텍스트 envelope: `deepseek-v4-flash` @ NVIDIA GB10 · vLLM 0.24.0

> ⚠ **CARRY-FORWARD 재검증**: 이 envelope 은 아래 환경 한정 특성화다(그때-그 HW·드라이버·이미지). 다른 세션·하드웨어·**드라이버 버전**에서 이 안전상한을 그대로 신뢰하지 말 것 — 재측정. **inform-only**(안전상한 특성화 표시 · 판정 게이트 아님). 별도 오퍼레이션(벤치마커 인프라 공유). 생성일 2026-07-15T11:00:00Z.

## 안전-최대 컨텍스트 (핵심 결과)

| 항목 | 값 |
|---|---|
| **안전상한 max-model-len** | **393216 tokens** |
| 스텝업 축 | max-model-len (안전측 오름차순) |
| 원 config max-model-len | 524288 |

> 안전상한 = **serve+smoke 를 통과한 최대 컨텍스트**(그 위 레벨은 실패/트립/미시도). 각 레벨은 재서빙(reload)+기능 스모크 + 협역 워치독/로드-전 RAM 게이트 관측으로 판정. **선-기록 후-위험**(레벨 결과는 다음 시도 전 기록).

## 레벨별 결과 (컨텍스트 스텝업)

| max-model-len | serve+smoke | 판정 |
|---|---|---|
| 131072 | exit 0 | ✅ 안전 |
| 262144 | exit 0 | ✅ 안전 |
| 393216 | exit 0 | ✅ 안전 |

**⚠ 안전상한 위 / 미시도 (silent truncation 금지):**
- max-model-len 524288 unsafe: serve+smoke exit 2 → 직전 안전상한=393216, 상위 중단

## 측정 환경 스냅샷

| 키 | 값 |
|---|---|
| model | deepseek-v4-flash |
| gpu_model | NVIDIA GB10 |
| vllm_version | 0.24.0 |
| topology | multi |
| tensor_parallel_size | 2 |
| driver_version | 580.159.03 |
| quantization | N/A |
| moe_backend | humming |
| image_tag | easy-vllm:0.24.0-cu132-aarch64-source-sm12x |

---
_Max = 별도 오퍼레이션(벤치마커 측정 인프라만 공유 · native 모드 아님) · 결정론 렌더 `render_max_report.sh`(LLM 표저작 ✗) · 안전 = 헌법 §호스트 안전체계 따름정리 · plan_26071510._
