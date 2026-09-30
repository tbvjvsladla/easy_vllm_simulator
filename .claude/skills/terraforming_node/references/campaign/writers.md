# 캠페인 — 상태를 쓰는 손

> terraforming_node 스킬 reference — workflow.md §캠페인 상태를 쓰는 손. 캠페인 절차의 **정본**은 이 폴더다(workflow.md 는 불변식 · CLAUDE.md 는 "왜").
> 이관 전 원문: `git show dcb713a:.claude/rules/workflow.md` §캠페인 아티팩트 체인 (plan_26093022 Step 4 — 본문 바이트는 그대로 옮겼다). 불변식은 workflow.md 에 남는다.

### 캠페인 상태를 쓰는 손 — 포맷 소유 1 · 호출부 N (2026-09-07 신설 · `plan_26090715` §4.1)

거처와 게이트가 계약대로 서 있어도 **채우는 손이 없으면** 그 자리를 대화 기억이 메운다. 2026-09-06
캠페인이 그렇게 돌았다 — 상태를 쓴 것은 세션과 함께 소멸하는 스크래치패드 스크립트였고, 저장소 안의
producer 는 0 이었다. 이제 바이트를 쓰는 문은 하나이고, 그 문을 **각 phase 의 실제 실행 스크립트**가 부른다.

| 슬롯 | 성립 시점 | 포맷 owner(단일) | 호출부(실행자) |
|---|---|---|---|
| `phases/<node>/<phase>.status.json` | 각 phase 종료 | `terraforming_node` `campaign_init.py --phase-set` | serve: `single_serve_up.sh` health 200 지점 · bench/build/publish: 각 owner 스크립트 종료부 |
| `cells/<cell>/cell.status.json` + `journey.jsonl` | 셀 트랜잭션 종료 | 동 `--cell-set` | `adversarial-benchmark` `broad_search.sh cell`(sweep 레코드를 쓴 **같은 트랜잭션** · 이번 셀 lite raw 를 `--lite-raw` 로) |
| `cells/<cell>/cell.status.json#reentry.decision` + 여정 | lite ② 재발동 제안에 사람이 답한 직후 | 동 `--reentry-decide` | 사람 승인 인자 필수(`--approved-by` 발화 전사) · 제안 없는 셀은 거부 |
| `evidence_pointers.json` | 증거 발행 직후 | 동 `--evidence-add` | `publish_benchmark_record.py`(인증서 발행 지점) · 리포트·testlog·devlog 발행자 |
| `campaign.yaml.revisions[]` | layer-1 개정 | 동 `--revise` | 사람 승인 인자 필수 · **메인 단일 창구**(서브는 릴레이로 요청만) |
| `grounding/<utc>.json` | 캠페인 착수·셀 축 변경 | 동 `--ground` | 서빙·트라이얼 진입 백스톱이 이 파일을 요구한다(`policy:LIBRARY_GROUNDING_FAIL_CLOSED`) |
| `docs/logs/<node>/campaign_brief.json` | phase 전이·publish 마다 | 동 `--write-brief` | 서브 실행 스크립트 종료부 · **메인이 서브 진행을 읽는 유일한 자리** |
| 회수 편입(서브 phase·셀·증거) | 문서 회수 직후 | 동 `--import-sub` | `fetch_sub_docs.sh` 종료부(사후 손저작 대체) |

- **`ACTIVE` 가 `_bootstrap` 이면 writer 는 no-op 이다** — 캠페인 밖 평시 서빙이 빈 인스턴스에 상태를
  쓰기 시작하면 `_bootstrap` 이 캠페인 흉내를 내게 된다.
- **읽는 눈은 `--resume-brief`** 다(README 읽기 순서 0번). 합격 기준은 그 출력만으로 **다음 셀에
  착수**하고 **반증된 축을 재시도하지 않는 것**이다.
- **여정 한 줄(`--next-intent`)은 새 절차가 아니라 이미 도는 자동쓰기에 얹은 인자 하나다.** 감수하지
  말아야 할 유실은 여정 하나이며, 벤치 결과·3+1+1 산출물은 결손 기재로 복원된다.
