---
name: upstream-version-watch
description: >-
  vLLM 업스트림 신버전을 추적해 변경을 독해하고, 커스텀 Docker 컨테이너 레이어(베이스 이미지·
  핵심 wheel·주변 의존성)의 bump를 제안한다. "vllm 업데이트", "vllm 버전 올려", "신규 vllm 버전
  반영", "컨테이너 버전 bump", "NGC 베이스 맞춰줘" 같은 지시에 발동. 제안만 하며, 실제 핀 변경·
  빌드·push는 .claude/rules/workflow.md 의 전파 워크플로(HITL 게이트)를 따른다.
---

# upstream-version-watch

업스트림 vLLM을 추적해 **최신 버전 감지(사람 지시) → 변경 독해 → 레이어 bump 제안**까지 수행한다.
이 스킬은 **제안한다.** 실제 적용은 핀 정책(`CLAUDE.md`)과 HITL 게이트(`.claude/rules/workflow.md`)를 따른다.

> **§0.0 진입 전제 — 테라포밍-완수 Flag 게이트 (헌법 §테라포밍-완수 Flag 게이트 따름정리 · plan_26063018)**: 컨테이너 빌드·렌더·bump(작업) 전 **Flag 확인 필수**. 미발급 시 **info-only**: vLLM GitHub 릴리즈 조회·버전해소 *설명* OK / **렌더·빌드·bump ✗** → "빌드는 HW(arch·cuda)·모델경로(manifest)를 알아야 — `terraforming_node` 먼저"로 redirect. 빌드 시 manifest `model_source`(managed|ephemeral|custom)로 compose 분기(P4). **결정론 백스톱** = 빌드/렌더 작업 *前* 에이전트가 `python3 .claude/skills/terraforming_node/scripts/manifest_contract.py --topology <t> --require-flag` 실행(workflow S3 게이트). upstream 은 main-only 빌딩블럭·다단계 빌드라 단일 작업스크립트 진입점이 분산 → recipe(replicated, main() 베이크-인)와 달리 **S3 워크플로 게이트가 정본**.

> 설계 원칙(하네스 엔지니어링): **버전 문자열 해소는 결정론적 스크립트**(`scripts/`)가, 변경 요약·
> 리스크 판단은 모델이 한다. 버전·태그를 추측(확률론)으로 단정하지 않는다.

## Contract

- **Goal** — 대상 vLLM 버전을 결정론으로 해소해 컨테이너 레이어(NGC 베이스·wheel·deps·빌드트랙) bump 를 제안하고, 승인된 핀으로 렌더·빌드·스모크까지 몰고 간다.
- **When to invoke** — 사람의 "업데이트/bump" 지시 · `vllm-recipe-explorer` §5.5 escalation 수신 · arch-wall 로 변종 트랙 결정이 필요할 때. 자동 폴링·cron ✗.
- **Inputs** — `config.yaml`(대상 버전·스모크 config_name·`ngc_probe_start`·`reconciliation_cap`) · `output/<topology>/manifest.yaml`(HW 사실·model_source) · 실패 시 빌드/serve 로그.
- **Outputs** — `resolved.json`(torch핀·NGC태그·wheel·build_track·변종·**`upstream_delta` 델타 attestation**) · `output/<t>/{Dockerfile*,docker-compose.yaml,requirements.txt,.env}` · bump 제안표 + risk-memo · 스모크 판정.
- **Mandatory procedural spine** — 아래 §Mandatory procedural spine 의 8단계(순서 고정).
- **State transitions** — Flag(전제) → 스모크 PASS 로 이미지의 `runtime-ready` 근거를 만든다. `evidence-complete`/`promotion-ready` 판정은 `.claude/policies/runtime/completion_gate.py` 소유(이 문서가 자체 판정 ✗).
- **HITL/safety boundaries** — 핀 변경·빌드·push 는 workflow S1–S4 HITL 게이트 · 스모크 모델 자동 다운로드 ✗ · 무증거 NGC/repo 오버라이드 ✗ · 추측 단정 ✗("확인 필요").
- **Failure → reference routing** — 아래 §Failure → reference routing 표(증상 → 정확 경로).
- **Deterministic commands** — `scripts/resolve_torch_pin.py` · `resolve_ngc_tag.py` · `resolve_wheel.py` · `regen_requirements.py` · `resolve_build_track.py` · **`judge_version_delta.py`** · `render_dockerfile.py` · `check_smoke_model.py` · `classify_failure.py` · `sync_to_sub.sh` · `multinode_serve_smoke.sh` · **`single_serve_down.sh`**.
- **Handoff contract** — 입력 ← `terraforming_node`(Flag·HW) · escalation ← `vllm-recipe-explorer` §5.5 / `adversarial-benchmark` §7 · 출력 → rebuild 이미지로 `vllm-recipe-explorer` 전략수립 재개.
- **Owns (state)** — `resolved.json` · `image-identity` · `build-track` · `sub-delivery` · **`build-patch(pre/post)`** · **`fork-pin`**(포크 좌표·arch-wall 변종)
- **3+1+1 소유 경계**(`plan_26081514` Q3/Step 4 · owner 표 정본 = `.claude/rules/workflow.md` §3+1+1): **빌드 시점에 성립하는 것**이 이 스킬 소유다 — `build_patches_src/`(pre · 컴파일 **전** 소스 수정) · `build_patches/`(post · 컴파일 **후** native 의존) · 포크 핀(`VLLM_REPO`/`VLLM_REF`)·변종 `IMAGE_TAG`. **serve 시점에 성립하는 것**(트리플렛 3 + 런타임 패치 `<model>_patch.py`)은 `vllm-recipe-explorer` 소유이며 이 스킬이 저작하지 않는다. **발견 ≠ 소유** — explorer 가 §5.5 로 발견해 넘긴 것을 이 스킬이 **소유·처방**한다(수신점 = 아래 §escalation 수신).

