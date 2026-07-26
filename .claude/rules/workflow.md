# workflow.md — 업스트림 추적 컨테이너 버전관리 전파 워크플로

> 이 파일은 다단계 절차 규칙이다. "항상 참인 사실"은 루트 `CLAUDE.md`에 있다.
> 사람이 "vLLM X로 업데이트" 지시를 내렸을 때 에이전트가 실행하는 절차를 정의한다.

## 스킬 파이프라인 순서 (plan_26063009_44_23)

- **의존순서**: `terraforming_node` → `upstream-version-watch` → `vllm-recipe-explorer` →
  `adversarial-benchmark`. "우선순위" = 의존순서(앞이 뒤의 전제)이지 충돌 승자가 아니다.
- **발동지점**: Flag 발급 후 빌드 질문 = upstream(제안만 — 빌드/bump 실행은 사람 지시) · 서빙전략 지시 =
  recipe · 성능 의심 = adversarial(recipe와 loop-until-done).

## init/runtime 2-모드 · 테라포밍-완수 Flag 게이트 (모든 런타임 절차의 진입 전제 · plan_26063018)

> "항상 참" 요약 = 루트 `CLAUDE.md` §테라포밍-완수 Flag 게이트 따름정리. 여기 = **절차-홈**.

- **init 순서**: `terraforming_node` 인터뷰 → real init-plan 발행(HITL) → 스캔 → topology/branch 정합 확인 → manifest attestation 기록. Runtime 진입 의미의 정본은 `policy:TERRAFORM_FLAG_GATE`다.
- **진입 검사**: S1–S4·escalation·B0–B3 시작 전 각 skill의 tracked validator를 실행하고, 비0종료면 작업을 중단한 뒤 아래 redirect 템플릿을 출력한다. 서브 진입은 `policy:A2A_DELEGATION_KEY_FAIL_CLOSED`의 validator를 함께 호출한다.
- **redirect 템플릿(정본)**: *"HW스캔이 덜 되어(Flag 미발행) HW 스펙(GPU·OS)을 알기 어려워 모델 `<HF URL>` 의 정확한 서빙전략을 세우기 어렵습니다. `terraforming_node` 로 ① HW스캔 + ② 모델 다운로드 전략(관리 NAS 경로? 컨테이너 임시 다운로드(컨테이너 down 시 삭제)? 특정 경로 저장·마운트?)을 먼저 정합시다."*
- **fresh-clone 능동발동**: `config.yaml`과 두 토폴로지 manifest가 모두 없거나 완수 Flag attestation이
  없으면 미테라포밍 신호로 보고, "이제 뭐해야해" 류 및 구체 서빙 요청 모두에 온보딩을 능동 제안한다
  (감지 → 제안 → 인터뷰 → 승인 → 스캔 순서 — 무단 스캔 금지는 스캔이 인터뷰·승인 뒤이므로 보존된다).
  구체 서빙 요청은 info-only redirect로 유도한다.
- **토폴로지는 인터뷰로 결정**: `terraforming_node`의 첫 동작이 토폴로지(단일/멀티) 인터뷰이며, 브랜치는
  작업공간 선택기일 뿐 토폴로지 추론에 쓰지 않는다. 미선언 시 emit은 fail-closed하고, 브랜치와 토폴로지가
  불일치하면 HITL로 브랜치를 전환한다.
- 절차 상세 = 스킬 `terraforming_node` §0.5 · 각 런타임 스킬 SKILL.md §0.

## 런타임 전파 4단계 (각 단계 = verify 동반)

