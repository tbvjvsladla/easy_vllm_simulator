---
name: adversarial-benchmark
description: >-
  실행 중인 vLLM serve 의 성능을 *적대적으로* 검증해 "기대 이하"를 게이트에서 차단한다. 결정론 루프라인
  (R_fp/R_token, spec-aware) + 외부 레퍼런스(E) + 사용자(c) 3중 루브릭으로 무장한 Devil's Advocate 가
  "이 서빙은 충분히 빠르다"는 주장을 공격 → 기각 시 recipe-explorer 를 자극해 전략 폐기·재탐색
  (loop-until-done). "서빙 성능 검증해줘", "벤치마크 돌려줘", "이 모델 너무 느린데", "디코드 속도 정상이야?",
  "기대 이하 성능 검증", "성능 게이트" 같은 지시에 발동. 측정=`vllm bench serve`(결정론), 적대 판정=LLM.
  실서빙을 기동하지 않는다(돌고 있는 serve 를 검증 — 기동은 recipe-explorer/compose). 근거: 적대적 검증
  패턴(Pattern 3) + 목표달성까지 반복(Pattern 6) · plan_26063014 · self-preference 사건 testlog_26063004.
---

# adversarial-benchmark

서빙 에이전트(recipe-explorer)의 **"떴다 = 잘됐다"** 주장을, **결정론 루프라인 + 외부 레퍼런스 + 사람**의
3중 루브릭으로 무장한 **Devil's Advocate**가 적대적으로 공격해 **기대 이하 성능을 사람이 눈치채기 전에
게이트에서 차단**하고, 기각 시 recipe-explorer 를 **자극해 전략을 폐기·재탐색**시키는(loop-until-done)
**런타임블럭 검증 스킬**. 동기 = self-preference 사건(에이전트가 DeepSeek 15 t/s 를 "천장"으로 자기-선호 →
사용자 외부 레퍼런스가 반증; testlog_26063004). **기능 스모크는 *작동*만, 이 스킬은 *성능*을 별도 게이트.**

> **§0.0 진입 전제 — 테라포밍-완수 Flag 게이트 (헌법 §테라포밍-완수 Flag 게이트 따름정리 · plan_26063018)**: bench·verdict 는 *돌고 있는 serve* 전제 — 그 serve 자체가 recipe/upstream(=Flag)을 거쳤다(**전이적 게이트**). 직접 진입 시에도 Flag 확인: 미발급이면 **info-only**(루프라인 개념 설명 OK / bench·verdict ✗) → "벤치할 serve 가 없음 — recipe/upstream(테라포밍 Flag) 먼저". **결정론 백스톱** = `run_bench.sh` 진입 `manifest_contract.py --require-flag`. **(b) 외부검색 arm**: 결정론 백스톱(`run_bench.sh`)이 실제로 검사하는 것은 **위임 키 ∨ Flag 한 축**이다 — 종전 서술의 "이중게이트(∧ egress-online)" 중 두 번째 문은 **코드에 없었다**(2026-09-04 감사 실측: manifest `network.egress` 를 읽는 소비자 0). egress 는 스캔 시점 attestation 이며 불일치는 fail-open 경고다. 그 값은 이제 **서브 페르소나로 전달**되어 서브가 자기 도달성을 알고 판단한다(`EGRESS_STATE` · `plan_26090412` B8). 그리고 2026-09-04 부터 **서브도 웹 도구를 직접 갖는다**(B안) — 서브는 자기 검색을 수행하고 그 이력을 `external_search[]` 로 회수한다. 위임 키 = `.claude/a2a_delegation.json` 1차 / `EASY_VLLM_A2A_DELEGATED` 2차 — `run_bench.sh` **fail-closed**(키·MC·Flag 모두 부재 → exit4).

> 설계 원칙(하네스 엔지니어링): **측정·루프라인·게이트는 결정론 스크립트**(`scripts/`). LLM 은 **외부검색(E)·
> 정성 진단·재탐색 힌트**만 생산해 결정론 게이트에 투입한다. PASS/REFUTE 는 규칙이 결정(LLM 다수결 아님).

