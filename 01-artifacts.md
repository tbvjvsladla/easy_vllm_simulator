# 01 · 산출물 — 무엇이 실제로 쓰였고, 어떻게 다시 띄우는가

## 1.1 적용 판정 표
> 기계가 적용 증거(빌드 원장 · 원장이 없으면 라벨된 재구성) × 파일 × 선언을 대사한 결과다. `artifacts/` 에는 **적용된 것만** 실렸다 — 빌드 때 스스로 skip 된 패치와 쓰이지 않은 평면의 파일은 표에만 남고 싣지 않는다.

<!-- FACT:slots -->
| 슬롯 | 실림 | 파일 | 신호 | 적용 증거 | 사유 |
|---|---|---|---|---|---|
| `build_patch_post` | 안 실림 | — | 1-signal | none:no-build-ledger | 적용 판정 없음 — 원장 부재 — 라벨된 재구성: applied 0 · skip 판정 = docker-history+gate(기계 파생) |
| `build_patch_pre` | 안 실림 | — | 1-signal | none:no-build-ledger | 적용 판정 없음 — 원장 부재 — 라벨된 재구성: applied 0 · skip 판정 = docker-history+gate(기계 파생) |
| `build_recipe` | 실림 | `artifacts/build_recipe/Dockerfile.source-build` · `artifacts/build_recipe/requirements.txt` | 2-signal | file:image-history:docker history build-arg 관측(VLLM_REF=v0.31.0 · VLLM_VERSION 없음 → 소스빌드 Dockerfile) — 셀 env 선택자 없음 · output/single/docker-compose.yaml 리터럴 build.dockerfile: Dockerfile.source-build(선택자 변수 없음 — compose build 는 늘 Dockerfile.source-build 를 짓는다 · 이 이미지를 지은 증거 아님) | 이 셀의 vLLM v0.31.0 소스빌드 레시피다. 1.1 표가 docker history 16/16 대조와 이미지 안 COPY 대상 sha256 일치로 쓰인 바이트임을 확인한다(관측). 이 Dockerfile 에는 빌드 원장 스탠자가 없다(W3) — 그래서 적용 판정은 재구성이다. 패치 이식 출처나 hunk 구성은 이 계보 문서에 없다. 폐기 조건: 다른 빌드 트랙(wheel) 사용 시. |
| `compose` | 실림 | `artifacts/compose/.env.template` · `artifacts/compose/docker-compose.yaml` | 2-signal | file:output/single/docker-compose.yaml | single 서빙 경로(compose·env 형상)이며 측정 당시 개정(커밋 80284db86dd7)이다. 빌드·기동 경로가 읽는 입력이다. `arm_patch.sh` 는 런타임 패치 파일이 없어 불해당이다. |
| `fork_pin` | 안 실림 | — | 1-signal | none:.claude/policies/arch_variant_ledger.json | 셀 env 에 VARIANT 줄 없음 = stock(workflow.md §변종 좌표의 거처)(기계 파생) |
| `runtime_patch` | 안 실림 | — | 1-signal | none:output/single/configs/lfm25-26b-v031_patch.py | output/single/configs/lfm25-26b-v031_patch.py 없음 — arm_patch.sh 는 no-op(런타임 패치 불해당)(기계 파생) |
| `triplet` | 실림 | `artifacts/triplet/.env.lfm25-26b-v031` · `artifacts/triplet/lfm25-26b-v031.sh` · `artifacts/triplet/lfm25-26b-v031.yaml` | 2-signal | file:output/single/configs/lfm25-26b-v031.yaml | 이 셀의 서빙 설정 3종이다. yaml 은 서빙 노브 전부(§1.3), .sh 는 파서 인자(tool-call-parser · reasoning-parser), .env 는 포트·이미지 태그·모델 이름을 가진다. 측정이 이 config 로 돌았다는 것은 lite raw 와 엔진 로그가 같은 모델 경로·max_model_len 을 기록한 데서 확인된다. |

**레시피 경고 · 실린 리비전**(기계 관측 — 수신자 빌드가 여기서 갈라질 수 있다)

- `build_recipe` `output/single/Dockerfile.source-build` 실린 리비전: 리비전 미특정 · 선택 방법 `worktree(측정 이미지 docker history 의 RUN·COPY 와 전수 일치)` · docker history 대조 16/16 · 실린 바이트 `worktree` · 쓰인 바이트임을 관측으로 확인 예
- `build_recipe` `output/single/requirements.txt` 실린 리비전: 리비전 미특정 · 선택 방법 `worktree(image-probe: 이미지 안 COPY 대상 바이트와 sha256 일치)` · docker history 대조 없음 · 실린 바이트 `worktree` · 쓰인 바이트임을 관측으로 확인 예
- `compose` `output/single/docker-compose.yaml` 실린 리비전: 커밋 `80284db86dd7` · 선택 방법 `worktree(mtime 2026-10-14T01:13:06Z ≤ 측정 2026-10-16T14:08:27Z(certificate.measured_utc) · 바이트 = 측정 전 마지막 커밋 80284db86dd7)` · docker history 대조 없음 · 실린 바이트 `worktree` · 근거 `mtime≤measured` · 쓰인 바이트임을 관측으로 확인 예

