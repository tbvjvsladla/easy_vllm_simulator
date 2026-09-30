# 오케스트레이션 — 토폴로지 분기(제1축) · 노드 정체성

> terraforming_node 스킬 reference — SKILL.md §2.7.0 · §2.7.6 의 본문이다. SKILL.md(라우터)는 § 번호·제목·포인터만 든다.
> 이관 전 원문: `git show dcb713a:.claude/skills/terraforming_node/SKILL.md` (plan_26093022 Step 4 — 본문 바이트는 그대로 옮겼다).

## 목차

- 2.7.0 토폴로지 분기 — **제1축** (신설 2026-08-22 · `plan_26082214` §4.2)
- 2.7.6 노드 정체성(node-identity) — `role` + `rank` 스킴
- (a) node_id — 이름 (양 토폴로지 공통)
- (b) rank — 위치 (**멀티 전용** · 신설 2026-08-22)
- (c) 렌더 산출물에 실릴 형태 — ✅ **배선 완료**(2026-08-22 · W-2 → 2026-09-05 Agent_Card v2 로 이관)
- node_role_contract 메모 (옛 §4 에서 이관)

### 2.7.0 토폴로지 분기 — **제1축** (신설 2026-08-22 · `plan_26082214` §4.2)

> 헌법 **불변식 A** 의 "어떻게". 헌법은 *왜 토폴로지가 제1축인가*를 말하고, 이 절은 *그래서 어느
> 절·어느 도구가 켜지는가*를 정한다. 외부 그라운딩 정본 = `docs/report/node-identity-topology-grounding.md`.

**한 단어가 두 존재를 덮고 있었다.** `output/single/manifest.yaml` 과 `output/multi/manifest.yaml` 은
같은 `nodes[].role: main|sub` 스킴을 쓰지만, 두 통로의 "sub" 는 본질이 다르다:

| | **multi 의 sub** = Ray 워커 | **single 의 sub** = A2A 원격 에이전트 |
|---|---|---|
| 정체성 권위 | **manifest `nodes[]` 인덱스(rank) + role** | **`Agent_Card.json`**(A2A 1.0.1 계약: 능력·엔드포인트 — unsigned capability metadata) + **서브 manifest**(`self_role: sub` · HW·경로·획득 모드 — terraforming 실측·발급 · §2.7.10) |
| 외부 정본 | NCCL rank + uniqueId · Ray head/worker | A2A Client·AgentCard·Task |
| sub↔sub 통신 | 대칭 collective(집단 연산) | **없음** — 각자 메인하고만 대화 |
| 제어 평면 | head 종속(SSH 제어) | client→server 호출(A2A Task 위임) |
| 버전·드라이버 | **동기 필수**(집단 연산 ABI 정합) | **독립**(핀 커플링 없음) |
| 빌드 흐름 | 메인 빌드 → `sync_to_sub` 전파 → **동일 빌드킷에서 파생된 동일 ABI** 분산 | 노드별 **독립 병렬 빌드**(서브가 자기 빌드킷 자율 저작) |
| `sub_mode` | `ray-worker` | `a2a-agent` |

- **버전 싱크의 이유는 로드 균형이 아니라 집단 연산의 lockstep 정합**이다(그라운딩 §6.1). 그래서 싱글엔
  적용될 이유가 애초에 없다 — 싱글은 collective 에 참여하지 않는다.
- ⚠ **"동일 이미지"가 아니라 "동일 ABI"다**(2026-08-22 실측 정밀화 · `testlog_26082215` §4.7 M-4).
  분산 서빙에 실제로 참여한 두 컨테이너의 **image digest 는 서로 달랐다**(`58fd5b62…` vs `c5487620…`)
  — 각 노드가 로컬에서 자기 빌드를 하므로 digest 일치는 애초에 성립하지 않는다. 정확히 일치해야 하는
  것은 **집단 연산 ABI 3종**(vLLM git SHA · torch · driver)이고, 실측에서 그 셋은 완전히 일치했다.
  digest 를 커플링 판정 기준으로 쓰면 정상 배포를 불일치로 오판한다.