## Contract

- **Goal** — 돌고 있는 serve 의 디코드 성능을 3중 루브릭(루프라인 R · 외부 E · 사용자 c)으로 적대 검증해 PASS/REFUTE 를 결정론으로 판정하고, 기각 시 재탐색 힌트를 낸다.
- **When to invoke** — "성능 검증/벤치마크" 지시 · recipe 서빙 성공 직후 lite 자동 핸드오프 · 멀티노드 VRAM 밸런스 의심 · (별도 오퍼레이션) Max envelope 특성화 승인 시.
- **Inputs** — `config.yaml`(대상 config_name·`reference_tps`/`target_tps`/`tolerance`/`realistic_fraction`) · **루브릭 authority 상태 3종**(§2 — 기본 `weak`, 사용자 HITL 트리거 시 `explicit` 또는 `explore`) · 라이브 serve(`:PORT/health` 200) · manifest(gpu_model·interconnect·topology) · 모델 config/safetensors index.
- **Outputs** — `verdict.json`(PASS/REFUTE/NEEDS_RUBRIC/INVALID + failure_axis + next_strategy_hint) · lite 채팅 표(inform-only) · full 종결 시 `docs/benchmark/` report(항상) + 인증서(PASS시만).
- **Mandatory procedural spine** — 아래 §Mandatory procedural spine 의 7단계(순서 고정).
- **State transitions** — full PASS + 인증서로 `promotion-ready` 의 성능 조건을 채운다(lite 는 어떤 상태도 진행시키지 않는다). 최종 상태 판정은 `.claude/policies/runtime/completion_gate.py` 소유.
- **HITL/safety boundaries** — **serve 를 기동하지 않는다**(미가동 시 중단·보고) · 모델 자동 다운로드 ✗ · 무승인 escalate/rebuild ✗ · 무한 기각 ✗(cap → Model-C) · 게이트는 규칙(LLM 다수결 ✗).
- **Failure → reference routing** — 아래 §Failure → reference routing 표(증상 → 정확 경로).
- **Deterministic commands** — `scripts/roofline.py` · `run_bench.sh` · `parse_bench.py` · `verdict_rule.py` · `lite_bench.sh` · `lite_metrics.py` · `sweep_bench.sh` · `render_report.py` · `publish_benchmark_record.py` · `max_envelope.sh` · `render_max_report.py`.
- **Handoff contract** — REFUTE → `vllm-recipe-explorer` 재탐색 자극(`next_strategy_hint`, 에이전트 매개) · 구조적 → `upstream-version-watch` escalation · 증거 조회/입고 → `wiki-desk`(sidecar).
- **Owns (state)** — `bench-verdict` · `bench-report` · `bench-certificate` · `max-envelope`

## Mandatory procedural spine

필수 순서다 — 루브릭을 **먼저 세우고** 측정하고 **마지막에 규칙으로 판정**한다(순서를 바꾸면 자기-선호가 들어온다).

1. **Flag/게이트 확인**(§0.0) — `run_bench.sh` 진입 백스톱. 미가동 serve 면 **기동하지 말고 중단·보고**.
2. **루프라인 (a)**(결정론, spec-aware) — `roofline.py` → `R_fp`/`R_token`/`expected_achievable`.
3. **serve 가동 확인** — `:PORT/health` 200. 로그 grep 금지(거짓양성).
4. **측정 M** — `run_bench.sh` → `parse_bench.py`(warmup 폐기 + engine-log 교차).
5. **외부 레퍼런스 (b) E** — Devil's Advocate 가 `references.md` warm-start → 검색 → 결과를 `--e-search {hit,empty,no}` 로 **기록**. 미시도 상태로 6단계 직행 ✗.
6. **판정(결정론 게이트)** — `judge_bench.sh <config> --authority {weak,explicit,explore}` (roofline→verdict 체인 · 권한 인자 필수 · 기본값 없음). 판정 규칙 자체는 `verdict_rule.py --authority {weak,explicit,explore}`(§2 — 기본 `weak`; 사용자가 목표를 HITL 명시했을 때만 `explicit`; 사용자가 *광범위 탐색/목표 미설정*을 HITL 지시했을 때만 `explore`). PASS → done-게이트 클리어 / REFUTE → 기각 리포트 + `next_strategy_hint` → recipe 재탐색 → 3단계로(cap 한정, **`explore` 에서는 해제** — 다음 항목으로 진행) / NEEDS_RUBRIC → (c) 사용자 백스톱.
7. **종결 발행** — cap 소진 or PASS 로 종결되면 사람용 report(항상) + 인증서(PASS시만) 발행(`references/lite-and-publication.md` §2).