**싣지 않은 파일**(슬롯별 사유 — 빌드 때 skip 된 패치는 아래 적용 집합 표에 있다)

- `build_patch_post` known-non-patch 1건: `.gitkeep`
- `build_patch_pre` known-non-patch 1건: `.gitkeep`
- `compose` `arm_patch.sh` — `not-armed(output/single/configs/lfm25-26b-v031_patch.py 없음 — arm_patch.sh 는 no-op)`

**파일별 검증 · 관련성**(`generated-unverified` = 실행 검증되지 않은 생성물 — 파일 첫 줄 경고 · PAYLOAD 파일 기록과 같은 표시 · 관련성 = ① 패치 선언 대상 × 모델 config ② 엔진 로그 발화 서명 — 둘 다 관측일 때만 required/inactive-inferred)

| 슬롯 | 파일 | 검증 | 검증 근거 | 관련성 | 관련성 근거 |
|---|---|---|---|---|---|
| `build_recipe` | `artifacts/build_recipe/Dockerfile.source-build` | `verified` · cell-run | 쓰인 레시피(image-history:docker history build-arg 관측(VLLM_REF=v0.31.0 · VLLM_VERSION 없음 → 소스빌드 Dockerfile) — 셀 env 선택자 없음 · output/single/docker-compose.yaml 리터럴 build.dockerfile: Dockerfile.source-build(선택자 변수 없음 — compose build 는 늘 Dockerfile.source-build 를 짓는다 · 이 이미지를 지은 증거 아님)) — 개정 선택 slots.build_recipe.evidence.selected_revisions | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `build_recipe` | `artifacts/build_recipe/requirements.txt` | `verified` · cell-run | 쓰인 레시피(image-history:docker history build-arg 관측(VLLM_REF=v0.31.0 · VLLM_VERSION 없음 → 소스빌드 Dockerfile) — 셀 env 선택자 없음 · output/single/docker-compose.yaml 리터럴 build.dockerfile: Dockerfile.source-build(선택자 변수 없음 — compose build 는 늘 Dockerfile.source-build 를 짓는다 · 이 이미지를 지은 증거 아님)) — 개정 선택 slots.build_recipe.evidence.selected_revisions | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/.env.template` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/docker-compose.yaml` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/.env.lfm25-26b-v031` | `verified` · cell-run | 셀 트리플렛 — 측정 대조 없음 | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/lfm25-26b-v031.sh` | `verified` · cell-run | 셀 트리플렛 — 측정 대조 없음 | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/lfm25-26b-v031.yaml` | `verified` · cell-run | 셀 트리플렛 — 측정 대조 없음 | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
<!-- /FACT:slots -->

<!-- FACT:applied_set -->
**적용 집합** — 상태 `unobservable` · 출처 `no-build-ledger` · 원장 없음: 아래 결과는 **라벨된 재구성**이다(관측 아님)

_빌드 패치 없음 — 이 이미지에 빌드 패치 슬롯 입력이 없다._

**원장 탐침**(순위: attestation → 메인 로컬 이미지 → 재구성)

| 순위 | 결과 | 대상 |
|---|---|---|
| attestation | absent | output/single/benchlog/attestation_lfm25-26b-v031.json(부재) |
| local-image | skipped(node=sub — 서브 스캔 ✗ · 문서기반 회수만) | — |
| reconstruction | done | — |
| image-probe | done | easy-vllm:0.31.0-cu133-aarch64-source |

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
이 셀의 vLLM v0.31.0 소스빌드 레시피다. 1.1 표가 docker history 16/16 대조와 이미지 안 COPY 대상 sha256 일치로 쓰인 바이트임을 확인한다(관측). 이 Dockerfile 에는 빌드 원장 스탠자가 없다(W3) — 그래서 적용 판정은 재구성이다. 패치 이식 출처나 hunk 구성은 이 계보 문서에 없다. 폐기 조건: 다른 빌드 트랙(wheel) 사용 시.

### compose (docker-compose.yaml · .env.template)
single 서빙 경로(compose·env 형상)이며 측정 당시 개정(커밋 80284db86dd7)이다. 빌드·기동 경로가 읽는 입력이다. `arm_patch.sh` 는 런타임 패치 파일이 없어 불해당이다.

### triplet (lfm25-26b-v031.yaml · .sh · .env)
이 셀의 서빙 설정 3종이다. yaml 은 서빙 노브 전부(§1.3), .sh 는 파서 인자(tool-call-parser · reasoning-parser), .env 는 포트·이미지 태그·모델 이름을 가진다. 측정이 이 config 로 돌았다는 것은 lite raw 와 엔진 로그가 같은 모델 경로·max_model_len 을 기록한 데서 확인된다.

### 싣지 않은 슬롯
build_patch_pre/post — 입력이 없어 불해당(원장 부재라 적용 판정은 재구성 `unobservable`). runtime_patch — `lfm25-26b-v031_patch.py` 부재. fork_pin — VARIANT 줄이 없어 stock.

## 1.3 값의 지위표
> 서빙 설정의 노브 **전부**와 기계가 모은 후보 지위다(lockset 출처 · yaml 주석 · 엔진 기본값). 최종 지위는 아래 `kind: value-status` 블록이 정한다 — 표의 노브마다 블록이 정확히 하나 있어야 린터를 통과한다.

<!-- FACT:value_status -->
서빙 노브 4개 — 노브마다 `kind: value-status` 블록이 **정확히 1개** 있어야 한다(노브·값은 이 표의 글자 그대로 · 값 칸의 `\|` 는 `|` 로 적는다).

| 노브 | 값 | 후보 지위 | 후보 근거 | yaml 주석 | 출처 |
|---|---|---|---|---|---|
| `gpu-memory-utilization` | 0.39 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `max-model-len` | 32768 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `max-num-seqs` | 8 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `kv-cache-memory-bytes` | 40694815129 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
<!-- /FACT:value_status -->

> 이 절이 답하는 질문: 각 서빙 노브의 값은 이 셀에서 어떤 지위인가 — 조정됐나, 승계됐나, 음성대조로 일부러 둔 것인가, 엔진 기본값인가, 필요조건인가?

```hint-event
id: V1
kind: value-status
노브: gpu-memory-utilization
값: 0.39
지위: inherited
근거: yaml 주석 C2-1 은 gmu_source=hand 라 적지만 같은 yaml 의 explorer 각인 줄은 gmu_source=target_gmu 라 적는다 — 두 표기가 갈린다. 어느 쪽이든 이 계보 문서에 값을 바꿔 잰 기록은 없다. 같은 노드 두 모델 공존용 free-memory 검사값이며 KV 는 클램프가 정한다고 yaml 주석이 적는다.
출처: [lfm25-26b-v031.yaml]
```

```hint-event
id: V2
kind: value-status
노브: max-model-len
값: 32768
지위: inherited
근거: 레시피 헤더 max_model_len=32768 — 이 계보 문서에 값을 바꿔 잰 기록이 없다.
출처: [lfm25-26b-v031.yaml]
```

```hint-event
id: V3
kind: value-status
노브: max-num-seqs
값: 8
지위: inherited
근거: yaml 주석 batch_source=hand-lever(손레버) — 스윕 기록 없음.
출처: [lfm25-26b-v031.yaml]
```

```hint-event
id: V4
kind: value-status
노브: kv-cache-memory-bytes
값: 40694815129
지위: inherited
근거: yaml 주석: 클램프 37.9 GiB = 천장 47.68 − weights 5.05 − overhead 실측 ≈4.7, kv_source=hand(손레버). 이 계보 문서에 클램프를 바꿔 잰 기록이 없다.
출처: [lfm25-26b-v031.yaml]
```

서빙 노브 전부가 승계값이다. 복사하면 위험한 값은 `kv-cache-memory-bytes` 와 `gpu-memory-utilization` 이다 — kv-cache-memory-bytes 는 yaml 주석이 kv_source=hand(손레버)로 적은 값이고, gpu-memory-utilization 은 주석(hand)과 explorer 각인(target_gmu)이 갈리며(V1) 주석은 47.7GiB÷호스트 121.7 로 같은 노드 두 모델 공존용이라 적는다. 둘 다 이 배치에 맞춘 값이라 다른 메모리 구성에는 맞지 않을 수 있다. 필요조건으로 확인된 노브는 없다.

## 1.4 재현 절차
> 셀 실행 기록에서 기계가 파생한 명령 순서와 단계별 소요다. 소요가 `미관측` 인 단계는 시각 기록이 없다는 뜻이다(0 이 아니다). 그 아래는 블랙박스 이벤트 원장에서 이 셀의 행을 기계가 뽑아 기동 시도로 묶은 것이고, 마지막은 측정한 실행의 엔진 로그가 echo 한 env 다(측정 당시 값의 정본).

<!-- FACT:reproduce -->
| 순서 | 단계 | 성공 판정 | 소요 | 관측 구간(UTC) | 소요 출처 | 명령 출처 |
|---|---|---|---|---|---|---|
| 1 | build | 빌드 OK · 이미지 실재 | 층 창 37분 3초(docker history 첫 층 → 끝 층 CreatedAt · 캐시 재사용 층 포함 — 빌드 소요 아님) | 2026-10-14T01:13:13Z → 2026-10-14T01:50:16Z (layer-window) | docker history --no-trunc --format '{{json .}}' easy-vllm:0.31.0-cu133-aarch64-source · 우리 Dockerfile 층 29개의 CreatedAt 첫 층 → 끝 층(캐시 재사용 층은 이전 빌드 시각 — 빌드 소요 아님) | reconstructed(docker-history build-args + compose build) |
| 2 | serve | health 200 + 추론 1회(스모크 판정) | ≤ 59시간 16분 37초(상한) | 2026-10-14T02:51:50Z → 2026-10-16T14:08:27Z (upper) | docs/logs/sub/events/2026-10.jsonl budget_declare(label serve-lfm25-26b-v031) → 첫 측정 시각(output/single/benchlog/lite_publish/lfm25-26b-v031_20261016T140827Z/lite_raw_lfm25-26b-v031.json measured_utc(= bench_report 머리 생성일 · lite_bench.sh 가 cold 레그 직전(부하 직전)에 찍는 측정 **시작** 시각)) | derived(스모크 사용법) |
| 3 | bench | 리포트 발행(+PASS 면 인증서) | ≈ 0분 36초(두 끝 시계 다름 — 컨테이너 date · 호스트 UTC · 근사) | 2026-10-16T14:08:27Z → 2026-10-16T14:09:03Z (clock-mixed) | 시작 = output/single/benchlog/lite_publish/lfm25-26b-v031_20261016T140827Z/lite_raw_lfm25-26b-v031.json measured_utc(= bench_report 머리 생성일 · lite_bench.sh 가 cold 레그 직전(부하 직전)에 찍는 측정 **시작** 시각) → 끝 = output/single/benchlog/lite_publish/lfm25-26b-v031_20261016T140827Z/lite_warm_lfm25-26b-v031.json date(마지막 레그 결과 저장 · 컨테이너 시계 = UTC 가정) | reconstructed(bench json) |

**빌드 층 시각**(관측 · 소요가 아니라 층 창 — 캐시 재사용 층은 이전 빌드의 시각을 가진다)

- `build` 이미지 층 CreatedAt(docker history): 2026-10-14T01:13:13Z → 2026-10-14T01:50:16Z · 층 29개 · 출처 `docker history --no-trunc --format '{{json .}}' easy-vllm:0.31.0-cu133-aarch64-source` — 우리 Dockerfile 층의 CreatedAt 범위 — 캐시 재사용 층은 이전 빌드 시각을 가진다(소요 아님)

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
# 동등 경로(compose): docker compose -f output/single/docker-compose.yaml --env-file output/single/envs/.env.lfm25-26b-v031 build
```

