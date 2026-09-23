# 01 · 산출물 — 무엇이 실제로 쓰였고, 어떻게 다시 띄우는가

## 1.1 적용 판정 표
> 기계가 적용 증거(빌드 원장 · 원장이 없으면 라벨된 재구성) × 파일 × 선언을 대사한 결과다. `artifacts/` 에는 **적용된 것만** 실렸다 — 빌드 때 스스로 skip 된 패치와 쓰이지 않은 평면의 파일은 표에만 남고 싣지 않는다.

<!-- FACT:slots -->
| 슬롯 | 실림 | 파일 | 신호 | 적용 증거 | 사유 |
|---|---|---|---|---|---|
| `build_patch_post` | 실림 | `artifacts/build_patch_post/10-deepgemm.sh` · `artifacts/build_patch_post/20-triton-kernels.sh` · `artifacts/build_patch_post/30-mxfp4-triton-sm121.sh` · `artifacts/build_patch_post/40-humming-nvml-gb10.sh` | 3-signal | build-ledger:attestation:output/multi/benchlog/attestation_nv4-bf-262k-mmp.json#main | 넷 다 **이 모델의 요구가 아니라 공유 이미지의 arch-enablement** 다 — 헤더의 model-trigger 는 DeepSeek-V4(FP8 block-scale · MXFP4 MoE · HUMMING 백엔드)이고, 이 계보의 어떤 벽도 이 넷을 해소 수단으로 쓰지 않았다. - `10-deepgemm.sh`: DeepGEMM nv_dev(a6b593d) 설치. 이 실행 엔진 로그에 `DeepGEMM PDL enabled` · `DeepGEMM E8M0 enabled`(L654 · L655)가 있어 모듈이 적재된 것은 관측됐다 — 이 모델 경로가 DeepGEMM 커널을 실제 호출했는지는 로그가 말하지 않는다. SM120 심볼이 없으면 빌드를 멈춘다(L52-L53). - `20-triton-ker… |
| `build_patch_pre` | 실림 | `artifacts/build_patch_pre/60-qwen4exp-nvfp4-mixed.sh` · `artifacts/build_patch_pre/62-qwen4exp-ple-mmap.sh` · `artifacts/build_patch_pre/64-qwen4exp-qsa-fp8kv.sh` | 3-signal | build-ledger:attestation:output/multi/benchlog/attestation_nv4-bf-262k-mmp.json#main | (a) NVFP4 mixed 체크포인트가 로드되게 하는 세 hunk — A: PLE FP8 라우트(자체 제작) · B1: MTP FP8_PB_WO MoE 디스패치 · B2: draft `quantized_layers` 리맵(B1/B2 = 미머지 PR #55513 이식 · 출처 dolf3131 `patch-nv-mixed.py`). (b) 빼면 W1 이 다시 난다. (c) 이 셀에서 발화했다 — 다만 전용 로그 서명은 없고, 양 rank 의 `Model loading took 39.05 GiB`(`lite_engine` 로그 L620 · L626)와 MTP draft 적재가 간접 증거다. (d) 상류가 같은 수정을 머지하면 트립와이어가 **빌드를 멈춘다**(skip 이 아니다): · (a) PLE n-gra… |
| `build_recipe` | 실림 | `artifacts/build_recipe/Dockerfile.source-build` · `artifacts/build_recipe/requirements.txt` | 3-signal | build-ledger:build-ledger.dockerfile | NGC `nvcr.io/nvidia/pytorch:26.07-py3` 위에서 vLLM `v0.29.0rc6` 을 소스 빌드한다(`TORCH_CUDA_ARCH=12.1a` · `RAY_VERSION=2.48.0` · 이 실행 `BUILD_JOBS=8`). 트랙 이유는 00 §0.4(1). Dockerfile 은 측정 이미지 docker history 의 RUN · COPY 17/17 과 일치한 바이트다. `requirements.txt` 는 워크트리 바이트이며 이미지 안 COPY 대상과의 일치는 관측되지 않았다(1.1 실린 리비전). 이 파일은 v0.28.0 wheel 의 Requires-Dist 를 기준선으로 삼고 flashinfer-python 만 대상 vLLM 선언(0.6.18)으로 양보한 핀 목록이다… |
| `compose` | 실림 | `artifacts/compose/.env.cluster.template` · `artifacts/compose/.env.interconnect.template` · `artifacts/compose/.env.template` · `artifacts/compose/docker-compose.yaml` · `artifacts/compose/serve_runner.sh` · `artifacts/compose/sub_recipe.json` | 2-signal | file:output/multi/docker-compose.yaml | compose 는 master(Ray head + vllm serve)와 slave(Ray worker)를 같은 이미지 · 같은 `serve_runner.sh` 로 띄우고 `NODE_ROLE` 로 가른다. `serve_runner.sh` 는 yaml 에 `distributed-executor-backend` 가 없으면 멈추고(헤더가 기록한 실패 — V2), master 는 `ray status` 의 GPU 합류가 리터럴 `2` 가 될 때까지 기다린다(L79 · 1.5). env 템플릿 셋은 형상이며 실값은 자기 manifest 에서 렌더러로 다시 파생한다. 둘 다 측정 전 마지막 커밋의 바이트다(1.1 실린 리비전). |
| `fork_pin` | 안 실림 | — | 1-signal | none:.claude/policies/arch_variant_ledger.json | 미관측 |
| `runtime_patch` | 안 실림 | — | 1-signal | none:output/multi/configs/nv4-bf-262k-mmp_patch.py | 미관측 |
| `triplet` | 실림 | `artifacts/triplet/.env.nv4-bf-262k-mmp` · `artifacts/triplet/nv4-bf-262k-mmp.sh` · `artifacts/triplet/nv4-bf-262k-mmp.yaml` | 3-signal | file:output/multi/configs/nv4-bf-262k-mmp.yaml | yaml 은 서빙 노브 9개(1.3), 러너는 `arm_patch.sh` 를 source 한 뒤 `vllm serve --config` 를 부르고, env 는 이미지 좌표(`IMAGE_TAG` · `BUILD_DOCKERFILE` · `VLLM_REF` · `BUILD_JOBS`)와 PLE mmap 두 키(`VLLM_PLE_MMAP=1` · `VLLM_PLE_MMAP_DIR`)를 든다. 트리플렛은 2026-09-12 res-min 캠페인의 A1 셀로 저작됐고 이 E2E 가 그대로 재사용했다(yaml 머리 주석). |

**레시피 경고 · 실린 리비전**(기계 관측 — 수신자 빌드가 여기서 갈라질 수 있다)

- `build_recipe` `output/multi/Dockerfile.source-build` 실린 리비전: 리비전 미특정 · 선택 방법 `worktree(측정 이미지 docker history 의 RUN·COPY 와 전수 일치)` · docker history 대조 17/17 · 실린 바이트 `worktree` · 쓰인 바이트임을 관측으로 확인 예
- `build_recipe` `output/multi/requirements.txt` 실린 리비전: 리비전 미특정 · 선택 방법 `worktree(이미지 안 COPY 대상 바이트 미관측 — 이미지 탐침 안 함)` · docker history 대조 없음 · 실린 바이트 `worktree` · 쓰인 바이트임을 관측으로 확인 아니오
- `compose` `.claude/skills/upstream-version-watch/assets/configs/serve_runner.sh` 실린 리비전: 커밋 `50a5ce486738` · 선택 방법 `worktree(mtime 2026-09-06T21:57:53Z ≤ 측정 2026-09-23T00:46:29Z(sweep_index.generated_utc) · 바이트 = 측정 전 마지막 커밋 50a5ce486738)` · docker history 대조 없음 · 실린 바이트 `worktree` · 근거 `mtime≤measured` · 쓰인 바이트임을 관측으로 확인 예
- `compose` `output/multi/docker-compose.yaml` 실린 리비전: 커밋 `26b17f6e560a` · 선택 방법 `worktree(mtime 2026-09-21T20:48:25Z ≤ 측정 2026-09-23T00:46:29Z(sweep_index.generated_utc) · 바이트 = 측정 전 마지막 커밋 26b17f6e560a)` · docker history 대조 없음 · 실린 바이트 `worktree` · 근거 `mtime≤measured` · 쓰인 바이트임을 관측으로 확인 예

**싣지 않은 파일**(슬롯별 사유 — 빌드 때 skip 된 패치는 아래 적용 집합 표에 있다)

- `build_patch_post` known-non-patch 1건: `.gitkeep`
- `build_patch_pre` known-non-patch 3건: `.gitkeep` · `PROVENANCE.json` · `files/`
- `compose` `arm_patch.sh` — `not-armed(output/multi/configs/nv4-bf-262k-mmp_patch.py 없음 · slave CONFIG_FILE='default' 의 _patch.py 없음 — arm_patch.sh 는 no-op)`
<!-- /FACT:slots -->

