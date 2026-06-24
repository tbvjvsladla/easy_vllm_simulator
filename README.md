# easy-vllm

> **Upstream-tracking, version-managed vLLM container & serving-strategy generator — a portable skeleton + generation engine.**

`easy-vllm`(= `easy_vllm_simulator`)은 **완성품 컨테이너가 아니라 "스켈레톤 + 생성엔진"** 이다. 누구든 이 레포를 클론한 뒤 자기 환경을 **테라포밍**(manifest 채우기)하면, 업스트림 vLLM 릴리즈를 추적해 **NGC PyTorch 베이스 기반 커스텀 vLLM 컨테이너 + 모델별 서빙전략**을 자기 환경에서 **결정론적으로** 생성한다. 환경 구체값(노드·네트워크·NAS 경로)은 헌법에 박지 않고 **manifest 포인터**로만 읽으므로 이식 가능하다 — clone → terraform → "vLLM X로 업데이트" → resolve → render → build → 스모크.

---

## 무엇인가 / 철학

이 워크스페이스의 Claude는 **임의 사용자 환경에서 NGC 기반 vLLM 컨테이너와 서빙전략을 생성하는 이식 가능한 코드 에이전트**다. 사람이 신규 vLLM을 감지해 "업데이트"를 지시하면, 에이전트가 업스트림(진실의 원천)을 독해하고 컨테이너 레이어를 결정론적으로 해소·렌더·빌드·검증한다.

- **진실의 원천(업스트림)** = vLLM — https://github.com/vllm-project/vllm. "버전" = GitHub Release 태그이며 **pre-release도 추적 대상**이다.
- **관리 대상(산출물)** = 커스텀 Docker 컨테이너 = **NGC PyTorch 베이스 + vLLM(prebuilt wheel 또는 소스빌드) + 주변 의존성**.
- **배포 단위 = 스켈레톤 + 생성엔진(완성품 아님)**. 추적·배포되는 것은 헌법(`CLAUDE.md`)·규칙(`.claude/rules/`)·스킬(`.claude/skills/`)·템플릿·폴더 통로뿐이고, 실제 빌드 산출물은 각 환경에서 생성된다.
- **포인터 원칙**: 노드 IP·호스트명·인터커넥트·NAS 경로·origin 같은 환경 구체값은 **헌법에 두지 않고** `output/<topology>/manifest.yaml`(테라포밍이 생성)에서 읽는다. 추적되는 스켈레톤은 루트 `manifest.template.yaml`(빈칸)뿐 — **PII는 추적물에 baking되지 않는다**.
- **결정론 vs 판단 분리**: 버전 문자열 해소(torch 핀·NGC 태그·wheel URL)는 확률론적 추론이 아니라 **결정론적 스크립트**가 처리하고(하네스 엔지니어링), LLM은 risk-memo·후보 브레인스토밍·인터뷰 같은 판단계층만 담당한다. 결정론 해소의 산출물은 `resolved.json`(아래)이다.

---

## 핵심 불변식

