# 오케스트레이션 — A2A 제어명령 · 턴제 릴레이 · 러너 사다리

> terraforming_node 스킬 reference — SKILL.md §2.7.7 · §2.7.7a · §2.7.11 의 본문이다. SKILL.md(라우터)는 § 번호·제목·포인터만 든다.
> 이관 전 원문: `git show dcb713a:.claude/skills/terraforming_node/SKILL.md` (plan_26093022 Step 4 — 본문 바이트는 그대로 옮겼다).

## 목차

정상 완료 결과는 인수하고 다음 업무로 진행한다. `relay.py`는 완료된 같은 context에 새 인계 문제 없는
지시가 반복되면 저장된 리포트를 반환하고 worker를 새로 기동하지 않는다. 이 결과 재사용은 native
실행 성공·실패가 아니며 별도 승인·캠페인 중단도 만들지 않는다. 실제 문제의 정정 Task에는
`--handoff-issue`로 반환 결과의 문제와 근거 위치를 함께 명시한다. 새 과업은 기존 계약대로 새 context다.
반환 결과에서 실제 인계 문제가 드러난 경우에만 해당 토폴로지의 예외 처리 참조를 읽는다.
참조: [토폴로지별 인계 예외](../../../../policies/handoff_exception.topology.md).

- 2.7.7 A2A 제어명령 프로토콜 (신설 — 교착 해제)
- 2.7.7a 턴제 릴레이와 자율 재개 (2026-09-04 · `plan_26090412`)
- 2.7.11 러너 사다리 — A2A 위임의 **실행자 축**과 회전 (신설 2026-09-08 · 사용자 지시)
- relay.py 메모 (옛 §4 에서 이관)

### 2.7.7 A2A 제어명령 프로토콜 (신설 — 교착 해제)

> 신설: `plan_26081514_…_구현.md` Step 1. §2.7.1 의 *"서브 git 은 관측 장치이고 메인은 완전한 조작 권한을 갖는다"* 를 **실행 가능한 명령 시퀀스**로 만든 것. 이것이 없으면 원칙은 있는데 처방 주체가 없어 §2.7.2 실증과 같은 교착이 반복된다.

**전제**: `policy:A2A_IDENTITY_PROOF_FAIL_CLOSED` 통과(정체성 증명 부재·위조 → 진입 금지) · 대상은 **서브 git 과 정보량 0 상태 결손만**(콘텐츠 저작은 §2.7.2 저작 경로로).

| 명령 | 언제 | 무엇 | 평면 |
|---|---|---|---|
| `sub.git.status` | 배달 전 상시 | 서브 워킹트리 dirty 여부 관측 | B(관측) |
| `sub.git.autosave` | dirty 감지 | `git add -A && git commit -m '[autosave] pre-sync'` — **소실 방지·메인 자율** | B |
| `sub.git.unstick` | 배달 검증기가 거부 | `git checkout -- <path>` / `git clean -fd <path>` — **경로 한정**, 전역 금지 | B |
| `sub.fs.repair` | 빈 디렉터리·mode 오차 | `rmdir`/`chmod` — **정보량 0 한정**(§2.7.2 정비) · 로그 필수 | B |
| `sub.git.log` | 상향 관측 | 서브 작업 이력 조회(내용 아닌 이력) | B(관측) |
| `sub.campaign.brief` | attempt 사이 상시 | 서브가 `docs/logs/<node_id>/campaign_brief.json` 에 낸 진행 요약을 **회수 미러에서** 읽는다(셀·phase·여정·`last_utc`) | B(관측) |

- **경로 한정이 안전장치다** — `unstick`·`repair` 는 배달 대상 경로에만 건다. `BAND2_EXCLUDED_TOP` 및 그 **하위 전부**는 대상 밖(§2.7.2 ⚠).
- **모든 제어명령은 로그를 남긴다** — 침묵 조작은 "원래 그랬던 것"과 구분되지 않는다.
- **바이트를 가진 것의 삭제는 이 프로토콜 밖**이다 — `ALLOW_DELETE` 게이트 관할이며, 그 안내문을 그대로 따르면 파괴가 완성되는 형태였던 선례(D5)가 있으므로 **안내문 복창 금지**.
- 실행문법(provider 별 `claude -p` 호출 형태)은 `references/orchestration/agent-control-adapter.md` 에서만 해소한다.
- **`sub.campaign.brief` 는 유일한 진행 관측면이다**(2026-09-08 신설 · `plan_26090813` §4.2). 서브가
  publish 마다·phase 전이마다 갱신하고, 메인은 `fetch_sub_docs.sh` 미러(`sync_staging/sub_docs/logs/
  <node>/campaign_brief.json`) 밖에서 서브를 읽지 않는다. **왜 신설했나**: attempt 사이에 허가된
  관측 채널이 없어서 메인이 서브를 ssh 로 32회 직접 관측했다(2026-09-07 실측 — 헌법 노드제어 ①
  무단 스캔 금지 위반). 채널이 없으면 사람은 우회를 만든다(D3: 우회 대신 경로를 만든다).
