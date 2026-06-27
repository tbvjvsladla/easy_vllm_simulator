# CLAUDE.md — vLLM 업스트림 추적 · 컨테이너 버전관리 에이전트

> 이 워크스페이스의 Claude는 **임의 사용자 환경에서 NGC 기반 vLLM 컨테이너와 서빙전략을 생성하는
> 이식 가능한 코드 에이전트**다(업스트림 vLLM 릴리즈 추적 → 컨테이너 버전관리 포함).
> 사용자의 노드 구성·네트워크·경로 같은 환경 구체값은 `manifest.yaml`에서 읽는다(테라포밍 스킬이 생성).
> 이 파일은 세션마다 로드되는 "항상 들고 있어야 할 사실"만 담는다.
> 다단계 절차(전파 워크플로)는 `.claude/rules/workflow.md`에 있다.
> 근거: Ouroboros 인터뷰 → Seed (비추적 `seed/` 참조 — 배포본엔 부재 가능).

## 정체성

- **업스트림(진실의 원천)**: vLLM — https://github.com/vllm-project/vllm
  - "버전" = GitHub Release 태그. **pre-release도 추적 대상**.
- **관리 대상**: 커스텀 Docker 컨테이너 = NGC PyTorch 베이스 + vLLM(**prebuilt wheel 또는 소스빌드**) + 주변 의존성.
- **토폴로지**: 사용자 환경에 따라 단일노드 또는 분산(다노드)으로 결정된다. 노드 수·역할(main/sub)·인터커넥트는
  헌법에 박지 않고 `manifest.yaml`의 `topology`·`nodes[]`에서 읽는다(테라포밍이 자동탐지+인터뷰로 채움).
- **브랜치**: `single-node` = 단일노드 전용 · `multi-node` = 분산 전용. 두 브랜치는 버전 핀이 독립.

## 레이어드 적응 원칙 (기초레이어 우선 — 가장 먼저)

- **헌법(철학)=기초레이어=단일 진실원천.** 기초레이어 규정이 바뀌면 상위레이어(스킬·workflow·recipe·산출물)는
  그 변경에 **적응패치**해 정합을 회복한다(역방향 금지 — 상위 편의가 헌법을 흔들지 않음).
- 운영: 기초레이어 변경 시 **상위 전 레이어 conformance 스윕**(부분수정 방치 금지 — 정합 회복까지가 1건).
  근거: `docs/plan/plan_2026062711_1`(첫 적용 = 모델구동 패치 + KV 이식성).

## 핵심 사실 (항상 보유)

- **레이어 커플링 규칙(가장 중요)**: 대상 vLLM 버전의 `pyproject.toml` `[build-system].requires`에
  **명시된 torch 버전**을 먼저 확인하고, 그 torch 버전과 NGC 컨테이너의 `NVIDIA_PYTORCH_BUILD_VERSION`
  **접두어**가 일치하는 NGC PyTorch 베이스 태그를 선정한다.
  (접미어 `+해시`·빌드메타는 무시. 예: vLLM 0.21.0 → torch 2.11.0 → `nvcr.io/nvidia/pytorch:26.03-py3`)
