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

새 세션이 캠페인 도중에 들어왔다면 **대화 기록을 찾지 말고** 아래 순서로 읽어라.

0. **`campaign_init.py --resume-brief`** ← **여기서 시작한다**(2026-09-07 신설).
   인스턴스만 읽고 한 화면으로 낸다: layer-1 현재 선언(+개정 수) · 셀 진행표 · 노드별 phase
   진행표 · 마지막 여정 3줄 · **다음 pending 셀** · **반증된 축(재시도 금지)** · 증거 포인터 수.
   아래 1~4 는 이 요약으로 부족할 때 보는 원문이다. 요약만으로 다음 셀에 착수하고 반증된 축을
   재시도하지 않는 것이 이 파일의 합격 기준이다.

**`campaigns/ACTIVE`** — 살아 있는 인스턴스 **하나**의 이름을 담은 한 줄짜리 포인터(비추적).
producer 는 자기 출력 경로를 이 포인터에서 파생하며, 포인터가 없거나 가리키는 디렉터리가 없으면
활성 캠페인은 `_bootstrap` 이다 — **루트가 아니다**. 부재를 루트 폴백으로 처리하면 선언을 잊은
실행이 조용히 루트에 쓴다(2026-09-06 실측: 그렇게 21개가 공개 원격까지 갔다). `_bootstrap` 일 때
campaigns writer(`--phase-set`·`--cell-set`·`--evidence-add`)는 **no-op** 이다 — 캠페인 밖 평시
서빙이 빈 인스턴스에 상태를 쓰기 시작하면 `_bootstrap` 이 캠페인 흉내를 내게 된다.

1. **`<camp-id>/campaign.yaml`** — 무엇을 왜 도는가. matrix(버전×모델) · **`assignments`**(노드별
   셀 배정 = 배정의 단일 권위) · `nodes` · `budgets`(예산 선언) · `control_variables`(통제변인) ·
   `hint_targets`(발행할 태그 · 배정에서 파생).
   `assignments` 는 `{"<node_id>": [{"cell": "<id>", "mode": "AUTO|HITL|STAY"}, …]}` 이고 **서로
   다른 노드의 리스트는 동시에 돈다**. `mode` 는 셀이 끝난 뒤의 전이다 — AUTO(기본·생략 가능)는
   정리 후 다음 셀, HITL 은 정리 후 사람에게 묻고 대기, STAY 는 벤치 뒤에도 서빙을 유지한다
   (STAY 는 리스트 **마지막**에만 온다). 옛 평면 `order` 는 2026-09-08 에 대체됐다 — 평면 목록은
   "메인이 A·B, 서브가 C·D" 를 표현하지 못했고, 표현할 수 없는 것은 배선될 수 없었다.
2. **`<camp-id>/phases/<node>/*.status.json`** — 어디까지 왔는가. `state` 와 `proof.ok` 를 보고
   **다음에 진입 가능한 phase** 를 정한다. `proof.ok` 가 거짓이면 그 phase 를 다시 돈다.
3. **`<camp-id>/cells/<cell>/cell.status.json`** — 어떤 셀이 끝났고 어떤 셀이 죽었는가.
   `cell_outcome` 이 `pending` 인 셀만 남은 작업이다. `provenance` 는 그 셀 lockset 의 출처이고
   `provenance_mismatch[]` 는 선언과 서빙 실물이 갈라진 자리다(아래 §셀 값의 두 계층).
4. **`<camp-id>/evidence_pointers.json`** — 지금까지의 증거가 docs 평면 어디에 있는가.
   `frozen_utc` 가 있으면 publish 위상에서 **동결된 스냅샷**이다(hint 발행의 입력 통로) —
   이후 추가는 사람이 `--unfreeze` 를 붙여야 열린다. 태그는 불변인데 근거가 움직이면 안 된다.
5. **`<camp-id>/journey.jsonl`** — **여정**. 이탈·반증·축 이동 사유와 "다음에 무엇을 할 참"이
   append-only 로 쌓인다. 벤치 결과·3+1+1 산출물은 결손 기재로 복원되지만 여정은 복원되지
   않는다 — 이 체인에서 유일하게 감수하지 말아야 할 유실이다.

읽지 **않아도** 되는 것: 릴레이 원장 원문(`relay/`)과 스윕 상태(`sweeps/`)는 실행자가 쓰는 자리다.
사람이 읽을 서사가 필요하면 그것은 `docs/devlog`·`docs/testlog` 에 있다.

## 채우기 규칙

- **`<<FILL>>` 이 남으면 검증기가 fail-closed 한다.** 모르는 값을 그럴듯하게 채우지 말고, 모른다는
  사실 자체를 사람에게 올려라(`relay/pending_hitl.json`).
- **상태 파일에 바이트를 쓰는 것은 `campaign_init.py` 하나다**(2026-09-07). `--phase-set`·
  `--cell-set`·`--evidence-add`·`--revise`·`--evidence-prune-stubs` 가 유일한 쓰기 문이고,
  호출부는 각 phase 의 **실제 실행 스크립트 종료부**다(포맷 소유 1 · 호출부 N). 2026-09-06 실측:
  상태를 쓴 것이 세션과 함께 소멸하는 스크래치패드 스크립트였다.
