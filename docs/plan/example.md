# plan/ — 계획서 (작업 **착수 전**)

> 이 `example.md`는 **추적되는 스켈레톤**이다(폴더 구조 + 작성 규칙 배포용). 실제 계획 문서는
> 이 폴더에 추가하되 **git 추적 대상이 아니다**(gitignore — 브랜치 간 persist·통합). 작성 규칙 정본: `.claude/rules/docs.md`.

## 역할
- 단계/Phase 작업의 **계획·설계·접근방식**. 실행 전 **사람 검토(HITL)** 합의용.
- 담는 것: 목표·범위, 결정론적 해소값 설계, 단계별 절차, 리스크, 검증(합격) 기준.
- 폐기/대체된 plan 은 헤더에 배너를 단다(후속 plan 작성 에이전트의 의무 — 정본: docs.md §3):
  `> ⛔ SUPERSEDED by `docs/plan/<후속 plan>.md``

## 파일명 규칙
```
docs/plan/plan_<YYMMDDHH>[_<MM>_<SS>]_<주제>.md
```
- `YYMMDDHH`: 작성 일시 절대표기 · 2자리 연도 (예 `26060814` = 2026-06-08 14시). 상대날짜 금지.
- `_MM_SS`: 같은 `YYMMDDHH`(같은 type)에 충돌 시에만 분·초 접미(구 `_seq_` 표기는 폐지됨).
- `주제`: 한국어, 밑줄(`_`) 구분, 버전·대상 포함 권장.
- 예: `plan_26060814_멀티노드_소스빌드_확장_계획.md`
- 명명 SSOT(결정론 헬퍼): `scripts/doc_naming.py`(`dated_doc_basename` — evidence_publisher.py가 소비).