- **사전 도달 확인도 허가 통로로 한다**(2026-09-08 실측 교정). 캠페인 착수 전 "서브가 살아 있나"를
  확인하려고 `ssh` 를 직접 여는 것은 무단 스캔이다 — 나는 그것을 한 번 했고 A1 이 잡았다. 대신
  **`fetch_sub_docs.sh --topology=<t>`(인자 없이 = DRY-RUN)** 를 쓴다: SSH 도달을 스스로 검사하고
  무엇을 가져올지만 보여주며 서브를 변경하지 않는다. 애초에 사전 확인이 없어도 릴레이가 전송
  실패로 fail-loud 하므로, 이 확인은 편의이지 전제가 아니다.
- **감독자의 주기 읽기는 승인된 attempt 에 종속된 관측이지 새 트리거가 아니다**(2026-09-08 명문화 ·
  사용자 결정 D8). `relay.py --supervise-step` 은 원장과 이 브리핑을 읽고 한 번 판정해 원장에 적고
  끝난다 — 상주하지 않으며, 스스로 새 과업을 열지 않는다. 헌법 §트리거의 "무인 자동 실행 금지" 는
  **작업 착수**를 말하는 것이고, 이미 승인돼 도는 attempt 의 진행을 읽는 것은 그 금지의 대상이 아니다.

#### 2.7.7a 턴제 릴레이와 자율 재개 (2026-09-04 · `plan_26090412`)

**소진·유보로 끊긴 작업을 다음 attempt 가 이어받는다.** 전송(같은 세션 `--resume`)은 2026-09-03 부터
실동했으나 **이어붙이는 실행자가 없어**(호출자 0) 사람이 본문을 손저작해야 했다 — 그 자리를 `relay.py`
가 채운다.

| 명령 | 언제 | 무엇 | 평면 |
|---|---|---|---|
| `relay.py --task ... --max-turns N --timeout-seconds N --budget-source "..." --resume new` | 첫 위임 | **선언한 예산·재개**로 delegate · 원장 개설(`campaigns/<camp-id>/relay/<ctx>.json`) | B |
| `relay.py --continue --apply --task-file <현재 적용할 전체 지시> --context-id <동일 context> --resume <id|new> …` | 서브가 메인 소유의 정본 수정·기술 판단을 요청했고 메인이 해결했을 때 | 정정과 유지할 조건을 포함한 명시 원문을 새 attempt의 지시 출처로 보존한다. 기존 캠페인·셀·provider·권한 결속과 예산·HITL·복구 가드는 그대로 적용한다 | B |
| `relay.py --continue` | 소진·유보 뒤 | **원장에서 본문을 조립**해 미리보기 + 예산·세션 **사실** 표시(기본 dry-run) | B |
| `relay.py --continue --apply --max-turns N --timeout-seconds N --budget-source "..." --resume <id\|new>` | 명시적 후속 지시·체크포인트 재개. 실제 미해결 사람 결정은 별도로 충족 | 조립 본문 + 한 번의 실행 예산 + **선언한 세션**으로 delegate | B |
| `relay.py --supervise-step <camp> [--apply]` | attempt 사이 | 원장 + 회수 브리핑을 읽어 **한 번 판정**하고 원장에 적는다. `--apply` 면 전진이 보이는 중단을 자동 재발급 | B |

- **재개의 기본은 감독 스텝 자동 재발급이다**(`--supervise-step <camp> --apply` · 사용자 결정 D9). `--continue --apply --task-file`로 기존 승인 범위의 기술 정정도 명시할 수 있다. 실제 미해결 사람 결정은 먼저 충족한다. 5시간 한도
  프로바이더만 쓰는 환경에서 매 재개마다 사람을 기다리면 그 대기가 캠페인의 벽시계를 지배한다
  (2026-09-07 실측: HITL 응답 대기 70분이 순차 실행의 직접 원인 중 하나였다). 자동의 **예외**는
  미해결 모델/통신 평면 실패·미해결 승인/권한 거부다. 누적 호출 횟수는 예외가 아니다. Codex의 관측된 신호 종료·중단·불명 native/control 실패도
  전진만으로 재개하지 않는다(아래 복구 계약). 전진이 없는 중단도 팝업이다(예산을 키우기 전에 묻는다).
- `--continue --task-file`은 파일의 전체 원문을 명시 지시로 보존하고 미리보기에도 표시한다. 파일이 없으면 기존 지시 출처를 이어 조립한다. `--continue --task` 및 `--task`와 `--task-file` 동시 사용은 거부한다. 기존 차단성 요청과 수동 복구 조건은 동일하게 적용한다.
- **기술 feedback은 사람의 답이 아니다.** 메인이 소유한 코드·계약을 수정한 뒤에는 같은 context에 `--task-file`로
  정정 내용·근거와 **계속 유효한 조건을 포함한 전체 지시**를 보낼 수 있다. 부분 변경을 발행기가 의미 추론으로
  병합하지 않는다. `pending_hitl.answer`에 이를 사람 답처럼 기입하지 않는다. 이 경로도
  `require_dispatch_recovery`와 동일한 결속·잔존 서비스 가드를 거치므로 미해결 승인·권한 거부·중단을 우회할 수 없다.
  최초 `task`는 이력으로 보존한다. 명시 입력 원문은 dispatch reservation을 거쳐 append-only attempt의
  `task_instruction`에 남고, `request_path`·`request_digest`·해당 attempt의 실행 결속·캠페인/셀 축으로 검증한다.
  수동·자동 재개의 활성 지시는 **최신 명시 원문**이다. 자동 조립된 이어받기 본문은 새 명시 지시로 승격하지 않고
  `source_attempt`만 이어받으므로 오래된 지시 재주입과 본문의 재귀 중첩을 막는다. 직전 리포트의 사실·남은 일과
  실제 사람 답은 별도로 보존하며, 지시 출처가 있다는 사실로 실패 복구·예산·잔존 서비스 가드를 열지 않는다.
