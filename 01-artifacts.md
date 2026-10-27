# 01 · 산출물 — 무엇이 실제로 쓰였고, 어떻게 다시 띄우는가

## 1.1 적용 판정 표
> 기계가 적용 증거(빌드 원장 · 원장이 없으면 라벨된 재구성) × 파일 × 선언을 대사한 결과다. `artifacts/` 에는 **적용된 것만** 실렸다 — 빌드 때 스스로 skip 된 패치와 쓰이지 않은 평면의 파일은 표에만 남고 싣지 않는다.

<!-- FACT:slots -->
| 슬롯 | 실림 | 파일 | 신호 | 적용 증거 | 사유 |
|---|---|---|---|---|---|
| `build_patch_post` | 안 실림 | — | 1-signal | build-ledger:docker-image(main-local):sha256:339737df6571ce00993e731ef064a8db66085beaa650e4aa63bdb24f414b2034:/opt/easy-vllm/build_ledger.json | 적용 판정 없음 — 원장 관측(기계 파생) |
| `build_patch_pre` | 안 실림 | — | 1-signal | build-ledger:docker-image(main-local):sha256:339737df6571ce00993e731ef064a8db66085beaa650e4aa63bdb24f414b2034:/opt/easy-vllm/build_ledger.json | 적용 판정 없음 — 원장 관측(기계 파생) |
| `build_recipe` | 실림 | `artifacts/build_recipe/Dockerfile.source-build` · `artifacts/build_recipe/requirements.txt` | 3-signal | build-ledger:build-ledger.dockerfile | sourceDockerfile과requirements의검증지위는위FACT를따른다. NGCtorch유지소스빌드는공유ABI경로이고이번재빌드는없다. Dockerfile17개중16개history일치같은표지가있으면정확실행바이트로승격하지않는다. 빌드입력제약과설치실측을구분한다. |
| `compose` | 실림 | `artifacts/compose/.env.template` · `artifacts/compose/docker-compose.yaml` | 2-signal | file:output/single/docker-compose.yaml | NASread-only마운트·셀env·호스트8912→내부8000을연결한다. W2의교정은yaml내부포트이며compose를다른호스트의경로에그대로복사하지않는다. 측정전리비전과당시envecho가정본이다. |
| `fork_pin` | 안 실림 | — | 1-signal | none:.claude/policies/arch_variant_ledger.json | 셀 env 에 VARIANT 줄 없음 = stock(workflow.md §변종 좌표의 거처)(기계 파생) |
| `runtime_patch` | 안 실림 | — | 1-signal | none:output/single/configs/gemma26b-ctx-single_patch.py | output/single/configs/gemma26b-ctx-single_patch.py 없음 — arm_patch.sh 는 no-op(런타임 패치 불해당)(기계 파생) |
| `triplet` | 실림 | `artifacts/triplet/.env.gemma26b-ctx-single` · `artifacts/triplet/gemma26b-ctx-single.sh` · `artifacts/triplet/gemma26b-ctx-single.yaml` | 3-signal | file:output/single/configs/gemma26b-ctx-single.yaml | yaml은gmu0.85·len262144·절대KV3368402176·KVfp8을담는다. runner는gemma4tool/reasoningparser를붙이고arm_patch를호출하지만셀patch.py부재면no-op다. flag존재를이번전용기능시험PASS로대신하지않는다. |

**레시피 경고 · 실린 리비전**(기계 관측 — 수신자 빌드가 여기서 갈라질 수 있다)

- ⚠ `build_recipe` `output/single/Dockerfile.source-build` — 실린 바이트(worktree)가 측정 이미지 docker history 의 RUN·COPY 와 전수 일치하지 않는다(16/17) · 이미지 빌드 뒤 바뀐 레시피일 수 있다 — 마지막 커밋이 이미지 Created 뒤(이미지 Created 2026-10-14T01:45:19Z · 마지막 커밋 2026-10-14T03:07:26Z)
- `build_recipe` `output/single/Dockerfile.source-build` 실린 리비전: 리비전 미특정 · 선택 방법 `worktree(어느 개정도 이미지 history 와 더 맞지 않는다 · 작업트리 16/17 · 전 이력(output/*/Dockerfile.source-build · 모든 ref)에도 전수 일치 개정 없음 — recipe_vs_image 경고 유지)` · docker history 대조 16/17 · 실린 바이트 `worktree` · 쓰인 바이트임을 관측으로 확인 아니오
- `build_recipe` `output/single/requirements.txt` 실린 리비전: 리비전 미특정 · 선택 방법 `worktree(빌드 원장 sha256 일치 — 쓰인 바이트 관측 확인 = 예)` · docker history 대조 없음 · 실린 바이트 `worktree` · 쓰인 바이트임을 관측으로 확인 예
- `compose` `output/single/docker-compose.yaml` 실린 리비전: 커밋 `2199257f7f71` · 선택 방법 `worktree(mtime 2026-10-26T21:40:37Z ≤ 측정 2026-10-27T02:09:53Z(sweep_index.generated_utc) · 바이트 = 측정 전 마지막 커밋 2199257f7f71)` · docker history 대조 없음 · 실린 바이트 `worktree` · 근거 `mtime≤measured` · 쓰인 바이트임을 관측으로 확인 예

**싣지 않은 파일**(슬롯별 사유 — 빌드 때 skip 된 패치는 아래 적용 집합 표에 있다)

- `build_patch_post` known-non-patch 1건: `.gitkeep`
- `build_patch_pre` known-non-patch 1건: `.gitkeep`
- `compose` `arm_patch.sh` — `not-armed(output/single/configs/gemma26b-ctx-single_patch.py 없음(CONFIG_FILE) — arm_patch.sh 는 no-op · arm_patch.sh 측정 당시 개정(c8bb20e77e52) `_cfg=` 키 체인 RUNTIME_PATCH_STEM > CONFIG_FILE > 'default')`

**파일별 검증 · 관련성**(`generated-unverified` = 실행 검증되지 않은 생성물 — 파일 첫 줄 경고 · PAYLOAD 파일 기록과 같은 표시 · 관련성 = ① 패치 선언 대상 × 모델 config ② 엔진 로그 발화 서명 — 둘 다 관측일 때만 required/inactive-inferred)

