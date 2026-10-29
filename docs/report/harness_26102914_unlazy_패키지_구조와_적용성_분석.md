# Unlazy 패키지의 완료 검증 구조와 적용성 분석

분석일: 2026-10-29 KST. 대상은 `seed/unlazy`의 로컬 체크아웃이며 `package.json` 버전은 `2.1.0`, Git HEAD는 `16671491f6679ad9378f52604d3bc2415b4120c7`이다. 분석 시작 시 패키지 작업 트리는 깨끗했다. README는 이 소스를 태그가 확정된 GitHub 릴리스로 소개하지 않는다. 아래 코드 근거는 이 커밋의 파일과 함수 이름을 기준으로 한다.

**Unlazy는 수용 조건을 원장에 적고 검사 명령과 연결해, 조용한 누락과 성급한 완료 선언을 드러내는 도구다.** 게이트 검사·상태 기록·협력 프로세스 간 조율은 구현되어 있지만, 요구사항을 자동 추출하거나 검사 명령의 의미를 보증하는 시스템은 아니다. 이 저장소에는 전면 설치보다 요구사항과 증거의 대응, 명시적 비완료, 변경된 증거의 재검증 원칙을 기존 캠페인·completion gate에 대조하는 방식이 적합하다.

## 1. 구성과 책임

| 구성 | 실제 역할 | 핵심 근거 |
|---|---|---|
| SKILL.md와 references | solo·orchestrated·parallel 방식 선택, 계약·검증·보고 규약 | `SKILL.md`, `references/method.md`, `references/orchestration.md` |
| 게이트 원장 | 결과별 ID, CHECK·EXPECT·CWD·EVIDENCE, OWNS·ABANDON 정의 | `templates/gates-leaf.md`, `scripts/lib/gates.mjs:parseGates` |
| gate-check.mjs | 승인된 명령 실행, 통과 판정, 증거 갱신, scope·lease 관리 | `runCheck`, `evidenceFor`, 최종 집계 |
| gate-lint.mjs | 허술한 검사·기대값의 어휘적 신호 탐지 | 일반 모드는 경고, `--strict`는 경고를 실패로 처리 |
| dispatch-check와 lib/dispatch | 시작 핸들을 모으고 wave 전이를 기록 | `updateDispatch`, `validateState`, `dispatchStatus` |
| stop-hook.mjs | Claude Code 종료 시 미완료 원장·wave를 확인 | `MAX_BLOCKS`, progressHash, block 응답 |
| install-hooks.mjs | Claude Code Stop 설정 설치·제거 | `nextSettings`, `persist`, 대상 옵션 |
| tests와 research | 구현 회귀 및 별도의 모델 효과 평가 설계 | `package.json`, `research/validation-protocol.md` |

런타임 코드는 Node 표준 라이브러리를 사용하며 package.json에는 외부 런타임 의존성이 없다. 지원 하한 선언은 Node 16이다. 이것은 설치·공급망 부담을 줄이는 장점이지만, 모든 Node·OS 조합을 이번 분석에서 검증했다는 뜻은 아니다.

## 2. 게이트가 완료로 판정되는 과정

작업자는 구현 전에 수용 조건을 원장에 적는다. 실행 가능한 항목에는 CHECK와 EXPECT가 모두 필요하고, 자동으로 결정할 수 없는 항목은 수동 근거를 남긴다. parser는 중복 ID, 빈 원장, 불완전한 실행 항목, 잘못된 abandonment 등을 오류로 처리한다. 코드 블록 속 예시는 실제 게이트로 세지 않는다. [U1][U1]

실행 모드에서 명령은 셸을 통해 실행된다. **종료 코드가 0이고 stdout·stderr를 결합한 문자열이 EXPECT를 만족해야** 성공이다. 타임아웃·명령 시작 실패·출력 한도 초과·정규식 판정 오류는 성공으로 바뀌지 않는다. 기본 실행 제한은 120초, 출력 한도는 1 MiB이며 정규식은 제한된 worker에서 판정한다. [U2][U2]

성공한 게이트에는 정의 digest와 출력 fingerprint, 종료·기대값 일치 상태, 제한된 실행환경 정보가 기록된다. 정의 digest는 CHECK·EXPECT·원장의 원래 CWD 값에 결속된다. 체크박스만 채우거나 임의의 성공 문구를 적어서는 runnable gate가 met가 되지 않는다. 정의가 바뀐 증거는 stale-unmet로 처리한다. [U1 `gateDefinitionDigest`·`classifyGateEvidence`·`gateState`][U1]