- **구 원장의 지시 출처 결손은 메인 소유의 기술 인계다.** 단일 최초 attempt는 실제 request와 durable outcome의
  결속 및 최초 원문의 일치를 확인한 경우만 호환해서 읽는다. 명시 입력과 자동 조립을 구분하지 않은 다중 attempt는
  문구를 보고 추측하지 않는다. 출처 부재·파손·불일치도 최초 `task`로 침묵 복귀하지 않고
  `TASK_INSTRUCTION_SOURCE_UNRESOLVED`를 낸다. 메인이 기존 request와 승인 범위를 검토하여 완전한 `--task-file`을
  명시 발급하면 이후 재개의 출처가 확정된다. 이것만으로 사용자 HITL이나 권한/제어 실패 복구 질문을 새로 만들지
  않는다. 기존 실제 차단 사유는 그대로 해소해야 하며, 구 원장·진행 중 reservation·예산은 손편집하거나 초기화하지 않는다.
- **독립 산출물 검증 인계**: 해당 노드의 `node_role_contract`와 산출물 owner가 독립 검증을 요구하는 경우,
  저자는 lint 0·자기 서비스 정리 뒤 draft·원천·남은 일을 checkpoint로 남긴다. 메인 호출자는 지원되는 독립 실행
  경로를 골라 같은 승인 셀을 참조하는 **새 검증 역할 context와 새 native 세션**을 발급한다. 이는 기존 저자 셀
  context의 소유·원장을 대체하거나 새 셀을 만드는 것이 아니다. provider·model·effort·권한 선택과 현재 단계의
  동적 실행 예산을 명시하고, 실행 요청과 실제 관측을 구분한다. 검증자는 저자가 아니며 factcheck 보고와 docs
  receipt를 남긴다. 열린 지적이 있어도 검증 범위 전체와 요구된 모든 부류를 먼저 대조·기록한다. owner가 처방하고
  독립 검증자가 재확인한 뒤 저자가 공식 발행 경로를 잇는다. 검증 역할 `completed`는 셀 완료나 태그 발행이 아니다.
  상세 게이트·7부류는 `hint-publisher` SKILL.md §2의 소유다.
  중첩 provider 실행이 항상 가능하다고 가정하지 않는다. 실제 사례에서 nested 실행은 세션 생성 전 ROFS로 실패해
  모델 실행·receipt가 없었고, 메인이 별도 정식 A2A 검증 context를 발급해 실제 세션을 관측했다. 그 오류의 원인을
  추측하지 않는다. 정식 경로도 막히면 기존 owner·request/HITL 계약으로 사실을 인계한다. profile 확대·우회,
  사적 설정 복사, 전역 쓰기 권한 부여 또는 새 blanket 승인을 만들지 않는다. 멀티 Ray worker의 역할은 manifest에서
  파생하며 이 절이 서브 publisher 권한을 만들지 않는다.
- **전진 신호는 넷이다**(2026-09-29 · `plan_26092919` P1): phase 변화 · 회수 브리핑 `last_utc` · 원장
  (리포트의 산출물·`next_steps` 변화) · **문서 평면**(회수 미러 문서가 그 context_id 를 담고 mtime 이 attempt
  창 안). 브리핑이 없는 서브(멀티 Ray 워커)와 리포트 없이 잘린 attempt 를 위해 뒤의 둘을 더했다. 질문의
  표지는 `hitl.needed` 하나이며 사람에게 간다. 질문 표지 없이 `next_steps` 만 남긴 `input-required` 는 예산 양보다.
  도서관 질문은 릴레이를 타지 않는다 — 서브가 manifest `library` 접근자(wiki-desk CLI · SKILL.md §2.7.8)로
  메인 서고를 **직접** 묻고 자기 턴 안에서 답을 얻는다(2026-10-19 · `plan_26101912`: 서고 교환은 은퇴).
- **HITL 은 셀 사이에 있고 재개는 셀 안에 있다**(D16). 셀이 끝난 뒤의 전이가 `HITL` 모드면 정리 후
  그 자리에서 사람을 기다리며, 무인이라도 기다린다 — 그것이 그 모드를 선언한 이유다.
- **셀 하나 = context 하나**(D20). 셀 context 는 서빙→벤치를 담고 빌드는 별도 문맥이다. 2026-09-07
  에는 셀 둘을 한 context 에 묶어 attempt 2회가 모두 캡에서 잘렸고, 서브 자신이 셀 단위를 권고했다.
  원장은 `campaign_node`·`campaign_context_kind`·`campaign_cell` 을 **선언으로** 든다(이름 추론 ✗).