```text
S1 resolve  → 대상 vLLM 버전 해소 (결정론적 스크립트)
   - torch핀 추출(pyproject [build-system].requires) → 접두어매칭 NGC 베이스 태그 선정 + 빌드트랙(torch세대 → wheel/source) 판정: 절차·결정론 스크립트 = 스킬 upstream-version-watch §1, 트랙분기 근거 = 스킬 §0.5 (여기 재서술 안 함 — 레이어 커플링 규칙)
   - CUDA_VERSION · CPU_ARCH($(uname -m)) · wheel URL 확정
   - 주변 의존성: vLLM requirements/{common,cuda,build}.txt + pyproject.toml 로 requirements.txt 재생성
   verify: 산출값(torch핀·NGC태그·CUDA·wheel URL·deps diff) 출력
   ── HITL 게이트 ① : 해소 결과를 사람이 확인한 뒤 S2 진행

S2 patch    → single-node · multi-node 두 브랜치 패치
   - Dockerfile ARG VLLM_VERSION · ARG CUDA_VERSION · FROM 태그 · manylinux 갱신
   - requirements.txt 갱신
   - render 는 시퀀스(스킬 §2.5): template(Dockerfile/compose)+regen_requirements + (multi)materialize-configs + materialize-env
     (--materialize-env → output/<t>/.env: serve-time NAS_MODEL_PATH/TIKTOKEN_HOST_PATH manifest 전파. 누락 시 serve 가 /mnt/models 기본마운트로 실패 — 결함#2)
   - 모델구동 런타임 패치(필요 시): 에이전트가 참조-그라운디드로 configs/<model>_patch.py 생성(휘발·비추적) + arm_patch.sh(materialize-configs 에 포함)가 serve_runner/생성.sh 에서 자동 arm. 헌법 모델구동 런타임 패치 따름정리.
   - multi-node 브랜치: 네트워크 디버그 apt · serve_runner.sh(Ray) · NCCL/RDMA env · /dev/infiniband은 보존(건드리지 않음)
   - multi-node: .gitignore 정렬 확인(산출물 통로·사적 파일 재제외 구조 유지). 빌딩블럭 동기화 정책 = 헌법 §금지 참조(S4 에서 실행).
   verify: 변경 라인이 S1 해소값에 직결(Karpathy B3)
   ── HITL 게이트 ② : 각 브랜치 diff를 사람이 검토

S2.5 sync   → (multi-node 전용) 메인 검증코드 → 서브 직접 전달
   - .claude/skills/upstream-version-watch/scripts/sync_to_sub.sh (기본 dry-run → --apply): rsync over SSH(인터커넥트는 manifest.interconnect), 체크섬 검증.
     서브 노드 접속값(host·ssh_user)은 manifest.yaml `nodes[]`에서 읽는다.
     빌딩블럭(.git/seed/docs/메인 CLAUDE.md 등)은 제외하되 **런타임블럭·위임키는 선별 오버레이 배달**(recipe.py·adversarial scripts(**lite_bench.sh·lite_metrics.py 포함** — git-tracked 자동 전파, `render_sub_env._copy_tracked`)·task-report.schema.json·a2a_delegation.json — 서브 렌더 페르소나 보호와 공존, 스크립트 실체 기준). GitHub 경유 X.
   - 서브는 메인 전달 코드로 생존. 서브 자작 envs/configs는 메인이 아카이브.
   - 모델구동 런타임 패치(configs/<model>_patch.py · arm_patch.sh)도 output/<topology>/ 에 있어 이 rsync 로 함께 하향 배달(서브 슬레이브가 마운트·arm). 서브는 패치 저작 ✗(상향은 docs 탐지보고만 — D12-06·13). 헌법 패치 전파.
   - `sync_to_sub.sh`는 `policy:MODEL_TRIPLET_NO_SUB_PROPAGATION`에 정의된 전달 필터를 적용하고, 선택된 runtime patch와 Band2 env만 rsync 입력에 넣는다.
   - `multinode_serve_smoke.sh`는 `policy:VARIANT_IMAGE_BUILD_VS_SERVE_PLANE`에 따라 이미지 identity 입력과 serve-config 입력을 분리해 slave compose를 렌더한다.

S3 smoke    → NAS 체크 + 로컬 빌드 + 실-서빙 스모크
   - ⑤ NAS 체크: `check_smoke_model.py <config_name> --topology <single|multi>`를 실행한다. 모델 부재면 중단·보고하고 §모델/안전의 `policy:MODEL_ACQUISITION_TERNARY_GATE` 절차로 분기한다.
   - ⑤.5 로드-전 RAM 게이트: check_smoke_model.py 가 모델 실재 확인 직후 preload_ram_gate 를 호출해 판정하고, 그 결과에 따라 기동을 진행하거나 거부한다. recipe simulate 경로는 recipe.py 가 매 trial 전 동일 게이트를 호출한다. `policy:HOST_SAFETY_LAYERED_DEFENSE`.
   - 단일노드는 렌더된 debug/serve profile 순서로 빌드·기동하고 프롬프트 스모크를 실행한다. 시작 전 `policy:HOST_SAFETY_LAYERED_DEFENSE`의 tracked preflight를 호출한다.
   - 대형 소스빌드는 host-safety preflight 결과와 현재 serve 상태를 확인한 뒤 해당 skill script가 정한 순서로 수행한다.
   - near-max batch(요구 시): 서빙 docker logs 의 kv_cache_tokens/max_concurrency 로 실측 near-max 산출(Phase-1.5, 스킬 §5) → recipe max-num-seqs 보강. 공식 batch 금지(헌법 near-max 따름정리).
   - KV recipe emission은 `policy:KV_ABSOLUTE_CLAMP_PORTABILITY`를 입력 계약으로 삼아 recipe script가 산출한다.
   - Runtime patch가 필요한 분기는 `policy:RUNTIME_PATCH_NO_CARRY_FORWARD`와 recipe script의 arming 순서를 따른 뒤 S3 스모크로 돌아온다.
   - (multi-node) 2노드 Ray 서빙: .claude/skills/upstream-version-watch/scripts/multinode_serve_smoke.sh <config> [--build]
       NAS체크(+RAM게이트, `policy:HOST_SAFETY_LAYERED_DEFENSE`) → 양노드 병렬빌드 → master(메인)+slave(서브) Ray클러스터 기동 → 엔드포인트 health 폴링 → 통과 시 master 엔드포인트 추론, 실패 시 정리 절차(`policy:HOST_SAFETY_LAYERED_DEFENSE`)
       준비판정 = :PORT/health http200 (master 로그 "startup complete"는 거짓양성 — grep 금지)
       reasoning 모델은 max_tokens 충분히(finish_reason=stop)
   verify: 스모크 통과
   ── 실패 시 ⑥ classify_failure.py 로 분기:
        · requirements-fixable → Loop-Until-Done(조정→재빌드→스모크), reconciliation_cap(기본 3) 한정. 소진→Model-C
        · source-build-class(torch 2.11+) → Phase 2 소스빌드 경로(SKILL.md §4.6, 검증됨): 인터랙티브 컨테이너에서
            _C를 NGC torch에 맞춰 컴파일(ABI 벽 해소) → 경험적·HITL 패치 루프(strip-hoist 등) → 스모크 →
            Dockerfile.source-build 동결 → clean 재빌드 재현. 패치는 판단계층(사전-codify 금지). 검증된 패치는 *재현성* 위해 Dockerfile.source-build 에 동결(**카탈로그 졸업 아님** — plan_26062711 Part 3 · 스킬 §4.6).
            └ NGC 베이스 오버라이드 = 1급 Model-C 서브분기(베이스 torch의 ABI 결여 심볼로 source-build FAIL일 때):
              (source-build 맥락·헤더grep 기법의 스킬-홈 = 스킬 upstream-version-watch §4.6 ↔ S3 = 절차-홈, 상호참조)
                ① classify=unknown → 정지(자동 행동 금지).
                ② **참조-그라운디드 해결**: 후보 신규 NGC 베이스의 torch::stable 헤더(tensor_struct.h/ops.h)를 grep해 결여 심볼(예: layout()/6-arg from_blob)이
                   **그 후보 베이스엔 존재함**을 사전 확증(현 베이스엔 부재가 빌드로그로 증명된 상태) — 자기추론 전 권위참조(헤더·빌드로그), 토큰 증가는 정확도를 사므로 장려.
                ③ 증거를 testlog에 기록 + 사람 승인 후 resolved.json의 NGC 태그만 오버라이드
                   (특정 버전값은 manifest·resolved.json에서 — 헌법·workflow에 박지 않음. 26.05 등 하드코딩 금지).
                ④ re-render → clean 재빌드 → 스모크. **무증거 오버라이드 금지.**
                   (resolve_ngc_tag.py 단발 prefix-매칭은 유지; 오버라이드는 이 워크플로 HITL 레이어.)
        · stock-구조적-불가(arch-wall: 대상 모델이 stock vLLM서 sm_xxx 하드월로 토큰 1개 전 사망 — 예 GB10 sm_121 DeepSeek-V4 = 어텐션 major∈[9,10] + MXFP4 오라클 비-repack 백엔드 전무→MARLIN-repack→통합메모리 OOM·호스트 하드다운). **반응적 진입**(빌드/스모크 실패 後) — 동일 처방의 **예방적 진입**(빌드 前 외부검증 발견) = §escalation 역루프.
            → **vLLM 소스-repo 오버라이드 = 1급 Model-C 서브분기**(NGC 베이스 오버라이드와 동일 HITL 핀-오버라이드 메커니즘, repo 축. 도커 패치 범위 사다리: deps-패치 → 소스-게이트 패치 → **소스-repo 오버라이드(포크 핀)** → 체크포인트-교체):
                ① 참조-그라운디드 확증(빌드 前): 후보 포크(예 jasl/vllm PR#41834)의 소스 직독으로 (a) stock 하드월 해소 (b) 비-repack 경로 존재하나 **명시 선택 필요**(oracle 직독 — auto=walled fallback=MARLIN-repack) 사전 확증. 커뮤니티 검증(동일 HW)도 증거. 자기추론 전 권위참조.
                ② testlog 기록 + 사람 승인 후 canonical `.claude/policies/arch_variant_ledger.json` `source_build_variants`에 `VLLM_REPO`/`VLLM_REF`(**SHA 핀** — force-push 면역, 태그명 금지) + 새 아치-트랙(`…-source-sm12x`, superset·모델-키잉 ✗) 기입하고, render input `resolved.json`에는 이를 mirror한다.
                ③ Dockerfile build-arg 파라미터화(`ARG VLLM_REPO`/`VLLM_REF` 기본=stock · blobless clone + SHA checkout) + compose `build.args` → 같은 Dockerfile이 stock/포크 분기. 멀티=클러스터-와이드 이미지(S2.5 슬레이브 이미지-정체성 전달). clean 빌드(양노드) → 스모크. **무증거 오버라이드 금지.** 헌법 "아치-enablement 변종 트랙 따름정리".
        · multi-node 서빙 실패(OOM/NCCL-RDMA/Ray join timeout) → Model-C(HITL). 서브만 빌드 실패=환경 불일치→Model-C
        · unknown              → Model-C: LLM {proposed_class, evidence} 제시 → 사람 승인 전 무행동
   ── HITL 게이트 ③ : 스모크 결과(+분류·risk-memo)를 사람이 확인 (smoke-before-commit)

S4 commit   → 스모크 통과분만 로컬 last-good 커밋 + 서브 전파 + 기록
   (origin은 사용자 환경 값(manifest.origin_url)에서 설정 — 없으면 로컬 전용·push 단계 생략.)
   - single-node · multi-node 각 브랜치 로컬 커밋 = 기록/last-good 앵커(브랜치 핀 독립 — `policy:LAST_GOOD_ROLLBACK_ANCHOR`). 필요 시 태깅한다.
   - 브랜치 간 공유 빌딩블럭 동기화(헌법 §금지 참조): `sync_branches.sh` 를 수동으로 실행한다(작업 종료 후 사람 질의).
   - 서브노드 전파: S2.5의 .claude/skills/upstream-version-watch/scripts/sync_to_sub.sh로 메인→서브 직접 rsync(검증됨). GitHub 경유 안 함.
   - 산출물 통로(헌법 "산출물 통로 불변식" 참조): render 산출물(Dockerfile · docker-compose.yaml · configs/*.{yaml,sh} · envs/.env.* · requirements.txt)은 그 통로에 둔다. 예외: multi 손작성 컨테이너 정의는 output/multi/에 추적(정본).
   - git 위생: 과거 루트-추적 산출물은 worktree 삭제만으론 부족 → 히스토리에서도 제거해야 롤백이 되살리지 않는다(엿본 정답/노이즈 방지).
     (.gitignore 규칙은 이미 올바름 — 재추가 말 것. 의존성 재생성 엔진 = 헌법 §스킬/도구 경계 참조)
   - 커밋·문서·서브전파 뒤 `hint_tag.py match`를 실행한다. 후보가 있으면 `policy:HINT_TAG_ACTIVATION_GATE`를 평가하고, 승인된 경우에만 create → judgment → finalize → verify 순서를 실행한다. Push/reverify 범위도 같은 policy와 script 결과를 따른다.
   verify: docs/devlog·testlog에 버전·변경·스모크 결과·last-good 기록
   ── HITL 게이트 ④ : 최종 커밋(+서브 전파) 승인
```