| 불변식 / 따름정리 | 내용 |
| --- | --- |
| **레이어 커플링 규칙** (가장 중요) | 대상 vLLM의 `pyproject.toml` `[build-system].requires`에 **명시된 torch 버전**을 먼저 읽고, 그 torch 버전과 NGC 컨테이너 `NVIDIA_PYTORCH_BUILD_VERSION` **접두어**가 일치하는 NGC PyTorch 베이스 태그를 선정한다(접미어 `+해시`·빌드메타 무시). 예: vLLM 0.21.0 → torch 2.11.0 → `nvcr.io/nvidia/pytorch:26.03-py3`. |
| **빌드트랙 따름정리** | torch **2.10대 → prebuilt wheel** · torch **2.11+ → 소스빌드 1차 트랙**. 2.11+에서는 NGC alpha와 prebuilt `_C`의 C++ ABI 충돌(**하드 ABI 벽**)이 발생한다. 트랙 판정 = 결정론 제안, **최종 중재 = 스모크**. |
| **커플링 보강 원칙** | prefix-매칭은 필요조건일 뿐 — 소스빌드에서 alpha 베이스가 stable-ABI 심볼을 결여하면 **더 새 NGC 베이스 승격이 정당**(전방호환). 단 **무증거 오버라이드 금지**(아래 §전파 워크플로 Model-C). |
| **이미지 네이밍 불변식** | `easy-vllm:{vllm}-cu{cuda}-{arch}-{track}` (예 `0.23.0-cu132-aarch64-source`). **모델-키잉 금지**(과거 태그 난립 원인) — **한 이미지가 모든 모델을 서빙**. 태그는 `render_dockerfile.py`가 산정. |
| **산출물 통로 불변식** | 빌드/렌더 산출물(Dockerfile·compose·requirements·configs·envs·**materialize된 manifest 실값**)은 **`output/<topology>/`(single\|multi)** 에 둔다. **통로 껍데기(`.gitkeep`)만 추적·생성물 비추적** → single/multi 산출물이 켜켜이 쌓여도 경로 격리로 충돌 0. **topology는 브랜치가 결정**(single-node=single, multi-node=multi)이므로 브랜치 빈번 전환 시 재작성 0(전환 = 그 통로 manifest를 읽음). |
| **manifest 포인터 원칙** | 환경 실값은 `output/<topology>/manifest.yaml`(비추적, 테라포밍 산출)에서만 읽는다. 추적 스켈레톤 = 루트 `manifest.template.yaml`. NAS 기본값 `/mnt/models`(manifest override). `CPU_ARCH`는 빌드타임 `$(uname -m)`(리터럴 baking 금지). |
| **serve-time env 통로 불변식** | serve 변수치환값(`NAS_MODEL_PATH`·`TIKTOKEN_HOST_PATH`)은 `render_dockerfile.py --materialize-env`가 manifest에서 `output/<topology>/.env`로 **materialize**한다(렌더 표준 단계). compose 기본값(`${NAS_MODEL_PATH:-/mnt/models}`)에 의존하면 serve가 모델을 못 찾는다. 해소 우선순위 = **env-주입 > manifest 정본 > 리터럴 default**. |
| **폐쇄망 / 에어갭 불변식** | 폐쇄망 전제. 모델은 사람이 사전 다운로드해 NAS에 두고 **read-only 마운트**한다. **런타임 다운로드 없음.** |
| **결정론 산출물 = `resolved.json`** | 결정론 해소(torch핀·NGC태그·CUDA·wheel URL·`build_track.decision`·`torch_cuda_arch`)의 단일 진실원. `resolve_*` 스크립트가 쓰고 `render_dockerfile.py --resolved`가 소비하며, NGC 베이스 오버라이드도 여기 NGC 태그만 고친다. 비추적. (확률론적 핀 추론 **금지** — §무엇인가/철학.) |
| **requirements 천장(KNOWN_INCOMPAT)** | `regen_requirements.py`는 wheel `Requires-Dist`(권위 소스) 위에 **알려진 비호환 천장**(예 `fastapi<0.137.0`)을 적용해 업스트림 `>=` 시간드리프트 회귀를 차단하고, 적용분을 stdout으로 surface해 S1 재평가에 노출한다. 상류 수정 시 천장 제거. |
| **통합메모리 gmu 따름정리** | 통합메모리 호스트(GB10 등)에서는 `gpu-memory-utilization`을 반드시 명시 emit — 기본 0.92는 통합메모리에서 OOM. |
| **인코딩 자산 따름정리** | 모델 가중치뿐 아니라 런타임 인코딩 자산(tiktoken o200k/harmony)도 에어갭 사전적재 대상. |
| **near-max batch 측정 따름정리** | per-token KV 공식은 full-attention 가정 → sliding-window/GQA/hybrid 모델서 KV를 **과대추정하는 상한**일 뿐. near-max batch·절대 KV 클램프는 **측정으로만**(Phase-1.5 serve KV-log 또는 Phase-2 trial-loop) 산정. formula-우선 batch 금지. |
| **MoE 백엔드 따름정리** (sm_121a) | 대형 MoE를 신규 아키(GB10/Blackwell **sm_121a**)서 서빙 시 기본 `moe_backend=auto`는 `flashinfer_cutlass`를 골라 sm_121a용 prebuilt 부재 → 런타임 nvcc JIT가 OOM/단일커널 stall. **`--moe-backend triton`**(in-process, nvcc 불요) 명시로 우회. Ray 분산이면 master serve에만 줘도 slave 워커로 전파(검증: 멀티노드 0.23.0 E2E combo③). |

---

## 토폴로지 & 브랜치

토폴로지는 헌법에 박지 않고 사용자 환경(`manifest.yaml`의 `topology`·`nodes[]`)이 결정한다. 두 브랜치 모두 배포 대상이며 **버전 핀이 독립**이다.

| 브랜치 | 용도 | 산출물 통로 |
| --- | --- | --- |
| `single-node` | 단일노드(기본 독립운용) | `output/single/` |
| `multi-node` | 분산(다노드) 전용 | `output/multi/` |

