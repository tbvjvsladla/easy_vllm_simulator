# agent-control adapter — provider 전용 실행문법 경계 (조건부 reference)

> **이 파일과 `scripts/providers/`만 provider-specific 평면이다.** public SKILL 본문과 다른 reference는
> 위임 의도만 선언한다. 실행은 provider-neutral `.claude/skills/terraforming_node/scripts/agent_control.py`가 request schema를 검증한 뒤
> adapter로 라우팅한다. 정본 계약: `agent-control-request.schema.json`, `agent-control-result.schema.json`.
> **소유 = terraforming_node**(plan_26093022 — 07-25 "agent_control = Constitution-owned" 결정은 폐기됐다: 호출자가 이 스킬의
> relay·canary 뿐이라 기초층 런타임이 아니다). **Claude CLI 구문 격리 경계는 그대로다** — `claude` argv 의
> 유일한 발행처는 `scripts/providers/claude_code.py` 이고, `scripts/agent_control.py --self-test` 가 그 경계를 검사한다.

## 1. provider-neutral request

계약 v1은 기존 Claude 경로이며 v2는 Codex의 시간 예산·권한 선택·세션 결속을 명시한다.
schema와 `agent_control.py`의 교차필드 검증이 정본이다. `capabilities`는 작업 의도이며
도구 가용성의 보안 경계가 아니다. 실제 집행은 하네스의 권한·파일·네트워크 정책이 소유한다.

필수 필드:

- `schema_version: 1`
- `provider: "claude_code"`
- `intent: delegate | probe | bootstrap_canary`
- `model`: 호출자가 선언한 모델(예시의 sonnet은 고정 제약이 아니다)
- `task`: 비어 있지 않은 instruction
- `target`: `main/local` 또는 `sub/ssh`, 명시적 `work_dir`; SSH는 `host`·`ssh_user` 필수
- `timeout_seconds`, `max_turns`: 유한한 실행 경계
- `capabilities`: 작업 의도 목록(`read`, `execute`, `edit`, `write`, `search`)

예시:

```json
{
  "schema_version": 1,
  "provider": "claude_code",
  "intent": "probe",
  "model": "sonnet",
  "task": "Read CLAUDE.md and return one bounded JSON verdict. Do not edit.",
  "target": {
    "role": "main",
    "transport": "local",
    "work_dir": "/workspace/easy_vllm_simulator"
  },
  "timeout_seconds": 180,
  "max_turns": 8,
  "capabilities": ["read"]
}
```

실행:

```bash
python3 .claude/skills/terraforming_node/scripts/agent_control.py invoke --request /path/to/request.json
```

## 2. Claude Code adapter (v1)

`claude_code.py`만 generic capability를 Claude tool 이름으로 변환하고 다음 provider argv를 생성한다.
이 명령은 구현 계약 설명용이며 운영 호출자는 직접 복제하지 않고 위 orchestrator를 사용한다.

```bash
claude -p '<task>' --model <요청이 선언한 model> --output-format json --max-turns <N> --allowedTools <TOOLS>
```

실제 도구 결과를 검증하는 카나리는 선택 필드 `capture_native_events: true`를 명시한다.
이때만 adapter가 `--output-format stream-json --verbose`로 실행하고 native 이벤트를 `raw_events`에 보존한다.
최종 result·session·turn/budget 판정은 동일하며, 잘린 stream·중복 terminal result를 성공으로 바꾸지 않는다.
필드 부재/false인 일반 호출은 기존 JSON 계약을 유지한다. 이 옵션은 Claude v1 전용이며 Codex v2에는 싣지 않는다.
native tool_use/tool_result 결속과 실제 출력은 실행 증거이지, 해당 업무의 성공을 자동으로 보장하는 표식은 아니다.

### 2.1 백엔드 축 (`backend` 선택 필드 · 2026-09-07 신설)

같은 Claude Code 하네스를 **내부 LLM만** 바꾸는 것은 provider 교체가 아니라 백엔드 선택이다
(사용자 지시: 서브 위임은 `kimi-claude` = 오케스트레이터-워커에서 워커의 이너 하네스 LLM이 Kimi K3).
request 의 선택 필드 `backend`(`anthropic`|`kimi`, 기본 anthropic)가 어댑터의 닫힌 열거
`BACKEND_TO_BINARY` 를 통해 실행 바이너리를 고른다: `anthropic → claude` · `kimi → kimi-claude`.

**2026-09-08 확장 — 백엔드 3종 + 러너 사다리.** `minimax` 가 더해져 열거는
`anthropic|kimi|minimax` 이고, 별칭 표 `RUNNER_ALIASES` 가 짧은 이름을 **(backend, model) 쌍**으로
편다(`opus`·`sonnet`·`haiku` → anthropic · `kimi-claude` → kimi/`k3[1m]` · `minimax-claude` →
minimax/`MiniMax-M3`). 백엔드별 기본 모델은 `BACKEND_DEFAULT_MODEL` 이 단일 소유한다 — 종전
`k3[1m]` 리터럴이 `bootstrap_canary` 에 손으로 적혀 있던 것을 걷어냈다. 중립 통로는
`agent_control.py runners` 이며 relay 는 이것을 읽고 자기 사본을 두지 않는다.
회전·소진의 정본은 `SKILL.md` §2.7.11 이다.

