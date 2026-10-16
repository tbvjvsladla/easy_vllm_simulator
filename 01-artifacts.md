# 01 · 산출물 — 무엇이 실제로 쓰였고, 어떻게 다시 띄우는가

## 1.1 적용 판정 표
> 기계가 적용 증거(빌드 원장 · 원장이 없으면 라벨된 재구성) × 파일 × 선언을 대사한 결과다. `artifacts/` 에는 **적용된 것만** 실렸다 — 빌드 때 스스로 skip 된 패치와 쓰이지 않은 평면의 파일은 표에만 남고 싣지 않는다.

<!-- FACT:slots -->
| 슬롯 | 실림 | 파일 | 신호 | 적용 증거 | 사유 |
|---|---|---|---|---|---|
| `build_patch_post` | 안 실림 | — | 1-signal | none:no-build-ledger | 적용 판정 없음 — 원장 부재 — 라벨된 재구성: applied 0 · skip 판정 = docker-history+gate(기계 파생) |
| `build_patch_pre` | 안 실림 | — | 1-signal | none:no-build-ledger | 적용 판정 없음 — 원장 부재 — 라벨된 재구성: applied 0 · skip 판정 = docker-history+gate(기계 파생) |
| `build_recipe` | 실림 | `artifacts/build_recipe/Dockerfile.source-build` · `artifacts/build_recipe/requirements.txt` | 2-signal | file:image-history:docker history build-arg 관측(VLLM_REF=v0.31.0 · VLLM_VERSION 없음 → 소스빌드 Dockerfile) — 셀 env 선택자 없음 · output/single/docker-compose.yaml 리터럴 build.dockerfile: Dockerfile.source-build(선택자 변수 없음 — compose build 는 늘 Dockerfile.source-build 를 짓는다 · 이 이미지를 지은 증거 아님) | 이 셀의 vLLM v0.31.0 소스빌드 레시피다. 1.1 표가 docker history 16/16 대조로 쓰인 바이트임을 확인한다(관측). 패치 이식 출처나 hunk 구성은 이 계보 문서에 없다. 폐기 조건: 다른 빌드 트랙(wheel) 사용 시. |
| `compose` | 실림 | `artifacts/compose/.env.template` · `artifacts/compose/docker-compose.yaml` | 2-signal | file:output/single/docker-compose.yaml | single 서빙 경로(compose·env 형상)이며 측정 당시 개정(커밋 80284db86dd7)이다. 빌드·기동 경로가 읽는 입력이다. `arm_patch.sh` 는 런타임 패치 파일이 없어 불해당이다. |
| `fork_pin` | 안 실림 | — | 1-signal | none:.claude/policies/arch_variant_ledger.json | 셀 env 에 VARIANT 줄 없음 = stock(workflow.md §변종 좌표의 거처)(기계 파생) |
| `runtime_patch` | 안 실림 | — | 1-signal | none:output/single/configs/ornith15-9b-v031_patch.py | output/single/configs/ornith15-9b-v031_patch.py 없음 — arm_patch.sh 는 no-op(런타임 패치 불해당)(기계 파생) |
| `triplet` | 실림 | `artifacts/triplet/.env.ornith15-9b-v031` · `artifacts/triplet/ornith15-9b-v031.sh` · `artifacts/triplet/ornith15-9b-v031.yaml` | 3-signal | file:output/single/configs/ornith15-9b-v031.yaml | 이 셀의 서빙 설정 3종이다. yaml 은 서빙 노브 전부(§1.3), .sh 는 파서 인자, .env 는 포트·이미지 태그·모델 이름을 가진다. 이 셀에서 측정이 이 config 로 돌았다는 것이 스윕 색인의 근거다. |

**레시피 경고 · 실린 리비전**(기계 관측 — 수신자 빌드가 여기서 갈라질 수 있다)

- `build_recipe` `output/single/Dockerfile.source-build` 실린 리비전: 리비전 미특정 · 선택 방법 `worktree(측정 이미지 docker history 의 RUN·COPY 와 전수 일치)` · docker history 대조 16/16 · 실린 바이트 `worktree` · 쓰인 바이트임을 관측으로 확인 예
- `build_recipe` `output/single/requirements.txt` 실린 리비전: 리비전 미특정 · 선택 방법 `worktree(image-probe: 이미지 안 COPY 대상 바이트와 sha256 일치)` · docker history 대조 없음 · 실린 바이트 `worktree` · 쓰인 바이트임을 관측으로 확인 예
- `compose` `output/single/docker-compose.yaml` 실린 리비전: 커밋 `80284db86dd7` · 선택 방법 `worktree(mtime 2026-10-14T01:13:06Z ≤ 측정 2026-10-16T11:25:05Z(sweep_index.generated_utc) · 바이트 = 측정 전 마지막 커밋 80284db86dd7)` · docker history 대조 없음 · 실린 바이트 `worktree` · 근거 `mtime≤measured` · 쓰인 바이트임을 관측으로 확인 예

