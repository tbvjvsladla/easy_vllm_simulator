# 01 · 산출물 — 무엇이 실제로 쓰였고, 어떻게 다시 띄우는가

## 1.1 적용 판정 표
> 기계가 적용 증거(빌드 원장 · 원장이 없으면 라벨된 재구성) × 파일 × 선언을 대사한 결과다. `artifacts/` 에는 **적용된 것만** 실렸다 — 빌드 때 스스로 skip 된 패치와 쓰이지 않은 평면의 파일은 표에만 남고 싣지 않는다.

<!-- FACT:slots -->
| 슬롯 | 실림 | 파일 | 신호 | 적용 증거 | 사유 |
|---|---|---|---|---|---|
| `build_patch_post` | 실림 | `artifacts/build_patch_post/10-deepgemm.sh` · `artifacts/build_patch_post/20-triton-kernels.sh` · `artifacts/build_patch_post/30-mxfp4-triton-sm121.sh` · `artifacts/build_patch_post/40-humming-nvml-gb10.sh` | 2-signal | build-ledger:attestation:output/multi/benchlog/attestation_ds4f0731-1m-spec7-roce.json#main | 미관측 |
| `build_patch_pre` | 실림 | `artifacts/build_patch_pre/60-qwen4exp-nvfp4-mixed.sh` · `artifacts/build_patch_pre/62-qwen4exp-ple-mmap.sh` · `artifacts/build_patch_pre/64-qwen4exp-qsa-fp8kv.sh` | 2-signal | build-ledger:attestation:output/multi/benchlog/attestation_ds4f0731-1m-spec7-roce.json#main | 미관측 |
| `build_recipe` | 실림 | `artifacts/build_recipe/Dockerfile.source-build` · `artifacts/build_recipe/requirements.txt` | 3-signal | build-ledger:build-ledger.dockerfile | `Dockerfile.source-build`(NGC `26.07-py3` 베이스 · `VLLM_REF=v0.29.0rc6` · `TORCH_CUDA_ARCH=12.1a` · `RAY_VERSION=2.48.0` · `SM12X_PORT=0` · `SRC_DEPS_AUTHORITY=0`)와 그것이 COPY 하는 `requirements.txt`(flashinfer 0.6.18 양보 · W1). 소스빌드인 이유는 00 §0.4. 1.1 이 Dockerfile 은 docker history 17/17 일치로 쓰인 바이트를 확인했고 requirements.txt 는 확인하지 못했다(이미지 탐침 안 함). |
| `compose` | 실림 | `artifacts/compose/.env.cluster.template` · `artifacts/compose/.env.interconnect.template` · `artifacts/compose/.env.template` · `artifacts/compose/docker-compose.yaml` · `artifacts/compose/serve_runner.sh` · `artifacts/compose/sub_recipe.json` | 2-signal | file:output/multi/docker-compose.yaml | `docker-compose.yaml`(측정 전 마지막 커밋 26b17f6e560a) · `serve_runner.sh`(50a5ce486738) · env 형상 3종 · `sub_recipe.json`. serve_runner 의 GPU 합류 리터럴 `2`(L79)가 노드 수와 숨은 결합이다(1.5). `arm_patch.sh` 는 런타임 패치 파일이 없어 no-op 이라 싣지 않았다. |
| `fork_pin` | 안 실림 | — | 1-signal | none:.claude/policies/arch_variant_ledger.json | 미관측 |
| `runtime_patch` | 안 실림 | — | 1-signal | none:output/multi/configs/ds4f0731-1m-spec7-roce_patch.py | 미관측 |
| `triplet` | 실림 | `artifacts/triplet/.env.ds4f0731-1m-spec7-roce` · `artifacts/triplet/ds4f0731-1m-spec7-roce.sh` · `artifacts/triplet/ds4f0731-1m-spec7-roce.yaml` | 3-signal | file:output/multi/configs/ds4f0731-1m-spec7-roce.yaml | `ds4f0731-1m-spec7-roce.{yaml,sh}` · `.env.ds4f0731-1m-spec7-roce` — 09-09 태그 트리플렛의 재명명본(값 변경 0 · plan_26092808 §2). 값의 지위는 1.3. |

**레시피 경고 · 실린 리비전**(기계 관측 — 수신자 빌드가 여기서 갈라질 수 있다)

- `build_recipe` `output/multi/Dockerfile.source-build` 실린 리비전: 리비전 미특정 · 선택 방법 `worktree(측정 이미지 docker history 의 RUN·COPY 와 전수 일치)` · docker history 대조 17/17 · 실린 바이트 `worktree` · 쓰인 바이트임을 관측으로 확인 예
- `build_recipe` `output/multi/requirements.txt` 실린 리비전: 리비전 미특정 · 선택 방법 `worktree(이미지 안 COPY 대상 바이트 미관측 — 이미지 탐침 안 함)` · docker history 대조 없음 · 실린 바이트 `worktree` · 쓰인 바이트임을 관측으로 확인 아니오
- `compose` `.claude/skills/upstream-version-watch/assets/configs/serve_runner.sh` 실린 리비전: 커밋 `50a5ce486738` · 선택 방법 `worktree(mtime 2026-09-06T21:57:53Z ≤ 측정 2026-09-28T00:35:13Z(certificate.measured_utc) · 바이트 = 측정 전 마지막 커밋 50a5ce486738)` · docker history 대조 없음 · 실린 바이트 `worktree` · 근거 `mtime≤measured` · 쓰인 바이트임을 관측으로 확인 예
- `compose` `output/multi/docker-compose.yaml` 실린 리비전: 커밋 `26b17f6e560a` · 선택 방법 `worktree(mtime 2026-09-21T20:48:25Z ≤ 측정 2026-09-28T00:35:13Z(certificate.measured_utc) · 바이트 = 측정 전 마지막 커밋 26b17f6e560a)` · docker history 대조 없음 · 실린 바이트 `worktree` · 근거 `mtime≤measured` · 쓰인 바이트임을 관측으로 확인 예

**싣지 않은 파일**(슬롯별 사유 — 빌드 때 skip 된 패치는 아래 적용 집합 표에 있다)

- `build_patch_post` known-non-patch 1건: `.gitkeep`
- `build_patch_pre` known-non-patch 3건: `.gitkeep` · `PROVENANCE.json` · `files/`
- `compose` `arm_patch.sh` — `not-armed(output/multi/configs/ds4f0731-1m-spec7-roce_patch.py 없음 · slave CONFIG_FILE='default' 의 _patch.py 없음 — arm_patch.sh 는 no-op)`
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
| attestation | observed | output/multi/benchlog/attestation_ds4f0731-1m-spec7-roce.json |

> **attestation 범위** — config `ds4f0731-1m-spec7-roce` · phase serve · 작성 2026-09-27T23:54:55Z(output/multi/benchlog/attestation_ds4f0731-1m-spec7-roce.json 파일 mtime(파일시스템 관측 — 문서에 시각 칸이 없다 · 기록 필드 아님)) · 묶은 근거 cell-name · 파일 `output/multi/benchlog/attestation_ds4f0731-1m-spec7-roce.json`: **same config(ds4f0731-1m-spec7-roce) · serve — 이 측정 실행과 같은 실행인지는 미검증(파일은 다음 실행이 덮는다)**
<!-- /FACT:applied_set -->

