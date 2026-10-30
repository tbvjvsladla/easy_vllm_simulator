# 01 · 산출물 — 무엇이 실제로 쓰였고, 어떻게 다시 띄우는가

## 1.1 적용 판정 표
> 기계가 적용 증거(빌드 원장 · 원장이 없으면 라벨된 재구성) × 파일 × 선언을 대사한 결과다. `artifacts/` 에는 재현 입력과 확인되지 않은 참고 형상도 포함될 수 있다. 파일별 검증 상태와 근거를 아래 표에서 확인한다.

<!-- FACT:slots -->
| 슬롯 | 실림 | 파일 | 신호 | 적용 증거 | 사유 |
|---|---|---|---|---|---|
| `build_patch_post` | 실림 | `artifacts/build_patch_post/05-ngc-source032-deps-compat.sh` · `artifacts/build_patch_post/06-b12x-optional032.sh` · `artifacts/build_patch_post/70-exllamav3-build.sh` · `artifacts/build_patch_post/72-exl3-row-store.sh` · `artifacts/build_patch_post/73-vllm-exl3-build.sh` · `artifacts/build_patch_post/74-exl3-moe-prep-fused.sh` · `artifacts/build_patch_post/75-exl3-python-deps032.sh` · `artifacts/build_patch_post/79-exl3-port-selfcheck.sh` | 3-signal | build-ledger:attestation:output/multi/benchlog/attestation_ds41-load-local-1m.json#main | 공유 NGC/vLLM 의존성의 hub/pytest 교집합을 설치·pip check한다. DeepSeek 전용 GPU 커널의 필요조건을 입증한 것은 아니다. 적용 증거와 이 셀에서의 활성 범위는1.1의 해당 행을 따른다. 제거 대조가 없으면 이 파일 전체를 현재 셀의 최소 필요조건으로 단정하지 않는다. 상위 소스에서는 대상 심벌·지원 여부·게이트·실제 종료 경로를 읽고 새 smoke로 판정한다. · 공식 optional B12X wheel을 설치한다. 설치 확인과 각 runtime kernel의 선택은 구별한다. 적용 증거와 이 셀에서의 활성 범위는1.1의 해당 행을 따른다. 제거 대조가 없으면 이 파일 전체를 현재 셀의 최소 필요조건으로 단정하지 않는다. 상위 소스에서는 대상 심벌·지원 여부·게이트·실제… |
| `build_patch_pre` | 실림 | `artifacts/build_patch_pre/80-exl3-quant-method.sh` · `artifacts/build_patch_pre/81-exl3-model-wiring.sh` · `artifacts/build_patch_pre/82-exl3-h2d-stage.sh` · `artifacts/build_patch_pre/83-engram-file-backend.sh` · `artifacts/build_patch_pre/84-sm12x-block-shim.sh` · `artifacts/build_patch_pre/85-exl3-serial-streams.sh` · `artifacts/build_patch_pre/86-exl3-worker-hooks.sh` · `artifacts/build_patch_pre/87-engram-embed-skip.sh` · `artifacts/build_patch_pre/88-engram-prestage.sh` · `artifacts/build_patch_pre/89-kv-null-block-zero.sh` | 3-signal | build-ledger:attestation:output/multi/benchlog/attestation_ds41-load-local-1m.json#main | EXL3 quant method와 registry 접점을 구성한다(W13). 적용 증거와 이 셀에서의 활성 범위는1.1의 해당 행을 따른다. 제거 대조가 없으면 이 파일 전체를 현재 셀의 최소 필요조건으로 단정하지 않는다. 상위 소스에서는 대상 심벌·지원 여부·게이트·실제 종료 경로를 읽고 새 smoke로 판정한다. · DeepSeek 모델과 EXL3 로더 접점을 구성한다. 적용 증거와 이 셀에서의 활성 범위는1.1의 해당 행을 따른다. 제거 대조가 없으면 이 파일 전체를 현재 셀의 최소 필요조건으로 단정하지 않는다. 상위 소스에서는 대상 심벌·지원 여부·게이트·실제 종료 경로를 읽고 새 smoke로 판정한다. · EXL3 로딩의 H2D staging 경로를 구성한다. 적용 증거와 이 셀에서의 활성 범위는… |
| `build_recipe` | 실림 | `artifacts/build_recipe/Dockerfile.source-build-sm12x` · `artifacts/build_recipe/parent-source-core/README.md` · `artifacts/build_recipe/parent-source-core/alias-evidence.json` · `artifacts/build_recipe/parent-source-core/context/Dockerfile.source-build` · `artifacts/build_recipe/parent-source-core/context/build_patches/.gitkeep` · `artifacts/build_recipe/parent-source-core/context/build_patches/05-ngc-source032-deps-compat.sh` · `artifacts/build_recipe/parent-source-core/context/build_patches_src/.gitkeep` · `artifacts/build_recipe/parent-source-core/context/requirements.txt` · `artifacts/build_recipe/parent-source-core/core-build-command.txt` · `artifacts/build_recipe/parent-source-core/intermediate-deps2/context/Dockerfile.source-build-deps2` · `artifacts/build_recipe/parent-source-core/intermediate-deps2/context/build_patches/06-b12x-optional032.sh` · `artifacts/build_recipe/parent-source-core/leaf-build-command.txt` · `artifacts/build_recipe/parent-source-core/leaf-ledger-evidence.json` · `artifacts/build_recipe/parent-source-core/leaf-result-evidence.json` · `artifacts/build_recipe/parent-source-core/leaf-support/build_patches/.gitkeep` · `artifacts/build_recipe/parent-source-core/leaf-support/build_patches/Dockerfile.source-build-deps2` · `artifacts/build_recipe/parent-source-core/leaf-support/build_patches_src/.gitkeep` · `artifacts/build_recipe/parent-source-core/leaf-support/build_patches_src/PROVENANCE.json` · `artifacts/build_recipe/parent-source-core/leaf-support/build_patches_src/exl3_port_lib.py` · `artifacts/build_recipe/parent-source-core/manifest.json` | 3-signal | build-ledger:build-ledger.dockerfile | 공유 NGC/vLLM 의존성의 hub/pytest 교집합을 설치·pip check한다. DeepSeek 전용 GPU 커널의 필요조건을 입증한 것은 아니다. 적용 증거와 이 셀에서의 활성 범위는1.1의 해당 행을 따른다. 제거 대조가 없으면 이 파일 전체를 현재 셀의 최소 필요조건으로 단정하지 않는다. 상위 소스에서는 대상 심벌·지원 여부·게이트·실제 종료 경로를 읽고 새 smoke로 판정한다. · 공식 optional B12X wheel을 설치한다. 설치 확인과 각 runtime kernel의 선택은 구별한다. 적용 증거와 이 셀에서의 활성 범위는1.1의 해당 행을 따른다. 제거 대조가 없으면 이 파일 전체를 현재 셀의 최소 필요조건으로 단정하지 않는다. 상위 소스에서는 대상 심벌·지원 여부·게이트·실제… |
| `compose` | 실림 | `artifacts/compose/.env.cluster.template` · `artifacts/compose/.env.interconnect.template` · `artifacts/compose/.env.template` · `artifacts/compose/docker-compose.yaml` · `artifacts/compose/serve_runner.sh` · `artifacts/compose/sub_recipe.json` | 1-signal | file:output/multi/docker-compose.yaml | multi 서빙 경로(compose · 러너 · env 형상) — 측정 당시 개정(selected_revisions)(기계 파생) |
| `fork_pin` | 안 실림 | — | 1-signal | none:.claude/policies/arch_variant_ledger.json | 셀 env 에 VARIANT 줄 없음 = stock(workflow.md §변종 좌표의 거처)(기계 파생) |
| `runtime_patch` | 안 실림 | — | 1-signal | none:output/multi/configs/ds41-load-local-1m_patch.py | output/multi/configs/ds41-load-local-1m_patch.py 없음 — arm_patch.sh 는 no-op(런타임 패치 불해당)(기계 파생) |
| `triplet` | 실림 | `artifacts/triplet/.env.ds41-load-local-1m` · `artifacts/triplet/ds41-load-local-1m.sh` · `artifacts/triplet/ds41-load-local-1m.yaml` | 2-signal | file:output/multi/configs/ds41-load-local-1m.yaml | 셀 id ds41-load-local-1m = 트리플렛 이름(configs/ds41-load-local-1m.yaml · .sh · envs/.env.ds41-load-local-1m)(기계 파생) |

**레시피 경고 · 실린 리비전**(기계 관측 — 수신자 빌드가 여기서 갈라질 수 있다)

- ⚠ `build_recipe` `output/multi/Dockerfile.source-build-sm12x` — 실린 바이트(worktree)가 측정 이미지 docker history 의 RUN·COPY 와 전수 일치하지 않는다(9/31)(이미지 Created 2026-10-29T11:59:03Z · 마지막 커밋 미관측)
- `build_recipe` `output/multi/Dockerfile.source-build-sm12x` 실린 리비전: 리비전 미특정 · 선택 방법 `worktree(어느 개정도 이미지 history 와 더 맞지 않는다 · 작업트리 9/31 · 대조할 개정 없음(이미지 Created 이전 커밋 부재 · 비추적) · 전 이력(output/*/Dockerfile.source-build-sm12x · 모든 ref)에도 전수 일치 개정 없음 — recipe_vs_image 경고 유지)` · docker history 대조 9/31 · 실린 바이트 `worktree` · 쓰인 바이트임을 관측으로 확인 아니오
- `compose` `.claude/skills/upstream-version-watch/assets/configs/serve_runner.sh` 실린 리비전: 커밋 `50a5ce486738` · 선택 방법 `worktree(mtime 2026-09-06T21:57:53Z ≤ 측정 2026-10-30T15:54:26Z(sweep_index.generated_utc) · 바이트 = 측정 전 마지막 커밋 50a5ce486738)` · docker history 대조 없음 · 실린 바이트 `worktree` · 근거 `mtime≤measured` · 쓰인 바이트임을 관측으로 확인 아니오
- `compose` `output/multi/docker-compose.yaml` 실린 리비전: 커밋 `bccd08091976` · 선택 방법 `worktree(mtime 2026-10-29T10:01:25Z ≤ 측정 2026-10-30T15:54:26Z(sweep_index.generated_utc) · 바이트 = 측정 전 마지막 커밋 bccd08091976)` · docker history 대조 없음 · 실린 바이트 `worktree` · 근거 `mtime≤measured` · 쓰인 바이트임을 관측으로 확인 아니오

**싣지 않은 파일**(슬롯별 사유 — 빌드 때 skip 된 패치는 아래 적용 집합 표에 있다)

- `build_patch_post` known-non-patch 1건: `.gitkeep`
- `build_patch_pre` `exl3_port_lib.py` — `nonconforming(<NN>-*.sh 규약 밖 — 싣지 않음)`
- `build_patch_pre` known-non-patch 2건: `.gitkeep` · `PROVENANCE.json`
- `compose` `arm_patch.sh` — `not-armed(output/multi/configs/ds41-load-local-1m_patch.py 없음(CONFIG_FILE) · slave default_patch.py(CONFIG_FILE) 없음 — arm_patch.sh 는 no-op · arm_patch.sh 측정 당시 개정(1b1df2cd4034) `_cfg=` 키 체인 RUNTIME_PATCH_STEM > CONFIG_FILE > 'default')`

**파일별 검증 · 관련성**(`generated-unverified` = 실행 검증되지 않은 생성물 — 파일 첫 줄 경고 · PAYLOAD 파일 기록과 같은 표시 · 관련성 = ① 패치 선언 대상 × 모델 config ② 엔진 로그 발화 서명 — 둘 다 관측일 때만 required/inactive-inferred)

