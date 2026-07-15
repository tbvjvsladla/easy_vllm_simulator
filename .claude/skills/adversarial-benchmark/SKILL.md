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

> **§0.0 진입 전제 — 테라포밍-완수 Flag 게이트 (헌법 §테라포밍-완수 Flag 게이트 따름정리 · plan_2026063018_1)**: bench·verdict 는 *돌고 있는 serve* 전제 — 그 serve 자체가 recipe/upstream(=Flag)을 거쳤다(**전이적 게이트**). 직접 진입 시에도 Flag 확인: 미발급이면 **info-only**(루프라인 개념 설명 OK / bench·verdict ✗) → "벤치할 serve 가 없음 — recipe/upstream(테라포밍 Flag) 먼저". **결정론 백스톱** = `run_bench.sh` 진입 `manifest_contract.py --require-flag`. (b) 외부검색 arm = **이중게이트**(A2A 위임 키 ∧ egress-online) 통과 서브 자율, 미통과 시 루프라인-only 판정 + 증상 docs 상향(메인 릴레이 · D12 동형 — 위임 키 확인 = `.claude/a2a_delegation.json` 1차 / `EASY_VLLM_A2A_DELEGATED` 2차 — `run_bench.sh` **fail-closed**(키·MC·Flag 모두 부재 → exit4; 옛 MC-부재 skip=fail-open 교정) · 헌법 §A2A-위임 Flag 따름정리·plan_2026063021_2·plan_2026070809_2).

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
| **외부검색 (b) E** | **LLM (이중게이트 조건부 서브 자율)** | HF 모델카드·해외포럼·vLLM PR/issue → 현실 달성치. 미통과 서브는 미수행→상향보고 |
| 적대 판정·진단·재탐색 힌트 | **LLM Devil's Advocate (다중 렌즈)** | "기각인가? 왜? 다음 무엇을?" — 게이트엔 *증거*만 투입 |
| 구조적 vs 전략소진 최종분류 | **LLM 제안 + HITL** | cap=결정론, 분류=LLM+HITL |

→ "**측정·게이트·루프라인은 결정론. 외부검색·적대 판정만 LLM.**"

## 2. Rubric Base — 3중 방어막 (루브릭 *세우기*)

매번 **(a) → (b) → (c)** 순. `verdict_rule.py` 의 primary 우선순위 = **E > c > expected**.
- **(a) 결정론 루프라인 = 척추(매번 먼저)**: `roofline.py` → `R_fp`(forward-pass/sec 상한, 100% MBU 낙관 천장)·
  `R_token = accept_len × R_fp`(speculative)·`expected_achievable = realistic_fraction × R_token`. **의심 임계**(SLA 아님).