<!-- FACT:applied_set -->
**적용 집합** — 상태 `observed` · 출처 `attestation:output/multi/benchlog/attestation_nv4-bf-262k-mmp.json#main`

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
| `50-dsv4-sm12x-port.sh` | — | — | — | — | log-token |
| `55-src-deps-authority.sh` | — | — | — | — | log-token |
| `60-qwen4exp-nvfp4-mixed.sh` | — | — | — | — | exit-code |
| `62-qwen4exp-ple-mmap.sh` | — | — | — | — | exit-code |
| `64-qwen4exp-qsa-fp8kv.sh` | — | — | — | — | exit-code |
| `10-deepgemm.sh` | — | — | — | — | exit-code |
| `20-triton-kernels.sh` | — | — | — | — | exit-code |
| `30-mxfp4-triton-sm121.sh` | — | — | — | — | exit-code |
| `40-humming-nvml-gb10.sh` | — | — | — | — | exit-code |
| `strip-hoist` | — | — | — | — | status-file |

**원장 탐침**(순위: attestation → 메인 로컬 이미지 → 재구성)

| 순위 | 결과 | 대상 |
|---|---|---|
| attestation | observed | output/multi/benchlog/attestation_nv4-bf-262k-mmp.json |

> **attestation 범위** — config `nv4-bf-262k-mmp` · phase serve · 작성 2026-09-23T00:13:58Z(output/multi/benchlog/attestation_nv4-bf-262k-mmp.json 파일 mtime(파일시스템 관측 — 문서에 시각 칸이 없다 · 기록 필드 아님)) · 묶은 근거 cell-name · 파일 `output/multi/benchlog/attestation_nv4-bf-262k-mmp.json`: **same config(nv4-bf-262k-mmp) · serve — 이 측정 실행과 같은 실행인지는 미검증(파일은 다음 실행이 덮는다)**
<!-- /FACT:applied_set -->

## 1.2 슬롯별 적용 사유
> 이 절이 답하는 질문: 실린 산출물 하나하나는 무엇을 고치며, 왜 이 셀에 필요했고, 이 셀에서 실제로 발화했으며, 언제 불필요해지는가?

적용 판정의 출처는 이 실행 serve attestation 에 묶인 이미지 안 빌드 원장(`build-ledger` · 관측)이다 — 1.1 의 10항목 중 적용 7 · skip 3 이며 `unobserved` 는 없다. 원장은 양 노드 identity 가 같다(01 §1.5). 판정 근거 표의 스크립트 바이트 정체 · 이미지 사본 칸은 이번 탐침에서 비었다. 대신 발행 전 독립 사실 검증이 원장(`docs/simlog/26092300_hint_publisher_G1G3_E2E/d1_build_identity.json` main)의 패치별 `script_sha256` 과 실린 7파일의 sha256 을 대조했고 7/7 일치했다 — 실린 스크립트 바이트는 이미지에 구워진 바이트와 같다.

### build_recipe — `Dockerfile.source-build` · `requirements.txt`

NGC `nvcr.io/nvidia/pytorch:26.07-py3` 위에서 vLLM `v0.29.0rc6` 을 소스 빌드한다(`TORCH_CUDA_ARCH=12.1a` · `RAY_VERSION=2.48.0` · 이 실행 `BUILD_JOBS=8`). 트랙 이유는 00 §0.4(1). Dockerfile 은 측정 이미지 docker history 의 RUN · COPY 17/17 과 일치한 바이트다. `requirements.txt` 는 워크트리 바이트이며 이미지 안 COPY 대상과의 일치는 관측되지 않았다(1.1 실린 리비전). 이 파일은 v0.28.0 wheel 의 Requires-Dist 를 기준선으로 삼고 flashinfer-python 만 대상 vLLM 선언(0.6.18)으로 양보한 핀 목록이다(파일 머리 L1-L5).

### build_patch_pre — `60-qwen4exp-nvfp4-mixed.sh`

