# 멀티노드 빌드·전달·2노드 Ray 스모크 (조건부 reference · 검증됨 testlog_260607_7)

> spine step 5(sync)·step 6(smoke)의 **멀티노드 분기 상세**. `multi-node` 브랜치 = 2노드 분산 서빙
> (master = Ray head+serve / sub = Ray worker, 실제 주소는 `manifest.yaml` `nodes[]`).
> 핵심: **메인에서 코드 완성 → 서브로 직접 전달·동기화 → 양 노드 빌드 → 2노드 Ray 서빙**.

## 절차 (검증된 순서)

1. **resolve**: 단일노드와 동일(①~④ 스크립트 — `resolve-and-render.md`). 버전·torch핀·NGC태그·wheel·deps.
2. **patch (multi-node 브랜치)**: Dockerfile **버전 라인만** 갱신(FROM 태그·VLLM_VERSION·CUDA·manylinux).
   **보존**: 네트워크 디버그 apt(iproute2/netcat)·`configs/serve_runner.sh`(Ray master/slave)·NCCL/RDMA env·/dev/infiniband.
   requirements.txt 갱신. `.gitignore` 에 빌딩블럭(CLAUDE.md/seed/) 제외 정렬.
3. **sync to sub**: `scripts/sync_to_sub.sh`(기본 dry-run → `--apply`). 검증된 rsync, 체크섬 검증,
   `.git/.claude/seed/docs/__pycache__/CLAUDE.md` 제외(빌딩블럭·서브 페르소나 보호).
4. **multi-smoke**: `scripts/multinode_serve_smoke.sh <config_name> [--build] [--keep-up]`.
   NAS체크 → (양 노드 병렬 빌드) → master+slave 기동(Ray 클러스터) → **엔드포인트 health 폴링** → master 엔드포인트 추론 → 정리.
   **변종이미지 시 build-plane ≠ serve-plane**: 이미지 정체(`IMAGE_TAG`·`BUILD_DOCKERFILE`·compose `build.args` 가 참조하는 변수)는
   클러스터-wide 라 슬레이브 compose 보간에도 forward(슬레이브가 변종 이미지를 직접 빌드+기동) · 모델 serve config(`CONFIG_FILE`·
   트리플렛)는 **Band3** 라 슬레이브 미forward. ∴ 슬레이브 Band2-only = **serve-plane 불변식**(build-plane 아님).
   헌법 "변종이미지 build-plane ≠ serve-plane 따름정리" · `.claude/rules/workflow.md` S2.5.
5. **슬레이브 전달 집합 = `scripts/slave_forward.py`**(2026-09-21 · plan_26092119 §4.6 · 단일 소유):
   스모크는 `IMG=`·`SLAVE_IMGVARS=` 를 그대로 대입하되 **값은 slave_forward 가 파생**한다 — 스모크가 실제로 쓰는
   `output/multi/docker-compose.yaml` 의 `${VAR}` 참조에서 그룹을 가른다:
   `image_identity`(image·dockerfile·build.args 참조 − BUILD_JOBS) · `build_tuning`(BUILD_JOBS) · `mount`(volumes · 프로젝트 `.env` 우선) ·
   `cluster`(slave container_name + Band2 렌더러 `render_dockerfile.env_tier` 가 소유한 slave env 키 — 셀이 덮어쓸 때만) ·
   `serve_env`(PLE mmap 등 나머지 slave env). `CONFIG_FILE` 은 어느 그룹에도 없다.
   의미: 첫 일치 · **빈 값은 키째 생략**(부재 = stock) · 인용 없는 원격 prefix 에 안전하지 않은 값은 **fail-closed**.
   왜: 손목록이 build-arg 와 따로 자라며 다섯 번 침묵 누락됐다(BUILD_DOCKERFILE 07-24 · VLLM_PRETEND_VERSION 08-02 ·
   SM12X_PORT·RAY_PORT 08-14 · SRC_DEPS_AUTHORITY 08-15 · VLLM_VERSION 09-05 · PLE 09-09 — 날짜 박힌 이력은 `slave_forward.WHY`).
   새 build-arg 를 compose 에 더하면 **코드 수정 없이** 전달된다. hint `sub_recipe.json` 도 **같은 함수**를 부른다(`as_sub_recipe` —
   원값 대신 형상). 자체검사: `python3 scripts/slave_forward.py --self-test`(옛 셸 파생과 실행 대조 · 템플릿/렌더 구조 tripwire).
   compose 기본값이 필요하면 `slave_forward.py default --compose <c> --key <K>`(옛 하드 폴백 `0.27.1` 대체).