- **빌드트랙 따름정리**: torch 2.10대 → prebuilt wheel · torch 2.11+ → NGC alpha와 prebuilt `_C`의 C++ ABI 충돌(하드 ABI 벽) → **소스빌드 1차 트랙**. 트랙 판정 = 스킬 `upstream-version-watch` §0.5, 최종 중재 = 스모크.
- **커플링 보강 원칙**: prefix-매칭은 필요조건일 뿐 — source-build에서 alpha 베이스가 stable-ABI 심볼 결여 시 **더 새 NGC 베이스 승격 정당**(전방호환). 절차·키잉 상세 = `.claude/rules/workflow.md` S3 · 스킬 §4.6.
- **이미지 네이밍 불변식**: `easy-vllm:{vllm}-cu{cuda}-{arch}-{track}`(예 `0.23.0-cu132-aarch64-source`). 모델-키잉 금지(과거 난립 원인) — 한 이미지가 모든 모델을 서빙. 태그 산정 = `render_dockerfile.py`.
- **산출물 통로 불변식 (single/multi 혼재 차단)**: 빌드/렌더 산출물(Dockerfile·compose·requirements·모델 configs·envs·**manifest 실값**)은 **`output/<topology>/`(single|multi)** 에 둔다. **통로 껍데기(`.gitkeep`)만 추적·생성물 비추적** → 단일/멀티 산출물이 켜켜이 쌓여도 경로 격리로 서로 침범 못 함. **topology 는 브랜치가 결정**(single-node=single, multi-node=multi) → manifest 실값도 `output/<topology>/manifest.yaml` 통로 분리, **브랜치 빈번 전환 시 재작성 0**(전환=그 통로 manifest를 읽음). 빌드 = `docker compose -f output/<topology>/docker-compose.yaml …`. 예외: multi 손작성 컨테이너 정의는 `*.template` 졸업 전까지 추적(정본 — Plan 2서 ignore 강등). 근거: `docs/plan/plan_2026062312_1`(통로)·`plan_2026062315_1`(manifest 이관).
- **serve-time env 통로 불변식 (결함#2 codify)**: serve 변수치환값(`NAS_MODEL_PATH`·`TIKTOKEN_HOST_PATH`) **해소 우선순위 = env-주입 > manifest 정본 > 리터럴 default**(포인터 원칙 연장 — `check_smoke_model.py`·render materialize 동일). materialize 절차(`render_dockerfile.py --materialize-env` → `output/<topology>/.env`) 상세 = 스킬 `upstream-version-watch` §2.5.
- **KV 절대클램프 따름정리 (이식성)**: 최종 recipe 의 KV 는 **측정된 GPU당 `kv-cache-memory-bytes`(절대값)** 로 제어한다 — 이게 **이식성**을 준다(gmu-derived KV 는 호스트 VRAM 차이로 비이식·OOM). **단 `gpu-memory-utilization` 도 함께 emit**(E2E 실증): vLLM 은 클램프 시 gmu 를 *KV 사이징*에만 무시(cache.py)하고 **startup free-memory 검증(free ≥ gmu×total)+총-cap 엔 여전히 사용** → 통합메모리(GB10 free/total≈0.91)는 기본 0.92 가 startup OOM 이라 **gmu ≤ 0.90 명시 필수**. ∴ gmu=startup/cap 게이트, clamp=KV·이식성. 시뮬레이터의 3종 산출물 = "**타겟 GPU 구동가능 환경의 시뮬레이션**". 이식성 = 선언된 절대 필요량 이상 GPU서 동일 구동(작은 GPU 자동맞춤 ✗, GPU당 값이라 TP 의존). 상세 = 스킬 `vllm-recipe-explorer` §5 · `plan_2026062711_1` Part 2.
- **모델구동 런타임 패치 따름정리**: 구동 불가 모델(포크-시대 VL processor 불일치 등)의 런타임 호환은 **stock 이미지 + 런타임 패치**로 해결한다(빌드타임 파생-이미지 금지 — 이미지 네이밍 불변식). 패치 내용 = **메인 저작 빌딩블럭**(서브 저작 ✗·하향 배달), **확률론·휘발**(에이전트가 참조-그라운디드로 환경·bump 마다 재유도 · 비추적 `<model>_patch.py` · 카탈로그 없음 · carry-forward ✗). **결정론 메커니즘 = arming**(serve_runner/단일진입의 `<model>_patch.py` 자동탐지 .pth → 모든 프로세스 적용; 메인 저작). 정본 = 방법론(스킬)+지식(docs). 단 source-build 패치는 *빌드타임*이라 추적 `Dockerfile.source-build`에 동결(평면별 지속성 비대칭). 상세 = 스킬 `vllm-recipe-explorer` §5 · `plan_2026062711_1` Part 1·3.
- **인코딩 자산 따름정리**: 런타임 인코딩 자산(tiktoken o200k/harmony)도 가중치와 함께 에어갭 사전적재 대상 — 상세 = 스킬 `vllm-recipe-explorer` §5.
- **near-max batch 측정 따름정리**: near-max batch·절대 KV 클램프는 **측정(serve KV log 또는 Phase-2)으로만** 산정한다 — per-token KV 공식은 과대추정 상한일 뿐(**측정 > 공식**, formula-우선 batch 금지). 숫자증거·상세 = 스킬 `vllm-recipe-explorer` §1·§5.
- **빌드 입력**: `CPU_ARCH=$(uname -m)` · `CUDA_VERSION`(예 129) · GitHub Releases pre-built wheel.
  wheel은 `pip install --no-deps`로 설치하고, **그 전에 `/etc/pip/constraint.txt`를 비운다**(NGC 핀 충돌 회피). 상세 = 스킬 `upstream-version-watch`.
- **주변 의존성 원천**: vLLM `requirements/{common,cuda,build}.txt` + `pyproject.toml` → `requirements.txt` 재생성(절차 = `.claude/rules/workflow.md` S1).
- **모델**: 폐쇄망 전제. 사람이 사전 다운로드해 NAS에 둔 모델을 read-only 마운트. 런타임 다운로드 없음.

## 버전 핀 / 트리거 정책

- **완전 수동**: 사람이 신규 vLLM을 감지(모델 구동 실패 또는 GitHub 확인) → "업데이트" 지시 → 에이전트 실행.
  자동 폴링·webhook·cron 없음. patch/minor/major 무관하게 항상 사람 지시로 시작.
- **config.yaml(영속)** 이 대상 버전 · 단일/멀티노드 스모크 모델명 · NAS 경로를 지정(스킬에서 사용, Step 2).

## 배포 / 환경 (manifest)

- **이식 모델**: 배포 단위 = 스켈레톤 + 생성엔진(완성품 아님). 누구든 클론 후 자기 환경을 테라포밍해 쓴다.
- **포인터 원칙**: 환경 구체값(노드 IP·호스트명·인터커넥트·NAS 경로·origin)은 **헌법에 두지 않고** `output/<topology>/manifest.yaml`(브랜치 파생 통로)에서 읽는다.
  추적되는 스켈레톤은 `manifest.template.yaml`(루트, 빈칸), 테라포밍이 채운 실값은 `output/<topology>/manifest.yaml`(비추적, 통로 분리 — 산출물 통로 불변식·plan_2026062315_1). NAS 기본값 = `/mnt/models`(manifest로 override).
  `CPU_ARCH`는 빌드타임 `$(uname -m)`(리터럴 baking 금지) · NAS 경로는 `${NAS_MODEL_PATH}` env(추적물에 PII 비박음).
- **2-브랜치 배포**: `single-node`(단일) · `multi-node`(분산) 모두 배포 대상. 공유 빌딩블럭은 `scripts/sync_branches.sh`로 동일하게 유지.

## 메인↔서브 양방향 싱크 / 서브개선 role (D12)

> 근거: `seed_e34dfbb6ec23` · `docs/plan/plan_2026062411_1`. 절차 상세 = `.claude/rules/workflow.md`(양방향 브랜치싱크).

- **메인의 지속적 서브개선 role (1급)**: 메인은 서브노드 **작업환경·헌법의 저작·수정권**을 보유한다 — 단 행사 방식은
  **템플릿→렌더→배달 파이프라인**(`render_sub_env.py`→`sync_to_sub.sh`)이지 **서브 디스크 재스캔이 아니다**.
  서브 *모델작업·triplet*은 서브 자율(런타임블럭) · 서브 *insight*는 문서로 회수 → 메인이 산출한 업데이트로 지속 개선(self-improving tooling).
- **A2A 경계(정밀)**: 메인 관측 = (a) push-attestation 리포트 + (b) 서브 `docs/` 로컬 미러 **열람**(`fetch_sub_docs.sh`).
  메인은 서브 **작업코드/설정을 재스캔·직접교정하지 않는다**. env/헌법 수정은 위 하향 파이프라인으로만(저작권위 ↔ 재스캔금지 공존).
- **서브 git = 로컬 전용**: 서브 워크스페이스는 `git init` 된 로컬 레포(`single`·`multi` 두 브랜치). **origin 영구 미설정**(push/pull/fetch/remote/clone deny — 방어심층, 진짜 구속은 "원격 없음" + 페르소나). git 역할 = 브랜치전환(모델로드 전략 분기) + 로컬 history/롤백 — **회수 vehicle 아님**.
- **하향(메인→서브)**: 브랜치별 rsync 배달 + 스크립트저작 `[sync]` 커밋(main-canonical, 겹침=sub-yields), dirty 트리=**fail-closed**(auto-stash 금지). 절차 = workflow.md B1.
- **상향(서브→메인) = 문서기반 only**: 서브 insight 를 `docs/` 규약으로 발행 → A2A 리포트로 경로 전달 → 메인 `fetch_sub_docs.sh` 미러 열람 → **HITL 재저작**(메인 템플릿/헌법/스킬). patch/추출층 없음. 절차 = workflow.md B2.
- **패치 전파(D12 연장)**: 모델구동 런타임 패치(`<model>_patch.py`)는 **메인 저작 → 하향 배달**(`sync_to_sub.sh` set; 슬레이브가 받는 *최초 model-keyed 파일*). 서브는 **패치 코드 저작 ✗** — "패치 필요" 탐지를 docs insight 로 상향 보고만(상향 코드/패치 추출층 없음 — D12-06·13). arming 관용구도 메인 배달분(서브 즉흥 저작 ✗).
- **PII 격리**: 회수가 문서기반(코드/설정 미추출)이라 서브 `CLAUDE.md`의 bake 정체성(PII)이 **메인 추적물로 유입되지 않는다** — `포인터 원칙`의 연장. 서브 헌법은 서브에 잔류.
- **single-node 확장기능**: single-node=기본 독립운용. sub-control("서브 제어 + 수행피드백 수신")은 single-node가 획득하는 **'확장기능'**(헌법 기재). **활성 게이트=결정론**: `output/single/manifest.yaml` `nodes[]`에 sub 존재 여부(`sync_to_sub.sh` 가 읽어 판정). 현재 single manifest 는 `nodes:[]` → **dormant**(독립 self-containment 보존). (HW탐지 결과를 single 통로로 채우는 전달 메커니즘은 **미구현·파킹** — plan_2026062411_1 §5.)
  - **라이브 형태 = A2A 모델서빙 위임(T3 검증, 0.23.0 E2E)**: 활성 시 메인이 서브에 A2A 태스크 발급(`ssh sub claude -p … --permission-mode acceptEdits`) → 서브가 자작 recipe + `--profile serve up -d` + 로컬 스모크 → push-attestation 1개 반환. 메인은 **리포트만 관측**(디스크 재스캔 X — A2A 경계). 실행평면 노드별 독립(교차검증). 절차 = 서브 `comms.md` serve 술어(single 분기).

## build / 검증 커맨드

- 산출물 통로: 빌드/서빙 산출물은 `output/<topology>/`(single|multi)에 위치 — 단일/멀티 혼재 차단. 통로 껍데기만 추적·생성물 비추적(plan_2026062312_1).
- 이미지 빌드(디버그): `docker compose -f output/<topology>/docker-compose.yaml --profile debug build`
- 서빙: `docker compose -f output/<topology>/docker-compose.yaml --env-file output/<topology>/envs/.env.<config> --profile <serve|master|slave> up`
- **스모크(합격 신호)**: `vllm serve /app/models/<모델디렉토리>` 후 **프롬프트 1회 → 비어있지 않은 완성 응답 1회**.
  단일노드·멀티노드 **양쪽** 스모크가 통과해야 bump 완료. (상세 절차: `.claude/rules/workflow.md`)

## 검증 게이트 (Goal-Driven — Karpathy B4)

- 컨테이너 변경은 위 스모크 통과 전 **done 선언 금지**.
- **smoke-before-commit**: 로컬 빌드+스모크 통과 코드만 last-good 커밋으로 남긴다.
  (origin은 사용자 환경 값(`manifest.origin_url`)에서 설정 — 없으면 로컬 전용. 멀티노드 서브 전파는 rsync `.claude/skills/upstream-version-watch/scripts/sync_to_sub.sh`.)
- 버전 bump는 사람 지시(핀 정책)와 단계별 HITL 게이트를 거친다.

## 계획 게이트 (체화 규율)

- container-gen · serving-strategy · branch-sync · terraforming_subnode 작업은 반드시 `docs/plan/` 문서를 **먼저 발행**하고
  **사람 검토(HITL)** 후 진행한다. 테라포밍된 환경에서도 이 문서 발행 규칙대로 작업하는 것이 **정본**이며
  루틴화한다(self-improving tooling). 문서 규약 상세: `.claude/rules/docs.md`.

## 금지 (안전 · 재현성)

- **무인(unattended) 자동 다운로드 금지가 기본** — 서빙대상 모델 부재 시 **사용자 승인 게이트** 후 (관리경로 존재 → 그 경로 영속 다운로드 / 부재 → 컨테이너 내부 HF cache 임시) 허용. 무인 자동 다운로드 절대 금지(절차: `.claude/rules/workflow.md` §모델/안전 가드).
- 외부/공식 스킬·플러그인·MCP를 사용자 검수 없이 **임의 설치 금지** (propose → review → install).
- **빌딩블럭**(`CLAUDE.md`·`.claude/`)은 이제 **추적·배포 대상**이다(이식 가능한 스켈레톤+생성엔진).
  단 사적/생성물(`.claude/settings.local.json`·`skills/*/config.yaml`·`manifest.yaml`·`envs/.env.*`·`seed/`·
  `sub_node/CLAUDE.md` 실값)은 **비추적**. 브랜치 간 공유 콘텐츠는 `scripts/sync_branches.sh`로
  동기화한다(수동 — 모든 작업 종료 후 사람 질의로 실행).
- 버전 문자열 해소(torch 핀·NGC 태그)를 **확률론적 추론으로 처리 금지** → **결정론적 스크립트**로(하네스 엔지니어링).
- **참조-그라운디드 해결**: 오류복구·진단 시 자기추론보다 **권위 참조**(업스트림 소스·이미지 내부·모델 config/chat_template·런타임 로그·레지스트리/헤더) 우선 — 위 결정론 해소의 error-recovery 연장. 상세는 각 스킬 error-recovery. (정확도 위해 토큰 증가 허용)
- 업스트림 핀/베이스 이미지를 가드레일·기록 없이 임의 변경 금지.
- 요청 범위 밖 기능·추상화 선반영 금지(Karpathy B2·B3).

## 롤백

- **last-good 앵커 = 로컬 스모크-통과 커밋**. 미커밋 작업분은 일회용 · `single-node`·`multi-node` 독립 롤백.
- 절차(`git reset --hard <last-good>`·`git tag last-good-<branch>`·origin = `manifest.origin_url`) 상세 = `.claude/rules/workflow.md` §실패/롤백.

## 스킬 / 도구 경계

- **커스텀(3-스킬 계층 · 빌딩블럭 vs 런타임블럭)**: `terraforming_subnode`(서브노드 진입 + 서브 에이전트 환경 구축 — `manifest.yaml` 생성·서브 페르소나/런타임블럭 렌더 배달; plan_2026062408_1) · `upstream-version-watch`(버전해소 + render + 소스빌드 + 빌드/스모크) · `vllm-recipe-explorer`(모델 yaml/sh/env + VRAM/KV trial + tiktoken 사전적재). **분류**: 빌딩블럭(`terraforming_subnode`·`upstream-version-watch`)=메인 전용(서브 전달 ✗) · 런타임블럭(`vllm-recipe-explorer`)=서브 복제(자기 모델에 자율 실행). 각 스킬 상세 = 해당 SKILL.md frontmatter. 결정론 vs 판단 분리는 각 스킬 내부.
- **외부**: Docker 작성/문법검사 보조 — 후보 `netresearch/docker-development-skill` (설치정책 거쳐 도입).
- **MCP**: 현재 없음. (멀티노드 서브노드 직접 SSH 제어 = 구현됨: `.claude/skills/upstream-version-watch/scripts/sync_to_sub.sh` = **브랜치-aware 하향 오케스트레이터**(rsync 배달 + 스크립트저작 `[sync]` 커밋 + fail-closed dirty 핸드셰이크 + 멱등 git-init) · `fetch_sub_docs.sh` = **상향 문서회수 미러** · 서브 빌드워커 CC `ssh sub bash -lc "claude -p"`. SKILL.md §4.5 / workflow.md S2.5·S3 · 양방향 싱크 D12절차.)
- 참고: 기존 `configs/check_reqs.py`(의존성 차이 분석 스크립트)를 결정론적 resolve 기반으로 재활용.

## 문서 발행 / 참조

- **3종 문서 역할**: `docs/plan/`(착수 전 계획·HITL) · `docs/devlog/`(작업 서사) · `docs/testlog/`(검증 증거·판정). 명명 = `docs/<type>/<type>_<YYYYMMDDHH>_<seq>_<주제>.md`. 역할·명명·구조·브랜치 통합모델 상세 = `.claude/rules/docs.md`.
- 전파 워크플로 규칙: `.claude/rules/workflow.md`.
- 부트스트랩 근거: 비추적 `seed/` 참조(GUIDE·question-catalog·REFERENCES·Seed) — 배포본엔 부재 가능.
