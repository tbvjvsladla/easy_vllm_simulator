# 캠페인 — purge 게이트 · 종결

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
