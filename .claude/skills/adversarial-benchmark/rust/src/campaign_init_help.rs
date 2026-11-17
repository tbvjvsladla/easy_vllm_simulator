//! Generated data - do not edit by hand.
//!
//! What `campaign_init.py -h` (and the no-argument run, which prints the same help and exits with 2) prints at 80 columns,
//! captured from the owner program by running it with `COLUMNS=80`. The layout depends on the terminal width: other widths
//! are answered by the owner program.

pub const COLUMNS: u32 = 80;

pub const HELP: &str = r####"usage: campaign_init.sh [-h] [--active] [--derive DERIVE] [--cell CELL]
                        [--sweep SWEEP] [--context CONTEXT]
                        [--campaign-id CAMPAIGN_ID]
                        [--verify-purge-gate PREV_ID] [--close CAMP_ID]
                        [--residue-scan] [--init CAMP_ID]
                        [--plan-ref PLAN_REF] [--purge-previous PREV_ID]
                        [--from-slice PATH|-] [--ground] [--terms TERMS]
                        [--no-warm-start] [--grounding-check]
                        [--lockset-precheck] [--escalation-summary]
                        [--warm-start-library] [--backfill-from-docs]
                        [--assigned-cells NODE] [--emit-slice NODE]
                        [--out PATH] [--import-sub MIRROR] [--write-brief]
                        [--apply] [--selftest] [--suspend-current]
                        [--derived-from CAMP_ID] [--preempt]
                        [--release CONFIG|none] [--dispose CAMP=keep]
                        [--target-topology {single,multi}] [--reason REASON]
                        [--suspend CAMP_ID] [--resume-campaign CAMP_ID]
                        [--accept-drift] [--abandon CAMP_ID]
                        [--summary-testlog PATH] [--answer-query CAMP_ID]
                        [--query-decision {keep}] [--sub-echo CAMP_ID]
                        [--control {suspend,resume,close,abandon,reslice}]
                        [--sub-unconverged-accept CAMP_ID]
                        [--lifecycle [CAMP_ID]] [--phase-set PHASE]
                        [--node NODE] [--state STATE]
                        [--proof-predicate PROOF_PREDICATE] [--proof-ok]
                        [--proof-source PROOF_SOURCE]
                        [--started-utc STARTED_UTC] [--ended-utc ENDED_UTC]
                        [--cell-set CELL] [--cell-materialize CELL]
                        [--set K=V] [--outcome OUTCOME] [--version VERSION]
                        [--model MODEL]
                        [--measurement-decode-tps MEASUREMENT_DECODE_TPS]
                        [--measurement-source MEASUREMENT_SOURCE]
                        [--sweep-state PATH] [--escalation-add]
                        [--symptom SYMPTOM] [--events EVENTS]
                        [--window-start-utc WINDOW_START_UTC]
                        [--window-end-utc WINDOW_END_UTC] [--image-digest SHA]
                        [--image-tag TAG] [--authored-by NODE]
                        [--void-reason VOID_REASON]
                        [--void-reason-source VOID_REASON_SOURCE]
                        [--lite-raw PATH] [--reentry-decide CELL]
                        [--decision {approve,decline}]
                        [--route {explorer,upstream}]
                        [--axis-citation AXIS_CITATION]
                        [--next-intent NEXT_INTENT] [--evidence-add]
                        [--kind KIND] [--path PATH] [--unfreeze]
                        [--evidence-untag] [--untag-reason UNTAG_REASON]
                        [--evidence-relocate] [--to RELOCATE_TO]
                        [--relocate-reason RELOCATE_REASON]
                        [--correct-reference] [--evidence-prune-stubs]
                        [--freeze-evidence] [--assign-set NODE]
                        [--assign-cells ASSIGN_CELLS]
                        [--assign-reason ASSIGN_REASON]
                        [--assign-evidence ASSIGN_EVIDENCE]
                        [--assign-approved-by ASSIGN_APPROVED_BY]
                        [--slice-update PATH|-] [--revise] [--revise-set K=V]
                        [--revise-reason REVISE_REASON]
                        [--revise-evidence REVISE_EVIDENCE]
                        [--revise-approved-by REVISE_APPROVED_BY]
                        [--hint-approve] [--cells A,B] [--approved-by TEXT]
                        [--source declaration-popup|publish-popup] [--utc UTC]
                        [--resume-brief]