- **"손으로 편집하지 마라" 의 범위는 상태 파일이다**(2026-09-08 범위 축소 · plan_26090813 D14).
  선언(`campaign.yaml`)과 셀 입력(`config.yaml`·`lockset.json`)은 **손저작 입력**이다 — 계획
  인터뷰에서 나온 값을 사람이 적는 자리이고, 내용이 부족하면 그때 HITL 로 묻는다. 상태·증거·여정만
  writer 전용이다. 종전 문장은 범위를 말하지 않아 선언을 고치는 것조차 규약 위반처럼 보였다.
- **셀 입력의 값은 두 계층이고, lockset 은 출처를 스스로 밝힌다**(2026-09-14 · plan_26091407 §4.0) —
  아래 §셀 값의 두 계층. 출처 표시가 없는 셀은 측정 진입(`broad_search.sh cell`)에서 멈춘다.
- **뼈대 잔재는 손으로 지우지 말고 `--evidence-prune-stubs` 로 지운다**(2026-09-08 신설). writer 에
  지우는 연산이 없어서 예시 포인터가 게이트를 막았을 때 손삭제가 유일한 경로였고, 손삭제의 흔적은
  "채워야 했는데 못 채운 빈칸"과 구분되지 않았다(사후감사 §A F1 · D3: 우회 대신 경로를 만든다).
- **`proof.ok` 는 관측이지 선언이 아니다.** 참으로 적을 때는 `proof.source` 에 그 판정을 낸 명령이나
  파일을 함께 적는다. 출처 없는 `ok` 는 단언이 검증을 대체한 것이고, 그러면 깨진 순간을 아무도 모른다.
- **실패해도 status 파일은 쓴다.** 부재와 실패는 다른 사실이다. 부재만 남기면 "돌지 않았다"와
  "돌다 죽었다"가 구분되지 않는다.
- **증거는 여기서 태어나지 않는다.** 인증서·리포트·sweep map·testlog·devlog 는 `docs/` 평면에서
  발행하고, 여기에는 **포인터만** 적는다. 사본을 만들면 두 자리가 갈라진다.
- **해시를 적지 않는다.** 뼈대는 추적물이라 git 이 이미 바이트를 들고, 인스턴스는 휘발이라 대조할
  두 번째 자리가 없다(`policy:GIT_SINGLE_AUTHORITY` 2문항).

## 셀 값의 두 계층 — 선언 · explorer 파생 (2026-09-14 · plan_26091407 §4.0)

> 왜: 2026-09 캠페인 셀 11/11 에 `lockset.json` 이 없었고, 서빙 yaml 은 explorer Phase-2 를 거치지 않고
> 손으로 적혔다. 승자 셀의 `max-num-seqs` 는 손레버였고 yaml gmu 0.80 은 config `target_gmu` 0.85 와
> 달랐는데, "측정 산물"이라 적힌 그 값들이 사람이 적은 값이라는 사실을 가리는 자리가 0 이었다.
> 처방은 손작성 금지가 아니라 **표시와 대조**다(사용자 결정).

| 계층 | 누가 정하나 | 무엇 | 자리 |
|---|---|---|---|
| **선언** | 사람(계획 인터뷰) — 적는 것이 정당하다 | context 길이 · 변종 · PLE 모드 · KV dtype · TP · 통제변인 · **동시성 요구**(`concurrency_requirement`) · **KV-fit 대표 요청 길이**(`typical_request_tokens`) · 타겟 gmu(`target_gpu.target_gmu`) | `cells/<cell>/config.yaml` 의 `declared_axes:` 블록 + `target_gpu` |
| **explorer 파생** | `vllm-recipe-explorer` — 측정·재조정한다 | max-num-seqs · KV 클램프(`kv-cache-memory-bytes`) · 서빙 yaml `gpu-memory-utilization` | `cells/<cell>/lockset.json` (+ 생성된 3종 세트) |

- **`declared_axes` 는 explorer 의 입력 슬롯이지 서빙 인자가 아니다.** 지금은 **선언 슬롯일 뿐 읽는
  코드가 없다** — 소비자(explorer 의 `max-num-seqs = min(concurrency_requirement, KV-fit)` 산식 ·
  `declared_axes.tp` ↔ manifest 대조)는 plan_26091407 단계 ② 에서 배선된다. `declared_axes.tp` 는 선언이고,
  TP 의 권위는 여전히 manifest 다 — 둘이 다르면 선언이 틀린 것이다. 통제변인(layer-1)의 정본은
  `campaign.yaml` 이며 셀은 자기 스윕의 통제변인 문장만 적는다(복제하지 않는다).