**싣지 않은 파일**(슬롯별 사유 — 빌드 때 skip 된 패치는 아래 적용 집합 표에 있다)

- `build_patch_post` known-non-patch 1건: `.gitkeep`
- `build_patch_pre` known-non-patch 1건: `.gitkeep`
- `compose` `arm_patch.sh` — `not-armed(output/single/configs/ornith15-9b-v031_patch.py 없음 — arm_patch.sh 는 no-op)`

**파일별 검증 · 관련성**(`generated-unverified` = 실행 검증되지 않은 생성물 — 파일 첫 줄 경고 · PAYLOAD 파일 기록과 같은 표시 · 관련성 = ① 패치 선언 대상 × 모델 config ② 엔진 로그 발화 서명 — 둘 다 관측일 때만 required/inactive-inferred)

| 슬롯 | 파일 | 검증 | 검증 근거 | 관련성 | 관련성 근거 |
|---|---|---|---|---|---|
| `build_recipe` | `artifacts/build_recipe/Dockerfile.source-build` | `verified` · cell-run | 쓰인 레시피(image-history:docker history build-arg 관측(VLLM_REF=v0.31.0 · VLLM_VERSION 없음 → 소스빌드 Dockerfile) — 셀 env 선택자 없음 · output/single/docker-compose.yaml 리터럴 build.dockerfile: Dockerfile.source-build(선택자 변수 없음 — compose build 는 늘 Dockerfile.source-build 를 짓는다 · 이 이미지를 지은 증거 아님)) — 개정 선택 slots.build_recipe.evidence.selected_revisions | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `build_recipe` | `artifacts/build_recipe/requirements.txt` | `verified` · cell-run | 쓰인 레시피(image-history:docker history build-arg 관측(VLLM_REF=v0.31.0 · VLLM_VERSION 없음 → 소스빌드 Dockerfile) — 셀 env 선택자 없음 · output/single/docker-compose.yaml 리터럴 build.dockerfile: Dockerfile.source-build(선택자 변수 없음 — compose build 는 늘 Dockerfile.source-build 를 짓는다 · 이 이미지를 지은 증거 아님)) — 개정 선택 slots.build_recipe.evidence.selected_revisions | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/.env.template` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/docker-compose.yaml` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/.env.ornith15-9b-v031` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/ornith15-9b-v031.sh` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/ornith15-9b-v031.yaml` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
<!-- /FACT:slots -->

<!-- FACT:applied_set -->
**적용 집합** — 상태 `unobservable` · 출처 `no-build-ledger` · 원장 없음: 아래 결과는 **라벨된 재구성**이다(관측 아님)

_빌드 패치 없음 — 이 이미지에 빌드 패치 슬롯 입력이 없다._

**원장 탐침**(순위: attestation → 메인 로컬 이미지 → 재구성)

| 순위 | 결과 | 대상 |
|---|---|---|
| attestation | absent | output/single/benchlog/attestation_ornith15-9b-v031.json(부재) |
| local-image | skipped(node=sub — 서브 스캔 ✗ · 문서기반 회수만) | — |
| reconstruction | done | — |
| image-probe | done | sha256:667eee7bd3b4e84b204ad4887bd811c87dd3537428d026eaeb580ab825d88c8e |

**재구성 입력** — `docker-history+gate-rule · fail-loud-rule · image-probe`(docker history build-arg × 패치 자기게이트 규칙)

| build-arg | 값 |
|---|---|
| `BUILD_JOBS` | 8 |
| `RAY_VERSION` | 2.48.0 |
| `SM12X_PORT` | 0 |
| `SRC_DEPS_AUTHORITY` | 0 |
| `TORCH_CUDA_ARCH` | 12.1a |
| `VLLM_PRETEND_VERSION` | (빈 값) |
| `VLLM_REF` | v0.31.0 |
| `VLLM_REPO` | https://github.com/vllm-project/vllm.git |
<!-- /FACT:applied_set -->

## 1.2 슬롯별 적용 사유
> 이 절이 답하는 질문: 실린 산출물 하나하나는 무엇을 고치며, 왜 이 셀에 필요했고, 이 셀에서 실제로 발화했으며, 언제 불필요해지는가?

### build_recipe (Dockerfile.source-build · requirements.txt)
이 셀의 vLLM v0.31.0 소스빌드 레시피다. 1.1 표가 docker history 16/16 대조로 쓰인 바이트임을 확인한다(관측). 패치 이식 출처나 hunk 구성은 이 계보 문서에 없다. 폐기 조건: 다른 빌드 트랙(wheel) 사용 시.

### compose (docker-compose.yaml · .env.template)
single 서빙 경로(compose·env 형상)이며 측정 당시 개정(커밋 80284db86dd7)이다. 빌드·기동 경로가 읽는 입력이다. `arm_patch.sh` 는 런타임 패치 파일이 없어 불해당이다.

### triplet (ornith15-9b-v031.yaml · .sh · .env)
이 셀의 서빙 설정 3종이다. yaml 은 서빙 노브 전부(§1.3), .sh 는 파서 인자, .env 는 포트·이미지 태그·모델 이름을 가진다. 이 셀에서 측정이 이 config 로 돌았다는 것이 스윕 색인의 근거다.

### 싣지 않은 슬롯
build_patch_pre/post — 입력이 없어 불해당(원장 부재라 적용 판정은 재구성 `unobservable`). runtime_patch — `ornith15-9b-v031_patch.py` 부재. fork_pin — VARIANT 줄이 없어 stock.

## 1.3 값의 지위표
> 서빙 설정의 노브 **전부**와 기계가 모은 후보 지위다(lockset 출처 · yaml 주석 · 엔진 기본값). 최종 지위는 아래 `kind: value-status` 블록이 정한다 — 표의 노브마다 블록이 정확히 하나 있어야 린터를 통과한다.

<!-- FACT:value_status -->
서빙 노브 5개 — 노브마다 `kind: value-status` 블록이 **정확히 1개** 있어야 한다(노브·값은 이 표의 글자 그대로 · 값 칸의 `\|` 는 `|` 로 적는다).

| 노브 | 값 | 후보 지위 | 후보 근거 | yaml 주석 | 출처 |
|---|---|---|---|---|---|
| `gpu-memory-utilization` | 0.39 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `max-model-len` | 32768 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `max-num-seqs` | 8 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `kv-cache-memory-bytes` | 9742874549 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `language-model-only` | true | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
<!-- /FACT:value_status -->

> 이 절이 답하는 질문: 각 서빙 노브의 값은 이 셀에서 어떤 지위인가 — 조정됐나, 승계됐나, 음성대조로 일부러 둔 것인가, 엔진 기본값인가, 필요조건인가?

```hint-event
id: V1
kind: value-status
노브: gpu-memory-utilization
값: 0.39
지위: inherited
근거: yaml 주석: 메인 지시로 정한 배포 gmu(gmu_source=hand · 손레버) — 이 계보 문서에 값을 바꿔 잰 기록이 없다. 같은 노드 두 모델 공존용 free-memory 검사값이며 KV 는 클램프가 정한다고 yaml 주석이 적는다.
출처: [ornith15-9b-v031.yaml]
```

```hint-event
id: V2
kind: value-status
노브: max-model-len
값: 32768
지위: inherited
근거: 레시피 헤더 max_model_len=32768 — 이 계보 문서에 값을 바꿔 잰 기록이 없다.
출처: [ornith15-9b-v031.yaml]
```

```hint-event
id: V3
kind: value-status
노브: max-num-seqs
값: 8
지위: inherited
근거: yaml 주석 batch_source=hand-lever(손레버) — 스윕 기록 없음. 측정 레벨 1/2/4/8 이 이 값 안에 들어가고 16 은 이 값 때문에 제외됐다고 testlog 가 적는다.
출처: [ornith15-9b-v031.yaml]
```

```hint-event
id: V4
kind: value-status
노브: kv-cache-memory-bytes
값: 9742874549
지위: inherited
근거: yaml 헤더가 절대 KV 클램프(kv_cache 9.07 GiB)를 "Phase 2 절대 KV 클램프 기준" 으로 적고 앞선 캠페인 lockset 은 kv_source=measured-clamp 다 — 트라이얼로 수렴시킨 값이며 그 이력은 이 태그 계보 4건 밖(앞선 캠페인 서빙 testlog)에 있다. 이 계보 안에는 바꿔 잰 기록이 없어 inherited 로 둔다(계보 밖 기록 기준으로는 tuned 후보).
출처: [ornith15-9b-v031.yaml]
```

```hint-event
id: V5
kind: value-status
노브: language-model-only
값: true
지위: inherited
근거: yaml 에 설정돼 있다 — 이 계보 문서에 켜고 끈 비교 기록이 없다.
출처: [ornith15-9b-v031.yaml]
```

서빙 노브 전부가 승계값이다. 복사하면 위험한 값은 `kv-cache-memory-bytes` 와 `gpu-memory-utilization` 이다 — 둘 다 같은 노드에 두 모델이 공존하는 배치와 이 모델의 가중치에 맞춰 사람이 손으로 정한 값이라 다른 메모리 구성에는 맞지 않는다. 필요조건으로 확인된 노브는 없다.

## 1.4 재현 절차
> 셀 실행 기록에서 기계가 파생한 명령 순서와 단계별 소요다. 소요가 `미관측` 인 단계는 시각 기록이 없다는 뜻이다(0 이 아니다). 그 아래는 블랙박스 이벤트 원장에서 이 셀의 행을 기계가 뽑아 기동 시도로 묶은 것이고, 마지막은 측정한 실행의 엔진 로그가 echo 한 env 다(측정 당시 값의 정본).

<!-- FACT:reproduce -->
| 순서 | 단계 | 성공 판정 | 소요 | 관측 구간(UTC) | 소요 출처 | 명령 출처 |
|---|---|---|---|---|---|---|
| 1 | build | 빌드 OK · 이미지 실재 | 층 창 37분 3초(docker history 첫 층 → 끝 층 CreatedAt · 캐시 재사용 층 포함 — 빌드 소요 아님) | 2026-10-14T01:13:13Z → 2026-10-14T01:50:16Z (layer-window) | docker history --no-trunc --format '{{json .}}' sha256:667eee7bd3b4e84b204ad4887bd811c87dd3537428d026eaeb580ab825d88c8e · 우리 Dockerfile 층 29개의 CreatedAt 첫 층 → 끝 층(캐시 재사용 층은 이전 빌드 시각 — 빌드 소요 아님) | reconstructed(docker-history build-args + compose build) |
| 2 | serve | health 200 + 추론 1회(스모크 판정) | ≤ 56시간 30분 8초(상한)(끝 = 최종 스윕 첫 측정 · 그 전 요청 186건 — 소요 출처 칸) | 2026-10-14T02:16:59Z → 2026-10-16T10:47:07Z (upper) | docs/logs/sub/events/2026-10.jsonl budget_declare(label serve-ornith15-9b-v031) → 첫 측정 시각(output/single/benchlog/sweep_ornith15-9b-v031/lite_raw_ornith15-9b-v031.json measured_utc(호스트 date -u · lite_bench.sh 가 cold 레그 직전에 찍는 측정 시작)) · ⚠ 최종(재실행) 스윕 기준 — 같은 serve 세션 안 최종 스윕 첫 측정 전 completions 요청 186건 관측(상한 시각 2026-10-14T02:20:32Z~2026-10-15T21:59:21Z · 스모크 추론 1회 초과 = 앞선 벤치 실행(끊긴 스윕 등)이 있었을 수 있다 · 출처 output/single/benchlog/sweep_ornith15-9b-v031/lite_engine_head_ornith15-9b-v031.log · output/single/benchlog/sweep_ornith15-9b-v031/lite_engine_ornith15-9b-v031.log 의 POST 접근 줄 × 다음 vLLM 시각 줄(컨테이너 시계 UTC 가정)) | derived(스모크 사용법) |
| 3 | bench | 리포트 발행(+PASS 면 인증서) | 37분 58초(최종 스윕 기준 · 그 전 요청 186건 — 소요 출처 칸) | 2026-10-16T10:47:07Z → 2026-10-16T11:25:05Z (exact) | output/single/benchlog/sweep_ornith15-9b-v031/lite_raw_ornith15-9b-v031.json measured_utc(호스트 date -u · lite_bench.sh 가 cold 레그 직전에 찍는 측정 시작) → sweep_index.generated_utc(호스트 UTC · 두 끝 같은 시계) · ⚠ 최종(재실행) 스윕 기준 — 같은 serve 세션 안 최종 스윕 첫 측정 전 completions 요청 186건 관측(상한 시각 2026-10-14T02:20:32Z~2026-10-15T21:59:21Z · 스모크 추론 1회 초과 = 앞선 벤치 실행(끊긴 스윕 등)이 있었을 수 있다 · 출처 output/single/benchlog/sweep_ornith15-9b-v031/lite_engine_head_ornith15-9b-v031.log · output/single/benchlog/sweep_ornith15-9b-v031/lite_engine_ornith15-9b-v031.log 의 POST 접근 줄 × 다음 vLLM 시각 줄(컨테이너 시계 UTC 가정)) | reconstructed(bench json) |

**빌드 층 시각**(관측 · 소요가 아니라 층 창 — 캐시 재사용 층은 이전 빌드의 시각을 가진다)

- `build` 이미지 층 CreatedAt(docker history): 2026-10-14T01:13:13Z → 2026-10-14T01:50:16Z · 층 29개 · 출처 `docker history --no-trunc --format '{{json .}}' sha256:667eee7bd3b4e84b204ad4887bd811c87dd3537428d026eaeb580ab825d88c8e` — 우리 Dockerfile 층의 CreatedAt 범위 — 캐시 재사용 층은 이전 빌드 시각을 가진다(소요 아님)

**단계별 명령**(기계 파생 — `명령 출처` 가 `derived(…)` 면 사용법에서 파생 · `reconstructed(…)` 면 괄호 안 관측(docker history build-arg · compose build · bench JSON 필드)에서 재구성한 명령이다. 어느 쪽도 실행 원문(로그에 남은 줄)은 아니다)

**1. build**

```sh
# 실린 artifacts/build_recipe/Dockerfile.source-build(이미지를 지은 개정: worktree)를 output/single/Dockerfile.source-build 자리에 두고 컨텍스트에서 짓는다.
docker build -f output/single/Dockerfile.source-build \
  --build-arg BUILD_JOBS=8 \
  --build-arg RAY_VERSION=2.48.0 \
  --build-arg SM12X_PORT=0 \
  --build-arg SRC_DEPS_AUTHORITY=0 \
  --build-arg TORCH_CUDA_ARCH=12.1a \
  --build-arg VLLM_PRETEND_VERSION= \
  --build-arg VLLM_REF=v0.31.0 \
  --build-arg VLLM_REPO=https://github.com/vllm-project/vllm.git \
  -t easy-vllm:0.31.0-cu133-aarch64-source \
  output/single
