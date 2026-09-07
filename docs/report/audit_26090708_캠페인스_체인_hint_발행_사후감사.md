# audit_26090708 — `campaigns/` 아티팩트 체인 · hint 발행 입력 · 유예 결함 8건 사후감사

> **유효맥락**: single-node HEAD `86a7adc` · 2026-09-07 08시 KST · 대상 = `campaigns/**`(뼈대 + 라이브 인스턴스
> `camp-26090617-gb10-native`) · `terraforming_node`(campaign_init · validator · relay) · `hint-publisher`(hint_tag ·
> hint_collect · 발행 계약 v4) · `adversarial-benchmark`(broad_search · run_bench · parse_guidellm) · 캠페인 ⑦ 이 기록만
> 하고 유예한 결함 8건.
> **선행**: `plan_26090616`(체인 신설) → `plan_26090617`(캠페인 ⑦) → `testlog_26090706`(열벽 · 종결판정) →
> `devlog_26090706`(서사) → hint 태그 3종 발행(`70e729a` · `80d80a3` · `86a7adc`).
> **방법**: pr-review-toolkit 3에이전트(code-reviewer × 2 · silent-failure-hunter)를 축별로 분리 투입 + 감사자 직접
> 판독으로 전 인용을 재검증. **코드는 한 줄도 바꾸지 않았다** — 이 문서는 판정이지 처방의 실행이 아니다.
> **후속**: 개선안은 우로보로스 딥 인터뷰로 도출한다(§6 에 넘길 축을 적었다).

## 0. 요지

1. **거처·게이트·등록부는 계약대로 섰고 실제로 작동한다.** purge 게이트가 지금 닫혀 있는 것(`relay_summary` 부재)이
   그 증거다. 갈라진 것은 **채우는 손과 읽는 눈**이다 — README 가 재개 에이전트에게 읽으라는 4개 아티팩트의 저장소 내
   producer 가 0 이고, hint 발행기는 `campaigns/` 도 A2A 리포트도 읽지 않는다. 체인이 없애려던 실패모드(대화 기억이
   정보를 나른다)가 체인 안에서 재생산됐다.
2. **감사자 자신의 결함**: native hint 태그 3종이 hint 브랜치 페이로드 커밋 없이 소스 트리 커밋에 앵커됐다(계약 §6
   위반 · `finalize/seal` 이 검사하지 않아 통과). 재발행 대상이다.
3. **hint 계약 v4 의 "강행 발행"은 코드에서 도달 불가다.** 계약은 "여정 정보만 필수, §3·§4·§5 는 결손 기재 후 발행"
   이라 적었지만 `hint_tag` 가 요구하는 promotion-ready work-manifest 는 `completion_gate` 가 full_benchmark ·
   mode=full · verdict=PASS · 증거 5종을 전부 요구할 때만 나온다. 캠페인 ⑦ b0 이 `EVIDENCE_MISSING:simlog` 로
   막힌 것이 그 실증이다.
4. 유예 결함 8건 중 **7건이 하네스 불변식 계통(B)**, 1건만 자율해소(A). 가장 급한 것은 ⑧ — `run_trial.py` 가 노드
   정체성을 손파싱해 서브의 예산 선언이 `docs/logs/main/` 으로 가고, 그 결과 **서브 워치독이 선언을 못 읽어 무보호
   로드**가 된다(2026-09-05 부터).

## 1. 질문 3축 판정

### 1.1 캠페인 중 기록이 유실을 억제하는가 — **부분**

**실질 원장은 `campaigns/<id>/sweeps/<sweep>.json` 이다.** `broad_search.sh init/cell` 이 자동 기록하며 셀마다
다음을 든다: 이미지 태그 + **digest** · 서빙 좌표(quantization · max_model_len · kv dtype · TP · moe/attn backend ·
gmu · batch) · KV 바이트 · 동시성 벡터 · 오류 분할(server / tool_boundary) · **kill_events**(b0 의 열 트립이 출처와
함께 남았다) · 절삭 사유. 이 파일이 있는 한 "무엇을 어떤 빌드로 어떤 전략으로 재서 어떻게 됐나"는 유실되지 않는다.