```text
수용 조건 작성 → 검사 명령 검토·승인 → 실행과 EXPECT 판정 → 정의에 결속된 증거
                                                          ↓
반환된 작업 재검증 → 하위 결과 통합 검사 → 현재 요청과 최종 주장 대조
```

여기서 `--status`는 명령을 실행하지 않는 구조 확인이다. `--reverify`는 이미 체크된 실행 항목도 다시 수행한다. 전자는 원장 정의와 기록의 일치, 후자는 현재 산출물에 대한 명령의 성공을 확인한다. 두 결과를 같은 증거로 취급하지 않는 점이 이 패키지의 중요한 장점이다.

## 3. 보장하는 것과 보장하지 않는 것

### 요구사항 누락은 원장 밖에서도 생긴다

원장에 적은 조건이 모두 통과해도 사용자의 요구 중 일부가 처음부터 빠졌다면 전체 과업 완료가 아니다. PLAN 템플릿은 요구사항마다 안정된 ID·owner·검증 항목·revision을 연결하고 현재 요청을 다시 읽도록 요구한다. 그러나 프로덕션 checker가 자연어 요청에서 요구사항을 추출하거나 PLAN 전체를 의미적으로 검증하지는 않는다. [U3][U3]

`tests/contract-tests.mjs`는 이 차이를 분명히 보여 준다. 파일 내부의 `reviewContract`와 사람이 제공한 required ID 집합으로 누락·오래된 revision·근거 없는 제거를 구별하는 fixture를 검증한다. 이 테스트가 통과했다는 사실을 실제 PLAN을 자동 집행하는 운영 모듈이 존재한다는 뜻으로 확장하면 안 된다.

### 검사 명령의 의미는 별도 검토가 필요하다

‘기능이 정상이다’라는 제목과 고정 성공 문구를 출력하는 CHECK도 실행 자체는 성공할 수 있다. 같은 무의미한 명령을 부모가 재실행해도 의미상 독립 검증이 되지는 않는다. gate-lint는 이런 패턴의 일부를 어휘적으로 경고하지만 셸 전체를 해석하거나 요구사항과 검사 사이의 동치성을 증명하지 않는다.

따라서 게이트는 해당 산출물·서비스·측정값을 직접 읽고, 단언이 모두 통과한 뒤에만 성공 표식을 출력해야 한다. 부재 검사는 알려진 양성 fixture를 넣었을 때 실패하는지도 확인해야 한다. 수동 항목은 사람이 기록한 근거를 받아들이므로 중요한 결과의 의미 검수 책임이 남는다. [U4][U4]

### 증거 digest는 서명이나 산출물 최신성 증명이 아니다

자동 증거의 SHA-256은 정의 변경을 탐지한다. 원장을 편집할 수 있는 주체는 형식에 맞는 증거도 만들 수 있으므로 결과의 진위 인증을 보장하지 않는다. 또한 같은 명령 문자열이 호출하는 스크립트·fixture·의존성·산출물 바이트까지 자동 추적하지 않는다. 입력이 달라졌다면 소스 재검토와 실제 `--reverify`가 필요하다. [U1][U1] [U5][U5]

이 제한은 결함을 숨긴 것이 아니라 README와 SECURITY가 명시한 설계 범위다. 별도 의존성 digest를 CHECK에 넣는 방법도 문서화되어 있으나, 그것은 사용자가 추가한 검증이며 패키지가 자동 제공하는 전이 의존성 추적이 아니다.

## 4. 승인과 실행 권한

CHECK는 검사기의 사용자 권한·환경·자격증명·네트워크를 상속하는 셸 코드다. Unlazy의 approval은 실행 동의 기록이며 sandbox가 아니다. `--status`로 구조를 확인한 뒤 CHECK와 호출 스크립트를 검토하고 승인하는 흐름을 전제로 한다. 기존 정확한 승인 기록이 있으면 일반 실행도 명령을 실행하므로, 일반 모드를 항상 dry-run이라고 생각해서는 안 된다. [U5][U5]

승인 기록은 원장 경로·게이트·명령·기대값·해소된 CWD·셸·제한값·platform·전체 PATH 등 런타임 조건에 결속된다. 보관소는 기본적으로 사용자 영역의 owner-private 디렉터리이며 canonical repository 밖에 있어야 한다. 연결 파일·교체된 파일·부적절한 권한을 거부하는 처리가 있다. 승인 결속은 기록의 재사용 범위를 좁히지만, 명령의 의미나 전이 의존성 바이트를 검증하지는 않는다. [U2 `oracle`·`approvalPath`·승인 저장소 검사][U2]

