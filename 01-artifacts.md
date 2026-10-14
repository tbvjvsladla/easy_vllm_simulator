# 01 · 산출물 — 무엇이 실제로 쓰였고, 어떻게 다시 띄우는가

## 1.1 적용 판정 표
> 기계가 적용 증거(빌드 원장 · 원장이 없으면 라벨된 재구성) × 파일 × 선언을 대사한 결과다. `artifacts/` 에는 **적용된 것만** 실렸다 — 빌드 때 스스로 skip 된 패치와 쓰이지 않은 평면의 파일은 표에만 남고 싣지 않는다.

<!-- FACT:slots -->
| 슬롯 | 실림 | 파일 | 신호 | 적용 증거 | 사유 |
|---|---|---|---|---|---|
| `build_patch_post` | 안 실림 | — | 1-signal | build-ledger:docker-image(main-local):easy-vllm:0.31.0-cu133-x86_64-source:/opt/easy-vllm/build_ledger.json | 적용 판정 없음 — 원장 관측(기계 파생) |
| `build_patch_pre` | 안 실림 | — | 1-signal | build-ledger:docker-image(main-local):easy-vllm:0.31.0-cu133-x86_64-source:/opt/easy-vllm/build_ledger.json | 적용 판정 없음 — 원장 관측(기계 파생) |
| `build_recipe` | 실림 | `artifacts/build_recipe/Dockerfile.source-build` · `artifacts/build_recipe/requirements.txt` | 3-signal | build-ledger:build-ledger.dockerfile | (a) NGC pytorch:26.07(CUDA 13.3.1 · torch 2.13.0a0) 위에 vLLM v0.31.0을 소스빌드한다. 0.31.0에서 `use_existing_torch.py`가 `tools/`로 이동하고 확장 모듈이 개명(`_C_stable_libtorch` 등)됐는데, 템플릿이 두 경로를 모두 감지하는 버전적응 스탠자를 쓴다(빼면 빌드가 W1에서 멈춘다). (b) 이 셀에 필요한 이유: torch 2.13 ABI라 prebuilt wheel 트랙이 성립하지 않기 때문(W1). (c) 이 셀에서 발화: 빌드 성공 이력이 있고(이미지 CreatedAt 2026-10-12), 셀의 모든 dtype 기동이 이 이미지로 돌았다. (d) 불필요 조건: 대상 vLLM의 torch 선언과 NGC… |
| `compose` | 실림 | `artifacts/compose/.env.template` · `artifacts/compose/docker-compose.yaml` | 2-signal | file:output/single/docker-compose.yaml | (a) 단일노드 서빙 평면: NAS 모델 루트를 `/app/models`에 ro 마운트하고, JIT/컴파일 캐시를 `./cache/vllm`·`./cache/flashinfer`에 영속 마운트한다. (b) 캐시 마운트가 W2의 부작용을 막는다 — 컨테이너 재생성마다 torch.compile·autotune이 cold가 되면 기동 때마다 JIT 발열 구간을 다시 지난다. (c) 이 셀에서 발화: fp8 재기동이 웜 캐시로 컴파일 구간을 건너뛰어 부팅됐다(testlog_26101319 §백필). (d) 불필요 조건: 호스트 쿨링 마진이 커서 cold JIT도 버티는 환경. |
| `fork_pin` | 안 실림 | — | 1-signal | none:.claude/policies/arch_variant_ledger.json | 셀 env 에 VARIANT 줄 없음 = stock(workflow.md §변종 좌표의 거처)(기계 파생) |
| `runtime_patch` | 안 실림 | — | 1-signal | none:output/single/configs/qwen3-4b_patch.py | output/single/configs/qwen3-4b_patch.py 없음 — arm_patch.sh 는 no-op(런타임 패치 불해당)(기계 파생) |
| `triplet` | 실림 | `artifacts/triplet/.env.qwen3-4b-kv-bf16` · `artifacts/triplet/qwen3-4b-kv-bf16.sh` · `artifacts/triplet/qwen3-4b-kv-bf16.yaml` | 2-signal | file:output/single/configs/qwen3-4b-kv-bf16.yaml | (a) 셀의 서빙 입력 전부: 모델 경로 · max-model-len · max-num-seqs · KV 절대 클램프 · prefix cache OFF · serve_env(MAX_JOBS=1). (b) KV 바이트를 명시해 gmu 추정에 의존하지 않는다(KV_ABSOLUTE_CLAMP_PORTABILITY). (c) 이 셀에서 발화: 이 트리플렛(bf16)과 fp8 변형으로 N* 측정이 완주됐다. (d) KV 클램프 값은 arch-scaled라 다른 환경에서는 재도출(V4). |

