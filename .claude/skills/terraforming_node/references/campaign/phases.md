# 캠페인 — 노드 오케스트레이터 Phase 감독 · phase 전이

> terraforming_node 스킬 reference — SKILL.md §2.7.9 (+ workflow.md 캠페인 phase 절 이관분) 의 본문이다. SKILL.md(라우터)는 § 번호·제목·포인터만 든다.
> 이관 전 원문: `git show dcb713a:.claude/skills/terraforming_node/SKILL.md` (plan_26093022 Step 4 — 본문 바이트는 그대로 옮겼다).

### 2.7.9 노드 오케스트레이터 — Phase 감독 (신설 2026-09-05 · `plan_26090516` §7.4 · H1)

> **브랜치별 오케스트레이션 전략은 특화층이 소유한다** — `references/orchestration.topology.md`
> (같은 경로가 반대 브랜치에서는 다른 내용이고 브랜치 동기화가 옮기지 않는다 ·
> policy:BRANCH_CONSTITUTION_LAYERING). 이 절은 토폴로지 무관한 감독 기계만 갖는다.

축 C/D 결정(2026-09-05): 런타임 3종 스킬이 도는 Phase 에는 **`output/**` 산출물이 메인↔서브 데이터 통신으로
오가지 않는다.** 완수 뒤 발행 Phase 에만, 발행에 필요한 정보가 **문서 평면으로** 서브→메인에 온다.
헌법의 "상향 회수 = 문서기반" 불변식은 개정하지 않는다 — 오케스트레이터는 그 불변식을 Phase 로 집행한다.

| Phase | 서브 | 메인(오케스트레이터) | 노드 간 이동 |
|---|---|---|---|
| **install** | (사람 request) 클론 → 카나리 | terraforming: `--peer-ssh` 실측 → 서브 manifest 발행(`scan_node.py --emit-sub-manifest`) → Agent_Card v2 렌더·구조검증 → **설치 오버레이** 배달(`sync_to_sub --provision`) → model-less 카나리 | 설치 산출물(오버레이)만 |
| **config · build · serve · bench** | 자율(도서관 인용은 `library_request[]` 로 요청) | 릴레이 감독(원장 append-only · 재개 결정은 에이전트 · §2.7.7) | **없음** — `output/**` 이동 ✗ · 이미지 전송 ✗ |
| **(모든 phase 전이)** | `campaign_init --write-brief` 로 `docs/logs/<node_id>/campaign_brief.json` 갱신 | 미러에서 그 파일 하나만 읽는다(`sub.campaign.brief`) | 브리핑은 기계판독 데이터 평면이다(산문 규약 밖) |
| **publish** | `docs/` 에 문서 발행: sweep map · benchmark 인증서/리포트 · devlog/testlog · **hint 입력 사이드카** `docs/benchmark/hint_inputs_<measured_utc>/`(렌더된 Dockerfile·compose·3+1+1·바깥영역 산출물의 사본) → task-report 에 경로 | `fetch_sub_docs.sh` 로 `docs/` 회수(simlog raw 제외) → devlog·benchmark 저작(서브 결과 신뢰 — raw 재요구 ✗) · hint 사이드카는 **참조용**이다(2026-09-22 `plan_26092119`: 메인 `hint.py publish` 는 읽지 않는다 — 발행 자격은 관측이라 서브 단독 셀은 `HINT_QUALIFICATION_UNOBSERVED` · 열린 설계) | 문서 평면만 |

- **책임 = terraforming_node** (사용자 결정 · 발견≠소유). 인증서가 무엇을 담는가는 `adversarial-benchmark`,
  hint 가 무엇을 싣는가는 `hint-publisher` 가 소유한다 — 오케스트레이터는 경계·Phase 전이·형식 검사만 집행한다.
- Phase 전이의 근거는 서브 task-report 의 `phase`·`status`(schema-valid)다. 메인은 서브 디스크가 아니라 **리포트와 문서**를 검증한다.

## phase 전이 · producer 경로 (workflow.md 에서 이관)

### phase 전이 — proof 술어가 다음 배선을 연다

한 셀(= 버전×모델 1조합)은 노드별로 `build → serve → bench → publish` 를 지난다. 각 phase 는
`campaigns/<id>/phases/<node>/<phase>.status.json` 에 **자기 결과와 proof** 를 적는다.

> **2026-09-07 정정**(`plan_26090715` §4.4 · 사용자 결정): 이 표는 **진행표**이지 진입 게이트가
> 아니다. 종전 문장("다음 phase 는 앞 phase 의 `proof.ok` 가 참일 때만 진입한다")은 그것을 집행하는
> **실행자가 0** 이었다 — 교착이 아니라 침묵 누락이었고, 헌법 노드제어 ③("처방을 누가 실행하는가를
> 먼저 적는다")이 이 자리에서 비어 있었다. 실차단은 `completion_gate`·purge 게이트·노드축 게이트에만
> 둔다. 대신 진행표는 **검증기 P1~P3** 이 읽고, 그 판정이 purge 선행조건이 된다.

| phase | 입력(앞 아티팩트) | 출력 | proof 술어 |
|---|---|---|---|
| `build` | `campaign.yaml` 의 matrix 행 · `cells/<cell>/config.yaml` | 이미지 태그·digest | 이미지가 실재하고 `--gpus=all` 기능 프로브 통과 |
| `serve` | build status · `cells/<cell>/lockset.json` | health·엔진 로그 경로 | health 200 + 추론 1회 성공 |
| `bench` | serve status | `docs/benchmark/` 리포트(+PASS 면 인증서) | 리포트 실재 + `measurement_ok` |
| `publish` | bench status | 메인: 셀 hint 태그 1개(`hint.py continue` · 셀의 승인 기록 필요) · 서브: `hint_inputs` 문서 평면 참조 사이드카(발행기는 읽지 않는다 — 서브 단독 셀 발행은 열린 설계) · 증거 포인터 | 메인: push 뒤 원격 태그 오브젝트 SHA = 로컬(`refs/tags/<tag>@<sha>`) · 서브: 포인터 전수 실재 |

- **proof 는 선언이 아니라 관측이다** — `ok: true` 옆에 `source`(그 판정을 낸 명령·파일)를 함께
  적는다. 출처 없는 `ok` 는 단언이 검증을 대체한 것이고, 그러면 깨진 순간을 아무도 모른다.
- phase 가 실패해도 status 파일은 **쓴다**. 부재와 실패는 다른 사실이며, 부재만 남기면 "돌지 않았다"와
  "돌다 죽었다"가 구분되지 않는다.

### producer 경로 파생 — 기본값을 루트로 두지 않는다

캠페인 중 산출물을 만드는 실행자는 자기 출력 경로를 **활성 캠페인 선언에서 파생**한다. 루트 기본값은
남기지 않는다 — 남기면 선언을 잊은 실행이 조용히 루트에 쓴다(누출 21개의 직접 원인).

| 실행자 | 옛 기본값 | 파생 경로 |
|---|---|---|
| `vllm-recipe-explorer` `recipe.py --config` | `/config.yaml` | `campaigns/<id>/cells/<cell>/config.yaml` |
| recipe lock-set | `/lockset.json` | `campaigns/<id>/cells/<cell>/lockset.json` |
| `adversarial-benchmark` `broad_search.sh --state` | 호출자 임의 | `campaigns/<id>/sweeps/<sweep>.json` |
| `terraforming_node` `relay.py` 원장 | `/tasks/` | `campaigns/<id>/relay/` (활성 캠페인 없으면 `_bootstrap`) |
