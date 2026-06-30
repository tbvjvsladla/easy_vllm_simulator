---
name: adversarial-benchmark
description: >-
  실행 중인 vLLM serve 의 성능을 *적대적으로* 검증해 "기대 이하"를 게이트에서 차단한다. 결정론 루프라인
  (R_fp/R_token, spec-aware) + 외부 레퍼런스(E) + 사용자(c) 3중 루브릭으로 무장한 Devil's Advocate 가
  "이 서빙은 충분히 빠르다"는 주장을 공격 → 기각 시 recipe-explorer 를 자극해 전략 폐기·재탐색
  (loop-until-done). "서빙 성능 검증해줘", "벤치마크 돌려줘", "이 모델 너무 느린데", "디코드 속도 정상이야?",
  "기대 이하 성능 검증", "성능 게이트" 같은 지시에 발동. 측정=`vllm bench serve`(결정론), 적대 판정=LLM.
  실서빙을 기동하지 않는다(돌고 있는 serve 를 검증 — 기동은 recipe-explorer/compose). 근거: 적대적 검증
  패턴(Pattern 3) + 목표달성까지 반복(Pattern 6) · plan_2026063014_1 · self-preference 사건 testlog_2026063004_1.
---

# adversarial-benchmark

서빙 에이전트(recipe-explorer)의 **"떴다 = 잘됐다"** 주장을, **결정론 루프라인 + 외부 레퍼런스 + 사람**의
3중 루브릭으로 무장한 **Devil's Advocate**가 적대적으로 공격해 **기대 이하 성능을 사람이 눈치채기 전에
게이트에서 차단**하고, 기각 시 recipe-explorer 를 **자극해 전략을 폐기·재탐색**시키는(loop-until-done)
**런타임블럭 검증 스킬**. 동기 = self-preference 사건(에이전트가 DeepSeek 15 t/s 를 "천장"으로 자기-선호 →
사용자 외부 레퍼런스가 반증; testlog_2026063004_1). **기능 스모크는 *작동*만, 이 스킬은 *성능*을 별도 게이트.**

> **§0.0 진입 전제 — 테라포밍-완수 Flag 게이트 (헌법 §테라포밍-완수 Flag 게이트 따름정리 · plan_2026063018_1)**: bench·verdict 는 *돌고 있는 serve* 전제 — 그 serve 자체가 recipe/upstream(=Flag)을 거쳤다(**전이적 게이트**). 직접 진입 시에도 Flag 확인: 미발급이면 **info-only**(루프라인 개념 설명 OK / bench·verdict ✗) → "벤치할 serve 가 없음 — recipe/upstream(테라포밍 Flag) 먼저". **결정론 백스톱** = `run_bench.sh` 진입 `manifest_contract.py --require-flag`. 서브 에어갭은 (b)외부검색 불가 → 루프라인-only 판정 + 증상 docs 상향(D12 동형 · `EASY_VLLM_SKIP_FLAG_GATE` 서브 우회).

> 설계 원칙(하네스 엔지니어링): **측정·루프라인·게이트는 결정론 스크립트**(`scripts/`). LLM 은 **외부검색(E)·
> 정성 진단·재탐색 힌트**만 생산해 결정론 게이트에 투입한다. PASS/REFUTE 는 규칙이 결정(LLM 다수결 아님).

## 0. 입력 (config.yaml)

사람이 작성한 `config.yaml`(영속·비추적). 없으면 `config.example.yaml` 참고. 핵심 키:
- `serving.config_name`(검증 대상 트리플렛 키, `.env.<config>`/`configs/<config>.yaml` 와 동일) · `topology`(미지정 시 브랜치 파생).
- `reference_tps`(E, 선택 — 미지정 시 검증기 외부검색) · `target_tps`(c, 선택 — 사용자 백스톱) · `tolerance`(기본 0.15).
- `realistic_fraction`(MBU 보정, 기본 0.35 — batch=1 MoE) · `reconciliation_cap`(기본 3).
- bench: `concurrency`(단일스트림=1) · `input_len` · `output_len` · `num_prompts` · `warmups`(콜드 JIT 폐기) · `spec_supported`(모델이 MTP 지원).

**하드 제약**: 대상 serve 가 안 떠 있으면(`:PORT/health≠200`) **기동하지 않고 중단·보고**(기동=recipe/compose 책임).

## 1. 결정론 / 확률론 경계 (하네스 엔지니어링)

| 구성요소 | 방식 | 스크립트 |
|---|---|---|
| 루프라인 (a) R_fp/R_token | **결정론** | `roofline.py` — manifest(gpu_model→대역폭 lookup·interconnect·topology) + 모델 config/safetensors index(NAS) |
| 성능 측정 M | **결정론** | `run_bench.sh`(`vllm bench serve`) → `parse_bench.py`(+engine-log 교차) |
| PASS/REFUTE 게이트 | **결정론** | `verdict_rule.py` — R·E·M 비교(임계·tolerance·3중 우선순위) |
| **외부검색 (b) E** | **LLM (메인전용)** | HF 모델카드·해외포럼·vLLM PR/issue → 현실 달성치. 서브는 미수행→상향보고 |
| 적대 판정·진단·재탐색 힌트 | **LLM Devil's Advocate (다중 렌즈)** | "기각인가? 왜? 다음 무엇을?" — 게이트엔 *증거*만 투입 |
| 구조적 vs 전략소진 최종분류 | **LLM 제안 + HITL** | cap=결정론, 분류=LLM+HITL |