**그러나 README 가 재개 에이전트에게 읽으라는 4개는 producer 가 없다.**

| README 읽기 순서 | 저장소 내 producer | 라이브 실태 |
|---|---|---|
| ① `campaign.yaml` | `campaign_init.scaffold` 가 `id`·`plan_ref` 2필드만 기입 | 나머지 전 필드 손저작 |
| ② `phases/<node>/*.status.json` | **없음** | `main-mn` 4 phase 전부 `pending`(캠페인 2 완주 후) · `main/publish` `pending`(태그 3종 발행 후) |
| ③ `cells/<cell>/cell.status.json` | **없음** — 세션 스크래치패드 스크립트가 썼다(세션과 함께 소멸) | b1~b6 `pending` vs 같은 인스턴스의 sweep `serve_failed`(b1·b4·b5·b6) / `measured`(b2·b3) |
| ④ `evidence_pointers.json` | **없음** — 손저작 | 18건 실재하나 `relay_summary` 부재 |

- **재개 에이전트는 README 를 믿고 b1~b6 을 다시 돌린다.** "cell_outcome 이 pending 인 셀만 남은 작업이다"가 README
  의 문장이다. 두 자리(sweep · cell.status)가 갈라졌고 검증기는 그것을 묻지 않는다.
- **phase 게이트("proof.ok 가 참일 때만 다음 phase 진입")는 실행자 0** — 어느 스크립트도 앞 phase 의 status 를 읽고
  거부하지 않는다. 교착이 아니라 **침묵 누락**이다(막힘 3분류). 헌법 노드제어 ③ "처방을 누가 실행하는가를 먼저
  적는다"가 이 자리에서 비어 있다.
- **검증기는 끝난 캠페인을 표현할 술어가 없다.** `pending` 이 합법 enum 이라 완주 후 전부-`pending` 이 PASS 다.
  sweep↔cell 교차검증 · 증거↔phase 무모순 · 선언 노드 전수(nodes[] 의 `sub`·`sub-mn` 은 `phases/` 디렉터리 자체가
  없다) — 셋 다 저장소 안의 데이터만으로 계산되는데 하나도 없다.
- **`campaigns/ACTIVE`** 가 활성 캠페인 해소의 유일한 입력인데 README · docs.md · root_registry · 뼈대 어디에도
  없다. 인스턴스가 2개인 현 상태에서 재개 에이전트는 `<camp-id>` 를 추론으로 고른다.
- **`campaign.yaml` 의미론 필드(`order`·`budgets`·`control_variables`·`hint_targets`)의 런타임 소비자 0.**
  `broad_search` 는 예산·통제변인·최대 셀 수를 호출자 인자로 다시 받는다. 선언에서 파생되는 것은 경로(`id`)뿐이고
  값은 파생되지 않는다.
- **구조적으로 어디에도 없는 필드**: 셀 레코드의 **대상 모델 id**(최상위 산문과 campaign.yaml 에만) · **측정
  엔드포인트**(chat / completions — 이 캠페인의 가장 큰 발견인데 자리가 없다) · latency 분포(sweep_index 에만).

### 1.2 hint 발행기가 `campaigns/` 와 A2A 평면에서 입력을 모으는가 — **아니오**

- `hint_tag.py`·`hint_collect.py` 에 `campaign`·`relay`·`hint_inputs` 참조 **0**. 입력은 `--tag --topology
  --from-resolved --hf-repo --recipe --manifest(work-manifest)` 이며 arch 세그먼트는 문법만 검증한다.
