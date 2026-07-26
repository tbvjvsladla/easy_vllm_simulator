# agent-control adapter — provider 전용 실행문법 경계 (조건부 reference)

> **이 파일이 유일한 provider-specific 평면이다.** 5개 SKILL.md 본문·다른 reference 는 "서브 코드에이전트를
> 호출한다"는 **의도**만 적고, 실제 CLI 문법(provider 이름·플래그·권한모드)은 여기서만 해소한다.
> plan_26072506 Phase 5 §"Claude 전용 명령이 adapter 외부에 남지 않음" · Phase 6 이 이 경계를
> `scripts/agent_control.py` + `scripts/providers/claude_code.py` 로 코드화한다(스키마·result metadata 검증).
> 계약 테스트 = `tests/harness/test_skill_contracts.py::TestClaudeCommandsIsolatedToAdapter`.

## 1. provider-neutral 의도 (본문이 쓰는 어휘)

| 의도 | 뜻 | 성공 판정 |
|---|---|---|
| `delegate(task)` | 서브 노드 코드에이전트에 Task 1개를 비대화로 위임 | `task-report.schema.json` 유효 JSON 1개 회신 |
| `probe(reachable)` | 서브 코드에이전트가 모델에 실제 도달하는지 확인 | 위임 1회가 오류 없이 리포트 반환 |
| `bootstrap_canary()` | 모델 없이 환경만 검증하는 라운드트립 | `phase=inspect · status=completed` + `self_verification` |

## 2. Claude Code adapter (현재 유일 구현)

전송 = SSH 단발 호출(HTTP/JSON-RPC 서버 없음). 정본 권한모드 = `acceptEdits` + 스코프드 allowlist
(`.claude/settings.local.json` — 렌더 산출물). `bypassPermissions` 는 하네스 가드레일이 차단한다(testlog_26062422).

```bash
# delegate / probe — 한 턴 = JSON 리포트 1개
ssh <ssh_user>@<sub_host> claude -p '<Task JSON or instruction>' --output-format json

# bootstrap canary (§2.5) — 모델 없이 환경만
ssh <sub_host> claude -p '<inspect bootstrap>' --output-format json
```

- **`cd <work_dir>` 선행 필수**: 서브 워크스페이스에서 실행해야 페르소나(`CLAUDE.md`)·런타임블럭이 로드된다.
  루트 밖에서 부르면 페르소나 미로드로 주입-거부가 난다.
- **login shell PATH**: 긴 작업은 `ssh <sub> 'bash -lc "..."'` 로 감싼다. **긴 빌드는 위임하지 말고**
  SSH 백그라운드로 직접 돌린다(위임 호출은 harness 타임아웃에 걸린다).
- 모델 핀은 서브 `settings.local.json` 이 소유한다(메인이 플래그로 강제하지 않는다 — 렌더 시 결정).

## 3. 다른 provider 를 붙일 때

같은 §1 의도 3개를 만족하는 어댑터를 추가하고, 본문(SKILL.md)은 손대지 않는다. placeholder 어댑터는
만들지 않는다(Phase 6 범위: Claude adapter 하나만).