- **`role: sub` 의 *존재*는 정체성을 말하지 않는다.** 판정 입력은 언제나 **`sub_mode`** 다.

**결정론 판정기 = `scripts/node_role_contract.py`**(이 계약의 단일 소유자):

| 판정 | 호출 | 산출 |
|---|---|---|
| 이 sub 는 무엇인가 | `resolve_sub_mode(topology, declared)` | `{value, source}` — `derived-from-topology` \| `declared-and-agrees` |
| 집단 안의 위치 | `resolve_rank(topology, nodes, role)` | multi=`nodes[]` 인덱스 · single=`None` + `not-applicable:single-a2a-agent` |
| 정체성 권위는 어디인가 | `identity_authority(topology)` | `agent-card` \| `manifest-rank-and-role` |
| **빌드킷 배달 평면이 켜지나** | `delivery_plane(topology, sub_mode)` | `active`(ray-worker) \| `dormant`(a2a-agent) |

```bash
# 셸 소비자는 한 줄 값으로 묻는다. 위반이면 값을 찍지 않고 종료코드 5.
python3 .claude/skills/terraforming_node/scripts/node_role_contract.py \
        evaluate --topology single --repo . --field delivery_plane --format value    # → dormant
```

> ✅ **배선 완료(2026-08-22 · W-1)** — `sync_to_sub.sh:_single_extension_active` 는 이제 위 판정기를
> 호출하고 그 답(`delivery_plane`)만 비교한다. `role: sub` 의 *존재* 는 더 이상 판정 입력이 아니다.
> 판정기 부재·파싱 실패·계약 위반은 전부 **dormant(fail-closed)** 이며 사유를 stderr 로 밝힌다.
> dry-run/B1 안내문도 판정기가 답한 **출처**를 그대로 인용한다(`source=sub-mode:a2a-agent` ·
> `no-sub-registered` · `fail-closed:*`) — "nodes[] 비어있음" 이라는 옛 모델 문구는 제거됐다.
> 이전 상태의 실증은 `testlog_26082215` §4.2(같은 평면이 **34회 발화**)다.

- **`sub_mode` 는 파생값이다 — manifest 선언은 tripwire다.** 사상이 1:1 이라 topology 만 알면 계산된다.
  필수 손저작으로 만들면 헌법 §판정표의 *"파생 가능한데 손으로 적은 것"*(하드코딩 **결함**)이 된다.
  선언은 **선택**이며, 선언되면 파생값과 **일치해야 한다**(어긋나면 `SUB_MODE_TOPOLOGY_CONFLICT`
  fail-closed). 선언의 값어치는 값 전달이 아니라 **혼동 지점에서 의미를 읽히게 하고 변경 시 리뷰를
  강제하는 것**이다(판정표의 tripwire **정당** 칸).
- **fail-closed 방향은 언제나 dormant** 다 — topology 미해소·`sub_mode` 미확정이면 배달은 열리지 않는다.

**절별 적용 범위** — 아래 표가 §2.7.1–§2.7.7 을 토폴로지로 가른다:

