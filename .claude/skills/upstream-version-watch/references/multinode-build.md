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
   **변종이미지 시 build-plane ≠ serve-plane**: 이미지 정체(`IMAGE_TAG`·`VLLM_REPO`/`VLLM_REF` build-arg)는 클러스터-wide **Band2**
   라 슬레이브 compose 보간에도 forward(슬레이브가 변종 이미지를 직접 빌드+기동) · 모델 serve config(`CONFIG_FILE`·트리플렛)는
   **Band3** 라 슬레이브 미forward → `multinode_serve_smoke.sh` 는 슬레이브에 `IMAGE_TAG`/`VLLM_REPO`/`VLLM_REF` 만
   forward(`CONFIG_FILE` ✗). ∴ 슬레이브 Band2-only = **serve-plane 불변식**(build-plane 아님).
   헌법 "변종이미지 build-plane ≠ serve-plane 따름정리" · `.claude/rules/workflow.md` S2.5.

## 학습 (반드시 적용)

- **빌딩블럭은 gitignored → 브랜치 전환에도 persist**(워킹디렉토리 단일 사본). cross-branch 동기화·cherry-pick **불필요**.
  단 multi-node `.gitignore` 도 `.claude/`+`CLAUDE.md`+`seed/` 제외하도록 정렬(실수 추적 방지).
- **준비 판정 = 엔드포인트 `:PORT/health` http 200**. master 로그의 "Application startup complete" 는 조기 컴포넌트에서도 떠
  **거짓양성**(로그 grep 금지).
- **reasoning 모델(gpt-oss 등)**: 스모크 `max_tokens` 충분히(content는 `finish_reason=stop` 도달 후).
  content 또는 reasoning 비어있지 않으면 통과.
- **서브 빌드는 직접 SSH 백그라운드**(긴 빌드를 코드에이전트에 위임하면 harness 타임아웃 위험 — 위임 문법은
  `../../terraforming_node/references/agent-control-adapter.md`). 서브 셸 호출은 login shell PATH 로.
- master 만 API 노출 → **프로브는 master 엔드포인트만**. slave 는 worker(API 없음).
- 멀티노드 실패(OOM/NCCL-RDMA/Ray join timeout)는 requirements/source-build 클래스가 아님 → **Model-C(HITL)**.
- **빌드 교차검증**: 서브 독립 빌드가 메인과 동일 동작(서브만 실패면 환경 불일치 신호 → Model-C).
- **멀티노드 deps = wheel METADATA + ray**: vLLM 은 ray 를 핵심 Requires-Dist 로 선언하지 않음(단일노드 불필요).
  멀티노드 분산(serve_runner 의 `ray start`)엔 **ray 필수** → multi-node requirements.txt 에 명시 추가
  (0.18.0 핀 `ray==2.48.0`, vLLM `requirements/test.txt` 기준). 캐시가 가리면 비재현 → `--build` 재현빌드로 확인(testlog_260607_8).