> 초기에는 위 4개 게이트를 모두 사람이 통과시킨다(최대 HITL).
> 단계가 안정화되면 하나씩 자동화 영역으로 이전한다(incremental trust).

## escalation 역루프 (서빙전략 수립 중 "현 vLLM 불가" 발견 → 버전 bump 요청)

> 헌법 §escalation 역루프 따름정리 · `plan_26063009_19_14`. 스킬 홈 = `vllm-recipe-explorer` §5.5(발견·핸드오프) ↔ `upstream-version-watch` §3.6(수신·3출구). 여기 = **절차-홈**(두 진입의 관계 · 분기 배치).

- **두 진입의 관계 (escalation ↔ arch-wall classify)**: 같은 "stock vLLM 구조적 불가"라도 **발견 시점·진입**이 다르다 —
  - **escalation**(본 절) = **서빙전략 수립 시(빌드 *前*)** `vllm-recipe-explorer`가 config + 외부 교차검증(HF 카드·GitHub)으로 *미리* 발견 → 승인 → upstream이 버전핀 처방(빌드 *하기 전에* 올바른 이미지를 정함) = **예방적**(외부 리서치로 빌드 낭비 회피).
  - **arch-wall classify**(S3 `classify_failure`의 `stock-구조적-불가`) = **빌드/스모크 실패 *後*** 사후 분류로 발견 → 동일한 vLLM 소스-repo 오버라이드(포크 SHA 핀) 처방 = **반응적**(실패 신호 기반).
  - ∴ **둘은 같은 처방(3출구)으로 수렴**하되 진입만 다르다. 처방 머신리(S1–S3 표준 bump · §4.6 포크핀 · 음성정직)는 **공유**.