## Failure → reference routing

| 실패 신호 | 라우팅 대상 (정확 경로) |
|---|---|
| PASS/REFUTE/NEEDS_RUBRIC 판정 규칙·우선순위·밸런스 축 | `.claude/skills/adversarial-benchmark/scripts/verdict_rule.py` |
| 루브릭을 못 세움(E 빈손·spec 축 혼동) · 렌즈 설계 · 측정 사양 | `.claude/skills/adversarial-benchmark/references/rubric-and-lenses.md` |
| lite 스냅샷 이상치 · full 종결 report/인증서 발행 절차 | `.claude/skills/adversarial-benchmark/references/lite-and-publication.md` |
| 안전-최대 컨텍스트 특성화(reload 반복·하드다운 위험) | `.claude/skills/adversarial-benchmark/references/max-envelope.md` |
| M ≪ expected 이고 재탐색으로도 미달(구조적 의심) | `.claude/skills/upstream-version-watch/references/failure-recovery.md` |
| 이전 동일 모델/HW 성능 증거를 먼저 확인하고 싶음 | `.claude/skills/wiki-desk/SKILL.md` |

## 2. 루브릭 authority 모델 — 트리거 기반 (2026-08-15 신설 · `plan_26081514` Q4/Step 5)

> **왜 필요했나**(본 세션 실증): 이 스킬은 **약한 권한**만 갖고 있었다 — E(외부 레퍼런스)가 무조건
> 정본이라, 사용자가 HITL 로 목표를 명시해도 E 가 이겼다. R0~R3 사다리에서 **E=41 t/s 가 REFUTE**
> 를 내는데 **사용자 목표 30 t/s 는 PASS** 인 상태가 되어, 진행하려면 매번 `perf_waiver` 서명이
> 필요했다. 루브릭 정의는 **이 스킬이 소유**하므로 권한 전환도 여기서 정의한다.

| 상태 | 루브릭 **정본** | 발동 조건 | `verdict_rule.py` |
|---|---|---|---|
| **약한 권한**(기본) | **E**(외부 레퍼런스) | 사용자가 목표 tok/s **미명시** | `--authority weak`(기본) → E > c > expected. E 부재 ∧ c 부재 ∧ expected 산출불가 → `NEEDS_RUBRIC`(사용자 백스톱) |
| **명시적 권한**(트리거) | **c**(사용자 목표) | ⓐ 사용자가 HITL 로 *"목표 X tok/s"* 명시 **or** ⓑ *"외부 커뮤니티 자료 검색→tok/s 확인→서빙전략 수립"* 지시 | `--authority explicit --target-tps X` → c > E > expected. `--target-tps` 부재 = **fail-closed(exit 2)** |
| **탐색 권한**(트리거 · 2026-08-22 신설) | **E**(외부 레퍼런스) — 단 **문턱으로 쓰지 않는다** | 사용자가 HITL 로 *"목표 0 / NULL / None / 광범위 탐색"* 지시 | `--authority explore` → **E > expected**(`c` 칸 **부재**). `--target-tps` 는 **금지(exit 2)** |

- **트리거 발동 시 c 달성 = PASS** 이며 **`perf_waiver` 서명이 불요**하다(본 세션 B1 마찰의 제거점).

### 2.1 `explore` 계약 — 게이트 미설정 상태 (`plan_26082219` D2·U1)