## 1.2 슬롯별 적용 사유
> 이 절이 답하는 질문: 실린 산출물 하나하나는 무엇을 고치며, 왜 이 셀에 필요했고, 이 셀에서 실제로 발화했으며, 언제 불필요해지는가?

1.1 의 적용 결과는 전부 이미지 안 빌드 원장(`build-ledger` · attestation 경유)이 기록한 관측이다. attestation 은 같은 config 의 serve phase 파일이고, 시각 기록(master 기동 23:37:18Z → attestation mtime 23:54:55Z → 측정 23:57:04Z · 그 사이 재선언 없음)은 이 측정 실행과 같은 실행을 가리킨다 — 실행 id 결속 필드는 없다(1.1 범위 줄 · Q7). 실린 빌드 패치 7개는 모두 **공유 이미지의 일부**다(한 이미지가 모든 모델을 서빙한다). 그중 10 · 20 · 30 · 40 은 DeepSeek-V4 계열이 계기인 arch-enablement(10 은 이 셀 dense 경로에서 발화 · 40 은 이 셀 MoE 백엔드에 직접 필요)이고, **이 모델의 요구가 아닌 것은 qwen4exp 60 · 62 · 64 뿐**이다. 그래서 파일마다 "이 셀에서 발화했나" 를 엔진 로그로 따로 가른다.

### build_patch_post/10-deepgemm.sh
(a) DeepGEMM 을 공식 `deepseek-ai/DeepGEMM` 의 `nv_dev` 브랜치(SHA 핀 a6b593d)로 오버라이드 설치한다 — SM120 FP8 GEMM 실행커널 원천 · arch-게이트 소스패치 없음(헤더). (b) 헤더의 model-trigger 가 DeepseekV4ForCausalLM FP8 block-scale dense/MoE 다 — 이 셀의 dense 가 그 경로다. 이 계보에서 이 패치를 빼 본 기록은 없다(W id 없음 · 헤더의 옛 계보 판정이 근거). (c) 발화: 양 rank 엔진 로그에 `Selected DeepGemmFp8BlockScaledMMKernel for Fp8LinearMethod` · `DeepGEMM E8M0 enabled on current platform.`(lite_engine L272 · L274 · L438) — **이 셀에서 활성**. (d) 폐기 조건: vLLM 자체 deepgemm 핀이 SM120 실행커널을 담은 커밋 이상이 되면 — 헤더의 probe 는 오버라이드로 클론한 nv_dev 소스에 SM120 심볼이 있는지(브랜치 변경 의심)만 보므로 이 조건을 감지하지 않는다 · 자동 트립와이어 없음.

### build_patch_post/20-triton-kernels.sh
(a) NGC 번들 standalone `triton_kernels`(불완전) 를 지워 vLLM vendored 완전본이 alias 되게 한다. (b) 헤더가 적은 필요 이유는 옛 DeepSeek-V4 MXFP4 MoE 가 TRITON 경로를 못 타 MARLIN repack 으로 통합메모리 OOM 을 낸 것이다(0.24/0.25 계보). (c) 이 셀은 MoE 에 HUMMING 을 골랐다(`Using 'HUMMING' Mxfp4 MoE backend.` · L277) — TRITON MoE 경로는 쓰지 않았으므로 **적용됐으나 이 셀에서 불활성**(추론 — 이 패치 고유의 발화 서명이 로그에 없다). (d) 폐기 조건: NGC 번들 triton_kernels 가 `matmul_ogs`·`routing` 을 갖추면 — 이 패치는 번들을 무조건 제거한 뒤 vendored alias 만 검증하므로 그 변화를 감지하지 않는다 · 자동 트립와이어 없음.

### build_patch_post/30-mxfp4-triton-sm121.sh
(a) OAI-Triton MXFP4 MoE 디바이스 게이트의 capability 상한을 (11,0) → (13,0) 으로 넓혀 SM121 을 포함시킨다. (b) 필요 이유는 20 과 같다(옛 DeepSeek-V4 MXFP4 가 MARLIN 으로 떨어지던 벽). (c) 이 셀은 humming 이므로 TRITON MoE 게이트를 거치지 않는다 — **불활성**(추론 · 발화 서명 없음). W8 은 triton 을 명시하면 이 게이트와 무관하게 SILU 미지원으로 거부된다는 벽이다. (d) 폐기 조건: 상류가 SM12x 를 그 게이트에 포함하면(헤더 REF 의 소스 줄이 바뀌면).

### build_patch_post/40-humming-nvml-gb10.sh
(a) humming-kernels 의 튜닝 휴리스틱이 부르는 NVML 클록 쿼리 2개(`NVML_CLOCK_MEM` · `NVML_CLOCK_SM`)를 try/except 로 가드하고 GB10 추정값으로 폴백한다. (b) GB10 은 NVML 로 그 클록을 노출하지 않아 가드 없이는 humming MoE 의 가중치 적재 직후 EngineCore init 에서 크래시했다(헤더 · 2026-06-28 측정). 이 셀의 moe-backend 가 humming 이므로(V9) 이 셀에 직접 필요한 공유 패치다. (c) 발화: 이 셀은 humming 튜닝 경로를 지났고(`Attempting to override humming GEMM config` · L447) 크래시가 없었다 — 가드가 폴백으로 먹었다는 직접 서명은 로그에 없다(추론). (d) 폐기 조건: humming-kernels 가 NotSupported 를 스스로 처리하면.

