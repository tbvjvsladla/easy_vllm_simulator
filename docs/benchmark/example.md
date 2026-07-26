# benchmark/ — full-런 성능 계측 vault (사람용 report + 기계용 인증서)

> 이 `example.md`는 **추적되는 스켈레톤**이다(폴더 구조 + 작성 규칙 배포용). 실제 산출물(report md·
> 인증서 yaml)은 이 폴더에 추가되되 **git 추적 대상이 아니다**(gitignore `docs/*/*` — 브랜치 간
> persist·통합, `docs/simlog/` 와 동일 규칙). 발행 정본: 스킬 `adversarial-benchmark` full 모드.
> 근거: `seed/letter_2026071516_1` · `docs/plan/plan_26071510`.

## 역할 (5번째 문서형 — simlog 자매)

adversarial-benchmark **full 모드** 종결 시 자동 발행되는 **계측 vault**. 두 산출물이 공존한다:

1. **사람용 report** (`bench_report_<YYMMDDHH>[_MM_SS]_<model>_<gpu>_<vllm>.md`) — **모든 full 런에 발행**(PASS/FAIL 무관).
   부하별 곡선(동시성 스윕)·루프라인 컨텍스트·환경 스냅샷을 사람이 읽는 표로 렌더. **inform-only** —
   verdict 를 *표시만* 한다(판정 권한 없음). 렌더 = 결정론 `render_report.py`(LLM 표저작 ✗).

2. **기계용 인증서** (`benchmark_<YYMMDDHH>[_MM_SS]_<model>_<gpu>_<vllm>.yaml`) — **PASS 일 때만 발행**.
   "이 모델을 이 HW/config 에서 테스트했고 통과했다"는 flat 계약 기록. **carry-forward 재검증 헤더** 필수
   (지도≠정답 — 소비 전 재확인). 발행 = 결정론 `publish_benchmark_record.py`.

3. **Max envelope 보고서** (`max_envelope_<YYMMDDHH>[_MM_SS]_<model>_<gpu>_<vllm>.md`) — **Max 오퍼레이션 실행 시** 발행.
   "이 HW 에서 안전하게 최대 어느 컨텍스트(max-model-len)까지 밀 수 있나"의 안전상한 특성화. **inform-only**
   (특성화 표시 · 판정 게이트 아님). Max = **별도 오퍼레이션**(벤치마커 인프라 공유·native 모드 ✗ · 이중 게이트
   `--confirm-risk`+챗 Y/N). 발행 = 결정론 `render_max_report.py`. 상세 = SKILL.md §8.5.

## 문서형 경계 (testlog 와 다름)

| | benchmark/ | testlog/ |
|---|---|---|
| 성격 | inform-only 계측 렌더 + 기계 계약 | 사람용 판정 서사·증거 |
| 판정 | **없음**(verdict 표시만 — verdict_rule 독점) | PASS/FAIL 판정 정본 |
| 저작 | 결정론 스크립트(LLM ✗) | 에이전트 서술 |

참조 체인: `raw(bench JSON/simlog)` → **`benchmark report`(inform-only 렌더)** → `testlog`(판정 인용) → `devlog`(서사).

## 명명 규칙

```
docs/benchmark/bench_report_<YYMMDDHH>[_MM_SS]_<model>_<gpu>_<vllm>.md   ← full 사람용 report (항상)
docs/benchmark/benchmark_<YYMMDDHH>[_MM_SS]_<model>_<gpu>_<vllm>.yaml    ← full 인증서 (PASS시만)
docs/benchmark/max_envelope_<YYMMDDHH>[_MM_SS]_<model>_<gpu>_<vllm>.md  ← Max 안전상한 특성화 (Max 실행시)
```
- `<model>` = config_name(트리플렛 키, 예 `deepseek-v4-flash`) · `<gpu>` = manifest gpu_model 정규화(예 `GB10`) ·
  `<vllm>` = 대상 vLLM 버전(예 `0.24.0`, 미상 시 `NA`).
- **파일 접두사 = 산출물 종류**(`bench_report_` / `benchmark_` / `max_envelope_`). 여러 모델·HW·버전 조합이 한 폴더에 공존.
- 명명 SSOT(결정론 헬퍼): `scripts/doc_naming.py`(`bench_filename`). **주의**: adversarial-benchmark
  스킬 자체의 `render_report.py`/`publish_benchmark_record.py`/`render_max_report.py` 는 이 루트 파일이
  아니라 **스킬-로컬 사본** `.claude/skills/adversarial-benchmark/scripts/doc_naming.py`(동일 로직의
  벤치 전용 부분집합)를 import 한다 — 루트 SSOT 를 직접 소비하는 것은 **`scripts/evidence_publisher.py
  publish-benchmark`**(plan_26072506 Phase 2 hybrid publisher) 뿐이다. 두 사본은 같은 명명 계약을
  따르지만 서로 다른 파일이다(Phase 2 는 스킬-로컬 사본을 수정·통합하지 않았다 — 범위 밖).
  `publish-benchmark`는 항상 `bench_report`를 발행하고, `certificate`는 **실제로 공급된, 파싱 가능하고,
  verdict=PASS·benchmark_mode=full·강한 식별자(model/gpu/vllm/quant/topology/tp)가 이 발행물의 identity
  와 정확히 일치하는 아티팩트가 있을 때만** 발행한다(모순되는 인증서는 게시 전에 거부 — verdict=PASS
  주장만으로 인증서를 날조하지 않음).

## 인증서(flat 계약) 필수 성질

- **flat 스키마**(중첩 ✗) — 미래 소비 도구가 stdlib/awk 한 줄로 독해.
- **carry-forward 재검증 헤더**: "이건 그때-그 환경 한정 측정 — 소비 전 재확인" 배너 + **강한 일치 키**
  (model·gpu·vllm·quant·topology·tp = 정확일치 실패 시 무효) + **소프트 지문**(driver·ngc·image·max-len·
  kv-bytes·gmu·moe = 불일치 시 stale 경고).
- **N/A fail-soft**: 결측 필드는 `N/A` 원문 기록(대체값 날조 ✗).

## 발행 시점 / 비용 규율

- report = **모든 full 런 종결**(PASS/FAIL) · 인증서 = **PASS 종결시만**. lite 모드는 발행 안 함(채팅 표만).
- 스윕/리치 리포트는 **재탐색 루프 매 회차가 아니라 종결 1회**(비용 규율 — 루프 내부는 값싼 단일점 판정).