| 슬롯 | 파일 | 검증 | 검증 근거 | 관련성 | 관련성 근거 |
|---|---|---|---|---|---|
| `build_patch_post` | `artifacts/build_patch_post/05-ngc-source032-deps-compat.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_ds41-load-local-1m.json#main) · 원장 관측 | `unknown` | ① 패치 헤더·본문에 대상 선언(model-trigger 아키텍처 · vllm/models/<model_type>) 없음 |
| `build_patch_post` | `artifacts/build_patch_post/06-b12x-optional032.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_ds41-load-local-1m.json#main) · 원장 관측 | `unknown` | ① 패치 헤더·본문에 대상 선언(model-trigger 아키텍처 · vllm/models/<model_type>) 없음 |
| `build_patch_post` | `artifacts/build_patch_post/70-exllamav3-build.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_ds41-load-local-1m.json#main) · 원장 관측 | `unknown` | ① 패치 헤더·본문에 대상 선언(model-trigger 아키텍처 · vllm/models/<model_type>) 없음 |
| `build_patch_post` | `artifacts/build_patch_post/72-exl3-row-store.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_ds41-load-local-1m.json#main) · 원장 관측 | `unknown` | ① 패치 헤더·본문에 대상 선언(model-trigger 아키텍처 · vllm/models/<model_type>) 없음 |
| `build_patch_post` | `artifacts/build_patch_post/73-vllm-exl3-build.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_ds41-load-local-1m.json#main) · 원장 관측 | `unknown` | ① 패치 헤더·본문에 대상 선언(model-trigger 아키텍처 · vllm/models/<model_type>) 없음 · 편집하는 공유(모델 디렉터리 밖) 파일 ['vllm/model_executor/model_loader/weight_utils.py'] |
| `build_patch_post` | `artifacts/build_patch_post/74-exl3-moe-prep-fused.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_ds41-load-local-1m.json#main) · 원장 관측 | `unknown` | ① 패치 헤더·본문에 대상 선언(model-trigger 아키텍처 · vllm/models/<model_type>) 없음 |
| `build_patch_post` | `artifacts/build_patch_post/75-exl3-python-deps032.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_ds41-load-local-1m.json#main) · 원장 관측 | `unknown` | ① 패치 헤더·본문에 대상 선언(model-trigger 아키텍처 · vllm/models/<model_type>) 없음 |
| `build_patch_post` | `artifacts/build_patch_post/79-exl3-port-selfcheck.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_ds41-load-local-1m.json#main) · 원장 관측 | `unknown` | ① 패치 헤더·본문에 대상 선언(model-trigger 아키텍처 · vllm/models/<model_type>) 없음 |
| `build_patch_pre` | `artifacts/build_patch_pre/80-exl3-quant-method.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_ds41-load-local-1m.json#main) · 원장 관측 | `unknown` | ① 패치 헤더·본문에 대상 선언(model-trigger 아키텍처 · vllm/models/<model_type>) 없음 · 편집하는 공유(모델 디렉터리 밖) 파일 ['vllm/model_executor/layers/quantization/__init__.py'] |
| `build_patch_pre` | `artifacts/build_patch_pre/81-exl3-model-wiring.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_ds41-load-local-1m.json#main) · 원장 관측 | `unknown` | ① 선언 ['models/deepseek_v41'] ∋ 셀 모델(architectures ['DeepseekV41ForCausalLM'] · model_type deepseek_v41) · ② 패치가 심는 logger 발화 서명 없음(발화 미관측) |
| `build_patch_pre` | `artifacts/build_patch_pre/82-exl3-h2d-stage.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_ds41-load-local-1m.json#main) · 원장 관측 | `unknown` | ① 선언 ['models/deepseek_v41'] ∋ 셀 모델(architectures ['DeepseekV41ForCausalLM'] · model_type deepseek_v41) · ② 패치가 심는 logger 발화 서명 없음(발화 미관측) · 편집하는 공유(모델 디렉터리 밖) 파일 ['vllm/model_executor/layers/linear.py', 'vllm/model_executor/layers/vocab_parallel_embedding.py', 'vllm/model_executor/model_loader/weight_utils.py'] |
| `build_patch_pre` | `artifacts/build_patch_pre/83-engram-file-backend.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_ds41-load-local-1m.json#main) · 원장 관측 | `unknown` | ① 선언 ['models/deepseek_v41'] ∋ 셀 모델(architectures ['DeepseekV41ForCausalLM'] · model_type deepseek_v41) · ② 엔진 로그 5개에서 서명 무발화(서명 3개 전부 0회) — 두 신호가 한 방향을 가리키지 않는다 |
| `build_patch_pre` | `artifacts/build_patch_pre/84-sm12x-block-shim.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_ds41-load-local-1m.json#main) · 원장 관측 | `unknown` | ① 선언 ['models/deepseek_v41'] ∋ 셀 모델(architectures ['DeepseekV41ForCausalLM'] · model_type deepseek_v41) · ② 패치가 심는 logger 발화 서명 없음(발화 미관측) · 편집하는 공유(모델 디렉터리 밖) 파일 ['vllm/model_executor/layers/sparse_attn_indexer.py', 'vllm/v1/attention/backends/mla/indexer.py'] |
| `build_patch_pre` | `artifacts/build_patch_pre/85-exl3-serial-streams.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_ds41-load-local-1m.json#main) · 원장 관측 | `unknown` | ① 선언 ['models/deepseek_v4', 'models/deepseek_v41'] ∋ 셀 모델(architectures ['DeepseekV41ForCausalLM'] · model_type deepseek_v41) · ② 패치가 심는 logger 발화 서명 없음(발화 미관측) |
| `build_patch_pre` | `artifacts/build_patch_pre/86-exl3-worker-hooks.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_ds41-load-local-1m.json#main) · 원장 관측 | `unknown` | ① 패치 헤더·본문에 대상 선언(model-trigger 아키텍처 · vllm/models/<model_type>) 없음 · 편집하는 공유(모델 디렉터리 밖) 파일 ['vllm/v1/worker/gpu_worker.py'] |
| `build_patch_pre` | `artifacts/build_patch_pre/87-engram-embed-skip.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_ds41-load-local-1m.json#main) · 원장 관측 | `unknown` | ① 선언 ['models/deepseek_v41'] ∋ 셀 모델(architectures ['DeepseekV41ForCausalLM'] · model_type deepseek_v41) · ② 패치가 심는 logger 발화 서명 없음(발화 미관측) · 편집하는 공유(모델 디렉터리 밖) 파일 ['vllm/model_executor/model_loader/ep_weight_filter.py'] |
| `build_patch_pre` | `artifacts/build_patch_pre/88-engram-prestage.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_ds41-load-local-1m.json#main) · 원장 관측 | `unknown` | ① 선언 ['models/deepseek_v41'] ∋ 셀 모델(architectures ['DeepseekV41ForCausalLM'] · model_type deepseek_v41) · ② 패치가 심는 logger 발화 서명 없음(발화 미관측) · 편집하는 공유(모델 디렉터리 밖) 파일 ['vllm/v1/worker/gpu/model_runner.py'] |
| `build_patch_pre` | `artifacts/build_patch_pre/89-kv-null-block-zero.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_ds41-load-local-1m.json#main) · 원장 관측 | `unknown` | ① 패치 헤더·본문에 대상 선언(model-trigger 아키텍처 · vllm/models/<model_type>) 없음 · 편집하는 공유(모델 디렉터리 밖) 파일 ['vllm/v1/worker/gpu/model_runner.py'] |
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
| `compose` | `artifacts/compose/docker-compose.yaml` | `generated-unverified` · renderer | docker-compose.yaml: worktree(mtime 2026-10-29T10:01:25Z ≤ 측정 2026-10-30T15:54:26Z(sweep_index.generated_utc) · 바이트 = 측정 전 마지막 커밋 bccd08091976) — 측정 당시 실행 바이트는 별도 관측되지 않았다 | `unknown` | 파일의 실행 당시 바이트가 확인되지 않았다(generated-unverified · 생성 renderer) — 이 표시만으로 경로 실행 여부나 필요 여부를 판정하지 않는다(참고 형상) |
| `compose` | `artifacts/compose/serve_runner.sh` | `generated-unverified` · renderer | serve_runner.sh: worktree(mtime 2026-09-06T21:57:53Z ≤ 측정 2026-10-30T15:54:26Z(sweep_index.generated_utc) · 바이트 = 측정 전 마지막 커밋 50a5ce486738) — 측정 당시 실행 바이트는 별도 관측되지 않았다 | `unknown` | 파일의 실행 당시 바이트가 확인되지 않았다(generated-unverified · 생성 renderer) — 이 표시만으로 경로 실행 여부나 필요 여부를 판정하지 않는다(참고 형상) |
| `compose` | `artifacts/compose/sub_recipe.json` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/.env.ds41-load-local-1m` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/ds41-load-local-1m.sh` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/ds41-load-local-1m.yaml` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
<!-- /FACT:slots -->

<!-- FACT:applied_set -->
**적용 집합** — 상태 `observed` · 출처 `attestation:output/multi/benchlog/attestation_ds41-load-local-1m.json#main`

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
| attestation | observed | output/multi/benchlog/attestation_ds41-load-local-1m.json |

> **attestation 범위** — config `ds41-load-local-1m` · phase serve · 작성 2026-10-30T15:42:25Z(output/multi/benchlog/attestation_ds41-load-local-1m.json 파일 mtime(파일시스템 관측 — 문서에 시각 칸이 없다 · 기록 필드 아님)) · 묶은 근거 cell-name · 파일 `output/multi/benchlog/attestation_ds41-load-local-1m.json` · 측정 창 대비 작성 2026-10-30T15:42:25Z < 측정 시작 2026-10-30T15:42:48Z — 측정 전(빌드·스모크)의 관측: **same config(ds41-load-local-1m) · serve — 원장 순서로 판별한 같은 기동의 관측(결속된 스모크 로그 미확인 · 제한검색 · 약한 판별)(판별: 원장 순서로 판별(결속된 스모크 로그 미확인 · 제한검색 — docs/simlog/**/*serve*.log 에서 이 attestation 을 적은 로그 미확인; 다른 경로의 보존 여부는 판정하지 않음): 원장 docs/logs/main/events/2026-10.jsonl:952 budget_declare(smoke-ds41-load-local-1m · 2026-10-30T15:34:53Z) ≤ attestation 작성 2026-10-30T15:42:25Z · serve_proof 작성 2026-10-30T15:42:25Z < 측정 시작 2026-10-30T15:42:48Z · 그 선언이 측정 끝 2026-10-30T15:54:26Z 까지 재선언 · 창 닫힘 없이 이어짐(main 원장 기록 범위 안) · API 서버 기동 2026-10-30T15:41:58Z(output/multi/benchlog/sweep_loading_ds41-load-local-1m/lite_engine_ds41-load-local-1m.log:726 · 컨테이너 시계 = UTC 가정) ∈ [선언, attestation 작성] · 측정 엔진 로그 APIServer pid 1 하나)**
<!-- /FACT:applied_set -->

## 1.2 슬롯별 적용 사유
> 이 절이 답하는 질문: 실린 산출물 하나하나는 무엇을 고치며, 왜 이 셀에 필요했고, 이 셀에서 실제로 발화했으며, 언제 불필요해지는가?

### 빌드 레시피·compose·트리플렛
빌드 레시피는 각 노드의 기존 source/deps2와 최소 EXL3 port의 계보다. 이번 로딩실험은 기존 이미지를 재사용했으며 새 core/native 빌드를 하지 않았다. 실린 parent 재구성은 generated-unverified이고 원 실행을 그대로 복제한 완성 빌드라고 포장하지 않는다. compose/runner는 Ray head·worker와 모델 ro bind를 연결하고 트리플렛은 이번1M 고정 형상이다. 별도 runtime patch 파일은 없다. 1.1의 파일별 검증·활성 판정을 우선한다.