- **single-node 확장기능 (sub-control)**: 단일노드의 기본은 **독립 self-containment**다. 단 single-node는 "서브 제어 + 수행피드백 수신"이라는 **확장기능**을 획득할 수 있다 — **활성 게이트는 결정론적**: `output/single/manifest.yaml`의 `nodes[]`에 `role:sub`가 있으면 활성, 없으면 **dormant**(`sync_to_sub.sh`가 읽어 판정). 활성 시 라이브 형태 = **A2A 모델서빙 위임**(메인이 `ssh sub claude -p`로 태스크 발급 → 서브가 자작 recipe + `--profile serve up -d` + 로컬 스모크 → push-attestation 1개 반환; 메인은 리포트만 관측). 0.23.0 듀얼모델 E2E에서 **T3 검증**(§검증 이력).
- **독립 핀 / 독립 롤백**: 단일노드는 신버전 성공인데 멀티노드가 실패하면 **멀티노드만 롤백**(`git reset --hard <last-good-commit>`).
- **공유 빌딩블럭 동기화**: 두 브랜치의 공유 빌딩블럭(`CLAUDE.md`·`.claude/`)은 `scripts/sync_branches.sh`로 동일하게 유지한다(수동 — 모든 작업 종료 후 사람 질의로 실행).
- **문서는 브랜치 통합**: 작업 문서(`docs/<type>/*.md`)는 gitignore되어 브랜치 전환에 persist → 단일/멀티 문서가 자동 통합·동일.

---

## 스킬 (생성엔진)

3개 커스텀 스킬이 생성엔진을 이룬다. **빌딩블럭**(메인 전용, 서브 전달 ✗)과 **런타임블럭**(서브 복제)으로 분류된다. 각 스킬은 영속 입력 `config.yaml`(아래 §사용법)을 진입 계약으로 읽는다.

| 스킬 | 분류 | 역할 |
| --- | --- | --- |
| **`terraforming_subnode`** | 빌딩블럭 (메인 전용) | 멀티노드 **서브노드 진입 + 서브 에이전트 환경 구축** 오케스트레이터. 5-전제조건 인터뷰 → 사용자 승인 → `scan_node.py` 결정론 스캔(cpu_arch/cuda/gpu/interconnect) → 성능게이트(`ib_write_bw`) → `manifest.yaml` 생성(스킬 간 단일 계약). 그리고 메인에서 서브 페르소나 `CLAUDE.md`·`Agent_Card.json`·스코프드 `settings.local.json`·런타임블럭·통신프로토콜을 렌더해 전달, **model-less 카나리**로 검증. 싱글노드엔 발동 안 함(서브 부재). |
| **`upstream-version-watch`** | 빌딩블럭 (메인 전용) | 버전 해소(`resolve_torch_pin` → `resolve_ngc_tag` → `resolve_build_track` → `resolve_wheel` → `regen_requirements`, 산출 = **`resolved.json`**) + render(`render_dockerfile.py` 시퀀스) + 소스빌드(Phase-2) + 빌드/스모크 + 실패분류(`classify_failure.py`). **제안만** 하고 실제 핀 변경·빌드·push는 HITL 게이트 전파 워크플로를 따른다. |
| **`vllm-recipe-explorer`** | **런타임블럭 (서브 복제)** | 폐쇄망 고정 모델 1개의 `config.json`을 결정론 파싱 → (quantization × max-model-len × gpu-memory-utilization) 3축 후보 → VRAM 추정·하드게이트·랭킹 → 3종 세트(.yaml+.sh+.env) 생성. **측정 3층**: Phase-1(공식 추정) → **Phase-1.5**(1회 `--profile serve up -d` → serve KV-log `kv_cache_tokens`/`max_concurrency` grep → near-max batch, 풀 루프 없이) → Phase-2(trial-loop로 **절대 KV 클램프 `--kv-cache-memory-bytes`** 수렴). + tiktoken 사전적재. 서브가 동일 결정론 엔진을 자기 모델에 자율 실행. |

> 결정론(스크립트)과 판단(LLM)의 분리는 각 스킬 **내부**에 둔다. 자율성 = 누가 실행하느냐, 방법 = 어디서나 결정론.

---

## 전파 워크플로 (S1–S4)

사람이 "vLLM X로 업데이트"를 지시하면(자동 폴링·webhook·cron 없음 — **완전 수동 트리거**) 에이전트는 4단계를 각각 verify와 **HITL 게이트**를 동반해 실행한다.