**2. serve**

```sh
bash .claude/skills/upstream-version-watch/scripts/single_serve_up.sh lfm25-26b-v031
```

**3. bench**

```sh
# bench JSON 필드로 복원: backend · model_id→--model · tokenizer_id→--tokenizer · num_prompts · max_concurrency · request_rate · burstiness · 요청당 입출력 길이 = total_*_tokens ÷ completed(나누어떨어질 때만 — 평균이다 · 요청별 길이는 JSON 에 없다) · 나누어떨어지면 `--dataset-name random` 으로 추정했다(측정 도구가 그 꼴로 부른다 — 아래 도구 원문).
# JSON 에 없는 인자(--endpoint · --base-url · --ignore-eos · --num-warmups · --temperature · --seed · --random-range-ratio · --trust-remote-code)는 측정 도구가 정했다 — level_* = run_bench.sh · lite_cold/lite_warm = lite_bench.sh(.claude/skills/adversarial-benchmark/scripts/) · 셀 스윕 순서는 sweep_bench.sh.
# 호출(재구성 — lite_raw config_name · topology · backend + 바인딩된 경량 리포트 = --publish-report · 원문 호출 줄 아님): bash .claude/skills/adversarial-benchmark/scripts/lite_bench.sh lfm25-26b-v031 --topology single --backend openai-chat --publish-report
# 측정 창: 시작 2026-10-16T14:08:27Z(output/single/benchlog/lite_publish/lfm25-26b-v031_20261016T140827Z/lite_raw_lfm25-26b-v031.json measured_utc(= bench_report 머리 생성일 · lite_bench.sh 가 cold 레그 직전(부하 직전)에 찍는 측정 **시작** 시각)) → 끝 2026-10-16T14:09:03Z(output/single/benchlog/lite_publish/lfm25-26b-v031_20261016T140827Z/lite_warm_lfm25-26b-v031.json date(마지막 레그 결과 저장 · 컨테이너 시계 = UTC 가정))
# output/single/benchlog/lite_publish/lfm25-26b-v031_20261016T140827Z/lite_cold_lfm25-26b-v031.json (date 20261016-140840) · ⚠ --random-input-len/--random-output-len = total ÷ completed 평균(chat 템플릿 포함 실측 토큰 · 측정 도구 원문 인자 미관측 — lite_bench.sh 원문 인자와 다를 수 있다)
vllm bench serve --backend openai-chat --model lfm2.5-2.6b --tokenizer /app/models/Liquid-AI/LFM2.5-2.6B --dataset-name random --random-input-len 522 --random-output-len 128 --num-prompts 1 --max-concurrency 1 --request-rate inf
# output/single/benchlog/lite_publish/lfm25-26b-v031_20261016T140827Z/lite_warm_lfm25-26b-v031.json (date 20261016-140903) · ⚠ --random-input-len/--random-output-len = total ÷ completed 평균(chat 템플릿 포함 실측 토큰 · 측정 도구 원문 인자 미관측 — lite_bench.sh 원문 인자와 다를 수 있다)
vllm bench serve --backend openai-chat --model lfm2.5-2.6b --tokenizer /app/models/Liquid-AI/LFM2.5-2.6B --dataset-name random --random-input-len 522 --random-output-len 128 --num-prompts 3 --max-concurrency 1 --request-rate inf
```

