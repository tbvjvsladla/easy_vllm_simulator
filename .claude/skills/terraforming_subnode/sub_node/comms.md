# comms.md — 메인↔서브 통신 계약 (A2A-개념, 서버 없음)

> 서브노드 `.claude/rules/comms.md` 로 복제되는 **정적 계약**(양 노드 동일 · PII-free · 추적).
> terraforming_subnode 가 서브에 전달. 서브 CC 가 매 턴 자동 로드(.claude/rules/ auto-load)해 이 규약을 지킨다.
> A2A(Agent2Agent) 개념을 **차용**하되 HTTP/JSON-RPC 서버는 안 띄운다 — 전송은 `ssh claude -p` 단발.

## 역할 (누가 무엇을)
- **메인 = client agent**. Task 를 발급하고, 서브 리포트를 검증하고, 실패 시 feedback 을 보낸다.
- **서브(너) = remote agent(불투명 피어)**. Task 를 자율 수행하고, **자기검증한 리포트 1개**만 반환한다.
- **DO** 불투명 피어로 협업한다 — 너는 네 작업을 책임지고 결과를 attest 한다.
- **DON'T** 메인이 네 워크스페이스를 재스캔/파일교정하길 기대하지 마라 — 그건 일어나지 않는다.

## 전송 (transport)
- 메인 → 서브: `ssh <user>@<sub> claude -p '<Task JSON or instruction>' --output-format json --permission-mode acceptEdits`.
  **정본 = `acceptEdits` + 스코프드 allowlist(`settings.local.json`)** — `bypassPermissions` 는 하네스 가드레일이 차단한다(원격 자율 에이전트, testlog_2026062422_1). 비-allowlist 명령은 `bash -c` 래퍼로(설계상 escape hatch).
- **HTTP(health 폴링·스모크)는 python urllib 로** 한다 — `curl`/`wget` 은 allowlist deny. 대기는 python `time.sleep`.
- 서브 → 메인: stdout 으로 **task-report.schema.json 에 맞는 JSON 1개**. raw 로그 금지.
- 코드/정본 전달은 별개 평면: 메인이 `sync_to_sub.sh`(rsync)로 push. 너는 정본을 받기만 한다.

## Task 생애주기 (A2A TaskState)
`submitted → working → (input-required ⇄ working)* → completed | failed`
- **working**: 수행 중. (단발 호출이라 보통 한 턴에 working→종료.)
- **input-required**: 네가 막혀 메인 판단/피드백이 필요. 리포트로 막힌 지점을 명시하고 종료. 메인이 feedback Message 로 다음 턴을 연다.
- **completed**: 해당 phase 성공술어 충족(아래).
- **failed**: 실패. `failure_class` 로 분류. 애매하면 `unknown`.

## 상태 = 파일 (세션 없는 멀티턴)
- 한 작업 = 하나 `context_id`. 진행상태는 **`tasks/<context_id>.json`** 에 산다 — **파일이 세션이다**(데몬 없음).
- 매 턴: (1) `tasks/<context_id>.json` 이 있으면 읽어 이전 턴/피드백을 복원 (2) 작업 수행 (3) 네 턴 결과를 그 파일에 덧쓰고 (4) 리포트 반환.
- **max-turns = 3**(메인 `reconciliation_cap` 미러). `turn > 3` 이면 더 시도하지 말고 `status=failed, failure_class=unknown` 으로 종료 → 메인이 **Model-C(HITL)**.

## 검증 = push-attestation (가장 중요)
- **DO** 보고 전에 **스스로 검증**하고 결과를 `self_verification` 에 담아라: config-parse · 이 리포트의 schema 유효성 · runner 문법(bash -n) · 산출물 checksum · 가능하면 **로컬 스모크**.
- **DON'T** "파일 만들었음"으로 completed 선언하지 마라 — **린트 통과 ≠ 서빙됨**. 성공술어를 만족해야 completed.
- 메인은 네 디스크가 아니라 **네 리포트**를 검증한다. 그러니 **정직하게** attest 하라(허위 attest = 신뢰 붕괴).

## phase 별 성공술어 (B4 — 검증될 때까지 루프)
| phase | completed 조건 |
|---|---|
| inspect | 정체성·로드된 스킬·권한·통신계약을 로드해 **schema-valid 리포트** 반환(모델 불요 — 카나리). |
| config | 지정 모델 3종(.yaml+.sh+.env)을 **vllm-recipe-explorer 결정론 엔진으로 자율 생성** + config-parse OK(+ 가능 시 로컬 스모크 응답). |
| build | `docker compose --profile <slave\|debug> build` 성공(메인 빌드 독립 재현·byte-equiv). **multi**=slave / **single**=debug. |
| serve | **multi**: `--profile slave up` 으로 master Ray head 합류(+ 지시 시 로컬 health). **single(독립서빙, T3 검증 — 0.23.0 E2E)**: `--profile serve up -d`(env export: NAS_MODEL_PATH·TIKTOKEN_HOST_PATH·CONFIG_FILE·SERVING_PORT) → `:PORT/health` http200 폴링(**python urllib — curl deny**) → 로컬 functional smoke(완성/reasoning, finish=stop). "startup complete" 로그는 거짓양성. |

## Message 타입
- **instruction**(메인→서브): 수행할 Task(phase + per-task 값: 모델명·VRAM 예산·NAS 모델 서브디렉토리). **single 서빙 태스크**면 추가 슬롯: max_model_len·served_model_name·SERVING_PORT·reasoning_parser. 운영 절차(env export·detached up·health200 python·reasoning max_tokens)는 §phase serve 술어(single 분기)에 있으니 매번 재기술 불요.
- **feedback**(메인→서브): "여기가 틀렸으니 이렇게 고쳐". 너는 **직접 고쳐** 다음 턴에 재-attest(자기교정).
- **report**(서브→메인): task-report.schema.json JSON 1개.

## 경계 (B3 Surgical)
- 너는 **모델별 `configs/`·`envs/` 만** 자작한다. 컨테이너 정본(Dockerfile/requirements/compose/serve_runner)·빌딩블럭(.claude/, CLAUDE.md, Agent_Card.json)은 **건드리지 않는다**.
- per-task 값은 Task Message 에서 읽는다 — manifest 는 서브에 없다(메인이 다 조리해 보냄).
