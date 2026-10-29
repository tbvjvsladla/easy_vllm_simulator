# 01 · 산출물 — 무엇이 실제로 쓰였고, 어떻게 다시 띄우는가

## 1.1 적용 판정 표
> 기계가 적용 증거(빌드 원장 · 원장이 없으면 라벨된 재구성) × 파일 × 선언을 대사한 결과다. `artifacts/` 에는 재현 입력과 확인되지 않은 참고 형상도 포함될 수 있다. 파일별 검증 상태와 근거를 아래 표에서 확인한다.

<!-- FACT:slots -->
| 슬롯 | 실림 | 파일 | 신호 | 적용 증거 | 사유 |
|---|---|---|---|---|---|
| `build_patch_post` | 안 실림 | — | 1-signal | build-ledger:docker-image(main-local):sha256:339737df6571ce00993e731ef064a8db66085beaa650e4aa63bdb24f414b2034:/opt/easy-vllm/build_ledger.json · 빌드 시 원장(후속 이미지 층은 별도 관측) | 적용 판정 없음 — 원장 관측(기계 파생) |
| `build_patch_pre` | 안 실림 | — | 1-signal | build-ledger:docker-image(main-local):sha256:339737df6571ce00993e731ef064a8db66085beaa650e4aa63bdb24f414b2034:/opt/easy-vllm/build_ledger.json · 빌드 시 원장(후속 이미지 층은 별도 관측) | 적용 판정 없음 — 원장 관측(기계 파생) |
| `build_recipe` | 실림 | `artifacts/build_recipe/Dockerfile.source-build` · `artifacts/build_recipe/requirements.txt` | 3-signal | build-ledger:build-ledger.dockerfile | Dockerfile.source-build는 generated-unverified다. 현재 worktree 바이트와 이미지history가16/17 일치하므로 이미지를 지은 정확 바이트로 확인하지 못했다. 이 경고와 첫 줄 표지를 유지한다. requirements.txt는 빌드 원장sha256 일치로 verified다. source 트랙은 NGC alpha torch와 prebuilt wheel C++ ABI 벽을 피하는 선택이다(00 §0.4·W0). |
| `compose` | 실림 | `artifacts/compose/.env.template` · `artifacts/compose/docker-compose.yaml` | 2-signal | file:output/single/docker-compose.yaml | docker-compose.yaml은 이 단일셀의 마운트·이미지·포트·자원 배선을 담은 generated-unverified 참고형상이다. 측정 전 개정이 선택됐지만 이 파일의 실행 당시 원본 바이트는 별도로 관측되지 않았다. 함께 실린 .env.template은 값 치환 사본으로 verified이며, 두 파일의 검증 상태를 구분한다. 실제 서비스의 동작은 health와추론으로 확인했으며 이것이 compose 파일의 바이트 동일성을 입증하지는 않는다. 다른 호스트로 옮길 때 물리 마운트와 선언을 다시 대조한다. |
| `fork_pin` | 안 실림 | — | 1-signal | none:.claude/policies/arch_variant_ledger.json | 셀 env 에 VARIANT 줄 없음 = stock(workflow.md §변종 좌표의 거처)(기계 파생) |
| `runtime_patch` | 안 실림 | — | 1-signal | none:output/single/configs/minicpm5-2b-v031_patch.py | output/single/configs/minicpm5-2b-v031_patch.py 없음 — arm_patch.sh 는 no-op(런타임 패치 불해당)(기계 파생) |
| `triplet` | 실림 | `artifacts/triplet/.env.minicpm5-2b-v031` · `artifacts/triplet/minicpm5-2b-v031.sh` · `artifacts/triplet/minicpm5-2b-v031.yaml` | 3-signal | file:output/single/configs/minicpm5-2b-v031.yaml | 셀yaml·runner·env형상은 현재 서빙의 입력이다. 내부port8000과 호스트8002는 compose매핑으로 구분한다. runner의 parser flags는 이번 전용 tool/reasoning 시험PASS를 뜻하지 않는다. completion만 새로확인했다. |

**레시피 경고 · 실린 리비전**(기계 관측 — 수신자 빌드가 여기서 갈라질 수 있다)

- ⚠ `build_recipe` `output/single/Dockerfile.source-build` — 실린 바이트(worktree)가 측정 이미지 docker history 의 RUN·COPY 와 전수 일치하지 않는다(16/17) · 이미지 빌드 뒤 바뀐 레시피일 수 있다 — 마지막 커밋이 이미지 Created 뒤(이미지 Created 2026-10-14T01:45:19Z · 마지막 커밋 2026-10-14T03:07:26Z)
- `build_recipe` `output/single/Dockerfile.source-build` 실린 리비전: 리비전 미특정 · 선택 방법 `worktree(어느 개정도 이미지 history 와 더 맞지 않는다 · 작업트리 16/17 · 전 이력(output/*/Dockerfile.source-build · 모든 ref)에도 전수 일치 개정 없음 — recipe_vs_image 경고 유지)` · docker history 대조 16/17 · 실린 바이트 `worktree` · 쓰인 바이트임을 관측으로 확인 아니오
- `build_recipe` `output/single/requirements.txt` 실린 리비전: 리비전 미특정 · 선택 방법 `worktree(빌드 원장 sha256 일치 — 쓰인 바이트 관측 확인 = 예)` · docker history 대조 없음 · 실린 바이트 `worktree` · 쓰인 바이트임을 관측으로 확인 예
- `compose` `output/single/docker-compose.yaml` 실린 리비전: 커밋 `2199257f7f71` · 선택 방법 `worktree(mtime 2026-10-26T21:40:37Z ≤ 측정 2026-10-29T09:39:38Z(sweep_index.generated_utc) · 바이트 = 측정 전 마지막 커밋 2199257f7f71)` · docker history 대조 없음 · 실린 바이트 `worktree` · 근거 `mtime≤measured` · 쓰인 바이트임을 관측으로 확인 아니오

**싣지 않은 파일**(슬롯별 사유 — 빌드 때 skip 된 패치는 아래 적용 집합 표에 있다)

- `build_patch_post` known-non-patch 1건: `.gitkeep`
- `build_patch_pre` known-non-patch 1건: `.gitkeep`
- `compose` `arm_patch.sh` — `not-armed(output/single/configs/minicpm5-2b-v031_patch.py 없음(CONFIG_FILE) — arm_patch.sh 는 no-op · arm_patch.sh 측정 당시 개정(c8bb20e77e52) `_cfg=` 키 체인 RUNTIME_PATCH_STEM > CONFIG_FILE > 'default')`

**파일별 검증 · 관련성**(`generated-unverified` = 실행 검증되지 않은 생성물 — 파일 첫 줄 경고 · PAYLOAD 파일 기록과 같은 표시 · 관련성 = ① 패치 선언 대상 × 모델 config ② 엔진 로그 발화 서명 — 둘 다 관측일 때만 required/inactive-inferred)