- **`declared_axes` 의 빈칸도 계약이다** — `--instance` 검증은 배정 셀 config 에 남은 `<<FILL>>` 을
  막는다(위 §채우기 규칙). 모르는 값을 그럴듯하게 채우지 않는다: 요구가 없는 축(lite 만 재는 셀의
  `concurrency_requirement`·`typical_request_tokens` 등)은 `null` 로 **"선언하지 않았다"** 를 적는다.
- **lockset 의 `provenance` 는 필수 표시다** — `explorer-phase2`(explorer Phase-2 절차가 잠갔다) 또는
  `hand-authored`(그 절차 밖에서 사람이 적었다). 뼈대에서는 `<<FILL>>` 로 출발한다. ⚠ **지금 이 값을
  각인하는 코드는 없다** — `recipe.py` 는 lockset 을 읽기만 하고, 표시는 Phase-2 lockset 을 저작하는 쪽이
  적는 **절차 자기선언**이다. 그래서 `explorer-phase2` 는 아직 `hand-authored` 와 기계적으로 구분되지
  않는다(explorer 경로의 기계 각인은 plan_26091407 단계 ② 의 `recipe.py` 편집 범위 · 후속). 예외 노브는
  `*_source` 로 값의 출처를 가른다: `batch_source` ∈ {`declared-requirement`, `kv-fit-measured`,
  `hand-lever`} · `gmu_source` ∈ {`target_gmu`, `hand`} · `kv_source` ∈ {`measured-clamp`, `hand`}. null 은
  "아직 정하지 않았다"이다. 어휘 정본은 검증기 상수이고 뼈대의 `_*_enum` 은 교차검증되는 안내 사본이다.
- **막는 자리는 하나다**: `broad_search.sh cell` 이 측정 진입에서 lockset 부재·`provenance` 부재·목록 밖
  값을 exit 2 로 거부한다(셀 materialize 는 explorer 소관 · 스윕 셀 키가 캠페인 셀 id 와 갈라지면 그 이름의
  lockset 을 찾지 못해 부재로 거부되고, 사유가 어휘 갈라짐 가능성을 함께 말한다). `hand-authored` 는 통과한다.
  캠페인 문맥은 `--state` 가 `campaigns/<id>/sweeps/` 아래면 그 `<id>`, 아니면 `ACTIVE` 가 정한다 —
  포인터 유실(부재·오타)이 캠페인 셀의 게이트를 열지 않는다. 둘 다 캠페인 밖(`_bootstrap`)이면 셀 입력의
  거처가 없어 대상이 아니며, 그 사실을 셀 기록의 `cell_provenance.status=not_applicable` 로 남긴다.
  `--serve-failed` 기록은 트리플렛이 없어 타지 않는다. 판정자가 판정하지 못한 경우(검증기 부재·예외)는
  "출처 표시 없음" 이 아니라 **판정 불가**로 따로 알리고 역시 exit 2 다 — 그때 lockset 라벨을 고치지 않는다.
- **불일치는 기재한다(차단 ✗)**: `--cell-set` 이 서빙 yaml gmu(sweep 좌표)와 선언 `target_gmu` 가 다르면
  `cell.status.provenance_mismatch[]` 에 양쪽 값·출처를 적는다. 이번 호출에서 대조하지 못한 노브(좌표
  NA·PyYAML 부재·sweep 없음)는 **직전 기재를 유지**한다(부재 ≠ 일치). 아직 대조 수단이 없는 노브(batch·KV
  클램프 — explorer 재계산 산식이 들어오면 채운다)는 `provenance.pending_knobs[]` 에 이름으로 남는다 —
  목록에 없는 노브를 "일치"로 읽지 않는다. 메인 인스턴스에 회수 편입된 서브 배정 셀은
  `provenance.observability=not_observable` 이다(부재가 아니라 관측 불가 — 메인은 서브 인스턴스를 읽지 않는다).
- **메인이 관측할 수 있는 배정 셀의 표시 전수는 합격 술어 P6 가 관측한다**(`campaign_template_validator
  --acceptance`) — purge 선행조건이 아니다. 표시가 빠진 셀이 다음 캠페인을 영원히 막으면 그것은
  안전장치가 아니라 교착이다. 메인 인스턴스의 서브 배정 셀은 P6 적색 대상이 아니고(서브 진입 precheck 가
  집행한다) `P6 ⓘ 관측 대상 밖` 줄로 **이름이 남는다**. 노브 `*_source` 목록 밖 값도 `ⓘ` 줄로 기재될 뿐
  P6 를 적색으로 만들지 않는다(P6 의 합격 정의는 provenance 표시다).

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
│  └─ residue.json              잔재 스캐너 출력 틀(`--residue-scan --utc <t>` 가 인스턴스에 쓴다)
├─ ACTIVE               ← 비추적. 살아 있는 인스턴스 하나의 이름(부재 = _bootstrap)
├─ _bootstrap/          ← 비추적. 활성 캠페인이 없을 때의 릴레이 **대기실**(온보딩·카나리).
│                          purge 게이트의 대상이 아니며(증거 포인터를 갖지 않는다),
│                          새 캠페인 init 때 옛 원장과 **함께 비운다**(사용자 결정 2026-09-07).
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
