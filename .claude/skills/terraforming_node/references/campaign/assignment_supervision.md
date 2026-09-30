# 캠페인 — 배정 계층 · 전이 모드 · 감독 스텝

> terraforming_node 스킬 reference — workflow.md §배정 계층. 캠페인 절차의 **정본**은 이 폴더다(workflow.md 는 불변식 · CLAUDE.md 는 "왜").
> 이관 전 원문: `git show dcb713a:.claude/rules/workflow.md` §캠페인 아티팩트 체인 (plan_26093022 Step 4 — 본문 바이트는 그대로 옮겼다). 불변식은 workflow.md 에 남는다.

### 배정 계층 · 전이 모드 · 감독 스텝 (2026-09-08 신설 · `plan_26090813` §4.1·§4.3)

> 왜: 선언의 실행 목록이 **평면 `order`** 였다. 평면 목록은 "메인이 A·B, 서브가 C·D" 를 표현하지
> 못했고, **표현할 수 없는 것은 배선될 수 없다** — 그래서 병렬로 돌 수 있었던 캠페인이 순차로 돌았다
> (2026-09-07 실측: 메인 2셀 완료 → 팝업 → 응답 대기 70분 → 서브 위임).

- **배정의 단일 권위는 `assignments`** 다: `{"<node_id>": [{"cell": "<id>", "mode": "AUTO|HITL|STAY"}, …]}`.
  리스트 순서가 그 노드의 실행 순서이고, **서로 다른 노드의 리스트는 동시에 돈다**. `hint_targets[].cells`
  는 여기서 파생된다(검증기가 부분집합을 검사한다). 옛 `order` 는 대체됐다. 캠페인 셀의 **발행 게이트**는 선언 확인
  팝업에서 받아 `campaign_init.py --hint-approve` 로 적은 `hint_targets[].approval`(셀별 사전 Y/N · 2026-09-21 O6)이다 — 셀마다
  멈추지 않되 무인 자동 태깅은 없다.
- **전이 모드**는 셀이 끝난 뒤의 행동이다 — `AUTO`(기본·생략 가능) = 정리 후 다음 셀 · `HITL` = 정리 후
  사람에게 묻고 대기(무인이라도 기다린다) · `STAY` = 벤치 뒤에도 서빙 유지. **STAY 는 각 노드 리스트의
  마지막에만** 올 수 있다(뒤에 셀이 남으면 그 셀은 영원히 돌지 않는다 · python 수준 검사).
  STAY 상주 컨테이너는 다음 캠페인 init 이 감지해 **예산 재확인 팝업**을 띄운다(자동 조정 ✗).
- **셀 하나 = 릴레이 context 하나**: `<camp>-<node>-<cell>` 이 서빙→벤치를 담고 빌드는
  `<camp>-<node>-build` 별도 문맥이다. 원장은 `campaign_node`·`campaign_context_kind`·`campaign_cell` 을
  **선언으로** 든다(이름 추론 ✗ — 이름을 바꾸면 술어가 조용히 눈이 먼다). 2026-09-07 에는 셀 둘을 한
  context 에 묶어 attempt 2회가 모두 시간 캡에서 잘렸다.
- **착수 순서와 셀의 단위는 토폴로지가 정한다** — 노드별 리스트를 동시에 돌릴 수 있는지, 서브 지시서가
  메인 첫 셀보다 먼저여야 하는지는 특화헌법
  (`.claude/skills/terraforming_node/references/orchestration.topology.md`)이 소유한다. 술어 P4 가 그
  시각 순서를 본다.
- **감독은 상주가 아니라 한 걸음**이다(`relay.py --supervise-step <camp>`): 원장과 회수된 브리핑을 읽고
  판정해 원장에 `supervisor_step` 을 적고 끝난다. 깨우는 손은 하네스의 예약 wakeup 또는 다음 세션의
  재개다(세션 리볼빙의 응용). 판정 4분기 — `completed`→다음 셀 · 중단∧전진→**자동 재발급** ·
  중단∧정체→팝업 · 통신·모델 붕괴 또는 선언된 비용 상한→팝업. 전진은 phase 변화 · 브리핑
  `last_utc` · 원장(리포트 산출물·남은 일) · 회수 문서 중 하나다(phase 만 보면 긴 벤치 한 판이 정체로
  보인다 · 신호 넷의 정의는 `terraforming_node` SKILL.md §2.7.7).
- **진행을 막는 자리는 셋뿐이다** — 선언 확인 팝업(캠페인 시작 직전) · purge 게이트 · 발행 게이트.
  나머지 결손은 **기재하고 진행한다**. 결정론이 과하면 캠페인이 그 자리에서 무한히 선다(사용자 경계).
  셀 **하나**의 측정 진입을 막는 셀 출처 precheck 는 이 셋과 층이 다르다(2026-09-14 · `plan_26091407`
  §4.0): `broad_search.sh cell` 은 캠페인 셀의 `lockset.json` 이 없거나 출처 표시(`provenance` ∈
  `explorer-phase2` · `hand-authored`)가 없거나 목록 밖일 때 exit 2 로 멈춘다(스윕 셀 키가 캠페인 셀 id
  와 갈라지면 그 이름의 lockset 을 못 찾아 같은 거부가 난다). 해소는 그 셀 lockset 에 출처를 표시하는
  것이다 — 파일이 없으면 explorer materialize 또는 `hand-authored` lockset 저작, 셀 키는 캠페인 셀 id 와
  같은 이름(손작성도 표시하면 통과한다). 판정자가 판정하지 못한 경우(검증기 부재·예외)는 같은 exit 2
  라도 **판정 불가**로 따로 알린다 — 라벨 수정이 처방이 아니다.
  선언과 서빙 실물의 불일치는 막지 않고 `cell.status.provenance_mismatch[]` 에 기재하며, **메인이 관측
  가능한** 배정 셀의 표시 전수는 합격 술어 P6 가 관측한다(purge 선행조건 ✗ · 메인 인스턴스의 서브 배정
  셀은 서브 진입 precheck 가 집행하고 P6 는 `ⓘ 관측 대상 밖` 줄로 이름만 남긴다).