- 서브는 스키마(`task-report.schema.json#hint_inputs`)대로 `{arch: gb10-sub-native, measured_node: sub,
  evidence[4]}` 를 냈다(relay attempt-04). `relay.py` 는 그 리포트를 파일로 보존만 하고 `record_attempt` 는
  status/phase/context_id 만 옮긴다. **메인 측 소비자 0.**
- 인증서 `measured_node` ↔ 태그 노드축(`<hw>-<main|sub|cluster>-<target>`) 대조가 **어디에도 없다**. 서브 인증서를
  main 태그로 봉인해도 게이트가 울리지 않는다. 라이브 3건이 맞는 것은 같은 에이전트가 기억으로 옮긴 결과다.
- 메인 manifest 에 `self_role` 이 없어 인증서의 `measured_node_source: derived(manifest.self_role=main)` 은 실제로는
  **기본값**이다(출처 오표시 — 헌법 §결정론 규율).
- work-manifest 3종의 `promotion_target`·`runtime.health_ok`·`runtime.containers[]` 는 스크립트 산출이 아니라
  에이전트 손저작 JSON(`publication_id=None` · `evidence_publisher` 미경유)이고, `completion_gate`·`hint_tag` 는
  그것을 신뢰했다(자기 증명).
- SKILL.md §2.7.9 가 약속한 사이드카 `docs/benchmark/hint_inputs_<measured_utc>/` 는 **생산자 0 · 검사 0 · 디스크
  0** 인 명세 고아다. 실제로 데이터를 나른 것은 `report.hint_inputs`(리포트 필드)라 사이드카의 정의가 두 벌이다.
- `hint_collect` 는 채널이 `working-tree(main)` 고정이라 서브 태그의 3+1+1 슬롯을 메인 `output/` 에서 읽는다
  (`--node` 옵션 없음). 계약이 선언 가능 결손으로 열거한 `HINT_MISSING_SUB_TRIPLET` 을 **내는 코드가 없다**.

### 1.3 옛 모호한 역할의 파일이 흡수됐는가 — **입력·원장은 됐고 상태기록자는 안 됐다**

| 구현됨 | 구현 안 됨 |
|---|---|
| `tasks/` → `campaigns/<id>/relay/`(tombstone · `relay_root` 파생 · `_bootstrap`) | cell/phase **상태 producer**(§1.1) |
| 루트 `config.yaml`/`lockset.json` → `cells/<cell>/`(tombstone · `recipe.py --config` 필수화) | `campaign.yaml` 값의 런타임 소비 |
| `root_registry.json` + tripwire ⑦ · gitignore 3규칙 | producer 경로 파생 4행 중 **1행**(relay)만 실배선 |
| purge 게이트(`evidence_pointers` 전수 실재 + `relay_summary`) · 잔재 스캔 | `_bootstrap` 은 `evidence_pointers` 가 없어 purge **영구 닫힘**(옛 원장 5건 잔존) |
| 검증기 `--campaign`·`--instance`·`--template` + 술어 C1~C3 | 릴레이 요청에 캠페인 id 없음 → 서브가 자기 id(`camp7-sub-native`) 저작 · 교차 상관 불가 |
| | `hint_targets[sub].cells: []` 가 서브 인스턴스 부재로 조용히 공집합 해소 · phase `cell_id` 에 산문(`"a0..a9(캠페인1 전 셀)"`) |
| | plan↔구현 이름 표류 4건(`evidence_manifest`/`campaign_contract.py`/`root_registry_tripwire.py`/purge 의 sweep_map 요건) |

`plan_26090616` §7.1 합격기준 중 "phase.status proof 술어 전부 결정론 초록"은 미달(8개 중 5개 `pending`), "도구 호출
로그 감사(`campaigns/<id>/` 밖 읽기 0)"는 검증기 없이 사람 감사로 남았고 라이브에서 실제로 밖(스크래치패드)에서 썼다.

## 2. 발행자 자기결함 — native 태그 3종은 페이로드 커밋이 없다

