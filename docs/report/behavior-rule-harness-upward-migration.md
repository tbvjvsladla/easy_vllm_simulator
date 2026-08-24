# 행동규칙 하네스의 상방 이동 — Unlazy 검증 · Boris 박형 감사

> 문서형: report (docs/report 무일자 예외 — kebab slug)
> 작성: 2026-08-24 KST
> 목적: easy_vllm_simulator 하네스에서 "~하라/~하지 마라" 행동규칙(산문)을 실행 게이트(안전불변식)로
> 상방 이동한 거버넌스 감사의 ①개념 ②검증 방법 ③박형 적용을 기록한다. 슬랙봇이 Unlazy 전역 검증자(Boris auditor
> 역할)로, easy_vllm_simulator Claude Code 에이전트는 맹인 검토 대상으로 분리된 ablation-first 감사다.

## 서론

easy_vllm_simulator 하네스는 헌법(CLAUDE.md)의 불변식·정책을 기초 레이어로 두고, 그 아래 스킬·술어가
"어떻게"를 소유한다. 그런데 "어떻게" 레이어에는 두 종류의 글이 섞여 있었다. **절차 서술**(~순서/~단계 — 저성능
로컬 LLM의 재현성을 위한 scaffolding)과 **행동규칙 산문**(~하라/~하지 마라 — "push는 사용자 소관", "무인 자동
태깅 금지", "보험판매식으로 권유하지 마라" 등)이 그것이다. 전자는 유지하고 후자만 실행 게이트로 상방 이동하는
것이 이번 감사의 목표다.

## 1. 하네스 상방 이동 개념

### 준위 체계

| 준위 | 형태 | 예 | 성질 |
|---|---|---|---|
| **준위 0** | 산문(prose) | "push 는 전부 사용자 소관" | 선언만 있고 실행 신호 없음 — 위반해도 아무것도 못 막음 |
| **준위 1** | 실행 게이트(코드) | `_require_terraform_flag(repo_root)` → exit 4 | fail-closed — 위반 시 결정론적으로 차단 |

상방 이동이란 **"글로만 존재하는 규칙을, 그 규칙을 실제로 집행하는 코드로 끌어올리는 것"**이다. 산문은
에이전트가 읽고 따르길 "기대"할 뿐이지만, 실행 게이트는 위반 경로 자체를 닫는다. B1이 그 증명이었다: "push는
사용자 소관"이라는 산문을 제거해도, 맹인 에이전트는 push 인가를 코드 4중 게이트(argparse `--manifest required`
→ `_require_central("push")` → `_require_promotion_authorization` → `_require_all_hint_tags_evidence_valid`
+ `--apply` 관문)에서 정확히 도출했다. **산문은 중복이었다.**

### 경계 기준 — 헌법 = 왜, 스킬 = 어떻게

상방 이동은 무분별한 "헌법 개정"이 아니다. 경계 기준은 확정되어 있다: **헌법(CLAUDE.md)은 "왜"(불변식, 왜 이
규칙이 안전에 필수인가)를 고정하고, 스킬·술어는 "어떻게"(도메인 절차)를 소유한다.** 스킬로 해결될 것을 헌법으로
올리지 않는다. 이 감사에서도 절차 scaffolding(~순서/~단계)은 전부 유지했고, "~하라/~하지 마라" 행동규칙만
실행 게이트로 상방 이동하거나 중복임이 증명되면 삭제했다.

## 2. Unlazy 검증 방법

Unlazy는 "주장을 실제 실행 결과로 검증하라"는 방법론으로, 세 원자로 구성된다: **CHECK**(무엇을 확인하는가),
**EXPECT**(통과 기준은 무엇인가), **EVIDENCE**(어떤 실행 결과가 그것을 증명하는가). 이 감사에서 슬랙봇(Hermes)은
Unlazy를 **전역 검증자**로 사용하고, easy_vllm_simulator의 Claude Code 에이전트는 **맹인 검토 대상**으로 삼는
ablation-first 구조를 취했다.

### 맹인 E2E (read-only 판단 호출)

각 배치의 검증은 "맹인 E2E"로 수행했다. 산문을 제거한 상태의 fresh Claude Code 세션에 `--allowedTools Read`만
주고, "이 행동의 권한이 **코드로 강제**되는지 **산문으로만 안내**되는지 판단하라"를 JSON으로 반환시킨다. 이는
완전 relay(서빙/벤치)보다 저비용이면서 ablation 검증에 충분하다.

- **B1 실측**: `push_requires_gate=true · auto_push_possible=false` — 산문 없이도 push 인가를 코드 게이트에서 도출.
- **B2 실측**: `host_safety_optin_enforced_by_code=true · block_when_declined=false · prose_nag_rule_remaining=false`
  — 호스트안전 설치의 opt-in 성격이 코드(APPLY=0 dry-run 기본 + --apply root 요구)로 강제됨을 확인.

### ablation 루프

각 배치는 ①제거(산문 + 해당 `_require` 원자 동시) → ②맹인 E2E → ③게이트(Unlazy CHECK/EXPECT/EVIDENCE) →
④재도입 판정(반복해서 깨지는 행동만 실행 게이트로 재도입, 1회 통과면 영구 삭제) 순으로 진행했다. "실측인척
금지" 원칙에 따라 게이트는 실행 결과를 실제로 관측해야 통과로 인정했다.

## 3. Boris 박형법 적용

Boris의 "하네스를 얇게 만드는" 접근은 **규칙을 삭제하는 것이 아니라, 실행 게이트가 이미 담보하는 행동을
중복 기술하는 산문만 걷어내는 것**이다. AST 기반으로 56개 술어·13 family·434개 `_require` 원자를 정밀 분류한
결과, 45개 "산문" 원자로 보였던 것의 실체는 3종이었다.

