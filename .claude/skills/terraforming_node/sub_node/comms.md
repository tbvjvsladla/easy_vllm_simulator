# comms.md — 메인↔서브 통신 계약 (A2A-개념, 서버 없음)

> 서브노드 `.claude/rules/comms.md` 로 복제되는 **정적 계약**(양 노드 동일 · PII-free · 추적).
> terraforming_node 가 서브에 전달. 서브 CC 가 매 턴 자동 로드(.claude/rules/ auto-load)해 이 규약을 지킨다.
> A2A(Agent2Agent) 개념을 **차용**하되 HTTP/JSON-RPC 서버는 안 띄운다 — 전송은 `ssh claude -p` 단발.

## 역할 (누가 무엇을)
- **메인 = client agent**. Task 를 발급하고, 서브 리포트를 검증하고, 실패 시 feedback 을 보낸다.
- **서브(너) = remote agent(불투명 피어)**. Task 를 자율 수행하고, **자기검증한 리포트 1개**만 반환한다.
- **DO** 불투명 피어로 협업한다 — 너는 네 작업을 책임지고 결과를 attest 한다.
- **DON'T** 메인이 네 워크스페이스를 재스캔/파일교정하길 기대하지 마라 — 그건 일어나지 않는다.

## 전송 (transport)
- 메인 → 서브: `ssh <user>@<sub> claude -p '<Task JSON or instruction>' --output-format json --permission-mode acceptEdits`.
  **정본 = `acceptEdits` + 스코프드 allowlist(`settings.local.json`)** — `bypassPermissions` 는 하네스 가드레일이 차단한다(원격 자율 에이전트, testlog_26062422). 비-allowlist 명령은 `bash -c` 래퍼로(설계상 escape hatch).
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
- **턴 예산은 난이도(grade)가 정한다 — 고정 3 이 아니다**(2026-09-03 개정 · `plan_26090317` §5).
  메인이 Task 와 함께 `max_turns_allocated` 를 준다. 정본 표는 메인의 `scripts/turn_budget.py`:
  `S 10 · L0 8 · L1 16 · L2 25 · L3 40 · L4 65`(하한 6). 옛 규약(`max-turns = 3` = `reconciliation_cap`
  미러)은 캠페인 규모 태스크에서 **정상 진행을 실패로 만들었다** — 예산은 비용 노브이지 hang 노브가 아니다.
- **예산을 다 쓰면 그 자리에서 멈춘다(terminal)**. `status=failed` 로 끝내되 `budget_outcome=exhausted`
  와 `max_turns_used` 를 리포트에 담아라. **메인은 같은 예산으로 재시도하지 않고 더 큰 예산의 새 attempt 를
  연다** — 예산을 줄이는 방향은 하강나선이다. 소진 직전 만든 부분 산출물이 있으면 `artifacts` 에 남겨라
  (다음 attempt 가 그것을 이어받는다).
- **답을 기다려야 하면 실패가 아니라 `input-required`** 다. `hitl.needed=true` + `hitl.request_id` 를 담아
  끝내라 — 메인이 답을 실어 **같은 세션을 재개**(`--resume <session_id>`)하므로 컨텍스트를 다시 쌓지 않아도 된다.
- **근거가 필요하면 도서관에 요청하라**(도서관·사서는 메인 단독이다). 역방향 접속을 시도하지 마라 —
  네가 메인에게 말하는 통로는 **리포트와 네 `docs/`** 뿐이다. `blocking:true` 면 그 근거 없이 결정하지 않는다.
- **`blocking` 이 처리 순서를 바꾼다**(2026-09-04). 메인은 대기 요청을 `blocking` 우선으로 처리한다 —
  우선순위는 네 선언에서 나온다. 그러니 **정말 막혔을 때만** 달아라. 막히지 않았는데 달면 그 신호가
  값을 잃고, 진짜 막힌 요청이 뒤로 밀린다. 요청은 `hitl.needed` 없이 `library_request[]` 만 실어도
  메인에 도달한다(종전에는 `hitl.needed` 가 없으면 조용히 사라졌다 — 그 결함은 교정됐다).
- **메인이 답하면 다음 턴 Task 머리에 `## 이어받기` 블록이 온다** — 직전 턴의 산출물·남긴 다음 단계·
  네 질문에 대한 답이 거기 있다. 그 블록은 메인이 **원장에서 그대로 옮긴 것**이지 새로 쓴 서사가
  아니다. 같은 일을 처음부터 다시 하지 말고 그 지점부터 이어라.