- **분기 절차** (`policy` 참조는 헌법 "escalation 역루프 따름정리" — 여기는 순서만): 발견 조건(로컬 증상 ∧ 외부 확증) 성립 → 승인 → `upstream-version-watch` §3.6 수신 → **3출구**((i) 공식 bump→S1–S3 · (ii) 포크핀→§4.6/S3 arch-wall · (iii) 음성정직 보고) → 처방 출구가 (i)/(ii)면 S1–S3(render+build+스모크, HITL 게이트) → rebuild 이미지로 recipe 재개. 순환은 `reconciliation_cap` 한정 → 소진 시 Model-C(무한 bump ✗). 최종 중재=스모크(린트·이슈글 ≠ 서빙됨).
- **서브 인스턴스**: **egress-restricted** = 외부검색 불가 → 증상만 docs 상향 보고(D12 — `<model>_patch.py`/special-dep 탐지 상향과 동형), 메인이 외부검색·처방(`vllm-recipe-explorer` §5.5). **egress-online+A2A 위임** = 서브가 발견(리서치) 자율 수행(단 bump 실행은 여전히 메인+승인 — `plan_26070809_46_57`).

## 메인↔서브 양방향 브랜치싱크 (D12 절차 · 멀티노드 전용)

> 근거: `docs/plan/plan_26062411`(D12). "항상 참" 요약 = 루트 `CLAUDE.md` §"메인↔서브 양방향 싱크 / 서브개선 role".
> 트리거 = 사람의 싱크 지시(자동 폴링·cron·webhook 없음 — 헌법 트리거 정책 동일). 스크립트 = `.claude/skills/upstream-version-watch/scripts/{sync_to_sub.sh,fetch_sub_docs.sh}`.