- **시간 예산은 실측에서 파생한다**(D21): `--budget-from-phase` 가 같은 노드의 지난 phase elapsed 에서
  파생하고, 실측이 없으면 스키마 상한을 **읽어** 그 사실을 근거에 적는다. 스키마 상한은 그대로 두고
  값을 코드에 리터럴로 복제하지 않는다.
- **재개는 선언이다**(2026-09-05 · 축 F): 종전에는 `latest_session_id()` 가 원장을 보고 코드 규칙으로
  정했고 그 규칙이 라이브에서 두 번 어긋났다(완결 뒤 옛 세션 반환 · 소진 세션 무조건 폐기). 이제
  dry-run 이 **마지막 알려진 세션과 그 맥락**을 보여주고, `--resume <session_id|new>` 로 선언하지 않으면
  fail-loud 한다. 리포트 없이 끝난 턴은 `campaigns/<camp-id>/relay/pending_hitl.json` 에 그 세션 id 를 남긴다(침묵 종결 ✗).
- **원장은 append-only 다**: attempt 마다 `request_path`(보낸 요청 원문)·`report_path`·`end_reason`·
  `started_utc`/`ended_utc`(메인 실측)·`duration_ms`/`duration_api_ms`(provider 보고)를 적는다.
  **정지 시간 = wall − api** 이며 두 값의 출처가 다르므로 섞지 않는다.
- **권한 거부는 사건이지 판정이 아니다**(2026-10-01 · plan_26100113 D2): provider 는 거부가 1건이라도 있으면
  `execution_failed · PERMISSION_DENIED` 를 낸다(exit-code 표 불변). 릴레이는 서브 리포트가 **파싱되고**
  `status: completed` 일 때 `end_reason = completed_with_denials` 로 가르고, 감독은 다음 셀로 간다(차단 ✗).
  거부 블록은 원장 `permission_denials` 에 그대로 남는다. 리포트 부재·다른 status 는 종전대로 `permission_denied`
  (팝업)다. `parse_report` 는 잘린 JSON(거부 상세는 도구 입력을 240자에서 자른다)이 뒤의 리포트를 가리지 않게
  `{` 마다 디코딩을 시도한다 — 라이브에서 completed 리포트가 "JSON 없음" 으로 집계된 원인이었다.

#### Codex 중단·실패 복구 — 답과 명시적 수동 재개

`approval_policy="never"`와 닫힌 stdin으로 실행하는 서브는 추가 권한을 대화형으로 승인받지 않는다.
이 설정은 허용 권한을 넓히는 승인이 아니다. 미완수 `approval_required`/권한 거부, 관측된 신호 종료,
native/process 실패와 불명 control 실패는 원장 `recovery_required`와 `pending_hitl.json`의
`relay-recovery-aNN`(`blocking:true`)에 같은 attempt·dispatch ID로 결속한다. 전진 흔적이나
provider 변경은 그 잠금을 풀지 않는다. 기존 `completed_with_denials`의 완수 판정과 거부 기록은 유지한다.

정상 native 실행 예산 소진은 위의 불명 실패와 구분한다. Claude의 관측된 native-turn 소진이나
Codex adapter의 제어된 deadline 집행·워커 회수 증거가 durable 요청/결과와 같은 세션·권한에
결속되면 자동 체크포인트 재개 대상이다. 이전 워커가 남긴 명령·서비스를 먼저 확인하며 예산 소진을
과업 완료로 세지 않는다. 단순 exit124나 SSH 감시 timeout은 워커 회수의 증거가 아니다.

- provider는 실제로 받은 signal 번호, 자식의 음수 rc, rc130/137/143의 shell signal 관례를
  `failure_evidence.termination`에 출처별로 보존한다. SIGTERM이나 ESC 화면 닫힘만으로
  **사용자가 취소했다**고 단정하지 않는다. 관측된 중단은 `process_interrupted`, 불명 control 실패는
  `control_failure`로 남기며 자동 회전·재개하지 않는다.
  adapter가 직접 받은 신호는 동시에 관측된 rc124보다 우선한다. 반면 정상 deadline 정리 과정의 자식
  SIGTERM은 wall timeout으로 남는다. 완수 리포트·거부가 함께 있어도 후속 adapter 신호가 있으면
  `completed_with_denials`로 넘어가지 않고 복구 질문으로 남긴다.
- 사람이 원장·고유 outcome·raw events와 이미 발생한 효과를 확인한다. `inflight`가 남았으면 먼저
  기존 `--reconcile-inflight` 계약으로 그 dispatch의 결과만 회수한다. 불명 결과를 버리거나
  예산/attempt를 초기화해 새 작업으로 대신하지 않는다.
