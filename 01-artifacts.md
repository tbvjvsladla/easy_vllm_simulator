# 01 · 산출물 — 무엇이 실제로 쓰였고, 어떻게 다시 띄우는가

## 1.1 적용 판정 표
> 기계가 적용 증거(빌드 원장 · 원장이 없으면 라벨된 재구성) × 파일 × 선언을 대사한 결과다. `artifacts/` 에는 재현 입력과 확인되지 않은 참고 형상도 포함될 수 있다. 파일별 검증 상태와 근거를 아래 표에서 확인한다.

<!-- FACT:slots -->
| 슬롯 | 실림 | 파일 | 신호 | 적용 증거 | 사유 |
|---|---|---|---|---|---|
| `build_patch_post` | 실림 | `artifacts/build_patch_post/06-b12x-optional032.sh` | 3-signal | build-ledger:attestation:output/multi/benchlog/attestation_ds4f0731-032-spec7-roce.json#main | 원장/바이트결속applied verified의공유optional B12X1.5.1 exactwheel설치다. 구operator1.2.3/DSL4.6을재사용하지않고공식0.32의API/CUTLASS4.8접점을맞춘다. assert/pipcheck실패시종료하며조용한skip은없다. 현재MoE/draft의B12X_MXFP4_MXFP8선택은GPU발화근거이나optional배포전체커널을검증한것은아니다. 기계relevance unknown은그대로두고모델정적헤더와적용/발화를구분한다. |
| `build_patch_pre` | 안 실림 | — | 1-signal | build-ledger:attestation:output/multi/benchlog/attestation_ds4f0731-032-spec7-roce.json#main | 적용 판정 없음 — 원장 관측(기계 파생) |
| `build_recipe` | 실림 | `artifacts/build_recipe/06-b12x-optional032.sh` · `artifacts/build_recipe/Dockerfile.source-build-deps2` · `artifacts/build_recipe/parent-source-core/README.md` · `artifacts/build_recipe/parent-source-core/alias-evidence.json` · `artifacts/build_recipe/parent-source-core/context/Dockerfile.source-build` · `artifacts/build_recipe/parent-source-core/context/build_patches/.gitkeep` · `artifacts/build_recipe/parent-source-core/context/build_patches/05-ngc-source032-deps-compat.sh` · `artifacts/build_recipe/parent-source-core/context/build_patches_src/.gitkeep` · `artifacts/build_recipe/parent-source-core/context/requirements.txt` · `artifacts/build_recipe/parent-source-core/core-build-command.txt` · `artifacts/build_recipe/parent-source-core/leaf-ledger-evidence.json` · `artifacts/build_recipe/parent-source-core/manifest.json` | 2-signal | build-ledger:build-ledger.dockerfile | 쓰인 Dockerfile 선택자 Dockerfile.source-build-deps2(build-ledger.dockerfile) + 그것이 COPY 하는 파일(기계 파생) |
| `compose` | 실림 | `artifacts/compose/.env.cluster.template` · `artifacts/compose/.env.interconnect.template` · `artifacts/compose/.env.template` · `artifacts/compose/docker-compose.yaml` · `artifacts/compose/serve_runner.sh` · `artifacts/compose/sub_recipe.json` | 2-signal | file:output/multi/docker-compose.yaml | .env.template/.env.cluster.template/.env.interconnect.template와sub_recipe는verified,docker-compose.yaml/serve_runner.sh는generated-unverified다. 후자는실행당시완전바이트복원이아닌참고형상이다. 실제측정env/노드관측을정본으로사용하고현재렌더값으로덮지않는다. |
| `fork_pin` | 안 실림 | — | 1-signal | none:.claude/policies/arch_variant_ledger.json | 셀 env 에 VARIANT 줄 없음 = stock(workflow.md §변종 좌표의 거처)(기계 파생) |
| `runtime_patch` | 안 실림 | — | 1-signal | none:output/multi/configs/ds4f0731-032-spec7-roce_patch.py | output/multi/configs/ds4f0731-032-spec7-roce_patch.py 없음 — arm_patch.sh 는 no-op(런타임 패치 불해당)(기계 파생) |
| `triplet` | 실림 | `artifacts/triplet/.env.ds4f0731-032-spec7-roce` · `artifacts/triplet/ds4f0731-032-spec7-roce.sh` · `artifacts/triplet/ds4f0731-032-spec7-roce.yaml` | 3-signal | file:output/multi/configs/ds4f0731-032-spec7-roce.yaml | yaml/sh/env형상은verified다. yaml이TP2/Ray/FP8로더·KV/B12X/graph/DSpark5/DeepSeekparser를선언하며enable-auto-tool-choice도명시한다. 현재health/content/reasoning/Full과과거0.29tool왕복을서로다른범위로읽는다. 이설정의0.32제거대조는없다. |

**레시피 경고 · 실린 리비전**(기계 관측 — 수신자 빌드가 여기서 갈라질 수 있다)

- ⚠ `build_recipe` `output/multi/Dockerfile.source-build-deps2` — 실린 바이트(worktree)가 측정 이미지 docker history 의 RUN·COPY 와 전수 일치하지 않는다(5/10)(이미지 Created 2026-10-29T10:50:18Z · 마지막 커밋 미관측)
- `build_recipe` `output/multi/Dockerfile.source-build-deps2` 실린 리비전: 리비전 미특정 · 선택 방법 `worktree(어느 개정도 이미지 history 와 더 맞지 않는다 · 작업트리 5/10 · 대조할 개정 없음(이미지 Created 이전 커밋 부재 · 비추적) · 전 이력(output/*/Dockerfile.source-build-deps2 · 모든 ref)에도 전수 일치 개정 없음 — recipe_vs_image 경고 유지)` · docker history 대조 5/10 · 실린 바이트 `worktree` · 쓰인 바이트임을 관측으로 확인 아니오
- `build_recipe` `output/multi/build_patches/06-b12x-optional032.sh` 실린 리비전: 리비전 미특정 · 선택 방법 `worktree(이미지 안 COPY 대상 바이트 미관측 — 이미지 탐침 안 함)` · docker history 대조 없음 · 실린 바이트 `worktree` · 쓰인 바이트임을 관측으로 확인 아니오
- `compose` `.claude/skills/upstream-version-watch/assets/configs/serve_runner.sh` 실린 리비전: 커밋 `50a5ce486738` · 선택 방법 `worktree(mtime 2026-09-06T21:57:53Z ≤ 측정 2026-10-29T16:08:59Z(sweep_index.generated_utc) · 바이트 = 측정 전 마지막 커밋 50a5ce486738)` · docker history 대조 없음 · 실린 바이트 `worktree` · 근거 `mtime≤measured` · 쓰인 바이트임을 관측으로 확인 아니오
- `compose` `output/multi/docker-compose.yaml` 실린 리비전: 커밋 `bccd08091976` · 선택 방법 `worktree(mtime 2026-10-29T10:01:25Z ≤ 측정 2026-10-29T16:08:59Z(sweep_index.generated_utc) · 바이트 = 측정 전 마지막 커밋 bccd08091976)` · docker history 대조 없음 · 실린 바이트 `worktree` · 근거 `mtime≤measured` · 쓰인 바이트임을 관측으로 확인 아니오
- ⚠ `triplet` 측정 뒤 재생성된 파일(mtime > 측정 시각)을 실었다: `output/multi/envs/.env.ds4f0731-032-spec7-roce` — 실린 형상은 측정 당시 값과 다를 수 있다(측정 당시 값 = 01 §1.4 측정 env 관측)

**싣지 않은 파일**(슬롯별 사유 — 빌드 때 skip 된 패치는 아래 적용 집합 표에 있다)

- `build_patch_post` `05-ngc-source032-deps-compat.sh` — `excluded_by_recipe(Dockerfile.source-build-deps2 이 build_patches/05-ngc-source032-deps-compat.sh 를 COPY 하지 않는다 — 실행된 적 없음)`
- `build_patch_post` `70-exllamav3-build.sh` — `excluded_by_recipe(Dockerfile.source-build-deps2 이 build_patches/70-exllamav3-build.sh 를 COPY 하지 않는다 — 실행된 적 없음)`
- `build_patch_post` `72-exl3-row-store.sh` — `excluded_by_recipe(Dockerfile.source-build-deps2 이 build_patches/72-exl3-row-store.sh 를 COPY 하지 않는다 — 실행된 적 없음)`
- `build_patch_post` `73-vllm-exl3-build.sh` — `excluded_by_recipe(Dockerfile.source-build-deps2 이 build_patches/73-vllm-exl3-build.sh 를 COPY 하지 않는다 — 실행된 적 없음)`
- `build_patch_post` `74-exl3-moe-prep-fused.sh` — `excluded_by_recipe(Dockerfile.source-build-deps2 이 build_patches/74-exl3-moe-prep-fused.sh 를 COPY 하지 않는다 — 실행된 적 없음)`
- `build_patch_post` `75-exl3-python-deps032.sh` — `excluded_by_recipe(Dockerfile.source-build-deps2 이 build_patches/75-exl3-python-deps032.sh 를 COPY 하지 않는다 — 실행된 적 없음)`
- `build_patch_post` `79-exl3-port-selfcheck.sh` — `excluded_by_recipe(Dockerfile.source-build-deps2 이 build_patches/79-exl3-port-selfcheck.sh 를 COPY 하지 않는다 — 실행된 적 없음)`
- `build_patch_post` known-non-patch 1건: `.gitkeep`
- `build_patch_pre` `80-exl3-quant-method.sh` — `excluded_by_recipe(Dockerfile.source-build-deps2 이 build_patches_src/80-exl3-quant-method.sh 를 COPY 하지 않는다 — 실행된 적 없음)`
- `build_patch_pre` `81-exl3-model-wiring.sh` — `excluded_by_recipe(Dockerfile.source-build-deps2 이 build_patches_src/81-exl3-model-wiring.sh 를 COPY 하지 않는다 — 실행된 적 없음)`
- `build_patch_pre` `82-exl3-h2d-stage.sh` — `excluded_by_recipe(Dockerfile.source-build-deps2 이 build_patches_src/82-exl3-h2d-stage.sh 를 COPY 하지 않는다 — 실행된 적 없음)`
- `build_patch_pre` `83-engram-file-backend.sh` — `excluded_by_recipe(Dockerfile.source-build-deps2 이 build_patches_src/83-engram-file-backend.sh 를 COPY 하지 않는다 — 실행된 적 없음)`
- `build_patch_pre` `84-sm12x-block-shim.sh` — `excluded_by_recipe(Dockerfile.source-build-deps2 이 build_patches_src/84-sm12x-block-shim.sh 를 COPY 하지 않는다 — 실행된 적 없음)`
- `build_patch_pre` `85-exl3-serial-streams.sh` — `excluded_by_recipe(Dockerfile.source-build-deps2 이 build_patches_src/85-exl3-serial-streams.sh 를 COPY 하지 않는다 — 실행된 적 없음)`
- `build_patch_pre` `86-exl3-worker-hooks.sh` — `excluded_by_recipe(Dockerfile.source-build-deps2 이 build_patches_src/86-exl3-worker-hooks.sh 를 COPY 하지 않는다 — 실행된 적 없음)`
- `build_patch_pre` `87-engram-embed-skip.sh` — `excluded_by_recipe(Dockerfile.source-build-deps2 이 build_patches_src/87-engram-embed-skip.sh 를 COPY 하지 않는다 — 실행된 적 없음)`
- `build_patch_pre` `88-engram-prestage.sh` — `excluded_by_recipe(Dockerfile.source-build-deps2 이 build_patches_src/88-engram-prestage.sh 를 COPY 하지 않는다 — 실행된 적 없음)`
- `build_patch_pre` `89-kv-null-block-zero.sh` — `excluded_by_recipe(Dockerfile.source-build-deps2 이 build_patches_src/89-kv-null-block-zero.sh 를 COPY 하지 않는다 — 실행된 적 없음)`
- `build_patch_pre` `exl3_port_lib.py` — `nonconforming(<NN>-*.sh 규약 밖 — 싣지 않음)`
- `build_patch_pre` known-non-patch 2건: `.gitkeep` · `PROVENANCE.json`
- `compose` `arm_patch.sh` — `not-armed(output/multi/configs/ds4f0731-032-spec7-roce_patch.py 없음(CONFIG_FILE) · slave default_patch.py(CONFIG_FILE) 없음 — arm_patch.sh 는 no-op · arm_patch.sh 측정 당시 개정(1b1df2cd4034) `_cfg=` 키 체인 RUNTIME_PATCH_STEM > CONFIG_FILE > 'default')`