### B0 멱등 self-bootstrap (서브 git 최초 1회, `policy:SUB_GIT_LOCAL_ONLY`)
- 서브 `.git` 부재 시: 초기 rsync 후 `sync_to_sub.sh` 가 서브에서 로컬 레포와 `single`·`multi` 브랜치, 초기 commit 을 스크립트로 구성한다.
- **첫 init 은 HITL**(dry-run 노출 → `--apply` 게이트). 이후 멱등(로컬 레포 있으면 skip).

### B1 하향(메인→서브) 4단 — 브랜치별, fail-closed (`policy:SUB_SYNC_DIRTY_FAIL_CLOSED`)
1. `sync_to_sub.sh`의 `policy:SUB_SYNC_DIRTY_FAIL_CLOSED` preflight를 실행한다. 비0종료면 배달을 중단하고, 서브의 ready-for-sync attestation을 받은 뒤 재시도한다.
2. **checkout**: 대상 토폴로지 브랜치(`single`|`multi`)로 서브 checkout.
3. **rsync(브랜치-타겟 콘텐츠)**: `render_sub_env.py --topology <t>` 산출 + 토폴로지 산출물(`output/<t>/`)을 서브로 배달.
   겹침 = **main-canonical(sub-yields)** — 메인 정본이 이긴다. 서브 `[improve]` history 는 git 에 잔존(덮어쓰되 history 보존).
