# 증거그래프(@ttsc/evidence) — 원리와 구현

> 문서형: report (docs/report 무일자 예외 — kebab slug)
> 대상 코드베이스: `/home/cona/ws_docker/easy_vllm_simulator/temp/ttsc` (samchon/ttsc, MIT)
> 범위: `@ttsc/evidence` 패키지의 **증거그래프(evidence graph)** 원리·구현만 보수적으로 추출. 동일 레포의 `@ttsc/graph`(코드그래프 MCP 서버)는 별개 기능이므로 제외.

## 1. Product Contract — 증거그래프가 무엇인가

증거그래프의 존재 이유는 두 문장으로 요약된다(`.agents/skills/project/evidence/SKILL.md` §Product Contract):

> "An artifact that cites nothing has no proof it was needed. An artifact that cites a target no configured source declares has proof of nothing."

즉, **① 아무것도 인용하지 않는 산출물은 "필요했다"는 증거가 없고, ② 구성된 소스가 선언하지 않는 대상을 인용하는 산출물은 "무엇"도 증명하지 못한다.** 증거그래프는 이 두 상태를 모두 **컴파일 에러**로 바꾼다.

핵심 분업은 `packages/evidence/README.md`의 한 줄에 요약된다:

> **"The compiler handles omissions. Humans handle falsehoods."**

누락(빠뜨린 인용)은 기계가 결정론적으로 잡아내고, 거짓(거짓 인용)은 사람이 리뷰한다. 이 분업이 전체 설계를 관통한다.

## 2. Claim / Reference 모델

그래프는 **claim**(빚을 지는 쪽)과 **reference**(빚의 원천) 두 모집단으로 구성된다.

- **Claim** — 인용을 달아야 하는 파일/선언 호스트 집합. "이 코드는 무엇을 근거로 존재하는가"를 답해야 한다.
- **Reference** — claim이 인용해야 할 증거 모집단. "이 증거는 누구에게 인용되는가"를 정의한다.
- **모든 claim–reference 쌍은 독립적으로 완결된 의무(obligation)**이며, reference 배열의 각 요소는 서로 분리되어 있다.

설정은 `lint.config.ts`의 `ITtscEvidenceGraphConfig`로 선언한다(`packages/evidence/README.md` §Configure). 예: `src/components/**/*.tsx`(claim, function)가 `docs/**/*.md`(reference, h2·h3)를 인용해야 한다고 선언하면, 문서의 모든 H2·H3가 컴포넌트에 인용되지 않을 때 빌드가 멈춘다.

## 3. Tag 문법

`SKILL.md` §Tag Grammar에 정의된 네 태그:

```text
@evidence <target> <reason>
@evidenceExclude <target> <reason>
@evidenceReview <target> #<fingerprint> <description>
@evidenceExcludeReview <target> #<fingerprint> <description>
```

핵심 규칙:

- target은 **공백으로 구분된 단일 토큰**. 단 `{@link …}` 인라인 링크만 중괄호까지 하나의 토큰으로 확장된다.
- **리뷰 태그가 두 개인 이유**: 검증해야 할 acknowledgement가 두 종류이고(`@evidence`의 검증 ≠ `@evidenceExclude`의 검증), 하나의 태그로 합치면 더 쉬운 검증이 더 어려운 검증을 방전(discharge)시켜 어느 질문이 답했는지 알 수 없게 된다.
- **리뷰는 인용의 주석이지, unit의 acknowledgement가 아니다.** 리뷰가 acknowledgement로 오인되면 coverage를 방전하고 `uniqueEvidence`·`singleEvidencePerSymbol`을 오염시킨다. 그래서 리뷰는 세 번째 tagKind가 아니라 별도 타입이다(`native/model.go`의 `evidenceReview`가 `evidenceDeclaration`의 형제 타입인 이유).
- `#fingerprint`는 선택적이나 `requireReview` 참조에서는 필수. `#`는 "브레이스 코드 타깃의 중괄호"와 같은 이유로 load-bearing — 토큰이 스스로를 구분한다.

## 4. 네 종류의 Unit과 계층

증거 단위(unit)를 만드는 산출물 종류는 넷(`SKILL.md` §Units And Hierarchy):

| 종류 | 주소 형식 | 비고 |
|---|---|---|
| **Markdown** | `<path>` 또는 `<path>#<anchor>` (H1–H4) | 파일이 heading 개요를 포함 |
| **Prisma** | `prisma:<Model>` / `prisma:<Model>.<member>` | 모델이 컬럼·관계를 포함 |
| **Swagger** | `<METHOD>:<path>` (예 `POST:/members`) | reference-only, operation이 독립 리프 |
| **TypeScript** | 정규화된 공개 이름 (inline link) | type/function/property |

구조적 포함 관계가 계층을 이룬다. `@evidence`는 선택된 target과 그 **모든 선택된 자손**을 acknowledgement하고, `@evidenceExclude`도 마찬가지다(참조가 `noEvidenceExclude`를 선언한 경우 제외).

