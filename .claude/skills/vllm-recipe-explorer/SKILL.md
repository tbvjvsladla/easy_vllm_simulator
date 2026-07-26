---
name: vllm-recipe-explorer
description: >-
  모델획득 격리 환경(외부접속 전반 차단 아님)에서 고정된 한 모델의 config.json을 결정론적으로 파싱하고, (quantization × max-model-len ×
  gpu-memory-utilization) 3축 레시피 후보를 결정론 VRAM 추정 → 예산×안전마진 하드게이트 →
  headroom→context 랭킹으로 제시하고, 사람이 고른 레시피를 DGX Spark 서빙용 3종 세트(.yaml+.sh+.env)로
  생성한다(Phase 1, 추정). Phase 2는 멀티턴 인터뷰로 lock/soft/free 변수를 정하고 실서빙 trial-loop로
  절대 KV 클램프(--kv-cache-memory-bytes)를 수렴시켜 검증된 레시피를 낸다. "레시피 추천",
  "서빙 레시피", "VRAM 예산에 맞춰", "이 모델 어떤 설정으로 띄울까", "RTX4090 예산 레시피",
  "gpu memory 예산", "recipe", "실제로 띄워서 검증", "KV 캐시 튜닝", "batch/동시요청", "tool/reasoning 파서",
  "attention backend" 같은 지시에 발동. tp는 manifest(len(nodes)×gpus_per_node) 자동결정(git 브랜치 ✗). 테라포밍-완수 Flag 없으면 info-only.
---

# vllm-recipe-explorer

> **§0.0 진입 전제 — 테라포밍-완수 Flag 게이트 (헌법 §테라포밍-완수 Flag 게이트 따름정리 · plan_26063018)**: 작업(estimate/generate/simulate = 서빙전략 deliverable) 전 **턴 시작 시 Flag 확인 필수**. 미발급(`output/<topology>/manifest.yaml` 부재 · `terraforming.complete/branch_verified != true` · model_source 미설정) 시 **info-only**: 모델 HF조회·개념·절차 설명 OK / **환경특정 deliverable(TP·recipe·serve 명령) 생성 ✗**(음성정직 — HW사실 없이 근거 있어보이는 답 *날조* 금지 = 보고된 버그) → 정본 redirect 템플릿으로 `terraforming_node` 유도. **결정론 백스톱** = `recipe.py` main() 의 `_require_terraform_flag`(estimate/generate/simulate 비0종료·**fail-closed**; **서브 면제 = 양성 위임 키 `.claude/a2a_delegation.json`** — 메인이 동질성 검증 후 발급, 메인 키와 UNIQUE, *부재로 면제 ✗*; 1차 / `EASY_VLLM_A2A_DELEGATED` 테스트 override 2차). TP = manifest(`len(nodes)×gpus_per_node`) 배선(git 브랜치 폴백 ✗) · 가드 tp>GPU·kv_heads%tp.

고정된 **한 모델**을 여러 서빙 레시피로 비교해 **타깃 GPU 예산**에 맞는 설정을 찾는다. 두 페이즈로 동작한다.

- **Phase 1 (추정·추천)**: `config.json` 결정론 파싱 → LLM 후보 생성(탐색) → 결정론 VRAM 추정·하드게이트·랭킹 → 리포트 → HITL 선택 → 3종 세트 생성 + 되먹임 로그. *실서빙 없음(추정만).*
- **Phase 2 (실서빙 검증)**: 멀티턴 인터뷰로 lock/soft/free lock-set 확정 → **실서빙 trial-loop**로 **절대 KV 클램프**를 실측 수렴 → 수렴 시 동일 3종 세트 + simlog 증거. *실제 컨테이너를 띄워 `/health` + 기능 스모크로 검증한다.*

> 설계 원칙(하네스 엔지니어링): **결정/게이트/랭킹/분류는 결정론 스크립트**(`scripts/`)가 책임진다.
> LLM은 후보 '생성'(탐색)과 인터뷰만 한다. OOM-critical 경로(VRAM 추정·게이트·OOM 분류)를 확률론으로
> 단정하지 않는다. whichllm 패키지를 import 하지 않는다 — 값/공식은 소스에서 **복사(벤더링)**하고 출처 주석을 단다.