발행 계약 §6: *태그는 hint 브랜치의 페이로드 커밋을 가리킨다.*

| 태그 | 앵커 | hint 브랜치 조상 | `PAYLOAD.json`/`PROVENANCE.json` |
|---|---|---|---|
| `gb10-sim-h100` 2종(직전 캠페인) | `588fd88` · `5f14d7d` | Y | 실재 |
| `gb10-main-native` | `e3ccfa5`(**multi-node** 소스 커밋) | **N** | **없음** |
| `gb10-sub-native` | `15b9037`(single-node 소스 커밋) | **N** | **없음** |
| `gb10x2-cluster-native` | `80d80a3`(single-node 소스 커밋) | **N** | **없음** |

- `hint_collect → check → hint_branch publish` 를 발행자가 통째로 건너뛰고 소스 트리에 봉인했다. main-native(single)가
  multi-node 커밋에, cluster-native(multi)가 single-node 커밋에 앵커된 것은 그 결과다.
- `finalize/seal/verify` 는 "앵커가 `refs/heads/hint` 조상인가 · `PAYLOAD.json` 이 있는가"를 검사하지 않는다
  (fail-open). `hint_tag.py` 의 일부 문구("본문이 곧 페이로드다")는 v2 시절 것으로 v3 계약과 모순된다.
- **처방 방향**: `finalize/seal` 에 `merge-base --is-ancestor <anchor> hint ∧ <anchor>:PAYLOAD.json 실재 ∧
  PROVENANCE.tag == tag` 를 강제한 뒤 3종을 **재발행**한다(옛 태그 폐기 → 새 태그 · 이력 재작성 ✗). 태그는 아직
  원격에 없다(`origin` 의 `hint/*` 0건).
- 두 번째 정정: 감사자가 직전 세션에서 "⑦·⑧ 완주"로 보고했으나 **purge 선행조건 ②(릴레이 요약 testlog)는
  미충족**이다. `campaign_init.py --verify-purge-gate` 가 닫힘을 정확히 보고한다.

## 3. hint 계약 v4 의 "§3·§4·§5 강행 발행" — 계약과 코드가 갈렸다

사용자 질문: *레시피 §3(모델 서빙 노브) · §4(빌드평면 노브) · §5(성능 baseline) 중 어디까지 fail-closed 로 승격됐나.*

**계약 문장**(`hints/HINT_ISSUANCE_CONTRACT.md` §3·§5 · v4 · 2026-09-06): 필수는 **여정 정보 하나**. 서빙 달성 ·
정량지표(lite/full) · 인증서는 "결손 기재 후 발행". 차단은 양성 검출과 **선언되지 않은 부재**만.

**코드는 세 층이 더 닫혀 있다.**

| 층 | 무엇이 fail-closed 인가 | 어느 절에 걸리나 | 근거 |
|---|---|---|---|
| A. `hint_collect.discover_slots` `exemptible: False` 3슬롯 | **트리플렛 3**(config·runner·env) · **build_recipe**(렌더된 Dockerfile) · **compose**(기동 방법) — 선언과 무관하게 `blocked` | **§3 의 뿌리 · §4 전부** | `hint_collect.py` 슬롯 표 · `dialogue()` "면제 불가 슬롯이 부재하다 — 선언과 무관하게 누락" |
| B. `completion_gate` promotion tier | promotion-ready 는 `task_class == full_benchmark` ∧ `mode == full` ∧ `verdict == PASS`(또는 4필드 waiver) ∧ 증거 5종(plan·devlog·**simlog**·testlog·**bench_report**) ∧ PASS 시 인증서 | **§5 전부**(정량지표·인증서) + simlog | `completion_gate.py` `BASE_REQUIRED_EVIDENCE` · "no other class may reach promotion-ready" |
| C. `hint_tag` 전 서브커맨드 `--manifest` 필수 | create·finalize·seal·verify·push 가 **promotion-ready manifest 없이는 진입 불가** → B 가 곧 hint 의 진입 조건 | 계약 §3 의 "선택" 행 전부를 사실상 필수로 만든다 | `hint_tag.py` argparse `--manifest required=True` |

