# 01 · 산출물 — 무엇이 실제로 쓰였고, 어떻게 다시 띄우는가

## 1.1 적용 판정 표
> 기계가 적용 증거(빌드 원장 · 원장이 없으면 라벨된 재구성) × 파일 × 선언을 대사한 결과다. `artifacts/` 에는 **적용된 것만** 실렸다 — 빌드 때 스스로 skip 된 패치와 쓰이지 않은 평면의 파일은 표에만 남고 싣지 않는다.

<!-- FACT:slots -->
| 슬롯 | 실림 | 파일 | 신호 | 적용 증거 | 사유 |
|---|---|---|---|---|---|
| `build_patch_post` | 안 실림 | — | 1-signal | build-ledger:docker-image(main-local):sha256:339737df6571ce00993e731ef064a8db66085beaa650e4aa63bdb24f414b2034:/opt/easy-vllm/build_ledger.json | 적용 판정 없음 — 원장 관측(기계 파생) |
| `build_patch_pre` | 안 실림 | — | 1-signal | build-ledger:docker-image(main-local):sha256:339737df6571ce00993e731ef064a8db66085beaa650e4aa63bdb24f414b2034:/opt/easy-vllm/build_ledger.json | 적용 판정 없음 — 원장 관측(기계 파생) |
| `build_recipe` | 실림 | `artifacts/build_recipe/Dockerfile.source-build` · `artifacts/build_recipe/requirements.txt` | 3-signal | build-ledger:build-ledger.dockerfile | Dockerfile.source-build의 검증 지위는 위 표를 따른다. 이미지history16/17일치·generated-unverified 경고가 있으면 정확 빌드 바이트로 승격하지 않는다. requirements는 원장hash 일치로verified다. source 트랙은 NGC alpha torch와 prebuilt wheel ABI 벽에 따른 공유 이미지의 선택이다. |
| `compose` | 실림 | `artifacts/compose/.env.template` · `artifacts/compose/docker-compose.yaml` | 2-signal | file:output/single/docker-compose.yaml | compose와env형상은 이미지·마운트·포트·자원 배선이다. 측정 전 선택 리비전과 당시env가 정본이다. 다른 호스트에서는 물리경로를 새로대조한다. |
| `fork_pin` | 안 실림 | — | 1-signal | none:.claude/policies/arch_variant_ledger.json | 셀 env 에 VARIANT 줄 없음 = stock(workflow.md §변종 좌표의 거처)(기계 파생) |
| `runtime_patch` | 안 실림 | — | 1-signal | none:output/single/configs/granite42-8b-v031_patch.py | output/single/configs/granite42-8b-v031_patch.py 없음 — arm_patch.sh 는 no-op(런타임 패치 불해당)(기계 파생) |
| `triplet` | 실림 | `artifacts/triplet/.env.granite42-8b-v031` · `artifacts/triplet/granite42-8b-v031.sh` · `artifacts/triplet/granite42-8b-v031.yaml` | 3-signal | file:output/single/configs/granite42-8b-v031.yaml | yaml·runner·셀env가 이번실행입력이다. 내부8000과호스트8001은매핑으로구분한다. runner의qwen3_coder/granite_thinking_parser는이번전용tool/reasoning시험PASS를뜻하지않는다. |

**레시피 경고 · 실린 리비전**(기계 관측 — 수신자 빌드가 여기서 갈라질 수 있다)

- ⚠ `build_recipe` `output/single/Dockerfile.source-build` — 실린 바이트(worktree)가 측정 이미지 docker history 의 RUN·COPY 와 전수 일치하지 않는다(16/17) · 이미지 빌드 뒤 바뀐 레시피일 수 있다 — 마지막 커밋이 이미지 Created 뒤(이미지 Created 2026-10-14T01:45:19Z · 마지막 커밋 2026-10-14T03:07:26Z)
- `build_recipe` `output/single/Dockerfile.source-build` 실린 리비전: 리비전 미특정 · 선택 방법 `worktree(어느 개정도 이미지 history 와 더 맞지 않는다 · 작업트리 16/17 · 전 이력(output/*/Dockerfile.source-build · 모든 ref)에도 전수 일치 개정 없음 — recipe_vs_image 경고 유지)` · docker history 대조 16/17 · 실린 바이트 `worktree` · 쓰인 바이트임을 관측으로 확인 아니오
- `build_recipe` `output/single/requirements.txt` 실린 리비전: 리비전 미특정 · 선택 방법 `worktree(빌드 원장 sha256 일치 — 쓰인 바이트 관측 확인 = 예)` · docker history 대조 없음 · 실린 바이트 `worktree` · 쓰인 바이트임을 관측으로 확인 예
- `compose` `output/single/docker-compose.yaml` 실린 리비전: 커밋 `2199257f7f71` · 선택 방법 `worktree(mtime 2026-10-26T21:40:37Z ≤ 측정 2026-10-26T23:00:21Z(sweep_index.generated_utc) · 바이트 = 측정 전 마지막 커밋 2199257f7f71)` · docker history 대조 없음 · 실린 바이트 `worktree` · 근거 `mtime≤measured` · 쓰인 바이트임을 관측으로 확인 예

**싣지 않은 파일**(슬롯별 사유 — 빌드 때 skip 된 패치는 아래 적용 집합 표에 있다)

- `build_patch_post` known-non-patch 1건: `.gitkeep`
- `build_patch_pre` known-non-patch 1건: `.gitkeep`
- `compose` `arm_patch.sh` — `not-armed(output/single/configs/granite42-8b-v031_patch.py 없음(CONFIG_FILE) — arm_patch.sh 는 no-op · arm_patch.sh 측정 당시 개정(c8bb20e77e52) `_cfg=` 키 체인 RUNTIME_PATCH_STEM > CONFIG_FILE > 'default')`

**파일별 검증 · 관련성**(`generated-unverified` = 실행 검증되지 않은 생성물 — 파일 첫 줄 경고 · PAYLOAD 파일 기록과 같은 표시 · 관련성 = ① 패치 선언 대상 × 모델 config ② 엔진 로그 발화 서명 — 둘 다 관측일 때만 required/inactive-inferred)

