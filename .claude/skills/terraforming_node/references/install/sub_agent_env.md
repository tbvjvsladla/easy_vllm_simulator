# 설치 — 서브 에이전트 환경 구축 · Agent_Card v2 · 서브 manifest

> terraforming_node 스킬 reference — SKILL.md §2 · §2.1–§2.5 · §2.7.10 의 본문이다. SKILL.md(라우터)는 § 번호·제목·포인터만 든다.
> 이관 전 원문: `git show dcb713a:.claude/skills/terraforming_node/SKILL.md` (plan_26093022 Step 4 — 본문 바이트는 그대로 옮겼다).

## 목차

- 2. 서브 에이전트 환경 구축 (A2A-개념 — plan_26062408)
- 2.1 스킬 분류학 (결정론↔자율성 충돌 해소)
- 2.2 구축 아티팩트 (서브 워크스페이스 레이아웃 · 행 수는 아래 표가 정본)
- 2.3 렌더-온-메인 → 전달 (결정론 + HITL)
- 2.4 A2A-개념 협업 계약 (서버 없음)
- 2.5 완료 게이트 — model-less 카나리 라운드트립 (R1 정합)
- 2.7.10 Agent_Card v2 (A2A 1.0.1) · 서브 manifest (신설 2026-09-05 · `plan_26090516` §7.2–7.3 · H1)
- 서브 템플릿 (옛 §4 에서 이관)

## 2. 서브 에이전트 환경 구축 (A2A-개념 — plan_26062408)

> 서브노드 코드에이전트가 메인과 **불투명 피어**로 협업하려면, 서브 워크스페이스에 **페르소나·능력·권한·
> 런타임스킬·통신프로토콜**이 있어야 한다. 진입 루틴(§1)이 manifest 를 채운 뒤, 이 단계가 그 환경을
> **메인에서 렌더해(render-on-main) 서브로 전달하고 카나리로 검증**한다.

### 2.1 스킬 분류학 (결정론↔자율성 충돌 해소)
- **빌딩블럭**(메인 전용, 서브 전달 ✗): `terraforming_node`. 온보딩·노드 계약·서브 정체성 판정은
  영구히 메인 단독이다(서브 자가스캔 ✗).
- **`upstream-version-watch` 는 스킬 전체가 아니라 경로 단위로 갈린다**(2026-09-03 개정 · `plan_26090317` P2).
  한 스킬 안에 성질이 다른 둘이 섞여 있었고, "전체를 주느냐 마느냐" 로 물으면 어느 답도 옳지 않았다:
  - **서브에 간다(a2a-agent 만)** — 해소·렌더 능력: `resolve_*`·`render_dockerfile`·`regen_requirements`·
    `classify_failure`·`check_smoke_model`·`references/*`. 싱글 서브가 "모델+엔진 핀을 받아 자율 빌드→서빙→벤치"
    를 하려면 이것이 있어야 한다(사용자 범위 선언).
  - **서브에 가지 않는다(전 모드)** — 노드 간 오케스트레이션: `sync_to_sub.sh`·`sync_branches.sh`·
    `fetch_sub_docs.sh`·`smoke_clone.sh`·`multinode_*_smoke.sh`. 이것들은 **메인이 서브를 향해** 쓰는
    도구다. 서브가 들면 배달 방향이 뒤집히고(§2.7.1 권한 평면), 서브가 다른 노드를 향해 쓰는 경로가 생긴다.
  - 정본 = `render_sub_env.RUNTIME_BLOCK_EXCLUDES` (닫힌 목록 · 자체검사가 새 스크립트의 미분류를 fail-loud).
- **ray-worker 서브는 런타임 스킬 0종**이다 — 정본(Dockerfile·compose·serve_runner)을 재현하는 워커이지
  전략을 세우는 주체가 아니다. 판정 정본 = `node_role_contract.tool_plane`.
- **런타임블럭**(서브 복제 ✓): a2a-agent 서브 = `vllm-recipe-explorer`·`adversarial-benchmark`·`upstream-version-watch` 3종 · ray-worker 서브 = 0종(정본 `node_role_contract.TOOL_PLANE_BY_SUB_MODE`). 서브가 **동일 결정론 엔진**을 자기 모델에 자율 실행 → 자율=실행 주체, 방법=결정론(헌법 "확률론 추론 금지" 보존).