```text
S1 resolve  → 대상 vLLM 버전 결정론 해소 → resolved.json
   torch 핀(pyproject [build-system].requires) → NGC 베이스(접두어 매칭)
   → CUDA_VERSION · CPU_ARCH=$(uname -m) · wheel URL · requirements 재생성(+KNOWN_INCOMPAT 천장)
   verify: torch핀·NGC태그·CUDA·wheel URL·deps diff 출력
   ── HITL 게이트 ① : 해소 결과 사람 확인

S2 patch    → single-node · multi-node 두 브랜치 패치
   Dockerfile ARG(VLLM_VERSION/CUDA_VERSION/FROM/manylinux) · requirements.txt
   render 시퀀스: template + regen_requirements + (multi)materialize-configs + materialize-env
   verify: 변경 라인이 S1 해소값(resolved.json)에 직결
   ── HITL 게이트 ② : 각 브랜치 diff 사람 검토

S2.5 sync   → (multi 전용) 메인 검증코드 → 서브 직접 rsync-over-SSH
   sync_to_sub.sh (dry-run → --apply, 체크섬). GitHub 경유 X.

S3 smoke    → NAS 체크 + 로컬 빌드 + 실-서빙 스모크
   check_smoke_model.py(부재면 중단·보고, 다운로드 X)
   (single) docker compose --profile debug build → --profile serve up → 프롬프트 1회 → 비어있지 않은 완성
   (multi)  multinode_serve_smoke.sh <config> [--build]  (준비판정 = master :PORT/health http200)
   실패 시 classify_failure.py 로 분기(아래)
   ── HITL 게이트 ③ : 스모크 결과(+분류·risk-memo) 사람 확인

S4 commit   → 스모크 통과분만 로컬 last-good 커밋 + 서브 전파 + 기록
   브랜치별 독립 핀 커밋(필요 시 git tag last-good-<branch>)
   ── HITL 게이트 ④ : 최종 커밋(+서브 전파) 승인
```

**S3 실패 분기 (`classify_failure.py`)**

- `requirements-fixable` → Loop-Until-Done(조정→재빌드→스모크, `reconciliation_cap` 기본 3). 소진 → Model-C.
- `source-build-class`(torch 2.11+) → Phase-2 소스빌드(인터랙티브 컨테이너서 `_C`를 NGC torch에 맞춰 컴파일 → ABI 벽 해소 → HITL 패치 루프 → 스모크 → `Dockerfile.source-build` 동결 → clean 재빌드 재현).
- **NGC 베이스 오버라이드** = 1급 Model-C 서브분기: 후보 신규 베이스의 `torch::stable` 헤더를 grep해 **결여 심볼이 그 후보엔 존재함**을 사전 확증 + testlog 기록 + 사람 승인 후 `resolved.json`의 NGC 태그만 오버라이드. **무증거 오버라이드 금지**(하드코딩 버전 금지).
- `unknown` / multi-node 서빙 실패(OOM/NCCL-RDMA/Ray join timeout) → Model-C: LLM이 `{proposed_class, evidence}` 제시 → 사람 승인 전 무행동.

> **계획 게이트(체화 규율)**: container-gen · serving-strategy · branch-sync · terraforming_subnode 작업은 반드시 `docs/plan/` 문서를 **먼저 발행**하고 사람 검토(HITL) 후 진행한다. 테라포밍된 환경에서도 이 규칙대로가 정본.

---

## 멀티노드 / A2A

멀티노드는 **Ray TP=2 진성 분산 서빙**이다 — master(메인) = Ray head + serve, slave(서브) = Ray worker. 한 모델을 두 노드가 NCCL/RoCE GDR 인터노드 텐서 통신으로 함께 서빙한다.

- **드라이버**: `multinode_serve_smoke.sh <config> [--build] [--keep-up]`. 준비판정 = master `:PORT/health`(PORT=`SERVING_PORT`) **http200 폴링**(로그 "startup complete"는 거짓양성 → grep 금지). 폴링 한도는 `READY_MAX` env로 조정(대형모델 CIFS 로드 대비). model-less 통신 검증은 `multinode_comms_smoke.sh`(torch.distributed NCCL all-reduce, socket-fallback = FAIL). 양방향 연결대기(master는 `ray status` 2-GPU 등록, slave는 head 포트 `nc -z`)가 랑데부를 fail-closed로 처리.

### A2A 경계 (Agent2Agent, 서버 없음)