> **`--target-tps 0` 은 explore 가 아니다.** 상수 0 투입은 **loud reject(exit 2)** 이며 explore 로 자동
> 매핑하지 **않는다** — 자동 매핑은 에이전트/스크립트가 authority 를 전환하는 것이고(트리거는 사용자만
> 당긴다), 산출물에서 사용자가 `weak` 를 의도했는지 `explore` 를 의도했는지 구분 불가하게 만든다
> (표시 없는 권한 전환 금지). 오타(`3O`→파싱실패 0)가 조용히 탐색모드가 되는 것도 같은 이유로 막는다.

- **`c` 칸을 사다리에서 제거한다.** explore 의 정의가 *"목표를 세우지 않는다"* 이므로 `c` 는 존재해서는
  안 되는 값이다. 칸을 남겨두면 "explore 인데 목표가 있다"는 **모순 상태가 표현 가능**해지고, 그 모순이
  다시 침묵 폴백의 자리가 된다. 출력의 `rubric.target_c` 는 explore 에서 **항상 `null`** 이고
  `rubric.candidates[]` 에 `{"source":"c(user_target)","state":"absent-by-authority"}` 로 남는다.
- **`E` 는 explore 에서도 사라지지 않는다** — `weak` 와 동일하게 1순위이며 `rubric.reference_E` 상수
  보존 규칙은 **전 authority 불변**이다(칸 비교의 성립 조건).
- **실질은 오케스트레이션 해제다**: REFUTE 여도 다음 항목으로 진행한다(loop-until-done 해제). 이는 판정
  규칙이 아니라 오케스트레이션 계약이므로 산출물이 `rubric.loop_until_done: false` 로 스스로 밝힌다.
  ⚠ 해제 대상은 *재탐색 강제*이지 `reconciliation_cap` 이 아니다 — §3 의 cap→Model-C 규율은 **불변**이다.
- ★ **hint 발행 자격(승격 기준)이 바뀐다 — `explore` 에서는 성능 판정이 게이트가 아니다**(2026-08-22
  사용자 결정 · `plan_26082219` U1 해소):

  | | `weak`·`explicit` | **`explore`** |
  |---|---|---|
  | 승격 게이트 | 성능 판정 `verdict == PASS`(미달 시 `perf_waiver`) | **서빙 성립(기능)** = 모델이 실제로 떠서 curl 통신으로 응답을 낸다 **∧ 유효 측정**(`floor > 0` ∧ `ratio ≠ null`) |
  | `PASS`/`REFUTE` 의 지위 | **게이트** | **서술** — 벽 지도 데이터(어디서 얼마나 못 미쳤나) |

  - "서빙 성립"의 기계 증거는 `runtime.health_ok ∧ containers[].oom_killed=false ∧
    runtime.functional_smoke_passed` 이며, 이는 `completion_gate.py` 의 **runtime tier 가 이미 강제**한다
    (그 관문을 통과하지 못하면 어떤 authority 로도 evidence-complete 에 못 간다).
  - **공허 PASS 배제 규칙은 어느 authority 에서도 불변이다** — `floor = 0` 은 **어떤 모드에서도** 승격
    불가다. explore 가 무는 것은 *문턱의 높이*이지 *측정의 유효성*이 아니다.
  - 잔여 경계(2026-08-22 현재): 인증서는 헌법(`CLAUDE.md` §불변식 · `.claude/rules/docs.md` §benchmark)상
    **PASS 때만 발행**되므로, explore 에서 `REFUTE` 로 끝난 항목은 여전히 인증서를 갖지 못한다 →
    그 경우의 승격은 종전대로 `perf_waiver`(사람 서명) 경로다. explore-REFUTE 를 인증서 없이 승격시키려면
    **인증서 PASS-only 규칙 자체를 개정**해야 하며 그것은 헌법 개정 사안이다(이 스킬이 단독으로 못 연다).