(a) NVFP4 mixed 체크포인트가 로드되게 하는 세 hunk — A: PLE FP8 라우트(자체 제작) · B1: MTP FP8_PB_WO MoE 디스패치 · B2: draft `quantized_layers` 리맵(B1/B2 = 미머지 PR #55513 이식 · 출처 dolf3131 `patch-nv-mixed.py`). (b) 빼면 W1 이 다시 난다. (c) 이 셀에서 발화했다 — 다만 전용 로그 서명은 없고, 양 rank 의 `Model loading took 39.05 GiB`(`lite_engine` 로그 L620 · L626)와 MTP draft 적재가 간접 증거다. (d) 상류가 같은 수정을 머지하면 트립와이어가 **빌드를 멈춘다**(skip 이 아니다):

> [원문] 60-qwen4exp-nvfp4-mixed.sh §L58-63
> def tripwire(path, needle, what):
>     if needle in open(path).read():
>         print(f"{TAG} FAILED — {what}: 상류가 이미 동등 수정을 머지한 것으로 보인다. "
>               f"이 패치(60-qwen4exp-nvfp4-mixed)를 제거하고 stock 경로로 스모크하라.",
>               file=sys.stderr)
>         raise SystemExit(1)

### build_patch_pre — `62-qwen4exp-ple-mmap.sh`

(a) PLE n-gram 테이블(47.7GiB)을 NVMe mmap 으로 서빙하는 경로 — blazux `vllm_ple_mmap.py` 이식 + 우리 트리 어댑테이션 6가지, 그중 TP=2 rank-aware masking + all-reduce 는 자체 설계다. `VLLM_PLE_MMAP` 이 없으면 `apply()` 가 no-op 이다(헤더 gate 절). (b) 빼면 W6 이 다시 난다(PLE 가 상주로 돌아가 선언 weights 가 63,266MiB 가 된다 — 02 §2.5). (c) 이 셀에서 발화했다 — 양 rank 로그 `PLE mmap patch applied to …Qwen4ExpNGramEmbedding`(L448 · L560)과 `PLE mmap: layer 1, 128 shards … tp_world 2`(L539 · L572). (d) mmap 경로가 이미 있으면 **빌드를 멈춘다**:

> [원문] 62-qwen4exp-ple-mmap.sh §L47-51
> # 머지/중복 트립와이어
> if [ -f "$MOD" ] || grep -q 'qwen4_exp_ple_mmap_lookup' "$PLE"; then
>     echo "$TAG FAIL: ple_mmap 경로가 이미 존재한다 — 상류 머지 또는 이전 적용. 이 패치를 제거하고 stock 경로를 확인하라." >&2
>     exit 1
> fi

### build_patch_pre — `64-qwen4exp-qsa-fp8kv.sh`

(a) QSA(Triton) attention 이 fp8_e4m3 KV 를 읽도록 가드 7곳을 넓히고 vLLM 정준 `_cast_kv_tile` 디콴트를 얹는다(출처 blazux `patch_qsa_fp8_kv.py`). (b) **이 셀에는 필요 없다** — 이 셀은 `kv-cache-dtype: auto` 이고 헤더대로 auto/bfloat16 이면 `FP8_KV` constexpr 이 거짓이라 분기가 소거된다. 같은 이미지가 자매 셀(KV fp8)도 서빙하므로 실렸다(이미지 하나가 모든 셀을 서빙한다). (c) 이 셀에서 불활성(적용 ≠ 활성). (d) 상류가 fp8 KV 를 받으면 **빌드를 멈춘다**:

> [원문] 64-qwen4exp-qsa-fp8kv.sh §L44-48
> # 머지 트립와이어: 상류가 fp8 KV 를 이미 받으면 빌드를 멈춰 이 패치 제거를 강제한다.
> if grep -q '"fp8_e4m3"' "$OWN" || grep -q '_cast_kv_tile' "$OPS"; then
>     echo "$TAG FAIL: QSA fp8 KV 경로가 이미 존재한다 — 상류 머지로 보인다. 이 패치를 제거하고 stock 경로로 스모크하라." >&2
>     exit 1
> fi

### build_patch_post — `10-deepgemm.sh` · `20-triton-kernels.sh` · `30-mxfp4-triton-sm121.sh` · `40-humming-nvml-gb10.sh`

넷 다 **이 모델의 요구가 아니라 공유 이미지의 arch-enablement** 다 — 헤더의 model-trigger 는 DeepSeek-V4(FP8 block-scale · MXFP4 MoE · HUMMING 백엔드)이고, 이 계보의 어떤 벽도 이 넷을 해소 수단으로 쓰지 않았다.
- `10-deepgemm.sh`: DeepGEMM nv_dev(a6b593d) 설치. 이 실행 엔진 로그에 `DeepGEMM PDL enabled` · `DeepGEMM E8M0 enabled`(L654 · L655)가 있어 모듈이 적재된 것은 관측됐다 — 이 모델 경로가 DeepGEMM 커널을 실제 호출했는지는 로그가 말하지 않는다. SM120 심볼이 없으면 빌드를 멈춘다(L52-L53).
- `20-triton-kernels.sh`: NGC 번들 standalone `triton_kernels` 제거 → vLLM vendored 판 사용. 이미 vendored 면 skip, 검증 실패는 assert 로 빌드를 멈춘다(L33 · L35). 이 셀의 MTP experts 는 TRITON Fp8 MoE 백엔드를 골랐지만(L614) 그 경로가 이 패치에 의존하는지는 관측되지 않았다.
- `30-mxfp4-triton-sm121.sh`: MXFP4 MoE Triton 게이트를 SM12x 로 넓힌다. 이 체크포인트의 MoE 는 NVFP4(주 experts)와 FP8(MTP)이라 이 게이트를 탈 층이 없다고 본다(추론 — 게이트 발화 로그 없음). 이미 완화돼 있으면 skip, 예상 문자열이 없으면 빌드를 멈춘다(L28 · L32-L34).
- `40-humming-nvml-gb10.sh`: HUMMING MoE 백엔드의 NVML 클록 쿼리 가드. 이 실행 엔진은 NvFp4 에 `FLASHINFER_CUTLASS`, Fp8 에 `TRITON` 을 골랐으므로(L454 · L614) HUMMING 경로는 타지 않았다(불활성). 이미 가드돼 있으면 skip, 예상 블록이 없으면 빌드를 멈춘다(L55 · L60 · L65).

### compose — `docker-compose.yaml` · `serve_runner.sh` · `.env*.template` · `sub_recipe.json`

compose 는 master(Ray head + vllm serve)와 slave(Ray worker)를 같은 이미지 · 같은 `serve_runner.sh` 로 띄우고 `NODE_ROLE` 로 가른다. `serve_runner.sh` 는 yaml 에 `distributed-executor-backend` 가 없으면 멈추고(헤더가 기록한 실패 — V2), master 는 `ray status` 의 GPU 합류가 리터럴 `2` 가 될 때까지 기다린다(L79 · 1.5). env 템플릿 셋은 형상이며 실값은 자기 manifest 에서 렌더러로 다시 파생한다. 둘 다 측정 전 마지막 커밋의 바이트다(1.1 실린 리비전).

### triplet — `nv4-bf-262k-mmp.yaml` · `nv4-bf-262k-mmp.sh` · `.env.nv4-bf-262k-mmp`

yaml 은 서빙 노브 9개(1.3), 러너는 `arm_patch.sh` 를 source 한 뒤 `vllm serve --config` 를 부르고, env 는 이미지 좌표(`IMAGE_TAG` · `BUILD_DOCKERFILE` · `VLLM_REF` · `BUILD_JOBS`)와 PLE mmap 두 키(`VLLM_PLE_MMAP=1` · `VLLM_PLE_MMAP_DIR`)를 든다. 트리플렛은 2026-09-12 res-min 캠페인의 A1 셀로 저작됐고 이 E2E 가 그대로 재사용했다(yaml 머리 주석).

### 싣지 않은 것

- `50-dsv4-sm12x-port.sh` · `55-src-deps-authority.sh`: 빌드 원장 skip(`SM12X_PORT=0` · `SRC_DEPS_AUTHORITY=0` — stock 빌드 경로 불변). 50 의 model-trigger 는 DeepSeek-V4 다.
- inline `strip-hoist`: 원장 skip(`register_opaque_type accepts hoist`).
- `fork_pin`: 없음 — stock 업스트림 `v0.29.0rc6` 이고 변종 원장에 이 셀 항목이 없다.
- `runtime_patch`: 없음 — `nv4-bf-262k-mmp_patch.py` 가 없어 `arm_patch.sh` 는 no-op 이다(master · slave 모두).

## 1.3 값의 지위표
> 서빙 설정의 노브 **전부**와 기계가 모은 후보 지위다(lockset 출처 · yaml 주석 · 엔진 기본값). 최종 지위는 아래 `kind: value-status` 블록이 정한다 — 표의 노브마다 블록이 정확히 하나 있어야 린터를 통과한다.

<!-- FACT:value_status -->
서빙 노브 9개 — 노브마다 `kind: value-status` 블록이 **정확히 1개** 있어야 한다(노브·값은 이 표의 글자 그대로 · 값 칸의 `\|` 는 `|` 로 적는다).

| 노브 | 값 | 후보 지위 | 후보 근거 | yaml 주석 | 출처 |
|---|---|---|---|---|---|
| `tensor-parallel-size` | 2 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `distributed-executor-backend` | ray | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `gpu-memory-utilization` | 0.85 | declared-requirement | lockset 출처 gmu_source=target_gmu · lockset provenance=hand-authored | — | lockset.gmu_source=target_gmu |
| `max-model-len` | 262144 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `kv-cache-dtype` | auto | engine-default | 값 `auto` = 엔진이 고른다 | — | serving-yaml 값 토큰 |
| `kv-cache-memory-bytes` | 21474836480 | inherited | lockset 출처 kv_source=hand · lockset provenance=hand-authored | 20480MiB/노드 | lockset.kv_source=hand |
| `enforce-eager` | true | inherited | 주석 단어 '승계': 승계 통제변인(캡처 스파이크 ~10GiB 회피) | 승계 통제변인(캡처 스파이크 ~10GiB 회피) | serving-yaml 줄 주석 |
| `async-scheduling` | false | inherited | 주석 단어 '필수': MTP+async 금지(0.29 자동비활성 안 탐 — 명시 필수) · yaml 주석만 있음 — 관측된 실패가 있으면 declared-requirement | MTP+async 금지(0.29 자동비활성 안 탐 — 명시 필수) | serving-yaml 줄 주석 |
| `speculative-config` | {"method":"mtp","num_speculative_tokens":3} | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
<!-- /FACT:value_status -->

> 이 절이 답하는 질문: 각 서빙 노브의 값은 이 셀에서 어떤 지위인가 — 조정됐나, 승계됐나, 음성대조로 일부러 둔 것인가, 엔진 기본값인가, 필요조건인가?

```hint-event
id: V1
kind: value-status
노브: tensor-parallel-size
값: 2
지위: inherited
근거: manifest 토폴로지(노드 2 × 노드당 GPU 1)에서 정해진 값 — 이 계보에서 바꿔 잰 기록 0
출처: [plan_26090918_qwen38_flashnext_multi_딥캠페인 §1., docker-compose.yaml]
```

```hint-event
id: V2
kind: value-status
노브: distributed-executor-backend
값: ray
지위: declared-requirement
근거: 없으면 vLLM 이 multiprocessing 경로로 가 'World size > available GPUs (1)' 로 죽는다고 serve_runner.sh 헤더가 기록했고 러너가 그 경우 기동을 멈춘다
출처: [serve_runner.sh]
```

```hint-event
id: V3
kind: value-status
노브: gpu-memory-utilization
값: 0.85
지위: inherited
근거: P0-1 스모크 최종 설정에서 승계 · 이 셀에서 바꿔 잰 기록 0 · kv-cache-memory-bytes 를 준 이 구성에서 엔진은 KV 예약에 gmu 를 따르지 않는다고 로그에 적었다(Q3)
출처: [testlog_26090921_p0-1_nvfp4_mixed_패치_스모크_판정 §패치 중재 세부, docs/simlog/26092300_hint_publisher_G1G3_E2E/d1_sweep/lite_engine_nv4-bf-262k-mmp.log]
```

```hint-event
id: V4
kind: value-status
노브: max-model-len
값: 262144
지위: inherited
근거: 모델 네이티브 컨텍스트(YaRN 없이) · 캠페인 셀 축으로 선언된 통제변인 — 이 셀에서 조정 0
출처: [plan_26090918_qwen38_flashnext_multi_딥캠페인 §3. 셀 매트릭스 (기본 24셀 + 레버 서브스윕)]
```

```hint-event
id: V5
kind: value-status
노브: kv-cache-dtype
값: auto
지위: engine-default
근거: auto = 엔진이 모델 dtype(bf16)을 쓴다 · 이 셀은 64 패치의 fp8 KV 경로를 쓰지 않는다
출처: [plan_26091216_qwen38fn_res최소조건_후속캠페인 §2.4, 64-qwen4exp-qsa-fp8kv.sh]
```

```hint-event
id: V6
kind: value-status
노브: kv-cache-memory-bytes
값: 21474836480
지위: negative-control
근거: 262k bf16 필요량 3,072MiB/노드 대비 약 6.7배 과잉 — res-min 캠페인이 레거시와 같은 KV 20GiB 로 음성대조하려고 일부러 유지한 값(손레버 · lockset kv_source=hand · 스윕 0) · 이 E2E 는 그 트리플렛을 조정 없이 재사용했다
출처: [plan_26091216_qwen38fn_res최소조건_후속캠페인 §2.4, plan_26091216_qwen38fn_res최소조건_후속캠페인 §4. 단계]
```

```hint-event
id: V7
kind: value-status
노브: enforce-eager
값: true
지위: inherited
근거: W5(P0-1 · PLE resident 구성에서 그래프 캡처 스파이크 트립) 이후 승계 통제변인 — 이 셀(mmp)에서 eager 를 끄고 잰 기록 0
출처: [testlog_26090921_p0-1_nvfp4_mixed_패치_스모크_판정 §판정, sweep_map_26091008_nv4-262k-mmp-levers]
```

```hint-event
id: V8
kind: value-status
노브: async-scheduling
값: false
지위: inherited
근거: 외부 선례의 MTP+async 금지 조합을 피하려는 명시(R4) — 우리 계보에서 켜서 실패한 관측은 없다
출처: [testlog_26091001_p0-2_ple_mmap_p0-3_qsa_fp8kv_스모크_판정 §§1. ★ 중재 술어 재설계 — "토큰 정확 일치"는 이 스택에서 도달 불가]
```

```hint-event
id: V9
kind: value-status
노브: speculative-config
값: '{"method":"mtp","num_speculative_tokens":3}'
지위: inherited
근거: 사람 결정(MTP 켠 채 · 2026-09-12)으로 고정된 통제변인 · 계획된 MTP k 스윕은 시도 0(손레버)
출처: [plan_26091216_qwen38fn_res최소조건_후속캠페인 §4. 단계, sweep_map_26091008_nv4-262k-mmp-levers]
```

**요약.** 이 레시피에서 조정해 찾은 값(`tuned`)은 하나도 없다. 복사하면 가장 위험한 값은 V6(KV 20GiB) — 필요량의 약 6.7배로 둔 음성대조값이라 최적도 필요조건도 아니다. V3(gmu 0.85) · V7(eager) · V9(MTP k=3)은 다른 구성 · 사람 결정에서 승계됐고 이 셀에서 대안을 잰 적이 없다. 빼면 깨진다고 기록된 것은 V2(Ray executor)뿐이다. `moe-backend` 는 표에 없다 — 명시하지 않는 것이 요건이다(W3 · W4).

## 1.4 재현 절차
> 셀 실행 기록에서 기계가 파생한 명령 순서와 단계별 소요다. 소요가 `미관측` 인 단계는 시각 기록이 없다는 뜻이다(0 이 아니다). 그 아래는 블랙박스 이벤트 원장에서 이 셀의 행을 기계가 뽑아 기동 시도로 묶은 것이고, 마지막은 측정한 실행의 엔진 로그가 echo 한 env 다(측정 당시 값의 정본).

<!-- FACT:reproduce -->
| 순서 | 단계 | 성공 판정 | 소요 | 관측 구간(UTC) | 소요 출처 | 명령 출처 |
|---|---|---|---|---|---|---|
| 1 | render | 러너 3종·.env·.env.cluster·.env.interconnect 생성(렌더러 fail-loud) | 미관측 | 미관측 | .claude/skills/upstream-version-watch/scripts/multinode_serve_smoke.sh 선행조건 안내 · .claude/skills/upstream-version-watch/scripts/render_dockerfile.py CLI | derived(렌더러 CLI) |
| 2 | build | 빌드 OK · 이미지 실재 | 44분 15초 | 2026-09-22T22:57:18Z → 2026-09-22T23:41:33Z (exact) | campaigns/camp-26092301-hint-v6-e2e/phases/<선언 노드 1>/build.status.json(같은 셀 · 가장 이른 시작 ~ 가장 늦은 끝) | reconstructed(build-ledger build_args + compose build) |
| 3 | serve | health 200 + 추론 1회(스모크 판정) | ≤ 32분 8초(상한) | 2026-09-22T23:42:54Z → 2026-09-23T00:15:02Z (upper) | docs/logs/main/events/2026-09.jsonl budget_declare(label smoke-nv4-bf-262k-mmp) → 첫 측정 date(output/multi/benchlog/sweep_nv4-bf-262k-mmp/lite_cold_nv4-bf-262k-mmp.json · date=컨테이너 시계(UTC 가정)) | derived(스모크 사용법) |
| 4 | bench | 리포트 발행(+PASS 면 인증서) | 31분 27초 | 2026-09-23T00:15:02Z → 2026-09-23T00:46:29Z (exact) | output/multi/benchlog/sweep_nv4-bf-262k-mmp/lite_cold_nv4-bf-262k-mmp.json · date=컨테이너 시계(UTC 가정) date → sweep_index.generated_utc | reconstructed(bench json) |

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
# 동등 경로(스모크 사용법): bash .claude/skills/upstream-version-watch/scripts/multinode_serve_smoke.sh nv4-bf-262k-mmp --build-only
```

**3. serve**

```sh
bash .claude/skills/upstream-version-watch/scripts/multinode_serve_smoke.sh nv4-bf-262k-mmp --keep-up
```

**4. bench**

```sh
# bench JSON 필드로 복원: backend · model_id→--model · tokenizer_id→--tokenizer · num_prompts · max_concurrency · request_rate · burstiness · 요청당 입출력 길이 = total_*_tokens ÷ completed(나누어떨어질 때만 — 평균이다 · 요청별 길이는 JSON 에 없다) · 나누어떨어지면 `--dataset-name random` 으로 추정했다(측정 도구가 그 꼴로 부른다 — 아래 도구 원문).
# JSON 에 없는 인자(--endpoint · --base-url · --ignore-eos · --num-warmups · --temperature · --seed · --random-range-ratio · --trust-remote-code)는 측정 도구가 정했다 — level_* = run_bench.sh · lite_cold/lite_warm = lite_bench.sh(.claude/skills/adversarial-benchmark/scripts/) · 셀 스윕 순서는 sweep_bench.sh.
# output/multi/benchlog/sweep_nv4-bf-262k-mmp/lite_cold_nv4-bf-262k-mmp.json (date 20260923-001502)
vllm bench serve --backend openai-chat --model qwen3.8-flash-next --tokenizer /app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 --dataset-name random --random-input-len 566 --random-output-len 128 --num-prompts 1 --max-concurrency 1 --request-rate inf
# output/multi/benchlog/sweep_nv4-bf-262k-mmp/lite_warm_nv4-bf-262k-mmp.json (date 20260923-001539)
vllm bench serve --backend openai-chat --model qwen3.8-flash-next --tokenizer /app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 --dataset-name random --random-input-len 565 --random-output-len 128 --num-prompts 3 --max-concurrency 1 --request-rate inf
```

**기동 성공 판정(관측 · 발행 자격)**: health 200 = true · 추론 1회 = true · 근거 `output/multi/benchlog/serve_proof_nv4-bf-262k-mmp.json`
<!-- /FACT:reproduce -->

<!-- FACT:event_timeline -->
**블랙박스 이벤트 43행**(원장에서 이 셀 label·config 에 맞는 행만 · 시각순 · 기계 발췌)

| # | UTC | 노드 | kind | label | 내용 | 출처 |
|---|---|---|---|---|---|---|
| 1 | 2026-09-10T02:04:20Z | 미관측 | `measurement` | `measurement (no budget event recorded)` | 같은 셀의 다른 측정(측정 노드 축 cluster) · 시각 = 측정 조립(sweep generated_utc = 인증서 measured_utc · 시작 시각 미기록) · 이 발행의 측정 2026-09-23T00:46:29Z 전 · 발행 기록 qwen38fn_full_nv4_bf_262k_mmp(셀 대조 simlog:sweep_index.meta.config_name · verdict PASS) · simlog 사본 generated_utc 2026-09-10T02:04:20Z(같음) · 원장: 이 시각을 관측한 원장(main, sub)의 어느 라벨의 예산 선언 창도 이 시각을 덮지 않는다 — 예산 선언 없이 잰 측정 · 직전 예산 행 budget_none 2026-09-09T22:49:33Z(docs/logs/main/events/watchdog.jsonl:367) · 직후 예산 행 budget_clear 2026-09-10T02:06:56Z(docs/logs/main/events/2026-09.jsonl:640) | docs/_evidence/qwen38fn_full_nv4_bf_262k_mmp.json:4 |
| 2 | 2026-09-12T07:43:46Z | main | `budget_declare` | `smoke-nv4-bf-262k-mmp` | 예산 선언(선언값 — 측정 아님) · floor_mib=40478 · weights_mib=38852 · kv_mib=20480 · overhead_mib=24800 · mem_total_mib=124610 · 기동 창 1/3: budget_renew 0회 · 닫힘 budget_clear 2026-09-12T08:14:38Z(선언 후 30m52s) · measured_utc 2026-09-23T00:46:29Z 는 이 창 밖(다른 기동 · 측정 전) | docs/logs/main/events/2026-09.jsonl:727 |
| 3 | 2026-09-12T07:43:46Z | main | `budget_honored` | `smoke-nv4-bf-262k-mmp` | budget_honored · floor_mib=40478 · arm_ceiling_mib=37406 · remaining_s=27000 · 창 시작 2026-09-12T07:43:46Z 후 0m00s | docs/logs/main/events/watchdog.jsonl:432 |
| 4 | 2026-09-12T07:45:09Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=67092 · rate_mib_s=24638 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → smoke-nv4-bf-262k-mmp 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-12T07:43:46Z 후 1m23s) | docs/logs/main/events/watchdog.jsonl:433 |
| 5 | 2026-09-12T08:11:38Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=45202 · rate_mib_s=21111 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → smoke-nv4-bf-262k-mmp 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-12T07:43:46Z 후 27m52s) | docs/logs/main/events/watchdog.jsonl:434 |
| 6 | 2026-09-12T08:14:38Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-nv4-bf-262k-mmp 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-12T07:43:46Z 후 30m52s) | docs/logs/main/events/2026-09.jsonl:728 |
| 7 | 2026-09-12T08:14:38Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-nv4-bf-262k-mmp 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-12T07:43:46Z 후 30m52s) | docs/logs/main/events/watchdog.jsonl:435 |
| 8 | 2026-09-12T18:54:37Z | main | `budget_declare` | `smoke-nv4-bf-262k-mmp` | 예산 선언(선언값 — 측정 아님) · floor_mib=40478 · weights_mib=38852 · kv_mib=20480 · overhead_mib=24800 · mem_total_mib=124610 · 기동 창 2/3: budget_renew 1회 · 닫힘 budget_clear 2026-09-12T19:34:53Z(선언 후 40m16s) · measured_utc 2026-09-23T00:46:29Z 는 이 창 밖(다른 기동 · 측정 전) | docs/logs/main/events/2026-09.jsonl:754 |
| 9 | 2026-09-12T18:54:38Z | main | `budget_honored` | `smoke-nv4-bf-262k-mmp` | budget_honored · floor_mib=40478 · arm_ceiling_mib=37406 · remaining_s=8999 · 창 시작 2026-09-12T18:54:38Z 후 0m00s | docs/logs/main/events/watchdog.jsonl:445 |
| 10 | 2026-09-12T18:56:04Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=67501 · rate_mib_s=23745 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → smoke-nv4-bf-262k-mmp 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-12T18:54:38Z 후 1m26s) | docs/logs/main/events/watchdog.jsonl:446 |
| 11 | 2026-09-12T19:24:49Z | main | `budget_renew` | `smoke-nv4-bf-262k-mmp` | budget_renew · floor_mib=40478 · ttl_s=9000 · remaining_before_s=7188 · 창 시작 2026-09-12T18:54:37Z 후 30m12s | docs/logs/main/events/2026-09.jsonl:755 |
| 12 | 2026-09-12T19:32:52Z | 미관측 | `measurement` | `measurement (budget window recorded)` | 같은 셀의 다른 측정(측정 노드 축 cluster) · 시각 = 측정 조립(sweep generated_utc = 인증서 measured_utc · 시작 시각 미기록) · 이 발행의 측정 2026-09-23T00:46:29Z 전 · 발행 기록 camp26091216_bench_nv4_bf_262k_mmp(셀 대조 simlog:sweep_index.meta.config_name · verdict PASS) · simlog 사본 generated_utc 2026-09-12T19:32:52Z(같음) · 원장: 이 셀 선언 창 안: smoke-nv4-bf-262k-mmp 2026-09-12T18:54:37Z~2026-09-12T19:34:53Z (docs/logs/main/events/2026-09.jsonl:754) · smoke-nv4-bf-262k-mmp 2026-09-12T18:54:38Z~2026-09-12T19:34:54Z (docs/logs/main/events/watchdog.jsonl:445) · 이 시각을 관측하지 않은 원장: sub(기록 2026-09-03T09:51:45Z~2026-09-11T11:10:38Z) · 직전 예산 행 budget_renew(smoke-nv4-bf-262k-mmp) 2026-09-12T19:24:49Z(docs/logs/main/events/2026-09.jsonl:755) · 직후 예산 행 budget_clear 2026-09-12T19:34:53Z(docs/logs/main/events/2026-09.jsonl:756) | docs/_evidence/camp26091216_bench_nv4_bf_262k_mmp.json:4 |
| 13 | 2026-09-12T19:34:53Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-nv4-bf-262k-mmp 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-12T18:54:37Z 후 40m16s) | docs/logs/main/events/2026-09.jsonl:756 |
| 14 | 2026-09-12T19:34:54Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-nv4-bf-262k-mmp 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-12T18:54:38Z 후 40m16s) | docs/logs/main/events/watchdog.jsonl:447 |
| 15 | 2026-09-22T23:42:54Z | main | `budget_declare` | `smoke-nv4-bf-262k-mmp` | 예산 선언(선언값 — 측정 아님) · floor_mib=40476 · weights_mib=38852 · kv_mib=20480 · overhead_mib=24800 · mem_total_mib=124608 · 기동 창 3/3: budget_renew 16회 · 닫힘 budget_clear 2026-09-23T00:48:05Z(선언 후 1h05m11s) · measured_utc 2026-09-23T00:46:29Z 가 이 창 안(이 측정의 기동) | docs/logs/main/events/2026-09.jsonl:947 |
| 16 | 2026-09-22T23:42:54Z | main | `budget_honored` | `smoke-nv4-bf-262k-mmp` | budget_honored · floor_mib=40476 · arm_ceiling_mib=37404 · remaining_s=13500 · 창 시작 2026-09-22T23:42:54Z 후 0m00s | docs/logs/main/events/watchdog.jsonl:495 |
| 17 | 2026-09-22T23:44:44Z | cluster | `engine_log_window_start` | — | 캡처된 엔진 로그의 첫 시각(꼬리 캡처일 수 있다 — 엔진 시작 시각이 아닐 수 있다) · 선언 2026-09-22T23:42:54Z 후 1m50s · 시계: 컨테이너 시각을 UTC 로 읽어 측정 창 [2026-09-22T23:42:54Z, 2026-09-23T00:48:05Z] 안에 드는 것을 확인 | output/multi/benchlog/sweep_nv4-bf-262k-mmp/lite_engine_nv4-bf-262k-mmp.log:355 |
| 18 | 2026-09-22T23:44:47Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=68771 · rate_mib_s=22610 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → smoke-nv4-bf-262k-mmp 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-22T23:42:54Z 후 1m53s) | docs/logs/main/events/watchdog.jsonl:496 |
| 19 | 2026-09-22T23:44:48Z | cluster | `engine_prefetch_disabled` | — | 가중치 auto-prefetch 꺼짐 줄 ×4 — filesystem CIFS(vLLM 로그 문구) | output/multi/benchlog/sweep_nv4-bf-262k-mmp/lite_engine_nv4-bf-262k-mmp.log:458 |
| 20 | 2026-09-22T23:57:40Z | cluster | `engine_weights_loaded` | — | 가중치 적재 완료 줄 ×4(rank·단계별) · 최장 402.68s · 이 행 = 마지막 줄 | output/multi/benchlog/sweep_nv4-bf-262k-mmp/lite_engine_nv4-bf-262k-mmp.log:623 |
| 21 | 2026-09-22T23:57:48Z | cluster | `engine_model_loaded` | — | 모델 적재 완료 줄 ×2(rank 별) · 마지막 줄 39.05 GiB · 782.8s | output/multi/benchlog/sweep_nv4-bf-262k-mmp/lite_engine_nv4-bf-262k-mmp.log:626 |
| 22 | 2026-09-22T23:58:50Z | cluster | `engine_shm_broadcast_wait` | — | shm_broadcast 대기 경고(60s 단위) ×14 · 2026-09-22T23:58:50Z–2026-09-23T00:12:23Z(13m33s) — vLLM 문구상 프로세스 hang 또는 긴 작업(컴파일·가중치/KV 양자화) 중 출현 | output/multi/benchlog/sweep_nv4-bf-262k-mmp/lite_engine_nv4-bf-262k-mmp.log:639 |
| 23 | 2026-09-23T00:11:21Z | cluster | `engine_kv_cache_sized` | — | GPU KV cache 1,343,310 tokens · 요청당 262,144 tokens 기준 최대 동시성 5.12x | output/multi/benchlog/sweep_nv4-bf-262k-mmp/lite_engine_nv4-bf-262k-mmp.log:664 |
| 24 | 2026-09-23T00:12:53Z | cluster | `engine_init_done` | — | 엔진 초기화(profile·KV 할당·warmup) 904.30s | output/multi/benchlog/sweep_nv4-bf-262k-mmp/lite_engine_nv4-bf-262k-mmp.log:708 |
| 25 | 2026-09-23T00:13:37Z | cluster | `server_starting` | — | API 서버 기동 — 이 측정 창 선언 2026-09-22T23:42:54Z 후 30m43s · 캡처 로그 첫 시각 2026-09-22T23:44:44Z 후 28m53s | output/multi/benchlog/sweep_nv4-bf-262k-mmp/lite_engine_nv4-bf-262k-mmp.log:735 |
| 26 | 2026-09-23T00:13:58Z | main | `budget_renew` | `smoke-nv4-bf-262k-mmp` | budget_renew · floor_mib=40476 · ttl_s=13500 · remaining_before_s=11636 · 창 시작 2026-09-22T23:42:54Z 후 31m04s | docs/logs/main/events/2026-09.jsonl:948 |
| 27 | 2026-09-23T00:15:40Z | main | `budget_renew` | `smoke-nv4-bf-262k-mmp` | budget_renew · floor_mib=40476 · ttl_s=7200 · remaining_before_s=13398 · 창 시작 2026-09-22T23:42:54Z 후 32m46s | docs/logs/main/events/2026-09.jsonl:949 |
| 28 | 2026-09-23T00:19:33Z | main | `budget_renew` | `smoke-nv4-bf-262k-mmp` | budget_renew · floor_mib=40476 · ttl_s=7200 · remaining_before_s=6967 · 창 시작 2026-09-22T23:42:54Z 후 36m39s | docs/logs/main/events/2026-09.jsonl:950 |
| 29 | 2026-09-23T00:23:08Z | main | `budget_renew` | `smoke-nv4-bf-262k-mmp` | budget_renew · floor_mib=40476 · ttl_s=7200 · remaining_before_s=6985 · 창 시작 2026-09-22T23:42:54Z 후 40m14s | docs/logs/main/events/2026-09.jsonl:951 |
| 30 | 2026-09-23T00:26:40Z | main | `budget_renew` | `smoke-nv4-bf-262k-mmp` | budget_renew · floor_mib=40476 · ttl_s=7200 · remaining_before_s=6988 · 창 시작 2026-09-22T23:42:54Z 후 43m46s | docs/logs/main/events/2026-09.jsonl:952 |
| 31 | 2026-09-23T00:29:05Z | main | `budget_renew` | `smoke-nv4-bf-262k-mmp` | budget_renew · floor_mib=40476 · ttl_s=7200 · remaining_before_s=7055 · 창 시작 2026-09-22T23:42:54Z 후 46m11s | docs/logs/main/events/2026-09.jsonl:953 |
| 32 | 2026-09-23T00:31:25Z | main | `budget_renew` | `smoke-nv4-bf-262k-mmp` | budget_renew · floor_mib=40476 · ttl_s=7200 · remaining_before_s=7060 · 창 시작 2026-09-22T23:42:54Z 후 48m31s | docs/logs/main/events/2026-09.jsonl:954 |
| 33 | 2026-09-23T00:33:56Z | main | `budget_renew` | `smoke-nv4-bf-262k-mmp` | budget_renew · floor_mib=40476 · ttl_s=7200 · remaining_before_s=7049 · 창 시작 2026-09-22T23:42:54Z 후 51m02s | docs/logs/main/events/2026-09.jsonl:955 |
| 34 | 2026-09-23T00:35:42Z | main | `budget_renew` | `smoke-nv4-bf-262k-mmp` | budget_renew · floor_mib=40476 · ttl_s=7200 · remaining_before_s=7094 · 창 시작 2026-09-22T23:42:54Z 후 52m48s | docs/logs/main/events/2026-09.jsonl:956 |
| 35 | 2026-09-23T00:37:24Z | main | `budget_renew` | `smoke-nv4-bf-262k-mmp` | budget_renew · floor_mib=40476 · ttl_s=7200 · remaining_before_s=7098 · 창 시작 2026-09-22T23:42:54Z 후 54m30s | docs/logs/main/events/2026-09.jsonl:957 |
| 36 | 2026-09-23T00:39:12Z | main | `budget_renew` | `smoke-nv4-bf-262k-mmp` | budget_renew · floor_mib=40476 · ttl_s=7200 · remaining_before_s=7092 · 창 시작 2026-09-22T23:42:54Z 후 56m18s | docs/logs/main/events/2026-09.jsonl:958 |
| 37 | 2026-09-23T00:40:34Z | main | `budget_renew` | `smoke-nv4-bf-262k-mmp` | budget_renew · floor_mib=40476 · ttl_s=7200 · remaining_before_s=7118 · 창 시작 2026-09-22T23:42:54Z 후 57m40s | docs/logs/main/events/2026-09.jsonl:959 |
| 38 | 2026-09-23T00:41:52Z | main | `budget_renew` | `smoke-nv4-bf-262k-mmp` | budget_renew · floor_mib=40476 · ttl_s=7200 · remaining_before_s=7122 · 창 시작 2026-09-22T23:42:54Z 후 58m58s | docs/logs/main/events/2026-09.jsonl:960 |
| 39 | 2026-09-23T00:43:10Z | main | `budget_renew` | `smoke-nv4-bf-262k-mmp` | budget_renew · floor_mib=40476 · ttl_s=7200 · remaining_before_s=7122 · 창 시작 2026-09-22T23:42:54Z 후 1h00m16s | docs/logs/main/events/2026-09.jsonl:961 |
| 40 | 2026-09-23T00:44:18Z | main | `budget_renew` | `smoke-nv4-bf-262k-mmp` | budget_renew · floor_mib=40476 · ttl_s=7200 · remaining_before_s=7132 · 창 시작 2026-09-22T23:42:54Z 후 1h01m24s | docs/logs/main/events/2026-09.jsonl:962 |
| 41 | 2026-09-23T00:45:24Z | main | `budget_renew` | `smoke-nv4-bf-262k-mmp` | budget_renew · floor_mib=40476 · ttl_s=7200 · remaining_before_s=7134 · 창 시작 2026-09-22T23:42:54Z 후 1h02m30s | docs/logs/main/events/2026-09.jsonl:963 |
| 42 | 2026-09-23T00:48:05Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-nv4-bf-262k-mmp 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-22T23:42:54Z 후 1h05m11s) | docs/logs/main/events/2026-09.jsonl:964 |
| 43 | 2026-09-23T00:48:06Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-nv4-bf-262k-mmp 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-22T23:42:54Z 후 1h05m12s) | docs/logs/main/events/watchdog.jsonl:497 |

**기동 시도**(예산 선언 `budget_declare` 마다 한 시도 · 같은 노드의 이어지는 행과 선언 없는 노드(엔진 로그)의 그 시각 행을 묶었다 · 사살 · 트립 · 거부는 닫힘과 따로 센다 · `measurement` 행은 측정 기록이라 시도로 세지 않는다(위 표에만) — 해석은 저작자 몫)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 1 | main | `smoke-nv4-bf-262k-mmp` | 2026-09-12T07:43:46Z | 없음 | 없음 | `budget_clear` 2026-09-12T08:14:38Z | 4 |
| 2 | main | `smoke-nv4-bf-262k-mmp` | 2026-09-12T18:54:37Z | 1회 · 첫 2026-09-12T19:24:49Z | 없음 | `budget_clear` 2026-09-12T19:34:53Z | 3 |
| 3 | main | `smoke-nv4-bf-262k-mmp` | 2026-09-22T23:42:54Z | 16회 · 첫 2026-09-23T00:13:58Z | 없음 | `budget_clear` 2026-09-23T00:48:05Z | 11 |
<!-- /FACT:event_timeline -->

<!-- FACT:measurement_env -->
**측정 실행의 env 관측 8행**(측정한 실행의 엔진 로그가 echo 한 값 · 기계 발췌 — 측정 당시 값의 정본이다. 표에 없는 키는 로그가 echo 하지 않은 것이지 설정되지 않았다는 뜻이 아니다 · 측정 뒤 재생성된 env 형상은 01 §1.1 을 본다)

| # | 키 | 값 | 노드 | 종류 | 출처(첫 줄) | 관측 |
|---|---|---|---|---|---|---|
| 1 | `NCCL_SOCKET_IFNAME` | `<nic:cluster>` | sub | env-echo | output/multi/benchlog/sweep_nv4-bf-262k-mmp/lite_engine_nv4-bf-262k-mmp.log:L463 | 로그 8개 · Ray 접힘 +2(접힌 사본의 노드 미관측) |
| 2 | `NCCL_SOCKET_IFNAME` | `<nic:cluster>` | sub | param-echo | output/multi/benchlog/sweep_nv4-bf-262k-mmp/lite_engine_nv4-bf-262k-mmp.log:L464 | 로그 8개 |
| 3 | `nccl:version` | `2.30.7+cuda13.3` | sub | runtime | output/multi/benchlog/sweep_nv4-bf-262k-mmp/lite_engine_nv4-bf-262k-mmp.log:L467 | 로그 8개 |
| 4 | `NCCL_NET_PLUGIN` | `spcx` | sub | env-echo | output/multi/benchlog/sweep_nv4-bf-262k-mmp/lite_engine_nv4-bf-262k-mmp.log:L469 | 로그 8개 |
| 5 | `NCCL_IB_DISABLE` | `1` | sub | env-echo | output/multi/benchlog/sweep_nv4-bf-262k-mmp/lite_engine_nv4-bf-262k-mmp.log:L482 | 로그 8개 · Ray 접힘 +2(접힌 사본의 노드 미관측) |
| 6 | `nccl:network` | `Socket` | main | runtime | output/multi/benchlog/sweep_nv4-bf-262k-mmp/lite_engine_nv4-bf-262k-mmp.log:L494 | 로그 8개 · Ray 접힘 +3(접힌 사본의 노드 미관측) |
| 7 | `NCCL_DMABUF_ENABLE` | `0` | sub | env-echo | output/multi/benchlog/sweep_nv4-bf-262k-mmp/lite_engine_nv4-bf-262k-mmp.log:L496 | 로그 8개 |
| 8 | `NCCL_CROSS_NIC` | `1` | main | env-echo | output/multi/benchlog/sweep_nv4-bf-262k-mmp/lite_engine_nv4-bf-262k-mmp.log:L499 | 로그 8개 |

- 값 주: 기계 치환(pii.substitution_table · 장치·호스트·주소는 자리표시 — 원문은 출처 줄)

- Ray 접힘: 출처 줄 끝의 `[repeated Nx across cluster]` — Ray 가 다른 프로세스의 같은 줄 N개를 한 줄로 접었다. 노드 칸은 **첫 줄**의 노드이고, 접힌 사본이 어느 노드의 것인지는 로그에 없다(관측 대상 밖).

- 로그 창: `tail-800(lite_bench.sh@434fa6740831 L159)`(이 표의 출처 로그 1개) — 이 캡처는 엔진 출력의 **마지막 N줄**이다(측정 도구의 `tail -N` 상한에 닿았다 · 생산자 evidence `log_window`). 창 밖에서 echo 한 줄은 이 표에 없다 — 표에 없는 키 · 한 노드에만 있는 키는 창 밖이었을 수 있다.
<!-- /FACT:measurement_env -->

> 이 절이 답하는 질문: 수신자가 이 zip 만으로 이 셀을 다시 띄우려면 어떤 순서로 무엇을 하고, 각 단계의 성공을 무엇으로 판정하며, 얼마나 기다려야 하는가?

**재현 표 읽는 법.** serve 행(≤ 32분 8초 · 상한)은 예산 선언부터 첫 측정(lite cold)까지이고 bench 행(31분 27초)은 그 뒤 스윕 조립까지다. bench 단계에 재구성된 명령은 lite 레그 `vllm bench serve` 두 줄뿐이다 — 동시성 레벨 레그는 GuideLLM 으로 쟀으며 bench JSON 에서 재구성된 명령이 없다. 레벨 레그의 도구 원문은 03 §3.4 에 있다.

**(a) 사전 준비.**
- 가중치: 양 노드의 **같은 경로**에 `<manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4`(NAS · 컨테이너 `/app/quant_models/…`). 엔진 로그는 이 파일시스템을 CIFS · 체크포인트 123.57 GiB 로 보고 가중치 auto-prefetch 를 껐다(`lite_engine` 로그 L615-L616) — 이 실행의 가중치 적재는 rank 별 374.35s · 402.68s 였다(L623 · L617).
- PLE 스테이징: 양 노드 **로컬 NVMe** 의 같은 자리 `<manifest.ple_mmap_host_path>/q38fn-nvfp4/` 에 PLE safetensors + index(노드당 53.7GB · testlog_26091001 §4). compose 가 이것을 `/app/ple_mmap` 에 읽기 전용으로 붙이고 트리플렛 env 의 `VLLM_PLE_MMAP_DIR=/app/ple_mmap/q38fn-nvfp4` 가 가리킨다. NAS 위에서 mmap 하지 않는다(02 §2.6).
- 필수 env 의 자리: 트리플렛 `.env.nv4-bf-262k-mmp`(이미지 좌표 · `VLLM_PLE_MMAP=1` · `VLLM_PLE_MMAP_DIR`) · `compose/.env.template`(마운트 원천 4개) · `.env.cluster.template`(노드 주소 · Ray) · `.env.interconnect.template`(NCCL). 템플릿은 형상이고 실값은 자기 manifest 에서 1단계 렌더 명령으로 다시 만든다.
- 측정 당시 NCCL 실효값은 위 '측정 실행의 env 관측' 표가 정본이다 — `NCCL_IB_DISABLE=1` · `NCCL_DMABUF_ENABLE=0` · `NCCL_CROSS_NIC=1` 은 interconnect 템플릿과 같고, 템플릿에 없는 `NCCL_NET_PLUGIN=spcx` 가 로그에 echo 됐다(어디서 설정됐는지는 관측 밖). 1.1 '싣지 않은 파일' 에 측정 뒤 재생성된 env 형상은 없다.

**(b) 빌드.** 양 노드에서 각각 병렬로 짓는다(이미지 전송 ✗). 선택자 `Dockerfile.source-build` · build-arg 는 위 2단계 명령과 같다(`BUILD_JOBS=8` 은 양 노드 동일해야 한다 — 1.5). 이 실행의 관측 소요는 44분 15초(위 표 2행 · 캠페인 phase 기록)이고 devlog 는 "약 44분" 으로 적었다(devlog_26092300 §4). 원장은 노드마다 10항목 중 적용 7 · skip 3 이다.

**(c) 기동 순서와 성공 판정.** 순서 = 양 노드 예산 선언(declare → honored) → master up(Ray head) → slave up(Ray worker · SSH) → master 가 GPU 합류 2 를 확인 → `vllm serve`. 이 실행의 시각(원장 · 엔진 로그 — 위 이벤트 표 15~25행):
- 2026-09-22T23:42:54Z 예산 선언 · honored.
- 23:44:47Z `watchdog_highrate_hold`(메모리 감소율 22,610MiB/s 가 상한 20,768 을 넘어 `legacy_rule_would_trip=true` 로 적혔지만 hold 로 끝났다) — 무서워 보이지만 사살 행은 없다. 같은 hold 가 2026-09-12 두 시도의 적재 초기에도 있었다(이벤트 표 4·10행).
- 23:57:40Z 가중치 적재 완료(최장 402.68s) · 23:57:48Z `Model loading took 39.05 GiB`(rank 별).
- 23:58:50Z ~ 00:12:23Z `shm_broadcast` 대기 경고 ×14(13분 33초) — 모델 적재 뒤 초기화 구간이다. 엔진 로그는 인코더 캐시 초기화 줄(23:57:54) 뒤 00:11:10 까지 진행 줄이 거의 없어 그 사이 무엇을 했는지는 말하지 않지만, 서버는 뒤이어 떴다. 벽이 아니다.
- 00:11:21Z KV 1,343,310 토큰 · 00:12:53Z 엔진 초기화 904.30s · 00:13:37Z API 서버 기동(선언 후 30분 43초).
- 스모크 판정: READY 약 1,835초 뒤 health 200 + chat 추론 1회(content_len 147 · finish_reason stop) PASS(devlog_26092300 §4 · `serve_proof`). 패치 발화 서명은 62 의 `PLE mmap patch applied` · `PLE mmap: layer 1, 128 shards … tp_world 2`(1.2).
- 기동 시도는 표대로 3이다 — 시도 1·2 는 2026-09-12 이전 캠페인의 것이고 이 실행은 시도 3 하나다(02 §2.2). 시도 3 은 측정 뒤 `multinode_serve_smoke.sh nv4-bf-262k-mmp --down` 으로 회수돼 00:48:05Z `budget_clear` 로 닫혔다(compose 를 직접 내리지 않았다) · 회수 뒤 양 노드 컨테이너 · GPU 프로세스 0(testlog_26092300 §3).

**(d) 호스트 메모리 안전 예산.** 선언(측정 아님): weights 38,852 · kv 20,480 · overhead 24,800 · MemTotal 124,608 → 바닥 40,476 · 워치독 arm 상한 37,404MiB(이벤트 표 15·16행). 산식은 `floor = MemTotal − weights − kv − overhead` 이고 weights 는 `(ckpt − PLE) ÷ tp` 다(02 §2.5). 실측 나란히: 엔진 `Model loading took 39.05 GiB`(rank 별) · lite 지표의 엔진 예약 합 59.05 GiB · 메인 시스템 RAM 80.0 GiB(66% · bench_report_26092309 lite 표). 선언 없이(compose 직접) 띄우면 워치독 상한이 무한대가 된다 — 스모크 스크립트 경로로만 띄운다.

**(e) 측정 명령.** 위 bench 단계의 두 줄은 lite 레그(cold · warm)를 bench JSON 에서 재구성한 것이다(실행 원문 아님). 레벨 레그(GuideLLM)의 도구 원문은 03 §3.4 를 본다.

**(f) 엔진이 스스로 고른 런타임 경로.**
- 통신: NCCL `Socket`(IB 비활성) · spcx 플러그인 적재 · `SymmMemCommunicator` 는 capability 12.1 미지원 · FlashInfer all-reduce 는 world_size=2 미지원으로 꺼짐 · custom all-reduce 는 MNNVL 부재로 꺼짐(`lite_engine` 로그 L355-L357) — 이 HW 조합에서 정상.
- MoE: 주 experts `FLASHINFER_CUTLASS` NvFp4 · MTP experts `TRITON` Fp8(L454 · L614). Fp8 fused_moe 에 대해 "Using default MoE config. Performance might be sub-optimal!"(GB10 용 E=512,N=320 fp8 튜닝 파일 없음 · L656) 이 떴다 — 이 경고가 decode 에 얼마를 먹는지는 잰 적이 없다(02 Q1 · Q4 의 후보일 뿐 원인으로 확인되지 않았다).
- eager 라 CUDA 그래프 · torch.compile 이 꺼졌고(L578 · L582), spec decoding 설정 때문에 `max_num_scheduled_tokens` 가 2048 로 정해졌다는 경고(L581) · MTP draft 가 외부 멀티모달 임베딩을 받지 않아 텍스트 전용 draft 입력을 쓴다는 경고(L619) · mtp 의 KV 그룹을 특정하지 못해 Mamba 그룹까지 draft 그룹으로 취급한다는 경고(L658)가 있다. 셋 다 기동을 막지 않았고, 성능 영향은 관측되지 않았다.

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
| `BUILD_JOBS` | build_tuning | cell_env | 2026-08-15: 이미지 정체성은 아니지만 양 노드에 같아야 한다 — slave 만 기본 16 으로 컴파일하면 거기서 OOM(하드다운 #2 가 서브였다). |
| `NAS_MODEL_PATH` | mount | project_env | 결함#2b(plan_26070119): --env-file 을 쓰면 프로젝트 .env 자동 로드가 꺼져 compose 기본 마운트로 폴백 → 모델 부재(serve 즉사). 셸 env 로 명시 주입한다. |
| `QUANT_MODEL_PATH` | mount | project_env | 결함#2b 와 같은 경로 — 양자화 모델 마운트원. |
| `TIKTOKEN_HOST_PATH` | mount | project_env | 결함#2b 와 같은 경로 — 오프라인 토크나이저 마운트원. |
| `PLE_MMAP_HOST_PATH` | mount | project_env | 2026-09-09(camp-26090918): PLE mmap 로컬 NVMe 스테이징 마운트원 — slave 도 같은 경로에 실재해야 한다. |
| `SLAVE_CONTAINER_NAME` | cluster | cell_env | 2026-08-14: 폴백 이름이면 slave 협역 워치독 필터가 매칭 0 — 서브에서만 계층 2층이 조용히 사라진다(devlog_26080212 ⑥ 부류 · 하드다운 #2 는 서브였다). 2026-08-02 에 이미 '엉뚱한 이름으로 떠 있었고 아무도 몰랐다' 로 한 번 드러났다(그때는 마스터측만 교정). |
| `VLLM_PLE_MMAP` | serve_env | cell_env | 2026-09-09(62-qwen4exp-ple-mmap): Ray 워커도 가중치를 올린다 — 마스터만 켜면 slave 는 상주 로드로 OOM/불일치. |
| `VLLM_PLE_MMAP_DIR` | serve_env | cell_env | 2026-09-09: 워커 로컬 mmap 스테이징 경로 — 마스터와 같은 값이어야 한다. |

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
| `observed` | {"build_ledger": {"compared": "dockerfile · build_args(− exempt) · patches(phase,file,script_sha256,result) · vllm git sha", "identity_exempt_build_args": ["BUILD_JOBS"], "main_identity_sha256": "fcc9de2eb54ffb0ba1e49089dc9584774f390126b2503ffc84d8014cd836e413", "sub_identity_sha256": "fcc9de2eb54ffb0ba1e49089dc9584774f390126b2503ffc84d8014cd836e413", "verdict": "equal"}, "driver": {"main": "580.178.04", "sub": "580.178.04", "verdict": "equal"}, "torch": {"main": "2.13.0a0+9186a08b2c.nv26.7.59513937", "sub": "2.13.0a0+9186a08b2c.nv26.7.59513937", "verdict": "equal"}, "vllm_sha": {"main": "74c96922ecb9017f413318c76d1af83aa2ab45a5", "sub": "74c96922ecb9017f413318c76d1af83aa2ab45a5", "verdict": "equal"}} |
| `source` | output/multi/benchlog/attestation_nv4-bf-262k-mmp.json |

> **attestation 범위** — config `nv4-bf-262k-mmp` · phase serve · 작성 2026-09-23T00:13:58Z(output/multi/benchlog/attestation_nv4-bf-262k-mmp.json 파일 mtime(파일시스템 관측 — 문서에 시각 칸이 없다 · 기록 필드 아님)) · 묶은 근거 cell-name · 파일 `output/multi/benchlog/attestation_nv4-bf-262k-mmp.json`: **same config(nv4-bf-262k-mmp) · serve — 이 측정 실행과 같은 실행인지는 미검증(파일은 다음 실행이 덮는다)**
<!-- /FACT:sub_recipe -->

<!-- FACT:measurement_env_nodes -->
**노드별 측정 env**(01 §1.4 표를 키 × 노드로 접은 것 · `만 관측` = 다른 쪽 줄이 캡처 로그에 없다 · Ray 가 같은 줄을 접은 키(`[repeated Nx across cluster]`)와 꼬리 캡처 로그(01 §1.4 로그 창)에서 한쪽만 남은 키는 다른 쪽 부재를 단언하지 않는다)

- 양 노드 같은 값 0개: 없음

| 키 | main | sub | 노드 미특정 | 판정 |
|---|---|---|---|---|
| `NCCL_CROSS_NIC` | `1` | — | — | main 만 캡처 창에 남음 — sub 여부 미관측(꼬리 캡처) |
| `NCCL_DMABUF_ENABLE` | — | `0` | — | sub 만 캡처 창에 남음 — main 여부 미관측(꼬리 캡처) |
| `NCCL_IB_DISABLE` | — | `1` | — | sub 만 줄에 남음 · Ray 접힘 +2 — main 사본 여부 미관측 |
| `NCCL_NET_PLUGIN` | — | `spcx` | — | sub 만 캡처 창에 남음 — main 여부 미관측(꼬리 캡처) |
| `NCCL_SOCKET_IFNAME` | — | `<nic:cluster>` | — | sub 만 줄에 남음 · Ray 접힘 +2 — main 사본 여부 미관측 |
| `nccl:network` | `Socket` | — | — | main 만 줄에 남음 · Ray 접힘 +3 — sub 사본 여부 미관측 |
| `nccl:version` | — | `2.30.7+cuda13.3` | — | sub 만 캡처 창에 남음 — main 여부 미관측(꼬리 캡처) |
<!-- /FACT:measurement_env_nodes -->

> 이 절이 답하는 질문: 서브(Ray worker) 노드는 메인과 무엇이 다르고, 무엇을 반드시 같게 맞춰야 하는가?

**(1) 역할 차이.** master 는 Ray head 를 띄우고 트리플렛(`nv4-bf-262k-mmp.{yaml,sh}`)을 받아 `vllm serve` 를 실행한다. slave 는 트리플렛을 받지 않는 Ray worker 다 — 같은 이미지 · 같은 compose · 같은 `serve_runner.sh` 가 `NODE_ROLE` 로 갈라지고, slave 의 `CONFIG_FILE` 은 `default` 라 `arm_patch.sh` 는 런타임 패치를 무장하지 않는다(이 셀은 master 에도 런타임 패치가 없다). master 는 `ray status` 의 GPU 합류가 리터럴 `2` 가 될 때까지 기다린다(`serve_runner.sh` L79) — 노드 · GPU 수가 다른 클러스터에서는 이 줄을 고쳐야 한다.

**(2) 서브에 전달돼야 하는 env(무리별).**
- 이미지 정체성(`IMAGE_TAG` · `BUILD_DOCKERFILE` · `VLLM_REF`): 빠지면 slave 가 compose 기본 태그 · Dockerfile · ref 로 빌드해 두 노드의 vLLM 이 갈라진다.
- 빌드 튜닝(`BUILD_JOBS`): 정체성은 아니지만 양쪽이 같아야 한다 — slave 만 기본 16 으로 컴파일하면 거기서 OOM 이 난 이력이 있다.
- 마운트(`NAS_MODEL_PATH` · `QUANT_MODEL_PATH` · `TIKTOKEN_HOST_PATH` · `PLE_MMAP_HOST_PATH`): `--env-file` 을 쓰면 프로젝트 .env 자동 로드가 꺼져 기본 마운트로 폴백하므로 셸 env 로 명시 주입한다.
- 클러스터(`SLAVE_CONTAINER_NAME` 과 `.env.cluster` 의 주소 · Ray 값): 이름이 어긋나면 slave 측 워치독 필터가 매칭 0 이 된다.
- 서빙 env(`VLLM_PLE_MMAP` · `VLLM_PLE_MMAP_DIR`): Ray 워커도 자기 rank 몫의 가중치를 올리므로 slave 도 mmap 을 켜고 같은 스테이징 경로를 봐야 한다.

**(3) 선행조건.** 가중치가 양 노드의 같은 마운트 경로에 실재 · PLE 스테이징 파일이 slave 로컬 NVMe 의 같은 경로에도 실재 · 드라이버 계열 동일 · 인터커넥트(`.env.interconnect` 를 두 노드가 같이 쓴다) 준비. 이 실행은 스모크 스크립트의 RAM 게이트가 양 노드 MemAvailable 을 확인한 뒤 기동했다.

**(4) ABI 동일성.** 요건은 동일 digest 가 아니라 동일 ABI 다 — 노드마다 로컬로 짓는다. 이 실행의 serve 단계 attestation 은 네 축이 전부 equal 이다: vLLM SHA `74c96922ecb9017f413318c76d1af83aa2ab45a5` · torch `2.13.0a0+9186a08b2c.nv26.7.59513937` · driver `580.178.04` · 빌드 원장 identity `fcc9de2e…`(Dockerfile · build-arg(`BUILD_JOBS` 면제) · 패치별 스크립트 sha256 · 결과 · vLLM git sha 를 합친 값). 이미지 digest 는 노드마다 다르다 — main `sha256:a2c4ca49…` · sub `sha256:87b6a67d…`(`docs/simlog/26092300_hint_publisher_G1G3_E2E/d1_build_identity.json`) — 로컬 빌드라 정상이다. 다만 위 범위 줄대로 이 attestation 파일이 **측정 실행과 같은 실행의 것인지는 발행기가 검증하지 못했다**(파일은 다음 실행이 덮는다 · 02 Q6). 파일 작성 시각 00:13:58Z 는 이 측정의 예산 창(23:42:54Z ~ 00:48:05Z) 안에 있다.

**(5) 측정 당시 인터커넥트 실효값.** 측정 실행 엔진 로그(꼬리 800줄 캡처)의 echo — `NCCL_SOCKET_IFNAME=<nic:cluster>`(L463 · sub 줄 · Ray 접힘 +2) · `NCCL_NET_PLUGIN=spcx`(L469 · sub) · `NCCL_IB_DISABLE=1`(L482 · sub · Ray 접힘 +2) · `NCCL_DMABUF_ENABLE=0`(L496 · sub) · `NCCL_CROSS_NIC=1`(L499 · main) · 런타임 네트워크 `Socket`(L494 · main · Ray 접힘 +3) · NCCL `2.30.7+cuda13.3`(L467 · sub). 표의 판정 칸이 "여부 미관측" 인 키는 다른 노드가 같은 줄을 냈는지 로그가 말하지 않는 것이다 — 한 노드에만 설정됐다는 뜻이 아니다(`.env.interconnect` 는 두 노드에 같은 파일로 간다). 측정 뒤 재생성된 interconnect 형상은 없다(1.1) — 템플릿 값과 위 echo 는 겹치는 키에서 같다.