# 동등 경로(compose): docker compose -f output/single/docker-compose.yaml --env-file output/single/envs/.env.ornith15-9b-v031 build
```

**2. serve**

```sh
bash .claude/skills/upstream-version-watch/scripts/single_serve_up.sh ornith15-9b-v031
```

**3. bench**

```sh
# bench JSON 필드로 복원: backend · model_id→--model · tokenizer_id→--tokenizer · num_prompts · max_concurrency · request_rate · burstiness · 요청당 입출력 길이 = total_*_tokens ÷ completed(나누어떨어질 때만 — 평균이다 · 요청별 길이는 JSON 에 없다) · 나누어떨어지면 `--dataset-name random` 으로 추정했다(측정 도구가 그 꼴로 부른다 — 아래 도구 원문).
# JSON 에 없는 인자(--endpoint · --base-url · --ignore-eos · --num-warmups · --temperature · --seed · --random-range-ratio · --trust-remote-code)는 측정 도구가 정했다 — level_* = run_bench.sh · lite_cold/lite_warm = lite_bench.sh(.claude/skills/adversarial-benchmark/scripts/) · 셀 스윕 순서는 sweep_bench.sh.
# output/single/benchlog/sweep_ornith15-9b-v031/lite_cold_ornith15-9b-v031.json (date 20261016-104726) · ⚠ --random-input-len/--random-output-len = total ÷ completed 평균(chat 템플릿 포함 실측 토큰 · 측정 도구 원문 인자 미관측 — lite_bench.sh 원문 인자와 다를 수 있다)
vllm bench serve --backend openai-chat --model ornith-1.5-9b --tokenizer /app/models/ornith-ai/Ornith-1.5-9B --dataset-name random --random-input-len 524 --random-output-len 128 --num-prompts 1 --max-concurrency 1 --request-rate inf
# output/single/benchlog/sweep_ornith15-9b-v031/lite_warm_ornith15-9b-v031.json (date 20261016-104815) · ⚠ --random-input-len/--random-output-len = total ÷ completed 평균(chat 템플릿 포함 실측 토큰 · 측정 도구 원문 인자 미관측 — lite_bench.sh 원문 인자와 다를 수 있다)
vllm bench serve --backend openai-chat --model ornith-1.5-9b --tokenizer /app/models/ornith-ai/Ornith-1.5-9B --dataset-name random --random-input-len 523 --random-output-len 128 --num-prompts 3 --max-concurrency 1 --request-rate inf
```

**슬롯 → 빌드 컨텍스트 매핑**(빌드 컨텍스트 = `output/<토폴로지>/` · 원천 = 실린 Dockerfile 의 COPY · compose 의 env_file · volumes · command — `—` 는 빌드 컨텍스트 입력이 아니다)

| zip 경로 | 빌드 컨텍스트 자리 | 근거 |
|---|---|---|
| `artifacts/triplet/.env.ornith15-9b-v031` | `envs/.env.ornith15-9b-v031` | compose env_file envs/.env.${CONFIG_FILE}(셀 env) |
| `artifacts/triplet/ornith15-9b-v031.sh` | `configs/ornith15-9b-v031.sh` | compose volumes ./configs:/app/configs |
| `artifacts/triplet/ornith15-9b-v031.yaml` | `configs/ornith15-9b-v031.yaml` | compose volumes ./configs:/app/configs |
| `artifacts/build_recipe/Dockerfile.source-build` | `Dockerfile.source-build` | compose build.dockerfile Dockerfile.source-build(리터럴) · build.context . |
| `artifacts/build_recipe/requirements.txt` | `requirements.txt` | Dockerfile.source-build COPY requirements.txt |
| `artifacts/compose/.env.template` | `.env` | compose 변수치환 프로젝트 .env(${…} 참조) |
| `artifacts/compose/docker-compose.yaml` | `docker-compose.yaml` | compose 파일(빌드 컨텍스트 루트) |

**기동 성공 판정(관측 · 발행 자격)**: health 200 = true · 추론 1회 = true · 근거 `output/single/benchlog/sweep_ornith15-9b-v031/level_01/post_health_ornith15-9b-v031.json` · `output/single/benchlog/sweep_ornith15-9b-v031/level_01/guidellm_ornith15-9b-v031.json`
<!-- /FACT:reproduce -->

<!-- FACT:event_timeline -->
**블랙박스 이벤트 12행**(원장에서 이 셀 label·config 에 맞는 행만 · 시각순 · 기계 발췌 · 이번 캠페인 0행 · 같은 셀 이름 · 앞선 캠페인 12행(내용 칸에 표기))

> **캠페인 경계** 2026-10-16T11:50:00Z(`camp-26101616-cnn-hint` · evidence.event_campaign_boundary): campaigns/camp-26101616-cnn-hint/campaign.yaml declared_utc 2026-10-16T11:50:00Z → 경계 = 앞선 값 2026-10-16T11:50:00Z(이전 행 = 같은 셀 이름 · 앞선 캠페인)

> **원장 관측 범위**(노드별 첫 행 → 마지막 행 · evidence.event_ledger_spans): sub = 2026-06-13T22:25:01Z → 2026-10-16T11:21:58Z(`docs/logs/sub/events/2026-06.jsonl` · `docs/logs/sub/events/2026-07.jsonl` · `docs/logs/sub/events/2026-09.jsonl` · `docs/logs/sub/events/2026-10.jsonl` · `docs/logs/sub/events/thermal.jsonl` · `docs/logs/sub/events/watchdog.jsonl`) — ⚠ sub(마지막 행 2026-10-16T11:21:58Z < 측정 끝 2026-10-16T11:25:05Z): 그 뒤 그 노드의 선언 · 사건은 **관측 범위 밖**이다(없음이 아니다 · 원장 미회수)

| # | UTC | 노드 | kind | label | 내용 | 출처 |
|---|---|---|---|---|---|---|
| 1 | 2026-10-14T02:10:48Z | sub | `budget_declare` | `serve-ornith15-9b-v031` | 예산 선언(선언값 — 측정 아님) · floor_mib=78938 · weights_mib=17947 · kv_mib=9291 · overhead_mib=18432 · mem_total_mib=124608 · 기동 창 1/2: budget_renew 1회 · 닫힘 budget_clear 2026-10-14T02:16:55Z(선언 후 6m07s) · measured_utc 2026-10-16T11:25:05Z 는 이 창 밖(다른 기동 · 측정 전) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T11:50:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:264 |
| 2 | 2026-10-14T02:10:48Z | sub | `serve_start` | — | serve_start · model=ornith-1.5-9b · tensor_parallel_size=1 · topology=single · session=serve-ornith15-9b-v031-20261014T021048Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T11:50:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:265 |
| 3 | 2026-10-14T02:10:49Z | sub | `budget_renew` | `serve-ornith15-9b-v031` | budget_renew · floor_mib=78938 · ttl_s=7200 · remaining_before_s=7199 · 창 시작 2026-10-14T02:10:48Z 후 0m01s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T11:50:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:266 |
| 4 | 2026-10-14T02:10:49Z | sub | `budget_honored` | `serve-ornith15-9b-v031` | budget_honored · floor_mib=78938 · arm_ceiling_mib=75866 · remaining_s=7199 · 창 시작 2026-10-14T02:10:49Z 후 0m00s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T11:50:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:542 |
| 5 | 2026-10-14T02:16:55Z | sub | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → serve-ornith15-9b-v031 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-14T02:10:48Z 후 6m07s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T11:50:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:267 |
| 6 | 2026-10-14T02:16:55Z | sub | `serve_stop` | — | serve_stop · model=ornith-1.5-9b · duration_s=367 · session=serve-ornith15-9b-v031-20261014T021048Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T11:50:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:268 |
| 7 | 2026-10-14T02:16:56Z | sub | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → serve-ornith15-9b-v031 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-14T02:10:49Z 후 6m07s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T11:50:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:543 |
| 8 | 2026-10-14T02:16:59Z | sub | `budget_declare` | `serve-ornith15-9b-v031` | 예산 선언(선언값 — 측정 아님) · floor_mib=78938 · weights_mib=17947 · kv_mib=9291 · overhead_mib=18432 · mem_total_mib=124608 · 기동 창 2/2: budget_renew 1회 · 닫힘 replaced(budget_declare) 2026-10-14T02:26:25Z(선언 후 9m26s) · measured_utc 2026-10-16T11:25:05Z 는 이 창 밖(다른 기동 · 측정 전) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T11:50:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:270 |
| 9 | 2026-10-14T02:16:59Z | sub | `serve_start` | — | serve_start · model=ornith-1.5-9b · tensor_parallel_size=1 · topology=single · session=serve-ornith15-9b-v031-20261014T021659Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T11:50:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:271 |
| 10 | 2026-10-14T02:17:00Z | sub | `budget_renew` | `serve-ornith15-9b-v031` | budget_renew · floor_mib=78938 · ttl_s=7200 · remaining_before_s=7199 · 창 시작 2026-10-14T02:16:59Z 후 0m01s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T11:50:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:272 |
| 11 | 2026-10-14T02:17:00Z | sub | `budget_honored` | `serve-ornith15-9b-v031` | budget_honored · floor_mib=78938 · arm_ceiling_mib=75866 · remaining_s=7199 · 창 시작 2026-10-14T02:17:00Z 후 0m00s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T11:50:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:544 |
| 12 | 2026-10-14T02:26:54Z | sub | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → serve-ornith15-9b-v031 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-14T02:17:00Z 후 9m54s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T11:50:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:545 |

**기동 시도**(예산 선언 `budget_declare` 마다 한 시도 · 같은 노드의 이어지는 행과 선언 없는 노드(엔진 로그)의 그 시각 행을 묶었다 · 사살 · 트립 · 거부는 닫힘과 따로 센다 · `measurement` 행은 측정 기록이라 시도로 세지 않는다(위 표에만) — 해석은 저작자 몫)

_이번 캠페인(경계 2026-10-16T11:50:00Z 이후)의 기동 시도 미관측 — 아래는 같은 셀 이름의 앞선 캠페인 행뿐이다._

**같은 셀 이름 · 앞선 캠페인의 기동 2회**(캠페인 경계 2026-10-16T11:50:00Z 이전 — 라벨이 같아 원장에서 함께 묶였을 뿐 이 셀의 시도로 세지 않는다 · 행 전문 = 01 §1.4)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 앞선-1 | sub | `serve-ornith15-9b-v031` | 2026-10-14T02:10:48Z | 1회 · 첫 2026-10-14T02:10:49Z | 없음 | `budget_clear` 2026-10-14T02:16:55Z | 4 |
| 앞선-2 | sub | `serve-ornith15-9b-v031` | 2026-10-14T02:16:59Z | 1회 · 첫 2026-10-14T02:17:00Z | 없음 | —(닫힘 행 없음) | 3 |
<!-- /FACT:event_timeline -->

<!-- FACT:measurement_env -->
_측정 실행의 env 관측 없음 — 측정 엔진 로그에서 env echo 줄(`<KEY> set by environment to <VALUE>` 등)을 찾지 못했다(evidence.measurement_env_observed · 부재 ≠ 미설정)._
<!-- /FACT:measurement_env -->

> 이 절이 답하는 질문: 수신자가 이 zip 만으로 이 셀을 다시 띄우려면 어떤 순서로 무엇을 하고, 각 단계의 성공을 무엇으로 판정하며, 얼마나 기다려야 하는가?

(a) 사전 준비 — 가중치는 관리 NAS 에서 읽는다(체크포인트 Ornith-1.5-9B, 리비전은 00 §0.2). 필수 env 는 `artifacts/triplet/.env.ornith15-9b-v031`(포트 8001, 이미지 태그, 모델 이름)에 있다. 측정 당시 env 값은 측정 실행 엔진 로그에서 찾지 못했다(위 "측정 실행의 env 관측" 은 비어 있다).
(b) 빌드 — 위 build 단계 명령. docker history 층 창이 37분 남짓이나 이것은 빌드 소요가 아니다(캐시 층 포함). 소스빌드 이미지는 이 셀 이전에 이미 있었다.
(c) 기동 — 위 serve 단계 명령으로 기동하고 `/health` 200 과 추론 1회를 판정으로 쓴다. 기동 판정 근거는 측정 post_health 와 guidellm JSON 이다(위 "기동 성공 판정"). 기동 소요: 위 표의 serve 소요는 상한일 뿐이다. 기동 로그가 구간을 기록한다 — 가중치 로드 104.15 초(Model loading 107.49 초), init engine 56.21 초, Starting vLLM server 10-14 02:20:11(원장 serve_start 02:16:59Z 이후 약 3분 12초; 로그 시각과 원장 시각을 같은 시계로 본 추정). 이번 캠페인의 기동 시도는 미관측이고, 같은 셀 이름의 앞선 캠페인 기동이 2회 있었다(위 표 · 이 셀의 시도로 세지 않는다).
(d) 호스트 메모리 예산 선언 — 앞선 캠페인 행에 선언값이 있다(위 이벤트 표의 floor·weights·kv·overhead 선언값). 선언이지 측정이 아니며 엔진 로그 실측(가중치 GiB)과 나란히 읽는다(03 §3.1 lite.engine_weights_gib).
(e) 측정 명령 — 위 bench 단계의 명령은 bench JSON 에서 재구성한 vllm bench serve 형태이나 이 셀의 full 측정은 GuideLLM 이다. 실제 명령 형태와 도구가 정한 인자는 03 §3.4 에 있다.
(f) 런타임 경로 — 기동 로그가 기록한다: 통신 backend=nccl(world_size=1) · Using FLASH_ATTN attention backend · Using FlashInfer GDN prefill kernel · GDN decode kernel: cuda · Mamba cache mode is set to 'align' · mm_prefix attention 모드는 multimodal 입력이 설정으로 꺼져 비활성 · Setting attention block size to 528 tokens · 체크포인트 파일시스템 CIFS 에서 Auto-prefetch 비활성. 계보 문서 4건에는 없고 서빙 세션 엔진 로그에 있다.

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

### 가중치 획득
관리 NAS 마운트(읽기 전용)에 있는 체크포인트를 쓴다. 모델 다운로드는 사람의 승인 게이트다. 받는 명령과 크기는 이 계보 문서에 없다(미기록) — 체크포인트는 NAS 의 `<manifest.nas_model_path>/…` 아래에 이미 있었다고 00 §0.2 의 출처 칸이 보인다.

### 스테이징(PLE 등)
해당 없음 — 이 모델은 PLE 키가 부재 관측이다(00 §0.9 후보 표).

### 데이터셋 준비
CNN/DailyMail 3.0.0 test 원본(parquet)을 공유 NAS 에서 읽고 sha256 을 03 §3.3 의 값과 대조한다(testlog_26101620_p10_cnn_fullbench §전제 — "NAS 원본 sha256 = 04e322d2…ffe4e → 지시값과 일치"). 프리셋은 `output/single/configs/cnn_preset.yaml` 에 두고 `--preset <경로>` 로 지정한다(W1).

### 네트워크가 필요한 단계
베이스 이미지 pull·pip/wheel·가중치 다운로드는 빌드 단계 필요이며, 오프라인 대체는 이 계보에 기록이 없다(추론 — 실행 검증 ✗).