**결론**: 계약 v4 의 "결손 기재 후 발행"은 §3·§4·§5 어느 절에도 **도달 가능한 경로가 없다.** 인증서 부재 · lite 부재 ·
벤치 리포트 부재 · 서브 트리플렛 부재는 사유코드(`MISSING_CODES`)로 등재만 됐고, 그 코드를 실어 발행에 이르는
길은 B·C 가 닫는다. 캠페인 ⑦ b0 의 `EVIDENCE_MISSING:simlog` 차단이 실증이며, 그때 감사자는 "simlog 만 복사" 조항
으로 vault 사본을 만들어 통과했다 — 계약이 열어 둔 문을 코드가 막고, 사람이 우회로를 냈다(D3 위반의 형태).

- 별도 결함: `--payload` 인자가 CLI 에 없어 `PAYLOAD.json.missing[]` 판독기가 항상 공집합이다. 오류문이 안내하는
  처방("`hint_collect` 가 이 코드를 적게 하라")은 실행 불가하다.
- 이 갈림은 **인터뷰의 첫 축**이다 — 계약을 코드에 맞출 것인가(§3·§4·§5 fail-closed 유지), 코드를 계약에 맞출 것인가
  (강행 발행 경로 개통), 아니면 층별로 다르게 둘 것인가(A 유지 · B/C 완화).

## 4. 유예 결함 8건 — 실재 검증 · 분류 · 처방

B = 매 캠페인 재발 · 호스트 안전/측정 유효성/노드 제어에 닿아 **하네스 불변식 계통**으로 해소. A = 에이전트 자율 우회 가능.

