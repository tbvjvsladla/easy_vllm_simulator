# 01 · 산출물 — 무엇이 실제로 쓰였고, 어떻게 다시 띄우는가

## 1.1 적용 판정 표
> 기계가 적용 증거(빌드 원장 · 원장이 없으면 라벨된 재구성) × 파일 × 선언을 대사한 결과다. `artifacts/` 에는 재현 입력과 확인되지 않은 참고 형상도 포함될 수 있다. 파일별 검증 상태와 근거를 아래 표에서 확인한다.

<!-- FACT:slots -->
| 슬롯 | 실림 | 파일 | 신호 | 적용 증거 | 사유 |
|---|---|---|---|---|---|
| `build_patch_post` | 실림 | `artifacts/build_patch_post/05-ngc-source032-deps-compat.sh` · `artifacts/build_patch_post/06-b12x-optional032.sh` · `artifacts/build_patch_post/70-exllamav3-build.sh` · `artifacts/build_patch_post/72-exl3-row-store.sh` · `artifacts/build_patch_post/73-vllm-exl3-build.sh` · `artifacts/build_patch_post/74-exl3-moe-prep-fused.sh` · `artifacts/build_patch_post/75-exl3-python-deps032.sh` · `artifacts/build_patch_post/79-exl3-port-selfcheck.sh` | 3-signal | build-ledger:attestation:output/multi/benchlog/attestation_i5-032-sm12x.json#main | NGC datasets/vLLM의hub교집합1.33.0과pytest9.2.0을설치/pipcheck하는공유의존수리。 core단계에도있으며childPOST재고/적용과부모이력을각원장으로구분한다。 상류의존교집합변경시재도출;set-e/pipcheck실패종료。 · 공식0.32 optional B12X1.5.1 exactwheel설치/API확인。 적용과GPU전체커널검증은별개이며옛operator1.2.3을이식성공으로쓰지않는다。 assert/pipcheck실패종료;공식optional핀변경시다시평가。 · pinnedexllamav3 1.4.5에Mia aarch64/fat-kernel을가공해native컴파일。 실제MCG bits2/384experts/hidden5120/intermediate_local1152가발화;exper… |
| `build_patch_pre` | 실림 | `artifacts/build_patch_pre/80-exl3-quant-method.sh` · `artifacts/build_patch_pre/81-exl3-model-wiring.sh` · `artifacts/build_patch_pre/82-exl3-h2d-stage.sh` · `artifacts/build_patch_pre/83-engram-file-backend.sh` · `artifacts/build_patch_pre/84-sm12x-block-shim.sh` · `artifacts/build_patch_pre/85-exl3-serial-streams.sh` · `artifacts/build_patch_pre/86-exl3-worker-hooks.sh` · `artifacts/build_patch_pre/87-engram-embed-skip.sh` · `artifacts/build_patch_pre/88-engram-prestage.sh` · `artifacts/build_patch_pre/89-kv-null-block-zero.sh` | 3-signal | build-ledger:attestation:output/multi/benchlog/attestation_i5-032-sm12x.json#main | 공식0.32양자화registry/lazyimport에EXL3를등록하고chattemplate를설치。 현재quantizationexl3로드의진입경로(W1/W11),일반모델로의필요조건확장은아님。 SM12X_PORT0skip;정식등록중복/버전·anchor차이는fail-loud;상류EXL3지원흡수시걷어낼지재평가。 · LMheadctor/mapper/VLprefix3접점을새모델에배선。 lmheadMXFP8실관측,language-only셀이라vision전체기능검증아님。 SM12X_PORT0skip/anchor개수불일치실패;동등정식배선이있으면재평가。 · loader/linear/vocab/model/dspark14접점의H2Dstage를이식。 로드는완주했으나이를빼본대조없어고유성능기여/필수성미확정。 SM12X_PORT0sk… |
| `build_recipe` | 실림 | `artifacts/build_recipe/Dockerfile.source-build-sm12x` · `artifacts/build_recipe/parent-source-core/README.md` · `artifacts/build_recipe/parent-source-core/alias-evidence.json` · `artifacts/build_recipe/parent-source-core/context/Dockerfile.source-build` · `artifacts/build_recipe/parent-source-core/context/build_patches/.gitkeep` · `artifacts/build_recipe/parent-source-core/context/build_patches/05-ngc-source032-deps-compat.sh` · `artifacts/build_recipe/parent-source-core/context/build_patches_src/.gitkeep` · `artifacts/build_recipe/parent-source-core/context/requirements.txt` · `artifacts/build_recipe/parent-source-core/core-build-command.txt` · `artifacts/build_recipe/parent-source-core/intermediate-deps2/context/Dockerfile.source-build-deps2` · `artifacts/build_recipe/parent-source-core/intermediate-deps2/context/build_patches/06-b12x-optional032.sh` · `artifacts/build_recipe/parent-source-core/leaf-build-command.txt` · `artifacts/build_recipe/parent-source-core/leaf-ledger-evidence.json` · `artifacts/build_recipe/parent-source-core/leaf-result-evidence.json` · `artifacts/build_recipe/parent-source-core/leaf-support/build_patches/.gitkeep` · `artifacts/build_recipe/parent-source-core/leaf-support/build_patches/Dockerfile.source-build-deps2` · `artifacts/build_recipe/parent-source-core/leaf-support/build_patches_src/.gitkeep` · `artifacts/build_recipe/parent-source-core/leaf-support/build_patches_src/PROVENANCE.json` · `artifacts/build_recipe/parent-source-core/leaf-support/build_patches_src/exl3_port_lib.py` · `artifacts/build_recipe/parent-source-core/manifest.json` | 3-signal | build-ledger:build-ledger.dockerfile | NGC datasets/vLLM의hub교집합1.33.0과pytest9.2.0을설치/pipcheck하는공유의존수리。 core단계에도있으며childPOST재고/적용과부모이력을각원장으로구분한다。 상류의존교집합변경시재도출;set-e/pipcheck실패종료。 · 공식0.32 optional B12X1.5.1 exactwheel설치/API확인。 적용과GPU전체커널검증은별개이며옛operator1.2.3을이식성공으로쓰지않는다。 assert/pipcheck실패종료;공식optional핀변경시다시평가。 |
| `compose` | 실림 | `artifacts/compose/.env.cluster.template` · `artifacts/compose/.env.interconnect.template` · `artifacts/compose/.env.template` · `artifacts/compose/docker-compose.yaml` · `artifacts/compose/serve_runner.sh` · `artifacts/compose/sub_recipe.json` | 1-signal | file:output/multi/docker-compose.yaml | multi 서빙 경로(compose · 러너 · env 형상) — 측정 당시 개정(selected_revisions)(기계 파생) |
| `fork_pin` | 안 실림 | — | 1-signal | none:.claude/policies/arch_variant_ledger.json | 셀 env 에 VARIANT 줄 없음 = stock(workflow.md §변종 좌표의 거처)(기계 파생) |
| `runtime_patch` | 안 실림 | — | 1-signal | none:output/multi/configs/i5-032-sm12x_patch.py | output/multi/configs/i5-032-sm12x_patch.py 없음 — arm_patch.sh 는 no-op(런타임 패치 불해당)(기계 파생) |
| `triplet` | 실림 | `artifacts/triplet/.env.i5-032-sm12x` · `artifacts/triplet/i5-032-sm12x.sh` · `artifacts/triplet/i5-032-sm12x.yaml` | 2-signal | file:output/multi/configs/i5-032-sm12x.yaml | 셀 id i5-032-sm12x = 트리플렛 이름(configs/i5-032-sm12x.yaml · .sh · envs/.env.i5-032-sm12x)(기계 파생) |

**레시피 경고 · 실린 리비전**(기계 관측 — 수신자 빌드가 여기서 갈라질 수 있다)

- ⚠ `build_recipe` `output/multi/Dockerfile.source-build-sm12x` — 실린 바이트(worktree)가 측정 이미지 docker history 의 RUN·COPY 와 전수 일치하지 않는다(9/31)(이미지 Created 2026-10-29T11:59:03Z · 마지막 커밋 미관측)
- `build_recipe` `output/multi/Dockerfile.source-build-sm12x` 실린 리비전: 리비전 미특정 · 선택 방법 `worktree(어느 개정도 이미지 history 와 더 맞지 않는다 · 작업트리 9/31 · 대조할 개정 없음(이미지 Created 이전 커밋 부재 · 비추적) · 전 이력(output/*/Dockerfile.source-build-sm12x · 모든 ref)에도 전수 일치 개정 없음 — recipe_vs_image 경고 유지)` · docker history 대조 9/31 · 실린 바이트 `worktree` · 쓰인 바이트임을 관측으로 확인 아니오
- `compose` `.claude/skills/upstream-version-watch/assets/configs/serve_runner.sh` 실린 리비전: 커밋 `50a5ce486738` · 선택 방법 `worktree(mtime 2026-09-06T21:57:53Z ≤ 측정 2026-10-29T16:43:41Z(sweep_index.generated_utc) · 바이트 = 측정 전 마지막 커밋 50a5ce486738)` · docker history 대조 없음 · 실린 바이트 `worktree` · 근거 `mtime≤measured` · 쓰인 바이트임을 관측으로 확인 아니오
- `compose` `output/multi/docker-compose.yaml` 실린 리비전: 커밋 `bccd08091976` · 선택 방법 `worktree(mtime 2026-10-29T10:01:25Z ≤ 측정 2026-10-29T16:43:41Z(sweep_index.generated_utc) · 바이트 = 측정 전 마지막 커밋 bccd08091976)` · docker history 대조 없음 · 실린 바이트 `worktree` · 근거 `mtime≤measured` · 쓰인 바이트임을 관측으로 확인 아니오
- ⚠ `triplet` 측정 뒤 재생성된 파일(mtime > 측정 시각)을 실었다: `output/multi/envs/.env.i5-032-sm12x` — 실린 형상은 측정 당시 값과 다를 수 있다(측정 당시 값 = 01 §1.4 측정 env 관측)

**싣지 않은 파일**(슬롯별 사유 — 빌드 때 skip 된 패치는 아래 적용 집합 표에 있다)

- `build_patch_post` known-non-patch 1건: `.gitkeep`
- `build_patch_pre` `exl3_port_lib.py` — `nonconforming(<NN>-*.sh 규약 밖 — 싣지 않음)`
- `build_patch_pre` known-non-patch 2건: `.gitkeep` · `PROVENANCE.json`
- `compose` `arm_patch.sh` — `not-armed(output/multi/configs/i5-032-sm12x_patch.py 없음(CONFIG_FILE) · slave default_patch.py(CONFIG_FILE) 없음 — arm_patch.sh 는 no-op · arm_patch.sh 측정 당시 개정(1b1df2cd4034) `_cfg=` 키 체인 RUNTIME_PATCH_STEM > CONFIG_FILE > 'default')`

**파일별 검증 · 관련성**(`generated-unverified` = 실행 검증되지 않은 생성물 — 파일 첫 줄 경고 · PAYLOAD 파일 기록과 같은 표시 · 관련성 = ① 패치 선언 대상 × 모델 config ② 엔진 로그 발화 서명 — 둘 다 관측일 때만 required/inactive-inferred)

