# devlog 260607-1 — Step 1: 작업환경 셋업 (에이전트 부트스트랩)

## 작업 요약

이 레포를 *"업스트림 vLLM 추적 → 커스텀 Docker 컨테이너 버전관리 에이전트"* 로 부팅하는
5단계 부트스트랩의 **Step 1(작업환경 셋업)** 을 완료했다.
근거: Ouroboros 딥 인터뷰 `interview_20260606_205859` → Seed `seed_26269d6807f5`(QA 0.92 PASS),
계획서 `docs/plan/plan_260607_1_step1_작업환경셋업_계획서.md`.

---

## 1. 산출물

| 파일 | 위치 | 줄수 | git 처리 |
|------|------|------|----------|
| `CLAUDE.md` | 레포 루트 | 72 | `.gitignore` 추가 → 빌딩블럭(origin push 안 함) |
| `workflow.md` | `.claude/rules/` | 72 | 기존 `.claude/` ignore 규칙으로 제외 |
| (수정) `.gitignore` | 루트 | — | `CLAUDE.md`, `CLAUDE.local.md` 항목 추가 |

- `CLAUDE.md`: 정체성 · 핵심사실(레이어 커플링·빌드입력·의존성 원천) · 수동 핀/트리거 ·
  build/검증 커맨드 · 검증게이트(smoke-before-push) · 금지(모델 자동다운로드·임의설치·빌딩블럭 push·확률론 해소) ·
  롤백(origin=last-good) · 스킬/도구 경계.
- `workflow.md`: 런타임 전파 4단계(S1 resolve → S2 patch → S3 smoke → S4 push) + 단계별 HITL 게이트 4개,
  트리거(수동)·롤백·기록·부트스트랩 5단계 참조.

## 2. 공식 Claude Code 가이드 준수

- CLAUDE.md ≤200줄(실제 72줄), "항상 참인 사실"만. 다단계 절차는 `.claude/rules/workflow.md`로 분리.
- 충돌 규칙 없음. 한국어(기존 docs 관행), 명령어·경로·키는 영문.
- 근거: `seed/bundle_pkg/REFERENCES.md` §A3 (docs.claude.com / code.claude.com).

## 3. 핵심 결정 반영

- **레이어 커플링 규칙**(가장 중요): 대상 vLLM `pyproject.toml`의 명시된 torch 버전 확인 →
  NGC `NVIDIA_PYTORCH_BUILD_VERSION` 접두어 매칭 베이스 선정. (사용자 교정: 부정어 대신 절차 지시형)
- **빌딩블럭 vs 런타임**: `.claude/`·`CLAUDE.md`·`seed/`는 origin push 금지(gitignore). 런타임(Dockerfile 등)만 push.
- **모델 자동 다운로드 금지** · **결정론적 스크립트로 버전 해소**(하네스 엔지니어링) · **origin=last-good 롤백**.

## 4. 완료 기준 점검

- [x] bundle done-criterion 1: 인터뷰 11영역 전수 해소 (Seed 0.92)
- [x] bundle done-criterion 2: 산출 아티팩트 `<!-- ASK -->` 빈칸 0 (스킬/config 슬롯은 Step 2로 의도적 보류)
- [~] bundle done-criterion 3: CLAUDE.md/rules 로드 — 다음 세션 자동 로드 / `/memory`로 확인(사용자 검증)
- [x] 문서 발행 규칙 확립: `docs/plan/`(계획) → 검토 → 실행 → `docs/devlog`·`testlog`(기록) → 다음 계획서

## 5. 다음 단계

→ `docs/plan/plan_260607_2_step2_스킬설계_계획서.md`: `upstream-version-watch/SKILL.md` +
결정론적 `scripts/`(기존 `configs/check_reqs.py` 발전형) + `config.yaml` 스키마 설계. 단일노드 기준 우선.