campaign_init.sh — 캠페인 인스턴스의 개설·경로 파생·purge 게이트 (fail-closed).

네 가지 일을 한다.

1. **경로 파생** (`--derive`): producer 가 자기 출력 자리를 활성 캠페인 선언에서 얻는다.
   루트 기본값을 남기지 않는 것이 이 스크립트의 존재 이유다 — 남기면 선언을 잊은 실행이 조용히
   루트에 쓴다(2026-09-06 실측: 그렇게 21개가 공개 원격까지 갔다).
2. **purge 게이트** (`--verify-purge-gate`): 직전 인스턴스를 지워도 되는가. 증거가 docs 평면에
   실제로 도착했을 때만 열린다. 열리지 않으면 새 캠페인이 **시작되지 않는다** — 증거를 흘린 채
   다음 캠페인을 도는 것보다 멈추는 편이 싸다.
3. **개설** (`--init ... --apply`): purge → 뼈대 복사 → 활성 포인터 기록.
4. **단일 writer** (`--phase-set`·`--cell-set`·`--evidence-add`·`--revise`·`--hint-approve`)와 **읽는 눈**
   (`--resume-brief`). 2026-09-07 신설 · plan_26090715 §4.1·§4.4. `--hint-approve` 는 2026-09-21
   (plan_26092119 O6) — hint 발행 사전승인을 선언의 `hint_targets` 에 전사한다.
   왜: 거처(campaigns/)와 게이트(purge)는 계약대로 섰는데 **채우는 손이 대화 기억**이었다 —
   README 가 재개 에이전트에게 읽으라는 네 아티팩트의 저장소 내 producer 가 0 이었고,
   라이브에선 세션과 함께 소멸하는 스크래치패드 스크립트가 썼다(audit_26090708 §1.1).
   포맷 소유는 여기 하나이고 호출부는 각 phase 의 실제 실행 스크립트다(포맷 소유 1 · 호출부 N).

무결성 해시는 두지 않는다(`policy:GIT_SINGLE_AUTHORITY` · 사용자 결정 2026-09-06).