| 슬롯 | 파일 | 검증 | 검증 근거 | 관련성 | 관련성 근거 |
|---|---|---|---|---|---|
| `build_patch_post` | `artifacts/build_patch_post/05-ngc-source032-deps-compat.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_i5-032-sm12x.json#main) · 원장 관측 | `unknown` | ① 패치 헤더·본문에 대상 선언(model-trigger 아키텍처 · vllm/models/<model_type>) 없음 |
| `build_patch_post` | `artifacts/build_patch_post/06-b12x-optional032.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_i5-032-sm12x.json#main) · 원장 관측 | `unknown` | ① 패치 헤더·본문에 대상 선언(model-trigger 아키텍처 · vllm/models/<model_type>) 없음 |
| `build_patch_post` | `artifacts/build_patch_post/70-exllamav3-build.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_i5-032-sm12x.json#main) · 원장 관측 | `unknown` | ① 패치 헤더·본문에 대상 선언(model-trigger 아키텍처 · vllm/models/<model_type>) 없음 |
| `build_patch_post` | `artifacts/build_patch_post/72-exl3-row-store.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_i5-032-sm12x.json#main) · 원장 관측 | `unknown` | ① 패치 헤더·본문에 대상 선언(model-trigger 아키텍처 · vllm/models/<model_type>) 없음 |
| `build_patch_post` | `artifacts/build_patch_post/73-vllm-exl3-build.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_i5-032-sm12x.json#main) · 원장 관측 | `unknown` | ① 패치 헤더·본문에 대상 선언(model-trigger 아키텍처 · vllm/models/<model_type>) 없음 · 편집하는 공유(모델 디렉터리 밖) 파일 ['vllm/model_executor/model_loader/weight_utils.py'] |
| `build_patch_post` | `artifacts/build_patch_post/74-exl3-moe-prep-fused.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_i5-032-sm12x.json#main) · 원장 관측 | `unknown` | ① 패치 헤더·본문에 대상 선언(model-trigger 아키텍처 · vllm/models/<model_type>) 없음 |
| `build_patch_post` | `artifacts/build_patch_post/75-exl3-python-deps032.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_i5-032-sm12x.json#main) · 원장 관측 | `unknown` | ① 패치 헤더·본문에 대상 선언(model-trigger 아키텍처 · vllm/models/<model_type>) 없음 |
| `build_patch_post` | `artifacts/build_patch_post/79-exl3-port-selfcheck.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_i5-032-sm12x.json#main) · 원장 관측 | `unknown` | ① 패치 헤더·본문에 대상 선언(model-trigger 아키텍처 · vllm/models/<model_type>) 없음 |
| `build_patch_pre` | `artifacts/build_patch_pre/80-exl3-quant-method.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_i5-032-sm12x.json#main) · 원장 관측 | `unknown` | ① 패치 헤더·본문에 대상 선언(model-trigger 아키텍처 · vllm/models/<model_type>) 없음 · 편집하는 공유(모델 디렉터리 밖) 파일 ['vllm/model_executor/layers/quantization/__init__.py'] |
| `build_patch_pre` | `artifacts/build_patch_pre/81-exl3-model-wiring.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_i5-032-sm12x.json#main) · 원장 관측 | `unknown` | ① 선언 ['models/deepseek_v41'] ∋ 셀 모델(architectures ['DeepseekV41ForCausalLM'] · model_type deepseek_v41) · ② 패치가 심는 logger 발화 서명 없음(발화 미관측) |
| `build_patch_pre` | `artifacts/build_patch_pre/82-exl3-h2d-stage.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_i5-032-sm12x.json#main) · 원장 관측 | `unknown` | ① 선언 ['models/deepseek_v41'] ∋ 셀 모델(architectures ['DeepseekV41ForCausalLM'] · model_type deepseek_v41) · ② 패치가 심는 logger 발화 서명 없음(발화 미관측) · 편집하는 공유(모델 디렉터리 밖) 파일 ['vllm/model_executor/layers/linear.py', 'vllm/model_executor/layers/vocab_parallel_embedding.py', 'vllm/model_executor/model_loader/weight_utils.py'] |
| `build_patch_pre` | `artifacts/build_patch_pre/83-engram-file-backend.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_i5-032-sm12x.json#main) · 원장 관측 | `unknown` | ① 선언 ['models/deepseek_v41'] ∋ 셀 모델(architectures ['DeepseekV41ForCausalLM'] · model_type deepseek_v41) · ② 엔진 로그 5개에서 서명 무발화(서명 3개 전부 0회) — 두 신호가 한 방향을 가리키지 않는다 |
| `build_patch_pre` | `artifacts/build_patch_pre/84-sm12x-block-shim.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_i5-032-sm12x.json#main) · 원장 관측 | `unknown` | ① 선언 ['models/deepseek_v41'] ∋ 셀 모델(architectures ['DeepseekV41ForCausalLM'] · model_type deepseek_v41) · ② 패치가 심는 logger 발화 서명 없음(발화 미관측) · 편집하는 공유(모델 디렉터리 밖) 파일 ['vllm/model_executor/layers/sparse_attn_indexer.py', 'vllm/v1/attention/backends/mla/indexer.py'] |
| `build_patch_pre` | `artifacts/build_patch_pre/85-exl3-serial-streams.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_i5-032-sm12x.json#main) · 원장 관측 | `unknown` | ① 선언 ['models/deepseek_v4', 'models/deepseek_v41'] ∋ 셀 모델(architectures ['DeepseekV41ForCausalLM'] · model_type deepseek_v41) · ② 패치가 심는 logger 발화 서명 없음(발화 미관측) |
| `build_patch_pre` | `artifacts/build_patch_pre/86-exl3-worker-hooks.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_i5-032-sm12x.json#main) · 원장 관측 | `unknown` | ① 패치 헤더·본문에 대상 선언(model-trigger 아키텍처 · vllm/models/<model_type>) 없음 · 편집하는 공유(모델 디렉터리 밖) 파일 ['vllm/v1/worker/gpu_worker.py'] |
| `build_patch_pre` | `artifacts/build_patch_pre/87-engram-embed-skip.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_i5-032-sm12x.json#main) · 원장 관측 | `unknown` | ① 선언 ['models/deepseek_v41'] ∋ 셀 모델(architectures ['DeepseekV41ForCausalLM'] · model_type deepseek_v41) · ② 패치가 심는 logger 발화 서명 없음(발화 미관측) · 편집하는 공유(모델 디렉터리 밖) 파일 ['vllm/model_executor/model_loader/ep_weight_filter.py'] |
| `build_patch_pre` | `artifacts/build_patch_pre/88-engram-prestage.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_i5-032-sm12x.json#main) · 원장 관측 | `unknown` | ① 선언 ['models/deepseek_v41'] ∋ 셀 모델(architectures ['DeepseekV41ForCausalLM'] · model_type deepseek_v41) · ② 패치가 심는 logger 발화 서명 없음(발화 미관측) · 편집하는 공유(모델 디렉터리 밖) 파일 ['vllm/v1/worker/gpu/model_runner.py'] |
| `build_patch_pre` | `artifacts/build_patch_pre/89-kv-null-block-zero.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_i5-032-sm12x.json#main) · 원장 관측 | `unknown` | ① 패치 헤더·본문에 대상 선언(model-trigger 아키텍처 · vllm/models/<model_type>) 없음 · 편집하는 공유(모델 디렉터리 밖) 파일 ['vllm/v1/worker/gpu/model_runner.py'] |
| `build_recipe` | `artifacts/build_recipe/Dockerfile.source-build-sm12x` | `generated-unverified` · renderer | 실린 Dockerfile.source-build-sm12x(worktree) · docker history RUN·COPY 9/31 일치 — Dockerfile 전체 원본 바이트의 동일성은 확인하지 못했다(_select_dockerfile · worktree(어느 개정도 이미지 history 와 더 맞지 않는다 · 작업트리 9/31 · 대조할 개정 없음(이미지 Created 이전 커밋 부재 · 비추적) · 전 이력(output/*/Dockerfile.source-build-sm12x · 모든 ref)에도 전수 일치 개정 없음 — recipe_vs_image 경고 유지)) | `unknown` | 파일의 실행 당시 바이트가 확인되지 않았다(generated-unverified · 생성 renderer) — 이 표시만으로 경로 실행 여부나 필요 여부를 판정하지 않는다(참고 형상) |
| `build_recipe` | `artifacts/build_recipe/parent-source-core/README.md` | `generated-unverified` · agent | main 최종 attempt2 동결 입력과 관측 원장에서 조립한 새 local 재현안이며 미실행 | `unknown` | 파일의 실행 당시 바이트가 확인되지 않았다(generated-unverified · 생성 agent) — 이 표시만으로 경로 실행 여부나 필요 여부를 판정하지 않는다(참고 형상) |
| `build_recipe` | `artifacts/build_recipe/parent-source-core/alias-evidence.json` | `verified` · cell-run | explicit manifest output/multi/parent-build/i5-sm12x032/manifest.json · selected image digest bound · frozen source sha256 verified · main historical scope only | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `build_recipe` | `artifacts/build_recipe/parent-source-core/context/Dockerfile.source-build` | `verified` · cell-run | explicit manifest output/multi/parent-build/i5-sm12x032/manifest.json · selected image digest bound · frozen source sha256 verified · main historical scope only | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `build_recipe` | `artifacts/build_recipe/parent-source-core/context/build_patches/.gitkeep` | `verified` · cell-run | explicit manifest output/multi/parent-build/i5-sm12x032/manifest.json · selected image digest bound · frozen source sha256 verified · main historical scope only | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `build_recipe` | `artifacts/build_recipe/parent-source-core/context/build_patches/05-ngc-source032-deps-compat.sh` | `verified` · cell-run | explicit manifest output/multi/parent-build/i5-sm12x032/manifest.json · selected image digest bound · frozen source sha256 verified · main historical scope only | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `build_recipe` | `artifacts/build_recipe/parent-source-core/context/build_patches_src/.gitkeep` | `verified` · cell-run | explicit manifest output/multi/parent-build/i5-sm12x032/manifest.json · selected image digest bound · frozen source sha256 verified · main historical scope only | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `build_recipe` | `artifacts/build_recipe/parent-source-core/context/requirements.txt` | `verified` · cell-run | explicit manifest output/multi/parent-build/i5-sm12x032/manifest.json · selected image digest bound · frozen source sha256 verified · main historical scope only | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `build_recipe` | `artifacts/build_recipe/parent-source-core/core-build-command.txt` | `verified` · cell-run | explicit manifest output/multi/parent-build/i5-sm12x032/manifest.json · selected image digest bound · frozen source sha256 verified · main historical scope only | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `build_recipe` | `artifacts/build_recipe/parent-source-core/intermediate-deps2/context/Dockerfile.source-build-deps2` | `verified` · cell-run | explicit manifest output/multi/parent-build/i5-sm12x032/manifest.json · selected image digest bound · frozen source sha256 verified · main historical scope only | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `build_recipe` | `artifacts/build_recipe/parent-source-core/intermediate-deps2/context/build_patches/06-b12x-optional032.sh` | `verified` · cell-run | explicit manifest output/multi/parent-build/i5-sm12x032/manifest.json · selected image digest bound · frozen source sha256 verified · main historical scope only | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `build_recipe` | `artifacts/build_recipe/parent-source-core/leaf-build-command.txt` | `verified` · cell-run | explicit manifest output/multi/parent-build/i5-sm12x032/manifest.json · selected image digest bound · frozen source sha256 verified · main historical scope only | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `build_recipe` | `artifacts/build_recipe/parent-source-core/leaf-ledger-evidence.json` | `verified` · cell-run | explicit manifest output/multi/parent-build/i5-sm12x032/manifest.json · selected image digest bound · frozen source sha256 verified · main historical scope only | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `build_recipe` | `artifacts/build_recipe/parent-source-core/leaf-result-evidence.json` | `verified` · cell-run | explicit manifest output/multi/parent-build/i5-sm12x032/manifest.json · selected image digest bound · frozen source sha256 verified · main historical scope only | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `build_recipe` | `artifacts/build_recipe/parent-source-core/leaf-support/build_patches/.gitkeep` | `verified` · cell-run | explicit manifest output/multi/parent-build/i5-sm12x032/manifest.json · selected image digest bound · frozen source sha256 verified · main historical scope only | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `build_recipe` | `artifacts/build_recipe/parent-source-core/leaf-support/build_patches/Dockerfile.source-build-deps2` | `verified` · cell-run | explicit manifest output/multi/parent-build/i5-sm12x032/manifest.json · selected image digest bound · frozen source sha256 verified · main historical scope only | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `build_recipe` | `artifacts/build_recipe/parent-source-core/leaf-support/build_patches_src/.gitkeep` | `verified` · cell-run | explicit manifest output/multi/parent-build/i5-sm12x032/manifest.json · selected image digest bound · frozen source sha256 verified · main historical scope only | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `build_recipe` | `artifacts/build_recipe/parent-source-core/leaf-support/build_patches_src/PROVENANCE.json` | `verified` · cell-run | explicit manifest output/multi/parent-build/i5-sm12x032/manifest.json · selected image digest bound · frozen source sha256 verified · main historical scope only | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `build_recipe` | `artifacts/build_recipe/parent-source-core/leaf-support/build_patches_src/exl3_port_lib.py` | `verified` · cell-run | explicit manifest output/multi/parent-build/i5-sm12x032/manifest.json · selected image digest bound · frozen source sha256 verified · main historical scope only | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `build_recipe` | `artifacts/build_recipe/parent-source-core/manifest.json` | `generated-unverified` · agent | main 최종 attempt2 동결 입력과 관측 원장에서 조립한 새 local 재현안이며 미실행 | `unknown` | 파일의 실행 당시 바이트가 확인되지 않았다(generated-unverified · 생성 agent) — 이 표시만으로 경로 실행 여부나 필요 여부를 판정하지 않는다(참고 형상) |
| `compose` | `artifacts/compose/.env.cluster.template` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/.env.interconnect.template` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/.env.template` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/docker-compose.yaml` | `generated-unverified` · renderer | docker-compose.yaml: worktree(mtime 2026-10-29T10:01:25Z ≤ 측정 2026-10-29T16:43:41Z(sweep_index.generated_utc) · 바이트 = 측정 전 마지막 커밋 bccd08091976) — 측정 당시 실행 바이트는 별도 관측되지 않았다 | `unknown` | 파일의 실행 당시 바이트가 확인되지 않았다(generated-unverified · 생성 renderer) — 이 표시만으로 경로 실행 여부나 필요 여부를 판정하지 않는다(참고 형상) |
| `compose` | `artifacts/compose/serve_runner.sh` | `generated-unverified` · renderer | serve_runner.sh: worktree(mtime 2026-09-06T21:57:53Z ≤ 측정 2026-10-29T16:43:41Z(sweep_index.generated_utc) · 바이트 = 측정 전 마지막 커밋 50a5ce486738) — 측정 당시 실행 바이트는 별도 관측되지 않았다 | `unknown` | 파일의 실행 당시 바이트가 확인되지 않았다(generated-unverified · 생성 renderer) — 이 표시만으로 경로 실행 여부나 필요 여부를 판정하지 않는다(참고 형상) |
| `compose` | `artifacts/compose/sub_recipe.json` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/.env.i5-032-sm12x` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) · ⚠ 측정(2026-10-29T16:43:41Z) 뒤 mtime: ['output/multi/envs/.env.i5-032-sm12x'] — 실린 값이 측정 당시 값과 같다는 보장 없음 | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/i5-032-sm12x.sh` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) · ⚠ 측정(2026-10-29T16:43:41Z) 뒤 mtime: ['output/multi/envs/.env.i5-032-sm12x'] — 실린 값이 측정 당시 값과 같다는 보장 없음 | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/i5-032-sm12x.yaml` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) · ⚠ 측정(2026-10-29T16:43:41Z) 뒤 mtime: ['output/multi/envs/.env.i5-032-sm12x'] — 실린 값이 측정 당시 값과 같다는 보장 없음 | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
<!-- /FACT:slots -->

<!-- FACT:applied_set -->
**적용 집합** — 상태 `observed` · 출처 `attestation:output/multi/benchlog/attestation_i5-032-sm12x.json#main`

| 단계 | 파일 | 결과 | 결과 출처 | 사유 | 실림 | model-trigger |
|---|---|---|---|---|---|---|
| pre | `80-exl3-quant-method.sh` | applied | build-ledger | lazy registration hook + chat template installed | 실림 | — |
| pre | `81-exl3-model-wiring.sh` | applied | build-ledger | 3 edits (3 newly applied) | 실림 | — |
| pre | `82-exl3-h2d-stage.sh` | applied | build-ledger | 14 new edits/footers | 실림 | — |
| pre | `83-engram-file-backend.sh` | applied | build-ledger | engram backend retargeted to common/engram.py | 실림 | — |
| pre | `84-sm12x-block-shim.sh` | applied | build-ledger | 6 new edits/footers | 실림 | — |
| pre | `85-exl3-serial-streams.sh` | applied | build-ledger | 2 new edits | 실림 | — |
| pre | `86-exl3-worker-hooks.sh` | applied | build-ledger | worker hooks footer installed | 실림 | — |
| pre | `87-engram-embed-skip.sh` | applied | build-ledger | engram embed skip installed | 실림 | — |
| pre | `88-engram-prestage.sh` | applied | build-ledger | 2 new edits (prestage v2: census+early) | 실림 | — |
| pre | `89-kv-null-block-zero.sh` | applied | build-ledger | 2 new edits | 실림 | — |
| post | `05-ngc-source032-deps-compat.sh` | applied | build-ledger | NGC26.07 source032 measured hub/pytest intersection reused; pip check passed | 실림 | generic shared runtime dependency consistency; no model weights. |
| post | `06-b12x-optional032.sh` | applied | build-ledger | Official vLLM0.32 optional B12X1.5.1 exact wheel; pip check passed; GPU kernels unverified | 실림 | Generic SM120/SM121 B12X optional backend; no model weights. |
| post | `70-exllamav3-build.sh` | applied | build-ledger | exllamav3 1.4.5 @ e648f1a13136 built for 12.1a | 실림 | — |
| post | `72-exl3-row-store.sh` | applied | build-ledger | librow_store.so built (/opt/exl3-port/librow_store.so) | 실림 | — |
| post | `73-vllm-exl3-build.sh` | applied | build-ledger | vllm-exl3 0.4.2 @ d3cfd3949203 + sfxnz p2b shapes/mrow/cfg1/codebook/fshift/srcsort/coop/dataflow @ 3de914682d08 built for 12.1a | 실림 | — |
| post | `74-exl3-moe-prep-fused.sh` | applied | build-ledger | sfxnz moe_prep_fused @ 3de914682d08 (runtime /opt/exl3-port/dsv41-patch) + exl3.py arming footer (DSV41_MOE_PREP_FUSED gate) | 실림 | — |
| post | `75-exl3-python-deps032.sh` | applied | build-ledger | FLA0.6.0/fla-core0.6.0/marisa1.4.1 closure; pinned wheel SHA | 실림 | — |
| post | `79-exl3-port-selfcheck.sh` | applied | build-ledger | self-check PASS (registry, classes, plugin edits, natives, engram hook + embed skip) | 실림 | — |

**결과 출처 범례**

- `build-ledger` — 이미지 안 빌드 원장이 적용 결과를 기록했다(관측)

**판정 근거**(패치별 — artifacts 가 판정에 쓴 관측 · 스크립트 바이트 정체 = 실린 바이트가 측정 이미지에 구워진 바이트인가)

| 파일 | 스크립트 바이트 정체 | 이미지 사본 sha256 | 적용 표지(상태 찾음/선언) | 정적 규칙 | 원장 분류 |
|---|---|---|---|---|---|
| `80-exl3-quant-method.sh` | build-ledger(원장 script_sha256 = 작업트리 바이트 — 실린 바이트 = 빌드가 실행한 바이트) | `19f70ed4dd0d98d6318209a1a987daec90515252397cecb33f21a5acb18a4e21`(빌드 원장 script_sha256 — 이미지 사본 아님) | — | — | status-file |
| `81-exl3-model-wiring.sh` | build-ledger(원장 script_sha256 = 작업트리 바이트 — 실린 바이트 = 빌드가 실행한 바이트) | `78d5606a73127488052cd1ffde5dbbe533cfeada1c2343095061f121d42f68b4`(빌드 원장 script_sha256 — 이미지 사본 아님) | — | — | status-file |
| `82-exl3-h2d-stage.sh` | build-ledger(원장 script_sha256 = 작업트리 바이트 — 실린 바이트 = 빌드가 실행한 바이트) | `373f1aa3fb7f80c921be8a793b85bf223b22520a0c5dbe19382e3fc15a8f6d4c`(빌드 원장 script_sha256 — 이미지 사본 아님) | — | — | status-file |
| `83-engram-file-backend.sh` | build-ledger(원장 script_sha256 = 작업트리 바이트 — 실린 바이트 = 빌드가 실행한 바이트) | `e65f4871384d970c6cb1833eebe24756e3449489523f85291bd25bea7ca51112`(빌드 원장 script_sha256 — 이미지 사본 아님) | — | — | status-file |
| `84-sm12x-block-shim.sh` | build-ledger(원장 script_sha256 = 작업트리 바이트 — 실린 바이트 = 빌드가 실행한 바이트) | `ce18151a93c9f9a6b101dc44efde8232aef5ed6a67c10c86c5fb2751b9dfc8ef`(빌드 원장 script_sha256 — 이미지 사본 아님) | — | — | status-file |
| `85-exl3-serial-streams.sh` | build-ledger(원장 script_sha256 = 작업트리 바이트 — 실린 바이트 = 빌드가 실행한 바이트) | `b6c5cadf73a667c892ce017edd33cdf7c56b0b3e4f4b44fc6b36feb5ded0147b`(빌드 원장 script_sha256 — 이미지 사본 아님) | — | — | status-file |
| `86-exl3-worker-hooks.sh` | build-ledger(원장 script_sha256 = 작업트리 바이트 — 실린 바이트 = 빌드가 실행한 바이트) | `f4b142ccb0360b887376306d9af1db78820b021aacf607b204346f4a4f9a8d58`(빌드 원장 script_sha256 — 이미지 사본 아님) | — | — | status-file |
| `87-engram-embed-skip.sh` | build-ledger(원장 script_sha256 = 작업트리 바이트 — 실린 바이트 = 빌드가 실행한 바이트) | `91c254f6bc88120d710ca02296d9e2faf352eae157080379b94b76d3fb79d39e`(빌드 원장 script_sha256 — 이미지 사본 아님) | — | — | status-file |
| `88-engram-prestage.sh` | build-ledger(원장 script_sha256 = 작업트리 바이트 — 실린 바이트 = 빌드가 실행한 바이트) | `185f6d23574158a812858240cfe55d8a6a60139377fd03466883ac4d42b88022`(빌드 원장 script_sha256 — 이미지 사본 아님) | — | — | status-file |
| `89-kv-null-block-zero.sh` | build-ledger(원장 script_sha256 = 작업트리 바이트 — 실린 바이트 = 빌드가 실행한 바이트) | `2ede1ae09b75a75af3580789ff632107514ffec5ee1f8ee166a9a51540209abe`(빌드 원장 script_sha256 — 이미지 사본 아님) | — | — | status-file |
| `05-ngc-source032-deps-compat.sh` | build-ledger(원장 script_sha256 = 작업트리 바이트 — 실린 바이트 = 빌드가 실행한 바이트) | `ef79a410b90cae256d9b520e95ad32e7480059a8f2ffce194810749e225dbc54`(빌드 원장 script_sha256 — 이미지 사본 아님) | — | — | status-file |
| `06-b12x-optional032.sh` | build-ledger(원장 script_sha256 = 작업트리 바이트 — 실린 바이트 = 빌드가 실행한 바이트) | `aebff978bb9bf4f06accc04b52ec872b66f96c11b3c1922032a4e2f0240bba15`(빌드 원장 script_sha256 — 이미지 사본 아님) | — | — | status-file |
| `70-exllamav3-build.sh` | build-ledger(원장 script_sha256 = 작업트리 바이트 — 실린 바이트 = 빌드가 실행한 바이트) | `e3ce003d2c2a69de56567f3b7c0d0ae0c4757e268255c393a139a415025342b6`(빌드 원장 script_sha256 — 이미지 사본 아님) | — | — | status-file |
| `72-exl3-row-store.sh` | build-ledger(원장 script_sha256 = 작업트리 바이트 — 실린 바이트 = 빌드가 실행한 바이트) | `110c360787c24033617519127e6f0d1373508288f1bb12c70b7d5d7251bf5e39`(빌드 원장 script_sha256 — 이미지 사본 아님) | — | — | status-file |
| `73-vllm-exl3-build.sh` | build-ledger(원장 script_sha256 = 작업트리 바이트 — 실린 바이트 = 빌드가 실행한 바이트) | `36fec33430a034392f6451be53557f0acba35b5c46dfe08729a530d729ab77e8`(빌드 원장 script_sha256 — 이미지 사본 아님) | — | — | status-file |
| `74-exl3-moe-prep-fused.sh` | build-ledger(원장 script_sha256 = 작업트리 바이트 — 실린 바이트 = 빌드가 실행한 바이트) | `cc290b5449c0a603bed4b3455b84e8bb9ae82c53579672f33ecbbc3e24886cfa`(빌드 원장 script_sha256 — 이미지 사본 아님) | — | — | status-file |
| `75-exl3-python-deps032.sh` | build-ledger(원장 script_sha256 = 작업트리 바이트 — 실린 바이트 = 빌드가 실행한 바이트) | `c1a3fc81d31d9b6be28f5c2c60c9f88b9ebca1055f0403149f5712e4755df89d`(빌드 원장 script_sha256 — 이미지 사본 아님) | — | — | status-file |
| `79-exl3-port-selfcheck.sh` | build-ledger(원장 script_sha256 = 작업트리 바이트 — 실린 바이트 = 빌드가 실행한 바이트) | `56227afebc5b1f22d864ee76dfd618cf6a54489e4a556cbf0a8cca9d9d7cb07c`(빌드 원장 script_sha256 — 이미지 사본 아님) | — | — | status-file |

**원장 탐침**(순위: attestation → 메인 로컬 이미지 → 재구성)

| 순위 | 결과 | 대상 |
|---|---|---|
| attestation | observed | output/multi/benchlog/attestation_i5-032-sm12x.json |

> **attestation 범위** — config `i5-032-sm12x` · phase serve · 작성 2026-10-29T16:31:49Z(output/multi/benchlog/attestation_i5-032-sm12x.json 파일 mtime(파일시스템 관측 — 문서에 시각 칸이 없다 · 기록 필드 아님)) · 묶은 근거 cell-name · 파일 `output/multi/benchlog/attestation_i5-032-sm12x.json` · 측정 창 대비 작성 2026-10-29T16:31:49Z < 측정 시작 2026-10-29T16:32:21Z — 측정 전(빌드·스모크)의 관측: **same config(i5-032-sm12x) · serve — 원장 순서로 판별한 같은 기동의 관측(결속된 스모크 로그 미확인 · 제한검색 · 약한 판별)(판별: 원장 순서로 판별(결속된 스모크 로그 미확인 · 제한검색 — docs/simlog/**/*serve*.log 에서 이 attestation 을 적은 로그 미확인; 다른 경로의 보존 여부는 판정하지 않음): 원장 docs/logs/main/events/2026-10.jsonl:840 budget_declare(smoke-i5-032-sm12x · 2026-10-29T16:10:33Z) ≤ attestation 작성 2026-10-29T16:31:49Z · serve_proof 작성 2026-10-29T16:31:49Z < 측정 시작 2026-10-29T16:32:21Z · 그 선언이 측정 끝 2026-10-29T16:43:41Z 까지 재선언 · 창 닫힘 없이 이어짐(main 원장 기록 범위 안) · API 서버 기동 2026-10-29T16:31:23Z(output/multi/benchlog/sweep_i5-032-sm12x_261030A/lite_engine_i5-032-sm12x.log:729 · 컨테이너 시계 = UTC 가정) ∈ [선언, attestation 작성] · 측정 엔진 로그 APIServer pid 788 하나)**
<!-- /FACT:applied_set -->

## 1.2 슬롯별 적용 사유
> 이 절이 답하는 질문: 실린 산출물 하나하나는 무엇을 고치며, 왜 이 셀에 필요했고, 이 셀에서 실제로 발화했으며, 언제 불필요해지는가?

### build_recipe/parent-source-core
maincore5파일context→POST05→sameIDdeps1alias→intermediatedeps2의Dockerfile+POST06→finalattempt2 child24파일을별도context로담았다. final24는기존19slot재사용+누락5support이며첫실패attempt나현재다른kit로치환하지않았다. core실제명령arch12.1/jobs8과child실제12.1a/jobs4를구분한다. nativePOST는75추가후실제로재컴파일됐고PRE층만cache였다. corecsrc/CMake/setup은불변이다.
selectedDockerfile은generated-unverified/renderer를유지한다. frozen원본문sha25fca737…와공식경고header가추가된배포본문shab93b29ea…는다른범위이며byte동일이라고하지않는다. 나머지23개는원문그대로이고표시전원본대조권위를보존했다. README/manifest의새조립명령은미실행generated-unverified이며실제core/leafargv및원장과다르다. sub과거원core미회수/독자수리역사는동일main경로로합치지않는다.

패치헤더의CPU후보표시는개발시점의범위다. 현재18개applied검증과native/TP2/Full근거를추가하되기존모델회귀가없어CANDIDATE지위는유지한다. 적용은런타임on이나고유필수조건의증명이아니다.

### 05-ngc-source032-deps-compat.sh
NGC datasets/vLLM의hub교집합1.33.0과pytest9.2.0을설치/pipcheck하는공유의존수리。
core단계에도있으며childPOST재고/적용과부모이력을각원장으로구분한다。
상류의존교집합변경시재도출;set-e/pipcheck실패종료。

### 06-b12x-optional032.sh
공식0.32 optional B12X1.5.1 exactwheel설치/API확인。
적용과GPU전체커널검증은별개이며옛operator1.2.3을이식성공으로쓰지않는다。
assert/pipcheck실패종료;공식optional핀변경시다시평가。

### 70-exllamav3-build.sh
pinnedexllamav3 1.4.5에Mia aarch64/fat-kernel을가공해native컴파일。
실제MCG bits2/384experts/hidden5120/intermediate_local1152가발화;expertBF16전체재구성없음。
상류동등SM12x지원흡수시재평가;SM12X_PORT0이면skip,1이면pin/helper/compile실패종료。

### 72-exl3-row-store.sh
Mia pinned row_store.cpp를librow_store.so로컴파일해file행조회제공。
rank별file-backedEngrammapping/loader skip와함께성립;모든cache메모리제거가아님。
정식filebackend가흡수되면재평가;SM12X_PORT0skip/필수helper부재및compile실패종료。

### 73-vllm-exl3-build.sh
pinnedplugin/p2b저자8편집과entrypoint/weightblock/lmhead/H2D를가공해설치。
EXL3registry·MCG·lmheadMXFP8/P2Bcooperative발화;POST75변경뒤실제다시컴파일。
공식EXL3지원/저자접점변경시재검토;SM12X_PORT0skip/부분적용또는anchor drift실패종료。

### 74-exl3-moe-prep-fused.sh
pinnedplugin의선택MoE입력준비fused모듈/footer를설치。
applied지만이번DSV41_MOE_PREP_FUSED0으로성능경로불활성;역사K6bit-exact를현재on성공으로쓰지않는다。
SM12X_PORT0skip;footer/모듈한쪽만있으면fail-loud;상류routing통합시필요성재평가。

### 75-exl3-python-deps032.sh
실제attempt1pipcheck결손의FLA0.6.0/fla-core0.6.0/marisa1.4.1을SHA고정wheel로보완。
finalattempt2의pipcheck0/import성공;vLLM/torch/CUDA/transformers를교체하지않음。
SM12X_PORT0skip;고정wheel/의존검사실패종료;상류의존폐포가바뀌면재도출。

### 79-exl3-port-selfcheck.sh
registry/classes/pluginedits/natives/Engramhook+embedskip의5항목compiled검사。
PASS/exit0이나post-exit weakref UnicodeDecodeError는보존;모델추론/Full은후속별도관측。
SM12X_PORT0skip/필수import·assert실패종료;상류접점변경시검사갱신。

### 80-exl3-quant-method.sh
공식0.32양자화registry/lazyimport에EXL3를등록하고chattemplate를설치。
현재quantizationexl3로드의진입경로(W1/W11),일반모델로의필요조건확장은아님。
SM12X_PORT0skip;정식등록중복/버전·anchor차이는fail-loud;상류EXL3지원흡수시걷어낼지재평가。

### 81-exl3-model-wiring.sh
LMheadctor/mapper/VLprefix3접점을새모델에배선。
lmheadMXFP8실관측,language-only셀이라vision전체기능검증아님。
SM12X_PORT0skip/anchor개수불일치실패;동등정식배선이있으면재평가。

### 82-exl3-h2d-stage.sh
loader/linear/vocab/model/dspark14접점의H2Dstage를이식。
로드는완주했으나이를빼본대조없어고유성능기여/필수성미확정。
SM12X_PORT0skip/편집anchor불일치실패;상류loadermemory동작변경시필요성재평가。

### 83-engram-file-backend.sh
Mia pinned backend8편집을commonParallelEngramEmbedding으로재타깃;새packed/shared속성설정。
실제rank별filemapping이며 use_thp=true 또는 dp_shared_memory=true를 거부한다. 지원 범위는 DP=1·dim=256·block_size=32이고 이 범위를 벗어나면 명시적으로 거부한다。
SM12X_PORT0skip/미지원옵션예외;정식동등filebackend흡수시재평가。

### 84-sm12x-block-shim.sh
main/indexercache·SWA64·indexerblock·FlashInfer새kv_cache_spec시그니처6편집。
이번FP8KV/블록64로실행;옛text-onlyvision/workspace4앵커는이식하지않음。
SM12X_PORT0skip/새anchor불일치실패;상류동등SM12x지원과정확접점을대조。

### 85-exl3-serial-streams.sh
model/MTPauxstream2접점의순차경로。
적용후실행은성립했으나off대조없어단독필수성/속도효과미확정。
SM12X_PORT0skip/편집일치검사실패종료;상류stream/API변경시다시판정。

### 86-exl3-worker-hooks.sh
workercompile/warmupfooter에cache반환/선택pretunehook。
현재로드/init완주,pretune심볼이없으면경고skip하는선택경로와기본hook을구분。
SM12X_PORT0skip/worker필수메서드drift실패;상류worker수명변경시재평가。

### 87-engram-embed-skip.sh
loaderweightfilter를commonbackendgate로연결해fileEngramembed텐서로드를건너뜀。
실제iterator skip관측;table_dir없는stock동작은selfcheck대조로보존。
SM12X_PORT0skip/편집중복불일치실패;공식동등skip경로가생기면재평가。

### 88-engram-prestage.sh
commonEngramoffload토글과V2prepare_inputs/lookback행조회를연결。
PRESTAGE1/VERIFY4/CENSUS0/EARLY0;TP2DP1·statelesshash/EngramBatch미사용가드,graph정확도전체검증아님。
SM12X_PORT0skip/예상밖EngramBatch/use_slot_cache·시그니처불일치명시중단;상류입력흐름변경시재저작。

### 89-kv-null-block-zero.sh
새runnercapture후nullblock0zero를접합。
applied지만현재eager라capture후발화는불활성;과거graphNaN해소만역사근거。
SM12X_PORT0skip/캡처anchor불일치실패;상류null초기화보장이동등하면재평가。

> [원문] artifacts/build_patch_pre/80-exl3-quant-method.sh §L9-14
> 
> : "${SM12X_PORT:=0}"
> if [ "${SM12X_PORT}" != "1" ]; then
>     echo "$TAG skip - SM12X_PORT=${SM12X_PORT} (stock 빌드 경로 불변)"
>     if [ -n "${EASY_VLLM_PATCH_STATUS:-}" ]; then printf 'skipped\tSM12X_PORT!=1\n' > "$EASY_VLLM_PATCH_STATUS"; fi
>     exit 0

### triplet/compose와싣지않은슬롯
실행yaml/sh/env형상은verified다. yaml은EXL3/fileEngram/FP8KV/eager/spec3/greedy/language-only를선언하고runner는no-prefix/no-async를명시한다. .env.template/cluster/interconnect/sub_recipe는verified이며docker-compose.yaml/serve_runner.sh는generated-unverified참고형상이다. 실제env관측을현재렌더형상으로덮지않는다. runtimepatch/forkpin은없고optional90~92는실리지않았다. helper/PROVENANCE는parentleaf-support에정식수집된script-modules재고로존재한다.

## 1.3 값의 지위표
> 서빙 설정의 노브 **전부**와 기계가 모은 후보 지위다(lockset 출처 · yaml 주석 · 엔진 기본값). 최종 지위는 아래 `kind: value-status` 블록이 정한다 — 표의 노브마다 블록이 정확히 하나 있어야 린터를 통과한다.

<!-- FACT:value_status -->
서빙 노브 21개 — 노브마다 `kind: value-status` 블록이 **정확히 1개** 있어야 한다(노브·값은 이 표의 글자 그대로 · 값 칸의 `\|` 는 `|` 로 적는다).

| 노브 | 값 | 후보 지위 | 후보 근거 | yaml 주석 | 출처 |
|---|---|---|---|---|---|
| `tensor-parallel-size` | 2 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `distributed-executor-backend` | ray | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `quantization` | exl3 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `tokenizer-mode` | deepseek_v41 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `hf-overrides` | {"engram_table_dir":"/app/ple_mmap/v41-engram-a"} | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `block-size` | 64 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `kernel-config` | {"sparse_indexer_topk_backend":"per_row"} | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `kv-cache-dtype` | fp8 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `kv-cache-memory-bytes` | 8589934592 | inherited | lockset 출처 kv_source=hand · lockset provenance=hand-authored | — | lockset.kv_source=hand |
| `gpu-memory-utilization` | 0.8 | inherited | lockset 출처 gmu_source=hand · lockset provenance=hand-authored | — | lockset.gmu_source=hand |
| `max-model-len` | 131072 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `max-num-seqs` | 1 | inherited | lockset 출처 batch_source=hand-lever · lockset provenance=hand-authored | — | lockset.batch_source=hand-lever |
| `max-num-batched-tokens` | 1536 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `enforce-eager` | true | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `no-enable-flashinfer-autotune` | true | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `reasoning-parser` | deepseek_v41 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `tool-call-parser` | deepseek_v41 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `enable-auto-tool-choice` | true | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `speculative-config` | {"method":"dspark","num_speculative_tokens":3,"draft_sample_method":"greedy"} | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `language-model-only` | true | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `engram-config` | {"cpu_offload": true, "dp_shared_memory": false, "use_thp": false} | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
<!-- /FACT:value_status -->

> 이 절이 답하는 질문: 각 서빙 노브의 값은 이 셀에서 어떤 지위인가 — 조정됐나, 승계됐나, 음성대조로 일부러 둔 것인가, 엔진 기본값인가, 필요조건인가?

```hint-event
id: V1
kind: value-status
노브: tensor-parallel-size
값: '2'
지위: inherited
근거: 현재0.32최소port안전형상의손레버/모델별접점선택;이셀값변경스윕·제거실패대조없음
출처: [testlog_26103005_i5_032_TP2와_Full_판정, devlog_26103005_i5_032_최소이식과_실측_인계]
```

```hint-event
id: V2
kind: value-status
노브: distributed-executor-backend
값: 'ray'
지위: inherited
근거: 현재0.32최소port안전형상의손레버/모델별접점선택;이셀값변경스윕·제거실패대조없음
출처: [testlog_26103005_i5_032_TP2와_Full_판정, devlog_26103005_i5_032_최소이식과_실측_인계]
```

```hint-event
id: V3
kind: value-status
노브: quantization
값: 'exl3'
지위: inherited
근거: 현재0.32최소port안전형상의손레버/모델별접점선택;이셀값변경스윕·제거실패대조없음
출처: [testlog_26103005_i5_032_TP2와_Full_판정, devlog_26103005_i5_032_최소이식과_실측_인계]
```

```hint-event
id: V4
kind: value-status
노브: tokenizer-mode
값: 'deepseek_v41'
지위: inherited
근거: 현재0.32최소port안전형상의손레버/모델별접점선택;이셀값변경스윕·제거실패대조없음
출처: [testlog_26103005_i5_032_TP2와_Full_판정, devlog_26103005_i5_032_최소이식과_실측_인계]
```

```hint-event
id: V5
kind: value-status
노브: hf-overrides
값: '{"engram_table_dir":"/app/ple_mmap/v41-engram-a"}'
지위: inherited
근거: 현재0.32최소port안전형상의손레버/모델별접점선택;이셀값변경스윕·제거실패대조없음
출처: [testlog_26103005_i5_032_TP2와_Full_판정, devlog_26103005_i5_032_최소이식과_실측_인계]
```

```hint-event
id: V6
kind: value-status
노브: block-size
값: '64'
지위: inherited
근거: 현재0.32최소port안전형상의손레버/모델별접점선택;이셀값변경스윕·제거실패대조없음
출처: [testlog_26103005_i5_032_TP2와_Full_판정, devlog_26103005_i5_032_최소이식과_실측_인계]
```

```hint-event
id: V7
kind: value-status
노브: kernel-config
값: '{"sparse_indexer_topk_backend":"per_row"}'
지위: inherited
근거: 현재0.32최소port안전형상의손레버/모델별접점선택;이셀값변경스윕·제거실패대조없음
출처: [testlog_26103005_i5_032_TP2와_Full_판정, devlog_26103005_i5_032_최소이식과_실측_인계]
```

```hint-event
id: V8
kind: value-status
노브: kv-cache-dtype
값: 'fp8'
지위: inherited
근거: 현재0.32최소port안전형상의손레버/모델별접점선택;이셀값변경스윕·제거실패대조없음
출처: [testlog_26103005_i5_032_TP2와_Full_판정, devlog_26103005_i5_032_최소이식과_실측_인계]
```

```hint-event
id: V9
kind: value-status
노브: kv-cache-memory-bytes
값: '8589934592'
지위: inherited
근거: 현재0.32최소port안전형상의손레버/모델별접점선택;이셀값변경스윕·제거실패대조없음
출처: [testlog_26103005_i5_032_TP2와_Full_판정, devlog_26103005_i5_032_최소이식과_실측_인계]
```

```hint-event
id: V10
kind: value-status
노브: gpu-memory-utilization
값: '0.8'
지위: inherited
근거: 현재0.32최소port안전형상의손레버/모델별접점선택;이셀값변경스윕·제거실패대조없음
출처: [testlog_26103005_i5_032_TP2와_Full_판정, devlog_26103005_i5_032_최소이식과_실측_인계]
```

```hint-event
id: V11
kind: value-status
노브: max-model-len
값: '131072'
지위: inherited
근거: 현재0.32최소port안전형상의손레버/모델별접점선택;이셀값변경스윕·제거실패대조없음
출처: [testlog_26103005_i5_032_TP2와_Full_판정, devlog_26103005_i5_032_최소이식과_실측_인계]
```

```hint-event
id: V12
kind: value-status
노브: max-num-seqs
값: '1'
지위: inherited
근거: 현재0.32최소port안전형상의손레버/모델별접점선택;이셀값변경스윕·제거실패대조없음
출처: [testlog_26103005_i5_032_TP2와_Full_판정, devlog_26103005_i5_032_최소이식과_실측_인계]
```

```hint-event
id: V13
kind: value-status
노브: max-num-batched-tokens
값: '1536'
지위: inherited
근거: 현재0.32최소port안전형상의손레버/모델별접점선택;이셀값변경스윕·제거실패대조없음
출처: [testlog_26103005_i5_032_TP2와_Full_판정, devlog_26103005_i5_032_최소이식과_실측_인계]
```

```hint-event
id: V14
kind: value-status
노브: enforce-eager
값: 'true'
지위: inherited
근거: 현재0.32최소port안전형상의손레버/모델별접점선택;이셀값변경스윕·제거실패대조없음
출처: [testlog_26103005_i5_032_TP2와_Full_판정, devlog_26103005_i5_032_최소이식과_실측_인계]
```

```hint-event
id: V15
kind: value-status
노브: no-enable-flashinfer-autotune
값: 'true'
지위: inherited
근거: 현재0.32최소port안전형상의손레버/모델별접점선택;이셀값변경스윕·제거실패대조없음
출처: [testlog_26103005_i5_032_TP2와_Full_판정, devlog_26103005_i5_032_최소이식과_실측_인계]
```

```hint-event
id: V16
kind: value-status
노브: reasoning-parser
값: 'deepseek_v41'
지위: inherited
근거: 현재0.32최소port안전형상의손레버/모델별접점선택;이셀값변경스윕·제거실패대조없음
출처: [testlog_26103005_i5_032_TP2와_Full_판정, devlog_26103005_i5_032_최소이식과_실측_인계]
```

```hint-event
id: V17
kind: value-status
노브: tool-call-parser
값: 'deepseek_v41'
지위: inherited
근거: 현재0.32최소port안전형상의손레버/모델별접점선택;이셀값변경스윕·제거실패대조없음
출처: [testlog_26103005_i5_032_TP2와_Full_판정, devlog_26103005_i5_032_최소이식과_실측_인계]
```

```hint-event
id: V18
kind: value-status
노브: enable-auto-tool-choice
값: 'true'
지위: inherited
근거: 현재0.32최소port안전형상의손레버/모델별접점선택;이셀값변경스윕·제거실패대조없음
출처: [testlog_26103005_i5_032_TP2와_Full_판정, devlog_26103005_i5_032_최소이식과_실측_인계]
```

```hint-event
id: V19
kind: value-status
노브: speculative-config
값: '{"method":"dspark","num_speculative_tokens":3,"draft_sample_method":"greedy"}'
지위: inherited
근거: 현재0.32최소port안전형상의손레버/모델별접점선택;이셀값변경스윕·제거실패대조없음
출처: [testlog_26103005_i5_032_TP2와_Full_판정, devlog_26103005_i5_032_최소이식과_실측_인계]
```

```hint-event
id: V20
kind: value-status
노브: language-model-only
값: 'true'
지위: inherited
근거: 현재0.32최소port안전형상의손레버/모델별접점선택;이셀값변경스윕·제거실패대조없음
출처: [testlog_26103005_i5_032_TP2와_Full_판정, devlog_26103005_i5_032_최소이식과_실측_인계]
```

```hint-event
id: V21
kind: value-status
노브: engram-config
값: '{"cpu_offload": true, "dp_shared_memory": false, "use_thp": false}'
지위: inherited
근거: 현재0.32최소port안전형상의손레버/모델별접점선택;이셀값변경스윕·제거실패대조없음
출처: [testlog_26103005_i5_032_TP2와_Full_판정, devlog_26103005_i5_032_최소이식과_실측_인계]
```

역사튜닝·필수성은현재0.32값의동일지위가아니다. KV는GPU당이며고정KV를GMU로다시조절하지않는다. cpu_offload와filetable/prestage는서로다른역할이며정확dir/지원가드를자기환경에서확인한다.

## 1.4 재현 절차
> 셀 실행 기록에서 기계가 파생한 명령 순서와 단계별 소요다. 소요가 `미관측` 인 단계는 시각 기록이 없다는 뜻이다(0 이 아니다). 그 아래는 블랙박스 이벤트 원장에서 이 셀의 행을 기계가 뽑아 기동 시도로 묶은 것이고, 마지막은 측정한 실행의 엔진 로그가 echo 한 env 다(측정 당시 값의 정본).

<!-- FACT:reproduce -->
| 순서 | 단계 | 성공 판정 | 소요 | 관측 구간(UTC) | 소요 출처 | 명령 출처 |
|---|---|---|---|---|---|---|
| 1 | render | 러너 3종·.env·.env.cluster·.env.interconnect 생성(렌더러 fail-loud) | 미관측 | 미관측 | .claude/skills/upstream-version-watch/scripts/multinode_serve_smoke.sh 선행조건 안내 · .claude/skills/upstream-version-watch/scripts/render_dockerfile.py CLI | derived(렌더러 CLI) |
| 2 | parent_source_core | generated-unverified: 새 양노드 local reproduction 예시 · 미실행 | 미관측 | 미관측 | Main official source+POST05 build -> same-ID deps1 alias -> observed deps2 POST06 -> final i5 attempt2: cached PRE layer, native POST rerun/recompiled including75. This new reconstruction was not built or GPU-tested. Sub normal result is accepted through its own documents, not reconstructed or revalidated here. | generated-unverified(explicit manifest output/multi/parent-build/i5-sm12x032/manifest.json) |
| 3 | alias | generated-unverified: 새 양노드 local reproduction 예시 · 미실행 | 미관측 | 미관측 | Main official source+POST05 build -> same-ID deps1 alias -> observed deps2 POST06 -> final i5 attempt2: cached PRE layer, native POST rerun/recompiled including75. This new reconstruction was not built or GPU-tested. Sub normal result is accepted through its own documents, not reconstructed or revalidated here. | generated-unverified(explicit manifest output/multi/parent-build/i5-sm12x032/manifest.json) |
| 4 | intermediate_deps2 | generated-unverified: explicit observed intermediate inputs; new local build not run | 미관측 | 미관측 | Main official source+POST05 build -> same-ID deps1 alias -> observed deps2 POST06 -> final i5 attempt2: cached PRE layer, native POST rerun/recompiled including75. This new reconstruction was not built or GPU-tested. Sub normal result is accepted through its own documents, not reconstructed or revalidated here. | generated-unverified(explicit manifest output/multi/parent-build/i5-sm12x032/manifest.json) |
| 5 | build | 빌드 OK · 이미지 실재 | 층 창 1시간 56분 45초(docker history 첫 층 → 끝 층 CreatedAt · 캐시 재사용 층 포함 — 빌드 소요 아님) | 2026-10-29T10:02:18Z → 2026-10-29T11:59:03Z (layer-window) | docker history --no-trunc --format '{{json .}}' sha256:436bd187022965d2e41f8969695d8446baeca6c5d50692d8b6db76ae2877a916 · 우리 Dockerfile 층 63개의 CreatedAt 첫 층 → 끝 층(캐시 재사용 층은 이전 빌드 시각 — 빌드 소요 아님) | generated-unverified(explicit manifest output/multi/parent-build/i5-sm12x032/manifest.json · leaf ledger args; full original CLI unavailable) |
| 6 | serve | health 200 + 추론 1회(스모크 판정) | ≤ 21분 48초(상한) | 2026-10-29T16:10:33Z → 2026-10-29T16:32:21Z (upper) | docs/logs/main/events/2026-10.jsonl:840 budget_declare(label smoke-i5-032-sm12x) · 관측된 예산 창 안의 측정  → 첫 측정 시각(output/multi/benchlog/sweep_i5-032-sm12x_261030A/lite_raw_i5-032-sm12x.json measured_utc(호스트 date -u · lite_bench.sh 가 cold 레그 직전에 찍는 측정 시작)) | derived(스모크 사용법) |
| 7 | bench | 리포트 발행(루브릭 권한 explicit ∧ PASS일 때만 full 인증서) | 11분 20초 | 2026-10-29T16:32:21Z → 2026-10-29T16:43:41Z (exact) | output/multi/benchlog/sweep_i5-032-sm12x_261030A/lite_raw_i5-032-sm12x.json measured_utc(호스트 date -u · lite_bench.sh 가 cold 레그 직전에 찍는 측정 시작) → sweep_index.generated_utc(호스트 UTC · 두 끝 같은 시계) | reconstructed(bench json + 측정 도구 원문 lite 인자) |

**빌드 층 시각**(관측 · 소요가 아니라 층 창 — 캐시 재사용 층은 이전 빌드의 시각을 가진다)

- `build` 이미지 층 CreatedAt(docker history): 2026-10-29T10:02:18Z → 2026-10-29T11:59:03Z · 층 63개 · 출처 `docker history --no-trunc --format '{{json .}}' sha256:436bd187022965d2e41f8969695d8446baeca6c5d50692d8b6db76ae2877a916` — 우리 Dockerfile 층의 CreatedAt 범위 — 캐시 재사용 층은 이전 빌드 시각을 가진다(소요 아님)

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

**4. intermediate_deps2**

```sh
# generated-unverified: reconstructed deps2 intermediate from observed parent ledger args
: "${REPRO_INTERMEDIATE_TAG:?set independent deps2 tag}"
docker build --build-arg BASE_IMAGE="$REPRO_DEPS1_TAG" --build-arg VLLM_REPO=https://github.com/vllm-project/vllm.git --build-arg VLLM_REF=5df63a7203cdb9328d824a27b682e77c7b9928be --build-arg SM12X_PORT=0 --build-arg SRC_DEPS_AUTHORITY=0 -t "$REPRO_INTERMEDIATE_TAG" -f parent-source-core/intermediate-deps2/context/Dockerfile.source-build-deps2 parent-source-core/intermediate-deps2/context
```

**5. build**

```sh
# generated-unverified: attempt2 source context assembled from existing slots plus five support files; selected Dockerfile may include its recorded warning
mkdir -p leaf-sm12x/context/build_patches leaf-sm12x/context/build_patches_src
cp Dockerfile.source-build-sm12x leaf-sm12x/context/Dockerfile
cp parent-source-core/leaf-support/build_patches/.gitkeep leaf-sm12x/context/build_patches/.gitkeep
cp ../build_patch_post/05-ngc-source032-deps-compat.sh leaf-sm12x/context/build_patches/05-ngc-source032-deps-compat.sh
cp ../build_patch_post/06-b12x-optional032.sh leaf-sm12x/context/build_patches/06-b12x-optional032.sh
cp ../build_patch_post/70-exllamav3-build.sh leaf-sm12x/context/build_patches/70-exllamav3-build.sh
cp ../build_patch_post/72-exl3-row-store.sh leaf-sm12x/context/build_patches/72-exl3-row-store.sh
cp ../build_patch_post/73-vllm-exl3-build.sh leaf-sm12x/context/build_patches/73-vllm-exl3-build.sh
cp ../build_patch_post/74-exl3-moe-prep-fused.sh leaf-sm12x/context/build_patches/74-exl3-moe-prep-fused.sh
cp ../build_patch_post/75-exl3-python-deps032.sh leaf-sm12x/context/build_patches/75-exl3-python-deps032.sh
cp ../build_patch_post/79-exl3-port-selfcheck.sh leaf-sm12x/context/build_patches/79-exl3-port-selfcheck.sh
cp parent-source-core/leaf-support/build_patches/Dockerfile.source-build-deps2 leaf-sm12x/context/build_patches/Dockerfile.source-build-deps2
cp parent-source-core/leaf-support/build_patches_src/.gitkeep leaf-sm12x/context/build_patches_src/.gitkeep
cp ../build_patch_pre/80-exl3-quant-method.sh leaf-sm12x/context/build_patches_src/80-exl3-quant-method.sh
cp ../build_patch_pre/81-exl3-model-wiring.sh leaf-sm12x/context/build_patches_src/81-exl3-model-wiring.sh
cp ../build_patch_pre/82-exl3-h2d-stage.sh leaf-sm12x/context/build_patches_src/82-exl3-h2d-stage.sh
cp ../build_patch_pre/83-engram-file-backend.sh leaf-sm12x/context/build_patches_src/83-engram-file-backend.sh
cp ../build_patch_pre/84-sm12x-block-shim.sh leaf-sm12x/context/build_patches_src/84-sm12x-block-shim.sh
cp ../build_patch_pre/85-exl3-serial-streams.sh leaf-sm12x/context/build_patches_src/85-exl3-serial-streams.sh
cp ../build_patch_pre/86-exl3-worker-hooks.sh leaf-sm12x/context/build_patches_src/86-exl3-worker-hooks.sh
cp ../build_patch_pre/87-engram-embed-skip.sh leaf-sm12x/context/build_patches_src/87-engram-embed-skip.sh
cp ../build_patch_pre/88-engram-prestage.sh leaf-sm12x/context/build_patches_src/88-engram-prestage.sh
cp ../build_patch_pre/89-kv-null-block-zero.sh leaf-sm12x/context/build_patches_src/89-kv-null-block-zero.sh
cp parent-source-core/leaf-support/build_patches_src/PROVENANCE.json leaf-sm12x/context/build_patches_src/PROVENANCE.json
cp parent-source-core/leaf-support/build_patches_src/exl3_port_lib.py leaf-sm12x/context/build_patches_src/exl3_port_lib.py
: "${REPRO_IMAGE_TAG:?set independent i5 leaf tag}"
docker build --build-arg BASE_IMAGE="$REPRO_INTERMEDIATE_TAG" --build-arg VLLM_REF=5df63a7203cdb9328d824a27b682e77c7b9928be --build-arg TORCH_CUDA_ARCH=12.1a --build-arg BUILD_JOBS=4 --build-arg SM12X_PORT=1 -t "$REPRO_IMAGE_TAG" -f leaf-sm12x/context/Dockerfile leaf-sm12x/context
```

**6. serve**

```sh
bash .claude/skills/upstream-version-watch/scripts/multinode_serve_smoke.sh i5-032-sm12x --keep-up
```

**7. bench**

```sh
# bench JSON 필드로 복원: backend · model_id→--model · tokenizer_id→--tokenizer · num_prompts · max_concurrency · request_rate · burstiness · 요청당 입출력 길이 = total_*_tokens ÷ completed(나누어떨어질 때만 — 평균이다 · 요청별 길이는 JSON 에 없다) · 나누어떨어지면 `--dataset-name random` 으로 추정했다(측정 도구가 그 꼴로 부른다 — 아래 도구 원문).
# JSON 에 없는 인자(--endpoint · --base-url · --ignore-eos · --num-warmups · --temperature · --seed · --random-range-ratio · --trust-remote-code)는 측정 도구가 정했다 — level_* = run_bench.sh · lite_cold/lite_warm = lite_bench.sh(.claude/skills/adversarial-benchmark/scripts/) · 셀 스윕 순서는 sweep_bench.sh.
# lite_cold/lite_warm 의 데이터셋 · 부하 인자 = 측정 시점 도구 원문 lite_bench.sh@d33b0878a6c3 L177-L183(리터럴 인자만 · `$변수` 인자는 도구가 값을 정하지 않아 JSON 필드로 채운다) — `total_*_tokens ÷ completed` 는 실측 토큰(chat 템플릿 포함)이지 인자가 아니다.
# output/multi/benchlog/sweep_i5-032-sm12x_261030A/lite_cold_i5-032-sm12x.json (date 20261029-163244) · 실측 토큰(chat 템플릿 포함 · total ÷ completed · 인자 아님): 입력 542/1 = 542 · 출력 128/1 = 128 · spec_decode_acceptance_length 2.16949(이 레그 JSON)
vllm bench serve --backend openai-chat --model deepseek-v4.1-flash-exl3 --tokenizer /app/quant_models/DeepSeek/DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz --trust-remote-code --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 --ignore-eos --num-prompts 1 --max-concurrency 1 --request-rate inf
# output/multi/benchlog/sweep_i5-032-sm12x_261030A/lite_warm_i5-032-sm12x.json (date 20261029-163319) · 실측 토큰(chat 템플릿 포함 · total ÷ completed · 인자 아님): 입력 1626/3 = 542 · 출력 384/3 = 128 · spec_decode_acceptance_length 1.84615(이 레그 JSON)
vllm bench serve --backend openai-chat --model deepseek-v4.1-flash-exl3 --tokenizer /app/quant_models/DeepSeek/DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz --trust-remote-code --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 --ignore-eos --num-prompts 3 --max-concurrency 1 --request-rate inf
```

**슬롯 → 재구성 예시 컨텍스트 매핑**(예시 위치 = `output/<토폴로지>/` · 원천 = 실린 Dockerfile 의 COPY · compose 설정; 실제 실행 경로는 별도 기록 확인) — 재구성 예시 통로 `leaf-sm12x/context/` · `parent-source-core/context/` · `parent-source-core/intermediate-deps2/context/`(실제 원본·후속층의 빌드 경로 증명 아님 · 별도 실행 기록 확인)

| zip 경로 | 빌드 컨텍스트 자리 | 근거 |
|---|---|---|
| `artifacts/triplet/.env.i5-032-sm12x` | `envs/.env.i5-032-sm12x` | compose env_file envs/.env.${CONFIG_FILE}(셀 env) |
| `artifacts/triplet/i5-032-sm12x.sh` | `configs/i5-032-sm12x.sh` | compose volumes ./configs:/app/configs |
| `artifacts/triplet/i5-032-sm12x.yaml` | `configs/i5-032-sm12x.yaml` | compose volumes ./configs:/app/configs |
| `artifacts/build_patch_pre/80-exl3-quant-method.sh` | `leaf-sm12x/context/build_patches_src/80-exl3-quant-method.sh` | raw slot equals explicit final frozen source before annotation; selected Dockerfile may carry the recorded generated-unverified warning; file_records.sha256 hashes shipped bytes; no duplicate artifact |
| `artifacts/build_patch_pre/81-exl3-model-wiring.sh` | `leaf-sm12x/context/build_patches_src/81-exl3-model-wiring.sh` | raw slot equals explicit final frozen source before annotation; selected Dockerfile may carry the recorded generated-unverified warning; file_records.sha256 hashes shipped bytes; no duplicate artifact |
| `artifacts/build_patch_pre/82-exl3-h2d-stage.sh` | `leaf-sm12x/context/build_patches_src/82-exl3-h2d-stage.sh` | raw slot equals explicit final frozen source before annotation; selected Dockerfile may carry the recorded generated-unverified warning; file_records.sha256 hashes shipped bytes; no duplicate artifact |
| `artifacts/build_patch_pre/83-engram-file-backend.sh` | `leaf-sm12x/context/build_patches_src/83-engram-file-backend.sh` | raw slot equals explicit final frozen source before annotation; selected Dockerfile may carry the recorded generated-unverified warning; file_records.sha256 hashes shipped bytes; no duplicate artifact |
| `artifacts/build_patch_pre/84-sm12x-block-shim.sh` | `leaf-sm12x/context/build_patches_src/84-sm12x-block-shim.sh` | raw slot equals explicit final frozen source before annotation; selected Dockerfile may carry the recorded generated-unverified warning; file_records.sha256 hashes shipped bytes; no duplicate artifact |
| `artifacts/build_patch_pre/85-exl3-serial-streams.sh` | `leaf-sm12x/context/build_patches_src/85-exl3-serial-streams.sh` | raw slot equals explicit final frozen source before annotation; selected Dockerfile may carry the recorded generated-unverified warning; file_records.sha256 hashes shipped bytes; no duplicate artifact |
| `artifacts/build_patch_pre/86-exl3-worker-hooks.sh` | `leaf-sm12x/context/build_patches_src/86-exl3-worker-hooks.sh` | raw slot equals explicit final frozen source before annotation; selected Dockerfile may carry the recorded generated-unverified warning; file_records.sha256 hashes shipped bytes; no duplicate artifact |
| `artifacts/build_patch_pre/87-engram-embed-skip.sh` | `leaf-sm12x/context/build_patches_src/87-engram-embed-skip.sh` | raw slot equals explicit final frozen source before annotation; selected Dockerfile may carry the recorded generated-unverified warning; file_records.sha256 hashes shipped bytes; no duplicate artifact |
| `artifacts/build_patch_pre/88-engram-prestage.sh` | `leaf-sm12x/context/build_patches_src/88-engram-prestage.sh` | raw slot equals explicit final frozen source before annotation; selected Dockerfile may carry the recorded generated-unverified warning; file_records.sha256 hashes shipped bytes; no duplicate artifact |
| `artifacts/build_patch_pre/89-kv-null-block-zero.sh` | `leaf-sm12x/context/build_patches_src/89-kv-null-block-zero.sh` | raw slot equals explicit final frozen source before annotation; selected Dockerfile may carry the recorded generated-unverified warning; file_records.sha256 hashes shipped bytes; no duplicate artifact |
| `artifacts/build_patch_post/05-ngc-source032-deps-compat.sh` | `leaf-sm12x/context/build_patches/05-ngc-source032-deps-compat.sh` | raw slot equals explicit final frozen source before annotation; selected Dockerfile may carry the recorded generated-unverified warning; file_records.sha256 hashes shipped bytes; no duplicate artifact |
| `artifacts/build_patch_post/06-b12x-optional032.sh` | `leaf-sm12x/context/build_patches/06-b12x-optional032.sh` | raw slot equals explicit final frozen source before annotation; selected Dockerfile may carry the recorded generated-unverified warning; file_records.sha256 hashes shipped bytes; no duplicate artifact |
| `artifacts/build_patch_post/70-exllamav3-build.sh` | `leaf-sm12x/context/build_patches/70-exllamav3-build.sh` | raw slot equals explicit final frozen source before annotation; selected Dockerfile may carry the recorded generated-unverified warning; file_records.sha256 hashes shipped bytes; no duplicate artifact |
| `artifacts/build_patch_post/72-exl3-row-store.sh` | `leaf-sm12x/context/build_patches/72-exl3-row-store.sh` | raw slot equals explicit final frozen source before annotation; selected Dockerfile may carry the recorded generated-unverified warning; file_records.sha256 hashes shipped bytes; no duplicate artifact |
| `artifacts/build_patch_post/73-vllm-exl3-build.sh` | `leaf-sm12x/context/build_patches/73-vllm-exl3-build.sh` | raw slot equals explicit final frozen source before annotation; selected Dockerfile may carry the recorded generated-unverified warning; file_records.sha256 hashes shipped bytes; no duplicate artifact |
| `artifacts/build_patch_post/74-exl3-moe-prep-fused.sh` | `leaf-sm12x/context/build_patches/74-exl3-moe-prep-fused.sh` | raw slot equals explicit final frozen source before annotation; selected Dockerfile may carry the recorded generated-unverified warning; file_records.sha256 hashes shipped bytes; no duplicate artifact |
| `artifacts/build_patch_post/75-exl3-python-deps032.sh` | `leaf-sm12x/context/build_patches/75-exl3-python-deps032.sh` | raw slot equals explicit final frozen source before annotation; selected Dockerfile may carry the recorded generated-unverified warning; file_records.sha256 hashes shipped bytes; no duplicate artifact |
| `artifacts/build_patch_post/79-exl3-port-selfcheck.sh` | `leaf-sm12x/context/build_patches/79-exl3-port-selfcheck.sh` | raw slot equals explicit final frozen source before annotation; selected Dockerfile may carry the recorded generated-unverified warning; file_records.sha256 hashes shipped bytes; no duplicate artifact |
| `artifacts/build_recipe/Dockerfile.source-build-sm12x` | `leaf-sm12x/context/Dockerfile` | raw slot equals explicit final frozen source before annotation; selected Dockerfile may carry the recorded generated-unverified warning; file_records.sha256 hashes shipped bytes; no duplicate artifact |
| `artifacts/build_recipe/parent-source-core/README.md` | — | explicit selected-image-bound parent manifest; main frozen context only; metadata is not build context |
| `artifacts/build_recipe/parent-source-core/alias-evidence.json` | — | explicit selected-image-bound parent manifest; main frozen context only; metadata is not build context |
| `artifacts/build_recipe/parent-source-core/context/Dockerfile.source-build` | `parent-source-core/context/Dockerfile.source-build` | explicit selected-image-bound parent manifest; main frozen context only; metadata is not build context |
| `artifacts/build_recipe/parent-source-core/context/build_patches/.gitkeep` | `parent-source-core/context/build_patches/.gitkeep` | explicit selected-image-bound parent manifest; main frozen context only; metadata is not build context |
| `artifacts/build_recipe/parent-source-core/context/build_patches/05-ngc-source032-deps-compat.sh` | `parent-source-core/context/build_patches/05-ngc-source032-deps-compat.sh` | explicit selected-image-bound parent manifest; main frozen context only; metadata is not build context |
| `artifacts/build_recipe/parent-source-core/context/build_patches_src/.gitkeep` | `parent-source-core/context/build_patches_src/.gitkeep` | explicit selected-image-bound parent manifest; main frozen context only; metadata is not build context |
| `artifacts/build_recipe/parent-source-core/context/requirements.txt` | `parent-source-core/context/requirements.txt` | explicit selected-image-bound parent manifest; main frozen context only; metadata is not build context |
| `artifacts/build_recipe/parent-source-core/core-build-command.txt` | — | explicit selected-image-bound parent manifest; main frozen context only; metadata is not build context |
| `artifacts/build_recipe/parent-source-core/intermediate-deps2/context/Dockerfile.source-build-deps2` | `parent-source-core/intermediate-deps2/context/Dockerfile.source-build-deps2` | explicit selected-image-bound parent manifest; main frozen context only; metadata is not build context |
| `artifacts/build_recipe/parent-source-core/intermediate-deps2/context/build_patches/06-b12x-optional032.sh` | `parent-source-core/intermediate-deps2/context/build_patches/06-b12x-optional032.sh` | explicit selected-image-bound parent manifest; main frozen context only; metadata is not build context |
| `artifacts/build_recipe/parent-source-core/leaf-build-command.txt` | — | explicit selected-image-bound parent manifest; main frozen context only; metadata is not build context |
| `artifacts/build_recipe/parent-source-core/leaf-ledger-evidence.json` | — | explicit selected-image-bound parent manifest; main frozen context only; metadata is not build context |
| `artifacts/build_recipe/parent-source-core/leaf-result-evidence.json` | — | explicit selected-image-bound parent manifest; main frozen context only; metadata is not build context |
| `artifacts/build_recipe/parent-source-core/leaf-support/build_patches/.gitkeep` | `leaf-sm12x/context/build_patches/.gitkeep` | explicit selected-image-bound parent manifest; main frozen context only; metadata is not build context |
| `artifacts/build_recipe/parent-source-core/leaf-support/build_patches/Dockerfile.source-build-deps2` | `leaf-sm12x/context/build_patches/Dockerfile.source-build-deps2` | explicit selected-image-bound parent manifest; main frozen context only; metadata is not build context |
| `artifacts/build_recipe/parent-source-core/leaf-support/build_patches_src/.gitkeep` | `leaf-sm12x/context/build_patches_src/.gitkeep` | explicit selected-image-bound parent manifest; main frozen context only; metadata is not build context |
| `artifacts/build_recipe/parent-source-core/leaf-support/build_patches_src/PROVENANCE.json` | `leaf-sm12x/context/build_patches_src/PROVENANCE.json` | explicit selected-image-bound parent manifest; main frozen context only; metadata is not build context |
| `artifacts/build_recipe/parent-source-core/leaf-support/build_patches_src/exl3_port_lib.py` | `leaf-sm12x/context/build_patches_src/exl3_port_lib.py` | explicit selected-image-bound parent manifest; main frozen context only; metadata is not build context |
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

**기동 성공 판정(관측 · 발행 자격)**: health 200 = true · 추론 1회 = true · 근거 `output/multi/benchlog/serve_proof_i5-032-sm12x.json` — 시점 범위: 작성 2026-10-29T16:31:49Z(output/multi/benchlog/serve_proof_i5-032-sm12x.json 파일 mtime(파일시스템 관측 — 문서에 시각 칸이 없다 · 기록 필드 아님)) · 측정 창 대비 작성 2026-10-29T16:31:49Z < 측정 시작 2026-10-29T16:32:21Z — 측정 전(빌드·스모크)의 관측
<!-- /FACT:reproduce -->

<!-- FACT:event_timeline -->
**블랙박스 이벤트 22행**(원장에서 이 셀 label·config 에 맞는 행만 · 시각순 · 기계 발췌)

> **캠페인 경계** 2026-10-29T09:57:19Z(`camp-26102919-codex-multi4-v032` · evidence.event_campaign_boundary): campaigns/camp-26102919-codex-multi4-v032/campaign.yaml declared_utc 2026-10-29T09:57:19Z · 이 발행이 묶인 측정을 서빙한 budget_declare 창 시작 2026-10-29T16:10:33Z → 경계 = 앞선 값 2026-10-29T09:57:19Z(이전 행 = 같은 셀 이름 · 앞선 캠페인)

> **원장 관측 범위**(노드별 첫 행 → 마지막 행 · evidence.event_ledger_spans): main = 2026-07-31T03:27:43Z → 2026-10-29T20:28:30Z(`docs/logs/main/events/2026-07.jsonl` · `docs/logs/main/events/2026-08.jsonl` · `docs/logs/main/events/2026-09.jsonl` · `docs/logs/main/events/2026-10.jsonl` · `docs/logs/main/events/thermal.jsonl` · `docs/logs/main/events/watchdog.jsonl`) · sub = 2026-06-13T22:25:01Z → 2026-10-29T14:09:01Z(`sync_staging/sub_docs/logs/sub/events/2026-06.jsonl` · `sync_staging/sub_docs/logs/sub/events/2026-07.jsonl` · `sync_staging/sub_docs/logs/sub/events/2026-08.jsonl` · `sync_staging/sub_docs/logs/sub/events/2026-09.jsonl` · `sync_staging/sub_docs/logs/sub/events/2026-10.jsonl` · `sync_staging/sub_docs/logs/sub/events/thermal.jsonl` · `sync_staging/sub_docs/logs/sub/events/watchdog.jsonl` · 회수 미러가 가린 옛 로컬 사본 `docs/logs/sub/events/2026-09.jsonl`) — ⚠ sub(마지막 행 2026-10-29T14:09:01Z < 측정 끝 2026-10-29T16:43:41Z): 그 뒤 그 노드의 선언 · 사건은 **관측 범위 밖**이다(없음이 아니다 · 원장 미회수)

| # | UTC | 노드 | kind | label | 내용 | 출처 |
|---|---|---|---|---|---|---|
| 1 | 2026-10-29T16:10:33Z | main | `budget_declare` | `smoke-i5-032-sm12x` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=17846 · weights_mib=73770 · kv_mib=8192 · overhead_mib=24800 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 기동 창 1/1: budget_renew 9회 · 닫힘 budget_clear 2026-10-29T20:28:29Z(선언 후 4h17m56s) · measured_utc 2026-10-29T16:43:41Z 가 이 창 안(이 측정의 기동) | docs/logs/main/events/2026-10.jsonl:840 |
| 2 | 2026-10-29T16:10:33Z | main | `budget_honored` | `smoke-i5-032-sm12x` | budget_honored · floor_mib=17846 · arm_ceiling_mib=14774 · remaining_s=10800 · 창 시작 2026-10-29T16:10:33Z 후 0m00s | docs/logs/main/events/watchdog.jsonl:872 |
| 3 | 2026-10-29T16:11:47Z | cluster | `engine_log_window_start` | — | 캡처된 엔진 로그의 첫 시각(꼬리 캡처일 수 있다 — 엔진 시작 시각이 아닐 수 있다) · 선언 2026-10-29T16:10:33Z 후 1m14s · 시계: 컨테이너 시각을 UTC 로 읽어 측정 창 [2026-10-29T16:10:33Z, 2026-10-29T20:28:29Z] 안에 드는 것을 확인 | output/multi/benchlog/sweep_i5-032-sm12x_261030A/lite_engine_i5-032-sm12x.log:308 |
| 4 | 2026-10-29T16:11:51Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=52419 · rate_mib_s=25989 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → smoke-i5-032-sm12x 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T16:10:33Z 후 1m18s) | docs/logs/main/events/watchdog.jsonl:873 |
| 5 | 2026-10-29T16:11:53Z | cluster | `engine_prefetch_disabled` | — | 가중치 auto-prefetch 꺼짐 줄 ×4 — filesystem CIFS(vLLM 로그 문구) | output/multi/benchlog/sweep_i5-032-sm12x_261030A/lite_engine_i5-032-sm12x.log:527 |
| 6 | 2026-10-29T16:28:12Z | cluster | `engine_weights_loaded` | — | 가중치 적재 완료 줄 ×4(rank·단계별) · 최장 638.29s · 이 행 = 마지막 줄 | output/multi/benchlog/sweep_i5-032-sm12x_261030A/lite_engine_i5-032-sm12x.log:639 |
| 7 | 2026-10-29T16:28:29Z | cluster | `engine_model_loaded` | — | 모델 적재 완료 줄 ×2(rank 별) · 마지막 줄 73.96 GiB · 1001.5s | output/multi/benchlog/sweep_i5-032-sm12x_261030A/lite_engine_i5-032-sm12x.log:650 |
| 8 | 2026-10-29T16:29:31Z | cluster | `engine_shm_broadcast_wait` | — | shm_broadcast 대기 경고(60s 단위) ×1 · 2026-10-29T16:29:31Z–2026-10-29T16:29:31Z(0m00s) — vLLM 문구상 프로세스 hang 또는 긴 작업(컴파일·가중치/KV 양자화) 중 출현 | output/multi/benchlog/sweep_i5-032-sm12x_261030A/lite_engine_i5-032-sm12x.log:657 |
| 9 | 2026-10-29T16:30:24Z | cluster | `engine_kv_cache_sized` | — | GPU KV cache 2,341,696 tokens · 요청당 131,072 tokens 기준 최대 동시성 17.87x | output/multi/benchlog/sweep_i5-032-sm12x_261030A/lite_engine_i5-032-sm12x.log:688 |
| 10 | 2026-10-29T16:31:06Z | cluster | `engine_init_done` | — | 엔진 초기화(profile·KV 할당·warmup) 156.29s | output/multi/benchlog/sweep_i5-032-sm12x_261030A/lite_engine_i5-032-sm12x.log:718 |
| 11 | 2026-10-29T16:31:23Z | cluster | `server_starting` | — | API 서버 기동 — 이 측정 창 선언 2026-10-29T16:10:33Z 후 20m50s · 캡처 로그 첫 시각 2026-10-29T16:11:47Z 후 19m36s | output/multi/benchlog/sweep_i5-032-sm12x_261030A/lite_engine_i5-032-sm12x.log:729 |
| 12 | 2026-10-29T16:31:49Z | main | `budget_renew` | `smoke-i5-032-sm12x` | budget_renew · floor_mib=17846 · ttl_s=10800 · remaining_before_s=9524 · 창 시작 2026-10-29T16:10:33Z 후 21m16s | docs/logs/main/events/2026-10.jsonl:841 |
| 13 | 2026-10-29T16:33:22Z | main | `budget_renew` | `smoke-i5-032-sm12x` | budget_renew · floor_mib=17846 · ttl_s=7200 · remaining_before_s=10707 · 창 시작 2026-10-29T16:10:33Z 후 22m49s | docs/logs/main/events/2026-10.jsonl:842 |
| 14 | 2026-10-29T16:36:48Z | main | `budget_renew` | `smoke-i5-032-sm12x` | budget_renew · floor_mib=17846 · ttl_s=7200 · remaining_before_s=6994 · 창 시작 2026-10-29T16:10:33Z 후 26m15s | docs/logs/main/events/2026-10.jsonl:843 |
| 15 | 2026-10-29T16:40:18Z | main | `budget_renew` | `smoke-i5-032-sm12x` | budget_renew · floor_mib=17846 · ttl_s=7200 · remaining_before_s=6990 · 창 시작 2026-10-29T16:10:33Z 후 29m45s | docs/logs/main/events/2026-10.jsonl:844 |
| 16 | 2026-10-29T17:16:49Z | main | `budget_renew` | `smoke-i5-032-sm12x` | budget_renew · floor_mib=17846 · ttl_s=10800 · remaining_before_s=5009 · 창 시작 2026-10-29T16:10:33Z 후 1h06m16s | docs/logs/main/events/2026-10.jsonl:845 |
| 17 | 2026-10-29T18:01:49Z | main | `budget_renew` | `smoke-i5-032-sm12x` | budget_renew · floor_mib=17846 · ttl_s=10800 · remaining_before_s=8100 · 창 시작 2026-10-29T16:10:33Z 후 1h51m16s | docs/logs/main/events/2026-10.jsonl:846 |
| 18 | 2026-10-29T18:46:49Z | main | `budget_renew` | `smoke-i5-032-sm12x` | budget_renew · floor_mib=17846 · ttl_s=10800 · remaining_before_s=8100 · 창 시작 2026-10-29T16:10:33Z 후 2h36m16s | docs/logs/main/events/2026-10.jsonl:847 |
| 19 | 2026-10-29T19:31:50Z | main | `budget_renew` | `smoke-i5-032-sm12x` | budget_renew · floor_mib=17846 · ttl_s=10800 · remaining_before_s=8099 · 창 시작 2026-10-29T16:10:33Z 후 3h21m17s | docs/logs/main/events/2026-10.jsonl:848 |
| 20 | 2026-10-29T20:16:50Z | main | `budget_renew` | `smoke-i5-032-sm12x` | budget_renew · floor_mib=17846 · ttl_s=10800 · remaining_before_s=8100 · 창 시작 2026-10-29T16:10:33Z 후 4h06m17s | docs/logs/main/events/2026-10.jsonl:849 |
| 21 | 2026-10-29T20:28:29Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-i5-032-sm12x 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T16:10:33Z 후 4h17m56s) | docs/logs/main/events/2026-10.jsonl:850 |
| 22 | 2026-10-29T20:28:30Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-i5-032-sm12x 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T16:10:33Z 후 4h17m57s) | docs/logs/main/events/watchdog.jsonl:874 |