| 절 | multi | single | 싱글 분기 주석 |
|---|---|---|---|
| §2.7.1 권한 평면 A/B | ✔ | ✔ | 평면 정의는 토폴로지 무관(승인 vs 소실방지) |
| §2.7.2 저작/스캔/정비 3범주 | ✔ | ✔ | **무단 스캔 금지는 양쪽 공통** — A2A 불투명 피어 원칙이 같은 결론을 낸다 |
| §2.7.3 B0–B3 상태 표 | ✔ | ✔ **B1 배달 ✔**(2026-09-03 라이브) | 싱글의 B1 은 **에이전트 환경 오버레이**(CLAUDE.md·comms·runtime 스킬·블랙박스·위임키) 배달이다. **빌드킷 평면은 `sub_mode=a2a-agent` 에서 설계상 dormant** — 싱글 서브는 자기 빌드킷을 자율 저작한다(불변식 A). 판정은 `node_role_contract` 의 `delivery_plane` 이 소유하고 `sync_to_sub` 는 그 값을 인용만 한다. 2026-09-03 이전의 "B1 ✗" 는 오버레이 평면이 없던 시절의 관측이다(`plan_26090317` P3 실증: 정착·카나리·릴레이 2회·오버레이 재배달) |
| §2.7.4 권위 평면 계약 | ✔ | ✔ | 커밋/인덱스/파일시스템 구분은 토폴로지 무관 |
| §2.7.5 sync 절차 평면 분리 | ✔ | ✔ | 하위행위·게이트 동일. 싱글은 **에이전트 환경 오버레이 평면**이 하나 더 있어 빌드킷 평면과 분리 배달된다(`SKIP_BUILDKIT` · plane-aware checksum · 2026-09-03) |
| §2.7.6 node-identity | ✔ rank+role | ✔ AgentCard | 같은 절 안에서 갈린다(아래) |
| §2.7.7 A2A 제어명령 | ✔ | ✔ | 싱글에서 **더** 중심적이다 — 제어 자체가 A2A Task 이므로 |
| §2.7.8 그라운딩 교환 | ✔ | ✔ | 도서관 비대칭은 토폴로지 무관(메인 단독) |

- **싱글에서 서브를 "제어"한다는 말의 의미**: 메인은 A2A Task 를 발급하고(§2.7.7 제어명령은 그 특수형),
  서브는 자율 수행 후 push-attestation 리포트를 돌려준다. 메인이 서브의 **빌드 산출물을 밀어 넣지
  않는다** — 그건 멀티의 평면이다.


### 2.7.6 노드 정체성(node-identity) — `role` + `rank` 스킴

> 원문: `docs/plan/plan_26081514_노드정체성_명시화_role스킴_PII구조해소.md` §3 (R 스킴).
> **실장 완료** — `scripts/node_blackbox/node_identity.sh`(node_id 단일 해소기, 소비자 4종이 source)
> + `scripts/node_role_contract.py`(rank·sub_mode·정체성 권위). 2026-08-22 `plan_26082214` §4.2 에서
> **토폴로지 축**(rank vs AgentCard)을 얹어 완성했다. 종전의 "⚠ 미실장" 표기는 이로써 해소된다.

**정체성은 두 질문으로 갈린다** — *"이 노드를 무엇이라 부르는가"*(node_id, 경로 성분)와 *"이 노드는
집단의 어디인가"*(rank, 멀티 전용). 앞엣것은 토폴로지 무관이고, 뒤엣것은 **§2.7.0 제1축에서 갈린다.**

#### (a) node_id — 이름 (양 토폴로지 공통)

```text
node_id ::= manifest nodes[].role 슬러그
정규식  ::= ^[a-z][a-z0-9-]{0,31}$          (fail-closed 검증)
경로    ::= docs/logs/<node_id>             (2노드에서 main | sub)
```

| 항목 | 규약 |
|---|---|
| **메인측 권위** | `output/<topology>/manifest.yaml` 의 `nodes[].role` |
| **서브측 권위** | 배달된 **서브 manifest** `output/<topology>/manifest.yaml` 의 최상위 `self_role: sub`(terraforming 실측·발급 · §2.7.10). 2026-09-05 이전에는 `Agent_Card.json:node_identity.role` 이었으나 Agent_Card v2 는 A2A 평면 계약이라 정체성 필드를 갖지 않는다 |
| **기본값** | **없음.** 해소 실패 시 `$(hostname)` 로 떨어지지 않고 **fail-loud 종료** |
| **해소 우선순위** | ① 명시 `--node-id=<slug>` ② `output/*/manifest.yaml` 의 `self_role`(서브측 · 여러 manifest 가 갈리면 fail-loud) ③ `output/*/manifest.yaml` 의 유일한 `role: main` ④ fail-loud |
| **hostname 의 남은 자리** | gitignored 렌더 산출물의 **비권위 속성**(`Agent_Card.json:supportedInterfaces[].url` 의 호스트 성분 · 서브 manifest `nodes[].hostname`)뿐. **경로 성분·문서 산문에는 등장 금지** |
| **의미 스코프** | **클러스터 스코프**(누구의 디스크에서 보든 같은 이름). 메인의 `docs/logs/sub/` = 서브 미러, 서브의 `docs/logs/sub/` = 정본. 경로 동일, 권위만 다름 |