서버를 띄우지 않고 SSH 단발로 메인↔서브가 협력한다 — 전송 = `ssh <user>@<sub> claude -p '<Task>' --output-format json --permission-mode acceptEdits`.

- **역할**: 메인 = 클라이언트(Task 발급·리포트 검증·피드백) · 서브 = 원격 에이전트(Task 자율 수행 → **self-verified 리포트 1개** 반환). `acceptEdits`가 정본(`bypassPermissions`는 하네스 가드레일로 차단).
- **검증 = push-attestation**: 서브가 자체검증(config-parse·schema·`bash -n`·체크섬·로컬 스모크)해 `self_verification`에 담는다. **메인은 리포트만 검증 — 서브 디스크 재스캔·직접교정 금지.** "lint passed ≠ served".
- **(b) 아티팩트모델**: 서브는 **개발산출물만 전파받고 실행결과물(빌드 이미지 + slave 컨테이너)을 자체 생산**. 결합서빙의 신뢰성·랑데부를 위해 검증된 `multinode_serve_smoke.sh`(ssh-bash 오케스트레이션)를 쓰되, 이는 서브의 자기생산이지 디스크 재스캔이 아니다(A2A 경계 유지).
- **경계(B3 정밀)**: 서브는 모델별 `configs/`·`envs/`만 저작. 컨테이너 정본(Dockerfile/requirements/compose/serve_runner)·빌딩블럭(`.claude/`·`CLAUDE.md`·`Agent_Card.json`)은 off-limits.

### D12 — 메인↔서브 양방향 싱크

- **서브 git = 로컬 전용, origin 영구 미설정**: 서브는 `git init`된 로컬 레포(`single`·`multi` 두 브랜치). push/pull/fetch/remote/clone deny(방어심층). git 역할 = 브랜치전환 + 로컬 history/롤백(회수 vehicle 아님).
- **하향(메인→서브) 4단, fail-closed** (`sync_to_sub.sh`): ① dirty 체크 — 서브 `git status --porcelain` 비어있지 않으면 **배달 거부**(스크립트 auto-stash 금지, 서브가 commit/stash로 clean화 후 ready 어테스트) → ② 토폴로지 브랜치 checkout → ③ rsync(겹침 = main-canonical, sub-yields, 서브 `[improve]` history 보존) → ④ **스크립트저작 `[sync]` 커밋**. dry-run 우선 → `--apply`.
- **상향(서브→메인) = 문서기반 회수 only** (`fetch_sub_docs.sh`): 서브가 자기개선 insight를 자기 `docs/`에 발행 → A2A 리포트로 경로 전달 → 메인이 서브 `docs/`만 로컬 gitignored 미러(`sync_staging/sub_docs/`)로 rsync → **열람** → **HITL 재저작**. patch/bundle/staging 추출층 없음 · 자동 머지 없음.
- **PII 격리**: 회수가 문서기반(코드/설정 미추출)이라 서브 `CLAUDE.md`의 bake 정체성(PII)이 메인 추적물로 유입되지 않는다(포인터 원칙의 연장).

---

## 레포 구조

추적되는 것은 **스켈레톤 + 생성엔진**뿐이다. 실제 빌드 산출물은 `output/<topology>/` 통로에서 생성·비추적된다.

