# Rubric Base · 측정 사양 · Devil's Advocate 렌즈 (조건부 reference)

> spine step 2(루프라인)·step 4(측정)·step 5(적대 판정)의 **세부**. 게이트 결정은 언제나
> `scripts/verdict_rule.py`(결정론)가 하고, 이 문서는 그 게이트에 **무엇을 증거로 넣는가**를 정의한다.

## 1. 3중 방어막 — 루브릭 *세우기*

매번 **(a) → (b) → (c)** 순. `verdict_rule.py` 의 primary 우선순위는 **루브릭 authority 가 고른다**
(SKILL.md §2 — 하드코딩된 단일 순서가 아니다):

| authority | 사다리 | 트리거 |
|---|---|---|
| `weak`(기본) | **E > c > expected** | 사용자 목표 미명시 |
| `explicit` | **c > E > expected** | 사용자 HITL *"목표 X tok/s"* (`--target-tps X` 필수) |
| `explore` | **E > expected** (`c` 칸 **부재**) | 사용자 HITL *"목표 0/NULL/광범위 탐색"* (`--target-tps` **금지**) |

세 칸 모두 **같은 유효성 술어**(`rubric_candidate()` — 유한 양수)를 통과해야 낙찰된다. `0`·음수·`NaN`·
`Inf` 는 사다리에 오르지 못하며(CLI 입력이면 exit 2), 그래서 `floor > 0` ∧ `ratio ≠ null` 이 **항상**
성립한다 — *문턱도 지표도 사라진 PASS*(공허 PASS)는 구조적으로 만들 수 없다(`plan_26082219` D1).

- **(a) 결정론 루프라인 = 척추(매번 먼저)**: `roofline.py` → `R_fp`(forward-pass/sec 상한, 100% MBU 낙관 천장)·
  `R_token = accept_len × R_fp`(speculative)·`expected_achievable = realistic_fraction × R_token`. **의심 임계**(SLA 아님).