## Contract

- **Goal** — 한 모델 × 이 하드웨어에서 **실서빙되는** 레시피(3종 세트 .yaml+.sh+.env)를 만들고, KV 를 측정 기반 절대 클램프로 고정해 이식 가능하게 한다.
- **When to invoke** — "이 모델 어떤 설정으로 띄울까" · VRAM 예산 맞춤 · KV/batch/파서/backend 튜닝 · 실서빙 검증 요청. Flag 미발급이면 info-only 로만.
- **Inputs** — `config.yaml`(모델 경로·예산·margin·serving 이름) · 모델 `config.json`+safetensors(NAS) · `output/<topology>/manifest.yaml`(TP·NAS·host_safety) · Phase 2 는 `lockset.json`.
- **Outputs** — `output/<t>/configs/<name>.{yaml,sh}` + `envs/.env.<name>` · `feedback/.last_ranking.json` · Phase 2 는 `docs/simlog/<run_id>/` 원시증거 + 수렴 레시피 · 라이브 serve.
- **Mandatory procedural spine** — 아래 §Mandatory procedural spine 의 9단계(순서 고정).
- **State transitions** — serve `/health` 200 + 기능 스모크 통과로 `runtime-ready` 를 만든다. `evidence-complete`/`promotion-ready` 는 `scripts/completion_gate.py` 소유(이 문서가 자체 판정 ✗).
- **HITL/safety boundaries** — 모델 자동 다운로드 ✗ · 전부-FAIL 인피저블 단정 전 측정·외부검증 2단계 의무 · cap 소진/`vram_infeasible`/`unknown` 은 즉시 Model-C · per-trial teardown 필수.
- **Failure → reference routing** — 아래 §Failure → reference routing 표(증상 → 정확 경로).
- **Deterministic commands** — `recipe.py {estimate,generate,simulate}` · `scripts/parse_model_config.py` · `crosscheck_model_card.py` · `estimate_vram.py` · `rank_recipes.py` · `gen_recipe_set.py` · `run_trial.py` · `sim_classify.py` · `parse_vllm_log.py` · `functional_smoke.py` · `preload_ram_gate.py` · `simlog_writer.py`.
- **Handoff contract** — 입력 ← `terraforming_node`(Flag·TP·NAS) · 빌드 필요/버전 불가 → `upstream-version-watch`(§4.7 native dep · §3.6 escalation) · 서빙 성공 → `adversarial-benchmark` lite(자동) / full(HITL).
- **Owns (state)** — `serving-triplet` · `kv-clamp` · `lockset.json` · `serve-lifecycle`

## Mandatory procedural spine

필수 순서다 — 파싱→교차검증→게이트→(측정)→emit 의 순서가 OOM·오판을 막는 장치다(건너뛰지 않는다).

1. **Flag 확인**(§0.0) — 미발급이면 info-only redirect 로 종료.
2. **parse**(결정론) — `parse_model_config.py`. 모델 부재면 **다운로드 금지·비0 종료·중단·보고**.
3. **모델카드 교차검증** — `crosscheck_model_card.py`. MISMATCH=게이트(비0), special-dep 경보는 분류 후 핸드오프(`references/parse-and-crosscheck.md` §3).
4. **후보 생성**(LLM 탐색 — 이 단계만 확률론) 또는 `--auto` 결정론 그리드.
5. **rank + 하드게이트**(결정론) → 리포트 → **HITL 로 `recipe_id` 선택**. 전부 FAIL 이면 인피저블 단정 전 2단계 의무.
6. **generate**(Phase 1 종료 가능 지점) — 3종 세트 emit. `max-num-seqs`·`kv-cache-memory-bytes` 는 여기서 emit 하지 않는다(측정 산물).
7. **Phase 2 인터뷰 → lock-set**(`references/phase2-interview.md`) — 타깃 GPU 0순위, soft 변수 `*_candidates` 채움.
8. **trial-loop**(`recipe.py simulate`) — 로드-전 RAM 게이트 → run_trial → `sim_classify` → 조정 → 반복(cap 3). 최소 2-트라이얼(측정→클램프 검증)로 **절대 KV 클램프 수렴**(`references/kv-clamp.md`).
9. **마무리**(`references/serving-closeout.md`) — lite 벤치 핸드오프 → opt-out 경고 → 유지/down 분기 → 용처 매뉴얼 → (S4 종결 후) hint 제안.