| 슬롯 | 파일 | 검증 | 검증 근거 | 관련성 | 관련성 근거 |
|---|---|---|---|---|---|
| `build_recipe` | `artifacts/build_recipe/Dockerfile.source-build` | `generated-unverified` · renderer | 실린 Dockerfile.source-build(worktree)이 측정 이미지 docker history RUN·COPY 와 16/17 만 일치 — 이 이미지를 지은 바이트임을 관측으로 확인하지 못했다(_select_dockerfile · worktree(어느 개정도 이미지 history 와 더 맞지 않는다 · 작업트리 16/17 · 전 이력(output/*/Dockerfile.source-build · 모든 ref)에도 전수 일치 개정 없음 — recipe_vs_image 경고 유지)) | `unknown` | 이 셀은 이 경로로 실행되지 않았다(generated-unverified · 생성 renderer) — 실행 기록이 없어 필요 여부를 판정하지 않는다(참고 형상) |
| `build_recipe` | `artifacts/build_recipe/requirements.txt` | `verified` · cell-run | 쓰인 레시피(build-ledger.dockerfile) — 개정 선택 slots.build_recipe.evidence.selected_revisions | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/.env.template` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/docker-compose.yaml` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/.env.granite42-8b-v031` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/granite42-8b-v031.sh` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/granite42-8b-v031.yaml` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
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
| attestation | absent | output/single/benchlog/attestation_granite42-8b-v031.json(부재) |
| local-image | observed | sha256:339737df6571ce00993e731ef064a8db66085beaa650e4aa63bdb24f414b2034 |
<!-- /FACT:applied_set -->

## 1.2 슬롯별 적용 사유
> 이 절이 답하는 질문: 실린 산출물 하나하나는 무엇을 고치며, 왜 이 셀에 필요했고, 이 셀에서 실제로 발화했으며, 언제 불필요해지는가?

### build_recipe
Dockerfile.source-build의 검증 지위는 위 표를 따른다. 이미지history16/17일치·generated-unverified 경고가 있으면 정확 빌드 바이트로 승격하지 않는다. requirements는 원장hash 일치로verified다. source 트랙은 NGC alpha torch와 prebuilt wheel ABI 벽에 따른 공유 이미지의 선택이다.

### compose
compose와env형상은 이미지·마운트·포트·자원 배선이다. 측정 전 선택 리비전과 당시env가 정본이다. 다른 호스트에서는 물리경로를 새로대조한다.

### triplet
yaml·runner·셀env가 이번실행입력이다. 내부8000과호스트8001은매핑으로구분한다. runner의qwen3_coder/granite_thinking_parser는이번전용tool/reasoning시험PASS를뜻하지않는다.

### 불해당 슬롯
build_patch_pre/post는빈슬롯이고inline strip-hoist는원장에skipped로기록된다. fork_pin은VARIANT부재, runtime_patch는셀patch.py부재다. 이를모델고유적용패치나활성패치로부르지않는다. 상위버전에서도불필요하다는일반화는하지않는다.

## 1.3 값의 지위표
> 서빙 설정의 노브 **전부**와 기계가 모은 후보 지위다(lockset 출처 · yaml 주석 · 엔진 기본값). 최종 지위는 아래 `kind: value-status` 블록이 정한다 — 표의 노브마다 블록이 정확히 하나 있어야 린터를 통과한다.

<!-- FACT:value_status -->
서빙 노브 4개 — 노브마다 `kind: value-status` 블록이 **정확히 1개** 있어야 한다(노브·값은 이 표의 글자 그대로 · 값 칸의 `\|` 는 `|` 로 적는다).

| 노브 | 값 | 후보 지위 | 후보 근거 | yaml 주석 | 출처 |
|---|---|---|---|---|---|
| `gpu-memory-utilization` | 0.39 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `max-model-len` | 32768 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `max-num-seqs` | 8 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `kv-cache-memory-bytes` | 25232932864 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
<!-- /FACT:value_status -->

> 이 절이 답하는 질문: 각 서빙 노브의 값은 이 셀에서 어떤 지위인가 — 조정됐나, 승계됐나, 음성대조로 일부러 둔 것인가, 엔진 기본값인가, 필요조건인가?

```hint-event
id: V1
kind: value-status
노브: "gpu-memory-utilization"
값: "0.39"
지위: "inherited"
근거: "과거64GB목표와두모델공존의호스트환산손값승계; 이번조정없음"
출처: [plan_26101410_vllm031_bump_14B이하_4종_양노드_서빙_lite §4, testlog_26102708_granite42_8b_v031_코덱스_메인_Full_판정 §1]
```

```hint-event
id: V2
kind: value-status
노브: "max-model-len"
값: "32768"
지위: "inherited"
근거: "사람RAG길이요구승계; 다른값대조없음"
출처: [plan_26101410_vllm031_bump_14B이하_4종_양노드_서빙_lite §4, testlog_26102708_granite42_8b_v031_코덱스_메인_Full_판정 §1]
```

```hint-event
id: V3
kind: value-status
노브: "max-num-seqs"
값: "8"
지위: "inherited"
근거: "사람동시성요구8승계; 계획산식의실행관측은주장하지않음"
출처: [plan_26101410_vllm031_bump_14B이하_4종_양노드_서빙_lite §4, testlog_26102708_granite42_8b_v031_코덱스_메인_Full_판정 §1]
```

```hint-event
id: V4
kind: value-status
노브: "kv-cache-memory-bytes"
값: "25232932864"
지위: "inherited"
근거: "과거목표천장의max_safe손산정승계; 이번조정없음"
출처: [plan_26101410_vllm031_bump_14B이하_4종_양노드_서빙_lite §4, testlog_26102708_granite42_8b_v031_코덱스_메인_Full_판정 §1]
```

네 값은 이번에 조정하지 않은 승계값이다. 현재 형상의 신규 Full을 확인한 것이며 최적성이나 제거 시 실패를 시험한 것이 아니다. 다른 메모리 구조에서는 특히 gmu와 절대 KV를 다시 도출한다.

## 1.4 재현 절차
> 셀 실행 기록에서 기계가 파생한 명령 순서와 단계별 소요다. 소요가 `미관측` 인 단계는 시각 기록이 없다는 뜻이다(0 이 아니다). 그 아래는 블랙박스 이벤트 원장에서 이 셀의 행을 기계가 뽑아 기동 시도로 묶은 것이고, 마지막은 측정한 실행의 엔진 로그가 echo 한 env 다(측정 당시 값의 정본).

<!-- FACT:reproduce -->
| 순서 | 단계 | 성공 판정 | 소요 | 관측 구간(UTC) | 소요 출처 | 명령 출처 |
|---|---|---|---|---|---|---|
| 1 | build | 빌드 OK · 이미지 실재 | 층 창 38분 19초(docker history 첫 층 → 끝 층 CreatedAt · 캐시 재사용 층 포함 — 빌드 소요 아님) | 2026-10-14T01:07:00Z → 2026-10-14T01:45:19Z (layer-window) | docker history --no-trunc --format '{{json .}}' sha256:339737df6571ce00993e731ef064a8db66085beaa650e4aa63bdb24f414b2034 · 우리 Dockerfile 층 24개의 CreatedAt 첫 층 → 끝 층(캐시 재사용 층은 이전 빌드 시각 — 빌드 소요 아님) | reconstructed(build-ledger build_args + compose build) |
| 2 | serve | health 200 + 추론 1회(스모크 판정) | ≤ 7분 12초(상한) | 2026-10-26T22:14:38Z → 2026-10-26T22:21:50Z (upper) | docs/logs/main/sessions/serve-granite42-8b-v031-20261026T221438Z.json started_utc(측정을 포함하는 세션) → 첫 측정 시각(output/single/benchlog/sweep_granite42-8b-v031/lite_raw_granite42-8b-v031.json measured_utc(호스트 date -u · lite_bench.sh 가 cold 레그 직전에 찍는 측정 시작)) | derived(스모크 사용법) |
| 3 | bench | 리포트 발행(루브릭 권한 explicit ∧ PASS일 때만 full 인증서) | 38분 31초 | 2026-10-26T22:21:50Z → 2026-10-26T23:00:21Z (exact) | output/single/benchlog/sweep_granite42-8b-v031/lite_raw_granite42-8b-v031.json measured_utc(호스트 date -u · lite_bench.sh 가 cold 레그 직전에 찍는 측정 시작) → sweep_index.generated_utc(호스트 UTC · 두 끝 같은 시계) | reconstructed(bench json + 측정 도구 원문 lite 인자) |

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
# 동등 경로(compose): docker compose -f output/single/docker-compose.yaml --env-file output/single/envs/.env.granite42-8b-v031 build
```

**2. serve**

```sh
bash .claude/skills/upstream-version-watch/scripts/single_serve_up.sh granite42-8b-v031
```

**3. bench**

```sh
# bench JSON 필드로 복원: backend · model_id→--model · tokenizer_id→--tokenizer · num_prompts · max_concurrency · request_rate · burstiness · 요청당 입출력 길이 = total_*_tokens ÷ completed(나누어떨어질 때만 — 평균이다 · 요청별 길이는 JSON 에 없다) · 나누어떨어지면 `--dataset-name random` 으로 추정했다(측정 도구가 그 꼴로 부른다 — 아래 도구 원문).
# JSON 에 없는 인자(--endpoint · --base-url · --ignore-eos · --num-warmups · --temperature · --seed · --random-range-ratio · --trust-remote-code)는 측정 도구가 정했다 — level_* = run_bench.sh · lite_cold/lite_warm = lite_bench.sh(.claude/skills/adversarial-benchmark/scripts/) · 셀 스윕 순서는 sweep_bench.sh.
# lite_cold/lite_warm 의 데이터셋 · 부하 인자 = 측정 시점 도구 원문 lite_bench.sh@90a3cdd56a94 L177-L183(리터럴 인자만 · `$변수` 인자는 도구가 값을 정하지 않아 JSON 필드로 채운다) — `total_*_tokens ÷ completed` 는 실측 토큰(chat 템플릿 포함)이지 인자가 아니다.
# output/single/benchlog/sweep_granite42-8b-v031/lite_cold_granite42-8b-v031.json (date 20261026-222209) · 실측 토큰(chat 템플릿 포함 · total ÷ completed · 인자 아님): 입력 527/1 = 527 · 출력 128/1 = 128
vllm bench serve --backend openai-chat --model granite-4.2-8b --tokenizer /app/models/IBM/Granite-4.2/granite-4.2-8b --trust-remote-code --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 --ignore-eos --num-prompts 1 --max-concurrency 1 --request-rate inf
# output/single/benchlog/sweep_granite42-8b-v031/lite_warm_granite42-8b-v031.json (date 20261026-222259) · 실측 토큰(chat 템플릿 포함 · total ÷ completed · 인자 아님): 입력 1581/3 = 527 · 출력 384/3 = 128
vllm bench serve --backend openai-chat --model granite-4.2-8b --tokenizer /app/models/IBM/Granite-4.2/granite-4.2-8b --trust-remote-code --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 --ignore-eos --num-prompts 3 --max-concurrency 1 --request-rate inf
```

**슬롯 → 빌드 컨텍스트 매핑**(빌드 컨텍스트 = `output/<토폴로지>/` · 원천 = 실린 Dockerfile 의 COPY · compose 의 env_file · volumes · command — `—` 는 빌드 컨텍스트 입력이 아니다)

| zip 경로 | 빌드 컨텍스트 자리 | 근거 |
|---|---|---|
| `artifacts/triplet/.env.granite42-8b-v031` | `envs/.env.granite42-8b-v031` | compose env_file envs/.env.${CONFIG_FILE}(셀 env) |
| `artifacts/triplet/granite42-8b-v031.sh` | `configs/granite42-8b-v031.sh` | compose volumes ./configs:/app/configs |
| `artifacts/triplet/granite42-8b-v031.yaml` | `configs/granite42-8b-v031.yaml` | compose volumes ./configs:/app/configs |
| `artifacts/build_recipe/Dockerfile.source-build` | `Dockerfile.source-build` | compose build.dockerfile Dockerfile.source-build(리터럴) · build.context . |
| `artifacts/build_recipe/requirements.txt` | `requirements.txt` | Dockerfile.source-build COPY requirements.txt |
| `artifacts/compose/.env.template` | `.env` | compose 변수치환 프로젝트 .env(${…} 참조) |
| `artifacts/compose/docker-compose.yaml` | `docker-compose.yaml` | compose 파일(빌드 컨텍스트 루트) |
| (실린 파일 없음 — 빈 디렉터리 생성) | `build_patches_src/` | Dockerfile.source-build COPY build_patches_src/ · 실린 패치 없음 — 빈 디렉터리를 만든다(없으면 COPY 단계에서 빌드가 멈춘다) · 빌드 원장 context_inventory.build_patch_pre = ['.gitkeep'] |
| (실린 파일 없음 — 빈 디렉터리 생성) | `build_patches/` | Dockerfile.source-build COPY build_patches/ · 실린 패치 없음 — 빈 디렉터리를 만든다(없으면 COPY 단계에서 빌드가 멈춘다) · 빌드 원장 context_inventory.build_patch_post = ['.gitkeep'] |

**기동 성공 판정(관측 · 발행 자격)**: health 200 = true · 추론 1회 = true · 근거 `output/single/benchlog/sweep_granite42-8b-v031/level_01/post_health_granite42-8b-v031.json` · `output/single/benchlog/sweep_granite42-8b-v031/level_01/guidellm_granite42-8b-v031.json`
<!-- /FACT:reproduce -->

<!-- FACT:event_timeline -->
**블랙박스 이벤트 100행**(원장에서 이 셀 label·config 에 맞는 행과 그와 시간이 겹치는 같은 노드의 다른 라벨 창 · 시각순 · 기계 발췌 · 라벨 다름(같은 노드 · 창 밖 이 셀 행과 시간이 겹치는 다른 라벨 선언 창) 66행 — 이 셀의 선언으로 세지 않는다)

> **캠페인 경계 미관측** — 재생 모드(캠페인 camp-26102705-codex-single8 인스턴스 보존 · 재생 모드는 읽지 않았다) — 캠페인 선언을 관측하지 못한다 · 경계 미관측 — 라벨(셀 이름)로 묶은 행 전부를 이 셀의 행으로 센다(같은 셀 이름의 앞선 캠페인 행이 섞였을 수 있다)

> **원장 관측 범위**(노드별 첫 행 → 마지막 행 · evidence.event_ledger_spans): main = 2026-07-31T03:27:43Z → 2026-10-27T02:51:28Z(`docs/logs/main/events/2026-07.jsonl` · `docs/logs/main/events/2026-08.jsonl` · `docs/logs/main/events/2026-09.jsonl` · `docs/logs/main/events/2026-10.jsonl` · `docs/logs/main/events/thermal.jsonl` · `docs/logs/main/events/watchdog.jsonl`)

| # | UTC | 노드 | kind | label | 내용 | 출처 |
|---|---|---|---|---|---|---|
| 1 | 2026-10-14T01:59:39Z | main | `budget_declare` | `serve-granite42-8b-v031` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=76176 · weights_mib=16768 · kv_mib=24064 · overhead_mib=7600 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 기동 창 1/2: budget_renew 1회 · 닫힘 replaced(budget_declare) 2026-10-14T02:04:02Z(선언 후 4m23s) · measured_utc 2026-10-26T23:00:21Z 는 이 창 밖(다른 기동 · 측정 전) | docs/logs/main/events/2026-10.jsonl:373 |
| 2 | 2026-10-14T01:59:39Z | main | `serve_start` | — | serve_start · model=granite-4.2-8b · tensor_parallel_size=1 · topology=single · session=serve-granite42-8b-v031-20261014T015939Z | docs/logs/main/events/2026-10.jsonl:374 |
| 3 | 2026-10-14T01:59:40Z | main | `budget_honored` | `serve-granite42-8b-v031` | budget_honored · floor_mib=76176 · arm_ceiling_mib=73104 · remaining_s=86399 · 창 시작 2026-10-14T01:59:40Z 후 0m00s | docs/logs/main/events/watchdog.jsonl:726 |
| 4 | 2026-10-14T01:59:41Z | main | `budget_renew` | `serve-granite42-8b-v031` | budget_renew · floor_mib=76176 · ttl_s=86400 · remaining_before_s=86398 · 창 시작 2026-10-14T01:59:39Z 후 0m02s | docs/logs/main/events/2026-10.jsonl:375 |
| 5 | 2026-10-14T02:04:02Z | main | `budget_declare` | `serve-main-combined-granite-minicpm` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=23966 · weights_mib=4800 · kv_mib=35840 · overhead_mib=60002 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · budget_renew 64회 · 닫힘 budget_clear 2026-10-17T11:36:13Z(선언 후 81h32m11s) · measured_utc 2026-10-26T23:00:21Z 는 이 창 밖(측정 전) | docs/logs/main/events/2026-10.jsonl:376 |
| 6 | 2026-10-14T02:04:03Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=86400 · remaining_before_s=86399 · 창 시작 2026-10-14T02:04:02Z 후 0m01s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:378 |
| 7 | 2026-10-14T08:04:03Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=86400 · remaining_before_s=64800 · 창 시작 2026-10-14T02:04:02Z 후 6h00m01s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:379 |
| 8 | 2026-10-14T14:04:03Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=86400 · remaining_before_s=64800 · 창 시작 2026-10-14T02:04:02Z 후 12h00m01s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:380 |
| 9 | 2026-10-14T20:04:03Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=86400 · remaining_before_s=64800 · 창 시작 2026-10-14T02:04:02Z 후 18h00m01s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:384 |
| 10 | 2026-10-15T02:04:03Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=86400 · remaining_before_s=64800 · 창 시작 2026-10-14T02:04:02Z 후 24h00m01s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:385 |
| 11 | 2026-10-15T08:04:03Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=86400 · remaining_before_s=64800 · 창 시작 2026-10-14T02:04:02Z 후 30h00m01s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:386 |
| 12 | 2026-10-15T14:04:03Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=86400 · remaining_before_s=64800 · 창 시작 2026-10-14T02:04:02Z 후 36h00m01s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:387 |
| 13 | 2026-10-15T20:04:04Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=86400 · remaining_before_s=64799 · 창 시작 2026-10-14T02:04:02Z 후 42h00m02s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:391 |
| 14 | 2026-10-16T02:04:04Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=86400 · remaining_before_s=64800 · 창 시작 2026-10-14T02:04:02Z 후 48h00m02s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:392 |
| 15 | 2026-10-16T06:22:00Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=70924 · 창 시작 2026-10-14T02:04:02Z 후 52h17m58s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:393 |
| 16 | 2026-10-16T06:22:00Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7200 · 창 시작 2026-10-14T02:04:02Z 후 52h17m58s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:394 |
| 17 | 2026-10-16T06:22:40Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7160 · 창 시작 2026-10-14T02:04:02Z 후 52h18m38s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:395 |
| 18 | 2026-10-16T06:23:33Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7147 · 창 시작 2026-10-14T02:04:02Z 후 52h19m31s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:396 |
| 19 | 2026-10-16T06:24:29Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7144 · 창 시작 2026-10-14T02:04:02Z 후 52h20m27s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:397 |
| 20 | 2026-10-16T06:47:05Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=5844 · 창 시작 2026-10-14T02:04:02Z 후 52h43m03s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:398 |
| 21 | 2026-10-16T06:54:03Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=6782 · 창 시작 2026-10-14T02:04:02Z 후 52h50m01s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:399 |
| 22 | 2026-10-16T07:00:24Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=6819 · 창 시작 2026-10-14T02:04:02Z 후 52h56m22s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:400 |
| 23 | 2026-10-16T07:06:45Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=6819 · 창 시작 2026-10-14T02:04:02Z 후 53h02m43s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:401 |
| 24 | 2026-10-16T07:09:55Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7010 · 창 시작 2026-10-14T02:04:02Z 후 53h05m53s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:402 |
| 25 | 2026-10-16T07:13:05Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7010 · 창 시작 2026-10-14T02:04:02Z 후 53h09m03s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:403 |
| 26 | 2026-10-16T07:16:16Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7009 · 창 시작 2026-10-14T02:04:02Z 후 53h12m14s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:404 |
| 27 | 2026-10-16T07:18:08Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7088 · 창 시작 2026-10-14T02:04:02Z 후 53h14m06s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:405 |
| 28 | 2026-10-16T07:20:00Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7088 · 창 시작 2026-10-14T02:04:02Z 후 53h15m58s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:406 |
| 29 | 2026-10-16T07:21:53Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7087 · 창 시작 2026-10-14T02:04:02Z 후 53h17m51s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:407 |
| 30 | 2026-10-16T07:23:07Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7126 · 창 시작 2026-10-14T02:04:02Z 후 53h19m05s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:408 |
| 31 | 2026-10-16T07:24:22Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7125 · 창 시작 2026-10-14T02:04:02Z 후 53h20m20s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:409 |
| 32 | 2026-10-16T07:25:36Z | 미관측 | `measurement` | `measurement (other label's budget window)` | 같은 셀의 다른 측정(측정 노드 축 main) · 시각 = 측정 조립(sweep generated_utc = 인증서 measured_utc · 시작 시각 미기록) · 이 발행의 측정 2026-10-26T23:00:21Z 전 · 발행 기록 camp_26101616_cnn_hint__main__granite42_8b_v031(셀 대조 simlog:sweep_index.meta.config_name · verdict PASS) · simlog 사본 generated_utc 2026-10-16T07:25:36Z(같음) · 원장: 이 셀 라벨의 선언은 없고 다른 라벨의 창 안: serve-main-combined-granite-minicpm 2026-10-14T02:04:02Z~2026-10-17T11:36:13Z (docs/logs/main/events/2026-10.jsonl:376) · 직전 예산 행 budget_renew(serve-main-combined-granite-minicpm) 2026-10-16T07:24:22Z(docs/logs/main/events/2026-10.jsonl:409) · 직후 예산 행 budget_renew(serve-main-combined-granite-minicpm) 2026-10-16T07:26:49Z(docs/logs/main/events/2026-10.jsonl:410) | docs/simlog/26101620_camp_26101616_cnn_hint__main__granite42_8b_v031/sweep_index.json:4 |
| 33 | 2026-10-16T07:26:49Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7053 · 창 시작 2026-10-14T02:04:02Z 후 53h22m47s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:410 |
| 34 | 2026-10-16T07:29:41Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7028 · 창 시작 2026-10-14T02:04:02Z 후 53h25m39s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:411 |
| 35 | 2026-10-16T07:31:48Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7073 · 창 시작 2026-10-14T02:04:02Z 후 53h27m46s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:412 |
| 36 | 2026-10-16T07:33:57Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7071 · 창 시작 2026-10-14T02:04:02Z 후 53h29m55s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:413 |
| 37 | 2026-10-16T07:34:55Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7142 · 창 시작 2026-10-14T02:04:02Z 후 53h30m53s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:414 |
| 38 | 2026-10-16T07:35:54Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7141 · 창 시작 2026-10-14T02:04:02Z 후 53h31m52s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:415 |
| 39 | 2026-10-16T07:36:52Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7142 · 창 시작 2026-10-14T02:04:02Z 후 53h32m50s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:416 |
| 40 | 2026-10-16T07:37:28Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7164 · 창 시작 2026-10-14T02:04:02Z 후 53h33m26s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:417 |
| 41 | 2026-10-16T07:38:05Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7163 · 창 시작 2026-10-14T02:04:02Z 후 53h34m03s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:418 |
| 42 | 2026-10-16T07:38:41Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7164 · 창 시작 2026-10-14T02:04:02Z 후 53h34m39s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:419 |
| 43 | 2026-10-16T07:39:07Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7174 · 창 시작 2026-10-14T02:04:02Z 후 53h35m05s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:420 |
| 44 | 2026-10-16T07:39:32Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7175 · 창 시작 2026-10-14T02:04:02Z 후 53h35m30s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:421 |
| 45 | 2026-10-16T07:41:46Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7066 · 창 시작 2026-10-14T02:04:02Z 후 53h37m44s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:422 |
| 46 | 2026-10-16T07:43:55Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7071 · 창 시작 2026-10-14T02:04:02Z 후 53h39m53s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:423 |
| 47 | 2026-10-16T07:46:04Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7071 · 창 시작 2026-10-14T02:04:02Z 후 53h42m02s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:424 |
| 48 | 2026-10-16T07:48:12Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7072 · 창 시작 2026-10-14T02:04:02Z 후 53h44m10s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:425 |
| 49 | 2026-10-16T07:49:12Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7140 · 창 시작 2026-10-14T02:04:02Z 후 53h45m10s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:426 |
| 50 | 2026-10-16T07:50:11Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7141 · 창 시작 2026-10-14T02:04:02Z 후 53h46m09s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:427 |
| 51 | 2026-10-16T07:51:10Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7141 · 창 시작 2026-10-14T02:04:02Z 후 53h47m08s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:428 |
| 52 | 2026-10-16T07:51:48Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7162 · 창 시작 2026-10-14T02:04:02Z 후 53h47m46s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:429 |
| 53 | 2026-10-16T07:52:25Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7163 · 창 시작 2026-10-14T02:04:02Z 후 53h48m23s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:430 |
| 54 | 2026-10-16T07:53:03Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7162 · 창 시작 2026-10-14T02:04:02Z 후 53h49m01s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:431 |
| 55 | 2026-10-16T07:53:29Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7174 · 창 시작 2026-10-14T02:04:02Z 후 53h49m27s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:432 |
| 56 | 2026-10-16T07:53:56Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7173 · 창 시작 2026-10-14T02:04:02Z 후 53h49m54s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:433 |
| 57 | 2026-10-16T08:04:04Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=86400 · remaining_before_s=6592 · 창 시작 2026-10-14T02:04:02Z 후 54h00m02s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:434 |
| 58 | 2026-10-16T14:04:04Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=86400 · remaining_before_s=64800 · 창 시작 2026-10-14T02:04:02Z 후 60h00m02s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:435 |
| 59 | 2026-10-16T14:05:33Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=86311 · 창 시작 2026-10-14T02:04:02Z 후 60h01m31s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:436 |
| 60 | 2026-10-16T14:11:52Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=6821 · 창 시작 2026-10-14T02:04:02Z 후 60h07m50s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:437 |
| 61 | 2026-10-16T14:18:13Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=6819 · 창 시작 2026-10-14T02:04:02Z 후 60h14m11s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:438 |
| 62 | 2026-10-16T14:24:34Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=6819 · 창 시작 2026-10-14T02:04:02Z 후 60h20m32s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:439 |
| 63 | 2026-10-16T14:27:45Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7009 · 창 시작 2026-10-14T02:04:02Z 후 60h23m43s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:440 |
| 64 | 2026-10-16T14:30:56Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7009 · 창 시작 2026-10-14T02:04:02Z 후 60h26m54s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:441 |
| 65 | 2026-10-16T14:34:07Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7009 · 창 시작 2026-10-14T02:04:02Z 후 60h30m05s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:442 |
| 66 | 2026-10-16T14:36:00Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7087 · 창 시작 2026-10-14T02:04:02Z 후 60h31m58s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:443 |
| 67 | 2026-10-16T14:37:53Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7087 · 창 시작 2026-10-14T02:04:02Z 후 60h33m51s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:444 |
| 68 | 2026-10-16T14:39:46Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7087 · 창 시작 2026-10-14T02:04:02Z 후 60h35m44s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:445 |
| 69 | 2026-10-16T14:41:00Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7126 · 창 시작 2026-10-14T02:04:02Z 후 60h36m58s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:446 |
| 70 | 2026-10-16T14:42:14Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7126 · 창 시작 2026-10-14T02:04:02Z 후 60h38m12s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:447 |
| 71 | 2026-10-16T22:23:36Z | 미관측 | `measurement` | `measurement (other label's budget window)` | 같은 셀의 다른 측정(측정 노드 축 main) · 시각 = 측정 조립(sweep generated_utc = 인증서 measured_utc · 시작 시각 미기록) · 이 발행의 측정 2026-10-26T23:00:21Z 전 · 발행 기록 camp_26101623_baseline__main__granite42_8b_v031(셀 대조 simlog:sweep_index.meta.config_name · verdict PASS) · simlog 사본 generated_utc 2026-10-16T22:23:36Z(같음) · 원장: 이 셀 라벨의 선언은 없고 다른 라벨의 창 안: serve-main-combined-granite-minicpm 2026-10-14T02:04:02Z~2026-10-17T11:36:13Z (docs/logs/main/events/2026-10.jsonl:376) · 직전 예산 행 budget_expired 2026-10-16T16:42:14Z(docs/logs/main/events/watchdog.jsonl:729) · 직후 예산 행 budget_clear 2026-10-17T11:36:13Z(docs/logs/main/events/2026-10.jsonl:451) | docs/simlog/26101709_camp_26101623_baseline__main__granite42_8b_v031/sweep_index.json:4 |
| 72 | 2026-10-17T11:36:13Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → serve-main-combined-granite-minicpm 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-14T02:04:02Z 후 81h32m11s) · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`granite42-8b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:451 |
| 73 | 2026-10-17T11:37:04Z | main | `serve_stop` | — | serve_stop · model=granite-4.2-8b · duration_s=293845 · session=serve-granite42-8b-v031-20261014T015939Z | docs/logs/main/events/2026-10.jsonl:453 |
| 74 | 2026-10-26T22:14:38Z | main | `budget_declare` | `serve-granite42-8b-v031` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=76176 · weights_mib=16768 · kv_mib=24064 · overhead_mib=7600 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 기동 창 2/2: budget_renew 14회 · 닫힘 budget_clear 2026-10-26T23:06:48Z(선언 후 52m10s) · measured_utc 2026-10-26T23:00:21Z 가 이 창 안(이 측정의 기동) | docs/logs/main/events/2026-10.jsonl:683 |
| 75 | 2026-10-26T22:14:38Z | main | `serve_start` | — | serve_start · model=granite-4.2-8b · tensor_parallel_size=1 · topology=single · session=serve-granite42-8b-v031-20261026T221438Z | docs/logs/main/events/2026-10.jsonl:684 |
| 76 | 2026-10-26T22:14:38Z | main | `budget_honored` | `serve-granite42-8b-v031` | budget_honored · floor_mib=76176 · arm_ceiling_mib=73104 · remaining_s=7200 · 창 시작 2026-10-26T22:14:38Z 후 0m00s | docs/logs/main/events/watchdog.jsonl:832 |
| 77 | 2026-10-26T22:14:39Z | main | `budget_renew` | `serve-granite42-8b-v031` | budget_renew · floor_mib=76176 · ttl_s=7200 · remaining_before_s=7199 · 창 시작 2026-10-26T22:14:38Z 후 0m01s | docs/logs/main/events/2026-10.jsonl:685 |
| 78 | 2026-10-26T22:14:54Z | main | `engine_log_window_start` | — | 캡처된 엔진 로그의 첫 시각(꼬리 캡처일 수 있다 — 엔진 시작 시각이 아닐 수 있다) · 선언 2026-10-26T22:14:38Z 후 0m16s · 시계: 컨테이너 시각을 UTC 로 읽어 측정 창 [2026-10-26T22:14:38Z, 2026-10-26T23:06:48Z] 안에 드는 것을 확인 | output/single/benchlog/sweep_granite42-8b-v031/lite_engine_granite42-8b-v031.log:36 |
| 79 | 2026-10-26T22:15:05Z | main | `engine_prefetch_disabled` | — | 가중치 auto-prefetch 꺼짐 줄 ×1 — filesystem CIFS(vLLM 로그 문구) | output/single/benchlog/sweep_granite42-8b-v031/lite_engine_granite42-8b-v031.log:56 |
| 80 | 2026-10-26T22:16:47Z | main | `engine_weights_loaded` | — | 가중치 적재 완료 줄 ×1(rank·단계별) · 최장 102.64s · 이 행 = 마지막 줄 | output/single/benchlog/sweep_granite42-8b-v031/lite_engine_granite42-8b-v031.log:64 |
| 81 | 2026-10-26T22:16:48Z | main | `engine_model_loaded` | — | 모델 적재 완료 줄 ×1(rank 별) · 마지막 줄 16.42 GiB · 105.6s | output/single/benchlog/sweep_granite42-8b-v031/lite_engine_granite42-8b-v031.log:65 |
| 82 | 2026-10-26T22:16:52Z | main | `engine_kv_cache_sized` | — | GPU KV cache 154,000 tokens · 요청당 32,768 tokens 기준 최대 동시성 4.70x | output/single/benchlog/sweep_granite42-8b-v031/lite_engine_granite42-8b-v031.log:74 |
| 83 | 2026-10-26T22:18:01Z | main | `engine_init_done` | — | 엔진 초기화(profile·KV 할당·warmup) 73.21s | output/single/benchlog/sweep_granite42-8b-v031/lite_engine_granite42-8b-v031.log:88 |
| 84 | 2026-10-26T22:18:02Z | main | `server_starting` | — | API 서버 기동 — 이 측정 창 선언 2026-10-26T22:14:38Z 후 3m24s · 캡처 로그 첫 시각 2026-10-26T22:14:54Z 후 3m08s | output/single/benchlog/sweep_granite42-8b-v031/lite_engine_granite42-8b-v031.log:95 |
| 85 | 2026-10-26T22:23:00Z | main | `budget_renew` | `serve-granite42-8b-v031` | budget_renew · floor_mib=76176 · ttl_s=7200 · remaining_before_s=6699 · 창 시작 2026-10-26T22:14:38Z 후 8m22s | docs/logs/main/events/2026-10.jsonl:686 |
| 86 | 2026-10-26T22:29:21Z | main | `budget_renew` | `serve-granite42-8b-v031` | budget_renew · floor_mib=76176 · ttl_s=7200 · remaining_before_s=6819 · 창 시작 2026-10-26T22:14:38Z 후 14m43s | docs/logs/main/events/2026-10.jsonl:687 |
| 87 | 2026-10-26T22:35:37Z | main | `budget_renew` | `serve-granite42-8b-v031` | budget_renew · floor_mib=76176 · ttl_s=7200 · remaining_before_s=6824 · 창 시작 2026-10-26T22:14:38Z 후 20m59s | docs/logs/main/events/2026-10.jsonl:688 |
| 88 | 2026-10-26T22:41:53Z | main | `budget_renew` | `serve-granite42-8b-v031` | budget_renew · floor_mib=76176 · ttl_s=7200 · remaining_before_s=6824 · 창 시작 2026-10-26T22:14:38Z 후 27m15s | docs/logs/main/events/2026-10.jsonl:689 |
| 89 | 2026-10-26T22:44:39Z | main | `budget_renew` | `serve-granite42-8b-v031` | budget_renew · floor_mib=76176 · ttl_s=7200 · remaining_before_s=7034 · 창 시작 2026-10-26T22:14:38Z 후 30m01s | docs/logs/main/events/2026-10.jsonl:690 |
| 90 | 2026-10-26T22:45:01Z | main | `budget_renew` | `serve-granite42-8b-v031` | budget_renew · floor_mib=76176 · ttl_s=7200 · remaining_before_s=7178 · 창 시작 2026-10-26T22:14:38Z 후 30m23s | docs/logs/main/events/2026-10.jsonl:691 |
| 91 | 2026-10-26T22:48:11Z | main | `budget_renew` | `serve-granite42-8b-v031` | budget_renew · floor_mib=76176 · ttl_s=7200 · remaining_before_s=7010 · 창 시작 2026-10-26T22:14:38Z 후 33m33s | docs/logs/main/events/2026-10.jsonl:692 |
| 92 | 2026-10-26T22:51:19Z | main | `budget_renew` | `serve-granite42-8b-v031` | budget_renew · floor_mib=76176 · ttl_s=7200 · remaining_before_s=7012 · 창 시작 2026-10-26T22:14:38Z 후 36m41s | docs/logs/main/events/2026-10.jsonl:693 |
| 93 | 2026-10-26T22:53:09Z | main | `budget_renew` | `serve-granite42-8b-v031` | budget_renew · floor_mib=76176 · ttl_s=7200 · remaining_before_s=7090 · 창 시작 2026-10-26T22:14:38Z 후 38m31s | docs/logs/main/events/2026-10.jsonl:694 |
| 94 | 2026-10-26T22:54:58Z | main | `budget_renew` | `serve-granite42-8b-v031` | budget_renew · floor_mib=76176 · ttl_s=7200 · remaining_before_s=7091 · 창 시작 2026-10-26T22:14:38Z 후 40m20s | docs/logs/main/events/2026-10.jsonl:695 |
| 95 | 2026-10-26T22:56:49Z | main | `budget_renew` | `serve-granite42-8b-v031` | budget_renew · floor_mib=76176 · ttl_s=7200 · remaining_before_s=7089 · 창 시작 2026-10-26T22:14:38Z 후 42m11s | docs/logs/main/events/2026-10.jsonl:696 |
| 96 | 2026-10-26T22:58:00Z | main | `budget_renew` | `serve-granite42-8b-v031` | budget_renew · floor_mib=76176 · ttl_s=7200 · remaining_before_s=7129 · 창 시작 2026-10-26T22:14:38Z 후 43m22s | docs/logs/main/events/2026-10.jsonl:697 |
| 97 | 2026-10-26T22:59:11Z | main | `budget_renew` | `serve-granite42-8b-v031` | budget_renew · floor_mib=76176 · ttl_s=7200 · remaining_before_s=7129 · 창 시작 2026-10-26T22:14:38Z 후 44m33s | docs/logs/main/events/2026-10.jsonl:698 |
| 98 | 2026-10-26T23:06:48Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → serve-granite42-8b-v031 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-26T22:14:38Z 후 52m10s) | docs/logs/main/events/2026-10.jsonl:699 |
| 99 | 2026-10-26T23:06:48Z | main | `serve_stop` | — | serve_stop · model=granite-4.2-8b · duration_s=3130 · session=serve-granite42-8b-v031-20261026T221438Z | docs/logs/main/events/2026-10.jsonl:700 |
| 100 | 2026-10-26T23:06:49Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → serve-granite42-8b-v031 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-26T22:14:38Z 후 52m11s) | docs/logs/main/events/watchdog.jsonl:833 |

**기동 시도**(예산 선언 `budget_declare` 마다 한 시도 · 같은 노드의 이어지는 행과 선언 없는 노드(엔진 로그)의 그 시각 행을 묶었다 · 사살 · 트립 · 거부는 닫힘과 따로 센다 · `measurement` 행은 측정 기록이라 시도로 세지 않는다(위 표에만) — 해석은 저작자 몫)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 1 | main | `serve-granite42-8b-v031` | 2026-10-14T01:59:39Z | 1회 · 첫 2026-10-14T01:59:41Z | 없음 | —(닫힘 행 없음) | 2 |
| 2 | main | `serve-main-combined-granite-minicpm`(라벨 다름 — 같은 노드 · 이 셀 라벨 아님) | 2026-10-14T02:04:02Z | 64회 · 첫 2026-10-14T02:04:03Z | 없음 | `budget_clear` 2026-10-17T11:36:13Z | 1 |
| 3 | main | `serve-granite42-8b-v031` | 2026-10-26T22:14:38Z | 14회 · 첫 2026-10-26T22:14:39Z | 없음 | `budget_clear` 2026-10-26T23:06:48Z | 11 |
<!-- /FACT:event_timeline -->

<!-- FACT:measurement_env -->
_측정 실행의 env 관측 없음 — 측정 엔진 로그에서 env echo 줄(`<KEY> set by environment to <VALUE>` 등)을 찾지 못했다(evidence.measurement_env_observed · 부재 ≠ 미설정)._
<!-- /FACT:measurement_env -->

> 이 절이 답하는 질문: 수신자가 이 zip 만으로 이 셀을 다시 띄우려면 어떤 순서로 무엇을 하고, 각 단계의 성공을 무엇으로 판정하며, 얼마나 기다려야 하는가?

1. 관리 NAS 체크포인트를 compose의컨테이너경로에read-only로매핑한다. 필수env자리는artifacts/compose/.env.template과triplet의셀env다. PLE스테이징은없다.
2. 이번에는 기존0.31.0이미지를사용했고 새로빌드하지않았다. FACT의층창은실제빌드소요가아니다. 계보의40분빌드와구분한다. 재빌드가필요하면Dockerfile미검증표지를해소한다.
3. 공식single_serve_up에overhead7600MiB·ready-max900을명시했다. health대기205초는이번serve로그관측이고위FACT의첫측정까지상한과다른종점이다. 이 측정을 서빙한 세션은 2026-10-26T22:14:38Z 한 회이며 사살 없이 23:06:48Z budget_clear로 닫혔다. 재생 FACT에 함께 보이는 과거 라벨 묶음은 이번 기동 횟수가 아니다. health200+completion실제응답을확인했고전용tool/reasoning시험은capability미선언N/A다.
4. 보호선언의총량124608/weights16768/KV24064/overhead7600/floor76176MiB는선언산식이다. enginecheckpoint16.38GiB·loading16.42GiB와구분한다. 과거두모델합산예산을이번단독모델에옮기지않는다.
5. 위bench재구성명령과03 §3.4의측정시점원문을구분한다. 현재Full은lite+GuideLLM레벨당3회·posthealth·요청성공·fresh키·실제PASS로확인했다.
6. TP1이므로노드간Ray/NCCL경로는없다. 실제엔진은FLASH_ATTN/FlashAttention2·graph캡처0.22GiB를기록했고초기KV23.5GiB/154000토큰을예약했다. 절대KV때문에profiling을건너뛰는메시지와DeepSelect미설치경고는관측후에도health·추론·벤치가섰다는범위로해석한다. 경고만으로GPU미동작이나서빙실패로단정하지않는다.

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
이번은 managed NAS 기존가중치 재사용이며 다운로드하지 않았다. ibm-granite/granite-4.2-8b의00 §0.2revision과 `<manifest.nas_model_path>/IBM/Granite-4.2/granite-4.2-8b`를대조한다. `test -r <manifest.nas_model_path>/IBM/Granite-4.2/granite-4.2-8b/config.json`과compose마운트확인은준비절차형상이다(artifacts/triplet yaml·compose; 이명령자체의신규실행검증주장은아님). checkpoint16.38GiB는엔진관측이며du값이아니다. 한노드만필요하다.

### 스테이징
PLE는해당없음 — 모델과트리플렛에설정없다. GuideLLM토크나이저스테이징은측정도구가만든다. 추가mmap가중치변환명령은없다.

### 네트워크
이번이미지·GuideLLM0.7.3·가중치는기존로컬자산이다. 새source빌드는Dockerfile의basepull·apt·git·pip접속을요구하므로오프라인환경은승인된이미지와체크포인트를미리준비한다. 새다운로드명령을검증한것처럼싣지않는다(artifacts/build_recipe·compose).
