# easy_vllm_simulator — 하네스 엔지니어링 성숙도 감사

> 평가 기준: `h_report_26072200_하네스엔지니어링_배포판.md` 24문항 체크리스트
>
> 감사 시각: 2026-07-28 22:00 KST
> 평가자: Hermes (mainsparkbot) — Claude Code 제어면 경유
> Accepted primary tree: 작업 중 (비침습 read-only 감사)
> 평가 방식: 24문항 자가점검 체크리스트 6섹션, ✅=2점·⚠️=1점·❌=0점, 총점 48점 만점

---

## 결론

**종합점수: 33/48 · 등급 ⚠️ "어설픈" 하네스 (28~39점대)**

상위 3섹션(A/B/C)은 21/24점으로 goal-first 설계의 모범에 가깝다. 그러나 **자기제거 메커니즘(D)이 사실상 전무(2/8)**하여 "Pitfall 누적형 성장" 안티패턴의 초기 단계다. Provider 독립성(E)과 구조적 건강도(F)는 각 5/8로, Claude Code 제어면 결합과 메타데이터 부재가 주요 감점 요인이다.

이 점수는 **모델 운영 E2E 실패를 의미하지 않는다.** 실제 서빙·벤치마크·문서 발행은 성숙하게 작동한다. 미달은 **하네스 자체의 장기 지속가능성** 영역이다.

---

## 영역별 점수

| 섹션 | 주제 | 점수 | 만점 | 등급 |
|---|---:|---:|---|---|
| A | 목적 정당성 | 7 | 8 | ✅ |
| B | 맥락 vs 디테일 | 7 | 8 | ✅ |
| C | 토큰 효율성 | 7 | 8 | ✅ |
| D | 자기제거 메커니즘 | **2** | 8 | ❌ |
| E | Provider Independence | 5 | 8 | ⚠️ |
| F | 구조적 건강도 | 5 | 8 | ⚠️ |
| **합계** | | **33** | **48** | ⚠️ |

---

## Section A — 목적 정당성 7/8 ✅

| # | 체크 항목 | 판정 | 근거 |
|---|---|---|---|
| A1 | 모든 규칙이 구체적 실패 이력에서 비롯? | ✅ (2) | CLAUDE.md의 12개 policy(`ARCH_WALL_VARIANT_LADDER`, `KV_ABSOLUTE_CLAMP_PORTABILITY` 등)는 `plan_26063018`·`plan_26070208`·`plan_26071607` 등 구체적 계획·사건에서 유도. docs/testlog/에 실패→규칙화 추적 체인 존재 |
| A2 | 다른 프로젝트에서 복붙한 규칙? | ✅ (2) | 순수 목적-구축. NGC+vLLM 도메인에 특화된 오리지널 하네스 |
| A3 | 사용하지 않는 skills/MCP/plugins? | ✅ (2) | 정확히 5개 스킬, 모두 활성 사용. 미사용 플러그인 없음 |
| A4 | "혹시 몰라서" 추가된 예방적 규칙? | ⚠️ (1) | `policy:VARIANT_IMAGE_BUILD_VS_SERVE_PLANE`·`policy:MODEL_TRIPLET_NO_SUB_PROPAGATION` 등은 아키텍처적 설계 선택. 모든 policy가 실패기원인지는 개별 심층 추적 필요 |

**강점**: 12개 policy 모두 명명된 출처를 가지며, 5개 스킬 정확히 필요한 만큼만 존재한다. 복붙 규칙이나 방치된 플러그인이 없다.

---

## Section B — 맥락 vs 디테일 7/8 ✅