### 05-ngc-source032-deps-compat.sh
공유 NGC/vLLM 의존성의 hub/pytest 교집합을 설치·pip check한다. DeepSeek 전용 GPU 커널의 필요조건을 입증한 것은 아니다.
적용 증거와 이 셀에서의 활성 범위는1.1의 해당 행을 따른다. 제거 대조가 없으면 이 파일 전체를 현재 셀의 최소 필요조건으로 단정하지 않는다. 상위 소스에서는 대상 심벌·지원 여부·게이트·실제 종료 경로를 읽고 새 smoke로 판정한다.

### 06-b12x-optional032.sh
공식 optional B12X wheel을 설치한다. 설치 확인과 각 runtime kernel의 선택은 구별한다.
적용 증거와 이 셀에서의 활성 범위는1.1의 해당 행을 따른다. 제거 대조가 없으면 이 파일 전체를 현재 셀의 최소 필요조건으로 단정하지 않는다. 상위 소스에서는 대상 심벌·지원 여부·게이트·실제 종료 경로를 읽고 새 smoke로 판정한다.

### 70-exllamav3-build.sh
EXL3 native 의존성을 빌드한다.
적용 증거와 이 셀에서의 활성 범위는1.1의 해당 행을 따른다. 제거 대조가 없으면 이 파일 전체를 현재 셀의 최소 필요조건으로 단정하지 않는다. 상위 소스에서는 대상 심벌·지원 여부·게이트·실제 종료 경로를 읽고 새 smoke로 판정한다.

### 72-exl3-row-store.sh
file-backed 행 조회의 row-store native 지원을 빌드한다.
적용 증거와 이 셀에서의 활성 범위는1.1의 해당 행을 따른다. 제거 대조가 없으면 이 파일 전체를 현재 셀의 최소 필요조건으로 단정하지 않는다. 상위 소스에서는 대상 심벌·지원 여부·게이트·실제 종료 경로를 읽고 새 smoke로 판정한다.

### 73-vllm-exl3-build.sh
vLLM EXL3 native 연동과 p2b 등 지원을 빌드한다.
적용 증거와 이 셀에서의 활성 범위는1.1의 해당 행을 따른다. 제거 대조가 없으면 이 파일 전체를 현재 셀의 최소 필요조건으로 단정하지 않는다. 상위 소스에서는 대상 심벌·지원 여부·게이트·실제 종료 경로를 읽고 새 smoke로 판정한다.

### 74-exl3-moe-prep-fused.sh
선택적 fused preparation native 지원을 설치한다. 현재 DSV41_MOE_PREP_FUSED=0이므로 설치를 새 활성화·성능이득으로 세지 않는다.
적용 증거와 이 셀에서의 활성 범위는1.1의 해당 행을 따른다. 제거 대조가 없으면 이 파일 전체를 현재 셀의 최소 필요조건으로 단정하지 않는다. 상위 소스에서는 대상 심벌·지원 여부·게이트·실제 종료 경로를 읽고 새 smoke로 판정한다.

### 75-exl3-python-deps032.sh
FLA/fla-core/marisa-trie 고정 wheel로 실제 attempt1의 Python 의존 결손을 해소한다(W15).
적용 증거와 이 셀에서의 활성 범위는1.1의 해당 행을 따른다. 제거 대조가 없으면 이 파일 전체를 현재 셀의 최소 필요조건으로 단정하지 않는다. 상위 소스에서는 대상 심벌·지원 여부·게이트·실제 종료 경로를 읽고 새 smoke로 판정한다.

### 79-exl3-port-selfcheck.sh
EXL3 port의 설치·API 정합을 검사한다. 자체검사와 분산 모델 서빙은 별도 검증이다.
적용 증거와 이 셀에서의 활성 범위는1.1의 해당 행을 따른다. 제거 대조가 없으면 이 파일 전체를 현재 셀의 최소 필요조건으로 단정하지 않는다. 상위 소스에서는 대상 심벌·지원 여부·게이트·실제 종료 경로를 읽고 새 smoke로 판정한다.

### 80-exl3-quant-method.sh
EXL3 quant method와 registry 접점을 구성한다(W13).
적용 증거와 이 셀에서의 활성 범위는1.1의 해당 행을 따른다. 제거 대조가 없으면 이 파일 전체를 현재 셀의 최소 필요조건으로 단정하지 않는다. 상위 소스에서는 대상 심벌·지원 여부·게이트·실제 종료 경로를 읽고 새 smoke로 판정한다.

### 81-exl3-model-wiring.sh
DeepSeek 모델과 EXL3 로더 접점을 구성한다.
적용 증거와 이 셀에서의 활성 범위는1.1의 해당 행을 따른다. 제거 대조가 없으면 이 파일 전체를 현재 셀의 최소 필요조건으로 단정하지 않는다. 상위 소스에서는 대상 심벌·지원 여부·게이트·실제 종료 경로를 읽고 새 smoke로 판정한다.

### 82-exl3-h2d-stage.sh
EXL3 로딩의 H2D staging 경로를 구성한다.
적용 증거와 이 셀에서의 활성 범위는1.1의 해당 행을 따른다. 제거 대조가 없으면 이 파일 전체를 현재 셀의 최소 필요조건으로 단정하지 않는다. 상위 소스에서는 대상 심벌·지원 여부·게이트·실제 종료 경로를 읽고 새 smoke로 판정한다.

### 83-engram-file-backend.sh
common Engram에 file-backed 경로를 연결한다. 현재 rank별 file-backed active/행 범위 관측으로 경로를 확인한다(W14/W18).
적용 증거와 이 셀에서의 활성 범위는1.1의 해당 행을 따른다. 제거 대조가 없으면 이 파일 전체를 현재 셀의 최소 필요조건으로 단정하지 않는다. 상위 소스에서는 대상 심벌·지원 여부·게이트·실제 종료 경로를 읽고 새 smoke로 판정한다.

### 84-sm12x-block-shim.sh
SM12x sparse-attention block 접점을 구성한다. 옛 anchor를 단순 재사용하지 않은 범위는W14다.
적용 증거와 이 셀에서의 활성 범위는1.1의 해당 행을 따른다. 제거 대조가 없으면 이 파일 전체를 현재 셀의 최소 필요조건으로 단정하지 않는다. 상위 소스에서는 대상 심벌·지원 여부·게이트·실제 종료 경로를 읽고 새 smoke로 판정한다.

### 85-exl3-serial-streams.sh
EXL3 serial-stream 경로를 구성한다.
적용 증거와 이 셀에서의 활성 범위는1.1의 해당 행을 따른다. 제거 대조가 없으면 이 파일 전체를 현재 셀의 최소 필요조건으로 단정하지 않는다. 상위 소스에서는 대상 심벌·지원 여부·게이트·실제 종료 경로를 읽고 새 smoke로 판정한다.

### 86-exl3-worker-hooks.sh
worker에 EXL3 runtime hook을 연결한다. container env 존재만으로 actor 도달을 단정하지 않는다.
적용 증거와 이 셀에서의 활성 범위는1.1의 해당 행을 따른다. 제거 대조가 없으면 이 파일 전체를 현재 셀의 최소 필요조건으로 단정하지 않는다. 상위 소스에서는 대상 심벌·지원 여부·게이트·실제 종료 경로를 읽고 새 smoke로 판정한다.

### 87-engram-embed-skip.sh
file-backed Engram의 embed tensor를 weight iterator에서 제외한다. 현재 iterator skip 발화가 확인 근거다.
적용 증거와 이 셀에서의 활성 범위는1.1의 해당 행을 따른다. 제거 대조가 없으면 이 파일 전체를 현재 셀의 최소 필요조건으로 단정하지 않는다. 상위 소스에서는 대상 심벌·지원 여부·게이트·실제 종료 경로를 읽고 새 smoke로 판정한다.

### 88-engram-prestage.sh
Engram 행 조회를 prestage하는 경로다. armed/verify4/early off/census0을 실제 로그와 대조하며 비동기 EARLY 성능 검증을 주장하지 않는다.
적용 증거와 이 셀에서의 활성 범위는1.1의 해당 행을 따른다. 제거 대조가 없으면 이 파일 전체를 현재 셀의 최소 필요조건으로 단정하지 않는다. 상위 소스에서는 대상 심벌·지원 여부·게이트·실제 종료 경로를 읽고 새 smoke로 판정한다.

### 89-kv-null-block-zero.sh
graph capture 뒤 KV null block zero의 역사적 교정이다. 현재 eager 형상에서 새 graph NaN 해소 실험을 수행한 것은 아니다.
적용 증거와 이 셀에서의 활성 범위는1.1의 해당 행을 따른다. 제거 대조가 없으면 이 파일 전체를 현재 셀의 최소 필요조건으로 단정하지 않는다. 상위 소스에서는 대상 심벌·지원 여부·게이트·실제 종료 경로를 읽고 새 smoke로 판정한다.

옛55/60/62/64 및 선택성능90~92를 현재 적용분으로 자동 승계하지 않는다. SM12X_PORT 게이트의 skip와 source/API 불일치 fail은 다른 분기이며 모든 드리프트가 안전하게 skip되는 것은 아니다.

