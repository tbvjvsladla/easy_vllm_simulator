# checklist/ — bot 전용 단계 트래킹 (3-Phase)

> 이 `example.md`는 **추적되는 스켈레톤**이다(폴더 구조 + 작성 규칙 배포용). 실제 체크리스트 문서는
> 이 폴더에 추가하되 **git 추적 대상이 아니다**(gitignore — 브랜치 간 persist·통합). 작성 규칙 정본: `.claude/rules/docs.md`.

## 역할
- Unlazy 감사(Planner→Builder→Validator)의 **현재 위치 단일 진실원천**. 최신 snapshot 기준으로 읽는다.
- 담는 것: phase별 항목 체크(`- [ ]`/`- [x]`), 재개지침(`PARKED → 재개 시`), gate 계약(`CHECK`+`EXPECT`+`EVIDENCE`).
- **메인/bot 전용** — 서브 skeleton에 렌더하지 않는다(docs.md §서브 docs 계약: report·request·checklist는 메인 전용).

## 파일명 규칙
```
docs/checklist/checklist_<YYMMDDHH>[_<MM>_<SS>]_<주제>.md
```
- `YYMMDDHH`: 발행 일시 절대표기 · 2자리 연도 (예 `26060814` = 2026-06-08 14시). 상대날짜 금지.
- `_MM_SS`: 같은 `YYMMDDHH`(같은 type)에 충돌 시에만 분·초 접미(구 `_seq_` 표기는 폐지됨).
- `주제`: 한국어, 밑줄(`_`) 구분.
- 명명 SSOT(결정론 헬퍼): `.claude/skills/wiki-desk/scripts/doc_naming.py`(`dated_doc_basename`, `type=checklist`).
