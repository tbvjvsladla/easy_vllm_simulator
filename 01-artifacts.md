# 01 · 산출물 — 무엇이 실제로 쓰였고, 어떻게 다시 띄우는가

## 1.1 적용 판정 표
> 기계가 적용 증거(빌드 원장 · 원장이 없으면 라벨된 재구성) × 파일 × 선언을 대사한 결과다. `artifacts/` 에는 **적용된 것만** 실렸다 — 빌드 때 스스로 skip 된 패치와 쓰이지 않은 평면의 파일은 표에만 남고 싣지 않는다.

<!-- FACT:slots -->
| 슬롯 | 실림 | 파일 | 신호 | 적용 증거 | 사유 |
|---|---|---|---|---|---|
| `build_patch_post` | 실림 | `artifacts/build_patch_post/10-deepgemm.sh` · `artifacts/build_patch_post/20-triton-kernels.sh` · `artifacts/build_patch_post/30-mxfp4-triton-sm121.sh` · `artifacts/build_patch_post/40-humming-nvml-gb10.sh` · `artifacts/build_patch_post/70-exllamav3-build.sh` · `artifacts/build_patch_post/72-exl3-row-store.sh` · `artifacts/build_patch_post/73-vllm-exl3-build.sh` · `artifacts/build_patch_post/74-exl3-moe-prep-fused.sh` · `artifacts/build_patch_post/79-exl3-port-selfcheck.sh` | 2-signal | build-ledger:attestation:output/multi/benchlog/attestation_i5-a-kern.json#main | build_patches/ — 적용 판정 build-ledger: applied 9 · 실린 것 = skip 아닌 패치(기계 파생) |
| `build_patch_pre` | 실림 | `artifacts/build_patch_pre/55-src-deps-authority.sh` · `artifacts/build_patch_pre/80-exl3-quant-method.sh` · `artifacts/build_patch_pre/81-exl3-model-wiring.sh` · `artifacts/build_patch_pre/82-exl3-h2d-stage.sh` · `artifacts/build_patch_pre/83-engram-file-backend.sh` · `artifacts/build_patch_pre/84-sm12x-block-shim.sh` · `artifacts/build_patch_pre/85-exl3-serial-streams.sh` · `artifacts/build_patch_pre/86-exl3-worker-hooks.sh` · `artifacts/build_patch_pre/87-engram-embed-skip.sh` · `artifacts/build_patch_pre/88-engram-prestage.sh` · `artifacts/build_patch_pre/89-kv-null-block-zero.sh` · `artifacts/build_patch_pre/90-woa-fp8-einsum.sh` · `artifacts/build_patch_pre/91-mxfp8-prefer-b12x.sh` · `artifacts/build_patch_pre/92-dense-gemv.sh` | 2-signal | build-ledger:attestation:output/multi/benchlog/attestation_i5-a-kern.json#main | build_patches_src/ — 적용 판정 build-ledger: applied 14 · 실린 것 = skip 아닌 패치(기계 파생) |
| `build_recipe` | 실림 | `artifacts/build_recipe/Dockerfile.source-build` · `artifacts/build_recipe/requirements.txt` | 3-signal | build-ledger:build-ledger.dockerfile | 쓰인 레시피(선택자 `Dockerfile.source-build` · docker history 대조 17/17)와 그것이 COPY 하는 requirements.txt(원장 sha256 일치)다. source-build 트랙인 이유는 00 §0.4(1). `SM12X_PORT=1` build-arg 가 이식 패치 전체의 게이트다. 공유 이미지 레시피이지 모델 전용 파일이 아니다. |
| `compose` | 실림 | `artifacts/compose/.env.cluster.template` · `artifacts/compose/.env.interconnect.template` · `artifacts/compose/.env.template` · `artifacts/compose/docker-compose.yaml` · `artifacts/compose/serve_runner.sh` · `artifacts/compose/sub_recipe.json` | 2-signal | file:output/multi/docker-compose.yaml | 멀티 서빙 경로(측정 당시 개정). serve_runner.sh 는 `NODE_ROLE` 로 master(Ray head + vllm serve)와 slave(Ray worker)를 가르고, 멀티 필수 노브 `distributed-executor-backend` 가 yaml 에 없으면 fail-loud 로 멈춘다(§L51-63). env 형상은 값이 자리표시로 바뀐 사본이다. |
| `fork_pin` | 안 실림 | — | 1-signal | none:.claude/policies/arch_variant_ledger.json | 셀 env 에 VARIANT 줄 없음 = stock(workflow.md §변종 좌표의 거처)(기계 파생) |
| `runtime_patch` | 안 실림 | — | 1-signal | none:output/multi/configs/i5-a-kern_patch.py | output/multi/configs/i5-a-kern_patch.py 없음 — arm_patch.sh 는 no-op(런타임 패치 불해당)(기계 파생) |
| `triplet` | 실림 | `artifacts/triplet/.env.i5-a-kern` · `artifacts/triplet/i5-a-kern.sh` · `artifacts/triplet/i5-a-kern.yaml` | 2-signal | file:output/multi/configs/i5-a-kern.yaml | 이 셀의 서빙 설정 · 러너 · env. 노브별 지위는 1.3, 커널 레버 env 는 02 §2.5. |

**레시피 경고 · 실린 리비전**(기계 관측 — 수신자 빌드가 여기서 갈라질 수 있다)

- `build_recipe` `output/multi/Dockerfile.source-build` 실린 리비전: 리비전 미특정 · 선택 방법 `worktree(측정 이미지 docker history 의 RUN·COPY 와 전수 일치)` · docker history 대조 17/17 · 실린 바이트 `worktree` · 쓰인 바이트임을 관측으로 확인 예
- `build_recipe` `output/multi/requirements.txt` 실린 리비전: 리비전 미특정 · 선택 방법 `worktree(빌드 원장 sha256 일치 — 쓰인 바이트 관측 확인 = 예)` · docker history 대조 없음 · 실린 바이트 `worktree` · 쓰인 바이트임을 관측으로 확인 예
- `compose` `.claude/skills/upstream-version-watch/assets/configs/serve_runner.sh` 실린 리비전: 커밋 `50a5ce486738` · 선택 방법 `worktree(mtime 2026-09-06T21:57:53Z ≤ 측정 2026-10-05T07:28:55Z(certificate.measured_utc) · 바이트 = 측정 전 마지막 커밋 50a5ce486738)` · docker history 대조 없음 · 실린 바이트 `worktree` · 근거 `mtime≤measured` · 쓰인 바이트임을 관측으로 확인 예
- `compose` `output/multi/docker-compose.yaml` 실린 리비전: 커밋 `5b992ad9cc60` · 선택 방법 `worktree(mtime 2026-10-03T20:10:39Z ≤ 측정 2026-10-05T07:28:55Z(certificate.measured_utc) · 바이트 = 측정 전 마지막 커밋 5b992ad9cc60)` · docker history 대조 없음 · 실린 바이트 `worktree` · 근거 `mtime≤measured` · 쓰인 바이트임을 관측으로 확인 예

**싣지 않은 파일**(슬롯별 사유 — 빌드 때 skip 된 패치는 아래 적용 집합 표에 있다)

- `build_patch_post` known-non-patch 1건: `.gitkeep`
- `build_patch_pre` `exl3_port_lib.py` — `nonconforming(<NN>-*.sh 규약 밖 — 싣지 않음)`
- `build_patch_pre` known-non-patch 3건: `.gitkeep` · `PROVENANCE.json` · `__pycache__/`
- `compose` `arm_patch.sh` — `not-armed(output/multi/configs/i5-a-kern_patch.py 없음 · slave CONFIG_FILE='default' 의 _patch.py 없음 — arm_patch.sh 는 no-op)`

**파일별 검증 · 관련성**(`generated-unverified` = 실행 검증되지 않은 생성물 — 파일 첫 줄 경고 · PAYLOAD 파일 기록과 같은 표시 · 관련성 = ① 패치 선언 대상 × 모델 config ② 엔진 로그 발화 서명 — 둘 다 관측일 때만 required/inactive-inferred)

| 슬롯 | 파일 | 검증 | 검증 근거 | 관련성 | 관련성 근거 |
|---|---|---|---|---|---|
| `build_patch_post` | `artifacts/build_patch_post/10-deepgemm.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_i5-a-kern.json#main) · 원장 관측 | `unknown` | ① 선언 ['DeepseekV4ForCausalLM'] ∌ 셀 모델(architectures ['DeepseekV41ForCausalLM'] · model_type deepseek_v41) · 범용 선언 · ② 패치가 심는 logger 발화 서명 없음(발화 미관측) |
| `build_patch_post` | `artifacts/build_patch_post/20-triton-kernels.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_i5-a-kern.json#main) · 원장 관측 | `unknown` | ① 선언 ['DeepseekV4ForCausalLM'] ∌ 셀 모델(architectures ['DeepseekV41ForCausalLM'] · model_type deepseek_v41) · 범용 선언 · ② 패치가 심는 logger 발화 서명 없음(발화 미관측) |
| `build_patch_post` | `artifacts/build_patch_post/30-mxfp4-triton-sm121.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_i5-a-kern.json#main) · 원장 관측 | `unknown` | ① 패치 헤더·본문에 대상 선언(model-trigger 아키텍처 · vllm/models/<model_type>) 없음 · 편집하는 공유(모델 디렉터리 밖) 파일 ['vllm/model_executor/layers/fused_moe/experts/gpt_oss_triton_kernels_moe.py'] |
| `build_patch_post` | `artifacts/build_patch_post/40-humming-nvml-gb10.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_i5-a-kern.json#main) · 원장 관측 | `unknown` | ① 패치 헤더·본문에 대상 선언(model-trigger 아키텍처 · vllm/models/<model_type>) 없음 |
| `build_patch_post` | `artifacts/build_patch_post/70-exllamav3-build.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_i5-a-kern.json#main) · 원장 관측 | `unknown` | ① 패치 헤더·본문에 대상 선언(model-trigger 아키텍처 · vllm/models/<model_type>) 없음 |
| `build_patch_post` | `artifacts/build_patch_post/72-exl3-row-store.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_i5-a-kern.json#main) · 원장 관측 | `unknown` | ① 패치 헤더·본문에 대상 선언(model-trigger 아키텍처 · vllm/models/<model_type>) 없음 |
| `build_patch_post` | `artifacts/build_patch_post/73-vllm-exl3-build.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_i5-a-kern.json#main) · 원장 관측 | `unknown` | ① 패치 헤더·본문에 대상 선언(model-trigger 아키텍처 · vllm/models/<model_type>) 없음 · 편집하는 공유(모델 디렉터리 밖) 파일 ['vllm/model_executor/model_loader/weight_utils.py'] |
| `build_patch_post` | `artifacts/build_patch_post/74-exl3-moe-prep-fused.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_i5-a-kern.json#main) · 원장 관측 | `unknown` | ① 패치 헤더·본문에 대상 선언(model-trigger 아키텍처 · vllm/models/<model_type>) 없음 |
| `build_patch_post` | `artifacts/build_patch_post/79-exl3-port-selfcheck.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_i5-a-kern.json#main) · 원장 관측 | `unknown` | ① 패치 헤더·본문에 대상 선언(model-trigger 아키텍처 · vllm/models/<model_type>) 없음 |
| `build_patch_pre` | `artifacts/build_patch_pre/55-src-deps-authority.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_i5-a-kern.json#main) · 원장 관측 | `unknown` | ① 패치 헤더·본문에 대상 선언(model-trigger 아키텍처 · vllm/models/<model_type>) 없음 |
| `build_patch_pre` | `artifacts/build_patch_pre/80-exl3-quant-method.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_i5-a-kern.json#main) · 원장 관측 | `unknown` | ① 패치 헤더·본문에 대상 선언(model-trigger 아키텍처 · vllm/models/<model_type>) 없음 · 편집하는 공유(모델 디렉터리 밖) 파일 ['vllm/model_executor/layers/quantization/__init__.py'] |
| `build_patch_pre` | `artifacts/build_patch_pre/81-exl3-model-wiring.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_i5-a-kern.json#main) · 원장 관측 | `unknown` | ① 선언 ['models/deepseek_v41'] ∋ 셀 모델(architectures ['DeepseekV41ForCausalLM'] · model_type deepseek_v41) · ② 패치가 심는 logger 발화 서명 없음(발화 미관측) |
| `build_patch_pre` | `artifacts/build_patch_pre/82-exl3-h2d-stage.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_i5-a-kern.json#main) · 원장 관측 | `unknown` | ① 선언 ['models/deepseek_v41'] ∋ 셀 모델(architectures ['DeepseekV41ForCausalLM'] · model_type deepseek_v41) · ② 패치가 심는 logger 발화 서명 없음(발화 미관측) · 편집하는 공유(모델 디렉터리 밖) 파일 ['vllm/model_executor/layers/linear.py', 'vllm/model_executor/layers/vocab_parallel_embedding.py', 'vllm/model_executor/model_loader/weight_utils.py'] |
| `build_patch_pre` | `artifacts/build_patch_pre/83-engram-file-backend.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_i5-a-kern.json#main) · 원장 관측 | `unknown` | ① 선언 ['models/deepseek_v41'] ∋ 셀 모델(architectures ['DeepseekV41ForCausalLM'] · model_type deepseek_v41) · ② 엔진 로그 1개에서 서명 무발화(서명 3개 전부 0회) — 두 신호가 한 방향을 가리키지 않는다 |
| `build_patch_pre` | `artifacts/build_patch_pre/84-sm12x-block-shim.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_i5-a-kern.json#main) · 원장 관측 | `unknown` | ① 선언 ['models/deepseek_v41'] ∋ 셀 모델(architectures ['DeepseekV41ForCausalLM'] · model_type deepseek_v41) · ② 패치가 심는 logger 발화 서명 없음(발화 미관측) · 편집하는 공유(모델 디렉터리 밖) 파일 ['vllm/model_executor/layers/sparse_attn_indexer.py', 'vllm/v1/attention/backends/mla/indexer.py', 'vllm/v1/attention/backends/mla/sparse_swa.py'] |
| `build_patch_pre` | `artifacts/build_patch_pre/85-exl3-serial-streams.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_i5-a-kern.json#main) · 원장 관측 | `unknown` | ① 선언 ['models/deepseek_v4', 'models/deepseek_v41'] ∋ 셀 모델(architectures ['DeepseekV41ForCausalLM'] · model_type deepseek_v41) · ② 패치가 심는 logger 발화 서명 없음(발화 미관측) |
| `build_patch_pre` | `artifacts/build_patch_pre/86-exl3-worker-hooks.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_i5-a-kern.json#main) · 원장 관측 | `unknown` | ① 패치 헤더·본문에 대상 선언(model-trigger 아키텍처 · vllm/models/<model_type>) 없음 · 편집하는 공유(모델 디렉터리 밖) 파일 ['vllm/v1/worker/gpu_worker.py'] |
| `build_patch_pre` | `artifacts/build_patch_pre/87-engram-embed-skip.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_i5-a-kern.json#main) · 원장 관측 | `unknown` | ① 선언 ['models/deepseek_v41'] ∋ 셀 모델(architectures ['DeepseekV41ForCausalLM'] · model_type deepseek_v41) · ② 패치가 심는 logger 발화 서명 없음(발화 미관측) · 편집하는 공유(모델 디렉터리 밖) 파일 ['vllm/model_executor/model_loader/ep_weight_filter.py'] |
| `build_patch_pre` | `artifacts/build_patch_pre/88-engram-prestage.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_i5-a-kern.json#main) · 원장 관측 | `unknown` | ① 선언 ['models/deepseek_v41'] ∋ 셀 모델(architectures ['DeepseekV41ForCausalLM'] · model_type deepseek_v41) · ② 패치가 심는 logger 발화 서명 없음(발화 미관측) · 편집하는 공유(모델 디렉터리 밖) 파일 ['vllm/v1/worker/gpu/model_runner.py'] |
| `build_patch_pre` | `artifacts/build_patch_pre/89-kv-null-block-zero.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_i5-a-kern.json#main) · 원장 관측 | `unknown` | ① 패치 헤더·본문에 대상 선언(model-trigger 아키텍처 · vllm/models/<model_type>) 없음 · 편집하는 공유(모델 디렉터리 밖) 파일 ['vllm/v1/worker/gpu/model_runner.py'] |
| `build_patch_pre` | `artifacts/build_patch_pre/90-woa-fp8-einsum.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_i5-a-kern.json#main) · 원장 관측 | `unknown` | ① 선언 ['models/deepseek_v4'] ∌ 셀 모델(architectures ['DeepseekV41ForCausalLM'] · model_type deepseek_v41) · ② 패치가 심는 logger 발화 서명 없음(발화 미관측) · 편집하는 공유(모델 디렉터리 밖) 파일 ['vllm/utils/deep_gemm.py'] |
| `build_patch_pre` | `artifacts/build_patch_pre/91-mxfp8-prefer-b12x.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_i5-a-kern.json#main) · 원장 관측 | `unknown` | ① 패치 헤더·본문에 대상 선언(model-trigger 아키텍처 · vllm/models/<model_type>) 없음 · 편집하는 공유(모델 디렉터리 밖) 파일 ['vllm/model_executor/kernels/linear/mxfp8/flashinfer.py'] |
| `build_patch_pre` | `artifacts/build_patch_pre/92-dense-gemv.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_i5-a-kern.json#main) · 원장 관측 | `unknown` | ① 선언 ['models/deepseek_v4'] ∌ 셀 모델(architectures ['DeepseekV41ForCausalLM'] · model_type deepseek_v41) · ② 패치가 심는 logger 발화 서명 없음(발화 미관측) · 편집하는 공유(모델 디렉터리 밖) 파일 ['vllm/model_executor/kernels/linear/mxfp8/flashinfer.py'] |
| `build_recipe` | `artifacts/build_recipe/Dockerfile.source-build` | `verified` · cell-run | 쓰인 레시피(build-ledger.dockerfile) — 개정 선택 slots.build_recipe.evidence.selected_revisions | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `build_recipe` | `artifacts/build_recipe/requirements.txt` | `verified` · cell-run | 쓰인 레시피(build-ledger.dockerfile) — 개정 선택 slots.build_recipe.evidence.selected_revisions | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/.env.cluster.template` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/.env.interconnect.template` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/.env.template` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/docker-compose.yaml` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/serve_runner.sh` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/sub_recipe.json` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/.env.i5-a-kern` | `verified` · cell-run | 셀 트리플렛 — 측정 대조 없음 | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/i5-a-kern.sh` | `verified` · cell-run | 셀 트리플렛 — 측정 대조 없음 | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/i5-a-kern.yaml` | `verified` · cell-run | 셀 트리플렛 — 측정 대조 없음 | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
<!-- /FACT:slots -->

