# 01 · 산출물 — 무엇이 실제로 쓰였고, 어떻게 다시 띄우는가

## 1.1 적용 판정 표
> 기계가 적용 증거(빌드 원장 · 원장이 없으면 라벨된 재구성) × 파일 × 선언을 대사한 결과다. `artifacts/` 에는 **적용된 것만** 실렸다 — 빌드 때 스스로 skip 된 패치와 쓰이지 않은 평면의 파일은 표에만 남고 싣지 않는다.

<!-- FACT:slots -->
| 슬롯 | 실림 | 파일 | 신호 | 적용 증거 | 사유 |
|---|---|---|---|---|---|
| `build_patch_post` | 실림 | `artifacts/build_patch_post/10-deepgemm.sh` · `artifacts/build_patch_post/20-triton-kernels.sh` · `artifacts/build_patch_post/30-mxfp4-triton-sm121.sh` · `artifacts/build_patch_post/40-humming-nvml-gb10.sh` | 2-signal | build-ledger:attestation:output/multi/benchlog/attestation_ds4f0731-1m-spec7-roce.json#main | build_patches/ — 적용 판정 build-ledger: applied 4 · 실린 것 = skip 아닌 패치(기계 파생) |
| `build_patch_pre` | 실림 | `artifacts/build_patch_pre/60-qwen4exp-nvfp4-mixed.sh` · `artifacts/build_patch_pre/62-qwen4exp-ple-mmap.sh` · `artifacts/build_patch_pre/64-qwen4exp-qsa-fp8kv.sh` | 2-signal | build-ledger:attestation:output/multi/benchlog/attestation_ds4f0731-1m-spec7-roce.json#main | build_patches_src/ — 적용 판정 build-ledger: applied 3 · skipped 2 · 실린 것 = skip 아닌 패치(기계 파생) |
| `build_recipe` | 실림 | `artifacts/build_recipe/Dockerfile.source-build` · `artifacts/build_recipe/requirements.txt` | 3-signal | build-ledger:build-ledger.dockerfile | `Dockerfile.source-build`(NGC `26.07-py3` 베이스 · `VLLM_REF=v0.29.0rc6` · `TORCH_CUDA_ARCH=12.1a` · `RAY_VERSION=2.48.0` · `SM12X_PORT=0` · `SRC_DEPS_AUTHORITY=0`)와 그것이 COPY 하는 `requirements.txt`(flashinfer 0.6.18 양보 · W4). 소스빌드인 이유는 00 §0.4 (1). 1.1 은 Dockerfile 을 docker history 17/17 일치로, requirements.txt 를 빌드 원장 sha256 일치로 **쓰인 바이트**임을 확인했다. §L39 는 이 빌드 키(26.07 × 0.29.0rc6)를 미검증 attempt-build 로… |
| `compose` | 실림 | `artifacts/compose/.env.cluster.template` · `artifacts/compose/.env.interconnect.template` · `artifacts/compose/.env.template` · `artifacts/compose/docker-compose.yaml` · `artifacts/compose/serve_runner.sh` · `artifacts/compose/sub_recipe.json` | 2-signal | file:output/multi/docker-compose.yaml | `docker-compose.yaml`(측정 전 마지막 커밋 26b17f6e560a) · `serve_runner.sh`(50a5ce486738) · env 형상 3종 · `sub_recipe.json`. `serve_runner.sh` 는 멀티에서 `distributed-executor-backend` 가 없으면 fail-loud 로 멈추는 게이트를 갖고(W3 · §L51-65), GPU 합류 리터럴 `2`(§L79)가 노드 수와 숨은 결합이다(1.5). `arm_patch.sh` 는 런타임 패치 파일이 없어 no-op 이라 싣지 않았다. |
| `fork_pin` | 안 실림 | — | 1-signal | none:.claude/policies/arch_variant_ledger.json | 셀 env 에 VARIANT 줄 없음 = stock(workflow.md §변종 좌표의 거처)(기계 파생) |
| `runtime_patch` | 안 실림 | — | 1-signal | none:output/multi/configs/ds4f0731-1m-spec7-roce_patch.py | output/multi/configs/ds4f0731-1m-spec7-roce_patch.py 없음 — arm_patch.sh 는 no-op(런타임 패치 불해당)(기계 파생) |
| `triplet` | 실림 | `artifacts/triplet/.env.ds4f0731-1m-spec7-roce` · `artifacts/triplet/ds4f0731-1m-spec7-roce.sh` · `artifacts/triplet/ds4f0731-1m-spec7-roce.yaml` | 3-signal | file:output/multi/configs/ds4f0731-1m-spec7-roce.yaml | `ds4f0731-1m-spec7-roce.{yaml,sh}` · `.env.ds4f0731-1m-spec7-roce` — 09-09 1M 승자 셀 트리플렛의 재명명본(plan_26092808 §2 "값 변경 0")에 `enable-auto-tool-choice: true` 한 줄(yaml §L21)이 더해진 것이다. **이 판에서 그 줄은 측정 구성의 일부다** — 측정한 실행의 엔진이 non-default args 에 `enable_auto_tool_choice: True` 를 찍었고(engine_v7 L62), yaml 파일 mtime(2026-09-28T01:12:41Z · 파일시스템 관측)도 이 run 의 측정(2026-09-29T05:13:42Z~)보다 앞선다. ⚠ 그 줄의 **주석**("202… |

**레시피 경고 · 실린 리비전**(기계 관측 — 수신자 빌드가 여기서 갈라질 수 있다)

- `build_recipe` `output/multi/Dockerfile.source-build` 실린 리비전: 리비전 미특정 · 선택 방법 `worktree(측정 이미지 docker history 의 RUN·COPY 와 전수 일치)` · docker history 대조 17/17 · 실린 바이트 `worktree` · 쓰인 바이트임을 관측으로 확인 예
- `build_recipe` `output/multi/requirements.txt` 실린 리비전: 리비전 미특정 · 선택 방법 `worktree(빌드 원장 sha256 일치 — 쓰인 바이트 관측 확인 = 예)` · docker history 대조 없음 · 실린 바이트 `worktree` · 쓰인 바이트임을 관측으로 확인 예
- `compose` `.claude/skills/upstream-version-watch/assets/configs/serve_runner.sh` 실린 리비전: 커밋 `50a5ce486738` · 선택 방법 `worktree(mtime 2026-09-06T21:57:53Z ≤ 측정 2026-09-29T05:52:10Z(sweep_index.generated_utc) · 바이트 = 측정 전 마지막 커밋 50a5ce486738)` · docker history 대조 없음 · 실린 바이트 `worktree` · 근거 `mtime≤measured` · 쓰인 바이트임을 관측으로 확인 예
- `compose` `output/multi/docker-compose.yaml` 실린 리비전: 커밋 `26b17f6e560a` · 선택 방법 `worktree(mtime 2026-09-21T20:48:25Z ≤ 측정 2026-09-29T05:52:10Z(sweep_index.generated_utc) · 바이트 = 측정 전 마지막 커밋 26b17f6e560a)` · docker history 대조 없음 · 실린 바이트 `worktree` · 근거 `mtime≤measured` · 쓰인 바이트임을 관측으로 확인 예

**싣지 않은 파일**(슬롯별 사유 — 빌드 때 skip 된 패치는 아래 적용 집합 표에 있다)

- `build_patch_post` known-non-patch 1건: `.gitkeep`
- `build_patch_pre` known-non-patch 3건: `.gitkeep` · `PROVENANCE.json` · `files/`
- `compose` `arm_patch.sh` — `not-armed(output/multi/configs/ds4f0731-1m-spec7-roce_patch.py 없음 · slave CONFIG_FILE='default' 의 _patch.py 없음 — arm_patch.sh 는 no-op)`

**파일별 검증 · 관련성**(`generated-unverified` = 실행 검증되지 않은 생성물 — 파일 첫 줄 경고 · PAYLOAD 파일 기록과 같은 표시 · 관련성 = ① 패치 선언 대상 × 모델 config ② 엔진 로그 발화 서명 — 둘 다 관측일 때만 required/inactive-inferred)

| 슬롯 | 파일 | 검증 | 검증 근거 | 관련성 | 관련성 근거 |
|---|---|---|---|---|---|
| `build_patch_post` | `artifacts/build_patch_post/10-deepgemm.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_ds4f0731-1m-spec7-roce.json#main) · 원장 관측 | `unknown` | ① 선언 ['DeepseekV4ForCausalLM'] ∋ 셀 모델(architectures ['DeepseekV4ForCausalLM'] · model_type deepseek_v4) · 범용 선언 · ② 패치가 심는 logger 발화 서명 없음(발화 미관측) |
| `build_patch_post` | `artifacts/build_patch_post/20-triton-kernels.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_ds4f0731-1m-spec7-roce.json#main) · 원장 관측 | `unknown` | ① 선언 ['DeepseekV4ForCausalLM'] ∋ 셀 모델(architectures ['DeepseekV4ForCausalLM'] · model_type deepseek_v4) · 범용 선언 · ② 패치가 심는 logger 발화 서명 없음(발화 미관측) |
| `build_patch_post` | `artifacts/build_patch_post/30-mxfp4-triton-sm121.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_ds4f0731-1m-spec7-roce.json#main) · 원장 관측 | `unknown` | ① 패치 헤더·본문에 대상 선언(model-trigger 아키텍처 · vllm/models/<model_type>) 없음 · 편집하는 공유(모델 디렉터리 밖) 파일 ['vllm/model_executor/layers/fused_moe/experts/gpt_oss_triton_kernels_moe.py'] |
| `build_patch_post` | `artifacts/build_patch_post/40-humming-nvml-gb10.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_ds4f0731-1m-spec7-roce.json#main) · 원장 관측 | `unknown` | ① 패치 헤더·본문에 대상 선언(model-trigger 아키텍처 · vllm/models/<model_type>) 없음 |
| `build_patch_pre` | `artifacts/build_patch_pre/60-qwen4exp-nvfp4-mixed.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_ds4f0731-1m-spec7-roce.json#main) · 원장 관측 | `unknown` | ① 선언 ['models/qwen4_exp'] ∌ 셀 모델(architectures ['DeepseekV4ForCausalLM'] · model_type deepseek_v4) · ② 패치가 심는 logger 발화 서명 없음(발화 미관측) · 편집하는 공유(모델 디렉터리 밖) 파일 ['vllm/model_executor/layers/quantization/modelopt.py'] |
| `build_patch_pre` | `artifacts/build_patch_pre/62-qwen4exp-ple-mmap.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_ds4f0731-1m-spec7-roce.json#main) · 원장 관측 | `inactive-inferred` | ① 선언 ['models/qwen4_exp'] ∌ 셀 모델(architectures ['DeepseekV4ForCausalLM'] · model_type deepseek_v4) · ② 엔진 로그 17개에서 서명 무발화(서명 6개 전부 0회) — 적용됐으나 이 모델에서 돌지 않은 것으로 **추론**(확정 ✗) |
| `build_patch_pre` | `artifacts/build_patch_pre/64-qwen4exp-qsa-fp8kv.sh` | `verified` · cell-run | 적용 판정 build-ledger(attestation:output/multi/benchlog/attestation_ds4f0731-1m-spec7-roce.json#main) · 원장 관측 | `unknown` | ① 선언 ['models/qwen4_exp'] ∌ 셀 모델(architectures ['DeepseekV4ForCausalLM'] · model_type deepseek_v4) · ② 패치가 심는 logger 발화 서명 없음(발화 미관측) |
| `build_recipe` | `artifacts/build_recipe/Dockerfile.source-build` | `verified` · cell-run | 쓰인 레시피(build-ledger.dockerfile) — 개정 선택 slots.build_recipe.evidence.selected_revisions | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `build_recipe` | `artifacts/build_recipe/requirements.txt` | `verified` · cell-run | 쓰인 레시피(build-ledger.dockerfile) — 개정 선택 slots.build_recipe.evidence.selected_revisions | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/.env.cluster.template` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/.env.interconnect.template` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/.env.template` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/docker-compose.yaml` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/serve_runner.sh` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/sub_recipe.json` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/.env.ds4f0731-1m-spec7-roce` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/ds4f0731-1m-spec7-roce.sh` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/ds4f0731-1m-spec7-roce.yaml` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
<!-- /FACT:slots -->

<!-- FACT:applied_set -->
**적용 집합** — 상태 `observed` · 출처 `attestation:output/multi/benchlog/attestation_ds4f0731-1m-spec7-roce.json#main`

| 단계 | 파일 | 결과 | 결과 출처 | 사유 | 실림 | model-trigger |
|---|---|---|---|---|---|---|
| pre | `50-dsv4-sm12x-port.sh` | skipped | build-ledger | [50-dsv4-sm12x-port] skip — SM12X_PORT=0 (stock 빌드 경로 불변) | 안 실림 | DeepseekV4ForCausalLM(0731 정식판) + `--speculative-config method=dspark`. |
| pre | `55-src-deps-authority.sh` | skipped | build-ledger | [55-src-deps] skip — SRC_DEPS_AUTHORITY=0 (constraint 그대로 · stock 빌드 경로 불변) | 안 실림 | — |
| pre | `60-qwen4exp-nvfp4-mixed.sh` | applied | build-ledger | — | 실림 | — |
| pre | `62-qwen4exp-ple-mmap.sh` | applied | build-ledger | — | 실림 | — |
| pre | `64-qwen4exp-qsa-fp8kv.sh` | applied | build-ledger | — | 실림 | — |
| post | `10-deepgemm.sh` | applied | build-ledger | — | 실림 | DeepseekV4ForCausalLM FP8 block-scale dense/MoE (SM12x). 범용 arch-enablement(모델-키잉 ✗ — 이미지 네이밍 불변식). |
| post | `20-triton-kernels.sh` | applied | build-ledger | — | 실림 | DeepseekV4ForCausalLM (MXFP4 MoE 74GiB/node — MARLIN repack 미적합). 범용(MXFP4 MoE 의 TRITON 경로 필요 모델 공통). |
| post | `30-mxfp4-triton-sm121.sh` | applied | build-ledger | — | 실림 | — |
| post | `40-humming-nvml-gb10.sh` | applied | build-ledger | — | 실림 | — |
| inline | `strip-hoist` | skipped | build-ledger | register_opaque_type accepts hoist / opaque_object 부재 | 안 실림 | — |

**결과 출처 범례**

- `build-ledger` — 이미지 안 빌드 원장이 적용 결과를 기록했다(관측)

**판정 근거**(패치별 — artifacts 가 판정에 쓴 관측 · 스크립트 바이트 정체 = 실린 바이트가 측정 이미지에 구워진 바이트인가)

| 파일 | 스크립트 바이트 정체 | 이미지 사본 sha256 | 적용 표지(상태 찾음/선언) | 정적 규칙 | 원장 분류 |
|---|---|---|---|---|---|
| `50-dsv4-sm12x-port.sh` | build-ledger(원장 script_sha256 = 작업트리 바이트 — 실린 바이트 = 빌드가 실행한 바이트) | `784cd57f20a070b7dee8a3aa4c1cc516b513d46ea161d735b9d43eca81bc77bb`(빌드 원장 script_sha256 — 이미지 사본 아님) | — | — | log-token |
| `55-src-deps-authority.sh` | build-ledger(원장 script_sha256 = 작업트리 바이트 — 실린 바이트 = 빌드가 실행한 바이트) | `1f2aed56d94233b341157370ce4cebefd34e15979d0e90e5d5fed4d3ef4467a6`(빌드 원장 script_sha256 — 이미지 사본 아님) | — | — | log-token |
| `60-qwen4exp-nvfp4-mixed.sh` | build-ledger(원장 script_sha256 = 작업트리 바이트 — 실린 바이트 = 빌드가 실행한 바이트) | `a28f42b0b7a4a462c005c6ec74be9896a11131f628038061b96c3ad967070957`(빌드 원장 script_sha256 — 이미지 사본 아님) | — | — | exit-code |
| `62-qwen4exp-ple-mmap.sh` | build-ledger(원장 script_sha256 = 작업트리 바이트 — 실린 바이트 = 빌드가 실행한 바이트) | `5e779aec4ac7987d00ca128faa9eb6b6fcb6d78f50b16ea0c3cfee63809825d2`(빌드 원장 script_sha256 — 이미지 사본 아님) | — | — | exit-code |
| `64-qwen4exp-qsa-fp8kv.sh` | build-ledger(원장 script_sha256 = 작업트리 바이트 — 실린 바이트 = 빌드가 실행한 바이트) | `c5ed51bba3901475d4482a99e7123de46a2f7a2c7fffbf74a0a5cb83736896e3`(빌드 원장 script_sha256 — 이미지 사본 아님) | — | — | exit-code |
| `10-deepgemm.sh` | build-ledger(원장 script_sha256 = 작업트리 바이트 — 실린 바이트 = 빌드가 실행한 바이트) | `bdfd5412c3b7a58133006436bc2c9960a2433a67d92277a2b12b63d1364c7589`(빌드 원장 script_sha256 — 이미지 사본 아님) | — | — | exit-code |
| `20-triton-kernels.sh` | build-ledger(원장 script_sha256 = 작업트리 바이트 — 실린 바이트 = 빌드가 실행한 바이트) | `e263f424720d666e7915aa2d07d0c4878257ad04b45ade39fabdf67aa3ff2824`(빌드 원장 script_sha256 — 이미지 사본 아님) | — | — | exit-code |
| `30-mxfp4-triton-sm121.sh` | build-ledger(원장 script_sha256 = 작업트리 바이트 — 실린 바이트 = 빌드가 실행한 바이트) | `161dce749a830f55c6c1b43a643929794006b8e5bfa3d55dd6e99654e5958a39`(빌드 원장 script_sha256 — 이미지 사본 아님) | — | — | exit-code |
| `40-humming-nvml-gb10.sh` | build-ledger(원장 script_sha256 = 작업트리 바이트 — 실린 바이트 = 빌드가 실행한 바이트) | `a9f1a94390997c669c3e469debdaa28f0e581910f1f0ed3432dfd8798c4f5c2e`(빌드 원장 script_sha256 — 이미지 사본 아님) | — | — | exit-code |
| `strip-hoist` | — | — | — | — | status-file |

**원장 탐침**(순위: attestation → 메인 로컬 이미지 → 재구성)

| 순위 | 결과 | 대상 |
|---|---|---|
| attestation | observed | output/multi/benchlog/attestation_ds4f0731-1m-spec7-roce.json |

> **attestation 범위** — config `ds4f0731-1m-spec7-roce` · phase serve · 작성 2026-09-29T05:12:32Z(output/multi/benchlog/attestation_ds4f0731-1m-spec7-roce.json 파일 mtime(파일시스템 관측 — 문서에 시각 칸이 없다 · 기록 필드 아님)) · 묶은 근거 cell-name · 파일 `output/multi/benchlog/attestation_ds4f0731-1m-spec7-roce.json` · 측정 창 대비 작성 2026-09-29T05:12:32Z < 측정 시작 2026-09-29T05:13:42Z — 측정 전(빌드·스모크)의 관측: **same config(ds4f0731-1m-spec7-roce) · serve — 측정을 서빙한 같은 기동의 스모크 직후 관측(판별: 스모크 로그 docs/simlog/26092914_54_41_hint_v7_live_ds4f/serve.log L33 SMOKE PASS → L34 attestation 작성 → L37 --keep-up · 원장 docs/logs/main/events/2026-09.jsonl:1073 budget_declare(smoke-ds4f0731-1m-spec7-roce · 2026-09-29T04:54:44Z) 가 측정 끝 2026-09-29T05:52:10Z 까지 재선언 · 창 닫힘 없이 이어짐(main 원장 기록 범위 안) · 스모크 로그 budget_honored ts 2026-09-29T04:54:45Z = 그 선언의 원장 budget_honored · 측정 엔진 로그 APIServer pid 790 하나 · 측정 시작 2026-09-29T05:13:42Z)**
<!-- /FACT:applied_set -->

## 1.2 슬롯별 적용 사유
> 이 절이 답하는 질문: 실린 산출물 하나하나는 무엇을 고치며, 왜 이 셀에 필요했고, 이 셀에서 실제로 발화했으며, 언제 불필요해지는가?

1.1 의 적용 결과는 전부 이미지 안 빌드 원장(`build-ledger` · attestation 경유)이 기록한 **관측**이다. 그 attestation 파일은 2026-09-29T05:12:32Z(측정 시작 05:13:42Z 전)에 쓰였고 측정을 서빙한 같은 기동의 스모크 직후 관측이다(FACT 판별 — 스모크 로그 SMOKE PASS → attestation 작성 → `--keep-up` 순서 · 그 기동의 예산 선언 창이 측정 끝까지 닫힘 없이 이어짐 · 측정 엔진 APIServer pid 하나) — 1.1 attestation 범위 줄. 측정 실행의 엔진 기동 배너(engine_v7 L58)도 attestation 의 wheel 메타와 같은 빌드 문자열을 보인다.

**왜 Qwen 전용 패치 60 · 62 · 64 가 이 DeepSeek 이미지에 실렸나.** 한 이미지가 모든 모델을 서빙하고(이미지 이름에 모델 축이 없다), 빌드 레시피는 `build_patches_src/` 의 `*.sh` 를 **전부** 정렬 적용하며 패치마다의 게이트는 스크립트 자신에게 맡긴다:

> [원문] Dockerfile.source-build §L165-171
>     for p in $(ls /tmp/build_patches_src/*.sh 2>/dev/null | sort); do \
>       b=$(basename "$p"); echo "[build-patch-src] applying $p"; \
>       sha256sum "$p" | cut -d' ' -f1 > "$L/pre.$b.sha256"; rm -f "$L/pre.$b.status" "$L/pre.$b.rc"; \
>       { EASY_VLLM_PATCH_STATUS="$L/pre.$b.status" bash "$p"; echo "$?" > "$L/pre.$b.rc"; } 2>&1 | tee "$L/pre.$b.log"; \
>       rc=$(cat "$L/pre.$b.rc" 2>/dev/null || echo missing); \
>       tail -c 65536 "$L/pre.$b.log" > "$L/pre.$b.log.t" && mv "$L/pre.$b.log.t" "$L/pre.$b.log"; \
>       [ "$rc" = 0 ] || { echo "[build-patch-src] FAIL $b rc=$rc"; exit 1; }; \

50(`SM12X_PORT`)과 55(`SRC_DEPS_AUTHORITY`)는 build-arg 0 으로 스스로 skip 했고(1.1 적용 집합), 60 · 62 · 64 는 헤더가 `gate : 없음(ungated)` — "불활성 백포트" 로 선언해 **어느 모델의 빌드든** 적용된다. 셋은 다른 캠페인(camp-26090918 · qwen4_exp 셀)의 전제로 같은 통로에 들어왔고, 이 셀의 측정 이미지는 그 뒤(마지막 층 2026-09-22T23:38:10Z · 01 §1.4)에 지어졌다. 그래서 이 zip 은 이미지 바이트 재현을 위해 셋을 싣되, **이 모델의 요구로 읽으면 안 된다**. 관련성(FACT)은 60 `unknown` · 62 `inactive-inferred` · 64 `unknown` 이다 — 셋 다 ① 선언 대상 `models/qwen4_exp` 가 이 셀 모델(`DeepseekV4ForCausalLM`)이 아니고, ② 발화 서명이 판정을 가른다: 62 만 이 모델에서 돌면 찍힐 서명(6개)을 갖고 있어 엔진 로그 17개의 무발화(서명 6개 전부 0회)로 '적용됐으나 돌지 않음' 을 **추론**할 수 있고, 60 · 64 는 심는 logger 서명이 없어 돌지 않았다는 것을 관측할 수 없다(`unknown`). 60 은 공유 파일 `vllm/model_executor/layers/quantization/modelopt.py` 도 편집하므로(§L106-107) 모델 트리만으로 가를 수도 없다.

### build_patch_post/10-deepgemm.sh
(a) DeepGEMM 을 공식 `deepseek-ai/DeepGEMM` 의 `nv_dev` 개발 브랜치(SHA 핀 `a6b593d2826719dcf4892609af7b84ee23aaf32a`)로 클론 · 빌드해 vLLM 이 번들하는 deepgemm(헤더가 적은 기준은 vLLM 0.24.0 핀 891d57b)을 런타임 설치로 오버라이드한다 — SM120 FP8 GEMM 실행커널의 원천 · arch-게이트 소스패치 없음(헤더 §L5-15). 이식 출처는 PR 이 아니라 상류 개발 브랜치다. (b) 헤더의 model-trigger 가 `DeepseekV4ForCausalLM FP8 block-scale dense/MoE (SM12x)` 다 — 이 계보에서 이 패치를 빼 본 기록은 없다(W id 없음 · 공유 이미지의 arch-enablement). (c) 이 run 엔진은 dense 에 `Selected DeepGemmFp8BlockScaledMMKernel for Fp8LinearMethod`(engine_v7 L890) · `DeepGEMM E8M0 enabled on current platform.`(L889 · L1056)를 찍었다 — DeepGEMM 경로 사용은 관측이지만 그것이 이 패치의 nv_dev 빌드라는 것은 이미지 설치 사실에서의 추론이고, 패치 고유 서명이 없어 관련성은 FACT 대로 `unknown` 이다. (d) 폐기 조건: vLLM 자체 deepgemm 핀이 SM120 실행커널을 담게 되면 — 이 스크립트의 probe 는 오버라이드로 클론한 nv_dev 소스의 SM120 심볼만 보므로 그 조건을 감지하지 않는다(자동 트립와이어 없음). 심볼이 사라지면 빌드를 멈춘다:

> [원문] 10-deepgemm.sh §L51-53
> if missing:
>     sys.stderr.write("[10-deepgemm/nv_dev] FAIL: nv_dev SM120 심볼 미발견(브랜치/레이아웃 변경 의심):\n  " + "\n  ".join(missing) + "\n")
>     sys.exit(1)

### build_patch_post/20-triton-kernels.sh
(a) NGC 번들 standalone `triton_kernels`(`matmul_ogs` · `routing` 부재)를 지워 vLLM vendored 완전본이 alias 되게 한다(헤더 §L4-14). (b) 헤더가 적은 필요 이유는 옛 DeepSeek-V4 MXFP4 MoE 가 TRITON 을 못 타 MARLIN repack 으로 통합메모리 OOM · 호스트 다운을 낸 벽이다(W1 · 2026-06-28 측정). (c) 이 셀은 MoE 에 HUMMING 을 골랐다(`Using 'HUMMING' Mxfp4 MoE backend.` · engine_v7 L893) — TRITON MoE 경로를 쓰지 않았으므로 이 셀에서의 역할은 없다고 **추론**한다(고유 발화 서명 없음 · 관련성 `unknown`). 이 모델에서 TRITON MoE 를 명시하면 엔진이 거부한다는 기록도 있다(R3). (d) 폐기 조건: NGC 번들이 두 서브모듈을 갖추면. 번들이 없거나 이미 vendored 면 **skip** 하고(§L23-27), 제거 뒤 vendored alias 검증이 실패하면 assert 로 빌드를 멈춘다(§L31-39).

### build_patch_post/30-mxfp4-triton-sm121.sh
(a) OAI-Triton MXFP4 MoE 디바이스 게이트의 capability 상한을 `(11, 0)` → `(13, 0)` 으로 넓혀 SM121 을 넣는다(헤더 §L4-6). (b) 필요 이유는 20 과 같다(MARLIN 강제 → repack OOM · W1). (c) 이 셀은 humming 이라 TRITON MoE 게이트를 지나지 않는다 — 불활성으로 **추론**(서명 없음 · 관련성 `unknown`). (d) 상류가 SM12x 를 게이트에 넣으면 불필요하다. 스크립트는 이미 완화된 문자열이면 skip, 예상 문자열이 없으면 빌드를 멈춘다:

> [원문] 30-mxfp4-triton-sm121.sh §L27-34
> if NEW in s:
>     print("[30-mxfp4-triton-sm121] 이미 완화됨 — skip")
> elif OLD in s:
>     open(f, "w").write(s.replace(OLD, NEW, 1))
>     print("[30-mxfp4-triton-sm121] OK — 게이트 완화:", OLD, "→ < (13, 0)")
> else:
>     sys.stderr.write("[30-mxfp4-triton-sm121] FAIL: 예상 게이트 문자열 미발견 — 상류 변경 의심(false-determinism 차단)\n")
>     sys.exit(1)

### build_patch_post/40-humming-nvml-gb10.sh
(a) humming-kernels 튜닝 휴리스틱의 NVML 클록 쿼리 2개(`NVML_CLOCK_MEM` · `NVML_CLOCK_SM`)를 try/except 로 가드하고 GB10 추정값(273 GB/s · 1500 MHz)으로 폴백한다 — 자체 설계(헤더 §L4-21). (b) GB10 은 NVML 로 그 클록을 노출하지 않아 가드 없이는 humming MoE 가중치 적재 직후 EngineCore init 에서 크래시했다(W2 · 2026-06-28 측정). 이 셀의 `moe-backend` 가 humming 이므로(V9) **이 셀에 직접 걸리는 공유 패치**다. (c) 이 run 은 humming 튜닝 경로를 지났고(`Attempting to override humming GEMM config` · engine_v7 L1065 · L1130) 초기화가 크래시 없이 끝났다(`init engine … took 191.98 s` · L1344) — 가드가 폴백으로 먹었다는 직접 서명은 로그에 없어 관련성은 FACT 대로 `unknown` 이다(패치가 logger 서명을 심지 않는다). (d) humming-kernels 가 NotSupported 를 스스로 처리하면 불필요하다. 이미 가드됐으면 skip(§L53-55) · 예상 블록이 없으면 빌드 중단(§L57-60).

### build_patch_pre/60-qwen4exp-nvfp4-mixed.sh
(a) qwen4_exp 에서 ModelOpt MIXED_PRECISION(NVFP4) 체크포인트가 로드되게 하는 소스 이식 — hunk A 자체 제작 + B1/B2 는 미머지 상류 PR #55513 이식(헤더 §L4-24). (b) **이 모델의 요구가 아니다** — camp-26090918 의 nvfp4 셀 전제다(헤더 §L16-18). 이 셀의 벽 어느 것도 이 패치로 넘지 않았다. (c) 관련성 `unknown` — 헤더는 세 변경이 `ModelOptMixedPrecisionConfig` 로드 경로에서만 살아나며 FP8(native) 체크포인트 동작은 바이트 단위로 같다고 적고, 이 체크포인트의 `quant_method` 는 fp8 이다(00 §0.4 양자화 구성) — 그래서 불활성이라는 것은 **추론**이다. (d) 상류가 #55513 등가 수정을 머지하면 트립와이어가 `SystemExit(1)` 로 빌드를 멈춰 제거를 강제한다(§L58-60).

### build_patch_pre/62-qwen4exp-ple-mmap.sh
(a) qwen4_exp 의 PLE n-gram 테이블을 NVMe mmap 으로 서빙하는 경로 이식(`VLLM_PLE_MMAP=1` 일 때만 활성 · 참조 blazux 구현 + TP=2 rank-aware masking 자체 설계 · 헤더 §L4-28). (b) 이 모델에는 PLE 가 없다 — 요구가 아니다(1.6 스테이징 불해당). (c) 관련성 `inactive-inferred`(엔진 로그 17개에서 서명 6개 무발화 · 1.1) — 적용됐으나 이 모델에서 돌지 않은 것으로 **추론**(확정 ✗). (d) 이미 경로가 있으면(상류 머지 · 중복 적용) 빌드를 멈춘다:

> [원문] 62-qwen4exp-ple-mmap.sh §L47-51
> # 머지/중복 트립와이어
> if [ -f "$MOD" ] || grep -q 'qwen4_exp_ple_mmap_lookup' "$PLE"; then
>     echo "$TAG FAIL: ple_mmap 경로가 이미 존재한다 — 상류 머지 또는 이전 적용. 이 패치를 제거하고 stock 경로를 확인하라." >&2
>     exit 1
> fi

### build_patch_pre/64-qwen4exp-qsa-fp8kv.sh
(a) qwen4_exp QSA(Triton) 어텐션의 KV 에 fp8_e4m3 읽기 디콴트를 얹고 가드 7개를 넓힌다(참조 blazux · vLLM `_cast_kv_tile` 재사용 · 헤더 §L4-24). (b) 요구가 아니다 — 이 셀의 fp8 KV 는 QSA 가 아니라 DeepseekV4 어텐션의 `fp8_ds_mla` 경로다(engine_v7 L891). (c) 관련성 `unknown`(logger 서명 없음) — QSA 커널을 이 아키텍처가 로드하지 않으므로 불활성이라는 것은 **추론**이다. (d) 상류가 fp8 KV 를 받으면(§L44-48) 빌드를 멈춘다.

### build_recipe
`Dockerfile.source-build`(NGC `26.07-py3` 베이스 · `VLLM_REF=v0.29.0rc6` · `TORCH_CUDA_ARCH=12.1a` · `RAY_VERSION=2.48.0` · `SM12X_PORT=0` · `SRC_DEPS_AUTHORITY=0`)와 그것이 COPY 하는 `requirements.txt`(flashinfer 0.6.18 양보 · W4). 소스빌드인 이유는 00 §0.4 (1). 1.1 은 Dockerfile 을 docker history 17/17 일치로, requirements.txt 를 빌드 원장 sha256 일치로 **쓰인 바이트**임을 확인했다. §L39 는 이 빌드 키(26.07 × 0.29.0rc6)를 미검증 attempt-build 로 경고하고 중재를 스모크에 맡긴다 — 09-08 스모크가 그 중재를 통과했다(testlog_26090904 판정 요약).

### compose
`docker-compose.yaml`(측정 전 마지막 커밋 26b17f6e560a) · `serve_runner.sh`(50a5ce486738) · env 형상 3종 · `sub_recipe.json`. `serve_runner.sh` 는 멀티에서 `distributed-executor-backend` 가 없으면 fail-loud 로 멈추는 게이트를 갖고(W3 · §L51-65), GPU 합류 리터럴 `2`(§L79)가 노드 수와 숨은 결합이다(1.5). `arm_patch.sh` 는 런타임 패치 파일이 없어 no-op 이라 싣지 않았다.

### triplet
`ds4f0731-1m-spec7-roce.{yaml,sh}` · `.env.ds4f0731-1m-spec7-roce` — 09-09 1M 승자 셀 트리플렛의 재명명본(plan_26092808 §2 "값 변경 0")에 `enable-auto-tool-choice: true` 한 줄(yaml §L21)이 더해진 것이다. **이 판에서 그 줄은 측정 구성의 일부다** — 측정한 실행의 엔진이 non-default args 에 `enable_auto_tool_choice: True` 를 찍었고(engine_v7 L62), yaml 파일 mtime(2026-09-28T01:12:41Z · 파일시스템 관측)도 이 run 의 측정(2026-09-29T05:13:42Z~)보다 앞선다. ⚠ 그 줄의 **주석**("2026-09-28 … 벤치·hint 봉인 뒤 추가 · 벤치 레시피 대비 유일한 변경")은 09-28 판의 사정을 적은 것이라 이 판에서는 틀린 안내다 — 이 판의 측정은 그 줄을 넣은 채로 했다(W19 · V14). 3번째 줄 주석의 `max_model_len=786432` 도 768K 시절 값이다(2.6). 값의 지위는 1.3.

### 싣지 않은 슬롯
- `build_patch_pre/50-dsv4-sm12x-port.sh` — skip(`SM12X_PORT=0` · stock 경로) · 0.29.0rc6 stock 으로 서빙이 섰다(R9).
- `build_patch_pre/55-src-deps-authority.sh` — skip(`SRC_DEPS_AUTHORITY=0` · constraint 그대로).
- inline `strip-hoist` — skip(원장 사유 `register_opaque_type accepts hoist / opaque_object 부재`).
- `runtime_patch` — 없음(`ds4f0731-1m-spec7-roce_patch.py` 부재) · `fork_pin` — 없음(변종 원장 미등재 · stock).

## 1.3 값의 지위표
> 서빙 설정의 노브 **전부**와 기계가 모은 후보 지위다(lockset 출처 · yaml 주석 · 엔진 기본값). 최종 지위는 아래 `kind: value-status` 블록이 정한다 — 표의 노브마다 블록이 정확히 하나 있어야 린터를 통과한다.

<!-- FACT:value_status -->
서빙 노브 14개 — 노브마다 `kind: value-status` 블록이 **정확히 1개** 있어야 한다(노브·값은 이 표의 글자 그대로 · 값 칸의 `\|` 는 `|` 로 적는다).

| 노브 | 값 | 후보 지위 | 후보 근거 | yaml 주석 | 출처 |
|---|---|---|---|---|---|
| `tensor-parallel-size` | 2 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | manifest 권위(len(nodes)×gpus_per_node=2) — 생성본 누락분 명시 | serving-yaml |
| `distributed-executor-backend` | ray | inherited | 주석 단어 '필수': 멀티노드 TP 필수(serve_runner fail-loud 지적) — 없으면 mp 로 가서 World size>GPU(1) · yaml 주석만 있음 — 관측된 실패가 있으면 declared-requirement | 멀티노드 TP 필수(serve_runner fail-loud 지적) — 없으면 mp 로 가서 World size>GPU(1) | serving-yaml 줄 주석 |
| `gpu-memory-utilization` | 0.85 | declared-requirement | lockset 출처 gmu_source=target_gmu · lockset provenance=hand-authored | — | lockset.gmu_source=target_gmu |
| `max-model-len` | 1048576 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `quantization` | fp8 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `kv-cache-dtype` | fp8 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | 셀 B 축 = fp8 (fp8_ds_mla 로 강제 해소됨 — 유일 지원 경로) | serving-yaml |
| `kv-cache-memory-bytes` | 10737418240 | inherited | lockset 출처 kv_source=hand · lockset provenance=hand-authored | 수렴 클램프 = 10GiB (3차 수렴 · binding = 호스트 기동 밸리: 14GiB 시 밸리 ~8.2GiB 가 절대플로어 10240 하회 → memwatch 킬 2회 실측 | lockset.kv_source=hand |
| `max-num-seqs` | 1 | inherited | lockset 출처 batch_source=hand-lever · lockset provenance=hand-authored | 동접 = 잔여 KV 최대활용 · 1M×6069B=5.91GiB → 10GiB/5.91 = 1.73x → 1 (bytes/token 1M 재측정 후 재산정) | lockset.batch_source=hand-lever |
| `moe-backend` | humming | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | 그라욼ing: auto→MARLIN-repack OOM 전력(hint/0.25.1 · Band2 correction) · 재검증 대상 | serving-yaml |
| `enforce-eager` | false | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | E1: b768k 승자 계승(spec+graph) | serving-yaml |
| `speculative-config` | {"method":"dspark","num_speculative_tokens":7} | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | ≥ dspark_block_size 5 · L1/L4 검증 | serving-yaml |
| `reasoning-parser` | deepseek_v4 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | hermes 용처 · 0.29.0rc6 등록명 확인(vllm/reasoning/__init__.py) | serving-yaml |
| `tool-call-parser` | deepseek_v4 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | 동상(vllm/parser 등록명 확인) | serving-yaml |
| `enable-auto-tool-choice` | true | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | 2026-09-28 Hermes 용처(사용자 결정) — 벤치·hint 봉인 뒤 추가 · 벤치 레시피 대비 유일한 변경(API 층) | serving-yaml |
<!-- /FACT:value_status -->

> 이 절이 답하는 질문: 각 서빙 노브의 값은 이 셀에서 어떤 지위인가 — 조정됐나, 승계됐나, 음성대조로 일부러 둔 것인가, 엔진 기본값인가, 필요조건인가?

```hint-event
id: V1
kind: value-status
노브: tensor-parallel-size
값: 2
지위: inherited
근거: manifest 파생(노드 2 × GPU 1) · 대조군 트리플렛 값 그대로 · 이 계보에서 다른 값을 시도한 기록 없음(누락 시 실패는 executor backend 쪽 — W3)
출처: [ds4f0731-1m-spec7-roce.yaml, plan_26092808_커널7_0_DS4F0731_멀티캠페인_RoCE재현_1M서빙_풀벤치_hint §2.]
```

```hint-event
id: V2
kind: value-status
노브: distributed-executor-backend
값: ray
지위: declared-requirement
근거: W3 — 없으면 vLLM 이 multiprocessing 으로 가 World size > GPU(1) 로 죽는다(serve_runner.sh 헤더 2026-09-04 실측 · 09-08 스모크 rc=2 · 러너가 fail-loud 로 막는다)
출처: [serve_runner.sh, testlog_26090904_ds4f0731_029rc6_attempt_build_스모크_판정 §2.]
```

```hint-event
id: V3
kind: value-status
노브: gpu-memory-utilization
값: 0.85
지위: inherited
근거: 캠페인 셀 config target_gmu 선언값을 대조군부터 그대로 승계 · 이 계보에서 바꿔 잰 기록 없음 · 뺐을 때의 실패 관측 없음(후보표의 declared-requirement 는 lockset 출처 표지에서 나온 것)
출처: [plan_26092808_커널7_0_DS4F0731_멀티캠페인_RoCE재현_1M서빙_풀벤치_hint §2., ds4f0731-1m-spec7-roce.yaml]
```

```hint-event
id: V4
kind: value-status
노브: max-model-len
값: 1048576
지위: inherited
근거: 캠페인 셀 축 1M · 모델 config max_position_embeddings 1048576 과 같다 · 768K 는 다른 셀이지 이 셀의 조정이 아니다 · 이 셀에서 조정 0
출처: [plan_26090819_ds4f0731_멀티TP2_KV3군_1M768K_광의탐색_Hermes §2., ds4f0731-1m-spec7-roce.yaml]
```

```hint-event
id: V5
kind: value-status
노브: quantization
값: fp8
지위: inherited
근거: 체크포인트 quantization_config(fp8 블록)와 같은 값을 명시한 승계 · 다른 값 시도 기록 없음(expert 는 fp4 · 00 §0.4 양자화 구성)
출처: [ds4f0731-1m-spec7-roce.yaml]
```

```hint-event
id: V6
kind: value-status
노브: kv-cache-dtype
값: fp8
지위: declared-requirement
근거: W8 — auto 는 엔진 assert 로 거부 · turboquant 도 같은 경로로 거부 · DeepseekV4 어텐션이 fp8_ds_mla 레이아웃을 강제해 fp8 이 유일 지원 경로
출처: [testlog_26090904_ds4f0731_029rc6_attempt_build_스모크_판정 §3., engine_v7_ds4f0731-1m-spec7-roce.log]
```

```hint-event
id: V7
kind: value-status
노브: kv-cache-memory-bytes
값: 10737418240
지위: tuned
근거: W9 · W10 — 768K 셀에서 18 GiB(벤치 상주 누락 사살) → 14 GiB(기동 밸리 사살 2회) → 10 GiB(같은 밸리 생존) 3차 수렴 · 1M 셀이 그 10 GiB 를 승계 · 1M 요청 하나 대비 엔진 보고 1.85x
출처: [ds4f0731-1m-spec7-roce.yaml, devlog_26090912_ds4f0731_광의탐색_셀루프_서사 §서사, engine_v7_ds4f0731-1m-spec7-roce.log]
```

```hint-event
id: V8
kind: value-status
노브: max-num-seqs
값: 1
지위: inherited
근거: 손레버 — yaml 주석이 10GiB ÷ 1M 요청 필요량 = 1.73x 를 내림해 1 · devlog 가 1M 동접 2 는 밸리 + 벤치 상주로 플로어 미달이라 산출(R6) · 1M 에서 2 를 띄워 잰 스윕 기록 없음
출처: [ds4f0731-1m-spec7-roce.yaml, devlog_26090912_ds4f0731_광의탐색_셀루프_서사 §재개 지침]
```

```hint-event
id: V9
kind: value-status
노브: moe-backend
값: humming
지위: inherited
근거: 대조군 레시피 승계 · 이 모델 · 0.29.0rc6 에서 triton 은 엔진 거부(R3 · 768K 셀) · auto 는 시도 0(yaml 주석 '재검증 대상') — auto 의 MARLIN repack OOM(W1)은 옛 체크포인트 · 옛 vLLM 의 실패 · 빼면(auto) 어떻게 되는지는 미관측 · humming 이면 GB10 NVML 가드(W2 · 40 패치)가 짝
출처: [20-triton-kernels.sh, 40-humming-nvml-gb10.sh, sweep_map_26090912_b768k_levers §셀]
```

```hint-event
id: V10
kind: value-status
노브: enforce-eager
값: false
지위: tuned
근거: 768K 에서 graph 셀(26.14)을 spec 셀(29.72) · 결합 셀(31.12)과 같은 클램프 · seqs 로 쟀다(기준 셀 대비 +52.8% 는 클램프 14→10 GiB · seqs 3→2 가 함께 바뀐 비교) · 1M 은 eager · spec off 기준 셀 대 spec + graph 셀로 함께 바꿔 쟀다(16.67 → 30.54) · 1M 에서 graph 단독 기여는 미분해 · 이 셀에서 조정 0
출처: [testlog_26090912_ds4f0731_029rc6_광의탐색_종합판정 §스코어보드, sweep_map_26090912_e1m_levers §셀, ds4f0731-1m-spec7-roce.yaml]
```

```hint-event
id: V11
kind: value-status
노브: speculative-config
값: '{"method":"dspark","num_speculative_tokens":7}'
지위: tuned
근거: 768K 에서 spec 셀(29.72)을 graph · 결합 셀과 같은 클램프 · seqs 로 쟀다(기준 대비 +73.7% 는 클램프 · seqs 도 함께 바뀐 비교) · 1M 에서 graph 와 함께 바꿔 쟀다 · k=7 자체는 스윕 0(dspark_block_size 5 이상 조건) · 판정에 쓴 accept_len 2.015789473684211 은 lite warm 레그(vllm bench serve · 3요청) 실측을 판정 레벨로 기계 승계한 값
출처: [testlog_26090912_ds4f0731_029rc6_광의탐색_종합판정 §스코어보드, bench_report_26092914_deepseek-v4-flash-0731_GB10_0.29.0 §루프라인 컨텍스트]
```

```hint-event
id: V12
kind: value-status
노브: reasoning-parser
값: deepseek_v4
지위: inherited
근거: Hermes 용처로 대조군 레시피가 등록명을 확인해 넣은 값의 승계 · 다른 값 시도 0 · 09-28 상주 chat 에서 reasoning 분리 관측 · 이 run 스모크 reasoning_len 95
출처: [ds4f0731-1m-spec7-roce.yaml, testlog_26092808_커널7_0_DS4F0731_RoCE_1M서빙_회귀판정 §5.]
```

```hint-event
id: V13
kind: value-status
노브: tool-call-parser
값: deepseek_v4
지위: inherited
근거: 대조군 레시피 승계 · 다른 값 시도 0 · 단독으로는 tool_choice auto 를 받지 못한다(W19 — enable-auto-tool-choice 가 함께 있어야 한다)
출처: [ds4f0731-1m-spec7-roce.yaml, testlog_26092808_커널7_0_DS4F0731_RoCE_1M서빙_회귀판정 §5.]
```

```hint-event
id: V14
kind: value-status
노브: enable-auto-tool-choice
값: true
지위: declared-requirement
근거: W19 — tool call 용처의 필요조건(없으면 tool_choice auto 가 HTTP 400 · 09-28 관측) · 이 판에서는 측정 구성의 일부(engine_v7 L62 non-default args) — 성능 수치는 이 줄을 넣은 채 잰 것
출처: [testlog_26092808_커널7_0_DS4F0731_RoCE_1M서빙_회귀판정 §5., engine_v7_ds4f0731-1m-spec7-roce.log]
```

**복사하면 위험한 값.** `kv-cache-memory-bytes`(V7)는 KV 필요량이 아니라 **호스트 기동 밸리**가 정한 값이다 — 노드 메모리 · 가중치 적재 경로가 다르면 다시 잰다. `max-num-seqs` 1(V8)은 산출로 정한 손레버라 최적이 아니며 동시성 2 이상의 요청당 decode 를 나눈다(03 §3.2). `gpu-memory-utilization` 0.85 · `tensor-parallel-size` 2(V3 · V1)는 바꿔 잰 적이 없다. 반대로 `kv-cache-dtype` fp8 · `distributed-executor-backend` ray(V6 · V2)는 빼거나 바꾸면 실패가 기록된 값이고, `enable-auto-tool-choice`(V14)는 tool call 용처에서 빼면 HTTP 400 이 기록된 값이다 — 이 판의 성능 수치는 그 줄을 **넣은** 구성의 것이다. `moe-backend` humming(V9)은 triton 이 거부된 기록은 있으나 auto 의 결과는 이 모델에서 관측되지 않았다.

## 1.4 재현 절차
> 셀 실행 기록에서 기계가 파생한 명령 순서와 단계별 소요다. 소요가 `미관측` 인 단계는 시각 기록이 없다는 뜻이다(0 이 아니다). 그 아래는 블랙박스 이벤트 원장에서 이 셀의 행을 기계가 뽑아 기동 시도로 묶은 것이고, 마지막은 측정한 실행의 엔진 로그가 echo 한 env 다(측정 당시 값의 정본).

<!-- FACT:reproduce -->
| 순서 | 단계 | 성공 판정 | 소요 | 관측 구간(UTC) | 소요 출처 | 명령 출처 |
|---|---|---|---|---|---|---|
| 1 | render | 러너 3종·.env·.env.cluster·.env.interconnect 생성(렌더러 fail-loud) | 미관측 | 미관측 | .claude/skills/upstream-version-watch/scripts/multinode_serve_smoke.sh 선행조건 안내 · .claude/skills/upstream-version-watch/scripts/render_dockerfile.py CLI | derived(렌더러 CLI) |
| 2 | build | 빌드 OK · 이미지 실재 | 층 창 347시간 46분 14초(docker history 첫 층 → 끝 층 CreatedAt · 캐시 재사용 층 포함 — 빌드 소요 아님) | 2026-09-08T11:51:56Z → 2026-09-22T23:38:10Z (layer-window) | docker history --no-trunc --format '{{json .}}' sha256:a2c4ca499fb2473191bcdfc7e192445750b4a7b6803cfe14e50b43f401000544 · 우리 Dockerfile 층 30개의 CreatedAt 첫 층 → 끝 층(캐시 재사용 층은 이전 빌드 시각 — 빌드 소요 아님) | reconstructed(build-ledger build_args + compose build) |
| 3 | serve | health 200 + 추론 1회(스모크 판정) | ≤ 18분 58초(상한) | 2026-09-29T04:54:44Z → 2026-09-29T05:13:42Z (upper) | docs/logs/main/events/2026-09.jsonl budget_declare(label smoke-ds4f0731-1m-spec7-roce) → 첫 측정 date(output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce/lite_cold_ds4f0731-1m-spec7-roce.json · date=컨테이너 시계(UTC 가정)) | derived(스모크 사용법) |
| 4 | bench | 리포트 발행(+PASS 면 인증서) | 38분 28초 | 2026-09-29T05:13:42Z → 2026-09-29T05:52:10Z (exact) | output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce/lite_cold_ds4f0731-1m-spec7-roce.json · date=컨테이너 시계(UTC 가정) date → sweep_index.generated_utc | reconstructed(bench json + 측정 도구 원문 lite 인자) |

**빌드 층 시각**(관측 · 소요가 아니라 층 창 — 캐시 재사용 층은 이전 빌드의 시각을 가진다)

- `build` 이미지 층 CreatedAt(docker history): 2026-09-08T11:51:56Z → 2026-09-22T23:38:10Z · 층 30개 · 출처 `docker history --no-trunc --format '{{json .}}' sha256:a2c4ca499fb2473191bcdfc7e192445750b4a7b6803cfe14e50b43f401000544` — 우리 Dockerfile 층의 CreatedAt 범위 — 캐시 재사용 층은 이전 빌드 시각을 가진다(소요 아님)

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
  --build-arg BUILD_JOBS=8 \
  --build-arg RAY_VERSION=2.48.0 \
  --build-arg SM12X_PORT=0 \
  --build-arg SRC_DEPS_AUTHORITY=0 \
  --build-arg TORCH_CUDA_ARCH=12.1a \
  --build-arg VLLM_PRETEND_VERSION= \
  --build-arg VLLM_REF=v0.29.0rc6 \
  --build-arg VLLM_REPO=https://github.com/vllm-project/vllm.git \
  -t easy-vllm:0.29.0rc6-cu133-aarch64-source \
  output/multi
# 동등 경로(스모크 사용법): bash .claude/skills/upstream-version-watch/scripts/multinode_serve_smoke.sh ds4f0731-1m-spec7-roce --build-only
```

**3. serve**

```sh
bash .claude/skills/upstream-version-watch/scripts/multinode_serve_smoke.sh ds4f0731-1m-spec7-roce --keep-up
```

**4. bench**

```sh
# bench JSON 필드로 복원: backend · model_id→--model · tokenizer_id→--tokenizer · num_prompts · max_concurrency · request_rate · burstiness · 요청당 입출력 길이 = total_*_tokens ÷ completed(나누어떨어질 때만 — 평균이다 · 요청별 길이는 JSON 에 없다) · 나누어떨어지면 `--dataset-name random` 으로 추정했다(측정 도구가 그 꼴로 부른다 — 아래 도구 원문).
# JSON 에 없는 인자(--endpoint · --base-url · --ignore-eos · --num-warmups · --temperature · --seed · --random-range-ratio · --trust-remote-code)는 측정 도구가 정했다 — level_* = run_bench.sh · lite_cold/lite_warm = lite_bench.sh(.claude/skills/adversarial-benchmark/scripts/) · 셀 스윕 순서는 sweep_bench.sh.
# lite_cold/lite_warm 의 데이터셋 · 부하 인자 = 측정 시점 도구 원문 lite_bench.sh@fcb0754fffe1 L136-L142(리터럴 인자만 · `$변수` 인자는 도구가 값을 정하지 않아 JSON 필드로 채운다) — `total_*_tokens ÷ completed` 는 실측 토큰(chat 템플릿 포함)이지 인자가 아니다.
# output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce/lite_cold_ds4f0731-1m-spec7-roce.json (date 20260929-051342) · 실측 토큰(chat 템플릿 포함 · total ÷ completed · 인자 아님): 입력 595/1 = 595 · 출력 128/1 = 128 · spec_decode_acceptance_length 2.01587(이 레그 JSON)
vllm bench serve --backend openai-chat --model deepseek-v4-flash-0731 --tokenizer /app/models/DeepSeek/DeepSeek-V4/DeepSeek-V4-Flash-0731 --trust-remote-code --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 --ignore-eos --num-prompts 1 --max-concurrency 1 --request-rate inf
# output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce/lite_warm_ds4f0731-1m-spec7-roce.json (date 20260929-051411) · 실측 토큰(chat 템플릿 포함 · total ÷ completed · 인자 아님): 입력 1785/3 = 595 · 출력 384/3 = 128 · spec_decode_acceptance_length 2.01579(이 레그 JSON)
vllm bench serve --backend openai-chat --model deepseek-v4-flash-0731 --tokenizer /app/models/DeepSeek/DeepSeek-V4/DeepSeek-V4-Flash-0731 --trust-remote-code --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 --ignore-eos --num-prompts 3 --max-concurrency 1 --request-rate inf
```

**슬롯 → 빌드 컨텍스트 매핑**(빌드 컨텍스트 = `output/<토폴로지>/` · 원천 = 실린 Dockerfile 의 COPY · compose 의 env_file · volumes · command — `—` 는 빌드 컨텍스트 입력이 아니다)

| zip 경로 | 빌드 컨텍스트 자리 | 근거 |
|---|---|---|
| `artifacts/triplet/.env.ds4f0731-1m-spec7-roce` | `envs/.env.ds4f0731-1m-spec7-roce` | compose env_file envs/.env.${CONFIG_FILE}(셀 env) |
| `artifacts/triplet/ds4f0731-1m-spec7-roce.sh` | `configs/ds4f0731-1m-spec7-roce.sh` | compose volumes ./configs:/app/configs |
| `artifacts/triplet/ds4f0731-1m-spec7-roce.yaml` | `configs/ds4f0731-1m-spec7-roce.yaml` | compose volumes ./configs:/app/configs |
| `artifacts/build_patch_pre/60-qwen4exp-nvfp4-mixed.sh` | `build_patches_src/60-qwen4exp-nvfp4-mixed.sh` | Dockerfile.source-build COPY build_patches_src/ |
| `artifacts/build_patch_pre/62-qwen4exp-ple-mmap.sh` | `build_patches_src/62-qwen4exp-ple-mmap.sh` | Dockerfile.source-build COPY build_patches_src/ |
| `artifacts/build_patch_pre/64-qwen4exp-qsa-fp8kv.sh` | `build_patches_src/64-qwen4exp-qsa-fp8kv.sh` | Dockerfile.source-build COPY build_patches_src/ |
| `artifacts/build_patch_post/10-deepgemm.sh` | `build_patches/10-deepgemm.sh` | Dockerfile.source-build COPY build_patches/ |
| `artifacts/build_patch_post/20-triton-kernels.sh` | `build_patches/20-triton-kernels.sh` | Dockerfile.source-build COPY build_patches/ |
| `artifacts/build_patch_post/30-mxfp4-triton-sm121.sh` | `build_patches/30-mxfp4-triton-sm121.sh` | Dockerfile.source-build COPY build_patches/ |
| `artifacts/build_patch_post/40-humming-nvml-gb10.sh` | `build_patches/40-humming-nvml-gb10.sh` | Dockerfile.source-build COPY build_patches/ |
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

**기동 성공 판정(관측 · 발행 자격)**: health 200 = true · 추론 1회 = true · 근거 `output/multi/benchlog/serve_proof_ds4f0731-1m-spec7-roce.json` — 시점 범위: 작성 2026-09-29T05:12:32Z(output/multi/benchlog/serve_proof_ds4f0731-1m-spec7-roce.json 파일 mtime(파일시스템 관측 — 문서에 시각 칸이 없다 · 기록 필드 아님)) · 측정 창 대비 작성 2026-09-29T05:12:32Z < 측정 시작 2026-09-29T05:13:42Z — 측정 전(빌드·스모크)의 관측
<!-- /FACT:reproduce -->

<!-- FACT:event_timeline -->
**블랙박스 이벤트 108행**(원장에서 이 셀 label·config 에 맞는 행만 · 시각순 · 기계 발췌 · 이번 캠페인 30행 · 같은 셀 이름 · 앞선 캠페인 78행(내용 칸에 표기))

> **캠페인 경계** 2026-09-29T04:54:44Z(`camp-26092913-hint-v7-e2e` · evidence.event_campaign_boundary): campaigns/camp-26092913-hint-v7-e2e/campaign.yaml declared_utc 2026-09-29T04:55:00Z · 이 발행이 묶인 측정을 서빙한 budget_declare 창 시작 2026-09-29T04:54:44Z → 경계 = 앞선 값 2026-09-29T04:54:44Z(이전 행 = 같은 셀 이름 · 앞선 캠페인)

> **원장 관측 범위**(노드별 첫 행 → 마지막 행 · evidence.event_ledger_spans): main = 2026-07-31T03:27:43Z → 2026-09-29T05:54:04Z(`docs/logs/main/events/2026-07.jsonl` · `docs/logs/main/events/2026-08.jsonl` · `docs/logs/main/events/2026-09.jsonl` · `docs/logs/main/events/thermal.jsonl` · `docs/logs/main/events/watchdog.jsonl`) · sub = 2026-09-03T09:51:45Z → 2026-09-11T11:10:38Z(`docs/logs/sub/events/2026-09.jsonl`) — ⚠ sub(마지막 행 2026-09-11T11:10:38Z < 측정 끝 2026-09-29T05:52:10Z): 그 뒤 그 노드의 선언 · 사건은 **관측 범위 밖**이다(없음이 아니다 · 원장 미회수) — 단 아래 보조 관측의 행은 예외
>
> **보조 관측 · sub**(sub 원장 미러 밖 — 스모크 로그 echo · 같은 기동 판별(attestation_same_boot 강한 판별)이 지목한 스모크 로그 docs/simlog/26092914_54_41_hint_v7_live_ds4f/serve.log 의 `sub:` 원장 JSON echo 줄): `budget_honored` 2026-09-29T04:54:48Z(`smoke-ds4f0731-1m-spec7-roce`) — `docs/simlog/26092914_54_41_hint_v7_live_ds4f/serve.log:26` — 이 기동에서 sub 에 대해 관측된 것은 `budget_honored` 뿐이다 · 그 밖의 sub 사건(사살 · 트립 · 갱신 · clear)은 여전히 **관측 범위 밖**이다(없음이 아니다)

| # | UTC | 노드 | kind | label | 내용 | 출처 |
|---|---|---|---|---|---|---|
| 1 | 2026-09-27T23:32:54Z | main | `budget_declare` | `smoke-ds4f0731-1m-spec7-roce` | 예산 선언(선언값 — 측정 아님) · floor_mib=21479 · weights_mib=79577 · kv_mib=10240 · overhead_mib=13312 · mem_total_mib=124608 · 기동 창 1/4: budget_renew 0회 · 닫힘 budget_clear 2026-09-27T23:34:58Z(선언 후 2m04s) · measured_utc 2026-09-29T05:52:10Z 는 이 창 밖(다른 기동 · 측정 전) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1000 |
| 2 | 2026-09-27T23:32:54Z | main | `budget_honored` | `smoke-ds4f0731-1m-spec7-roce` | budget_honored · floor_mib=21479 · arm_ceiling_mib=18407 · remaining_s=7200 · 창 시작 2026-09-27T23:32:54Z 후 0m00s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:515 |
| 3 | 2026-09-27T23:34:09Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=69032 · rate_mib_s=22969 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → smoke-ds4f0731-1m-spec7-roce 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-27T23:32:54Z 후 1m15s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:516 |
| 4 | 2026-09-27T23:34:58Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-ds4f0731-1m-spec7-roce 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-27T23:32:54Z 후 2m04s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1001 |
| 5 | 2026-09-27T23:34:59Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-ds4f0731-1m-spec7-roce 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-27T23:32:54Z 후 2m05s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:517 |
| 6 | 2026-09-27T23:37:13Z | main | `budget_declare` | `smoke-ds4f0731-1m-spec7-roce` | 예산 선언(선언값 — 측정 아님) · floor_mib=21479 · weights_mib=79577 · kv_mib=10240 · overhead_mib=13312 · mem_total_mib=124608 · 기동 창 2/4: budget_renew 18회 · 닫힘 budget_clear 2026-09-28T01:13:03Z(선언 후 1h35m50s) · measured_utc 2026-09-29T05:52:10Z 는 이 창 밖(다른 기동 · 측정 전) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1003 |
| 7 | 2026-09-27T23:37:14Z | main | `budget_honored` | `smoke-ds4f0731-1m-spec7-roce` | budget_honored · floor_mib=21479 · arm_ceiling_mib=18407 · remaining_s=7199 · 창 시작 2026-09-27T23:37:14Z 후 0m00s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:518 |
| 8 | 2026-09-27T23:38:34Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=80175 · rate_mib_s=22898 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → smoke-ds4f0731-1m-spec7-roce 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-27T23:37:14Z 후 1m20s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:519 |
| 9 | 2026-09-27T23:54:55Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=6138 · 창 시작 2026-09-27T23:37:13Z 후 17m42s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1004 |
| 10 | 2026-09-27T23:57:34Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=7041 · 창 시작 2026-09-27T23:37:13Z 후 20m21s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1005 |
| 11 | 2026-09-28T00:00:21Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=7033 · 창 시작 2026-09-27T23:37:13Z 후 23m08s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1006 |
| 12 | 2026-09-28T00:02:52Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=7049 · 창 시작 2026-09-27T23:37:13Z 후 25m39s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1007 |
| 13 | 2026-09-28T00:05:22Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=7050 · 창 시작 2026-09-27T23:37:13Z 후 28m09s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1008 |
| 14 | 2026-09-28T00:07:53Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=7049 · 창 시작 2026-09-27T23:37:13Z 후 30m40s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1009 |
| 15 | 2026-09-28T00:10:23Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=7050 · 창 시작 2026-09-27T23:37:13Z 후 33m10s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1010 |
| 16 | 2026-09-28T00:12:54Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=7049 · 창 시작 2026-09-27T23:37:13Z 후 35m41s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1011 |
| 17 | 2026-09-28T00:15:22Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=7052 · 창 시작 2026-09-27T23:37:13Z 후 38m09s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1012 |
| 18 | 2026-09-28T00:17:54Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=7048 · 창 시작 2026-09-27T23:37:13Z 후 40m41s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1013 |
| 19 | 2026-09-28T00:20:25Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=7049 · 창 시작 2026-09-27T23:37:13Z 후 43m12s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1014 |
| 20 | 2026-09-28T00:22:51Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=7054 · 창 시작 2026-09-27T23:37:13Z 후 45m38s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1015 |
| 21 | 2026-09-28T00:24:55Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=7076 · 창 시작 2026-09-27T23:37:13Z 후 47m42s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1016 |
| 22 | 2026-09-28T00:25:19Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=7176 · 창 시작 2026-09-27T23:37:13Z 후 48m06s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1017 |
| 23 | 2026-09-28T00:27:47Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=7052 · 창 시작 2026-09-27T23:37:13Z 후 50m34s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1018 |
| 24 | 2026-09-28T00:30:18Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=7049 · 창 시작 2026-09-27T23:37:13Z 후 53m05s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1019 |
| 25 | 2026-09-28T00:32:43Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=7055 · 창 시작 2026-09-27T23:37:13Z 후 55m30s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1020 |
| 26 | 2026-09-28T00:35:13Z | 미관측 | `measurement` | `measurement (budget window recorded)` | 같은 셀의 다른 측정(측정 노드 축 cluster) · 시각 = 측정 조립(sweep generated_utc = 인증서 measured_utc · 시작 시각 미기록) · 이 발행의 측정 2026-09-29T05:52:10Z 전 · 발행 기록 camp_26092808_ds4f_k70_roce__cluster__ds4f0731_1m_spec7_roce(셀 대조 simlog:sweep_index.meta.config_name · verdict PASS) · simlog 사본 generated_utc 2026-09-28T00:35:13Z(같음) · 원장: 이 셀 선언 창 안: smoke-ds4f0731-1m-spec7-roce 2026-09-27T23:37:13Z~2026-09-28T01:13:03Z (docs/logs/main/events/2026-09.jsonl:1003) · smoke-ds4f0731-1m-spec7-roce 2026-09-27T23:37:14Z~2026-09-28T01:13:04Z (docs/logs/main/events/watchdog.jsonl:518) · 이 시각을 관측하지 않은 원장: sub(기록 2026-09-03T09:51:45Z~2026-09-11T11:10:38Z) · 직전 예산 행 budget_renew(smoke-ds4f0731-1m-spec7-roce) 2026-09-28T00:32:43Z(docs/logs/main/events/2026-09.jsonl:1020) · 직후 예산 행 budget_renew(smoke-ds4f0731-1m-spec7-roce) 2026-09-28T00:54:55Z(docs/logs/main/events/2026-09.jsonl:1021) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/_evidence/camp_26092808_ds4f_k70_roce__cluster__ds4f0731_1m_spec7_roce.json:4 |
| 27 | 2026-09-28T00:54:55Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=5868 · 창 시작 2026-09-27T23:37:13Z 후 1h17m42s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1021 |
| 28 | 2026-09-28T01:13:03Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-ds4f0731-1m-spec7-roce 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-27T23:37:13Z 후 1h35m50s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1022 |
| 29 | 2026-09-28T01:13:04Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-ds4f0731-1m-spec7-roce 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-27T23:37:14Z 후 1h35m50s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:520 |
| 30 | 2026-09-28T01:13:32Z | main | `budget_declare` | `smoke-ds4f0731-1m-spec7-roce` | 예산 선언(선언값 — 측정 아님) · floor_mib=21479 · weights_mib=79577 · kv_mib=10240 · overhead_mib=13312 · mem_total_mib=124608 · 기동 창 3/4: budget_renew 44회 · 닫힘 budget_clear 2026-09-28T23:23:40Z(선언 후 22h10m08s) · measured_utc 2026-09-29T05:52:10Z 는 이 창 밖(다른 기동 · 측정 전) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1024 |
| 31 | 2026-09-28T01:13:32Z | main | `budget_honored` | `smoke-ds4f0731-1m-spec7-roce` | budget_honored · floor_mib=21479 · arm_ceiling_mib=18407 · remaining_s=7200 · 창 시작 2026-09-28T01:13:32Z 후 0m00s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:521 |
| 32 | 2026-09-28T01:14:52Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=80635 · rate_mib_s=21881 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → smoke-ds4f0731-1m-spec7-roce 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-28T01:13:32Z 후 1m20s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:522 |
| 33 | 2026-09-28T01:31:48Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=6104 · 창 시작 2026-09-28T01:13:32Z 후 18m16s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1025 |
| 34 | 2026-09-28T02:01:48Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-09-28T01:13:32Z 후 48m16s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1026 |
| 35 | 2026-09-28T02:31:49Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=5399 · 창 시작 2026-09-28T01:13:32Z 후 1h18m17s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1027 |
| 36 | 2026-09-28T03:01:49Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-09-28T01:13:32Z 후 1h48m17s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1028 |
| 37 | 2026-09-28T03:31:49Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-09-28T01:13:32Z 후 2h18m17s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1029 |
| 38 | 2026-09-28T04:01:49Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-09-28T01:13:32Z 후 2h48m17s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1030 |
| 39 | 2026-09-28T04:31:49Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-09-28T01:13:32Z 후 3h18m17s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1031 |
| 40 | 2026-09-28T05:01:49Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-09-28T01:13:32Z 후 3h48m17s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1032 |
| 41 | 2026-09-28T05:31:49Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-09-28T01:13:32Z 후 4h18m17s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1033 |
| 42 | 2026-09-28T06:01:49Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-09-28T01:13:32Z 후 4h48m17s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1034 |
| 43 | 2026-09-28T06:31:49Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-09-28T01:13:32Z 후 5h18m17s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1035 |
| 44 | 2026-09-28T07:01:49Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-09-28T01:13:32Z 후 5h48m17s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1036 |
| 45 | 2026-09-28T07:31:49Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-09-28T01:13:32Z 후 6h18m17s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1037 |
| 46 | 2026-09-28T08:01:49Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-09-28T01:13:32Z 후 6h48m17s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1038 |
| 47 | 2026-09-28T08:31:49Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-09-28T01:13:32Z 후 7h18m17s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1039 |
| 48 | 2026-09-28T09:01:50Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=5399 · 창 시작 2026-09-28T01:13:32Z 후 7h48m18s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1040 |
| 49 | 2026-09-28T09:31:50Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-09-28T01:13:32Z 후 8h18m18s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1041 |
| 50 | 2026-09-28T10:01:50Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-09-28T01:13:32Z 후 8h48m18s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1042 |
| 51 | 2026-09-28T10:31:50Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-09-28T01:13:32Z 후 9h18m18s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1043 |
| 52 | 2026-09-28T11:01:50Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-09-28T01:13:32Z 후 9h48m18s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1044 |
| 53 | 2026-09-28T11:31:50Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-09-28T01:13:32Z 후 10h18m18s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1045 |
| 54 | 2026-09-28T12:01:50Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-09-28T01:13:32Z 후 10h48m18s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1046 |
| 55 | 2026-09-28T12:31:50Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-09-28T01:13:32Z 후 11h18m18s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1047 |
| 56 | 2026-09-28T13:01:50Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-09-28T01:13:32Z 후 11h48m18s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1048 |
| 57 | 2026-09-28T13:31:50Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-09-28T01:13:32Z 후 12h18m18s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1049 |
| 58 | 2026-09-28T14:01:50Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-09-28T01:13:32Z 후 12h48m18s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1050 |
| 59 | 2026-09-28T14:31:50Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-09-28T01:13:32Z 후 13h18m18s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1051 |
| 60 | 2026-09-28T15:01:51Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=5399 · 창 시작 2026-09-28T01:13:32Z 후 13h48m19s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1052 |
| 61 | 2026-09-28T15:31:51Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-09-28T01:13:32Z 후 14h18m19s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1055 |
| 62 | 2026-09-28T16:01:51Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-09-28T01:13:32Z 후 14h48m19s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1056 |
| 63 | 2026-09-28T16:31:51Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-09-28T01:13:32Z 후 15h18m19s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1057 |
| 64 | 2026-09-28T17:01:51Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-09-28T01:13:32Z 후 15h48m19s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1058 |
| 65 | 2026-09-28T17:31:51Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-09-28T01:13:32Z 후 16h18m19s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1059 |
| 66 | 2026-09-28T18:01:51Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-09-28T01:13:32Z 후 16h48m19s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1060 |
| 67 | 2026-09-28T18:31:51Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-09-28T01:13:32Z 후 17h18m19s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1061 |
| 68 | 2026-09-28T19:01:51Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-09-28T01:13:32Z 후 17h48m19s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1062 |
| 69 | 2026-09-28T19:31:51Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-09-28T01:13:32Z 후 18h18m19s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1063 |
| 70 | 2026-09-28T20:01:51Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-09-28T01:13:32Z 후 18h48m19s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1064 |
| 71 | 2026-09-28T20:31:51Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-09-28T01:13:32Z 후 19h18m19s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1065 |
| 72 | 2026-09-28T21:01:52Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=5399 · 창 시작 2026-09-28T01:13:32Z 후 19h48m20s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1066 |
| 73 | 2026-09-28T21:31:52Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-09-28T01:13:32Z 후 20h18m20s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1067 |
| 74 | 2026-09-28T22:01:52Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-09-28T01:13:32Z 후 20h48m20s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1068 |
| 75 | 2026-09-28T22:31:52Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-09-28T01:13:32Z 후 21h18m20s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1069 |
| 76 | 2026-09-28T23:01:52Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-09-28T01:13:32Z 후 21h48m20s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1070 |
| 77 | 2026-09-28T23:23:40Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-ds4f0731-1m-spec7-roce 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-28T01:13:32Z 후 22h10m08s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1071 |
| 78 | 2026-09-28T23:23:41Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-ds4f0731-1m-spec7-roce 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-28T01:13:32Z 후 22h10m09s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-09-29T04:54:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:523 |
| 79 | 2026-09-29T04:54:44Z | main | `budget_declare` | `smoke-ds4f0731-1m-spec7-roce` | 예산 선언(선언값 — 측정 아님) · floor_mib=21479 · weights_mib=79577 · kv_mib=10240 · overhead_mib=13312 · mem_total_mib=124608 · 기동 창 4/4: budget_renew 17회 · 닫힘 budget_clear 2026-09-29T05:54:03Z(선언 후 59m19s) · measured_utc 2026-09-29T05:52:10Z 가 이 창 안(이 측정의 기동) | docs/logs/main/events/2026-09.jsonl:1073 |
| 80 | 2026-09-29T04:54:45Z | main | `budget_honored` | `smoke-ds4f0731-1m-spec7-roce` | budget_honored · floor_mib=21479 · arm_ceiling_mib=18407 · remaining_s=7199 · 창 시작 2026-09-29T04:54:45Z 후 0m00s | docs/logs/main/events/watchdog.jsonl:524 |
| 81 | 2026-09-29T04:56:02Z | cluster | `engine_log_window_start` | — | 캡처된 엔진 로그의 첫 시각(꼬리 캡처일 수 있다 — 엔진 시작 시각이 아닐 수 있다) · 선언 2026-09-29T04:54:44Z 후 1m18s · 시계: 컨테이너 시각을 UTC 로 읽어 측정 창 [2026-09-29T04:54:44Z, 2026-09-29T05:54:03Z] 안에 드는 것을 확인 | output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce/lite_engine_ds4f0731-1m-spec7-roce.log:151 |
| 82 | 2026-09-29T04:56:06Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=81406 · rate_mib_s=22960 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → smoke-ds4f0731-1m-spec7-roce 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-29T04:54:45Z 후 1m21s) | docs/logs/main/events/watchdog.jsonl:525 |
| 83 | 2026-09-29T04:56:08Z | cluster | `engine_prefetch_disabled` | — | 가중치 auto-prefetch 꺼짐 줄 ×4 — filesystem CIFS(vLLM 로그 문구) | output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce/lite_engine_ds4f0731-1m-spec7-roce.log:282 |
| 84 | 2026-09-29T05:08:25Z | cluster | `engine_weights_loaded` | — | 가중치 적재 완료 줄 ×4(rank·단계별) · 최장 362.21s · 이 행 = 마지막 줄 | output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce/lite_engine_ds4f0731-1m-spec7-roce.log:585 |
| 85 | 2026-09-29T05:08:39Z | cluster | `engine_model_loaded` | — | 모델 적재 완료 줄 ×2(rank 별) · 마지막 줄 79.04 GiB · 754.4s | output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce/lite_engine_ds4f0731-1m-spec7-roce.log:592 |
| 86 | 2026-09-29T05:09:30Z | cluster | `engine_kv_cache_sized` | — | GPU KV cache 1,941,478 tokens · 요청당 1,048,576 tokens 기준 최대 동시성 1.85x | output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce/lite_engine_ds4f0731-1m-spec7-roce.log:632 |
| 87 | 2026-09-29T05:10:32Z | cluster | `engine_shm_broadcast_wait` | — | shm_broadcast 대기 경고(60s 단위) ×2 · 2026-09-29T05:10:32Z–2026-09-29T05:11:32Z(1m00s) — vLLM 문구상 프로세스 hang 또는 긴 작업(컴파일·가중치/KV 양자화) 중 출현 | output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce/lite_engine_ds4f0731-1m-spec7-roce.log:683 |
| 88 | 2026-09-29T05:11:51Z | cluster | `engine_init_done` | — | 엔진 초기화(profile·KV 할당·warmup) 191.98s | output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce/lite_engine_ds4f0731-1m-spec7-roce.log:730 |
| 89 | 2026-09-29T05:12:07Z | cluster | `server_starting` | — | API 서버 기동 — 이 측정 창 선언 2026-09-29T04:54:44Z 후 17m23s · 캡처 로그 첫 시각 2026-09-29T04:56:02Z 후 16m05s | output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce/lite_engine_ds4f0731-1m-spec7-roce.log:737 |
| 90 | 2026-09-29T05:12:32Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=6132 · 창 시작 2026-09-29T04:54:44Z 후 17m48s | docs/logs/main/events/2026-09.jsonl:1074 |
| 91 | 2026-09-29T05:14:13Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=7099 · 창 시작 2026-09-29T04:54:44Z 후 19m29s | docs/logs/main/events/2026-09.jsonl:1075 |
| 92 | 2026-09-29T05:17:04Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=7029 · 창 시작 2026-09-29T04:54:44Z 후 22m20s | docs/logs/main/events/2026-09.jsonl:1076 |
| 93 | 2026-09-29T05:19:33Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=7051 · 창 시작 2026-09-29T04:54:44Z 후 24m49s | docs/logs/main/events/2026-09.jsonl:1077 |
| 94 | 2026-09-29T05:21:56Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=7057 · 창 시작 2026-09-29T04:54:44Z 후 27m12s | docs/logs/main/events/2026-09.jsonl:1078 |
| 95 | 2026-09-29T05:24:24Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=7052 · 창 시작 2026-09-29T04:54:44Z 후 29m40s | docs/logs/main/events/2026-09.jsonl:1079 |
| 96 | 2026-09-29T05:26:57Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=7047 · 창 시작 2026-09-29T04:54:44Z 후 32m13s | docs/logs/main/events/2026-09.jsonl:1080 |
| 97 | 2026-09-29T05:29:27Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=7050 · 창 시작 2026-09-29T04:54:44Z 후 34m43s | docs/logs/main/events/2026-09.jsonl:1081 |
| 98 | 2026-09-29T05:31:58Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=7049 · 창 시작 2026-09-29T04:54:44Z 후 37m14s | docs/logs/main/events/2026-09.jsonl:1082 |
| 99 | 2026-09-29T05:34:30Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=7048 · 창 시작 2026-09-29T04:54:44Z 후 39m46s | docs/logs/main/events/2026-09.jsonl:1083 |
| 100 | 2026-09-29T05:37:02Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=7048 · 창 시작 2026-09-29T04:54:44Z 후 42m18s | docs/logs/main/events/2026-09.jsonl:1084 |
| 101 | 2026-09-29T05:39:32Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=7050 · 창 시작 2026-09-29T04:54:44Z 후 44m48s | docs/logs/main/events/2026-09.jsonl:1085 |
| 102 | 2026-09-29T05:42:04Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=7048 · 창 시작 2026-09-29T04:54:44Z 후 47m20s | docs/logs/main/events/2026-09.jsonl:1086 |
| 103 | 2026-09-29T05:42:32Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=7172 · 창 시작 2026-09-29T04:54:44Z 후 47m48s | docs/logs/main/events/2026-09.jsonl:1087 |
| 104 | 2026-09-29T05:44:37Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=7075 · 창 시작 2026-09-29T04:54:44Z 후 49m53s | docs/logs/main/events/2026-09.jsonl:1088 |
| 105 | 2026-09-29T05:47:10Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=7047 · 창 시작 2026-09-29T04:54:44Z 후 52m26s | docs/logs/main/events/2026-09.jsonl:1089 |
| 106 | 2026-09-29T05:49:42Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=7048 · 창 시작 2026-09-29T04:54:44Z 후 54m58s | docs/logs/main/events/2026-09.jsonl:1090 |
| 107 | 2026-09-29T05:54:03Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-ds4f0731-1m-spec7-roce 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-29T04:54:44Z 후 59m19s) | docs/logs/main/events/2026-09.jsonl:1091 |
| 108 | 2026-09-29T05:54:04Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-ds4f0731-1m-spec7-roce 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-29T04:54:45Z 후 59m19s) | docs/logs/main/events/watchdog.jsonl:526 |

**기동 시도**(예산 선언 `budget_declare` 마다 한 시도 · 같은 노드의 이어지는 행과 선언 없는 노드(엔진 로그)의 그 시각 행을 묶었다 · 사살 · 트립 · 거부는 닫힘과 따로 센다 · `measurement` 행은 측정 기록이라 시도로 세지 않는다(위 표에만) — 해석은 저작자 몫)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 1 | main | `smoke-ds4f0731-1m-spec7-roce` | 2026-09-29T04:54:44Z | 17회 · 첫 2026-09-29T05:12:32Z | 없음 | `budget_clear` 2026-09-29T05:54:03Z | 11 |

**같은 셀 이름 · 앞선 캠페인의 기동 3회**(캠페인 경계 2026-09-29T04:54:44Z 이전 — 라벨이 같아 원장에서 함께 묶였을 뿐 이 셀의 시도로 세지 않는다 · 행 전문 = 01 §1.4)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 앞선-1 | main | `smoke-ds4f0731-1m-spec7-roce` | 2026-09-27T23:32:54Z | 없음 | 없음 | `budget_clear` 2026-09-27T23:34:58Z | 3 |
| 앞선-2 | main | `smoke-ds4f0731-1m-spec7-roce` | 2026-09-27T23:37:13Z | 18회 · 첫 2026-09-27T23:54:55Z | 없음 | `budget_clear` 2026-09-28T01:13:03Z | 3 |
| 앞선-3 | main | `smoke-ds4f0731-1m-spec7-roce` | 2026-09-28T01:13:32Z | 44회 · 첫 2026-09-28T01:31:48Z | 없음 | `budget_clear` 2026-09-28T23:23:40Z | 3 |
<!-- /FACT:event_timeline -->

<!-- FACT:measurement_env -->
**측정 실행의 env 관측 27행**(측정한 실행의 엔진 로그가 echo 한 값 · 기계 발췌 — 측정 당시 값의 정본이다. 표에 없는 키는 로그가 echo 하지 않은 것이지 설정되지 않았다는 뜻이 아니다 · 측정 뒤 재생성된 env 형상은 01 §1.1 을 본다)

| # | 키 | 값 | 노드 | 종류 | 출처(첫 줄) | 관측 |
|---|---|---|---|---|---|---|
| 1 | `NCCL_SOCKET_IFNAME` | `<nic:cluster>` | main | env-echo | output/multi/benchlog/engine_v7_ds4f0731-1m-spec7-roce.log:L92 | 로그 1개 |
| 2 | `NCCL_SOCKET_IFNAME` | `<nic:cluster>` | main | param-echo | output/multi/benchlog/engine_v7_ds4f0731-1m-spec7-roce.log:L93 | 로그 1개 |
| 3 | `nccl:version` | `2.30.7+cuda13.3` | main | runtime | output/multi/benchlog/engine_v7_ds4f0731-1m-spec7-roce.log:L96 | 로그 1개 |
| 4 | `NCCL_NET_PLUGIN` | `spcx` | main | env-echo | output/multi/benchlog/engine_v7_ds4f0731-1m-spec7-roce.log:L98 | 로그 1개 |
| 5 | `NCCL_IB_DISABLE` | `0` | main | env-echo | output/multi/benchlog/engine_v7_ds4f0731-1m-spec7-roce.log:L103 | 로그 1개 |
| 6 | `NCCL_IB_HCA` | `=<nic:cluster>,<nic:cluster>` | main | param-echo | output/multi/benchlog/engine_v7_ds4f0731-1m-spec7-roce.log:L105 | 로그 1개 |
| 7 | `NCCL_IB_MERGE_NICS` | `1` | main | env-echo | output/multi/benchlog/engine_v7_ds4f0731-1m-spec7-roce.log:L108 | 로그 1개 |
| 8 | `nccl:net_ib_devices` | `[0]<nic:cluster>:1/RoCE [1]<nic:cluster>:1/RoCE [RO]; OOB <nic:cluster>:<node:main><0>` | main | runtime | output/multi/benchlog/engine_v7_ds4f0731-1m-spec7-roce.log:L111 | 로그 1개 |
| 9 | `NCCL_IB_QPS_PER_CONNECTION` | `4` | main | env-echo | output/multi/benchlog/engine_v7_ds4f0731-1m-spec7-roce.log:L124 | 로그 1개 |
| 10 | `nccl:network` | `IB` | sub | runtime | output/multi/benchlog/engine_v7_ds4f0731-1m-spec7-roce.log:L147 | 로그 8개 |
| 11 | `NCCL_DMABUF_ENABLE` | `0` | sub | env-echo | output/multi/benchlog/engine_v7_ds4f0731-1m-spec7-roce.log:L149 | 로그 1개 |
| 12 | `NCCL_CROSS_NIC` | `1` | sub | env-echo | output/multi/benchlog/engine_v7_ds4f0731-1m-spec7-roce.log:L152 | 로그 1개 |
| 13 | `NCCL_IB_SPLIT_DATA_ON_QPS` | `0` | main | env-echo | output/multi/benchlog/engine_v7_ds4f0731-1m-spec7-roce.log:L608 | 로그 1개 |
| 14 | `NCCL_IB_GID_INDEX` | `3` | main | env-echo | output/multi/benchlog/engine_v7_ds4f0731-1m-spec7-roce.log:L617 | 로그 2개 |
| 15 | `NCCL_SOCKET_IFNAME` | `<nic:cluster>` | sub | env-echo | output/multi/benchlog/engine_v7_ds4f0731-1m-spec7-roce.log:L899 | 로그 7개 · Ray 접힘 +3(접힌 사본의 노드 미관측) |
| 16 | `NCCL_SOCKET_IFNAME` | `<nic:cluster>` | sub | param-echo | output/multi/benchlog/engine_v7_ds4f0731-1m-spec7-roce.log:L900 | 로그 7개 · Ray 접힘 +2(접힌 사본의 노드 미관측) |
| 17 | `nccl:version` | `2.30.7+cuda13.3` | sub | runtime | output/multi/benchlog/engine_v7_ds4f0731-1m-spec7-roce.log:L903 | 로그 7개 |
| 18 | `NCCL_NET_PLUGIN` | `spcx` | sub | env-echo | output/multi/benchlog/engine_v7_ds4f0731-1m-spec7-roce.log:L905 | 로그 7개 |
| 19 | `NCCL_IB_DISABLE` | `0` | sub | env-echo | output/multi/benchlog/engine_v7_ds4f0731-1m-spec7-roce.log:L910 | 로그 7개 · Ray 접힘 +2(접힌 사본의 노드 미관측) |
| 20 | `NCCL_IB_HCA` | `=<nic:cluster>,<nic:cluster>` | sub | param-echo | output/multi/benchlog/engine_v7_ds4f0731-1m-spec7-roce.log:L911 | 로그 7개 · Ray 접힘 +2(접힌 사본의 노드 미관측) |
| 21 | `NCCL_IB_MERGE_NICS` | `1` | sub | env-echo | output/multi/benchlog/engine_v7_ds4f0731-1m-spec7-roce.log:L913 | 로그 7개 |
| 22 | `nccl:net_ib_devices` | `[RO]; OOB <nic:cluster>:<node:main><0>` | main | runtime | output/multi/benchlog/engine_v7_ds4f0731-1m-spec7-roce.log:L915 | 로그 7개 · Ray 접힘 +3(접힌 사본의 노드 미관측) |
| 23 | `NCCL_IB_QPS_PER_CONNECTION` | `4` | sub | env-echo | output/multi/benchlog/engine_v7_ds4f0731-1m-spec7-roce.log:L925 | 로그 7개 · Ray 접힘 +3(접힌 사본의 노드 미관측) |
| 24 | `NCCL_DMABUF_ENABLE` | `0` | main | env-echo | output/multi/benchlog/engine_v7_ds4f0731-1m-spec7-roce.log:L940 | 로그 8개 |
| 25 | `NCCL_CROSS_NIC` | `1` | main | env-echo | output/multi/benchlog/engine_v7_ds4f0731-1m-spec7-roce.log:L943 | 로그 8개 |
| 26 | `NCCL_IB_SPLIT_DATA_ON_QPS` | `0` | sub | env-echo | output/multi/benchlog/engine_v7_ds4f0731-1m-spec7-roce.log:L984 | 로그 8개 |
| 27 | `NCCL_IB_GID_INDEX` | `3` | sub | env-echo | output/multi/benchlog/engine_v7_ds4f0731-1m-spec7-roce.log:L989 | 로그 9개 |

- 값 주: 기계 치환(pii.substitution_table · 장치·호스트·주소는 자리표시 — 원문은 출처 줄)

- Ray 접힘: 출처 줄 끝의 `[repeated Nx across cluster]` — Ray 가 다른 프로세스의 같은 줄 N개를 한 줄로 접었다. 노드 칸은 **첫 줄**의 노드이고, 접힌 사본이 어느 노드의 것인지는 로그에 없다(관측 대상 밖).
<!-- /FACT:measurement_env -->

> 이 절이 답하는 질문: 수신자가 이 zip 만으로 이 셀을 다시 띄우려면 어떤 순서로 무엇을 하고, 각 단계의 성공을 무엇으로 판정하며, 얼마나 기다려야 하는가?

**(a) 사전 준비.** 가중치 획득 · 스테이징 · 네트워크 단계는 1.6 이 명령 수준으로 다룬다. 요지: 가중치는 관리 NAS(`<manifest.nas_model_path>`) 아래 `DeepSeek/DeepSeek-V4/DeepSeek-V4-Flash-0731` 에 있고 compose 가 `NAS_MODEL_PATH` 를 `/app/models` 로 마운트한다 — **양 노드 같은 경로**여야 한다(1.5 · `sub_recipe.json` mount 무리). 스테이징 파일은 없다(PLE 없음 · mmap 미사용). 엔진은 이 파일시스템(CIFS)을 인식된 네트워크 FS 로 보지 않아 가중치 auto-prefetch 를 껐다(engine_v7 L896 · 01 §1.4 행 83 — 같은 줄이 체크포인트 크기를 155.43 GiB 로 적는다) — 경고이지 벽이 아니다. 필수 env 의 자리: 셀 env(`artifacts/triplet/.env.ds4f0731-1m-spec7-roce` — `CONFIG_FILE` · `SERVING_MODEL_NAME` · `IMAGE_TAG` · master/slave 컨테이너 이름 쌍) · 클러스터 env(`.env.cluster.template`) · NCCL env(`.env.interconnect.template` — `NCCL_IB_DISABLE` · `NCCL_NET` 이 `<derived:interconnect.nccl_transport→…>` 자리표시). 측정 당시 NCCL 값은 위 '측정 실행의 env 관측' 표가 정본이다(`NCCL_IB_DISABLE` `0` · `nccl:network` `IB` · `NCCL_IB_GID_INDEX` `3` 등). 이 형상은 manifest 에 `interconnect.nccl_transport: rdma` 를 두고 렌더러로 다시 파생해 얻는다(testlog_26092808 §1 시도 #2 행). 1.1 은 이 판의 compose · 러너 개정이 측정 전 커밋임을 적었고 렌더러의 측정 뒤 변경 경고는 없다 — 그래도 지금 렌더한 `.env.interconnect` 는 위 관측 표와 대조해 `NCCL_IB_DISABLE` · `NCCL_NET` · GID · HCA 가 같은지 맞춘다. 트리플렛은 실린 그대로가 측정 구성이다(`enable-auto-tool-choice` 포함 · 1.2 triplet).

**(b) 빌드.** 양 노드 각각 로컬로 짓는다(이미지 전송 ✗ — 같아야 하는 것은 digest 가 아니라 ABI). 선택자 `Dockerfile.source-build` · build-arg 는 위 표(`VLLM_REF=v0.29.0rc6` · `TORCH_CUDA_ARCH=12.1a` · `RAY_VERSION=2.48.0` · `SM12X_PORT=0` · `SRC_DEPS_AUTHORITY=0` · `BUILD_JOBS=8`). 빌드 소요는 관측되지 않았다 — 위 표의 347시간은 캐시 재사용 층을 포함한 층 창이지 빌드 시간이 아니다. 이 run 의 측정 이미지 digest 는 09-28 측정과 같다(두 sweep meta `image_digest` · 끝 층 2026-09-22T23:38:10Z).

**(c) 기동과 성공 판정.**(L 번호는 같은 실행의 전체 엔진 로그 사본 `engine_v7` 기준 — 꼬리 캡처 `lite_engine` 로그는 01 §1.4 이벤트 표가 인용한다) `multinode_serve_smoke.sh <cell> --keep-up` 이 예산 선언(양 노드 honored) → master(Ray head + serve) → slave(Ray worker) → health 폴링 → chat 추론 1회를 한다. 이 run 의 관측(원장 시도 1): 선언 04:54:44Z → 가중치 적재 줄이 rank 당 두 번(355.62 · 355.11 s 뒤 dspark draft 적재 줄과 함께 362.03 · 362.21 s · L1050 · L1126 · L1192 · L1199) → 모델 적재 79.04 GiB · 753.6~754.4 s(L1197 · L1206) → KV 크기 결정 05:09:30Z(L1246) → 엔진 초기화 191.98 s(L1344) → API 서버 기동 05:12:07Z(L1351), **선언 후 17분 23초**(01 §1.4 행 89). 스모크 로그가 이 기동을 "READY ~1040s" 로 적었다(아래 발췌) — 원장 선언 → API 서버 기동 1043 s 와 사실상 같다. 성공 판정은 health 200 + chat 추론 1회다 — 위 FACT 의 serve_proof(05:12:32Z 작성 · content_len 1 · reasoning_len 95 · finish stop)는 측정 전의 스모크 산출물이고 측정을 서빙한 같은 기동의 스모크 직후 관측이다(FACT 판별 — 스모크 로그 SMOKE PASS → attestation 작성 → `--keep-up` 순서 · 그 기동의 예산 선언 창이 측정 끝까지 닫힘 없이 이어짐 · 측정 엔진 APIServer pid 하나). NCCL 전송은 엔진 로그 `Using network IB`(L147 · L938 `[repeated 3x across cluster]`)로 실증한다 — 이 줄 없이 PASS 를 선언하지 마라(W17). tool call 용처는 `"auto" tool choice has been enabled.`(L1349)를 보고, tools + `tool_choice:"auto"` 요청 1회가 finish `tool_calls` 로 돌아오는지 본다(이 run 은 통과 · 02 W19 발췌 · testlog_26092914_54_41_hint_v7_라이브E2E_DS4F0731_판정 §2). 정상인데 무서워 보이는 것: 초기화 중 `No available shared memory broadcast block found in 60 seconds` 가 1분 간격 2회(L1297 · L1340 — 그 사이 CUDA graph 캡처 `Graph capturing finished in 9 secs` · L1339 · L1343) 뒤 초기화가 끝났다(01 §1.4 행 87) · 로드 중 워치독 `watchdog_highrate_hold`(`legacy_rule_would_trip=true`)가 이 run 시도 1 에서 1회(행 82) — 선언 창이 급락률을 보류한 기록이다.

> [원문] docs/simlog/26092914_54_41_hint_v7_live_ds4f/serve.log §L31-35
> [mn] READY ~1040s
> [mn] smoke(chat): verdict=content content_len=1 reasoning_len=95 finish_reason=stop
> [mn] SMOKE PASS — evidence=chat.content (content_len=1 fr=stop)
> [mn] 노드 정합 attestation v2(serve) → output/multi/benchlog/attestation_ds4f0731-1m-spec7-roce.json build_ledger=equal driver=equal torch=equal vllm_sha=equal
> [mn] serve proof → output/multi/benchlog/serve_proof_ds4f0731-1m-spec7-roce.json (verdict=pass evidence=chat.content)


**기동 시도 수: 1.** 02 §2.2 기동 시도 표는 캠페인 경계(2026-09-29T04:54:44Z)로 갈려 이번 캠페인의 시도는 1 하나이고, 같은 셀 이름의 앞선 캠페인(09-28) 기동 3회는 별도 표 `앞선-1~3` 이다(이 셀의 시도로 세지 않는다) — 앞선-1(갱신 없음 · 2분 만에 clear)은 rdma 첫 정의가 Socket 으로 떨어져 로드 도중 회수한 것(W17 · testlog_26092808 §1), 앞선-2 가 그 판의 측정(행 26 이 그 측정), 앞선-3 은 그 판의 측정 뒤 tool-choice 줄을 더해 재기동한 상주다(시각 순서상 testlog_26092808 §6 과 맞는다 — 원장 행 자체는 yaml 차이를 적지 않는다). **시도 1(선언 04:54:44Z · 갱신 17회 · `budget_clear` 05:54:03Z)이 이 run 이며 측정(05:13:42Z → 05:52:10Z)이 이 창 안이다**(행 79 의 기동 창 표시). main 원장 기준 사살 · 트립 · 거부 0 이다(sub 원장은 2026-09-11 뒤 미회수 — 서브의 트립은 관측 범위 밖).

**(d) 호스트 메모리 안전 예산.** 선언값(행 79): `mem_total_mib=124608` · `weights_mib=79577`(체크포인트 ÷ TP) · `kv_mib=10240` · `overhead_mib=13312` → `floor_mib=21479` · arm 상한 18,407 MiB(행 80) · TTL 7200 s(갱신 루프가 상주 중 연장). 전부 **선언**이다 — 엔진 로그의 실측은 rank 당 모델 적재 79.04 GiB · KV 10.0 GiB · 예약 합 89.04 GiB(03 §3.1 `lite.engine_weights_gib` · `lite.engine_kv_gib` · `lite.engine_reserved_total_gib`)다. overhead 13312 는 09-09 의 12265 에 여유를 얹은 선언이다(plan_26092808 §2 예산 행). 스모크 입력은 `SMOKE_BUDGET_OVERHEAD_MIB=13312` · `READY_MAX=360`(5초 폴링 · plan_26092808 §3 단계 3).

**(e) 측정 명령.** 위 표 bench 단계의 `vllm bench serve` 두 줄은 lite 레그 bench JSON 과 측정 도구 원문의 리터럴 인자에서 **재구성**한 것이지 실행 원문이 아니다(입력 512 는 인자 · 595 는 chat 템플릿을 포함한 실측 토큰). full 레그는 GuideLLM 0.7.3 · 입력 1024 / 출력 256 · 동시성 1/2/4/8/16 × 반복 3 이고, JSON 에 없는 도구 인자는 03 §3.4 의 측정 도구 원문 발췌가 정본이다.

**(f) 엔진이 스스로 고른 런타임 경로.** 통신: NCCL `2.30.7+cuda13.3` · SPCX 플러그인(v12) 로드(L100) 뒤 장치 미지원으로 건너뛰고(L129 — 무해한 WARN) 내장 IB 로 `Using network IB`(L147 · L938) · `Connected all rings, use ring PXN 0 GDR 0`(L770 · L1049) — GPU Direct RDMA 는 꺼져 있다. 드라이버의 DMA-BUF/GDR 미지원 보고(testlog_26092807 §1)와 env `NCCL_DMABUF_ENABLE=0`(양 노드 echo · 01 §1.4 행 11 · 24)이 함께 걸려 있다 — 같은 커널 · 드라이버의 모델리스 대조 변종 B(`NCCL_DMABUF_ENABLE=1` · `NCCL_NET_GDR_LEVEL=SYS` · `NCCL_NET_GDR_C2C=1`)에서도 NCCL 이 GDR 을 스스로 껐으므로(testlog_26092807 §2) 드라이버 쪽을 가리키지만, 서빙 경로에서 직접 가른 대조는 없다. 엔진이 끈 기능: `SymmMemCommunicator` · FlashInfer All Reduce(world_size=2 미지원) · custom collectives(MNNVL 아님) — 경고 3줄(L765-767 · L1046-1048)이며 벽이 아니다 · spec 설정 때문에 `max_num_scheduled_tokens` 를 2048 로 정했다(L74 · L1346). 모델: dense `DeepGemmFp8BlockScaledMMKernel`(L890) · KV `Using DeepSeek's fp8_ds_mla KV cache format.`(L891) · MoE `Using 'HUMMING' Mxfp4 MoE backend.`(L893) 뒤 humming GEMM config 재정의 줄(L1065~) — 경고가 아니라 튜닝 로그다 · dspark draft `DSpark draft model loaded: 97 params`(L1191). API 층: `"auto" tool choice has been enabled.`(L1349).

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
| `NAS_MODEL_PATH` | mount | project_env | 결함#2b(plan_26070119): --env-file 을 쓰면 프로젝트 .env 자동 로드가 꺼져 compose 기본 마운트로 폴백 → 모델 부재(serve 즉사). 셸 env 로 명시 주입한다. |
| `QUANT_MODEL_PATH` | mount | project_env | 결함#2b 와 같은 경로 — 양자화 모델 마운트원. |
| `TIKTOKEN_HOST_PATH` | mount | project_env | 결함#2b 와 같은 경로 — 오프라인 토크나이저 마운트원. |
| `PLE_MMAP_HOST_PATH` | mount | project_env | 2026-09-09(camp-26090918): PLE mmap 로컬 NVMe 스테이징 마운트원 — slave 도 같은 경로에 실재해야 한다. |
| `SLAVE_CONTAINER_NAME` | cluster | cell_env | 2026-08-14: 폴백 이름이면 slave 협역 워치독 필터가 매칭 0 — 서브에서만 계층 2층이 조용히 사라진다(devlog_26080212 ⑥ 부류 · 하드다운 #2 는 서브였다). 2026-08-02 에 이미 '엉뚱한 이름으로 떠 있었고 아무도 몰랐다' 로 한 번 드러났다(그때는 마스터측만 교정). |

**이미지 정체성 build-arg**: `IMAGE_TAG`

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
| `observed` | {"build_ledger": {"compared": "dockerfile · build_args(− exempt) · patches(phase,file,script_sha256,result) · vllm git sha", "identity_exempt_build_args": ["BUILD_JOBS"], "main_identity_sha256": "fcc9de2eb54ffb0ba1e49089dc9584774f390126b2503ffc84d8014cd836e413", "sub_identity_sha256": "fcc9de2eb54ffb0ba1e49089dc9584774f390126b2503ffc84d8014cd836e413", "verdict": "equal"}, "driver": {"main": "580.178.04", "sub": "580.178.04", "verdict": "equal"}, "torch": {"main": "2.13.0a0+9186a08b2c.nv26.7.59513937", "sub": "2.13.0a0+9186a08b2c.nv26.7.59513937", "verdict": "equal"}, "vllm_sha": {"main": "74c96922ecb9017f413318c76d1af83aa2ab45a5", "sub": "74c96922ecb9017f413318c76d1af83aa2ab45a5", "verdict": "equal"}} |
| `source` | output/multi/benchlog/attestation_ds4f0731-1m-spec7-roce.json |

> **attestation 범위** — config `ds4f0731-1m-spec7-roce` · phase serve · 작성 2026-09-29T05:12:32Z(output/multi/benchlog/attestation_ds4f0731-1m-spec7-roce.json 파일 mtime(파일시스템 관측 — 문서에 시각 칸이 없다 · 기록 필드 아님)) · 묶은 근거 cell-name · 파일 `output/multi/benchlog/attestation_ds4f0731-1m-spec7-roce.json` · 측정 창 대비 작성 2026-09-29T05:12:32Z < 측정 시작 2026-09-29T05:13:42Z — 측정 전(빌드·스모크)의 관측: **same config(ds4f0731-1m-spec7-roce) · serve — 측정을 서빙한 같은 기동의 스모크 직후 관측(판별: 스모크 로그 docs/simlog/26092914_54_41_hint_v7_live_ds4f/serve.log L33 SMOKE PASS → L34 attestation 작성 → L37 --keep-up · 원장 docs/logs/main/events/2026-09.jsonl:1073 budget_declare(smoke-ds4f0731-1m-spec7-roce · 2026-09-29T04:54:44Z) 가 측정 끝 2026-09-29T05:52:10Z 까지 재선언 · 창 닫힘 없이 이어짐(main 원장 기록 범위 안) · 스모크 로그 budget_honored ts 2026-09-29T04:54:45Z = 그 선언의 원장 budget_honored · 측정 엔진 로그 APIServer pid 790 하나 · 측정 시작 2026-09-29T05:13:42Z)**
<!-- /FACT:sub_recipe -->

<!-- FACT:measurement_env_nodes -->
**노드별 측정 env**(01 §1.4 표를 키 × 노드로 접은 것 · `만 관측` = 다른 쪽 줄이 캡처 로그에 없다 · Ray 가 같은 줄을 접은 키(`[repeated Nx across cluster]`)와 꼬리 캡처 로그(01 §1.4 로그 창)에서 한쪽만 남은 키는 다른 쪽 부재를 단언하지 않는다)

- 양 노드 같은 값 11개: `NCCL_CROSS_NIC` · `NCCL_DMABUF_ENABLE` · `NCCL_IB_DISABLE` · `NCCL_IB_GID_INDEX` · `NCCL_IB_HCA` · `NCCL_IB_MERGE_NICS` · `NCCL_IB_QPS_PER_CONNECTION` · `NCCL_IB_SPLIT_DATA_ON_QPS` · `NCCL_NET_PLUGIN` · `NCCL_SOCKET_IFNAME` · `nccl:version`

| 키 | main | sub | 노드 미특정 | 판정 |
|---|---|---|---|---|
| `nccl:net_ib_devices` | `[0]<nic:cluster>:1/RoCE [1]<nic:cluster>:1/RoCE [RO]; OOB <nic:cluster>:<node:main><0>` · `[RO]; OOB <nic:cluster>:<node:main><0>` | — | — | main 만 줄에 남음 · Ray 접힘 +3 — sub 사본 여부 미관측 |
| `nccl:network` | — | `IB` | — | sub 만 관측 |
<!-- /FACT:measurement_env_nodes -->

> 이 절이 답하는 질문: 서브(Ray worker) 노드는 메인과 무엇이 다르고, 무엇을 반드시 같게 맞춰야 하는가?

**(1) 역할 차이.** master(메인)는 Ray head 와 `vllm serve` 를 띄우고 트리플렛(`CONFIG_FILE=ds4f0731-1m-spec7-roce`)을 받는다. slave(서브)는 Ray worker 이며 트리플렛을 받지 않는다 — 같은 이미지 이름 · 같은 compose · 같은 `serve_runner.sh` 가 `NODE_ROLE` 로 갈리고 slave 의 `CONFIG_FILE` 은 `default` 다. 그래서 `enable-auto-tool-choice` 같은 서빙 노브는 master 쪽 yaml 에만 있으면 된다. slave 는 master 를 기다렸다 `ray start --address` 로 합류하고, master 는 `ray status` 의 GPU 합류가 리터럴 `2`(serve_runner.sh §L79)와 같아질 때까지 기다린 뒤 serve 한다 — 노드 · GPU 수가 다르면 이 줄을 고치지 않으면 대기에서 멈춘다. 런타임 패치는 양쪽 다 무장되지 않는다(패치 파일 없음 · slave 는 `default_patch.py` 도 없어 no-op).

**(2) 서브 전달 env(무리별).** 이미지 정체성 — `IMAGE_TAG`(모르면 compose 기본 태그로 빌드 · 기동한다). 마운트 — `NAS_MODEL_PATH` · `QUANT_MODEL_PATH` · `TIKTOKEN_HOST_PATH` · `PLE_MMAP_HOST_PATH`(서브도 가중치 절반을 올리므로 같은 경로의 마운트원이 필요하다 · `--env-file` 사용 시 프로젝트 `.env` 자동 로드가 꺼지는 결함 때문에 셸 env 로 명시 주입 · PLE 마운트원은 이 모델에 쓰이지 않는다). 클러스터 — `SLAVE_CONTAINER_NAME`(폴백 이름이면 서브 워치독 필터가 매칭 0 · W5 의 이름 쌍). 노드별 값은 `<manifest.nodes[main].host>` · `<manifest.nodes[sub].host>` 형상이다. NCCL · Ray env 는 서브가 렌더러 산출 `.env.interconnect` · `.env.cluster` 를 정식 배달(`sync_to_sub.sh`)로 받는다 — 전송 선택은 그 배달로만 바꾼다(W16 · 서브 직접 수정 ✗).

**(3) 선행조건.** 가중치가 서브의 **같은 경로**에 실재 · 드라이버 계열 동일(양 노드 관측 580.178.04 · attestation `parity.driver` equal) · 인터커넥트(RoCE v2 · manifest `interconnect` 의 HCA · GID 3) 준비와 `nccl_transport: rdma` 를 반영한 렌더 산출물이 서브에 배달돼 있어야 한다. 스모크의 RAM 게이트는 서브 가용 메모리도 본다(testlog_26090904 §4 — 요구 89818MiB = 체크포인트 ÷ TP + 바닥).

**(4) ABI 동일성.** attestation 은 vLLM SHA(`74c96922ecb9017f413318c76d1af83aa2ab45a5`) · torch(`2.13.0a0+9186a08b2c.nv26.7.59513937`) · driver(580.178.04) · 빌드 원장 identity sha256 이 양 노드 `equal` 이라 기록했다(위 FACT). 이 파일은 측정 전(05:12:32Z)에 쓰였고 측정을 서빙한 같은 기동의 스모크 직후 관측이다(FACT 판별 — 스모크 로그 SMOKE PASS → attestation 작성 → `--keep-up` 순서 · 그 기동의 예산 선언 창이 측정 끝까지 닫힘 없이 이어짐 · 측정 엔진 APIServer pid 하나) — 위 FACT 범위 줄. vLLM 빌드 축은 측정 실행의 엔진 기동 배너(engine_v7 L58)로도 같은 문자열이 보인다. 이미지 digest 는 노드마다 로컬 빌드라 같을 필요가 없고, 이 페이로드는 메인 측정 digest 만 싣는다(00 §0.4). sub 의 블랙박스 원장은 2026-09-11 이후가 회수되지 않았다(01 §1.4 원장 관측 범위) — 다만 이 run 서브의 `budget_honored`(ts 2026-09-29T04:54:48Z · floor 21479 · arm 18407)는 스모크 로그가 원장 JSON 그대로 echo 했다(docs/simlog/26092914_54_41_hint_v7_live_ds4f/serve.log L26 · 01 §1.4 보조 관측 · sub). 서브의 예산 선언은 스모크 스크립트의 문장 줄(선언 발행 · 예상 바닥 · arm 상한 — serve.log L23-25)로만 남았고 원장 JSON 은 아니다. 그 밖의 서브 사건(갱신 · 트립 · clear)은 관측 범위 밖이다.

**(5) 측정 당시 인터커넥트 실효값.** 01 §1.4 '측정 실행의 env 관측' 표(27행)는 이 run 의 전체 엔진 로그 사본 `engine_v7_ds4f0731-1m-spec7-roce.log` 에서 뽑았고, 두 노드 모두의 줄이 남아 있다 — 양 노드 같은 값 11개: `NCCL_CROSS_NIC` `1` · `NCCL_DMABUF_ENABLE` `0` · `NCCL_IB_DISABLE` `0` · `NCCL_IB_GID_INDEX` `3` · `NCCL_IB_HCA` `=<nic:cluster>,<nic:cluster>` · `NCCL_IB_MERGE_NICS` `1` · `NCCL_IB_QPS_PER_CONNECTION` `4` · `NCCL_IB_SPLIT_DATA_ON_QPS` `0` · `NCCL_NET_PLUGIN` `spcx` · `NCCL_SOCKET_IFNAME` `<nic:cluster>` · `nccl:version` `2.30.7+cuda13.3`(main L92~L124 · L608 · L617 · sub L899~L989 등 · 위 노드별 표). 한쪽만 남은 것은 `nccl:net_ib_devices`(main 줄 · Ray 접힘 +3 — sub 사본 여부 미관측)와 `nccl:network` `IB`(sub 만 관측 · 표의 관측 칸 '로그 8개')뿐이고, Ray 가 접은 줄은 다른 노드가 같은 줄을 냈는지 로그가 말하지 않는다. 실린 `.env.interconnect.template` 은 `NCCL_IB_DISABLE` · `NCCL_NET` 을 키마다 `<derived:interconnect.nccl_transport→…>` 자리표시로 싣고, 1.4 파생 표가 실효값 규칙을 적는다: rdma → `NCCL_IB_DISABLE=0` · `NCCL_NET=IB` | socket → `NCCL_IB_DISABLE=1` · `NCCL_NET=Socket`. 측정 당시 값은 위 로그 관측이고 manifest 선언은 `nccl_transport: rdma` 였다(testlog_26092808 §1 시도 #2 행과 같은 규칙). echo 된 키 · 값은 09-28 측정 실행의 엔진 로그(engine_roce)와 같다.

## 1.6 재현 절차 — 기동 전 준비(사람이 할 일)
> 1.4 의 명령을 치기 **전에** 사람이 손으로 해야 하는 것이다 — 가중치 획득 · 스테이징 생성 · 네트워크가 필요한 단계. 에이전트가 대신 할 수 없는 자리(다운로드 승인 · 물리 경로 · 외부 계정)가 여기 모인다.

> 이 절이 답하는 질문: 이 셀을 기동하기 전에 사람이 준비해야 하는 것은 무엇이며, 각각을 어떤 명령으로 만들고, 그 절차의 출처는 어디인가?

### 가중치 획득
- 체크포인트: `deepseek-ai/DeepSeek-V4-Flash-0731` · revision(체크포인트 git HEAD) `7872f01b1d1fe23eabc4c98b48bffcef5a386062`(00 §0.2 사실 블록 — reflog 상 측정 시점 checkout = 지금 HEAD). 크기는 엔진 보고 155.43 GiB(engine_v7 L896) · plan 기재 155.4 GiB · 노드당 ~78 GiB(plan_26090819 §2).
- 획득 모드: **관리 NAS 에 이미 실재** — 이 계보는 다운로드하지 않았다. 캠페인 plan 이 모델 획득을 범위 밖으로 적었다:

> [원문] plan_26090819_ds4f0731_멀티TP2_KV3군_1M768K_광의탐색_Hermes §7. 명시적 범위 밖
> - 모델 다운로드 — DS4F-0731은 NAS 실재(획득 게이트 해당 없음).

- 네 환경에 없으면 받는 명령(**추론 — 실행 검증 ✗** · 모델 획득은 사람 승인 게이트다 · 무인 실행 금지): `huggingface-cli download deepseek-ai/DeepSeek-V4-Flash-0731 --revision 7872f01b1d1fe23eabc4c98b48bffcef5a386062 --local-dir <manifest.nas_model_path>/DeepSeek/DeepSeek-V4/DeepSeek-V4-Flash-0731`.
- 받은 뒤 revision 확인(**추론 — 실행 검증 ✗** · 이 페이로드는 git 저장소 형태의 체크포인트에서 HEAD 를 읽었다 — 00 §0.2 출처 칸): `git -C <manifest.nas_model_path>/DeepSeek/DeepSeek-V4/DeepSeek-V4-Flash-0731 rev-parse HEAD`.
- 노드마다: 두 노드가 **같은 경로**로 마운트해야 한다(1.5 (2) · `sub_recipe.json` mount 무리 — `NAS_MODEL_PATH` 가 서브에 셸 env 로 전달된다). 스모크 사용법이 기동 전 경로 실재와 RAM 게이트(요구 = 체크포인트 ÷ TP + 바닥)를 양 노드에서 본다(testlog_26090904 §4 RAM 게이트 줄). 가중치를 CIFS 위에서 읽으면 엔진이 auto-prefetch 를 끈다(engine_v7 L896 — 경고).

### 스테이징(PLE · 기타)
- **해당 없음 — 이 모델에는 PLE 가 없다.** 근거: 모델 config 에 PLE 키(`ple_layer_ids` · `ple_embed_dim`)가 없다는 관측(00 §0.9 후보 표 ple 행) · 캠페인 셀 config `declared_axes.ple_mode: none` · 측정 sweep meta `ple_mode` 가 `resident`(env 에 `VLLM_PLE_MMAP` 부재 = stock 기본값 · 이 모델에선 뜻 없는 기본 표지) · PLE mmap 패치 62 의 발화 서명 무발화(01 §1.1 `inactive-inferred`). 그래서 `PLE_MMAP_HOST_PATH` 마운트원(1.5 (2))은 이 셀에 쓰이지 않는다.
- 스테이징 대신 필요한 준비는 **벤치 전 페이지캐시 드롭**(양 노드)이다 — 1M 로드 밸리가 호스트 플로어를 침범하는 위험의 대응으로 plan 이 적고(plan_26092808 §4), 명령 이름은 광의탐색 재개 지침에 있다:

> [원문] devlog_26090912_ds4f0731_광의탐색_셀루프_서사 §재개 지침
> - 벤치 전 캐시 드롭(`vllm-drop-caches` · 무암호 sudo)은 양노드 모두.

  `vllm-drop-caches` 는 운영자 호스트에 설치된 무암호 sudo 래퍼로 적혀 있고 이 zip 에는 없다 — 네 호스트의 등가 명령 `sync; echo 3 | sudo tee /proc/sys/vm/drop_caches` 는 **추론 — 실행 검증 ✗**. 스모크 사용법(`.claude/skills/upstream-version-watch/scripts/multinode_serve_smoke.sh` — 이 zip 밖)은 RAM 게이트가 모자랄 때와 회수(`--down`) 때 양 노드에서 `vllm-drop-caches` 가 있으면 스스로 부른다(스크립트 원문 관측) · 측정 직전에 드롭을 따로 실행한 기록은 계보에 없다.
- 호스트 메모리 예산 선언은 사람이 따로 하지 않는다 — 스모크 사용법이 `SMOKE_BUDGET_OVERHEAD_MIB` · `READY_MAX` 입력으로 양 노드에 선언한다(plan_26092808 §3 단계 3 · 1.4 (d)).

### 네트워크가 필요한 단계
- 베이스 이미지 pull `nvcr.io/nvidia/pytorch:26.07-py3`(Dockerfile.source-build §L36 `FROM`) — 오프라인 대체: 미리 `docker pull nvcr.io/nvidia/pytorch:26.07-py3` 해 두기(**추론 — 실행 검증 ✗**).
- apt 설치 `ccache`(§L58) · `iproute2 netcat-openbsd iputils-ping dnsutils`(§L218-219) — 오프라인 대체: 사내 apt 미러(추론 — 실행 검증 ✗).
- vLLM 소스 클론 `git clone --filter=blob:none ${VLLM_REPO}` → `checkout --detach ${VLLM_REF}`(§L75-76 · `VLLM_REPO=https://github.com/vllm-project/vllm.git` · `VLLM_REF=v0.29.0rc6`) — 오프라인 대체: 미러 저장소를 `--build-arg VLLM_REPO=<미러>` 로 지정(Dockerfile 이 이 인자를 파라미터로 받는다 · 미러 운영은 추론 — 실행 검증 ✗).
- pip: 빌드 요건(§L84) · `pip install --no-build-isolation -e .`(§L189 — vLLM 의존 해소) · `ray==2.48.0`(§L222) — 오프라인 대체: 사내 PyPI 미러 또는 wheelhouse(추론 — 실행 검증 ✗).
- DeepGEMM `nv_dev` 클론 · wheel 빌드(10-deepgemm.sh §L22 · §L31 · §L61 `pip install … dist/*.whl`) — 오프라인 대체: SHA 핀 `a6b593d2826719dcf4892609af7b84ee23aaf32a` 의 미러(추론 — 실행 검증 ✗).
- 가중치 다운로드(위 가중치 획득 — NAS 에 있으면 불필요).
- 측정 도구 이미지 `ghcr.io/vllm-project/guidellm:v0.7.3`(sweep meta `bench_tool_image_ref`) — 오프라인 대체: 미리 pull. 측정 컨테이너 자체는 `HF_HUB_OFFLINE=1` · `TRANSFORMERS_OFFLINE=1` 로 돈다(run_bench.sh@434fa6740831 §L380 · 03 §3.4).
- 네트워크가 필요 없는 단계: 서브 배달(`sync_to_sub.sh` — 노드 간 LAN) · 기동 · 측정(가중치 · 토크나이저가 로컬 마운트).