- 현재 dispatch에 결속된 질문의 `answer`에 사람의 복구 결정을 **비어 있지 않은 문자열**로 기록한 뒤,
  `--continue --apply --resume <session_id|new>`와 예산 근거를 명시한다. 답만 기록하고 감독을 다시 부르면
  여전히 팝업이다. 답은 도구 권한 확대를 허용하지 않으며, 같은 provider/target/권한 resume 결속,
  요청 digest, 잔존 서비스·정체성 가드는 그대로 적용된다. 과거 외부 호출 상한은 실행 이력으로만 남는다.
  완수 리포트 뒤 제어 프로세스가 신호로 끊긴 경우에도 이 명시적 복구 경로를 사용한다. 리포트의
  completed를 지우지 않으며, 그것만으로 미해결 제어 상태를 완료 처리하거나 복구를 영구 금지하지 않는다.
- producer가 원장 기록 후 질문 기록 전에 종료됐거나 pending 파일이 사라져도, 다음 continue/감독/dispatch
  진입은 원장 관측에서 같은 blocking 질문을 복원한다. 다른 dispatch의 답은 승계하지 않는다.
  복구한 실행도 새 dispatch 1회로 계상하며, 이전 실패와 raw 증거는 유지한다.

wall timeout은 사용자 취소로 단정하지 않는다. 제어된 워커 회수 증명이 있는 정상 예산 소진과,
**CLI는 completed이고 task report만 파싱되지 않은** 경우의 기존 D9 재개는 유지한다. 불명 timeout을
전진 흔적만으로 정상 소진이라고 바꾸지 않는다.
다만 같은 timeout에 구조화된 승인/권한 거부가 관측되면 그 거부를 지우지 않고 복구 질문으로 차단한다.

- **조립기는 합성하지 않는다** — 직전 attempt 의 제어 상태(원장) · 서브가 보낸 `artifacts[]`·
  `next_steps`·`notes` · 사람이 `campaigns/<camp-id>/relay/pending_hitl.json` 에 적은 `answer` · 발급 증거로 검증한 최신 명시 지시. 그 넷뿐이다.
  메인이 추측한 진행상황을 본문에 적으면 그것이 SILENT_FALLBACK 이다.
- **정지 조건 둘**(루프를 만들면서 정지 조건을 미루지 않는다): ⓐ 직전이 `completed` 이고 미해결 제어
  상태가 없으면 이을 중단점이 없다. 완료 뒤 신호 중단은 위 Codex 복구 계약을 따른다.
  ⓑ **차단성 요청에 답이 없으면** 진행하지 않는다. 답의 존재를 승인으로 간주하지 않고, 호출자가
  사용자의 실제 의사에 따라 재개 여부를 판단한다. 가드는 답의 의미를 자동 판정하지 않는다.
  Codex 미해결 실패는 복구 결정 기록과 명시적 수동 재개를 모두 요구한다.
  종전의 ⓒ("전진 없는 attempt 3회 = `MAX_ATTEMPTS_BEFORE_HITL`")는 2026-09-05 삭제됐다 — 그 3 은 어떤
  실측에서도 오지 않았고 정지 결정은 이미 `--apply` 가 쥐고 있었다. 전진 없는 연속 수는 이제 dry-run 이
  **보여주고**(집행된 예산 바닥·직전 소진 여부와 함께), 멈출지는 그 화면을 본 쪽이 정한다.
- **우선순위는 차단성 선언에서 나온다**: 대기 항목의 `blocking`(릴레이 crash 항목 · hitl 절의 선언)을
  메인이 **읽는다**(2026-09-04 이전에는 읽는 코드가 0 이었다). 대기 목록은 `blocking` 우선, 그 안에서 attempt 순이다.
  표면화 표지는 `hitl.needed` 다. 근거 자료는 릴레이로 묻지 않는다 — 서브가 manifest `library`
  접근자(wiki-desk CLI · SKILL.md §2.7.8)로 메인 서고에 직접 닿는다(2026-10-19 · `plan_26101912`).
  옛 대기 파일에 남은 서고 요청 본문은 읽혀도 표면화·차단성을 만들지 않는다.
- **예산 바닥은 집행된 사실이다**: 한 context 안에서 예산은 내려가지 않으며, **서브에 닿지 못한
  attempt**(전송·스키마 실패)의 요청 예산은 바닥으로 치지 않는다(거절된 요청이 릴레이를 잠그던 형태).


### 2.7.11 러너 사다리 — A2A 위임의 **실행자 축**과 회전 (신설 2026-09-08 · 사용자 지시)

> **정본은 여기다.** 헌법에는 "왜" 한 줄만 남고(`policy:` 접두 없음 — registry 등재 정책이 아니다),
> 어휘·판정·회전·소진은 이 절이 소유한다. 실행 문법(바이너리·별칭 표)은
> `references/orchestration/agent-control-adapter.md` §2.1 이 소유한다.

**어휘.** 러너 = **(provider, backend, model)** 이다. provider는 이너 하네스, backend는 그 하네스의
모델 제공 경로다. Claude 내부에서는 backend/model이 한 쌍이다 — shim 이
`ANTHROPIC_DEFAULT_*_MODEL` 을 자기 슬롯으로 덮으므로 kimi 아래의 `sonnet` 은 sonnet 이 아니고,
모델 토큰은 백엔드 사이에서 **이식되지 않는다**. 별칭(`sonnet`·`opus`·`haiku`·`kimi-claude`·
`minimax-claude`·`meta-claude`)을 쌍으로 펴는 **닫힌 표**는 어댑터가 소유하고, 중립 통로는
`agent_control.py runners --provider all` 다(사본 ✗). Codex 선택은 `codex-gpt-6.1-sol`이며
기존 `openai-claude`와 다르다. 별칭 등재는 선택 가능성을 뜻하고 기본 사다리를 바꾸지 않는다.