**파일별 검증 · 관련성**(`generated-unverified` = 실행 검증되지 않은 생성물 — 파일 첫 줄 경고 · PAYLOAD 파일 기록과 같은 표시 · 관련성 = ① 패치 선언 대상 × 모델 config ② 엔진 로그 발화 서명 — 둘 다 관측일 때만 required/inactive-inferred)

| 슬롯 | 파일 | 검증 | 검증 근거 | 관련성 | 관련성 근거 |
|---|---|---|---|---|---|
| `build_patch_post` | `artifacts/build_patch_post/06-b12x-optional032.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_ds4f0731-032-spec7-roce.json#main) · 원장 관측 | `unknown` | ① 패치 헤더·본문에 대상 선언(model-trigger 아키텍처 · vllm/models/<model_type>) 없음 |
| `build_recipe` | `artifacts/build_recipe/06-b12x-optional032.sh` | `verified` · cell-run | 쓰인 레시피(build-ledger.dockerfile) — 개정 선택 slots.build_recipe.evidence.selected_revisions | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `build_recipe` | `artifacts/build_recipe/Dockerfile.source-build-deps2` | `generated-unverified` · renderer | 실린 Dockerfile.source-build-deps2(worktree) · docker history RUN·COPY 5/10 일치 — Dockerfile 전체 원본 바이트의 동일성은 확인하지 못했다(_select_dockerfile · worktree(어느 개정도 이미지 history 와 더 맞지 않는다 · 작업트리 5/10 · 대조할 개정 없음(이미지 Created 이전 커밋 부재 · 비추적) · 전 이력(output/*/Dockerfile.source-build-deps2 · 모든 ref)에도 전수 일치 개정 없음 — recipe_vs_image 경고 유지)) | `unknown` | 파일의 실행 당시 바이트가 확인되지 않았다(generated-unverified · 생성 renderer) — 이 표시만으로 경로 실행 여부나 필요 여부를 판정하지 않는다(참고 형상) |
| `build_recipe` | `artifacts/build_recipe/parent-source-core/README.md` | `generated-unverified` · agent | 명시 parent manifest와 main 실행/alias/leaf 원장에서 재구성한 새 양노드 local-build 예시이며 실행하지 않았다 | `unknown` | 파일의 실행 당시 바이트가 확인되지 않았다(generated-unverified · 생성 agent) — 이 표시만으로 경로 실행 여부나 필요 여부를 판정하지 않는다(참고 형상) |
| `build_recipe` | `artifacts/build_recipe/parent-source-core/alias-evidence.json` | `verified` · cell-run | explicit manifest output/multi/parent-build/qwen032-deps2/manifest.json · selected image digest bound · frozen source sha256 verified · main historical scope only | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `build_recipe` | `artifacts/build_recipe/parent-source-core/context/Dockerfile.source-build` | `verified` · cell-run | explicit manifest output/multi/parent-build/qwen032-deps2/manifest.json · selected image digest bound · frozen source sha256 verified · main historical scope only | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `build_recipe` | `artifacts/build_recipe/parent-source-core/context/build_patches/.gitkeep` | `verified` · cell-run | explicit manifest output/multi/parent-build/qwen032-deps2/manifest.json · selected image digest bound · frozen source sha256 verified · main historical scope only | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `build_recipe` | `artifacts/build_recipe/parent-source-core/context/build_patches/05-ngc-source032-deps-compat.sh` | `verified` · cell-run | explicit manifest output/multi/parent-build/qwen032-deps2/manifest.json · selected image digest bound · frozen source sha256 verified · main historical scope only | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `build_recipe` | `artifacts/build_recipe/parent-source-core/context/build_patches_src/.gitkeep` | `verified` · cell-run | explicit manifest output/multi/parent-build/qwen032-deps2/manifest.json · selected image digest bound · frozen source sha256 verified · main historical scope only | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `build_recipe` | `artifacts/build_recipe/parent-source-core/context/requirements.txt` | `verified` · cell-run | explicit manifest output/multi/parent-build/qwen032-deps2/manifest.json · selected image digest bound · frozen source sha256 verified · main historical scope only | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `build_recipe` | `artifacts/build_recipe/parent-source-core/core-build-command.txt` | `verified` · cell-run | explicit manifest output/multi/parent-build/qwen032-deps2/manifest.json · selected image digest bound · frozen source sha256 verified · main historical scope only | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `build_recipe` | `artifacts/build_recipe/parent-source-core/leaf-ledger-evidence.json` | `verified` · cell-run | explicit manifest output/multi/parent-build/qwen032-deps2/manifest.json · selected image digest bound · frozen source sha256 verified · main historical scope only | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `build_recipe` | `artifacts/build_recipe/parent-source-core/manifest.json` | `generated-unverified` · agent | 명시 parent manifest와 main 실행/alias/leaf 원장에서 재구성한 새 양노드 local-build 예시이며 실행하지 않았다 | `unknown` | 파일의 실행 당시 바이트가 확인되지 않았다(generated-unverified · 생성 agent) — 이 표시만으로 경로 실행 여부나 필요 여부를 판정하지 않는다(참고 형상) |
| `compose` | `artifacts/compose/.env.cluster.template` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/.env.interconnect.template` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/.env.template` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/docker-compose.yaml` | `generated-unverified` · renderer | docker-compose.yaml: worktree(mtime 2026-10-29T10:01:25Z ≤ 측정 2026-10-29T16:08:59Z(sweep_index.generated_utc) · 바이트 = 측정 전 마지막 커밋 bccd08091976) — 측정 당시 실행 바이트는 별도 관측되지 않았다 | `unknown` | 파일의 실행 당시 바이트가 확인되지 않았다(generated-unverified · 생성 renderer) — 이 표시만으로 경로 실행 여부나 필요 여부를 판정하지 않는다(참고 형상) |
| `compose` | `artifacts/compose/serve_runner.sh` | `generated-unverified` · renderer | serve_runner.sh: worktree(mtime 2026-09-06T21:57:53Z ≤ 측정 2026-10-29T16:08:59Z(sweep_index.generated_utc) · 바이트 = 측정 전 마지막 커밋 50a5ce486738) — 측정 당시 실행 바이트는 별도 관측되지 않았다 | `unknown` | 파일의 실행 당시 바이트가 확인되지 않았다(generated-unverified · 생성 renderer) — 이 표시만으로 경로 실행 여부나 필요 여부를 판정하지 않는다(참고 형상) |
| `compose` | `artifacts/compose/sub_recipe.json` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/.env.ds4f0731-032-spec7-roce` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) · ⚠ 측정(2026-10-29T16:08:59Z) 뒤 mtime: ['output/multi/envs/.env.ds4f0731-032-spec7-roce'] — 실린 값이 측정 당시 값과 같다는 보장 없음 | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/ds4f0731-032-spec7-roce.sh` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) · ⚠ 측정(2026-10-29T16:08:59Z) 뒤 mtime: ['output/multi/envs/.env.ds4f0731-032-spec7-roce'] — 실린 값이 측정 당시 값과 같다는 보장 없음 | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/ds4f0731-032-spec7-roce.yaml` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) · ⚠ 측정(2026-10-29T16:08:59Z) 뒤 mtime: ['output/multi/envs/.env.ds4f0731-032-spec7-roce'] — 실린 값이 측정 당시 값과 같다는 보장 없음 | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
<!-- /FACT:slots -->

<!-- FACT:applied_set -->
**적용 집합** — 상태 `observed` · 출처 `attestation:output/multi/benchlog/attestation_ds4f0731-032-spec7-roce.json#main`

| 단계 | 파일 | 결과 | 결과 출처 | 사유 | 실림 | model-trigger |
|---|---|---|---|---|---|---|
| post | `06-b12x-optional032.sh` | applied | build-ledger | Official vLLM0.32 optional B12X1.5.1 exact wheel; pip check passed; GPU kernels unverified | 실림 | Generic SM120/SM121 B12X optional backend; no model weights. |

**결과 출처 범례**

- `build-ledger` — 이미지 안 빌드 원장이 적용 결과를 기록했다(관측)

**판정 근거**(패치별 — artifacts 가 판정에 쓴 관측 · 스크립트 바이트 정체 = 실린 바이트가 측정 이미지에 구워진 바이트인가)

| 파일 | 스크립트 바이트 정체 | 이미지 사본 sha256 | 적용 표지(상태 찾음/선언) | 정적 규칙 | 원장 분류 |
|---|---|---|---|---|---|
| `06-b12x-optional032.sh` | build-ledger(원장 script_sha256 = 작업트리 바이트 — 실린 바이트 = 빌드가 실행한 바이트) | `aebff978bb9bf4f06accc04b52ec872b66f96c11b3c1922032a4e2f0240bba15`(빌드 원장 script_sha256 — 이미지 사본 아님) | — | — | status-file |

**원장 탐침**(순위: attestation → 메인 로컬 이미지 → 재구성)

| 순위 | 결과 | 대상 |
|---|---|---|
| attestation | observed | output/multi/benchlog/attestation_ds4f0731-032-spec7-roce.json |

