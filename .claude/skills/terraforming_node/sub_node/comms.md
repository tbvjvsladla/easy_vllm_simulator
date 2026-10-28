# comms.md — 메인↔서브 통신 계약 (A2A-개념, 서버 없음)

> 서브노드 `.claude/rules/comms.md` 로 복제되는 **정적 계약**(양 노드 동일 · PII-free · 추적).
> terraforming_node 가 서브에 전달. 각 하네스의 진입문서가 이 규약을 연결한다.
> A2A(Agent2Agent) 개념을 **차용**하되 HTTP/JSON-RPC 서버는 안 띄운다 — 전송은 SSH, 실행은 명시적으로 선택한 provider다.

## 역할 (누가 무엇을)
- **메인 = client agent**. Task 를 발급하고, 서브 리포트를 검증하고, 실패 시 feedback 을 보낸다.
- **서브(너) = remote agent(불투명 피어)**. Task 를 자율 수행하고, **자기검증한 리포트 1개**만 반환한다.
- **DO** 불투명 피어로 협업한다 — 너는 네 작업을 책임지고 결과를 attest 한다.
- **DON'T** 메인이 네 워크스페이스를 재스캔/파일교정하길 기대하지 마라 — 그건 일어나지 않는다.

## 전송 (transport)
- 메인 → 서브: provider-neutral 요청을 메인 `agent_control` adapter가 SSH·native CLI로 변환한다.
  Claude Code/claude-sub와 Codex는 같은 역할·업무·승인 계약을 따르되 도구 승인·OS sandbox는 다른 집행 표면이다.
  실제 호출 옵션이 정본이며 과거 문서의 permission mode 예제를 현재 실행으로 간주하지 않는다.
  권한 부족은 `input-required` 리포트로 돌린다. 셸 래퍼·다른 provider·권한 완화로 우회하지 않는다.
- **직접 작성하는 HTTP health·스모크 점검은 python urllib 로** 한다. 직접 `curl`/`wget` 호출의 deny는 유지한다. 승인된 정본 서빙·벤치 스크립트가 선언된 서비스 endpoint에 수행하는 내부 health·metrics·추론 HTTP 호출은 그 업무의 구현으로 허용한다. 임의 래퍼·다운로드·endpoint 변경으로 제한을 우회하지 않으며, 실제 도구 거부는 그대로 보고한다. 직접 작성하는 대기는 python `time.sleep`을 사용한다.
- 서브 → 메인: stdout 으로 **task-report.schema.json 에 맞는 JSON 1개**. raw 로그 금지.
- 코드/정본 전달은 별개 평면: 메인이 `sync_to_sub.sh`(rsync)로 push. 너는 정본을 받기만 한다.

## Task 생애주기 (A2A TaskState)
`submitted → working → (input-required ⇄ working)* → completed | failed`
- **working**: 수행 중. (단발 호출이라 보통 한 턴에 working→종료.)
- **input-required**: 네가 막혀 메인 판단/피드백이 필요. 리포트로 막힌 지점을 명시하고 종료. 메인이 feedback Message 로 다음 턴을 연다.
- **completed**: 해당 phase 성공술어 충족(아래).
- **failed**: 실패. `failure_class` 로 분류. 애매하면 `unknown`.

## 상태 = 파일 (세션 없는 멀티턴)
- 한 작업 = 하나 `context_id`. 진행상태는 **`campaigns/<camp-id>/relay/<context_id>.json`** 에 산다 —
  **파일이 세션이다**(데몬 없음). 활성 캠페인을 모르면 `_bootstrap` 이며 루트에 쓰지 않는다(옛 `tasks/` 폐기 · 2026-09-06).