**레시피 경고 · 실린 리비전**(기계 관측 — 수신자 빌드가 여기서 갈라질 수 있다)

- `build_recipe` `output/single/Dockerfile.source-build` 실린 리비전: 리비전 미특정 · 선택 방법 `worktree(측정 이미지 docker history 의 RUN·COPY 와 전수 일치)` · docker history 대조 17/17 · 실린 바이트 `worktree` · 쓰인 바이트임을 관측으로 확인 예
- `build_recipe` `output/single/requirements.txt` 실린 리비전: 리비전 미특정 · 선택 방법 `worktree(빌드 원장 sha256 일치 — 쓰인 바이트 관측 확인 = 예)` · docker history 대조 없음 · 실린 바이트 `worktree` · 쓰인 바이트임을 관측으로 확인 예
- `compose` `output/single/docker-compose.yaml` 실린 리비전: 커밋 `ca7bfb99a397` · 선택 방법 `worktree(mtime 2026-10-12T10:49:21Z ≤ 측정 2026-10-14T04:55:42Z(certificate.measured_utc) · 바이트 = 측정 전 마지막 커밋 ca7bfb99a397)` · docker history 대조 없음 · 실린 바이트 `worktree` · 근거 `mtime≤measured` · 쓰인 바이트임을 관측으로 확인 예

**싣지 않은 파일**(슬롯별 사유 — 빌드 때 skip 된 패치는 아래 적용 집합 표에 있다)

- `build_patch_post` known-non-patch 1건: `.gitkeep`
- `build_patch_pre` known-non-patch 1건: `.gitkeep`
- `compose` `arm_patch.sh` — `not-armed(output/single/configs/qwen3-4b_patch.py 없음 — arm_patch.sh 는 no-op)`

**파일별 검증 · 관련성**(`generated-unverified` = 실행 검증되지 않은 생성물 — 파일 첫 줄 경고 · PAYLOAD 파일 기록과 같은 표시 · 관련성 = ① 패치 선언 대상 × 모델 config ② 엔진 로그 발화 서명 — 둘 다 관측일 때만 required/inactive-inferred)

