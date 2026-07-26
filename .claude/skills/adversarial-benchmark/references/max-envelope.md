# Max 모드 — HW 안전-최대 컨텍스트 envelope 특성화 (조건부 reference · **별도 오퍼레이션**)

> **벤치마커 native 모드(적대검증)가 아니다.** 벤치마커의 *측정 인프라만* 공유하고, 진입·트리거·안전게이트는 전부 별도.
> **Max 가 기동/reload 를 소유**하므로 벤치마커 본체 "기동 안 함" 불변식은 보존된다.
> **라이브 E2E 는 후속**(실 스텝업 = 실제 하드다운 위험 → 드라이버 안전 재검증과 함께 신중히).
> 결정론 로직은 dry-run+fixture 로 검증됨. 근거: `seed/letter_2026071516_1` · `plan_26071510` · `testlog_26071113`.

## 1. 정체성

Max 는 config 를 **재서빙(reload)** 하며 탐색하므로 벤치마커 "기동 안 함" 불변식과 충돌 → **서브모드 ✗**.
측정 라이브러리(`run_bench`·`verdict_rule`·render 계열)만 재사용하고 진입·트리거는 완전 별도(미래 soak-stress 스킬과 동형 배치).
코드는 `adversarial-benchmark/scripts/` 에 동거(인프라 재사용)하되 **정체성은 별도**.

- **목적**: HW **안전-최대** envelope 특성화 — explorer("용처-최적 config *선택*") ↔ Max("절대-최대 config *특성화*"). 중복 ✗.

## 2. 축 = 컨텍스트(max-model-len) 안전측 스텝업

```bash
max_envelope.sh <config> [--levels 131072,262144,393216,524288] --confirm-risk [--dry-run]
```

낮은 컨텍스트 → 높은 컨텍스트 **오름차순 재서빙**, 각 레벨 serve+smoke 통과=안전·기록·상향 / 실패·트립=**직전이 안전상한**·중단
(안전측 적응 클램프 + 절삭 로그). 컨텍스트 축은 weights 불변이라 KV/prefill 위험을 **워치독+스모크가 관측**한다
(하드다운 봉투 512k안전/768k치명과 동형). batch 축은 full 경로의 client-load 스윕이 부분 커버.
**default 상한 = 524288(검증된 안전상한)** — 초과 probe 는 `--levels` 명시로만.

## 3. 트리거 · 안전 이중 게이트

- **트리거(완전 옵트인 · 자동 아님)**: **전작업 완료** 후에만 — 서빙 확정 + 문서(report/인증서) 발행 + wiki 등록까지 끝난
  지점에서 **에이전트가 챗 경고톤 Y/N**("Max 벤치? — reload 반복·통합메모리 하드다운 위험") → 승인 시에만 스크립트 실행.
  **"무인 자동실행 없음" 유지** · **선-기록 후-위험**(각 레벨 결과를 다음 시도 전 기록 — 하드다운이 진행분을 소실시키지 않게).
- **안전 이중 게이트**: (1) 스크립트 `--confirm-risk` 명시(무심코 실행 차단·미명시 exit 5) (2) 에이전트 챗 Y/N.
  \+ serve+smoke = `multinode_serve_smoke.sh`(협역 워치독 자동 arming + 로드-전 RAM 게이트 내장) ·
  per-level config 스냅샷 + EXIT-트랩 원복. 헌법 §호스트 안전체계 따름정리 정합
  (**파킹된 드라이버 안전 재검증의 실행 vehicle**).

## 4. 산출물

`max_envelope.sh` → `max_index.json` → `render_max_report.py` →
`docs/benchmark/max_envelope_<YYMMDDHH>_<model>_<gpu>_<vllm>.md`(안전상한·레벨별 결과·절삭 로그 · **inform-only** 특성화 표시 ·
판정 게이트 아님). single-node serve+smoke = 후속(현재 multi 우선).