> [원문] 80-exl3-quant-method.sh §L10-15
> : "${SM12X_PORT:=0}"
> if [ "${SM12X_PORT}" != "1" ]; then
>     echo "$TAG skip - SM12X_PORT=${SM12X_PORT} (stock 빌드 경로 불변)"
>     if [ -n "${EASY_VLLM_PATCH_STATUS:-}" ]; then printf 'skipped\tSM12X_PORT!=1\n' > "$EASY_VLLM_PATCH_STATUS"; fi
>     exit 0
> fi

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
| `hf-overrides` | {"engram_table_dir":"/app/quant_models/DeepSeek/DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz"} | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `block-size` | 64 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `kernel-config` | {"sparse_indexer_topk_backend":"per_row"} | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `kv-cache-dtype` | fp8 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `kv-cache-memory-bytes` | 8589934592 | inherited | lockset 출처 kv_source=hand · lockset provenance=hand-authored | — | lockset.kv_source=hand |
| `gpu-memory-utilization` | 0.8 | declared-requirement | lockset 출처 gmu_source=target_gmu · lockset provenance=hand-authored | — | lockset.gmu_source=target_gmu |
| `max-model-len` | 1048576 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
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
id: 'V1'
kind: 'value-status'
노브: 'tensor-parallel-size'
값: '2'
지위: 'inherited'
근거: '이번 저장정책 비교의 고정 손레버 또는 선례 승계값; 현재 셀에서 조정 스윕·제거 실패 대조는 수행하지 않았다.'
출처: ["testlog_26103023_DeepSeek_1M_로딩비교_진행", "artifacts/triplet/ds41-load-local-1m.yaml"]
```

```hint-event
id: 'V2'
kind: 'value-status'
노브: 'distributed-executor-backend'
값: 'ray'
지위: 'inherited'
근거: '이번 저장정책 비교의 고정 손레버 또는 선례 승계값; 현재 셀에서 조정 스윕·제거 실패 대조는 수행하지 않았다.'
출처: ["testlog_26103023_DeepSeek_1M_로딩비교_진행", "artifacts/triplet/ds41-load-local-1m.yaml"]
```

```hint-event
id: 'V3'
kind: 'value-status'
노브: 'quantization'
값: 'exl3'
지위: 'inherited'
근거: '이번 저장정책 비교의 고정 손레버 또는 선례 승계값; 현재 셀에서 조정 스윕·제거 실패 대조는 수행하지 않았다.'
출처: ["testlog_26103023_DeepSeek_1M_로딩비교_진행", "artifacts/triplet/ds41-load-local-1m.yaml"]
```

```hint-event
id: 'V4'
kind: 'value-status'
노브: 'tokenizer-mode'
값: 'deepseek_v41'
지위: 'inherited'
근거: '이번 저장정책 비교의 고정 손레버 또는 선례 승계값; 현재 셀에서 조정 스윕·제거 실패 대조는 수행하지 않았다.'
출처: ["testlog_26103023_DeepSeek_1M_로딩비교_진행", "artifacts/triplet/ds41-load-local-1m.yaml"]
```

```hint-event
id: 'V5'
kind: 'value-status'
노브: 'hf-overrides'
값: '{"engram_table_dir":"/app/quant_models/DeepSeek/DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz"}'
지위: 'inherited'
근거: '이번 저장정책 비교의 고정 손레버 또는 선례 승계값; 현재 셀에서 조정 스윕·제거 실패 대조는 수행하지 않았다.'
출처: ["testlog_26103023_DeepSeek_1M_로딩비교_진행", "artifacts/triplet/ds41-load-local-1m.yaml"]
```

```hint-event
id: 'V6'
kind: 'value-status'
노브: 'block-size'
값: '64'
지위: 'inherited'
근거: '이번 저장정책 비교의 고정 손레버 또는 선례 승계값; 현재 셀에서 조정 스윕·제거 실패 대조는 수행하지 않았다.'
출처: ["testlog_26103023_DeepSeek_1M_로딩비교_진행", "artifacts/triplet/ds41-load-local-1m.yaml"]
```

```hint-event
id: 'V7'
kind: 'value-status'
노브: 'kernel-config'
값: '{"sparse_indexer_topk_backend":"per_row"}'
지위: 'inherited'
근거: '이번 저장정책 비교의 고정 손레버 또는 선례 승계값; 현재 셀에서 조정 스윕·제거 실패 대조는 수행하지 않았다.'
출처: ["testlog_26103023_DeepSeek_1M_로딩비교_진행", "artifacts/triplet/ds41-load-local-1m.yaml"]
```

```hint-event
id: 'V8'
kind: 'value-status'
노브: 'kv-cache-dtype'
값: 'fp8'
지위: 'inherited'
근거: '이번 저장정책 비교의 고정 손레버 또는 선례 승계값; 현재 셀에서 조정 스윕·제거 실패 대조는 수행하지 않았다.'
출처: ["testlog_26103023_DeepSeek_1M_로딩비교_진행", "artifacts/triplet/ds41-load-local-1m.yaml"]
```

```hint-event
id: 'V9'
kind: 'value-status'
노브: 'kv-cache-memory-bytes'
값: '8589934592'
지위: 'inherited'
근거: '이번 저장정책 비교의 고정 손레버 또는 선례 승계값; 현재 셀에서 조정 스윕·제거 실패 대조는 수행하지 않았다.'
출처: ["testlog_26103023_DeepSeek_1M_로딩비교_진행", "artifacts/triplet/ds41-load-local-1m.yaml"]
```

```hint-event
id: 'V10'
kind: 'value-status'
노브: 'gpu-memory-utilization'
값: '0.8'
지위: 'inherited'
근거: '이번 저장정책 비교의 고정 손레버 또는 선례 승계값; 현재 셀에서 조정 스윕·제거 실패 대조는 수행하지 않았다.'
출처: ["testlog_26103023_DeepSeek_1M_로딩비교_진행", "artifacts/triplet/ds41-load-local-1m.yaml"]
```

```hint-event
id: 'V11'
kind: 'value-status'
노브: 'max-model-len'
값: '1048576'
지위: 'inherited'
근거: '이번 저장정책 비교의 고정 손레버 또는 선례 승계값; 현재 셀에서 조정 스윕·제거 실패 대조는 수행하지 않았다.'
출처: ["testlog_26103023_DeepSeek_1M_로딩비교_진행", "artifacts/triplet/ds41-load-local-1m.yaml"]
```

```hint-event
id: 'V12'
kind: 'value-status'
노브: 'max-num-seqs'
값: '1'
지위: 'inherited'
근거: '이번 저장정책 비교의 고정 손레버 또는 선례 승계값; 현재 셀에서 조정 스윕·제거 실패 대조는 수행하지 않았다.'
출처: ["testlog_26103023_DeepSeek_1M_로딩비교_진행", "artifacts/triplet/ds41-load-local-1m.yaml"]
```

```hint-event
id: 'V13'
kind: 'value-status'
노브: 'max-num-batched-tokens'
값: '1536'
지위: 'inherited'
근거: '이번 저장정책 비교의 고정 손레버 또는 선례 승계값; 현재 셀에서 조정 스윕·제거 실패 대조는 수행하지 않았다.'
출처: ["testlog_26103023_DeepSeek_1M_로딩비교_진행", "artifacts/triplet/ds41-load-local-1m.yaml"]
```

```hint-event
id: 'V14'
kind: 'value-status'
노브: 'enforce-eager'
값: 'true'
지위: 'inherited'
근거: '이번 저장정책 비교의 고정 손레버 또는 선례 승계값; 현재 셀에서 조정 스윕·제거 실패 대조는 수행하지 않았다.'
출처: ["testlog_26103023_DeepSeek_1M_로딩비교_진행", "artifacts/triplet/ds41-load-local-1m.yaml"]
```

```hint-event
id: 'V15'
kind: 'value-status'
노브: 'no-enable-flashinfer-autotune'
값: 'true'
지위: 'inherited'
근거: '이번 저장정책 비교의 고정 손레버 또는 선례 승계값; 현재 셀에서 조정 스윕·제거 실패 대조는 수행하지 않았다.'
출처: ["testlog_26103023_DeepSeek_1M_로딩비교_진행", "artifacts/triplet/ds41-load-local-1m.yaml"]
```

```hint-event
id: 'V16'
kind: 'value-status'
노브: 'reasoning-parser'
값: 'deepseek_v41'
지위: 'inherited'
근거: '이번 저장정책 비교의 고정 손레버 또는 선례 승계값; 현재 셀에서 조정 스윕·제거 실패 대조는 수행하지 않았다.'
출처: ["testlog_26103023_DeepSeek_1M_로딩비교_진행", "artifacts/triplet/ds41-load-local-1m.yaml"]
```

```hint-event
id: 'V17'
kind: 'value-status'
노브: 'tool-call-parser'
값: 'deepseek_v41'
지위: 'inherited'
근거: '이번 저장정책 비교의 고정 손레버 또는 선례 승계값; 현재 셀에서 조정 스윕·제거 실패 대조는 수행하지 않았다.'
출처: ["testlog_26103023_DeepSeek_1M_로딩비교_진행", "artifacts/triplet/ds41-load-local-1m.yaml"]
```

```hint-event
id: 'V18'
kind: 'value-status'
노브: 'enable-auto-tool-choice'
값: 'true'
지위: 'inherited'
근거: '이번 저장정책 비교의 고정 손레버 또는 선례 승계값; 현재 셀에서 조정 스윕·제거 실패 대조는 수행하지 않았다.'
출처: ["testlog_26103023_DeepSeek_1M_로딩비교_진행", "artifacts/triplet/ds41-load-local-1m.yaml"]
```

```hint-event
id: 'V19'
kind: 'value-status'
노브: 'speculative-config'
값: '{"method":"dspark","num_speculative_tokens":3,"draft_sample_method":"greedy"}'
지위: 'inherited'
근거: '이번 저장정책 비교의 고정 손레버 또는 선례 승계값; 현재 셀에서 조정 스윕·제거 실패 대조는 수행하지 않았다.'
출처: ["testlog_26103023_DeepSeek_1M_로딩비교_진행", "artifacts/triplet/ds41-load-local-1m.yaml"]
```

```hint-event
id: 'V20'
kind: 'value-status'
노브: 'language-model-only'
값: 'true'
지위: 'inherited'
근거: '이번 저장정책 비교의 고정 손레버 또는 선례 승계값; 현재 셀에서 조정 스윕·제거 실패 대조는 수행하지 않았다.'
출처: ["testlog_26103023_DeepSeek_1M_로딩비교_진행", "artifacts/triplet/ds41-load-local-1m.yaml"]
```

```hint-event
id: 'V21'
kind: 'value-status'
노브: 'engram-config'
값: '{"cpu_offload": true, "dp_shared_memory": false, "use_thp": false}'
지위: 'inherited'
근거: '이번 저장정책 비교의 고정 손레버 또는 선례 승계값; 현재 셀에서 조정 스윕·제거 실패 대조는 수행하지 않았다.'
출처: ["testlog_26103023_DeepSeek_1M_로딩비교_진행", "artifacts/triplet/ds41-load-local-1m.yaml"]
```

설정은 저장정책 사이에서 고정했고 자동 Phase2 수렴·최소 KV·최대 동시성이라고 주장하지 않는다. cpu_offload=true도 UMA 밖의 별도 물리 메모리를 만들지 않으며 실제 file-backed mapping과 iterator skip을 함께 확인한다. gmu와 절대KV는 엔진의 현재 해석을 읽는다. 선언 값을 실제 RAM 측정으로 바꾸어 쓰지 않는다.

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
| 6 | serve | health 200 + 추론 1회(스모크 판정) | ≤ 7분 55초(상한) | 2026-10-30T15:34:53Z → 2026-10-30T15:42:48Z (upper) | docs/logs/main/events/2026-10.jsonl:952 budget_declare(label smoke-ds41-load-local-1m) · 관측된 예산 창 안의 측정  → 첫 측정 시각(output/multi/benchlog/sweep_loading_ds41-load-local-1m/lite_raw_ds41-load-local-1m.json measured_utc(호스트 date -u · lite_bench.sh 가 cold 레그 직전에 찍는 측정 시작)) | derived(스모크 사용법) |
| 7 | bench | 리포트 발행(루브릭 권한 explicit ∧ PASS일 때만 full 인증서) | 11분 38초 | 2026-10-30T15:42:48Z → 2026-10-30T15:54:26Z (exact) | output/multi/benchlog/sweep_loading_ds41-load-local-1m/lite_raw_ds41-load-local-1m.json measured_utc(호스트 date -u · lite_bench.sh 가 cold 레그 직전에 찍는 측정 시작) → sweep_index.generated_utc(호스트 UTC · 두 끝 같은 시계) | reconstructed(bench json + 측정 도구 원문 lite 인자) |

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
bash .claude/skills/upstream-version-watch/scripts/multinode_serve_smoke.sh ds41-load-local-1m --keep-up
```

**7. bench**

```sh
# bench JSON 필드로 복원: backend · model_id→--model · tokenizer_id→--tokenizer · num_prompts · max_concurrency · request_rate · burstiness · 요청당 입출력 길이 = total_*_tokens ÷ completed(나누어떨어질 때만 — 평균이다 · 요청별 길이는 JSON 에 없다) · 나누어떨어지면 `--dataset-name random` 으로 추정했다(측정 도구가 그 꼴로 부른다 — 아래 도구 원문).
# JSON 에 없는 인자(--endpoint · --base-url · --ignore-eos · --num-warmups · --temperature · --seed · --random-range-ratio · --trust-remote-code)는 측정 도구가 정했다 — level_* = run_bench.sh · lite_cold/lite_warm = lite_bench.sh(.claude/skills/adversarial-benchmark/scripts/) · 셀 스윕 순서는 sweep_bench.sh.
# lite_cold/lite_warm 의 데이터셋 · 부하 인자 = 측정 시점 도구 원문 lite_bench.sh@d33b0878a6c3 L177-L183(리터럴 인자만 · `$변수` 인자는 도구가 값을 정하지 않아 JSON 필드로 채운다) — `total_*_tokens ÷ completed` 는 실측 토큰(chat 템플릿 포함)이지 인자가 아니다.
# output/multi/benchlog/sweep_loading_ds41-load-local-1m/lite_cold_ds41-load-local-1m.json (date 20261030-154313) · 실측 토큰(chat 템플릿 포함 · total ÷ completed · 인자 아님): 입력 542/1 = 542 · 출력 128/1 = 128 · spec_decode_acceptance_length 1.67105(이 레그 JSON)
vllm bench serve --backend openai-chat --model deepseek-v4.1-flash-exl3 --tokenizer /app/quant_models/DeepSeek/DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz --trust-remote-code --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 --ignore-eos --num-prompts 1 --max-concurrency 1 --request-rate inf
# output/multi/benchlog/sweep_loading_ds41-load-local-1m/lite_warm_ds41-load-local-1m.json (date 20261030-154349) · 실측 토큰(chat 템플릿 포함 · total ÷ completed · 인자 아님): 입력 1626/3 = 542 · 출력 384/3 = 128 · spec_decode_acceptance_length 1.75799(이 레그 JSON)
vllm bench serve --backend openai-chat --model deepseek-v4.1-flash-exl3 --tokenizer /app/quant_models/DeepSeek/DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz --trust-remote-code --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 --ignore-eos --num-prompts 3 --max-concurrency 1 --request-rate inf
```