**슬롯 → 빌드 컨텍스트 매핑**(빌드 컨텍스트 = `output/<토폴로지>/` · 원천 = 실린 Dockerfile 의 COPY · compose 의 env_file · volumes · command — `—` 는 빌드 컨텍스트 입력이 아니다)

| zip 경로 | 빌드 컨텍스트 자리 | 근거 |
|---|---|---|
| `artifacts/triplet/.env.lfm25-26b-v031` | `envs/.env.lfm25-26b-v031` | compose env_file envs/.env.${CONFIG_FILE}(셀 env) |
| `artifacts/triplet/lfm25-26b-v031.sh` | `configs/lfm25-26b-v031.sh` | compose volumes ./configs:/app/configs |
| `artifacts/triplet/lfm25-26b-v031.yaml` | `configs/lfm25-26b-v031.yaml` | compose volumes ./configs:/app/configs |
| `artifacts/build_recipe/Dockerfile.source-build` | `Dockerfile.source-build` | compose build.dockerfile Dockerfile.source-build(리터럴) · build.context . |
| `artifacts/build_recipe/requirements.txt` | `requirements.txt` | Dockerfile.source-build COPY requirements.txt |
| `artifacts/compose/.env.template` | `.env` | compose 변수치환 프로젝트 .env(${…} 참조) |
| `artifacts/compose/docker-compose.yaml` | `docker-compose.yaml` | compose 파일(빌드 컨텍스트 루트) |

