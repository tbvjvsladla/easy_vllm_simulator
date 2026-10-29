# 01 · 산출물 — 무엇이 실제로 쓰였고, 어떻게 다시 띄우는가

## 1.1 적용 판정 표
> 기계가 적용 증거(빌드 원장 · 원장이 없으면 라벨된 재구성) × 파일 × 선언을 대사한 결과다. `artifacts/` 에는 재현 입력과 확인되지 않은 참고 형상도 포함될 수 있다. 파일별 검증 상태와 근거를 아래 표에서 확인한다.

<!-- FACT:slots -->
| 슬롯 | 실림 | 파일 | 신호 | 적용 증거 | 사유 |
|---|---|---|---|---|---|
| `build_patch_post` | 실림 | `artifacts/build_patch_post/06-b12x-optional032.sh` | 3-signal | build-ledger:attestation:output/multi/benchlog/attestation_q38fn032-nv4-safe-tp2.json#main | 06-b12x-optional032.sh는SM120/121 optional backend공유의존이다. 정확wheelURL/해시를pip설치하고version1.5.1 assertion과pipcheck가실패하면set-e로빌드를중단한다. 모델trigger가안맞으면자동skip하는가드는없다. 상태줄applied는설치/pipcheck이며GPUkernel미검증을명시한다. 선택B의MoE경로는CUTLASS/TRITON이므로설치됐다는사실을nativeb12x사용으로읽지않는다. |
| `build_patch_pre` | 안 실림 | — | 1-signal | build-ledger:attestation:output/multi/benchlog/attestation_q38fn032-nv4-safe-tp2.json#main | 적용 판정 없음 — 원장 관측(기계 파생) |
| `build_recipe` | 실림 | `artifacts/build_recipe/06-b12x-optional032.sh` · `artifacts/build_recipe/Dockerfile.source-build-deps2` · `artifacts/build_recipe/parent-source-core/README.md` · `artifacts/build_recipe/parent-source-core/alias-evidence.json` · `artifacts/build_recipe/parent-source-core/context/Dockerfile.source-build` · `artifacts/build_recipe/parent-source-core/context/build_patches/.gitkeep` · `artifacts/build_recipe/parent-source-core/context/build_patches/05-ngc-source032-deps-compat.sh` · `artifacts/build_recipe/parent-source-core/context/build_patches_src/.gitkeep` · `artifacts/build_recipe/parent-source-core/context/requirements.txt` · `artifacts/build_recipe/parent-source-core/core-build-command.txt` · `artifacts/build_recipe/parent-source-core/leaf-ledger-evidence.json` · `artifacts/build_recipe/parent-source-core/manifest.json` | 3-signal | build-ledger:build-ledger.dockerfile | 실린Dockerfile.source-build-deps2는history5/10일치의generated-unverified참고형상이다. deps1부모에서공식40SHA를test하고원장을parents로보존한뒤06만COPY·실행하는층이다. 이파일전체가실제build바이트와같다는보장은없다. 같이실린06은원장script hash와일치한verified바이트다. |
| `compose` | 실림 | `artifacts/compose/.env.cluster.template` · `artifacts/compose/.env.interconnect.template` · `artifacts/compose/.env.template` · `artifacts/compose/docker-compose.yaml` · `artifacts/compose/serve_runner.sh` · `artifacts/compose/sub_recipe.json` | 1-signal | file:output/multi/docker-compose.yaml | multi 서빙 경로(compose · 러너 · env 형상) — 측정 당시 개정(selected_revisions)(기계 파생) |
| `fork_pin` | 안 실림 | — | 1-signal | none:.claude/policies/arch_variant_ledger.json | 셀 env 에 VARIANT 줄 없음 = stock(workflow.md §변종 좌표의 거처)(기계 파생) |
| `runtime_patch` | 안 실림 | — | 1-signal | none:output/multi/configs/q38fn032-nv4-safe-tp2_patch.py | output/multi/configs/q38fn032-nv4-safe-tp2_patch.py 없음 — arm_patch.sh 는 no-op(런타임 패치 불해당)(기계 파생) |
| `triplet` | 실림 | `artifacts/triplet/.env.q38fn032-nv4-safe-tp2` · `artifacts/triplet/q38fn032-nv4-safe-tp2.sh` · `artifacts/triplet/q38fn032-nv4-safe-tp2.yaml` | 2-signal | file:output/multi/configs/q38fn032-nv4-safe-tp2.yaml | 셀 id q38fn032-nv4-safe-tp2 = 트리플렛 이름(configs/q38fn032-nv4-safe-tp2.yaml · .sh · envs/.env.q38fn032-nv4-safe-tp2)(기계 파생) |

**레시피 경고 · 실린 리비전**(기계 관측 — 수신자 빌드가 여기서 갈라질 수 있다)

- ⚠ `build_recipe` `output/multi/Dockerfile.source-build-deps2` — 실린 바이트(worktree)가 측정 이미지 docker history 의 RUN·COPY 와 전수 일치하지 않는다(5/10)(이미지 Created 2026-10-29T10:50:18Z · 마지막 커밋 미관측)
- `build_recipe` `output/multi/Dockerfile.source-build-deps2` 실린 리비전: 리비전 미특정 · 선택 방법 `worktree(어느 개정도 이미지 history 와 더 맞지 않는다 · 작업트리 5/10 · 대조할 개정 없음(이미지 Created 이전 커밋 부재 · 비추적) · 전 이력(output/*/Dockerfile.source-build-deps2 · 모든 ref)에도 전수 일치 개정 없음 — recipe_vs_image 경고 유지)` · docker history 대조 5/10 · 실린 바이트 `worktree` · 쓰인 바이트임을 관측으로 확인 아니오
- `build_recipe` `output/multi/build_patches/06-b12x-optional032.sh` 실린 리비전: 리비전 미특정 · 선택 방법 `worktree(이미지 안 COPY 대상 바이트 미관측 — 이미지 탐침 안 함)` · docker history 대조 없음 · 실린 바이트 `worktree` · 쓰인 바이트임을 관측으로 확인 아니오
- `compose` `.claude/skills/upstream-version-watch/assets/configs/serve_runner.sh` 실린 리비전: 커밋 `50a5ce486738` · 선택 방법 `worktree(mtime 2026-09-06T21:57:53Z ≤ 측정 2026-10-29T11:41:26Z(sweep_index.generated_utc) · 바이트 = 측정 전 마지막 커밋 50a5ce486738)` · docker history 대조 없음 · 실린 바이트 `worktree` · 근거 `mtime≤measured` · 쓰인 바이트임을 관측으로 확인 아니오
- `compose` `output/multi/docker-compose.yaml` 실린 리비전: 리비전 미특정 · 선택 방법 `worktree(mtime 2026-10-29T10:01:25Z ≤ 측정 2026-10-29T11:41:26Z(sweep_index.generated_utc) · 측정 전 마지막 커밋 d781f26e4ebc 와 다르다 — 측정 당시 바이트는 미검증)` · docker history 대조 없음 · 실린 바이트 `worktree` · 근거 `mtime≤measured` · 쓰인 바이트임을 관측으로 확인 아니오 · 측정 뒤 커밋 1건
- ⚠ `triplet` 측정 뒤 재생성된 파일(mtime > 측정 시각)을 실었다: `output/multi/configs/q38fn032-nv4-safe-tp2.yaml` · `output/multi/configs/q38fn032-nv4-safe-tp2.sh` · `output/multi/envs/.env.q38fn032-nv4-safe-tp2` — 실린 형상은 측정 당시 값과 다를 수 있다(측정 당시 값 = 01 §1.4 측정 env 관측)

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
- `compose` `arm_patch.sh` — `not-armed(output/multi/configs/q38fn032-nv4-safe-tp2_patch.py 없음(CONFIG_FILE) · slave default_patch.py(CONFIG_FILE) 없음 — arm_patch.sh 는 no-op · arm_patch.sh 측정 당시 개정(1b1df2cd4034) `_cfg=` 키 체인 RUNTIME_PATCH_STEM > CONFIG_FILE > 'default')`

**파일별 검증 · 관련성**(`generated-unverified` = 실행 검증되지 않은 생성물 — 파일 첫 줄 경고 · PAYLOAD 파일 기록과 같은 표시 · 관련성 = ① 패치 선언 대상 × 모델 config ② 엔진 로그 발화 서명 — 둘 다 관측일 때만 required/inactive-inferred)

| 슬롯 | 파일 | 검증 | 검증 근거 | 관련성 | 관련성 근거 |
|---|---|---|---|---|---|
| `build_patch_post` | `artifacts/build_patch_post/06-b12x-optional032.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_q38fn032-nv4-safe-tp2.json#main) · 원장 관측 | `unknown` | ① 패치 헤더·본문에 대상 선언(model-trigger 아키텍처 · vllm/models/<model_type>) 없음 |
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
| `compose` | `artifacts/compose/docker-compose.yaml` | `generated-unverified` · renderer | docker-compose.yaml: worktree(mtime 2026-10-29T10:01:25Z ≤ 측정 2026-10-29T11:41:26Z(sweep_index.generated_utc) · 측정 전 마지막 커밋 d781f26e4ebc 와 다르다 — 측정 당시 바이트는 미검증) — 측정 당시 실행 바이트는 별도 관측되지 않았다 | `unknown` | 파일의 실행 당시 바이트가 확인되지 않았다(generated-unverified · 생성 renderer) — 이 표시만으로 경로 실행 여부나 필요 여부를 판정하지 않는다(참고 형상) |
| `compose` | `artifacts/compose/serve_runner.sh` | `generated-unverified` · renderer | serve_runner.sh: worktree(mtime 2026-09-06T21:57:53Z ≤ 측정 2026-10-29T11:41:26Z(sweep_index.generated_utc) · 바이트 = 측정 전 마지막 커밋 50a5ce486738) — 측정 당시 실행 바이트는 별도 관측되지 않았다 | `unknown` | 파일의 실행 당시 바이트가 확인되지 않았다(generated-unverified · 생성 renderer) — 이 표시만으로 경로 실행 여부나 필요 여부를 판정하지 않는다(참고 형상) |
| `compose` | `artifacts/compose/sub_recipe.json` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/.env.q38fn032-nv4-safe-tp2` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) · ⚠ 측정(2026-10-29T11:41:26Z) 뒤 mtime: ['output/multi/configs/q38fn032-nv4-safe-tp2.yaml', 'output/multi/configs/q38fn032-nv4-safe-tp2.sh', 'output/multi/envs/.env.q38fn032-nv4-safe-tp2'] — 실린 값이 측정 당시 값과 같다는 보장 없음 | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/q38fn032-nv4-safe-tp2.sh` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) · ⚠ 측정(2026-10-29T11:41:26Z) 뒤 mtime: ['output/multi/configs/q38fn032-nv4-safe-tp2.yaml', 'output/multi/configs/q38fn032-nv4-safe-tp2.sh', 'output/multi/envs/.env.q38fn032-nv4-safe-tp2'] — 실린 값이 측정 당시 값과 같다는 보장 없음 | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/q38fn032-nv4-safe-tp2.yaml` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) · ⚠ 측정(2026-10-29T11:41:26Z) 뒤 mtime: ['output/multi/configs/q38fn032-nv4-safe-tp2.yaml', 'output/multi/configs/q38fn032-nv4-safe-tp2.sh', 'output/multi/envs/.env.q38fn032-nv4-safe-tp2'] — 실린 값이 측정 당시 값과 같다는 보장 없음 | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
<!-- /FACT:slots -->

<!-- FACT:applied_set -->
**적용 집합** — 상태 `observed` · 출처 `attestation:output/multi/benchlog/attestation_q38fn032-nv4-safe-tp2.json#main`

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
| attestation | observed | output/multi/benchlog/attestation_q38fn032-nv4-safe-tp2.json |

> **attestation 범위** — config `q38fn032-nv4-safe-tp2` · phase serve · 작성 2026-10-29T11:42:14Z(output/multi/benchlog/attestation_q38fn032-nv4-safe-tp2.json 파일 mtime(파일시스템 관측 — 문서에 시각 칸이 없다 · 기록 필드 아님)) · 묶은 근거 cell-name · 파일 `output/multi/benchlog/attestation_q38fn032-nv4-safe-tp2.json` · 측정 창 대비 작성 2026-10-29T11:42:14Z > 측정 끝 2026-10-29T11:41:26Z — 측정 뒤 작성된 관측(측정 시점·같은 기동 여부는 시각만으로 미검증): **same config(q38fn032-nv4-safe-tp2) · serve — 측정 뒤 작성된 관측(측정 시점·같은 기동 여부는 시각만으로 미검증 · 파일은 다음 실행이 덮는다)**
<!-- /FACT:applied_set -->