<!-- FACT:applied_set -->
**적용 집합** — 상태 `observed` · 출처 `attestation:output/multi/benchlog/attestation_i5-a-kern.json#main`

| 단계 | 파일 | 결과 | 결과 출처 | 사유 | 실림 | model-trigger |
|---|---|---|---|---|---|---|
| pre | `55-src-deps-authority.sh` | applied | build-ledger | — | 실림 | — |
| pre | `80-exl3-quant-method.sh` | applied | build-ledger | lazy registration hook + chat template installed | 실림 | sfxnz 하이브리드 EXL3 팩(및 EXL3 라우티드 전문가 체크포인트 일반). SM12X_PORT=1 변종 이미지에서만 적용. |
| pre | `81-exl3-model-wiring.sh` | applied | build-ledger | 3 edits (3 newly applied) | 실림 | DeepseekV41 + MXFP8 lm_head 체크포인트 + --quantization exl3. **이 스크립트는 SM12X_PORT=1 변종에서만 적용**되므로 stock 이미지는 영향 없음. |
| pre | `82-exl3-h2d-stage.sh` | applied | build-ledger | 14 new edits/footers | 실림 | 모든 모델 로드(이식 변종 이미지 한정). 호스트 안전 방어(policy:HOST_SAFETY_LAYERED_DEFENSE)와 같은 방향의 방어. |
| pre | `83-engram-file-backend.sh` | applied | build-ledger | engram backend retargeted to nvidia/engram.py | 실림 | DeepseekV41 + EXL3 팩 + Engram 원본 샤드 디렉터리. 비-EXL3 모델/경로 미설정은 stock 경로(디스패처). |
| pre | `84-sm12x-block-shim.sh` | applied | build-ledger | 10 new edits/footers | 실림 | DeepseekV41 on SM12x(GB10). 비-SM12x 에서는 헬퍼가 stock 값을 돌려준다(런타임 분기). |
| pre | `85-exl3-serial-streams.sh` | applied | build-ledger | 2 new edits | 실림 | DeepseekV41 + --quantization exl3. 조건부(quant_config.get_name()=="exl3") - 같은 이미지의 비-EXL3 모델은 stock 병렬 경로 유지. |
| pre | `86-exl3-worker-hooks.sh` | applied | build-ledger | worker hooks footer installed | 실림 | quant_config.get_name()=="exl3" 인 워커만. 그 외 모델은 원래 메서드를 그대로 호출. |
| pre | `87-engram-embed-skip.sh` | applied | build-ledger | engram embed skip installed | 실림 | DeepseekV41 + 파일 기반 Engram(83). 다른 모델은 `.engram.embed.` 이름이 없어 불변. |
| pre | `88-engram-prestage.sh` | applied | build-ledger | 2 new edits (prestage v2: census+early) | 실림 | DeepseekV41 + 파일 기반 Engram(83 활성 = engram_table_dir 설정) + V2 Model Runner(model_state 경로). 그 외 모델은 footer 가 아무 층도 못 찾아 no-op. |
| pre | `89-kv-null-block-zero.sh` | applied | build-ledger | 2 new edits | 실림 | 캡처를 하는 모든 모델(그래프 모드). eager(--enforce-eager)는 캡처가 없으므로 no-op. block 0 은 정의상 쓰이지 않는 자리라 |
| pre | `90-woa-fp8-einsum.sh` | applied | build-ledger | sfxnz woa probe+requant+prepack @ 3de914682d08 + exl3-port gate | 실림 | deepseek_v4 o_proj 공용 경로(V4·V4.1 · flashmla/flashinfer_sparse). wo_a 가 이미 e4m3 면 stage 1 은 no-op. |
| pre | `91-mxfp8-prefer-b12x.sh` | applied | build-ledger | sfxnz prefer_b12x_mxfp8 @ 3de914682d08 + exl3-port backend gate | 실림 | — |
| pre | `92-dense-gemv.sh` | applied | build-ledger | sfxnz dense_gemv @ 3de914682d08 (runtime /opt/exl3-port/dsv41-patch) + exl3-port path/import-report shim | 실림 | — |
| post | `10-deepgemm.sh` | applied | build-ledger | — | 실림 | DeepseekV4ForCausalLM FP8 block-scale dense/MoE (SM12x). 범용 arch-enablement(모델-키잉 ✗ — 이미지 네이밍 불변식). |
| post | `20-triton-kernels.sh` | applied | build-ledger | — | 실림 | DeepseekV4ForCausalLM (MXFP4 MoE 74GiB/node — MARLIN repack 미적합). 범용(MXFP4 MoE 의 TRITON 경로 필요 모델 공통). |
| post | `30-mxfp4-triton-sm121.sh` | applied | build-ledger | — | 실림 | — |
| post | `40-humming-nvml-gb10.sh` | applied | build-ledger | — | 실림 | — |
| post | `70-exllamav3-build.sh` | applied | build-ledger | exllamav3 1.4.5 @ e648f1a13136 built for 12.1a | 실림 | EXL3 체크포인트. SM12X_PORT=1 변종에서만. |
| post | `72-exl3-row-store.sh` | applied | build-ledger | librow_store.so built (/opt/exl3-port/librow_store.so) | 실림 | EXL3 팩 + 원본 Engram 샤드 디렉터리 서빙. SM12X_PORT=1 변종에서만. |
| post | `73-vllm-exl3-build.sh` | applied | build-ledger | vllm-exl3 0.4.2 @ d3cfd3949203 + sfxnz p2b shapes/mrow/cfg1/codebook/fshift/srcsort/coop/dataflow @ 3de914682d08 built for 12.1a | 실림 | sfxnz EXL3 팩(quant_method=exl3 + non_routed_quantization). SM12X_PORT=1 변종에서만. |
| post | `74-exl3-moe-prep-fused.sh` | applied | build-ledger | sfxnz moe_prep_fused @ 3de914682d08 (runtime /opt/exl3-port/dsv41-patch) + exl3.py arming footer (DSV41_MOE_PREP_FUSED gate) | 실림 | — |
| post | `79-exl3-port-selfcheck.sh` | applied | build-ledger | self-check PASS (registry, classes, plugin edits, natives, engram hook + embed skip) | 실림 | SM12X_PORT=1 변종 이미지 한정. |
| inline | `strip-hoist` | skipped | build-ledger | register_opaque_type accepts hoist / opaque_object 부재 | 안 실림 | — |

**결과 출처 범례**

- `build-ledger` — 이미지 안 빌드 원장이 적용 결과를 기록했다(관측)

**판정 근거**(패치별 — artifacts 가 판정에 쓴 관측 · 스크립트 바이트 정체 = 실린 바이트가 측정 이미지에 구워진 바이트인가)

| 파일 | 스크립트 바이트 정체 | 이미지 사본 sha256 | 적용 표지(상태 찾음/선언) | 정적 규칙 | 원장 분류 |
|---|---|---|---|---|---|
| `55-src-deps-authority.sh` | build-ledger(원장 script_sha256 = 작업트리 바이트 — 실린 바이트 = 빌드가 실행한 바이트) | `7a1904c8ee2f96e0d73203c954faaa545be292feaf61aadccf36f5a49d0687db`(빌드 원장 script_sha256 — 이미지 사본 아님) | — | — | exit-code |
| `80-exl3-quant-method.sh` | build-ledger(원장 script_sha256 = 작업트리 바이트 — 실린 바이트 = 빌드가 실행한 바이트) | `ebc3caa7f2cdc072586693d943377ac24586117ad519543484057c21ef62aa29`(빌드 원장 script_sha256 — 이미지 사본 아님) | — | — | status-file |
| `81-exl3-model-wiring.sh` | build-ledger(원장 script_sha256 = 작업트리 바이트 — 실린 바이트 = 빌드가 실행한 바이트) | `5078f31d0538aee23f06796cd93ca4f2935d07564450d7e1f657f7e9bba30f76`(빌드 원장 script_sha256 — 이미지 사본 아님) | — | — | status-file |
| `82-exl3-h2d-stage.sh` | build-ledger(원장 script_sha256 = 작업트리 바이트 — 실린 바이트 = 빌드가 실행한 바이트) | `db2447425328311be0502653f13ad90bf48b3f27ceb308cc5a64b7799b0e9bc2`(빌드 원장 script_sha256 — 이미지 사본 아님) | — | — | status-file |
| `83-engram-file-backend.sh` | build-ledger(원장 script_sha256 = 작업트리 바이트 — 실린 바이트 = 빌드가 실행한 바이트) | `23104df4a0915a149f04cc280c8ebe175cbf090be4df4273b0546f77b53b0d31`(빌드 원장 script_sha256 — 이미지 사본 아님) | — | — | status-file |
| `84-sm12x-block-shim.sh` | build-ledger(원장 script_sha256 = 작업트리 바이트 — 실린 바이트 = 빌드가 실행한 바이트) | `0bac1dec1989b234286161e006e6b76a22464423da56e294cc78bc0c91c788c6`(빌드 원장 script_sha256 — 이미지 사본 아님) | — | — | status-file |
| `85-exl3-serial-streams.sh` | build-ledger(원장 script_sha256 = 작업트리 바이트 — 실린 바이트 = 빌드가 실행한 바이트) | `4ce7f6d5df26fad069fff8cc69ca916ce9745d83842eb359ec9b59c9df1c477a`(빌드 원장 script_sha256 — 이미지 사본 아님) | — | — | status-file |
| `86-exl3-worker-hooks.sh` | build-ledger(원장 script_sha256 = 작업트리 바이트 — 실린 바이트 = 빌드가 실행한 바이트) | `2aa1add673b97729f743169a1461c62a0f0cc2ac096fad447943a74d0a9ded5c`(빌드 원장 script_sha256 — 이미지 사본 아님) | — | — | status-file |
| `87-engram-embed-skip.sh` | build-ledger(원장 script_sha256 = 작업트리 바이트 — 실린 바이트 = 빌드가 실행한 바이트) | `4a5b08d50136957e84b9f3e8591ee43624613230681c0d5a1629fb9b7b382359`(빌드 원장 script_sha256 — 이미지 사본 아님) | — | — | status-file |
| `88-engram-prestage.sh` | build-ledger(원장 script_sha256 = 작업트리 바이트 — 실린 바이트 = 빌드가 실행한 바이트) | `c777073cacb84c5143d3fa52b10da009b3332125e9bea807b4e3741afb497f8a`(빌드 원장 script_sha256 — 이미지 사본 아님) | — | — | status-file |
| `89-kv-null-block-zero.sh` | build-ledger(원장 script_sha256 = 작업트리 바이트 — 실린 바이트 = 빌드가 실행한 바이트) | `64faab36dfc84835927032bba714458035397b300cd1f2a67301ac2f8e00754b`(빌드 원장 script_sha256 — 이미지 사본 아님) | — | — | status-file |
| `90-woa-fp8-einsum.sh` | build-ledger(원장 script_sha256 = 작업트리 바이트 — 실린 바이트 = 빌드가 실행한 바이트) | `bb648abf8f0cae4f164180f6bba92a3c517c53f984655360aeb8b2f93e85c34c`(빌드 원장 script_sha256 — 이미지 사본 아님) | — | — | status-file |
| `91-mxfp8-prefer-b12x.sh` | build-ledger(원장 script_sha256 = 작업트리 바이트 — 실린 바이트 = 빌드가 실행한 바이트) | `4c26a0a8dfbed1a449c696c1175d916255e6ab91618eb537f59cc7cfba2fa4ba`(빌드 원장 script_sha256 — 이미지 사본 아님) | — | — | status-file |
| `92-dense-gemv.sh` | build-ledger(원장 script_sha256 = 작업트리 바이트 — 실린 바이트 = 빌드가 실행한 바이트) | `8b84c3dab7d95d2cf68f4a9590f4a901d6a53ef5be8d89bd2afb67217c86f842`(빌드 원장 script_sha256 — 이미지 사본 아님) | — | — | status-file |
| `10-deepgemm.sh` | build-ledger(원장 script_sha256 = 작업트리 바이트 — 실린 바이트 = 빌드가 실행한 바이트) | `bdfd5412c3b7a58133006436bc2c9960a2433a67d92277a2b12b63d1364c7589`(빌드 원장 script_sha256 — 이미지 사본 아님) | — | — | exit-code |
| `20-triton-kernels.sh` | build-ledger(원장 script_sha256 = 작업트리 바이트 — 실린 바이트 = 빌드가 실행한 바이트) | `e263f424720d666e7915aa2d07d0c4878257ad04b45ade39fabdf67aa3ff2824`(빌드 원장 script_sha256 — 이미지 사본 아님) | — | — | exit-code |
| `30-mxfp4-triton-sm121.sh` | build-ledger(원장 script_sha256 = 작업트리 바이트 — 실린 바이트 = 빌드가 실행한 바이트) | `161dce749a830f55c6c1b43a643929794006b8e5bfa3d55dd6e99654e5958a39`(빌드 원장 script_sha256 — 이미지 사본 아님) | — | — | exit-code |
| `40-humming-nvml-gb10.sh` | build-ledger(원장 script_sha256 = 작업트리 바이트 — 실린 바이트 = 빌드가 실행한 바이트) | `a9f1a94390997c669c3e469debdaa28f0e581910f1f0ed3432dfd8798c4f5c2e`(빌드 원장 script_sha256 — 이미지 사본 아님) | — | — | exit-code |
| `70-exllamav3-build.sh` | build-ledger(원장 script_sha256 = 작업트리 바이트 — 실린 바이트 = 빌드가 실행한 바이트) | `a9a9804a4d9b3737f6caaacd127137eb5bdafe12f523c6bddd9b8bae8cff35a1`(빌드 원장 script_sha256 — 이미지 사본 아님) | — | — | status-file |
| `72-exl3-row-store.sh` | build-ledger(원장 script_sha256 = 작업트리 바이트 — 실린 바이트 = 빌드가 실행한 바이트) | `7c2d75c409e5f7ef86e21d490a03200b7ca75faf8ec06abb426e86310bc20b71`(빌드 원장 script_sha256 — 이미지 사본 아님) | — | — | status-file |
| `73-vllm-exl3-build.sh` | build-ledger(원장 script_sha256 = 작업트리 바이트 — 실린 바이트 = 빌드가 실행한 바이트) | `beb64e00fbfd43cc094eec84de0671b20b9969cf4601471418494f188d6a052f`(빌드 원장 script_sha256 — 이미지 사본 아님) | — | — | status-file |
| `74-exl3-moe-prep-fused.sh` | build-ledger(원장 script_sha256 = 작업트리 바이트 — 실린 바이트 = 빌드가 실행한 바이트) | `371155ac41ba966e7a700c146003a51a83e3a58868ab629f45067ef14ee8630f`(빌드 원장 script_sha256 — 이미지 사본 아님) | — | — | status-file |
| `79-exl3-port-selfcheck.sh` | build-ledger(원장 script_sha256 = 작업트리 바이트 — 실린 바이트 = 빌드가 실행한 바이트) | `6b3d0cb89eae415afa2dadd27235e50ba36442d5955a5940e45787ebc5b1c73d`(빌드 원장 script_sha256 — 이미지 사본 아님) | — | — | status-file |
| `strip-hoist` | — | — | — | — | status-file |

**원장 탐침**(순위: attestation → 메인 로컬 이미지 → 재구성)

| 순위 | 결과 | 대상 |
|---|---|---|
| attestation | observed | output/multi/benchlog/attestation_i5-a-kern.json |

> **attestation 범위** — config `i5-a-kern` · phase serve · 작성 2026-10-06T02:15:44Z(output/multi/benchlog/attestation_i5-a-kern.json 파일 mtime(파일시스템 관측 — 문서에 시각 칸이 없다 · 기록 필드 아님)) · 묶은 근거 cell-name · 파일 `output/multi/benchlog/attestation_i5-a-kern.json` · 측정 창 대비 작성 2026-10-06T02:15:44Z > 측정 끝 2026-10-05T07:29:30Z — 측정 뒤 재기동의 관측: **same config(i5-a-kern) · serve — 측정 뒤 재기동의 관측이다(이 측정 실행의 관측이 아니다 · 파일은 다음 실행이 덮는다)**
<!-- /FACT:applied_set -->

## 1.2 슬롯별 적용 사유
> 이 절이 답하는 질문: 실린 산출물 하나하나는 무엇을 고치며, 왜 이 셀에 필요했고, 이 셀에서 실제로 발화했으며, 언제 불필요해지는가?