| 슬롯 | 파일 | 검증 | 검증 근거 | 관련성 | 관련성 근거 |
|---|---|---|---|---|---|
| `build_recipe` | `artifacts/build_recipe/Dockerfile.source-build` | `generated-unverified` · renderer | 실린 Dockerfile.source-build(worktree)이 측정 이미지 docker history RUN·COPY 와 16/17 만 일치 — 이 이미지를 지은 바이트임을 관측으로 확인하지 못했다(_select_dockerfile · worktree(어느 개정도 이미지 history 와 더 맞지 않는다 · 작업트리 16/17 · 전 이력(output/*/Dockerfile.source-build · 모든 ref)에도 전수 일치 개정 없음 — recipe_vs_image 경고 유지)) | `unknown` | 이 셀은 이 경로로 실행되지 않았다(generated-unverified · 생성 renderer) — 실행 기록이 없어 필요 여부를 판정하지 않는다(참고 형상) |
| `build_recipe` | `artifacts/build_recipe/requirements.txt` | `verified` · cell-run | 쓰인 레시피(build-ledger.dockerfile) — 개정 선택 slots.build_recipe.evidence.selected_revisions | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/.env.template` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/docker-compose.yaml` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/.env.gemma26b-ctx-single` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/gemma26b-ctx-single.sh` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/gemma26b-ctx-single.yaml` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
<!-- /FACT:slots -->

<!-- FACT:applied_set -->
**적용 집합** — 상태 `observed` · 출처 `docker-image(main-local):sha256:339737df6571ce00993e731ef064a8db66085beaa650e4aa63bdb24f414b2034:/opt/easy-vllm/build_ledger.json`

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
| attestation | absent | output/single/benchlog/attestation_gemma26b-ctx-single.json(부재) |
| local-image | observed | sha256:339737df6571ce00993e731ef064a8db66085beaa650e4aa63bdb24f414b2034 |
<!-- /FACT:applied_set -->

## 1.2 슬롯별 적용 사유
> 이 절이 답하는 질문: 실린 산출물 하나하나는 무엇을 고치며, 왜 이 셀에 필요했고, 이 셀에서 실제로 발화했으며, 언제 불필요해지는가?

### build_recipe
sourceDockerfile과requirements의검증지위는위FACT를따른다. NGCtorch유지소스빌드는공유ABI경로이고이번재빌드는없다. Dockerfile17개중16개history일치같은표지가있으면정확실행바이트로승격하지않는다. 빌드입력제약과설치실측을구분한다.

### compose
NASread-only마운트·셀env·호스트8912→내부8000을연결한다. W2의교정은yaml내부포트이며compose를다른호스트의경로에그대로복사하지않는다. 측정전리비전과당시envecho가정본이다.

### triplet
yaml은gmu0.85·len262144·절대KV3368402176·KVfp8을담는다. runner는gemma4tool/reasoningparser를붙이고arm_patch를호출하지만셀patch.py부재면no-op다. flag존재를이번전용기능시험PASS로대신하지않는다.

### 불해당 슬롯
이모델은stock이며PLEmmap플러그인과대상이다르다. build_patch/pre/post·fork·runtime_patch의적용/skip은위표와원장을따른다. 공유inline strip-hoist가skipped면적용패치로부르지않는다.

## 1.3 값의 지위표
> 서빙 설정의 노브 **전부**와 기계가 모은 후보 지위다(lockset 출처 · yaml 주석 · 엔진 기본값). 최종 지위는 아래 `kind: value-status` 블록이 정한다 — 표의 노브마다 블록이 정확히 하나 있어야 린터를 통과한다.

<!-- FACT:value_status -->
서빙 노브 4개 — 노브마다 `kind: value-status` 블록이 **정확히 1개** 있어야 한다(노브·값은 이 표의 글자 그대로 · 값 칸의 `\|` 는 `|` 로 적는다).

| 노브 | 값 | 후보 지위 | 후보 근거 | yaml 주석 | 출처 |
|---|---|---|---|---|---|
| `gpu-memory-utilization` | 0.85 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `max-model-len` | 262144 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `kv-cache-memory-bytes` | 3368402176 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `kv-cache-dtype` | fp8 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
<!-- /FACT:value_status -->

> 이 절이 답하는 질문: 각 서빙 노브의 값은 이 셀에서 어떤 지위인가 — 조정됐나, 승계됐나, 음성대조로 일부러 둔 것인가, 엔진 기본값인가, 필요조건인가?

```hint-event
id: V1
kind: value-status
노브: gpu-memory-utilization
값: 0.85
지위: inherited
근거: yamlgmu_source=hand와과거safety_margin승계; 이번레버대조없음; 절대KV예약산정과구분
출처: [gemma26b-ctx-single.yaml, docs/simlog/2026102406_2_gemma4-26b-a4b-262k/run_summary.json]
```

```hint-event
id: V2
kind: value-status
노브: max-model-len
값: 262144
지위: inherited
근거: config네이티브최대길이승계; YaRN선언없어확장미시도; 과거needle240932토큰과이번1024부하구분
출처: [testlog_26102412_gemma26b_싱글_262k_기동_근접추론_lite_판정 §1, testlog_26102711_gemma26b_ctx_single_코덱스_메인_Full_판정 §2]
```

```hint-event
id: V3
kind: value-status
노브: kv-cache-memory-bytes
값: 3368402176
지위: declared-requirement
근거: 과거클램프null은KV52.89GiB뒤memwatch종료, 클램프존재후통과(W1); 정확바이트값은262144×batch1파생이며다른값대조없음
출처: [docs/simlog/2026102406_2_gemma4-26b-a4b-262k/run_summary.json, docs/simlog/2026102406_2_gemma4-26b-a4b-262k/trial01_memwatch.log]
```

```hint-event
id: V4
kind: value-status
노브: kv-cache-dtype
값: fp8
지위: inherited
근거: 과거레시피후보값승계; auto대조없고fp8정확도검증미수행
출처: [gemma26b-ctx-single.yaml, docs/simlog/2026102406_2_gemma4-26b-a4b-262k/run_summary.json]
```

클램프존재의필요성은해당과거실행조건에서확인했다. 긴요청동시성이나정확도목표가다르면바이트와dtype를다시도출하며현값을보편최적값으로승격하지않는다.

## 1.4 재현 절차
> 셀 실행 기록에서 기계가 파생한 명령 순서와 단계별 소요다. 소요가 `미관측` 인 단계는 시각 기록이 없다는 뜻이다(0 이 아니다). 그 아래는 블랙박스 이벤트 원장에서 이 셀의 행을 기계가 뽑아 기동 시도로 묶은 것이고, 마지막은 측정한 실행의 엔진 로그가 echo 한 env 다(측정 당시 값의 정본).

<!-- FACT:reproduce -->
| 순서 | 단계 | 성공 판정 | 소요 | 관측 구간(UTC) | 소요 출처 | 명령 출처 |
|---|---|---|---|---|---|---|
| 1 | build | 빌드 OK · 이미지 실재 | 층 창 38분 19초(docker history 첫 층 → 끝 층 CreatedAt · 캐시 재사용 층 포함 — 빌드 소요 아님) | 2026-10-14T01:07:00Z → 2026-10-14T01:45:19Z (layer-window) | docker history --no-trunc --format '{{json .}}' sha256:339737df6571ce00993e731ef064a8db66085beaa650e4aa63bdb24f414b2034 · 우리 Dockerfile 층 24개의 CreatedAt 첫 층 → 끝 층(캐시 재사용 층은 이전 빌드 시각 — 빌드 소요 아님) | reconstructed(build-ledger build_args + compose build) |
| 2 | serve | health 200 + 추론 1회(스모크 판정) | ≤ 6분 35초(상한) | 2026-10-27T01:35:55Z → 2026-10-27T01:42:30Z (upper) | docs/logs/main/events/2026-10.jsonl budget_declare(label serve-gemma26b-ctx-single) → 첫 측정 시각(output/single/benchlog/sweep_gemma26b-ctx-single/lite_raw_gemma26b-ctx-single.json measured_utc(호스트 date -u · lite_bench.sh 가 cold 레그 직전에 찍는 측정 시작)) | derived(스모크 사용법) |
| 3 | bench | 리포트 발행(+PASS 면 인증서) | 27분 23초 | 2026-10-27T01:42:30Z → 2026-10-27T02:09:53Z (exact) | output/single/benchlog/sweep_gemma26b-ctx-single/lite_raw_gemma26b-ctx-single.json measured_utc(호스트 date -u · lite_bench.sh 가 cold 레그 직전에 찍는 측정 시작) → sweep_index.generated_utc(호스트 UTC · 두 끝 같은 시계) | reconstructed(bench json + 측정 도구 원문 lite 인자) |

**빌드 층 시각**(관측 · 소요가 아니라 층 창 — 캐시 재사용 층은 이전 빌드의 시각을 가진다)

- `build` 이미지 층 CreatedAt(docker history): 2026-10-14T01:07:00Z → 2026-10-14T01:45:19Z · 층 24개 · 출처 `docker history --no-trunc --format '{{json .}}' sha256:339737df6571ce00993e731ef064a8db66085beaa650e4aa63bdb24f414b2034` — 우리 Dockerfile 층의 CreatedAt 범위 — 캐시 재사용 층은 이전 빌드 시각을 가진다(소요 아님)

**단계별 명령**(기계 파생 — `명령 출처` 가 `derived(…)` 면 사용법에서 파생 · `reconstructed(…)` 면 괄호 안 관측(docker history build-arg · compose build · bench JSON 필드)에서 재구성한 명령이다. 어느 쪽도 실행 원문(로그에 남은 줄)은 아니다)

**1. build**

```sh
# 실린 artifacts/build_recipe/Dockerfile.source-build(실린 개정: worktree — 이미지를 지은 바이트임을 관측으로 확인 못함(docker history RUN·COPY 16/17))를 output/single/Dockerfile.source-build 자리에 두고 컨텍스트에서 짓는다.
mkdir -p output/single/build_patches_src output/single/build_patches   # 빌드 원장 context_inventory.build_patch_pre = ['.gitkeep'] · 빌드 원장 context_inventory.build_patch_post = ['.gitkeep']
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
# 동등 경로(compose): docker compose -f output/single/docker-compose.yaml --env-file output/single/envs/.env.gemma26b-ctx-single build
```

**2. serve**

```sh
bash .claude/skills/upstream-version-watch/scripts/single_serve_up.sh gemma26b-ctx-single
```

**3. bench**

```sh
# bench JSON 필드로 복원: backend · model_id→--model · tokenizer_id→--tokenizer · num_prompts · max_concurrency · request_rate · burstiness · 요청당 입출력 길이 = total_*_tokens ÷ completed(나누어떨어질 때만 — 평균이다 · 요청별 길이는 JSON 에 없다) · 나누어떨어지면 `--dataset-name random` 으로 추정했다(측정 도구가 그 꼴로 부른다 — 아래 도구 원문).
# JSON 에 없는 인자(--endpoint · --base-url · --ignore-eos · --num-warmups · --temperature · --seed · --random-range-ratio · --trust-remote-code)는 측정 도구가 정했다 — level_* = run_bench.sh · lite_cold/lite_warm = lite_bench.sh(.claude/skills/adversarial-benchmark/scripts/) · 셀 스윕 순서는 sweep_bench.sh.
# lite_cold/lite_warm 의 데이터셋 · 부하 인자 = 측정 시점 도구 원문 lite_bench.sh@90a3cdd56a94 L177-L183(리터럴 인자만 · `$변수` 인자는 도구가 값을 정하지 않아 JSON 필드로 채운다) — `total_*_tokens ÷ completed` 는 실측 토큰(chat 템플릿 포함)이지 인자가 아니다.
# output/single/benchlog/sweep_gemma26b-ctx-single/lite_cold_gemma26b-ctx-single.json (date 20261027-014254) · 실측 토큰(chat 템플릿 포함 · total ÷ completed · 인자 아님): 입력 525/1 = 525 · 출력 128/1 = 128
vllm bench serve --backend openai-chat --model gemma-4-26b-a4b --tokenizer /app/models/Google/Gemma_4/gemma-4-26B-A4B-it --trust-remote-code --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 --ignore-eos --num-prompts 1 --max-concurrency 1 --request-rate inf
# output/single/benchlog/sweep_gemma26b-ctx-single/lite_warm_gemma26b-ctx-single.json (date 20261027-014327) · 실측 토큰(chat 템플릿 포함 · total ÷ completed · 인자 아님): 입력 1576/3 = 525.333 · 출력 384/3 = 128
vllm bench serve --backend openai-chat --model gemma-4-26b-a4b --tokenizer /app/models/Google/Gemma_4/gemma-4-26B-A4B-it --trust-remote-code --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 --ignore-eos --num-prompts 3 --max-concurrency 1 --request-rate inf
```

**슬롯 → 빌드 컨텍스트 매핑**(빌드 컨텍스트 = `output/<토폴로지>/` · 원천 = 실린 Dockerfile 의 COPY · compose 의 env_file · volumes · command — `—` 는 빌드 컨텍스트 입력이 아니다)

| zip 경로 | 빌드 컨텍스트 자리 | 근거 |
|---|---|---|
| `artifacts/triplet/.env.gemma26b-ctx-single` | `envs/.env.gemma26b-ctx-single` | compose env_file envs/.env.${CONFIG_FILE}(셀 env) |
| `artifacts/triplet/gemma26b-ctx-single.sh` | `configs/gemma26b-ctx-single.sh` | compose volumes ./configs:/app/configs |
| `artifacts/triplet/gemma26b-ctx-single.yaml` | `configs/gemma26b-ctx-single.yaml` | compose volumes ./configs:/app/configs |
| `artifacts/build_recipe/Dockerfile.source-build` | `Dockerfile.source-build` | compose build.dockerfile Dockerfile.source-build(리터럴) · build.context . |
| `artifacts/build_recipe/requirements.txt` | `requirements.txt` | Dockerfile.source-build COPY requirements.txt |
| `artifacts/compose/.env.template` | `.env` | compose 변수치환 프로젝트 .env(${…} 참조) |
| `artifacts/compose/docker-compose.yaml` | `docker-compose.yaml` | compose 파일(빌드 컨텍스트 루트) |
| (실린 파일 없음 — 빈 디렉터리 생성) | `build_patches_src/` | Dockerfile.source-build COPY build_patches_src/ · 실린 패치 없음 — 빈 디렉터리를 만든다(없으면 COPY 단계에서 빌드가 멈춘다) · 빌드 원장 context_inventory.build_patch_pre = ['.gitkeep'] |
| (실린 파일 없음 — 빈 디렉터리 생성) | `build_patches/` | Dockerfile.source-build COPY build_patches/ · 실린 패치 없음 — 빈 디렉터리를 만든다(없으면 COPY 단계에서 빌드가 멈춘다) · 빌드 원장 context_inventory.build_patch_post = ['.gitkeep'] |

**기동 성공 판정(관측 · 발행 자격)**: health 200 = true · 추론 1회 = true · 근거 `output/single/benchlog/sweep_gemma26b-ctx-single/level_01/post_health_gemma26b-ctx-single.json` · `output/single/benchlog/sweep_gemma26b-ctx-single/level_01/guidellm_gemma26b-ctx-single.json`
<!-- /FACT:reproduce -->

<!-- FACT:event_timeline -->
**블랙박스 이벤트 88행**(원장에서 이 셀 label·config 에 맞는 행과 그와 시간이 겹치는 같은 노드의 다른 라벨 창 · 시각순 · 기계 발췌 · 이번 캠페인 32행 · 같은 셀 이름 · 앞선 캠페인 56행(내용 칸에 표기) · 라벨 다름(같은 노드 · 창 밖 이 셀 행과 시간이 겹치는 다른 라벨 선언 창) 19행 — 이 셀의 선언으로 세지 않는다)

> **캠페인 경계** 2026-10-26T21:46:47Z(`camp-26102705-codex-single8` · evidence.event_campaign_boundary): campaigns/camp-26102705-codex-single8/campaign.yaml declared_utc 2026-10-26T21:46:47Z · 이 발행이 묶인 측정을 서빙한 budget_declare 창 시작 2026-10-27T01:35:55Z → 경계 = 앞선 값 2026-10-26T21:46:47Z(이전 행 = 같은 셀 이름 · 앞선 캠페인)

> **원장 관측 범위**(노드별 첫 행 → 마지막 행 · evidence.event_ledger_spans): main = 2026-07-31T03:27:43Z → 2026-10-27T02:51:28Z(`docs/logs/main/events/2026-07.jsonl` · `docs/logs/main/events/2026-08.jsonl` · `docs/logs/main/events/2026-09.jsonl` · `docs/logs/main/events/2026-10.jsonl` · `docs/logs/main/events/thermal.jsonl` · `docs/logs/main/events/watchdog.jsonl`)

| # | UTC | 노드 | kind | label | 내용 | 출처 |
|---|---|---|---|---|---|---|
| 1 | 2026-10-23T21:48:33Z | main | `budget_declare` | `camp-26102401-gemma26b` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=47375 · weights_mib=49221 · kv_mib=3212 · overhead_mib=24800 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 라벨 다름: `camp-26102401-gemma26b` — 같은 노드 · 이 셀 라벨(`gemma26b-ctx-single`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · budget_renew 1회 · 닫힘 budget_clear 2026-10-23T22:03:55Z(선언 후 15m22s) · measured_utc 2026-10-27T02:09:53Z 는 이 창 밖(측정 전) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:496 |
| 2 | 2026-10-23T21:48:33Z | main | `serve_start` | — | serve_start · model=gemma-4-26b-a4b · tensor_parallel_size=1 · topology=single · session=serve-gemma26b-ctx-single-20261023T214833Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:497 |
| 3 | 2026-10-23T21:48:33Z | main | `budget_honored` | `camp-26102401-gemma26b` | budget_honored · floor_mib=47375 · arm_ceiling_mib=44303 · remaining_s=7200 · 창 시작 2026-10-23T21:48:33Z 후 0m00s · 라벨 다름: `camp-26102401-gemma26b` — 같은 노드 · 이 셀 라벨(`gemma26b-ctx-single`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:745 |
| 4 | 2026-10-23T21:48:34Z | main | `budget_renew` | `camp-26102401-gemma26b` | budget_renew · floor_mib=47375 · ttl_s=7200 · remaining_before_s=7199 · 창 시작 2026-10-23T21:48:33Z 후 0m01s · 라벨 다름: `camp-26102401-gemma26b` — 같은 노드 · 이 셀 라벨(`gemma26b-ctx-single`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:498 |
| 5 | 2026-10-23T21:49:07Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=63404 · rate_mib_s=28704 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → camp-26102401-gemma26b 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-23T21:48:33Z 후 0m34s) · 라벨 다름: `camp-26102401-gemma26b` — 같은 노드 · 이 셀 라벨(`gemma26b-ctx-single`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:746 |
| 6 | 2026-10-23T22:03:55Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → camp-26102401-gemma26b 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-23T21:48:33Z 후 15m22s) · 라벨 다름: `camp-26102401-gemma26b` — 같은 노드 · 이 셀 라벨(`gemma26b-ctx-single`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:499 |
| 7 | 2026-10-23T22:03:55Z | main | `serve_stop` | — | serve_stop · model=gemma-4-26b-a4b · duration_s=922 · session=serve-gemma26b-ctx-single-20261023T214833Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:500 |
| 8 | 2026-10-23T22:03:55Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → camp-26102401-gemma26b 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-23T21:48:33Z 후 15m22s) · 라벨 다름: `camp-26102401-gemma26b` — 같은 노드 · 이 셀 라벨(`gemma26b-ctx-single`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:747 |
| 9 | 2026-10-23T23:52:08Z | main | `budget_declare` | `camp-26102401-gemma26b` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=47375 · weights_mib=49221 · kv_mib=3212 · overhead_mib=24800 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 라벨 다름: `camp-26102401-gemma26b` — 같은 노드 · 이 셀 라벨(`gemma26b-ctx-single`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · budget_renew 8회 · 닫힘 budget_clear 2026-10-24T03:50:45Z(선언 후 3h58m37s) · measured_utc 2026-10-27T02:09:53Z 는 이 창 밖(측정 전) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:501 |
| 10 | 2026-10-23T23:52:08Z | main | `serve_start` | — | serve_start · model=gemma-4-26b-a4b · tensor_parallel_size=1 · topology=single · session=serve-gemma26b-ctx-single-20261023T235208Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:502 |
| 11 | 2026-10-23T23:52:09Z | main | `budget_honored` | `camp-26102401-gemma26b` | budget_honored · floor_mib=47375 · arm_ceiling_mib=44303 · remaining_s=7199 · 창 시작 2026-10-23T23:52:09Z 후 0m00s · 라벨 다름: `camp-26102401-gemma26b` — 같은 노드 · 이 셀 라벨(`gemma26b-ctx-single`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:748 |
| 12 | 2026-10-23T23:52:10Z | main | `budget_renew` | `camp-26102401-gemma26b` | budget_renew · floor_mib=47375 · ttl_s=7200 · remaining_before_s=7198 · 창 시작 2026-10-23T23:52:08Z 후 0m02s · 라벨 다름: `camp-26102401-gemma26b` — 같은 노드 · 이 셀 라벨(`gemma26b-ctx-single`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:503 |
| 13 | 2026-10-23T23:52:41Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=84180 · rate_mib_s=27629 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → camp-26102401-gemma26b 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-23T23:52:09Z 후 0m32s) · 라벨 다름: `camp-26102401-gemma26b` — 같은 노드 · 이 셀 라벨(`gemma26b-ctx-single`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:749 |
| 14 | 2026-10-24T00:22:10Z | main | `budget_renew` | `camp-26102401-gemma26b` | budget_renew · floor_mib=47375 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-23T23:52:08Z 후 30m02s · 라벨 다름: `camp-26102401-gemma26b` — 같은 노드 · 이 셀 라벨(`gemma26b-ctx-single`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:504 |
| 15 | 2026-10-24T00:52:10Z | main | `budget_renew` | `camp-26102401-gemma26b` | budget_renew · floor_mib=47375 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-23T23:52:08Z 후 1h00m02s · 라벨 다름: `camp-26102401-gemma26b` — 같은 노드 · 이 셀 라벨(`gemma26b-ctx-single`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:505 |
| 16 | 2026-10-24T01:22:10Z | main | `budget_renew` | `camp-26102401-gemma26b` | budget_renew · floor_mib=47375 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-23T23:52:08Z 후 1h30m02s · 라벨 다름: `camp-26102401-gemma26b` — 같은 노드 · 이 셀 라벨(`gemma26b-ctx-single`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:506 |
| 17 | 2026-10-24T01:52:10Z | main | `budget_renew` | `camp-26102401-gemma26b` | budget_renew · floor_mib=47375 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-23T23:52:08Z 후 2h00m02s · 라벨 다름: `camp-26102401-gemma26b` — 같은 노드 · 이 셀 라벨(`gemma26b-ctx-single`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:507 |
| 18 | 2026-10-24T02:22:10Z | main | `budget_renew` | `camp-26102401-gemma26b` | budget_renew · floor_mib=47375 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-23T23:52:08Z 후 2h30m02s · 라벨 다름: `camp-26102401-gemma26b` — 같은 노드 · 이 셀 라벨(`gemma26b-ctx-single`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:508 |
| 19 | 2026-10-24T02:52:10Z | main | `budget_renew` | `camp-26102401-gemma26b` | budget_renew · floor_mib=47375 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-23T23:52:08Z 후 3h00m02s · 라벨 다름: `camp-26102401-gemma26b` — 같은 노드 · 이 셀 라벨(`gemma26b-ctx-single`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:509 |
| 20 | 2026-10-24T03:22:10Z | main | `budget_renew` | `camp-26102401-gemma26b` | budget_renew · floor_mib=47375 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-23T23:52:08Z 후 3h30m02s · 라벨 다름: `camp-26102401-gemma26b` — 같은 노드 · 이 셀 라벨(`gemma26b-ctx-single`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:510 |
| 21 | 2026-10-24T03:50:45Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → camp-26102401-gemma26b 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-23T23:52:08Z 후 3h58m37s) · 라벨 다름: `camp-26102401-gemma26b` — 같은 노드 · 이 셀 라벨(`gemma26b-ctx-single`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:511 |
| 22 | 2026-10-24T03:50:46Z | main | `serve_stop` | — | serve_stop · model=gemma-4-26b-a4b · duration_s=14318 · session=serve-gemma26b-ctx-single-20261023T235208Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:512 |
| 23 | 2026-10-24T03:50:46Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → camp-26102401-gemma26b 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-23T23:52:09Z 후 3h58m37s) · 라벨 다름: `camp-26102401-gemma26b` — 같은 노드 · 이 셀 라벨(`gemma26b-ctx-single`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:750 |
| 24 | 2026-10-25T00:02:06Z | main | `budget_declare` | `serve-gemma26b-ctx-single` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=53743 · weights_mib=49221 · kv_mib=3212 · overhead_mib=18432 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 기동 창 1/3: budget_renew 1회 · 닫힘 budget_clear 2026-10-25T00:09:42Z(선언 후 7m36s) · measured_utc 2026-10-27T02:09:53Z 는 이 창 밖(다른 기동 · 측정 전) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:595 |
| 25 | 2026-10-25T00:02:06Z | main | `serve_start` | — | serve_start · model=gemma-4-26b-a4b · tensor_parallel_size=1 · topology=single · session=serve-gemma26b-ctx-single-20261025T000206Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:596 |
| 26 | 2026-10-25T00:02:06Z | main | `budget_honored` | `serve-gemma26b-ctx-single` | budget_honored · floor_mib=53743 · arm_ceiling_mib=50671 · remaining_s=7200 · 창 시작 2026-10-25T00:02:06Z 후 0m00s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:805 |
| 27 | 2026-10-25T00:02:08Z | main | `budget_renew` | `serve-gemma26b-ctx-single` | budget_renew · floor_mib=53743 · ttl_s=7200 · remaining_before_s=7198 · 창 시작 2026-10-25T00:02:06Z 후 0m02s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:597 |
| 28 | 2026-10-25T00:02:39Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=86372 · rate_mib_s=25208 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → serve-gemma26b-ctx-single 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-25T00:02:06Z 후 0m33s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:806 |
| 29 | 2026-10-25T00:09:42Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → serve-gemma26b-ctx-single 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-25T00:02:06Z 후 7m36s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:598 |
| 30 | 2026-10-25T00:09:42Z | main | `serve_stop` | — | serve_stop · model=gemma-4-26b-a4b · duration_s=456 · session=serve-gemma26b-ctx-single-20261025T000206Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:599 |
| 31 | 2026-10-25T00:09:43Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → serve-gemma26b-ctx-single 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-25T00:02:06Z 후 7m37s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:807 |
| 32 | 2026-10-25T00:11:00Z | main | `budget_declare` | `serve-gemma26b-ctx-single` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=53743 · weights_mib=49221 · kv_mib=3212 · overhead_mib=18432 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 기동 창 2/3: budget_renew 17회 · 닫힘 budget_clear 2026-10-25T00:44:20Z(선언 후 33m20s) · measured_utc 2026-10-27T02:09:53Z 는 이 창 밖(다른 기동 · 측정 전) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:601 |
| 33 | 2026-10-25T00:11:00Z | main | `serve_start` | — | serve_start · model=gemma-4-26b-a4b · tensor_parallel_size=1 · topology=single · session=serve-gemma26b-ctx-single-20261025T001100Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:602 |
| 34 | 2026-10-25T00:11:00Z | main | `budget_honored` | `serve-gemma26b-ctx-single` | budget_honored · floor_mib=53743 · arm_ceiling_mib=50671 · remaining_s=7200 · 창 시작 2026-10-25T00:11:00Z 후 0m00s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:808 |
| 35 | 2026-10-25T00:11:01Z | main | `budget_renew` | `serve-gemma26b-ctx-single` | budget_renew · floor_mib=53743 · ttl_s=7200 · remaining_before_s=7199 · 창 시작 2026-10-25T00:11:00Z 후 0m01s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:603 |
| 36 | 2026-10-25T00:11:33Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=77433 · rate_mib_s=29684 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → serve-gemma26b-ctx-single 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-25T00:11:00Z 후 0m33s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:809 |
| 37 | 2026-10-25T00:17:56Z | main | `budget_renew` | `serve-gemma26b-ctx-single` | budget_renew · floor_mib=53743 · ttl_s=7200 · remaining_before_s=6785 · 창 시작 2026-10-25T00:11:00Z 후 6m56s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:604 |
| 38 | 2026-10-25T00:21:28Z | main | `budget_renew` | `serve-gemma26b-ctx-single` | budget_renew · floor_mib=53743 · ttl_s=7200 · remaining_before_s=6988 · 창 시작 2026-10-25T00:11:00Z 후 10m28s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:605 |
| 39 | 2026-10-25T00:24:52Z | main | `budget_renew` | `serve-gemma26b-ctx-single` | budget_renew · floor_mib=53743 · ttl_s=7200 · remaining_before_s=6996 · 창 시작 2026-10-25T00:11:00Z 후 13m52s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:606 |
| 40 | 2026-10-25T00:28:16Z | main | `budget_renew` | `serve-gemma26b-ctx-single` | budget_renew · floor_mib=53743 · ttl_s=7200 · remaining_before_s=6996 · 창 시작 2026-10-25T00:11:00Z 후 17m16s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:607 |
| 41 | 2026-10-25T00:30:15Z | main | `budget_renew` | `serve-gemma26b-ctx-single` | budget_renew · floor_mib=53743 · ttl_s=7200 · remaining_before_s=7081 · 창 시작 2026-10-25T00:11:00Z 후 19m15s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:608 |
| 42 | 2026-10-25T00:32:13Z | main | `budget_renew` | `serve-gemma26b-ctx-single` | budget_renew · floor_mib=53743 · ttl_s=7200 · remaining_before_s=7082 · 창 시작 2026-10-25T00:11:00Z 후 21m13s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:609 |
| 43 | 2026-10-25T00:34:12Z | main | `budget_renew` | `serve-gemma26b-ctx-single` | budget_renew · floor_mib=53743 · ttl_s=7200 · remaining_before_s=7081 · 창 시작 2026-10-25T00:11:00Z 후 23m12s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:610 |
| 44 | 2026-10-25T00:35:38Z | main | `budget_renew` | `serve-gemma26b-ctx-single` | budget_renew · floor_mib=53743 · ttl_s=7200 · remaining_before_s=7114 · 창 시작 2026-10-25T00:11:00Z 후 24m38s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:611 |
| 45 | 2026-10-25T00:37:03Z | main | `budget_renew` | `serve-gemma26b-ctx-single` | budget_renew · floor_mib=53743 · ttl_s=7200 · remaining_before_s=7115 · 창 시작 2026-10-25T00:11:00Z 후 26m03s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:612 |
| 46 | 2026-10-25T00:38:28Z | main | `budget_renew` | `serve-gemma26b-ctx-single` | budget_renew · floor_mib=53743 · ttl_s=7200 · remaining_before_s=7115 · 창 시작 2026-10-25T00:11:00Z 후 27m28s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:613 |
| 47 | 2026-10-25T00:39:32Z | main | `budget_renew` | `serve-gemma26b-ctx-single` | budget_renew · floor_mib=53743 · ttl_s=7200 · remaining_before_s=7136 · 창 시작 2026-10-25T00:11:00Z 후 28m32s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:614 |
| 48 | 2026-10-25T00:40:36Z | main | `budget_renew` | `serve-gemma26b-ctx-single` | budget_renew · floor_mib=53743 · ttl_s=7200 · remaining_before_s=7136 · 창 시작 2026-10-25T00:11:00Z 후 29m36s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:615 |
| 49 | 2026-10-25T00:41:01Z | main | `budget_renew` | `serve-gemma26b-ctx-single` | budget_renew · floor_mib=53743 · ttl_s=7200 · remaining_before_s=7175 · 창 시작 2026-10-25T00:11:00Z 후 30m01s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:616 |
| 50 | 2026-10-25T00:41:40Z | main | `budget_renew` | `serve-gemma26b-ctx-single` | budget_renew · floor_mib=53743 · ttl_s=7200 · remaining_before_s=7161 · 창 시작 2026-10-25T00:11:00Z 후 30m40s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:617 |
| 51 | 2026-10-25T00:42:29Z | main | `budget_renew` | `serve-gemma26b-ctx-single` | budget_renew · floor_mib=53743 · ttl_s=7200 · remaining_before_s=7151 · 창 시작 2026-10-25T00:11:00Z 후 31m29s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:618 |
| 52 | 2026-10-25T00:43:19Z | main | `budget_renew` | `serve-gemma26b-ctx-single` | budget_renew · floor_mib=53743 · ttl_s=7200 · remaining_before_s=7150 · 창 시작 2026-10-25T00:11:00Z 후 32m19s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:619 |
| 53 | 2026-10-25T00:44:08Z | 미관측 | `measurement` | `measurement (budget window recorded)` | 같은 셀의 다른 측정(측정 노드 축 main) · 시각 = 측정 조립(sweep generated_utc = 인증서 measured_utc · 시작 시각 미기록) · 이 발행의 측정 2026-10-27T02:09:53Z 전 · 발행 기록 gemma26b_ctx_single(셀 대조 simlog:sweep_index.meta.config_name · verdict PASS) · simlog 사본 generated_utc 2026-10-25T00:44:08Z(같음) · 원장: 이 셀 선언 창 안: serve-gemma26b-ctx-single 2026-10-25T00:11:00Z~2026-10-25T00:44:20Z (docs/logs/main/events/2026-10.jsonl:601) · serve-gemma26b-ctx-single 2026-10-25T00:11:00Z~2026-10-25T00:44:21Z (docs/logs/main/events/watchdog.jsonl:808) · 직전 예산 행 budget_renew(serve-gemma26b-ctx-single) 2026-10-25T00:43:19Z(docs/logs/main/events/2026-10.jsonl:619) · 직후 예산 행 budget_clear 2026-10-25T00:44:20Z(docs/logs/main/events/2026-10.jsonl:620) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/simlog/26102513_00_00_gemma26b_ctx_single/sweep_index.json:4 |
| 54 | 2026-10-25T00:44:20Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → serve-gemma26b-ctx-single 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-25T00:11:00Z 후 33m20s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:620 |
| 55 | 2026-10-25T00:44:20Z | main | `serve_stop` | — | serve_stop · model=gemma-4-26b-a4b · duration_s=2000 · session=serve-gemma26b-ctx-single-20261025T001100Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:621 |
| 56 | 2026-10-25T00:44:21Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → serve-gemma26b-ctx-single 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-25T00:11:00Z 후 33m21s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:810 |
| 57 | 2026-10-27T01:35:55Z | main | `budget_declare` | `serve-gemma26b-ctx-single` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=53743 · weights_mib=49221 · kv_mib=3212 · overhead_mib=18432 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 기동 창 3/3: budget_renew 18회 · 닫힘 budget_clear 2026-10-27T02:51:28Z(선언 후 1h15m33s) · measured_utc 2026-10-27T02:09:53Z 가 이 창 안(이 측정의 기동) | docs/logs/main/events/2026-10.jsonl:722 |
| 58 | 2026-10-27T01:35:55Z | main | `serve_start` | — | serve_start · model=gemma-4-26b-a4b · tensor_parallel_size=1 · topology=single · session=serve-gemma26b-ctx-single-20261027T013555Z | docs/logs/main/events/2026-10.jsonl:723 |
| 59 | 2026-10-27T01:35:56Z | main | `budget_renew` | `serve-gemma26b-ctx-single` | budget_renew · floor_mib=53743 · ttl_s=7200 · remaining_before_s=7199 · 창 시작 2026-10-27T01:35:55Z 후 0m01s | docs/logs/main/events/2026-10.jsonl:724 |
| 60 | 2026-10-27T01:35:56Z | main | `budget_honored` | `serve-gemma26b-ctx-single` | budget_honored · floor_mib=53743 · arm_ceiling_mib=50671 · remaining_s=7199 · 창 시작 2026-10-27T01:35:56Z 후 0m00s | docs/logs/main/events/watchdog.jsonl:837 |
| 61 | 2026-10-27T01:36:11Z | main | `engine_log_window_start` | — | 캡처된 엔진 로그의 첫 시각(꼬리 캡처일 수 있다 — 엔진 시작 시각이 아닐 수 있다) · 선언 2026-10-27T01:35:55Z 후 0m16s · 시계: 컨테이너 시각을 UTC 로 읽어 측정 창 [2026-10-27T01:35:55Z, 2026-10-27T02:51:28Z] 안에 드는 것을 확인 | output/single/benchlog/sweep_gemma26b-ctx-single/lite_engine_gemma26b-ctx-single.log:36 |
| 62 | 2026-10-27T01:36:28Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=67658 · rate_mib_s=28058 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → serve-gemma26b-ctx-single 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-27T01:35:56Z 후 0m32s) | docs/logs/main/events/watchdog.jsonl:838 |
| 63 | 2026-10-27T01:36:29Z | main | `engine_prefetch_disabled` | — | 가중치 auto-prefetch 꺼짐 줄 ×1 — filesystem CIFS(vLLM 로그 문구) | output/single/benchlog/sweep_gemma26b-ctx-single/lite_engine_gemma26b-ctx-single.log:65 |
| 64 | 2026-10-27T01:40:51Z | main | `engine_weights_loaded` | — | 가중치 적재 완료 줄 ×1(rank·단계별) · 최장 261.99s · 이 행 = 마지막 줄 | output/single/benchlog/sweep_gemma26b-ctx-single/lite_engine_gemma26b-ctx-single.log:72 |
| 65 | 2026-10-27T01:41:03Z | main | `engine_model_loaded` | — | 모델 적재 완료 줄 ×1(rank 별) · 마지막 줄 48.54 GiB · 278.1s | output/single/benchlog/sweep_gemma26b-ctx-single/lite_engine_gemma26b-ctx-single.log:75 |
| 66 | 2026-10-27T01:41:27Z | main | `engine_kv_cache_sized` | — | GPU KV cache 267,398 tokens · 요청당 262,144 tokens 기준 최대 동시성 1.02x | output/single/benchlog/sweep_gemma26b-ctx-single/lite_engine_gemma26b-ctx-single.log:85 |
| 67 | 2026-10-27T01:41:39Z | main | `engine_init_done` | — | 엔진 초기화(profile·KV 할당·warmup) 35.47s | output/single/benchlog/sweep_gemma26b-ctx-single/lite_engine_gemma26b-ctx-single.log:102 |
| 68 | 2026-10-27T01:42:02Z | main | `server_starting` | — | API 서버 기동 — 이 측정 창 선언 2026-10-27T01:35:55Z 후 6m07s · 캡처 로그 첫 시각 2026-10-27T01:36:11Z 후 5m51s | output/single/benchlog/sweep_gemma26b-ctx-single/lite_engine_gemma26b-ctx-single.log:111 |
| 69 | 2026-10-27T01:43:28Z | main | `budget_renew` | `serve-gemma26b-ctx-single` | budget_renew · floor_mib=53743 · ttl_s=7200 · remaining_before_s=6748 · 창 시작 2026-10-27T01:35:55Z 후 7m33s | docs/logs/main/events/2026-10.jsonl:725 |
| 70 | 2026-10-27T01:46:59Z | main | `budget_renew` | `serve-gemma26b-ctx-single` | budget_renew · floor_mib=53743 · ttl_s=7200 · remaining_before_s=6989 · 창 시작 2026-10-27T01:35:55Z 후 11m04s | docs/logs/main/events/2026-10.jsonl:726 |
| 71 | 2026-10-27T01:50:23Z | main | `budget_renew` | `serve-gemma26b-ctx-single` | budget_renew · floor_mib=53743 · ttl_s=7200 · remaining_before_s=6996 · 창 시작 2026-10-27T01:35:55Z 후 14m28s | docs/logs/main/events/2026-10.jsonl:727 |
| 72 | 2026-10-27T01:53:48Z | main | `budget_renew` | `serve-gemma26b-ctx-single` | budget_renew · floor_mib=53743 · ttl_s=7200 · remaining_before_s=6995 · 창 시작 2026-10-27T01:35:55Z 후 17m53s | docs/logs/main/events/2026-10.jsonl:728 |
| 73 | 2026-10-27T01:55:46Z | main | `budget_renew` | `serve-gemma26b-ctx-single` | budget_renew · floor_mib=53743 · ttl_s=7200 · remaining_before_s=7082 · 창 시작 2026-10-27T01:35:55Z 후 19m51s | docs/logs/main/events/2026-10.jsonl:729 |
| 74 | 2026-10-27T01:57:45Z | main | `budget_renew` | `serve-gemma26b-ctx-single` | budget_renew · floor_mib=53743 · ttl_s=7200 · remaining_before_s=7081 · 창 시작 2026-10-27T01:35:55Z 후 21m50s | docs/logs/main/events/2026-10.jsonl:730 |
| 75 | 2026-10-27T01:59:43Z | main | `budget_renew` | `serve-gemma26b-ctx-single` | budget_renew · floor_mib=53743 · ttl_s=7200 · remaining_before_s=7082 · 창 시작 2026-10-27T01:35:55Z 후 23m48s | docs/logs/main/events/2026-10.jsonl:731 |
| 76 | 2026-10-27T02:01:10Z | main | `budget_renew` | `serve-gemma26b-ctx-single` | budget_renew · floor_mib=53743 · ttl_s=7200 · remaining_before_s=7113 · 창 시작 2026-10-27T01:35:55Z 후 25m15s | docs/logs/main/events/2026-10.jsonl:732 |
| 77 | 2026-10-27T02:02:35Z | main | `budget_renew` | `serve-gemma26b-ctx-single` | budget_renew · floor_mib=53743 · ttl_s=7200 · remaining_before_s=7115 · 창 시작 2026-10-27T01:35:55Z 후 26m40s | docs/logs/main/events/2026-10.jsonl:733 |
| 78 | 2026-10-27T02:04:01Z | main | `budget_renew` | `serve-gemma26b-ctx-single` | budget_renew · floor_mib=53743 · ttl_s=7200 · remaining_before_s=7114 · 창 시작 2026-10-27T01:35:55Z 후 28m06s | docs/logs/main/events/2026-10.jsonl:734 |
| 79 | 2026-10-27T02:05:04Z | main | `budget_renew` | `serve-gemma26b-ctx-single` | budget_renew · floor_mib=53743 · ttl_s=7200 · remaining_before_s=7137 · 창 시작 2026-10-27T01:35:55Z 후 29m09s | docs/logs/main/events/2026-10.jsonl:735 |
| 80 | 2026-10-27T02:05:57Z | main | `budget_renew` | `serve-gemma26b-ctx-single` | budget_renew · floor_mib=53743 · ttl_s=7200 · remaining_before_s=7147 · 창 시작 2026-10-27T01:35:55Z 후 30m02s | docs/logs/main/events/2026-10.jsonl:736 |
| 81 | 2026-10-27T02:06:08Z | main | `budget_renew` | `serve-gemma26b-ctx-single` | budget_renew · floor_mib=53743 · ttl_s=7200 · remaining_before_s=7189 · 창 시작 2026-10-27T01:35:55Z 후 30m13s | docs/logs/main/events/2026-10.jsonl:737 |
| 82 | 2026-10-27T02:07:11Z | main | `budget_renew` | `serve-gemma26b-ctx-single` | budget_renew · floor_mib=53743 · ttl_s=7200 · remaining_before_s=7137 · 창 시작 2026-10-27T01:35:55Z 후 31m16s | docs/logs/main/events/2026-10.jsonl:738 |
| 83 | 2026-10-27T02:08:06Z | main | `budget_renew` | `serve-gemma26b-ctx-single` | budget_renew · floor_mib=53743 · ttl_s=7200 · remaining_before_s=7145 · 창 시작 2026-10-27T01:35:55Z 후 32m11s | docs/logs/main/events/2026-10.jsonl:739 |
| 84 | 2026-10-27T02:09:00Z | main | `budget_renew` | `serve-gemma26b-ctx-single` | budget_renew · floor_mib=53743 · ttl_s=7200 · remaining_before_s=7146 · 창 시작 2026-10-27T01:35:55Z 후 33m05s | docs/logs/main/events/2026-10.jsonl:740 |
| 85 | 2026-10-27T02:35:57Z | main | `budget_renew` | `serve-gemma26b-ctx-single` | budget_renew · floor_mib=53743 · ttl_s=7200 · remaining_before_s=5583 · 창 시작 2026-10-27T01:35:55Z 후 1h00m02s | docs/logs/main/events/2026-10.jsonl:741 |
| 86 | 2026-10-27T02:51:28Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → serve-gemma26b-ctx-single 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-27T01:35:55Z 후 1h15m33s) | docs/logs/main/events/2026-10.jsonl:742 |
| 87 | 2026-10-27T02:51:28Z | main | `serve_stop` | — | serve_stop · model=gemma-4-26b-a4b · duration_s=4533 · session=serve-gemma26b-ctx-single-20261027T013555Z | docs/logs/main/events/2026-10.jsonl:743 |
| 88 | 2026-10-27T02:51:28Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → serve-gemma26b-ctx-single 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-27T01:35:56Z 후 1h15m32s) | docs/logs/main/events/watchdog.jsonl:839 |

**기동 시도**(예산 선언 `budget_declare` 마다 한 시도 · 같은 노드의 이어지는 행과 선언 없는 노드(엔진 로그)의 그 시각 행을 묶었다 · 사살 · 트립 · 거부는 닫힘과 따로 센다 · `measurement` 행은 측정 기록이라 시도로 세지 않는다(위 표에만) — 해석은 저작자 몫)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 1 | main | `serve-gemma26b-ctx-single` | 2026-10-27T01:35:55Z | 18회 · 첫 2026-10-27T01:35:56Z | 없음 | `budget_clear` 2026-10-27T02:51:28Z | 12 |

**캠페인 경계 이전 기동 4회**(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀 라벨로 묶인 2회 · 라벨 다름(같은 노드 · 창 밖 이 셀 행과 시간이 겹치는 다른 라벨 선언 창) 2회 · 어느 쪽도 이 셀의 시도로 세지 않는다 · 행 전문 = 01 §1.4)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 앞선-1 | main | `camp-26102401-gemma26b`(라벨 다름 — 같은 노드 · 이 셀 라벨 아님) | 2026-10-23T21:48:33Z | 1회 · 첫 2026-10-23T21:48:34Z | 없음 | `budget_clear` 2026-10-23T22:03:55Z | 5 |
| 앞선-2 | main | `camp-26102401-gemma26b`(라벨 다름 — 같은 노드 · 이 셀 라벨 아님) | 2026-10-23T23:52:08Z | 8회 · 첫 2026-10-23T23:52:10Z | 없음 | `budget_clear` 2026-10-24T03:50:45Z | 5 |
| 앞선-3 | main | `serve-gemma26b-ctx-single` | 2026-10-25T00:02:06Z | 1회 · 첫 2026-10-25T00:02:08Z | 없음 | `budget_clear` 2026-10-25T00:09:42Z | 5 |
| 앞선-4 | main | `serve-gemma26b-ctx-single` | 2026-10-25T00:11:00Z | 17회 · 첫 2026-10-25T00:11:01Z | 없음 | `budget_clear` 2026-10-25T00:44:20Z | 5 |
<!-- /FACT:event_timeline -->

<!-- FACT:measurement_env -->
_측정 실행의 env 관측 없음 — 측정 엔진 로그에서 env echo 줄(`<KEY> set by environment to <VALUE>` 등)을 찾지 못했다(evidence.measurement_env_observed · 부재 ≠ 미설정)._
<!-- /FACT:measurement_env -->

> 이 절이 답하는 질문: 수신자가 이 zip 만으로 이 셀을 다시 띄우려면 어떤 순서로 무엇을 하고, 각 단계의 성공을 무엇으로 판정하며, 얼마나 기다려야 하는가?

1. managedNAS기존체크포인트를read-only마운트한다. env형상은compose .env.template과셀env를따른다. PLE스테이징은없다.
2. 기존0.31.0이미지로기동했고새빌드는없다. FACT층창은빌드벽시계소요가아니며정확리비전검증표와함께읽는다.
3. single_serve_up에overhead18432MiB·ready-max900을명시했고이번health대기는365초다. 기동시도/닫힘은현재FACT, 새completionPASS는testlog를따른다. 전용tool/reasoning시험은capability미선언N/A다.
4. 선언예산과실제engineweights/KV/graph는testlog_26102711_gemma26b_ctx_single_코덱스_메인_Full_판정 §1·§4에서나란히기록한다. 과거watchdog9988/9962MiB나oldfloor를현재측정으로옮기지않는다.
5. 이번Full은lite→GuideLLM1/2/4/8/16각3회요청형상이며maxseq미선언과실행중적응상한의결과는현재리포트가정본이다. 신규측정키2026-10-27T02:09:53Z와posthealth/실패/PASS를함께확인한다.
6. TP1이라Ray/NCCL통신경로는없다. attention backend는셀env에선언하지않았으며TRITON_ATTN은실제로그관측이다. FlashInfer CUTLASS Unquantized MoE선택관측도정적moe_backend기재NA와구분한다. fp8정확도경고는벤치가통과해도품질검증으로해소되지않는다.

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

### 가중치
획득은managedNAS기존자산재사용이며다운로드하지않았다. google/gemma-4-26B-A4B-it revision은00 §0.2정본이다. `<manifest.nas_model_path>/Google/Gemma_4/gemma-4-26B-A4B-it/config.json`읽기와compose마운트가준비조건이다. `test -r <manifest.nas_model_path>/Google/Gemma_4/gemma-4-26B-A4B-it/config.json`은준비절차예시이며이명령자체신규실행검증주장은아니다. index48.07GiB와엔진loading48.54GiB는방법을구분한다.

### 스테이징과네트워크
PLE는해당없고추가변환산출물도없다. GuideLLM토크나이저스테이징은도구가한다. 이번이미지/클라이언트/가중치는기존자산이다. 새source빌드에는basepull·apt·git·pip접속이필요하므로오프라인환경은승인된이미지와체크포인트를미리준비한다. 실행하지않은새다운로드명령을검증된것처럼싣지않는다.