- **외부지식은 네가 직접 검색한다**(singleton `a2a-agent` 모드 · 2026-09-04). 웹 도구(`WebSearch`·
  `WebFetch`)가 열려 있다. 검색했으면 **반드시** 리포트의 `external_search[]` 에 `query`·`sources`·
  `finding`·`used_for`·`accepted` 를 남겨라 — 인용 없는 결정은 거짓이 아니라 **누락**이고(불변식 B),
  그 기록이 메인의 자산이 된다. 웹은 *바깥* 지식이고 도서관은 *이 프로젝트가 쌓은* 지식이다 —
  둘을 섞지 마라. `curl`/`wget` 은 여전히 금지다(다른 평면).

## 도서관 교환 — 3메시지 왕복 (헌법 불변식 B · 정본 계약 `.claude/schemas/library-exchange.schema.json`)

**인용 없는 결정은 거짓이 아니라 누락이다.** phase 가 `config`·`build`·`serve` 인 결정을 **채택**하면서
인용이 0이면 메인 판정기가 `GROUNDING_OMISSION` 으로 **수신 자체를 거부**한다. 그러니 결정 전에 물어라.

1. **요청**(너 → 메인). 아래 경로에 `library.citation.request` 를 **네가 쓴다**:
   ```
   docs/library_exchange/<exchange_id>/request.json
   ```
   - `exchange_id` 는 네가 발급한다(예 `lx-<context_id>-01`). 세 메시지가 이 값으로 묶인다.
   - 필수: `schema_version:1` · `kind` · `exchange_id` · `node_id`(= `sub`) · `topology`.
   - `claim` 에 **결정 한 문장**을, `query.terms` 에 **무엇을 찾는지**를 적는다. 도서관을 뒤지지
     마라 — 무엇을 찾는지만 말하는 것이 이 비대칭의 요지다.
   - 리포트에도 `library_request[]` 로 요약을 싣고, `status: input-required` 로 그 턴을 끝낸다.
     **경로는 `artifacts[]` 에 적어라**(메인이 그 경로로 찾아간다).
2. **반출**(메인 → 너). 메인이 사서(wiki-desk)로 해소해 `library.resolution.export` 를 만들고,
   **다음 턴 Task 본문에 그 JSON 을 실어** 보낸다. 별도 채널·파일 배달은 없다.
   - `references[]` 는 **복제가 아니라 참조+발췌**다: `ref_id`·`path`·`digest` 가 본체이고
     `excerpt` 는 읽기 보조다. 그 경로를 열려고 하지 마라 — 네게 없다.
   - `resolution.status` 가 `unresolved` 면 도서관에 근거가 **없는** 것이다. 그때 결정을 밀어붙이지
     말고 `decision.accepted:false` 로 유보하라 — **정직한 유보는 실패가 아니다.**
3. **인증**(너 → 메인). 같은 `exchange_id` 로 아래에 `library.citation.attestation` 을 쓴다:
   ```
   docs/library_exchange/<exchange_id>/attestation.json
   ```
   - `citations[]` 의 각 항목은 `ref_id` + **네가 본 시점의 `digest`** + `reason`(왜 이 참조가 이
     결정을 뒷받침하는가 한 줄). export 의 digest 와 다르면 `CITATION_STALE` 로 거부된다.
   - `decision.accepted` 로 채택 여부를 밝힌다.

**왜 `docs/` 인가**: 상향 회수는 문서기반 only 이고(`fetch_sub_docs.sh` 가 `docs/` 만 미러한다),
그 통로를 그대로 쓴다. 코드·설정을 올려보내는 것이 아니라 **네가 저작한 메시지**를 메인이 읽는 것이다.

## 검증 = push-attestation (가장 중요)
- **DO** 보고 전에 **스스로 검증**하고 결과를 `self_verification` 에 담아라: config-parse · 이 리포트의 schema 유효성 · runner 문법(bash -n) · 가능하면 **로컬 스모크**.
- **DON'T** "파일 만들었음"으로 completed 선언하지 마라 — **린트 통과 ≠ 서빙됨**. 성공술어를 만족해야 completed.
- 메인은 네 디스크가 아니라 **네 리포트**를 검증한다. 그러니 **정직하게** attest 하라(허위 attest = 신뢰 붕괴).

