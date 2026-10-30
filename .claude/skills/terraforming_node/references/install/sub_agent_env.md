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
- 2.5.1 hint 발행 장착 — single 서브 한정 (신설 2026-10-11 · `plan_26101104` Step 3)
- 2.5.2 Codex 배포와 실제 권한 준비성
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
- **런타임블럭**(서브 복제 ✓): a2a-agent 서브 = 런타임 3종(`vllm-recipe-explorer`·`adversarial-benchmark`·`upstream-version-watch`) **+ `hint-publisher`**(2026-10-11 · 자기 셀 hint 자체 발행 · §2.5.1) · ray-worker 서브 = 0종(정본 `node_role_contract.TOOL_PLANE_BY_SUB_MODE`). 서브가 **동일 결정론 엔진**을 자기 모델에 자율 실행 → 자율=실행 주체, 방법=결정론(헌법 "확률론 추론 금지" 보존).

### 2.2 구축 아티팩트 (서브 워크스페이스 레이아웃 · 행 수는 아래 표가 정본)
| 경로(서브 루트) | 내용 | 전달타입 |
|---|---|---|
| `CLAUDE.md` | 페르소나(Karpathy B1–B4, **노드정체성 bake**, 자율 triplet 저작 + 자기교정) | 렌더(gitignored) |
| `AGENTS.md` | 같은 역할 헌법·통신·리포트로 연결하는 Codex 진입문서 | 역할별 렌더 |
| `.agents/skills/*/SKILL.md` | canonical `.claude/skills`를 가리키는 얇은 발견 wrapper | `node_role_contract.tool_plane`이 지목한 스킬만 복제 |
| `.codex/config.toml` · `.codex/rules/sub.rules` | main/sub 공통 `:danger-full-access` 기본값·직접 argv 차단 보조층 | 공통 권한 렌더 + 정적 rules 복제 |
| `Agent_Card.json` | A2A 능력카드(skills[]·노드정체성) | 렌더(gitignored) |
| `.claude/settings.local.json` | **스코프드** 권한(블랭킷 ✗) | 렌더(gitignored) |
| `.claude/skills/vllm-recipe-explorer/` | 런타임블럭(git-tracked만) | 복제 |
| `.claude/skills/hint-publisher/` · `hints/{HINT_ISSUANCE_CONTRACT.md,vocab.json}` · 엔진 의존(`completion_gate.py`·`evidence_publisher.py`·스키마 3·명명 shim `policies/runtime/doc_naming.py` → 정본 `wiki-desk/src/doc_naming.ts`) · `check_sub_hint_equip.py` | single 서브 hint 엔진(정본 `render_sub_env.HINT_ENGINE_DEPENDENCIES` · 색인·카탈로그·역할 마커는 `HINT_NEVER_DELIVER` 로 영구 미배달) | 복제(a2a-agent 만) |
| `.claude/rules/comms.md` | 통신 정적계약 | 복제 |
| `.claude/rules/strategy.topology.md` | 이 체크아웃의 특화헌법 | 복제(반대 브랜치의 특화 바이트는 옮기지 않음) |
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
- 기존 서브의 지침·하네스 환경만 갱신하면 같은 정식 명령에 `--overlay-only`를 준다.
  빌드킷·source-port·모델 런타임 패치를 배달하지 않고 오버레이 checksum만 검사한다. 기본 배달과
  토폴로지·인가·dirty 보존·rollback 계약은 유지하며, 서브 git이 없는 초기 구축에는 사용할 수 없다.

### 2.4 A2A-개념 협업 계약 (서버 없음)
- 메인=client(Task 발급·리포트 검증·피드백) · 서브=remote(자율 수행·자기검증 리포트 1개). A2A 어휘 차용, HTTP 서버 ✗(전송=SSH 단발 `delegate(task)` — provider 문법은 `references/orchestration/agent-control-adapter.md`).
- **검증 = push-attestation**: 서브가 self-verification(config-parse·schema·runner 문법·checksum·**로컬 스모크**)을 리포트에 담아 회신 → **메인은 리포트만 검증, 서브 워크스페이스 재스캔 ✗**.
- **성공술어**: phase 별(comms.md). 예: config = triplet 생성 + 로컬 스모크 응답("린트 통과 ≠ 서빙됨").
- **상태=파일**: `campaigns/<camp-id>/relay/<context_id>.json`("파일=세션" · 2026-09-06 루트 `tasks/` 에서 이관 · 활성 캠페인 부재 시 `_bootstrap`). 예산은 **메인이 매 attempt 단위와 함께 선언한다**(공통 `--timeout-seconds`·`--budget-source`, Claude Code의 native-turn 한도 `--max-turns`, Codex의 wall-time·outer dispatch 한도) — 옛 `max-turns=3`(2026-09-03 폐기)에 이어 그 대체물이던 **grade 표도 2026-09-05 폐기**됐다(표가 실측 없이 정본 행세를 했고 교정 소비자가 0 이었다 · `audit_26090515` G-A2). `scripts/turn_budget.py` 는 이제 선언을 **검증**만 한다(상한은 요청 스키마에서 읽는다). 소진은 terminal 이고 다음은 **더 큰 예산의 새 attempt** 이며, 그 이어붙이기는 `scripts/relay.py --continue` 가 **본문을 조립**한다(재개의 기본은 `--supervise-step --apply` 자동 재발급이고, 사람은 팝업된 예외에서만 답·승인한다 — §2.7.7a).
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

