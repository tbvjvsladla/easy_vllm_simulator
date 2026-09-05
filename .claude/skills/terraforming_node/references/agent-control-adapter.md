# agent-control adapter — provider 전용 실행문법 경계 (조건부 reference)

> **이 파일과 `scripts/providers/`만 provider-specific 평면이다.** 5개 public SKILL 본문과 다른 reference는
> 위임 의도만 선언한다. 실행은 provider-neutral `.claude/policies/runtime/agent_control.py`가 request schema를 검증한 뒤
> adapter로 라우팅한다. 정본 계약: `agent-control-request.schema.json`, `agent-control-result.schema.json`.

## 1. provider-neutral request

필수 필드:

- `schema_version: 1`
- `provider: "claude_code"`
- `intent: delegate | probe | bootstrap_canary`
- `model: "sonnet"`
- `task`: 비어 있지 않은 instruction
- `target`: `main/local` 또는 `sub/ssh`, 명시적 `work_dir`; SSH는 `host` 필수
- `timeout_seconds`, `max_turns`: 유한한 실행 경계
- `capabilities`: provider-neutral least-privilege 목록(`read`, `execute`, `edit`, `write`)

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
python3 .claude/policies/runtime/agent_control.py invoke --request /path/to/request.json
```

## 2. Claude Code adapter (현재 유일 구현)

`claude_code.py`만 generic capability를 Claude tool 이름으로 변환하고 다음 provider argv를 생성한다.
이 명령은 구현 계약 설명용이며 운영 호출자는 직접 복제하지 않고 위 orchestrator를 사용한다.

```bash
claude -p '<task>' --model sonnet --output-format json --max-turns <N> --allowedTools <TOOLS>
```

- main은 explicit `work_dir`에서 local subprocess로 실행한다.
- sub는 `ssh -- <user@host> 'cd <quoted-work_dir> && CLAUDE_CODE_RETRY_WATCHDOG=1 timeout <N> <quoted-provider-command>'`로 실행한다.
  env 대입은 **`timeout` 앞**에 온다(뒤에 두면 `timeout` 이 `VAR=1` 을 실행 파일로 알고 즉사한다 —
  `runtime_selftest` 가 이 순서를 계약으로 고정한다). 자동 재개는 공식적으로 대화형 claude.ai 로그인에만
  있고 `-p`/게이트웨이 경로에는 없다 — 있는 것은 이 재시도 변수뿐이라 서브 위임에만 켠다(2026-09-05).
- `bypassPermissions`와 `--dangerously-skip-permissions`는 어떤 경로에서도 허용하지 않는다.
- `capabilities`는 adapter 내부에서만 `Read`, `Bash`, `Edit`, `Write`로 매핑한다.
- 긴 build/serve는 agent-control에 위임하지 않고 별도 감독 실행계약을 사용한다.

## 3. result와 fail-closed 판정

성공은 provider exit 0만으로 판정하지 않는다. wrapper가 `is_error=false`, `subtype=success`, 비어 있지 않은
`result`, Sonnet-only `modelUsage`를 모두 제공해야 한다. 다음은 stable structured nonzero result로 차단한다.

- request/target/bounds/capability 위반
- non-Sonnet 요청 또는 Opus/unknown/mixed model metadata
- model metadata 누락
- provider nonzero, timeout, malformed JSON
- wrapper success-shape 또는 provider result schema 위반

stdout은 항상 `agent-control-result.schema.json`의 정렬된 JSON이다. 성공 시 `output`에 agent text를 보존하고,
실패 시 `output=null`이다.

## 4. 다른 provider를 붙일 때

동일 provider-neutral request/result를 구현하는 adapter만 추가한다. public skill 본문은 수정하지 않는다.
placeholder adapter는 만들지 않는다(Phase 6 범위는 Claude Code adapter 하나뿐이다).