### build_patch_pre/60-qwen4exp-nvfp4-mixed.sh · 62-qwen4exp-ple-mmap.sh · 64-qwen4exp-qsa-fp8kv.sh
(a) 셋 다 qwen4_exp(Qwen3.8-Flash-Next) 전용 소스 이식이다 — NVFP4 mixed 체크포인트 로드 · PLE n-gram NVMe mmap(`VLLM_PLE_MMAP=1` 일 때만) · QSA 어텐션 fp8 KV 읽기. (b) 이 모델(DeepseekV4ForCausalLM)의 요구가 아니다 — 같은 이미지를 공유하는 다른 캠페인(camp-26090918)의 셀 전제다(헤더). (c) **이 셀에서 불활성**(추론 · 발화 서명 없음) — 62 · 64 는 qwen4_exp 모델 경로의 코드라 이 아키텍처를 로드하지 않는 이 셀에서 타지 않는다. 60 은 공유 파일 `vllm/model_executor/layers/quantization/modelopt.py` 도 편집하므로 아키텍처로 가를 수 없다 — 근거는 헤더의 '세 변경은 ModelOptMixedPrecisionConfig 로드 경로에서만 살아나며 FP8(native) 체크포인트 동작은 바이트 단위로 동일' 이고, 이 체크포인트는 `quant_method` fp8 이다. (d) 폐기 조건: 상류 PR(#55513 등 헤더 인용)이 머지되거나 qwen4_exp 계열을 서빙하지 않게 되면.

### build_recipe
`Dockerfile.source-build`(NGC `26.07-py3` 베이스 · `VLLM_REF=v0.29.0rc6` · `TORCH_CUDA_ARCH=12.1a` · `RAY_VERSION=2.48.0` · `SM12X_PORT=0` · `SRC_DEPS_AUTHORITY=0`)와 그것이 COPY 하는 `requirements.txt`(flashinfer 0.6.18 양보 · W1). 소스빌드인 이유는 00 §0.4. 1.1 이 Dockerfile 은 docker history 17/17 일치로 쓰인 바이트를 확인했고 requirements.txt 는 확인하지 못했다(이미지 탐침 안 함).

### compose
`docker-compose.yaml`(측정 전 마지막 커밋 26b17f6e560a) · `serve_runner.sh`(50a5ce486738) · env 형상 3종 · `sub_recipe.json`. serve_runner 의 GPU 합류 리터럴 `2`(L79)가 노드 수와 숨은 결합이다(1.5). `arm_patch.sh` 는 런타임 패치 파일이 없어 no-op 이라 싣지 않았다.

### triplet
`ds4f0731-1m-spec7-roce.{yaml,sh}` · `.env.ds4f0731-1m-spec7-roce` — 09-09 태그 트리플렛의 재명명본(값 변경 0 · plan_26092808 §2). 값의 지위는 1.3.

### 싣지 않은 슬롯
- `build_patch_pre/50-dsv4-sm12x-port.sh` — skip(`SM12X_PORT=0` · stock 경로) · 0.29.0rc6 stock 이 arch-wall 을 해소해 이식이 필요 없었다(R7).
- `build_patch_pre/55-src-deps-authority.sh` — skip(`SRC_DEPS_AUTHORITY=0`).
- inline `strip-hoist` — skip(원장 사유: `register_opaque_type accepts hoist / opaque_object 부재`).
- `runtime_patch` — 없음(`ds4f0731-1m-spec7-roce_patch.py` 부재) · `fork_pin` — 없음(변종 원장 미등재 · stock).

## 1.3 값의 지위표
> 서빙 설정의 노브 **전부**와 기계가 모은 후보 지위다(lockset 출처 · yaml 주석 · 엔진 기본값). 최종 지위는 아래 `kind: value-status` 블록이 정한다 — 표의 노브마다 블록이 정확히 하나 있어야 린터를 통과한다.

<!-- FACT:value_status -->
서빙 노브 13개 — 노브마다 `kind: value-status` 블록이 **정확히 1개** 있어야 한다(노브·값은 이 표의 글자 그대로 · 값 칸의 `\|` 는 `|` 로 적는다).

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
<!-- /FACT:value_status -->

> 이 절이 답하는 질문: 각 서빙 노브의 값은 이 셀에서 어떤 지위인가 — 조정됐나, 승계됐나, 음성대조로 일부러 둔 것인가, 엔진 기본값인가, 필요조건인가?

```hint-event
id: V1
kind: value-status
노브: tensor-parallel-size
값: 2
지위: declared-requirement
근거: W2 — 멀티 트리플렛에서 TP·ray backend 가 빠져 스모크가 rc=2 로 멈췄고 명시로 넘었다 · 값 2 는 manifest 파생(노드 2 × GPU 1)
출처: [testlog_26090904_ds4f0731_029rc6_attempt_build_스모크_판정 §2., ds4f0731-1m-spec7-roce.yaml]
```

```hint-event
id: V2
kind: value-status
노브: distributed-executor-backend
값: ray
지위: declared-requirement
근거: W2 — 누락 시 rc=2(yaml 주석: 없으면 mp 로 가서 World size>GPU(1))
출처: [testlog_26090904_ds4f0731_029rc6_attempt_build_스모크_판정 §2., ds4f0731-1m-spec7-roce.yaml]
```

```hint-event
id: V3
kind: value-status
노브: gpu-memory-utilization
값: 0.85
지위: inherited
근거: 셀 config target_gmu 선언을 09-09 태그에서 승계 · 두 레버 스윕의 전 셀이 같은 값(바꿔 잰 기록 0) · 뺐을 때의 실패 관측 없음
출처: [sweep_map_26090912_e1m_levers §셀 (실행 순서), sweep_map_26090912_b768k_levers §셀 (실행 순서)]
```

```hint-event
id: V4
kind: value-status
노브: max-model-len
값: 1048576
지위: inherited
근거: 사용자 지정 컨텍스트 축(768K · 1M 두 값)의 1M · 모델 네이티브 한계와 같다 · 이 셀에서 조정 0
출처: [plan_26090819_ds4f0731_멀티TP2_KV3군_1M768K_광의탐색_Hermes §2.]
```

```hint-event
id: V5
kind: value-status
노브: quantization
값: fp8
지위: inherited
근거: 체크포인트 quantization_config(fp8 블록)와 같은 값을 명시한 승계 · 다른 값을 시도한 기록 없음(expert 는 fp4 — M4)
출처: [ds4f0731-1m-spec7-roce.yaml, plan_26090819_ds4f0731_멀티TP2_KV3군_1M768K_광의탐색_Hermes §2.]
```

```hint-event
id: V6
kind: value-status
노브: kv-cache-dtype
값: fp8
지위: declared-requirement
근거: W5 — auto 는 엔진 assert 로 거부 · turboquant 도 같은 경로로 구조적 거부(fp8_ds_mla 유일)
출처: [testlog_26090904_ds4f0731_029rc6_attempt_build_스모크_판정 §3.]
```

```hint-event
id: V7
kind: value-status
노브: kv-cache-memory-bytes
값: 10737418240
지위: tuned
근거: 계보 768K 셀에서 18→14→10 GiB 로 바꿔 가며 잰 기록(W7 · 18 GiB 사살 1회 · 14 GiB 사살 2회 뒤 수렴) · 이 셀은 그 10 GiB 를 승계 · 1M 요청 하나 대비 엔진 보고 1.85x
출처: [testlog_26090912_ds4f0731_029rc6_광의탐색_종합판정 §판정, ds4f0731-1m-spec7-roce.yaml]
```

```hint-event
id: V8
kind: value-status
노브: max-num-seqs
값: 1
지위: inherited
근거: 손레버(lockset batch_source=hand-lever) — yaml 주석의 1.73x 를 내림한 값 · 스윕 기록 0
출처: [ds4f0731-1m-spec7-roce.yaml]
```

```hint-event
id: V9
kind: value-status
노브: moe-backend
값: humming
지위: declared-requirement
근거: W8 — triton 으로 바꾸면 엔진 거부(SILU 미지원) · auto 는 MARLIN repack OOM 전력(20-triton-kernels.sh 헤더 · 이 계보 재시도 0)
출처: [sweep_map_26090912_b768k_levers §셀 (실행 순서), 20-triton-kernels.sh]
```

```hint-event
id: V10
kind: value-status
노브: enforce-eager
값: false
지위: tuned
근거: 768K 스윕에서 eager(L0 17.11) 대 cudagraph(L2 26.14) 를 바꿔 잰 기록 · 1M 은 결합 셀로만 재어 승계
출처: [sweep_map_26090912_b768k_levers §셀 (실행 순서)]
```

```hint-event
id: V11
kind: value-status
노브: speculative-config
값: '{"method":"dspark","num_speculative_tokens":7}'
지위: tuned
근거: spec on/off 를 바꿔 잰 기록(768K L0 17.11 → L1 29.72 · 1M E0 16.67 → E1 30.54) · k=7 자체는 스윕 0(dspark_block_size 5 이상 조건의 09-09 검증값)
출처: [sweep_map_26090912_b768k_levers §셀 (실행 순서), sweep_map_26090912_e1m_levers §셀 (실행 순서)]
```

```hint-event
id: V12
kind: value-status
노브: reasoning-parser
값: deepseek_v4
지위: inherited
근거: Hermes 용처로 09-09 레시피가 등록명을 확인해 넣은 값의 승계 · 다른 값 시도 0 · 이 셀 상주 chat 에서 reasoning 분리 관측(testlog_26092808 §5.)
출처: [ds4f0731-1m-spec7-roce.yaml, testlog_26092808_커널7_0_DS4F0731_RoCE_1M서빙_회귀판정 §5.]
```

```hint-event
id: V13
kind: value-status
노브: tool-call-parser
값: deepseek_v4
지위: inherited
근거: 09-09 레시피 승계 · 단독으로는 tool_choice auto 를 받지 못한다(enable-auto-tool-choice 부재 → HTTP 400 관측 · Q3)
출처: [ds4f0731-1m-spec7-roce.yaml, testlog_26092808_커널7_0_DS4F0731_RoCE_1M서빙_회귀판정 §5.]
```

**복사하면 위험한 값.** `kv-cache-memory-bytes`(V7)는 KV 필요량이 아니라 **호스트 기동 밸리**가 정한 값이다 — 노드 메모리 · 가중치 적재 경로가 다르면 다시 잰다. `max-num-seqs` 1(V8)은 손레버라 최적이 아니며, 이 값 때문에 동시성 2 이상의 요청당 decode 가 나뉜다. `gpu-memory-utilization` 0.85(V3)는 바꿔 잰 적이 없다 — 필요조건으로 복사하지 마라. 반대로 `kv-cache-dtype` fp8 · `moe-backend` humming · TP/ray 두 줄(V1 · V2 · V6 · V9)은 빼거나 바꾸면 실패가 관측된 값이다. 그리고 **이 표에 없는 노브** `enable-auto-tool-choice` 가 tool call 용처에서는 사실상 필요하다(관측 · Q3) — 이 레시피는 그것 없이 측정됐다.

## 1.4 재현 절차
> 셀 실행 기록에서 기계가 파생한 명령 순서와 단계별 소요다. 소요가 `미관측` 인 단계는 시각 기록이 없다는 뜻이다(0 이 아니다). 그 아래는 블랙박스 이벤트 원장에서 이 셀의 행을 기계가 뽑아 기동 시도로 묶은 것이고, 마지막은 측정한 실행의 엔진 로그가 echo 한 env 다(측정 당시 값의 정본).

<!-- FACT:reproduce -->
| 순서 | 단계 | 성공 판정 | 소요 | 관측 구간(UTC) | 소요 출처 | 명령 출처 |
|---|---|---|---|---|---|---|
| 1 | render | 러너 3종·.env·.env.cluster·.env.interconnect 생성(렌더러 fail-loud) | 미관측 | 미관측 | .claude/skills/upstream-version-watch/scripts/multinode_serve_smoke.sh 선행조건 안내 · .claude/skills/upstream-version-watch/scripts/render_dockerfile.py CLI | derived(렌더러 CLI) |
| 2 | build | 빌드 OK · 이미지 실재 | 층 창 347시간 46분 14초(docker history 첫 층 → 끝 층 CreatedAt · 캐시 재사용 층 포함 — 빌드 소요 아님) | 2026-09-08T11:51:56Z → 2026-09-22T23:38:10Z (layer-window) | docker history --no-trunc --format '{{json .}}' sha256:a2c4ca499fb2473191bcdfc7e192445750b4a7b6803cfe14e50b43f401000544 · 우리 Dockerfile 층 30개의 CreatedAt 첫 층 → 끝 층(캐시 재사용 층은 이전 빌드 시각 — 빌드 소요 아님) | reconstructed(build-ledger build_args + compose build) |
| 3 | serve | health 200 + 추론 1회(스모크 판정) | ≤ 19분 51초(상한) | 2026-09-27T23:37:13Z → 2026-09-27T23:57:04Z (upper) | docs/logs/main/events/2026-09.jsonl budget_declare(label smoke-ds4f0731-1m-spec7-roce) → 첫 측정 date(output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce/lite_cold_ds4f0731-1m-spec7-roce.json · date=컨테이너 시계(UTC 가정)) | derived(스모크 사용법) |
| 4 | bench | 리포트 발행(+PASS 면 인증서) | 38분 9초 | 2026-09-27T23:57:04Z → 2026-09-28T00:35:13Z (exact) | output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce/lite_cold_ds4f0731-1m-spec7-roce.json · date=컨테이너 시계(UTC 가정) date → sweep_index.generated_utc | reconstructed(bench json) |

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
# output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce/lite_cold_ds4f0731-1m-spec7-roce.json (date 20260927-235704)
vllm bench serve --backend openai-chat --model deepseek-v4-flash-0731 --tokenizer /app/models/DeepSeek/DeepSeek-V4/DeepSeek-V4-Flash-0731 --dataset-name random --random-input-len 595 --random-output-len 128 --num-prompts 1 --max-concurrency 1 --request-rate inf
# output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce/lite_warm_ds4f0731-1m-spec7-roce.json (date 20260927-235732)
vllm bench serve --backend openai-chat --model deepseek-v4-flash-0731 --tokenizer /app/models/DeepSeek/DeepSeek-V4/DeepSeek-V4-Flash-0731 --dataset-name random --random-input-len 595 --random-output-len 128 --num-prompts 3 --max-concurrency 1 --request-rate inf
```

**기동 성공 판정(관측 · 발행 자격)**: health 200 = true · 추론 1회 = true · 근거 `output/multi/benchlog/serve_proof_ds4f0731-1m-spec7-roce.json`
<!-- /FACT:reproduce -->

<!-- FACT:event_timeline -->
**블랙박스 이벤트 33행**(원장에서 이 셀 label·config 에 맞는 행만 · 시각순 · 기계 발췌)

| # | UTC | 노드 | kind | label | 내용 | 출처 |
|---|---|---|---|---|---|---|
| 1 | 2026-09-27T23:32:54Z | main | `budget_declare` | `smoke-ds4f0731-1m-spec7-roce` | 예산 선언(선언값 — 측정 아님) · floor_mib=21479 · weights_mib=79577 · kv_mib=10240 · overhead_mib=13312 · mem_total_mib=124608 · 기동 창 1/2: budget_renew 0회 · 닫힘 budget_clear 2026-09-27T23:34:58Z(선언 후 2m04s) · measured_utc 2026-09-28T00:35:13Z 는 이 창 밖(다른 기동 · 측정 전) | docs/logs/main/events/2026-09.jsonl:1000 |
| 2 | 2026-09-27T23:32:54Z | main | `budget_honored` | `smoke-ds4f0731-1m-spec7-roce` | budget_honored · floor_mib=21479 · arm_ceiling_mib=18407 · remaining_s=7200 · 창 시작 2026-09-27T23:32:54Z 후 0m00s | docs/logs/main/events/watchdog.jsonl:515 |
| 3 | 2026-09-27T23:34:09Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=69032 · rate_mib_s=22969 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → smoke-ds4f0731-1m-spec7-roce 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-27T23:32:54Z 후 1m15s) | docs/logs/main/events/watchdog.jsonl:516 |
| 4 | 2026-09-27T23:34:58Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-ds4f0731-1m-spec7-roce 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-27T23:32:54Z 후 2m04s) | docs/logs/main/events/2026-09.jsonl:1001 |
| 5 | 2026-09-27T23:34:59Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-ds4f0731-1m-spec7-roce 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-27T23:32:54Z 후 2m05s) | docs/logs/main/events/watchdog.jsonl:517 |
| 6 | 2026-09-27T23:37:13Z | main | `budget_declare` | `smoke-ds4f0731-1m-spec7-roce` | 예산 선언(선언값 — 측정 아님) · floor_mib=21479 · weights_mib=79577 · kv_mib=10240 · overhead_mib=13312 · mem_total_mib=124608 · 기동 창 2/2: budget_renew 17회 · 닫힘 미관측(창이 열린 채 원장이 끝난다) · measured_utc 2026-09-28T00:35:13Z 가 이 창 안(이 측정의 기동) | docs/logs/main/events/2026-09.jsonl:1003 |
| 7 | 2026-09-27T23:37:14Z | main | `budget_honored` | `smoke-ds4f0731-1m-spec7-roce` | budget_honored · floor_mib=21479 · arm_ceiling_mib=18407 · remaining_s=7199 · 창 시작 2026-09-27T23:37:14Z 후 0m00s | docs/logs/main/events/watchdog.jsonl:518 |
| 8 | 2026-09-27T23:38:30Z | cluster | `engine_log_window_start` | — | 캡처된 엔진 로그의 첫 시각(꼬리 캡처일 수 있다 — 엔진 시작 시각이 아닐 수 있다) · 선언 2026-09-27T23:37:13Z 후 1m17s · 시계: 컨테이너 시각을 UTC 로 읽어 측정 창 [2026-09-27T23:37:13Z, 2026-09-28T00:35:13Z] 안에 드는 것을 확인 | output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce/lite_engine_ds4f0731-1m-spec7-roce.log:143 |
| 9 | 2026-09-27T23:38:34Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=80175 · rate_mib_s=22898 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → smoke-ds4f0731-1m-spec7-roce 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-27T23:37:14Z 후 1m20s) | docs/logs/main/events/watchdog.jsonl:519 |
| 10 | 2026-09-27T23:38:36Z | cluster | `engine_prefetch_disabled` | — | 가중치 auto-prefetch 꺼짐 줄 ×4 — filesystem CIFS(vLLM 로그 문구) | output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce/lite_engine_ds4f0731-1m-spec7-roce.log:280 |
| 11 | 2026-09-27T23:50:43Z | cluster | `engine_weights_loaded` | — | 가중치 적재 완료 줄 ×4(rank·단계별) · 최장 353.89s · 이 행 = 마지막 줄 | output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce/lite_engine_ds4f0731-1m-spec7-roce.log:580 |
| 12 | 2026-09-27T23:50:57Z | cluster | `engine_model_loaded` | — | 모델 적재 완료 줄 ×2(rank 별) · 마지막 줄 79.04 GiB · 744.4s | output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce/lite_engine_ds4f0731-1m-spec7-roce.log:586 |
| 13 | 2026-09-27T23:51:49Z | cluster | `engine_kv_cache_sized` | — | GPU KV cache 1,941,478 tokens · 요청당 1,048,576 tokens 기준 최대 동시성 1.85x | output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce/lite_engine_ds4f0731-1m-spec7-roce.log:628 |
| 14 | 2026-09-27T23:52:50Z | cluster | `engine_shm_broadcast_wait` | — | shm_broadcast 대기 경고(60s 단위) ×2 · 2026-09-27T23:52:50Z–2026-09-27T23:53:50Z(1m00s) — vLLM 문구상 프로세스 hang 또는 긴 작업(컴파일·가중치/KV 양자화) 중 출현 | output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce/lite_engine_ds4f0731-1m-spec7-roce.log:682 |
| 15 | 2026-09-27T23:54:13Z | cluster | `engine_init_done` | — | 엔진 초기화(profile·KV 할당·warmup) 195.43s | output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce/lite_engine_ds4f0731-1m-spec7-roce.log:730 |
| 16 | 2026-09-27T23:54:29Z | cluster | `server_starting` | — | API 서버 기동 — 이 측정 창 선언 2026-09-27T23:37:13Z 후 17m16s · 캡처 로그 첫 시각 2026-09-27T23:38:30Z 후 15m59s | output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce/lite_engine_ds4f0731-1m-spec7-roce.log:736 |
| 17 | 2026-09-27T23:54:55Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=6138 · 창 시작 2026-09-27T23:37:13Z 후 17m42s | docs/logs/main/events/2026-09.jsonl:1004 |
| 18 | 2026-09-27T23:57:34Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=7041 · 창 시작 2026-09-27T23:37:13Z 후 20m21s | docs/logs/main/events/2026-09.jsonl:1005 |
| 19 | 2026-09-28T00:00:21Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=7033 · 창 시작 2026-09-27T23:37:13Z 후 23m08s | docs/logs/main/events/2026-09.jsonl:1006 |
| 20 | 2026-09-28T00:02:52Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=7049 · 창 시작 2026-09-27T23:37:13Z 후 25m39s | docs/logs/main/events/2026-09.jsonl:1007 |
| 21 | 2026-09-28T00:05:22Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=7050 · 창 시작 2026-09-27T23:37:13Z 후 28m09s | docs/logs/main/events/2026-09.jsonl:1008 |
| 22 | 2026-09-28T00:07:53Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=7049 · 창 시작 2026-09-27T23:37:13Z 후 30m40s | docs/logs/main/events/2026-09.jsonl:1009 |
| 23 | 2026-09-28T00:10:23Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=7050 · 창 시작 2026-09-27T23:37:13Z 후 33m10s | docs/logs/main/events/2026-09.jsonl:1010 |
| 24 | 2026-09-28T00:12:54Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=7049 · 창 시작 2026-09-27T23:37:13Z 후 35m41s | docs/logs/main/events/2026-09.jsonl:1011 |
| 25 | 2026-09-28T00:15:22Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=7052 · 창 시작 2026-09-27T23:37:13Z 후 38m09s | docs/logs/main/events/2026-09.jsonl:1012 |
| 26 | 2026-09-28T00:17:54Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=7048 · 창 시작 2026-09-27T23:37:13Z 후 40m41s | docs/logs/main/events/2026-09.jsonl:1013 |
| 27 | 2026-09-28T00:20:25Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=7049 · 창 시작 2026-09-27T23:37:13Z 후 43m12s | docs/logs/main/events/2026-09.jsonl:1014 |
| 28 | 2026-09-28T00:22:51Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=7054 · 창 시작 2026-09-27T23:37:13Z 후 45m38s | docs/logs/main/events/2026-09.jsonl:1015 |
| 29 | 2026-09-28T00:24:55Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=7076 · 창 시작 2026-09-27T23:37:13Z 후 47m42s | docs/logs/main/events/2026-09.jsonl:1016 |
| 30 | 2026-09-28T00:25:19Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=7176 · 창 시작 2026-09-27T23:37:13Z 후 48m06s | docs/logs/main/events/2026-09.jsonl:1017 |
| 31 | 2026-09-28T00:27:47Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=7052 · 창 시작 2026-09-27T23:37:13Z 후 50m34s | docs/logs/main/events/2026-09.jsonl:1018 |
| 32 | 2026-09-28T00:30:18Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=7049 · 창 시작 2026-09-27T23:37:13Z 후 53m05s | docs/logs/main/events/2026-09.jsonl:1019 |
| 33 | 2026-09-28T00:32:43Z | main | `budget_renew` | `smoke-ds4f0731-1m-spec7-roce` | budget_renew · floor_mib=21479 · ttl_s=7200 · remaining_before_s=7055 · 창 시작 2026-09-27T23:37:13Z 후 55m30s | docs/logs/main/events/2026-09.jsonl:1020 |

**기동 시도**(예산 선언 `budget_declare` 마다 한 시도 · 같은 노드의 이어지는 행과 선언 없는 노드(엔진 로그)의 그 시각 행을 묶었다 · 사살 · 트립 · 거부는 닫힘과 따로 센다 · `measurement` 행은 측정 기록이라 시도로 세지 않는다(위 표에만) — 해석은 저작자 몫)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 1 | main | `smoke-ds4f0731-1m-spec7-roce` | 2026-09-27T23:32:54Z | 없음 | 없음 | `budget_clear` 2026-09-27T23:34:58Z | 3 |
| 2 | main | `smoke-ds4f0731-1m-spec7-roce` | 2026-09-27T23:37:13Z | 17회 · 첫 2026-09-27T23:54:55Z | 없음 | —(닫힘 행 없음) | 10 |
<!-- /FACT:event_timeline -->

<!-- FACT:measurement_env -->
**측정 실행의 env 관측 16행**(측정한 실행의 엔진 로그가 echo 한 값 · 기계 발췌 — 측정 당시 값의 정본이다. 표에 없는 키는 로그가 echo 하지 않은 것이지 설정되지 않았다는 뜻이 아니다 · 측정 뒤 재생성된 env 형상은 01 §1.1 을 본다)

| # | 키 | 값 | 노드 | 종류 | 출처(첫 줄) | 관측 |
|---|---|---|---|---|---|---|
| 1 | `NCCL_IB_SPLIT_DATA_ON_QPS` | `0` | main | env-echo | output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce/lite_engine_ds4f0731-1m-spec7-roce.log:L36 | 로그 1개 |
| 2 | `NCCL_IB_GID_INDEX` | `3` | main | env-echo | output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce/lite_engine_ds4f0731-1m-spec7-roce.log:L72 | 로그 2개 |
| 3 | `NCCL_SOCKET_IFNAME` | `<nic:cluster>` | sub | env-echo | output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce/lite_engine_ds4f0731-1m-spec7-roce.log:L283 | 로그 6개 · Ray 접힘 +3(접힌 사본의 노드 미관측) |
| 4 | `NCCL_SOCKET_IFNAME` | `<nic:cluster>` | sub | param-echo | output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce/lite_engine_ds4f0731-1m-spec7-roce.log:L284 | 로그 6개 · Ray 접힘 +2(접힌 사본의 노드 미관측) |
| 5 | `nccl:version` | `2.30.7+cuda13.3` | sub | runtime | output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce/lite_engine_ds4f0731-1m-spec7-roce.log:L287 | 로그 6개 |
| 6 | `NCCL_NET_PLUGIN` | `spcx` | sub | env-echo | output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce/lite_engine_ds4f0731-1m-spec7-roce.log:L289 | 로그 6개 |
| 7 | `NCCL_IB_DISABLE` | `0` | sub | env-echo | output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce/lite_engine_ds4f0731-1m-spec7-roce.log:L294 | 로그 6개 · Ray 접힘 +2(접힌 사본의 노드 미관측) |
| 8 | `NCCL_IB_HCA` | `=<nic:cluster>,<nic:cluster>` | sub | param-echo | output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce/lite_engine_ds4f0731-1m-spec7-roce.log:L295 | 로그 6개 · Ray 접힘 +2(접힌 사본의 노드 미관측) |
| 9 | `NCCL_IB_MERGE_NICS` | `1` | sub | env-echo | output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce/lite_engine_ds4f0731-1m-spec7-roce.log:L297 | 로그 6개 |
| 10 | `nccl:net_ib_devices` | `[RO]; OOB <nic:cluster>:<node:main><0>` | main | runtime | output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce/lite_engine_ds4f0731-1m-spec7-roce.log:L299 | 로그 6개 · Ray 접힘 +3(접힌 사본의 노드 미관측) |
| 11 | `NCCL_IB_QPS_PER_CONNECTION` | `4` | sub | env-echo | output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce/lite_engine_ds4f0731-1m-spec7-roce.log:L309 | 로그 6개 · Ray 접힘 +3(접힌 사본의 노드 미관측) |
| 12 | `nccl:network` | `IB` | sub | runtime | output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce/lite_engine_ds4f0731-1m-spec7-roce.log:L322 | 로그 7개 · Ray 접힘 +3(접힌 사본의 노드 미관측) |
| 13 | `NCCL_DMABUF_ENABLE` | `0` | main | env-echo | output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce/lite_engine_ds4f0731-1m-spec7-roce.log:L324 | 로그 7개 |
| 14 | `NCCL_CROSS_NIC` | `1` | main | env-echo | output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce/lite_engine_ds4f0731-1m-spec7-roce.log:L327 | 로그 7개 |
| 15 | `NCCL_IB_SPLIT_DATA_ON_QPS` | `0` | sub | env-echo | output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce/lite_engine_ds4f0731-1m-spec7-roce.log:L368 | 로그 7개 |
| 16 | `NCCL_IB_GID_INDEX` | `3` | sub | env-echo | output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce/lite_engine_ds4f0731-1m-spec7-roce.log:L423 | 로그 8개 |

- 값 주: 기계 치환(pii.substitution_table · 장치·호스트·주소는 자리표시 — 원문은 출처 줄)

- Ray 접힘: 출처 줄 끝의 `[repeated Nx across cluster]` — Ray 가 다른 프로세스의 같은 줄 N개를 한 줄로 접었다. 노드 칸은 **첫 줄**의 노드이고, 접힌 사본이 어느 노드의 것인지는 로그에 없다(관측 대상 밖).

- 로그 창: `tail-800(lite_bench.sh@fcb0754fffe1 L159)`(이 표의 출처 로그 1개) — 이 캡처는 엔진 출력의 **마지막 N줄**이다(측정 도구의 `tail -N` 상한에 닿았다 · 생산자 evidence `log_window`). 창 밖에서 echo 한 줄은 이 표에 없다 — 표에 없는 키 · 한 노드에만 있는 키는 창 밖이었을 수 있다.
<!-- /FACT:measurement_env -->

> 이 절이 답하는 질문: 수신자가 이 zip 만으로 이 셀을 다시 띄우려면 어떤 순서로 무엇을 하고, 각 단계의 성공을 무엇으로 판정하며, 얼마나 기다려야 하는가?

**(a) 사전 준비.** 가중치는 관리 NAS(`<manifest.nas_model_path>`) 아래 `DeepSeek/DeepSeek-V4/DeepSeek-V4-Flash-0731` 에 있고 compose 가 `NAS_MODEL_PATH` 를 `/app/models` 로 마운트한다 — **양 노드 같은 경로**여야 한다(1.5 · `sub_recipe.json` 의 mount 무리). 스테이징 파일은 없다(PLE 없음 · mmap 미사용). 엔진은 이 파일시스템(CIFS)을 인식된 네트워크 FS 로 보지 않아 가중치 auto-prefetch 를 껐다(lite_engine L280 · 01 §1.4 행 10) — 경고이지 벽이 아니다. 필수 env 의 자리: 셀 env(`artifacts/triplet/.env.ds4f0731-1m-spec7-roce` — `CONFIG_FILE` · `SERVING_MODEL_NAME` · `IMAGE_TAG` · 컨테이너 이름 쌍) · 클러스터 env(`.env.cluster.template`) · NCCL env(`.env.interconnect.template`). 측정 당시 NCCL 값은 위 '측정 실행의 env 관측' 표가 정본이다(`NCCL_IB_DISABLE` `0` · `nccl:network` `IB` 등). 이 형상을 얻으려면 자기 manifest 에 `interconnect.nccl_transport: rdma` 를 두고 렌더러 `--nccl-envfile` 로 다시 파생한다(렌더러 커밋 `8319b4f` 이후 판본 — 그 이전 판본은 Socket 을 강제한다 · W11 · W12).

**(b) 빌드.** 양 노드 각각 로컬로 짓는다(이미지 전송 ✗ — 같아야 하는 것은 digest 가 아니라 ABI). 선택자 `Dockerfile.source-build` · build-arg 는 위 표(`VLLM_REF=v0.29.0rc6` · `TORCH_CUDA_ARCH=12.1a` · `RAY_VERSION=2.48.0` · `SM12X_PORT=0` · `SRC_DEPS_AUTHORITY=0` · `BUILD_JOBS=8`). 소요는 관측되지 않았다 — 위 표의 347시간은 캐시 층을 포함한 층 창이지 빌드 시간이 아니다.

**(c) 기동과 성공 판정.** `multinode_serve_smoke.sh <cell> --keep-up` 이 순서대로 로드-전 RAM 게이트(양 노드 · `required_mib` 대비 가용) → 예산 선언(양 노드 honored 확인) → master(Ray head + serve) → slave(Ray worker) → health 폴링 → chat 추론 1회를 한다. 이 셀의 관측(시도 2): 선언 23:37:13Z → 가중치 적재 완료 23:50:43Z(rank 당 `Loading weights took` 약 353 s 가 target · dspark draft 두 번) → KV 크기 결정 23:51:49Z → 엔진 초기화 195.43 s → API 서버 기동 23:54:29Z, **선언 후 17분 16초**(01 §1.4 행 6 · 11 · 13 · 15 · 16). 계보 문서는 이 기동을 "READY ~1035s" 로 적었고(testlog_26092808 §2), 09-09 768K 셀 스모크는 "READY ~680s" 였다(testlog_26090904 §4 — 다른 셀 · 다른 컨텍스트). 성공 판정은 health 200 + chat 추론 1회(testlog_26092808 §2: content_len=1 · reasoning_len=96 · finish=stop)이고, NCCL 전송은 엔진 로그 `Using network IB`(L322)로 실증한다 — 이 줄 없이 PASS 를 선언하지 마라(W12). 정상인데 무서워 보이는 것: 엔진 초기화 중 `No available shared memory broadcast block found in 60 seconds` 가 1분 간격 2회(L682 · L726) — 그 뒤 초기화가 끝났다. 로드 중 워치독 `watchdog_highrate_hold`(`legacy_rule_would_trip=true`)는 시도 1 · 시도 2 에서 각 1회 찍혔다 — 선언 창이 급락률을 보류한 기록이다(행 3 · 9).

**기동 시도 수: 2.** 시도 1(선언 23:32:54Z · 갱신 없음 · `budget_clear` 23:34:58Z)은 rdma 첫 정의가 엔진을 Socket 전송으로 떨어뜨려 로드 도중 `--down` 으로 회수한 시도다(W12 · testlog_26092808 §1). 시도 2(선언 23:37:13Z · 갱신 17회 · 닫힘 행 없음 = 상주)가 측정 대상이다.

**(d) 호스트 메모리 안전 예산.** 선언값: `mem_total_mib=124608` · `weights_mib=79577`(체크포인트 ÷ TP) · `kv_mib=10240` · `overhead_mib=13312` → `floor_mib=21479` · arm 상한 18,407 MiB · TTL 7200 s(갱신 루프가 상주 중 연장). 전부 **선언**이다 — 엔진 로그의 실측은 rank 당 모델 적재 79.04 GiB · KV 10.0 GiB(03 §3.1 `lite.engine_weights_gib` · `lite.engine_kv_gib`)다. 선언 입력으로 `SMOKE_BUDGET_OVERHEAD_MIB`(기본값 없음)와 `READY_MAX`(5초 폴링 횟수 · 이 셀 360)를 넘긴다.

**(e) 측정 명령.** 위 표 bench 단계의 `vllm bench serve` 두 줄은 lite 레그의 bench JSON 에서 재구성한 것이지 실행 원문이 아니다. full 레그는 GuideLLM 0.7.3 · 입력 1024 / 출력 256 · 레벨 1/2/4/8/16 × 반복 3 이고, 도구가 정한 인자는 03 §3.4 의 측정 도구 원문 발췌가 정본이다.

**(f) 엔진이 스스로 고른 런타임 경로.** 통신: NCCL 2.30.7 · SPCX 플러그인(v12) 로드 뒤 장치 미지원으로 건너뛰고(L291 · L311) 내장 IB 로 `Using network IB`(L322) · `Connected all rings, use ring PXN 0 GDR 0`(L148) — GPU Direct RDMA 는 꺼져 있다. 이 셀에서는 드라이버의 DMA-BUF/GDR 미지원 보고(Q2)와 렌더러 env `NCCL_DMABUF_ENABLE=0`(엔진 로그 echo · fe1bc42)이 함께 걸려 있어 어느 쪽이 GDR 을 껐는지 가를 수 없다. 모델: dense `DeepGemmFp8BlockScaledMMKernel`(L272) · KV `Using DeepSeek's fp8_ds_mla KV cache format.`(L275) · MoE `Using 'HUMMING' Mxfp4 MoE backend.`(L277) 뒤 humming GEMM config 재정의 줄 다수(L447~) — 경고가 아니라 튜닝 로그다.

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

> **attestation 범위** — config `ds4f0731-1m-spec7-roce` · phase serve · 작성 2026-09-27T23:54:55Z(output/multi/benchlog/attestation_ds4f0731-1m-spec7-roce.json 파일 mtime(파일시스템 관측 — 문서에 시각 칸이 없다 · 기록 필드 아님)) · 묶은 근거 cell-name · 파일 `output/multi/benchlog/attestation_ds4f0731-1m-spec7-roce.json`: **same config(ds4f0731-1m-spec7-roce) · serve — 이 측정 실행과 같은 실행인지는 미검증(파일은 다음 실행이 덮는다)**
<!-- /FACT:sub_recipe -->

<!-- FACT:measurement_env_nodes -->
**노드별 측정 env**(01 §1.4 표를 키 × 노드로 접은 것 · `만 관측` = 다른 쪽 줄이 캡처 로그에 없다 · Ray 가 같은 줄을 접은 키(`[repeated Nx across cluster]`)와 꼬리 캡처 로그(01 §1.4 로그 창)에서 한쪽만 남은 키는 다른 쪽 부재를 단언하지 않는다)

- 양 노드 같은 값 2개: `NCCL_IB_GID_INDEX` · `NCCL_IB_SPLIT_DATA_ON_QPS`

| 키 | main | sub | 노드 미특정 | 판정 |
|---|---|---|---|---|
| `NCCL_CROSS_NIC` | `1` | — | — | main 만 캡처 창에 남음 — sub 여부 미관측(꼬리 캡처) |
| `NCCL_DMABUF_ENABLE` | `0` | — | — | main 만 캡처 창에 남음 — sub 여부 미관측(꼬리 캡처) |
| `NCCL_IB_DISABLE` | — | `0` | — | sub 만 줄에 남음 · Ray 접힘 +2 — main 사본 여부 미관측 |
| `NCCL_IB_HCA` | — | `=<nic:cluster>,<nic:cluster>` | — | sub 만 줄에 남음 · Ray 접힘 +2 — main 사본 여부 미관측 |
| `NCCL_IB_MERGE_NICS` | — | `1` | — | sub 만 캡처 창에 남음 — main 여부 미관측(꼬리 캡처) |
| `NCCL_IB_QPS_PER_CONNECTION` | — | `4` | — | sub 만 줄에 남음 · Ray 접힘 +3 — main 사본 여부 미관측 |
| `NCCL_NET_PLUGIN` | — | `spcx` | — | sub 만 캡처 창에 남음 — main 여부 미관측(꼬리 캡처) |
| `NCCL_SOCKET_IFNAME` | — | `<nic:cluster>` | — | sub 만 줄에 남음 · Ray 접힘 +3 — main 사본 여부 미관측 |
| `nccl:net_ib_devices` | `[RO]; OOB <nic:cluster>:<node:main><0>` | — | — | main 만 줄에 남음 · Ray 접힘 +3 — sub 사본 여부 미관측 |
| `nccl:network` | — | `IB` | — | sub 만 줄에 남음 · Ray 접힘 +3 — main 사본 여부 미관측 |
| `nccl:version` | — | `2.30.7+cuda13.3` | — | sub 만 캡처 창에 남음 — main 여부 미관측(꼬리 캡처) |
<!-- /FACT:measurement_env_nodes -->

> 이 절이 답하는 질문: 서브(Ray worker) 노드는 메인과 무엇이 다르고, 무엇을 반드시 같게 맞춰야 하는가?

**(1) 역할 차이.** master(메인)는 Ray head 와 `vllm serve` 를 띄우고 트리플렛(`CONFIG_FILE=ds4f0731-1m-spec7-roce`)을 받는다. slave(서브)는 Ray worker 이며 트리플렛을 받지 않는다 — 같은 이미지 이름 · 같은 compose · 같은 `serve_runner.sh` 가 `NODE_ROLE` 로 갈리고 slave 의 `CONFIG_FILE` 은 `default` 다. master 는 `ray status` 의 GPU 합류가 리터럴 `2`(serve_runner.sh L79)와 같아질 때까지 기다린 뒤 serve 한다 — 노드 · GPU 수가 다르면 이 줄을 고치지 않으면 대기에서 멈춘다. 런타임 패치는 양쪽 다 무장되지 않는다(패치 파일 없음 · slave 는 `default_patch.py` 도 없어 no-op).

**(2) 서브 전달 env(무리별).** 이미지 정체성 — `IMAGE_TAG`(모르면 compose 기본 태그로 빌드 · 기동한다). 마운트 — `NAS_MODEL_PATH` · `QUANT_MODEL_PATH` · `TIKTOKEN_HOST_PATH` · `PLE_MMAP_HOST_PATH`(서브도 가중치 절반을 올리므로 같은 경로의 마운트원이 필요하다 · `--env-file` 사용 시 프로젝트 `.env` 자동 로드가 꺼지는 결함 때문에 셸 env 로 명시 주입). 클러스터 — `SLAVE_CONTAINER_NAME`(폴백 이름이면 서브 워치독 필터가 매칭 0). 노드별 값은 `<manifest.nodes[main].host>` · `<manifest.nodes[sub].host>` 형상이다. NCCL · Ray env 는 서브가 렌더러 산출 `.env.interconnect` · `.env.cluster` 를 정식 배달(sync_to_sub · 재렌더)로 받는다 — 이 셀은 그 배달로만 전송을 바꿨다(서브 직접 수정 ✗ · W11).

**(3) 선행조건.** 가중치가 서브의 **같은 경로**에 실재(이 셀 RAM 게이트: 서브 가용이 `required_mib` 이상 · 발행 스모크 로그) · 드라이버 계열 동일(580.173.02 — 인증서 · attestation) · 인터커넥트(RoCE v2 · manifest `interconnect` 의 HCA · GID 3)와 `nccl_transport: rdma` 를 반영한 렌더 산출물이 서브에 배달돼 있어야 한다.

**(4) ABI 동일성.** attestation v2 는 vLLM SHA(74c96922…) · torch · driver · build_ledger 가 양 노드 `equal` 이라 기록했다(testlog_26092808 §2). 이 attestation 의 범위는 같은 config 의 serve phase 이고, 시각 기록(master 기동 23:37:18Z → attestation mtime 23:54:55Z → 측정 23:57:04Z · 그 사이 재선언 없음)이 이 측정 실행과 같은 실행을 가리킨다 — 실행 id 결속 필드가 없다는 것이 한계다(01 §1.1 범위 줄 · Q7). 이미지 digest 는 노드마다 로컬 빌드라 같을 필요가 없고, 이 페이로드는 메인 측정 digest 만 싣는다(00 §0.4).

**(5) 측정 당시 인터커넥트 실효값.** 캡처 로그(꼬리 800줄)에 남은 값: `NCCL_IB_GID_INDEX` `3` · `NCCL_IB_SPLIT_DATA_ON_QPS` `0`(양 노드) · `NCCL_IB_DISABLE` `0` · `NCCL_NET_PLUGIN` `spcx` · `NCCL_IB_MERGE_NICS` `1` · `NCCL_IB_QPS_PER_CONNECTION` `4` · `nccl:network` `IB` · `nccl:version` `2.30.7+cuda13.3`(sub 줄) · `NCCL_CROSS_NIC` `1` · `NCCL_DMABUF_ENABLE` `0`(main 줄) — 출처 줄은 01 §1.4 '측정 실행의 env 관측' 표(lite_engine L36~L423). 한쪽 노드에만 남은 키는 Ray 접힘(`[repeated Nx across cluster]`) 또는 꼬리 캡처 창 때문이며, 다른 노드가 같은 값을 쓰지 않았다는 뜻이 아니다. 실린 `.env.interconnect.template` 은 `NCCL_IB_DISABLE` · `NCCL_NET` 을 `<manifest.interconnect.nccl_transport>` 표지(tier=env)로 싣는다 — 값은 수신자 manifest 에서 파생되며, 키가 없으면(= socket) 렌더러가 `NCCL_IB_DISABLE=1` · `NCCL_NET=Socket` 을 낸다. 이 셀의 측정 당시 값은 위 로그 관측(`NCCL_IB_DISABLE` `0` · `nccl:network` `IB`)이고 manifest 선언은 `nccl_transport: rdma` 였다.
