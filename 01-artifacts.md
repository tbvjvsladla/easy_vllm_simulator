# 01 · 산출물 — 무엇이 실제로 쓰였고, 어떻게 다시 띄우는가

## 1.1 적용 판정 표
> 기계가 적용 증거(빌드 원장 · 원장이 없으면 라벨된 재구성) × 파일 × 선언을 대사한 결과다. `artifacts/` 에는 **적용된 것만** 실렸다 — 빌드 때 스스로 skip 된 패치와 쓰이지 않은 평면의 파일은 표에만 남고 싣지 않는다.

<!-- FACT:slots -->
| 슬롯 | 실림 | 파일 | 신호 | 적용 증거 | 사유 |
|---|---|---|---|---|---|
| `build_patch_post` | 실림 | `artifacts/build_patch_post/10-deepgemm.sh` · `artifacts/build_patch_post/20-triton-kernels.sh` · `artifacts/build_patch_post/30-mxfp4-triton-sm121.sh` · `artifacts/build_patch_post/40-humming-nvml-gb10.sh` | 2-signal | none:no-build-ledger | - 공통: 1.1 의 적용 집합은 원장이 없어 재구성이다 — 10 은 applied(reconstructed, fail-loud 빌드 성공), 20 은 **unobserved**(적용 미관측), 30 · 40 은 applied(이미지 탐침: 스크립트 sha 와 적용 표지 관측). 네 파일의 관련성은 모두 `unknown` 이고, 이 중 어느 것도 이 모델의 요구라는 근거가 없다 — 이들은 공유 이미지의 arch-enablement 로 보이며 이 모델이 쓰는 경로(bf16 비양자화 MoE · triton)를 지나는지는 관측되지 않았다. - 10-deepgemm.sh: (a) DeepGEMM nv_dev 브랜치(헤더에 a6b593d)를 클론 · 빌드해 SM120 FP8 커널을 얻는다 — 헤더가 선언한 대상은… |
| `build_patch_pre` | 안 실림 | — | 1-signal | none:no-build-ledger | build_patches_src/ — 적용 판정 none: skipped 2(기계 파생) |
| `build_recipe` | 실림 | `artifacts/build_recipe/Dockerfile.source-build` · `artifacts/build_recipe/requirements.txt` | 2-signal | file:image-history:docker history build-arg 관측(VLLM_REF=v0.30.0 · VLLM_VERSION 없음 → 소스빌드 Dockerfile) — 셀 env 선택자 없음 · output/single/docker-compose.yaml 리터럴 build.dockerfile: Dockerfile.source-build(선택자 변수 없음 — compose build 는 늘 Dockerfile.source-build 를 짓는다 · 이 이미지를 지은 증거 아님) | - (a) 소스 빌드 레시피다. **그러나 실린 Dockerfile 은 이 이미지를 지은 바이트가 아니다** — 이미지 history 와 11/20 만 일치하고(1.1 경고), 파일 머리의 가드는 NGC 26.05 · vLLM 0.26.0 키를 가리키는데 이미지는 NGC 26.07 · v0.30.0 이다. requirements.txt 도 이미지 안 바이트와 다르다. (b) 재현 경로의 입력이지만 신뢰할 근거가 약하다(Q5). (c) 발화 개념이 아니라 빌드 입력이다. (d) 정확한 재현은 1.4 의 build-arg 와 docker history 를 기준으로 하고 이 파일은 참고용으로만 읽는다. |
| `compose` | 실림 | `artifacts/compose/.env.template` · `artifacts/compose/docker-compose.yaml` | 2-signal | file:output/single/docker-compose.yaml | - (a) 서빙 프로필(`vllm-serve`)이 NAS 읽기전용 마운트 · 컴파일 캐시 영속 마운트(`./cache/vllm` · `./cache/flashinfer`) · `ipc: host` · `oom_score_adj: 800` 을 건다. (b) 재현 경로 입력이다. (c) 측정 당시 개정(커밋 80284db86dd7)이 실렸다. (d) 호스트 OOM 우선 희생 지정(`oom_score_adj`)은 통합메모리에서 데스크톱 프로세스 대신 서빙 컨테이너를 먼저 잡으려는 것이며, 이 셀의 사살은 커널 OOM 이 아니라 호스트 워치독이었다(W2). `.env.template` 은 운영자 실값 대신 자리표시를 둔 형상 사본이다. |
| `fork_pin` | 안 실림 | — | 1-signal | none:.claude/policies/arch_variant_ledger.json | 셀 env 에 VARIANT 줄 없음 = stock(workflow.md §변종 좌표의 거처)(기계 파생) |
| `runtime_patch` | 안 실림 | — | 1-signal | none:output/single/configs/ornith15-35b-v030_patch.py | output/single/configs/ornith15-35b-v030_patch.py 없음 — arm_patch.sh 는 no-op(런타임 패치 불해당)(기계 파생) |
| `triplet` | 실림 | `artifacts/triplet/.env.ornith15-35b-v030` · `artifacts/triplet/ornith15-35b-v030.sh` · `artifacts/triplet/ornith15-35b-v030.yaml` | 3-signal | file:output/single/configs/ornith15-35b-v030.yaml | - (a) 서빙 인자 전부(모델 경로 · 컨텍스트 · KV 절대 클램프 · 시퀀스 · 파서 · MoE 백엔드)를 정한다. (b) W2 를 넘은 `moe-backend: triton` 이 여기 있다. (c) 10-10 기동의 실제 인자였다(스윕 색인 · 측정이 이 config 로 돌았다는 1.1 의 검증 칸). (d) 모델 · 버전 · 메모리 구조가 달라지면 값 대부분을 다시 도출한다(00 §0.5). 머리 주석 일부는 옛 판단이다(M1). · - (a) 컨테이너 안에서 `vllm serve --config <yaml> --served-model-name <env>` 를 실행하고, 파서 · KV · 컨텍스트는 yaml 이 소유한다는 주석을 둔다. 맨 앞에서 `arm_patch.sh` 가 있으면 source… |

**레시피 경고 · 실린 리비전**(기계 관측 — 수신자 빌드가 여기서 갈라질 수 있다)

- ⚠ `build_recipe` `output/single/Dockerfile.source-build` — 실린 바이트(worktree)가 측정 이미지 docker history 의 RUN·COPY 와 전수 일치하지 않는다(11/20)(이미지 Created 2026-09-30T23:53:22Z · 마지막 커밋 2026-09-08T06:19:07Z)
- ⚠ `build_recipe` `output/single/requirements.txt` — 이미지 안 COPY 대상 바이트가 실린 작업트리 바이트와 다르다(일치하는 개정 없음)(이미지 Created 2026-09-30T23:53:22Z · 마지막 커밋 2026-09-08T06:19:07Z)
- `build_recipe` `output/single/Dockerfile.source-build` 실린 리비전: 리비전 미특정 · 선택 방법 `worktree(어느 개정도 이미지 history 와 더 맞지 않는다 · 작업트리 11/20 — recipe_vs_image 경고 유지)` · docker history 대조 11/20 · 실린 바이트 `worktree` · 쓰인 바이트임을 관측으로 확인 아니오
- `build_recipe` `output/single/requirements.txt` 실린 리비전: 리비전 미특정 · 선택 방법 `worktree(이미지 안 바이트와 다르고 이미지 Created 이전 개정도 같지 않다 — recipe_vs_image 경고 · 이미지 sha256 39aaa69b2bafa6c1)` · docker history 대조 없음 · 실린 바이트 `worktree` · 쓰인 바이트임을 관측으로 확인 아니오
- `compose` `output/single/docker-compose.yaml` 실린 리비전: 커밋 `80284db86dd7` · 선택 방법 `worktree(mtime 2026-10-10T04:54:24Z ≤ 측정 2026-10-10T05:29:12Z(sweep_index.generated_utc) · 바이트 = 측정 전 마지막 커밋 80284db86dd7)` · docker history 대조 없음 · 실린 바이트 `worktree` · 근거 `mtime≤measured` · 쓰인 바이트임을 관측으로 확인 예

**싣지 않은 파일**(슬롯별 사유 — 빌드 때 skip 된 패치는 아래 적용 집합 표에 있다)

- `build_patch_post` known-non-patch 1건: `.gitkeep`
- `build_patch_pre` known-non-patch 1건: `.gitkeep`
- `compose` `arm_patch.sh` — `not-armed(output/single/configs/ornith15-35b-v030_patch.py 없음 — arm_patch.sh 는 no-op)`

**파일별 검증 · 관련성**(`generated-unverified` = 실행 검증되지 않은 생성물 — 파일 첫 줄 경고 · PAYLOAD 파일 기록과 같은 표시 · 관련성 = ① 패치 선언 대상 × 모델 config ② 엔진 로그 발화 서명 — 둘 다 관측일 때만 required/inactive-inferred)