> ⚠ **shim 은 판정 모델까지 덮는다**(2026-09-15 실측). auto mode 의 안전 판정 호출도 shim 의 모델
> 슬롯을 타므로, 그 엔드포인트가 판정에 응답하지 못하면 Edit·Bash·Agent 가 전부
> `<model> is temporarily unavailable, so auto mode cannot determine the safety of <Tool>` 로 선다
> (Read 만 통과 · **권한 거부가 아니다**). 위임 `-p` 는 렌더된 `defaultMode: default` 로 돌아 이 판정을
> 타지 않는다. 대화형 메인을 shim 으로 띄울 때는 auto mode 를 쓰지 않는다. 서브의 root 관리설정은
> 머신 전체·최상위라 **서브에만** 둔다 — 메인에 깔면 `Edit(<ws>/.claude/**)` deny 가 하네스 수정을 막는다.

**사다리와 회전.** 사다리 = 러너의 순서 있는 목록(`relay.py --runners a,b,c`). 회전은
실행 전 러너 불가가 증명된 경우에만 승인된 과업을 다음 칸으로 넘긴다. provider별 예산 단위는
보존한다. provider가 바뀌면 native 세션은 새로 열고 아티팩트로 인계한다. 사다리는 **순환**한다(4→1→…).
바뀌는 것은 *누가 실행하는가* 하나이므로 **회전은 재시도도 예산 사건도 아니다**(`scope ⊥ budget`
에 축이 하나 붙는다). 생략 시 1칸(`sonnet`)이며 이는 이 기능 도입 **전과 동작이 같다**.

**기본값은 하나뿐이다**(2026-10-26 사용자 정의). 고정된 것은 `claude -p` · 모델 `sonnet` 1칸이다. 그 뒤에 슬롯을
몇 개 두는지, 어떤 순서인지, 서드파티 LLM 을 어떤 명령(예: 라우터 런처)으로 부르는지는 provider 정책과 크레딧
상황에 따라 세션마다 바뀌는 **가변 요소**다 — 공통층 기본값에 굽지 않고 그때그때 `--runners` 로 선언한다.
별칭 표의 등재는 *고를 수 있다*는 뜻이지 *기본으로 쓴다*는 뜻이 아니다.

**추론 강도도 선택 선언이다**(2026-10-30). 슬롯별 `--runner-effort <러너>=<강도>`가 공통
`--reasoning-effort`보다 우선하며, 둘 다 없으면 해당 대상의 native 기본 정책을 선택한다.
`default`는 공통 또는 슬롯에 명시하여 이전 override 대신 native 정책을 선택할 수도 있다.
전역/슬롯 선언은 원장에 각각 보존하여 회전·수동 재개·감독 재개에서 다른 슬롯으로 새지 않게 한다.
요청 출처와 결과의 native 정책 관측을 구분한다. provider별 지원·실행문법·default 관측의 한계는
`agent-control-adapter.md` §3.2가 소유한다. 강도 설정이 한 바퀴 소진 HITL이나 세션/권한 가드를 바꾸지 않는다.

**판정이 회전보다 먼저다.** 판정기 없이 회전을 얹으면 빌드 실패 한 번에 사다리를 전부 태우고
**틀린 서사로** HITL 한다. 판정은 문구가 아니라 **구조 신호**를 읽는다(2026-09-08 실측 수확):

| 관측 | rc | `terminal_reason` | `api_error_status` | 판정 |
|---|---|---|---|---|
| 정상 | 0 | `completed` | null | — |
| 인증 실패 | 1 | **`api_error`** | 401 | 회전 ○ |
| 백엔드 미도달 | 1 | **`api_error`** | null(연결 오류) | 회전 ○ |
| 요청 거절 | 1 | `api_error` | 400·404·413·422 | 회전 **✗**(바꿔도 같은 거절) |
| 러너 바이너리 부재 | 127 | (봉투 없음) | — | 회전 ○ |
| ssh 전송 실패 | 255 | (봉투 없음) | — | 회전 **✗**(전송 평면) |
| 서브가 자기 과업에 실패 | * | `completed` | — | 회전 **✗**(세션이 있다) |

- ★ 실패 봉투도 `subtype == "success"` 다 — **봉투 형태로는 갈리지 않는다**. 갈리는 것은
  `terminal_reason` 이고, 그래서 판정이 언어·백엔드·버전에 흔들리지 않는다.
- **모르면 회전하지 않는다**(fail-closed). 판정하지 못한 실패에서 회전을 열면 근거 없이 사다리를
  태우고, 그 비용은 조용하다 — 다음 칸도 같은 이유로 죽고 사람은 마지막 칸의 사유만 본다.
- 사용량 한도(429)는 위 표의 `api_error` + 상태코드 경로로 **이미 덮인다** — 메시지 문구를
  추측해 넣지 않는다.