**슬롯 → 재구성 예시 컨텍스트 매핑**(예시 위치 = `output/<토폴로지>/` · 원천 = 실린 Dockerfile 의 COPY · compose 설정; 실제 실행 경로는 별도 기록 확인) — 재구성 예시 통로 `leaf-sm12x/context/` · `parent-source-core/context/` · `parent-source-core/intermediate-deps2/context/`(실제 원본·후속층의 빌드 경로 증명 아님 · 별도 실행 기록 확인)

| zip 경로 | 빌드 컨텍스트 자리 | 근거 |
|---|---|---|
| `artifacts/triplet/.env.ds41-load-local-1m` | `envs/.env.ds41-load-local-1m` | compose env_file envs/.env.${CONFIG_FILE}(셀 env) |
| `artifacts/triplet/ds41-load-local-1m.sh` | `configs/ds41-load-local-1m.sh` | compose volumes ./configs:/app/configs |
| `artifacts/triplet/ds41-load-local-1m.yaml` | `configs/ds41-load-local-1m.yaml` | compose volumes ./configs:/app/configs |
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

**기동 성공 판정(관측 · 발행 자격)**: health 200 = true · 추론 1회 = true · 근거 `output/multi/benchlog/serve_proof_ds41-load-local-1m.json` — 시점 범위: 작성 2026-10-30T15:42:25Z(output/multi/benchlog/serve_proof_ds41-load-local-1m.json 파일 mtime(파일시스템 관측 — 문서에 시각 칸이 없다 · 기록 필드 아님)) · 측정 창 대비 작성 2026-10-30T15:42:25Z < 측정 시작 2026-10-30T15:42:48Z — 측정 전(빌드·스모크)의 관측
<!-- /FACT:reproduce -->

<!-- FACT:event_timeline -->
**블랙박스 이벤트 48행**(원장에서 이 셀 label·config 에 맞는 행만 · 시각순 · 기계 발췌)

> **캠페인 경계** 2026-10-30T08:39:00Z(`camp-26103017-nas-nvme-loading` · evidence.event_campaign_boundary): campaigns/camp-26103017-nas-nvme-loading/campaign.yaml declared_utc 2026-10-30T08:39:00Z · 이 발행이 묶인 측정을 서빙한 budget_declare 창 시작 2026-10-30T15:34:53Z → 경계 = 앞선 값 2026-10-30T08:39:00Z(이전 행 = 같은 셀 이름 · 앞선 캠페인)

> **원장 관측 범위**(노드별 첫 행 → 마지막 행 · evidence.event_ledger_spans): main = 2026-07-31T03:27:43Z → 2026-10-30T15:59:14Z(`docs/logs/main/events/2026-07.jsonl` · `docs/logs/main/events/2026-08.jsonl` · `docs/logs/main/events/2026-09.jsonl` · `docs/logs/main/events/2026-10.jsonl` · `docs/logs/main/events/thermal.jsonl` · `docs/logs/main/events/watchdog.jsonl`) · sub = 2026-06-13T22:25:01Z → 2026-10-30T15:42:27Z(`sync_staging/sub_docs/logs/sub/events/2026-06.jsonl` · `sync_staging/sub_docs/logs/sub/events/2026-07.jsonl` · `sync_staging/sub_docs/logs/sub/events/2026-08.jsonl` · `sync_staging/sub_docs/logs/sub/events/2026-09.jsonl` · `sync_staging/sub_docs/logs/sub/events/2026-10.jsonl` · `sync_staging/sub_docs/logs/sub/events/thermal.jsonl` · `sync_staging/sub_docs/logs/sub/events/watchdog.jsonl` · 회수 미러가 가린 옛 로컬 사본 `docs/logs/sub/events/2026-09.jsonl`) — ⚠ sub(마지막 행 2026-10-30T15:42:27Z < 측정 끝 2026-10-30T15:54:26Z): 그 뒤 그 노드의 선언 · 사건은 **관측 범위 밖**이다(없음이 아니다 · 원장 미회수)