- ★ **밸런스 축(`--node-vram-gib`)도 explore 에서는 서술이다**(2026-08-22 · U1 후속). 노드간 VRAM
  편차는 decode-tps 축과 **직교**하지만 성질은 같은 *성능 문턱*이므로, explore 에서 그것만 게이트로
  남기면 U1 계약이 옆문으로 뚫린다. 값은 종전대로 전부 기록하되(`balance.pass` · `balance_dev` ·
  `node_vram_gib`) **`verdict` 를 `REFUTE` 로 뒤집지 않는다**.
  - 우회의 실체: 밸런스가 `verdict` 를 뒤집으면 → 인증서가 **PASS 때만 발행**되는 규칙에 걸려 미발행 →
    인증서가 없으면 `completion_gate.py` 의 explore 승격 경로(`cert_rubric_authority=='explore'`)가
    닫힌다 → 결국 **`perf_waiver`(사람 서명)를 강요**한다. 즉 "서빙 성립이면 승격"이 성립하지 않는다.
  - **`weak`·`explicit` 은 종전 그대로 게이트다** — `balance_dev > --balance-tol` → `REFUTE` +
    `failure_axis="balance"` + recipe-explorer loop-back. 이 완화는 **explore 에만** 적용된다.
  - 출력이 스스로 밝힌다(헌법 §결정론 규율 — 출처 표시): `balance.gates_verdict` 가 `true`(게이트) /
    `false`(서술)이며, explore 의 편차 초과는 `refuted_claims` 가 아니라 `diagnosis_hint` 에 **벽 지도
    데이터**로 적힌다(기각 목록과 서술을 섞지 않는다).
- **E 는 사라지지 않는다** — 트리거 상태에서도 `rubric.reference_E` 로 **칸 비교용 상수**로 보존된다.
  R0~R3 같은 사다리 비교는 "모든 칸이 같은 E 로 재어졌다"가 성립 조건이므로, 우선순위만 바뀌고
  **상수 자체는 불변**이어야 한다. E 를 지우면 칸 간 비교가 무효가 된다.
- **트리거는 사용자만 당긴다**(에이전트 자기선언 ✗ — `explicit`·`explore` 둘 다). 근거: c 가 정본이
  되면 낮은 목표로 검증을 우회할 수 있고, explore 는 문턱 자체를 내리므로, 두 전환 모두 **HITL 명시**
  로만 성립한다 — 이것이 남용 방어의 전부다.
- **게이트는 여전히 결정론**이다 — authority 는 *어느 상수를 정본으로 쓸지*만 고르고, PASS/REFUTE
  판정 자체는 `verdict_rule.py` 의 규칙이 한다(LLM 다수결 ✗ — §8 불변).
- 판정 출력의 `rubric.authority` · `rubric.source` 로 **어느 권한에서 잰 판정인지 산출물이 스스로
  밝힌다**(헌법 §결정론 규율 — 출처 표시). 표시 없는 권한 전환은 금지다.

## 3. 두 실패축 (반드시 구분)

| 축 | 의미 | 처리 |
|---|---|---|
| **루브릭 못 *세움*** | (a) 불확실 ∧ (b) 빈손 | → **(c) 사용자 백스톱**(`NEEDS_RUBRIC`) |
| **루브릭 못 *충족*** | M < 루브릭, recipe 재탐색해도 미달 | → `reconciliation_cap` 한정 루프 → 소진 시 **구조적**=escalation 역루프(upstream rebuild) / **전략소진**=음성정직+best-so-far. **무한 기각·무한 루프 금지** |

- 매 iteration = serve(재)기동(recipe/compose) → 벤치 측정 → 적대 판정. teardown 규율 = recipe Phase-2 동일(통합메모리 OOM 보호).
- **누가 누구를 부르나**: 이 스킬은 *측정+판정+힌트*만 소유. **재탐색은 recipe-explorer 가**(에이전트가 `next_strategy_hint` 를 recipe 에 전달 — 스킬↔스킬 직접호출 아님). recipe 측정/serve 재구현 금지.

## 7. 스킬 경계 / 인터페이스