## 1.2 슬롯별 적용 사유
> 이 절이 답하는 질문: 실린 산출물 하나하나는 무엇을 고치며, 왜 이 셀에 필요했고, 이 셀에서 실제로 발화했으며, 언제 불필요해지는가?

### build_recipe
실린Dockerfile.source-build-deps2는history5/10일치의generated-unverified참고형상이다. deps1부모에서공식40SHA를test하고원장을parents로보존한뒤06만COPY·실행하는층이다. 이파일전체가실제build바이트와같다는보장은없다. 같이실린06은원장script hash와일치한verified바이트다.
### 부모 source core 재현 키트
`artifacts/build_recipe/parent-source-core/context/`가 별도 core context_root다. 실제 main에 사용한 `Dockerfile.source-build`, `requirements.txt`, POST05와 두 `.gitkeep`의5파일만들어있다. PRE는빈스캐폴드뿐이며현재i5 PRE/POST를섞지않는다. alias/command/leafledger/README/manifest는context밖에둔다. 실제core명령의arch는12.1, BUILD_JOBS는8이고공식40SHA를고정했다.

> [원문] artifacts/build_recipe/parent-source-core/core-build-command.txt §L1-1
> docker build --progress plain --build-arg VLLM_REF=5df63a7203cdb9328d824a27b682e77c7b9928be --build-arg TORCH_CUDA_ARCH=12.1 --build-arg BUILD_JOBS=8 -t easy-vllm:0.32.0-cu133-aarch64-source -f output/multi/Dockerfile.source-build output/multi/cache/v032-stock-build

main은core+POST05의hub/pytest정합→같은imageId의deps1alias(별도build없음)→deps2/POST06으로진행했다. 이역사의명령은`core-build-command.txt`,alias는`alias-evidence.json`,leafargs는`leaf-ledger-evidence.json`이관측근거다. deps2전체CLI원문은회수되지않아README의leaf명령은원장근거재구성이다.

sub은이전core→실제별도deps1수리→deps2의다른역사이며최초subcoreDockerfile전체원본은회수근거에없다. 새README는main의완결경로를양노드각자새로localbuild하는재현안이다. README/manifest와parent_source_core·alias·leaf의세재구성명령은generated-unverified/미실행이며관측된maincore명령·원본Dockerfile과다른지위다. 이키트추가로sub과거바이트동등이나새양노드ABI·서빙을검증했다고주장하지않는다.

### build_patch_post
06-b12x-optional032.sh는SM120/121 optional backend공유의존이다. 정확wheelURL/해시를pip설치하고version1.5.1 assertion과pipcheck가실패하면set-e로빌드를중단한다. 모델trigger가안맞으면자동skip하는가드는없다. 상태줄applied는설치/pipcheck이며GPUkernel미검증을명시한다. 선택B의MoE경로는CUTLASS/TRITON이므로설치됐다는사실을nativeb12x사용으로읽지않는다.
### compose와triplet
compose·serve_runner는generated-unverified 참고형상, env형상·sub_recipe·트리플렛은각FACT의verified범위다. master는Rayhead와vllm을실행하고slave는worker로합류한다. triplet의12노브는01§1.3에설정/지위로기록한다. mainenv40SHA와flatselector정정은같은이미지/서빙형상의출처결속이다(testlog_26103000_Qwen032_유효Full과_발행후보_선택).
### 불해당슬롯
fork-pin은VARIANT부재stock, runtimepatch는셀patch.py부재no-op다. post06은applied로실렸고다른모델EXL3파일들은이번선택B의적용패치가아니다. 초안의exactCOPY06을제외하던생산자오판은정식재파생으로해소됐다. 현재applied와COPY근거는06하나에한정되며미복사i5슬롯을적용한것으로넓히지않는다.

## 1.3 값의 지위표
> 서빙 설정의 노브 **전부**와 기계가 모은 후보 지위다(lockset 출처 · yaml 주석 · 엔진 기본값). 최종 지위는 아래 `kind: value-status` 블록이 정한다 — 표의 노브마다 블록이 정확히 하나 있어야 린터를 통과한다.

<!-- FACT:value_status -->
서빙 노브 12개 — 노브마다 `kind: value-status` 블록이 **정확히 1개** 있어야 한다(노브·값은 이 표의 글자 그대로 · 값 칸의 `\|` 는 `|` 로 적는다).

| 노브 | 값 | 후보 지위 | 후보 근거 | yaml 주석 | 출처 |
|---|---|---|---|---|---|
| `tensor-parallel-size` | 2 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `distributed-executor-backend` | ray | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `gpu-memory-utilization` | 0.8 | inherited | lockset 출처 gmu_source=hand · lockset provenance=hand-authored | — | lockset.gmu_source=hand |
| `max-model-len` | 262144 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `kv-cache-dtype` | fp8_e4m3 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `kv-cache-memory-bytes` | 15032385536 | inherited | lockset 출처 kv_source=hand · lockset provenance=hand-authored | — | lockset.kv_source=hand |
| `enforce-eager` | false | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `max-num-seqs` | 6 | inherited | lockset 출처 batch_source=hand-lever · lockset provenance=hand-authored | — | lockset.batch_source=hand-lever |
| `max-num-batched-tokens` | 4096 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `engram-config` | {"cpu_offload": false} | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `compilation-config` | {"mode": 0, "cudagraph_mode": "FULL_DECODE_ONLY"} | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `speculative-config` | {"method": "mtp", "num_speculative_tokens": 3} | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
<!-- /FACT:value_status -->

> 이 절이 답하는 질문: 각 서빙 노브의 값은 이 셀에서 어떤 지위인가 — 조정됐나, 승계됐나, 음성대조로 일부러 둔 것인가, 엔진 기본값인가, 필요조건인가?

```hint-event
id: V1
kind: value-status
노브: tensor-parallel-size
값: 2
지위: inherited
근거: manifest의노드당GPU1×2/Ray분산형상
출처: [testlog_26102920_Qwen032_CUTLASS_Full_판정 §빌드와 기능]
```

```hint-event
id: V2
kind: value-status
노브: distributed-executor-backend
값: ray
지위: inherited
근거: 승인된TP2클러스터실행기
출처: [testlog_26102920_Qwen032_CUTLASS_Full_판정 §빌드와 기능]
```

```hint-event
id: V3
kind: value-status
노브: gpu-memory-utilization
값: 0.8
지위: inherited
근거: hand gmu; 이전동일모델근거에서새후보로선언
출처: [testlog_26102920_Qwen032_CUTLASS_Full_판정 §빌드와 기능]
```

```hint-event
id: V4
kind: value-status
노브: max-model-len
값: 262144
지위: inherited
근거: 네이티브context설정;Full부하길이측정과구분
출처: [testlog_26102920_Qwen032_CUTLASS_Full_판정 §빌드와 기능]
```

```hint-event
id: V5
kind: value-status
노브: kv-cache-dtype
값: fp8_e4m3
지위: inherited
근거: 초기후보의FP8 KV;제거대조없음
출처: [testlog_26102920_Qwen032_CUTLASS_Full_판정 §빌드와 기능]
```

```hint-event
id: V6
kind: value-status
노브: kv-cache-memory-bytes
값: 15032385536
지위: inherited
근거: hand KV14GiB/rank;최소필요량수렴아님
출처: [testlog_26102920_Qwen032_CUTLASS_Full_판정 §빌드와 기능]
```

```hint-event
id: V7
kind: value-status
노브: enforce-eager
값: false
지위: inherited
근거: 초기후보graph경로;동일B에서on/off대조없음
출처: [testlog_26102920_Qwen032_CUTLASS_Full_판정 §빌드와 기능]
```

```hint-event
id: V8
kind: value-status
노브: max-num-seqs
값: 6
지위: inherited
근거: hand-lever상한;Full실측동시성1/2/4
출처: [testlog_26102920_Qwen032_CUTLASS_Full_판정 §빌드와 기능]
```

```hint-event
id: V9
kind: value-status
노브: max-num-batched-tokens
값: 4096
지위: inherited
근거: 초기후보chunk입력;최적chunk탐색아님
출처: [testlog_26102920_Qwen032_CUTLASS_Full_판정 §빌드와 기능]
```

```hint-event
id: V10
kind: value-status
노브: engram-config
값: '{"cpu_offload": false}'
지위: inherited
근거: UMA residentPLE선택;현재cpuoffload대조없음
출처: [testlog_26102920_Qwen032_CUTLASS_Full_판정 §빌드와 기능]
```

```hint-event
id: V11
kind: value-status
노브: compilation-config
값: '{"mode": 0, "cudagraph_mode": "FULL_DECODE_ONLY"}'
지위: inherited
근거: 초기compile/graph조합;단독효과미검증
출처: [testlog_26102920_Qwen032_CUTLASS_Full_판정 §빌드와 기능]
```

```hint-event
id: V12
kind: value-status
노브: speculative-config
값: '{"method": "mtp", "num_speculative_tokens": 3}'
지위: inherited
근거: 기존MTP3근거에서선택;수용길이는litewarm실측
출처: [testlog_26102920_Qwen032_CUTLASS_Full_판정 §빌드와 기능]
```

모든값은이번선택B의성립형상이며제거시실패가확인된필요조건이나최적값으로승격하지않는다. KV14GiB는엔진이되읽은선언클램프다.

## 1.4 재현 절차
> 셀 실행 기록에서 기계가 파생한 명령 순서와 단계별 소요다. 소요가 `미관측` 인 단계는 시각 기록이 없다는 뜻이다(0 이 아니다). 그 아래는 블랙박스 이벤트 원장에서 이 셀의 행을 기계가 뽑아 기동 시도로 묶은 것이고, 마지막은 측정한 실행의 엔진 로그가 echo 한 env 다(측정 당시 값의 정본).

<!-- FACT:reproduce -->
| 순서 | 단계 | 성공 판정 | 소요 | 관측 구간(UTC) | 소요 출처 | 명령 출처 |
|---|---|---|---|---|---|---|
| 1 | render | 러너 3종·.env·.env.cluster·.env.interconnect 생성(렌더러 fail-loud) | 미관측 | 미관측 | .claude/skills/upstream-version-watch/scripts/multinode_serve_smoke.sh 선행조건 안내 · .claude/skills/upstream-version-watch/scripts/render_dockerfile.py CLI | derived(렌더러 CLI) |
| 2 | parent_source_core | generated-unverified: 새 양노드 local reproduction 예시 · 미실행 | 미관측 | 미관측 | main observed core+POST05 build → same-ID deps1 alias (no build) → deps2 POST06. Sub historical old core unavailable; this is a new main-path reproduction proposal for independent local builds on both nodes, not sub historical execution proof. | generated-unverified(explicit manifest output/multi/parent-build/qwen032-deps2/manifest.json) |
| 3 | alias | generated-unverified: 새 양노드 local reproduction 예시 · 미실행 | 미관측 | 미관측 | main observed core+POST05 build → same-ID deps1 alias (no build) → deps2 POST06. Sub historical old core unavailable; this is a new main-path reproduction proposal for independent local builds on both nodes, not sub historical execution proof. | generated-unverified(explicit manifest output/multi/parent-build/qwen032-deps2/manifest.json) |
| 4 | build | 빌드 OK · 이미지 실재 | 층 창 9분 23초(docker history 첫 층 → 끝 층 CreatedAt · 캐시 재사용 층 포함 — 빌드 소요 아님) | 2026-10-29T10:40:55Z → 2026-10-29T10:50:18Z (layer-window) | docker history --no-trunc --format '{{json .}}' sha256:8227e7d79471b6b92f21157acfb38c7cc617d1510a5c56f3dec7eb8e48567884 · 우리 Dockerfile 층 22개의 CreatedAt 첫 층 → 끝 층(캐시 재사용 층은 이전 빌드 시각 — 빌드 소요 아님) | generated-unverified(explicit manifest output/multi/parent-build/qwen032-deps2/manifest.json · leaf ledger args; full original CLI unavailable) |
| 5 | serve | health 200 + 추론 1회(스모크 판정) | 미관측(관측 시각: 2026-10-29T11:27:11Z) | ? → 2026-10-29T11:27:11Z (none) | 관측됨(이 측정 전체를 포함하는 예산 창 2개 · 복수 후보이므로 단일 상한 선택 안 함; 서빙 세션·하나의 소요시간으로 합성하지 않음) | derived(스모크 사용법) |
| 6 | bench | 리포트 발행(루브릭 권한 explicit ∧ PASS일 때만 full 인증서) | 14분 15초 | 2026-10-29T11:27:11Z → 2026-10-29T11:41:26Z (exact) | output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B/lite_raw_q38fn032-nv4-safe-tp2.json measured_utc(호스트 date -u · lite_bench.sh 가 cold 레그 직전에 찍는 측정 시작) → sweep_index.generated_utc(호스트 UTC · 두 끝 같은 시계) | reconstructed(bench json + 측정 도구 원문 lite 인자) |