- 매 턴: (1) `campaigns/<camp-id>/relay/<context_id>.json` 이 있으면 읽어 이전 턴/피드백을 복원 (2) 작업 수행 (3) 네 턴 결과를 그 파일에 덧쓰고 (4) 리포트 반환.
- **실행 예산은 메인이 매 attempt 선언한다 — 고정값도 등급표도 없다**(2026-09-05 개정 · `plan_26090516` ③).
  위임 헤더가 native-turn 또는 한 번의 wall-time 예산과 **그 예산을 그렇게 정한 근거**를 함께 준다. 누적 호출 횟수는 과업의 상한이 아니다. 등급 어휘(S·L0~L4)는
  폐기됐다 — 표가 실측 없이 정본 행세를 했고 교정하는 코드가 없었다. 필요하다고 보는 예산이 있으면
  리포트 `budget_recommendation`에 해당 하네스가 지원하는 단위와 근거로 제안하라(배정은 메인이 한다).
  예산은 비용 노브이지 hang 노브가 아니다 — 매달림은 `timeout_seconds` 가 잡는다(scope ⊥ budget).
- **한 번의 실행 예산 소진은 과업 종료가 아니다.** 그 실행을 끝내기 전에 가능한 체크포인트·부분 산출물·진행 중 명령과 서비스를 리포트에 남긴다. 정상 인계는 `status=input-required`, `budget_outcome=exhausted`로 보고하며 완료나 권한 요청으로 꾸미지 않는다. native-turn 사용량은 실제 관측됐을 때만 적고 Codex 완료 이벤트를 Claude 턴으로 세지 않는다.
  상위 오케스트레이터는 같은 승인 범위에서 남은 작업·동일 단계 실측·소진 원인·서브의 근거 있는 제안으로 다음 실행 예산을 동적으로 편성하고 체크포인트를 이어받는다. 같은 값은 그 판단 결과가 같은 경우 가능하며 고정 반복 규칙이 아니다. 작업을 처음부터 반복하지 않는다. 메인 자체가 워커일 때도 같다. 호출 횟수만으로 중단하거나 증액 승인을 요구하지 않는다. 부분 산출물은 `artifacts`에 남기고 다음 워커는 기존 명령·서비스가 살아 있는지 먼저 확인한다.
- **답을 기다려야 하면 실패가 아니라 `input-required`** 다. `hitl.needed=true` + `hitl.request_id` 를 담아
  끝내라 — 메인이 답을 실어 **같은 provider·대상·권한에 결속된 세션을 재개**한다. 하네스를 바꾸면 새 native 세션에서 기존 아티팩트를 잇는다.
- **근거가 필요하면 도서관을 직접 조회하라** — 도서관(`__llm-wiki`)은 메인에 하나지만 네 wiki-desk 는 메인과
  **같은 CLI** 로 그 서고에 닿는다(경로만 다르다 · 아래 §도서관). 메인에 물어 달라고 턴을 끝내지 마라.
  열린 역방향 통로는 **사서 게이트웨이 하나**다 — 그 밖의 메인 접속(ssh 셸·scp·rsync 직접)은 시도하지 마라.
  네가 메인에게 말하는 통로는 여전히 **리포트와 네 `docs/`** 뿐이다.
- **사람의 답이 있어야만 진행할 수 있으면 `hitl.needed:true`** 로 끝내라. 그 답 없이 결정하지 않을 때만
  `hitl.blocking:true` 를 단다 — 메인은 대기 요청을 차단성 우선으로 처리한다. 정말 막혔을 때만 달아라.
- **메인이 답하면 다음 턴 Task 머리에 `## 이어받기` 블록이 온다** — 직전 턴의 산출물·남긴 다음 단계·
  네 질문에 대한 답이 거기 있다. 그 블록은 메인이 **원장에서 그대로 옮긴 것**이지 새로 쓴 서사가
  아니다. 같은 일을 처음부터 다시 하지 말고 그 지점부터 이어라.