| # | UTC | 노드 | kind | label | 내용 | 출처 |
|---|---|---|---|---|---|---|
| 1 | 2026-10-30T13:30:48Z | main | `budget_declare` | `smoke-ds41-load-local-1m` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=17846 · weights_mib=73770 · kv_mib=8192 · overhead_mib=24800 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 기동 창 1/3: budget_renew 1회 · 닫힘 budget_clear 2026-10-30T13:39:59Z(선언 후 9m11s) · measured_utc 2026-10-30T15:54:26Z 는 이 창 밖(다른 기동 · 측정 전) | docs/logs/main/events/2026-10.jsonl:914 |
| 2 | 2026-10-30T13:30:48Z | main | `budget_honored` | `smoke-ds41-load-local-1m` | budget_honored · floor_mib=17846 · arm_ceiling_mib=14774 · remaining_s=10800 · 창 시작 2026-10-30T13:30:48Z 후 0m00s | docs/logs/main/events/watchdog.jsonl:913 |
| 3 | 2026-10-30T13:30:50Z | sub | `budget_declare` | `smoke-ds41-load-local-1m` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=17846 · weights_mib=73770 · kv_mib=8192 · overhead_mib=24800 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 기동 창 1/3: budget_renew 1회 · 닫힘 budget_clear 2026-10-30T13:39:59Z(선언 후 9m09s) · measured_utc 2026-10-30T15:54:26Z 는 이 창 밖(다른 기동 · 측정 전) | sync_staging/sub_docs/logs/sub/events/2026-10.jsonl:819 |
| 4 | 2026-10-30T13:30:51Z | sub | `budget_honored` | `smoke-ds41-load-local-1m` | budget_honored · floor_mib=17846 · arm_ceiling_mib=14774 · remaining_s=10799 · 창 시작 2026-10-30T13:30:51Z 후 0m00s | sync_staging/sub_docs/logs/sub/events/watchdog.jsonl:731 |
| 5 | 2026-10-30T13:32:05Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=85010 · rate_mib_s=24141 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → smoke-ds41-load-local-1m 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-30T13:30:48Z 후 1m17s) | docs/logs/main/events/watchdog.jsonl:914 |
| 6 | 2026-10-30T13:32:06Z | sub | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=87976 · rate_mib_s=18359 · max_rate_mib_s=17801 · legacy_rule_would_trip=true · 라벨 없음 → smoke-ds41-load-local-1m 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-30T13:30:51Z 후 1m15s) | sync_staging/sub_docs/logs/sub/events/watchdog.jsonl:732 |
| 7 | 2026-10-30T13:32:07Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=39392 · rate_mib_s=28569 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → smoke-ds41-load-local-1m 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-30T13:30:48Z 후 1m19s) | docs/logs/main/events/watchdog.jsonl:915 |
| 8 | 2026-10-30T13:38:26Z | main | `budget_renew` | `smoke-ds41-load-local-1m` | budget_renew · floor_mib=17846 · ttl_s=10800 · remaining_before_s=10342 · 창 시작 2026-10-30T13:30:48Z 후 7m38s | docs/logs/main/events/2026-10.jsonl:915 |
| 9 | 2026-10-30T13:38:27Z | sub | `budget_renew` | `smoke-ds41-load-local-1m` | budget_renew · floor_mib=17846 · ttl_s=10800 · remaining_before_s=10343 · 창 시작 2026-10-30T13:30:50Z 후 7m37s | sync_staging/sub_docs/logs/sub/events/2026-10.jsonl:820 |
| 10 | 2026-10-30T13:39:59Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-ds41-load-local-1m 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-30T13:30:48Z 후 9m11s) | docs/logs/main/events/2026-10.jsonl:916 |
| 11 | 2026-10-30T13:39:59Z | sub | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-ds41-load-local-1m 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-30T13:30:50Z 후 9m09s) | sync_staging/sub_docs/logs/sub/events/2026-10.jsonl:821 |
| 12 | 2026-10-30T13:40:00Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-ds41-load-local-1m 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-30T13:30:48Z 후 9m12s) | docs/logs/main/events/watchdog.jsonl:916 |
| 13 | 2026-10-30T13:40:00Z | sub | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-ds41-load-local-1m 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-30T13:30:51Z 후 9m09s) | sync_staging/sub_docs/logs/sub/events/watchdog.jsonl:733 |
| 14 | 2026-10-30T14:12:10Z | main | `budget_declare` | `smoke-ds41-load-local-1m` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=17846 · weights_mib=73770 · kv_mib=8192 · overhead_mib=24800 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 기동 창 2/3: budget_renew 1회 · 닫힘 budget_clear 2026-10-30T14:21:15Z(선언 후 9m05s) · measured_utc 2026-10-30T15:54:26Z 는 이 창 밖(다른 기동 · 측정 전) | docs/logs/main/events/2026-10.jsonl:924 |
| 15 | 2026-10-30T14:12:11Z | main | `budget_honored` | `smoke-ds41-load-local-1m` | budget_honored · floor_mib=17846 · arm_ceiling_mib=14774 · remaining_s=10799 · 창 시작 2026-10-30T14:12:11Z 후 0m00s | docs/logs/main/events/watchdog.jsonl:920 |
| 16 | 2026-10-30T14:12:13Z | sub | `budget_declare` | `smoke-ds41-load-local-1m` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=17846 · weights_mib=73770 · kv_mib=8192 · overhead_mib=24800 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 기동 창 2/3: budget_renew 1회 · 닫힘 budget_clear 2026-10-30T14:21:15Z(선언 후 9m02s) · measured_utc 2026-10-30T15:54:26Z 는 이 창 밖(다른 기동 · 측정 전) | sync_staging/sub_docs/logs/sub/events/2026-10.jsonl:829 |
| 17 | 2026-10-30T14:12:14Z | sub | `budget_honored` | `smoke-ds41-load-local-1m` | budget_honored · floor_mib=17846 · arm_ceiling_mib=14774 · remaining_s=10799 · 창 시작 2026-10-30T14:12:14Z 후 0m00s | sync_staging/sub_docs/logs/sub/events/watchdog.jsonl:737 |
| 18 | 2026-10-30T14:13:28Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=84570 · rate_mib_s=21715 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → smoke-ds41-load-local-1m 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-30T14:12:11Z 후 1m17s) | docs/logs/main/events/watchdog.jsonl:921 |
| 19 | 2026-10-30T14:13:30Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=38840 · rate_mib_s=25600 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → smoke-ds41-load-local-1m 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-30T14:12:11Z 후 1m19s) | docs/logs/main/events/watchdog.jsonl:922 |
| 20 | 2026-10-30T14:13:30Z | sub | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=56569 · rate_mib_s=28709 · max_rate_mib_s=17801 · legacy_rule_would_trip=true · 라벨 없음 → smoke-ds41-load-local-1m 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-30T14:12:14Z 후 1m16s) | sync_staging/sub_docs/logs/sub/events/watchdog.jsonl:738 |
| 21 | 2026-10-30T14:19:44Z | main | `budget_renew` | `smoke-ds41-load-local-1m` | budget_renew · floor_mib=17846 · ttl_s=10800 · remaining_before_s=10346 · 창 시작 2026-10-30T14:12:10Z 후 7m34s | docs/logs/main/events/2026-10.jsonl:925 |
| 22 | 2026-10-30T14:19:45Z | sub | `budget_renew` | `smoke-ds41-load-local-1m` | budget_renew · floor_mib=17846 · ttl_s=10800 · remaining_before_s=10348 · 창 시작 2026-10-30T14:12:13Z 후 7m32s | sync_staging/sub_docs/logs/sub/events/2026-10.jsonl:830 |
| 23 | 2026-10-30T14:21:15Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-ds41-load-local-1m 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-30T14:12:10Z 후 9m05s) | docs/logs/main/events/2026-10.jsonl:926 |
| 24 | 2026-10-30T14:21:15Z | sub | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-ds41-load-local-1m 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-30T14:12:13Z 후 9m02s) | sync_staging/sub_docs/logs/sub/events/2026-10.jsonl:831 |
| 25 | 2026-10-30T14:21:16Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-ds41-load-local-1m 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-30T14:12:11Z 후 9m05s) | docs/logs/main/events/watchdog.jsonl:923 |
| 26 | 2026-10-30T14:21:16Z | sub | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-ds41-load-local-1m 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-30T14:12:14Z 후 9m02s) | sync_staging/sub_docs/logs/sub/events/watchdog.jsonl:739 |
| 27 | 2026-10-30T15:34:53Z | main | `budget_declare` | `smoke-ds41-load-local-1m` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=17846 · weights_mib=73770 · kv_mib=8192 · overhead_mib=24800 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 기동 창 3/3: budget_renew 4회 · 닫힘 budget_clear 2026-10-30T15:59:13Z(선언 후 24m20s) · measured_utc 2026-10-30T15:54:26Z 가 이 창 안(이 측정의 기동) | docs/logs/main/events/2026-10.jsonl:952 |
| 28 | 2026-10-30T15:34:54Z | main | `budget_honored` | `smoke-ds41-load-local-1m` | budget_honored · floor_mib=17846 · arm_ceiling_mib=14774 · remaining_s=10799 · 창 시작 2026-10-30T15:34:54Z 후 0m00s | docs/logs/main/events/watchdog.jsonl:938 |
| 29 | 2026-10-30T15:34:56Z | sub | `budget_declare` | `smoke-ds41-load-local-1m` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=17846 · weights_mib=73770 · kv_mib=8192 · overhead_mib=24800 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 기동 창 3/3: budget_renew 1회 · 닫힘 미관측(창이 열린 채 원장이 끝난다) · measured_utc 2026-10-30T15:54:26Z 는 이 창의 관측 범위 밖(창이 열린 채 스트림이 2026-10-30T15:42:27Z 에서 끝난다 — 이 측정의 기동으로 귀속하지 않는다) | sync_staging/sub_docs/logs/sub/events/2026-10.jsonl:857 |
| 30 | 2026-10-30T15:34:57Z | sub | `budget_honored` | `smoke-ds41-load-local-1m` | budget_honored · floor_mib=17846 · arm_ceiling_mib=14774 · remaining_s=10799 · 창 시작 2026-10-30T15:34:57Z 후 0m00s | sync_staging/sub_docs/logs/sub/events/watchdog.jsonl:754 |
| 31 | 2026-10-30T15:36:08Z | cluster | `engine_log_window_start` | — | 캡처된 엔진 로그의 첫 시각(꼬리 캡처일 수 있다 — 엔진 시작 시각이 아닐 수 있다) · 선언 2026-10-30T15:34:53Z 후 1m15s · 시계: 컨테이너 시각을 UTC 로 읽어 측정 창 [2026-10-30T15:34:53Z, 2026-10-30T15:59:13Z] 안에 드는 것을 확인 | output/multi/benchlog/sweep_loading_ds41-load-local-1m/lite_engine_ds41-load-local-1m.log:250 |
| 32 | 2026-10-30T15:36:11Z | sub | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=89918 · rate_mib_s=23087 · max_rate_mib_s=17801 · legacy_rule_would_trip=true · 라벨 없음 → smoke-ds41-load-local-1m 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-30T15:34:57Z 후 1m14s) | sync_staging/sub_docs/logs/sub/events/watchdog.jsonl:755 |
| 33 | 2026-10-30T15:36:12Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=60505 · rate_mib_s=23994 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → smoke-ds41-load-local-1m 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-30T15:34:54Z 후 1m18s) | docs/logs/main/events/watchdog.jsonl:939 |
| 34 | 2026-10-30T15:36:13Z | sub | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=49505 · rate_mib_s=25836 · max_rate_mib_s=17801 · legacy_rule_would_trip=true · 라벨 없음 → smoke-ds41-load-local-1m 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-30T15:34:57Z 후 1m16s) | sync_staging/sub_docs/logs/sub/events/watchdog.jsonl:756 |
| 35 | 2026-10-30T15:36:14Z | cluster | `engine_prefetch_disabled` | — | 가중치 auto-prefetch 꺼짐 줄 ×4 — filesystem EXT4(vLLM 로그 문구) | output/multi/benchlog/sweep_loading_ds41-load-local-1m/lite_engine_ds41-load-local-1m.log:465 |
| 36 | 2026-10-30T15:38:52Z | cluster | `engine_weights_loaded` | — | 가중치 적재 완료 줄 ×4(rank·단계별) · 최장 121.99s · 이 행 = 마지막 줄 | output/multi/benchlog/sweep_loading_ds41-load-local-1m/lite_engine_ds41-load-local-1m.log:631 |
| 37 | 2026-10-30T15:39:07Z | cluster | `engine_model_loaded` | — | 모델 적재 완료 줄 ×2(rank 별) · 마지막 줄 73.96 GiB · 178.8s | output/multi/benchlog/sweep_loading_ds41-load-local-1m/lite_engine_ds41-load-local-1m.log:644 |
| 38 | 2026-10-30T15:40:10Z | cluster | `engine_shm_broadcast_wait` | — | shm_broadcast 대기 경고(60s 단위) ×1 · 2026-10-30T15:40:10Z–2026-10-30T15:40:10Z(0m00s) — vLLM 문구상 프로세스 hang 또는 긴 작업(컴파일·가중치/KV 양자화) 중 출현 | output/multi/benchlog/sweep_loading_ds41-load-local-1m/lite_engine_ds41-load-local-1m.log:651 |
| 39 | 2026-10-30T15:41:01Z | cluster | `engine_kv_cache_sized` | — | GPU KV cache 2,608,092 tokens · 요청당 1,048,576 tokens 기준 최대 동시성 2.49x | output/multi/benchlog/sweep_loading_ds41-load-local-1m/lite_engine_ds41-load-local-1m.log:686 |
| 40 | 2026-10-30T15:41:42Z | cluster | `engine_init_done` | — | 엔진 초기화(profile·KV 할당·warmup) 153.47s | output/multi/benchlog/sweep_loading_ds41-load-local-1m/lite_engine_ds41-load-local-1m.log:715 |
| 41 | 2026-10-30T15:41:58Z | cluster | `server_starting` | — | API 서버 기동 — 이 측정 창 선언 2026-10-30T15:34:53Z 후 7m05s · 캡처 로그 첫 시각 2026-10-30T15:36:08Z 후 5m50s | output/multi/benchlog/sweep_loading_ds41-load-local-1m/lite_engine_ds41-load-local-1m.log:726 |
| 42 | 2026-10-30T15:42:26Z | main | `budget_renew` | `smoke-ds41-load-local-1m` | budget_renew · floor_mib=17846 · ttl_s=10800 · remaining_before_s=10347 · 창 시작 2026-10-30T15:34:53Z 후 7m33s | docs/logs/main/events/2026-10.jsonl:953 |
| 43 | 2026-10-30T15:42:27Z | sub | `budget_renew` | `smoke-ds41-load-local-1m` | budget_renew · floor_mib=17846 · ttl_s=10800 · remaining_before_s=10349 · 창 시작 2026-10-30T15:34:56Z 후 7m31s | sync_staging/sub_docs/logs/sub/events/2026-10.jsonl:858 |
| 44 | 2026-10-30T15:43:51Z | main | `budget_renew` | `smoke-ds41-load-local-1m` | budget_renew · floor_mib=17846 · ttl_s=7200 · remaining_before_s=10715 · 창 시작 2026-10-30T15:34:53Z 후 8m58s | docs/logs/main/events/2026-10.jsonl:954 |
| 45 | 2026-10-30T15:47:33Z | main | `budget_renew` | `smoke-ds41-load-local-1m` | budget_renew · floor_mib=17846 · ttl_s=7200 · remaining_before_s=6978 · 창 시작 2026-10-30T15:34:53Z 후 12m40s | docs/logs/main/events/2026-10.jsonl:955 |
| 46 | 2026-10-30T15:51:02Z | main | `budget_renew` | `smoke-ds41-load-local-1m` | budget_renew · floor_mib=17846 · ttl_s=7200 · remaining_before_s=6991 · 창 시작 2026-10-30T15:34:53Z 후 16m09s | docs/logs/main/events/2026-10.jsonl:956 |
| 47 | 2026-10-30T15:59:13Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-ds41-load-local-1m 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-30T15:34:53Z 후 24m20s) | docs/logs/main/events/2026-10.jsonl:957 |
| 48 | 2026-10-30T15:59:14Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-ds41-load-local-1m 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-30T15:34:54Z 후 24m20s) | docs/logs/main/events/watchdog.jsonl:940 |

**기동 시도**(예산 선언 `budget_declare` 마다 한 시도 · 같은 노드의 이어지는 행과 선언 없는 노드(엔진 로그)의 그 시각 행을 묶었다 · 사살 · 트립 · 거부는 닫힘과 따로 센다 · `measurement` 행은 측정 기록이라 시도로 세지 않는다(위 표에만) — 해석은 저작자 몫)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 1 | main | `smoke-ds41-load-local-1m` | 2026-10-30T13:30:48Z | 1회 · 첫 2026-10-30T13:38:26Z | 없음 | `budget_clear` 2026-10-30T13:39:59Z | 4 |
| 2 | sub | `smoke-ds41-load-local-1m` | 2026-10-30T13:30:50Z | 1회 · 첫 2026-10-30T13:38:27Z | 없음 | `budget_clear` 2026-10-30T13:39:59Z | 3 |
| 3 | main | `smoke-ds41-load-local-1m` | 2026-10-30T14:12:10Z | 1회 · 첫 2026-10-30T14:19:44Z | 없음 | `budget_clear` 2026-10-30T14:21:15Z | 4 |
| 4 | sub | `smoke-ds41-load-local-1m` | 2026-10-30T14:12:13Z | 1회 · 첫 2026-10-30T14:19:45Z | 없음 | `budget_clear` 2026-10-30T14:21:15Z | 3 |
| 5 | main | `smoke-ds41-load-local-1m` | 2026-10-30T15:34:53Z | 4회 · 첫 2026-10-30T15:42:26Z | 없음 | `budget_clear` 2026-10-30T15:59:13Z | 3 |
| 6 | sub | `smoke-ds41-load-local-1m` | 2026-10-30T15:34:56Z | 1회 · 첫 2026-10-30T15:42:27Z | 없음 | —(닫힘 행 없음) | 11 |
<!-- /FACT:event_timeline -->