**기동 성공 판정(관측 · 발행 자격)**: health 200 = true · 추론 1회 = true · 근거 `output/single/benchlog/lite_publish/lfm25-26b-v031_20261016T140827Z/lite_raw_lfm25-26b-v031.json` · `output/single/benchlog/lite_publish/lfm25-26b-v031_20261016T140827Z/lite_warm_lfm25-26b-v031.json`
<!-- /FACT:reproduce -->

<!-- FACT:event_timeline -->
**블랙박스 이벤트 124행**(원장에서 이 셀 label·config 에 맞는 행만 · 시각순 · 기계 발췌 · 이번 캠페인 0행 · 같은 셀 이름 · 앞선 캠페인 124행(내용 칸에 표기))

> **캠페인 경계** 2026-10-16T14:03:30Z(`camp-26101623-baseline` · evidence.event_campaign_boundary): campaigns/camp-26101623-baseline/campaign.yaml declared_utc 2026-10-16T14:03:30Z → 경계 = 앞선 값 2026-10-16T14:03:30Z(이전 행 = 같은 셀 이름 · 앞선 캠페인)

> **원장 관측 범위**(노드별 첫 행 → 마지막 행 · evidence.event_ledger_spans): sub = 2026-06-13T22:25:01Z → 2026-10-16T13:51:58Z(`docs/logs/sub/events/2026-06.jsonl` · `docs/logs/sub/events/2026-07.jsonl` · `docs/logs/sub/events/2026-09.jsonl` · `docs/logs/sub/events/2026-10.jsonl` · `docs/logs/sub/events/thermal.jsonl` · `docs/logs/sub/events/watchdog.jsonl`) — sub(마지막 행 2026-10-16T13:51:58Z < 측정 끝 2026-10-16T14:09:03Z): 마지막 행 뒤 행 없음(이 노드의 로컬 원장 · 조회 시각까지 관측 · 미회수 아님 — 행은 사건 · 6시간 갱신 때만 쓰인다)