> **attestation 범위** — config `ds4f0731-032-spec7-roce` · phase serve · 작성 2026-10-29T15:57:10Z(output/multi/benchlog/attestation_ds4f0731-032-spec7-roce.json 파일 mtime(파일시스템 관측 — 문서에 시각 칸이 없다 · 기록 필드 아님)) · 묶은 근거 cell-name · 파일 `output/multi/benchlog/attestation_ds4f0731-032-spec7-roce.json` · 측정 창 대비 작성 2026-10-29T15:57:10Z < 측정 시작 2026-10-29T16:01:20Z — 측정 전(빌드·스모크)의 관측: **same config(ds4f0731-032-spec7-roce) · serve — 원장 순서로 판별한 같은 기동의 관측(결속된 스모크 로그 미확인 · 제한검색 · 약한 판별)(판별: 원장 순서로 판별(결속된 스모크 로그 미확인 · 제한검색 — docs/simlog/**/*serve*.log 에서 이 attestation 을 적은 로그 미확인; 다른 경로의 보존 여부는 판정하지 않음): 원장 docs/logs/main/events/2026-10.jsonl:833 budget_declare(smoke-ds4f0731-032-spec7-roce · 2026-10-29T15:38:05Z) ≤ attestation 작성 2026-10-29T15:57:10Z · serve_proof 작성 2026-10-29T15:57:10Z < 측정 시작 2026-10-29T16:01:20Z · 그 선언이 측정 끝 2026-10-29T16:08:59Z 까지 재선언 · 창 닫힘 없이 이어짐(main 원장 기록 범위 안) · API 서버 기동 2026-10-29T15:56:52Z(output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/lite_engine_ds4f0731-032-spec7-roce.log:734 · 컨테이너 시계 = UTC 가정) ∈ [선언, attestation 작성] · 측정 엔진 로그 APIServer pid 791 하나)**
<!-- /FACT:applied_set -->

## 1.2 슬롯별 적용 사유
> 이 절이 답하는 질문: 실린 산출물 하나하나는 무엇을 고치며, 왜 이 셀에 필요했고, 이 셀에서 실제로 발화했으며, 언제 불필요해지는가?

### build_recipe와parent-source-core
선택Dockerfile.source-build-deps2는history5/10으로전체실행바이트동일성미확인/generated-unverified다. parent-source-core/context의실제coreDockerfile/requirements/POST05/두.gitkeep5파일과별도context_root를구분한다. PRE는빈scaffold이고alias/argv/ledger/README/manifest는context밖이다.
main은core+POST05→deps1같은IDalias(빌드없음)→deps2POST06이관측됐다. actualcoreargv는core-build-command.txt의exactSHA/arch12.1/jobs8이다. sub는oldcore→실제별도deps1수리→deps2의다른역사이고원core전체Dockerfile은미회수다. README/manifest와3명령은generated-unverified의새양노드localbuild재현안으로실행하지않았다. 부모증거로leaf전체바이트경고를덮지않는다.

### parent POST05: 05-ngc-source032-deps-compat.sh
NGC datasets의hub<2와vLLM>=1.31교집합hub1.33.0,triton-kernels의pytest9.2.0을설치/pipcheck했다. maincore의공유의존수리로leaf의새적용이나모델고유요구가아니다. set-e/pipcheck는실패종료이며skip분기는없다. 상류의존변경시교집합을재평가한다.

### build_patch_post: 06-b12x-optional032.sh
원장/바이트결속applied verified의공유optional B12X1.5.1 exactwheel설치다. 구operator1.2.3/DSL4.6을재사용하지않고공식0.32의API/CUTLASS4.8접점을맞춘다. assert/pipcheck실패시종료하며조용한skip은없다. 현재MoE/draft의B12X_MXFP4_MXFP8선택은GPU발화근거이나optional배포전체커널을검증한것은아니다. 기계relevance unknown은그대로두고모델정적헤더와적용/발화를구분한다.

> [원문] artifacts/build_patch_post/06-b12x-optional032.sh §L7-15
> set -euo pipefail
> python3 -m pip install --no-build-isolation 'https://files.pythonhosted.org/packages/0c/7f/31fbc8311188835a509d97c38f6400071ba5b01a5ca54cb33d7b22dec3fa/b12x-1.5.1-py3-none-any.whl#sha256=8731e23571d8d45db26d78e7699ec0b23e7ba20b52487adf8c1063c8d8b53a3d'
> python3 - <<'CHECK'
> from importlib.metadata import version
> assert version('b12x') == '1.5.1'
> print('[06-b12x] exact installed distribution', version('b12x'))
> CHECK
> python3 -m pip check
> if [ -n "${EASY_VLLM_PATCH_STATUS:-}" ]; then printf 'applied\tOfficial vLLM0.32 optional B12X1.5.1 exact wheel; pip check passed; GPU kernels unverified\n' > "$EASY_VLLM_PATCH_STATUS"; fi

### triplet
yaml/sh/env형상은verified다. yaml이TP2/Ray/FP8로더·KV/B12X/graph/DSpark5/DeepSeekparser를선언하며enable-auto-tool-choice도명시한다. 현재health/content/reasoning/Full과과거0.29tool왕복을서로다른범위로읽는다. 이설정의0.32제거대조는없다.

### compose
.env.template/.env.cluster.template/.env.interconnect.template와sub_recipe는verified,docker-compose.yaml/serve_runner.sh는generated-unverified다. 후자는실행당시완전바이트복원이아닌참고형상이다. 실제측정env/노드관측을정본으로사용하고현재렌더값으로덮지않는다.

### 적용하지 않은 슬롯
runtime패치/forkpin/leafPRE는없다. 공식stock0.32가DSpark/B12X를제공하며과거0.29buildpatch를현재에다시적용하지않았다. 현재다른모델i5의PRE/POST도이셀입력이아니다.

## 1.3 값의 지위표
> 서빙 설정의 노브 **전부**와 기계가 모은 후보 지위다(lockset 출처 · yaml 주석 · 엔진 기본값). 최종 지위는 아래 `kind: value-status` 블록이 정한다 — 표의 노브마다 블록이 정확히 하나 있어야 린터를 통과한다.

<!-- FACT:value_status -->
서빙 노브 14개 — 노브마다 `kind: value-status` 블록이 **정확히 1개** 있어야 한다(노브·값은 이 표의 글자 그대로 · 값 칸의 `\|` 는 `|` 로 적는다).

| 노브 | 값 | 후보 지위 | 후보 근거 | yaml 주석 | 출처 |
|---|---|---|---|---|---|
| `tensor-parallel-size` | 2 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `distributed-executor-backend` | ray | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `gpu-memory-utilization` | 0.85 | inherited | lockset 출처 gmu_source=hand · lockset provenance=hand-authored | — | lockset.gmu_source=hand |
| `max-model-len` | 262144 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `quantization` | fp8 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `kv-cache-dtype` | fp8 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `kv-cache-memory-bytes` | 10737418240 | inherited | lockset 출처 kv_source=hand · lockset provenance=hand-authored | — | lockset.kv_source=hand |
| `max-num-seqs` | 1 | inherited | lockset 출처 batch_source=hand-lever · lockset provenance=hand-authored | — | lockset.batch_source=hand-lever |
| `moe-backend` | b12x | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `enforce-eager` | false | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `speculative-config` | {"method": "dspark", "num_speculative_tokens": 5} | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `reasoning-parser` | deepseek_v4 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `tool-call-parser` | deepseek_v4 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `enable-auto-tool-choice` | true | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
<!-- /FACT:value_status -->

> 이 절이 답하는 질문: 각 서빙 노브의 값은 이 셀에서 어떤 지위인가 — 조정됐나, 승계됐나, 음성대조로 일부러 둔 것인가, 엔진 기본값인가, 필요조건인가?

```hint-event
id: V1
kind: value-status
노브: tensor-parallel-size
값: '2'
지위: inherited
근거: 새0.32모델별근거와과거관측을평가한손레버; 이셀변경스윕/제거실패대조없음
출처: [devlog_26103001_DS4F0731_032_실측_인계, testlog_26103001_DS4F0731_032_TP2와_Full_판정]
```

```hint-event
id: V2
kind: value-status
노브: distributed-executor-backend
값: 'ray'
지위: inherited
근거: 새0.32모델별근거와과거관측을평가한손레버; 이셀변경스윕/제거실패대조없음
출처: [devlog_26103001_DS4F0731_032_실측_인계, testlog_26103001_DS4F0731_032_TP2와_Full_판정]
```

```hint-event
id: V3
kind: value-status
노브: gpu-memory-utilization
값: '0.85'
지위: inherited
근거: 새0.32모델별근거와과거관측을평가한손레버; 이셀변경스윕/제거실패대조없음
출처: [devlog_26103001_DS4F0731_032_실측_인계, testlog_26103001_DS4F0731_032_TP2와_Full_판정]
```

```hint-event
id: V4
kind: value-status
노브: max-model-len
값: '262144'
지위: inherited
근거: 새0.32모델별근거와과거관측을평가한손레버; 이셀변경스윕/제거실패대조없음
출처: [devlog_26103001_DS4F0731_032_실측_인계, testlog_26103001_DS4F0731_032_TP2와_Full_판정]
```

```hint-event
id: V5
kind: value-status
노브: quantization
값: 'fp8'
지위: inherited
근거: 새0.32모델별근거와과거관측을평가한손레버; 이셀변경스윕/제거실패대조없음
출처: [devlog_26103001_DS4F0731_032_실측_인계, testlog_26103001_DS4F0731_032_TP2와_Full_판정]
```

```hint-event
id: V6
kind: value-status
노브: kv-cache-dtype
값: 'fp8'
지위: inherited
근거: 새0.32모델별근거와과거관측을평가한손레버; 이셀변경스윕/제거실패대조없음
출처: [devlog_26103001_DS4F0731_032_실측_인계, testlog_26103001_DS4F0731_032_TP2와_Full_판정]
```

```hint-event
id: V7
kind: value-status
노브: kv-cache-memory-bytes
값: '10737418240'
지위: inherited
근거: 새0.32모델별근거와과거관측을평가한손레버; 이셀변경스윕/제거실패대조없음
출처: [devlog_26103001_DS4F0731_032_실측_인계, testlog_26103001_DS4F0731_032_TP2와_Full_판정]
```

```hint-event
id: V8
kind: value-status
노브: max-num-seqs
값: '1'
지위: inherited
근거: 새0.32모델별근거와과거관측을평가한손레버; 이셀변경스윕/제거실패대조없음
출처: [devlog_26103001_DS4F0731_032_실측_인계, testlog_26103001_DS4F0731_032_TP2와_Full_판정]
```

```hint-event
id: V9
kind: value-status
노브: moe-backend
값: 'b12x'
지위: inherited
근거: 새0.32모델별근거와과거관측을평가한손레버; 이셀변경스윕/제거실패대조없음
출처: [devlog_26103001_DS4F0731_032_실측_인계, testlog_26103001_DS4F0731_032_TP2와_Full_판정]
```