**빌드 층 시각**(관측 · 소요가 아니라 층 창 — 캐시 재사용 층은 이전 빌드의 시각을 가진다)

- `build` 이미지 층 CreatedAt(docker history): 2026-10-29T10:40:55Z → 2026-10-29T10:50:18Z · 층 22개 · 출처 `docker history --no-trunc --format '{{json .}}' sha256:8227e7d79471b6b92f21157acfb38c7cc617d1510a5c56f3dec7eb8e48567884` — 우리 Dockerfile 층의 CreatedAt 범위 — 캐시 재사용 층은 이전 빌드 시각을 가진다(소요 아님)

**단계별 명령**(기계 파생 — `명령 출처`가 `derived(…)`이면 사용법에서 파생하고 `reconstructed(…)`이면 괄호 안에 명시된 관측, 파일 선언·설정 또는 추론을 조합해 재구성한다. 예를 들어 docker history와 bench JSON은 기록된 관측이며 compose 파일은 선언·설정이다. 설정만으로 과거 compose build 실행 여부는 알 수 없다. 어느 명령도 실행 원문(로그에 남은 줄)은 아니다)

**1. render**

```sh
# ⚠ 측정 당시 렌더러 ≠ 작업트리 렌더러: .claude/skills/upstream-version-watch/scripts/render_dockerfile.py 측정 전 마지막 커밋 29cbdbc89a3d · 측정 뒤 커밋 1건(bccd08091976) — 아래 명령을 지금 돌리면 측정 당시와 다른 env 형상·러너가 나올 수 있다
# 측정 당시 env 값 = 01 §1.4 '측정 실행의 env 관측' 표 · PAYLOAD.json measurement_env_observed(측정 엔진 로그 관측) · 측정 당시 렌더러 원문 = git show 29cbdbc89a3d:.claude/skills/upstream-version-watch/scripts/render_dockerfile.py
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
bash .claude/skills/upstream-version-watch/scripts/multinode_serve_smoke.sh q38fn032-nv4-safe-tp2 --keep-up
```

**6. bench**

```sh
# bench JSON 필드로 복원: backend · model_id→--model · tokenizer_id→--tokenizer · num_prompts · max_concurrency · request_rate · burstiness · 요청당 입출력 길이 = total_*_tokens ÷ completed(나누어떨어질 때만 — 평균이다 · 요청별 길이는 JSON 에 없다) · 나누어떨어지면 `--dataset-name random` 으로 추정했다(측정 도구가 그 꼴로 부른다 — 아래 도구 원문).
# JSON 에 없는 인자(--endpoint · --base-url · --ignore-eos · --num-warmups · --temperature · --seed · --random-range-ratio · --trust-remote-code)는 측정 도구가 정했다 — level_* = run_bench.sh · lite_cold/lite_warm = lite_bench.sh(.claude/skills/adversarial-benchmark/scripts/) · 셀 스윕 순서는 sweep_bench.sh.
# lite_cold/lite_warm 의 데이터셋 · 부하 인자 = 측정 시점 도구 원문 lite_bench.sh@d33b0878a6c3 L177-L183(리터럴 인자만 · `$변수` 인자는 도구가 값을 정하지 않아 JSON 필드로 채운다) — `total_*_tokens ÷ completed` 는 실측 토큰(chat 템플릿 포함)이지 인자가 아니다.
# output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B/lite_cold_q38fn032-nv4-safe-tp2.json (date 20261029-112723) · 실측 토큰(chat 템플릿 포함 · total ÷ completed · 인자 아님): 입력 512/1 = 512 · 출력 128/1 = 128 · spec_decode_acceptance_length 2.11667(이 레그 JSON)
vllm bench serve --backend openai --model qwen3.8-flash-next --tokenizer /app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 --trust-remote-code --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 --ignore-eos --num-prompts 1 --max-concurrency 1 --request-rate inf
# output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B/lite_warm_q38fn032-nv4-safe-tp2.json (date 20261029-112744) · 실측 토큰(chat 템플릿 포함 · total ÷ completed · 인자 아님): 입력 1536/3 = 512 · 출력 384/3 = 128 · spec_decode_acceptance_length 2.28402(이 레그 JSON)
vllm bench serve --backend openai --model qwen3.8-flash-next --tokenizer /app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 --trust-remote-code --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 --ignore-eos --num-prompts 3 --max-concurrency 1 --request-rate inf
```

**슬롯 → 재구성 예시 컨텍스트 매핑**(예시 위치 = `output/<토폴로지>/` · 원천 = 실린 Dockerfile 의 COPY · compose 설정; 실제 실행 경로는 별도 기록 확인) — 재구성 예시 통로 `parent-source-core/context/`(실제 원본·후속층의 빌드 경로 증명 아님 · 별도 실행 기록 확인)

| zip 경로 | 빌드 컨텍스트 자리 | 근거 |
|---|---|---|
| `artifacts/triplet/.env.q38fn032-nv4-safe-tp2` | `envs/.env.q38fn032-nv4-safe-tp2` | compose env_file envs/.env.${CONFIG_FILE}(셀 env) |
| `artifacts/triplet/q38fn032-nv4-safe-tp2.sh` | `configs/q38fn032-nv4-safe-tp2.sh` | compose volumes ./configs:/app/configs |
| `artifacts/triplet/q38fn032-nv4-safe-tp2.yaml` | `configs/q38fn032-nv4-safe-tp2.yaml` | compose volumes ./configs:/app/configs |
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

**기동 성공 판정(관측 · 발행 자격)**: health 200 = true · 추론 1회 = true · 근거 `output/multi/benchlog/serve_proof_q38fn032-nv4-safe-tp2.json` — 시점 범위: 작성 2026-10-29T11:42:14Z(output/multi/benchlog/serve_proof_q38fn032-nv4-safe-tp2.json 파일 mtime(파일시스템 관측 — 문서에 시각 칸이 없다 · 기록 필드 아님)) · 측정 창 대비 작성 2026-10-29T11:42:14Z > 측정 끝 2026-10-29T11:41:26Z — 측정 뒤 작성된 스모크 산출물(측정 시점·같은 기동 여부는 시각만으로 미검증)
<!-- /FACT:reproduce -->

<!-- FACT:event_timeline -->
**블랙박스 이벤트 118행**(원장에서 이 셀 label·config 에 맞는 행만 · 시각순 · 기계 발췌)

> **캠페인 경계** 2026-10-29T09:57:19Z(`camp-26102919-codex-multi4-v032` · evidence.event_campaign_boundary): campaigns/camp-26102919-codex-multi4-v032/campaign.yaml declared_utc 2026-10-29T09:57:19Z · 이 발행이 묶인 측정을 서빙한 budget_declare 창 시작 2026-10-29T10:53:09Z → 경계 = 앞선 값 2026-10-29T09:57:19Z(이전 행 = 같은 셀 이름 · 앞선 캠페인)

> **원장 관측 범위**(노드별 첫 행 → 마지막 행 · evidence.event_ledger_spans): main = 2026-07-31T03:27:43Z → 2026-10-29T16:02:11Z(`docs/logs/main/events/2026-07.jsonl` · `docs/logs/main/events/2026-08.jsonl` · `docs/logs/main/events/2026-09.jsonl` · `docs/logs/main/events/2026-10.jsonl` · `docs/logs/main/events/thermal.jsonl` · `docs/logs/main/events/watchdog.jsonl`) · sub = 2026-06-13T22:25:01Z → 2026-10-29T14:09:01Z(`sync_staging/sub_docs/logs/sub/events/2026-06.jsonl` · `sync_staging/sub_docs/logs/sub/events/2026-07.jsonl` · `sync_staging/sub_docs/logs/sub/events/2026-08.jsonl` · `sync_staging/sub_docs/logs/sub/events/2026-09.jsonl` · `sync_staging/sub_docs/logs/sub/events/2026-10.jsonl` · `sync_staging/sub_docs/logs/sub/events/thermal.jsonl` · `sync_staging/sub_docs/logs/sub/events/watchdog.jsonl` · 회수 미러가 가린 옛 로컬 사본 `docs/logs/sub/events/2026-09.jsonl`)