이번 Codex 공통 권한 정책과 연결하면, unrestricted 실행에서도 승인된 업무 범위와 게이트 명령 검토는 따로 남는다. Unlazy를 추가한다고 OS 파일 제한이 복원되는 것은 아니며, native `approval_policy = "never"`가 Unlazy 내부 승인 기록을 자동 대체하는 것도 아니다. 반대로 이 패키지를 분석했다는 사실이 새 승인 체계나 설치를 채택했다는 뜻도 아니다.

## 5. 분업과 병렬 실행의 구현 범위

### Scope와 lease

scope는 원장 발견·상태 로그·세션 연결·조율 기록을 구분한다. OWNS의 상대경로 집합으로 lease를 취득하면 겹칠 가능성이 있는 선언을 보수적으로 거부한다. claim 판정과 기록을 하나의 lock 안에서 처리하고 동일 scope/leaf의 중복 claim도 막는다. [U1 `claimLeases`][U1]

이것은 규약을 지키는 프로세스의 충돌 예방이다. 프로세스가 선언 밖에 쓰는 것을 OS가 차단하지 않으며, 서로 다른 worktree도 외부 캐시·서비스를 공유할 수 있다. scope나 lease를 노드 인증·권한 격리로 사용해서는 안 된다.

### Dispatch wave

wave는 open → sealed → complete로 전이한다. 모든 leaf에 서로 다른 start handle이 기록되기 전에는 seal할 수 없고, seal 전에는 return할 수 없다. 복구 불가능한 부분 시작은 기록을 삭제하는 대신 abandoned로 남긴다. [U6 `updateDispatch`][U6]

Unlazy는 native 에이전트를 직접 생성하지 않는다. 호출자가 호스트에서 받은 handle을 기록하며, 핸들의 실제 진위를 외부 호스트에 조회해 인증하는 코드도 아니다. seal은 모든 시작 기록이 반환 기록보다 먼저 수락됐음을 뜻하며 실제 CPU 동시 실행·worker 정직성·게이트 성공을 입증하지 않는다.

또한 dispatch 전이 함수 자체는 PLAN의 Needs가 VERIFIED인지, PLAN Owns와 원장 OWNS가 같은지, leaf lease가 실제 취득됐는지를 모두 연결해 판정하는 스케줄러가 아니다. 이 조건들은 문서가 driver에게 부과하는 운영 계약이다. 이 구분 없이 ‘의존성과 소유가 자동으로 전부 집행된다’고 설명하면 구현 범위를 과장하게 된다. [U3][U3] [U6][U6]

## 6. Stop hook은 완료의 최종 보증이 아니다

Stop hook은 Claude Code용이다. 원장과 dispatch 상태를 읽어 미완료가 있으면 top-level block 응답을 내지만 CHECK를 실행하지 않는다. SKILL.md의 Codex 사용과 dispatch 지침이 있다는 사실이 Codex의 native 종료 차단까지 구현했다는 뜻은 아니다. [U7][U7]

진행 판정은 원장 전체 바이트가 아니라 해소된 게이트·dispatch 상태를 기준으로 한다. 주석이나 타임스탬프만 바꿔도 계속 진행 중으로 인정되는 문제를 줄인다. 그러나 동일 상태에서 여섯 차례 차단한 뒤 다음 호출에서는 종료를 허용하며, scope 모호성 등 일부 진단 상황에서도 차단하지 않는다. 따라서 **종료가 허용됨과 과업이 완료됨은 다르다.**

게이트나 wave의 abandonment도 실행을 마칠 수 있는 인계 상태지만 성공은 아니다. checker는 HANDOFF REQUIRED와 비0 종료를 반환하고, scope 집계 역시 abandoned wave를 ALL MET로 올리지 않는다. 이 구분은 장기 작업의 정직한 인계에 유용하다.

installer는 기본 프로젝트 로컬 설정, shared 설정, 사용자 global 설정을 지원한다. 기존 형식을 검증하고 관련 없는 hook을 보존하며 기존 파일의 백업을 만든 뒤 갱신한다. 설치 명령에는 Node와 패키지의 절대경로가 들어가므로 shared 설치는 이식성과 경로 노출을 검토해야 한다. 이번 분석은 실제 작업 프로젝트나 사용자 설정에 hook을 설치하지 않았다. [U8][U8]

## 7. 로컬 실행 검증과 해석

`seed/unlazy`에서 의존성 설치 없이 `npm test`를 실행했다. 환경은 Linux aarch64, Node `v26.7.0`, npm `12.2.1`이며 종료 코드는 0이었다.