공통 사항 먼저. 1.1 의 적용 판정은 24 건 모두 이미지 안 빌드 원장(attestation main)이 기록한 `applied` 이고(inline `strip-hoist` 1건만 `skipped` · 실리지 않음), 관련성은 전부 `unknown` 이다 — 기계는 바인딩 측정의 엔진 로그 보존본 1개(`lite_publish/…_072855Z/lite_engine_i5-a-kern.log` · 꼬리 800줄)를 대조했는데 패치 발화 서명은 대부분 기동 초반에 찍혀 그 창 밖이다. 아래 '발화' 는 같은 기동(시도 3 · EngineCore pid 1162)의 전체 사본(`docs/simlog/26100420_V4_1_Flash_정식화_F2_K6/i5_master.log`)의 줄로 적는다. 이식 패치(PRE 80~92 · POST 70~79)는 모두 같은 게이트를 갖는다 — `SM12X_PORT != 1` 이면 **skip(exit 0 · 원장에 skipped)**, 소스트리 · 앵커가 기대와 다르면 **FAIL(exit 1 · 빌드 중단)** 이다. 대표 원문:

> [원문] 80-exl3-quant-method.sh §L26-35
> if [ "${SM12X_PORT}" != "1" ]; then
>     echo "$TAG skip - SM12X_PORT=${SM12X_PORT} (stock 빌드 경로 불변)"
>     if [ -n "${EASY_VLLM_PATCH_STATUS:-}" ]; then printf 'skipped\tSM12X_PORT!=1\n' > "$EASY_VLLM_PATCH_STATUS"; fi
>     exit 0
> fi
>
> HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
> export PYTHONPATH="$HERE${PYTHONPATH:+:$PYTHONPATH}"
> DST="${EXL3_PORT_VLLM_SRC:-/workspace/vllm-src}"
> [ -d "$DST/vllm/model_executor/layers/quantization" ] || { echo "$TAG FAIL: vLLM 소스트리 부재: $DST" >&2; exit 1; }

같은 skip 블록이 81 §L26-29 · 82 §L22-25 · 83 §L35-38 · 84 §L31-34 · 85 §L19-22 · 86 §L25-28 · 87 §L21-24 · 88 §L53-56 · 89 §L20-23 · 90 §L41-44 · 91 §L24-27 · 92 §L41-44 · 70 §L22-25 · 72 §L17-20 · 73 §L36-39 · 74 §L44-47 · 79 §L19-22 에 있다(스크립트 원문 줄 번호 — grep 관측). 이 셀은 `SM12X_PORT=1`(셀 env)이라 전부 적용됐다. 외부 코드는 대부분 **핀 커밋에서 빌드 때 받아 sha256 검증 후 실행**하고(PROVENANCE) zip 에는 받는 스크립트만 실렸다 — AGPL 코드 동봉 금지 결정(plan_26100208 §8 D6)을 따른 것이다.

### build_patch_pre/55-src-deps-authority.sh
(a) 소스트리가 자기 의존 핀의 권위가 되게 constraint 를 양보시키고(천장 핀은 보존 · 침범 = FAIL), flashinfer-cubin 을 업스트림 인덱스에서 직접 설치한다 — 그 1.5 GB 휠을 청크별 리다이렉트 재해석 + Range 이어받기 + sha256 검증으로 받는 경로가 이 계보에서 추가됐다. (b) W5(서명 URL 만료로 빌드 사망). (c) 빌드 시점 패치라 서빙 로그 서명은 없다 — 원장 `applied`(exit-code 분류). (d) `SRC_DEPS_AUTHORITY != 1` 이고 `SM12X_PORT != 1` 이면 skip(§L44-58 — `SM12X_PORT=1` 이면 경고와 함께 켜진 것으로 간주한다). 업스트림이 cubin 을 PyPI 에 올리고 서명 URL 문제가 사라지면 이어받기 경로는 불필요해진다.

### build_patch_pre/80-exl3-quant-method.sh
(a) 0.30.0 `quantization/__init__.py` 에 지연 등록 2곳(`QUANTIZATION_METHODS.append("exl3")` · `get_quantization_config()` 안의 `vllm_exl3.register()` + 등록 사후 단언)과 chat template 설치. 본체 메서드는 POST 73 의 플러그인이다. 자체 설계(round 1 즉시 import 훅은 순환 ImportError 로 조용히 미등록이 돼 버렸다 — R8). (b) W1. (c) 직접 서명은 없지만 `--quantization exl3` 서빙이 섰고 `EXL3 MCG trellis engaged` 가 찍혔다(i5_master.log L726 · L800). (d) skip/FAIL 은 위 원문. 업스트림이 exl3 를 정식 등록하면 불필요하다.

### build_patch_pre/81-exl3-model-wiring.sh
(a) DeepSeek-V4.1 모델 코드 3곳 — lm_head ctor 에 quant_config 전달 · WeightsMapper regex 2개 · VL 매퍼 prefix — 로 MXFP8 lm_head 를 싣는다. 자체 편집(sfxnz lmhead_mxfp8.py 의 매퍼 규칙 분석에 근거 · 파일 비복사). (b) W1 의 일부 — 팩 A 의 MXFP8 lm_head. 헤더가 적은 stock 실패("no module or parameter named 'lm_head'")는 선례 분석이고 이 계보의 기동 관측 기록은 아니다. (c) `exl3-port: lm_head MXFP8 via ModelOptLinearMethod` 양 랭크(i5_master.log L638 · L711). (d) skip(§L26-29) · 앵커 count != 1 이면 빌드 실패(헤더 idempotent 줄). 팩 A 외 체크포인트에는 불활성.

### build_patch_pre/82-exl3-h2d-stage.sh
(a) stock 로더의 `X.copy_(Y)` 7개 지점을 pinned 스테이징 경유 복사로(Mia `patch_h2d_stage.py` 의 0.30.0 재앵커 · 편집문 재작성). (b) GB10 UMA + safetensors 0.8 에서 mmap 직통 복사가 copy-on-write 로 익명 호스트 메모리를 쌓아 두 노드가 wedge 됐다는 **Mia 의 기록**이 근거다 — 이 계보에서 이 패치를 빼고 기동한 관측은 없다(Q6). (c) 이 패치 고유의 발화 로그는 없다 — 미관측. (d) skip(§L22-25) · 발생 횟수 불일치 = 빌드 실패.

### build_patch_pre/83-engram-file-backend.sh
(a) Mia file-backed Engram(`engram_file_backend.py` · `engram_layout.py` — 빌드 때 받아 sha256)을 0.30.0 CUDA 서브클래스(`nvidia/engram.py`)에 재타깃한 옵트인 디스패처. Engram 테이블 경로가 설정될 때만(`hf-overrides` engram_table_dir) 쓰인다. (b) W2. (c) `File-backed Engram layer=1 rank=0/2 rows=[0,192001740) cache=0.0GiB packed=False`(i5_master.log L529) — 설정된 경로가 실제로 열렸다. (d) skip(§L35-38). 헤더는 prefetch 스트림 · CUDA graph 와의 맞물림을 UNTESTED 로 적었는데, 이 셀의 graph 서빙은 88 prestage 가 조회를 캡처 밖으로 옮긴 상태에서 성립했다.

### build_patch_pre/84-sm12x-block-shim.sh
(a) SM12x 에서 DeepSeek-V4.1 sparse-MLA 가 64-상태 페이지로 돌게 하는 shim(압축 KV · 인덱서 캐시 block_size · SWA 페이지 32→64 · 커널 블록 [64,128]) — Mia `patch_sm120_block64.py` 의 선별 재앵커. (b) 근거는 DeepGEMM `_C.so` 의 `block_kv == 32 or block_kv == 64` assert(문자열 실측)와 선례(Mia · sfxnz)이며 헤더 스스로 0.30.0 에서 필요한지는 코드 독해로 판정할 수 없다고 적는다(UNTESTED) — 이 계보에서 빼고 기동한 기록이 없다(Q6). 서빙 yaml `block-size 64` 와 짝이다. (c) 고유 로그 없음 — 미관측. (d) skip(§L31-34) · 앵커 정확히 1회 아니면 실패. 0.30.0 의 stock SM120 경로로 부팅이 되면 걷어낼 후보다.

### build_patch_pre/85-exl3-serial-streams.sh
(a) EXL3 양자화일 때 보조 CUDA 스트림 3개를 만들지 않는다(직렬 경로 · `DSV41_EXL3_SERIAL_STREAMS=0` 이면 stock). (b) exllamav3 trellis 커널이 디바이스당 락 버퍼 1개를 공유해 서로 다른 스트림의 동시 실행이 영원히 spin 한다는 Mia 의 cuda-gdb 기록 — 이 계보의 관측 아님(Q6). (c) 고유 로그 없음 — 미관측(측정 실행은 데드락 없이 끝났다). (d) skip(§L19-22).

### build_patch_pre/86-exl3-worker-hooks.sh
(a) `Worker.compile_or_warm_up_model` 을 감싸 로드 직후 샤드 page cache 를 `posix_fadvise(DONTNEED)` 로 반환하고, 옵트인(`DSV41_EXL3_PRETUNE=1`)일 때만 exllamav3 GEMM pretune. 이 셀은 PRETUNE 을 켜지 않았고, 플러그인(vllm-exl3 @ d3cfd394)에는 pretune 심볼이 없다(헤더). (b) GB10 에서 드라이버가 page cache 를 회수하지 않는다는 Mia 계보 실측(NVRM NO_MEMORY) — 이 계보에서 빼고 잰 기록은 없다(Q6). (c) 고유 로그 미관측. (d) skip(§L25-28).

### build_patch_pre/87-engram-embed-skip.sh
(a) `should_skip_weight()` 맨 앞에 가드 — 파일 기반 Engram 이 활성이면 `*.engram.embed.{weight,scale}`(레이어당 ~94 GiB)를 가중치 이터레이터에서 건너뛴다. 자체 편집. (b) W2 — 읽는 것 자체가 샤드당 ~94 GiB mmap 읽기 + page cache 팽창이다. (c) `exl3-port: file-backed Engram active (/app/ple_mmap/v41-engram-a) - skipping *.engram.embed.* tensors in the weight iterator` 양 랭크(i5_master.log L708 · L716). (d) skip(§L21-24) · 83 모듈이 없으면 빌드 실패. 헤더 한계: multithread · fastsafetensors · eager/prefetch 로드 전략은 이 필터를 타지 않는다 — 이 팩에서는 기본(lazy) 로드만 쓴다.

### build_patch_pre/88-engram-prestage.sh
(a) Engram 행 수집을 캡처된 forward 밖 `prepare_inputs` 로 옮기는 스테이저(sfx `apply_engram_prestage.py` 의 **설계만** 참조 · 코드 자체 작성 · 수집 수단은 Mia row_store) + v2 footer(census · early — 둘 다 기본 off · 이 셀 off). (b) 처음에는 graph NaN 의 처방으로 만들었으나(M1) NaN 처방은 89 였다 — 지금은 Engram I/O 를 캡처 밖에 두는 정합 경로다. (c) `exl3-port: engram prestage armed (tables=2, max_tokens=1536, heads=[0,12), verify=4, census=0, early=off)` · `engram prestage self-check bit-exact (2 tables)`(i5_master.log L826 · L833 · 캡처 뒤 bit-exact 줄). (d) skip(§L53-56) · v1/v2 혼재 · 꼬리가 아닌 footer = 실패. V2 Model Runner(model_state 경로)가 전제다.

### build_patch_pre/89-kv-null-block-zero.sh
(a) 캡처 루틴 끝에서 KV 텐서마다 block 0(null 블록)을 `zero_()`(`EXL3PORT_KV_NULL_BLOCK_ZERO=1` 기본). 진단 이미지의 실험을 상시 동작으로 정식화한 자체 편집. (b) W4 — 없으면 graph 전부 NaN. (c) `exl3-port: kv null block zeroed after capture (tensors=10)`(i5_master.log L1006). (d) skip(§L20-23). eager 면 캡처가 없어 no-op 이고, block 0 은 정의상 안 쓰이는 자리라 stock 경로에서도 무해하다는 것이 헤더의 판단이다. 업스트림이 캡처 더미 forward 의 null 블록을 정리하면 불필요하다.

### build_patch_pre/90-woa-fp8-einsum.sh
(a) sfxnz 스크립트 2종(`probe_wo_a.py` · `fix_o_proj_woa_fp8.py`)을 핀 커밋 3de91468 에서 받아 그대로 실행 — 로드 때 bf16 로 풀린 wo_a 를 층의 e8m0 스케일로 e4m3 재양자화해 deep_gemm `fp8_einsum` 경로로 · `DSV41_WOA_PREPACK=1` 이면 스케일 prepack. 우리 편집 1개(`_exl3port_woa_requant_gate` — `EXL3PORT_WOA_REQUANT` A/B 손잡이 · recipe 32 아니면 재양자화 안 함). (b) W6 · M3. (c) `[woa-requant] fp8 einsum engaged: (4, 1024, 4096)` · `[woa-prepack] packed ue8m0 scale engaged`(i5_master.log L838-839 · L855-856). (d) skip(§L41-44) · recipe 불일치는 경고 1회 후 bf16 유지. wo_a 가 이미 e4m3 로 로드되면 stage 1 은 no-op 이다.

### build_patch_pre/91-mxfp8-prefer-b12x.sh
(a) sfxnz `prefer_b12x_mxfp8.py` 그대로 실행(`backend="cutlass"` → `"auto"`) + 우리 손잡이 `EXL3PORT_MXFP8_BACKEND`(auto | cutlass | b12x · 랭크당 1회 로그). (b) W7. (c) `exl3-port: mm_mxfp8 backend=auto` 양 랭크(i5_master.log L717 · L720). (d) skip(§L24-27) · 부분 편집이면 die(헤더 idempotent 줄). 업스트림이 SM12x 에서 b12x 를 기본 선택하면 불필요하다.

### build_patch_pre/92-dense-gemv.sh
(a) sfxnz `dense_gemv.py` 를 빌드 때 1회 실행(flashinfer.py · o_proj.py 에 훅) + 런타임 모듈 2종 원문 설치 · 첫 armed 층에서 nvcc JIT(sm_121a). 우리 편집 = sys.path 헤더 + 저자 침묵 폴백을 fail-loud 로그로. (b) W8 · 전제 90 · 91(없으면 die). (c) `exl3-port: dense-gemv hooks loaded (... DSV41_DENSE_GEMV=1)` · 9 모양 `dense GEMV armed ... self-test M=1..8 x 3 dist bitwise, graph replay ok` 양 랭크(i5_master.log L69 · L719-L883 구간). (d) skip(§L41-44) · o_proj 드리프트는 저자가 skip 로그로 넘기지만 우리는 die(§L170 주석). `DSV41_DENSE_GEMV` 미설정 = stock(롤백 손잡이).

### build_patch_post/10-deepgemm.sh · 20-triton-kernels.sh · 30-mxfp4-triton-sm121.sh · 40-humming-nvml-gb10.sh
**이 모델의 요구가 아니라 공유 이미지의 일부다**(이미지 하나가 모든 모델을 서빙 — 헤더 model-trigger 는 DeepseekV4ForCausalLM MXFP4 · humming 경로). 10 = DeepGEMM nv_dev SM120 실행커널(심볼 부재 = FAIL) · 20 = NGC 불완전 standalone triton_kernels 제거(이미 vendored 면 skip) · 30 = OAI-Triton MXFP4 MoE 게이트 상한 (11,0)→(13,0)(이미 완화면 skip · 문자열 미발견 = FAIL — 아래 원문) · 40 = humming NVML 클록 쿼리 가드(이미 가드면 skip · 블록 미발견 = FAIL). 이 셀의 MoE 는 EXL3 native p2b 경로(`native p2b_fused_moe engaged`)라 MXFP4 Triton · humming MoE 를 타지 않는 것으로 보이며(추론 — 이 경로들의 비활성을 로그로 판정한 기록은 없다) 10 의 DeepGEMM 은 90 의 `fp8_einsum` 이 쓰는 deep_gemm 과 같은 패키지일 가능성이 있다(추론). 원장은 4건 모두 `applied`(exit-code).

> [원문] 30-mxfp4-triton-sm121.sh §L26-34
> NEW = "(9, 0) <= (cap.major, cap.minor) < (13, 0)"
> if NEW in s:
>     print("[30-mxfp4-triton-sm121] 이미 완화됨 — skip")
> elif OLD in s:
>     open(f, "w").write(s.replace(OLD, NEW, 1))
>     print("[30-mxfp4-triton-sm121] OK — 게이트 완화:", OLD, "→ < (13, 0)")
> else:
>     sys.stderr.write("[30-mxfp4-triton-sm121] FAIL: 예상 게이트 문자열 미발견 — 상류 변경 의심(false-determinism 차단)\n")
>     sys.exit(1)

### build_patch_post/70-exllamav3-build.sh
(a) exllamav3 1.4.5(커밋 e648f1a1 · 트리 digest 검증)를 Mia aarch64 · fat-kernel 패치와 함께 `TORCH_CUDA_ARCH_LIST=12.1a` 로 설치. (b) W1 — EXL3 네이티브 백엔드. (c) 원장 `exllamav3 1.4.5 @ e648f1a13136 built for 12.1a` · 서빙에서 `EXL3 MCG trellis engaged for routed experts: bits=2 experts_local=384 hidden=5120 intermediate_local=1152 fused_moe=exl3_moe`(i5_master.log L726 — 이 줄은 플러그인이 낸다). (d) skip(§L22-25) · LIBDIR 부재 = FAIL.

### build_patch_post/72-exl3-row-store.sh
(a) Mia `row_store.cpp` 를 `librow_store.so` 로 빌드(sha256 검증 · 5개 심볼 ctypes 로드 확인). (b) W2 — file-backed Engram 의 pread 행 저장소. (c) 83 의 `File-backed Engram layer=...` 줄이 이 라이브러리를 거친 조회다(83 헤더의 경로 설명 — 라이브러리 고유 로그는 없음). (d) skip(§L17-20).