| 슬롯 | 파일 | 검증 | 검증 근거 | 관련성 | 관련성 근거 |
|---|---|---|---|---|---|
| `build_patch_post` | `artifacts/build_patch_post/10-deepgemm.sh` | `verified` · cell-run | 적용 판정 none(no-build-ledger) · 원장 부재 — 라벨된 재구성: applied 3(image-probe(script-sha+marker) 2 · reconstructed(fail-loud+built) 1) · 적용 미관측 ['20-triton-kernels.sh'] · skip 판정 = docker-history+gate | `unknown` | ① 선언 ['DeepseekV4ForCausalLM'] ∌ 셀 모델(architectures ['Qwen3_5MoeForConditionalGeneration'] · model_type qwen3_5_moe) · 범용 선언 · ② 패치가 심는 logger 발화 서명 없음(발화 미관측) |
| `build_patch_post` | `artifacts/build_patch_post/20-triton-kernels.sh` | `verified` · cell-run | 적용 판정 none(no-build-ledger) · 원장 부재 — 라벨된 재구성: applied 3(image-probe(script-sha+marker) 2 · reconstructed(fail-loud+built) 1) · 적용 미관측 ['20-triton-kernels.sh'] · skip 판정 = docker-history+gate | `unknown` | ① 선언 ['DeepseekV4ForCausalLM'] ∌ 셀 모델(architectures ['Qwen3_5MoeForConditionalGeneration'] · model_type qwen3_5_moe) · 범용 선언 · ② 패치가 심는 logger 발화 서명 없음(발화 미관측) |
| `build_patch_post` | `artifacts/build_patch_post/30-mxfp4-triton-sm121.sh` | `verified` · cell-run | 적용 판정 none(no-build-ledger) · 원장 부재 — 라벨된 재구성: applied 3(image-probe(script-sha+marker) 2 · reconstructed(fail-loud+built) 1) · 적용 미관측 ['20-triton-kernels.sh'] · skip 판정 = docker-history+gate | `unknown` | ① 패치 헤더·본문에 대상 선언(model-trigger 아키텍처 · vllm/models/<model_type>) 없음 · 편집하는 공유(모델 디렉터리 밖) 파일 ['vllm/model_executor/layers/fused_moe/experts/gpt_oss_triton_kernels_moe.py'] |
| `build_patch_post` | `artifacts/build_patch_post/40-humming-nvml-gb10.sh` | `verified` · cell-run | 적용 판정 none(no-build-ledger) · 원장 부재 — 라벨된 재구성: applied 3(image-probe(script-sha+marker) 2 · reconstructed(fail-loud+built) 1) · 적용 미관측 ['20-triton-kernels.sh'] · skip 판정 = docker-history+gate | `unknown` | ① 패치 헤더·본문에 대상 선언(model-trigger 아키텍처 · vllm/models/<model_type>) 없음 |
| `build_recipe` | `artifacts/build_recipe/Dockerfile.source-build` | `generated-unverified` · renderer | 실린 Dockerfile.source-build(worktree)이 측정 이미지 docker history RUN·COPY 와 11/20 만 일치 — 이 이미지를 지은 바이트임을 관측으로 확인하지 못했다(_select_dockerfile · worktree(어느 개정도 이미지 history 와 더 맞지 않는다 · 작업트리 11/20 — recipe_vs_image 경고 유지)) | `unknown` | 이 셀은 이 경로로 실행되지 않았다(generated-unverified · 생성 renderer) — 실행 기록이 없어 필요 여부를 판정하지 않는다(참고 형상) |
| `build_recipe` | `artifacts/build_recipe/requirements.txt` | `verified` · cell-run | 쓰인 레시피(image-history:docker history build-arg 관측(VLLM_REF=v0.30.0 · VLLM_VERSION 없음 → 소스빌드 Dockerfile) — 셀 env 선택자 없음 · output/single/docker-compose.yaml 리터럴 build.dockerfile: Dockerfile.source-build(선택자 변수 없음 — compose build 는 늘 Dockerfile.source-build 를 짓는다 · 이 이미지를 지은 증거 아님)) — 개정 선택 slots.build_recipe.evidence.selected_revisions | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/.env.template` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/docker-compose.yaml` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/.env.ornith15-35b-v030` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/ornith15-35b-v030.sh` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/ornith15-35b-v030.yaml` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
<!-- /FACT:slots -->

<!-- FACT:applied_set -->
**적용 집합** — 상태 `unobservable` · 출처 `no-build-ledger` · 원장 없음: 아래 결과는 **라벨된 재구성**이다(관측 아님)

| 단계 | 파일 | 결과 | 결과 출처 | 사유 | 실림 | model-trigger |
|---|---|---|---|---|---|---|
| pre | `50-dsv4-sm12x-port.sh` | skipped | reconstructed(docker-history+gate) | SM12X_PORT=0(docker history) — 자기게이트 `!= 1 → exit 0` | 안 실림 | DeepseekV4ForCausalLM(0731 정식판) + `--speculative-config method=dspark`. |
| pre | `55-src-deps-authority.sh` | skipped | reconstructed(docker-history+gate) | SM12X_PORT=0 · SRC_DEPS_AUTHORITY=0(docker history) — 자기게이트 `!= 1 → exit 0` | 안 실림 | — |
| post | `10-deepgemm.sh` | applied | reconstructed(fail-loud+built) | skip 경로 없음(set -e · 성공 종료 exit·skip 문구·상태 프로토콜 없음) × docker history 루프 `/tmp/build_patches/*.sh … \|\| exit 1`(비-0 = 빌드 중단) × 측정 이미지 실재 × 판정한 바이트 = 이미지에 구워진 바이트 — 빌드가 끝났으므로 끝까지 돌았다 | 실림 | DeepseekV4ForCausalLM FP8 block-scale dense/MoE (SM12x). 범용 arch-enablement(모델-키잉 ✗ — 이미지 네이밍 불변식). |
| post | `20-triton-kernels.sh` | unobserved(적용 미관측) | none | 원장 부재 · 정적 판정 불가(실패 삼킴 `\|\| true`·`\|\| :`(정리 명령 밖 — set -e 가 그 실패를 못 본다) L[26]; skip 문구(내용 조건부 skip 경로) L[24, 25]) · 스크립트가 선언한 적용 마커 없음 — 이미지 탐침으로도 가르지 못한다 — 적용 미관측 | 실림 | DeepseekV4ForCausalLM (MXFP4 MoE 74GiB/node — MARLIN repack 미적합). 범용(MXFP4 MoE 의 TRITON 경로 필요 모델 공통). |
| post | `30-mxfp4-triton-sm121.sh` | applied | image-probe(script-sha+marker) | 이미지 탐침: 판정한 스크립트 = 이미지에 구워진 바이트 · 선언 마커 1/1 가 대상 파일에 있다(패치 **후** 상태 관측 — 이 빌드가 썼는지 베이스가 이미 가졌는지는 가르지 않는다) · 정적 판정 불가 사유: skip 문구(내용 조건부 skip 경로) L[28] | 실림 | — |
| post | `40-humming-nvml-gb10.sh` | applied | image-probe(script-sha+marker) | 이미지 탐침: 판정한 스크립트 = 이미지에 구워진 바이트 · 선언 마커 2/2 가 대상 파일에 있다(패치 **후** 상태 관측 — 이 빌드가 썼는지 베이스가 이미 가졌는지는 가르지 않는다) · 정적 판정 불가 사유: 내장 파이썬 성공 조기 종료 L[55]; skip 문구(내용 조건부 skip 경로) L[55] | 실림 | — |

**결과 출처 범례**

- `image-probe(script-sha+marker)` — 이미지 안 패치 사본이 실린 파일과 바이트 동일하고 그 패치의 적용 표지가 이미지 트리에 있다(관측)
- `none` — 적용 결과를 관측하지도 재구성하지도 못했다
- `reconstructed(docker-history+gate)` — docker history 의 build-arg 와 패치 자기게이트 규칙으로 재구성했다(관측 아님)
- `reconstructed(fail-loud+built)` — skip 경로가 없고 실패하면 빌드를 멈추는(fail-loud) 패치인데 빌드가 성공했다 — 적용으로 재구성했다(관측 아님)

**판정 근거**(패치별 — artifacts 가 판정에 쓴 관측 · 스크립트 바이트 정체 = 실린 바이트가 측정 이미지에 구워진 바이트인가)

| 파일 | 스크립트 바이트 정체 | 이미지 사본 sha256 | 적용 표지(상태 찾음/선언) | 정적 규칙 | 원장 분류 |
|---|---|---|---|---|---|
| `50-dsv4-sm12x-port.sh` | image-probe(작업트리에 없음 — 이미지 바이트로 판정 · 이미지 바이트를 싣는다) | `784cd57f20a070b7dee8a3aa4c1cc516b513d46ea161d735b9d43eca81bc77bb` | — | — | — |
| `55-src-deps-authority.sh` | image-probe(작업트리에 없음 — 이미지 바이트로 판정 · 이미지 바이트를 싣는다) | `1f2aed56d94233b341157370ce4cebefd34e15979d0e90e5d5fed4d3ef4467a6` | — | — | — |
| `10-deepgemm.sh` | image-probe(작업트리에 없음 — 이미지 바이트로 판정 · 이미지 바이트를 싣는다) | `bdfd5412c3b7a58133006436bc2c9960a2433a67d92277a2b12b63d1364c7589` | no-declared-markers 0/0 | skip 경로 없음 | — |
| `20-triton-kernels.sh` | image-probe(작업트리에 없음 — 이미지 바이트로 판정 · 이미지 바이트를 싣는다) | `e263f424720d666e7915aa2d07d0c4878257ad04b45ade39fabdf67aa3ff2824` | no-declared-markers 0/0 | 실패 삼킴 `\|\| true`·`\|\| :`(정리 명령 밖 — set -e 가 그 실패를 못 본다) L[26] · skip 문구(내용 조건부 skip 경로) L[24, 25] | — |
| `30-mxfp4-triton-sm121.sh` | image-probe(작업트리에 없음 — 이미지 바이트로 판정 · 이미지 바이트를 싣는다) | `161dce749a830f55c6c1b43a643929794006b8e5bfa3d55dd6e99654e5958a39` | all-found 1/1 | skip 문구(내용 조건부 skip 경로) L[28] | — |
| `40-humming-nvml-gb10.sh` | image-probe(작업트리에 없음 — 이미지 바이트로 판정 · 이미지 바이트를 싣는다) | `a9f1a94390997c669c3e469debdaa28f0e581910f1f0ed3432dfd8798c4f5c2e` | all-found 2/2 | 내장 파이썬 성공 조기 종료 L[55] · skip 문구(내용 조건부 skip 경로) L[55] | — |

- `post` 빌드 루프: docker history 루프 `/tmp/build_patches/*.sh … \|\| exit 1`(비-0 = 빌드 중단)

**원장 탐침**(순위: attestation → 메인 로컬 이미지 → 재구성)

| 순위 | 결과 | 대상 |
|---|---|---|
| attestation | absent | output/single/benchlog/attestation_ornith15-35b-v030.json(부재) |
| local-image | skipped(node=sub — 서브 스캔 ✗ · 문서기반 회수만) | — |
| reconstruction | done | — |
| image-probe | done | sha256:055ce79184da480044540e62d385307a522d9e087b6726c51f2099e8a3d10705 |

**재구성 입력** — `docker-history+gate-rule · fail-loud-rule · image-probe`(docker history build-arg × 패치 자기게이트 규칙)

| build-arg | 값 |
|---|---|
| `BUILD_JOBS` | 8 |
| `RAY_VERSION` | 2.48.0 |
| `SM12X_PORT` | 0 |
| `SRC_DEPS_AUTHORITY` | 0 |
| `TORCH_CUDA_ARCH` | 12.1a |
| `VLLM_PRETEND_VERSION` | (빈 값) |
| `VLLM_REF` | v0.30.0 |
| `VLLM_REPO` | https://github.com/vllm-project/vllm.git |
<!-- /FACT:applied_set -->

## 1.2 슬롯별 적용 사유
> 이 절이 답하는 질문: 실린 산출물 하나하나는 무엇을 고치며, 왜 이 셀에 필요했고, 이 셀에서 실제로 발화했으며, 언제 불필요해지는가?

아래 항목의 '발화' 칸은 이 셀의 엔진 로그에서 그 파일이 심는 서명을 찾지 못했다는 뜻이며, 1.1 표의 적용 판정(applied · unobserved)을 바꾸지 않는다. 이 계보의 벽(W1~W3) 가운데 아래 어느 파일에 의존해 넘은 것도 없다 — 벽을 넘은 수단은 서빙 yaml 의 `moe-backend` 한 줄(W2)이다.

### triplet / ornith15-35b-v030.yaml
- (a) 서빙 인자 전부(모델 경로 · 컨텍스트 · KV 절대 클램프 · 시퀀스 · 파서 · MoE 백엔드)를 정한다. (b) W2 를 넘은 `moe-backend: triton` 이 여기 있다. (c) 10-10 기동의 실제 인자였다(스윕 색인 · 측정이 이 config 로 돌았다는 1.1 의 검증 칸). (d) 모델 · 버전 · 메모리 구조가 달라지면 값 대부분을 다시 도출한다(00 §0.5). 머리 주석 일부는 옛 판단이다(M1).

### triplet / ornith15-35b-v030.sh
- (a) 컨테이너 안에서 `vllm serve --config <yaml> --served-model-name <env>` 를 실행하고, 파서 · KV · 컨텍스트는 yaml 이 소유한다는 주석을 둔다. 맨 앞에서 `arm_patch.sh` 가 있으면 source 한다. (b) 필수 실행 경로다. (c) `ornith15-35b-v030_patch.py` 가 없어 arm_patch 는 no-op 이다(1.1 `runtime_patch`). (d) 서빙 인자를 CLI 로 옮기는 구성이 생기면 불필요하다.

### triplet / .env.ornith15-35b-v030
- (a) 컨테이너 이름 · 포트 · served model name · CONFIG_FILE · IMAGE_TAG 를 정한다(포트 값과 이미지 태그는 파일 안). (b) compose 의 `env_file` 이 읽는다. (c) 10-10 기동이 이 env 로 돌았다. (d) 이미지 태그는 이미지가 바뀌면 갱신한다.

### compose / docker-compose.yaml · .env.template
- (a) 서빙 프로필(`vllm-serve`)이 NAS 읽기전용 마운트 · 컴파일 캐시 영속 마운트(`./cache/vllm` · `./cache/flashinfer`) · `ipc: host` · `oom_score_adj: 800` 을 건다. (b) 재현 경로 입력이다. (c) 측정 당시 개정(커밋 80284db86dd7)이 실렸다. (d) 호스트 OOM 우선 희생 지정(`oom_score_adj`)은 통합메모리에서 데스크톱 프로세스 대신 서빙 컨테이너를 먼저 잡으려는 것이며, 이 셀의 사살은 커널 OOM 이 아니라 호스트 워치독이었다(W2). `.env.template` 은 운영자 실값 대신 자리표시를 둔 형상 사본이다.

### build_recipe / Dockerfile.source-build · requirements.txt
- (a) 소스 빌드 레시피다. **그러나 실린 Dockerfile 은 이 이미지를 지은 바이트가 아니다** — 이미지 history 와 11/20 만 일치하고(1.1 경고), 파일 머리의 가드는 NGC 26.05 · vLLM 0.26.0 키를 가리키는데 이미지는 NGC 26.07 · v0.30.0 이다. requirements.txt 도 이미지 안 바이트와 다르다. (b) 재현 경로의 입력이지만 신뢰할 근거가 약하다(Q5). (c) 발화 개념이 아니라 빌드 입력이다. (d) 정확한 재현은 1.4 의 build-arg 와 docker history 를 기준으로 하고 이 파일은 참고용으로만 읽는다.

### build_patch_post / 10-deepgemm.sh · 20-triton-kernels.sh · 30-mxfp4-triton-sm121.sh · 40-humming-nvml-gb10.sh
- 공통: 1.1 의 적용 집합은 원장이 없어 재구성이다 — 10 은 applied(reconstructed, fail-loud 빌드 성공), 20 은 **unobserved**(적용 미관측), 30 · 40 은 applied(이미지 탐침: 스크립트 sha 와 적용 표지 관측). 네 파일의 관련성은 모두 `unknown` 이고, 이 중 어느 것도 이 모델의 요구라는 근거가 없다 — 이들은 공유 이미지의 arch-enablement 로 보이며 이 모델이 쓰는 경로(bf16 비양자화 MoE · triton)를 지나는지는 관측되지 않았다.
- 10-deepgemm.sh: (a) DeepGEMM nv_dev 브랜치(헤더에 a6b593d)를 클론 · 빌드해 SM120 FP8 커널을 얻는다 — 헤더가 선언한 대상은 DeepSeek-V4 FP8. (d) 업스트림 정식 지원 시 불필요라는 판단은 헤더에 없고 계보에서도 확인되지 않는다.
- 20-triton-kernels.sh: (a) NGC 번들의 불완전한 standalone `triton_kernels` 를 제거해 vLLM 의 vendored 완전본이 활성화되게 한다. 헤더 선언 대상은 DeepSeek-V4 MXFP4 MoE. 적용 미관측.
- 30-mxfp4-triton-sm121.sh: (a) `_triton_kernel_moe_supports_current_device()` 의 capability 창을 SM120/121 포함으로 넓힌다(헤더). 헤더 선언 대상은 MXFP4 expert. 이 셀의 가중치는 비양자화라 이 게이트를 지나는지 미확인.
- 40-humming-nvml-gb10.sh: (a) NVML 클록 쿼리 2개를 try/except 로 가드한다(GB10 은 메모리/SM 클록을 NVML 로 노출하지 않는다는 헤더 설명). 헤더 선언 대상은 HUMMING MoE 백엔드. 이 셀이 HUMMING 을 쓰는지는 미확인.

### 싣지 않은 슬롯
- `build_patch_pre` 2건(50-dsv4-sm12x-port · 55-src-deps-authority): build-arg `SM12X_PORT=0` · `SRC_DEPS_AUTHORITY=0` 으로 자기게이트 skip(재구성). `fork_pin`: 셀 env 에 VARIANT 줄 없음 = stock. `runtime_patch`: 패치 파일 없음, arm_patch no-op.


## 1.3 값의 지위표
> 서빙 설정의 노브 **전부**와 기계가 모은 후보 지위다(lockset 출처 · yaml 주석 · 엔진 기본값). 최종 지위는 아래 `kind: value-status` 블록이 정한다 — 표의 노브마다 블록이 정확히 하나 있어야 린터를 통과한다.

<!-- FACT:value_status -->
서빙 노브 11개 — 노브마다 `kind: value-status` 블록이 **정확히 1개** 있어야 한다(노브·값은 이 표의 글자 그대로 · 값 칸의 `\|` 는 `|` 로 적는다).

| 노브 | 값 | 후보 지위 | 후보 근거 | yaml 주석 | 출처 |
|---|---|---|---|---|---|
| `gpu-memory-utilization` | 0.80 | inherited | lockset 출처 gmu_source=hand · lockset provenance=hand-authored | — | lockset.gmu_source=hand |
| `max-model-len` | 262144 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `max-num-seqs` | 4 | inherited | lockset 출처 batch_source=hand-lever · lockset provenance=hand-authored | — | lockset.batch_source=hand-lever |
| `max-num-batched-tokens` | 4096 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `limit-mm-per-prompt` | {"image": 0, "video": 0} | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `kv-cache-memory-bytes` | 8589934592 | inherited | lockset 출처 kv_source=hand · lockset provenance=hand-authored | — | lockset.kv_source=hand |
| `tensor-parallel-size` | 1 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `enable-auto-tool-choice` | true | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `tool-call-parser` | qwen3_xml | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `reasoning-parser` | qwen3 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `moe-backend` | triton | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
<!-- /FACT:value_status -->

> 이 절이 답하는 질문: 각 서빙 노브의 값은 이 셀에서 어떤 지위인가 — 조정됐나, 승계됐나, 음성대조로 일부러 둔 것인가, 엔진 기본값인가, 필요조건인가?

```hint-event
id: V1
kind: value-status
노브: gpu-memory-utilization
값: 0.80
지위: inherited
근거: 손으로 정한 값(lockset gmu_source=hand · 손레버). yaml 주석의 산식으로 정했고 값을 바꿔 가며 잰 기록이 없다. 실제 KV 는 kv-cache-memory-bytes 클램프가 정한다
출처: [ornith15-35b-v030.yaml, testlog_26100111_ornith15-35b-v030_싱글서브_서빙_lite §2]
```

```hint-event
id: V2
kind: value-status
노브: max-model-len
값: 262144
지위: inherited
근거: 체크포인트 네이티브 컨텍스트(yaml 머리 주석 ctx 262144)를 그대로 썼다. 줄여 본 기록 없음
출처: [ornith15-35b-v030.yaml, testlog_26100111_ornith15-35b-v030_싱글서브_서빙_lite §2]
```

```hint-event
id: V3
kind: value-status
노브: max-num-seqs
값: 4
지위: inherited
근거: 손레버(lockset batch_source=hand-lever). 시도 3 에서 8 → 4 로 줄였으나 다른 변인과 동시에 바꿨고 값별 측정은 없다
출처: [ornith15-35b-v030.yaml, testlog_26100111_ornith15-35b-v030_싱글서브_서빙_lite §3]
```

```hint-event
id: V4
kind: value-status
노브: max-num-batched-tokens
값: 4096
지위: inherited
근거: 시도 3 에서 정했고 이후 조정된 적 없다. 값별 측정 없음
출처: [ornith15-35b-v030.yaml, testlog_26100111_ornith15-35b-v030_싱글서브_서빙_lite §3]
```

```hint-event
id: V5
kind: value-status
노브: limit-mm-per-prompt
값: '{"image": 0, "video": 0}'
지위: inherited
근거: 텍스트 전용 서빙 선택. yaml 주석은 사살 회피 목적이라 적었지만 시도 3 이 이 값을 켠 채로도 사살돼 그 근거는 기각됐다(M1). 켜면 깨지는지는 triton 위에서 시험되지 않았다
출처: [ornith15-35b-v030.yaml, testlog_26100111_ornith15-35b-v030_싱글서브_서빙_lite §3]
```

```hint-event
id: V6
kind: value-status
노브: kv-cache-memory-bytes
값: 8589934592
지위: inherited
근거: 손으로 정한 절대 클램프(lockset kv_source=hand). 16 GiB → 8 GiB 로 줄였고(시도 3) 값별 측정은 없다. 이 값에서 엔진 로그가 408,122 토큰 · 262,144 토큰 요청 기준 동시성 1.56x 를 보고한다
출처: [ornith15-35b-v030.yaml, testlog_26100111_ornith15-35b-v030_싱글서브_서빙_lite §2, testlog_26100111_ornith15-35b-v030_싱글서브_서빙_lite §4]
```

```hint-event
id: V7
kind: value-status
노브: tensor-parallel-size
값: 1
지위: inherited
근거: GPU 1장 노드라 선택지가 없다. 조정 대상이 아니다
출처: [ornith15-35b-v030.yaml]
```

```hint-event
id: V8
kind: value-status
노브: enable-auto-tool-choice
값: true
지위: inherited
근거: 도구 호출 파서와 짝으로 켠 값. 10-10 · 10-01 측정에서 도구 호출을 시험한 기록은 없다
출처: [ornith15-35b-v030.yaml]
```

```hint-event
id: V9
kind: value-status
노브: tool-call-parser
값: qwen3_xml
지위: inherited
근거: 선택 근거가 계보에 없다. 도구 호출을 시험한 기록이 없어 동작 미확인
출처: [ornith15-35b-v030.yaml]
```

```hint-event
id: V10
kind: value-status
노브: reasoning-parser
값: qwen3
지위: inherited
근거: 스모크에서 reasoning 필드가 분리됨(17*23 질의)을 1회 관측했다. 이 값을 빼 본 기록은 없다
출처: [ornith15-35b-v030.yaml, testlog_26100111_ornith15-35b-v030_싱글서브_서빙_lite §4]
```

```hint-event
id: V11
kind: value-status
노브: moe-backend
값: triton
지위: declared-requirement
근거: auto(FlashInfer CUTLASS)에서는 3회 사살, 시도 3 에 이 한 변인만 더한 시도 4 가 통과(W2). 호스트 워치독(절대 임계 10240 MiB와 속도 예측 규칙 둘, 00 §0.2) 아래에서의 관측이며 워치독이 없는 호스트에서의 거동은 관측 없음
출처: [testlog_26100111_ornith15-35b-v030_싱글서브_서빙_lite §3, ornith15-35b-v030.yaml]
```

요약: 11개 노브 중 `moe-backend` 만 '빼면 실패가 관측된 필요조건'이다. 나머지는 값을 바꿔 가며 잰 기록이 없는 승계값이거나 손레버이며 최적이라는 뜻이 아니다. 복사하면 위험한 값은 `limit-mm-per-prompt`(yaml 주석이 옛 판단 — M1)와 `kv-cache-memory-bytes`(통합메모리 호스트의 예산 산식에 묶인 값)이다. 도구 호출 파서 두 노브는 스모크에서 시험되지 않았다.


## 1.4 재현 절차
> 셀 실행 기록에서 기계가 파생한 명령 순서와 단계별 소요다. 소요가 `미관측` 인 단계는 시각 기록이 없다는 뜻이다(0 이 아니다). 그 아래는 블랙박스 이벤트 원장에서 이 셀의 행을 기계가 뽑아 기동 시도로 묶은 것이고, 마지막은 측정한 실행의 엔진 로그가 echo 한 env 다(측정 당시 값의 정본).

<!-- FACT:reproduce -->
| 순서 | 단계 | 성공 판정 | 소요 | 관측 구간(UTC) | 소요 출처 | 명령 출처 |
|---|---|---|---|---|---|---|
| 1 | build | 빌드 OK · 이미지 실재 | 층 창 40분 42초(docker history 첫 층 → 끝 층 CreatedAt · 캐시 재사용 층 포함 — 빌드 소요 아님) | 2026-09-30T23:12:40Z → 2026-09-30T23:53:22Z (layer-window) | docker history --no-trunc --format '{{json .}}' sha256:055ce79184da480044540e62d385307a522d9e087b6726c51f2099e8a3d10705 · 우리 Dockerfile 층 23개의 CreatedAt 첫 층 → 끝 층(캐시 재사용 층은 이전 빌드 시각 — 빌드 소요 아님) | reconstructed(docker-history build-args + compose build) |
| 2 | serve | health 200 + 추론 1회(스모크 판정) | ≤ 6분 57초(상한) | 2026-10-10T05:05:03Z → 2026-10-10T05:12:00Z (upper) | docs/logs/sub/events/2026-10.jsonl budget_declare(label serve-ornith15-35b-v030) → 첫 측정 시각(output/single/benchlog/sweep_ornith15-35b-v030/lite_raw_ornith15-35b-v030.json measured_utc(호스트 date -u · lite_bench.sh 가 cold 레그 직전에 찍는 측정 시작)) | derived(스모크 사용법) |
| 3 | bench | 리포트 발행(+PASS 면 인증서) | 17분 12초 | 2026-10-10T05:12:00Z → 2026-10-10T05:29:12Z (exact) | output/single/benchlog/sweep_ornith15-35b-v030/lite_raw_ornith15-35b-v030.json measured_utc(호스트 date -u · lite_bench.sh 가 cold 레그 직전에 찍는 측정 시작) → sweep_index.generated_utc(호스트 UTC · 두 끝 같은 시계) | reconstructed(bench json) |

**빌드 층 시각**(관측 · 소요가 아니라 층 창 — 캐시 재사용 층은 이전 빌드의 시각을 가진다)

- `build` 이미지 층 CreatedAt(docker history): 2026-09-30T23:12:40Z → 2026-09-30T23:53:22Z · 층 23개 · 출처 `docker history --no-trunc --format '{{json .}}' sha256:055ce79184da480044540e62d385307a522d9e087b6726c51f2099e8a3d10705` — 우리 Dockerfile 층의 CreatedAt 범위 — 캐시 재사용 층은 이전 빌드 시각을 가진다(소요 아님)

**단계별 명령**(기계 파생 — `명령 출처` 가 `derived(…)` 면 사용법에서 파생 · `reconstructed(…)` 면 괄호 안 관측(docker history build-arg · compose build · bench JSON 필드)에서 재구성한 명령이다. 어느 쪽도 실행 원문(로그에 남은 줄)은 아니다)

**1. build**

```sh
# 실린 artifacts/build_recipe/Dockerfile.source-build(실린 개정: worktree — 이미지를 지은 바이트임을 관측으로 확인 못함(docker history RUN·COPY 11/20))를 output/single/Dockerfile.source-build 자리에 두고 컨텍스트에서 짓는다.
docker build -f output/single/Dockerfile.source-build \
  --build-arg BUILD_JOBS=8 \
  --build-arg RAY_VERSION=2.48.0 \
  --build-arg SM12X_PORT=0 \
  --build-arg SRC_DEPS_AUTHORITY=0 \
  --build-arg TORCH_CUDA_ARCH=12.1a \
  --build-arg VLLM_PRETEND_VERSION= \
  --build-arg VLLM_REF=v0.30.0 \
  --build-arg VLLM_REPO=https://github.com/vllm-project/vllm.git \
  -t easy-vllm:0.30.0-cu133-aarch64-source \
  output/single
# 동등 경로(compose): docker compose -f output/single/docker-compose.yaml --env-file output/single/envs/.env.ornith15-35b-v030 build
```

**2. serve**

```sh
bash .claude/skills/upstream-version-watch/scripts/single_serve_up.sh ornith15-35b-v030
```

**3. bench**

```sh
# bench JSON 필드로 복원: backend · model_id→--model · tokenizer_id→--tokenizer · num_prompts · max_concurrency · request_rate · burstiness · 요청당 입출력 길이 = total_*_tokens ÷ completed(나누어떨어질 때만 — 평균이다 · 요청별 길이는 JSON 에 없다) · 나누어떨어지면 `--dataset-name random` 으로 추정했다(측정 도구가 그 꼴로 부른다 — 아래 도구 원문).
# JSON 에 없는 인자(--endpoint · --base-url · --ignore-eos · --num-warmups · --temperature · --seed · --random-range-ratio · --trust-remote-code)는 측정 도구가 정했다 — level_* = run_bench.sh · lite_cold/lite_warm = lite_bench.sh(.claude/skills/adversarial-benchmark/scripts/) · 셀 스윕 순서는 sweep_bench.sh.
# output/single/benchlog/sweep_ornith15-35b-v030/lite_cold_ornith15-35b-v030.json (date 20261010-051217) · ⚠ --random-input-len/--random-output-len = total ÷ completed 평균(chat 템플릿 포함 실측 토큰 · 측정 도구 원문 인자 미관측 — lite_bench.sh 원문 인자와 다를 수 있다)
vllm bench serve --backend openai-chat --model ornith-1.5-35b-a3b --tokenizer /app/models/ornith-ai/Ornith-1.5-35B-A3B --dataset-name random --random-input-len 524 --random-output-len 128 --num-prompts 1 --max-concurrency 1 --request-rate inf
# output/single/benchlog/sweep_ornith15-35b-v030/lite_warm_ornith15-35b-v030.json (date 20261010-051244) · ⚠ --random-input-len/--random-output-len = total ÷ completed 평균(chat 템플릿 포함 실측 토큰 · 측정 도구 원문 인자 미관측 — lite_bench.sh 원문 인자와 다를 수 있다)
vllm bench serve --backend openai-chat --model ornith-1.5-35b-a3b --tokenizer /app/models/ornith-ai/Ornith-1.5-35B-A3B --dataset-name random --random-input-len 523 --random-output-len 128 --num-prompts 3 --max-concurrency 1 --request-rate inf
```

**슬롯 → 빌드 컨텍스트 매핑**(빌드 컨텍스트 = `output/<토폴로지>/` · 원천 = 실린 Dockerfile 의 COPY · compose 의 env_file · volumes · command — `—` 는 빌드 컨텍스트 입력이 아니다)

| zip 경로 | 빌드 컨텍스트 자리 | 근거 |
|---|---|---|
| `artifacts/triplet/.env.ornith15-35b-v030` | `envs/.env.ornith15-35b-v030` | compose env_file envs/.env.${CONFIG_FILE}(셀 env) |
| `artifacts/triplet/ornith15-35b-v030.sh` | `configs/ornith15-35b-v030.sh` | compose volumes ./configs:/app/configs |
| `artifacts/triplet/ornith15-35b-v030.yaml` | `configs/ornith15-35b-v030.yaml` | compose volumes ./configs:/app/configs |
| `artifacts/build_patch_post/10-deepgemm.sh` | `build_patches/10-deepgemm.sh` | Dockerfile.source-build COPY build_patches/ |
| `artifacts/build_patch_post/20-triton-kernels.sh` | `build_patches/20-triton-kernels.sh` | Dockerfile.source-build COPY build_patches/ |
| `artifacts/build_patch_post/30-mxfp4-triton-sm121.sh` | `build_patches/30-mxfp4-triton-sm121.sh` | Dockerfile.source-build COPY build_patches/ |
| `artifacts/build_patch_post/40-humming-nvml-gb10.sh` | `build_patches/40-humming-nvml-gb10.sh` | Dockerfile.source-build COPY build_patches/ |
| `artifacts/build_recipe/Dockerfile.source-build` | `Dockerfile.source-build` | compose build.dockerfile Dockerfile.source-build(리터럴) · build.context . |
| `artifacts/build_recipe/requirements.txt` | `requirements.txt` | Dockerfile.source-build COPY requirements.txt |
| `artifacts/compose/.env.template` | `.env` | compose 변수치환 프로젝트 .env(${…} 참조) |
| `artifacts/compose/docker-compose.yaml` | `docker-compose.yaml` | compose 파일(빌드 컨텍스트 루트) |

**기동 성공 판정(관측 · 발행 자격)**: health 200 = true · 추론 1회 = true · 근거 `output/single/benchlog/sweep_ornith15-35b-v030/level_01/post_health_ornith15-35b-v030.json` · `output/single/benchlog/sweep_ornith15-35b-v030/level_01/guidellm_ornith15-35b-v030.json`
<!-- /FACT:reproduce -->

<!-- FACT:event_timeline -->
**블랙박스 이벤트 56행**(원장에서 이 셀 label·config 에 맞는 행과 그와 시간이 겹치는 같은 노드의 다른 라벨 창 · 시각순 · 기계 발췌 · 이번 캠페인 15행 · 같은 셀 이름 · 앞선 캠페인 41행(내용 칸에 표기) · 라벨 다름(같은 노드 · 창 밖 이 셀 행과 시간이 겹치는 다른 라벨 선언 창) 31행 — 이 셀의 선언으로 세지 않는다)

> **캠페인 경계** 2026-10-10T05:05:03Z(`camp-26101104-hint-cd` · evidence.event_campaign_boundary): campaigns/camp-26101104-hint-cd/campaign.yaml declared_utc 2026-10-10T19:53:02Z · 이 발행이 묶인 측정을 서빙한 budget_declare 창 시작 2026-10-10T05:05:03Z → 경계 = 앞선 값 2026-10-10T05:05:03Z(이전 행 = 같은 셀 이름 · 앞선 캠페인)

> **원장 관측 범위**(노드별 첫 행 → 마지막 행 · evidence.event_ledger_spans): sub = 2026-06-13T22:25:01Z → 2026-10-10T15:17:46Z(`docs/logs/sub/events/2026-06.jsonl` · `docs/logs/sub/events/2026-07.jsonl` · `docs/logs/sub/events/2026-09.jsonl` · `docs/logs/sub/events/2026-10.jsonl` · `docs/logs/sub/events/thermal.jsonl` · `docs/logs/sub/events/watchdog.jsonl`)

| # | UTC | 노드 | kind | label | 내용 | 출처 |
|---|---|---|---|---|---|---|
| 1 | 2026-10-01T01:43:16Z | sub | `budget_declare` | `ornith15-v030` | 예산 선언(선언값 — 측정 아님) · floor_mib=23268 · weights_mib=68572 · kv_mib=16384 · overhead_mib=16384 · mem_total_mib=124608 · 라벨 다름: `ornith15-v030` — 같은 노드 · 이 셀 라벨(`ornith15-35b-v030`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · budget_renew 0회 · 닫힘 replaced(budget_declare) 2026-10-01T01:47:22Z(선언 후 4m06s) · measured_utc 2026-10-10T05:29:12Z 는 이 창 밖(측정 전) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:05:03Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:3 |
| 2 | 2026-10-01T01:43:16Z | sub | `serve_start` | — | serve_start · model=ornith-1.5-35b-a3b · tensor_parallel_size=1 · topology=single · session=serve-ornith15-35b-v030-20261001T014316Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:05:03Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:4 |
| 3 | 2026-10-01T01:43:17Z | sub | `budget_honored` | `ornith15-v030` | budget_honored · floor_mib=23268 · arm_ceiling_mib=20196 · remaining_s=7199 · 창 시작 2026-10-01T01:43:17Z 후 0m00s · 라벨 다름: `ornith15-v030` — 같은 노드 · 이 셀 라벨(`ornith15-35b-v030`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:05:03Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:374 |
| 4 | 2026-10-01T01:47:22Z | sub | `budget_declare` | `ornith15-v030` | 예산 선언(선언값 — 측정 아님) · floor_mib=23268 · weights_mib=68572 · kv_mib=16384 · overhead_mib=16384 · mem_total_mib=124608 · 라벨 다름: `ornith15-v030` — 같은 노드 · 이 셀 라벨(`ornith15-35b-v030`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · budget_renew 1회 · 닫힘 budget_clear 2026-10-01T01:58:12Z(선언 후 10m50s) · measured_utc 2026-10-10T05:29:12Z 는 이 창 밖(측정 전) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:05:03Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:5 |
| 5 | 2026-10-01T01:47:22Z | sub | `serve_start` | — | serve_start · model=ornith-1.5-35b-a3b · tensor_parallel_size=1 · topology=single · session=serve-ornith15-35b-v030-20261001T014722Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:05:03Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:6 |
| 6 | 2026-10-01T01:47:23Z | sub | `budget_renew` | `ornith15-v030` | budget_renew · floor_mib=23268 · ttl_s=7200 · remaining_before_s=7199 · 창 시작 2026-10-01T01:47:22Z 후 0m01s · 라벨 다름: `ornith15-v030` — 같은 노드 · 이 셀 라벨(`ornith15-35b-v030`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:05:03Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:7 |
| 7 | 2026-10-01T01:48:02Z | sub | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=78353 · rate_mib_s=27581 · max_rate_mib_s=17801 · legacy_rule_would_trip=true · 라벨 없음 → ornith15-v030 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-01T01:43:17Z 후 4m45s) · 라벨 다름: `ornith15-v030` — 같은 노드 · 이 셀 라벨(`ornith15-35b-v030`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:05:03Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:375 |
| 8 | 2026-10-01T01:58:07Z | sub | `watchdog_trip` | — | watchdog_trip · mem_avail_mib=10022 · rule=absolute · threshold_mib=10240 · action=docker_kill · target=2990eed03d3a · 라벨 없음 → ornith15-v030 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-01T01:47:22Z 후 10m45s) · 라벨 다름: `ornith15-v030` — 같은 노드 · 이 셀 라벨(`ornith15-35b-v030`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:05:03Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:15 |
| 9 | 2026-10-01T01:58:08Z | sub | `watchdog_kill_ack` | — | watchdog_kill_ack · target=2990eed03d3a · 라벨 없음 → ornith15-v030 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-01T01:47:22Z 후 10m46s) · 라벨 다름: `ornith15-v030` — 같은 노드 · 이 셀 라벨(`ornith15-35b-v030`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:05:03Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:16 |
| 10 | 2026-10-01T01:58:12Z | sub | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → ornith15-v030 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-01T01:47:22Z 후 10m50s) · 라벨 다름: `ornith15-v030` — 같은 노드 · 이 셀 라벨(`ornith15-35b-v030`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:05:03Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:8 |
| 11 | 2026-10-01T01:58:13Z | sub | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → ornith15-v030 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-01T01:43:17Z 후 14m56s) · 라벨 다름: `ornith15-v030` — 같은 노드 · 이 셀 라벨(`ornith15-35b-v030`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:05:03Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:376 |
| 12 | 2026-10-01T01:58:29Z | sub | `serve_stop` | — | serve_stop · model=ornith-1.5-35b-a3b · duration_s=913 · session=serve-ornith15-35b-v030-20261001T014316Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:05:03Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:10 |
| 13 | 2026-10-01T01:58:30Z | sub | `serve_stop` | — | serve_stop · model=ornith-1.5-35b-a3b · duration_s=668 · session=serve-ornith15-35b-v030-20261001T014722Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:05:03Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:12 |
| 14 | 2026-10-01T01:58:34Z | sub | `budget_declare` | `ornith15-v030` | 예산 선언(선언값 — 측정 아님) · floor_mib=23268 · weights_mib=68572 · kv_mib=16384 · overhead_mib=16384 · mem_total_mib=124608 · 라벨 다름: `ornith15-v030` — 같은 노드 · 이 셀 라벨(`ornith15-35b-v030`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · budget_renew 1회 · 닫힘 budget_clear 2026-10-01T02:05:23Z(선언 후 6m49s) · measured_utc 2026-10-10T05:29:12Z 는 이 창 밖(측정 전) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:05:03Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:13 |
| 15 | 2026-10-01T01:58:34Z | sub | `serve_start` | — | serve_start · model=ornith-1.5-35b-a3b · tensor_parallel_size=1 · topology=single · session=serve-ornith15-35b-v030-20261001T015834Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:05:03Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:14 |
| 16 | 2026-10-01T01:58:35Z | sub | `budget_honored` | `ornith15-v030` | budget_honored · floor_mib=23268 · arm_ceiling_mib=20196 · remaining_s=7199 · 창 시작 2026-10-01T01:58:35Z 후 0m00s · 라벨 다름: `ornith15-v030` — 같은 노드 · 이 셀 라벨(`ornith15-35b-v030`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:05:03Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:377 |
| 17 | 2026-10-01T01:58:36Z | sub | `budget_renew` | `ornith15-v030` | budget_renew · floor_mib=23268 · ttl_s=7200 · remaining_before_s=7198 · 창 시작 2026-10-01T01:58:34Z 후 0m02s · 라벨 다름: `ornith15-v030` — 같은 노드 · 이 셀 라벨(`ornith15-35b-v030`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:05:03Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:17 |
| 18 | 2026-10-01T01:59:09Z | sub | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=88707 · rate_mib_s=24652 · max_rate_mib_s=17801 · legacy_rule_would_trip=true · 라벨 없음 → ornith15-v030 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-01T01:58:35Z 후 0m34s) · 라벨 다름: `ornith15-v030` — 같은 노드 · 이 셀 라벨(`ornith15-35b-v030`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:05:03Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:378 |
| 19 | 2026-10-01T02:05:19Z | sub | `watchdog_trip` | — | watchdog_trip · mem_avail_mib=11483 · rate_mib_s=2149 · rule=eta · action=docker_kill · 라벨 없음 → ornith15-v030 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-01T01:58:35Z 후 6m44s) · 라벨 다름: `ornith15-v030` — 같은 노드 · 이 셀 라벨(`ornith15-35b-v030`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:05:03Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:379 |
| 20 | 2026-10-01T02:05:21Z | sub | `watchdog_kill_ack` | — | watchdog_kill_ack · target=8ec753abc622 · 라벨 없음 → ornith15-v030 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-01T01:58:34Z 후 6m47s) · 라벨 다름: `ornith15-v030` — 같은 노드 · 이 셀 라벨(`ornith15-35b-v030`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 사건의 다른 스트림 사본 docs/logs/sub/events/watchdog.jsonl:380(한 번으로 싣는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:05:03Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:23 |
| 21 | 2026-10-01T02:05:23Z | sub | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → ornith15-v030 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-01T01:58:34Z 후 6m49s) · 라벨 다름: `ornith15-v030` — 같은 노드 · 이 셀 라벨(`ornith15-35b-v030`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:05:03Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:18 |
| 22 | 2026-10-01T02:05:23Z | sub | `serve_stop` | — | serve_stop · model=ornith-1.5-35b-a3b · duration_s=409 · session=serve-ornith15-35b-v030-20261001T015834Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:05:03Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:19 |
| 23 | 2026-10-01T02:05:24Z | sub | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → ornith15-v030 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-01T01:58:35Z 후 6m49s) · 라벨 다름: `ornith15-v030` — 같은 노드 · 이 셀 라벨(`ornith15-35b-v030`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:05:03Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:381 |
| 24 | 2026-10-01T02:05:49Z | sub | `budget_declare` | `ornith15-v030-r3` | 예산 선언(선언값 — 측정 아님) · floor_mib=31460 · weights_mib=68572 · kv_mib=8192 · overhead_mib=16384 · mem_total_mib=124608 · 라벨 다름: `ornith15-v030-r3` — 같은 노드 · 이 셀 라벨(`ornith15-35b-v030`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · budget_renew 1회 · 닫힘 budget_clear 2026-10-01T02:12:23Z(선언 후 6m34s) · measured_utc 2026-10-10T05:29:12Z 는 이 창 밖(측정 전) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:05:03Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:20 |
| 25 | 2026-10-01T02:05:49Z | sub | `serve_start` | — | serve_start · model=ornith-1.5-35b-a3b · tensor_parallel_size=1 · topology=single · session=serve-ornith15-35b-v030-20261001T020549Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:05:03Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:21 |
| 26 | 2026-10-01T02:05:50Z | sub | `budget_honored` | `ornith15-v030-r3` | budget_honored · floor_mib=31460 · arm_ceiling_mib=28388 · remaining_s=7199 · 창 시작 2026-10-01T02:05:50Z 후 0m00s · 라벨 다름: `ornith15-v030-r3` — 같은 노드 · 이 셀 라벨(`ornith15-35b-v030`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:05:03Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:382 |
| 27 | 2026-10-01T02:05:51Z | sub | `budget_renew` | `ornith15-v030-r3` | budget_renew · floor_mib=31460 · ttl_s=7200 · remaining_before_s=7198 · 창 시작 2026-10-01T02:05:49Z 후 0m02s · 라벨 다름: `ornith15-v030-r3` — 같은 노드 · 이 셀 라벨(`ornith15-35b-v030`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:05:03Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:22 |
| 28 | 2026-10-01T02:06:22Z | sub | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=71757 · rate_mib_s=28313 · max_rate_mib_s=17801 · legacy_rule_would_trip=true · 라벨 없음 → ornith15-v030-r3 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-01T02:05:50Z 후 0m32s) · 라벨 다름: `ornith15-v030-r3` — 같은 노드 · 이 셀 라벨(`ornith15-35b-v030`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:05:03Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:383 |
| 29 | 2026-10-01T02:12:19Z | sub | `watchdog_trip` | — | watchdog_trip · mem_avail_mib=11085 · rate_mib_s=2342 · rule=eta · action=docker_kill · 라벨 없음 → ornith15-v030-r3 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-01T02:05:50Z 후 6m29s) · 라벨 다름: `ornith15-v030-r3` — 같은 노드 · 이 셀 라벨(`ornith15-35b-v030`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:05:03Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:384 |
| 30 | 2026-10-01T02:12:20Z | sub | `watchdog_kill_ack` | — | watchdog_kill_ack · target=bb4e525c4b72 · 라벨 없음 → ornith15-v030-r3 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-01T02:05:49Z 후 6m31s) · 라벨 다름: `ornith15-v030-r3` — 같은 노드 · 이 셀 라벨(`ornith15-35b-v030`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 사건의 다른 스트림 사본 docs/logs/sub/events/watchdog.jsonl:385(한 번으로 싣는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:05:03Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:29 |
| 31 | 2026-10-01T02:12:23Z | sub | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → ornith15-v030-r3 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-01T02:05:49Z 후 6m34s) · 라벨 다름: `ornith15-v030-r3` — 같은 노드 · 이 셀 라벨(`ornith15-35b-v030`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:05:03Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:24 |
| 32 | 2026-10-01T02:12:23Z | sub | `serve_stop` | — | serve_stop · model=ornith-1.5-35b-a3b · duration_s=394 · session=serve-ornith15-35b-v030-20261001T020549Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:05:03Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:25 |
| 33 | 2026-10-01T02:12:24Z | sub | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → ornith15-v030-r3 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-01T02:05:50Z 후 6m34s) · 라벨 다름: `ornith15-v030-r3` — 같은 노드 · 이 셀 라벨(`ornith15-35b-v030`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:05:03Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:386 |
| 34 | 2026-10-01T02:13:25Z | sub | `budget_declare` | `ornith15-v030-r4` | 예산 선언(선언값 — 측정 아님) · floor_mib=31460 · weights_mib=68572 · kv_mib=8192 · overhead_mib=16384 · mem_total_mib=124608 · 라벨 다름: `ornith15-v030-r4` — 같은 노드 · 이 셀 라벨(`ornith15-35b-v030`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · budget_renew 1회 · 닫힘 budget_clear 2026-10-01T02:22:21Z(선언 후 8m56s) · measured_utc 2026-10-10T05:29:12Z 는 이 창 밖(측정 전) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:05:03Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:26 |
| 35 | 2026-10-01T02:13:25Z | sub | `serve_start` | — | serve_start · model=ornith-1.5-35b-a3b · tensor_parallel_size=1 · topology=single · session=serve-ornith15-35b-v030-20261001T021325Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:05:03Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:27 |
| 36 | 2026-10-01T02:13:26Z | sub | `budget_renew` | `ornith15-v030-r4` | budget_renew · floor_mib=31460 · ttl_s=7200 · remaining_before_s=7199 · 창 시작 2026-10-01T02:13:25Z 후 0m01s · 라벨 다름: `ornith15-v030-r4` — 같은 노드 · 이 셀 라벨(`ornith15-35b-v030`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:05:03Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:28 |
| 37 | 2026-10-01T02:13:26Z | sub | `budget_honored` | `ornith15-v030-r4` | budget_honored · floor_mib=31460 · arm_ceiling_mib=28388 · remaining_s=7199 · 창 시작 2026-10-01T02:13:26Z 후 0m00s · 라벨 다름: `ornith15-v030-r4` — 같은 노드 · 이 셀 라벨(`ornith15-35b-v030`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:05:03Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:387 |
| 38 | 2026-10-01T02:13:51Z | sub | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=71569 · rate_mib_s=30899 · max_rate_mib_s=17801 · legacy_rule_would_trip=true · 라벨 없음 → ornith15-v030-r4 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-01T02:13:26Z 후 0m25s) · 라벨 다름: `ornith15-v030-r4` — 같은 노드 · 이 셀 라벨(`ornith15-35b-v030`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:05:03Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:388 |
| 39 | 2026-10-01T02:22:21Z | sub | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → ornith15-v030-r4 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-01T02:13:25Z 후 8m56s) · 라벨 다름: `ornith15-v030-r4` — 같은 노드 · 이 셀 라벨(`ornith15-35b-v030`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:05:03Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:30 |
| 40 | 2026-10-01T02:22:21Z | sub | `serve_stop` | — | serve_stop · model=ornith-1.5-35b-a3b · duration_s=536 · session=serve-ornith15-35b-v030-20261001T021325Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:05:03Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:31 |
| 41 | 2026-10-01T02:22:22Z | sub | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → ornith15-v030-r4 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-01T02:13:26Z 후 8m56s) · 라벨 다름: `ornith15-v030-r4` — 같은 노드 · 이 셀 라벨(`ornith15-35b-v030`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:05:03Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:389 |
| 42 | 2026-10-10T05:05:03Z | sub | `budget_declare` | `serve-ornith15-35b-v030` | 예산 선언(선언값 — 측정 아님) · floor_mib=31460 · weights_mib=68572 · kv_mib=8192 · overhead_mib=16384 · mem_total_mib=124608 · 기동 창 1/1: budget_renew 1회 · 닫힘 budget_clear 2026-10-10T05:31:39Z(선언 후 26m36s) · measured_utc 2026-10-10T05:29:12Z 가 이 창 안(이 측정의 기동) | docs/logs/sub/events/2026-10.jsonl:234 |
| 43 | 2026-10-10T05:05:03Z | sub | `serve_start` | — | serve_start · model=ornith-1.5-35b-a3b · tensor_parallel_size=1 · topology=single · session=serve-ornith15-35b-v030-20261010T050503Z | docs/logs/sub/events/2026-10.jsonl:235 |
| 44 | 2026-10-10T05:05:04Z | sub | `budget_honored` | `serve-ornith15-35b-v030` | budget_honored · floor_mib=31460 · arm_ceiling_mib=28388 · remaining_s=7199 · 창 시작 2026-10-10T05:05:04Z 후 0m00s | docs/logs/sub/events/watchdog.jsonl:529 |
| 45 | 2026-10-10T05:05:05Z | sub | `budget_renew` | `serve-ornith15-35b-v030` | budget_renew · floor_mib=31460 · ttl_s=7200 · remaining_before_s=7198 · 창 시작 2026-10-10T05:05:03Z 후 0m02s | docs/logs/sub/events/2026-10.jsonl:236 |
| 46 | 2026-10-10T05:05:19Z | sub | `engine_log_window_start` | — | 캡처된 엔진 로그의 첫 시각(꼬리 캡처일 수 있다 — 엔진 시작 시각이 아닐 수 있다) · 선언 2026-10-10T05:05:03Z 후 0m16s · 시계: 컨테이너 시각을 UTC 로 읽어 측정 창 [2026-10-10T05:05:03Z, 2026-10-10T05:31:39Z] 안에 드는 것을 확인 | output/single/benchlog/sweep_ornith15-35b-v030/lite_engine_ornith15-35b-v030.log:36 |
| 47 | 2026-10-10T05:05:34Z | sub | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=77192 · rate_mib_s=31506 · max_rate_mib_s=17801 · legacy_rule_would_trip=true · 라벨 없음 → serve-ornith15-35b-v030 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-10T05:05:04Z 후 0m30s) | docs/logs/sub/events/watchdog.jsonl:530 |
| 48 | 2026-10-10T05:05:35Z | sub | `engine_prefetch_disabled` | — | 가중치 auto-prefetch 꺼짐 줄 ×1 — filesystem CIFS(vLLM 로그 문구) | output/single/benchlog/sweep_ornith15-35b-v030/lite_engine_ornith15-35b-v030.log:66 |
| 49 | 2026-10-10T05:10:44Z | sub | `engine_weights_loaded` | — | 가중치 적재 완료 줄 ×1(rank·단계별) · 최장 308.61s · 이 행 = 마지막 줄 | output/single/benchlog/sweep_ornith15-35b-v030/lite_engine_ornith15-35b-v030.log:86 |
| 50 | 2026-10-10T05:10:45Z | sub | `engine_model_loaded` | — | 모델 적재 완료 줄 ×1(rank 별) · 마지막 줄 64.69 GiB · 312.9s | output/single/benchlog/sweep_ornith15-35b-v030/lite_engine_ornith15-35b-v030.log:89 |
| 51 | 2026-10-10T05:10:53Z | sub | `engine_kv_cache_sized` | — | GPU KV cache 408,122 tokens · 요청당 262,144 tokens 기준 최대 동시성 1.56x | output/single/benchlog/sweep_ornith15-35b-v030/lite_engine_ornith15-35b-v030.log:100 |
| 52 | 2026-10-10T05:11:42Z | sub | `engine_init_done` | — | 엔진 초기화(profile·KV 할당·warmup) 57.41s | output/single/benchlog/sweep_ornith15-35b-v030/lite_engine_ornith15-35b-v030.log:118 |
| 53 | 2026-10-10T05:11:47Z | sub | `server_starting` | — | API 서버 기동 — 이 측정 창 선언 2026-10-10T05:05:03Z 후 6m44s · 캡처 로그 첫 시각 2026-10-10T05:05:19Z 후 6m28s | output/single/benchlog/sweep_ornith15-35b-v030/lite_engine_ornith15-35b-v030.log:127 |
| 54 | 2026-10-10T05:31:39Z | sub | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → serve-ornith15-35b-v030 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-10T05:05:03Z 후 26m36s) | docs/logs/sub/events/2026-10.jsonl:237 |
| 55 | 2026-10-10T05:31:39Z | sub | `serve_stop` | — | serve_stop · model=ornith-1.5-35b-a3b · duration_s=1596 · session=serve-ornith15-35b-v030-20261010T050503Z | docs/logs/sub/events/2026-10.jsonl:238 |
| 56 | 2026-10-10T05:31:40Z | sub | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → serve-ornith15-35b-v030 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-10T05:05:04Z 후 26m36s) | docs/logs/sub/events/watchdog.jsonl:531 |

**기동 시도**(예산 선언 `budget_declare` 마다 한 시도 · 같은 노드의 이어지는 행과 선언 없는 노드(엔진 로그)의 그 시각 행을 묶었다 · 사살 · 트립 · 거부는 닫힘과 따로 센다 · `measurement` 행은 측정 기록이라 시도로 세지 않는다(위 표에만) — 해석은 저작자 몫)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 1 | sub | `serve-ornith15-35b-v030` | 2026-10-10T05:05:03Z | 1회 · 첫 2026-10-10T05:05:05Z | 없음 | `budget_clear` 2026-10-10T05:31:39Z | 12 |

**같은 셀 이름 · 앞선 캠페인의 기동 5회**(캠페인 경계 2026-10-10T05:05:03Z 이전 — 라벨이 같아 원장에서 함께 묶였을 뿐 이 셀의 시도로 세지 않는다 · 행 전문 = 01 §1.4)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 앞선-1 | sub | `ornith15-v030`(라벨 다름 — 같은 노드 · 이 셀 라벨 아님) | 2026-10-01T01:43:16Z | 없음 | 없음 | —(닫힘 행 없음) | 2 |
| 앞선-2 | sub | `ornith15-v030`(라벨 다름 — 같은 노드 · 이 셀 라벨 아님) | 2026-10-01T01:47:22Z | 1회 · 첫 2026-10-01T01:47:23Z | `watchdog_kill_ack` 1 · `watchdog_trip` 1 | `budget_clear` 2026-10-01T01:58:12Z | 5 |
| 앞선-3 | sub | `ornith15-v030`(라벨 다름 — 같은 노드 · 이 셀 라벨 아님) | 2026-10-01T01:58:34Z | 1회 · 첫 2026-10-01T01:58:36Z | `watchdog_kill_ack` 1 · `watchdog_trip` 1 | `budget_clear` 2026-10-01T02:05:23Z | 5 |
| 앞선-4 | sub | `ornith15-v030-r3`(라벨 다름 — 같은 노드 · 이 셀 라벨 아님) | 2026-10-01T02:05:49Z | 1회 · 첫 2026-10-01T02:05:51Z | `watchdog_kill_ack` 1 · `watchdog_trip` 1 | `budget_clear` 2026-10-01T02:12:23Z | 5 |
| 앞선-5 | sub | `ornith15-v030-r4`(라벨 다름 — 같은 노드 · 이 셀 라벨 아님) | 2026-10-01T02:13:25Z | 1회 · 첫 2026-10-01T02:13:26Z | 없음 | `budget_clear` 2026-10-01T02:22:21Z | 5 |
<!-- /FACT:event_timeline -->

<!-- FACT:measurement_env -->
_측정 실행의 env 관측 없음 — 측정 엔진 로그에서 env echo 줄(`<KEY> set by environment to <VALUE>` 등)을 찾지 못했다(evidence.measurement_env_observed · 부재 ≠ 미설정)._
<!-- /FACT:measurement_env -->

> 이 절이 답하는 질문: 수신자가 이 zip 만으로 이 셀을 다시 띄우려면 어떤 순서로 무엇을 하고, 각 단계의 성공을 무엇으로 판정하며, 얼마나 기다려야 하는가?

**한 눈에.** 총 소요(관측): serve ≤ 6분 57초(예산 선언 → 첫 측정 시각, 상한) · READY 410 s(10-10 testlog) · bench 17분 12초. 기동 시도: **이번 캠페인 창 1회**(위 표와 같다) — 10-01 의 앞선 시도 5세션은 별도(02 §2.2). 빌드는 이 지도에서 **재현되지 않았다**(이미지 보유 검증, Q5).

**(a) 사전 준비.** 가중치는 관리형 NAS 의 읽기전용 마운트에서 `/app/models/ornith-ai/Ornith-1.5-35B-A3B` 로 본다(compose `NAS_MODEL_PATH`, 00 §0.2 의 HF revision). 엔진 로그는 가중치 auto-prefetch 가 꺼졌다고 적었다(파일시스템 CIFS — 01 §1.4 이벤트 48행); 적재 시간은 실행마다 다르다: 10-01 성공 시도 `Loading weights took 282.89 seconds` · `Model loading took 64.69 GiB … 287.273767 seconds`(testlog_26100111_ornith15-35b-v030_싱글서브_서빙_lite §4 의 287 s), 10-10 실행 `Loading weights took 308.61 seconds` · `Model loading took 64.69 GiB … 312.909736 seconds`(이벤트 49 · 50행). 같은 두 로그 줄이 실행 사이에 약 25 초 달랐다 — 같은 구간의 정의 차이가 아니라 서로 다른 실행의 값이다(한 실행 안에서 weights 줄과 model 줄이 다른 것만 정의 차이). 필수 env 는 `.env.template` 의 `NAS_MODEL_PATH` · `QUANT_MODEL_PATH` · `TIKTOKEN_HOST_PATH` 와 셀 env 의 `CONFIG_FILE` · `SERVING_PORT` · `SERVING_MODEL_NAME` · `IMAGE_TAG` 다.

**(b) 빌드.** 이 호스트에 이미 있던 이미지를 검증해 썼다(재빌드 ✗, testlog_26100111_ornith15-35b-v030_싱글서브_서빙_lite §1). 이미지 층 창은 약 40분 42초이나 캐시 재사용 층이 섞여 빌드 소요가 아니다(FACT). 소스 빌드 build-arg 는 위 FACT 명령 그대로다.

**(c) 기동.** `single_serve_up.sh ornith15-35b-v030 --overhead-mib 16384 --ready-max 1200`(10-10 testlog §2)으로 READY 410 s. 성공 판정: `GET /health` 200 + chat completion 1회(`17*23` 질의, `finish_reason=stop`, reasoning 분리 — testlog_26100111_ornith15-35b-v030_싱글서브_서빙_lite §4). 로그 서명: `Model loading took 64.69 GiB`, `GPU KV cache size: 408,122 tokens`, `Maximum concurrency for 262,144 tokens: 1.56x`, CUDA graph PIECEWISE+FULL 캡처 약 1 s. 패치가 발화했음을 보여주는 서명은 이 셀에서 관측되지 않았다(1.2). 정상인데 무서워 보이는 줄: 이벤트 47행 `watchdog_highrate_hold`(고속 하락 감시가 legacy 규칙이면 트립이었다고 적었으나 hold 됨 — 이 기동은 사살 없이 끝났다)와 48행 prefetch 꺼짐 — 둘 다 이 기동에서 사살로 이어지지 않았다. 같은 `watchdog_highrate_hold` 가 사살로 끝난 10-01 시도 1~3 에도 기동 약 40 초 뒤에 있었고(이벤트 7 · 18 · 28행) 성공한 시도 4 에도 있었다(38행) — 이 줄만으로는 사살 여부를 가를 수 없다.

**(d) 호스트 메모리 안전 예산.** 선언 행(이벤트 42행)의 입력: mem_total_mib=124608 · weights_mib=68572 · kv_mib=8192 · overhead_mib=16384 → floor_mib=31460(대입하면 124608 − 68572 − 8192 − 16384 = 31460 으로 일치, 계산 확인). arm 천장 28388 MiB(testlog_26101014_knob프리셋_E2E_셀C_ornith §2). 이 값들은 선언이며 엔진 실측(가중치 64.69 GiB · KV 8.0 GiB · 총 예약 72.69 GiB)과 다르다(02 §2.5).

**(e) 측정.** lite 구간은 위 표의 `vllm bench serve` 재구성 명령을 가리키고, full 구간은 guidellm 이며 03 §3.4 에 도구 원문 발췌가 있다. 재구성 명령의 입력 길이는 총 토큰 ÷ 완료 수의 평균이라 실행 원문이 아니다.

**(f) 엔진이 스스로 고른 경로.** 엔진 로그에 attention backend FLASH_ATTN · GDN prefill FlashInfer 커널 · Mamba cache mode `align`(testlog_26100111_ornith15-35b-v030_싱글서브_서빙_lite §4). MoE 백엔드는 yaml 이 triton 으로 고정했다. 측정 실행의 env echo 는 관측되지 않았다(FACT: 부재 ≠ 미설정).


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

### 가중치 획득
- 체크포인트: HF `ornith-ai/Ornith-1.5-35B-A3B`, revision 은 00 §0.2 사실 블록의 값. 이 노드는 관리형 NAS(manifest `model_source: managed`)의 읽기전용 마운트에서 읽었고, **획득 명령은 계보에 기록돼 있지 않다**(미기록 · 출처 testlog_26100111_ornith15-35b-v030_싱글서브_서빙_lite §2, 본문 내 획득 언급 없음). 받는다면 명령 형태는 `huggingface-cli download ornith-ai/Ornith-1.5-35B-A3B --revision <rev> --local-dir <manifest.nas_model_path>/ornith-ai/Ornith-1.5-35B-A3B` 이다 — **추론 · 실행 검증 ✗**. 모델 획득은 사람의 승인 게이트이며 무인 다운로드를 지시하지 않는다. 크기: 엔진 로그 적재 64.69 GiB(bf16, 노드 1개분).

### 스테이징(PLE 등)
- 해당 없음 — 모델 config 에 PLE 키가 없다(00 §0.9 후보 표의 관측). 가중치를 로컬 디스크로 옮기는 스테이징도 이 셀은 하지 않았다(NAS CIFS 위에서 직접 읽었고 prefetch 가 꺼졌다 — 01 §1.4 이벤트 48행). 컴파일 캐시는 compose 의 `./cache/vllm` · `./cache/flashinfer` 마운트로 영속되며, 시도 2 가 '컴파일 캐시 적중'으로 기록돼 있다(testlog_26100111_ornith15-35b-v030_싱글서브_서빙_lite §3).

### 네트워크가 필요한 단계
- 이미지 재빌드가 필요하면 NGC 베이스 pull · vLLM 저장소 clone(`https://github.com/vllm-project/vllm.git`, ref v0.30.0 — build-arg) · pip 의존 설치가 외부 접속을 요구한다(Dockerfile 의 일반 구조에서의 **추론**, 실린 Dockerfile 이 이미지와 불일치하므로 정확한 단계 목록은 확인되지 않았다 — Q5). 서빙 자체는 NAS 마운트만 쓰며 외부 접속이 없다(측정 컨테이너는 `HF_HUB_OFFLINE=1`, 03 §3.4 도구 원문). 오프라인 대체: 베이스 이미지와 소스 트리를 미리 받아 둔다.