- **(b) 외부 레퍼런스 E = 목표치**: 검증기(Devil's Advocate)가 **외부검색 수행** — 동일 HW 에서 남들이 내는 실제 달성치
  (HF 카드·포럼·vLLM PR — **1차 진입점 = `.claude/skills/wiki-desk/reference/references.md` §3·§4 warm-start → 미스 시 신규 검색 →
  히트 baseline 재입고**). 혼자 루프라인을 안 믿고 E 로 정밀화. **E 가 진짜 판별자**(측정>공식). **메인은 E 검색을
  시도·기록한 후에만 판정 진입** — `verdict_rule.py --e-search {hit,empty,no}` 로 상태를 결정론 게이트에 전달
  (빈손이면 `empty` 로 *기록된* roofline-only 강등 = 음성정직 / 미시도 `no` 는 출력에 경고 표기 — silent 강등 차단;
  egress-restricted 서브 = `no`+증상 상향이 설계, egress-online+위임 서브는 검색 시도).
- **(c) 사용자 = 최종 백스톱**: (a)·(b) 둘 다 루브릭을 못 세울 때만. `verdict_rule` 이 `NEEDS_RUBRIC`(axis=establish) 반환 → 사람에게 레퍼런스 요청.

**spec-aware(중요)**: no-MTP 서브는 `R_fp` 와, MTP 서브는 `R_token` 와 비교(like-with-like). speculative 면 token/s 가
단일패스 천장 `R_fp` 를 *초과* 가능 → 섞으면 M-vs-R 무의미(dogfood BLOCK 교훈).

## 2. 노드간 VRAM 밸런스 축 (멀티노드 — γ, `plan_26070809_47_07`)

decode-tps 축과 **직교**한 별도 루브릭 축. recipe-explorer 가 산정만 하고, **이 스킬이 밸런스를 게이트**한다.

- **산정식**: `balance_dev = (max_node_used_gib − min_node_used_gib) / max_node_used_gib`.
- **판정**(`weak`·`explicit`): `balance_dev > 0.10` → **REFUTE**(`failure_axis="balance"`) → recipe-explorer **loop-back**(재탐색, cap 한정).
  `≤ 0.10` → 밸런스 축 PASS.
- **`explore` 에서는 게이트가 아니라 서술이다**(2026-08-22 · SKILL.md §2.1 U1 후속): 값은 그대로 기록하되
  `verdict` 를 뒤집지 않는다. 편차 초과는 `refuted_claims` 가 아니라 `diagnosis_hint` 에 **벽 지도 데이터**로
  적힌다. 어느 쪽인지는 출력의 `balance.gates_verdict`(`true`=게이트 / `false`=서술)가 밝힌다.
  이유: 뒤집으면 인증서 미발행 → `completion_gate` 의 explore 승격 경로가 닫혀 `perf_waiver` 를 강요하게
  되어, "explore 의 승격 자격 = 서빙 성립" 계약이 이 축으로 우회된다.
- **입력**: `multinode_serve_smoke.sh` 양노드 measured VRAM → `verdict_rule.py --node-vram-gib <n1,n2,...> --balance-tol 0.10`.
  결정론 유지(LLM 다수결 ✗).
- **기본 비활성**: `--node-vram-gib` 미지정 시 balance 축은 판정에 관여하지 않음(기존 decode-tps 전용 판정 완전 보존).

## 3. 측정 — `vllm bench serve`

`run_bench.sh <config> [--topology] [--concurrency N] [--input-len N] [--output-len N] [--num-prompts N] [--warmups N]`:

- 돌고 있는 serve 의 컨테이너(`MASTER_CONTAINER_NAME`/`CONTAINER_NAME`)에서 `vllm bench serve`
  (client-side 토크나이저=마운트 모델 경로, 네트워크 불요) 실행 → `--save-result` JSON 회수 + `docker logs` 교차캡처.
- **정본 디코드 지표 = `1000 / median_tpot_ms`**(단일스트림 warm; output_throughput 은 콜드 TTFT 에 끌려 과소 —
  `--num-warmups` 로 폐기). `accept_len` 은 bench JSON 의 `spec_decode_acceptance_length` 직독.
- 교차검증: `parse_bench.py` 가 engine-log `generation throughput` 최대값과 client decode_tps 비교(괴리 = 콜드/warmup 의심).
- **버전-exact 확증**: `vllm bench serve` 존재·플래그를 이미지에서 확인 후 사용(가정 금지).
- 콜드 JIT(예 flashinfer SM120 첫 요청) → warmup 폐기로 흡수.

## 4. Devil's Advocate — 다중 렌즈 (LLM)

표적 주장 = *"이 서빙은 production/agent-ready 다(충분히 빠르고 일관적)."* 렌즈는 **서로 다른 실패모드**(같은 회의론 N개 ✗):

- **루프라인 렌즈**: M ≪ R 인가?(no-MTP↔R_fp · MTP↔R_token) comm-bound 인가(RDMA)?
- **레퍼런스 렌즈(외부검색 b)**: 동일 HW 서 남들 E 는? 포럼·PR·HF카드 대조 → E 산출(이중게이트 조건부 서브 자율).
- **일관성 렌즈**: 콜드 vs warm 격차? batch 키우면 무너지나?
- **회귀 렌즈(carry-forward 금지)**: 이 전략이 *이전 모델*의 검증된 성능을 깨나?
- 렌즈들의 *증거*를 합의 통합 → **결정론 `verdict_rule.py` 에 투입**(게이트 결정은 규칙 — LLM 다수결 아님).
- **수렴 보장(역-자기편향)**: PASS 조건은 규칙이 명시(M ≥ primary×(1−tol)) → Devil's Advocate 가 근거 없이 영원히 기각 불가.
  cap+escalation 이 종료를 강제한다.
- 오케스트레이션: 렌즈 fan-out = 병렬 에이전트(동시 공격 → verdict_rule 투입).

`verdict_rule.py` 출력 = `{verdict: PASS|REFUTE|NEEDS_RUBRIC|INVALID, failure_axis, structural_or_strategy(힌트),
rubric{primary,source,floor,ratio_M_over_primary,R_fp,R_token,expected,reference_E,target_c,
**authority**,**loop_until_done**,**candidates[]**}, refuted_claims[], diagnosis_hint[]}`.
`candidates[]` 는 사다리 세 칸의 3-state(`valid`/`invalid`/`unset`/`absent-by-authority`)를 그대로
표면화한다 — 하류(리포트·인증서·사람)가 *어느 칸이 왜 낙찰됐는지*를 추론이 아니라 **조회**로 안다.

## 5. 입력 (`config.yaml`)

사람이 작성한 `config.yaml`(영속·비추적). 없으면 `config.example.yaml` 참고. 핵심 키:

- `serving.config_name`(검증 대상 트리플렛 키) · `topology`(미지정 시 브랜치 파생).
- `reference_tps`(E, 선택 — 미지정 시 검증기 외부검색) · `target_tps`(c, 선택) · `tolerance`(기본 0.15).
  ⚠ `reference_tps`·`target_tps` 에 **`0`/`NULL`(빈 값) 을 '게이트 미설정'의 뜻으로 쓰지 않는다** —
  상수 0 은 루브릭이 될 수 없어 `verdict_rule.py` 가 **exit 2 로 거부**한다(공허 PASS 차단). *키를
  아예 두지 않는 것*이 미설정이고, **광범위 탐색은 `--authority explore`**(CLI 트리거)다. `authority`
  는 config 키가 아니다 — 트리거는 사용자 HITL 이며, 파일에 적히면 "사용자가 당겼다"는 증거가 약해진다.
  `tolerance` 는 `0 ≤ tol < 1`(tol=1 이면 floor=0 이라 같은 결함의 두 번째 입구가 된다).
- `realistic_fraction`(MBU 보정, 기본 0.35 — batch=1 MoE) · `reconciliation_cap`(기본 3).
- bench: `concurrency`(단일스트림=1) · `input_len` · `output_len` · `num_prompts` · `warmups`(콜드 JIT 폐기) ·
  `spec_supported`(모델이 MTP 지원) · `bench.lite_burst_n`(lite warm burst 크기).