## Failure → reference routing

| 실패 신호 | 라우팅 대상 (정확 경로) |
|---|---|
| trial 실패의 class 판정(vram_oom / functional / vram_infeasible / unknown) | `.claude/skills/vllm-recipe-explorer/scripts/sim_classify.py` |
| KV 가 안 맞음 · 클램프 산정 · 타겟-GPU 이식 예산 · MoE backend 즉사 | `.claude/skills/vllm-recipe-explorer/references/kv-clamp.md` |
| lock/soft 변수·파서명·backend 폴백 순서를 다시 정해야 함 | `.claude/skills/vllm-recipe-explorer/references/phase2-interview.md` |
| `functional`·`unknown` 복구 절차 · 수렴 후 마무리 시퀀스 | `.claude/skills/vllm-recipe-explorer/references/serving-closeout.md` |
| 파싱·교차검증·랭킹 입력이 의심스러움(coarse quant·du 오염·text_config) | `.claude/skills/vllm-recipe-explorer/references/parse-and-crosscheck.md` |
| 기동 전 호스트 RAM 여유 부족 | `.claude/skills/vllm-recipe-explorer/scripts/preload_ram_gate.py` |
| 모델이 요구하는 native lib(DeepGEMM 류) — 빌드평면 소유 | `.claude/skills/upstream-version-watch/references/source-build.md` |

## 3. 안전 (절대 규칙)

- **모델 자체 다운로드 금지.** NAS 경로에 없으면 비0 종료 + 명확한 중단·보고. 런타임 다운로드 없음.
- **ephemeral 다운로드 승인 요청 시 이** 사전추정 결과(대략 GiB)를 사용자에게 함께 제시할 것.
- 호스트 `python3`(3.12, PyYAML 6) 단독 실행. **whichllm 패키지를 import 하지 말 것**(값은 벤더링).
- stdlib + yaml만 사용. **(b) 소비자 파서(quant_table/estimate_vram)는 로컬 config 파싱만 하는 순수 stdlib — 모델획득 평면 격리이지 환경 offline 이 아니다**(대조적으로 upstream 의 resolver 는 GitHub API·레지스트리를 라이브 조회 = 환경 online 전제). **단 이는 *스크립트* 제약이지 *에이전트 전략수립* 제약이 아니다** — 서빙전략의 외부 교차검증(HF 모델카드·vLLM GitHub)은 허용·의무(헌법 §모델 획득 모드 따름정리 · escalation = `plan_26063009_19_14` B부). **모델획득 격리 한정**이지 외부접속 전반 차단 ✗.
- 결정/게이트/랭킹/분류는 결정론 스크립트가 책임진다(LLM은 후보 탐색·인터뷰만).
- **Phase 2 teardown(필수)**: 트라이얼은 끝날 때마다 `docker rm -f` 로 컨테이너를 제거한다(통합메모리 잔류 OOM 방지 — `run_trial` 의 `finally` 가 보장).
- 빌딩블럭(`.claude/`)은 추적 대상 — 외부/공식 스킬·플러그인은 propose→review→install.

## 5.5 escalation — "현 vLLM 불가" 발견 → upstream 핸드오프 (버전-bump 축 · 발견≠소유)

