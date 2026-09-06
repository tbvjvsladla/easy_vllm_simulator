# campaigns/ — 캠페인 아티팩트 체인

> **한 줄**: 캠페인의 단계 사이에서 정보를 나르는 것은 대화 기억이 아니라 이 폴더의 파일이다.
> 정책 `policy:ROOT_SURFACE_REGISTRY` · 절차 정본 `.claude/rules/workflow.md` §캠페인 아티팩트 체인 ·
> 문서 규약 `.claude/rules/docs.md` §캠페인 워크스페이스 · 실행 소유 `terraforming_node`.

## 왜 있는가

여러 버전 × 여러 모델을 순차로 도는 캠페인은 한 세션에 끝나지 않는다. 세션이 끊기거나 문맥이 압축되면
**앞 단계가 무엇을 정했는지가 사라지고**, 각 스킬은 자기 기본값으로 되돌아간다. 2026-09-05~06 캠페인에서
그 되돌아감이 실제로 일어났다 — 셀 입력이 저장소 루트에 쌓여 21개가 공개 원격까지 추적 누출됐고,
발행돼야 할 hint 태그 3종 중 2종만 나갔다.

처방은 "잊지 말자"는 규율이 아니라 **거처**다. 날라야 할 것을 파일로 만들고, 그 파일의 자리를 선언에서
파생시키면, 기억이 없어도 다음 단계가 앞 단계의 결정을 읽는다.

## Agent 가 읽어야 할 순서

새 세션이 캠페인 도중에 들어왔다면 **대화 기록을 찾지 말고** 아래 순서로 읽어라. 이 넷이면 재개에
필요한 모든 것이 나온다.

1. **`<camp-id>/campaign.yaml`** — 무엇을 왜 도는가. matrix(버전×모델) · `order`(셀 실행 순서) ·
   `nodes` · `budgets`(예산 선언) · `control_variables`(통제변인) · `hint_targets`(발행할 태그).
2. **`<camp-id>/phases/<node>/*.status.json`** — 어디까지 왔는가. `state` 와 `proof.ok` 를 보고
   **다음에 진입 가능한 phase** 를 정한다. `proof.ok` 가 거짓이면 그 phase 를 다시 돈다.
3. **`<camp-id>/cells/<cell>/cell.status.json`** — 어떤 셀이 끝났고 어떤 셀이 죽었는가.
   `cell_outcome` 이 `pending` 인 셀만 남은 작업이다.
4. **`<camp-id>/evidence_pointers.json`** — 지금까지의 증거가 docs 평면 어디에 있는가.

읽지 **않아도** 되는 것: 릴레이 원장 원문(`relay/`)과 스윕 상태(`sweeps/`)는 실행자가 쓰는 자리다.
사람이 읽을 서사가 필요하면 그것은 `docs/devlog`·`docs/testlog` 에 있다.

## 채우기 규칙

- **`<<FILL>>` 이 남으면 검증기가 fail-closed 한다.** 모르는 값을 그럴듯하게 채우지 말고, 모른다는
  사실 자체를 사람에게 올려라(`relay/pending_hitl.json`).
- **`proof.ok` 는 관측이지 선언이 아니다.** 참으로 적을 때는 `proof.source` 에 그 판정을 낸 명령이나
  파일을 함께 적는다. 출처 없는 `ok` 는 단언이 검증을 대체한 것이고, 그러면 깨진 순간을 아무도 모른다.
- **실패해도 status 파일은 쓴다.** 부재와 실패는 다른 사실이다. 부재만 남기면 "돌지 않았다"와
  "돌다 죽었다"가 구분되지 않는다.
- **증거는 여기서 태어나지 않는다.** 인증서·리포트·sweep map·testlog·devlog 는 `docs/` 평면에서
  발행하고, 여기에는 **포인터만** 적는다. 사본을 만들면 두 자리가 갈라진다.
- **해시를 적지 않는다.** 뼈대는 추적물이라 git 이 이미 바이트를 들고, 인스턴스는 휘발이라 대조할
  두 번째 자리가 없다(`policy:GIT_SINGLE_AUTHORITY` 2문항).

## 구조

```
campaigns/
├─ README.md            ← 이 파일(추적)
├─ _template/           ← 뼈대(추적). 사용자가 관리하는 유일한 부분
│  ├─ campaign.schema.json      선언의 SHAPE 계약
│  ├─ campaign.yaml             선언 틀
│  ├─ cells/_cell/              셀 틀(config.yaml · lockset.json · cell.status.json)
│  ├─ phases/_node/             phase 틀(build·serve·bench·publish · 각자 proof)
│  ├─ relay/                    릴레이 원장 루트(옛 루트 tasks/)
│  ├─ sweeps/                   스윕 상태·정지판정
│  ├─ evidence_pointers.json    docs 평면 증거 포인터(purge 선행조건)
│  └─ residue.json              잔재 스캐너 출력 틀
├─ _bootstrap/          ← 비추적. 활성 캠페인이 없을 때의 릴레이(온보딩·카나리)
└─ <camp-id>/           ← 비추적(휘발). 캠페인 1회의 인스턴스
```

## 수명 — 새 캠페인이 직전 것을 지운다

인스턴스는 **다음 캠페인이 시작될 때** 통째로 지워진다. 그 삭제는 아래가 모두 참일 때만 열린다.

1. 직전 인스턴스의 `evidence_pointers.json` 포인터가 전수 실재한다.
2. 릴레이 요약이 testlog 로 발행돼 있다.
3. 새 캠페인 plan 의 HITL 승인이 있다.

선행조건이 깨지면 purge 가 열리지 않고 **새 캠페인이 시작되지 않는다** — 증거를 흘린 채 다음 캠페인을
도는 것보다 멈추는 편이 싸다. `sync_staging/` 은 purge 대상이 아니다(캠페인과 수명이 다른 상시 자원).

## 서브 노드

서브도 같은 뼈대를 설치 오버레이로 받아 **자기** `campaigns/<camp-id>/` 를 자율 저작한다. 메인이
서브의 인스턴스를 직접 읽거나 고치지 않는다 — 상향 회수는 문서기반이며(`fetch_sub_docs.sh`),
서브의 결과는 `publish` phase 가 만든 문서로 돌아온다(헌법 노드 제어 5불변식 ①).