| 슬롯 | 파일 | 검증 | 검증 근거 | 관련성 | 관련성 근거 |
|---|---|---|---|---|---|
| `build_recipe` | `artifacts/build_recipe/Dockerfile.source-build` | `generated-unverified` · renderer | 실린 Dockerfile.source-build(worktree) · docker history RUN·COPY 16/17 일치 — Dockerfile 전체 원본 바이트의 동일성은 확인하지 못했다(_select_dockerfile · worktree(어느 개정도 이미지 history 와 더 맞지 않는다 · 작업트리 16/17 · 전 이력(output/*/Dockerfile.source-build · 모든 ref)에도 전수 일치 개정 없음 — recipe_vs_image 경고 유지)) | `unknown` | 파일의 실행 당시 바이트가 확인되지 않았다(generated-unverified · 생성 renderer) — 이 표시만으로 경로 실행 여부나 필요 여부를 판정하지 않는다(참고 형상) |
| `build_recipe` | `artifacts/build_recipe/requirements.txt` | `verified` · cell-run | 쓰인 레시피(build-ledger.dockerfile) — 개정 선택 slots.build_recipe.evidence.selected_revisions | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/.env.template` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/docker-compose.yaml` | `generated-unverified` · renderer | docker-compose.yaml: worktree(mtime 2026-10-26T21:40:37Z ≤ 측정 2026-10-29T09:39:38Z(sweep_index.generated_utc) · 바이트 = 측정 전 마지막 커밋 2199257f7f71) — 측정 당시 실행 바이트는 별도 관측되지 않았다 | `unknown` | 파일의 실행 당시 바이트가 확인되지 않았다(generated-unverified · 생성 renderer) — 이 표시만으로 경로 실행 여부나 필요 여부를 판정하지 않는다(참고 형상) |
| `triplet` | `artifacts/triplet/.env.minicpm5-2b-v031` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/minicpm5-2b-v031.sh` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/minicpm5-2b-v031.yaml` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
<!-- /FACT:slots -->

<!-- FACT:applied_set -->
**적용 집합** — 상태 `observed` · 출처 `docker-image(main-local):sha256:339737df6571ce00993e731ef064a8db66085beaa650e4aa63bdb24f414b2034:/opt/easy-vllm/build_ledger.json · 빌드 시 원장(후속 이미지 층은 별도 관측)`

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
| attestation | absent | output/single/benchlog/attestation_minicpm5-2b-v031.json(부재) |
| local-image | observed | sha256:339737df6571ce00993e731ef064a8db66085beaa650e4aa63bdb24f414b2034 |
<!-- /FACT:applied_set -->

## 1.2 슬롯별 적용 사유
> 이 절이 답하는 질문: 실린 산출물 하나하나는 무엇을 고치며, 왜 이 셀에 필요했고, 이 셀에서 실제로 발화했으며, 언제 불필요해지는가?

### build_recipe
Dockerfile.source-build는 generated-unverified다. 현재 worktree 바이트와 이미지history가16/17 일치하므로 이미지를 지은 정확 바이트로 확인하지 못했다. 이 경고와 첫 줄 표지를 유지한다. requirements.txt는 빌드 원장sha256 일치로 verified다. source 트랙은 NGC alpha torch와 prebuilt wheel C++ ABI 벽을 피하는 선택이다(00 §0.4·W0).

### compose
docker-compose.yaml은 이 단일셀의 마운트·이미지·포트·자원 배선을 담은 generated-unverified 참고형상이다. 측정 전 개정이 선택됐지만 이 파일의 실행 당시 원본 바이트는 별도로 관측되지 않았다. 함께 실린 .env.template은 값 치환 사본으로 verified이며, 두 파일의 검증 상태를 구분한다. 실제 서비스의 동작은 health와추론으로 확인했으며 이것이 compose 파일의 바이트 동일성을 입증하지는 않는다. 다른 호스트로 옮길 때 물리 마운트와 선언을 다시 대조한다.

### triplet
셀yaml·runner·env형상은 현재 서빙의 입력이다. 내부port8000과 호스트8002는 compose매핑으로 구분한다. runner의 parser flags는 이번 전용 tool/reasoning 시험PASS를 뜻하지 않는다. completion만 새로확인했다.

### 싣지 않은 슬롯
build_patch_pre/post는 .gitkeep뿐이며 inline strip-hoist는 이미지 원장이 skipped로 기록했다. 이를 적용 패치라 부르지 않는다. fork_pin은 env VARIANT부재, runtime_patch는 셀patch.py부재라 no-op다. 이 모델에 별도 패치 발화서명을 요구하는 경로는 없지만 상위버전도 불필요하다고 일반화하지 않는다.

## 1.3 값의 지위표
> 서빙 설정의 노브 **전부**와 기계가 모은 후보 지위다(lockset 출처 · yaml 주석 · 엔진 기본값). 최종 지위는 아래 `kind: value-status` 블록이 정한다 — 표의 노브마다 블록이 정확히 하나 있어야 린터를 통과한다.

<!-- FACT:value_status -->
서빙 노브 4개 — 노브마다 `kind: value-status` 블록이 **정확히 1개** 있어야 한다(노브·값은 이 표의 글자 그대로 · 값 칸의 `\|` 는 `|` 로 적는다).

| 노브 | 값 | 후보 지위 | 후보 근거 | yaml 주석 | 출처 |
|---|---|---|---|---|---|
| `gpu-memory-utilization` | 0.39 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `max-model-len` | 32768 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `max-num-seqs` | 8 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `kv-cache-memory-bytes` | 37580963840 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
<!-- /FACT:value_status -->

> 이 절이 답하는 질문: 각 서빙 노브의 값은 이 셀에서 어떤 지위인가 — 조정됐나, 승계됐나, 음성대조로 일부러 둔 것인가, 엔진 기본값인가, 필요조건인가?

```hint-event
id: V1
kind: value-status
노브: gpu-memory-utilization
값: 0.39
지위: inherited
근거: 과거 64GB 목표 천장 환산값; 이번 단독셀에서 조정하지 않음
출처: [plan_26101410_vllm031_bump_14B이하_4종_양노드_서빙_lite §4, testlog_26102918_minicpm5_2b_E2E_Full_판정 §새 Full 측정]
```

```hint-event
id: V2
kind: value-status
노브: max-model-len
값: 32768
지위: inherited
근거: 과거 사람의 RAG 길이 고정 요구 승계; 이번 레버 대조 없음
출처: [plan_26101410_vllm031_bump_14B이하_4종_양노드_서빙_lite §4, testlog_26102918_minicpm5_2b_E2E_Full_판정 §새 Full 측정]
```

```hint-event
id: V3
kind: value-status
노브: max-num-seqs
값: 8
지위: inherited
근거: 과거 사람의 동시성 요구 8(D5)을 승계; 계획은 대표8192길이와 min(8, KV_fit@8192) 산식을 제안했지만 실행 산식 결과를 새로 관측한 주장이 아님; 이번 레버 대조 없음
출처: [plan_26101410_vllm031_bump_14B이하_4종_양노드_서빙_lite §4, testlog_26102918_minicpm5_2b_E2E_Full_판정 §새 Full 측정]
```

```hint-event
id: V4
kind: value-status
노브: kv-cache-memory-bytes
값: 37580963840
지위: inherited
근거: 과거 목표 천장에서 max_safe를 선택한 hand값 승계; 이번 최소필요량 검증 없음
출처: [plan_26101410_vllm031_bump_14B이하_4종_양노드_서빙_lite §4, testlog_26102918_minicpm5_2b_E2E_Full_판정 §새 Full 측정]
```

네 값은 모두 승계값이다. 이번 Full은 이 형상의 새 관측이며, 값의 조정·제거 대조가 아니다. 절대 KV와호스트gmu를 다른 메모리 구조에 그대로 복사하지 않는다. 명시된 필요조건이나 음성대조값으로 승격하지 않는다.

## 1.4 재현 절차
> 셀 실행 기록에서 기계가 파생한 명령 순서와 단계별 소요다. 소요가 `미관측` 인 단계는 시각 기록이 없다는 뜻이다(0 이 아니다). 그 아래는 블랙박스 이벤트 원장에서 이 셀의 행을 기계가 뽑아 기동 시도로 묶은 것이고, 마지막은 측정한 실행의 엔진 로그가 echo 한 env 다(측정 당시 값의 정본).

<!-- FACT:reproduce -->
| 순서 | 단계 | 성공 판정 | 소요 | 관측 구간(UTC) | 소요 출처 | 명령 출처 |
|---|---|---|---|---|---|---|
| 1 | build | 빌드 OK · 이미지 실재 | 층 창 38분 19초(docker history 첫 층 → 끝 층 CreatedAt · 캐시 재사용 층 포함 — 빌드 소요 아님) | 2026-10-14T01:07:00Z → 2026-10-14T01:45:19Z (layer-window) | docker history --no-trunc --format '{{json .}}' sha256:339737df6571ce00993e731ef064a8db66085beaa650e4aa63bdb24f414b2034 · 우리 Dockerfile 층 24개의 CreatedAt 첫 층 → 끝 층(캐시 재사용 층은 이전 빌드 시각 — 빌드 소요 아님) | reconstructed(build-ledger build_args + compose build 선언/설정(실행 관측 아님)) |
| 2 | serve | health 200 + 추론 1회(스모크 판정) | ≤ 2분 38초(상한) | 2026-10-29T09:23:33Z → 2026-10-29T09:26:11Z (upper) | docs/logs/main/sessions/serve-minicpm5-2b-v031-20261029T092333Z.json started_utc(측정을 포함하는 세션) → 첫 측정 시각(output/single/benchlog/sweep_handoff2_minicpm5_20261029T092610Z/lite_raw_minicpm5-2b-v031.json measured_utc(호스트 date -u · lite_bench.sh 가 cold 레그 직전에 찍는 측정 시작)) | derived(스모크 사용법) |
| 3 | bench | 리포트 발행(루브릭 권한 explicit ∧ PASS일 때만 full 인증서) | 13분 27초 | 2026-10-29T09:26:11Z → 2026-10-29T09:39:38Z (exact) | output/single/benchlog/sweep_handoff2_minicpm5_20261029T092610Z/lite_raw_minicpm5-2b-v031.json measured_utc(호스트 date -u · lite_bench.sh 가 cold 레그 직전에 찍는 측정 시작) → sweep_index.generated_utc(호스트 UTC · 두 끝 같은 시계) | reconstructed(bench json + 측정 도구 원문 lite 인자) |

**빌드 층 시각**(관측 · 소요가 아니라 층 창 — 캐시 재사용 층은 이전 빌드의 시각을 가진다)

- `build` 이미지 층 CreatedAt(docker history): 2026-10-14T01:07:00Z → 2026-10-14T01:45:19Z · 층 24개 · 출처 `docker history --no-trunc --format '{{json .}}' sha256:339737df6571ce00993e731ef064a8db66085beaa650e4aa63bdb24f414b2034` — 우리 Dockerfile 층의 CreatedAt 범위 — 캐시 재사용 층은 이전 빌드 시각을 가진다(소요 아님)

**단계별 명령**(기계 파생 — `명령 출처`가 `derived(…)`이면 사용법에서 파생하고 `reconstructed(…)`이면 괄호 안에 명시된 관측, 파일 선언·설정 또는 추론을 조합해 재구성한다. 예를 들어 docker history와 bench JSON은 기록된 관측이며 compose 파일은 선언·설정이다. 설정만으로 과거 compose build 실행 여부는 알 수 없다. 어느 명령도 실행 원문(로그에 남은 줄)은 아니다)

**1. build**

```sh
# 실린 artifacts/build_recipe/Dockerfile.source-build(실린 개정: worktree — 이미지를 지은 바이트임을 관측으로 확인 못함(docker history RUN·COPY 16/17))를 output/single/Dockerfile.source-build 자리에 두는 재현 예시다(실제 빌드 컨텍스트 아님).
# 독립 재현용 태그를 지정한다. 측정 이미지와 동일한 바이트·후속 빌드 층까지 재현되는지는 별도 검증한다.
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
  -t "${REPRO_IMAGE_TAG:?set a separate reproduction image tag}" \
  output/single
# compose 빌드 사용법(실제 실행·재현 등치는 미검증): docker compose -f output/single/docker-compose.yaml --env-file output/single/envs/.env.minicpm5-2b-v031 build
```

**2. serve**

```sh
bash .claude/skills/upstream-version-watch/scripts/single_serve_up.sh minicpm5-2b-v031
```

**3. bench**

```sh
# bench JSON 필드로 복원: backend · model_id→--model · tokenizer_id→--tokenizer · num_prompts · max_concurrency · request_rate · burstiness · 요청당 입출력 길이 = total_*_tokens ÷ completed(나누어떨어질 때만 — 평균이다 · 요청별 길이는 JSON 에 없다) · 나누어떨어지면 `--dataset-name random` 으로 추정했다(측정 도구가 그 꼴로 부른다 — 아래 도구 원문).
# JSON 에 없는 인자(--endpoint · --base-url · --ignore-eos · --num-warmups · --temperature · --seed · --random-range-ratio · --trust-remote-code)는 측정 도구가 정했다 — level_* = run_bench.sh · lite_cold/lite_warm = lite_bench.sh(.claude/skills/adversarial-benchmark/scripts/) · 셀 스윕 순서는 sweep_bench.sh.
# lite_cold/lite_warm 의 데이터셋 · 부하 인자 = 측정 시점 도구 원문 lite_bench.sh@90a3cdd56a94 L177-L183(리터럴 인자만 · `$변수` 인자는 도구가 값을 정하지 않아 JSON 필드로 채운다) — `total_*_tokens ÷ completed` 는 실측 토큰(chat 템플릿 포함)이지 인자가 아니다.
# output/single/benchlog/sweep_handoff2_minicpm5_20261029T092610Z/lite_cold_minicpm5-2b-v031.json (date 20261029-092624) · 실측 토큰(chat 템플릿 포함 · total ÷ completed · 인자 아님): 입력 520/1 = 520 · 출력 128/1 = 128
vllm bench serve --backend openai-chat --model minicpm5-2b --tokenizer /app/models/Openbmb/MiniCPM5/MiniCPM5-2B --trust-remote-code --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 --ignore-eos --num-prompts 1 --max-concurrency 1 --request-rate inf
# output/single/benchlog/sweep_handoff2_minicpm5_20261029T092610Z/lite_warm_minicpm5-2b-v031.json (date 20261029-092647) · 실측 토큰(chat 템플릿 포함 · total ÷ completed · 인자 아님): 입력 1560/3 = 520 · 출력 384/3 = 128
vllm bench serve --backend openai-chat --model minicpm5-2b --tokenizer /app/models/Openbmb/MiniCPM5/MiniCPM5-2B --trust-remote-code --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 --ignore-eos --num-prompts 3 --max-concurrency 1 --request-rate inf
```

**슬롯 → 재구성 예시 컨텍스트 매핑**(예시 위치 = `output/<토폴로지>/` · 원천 = 실린 Dockerfile 의 COPY · compose 설정; 실제 실행 경로는 별도 기록 확인)

| zip 경로 | 빌드 컨텍스트 자리 | 근거 |
|---|---|---|
| `artifacts/triplet/.env.minicpm5-2b-v031` | `envs/.env.minicpm5-2b-v031` | compose env_file envs/.env.${CONFIG_FILE}(셀 env) |
| `artifacts/triplet/minicpm5-2b-v031.sh` | `configs/minicpm5-2b-v031.sh` | compose volumes ./configs:/app/configs |
| `artifacts/triplet/minicpm5-2b-v031.yaml` | `configs/minicpm5-2b-v031.yaml` | compose volumes ./configs:/app/configs |
| `artifacts/build_recipe/Dockerfile.source-build` | `Dockerfile.source-build` | compose build.dockerfile Dockerfile.source-build(리터럴) · build.context . |
| `artifacts/build_recipe/requirements.txt` | `requirements.txt` | Dockerfile.source-build COPY requirements.txt |
| `artifacts/compose/.env.template` | `.env` | compose 변수치환 프로젝트 .env(${…} 참조) |
| `artifacts/compose/docker-compose.yaml` | `docker-compose.yaml` | compose 파일(빌드 컨텍스트 루트) |
| (실린 파일 없음 — 빈 디렉터리 생성) | `build_patches_src/` | Dockerfile.source-build COPY build_patches_src/ · 실린 패치 없음 — 빈 디렉터리를 만든다(없으면 COPY 단계에서 빌드가 멈춘다) · 빌드 원장 context_inventory.build_patch_pre = ['.gitkeep'] |
| (실린 파일 없음 — 빈 디렉터리 생성) | `build_patches/` | Dockerfile.source-build COPY build_patches/ · 실린 패치 없음 — 빈 디렉터리를 만든다(없으면 COPY 단계에서 빌드가 멈춘다) · 빌드 원장 context_inventory.build_patch_post = ['.gitkeep'] |

**기동 성공 판정(관측 · 발행 자격)**: health 200 = true · 추론 1회 = true · 근거 `output/single/benchlog/sweep_handoff2_minicpm5_20261029T092610Z/level_01/post_health_minicpm5-2b-v031.json` · `output/single/benchlog/sweep_handoff2_minicpm5_20261029T092610Z/level_01/guidellm_minicpm5-2b-v031.json`
<!-- /FACT:reproduce -->

<!-- FACT:event_timeline -->
**블랙박스 이벤트 137행**(원장에서 이 셀 label·config 에 맞는 행과 그와 시간이 겹치는 같은 노드의 다른 라벨 창 · 시각순 · 기계 발췌 · 이번 캠페인 27행 · 같은 셀 이름 · 앞선 캠페인 110행(내용 칸에 표기) · 라벨 다름(같은 노드 · 창 밖 이 셀 행과 시간이 겹치는 다른 라벨 선언 창) 81행 — 셀 라벨 일치 집계에서 제외(세션·호출 기록의 귀속 확인과 별도))

> **캠페인 경계** 2026-10-29T09:21:44Z(`camp-26102918-handoff-single2` · evidence.event_campaign_boundary): campaigns/camp-26102918-handoff-single2/campaign.yaml declared_utc 2026-10-29T09:21:44Z · 이 발행이 묶인 측정을 서빙한 budget_declare 창 시작 2026-10-29T09:23:33Z → 경계 = 앞선 값 2026-10-29T09:21:44Z(이전 행 = 같은 셀 이름 · 앞선 캠페인)

> **원장 관측 범위**(노드별 첫 행 → 마지막 행 · evidence.event_ledger_spans): main = 2026-07-31T03:27:43Z → 2026-10-29T09:40:02Z(`docs/logs/main/events/2026-07.jsonl` · `docs/logs/main/events/2026-08.jsonl` · `docs/logs/main/events/2026-09.jsonl` · `docs/logs/main/events/2026-10.jsonl` · `docs/logs/main/events/thermal.jsonl` · `docs/logs/main/events/watchdog.jsonl`)

| # | UTC | 노드 | kind | label | 내용 | 출처 |
|---|---|---|---|---|---|---|
| 1 | 2026-10-14T01:59:39Z | main | `budget_declare` | `serve-granite42-8b-v031` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=76176 · weights_mib=16768 · kv_mib=24064 · overhead_mib=7600 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 라벨 다름: `serve-granite42-8b-v031` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · budget_renew 1회 · 닫힘 replaced(budget_declare) 2026-10-14T02:04:02Z(선언 후 4m23s) · measured_utc 2026-10-29T09:39:38Z 는 이 창 밖(측정 전) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:373 |
| 2 | 2026-10-14T01:59:40Z | main | `budget_honored` | `serve-granite42-8b-v031` | budget_honored · floor_mib=76176 · arm_ceiling_mib=73104 · remaining_s=86399 · 창 시작 2026-10-14T01:59:40Z 후 0m00s · 라벨 다름: `serve-granite42-8b-v031` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:726 |
| 3 | 2026-10-14T01:59:41Z | main | `budget_renew` | `serve-granite42-8b-v031` | budget_renew · floor_mib=76176 · ttl_s=86400 · remaining_before_s=86398 · 창 시작 2026-10-14T01:59:39Z 후 0m02s · 라벨 다름: `serve-granite42-8b-v031` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:375 |
| 4 | 2026-10-14T02:04:02Z | main | `budget_declare` | `serve-main-combined-granite-minicpm` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=23966 · weights_mib=4800 · kv_mib=35840 · overhead_mib=60002 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · budget_renew 64회 · 닫힘 budget_clear 2026-10-17T11:36:13Z(선언 후 81h32m11s) · measured_utc 2026-10-29T09:39:38Z 는 이 창 밖(측정 전) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:376 |
| 5 | 2026-10-14T02:04:02Z | main | `serve_start` | — | serve_start · model=minicpm5-2b · tensor_parallel_size=1 · topology=single · session=serve-minicpm5-2b-v031-20261014T020402Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:377 |
| 6 | 2026-10-14T02:04:03Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=86400 · remaining_before_s=86399 · 창 시작 2026-10-14T02:04:02Z 후 0m01s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:378 |
| 7 | 2026-10-14T08:04:03Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=86400 · remaining_before_s=64800 · 창 시작 2026-10-14T02:04:02Z 후 6h00m01s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:379 |
| 8 | 2026-10-14T14:04:03Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=86400 · remaining_before_s=64800 · 창 시작 2026-10-14T02:04:02Z 후 12h00m01s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:380 |
| 9 | 2026-10-14T20:04:03Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=86400 · remaining_before_s=64800 · 창 시작 2026-10-14T02:04:02Z 후 18h00m01s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:384 |
| 10 | 2026-10-15T02:04:03Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=86400 · remaining_before_s=64800 · 창 시작 2026-10-14T02:04:02Z 후 24h00m01s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:385 |
| 11 | 2026-10-15T08:04:03Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=86400 · remaining_before_s=64800 · 창 시작 2026-10-14T02:04:02Z 후 30h00m01s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:386 |
| 12 | 2026-10-15T14:04:03Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=86400 · remaining_before_s=64800 · 창 시작 2026-10-14T02:04:02Z 후 36h00m01s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:387 |
| 13 | 2026-10-15T20:04:04Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=86400 · remaining_before_s=64799 · 창 시작 2026-10-14T02:04:02Z 후 42h00m02s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:391 |
| 14 | 2026-10-16T02:04:04Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=86400 · remaining_before_s=64800 · 창 시작 2026-10-14T02:04:02Z 후 48h00m02s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:392 |
| 15 | 2026-10-16T06:22:00Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=70924 · 창 시작 2026-10-14T02:04:02Z 후 52h17m58s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:393 |
| 16 | 2026-10-16T06:22:00Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7200 · 창 시작 2026-10-14T02:04:02Z 후 52h17m58s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:394 |
| 17 | 2026-10-16T06:22:40Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7160 · 창 시작 2026-10-14T02:04:02Z 후 52h18m38s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:395 |
| 18 | 2026-10-16T06:23:33Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7147 · 창 시작 2026-10-14T02:04:02Z 후 52h19m31s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:396 |
| 19 | 2026-10-16T06:24:29Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7144 · 창 시작 2026-10-14T02:04:02Z 후 52h20m27s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:397 |
| 20 | 2026-10-16T06:47:05Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=5844 · 창 시작 2026-10-14T02:04:02Z 후 52h43m03s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:398 |
| 21 | 2026-10-16T06:54:03Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=6782 · 창 시작 2026-10-14T02:04:02Z 후 52h50m01s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:399 |
| 22 | 2026-10-16T07:00:24Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=6819 · 창 시작 2026-10-14T02:04:02Z 후 52h56m22s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:400 |
| 23 | 2026-10-16T07:06:45Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=6819 · 창 시작 2026-10-14T02:04:02Z 후 53h02m43s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:401 |
| 24 | 2026-10-16T07:09:55Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7010 · 창 시작 2026-10-14T02:04:02Z 후 53h05m53s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:402 |
| 25 | 2026-10-16T07:13:05Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7010 · 창 시작 2026-10-14T02:04:02Z 후 53h09m03s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:403 |
| 26 | 2026-10-16T07:16:16Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7009 · 창 시작 2026-10-14T02:04:02Z 후 53h12m14s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:404 |
| 27 | 2026-10-16T07:18:08Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7088 · 창 시작 2026-10-14T02:04:02Z 후 53h14m06s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:405 |
| 28 | 2026-10-16T07:20:00Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7088 · 창 시작 2026-10-14T02:04:02Z 후 53h15m58s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:406 |
| 29 | 2026-10-16T07:21:53Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7087 · 창 시작 2026-10-14T02:04:02Z 후 53h17m51s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:407 |
| 30 | 2026-10-16T07:23:07Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7126 · 창 시작 2026-10-14T02:04:02Z 후 53h19m05s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:408 |
| 31 | 2026-10-16T07:24:22Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7125 · 창 시작 2026-10-14T02:04:02Z 후 53h20m20s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:409 |
| 32 | 2026-10-16T07:26:49Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7053 · 창 시작 2026-10-14T02:04:02Z 후 53h22m47s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:410 |
| 33 | 2026-10-16T07:29:41Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7028 · 창 시작 2026-10-14T02:04:02Z 후 53h25m39s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:411 |
| 34 | 2026-10-16T07:31:48Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7073 · 창 시작 2026-10-14T02:04:02Z 후 53h27m46s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:412 |
| 35 | 2026-10-16T07:33:57Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7071 · 창 시작 2026-10-14T02:04:02Z 후 53h29m55s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:413 |
| 36 | 2026-10-16T07:34:55Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7142 · 창 시작 2026-10-14T02:04:02Z 후 53h30m53s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:414 |
| 37 | 2026-10-16T07:35:54Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7141 · 창 시작 2026-10-14T02:04:02Z 후 53h31m52s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:415 |
| 38 | 2026-10-16T07:36:52Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7142 · 창 시작 2026-10-14T02:04:02Z 후 53h32m50s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:416 |
| 39 | 2026-10-16T07:37:28Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7164 · 창 시작 2026-10-14T02:04:02Z 후 53h33m26s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:417 |
| 40 | 2026-10-16T07:38:05Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7163 · 창 시작 2026-10-14T02:04:02Z 후 53h34m03s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:418 |
| 41 | 2026-10-16T07:38:41Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7164 · 창 시작 2026-10-14T02:04:02Z 후 53h34m39s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:419 |
| 42 | 2026-10-16T07:39:07Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7174 · 창 시작 2026-10-14T02:04:02Z 후 53h35m05s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:420 |
| 43 | 2026-10-16T07:39:32Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7175 · 창 시작 2026-10-14T02:04:02Z 후 53h35m30s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:421 |
| 44 | 2026-10-16T07:41:46Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7066 · 창 시작 2026-10-14T02:04:02Z 후 53h37m44s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:422 |
| 45 | 2026-10-16T07:43:55Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7071 · 창 시작 2026-10-14T02:04:02Z 후 53h39m53s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:423 |
| 46 | 2026-10-16T07:46:04Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7071 · 창 시작 2026-10-14T02:04:02Z 후 53h42m02s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:424 |
| 47 | 2026-10-16T07:48:12Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7072 · 창 시작 2026-10-14T02:04:02Z 후 53h44m10s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:425 |
| 48 | 2026-10-16T07:49:12Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7140 · 창 시작 2026-10-14T02:04:02Z 후 53h45m10s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:426 |
| 49 | 2026-10-16T07:50:11Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7141 · 창 시작 2026-10-14T02:04:02Z 후 53h46m09s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:427 |
| 50 | 2026-10-16T07:51:10Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7141 · 창 시작 2026-10-14T02:04:02Z 후 53h47m08s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:428 |
| 51 | 2026-10-16T07:51:48Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7162 · 창 시작 2026-10-14T02:04:02Z 후 53h47m46s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:429 |
| 52 | 2026-10-16T07:52:25Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7163 · 창 시작 2026-10-14T02:04:02Z 후 53h48m23s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:430 |
| 53 | 2026-10-16T07:53:03Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7162 · 창 시작 2026-10-14T02:04:02Z 후 53h49m01s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:431 |
| 54 | 2026-10-16T07:53:29Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7174 · 창 시작 2026-10-14T02:04:02Z 후 53h49m27s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:432 |
| 55 | 2026-10-16T07:53:56Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7173 · 창 시작 2026-10-14T02:04:02Z 후 53h49m54s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:433 |
| 56 | 2026-10-16T07:54:23Z | 미관측 | `measurement` | `measurement (other label's budget window)` | 같은 셀의 다른 측정(측정 노드 축 main) · 시각 = 측정 조립(sweep generated_utc = 인증서 measured_utc · 시작 시각 미기록) · 이 발행의 측정 2026-10-29T09:39:38Z 전 · 발행 기록 camp_26101616_cnn_hint__main__minicpm5_2b_v031(셀 대조 simlog:sweep_index.meta.config_name · verdict PASS) · simlog 사본 generated_utc 2026-10-16T07:54:23Z(같음) · 원장: 이 셀 라벨의 선언은 없고 다른 라벨의 창 안: serve-main-combined-granite-minicpm 2026-10-14T02:04:02Z~2026-10-17T11:36:13Z (docs/logs/main/events/2026-10.jsonl:376) · 직전 예산 행 budget_renew(serve-main-combined-granite-minicpm) 2026-10-16T07:53:56Z(docs/logs/main/events/2026-10.jsonl:433) · 직후 예산 행 budget_renew(serve-main-combined-granite-minicpm) 2026-10-16T08:04:04Z(docs/logs/main/events/2026-10.jsonl:434) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/simlog/26101620_45_28_camp_26101616_cnn_hint__main__minicpm5_2b_v031/sweep_index.json:4 |
| 57 | 2026-10-16T08:04:04Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=86400 · remaining_before_s=6592 · 창 시작 2026-10-14T02:04:02Z 후 54h00m02s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:434 |
| 58 | 2026-10-16T12:38:15Z | 미관측 | `measurement` | `measurement (other label's budget window)` | 같은 셀의 다른 측정(측정 노드 축 main) · 시각 = 측정 조립(sweep generated_utc = 인증서 measured_utc · 시작 시각 미기록) · 이 발행의 측정 2026-10-29T09:39:38Z 전 · 발행 기록 camp_26101621_minicpm_lite__main__minicpm5_2b_v031(셀 대조 publication-id-suffix(simlog 미관측) · verdict 미기재) · 원장: 이 셀 라벨의 선언은 없고 다른 라벨의 창 안: serve-main-combined-granite-minicpm 2026-10-14T02:04:02Z~2026-10-17T11:36:13Z (docs/logs/main/events/2026-10.jsonl:376) · 직전 예산 행 budget_renew(serve-main-combined-granite-minicpm) 2026-10-16T08:04:04Z(docs/logs/main/events/2026-10.jsonl:434) · 직후 예산 행 budget_renew(serve-main-combined-granite-minicpm) 2026-10-16T14:04:04Z(docs/logs/main/events/2026-10.jsonl:435) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/_evidence/camp_26101621_minicpm_lite__main__minicpm5_2b_v031.json:3 |
| 59 | 2026-10-16T14:04:04Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=86400 · remaining_before_s=64800 · 창 시작 2026-10-14T02:04:02Z 후 60h00m02s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:435 |
| 60 | 2026-10-16T14:05:33Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=86311 · 창 시작 2026-10-14T02:04:02Z 후 60h01m31s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:436 |
| 61 | 2026-10-16T14:11:52Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=6821 · 창 시작 2026-10-14T02:04:02Z 후 60h07m50s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:437 |
| 62 | 2026-10-16T14:18:13Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=6819 · 창 시작 2026-10-14T02:04:02Z 후 60h14m11s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:438 |
| 63 | 2026-10-16T14:24:34Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=6819 · 창 시작 2026-10-14T02:04:02Z 후 60h20m32s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:439 |
| 64 | 2026-10-16T14:27:45Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7009 · 창 시작 2026-10-14T02:04:02Z 후 60h23m43s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:440 |
| 65 | 2026-10-16T14:30:56Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7009 · 창 시작 2026-10-14T02:04:02Z 후 60h26m54s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:441 |
| 66 | 2026-10-16T14:34:07Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7009 · 창 시작 2026-10-14T02:04:02Z 후 60h30m05s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:442 |
| 67 | 2026-10-16T14:36:00Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7087 · 창 시작 2026-10-14T02:04:02Z 후 60h31m58s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:443 |
| 68 | 2026-10-16T14:37:53Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7087 · 창 시작 2026-10-14T02:04:02Z 후 60h33m51s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:444 |
| 69 | 2026-10-16T14:39:46Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7087 · 창 시작 2026-10-14T02:04:02Z 후 60h35m44s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:445 |
| 70 | 2026-10-16T14:41:00Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7126 · 창 시작 2026-10-14T02:04:02Z 후 60h36m58s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:446 |
| 71 | 2026-10-16T14:42:14Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7126 · 창 시작 2026-10-14T02:04:02Z 후 60h38m12s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:447 |
| 72 | 2026-10-17T11:36:13Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → serve-main-combined-granite-minicpm 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-14T02:04:02Z 후 81h32m11s) · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:451 |
| 73 | 2026-10-17T11:37:04Z | main | `serve_stop` | — | serve_stop · model=minicpm5-2b · duration_s=293582 · session=serve-minicpm5-2b-v031-20261014T020402Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:454 |
| 74 | 2026-10-17T11:42:00Z | main | `budget_declare` | `plan_26101719-e2e-a` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=65536 · weights_mib=4800 · kv_mib=35840 · overhead_mib=18432 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 라벨 다름: `plan_26101719-e2e-a` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · budget_renew 1회 · 닫힘 budget_clear 2026-10-17T11:46:05Z(선언 후 4m05s) · measured_utc 2026-10-29T09:39:38Z 는 이 창 밖(측정 전) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:456 |
| 75 | 2026-10-17T11:42:00Z | main | `serve_start` | — | serve_start · model=minicpm5-2b · tensor_parallel_size=1 · topology=single · session=serve-minicpm5-2b-v031-20261017T114200Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:457 |
| 76 | 2026-10-17T11:42:01Z | main | `budget_renew` | `plan_26101719-e2e-a` | budget_renew · floor_mib=65536 · ttl_s=7200 · remaining_before_s=7199 · 창 시작 2026-10-17T11:42:00Z 후 0m01s · 라벨 다름: `plan_26101719-e2e-a` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:458 |
| 77 | 2026-10-17T11:42:01Z | main | `budget_honored` | `plan_26101719-e2e-a` | budget_honored · floor_mib=65536 · arm_ceiling_mib=62464 · remaining_s=7199 · 창 시작 2026-10-17T11:42:01Z 후 0m00s · 라벨 다름: `plan_26101719-e2e-a` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:731 |
| 78 | 2026-10-17T11:43:43Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=74402 · rate_mib_s=35079 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → plan_26101719-e2e-a 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-17T11:42:01Z 후 1m42s) · 라벨 다름: `plan_26101719-e2e-a` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:732 |
| 79 | 2026-10-17T11:46:05Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → plan_26101719-e2e-a 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-17T11:42:00Z 후 4m05s) · 라벨 다름: `plan_26101719-e2e-a` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:459 |
| 80 | 2026-10-17T11:46:05Z | main | `serve_stop` | — | serve_stop · model=minicpm5-2b · duration_s=245 · session=serve-minicpm5-2b-v031-20261017T114200Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:460 |
| 81 | 2026-10-17T11:46:06Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → plan_26101719-e2e-a 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-17T11:42:01Z 후 4m05s) · 라벨 다름: `plan_26101719-e2e-a` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:733 |
| 82 | 2026-10-17T13:22:07Z | main | `budget_declare` | `plan_26101722-e2e-f` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=65536 · weights_mib=4800 · kv_mib=35840 · overhead_mib=18432 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 라벨 다름: `plan_26101722-e2e-f` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · budget_renew 1회 · 닫힘 budget_clear 2026-10-17T13:27:24Z(선언 후 5m17s) · measured_utc 2026-10-29T09:39:38Z 는 이 창 밖(측정 전) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:461 |
| 83 | 2026-10-17T13:22:07Z | main | `serve_start` | — | serve_start · model=minicpm5-2b · tensor_parallel_size=1 · topology=single · session=serve-minicpm5-2b-v031-20261017T132207Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:462 |
| 84 | 2026-10-17T13:22:07Z | main | `budget_honored` | `plan_26101722-e2e-f` | budget_honored · floor_mib=65536 · arm_ceiling_mib=62464 · remaining_s=7200 · 창 시작 2026-10-17T13:22:07Z 후 0m00s · 라벨 다름: `plan_26101722-e2e-f` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:734 |
| 85 | 2026-10-17T13:22:08Z | main | `budget_renew` | `plan_26101722-e2e-f` | budget_renew · floor_mib=65536 · ttl_s=7200 · remaining_before_s=7199 · 창 시작 2026-10-17T13:22:07Z 후 0m01s · 라벨 다름: `plan_26101722-e2e-f` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:463 |
| 86 | 2026-10-17T13:23:39Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=73319 · rate_mib_s=36226 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → plan_26101722-e2e-f 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-17T13:22:07Z 후 1m32s) · 라벨 다름: `plan_26101722-e2e-f` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:735 |
| 87 | 2026-10-17T13:27:24Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → plan_26101722-e2e-f 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-17T13:22:07Z 후 5m17s) · 라벨 다름: `plan_26101722-e2e-f` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:464 |
| 88 | 2026-10-17T13:27:24Z | main | `serve_stop` | — | serve_stop · model=minicpm5-2b · duration_s=317 · session=serve-minicpm5-2b-v031-20261017T132207Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:465 |
| 89 | 2026-10-17T13:27:25Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → plan_26101722-e2e-f 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-17T13:22:07Z 후 5m18s) · 라벨 다름: `plan_26101722-e2e-f` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:736 |
| 90 | 2026-10-26T21:52:55Z | main | `budget_declare` | `serve-minicpm5-2b-v031` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=65536 · weights_mib=4800 · kv_mib=35840 · overhead_mib=18432 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 기동 창 1/2: budget_renew 13회 · 닫힘 budget_clear 2026-10-26T22:13:43Z(선언 후 20m48s) · measured_utc 2026-10-29T09:39:38Z 는 이 창 밖(다른 기동 · 측정 전) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:666 |
| 91 | 2026-10-26T21:52:55Z | main | `serve_start` | — | serve_start · model=minicpm5-2b · tensor_parallel_size=1 · topology=single · session=serve-minicpm5-2b-v031-20261026T215255Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:667 |
| 92 | 2026-10-26T21:52:55Z | main | `budget_honored` | `serve-minicpm5-2b-v031` | budget_honored · floor_mib=65536 · arm_ceiling_mib=62464 · remaining_s=7200 · 창 시작 2026-10-26T21:52:55Z 후 0m00s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:829 |
| 93 | 2026-10-26T21:52:56Z | main | `budget_renew` | `serve-minicpm5-2b-v031` | budget_renew · floor_mib=65536 · ttl_s=7200 · remaining_before_s=7199 · 창 시작 2026-10-26T21:52:55Z 후 0m01s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:668 |
| 94 | 2026-10-26T21:54:36Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=88027 · rate_mib_s=22458 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → serve-minicpm5-2b-v031 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-26T21:52:55Z 후 1m41s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:830 |
| 95 | 2026-10-26T22:00:20Z | main | `budget_renew` | `serve-minicpm5-2b-v031` | budget_renew · floor_mib=65536 · ttl_s=7200 · remaining_before_s=6756 · 창 시작 2026-10-26T21:52:55Z 후 7m25s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:669 |
| 96 | 2026-10-26T22:02:35Z | main | `budget_renew` | `serve-minicpm5-2b-v031` | budget_renew · floor_mib=65536 · ttl_s=7200 · remaining_before_s=7065 · 창 시작 2026-10-26T21:52:55Z 후 9m40s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:670 |
| 97 | 2026-10-26T22:04:47Z | main | `budget_renew` | `serve-minicpm5-2b-v031` | budget_renew · floor_mib=65536 · ttl_s=7200 · remaining_before_s=7068 · 창 시작 2026-10-26T21:52:55Z 후 11m52s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:671 |
| 98 | 2026-10-26T22:06:59Z | main | `budget_renew` | `serve-minicpm5-2b-v031` | budget_renew · floor_mib=65536 · ttl_s=7200 · remaining_before_s=7068 · 창 시작 2026-10-26T21:52:55Z 후 14m04s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:672 |
| 99 | 2026-10-26T22:07:58Z | main | `budget_renew` | `serve-minicpm5-2b-v031` | budget_renew · floor_mib=65536 · ttl_s=7200 · remaining_before_s=7141 · 창 시작 2026-10-26T21:52:55Z 후 15m03s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:673 |
| 100 | 2026-10-26T22:08:57Z | main | `budget_renew` | `serve-minicpm5-2b-v031` | budget_renew · floor_mib=65536 · ttl_s=7200 · remaining_before_s=7141 · 창 시작 2026-10-26T21:52:55Z 후 16m02s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:674 |
| 101 | 2026-10-26T22:09:56Z | main | `budget_renew` | `serve-minicpm5-2b-v031` | budget_renew · floor_mib=65536 · ttl_s=7200 · remaining_before_s=7141 · 창 시작 2026-10-26T21:52:55Z 후 17m01s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:675 |
| 102 | 2026-10-26T22:10:34Z | main | `budget_renew` | `serve-minicpm5-2b-v031` | budget_renew · floor_mib=65536 · ttl_s=7200 · remaining_before_s=7162 · 창 시작 2026-10-26T21:52:55Z 후 17m39s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:676 |
| 103 | 2026-10-26T22:11:11Z | main | `budget_renew` | `serve-minicpm5-2b-v031` | budget_renew · floor_mib=65536 · ttl_s=7200 · remaining_before_s=7163 · 창 시작 2026-10-26T21:52:55Z 후 18m16s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:677 |
| 104 | 2026-10-26T22:11:48Z | main | `budget_renew` | `serve-minicpm5-2b-v031` | budget_renew · floor_mib=65536 · ttl_s=7200 · remaining_before_s=7163 · 창 시작 2026-10-26T21:52:55Z 후 18m53s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:678 |
| 105 | 2026-10-26T22:12:15Z | main | `budget_renew` | `serve-minicpm5-2b-v031` | budget_renew · floor_mib=65536 · ttl_s=7200 · remaining_before_s=7173 · 창 시작 2026-10-26T21:52:55Z 후 19m20s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:679 |
| 106 | 2026-10-26T22:12:41Z | main | `budget_renew` | `serve-minicpm5-2b-v031` | budget_renew · floor_mib=65536 · ttl_s=7200 · remaining_before_s=7174 · 창 시작 2026-10-26T21:52:55Z 후 19m46s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:680 |
| 107 | 2026-10-26T22:13:07Z | 미관측 | `measurement` | `measurement (budget window recorded)` | 같은 셀의 다른 측정(측정 노드 축 main) · 시각 = 측정 조립(sweep generated_utc = 인증서 measured_utc · 시작 시각 미기록) · 이 발행의 측정 2026-10-29T09:39:38Z 전 · 발행 기록 camp_26102705_codex_single8__main__minicpm5_2b_v031(셀 대조 simlog:sweep_index.meta.config_name · verdict PASS) · simlog 사본 generated_utc 2026-10-26T22:13:07Z(같음) / 출력 평면에 보존된 같은 셀의 다른 측정 · 원장: 이 셀 선언 창 안: serve-minicpm5-2b-v031 2026-10-26T21:52:55Z~2026-10-26T22:13:43Z (docs/logs/main/events/2026-10.jsonl:666) · serve-minicpm5-2b-v031 2026-10-26T21:52:55Z~2026-10-26T22:13:44Z (docs/logs/main/events/watchdog.jsonl:829) · 직전 예산 행 budget_renew(serve-minicpm5-2b-v031) 2026-10-26T22:12:41Z(docs/logs/main/events/2026-10.jsonl:680) · 직후 예산 행 budget_clear 2026-10-26T22:13:43Z(docs/logs/main/events/2026-10.jsonl:681) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/simlog/26102707_camp_26102705_codex_single8__main__minicpm5_2b_v031/sweep_index.json:4 |
| 108 | 2026-10-26T22:13:43Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → serve-minicpm5-2b-v031 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-26T21:52:55Z 후 20m48s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:681 |
| 109 | 2026-10-26T22:13:43Z | main | `serve_stop` | — | serve_stop · model=minicpm5-2b · duration_s=1248 · session=serve-minicpm5-2b-v031-20261026T215255Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:682 |
| 110 | 2026-10-26T22:13:44Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → serve-minicpm5-2b-v031 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-26T21:52:55Z 후 20m49s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:831 |
| 111 | 2026-10-29T09:23:33Z | main | `budget_declare` | `serve-minicpm5-2b-v031` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=65536 · weights_mib=4800 · kv_mib=35840 · overhead_mib=18432 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 기동 창 2/2: budget_renew 13회 · 닫힘 budget_clear 2026-10-29T09:40:01Z(선언 후 16m28s) · measured_utc 2026-10-29T09:39:38Z 가 이 창 안(이 측정의 기동) | docs/logs/main/events/2026-10.jsonl:750 |
| 112 | 2026-10-29T09:23:33Z | main | `serve_start` | — | serve_start · model=minicpm5-2b · tensor_parallel_size=1 · topology=single · session=serve-minicpm5-2b-v031-20261029T092333Z | docs/logs/main/events/2026-10.jsonl:751 |
| 113 | 2026-10-29T09:23:33Z | main | `budget_honored` | `serve-minicpm5-2b-v031` | budget_honored · floor_mib=65536 · arm_ceiling_mib=62464 · remaining_s=7200 · 창 시작 2026-10-29T09:23:33Z 후 0m00s | docs/logs/main/events/watchdog.jsonl:840 |
| 114 | 2026-10-29T09:23:34Z | main | `budget_renew` | `serve-minicpm5-2b-v031` | budget_renew · floor_mib=65536 · ttl_s=7200 · remaining_before_s=7199 · 창 시작 2026-10-29T09:23:33Z 후 0m01s | docs/logs/main/events/2026-10.jsonl:752 |
| 115 | 2026-10-29T09:23:48Z | main | `engine_log_window_start` | — | 캡처된 엔진 로그의 첫 시각(꼬리 캡처일 수 있다 — 엔진 시작 시각이 아닐 수 있다) · 선언 2026-10-29T09:23:33Z 후 0m15s · 시계: 컨테이너 시각을 UTC 로 읽어 측정 창 [2026-10-29T09:23:33Z, 2026-10-29T09:40:01Z] 안에 드는 것을 확인 | output/single/benchlog/sweep_handoff2_minicpm5_20261029T092610Z/lite_engine_minicpm5-2b-v031.log:36 |
| 116 | 2026-10-29T09:24:12Z | main | `engine_prefetch_disabled` | — | 가중치 auto-prefetch 꺼짐 줄 ×1 — filesystem CIFS(vLLM 로그 문구) | output/single/benchlog/sweep_handoff2_minicpm5_20261029T092610Z/lite_engine_minicpm5-2b-v031.log:56 |
| 117 | 2026-10-29T09:24:41Z | main | `engine_weights_loaded` | — | 가중치 적재 완료 줄 ×1(rank·단계별) · 최장 29.60s · 이 행 = 마지막 줄 | output/single/benchlog/sweep_handoff2_minicpm5_20261029T092610Z/lite_engine_minicpm5-2b-v031.log:61 |
| 118 | 2026-10-29T09:24:42Z | main | `engine_model_loaded` | — | 모델 적재 완료 줄 ×1(rank 별) · 마지막 줄 4.75 GiB · 32.0s | output/single/benchlog/sweep_handoff2_minicpm5_20261029T092610Z/lite_engine_minicpm5-2b-v031.log:62 |
| 119 | 2026-10-29T09:24:45Z | main | `engine_kv_cache_sized` | — | GPU KV cache 873,808 tokens · 요청당 32,768 tokens 기준 최대 동시성 26.67x | output/single/benchlog/sweep_handoff2_minicpm5_20261029T092610Z/lite_engine_minicpm5-2b-v031.log:71 |
| 120 | 2026-10-29T09:24:46Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=73616 · rate_mib_s=22402 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → serve-minicpm5-2b-v031 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T09:23:33Z 후 1m13s) | docs/logs/main/events/watchdog.jsonl:841 |
| 121 | 2026-10-29T09:25:53Z | main | `engine_init_done` | — | 엔진 초기화(profile·KV 할당·warmup) 70.59s | output/single/benchlog/sweep_handoff2_minicpm5_20261029T092610Z/lite_engine_minicpm5-2b-v031.log:85 |
| 122 | 2026-10-29T09:25:54Z | main | `server_starting` | — | API 서버 기동 — 이 측정 창 선언 2026-10-29T09:23:33Z 후 2m21s · 캡처 로그 첫 시각 2026-10-29T09:23:48Z 후 2m06s | output/single/benchlog/sweep_handoff2_minicpm5_20261029T092610Z/lite_engine_minicpm5-2b-v031.log:92 |
| 123 | 2026-10-29T09:26:48Z | main | `budget_renew` | `serve-minicpm5-2b-v031` | budget_renew · floor_mib=65536 · ttl_s=7200 · remaining_before_s=7006 · 창 시작 2026-10-29T09:23:33Z 후 3m15s | docs/logs/main/events/2026-10.jsonl:753 |
| 124 | 2026-10-29T09:29:04Z | main | `budget_renew` | `serve-minicpm5-2b-v031` | budget_renew · floor_mib=65536 · ttl_s=7200 · remaining_before_s=7064 · 창 시작 2026-10-29T09:23:33Z 후 5m31s | docs/logs/main/events/2026-10.jsonl:754 |
| 125 | 2026-10-29T09:31:17Z | main | `budget_renew` | `serve-minicpm5-2b-v031` | budget_renew · floor_mib=65536 · ttl_s=7200 · remaining_before_s=7067 · 창 시작 2026-10-29T09:23:33Z 후 7m44s | docs/logs/main/events/2026-10.jsonl:755 |
| 126 | 2026-10-29T09:33:30Z | main | `budget_renew` | `serve-minicpm5-2b-v031` | budget_renew · floor_mib=65536 · ttl_s=7200 · remaining_before_s=7067 · 창 시작 2026-10-29T09:23:33Z 후 9m57s | docs/logs/main/events/2026-10.jsonl:756 |
| 127 | 2026-10-29T09:34:29Z | main | `budget_renew` | `serve-minicpm5-2b-v031` | budget_renew · floor_mib=65536 · ttl_s=7200 · remaining_before_s=7141 · 창 시작 2026-10-29T09:23:33Z 후 10m56s | docs/logs/main/events/2026-10.jsonl:757 |
| 128 | 2026-10-29T09:35:29Z | main | `budget_renew` | `serve-minicpm5-2b-v031` | budget_renew · floor_mib=65536 · ttl_s=7200 · remaining_before_s=7140 · 창 시작 2026-10-29T09:23:33Z 후 11m56s | docs/logs/main/events/2026-10.jsonl:758 |
| 129 | 2026-10-29T09:36:27Z | main | `budget_renew` | `serve-minicpm5-2b-v031` | budget_renew · floor_mib=65536 · ttl_s=7200 · remaining_before_s=7142 · 창 시작 2026-10-29T09:23:33Z 후 12m54s | docs/logs/main/events/2026-10.jsonl:759 |
| 130 | 2026-10-29T09:37:05Z | main | `budget_renew` | `serve-minicpm5-2b-v031` | budget_renew · floor_mib=65536 · ttl_s=7200 · remaining_before_s=7162 · 창 시작 2026-10-29T09:23:33Z 후 13m32s | docs/logs/main/events/2026-10.jsonl:760 |
| 131 | 2026-10-29T09:37:42Z | main | `budget_renew` | `serve-minicpm5-2b-v031` | budget_renew · floor_mib=65536 · ttl_s=7200 · remaining_before_s=7163 · 창 시작 2026-10-29T09:23:33Z 후 14m09s | docs/logs/main/events/2026-10.jsonl:761 |
| 132 | 2026-10-29T09:38:19Z | main | `budget_renew` | `serve-minicpm5-2b-v031` | budget_renew · floor_mib=65536 · ttl_s=7200 · remaining_before_s=7163 · 창 시작 2026-10-29T09:23:33Z 후 14m46s | docs/logs/main/events/2026-10.jsonl:762 |
| 133 | 2026-10-29T09:38:46Z | main | `budget_renew` | `serve-minicpm5-2b-v031` | budget_renew · floor_mib=65536 · ttl_s=7200 · remaining_before_s=7173 · 창 시작 2026-10-29T09:23:33Z 후 15m13s | docs/logs/main/events/2026-10.jsonl:763 |
| 134 | 2026-10-29T09:39:12Z | main | `budget_renew` | `serve-minicpm5-2b-v031` | budget_renew · floor_mib=65536 · ttl_s=7200 · remaining_before_s=7174 · 창 시작 2026-10-29T09:23:33Z 후 15m39s | docs/logs/main/events/2026-10.jsonl:764 |
| 135 | 2026-10-29T09:40:01Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → serve-minicpm5-2b-v031 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T09:23:33Z 후 16m28s) | docs/logs/main/events/2026-10.jsonl:765 |
| 136 | 2026-10-29T09:40:01Z | main | `serve_stop` | — | serve_stop · model=minicpm5-2b · duration_s=988 · session=serve-minicpm5-2b-v031-20261029T092333Z | docs/logs/main/events/2026-10.jsonl:766 |
| 137 | 2026-10-29T09:40:02Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → serve-minicpm5-2b-v031 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T09:23:33Z 후 16m29s) | docs/logs/main/events/watchdog.jsonl:842 |

**기동 시도**(예산 선언 `budget_declare` 마다 한 시도 · 같은 노드의 이어지는 행과 선언 없는 노드(엔진 로그)의 그 시각 행을 묶었다 · 사살 · 트립 · 거부는 닫힘과 따로 센다 · `measurement` 행은 측정 기록이라 시도로 세지 않는다(위 표에만) — 해석은 저작자 몫)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 1 | main | `serve-minicpm5-2b-v031` | 2026-10-29T09:23:33Z | 13회 · 첫 2026-10-29T09:23:34Z | 없음 | `budget_clear` 2026-10-29T09:40:01Z | 12 |

**캠페인 경계 이전 기동 5회**(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀 라벨로 묶인 1회 · 라벨 다름(같은 노드 · 창 밖 이 셀 행과 시간이 겹치는 다른 라벨 선언 창) 4회 · 어느 쪽도 이 셀의 시도로 세지 않는다 · 행 전문 = 01 §1.4)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 앞선-1 | main | `serve-granite42-8b-v031`(라벨 다름 — 같은 노드 · 이 셀 라벨 아님) | 2026-10-14T01:59:39Z | 1회 · 첫 2026-10-14T01:59:41Z | 없음 | —(닫힘 행 없음) | 1 |
| 앞선-2 | main | `serve-main-combined-granite-minicpm`(라벨 다름 — 같은 노드 · 이 셀 라벨 아님) | 2026-10-14T02:04:02Z | 64회 · 첫 2026-10-14T02:04:03Z | 없음 | `budget_clear` 2026-10-17T11:36:13Z | 2 |
| 앞선-3 | main | `plan_26101719-e2e-a`(라벨 다름 — 같은 노드 · 이 셀 라벨 아님) | 2026-10-17T11:42:00Z | 1회 · 첫 2026-10-17T11:42:01Z | 없음 | `budget_clear` 2026-10-17T11:46:05Z | 5 |
| 앞선-4 | main | `plan_26101722-e2e-f`(라벨 다름 — 같은 노드 · 이 셀 라벨 아님) | 2026-10-17T13:22:07Z | 1회 · 첫 2026-10-17T13:22:08Z | 없음 | `budget_clear` 2026-10-17T13:27:24Z | 5 |
| 앞선-5 | main | `serve-minicpm5-2b-v031` | 2026-10-26T21:52:55Z | 13회 · 첫 2026-10-26T21:52:56Z | 없음 | `budget_clear` 2026-10-26T22:13:43Z | 5 |
<!-- /FACT:event_timeline -->

<!-- FACT:measurement_env -->
_측정 실행의 env 관측 없음 — 측정 엔진 로그에서 env echo 줄(`<KEY> set by environment to <VALUE>` 등)을 찾지 못했다(evidence.measurement_env_observed · 부재 ≠ 미설정)._
<!-- /FACT:measurement_env -->

> 이 절이 답하는 질문: 수신자가 이 zip 만으로 이 셀을 다시 띄우려면 어떤 순서로 무엇을 하고, 각 단계의 성공을 무엇으로 판정하며, 얼마나 기다려야 하는가?

1. 관리 NAS의 체크포인트를 compose의 MODEL_PATH와 같은 컨테이너 위치에 read-only로 마운트한다. 파일·env자리는 슬롯매핑과 artifacts/compose/.env.template, artifacts/triplet/.env.minicpm5-2b-v031이다. PLE 추가스테이징은 없다.
2. 기존 이미지 재사용이며 이번 새 빌드는 없다. 위 층창38분19초는 빌드소요가 아니다. 계보는 약40분 빌드를 기록한다(testlog_26101412_v031_14B이하_4종_양노드_상주서빙_판정 §1). 재빌드 명령은 참고 재구성이고 Dockerfile 미검증경고를 유지한다.
3. 공식 single_serve_up에 overhead18432MiB와ready-max900을 명시했다. 새 health까지140초는 serve.log의 종점이고 FACT의≤2분38초는 첫 lite측정까지의 상한으로 종점이 다르다. 이번 측정을 서빙한 세션은20261029T092333Z 한 회이며, 과거 동명 라벨 행은 FACT의 캠페인 경계 밖이다. health200과 실제 completion 응답으로 확인했고 전용tool/reasoning은 capability미선언N/A다(testlog_26102918_minicpm5_2b_E2E_Full_판정 §기동과 기능). Full 직후 정규down으로 닫혔다.
4. 선언weights4800/KV35840/overhead18432/floor65536MiB를 실제노드 입력으로 다시 도출한다. 예산 선언을 엔진 로드4.75GiB 실측으로 부르지 않는다. KV35GiB도 선언클램프를 엔진이 되읽은 값이다.
5. 위 재구성 bench명령과03 §3.4의 실제호출기록·도구원문을 구분한다. 이번13분27초는lite시작→sweep완료의 같은호스트시각차다. Full3회씩1/2/4/8, posthealth·completed·판정PASS를 함께 확인한다.
6. TP1이라 노드간NCCL/Ray 경로는 해당없다. 엔진 설정과 측정env는 당시관측의 정본이다. 명시된 절대KV 때문에 메모리프로파일링을 건너뛰는 경고는 선언을 읽는 분기이며, 다른 경고도 원문과 health·추론으로 판단한다. 이 셀 별도 런타임패치는 없다.

## 1.5 서브 레시피 해설
<!-- FACT:sub_recipe -->
해당 없음 — single 토폴로지(서브 노드 · Ray worker 없음).
<!-- /FACT:sub_recipe -->

<!-- FACT:measurement_env_nodes -->
해당 없음 — single 토폴로지의 단일 측정 노드에는 노드 간 비교 대상이 없다.
<!-- /FACT:measurement_env_nodes -->

> 해당 없음 — 이 절의 조건 `topology=multi` 이 이 셀에서 성립하지 않는다(`topology=single`).

## 1.6 재현 절차 — 기동 전 준비(사람이 할 일)
> 1.4 의 명령을 치기 **전에** 사람이 손으로 해야 하는 것이다 — 가중치 획득 · 스테이징 생성 · 네트워크가 필요한 단계. 에이전트가 대신 할 수 없는 자리(다운로드 승인 · 물리 경로 · 외부 계정)가 여기 모인다.

> 이 절이 답하는 질문: 이 셀을 기동하기 전에 사람이 준비해야 하는 것은 무엇이며, 각각을 어떤 명령으로 만들고, 그 절차의 출처는 어디인가?

### 가중치
이번 획득모드는managed NAS 재사용이며 다운로드하지 않았다. openbmb/MiniCPM5-2B의00 §0.2 revision과 `<manifest.nas_model_path>/Openbmb/MiniCPM5/MiniCPM5-2B`를 대조한다. 사전확인은 `test -r <manifest.nas_model_path>/Openbmb/MiniCPM5/MiniCPM5-2B/config.json` 후 compose의`/app/models`매핑확인이다(artifacts/triplet yaml·compose; 경로형상검사, 이명령자체의새실행검증주장은아님). checkpoint크기는엔진4.69GiB이며du로검증한값이아니다. 이셀은메인한노드만필요하다.

### PLE·스테이징
해당없음 — 이체크포인트와트리플렛에PLE설정없음(00후보표·artifacts/triplet). GuideLLM토크나이저스테이징은측정도구가만든다. 임의mmap변환이나추가가중치생성을하지않는다.

### 네트워크 준비
기존서빙이미지와GuideLLM0.7.3이미지·체크포인트가로컬에있어이번실행의모델다운로드/pip/pull은없었다. 새환경의source재빌드는Dockerfile의basepull·apt·git·pip가외부접속을요구한다. 오프라인대안은사람이승인한이미지와체크포인트를미리준비하는것이다(artifacts/build_recipe·compose). 출처에확인된다운로드실행명령은이셀여정에없으므로받는명령을검증된것처럼새로싣지않는다.