| # | UTC | 노드 | kind | label | 내용 | 출처 |
|---|---|---|---|---|---|---|
| 1 | 2026-10-29T10:53:09Z | main | `budget_declare` | `smoke-q38fn032-nv4-safe-tp2` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=22206 · weights_mib=63266 · kv_mib=14336 · overhead_mib=24800 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 기동 창 1/7: budget_renew 10회 · 닫힘 budget_clear 2026-10-29T11:42:50Z(선언 후 49m41s) · measured_utc 2026-10-29T11:41:26Z 가 이 창 안(이 측정의 기동) | docs/logs/main/events/2026-10.jsonl:768 |
| 2 | 2026-10-29T10:53:10Z | main | `budget_honored` | `smoke-q38fn032-nv4-safe-tp2` | budget_honored · floor_mib=22206 · arm_ceiling_mib=19134 · remaining_s=10799 · 창 시작 2026-10-29T10:53:10Z 후 0m00s | docs/logs/main/events/watchdog.jsonl:843 |
| 3 | 2026-10-29T10:53:12Z | sub | `budget_declare` | `smoke-q38fn032-nv4-safe-tp2` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=22206 · weights_mib=63266 · kv_mib=14336 · overhead_mib=24800 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 기동 창 1/6: budget_renew 1회 · 닫힘 budget_clear 2026-10-29T11:42:50Z(선언 후 49m38s) · measured_utc 2026-10-29T11:41:26Z 는 이 창 밖(다른 기동 · 측정 전) | sync_staging/sub_docs/logs/sub/events/2026-10.jsonl:709 |
| 4 | 2026-10-29T10:53:13Z | sub | `budget_honored` | `smoke-q38fn032-nv4-safe-tp2` | budget_honored · floor_mib=22206 · arm_ceiling_mib=19134 · remaining_s=10799 · 창 시작 2026-10-29T10:53:13Z 후 0m00s | sync_staging/sub_docs/logs/sub/events/watchdog.jsonl:659 |
| 5 | 2026-10-29T10:54:58Z | cluster | `engine_log_window_start` | — | 캡처된 엔진 로그의 첫 시각(꼬리 캡처일 수 있다 — 엔진 시작 시각이 아닐 수 있다) · 선언 2026-10-29T10:53:09Z 후 1m49s · 시계: 컨테이너 시각을 UTC 로 읽어 측정 창 [2026-10-29T10:53:09Z, 2026-10-29T11:42:50Z] 안에 드는 것을 확인 | output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B/lite_engine_q38fn032-nv4-safe-tp2.log:221 |
| 6 | 2026-10-29T10:55:02Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=80341 · rate_mib_s=26877 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → smoke-q38fn032-nv4-safe-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T10:53:10Z 후 1m52s) | docs/logs/main/events/watchdog.jsonl:844 |
| 7 | 2026-10-29T10:55:03Z | sub | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=79045 · rate_mib_s=28461 · max_rate_mib_s=17801 · legacy_rule_would_trip=true · 라벨 없음 → smoke-q38fn032-nv4-safe-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T10:53:13Z 후 1m50s) | sync_staging/sub_docs/logs/sub/events/watchdog.jsonl:660 |
| 8 | 2026-10-29T10:55:04Z | cluster | `engine_prefetch_disabled` | — | 가중치 auto-prefetch 꺼짐 줄 ×4 — filesystem CIFS(vLLM 로그 문구) | output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B/lite_engine_q38fn032-nv4-safe-tp2.log:478 |
| 9 | 2026-10-29T11:09:34Z | cluster | `engine_weights_loaded` | — | 가중치 적재 완료 줄 ×4(rank·단계별) · 최장 476.70s · 이 행 = 마지막 줄 | output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B/lite_engine_q38fn032-nv4-safe-tp2.log:537 |
| 10 | 2026-10-29T11:09:35Z | cluster | `engine_model_loaded` | — | 모델 적재 완료 줄 ×2(rank 별) · 마지막 줄 63.04 GiB · 875.0s | output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B/lite_engine_q38fn032-nv4-safe-tp2.log:540 |
| 11 | 2026-10-29T11:10:37Z | cluster | `engine_shm_broadcast_wait` | — | shm_broadcast 대기 경고(60s 단위) ×13 · 2026-10-29T11:10:37Z–2026-10-29T11:23:24Z(12m47s) — vLLM 문구상 프로세스 hang 또는 긴 작업(컴파일·가중치/KV 양자화) 중 출현 | output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B/lite_engine_q38fn032-nv4-safe-tp2.log:547 |
| 12 | 2026-10-29T11:22:22Z | cluster | `engine_kv_cache_sized` | — | GPU KV cache 1,595,769 tokens · 요청당 262,144 tokens 기준 최대 동시성 6.09x | output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B/lite_engine_q38fn032-nv4-safe-tp2.log:565 |
| 13 | 2026-10-29T11:24:04Z | cluster | `engine_init_done` | — | 엔진 초기화(profile·KV 할당·warmup) 867.91s | output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B/lite_engine_q38fn032-nv4-safe-tp2.log:690 |
| 14 | 2026-10-29T11:24:48Z | cluster | `server_starting` | — | API 서버 기동 — 이 측정 창 선언 2026-10-29T10:53:09Z 후 31m39s · 캡처 로그 첫 시각 2026-10-29T10:54:58Z 후 29m50s | output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B/lite_engine_q38fn032-nv4-safe-tp2.log:700 |
| 15 | 2026-10-29T11:24:58Z | main | `budget_renew` | `smoke-q38fn032-nv4-safe-tp2` | budget_renew · floor_mib=22206 · ttl_s=10800 · remaining_before_s=8891 · 창 시작 2026-10-29T10:53:09Z 후 31m49s | docs/logs/main/events/2026-10.jsonl:769 |
| 16 | 2026-10-29T11:24:59Z | sub | `budget_renew` | `smoke-q38fn032-nv4-safe-tp2` | budget_renew · floor_mib=22206 · ttl_s=10800 · remaining_before_s=8893 · 창 시작 2026-10-29T10:53:12Z 후 31m47s | sync_staging/sub_docs/logs/sub/events/2026-10.jsonl:710 |
| 17 | 2026-10-29T11:27:46Z | main | `budget_renew` | `smoke-q38fn032-nv4-safe-tp2` | budget_renew · floor_mib=22206 · ttl_s=7200 · remaining_before_s=10632 · 창 시작 2026-10-29T10:53:09Z 후 34m37s | docs/logs/main/events/2026-10.jsonl:770 |
| 18 | 2026-10-29T11:30:01Z | main | `budget_renew` | `smoke-q38fn032-nv4-safe-tp2` | budget_renew · floor_mib=22206 · ttl_s=7200 · remaining_before_s=7065 · 창 시작 2026-10-29T10:53:09Z 후 36m52s | docs/logs/main/events/2026-10.jsonl:771 |
| 19 | 2026-10-29T11:32:06Z | main | `budget_renew` | `smoke-q38fn032-nv4-safe-tp2` | budget_renew · floor_mib=22206 · ttl_s=7200 · remaining_before_s=7075 · 창 시작 2026-10-29T10:53:09Z 후 38m57s | docs/logs/main/events/2026-10.jsonl:772 |
| 20 | 2026-10-29T11:34:18Z | main | `budget_renew` | `smoke-q38fn032-nv4-safe-tp2` | budget_renew · floor_mib=22206 · ttl_s=7200 · remaining_before_s=7068 · 창 시작 2026-10-29T10:53:09Z 후 41m09s | docs/logs/main/events/2026-10.jsonl:773 |
| 21 | 2026-10-29T11:35:42Z | main | `budget_renew` | `smoke-q38fn032-nv4-safe-tp2` | budget_renew · floor_mib=22206 · ttl_s=7200 · remaining_before_s=7116 · 창 시작 2026-10-29T10:53:09Z 후 42m33s | docs/logs/main/events/2026-10.jsonl:774 |
| 22 | 2026-10-29T11:37:02Z | main | `budget_renew` | `smoke-q38fn032-nv4-safe-tp2` | budget_renew · floor_mib=22206 · ttl_s=7200 · remaining_before_s=7120 · 창 시작 2026-10-29T10:53:09Z 후 43m53s | docs/logs/main/events/2026-10.jsonl:775 |
| 23 | 2026-10-29T11:38:28Z | main | `budget_renew` | `smoke-q38fn032-nv4-safe-tp2` | budget_renew · floor_mib=22206 · ttl_s=7200 · remaining_before_s=7114 · 창 시작 2026-10-29T10:53:09Z 후 45m19s | docs/logs/main/events/2026-10.jsonl:776 |
| 24 | 2026-10-29T11:39:31Z | main | `budget_renew` | `smoke-q38fn032-nv4-safe-tp2` | budget_renew · floor_mib=22206 · ttl_s=7200 · remaining_before_s=7137 · 창 시작 2026-10-29T10:53:09Z 후 46m22s | docs/logs/main/events/2026-10.jsonl:777 |
| 25 | 2026-10-29T11:40:26Z | main | `budget_renew` | `smoke-q38fn032-nv4-safe-tp2` | budget_renew · floor_mib=22206 · ttl_s=7200 · remaining_before_s=7145 · 창 시작 2026-10-29T10:53:09Z 후 47m17s | docs/logs/main/events/2026-10.jsonl:778 |
| 26 | 2026-10-29T11:42:50Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-q38fn032-nv4-safe-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T10:53:09Z 후 49m41s) | docs/logs/main/events/2026-10.jsonl:779 |
| 27 | 2026-10-29T11:42:50Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-q38fn032-nv4-safe-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T10:53:10Z 후 49m40s) | docs/logs/main/events/watchdog.jsonl:845 |
| 28 | 2026-10-29T11:42:50Z | sub | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-q38fn032-nv4-safe-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T10:53:12Z 후 49m38s) | sync_staging/sub_docs/logs/sub/events/2026-10.jsonl:711 |
| 29 | 2026-10-29T11:42:51Z | sub | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-q38fn032-nv4-safe-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T10:53:13Z 후 49m38s) | sync_staging/sub_docs/logs/sub/events/watchdog.jsonl:661 |
| 30 | 2026-10-29T12:17:18Z | main | `budget_declare` | `smoke-q38fn032-nv4-safe-tp2` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=22206 · weights_mib=63266 · kv_mib=14336 · overhead_mib=24800 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 기동 창 2/7: budget_renew 10회 · 닫힘 budget_clear 2026-10-29T12:55:14Z(선언 후 37m56s) · measured_utc 2026-10-29T11:41:26Z 는 이 창 밖(다른 기동 · 측정 뒤 — 이 측정과 무관할 수 있다) | docs/logs/main/events/2026-10.jsonl:781 |
| 31 | 2026-10-29T12:17:19Z | main | `budget_honored` | `smoke-q38fn032-nv4-safe-tp2` | budget_honored · floor_mib=22206 · arm_ceiling_mib=19134 · remaining_s=10799 · 창 시작 2026-10-29T12:17:19Z 후 0m00s | docs/logs/main/events/watchdog.jsonl:846 |
| 32 | 2026-10-29T12:17:20Z | sub | `budget_declare` | `smoke-q38fn032-nv4-safe-tp2` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=22206 · weights_mib=63266 · kv_mib=14336 · overhead_mib=24800 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 기동 창 2/6: budget_renew 1회 · 닫힘 budget_clear 2026-10-29T12:55:14Z(선언 후 37m54s) · measured_utc 2026-10-29T11:41:26Z 는 이 창 밖(다른 기동 · 측정 뒤 — 이 측정과 무관할 수 있다) | sync_staging/sub_docs/logs/sub/events/2026-10.jsonl:713 |
| 33 | 2026-10-29T12:17:21Z | sub | `budget_honored` | `smoke-q38fn032-nv4-safe-tp2` | budget_honored · floor_mib=22206 · arm_ceiling_mib=19134 · remaining_s=10799 · 창 시작 2026-10-29T12:17:21Z 후 0m00s | sync_staging/sub_docs/logs/sub/events/watchdog.jsonl:662 |
| 34 | 2026-10-29T12:18:33Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=75755 · rate_mib_s=28827 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → smoke-q38fn032-nv4-safe-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T12:17:19Z 후 1m14s) | docs/logs/main/events/watchdog.jsonl:847 |
| 35 | 2026-10-29T12:18:33Z | sub | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=92514 · rate_mib_s=21212 · max_rate_mib_s=17801 · legacy_rule_would_trip=true · 라벨 없음 → smoke-q38fn032-nv4-safe-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T12:17:21Z 후 1m12s) | sync_staging/sub_docs/logs/sub/events/watchdog.jsonl:663 |
| 36 | 2026-10-29T12:36:16Z | main | `budget_renew` | `smoke-q38fn032-nv4-safe-tp2` | budget_renew · floor_mib=22206 · ttl_s=10800 · remaining_before_s=9662 · 창 시작 2026-10-29T12:17:18Z 후 18m58s | docs/logs/main/events/2026-10.jsonl:782 |
| 37 | 2026-10-29T12:36:17Z | sub | `budget_renew` | `smoke-q38fn032-nv4-safe-tp2` | budget_renew · floor_mib=22206 · ttl_s=10800 · remaining_before_s=9663 · 창 시작 2026-10-29T12:17:20Z 후 18m57s | sync_staging/sub_docs/logs/sub/events/2026-10.jsonl:714 |
| 38 | 2026-10-29T12:37:51Z | main | `budget_renew` | `smoke-q38fn032-nv4-safe-tp2` | budget_renew · floor_mib=22206 · ttl_s=7200 · remaining_before_s=10705 · 창 시작 2026-10-29T12:17:18Z 후 20m33s | docs/logs/main/events/2026-10.jsonl:783 |
| 39 | 2026-10-29T12:39:59Z | main | `budget_renew` | `smoke-q38fn032-nv4-safe-tp2` | budget_renew · floor_mib=22206 · ttl_s=7200 · remaining_before_s=7072 · 창 시작 2026-10-29T12:17:18Z 후 22m41s | docs/logs/main/events/2026-10.jsonl:784 |
| 40 | 2026-10-29T12:42:05Z | main | `budget_renew` | `smoke-q38fn032-nv4-safe-tp2` | budget_renew · floor_mib=22206 · ttl_s=7200 · remaining_before_s=7074 · 창 시작 2026-10-29T12:17:18Z 후 24m47s | docs/logs/main/events/2026-10.jsonl:785 |
| 41 | 2026-10-29T12:44:10Z | main | `budget_renew` | `smoke-q38fn032-nv4-safe-tp2` | budget_renew · floor_mib=22206 · ttl_s=7200 · remaining_before_s=7075 · 창 시작 2026-10-29T12:17:18Z 후 26m52s | docs/logs/main/events/2026-10.jsonl:786 |
| 42 | 2026-10-29T12:45:34Z | main | `budget_renew` | `smoke-q38fn032-nv4-safe-tp2` | budget_renew · floor_mib=22206 · ttl_s=7200 · remaining_before_s=7116 · 창 시작 2026-10-29T12:17:18Z 후 28m16s | docs/logs/main/events/2026-10.jsonl:787 |
| 43 | 2026-10-29T12:46:59Z | main | `budget_renew` | `smoke-q38fn032-nv4-safe-tp2` | budget_renew · floor_mib=22206 · ttl_s=7200 · remaining_before_s=7115 · 창 시작 2026-10-29T12:17:18Z 후 29m41s | docs/logs/main/events/2026-10.jsonl:788 |
| 44 | 2026-10-29T12:48:29Z | main | `budget_renew` | `smoke-q38fn032-nv4-safe-tp2` | budget_renew · floor_mib=22206 · ttl_s=7200 · remaining_before_s=7110 · 창 시작 2026-10-29T12:17:18Z 후 31m11s | docs/logs/main/events/2026-10.jsonl:789 |
| 45 | 2026-10-29T12:49:29Z | main | `budget_renew` | `smoke-q38fn032-nv4-safe-tp2` | budget_renew · floor_mib=22206 · ttl_s=7200 · remaining_before_s=7140 · 창 시작 2026-10-29T12:17:18Z 후 32m11s | docs/logs/main/events/2026-10.jsonl:790 |
| 46 | 2026-10-29T12:50:29Z | main | `budget_renew` | `smoke-q38fn032-nv4-safe-tp2` | budget_renew · floor_mib=22206 · ttl_s=7200 · remaining_before_s=7140 · 창 시작 2026-10-29T12:17:18Z 후 33m11s | docs/logs/main/events/2026-10.jsonl:791 |
| 47 | 2026-10-29T12:51:30Z | 미관측 | `measurement` | `measurement (budget window recorded)` | 같은 셀의 다른 측정(측정 노드 축 cluster) · 시각 = 측정 조립(sweep generated_utc = 인증서 measured_utc · 시작 시각 미기록) · 이 발행의 측정 2026-10-29T11:41:26Z 뒤 — 이 측정과 무관할 수 있다 · 출력 평면에 보존된 같은 셀의 다른 측정 · 원장: 이 셀 선언 창 안: smoke-q38fn032-nv4-safe-tp2 2026-10-29T12:17:18Z~2026-10-29T12:55:14Z (docs/logs/main/events/2026-10.jsonl:781) · smoke-q38fn032-nv4-safe-tp2 2026-10-29T12:17:19Z~2026-10-29T12:55:14Z (docs/logs/main/events/watchdog.jsonl:846) · smoke-q38fn032-nv4-safe-tp2 2026-10-29T12:17:20Z~2026-10-29T12:55:14Z (sync_staging/sub_docs/logs/sub/events/2026-10.jsonl:713) · smoke-q38fn032-nv4-safe-tp2 2026-10-29T12:17:21Z~2026-10-29T12:55:15Z (sync_staging/sub_docs/logs/sub/events/watchdog.jsonl:662) · 직전 예산 행 budget_renew(smoke-q38fn032-nv4-safe-tp2) 2026-10-29T12:50:29Z(docs/logs/main/events/2026-10.jsonl:791) · 직후 예산 행 budget_clear 2026-10-29T12:55:14Z(docs/logs/main/events/2026-10.jsonl:792) | output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029C/sweep_index.json:4 |
| 48 | 2026-10-29T12:55:14Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-q38fn032-nv4-safe-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T12:17:18Z 후 37m56s) | docs/logs/main/events/2026-10.jsonl:792 |
| 49 | 2026-10-29T12:55:14Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-q38fn032-nv4-safe-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T12:17:19Z 후 37m55s) | docs/logs/main/events/watchdog.jsonl:848 |
| 50 | 2026-10-29T12:55:14Z | sub | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-q38fn032-nv4-safe-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T12:17:20Z 후 37m54s) | sync_staging/sub_docs/logs/sub/events/2026-10.jsonl:715 |
| 51 | 2026-10-29T12:55:15Z | sub | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-q38fn032-nv4-safe-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T12:17:21Z 후 37m54s) | sync_staging/sub_docs/logs/sub/events/watchdog.jsonl:664 |
| 52 | 2026-10-29T12:57:24Z | main | `budget_declare` | `smoke-q38fn032-nv4-safe-tp2` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=22206 · weights_mib=63266 · kv_mib=14336 · overhead_mib=24800 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 기동 창 3/7: budget_renew 1회 · 닫힘 budget_clear 2026-10-29T13:09:33Z(선언 후 12m09s) · measured_utc 2026-10-29T11:41:26Z 는 이 창 밖(다른 기동 · 측정 뒤 — 이 측정과 무관할 수 있다) | docs/logs/main/events/2026-10.jsonl:794 |
| 53 | 2026-10-29T12:57:24Z | main | `budget_honored` | `smoke-q38fn032-nv4-safe-tp2` | budget_honored · floor_mib=22206 · arm_ceiling_mib=19134 · remaining_s=10800 · 창 시작 2026-10-29T12:57:24Z 후 0m00s | docs/logs/main/events/watchdog.jsonl:849 |
| 54 | 2026-10-29T12:57:26Z | sub | `budget_declare` | `smoke-q38fn032-nv4-safe-tp2` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=22206 · weights_mib=63266 · kv_mib=14336 · overhead_mib=24800 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 기동 창 3/6: budget_renew 1회 · 닫힘 budget_clear 2026-10-29T13:09:33Z(선언 후 12m07s) · measured_utc 2026-10-29T11:41:26Z 는 이 창 밖(다른 기동 · 측정 뒤 — 이 측정과 무관할 수 있다) | sync_staging/sub_docs/logs/sub/events/2026-10.jsonl:717 |
| 55 | 2026-10-29T12:57:27Z | sub | `budget_honored` | `smoke-q38fn032-nv4-safe-tp2` | budget_honored · floor_mib=22206 · arm_ceiling_mib=19134 · remaining_s=10799 · 창 시작 2026-10-29T12:57:27Z 후 0m00s | sync_staging/sub_docs/logs/sub/events/watchdog.jsonl:665 |
| 56 | 2026-10-29T12:58:51Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=77619 · rate_mib_s=25982 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → smoke-q38fn032-nv4-safe-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T12:57:24Z 후 1m27s) | docs/logs/main/events/watchdog.jsonl:850 |
| 57 | 2026-10-29T12:58:52Z | sub | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=76814 · rate_mib_s=24532 · max_rate_mib_s=17801 · legacy_rule_would_trip=true · 라벨 없음 → smoke-q38fn032-nv4-safe-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T12:57:27Z 후 1m25s) | sync_staging/sub_docs/logs/sub/events/watchdog.jsonl:666 |
| 58 | 2026-10-29T13:06:44Z | main | `budget_renew` | `smoke-q38fn032-nv4-safe-tp2` | budget_renew · floor_mib=22206 · ttl_s=10800 · remaining_before_s=10240 · 창 시작 2026-10-29T12:57:24Z 후 9m20s | docs/logs/main/events/2026-10.jsonl:795 |
| 59 | 2026-10-29T13:06:45Z | sub | `budget_renew` | `smoke-q38fn032-nv4-safe-tp2` | budget_renew · floor_mib=22206 · ttl_s=10800 · remaining_before_s=10241 · 창 시작 2026-10-29T12:57:26Z 후 9m19s | sync_staging/sub_docs/logs/sub/events/2026-10.jsonl:718 |
| 60 | 2026-10-29T13:09:33Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-q38fn032-nv4-safe-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T12:57:24Z 후 12m09s) | docs/logs/main/events/2026-10.jsonl:796 |
| 61 | 2026-10-29T13:09:33Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-q38fn032-nv4-safe-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T12:57:24Z 후 12m09s) | docs/logs/main/events/watchdog.jsonl:851 |
| 62 | 2026-10-29T13:09:33Z | sub | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-q38fn032-nv4-safe-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T12:57:26Z 후 12m07s) | sync_staging/sub_docs/logs/sub/events/2026-10.jsonl:719 |
| 63 | 2026-10-29T13:09:34Z | sub | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-q38fn032-nv4-safe-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T12:57:27Z 후 12m07s) | sync_staging/sub_docs/logs/sub/events/watchdog.jsonl:667 |
| 64 | 2026-10-29T13:11:58Z | main | `budget_declare` | `smoke-q38fn032-nv4-safe-tp2` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=22206 · weights_mib=63266 · kv_mib=14336 · overhead_mib=24800 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 기동 창 4/7: budget_renew 0회 · 닫힘 budget_clear 2026-10-29T13:30:21Z(선언 후 18m23s) · measured_utc 2026-10-29T11:41:26Z 는 이 창 밖(다른 기동 · 측정 뒤 — 이 측정과 무관할 수 있다) | docs/logs/main/events/2026-10.jsonl:798 |
| 65 | 2026-10-29T13:11:59Z | main | `budget_honored` | `smoke-q38fn032-nv4-safe-tp2` | budget_honored · floor_mib=22206 · arm_ceiling_mib=19134 · remaining_s=10799 · 창 시작 2026-10-29T13:11:59Z 후 0m00s | docs/logs/main/events/watchdog.jsonl:852 |
| 66 | 2026-10-29T13:12:00Z | sub | `budget_declare` | `smoke-q38fn032-nv4-safe-tp2` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=22206 · weights_mib=63266 · kv_mib=14336 · overhead_mib=24800 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 기동 창 4/6: budget_renew 0회 · 닫힘 budget_clear 2026-10-29T13:30:21Z(선언 후 18m21s) · measured_utc 2026-10-29T11:41:26Z 는 이 창 밖(다른 기동 · 측정 뒤 — 이 측정과 무관할 수 있다) | sync_staging/sub_docs/logs/sub/events/2026-10.jsonl:721 |
| 67 | 2026-10-29T13:12:01Z | sub | `budget_honored` | `smoke-q38fn032-nv4-safe-tp2` | budget_honored · floor_mib=22206 · arm_ceiling_mib=19134 · remaining_s=10799 · 창 시작 2026-10-29T13:12:01Z 후 0m00s | sync_staging/sub_docs/logs/sub/events/watchdog.jsonl:668 |
| 68 | 2026-10-29T13:13:24Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=79437 · rate_mib_s=28804 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → smoke-q38fn032-nv4-safe-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T13:11:59Z 후 1m25s) | docs/logs/main/events/watchdog.jsonl:853 |
| 69 | 2026-10-29T13:13:25Z | sub | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=78658 · rate_mib_s=24433 · max_rate_mib_s=17801 · legacy_rule_would_trip=true · 라벨 없음 → smoke-q38fn032-nv4-safe-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T13:12:01Z 후 1m24s) | sync_staging/sub_docs/logs/sub/events/watchdog.jsonl:669 |
| 70 | 2026-10-29T13:28:24Z | main | `watchdog_trip` | — | watchdog_trip · mem_avail_mib=9512 · rule=absolute · threshold_mib=10240 · action=docker_kill · target=c783338a7a1a · 라벨 없음 → smoke-q38fn032-nv4-safe-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T13:11:58Z 후 16m26s) | docs/logs/main/events/2026-10.jsonl:800 |
| 71 | 2026-10-29T13:28:24Z | sub | `watchdog_trip` | — | watchdog_trip · mem_avail_mib=9224 · rule=absolute · threshold_mib=10240 · action=docker_kill · target=a4b00eb818ba · 라벨 없음 → smoke-q38fn032-nv4-safe-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T13:12:00Z 후 16m24s) | sync_staging/sub_docs/logs/sub/events/2026-10.jsonl:723 |
| 72 | 2026-10-29T13:28:28Z | main | `watchdog_kill_ack` | — | watchdog_kill_ack · target=c783338a7a1a · 라벨 없음 → smoke-q38fn032-nv4-safe-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T13:11:58Z 후 16m30s) | docs/logs/main/events/2026-10.jsonl:801 |
| 73 | 2026-10-29T13:28:28Z | sub | `watchdog_kill_ack` | — | watchdog_kill_ack · target=a4b00eb818ba · 라벨 없음 → smoke-q38fn032-nv4-safe-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T13:12:00Z 후 16m28s) | sync_staging/sub_docs/logs/sub/events/2026-10.jsonl:724 |
| 74 | 2026-10-29T13:30:21Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-q38fn032-nv4-safe-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T13:11:58Z 후 18m23s) | docs/logs/main/events/2026-10.jsonl:799 |
| 75 | 2026-10-29T13:30:21Z | sub | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-q38fn032-nv4-safe-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T13:12:00Z 후 18m21s) | sync_staging/sub_docs/logs/sub/events/2026-10.jsonl:722 |
| 76 | 2026-10-29T13:30:22Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-q38fn032-nv4-safe-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T13:11:59Z 후 18m23s) | docs/logs/main/events/watchdog.jsonl:854 |
| 77 | 2026-10-29T13:30:22Z | sub | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-q38fn032-nv4-safe-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T13:12:01Z 후 18m21s) | sync_staging/sub_docs/logs/sub/events/watchdog.jsonl:670 |
| 78 | 2026-10-29T13:33:11Z | main | `budget_declare` | `smoke-q38fn032-nv4-safe-tp2` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=32446 · weights_mib=63266 · kv_mib=4096 · overhead_mib=24800 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 기동 창 5/7: budget_renew 0회 · 닫힘 budget_clear 2026-10-29T13:49:10Z(선언 후 15m59s) · measured_utc 2026-10-29T11:41:26Z 는 이 창 밖(다른 기동 · 측정 뒤 — 이 측정과 무관할 수 있다) | docs/logs/main/events/2026-10.jsonl:803 |
| 79 | 2026-10-29T13:33:12Z | main | `budget_honored` | `smoke-q38fn032-nv4-safe-tp2` | budget_honored · floor_mib=32446 · arm_ceiling_mib=29374 · remaining_s=10799 · 창 시작 2026-10-29T13:33:12Z 후 0m00s | docs/logs/main/events/watchdog.jsonl:855 |
| 80 | 2026-10-29T13:33:14Z | sub | `budget_declare` | `smoke-q38fn032-nv4-safe-tp2` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=32446 · weights_mib=63266 · kv_mib=4096 · overhead_mib=24800 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 기동 창 5/6: budget_renew 0회 · 닫힘 budget_clear 2026-10-29T13:49:10Z(선언 후 15m56s) · measured_utc 2026-10-29T11:41:26Z 는 이 창 밖(다른 기동 · 측정 뒤 — 이 측정과 무관할 수 있다) | sync_staging/sub_docs/logs/sub/events/2026-10.jsonl:726 |
| 81 | 2026-10-29T13:33:14Z | sub | `budget_honored` | `smoke-q38fn032-nv4-safe-tp2` | budget_honored · floor_mib=32446 · arm_ceiling_mib=29374 · remaining_s=10800 · 창 시작 2026-10-29T13:33:14Z 후 0m00s | sync_staging/sub_docs/logs/sub/events/watchdog.jsonl:671 |
| 82 | 2026-10-29T13:34:36Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=64915 · rate_mib_s=23769 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → smoke-q38fn032-nv4-safe-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T13:33:12Z 후 1m24s) | docs/logs/main/events/watchdog.jsonl:856 |
| 83 | 2026-10-29T13:34:36Z | sub | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=76532 · rate_mib_s=23654 · max_rate_mib_s=17801 · legacy_rule_would_trip=true · 라벨 없음 → smoke-q38fn032-nv4-safe-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T13:33:14Z 후 1m22s) | sync_staging/sub_docs/logs/sub/events/watchdog.jsonl:672 |
| 84 | 2026-10-29T13:47:00Z | main | `watchdog_trip` | — | watchdog_trip · mem_avail_mib=9306 · rule=absolute · threshold_mib=10240 · action=docker_kill · target=e338a3f684c6 · 라벨 없음 → smoke-q38fn032-nv4-safe-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T13:33:11Z 후 13m49s) · 같은 사건의 다른 스트림 사본 docs/logs/main/events/watchdog.jsonl:857(한 번으로 싣는다) | docs/logs/main/events/2026-10.jsonl:805 |
| 85 | 2026-10-29T13:47:00Z | sub | `watchdog_trip` | — | watchdog_trip · mem_avail_mib=9268 · rule=absolute · threshold_mib=10240 · action=docker_kill · target=d95c23de7d7f · 라벨 없음 → smoke-q38fn032-nv4-safe-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T13:33:14Z 후 13m46s) | sync_staging/sub_docs/logs/sub/events/2026-10.jsonl:729 |
| 86 | 2026-10-29T13:47:01Z | sub | `watchdog_trip` | — | watchdog_trip · mem_avail_mib=9147 · rate_mib_s=3950 · rule=eta · action=docker_kill · 라벨 없음 → smoke-q38fn032-nv4-safe-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T13:33:14Z 후 13m47s) | sync_staging/sub_docs/logs/sub/events/watchdog.jsonl:673 |
| 87 | 2026-10-29T13:47:04Z | main | `watchdog_kill_ack` | — | watchdog_kill_ack · target=e338a3f684c6 · 라벨 없음 → smoke-q38fn032-nv4-safe-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T13:33:11Z 후 13m53s) · 같은 사건의 다른 스트림 사본 docs/logs/main/events/watchdog.jsonl:858(한 번으로 싣는다) | docs/logs/main/events/2026-10.jsonl:804 |
| 88 | 2026-10-29T13:47:04Z | main | `watchdog_kill_ack` | — | watchdog_kill_ack · target=e338a3f684c6 · 라벨 없음 → smoke-q38fn032-nv4-safe-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T13:33:11Z 후 13m53s) | docs/logs/main/events/2026-10.jsonl:806 |
| 89 | 2026-10-29T13:47:04Z | sub | `watchdog_kill_ack` | — | watchdog_kill_ack · target=d95c23de7d7f · 라벨 없음 → smoke-q38fn032-nv4-safe-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T13:33:14Z 후 13m50s) · 같은 사건의 다른 스트림 사본 sync_staging/sub_docs/logs/sub/events/watchdog.jsonl:674(한 번으로 싣는다) | sync_staging/sub_docs/logs/sub/events/2026-10.jsonl:728 |
| 90 | 2026-10-29T13:47:04Z | sub | `watchdog_kill_ack` | — | watchdog_kill_ack · target=d95c23de7d7f · 라벨 없음 → smoke-q38fn032-nv4-safe-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T13:33:14Z 후 13m50s) | sync_staging/sub_docs/logs/sub/events/2026-10.jsonl:730 |
| 91 | 2026-10-29T13:49:10Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-q38fn032-nv4-safe-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T13:33:11Z 후 15m59s) | docs/logs/main/events/2026-10.jsonl:807 |
| 92 | 2026-10-29T13:49:10Z | sub | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-q38fn032-nv4-safe-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T13:33:14Z 후 15m56s) | sync_staging/sub_docs/logs/sub/events/2026-10.jsonl:727 |
| 93 | 2026-10-29T13:49:11Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-q38fn032-nv4-safe-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T13:33:12Z 후 15m59s) | docs/logs/main/events/watchdog.jsonl:859 |
| 94 | 2026-10-29T13:49:12Z | sub | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-q38fn032-nv4-safe-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T13:33:14Z 후 15m58s) | sync_staging/sub_docs/logs/sub/events/watchdog.jsonl:675 |
| 95 | 2026-10-29T13:55:10Z | main | `budget_declare` | `smoke-q38fn032-nv4-safe-tp2` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=32446 · weights_mib=63266 · kv_mib=4096 · overhead_mib=24800 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 기동 창 6/7: budget_renew 1회 · 닫힘 budget_clear 2026-10-29T14:18:48Z(선언 후 23m38s) · measured_utc 2026-10-29T11:41:26Z 는 이 창 밖(다른 기동 · 측정 뒤 — 이 측정과 무관할 수 있다) | docs/logs/main/events/2026-10.jsonl:809 |
| 96 | 2026-10-29T13:55:10Z | main | `budget_honored` | `smoke-q38fn032-nv4-safe-tp2` | budget_honored · floor_mib=32446 · arm_ceiling_mib=29374 · remaining_s=10800 · 창 시작 2026-10-29T13:55:10Z 후 0m00s | docs/logs/main/events/watchdog.jsonl:860 |
| 97 | 2026-10-29T13:55:12Z | sub | `budget_declare` | `smoke-q38fn032-nv4-safe-tp2` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=32446 · weights_mib=63266 · kv_mib=4096 · overhead_mib=24800 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 기동 창 6/6: budget_renew 0회 · 닫힘 미관측(창이 열린 채 원장이 끝난다) · measured_utc 2026-10-29T11:41:26Z 는 이 창 밖(다른 기동 · 측정 뒤 — 이 측정과 무관할 수 있다) | sync_staging/sub_docs/logs/sub/events/2026-10.jsonl:732 |
| 98 | 2026-10-29T13:55:13Z | sub | `budget_honored` | `smoke-q38fn032-nv4-safe-tp2` | budget_honored · floor_mib=32446 · arm_ceiling_mib=29374 · remaining_s=10799 · 창 시작 2026-10-29T13:55:13Z 후 0m00s | sync_staging/sub_docs/logs/sub/events/watchdog.jsonl:676 |
| 99 | 2026-10-29T13:56:35Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=79733 · rate_mib_s=28453 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → smoke-q38fn032-nv4-safe-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T13:55:10Z 후 1m25s) | docs/logs/main/events/watchdog.jsonl:861 |
| 100 | 2026-10-29T13:56:36Z | sub | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=86011 · rate_mib_s=27678 · max_rate_mib_s=17801 · legacy_rule_would_trip=true · 라벨 없음 → smoke-q38fn032-nv4-safe-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T13:55:13Z 후 1m23s) | sync_staging/sub_docs/logs/sub/events/watchdog.jsonl:677 |
| 101 | 2026-10-29T14:08:57Z | sub | `watchdog_trip` | — | watchdog_trip · mem_avail_mib=16449 · rate_mib_s=5653 · rule=eta · action=docker_kill · 라벨 없음 → smoke-q38fn032-nv4-safe-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T13:55:13Z 후 13m44s) | sync_staging/sub_docs/logs/sub/events/watchdog.jsonl:678 |
| 102 | 2026-10-29T14:09:01Z | sub | `watchdog_kill_ack` | — | watchdog_kill_ack · target=a7537d313a29 · 라벨 없음 → smoke-q38fn032-nv4-safe-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T13:55:12Z 후 13m49s) · 같은 사건의 다른 스트림 사본 sync_staging/sub_docs/logs/sub/events/watchdog.jsonl:679(한 번으로 싣는다) | sync_staging/sub_docs/logs/sub/events/2026-10.jsonl:733 |
| 103 | 2026-10-29T14:09:07Z | main | `budget_renew` | `smoke-q38fn032-nv4-safe-tp2` | budget_renew · floor_mib=32446 · ttl_s=10800 · remaining_before_s=9963 · 창 시작 2026-10-29T13:55:10Z 후 13m57s | docs/logs/main/events/2026-10.jsonl:810 |
| 104 | 2026-10-29T14:18:48Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-q38fn032-nv4-safe-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T13:55:10Z 후 23m38s) | docs/logs/main/events/2026-10.jsonl:811 |
| 105 | 2026-10-29T14:18:49Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-q38fn032-nv4-safe-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T13:55:10Z 후 23m39s) | docs/logs/main/events/watchdog.jsonl:862 |
| 106 | 2026-10-29T14:26:46Z | main | `budget_declare` | `smoke-q38fn032-nv4-safe-tp2` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=32446 · weights_mib=63266 · kv_mib=4096 · overhead_mib=24800 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 기동 창 7/7: budget_renew 7회 · 닫힘 budget_clear 2026-10-29T15:00:27Z(선언 후 33m41s) · measured_utc 2026-10-29T11:41:26Z 는 이 창 밖(다른 기동 · 측정 뒤 — 이 측정과 무관할 수 있다) | docs/logs/main/events/2026-10.jsonl:813 |
| 107 | 2026-10-29T14:26:46Z | main | `budget_honored` | `smoke-q38fn032-nv4-safe-tp2` | budget_honored · floor_mib=32446 · arm_ceiling_mib=29374 · remaining_s=10800 · 창 시작 2026-10-29T14:26:46Z 후 0m00s | docs/logs/main/events/watchdog.jsonl:863 |
| 108 | 2026-10-29T14:28:05Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=83705 · rate_mib_s=26272 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → smoke-q38fn032-nv4-safe-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T14:26:46Z 후 1m19s) | docs/logs/main/events/watchdog.jsonl:864 |
| 109 | 2026-10-29T14:45:10Z | main | `budget_renew` | `smoke-q38fn032-nv4-safe-tp2` | budget_renew · floor_mib=32446 · ttl_s=10800 · remaining_before_s=9696 · 창 시작 2026-10-29T14:26:46Z 후 18m24s | docs/logs/main/events/2026-10.jsonl:814 |
| 110 | 2026-10-29T14:47:06Z | main | `budget_renew` | `smoke-q38fn032-nv4-safe-tp2` | budget_renew · floor_mib=32446 · ttl_s=7200 · remaining_before_s=10684 · 창 시작 2026-10-29T14:26:46Z 후 20m20s | docs/logs/main/events/2026-10.jsonl:815 |
| 111 | 2026-10-29T14:49:38Z | main | `budget_renew` | `smoke-q38fn032-nv4-safe-tp2` | budget_renew · floor_mib=32446 · ttl_s=7200 · remaining_before_s=7048 · 창 시작 2026-10-29T14:26:46Z 후 22m52s | docs/logs/main/events/2026-10.jsonl:816 |
| 112 | 2026-10-29T14:52:02Z | main | `budget_renew` | `smoke-q38fn032-nv4-safe-tp2` | budget_renew · floor_mib=32446 · ttl_s=7200 · remaining_before_s=7056 · 창 시작 2026-10-29T14:26:46Z 후 25m16s | docs/logs/main/events/2026-10.jsonl:817 |
| 113 | 2026-10-29T14:54:22Z | main | `budget_renew` | `smoke-q38fn032-nv4-safe-tp2` | budget_renew · floor_mib=32446 · ttl_s=7200 · remaining_before_s=7060 · 창 시작 2026-10-29T14:26:46Z 후 27m36s | docs/logs/main/events/2026-10.jsonl:818 |
| 114 | 2026-10-29T14:55:52Z | main | `budget_renew` | `smoke-q38fn032-nv4-safe-tp2` | budget_renew · floor_mib=32446 · ttl_s=7200 · remaining_before_s=7110 · 창 시작 2026-10-29T14:26:46Z 후 29m06s | docs/logs/main/events/2026-10.jsonl:819 |
| 115 | 2026-10-29T14:57:22Z | main | `budget_renew` | `smoke-q38fn032-nv4-safe-tp2` | budget_renew · floor_mib=32446 · ttl_s=7200 · remaining_before_s=7110 · 창 시작 2026-10-29T14:26:46Z 후 30m36s | docs/logs/main/events/2026-10.jsonl:820 |
| 116 | 2026-10-29T14:58:48Z | 미관측 | `measurement` | `measurement (budget window recorded)` | 같은 셀의 다른 측정(측정 노드 축 cluster) · 시각 = 측정 조립(sweep generated_utc = 인증서 measured_utc · 시작 시각 미기록) · 이 발행의 측정 2026-10-29T11:41:26Z 뒤 — 이 측정과 무관할 수 있다 · 출력 평면에 보존된 같은 셀의 다른 측정 · 원장: 이 셀 선언 창 안: smoke-q38fn032-nv4-safe-tp2 2026-10-29T14:26:46Z~2026-10-29T15:00:27Z (docs/logs/main/events/2026-10.jsonl:813) · smoke-q38fn032-nv4-safe-tp2 2026-10-29T14:26:46Z~2026-10-29T15:00:28Z (docs/logs/main/events/watchdog.jsonl:863) · 이 시각을 관측하지 않은 원장: sub(기록 2026-06-13T22:25:01Z~2026-10-29T14:09:01Z) · 직전 예산 행 budget_renew(smoke-q38fn032-nv4-safe-tp2) 2026-10-29T14:57:22Z(docs/logs/main/events/2026-10.jsonl:820) · 직후 예산 행 budget_clear 2026-10-29T15:00:27Z(docs/logs/main/events/2026-10.jsonl:821) | output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029H/sweep_index.json:4 |
| 117 | 2026-10-29T15:00:27Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-q38fn032-nv4-safe-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T14:26:46Z 후 33m41s) | docs/logs/main/events/2026-10.jsonl:821 |
| 118 | 2026-10-29T15:00:28Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-q38fn032-nv4-safe-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T14:26:46Z 후 33m42s) | docs/logs/main/events/watchdog.jsonl:865 |