- **(b) 외부 레퍼런스 E = 목표치**: 검증기(Devil's Advocate)가 **외부검색 수행** — 동일 HW 에서 남들이 내는 실제 달성치
  (HF 카드·포럼·vLLM PR — **1차 진입점 = `.claude/rules/references.md` §3·§4 warm-start → 미스 시 신규 검색 →
  히트 baseline 재입고**). 혼자 루프라인을 안 믿고 E 로 정밀화. **E 가 진짜 판별자**(측정>공식). **메인은 E
  검색을 시도·기록한 후에만 판정 진입** — `verdict_rule.py --e-search {hit,empty,no}` 로 상태를 결정론 게이트에
  전달(빈손이면 `empty` 로 *기록된* roofline-only 강등 = 음성정직 / 미시도 `no` 는 출력에 경고 표기 — silent
  강등 차단 · plan_2026070208_1; egress-restricted 서브 = `no`+증상 상향이 설계, egress-online+위임 서브는 검색 시도).
- **(c) 사용자 = 최종 백스톱**: (a)·(b) 둘 다 루브릭을 못 세울 때만. `verdict_rule` 이 `NEEDS_RUBRIC`(axis=establish) 반환 → 사람에게 레퍼런스 요청.

**spec-aware(중요)**: no-MTP 서브는 `R_fp` 와, MTP 서브는 `R_token` 와 비교(like-with-like). speculative 면 token/s 가 단일패스 천장 `R_fp` 를 *초과* 가능 → 섞으면 M-vs-R 무의미(dogfood BLOCK 교훈).

## 2.5 노드간 VRAM 밸런스 (멀티노드 — γ, `plan_2026070809_3`)

decode-tps 축(§2)과 **직교**한 별도 루브릭 축. recipe-explorer 가 산정만 하고(§4 · per-GPU 클램프 ÷TP),
**이 스킬이 밸런스를 게이트**한다.

- **산정식**: `balance_dev = (max_node_used_gib − min_node_used_gib) / max_node_used_gib`.
- **판정**: `balance_dev > 0.10` → **REFUTE**(`failure_axis="balance"`) → recipe-explorer **loop-back**(재탐색,
  헌법 §적대적 성능 검증 따름정리 "기각→재탐색·cap 한정"). `≤ 0.10` → 밸런스 축 PASS.
- **입력**: `multinode_serve_smoke.sh` 양노드 measured VRAM(노드별 used) → `verdict_rule.py --node-vram-gib
  <n1,n2,...> --balance-tol 0.10`(기본 0.10). 결정론 유지(LLM 다수결 ✗ — §서두 설계 원칙).
- **기본 비활성**: `--node-vram-gib` 미지정 시 balance 축은 판정에 관여하지 않음(기존 decode-tps 전용 판정
  완전 보존 — 단일노드·기존 멀티노드 호출 회귀 0).

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
     ④ Devil's Advocate (다중 렌즈, LLM): E 외부검색 + 주장 공격(이중게이트 조건부 서브 자율)
        — 메인은 ④의 E 검색(references.md warm-start 포함) 수행·기록 후에만 ⑤ 진입(미시도 ⑤ 직행 ✗)
     ⑤ verdict_rule.py (결정론 게이트, --e-search 로 E 상태 기록):
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
- 돌고 있는 serve 의 컨테이너(`MASTER_CONTAINER_NAME`/`CONTAINER_NAME`)에서 `vllm bench serve`(client-side 토크나이저=마운트 모델 경로, 네트워크 불요) 실행 → `--save-result` JSON 회수 + `docker logs` 교차캡처.
- **정본 디코드 지표 = `1000 / median_tpot_ms`**(단일스트림 warm; output_throughput 은 콜드 TTFT 에 끌려 과소 — `--num-warmups` 로 폐기). `accept_len` 은 bench JSON 의 `spec_decode_acceptance_length` 직독.
- 교차검증: `parse_bench.py` 가 engine-log `generation throughput` 최대값과 client decode_tps 비교(괴리 = 콜드/warmup 의심).
- **버전-exact 확증**: `vllm bench serve` 존재·플래그를 이미지에서 확인 후 사용(가정 금지 — 파서명 caveat 동형).
- 콜드 JIT(예 flashinfer SM120 첫 요청) → warmup 폐기로 흡수.

## 5.5 경량(lite) 모드 — inform-only · 기본 ON (plan_2026071115_1 · Phase B)

> full 적대 게이트(§2·§4·§6·§8 loop-until-done)와 **다른 모드**. lite 는 **서빙이 성공하면 자동으로 도는
> 가벼운 상태-스냅샷**이다 — 배포 사용자가 "일단 떴다" 다음 곧바로 *속도·용량 현재치*를 눈으로 확인하게 해
> Convenience/Experiences(검증 투명성)를 올린다. **소유 = adversarial-benchmark**(D4 — explorer 는 트리거·핸드오프만, D5).

- **기본 ON (D7)**: serve 성공(`recipe-explorer` 서빙 완료) 직후 **자동 수행**. 가벼운 스킵 시그널("스킵해"·묵시적
  넘어감)에도 **수행한다** — 경량 벤치는 시스템 안정성과 직결되어 *개발자 의지*로 기본 실행이다. **억제 = 강력 거부
  구문**("무조건 어떠한 경우에서라도 구동하지 마" 급)만, 그리고 **세션 한정**(D25 — config/manifest 영구 기록 ✗;
  매 세션 기본 ON 복귀). 영구 opt-out 불허(안정성=프로젝트 신뢰성 직결 · NG-4).
- **inform-only (D8 · NG-6)**: **PASS/FAIL 판정 없음 · 자동 loop-back 없음**. `verdict_rule.py` 에 투입하지 않는다
  (done-게이트는 오직 full 경로 §6·§8 소유 — 기능≠성능 따름정리 불변, lite 는 done-게이트가 **아니다**). lite 출력이
  커뮤니티/레퍼런스 대비 **심각한 괴리**로 보이면 → **이상징후 안내 + full 승격 권유**까지만(자동 재탐색 ✗).
- **괴리 판단 = 에이전트 재량 (D18 · NG-3)**: `references.md` §4 HW-스코프 baseline·포럼 수치 대비 *정성 판단*.
  **정량 threshold 금지**(커뮤니티 자료 신뢰성은 가변 — "A 유저가 이만큼 냈다"가 늘 신뢰되진 않음). 결정론 게이트화 ✗.
- **측정 사양 (D12·D23·D26)**: `lite_bench.sh <config>` — cold(무-warmup 단일요청→cold TTFT 별도 1줄) + warm
  burst(**N=3**·conc=1·warmup 1 제외, `config bench.lite_burst_n`/`--burst-n` 조정) 2회 `vllm bench serve`. **5종 메트릭**:
  gen tokens/sec(warm, =1000/median_tpot) · cold-start TTFT · GPU VRAM 점유(GiB+%) · KV cache 점유(GiB+%) · 시스템 RAM(GiB+%).
  산정·표 렌더 = 결정론 `lite_metrics.py`(확률론 산정 ✗). 통합메모리(GB10)는 nvidia-smi 메모리 N/A → **serve-log VRAM
  분해 폴백**, 그도 없으면 값 없이 "N/A(source)" 음성정직(대체값 날조 ✗). engine-log KV 라인 부재 시 fail-soft null.
- **출력 (D29·D32)**: **single = 채팅 5행 표**(메트릭|값 · 용량은 GiB+% 병기 · cold TTFT 별도 행). **multi = 병합 표 1개**
  — 용량 3종 열=**Main|Sub**(per-node), 속도·cold TTFT 는 마스터 엔드포인트 기준 1행(D19 — 신분차는 명령체계뿐, 관측은 평등).
- **멀티 수집 (D19·D22 · A2A 정합)**: per-node 평등 수집. 서브 = **SSH 읽기전용 probe**(`nvidia-smi`/`/proc/meminfo` —
  `multinode_serve_smoke.sh` 패턴 재사용). 이 읽기전용 런타임 관측은 **health 폴링과 동형 평면**이지 "서브 작업코드/설정
  재스캔 금지"(A2A 경계 §7)와 **다른 평면**이다. 서브 probe 실패 시 graceful — 마스터 단독 + 실패 음성정직 표기.
- **down 시나리오 (D21)**: "테스트만 하고 down" 케이스도 serve → **lite 수행** → 결과 표시 → down(라이브 엔드포인트가
  있는 동안 측정). 용처 매뉴얼은 생략(recipe-explorer §6.5 — 용처 없는 케이스).
- **자동 핸드오프 = 헌법 명시 예외**: recipe→adversarial **lite 한정** 자동 수행은 "무인 자동실행 없음" 트리거 정책의
  **명시 예외**다(안전망 데몬 예외와 동형 — 관측·inform-only 한정). 헌법 §경량 벤치 자동 핸드오프 따름정리 참조.
  **full 벤치(§6·§8)·bump·다운로드의 완전-수동 속성은 불변**. Flag 게이트: lite 는 이미 Flag-게이트된 serve 위에서
  돈다(§0.0 전이적) + `lite_bench.sh` 가 `run_bench.sh` 와 동형 **fail-closed 백스톱**(키·MC·Flag 부재 exit 4).

## 5.6 full-모드 종결 발행 — report + 인증서 (편지 패턴 A·B · plan_2026071510_1)

> lite(§5.5)와 다른 **full 경로의 종결 산출물**. full 적대 게이트(§4 loop-until-done)가 **종결**(cap 소진 or PASS)되면,
> 판정과 **별개로** 사람용 report + (PASS시)기계용 인증서를 `docs/benchmark/`(5번째 문서형 · docs.md §benchmark)에 발행한다.

- **부하 스윕(client-load · reload 0)** — `sweep_bench.sh <config> [--topology] [--levels 1,2,4,8,16]`: 단일 running
  serve 에 **동시성만** 변화(reload 0 — 벤치마커 "기동 안 함" 불변식 보존). **판정점(동시성=1) 강제 포함** → verdict
  재사용(재측정 0). **적응 상한 클램프 + 절삭 로그**(레벨 실패 시 상위 중단·"레벨 N 절삭" 기록 — silent truncation ✗).
  각 레벨 = `run_bench.sh` 메커니즘 재사용(Flag/A2A 게이트 전이). config-space(batch×maxlen) reload 는 이 스윕 **밖**(Max/explorer 소관).
- **사람용 report(항상)** — `render_report.py --sweep-index <sweep_index.json> --verdict-json <verdict> [--roofline-json]`
  → `docs/benchmark/report_<model>_<gpu>_<vllm>.md`. **PASS/FAIL 무관 발행**("왜 느렸나"도 사람이 봐야). **inform-only**
  (verdict 를 *표시만* — 판정권한 ✗·verdict_rule 독점) · 결정론 렌더(LLM 표·숫자 저작 ✗) · N/A fail-soft.
- **기계용 인증서(PASS시만)** — `publish_benchmark_record.py --sweep-index … --verdict-json …`
  → `docs/benchmark/benchmark_<model>_<gpu>_<vllm>.yaml`. **flat 계약**(중첩 ✗ — 소비자 stdlib 독해) + **carry-forward
  재검증 헤더**(강한키=model/gpu/vllm/quant/topology/tp 정확일치 + 소프트지문=driver/cuda/image/max-len/kv-bytes/gmu/moe
  불일치 시 stale). verdict≠PASS 면 **미발행**(report 만).
- **비용 규율(편지 B.5)**: 재탐색 루프 **내부는 값싼 단일점 판정** 유지 · 스윕·리치리포트는 **종결 1회**만. 오케스트레이션은
  **에이전트 매개**(스킬↔스킬 직접호출 ✗ — §7). full 런은 사람-트리거이므로 종결 발행은 최소 예외면(inform-only·결정론 —
  lite 자동핸드오프와 동형 평면, 헌법 §경량벤치 자동핸드오프 따름정리). **done-게이트는 여전히 verdict(§6) 독점** · lite 는 발행 안 함(채팅 표만).

## 6. 적대적 검증기 — Devil's Advocate (다중 렌즈, LLM)

표적 주장 = *"이 서빙은 production/agent-ready 다(충분히 빠르고 일관적)."* 렌즈는 **서로 다른 실패모드**(같은 회의론 N개 ✗):
- **루프라인 렌즈**: M ≪ R 인가?(no-MTP↔R_fp · MTP↔R_token) comm-bound 인가(RDMA)?
- **레퍼런스 렌즈(외부검색 b)**: 동일 HW 서 남들 E 는? 포럼·PR·HF카드 대조 → E 산출(이중게이트 조건부 서브 자율).
- **일관성 렌즈**: 콜드 vs warm 격차? batch 키우면 무너지나?
- **회귀 렌즈(carry-forward 금지)**: 이 전략이 *이전 모델*의 검증된 성능을 깨나?
- 렌즈들의 *증거*를 합의 통합 → **결정론 `verdict_rule.py` 에 투입**(게이트 결정은 규칙 — LLM 다수결 아님).
- **수렴 보장(역-자기편향)**: PASS 조건은 규칙이 명시(M ≥ primary×(1−tol)) → Devil's Advocate 가 근거 없이 영원히 기각 불가. cap+escalation 이 종료 강제.
- 오케스트레이션: 렌즈 fan-out = Workflow/parallel Agent(동시 공격 → verdict_rule 투입).

`verdict_rule.py` 출력 = `{verdict: PASS|REFUTE|NEEDS_RUBRIC|INVALID, failure_axis, structural_or_strategy(힌트), rubric{primary,source,floor,R_fp,R_token,expected}, refuted_claims[], diagnosis_hint[]}`.

## 7. 스킬 경계 / 인터페이스

- **↔ recipe-explorer**: recipe 의 측정·serve 인프라(§5 Phase-1.5·simlog·`multinode_serve_smoke.sh`)를 **소비**, 위에 적대 루브릭/게이트만 얹는다. 기각 시 `next_strategy_hint` 로 recipe 재탐색 **자극**(recipe 가 전략 폐기·재생성 — feasibility 탐색은 recipe, performance 목표는 이 스킬이 주입). **recipe 측정/serve 재구현 금지**.
- **↔ upstream-version-watch**: "루브릭 못 충족 + 구조적" → **escalation 역루프** 핸드오프(구동불가 증상 M≪expected + 외부 확증 = 적대 증거). upstream 이 버전핀/rebuild 소유(승인 게이트). 헌법 §escalation 역루프.
- **↔ wiki-desk**: 진입 시 warm-start(이전 동일 모델/HW 성능 증거 우선소비). 새 testlog 발행 시 입고.
- **블럭 분류 = 런타임블럭(서브 복제)**: 서브가 자기 모델에 자율 실행. **(b) 외부검색 arm = 이중게이트(A2A 위임 키 ∧ egress-online) 통과 시 서브 자율, 미통과 시 루프라인-only 판정 + 증상 docs 상향 보고**(메인 릴레이 · D12·escalation 서브 인스턴스 동형). 발견≠소유의 *성능-검증 축*. lite 스크립트(`lite_bench.sh`·`lite_metrics.py`)도 이 런타임블럭에 속해 **git-tracked 로 서브 자동 전파**(sync_to_sub 명시 추가 불요 — `render_sub_env` `_copy_tracked`).
- **lite 멀티 수집의 A2A 관측 평면 (§5.5)**: 서브 `nvidia-smi`/`/proc/meminfo` **읽기전용 probe** 는 health 폴링과 **동형 관측 평면**이지 A2A 경계의 "서브 작업코드/설정 재스캔·직접교정 금지"와 **다른 평면**이다(관측 ≠ 재스캔·교정). 실패 시 마스터 단독 + 음성정직.

## 8. 안전 / 금지

- **serve 를 기동하지 않는다**(돌고 있는 serve 검증만). 미가동 시 중단·보고.
- 모델 자동 다운로드 금지(NAS 부재면 중단). 결정론 스크립트는 외부 네트워크 호출 없음 — **단 검증기 (b) 외부검색(서빙전략 외부 교차검증)은 허용·의무**(헌법 §모델 획득 모드 따름정리; 모델획득 격리 한정).
- 무승인 자동 escalate/rebuild ✗(escalation 은 승인 게이트). 무한 기각·무한 루프 ✗(cap → Model-C).
- 게이트(PASS/REFUTE)는 결정론 규칙 — LLM 다수결로 결정하지 않는다.

## 8.5 Max 모드 — HW 안전-최대 컨텍스트 envelope 특성화 (별도 오퍼레이션 · **구현됨** · plan_2026071510_1)

> **별도 오퍼레이션 — 벤치마커 native 모드(§2·§4 적대검증)가 아니다.** 벤치마커의 *측정 인프라만* 공유하고,
> 진입·트리거·안전게이트는 전부 별도. **Max 가 기동/reload 를 소유**하므로 벤치마커 본체 "기동 안 함" 불변식은 보존.
> **라이브 E2E 는 후속**(실 스텝업 = 실제 하드다운 위험 → 드라이버 안전 재검증과 함께 신중히). 결정론 로직은 dry-run+fixture 검증됨.

- **정체성 = 별도 오퍼레이션(벤치마커 인프라만 공유)**: Max 는 config 를 **재서빙(reload)** 하며 탐색하므로 벤치마커
  "기동 안 함" 불변식과 충돌 → 서브모드 ✗. 측정 라이브러리(`run_bench`·`verdict_rule`·render 계열)만 재사용하고
  진입·트리거 완전 별도(미래 soak-stress 스킬과 동형 배치). 코드는 `adversarial-benchmark/scripts/` 동거(인프라 재사용)·정체성 별도.
- **축 = 컨텍스트(max-model-len) 안전측 스텝업(옵션 A)**: `max_envelope.sh <config> [--levels 131072,262144,393216,524288]
  --confirm-risk`. 낮은 컨텍스트→높은 컨텍스트 오름차순 재서빙, 각 레벨 serve+smoke 통과=안전·기록·상향 / 실패·트립=**직전이
  안전상한**·중단(안전측 적응 클램프·절삭 로그). 컨텍스트 축은 weights 불변이라 KV/prefill 위험을 **워치독+스모크가 관측**
  (하드다운 봉투 512k안전/768k치명과 동형 — testlog_2026071113_1 안전측 프로브를 재사용 오퍼레이션으로 일반화). batch 축은
  full(§5.6) client-load 스윕이 부분 커버. **default 상한 = 524288(검증된 안전상한)** — 초과 probe 는 --levels 명시로만.
- **목적**: HW **안전-최대** envelope 특성화 — explorer("용처-최적 config *선택*") ↔ Max("절대-최대 config *특성화*"). 중복 ✗.
- **트리거(완전 옵트인 · 자동 아님)**: **전작업 완료** 후에만 — 서빙 확정 + 문서(report/인증서) 발행 + wiki 등록까지 끝난
  지점에서 **에이전트가 챗 경고톤 Y/N**("Max 벤치? — reload 반복·통합메모리 하드다운 위험") → 승인 시에만 스크립트 실행.
  **"무인 자동실행 없음" 유지** · **선-기록 후-위험**(각 레벨 결과를 다음 시도 전 기록 — 하드다운이 진행분 소실 안 하게).
- **안전 이중 게이트**: (1) 스크립트 `--confirm-risk` 명시(무심코 실행 차단·미명시 exit 5) (2) 에이전트 챗 Y/N. + serve+smoke =
  `multinode_serve_smoke.sh`(협역 워치독 자동 arming + 로드-전 RAM 게이트 내장) · per-level config 스냅샷 + EXIT-트랩 원복.
  헌법 §호스트 안전체계 따름정리 정합(**파킹된 드라이버 580.159.03 안전 재검증의 실행 vehicle**).
- **산출물**: `max_envelope.sh` → `max_index.json` → `render_max_report.py` → `docs/benchmark/max_envelope_<model>_<gpu>_<vllm>.md`
  (안전상한·레벨별 결과·절삭 로그 · **inform-only** 특성화 표시 · 판정 게이트 아님). single-node serve+smoke = 후속(현재 multi 우선).
- 근거: `seed/letter_2026071516_1` · `plan_2026071510_1` · 헌법 §호스트 안전체계 따름정리 · `testlog_2026071113_1`(안전측 프로브).

## 9. 보조 파일

- `scripts/roofline.py` — (a) spec-aware R_fp/R_token/expected (결정론, manifest+config/index).
- `scripts/run_bench.sh` — 돌고 있는 serve 에 `vllm bench serve` → 결과 JSON + engine-log 캡처(full 경로).
- `scripts/lite_bench.sh` — 경량 inform-only 벤치 오케스트레이터(§5.5): cold+warm(N=3·conc1) `vllm bench serve` + per-node nvidia-smi/proc·meminfo + master engine-log KV + multi SSH 읽기전용 probe → raw JSON. `run_bench.sh` 의 Flag/A2A 게이트·envfile 해소 재사용. 서브 복제(런타임블럭 · git-tracked 자동 전파).
- `scripts/lite_metrics.py` — lite 5종 메트릭 결정론 파서/렌더러(§5.5): raw JSON+bench JSON+engine-log → GiB/% 산정 + single 5행/multi 병합표 렌더. **verdict_rule 미투입(inform-only)** · 통합메모리 nvidia-smi N/A 폴백·음성정직.
- `scripts/parse_bench.py` — bench JSON(+engine-log) → 측정 M(decode_tps=1000/median_tpot, accept_len, 교차검증).
- `scripts/verdict_rule.py` — 결정론 PASS/REFUTE 게이트(3중 우선순위 E>c>expected, like-with-like, spec-off 강제함수).
- `scripts/sweep_bench.sh` — full-모드 client-load 부하 스윕(§5.6): 동시성 레벨 × `run_bench.sh` 재사용(reload 0)·
  판정점(1) 강제포함·적응 상한 클램프+절삭 로그 → `sweep_index.json`(meta+per-level measured). `--dry-run` 지원. 런타임블럭.
- `scripts/render_report.py` — 사람용 보고서 결정론 렌더러(§5.6 · inform-only · **항상** 발행 · N/A fail-soft):
  sweep_index+verdict → `docs/benchmark/report_<model>_<gpu>_<vllm>.md`(부하 곡선·루프라인·환경 스냅샷). LLM 표저작 ✗. 런타임블럭.
- `scripts/publish_benchmark_record.py` — 인증서(flat 계약) 발행(§5.6 · **PASS시만**): sweep_index+verdict →
  `docs/benchmark/benchmark_<model>_<gpu>_<vllm>.yaml`(flat·carry-forward 재검증 헤더·강한키+소프트지문). stdlib only. 런타임블럭.
- `scripts/max_envelope.sh` — **Max 오퍼레이션**(§8.5 · 별도 오퍼레이션): 컨텍스트(max-model-len) 안전측 스텝업 재서빙 →
  안전상한 특성화. **이중 게이트**(`--confirm-risk` 미명시 exit 5 + 에이전트 챗 Y/N) · serve+smoke=`multinode_serve_smoke.sh`
  (워치독/RAM게이트 내장) · per-level config 스냅샷+EXIT-트랩 원복 · 선-기록 후-위험 · `--dry-run` 지원 → `max_index.json`. 런타임블럭.
- `scripts/render_max_report.py` — Max envelope 보고서 결정론 렌더러(§8.5 · inform-only · N/A fail-soft): max_index →
  `docs/benchmark/max_envelope_<model>_<gpu>_<vllm>.md`(안전상한·레벨별·절삭 로그). stdlib only. 런타임블럭.
- `fixtures/` — verdict 단위검증(`measured_refute_no_mtp.json`) + full-모드 발행 검증(`roofline_sample.json`·`measured_pass.json`·
  `sweep_index_sample.json`) + Max 발행 검증(`max_index_sample.json`) — render/publish 결정론 체인 라이브-불요 검증.
- `config.example.yaml` — 입력 스키마.