| # | 결함 | 실재 판정 | 분류 · 소속 | 최소 처방 (실행자) |
|---|---|---|---|---|
| **⑧** | 서브 예산 선언이 `docs/logs/main/` | **기전 재특정** — 해소기 `node_identity.sh` 는 정상(서브 manifest `self_role: sub` → 회수 미러 `logs/sub/` 가 증명). 범인은 `run_trial.py` `_budget_node_dir()` 가 `role != "main"` 을 손파싱해 서브에서도 `main` 을 확신 있게 냄. 2026-09-05(멀티)부터 발생 = single 전환 무관. **귀결: 서브 `mem_watchdog_eta` 가 그 선언을 못 읽어 무보호 로드** | **B** · SKILL §2.7.6 "각자 파싱 금지" + HOST_SAFETY · 하드코딩 결함칸 · 오배달 | `node_identity.sh --resolve` CLI 신설(terraforming) → `_budget_node_dir()` 를 그 호출로 교체(explorer) · task-report `node_id` 정의를 role 슬러그로 정렬(현재 호스트명 접미 유입) |
| **①** | harmony 엔드포인트 가드 도달 불가 | **전제 반증 · 결론 성립** — 러너는 `--reasoning-parser` 를 CLI 로 방출한다(47 중 22 매치). 그러나 캠페인 ⑦ 셀은 전부 미매치 → "부재는 통과"가 침묵 폴백. 더 무거운 것: 선언은 "완결 엔드포인트"인데 실제는 `openai-chat`(`/v1/chat/completions` 158 vs `/v1/completions` 14) — `broad_search --backend` 미지정 시 기본값이 이긴다 | **B** · 측정 유효성 · 폴백 결함칸 | 신호원을 러너 선언 → **엔진 로그 실측**으로 · `--backend` 기본값 삭제(fail-loud) · `backend_source` 를 측정 필드로 기록 (adversarial-benchmark) |
| **②** | `RemoteProtocolError` 이중 원인 | 실재. 절단선 = 벤치 종료 시 서버 생존인데 기록 0. 역방향 fail-open 도 있다(엔진 사망 중 잘린 SSE 가 `tool_boundary` 로 면제) | **B** · 측정 유효성 | run_bench 종료 시 `/health` + `docker inspect` → `post_health_<cfg>.json` · parse_guidellm 이 그것으로 면제 무효화 · 인증서에 `server_alive_at_bench_end` (adversarial-benchmark) |
| **③** | 열 워치독 임계 미교정 | 값·출처는 추적 가능(`blackbox_thermal.py` `external_report … UNCALIBRATED`) · 표면화도 됨. **결정하는 소비자 0**(벤치·트라이얼 경로 grep 0). 실킬 3건 + 직전 인증서 1건 오기록. 부수: `--levels` 배선(`e3ccfa5`)이 **multi-node 에만** 있다 | **B** · HOST_SAFETY(정상 차단 — 임계 상향 ✗) | 셀 진입 시 `BB_TP_UNCALIBRATED` 판독 + ack 인자 요구 + 셀 기록 기재(adversarial-benchmark) · `--levels` 브랜치싱크(사람 질의) · 교정 자체는 벤더 근거 → `request/` 위임 |
| **④** | `--serve-failed` 가 `--confirm-risk` 요구 | 실재 · stale 아님. 면제는 정지조건 게이트에만 있고 위험 게이트에는 없다. b1·b4·b5·b6 은 위험 플래그를 붙여 무위험 기록을 남겼다 | **B**(약) · 게이트 의미 희석 · 오배달 | `_NO_LOAD` 파생변수 하나로 두 게이트가 같은 질문을 보게 (adversarial-benchmark) |
| **⑥** | simulate / vault 배타 | 실재. 추가 fail-open: 사전 저작 트리플렛 + simulate → generate 실패로 `run_summary` 없는 vault 가 completion_gate 를 **통과**(비어있지 않은 파일 1개만 검사) | **B** · 증거 무결성 | `--vault-only` 신설 · `_die` 전 run_summary 기록 · completion_gate 가 `run_summary.converged=true` 요구 (explorer · 헌법 runtime) |
| **⑦** | 릴레이 고아 프로세스 | 실재. 원장 필드 · 프로브 0. 회수 미러의 `budget_honored`/`budget_none` 이벤트가 문서기반 생존 신호인데 읽는 코드 0 | **B** · 노드제어 + HOST_SAFETY | task-report 에 `running_services[]`(status≠completed 시 필수) · 소진 후 `intent=probe` 회신 전 `--continue` 거부 (terraforming) |
| **⑤** | `campaigns/README.md` 미배달 | 실재. 지목된 `sync_to_sub` 줄은 잔재 면제이고 실제 누락은 `render_sub_env.py` 오버레이. 서브는 스키마만으로 완주 → **advisory** | **A** | 오버레이 복사 2줄 + fail-loud · `render_sub_env --self-test` 기대 목록 추가 (terraforming) |

## 5. 즉시 조치 순서 (인터뷰 전에도 유효한 것)

1. **릴레이 요약 testlog 발행** → purge 선행조건 ② 닫기(감사자 미완 · 승인 불요).
2. **⑧** — 현재 진행형 무보호 로드를 닫는 유일한 항목.
3. hint `finalize/seal` 앵커 검사 → native 3종 재발행.

나머지는 §6 의 결정 뒤에 착수한다 — 순서를 먼저 정하면 스윗스팟이 아니라 절차가 늘어난다.

## 6. 우로보로스 인터뷰로 넘기는 축 (결정하지 않았다)

설계 목적은 둘이다. (1) 세션 회전(Session Revolving) 중 **초기 정보 유실로 업무 방향이 흔들리는 것**을 막는다.
(2) hint 발행기가 캠페인 자료를 **빠르게 색인**해 토큰노믹스 산출물을 낸다 — 자료 요청을 전부 결정론 게이트로 걸면
발행 불가 또는 캠페인 재실행이 되므로, §3·§4·§5 는 캠페인에 있으면 싣고 없으면 결손 기재 후 강행 발행한다.
목표는 **과도한 절차 없이 유실율을 줄이는 스윗스팟**이다 — 100% 는 기대하지 않는다.