**기동 시도**(예산 선언 `budget_declare` 마다 한 시도 · 같은 노드의 이어지는 행과 선언 없는 노드(엔진 로그)의 그 시각 행을 묶었다 · 사살 · 트립 · 거부는 닫힘과 따로 센다 · `measurement` 행은 측정 기록이라 시도로 세지 않는다(위 표에만) — 해석은 저작자 몫)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 1 | main | `smoke-q38fn032-nv4-safe-tp2` | 2026-10-29T10:53:09Z | 10회 · 첫 2026-10-29T11:24:58Z | 없음 | `budget_clear` 2026-10-29T11:42:50Z | 3 |
| 2 | sub | `smoke-q38fn032-nv4-safe-tp2` | 2026-10-29T10:53:12Z | 1회 · 첫 2026-10-29T11:24:59Z | 없음 | `budget_clear` 2026-10-29T11:42:50Z | 11 |
| 3 | main | `smoke-q38fn032-nv4-safe-tp2` | 2026-10-29T12:17:18Z | 10회 · 첫 2026-10-29T12:36:16Z | 없음 | `budget_clear` 2026-10-29T12:55:14Z | 3 |
| 4 | sub | `smoke-q38fn032-nv4-safe-tp2` | 2026-10-29T12:17:20Z | 1회 · 첫 2026-10-29T12:36:17Z | 없음 | `budget_clear` 2026-10-29T12:55:14Z | 3 |
| 5 | main | `smoke-q38fn032-nv4-safe-tp2` | 2026-10-29T12:57:24Z | 1회 · 첫 2026-10-29T13:06:44Z | 없음 | `budget_clear` 2026-10-29T13:09:33Z | 3 |
| 6 | sub | `smoke-q38fn032-nv4-safe-tp2` | 2026-10-29T12:57:26Z | 1회 · 첫 2026-10-29T13:06:45Z | 없음 | `budget_clear` 2026-10-29T13:09:33Z | 3 |
| 7 | main | `smoke-q38fn032-nv4-safe-tp2` | 2026-10-29T13:11:58Z | 없음 | `watchdog_kill_ack` 1 · `watchdog_trip` 1 | `budget_clear` 2026-10-29T13:30:21Z | 3 |
| 8 | sub | `smoke-q38fn032-nv4-safe-tp2` | 2026-10-29T13:12:00Z | 없음 | `watchdog_kill_ack` 1 · `watchdog_trip` 1 | `budget_clear` 2026-10-29T13:30:21Z | 3 |
| 9 | main | `smoke-q38fn032-nv4-safe-tp2` | 2026-10-29T13:33:11Z | 없음 | `watchdog_kill_ack` 2 · `watchdog_trip` 1 | `budget_clear` 2026-10-29T13:49:10Z | 3 |
| 10 | sub | `smoke-q38fn032-nv4-safe-tp2` | 2026-10-29T13:33:14Z | 없음 | `watchdog_kill_ack` 2 · `watchdog_trip` 2 | `budget_clear` 2026-10-29T13:49:10Z | 3 |
| 11 | main | `smoke-q38fn032-nv4-safe-tp2` | 2026-10-29T13:55:10Z | 1회 · 첫 2026-10-29T14:09:07Z | 없음 | `budget_clear` 2026-10-29T14:18:48Z | 3 |
| 12 | sub | `smoke-q38fn032-nv4-safe-tp2` | 2026-10-29T13:55:12Z | 없음 | `watchdog_kill_ack` 1 · `watchdog_trip` 1 | —(닫힘 행 없음) | 2 |
| 13 | main | `smoke-q38fn032-nv4-safe-tp2` | 2026-10-29T14:26:46Z | 7회 · 첫 2026-10-29T14:45:10Z | 없음 | `budget_clear` 2026-10-29T15:00:27Z | 3 |
<!-- /FACT:event_timeline -->

