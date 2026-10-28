# 캠페인 — 상태를 쓰는 손

> terraforming_node 스킬 reference — workflow.md §캠페인 상태를 쓰는 손. 캠페인 절차의 **정본**은 이 폴더다(workflow.md 는 불변식 · CLAUDE.md 는 "왜").
> 이관 전 원문: `git show dcb713a:.claude/rules/workflow.md` §캠페인 아티팩트 체인 (plan_26093022 Step 4 — 본문 바이트는 그대로 옮겼다). 불변식은 workflow.md 에 남는다.

### 캠페인 상태를 쓰는 손 — 포맷 소유 1 · 호출부 N (2026-09-07 신설 · `plan_26090715` §4.1)

거처와 게이트가 계약대로 서 있어도 **채우는 손이 없으면** 그 자리를 대화 기억이 메운다. 2026-09-06
캠페인이 그렇게 돌았다 — 상태를 쓴 것은 세션과 함께 소멸하는 스크래치패드 스크립트였고, 저장소 안의
producer 는 0 이었다. 이제 바이트를 쓰는 문은 하나이고, 그 문을 **각 phase 의 실제 실행 스크립트**가 부른다.

| 슬롯 | 성립 시점 | 포맷 owner(단일) | 호출부(실행자) |
|---|---|---|---|
| `phases/<node>/<phase>.status.json` | 각 phase 종료 | `terraforming_node` `campaign_init.py --phase-set` | serve: `single_serve_up.sh` health 200 지점 · bench/build/publish: 각 owner 스크립트 종료부 · 단독 lite·sweep 은 `lite_bench.sh`·`sweep_bench.sh --campaign-cell <cell>`(측정 진입 running · 종료 done/failed · EXIT trap · 노드는 배정 파생 · 명시로만 — broad_search 는 자기 트랜잭션이 적으므로 넘기지 않는다 · 2026-10-17 plan_26101722 K5) |
| `cells/<cell>/{config.yaml,lockset.json,cell.status.json}` 생성 | 셀 착수 전(서빙된 셀이면 발행 전) | 동 `--cell-materialize <cell> [--set K=V…] --utc` | 셀을 맡은 노드(메인 · single 서브 모두) — 파생값은 서빙 yaml·env·manifest·`/proc/meminfo`, 선언 축은 `--set`(닫힌 목록 · 요구 없음 = `K=null`) · 빈칸이 남거나 이미 있으면 거부(2026-10-16 · plan_26101622 F1: 서브 hint 2건이 이 단계 없이 셀 입력 결손 3건을 안고 발행됐다) |
| `cells/<cell>/cell.status.json` + `journey.jsonl` | 셀 트랜잭션 종료 | 동 `--cell-set` | `adversarial-benchmark` `broad_search.sh cell`(sweep 레코드를 쓴 **같은 트랜잭션** · 이번 셀 lite raw 를 `--lite-raw` 로) |
| `cells/<cell>/cell.status.json#reentry.decision` + 여정 | lite ② 재발동 제안에 사람이 답한 직후 | 동 `--reentry-decide` | 사람 승인 인자 필수(`--approved-by` 발화 전사) · 제안 없는 셀은 거부 |
| `evidence_pointers.json` | 증거 발행 직후 | 동 `--evidence-add` | `publish_benchmark_record.py`(인증서 발행 지점) · 리포트·로컬 실행계획·testlog·devlog 발행자 |
| `campaign.yaml.revisions[]` | layer-1 개정 | 동 `--revise` | 사람 승인 인자 필수 · **메인 단일 창구**(서브는 릴레이로 요청만) |
| `campaign.yaml.assignments` + `assignment_revisions[]` | 배정 변경(셀 추가·은퇴·순서·mode) | 동 `--assign-set <node>` · 서브 `--slice-update -` | 사람 승인 4필드(`--assign-reason`·`--assign-evidence`·`--assign-approved-by`·`--utc`) · **pending 셀만**(돌았거나 도는 셀 · hint 승인 셀은 거부) · 다른 노드 셀 = 이중 배정 거부(이동 = 제거+추가) · 서브 몫은 `relay.py --campaign-control reslice`(2026-10-17 · plan_26101722 K8) |
| `grounding/<utc>.json` | 캠페인 착수·셀 축 변경 | 동 `--ground` | 서빙·트라이얼 진입 백스톱이 이 파일을 요구한다(`policy:LIBRARY_GROUNDING_FAIL_CLOSED`) |
| `docs/logs/<node>/campaign_brief.json` | phase 전이·publish 마다 | 동 `--write-brief` | 서브 실행 스크립트 종료부 · **메인이 서브 진행을 읽는 유일한 자리** |
| 회수 편입(서브 phase·셀·증거) | 문서 회수 직후 | 동 `--import-sub` | `fetch_sub_docs.sh` 종료부(사후 손저작 대체) |