### build_patch_post/73-vllm-exl3-build.sh
(a) vcruz305/vllm-exl3 @ d3cfd394 를 pip 패키지로 설치하며 setup.py 경로 · entry-point 제거 · exl3.py 3편집(weight_block_size · MXFP8 헤드 · 82 스테이징 H2D)을 얹고, sfxnz p2b 체인 8종(shapes → … → dataflow · 핀 3de91468)을 저자 순서대로 그대로 실행한다(widen_p2b_mma 제외 — R7). (b) W1 · W6. (c) `dsv41: p2b coop dataflow kernel engaged (DSV41_P2B_COOP=2)` · `exl3-port: native p2b_fused_moe engaged rows=2 hidden=5120 inter=1152 k=2 mcg=True abi=2 coop=2 src_sort=unset`(i5_master.log). 백엔드 선택은 런타임 env `VLLM_EXL3_MOE_KERNEL=native`(셀 env). (d) skip(§L36-39).

### build_patch_post/74-exl3-moe-prep-fused.sh
(a) sfxnz `moe_prep_fused.py` 원문 설치 + 73 설치본 exl3.py 끝에 무장 footer(`DSV41_MOE_PREP_FUSED=1` 일 때만) · 빌드 시점 앵커 사전판정. (b) W9. (c) `dsv41: moe prep fused: vllm_exl3 _apply_native_fused_moe glue fused, p2b output cast once (verify=16)` · `dsv41: moe prep fused self-check bit-exact (synthetic, m=1..8)`(i5_master.log L72 · L861). (d) skip(§L44-47) · 런타임 예외는 저자와 같은 `FAILED, lever is OFF` 한 줄로 삼켜 stock 유지(§L122 — 이 셀 로그에는 없다).

### build_patch_post/79-exl3-port-selfcheck.sh
(a) 컴파일된 vLLM 위에서만 가능한 import 수준 자체검사(레지스트리 훅 · 메서드 구체성 · 플러그인 편집 · Engram 훅 · 네이티브 심볼 · 라이선스). (b) "만든 것과 도는 것은 다르다" — 첫 부팅 사망 부류를 빌드에서 잡는다. (c) 원장 `self-check PASS (registry, classes, plugin edits, natives, engram hook + embed skip)`(1.1 표). 서빙 로그 서명은 없다(빌드 전용). (d) skip(§L19-22) · 하나라도 실패하면 빌드 실패.

### build_recipe — Dockerfile.source-build · requirements.txt
쓰인 레시피(선택자 `Dockerfile.source-build` · docker history 대조 17/17)와 그것이 COPY 하는 requirements.txt(원장 sha256 일치)다. source-build 트랙인 이유는 00 §0.4(1). `SM12X_PORT=1` build-arg 가 이식 패치 전체의 게이트다. 공유 이미지 레시피이지 모델 전용 파일이 아니다.

### compose — docker-compose.yaml · serve_runner.sh · env 형상 3종 · sub_recipe.json
멀티 서빙 경로(측정 당시 개정). serve_runner.sh 는 `NODE_ROLE` 로 master(Ray head + vllm serve)와 slave(Ray worker)를 가르고, 멀티 필수 노브 `distributed-executor-backend` 가 yaml 에 없으면 fail-loud 로 멈춘다(§L51-63). env 형상은 값이 자리표시로 바뀐 사본이다.

### triplet — i5-a-kern.yaml · i5-a-kern.sh · .env.i5-a-kern
이 셀의 서빙 설정 · 러너 · env. 노브별 지위는 1.3, 커널 레버 env 는 02 §2.5.

### 싣지 않은 슬롯
`fork_pin` — 셀 env 에 VARIANT 줄이 없다 = stock 업스트림 `v0.30.0`(엔진 사다리 R3 미사용). `runtime_patch` — `i5-a-kern_patch.py` 가 없어 arm_patch.sh 는 no-op 이다. 이 셀의 모든 이식은 빌드 슬롯에 있다(런타임 `.pth` 는 serve 워커에 닿지 않는다는 plan_26100208 §4 R2 의 판단). 부재는 미판정이 아니라 '해당 없음' 이지만, 런타임 패치 없이 성립했다는 것은 이 셀 한정이다.

## 1.3 값의 지위표
> 서빙 설정의 노브 **전부**와 기계가 모은 후보 지위다(lockset 출처 · yaml 주석 · 엔진 기본값). 최종 지위는 아래 `kind: value-status` 블록이 정한다 — 표의 노브마다 블록이 정확히 하나 있어야 린터를 통과한다.

<!-- FACT:value_status -->
서빙 노브 20개 — 노브마다 `kind: value-status` 블록이 **정확히 1개** 있어야 한다(노브·값은 이 표의 글자 그대로 · 값 칸의 `\|` 는 `|` 로 적는다).

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
| `gpu-memory-utilization` | 0.80 | inherited | lockset 출처 gmu_source=hand · lockset provenance=hand-authored | — | lockset.gmu_source=hand |
| `max-model-len` | 1048576 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `max-num-seqs` | 2 | inherited | lockset 출처 batch_source=hand-lever · lockset provenance=hand-authored | — | lockset.batch_source=hand-lever |
| `max-num-batched-tokens` | 1536 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `enforce-eager` | false | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `compilation-config` | {"cudagraph_mode":"FULL_AND_PIECEWISE","cudagraph_capture_sizes":[1,3,4,6,8],"custom_ops":["all"]} | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `no-enable-flashinfer-autotune` | true | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `reasoning-parser` | deepseek_v41 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `tool-call-parser` | deepseek_v41 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `enable-auto-tool-choice` | true | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `speculative-config` | {"method":"dspark","num_speculative_tokens":3,"draft_sample_method":"greedy"} | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
<!-- /FACT:value_status -->

> 이 절이 답하는 질문: 각 서빙 노브의 값은 이 셀에서 어떤 지위인가 — 조정됐나, 승계됐나, 음성대조로 일부러 둔 것인가, 엔진 기본값인가, 필요조건인가?

```hint-event
id: V1
kind: value-status
노브: tensor-parallel-size
값: 2
지위: inherited
근거: 캠페인 공통조건(D1 멀티노드 · 노드 2 × GPU 1)에서 파생 · 계보 전 셀이 같은 값 · 다른 값을 시도한 기록 없음
출처: [plan_26100208_V4_1_Flash_멀티노드_1M_캠페인 §3., i5-a-kern.yaml]
```

```hint-event
id: V2
kind: value-status
노브: distributed-executor-backend
값: ray
지위: declared-requirement
근거: 없으면 vLLM 이 multiprocessing 으로 가 World size 가 노드당 GPU(1)를 넘어 죽는다 — 러너 헤더의 실측 기록 · 러너가 이 줄이 없으면 fail-loud 로 멈춘다
출처: [serve_runner.sh]
```

```hint-event
id: V3
kind: value-status
노브: quantization
값: exl3
지위: declared-requirement
근거: W1 — 팩 A 의 quant_method 가 exl3 이고 stock 0.30.0 은 그 경로가 없었다 · 80 의 지연 등록 훅은 이 값으로 get_quantization_config 가 불릴 때 플러그인을 등록한다(85 · 86 의 조건부 경로도 이 이름으로 켜진다)
출처: [80-exl3-quant-method.sh, plan_26100208_V4_1_Flash_멀티노드_1M_캠페인 §4.]
```

```hint-event
id: V4
kind: value-status
노브: tokenizer-mode
값: deepseek_v41
지위: inherited
근거: 0.30.0 이 가진 V4.1 토크나이저 모드 · 저자 레시피 계열 승계 · 이 계보에서 바꿔 잰 기록 없음
출처: [i5-a-kern.yaml, plan_26100208_V4_1_Flash_멀티노드_1M_캠페인 §4.]
```

```hint-event
id: V5
kind: value-status
노브: hf-overrides
값: '{"engram_table_dir":"/app/ple_mmap/v41-engram-a"}'
지위: declared-requirement
근거: W2 — 이 키가 83 디스패처 · 87 embed 건너뛰기를 켠다 · 없으면 stock Engram 이 테이블을 pinned UVA 로 올려 OOM 이라는 것이 83 헤더의 기록(이 계보가 빼고 기동한 관측은 없다 — 헤더 기록 근거) · 경로는 노드 로컬 스테이징 자리라 환경마다 다시 정한다
출처: [83-engram-file-backend.sh, 87-engram-embed-skip.sh]
```

```hint-event
id: V6
kind: value-status
노브: block-size
값: 64
지위: inherited
근거: 84 SM12x 64-토큰 블록 shim 과 짝인 저자/Mia 계열 값 · 근거 assert 는 문자열 실측이지만 이 계보에서 다른 값으로 기동한 관측 없음(84 헤더 UNTESTED)
출처: [84-sm12x-block-shim.sh, i5-a-kern.yaml]
```

```hint-event
id: V7
kind: value-status
노브: kernel-config
값: '{"sparse_indexer_topk_backend":"per_row"}'
지위: inherited
근거: 84 가 버린 persistent_topk 스탠자의 0.30.0 대체 런타임 노브 · 승계 · 다른 값 시도 기록 없음
출처: [84-sm12x-block-shim.sh]
```

```hint-event
id: V8
kind: value-status
노브: kv-cache-dtype
값: fp8
지위: inherited
근거: 계보 전 셀이 KV fp8 8 GiB(testlog 유효맥락) · plan 의 KV 축(fp8 ↔ 모델 기본)은 이 계보에서 시도 기록 없음 · 엔진은 fp8_ds_mla KV 형식을 썼다
출처: [plan_26100208_V4_1_Flash_멀티노드_1M_캠페인 §5., testlog_26100420_V4_1_Flash_정식화_F2_K6_moe_prep_판정 §1.]
```

```hint-event
id: V9
kind: value-status
노브: kv-cache-memory-bytes
값: 8589934592
지위: inherited
근거: 손레버(lockset kv_source=hand) · 계보 내내 8 GiB 고정 · 필요량 산식은 계보에 없고 엔진 보고는 KV 2,571,042 토큰 = 1M 요청 기준 2.45배
출처: [testlog_26100417_V4_1_Flash_커널프로파일_b12x_dense_gemv_판정 §1., docs/simlog/26100420_V4_1_Flash_정식화_F2_K6/i5_master.log]
```

```hint-event
id: V10
kind: value-status
노브: gpu-memory-utilization
값: 0.80
지위: inherited
근거: 손레버(lockset gmu_source=hand) · 바꿔 잰 기록 없음 · KV 절대 클램프가 있어 엔진이 메모리 프로파일링을 건너뛰고 이 값을 존중하지 않는다(엔진 로그)
출처: [docs/simlog/26100420_V4_1_Flash_정식화_F2_K6/i5_master.log, docs/simlog/26100420_V4_1_Flash_정식화_F2_K6/smoke_i5.log]
```

```hint-event
id: V11
kind: value-status
노브: max-model-len
값: 1048576
지위: inherited
근거: 캠페인 공통조건(D1 · 1M) · 셀 성립 기준이 '1M 기동' · 이 셀에서 조정 0 · 1M 프롬프트 처리는 별개(W3 · Q5)
출처: [plan_26100208_V4_1_Flash_멀티노드_1M_캠페인 §3., plan_26100319_V4_1_Flash_2노드_광의탐색_캠페인_재설계 §2.]
```

```hint-event
id: V12
kind: value-status
노브: max-num-seqs
값: 2
지위: inherited
근거: 손레버(lockset batch_source=hand-lever) · plan 의 동시성 축 1~2 중 저자 값 2 · 스윕 기록 없음
출처: [plan_26100208_V4_1_Flash_멀티노드_1M_캠페인 §5.]
```

```hint-event
id: V13
kind: value-status
노브: max-num-batched-tokens
값: 1536
지위: inherited
근거: 승계 · 바꿔 잰 기록 없음 · 엔진이 spec 설정 기준으로 max_num_scheduled_tokens 를 1536 으로 둔다고 경고했다
출처: [i5-a-kern.yaml, docs/simlog/26100420_V4_1_Flash_정식화_F2_K6/i5_master.log]
```

```hint-event
id: V14
kind: value-status
노브: enforce-eager
값: false
지위: tuned
근거: 10-03 셀탐색은 eager(graph NaN · lite 13.89~19.35 t/s) → 89 로 NaN 을 푼 뒤 graph 로 바꿔 계보에서 두 값을 모두 잰 기록이 있다(W4)
출처: [plan_26100319_V4_1_Flash_2노드_광의탐색_캠페인_재설계 §1., 89-kv-null-block-zero.sh]
```

```hint-event
id: V15
kind: value-status
노브: compilation-config
값: '{"cudagraph_mode":"FULL_AND_PIECEWISE","cudagraph_capture_sizes":[1,3,4,6,8],"custom_ops":["all"]}'
지위: inherited
근거: 저자 그래프 설정(R-1 · yaml 머리 주석) 승계 · capture sizes 는 DSpark k=3 정합(I-1) · 다른 모드 시도(PIECEWISE · FULL_DECODE_ONLY)는 NaN 진단이었지 성능 비교가 아니다
출처: [plan_26100319_V4_1_Flash_2노드_광의탐색_캠페인_재설계 §3., 88-engram-prestage.sh]
```

```hint-event
id: V16
kind: value-status
노브: no-enable-flashinfer-autotune
값: true
지위: inherited
근거: 승계 · 이 계보에서 켜고 잰 기록 없음
출처: [i5-a-kern.yaml]
```

```hint-event
id: V17
kind: value-status
노브: reasoning-parser
값: deepseek_v41
지위: inherited
근거: 0.30.0 의 V4.1 파서 · 기능 노브(성능 무관) · 승계
출처: [i5-a-kern.yaml, plan_26100208_V4_1_Flash_멀티노드_1M_캠페인 §4.]
```

```hint-event
id: V18
kind: value-status
노브: tool-call-parser
값: deepseek_v41
지위: inherited
근거: 0.30.0 의 V4.1 파서 · 기능 노브 · 승계 · 도구 호출을 시험한 기록은 계보에 없다
출처: [i5-a-kern.yaml]
```

```hint-event
id: V19
kind: value-status
노브: enable-auto-tool-choice
값: true
지위: inherited
근거: 기능 노브 승계 · 끄고 잰 기록 없음
출처: [i5-a-kern.yaml]
```

```hint-event
id: V20
kind: value-status
노브: speculative-config
값: '{"method":"dspark","num_speculative_tokens":3,"draft_sample_method":"greedy"}'
지위: tuned
근거: DSpark n=0 · 3 · 5 · 10 을 lite 로 잰 기록(eager · 10-03)이 있고 k=3 은 저자 graph 기록과 capture sizes 정합으로 골랐다(R4) · graph 상태 k 재비교는 없다
출처: [plan_26100319_V4_1_Flash_2노드_광의탐색_캠페인_재설계 §3., plan_26100319_V4_1_Flash_2노드_광의탐색_캠페인_재설계 §1.]
```

복사하면 위험한 값: `kv-cache-memory-bytes`(V9)는 손레버이지 필요량 역산이 아니다 — 네 HW 에서는 예산 선언과 엔진의 `GPU KV cache size` 보고로 다시 정한다. 이 값이 있는 한 `gpu-memory-utilization`(V10)은 KV 를 정하지 않는다. `hf-overrides`(V5)의 경로는 이 노드들의 로컬 스테이징 자리이고, 값 자체가 아니라 '디스크 Engram 을 켠다' 는 기능이 필요조건이다. `max-model-len`(V11)은 1M 기동 조건이지 1M 처리 검증이 아니다. `block-size`(V6) · `kernel-config`(V7)는 84 shim 과 묶인 승계값이라 84 를 걷어내면 함께 다시 본다. 커널 레버는 yaml 이 아니라 셀 env(`DSV41_*` · `EXL3PORT_*`)에 있다 — 02 §2.5.

## 1.4 재현 절차
> 셀 실행 기록에서 기계가 파생한 명령 순서와 단계별 소요다. 소요가 `미관측` 인 단계는 시각 기록이 없다는 뜻이다(0 이 아니다). 그 아래는 블랙박스 이벤트 원장에서 이 셀의 행을 기계가 뽑아 기동 시도로 묶은 것이고, 마지막은 측정한 실행의 엔진 로그가 echo 한 env 다(측정 당시 값의 정본).

<!-- FACT:reproduce -->
| 순서 | 단계 | 성공 판정 | 소요 | 관측 구간(UTC) | 소요 출처 | 명령 출처 |
|---|---|---|---|---|---|---|
| 1 | render | 러너 3종·.env·.env.cluster·.env.interconnect 생성(렌더러 fail-loud) | 미관측 | 미관측 | .claude/skills/upstream-version-watch/scripts/multinode_serve_smoke.sh 선행조건 안내 · .claude/skills/upstream-version-watch/scripts/render_dockerfile.py CLI | derived(렌더러 CLI) |
| 2 | build | 빌드 OK · 이미지 실재 | 43분 47초 | 2026-10-04T12:12:35Z → 2026-10-04T12:56:22Z (exact) | campaigns/camp-26100320-v41flash-2node-broad/phases/<선언 노드 1>/build.status.json(같은 셀 · 가장 이른 시작 ~ 가장 늦은 끝) | reconstructed(build-ledger build_args + compose build) |
| 3 | serve | health 200 + 추론 1회(스모크 판정) | ≤ 6시간 11분 50초(상한) | 2026-10-05T01:17:05Z → 2026-10-05T07:28:55Z (upper) | docs/logs/main/events/2026-10.jsonl budget_declare(label smoke-i5-a-kern) → 첫 측정 시각(output/multi/benchlog/lite_publish/i5-a-kern_20261005T072855Z/lite_raw_i5-a-kern.json measured_utc(= bench_report 머리 생성일 · lite_bench.sh 가 cold 레그 직전(부하 직전)에 찍는 측정 **시작** 시각)) | derived(스모크 사용법) |
| 4 | bench | 리포트 발행(+PASS 면 인증서) | 0분 35초 | 2026-10-05T07:28:55Z → 2026-10-05T07:29:30Z (exact) | 시작 = output/multi/benchlog/lite_publish/i5-a-kern_20261005T072855Z/lite_raw_i5-a-kern.json measured_utc(= bench_report 머리 생성일 · lite_bench.sh 가 cold 레그 직전(부하 직전)에 찍는 측정 **시작** 시각) → 끝 = output/multi/benchlog/lite_publish/i5-a-kern_20261005T072855Z/lite_warm_i5-a-kern.json date(마지막 레그 결과 저장 · 컨테이너 시계 = UTC 가정) | reconstructed(bench json) |