| 테스트 스크립트 | 하네스가 보고한 결과 | 주된 확인 영역 |
|---|---|---|
| run-tests.mjs | 34/34 | 기본 parser·checker·scope·hook·installer |
| dispatch-tests.mjs | 27/27 | wave 전이·부분 시작·집계·상태 조율 |
| hardening-tests.mjs | 51/51 | 입력·승인·증거·파일·출력·프로세스 경계 |
| stress-tests.mjs | 24/24 | 경합·변경·상태 기록·설정 갱신 |
| lint-tests.mjs | 29/29 | 경고·strict·출력·parser 호환 |
| contract-tests.mjs | 8/8 | 별도 fixture상의 요구사항 대응 검토 |
| self-check.mjs | 15/15 | 소스·참조·PLAN 구조 검사 |

이는 테스트 하네스 기준 173개 사례와 구조 검사 15개의 통과다. **실제로 실행된 플랫폼별 단언 수와 같지는 않다.** hardening-tests.mjs의 Windows integration 사례는 비Windows에서 즉시 반환하며 하네스가 이를 ok로 센다. Windows 관련 모의 검사와 실제 Windows 실행도 구별해야 한다.

소스 CI는 Linux·Windows·macOS와 여러 Node 버전을 선언하지만, 이번 결과는 그 원격 CI 결과나 Node 16 하한 검증을 대신하지 않는다. 원시 실행 로그와 명령·환경·범위는 `docs/simlog/26102914_unlazy_패키지검증/npm-test.log` 및 `docs/testlog/testlog_26102914_unlazy_패키지분석_검증.md`에 보존했다. 이 비배포 증거는 배포본에 없을 수 있으므로 위 표에 판단에 필요한 결과와 한계를 함께 적었다.

## 8. 효과 주장과 비용 측면의 한계

README와 validation-protocol은 과거의 여섯 실행 비교에 정확한 프롬프트·하네스 버전·trace·산출물·채점표·계산 코드가 없다고 밝힌다. 따라서 과거 토큰 배율이나 결함 수를 이 패키지의 재현된 효과로 제시할 수 없다. 이번 소프트웨어 회귀 성공 역시 모델의 작업 품질이나 비용 개선을 입증하지 않는다. [U9][U9]

운영 비용도 따로 평가해야 한다. 부모가 leaf를 재검증하고 상위 branch가 다시 하위 원장을 재검증하는 구조는 신뢰를 보강하지만, 깊은 트리에서는 같은 하위 명령이 여러 번 실행될 수 있다. 문서는 전 프로젝트 검사를 leaf마다 넣지 말고 branch에 배치하라고 명시한다. 그래도 계층별 재실행 비용은 실제 트리와 검사 시간에 따라 측정해야 한다. [게이트 배치 규약][U10]

SKILL.md의 4-pass 개선과 ‘더 나아질 점이 없을 때까지’라는 종료 조건은 정성적이다. 이를 작은 변경에도 고정 적용하면 과도한 재독·수정·검증을 유발할 수 있다. 패키지도 사소한 수정과 사실 답변에는 게이트를 만들지 말라고 한다. 아스트라 작업 원칙과 함께 쓰려면 합의된 수용 기준·실제 위험·예산을 종료 조건으로 우선 해석할 필요가 있다.

## 9. 이 저장소에 대한 적용 판단

다음은 구현 사실과 구분되는 적용 제안이다. 새 스킬 설치나 코드 변경을 수행한 결과는 아니다.

| Unlazy의 요소 | 현행 저장소와의 관계 | 판단 |
|---|---|---|
| 요구사항별 owner·증거 대응 | workflow의 owner 및 캠페인 계약과 상보적 | 누락 방지 검토표로 활용할 가치가 있다. 상태 writer를 복제하지 않는다. |
| 실행 성공과 업무 완료의 구분 | 기존 completion gate·스모크·벤치 체계와 일치 | 원칙을 유지하고 ALL MET로 serving·성능 인증을 대체하지 않는다. |
| stale 증거와 실제 재실행 구분 | 승인·입력·산출물의 currentness 검토와 연결 | 이미 있는 provenance·증거 계약에서 부족한 경우만 보강한다. |
| abandonment의 명시적 비성공 | 기존 input-required·실패·중단·재개 계약과 연결 | 인계는 보존하되 정상 예산 소진을 과업 포기로 바꾸지 않는다. |
| PLAN·GATES·.unlazy 런타임 상태 | 기존 캠페인 원장·root registry와 중복·충돌 가능 | 새 루트 항목을 자동 추가하지 않는다. 필요하면 기존 거처와 단일 writer에 맞춘 설계를 먼저 한다. |
| native dispatch wave | 현재 SSH A2A·provider별 세션 계약과 다른 실행 표면 | 로컬 native agent와 원격 CLI worker를 같은 성공으로 기록하지 않는다. |
| Stop hook | Claude 전용, 무진행 차단 해제 존재 | Codex와 공통인 업무 완료 게이트로 간주하지 않는다. |
| 자동 증거 digest | 저장소의 Git 단일 권위 정책과 별도 목적 | 기록 정의 결속과 추적 소스 무결성 복제를 구분하고 관례적으로 해시를 추가하지 않는다. |