<!-- FACT:measurement_env -->
**측정 실행의 env 관측 30행**(측정한 실행의 엔진 로그가 echo 한 값 · 기계 발췌 — 측정 당시 값의 정본이다. 표에 없는 키는 로그가 echo 하지 않은 것이지 설정되지 않았다는 뜻이 아니다 · 측정 뒤 재생성된 env 형상은 01 §1.1 을 본다) — 그중 3행은 종류 `key-present`: 키가 프로세스 env 에 **있었다**는 관측뿐이고 값은 로그에 없다(값 미관측)

| # | 키 | 값 | 노드 | 종류 | 출처(첫 줄) | 관측 |
|---|---|---|---|---|---|---|
| 1 | `VLLM_PLE_MMAP` | (키 존재 · 값 미관측) | 미특정 | key-present | output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B/lite_engine_head_q38fn032-nv4-safe-tp2.log:L64 | 로그 1개 |
| 2 | `VLLM_PLE_MMAP_DIR` | (키 존재 · 값 미관측) | 미특정 | key-present | output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B/lite_engine_head_q38fn032-nv4-safe-tp2.log:L65 | 로그 1개 |
| 3 | `VLLM_REF` | (키 존재 · 값 미관측) | 미특정 | key-present | output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B/lite_engine_head_q38fn032-nv4-safe-tp2.log:L66 | 로그 1개 |
| 4 | `NCCL_SOCKET_IFNAME` | `<nic:cluster>` | main | env-echo | output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B/lite_engine_head_q38fn032-nv4-safe-tp2.log:L101 | 로그 1개 |
| 5 | `NCCL_SOCKET_IFNAME` | `<nic:cluster>` | main | param-echo | output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B/lite_engine_head_q38fn032-nv4-safe-tp2.log:L102 | 로그 1개 |
| 6 | `nccl:version` | `2.30.7+cuda13.3` | main | runtime | output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B/lite_engine_head_q38fn032-nv4-safe-tp2.log:L105 | 로그 1개 |
| 7 | `NCCL_NET_PLUGIN` | `spcx` | main | env-echo | output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B/lite_engine_head_q38fn032-nv4-safe-tp2.log:L107 | 로그 1개 |
| 8 | `NCCL_IB_DISABLE` | `0` | main | env-echo | output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B/lite_engine_head_q38fn032-nv4-safe-tp2.log:L112 | 로그 1개 |
| 9 | `NCCL_IB_HCA` | `=<nic:cluster>,<nic:cluster>` | main | param-echo | output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B/lite_engine_head_q38fn032-nv4-safe-tp2.log:L114 | 로그 1개 |
| 10 | `NCCL_IB_MERGE_NICS` | `1` | main | env-echo | output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B/lite_engine_head_q38fn032-nv4-safe-tp2.log:L117 | 로그 1개 |
| 11 | `nccl:net_ib_devices` | `[0]<nic:cluster>:1/RoCE [1]<nic:cluster>:1/RoCE [RO]; OOB <nic:cluster>:<node:main><0>` | main | runtime | output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B/lite_engine_head_q38fn032-nv4-safe-tp2.log:L120 | 로그 1개 |
| 12 | `NCCL_IB_QPS_PER_CONNECTION` | `4` | main | env-echo | output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B/lite_engine_head_q38fn032-nv4-safe-tp2.log:L133 | 로그 2개 |
| 13 | `nccl:network` | `IB` | sub | runtime | output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B/lite_engine_head_q38fn032-nv4-safe-tp2.log:L156 | 로그 1개 |
| 14 | `NCCL_DMABUF_ENABLE` | `0` | sub | env-echo | output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B/lite_engine_head_q38fn032-nv4-safe-tp2.log:L158 | 로그 1개 |
| 15 | `NCCL_CROSS_NIC` | `1` | sub | env-echo | output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B/lite_engine_head_q38fn032-nv4-safe-tp2.log:L161 | 로그 1개 |
| 16 | `NCCL_IB_SPLIT_DATA_ON_QPS` | `0` | sub | env-echo | output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B/lite_engine_head_q38fn032-nv4-safe-tp2.log:L549 | 로그 2개 |
| 17 | `NCCL_IB_GID_INDEX` | `3` | sub | env-echo | output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B/lite_engine_head_q38fn032-nv4-safe-tp2.log:L558 | 로그 2개 |
| 18 | `NCCL_SOCKET_IFNAME` | `<nic:cluster>` | sub | env-echo | output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B/lite_engine_q38fn032-nv4-safe-tp2.log:L358 | 로그 1개 · Ray 접힘 +3(접힌 사본의 노드 미관측) |
| 19 | `NCCL_SOCKET_IFNAME` | `<nic:cluster>` | sub | param-echo | output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B/lite_engine_q38fn032-nv4-safe-tp2.log:L359 | 로그 1개 · Ray 접힘 +2(접힌 사본의 노드 미관측) |
| 20 | `nccl:version` | `2.30.7+cuda13.3` | sub | runtime | output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B/lite_engine_q38fn032-nv4-safe-tp2.log:L362 | 로그 1개 |
| 21 | `NCCL_NET_PLUGIN` | `spcx` | sub | env-echo | output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B/lite_engine_q38fn032-nv4-safe-tp2.log:L364 | 로그 1개 |
| 22 | `NCCL_IB_DISABLE` | `0` | sub | env-echo | output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B/lite_engine_q38fn032-nv4-safe-tp2.log:L369 | 로그 1개 · Ray 접힘 +2(접힌 사본의 노드 미관측) |
| 23 | `NCCL_IB_HCA` | `=<nic:cluster>,<nic:cluster>` | sub | param-echo | output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B/lite_engine_q38fn032-nv4-safe-tp2.log:L370 | 로그 1개 · Ray 접힘 +2(접힌 사본의 노드 미관측) |
| 24 | `NCCL_IB_MERGE_NICS` | `1` | sub | env-echo | output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B/lite_engine_q38fn032-nv4-safe-tp2.log:L372 | 로그 1개 |
| 25 | `nccl:net_ib_devices` | `[RO]; OOB <nic:cluster>:<node:main><0>` | main | runtime | output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B/lite_engine_q38fn032-nv4-safe-tp2.log:L374 | 로그 1개 · Ray 접힘 +3(접힌 사본의 노드 미관측) |
| 26 | `nccl:network` | `IB` | main | runtime | output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B/lite_engine_q38fn032-nv4-safe-tp2.log:L397 | 로그 1개 · Ray 접힘 +3(접힌 사본의 노드 미관측) |
| 27 | `NCCL_DMABUF_ENABLE` | `0` | main | env-echo | output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B/lite_engine_q38fn032-nv4-safe-tp2.log:L399 | 로그 1개 |
| 28 | `NCCL_CROSS_NIC` | `1` | main | env-echo | output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B/lite_engine_q38fn032-nv4-safe-tp2.log:L402 | 로그 1개 |
| 29 | `NCCL_IB_SPLIT_DATA_ON_QPS` | `0` | main | env-echo | output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B/lite_engine_q38fn032-nv4-safe-tp2.log:L457 | 로그 1개 |
| 30 | `NCCL_IB_GID_INDEX` | `3` | main | env-echo | output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B/lite_engine_q38fn032-nv4-safe-tp2.log:L462 | 로그 1개 |