**빌드 층 시각**(관측 · 소요가 아니라 층 창 — 캐시 재사용 층은 이전 빌드의 시각을 가진다)

- `build` 이미지 층 CreatedAt(docker history): 2026-10-01T07:38:01Z → 2026-10-04T12:52:29Z · 층 30개 · 출처 `docker history --no-trunc --format '{{json .}}' easy-vllm:0.30.0-cu133-aarch64-source-sm12x` — 우리 Dockerfile 층의 CreatedAt 범위 — 캐시 재사용 층은 이전 빌드 시각을 가진다(소요 아님)

**단계별 명령**(기계 파생 — `명령 출처` 가 `derived(…)` 면 사용법에서 파생 · `reconstructed(…)` 면 괄호 안 관측(docker history build-arg · compose build · bench JSON 필드)에서 재구성한 명령이다. 어느 쪽도 실행 원문(로그에 남은 줄)은 아니다)

**1. render**

```sh
python3 .claude/skills/upstream-version-watch/scripts/render_dockerfile.py --materialize-configs --topology multi
python3 .claude/skills/upstream-version-watch/scripts/render_dockerfile.py --materialize-env --topology multi --manifest output/multi/manifest.yaml
python3 .claude/skills/upstream-version-watch/scripts/render_dockerfile.py --cluster-envfile --topology multi --manifest output/multi/manifest.yaml -o output/multi/envs/.env.cluster
python3 .claude/skills/upstream-version-watch/scripts/render_dockerfile.py --nccl-envfile --manifest output/multi/manifest.yaml -o output/multi/envs/.env.interconnect
```

**2. build**

```sh
# 실린 artifacts/build_recipe/Dockerfile.source-build(이미지를 지은 개정: worktree)를 output/multi/Dockerfile.source-build 자리에 두고 컨텍스트에서 짓는다.
# 양 노드에서 각각 짓는다(이미지 save/load 전송 ✗ — 노드마다 자기 이미지 · 요건은 동일 digest 가 아니라 동일 ABI).
docker build -f output/multi/Dockerfile.source-build \
  --build-arg BUILD_JOBS=16 \
  --build-arg RAY_VERSION=2.48.0 \
  --build-arg SM12X_PORT=1 \
  --build-arg SRC_DEPS_AUTHORITY=0 \
  --build-arg TORCH_CUDA_ARCH=12.1a \
  --build-arg VLLM_PRETEND_VERSION= \
  --build-arg VLLM_REF=v0.30.0 \
  --build-arg VLLM_REPO=https://github.com/vllm-project/vllm.git \
  -t easy-vllm:0.30.0-cu133-aarch64-source-sm12x \
  output/multi
# 동등 경로(스모크 사용법): bash .claude/skills/upstream-version-watch/scripts/multinode_serve_smoke.sh i5-a-kern --build-only
```

**3. serve**

```sh
bash .claude/skills/upstream-version-watch/scripts/multinode_serve_smoke.sh i5-a-kern --keep-up
```

**4. bench**

```sh
# bench JSON 필드로 복원: backend · model_id→--model · tokenizer_id→--tokenizer · num_prompts · max_concurrency · request_rate · burstiness · 요청당 입출력 길이 = total_*_tokens ÷ completed(나누어떨어질 때만 — 평균이다 · 요청별 길이는 JSON 에 없다) · 나누어떨어지면 `--dataset-name random` 으로 추정했다(측정 도구가 그 꼴로 부른다 — 아래 도구 원문).
# JSON 에 없는 인자(--endpoint · --base-url · --ignore-eos · --num-warmups · --temperature · --seed · --random-range-ratio · --trust-remote-code)는 측정 도구가 정했다 — level_* = run_bench.sh · lite_cold/lite_warm = lite_bench.sh(.claude/skills/adversarial-benchmark/scripts/) · 셀 스윕 순서는 sweep_bench.sh.
# 호출(재구성 — lite_raw config_name · topology · backend + 바인딩된 경량 리포트 = --publish-report · 원문 호출 줄 아님): bash .claude/skills/adversarial-benchmark/scripts/lite_bench.sh i5-a-kern --topology multi --backend openai-chat --publish-report
# 측정 창: 시작 2026-10-05T07:28:55Z(output/multi/benchlog/lite_publish/i5-a-kern_20261005T072855Z/lite_raw_i5-a-kern.json measured_utc(= bench_report 머리 생성일 · lite_bench.sh 가 cold 레그 직전(부하 직전)에 찍는 측정 **시작** 시각)) → 끝 2026-10-05T07:29:30Z(output/multi/benchlog/lite_publish/i5-a-kern_20261005T072855Z/lite_warm_i5-a-kern.json date(마지막 레그 결과 저장 · 컨테이너 시계 = UTC 가정))
# output/multi/benchlog/lite_publish/i5-a-kern_20261005T072855Z/lite_cold_i5-a-kern.json (date 20261005-072907) · ⚠ --random-input-len/--random-output-len = total ÷ completed 평균(chat 템플릿 포함 실측 토큰 · 측정 도구 원문 인자 미관측 — lite_bench.sh 원문 인자와 다를 수 있다) · spec_decode_acceptance_length 1.91045(이 레그 JSON)
vllm bench serve --backend openai-chat --model deepseek-v4.1-flash-exl3 --tokenizer /app/quant_models/DeepSeek/DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz --dataset-name random --random-input-len 542 --random-output-len 128 --num-prompts 1 --max-concurrency 1 --request-rate inf
# output/multi/benchlog/lite_publish/i5-a-kern_20261005T072855Z/lite_warm_i5-a-kern.json (date 20261005-072930) · ⚠ --random-input-len/--random-output-len = total ÷ completed 평균(chat 템플릿 포함 실측 토큰 · 측정 도구 원문 인자 미관측 — lite_bench.sh 원문 인자와 다를 수 있다) · spec_decode_acceptance_length 2.05348(이 레그 JSON)
vllm bench serve --backend openai-chat --model deepseek-v4.1-flash-exl3 --tokenizer /app/quant_models/DeepSeek/DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz --dataset-name random --random-input-len 542 --random-output-len 128 --num-prompts 3 --max-concurrency 1 --request-rate inf
```

**슬롯 → 빌드 컨텍스트 매핑**(빌드 컨텍스트 = `output/<토폴로지>/` · 원천 = 실린 Dockerfile 의 COPY · compose 의 env_file · volumes · command — `—` 는 빌드 컨텍스트 입력이 아니다)

| zip 경로 | 빌드 컨텍스트 자리 | 근거 |
|---|---|---|
| `artifacts/triplet/.env.i5-a-kern` | `envs/.env.i5-a-kern` | compose env_file envs/.env.${CONFIG_FILE}(셀 env) |
| `artifacts/triplet/i5-a-kern.sh` | `configs/i5-a-kern.sh` | compose volumes ./configs:/app/configs |
| `artifacts/triplet/i5-a-kern.yaml` | `configs/i5-a-kern.yaml` | compose volumes ./configs:/app/configs |
| `artifacts/build_patch_pre/55-src-deps-authority.sh` | `build_patches_src/55-src-deps-authority.sh` | Dockerfile.source-build COPY build_patches_src/ |
| `artifacts/build_patch_pre/80-exl3-quant-method.sh` | `build_patches_src/80-exl3-quant-method.sh` | Dockerfile.source-build COPY build_patches_src/ |
| `artifacts/build_patch_pre/81-exl3-model-wiring.sh` | `build_patches_src/81-exl3-model-wiring.sh` | Dockerfile.source-build COPY build_patches_src/ |
| `artifacts/build_patch_pre/82-exl3-h2d-stage.sh` | `build_patches_src/82-exl3-h2d-stage.sh` | Dockerfile.source-build COPY build_patches_src/ |
| `artifacts/build_patch_pre/83-engram-file-backend.sh` | `build_patches_src/83-engram-file-backend.sh` | Dockerfile.source-build COPY build_patches_src/ |
| `artifacts/build_patch_pre/84-sm12x-block-shim.sh` | `build_patches_src/84-sm12x-block-shim.sh` | Dockerfile.source-build COPY build_patches_src/ |
| `artifacts/build_patch_pre/85-exl3-serial-streams.sh` | `build_patches_src/85-exl3-serial-streams.sh` | Dockerfile.source-build COPY build_patches_src/ |
| `artifacts/build_patch_pre/86-exl3-worker-hooks.sh` | `build_patches_src/86-exl3-worker-hooks.sh` | Dockerfile.source-build COPY build_patches_src/ |
| `artifacts/build_patch_pre/87-engram-embed-skip.sh` | `build_patches_src/87-engram-embed-skip.sh` | Dockerfile.source-build COPY build_patches_src/ |
| `artifacts/build_patch_pre/88-engram-prestage.sh` | `build_patches_src/88-engram-prestage.sh` | Dockerfile.source-build COPY build_patches_src/ |
| `artifacts/build_patch_pre/89-kv-null-block-zero.sh` | `build_patches_src/89-kv-null-block-zero.sh` | Dockerfile.source-build COPY build_patches_src/ |
| `artifacts/build_patch_pre/90-woa-fp8-einsum.sh` | `build_patches_src/90-woa-fp8-einsum.sh` | Dockerfile.source-build COPY build_patches_src/ |
| `artifacts/build_patch_pre/91-mxfp8-prefer-b12x.sh` | `build_patches_src/91-mxfp8-prefer-b12x.sh` | Dockerfile.source-build COPY build_patches_src/ |
| `artifacts/build_patch_pre/92-dense-gemv.sh` | `build_patches_src/92-dense-gemv.sh` | Dockerfile.source-build COPY build_patches_src/ |
| `artifacts/build_patch_post/10-deepgemm.sh` | `build_patches/10-deepgemm.sh` | Dockerfile.source-build COPY build_patches/ |
| `artifacts/build_patch_post/20-triton-kernels.sh` | `build_patches/20-triton-kernels.sh` | Dockerfile.source-build COPY build_patches/ |
| `artifacts/build_patch_post/30-mxfp4-triton-sm121.sh` | `build_patches/30-mxfp4-triton-sm121.sh` | Dockerfile.source-build COPY build_patches/ |
| `artifacts/build_patch_post/40-humming-nvml-gb10.sh` | `build_patches/40-humming-nvml-gb10.sh` | Dockerfile.source-build COPY build_patches/ |
| `artifacts/build_patch_post/70-exllamav3-build.sh` | `build_patches/70-exllamav3-build.sh` | Dockerfile.source-build COPY build_patches/ |
| `artifacts/build_patch_post/72-exl3-row-store.sh` | `build_patches/72-exl3-row-store.sh` | Dockerfile.source-build COPY build_patches/ |
| `artifacts/build_patch_post/73-vllm-exl3-build.sh` | `build_patches/73-vllm-exl3-build.sh` | Dockerfile.source-build COPY build_patches/ |
| `artifacts/build_patch_post/74-exl3-moe-prep-fused.sh` | `build_patches/74-exl3-moe-prep-fused.sh` | Dockerfile.source-build COPY build_patches/ |
| `artifacts/build_patch_post/79-exl3-port-selfcheck.sh` | `build_patches/79-exl3-port-selfcheck.sh` | Dockerfile.source-build COPY build_patches/ |
| `artifacts/build_recipe/Dockerfile.source-build` | `Dockerfile.source-build` | compose build.dockerfile ${BUILD_DOCKERFILE} · build.context . |
| `artifacts/build_recipe/requirements.txt` | `requirements.txt` | Dockerfile.source-build COPY requirements.txt |
| `artifacts/compose/.env.cluster.template` | `envs/.env.cluster` | compose env_file envs/.env.cluster |
| `artifacts/compose/.env.interconnect.template` | `envs/.env.interconnect` | compose env_file envs/.env.interconnect |
| `artifacts/compose/.env.template` | `.env` | compose 변수치환 프로젝트 .env(${…} 참조) |
| `artifacts/compose/docker-compose.yaml` | `docker-compose.yaml` | compose 파일(빌드 컨텍스트 루트) |
| `artifacts/compose/serve_runner.sh` | `configs/serve_runner.sh` | compose volumes ./configs:/app/configs |
| `artifacts/compose/sub_recipe.json` | — | 빌드 컨텍스트 입력 아님(Dockerfile COPY · compose 참조 밖) |

**env 형상의 파생 키 · 실효값**(한 manifest 필드 → 여러 키 · 키마다 자기 자리표시 — 값은 필드 값이 아니라 아래 규칙의 결과다)

| 형상 | manifest 필드 | 키 | 자리표시 | 실효값 규칙 | 출처 |
|---|---|---|---|---|---|
| `artifacts/compose/.env.interconnect.template` | `interconnect.nccl_transport` | `NCCL_IB_DISABLE` · `NCCL_NET` | `<derived:interconnect.nccl_transport→NCCL_IB_DISABLE>` · `<derived:interconnect.nccl_transport→NCCL_NET>` | rdma → NCCL_IB_DISABLE=0 · NCCL_NET=IB \| socket → NCCL_IB_DISABLE=1 · NCCL_NET=Socket | render_dockerfile.NCCL_TRANSPORTS(선택지 → 키 값) |
| `artifacts/compose/.env.interconnect.template` | `interconnect.socket_iface` | `GLOO_SOCKET_IFNAME` · `NCCL_SOCKET_IFNAME` · `OMPI_MCA_btl_tcp_if_include` · `TP_SOCKET_IFNAME` · `UCX_NET_DEVICES` | `<derived:interconnect.socket_iface→GLOO_SOCKET_IFNAME>` · `<derived:interconnect.socket_iface→NCCL_SOCKET_IFNAME>` · `<derived:interconnect.socket_iface→OMPI_MCA_btl_tcp_if_include>` · `<derived:interconnect.socket_iface→TP_SOCKET_IFNAME>` · `<derived:interconnect.socket_iface→UCX_NET_DEVICES>` | GLOO_SOCKET_IFNAME={interconnect.socket_iface} · NCCL_SOCKET_IFNAME={interconnect.socket_iface} · OMPI_MCA_btl_tcp_if_include={interconnect.socket_iface} · TP_SOCKET_IFNAME={interconnect.socket_iface} · UCX_NET_DEVICES={interconnect.socket_iface} | render_dockerfile.env_tier 탐침 manifest(탐침 값 → {field} 규칙) |

**기동 성공 판정(관측 · 발행 자격)**: health 200 = true · 추론 1회 = true · 근거 `output/multi/benchlog/serve_proof_i5-a-kern.json` — 시점 범위: 작성 2026-10-06T02:15:45Z(output/multi/benchlog/serve_proof_i5-a-kern.json 파일 mtime(파일시스템 관측 — 문서에 시각 칸이 없다 · 기록 필드 아님)) · 측정 창 대비 작성 2026-10-06T02:15:45Z > 측정 끝 2026-10-05T07:29:30Z — 측정 뒤 재기동의 스모크 산출물 — 측정을 서빙한 기동의 자격 관측이 아니다
<!-- /FACT:reproduce -->

<!-- FACT:event_timeline -->
**블랙박스 이벤트 55행**(원장에서 이 셀 label·config 에 맞는 행만 · 시각순 · 기계 발췌)

> **캠페인 경계** 2026-10-03T10:52:26Z(`camp-26100320-v41flash-2node-broad` · evidence.event_campaign_boundary): campaigns/camp-26100320-v41flash-2node-broad/campaign.yaml declared_utc 2026-10-03T10:52:26Z · 이 발행이 묶인 측정을 서빙한 budget_declare 창 시작 2026-10-05T01:17:05Z → 경계 = 앞선 값 2026-10-03T10:52:26Z(이전 행 = 같은 셀 이름 · 앞선 캠페인)

> **원장 관측 범위**(노드별 첫 행 → 마지막 행 · evidence.event_ledger_spans): main = 2026-07-31T03:27:43Z → 2026-10-06T04:53:37Z(`docs/logs/main/events/2026-07.jsonl` · `docs/logs/main/events/2026-08.jsonl` · `docs/logs/main/events/2026-09.jsonl` · `docs/logs/main/events/2026-10.jsonl` · `docs/logs/main/events/thermal.jsonl` · `docs/logs/main/events/watchdog.jsonl`) · sub = 2026-09-03T09:51:45Z → 2026-09-11T11:10:38Z(`docs/logs/sub/events/2026-09.jsonl`) — ⚠ sub(마지막 행 2026-09-11T11:10:38Z < 측정 끝 2026-10-05T07:29:30Z): 그 뒤 그 노드의 선언 · 사건은 **관측 범위 밖**이다(없음이 아니다 · 원장 미회수)