| 슬롯 | 파일 | 검증 | 검증 근거 | 관련성 | 관련성 근거 |
|---|---|---|---|---|---|
| `build_recipe` | `artifacts/build_recipe/Dockerfile.source-build` | `verified` · cell-run | 쓰인 레시피(build-ledger.dockerfile) — 개정 선택 slots.build_recipe.evidence.selected_revisions | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `build_recipe` | `artifacts/build_recipe/requirements.txt` | `verified` · cell-run | 쓰인 레시피(build-ledger.dockerfile) — 개정 선택 slots.build_recipe.evidence.selected_revisions | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/.env.template` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/docker-compose.yaml` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/.env.qwen3-4b-kv-bf16` | `verified` · cell-run | 셀 트리플렛 — 측정 대조 없음 | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/qwen3-4b-kv-bf16.sh` | `verified` · cell-run | 셀 트리플렛 — 측정 대조 없음 | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/qwen3-4b-kv-bf16.yaml` | `verified` · cell-run | 셀 트리플렛 — 측정 대조 없음 | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
<!-- /FACT:slots -->

<!-- FACT:applied_set -->
**적용 집합** — 상태 `observed` · 출처 `docker-image(main-local):easy-vllm:0.31.0-cu133-x86_64-source:/opt/easy-vllm/build_ledger.json`

| 단계 | 파일 | 결과 | 결과 출처 | 사유 | 실림 | model-trigger |
|---|---|---|---|---|---|---|
| inline | `strip-hoist` | skipped | build-ledger | register_opaque_type accepts hoist / opaque_object 부재 | 안 실림 | — |

**결과 출처 범례**

- `build-ledger` — 이미지 안 빌드 원장이 적용 결과를 기록했다(관측)

**판정 근거**(패치별 — artifacts 가 판정에 쓴 관측 · 스크립트 바이트 정체 = 실린 바이트가 측정 이미지에 구워진 바이트인가)

| 파일 | 스크립트 바이트 정체 | 이미지 사본 sha256 | 적용 표지(상태 찾음/선언) | 정적 규칙 | 원장 분류 |
|---|---|---|---|---|---|
| `strip-hoist` | — | — | — | — | status-file |

**원장 탐침**(순위: attestation → 메인 로컬 이미지 → 재구성)

| 순위 | 결과 | 대상 |
|---|---|---|
| attestation | absent | output/single/benchlog/attestation_qwen3-4b.json(부재) |
| local-image | observed | easy-vllm:0.31.0-cu133-x86_64-source |
<!-- /FACT:applied_set -->

## 1.2 슬롯별 적용 사유
> 이 절이 답하는 질문: 실린 산출물 하나하나는 무엇을 고치며, 왜 이 셀에 필요했고, 이 셀에서 실제로 발화했으며, 언제 불필요해지는가?

### build_recipe (Dockerfile.source-build + requirements.txt)
(a) NGC pytorch:26.07(CUDA 13.3.1 · torch 2.13.0a0) 위에 vLLM v0.31.0을 소스빌드한다. 0.31.0에서 `use_existing_torch.py`가 `tools/`로 이동하고 확장 모듈이 개명(`_C_stable_libtorch` 등)됐는데, 템플릿이 두 경로를 모두 감지하는 버전적응 스탠자를 쓴다(빼면 빌드가 W1에서 멈춘다). (b) 이 셀에 필요한 이유: torch 2.13 ABI라 prebuilt wheel 트랙이 성립하지 않기 때문(W1). (c) 이 셀에서 발화: 빌드 성공 이력이 있고(이미지 CreatedAt 2026-10-12), 셀의 모든 dtype 기동이 이 이미지로 돌았다. (d) 불필요 조건: 대상 vLLM의 torch 선언과 NGC 베이스 torch 빌드가 접두어 일치하는 버전 조합이 되면 wheel 트랙으로 갈 수 있다 — 스탠자는 감지 경로가 없으면 `exit 1`로 멈춘다(skip 아님).

### compose (docker-compose.yaml + .env.template)
(a) 단일노드 서빙 평면: NAS 모델 루트를 `/app/models`에 ro 마운트하고, JIT/컴파일 캐시를 `./cache/vllm`·`./cache/flashinfer`에 영속 마운트한다. (b) 캐시 마운트가 W2의 부작용을 막는다 — 컨테이너 재생성마다 torch.compile·autotune이 cold가 되면 기동 때마다 JIT 발열 구간을 다시 지난다. (c) 이 셀에서 발화: fp8 재기동이 웜 캐시로 컴파일 구간을 건너뛰어 부팅됐다(testlog_26101319 §백필). (d) 불필요 조건: 호스트 쿨링 마진이 커서 cold JIT도 버티는 환경.

### triplet (.env + .sh + .yaml, qwen3-4b-kv-bf16)
(a) 셀의 서빙 입력 전부: 모델 경로 · max-model-len · max-num-seqs · KV 절대 클램프 · prefix cache OFF · serve_env(MAX_JOBS=1). (b) KV 바이트를 명시해 gmu 추정에 의존하지 않는다(KV_ABSOLUTE_CLAMP_PORTABILITY). (c) 이 셀에서 발화: 이 트리플렛(bf16)과 fp8 변형으로 N* 측정이 완주됐다. (d) KV 클램프 값은 arch-scaled라 다른 환경에서는 재도출(V4).

### 실리지 않은 슬롯
- build_patch_pre/post: 적용 집합 `strip-hoist` 1걿� `skipped`(원장: register_opaque_type accepts hoist / opaque_object 부재) — 이 셀은 자체 빌드 패치가 0건이다.
- fork_pin: 셀 env에 VARIANT 줄 없음 = stock(vLLM 공식 태그).
- runtime_patch: `output/single/configs/qwen3-4b_patch.py` 없음 — arm_patch.sh는 no-op.

## 1.3 값의 지위표
> 서빙 설정의 노브 **전부**와 기계가 모은 후보 지위다(lockset 출처 · yaml 주석 · 엔진 기본값). 최종 지위는 아래 `kind: value-status` 블록이 정한다 — 표의 노브마다 블록이 정확히 하나 있어야 린터를 통과한다.

<!-- FACT:value_status -->
서빙 노브 5개 — 노브마다 `kind: value-status` 블록이 **정확히 1개** 있어야 한다(노브·값은 이 표의 글자 그대로 · 값 칸의 `\|` 는 `|` 로 적는다).

| 노브 | 값 | 후보 지위 | 후보 근거 | yaml 주석 | 출처 |
|---|---|---|---|---|---|
| `gpu-memory-utilization` | 0.9 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `max-model-len` | 40960 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `max-num-seqs` | 512 | inherited | lockset 출처 batch_source=hand-lever · lockset provenance=hand-authored | — | lockset.batch_source=hand-lever |
| `kv-cache-memory-bytes` | 81668803133 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `no-enable-prefix-caching` | true | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
<!-- /FACT:value_status -->

> 이 절이 답하는 질문: 각 서빙 노브의 값은 이 셀에서 어떤 지위인가 — 조정됐나, 승계됐나, 음성대조로 일부러 둔 것인가, 엔진 기본값인가, 필요조건인가?

```hint-event
id: V1
kind: value-status
노브: gpu-memory-utilization
값: 0.9
지위: inherited
근거: 캠페인 선언 상수(config.yaml target_gmu) — 이 셀에서 값을 바꿔 잰 기록 없음
출처: [plan_26101219 §2. 인터뷰 확정값]
```
```hint-event
id: V2
kind: value-status
노브: max-model-len
값: 40960
지위: inherited
근거: 모델 config.json max_position_embeddings 선언을 그대로 옮긴 측정 통제변인(RoPE/YaRN 확장 미적용 결정) — 조정 기록 없음
출처: [plan_26101219 §2. 인터뷰 확정값]
```
```hint-event
id: V3
kind: value-status
노브: max-num-seqs
값: 512
지위: inherited
근거: 손레버(lockset batch_source=hand-lever) — 동시성 측정을 배치 상한이 막지 않게 넉넉히 둔 값 · 스윕 미실시
출처: [testlog_26101317 §머리 셀 lockset]
```
```hint-event
id: V4
kind: value-status
노브: kv-cache-memory-bytes
값: 81668803133
지위: tuned
근거: 이 셀 계보에서 도출 — bf16 무클��프 부팅 엔진 실측 Available KV 76.06 GiB로 고정(measured-clamp · arch-scaled)
출처: [testlog_26101317_qwen3-4b_KV양자화_동시성_판정 §머리 셀 lockset]
```
```hint-event
id: V5
kind: value-status
노브: no-enable-prefix-caching
값: true
지위: inherited
근거: 측정 통제변인(인터뷰 확정) — dtype 간 비교에서 prefix cache 효과를 배제하기 위한 선언 · 서빙 필요조건은 아님
출처: [plan_26101219 §2. 인터뷰 확정값]
```

**요약** — 복사 위험이 가장 큰 값은 V4(kv-cache-memory-bytes)다: 이 노드의 VRAM 95.6GiB × gmu 0.90에서 측정된 것이라 다른 VRAM·gmu에서는 과잉(기동 실패) 또는 과소(용량 낭비)다. V3·V5는 측정 정의의 일부이지 서빙 필요조건이 아니다.

## 1.4 재현 절차
> 셀 실행 기록에서 기계가 파생한 명령 순서와 단계별 소요다. 소요가 `미관측` 인 단계는 시각 기록이 없다는 뜻이다(0 이 아니다). 그 아래는 블랙박스 이벤트 원장에서 이 셀의 행을 기계가 뽑아 기동 시도로 묶은 것이고, 마지막은 측정한 실행의 엔진 로그가 echo 한 env 다(측정 당시 값의 정본).

<!-- FACT:reproduce -->
| 순서 | 단계 | 성공 판정 | 소요 | 관측 구간(UTC) | 소요 출처 | 명령 출처 |
|---|---|---|---|---|---|---|
| 1 | build | 빌드 OK · 이미지 실재 | 층 창 1시간 39분 25초(docker history 첫 층 → 끝 층 CreatedAt · 캐시 재사용 층 포함 — 빌드 소요 아님) | 2026-10-12T10:55:01Z → 2026-10-12T12:34:26Z (layer-window) | docker history --no-trunc --format '{{json .}}' easy-vllm:0.31.0-cu133-x86_64-source · 우리 Dockerfile 층 30개의 CreatedAt 첫 층 → 끝 층(캐시 재사용 층은 이전 빌드 시각 — 빌드 소요 아님) | reconstructed(build-ledger build_args + compose build) |
| 2 | serve | health 200 + 추론 1회(스모크 판정) | ≤ 1분 36초(상한) | 2026-10-14T04:54:06Z → 2026-10-14T04:55:42Z (upper) | docs/logs/main/events/2026-10.jsonl budget_declare(label serve-qwen3-4b) → 첫 측정 시각(output/single/benchlog/lite_publish/qwen3-4b_20261014T045542Z/lite_raw_qwen3-4b.json measured_utc(= bench_report 머리 생성일 · lite_bench.sh 가 cold 레그 직전(부하 직전)에 찍는 측정 **시작** 시각)) | derived(스모크 사용법) |
| 3 | bench | 리포트 발행(+PASS 면 인증서) | ≈ 0분 21초(두 끝 시계 다름 — 컨테이너 date · 호스트 UTC · 근사) | 2026-10-14T04:55:42Z → 2026-10-14T04:56:03Z (clock-mixed) | 시작 = output/single/benchlog/lite_publish/qwen3-4b_20261014T045542Z/lite_raw_qwen3-4b.json measured_utc(= bench_report 머리 생성일 · lite_bench.sh 가 cold 레그 직전(부하 직전)에 찍는 측정 **시작** 시각) → 끝 = output/single/benchlog/lite_publish/qwen3-4b_20261014T045542Z/lite_warm_qwen3-4b.json date(마지막 레그 결과 저장 · 컨테이너 시계 = UTC 가정) | reconstructed(bench json) |

**빌드 층 시각**(관측 · 소요가 아니라 층 창 — 캐시 재사용 층은 이전 빌드의 시각을 가진다)

- `build` 이미지 층 CreatedAt(docker history): 2026-10-12T10:55:01Z → 2026-10-12T12:34:26Z · 층 30개 · 출처 `docker history --no-trunc --format '{{json .}}' easy-vllm:0.31.0-cu133-x86_64-source` — 우리 Dockerfile 층의 CreatedAt 범위 — 캐시 재사용 층은 이전 빌드 시각을 가진다(소요 아님)

**단계별 명령**(기계 파생 — `명령 출처` 가 `derived(…)` 면 사용법에서 파생 · `reconstructed(…)` 면 괄호 안 관측(docker history build-arg · compose build · bench JSON 필드)에서 재구성한 명령이다. 어느 쪽도 실행 원문(로그에 남은 줄)은 아니다)

**1. build**

```sh
# 실린 artifacts/build_recipe/Dockerfile.source-build(이미지를 지은 개정: worktree)를 output/single/Dockerfile.source-build 자리에 두고 컨텍스트에서 짓는다.
docker build -f output/single/Dockerfile.source-build \
  --build-arg BUILD_JOBS=8 \
  --build-arg RAY_VERSION=2.48.0 \
  --build-arg SM12X_PORT=0 \
  --build-arg SRC_DEPS_AUTHORITY=0 \
  --build-arg TORCH_CUDA_ARCH=12.0 \
  --build-arg VLLM_PRETEND_VERSION= \
  --build-arg VLLM_REF=v0.31.0 \
  --build-arg VLLM_REPO=https://github.com/vllm-project/vllm.git \
  -t easy-vllm:0.31.0-cu133-x86_64-source \
  output/single