**"계층은 identity이지 spelling이 아니다"** — materialization 시 명시적 부모 unit ID를 저장하며, TypeScript 조상을 점(dot) 접두사 문자열에서 추론하지 않는다(리터럴 이름이 점을 포함할 수 있으므로).

`@internal`/`@hidden`/`@ignore` 태그를 단 선언은 unit을 만들지 않으며, 중첩된 것도 마찬가지다(철회된 선언은 인용·호스트·제외 운반자 모두 자격이 없다).

## 5. 평가(Evaluation)의 세 질문

`evidence/graph`는 Program당 한 번 전체 그래프를 평가하고 세 질문을 답한다(`SKILL.md` §Evaluation):

1. **Resolution** — 모든 declaration의 target이 정확히 하나의 선택된 unit 또는 구조적 조상으로 해석되는가?
2. **Host eligibility** — `@evidence`가 claim이 선택한 symbol 종류의 호스트에 있는가?
3. **Coverage** — 모든 선택된 reference unit이 이 claim에서 최소 하나의 acknowledgement를 갖고, 그 acknowledgement가 참조 정책이 요구하는 것을 충족하는가?

claim 상태와 reference 상태는 분리되어, 한 claim/reference를 만족하는 declaration이 물리적으로 같은 target이라도 다른 쪽의 coverage로 새지 않는다.

## 6. 참조 정책 (Reference Policies)

참조는 자신의 acknowledgement 관계를 강화하는 네 옵션 + checklist를 가진다(`SKILL.md` §Reference Policies):

- **`noEvidenceExclude`** — 이 모집단에 대한 `@evidenceExclude`를 거부, target이 양성 증거를 빚지게 한다.
- **`uniqueEvidence`** — 선택된 unit당 최대 하나의 양성 의미론적 claim 호스트.
- **`singleEvidencePerSymbol`** — 모든 선택된 claim 호스트(태그 없는 호스트 포함)가 정확히 하나의 선택된 unit을 인용.
- **`requireReview`** — 모든 acknowledgement가 현재 콘텐츠의 fingerprint를 지닌 리뷰를 갖게 해, 리뷰가 "인용된 것"이 바뀌면 만료되게 한다.
- **`checklist`** — 의무를 참조에서 각 호스트로 이동(모든 호스트가 모든 unit을 답함). 네 cardinality 옵션과 동급이 아니라 다른 차원이며, `uniqueEvidence`·`singleEvidencePerSymbol`과 병용 시 decode에서 거부된다.

모든 옵션은 opt-in이며, false 값은 역사적 동작을 보존한다.

## 7. 구현 — Go 네이티브, 5개 규칙

### 7.1 아키텍처

증거그래프는 **Go 네이티브 구현**이다(`packages/evidence/native/`, 비테스트 28개 파일 약 43K 라인). `@ttsc/evidence`는 `@ttsc/lint`의 규칙 기여자(rule contributor)로서, stock `tsc`가 아니라 `ttsc`(TypeScript-Go 컴파일러) 위에서 돈다(`package.json` description: "Evidence-graph lint contributor for @ttsc/lint").

규칙은 다섯(`native/model.go`의 상수):

| 규칙 | 진입점 | 성격 |
|---|---|---|
| `evidence/graph` | `graphRule.Check(ctx *rule.ProjectContext)` | **프로젝트 레벨** — Program당 1회 전체 그래프 평가. `NeedsTypeChecker() false` |
| `evidence/singular` | `singularRule.Check(ctx, node)` | 노드 레벨 |
| `evidence/documented` | `documentedRule.Check(ctx, node)` | 노드 레벨 |
| `evidence/todo` | `todoRule.Check(ctx, node)` | 노드 레벨 |
| `evidence/review` | `reviewRule.Check(ctx, node)` | 노드 레벨 |

즉, 그래프 평가는 **프로젝트 레벨 규칙 하나**가 담당하고, 나머지 넷은 AST 순회 중 노드별로 도는 보조 규칙이다.

`graphRule`은 `Check` 외에도 세 표면을 가진다:
- **`ProjectInputs`** (`inputs.go`) — Markdown/Prisma/Swagger처럼 TypeScript Program에 들어오지 않는 외부 파일 의존성을 호스트에 선언(§7.4).
- **`GraphNodes`** (`graph_nodes.go`) — 편집기/소비자에 그래프 노드 공개.
- **`Hints`** (`hints.go`) — 자동완성 힌트.

### 7.2 데이터 모델 (`model.go`)

핵심 구조(`native/model.go`):