### 2.5.1 hint 발행 장착 — single 서브 한정 (신설 2026-10-11 · `plan_26101104` Step 3)

single 서브(sub_mode `a2a-agent`)는 **자기가 잰 셀을 스스로 hint 태그로 발행**한다 — 서브에서만 잰 셀은 메인이 관측 원시를
볼 수 없어 발행이 구조적으로 불가했다(계약 D-e · `HINT_QUALIFICATION_UNOBSERVED`). 멀티 서브(Ray 워커)에는 해당이 없다.
이 장착은 terraforming_node 가 소유한다(새 스킬 ✗). **실행자**: ①②④ = 서브 에이전트(A2A 지시) · ③ = 사람 · 판정 수용 = 메인.

| 자리 | 선언(단일) | 무엇 |
|---|---|---|
| 엔진 | `node_role_contract.TOOL_PLANE_BY_SUB_MODE["a2a-agent"]` → `render_sub_env` | `hint-publisher` + 엔진 의존. 정상 배달(§2.3)로 간다 — 별도 절차 없음 |
| hint 클론 경로 | 서브 워크스페이스 `envs/.env` 의 `HINT_CLONE_DIR=<절대경로>` | **워크스페이스 밖**. `envs/.env` 는 서브 저작·비추적·비배달 평면이라 그 노드의 경로를 그 노드가 든다 |
| push 역할 선언 | 서브 워크스페이스 `hints/.push_authority`(비추적) | hint.py 가 도는 체크아웃의 역할 선언. 카탈로그 권위 `hints/.central_authority` 는 메인 단독 — 서브에 두면 위반 |
| push 토큰 | 서브 워크스페이스 `envs/.env` 의 `GITHUB_TOKEN`(또는 환경변수) | 메인이 `envs/.env` 에서 읽는 것과 **같은 규칙**(`hintlib.tag.read_push_token`). **사람이 둔다** — 비밀은 배달 평면 ✗ |
| PII 리터럴 | 서브 워크스페이스 `.claude/pii_terms.txt`(비추적) | 그 노드의 리터럴. hint 4종 스캔이 부재를 fail-closed 로 막는다. `.claude/**` 는 서브 에이전트가 쓸 수 없어 **사람이 둔다** |

절차(메인이 A2A Task 로 지시 · 서브가 수행 · 결과는 서브 testlog 로 회수 — ssh 직접 관측 ✗):

1. **클론** — 메인이 Task Message 에 프로젝트 원격 URL(자격증명 없는 https 형 · 토큰 방식과 짝)을 싣는다. 서브는 `envs/.env` 에
   `HINT_CLONE_DIR=<워크스페이스 밖 절대경로>` 한 줄을 두고
   `python3 .claude/skills/terraforming_node/scripts/check_sub_hint_equip.py --clone --remote-url <URL>` —
   `git clone --single-branch --branch hint --depth 1 --no-tags` 로 별도 저장소를 만든다(워크스페이스 git 에 원격을 붙이지 않는다 ·
   policy:SUB_GIT_LOCAL_ONLY 는 워크스페이스 git 을 다스린다). 멱등이며, 경로가 비어 있지 않은 다른 것이면 지우지 않고 멈춘다.
2. **push 역할 선언** — `check_sub_hint_equip.py --write-push-authority [--utc <UTC>]`. 그 경로가 서브 `.gitignore` 에 덮이지
   않으면 쓰지 않는다(추적되면 `[sync]` 커밋으로 남고 복사된다).
3. **토큰·PII 리터럴 = 사람** — 판정에 `token: absent` 또는 `pii_terms: absent` 가 나오면 서브는 **보고만** 하고 그 단계를 멈춘다.
   메인은 회수한 testlog 를 읽고 템플릿 `templates/requests/sub_hint_push_equip.md` 를 복제해
   `docs/request/request_<YYMMDDHH>[_MM_SS]_서브_hint_push_자격_배치.md` 로 발행한다(이름 = `wiki-desk/src/doc_naming.ts` ·
   전제·명령·성공판정·실패분기·회수물·소요를 갖춘 수행지시서 · 서브에는 `docs/request/` 가 없다 — 메인 몫). 사람이 둔 뒤 에이전트는
   **존재만** 다시 확인한다 — 값을 읽어 비교하거나 출력하지 않는다.
