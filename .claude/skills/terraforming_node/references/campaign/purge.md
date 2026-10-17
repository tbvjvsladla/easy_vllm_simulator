# 캠페인 — purge 게이트 · 종결 · 수명 전이

> terraforming_node 스킬 reference — workflow.md §purge 게이트. 캠페인 절차의 **정본**은 이 폴더다(workflow.md 는 불변식 · CLAUDE.md 는 "왜").
> 이관 전 원문: `git show dcb713a:.claude/rules/workflow.md` §캠페인 아티팩트 체인 (plan_26093022 Step 4 — 본문 바이트는 그대로 옮겼다). 불변식은 workflow.md 에 남는다.

### purge 게이트 — 지우기 전에 증거가 docs 평면에 도착했는가

새 캠페인 init 은 **직전 인스턴스를 통째로 지운 뒤** 시작한다. 그 삭제는 아래 선행조건이 모두 참일
때만 열린다(fail-closed).

1. `campaigns/<직전>/evidence_pointers.json` 의 포인터가 **전수 실재**한다(인증서·리포트·sweep map·
   testlog·devlog). 증거는 docs 평면에서 **태어나므로** 인스턴스를 지워도 살아남는다 — 이 검사는
   "정말 거기서 태어났는가"를 묻는 것이다.
2. 릴레이 요약이 testlog 로 발행돼 있다(원장 원문은 휘발이지만 서사는 남는다).
3. **P1~P3 이 초록이다**(2026-09-07 신설 · `campaign_template_validator.instance_predicates`).
   P1 = sweep 레코드와 `cell.status` 가 같은 말을 한다 · P2 = 벤치 증거가 도착한 노드의 bench
   phase 가 그 사실을 반영한다 · P3 = 선언된 노드 전수에 `phases/` 가 있다. 셋 다 인스턴스 안의
   데이터만으로 계산되는데 종전에는 하나도 없었고, 그래서 **완주 후 전부-`pending` 이 PASS** 였다.
4. 삭제 실행자는 메인이고, 게이트는 새 캠페인 plan 의 HITL 이다.
5. `_bootstrap/relay/` 의 옛 원장도 새 캠페인 init 때 **함께 비운다**(사용자 결정 2026-09-07).
   `_bootstrap` 은 캠페인 밖 대기실이라 증거 포인터를 갖지 않으므로 purge 게이트의 대상이 아니지만,
   방치하면 다음 캠페인의 정지판정과 섞인다.

- 선행조건이 깨지면 purge 는 열리지 않고 **새 캠페인이 시작되지 않는다**. 증거를 흘린 채 다음 캠페인을
  도는 것보다 멈추는 편이 싸다.
- **종결은 purge 없이 ACTIVE 만 내린다**(`campaign_init --close <id> --utc <T> --apply` · 2026-09-15). 선행조건은
  위 purge 게이트와 같고, 인스턴스는 남아 다음 init 의 `--purge-previous` 가 지운다. 이것이 **캠페인 사이 창**을
  여는 유일한 정식 경로다 — 없으면 종결된 캠페인이 ACTIVE 로 남아 반대 토폴로지 체크아웃의 4자일치가 막히고,
  남는 길은 ACTIVE 손편집(우회)뿐이다.
- `sync_staging/` 은 purge 대상이 **아니다** — 서브 docs 회수 미러는 캠페인과 수명이 다른 루트 상시
  자원이다(등록부 참조).
- 완료 조건은 잔재 스캔 0 이다: 루트에 등록부 밖 항목 0 · 직전 `campaigns/<id>/` 부재.

### 수명 전이 — 보류 · 재개 · 포기 · 선점 (2026-10-17 · plan_26101719)

A 를 진행하던 중 성격이 다른 B 로 넘어가야 할 때가 있다(예: 3모드 테스트 A 에서 모드3 동작 조건 탐색 B 를 파생).
종전에는 그 경로가 없었다 — close·purge 는 "다 끝났다"를 요구하고, `--purge-previous` 없이 `--init B` 를 치면
ACTIVE 만 덮이고 A 가 이름 없는 잔재가 됐다. 이제 수명은 **journey 이벤트가 단일 권위**이고 상태는 `campaign_init`
이 fold 로 파생한다(별도 상태 필드 ✗). 확인: `campaign_init.py --lifecycle [<id>]`.