```hint-event
id: V10
kind: value-status
노브: enforce-eager
값: 'false'
지위: inherited
근거: 새0.32모델별근거와과거관측을평가한손레버; 이셀변경스윕/제거실패대조없음
출처: [devlog_26103001_DS4F0731_032_실측_인계, testlog_26103001_DS4F0731_032_TP2와_Full_판정]
```

```hint-event
id: V11
kind: value-status
노브: speculative-config
값: '{"method": "dspark", "num_speculative_tokens": 5}'
지위: inherited
근거: 새0.32모델별근거와과거관측을평가한손레버; 이셀변경스윕/제거실패대조없음
출처: [devlog_26103001_DS4F0731_032_실측_인계, testlog_26103001_DS4F0731_032_TP2와_Full_판정]
```

```hint-event
id: V12
kind: value-status
노브: reasoning-parser
값: 'deepseek_v4'
지위: inherited
근거: 새0.32모델별근거와과거관측을평가한손레버; 이셀변경스윕/제거실패대조없음
출처: [devlog_26103001_DS4F0731_032_실측_인계, testlog_26103001_DS4F0731_032_TP2와_Full_판정]
```

```hint-event
id: V13
kind: value-status
노브: tool-call-parser
값: 'deepseek_v4'
지위: inherited
근거: 새0.32모델별근거와과거관측을평가한손레버; 이셀변경스윕/제거실패대조없음
출처: [devlog_26103001_DS4F0731_032_실측_인계, testlog_26103001_DS4F0731_032_TP2와_Full_판정]
```

```hint-event
id: V14
kind: value-status
노브: enable-auto-tool-choice
값: 'true'
지위: inherited
근거: 새0.32모델별근거와과거관측을평가한손레버; 이셀변경스윕/제거실패대조없음
출처: [devlog_26103001_DS4F0731_032_실측_인계, testlog_26103001_DS4F0731_032_TP2와_Full_판정]
```

과거버전에서tuned/필요조건이었던값도이번0.32의제거실패대조를뜻하지않는다. KV10GiB는GPU당이며TP로나누지않는다. 실제K5와승인IDspec7을혼동하지않고seq1/C1조건밖최적성을복사하지않는다.

## 1.4 재현 절차
> 셀 실행 기록에서 기계가 파생한 명령 순서와 단계별 소요다. 소요가 `미관측` 인 단계는 시각 기록이 없다는 뜻이다(0 이 아니다). 그 아래는 블랙박스 이벤트 원장에서 이 셀의 행을 기계가 뽑아 기동 시도로 묶은 것이고, 마지막은 측정한 실행의 엔진 로그가 echo 한 env 다(측정 당시 값의 정본).

<!-- FACT:reproduce -->
| 순서 | 단계 | 성공 판정 | 소요 | 관측 구간(UTC) | 소요 출처 | 명령 출처 |
|---|---|---|---|---|---|---|
| 1 | render | 러너 3종·.env·.env.cluster·.env.interconnect 생성(렌더러 fail-loud) | 미관측 | 미관측 | .claude/skills/upstream-version-watch/scripts/multinode_serve_smoke.sh 선행조건 안내 · .claude/skills/upstream-version-watch/scripts/render_dockerfile.py CLI | derived(렌더러 CLI) |
| 2 | parent_source_core | generated-unverified: 새 양노드 local reproduction 예시 · 미실행 | 미관측 | 미관측 | main observed core+POST05 build → same-ID deps1 alias (no build) → deps2 POST06. Sub historical old core unavailable; this is a new main-path reproduction proposal for independent local builds on both nodes, not sub historical execution proof. | generated-unverified(explicit manifest output/multi/parent-build/qwen032-deps2/manifest.json) |
| 3 | alias | generated-unverified: 새 양노드 local reproduction 예시 · 미실행 | 미관측 | 미관측 | main observed core+POST05 build → same-ID deps1 alias (no build) → deps2 POST06. Sub historical old core unavailable; this is a new main-path reproduction proposal for independent local builds on both nodes, not sub historical execution proof. | generated-unverified(explicit manifest output/multi/parent-build/qwen032-deps2/manifest.json) |
| 4 | build | 빌드 OK · 이미지 실재 | 층 창 9분 23초(docker history 첫 층 → 끝 층 CreatedAt · 캐시 재사용 층 포함 — 빌드 소요 아님) | 2026-10-29T10:40:55Z → 2026-10-29T10:50:18Z (layer-window) | docker history --no-trunc --format '{{json .}}' sha256:8227e7d79471b6b92f21157acfb38c7cc617d1510a5c56f3dec7eb8e48567884 · 우리 Dockerfile 층 22개의 CreatedAt 첫 층 → 끝 층(캐시 재사용 층은 이전 빌드 시각 — 빌드 소요 아님) | generated-unverified(explicit manifest output/multi/parent-build/qwen032-deps2/manifest.json · leaf ledger args; full original CLI unavailable) |
| 5 | serve | health 200 + 추론 1회(스모크 판정) | ≤ 23분 15초(상한) | 2026-10-29T15:38:05Z → 2026-10-29T16:01:20Z (upper) | docs/logs/main/events/2026-10.jsonl:833 budget_declare(label smoke-ds4f0731-032-spec7-roce) · 관측된 예산 창 안의 측정  → 첫 측정 시각(output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/lite_raw_ds4f0731-032-spec7-roce.json measured_utc(호스트 date -u · lite_bench.sh 가 cold 레그 직전에 찍는 측정 시작)) | derived(스모크 사용법) |
| 6 | bench | 리포트 발행(루브릭 권한 explicit ∧ PASS일 때만 full 인증서) | 7분 39초 | 2026-10-29T16:01:20Z → 2026-10-29T16:08:59Z (exact) | output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/lite_raw_ds4f0731-032-spec7-roce.json measured_utc(호스트 date -u · lite_bench.sh 가 cold 레그 직전에 찍는 측정 시작) → sweep_index.generated_utc(호스트 UTC · 두 끝 같은 시계) | reconstructed(bench json + 측정 도구 원문 lite 인자) |

**빌드 층 시각**(관측 · 소요가 아니라 층 창 — 캐시 재사용 층은 이전 빌드의 시각을 가진다)

- `build` 이미지 층 CreatedAt(docker history): 2026-10-29T10:40:55Z → 2026-10-29T10:50:18Z · 층 22개 · 출처 `docker history --no-trunc --format '{{json .}}' sha256:8227e7d79471b6b92f21157acfb38c7cc617d1510a5c56f3dec7eb8e48567884` — 우리 Dockerfile 층의 CreatedAt 범위 — 캐시 재사용 층은 이전 빌드 시각을 가진다(소요 아님)

**단계별 명령**(기계 파생 — `명령 출처`가 `derived(…)`이면 사용법에서 파생하고 `reconstructed(…)`이면 괄호 안에 명시된 관측, 파일 선언·설정 또는 추론을 조합해 재구성한다. 예를 들어 docker history와 bench JSON은 기록된 관측이며 compose 파일은 선언·설정이다. 설정만으로 과거 compose build 실행 여부는 알 수 없다. 어느 명령도 실행 원문(로그에 남은 줄)은 아니다)

**1. render**

```sh
python3 .claude/skills/upstream-version-watch/scripts/render_dockerfile.py --materialize-configs --topology multi
python3 .claude/skills/upstream-version-watch/scripts/render_dockerfile.py --materialize-env --topology multi --manifest output/multi/manifest.yaml
python3 .claude/skills/upstream-version-watch/scripts/render_dockerfile.py --cluster-envfile --topology multi --manifest output/multi/manifest.yaml -o output/multi/envs/.env.cluster
python3 .claude/skills/upstream-version-watch/scripts/render_dockerfile.py --nccl-envfile --manifest output/multi/manifest.yaml -o output/multi/envs/.env.interconnect
```

**2. parent_source_core**

```sh
# generated-unverified: new reproduction, not an observed invocation
: "${REPRO_CORE_TAG:?set an independent local core tag}"
docker pull nvcr.io/nvidia/pytorch@sha256:2140e699b3beaf7f96a0081fd9c9406bc3832b435cdb60dfa2d261f7d2f34a1c
docker tag nvcr.io/nvidia/pytorch@sha256:2140e699b3beaf7f96a0081fd9c9406bc3832b435cdb60dfa2d261f7d2f34a1c nvcr.io/nvidia/pytorch:26.07-py3
docker build --progress plain --build-arg VLLM_REF=5df63a7203cdb9328d824a27b682e77c7b9928be --build-arg TORCH_CUDA_ARCH=12.1 --build-arg BUILD_JOBS=8 -t "$REPRO_CORE_TAG" -f parent-source-core/context/Dockerfile.source-build parent-source-core/context
```

**3. alias**

```sh
# generated-unverified: main observed same-ID alias; reproduce locally, no separate build
: "${REPRO_DEPS1_TAG:?set an independent local alias tag}"
docker tag "$REPRO_CORE_TAG" "$REPRO_DEPS1_TAG"
```

**4. build**

```sh
# generated-unverified: reconstructed from selected leaf ledger, original full CLI unavailable
: "${REPRO_IMAGE_TAG:?set an independent local leaf tag}"
mkdir -p build_patches
cp ../build_patch_post/06-b12x-optional032.sh build_patches/06-b12x-optional032.sh
docker build --build-arg BASE_IMAGE="$REPRO_DEPS1_TAG" --build-arg VLLM_REPO=https://github.com/vllm-project/vllm.git --build-arg VLLM_REF=5df63a7203cdb9328d824a27b682e77c7b9928be --build-arg SM12X_PORT=0 --build-arg SRC_DEPS_AUTHORITY=0 -t "$REPRO_IMAGE_TAG" -f Dockerfile.source-build-deps2 .
```

**5. serve**

```sh
bash .claude/skills/upstream-version-watch/scripts/multinode_serve_smoke.sh ds4f0731-032-spec7-roce --keep-up
```

**6. bench**