### 3종 정밀 분류

| 종 | 판별 | 예 | 처리 |
|---|---|---|---|
| **실행 앵커** (준위 1 실제) | 코드가 `index()`/출력 검증으로 참조하는 문자열 | `'--hf-repo-id 필수'` · `'DRY-RUN 종료'` · `'startup free-memory 게이트'` | **유지** (제거하면 술어가 깨짐) |
| **절차 서술** | ~순서/~단계 scaffolding | `'스모크 통과분만 로컬 last-good 커밋'` · `'재부팅 1회'` | **유지** |
| **행동규칙 scolding** | ~하라/~하지 마라 금지·소관 | `'무인 자동 태깅 ✗'` · `'모델-키잉 금지'` · `'무단 다운로드 없음'` | **제거/박형** |

즉, "산문 45개"는 과대집계였고, 실제 제거 대상인 "행동규칙 scolding"은 **소수**였다. 나머지는 실행 게이트가
이미 강제하는 불변식을 문서로 되풀이한 것(중복)이거나, 저성능 로컬 LLM의 재현성을 위해 의도적으로 둔 절차였다.

### 6배치 ablation (저위험 → 고위험)

| 배치 | 테마 | 결과 |
|---|---|---|
| **B1** | push/태깅 소관 | 영구 삭제 — 코드 4중 게이트로 재도입 불요 증명 |
| **B2** | 호스트안전 선택조항 표현(보험판매식 잔소리) | 영구 삭제 — 맹인 E2E `code_enforced` |
| **B3** | 기타 산문(모델-키잉 금지) | 삭제 — 실행 게이트(`build_context` 시그니처)가 강제 |
| **B5** | 모델획득(무단 다운로드 없음) | 삭제 — read-only 마운트(compose)가 강제 |
| **B4/B6** | Flag 게이트·서브 회수 규칙 | **안전불변식으로 유지** (산문이 아닌 실질 규칙) |

B4(플래그게이트)·B6(서브 회수)은 "scolding"이 아니라 **실질 안전불변식 자체**로 판명되어 제거하지 않았다.
대신 이들을 포함한 전체 안전불변식 하네스가 "설계 의도에 정합하게 게이트 역할을 수행하는지"를 감사하는 방향으로
전환했다.

## 4. 감사 결과 — 게이트 강제공백 3건 개정

안전불변식 하네스는 대부분 **실행기반 fail-closed**(변이민감 매트릭스 — 양성 대조 + 각 차원 단독 flip)로
설계 의도에 정합하게 동작했다. 다만 3지점에서 "설계의도(불변식) vs 실제강제" 불일치가 확인되어 개정했다.

| # | 게이트 | 불일치 | 개정 |
|---|---|---|---|
| **1** | HOST_SAFETY C7 `재부팅 1회` | 문자열 존재만 확인, 실제 재부팅은 HITL | "다음 단계(사람)" 위임 + `reboot` 실행문 부재(무인 재부팅 금지) 단언 추가 |
| **2** | TERRAFORM C2 `렌더·빌드·bump ✗` | recipe/bench는 코드 강제, **upstream은 문서(S3 워크플로)만** | `render_dockerfile.py`에 `_require_terraform_flag` bake-in (recipe.py 동일 계약) |
| **3** | MODEL_ACQUISITION C2 `다운로드 금지` | 블랙리스트(6종)만 — `subprocess` 경유 우회 가능 | 블랙리스트 확대(urllib/requests/http.client/socket/httpx/aiohttp/os.system/os.popen/Popen) + subprocess=로컬 스펙스크립트 전용 단언 |

개정 후 `verify_distribution.py` **61/61 PASS**(56 술어 + digest/snapshot 재기록). render 게이트는 실행 검증으로
Flag-absent → exit 4, Flag-valid → 통과, 테스트 override → bypass를 확인했다.

## 5. 산출물·증거 요약

- **감사 작업공간**(bot 전용, gitignored): `docs/plan/`·`docs/devlog/`·`docs/checklist/` — 3-Phase(Planner→
  Builder→Validator)로 항목별 dated 발행. `checklist`는 신규 doc type으로 `doc_naming.py` SSOT에 등록.
- **하네스 변경**: `.claude/policies/predicates/claim_predicates.py`(56 술어), `render_dockerfile.py`,
  `terraforming_node/SKILL.md`, `manifest.template.yaml` + digest 3종(`tracked_index.json`·
  `evidence_manifest.json`·`governed_prose_snapshot.json`).
- **전파**: single-node → multi-node(sync_branches) → 서브 노드(sync_to_sub, 런타임블럭만 전달) 순으로
  모든 노드에 반영. digest/snapshot은 git blob SHA1로 재기록해야 verify가 통과한다(자기참조 해소를 위해
  재기록 2회 + `git add -u` 순서가 중요).

## 결론

이 감사가 보여준 것은 두 가지다. 첫째, **"산문 규칙 제거"는 사실상 "중복 산문 제거"였다** — 실행 게이트가 이미
강제하는 불변식을 글로 되풀이한 것이 대부분이었고, 맹인 E2E가 이를 실측으로 확증했다. 둘째, **남는 것은
안전불변식 하네스 자체의 강제력 검증**이었고, 그 검증에서 upstream Flag 게이트·다운로드 블랙리스트·재부팅 HITL
세 곳의 강제공백을 찾아 실행 게이트로 보강했다. 상방 이동의 최종 형상은 "얇아진 헌법 + 두꺼워진(더 fail-closed 한)
실행 게이트"다.
