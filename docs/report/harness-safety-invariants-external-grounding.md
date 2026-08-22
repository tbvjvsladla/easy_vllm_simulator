# 하네스 엔지니어링의 안전 불변식 — 외부 참조 그라운딩

> 문서형: report (docs/report 무일자 예외 — kebab slug)
> 작성: 2026-08-22 KST · grounded-citations ledger 기반 ([n] 인라인 + Sources 블록)
> 목적: easy_vllm_simulator 하네스가 CLAUDE.md "불변식"·"정책 경계"로 삼는 **안전 불변식** 개념을, 소프트웨어·안전 공학의 권위 출처로 그라운딩한다.

## 서론

easy_vllm_simulator 하네스는 헌법(CLAUDE.md)의 "불변식"과 13개 "정책 경계"를 안전의 기초 레이어로 둔다. "불변식"이라는 말은 자의적 은유가 아니라, 정형 검증과 안전 공학에서 수십 년에 걸쳐 정립된 개념이다. 이 문서는 그 외부 정본을 추적해, 하네스의 안전 불변식이 무엇을 계승하고 있는지를 명확히 한다.

## 1. 불변식(invariant)의 정형적 기원

"불변식"은 프로그램 정확성 논증의 핵심 개념으로, 1969년 Tony Hoare가 제안한 Hoare 논리에서 비롯한다. Hoare 논리는 "컴퓨터 프로그램의 정확성을 엄밀하게 추론하기 위한 논리 규칙의 집합을 가진 형식 체계"이다.[1] 그 중심은 Hoare triple `{P} C {Q}` — 사전조건 P가 성립하는 상태에서 명령 C를 실행하면 사후조건 Q가 성립한다는 삼중 구조이며, 루프 불변식(loop invariant)은 이 틀에서 반복문의 정확성을 귀납적으로 보증하는 장치다.

이 개념은 Design by Contract(DbC)로 실용화되었다. DbC는 "소프트웨어 설계자가 컴포넌트의 형식적·정밀하고·검증 가능한 인터페이스 명세를 정의해야 하며, 이는 추상 데이터 타입의 일반적 정의를 사전조건·사후조건·불변식으로 확장한다"고 규정한다.[2] Bertrand Meyer는 이를 "정확하고 견고한 소프트웨어 — 다시 말해 올바르면서도 강건한 소프트웨어 — 를 어떻게 구축할 것인가"라는 질문에서 출발시켰다.[8] Eiffel 언어는 이 계약(불변식·사전조건·사후조건)을 실행 시 단언(assertion)으로 검사한다.[9]

**워크스페이스 매핑**: CLAUDE.md의 "불변식" 절(레이어드 적응·레이어 커플링·이미지 네이밍·산출물 통로 등)은 이 전통을 따른다. 특히 "헌법(철학)이 기초레이어=단일 진실원천이며, 기초레이어가 바뀌면 상위레이어가 적응하고 역방향은 금지"라는 문장은, 불변식이 상위 구성요소의 변경보다 우선하며 위반 시 시스템이 성립하지 않는다는 Hoare/DbC의 전제-우선 정신과 동형이다.

## 2. 안전 기본값: fail-closed vs fail-open

"fail-open은 회계처리되지 않은 상황을 진행시키고, fail-closed는 그것을 차단한다."[3] 안전이 우선인 시스템에서 기본값은 fail-closed다 — 판단 불가능하거나 기대하지 않은 상태에서는 진행하지 않고 차단하는 것이, "그냥 넘어가서" 나중에 더 큰 사고로 번지는 것보다 안전하다.

**워크스페이스 매핑**: `policy:A2A_DELEGATION_KEY_FAIL_CLOSED`(위임 키 불성립 시 위임 차단), `policy:TERRAFORM_FLAG_GATE`(테라포밍 미완수 시 진행 차단), 그리고 버전 델타 Judge의 "UNDETERMINED는 IMPACT와 동일 취급하되 기록은 분리" 설계가 모두 fail-closed다. 닫힌 열거(closed enum) 밖 경로를 `UNKNOWN_PLANE`으로 차단하는 것도 같은 원리다.

## 3. 계층 방어: defense in depth

Defense in depth는 "정보 보안에서 여러 계층의 보안 통제(방어)를 IT 시스템 전반에 배치하는 개념으로, 그 의도는 어떤 통제가 실패하거나 취약점이 악용될 때 중복성을 제공하는 것"이다.[4]

**워크스페이스 매핑**: `policy:HOST_SAFETY_LAYERED_DEFENSE`(호스트 하드다운 방어)가 이에 해당한다. 예산 선언(arm_ceiling)·메모리 워치독·earlyoom·gmu 하드클램프가 각각 독립적인 방어층을 이루어, 단일층 실패가 즉시 호스트 하드다운으로 이어지지 않게 한다.

## 4. 안전 불변식의 검증 (자율시스템 문헌)

자율시스템(로보틱스) 안전 문헌은 "안전 요구사항은 전형적으로 불변식 — 시스템이 안전하다고 간주되기 위해 항상 참이어야 하는 계산 가능한 속성 — 으로 정의된다"고 말한다.[7] 같은 논문은 "단위 테스트 통과와 소프트웨어 크래시의 부재만으로는 충분한 안전 사례(safety case)를 구성하지 못한다"고 지적한다.[7]