이 저장소는 이미 캠페인 상태 writer, 실행 증거 publisher, 완료 판정, 노드 정체성 및 배달 계약을 갖는다. Unlazy 전체를 그대로 얹으면 같은 완료 사실을 두 곳에서 관리하고 승인·재개 경로가 갈라질 수 있다. 분석상 우선순위는 새 도구 설치보다 **요구사항이 기존 owner·산출물·검증으로 빠짐없이 연결되는지 확인하는 것**이다.

외부 스킬의 검토와 설치도 구분해야 한다. 이 보고서의 소스 검토와 임시 fixture 테스트는 검토 범위이며, 프로젝트 hook·전역 설정·스킬 발견 경로를 변경할 권한을 추가하지 않는다.

## 10. 최종 판단

Unlazy는 ‘더 열심히 하라’는 프롬프트를 유효한 원장·실행 검사·재검증·명시적 인계로 옮겼다는 점에서 좋은 참고 구현이다. 특히 정의가 바뀐 증거, 버려진 요구, 미완료 dispatch가 녹색 완료로 합쳐지는 것을 막는 설계가 유용하다.

반면 업무 의미·전체 요구사항·호스트 핸들의 진실성·실제 권한 격리까지 보증하지는 않는다. 현재 저장소에는 원칙과 필요한 일부 검증 패턴을 기존 계약 안에서 선별적으로 검토하는 편이 적절하다. 전면 도입의 효과는 현행 체계와 같은 과제를 비교한 별도 실행 증거가 있어야 판단할 수 있다.

## 근거 위치

코드와 패키지 문서는 앞서 명시한 Git 커밋을 기준으로 한다. 함수 이름·절 제목을 재검색 키로 사용한다.

- **U1:** `scripts/lib/gates.mjs`의 `parseGates`, `gateDefinitionDigest`, `classifyGateEvidence`, `gateState`, `claimLeases`.
- **U2:** `scripts/gate-check.mjs`의 `oracle`, `approvalPath`, `runCheck`, `evidenceFor`, scope 최종 집계.
- **U3:** `templates/PLAN.md`의 Current contract inventory·Leaf dispatch table, `tests/contract-tests.mjs`의 로컬 `reviewContract`.
- **U4:** `scripts/gate-lint.mjs`의 lexical warning·strict 계약, `references/gates.md`의 Author gates that can fail.
- **U5:** `SECURITY.md`의 CHECK lines are code·Scopes and leases are not a sandbox·Evidence and logs.
- **U6:** `scripts/lib/dispatch.mjs`의 `updateDispatch`, `validateState`, `dispatchStatus`; `references/dispatch.md`의 Evidence boundary.
- **U7:** `scripts/stop-hook.mjs`의 `MAX_BLOCKS`, progressHash, release·block 분기.
- **U8:** `scripts/install-hooks.mjs`의 target 선택, `isOurHandler`, `nextSettings`, `persist`.
- **U9:** `README.md`의 Version status·Research basis, `research/validation-protocol.md`의 Historical limitations.
- **U10:** `references/gates.md`의 Leaf gates versus branch gates, `SKILL.md`의 Work each leaf in four passes와 작은 작업 제외 조항.
- **저장소 대조:** `CLAUDE.md`, `.claude/rules/workflow.md`, `.claude/policies/root_registry.json`, `.claude/policies/runtime/completion_gate.py`, terraforming의 `references/install/harness_environment.md` 및 `references/orchestration/agent-control-adapter.md`.

[U1]: ../../seed/unlazy/scripts/lib/gates.mjs
[U2]: ../../seed/unlazy/scripts/gate-check.mjs
[U3]: ../../seed/unlazy/templates/PLAN.md
[U4]: ../../seed/unlazy/scripts/gate-lint.mjs
[U5]: ../../seed/unlazy/SECURITY.md
[U6]: ../../seed/unlazy/scripts/lib/dispatch.mjs
[U7]: ../../seed/unlazy/scripts/stop-hook.mjs
[U8]: ../../seed/unlazy/scripts/install-hooks.mjs
[U9]: ../../seed/unlazy/research/validation-protocol.md
[U10]: ../../seed/unlazy/references/gates.md