## Mandatory procedural spine

필수 순서다 — 앞 단계 산출 없이 뒤 단계로 가지 않는다(부분 적용 상태 빌드 금지 = 이 순서의 보장).

1. **Flag 확인**(§0.0) — `manifest_contract.py --require-flag`. 미발급이면 여기서 멈추고 info-only.
2. **resolve** — ①torch핀 → ②NGC 태그 → ③wheel URL → ④requirements 재생성 → ⑦빌드트랙 제안. 명령·근거 = `references/resolve-and-render.md` §1.
2.5. **delta judge** — `judge_version_delta.py` 로 `from_ref→to_ref` 3축 판정(A 빌드입력 · B 이식 스코프 · C 모델 코드경로) → `resolved.json#upstream_delta` attestation 발행. **3의 사실 행이 여기서 나온다**(손저작 금지 — 전사 오류가 구조적으로 불가능해진다). 비-0 = 델타 수집 실패(추정 금지). 계약 = `references/resolve-and-render.md` §1.5.
3. **bump 제안표 + risk-memo** — 사실 행은 스크립트 JSON 그대로, LLM 은 리스크 해석만(`references/resolve-and-render.md` §2). HITL 게이트 ①.
3.5. **HITL 게이트 ①.5** — 3축 verdict 를 **렌더 전** 사람이 확인: ⓐ `axis_A` 가 정말 ∅ 인지 ⓑ `axis_B.silent_revert_risk` 처리 계획 ⓒ `unknown[]` 이 비었는지. **`UNDETERMINED` 면 렌더 진입 금지.** 출구① 상속을 쓸 때만 **게이트 ①.6**(원장 한 줄을 **사람이 손으로** 추가) — 3출구 계약 = `references/source-build.md` §3.1.
4. **render 시퀀스** — template → (multi)`--materialize-configs` → `--materialize-env`. **단일 호출 아님**(누락 시 serve 잠복 실패). HITL 게이트 ②.
5. **(multi) sync to sub** — `sync_to_sub.sh` dry-run → `--apply`. Band2-only 전달 경계 = `references/multinode-build.md`.
6. **smoke** — NAS 체크(⑤) → 빌드 → 실서빙 스모크(단일 `docker compose`, 멀티 `multinode_serve_smoke.sh`). 실패면 ⑥`classify_failure.py` 로 분기 → §Failure routing. HITL 게이트 ③.
7. **escalation 수신 판정**(있을 때만) — 아래 §escalation 3출구.
8. **종결** — 커밋·문서·전파(workflow S4) 후 hint 태그 제안(아래 §hint). HITL 게이트 ④.

## Failure → reference routing