<!-- MODE:a2a-agent -->
- **외부지식은 네가 직접 검색한다**(singleton `a2a-agent` 모드 · 2026-09-04). 웹 도구(`WebSearch`·
  `WebFetch`)가 열려 있다. 검색했으면 **반드시** 리포트의 `external_search[]` 에 `query`·`sources`·
  `finding`·`used_for`·`accepted` 를 남겨라 — 인용 없는 결정은 거짓이 아니라 **누락**이고(불변식 B),
  그 기록이 메인의 자산이 된다. 웹은 *바깥* 지식이고 도서관은 *이 프로젝트가 쌓은* 지식이다 —
  둘을 섞지 마라. 외부 자료 획득을 위한 직접 `curl`/`wget` 호출과 임의 래퍼 우회는 금지다(위 §전송의 정본 서비스 점검과 다른 평면).
<!-- /MODE:a2a-agent -->

## 도서관 — 메인 서고를 직접 조회·입고한다 (헌법 불변식 B · policy:LIBRARY_GROUNDING_FAIL_CLOSED C5·C6)

**인용 없는 결정은 거짓이 아니라 누락이다.** 결정 전에 도서관을 조회하고, 쓴 근거를 리포트·문서에 `path:시작-끝` 으로 적어라.
네 manifest `library` 블록(transport: ssh)이 접근자이며, 명령은 메인과 같다(한 줄에 하나씩 · 셸 변수·`cd &&` 로 묶지 마라):

```bash
node .claude/skills/wiki-desk/src/cli.ts query --terms "<핵심어>" --terms "<동의어·영문>"   # 후보 + 근거 span
node .claude/skills/wiki-desk/src/cli.ts read <경로>:<시작>-<끝>                        # 근거 구간 원문
node .claude/skills/wiki-desk/src/cli.ts tree <경로>                                    # 헤딩 구조
node .claude/skills/wiki-desk/src/cli.ts sync --apply                                   # 입고(네 docs 를 메인 서고에)
```

- 결과의 경로는 **메인 저장소 기준**이다. 그 경로를 네 디스크에서 열려고 하지 마라 — `read` 로 연다. 반출은 질의 결과뿐이고
  네 디스크에 사본을 만들지 않는다(복제 ✗).
- 결과가 0건이면 도서관에 근거가 **없는** 것이다. 결정을 밀어붙이지 말고 그 사실을 적은 뒤 진행하라 — **정직한 공백은 실패가 아니다.**
- 입고(`sync --apply`)는 발행 도구(evidence_publisher finalize·campaign_init)가 **자동으로** 부른다. 서고에 닿지 못하면
  exit 4 · `not-yet-ingested` 가 나온다 — 발행은 그대로 성공이고 다음 입고가 따라잡는다. 우회 경로를 만들지 마라.
- 쓰기는 네 미러(메인 `sync_staging/library_mirror/<node_id>/`)의 카드만이다. 남의 카드·메인 원문 카드는 게이트웨이가 거부한다.
- 게이트웨이가 거부(exit 126)하면 그것은 공백이 아니라 **거절**이다 — 우회하지 말고 리포트에 그대로 적어라.

## 검증 = push-attestation (가장 중요)
- **DO** 보고 전에 **스스로 검증**하고 결과를 `self_verification` 에 담아라: config-parse · 이 리포트의 schema 유효성 · runner 문법(bash -n) · 가능하면 **로컬 스모크**.
- **DON'T** "파일 만들었음"으로 completed 선언하지 마라 — **린트 통과 ≠ 서빙됨**. 성공술어를 만족해야 completed.
- 메인은 네 디스크가 아니라 **네 리포트**를 검증한다. 그러니 **정직하게** attest 하라(허위 attest = 신뢰 붕괴).

## 서브 준비성 — manifest/Flag/topology (자격증명·면제 없음)
- SSH public-key 관계가 메인이 프로비저닝한 endpoint를 인증한다. 실행 준비성은 배달된 서브 manifest의 `self_role: sub`·`terraforming.complete`·`branch_verified`·`issued_by: main`과 topology 계약으로 fail-closed 판정한다.
- `Agent_Card.json`은 메인이 배달한 unsigned capability/discovery metadata다. 직접 고치지 마라. 부재·손상은 capability discovery 결손으로 `status=input-required`에 보고하되 실행 게이트의 면제가 되거나 실행을 막지 않는다.
- 리포트의 `self_verification.identity_ok`에는 manifest Flag와 topology 계약을 대조한 결과를 기록한다.