| # | UTC | 노드 | kind | label | 내용 | 출처 |
|---|---|---|---|---|---|---|
| 1 | 2026-10-05T00:11:56Z | main | `budget_declare` | `smoke-i5-a-kern` | 예산 선언(선언값 — 측정 아님) · floor_mib=31382 · weights_mib=73770 · kv_mib=8192 · overhead_mib=11264 · mem_total_mib=124608 · 기동 창 1/4: budget_renew 1회 · 닫힘 budget_clear 2026-10-05T00:40:43Z(선언 후 28m47s) · measured_utc 2026-10-05T07:28:55Z 는 이 창 밖(다른 기동 · 측정 전) | docs/logs/main/events/2026-10.jsonl:225 |
| 2 | 2026-10-05T00:11:57Z | main | `budget_honored` | `smoke-i5-a-kern` | budget_honored · floor_mib=31382 · arm_ceiling_mib=28310 · remaining_s=10799 · 창 시작 2026-10-05T00:11:57Z 후 0m00s | docs/logs/main/events/watchdog.jsonl:689 |
| 3 | 2026-10-05T00:13:00Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=85014 · rate_mib_s=22910 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → smoke-i5-a-kern 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-05T00:11:57Z 후 1m03s) | docs/logs/main/events/watchdog.jsonl:690 |
| 4 | 2026-10-05T00:13:03Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=48964 · rate_mib_s=21354 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → smoke-i5-a-kern 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-05T00:11:57Z 후 1m06s) | docs/logs/main/events/watchdog.jsonl:691 |
| 5 | 2026-10-05T00:35:51Z | main | `budget_renew` | `smoke-i5-a-kern` | budget_renew · floor_mib=31382 · ttl_s=10800 · remaining_before_s=9365 · 창 시작 2026-10-05T00:11:56Z 후 23m55s | docs/logs/main/events/2026-10.jsonl:226 |
| 6 | 2026-10-05T00:40:43Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-i5-a-kern 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-05T00:11:56Z 후 28m47s) | docs/logs/main/events/2026-10.jsonl:227 |
| 7 | 2026-10-05T00:40:43Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-i5-a-kern 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-05T00:11:57Z 후 28m46s) | docs/logs/main/events/watchdog.jsonl:692 |
| 8 | 2026-10-05T00:41:06Z | main | `budget_declare` | `smoke-i5-a-kern` | 예산 선언(선언값 — 측정 아님) · floor_mib=31382 · weights_mib=73770 · kv_mib=8192 · overhead_mib=11264 · mem_total_mib=124608 · 기동 창 2/4: budget_renew 6회 · 닫힘 budget_clear 2026-10-05T01:16:34Z(선언 후 35m28s) · measured_utc 2026-10-05T07:28:55Z 는 이 창 밖(다른 기동 · 측정 전) | docs/logs/main/events/2026-10.jsonl:229 |
| 9 | 2026-10-05T00:41:06Z | main | `budget_honored` | `smoke-i5-a-kern` | budget_honored · floor_mib=31382 · arm_ceiling_mib=28310 · remaining_s=10800 · 창 시작 2026-10-05T00:41:06Z 후 0m00s | docs/logs/main/events/watchdog.jsonl:693 |
| 10 | 2026-10-05T00:42:21Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=84947 · rate_mib_s=23006 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → smoke-i5-a-kern 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-05T00:41:06Z 후 1m15s) | docs/logs/main/events/watchdog.jsonl:694 |
| 11 | 2026-10-05T00:42:23Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=44756 · rate_mib_s=25432 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → smoke-i5-a-kern 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-05T00:41:06Z 후 1m17s) | docs/logs/main/events/watchdog.jsonl:695 |
| 12 | 2026-10-05T01:05:17Z | main | `budget_renew` | `smoke-i5-a-kern` | budget_renew · floor_mib=31382 · ttl_s=10800 · remaining_before_s=9349 · 창 시작 2026-10-05T00:41:06Z 후 24m11s | docs/logs/main/events/2026-10.jsonl:230 |
| 13 | 2026-10-05T01:06:36Z | main | `budget_renew` | `smoke-i5-a-kern` | budget_renew · floor_mib=31382 · ttl_s=7200 · remaining_before_s=10721 · 창 시작 2026-10-05T00:41:06Z 후 25m30s | docs/logs/main/events/2026-10.jsonl:231 |
| 14 | 2026-10-05T01:08:57Z | main | `budget_renew` | `smoke-i5-a-kern` | budget_renew · floor_mib=31382 · ttl_s=7200 · remaining_before_s=7059 · 창 시작 2026-10-05T00:41:06Z 후 27m51s | docs/logs/main/events/2026-10.jsonl:232 |
| 15 | 2026-10-05T01:11:03Z | main | `budget_renew` | `smoke-i5-a-kern` | budget_renew · floor_mib=31382 · ttl_s=7200 · remaining_before_s=7074 · 창 시작 2026-10-05T00:41:06Z 후 29m57s | docs/logs/main/events/2026-10.jsonl:233 |
| 16 | 2026-10-05T01:13:07Z | main | `budget_renew` | `smoke-i5-a-kern` | budget_renew · floor_mib=31382 · ttl_s=7200 · remaining_before_s=7076 · 창 시작 2026-10-05T00:41:06Z 후 32m01s | docs/logs/main/events/2026-10.jsonl:234 |
| 17 | 2026-10-05T01:14:31Z | main | `budget_renew` | `smoke-i5-a-kern` | budget_renew · floor_mib=31382 · ttl_s=7200 · remaining_before_s=7116 · 창 시작 2026-10-05T00:41:06Z 후 33m25s | docs/logs/main/events/2026-10.jsonl:235 |
| 18 | 2026-10-05T01:16:34Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-i5-a-kern 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-05T00:41:06Z 후 35m28s) | docs/logs/main/events/2026-10.jsonl:236 |
| 19 | 2026-10-05T01:16:36Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-i5-a-kern 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-05T00:41:06Z 후 35m30s) | docs/logs/main/events/watchdog.jsonl:696 |
| 20 | 2026-10-05T01:17:05Z | main | `budget_declare` | `smoke-i5-a-kern` | 예산 선언(선언값 — 측정 아님) · floor_mib=31382 · weights_mib=73770 · kv_mib=8192 · overhead_mib=11264 · mem_total_mib=124608 · 기동 창 3/4: budget_renew 18회 · 닫힘 budget_clear 2026-10-05T07:42:33Z(선언 후 6h25m28s) · measured_utc 2026-10-05T07:28:55Z 가 이 창 안(이 측정의 기동) | docs/logs/main/events/2026-10.jsonl:238 |
| 21 | 2026-10-05T01:17:05Z | main | `budget_honored` | `smoke-i5-a-kern` | budget_honored · floor_mib=31382 · arm_ceiling_mib=28310 · remaining_s=10800 · 창 시작 2026-10-05T01:17:05Z 후 0m00s | docs/logs/main/events/watchdog.jsonl:697 |
| 22 | 2026-10-05T01:18:23Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=55890 · rate_mib_s=23375 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → smoke-i5-a-kern 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-05T01:17:05Z 후 1m18s) | docs/logs/main/events/watchdog.jsonl:698 |
| 23 | 2026-10-05T01:41:11Z | main | `budget_renew` | `smoke-i5-a-kern` | budget_renew · floor_mib=31382 · ttl_s=10800 · remaining_before_s=9354 · 창 시작 2026-10-05T01:17:05Z 후 24m06s | docs/logs/main/events/2026-10.jsonl:239 |
| 24 | 2026-10-05T01:42:17Z | main | `budget_renew` | `smoke-i5-a-kern` | budget_renew · floor_mib=31382 · ttl_s=7200 · remaining_before_s=10734 · 창 시작 2026-10-05T01:17:05Z 후 25m12s | docs/logs/main/events/2026-10.jsonl:240 |
| 25 | 2026-10-05T01:44:38Z | main | `budget_renew` | `smoke-i5-a-kern` | budget_renew · floor_mib=31382 · ttl_s=7200 · remaining_before_s=7059 · 창 시작 2026-10-05T01:17:05Z 후 27m33s | docs/logs/main/events/2026-10.jsonl:241 |
| 26 | 2026-10-05T01:46:44Z | main | `budget_renew` | `smoke-i5-a-kern` | budget_renew · floor_mib=31382 · ttl_s=7200 · remaining_before_s=7074 · 창 시작 2026-10-05T01:17:05Z 후 29m39s | docs/logs/main/events/2026-10.jsonl:242 |
| 27 | 2026-10-05T02:26:11Z | main | `budget_renew` | `smoke-i5-a-kern` | budget_renew · floor_mib=31382 · ttl_s=10800 · remaining_before_s=4833 · 창 시작 2026-10-05T01:17:05Z 후 1h09m06s | docs/logs/main/events/2026-10.jsonl:243 |
| 28 | 2026-10-05T03:11:11Z | main | `budget_renew` | `smoke-i5-a-kern` | budget_renew · floor_mib=31382 · ttl_s=10800 · remaining_before_s=8100 · 창 시작 2026-10-05T01:17:05Z 후 1h54m06s | docs/logs/main/events/2026-10.jsonl:244 |
| 29 | 2026-10-05T03:56:11Z | main | `budget_renew` | `smoke-i5-a-kern` | budget_renew · floor_mib=31382 · ttl_s=10800 · remaining_before_s=8100 · 창 시작 2026-10-05T01:17:05Z 후 2h39m06s | docs/logs/main/events/2026-10.jsonl:245 |
| 30 | 2026-10-05T04:41:11Z | main | `budget_renew` | `smoke-i5-a-kern` | budget_renew · floor_mib=31382 · ttl_s=10800 · remaining_before_s=8100 · 창 시작 2026-10-05T01:17:05Z 후 3h24m06s | docs/logs/main/events/2026-10.jsonl:246 |
| 31 | 2026-10-05T05:26:11Z | main | `budget_renew` | `smoke-i5-a-kern` | budget_renew · floor_mib=31382 · ttl_s=10800 · remaining_before_s=8100 · 창 시작 2026-10-05T01:17:05Z 후 4h09m06s | docs/logs/main/events/2026-10.jsonl:247 |
| 32 | 2026-10-05T06:11:11Z | main | `budget_renew` | `smoke-i5-a-kern` | budget_renew · floor_mib=31382 · ttl_s=10800 · remaining_before_s=8100 · 창 시작 2026-10-05T01:17:05Z 후 4h54m06s | docs/logs/main/events/2026-10.jsonl:248 |
| 33 | 2026-10-05T06:56:12Z | main | `budget_renew` | `smoke-i5-a-kern` | budget_renew · floor_mib=31382 · ttl_s=10800 · remaining_before_s=8099 · 창 시작 2026-10-05T01:17:05Z 후 5h39m07s | docs/logs/main/events/2026-10.jsonl:249 |
| 34 | 2026-10-05T07:31:11Z | main | `budget_renew` | `smoke-i5-a-kern` | budget_renew · floor_mib=31382 · ttl_s=7200 · remaining_before_s=8701 · 창 시작 2026-10-05T01:17:05Z 후 6h14m06s | docs/logs/main/events/2026-10.jsonl:250 |
| 35 | 2026-10-05T07:33:14Z | main | `budget_renew` | `smoke-i5-a-kern` | budget_renew · floor_mib=31382 · ttl_s=7200 · remaining_before_s=7077 · 창 시작 2026-10-05T01:17:05Z 후 6h16m09s | docs/logs/main/events/2026-10.jsonl:251 |
| 36 | 2026-10-05T07:35:17Z | main | `budget_renew` | `smoke-i5-a-kern` | budget_renew · floor_mib=31382 · ttl_s=7200 · remaining_before_s=7077 · 창 시작 2026-10-05T01:17:05Z 후 6h18m12s | docs/logs/main/events/2026-10.jsonl:252 |
| 37 | 2026-10-05T07:37:22Z | main | `budget_renew` | `smoke-i5-a-kern` | budget_renew · floor_mib=31382 · ttl_s=7200 · remaining_before_s=7075 · 창 시작 2026-10-05T01:17:05Z 후 6h20m17s | docs/logs/main/events/2026-10.jsonl:253 |
| 38 | 2026-10-05T07:38:46Z | main | `budget_renew` | `smoke-i5-a-kern` | budget_renew · floor_mib=31382 · ttl_s=7200 · remaining_before_s=7116 · 창 시작 2026-10-05T01:17:05Z 후 6h21m41s | docs/logs/main/events/2026-10.jsonl:254 |
| 39 | 2026-10-05T07:40:10Z | main | `budget_renew` | `smoke-i5-a-kern` | budget_renew · floor_mib=31382 · ttl_s=7200 · remaining_before_s=7116 · 창 시작 2026-10-05T01:17:05Z 후 6h23m05s | docs/logs/main/events/2026-10.jsonl:255 |
| 40 | 2026-10-05T07:41:12Z | main | `budget_renew` | `smoke-i5-a-kern` | budget_renew · floor_mib=31382 · ttl_s=10800 · remaining_before_s=7138 · 창 시작 2026-10-05T01:17:05Z 후 6h24m07s | docs/logs/main/events/2026-10.jsonl:256 |
| 41 | 2026-10-05T07:42:33Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-i5-a-kern 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-05T01:17:05Z 후 6h25m28s) | docs/logs/main/events/2026-10.jsonl:257 |
| 42 | 2026-10-05T07:42:34Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-i5-a-kern 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-05T01:17:05Z 후 6h25m29s) | docs/logs/main/events/watchdog.jsonl:699 |
| 43 | 2026-10-06T01:51:41Z | main | `budget_declare` | `smoke-i5-a-kern` | 예산 선언(선언값 — 측정 아님) · floor_mib=31382 · weights_mib=73770 · kv_mib=8192 · overhead_mib=11264 · mem_total_mib=124608 · 기동 창 4/4: budget_renew 7회 · 닫힘 budget_clear 2026-10-06T04:53:36Z(선언 후 3h01m55s) · measured_utc 2026-10-05T07:28:55Z 는 이 창 밖(다른 기동 · 측정 뒤 — 이 측정과 무관할 수 있다) | docs/logs/main/events/2026-10.jsonl:262 |
| 44 | 2026-10-06T01:51:41Z | main | `budget_honored` | `smoke-i5-a-kern` | budget_honored · floor_mib=31382 · arm_ceiling_mib=28310 · remaining_s=10800 · 창 시작 2026-10-06T01:51:41Z 후 0m00s | docs/logs/main/events/watchdog.jsonl:700 |
| 45 | 2026-10-06T01:52:56Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=55456 · rate_mib_s=23167 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → smoke-i5-a-kern 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-06T01:51:41Z 후 1m15s) | docs/logs/main/events/watchdog.jsonl:701 |
| 46 | 2026-10-06T02:15:45Z | main | `budget_renew` | `smoke-i5-a-kern` | budget_renew · floor_mib=31382 · ttl_s=10800 · remaining_before_s=9356 · 창 시작 2026-10-06T01:51:41Z 후 24m04s | docs/logs/main/events/2026-10.jsonl:263 |
| 47 | 2026-10-06T03:00:45Z | main | `budget_renew` | `smoke-i5-a-kern` | budget_renew · floor_mib=31382 · ttl_s=10800 · remaining_before_s=8100 · 창 시작 2026-10-06T01:51:41Z 후 1h09m04s | docs/logs/main/events/2026-10.jsonl:264 |
| 48 | 2026-10-06T03:45:45Z | main | `budget_renew` | `smoke-i5-a-kern` | budget_renew · floor_mib=31382 · ttl_s=10800 · remaining_before_s=8100 · 창 시작 2026-10-06T01:51:41Z 후 1h54m04s | docs/logs/main/events/2026-10.jsonl:265 |
| 49 | 2026-10-06T04:30:45Z | main | `budget_renew` | `smoke-i5-a-kern` | budget_renew · floor_mib=31382 · ttl_s=10800 · remaining_before_s=8100 · 창 시작 2026-10-06T01:51:41Z 후 2h39m04s | docs/logs/main/events/2026-10.jsonl:266 |
| 50 | 2026-10-06T04:45:46Z | main | `budget_renew` | `smoke-i5-a-kern` | budget_renew · floor_mib=31382 · ttl_s=7200 · remaining_before_s=9899 · 창 시작 2026-10-06T01:51:41Z 후 2h54m05s | docs/logs/main/events/2026-10.jsonl:267 |
| 51 | 2026-10-06T04:48:12Z | main | `budget_renew` | `smoke-i5-a-kern` | budget_renew · floor_mib=31382 · ttl_s=7200 · remaining_before_s=7054 · 창 시작 2026-10-06T01:51:41Z 후 2h56m31s | docs/logs/main/events/2026-10.jsonl:268 |
| 52 | 2026-10-06T04:50:16Z | main | `budget_renew` | `smoke-i5-a-kern` | budget_renew · floor_mib=31382 · ttl_s=7200 · remaining_before_s=7076 · 창 시작 2026-10-06T01:51:41Z 후 2h58m35s | docs/logs/main/events/2026-10.jsonl:269 |
| 53 | 2026-10-06T04:52:20Z | 미관측 | `measurement` | `measurement (budget window recorded)` | 같은 셀의 다른 측정(측정 노드 축 cluster) · 시각 = 측정 조립(sweep generated_utc = 인증서 measured_utc · 시작 시각 미기록) · 이 발행의 측정 2026-10-05T07:28:55Z 뒤 — 이 측정과 무관할 수 있다 · 출력 평면 스윕(이 측정에 묶이지 않음 — 다른 측정이 덮었다) · 원장: 이 셀 선언 창 안: smoke-i5-a-kern 2026-10-06T01:51:41Z~2026-10-06T04:53:36Z (docs/logs/main/events/2026-10.jsonl:262) · smoke-i5-a-kern 2026-10-06T01:51:41Z~2026-10-06T04:53:37Z (docs/logs/main/events/watchdog.jsonl:700) · 이 시각을 관측하지 않은 원장: sub(기록 2026-09-03T09:51:45Z~2026-09-11T11:10:38Z) · 직전 예산 행 budget_renew(smoke-i5-a-kern) 2026-10-06T04:50:16Z(docs/logs/main/events/2026-10.jsonl:269) · 직후 예산 행 budget_clear 2026-10-06T04:53:36Z(docs/logs/main/events/2026-10.jsonl:270) | output/multi/benchlog/sweep_i5-a-kern/sweep_index.json:4 |
| 54 | 2026-10-06T04:53:36Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-i5-a-kern 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-06T01:51:41Z 후 3h01m55s) | docs/logs/main/events/2026-10.jsonl:270 |
| 55 | 2026-10-06T04:53:37Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-i5-a-kern 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-06T01:51:41Z 후 3h01m56s) | docs/logs/main/events/watchdog.jsonl:702 |