- **기본값 제거가 스킴의 핵심이었고, 지금은 제거되어 있다.** 옛 `NODE_ID="$(hostname)"` 은 헌법이 금지한 *"결정·게이트·안전 경로의 침묵 폴백"*(§결정론 규율 4종 안티패턴 판정표의 **결함** 칸)이었다 — 틀려도 조용히 새 로그 트리를 만들고, 워치독은 아무도 안 보는 곳에 기록하며, 관측 공백은 사고가 나야 발견된다. 현행 배선은 `ni_resolve_node_id` 하나가 해소하고 **실패하면 죽는다**(`node_identity.sh:ni_resolve_node_id` 마지막 분기 = 옛 hostname 자리).
- **각자 파싱 금지.** 소비자(`install_node_blackbox.sh`·`verify_node_blackbox.sh`·`purge_host_safety.sh`·`multinode_serve_smoke.sh`)는 이 파일을 source 하고 `ni_resolve_node_id` 만 부른다 — 5개 스크립트가 제각기 `$(hostname)` 을 파생하던 것이 원래 문제였다.

#### (b) rank — 위치 (**멀티 전용** · 신설 2026-08-22)

```text
rank ::= manifest nodes[] 배열 인덱스 (0..n-1)
권위 ::= scripts/node_role_contract.py :: resolve_rank(topology, nodes, role)
```

| 토폴로지 | rank | `source` 값 | 근거 |
|---|---|---|---|
| **multi** | `nodes[]` 인덱스 | `manifest-nodes-index` | NCCL "n개 디바이스 각각에 0..n-1 의 고유 rank" 차용 |
| **single** | **`None`** | `not-applicable:single-a2a-agent` | A2A 는 집단이 아니다 — 정체성은 AgentCard이지 위치가 아니다 |

- **싱글에서 rank 를 조용히 0 으로 채우지 않는다.** 그건 "싱글 sub 가 집단의 0번"이라는 거짓말이고,
  하류가 그 값을 TP·NCCL 입력으로 오해할 수 있다. `None` + 출처 문자열이 **음성정직** 표기다
  (`staleness_gate` 의 `skipped:*` 선례와 동형).
- ⚠ **알려진 경계 — N>2**: 현행 role 어휘는 `{main, sub}` 닫힌 목록이라 **서브가 2대 이상이면 슬러그가
  겹치고 node_id 도 겹친다**(로그 트리 혼합). `resolve_rank` 는 조용히 첫 항목을 고르지 않고
  `RANK_AMBIGUOUS_ROLE` 로 죽는다. 어휘를 넓히려면 **세 공동 소유자가 함께** 바뀐다 —
  `node_role_contract.py`(SUB_MODE/role) · `scan_node.py` manifest 게이트 · `node_identity.sh:NI_MAIN_ROLE`.
- multi 에서 `role: main` 이 인덱스 0 이 아니면 **위반이 아니라 note** 다(Ray/NCCL 은 head 가 배열
  첫 항목일 것을 요구하지 않는다). 다만 관례와 어긋나므로 침묵하지 않는다.

#### (c) 렌더 산출물에 실릴 형태 — ✅ **배선 완료**(2026-08-22 · W-2 → 2026-09-05 Agent_Card v2 로 이관)

Agent_Card v2 는 A2A 1.0.1 표준 필드만 최상위에 둔다. 토폴로지 축 해소값은 표준이 정한 확장 자리
`capabilities.extensions[]` 의 **`urn:easy-vllm:ext:node-role:v1`** 하나에 **값과 출처를 함께** 싣는다 —
`rank`/`sub_mode` 는 파생값이라 출처 없이는 하류가 측정·선언·파생을 구분하지 못한다(헌법 §결정론 규율 "출처 표시").
검증기 = `scripts/agent_card_contract.py`(§2.7.10 · 서명하지 않는다):