```sh
# bench JSON 필드로 복원: backend · model_id→--model · tokenizer_id→--tokenizer · num_prompts · max_concurrency · request_rate · burstiness · 요청당 입출력 길이 = total_*_tokens ÷ completed(나누어떨어질 때만 — 평균이다 · 요청별 길이는 JSON 에 없다) · 나누어떨어지면 `--dataset-name random` 으로 추정했다(측정 도구가 그 꼴로 부른다 — 아래 도구 원문).
# JSON 에 없는 인자(--endpoint · --base-url · --ignore-eos · --num-warmups · --temperature · --seed · --random-range-ratio · --trust-remote-code)는 측정 도구가 정했다 — level_* = run_bench.sh · lite_cold/lite_warm = lite_bench.sh(.claude/skills/adversarial-benchmark/scripts/) · 셀 스윕 순서는 sweep_bench.sh.
# lite_cold/lite_warm 의 데이터셋 · 부하 인자 = 측정 시점 도구 원문 lite_bench.sh@d33b0878a6c3 L177-L183(리터럴 인자만 · `$변수` 인자는 도구가 값을 정하지 않아 JSON 필드로 채운다) — `total_*_tokens ÷ completed` 는 실측 토큰(chat 템플릿 포함)이지 인자가 아니다.
# output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/lite_cold_ds4f0731-032-spec7-roce.json (date 20261029-160141) · 실측 토큰(chat 템플릿 포함 · total ÷ completed · 인자 아님): 입력 595/1 = 595 · 출력 128/1 = 128 · spec_decode_acceptance_length 2.54902(이 레그 JSON)
vllm bench serve --backend openai-chat --model deepseek-v4-flash-0731 --tokenizer /app/models/DeepSeek/DeepSeek-V4/DeepSeek-V4-Flash-0731 --trust-remote-code --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 --ignore-eos --num-prompts 1 --max-concurrency 1 --request-rate inf
# output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/lite_warm_ds4f0731-032-spec7-roce.json (date 20261029-160209) · 실측 토큰(chat 템플릿 포함 · total ÷ completed · 인자 아님): 입력 1785/3 = 595 · 출력 384/3 = 128 · spec_decode_acceptance_length 2.08602(이 레그 JSON)
vllm bench serve --backend openai-chat --model deepseek-v4-flash-0731 --tokenizer /app/models/DeepSeek/DeepSeek-V4/DeepSeek-V4-Flash-0731 --trust-remote-code --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 --ignore-eos --num-prompts 3 --max-concurrency 1 --request-rate inf
```

**슬롯 → 재구성 예시 컨텍스트 매핑**(예시 위치 = `output/<토폴로지>/` · 원천 = 실린 Dockerfile 의 COPY · compose 설정; 실제 실행 경로는 별도 기록 확인) — 재구성 예시 통로 `parent-source-core/context/`(실제 원본·후속층의 빌드 경로 증명 아님 · 별도 실행 기록 확인)

| zip 경로 | 빌드 컨텍스트 자리 | 근거 |
|---|---|---|
| `artifacts/triplet/.env.ds4f0731-032-spec7-roce` | `envs/.env.ds4f0731-032-spec7-roce` | compose env_file envs/.env.${CONFIG_FILE}(셀 env) |
| `artifacts/triplet/ds4f0731-032-spec7-roce.sh` | `configs/ds4f0731-032-spec7-roce.sh` | compose volumes ./configs:/app/configs |
| `artifacts/triplet/ds4f0731-032-spec7-roce.yaml` | `configs/ds4f0731-032-spec7-roce.yaml` | compose volumes ./configs:/app/configs |
| `artifacts/build_patch_post/06-b12x-optional032.sh` | `build_patches/06-b12x-optional032.sh` | Dockerfile.source-build-deps2 COPY build_patches/06-b12x-optional032.sh (or whole directory) |
| `artifacts/build_recipe/06-b12x-optional032.sh` | `build_patches/06-b12x-optional032.sh` | Dockerfile.source-build-deps2 COPY build_patches/06-b12x-optional032.sh |
| `artifacts/build_recipe/Dockerfile.source-build-deps2` | `Dockerfile.source-build-deps2` | compose build.dockerfile ${BUILD_DOCKERFILE} · build.context . |
| `artifacts/build_recipe/parent-source-core/README.md` | — | explicit selected-image-bound parent manifest; main frozen context only; metadata is not build context |
| `artifacts/build_recipe/parent-source-core/alias-evidence.json` | — | explicit selected-image-bound parent manifest; main frozen context only; metadata is not build context |
| `artifacts/build_recipe/parent-source-core/context/Dockerfile.source-build` | `parent-source-core/context/Dockerfile.source-build` | explicit selected-image-bound parent manifest; main frozen context only; metadata is not build context |
| `artifacts/build_recipe/parent-source-core/context/build_patches/.gitkeep` | `parent-source-core/context/build_patches/.gitkeep` | explicit selected-image-bound parent manifest; main frozen context only; metadata is not build context |
| `artifacts/build_recipe/parent-source-core/context/build_patches/05-ngc-source032-deps-compat.sh` | `parent-source-core/context/build_patches/05-ngc-source032-deps-compat.sh` | explicit selected-image-bound parent manifest; main frozen context only; metadata is not build context |
| `artifacts/build_recipe/parent-source-core/context/build_patches_src/.gitkeep` | `parent-source-core/context/build_patches_src/.gitkeep` | explicit selected-image-bound parent manifest; main frozen context only; metadata is not build context |
| `artifacts/build_recipe/parent-source-core/context/requirements.txt` | `parent-source-core/context/requirements.txt` | explicit selected-image-bound parent manifest; main frozen context only; metadata is not build context |
| `artifacts/build_recipe/parent-source-core/core-build-command.txt` | — | explicit selected-image-bound parent manifest; main frozen context only; metadata is not build context |
| `artifacts/build_recipe/parent-source-core/leaf-ledger-evidence.json` | — | explicit selected-image-bound parent manifest; main frozen context only; metadata is not build context |
| `artifacts/build_recipe/parent-source-core/manifest.json` | — | explicit selected-image-bound parent manifest; main frozen context only; metadata is not build context |
| `artifacts/compose/.env.cluster.template` | `envs/.env.cluster` | compose env_file envs/.env.cluster |
| `artifacts/compose/.env.interconnect.template` | `envs/.env.interconnect` | compose env_file envs/.env.interconnect |
| `artifacts/compose/.env.template` | `.env` | compose 변수치환 프로젝트 .env(${…} 참조) |
| `artifacts/compose/docker-compose.yaml` | `docker-compose.yaml` | compose 파일(빌드 컨텍스트 루트) |
| `artifacts/compose/serve_runner.sh` | `configs/serve_runner.sh` | compose volumes ./configs:/app/configs |
| `artifacts/compose/sub_recipe.json` | — | 빌드 컨텍스트 입력 아님(Dockerfile COPY · compose 참조 밖) |
| (실린 파일 없음 — 빈 디렉터리 생성) | `parent-source-core/context/build_patches_src/` | parent frozen inventory: empty PRE scaffold only; no current i5 PRE modules |

**env 형상의 파생 키 · 실효값**(한 manifest 필드 → 여러 키 · 키마다 자기 자리표시 — 값은 필드 값이 아니라 아래 규칙의 결과다)

| 형상 | manifest 필드 | 키 | 자리표시 | 실효값 규칙 | 출처 |
|---|---|---|---|---|---|
| `artifacts/compose/.env.interconnect.template` | `interconnect.nccl_transport` | `NCCL_IB_DISABLE` · `NCCL_NET` | `<derived:interconnect.nccl_transport→NCCL_IB_DISABLE>` · `<derived:interconnect.nccl_transport→NCCL_NET>` | rdma → NCCL_IB_DISABLE=0 · NCCL_NET=IB \| socket → NCCL_IB_DISABLE=1 · NCCL_NET=Socket | render_dockerfile.NCCL_TRANSPORTS(선택지 → 키 값) |
| `artifacts/compose/.env.interconnect.template` | `interconnect.socket_iface` | `GLOO_SOCKET_IFNAME` · `NCCL_SOCKET_IFNAME` · `OMPI_MCA_btl_tcp_if_include` · `TP_SOCKET_IFNAME` · `UCX_NET_DEVICES` | `<derived:interconnect.socket_iface→GLOO_SOCKET_IFNAME>` · `<derived:interconnect.socket_iface→NCCL_SOCKET_IFNAME>` · `<derived:interconnect.socket_iface→OMPI_MCA_btl_tcp_if_include>` · `<derived:interconnect.socket_iface→TP_SOCKET_IFNAME>` · `<derived:interconnect.socket_iface→UCX_NET_DEVICES>` | GLOO_SOCKET_IFNAME={interconnect.socket_iface} · NCCL_SOCKET_IFNAME={interconnect.socket_iface} · OMPI_MCA_btl_tcp_if_include={interconnect.socket_iface} · TP_SOCKET_IFNAME={interconnect.socket_iface} · UCX_NET_DEVICES={interconnect.socket_iface} | render_dockerfile.env_tier 탐침 manifest(탐침 값 → {field} 규칙) |

**기동 성공 판정(관측 · 발행 자격)**: health 200 = true · 추론 1회 = true · 근거 `output/multi/benchlog/serve_proof_ds4f0731-032-spec7-roce.json` — 시점 범위: 작성 2026-10-29T15:57:10Z(output/multi/benchlog/serve_proof_ds4f0731-032-spec7-roce.json 파일 mtime(파일시스템 관측 — 문서에 시각 칸이 없다 · 기록 필드 아님)) · 측정 창 대비 작성 2026-10-29T15:57:10Z < 측정 시작 2026-10-29T16:01:20Z — 측정 전(빌드·스모크)의 관측
<!-- /FACT:reproduce -->

<!-- FACT:event_timeline -->
**블랙박스 이벤트 17행**(원장에서 이 셀 label·config 에 맞는 행만 · 시각순 · 기계 발췌)

> **캠페인 경계** 2026-10-29T09:57:19Z(`camp-26102919-codex-multi4-v032` · evidence.event_campaign_boundary): campaigns/camp-26102919-codex-multi4-v032/campaign.yaml declared_utc 2026-10-29T09:57:19Z · 이 발행이 묶인 측정을 서빙한 budget_declare 창 시작 2026-10-29T15:38:05Z → 경계 = 앞선 값 2026-10-29T09:57:19Z(이전 행 = 같은 셀 이름 · 앞선 캠페인)

> **원장 관측 범위**(노드별 첫 행 → 마지막 행 · evidence.event_ledger_spans): main = 2026-07-31T03:27:43Z → 2026-10-29T16:33:22Z(`docs/logs/main/events/2026-07.jsonl` · `docs/logs/main/events/2026-08.jsonl` · `docs/logs/main/events/2026-09.jsonl` · `docs/logs/main/events/2026-10.jsonl` · `docs/logs/main/events/thermal.jsonl` · `docs/logs/main/events/watchdog.jsonl`) · sub = 2026-06-13T22:25:01Z → 2026-10-29T14:09:01Z(`sync_staging/sub_docs/logs/sub/events/2026-06.jsonl` · `sync_staging/sub_docs/logs/sub/events/2026-07.jsonl` · `sync_staging/sub_docs/logs/sub/events/2026-08.jsonl` · `sync_staging/sub_docs/logs/sub/events/2026-09.jsonl` · `sync_staging/sub_docs/logs/sub/events/2026-10.jsonl` · `sync_staging/sub_docs/logs/sub/events/thermal.jsonl` · `sync_staging/sub_docs/logs/sub/events/watchdog.jsonl` · 회수 미러가 가린 옛 로컬 사본 `docs/logs/sub/events/2026-09.jsonl`) — ⚠ sub(마지막 행 2026-10-29T14:09:01Z < 측정 끝 2026-10-29T16:08:59Z): 그 뒤 그 노드의 선언 · 사건은 **관측 범위 밖**이다(없음이 아니다 · 원장 미회수)