- 값 주: 기계 치환(pii.substitution_table · 장치·호스트·주소는 자리표시 — 원문은 출처 줄)
- 값 주: 키 존재만 관측 · 값 미관측(vLLM 'Unknown vLLM environment variable detected' 경고 — 값은 로그에 없다)

- Ray 접힘: 출처 줄 끝의 `[repeated Nx across cluster]` — Ray 가 다른 프로세스의 같은 줄 N개를 한 줄로 접었다. 노드 칸은 **첫 줄**의 노드이고, 접힌 사본이 어느 노드의 것인지는 로그에 없다(관측 대상 밖).

- 로그 창(꼬리): `tail-800(lite_bench.sh@d33b0878a6c3 L234)`(꼬리 창 출처 로그 1개 · 이 표 행 #18–30) — 이 캡처는 엔진 출력의 **마지막 N줄**이다(측정 도구의 `tail -N` 상한에 닿았다 · 생산자 evidence `log_window`). 창 앞에서 echo 한 줄은 그 로그에 없다 — 이 주의는 꼬리 창 출처 행에만 걸린다(머리 캡처 행은 아래).

- 로그 창(머리): `head-600(output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B/lite_raw_q38fn032-nv4-safe-tp2.json engine_log_head_lines)`(머리 캡처 출처 로그 1개 · 이 표 행 #1–17) — 이 캡처는 엔진 출력의 **처음 N줄**이다(lite_bench.sh 머리 캡처 `head -n N` 상한에 닿았다 · 생산자 evidence `log_head_window`). 꼬리 창의 주의는 이 행에 걸리지 않는다 · 머리 N줄 뒤에서 echo 한 줄은 그 로그에 없다. 머리 캡처와 꼬리 창 사이(엔진 출력의 가운데)는 어느 캡처에도 없다 — 표에 없는 키 · 한 노드에만 있는 키는 그 사이였을 수 있다.
<!-- /FACT:measurement_env -->

> 이 절이 답하는 질문: 수신자가 이 zip 만으로 이 셀을 다시 띄우려면 어떤 순서로 무엇을 하고, 각 단계의 성공을 무엇으로 판정하며, 얼마나 기다려야 하는가?

1. 양노드관리NAS체크포인트를같은컨테이너경로에read-only마운트한다. env자리는artifacts/compose의형상과triplet셀env다. B는PLEresident이므로PLEmmap테이블생성이필요하지않다.
2. 부모키트는`artifacts/build_recipe/parent-source-core/README.md`의parent_source_core→alias→leaf순서다. build_recipe를작업디렉터리로하고core의context_root는`parent-source-core/context/`만쓴다. 실제main은core+05를arch12.1/jobs8로빌드한뒤deps1동일IDalias를만들고deps2/06을빌드했다. sub의과거는기존core/별도deps1수리라동일역사로쓰지않는다. README의세명령은별도REPRO태그를쓰는새localbuild제안이며미실행이다. 원본core5파일/실제core명령과재구성된leafargv의지위를구분한다. leafcontext는선택Dockerfile.source-build-deps2와POST06만준비하고현i5슬롯을복사하지않는다. 위9분23초는층창이지전체build소요가아니다. leafDockerfile전체원본은미검증경고를유지한다.
3. mainhead→subworker→GPU합류2→mainserve순서다. 양노드weights63266/KV14336/overhead24800MiB는보호선언입력이고MemTotal124608은러너관측이다. floor22206MiB도예산계산값이다. 엔진target+draft63.04GiB/874.953210초는별도로드관측이다(testlog_26102920_Qwen032_CUTLASS_Full_판정 §빌드와 기능).
4. 선택B의servephase10:53:14~11:24:57Z와health200/chat143자는기능근거다. profile/KV/warmup867.91초와graph2초/0.07GiB도그문서의엔진관측이다. 두checkpoint순회는target→draft로드였고새프로세스재시작이아니다. 기동JIT가길어도컴파일/자동튜닝이전진했고그뒤기능응답을얻었다.
5. 이번B측정은11:27:11Z~11:41:26Z, lite∪GuideLLM9run이고14분15초는그구간시각차다. 03§3.4의도구원문과위JSON재구성명령은서로다른증거층이다. Full후정식down이양노드컨테이너·워치독·TTL·선언을회수했다.
6. 엔진은targetNVFP4 FLASHINFER_CUTLASS/draftFP8 TRITON을골랐다. NCCL/IB실효값은측정env표를따른다. UnknownPLEenv키경고의키존재를mmap실행증거로읽지않는다. 별도cellruntimepatch는없어전용발화서명도없다. attestation/proof의복원mtime만으로재기동을확정하지않고보존B원형과원장창으로대조한다.

## 1.5 서브 레시피 해설
<!-- FACT:sub_recipe -->
원본: `artifacts/compose/sub_recipe.json`(기계 파생 — 스모크와 같은 함수 `slave_forward.derive` 의 출력).

| 역할 | 하는 일 |
|---|---|
| `master` | ray head + vllm serve(트리플렛 source · 모델 러너 실행) |
| `master_runtime_patch` | none(런타임 패치 파일 없음 — CONFIG_FILE='q38fn032-nv4-safe-tp2' → q38fn032-nv4-safe-tp2_patch.py 부재) |
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
| `source` | output/multi/benchlog/attestation_q38fn032-nv4-safe-tp2.json |

> **attestation 범위** — config `q38fn032-nv4-safe-tp2` · phase serve · 작성 2026-10-29T11:42:14Z(output/multi/benchlog/attestation_q38fn032-nv4-safe-tp2.json 파일 mtime(파일시스템 관측 — 문서에 시각 칸이 없다 · 기록 필드 아님)) · 묶은 근거 cell-name · 파일 `output/multi/benchlog/attestation_q38fn032-nv4-safe-tp2.json` · 측정 창 대비 작성 2026-10-29T11:42:14Z > 측정 끝 2026-10-29T11:41:26Z — 측정 뒤 작성된 관측(측정 시점·같은 기동 여부는 시각만으로 미검증): **same config(q38fn032-nv4-safe-tp2) · serve — 측정 뒤 작성된 관측(측정 시점·같은 기동 여부는 시각만으로 미검증 · 파일은 다음 실행이 덮는다)**
<!-- /FACT:sub_recipe -->

<!-- FACT:measurement_env_nodes -->
**노드별 측정 env**(01 §1.4 표를 키 × 노드로 접은 것 · `만 관측` = 다른 쪽 줄이 캡처 로그에 없다 · Ray 가 같은 줄을 접은 키(`[repeated Nx across cluster]`)와 머리·꼬리 캡처 로그(01 §1.4 로그 창)에서 한쪽만 남은 키는 다른 쪽 부재를 단언하지 않는다)

- 양 노드 같은 값 11개: `NCCL_CROSS_NIC` · `NCCL_DMABUF_ENABLE` · `NCCL_IB_DISABLE` · `NCCL_IB_GID_INDEX` · `NCCL_IB_HCA` · `NCCL_IB_MERGE_NICS` · `NCCL_IB_SPLIT_DATA_ON_QPS` · `NCCL_NET_PLUGIN` · `NCCL_SOCKET_IFNAME` · `nccl:network` · `nccl:version`

| 키 | main | sub | 노드 미특정 | 판정 |
|---|---|---|---|---|
| `NCCL_IB_QPS_PER_CONNECTION` | `4` | — | — | main 만 캡처 창에 남음 — sub 여부 미관측(머리 캡처) |
| `VLLM_PLE_MMAP` | — | — | (키 존재 · 값 미관측) | 노드 미특정 |
| `VLLM_PLE_MMAP_DIR` | — | — | (키 존재 · 값 미관측) | 노드 미특정 |
| `VLLM_REF` | — | — | (키 존재 · 값 미관측) | 노드 미특정 |
| `nccl:net_ib_devices` | `[0]<nic:cluster>:1/RoCE [1]<nic:cluster>:1/RoCE [RO]; OOB <nic:cluster>:<node:main><0>` · `[RO]; OOB <nic:cluster>:<node:main><0>` | — | — | main 만 줄에 남음 · Ray 접힘 +3 — sub 사본 여부 미관측 |
<!-- /FACT:measurement_env_nodes -->

> 이 절이 답하는 질문: 서브(Ray worker) 노드는 메인과 무엇이 다르고, 무엇을 반드시 같게 맞춰야 하는가?

master는Rayhead+vllmserve,slave는Rayworker다. 트리플렛은master소유이고slave의CONFIG_FILE=default는모델트리플렛미배달계약이다. runtimepatch는이번셀에없으며slave가모델별patch를무장했다고주장하지않는다.

서브에는이미지정체성IMAGE_TAG/BUILD_DOCKERFILE/VLLM_REF,빌드튜닝BUILD_JOBS,공유모델마운트,클러스터역할/컨테이너이름,인터커넥트env가각소유평면으로전달돼야한다. 노드별물리경로와접속좌표는manifest에서해소한다. B는resident이므로PLEmmap로컬스테이징은불해당이다.

양노드SHA/torch/driver3축equal이지만원장identity는differ다. 실제SHA는같고상속tag/40SHA표현·부모원장/의존수리범위와imageId·SCM날짜는다르다(testlog_26102920_Qwen032_CUTLASS_Full_판정 §빌드와 기능). 전체이미지동등은주장하지않는다. attestation/proof시각은측정뒤작성된파일이라는관측이며동일기동여부를파일시각만으로확정하지않는다. 보존B원형/원장창과함께읽는다.

측정NCCL env표의양노드동일키만동일로읽고Ray접힘/노드미특정/로그창제한은한쪽만설정됐다는증거가아니다. 현재렌더러bccd080의선택과측정전판본은다를수있으므로실린env형상/실제로그값/재현렌더경고를나란히읽는다. masterGPU합류리터럴2는다른노드/GPU구성에서다시도출할값이다.

## 1.6 재현 절차 — 기동 전 준비(사람이 할 일)
> 1.4 의 명령을 치기 **전에** 사람이 손으로 해야 하는 것이다 — 가중치 획득 · 스테이징 생성 · 네트워크가 필요한 단계. 에이전트가 대신 할 수 없는 자리(다운로드 승인 · 물리 경로 · 외부 계정)가 여기 모인다.

> 이 절이 답하는 질문: 이 셀을 기동하기 전에 사람이 준비해야 하는 것은 무엇이며, 각각을 어떤 명령으로 만들고, 그 절차의 출처는 어디인가?

### 가중치
관리NAS의nvidia/Qwen3.8-Flash-Next-NVFP4를양노드같은마운트형상에준비한다. checkout은00§0.2의40자리revision이며작업트리바이트전체일치는미검증이다. 사전검사예시는 `test -r <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4/config.json` 과safetensors index존재검사다(artifacts/triplet yaml·compose에서유도;이명령새실행검증주장아님). 크기는safetensors index132639846394B/123.53GiB근거로판단하고NASdu248G를사용하지않는다(testlog_26102406 §근거). 자동다운로드는하지않는다.
### PLE와기타스테이징
B의PLE는resident라mmap생성불해당이다. 과거mmap플러그인/스테이징을이셀로옮기지않는다. GuideLLM tokenizer준비는측정도구가별도로수행한다.
### 부모 core와 local-build 사전준비
parent-source-core/README.md는새재현안이고미실행이다. `context/`의5개고정원본파일과빈PRE를보존하고alias/argv/ledger/README/manifest를그밖에둔다. README의명령수준순서는고정NGCdigest준비→corearch12.1/jobs8빌드→로컬deps1태그alias→POST06만있는leafcontext빌드다. core원명령은core-build-command.txt이고leaf명령은leaf-ledger-evidence.json에서재구성했으며같은관측등급으로읽지않는다. 각노드의독립localbuild가끝나야새ABI/기능을검증할수있다. 이번키트수집때실제로양노드를다시빌드하지않았다.
### 외부네트워크
새환경에서core/deps1/deps2빌드의고정베이스이미지·공식소스·pip·POST05의의존수리·POST06exactwheel과GuideLLM이미지를미리준비해야한다. 06파일에설치URL/해시와검증명령이있으나이것은모델가중치다운로드승인이아니다. 오프라인환경은사람이승인한동일소스/의존물/이미지를사전준비하고양노드각자로컬빌드한다. 실제실행되지않은획득명령을검증된것처럼싣지않는다.