options:
  -h, --help            show this help message and exit
  --active              활성 캠페인 id 를 출력
  --derive DERIVE       producer 출력 경로 파생 (root|config|lockset|cell-
                        status|sweep|relay|relay-root|pending-
                        hitl|evidence|phases)
  --cell CELL
  --sweep SWEEP
  --context CONTEXT
  --campaign-id CAMPAIGN_ID
                        파생 대상 캠페인(생략 시 활성)
  --verify-purge-gate PREV_ID
                        직전 인스턴스의 purge 선행조건 검사
  --close CAMP_ID       활성 캠페인 종결 — purge 게이트와 같은 선행조건을 묻고 ACTIVE 만 _bootstrap
                        으로 되돌린다(인스턴스 보존) · --utc 필수 · 기본 dry-run(--apply 로 실행)
  --residue-scan
  --init CAMP_ID        새 캠페인 개설
  --plan-ref PLAN_REF   --init 의 승인 plan 경로(필수)
  --purge-previous PREV_ID
                        --init 과 함께: 먼저 지울 직전 인스턴스(반복 지정 가능)
  --from-slice PATH|-   --init 과 함께: 메인이 보낸 파생 선언으로 campaign.yaml 을 연다 (`-` =
                        A2A 지시서 본문의 stdin JSON)
  --ground              통제변인 파생 질의로 사서를 부르고 grounding/<utc>.json 에 3메시지로 남긴다 ·
                        --utc 필수 (policy:LIBRARY_GROUNDING_FAIL_CLOSED C1)
  --terms TERMS         --ground 의 질의어를 쉼표로 명시(생략 시 통제변인에서 파생)
  --no-warm-start       --ground 전 서가 증분 입고를 생략한다(정지한 서가의 '없음' 은 거짓이다)
  --grounding-check     그라운딩 기록이 성립하는지 묻는다 — 진입 백스톱(C2)이 부른다. 부재는 rc 4(fail-
                        closed) · 활성 캠페인이 없으면 rc 0
  --lockset-precheck    셀 출처 precheck(--cell 필수) — lockset 존재 ∧ provenance ∈
                        목록이면 rc 0, 부재·무효면 rc 2(fail-closed). 활성 캠페인이
                        없으면(_bootstrap) 대상이 아님을 밝히고 rc 0. 판정 JSON 은 stdout,
                        사유는 stderr (broad_search.sh cell 이 부른다)
  --escalation-summary  캠페인 종결 HITL 이 읽는 escalation 후보 요약(JSON) — 판정하지 않고 센다.
                        술어 ① 이 선 후보가 있으면 hitl_required=true
  --warm-start-library  서가 증분 입고만 수행한다(C4 — 발행기·publish 위상 종료부가 부른다)
  --backfill-from-docs  결손 셀의 측정·노드를 스윕 기록·sweep map 에서 사후 수리한다 · --utc 필수
                        (hint 발행 직전 · 차단 ✗ · 남은 결손은 이름을 부른다)
  --assigned-cells NODE
                        그 노드에 배정된 셀을 쉼표로 출력한다(스윕 --cells 의 파생 원천). 손으로 적은 목록은
                        선언과 갈라진다 — 갈라진 목록은 지도를 거짓말하게 만든다
  --emit-slice NODE     그 노드 몫만 자른 파생 선언을 낸다(메인 오케스트레이션) · --utc 필수
  --out PATH            --emit-slice 산출 경로(생략 시 stdout)
  --import-sub MIRROR   회수 미러(sync_staging/sub_docs)를 메인 인스턴스에 편입한다 · --utc 필수
                        (fetch_sub_docs.sh 종료부가 부른다 — 사후 손저작 대체)
  --write-brief         docs/logs/<node>/campaign_brief.json 갱신 · --node --utc
                        필수 (선언된 관측면 — 메인은 이 파일 외에 서브를 읽지 않는다)
  --apply               실제로 쓰고 지운다(기본은 dry-run)
  --selftest

수명 전이 (보류·재개·포기·선점 — journey 단일 권위 · 사람 확인 = --approved-by):
  --suspend-current     --init 과 함께: 활성 캠페인을 보류하고 새 캠페인을 연다(원자 전이 · 처분 없는
                        덮어쓰기는 거부)
  --derived-from CAMP_ID
                        --init 의 파생 부모(기록되면 자식 종결 시 부모 재개 질의가 1회 걸린다 · 계기 a)
  --preempt             진행 중 셀이 있어도 강제 전환(측정 중단 · 서빙 내림 · 그 셀 interrupted ·
                        부분값 증거 ✗)
  --release CONFIG|none
                        보류 시 정식 teardown 으로 내릴 서빙 config(반복) · 띄운 것이 없으면
                        `none`
  --dispose CAMP=keep   무관 개설(계기 d) 때 답 없는 보류 캠페인의 처분 전사(반복 · keep 만 — 포기는
                        --abandon)
  --target-topology {single,multi}
                        새 캠페인의 토폴로지(생략 = 체크아웃). 체크아웃과 다르면 1단(보류+개설 대기)까지만 한다
  --reason REASON       보류·포기 사유
  --suspend CAMP_ID     단독 보류(서브 A2A 보류 지시의 실행부 · 개설 없음) · --release
                        --approved-by --utc
  --resume-campaign CAMP_ID
                        보류 캠페인 재개(핀·digest 대조 · 토폴로지는 체크아웃과 같아야 한다) ·
                        --approved-by --utc
  --accept-drift        --resume-campaign: 보류 동안 바뀐 핀을 사람이 수용
  --abandon CAMP_ID     포기 종결(축소 게이트: 측정 증거 도착 + --summary-testlog) · --reason
                        --approved-by --utc
  --summary-testlog PATH
                        --abandon 의 포기 요약 testlog(저장소 상대경로)
  --answer-query CAMP_ID
                        계기 질의에 '보류 유지' 로 답한다(--query-decision keep) ·
                        --approved-by --utc
  --query-decision {keep}
  --sub-echo CAMP_ID    서브가 자기 인스턴스에서 보류/재개했다는 echo 기록 · --node --control
                        --utc
  --control {suspend,resume,close,abandon,reslice}
  --sub-unconverged-accept CAMP_ID
                        종결된 캠페인의 서브 미도달을 사람이 수용(purge 게이트 탈출구) · --node
                        --approved-by --utc
  --lifecycle [CAMP_ID]
                        수명 fold 를 JSON 으로 낸다(생략 = 전 인스턴스 · 읽기 전용)