> `upstream-version-watch` §4.7(빌드-바깥 **native dep** 추가 = *같은* vLLM 버전에 lib 보강)와 **다른 축**: 여기는 **vLLM 버전 자체가 모델을 못 받는** 경우(공식 미지원·포크 필요·transformers-only). 헌법 §escalation 역루프 따름정리 · 절차-홈 `workflow.md` §escalation 역루프.

- **드문 예외 경로**: 대다수 신규 모델은 현 컨테이너로 그냥 뜬다 → **매 서빙요청마다 외부리서치 ✗**. 구동불가 증상이 "못 띄움"을 가리킬 때만 발동.
- **발견 술어(둘 다 요구 — 오발 방지)**: ① **구동불가 증상**(config arch/quant 미지원, serve init 즉사, transformers-only 폴백 신호) ∧ ② **외부 교차검증 확증**(HF 모델카드의 포크 지목 + vLLM GitHub issue/release/PR). **단일 신호로 escalate ✗**.
- **핸드오프(recipe는 소유 ✗)**: 사용자에게 *"이건 vLLM 측 문제 → upstream 에 버전핀을 넘길까요?"* **명시 승인 요청** → 승인 시 `upstream-version-watch` §escalation 으로 핸드오프(증거 첨부). **버전핀 변경·이미지 빌드를 하지 않는다**. rebuild 된 이미지로 전략수립 재진입.
- **carry-forward 금지 정합**: escalation 판정도 **모델×하드웨어마다 재확정**(이전 모델의 "됐다/안 됐다" 전가 ✗).
- **런타임블럭/서브 주의**: 외부검색은 **이중게이트**(A2A 위임 키 ∧ egress-online) 조건부다. restricted 서브는 **증상만 docs insight 상향 보고**(D12), 메인이 외부검색·처방한다.

## 7. 범위 (Phase 1 / Phase 2)

- **Phase 1 (In)**: 파싱 → 후보 생성 → 결정론 추정·하드게이트·랭킹 → 리포트 → HITL → 3종 세트 → 되먹임 로그. *실서빙 없음.*
- **Phase 2 (In)**: 인터뷰 → lock-set → 실서빙 trial-loop(절대 KV 클램프 수렴) → 검증된 3종 세트 + simlog 증거.
- **Out (명시적 제외)**: 품질/정확도 측정·랭킹(영구 밖) · 속도/레이턴시 벤치(=`adversarial-benchmark`) · `tensor-parallel-size` 축 자동탐색 · 멀티노드 2-node Ray 서빙(workflow S3).

## 8. 보조 파일

**Phase 1 스크립트**: `scripts/quant_table.py` · `parse_model_config.py` · `crosscheck_model_card.py`(모델카드 교차검증 + 외부 HF API VRAM 이중검증 — 유일한 예외적 네트워크 호출) · `estimate_vram.py` · `rank_recipes.py` · `gen_recipe_set.py` · `feedback_log.py`.

**Phase 2 스크립트**: `scripts/parse_vllm_log.py` · `functional_smoke.py` · `sim_classify.py` · `run_trial.py` · `preload_ram_gate.py` · `simlog_writer.py`(+`vllm_logging_config.json`).

**오케스트레이터 / 입력**: `recipe.py`(`estimate`/`generate`/`simulate`) · `config.example.yaml` · `config.yaml` · `lockset.json`.

**조건부 references(필요할 때만 연다)**
- `references/parse-and-crosscheck.md` — config 입력·결정론 경계표·parse/crosscheck/후보/rank/generate 상세.
- `references/phase2-interview.md` — lock/soft/free 인터뷰 질문지·파서 caveat·lock-set 스키마·UX 4항목.
- `references/kv-clamp.md` — 절대 KV 클램프·타겟-GPU 이식 예산·측정 vs 공식·인코딩 사전적재·MoE 백엔드·런타임 패치.
- `references/serving-closeout.md` — trial-loop 배선·참조-그라운디드 복구·마무리(lite/용처/hint) 시퀀스.

근거: `docs/plan/plan_2026060819_1`(Phase 1 승인본) · `plan_2026062121_1`(Phase 2) · Seed `seed_e02364cce49d`.
