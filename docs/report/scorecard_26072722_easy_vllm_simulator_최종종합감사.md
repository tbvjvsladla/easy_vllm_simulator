# easy-vllm-simulator 최종 종합 성숙도 감사

- 감사 시각: 2026-07-27
- Accepted primary commit: `ed08c3f719dc985ca5ba230f8f32aefa7d900c6e`
- Accepted primary tree: `0e9d6f63fc8fc7e456a4eb216a072c67376b0018`
- 평가 방식: 사전 선언한 3-lane 100점 scorecard, exact-tree 독립 reviewer 3명, 구조적 blocker fail-closed
- 이전 baseline 종합점수: 확인 가능한 기록 없음. 따라서 개선 델타는 산출하지 않고 현재 절대점수만 보고한다.

## 결론

**종합점수: 89/100 (A- 수준)**

점수·영역 floor는 모두 통과했으나 구조적 blocker 4건이 남아 있어 최종 성숙도 verdict는 **FAIL-CLOSED / 조건부 승인 불가**다. 이는 모델 운영 E2E 실패를 의미하지 않는다. 계획된 E2E는 모두 완료됐고 운영 lane은 PASS다. 미통과 대상은 프로젝트 전체의 구조적 완결성 선언이다.

## 영역별 점수

| Lane | 배점 | 점수 | Floor | 판정 |
|---|---:|---:|---:|---|
| A. 헌법·policy trust | 35 | **31** | 28 | 점수 통과, blocker 2 |
| B. 5개 skill·workflow 품질 | 30 | **26** | 24 | 점수 통과, blocker 2 |
| C. 운영 E2E·안전·evidence closure | 35 | **32** | 28 | PASS, blocker 0 |
| **합계** | **100** | **89** | **85** | 점수 통과, blocker gate 실패 |

## Lane A — 31/35

| 항목 | 점수 |
|---|---:|
| 헌법/SSOT 및 registry 완결성 | 10/10 |
| 56 clause→binding→predicate 폐쇄 및 mutation sensitivity | 8/10 |
| topology-neutral renderer/tracked-index/evidence trust | 8/10 |
| lifecycle/self-removal/provenance | 5/5 |

확인된 강점:

- 정확히 5 public skills, 13 policies, 56 clauses, 56 bindings, 56 dedicated predicates
- evidence entries 29, tracked entries 93
- 폐쇄형 registry schema, governed-prose snapshot, citation closure
- 명시적 review date/remove-when/provenance 및 removal lifecycle
- canonical renderer, source/materialized/destination byte·mode trust

### A-MUT-01 — 확정 blocker

`HOST_SAFETY_LAYERED_DEFENSE.C5` dedicated predicate가 일부 Docker command 문자열 부재만 검사한다. Disposable exact-tree copy의 `scripts/mem_watchdog.sh`에 다음 material violation을 삽입해도 predicate가 성공했다.

```text
podman build -t unattended-policy-violation .
A_MUT_ACCEPTED
exit 0
```

따라서 56:56:56 cardinality는 완전하지만 atom-level mutation sensitivity는 완결되지 않았다.

### A-INDEX-GITLESS-01 — gitless/export trust gap

정방향 `opened - index == ∅`와 required subset은 검증하지만, gitless tier에서 역방향 `index - justified_closure == ∅`는 검증하지 않는다. Ambient Git에서는 staged blob 재검증 테스트가 surplus/nonexistent entry를 잡지만, gitless/export production verifier는 모든 entry의 존재와 정당화된 closure를 독립적으로 재도출하지 못한다. 따라서 blocker 범위는 gitless/export trust로 한정한다.

## Lane B — 26/30

| 항목 | 점수 |
|---|---:|
| 역할 분리·interface 명확성 | 6/6 |
| deterministic script와 LLM 경계 | 6/6 |
| 이식성·topology-neutral·provider boundary | 4/6 |
| 실패복구·HITL·안전 계약 | 6/6 |
| context economy·attachment usage·maintenance | 4/6 |

확인된 강점:

- `terraforming_node → upstream-version-watch → vllm-recipe-explorer → adversarial-benchmark` ownership chain
- `wiki-desk` read-only sidecar 분리
- 스캔·version resolution·RAM gate·ranking·benchmark verdict는 deterministic script가 소유
- 4단계 HITL, smoke-before-commit, dry-run→apply, unattended push/download/sudo 금지
- provider-specific CLI syntax가 adapter에 격리됨

### B-PROVIDER-PLURALITY

Provider-neutral schema/orchestrator는 존재하지만 실제 provider는 `claude_code` 하나뿐이다. `KNOWN_PROVIDERS=("claude_code",)`이며 검증된 generic-command fallback 또는 두 번째 실제 adapter가 없다. 이는 adapter isolation이지 실제 provider independence는 아니다.

### B-ATTACHMENT-USAGE-CLOSURE

Markdown reference orphan 검사는 있으나 5개 skill tree의 scripts/templates/fixtures/assets 전체에 대해 `used_by`, executable coverage, 생성→소비 mapping, last-use를 닫는 inventory가 없다. Digest/tracked membership은 존재·identity 증거이지 사용성 증거가 아니다.

## Lane C — 32/35 PASS

| 항목 | 점수 |
|---|---:|
| Positive/negative scenario coverage | 10/10 |
| Host safety·scoped cleanup·service preservation | 6/7 |
| 성능·기능·identity evidence | 6/7 |
| Lifecycle/evidence/promotion fail-closed | 6/6 |
| 재현성·last-good restore·audit integrity | 4/5 |

완료된 운영 계약:

- Solar positive baseline 및 final last-good restore
- DeepSeek attempt 1 REFUTE, attempt 2 n=3 REFUTE, stabilized n=8 PASS 보존
- Laguna main DFlash/sub baseline A/B 및 attempt-1 REFUTE 보존
- Hy3 runtime patch/MTP/parser/default-no-think negative 및 memwatch 원복
- Qwen3.5-397B capacity expected-negative: 양 노드 exit 7, model load 0, promotion 차단
- Solar restore initial exit 2를 active JIT로 분류하고 기존 attempt를 보존한 뒤 READY·health·model identity·low/high/tool을 검증

비차단 감점:

- DeepSeek cleanup과 일부 remote raw evidence가 final manifest에서 상대적으로 약함
- Hy3 benchmark가 concurrency 1, 4 requests로 다른 시나리오보다 얕음
- 일부 핵심 raw/manifest가 `/tmp` 또는 ignored docs에 있어 장기 보존성이 낮음
- governing plan 상태가 완료 상태로 갱신되지 않음

## 검증 상태

- Ambient working harness: `Ran 924 tests`, `OK`
- Exact clean/gitled export canonical harness: `Ran 920 tests`, `skipped=2`, `OK`
- Focused constitution/policy suites: 193 tests, OK
- Focused skill/context/provider contract suites: 150 tests, OK
- Policy audit violations: 0
- Reviewer files modified: `[]`
- 기존 unrelated scanner patch와 `.p7*`는 보존
- Origin push/tag/force-push: 없음

## 최종 판정

1. **모든 계획된 운영 E2E는 완료됐다.**
2. **현재 프로젝트 성숙도 점수는 89/100이다.**
3. **그러나 구조적 완결성 PASS는 아니다.** 사전 선언한 blocker gate 때문에 `FAIL-CLOSED`다.
4. 89점은 높은 운영·정책 성숙도를 의미하지만, `A-MUT-01`, gitless reverse closure, provider plurality, attachment usage closure를 해결하기 전에는 100점 또는 완전 provider-neutral/self-auditing 시스템으로 선언해서는 안 된다.