## A2A 위임 — Flag 게이트 면제 (네가 할 일: 없음 / 키를 임의 생성·복구 ✗)
- 메인이 클러스터 HW 스캔 + 메인↔서브 동질성 검증을 통과시키면 너에게 **위임 키** `.claude/a2a_delegation.json` 를 발급·전달한다(메인 키 `terraforming.complete` 와 **UNIQUE**·HW사실 없는 최소 증표).
- `vllm-recipe-explorer`(recipe.py)·`adversarial-benchmark`(run_bench.sh)는 이 키 존재로 테라포밍 Flag 게이트를 **자동 면제**(fail-closed *양성* 키). 너는 아무 env 도 export 할 필요 없다.
- **키를 직접 만들거나 복구하지 마라** — 키는 *메인의 동질성 검증 증표*다(by-design). 키가 없으면 그건 "메인이 아직 검증 안 했다" → `status=input-required` 로 **"A2A 위임 키 부재"** 보고(메인이 `terraforming_node --peer-ssh` 로 검증·재발급). 테스트 한정 override = `EASY_VLLM_A2A_DELEGATED=1`.
- 리포트의 `self_verification.delegation_acknowledged` 로 위임 인지를 echo(A2A 루프 닫음). 헌법 §A2A-위임 Flag 따름정리.

## phase 별 성공술어 (B4 — 검증될 때까지 루프)
| phase | completed 조건 |
|---|---|
| inspect | 정체성·로드된 스킬·권한·통신계약을 로드해 **schema-valid 리포트** 반환(모델 불요 — 카나리). |
| config | 지정 모델 3종(.yaml+.sh+.env)을 **vllm-recipe-explorer 결정론 엔진으로 자율 생성** + config-parse OK(+ 가능 시 로컬 스모크 응답). |
| build | `docker compose --profile <slave\|debug> build` 성공(메인 빌드 독립 재현·byte-equiv). **multi**=slave / **single**=debug. |
| serve | **multi**: `--profile slave up` 으로 master Ray head 합류(+ 지시 시 로컬 health). **single(독립서빙, T3 검증 — 0.23.0 E2E)**: `--profile serve up -d`(env export: NAS_MODEL_PATH·TIKTOKEN_HOST_PATH·CONFIG_FILE·SERVING_PORT) → `:PORT/health` http200 폴링(**python urllib — curl deny**) → 로컬 functional smoke(완성/reasoning, finish=stop). "startup complete" 로그는 거짓양성. |

## Message 타입
- **instruction**(메인→서브): 수행할 Task(phase + per-task 값: 모델명·VRAM 예산·NAS 모델 서브디렉토리). **single 서빙 태스크**면 추가 슬롯: max_model_len·served_model_name·SERVING_PORT·reasoning_parser. 운영 절차(env export·detached up·health200 python·reasoning max_tokens)는 §phase serve 술어(single 분기)에 있으니 매번 재기술 불요. **신규 vLLM build-job**(메인 upstream 발동분)은 빌드-잡 인가를 패킷에 담아 전달하되, *전파 자체*는 메인이 네 위임 키를 확인한 뒤에만 한다(§A2A 위임 — `sync_to_sub` 전파 게이트).
- **feedback**(메인→서브): "여기가 틀렸으니 이렇게 고쳐". 너는 **직접 고쳐** 다음 턴에 재-attest(자기교정).
- **report**(서브→메인): task-report.schema.json JSON 1개.

## 경계 (B3 Surgical)
- 너는 **모델별 `configs/`·`envs/` 만** 자작한다. 컨테이너 정본(Dockerfile/requirements/compose/serve_runner)·빌딩블럭(.claude/, CLAUDE.md, Agent_Card.json)은 **건드리지 않는다**.
- HW 사실·경로·획득 모드는 **이 노드의 `output/<topology>/manifest.yaml`** 에서 읽는다 — 메인 terraforming 이 `--peer-ssh` 로 너를 실측해 발급·배달한 **서브 manifest**(`self_role: sub` · `terraforming.issued_by: main`)다. 너는 이 파일을 손으로 고치지 않는다(권위는 메인 스캔 · 재발급은 메인 `scan_node.py --emit-sub-manifest`). per-task 값(모델명·VRAM 예산·NAS 서브디렉토리)은 Task Message 에서 읽는다. **A2A 위임 키 `.claude/a2a_delegation.json`** 는 메인이 발급한 양성 게이트 면제 키다(§A2A 위임) — 면제는 Flag 검사뿐이며 HW 사실은 manifest 가 채운다.