**기동 시도**(예산 선언 `budget_declare` 마다 한 시도 · 같은 노드의 이어지는 행과 선언 없는 노드(엔진 로그)의 그 시각 행을 묶었다 · 사살 · 트립 · 거부는 닫힘과 따로 센다 · `measurement` 행은 측정 기록이라 시도로 세지 않는다(위 표에만) — 해석은 저작자 몫)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 1 | main | `smoke-i5-032-sm12x` | 2026-10-29T16:10:33Z | 9회 · 첫 2026-10-29T16:31:49Z | 없음 | `budget_clear` 2026-10-29T20:28:29Z | 11 |
<!-- /FACT:event_timeline -->

<!-- FACT:measurement_env -->
**측정 실행의 env 관측 39행**(측정한 실행의 엔진 로그가 echo 한 값 · 기계 발췌 — 측정 당시 값의 정본이다. 표에 없는 키는 로그가 echo 하지 않은 것이지 설정되지 않았다는 뜻이 아니다 · 측정 뒤 재생성된 env 형상은 01 §1.1 을 본다) — 그중 4행은 종류 `key-present`: 키가 프로세스 env 에 **있었다**는 관측뿐이고 값은 로그에 없다(값 미관측)

| # | 키 | 값 | 노드 | 종류 | 출처(첫 줄) | 관측 |
|---|---|---|---|---|---|---|
| 1 | `VLLM_PLE_MMAP` | (키 존재 · 값 미관측) | 미특정 | key-present | output/multi/benchlog/sweep_i5-032-sm12x_261030A/lite_engine_head_i5-032-sm12x.log:L64 | 로그 1개 |
| 2 | `VLLM_PLE_MMAP_DIR` | (키 존재 · 값 미관측) | 미특정 | key-present | output/multi/benchlog/sweep_i5-032-sm12x_261030A/lite_engine_head_i5-032-sm12x.log:L65 | 로그 1개 |
| 3 | `VLLM_REF` | (키 존재 · 값 미관측) | 미특정 | key-present | output/multi/benchlog/sweep_i5-032-sm12x_261030A/lite_engine_head_i5-032-sm12x.log:L66 | 로그 1개 |
| 4 | `VLLM_EXL3_MOE_KERNEL` | (키 존재 · 값 미관측) | 미특정 | key-present | output/multi/benchlog/sweep_i5-032-sm12x_261030A/lite_engine_head_i5-032-sm12x.log:L67 | 로그 1개 |
| 5 | `NCCL_SOCKET_IFNAME` | `<nic:cluster>` | sub | env-echo | output/multi/benchlog/sweep_i5-032-sm12x_261030A/lite_engine_head_i5-032-sm12x.log:L104 | 로그 1개 |
| 6 | `NCCL_SOCKET_IFNAME` | `<nic:cluster>` | sub | param-echo | output/multi/benchlog/sweep_i5-032-sm12x_261030A/lite_engine_head_i5-032-sm12x.log:L105 | 로그 1개 |
| 7 | `nccl:version` | `2.30.7+cuda13.3` | sub | runtime | output/multi/benchlog/sweep_i5-032-sm12x_261030A/lite_engine_head_i5-032-sm12x.log:L106 | 로그 1개 |
| 8 | `NCCL_NET_PLUGIN` | `spcx` | sub | env-echo | output/multi/benchlog/sweep_i5-032-sm12x_261030A/lite_engine_head_i5-032-sm12x.log:L108 | 로그 1개 |
| 9 | `NCCL_IB_DISABLE` | `0` | sub | env-echo | output/multi/benchlog/sweep_i5-032-sm12x_261030A/lite_engine_head_i5-032-sm12x.log:L113 | 로그 1개 |
| 10 | `NCCL_IB_HCA` | `=<nic:cluster>,<nic:cluster>` | sub | param-echo | output/multi/benchlog/sweep_i5-032-sm12x_261030A/lite_engine_head_i5-032-sm12x.log:L115 | 로그 1개 |
| 11 | `NCCL_IB_MERGE_NICS` | `1` | sub | env-echo | output/multi/benchlog/sweep_i5-032-sm12x_261030A/lite_engine_head_i5-032-sm12x.log:L118 | 로그 1개 |
| 12 | `nccl:net_ib_devices` | `[0]<nic:cluster>:1/RoCE [1]<nic:cluster>:1/RoCE [RO]; OOB <nic:cluster>:<node:sub><0>` | sub | runtime | output/multi/benchlog/sweep_i5-032-sm12x_261030A/lite_engine_head_i5-032-sm12x.log:L121 | 로그 1개 |
| 13 | `NCCL_IB_QPS_PER_CONNECTION` | `4` | sub | env-echo | output/multi/benchlog/sweep_i5-032-sm12x_261030A/lite_engine_head_i5-032-sm12x.log:L134 | 로그 2개 |
| 14 | `nccl:network` | `IB` | main | runtime | output/multi/benchlog/sweep_i5-032-sm12x_261030A/lite_engine_head_i5-032-sm12x.log:L159 | 로그 1개 |
| 15 | `NCCL_DMABUF_ENABLE` | `0` | main | env-echo | output/multi/benchlog/sweep_i5-032-sm12x_261030A/lite_engine_head_i5-032-sm12x.log:L161 | 로그 1개 |
| 16 | `NCCL_CROSS_NIC` | `1` | main | env-echo | output/multi/benchlog/sweep_i5-032-sm12x_261030A/lite_engine_head_i5-032-sm12x.log:L164 | 로그 1개 |
| 17 | `NCCL_MAX_NCHANNELS` | `8` | main | env-echo | output/multi/benchlog/sweep_i5-032-sm12x_261030A/lite_engine_head_i5-032-sm12x.log:L337 | 로그 2개 |
| 18 | `NCCL_LL128_BUFFSIZE` | `262144` | main | env-echo | output/multi/benchlog/sweep_i5-032-sm12x_261030A/lite_engine_head_i5-032-sm12x.log:L355 | 로그 2개 |
| 19 | `NCCL_BUFFSIZE` | `1048576` | main | env-echo | output/multi/benchlog/sweep_i5-032-sm12x_261030A/lite_engine_head_i5-032-sm12x.log:L356 | 로그 2개 |
| 20 | `NCCL_PROTO` | `^LL128` | main | env-echo | output/multi/benchlog/sweep_i5-032-sm12x_261030A/lite_engine_head_i5-032-sm12x.log:L375 | 로그 2개 |
| 21 | `NCCL_IB_SPLIT_DATA_ON_QPS` | `0` | main | env-echo | output/multi/benchlog/sweep_i5-032-sm12x_261030A/lite_engine_head_i5-032-sm12x.log:L408 | 로그 2개 |
| 22 | `NCCL_IB_GID_INDEX` | `3` | sub | env-echo | output/multi/benchlog/sweep_i5-032-sm12x_261030A/lite_engine_head_i5-032-sm12x.log:L427 | 로그 2개 |
| 23 | `NCCL_SOCKET_IFNAME` | `<nic:cluster>` | main | env-echo | output/multi/benchlog/sweep_i5-032-sm12x_261030A/lite_engine_head_i5-032-sm12x.log:L542 | 로그 2개 · Ray 접힘 +3(접힌 사본의 노드 미관측) |
| 24 | `NCCL_SOCKET_IFNAME` | `<nic:cluster>` | main | param-echo | output/multi/benchlog/sweep_i5-032-sm12x_261030A/lite_engine_head_i5-032-sm12x.log:L543 | 로그 2개 · Ray 접힘 +2(접힌 사본의 노드 미관측) |
| 25 | `nccl:version` | `2.30.7+cuda13.3` | main | runtime | output/multi/benchlog/sweep_i5-032-sm12x_261030A/lite_engine_head_i5-032-sm12x.log:L544 | 로그 2개 |
| 26 | `NCCL_NET_PLUGIN` | `spcx` | main | env-echo | output/multi/benchlog/sweep_i5-032-sm12x_261030A/lite_engine_head_i5-032-sm12x.log:L546 | 로그 2개 |
| 27 | `NCCL_IB_DISABLE` | `0` | main | env-echo | output/multi/benchlog/sweep_i5-032-sm12x_261030A/lite_engine_head_i5-032-sm12x.log:L551 | 로그 2개 · Ray 접힘 +2(접힌 사본의 노드 미관측) |
| 28 | `NCCL_IB_HCA` | `=<nic:cluster>,<nic:cluster>` | main | param-echo | output/multi/benchlog/sweep_i5-032-sm12x_261030A/lite_engine_head_i5-032-sm12x.log:L552 | 로그 2개 · Ray 접힘 +2(접힌 사본의 노드 미관측) |
| 29 | `NCCL_IB_MERGE_NICS` | `1` | main | env-echo | output/multi/benchlog/sweep_i5-032-sm12x_261030A/lite_engine_head_i5-032-sm12x.log:L554 | 로그 2개 |
| 30 | `nccl:net_ib_devices` | `[RO]; OOB <nic:cluster>:<node:sub><0>` | sub | runtime | output/multi/benchlog/sweep_i5-032-sm12x_261030A/lite_engine_head_i5-032-sm12x.log:L556 | 로그 2개 · Ray 접힘 +3(접힌 사본의 노드 미관측) |
| 31 | `nccl:network` | `IB` | sub | runtime | output/multi/benchlog/sweep_i5-032-sm12x_261030A/lite_engine_head_i5-032-sm12x.log:L579 | 로그 2개 · Ray 접힘 +3(접힌 사본의 노드 미관측) |
| 32 | `NCCL_DMABUF_ENABLE` | `0` | sub | env-echo | output/multi/benchlog/sweep_i5-032-sm12x_261030A/lite_engine_head_i5-032-sm12x.log:L581 | 로그 2개 |
| 33 | `NCCL_CROSS_NIC` | `1` | sub | env-echo | output/multi/benchlog/sweep_i5-032-sm12x_261030A/lite_engine_head_i5-032-sm12x.log:L584 | 로그 2개 |
| 34 | `NCCL_MAX_NCHANNELS` | `8` | sub | env-echo | output/multi/benchlog/sweep_i5-032-sm12x_261030A/lite_engine_i5-032-sm12x.log:L424 | 로그 1개 |
| 35 | `NCCL_LL128_BUFFSIZE` | `262144` | sub | env-echo | output/multi/benchlog/sweep_i5-032-sm12x_261030A/lite_engine_i5-032-sm12x.log:L427 | 로그 1개 |
| 36 | `NCCL_BUFFSIZE` | `1048576` | sub | env-echo | output/multi/benchlog/sweep_i5-032-sm12x_261030A/lite_engine_i5-032-sm12x.log:L428 | 로그 1개 |
| 37 | `NCCL_PROTO` | `^LL128` | sub | env-echo | output/multi/benchlog/sweep_i5-032-sm12x_261030A/lite_engine_i5-032-sm12x.log:L442 | 로그 1개 · Ray 접힘 +3(접힌 사본의 노드 미관측) |
| 38 | `NCCL_IB_SPLIT_DATA_ON_QPS` | `0` | sub | env-echo | output/multi/benchlog/sweep_i5-032-sm12x_261030A/lite_engine_i5-032-sm12x.log:L451 | 로그 1개 |
| 39 | `NCCL_IB_GID_INDEX` | `3` | main | env-echo | output/multi/benchlog/sweep_i5-032-sm12x_261030A/lite_engine_i5-032-sm12x.log:L456 | 로그 1개 |

