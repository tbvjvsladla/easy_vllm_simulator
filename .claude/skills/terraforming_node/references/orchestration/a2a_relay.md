# 오케스트레이션 — A2A 제어명령 · 턴제 릴레이 · 러너 사다리

> terraforming_node 스킬 reference — SKILL.md §2.7.7 · §2.7.7a · §2.7.11 의 본문이다. SKILL.md(라우터)는 § 번호·제목·포인터만 든다.
> 이관 전 원문: `git show dcb713a:.claude/skills/terraforming_node/SKILL.md` (plan_26093022 Step 4 — 본문 바이트는 그대로 옮겼다).

## 목차

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
| `relay.py --continue` | 소진·유보 뒤 | **원장에서 본문을 조립**해 미리보기 + 예산·세션 **사실** 표시(기본 dry-run) | B |
| `relay.py --continue --apply --max-turns N --timeout-seconds N --budget-source "..." --resume <id\|new>` | **예외 경로** — 감독 스텝이 팝업한 뒤 사람이 본문을 승인 | 조립 본문 + 새로 선언한 예산 + **선언한 세션**으로 delegate | B |
| `relay.py --supervise-step <camp> [--apply]` | attempt 사이 | 원장 + 회수 브리핑을 읽어 **한 번 판정**하고 원장에 적는다. `--apply` 면 전진이 보이는 중단을 자동 재발급 | B |

- **재개의 기본은 감독 스텝 자동 재발급이다**(`--supervise-step <camp> --apply` · 사용자 결정 D9). 수동 경로 `--continue --apply` 는 **예외 팝업 뒤 사람이 고른 경우에만** 쓴다. 5시간 한도
  프로바이더만 쓰는 환경에서 매 재개마다 사람을 기다리면 그 대기가 캠페인의 벽시계를 지배한다
  (2026-09-07 실측: HITL 응답 대기 70분이 순차 실행의 직접 원인 중 하나였다). 자동의 **예외**는
  둘뿐이다 — 선언된 비용 상한에 닿았거나, 모델·통신 평면이 깨졌을 때. 그 둘은 팝업으로 간다.
  전진이 없는 중단도 팝업이다(예산을 키우기 전에 묻는다).
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

- **조립기는 합성하지 않는다** — 직전 attempt 의 제어 상태(원장) · 서브가 보낸 `artifacts[]`·
  `next_steps`·`notes` · 사람이 `campaigns/<camp-id>/relay/pending_hitl.json` 에 적은 `answer` · 원 지시. 그 넷뿐이다.
  메인이 추측한 진행상황을 본문에 적으면 그것이 SILENT_FALLBACK 이다.
- **정지 조건 둘**(루프를 만들면서 정지 조건을 미루지 않는다): ⓐ 직전이 `completed` 면 이을 중단점이
  없다 ⓑ **차단성 요청에 답이 없으면** 진행하지 않는다. ⓑ가 사람의 승인 정문이다 — **답이 곧 승인**이다.
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
- **회전은 비용이 아니다** — `--cost-cap-attempts` 는 서브에 닿은 attempt 만 센다.

**Codex 예산과 상태 소유권.** Codex는 v2 wall-time 계약을 쓴다. `--max-turns`를 넘기지 않고
`--timeout-seconds`·`--max-dispatches`·`--budget-source`를 선언한다. native turn 수는 모르면 null이다.
외부 dispatch cap은 실패한 발행도 세며 Claude native 비용 상한과 별도다. 운영 서브는
`--permission-profile vllm-sub`, 읽기 전용 카나리는 `--sandbox-mode read-only`를 사용한다.
named profile이 없거나 신뢰되지 않았으면 수리·검증하고 다시 진입한다. 더 넓은 sandbox로 대체하지 않는다.

이 cap은 context 전체에 결속되므로 Claude-only 러너로 전환해도 없어지지 않는다. 마지막 허용
dispatch가 완료된 경우에는 완료를 보고하며, cap은 다음 발행을 막는다. 감독은 context마다
campaign·통제변인·node·권한을 원장에서 새로 복원하고 수동 재개와 같은 정체성·잔존 서비스 게이트를 통과한다.

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

**전제(노드 로컬).** shim 은 **실행 파일**이어야 한다(`~/.local/bin/<name>`, 비추적). `.bashrc`
셸 함수로는 안 된다 — 이유가 둘이고 각각 독립이다: ⓐ ssh 비대화형(`bash -lc`)에서 `.bashrc` 는
`case $- in *i*)` 로 **조기 return** 한다 ⓑ 어댑터가 감싸는 `timeout <n> <cmd>` 는 셸 함수를
**exec 할 수 없다**. 키는 shim 옆 `~/.<name>.env`(0600)가 소유하며 **추적 파일에 적지 않는다**.



## relay.py 메모 (옛 §4 에서 이관)

- `scripts/relay.py` — **메인↔서브 턴제 릴레이 실행자**(§2.7.7a). `--task|--task-file` 첫 위임 · `--continue [--apply]` 자율 재개 · `--emit-only` · `--self-test`. 원장 `campaigns/<camp-id>/relay/<ctx>.json`, 대기 요청 같은 자리의 `pending_hitl.json`(사람이 `answer` 를 적는 자리). 경로 파생의 단일 소유자는 `scripts/campaign_init.py --derive`.