<!-- FACT:measurement_env -->
**측정 실행의 env 관측 38행**(측정한 실행의 엔진 로그가 echo 한 값 · 기계 발췌 — 측정 당시 값의 정본이다. 표에 없는 키는 로그가 echo 하지 않은 것이지 설정되지 않았다는 뜻이 아니다 · 측정 뒤 재생성된 env 형상은 01 §1.1 을 본다) — 그중 4행은 종류 `key-present`: 키가 프로세스 env 에 **있었다**는 관측뿐이고 값은 로그에 없다(값 미관측)

| # | 키 | 값 | 노드 | 종류 | 출처(첫 줄) | 관측 |
|---|---|---|---|---|---|---|
| 1 | `VLLM_PLE_MMAP` | (키 존재 · 값 미관측) | 미특정 | key-present | output/multi/benchlog/sweep_loading_ds41-load-local-1m/lite_engine_head_ds41-load-local-1m.log:L65 | 로그 1개 |
| 2 | `VLLM_PLE_MMAP_DIR` | (키 존재 · 값 미관측) | 미특정 | key-present | output/multi/benchlog/sweep_loading_ds41-load-local-1m/lite_engine_head_ds41-load-local-1m.log:L66 | 로그 1개 |
| 3 | `VLLM_REF` | (키 존재 · 값 미관측) | 미특정 | key-present | output/multi/benchlog/sweep_loading_ds41-load-local-1m/lite_engine_head_ds41-load-local-1m.log:L67 | 로그 1개 |
| 4 | `VLLM_EXL3_MOE_KERNEL` | (키 존재 · 값 미관측) | 미특정 | key-present | output/multi/benchlog/sweep_loading_ds41-load-local-1m/lite_engine_head_ds41-load-local-1m.log:L68 | 로그 1개 |
| 5 | `NCCL_SOCKET_IFNAME` | `<nic:cluster>` | main | env-echo | output/multi/benchlog/sweep_loading_ds41-load-local-1m/lite_engine_head_ds41-load-local-1m.log:L103 | 로그 1개 |
| 6 | `NCCL_SOCKET_IFNAME` | `<nic:cluster>` | main | param-echo | output/multi/benchlog/sweep_loading_ds41-load-local-1m/lite_engine_head_ds41-load-local-1m.log:L104 | 로그 1개 |
| 7 | `nccl:version` | `2.30.7+cuda13.3` | main | runtime | output/multi/benchlog/sweep_loading_ds41-load-local-1m/lite_engine_head_ds41-load-local-1m.log:L107 | 로그 1개 |
| 8 | `NCCL_NET_PLUGIN` | `spcx` | main | env-echo | output/multi/benchlog/sweep_loading_ds41-load-local-1m/lite_engine_head_ds41-load-local-1m.log:L109 | 로그 1개 |
| 9 | `NCCL_IB_DISABLE` | `0` | main | env-echo | output/multi/benchlog/sweep_loading_ds41-load-local-1m/lite_engine_head_ds41-load-local-1m.log:L114 | 로그 1개 |
| 10 | `NCCL_IB_HCA` | `=<nic:cluster>,<nic:cluster>` | main | param-echo | output/multi/benchlog/sweep_loading_ds41-load-local-1m/lite_engine_head_ds41-load-local-1m.log:L116 | 로그 1개 |
| 11 | `NCCL_IB_MERGE_NICS` | `1` | main | env-echo | output/multi/benchlog/sweep_loading_ds41-load-local-1m/lite_engine_head_ds41-load-local-1m.log:L119 | 로그 1개 |
| 12 | `nccl:net_ib_devices` | `[0]<nic:cluster>:1/RoCE [1]<nic:cluster>:1/RoCE [RO]; OOB <nic:cluster>:<node:main><0>` | main | runtime | output/multi/benchlog/sweep_loading_ds41-load-local-1m/lite_engine_head_ds41-load-local-1m.log:L122 | 로그 1개 |
| 13 | `NCCL_IB_QPS_PER_CONNECTION` | `4` | main | env-echo | output/multi/benchlog/sweep_loading_ds41-load-local-1m/lite_engine_head_ds41-load-local-1m.log:L135 | 로그 1개 |
| 14 | `nccl:network` | `IB` | sub | runtime | output/multi/benchlog/sweep_loading_ds41-load-local-1m/lite_engine_head_ds41-load-local-1m.log:L158 | 로그 2개 |
| 15 | `NCCL_DMABUF_ENABLE` | `0` | sub | env-echo | output/multi/benchlog/sweep_loading_ds41-load-local-1m/lite_engine_head_ds41-load-local-1m.log:L160 | 로그 1개 |
| 16 | `NCCL_CROSS_NIC` | `1` | sub | env-echo | output/multi/benchlog/sweep_loading_ds41-load-local-1m/lite_engine_head_ds41-load-local-1m.log:L163 | 로그 1개 |
| 17 | `NCCL_MAX_NCHANNELS` | `8` | sub | env-echo | output/multi/benchlog/sweep_loading_ds41-load-local-1m/lite_engine_head_ds41-load-local-1m.log:L334 | 로그 2개 |
| 18 | `NCCL_LL128_BUFFSIZE` | `262144` | sub | env-echo | output/multi/benchlog/sweep_loading_ds41-load-local-1m/lite_engine_head_ds41-load-local-1m.log:L344 | 로그 2개 |
| 19 | `NCCL_BUFFSIZE` | `1048576` | main | env-echo | output/multi/benchlog/sweep_loading_ds41-load-local-1m/lite_engine_head_ds41-load-local-1m.log:L356 | 로그 2개 |
| 20 | `NCCL_PROTO` | `^LL128` | main | env-echo | output/multi/benchlog/sweep_loading_ds41-load-local-1m/lite_engine_head_ds41-load-local-1m.log:L375 | 로그 2개 |
| 21 | `NCCL_IB_SPLIT_DATA_ON_QPS` | `0` | main | env-echo | output/multi/benchlog/sweep_loading_ds41-load-local-1m/lite_engine_head_ds41-load-local-1m.log:L407 | 로그 2개 |
| 22 | `NCCL_IB_GID_INDEX` | `3` | main | env-echo | output/multi/benchlog/sweep_loading_ds41-load-local-1m/lite_engine_head_ds41-load-local-1m.log:L416 | 로그 2개 |
| 23 | `NCCL_SOCKET_IFNAME` | `<nic:cluster>` | sub | env-echo | output/multi/benchlog/sweep_loading_ds41-load-local-1m/lite_engine_head_ds41-load-local-1m.log:L555 | 로그 2개 · Ray 접힘 +3(접힌 사본의 노드 미관측) |
| 24 | `NCCL_SOCKET_IFNAME` | `<nic:cluster>` | sub | param-echo | output/multi/benchlog/sweep_loading_ds41-load-local-1m/lite_engine_head_ds41-load-local-1m.log:L556 | 로그 2개 · Ray 접힘 +2(접힌 사본의 노드 미관측) |
| 25 | `nccl:version` | `2.30.7+cuda13.3` | sub | runtime | output/multi/benchlog/sweep_loading_ds41-load-local-1m/lite_engine_head_ds41-load-local-1m.log:L559 | 로그 2개 |
| 26 | `NCCL_NET_PLUGIN` | `spcx` | sub | env-echo | output/multi/benchlog/sweep_loading_ds41-load-local-1m/lite_engine_head_ds41-load-local-1m.log:L561 | 로그 2개 |
| 27 | `NCCL_IB_DISABLE` | `0` | sub | env-echo | output/multi/benchlog/sweep_loading_ds41-load-local-1m/lite_engine_head_ds41-load-local-1m.log:L566 | 로그 2개 · Ray 접힘 +2(접힌 사본의 노드 미관측) |
| 28 | `NCCL_IB_HCA` | `=<nic:cluster>,<nic:cluster>` | sub | param-echo | output/multi/benchlog/sweep_loading_ds41-load-local-1m/lite_engine_head_ds41-load-local-1m.log:L567 | 로그 2개 · Ray 접힘 +2(접힌 사본의 노드 미관측) |
| 29 | `NCCL_IB_MERGE_NICS` | `1` | sub | env-echo | output/multi/benchlog/sweep_loading_ds41-load-local-1m/lite_engine_head_ds41-load-local-1m.log:L569 | 로그 2개 |
| 30 | `nccl:net_ib_devices` | `[RO]; OOB <nic:cluster>:<node:main><0>` | main | runtime | output/multi/benchlog/sweep_loading_ds41-load-local-1m/lite_engine_head_ds41-load-local-1m.log:L571 | 로그 2개 · Ray 접힘 +3(접힌 사본의 노드 미관측) |
| 31 | `NCCL_IB_QPS_PER_CONNECTION` | `4` | sub | env-echo | output/multi/benchlog/sweep_loading_ds41-load-local-1m/lite_engine_head_ds41-load-local-1m.log:L581 | 로그 2개 · Ray 접힘 +3(접힌 사본의 노드 미관측) |
| 32 | `NCCL_DMABUF_ENABLE` | `0` | main | env-echo | output/multi/benchlog/sweep_loading_ds41-load-local-1m/lite_engine_head_ds41-load-local-1m.log:L596 | 로그 2개 |
| 33 | `NCCL_CROSS_NIC` | `1` | main | env-echo | output/multi/benchlog/sweep_loading_ds41-load-local-1m/lite_engine_head_ds41-load-local-1m.log:L599 | 로그 2개 |
| 34 | `NCCL_MAX_NCHANNELS` | `8` | main | env-echo | output/multi/benchlog/sweep_loading_ds41-load-local-1m/lite_engine_ds41-load-local-1m.log:L369 | 로그 1개 |
| 35 | `NCCL_LL128_BUFFSIZE` | `262144` | main | env-echo | output/multi/benchlog/sweep_loading_ds41-load-local-1m/lite_engine_ds41-load-local-1m.log:L372 | 로그 1개 |
| 36 | `NCCL_BUFFSIZE` | `1048576` | sub | env-echo | output/multi/benchlog/sweep_loading_ds41-load-local-1m/lite_engine_ds41-load-local-1m.log:L373 | 로그 1개 |
| 37 | `NCCL_IB_SPLIT_DATA_ON_QPS` | `0` | sub | env-echo | output/multi/benchlog/sweep_loading_ds41-load-local-1m/lite_engine_ds41-load-local-1m.log:L396 | 로그 1개 |
| 38 | `NCCL_IB_GID_INDEX` | `3` | sub | env-echo | output/multi/benchlog/sweep_loading_ds41-load-local-1m/lite_engine_ds41-load-local-1m.log:L448 | 로그 1개 |

- 값 주: 기계 치환(pii.substitution_table · 장치·호스트·주소는 자리표시 — 원문은 출처 줄)
- 값 주: 키 존재만 관측 · 값 미관측(vLLM 'Unknown vLLM environment variable detected' 경고 — 값은 로그에 없다)

- Ray 접힘: 출처 줄 끝의 `[repeated Nx across cluster]` — Ray 가 다른 프로세스의 같은 줄 N개를 한 줄로 접었다. 노드 칸은 **첫 줄**의 노드이고, 접힌 사본이 어느 노드의 것인지는 로그에 없다(관측 대상 밖).