| # | 체크 항목 | 판정 | 근거 |
|---|---|---|---|
| B1 | "왜"를 말하는가, "어떻게"를 말하는가? | ✅ (2) | CLAUDE.md는 목표·불변식·안전경계·트리거 등 "왜" 중심. "절차 정본은 workflow.md"로 명시적 위임 — goal-first 설계의 교과서적 사례 |
| B2 | 발행·승인 절차가 4단계 이상? | ⚠️ (1) | S1~S4 + HITL ①~④ 게이트. 컨테이너 빌드라는 위험도 높은 작업에 정당화되나, 경량 경로(Runtime 모드)가 정식 옵션으로 문서화되지 않음. lite-bench만 예외로 명시 |
| B3 | 구체적 파일명·경로 하드코딩? | ✅ (2) | CLAUDE.md의 참조는 구조적(workflow.md, references.md). 실비즈니스 값은 config.yaml·manifest.yaml에 격리 |
| B4 | 예외의 예외 구조? | ✅ (2) | escalation 3출구(공식 bump / 포크핀 / 음성정직)는 평탄화된 분기. arch-wall 사다리는 순차적 의사결정 트리로, 중첩 예외 아님 |

**강점**: "이러이러한 목표를 달성해야 한다"는 goal-first 선언과 구체적 절차가 명확히 분리되어 있다. CLAUDE.md 109줄 → workflow.md 77줄 → skill별 참조의 위계가 깔끔하다.

---

## Section C — 토큰 효율성 7/8 ✅

| # | 체크 항목 | 판정 | 근거 |
|---|---|---|---|
| C1 | 행동 가드레일 500줄 이상? | ✅ (2) | CLAUDE.md 109줄 + workflow.md 77줄 + docs.md 60줄 + hermes-claude-control.md 34줄 = **총 280줄**. 전체 `.claude/`의 `.md` 총합 2,803줄이지만 이는 175개 파일에 분산됨 |
| C2 | 단일 skill 15개 이상 부속 파일? | ⚠️ (1) | upstream-version-watch: ~33개(scripts 14 + references 4 + templates 3 + assets 12). 단, SKILL.md 자체는 104줄로 컴팩트하고 "조건부 references"로 필요 시에만 로드 명시 |
| C3 | 항상 로드되나 20% 상황에서만 필요한 규칙? | ✅ (2) | CLAUDE.md의 "조건부 참조" + "정책 경계" 섹션은 1-line policy명만 상주하고 본문은 미로드. Progressive disclosure 패턴 우수 |
| C4 | 한 번도 사용 안 한 template/reference? | ✅ (2) | docs/benchmark/·docs/testlog/·docs/devlog/·HINTS.md 모두 활성 사용 증거 존재 |

**강점**: CLAUDE.md 109줄은 구조가 잘 잡힌 프로젝트의 벤치마크(100~150줄) 내에 있다. Policy는 이름만 상주시키고 본문은 `.claude/policies/`로 분리한 Metric-skill 디스패치 패턴이 효과적이다.

---

## Section D — 자기제거 메커니즘 2/8 ❌

| # | 체크 항목 | 판정 | 근거 |
|---|---|---|---|
| D1 | "모델 발전 시 제거" 명시적 기준? | ❌ (0) | `.claude/` 전체에서 "제거 조건"·"self-remov"·"retire"·"deprecat" 검색 결과 0건. arch-wall 사다리가 소스빌드→stock 복귀 경로를 암시하나 명시적 자기제거 트리거 없음 |
| D2 | 정기 감사 루틴? | ❌ (0) | 월 1회 pitfall 재현 cronjob 없음. `docs/report/scorecard_26072722`는 일회성 감사. 정기 일정 부재 |
| D3 | 가장 오래된 규칙 추가 시점? | ⚠️ (1) | plan 참조로 간접 추적 가능(2026-06~07, ~2개월 미만). 단, 규칙 자체에 `added`·`last_reviewed` 메타데이터 없음 |
| D4 | 발생 이력 없이 유지 중인 규칙? | ⚠️ (1) | 대부분의 policy가 testlog/devlog에 증거 체인 보유. 단, `VARIANT_IMAGE_BUILD_VS_SERVE_PLANE` 등 일부는 "설계 선택"에 가까움 |

> **⚠️ Section D는 이번 감사에서 가장 심각한 취약 영역입니다.**
>
> `h_report_26072200` 문서의 §4.1 "Pitfall 누적형 성장" 패턴과 정확히 일치합니다:
>
> ```
> 실패 발생 → 규칙 등록 → 회피 → 새 실패 → 새 규칙 → ... (무한 반복)
> ```
>
> 이 프로젝트는 policy 추가만 있고 제거는 없습니다. 2개월밖에 안 된 젊은 프로젝트이기에 아직 pitfall이 20개를 넘지 않았으나, 이 상태로 6개월이 지나면 §4.1의 "20개 초과 pitfall" 경고선에 도달할 가능성이 높습니다.