4. **스크립트저작 sync 커밋**: 배달 후 `sync_to_sub.sh` 가 서브에서 커밋을 **스크립트로** 저작(에이전트 아님).
- dry-run 우선(무엇이 떨어지나·삭제 0 미리보기) → `--apply`.

### B2 상향(서브→메인) = 문서기반 회수 only
1. 서브가 자기개선 insight 를 **docs 규약**(`.claude/rules/docs.md`, `YYYYMMDDHH_seq`)으로 발행(서브 로컬 `[improve]` 커밋).
2. 서브가 A2A 리포트(`notes`/`artifacts`)로 **그 문서 경로**를 메인에 전달.
3. 메인이 **`fetch_sub_docs.sh`** 로 서브 `docs/` 만 로컬 gitignored 미러(`sync_staging/sub_docs/`)로 rsync — **에이전트 직접 SSH 재스캔 아님**.
4. 메인이 미러 문서를 **열람** → **HITL 재저작**: 메인 템플릿(`sub_node/*.template`)·헌법·스킬에 반영. **자동 머지 없음**(안전 > 자동화).
- patch/format-patch/git-bundle/staging/apply-check 추출층 **없음**(전부 문서기반으로 붕괴 — plan_26062411 D12-06·13).

### B3 경계 (A2A — CLAUDE.md 와 정합)
- 메인 관측 = push-attestation 리포트 + 미러 `docs/` 열람. **서브 작업코드/설정 재스캔·직접교정 금지.**
- 메인 수정권(env/헌법)은 **하향 파이프라인으로만** 행사(템플릿→렌더→배달). 서브 모델작업·triplet 은 서브 자율.

## 트리거 (수동)

- 재빌드/bump 트리거는 **사람의 "업데이트" 지시**뿐. 자동 폴링·cron·webhook 없음.
- 신규 버전 감지는 사람의 역할(모델 구동 실패 또는 GitHub 확인).
  - **예외**: 헌법 §버전 핀/트리거 정책 의 escalation 예외 조항을 따른다.

## 모델 / 안전 가드 (절대 규칙)

- 모델 weights 획득은 `policy:MODEL_ACQUISITION_TERNARY_GATE`를 먼저 평가한다. 모델 부재 시 manifest `model_source`를 validator에 전달하고, 반환된 handler/approval-request action만 실행한다. 승인 요청에는 validator가 반환한 estimate 또는 structured lookup error를 그대로 제시한다. Token은 manifest file pointer로 전달한다.
- Host-safety 설치·opt-out·가동 경계는 `policy:HOST_SAFETY_LAYERED_DEFENSE`가 소유한다. Workflow는 manifest 상태를 validator에 전달하고, 반환된 preflight/cleanup action만 순서대로 실행한다.
- HITL 게이트 이전 자동 핀 변경/자동 커밋·서브 전파 금지.
- 단계 건너뛴 부분 적용 상태로 빌드 금지(일관성).

## 실패 / 롤백

- 실패 시 last-good 커밋으로 복구한다(`policy:LAST_GOOD_ROLLBACK_ANCHOR`).

## 검증 모드 / 기록

- 컨테이너 변경은 S3 스모크 통과 전 done 금지(Karpathy B4).
- 빌드/검증 로그를 `docs/testlog/`에, 버전 전파 작업 내역을 `docs/devlog/`에 남긴다.

## 부트스트랩 5단계 (이 에이전트를 만드는 메타 절차 — 완료된 기록·참조용)

```text
Step 1 작업환경 셋업 (CLAUDE.md + .claude/rules)
Step 2 스킬 설계 (upstream-version-watch + 결정론적 scripts + config.yaml)
Step 3 단일노드 검증 (single-node 브랜치 dry-run + 스모크)
Step 4 멀티노드 확장 (서브노드 동기화)
Step 5 멀티노드 검증
```
- 각 Step: `docs/plan/`에 계획서 → 사람 검토 → 실행 → 기록 → 다음 Step. 단일(1–3) 검증 후 멀티(4–5).