| 축 | 질문 | 이 감사가 준 사실 |
|---|---|---|
| A. 채우는 손 | 뼈대(fill) 방식을 유지할 때 **최소 producer 집합**은 무엇인가 — sweep 기록과 같은 트랜잭션에서 cell.status 를 파생하는 것 하나로 충분한가, phase status 까지 필요한가 | sweep 은 이미 자동 · cell/phase 는 0 · 대상 모델 id · 엔드포인트는 어디에도 없다 |
| B. 게이트인가 진행표인가 | phase "proof.ok → 진입" 문장을 **실행자가 있는 게이트**로 세울 것인가, 사람이 읽는 진행표로 격하할 것인가 | 실행자 0 · 검증기는 완주를 표현할 술어가 없다 |
| C. 계약 vs 코드 | §3·§4·§5 강행 발행을 **개통**할 것인가(B·C 층 완화), 계약을 코드에 맞출 것인가, 층별로 갈라 둘 것인가(A 층 면제불가 3슬롯 유지) | 강행 경로 도달 불가 · b0 가 실증 |
| D. hint 입력 통로 | `report.hint_inputs` 를 단일 사이드카로 확정하고 hint_tag 가 `campaigns/ACTIVE`+`hint_targets`+리포트를 읽게 할 것인가 | 소비자 0 · 디렉터리 사이드카는 고아 |
| E. 노드축 대조 | 인증서 `measured_node` ↔ 태그 노드축 대조를 fail-closed 로 둘 것인가 | 대조 0 · `self_role` 부재 시 기본값이 "파생"으로 표시됨 |
| F. 서브 인스턴스 정체성 | 릴레이 요청에 캠페인 id 를 실어 서브 인스턴스 id 를 메인과 일치시킬 것인가 | 서브가 자기 id 저작 · 교차 상관 불가 |
| G. 수명 | `_bootstrap` purge 예외 · `campaigns/ACTIVE` 문서화 · 인스턴스 1 검사 | `_bootstrap` 영구 닫힘 · ACTIVE 미문서 |
| H. 재발행 절차 | native 3종 재발행을 인터뷰 전에 할 것인가, D·E 결정 뒤 새 경로로 할 것인가 | 원격 미push · 앵커 검사 부재 |

## 7. 문서 · 증거 체인

- 라이브 인스턴스: `campaigns/camp-26090617-gb10-native/`(비추적 · sweeps/ · cells/ · phases/ · relay/ · evidence_pointers.json)
- 서브 회수 미러: `sync_staging/sub_docs/`(`logs/main` 과 `logs/sub` 두 트리 동시 기록이 결함 ⑧ 의 증거)
- 인증서 3종: `docs/benchmark/benchmark_26090618_*` · `benchmark_26090705_*` · `benchmark_26090707_*`
- work-manifest 3종: `docs/_evidence/camp7_*.work-manifest.json`
- 발행 계약: `hints/HINT_ISSUANCE_CONTRACT.md`(v4) · 레시피 템플릿 `.claude/skills/hint-publisher/templates/hint_recipe.template.md`
- 선행 판정: `docs/testlog/testlog_26090706_캠페인7_C2_열벽_SoC_hard_ceiling_판정.md` ·
  `docs/testlog/testlog_26090706_35_20_캠페인7_종결판정_커널축부재_2버전확증.md` ·
  `docs/devlog/devlog_26090706_캠페인7_GB10네이티브_재수행_서사.md`
- 계획: `docs/plan/plan_26090616_캠페인_아티팩트체인_campaigns_신설_hint_재발행_헌법개정.md` ·
  `docs/plan/plan_26090617_캠페인7_GB10네이티브_재수행.md`