```text
easy_vllm_simulator/
├── CLAUDE.md                       # 헌법 (항상 보유할 사실)
├── README.md                       # 이 문서
├── manifest.template.yaml          # 추적되는 환경 스켈레톤(빈칸). 실값은 output/<t>/manifest.yaml(비추적)
├── .gitignore / .gitattributes / .dockerignore
│
├── .claude/
│   ├── rules/
│   │   ├── workflow.md             # 전파 4단계(S1–S4) + D12 양방향 싱크 절차
│   │   └── docs.md                 # 문서 4종 규약
│   └── skills/
│       ├── terraforming_subnode/   # [빌딩블럭] SKILL.md + scripts(scan_node, render_sub_env)
│       │   └── sub_node/           #   서브 에이전트 템플릿(PII-free): CLAUDE.template.md, Agent_Card.template.json,
│       │                           #   settings.local.template.json, comms.md, task-report.schema.json, gitignore.template
│       ├── upstream-version-watch/ # [빌딩블럭] SKILL.md + config.example.yaml (영속 입력 스켈레톤)
│       │   └── scripts/            #   resolve_torch_pin / resolve_ngc_tag / resolve_build_track / resolve_wheel
│       │                           #   regen_requirements / render_dockerfile / check_smoke_model / classify_failure
│       │                           #   multinode_serve_smoke.sh / multinode_comms_smoke.sh / sync_to_sub.sh / fetch_sub_docs.sh
│       └── vllm-recipe-explorer/   # [런타임블럭, 서브 복제] recipe.py + scripts(estimate_vram, parse_model_config,
│                                   #   gen_recipe_set, run_trial, parse_vllm_log, rank_recipes, simlog_writer …) + config.example.yaml
│
├── configs/                        # serve_runner.sh(Ray master/slave) + debug-init.sh 만 추적
│                                   #   (모델별 configs/*.{yaml,sh} 는 output/<t>/configs/ 로 — 루트는 gitignore)
├── envs/                           # .env.example 만 추적 (실 .env.* 는 output/<t>/envs/ 로 — 비추적)
├── scripts/
│   ├── sync_branches.sh            # 브랜치 간 빌딩블럭 수동 동기화
│   └── smoke_clone.sh              # 배포가능성 read-only 게이트
│
├── docs/                           # 스켈레톤 = 폴더 + 폴더당 example.md 1개만 추적
│   └── plan/ devlog/ testlog/ simlog/   #   각 example.md 추적 · 실 작업문서(docs/*/*)는 gitignore(브랜치 persist)
│
└── output/                         # 산출물 통로 — 통로 껍데기만 추적, 생성물 비추적
    ├── single/.gitkeep             #   (single 산출물 전부 gitignored: Dockerfile·compose·requirements·configs·envs·manifest)
    └── multi/
        ├── .gitkeep
        ├── Dockerfile              #   multi 손작성 컨테이너 정의 = *.template 졸업 전까지 추적(정본)
        ├── Dockerfile.source-build
        └── docker-compose.yaml
```

> **비추적**(생성물/사적): `output/<t>/{Dockerfile.source-build,docker-compose.yaml,requirements.txt,manifest.yaml,.env,configs/*,envs/.env.*,sub_provision/**}`(single 전부 · multi 일부), 루트 `requirements.txt`·`resolved.json`·`tiktoken_cache/`·`seed/`·`sync_staging/`, `.claude/settings.local.json`·`skills/*/config.yaml`·`sub_node/CLAUDE.md` 실값.
> render 템플릿(`Dockerfile[.source-build].template`·`docker-compose.template.yaml`·requirements regen)은 `upstream-version-watch`의 `render_dockerfile.py` 렌더 계약이 소유·소비한다(위 추적 트리의 스켈레톤이 아니라 스킬 평면).

---

## 사용법

### 0. 클론 → 테라포밍

```bash
git clone <repo> && cd easy_vllm_simulator
# 멀티노드면: terraforming_subnode 스킬 발동(5-전제조건 인터뷰 → 승인 → scan → manifest 생성)
#   → output/<topology>/manifest.yaml 채움 (노드·인터커넥트·NAS 경로·origin)
# 싱글노드면: output/single/manifest.yaml 채움.
#   서브는 manifest nodes[] 가 비면 dormant(기본 독립운용);
#   sub-control 확장을 켜려면 nodes[] 에 role:sub 기입(sync_to_sub.sh 가 읽어 결정론 게이트)
```

### 0.5. 입력 계약 — `config.yaml` (영속)

각 스킬의 **실제 사람 진입점**은 영속 `config.yaml`이다(스킬별 `config.example.yaml`에서 저작, 비추적). "vLLM X로 업데이트"를 지시하기 **전에** 이걸 채운다.

```yaml
# upstream-version-watch/config.yaml (예시 키)
target_vllm_version: "0.23.0"
ngc_probe_start: "26.05"                 # NGC 태그 프로빙 시작점(newest→oldest)
smoke:
  single_node: { config_name: "gemma-4-12b-it-dgxspark" }
  multi_node:  { config_name: "gpt-oss-120b-source" }
nas_model_path: "/mnt/llm/Model/hugging_face_ver_model"

# vllm-recipe-explorer/config.yaml (예시 키)
target_model: { path: "/app/models/OpenAI/gpt-oss-120b" }
vram_budget_gb: 121.69
safety_margin: 0.08
```

### 1. 버전 지시 → 결정론 해소 → 렌더

```bash
# 사람이 신규 vLLM 감지(모델 구동 실패 또는 GitHub 확인) → "vLLM X로 업데이트" 지시
# upstream-version-watch 스킬:
#   resolve(torch핀 → NGC 베이스 접두어매칭 → CUDA/arch/wheel URL → requirements 재생성) → resolved.json
#   ── HITL 게이트 ① 해소값 확인 ──
#   render_dockerfile.py 시퀀스: template(--resolved resolved.json) + regen_requirements
#     + (multi) --materialize-configs + --materialize-env  → output/<topology>/.env
#   ── HITL 게이트 ② 브랜치 diff 검토 ──
```