**기동 시도**(예산 선언 `budget_declare` 마다 한 시도 · 같은 노드의 이어지는 행과 선언 없는 노드(엔진 로그)의 그 시각 행을 묶었다 · 사살 · 트립 · 거부는 닫힘과 따로 센다 · `measurement` 행은 측정 기록이라 시도로 세지 않는다(위 표에만) — 해석은 저작자 몫)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 1 | main | `smoke-i5-a-kern` | 2026-10-05T00:11:56Z | 1회 · 첫 2026-10-05T00:35:51Z | 없음 | `budget_clear` 2026-10-05T00:40:43Z | 4 |
| 2 | main | `smoke-i5-a-kern` | 2026-10-05T00:41:06Z | 6회 · 첫 2026-10-05T01:05:17Z | 없음 | `budget_clear` 2026-10-05T01:16:34Z | 4 |
| 3 | main | `smoke-i5-a-kern` | 2026-10-05T01:17:05Z | 18회 · 첫 2026-10-05T01:41:11Z | 없음 | `budget_clear` 2026-10-05T07:42:33Z | 3 |
| 4 | main | `smoke-i5-a-kern` | 2026-10-06T01:51:41Z | 7회 · 첫 2026-10-06T02:15:45Z | 없음 | `budget_clear` 2026-10-06T04:53:36Z | 3 |
<!-- /FACT:event_timeline -->

<!-- FACT:measurement_env -->
**측정 실행의 env 관측 18행**(측정한 실행의 엔진 로그가 echo 한 값 · 기계 발췌 — 측정 당시 값의 정본이다. 표에 없는 키는 로그가 echo 하지 않은 것이지 설정되지 않았다는 뜻이 아니다 · 측정 뒤 재생성된 env 형상은 01 §1.1 을 본다)

| # | 키 | 값 | 노드 | 종류 | 출처(첫 줄) | 관측 |
|---|---|---|---|---|---|---|
| 1 | `NCCL_SOCKET_IFNAME` | `<nic:cluster>` | sub | env-echo | output/multi/benchlog/lite_publish/i5-a-kern_20261005T072855Z/lite_engine_i5-a-kern.log:L85 | 로그 1개 · Ray 접힘 +3(접힌 사본의 노드 미관측) |
| 2 | `NCCL_SOCKET_IFNAME` | `<nic:cluster>` | sub | param-echo | output/multi/benchlog/lite_publish/i5-a-kern_20261005T072855Z/lite_engine_i5-a-kern.log:L86 | 로그 1개 · Ray 접힘 +2(접힌 사본의 노드 미관측) |
| 3 | `nccl:version` | `2.30.7+cuda13.3` | sub | runtime | output/multi/benchlog/lite_publish/i5-a-kern_20261005T072855Z/lite_engine_i5-a-kern.log:L89 | 로그 1개 |
| 4 | `NCCL_NET_PLUGIN` | `spcx` | sub | env-echo | output/multi/benchlog/lite_publish/i5-a-kern_20261005T072855Z/lite_engine_i5-a-kern.log:L91 | 로그 1개 |
| 5 | `NCCL_IB_DISABLE` | `0` | sub | env-echo | output/multi/benchlog/lite_publish/i5-a-kern_20261005T072855Z/lite_engine_i5-a-kern.log:L96 | 로그 1개 · Ray 접힘 +2(접힌 사본의 노드 미관측) |
| 6 | `NCCL_IB_HCA` | `=<nic:cluster>,<nic:cluster>` | sub | param-echo | output/multi/benchlog/lite_publish/i5-a-kern_20261005T072855Z/lite_engine_i5-a-kern.log:L97 | 로그 1개 · Ray 접힘 +2(접힌 사본의 노드 미관측) |
| 7 | `NCCL_IB_MERGE_NICS` | `1` | sub | env-echo | output/multi/benchlog/lite_publish/i5-a-kern_20261005T072855Z/lite_engine_i5-a-kern.log:L99 | 로그 1개 |
| 8 | `nccl:net_ib_devices` | `[RO]; OOB <nic:cluster>:<node:sub><0>` | sub | runtime | output/multi/benchlog/lite_publish/i5-a-kern_20261005T072855Z/lite_engine_i5-a-kern.log:L101 | 로그 1개 · Ray 접힘 +2(접힌 사본의 노드 미관측) |
| 9 | `NCCL_IB_QPS_PER_CONNECTION` | `4` | sub | env-echo | output/multi/benchlog/lite_publish/i5-a-kern_20261005T072855Z/lite_engine_i5-a-kern.log:L111 | 로그 1개 · Ray 접힘 +2(접힌 사본의 노드 미관측) |
| 10 | `nccl:network` | `IB` | sub | runtime | output/multi/benchlog/lite_publish/i5-a-kern_20261005T072855Z/lite_engine_i5-a-kern.log:L124 | 로그 1개 · Ray 접힘 +3(접힌 사본의 노드 미관측) |
| 11 | `NCCL_DMABUF_ENABLE` | `0` | sub | env-echo | output/multi/benchlog/lite_publish/i5-a-kern_20261005T072855Z/lite_engine_i5-a-kern.log:L126 | 로그 1개 |
| 12 | `NCCL_CROSS_NIC` | `1` | sub | env-echo | output/multi/benchlog/lite_publish/i5-a-kern_20261005T072855Z/lite_engine_i5-a-kern.log:L129 | 로그 1개 |
| 13 | `NCCL_MAX_NCHANNELS` | `8` | sub | env-echo | output/multi/benchlog/lite_publish/i5-a-kern_20261005T072855Z/lite_engine_i5-a-kern.log:L148 | 로그 1개 |
| 14 | `NCCL_LL128_BUFFSIZE` | `262144` | sub | env-echo | output/multi/benchlog/lite_publish/i5-a-kern_20261005T072855Z/lite_engine_i5-a-kern.log:L151 | 로그 1개 |
| 15 | `NCCL_BUFFSIZE` | `1048576` | sub | env-echo | output/multi/benchlog/lite_publish/i5-a-kern_20261005T072855Z/lite_engine_i5-a-kern.log:L152 | 로그 1개 |
| 16 | `NCCL_PROTO` | `^LL128` | sub | env-echo | output/multi/benchlog/lite_publish/i5-a-kern_20261005T072855Z/lite_engine_i5-a-kern.log:L166 | 로그 1개 · Ray 접힘 +3(접힌 사본의 노드 미관측) |
| 17 | `NCCL_IB_SPLIT_DATA_ON_QPS` | `0` | sub | env-echo | output/multi/benchlog/lite_publish/i5-a-kern_20261005T072855Z/lite_engine_i5-a-kern.log:L175 | 로그 1개 |
| 18 | `NCCL_IB_GID_INDEX` | `3` | sub | env-echo | output/multi/benchlog/lite_publish/i5-a-kern_20261005T072855Z/lite_engine_i5-a-kern.log:L179 | 로그 1개 |

- 값 주: 기계 치환(pii.substitution_table · 장치·호스트·주소는 자리표시 — 원문은 출처 줄)

- Ray 접힘: 출처 줄 끝의 `[repeated Nx across cluster]` — Ray 가 다른 프로세스의 같은 줄 N개를 한 줄로 접었다. 노드 칸은 **첫 줄**의 노드이고, 접힌 사본이 어느 노드의 것인지는 로그에 없다(관측 대상 밖).

- 로그 창: `tail-800(lite_bench.sh@dcb713a7436c L191)`(이 표의 출처 로그 1개) — 이 캡처는 엔진 출력의 **마지막 N줄**이다(측정 도구의 `tail -N` 상한에 닿았다 · 생산자 evidence `log_window`). 창 밖에서 echo 한 줄은 이 표에 없다 — 표에 없는 키 · 한 노드에만 있는 키는 창 밖이었을 수 있다.
<!-- /FACT:measurement_env -->

> 이 절이 답하는 질문: 수신자가 이 zip 만으로 이 셀을 다시 띄우려면 어떤 순서로 무엇을 하고, 각 단계의 성공을 무엇으로 판정하며, 얼마나 기다려야 하는가?

**(a) 사전 준비.** 가중치는 팩 A(sfxnz EXL3 2.0bpw · plan 표 333 GiB · 엔진 보고 `Checkpoint size: 332.92 GiB`)를 컨테이너 `/app/quant_models/DeepSeek/DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz` 로 보이게 마운트한다 — 호스트 원천은 `<manifest.quant_model_path>`(서브 전달 env `QUANT_MODEL_PATH` · 1.5). 측정 실행에서 이 원천은 CIFS(NAS)였다(i5_master.log L705 · L713 `Filesystem type for checkpoints: CIFS`). Engram 테이블은 **각 노드 로컬 디스크**에 스테이징한 디렉터리를 컨테이너 `/app/ple_mmap/v41-engram-a` 로 마운트한다(호스트 `<manifest.ple_mmap_host_path>` · 서브 전달 env `PLE_MMAP_HOST_PATH` · yaml `hf-overrides`). 스테이징 파일 목록 · 크기는 계보에 노드별로 기록되지 않았다(스모크 게이트의 Engram 산정은 193,365 MiB — smoke_i5.log L19). 필수 env 의 자리: 커널 레버 · 이식 손잡이 · NCCL tail 은 `artifacts/triplet/.env.i5-a-kern`(셀 env) · 이미지 정체성(`IMAGE_TAG` · `BUILD_DOCKERFILE` · `VLLM_REF` · `SM12X_PORT`)도 같은 파일 · 마운트원은 `artifacts/compose/.env.template` · 인터커넥트는 `.env.interconnect.template`. 측정 당시 env 의 정본은 위 '측정 실행의 env 관측' 18행(바인딩 측정 엔진 로그 보존본의 echo — `NCCL_NET_PLUGIN=spcx` · `NCCL_IB_DISABLE=0` · NCCL tail 4줄 등)이다. 그 보존본은 꼬리 800줄이라 sub 줄만 남았고, 같은 기동 전체 사본에는 rank0 의 같은 echo 도 있다(i5_master.log L114 · L337 · L356 · L375). 렌더 단계 명령은 **지금의** 렌더러로 형상을 다시 만드는 명령이다 — 측정 당시 값은 셀 env 와 위 로그 echo 로 대조한다.

**(b) 빌드.** 양 노드에서 각각 짓는다(이미지 전송 ✗ — 요건은 동일 ABI). 선택자 `Dockerfile.source-build` · build-arg `SM12X_PORT=1` · `VLLM_REF=v0.30.0` · `TORCH_CUDA_ARCH=12.1a` · `RAY_VERSION=2.48.0`(위 재구성 명령). 관측 소요는 위 표 2행 43분 47초(캠페인 build.status — 메인 기준 같은 셀 가장 이른 시작 ~ 가장 늦은 끝)다. 빌드는 GitHub · 업스트림 인덱스에서 핀 커밋 tarball 과 1.5 GB 휠을 받는다 — 저하된 회선에서는 W5 의 이어받기 경로가 동작한다. 서브 빌드는 A2A 지시 루프로 했다(devlog_26100610 §2 — 서브 직접 조작 ✗).

**(c) 기동 순서와 성공 판정.** `multinode_serve_smoke.sh i5-a-kern --keep-up` 이 예산 선언(양 노드) → master(Ray head + serve) → slave(Ray worker) → health 폴링(READY_MAX 60분) → chat 추론 1회 → attestation → serve proof 순으로 간다(smoke_i5.log). 성공 판정 = `READY` + `SMOKE PASS` + attestation 4축 equal. 관측 소요: 시도 1 의 `READY ~1410s`(smoke_i5.log L35) · testlog_26100420 §2.6 도 READY ~1410 s · 측정 기동(시도 3)은 예산 선언 01:17:05Z → 엔진 로그 `Application startup complete` 직후 첫 엔진 통계 01:41:11Z 로 약 24분이다(i5_master.log L1051 근처 — 관측 간격이지 독립 측정 아님). 그 안에서 가중치 로드가 랭크당 660 s + 329 s(두 번째 로드는 드래프터로 보인다 — 추론) · `Model loading took 74.64 GiB memory and 1172.6~1175.0 seconds` · 캡처 3초다. 패치 발화 서명은 1.2 각 항목에 적었다 — 최소한 `kv null block zeroed after capture` · `engram prestage armed` · `dense GEMV armed`(9 모양 × 2 랭크) · `moe prep fused self-check bit-exact` · `mm_mxfp8 backend=auto` · `woa-requant fp8 einsum engaged` · `p2b coop dataflow kernel engaged` 를 본다. 정상인데 무서워 보이는 줄: 로드 중 `No available shared memory broadcast block found in 60 seconds`(가중치 로드 · 캡처 동안 반복 — 엔진이 '컴파일 · 가중치/KV 양자화 같은 긴 작업' 일 수 있다고 스스로 적는다) · `Spectrum-X (SPCX) NCCL plugin is not supported on device ..., skipping`(장치별 DEVX 경로를 건너뛴 줄 — 그 뒤에도 SPCX net 플러그인 위에서 `Using network IB` 로 진행) · `Failed to import the DeepSelect extension (vllm._deepselect_C)` 경고. 이 셀 기동에서 이것들은 기동 실패로 이어지지 않았다.

**기동 시도 수 = 4**(위 표). 원장의 사살 · 트립 · 거부 행은 네 시도 모두 0 이지만, **세션 열 가드(thermal_kill)의 kill 은 원장에 기록되지 않는다** — 시도 1(기동 1 · 125k needle 프리필 중) · 시도 2(기동 2 · Full c=2 run2) · 시도 3(기동 3 · Full c=2 run3 끝)이 각각 1회 그 kill 로 끝났다(testlog_26100510 §2 · §3 · §4). 바인딩 측정(lite 07:28:55Z)은 시도 3 창 안이고 그 kill 이전이다. 시도 4(기동 4 · 2026-10-06)는 측정 뒤 재기동으로 Full c=1 만 쟀다(kill 없음). 각 기동의 READY: 기동 1 · 3 ~1410 s · 기동 4 ~1420 s(testlog_26100510). 원장의 `watchdog_highrate_hold`(`legacy_rule_would_trip=true`)는 가중치 로드 초반 가용 메모리가 초당 2만 MiB 넘게 줄 때 옛 규칙이라면 트립했을 것을 현 규칙이 보류한 기록이다(행 3 · 4 · 10 · 11 · 22) (시도 4 는 행 45) — 모든 시도에서 로드 시작 1분 남짓에 나타났고 사살로 이어지지 않았다.

**(d) 호스트 메모리 안전 예산 선언.** 선언값(측정 아님): weights 73,770 MiB · KV 8,192 MiB · overhead 11,264 MiB · mem_total 124,608 MiB → 바닥 31,382 MiB · 워치독 arm 상한 28,310 MiB · TTL 10,800 s(양 노드 · 01 §1.4 행 1 · smoke_i5.log L19-20). 실측은 엔진 `Model loading took 74.64 GiB`(랭크당) · `Initial free memory 110.58 GiB`(TP0) / `114.81 GiB`(TP1) · KV 예약 8.0 GiB · 캡처 0.43 GiB · lite 측정 시 시스템 RAM main 105.5 GiB(87%) · sub 102.2 GiB(84%). `--keep-up` 이면 갱신 루프가 TTL 을 이어 붙인다(시도 3 갱신 18회).

**(e) 측정 명령.** 위 표의 bench 단계는 bench JSON 필드에서 **재구성**한 명령이다(실행 원문 아님 · `--random-input-len 542` 는 chat 템플릿을 포함한 실측 입력 토큰의 평균이라 도구 원문 인자 512 와 다르다). 도구가 정한 인자는 03 §3.4 의 `lite_bench.sh@dcb713a7436c` 원문(512 → 128 · ignore-eos · conc 1 · warmup 0/1)에서 읽는다. 측정 창은 07:28:55Z → 07:29:30Z(35 초)다.

**(f) 엔진이 스스로 고른 런타임 경로.** 통신: NCCL 2.30.7 이 `Using network IB`(RoCE · 장치 2개 병합) · `Connected all rings, use ring PXN 0 GDR 0`(GPU Direct RDMA 꺼짐) · SPCX 외부 플러그인은 장치 미지원으로 건너뜀(i5_master.log L159 · L490 · L140). KV: `Using DeepSeek's fp8_ds_mla KV cache format.` · `Using BLHNC KV cache layout.` · KV 그룹 블록 크기 lcm 128. 그래프: PIECEWISE 5 · FULL 2 · DSpark FULL 2 캡처 + `Breakable CUDA graph enabled`. 스케줄러: spec 설정 때문에 `max_num_scheduled_tokens` 를 1536 으로 둔다는 경고. Engram: `Resolved Engram configuration: EngramConfig(cpu_offload=True, ...)` 로 찍히지만 88 prestage 가 켜져 있으면 file-backed 백엔드의 prefetch 사이드 스트림을 만들지 않는다(88 헤더 what (1)). 가중치 로드: CIFS 라 auto-prefetch 가 꺼졌다.

## 1.5 서브 레시피 해설
<!-- FACT:sub_recipe -->
원본: `artifacts/compose/sub_recipe.json`(기계 파생 — 스모크와 같은 함수 `slave_forward.derive` 의 출력).