| # | UTC | 노드 | kind | label | 내용 | 출처 |
|---|---|---|---|---|---|---|
| 1 | 2026-10-29T15:38:05Z | main | `budget_declare` | `smoke-ds4f0731-032-spec7-roce` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=21479 · weights_mib=79577 · kv_mib=10240 · overhead_mib=13312 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 기동 창 1/1: budget_renew 4회 · 닫힘 budget_clear 2026-10-29T16:10:00Z(선언 후 31m55s) · measured_utc 2026-10-29T16:08:59Z 가 이 창 안(이 측정의 기동) | docs/logs/main/events/2026-10.jsonl:833 |
| 2 | 2026-10-29T15:38:05Z | main | `budget_honored` | `smoke-ds4f0731-032-spec7-roce` | budget_honored · floor_mib=21479 · arm_ceiling_mib=18407 · remaining_s=7200 · 창 시작 2026-10-29T15:38:05Z 후 0m00s | docs/logs/main/events/watchdog.jsonl:869 |
| 3 | 2026-10-29T15:39:29Z | cluster | `engine_log_window_start` | — | 캡처된 엔진 로그의 첫 시각(꼬리 캡처일 수 있다 — 엔진 시작 시각이 아닐 수 있다) · 선언 2026-10-29T15:38:05Z 후 1m24s · 시계: 컨테이너 시각을 UTC 로 읽어 측정 창 [2026-10-29T15:38:05Z, 2026-10-29T16:10:00Z] 안에 드는 것을 확인 | output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/lite_engine_ds4f0731-032-spec7-roce.log:181 |
| 4 | 2026-10-29T15:39:34Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=62563 · rate_mib_s=23865 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → smoke-ds4f0731-032-spec7-roce 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T15:38:05Z 후 1m29s) | docs/logs/main/events/watchdog.jsonl:870 |
| 5 | 2026-10-29T15:39:36Z | cluster | `engine_prefetch_disabled` | — | 가중치 auto-prefetch 꺼짐 줄 ×4 — filesystem CIFS(vLLM 로그 문구) | output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/lite_engine_ds4f0731-032-spec7-roce.log:533 |
| 6 | 2026-10-29T15:51:45Z | cluster | `engine_weights_loaded` | — | 가중치 적재 완료 줄 ×4(rank·단계별) · 최장 352.76s · 이 행 = 마지막 줄 | output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/lite_engine_ds4f0731-032-spec7-roce.log:568 |
| 7 | 2026-10-29T15:52:01Z | cluster | `engine_model_loaded` | — | 모델 적재 완료 줄 ×2(rank 별) · 마지막 줄 79.02 GiB · 750.1s | output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/lite_engine_ds4f0731-032-spec7-roce.log:575 |
| 8 | 2026-10-29T15:52:48Z | cluster | `engine_kv_cache_sized` | — | GPU KV cache 1,034,763 tokens · 요청당 262,144 tokens 기준 최대 동시성 3.95x | output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/lite_engine_ds4f0731-032-spec7-roce.log:622 |
| 9 | 2026-10-29T15:53:50Z | cluster | `engine_shm_broadcast_wait` | — | shm_broadcast 대기 경고(60s 단위) ×3 · 2026-10-29T15:53:50Z–2026-10-29T15:55:50Z(2m00s) — vLLM 문구상 프로세스 hang 또는 긴 작업(컴파일·가중치/KV 양자화) 중 출현 | output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/lite_engine_ds4f0731-032-spec7-roce.log:632 |
| 10 | 2026-10-29T15:56:36Z | cluster | `engine_init_done` | — | 엔진 초기화(profile·KV 할당·warmup) 274.21s | output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/lite_engine_ds4f0731-032-spec7-roce.log:725 |
| 11 | 2026-10-29T15:56:52Z | cluster | `server_starting` | — | API 서버 기동 — 이 측정 창 선언 2026-10-29T15:38:05Z 후 18m47s · 캡처 로그 첫 시각 2026-10-29T15:39:29Z 후 17m23s | output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/lite_engine_ds4f0731-032-spec7-roce.log:734 |
| 12 | 2026-10-29T15:57:10Z | main | `budget_renew` | `smoke-ds4f0731-032-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=6055 · 창 시작 2026-10-29T15:38:05Z 후 19m05s | docs/logs/main/events/2026-10.jsonl:834 |
| 13 | 2026-10-29T16:02:11Z | main | `budget_renew` | `smoke-ds4f0731-032-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=6899 · 창 시작 2026-10-29T15:38:05Z 후 24m06s | docs/logs/main/events/2026-10.jsonl:835 |
| 14 | 2026-10-29T16:04:38Z | main | `budget_renew` | `smoke-ds4f0731-032-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=7053 · 창 시작 2026-10-29T15:38:05Z 후 26m33s | docs/logs/main/events/2026-10.jsonl:836 |
| 15 | 2026-10-29T16:06:49Z | main | `budget_renew` | `smoke-ds4f0731-032-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=7069 · 창 시작 2026-10-29T15:38:05Z 후 28m44s | docs/logs/main/events/2026-10.jsonl:837 |
| 16 | 2026-10-29T16:10:00Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-ds4f0731-032-spec7-roce 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T15:38:05Z 후 31m55s) | docs/logs/main/events/2026-10.jsonl:838 |
| 17 | 2026-10-29T16:10:00Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-ds4f0731-032-spec7-roce 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T15:38:05Z 후 31m55s) | docs/logs/main/events/watchdog.jsonl:871 |

**기동 시도**(예산 선언 `budget_declare` 마다 한 시도 · 같은 노드의 이어지는 행과 선언 없는 노드(엔진 로그)의 그 시각 행을 묶었다 · 사살 · 트립 · 거부는 닫힘과 따로 센다 · `measurement` 행은 측정 기록이라 시도로 세지 않는다(위 표에만) — 해석은 저작자 몫)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 1 | main | `smoke-ds4f0731-032-spec7-roce` | 2026-10-29T15:38:05Z | 4회 · 첫 2026-10-29T15:57:10Z | 없음 | `budget_clear` 2026-10-29T16:10:00Z | 11 |
<!-- /FACT:event_timeline -->

<!-- FACT:measurement_env -->
**측정 실행의 env 관측 32행**(측정한 실행의 엔진 로그가 echo 한 값 · 기계 발췌 — 측정 당시 값의 정본이다. 표에 없는 키는 로그가 echo 하지 않은 것이지 설정되지 않았다는 뜻이 아니다 · 측정 뒤 재생성된 env 형상은 01 §1.1 을 본다) — 그중 3행은 종류 `key-present`: 키가 프로세스 env 에 **있었다**는 관측뿐이고 값은 로그에 없다(값 미관측)

| # | 키 | 값 | 노드 | 종류 | 출처(첫 줄) | 관측 |
|---|---|---|---|---|---|---|
| 1 | `VLLM_PLE_MMAP` | (키 존재 · 값 미관측) | 미특정 | key-present | output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/lite_engine_head_ds4f0731-032-spec7-roce.log:L64 | 로그 1개 |
| 2 | `VLLM_PLE_MMAP_DIR` | (키 존재 · 값 미관측) | 미특정 | key-present | output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/lite_engine_head_ds4f0731-032-spec7-roce.log:L65 | 로그 1개 |
| 3 | `VLLM_REF` | (키 존재 · 값 미관측) | 미특정 | key-present | output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/lite_engine_head_ds4f0731-032-spec7-roce.log:L66 | 로그 1개 |
| 4 | `NCCL_SOCKET_IFNAME` | `<nic:cluster>` | main | env-echo | output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/lite_engine_head_ds4f0731-032-spec7-roce.log:L98 | 로그 1개 |
| 5 | `NCCL_SOCKET_IFNAME` | `<nic:cluster>` | main | param-echo | output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/lite_engine_head_ds4f0731-032-spec7-roce.log:L99 | 로그 1개 |
| 6 | `nccl:version` | `2.30.7+cuda13.3` | main | runtime | output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/lite_engine_head_ds4f0731-032-spec7-roce.log:L102 | 로그 1개 |
| 7 | `NCCL_NET_PLUGIN` | `spcx` | main | env-echo | output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/lite_engine_head_ds4f0731-032-spec7-roce.log:L104 | 로그 1개 |
| 8 | `NCCL_IB_DISABLE` | `0` | main | env-echo | output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/lite_engine_head_ds4f0731-032-spec7-roce.log:L109 | 로그 1개 |
| 9 | `NCCL_IB_HCA` | `=<nic:cluster>,<nic:cluster>` | main | param-echo | output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/lite_engine_head_ds4f0731-032-spec7-roce.log:L111 | 로그 1개 |
| 10 | `NCCL_IB_MERGE_NICS` | `1` | main | env-echo | output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/lite_engine_head_ds4f0731-032-spec7-roce.log:L114 | 로그 1개 |
| 11 | `nccl:net_ib_devices` | `[0]<nic:cluster>:1/RoCE [1]<nic:cluster>:1/RoCE [RO]; OOB <nic:cluster>:<node:main><0>` | main | runtime | output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/lite_engine_head_ds4f0731-032-spec7-roce.log:L117 | 로그 1개 |
| 12 | `NCCL_IB_QPS_PER_CONNECTION` | `4` | main | env-echo | output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/lite_engine_head_ds4f0731-032-spec7-roce.log:L130 | 로그 1개 |
| 13 | `nccl:net_ib_devices` | `[RO]; OOB <nic:cluster>:<node:main><0>` | main | runtime | output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/lite_engine_head_ds4f0731-032-spec7-roce.log:L141 | 로그 1개 |
| 14 | `nccl:network` | `IB` | main | runtime | output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/lite_engine_head_ds4f0731-032-spec7-roce.log:L153 | 로그 1개 |
| 15 | `NCCL_DMABUF_ENABLE` | `0` | main | env-echo | output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/lite_engine_head_ds4f0731-032-spec7-roce.log:L155 | 로그 1개 |
| 16 | `NCCL_CROSS_NIC` | `1` | main | env-echo | output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/lite_engine_head_ds4f0731-032-spec7-roce.log:L158 | 로그 1개 |
| 17 | `NCCL_IB_SPLIT_DATA_ON_QPS` | `0` | main | env-echo | output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/lite_engine_ds4f0731-032-spec7-roce.log:L51 | 로그 1개 |
| 18 | `NCCL_IB_GID_INDEX` | `3` | main | env-echo | output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/lite_engine_ds4f0731-032-spec7-roce.log:L93 | 로그 1개 |
| 19 | `NCCL_SOCKET_IFNAME` | `<nic:cluster>` | sub | env-echo | output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/lite_engine_ds4f0731-032-spec7-roce.log:L321 | 로그 1개 · Ray 접힘 +3(접힌 사본의 노드 미관측) |
| 20 | `NCCL_SOCKET_IFNAME` | `<nic:cluster>` | sub | param-echo | output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/lite_engine_ds4f0731-032-spec7-roce.log:L322 | 로그 1개 · Ray 접힘 +2(접힌 사본의 노드 미관측) |
| 21 | `nccl:version` | `2.30.7+cuda13.3` | sub | runtime | output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/lite_engine_ds4f0731-032-spec7-roce.log:L325 | 로그 1개 |
| 22 | `NCCL_NET_PLUGIN` | `spcx` | sub | env-echo | output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/lite_engine_ds4f0731-032-spec7-roce.log:L327 | 로그 1개 |
| 23 | `NCCL_IB_DISABLE` | `0` | sub | env-echo | output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/lite_engine_ds4f0731-032-spec7-roce.log:L332 | 로그 1개 · Ray 접힘 +2(접힌 사본의 노드 미관측) |
| 24 | `NCCL_IB_HCA` | `=<nic:cluster>,<nic:cluster>` | sub | param-echo | output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/lite_engine_ds4f0731-032-spec7-roce.log:L333 | 로그 1개 · Ray 접힘 +2(접힌 사본의 노드 미관측) |
| 25 | `NCCL_IB_MERGE_NICS` | `1` | sub | env-echo | output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/lite_engine_ds4f0731-032-spec7-roce.log:L335 | 로그 1개 |
| 26 | `nccl:net_ib_devices` | `[RO]; OOB <nic:cluster>:<node:sub><0>` | sub | runtime | output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/lite_engine_ds4f0731-032-spec7-roce.log:L337 | 로그 1개 · Ray 접힘 +2(접힌 사본의 노드 미관측) |
| 27 | `NCCL_IB_QPS_PER_CONNECTION` | `4` | sub | env-echo | output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/lite_engine_ds4f0731-032-spec7-roce.log:L347 | 로그 1개 · Ray 접힘 +3(접힌 사본의 노드 미관측) |
| 28 | `nccl:network` | `IB` | sub | runtime | output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/lite_engine_ds4f0731-032-spec7-roce.log:L360 | 로그 1개 · Ray 접힘 +3(접힌 사본의 노드 미관측) |
| 29 | `NCCL_DMABUF_ENABLE` | `0` | sub | env-echo | output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/lite_engine_ds4f0731-032-spec7-roce.log:L362 | 로그 1개 |
| 30 | `NCCL_CROSS_NIC` | `1` | sub | env-echo | output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/lite_engine_ds4f0731-032-spec7-roce.log:L365 | 로그 1개 |
| 31 | `NCCL_IB_SPLIT_DATA_ON_QPS` | `0` | sub | env-echo | output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/lite_engine_ds4f0731-032-spec7-roce.log:L406 | 로그 1개 |
| 32 | `NCCL_IB_GID_INDEX` | `3` | sub | env-echo | output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/lite_engine_ds4f0731-032-spec7-roce.log:L411 | 로그 1개 |