| 실패 신호 | 라우팅 대상 (정확 경로) |
|---|---|
| 빌드/스모크 실패 — class 판정이 먼저 필요 | `.claude/skills/upstream-version-watch/scripts/classify_failure.py` |
| `requirements-fixable` · `unknown`(Model-C 참조-그라운디드) | `.claude/skills/upstream-version-watch/references/failure-recovery.md` |
| `source-build-class`(torch 2.11+ ABI 벽) · 빌드-바깥 native dep · 패치 래더 | `.claude/skills/upstream-version-watch/references/source-build.md` |
| 2노드 Ray 서빙 실패(OOM/NCCL-RDMA/join timeout) · 서브만 빌드 실패 | `.claude/skills/upstream-version-watch/references/multinode-build.md` |
| render 후 serve 가 `/mnt/models` 로 오마운트(결함#2) · 해소값 재확인 | `.claude/skills/upstream-version-watch/references/resolve-and-render.md` |
| 미인식 `(NGC×vLLM)` 키로 렌더가 막힘 · 상속 거부 스탠자(`NOT eligible`) | `.claude/skills/upstream-version-watch/references/source-build.md` §3.1(가드 3출구) |
| 델타 판정 `UNDETERMINED`/`UNKNOWN_PLANE` — 렌더 진입 불가 | `.claude/skills/upstream-version-watch/references/resolve-and-render.md` §1.5 |
| 스모크 모델이 NAS 에 부재 | `.claude/skills/upstream-version-watch/scripts/check_smoke_model.py` |
| 서브 위임의 provider 실행문법이 필요 | `.claude/skills/terraforming_node/references/agent-control-adapter.md` |

## escalation 수신 — recipe 핸드오프 → 버전핀 소유·3출구 (발견≠소유의 버전-bump 축)

> `vllm-recipe-explorer` §5.5가 "현 vLLM 불가"를 외부 교차검증으로 **발견**하고 사용자 승인을 거쳐 넘긴 핸드오프의 **수신점**. recipe는 발견·핸드오프까지, **버전핀 소유·처방·rebuild는 본 스킬**. 헌법 §escalation 역루프 따름정리 · `plan_26063009_19_14` · 절차-홈 `workflow.md` §escalation 역루프.

- **진입(승인 완료 전제)**: recipe가 첨부한 증거(구동불가 증상 + 외부 확증: HF 모델카드·vLLM GitHub issue/release/PR) 수신 → **버전해소 리서치**(release 노트·머지 PR·포크) → **3출구 판정**. 무승인/무증거 수신 ✗.
- **(i) 공식 bump** — 모델이 더 새 *공식* vLLM release 에서 지원 → **표준 bump 경로**(`workflow.md` S1–S3, HITL 게이트). 가장 단순한 출구.
- **(ii) 커스텀/포크핀** — 모델카드가 포크·미머지 PR 지목 → **source-repo 오버라이드**(fork **SHA 핀** `VLLM_REPO`/`VLLM_REF` build-arg) + `…-source-<변종>` superset 변종 트랙(`resolved.json` `source_build_variants`). 거버넌스 = 아치-enablement 변종 트랙 따름정리(클러스터-와이드 이미지·**기존모델 회귀 재스모크**·단일 변종-트랙·무증거 오버라이드 금지). 절차 정본 = `workflow.md` S3 arch-wall 분기 · 사다리 상세 = `references/source-build.md` §5.
- **(iii) 음성정직** — 공식·포크 모두 부재, transformers-only → *"현재 vLLM으로 서빙 불가"* 보고(없는 길 날조 ✗). **단 이 선언은 `.claude/skills/wiki-desk/reference/references.md` §5 최소범위 레시피 수행 + testlog "탐색 증거"(검색어·URL·일자) 기록 후에만 허용** — 가장 강한 부정 결론엔 가장 강한 증거.
- **최종 중재 = 스모크**(린트·이슈글 ≠ 서빙됨). **순환 차단** — rebuild 후도 미구동이면 `reconciliation_cap` 한정 재진입 → 소진 시 Model-C(무한 bump ✗). 완료 후 rebuild 이미지로 `vllm-recipe-explorer` 재진입.

## hint 태그 발동 (bump closer) — **소유는 `hint-publisher`**

**전작업 완료 후** — bump 사이클이 **서빙성공+커밋+문서+전파까지 끝난** S4 종결부에서, 새 `(vllm×model×arch)` 면
hint 태그 발행을 **제안(Y/N)** 한다(**무인 자동 태깅 ✗** · **push 는 전부 사용자 소관** — **브랜치 push 도 루틴 대상 아님**).

> **2026-08-20 이관**(`plan_26082009`): 절차·엔진·계약·템플릿·린터의 정본은 **스킬 `hint-publisher`** 다.
> 이 스킬은 **제안 트리거만** 갖는다 — 발행 조건 A(서빙 성공)는 `vllm-recipe-explorer`,
> B(lite 계측)는 `adversarial-benchmark` 소유이므로 엔진이 여기 있을 이유가 없었다(배치 오류).
> 넘길 때 함께 전달할 것: 앵커 커밋 · topology · promotion-ready work-manifest 경로 · 정본 HF repo.

절차 `.claude/skills/hint-publisher/SKILL.md` · 헌법 §hint 배포 레이어 따름정리 §발동 시점 ·
절차-홈 `workflow.md` S4 · 설계 `plan_26070222`·`plan_26071607`·`plan_26082008`. main-only(서브 미전파).

## 금지

- 사람 승인(HITL 게이트) 없는 핀 변경·자동 push.
- 스모크 모델 자동 다운로드(NAS 부재 시 중단·보고).
- 버전·태그·매핑을 추측으로 단정(불명은 "확인 필요").
- 무증거 NGC 베이스/소스-repo 오버라이드 · 단계 건너뛴 부분 적용 빌드.
- **`INHERITED_SOURCE_BUILD_KEYS` 자동 등재**(에이전트가 항목을 추가하는 것). 등재는 게이트 ①.6 의 **사람 손**이다 — Judge 산출물은 evidence 이지 approval 이 아니다. 부적격 상속을 `--allow-unvalidated` 로 우회하는 것도 금지(원장 결함은 고쳐야 한다).

## 보조 파일

**결정론 스크립트**
- `scripts/resolve_torch_pin.py` `resolve_ngc_tag.py` `resolve_wheel.py` — ①②③ 해소.
- `scripts/regen_requirements.py` — ④ requirements 재생성(wheel METADATA 기준, `--from-wheel-url`/`--use-installed`).
- `scripts/resolve_build_track.py` — ⑦ 트랙 제안자(휴리스틱만 결정론, 최종은 스모크 중재).
- `scripts/judge_version_delta.py` — 2.5 버전 델타 3축 영향판정(D0 수집 · D1 닫힌열거 평면 · D2 이식 스코프 교차 + 침묵-되돌림 프로브). `--self-test`/`--check-fixture` 회귀. **C축(D3·D4)은 미구현**(`NOT_IMPLEMENTED` · 전역 verdict 에서 제외).
- `scripts/render_dockerfile.py` — 렌더 시퀀스(template · `--materialize-configs` · `--materialize-env`) + source-build 가드 3출구(상속/시도-빌드/발견 — `references/source-build.md` §3.1).
- `scripts/check_smoke_model.py` — ⑤ no-download NAS 모델 실재 체크.
- `scripts/classify_failure.py` + `failure_patterns.yaml` — ⑥ 실패 결정론 분류(미지→Model-C).
- `scripts/sync_to_sub.sh` — 멀티노드 [전달](dry-run 기본/`--apply`, 체크섬, 빌딩블럭 제외).
- `scripts/multinode_serve_smoke.sh` — 2노드 Ray 서빙+multi-smoke 오케스트레이션. **teardown(`--down`)은 멀티 전용**이다(경로가 `output/multi/` 고정 · SSH 슬레이브 전제).
  - 스모크 합격 기준은 **생성 실재**다(2026-08-22 · W-10): `content` 비었고 `reasoning` 만 있으면
    PASS 하지 않고 파서를 우회한 `/v1/completions` 로 확증한다. 통과 로그는 언제나 증거원을 밝힌다
    (`evidence=chat.content` \| `v1.completions`). 완화 탈출구 `SMOKE_ALLOW_REASONING_ONLY=1` 은
    쓰면 로그에 크게 남는다.
  - `--keep-up` 상주분은 **예산 갱신 루프**(`budget_renew_loop.sh`)를 양 노드에 무장한다(W-7).
    컨테이너가 사라지면 루프가 스스로 끝나고, 회수는 `--down` 이 함께 한다.
- `scripts/single_serve_down.sh` — **단일노드 정규 teardown 진입점**(2026-08-22 신설 · W-6).
  `bash single_serve_down.sh <config> [--dry-run]`. 5단계(컨테이너 down · 상주 사이드카 회수 ·
  페이지캐시 · 예산선언 회수 · 블랙박스 세션 stop)를 수행하고 **각 단계의 DONE/SKIPPED(사유)/FAIL 을
  반드시 출력한다** — README §B-3 의 `docker compose … down` 직접 호출은 뒤 셋을 조용히 빠뜨린다
  (`testlog_26082215` §4.0 R-0). 열린 세션이 둘 이상이면 조용히 고르지 않고 fail-loud 한다.

**조건부 references(필요할 때만 연다)**
- `references/resolve-and-render.md` — config 입력 · Phase 분기 · ①~⑦ 해소 근거 · bump 매핑표 · render 시퀀스.
- `references/failure-recovery.md` — class 별 처방(requirements-fixable / source-build-class / 멀티 / arch-wall / unknown).
- `references/source-build.md` — Phase 2 소스빌드 · 패치 동결 라이프사이클 · `build_patches/` · 패치 래더.
- `references/multinode-build.md` — 멀티노드 4단 절차 + 학습(health 판정·ray 핀·Band2 경계).

**기타**
- (서브노드 CC 페르소나·Agent_Card·통신프로토콜은 **`terraforming_node` 소유·렌더** — 이 스킬은 `sync_to_sub.sh`·`multinode_serve_smoke.sh` 제어평면만.)
- `<repo>/Dockerfile.source-build` — Phase 2 소스빌드 동결 산출물. `config.example.yaml` — 입력 스키마.
- 외부 레퍼런스 = **`.claude/skills/wiki-desk/reference/references.md`**(ID→URL 정규화·HW-스코프·부정판정 최소범위 레시피). 외부검색 전 1차 조회(warm-start).