### 2.2 구축 아티팩트 (서브 워크스페이스 레이아웃 · 행 수는 아래 표가 정본)
| 경로(서브 루트) | 내용 | 전달타입 |
|---|---|---|
| `CLAUDE.md` | 페르소나(Karpathy B1–B4, **노드정체성 bake**, 자율 triplet 저작 + 자기교정) | 렌더(gitignored) |
| `Agent_Card.json` | A2A 능력카드(skills[]·노드정체성) | 렌더(gitignored) |
| `.claude/settings.local.json` | **스코프드** 권한(블랭킷 ✗) | 렌더(gitignored) |
| `.claude/skills/vllm-recipe-explorer/` | 런타임블럭(git-tracked만) | 복제 |
| `.claude/rules/comms.md` | 통신 정적계약 | 복제 |
| `.claude/rules/docs.md` | 문서발행 규약(D12 — 서브 동일 규약 발행 → 상향 문서기반 회수) | 복제(메인 `.claude/rules/docs.md`) |
| `.claude/schemas/task-report.schema.json` | 자기검증 스키마 | 복제 |
| `.gitignore` | 서브 로컬 git 추적규칙(D12 — docs persist·생성물 무시) | 복제(`sub_node/gitignore.template`) |
| `docs/{plan,devlog,testlog,simlog}/example.md` | 발행 스켈레톤(D12 — 서브 insight 문서) | 복제(메인 docs/*/example.md) |
| `campaigns/_template/**` · `campaigns/_bootstrap/relay/` | 캠페인 뼈대 + 릴레이 원장 스캐폴드(파일=세션) | 메인 뼈대 복제 + 빈 디렉토리 |

추적 템플릿·정적자산은 `sub_node/`(CLAUDE.template.md·Agent_Card.template.json·settings.local.template.json·comms.md·task-report.schema.json·gitignore.template) — **PII-free**(IP·호스트 비박음, 렌더 시 manifest 에서 치환). (`docs.md`·`docs/*/example.md` 는 메인 정본을 D12 복제 — sub_node/ 외부 원천.)

### 2.3 렌더-온-메인 → 전달 (결정론 + HITL)
- **렌더**(결정론): `python3 scripts/render_sub_env.py --topology multi` → manifest 노드정체성을 템플릿에 치환,
  gitignored 스테이징 `output/multi/sub_provision/` 산출(서브 루트 미러). 미치환 placeholder·필수 누락 시 fail-loud.
- **전달**(HITL) — **인가 체인이 먼저다**. `sync_to_sub.sh` 는 `--mode` 와 `--manifest` 를 **필수**로 받고,
  `completion_gate.py authorize --action sync_to_sub` 로 포워딩해 **discovery·ssh·rsync 이전에** fail-closed 한다.
  2026-09-03 이전에는 이 문서·렌더러 출력·스크립트 자기안내 **세 곳 모두** 그 두 인자를 빠뜨려, 안내대로 치면
  `CLI_USAGE_ERROR` 로 거부됐다(B0).

  ```bash
  # ① 증거 레코드 발행(work-manifest 의 뼈대)
  python3 .claude/policies/runtime/evidence_publisher.py init       --task-class harness_change --topic <주제>       --generated-utc <YYYY-MM-DDTHH:MM:SSZ> --identity-json <identity.json>
  # ② 사람이 plan 문서에 `## Execution approval` 앵커 + 원자 3종을 적고(approved_by/approved_at_utc/allowed_action),
  #    그 값을 work-manifest 의 execution_approval 에 기입한다(plan_sha256 = 그 plan 바이트의 sha256).
  #    → allowed_actions 에 `sync_to_sub` 가 있어야 인가가 열린다.
  # ③ 전달
  bash .claude/skills/upstream-version-watch/scripts/sync_to_sub.sh       --mode experimental --manifest <work-manifest.json> --apply --provision --branch <이 체크아웃의 토폴로지: multi|single>
  ```
  — `--branch` 는 **이 체크아웃의 특화헌법·4자일치가 말하는 토폴로지와 같아야** 한다. 어긋나거나 `both` 면 인가 전에
  exit 12 로 멈춘다(특화층 오배달 · 옛 빌드킷 다운그레이드 차단 · 2026-09-14). 렌더 성공 화면은 렌더한 통로를 그대로 찍는다.
  — 메인 rsync(코드) **이후** 스테이징을 서브 루트로 **오버레이(--delete 없음)**. 빌딩블럭 스킬·메인 manifest 는 전달 안 됨(런타임블럭 · 서브 manifest 는 §2.7.10 설치 오버레이만).
  - `--mode promotion` 은 verify 가 `promotion-ready` 에 도달한 경우에만 열린다(hint·last-good 평면). 서브 배달은 통상 `experimental`.
- **단일 전달차**: 코드+에이전트환경 모두 sync_to_sub.sh 한 경로. dry-run 기본 → 사람 검토 후 --apply.

### 2.4 A2A-개념 협업 계약 (서버 없음)
- 메인=client(Task 발급·리포트 검증·피드백) · 서브=remote(자율 수행·자기검증 리포트 1개). A2A 어휘 차용, HTTP 서버 ✗(전송=SSH 단발 `delegate(task)` — provider 문법은 `references/orchestration/agent-control-adapter.md`).
- **검증 = push-attestation**: 서브가 self-verification(config-parse·schema·runner 문법·checksum·**로컬 스모크**)을 리포트에 담아 회신 → **메인은 리포트만 검증, 서브 워크스페이스 재스캔 ✗**.
- **성공술어**: phase 별(comms.md). 예: config = triplet 생성 + 로컬 스모크 응답("린트 통과 ≠ 서빙됨").
- **상태=파일**: `campaigns/<camp-id>/relay/<context_id>.json`("파일=세션" · 2026-09-06 루트 `tasks/` 에서 이관 · 활성 캠페인 부재 시 `_bootstrap`). 턴 예산은 **메인이 매 attempt 선언한다**(`--max-turns`·`--timeout-seconds`·`--budget-source`) — 옛 `max-turns=3`(2026-09-03 폐기)에 이어 그 대체물이던 **grade 표도 2026-09-05 폐기**됐다(표가 실측 없이 정본 행세를 했고 교정 소비자가 0 이었다 · `audit_26090515` G-A2). `scripts/turn_budget.py` 는 이제 선언을 **검증**만 한다(상한은 요청 스키마에서 읽는다). 소진은 terminal 이고 다음은 **더 큰 예산의 새 attempt** 이며, 그 이어붙이기는 `scripts/relay.py --continue` 가 **본문을 조립**한다(재개의 기본은 `--supervise-step --apply` 자동 재발급이고, 사람은 팝업된 예외에서만 답·승인한다 — §2.7.7a).
- per-task 휘발값(모델명·예산·NAS 서브디렉토리)은 **Task Message** 로(manifest 복제 아님).
- single의 독립 `a2a-agent` sub campaign은 main-derived declaration을 relay task body의 JSON으로 받고, `campaign_init.py --init <id> --plan-ref <ref> --from-slice - --apply`로 stdin에서 소비한다. 별도 slice 파일은 만들지 않는다. `--from-slice <PATH>`는 호환 입력이다. multi의 `ray-worker`는 main-owned distributed cell에 참여하므로 독립 campaign slice/자율 init 대상이 아니다.

### 2.5 완료 게이트 — model-less 카나리 라운드트립 (R1 정합)
전달 후 메인이 **모델 없이** 부트스트랩 Task 1회. **실행자 = `scripts/bootstrap_canary.py`**(2026-09-03 신설 · S1):
manifest 의 `nodes[sub]` 에서 host·ssh_user·work_dir 를 읽고 `node_role_contract` 가 정한 `sub_mode` 로
**정체성에 맞는 카나리 문구**를 골라 request 를 조립한다(ray-worker 에게 "런타임 스킬 3종" 을 묻지 않는다).
```bash
python3 scripts/bootstrap_canary.py --topology <single|multi> \
    --max-turns 10 --timeout-seconds 600 --budget-source "선언: 카나리 1왕복(인스펙트 전용)" \
    --emit /tmp/canary.json            # 조립(결정론)
python3 scripts/bootstrap_canary.py --topology <single|multi> \
    --max-turns 10 --timeout-seconds 600 --budget-source "..." --invoke   # HITL 승인 뒤 실행
```
turn 예산은 **선언**이다 — 등급표가 사라졌으므로 부르는 쪽이 값과 근거를 함께 준다(미선언 = fail-loud).
서브 미등록·`__REQUIRED__` 센티넬 잔존 시 **조립 자체를 거부**한다(틀린 계정/경로로 접속하지 않는다).
이전 판본은 이 자리에 `bootstrap_canary()` 라고만 적혀 있었고 **생산자가 0개**였다(실행자 없는 금지 — §2.7.1 위반).
(실행문법 = `references/orchestration/agent-control-adapter.md` §2)
→ 서브가 새 CLAUDE.md+런타임블럭+settings+comms 로드, **phase=inspect·status=completed + self_verification** 의 schema-valid 리포트 반환.
이로써 "구성된 환경이 프로토콜대로 작동함"을 전체로서 증명(권한행·skill YAML·페르소나 비준수 포착 — 체크섬이 못 잡는 것). 실패 시 max-turns→Model-C. **카나리 미통과 시 done 선언 금지.**

- 카나리 통과 → **호스트 안전체계 세션 최종 Y/N**(§2.6, 양노드) 수행 후 온보딩 완료.


### 2.7.10 Agent_Card v2 (A2A 1.0.1) · 서브 manifest (신설 2026-09-05 · `plan_26090516` §7.2–7.3 · H1)

| 항목 | 정본 | 내용 |
|---|---|---|
| **표준** | `a2aproject/A2A` v1.0.1 `specification/a2a.proto`(JSON camelCase) | 필수: `name`·`description`·`supportedInterfaces[]`·`version`·`capabilities`·`defaultInputModes[]`·`defaultOutputModes[]`·`skills[]`. 선택: `provider`·`documentationUrl`·`securitySchemes`·`securityRequirements`·`signatures[]`·`iconUrl`(표준엔 있으나 이 계약은 `signatures` 를 거부한다 — 아래 보안 경계). **표준 밖 최상위 키 금지** |
| **전송** | `supportedInterfaces[0]` | `url: ssh://<user>@<host>` · `protocolBinding: urn:easy-vllm:a2a-binding:ssh-claude-p:v1`(커스텀 바인딩은 **URI** — 표준 §5.8·§12.7) · `protocolVersion: "1.0"`. 전송 인증 = SSH 공개키(표준 SecurityScheme 5종 밖 → `securitySchemes` 비움 · 차이는 comms.md 가 문서화) |
| **노드 역할** | `capabilities.extensions[urn:easy-vllm:ext:node-role:v1]` | §2.7.6(c). 값은 `node_role_contract.py` 해소 · 렌더러는 적기만 |
| **skills 6** | `inspect`·`config`·`build`·`serve`·`bench`·`publish` | `publish` = §2.7.9 발행 Phase. 6종 미만이면 계약 위반 |
| **HW 사실** | **카드에 없다** → 서브 manifest | 카드 = "무엇을 할 수 있나"(A2A 평면) · manifest = "무엇 위에서 도나"(HW·경로·획득 모드) |
| **서브 manifest** | `scan_node.py --topology single --peer-ssh <sub> --model-source <m> --emit-sub-manifest output/single/sub_manifest.yaml` | 메인이 `--peer-ssh` 로 실측(HW 5종·모델 환경·egress)해 조립 · 스키마 = 메인 manifest + `self_role: sub` + `terraforming.issued_by: main`. 서브는 HW 스캔 권위 데이터를 스스로 만들지 않는다. `render_sub_env.py --sub-manifest` 가 스테이징 `output/<topology>/manifest.yaml` 로 넣고 **설치 오버레이**가 배달한다(빌드킷 배달 평면 D10 과 무관) |
| **Card 보안 경계** | `agent_card_contract.py validate` | Card는 unsigned capability/discovery metadata다. `signatures`·JWS·별도 trust store는 계약 밖이며 endpoint 인증이나 실행 허가를 대신하지 않는다. |
| **검증기** | `agent_card_contract.py validate` · `render_sub_env --self-test` | 필수 필드·바인딩 URI·node-role 확장·skills·float 금지를 검증하고 credential 필드 재도입을 거부한다. |
| **정체성·준비성** | SSH + `manifest_contract.py` | SSH public-key/known-host가 endpoint를 인증한다. manifest가 role·rank·HW 사실을 소유하며, sub readiness는 Flag와 `terraforming.issued_by: main`을 요구한다. 실행 허가는 work manifest·campaign assignment·scope가 별도로 소유한다. |


## 서브 템플릿 (옛 §4 에서 이관)

- `sub_node/` — 추적 PII-free 템플릿·정적계약: `CLAUDE.template.md`·`Agent_Card.template.json`·`settings.local.template.json`·`comms.md`·`task-report.schema.json`·**`library-exchange.schema.json`**(§2.7.8 그라운딩 교환)·`gitignore.template`.