**러너 평면 실패 판정**(`classify_runner_failure`) — 이 파일이 소유한다(provider 어휘). 판정은
문구가 아니라 **구조**를 읽는다: `terminal_reason == "api_error"` 가 1차 신호이고
`api_error_status`(HTTP)가 회전 여부를 가른다. ★ 실패 봉투도 `subtype == "success"` 이므로 봉투
형태로는 갈리지 않는다(2026-09-08 실측). rc 127 = 바이너리 부재(회전 ○) · rc 255 = ssh 전송
실패(회전 ✗). 결과는 `RUNNER_UNAVAILABLE` 이며 증거는 `output` 에 JSON 으로 실린다(스키마를
넓히지 않는다 — PERMISSION_DENIED 선례). **모르면 회전하지 않는다.**

**2026-09-15 — 백엔드 4종.** `meta`(`meta-claude` shim · 기본 모델 `muse-spark-1.3`)가 더해졌다. 백엔드
어휘는 바이너리 표·기본모델 표·요청 스키마 enum 세 자리에 있고 `runtime_selftest` 가 셋의 일치를
교차검증한다. `relay.py`·`bootstrap_canary.py` 의 `--backend` 선택지는 스키마 enum 에서 읽는다.
⚠ shim 은 auto mode 판정 호출까지 자기 모델로 보낸다 — 대화형 세션을 shim 으로 띄울 때는 auto mode 를
쓰지 않는다(판정 불가 오류는 권한 거부가 아니다 · 정본 `SKILL.md` §2.7.11).

- `kimi-claude`·`minimax-claude`·`meta-claude` 는 **노드 로컬 shim**(`~/.local/bin/<name>`, 비추적)이다 — .bashrc 의 셸 함수는
  ssh 비대화형(`bash -lc`)에서 로드되지 않으므로 실행 파일 형태가 계약이다 — 2026-09-08 양 노드
  실측으로 이유가 **둘**임이 확인됐다: ⓐ `.bashrc` 가 `case $- in *i*)` 로 조기 return 한다
  ⓑ 어댑터가 감싸는 `timeout <n> <cmd>` 는 셸 함수를 exec 할 수 없다(둘은 독립이라 한쪽만 고쳐도
  여전히 안 된다). env 라우팅(엔드포인트·
  API 키·모델 슬롯)은 shim 과 `~/.kimi-claude.env`(0600)가 소유하며, **이 저장소의 추적 파일에는
  키·엔드포인트를 적지 않는다**.
- backend 별 model 기본 선언은 어댑터의 `BACKEND_DEFAULT_MODEL` 이 준다(선언일 뿐이며, 실제 실행 모델은
  결과의 `model_used` 가 기록한다 — G-A1 계약 불변).
- shim 부재는 provider 실행 실패(`NONZERO_EXIT` + stderr 진단)로 표면화된다 — 조용한 anthropic
  폴백은 없다(폴백이면 어느 모델이 돌았는지가 섞인다).

- main은 explicit `work_dir`에서 local subprocess로 실행한다.
- sub는 `ssh -- <user@host> 'cd <quoted-work_dir> && CLAUDE_CODE_RETRY_WATCHDOG=1 timeout <N> <quoted-provider-command>'`로 실행한다.
  env 대입은 **`timeout` 앞**에 온다(뒤에 두면 `timeout` 이 `VAR=1` 을 실행 파일로 알고 즉사한다 —
  `runtime_selftest` 가 이 순서를 계약으로 고정한다). 자동 재개는 공식적으로 대화형 claude.ai 로그인에만
  있고 `-p`/게이트웨이 경로에는 없다 — 있는 것은 이 재시도 변수뿐이라 서브 위임에만 켠다(2026-09-05).
- `bypassPermissions`와 `--dangerously-skip-permissions`는 어떤 경로에서도 허용하지 않는다.
- `capabilities`는 adapter 내부에서만 `Read`, `Bash`, `Edit`, `Write`로 매핑한다.
- 긴 build/serve는 agent-control에 위임하지 않고 별도 감독 실행계약을 사용한다.

## 3. result와 fail-closed 판정

성공은 provider exit 0만으로 판정하지 않는다. Claude wrapper의 성공 형상과 비어 있지 않은
`result`를 확인한다. `modelUsage`는 관측 기록이며 Sonnet 전용 제약이 아니다. Codex는 별도 JSONL
파서로 thread/turn/최종 agent_message/프로세스 종료를 함께 확인한다. 다음은 structured nonzero result로 차단한다.

- request/target/bounds/capability 위반
- provider nonzero, timeout, malformed JSON
- wrapper success-shape 또는 provider result schema 위반

stdout은 항상 `agent-control-result.schema.json`의 정렬된 JSON이다. 성공 시 `output`에 agent text를 보존하고,
실패 시 관측된 진단·부분 응답을 보존할 수 있다. 프로세스 성공은 task-report나 도메인 완료의 증거를 대신하지 않는다.