| 상태 | 뜻 | 들어오는 전이 |
|---|---|---|
| `active` | ACTIVE 슬롯을 쥔 하나 | `campaign_opened` · `campaign_resumed` |
| `suspended` | 재개 약속이 걸린 보류(자원은 반납) | `campaign_suspended` · 미결에 keep 답 |
| `resume_pending` | 파생 자식이 닫혀 재개 질의가 걸림(미결) | `campaign_resume_pending`(계기 a) |
| `closed{completed\|abandoned}` | 종결 — 상태와 사유를 분리한다 | `campaign_closed` |
| `orphan` | 이벤트 없음 + ACTIVE 미지목(옛 허점의 잔재) | — · 재개 ✗ · `--abandon` 으로만 정리 |
| `unknown` | 이 버전이 모르는 이벤트(브랜치 간 버전 차) | — · 쓰기 전부 거부 · 공통층 동기화 |

- **단일 정문**: `--init <B> --plan-ref <P> --suspend-current [--derived-from <A>] --release <config|none>
  --approved-by <발화 전사> --utc <T> --apply`. A 보류와 B 개설은 사람 확인 1회의 원자 전이다. 처분 없이 활성
  캠페인을 덮는 `--init` 은 거부된다. 실행 순서는 검사 전부 → teardown → 쓰기이고, teardown 이 실패하면 아무것도
  쓰지 않는다(ACTIVE 불변).
- **협조가 기본**: running 셀이 있으면 거부한다. 사람이 강제를 고르면 `--preempt` — 서빙을 내리고 그 셀을
  `cell_interrupted` 로 남긴다(부분값은 증거 ✗). 재개하면 그 셀은 pending 으로 돌아가 처음부터 다시 돈다.
- **보류하면 자원은 항상 반납**한다(정식 teardown · compose down 직접 ✗). 내린 자원·핀·중단 셀은 suspended
  이벤트에 실리고, docs 평면에 PARKED devlog 1건이 난다(인스턴스가 사라졌을 때 사람이 재구성하는 서사).
- **재질의 계기는 둘뿐이다**: (a) 파생 자식의 종결(completed·abandoned 모두 · 부모에 재개 미결 1회) ·
  (d) 무관한 캠페인 개설(답 없는 보류분 → `--dispose <id>=keep`). 리볼빙·새 세션·시간 경과는 계기가 아니다.
  "이미 답함" 은 (캠페인, 마지막 상태 전이) 단위다. 평시에는 `--resume-brief` **첫 줄**의 상태 한 줄만 보인다.
  무인 세션에서 미결이 걸리면 그 줄에만 남고 묻지 않는다 — 사람이 있는 세션에서 한 번 묻는다.
- **답하는 명령**: 재개 `--resume-campaign <id>` · 보류 유지 `--answer-query <id> --query-decision keep` ·
  포기 `--abandon <id> --reason … --summary-testlog <docs/testlog/…>`.
- **포기는 축소 게이트**다: 측정된 셀의 증거가 docs 평면에 도착 + 포기 요약 testlog 1건. P1~P3 미완분은 면제다.
  **보류 상태로는 purge 할 수 없다** — 종결 사유가 기록된 뒤에만 purge 대상이 된다.
- **재개 게이트**: 보류 당시 이미지 digest 를 대조한다(어긋나면 `--accept-drift` 또는 abandoned). 토폴로지는
  drift 가 아니다 — 체크아웃과 다르면 무조건 거부하고, 해당 브랜치로 전환한 뒤 재개한다.
- **토폴로지 교차는 2단**이다: ① 현 브랜치에서 보류 + 개설 대기(`--target-topology`) → ② 미커밋 0 확인 후
  대상 브랜치 checkout → 같은 `--init <B> --plan-ref <P> --utc <T> --apply`. 비추적 `campaigns/` 는 워킹트리에
  남으므로 양 브랜치가 같은 journey 를 접는다.
- **보류 중 쓰기**: 진행성(`--cell-materialize`·`--cell-set`·`--phase-set`·`--reentry-decide`)은 거부, 증거 정리와
  읽기는 허용. 감독 `relay.py --supervise-step` 도 보류 캠페인의 전진 판정을 `hold` 로 바꾼다.
- **서브(single · A2A)**: 메인 전이 뒤 `relay.py --campaign-control suspend|resume --campaign-id <id> --node <sub>`
  가 서브에 같은 전이를 지시하고, echo 가 수렴하면 메인 journey 에 `sub_control_echo` 가 적힌다. 미도달이면
  "서브 미수렴" 으로 남고 메인은 진행한다(차단 ✗). 서브 회수(`--import-sub`)는 서브의 수명 이벤트를 메인 journey 로
  편입하지 않는다.
- 사람 확인 자리(전환 · 재개 · 처분 · drift 수용 · 포기)는 **선언 확인 팝업의 종류**다 — 진행을 막는 자리는 여전히 셋이다.
- 범위 밖(후속): assignments writer(셀 추가·은퇴) · 이미 보낸 서브 slice 갱신 · 보류 기한(사람이 계기가 아니라고 결정).