→ "**측정·게이트·루프라인은 결정론. 외부검색·적대 판정만 LLM.**"

## 2. Rubric Base — 3중 방어막 (루브릭 *세우기*)

매번 **(a) → (b) → (c)** 순. `verdict_rule.py` 의 primary 우선순위 = **E > c > expected**.
- **(a) 결정론 루프라인 = 척추(매번 먼저)**: `roofline.py` → `R_fp`(forward-pass/sec 상한, 100% MBU 낙관 천장)·
  `R_token = accept_len × R_fp`(speculative)·`expected_achievable = realistic_fraction × R_token`. **의심 임계**(SLA 아님).
- **(b) 외부 레퍼런스 E = 목표치**: 검증기(Devil's Advocate)가 **외부검색 수행** — 동일 HW 에서 남들이 내는 실제 달성치
  (HF 카드·포럼·vLLM PR). 혼자 루프라인을 안 믿고 E 로 정밀화. **E 가 진짜 판별자**(측정>공식).
- **(c) 사용자 = 최종 백스톱**: (a)·(b) 둘 다 루브릭을 못 세울 때만. `verdict_rule` 이 `NEEDS_RUBRIC`(axis=establish) 반환 → 사람에게 레퍼런스 요청.

**spec-aware(중요)**: no-MTP 서브는 `R_fp` 와, MTP 서브는 `R_token` 와 비교(like-with-like). speculative 면 token/s 가 단일패스 천장 `R_fp` 를 *초과* 가능 → 섞으면 M-vs-R 무의미(dogfood BLOCK 교훈).

## 3. 두 실패축 (반드시 구분)

| 축 | 의미 | 처리 |
|---|---|---|
| **루브릭 못 *세움*** | (a) 불확실 ∧ (b) 빈손 | → **(c) 사용자 백스톱**(`NEEDS_RUBRIC`) |
| **루브릭 못 *충족*** | M < 루브릭, recipe 재탐색해도 미달 | → `reconciliation_cap` 한정 루프 → 소진 시 **구조적**=escalation 역루프(upstream rebuild) / **전략소진**=음성정직+best-so-far. **무한 기각·무한 루프 금지** |

## 4. 파이프라인 (loop-until-done · 강제함수)

```text
[recipe-explorer: 레시피 생성 + serve up]            ← Blue Team("떴다, X t/s, 좋다")
  → adversarial-benchmark:
     ① roofline.py (결정론, spec-aware)               → R_fp/R_token/expected
     ② serve 가동 확인(:PORT/health 200) — 미가동이면 중단(기동 안 함)
     ③ run_bench.sh → parse_bench.py (warmup 폐기 + engine 교차)  → M
     ④ Devil's Advocate (다중 렌즈, LLM): E 외부검색 + 주장 공격(메인전용)
     ⑤ verdict_rule.py (결정론 게이트):
          PASS  → done-게이트 클리어 ✅
          REFUTE→ 기각 리포트 + next_strategy_hint
                  → recipe-explorer 자극(전략 폐기·재탐색) → 재 serve → ②로 (cap 한정)
                  → cap 소진: 구조적→escalation 역루프 / 전략소진→음성정직
          NEEDS_RUBRIC → (c) 사용자 백스톱
```
- 매 iteration = serve(재)기동(recipe/compose) → 벤치 측정 → 적대 판정. teardown 규율 = recipe Phase-2 동일(통합메모리 OOM 보호).
- **누가 누구를 부르나**: 이 스킬은 *측정+판정+힌트*만 소유. **재탐색은 recipe-explorer 가**(에이전트가 `next_strategy_hint` 를 recipe 에 전달 — 스킬↔스킬 직접호출 아님, 에이전트 매개). recipe 측정 재구현 금지(§7).

## 5. 측정 — `vllm bench serve` (§7 plan)

`run_bench.sh <config> [--topology] [--concurrency N] [--input-len N] [--output-len N] [--num-prompts N] [--warmups N]`:
- 돌고 있는 serve 의 컨테이너(`MASTER_CONTAINER_NAME`/`CONTAINER_NAME`)에서 `vllm bench serve`(client-side 토크나이저=마운트 모델 경로, airgap-safe) 실행 → `--save-result` JSON 회수 + `docker logs` 교차캡처.
- **정본 디코드 지표 = `1000 / median_tpot_ms`**(단일스트림 warm; output_throughput 은 콜드 TTFT 에 끌려 과소 — `--num-warmups` 로 폐기). `accept_len` 은 bench JSON 의 `spec_decode_acceptance_length` 직독.
- 교차검증: `parse_bench.py` 가 engine-log `generation throughput` 최대값과 client decode_tps 비교(괴리 = 콜드/warmup 의심).
- **버전-exact 확증**: `vllm bench serve` 존재·플래그를 이미지에서 확인 후 사용(가정 금지 — 파서명 caveat 동형).
- 콜드 JIT(예 flashinfer SM120 첫 요청) → warmup 폐기로 흡수.

## 6. 적대적 검증기 — Devil's Advocate (다중 렌즈, LLM)

표적 주장 = *"이 서빙은 production/agent-ready 다(충분히 빠르고 일관적)."* 렌즈는 **서로 다른 실패모드**(같은 회의론 N개 ✗):
- **루프라인 렌즈**: M ≪ R 인가?(no-MTP↔R_fp · MTP↔R_token) comm-bound 인가(RDMA)?
- **레퍼런스 렌즈(외부검색 b)**: 동일 HW 서 남들 E 는? 포럼·PR·HF카드 대조 → E 산출(메인전용).
- **일관성 렌즈**: 콜드 vs warm 격차? batch 키우면 무너지나?
- **회귀 렌즈(carry-forward 금지)**: 이 전략이 *이전 모델*의 검증된 성능을 깨나?
- 렌즈들의 *증거*를 합의 통합 → **결정론 `verdict_rule.py` 에 투입**(게이트 결정은 규칙 — LLM 다수결 아님).
- **수렴 보장(역-자기편향)**: PASS 조건은 규칙이 명시(M ≥ primary×(1−tol)) → Devil's Advocate 가 근거 없이 영원히 기각 불가. cap+escalation 이 종료 강제.
- 오케스트레이션: 렌즈 fan-out = Workflow/parallel Agent(동시 공격 → verdict_rule 투입).

`verdict_rule.py` 출력 = `{verdict: PASS|REFUTE|NEEDS_RUBRIC|INVALID, failure_axis, structural_or_strategy(힌트), rubric{primary,source,floor,R_fp,R_token,expected}, refuted_claims[], diagnosis_hint[]}`.

## 7. 스킬 경계 / 인터페이스

- **↔ recipe-explorer**: recipe 의 측정·serve 인프라(§5 Phase-1.5·simlog·`multinode_serve_smoke.sh`)를 **소비**, 위에 적대 루브릭/게이트만 얹는다. 기각 시 `next_strategy_hint` 로 recipe 재탐색 **자극**(recipe 가 전략 폐기·재생성 — feasibility 탐색은 recipe, performance 목표는 이 스킬이 주입). **recipe 측정/serve 재구현 금지**.
- **↔ upstream-version-watch**: "루브릭 못 충족 + 구조적" → **escalation 역루프** 핸드오프(오프라인 증상 M≪expected + 외부 확증 = 적대 증거). upstream 이 버전핀/rebuild 소유(승인 게이트). 헌법 §escalation 역루프.
- **↔ wiki-desk**: 진입 시 warm-start(이전 동일 모델/HW 성능 증거 우선소비). 새 testlog 발행 시 입고.
- **블럭 분류 = 런타임블럭(서브 복제)**: 서브가 자기 모델에 자율 실행. **단 (b) 외부검색 arm 은 메인전용**(서브 에어갭 → (a) 루프라인-only 판정 + 증상 docs 상향 보고; D12·escalation 서브 인스턴스 동형). 발견≠소유의 *성능-검증 축*.

## 8. 안전 / 금지

- **serve 를 기동하지 않는다**(돌고 있는 serve 검증만). 미가동 시 중단·보고.
- 모델 자동 다운로드 금지(NAS 부재면 중단). 결정론 스크립트는 외부 네트워크 호출 없음 — **단 검증기 (b) 외부검색(서빙전략 외부 교차검증)은 허용·의무**(헌법 §모델 획득 모드 따름정리; 폐쇄망=모델획득 한정).
- 무승인 자동 escalate/rebuild ✗(escalation 은 승인 게이트). 무한 기각·무한 루프 ✗(cap → Model-C).
- 게이트(PASS/REFUTE)는 결정론 규칙 — LLM 다수결로 결정하지 않는다.

## 9. 보조 파일

- `scripts/roofline.py` — (a) spec-aware R_fp/R_token/expected (결정론, manifest+config/index).
- `scripts/run_bench.sh` — 돌고 있는 serve 에 `vllm bench serve` → 결과 JSON + engine-log 캡처.
- `scripts/parse_bench.py` — bench JSON(+engine-log) → 측정 M(decode_tps=1000/median_tpot, accept_len, 교차검증).
- `scripts/verdict_rule.py` — 결정론 PASS/REFUTE 게이트(3중 우선순위 E>c>expected, like-with-like, spec-off 강제함수).
- `fixtures/` — verdict 단위검증 fixture(REFUTE 등).
- `config.example.yaml` — 입력 스키마.