6. **serve 시점 관측 영속**(2026-09-21 · 스모크가 쓴다 · `output/multi/benchlog/` 비추적):
   - `attestation_<CONFIG>.json` **v2** — READY 뒤 두 노드의 **실행 중 컨테이너**에서 `build_ledger.json`(옛 이미지는
     `unobservable`)·vllm git sha·vllm/torch 배포판 버전(`importlib.metadata` — `import vllm` ✗)·호스트 driver 를 캡처하고 축별
     `equal|differ|unobservable` 을 **기록만** 한다(differ 차단 여부 = 사람 결정 D-5). image digest 는 판정 축이 아니다(동일 ABI ≠
     동일 이미지 — 2026-08-22). `checks` 는 대조 **행** 목록이다: `--build` 실행의 빌드 후 대조 행(`phase: build`)과
     serve 시점 관측 행(`phase: serve` · `expected` = 상대 노드의 관측값 · 관측 못 한 값은 행을 만들지 않는다 — 행 0 이 곧 결손).
   - `serve_proof_<CONFIG>.json` — `health_http`·`inference_verdict`(pass | relaxed-reasoning-only | fail | not-attempted)·`evidence`
     (chat.content | v1.completions)·`content_len`(**추론 증거가 된 생성 텍스트의 길이** — v1.completions 증거면 그 텍스트 길이 ·
     완화 PASS·실패 = 0 · 미시도 = null) · `chat_content_len` · `provenance: measured(multinode_serve_smoke)`. 실패도 적는다
     (옛 PASS 가 남지 않게) · 시각 없음. hint 발행 자격(X8)의 1순위 입력이다(`content_len ≥ 1` 을 본다).
   - 캠페인 진행표: 빌드 성공/실패 시 `campaign_init --phase-set build`, 스모크 뒤 `--phase-set serve`(`--cell $CONFIG`).
     **이 스크립트를 돌리는 노드(master 가 로컬인 메인)의 진행표만** 적는다 — 멀티 셀은 배정 노드의 클러스터 셀이고,
     `phases/<node>/` 는 노드당 한 자리(마지막 쓰기가 이긴다)라 메인이 `phases/<sub>` 를 쓰면 서브 자기저작 진행표를 덮는다
     (P5 가 결함으로 잡는 형태). slave 의 빌드·기동 결과는 proof 출처(양 노드 rc · attestation v2)에 실린다.
     캠페인 밖(ACTIVE=_bootstrap)이면 writer 가 no-op.

## 학습 (반드시 적용)

- **빌딩블럭은 gitignored → 브랜치 전환에도 persist**(워킹디렉토리 단일 사본). cross-branch 동기화·cherry-pick **불필요**.
  단 multi-node `.gitignore` 도 `.claude/`+`CLAUDE.md`+`seed/` 제외하도록 정렬(실수 추적 방지).
- **준비 판정 = 엔드포인트 `:PORT/health` http 200**. master 로그의 "Application startup complete" 는 조기 컴포넌트에서도 떠
  **거짓양성**(로그 grep 금지).
- **reasoning 모델(gpt-oss 등)**: 스모크 `max_tokens` 충분히(content는 `finish_reason=stop` 도달 후).
  content 또는 reasoning 비어있지 않으면 통과.
- **서브 빌드는 직접 SSH 백그라운드**(긴 빌드를 코드에이전트에 위임하면 harness 타임아웃 위험 — 위임 문법은
  `../../terraforming_node/references/orchestration/agent-control-adapter.md`). 서브 셸 호출은 login shell PATH 로.
- master 만 API 노출 → **프로브는 master 엔드포인트만**. slave 는 worker(API 없음).
- 멀티노드 실패(OOM/NCCL-RDMA/Ray join timeout)는 requirements/source-build 클래스가 아님 → **Model-C(HITL)**.
- **빌드 교차검증**: 서브 독립 빌드가 메인과 동일 동작(서브만 실패면 환경 불일치 신호 → Model-C). 2026-09-21 부터는
  **측정 가능하다** — 양 노드 원장의 정체성 투영(dockerfile · build_args − BUILD_JOBS · 패치 {phase,file,sha256,result} · vllm sha)을
  attestation v2 가 대조한다. post 패치 배달 경로가 비어 있어(sync_to_sub `BAND2_PATCH_DIRS=()`) 서브 `build_patches/` 가 낡을 수 있고,
  그 갈림을 잡는 유일한 검출기가 이 대조다.
- **멀티노드 deps = wheel METADATA + ray**: vLLM 은 ray 를 핵심 Requires-Dist 로 선언하지 않음(단일노드 불필요).
  멀티노드 분산(serve_runner 의 `ray start`)엔 **ray 필수** → multi-node requirements.txt 에 명시 추가
  (0.18.0 핀 `ray==2.48.0`, vLLM `requirements/test.txt` 기준). 캐시가 가리면 비재현 → `--build` 재현빌드로 확인(testlog_260607_8).