**워크스페이스 매핑**: CLAUDE.md의 "성능은 기능 스모크와 별개로 검증한다 — 적대적 벤치마크 게이트 없이 성능 '완료'를 선언하지 않는다"는 불변식과 정확히 같은 논리다. 스모크(기능 단위 테스트) 통과만으로는 안전/성능을 선언하지 않고, 별도의 게이트를 두는 것이다.

## 5. 헌법: Constitutional AI

Anthropic의 Constitutional AI는 "유일한 인간 감독이 규칙·원칙의 목록으로 제공되며, 그래서 이 방법을 'Constitutional AI'라 부른다"고 정의한다.[5] AI 시스템을 감독하는 인간의 개입이 "원칙 목록(헌법)"으로 축약·형식화된다는 이 설계는, 이 워크스페이스가 헌법(CLAUDE.md)을 단일 진실원천으로 삼는 구조와 직접 대응한다.

**워크스페이스 매핑**: CLAUDE.md의 "헌법(철학)이 기초레이어=단일 진실원천"이라는 표현은 Constitutional AI의 "원칙 목록이 감독의 정본"이라는 설계와 같은 구조다. 두 경우 모두, 구체적 사례마다 인간이 개입하는 대신 "왜"(원칙)를 헌법으로 고정하고 "어떻게"(절차)는 그 아래 레이어가 소유한다.

## 6. content-addressed 재현성

Nix 스토어는 "스토어 경로 기본 이름이 항상 정확히 하나의 스토어 객체를 참조"하도록, 콘텐츠 해시(20바이트 digest)로 객체를 식별한다.[6] 콘텐츠 주소 지정(content-addressing)은 "동일 빌드"의 정의를 "동일 입력 집합"으로 바꾼다 — 버전 라벨이 아니라 콘텐츠 해시가 동일성의 판정 기준이 된다.

**워크스페이스 매핑**: "결정론 산출물은 출처를 표시한다"는 규율과, 버전 델타 Judge의 3-sha 프로브(번들/상류 from/to)가 같은 정신이다. 라벨(버전 문자열)이 아니라 콘텐츠 해시로 동일성을 판정하며, 이것이 0.27.0 라벨과 0.27.1.dev0 실체의 불일치를 결정론적으로 검출하는 근거가 된다.

## 7. 종합 매핑

| 외부 원칙 | 출처 | 워크스페이스 대응 |
|---|---|---|
| Hoare triple · 루프 불변식 | [1] | CLAUDE.md 불변식 절(기초레이어 우선) |
| Design by Contract(사전/사후/불변식) | [2][8][9] | 불변식 + 실행 시 게이트 검사 |
| fail-closed 기본값 | [3] | policy:*_FAIL_CLOSED 계열 · UNKNOWN_PLANE 차단 |
| defense in depth | [4] | policy:HOST_SAFETY_LAYERED_DEFENSE |
| 안전 불변식 검증 | [7] | "스모크 ≠ 성능/안전 선언" 불변식 |
| Constitutional AI | [5] | 헌법=왜(기초레이어=단일 진실원천) |
| content-addressing | [6] | 결정론 출처 표시 · Judge 3-sha 프로브 |

## 결론

하네스의 "안전 불변식"은 ① 정형 검증의 불변식(Hoare/DbC), ② 안전 기본값(fail-closed), ③ 계층 방어(defense in depth), ④ 원칙 목록에 의한 감독(Constitutional AI), ⑤ 콘텐츠 주소 지정(content-addressing)이라는 다섯 갈래의 정립된 공학 전통을 계승한다.[unverified] 이 매핑(각 절의 "워크스페이스 매핑")은 외부 출처가 아니라 easy_vllm_simulator의 헌법(CLAUDE.md) 불변식·정책을 본 저자의 해석으로 대응시킨 것으로, 헌법 불변식이 "왜"를 고정하고 스킬·정책이 "어떻게"를 소유한다는 경계기준이 자의적이 아니라 외부에서 독립적으로 정립된 안전 공학 원칙들에 그라운딩되어 있음을 보여준다.[unverified]

## Sources

[1] https://en.wikipedia.org/wiki/Hoare_logic — Hoare logic (Wikipedia)
[2] https://en.wikipedia.org/wiki/Design_by_contract — Design by contract (Wikipedia)
[3] https://authzed.com/blog/fail-open — AuthZed — Fail Open vs Fail Closed
[4] https://en.wikipedia.org/wiki/Defense_in_depth_(computing) — Defense in depth (computing) (Wikipedia)
[5] https://arxiv.org/abs/2212.08073 — Constitutional AI: Harmlessness from AI Feedback (arXiv)
[6] https://nix.dev/manual/nix/latest/store/store-path — Nix Reference Manual — Store Path
[7] https://squareslab.github.io/materials/zizyte21dsn.pdf — The Importance of Safety Invariants in Robustness Testing Autonomy Systems (DSN 2021)
[8] https://se.inf.ethz.ch/~meyer/publications/old/dbc_chapter.pdf — Design by Contract (Bertrand Meyer)
[9] https://www.eiffel.org/doc/solutions/Design_by_Contract_and_Assertions — Eiffel — Design by Contract and Assertions