- 값 주: 기계 치환(pii.substitution_table · 장치·호스트·주소는 자리표시 — 원문은 출처 줄)
- 값 주: 키 존재만 관측 · 값 미관측(vLLM 'Unknown vLLM environment variable detected' 경고 — 값은 로그에 없다)

- Ray 접힘: 출처 줄 끝의 `[repeated Nx across cluster]` — Ray 가 다른 프로세스의 같은 줄 N개를 한 줄로 접었다. 노드 칸은 **첫 줄**의 노드이고, 접힌 사본이 어느 노드의 것인지는 로그에 없다(관측 대상 밖).

- 로그 창(꼬리): `tail-800(lite_bench.sh@d33b0878a6c3 L234)`(꼬리 창 출처 로그 1개 · 이 표 행 #34–39) — 이 캡처는 엔진 출력의 **마지막 N줄**이다(측정 도구의 `tail -N` 상한에 닿았다 · 생산자 evidence `log_window`). 창 앞에서 echo 한 줄은 그 로그에 없다 — 이 주의는 꼬리 창 출처 행에만 걸린다(머리 캡처 행은 아래).

- 로그 창(머리): `head-600(output/multi/benchlog/sweep_i5-032-sm12x_261030A/lite_raw_i5-032-sm12x.json engine_log_head_lines)`(머리 캡처 출처 로그 1개 · 이 표 행 #1–33) — 이 캡처는 엔진 출력의 **처음 N줄**이다(lite_bench.sh 머리 캡처 `head -n N` 상한에 닿았다 · 생산자 evidence `log_head_window`). 꼬리 창의 주의는 이 행에 걸리지 않는다 · 머리 N줄 뒤에서 echo 한 줄은 그 로그에 없다. 머리 캡처와 꼬리 창 사이(엔진 출력의 가운데)는 어느 캡처에도 없다 — 표에 없는 키 · 한 노드에만 있는 키는 그 사이였을 수 있다.
<!-- /FACT:measurement_env -->

> 이 절이 답하는 질문: 수신자가 이 zip 만으로 이 셀을 다시 띄우려면 어떤 순서로 무엇을 하고, 각 단계의 성공을 무엇으로 판정하며, 얼마나 기다려야 하는가?

1. 기존NAS A팩과각노드로컬Engramdir를준비한다. 양노드는같은container경로로가중치와filetable을보지만테이블I/O는각자로컬NVMe조건이다. main에triplet3개,양노드에각자localbuild이미지가필요하며runtimepatch전파는없다. 필수env는triplet셀env와compose프로젝트/cluster/interconnect형상이다. 측정실효값은위env관측표가정본이다.
2. parentkit의core5파일별도context→alias→deps2중간2파일→childfinal24를순서대로읽는다. 실제coreargv12.1/jobs8과leafargv12.1a/jobs4를구분하고README의새조립명령은generated-unverified다. POST75추가후native확장실제재컴파일/PREcache/core불변을구분한다. 위history층창1시간56분45초는cache층을포함해wallbuildtime이아니다.
3. 현재기동1창16:10:33Z→20:28:29Z/renew9회다. MemTotal124608MiB는러너실측;weights73770/KV8192/overhead24800/floor17846/arm14774는예산선언이다. bench4096은main클라이언트별도몫이다. 로드-전게이트의filetable제외와실제cache/익명메모리/가용을구분해자기호스트예산을다시도출한다.
4. mainRayhead/serve후subRayworker합류,EXL3MCG/nativeP2B/lmheadMXFP8/filemapping/iterator skip를로그에서확인한다. 첫48shard뒤공백은기존PID의read-onlycounter/folio대기와loader638.28/638.29초전진으로해소됐으며새actor/재기동으로덮지않았다(devlog_26103005_i5_032_최소이식과_실측_인계). 전체model+draft로드main995.574696s/sub1001.462683s/73.96GiB,READY약1245s,init156.29s다. 위첫lite까지상한21분48초와다른창이다.
5. 성공은health200+chat.content1/reasoning101/stop과실제MCG/P2B/fileEngram서명이다. KV8GiB reread/2341696tokens·131072당17.87x는pool수용이며servingseq1/C1을확대하지않는다. NCCL·Ray실효키는위env와아래노드별표로읽고접힌로그를다른노드부재로판정하지않는다. eager/optionaloff경로와과거graph성공을분리한다.
6. bench JSON복원은실행원문이아니며03§3.4도구원문과함께본다. lite→GuideLLMC1×3창은16:32:21Z→16:43:41Z/11분20초다. native예산중단동안기존Full이끝나원session5043exit0를회수했고새벤치/대체executor는없다. 남아있던상주는장시간성능검증이아니다. 정식down31290exit0으로컨테이너/watch/TTL/선언을회수했다.

## 1.5 서브 레시피 해설
<!-- FACT:sub_recipe -->
원본: `artifacts/compose/sub_recipe.json`(기계 파생 — 스모크와 같은 함수 `slave_forward.derive` 의 출력).

| 역할 | 하는 일 |
|---|---|
| `master` | ray head + vllm serve(트리플렛 source · 모델 러너 실행) |
| `master_runtime_patch` | none(런타임 패치 파일 없음 — CONFIG_FILE='i5-032-sm12x' → i5-032-sm12x_patch.py 부재) |
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
| `SM12X_PORT` | image_identity | cell_env | 2026-08-14(plan_26081418 G-4): build_patches_src 소스 이식 변종 게이트 — 빠지면 마스터만 이식본·slave 는 stock. 부재 = stock(빈 값을 흘리면 compose 기본값을 빈 문자열로 덮는다). |
| `BUILD_JOBS` | build_tuning | cell_env | 2026-08-15: 이미지 정체성은 아니지만 양 노드에 같아야 한다 — slave 만 기본 16 으로 컴파일하면 거기서 OOM(하드다운 #2 가 서브였다). |
| `NAS_MODEL_PATH` | mount | project_env | 결함#2b(plan_26070119): --env-file 을 쓰면 프로젝트 .env 자동 로드가 꺼져 compose 기본 마운트로 폴백 → 모델 부재(serve 즉사). 셸 env 로 명시 주입한다. |
| `QUANT_MODEL_PATH` | mount | project_env | 결함#2b 와 같은 경로 — 양자화 모델 마운트원. |
| `TIKTOKEN_HOST_PATH` | mount | project_env | 결함#2b 와 같은 경로 — 오프라인 토크나이저 마운트원. |
| `PLE_MMAP_HOST_PATH` | mount | project_env | 2026-09-09(camp-26090918): PLE mmap 로컬 NVMe 스테이징 마운트원 — slave 도 같은 경로에 실재해야 한다. |
| `SLAVE_CONTAINER_NAME` | cluster | cell_env | 2026-08-14: 폴백 이름이면 slave 협역 워치독 필터가 매칭 0 — 서브에서만 계층 2층이 조용히 사라진다(devlog_26080212 ⑥ 부류 · 하드다운 #2 는 서브였다). 2026-08-02 에 이미 '엉뚱한 이름으로 떠 있었고 아무도 몰랐다' 로 한 번 드러났다(그때는 마스터측만 교정). |

**이미지 정체성 build-arg**: `IMAGE_TAG` · `BUILD_DOCKERFILE` · `VLLM_REF` · `SM12X_PORT`

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
| `observed` | {"build_ledger": {"compared": "dockerfile · build_args(− exempt) · patches(phase,file,script_sha256,result) · vllm git sha", "identity_exempt_build_args": ["BUILD_JOBS"], "main_identity_sha256": "90f5791a1787d984487dcea76cf26f3c60d3340227661b150bbcf44332795b33", "sub_identity_sha256": "039bc6e43a6f045623d7254c5f9474f84e40689b65d57bb0548648aedf6aacba", "verdict": "differ"}, "driver": {"main": "580.178.04", "sub": "580.178.04", "verdict": "equal"}, "torch": {"main": "2.13.0a0+9186a08b2c.nv26.7.59513937", "sub": "2.13.0a0+9186a08b2c.nv26.7.59513937", "verdict": "equal"}, "vllm_sha": {"main": "5df63a7203cdb9328d824a27b682e77c7b9928be", "sub": "5df63a7203cdb9328d824a27b682e77c7b9928be", "verdict": "equal"}} |
| `source` | output/multi/benchlog/attestation_i5-032-sm12x.json |

> **attestation 범위** — config `i5-032-sm12x` · phase serve · 작성 2026-10-29T16:31:49Z(output/multi/benchlog/attestation_i5-032-sm12x.json 파일 mtime(파일시스템 관측 — 문서에 시각 칸이 없다 · 기록 필드 아님)) · 묶은 근거 cell-name · 파일 `output/multi/benchlog/attestation_i5-032-sm12x.json` · 측정 창 대비 작성 2026-10-29T16:31:49Z < 측정 시작 2026-10-29T16:32:21Z — 측정 전(빌드·스모크)의 관측: **same config(i5-032-sm12x) · serve — 원장 순서로 판별한 같은 기동의 관측(결속된 스모크 로그 미확인 · 제한검색 · 약한 판별)(판별: 원장 순서로 판별(결속된 스모크 로그 미확인 · 제한검색 — docs/simlog/**/*serve*.log 에서 이 attestation 을 적은 로그 미확인; 다른 경로의 보존 여부는 판정하지 않음): 원장 docs/logs/main/events/2026-10.jsonl:840 budget_declare(smoke-i5-032-sm12x · 2026-10-29T16:10:33Z) ≤ attestation 작성 2026-10-29T16:31:49Z · serve_proof 작성 2026-10-29T16:31:49Z < 측정 시작 2026-10-29T16:32:21Z · 그 선언이 측정 끝 2026-10-29T16:43:41Z 까지 재선언 · 창 닫힘 없이 이어짐(main 원장 기록 범위 안) · API 서버 기동 2026-10-29T16:31:23Z(output/multi/benchlog/sweep_i5-032-sm12x_261030A/lite_engine_i5-032-sm12x.log:729 · 컨테이너 시계 = UTC 가정) ∈ [선언, attestation 작성] · 측정 엔진 로그 APIServer pid 788 하나)**
<!-- /FACT:sub_recipe -->

<!-- FACT:measurement_env_nodes -->
**노드별 측정 env**(01 §1.4 표를 키 × 노드로 접은 것 · `만 관측` = 다른 쪽 줄이 캡처 로그에 없다 · Ray 가 같은 줄을 접은 키(`[repeated Nx across cluster]`)와 머리·꼬리 캡처 로그(01 §1.4 로그 창)에서 한쪽만 남은 키는 다른 쪽 부재를 단언하지 않는다)

- 양 노드 같은 값 15개: `NCCL_BUFFSIZE` · `NCCL_CROSS_NIC` · `NCCL_DMABUF_ENABLE` · `NCCL_IB_DISABLE` · `NCCL_IB_GID_INDEX` · `NCCL_IB_HCA` · `NCCL_IB_MERGE_NICS` · `NCCL_IB_SPLIT_DATA_ON_QPS` · `NCCL_LL128_BUFFSIZE` · `NCCL_MAX_NCHANNELS` · `NCCL_NET_PLUGIN` · `NCCL_PROTO` · `NCCL_SOCKET_IFNAME` · `nccl:network` · `nccl:version`

| 키 | main | sub | 노드 미특정 | 판정 |
|---|---|---|---|---|
| `NCCL_IB_QPS_PER_CONNECTION` | — | `4` | — | sub 만 캡처 창에 남음 — main 여부 미관측(머리 캡처) |
| `VLLM_EXL3_MOE_KERNEL` | — | — | (키 존재 · 값 미관측) | 노드 미특정 |
| `VLLM_PLE_MMAP` | — | — | (키 존재 · 값 미관측) | 노드 미특정 |
| `VLLM_PLE_MMAP_DIR` | — | — | (키 존재 · 값 미관측) | 노드 미특정 |
| `VLLM_REF` | — | — | (키 존재 · 값 미관측) | 노드 미특정 |
| `nccl:net_ib_devices` | — | `[0]<nic:cluster>:1/RoCE [1]<nic:cluster>:1/RoCE [RO]; OOB <nic:cluster>:<node:sub><0>` · `[RO]; OOB <nic:cluster>:<node:sub><0>` | — | sub 만 줄에 남음 · Ray 접힘 +3 — main 사본 여부 미관측 |
<!-- /FACT:measurement_env_nodes -->

> 이 절이 답하는 질문: 서브(Ray worker) 노드는 메인과 무엇이 다르고, 무엇을 반드시 같게 맞춰야 하는가?

main은Rayhead/APIserve와triplet수령,sub는Rayworker로서EXL3native/filetable을각자로컬에서읽는다. 이셀runtimepatch는없고모델triplet을sub에복사하지않는다. 이미지핀/빌드튜닝/SM12x옵션,클러스터NCCL키,가중치·로컬Engram마운트,DSV41_/EXL3PORT_런타임env가필요무리다.
양노드같은container경로·로컬rowtable·호환driver/RoCE를선행준비한다. node-local부모/child빌드역사는다르므로main이미지/입력관측을sub과거동일바이트증거로쓰지않는다. sub정상결과는문서로인수했고추가스캔/재검증하지않았다(testlog_26103005_i5_032_TP2와_Full_판정).
ABI의SHA/torch/driver관측범위는위attestation줄이권위다. imageID/상속원장/SCM날짜와wholeimage동등을혼동하지않는다. 결속된스모크로그미확인은producer의docs/simlog한정검색이고다른경로보존부재판정이아니다. 원장기반같은boot판별의약한범위를유지한다.
실효interconnect키는위노드별표와01§1.4출처줄을따른다. Ray접힘/꼬리캡처/노드미특정줄은다른쪽키미설정이아니다. 자기NIC/endpoint는자리표시형상으로재도출하고현재렌더형상으로측정당시값을덮지않는다.

## 1.6 재현 절차 — 기동 전 준비(사람이 할 일)
> 1.4 의 명령을 치기 **전에** 사람이 손으로 해야 하는 것이다 — 가중치 획득 · 스테이징 생성 · 네트워크가 필요한 단계. 에이전트가 대신 할 수 없는 자리(다운로드 승인 · 물리 경로 · 외부 계정)가 여기 모인다.

> 이 절이 답하는 질문: 이 셀을 기동하기 전에 사람이 준비해야 하는 것은 무엇이며, 각각을 어떤 명령으로 만들고, 그 절차의 출처는 어디인가?

### 가중치와filetable
이번은기존관리NAS직독이며새다운로드없음이다. A팩은Engram동봉333GiB이고정확HF repo/revision은.git부재로미관측이다(plan_26100208_V4_1_Flash_멀티노드_1M_캠페인 §2/00§0.4). 양노드`/app/quant_models/DeepSeek/DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz`와각로컬`/app/ple_mmap/v41-engram-a`의마운트형상을yaml hf-overrides/셀env/compose로맞춘다. 확인예시 `test -d <manifest.quant_model_path>/DeepSeek/DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz`는추론—실행검증✗다. 외부oldpack과정확바이트동등을가정하지않는다.
### 스테이징
Engram테이블은각노드로컬NVMe에사전준비돼있고이번새생성실행은없다. 파일생성에쓴원래명령원문은이번direct자료에없으므로새검증명령으로합성하지않는다. 파일레이아웃/row_store조건은pinnedMia backend/layout과83/72/leafPROVENANCE를따르고table_dir의필수샤드/header/mapping을새환경에서확인해야한다. parentkit의core5/중간2/leaf24는각기다른context이며누락5support를현재activekit로대체하지않는다.
### 네트워크
corebaseNGC26.07pull/공식vLLMsourcegit,POST05/06/75의pin된wheel,pinnedexllamav3/Mia/plugin/p2b원문을빌드중받는명령은Dockerfile/패치/PROVENANCE/helper에있다. README새core→alias→deps2→child조립명령은미실행generated-unverified다. 오프라인이면동일base/source/tarball/wheel/고정해시입력을미리준비해각노드localbuild/스모크해야한다는대안은추론—실행검증✗다. 완성이미지전송이나무인가중치획득을지시하지않는다.