writer (campaigns/<id>/ 에 바이트를 쓰는 유일한 자리):
  --phase-set PHASE     phase 상태 기록 (build|serve|bench|publish) · --node 필수
  --node NODE           --phase-set 의 노드 id(= manifest role 슬러그) · --hint-
                        approve 의 선언 노드
  --state STATE         --phase-set 의 상태 (pending|running|done|failed)
  --proof-predicate PROOF_PREDICATE
                        무엇을 물었는가
  --proof-ok            관측 결과 참
  --proof-source PROOF_SOURCE
                        그 판정을 낸 명령·파일 (proof-ok 에는 필수)
  --started-utc STARTED_UTC
  --ended-utc ENDED_UTC
  --cell-set CELL       셀 상태 기록 · --outcome 필수
  --cell-materialize CELL
                        배정 셀의 셀 입력(config.yaml·lockset.json·cell.status.json)을
                        뼈대에서 만든다 · 파생값은 서빙 yaml·env·manifest·/proc/meminfo ·
                        선언 축은 --set · --utc 필수 · 덮어쓰기 ✗(plan_26101622 F1)
  --set K=V             --cell-materialize 의 선언 축(반복 · 닫힌 목록
                        MATERIALIZE_SET_KEYS · 요구 없음 = K=null)
  --outcome OUTCOME     pending|measured|serve_failed|build_failed|void
  --version VERSION
  --model MODEL
  --measurement-decode-tps MEASUREMENT_DECODE_TPS
  --measurement-source MEASUREMENT_SOURCE
  --sweep-state PATH    --cell-set 이 이 sweep 기록에서 측정값·이미지 좌표를 직접 읽는다 (호출부가 값을
                        나르지 않는다 · 없으면 결손 기재)
  --escalation-add      escalation 후보 한 줄을 원장에 쌓는다 · --cell --utc 필수
                        (--symptom 반복 · 술어 ① 은 escalation_predicate.sh 가 판정한다)
  --symptom SYMPTOM     --escalation-add 의 관측 증상(반복 가능). 호스트 평면 사인은 술어 ① 을 만들지
                        못하지만 **기록은 남는다**(부재와 무관은 다른 사실)
  --events EVENTS       --escalation-add 의 시각 대조용 노드 블랙박스 events jsonl(반복 가능)
  --window-start-utc WINDOW_START_UTC
                        --escalation-add 의 대조 구간 시작
  --window-end-utc WINDOW_END_UTC
                        --escalation-add 의 대조 구간 끝
  --image-digest SHA    이 셀을 돌린 이미지의 digest(태그 ✗ — 태그는 이름이고 digest 는 내용이다).
                        --sweep-state 로 파생되면 그쪽이 이기고, 둘이 다르면 거부한다
  --image-tag TAG       사람이 읽는 이름(비교 권위 아님)
  --authored-by NODE    --phase-set: 이 진행표를 적은 주체(서브 자기저작 vs 메인 사후 재저작 판별)
  --void-reason VOID_REASON
  --void-reason-source VOID_REASON_SOURCE
  --lite-raw PATH       --cell-set: 이번 셀의 lite raw — lite 판정을 셀 상태에 싣고 ② 면 cap
                        차감 1(멱등)·재발동 제안을 적는다
  --reentry-decide CELL
                        재발동 제안에 대한 사람 결정 기록 · --decision --approved-by --utc
                        필수(approve 는 --route 도)
  --decision {approve,decline}
  --route {explorer,upstream}
  --axis-citation AXIS_CITATION
                        이 셀이 움직인 축의 근거(layer-2 자율)
  --next-intent NEXT_INTENT
                        여정 한 줄 — 다음에 무엇을 할 참인가
  --evidence-add        증거 포인터 추가 · --kind --path 필수
  --kind KIND           증거 종류 ('certificate', 'bench_report', 'sweep_map',
                        'plan', 'testlog', 'devlog', 'simlog',
                        'relay_summary')
  --path PATH           저장소 상대경로(실재해야 한다)
  --unfreeze            동결된 스냅샷에 사람이 명시로 추가
  --evidence-untag      포인터의 셀 귀속 해제(포인터는 남김 · untagged[] 이력) · --kind --path
                        --cell --untag-reason --utc 필수
  --untag-reason UNTAG_REASON
                        --evidence-untag 의 사유(필수)
  --evidence-relocate   포인터 경로 이전(문서 개명 · 옛 경로 부재 ∧ 새 경로 실재일 때만 · relocated[]
                        이력) · --kind --path(옛) --to(새) --relocate-reason --utc
                        필수
  --to RELOCATE_TO      --evidence-relocate 의 새 저장소 상대경로
  --relocate-reason RELOCATE_REASON
                        --evidence-relocate 의 사유(필수)
  --correct-reference   --evidence-relocate: sub relay_summary의 잘못된 캠페인 relay
                        포인터를 docs/testlog 요약으로 정정. 원본 보존·node/cell 불변·이력 기록 ·
                        --node --cell 필수
  --evidence-prune-stubs
                        뼈대에서 딸려온 <<FILL>> 스텁 포인터를 정식 경로로 제거한다 (값이 든 포인터는 건드리지
                        않는다 · 손삭제 대체)
  --freeze-evidence     캠페인 전체 증거 스냅샷을 동결한다(사람 명시 연산 · hint 발행기는 부르지 않는다 — 셀
                        단위 발행 2026-09-21 D9) · --utc 필수
  --assign-set NODE     assignments[NODE] 개정(pending 셀만 · 4필드 필수) · --assign-
                        cells c1[:MODE],c2,… --assign-reason --assign-evidence
                        --assign-approved-by --utc
  --assign-cells ASSIGN_CELLS
  --assign-reason ASSIGN_REASON
  --assign-evidence ASSIGN_EVIDENCE
  --assign-approved-by ASSIGN_APPROVED_BY
  --slice-update PATH|-
                        서브: 메인 파생 선언으로 자기 인스턴스 배정을 바꾼다(relay reslice 의 실행부) ·
                        --utc --apply
  --revise              layer-1 control_variables append-only 개정(메인 단일 창구)
  --revise-set K=V      개정할 키=값(반복)
  --revise-reason REVISE_REASON
  --revise-evidence REVISE_EVIDENCE
  --revise-approved-by REVISE_APPROVED_BY
  --hint-approve        hint 발행 사전승인을 선언 hint_targets 에 전사한다 · --node --cells
                        --approved-by --utc 필수(넷 중 하나라도 없으면 승인이 아니다) · 메인 선언
                        전용
  --cells A,B           --hint-approve 의 승인 셀(쉼표 · 명시 열거 · 그 노드의 배정 셀이어야 한다)
  --approved-by TEXT    --hint-approve · --reentry-decide 의 사람 발화 전사(이름·연락처 대신
                        역할 — hint 는 봉인 시 페이로드로 복사된다)
  --source declaration-popup|publish-popup
                        --hint-approve 의 승인 자리(생략 = declaration-popup · 선언 확인
                        팝업이 O6 기본 경로)
  --utc UTC             시각은 주입만 받는다(벽시계 금지)
  --resume-brief        인스턴스만 읽고 재개 요약을 낸다(README 읽기 순서 0번)
"####;