- **↔ recipe-explorer**: recipe 의 측정·serve 인프라를 **소비**, 위에 적대 루브릭/게이트만 얹는다. 기각 시 `next_strategy_hint` 로 재탐색 **자극**(feasibility 탐색은 recipe, performance 목표는 이 스킬이 주입).
- **↔ upstream-version-watch**: "루브릭 못 충족 + 구조적" → **escalation 역루프** 핸드오프(M≪expected + 외부 확증 = 적대 증거). upstream 이 버전핀/rebuild 소유(승인 게이트).
- **↔ wiki-desk**: 진입 시 warm-start(이전 동일 모델/HW 성능 증거 우선소비), 새 testlog 발행 시 입고. wiki 는 **sidecar** — 벤치 report/인증서/verdict 의 **소유자가 아니다**(이 스킬이 소유).
- **블럭 분류 = 런타임블럭(서브 복제)**: 서브가 자기 모델에 자율 실행. **(b) 외부검색 arm = 서브도 웹 도구를 직접 갖는다**(2026-09-04 B안) — 검색 이력은 `external_search[]` 로 메인에 회수돼 자산이 된다. egress 가 `online` 이 아니면 **빈손을 빈손이라고 보고**하고 루프라인-only 로 판정한다(빈손과 미수행은 다른 사실이다). lite 스크립트(`lite_bench.sh`·`lite_metrics.py`)도 런타임블럭이라 **git-tracked 로 서브 자동 전파**.
- **lite 멀티 수집의 A2A 관측 평면**: 서브 `nvidia-smi`/`/proc/meminfo` **읽기전용 probe** 는 health 폴링과 **동형 관측 평면**이지 "서브 작업코드/설정 재스캔·직접교정 금지"와 **다른 평면**이다(관측 ≠ 재스캔·교정).

## 8. 안전 / 금지

- **serve 를 기동하지 않는다**(돌고 있는 serve 검증만). 미가동 시 중단·보고. — 예외는 **별도 오퍼레이션 Max**(reload 를 스스로 소유, 이중 게이트).
  - ★ **이 금지의 목적어는 추론 서버다**(2026-09-04 명문화 · `plan_26090415` §3.2). 원문이 "어떤 컨테이너도"로 읽히면 오늘의 `docker exec` 조차 금지되어 현실과 모순된다. **측정 도구 컨테이너는 teardown 계약 하에 이 스킬이 소유한다** — full 모드의 GuideLLM 이 그것이다. 소유의 기준은 **기동이 아니라 정리**다: 실질 실패모드는 "누가 띄웠나"가 아니라 "크래시가 컨테이너를 흘렸나"이므로 `--rm` + 모든 종료 경로의 teardown trap 이 계약이며, 메모리 예산은 **선언에서만** 오고(`--bench-budget-mib` · 기본값 없음) 커널이 `--memory` 로 강제한다. Max 의 이중 게이트는 여기 적용하지 않는다 — Max 가 이중인 이유는 추론 서버를 reload 해 통합메모리 OOM 이력에 직결되기 때문이고, 측정 도구는 그 축을 건드리지 않는다. 정본 술어는 `policy:HOST_SAFETY_LAYERED_DEFENSE.C10`.
- 모델 자동 다운로드 금지(NAS 부재면 중단). 결정론 스크립트는 외부 네트워크 호출 없음 — **단 검증기 (b) 외부검색은 허용·의무**(모델획득 격리 한정).
- 무승인 자동 escalate/rebuild ✗(escalation 은 승인 게이트). 무한 기각·무한 루프 ✗(cap → Model-C).
- 게이트(PASS/REFUTE)는 결정론 규칙 — LLM 다수결로 결정하지 않는다. lite 는 `verdict_rule` 에 투입하지 않는다(inform-only).
- **공허 PASS 는 구조적으로 불가하다**(`plan_26082219` D1): 사다리 세 칸(E·c·expected) 모두 단일 술어
  `rubric_candidate()` 로 *유한 양수* 검사를 통과해야 낙찰된다 ⇒ `rubric.floor > 0` ∧
  `rubric.ratio_M_over_primary ≠ null` 이 **항상** 성립한다. `--target-tps 0`/`--reference-tps 0`/
  `--tolerance 1` 은 **exit 2 loud reject**(조용한 무문턱 PASS 금지). `verdict_rule.py --self-test` 가
  이 불변식을 상시 단언하고, `verify_distribution.py` 가 매 검증마다 그 자체검사를 **실행**한다.