| # | UTC | 노드 | kind | label | 내용 | 출처 |
|---|---|---|---|---|---|---|
| 1 | 2026-10-14T02:51:50Z | sub | `budget_declare` | `serve-lfm25-26b-v031` | 예산 선언(선언값 — 측정 아님) · floor_mib=38862 · weights_mib=5144 · kv_mib=38809 · overhead_mib=41793 · mem_total_mib=124608 · 기동 창 1/1: budget_renew 119회 · 닫힘 미관측(창이 열린 채 원장이 끝난다) · measured_utc 2026-10-16T14:08:27Z 는 이 창의 관측 범위 밖(창이 열린 채 스트림이 2026-10-16T13:51:58Z 에서 끝난다 — 이 측정의 기동으로 귀속하지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:279 |
| 2 | 2026-10-14T02:51:50Z | sub | `serve_start` | — | serve_start · model=lfm2.5-2.6b · tensor_parallel_size=1 · topology=single · session=serve-lfm25-26b-v031-20261014T025150Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:280 |
| 3 | 2026-10-14T02:51:50Z | sub | `budget_honored` | `serve-lfm25-26b-v031` | budget_honored · floor_mib=38862 · arm_ceiling_mib=35790 · remaining_s=7200 · 창 시작 2026-10-14T02:51:50Z 후 0m00s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:551 |
| 4 | 2026-10-14T02:51:51Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=7199 · 창 시작 2026-10-14T02:51:50Z 후 0m01s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:281 |
| 5 | 2026-10-14T02:52:50Z | sub | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=55494 · rate_mib_s=23142 · max_rate_mib_s=17801 · legacy_rule_would_trip=true · 라벨 없음 → serve-lfm25-26b-v031 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-14T02:51:50Z 후 1m00s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:552 |
| 6 | 2026-10-14T03:21:51Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 30m01s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:282 |
| 7 | 2026-10-14T03:51:51Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 1h00m01s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:283 |
| 8 | 2026-10-14T04:21:51Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 1h30m01s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:284 |
| 9 | 2026-10-14T04:51:51Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 2h00m01s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:285 |
| 10 | 2026-10-14T05:21:52Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5399 · 창 시작 2026-10-14T02:51:50Z 후 2h30m02s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:286 |
| 11 | 2026-10-14T05:51:52Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 3h00m02s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:287 |
| 12 | 2026-10-14T06:21:52Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 3h30m02s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:288 |
| 13 | 2026-10-14T06:51:52Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 4h00m02s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:289 |
| 14 | 2026-10-14T07:21:52Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 4h30m02s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:290 |
| 15 | 2026-10-14T07:51:52Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 5h00m02s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:291 |
| 16 | 2026-10-14T08:21:52Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 5h30m02s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:292 |
| 17 | 2026-10-14T08:51:52Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 6h00m02s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:293 |
| 18 | 2026-10-14T09:21:52Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 6h30m02s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:294 |
| 19 | 2026-10-14T09:51:52Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 7h00m02s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:295 |
| 20 | 2026-10-14T10:21:52Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 7h30m02s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:296 |
| 21 | 2026-10-14T10:51:52Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 8h00m02s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:297 |
| 22 | 2026-10-14T11:21:52Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 8h30m02s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:298 |
| 23 | 2026-10-14T11:51:52Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 9h00m02s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:299 |
| 24 | 2026-10-14T12:21:52Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 9h30m02s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:300 |
| 25 | 2026-10-14T12:51:52Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 10h00m02s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:301 |
| 26 | 2026-10-14T13:21:53Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5399 · 창 시작 2026-10-14T02:51:50Z 후 10h30m03s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:302 |
| 27 | 2026-10-14T13:51:53Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 11h00m03s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:303 |
| 28 | 2026-10-14T14:21:53Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 11h30m03s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:304 |
| 29 | 2026-10-14T14:51:53Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 12h00m03s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:305 |
| 30 | 2026-10-14T15:21:53Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 12h30m03s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:309 |
| 31 | 2026-10-14T15:51:53Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 13h00m03s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:310 |
| 32 | 2026-10-14T16:21:53Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 13h30m03s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:311 |
| 33 | 2026-10-14T16:51:53Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 14h00m03s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:312 |
| 34 | 2026-10-14T17:21:53Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 14h30m03s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:313 |
| 35 | 2026-10-14T17:51:53Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 15h00m03s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:314 |
| 36 | 2026-10-14T18:21:53Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 15h30m03s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:315 |
| 37 | 2026-10-14T18:51:53Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 16h00m03s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:316 |
| 38 | 2026-10-14T19:21:53Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 16h30m03s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:317 |
| 39 | 2026-10-14T19:51:53Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 17h00m03s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:318 |
| 40 | 2026-10-14T20:21:53Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 17h30m03s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:319 |
| 41 | 2026-10-14T20:51:53Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 18h00m03s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:320 |
| 42 | 2026-10-14T21:21:53Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 18h30m03s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:321 |
| 43 | 2026-10-14T21:51:54Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5399 · 창 시작 2026-10-14T02:51:50Z 후 19h00m04s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:322 |
| 44 | 2026-10-14T22:21:54Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 19h30m04s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:323 |
| 45 | 2026-10-14T22:51:54Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 20h00m04s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:324 |
| 46 | 2026-10-14T23:21:54Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 20h30m04s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:325 |
| 47 | 2026-10-14T23:51:54Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 21h00m04s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:326 |
| 48 | 2026-10-15T00:21:54Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 21h30m04s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:327 |
| 49 | 2026-10-15T00:51:54Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 22h00m04s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:328 |
| 50 | 2026-10-15T01:21:54Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 22h30m04s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:329 |
| 51 | 2026-10-15T01:51:54Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 23h00m04s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:330 |
| 52 | 2026-10-15T02:21:54Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 23h30m04s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:331 |
| 53 | 2026-10-15T02:51:54Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 24h00m04s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:332 |
| 54 | 2026-10-15T03:21:54Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 24h30m04s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:333 |
| 55 | 2026-10-15T03:51:54Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 25h00m04s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:334 |
| 56 | 2026-10-15T04:21:54Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 25h30m04s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:335 |
| 57 | 2026-10-15T04:51:54Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 26h00m04s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:336 |
| 58 | 2026-10-15T05:21:55Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5399 · 창 시작 2026-10-14T02:51:50Z 후 26h30m05s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:337 |
| 59 | 2026-10-15T05:51:55Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 27h00m05s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:338 |
| 60 | 2026-10-15T06:21:55Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 27h30m05s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:339 |
| 61 | 2026-10-15T06:51:55Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 28h00m05s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:340 |
| 62 | 2026-10-15T07:21:55Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 28h30m05s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:341 |
| 63 | 2026-10-15T07:51:55Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 29h00m05s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:342 |
| 64 | 2026-10-15T08:21:55Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 29h30m05s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:343 |
| 65 | 2026-10-15T08:51:55Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 30h00m05s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:344 |
| 66 | 2026-10-15T09:21:55Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 30h30m05s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:345 |
| 67 | 2026-10-15T09:51:55Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 31h00m05s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:346 |
| 68 | 2026-10-15T10:21:55Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 31h30m05s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:347 |
| 69 | 2026-10-15T10:51:55Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 32h00m05s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:348 |
| 70 | 2026-10-15T11:21:55Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 32h30m05s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:349 |
| 71 | 2026-10-15T11:51:55Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 33h00m05s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:350 |
| 72 | 2026-10-15T12:21:55Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 33h30m05s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:351 |
| 73 | 2026-10-15T12:51:56Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5399 · 창 시작 2026-10-14T02:51:50Z 후 34h00m06s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:352 |
| 74 | 2026-10-15T13:21:56Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 34h30m06s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:353 |
| 75 | 2026-10-15T13:51:56Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 35h00m06s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:354 |
| 76 | 2026-10-15T14:21:56Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 35h30m06s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:355 |
| 77 | 2026-10-15T14:51:56Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 36h00m06s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:356 |
| 78 | 2026-10-15T15:21:56Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 36h30m06s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:360 |
| 79 | 2026-10-15T15:51:56Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 37h00m06s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:361 |
| 80 | 2026-10-15T16:21:56Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 37h30m06s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:362 |
| 81 | 2026-10-15T16:51:56Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 38h00m06s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:363 |
| 82 | 2026-10-15T17:21:56Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 38h30m06s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:364 |
| 83 | 2026-10-15T17:51:56Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 39h00m06s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:365 |
| 84 | 2026-10-15T18:21:56Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 39h30m06s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:366 |
| 85 | 2026-10-15T18:51:56Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 40h00m06s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:367 |
| 86 | 2026-10-15T19:21:56Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 40h30m06s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:368 |
| 87 | 2026-10-15T19:51:56Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 41h00m06s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:369 |
| 88 | 2026-10-15T20:21:56Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 41h30m06s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:370 |
| 89 | 2026-10-15T20:51:56Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 42h00m06s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:371 |
| 90 | 2026-10-15T21:21:57Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5399 · 창 시작 2026-10-14T02:51:50Z 후 42h30m07s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:372 |
| 91 | 2026-10-15T21:51:57Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 43h00m07s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:373 |
| 92 | 2026-10-15T22:21:57Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 43h30m07s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:374 |
| 93 | 2026-10-15T22:51:57Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 44h00m07s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:375 |
| 94 | 2026-10-15T23:21:57Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 44h30m07s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:376 |
| 95 | 2026-10-15T23:51:57Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 45h00m07s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:377 |
| 96 | 2026-10-16T00:21:57Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 45h30m07s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:378 |
| 97 | 2026-10-16T00:51:57Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 46h00m07s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:379 |
| 98 | 2026-10-16T01:21:57Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 46h30m07s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:380 |
| 99 | 2026-10-16T01:51:57Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 47h00m07s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:381 |
| 100 | 2026-10-16T02:21:57Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 47h30m07s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:382 |
| 101 | 2026-10-16T02:51:57Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 48h00m07s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:383 |
| 102 | 2026-10-16T03:21:57Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 48h30m07s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:384 |
| 103 | 2026-10-16T03:51:57Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 49h00m07s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:385 |
| 104 | 2026-10-16T04:21:57Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 49h30m07s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:386 |
| 105 | 2026-10-16T04:51:57Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 50h00m07s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:387 |
| 106 | 2026-10-16T05:21:57Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 50h30m07s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:388 |
| 107 | 2026-10-16T05:51:58Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5399 · 창 시작 2026-10-14T02:51:50Z 후 51h00m08s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:389 |
| 108 | 2026-10-16T06:21:58Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 51h30m08s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:390 |
| 109 | 2026-10-16T06:51:58Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 52h00m08s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:391 |
| 110 | 2026-10-16T07:21:58Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 52h30m08s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:392 |
| 111 | 2026-10-16T07:51:58Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 53h00m08s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:393 |
| 112 | 2026-10-16T08:21:58Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 53h30m08s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:394 |
| 113 | 2026-10-16T08:51:58Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 54h00m08s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:395 |
| 114 | 2026-10-16T09:21:58Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 54h30m08s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:396 |
| 115 | 2026-10-16T09:51:58Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 55h00m08s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:397 |
| 116 | 2026-10-16T10:21:58Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 55h30m08s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:398 |
| 117 | 2026-10-16T10:51:58Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 56h00m08s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:399 |
| 118 | 2026-10-16T11:21:58Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 56h30m08s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:400 |
| 119 | 2026-10-16T11:40:37Z | 미관측 | `measurement` | `measurement (budget window recorded)` | 같은 셀의 다른 측정(측정 노드 축 sub) · 시각 = 측정 조립(sweep generated_utc = 인증서 measured_utc · 시작 시각 미기록) · 이 발행의 측정 2026-10-16T14:08:27Z 전 · 발행 기록 camp_26101616_cnn_hint__sub__lfm25_26b_v031(셀 대조 simlog:sweep_index.meta.config_name · verdict PASS) · simlog 사본 generated_utc 2026-10-16T11:40:37Z(같음) / 출력 평면 스윕(이 측정에 묶이지 않음 — 다른 측정이 덮었다) · 원장: 이 셀 선언 창 안: serve-lfm25-26b-v031 2026-10-14T02:51:50Z~닫힘 미관측(스트림 마지막 행 2026-10-16T13:51:58Z) (docs/logs/sub/events/2026-10.jsonl:279) · 직전 예산 행 budget_renew(serve-lfm25-26b-v031) 2026-10-16T11:21:58Z(docs/logs/sub/events/2026-10.jsonl:400) · 직후 예산 행 budget_renew(serve-lfm25-26b-v031) 2026-10-16T11:51:58Z(docs/logs/sub/events/2026-10.jsonl:401) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/simlog/26101620_52_00_camp_26101616_cnn_hint__sub__lfm25_26b_v031/sweep_index.json:4 |
| 120 | 2026-10-16T11:51:58Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 57h00m08s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:401 |
| 121 | 2026-10-16T12:21:58Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 57h30m08s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:402 |
| 122 | 2026-10-16T12:51:58Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 58h00m08s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:403 |
| 123 | 2026-10-16T13:21:58Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 58h30m08s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:404 |
| 124 | 2026-10-16T13:51:58Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 59h00m08s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:405 |

**기동 시도**(예산 선언 `budget_declare` 마다 한 시도 · 같은 노드의 이어지는 행과 선언 없는 노드(엔진 로그)의 그 시각 행을 묶었다 · 사살 · 트립 · 거부는 닫힘과 따로 센다 · `measurement` 행은 측정 기록이라 시도로 세지 않는다(위 표에만) — 해석은 저작자 몫)

_이번 캠페인(경계 2026-10-16T14:03:30Z 이후)의 기동 시도 미관측 — 아래는 같은 셀 이름의 앞선 캠페인 행뿐이다._

**같은 셀 이름 · 앞선 캠페인의 기동 1회**(캠페인 경계 2026-10-16T14:03:30Z 이전 — 라벨이 같아 원장에서 함께 묶였을 뿐 이 셀의 시도로 세지 않는다 · 행 전문 = 01 §1.4)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 앞선-1 | sub | `serve-lfm25-26b-v031` | 2026-10-14T02:51:50Z | 119회 · 첫 2026-10-14T02:51:51Z | 없음 | —(닫힘 행 없음) | 3 |
<!-- /FACT:event_timeline -->

<!-- FACT:measurement_env -->
_측정 실행의 env 관측 없음 — 측정 엔진 로그에서 env echo 줄(`<KEY> set by environment to <VALUE>` 등)을 찾지 못했다(evidence.measurement_env_observed · 부재 ≠ 미설정)._
<!-- /FACT:measurement_env -->

> 이 절이 답하는 질문: 수신자가 이 zip 만으로 이 셀을 다시 띄우려면 어떤 순서로 무엇을 하고, 각 단계의 성공을 무엇으로 판정하며, 얼마나 기다려야 하는가?

(a) 사전 준비 — 가중치는 관리 NAS 에서 읽는다(체크포인트 LFM2.5-2.6B, 리비전은 00 §0.2). 필수 env 는 `artifacts/triplet/.env.lfm25-26b-v031`(포트 8002, 이미지 태그, 모델 이름)에 있다. 측정 당시 env 값은 측정 실행 엔진 로그에서 찾지 못했다(위 "측정 실행의 env 관측" 은 비어 있다).
(b) 빌드 — 위 build 단계 명령. docker history 층 창이 37분 남짓이나 이것은 빌드 소요가 아니다(캐시 층 포함). 소스빌드 이미지는 이 셀의 lite 측정 이전에 이미 있었다.
(c) 기동 — 위 serve 단계 명령으로 기동하고 `/health` 200 과 추론 1회를 판정으로 쓴다. 기동 판정 근거는 lite 의 precheck(health)와 cold·warm 응답이다. 기동 소요: 위 표의 serve 소요는 상한일 뿐이다(상주 서버라 기동 시각과 측정 시각 사이가 크다). 엔진 로그가 구간을 기록한다 — 가중치 로드 Model loading took 5.05 GiB memory and 32.594985 seconds. 이번 캠페인의 기동 시도는 미관측이고, 같은 셀 이름의 앞선 캠페인 기동이 1회 있었다(위 표 · 이 셀의 시도로 세지 않는다). 이번 lite 측정은 상주 서버를 재기동하지 않고 그대로 쟀다.
(d) 호스트 메모리 예산 선언 — 앞선 캠페인 행에 선언값이 있다(위 이벤트 표의 floor·weights·kv·overhead 선언값). 선언이지 측정이 아니며 엔진 로그 실측(Model loading 5.05 GiB)과 나란히 읽는다.
(e) 측정 명령 — 위 bench 단계의 명령은 bench JSON 에서 재구성한 vllm bench serve 형태이며, 실제 인자는 00 머리 lite 측정 블록의 `lite_bench.sh` 원문 스냅샷에 있다(cold 1회 + warm burst N=3 · 동시성 1 · warmup 1).
(f) 런타임 경로 — 엔진 로그가 기록한다: Using FLASH_ATTN attention backend · Mamba cache mode is set to 'align' · 체크포인트 파일시스템 CIFS(Checkpoint size 5.02 GiB) · KV 는 kv_cache_memory_bytes 설정으로 37.9 GiB 를 예약하고 메모리 프로파일링을 건너뜀(GPU KV cache size 2,476,552 tokens).

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

### 네트워크가 필요한 단계
베이스 이미지 pull·pip/wheel·소스 clone 은 빌드 단계 필요이며, 오프라인 대체는 이 계보에 기록이 없다(추론 — 실행 검증 ✗).