**회전의 실행자**(헌법 노드제어 ③). 두 자리이고 **규칙 함수는 하나**다(`next_runner_index` ·
`consecutive_runner_unavailable`):

| 자리 | 언제 |
|---|---|
| `relay.py` `run_attempt` 의 회전 루프 | 위임을 보냈는데 러너 평면에서 튕겼을 때 — 즉시 다음 칸 |
| `supervise_decide` 의 `rotate_runner` 분기 | 스텝 사이에 죽어 있던 것을 감독이 볼 때(`--apply` 면 재발급) |

- **커서를 저장하지 않는다** — 다음 칸은 원장에서 파생한다(같은 개념이 두 자리에 앉으면 갈라진다).
- **native 세션은 provider·target·권한 선택에 결속한다.** 같은 provider에서만 일치하는 세션을
  재개한다. provider 변경은 새 native 세션과 원장 기반 인계이며, 이전 ID를 새 provider에 보내지 않는다.
  실행이 시작된 뒤의 러너 실패는 이미 발생한 효과가 있을 수 있으므로 자동 회전하지 않는다.
- `run_attempt`와 감독 `supervise_decide` 모두 `runner_evidence.rotate=true`를 요구한다.
  판정이 없거나 false이면 감독이 다음 스텝에서 회전을 다시 열지 않는다.
- **호출 이력은 중단 조건이 아니다.** 회전·실제 워커 실행·실패는 각각 관측대로 센다. 과거 `--cost-cap-attempts` 값이나 원장의 누적 횟수만으로 다음 워커를 막지 않는다.

**Codex 예산과 상태 소유권.** Codex는 v2 wall-time 계약을 쓴다. `--max-turns`를 넘기지 않고
`--timeout-seconds`·`--budget-source`로 한 번의 실행 예산을 선언한다. native turn 수는 모르면 null이다.
`max_dispatches`는 과거 요청을 읽기 위한 호환 필드이며 새 요청에는 싣지 않는다. 누적 호출 횟수는
관측 이력이고 Codex·Claude 어느 경로에서도 자동 재개의 한도가 아니다. 운영 서브는
`--permission-profile :danger-full-access`, 읽기 전용 카나리는 `--sandbox-mode read-only`를 사용한다.
main/sub는 같은 built-in 권한을 쓰며 역할별 named 프로필을 생성하지 않는다. 설정이 신뢰되지 않았거나
managed 정책이 거부하면 기존 request로 보고한다. 기존 세션의 권한 결속이 다르면 아티팩트로 새 세션에 인계한다.

메인 자신이 상위 오케스트레이터가 호출한 워커여도 같은 규칙이다. 정상 실행 예산 소진 뒤에는
캠페인과 체크포인트에서 이어받으며 과업 종료·성공·새 승인을 추론하지 않는다. 감독은 context마다
campaign·통제변인·node·권한을 원장에서 새로 복원하고 수동 재개와 같은 정체성·잔존 서비스 게이트를 통과한다.

**호출별 예산은 동적으로 편성한다.** 명시된 현재 작업의 예산과 근거를 우선하고, 감독 재개는
서브의 유효한 제안·같은 context/단계의 최근 실제 실행 사용량·직전 예산 소진을 평가한다.
관련 없는 다른 단계의 총 경과시간이나 메인 대기 시간을 필요 예산으로 쓰지 않는다. 실제 예산
부족으로 소진됐으면 여유를 늘리고, 체크포인트가 전진해 남은 일이 줄었다면 근거 있는 작은
제안도 반영할 수 있다. native 전송 상한에 닿아도 과업 전체를 끝내지 않고 다음 실행에서 잇는다.
관측값·제안값·정책 여유계수·상한 조정의 출처를 `budget_source`에 구분한다. 같은 크기 반복이나
고정 등급표가 기본 정책은 아니며, Codex의 미관측 native 턴을 추정해서 만들지 않는다.

릴레이 CLI는 저장소 단일 writer 잠금을 얻고, 외부 실행 **전에** 원장에 context·과업·topology·manifest,
`inflight`·dispatch ID·요청 바이트 digest(비추적 요청)·예약 outcome 경로를 기록한다.
원장은 revision을 대조해 오래된 writer를 거부하며 pending HITL은 읽기부터 쓰기까지
직렬화한다. 프로세스가 사라져도 미해소 inflight를 시간 경과만으로 지우거나 재발행하지 않는다.
반환된 dispatch ID가 현재 예약과 다르면 늦은 결과로 거부한다. 이 상태에서는 기존 A2A·문서 회수
경로로 종료와 효과를 확인한 증거가 있어야 한다. 로컬 잠금 자체가 원격 종료를 증명하지는 않는다.