### 3.1 Codex adapter (v2)

`scripts/providers/codex_cli.py`가 `codex exec` 구문의 유일한 발행처다. 선택적 `reasoning_effort`는 Codex 요청에만 허용되며 명시 시 `-c model_reasoning_effort="<값>"`로 전달한다. 생략 시 override는 없다. relay의 명시 값은 request와 재개 선택에 보존된다; 실제 모델 관측은 별도다. runner의 정체성은
`provider × backend × model`이며 `backend=openai`인 Claude shim과 Codex는 다른 provider다.
`agent_control.py runners --provider all`로 전체 별칭을 읽는다. 이번 명시적 선택은
`codex-gpt-6.1-sol`이고 기존 기본 사다리는 바꾸지 않는다.

- 예산: `max_turns: null`, `budget_unit: wall_time`, 한 번의 실행에 대한 유한한 `timeout_seconds`를
  선언한다. 새 요청에 누적 `max_dispatches`를 싣지 않으며 과거 필드는 호환 관측으로만 읽는다. Codex의 완료 이벤트를
  Claude native turn으로 세지 않는다. 시간 한도는 토큰·금액의 하드 한도가 아니다.
- 권한: `sandbox_mode` 또는 `permission_profile` 중 하나. 읽기 전용 카나리는 `read-only`,
  운영 위임은 검증된 `vllm-main`/`vllm-sub` named profile을 사용한다. named profile에서는
  legacy sandbox 옵션을 붙이지 않는다. 미설치·미신뢰 profile을 다른 권한 모드로 대체하지 않는다.
- 재개: `resume_session_id`와 정확한 `{provider,target,permission selector}` 결속을 함께 보낸다.
  provider가 바뀌면 native session은 새로 시작하고 원장·리포트·아티팩트를 전달한다.
- 관측: 실제 보고된 값만 `model_used`, `usage`, `raw_exit_code`, `execution_started`,
  `raw_events`에 남긴다. 보고하지 않은 native 턴·API 시간·모델은 null/빈 배열이다.
- 실패: 잘린 JSONL, 빈 최종 메시지, native failure, 권한 거부, timeout을 성공으로 바꾸지 않는다.
  미지 오류·권한 거부·효과가 발생했을 수 있는 실패는 자동 회전하지 않는다.

정상 워커 예산 소진과 과업 종료는 다르다. adapter가 실제 deadline 집행과 워커 종료·회수를
요청에 결속해 관측한 경우 체크포인트 재개를 허용한다. 단순 exit124나 원격 SSH 감시 timeout은
그 증명이 아니다. 재개 워커는 남아 있는 명령·서비스·부분 산출물을 먼저 확인하며 성공이나 중복
실행을 추론하지 않는다. 이 계약은 메인 자체가 상위 오케스트레이터의 워커인 경우에도 같다.

`agent_control.py invoke --request <file> --result-file <unique-path> --dispatch-id <id>`는
선택적 provider-neutral 영속 출력 계약이다. 두 추가 옵션은 함께 사용한다. schema 검증을 마친
`{schema_version:1,dispatch_id,request,result}`를 고유 파일에 flush·fsync·원자적 publish하고 나서
stdout을 낸다. 기존 파일은 덮어쓰지 않는다. relay는 이 경로와 요청 바이트 digest를 **실행 전**에
예약하며 정상 반환도 영속본과 대조한다. stdout이 끊겼으면 relay의 `--reconcile-inflight`가
저장된 결과를 검증해 1회만 소비한다. 인증 파일이나 환경 전체를 수집하는 인터페이스는 아니다.
- SSH: manifest의 명시적 계정·경로, 기존 host-key 검증, 원격 deadline과 종료 유예를 사용한다.
  로컬 프로세스 종료만으로 원격 작업이 정리됐다고 주장하지 않는다. 실제 종료 카나리가 필요하다.

참조한 seed는 `hermes-subagent-control-plane`의 `codex-cli-control` 브랜치다. 작업공간 결속과
실제 CLI 호출·독립 검증 원칙을 적용한다. seed의 Agent Card 스키마나 Hermes 등급표를 프로젝트의
정체성·예산 정본에 덧씌우지 않으며, 손상된 JSONL을 일반 텍스트로 통과시키는 폴백도 채택하지 않는다.

공식 실행 계약: [Codex non-interactive mode](https://learn.chatgpt.com/docs/non-interactive-mode).
권한 계약: [Codex permissions](https://learn.chatgpt.com/docs/permissions).

## 4. 다른 provider를 붙일 때

동일 provider-neutral request/result를 구현하는 adapter만 추가한다. public skill 본문은 수정하지 않는다.
placeholder adapter는 만들지 않는다. 현재 구현된 provider는 Claude Code와 native Codex CLI다.

Codex의 정확한 구조화 capacity 오류는 `provider_capacity`로 관측하며 실패 상태를 유지한다. 다른 오류나 명령 출력으로부터 용량을 추정하지 않는다. 제한된 선택적 재시도는 relay가 별도 dispatch로 발행한다.
