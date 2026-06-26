# workflow.md — 업스트림 추적 컨테이너 버전관리 전파 워크플로

> 이 파일은 다단계 절차 규칙이다. "항상 참인 사실"은 루트 `CLAUDE.md`에 있다.
> 사람이 "vLLM X로 업데이트" 지시를 내렸을 때 에이전트가 실행하는 절차를 정의한다.

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
   - multi-node 브랜치: 네트워크 디버그 apt · serve_runner.sh(Ray) · NCCL/RDMA env · /dev/infiniband은 보존(건드리지 않음)
   - multi-node: .gitignore에 빌딩블럭(CLAUDE.md/seed/) 제외 정렬. (빌딩블럭은 gitignored→브랜치 전환 persist, cross-branch 동기화 불필요)
   verify: 변경 라인이 S1 해소값에 직결(Karpathy B3)
   ── HITL 게이트 ② : 각 브랜치 diff를 사람이 검토

S2.5 sync   → (multi-node 전용) 메인 검증코드 → 서브 직접 전달
   - scripts/sync_to_sub.sh (기본 dry-run → --apply): rsync over SSH(인터커넥트는 manifest.interconnect), 체크섬 검증.
     서브 노드 접속값(host·ssh_user)은 manifest.yaml `nodes[]`에서 읽는다.
     제외: .git/.claude/seed/docs/__pycache__/CLAUDE.md (빌딩블럭·서브 빌드워커 페르소나 보호). GitHub 경유 X.
   - 서브는 메인 전달 코드로 생존. 서브 자작 envs/configs는 메인이 아카이브.

S3 smoke    → NAS 체크 + 로컬 빌드 + 실-서빙 스모크
   - ⑤ NAS 체크: check_smoke_model.py <config_name> --topology <single|multi> — 모델 부재면 중단·보고(다운로드 금지). --topology 필수(산출물 통로 output/<topology>/)
   - (단일노드) 빌드: docker compose --profile debug build · 서빙: --profile serve up → 프롬프트 1회 → 비어있지 않은 완성
   - near-max batch(요구 시): 서빙 docker logs 의 kv_cache_tokens/max_concurrency 로 실측 near-max 산출(Phase-1.5, 스킬 §5) → recipe max-num-seqs 보강. 공식 batch 금지(헌법 near-max 따름정리).
   - (multi-node) 2노드 Ray 서빙: scripts/multinode_serve_smoke.sh <config> [--build]
       NAS체크 → 양노드 병렬빌드 → master(메인)+slave(서브) Ray클러스터 → 엔드포인트 health 폴링 → master 엔드포인트 추론
       준비판정 = :PORT/health http200 (master 로그 "startup complete"는 거짓양성 — grep 금지)
       reasoning 모델은 max_tokens 충분히(finish_reason=stop)
   verify: 스모크 통과
   ── 실패 시 ⑥ classify_failure.py 로 분기:
        · requirements-fixable → Loop-Until-Done(조정→재빌드→스모크), reconciliation_cap(기본 3) 한정. 소진→Model-C
        · source-build-class(torch 2.11+) → Phase 2 소스빌드 경로(SKILL.md §4.6, 검증됨): 인터랙티브 컨테이너에서
            _C를 NGC torch에 맞춰 컴파일(ABI 벽 해소) → 경험적·HITL 패치 루프(strip-hoist 등) → 스모크 →
            Dockerfile.source-build 동결 → clean 재빌드 재현. 패치는 판단계층(사전-codify 금지). 단 E2E 검증 후 키잉된 조건부 카탈로그로 졸업 가능(스킬 §4.6).
            └ NGC 베이스 오버라이드 = 1급 Model-C 서브분기(베이스 torch의 ABI 결여 심볼로 source-build FAIL일 때):
              (source-build 맥락·헤더grep 기법의 스킬-홈 = 스킬 upstream-version-watch §4.6 ↔ S3 = 절차-홈, 상호참조)
                ① classify=unknown → 정지(자동 행동 금지).
                ② **참조-그라운디드 해결**: 후보 신규 NGC 베이스의 torch::stable 헤더(tensor_struct.h/ops.h)를 grep해 결여 심볼(예: layout()/6-arg from_blob)이
                   **그 후보 베이스엔 존재함**을 사전 확증(현 베이스엔 부재가 빌드로그로 증명된 상태) — 자기추론 전 권위참조(헤더·빌드로그), 토큰 증가는 정확도를 사므로 장려.
                ③ 증거를 testlog에 기록 + 사람 승인 후 resolved.json의 NGC 태그만 오버라이드
                   (특정 버전값은 manifest·resolved.json에서 — 헌법·workflow에 박지 않음. 26.05 등 하드코딩 금지).
                ④ re-render → clean 재빌드 → 스모크. **무증거 오버라이드 금지.**
                   (resolve_ngc_tag.py 단발 prefix-매칭은 유지; 오버라이드는 이 워크플로 HITL 레이어.)
        · multi-node 서빙 실패(OOM/NCCL-RDMA/Ray join timeout) → Model-C(HITL). 서브만 빌드 실패=환경 불일치→Model-C
        · unknown              → Model-C: LLM {proposed_class, evidence} 제시 → 사람 승인 전 무행동
   ── HITL 게이트 ③ : 스모크 결과(+분류·risk-memo)를 사람이 확인 (smoke-before-commit)