## 9. 보조 파일

**결정론 스크립트**
- `scripts/roofline.py` — (a) spec-aware R_fp/R_token/expected(manifest+config/index).
- `scripts/run_bench.sh` · `scripts/parse_bench.py` — 측정 M(+engine-log 교차). `--tool vllm|guidellm`(기본 `vllm`)이 도구를 고른다 — **lite 는 언제나 `vllm bench serve`**, full 만 GuideLLM 으로 간다(`full = lite ∪ GuideLLM`). 두 경로를 이질적으로 유지하는 것이 설계다: 통합하면 같은 버그가 양쪽에 균일하게 먹어 **일치해 보이면서 둘 다 틀리는** 상태가 되고, 실제로 그 이질성이 실결함 2건을 잡았다(2026-09-01·09-03).
- `scripts/parse_guidellm.py` — GuideLLM `benchmarks.json` → **`parse_bench` 와 동일 계약**의 측정 M(소비자가 도구를 몰라도 되게). 정본 지표도 그대로 `1000/median(TPOT)`. **spec 축은 추측하지 않고 요구한다** — GuideLLM 은 수용길이를 보고하지 않는데 `verdict_rule` 은 `spec_on` 으로 물리 상한을 R_token/R_fp 중에서 고르므로, 조용한 False 는 거짓 판정이 된다. `--accept-len-src`(같은 스윕 lite 레그에서 승계) 또는 `--spec-axis-absent`(부재 명시) 중 **정확히 하나**가 필수다. **`--self-test`**(G1~G11 · 실측 산출물 픽스처 포함).
- `scripts/verdict_rule.py` — 결정론 PASS/REFUTE 게이트(**`--authority weak|explicit|explore`** = E>c>expected / c>E>expected / E>expected(c 부재)(§2), like-with-like, spec-off 강제함수, 밸런스 축 — **`explore` 에서 밸런스는 게이트가 아니라 서술**(§2.1)). `explicit` + `--target-tps` 부재, `explore` + `--target-tps` 존재, `--target-tps|--reference-tps ≤0·NaN·Inf`, `--tolerance ∉[0,1)` 은 전부 **fail-closed(exit 2)** — 침묵 폴백 금지. **`--self-test`** 로 T1~T16 결정론 자체검사(파일 입력 불요 — T16 = explore 밸런스=서술 회귀).
- `scripts/resolve_bench_tool.py` + `bench_tool_pin.json` — **측정 도구 컨테이너 핀**. 태그와 digest 를 병기하되 **판정 권위는 digest 단일**이다(같은 태그가 노드마다 다른 이미지를 가리킨 실측 선례). 이미지 부재는 **자동 pull 하지 않고 exit 3**(사전 스테이징 계약 — airgap-safe 불변식 보존), digest 드리프트는 **exit 4**(태그가 같아도 통과 ✗). 핀 갱신은 파일을 고쳐 리뷰에 태우는 것이지 실행 시점 흡수가 아니다. **`--self-test`**(P1~P8).
- `scripts/judge_bench.sh` — **루브릭 권한 통로**(roofline → verdict 체인). `--authority` 는 **필수이며 기본값이 없다** — 권한은 사용자 HITL 트리거이지 스크립트의 판단이 아니고, 기본값을 두면 "사용자가 약한 권한을 골랐다"와 "아무도 안 골랐다"가 구분 불가가 된다(이것이 1년간 explore 인증서 0건이었던 원인이다). 권한↔`--target-tps` 조합 규칙은 **복제하지 않고** `verdict_rule.py` 의 exit 2 를 그대로 전달한다. 루프라인 입력은 `sweep_index.json` meta 에서 **승계**한다(파생 복제 ✗).
- `scripts/lite_bench.sh` · `scripts/lite_metrics.py` — lite 오케스트레이터 + 5종 메트릭 렌더(inform-only).
- `scripts/sweep_bench.sh` · `scripts/render_report.py` · `scripts/publish_benchmark_record.py` — full 종결 스윕·report·인증서.
- `scripts/max_envelope.sh` · `scripts/render_max_report.py` — **Max 오퍼레이션**(별도 정체성).
- `scripts/broad_search.sh` — **광의의 탐색 오퍼레이션**(Max 동형 · 별도 정체성 · 이중 게이트 `--confirm-risk` ∧ 챗 Y/N). `init`/`cell`/`status`/`map`. **축을 고르지 않는다** — 축 선택·후보 생성·캠페인 안은 에이전트 판단이고(결정론이 축을 고르면 탐색 품질이 떨어진다), 이 스크립트가 소유하는 것은 게이트·안전·상태·산출물이다. **serve 를 기동하지 않는다** — 셀 materialize 는 explorer 소관이며, 미가동은 `serve_failed` 로 정직하게 기록하고 멈춘다(단일노드에 정규 기동 경로가 없다고 해서 여기서 무보호 `compose up` 으로 메우지 않는다 — D3).
- `scripts/classify_cell.py` — 셀 종결 3분류(`measured`/`serve_failed`/`measurement_void`). `void_reason` 은 자기추론이 아니라 **노드 블랙박스 이벤트와의 시각 대조**로 채우고, 대조 실패는 `unknown` 이다(추측 ✗). dry-run 트립은 사인 후보가 아니다 — 아무것도 죽이지 않았다. 허용오차 기본 0(같은 호스트 같은 시계). **`--self-test`**(K1~K3 · C1~C6).
- `scripts/render_sweep_map.py` — 탐색 지도 렌더러. **순위를 만들지 않는다**(목적함수 ✗ · 파레토 선언 ✗ · 셀은 **실행 순서**로만). 그 금지를 산문이 아니라 `assert_no_ranking` 이 **깊이 무제한 키 검사**로 집행하고, 위반 시 자기 산출물을 스스로 거부한다(exit 3). 미완 지도도 발행한다(`sweep_status: incomplete` · `remaining[]`). **`--self-test`**(R1~R10 · 음성대조 3건).
- `scripts/sweep_stop.py` — **광의의 탐색(Broad Search) 정지 조건 평가기**(`stop ⟸ 남은 셀 0 ∨ 셀 수 예산 ∨ 벽시계 예산 ∨ 연속 실패 한도`). 세 한도는 전부 상태 파일의 `declared_budget` 에서만 오고 **기본값이 없다** — 미선언은 exit 2 다(깊이 HITL 이 숫자를 낳는다). 셀 종결 3분류(`measured`/`serve_failed`/`measurement_void`) 중 뒤 둘만 연속 실패로 세며, `measured` 는 verdict 가 REFUTE 여도 실패가 아니다(잴 수 있었다). 시각은 `--now-utc` 주입만. **`--self-test`**(S1~S16 · 음성 사례 6건). 종료 `0=계속 · 3=정지 · 2=판정 불가`.
- `.claude/skills/wiki-desk/scripts/doc_naming.py` — 발행 명명 SSOT.

**조건부 references(필요할 때만 연다)**
- `references/rubric-and-lenses.md` — 3중 방어막·밸런스 축·`vllm bench serve` 측정 사양·Devil's Advocate 렌즈·config 입력.
- `references/lite-and-publication.md` — lite 모드(기본 ON·inform-only) + full 종결 report/인증서 발행.
- `references/max-envelope.md` — Max 오퍼레이션(컨텍스트 안전측 스텝업·이중 게이트·산출물).

**기타**: `fixtures/`(verdict 단위검증 + full/Max 발행 결정론 체인 라이브-불요 검증) · `config.example.yaml`(입력 스키마).