| 역할 | 하는 일 |
|---|---|
| `master` | ray head + vllm serve(트리플렛 source · 모델 러너 실행) |
| `master_runtime_patch` | none(런타임 패치 파일 없음) |
| `runner_gpu_join_literal` | {"lines": [79], "note": "master 는 ray status 의 GPU 합류가 이 리터럴과 같아질 때까지 기다린다 — 노드당 GPU·노드 수가 다른 클러스터에서는 이 줄을 고치지 않으면 대기에서 멈춘다(arch 의 <G>g<N>n 과 숨은 결합).", "value": "2"} |
| `slave` | ray worker(트리플렛 미수령 — 같은 이미지·같은 compose·같은 serve_runner.sh 가 NODE_ROLE 로 분기) |
| `slave_config_file` | default |
| `slave_runtime_patch` | not-armed — slave CONFIG_FILE='default'(slave_forward.NEVER_FORWARD) → arm_patch.sh 가 /app/configs/default_patch.py 를 찾고 없으면 no-op (K11) |
| `source` | {"runner_gpu_join_literal": "render_dockerfile.SHARED_ASSET_DIR/serve_runner.sh(측정 당시 개정 — slots.compose.evidence.selected_revisions)", "runtime_patch": "output/multi/configs/<CONFIG_FILE>_patch.py 실재 여부", "slave_config_file": "compose-default:CONFIG_FILE × slave_forward.NEVER_FORWARD"} |

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
| `observed` | {"build_ledger": {"compared": "dockerfile · build_args(− exempt) · patches(phase,file,script_sha256,result) · vllm git sha", "identity_exempt_build_args": ["BUILD_JOBS"], "main_identity_sha256": "6530d8816e246fb03bf76bc7fa723ad846651a00312aa4e5fb6d6bd23eacf13a", "sub_identity_sha256": "6530d8816e246fb03bf76bc7fa723ad846651a00312aa4e5fb6d6bd23eacf13a", "verdict": "equal"}, "driver": {"main": "580.178.04", "sub": "580.178.04", "verdict": "equal"}, "torch": {"main": "2.13.0a0+9186a08b2c.nv26.7.59513937", "sub": "2.13.0a0+9186a08b2c.nv26.7.59513937", "verdict": "equal"}, "vllm_sha": {"main": "ced6857afa0ea7b2e3f0846a62e1394e90f15607", "sub": "ced6857afa0ea7b2e3f0846a62e1394e90f15607", "verdict": "equal"}} |
| `source` | output/multi/benchlog/attestation_i5-a-kern.json |

> **attestation 범위** — config `i5-a-kern` · phase serve · 작성 2026-10-06T02:15:44Z(output/multi/benchlog/attestation_i5-a-kern.json 파일 mtime(파일시스템 관측 — 문서에 시각 칸이 없다 · 기록 필드 아님)) · 묶은 근거 cell-name · 파일 `output/multi/benchlog/attestation_i5-a-kern.json` · 측정 창 대비 작성 2026-10-06T02:15:44Z > 측정 끝 2026-10-05T07:29:30Z — 측정 뒤 재기동의 관측: **same config(i5-a-kern) · serve — 측정 뒤 재기동의 관측이다(이 측정 실행의 관측이 아니다 · 파일은 다음 실행이 덮는다)**
<!-- /FACT:sub_recipe -->

<!-- FACT:measurement_env_nodes -->
**노드별 측정 env**(01 §1.4 표를 키 × 노드로 접은 것 · `만 관측` = 다른 쪽 줄이 캡처 로그에 없다 · Ray 가 같은 줄을 접은 키(`[repeated Nx across cluster]`)와 꼬리 캡처 로그(01 §1.4 로그 창)에서 한쪽만 남은 키는 다른 쪽 부재를 단언하지 않는다)

- 양 노드 같은 값 0개: 없음

| 키 | main | sub | 노드 미특정 | 판정 |
|---|---|---|---|---|
| `NCCL_BUFFSIZE` | — | `1048576` | — | sub 만 캡처 창에 남음 — main 여부 미관측(꼬리 캡처) |
| `NCCL_CROSS_NIC` | — | `1` | — | sub 만 캡처 창에 남음 — main 여부 미관측(꼬리 캡처) |
| `NCCL_DMABUF_ENABLE` | — | `0` | — | sub 만 캡처 창에 남음 — main 여부 미관측(꼬리 캡처) |
| `NCCL_IB_DISABLE` | — | `0` | — | sub 만 줄에 남음 · Ray 접힘 +2 — main 사본 여부 미관측 |
| `NCCL_IB_GID_INDEX` | — | `3` | — | sub 만 캡처 창에 남음 — main 여부 미관측(꼬리 캡처) |
| `NCCL_IB_HCA` | — | `=<nic:cluster>,<nic:cluster>` | — | sub 만 줄에 남음 · Ray 접힘 +2 — main 사본 여부 미관측 |
| `NCCL_IB_MERGE_NICS` | — | `1` | — | sub 만 캡처 창에 남음 — main 여부 미관측(꼬리 캡처) |
| `NCCL_IB_QPS_PER_CONNECTION` | — | `4` | — | sub 만 줄에 남음 · Ray 접힘 +2 — main 사본 여부 미관측 |
| `NCCL_IB_SPLIT_DATA_ON_QPS` | — | `0` | — | sub 만 캡처 창에 남음 — main 여부 미관측(꼬리 캡처) |
| `NCCL_LL128_BUFFSIZE` | — | `262144` | — | sub 만 캡처 창에 남음 — main 여부 미관측(꼬리 캡처) |
| `NCCL_MAX_NCHANNELS` | — | `8` | — | sub 만 캡처 창에 남음 — main 여부 미관측(꼬리 캡처) |
| `NCCL_NET_PLUGIN` | — | `spcx` | — | sub 만 캡처 창에 남음 — main 여부 미관측(꼬리 캡처) |
| `NCCL_PROTO` | — | `^LL128` | — | sub 만 줄에 남음 · Ray 접힘 +3 — main 사본 여부 미관측 |
| `NCCL_SOCKET_IFNAME` | — | `<nic:cluster>` | — | sub 만 줄에 남음 · Ray 접힘 +3 — main 사본 여부 미관측 |
| `nccl:net_ib_devices` | — | `[RO]; OOB <nic:cluster>:<node:sub><0>` | — | sub 만 줄에 남음 · Ray 접힘 +2 — main 사본 여부 미관측 |
| `nccl:network` | — | `IB` | — | sub 만 줄에 남음 · Ray 접힘 +3 — main 사본 여부 미관측 |
| `nccl:version` | — | `2.30.7+cuda13.3` | — | sub 만 캡처 창에 남음 — main 여부 미관측(꼬리 캡처) |
<!-- /FACT:measurement_env_nodes -->

> 이 절이 답하는 질문: 서브(Ray worker) 노드는 메인과 무엇이 다르고, 무엇을 반드시 같게 맞춰야 하는가?

**(1) 역할 차이.** master 는 Ray head + `vllm serve`(트리플렛 `i5-a-kern` 을 받아 모델 러너 실행)이고, slave 는 Ray worker 다 — 트리플렛을 받지 않고(`CONFIG_FILE=default`) 같은 이미지 · 같은 compose · 같은 `serve_runner.sh` 가 `NODE_ROLE` 로 분기한다. master 는 `ray status` 의 GPU 합류가 러너 리터럴 `2` 와 같아질 때까지 기다린 뒤 serve 한다(위 표 `runner_gpu_join_literal` — 노드 수 · GPU 수가 다르면 이 줄을 고쳐야 한다). slave 는 모델 런타임 패치를 무장하지 않는다(`default_patch.py` 부재 → no-op). 이 셀은 런타임 패치 자체가 없어 양쪽이 같다 — 모든 이식은 이미지에 구워져 있고, 서브 랭크(TP1)의 발화는 Ray 가 전달한 env 로 켜진다(i5_master.log 의 `Worker_TP1` 줄: `dense GEMV armed` · `mm_mxfp8 backend=auto` · `woa-requant` · `engram prestage armed`).

**(2) 서브 전달 env 무리.** 이미지 정체성(`IMAGE_TAG` · `BUILD_DOCKERFILE` · `VLLM_REF` · `SM12X_PORT`) — 빠지면 서브가 compose 기본 태그 · Dockerfile · ref 로, 또는 `SM12X_PORT` 없이 stock 으로 빌드돼 이식 패치가 마스터에만 들어간다. 마운트(`QUANT_MODEL_PATH` · `NAS_MODEL_PATH` · `TIKTOKEN_HOST_PATH` · `PLE_MMAP_HOST_PATH`) — 서브도 자기 몫의 가중치와 Engram 행을 직접 읽으므로(`File-backed Engram ... rank=1/2` · i5_master.log L707) 마운트원이 서브에 있어야 한다. 클러스터(`SLAVE_CONTAINER_NAME`) — 워치독 필터 매칭. 서빙 env(커널 레버 `DSV41_*` · `EXL3PORT_*` · NCCL)는 서브 컨테이너 env 가 아니라 master 의 vLLM 이 Ray 로 워커에 복사한다 — 셀 env 의 `VLLM_RAY_EXTRA_ENV_VAR_PREFIXES_TO_COPY=EXL3_,DSV41_,GLM53_,EXL3PORT_` 가 그 목록이다(빠지면 TP1 레버가 꺼진 채 돈다 — 02 §2.6).

**(3) 선행조건.** 팩 A 가 서브에서도 **같은 컨테이너 경로**로 보여야 하고, Engram 스테이징 디렉터리는 서브의 로컬 디스크에도 같은 경로로 실재해야 한다(위 표 선행조건 · plan_26100319 §3 R-1 스테이징). 드라이버 계열 동일(관측 양 노드 580.178.04). 인터커넥트는 RoCE 2포트(`NCCL_IB_HCA` 2장치 · GID 인덱스 3 env echo)가 준비돼 있어야 한다 — 실값(장치 이름 · 주소)은 manifest `interconnect` 에서 렌더한 `.env.interconnect` 가 갖는다.

**(4) ABI 동일성.** attestation 관측: build_ledger identity sha256 main = sub(`6530d881…`) · driver 580.178.04 = 580.178.04 · torch · vllm_sha equal(위 표 `observed`) — 같아야 하는 것은 digest 가 아니라 ABI 이고 실제로 image_id 는 노드마다 다르다(00 §0.4). 이 attestation 에는 범위 줄이 붙어 있다 — 지금 파일(mtime 2026-10-06T02:15:44Z)은 측정 **뒤** 재기동(시도 4)의 관측이라 바인딩 측정 기동의 ABI 관측이 아니다. 같은 이미지 · config 의 기동 1 · 3 · 4 스모크가 모두 4축 equal 이었다고 testlog_26100510 이 적는다(기동 3 무렵의 사본으로 보이는 파일이 docs/simlog/26100420_V4_1_Flash_정식화_F2_K6/attestation_i5-a-kern.json 에 있다 — 사본 mtime 2026-10-05T07:42Z 로 추정한 귀속이다). 빌드 원장은 노드별로 같은 패치 · 같은 script_sha256 · 같은 결과를 기록했다(identity 일치).

**(5) 측정 당시 인터커넥트 실효값.** 01 §1.4 측정 env 관측 18행과 위 '노드별 측정 env' 가 정본이다 — 바인딩 측정의 엔진 로그 보존본(꼬리 800줄)에서 **sub 줄만** 남아 main 여부는 그 표가 '미관측' 으로 둔다. 같은 기동의 전체 사본(i5_master.log — FACT 가 대조하지 않은 파일)에는 rank0 워커의 echo 도 있다: `NCCL_IB_DISABLE set by environment to 0.` · `NCCL_IB_MERGE_NICS set by environment to 1.` · `NCCL_IB_QPS_PER_CONNECTION set by environment to 4.` · `NCCL_DMABUF_ENABLE set by environment to 0.` · `NCCL_CROSS_NIC set by environment to 1.` · `NCCL_IB_GID_INDEX set by environment to 3.` · `NCCL_NET_PLUGIN set by environment to spcx` · tail 4줄(`NCCL_MAX_NCHANNELS 8` · `NCCL_LL128_BUFFSIZE 262144` · `NCCL_BUFFSIZE 1048576` · `NCCL_PROTO ^LL128`). 이 줄들은 대부분 rank0 워커(`RayWorkerProc pid=2316`) 줄이고 일부는 Ray 가 `[repeated 2x across cluster]` 로 접었다(L547 · L562) — 접힌 사본이 어느 노드의 것인지는 로그에 없다. 두 사본을 합치면 env 관측 표의 키 대부분이 rank0 · sub 양쪽에서 같은 값으로 echo 됐다(전체 사본 rank0 줄 + 보존본 sub 줄 — 두 파일을 저작자가 대조한 것이지 FACT 판정이 아니다). 장치 이름 · 주소는 manifest 형상 자리표시로만 적는다. 지금의 렌더러가 내는 `.env.interconnect` 와 위 echo 가 다르면 echo 쪽이 측정 당시다.

## 1.6 재현 절차 — 기동 전 준비(사람이 할 일)
> 1.4 의 명령을 치기 **전에** 사람이 손으로 해야 하는 것이다 — 가중치 획득 · 스테이징 생성 · 네트워크가 필요한 단계. 에이전트가 대신 할 수 없는 자리(다운로드 승인 · 물리 경로 · 외부 계정)가 여기 모인다.

> 이 절이 답하는 질문: 이 셀을 기동하기 전에 사람이 준비해야 하는 것은 무엇이며, 각각을 어떤 명령으로 만들고, 그 절차의 출처는 어디인가?

### 가중치 획득 — 팩 A(sfxnz EXL3 2.0bpw)
- 체크포인트: DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz(viterbi · lmhead-mxfp8 · Engram 동봉). base_model 은 `deepseek-ai/DeepSeek-V4.1-Flash`(체크포인트 README · 00 §0.2). HF repo · revision 은 체크포인트에 `.git` 이 없어 **미관측**이고, 계보 문서도 팩 A 의 다운로드 repo · revision 을 적지 않았다(plan_26100319 §9 는 저자 GitHub 쿡북 HEAD 3de9146 만 적는다).
- 획득 모드: 관리 NAS 적재 — 다운로드는 **사람이 자기 토큰으로** 분리 실행했고(plan_26100208 §5 Phase 0 · 외부 회선 수신 ≈12 MB/s 로 약 24시간 예상), 완료 판정은 후보별 verify JSON(`ok: true`)이었다. 수행지시서(request_26100207)는 이 계보 목록 밖이라 명령 원문을 싣지 않는다. 명령 형태는 `huggingface-cli download <팩 A repo> --revision <rev> --local-dir <manifest.quant_model_path>/DeepSeek/DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz` 꼴이 될 것이다 — **추론 — 실행 검증 ✗**(repo 이름 · revision 미기록).
- 크기: plan 표 333 GiB(plan_26100208 §2) · 엔진 보고 332.92 GiB(i5_master.log L705). 노드마다 같은 컨테이너 경로로 보여야 한다 — 이 셀은 양 노드가 같은 NAS(CIFS)를 마운트했다(i5_master.log L705 · L713 — 두 랭크 모두 CIFS 보고).
- 사람 결정: 서드파티 가중치 사용 승인(plan_26100208 §8 D9 — 그 plan 시점 미결) · 라이선스(vllm-exl3 AGPL-3.0 · 가중치 서드파티 — plan_26100208 §7 R4)는 수신자가 다시 판단한다. 모델 획득은 사람의 승인 게이트다.

### Engram 스테이징 — 노드 로컬 디스크
- 무엇: 팩 A 동봉 Engram 테이블을 각 노드 로컬 NVMe 의 `<manifest.ple_mmap_host_path>/v41-engram-a` 로 둔다(plan_26100319 §3 R-1 · §4 표 '팩 동봉 → 로컬 NVMe 스테이징 완료'). 컨테이너 안 경로는 `/app/ple_mmap/v41-engram-a`(yaml `hf-overrides`).
- 생성 명령: 계보에 **미기록**이다(복사 · 패킹 명령 · 파일 목록 · 크기 · 해시 모두 계보 문서에 없다). 83 헤더는 `engram_layout.py` 를 오프라인 `pack_engram` 도구용으로 설치한다고만 적고, 이 셀의 로그는 `packed=False`(i5_master.log L529)라 packed 샤드를 쓰지 않았다 — 원본 safetensors 샤드를 그대로 두는 배치로 보인다(추론 — 실행 검증 ✗).
- 위치 조건: 서브에도 **같은 경로**로 실재해야 한다(1.5 선행조건 · 서브 전달 env `PLE_MMAP_HOST_PATH`). 가중치처럼 NAS 원천을 직접 가리키는 구성은 이 셀에서 쓰지 않았다.
- 생성 확인: 스모크 게이트가 Engram 선언을 읽어 `engram=file-backed 선언 → 게이트 ckpt 에서 −193365MiB(상주 ✗)` 를 찍는지(smoke_i5.log) · 엔진이 `file-backed Engram active (/app/ple_mmap/v41-engram-a)` 와 층별 `rows=[...]` 를 찍는지(i5_master.log L529 · L708).

### 네트워크가 필요한 단계와 오프라인 대체
- 가중치 다운로드(위) — 미리 NAS 에 받아 둔다.
- 베이스 이미지 pull: `nvcr.io/nvidia/pytorch:26.07-py3`(Dockerfile.source-build `FROM` · 00 §0.4) — 미리 pull 해 둔다.
- 빌드 중 외부 fetch: vLLM 소스 `v0.30.0`(GitHub) · 핀 커밋 tarball(sfxnz 3de91468 · Mia 커밋 · vllm-exl3 d3cfd394 · exllamav3 e648f1a1 — 각 패치 헤더 provenance · sha256 검증) · DeepGEMM nv_dev(10 헤더 '폐쇄망: 사전미러 후 로컬 경로 지정') · flashinfer-cubin 휠(업스트림 인덱스 · 1.5 GB · W5) · pip 의존. 오프라인 대체는 각 출처를 같은 커밋 · 같은 sha256 으로 미러하는 것이다 — 이 계보에서 오프라인 빌드를 실행한 기록은 없다(추론 — 실행 검증 ✗).
- 런타임 JIT: 92 의 dense GEMV 커널은 첫 armed 층에서 nvcc 로 컴파일한다(네트워크 불필요 · 랭크당 ~1분 — 92 헤더).