# 동등 경로(compose): docker compose -f output/single/docker-compose.yaml --env-file output/single/envs/.env.qwen3-4b build
```

**2. serve**

```sh
bash .claude/skills/upstream-version-watch/scripts/single_serve_up.sh qwen3-4b
```

**3. bench**

```sh
# bench JSON 필드로 복원: backend · model_id→--model · tokenizer_id→--tokenizer · num_prompts · max_concurrency · request_rate · burstiness · 요청당 입출력 길이 = total_*_tokens ÷ completed(나누어떨어질 때만 — 평균이다 · 요청별 길이는 JSON 에 없다) · 나누어떨어지면 `--dataset-name random` 으로 추정했다(측정 도구가 그 꼴로 부른다 — 아래 도구 원문).
# JSON 에 없는 인자(--endpoint · --base-url · --ignore-eos · --num-warmups · --temperature · --seed · --random-range-ratio · --trust-remote-code)는 측정 도구가 정했다 — level_* = run_bench.sh · lite_cold/lite_warm = lite_bench.sh(.claude/skills/adversarial-benchmark/scripts/) · 셀 스윕 순서는 sweep_bench.sh.
# 호출(재구성 — lite_raw config_name · topology · backend + 바인딩된 경량 리포트 = --publish-report · 원문 호출 줄 아님): bash .claude/skills/adversarial-benchmark/scripts/lite_bench.sh qwen3-4b --topology single --backend openai-chat --publish-report
# 측정 창: 시작 2026-10-14T04:55:42Z(output/single/benchlog/lite_publish/qwen3-4b_20261014T045542Z/lite_raw_qwen3-4b.json measured_utc(= bench_report 머리 생성일 · lite_bench.sh 가 cold 레그 직전(부하 직전)에 찍는 측정 **시작** 시각)) → 끝 2026-10-14T04:56:03Z(output/single/benchlog/lite_publish/qwen3-4b_20261014T045542Z/lite_warm_qwen3-4b.json date(마지막 레그 결과 저장 · 컨테이너 시계 = UTC 가정))
# output/single/benchlog/lite_publish/qwen3-4b_20261014T045542Z/lite_cold_qwen3-4b.json (date 20261014-045552) · ⚠ --random-input-len/--random-output-len = total ÷ completed 평균(chat 템플릿 포함 실측 토큰 · 측정 도구 원문 인자 미관측 — lite_bench.sh 원문 인자와 다를 수 있다)
vllm bench serve --backend openai-chat --model Qwen3-4B --tokenizer /app/models/Qwen/Qwen3-4B --dataset-name random --random-input-len 520 --random-output-len 128 --num-prompts 1 --max-concurrency 1 --request-rate inf
# output/single/benchlog/lite_publish/qwen3-4b_20261014T045542Z/lite_warm_qwen3-4b.json (date 20261014-045603) · ⚠ --random-input-len/--random-output-len = total ÷ completed 평균(chat 템플릿 포함 실측 토큰 · 측정 도구 원문 인자 미관측 — lite_bench.sh 원문 인자와 다를 수 있다)
vllm bench serve --backend openai-chat --model Qwen3-4B --tokenizer /app/models/Qwen/Qwen3-4B --dataset-name random --random-input-len 520 --random-output-len 128 --num-prompts 3 --max-concurrency 1 --request-rate inf
```

**슬롯 → 빌드 컨텍스트 매핑**(빌드 컨텍스트 = `output/<토폴로지>/` · 원천 = 실린 Dockerfile 의 COPY · compose 의 env_file · volumes · command — `—` 는 빌드 컨텍스트 입력이 아니다)

| zip 경로 | 빌드 컨텍스트 자리 | 근거 |
|---|---|---|
| `artifacts/triplet/.env.qwen3-4b-kv-bf16` | `envs/.env.qwen3-4b-kv-bf16` | compose env_file envs/.env.${CONFIG_FILE}(셀 env) |
| `artifacts/triplet/qwen3-4b-kv-bf16.sh` | — | 빌드 컨텍스트 입력 아님(Dockerfile COPY · compose 참조 밖) |
| `artifacts/triplet/qwen3-4b-kv-bf16.yaml` | — | 빌드 컨텍스트 입력 아님(Dockerfile COPY · compose 참조 밖) |
| `artifacts/build_recipe/Dockerfile.source-build` | `Dockerfile.source-build` | compose build.dockerfile Dockerfile.source-build(리터럴) · build.context . |
| `artifacts/build_recipe/requirements.txt` | `requirements.txt` | Dockerfile.source-build COPY requirements.txt |
| `artifacts/compose/.env.template` | `.env` | compose 변수치환 프로젝트 .env(${…} 참조) |
| `artifacts/compose/docker-compose.yaml` | `docker-compose.yaml` | compose 파일(빌드 컨텍스트 루트) |

**기동 성공 판정(관측 · 발행 자격)**: health 200 = true · 추론 1회 = true · 근거 `output/single/benchlog/lite_publish/qwen3-4b_20261014T045542Z/lite_raw_qwen3-4b.json` · `output/single/benchlog/lite_publish/qwen3-4b_20261014T045542Z/lite_warm_qwen3-4b.json`
<!-- /FACT:reproduce -->

<!-- FACT:event_timeline -->
**블랙박스 이벤트 7행**(원장에서 이 셀 label·config 에 맞는 행만 · 시각순 · 기계 발췌)

> **캠페인 경계** 2026-10-12T10:49:53Z(`camp-26101210-kvq-concurrency` · evidence.event_campaign_boundary): campaigns/camp-26101210-kvq-concurrency/campaign.yaml declared_utc 2026-10-12T10:49:53Z · 이 발행이 묶인 측정을 서빙한 budget_declare 창 시작 2026-10-14T04:54:06Z → 경계 = 앞선 값 2026-10-12T10:49:53Z(이전 행 = 같은 셀 이름 · 앞선 캠페인)

> **원장 관측 범위**(노드별 첫 행 → 마지막 행 · evidence.event_ledger_spans): main = 2026-10-12T10:41:33Z → 2026-10-14T04:56:14Z(`docs/logs/main/events/2026-10.jsonl` · `docs/logs/main/events/thermal.jsonl` · `docs/logs/main/events/watchdog.jsonl`)

| # | UTC | 노드 | kind | label | 내용 | 출처 |
|---|---|---|---|---|---|---|
| 1 | 2026-10-14T04:54:06Z | main | `budget_declare` | `serve-qwen3-4b` | 예산 선언(선언값 — 측정 아님) · floor_mib=43736 · weights_mib=3802 · kv_mib=0 · overhead_mib=16384 · mem_total_mib=63922 · 기동 창 1/1: budget_renew 1회 · 닫힘 budget_clear 2026-10-14T04:56:14Z(선언 후 2m08s) · measured_utc 2026-10-14T04:55:42Z 가 이 창 안(이 측정의 기동) | docs/logs/main/events/2026-10.jsonl:735 |
| 2 | 2026-10-14T04:54:06Z | main | `serve_start` | — | serve_start · model=Qwen3-4B · tensor_parallel_size=1 · topology=single · session=serve-qwen3-4b-20261014T045406Z | docs/logs/main/events/2026-10.jsonl:736 |
| 3 | 2026-10-14T04:54:06Z | main | `budget_renew` | `serve-qwen3-4b` | budget_renew · floor_mib=43736 · ttl_s=7200 · remaining_before_s=7200 · 창 시작 2026-10-14T04:54:06Z 후 0m00s | docs/logs/main/events/2026-10.jsonl:737 |
| 4 | 2026-10-14T04:54:06Z | main | `budget_honored` | `serve-qwen3-4b` | budget_honored · floor_mib=43736 · arm_ceiling_mib=40664 · remaining_s=7200 · 창 시작 2026-10-14T04:54:06Z 후 0m00s | docs/logs/main/events/watchdog.jsonl:75 |
| 5 | 2026-10-14T04:56:14Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → serve-qwen3-4b 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-14T04:54:06Z 후 2m08s) | docs/logs/main/events/2026-10.jsonl:738 |
| 6 | 2026-10-14T04:56:14Z | main | `serve_stop` | — | serve_stop · model=Qwen3-4B · duration_s=128 · session=serve-qwen3-4b-20261014T045406Z | docs/logs/main/events/2026-10.jsonl:739 |
| 7 | 2026-10-14T04:56:14Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → serve-qwen3-4b 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-14T04:54:06Z 후 2m08s) | docs/logs/main/events/watchdog.jsonl:76 |

**기동 시도**(예산 선언 `budget_declare` 마다 한 시도 · 같은 노드의 이어지는 행과 선언 없는 노드(엔진 로그)의 그 시각 행을 묶었다 · 사살 · 트립 · 거부는 닫힘과 따로 센다 · `measurement` 행은 측정 기록이라 시도로 세지 않는다(위 표에만) — 해석은 저작자 몫)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 1 | main | `serve-qwen3-4b` | 2026-10-14T04:54:06Z | 1회 · 첫 2026-10-14T04:54:06Z | 없음 | `budget_clear` 2026-10-14T04:56:14Z | 4 |
<!-- /FACT:event_timeline -->

<!-- FACT:measurement_env -->
_측정 실행의 env 관측 없음 — 측정 엔진 로그에서 env echo 줄(`<KEY> set by environment to <VALUE>` 등)을 찾지 못했다(evidence.measurement_env_observed · 부재 ≠ 미설정)._
<!-- /FACT:measurement_env -->

> 이 절이 답하는 질문: 수신자가 이 zip 만으로 이 셀을 다시 띄우려면 어떤 순서로 무엇을 하고, 각 단계의 성공을 무엇으로 판정하며, 얼마나 기다려야 하는가?

(a) **사전 준비**: 가중치 Qwen/Qwen3-4B(약 8GB)를 `<manifest.nas_model_path>/Qwen/Qwen3-4B`에 둔다(§1.6). 모델 루트가 compose로 `/app/models`에 ro 마운트돼야 하고, JIT 캐시 디렉터리(`output/single/cache/`)는 호스트에 쓰기 가능해야 한다. env 실값은 `output/single/envs/.env.qwen3-4b-kv-bf16`(프로젝트 `.env`는 compose/.env.template 형상을 채운다).
(b) **빌드**: 메인 노드에서 `docker build -f output/single/Dockerfile.source-build --build-arg VLLM_REF=v0.31.0 --build-arg TORCH_CUDA_ARCH=12.0 …`(01 §1.4 표의 전체 build-arg). 층 창 1시간 39분(캐시 재사용 포함 관측치 — 순수 빌드 소요 아님).
(c) **기동**: `bash .claude/skills/upstream-version-watch/scripts/single_serve_up.sh qwen3-4b` → 성공 판정 = health 200 + 추론 1회(관측 소요 ≤1분 36초 · 캐시 웜 상태). 이 셀은 자체 패치가 없어 발화 서명 확인 대상이 없다. 기동 중 JIT 컴파일·autotune 로그는 정상 진행이다(수 분, cold일수록 김).
(d) **호스트 메모리 예산 선언**: 이 측정의 선언은 floor_mib=43736 · weights_mib=3802(최대 샤드 · discrete) · kv_mib=0(KV는 VRAM) · overhead_mib=16384(선언값 — 원장 budget_declare 행; 엔진 로그 실측과는 다른 종류다). Muse 같은 단일 대형 샤드 모델에서는 이 산식이 교착해 --no-budget 승인 경로가 필요하다.
(e) **측정**: 01 §1.4 bench 단계(재구성 명령 — `lite_bench.sh qwen3-4b --topology single --publish-report`)와 03 §3.4 도구 원문. 본 캠페인의 N* 측정은 별도 프로브 드라이버(GuideLLM concurrent · E-core 피닝 · 런당 쿨다운 ≤55°C)로 했고 그 원시는 캠페인 probe 디렉터리에 있다.
(f) **런타임 경로**: bf16은 FLASH_ATTN 자동 선택(엔진 로그 `Using FLASH_ATTN attention backend` 행 관측). TP=1이라 NCCL 초기화는 단일 rank로 단순하다.

## 1.5 서브 레시피 해설
<!-- FACT:sub_recipe -->
해당 없음 — single 토폴로지(서브 노드 · Ray worker 없음).
<!-- /FACT:sub_recipe -->

<!-- FACT:measurement_env_nodes -->
해당 없음 — single 토폴로지(서브 노드 없음).
<!-- /FACT:measurement_env_nodes -->

> 해당 없음 — 이 절의 조건 `topology=multi` 이 이 셀에서 성립하지 않는다(`topology=single`).

## 1.6 재현 절차 — 기동 전 준비(사람이 할 일)
> 1.4 의 명령을 치기 **전에** 사람이 손으로 해야 하는 것이다 — 가중치 획득 · 스테이징 생성 · 네트워크가 필요한 단계. 에이전트가 대신 할 수 없는 자리(다운로드 승인 · 물리 경로 · 외부 계정)가 여기 모인다.

> 이 절이 답하는 질문: 이 셀을 기동하기 전에 사람이 준비해야 하는 것은 무엇이며, 각각을 어떤 명령으로 만들고, 그 절차의 출처는 어디인가?

### (a) 가중치 획득
Qwen/Qwen3-4B(체크포인트 BF16 · 약 8GB · revision 미관측)를 사람 승인 게이트 뒤 HF에서 받아 `<manifest.nas_model_path>/Qwen/Qwen3-4B`에 둔다. 이 캠페인에서는 stdlib 스냅샷 다운로더(이어받기·sha256 검증)로 받았고, 일반 명령은 `huggingface-cli download Qwen/Qwen3-4B --local-dir <manifest.nas_model_path>/Qwen/Qwen3-4B`다(추론 — 이 캠페인에서 쓴 도구는 `hf_fetch.py`, 스크립트는 세션 임시물이라 계보에 없다). 단일노드라 1개 호스트에만 필요하다.

### (b) PLE · 스테이징 생성
해당 없음 — 이 모델은 PLE 키(ple_layer_ids · ple_embed_dim) 부재 관측(00 §0.9 후보 표). 스테이징이 필요한 별도 자산은 없다.

### (c) 네트워크가 필요한 단계
- HF 가중치 다운로드(승인 게이트 — 무인 다운로드 ✗)
- NGC 베이스 이미지 pull(`nvcr.io/nvidia/pytorch:26.07-py3`)과 빌드 중 pip/wheel·vLLM 소스 clone — 오프라인 환경이면 베이스 이미지와 소스 스냅샷을 미리 받아 둔다
- 측정 도구 이미지 pull(`ghcr.io/vllm-project/guidellm`) — 벤치 컨테이너는 `HF_HUB_OFFLINE=1`로 돌아가므로 토크나이저는 모델 디렉터리에서 읽는다