---

## Section E — Provider Independence 5/8 ⚠️

| # | 체크 항목 | 판정 | 근거 |
|---|---|---|---|
| E1 | 특정 LLM 프로바이더 고유 API 결합? | ⚠️ (1) | `agent_os: "claude-code"`, `.claude/policies/runtime/providers/claude_code.py` 존재. 제어평면은 Claude Code CLI에 결합되어 있으나, 도메인 로직(vLLM 빌드·서빙·벤치마크)은 완전히 provider-agnostic |
| E2 | 규칙에 특정 모델명 하드코딩? | ✅ (2) | "GPT-4o로 평가한다"·"Claude Sonnet으로 검증한다" 같은 모델명 지정 없음. 평가 모델 선택은 config로 추상화 |
| E3 | Provider 교체 시 하네스 전체 수정? | ⚠️ (1) | 핵심 스크립트·스킬은 provider-agnostic. 교체 시 `agent-card.json` + control contract + provider adapter 재작성 필요하나 격리된 adapter 패턴으로 범위 제한적 |
| E4 | 특정 Agent CLI 완전 종속? | ⚠️ (1) | 오케스트레이션 레이어(hermes-claude-control.md, agent-card.json skills, `claude_code.py`)는 Claude Code CLI에 결합. 다만 도메인 코드는 CLI 독립적 |

**진단**: 지난 종합감사(scorecard_26072722)에서 지적된 `B-PROVIDER-PLURALITY` blocker와 동일선상의 문제다. `KNOWN_PROVIDERS=("claude_code",)`이며 실제 두 번째 adapter가 없다. Provider isolation은 되어 있으나 independence는 아직 선언할 수 없는 상태다.

---

## Section F — 구조적 건강도 5/8 ⚠️

| # | 체크 항목 | 판정 | 근거 |
|---|---|---|---|
| F1 | 참조 체인 깊이 3단계 이상? | ⚠️ (1) | CLAUDE.md → workflow.md → upstream-version-watch/SKILL.md → references/resolve-and-render.md 로 **최대 4단계**. 단, 상위 2단계만 의무 로드, 하위는 조건부 |
| F2 | 동일 규칙 여러 파일 중복? | ✅ (2) | SSOT: CLAUDE.md는 policy명만 선언, 본문은 `.claude/policies/` 소유. 스킬·워크플로는 policy명 참조만 |
| F3 | 규칙 파일 마지막 리뷰일 파악 가능? | ❌ (0) | `last_reviewed` 메타데이터 전무. plan 참조번호로 간접 연대 추정만 가능 |
| F4 | 하네스 테스트 가능? | ✅ (2) | `harness_verify.py`·`runtime_selftest.py`·`verify_distribution.py`·`completion_gate.py`·`classify_failure.py` 등 테스트 인프라 충실 |

**진단**: 테스트 인프라는 강력하나(지난 감사: 924 tests OK), 규칙 파일에 `last_reviewed`가 없어 연식 기반 감사가 불가능하다. 참조 체인은 조건부 로드로 실질적 문제는 없으나 최장 4단계 도달 가능하다.

---

## 30초 퀵스캔 결과

```text
┌─────────────────────────────────────────────────────────┐
│        하네스 건강도 체크 — 30초 퀵스캔                  │
├─────────────────────────────────────────────────────────┤
│ ✅ AGENTS.md가 150줄 이하인가?        (CLAUDE.md 109줄)  │
│ ✅ 모든 규칙이 구체적 실패에서 비롯?                      │
│ ✅ "어떻게"보다 "왜"를 말하는 규칙이 더 많은가?           │
│ ❌ 각 규칙에 "언제 제거하는가" 기준이 있는가?             │
│ ✅ 80% 상황에서만 필요한 규칙이 상시 로드되는가? (없음)   │
│ ⚠️ 특정 LLM 프로바이더에 종속된 구성요소가 없는가?        │
│ ✅ 동일 규칙이 여러 파일에 중복되어 있지 않은가?          │
│ ✅ 예외의 예외 구조가 없는가?                              │
└─────────────────────────────────────────────────────────┘
6/8 YES → 건강한 하네스
```