### 2. 빌드 → 스모크 (단일노드)

```bash
# NAS 체크 (부재면 중단·보고, 다운로드 금지)
python .claude/skills/upstream-version-watch/scripts/check_smoke_model.py <config> --topology single

# 빌드 (디버그)
docker compose -f output/single/docker-compose.yaml --profile debug build

# 서빙 + 스모크: vllm serve /app/models/<모델디렉토리> → 프롬프트 1회 → 비어있지 않은 완성 1회
docker compose -f output/single/docker-compose.yaml \
  --env-file output/single/envs/.env.<config> --profile serve up
```

### 3. 빌드 → 스모크 (멀티노드, Ray 2노드 분산)

```bash
# master(메인) + slave(서브) Ray 클러스터 build → up → master :PORT/health http200 폴링 → 엔드포인트 추론
bash .claude/skills/upstream-version-watch/scripts/multinode_serve_smoke.sh <config> --build
# 대형모델(긴 CIFS 로드)이면 폴링 연장: READY_MAX=360 bash …
```

**합격 신호(스모크)** = `vllm serve /app/models/<모델디렉토리>` 후 **프롬프트 1회 → 비어있지 않은 완성 응답 1회**. 단일노드·멀티노드 **양쪽** 스모크가 통과해야 bump 완료. 통과분만 last-good 커밋으로 남긴다(smoke-before-commit).

> 모델별 서빙전략(.yaml/.sh/.env)이 필요하면 `vllm-recipe-explorer` 스킬로 VRAM 예산에 맞춘 레시피를 생성·검증한다: **Phase-1 추정 → Phase-1.5(1회 serve로 KV-log 측정 → near-max batch) → Phase-2(trial-loop로 절대 KV 클램프 수렴)**.

---

## 검증 이력 (validated)

> 하드웨어: 2× **NVIDIA DGX Spark**(GB10 superchip, **aarch64**, **sm_121a**, 128GB 통합메모리/노드, **CUDA 13.2**). 폐쇄망 NAS(CIFS, 42T) read-only 사전적재. 인터커넥트 **RoCE v2 / NCCL GPU Direct RDMA(DMABUF)**. aarch64/cu13x prebuilt wheel 부재 → torch 2.11+ **소스빌드 트랙 강제**(ABI 벽).

### 단일노드 듀얼모델 E2E (vLLM 0.23.0 source-build) — **VERDICT: PASS**

- vLLM **0.23.0** → torch **2.11.0** → 소스빌드. NGC 베이스는 접두어매칭으로 `26.03`이나 **증거기반 오버라이드 → `nvcr.io/nvidia/pytorch:26.05-py3`**(26.03이 stable-ABI 심볼 `layout()`/6-arg `from_blob` 결여로 빌드 FAIL → workflow S3 Model-C 오버라이드). `TORCH_CUDA_ARCH=12.1a`. 이미지 `easy-vllm:0.23.0-cu132-aarch64-source`.
- **교차검증 빌드**: 양 노드 독립 clean-build → **byte-equivalent** vLLM `0.23.1.dev0+g0fc695fc6`, 런타임 `import vllm._C` OK(ABI 벽 해소) 양쪽.
- **메인 = gemma-4-12B-it** (`--profile serve up`): health 200 → 비어있지 않은 완성, `finish_reason=stop`, **PASS**. max-model-len 32768, **max-num-seqs 52(측정 near-max)**.
- **서브 = gpt-oss-20b** (A2A 자율, **T3 검증**): 메인이 `ssh sub claude -p … acceptEdits`로 태스크 발급 → 서브가 자작 recipe + 서빙 + 스모크 + tiktoken o200k 자가복구 → push-attestation 1개 반환, 메인은 리포트만 관측. **PASS**. max-num-seqs **86(측정)**.
- **measurement > formula 재강화**: gemma per-token KV 측정 vs 공식 · gpt-oss(GQA, full-attention 아님) 공식 1.9× 과대. vLLM `max_concurrency` 로그가 near-max batch 진실의 원천.

### 멀티노드 진성 분산 서빙 E2E (3 조합) — **VERDICT: 3/3 PASS**

> 드라이버 `multinode_serve_smoke.sh <config> --build --keep-up`. 합격 = 양노드 독립빌드 → master `:PORT/health` 200 → 엔드포인트 실추론 비어있지않음(`finish_reason=stop`) + **2노드 Ray TP=2 NCCL 확인**.