4. **판정·보고** — `check_sub_hint_equip.py`(인자 없음)의 JSON(`checks.{hint_clone,push_authority,central_authority,token,pii_terms,
   workspace_git_local_only}` · `ready` · `next_actions[]` · `provenance: measured`)을 서브 testlog 에 그대로 싣고 task-report 로 경로를
   전달한다. exit 0 = ready · 3 = 미장착(판정 정상) · 2 = 선언 결함 · 4 = 장착 실패. 역할·원격 위반(`present_violation`·`violation`)은
   서브가 스스로 고치지 않는다 — 메인 HITL.
5. **발행(장착 뒤 · 승인된 자기 셀만)** — 서브는 파생 선언(`--from-slice`)에 실린 `hint_targets` 의 셀만
   `hint.py publish → 저작 → 독립 사실 검증(저작자 ≠ 검증자 · 서브 안의 별도 Agent) → continue --hint-clone "$HINT_CLONE_DIR"` 로 낸다.
   카탈로그 단계는 건너뛰고(push 역할만), 서브 testlog 에 승인 출처(메인 캠페인 id · approved_by · approved_utc · 슬라이스 발급 시각) ·
   태그 · 원격 SHA · 엔진 리비전(배달 커밋)을 적는다. 메인은 회수해 자기 선언의 `hint_targets[].approval` 과 대조한 뒤 `catalog derive`.

발행 자체(장착 이후)의 계약:

- **승인** — 메인 선언 확인 팝업의 셀별 Y/N 을 메인이 `campaign_init.py --hint-approve --node <서브 노드> --cells …` 로 **메인 선언**에
  적는다. `--emit-slice <서브 노드>` 가 그 노드 몫 `hint_targets`(승인 포함)만 운반하고 메인·다른 노드 몫은 자른다(판정 =
  `node_role_contract.sub_self_publishes` 파생). 서브는 자기 인스턴스에 승인을 쓰지 못한다(writer 거부 · 검증기가 남의 몫을 거부).
- **push** — 태그 1개를 별도 hint 클론으로만(`hint.py` · exact refspec). 카탈로그 단계는 서브에서 건너뛴다(카탈로그 = 메인 단독).
- **보고** — 서브 testlog 에 승인 출처 · 태그 · 원격 태그 SHA · 엔진 리비전. 메인이 `fetch_sub_docs.sh` 로 회수 → 승인 출처 대조 →
  `hint.py catalog derive`. 엔진 버전 어긋남은 차단하지 않고 기재한다.
- 자체검사: `check_sub_hint_equip.py --self-test`(verify_distribution) · `render_sub_env.py --self-test` (4b)·(4d) · `campaign_init.py --selftest`(운반 양·음성).


### 2.5.2 하네스 실행환경과 실제 업무 권한 준비성

정본은 [harness_environment.md](harness_environment.md)다. 공통 역할·소유·승인을 두 provider의 네이티브
계약으로 연결하며, 테라포밍이 경로 관측·설정 렌더·공식 배달·조건부 호스트 request·실제 카나리를 주관한다.
HW Flag가 FRESH여도 하네스/권한/신뢰 변경 시 이 절로 부분 재진입한다. single의 등록된 A2A 서브도 대상이다.
`AGENTS.md`·`.agents`는 기존 canonical 지침으로 연결하고, 메인 오케스트레이션·개인 설정·인증은 배달하지 않는다.

`bootstrap_canary.py --phase discovery`의 읽기 확인만으로 업무 권한 준비를 선언하지 않는다.
`--phase permissions --target main|sub`의 실제 도구 실행 증거·schema·역할 업무를 확인한다. single의 외부
hint clone/git/common-dir은 명시 관측·기존 승인에 결속한 `--hint-write-scopes`를 renderer와 공식 sync에 넘긴다.
이 기록은 대상·승인 결속이며 Codex filesystem 허용 목록을 생성하지 않는다. 미지정은 hint 발행 준비의 증거가 아니다. multi에 같은 발행 범위를 추가하지 않는다.

Codex 설치 기본값은 main/sub 모두 `default_permissions = ":danger-full-access"`이며 별도 역할 프로필과
bwrap/AppArmor 설치를 요구하지 않는다. bwrap 기동 실패는 실제 선택·legacy override·신뢰 로딩을 먼저
진단한다. 설정 진단 request 경로는 `templates/requests/codex_linux_sandbox.md`다.
운영자 설치 완료·실제 native 권한·A2A 왕복·캠페인 Full/hint는 서로 별개의 완료 조건이다.

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

- `sub_node/` — 추적 PII-free 템플릿·정적계약: `CLAUDE.template.md`·`Agent_Card.template.json`·`settings.local.template.json`·`comms.md`·`task-report.schema.json`·`gitignore.template`. (옛 `library-exchange.schema.json` 은 2026-10-19 은퇴 — 서브는 서고 접근자로 직접 조회한다 · §2.7.8)