## phase 별 성공술어 (B4 — 검증될 때까지 루프)
| phase | completed 조건 |
|---|---|
| inspect | 정체성·로드된 스킬·권한·통신계약을 로드해 **schema-valid 리포트** 반환(모델 불요 — 카나리). |
<!-- MODE:a2a-agent -->
| config | 지정 모델 3종(.yaml+.sh+.env)을 **vllm-recipe-explorer 결정론 엔진으로 자율 생성** + config-parse OK(+ 가능 시 로컬 스모크 응답). |
| build | `docker compose --profile debug build` 성공(메인 빌드 독립 재현). |
| serve | **독립서빙(T3 검증 — 0.23.0 E2E)**: `--profile serve up -d`(env export: NAS_MODEL_PATH·TIKTOKEN_HOST_PATH·CONFIG_FILE·SERVING_PORT) → `:PORT/health` http200 폴링(정본 스크립트 내부 호출 또는 직접 점검 시 python urllib — §전송) → 로컬 functional smoke(완성/reasoning, finish=stop). "startup complete" 로그는 거짓양성. |
<!-- /MODE:a2a-agent -->
<!-- MODE:ray-worker -->
| config | 저작하지 않는다 — 받은 빌드킷과 주입된 env 로 합류만 한다(트리플렛은 서브로 전파되지 않는다). |
| build | `docker compose --profile slave build` 성공(메인 빌드 독립 재현). |
| serve | `--profile slave up` 으로 master Ray head 합류(+ 지시 시 로컬 health). |
<!-- /MODE:ray-worker -->

## Message 타입
- **instruction**(메인→서브): 수행할 Task(phase + per-task 값: 모델명·VRAM 예산·NAS 모델 서브디렉토리). **single 서빙 태스크**면 추가 슬롯: max_model_len·served_model_name·SERVING_PORT·reasoning_parser. 운영 절차(env export·detached up·health200·reasoning max_tokens)는 §phase serve 술어(single 분기)에 있으니 매번 재기술 불요. **신규 vLLM build-job**은 승인된 패킷과 manifest/Flag/topology 준비성이 모두 확인된 경우에만 전파한다.
- **feedback**(메인→서브): "여기가 틀렸으니 이렇게 고쳐". 너는 **직접 고쳐** 다음 턴에 재-attest(자기교정).
- **report**(서브→메인): task-report.schema.json JSON 1개.

## 경계 (B3 Surgical)
<!-- MODE:a2a-agent -->
- 너는 **자기 빌드킷과 모델별 `configs/`·`envs/`를** 해당 owner 스킬로 저작한다. 배달된 공통 runner·빌딩블럭(.claude/, CLAUDE.md, AGENTS.md, .agents/, .codex/, Agent_Card.json)은 **건드리지 않는다**.
<!-- /MODE:a2a-agent -->
<!-- MODE:ray-worker -->
- 너는 서빙 설정을 자작하지 않는다 — `configs/`·`envs/` 도 메인이 배달한다. 컨테이너 정본(Dockerfile/requirements/compose/serve_runner)·빌딩블럭(.claude/, CLAUDE.md, Agent_Card.json)은 **건드리지 않는다**.
<!-- /MODE:ray-worker -->
- HW 사실·경로·획득 모드와 노드 role은 **이 노드의 `output/<topology>/manifest.yaml`** 에서 읽는다 — 메인 terraforming 이 `--peer-ssh` 로 너를 실측해 발급·배달한 **서브 manifest**(`self_role: sub` · `terraforming.issued_by: main`)다. 너는 이 파일을 손으로 고치지 않는다(권위는 메인 스캔 · 재발급은 메인 `scan_node.py --emit-sub-manifest`). per-task 값(모델명·VRAM 예산·NAS 서브디렉토리)은 Task Message 에서 읽는다. SSH는 endpoint 인증만 담당하며, Flag와 topology 계약이 실행 준비성을 결정한다.