| 조합 | vLLM / 트랙 × 모델 | 결과 | 핵심 |
| --- | --- | --- | --- |
| ① | **0.18.0 / wheel** × gpt-oss-120b(MXFP4) | ✅ PASS | NGC 26.01, torch 2.10.0, ray 2.48.0. NCCL 2.29.2+cu13.1, NET/IB GDR(DMABUF). near-max **176**(concurrency 178.74×). |
| ② | **0.23.0 / source** × gpt-oss-120b(MXFP4) | ✅ PASS | NGC 26.05, torch 2.11.0. 단일노드 source core byte-identical → layer-cache 재사용. NCCL 2.30.4+cu13.2. near-max **150**(152.71×). |
| ③ | **0.23.0 / source** × Qwen3-Next-80B-A3B(bf16, 512-expert MoE, 151GB) | ✅ PASS (att4) | 최난도. sm_121a에서 FlashInfer CUTLASS MoE 커널 JIT가 OOM/30분 stall → **`--moe-backend triton`**(in-process MoE, nvcc JIT 0)로 우회 후 PASS. READY ~865s, near-max **6**(bf16 가중치 74GiB/노드 지배 → 작은 KV, 예상대로). |

- **멀티노드 codified fixes**: `--moe-backend triton`(sm_121a MoE 따름정리) · `READY_MAX` env(대형모델 health-poll 연장) · serve_runner `--object-store-memory` CLI 플래그(Ray가 env var 무시) · 통합메모리 bf16 near-max는 공식 아닌 측정.

---

## 문서 규약

작업 산출 문서는 `docs/<type>/<type>_<YYYYMMDDHH>_<seq>_<주제>.md` 명명을 따른다(절대일시 시각까지, seq 1부터, 한국어 밑줄 주제). 추적·배포되는 것은 **폴더 스켈레톤 + 폴더당 `example.md` 1개**뿐이고, 실제 작업문서는 gitignore되어 브랜치 전환에 persist·자동 통합된다.

| type | 역할 | 시점 |
| --- | --- | --- |
| `plan/` | 계획서 — 단계/Phase 작업의 설계·접근. **HITL 검토 대상** | 착수 **전** |
| `devlog/` | 작업 로그 — 수행 내역·결정·전파의 **서사** | 작업 중·후 |
| `testlog/` | 검증 로그 — 빌드/스모크/실험의 **증거와 판정** | 검증 결과 |
| `simlog/` | 시뮬레이션 증거 vault — `recipe.py simulate` **한 run = 디렉토리 1개**(파일 아님) | trial run |

> 참조 체인: `simlog`(원시 증거) → `testlog`(인용·종합·판정) → `devlog`(서사). 상세 규약 = `.claude/rules/docs.md`.

---

## 안전 / 가드

- **무인 자동 다운로드 절대 금지** — 서빙대상 모델 부재 시 **사용자 승인 게이트** 후에만: (관리 경로 존재 → 그 경로 영속 다운로드) / (부재 → 컨테이너 내부 HF cache 임시). `hf_token`은 manifest 파일 포인터로 등록(원시 토큰 비추적).
- **임의 설치 금지** — 외부/공식 스킬·플러그인·MCP는 사용자 검수 없이 설치 금지(propose → review → install).
- **smoke-before-commit** — 컨테이너 변경은 스모크 통과(testlog 증거) 전 **done 선언 금지**. 로컬 빌드+스모크 통과 코드만 last-good 커밋으로 남긴다. last-good 앵커 = 로컬 스모크-통과 커밋, 미커밋분은 일회용(`git reset --hard <last-good-commit>`로 복귀, 브랜치 독립).
- **계획 게이트** — container-gen·serving-strategy·branch-sync·terraforming_subnode 작업은 `docs/plan/` 문서를 먼저 발행하고 사람 검토(HITL) 후 진행.
- **결정론 강제** — 버전 문자열 해소(torch 핀·NGC 태그)를 확률론적 추론으로 처리 금지. 업스트림 핀/베이스 이미지를 가드레일·기록 없이 임의 변경 금지. 요청 범위 밖 기능·추상화 선반영 금지.

---

> 근거 헌법: [`CLAUDE.md`](./CLAUDE.md) · 전파 절차: [`.claude/rules/workflow.md`](./.claude/rules/workflow.md) · 문서 규약: [`.claude/rules/docs.md`](./.claude/rules/docs.md)