- 값 주: 기계 치환(pii.substitution_table · 장치·호스트·주소는 자리표시 — 원문은 출처 줄)
- 값 주: 키 존재만 관측 · 값 미관측(vLLM 'Unknown vLLM environment variable detected' 경고 — 값은 로그에 없다)

- Ray 접힘: 출처 줄 끝의 `[repeated Nx across cluster]` — Ray 가 다른 프로세스의 같은 줄 N개를 한 줄로 접었다. 노드 칸은 **첫 줄**의 노드이고, 접힌 사본이 어느 노드의 것인지는 로그에 없다(관측 대상 밖).

- 로그 창(꼬리): `tail-800(lite_bench.sh@d33b0878a6c3 L234)`(꼬리 창 출처 로그 1개 · 이 표 행 #17–32) — 이 캡처는 엔진 출력의 **마지막 N줄**이다(측정 도구의 `tail -N` 상한에 닿았다 · 생산자 evidence `log_window`). 창 앞에서 echo 한 줄은 그 로그에 없다 — 이 주의는 꼬리 창 출처 행에만 걸린다(머리 캡처 행은 아래).

- 로그 창(머리): `head-600(output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/lite_raw_ds4f0731-032-spec7-roce.json engine_log_head_lines)`(머리 캡처 출처 로그 1개 · 이 표 행 #1–16) — 이 캡처는 엔진 출력의 **처음 N줄**이다(lite_bench.sh 머리 캡처 `head -n N` 상한에 닿았다 · 생산자 evidence `log_head_window`). 꼬리 창의 주의는 이 행에 걸리지 않는다 · 머리 N줄 뒤에서 echo 한 줄은 그 로그에 없다. 머리 캡처와 꼬리 창 사이(엔진 출력의 가운데)는 어느 캡처에도 없다 — 표에 없는 키 · 한 노드에만 있는 키는 그 사이였을 수 있다.
<!-- /FACT:measurement_env -->

> 이 절이 답하는 질문: 수신자가 이 zip 만으로 이 셀을 다시 띄우려면 어떤 순서로 무엇을 하고, 각 단계의 성공을 무엇으로 판정하며, 얼마나 기다려야 하는가?

1. 관리NAS의0731공식가중치를양노드동일container경로로마운트한다. main에triplet3개,양노드에각자localbuild한공유이미지가필요하다. 필수env자리는셀env와compose프로젝트/cluster/interconnect형상이며실효값은위측정env표가정본이다. 별도PLE/mmapstage는없다. 전체가중치파일은약155.4GiB이며model+draft실제적재79.02GiB/노드와구분한다.
2. 빌드는parent-source-core/context5파일/corePOST05→alias→leafPOST06순서다. actualcoreargvarch12.1/jobs8과원본context를그대로읽되수신자양노드localbuild안3명령은새generated-unverified다. sub과거원core미회수한계와leafhistory5/10경고는남는다. 위563초층CreatedAt창은cache재사용을포함하므로전체빌드벽시간이아니다.
3. 현재원장기동은선언1창15:38:05Z→16:10:00Z/renew4회다. MemTotal124608MiB만러너실측이고weights79577/KV10240/overhead13312/floor21479/arm18407은선언이다. bench4096은별도main클라이언트예산이다. 자기호스트의현재가용과가중치/KV/overhead를다시도출하고가드거부면정식경로에서형상을바꾼다.
4. mainRayhead+vllmserve후subRayworker합류,모델과DSparkdraft양rank적재,DeepGEMM984예열과graph초기화를기다렸다. 현재READY약1120초,main749.442429s/sub750.079972s모델로드,profile/KV/warmup274.21초다(testlog_26103001_DS4F0731_032_TP2와_Full_판정). 위첫lite까지상한23분15초는더큰창이다. sharedmemory60초경고3회/2분뒤정상init이관측됐으며경고만으로hang판정하지않는다. 성공은health200+chat.content1/reasoning96/stop및실제backend/draft로그다.
5. 엔진은B12X_MXFP4_MXFP8,KV10GiB/1034763토큰·262144당3.95x를보고했다. fp8는dense로더선언과KVdtype,expert는FP4혼합이다. graph/DSpark5가켜졌고실측acceptance는litewarm계승이다. NCCL실효키/NET/IB는위env와아래노드별출처로읽고접힌로그를다른노드부재로판정하지않는다.
6. bench JSON복원명령은실행원문이아니며03§3.4의도구원문과함께본다. lite→GuideLLMC1×3은16:01:20Z→16:08:59Z의7분39초창이었다. 정식downexit0으로컨테이너/watch/TTL/선언을회수했다. 여기에새로운명령실행검증이나서비스반복은없다.

## 1.5 서브 레시피 해설
<!-- FACT:sub_recipe -->
원본: `artifacts/compose/sub_recipe.json`(기계 파생 — 스모크와 같은 함수 `slave_forward.derive` 의 출력).

| 역할 | 하는 일 |
|---|---|
| `master` | ray head + vllm serve(트리플렛 source · 모델 러너 실행) |
| `master_runtime_patch` | none(런타임 패치 파일 없음 — CONFIG_FILE='ds4f0731-032-spec7-roce' → ds4f0731-032-spec7-roce_patch.py 부재) |
| `runner_gpu_join_literal` | {"lines": [79], "note": "master 는 ray status 의 GPU 합류가 이 리터럴과 같아질 때까지 기다린다 — 노드당 GPU·노드 수가 다른 클러스터에서는 이 줄을 고치지 않으면 대기에서 멈춘다(arch 의 <G>g<N>n 과 숨은 결합).", "value": "2"} |
| `runtime_patch_key_chain` | RUNTIME_PATCH_STEM · CONFIG_FILE · 'default' |
| `slave` | ray worker(트리플렛 미수령 — 같은 이미지·같은 compose·같은 serve_runner.sh 가 NODE_ROLE 로 분기) |
| `slave_config_file` | default |
| `slave_runtime_patch` | not-armed — slave CONFIG_FILE='default' → arm_patch.sh 가 /app/configs/default_patch.py 를 찾고 없으면 no-op (K11) |
| `source` | {"runner_gpu_join_literal": "render_dockerfile.SHARED_ASSET_DIR/serve_runner.sh(측정 당시 개정 — slots.compose.evidence.selected_revisions)", "runtime_patch": "output/multi/configs/<stem>_patch.py 실재 여부 × arm_patch.sh 측정 당시 개정(1b1df2cd4034) `_cfg=` 키 체인 RUNTIME_PATCH_STEM > CONFIG_FILE > 'default'", "slave_config_file": "compose-default:CONFIG_FILE × slave_forward.NEVER_FORWARD"} |

**기동 순서**

1. build(양 노드 병렬)
2. master up(head)
3. slave up(join)
4. master: GPU 합류 확인 → vllm serve

**서브 전달 env**(값은 원본 JSON · 여기서는 무리와 이유만)

| 키 | 무리 | 출처 | 이유 |
|---|---|---|---|
| `IMAGE_TAG` | image_identity | cell_env | 이미지 정체성의 이름 — slave 가 모르면 compose 기본 태그로 빌드·기동한다(plan_26062818 §S2.5 R10). |
| `BUILD_DOCKERFILE` | image_identity | cell_env | 2026-07-24 Solar-Open2: slave build 가 --env-file EFC 만 받아 compose 기본 Dockerfile 로 폴백 → 변종 트랙에서 slave 만 다른 Dockerfile 로 빌드됐다(기본값 콤보에서만 잠복). |
| `VLLM_REF` | image_identity | cell_env | 포크/릴리스 ref — 빠지면 slave 가 compose 기본 ref 로 빌드된다(plan_26062818 §S2.5 R10). |
| `BUILD_JOBS` | build_tuning | cell_env | 2026-08-15: 이미지 정체성은 아니지만 양 노드에 같아야 한다 — slave 만 기본 16 으로 컴파일하면 거기서 OOM(하드다운 #2 가 서브였다). |
| `NAS_MODEL_PATH` | mount | project_env | 결함#2b(plan_26070119): --env-file 을 쓰면 프로젝트 .env 자동 로드가 꺼져 compose 기본 마운트로 폴백 → 모델 부재(serve 즉사). 셸 env 로 명시 주입한다. |
| `QUANT_MODEL_PATH` | mount | project_env | 결함#2b 와 같은 경로 — 양자화 모델 마운트원. |
| `TIKTOKEN_HOST_PATH` | mount | project_env | 결함#2b 와 같은 경로 — 오프라인 토크나이저 마운트원. |
| `PLE_MMAP_HOST_PATH` | mount | project_env | 2026-09-09(camp-26090918): PLE mmap 로컬 NVMe 스테이징 마운트원 — slave 도 같은 경로에 실재해야 한다. |
| `SLAVE_CONTAINER_NAME` | cluster | cell_env | 2026-08-14: 폴백 이름이면 slave 협역 워치독 필터가 매칭 0 — 서브에서만 계층 2층이 조용히 사라진다(devlog_26080212 ⑥ 부류 · 하드다운 #2 는 서브였다). 2026-08-02 에 이미 '엉뚱한 이름으로 떠 있었고 아무도 몰랐다' 로 한 번 드러났다(그때는 마스터측만 교정). |

**이미지 정체성 build-arg**: `IMAGE_TAG` · `BUILD_DOCKERFILE` · `VLLM_REF`

**노드별 값(형상)**

| 키 | 자리표시 |
|---|---|
| `MASTER_HOST_IP` | `<manifest.nodes[main].host>` |
| `NAS_MODEL_PATH` | `<manifest.nas_model_path>` |
| `PLE_MMAP_HOST_PATH` | `<manifest.ple_mmap_host_path>` |
| `QUANT_MODEL_PATH` | `<manifest.quant_model_path>` |
| `SLAVE_HOST_IP` | `<manifest.nodes[sub].host>` |
| `SSH_USER` | `<manifest.nodes[main].ssh_user>` |
| `TIKTOKEN_HOST_PATH` | `<manifest.tiktoken_host_path>` |
| `interconnect` | `artifacts/compose/.env.interconnect.template` |

**선행조건**

- 가중치가 같은 마운트 경로에 실재(메인 .env 의 mount 값이 서브에 전달된다)
- PLE mmap 이면 slave 로컬 스테이징도 실재
- vLLM SHA · torch · driver 동일(동일 이미지가 아니라 동일 ABI)

**ABI 동일성 관측**(같아야 하는 것은 digest 가 아니라 ABI — 노드마다 로컬 빌드)

| 축 | 관측 |
|---|---|
| `axes` | vllm_sha · torch · driver · build_ledger |
| `blocking` | false |
| `observed` | {"build_ledger": {"compared": "dockerfile · build_args(− exempt) · patches(phase,file,script_sha256,result) · vllm git sha", "identity_exempt_build_args": ["BUILD_JOBS"], "main_identity_sha256": "fbabd8e2535e244426ec94915ad0187d83030f8962a03d35d395f259a352cb5e", "sub_identity_sha256": "7a80098a295b98439d068d9ae11f0c7e6925dffe13681cf12011e93e4d7b6f17", "verdict": "differ"}, "driver": {"main": "580.178.04", "sub": "580.178.04", "verdict": "equal"}, "torch": {"main": "2.13.0a0+9186a08b2c.nv26.7.59513937", "sub": "2.13.0a0+9186a08b2c.nv26.7.59513937", "verdict": "equal"}, "vllm_sha": {"main": "5df63a7203cdb9328d824a27b682e77c7b9928be", "sub": "5df63a7203cdb9328d824a27b682e77c7b9928be", "verdict": "equal"}} |
| `source` | output/multi/benchlog/attestation_ds4f0731-032-spec7-roce.json |

> **attestation 범위** — config `ds4f0731-032-spec7-roce` · phase serve · 작성 2026-10-29T15:57:10Z(output/multi/benchlog/attestation_ds4f0731-032-spec7-roce.json 파일 mtime(파일시스템 관측 — 문서에 시각 칸이 없다 · 기록 필드 아님)) · 묶은 근거 cell-name · 파일 `output/multi/benchlog/attestation_ds4f0731-032-spec7-roce.json` · 측정 창 대비 작성 2026-10-29T15:57:10Z < 측정 시작 2026-10-29T16:01:20Z — 측정 전(빌드·스모크)의 관측: **same config(ds4f0731-032-spec7-roce) · serve — 원장 순서로 판별한 같은 기동의 관측(결속된 스모크 로그 미확인 · 제한검색 · 약한 판별)(판별: 원장 순서로 판별(결속된 스모크 로그 미확인 · 제한검색 — docs/simlog/**/*serve*.log 에서 이 attestation 을 적은 로그 미확인; 다른 경로의 보존 여부는 판정하지 않음): 원장 docs/logs/main/events/2026-10.jsonl:833 budget_declare(smoke-ds4f0731-032-spec7-roce · 2026-10-29T15:38:05Z) ≤ attestation 작성 2026-10-29T15:57:10Z · serve_proof 작성 2026-10-29T15:57:10Z < 측정 시작 2026-10-29T16:01:20Z · 그 선언이 측정 끝 2026-10-29T16:08:59Z 까지 재선언 · 창 닫힘 없이 이어짐(main 원장 기록 범위 안) · API 서버 기동 2026-10-29T15:56:52Z(output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/lite_engine_ds4f0731-032-spec7-roce.log:734 · 컨테이너 시계 = UTC 가정) ∈ [선언, attestation 작성] · 측정 엔진 로그 APIServer pid 791 하나)**
<!-- /FACT:sub_recipe -->

<!-- FACT:measurement_env_nodes -->
**노드별 측정 env**(01 §1.4 표를 키 × 노드로 접은 것 · `만 관측` = 다른 쪽 줄이 캡처 로그에 없다 · Ray 가 같은 줄을 접은 키(`[repeated Nx across cluster]`)와 머리·꼬리 캡처 로그(01 §1.4 로그 창)에서 한쪽만 남은 키는 다른 쪽 부재를 단언하지 않는다)

- 양 노드 같은 값 12개: `NCCL_CROSS_NIC` · `NCCL_DMABUF_ENABLE` · `NCCL_IB_DISABLE` · `NCCL_IB_GID_INDEX` · `NCCL_IB_HCA` · `NCCL_IB_MERGE_NICS` · `NCCL_IB_QPS_PER_CONNECTION` · `NCCL_IB_SPLIT_DATA_ON_QPS` · `NCCL_NET_PLUGIN` · `NCCL_SOCKET_IFNAME` · `nccl:network` · `nccl:version`

| 키 | main | sub | 노드 미특정 | 판정 |
|---|---|---|---|---|
| `VLLM_PLE_MMAP` | — | — | (키 존재 · 값 미관측) | 노드 미특정 |
| `VLLM_PLE_MMAP_DIR` | — | — | (키 존재 · 값 미관측) | 노드 미특정 |
| `VLLM_REF` | — | — | (키 존재 · 값 미관측) | 노드 미특정 |
| `nccl:net_ib_devices` | `[0]<nic:cluster>:1/RoCE [1]<nic:cluster>:1/RoCE [RO]; OOB <nic:cluster>:<node:main><0>` · `[RO]; OOB <nic:cluster>:<node:main><0>` | `[RO]; OOB <nic:cluster>:<node:sub><0>` | — | 다름 |
<!-- /FACT:measurement_env_nodes -->

> 이 절이 답하는 질문: 서브(Ray worker) 노드는 메인과 무엇이 다르고, 무엇을 반드시 같게 맞춰야 하는가?

main은Rayhead/APIserve와triplet수령,sub는Rayworker로서가중치/draft를로드한다. 이셀은runtime패치가없고sub에모델트리플렛을보내지않는다. 이미지핀/빌드튜닝,클러스터·NCCL통신,가중치마운트와공통서빙env가필요무리다.
양노드동일container경로의NAS접근·호환driver·RoCE선행조건을맞춘다. 관측된SHA/torch/driver는equal이나imageId/parentledger/SCM날짜는다르다(testlog_26103001_DS4F0731_032_TP2와_Full_판정). image바이트동일이나sub부모core역사동등을주장하지않는다. attestation은측정전관측이고원장기반같은기동판별의약한범위줄을보존한다.
측정env/NCCL키는위노드별표와01§1.4출처줄의값이정본이다. 현재렌더와다르면측정당시관측을기준으로자기NIC/endpoint형상을재도출한다. Ray접힘/꼬리창/노드미특정줄은다른노드미설정이아니다. 역사SPCX스킵→Socket/IB구분(W12)을현재설정없이결론으로쓰지않는다.

## 1.6 재현 절차 — 기동 전 준비(사람이 할 일)
> 1.4 의 명령을 치기 **전에** 사람이 손으로 해야 하는 것이다 — 가중치 획득 · 스테이징 생성 · 네트워크가 필요한 단계. 에이전트가 대신 할 수 없는 자리(다운로드 승인 · 물리 경로 · 외부 계정)가 여기 모인다.

> 이 절이 답하는 질문: 이 셀을 기동하기 전에 사람이 준비해야 하는 것은 무엇이며, 각각을 어떤 명령으로 만들고, 그 절차의 출처는 어디인가?

### 가중치
기존관리NAS직독이며새자동획득은없었다(plan_26102919_멀티4종_v032_완주). deepseek-ai/DeepSeek-V4-Flash-0731 revision7872f01b1d1fe23eabc4c98b48bffcef5a386062를양노드`/app/models/DeepSeek/DeepSeek-V4/DeepSeek-V4-Flash-0731`로보이게한다. 확인예시 `test -r <manifest.nas_model_path>/DeepSeek/DeepSeek-V4/DeepSeek-V4-Flash-0731/config.json`은추론—실행검증✗이며실제mount는env형상에서정한다. 외부Abliterated/다른checkpoint는이입력의대체가아니다.
### 스테이징
PLE별도생성은해당없음—configPLE키부재(00§0.4)다. sourcecore5파일context와빈PRE를보존하고metadata는context밖에둔다. leafPOST06복사및corebuild/alias/leaf명령은parent-source-core/README의generated-unverified새안이며양노드재빌드성공으로적지않는다. GuideLLM토크나이저stage는측정도구가필요파일만복사하며config는제외한다(03§3.4도구원문).
### 네트워크
원본Dockerfile/requirements/POST05/06이NGC26.07basepull,공식vLLMexactSHA git,hub/pytest/B12Xwheel/pip접속을정한다. parentREADME재구성명령은실제coreargv·alias관측·leaf원장과구분한다. 오프라인이면동일base/source/wheel/의존파일사전준비후localbuild/스모크가필요하다는대안은추론—실행검증✗다. 완성이미지전송을localbuild대신쓰지않는다.