- 로그 창(꼬리): `tail-800(lite_bench.sh@d33b0878a6c3 L234)`(꼬리 창 출처 로그 1개 · 이 표 행 #34–38) — 이 캡처는 엔진 출력의 **마지막 N줄**이다(측정 도구의 `tail -N` 상한에 닿았다 · 생산자 evidence `log_window`). 창 앞에서 echo 한 줄은 그 로그에 없다 — 이 주의는 꼬리 창 출처 행에만 걸린다(머리 캡처 행은 아래).

- 로그 창(머리): `head-600(output/multi/benchlog/sweep_loading_ds41-load-local-1m/lite_raw_ds41-load-local-1m.json engine_log_head_lines)`(머리 캡처 출처 로그 1개 · 이 표 행 #1–33) — 이 캡처는 엔진 출력의 **처음 N줄**이다(lite_bench.sh 머리 캡처 `head -n N` 상한에 닿았다 · 생산자 evidence `log_head_window`). 꼬리 창의 주의는 이 행에 걸리지 않는다 · 머리 N줄 뒤에서 echo 한 줄은 그 로그에 없다. 머리 캡처와 꼬리 창 사이(엔진 출력의 가운데)는 어느 캡처에도 없다 — 표에 없는 키 · 한 노드에만 있는 키는 그 사이였을 수 있다.
<!-- /FACT:measurement_env -->

> 이 절이 답하는 질문: 수신자가 이 zip 만으로 이 셀을 다시 띄우려면 어떤 순서로 무엇을 하고, 각 단계의 성공을 무엇으로 판정하며, 얼마나 기다려야 하는가?

1. 각 노드의 NVMe에 같은56파일 payload를 준비하고 원본과 파일명·크기·SHA256을 대조한다(1.6). recipient의 project mount selector를 각 로컬 root로 채우고 두 컨테이너의 실제 ro bind를 확인한다. 이 대표 셀은 NFS를 필요로 하지 않는다.
2. 재현 표의 parent/core/deps2/leaf 범위를 읽는다. 새 재구성 명령은 generated-unverified다. 현재 사용 이미지가 없는 환경에서는 양 노드 로컬 빌드·native API·TP2 smoke를 새로 수행하고 이미지 save/load로 대체하지 않는다.
3. weights/KV/overhead 예산을 각 노드에서 선언·honored 후 head→worker→GPU join→vllm 순서로 기동한다. 양 노드 SHA/torch/driver와 선택된 Engram source를 확인한다. source83의 DSV41_SOURCE와 hf table dir가 모델 mount를 따라야 한다.
4. 기동 시간은 현재 testlog의 세 local 회차와 raw command marker→첫 health200이다. preflight/Ray/Compose는 타이머 밖이고 JIT/KV/API 초기화는 안이다. health200·1M 모델 목록·비어 있지 않은 짧은 추론과 file-backed/iterator skip/Engram prestage 경로를 실제 로그로 확인한다. checkpoint 순회 뒤 로그 공백이나 shared-memory 경고만으로 hang를 단정하지 않는다.
5. lite pass 후 같은 서버에서 GuideLLM full 세 반복을 수행한 조건은03에 있다. 완료 후 canonical down이 컨테이너·감시·TTL·예산을 회수한다. 실제 기동 시도와 갱신 횟수는 위 원장 표에 따르며 측정 행을 새 기동으로 세지 않는다. 선언 바닥과 엔진이 보고한73.96GiB model+draft, KV 토큰 용량은 서로 다른 근거다.

## 1.5 서브 레시피 해설
<!-- FACT:sub_recipe -->
원본: `artifacts/compose/sub_recipe.json`(기계 파생 — 스모크와 같은 함수 `slave_forward.derive` 의 출력).

| 역할 | 하는 일 |
|---|---|
| `master` | ray head + vllm serve(트리플렛 source · 모델 러너 실행) |
| `master_runtime_patch` | none(런타임 패치 파일 없음 — CONFIG_FILE='ds41-load-local-1m' → ds41-load-local-1m_patch.py 부재) |
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
| `source` | output/multi/benchlog/attestation_ds41-load-local-1m.json |

> **attestation 범위** — config `ds41-load-local-1m` · phase serve · 작성 2026-10-30T15:42:25Z(output/multi/benchlog/attestation_ds41-load-local-1m.json 파일 mtime(파일시스템 관측 — 문서에 시각 칸이 없다 · 기록 필드 아님)) · 묶은 근거 cell-name · 파일 `output/multi/benchlog/attestation_ds41-load-local-1m.json` · 측정 창 대비 작성 2026-10-30T15:42:25Z < 측정 시작 2026-10-30T15:42:48Z — 측정 전(빌드·스모크)의 관측: **same config(ds41-load-local-1m) · serve — 원장 순서로 판별한 같은 기동의 관측(결속된 스모크 로그 미확인 · 제한검색 · 약한 판별)(판별: 원장 순서로 판별(결속된 스모크 로그 미확인 · 제한검색 — docs/simlog/**/*serve*.log 에서 이 attestation 을 적은 로그 미확인; 다른 경로의 보존 여부는 판정하지 않음): 원장 docs/logs/main/events/2026-10.jsonl:952 budget_declare(smoke-ds41-load-local-1m · 2026-10-30T15:34:53Z) ≤ attestation 작성 2026-10-30T15:42:25Z · serve_proof 작성 2026-10-30T15:42:25Z < 측정 시작 2026-10-30T15:42:48Z · 그 선언이 측정 끝 2026-10-30T15:54:26Z 까지 재선언 · 창 닫힘 없이 이어짐(main 원장 기록 범위 안) · API 서버 기동 2026-10-30T15:41:58Z(output/multi/benchlog/sweep_loading_ds41-load-local-1m/lite_engine_ds41-load-local-1m.log:726 · 컨테이너 시계 = UTC 가정) ∈ [선언, attestation 작성] · 측정 엔진 로그 APIServer pid 1 하나)**
<!-- /FACT:sub_recipe -->

<!-- FACT:measurement_env_nodes -->
**노드별 측정 env**(01 §1.4 표를 키 × 노드로 접은 것 · `만 관측` = 다른 쪽 줄이 캡처 로그에 없다 · Ray 가 같은 줄을 접은 키(`[repeated Nx across cluster]`)와 머리·꼬리 캡처 로그(01 §1.4 로그 창)에서 한쪽만 남은 키는 다른 쪽 부재를 단언하지 않는다)

- 양 노드 같은 값 14개: `NCCL_BUFFSIZE` · `NCCL_CROSS_NIC` · `NCCL_DMABUF_ENABLE` · `NCCL_IB_DISABLE` · `NCCL_IB_GID_INDEX` · `NCCL_IB_HCA` · `NCCL_IB_MERGE_NICS` · `NCCL_IB_QPS_PER_CONNECTION` · `NCCL_IB_SPLIT_DATA_ON_QPS` · `NCCL_LL128_BUFFSIZE` · `NCCL_MAX_NCHANNELS` · `NCCL_NET_PLUGIN` · `NCCL_SOCKET_IFNAME` · `nccl:version`

| 키 | main | sub | 노드 미특정 | 판정 |
|---|---|---|---|---|
| `NCCL_PROTO` | `^LL128` | — | — | main 만 캡처 창에 남음 — sub 여부 미관측(머리 캡처) |
| `VLLM_EXL3_MOE_KERNEL` | — | — | (키 존재 · 값 미관측) | 노드 미특정 |
| `VLLM_PLE_MMAP` | — | — | (키 존재 · 값 미관측) | 노드 미특정 |
| `VLLM_PLE_MMAP_DIR` | — | — | (키 존재 · 값 미관측) | 노드 미특정 |
| `VLLM_REF` | — | — | (키 존재 · 값 미관측) | 노드 미특정 |
| `nccl:net_ib_devices` | `[0]<nic:cluster>:1/RoCE [1]<nic:cluster>:1/RoCE [RO]; OOB <nic:cluster>:<node:main><0>` · `[RO]; OOB <nic:cluster>:<node:main><0>` | — | — | main 만 줄에 남음 · Ray 접힘 +3 — sub 사본 여부 미관측 |
| `nccl:network` | — | `IB` | — | sub 만 캡처 창에 남음 — main 여부 미관측(머리 캡처) |
<!-- /FACT:measurement_env_nodes -->

> 이 절이 답하는 질문: 서브(Ray worker) 노드는 메인과 무엇이 다르고, 무엇을 반드시 같게 맞춰야 하는가?

master는 Ray head와 vllm serve를 실행하고 slave는 head에 합류하는 worker다. 모델 트리플렛은 메인이 소유하며 worker가 독립 서버/벤치/hint를 띄우지 않는다. 이번 셀에는 별도 runtime patch 파일이 없다.
이미지 정체성·빌드 튜닝·클러스터 이름·마운트·DSV41/EXL3 runtime env의 전달 이유와 값은 sub_recipe와 측정 env 관측 표를 따른다. 특히 worker도 자기 모델 파일과 Engram 테이블을 읽으므로 같은 컨테이너 상대 경로에 같은 payload가 있어야 한다. forwarder가 main project mount selector를 보내므로 worker에서도 전달된 동일 호스트 경로가 실재해야 한다.
attestation의 vLLM SHA·torch·driver는 equal이지만 image ID와 build-ledger identity는 differ다. 동일한 전체 빌드라고 확대하지 않으며 노드별 원장·관측 범위 제한을 유지한다.
실제 NCCL 환경과 네트워크 선택은 위 노드별 측정 표를 인용한다. NCCL tail env는 source env와 actor에 전달되는 경로를 함께 읽는다. Ray repeated·머리/꼬리 캡처에서 한쪽만 남은 줄은 다른 쪽 설정 부재를 뜻하지 않는다. recipient NIC/주소는 manifest 좌표로 채운다.

## 1.6 재현 절차 — 기동 전 준비(사람이 할 일)
> 1.4 의 명령을 치기 **전에** 사람이 손으로 해야 하는 것이다 — 가중치 획득 · 스테이징 생성 · 네트워크가 필요한 단계. 에이전트가 대신 할 수 없는 자리(다운로드 승인 · 물리 경로 · 외부 계정)가 여기 모인다.

> 이 절이 답하는 질문: 이 셀을 기동하기 전에 사람이 준비해야 하는 것은 무엇이며, 각각을 어떤 명령으로 만들고, 그 절차의 출처는 어디인가?

### 가중치 획득과 복제
이번에는 기존 관리 NAS의 DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz를 복제했다. 현재 HF metadata revision982b70452f399814f56b46272fd30394ae10d58c, runtime56파일357486881279B와 실제 SHA 검증이 있다. 과거 hint의 HF.git 부재를 현재 revision 미관측이라고 승계하지 않는다. 외부 저자 pack과의 정확한 바이트 동등은 별개로 미확정이다.
양 노드에서 `rsync -aL --partial --exclude=/.git --exclude=/.cache <manifest.quant_model_path>/DeepSeek/DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz/ <owned-local-root>/DeepSeek/DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz/` 형상으로 복제한 뒤 목록·바이트·실제 SHA256을 확인한다. 실행된 복제/검증의 환경 좌표를 자리표시한 절차이며 준비 testlog와 staging_complete가 근거다. 새 다운로드는 사람 승인 뒤 별도 수행한다.
### Engram과 위치 조건
현재는 모델 payload 안의 table을 source로 사용한다. 별도 PLE mmap 생성은 해당 없음이며 Engram file-backed row lookup과 PLE mmap을 섞지 않는다. `DSV41_SOURCE` 및 hf engram_table_dir의 컨테이너 경로는 실제 선택된 mount의 모델 root와 같아야 한다. 원문은 이번 devlog의 입력 정합 기록과83/87/88이다. 원본테이블의 전체 크기, file mapping, page cache와 GPU/UMA 상주량을 구분한다.
### 외부 접속과 오프라인 준비
이번 실험은 사전 확보된 모델·이미지를 사용했다. 새 parent/leaf 빌드에는 base 및 고정 wheel/source 취득이 필요하며 슬롯의 핀·해시·원장을 따른다. 오프라인이면 승인된 동일 payload와 base/wheel을 미리 확보한다. parent 재구성의 미실행 상태는 유지하고 실제 새 빌드 성공으로 바꾸어 쓰지 않는다.