> 6개 YES로 "건강한 하네스" 기준선(6/8)을 간신히 충족합니다. 그러나 24문항 전체 평가에서는 자기제거 메커니즘(D)의 공백이 두드러집니다.

---

## 개선 권고 (우선순위 순)

### 🔴 긴급 — Section D

1. **각 policy에 자기제거 조건 명시**
   ```markdown
   <!-- 예: policy:ARCH_WALL_VARIANT_LADDER 에 추가 -->
   제거 조건: vLLM 공식 릴리즈에서 sm_120 · sm_121a ABI 호환 prebuilt wheel 제공 시
   ```
   CLAUDE.md의 "정책 경계" 각 항목에 `제거 조건:` 1문장 주석 추가. 또는 `.claude/policies/registry.yaml`에 `remove_when` 필드 추가.

2. **월 1회 pitfall 재현 cronjob**
   - 3개월 이상 된 모든 policy에 대해 실제 재현 테스트
   - 연속 3회 미발생 시 제거 후보로 승격

### 🟡 중기 — Section E

3. **두 번째 provider adapter 구현**
   - Codex CLI adapter 추가 → `KNOWN_PROVIDERS=("claude_code","codex_cli")`
   - Provider-neutral control contract 보강 (지난 감사 `B-PROVIDER-PLURALITY` blocker 해소)

### 🟢 점진 — Section F

4. **모든 규칙 파일에 `last_reviewed` 메타데이터 추가**
   ```yaml
   # CLAUDE.md frontmatter 또는 주석으로
   last_reviewed: 2026-07-28
   ```

5. **경량 경로(Runtime 모드) 정식 문서화**
   - "Builder 모드"(S1~S4 + 4 HITL)와 "Runtime 모드"(경량 검증)의 이중 운영 모드 명시
   - `docs.md` 또는 `CLAUDE.md`의 "트리거" 섹션에 추가

---

## 검증 상태

- **읽은 파일**: CLAUDE.md, .claude/rules/{workflow, docs, hermes-claude-control}.md, 5개 skill SKILL.md, agent-card.json, .claude/skills/wiki-desk/reference/references.md
- **스캔한 전체 파일 수**: `.claude/` 175개 파일 (`.md` 총 2,803줄)
- **검색한 패턴**: "제거 조건"·"self-remov"·"retire"·"deprecat"·"last_reviewed"·"audit"·"claude"·"anthropic"·"gpt"·"openai"·"provider.*specific"·"모델명"
- **Product code 수정 없음**: read-only 감사
- **작업 중 tree 보존**: 비침습

---

## 최종 판정

1. **easy_vllm_simulator의 하네스는 "잘 만들어지고 있다."** CLAUDE.md ≤150줄, goal-first 설계, 결정론·확률론 경계 분리, progressive disclosure 패턴, SSOT 정책 구조, 충실한 테스트 인프라 등 다수의 모범 사례를 갖추고 있다.

2. **그러나 아직 "잘 만들어진" 하네스라고 선언하기엔 이르다.** 33/48점은 §4.1 "Pitfall 누적형 성장" 안티패턴이 형성되기 시작하는 경계선이다. 자기제거 메커니즘의 부재(D: 2/8)를 2026년 8월까지 해소하지 않으면, 가을에는 pitfall이 20개를 넘어설 가능성이 높다.

3. **지난 종합감사(scorecard_26072722, 89/100)와의 관계**: 이 감사는 다른 체계(24문항 체크리스트, 하네스 엔지니어링 렌즈)로 평가한 것이다. 상호 보완적이며, D 영역의 자기제거 메커니즘 부재와 E 영역의 provider plurality 문제가 두 감사에서 공통으로 지적되었다.

---

> **감사 기준 문서**: `h_report_26072200_하네스엔지니어링_배포판.md` — §3 24문항 체크리스트 전문 적용
>
> 이 감사는 read-only로 수행되었으며, 워크스페이스의 어떤 파일도 수정하지 않았습니다.