`agent_control invoke`는 정규화한 provider 결과를 예약된 outcome 파일에 원자적으로 영속화한 뒤
stdout으로 보낸다. 파일은 dispatch ID와 **전체 요청**에 결속되며 덮어쓰지 않는다. stdout 유실 뒤에는
`relay.py --context-id <ctx> --reconcile-inflight [--campaign-id <camp>]`로 검증하고, 같은 명령에
`--apply`를 붙여 **저장된 결과만** 원장에 append한다. 선택 경로가 필요하면 `--reconcile-result
<예약 outcome 경로>`를 쓴다. 재위임은 하지 않고 해당 dispatch를 소비한 것으로 센다.
요청 digest·dispatch ID·provider 결과 schema·요청 echo를 대조하며 경로 구성요소의 symlink를 거부한다.
잘못된 task self-report는 raw로 보존하며 completed로 승격하지 않는다. outcome이 없거나 정체성이
갈리면 계속 차단한다 — 임의 산문·시간 경과로 inflight를 지우거나 성공·종료를 만들어 적지 않는다.
이 outcome은 제어평면 증거이며 benchmark·artifact·hint 완료 게이트를 대신하지 않는다.

**소진 → HITL.** 서브에 닿은 마지막 attempt 이후로 사다리를 **한 바퀴** 다 돌았는데 전부 러너
평면에서 실패하면, 차단성 항목을 `pending_hitl` 에 표면화하고(답 없이는 `--continue` 진행 ✗)
감독은 `popup` 으로 끝난다. 사다리가 1칸이면 **첫 실패가 곧 소진**이다(사용자 규격).
대기 후 재시도는 이 절의 범위 밖이다 — 한도 리셋을 기다릴지 칸을 더할지는 **사람이 정한다**.

**사정거리.** `sub`/`ssh` 위임과 `main`/`local` 위임(probe·카나리)은 같은 어댑터를 타므로 사다리가
그대로 먹는다. **대화형 메인 세션 자신은 회전 대상이 아니다** — 자기가 도는 하네스의 LLM 을 자기가
바꿀 수 없다. 그것은 경계이지 결함이며, 처방(바깥 감독자가 러너를 골라 메인 루프를 기동)은
별도 안이다. 그래서 소진 팝업은 "메인 세션이 한도로 죽으면 캠페인 전체가 선다" 는 사실을 함께 적는다.

**전제(노드 로컬).** 현재 외부 backend는 adapter의 `bash -ic` 경유로 호출한다. 실행 파일뿐
아니라 대상 셸에 정의된 함수도 해소된다. 과거 비대화형 셸 조기 return 및 timeout의 함수 exec
불가 사례를 현재 호출 경로의 실행 파일 필수 조건으로 확대하지 않는다. 노드 로컬 인증정보는
launcher가 소유하며 추적물이나 노드 간 배달에 복사하지 않는다. 실행문법 정본은
`agent-control-adapter.md` §2.1과 provider adapter다.



## relay.py 메모 (옛 §4 에서 이관)

- `scripts/relay.py` — **메인↔서브 턴제 릴레이 실행자**(§2.7.7a). `--task|--task-file` 첫 위임 · `--continue [--apply]` 자율 재개 · `--emit-only` · `--self-test`. 원장 `campaigns/<camp-id>/relay/<ctx>.json`, 대기 요청 같은 자리의 `pending_hitl.json`(사람이 `answer` 를 적는 자리). 경로 파생의 단일 소유자는 `scripts/campaign_init.py --derive`.

#### 명시된 용량 재시도 배치

Codex 구조화 오류의 정확한 `Selected model is at capacity. Please try a different model.`만 선택적 재시도 대상이다. HTTP 코드나 rate limit은 추론하지 않는다. `--capacity-retries`는 한 번의 재시도 묶음에서 즉시 수행할 추가 시도 수다. 과업 전체나 세션의 호출 상한이 아니다. backoff·해당 묶음의 시간 조건·실제 사용자 사전승인 출처를 선언하고, 각 실행과 묶음은 고유 ID로 원장에 보존한다. 예전 `start_attempt`·`allocated_slots`는 과거 기록이며 다음 묶음을 막지 않는다.

한 묶음이 끝나도 실패를 성공으로 바꾸거나 호출 횟수 때문에 사용자에게 묻지 않는다. 체크포인트를 남기고 다음 감독 시점에 다시 판단한다. 재개는 같은 모델·provider·권한·대상과 관측된 세션 및 durable 요청/결과에 결속되며, capacity 복구의 미해결 명령·실제 권한 거부·불명 실패 가드는 유지한다. 현재 묶음에서 넘겨받은 deadline은 지키지만 모든 과거 묶음의 시각을 영구 과업 상한으로 합치지 않는다. 새 작업 상황에 대한 명시적 예산 선언은 새 이력이며 기존 기록을 충전하거나 지우지 않는다. GPU 탐색의 중지 조건처럼 독립된 owner의 제약과 사용자 중지는 그대로 지킨다.

정상 워커 실행 예산 소진은 별도로 다룬다. 실제 제한 집행과 워커 종료·회수가 결속된 관측으로 확인되면 다음 워커는 서비스·진행 중 명령·sweep·부분 산출물을 먼저 재확인하고 이어받는다. 진행 중인 벤치가 있어도 이를 완료로 기록하거나 새 벤치로 덮어쓰지 않는다. 원격 SSH 감시 timeout이나 단순 exit124처럼 이전 워커의 종료를 확인하지 못한 경우는 정상 회수라고 추정하지 않는다. 실제 권한·정체성·미해결 실행 상태의 차단은 유지한다.
