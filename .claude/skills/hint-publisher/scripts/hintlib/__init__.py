"""hintlib — hint-publisher 의 모듈 패키지 (plan_26092119 §4.9 · 2026-09-21 재구성).

단일 CLI `scripts/hint.py` 가 이 패키지를 부른다. 모듈 배치:

    core       저장소 경로 · 실패 표현(HintError) · git · 주입 시각 · 결정론 직렬화 · 교차 스킬 모듈 적재
    naming     태그 이름 파생(도구 전량 파생 · D5~D8) · 어휘표 · 신/구 문법 파서(구 문법 읽기 전용)
    pii        PII 스캐너 단일본(배포 4종 · 비배포 산문 3종) · 발췌 기계 치환
    evidence   셀 증거 수집(캠페인 · 발행 기록) · 발행 자격 관측 · completion_gate · evidence_publisher 구동
    lineage    계보 파생(LINEAGE.json)
    artifacts  슬롯 발견 · 적용 판정(빌드 원장) · 실행 평면 · 서브 레시피
    template   챕터별 기재 지시(PROMPT) 템플릿 · 사실 블록 · 린터 · 발췌 무결성
    branch     hint 브랜치 배관 커밋(합성 신원 · 주입 시각)
    tag        봉인(annotation = brief + 포인터 + footer) · 로컬 검증 · 선별 push
    catalog    원격 발행 태그 → hints/index.json + HINTS.md 파생

**import 시점 부수효과 0** 이 패키지 전체의 불변식이다 — git 을 부르지 않고, 저장소 경로를 계산하지
않고, 파일을 읽지 않는다. 옛 `hint_tag.py` 는 import 순간 `ROOT = repo_root()` 로 git 을 불렀고, 그 한 줄이
다섯 호출자(claim_predicates · campaign_template_validator · push_branches · runtime_selftest ·
selftest_hint_gate)에 우회 코드를 낳았다(2026-09-21 코드맵 H1). 저장소는 언제나 인자로 받는다.
"""