S4 commit   → 스모크 통과분만 로컬 last-good 커밋 + 서브 전파 + 기록
   (origin은 사용자 환경 값(manifest.origin_url)에서 설정 — 없으면 로컬 전용·push 단계 생략.)
   - single-node · multi-node 각 브랜치 로컬 커밋 = 기록/last-good 앵커(브랜치 핀 독립). 필요 시 태그(git tag last-good-<branch>).
   - 브랜치 간 공유 빌딩블럭 동기화: scripts/sync_branches.sh(수동, 작업 종료 후 사람 질의).
   - 서브노드 전파: S2.5의 scripts/sync_to_sub.sh로 메인→서브 직접 rsync(검증됨). GitHub 경유 안 함.
   - 산출물 통로(single/multi 혼재 차단): render 산출물(Dockerfile · docker-compose.yaml · configs/*.{yaml,sh} · envs/.env.* · requirements.txt)은
     **`output/<topology>/`(single|multi)** 에 둔다 — 통로 껍데기 `.gitkeep`만 추적·생성물 비추적(CLAUDE.md "산출물 통로 불변식" · plan_2026062312_1). 예외: multi 손작성 컨테이너 정의는 output/multi/에 추적(정본).
   - git 위생: 과거 루트-추적 산출물은 worktree 삭제만으론 부족 → git rm + 커밋으로 HEAD에서도 제거해야 reset --hard가 되살리지 않음(엿본 정답/노이즈 방지).
     configs/check_reqs.py는 엔진 = 유지. (.gitignore 규칙은 이미 올바름 — 재추가 말 것.)
   verify: docs/devlog·testlog에 버전·변경·스모크 결과·last-good 기록
   ── HITL 게이트 ④ : 최종 커밋(+서브 전파) 승인
```

> 초기에는 위 4개 게이트를 모두 사람이 통과시킨다(최대 HITL).
> 단계가 안정화되면 하나씩 자동화 영역으로 이전한다(incremental trust).

## 메인↔서브 양방향 브랜치싱크 (D12 절차 · 멀티노드 전용)

> 근거: `seed_e34dfbb6ec23` · `docs/plan/plan_2026062411_1`. "항상 참" 요약 = 루트 `CLAUDE.md` §"메인↔서브 양방향 싱크 / 서브개선 role".
> 트리거 = 사람의 싱크 지시(자동 폴링·cron·webhook 없음 — 헌법 트리거 정책 동일). 스크립트 = `.claude/skills/upstream-version-watch/scripts/{sync_to_sub.sh,fetch_sub_docs.sh}`.

### B0 멱등 self-bootstrap (서브 git 최초 1회)
- 서브 `.git` 부재 시: 초기 rsync 후 `sync_to_sub.sh` 가 서브에서 `git init` + `single`·`multi` 브랜치 생성 + 초기 commit.
- **첫 init 은 HITL**(dry-run 노출 → `--apply` 게이트). 이후 멱등(`.git` 있으면 skip). 서브 git = **로컬 전용·origin 영구 미설정**.

### B1 하향(메인→서브) 4단 — 브랜치별, fail-closed
1. **dirty 체크(fail-closed)**: 서브 `git status --porcelain` 이 비어있지 않으면 **배달 거부**(non-zero exit). **스크립트 auto-stash 금지.**
   → 서브가 스스로 commit/stash 로 clean 화 후 "ready-for-sync" 어테스트(핸드셰이크). 그 뒤 재시도.
2. **checkout**: 대상 토폴로지 브랜치(`single`|`multi`)로 서브 checkout(`--branch`).
3. **rsync(브랜치-타겟 콘텐츠)**: `render_sub_env.py --topology <t>` 산출 + 토폴로지 산출물(`output/<t>/`)을 서브로 배달.
   겹침 = **main-canonical(sub-yields)** — 메인 정본이 이긴다. 서브 `[improve]` history 는 git 에 잔존(덮어쓰되 history 보존).
4. **스크립트저작 `[sync]` 커밋**: 배달 후 `sync_to_sub.sh` 가 서브에서 `git add -A && git commit -m "[sync] …"` 를 **스크립트로** 저작(에이전트 아님).
- dry-run 우선(무엇이 떨어지나·삭제 0 미리보기) → `--apply`.

### B2 상향(서브→메인) = 문서기반 회수 only
1. 서브가 자기개선 insight 를 **docs 규약**(`.claude/rules/docs.md`, `YYYYMMDDHH_seq`)으로 발행(서브 로컬 `[improve]` 커밋).
2. 서브가 A2A 리포트(`notes`/`artifacts`)로 **그 문서 경로**를 메인에 전달.
3. 메인이 **`fetch_sub_docs.sh`** 로 서브 `docs/` 만 로컬 gitignored 미러(`sync_staging/sub_docs/`)로 rsync — **에이전트 직접 SSH 재스캔 아님**.
4. 메인이 미러 문서를 **열람** → **HITL 재저작**: 메인 템플릿(`sub_node/*.template`)·헌법·스킬에 반영. **자동 머지 없음**(안전 > 자동화).
- patch/format-patch/git-bundle/staging/apply-check 추출층 **없음**(전부 문서기반으로 붕괴 — plan_2026062411_1 D12-06·13).

### B3 경계 (A2A — CLAUDE.md 와 정합)
- 메인 관측 = push-attestation 리포트 + 미러 `docs/` 열람. **서브 작업코드/설정 재스캔·직접교정 금지.**
- 메인 수정권(env/헌법)은 **하향 파이프라인으로만** 행사(템플릿→렌더→배달). 서브 모델작업·triplet 은 서브 자율.

## 트리거 (수동)

- 재빌드/bump 트리거는 **사람의 "업데이트" 지시**뿐. 자동 폴링·cron·webhook 없음.
- 신규 버전 감지는 사람의 역할(모델 구동 실패 또는 GitHub 확인).

## 모델 / 안전 가드 (절대 규칙)

- 스모크 모델이 NAS 경로에 없으면 → **무인 자동 다운로드 금지**. 아래 결정트리로만 진행.
- **모델 확보 결정트리** (헌법 §금지 1줄정책의 절차):
  - **(0)** 서빙대상 모델 부재 → 다운로드 필요(0/1/2 분기).
  - **(1)** 명시적 다운로드·관리 경로 **존재** → **사용자 승인 시** 그 경로에 영속(관리) 다운로드.
  - **(2)** 관리 경로 **부재** → **사용자 승인 시** 컨테이너 내부 HF cache **임시(ephemeral)** 다운로드.
  - (1)·(2) 모두 **hf_token 필요** → manifest 파일 포인터로 등록(원시 토큰 비추적). **무인 자동 다운로드 절대 금지.**
- HITL 게이트 이전 자동 핀 변경/자동 커밋·서브 전파 금지.
- 단계 건너뛴 부분 적용 상태로 빌드 금지(일관성).

## 실패 / 롤백

- **last-good = 로컬 스모크-통과 커밋**. 미커밋분은 일회용.
- 실패 시: `git reset --hard <last-good-commit>` (잔여 로컬 상태 0). `single-node`·`multi-node` 독립 롤백. (origin은 사용자 환경 값(`manifest.origin_url`)에서 설정: `git remote add origin <manifest.origin_url>`.)
- 단일노드는 신버전 성공인데 멀티노드 실패 시 → 멀티노드만 롤백(브랜치 독립).

## 검증 모드 / 기록

- 컨테이너 변경은 S3 스모크 통과 전 done 금지(Karpathy B4).
- 빌드/검증 로그를 `docs/testlog/`에, 버전 전파 작업 내역을 `docs/devlog/`에 남긴다.

## 부트스트랩 5단계 (이 에이전트를 만드는 메타 절차 — 참조)

```text
Step 1 작업환경 셋업 (CLAUDE.md + .claude/rules)            ← 현재
Step 2 스킬 설계 (upstream-version-watch + 결정론적 scripts + config.yaml)
Step 3 단일노드 검증 (single-node 브랜치 dry-run + 스모크)
Step 4 멀티노드 확장 (서브노드 동기화)
Step 5 멀티노드 검증
```
- 각 Step: `docs/plan/`에 계획서 → 사람 검토 → 실행 → 기록 → 다음 Step. 단일(1–3) 검증 후 멀티(4–5).