```jsonc
"capabilities": { "extensions": [ {
  "uri": "urn:easy-vllm:ext:node-role:v1", "required": true,
  "params": {
    "topology": "single",
    "sub_mode": "a2a-agent", "sub_mode_source": "derived-from-topology",   // 선언되어 일치하면 declared-and-agrees
    "rank": null, "rank_source": "not-applicable:single-a2a-agent",          // multi 면 nodes[] 인덱스
    "identity_authority": "agent-card", "delivery_plane": "dormant",
    "tool_plane": ["vllm-recipe-explorer", "adversarial-benchmark", "upstream-version-watch"],
    "tool_plane_source": "sub-mode:a2a-agent"
  } } ] }
```

- 값의 산출 권위는 `node_role_contract.py` 이며 렌더러는 그것을 **적기만** 한다(두 번째 파생 구현 ✗).
  배선: `render_sub_env.py::_contract_placeholders` 가 `evaluate_manifest` 를 불러 네 값을
  `{{SUB_MODE}}`·`{{SUB_MODE_SOURCE}}`·`{{SUB_RANK}}`·`{{SUB_RANK_SOURCE}}` 로 옮긴다.
  **회귀핀**: `render_sub_env.py --self-test` 가 렌더 산출물과 판정기 산출의 **문자 그대로 일치**를
  검사한다(`testlog_26082215` S-11 FAIL 재발 차단).
- `rank` 는 JSON **정수 또는 null** 이지 문자열이 아니다 — 템플릿에서 따옴표 없이 치환된다.
  `"null"` 로 실으면 하류가 rank 를 문자열로 읽어 TP·NCCL 입력으로 오해할 수 있다.
- 계약 위반(예: single 에 `sub_mode: ray-worker` 선언)이면 네 값이 비고, 렌더러는 **필수 필드 누락**
  경로로 **fail-loud 종료**한다(빈 정체성 렌더 금지). 위반 코드도 함께 출력한다.
- 카드의 서술 자체도 토폴로지로 갈랐다(W-3): `topology_contract.{a2a-agent,ray-worker}` 블록이
  분리돼 있고, 종전의 *"현재 브랜치로 분기"*·*"메인 정본 빌드를 byte-equiv 재현"* 문구는 제거됐다
  (브랜치 추론은 헌법 금지이고, single 서브는 재현자가 아니라 **자율 저작자**다).
- `hostname` 은 여기에만 남고 **경로 성분이 되지 않는다.**
- **PII 구조 해소**: `hostname` 이 경로에서 사라지면 `spark-host` 패턴은 애초에 걸릴 것이 없다. `docs.md` 의 `<node_id>` 규약은 문자 그대로 유효하게 남는다(개정 대상은 규약이 아니라 `<node_id>` 의 **정의**뿐).
- **예외**: `--confirm=CRASH-$(hostname)` 파괴적 확인 토큰은 PII 스캔 평면 밖이라 **의도적으로 hostname 을 남긴다**(경로·기록은 role, 파괴적 확인만 호스트). 최종 결정은 해당 plan 의 G2.


## node_role_contract 메모 (옛 §4 에서 이관)

- `scripts/node_role_contract.py` — **토폴로지 축 노드 계약의 단일 소유자**(§2.7.0·§2.7.6b). `evaluate --topology <t> --field {sub_mode,rank,identity_authority,delivery_plane} --format {json,value}` · `--self-test`. 배달 평면 판정의 **정본**이며 `role: sub` 존재로 추론하지 않는다. ✅ 소비자 배선 완료(2026-08-22): `sync_to_sub.sh:_single_extension_active`(배달 평면) · `render_sub_env.py::_contract_placeholders`(Agent_Card 4필드). ⚠ venv `-S` shim 을 포함한 `load_yaml` 을 자체 보유한다 — `staleness_gate._load_yaml` 과 **같은 shim 이 두 곳에 있다**. 지금은 의도된 비결합(preflight 게이트가 이 파일 부재로 죽지 않게)이며, 갈라지면 신호는 두 파서의 판정 불일치로 온다.