- `graphConfig` → `claimSpec` (Index/Type/Name/Root/Files/ExclusionCarriers/Symbols/References) → `referenceSpec` (Type/Policy/Files/Source/Package/Symbols)
- `referencePolicy` — `NoExclude`/`UniqueEvidence`/`SingleEvidencePerSymbol`/`RequireReview`/`Checklist` bool 필드(제로값이 역사적 동작을 보존)
- `evidenceUnit` — ID/ParentID/Target/Identity(segments)/Aliases/Type/Symbol/Path/Line/**Digest**/Hidden/ValueSpace/TypeSpace
- `evidenceDeclaration` — HostID/**SemanticHostIDs**(선택된 그래프 identity)/Type/Tag/Target/Reason/Hosts
- `evidenceReview` — **`evidenceDeclaration`의 형제 타입**(acknowledgement를 세는 어떤 소비자도 리뷰에 도달할 수 없게 분리). Target은 재해석하지 않고 declaration의 target과 비교된다.
- `artifactInventory` — UnitNodes(unit→노드 연관)와 UnitContent(그 identity의 **콘텐츠**인 노드, UnitNodes의 부분집합)를 분리.

**identity와 콘텐츠의 분리**가 반복되는 주제다: 변수 선언에서 TypeScript는 선행 문서를 statement wrapper에 붙이므로, wrapper는 identity가 소유한 *위치*이지만 그 안의 형제 선언 텍스트는 이 identity의 *콘텐츠*가 아니다. 둘을 혼동하면 한 declarator의 편집이 다른 declarator의 fingerprint를 바꾼다.

### 7.3 평가 흐름 (`graph.go`, 1907줄)

`graphRule.Check`(`graph.go:18`)는 다음 순서로 돈다:

1. `materializeClaimStates` (`:234`) — 각 claim의 파일/호스트/declaration/reference를 materialize.
2. `evaluateEvidenceGraph` (`:492`) — resolution·host eligibility·coverage 세 질문을 평가.
3. `reviewProblems` (`:1232`) — 리뷰 원장(ledger)으로 인용↔리뷰 짝 검증.
4. `reportProblems` (`:1897`) — 진단 보고.

핵심 해석 함수들: `resolveInlineLinkDeclaration`(`:1679`)은 TypeScript 인라인 링크 target을 **인용 모듈의 import 스코프**로 해석하고, `materializeEntryReference`(`:1422`)는 package 참조의 entry traversal을 수행한다.

### 7.4 핵심 구현 디테일

**① Fingerprint/Digest** (`fingerprint.go`) — `requireReview`의 근간. unit의 콘텐츠를 **정규화 후, 태그가 위치할 수 있는 모든 위치를 제거하고** 해시한다. 그렇지 않으면 "리뷰를 쓰는 행위 자체가 그 리뷰가 검증하는 fingerprint를 바꿔" 복구가 종료되지 않는다. Markdown은 HTML 주석이, TypeScript는 문서 블록이 빠진다. 줄바꿈 정규화·후행 공백 제거도 한다. unit별 digest의 커버 범위는 산출물별로 다르며(TypeScript unit은 중첩 멤버 텍스트까지 포함), "digest가 하위트리와 독립적"이라는 가정은 하지 않는다(`model.go` `evidenceUnit.Digest` 주석).

**② Import 스코프 해석** (`imports.go`) — 인라인 링크 target은 전역 심볼 테이블이 아니라 **인용 모듈의 import 바인딩**으로 해석된다. 이로써 인용은 "심볼 이름을 철자한 문자열"이 아니라 실제 참조가 된다. 동시에 **"TypeScript claim만 TypeScript 증거를 인용할 수 있다"**는 제약이 성립한다 — 다른 산출물은 import 스코프가 없으므로, 이를 허용하면 심볼 이름의 전역 유일성이 load-bearing이 된다(§Tag Grammar).

**③ ProjectInputs 계약** (`inputs.go`) — Markdown·Prisma·Swagger 증거는 TypeScript Program에 들어오지 않으므로, `@ttsc/lint@0.22.0`의 `ProjectInputs` 계약이 생기기 전까지 호스트는 그래프가 이 파일들에 의존하는지 알 수 없었다. 그 비대칭은 "코드 편집자는 신선한 진단을 보는데, 문서만 편집한 개발자는 방금 stale해진 인용이 계속 초록불로 보고되는" invisible 결함이었다. 이제 규칙이 외부 의존을 선언해 호스트가 watch하게 한다. URL 형식 Swagger는 파일시스템 이벤트가 없으므로 선언하지 않고, 로더가 평가마다 fresh를 책임진다.

## 8. 결론

증거그래프는 "누락은 기계가, 거짓은 인간이"라는 분업 위에, **claim–reference 의무 그래프**를 컴파일 타임에 평가하는 결정론 장치다. 원리 측면에서 네 산출물 종류를 하나의 주소·계층 모델로 통합하고, 참조 정책으로 의무의 강도를 opt-in으로 조절한다. 구현 측면에서 Go 네이티브 규칙 다섯 개(프로젝트 레벨 `evidence/graph` + 노드 레벨 넷)가 TypeScript 컴파일러의 Program 위에서 materialize→resolve→coverage 순으로 평가하며, fingerprint·import 스코프·ProjectInputs 세 장치가 각각 "리뷰의 자기무효화 방지", "실제 참조 해석", "외부 의존성 관찰"이라는 세 실패 모드를 닫는다.