`--slice-update`는 메인의 승인된 layer-1 개정 이력도 t0와 기존 이력의 접두 일치를 검사한 뒤 그대로 이어 붙인다. 이력이 충돌하거나 후퇴하면 거부하고, 배정이 같으면 배정 이력을 바꾸지 않는다. 적용 뒤 서브의 개정 번호와 값을 직접 확인한다. 기존 relay 문맥에서 layer-1 값이 바뀌면 새 값을 반영한 전체 `--task-file`을 명시해 이어간다.

- **`ACTIVE` 가 `_bootstrap` 이면 writer 는 no-op 이다** — 캠페인 밖 평시 서빙이 빈 인스턴스에 상태를
  쓰기 시작하면 `_bootstrap` 이 캠페인 흉내를 내게 된다.
- **보류·미결(suspended·resume_pending) 캠페인에는 진행성 writer 가 거부된다**(2026-10-17 · plan_26101719) — 증거 정리와
  읽기는 허용된다. 수명 이벤트를 모르는 버전(unknown)이면 모든 writer 가 거부된다. 수명 전이는 `purge.md` §수명 전이.
- **읽는 눈은 `--resume-brief`** 다(README 읽기 순서 0번). **첫 줄은 수명 상태 한 줄**이다(위치 고정). 합격 기준은 그 출력만으로 **다음 셀에
  착수**하고 **반증된 축을 재시도하지 않는 것**이다.
- **여정 한 줄(`--next-intent`)은 새 절차가 아니라 이미 도는 자동쓰기에 얹은 인자 하나다.** 감수하지
  말아야 할 유실은 여정 하나이며, 벤치 결과·3+1+1 산출물은 결손 기재로 복원된다.

### 위임된 계획 권위와 로컬 실행계획

서브 선언의 `campaign.yaml.plan_ref`는 메인에서 승인·선언한 계획을 그대로 가리킨다. 서브에 그 파일이
배포되지 않아도 위임은 유효하다. hint용 서사를 채우려고 메인 계획을 복사하거나 `plan_ref`를 바꾸지 않는다.
서브는 배정된 셀의 기존 승인 범위 안에서 **자기가 수행할 절차와 근거**를 `docs/plan/`에 작성하고,
공식 writer로 `--evidence-add --kind plan --path docs/plan/<문서>.md --cell <셀> --node <서브> --campaign-id <id>`를
등록한다. 이 포인터는 로컬 실행 서사이며, 새 승인이나 `hint_targets`·배정·통제변인 개정의 근거를 만들어내지 않는다.

`--write-brief`와 문서 회수는 이 포인터의 셀·노드 귀속을 유지한다. hint 발행기는 실재하는 계획 후보가
하나여야 진행한다. 로컬 `plan_ref` 파일이 없고 로컬 실행계획 포인터만 있으면 후자를 쓰며, 둘 다 실재하면
기존 모호성 오류를 유지한다(발행기가 임의로 선택하지 않는다). 로컬 실행계획까지 없으면 서사 결손이다.
