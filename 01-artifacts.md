# 01 · 산출물 — 무엇이 실제로 쓰였고, 어떻게 다시 띄우는가

## 1.1 적용 판정 표
> 기계가 적용 증거(빌드 원장 · 원장이 없으면 라벨된 재구성) × 파일 × 선언을 대사한 결과다. `artifacts/` 에는 **적용된 것만** 실렸다 — 빌드 때 스스로 skip 된 패치와 쓰이지 않은 평면의 파일은 표에만 남고 싣지 않는다.

<!-- FACT:slots -->
| 슬롯 | 실림 | 파일 | 신호 | 적용 증거 | 사유 |
|---|---|---|---|---|---|
| `build_patch_post` | 안 실림 | — | 1-signal | build-ledger:attestation:output/multi/benchlog/attestation_q38fn030-nv4-res-262k.json#main | 적용 판정 없음 — 원장 관측 · 노드 비대칭(cluster) — main(대표) 원장: 이 단계 행 없음 / sub 원장: applied 10-deepgemm.sh, 20-triton-kernels.sh, 30-mxfp4-triton-sm121.sh, 40-humming-nvml-gb10.sh — 실림·판정은 대표 원장 기준(기계 파생) |
| `build_patch_pre` | 안 실림 | — | 1-signal | build-ledger:attestation:output/multi/benchlog/attestation_q38fn030-nv4-res-262k.json#main | 적용 판정 없음 — 원장 관측 · 노드 비대칭(cluster) — main(대표) 원장: 이 단계 행 없음 / sub 원장: skipped 50-dsv4-sm12x-port.sh, 55-src-deps-authority.sh — 실림·판정은 대표 원장 기준(기계 파생) |
| `build_recipe` | 실림 | `artifacts/build_recipe/Dockerfile.source-build` · `artifacts/build_recipe/requirements.txt` | 3-signal | build-ledger:build-ledger.dockerfile | (a) NGC `nvcr.io/nvidia/pytorch:26.07-py3` 위에서 `VLLM_REPO`@`VLLM_REF`(v0.30.0)를 소스빌드하는 stock 트랙 템플릿이다 — `TORCH_CUDA_ARCH=12.1a` · ccache 와 `libdw-dev` 설치 줄 · `RAY_VERSION=2.48.0` 의 ray 설치 · nc/nvcc/ray 를 명시 실패로 검증하는 RUN · 마지막 RUN 의 빌드 원장(`/opt/easy-vllm/build_ledger.json`) 주입. 레시피 일반 동작으로 패치 50 · 55 는 `SM12X_PORT=0` · `SRC_DEPS_AUTHORITY=0` build-arg 게이트로 스스로 skip 된다(`plan_26100107` §3 — 판독). 이 측… |
| `compose` | 실림 | `artifacts/compose/.env.cluster.template` · `artifacts/compose/.env.interconnect.template` · `artifacts/compose/.env.template` · `artifacts/compose/docker-compose.yaml` · `artifacts/compose/serve_runner.sh` · `artifacts/compose/sub_recipe.json` | 2-signal | file:output/multi/docker-compose.yaml | (a) compose 는 master(Ray head + vllm serve) · slave(Ray worker) 서비스를 같은 이미지로 띄우고 env_file 로 셀 env · `.env.cluster` · `.env.interconnect` 를 받는다. `serve_runner.sh` 는 `NODE_ROLE` 로 분기해 master 는 Ray head 기동 → GPU 합류 대기 → 셀 러너 source, slave 는 head 대기 → Ray worker `--block` 이다. env 형상 3종은 manifest 값이 들어갈 자리표시 사본이다. `sub_recipe.json` 은 서브에 전달돼야 하는 env 와 이유를 기계가 파생한 것이다(1.5). (b) 분산 서빙 경로 자체라 빼면 이 셀이 뜨지… |
| `fork_pin` | 안 실림 | — | 1-signal | none:.claude/policies/arch_variant_ledger.json | 셀 env 에 VARIANT 줄 없음 = stock(workflow.md §변종 좌표의 거처)(기계 파생) |
| `runtime_patch` | 안 실림 | — | 1-signal | none:output/multi/configs/q38fn030-nv4-res-262k_patch.py | output/multi/configs/q38fn030-nv4-res-262k_patch.py 없음 — arm_patch.sh 는 no-op(런타임 패치 불해당)(기계 파생) |
| `triplet` | 실림 | `artifacts/triplet/.env.q38fn030-nv4-res-262k` · `artifacts/triplet/q38fn030-nv4-res-262k.sh` · `artifacts/triplet/q38fn030-nv4-res-262k.yaml` | 3-signal | file:output/multi/configs/q38fn030-nv4-res-262k.yaml | (a) 서빙 노브 12개(1.3) · 러너(`vllm serve --config` + `--served-model-name`) · 셀 env(이미지 태그 · Dockerfile 선택자 · `VLLM_REF` · 컨테이너 이름). yaml 머리 주석이 이 셀의 선언 축과 "참조(승계 아님)" 관계, 0.30.0 의 `VLLM_PLE_CPU_OFFLOAD=1` 기본값 경고를 적는다. (b) W5(eager) · W7(gmu · KV · 배치) · W10(engram-config)의 해소가 노브로 여기 있다. (c) 측정이 이 config 로 돌았다(엔진 로그 non-default args 가 yaml 값과 같다 — 1.4 (f)). (d) 노브마다의 폐기 조건은 1.3 · 0.5 를 본다. |

**레시피 경고 · 실린 리비전**(기계 관측 — 수신자 빌드가 여기서 갈라질 수 있다)

- `build_recipe` `output/multi/Dockerfile.source-build` 실린 리비전: 리비전 미특정 · 선택 방법 `worktree(측정 이미지 docker history 의 RUN·COPY 와 전수 일치)` · docker history 대조 17/17 · 실린 바이트 `worktree` · 쓰인 바이트임을 관측으로 확인 예
- `build_recipe` `output/multi/requirements.txt` 실린 리비전: 리비전 미특정 · 선택 방법 `worktree(빌드 원장 sha256 일치 — 쓰인 바이트 관측 확인 = 예)` · docker history 대조 없음 · 실린 바이트 `worktree` · 쓰인 바이트임을 관측으로 확인 예
- `compose` `.claude/skills/upstream-version-watch/assets/configs/serve_runner.sh` 실린 리비전: 커밋 `50a5ce486738` · 선택 방법 `worktree(mtime 2026-09-06T21:57:53Z ≤ 측정 2026-10-10T04:07:13Z(sweep_index.generated_utc) · 바이트 = 측정 전 마지막 커밋 50a5ce486738)` · docker history 대조 없음 · 실린 바이트 `worktree` · 근거 `mtime≤measured` · 쓰인 바이트임을 관측으로 확인 예
- `compose` `output/multi/docker-compose.yaml` 실린 리비전: 커밋 `5b992ad9cc60` · 선택 방법 `worktree(git log -1 --before=2026-10-10T04:07:13Z -- output/multi/docker-compose.yaml → 5b992ad9cc60 · 작업트리 mtime 2026-10-10T10:01:49Z > 측정(sweep_index.generated_utc)이지만 바이트가 그 개정과 같다 · 측정 뒤 커밋 0 · 작업트리 = HEAD)` · docker history 대조 없음 · 실린 바이트 `worktree` · 근거 `git-rev-before-measurement` · 쓰인 바이트임을 관측으로 확인 아니오

**싣지 않은 파일**(슬롯별 사유 — 빌드 때 skip 된 패치는 아래 적용 집합 표에 있다)

- `build_patch_post` `10-deepgemm.sh` — `not-in-ledger(대표 main 원장에 없음 · sub 원장 applied — 노드 비대칭 · 대표 원장 밖이라 싣지 않음)`
- `build_patch_post` `20-triton-kernels.sh` — `not-in-ledger(대표 main 원장에 없음 · sub 원장 applied — 노드 비대칭 · 대표 원장 밖이라 싣지 않음)`
- `build_patch_post` `30-mxfp4-triton-sm121.sh` — `not-in-ledger(대표 main 원장에 없음 · sub 원장 applied — 노드 비대칭 · 대표 원장 밖이라 싣지 않음)`
- `build_patch_post` `40-humming-nvml-gb10.sh` — `not-in-ledger(대표 main 원장에 없음 · sub 원장 applied — 노드 비대칭 · 대표 원장 밖이라 싣지 않음)`
- `build_patch_post` `70-exllamav3-build.sh` — `not-in-ledger(원장에 없음 — 빌드 뒤 추가된 파일로 본다 · 싣지 않음)`
- `build_patch_post` `72-exl3-row-store.sh` — `not-in-ledger(원장에 없음 — 빌드 뒤 추가된 파일로 본다 · 싣지 않음)`
- `build_patch_post` `73-vllm-exl3-build.sh` — `not-in-ledger(원장에 없음 — 빌드 뒤 추가된 파일로 본다 · 싣지 않음)`
- `build_patch_post` `74-exl3-moe-prep-fused.sh` — `not-in-ledger(원장에 없음 — 빌드 뒤 추가된 파일로 본다 · 싣지 않음)`
- `build_patch_post` `79-exl3-port-selfcheck.sh` — `not-in-ledger(원장에 없음 — 빌드 뒤 추가된 파일로 본다 · 싣지 않음)`
- `build_patch_post` known-non-patch 1건: `.gitkeep`
- `build_patch_pre` `55-src-deps-authority.sh` — `not-in-ledger(대표 main 원장에 없음 · sub 원장 skipped — 노드 비대칭 · 대표 원장 밖이라 싣지 않음)`
- `build_patch_pre` `80-exl3-quant-method.sh` — `not-in-ledger(원장에 없음 — 빌드 뒤 추가된 파일로 본다 · 싣지 않음)`
- `build_patch_pre` `81-exl3-model-wiring.sh` — `not-in-ledger(원장에 없음 — 빌드 뒤 추가된 파일로 본다 · 싣지 않음)`
- `build_patch_pre` `82-exl3-h2d-stage.sh` — `not-in-ledger(원장에 없음 — 빌드 뒤 추가된 파일로 본다 · 싣지 않음)`
- `build_patch_pre` `83-engram-file-backend.sh` — `not-in-ledger(원장에 없음 — 빌드 뒤 추가된 파일로 본다 · 싣지 않음)`
- `build_patch_pre` `84-sm12x-block-shim.sh` — `not-in-ledger(원장에 없음 — 빌드 뒤 추가된 파일로 본다 · 싣지 않음)`
- `build_patch_pre` `85-exl3-serial-streams.sh` — `not-in-ledger(원장에 없음 — 빌드 뒤 추가된 파일로 본다 · 싣지 않음)`
- `build_patch_pre` `86-exl3-worker-hooks.sh` — `not-in-ledger(원장에 없음 — 빌드 뒤 추가된 파일로 본다 · 싣지 않음)`
- `build_patch_pre` `87-engram-embed-skip.sh` — `not-in-ledger(원장에 없음 — 빌드 뒤 추가된 파일로 본다 · 싣지 않음)`
- `build_patch_pre` `88-engram-prestage.sh` — `not-in-ledger(원장에 없음 — 빌드 뒤 추가된 파일로 본다 · 싣지 않음)`
- `build_patch_pre` `89-kv-null-block-zero.sh` — `not-in-ledger(원장에 없음 — 빌드 뒤 추가된 파일로 본다 · 싣지 않음)`
- `build_patch_pre` `90-woa-fp8-einsum.sh` — `not-in-ledger(원장에 없음 — 빌드 뒤 추가된 파일로 본다 · 싣지 않음)`
- `build_patch_pre` `91-mxfp8-prefer-b12x.sh` — `not-in-ledger(원장에 없음 — 빌드 뒤 추가된 파일로 본다 · 싣지 않음)`
- `build_patch_pre` `92-dense-gemv.sh` — `not-in-ledger(원장에 없음 — 빌드 뒤 추가된 파일로 본다 · 싣지 않음)`
- `build_patch_pre` `exl3_port_lib.py` — `nonconforming(<NN>-*.sh 규약 밖 — 싣지 않음)`
- `build_patch_pre` known-non-patch 3건: `.gitkeep` · `PROVENANCE.json` · `__pycache__/`
- `compose` `arm_patch.sh` — `not-armed(output/multi/configs/q38fn030-nv4-res-262k_patch.py 없음 · slave CONFIG_FILE='default' 의 _patch.py 없음 — arm_patch.sh 는 no-op)`

**파일별 검증 · 관련성**(`generated-unverified` = 실행 검증되지 않은 생성물 — 파일 첫 줄 경고 · PAYLOAD 파일 기록과 같은 표시 · 관련성 = ① 패치 선언 대상 × 모델 config ② 엔진 로그 발화 서명 — 둘 다 관측일 때만 required/inactive-inferred)

| 슬롯 | 파일 | 검증 | 검증 근거 | 관련성 | 관련성 근거 |
|---|---|---|---|---|---|
| `build_recipe` | `artifacts/build_recipe/Dockerfile.source-build` | `verified` · cell-run | 쓰인 레시피(build-ledger.dockerfile) — 개정 선택 slots.build_recipe.evidence.selected_revisions | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `build_recipe` | `artifacts/build_recipe/requirements.txt` | `verified` · cell-run | 쓰인 레시피(build-ledger.dockerfile) — 개정 선택 slots.build_recipe.evidence.selected_revisions | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/.env.cluster.template` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/.env.interconnect.template` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/.env.template` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/docker-compose.yaml` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/serve_runner.sh` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/sub_recipe.json` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/.env.q38fn030-nv4-res-262k` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/q38fn030-nv4-res-262k.sh` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/q38fn030-nv4-res-262k.yaml` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
<!-- /FACT:slots -->

<!-- FACT:applied_set -->
**적용 집합** — 상태 `observed` · 출처 `attestation:output/multi/benchlog/attestation_q38fn030-nv4-res-262k.json#main`

| 단계 | 파일 | 결과 | 결과 출처 | 사유 | 실림 | model-trigger |
|---|---|---|---|---|---|---|
| inline | `strip-hoist` | skipped | build-ledger | register_opaque_type accepts hoist / opaque_object 부재 | 안 실림 | — |

**결과 출처 범례**

- `build-ledger` — 이미지 안 빌드 원장이 적용 결과를 기록했다(관측)

**판정 근거**(패치별 — artifacts 가 판정에 쓴 관측 · 스크립트 바이트 정체 = 실린 바이트가 측정 이미지에 구워진 바이트인가)

| 파일 | 스크립트 바이트 정체 | 이미지 사본 sha256 | 적용 표지(상태 찾음/선언) | 정적 규칙 | 원장 분류 |
|---|---|---|---|---|---|
| `strip-hoist` | — | — | — | — | status-file |

**노드별 원장**(cluster · 대표 `main` — 노드별 빌드 원장의 적용 집합이 다르다 — 이 측정은 cluster(노드 전부)가 서빙했다. 대표 원장 하나로 cluster 의 적용 집합을 말하지 않는다(합집합 ✗ · 한 노드 선택 ✗). 실린 패치 파일 · 슬롯 판정의 기준은 대표 원장이다)

| 노드 | 단계 | 파일 | 결과 | 출처 |
|---|---|---|---|---|
| `main` | inline | `strip-hoist` | skipped | `attestation:output/multi/benchlog/attestation_q38fn030-nv4-res-262k.json#main` |
| `sub` | pre | `50-dsv4-sm12x-port.sh` | skipped | `attestation:output/multi/benchlog/attestation_q38fn030-nv4-res-262k.json#sub` |
| `sub` | pre | `55-src-deps-authority.sh` | skipped | `attestation:output/multi/benchlog/attestation_q38fn030-nv4-res-262k.json#sub` |
| `sub` | post | `10-deepgemm.sh` | applied | `attestation:output/multi/benchlog/attestation_q38fn030-nv4-res-262k.json#sub` |
| `sub` | post | `20-triton-kernels.sh` | applied | `attestation:output/multi/benchlog/attestation_q38fn030-nv4-res-262k.json#sub` |
| `sub` | post | `30-mxfp4-triton-sm121.sh` | applied | `attestation:output/multi/benchlog/attestation_q38fn030-nv4-res-262k.json#sub` |
| `sub` | post | `40-humming-nvml-gb10.sh` | applied | `attestation:output/multi/benchlog/attestation_q38fn030-nv4-res-262k.json#sub` |
| `sub` | inline | `strip-hoist` | skipped | `attestation:output/multi/benchlog/attestation_q38fn030-nv4-res-262k.json#sub` |

**원장 탐침**(순위: attestation → 메인 로컬 이미지 → 재구성)

| 순위 | 결과 | 대상 |
|---|---|---|
| attestation | observed | output/multi/benchlog/attestation_q38fn030-nv4-res-262k.json |

> **attestation 범위** — config `q38fn030-nv4-res-262k` · phase serve · 작성 2026-10-10T03:50:09Z(output/multi/benchlog/attestation_q38fn030-nv4-res-262k.json 파일 mtime(파일시스템 관측 — 문서에 시각 칸이 없다 · 기록 필드 아님)) · 묶은 근거 cell-name · 파일 `output/multi/benchlog/attestation_q38fn030-nv4-res-262k.json` · 측정 창 대비 작성 2026-10-10T03:50:09Z < 측정 시작 2026-10-10T03:50:30Z — 측정 전(빌드·스모크)의 관측: **same config(q38fn030-nv4-res-262k) · serve — 측정을 서빙한 같은 기동의 스모크 직후 관측(판별: 스모크 로그 docs/simlog/26101012_벤치_knob_프리셋_E2E/m1_serve.log L33 SMOKE PASS → L34 attestation 작성 → L37 --keep-up · 원장 docs/logs/main/events/2026-10.jsonl:288 budget_declare(smoke-q38fn030-nv4-res-262k · 2026-10-10T03:19:17Z) 가 측정 끝 2026-10-10T04:07:13Z 까지 재선언 · 창 닫힘 없이 이어짐(main 원장 기록 범위 안) · 스모크 로그 budget_honored ts 2026-10-10T03:19:18Z = 그 선언의 원장 budget_honored · 측정 엔진 로그 APIServer pid 789 하나 · 측정 시작 2026-10-10T03:50:30Z)**
<!-- /FACT:applied_set -->

## 1.2 슬롯별 적용 사유
> 이 절이 답하는 질문: 실린 산출물 하나하나는 무엇을 고치며, 왜 이 셀에 필요했고, 이 셀에서 실제로 발화했으며, 언제 불필요해지는가?

### build_recipe · `Dockerfile.source-build`

(a) NGC `nvcr.io/nvidia/pytorch:26.07-py3` 위에서 `VLLM_REPO`@`VLLM_REF`(v0.30.0)를 소스빌드하는 stock 트랙 템플릿이다 — `TORCH_CUDA_ARCH=12.1a` · ccache 와 `libdw-dev` 설치 줄 · `RAY_VERSION=2.48.0` 의 ray 설치 · nc/nvcc/ray 를 명시 실패로 검증하는 RUN · 마지막 RUN 의 빌드 원장(`/opt/easy-vllm/build_ledger.json`) 주입. 레시피 일반 동작으로 패치 50 · 55 는 `SM12X_PORT=0` · `SRC_DEPS_AUTHORITY=0` build-arg 게이트로 스스로 skip 된다(`plan_26100107` §3 — 판독). 이 측정의 관측은 노드마다 다르다 — 메인 이미지 원장에는 50 · 55 가 아예 없고(패치 슬롯이 .gitkeep 뿐) 게이트 skip 기록은 서브 원장에만 있다(1.1 노드별 원장). (b) `libdw-dev` 줄을 빼면 0.30.0 deepgemm 빌드가 elfutils 헤더 부재로 깨진다(W9 — 이 줄의 헤더 주석이 그 실패를 기록한다). ray 는 2노드 Ray TP 의 런타임 의존이다. (c) 이 이미지가 측정 이미지를 지었다 — docker history 의 RUN · COPY 17/17 이 일치한다(1.1 레시피 경고). (d) elfutils 헤더가 베이스 이미지에 들어오거나 deepgemm 이 그 의존을 버리면 `libdw-dev` 줄이 불필요해진다. 빌드 원장의 적용 집합은 인라인 `strip-hoist` 1건 skip(`register_opaque_type accepts hoist`)뿐이다(1.1 적용 집합).

> [원문] Dockerfile.source-build §L58-60
> # libdw-dev: vLLM 0.30.0 의 deepgemm(deep_jit, #56876)이 elfutils/libdwfl.h 를 요구 — 상류 CUDA Dockerfile 의
> #   elfutils-devel 에 대응하는 Ubuntu 패키지(2026-10-01 양노드 빌드 실패 실측 · plan_26100107).
> RUN apt-get update && apt-get install -y --no-install-recommends ccache libdw-dev && rm -rf /var/lib/apt/lists/*

### build_recipe · `requirements.txt`

(a) v0.30.0 aarch64 wheel(cu129) METADATA 의 Requires-Dist 를 권위 소스로 한 런타임 의존 목록이다 — 베이스가 주는 torch 계열은 제외하고, `fastapi<0.137.0` · `transformers<5.15.0` 천장(KNOWN_INCOMPAT)이 붙어 있다. 이 셀이 실제로 쓴 커널 계열 핀으로 `flashinfer-python==0.6.18.post1`(엔진이 고른 NvFp4 MoE 백엔드 `FLASHINFER_CUTLASS`) 이 있다. (b) 특정 벽을 고치는 파일이 아니라 소스빌드 트랙의 의존 입력이다(빌드 원장 sha256 일치 — 1.1). (c) 쓰인 바이트임이 관측으로 확인됐다. (d) vLLM 버전이 바뀌면 그 버전 wheel METADATA 로 다시 생성한다 — 이 파일을 다른 버전에 그대로 쓰지 않는다.

### compose · `docker-compose.yaml` · `serve_runner.sh` · env 형상 3종 · `sub_recipe.json`

(a) compose 는 master(Ray head + vllm serve) · slave(Ray worker) 서비스를 같은 이미지로 띄우고 env_file 로 셀 env · `.env.cluster` · `.env.interconnect` 를 받는다. `serve_runner.sh` 는 `NODE_ROLE` 로 분기해 master 는 Ray head 기동 → GPU 합류 대기 → 셀 러너 source, slave 는 head 대기 → Ray worker `--block` 이다. env 형상 3종은 manifest 값이 들어갈 자리표시 사본이다. `sub_recipe.json` 은 서브에 전달돼야 하는 env 와 이유를 기계가 파생한 것이다(1.5). (b) 분산 서빙 경로 자체라 빼면 이 셀이 뜨지 않는다 — 특정 W 를 고치는 파일은 아니다. (c) 측정을 서빙한 경로다(스모크 PASS · `testlog_26101017` §1). 단 `docker-compose.yaml` 은 측정 전 마지막 커밋과 바이트가 같다는 것까지만 확인됐고 "쓰인 바이트임을 관측으로 확인" 은 아니오다(1.1). 엔진 로그의 `Unknown vLLM environment variable detected: VLLM_PLE_MMAP` 경고는 compose 에 남은 옛 패치 62 용 env 가 0.30.0 에서 무해하게 무시된 것이다(`testlog_26100109` §3 O2). (d) `serve_runner.sh` 의 GPU 합류 리터럴(아래 79행 `"2"`)은 노드당 GPU 1 · 노드 2 에 묶여 있어 다른 클러스터 형상이면 고쳐야 한다(1.5 사실 블록 `runner_gpu_join_literal`).

> [원문] serve_runner.sh §L78-80
>         GPU_COUNT=$(ray status 2>/dev/null | grep -oP '\d+(?=\.\d+/\d+\.\d+ GPU)' | head -1)
>         if [ "${GPU_COUNT}" = "2" ] 2>/dev/null || ray status 2>/dev/null | grep -q "0.0/2.0 GPU"; then
>             echo "[master] Slave node connected. Cluster ready with 2 GPUs."

### triplet · `q38fn030-nv4-res-262k.yaml` · `.sh` · `.env.q38fn030-nv4-res-262k`

(a) 서빙 노브 12개(1.3) · 러너(`vllm serve --config` + `--served-model-name`) · 셀 env(이미지 태그 · Dockerfile 선택자 · `VLLM_REF` · 컨테이너 이름). yaml 머리 주석이 이 셀의 선언 축과 "참조(승계 아님)" 관계, 0.30.0 의 `VLLM_PLE_CPU_OFFLOAD=1` 기본값 경고를 적는다. (b) W5(eager) · W7(gmu · KV · 배치) · W10(engram-config)의 해소가 노브로 여기 있다. (c) 측정이 이 config 로 돌았다(엔진 로그 non-default args 가 yaml 값과 같다 — 1.4 (f)). (d) 노브마다의 폐기 조건은 1.3 · 0.5 를 본다.

### 싣지 않은 슬롯

- `build_patch_pre` · `build_patch_post`: 대표(메인) 측정 이미지의 빌드 원장에 적용 판정이 없다(1.1) — 저장소의 패치 파일들은 대표 원장 밖이라 실리지 않았다(1.1 '싣지 않은 파일' 사유). 메인 측정 이미지에 실린 빌드 패치가 없었다는 관측이지 필요 없다는 판정이 아니다(0.4 "이 모델에 필요한 패치" — 필요 여부 미판정 · 이전 2026-10-01 측정의 양 노드 이미지는 post 10 · 20 · 30 · 40 을 지녔다 — `testlog_26100109` §1). 서브 이미지의 원장은 메인과 다르다 — attestation 이 서브에서 post 패치 10 · 20 · 30 · 40 applied, pre 50 · 55 skipped 를 기록했다(1.5 (4) · Q4). 그 패치들은 이 페이로드에 실리지 않았고 이 모델에 필요한지 판정되지 않았다.
- `runtime_patch`: `q38fn030-nv4-res-262k_patch.py` 가 없어 `arm_patch.sh` 는 no-op 이다(1.1).
- `fork_pin`: 셀 env 에 `VARIANT` 줄이 없다 = stock(1.1).

## 1.3 값의 지위표
> 서빙 설정의 노브 **전부**와 기계가 모은 후보 지위다(lockset 출처 · yaml 주석 · 엔진 기본값). 최종 지위는 아래 `kind: value-status` 블록이 정한다 — 표의 노브마다 블록이 정확히 하나 있어야 린터를 통과한다.

<!-- FACT:value_status -->
서빙 노브 12개 — 노브마다 `kind: value-status` 블록이 **정확히 1개** 있어야 한다(노브·값은 이 표의 글자 그대로 · 값 칸의 `\|` 는 `|` 로 적는다).

| 노브 | 값 | 후보 지위 | 후보 근거 | yaml 주석 | 출처 |
|---|---|---|---|---|---|
| `tensor-parallel-size` | 2 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `distributed-executor-backend` | ray | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `gpu-memory-utilization` | 0.80 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `max-model-len` | 262144 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `kv-cache-dtype` | auto | engine-default | 값 `auto` = 엔진이 고른다 | — | serving-yaml 값 토큰 |
| `kv-cache-memory-bytes` | 8589934592 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `enforce-eager` | true | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `async-scheduling` | false | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `speculative-config` | {"method":"mtp","num_speculative_tokens":3} | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `max-num-seqs` | 8 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `max-num-batched-tokens` | 2048 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `engram-config` | {"cpu_offload": false} | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
<!-- /FACT:value_status -->

> 이 절이 답하는 질문: 각 서빙 노브의 값은 이 셀에서 어떤 지위인가 — 조정됐나, 승계됐나, 음성대조로 일부러 둔 것인가, 엔진 기본값인가, 필요조건인가?

```hint-event
id: V1
kind: value-status
노브: tensor-parallel-size
값: 2
지위: inherited
근거: 토폴로지 결정(노드당 GPU 1 × 2노드) — 캠페인 출발부터 TP=2 고정이며 다른 값을 잰 기록이 없다
출처: [plan_26090918_qwen38_flashnext_multi_딥캠페인 §1. 목표, plan_26100107_vllm030_bump_서브지침배달_멀티싱글_E2E캠페인 §2]
```

```hint-event
id: V2
kind: value-status
노브: distributed-executor-backend
값: ray
지위: inherited
근거: 멀티 서빙 경로(serve_runner.sh 의 Ray head/worker)가 전제하는 값 — 다른 executor 를 시도한 기록 없음
출처: [plan_26090918_qwen38_flashnext_multi_딥캠페인 §4. 단계 계획, serve_runner.sh]
```

```hint-event
id: V3
kind: value-status
노브: gpu-memory-utilization
값: 0.80
지위: tuned
근거: 0.85 → 0.80 으로 바꿔 최저 MemAvailable 이 +390 → +5,952MiB 로 열렸다(B1-c · W7) — 0.80 보다 낮은 값은 재지 않았다
출처: [testlog_26091304_qwen38fn_res최소조건_262k재발행_판정 §3.2]
```

```hint-event
id: V4
kind: value-status
노브: max-model-len
값: 262144
지위: inherited
근거: 사람이 고른 이 셀의 통제변인(네이티브 최대 위치) — 512k/1m 은 YaRN 오버라이드가 필요한 다른 셀이고 이 셀에서 조정하지 않았다
출처: [plan_26100107_vllm030_bump_서브지침배달_멀티싱글_E2E캠페인 §2, testlog_26091009_qwen38fn_24셀_판정 §void 셀]
```

```hint-event
id: V5
kind: value-status
노브: kv-cache-dtype
값: auto
지위: engine-default
근거: auto = 엔진이 고른다(엔진 config kv_cache_dtype=auto · bf16 계산) — fp8 은 0.30.0 에서 패치 64 parked 로 열리지 않았다(R7)
출처: [plan_26100107_vllm030_bump_서브지침배달_멀티싱글_E2E캠페인 §3, output/multi/benchlog/sweep_q38fn030-nv4-res-262k/lite_engine_head_q38fn030-nv4-res-262k.log]
```

```hint-event
id: V6
kind: value-status
노브: kv-cache-memory-bytes
값: 8589934592
지위: tuned
근거: 20,480 → 8,192MiB/노드로 바꿔 생사를 쟀다(B1-a) — 262k bf16 필요량 3,072MiB/노드의 약 2.7배로 최적값 측정은 없다 · 더 낮추는 것은 무의미 판정(R4)
출처: [testlog_26091304_qwen38fn_res최소조건_262k재발행_판정 §3.2, plan_26091216_qwen38fn_res최소조건_후속캠페인 §2.4]
```

```hint-event
id: V7
kind: value-status
노브: enforce-eager
값: true
지위: declared-requirement
근거: W5 — 그래프 캡처 구간이 워치독 임계를 넘어 사살됐다(KV 20GiB · gmu 0.85 · 0.29.0rc6 조건) · 지금 형상에서 캡처가 들어가는지는 미결(Q3)
출처: [testlog_26090921_p0-1_nvfp4_mixed_패치_스모크_판정 §판정, testlog_26091001_p0-2_ple_mmap_p0-3_qsa_fp8kv_스모크_판정 §3. P0-3]
```

```hint-event
id: V8
kind: value-status
노브: async-scheduling
값: false
지위: inherited
근거: 커뮤니티 선례의 MTP+async 문맥 손상 함정을 피하는 예방값 — 0.29 는 mtp 와 함께 async 를 자동으로 끄지 않아 명시했다 · 이 계보에서 켜고 잰 기록은 없다
출처: [testlog_26091001_p0-2_ple_mmap_p0-3_qsa_fp8kv_스모크_판정 §1, plan_26090918_qwen38_flashnext_multi_딥캠페인 §2.3]
```

```hint-event
id: V9
kind: value-status
노브: speculative-config
값: '{"method":"mtp","num_speculative_tokens":3}'
지위: inherited
근거: 커뮤니티 선례의 MTP k=3 — k ∈ {1,2,3} 레버는 plan 레버 서브스윕에 있었으나 이 res 계열의 레버 지도는 L0-base 1셀만 시도(계획/시도/남음 1/1/0) · k 를 바꿔 잰 기록 없음
출처: [plan_26090918_qwen38_flashnext_multi_딥캠페인 §3. 셀 매트릭스, sweep_map_26091008_nv4-262k-res]
```

```hint-event
id: V10
kind: value-status
노브: max-num-seqs
값: 8
지위: tuned
근거: 미설정 → 8 로 배치 억제(B1-b · 실효 544MiB 의 일부)로 사살 −154 → 생존 +390 — 16 이상은 재지 않았다(Q5) · 이 셀 스윕의 레벨 16 을 제외시킨 상한
출처: [testlog_26091304_qwen38fn_res최소조건_262k재발행_판정 §3.2, testlog_26101017_knob프리셋_E2E_셀M1_q38fn030_nv4_res_262k_판정 §2]
```

```hint-event
id: V11
kind: value-status
노브: max-num-batched-tokens
값: 2048
지위: tuned
근거: max-num-seqs 와 함께 B1-b 배치 억제로 넣어 생사를 쟀다 — 0.30.0 엔진은 MTP k=3 때문에 max_num_scheduled_tokens 를 2048 로 잡는다고 경고한다
출처: [testlog_26091304_qwen38fn_res최소조건_262k재발행_판정 §3.2, output/multi/benchlog/sweep_q38fn030-nv4-res-262k/lite_engine_head_q38fn030-nv4-res-262k.log]
```

```hint-event
id: V12
kind: value-status
노브: engram-config
값: '{"cpu_offload": false}'
지위: inherited
근거: 0.30.0 의 VLLM_PLE_CPU_OFFLOAD 기본 1 을 코드 판독으로 막은 손레버(W10) — 빼고 띄운 음성대조 없음 · 엔진 로그 EngramConfig(cpu_offload=False) 와 PLE weight_device=cuda:0 로 적용은 관측됨
출처: [plan_26100107_vllm030_bump_서브지침배달_멀티싱글_E2E캠페인 §3, output/multi/benchlog/sweep_q38fn030-nv4-res-262k/lite_engine_q38fn030-nv4-res-262k.log]
```

복사하면 위험한 값은 셋이다. `kv-cache-memory-bytes`(V6)는 이 HW · 이 예산 산식에서 생존을 가른 계단 값이지 필요량이 아니다 — 네 환경에서는 토큰당 KV × 목표 컨텍스트로 다시 도출한다. `gpu-memory-utilization`(V3)은 통합메모리 노드에서 로드 피크의 천장을 정하는 레버라 메모리 구조가 다르면 의미가 달라진다. `enforce-eager`(V7)는 필요조건으로 기록돼 있지만 그 실패는 다른 KV · gmu 조건에서 관측된 것이다. 나머지 승계값(V1 · V2 · V4 · V8 · V9 · V12)은 조정된 적이 없으므로 최적이라는 뜻이 아니다.

## 1.4 재현 절차
> 셀 실행 기록에서 기계가 파생한 명령 순서와 단계별 소요다. 소요가 `미관측` 인 단계는 시각 기록이 없다는 뜻이다(0 이 아니다). 그 아래는 블랙박스 이벤트 원장에서 이 셀의 행을 기계가 뽑아 기동 시도로 묶은 것이고, 마지막은 측정한 실행의 엔진 로그가 echo 한 env 다(측정 당시 값의 정본).

<!-- FACT:reproduce -->
| 순서 | 단계 | 성공 판정 | 소요 | 관측 구간(UTC) | 소요 출처 | 명령 출처 |
|---|---|---|---|---|---|---|
| 1 | render | 러너 3종·.env·.env.cluster·.env.interconnect 생성(렌더러 fail-loud) | 미관측 | 미관측 | .claude/skills/upstream-version-watch/scripts/multinode_serve_smoke.sh 선행조건 안내 · .claude/skills/upstream-version-watch/scripts/render_dockerfile.py CLI | derived(렌더러 CLI) |
| 2 | build | 빌드 OK · 이미지 실재 | 층 창 36분 33초(docker history 첫 층 → 끝 층 CreatedAt · 캐시 재사용 층 포함 — 빌드 소요 아님) | 2026-10-01T07:38:01Z → 2026-10-01T08:14:34Z (layer-window) | docker history --no-trunc --format '{{json .}}' sha256:123c331f7a5266b069fcb8b9b28009e862d5662f50dcd12e9321fa08937c1c16 · 우리 Dockerfile 층 30개의 CreatedAt 첫 층 → 끝 층(캐시 재사용 층은 이전 빌드 시각 — 빌드 소요 아님) | reconstructed(build-ledger build_args + compose build) |
| 3 | serve | health 200 + 추론 1회(스모크 판정) | ≤ 31분 13초(상한) | 2026-10-10T03:19:17Z → 2026-10-10T03:50:30Z (upper) | docs/logs/main/events/2026-10.jsonl budget_declare(label smoke-q38fn030-nv4-res-262k) → 첫 측정 시각(output/multi/benchlog/sweep_q38fn030-nv4-res-262k/lite_raw_q38fn030-nv4-res-262k.json measured_utc(호스트 date -u · lite_bench.sh 가 cold 레그 직전에 찍는 측정 시작)) | derived(스모크 사용법) |
| 4 | bench | 리포트 발행(+PASS 면 인증서) | 16분 43초 | 2026-10-10T03:50:30Z → 2026-10-10T04:07:13Z (exact) | output/multi/benchlog/sweep_q38fn030-nv4-res-262k/lite_raw_q38fn030-nv4-res-262k.json measured_utc(호스트 date -u · lite_bench.sh 가 cold 레그 직전에 찍는 측정 시작) → sweep_index.generated_utc(호스트 UTC · 두 끝 같은 시계) | reconstructed(bench json) |

**빌드 층 시각**(관측 · 소요가 아니라 층 창 — 캐시 재사용 층은 이전 빌드의 시각을 가진다)

- `build` 이미지 층 CreatedAt(docker history): 2026-10-01T07:38:01Z → 2026-10-01T08:14:34Z · 층 30개 · 출처 `docker history --no-trunc --format '{{json .}}' sha256:123c331f7a5266b069fcb8b9b28009e862d5662f50dcd12e9321fa08937c1c16` — 우리 Dockerfile 층의 CreatedAt 범위 — 캐시 재사용 층은 이전 빌드 시각을 가진다(소요 아님)

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
  --build-arg VLLM_REF=v0.30.0 \
  --build-arg VLLM_REPO=https://github.com/vllm-project/vllm.git \
  -t easy-vllm:0.30.0-cu133-aarch64-source \
  output/multi
# 동등 경로(스모크 사용법): bash .claude/skills/upstream-version-watch/scripts/multinode_serve_smoke.sh q38fn030-nv4-res-262k --build-only
```

**3. serve**

```sh
bash .claude/skills/upstream-version-watch/scripts/multinode_serve_smoke.sh q38fn030-nv4-res-262k --keep-up
```

**4. bench**

```sh
# bench JSON 필드로 복원: backend · model_id→--model · tokenizer_id→--tokenizer · num_prompts · max_concurrency · request_rate · burstiness · 요청당 입출력 길이 = total_*_tokens ÷ completed(나누어떨어질 때만 — 평균이다 · 요청별 길이는 JSON 에 없다) · 나누어떨어지면 `--dataset-name random` 으로 추정했다(측정 도구가 그 꼴로 부른다 — 아래 도구 원문).
# JSON 에 없는 인자(--endpoint · --base-url · --ignore-eos · --num-warmups · --temperature · --seed · --random-range-ratio · --trust-remote-code)는 측정 도구가 정했다 — level_* = run_bench.sh · lite_cold/lite_warm = lite_bench.sh(.claude/skills/adversarial-benchmark/scripts/) · 셀 스윕 순서는 sweep_bench.sh.
# output/multi/benchlog/sweep_q38fn030-nv4-res-262k/lite_cold_q38fn030-nv4-res-262k.json (date 20261010-035048) · ⚠ --random-input-len/--random-output-len = total ÷ completed 평균(chat 템플릿 포함 실측 토큰 · 측정 도구 원문 인자 미관측 — lite_bench.sh 원문 인자와 다를 수 있다) · spec_decode_acceptance_length 2.04762(이 레그 JSON)
vllm bench serve --backend openai --model qwen3.8-flash-next --tokenizer /app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 --dataset-name random --random-input-len 512 --random-output-len 128 --num-prompts 1 --max-concurrency 1 --request-rate inf
# output/multi/benchlog/sweep_q38fn030-nv4-res-262k/lite_warm_q38fn030-nv4-res-262k.json (date 20261010-035112) · ⚠ --random-input-len/--random-output-len = total ÷ completed 평균(chat 템플릿 포함 실측 토큰 · 측정 도구 원문 인자 미관측 — lite_bench.sh 원문 인자와 다를 수 있다) · spec_decode_acceptance_length 2.07527(이 레그 JSON)
vllm bench serve --backend openai --model qwen3.8-flash-next --tokenizer /app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 --dataset-name random --random-input-len 512 --random-output-len 128 --num-prompts 3 --max-concurrency 1 --request-rate inf
```

**슬롯 → 빌드 컨텍스트 매핑**(빌드 컨텍스트 = `output/<토폴로지>/` · 원천 = 실린 Dockerfile 의 COPY · compose 의 env_file · volumes · command — `—` 는 빌드 컨텍스트 입력이 아니다)

| zip 경로 | 빌드 컨텍스트 자리 | 근거 |
|---|---|---|
| `artifacts/triplet/.env.q38fn030-nv4-res-262k` | `envs/.env.q38fn030-nv4-res-262k` | compose env_file envs/.env.${CONFIG_FILE}(셀 env) |
| `artifacts/triplet/q38fn030-nv4-res-262k.sh` | `configs/q38fn030-nv4-res-262k.sh` | compose volumes ./configs:/app/configs |
| `artifacts/triplet/q38fn030-nv4-res-262k.yaml` | `configs/q38fn030-nv4-res-262k.yaml` | compose volumes ./configs:/app/configs |
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

**기동 성공 판정(관측 · 발행 자격)**: health 200 = true · 추론 1회 = true · 근거 `output/multi/benchlog/serve_proof_q38fn030-nv4-res-262k.json` — 시점 범위: 작성 2026-10-10T03:50:09Z(output/multi/benchlog/serve_proof_q38fn030-nv4-res-262k.json 파일 mtime(파일시스템 관측 — 문서에 시각 칸이 없다 · 기록 필드 아님)) · 측정 창 대비 작성 2026-10-10T03:50:09Z < 측정 시작 2026-10-10T03:50:30Z — 측정 전(빌드·스모크)의 관측
<!-- /FACT:reproduce -->

<!-- FACT:event_timeline -->
**블랙박스 이벤트 47행**(원장에서 이 셀 label·config 에 맞는 행만 · 시각순 · 기계 발췌 · 이번 캠페인 26행 · 같은 셀 이름 · 앞선 캠페인 21행(내용 칸에 표기))

> **캠페인 경계** 2026-10-10T03:19:17Z(`camp-26101104-hint-m1` · evidence.event_campaign_boundary): campaigns/camp-26101104-hint-m1/campaign.yaml declared_utc 2026-10-10T19:53:02Z · 이 발행이 묶인 측정을 서빙한 budget_declare 창 시작 2026-10-10T03:19:17Z → 경계 = 앞선 값 2026-10-10T03:19:17Z(이전 행 = 같은 셀 이름 · 앞선 캠페인)

> **원장 관측 범위**(노드별 첫 행 → 마지막 행 · evidence.event_ledger_spans): main = 2026-07-31T03:27:43Z → 2026-10-10T15:21:39Z(`docs/logs/main/events/2026-07.jsonl` · `docs/logs/main/events/2026-08.jsonl` · `docs/logs/main/events/2026-09.jsonl` · `docs/logs/main/events/2026-10.jsonl` · `docs/logs/main/events/thermal.jsonl` · `docs/logs/main/events/watchdog.jsonl`) · sub = 2026-09-03T09:51:45Z → 2026-09-11T11:10:38Z(`docs/logs/sub/events/2026-09.jsonl`) — ⚠ sub(마지막 행 2026-09-11T11:10:38Z < 측정 끝 2026-10-10T04:07:13Z): 그 뒤 그 노드의 선언 · 사건은 **관측 범위 밖**이다(없음이 아니다 · 원장 미회수) — 단 아래 보조 관측의 행은 예외
>
> **보조 관측 · sub**(sub 원장 미러 밖 — 스모크 로그 echo · 같은 기동 판별(attestation_same_boot 강한 판별)이 지목한 스모크 로그 docs/simlog/26101012_벤치_knob_프리셋_E2E/m1_serve.log 의 `sub:` 원장 JSON echo 줄): `budget_honored` 2026-10-10T03:19:21Z(`smoke-q38fn030-nv4-res-262k`) — `docs/simlog/26101012_벤치_knob_프리셋_E2E/m1_serve.log:26` — 이 기동에서 sub 에 대해 관측된 것은 `budget_honored` 뿐이다 · 그 밖의 sub 사건(사살 · 트립 · 갱신 · clear)은 여전히 **관측 범위 밖**이다(없음이 아니다)

| # | UTC | 노드 | kind | label | 내용 | 출처 |
|---|---|---|---|---|---|---|
| 1 | 2026-09-30T23:58:21Z | main | `budget_declare` | `smoke-q38fn030-nv4-res-262k` | 예산 선언(선언값 — 측정 아님) · floor_mib=28350 · weights_mib=63266 · kv_mib=8192 · overhead_mib=24800 · mem_total_mib=124608 · 기동 창 1/2: budget_renew 16회 · 닫힘 budget_clear 2026-10-01T00:58:02Z(선언 후 59m41s) · measured_utc 2026-10-10T04:07:13Z 는 이 창 밖(다른 기동 · 측정 전) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T03:19:17Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:1139 |
| 2 | 2026-09-30T23:58:22Z | main | `budget_honored` | `smoke-q38fn030-nv4-res-262k` | budget_honored · floor_mib=28350 · arm_ceiling_mib=25278 · remaining_s=7199 · 창 시작 2026-09-30T23:58:22Z 후 0m00s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T03:19:17Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:553 |
| 3 | 2026-09-30T23:59:35Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=81597 · rate_mib_s=23773 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → smoke-q38fn030-nv4-res-262k 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-30T23:58:22Z 후 1m13s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T03:19:17Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:554 |
| 4 | 2026-10-01T00:29:25Z | main | `budget_renew` | `smoke-q38fn030-nv4-res-262k` | budget_renew · floor_mib=28350 · ttl_s=7200 · remaining_before_s=5336 · 창 시작 2026-09-30T23:58:21Z 후 31m04s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T03:19:17Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:1 |
| 5 | 2026-10-01T00:38:07Z | main | `budget_renew` | `smoke-q38fn030-nv4-res-262k` | budget_renew · floor_mib=28350 · ttl_s=7200 · remaining_before_s=6678 · 창 시작 2026-09-30T23:58:21Z 후 39m46s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T03:19:17Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:2 |
| 6 | 2026-10-01T00:40:24Z | main | `budget_renew` | `smoke-q38fn030-nv4-res-262k` | budget_renew · floor_mib=28350 · ttl_s=7200 · remaining_before_s=7063 · 창 시작 2026-09-30T23:58:21Z 후 42m03s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T03:19:17Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:3 |
| 7 | 2026-10-01T00:42:44Z | main | `budget_renew` | `smoke-q38fn030-nv4-res-262k` | budget_renew · floor_mib=28350 · ttl_s=7200 · remaining_before_s=7060 · 창 시작 2026-09-30T23:58:21Z 후 44m23s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T03:19:17Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:4 |
| 8 | 2026-10-01T00:45:02Z | main | `budget_renew` | `smoke-q38fn030-nv4-res-262k` | budget_renew · floor_mib=28350 · ttl_s=7200 · remaining_before_s=7062 · 창 시작 2026-09-30T23:58:21Z 후 46m41s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T03:19:17Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:5 |
| 9 | 2026-10-01T00:46:32Z | main | `budget_renew` | `smoke-q38fn030-nv4-res-262k` | budget_renew · floor_mib=28350 · ttl_s=7200 · remaining_before_s=7110 · 창 시작 2026-09-30T23:58:21Z 후 48m11s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T03:19:17Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:6 |
| 10 | 2026-10-01T00:48:00Z | main | `budget_renew` | `smoke-q38fn030-nv4-res-262k` | budget_renew · floor_mib=28350 · ttl_s=7200 · remaining_before_s=7112 · 창 시작 2026-09-30T23:58:21Z 후 49m39s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T03:19:17Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:7 |
| 11 | 2026-10-01T00:49:25Z | main | `budget_renew` | `smoke-q38fn030-nv4-res-262k` | budget_renew · floor_mib=28350 · ttl_s=7200 · remaining_before_s=7115 · 창 시작 2026-09-30T23:58:21Z 후 51m04s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T03:19:17Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:8 |
| 12 | 2026-10-01T00:50:25Z | main | `budget_renew` | `smoke-q38fn030-nv4-res-262k` | budget_renew · floor_mib=28350 · ttl_s=7200 · remaining_before_s=7140 · 창 시작 2026-09-30T23:58:21Z 후 52m04s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T03:19:17Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:9 |
| 13 | 2026-10-01T00:51:30Z | main | `budget_renew` | `smoke-q38fn030-nv4-res-262k` | budget_renew · floor_mib=28350 · ttl_s=7200 · remaining_before_s=7135 · 창 시작 2026-09-30T23:58:21Z 후 53m09s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T03:19:17Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:10 |
| 14 | 2026-10-01T00:52:30Z | main | `budget_renew` | `smoke-q38fn030-nv4-res-262k` | budget_renew · floor_mib=28350 · ttl_s=7200 · remaining_before_s=7140 · 창 시작 2026-09-30T23:58:21Z 후 54m09s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T03:19:17Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:11 |
| 15 | 2026-10-01T00:53:15Z | main | `budget_renew` | `smoke-q38fn030-nv4-res-262k` | budget_renew · floor_mib=28350 · ttl_s=7200 · remaining_before_s=7155 · 창 시작 2026-09-30T23:58:21Z 후 54m54s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T03:19:17Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:12 |
| 16 | 2026-10-01T00:53:59Z | main | `budget_renew` | `smoke-q38fn030-nv4-res-262k` | budget_renew · floor_mib=28350 · ttl_s=7200 · remaining_before_s=7156 · 창 시작 2026-09-30T23:58:21Z 후 55m38s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T03:19:17Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:13 |
| 17 | 2026-10-01T00:54:43Z | main | `budget_renew` | `smoke-q38fn030-nv4-res-262k` | budget_renew · floor_mib=28350 · ttl_s=7200 · remaining_before_s=7156 · 창 시작 2026-09-30T23:58:21Z 후 56m22s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T03:19:17Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:14 |
| 18 | 2026-10-01T00:55:29Z | main | `budget_renew` | `smoke-q38fn030-nv4-res-262k` | budget_renew · floor_mib=28350 · ttl_s=7200 · remaining_before_s=7154 · 창 시작 2026-09-30T23:58:21Z 후 57m08s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T03:19:17Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:15 |
| 19 | 2026-10-01T00:56:12Z | main | `budget_renew` | `smoke-q38fn030-nv4-res-262k` | budget_renew · floor_mib=28350 · ttl_s=7200 · remaining_before_s=7157 · 창 시작 2026-09-30T23:58:21Z 후 57m51s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T03:19:17Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:16 |
| 20 | 2026-10-01T00:58:02Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-q38fn030-nv4-res-262k 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-30T23:58:21Z 후 59m41s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T03:19:17Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:17 |
| 21 | 2026-10-01T00:58:03Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-q38fn030-nv4-res-262k 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-30T23:58:22Z 후 59m41s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T03:19:17Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:555 |
| 22 | 2026-10-10T03:19:17Z | main | `budget_declare` | `smoke-q38fn030-nv4-res-262k` | 예산 선언(선언값 — 측정 아님) · floor_mib=28350 · weights_mib=63266 · kv_mib=8192 · overhead_mib=24800 · mem_total_mib=124608 · 기동 창 2/2: budget_renew 13회 · 닫힘 budget_clear 2026-10-10T04:07:56Z(선언 후 48m39s) · measured_utc 2026-10-10T04:07:13Z 가 이 창 안(이 측정의 기동) | docs/logs/main/events/2026-10.jsonl:288 |
| 23 | 2026-10-10T03:19:18Z | main | `budget_honored` | `smoke-q38fn030-nv4-res-262k` | budget_honored · floor_mib=28350 · arm_ceiling_mib=25278 · remaining_s=7199 · 창 시작 2026-10-10T03:19:18Z 후 0m00s | docs/logs/main/events/watchdog.jsonl:706 |
| 24 | 2026-10-10T03:20:39Z | cluster | `engine_log_window_start` | — | 캡처된 엔진 로그의 첫 시각(꼬리 캡처일 수 있다 — 엔진 시작 시각이 아닐 수 있다) · 선언 2026-10-10T03:19:17Z 후 1m22s · 시계: 컨테이너 시각을 UTC 로 읽어 측정 창 [2026-10-10T03:19:17Z, 2026-10-10T04:07:56Z] 안에 드는 것을 확인 | output/multi/benchlog/sweep_q38fn030-nv4-res-262k/lite_engine_q38fn030-nv4-res-262k.log:314 |
| 25 | 2026-10-10T03:20:43Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=82101 · rate_mib_s=23690 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → smoke-q38fn030-nv4-res-262k 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-10T03:19:18Z 후 1m25s) | docs/logs/main/events/watchdog.jsonl:707 |
| 26 | 2026-10-10T03:20:45Z | cluster | `engine_prefetch_disabled` | — | 가중치 auto-prefetch 꺼짐 줄 ×4 — filesystem CIFS(vLLM 로그 문구) | output/multi/benchlog/sweep_q38fn030-nv4-res-262k/lite_engine_q38fn030-nv4-res-262k.log:578 |
| 27 | 2026-10-10T03:35:20Z | cluster | `engine_weights_loaded` | — | 가중치 적재 완료 줄 ×4(rank·단계별) · 최장 485.20s · 이 행 = 마지막 줄 | output/multi/benchlog/sweep_q38fn030-nv4-res-262k/lite_engine_q38fn030-nv4-res-262k.log:640 |
| 28 | 2026-10-10T03:35:31Z | cluster | `engine_model_loaded` | — | 모델 적재 완료 줄 ×2(rank 별) · 마지막 줄 62.89 GiB · 889.8s | output/multi/benchlog/sweep_q38fn030-nv4-res-262k/lite_engine_q38fn030-nv4-res-262k.log:643 |
| 29 | 2026-10-10T03:36:33Z | cluster | `engine_shm_broadcast_wait` | — | shm_broadcast 대기 경고(60s 단위) ×13 · 2026-10-10T03:36:33Z–2026-10-10T03:49:09Z(12m36s) — vLLM 문구상 프로세스 hang 또는 긴 작업(컴파일·가중치/KV 양자화) 중 출현 | output/multi/benchlog/sweep_q38fn030-nv4-res-262k/lite_engine_q38fn030-nv4-res-262k.log:652 |
| 30 | 2026-10-10T03:48:08Z | cluster | `engine_kv_cache_sized` | — | GPU KV cache 537,040 tokens · 요청당 262,144 tokens 기준 최대 동시성 2.05x | output/multi/benchlog/sweep_q38fn030-nv4-res-262k/lite_engine_q38fn030-nv4-res-262k.log:672 |
| 31 | 2026-10-10T03:49:29Z | cluster | `engine_init_done` | — | 엔진 초기화(profile·KV 할당·warmup) 836.93s | output/multi/benchlog/sweep_q38fn030-nv4-res-262k/lite_engine_q38fn030-nv4-res-262k.log:721 |
| 32 | 2026-10-10T03:49:55Z | cluster | `server_starting` | — | API 서버 기동 — 이 측정 창 선언 2026-10-10T03:19:17Z 후 30m38s · 캡처 로그 첫 시각 2026-10-10T03:20:39Z 후 29m16s | output/multi/benchlog/sweep_q38fn030-nv4-res-262k/lite_engine_q38fn030-nv4-res-262k.log:746 |
| 33 | 2026-10-10T03:50:09Z | main | `budget_renew` | `smoke-q38fn030-nv4-res-262k` | budget_renew · floor_mib=28350 · ttl_s=7200 · remaining_before_s=5348 · 창 시작 2026-10-10T03:19:17Z 후 30m52s | docs/logs/main/events/2026-10.jsonl:289 |
| 34 | 2026-10-10T03:51:14Z | main | `budget_renew` | `smoke-q38fn030-nv4-res-262k` | budget_renew · floor_mib=28350 · ttl_s=7200 · remaining_before_s=7135 · 창 시작 2026-10-10T03:19:17Z 후 31m57s | docs/logs/main/events/2026-10.jsonl:290 |
| 35 | 2026-10-10T03:53:35Z | main | `budget_renew` | `smoke-q38fn030-nv4-res-262k` | budget_renew · floor_mib=28350 · ttl_s=7200 · remaining_before_s=7059 · 창 시작 2026-10-10T03:19:17Z 후 34m18s | docs/logs/main/events/2026-10.jsonl:291 |
| 36 | 2026-10-10T03:55:42Z | main | `budget_renew` | `smoke-q38fn030-nv4-res-262k` | budget_renew · floor_mib=28350 · ttl_s=7200 · remaining_before_s=7073 · 창 시작 2026-10-10T03:19:17Z 후 36m25s | docs/logs/main/events/2026-10.jsonl:292 |
| 37 | 2026-10-10T03:57:55Z | main | `budget_renew` | `smoke-q38fn030-nv4-res-262k` | budget_renew · floor_mib=28350 · ttl_s=7200 · remaining_before_s=7067 · 창 시작 2026-10-10T03:19:17Z 후 38m38s | docs/logs/main/events/2026-10.jsonl:293 |
| 38 | 2026-10-10T03:59:20Z | main | `budget_renew` | `smoke-q38fn030-nv4-res-262k` | budget_renew · floor_mib=28350 · ttl_s=7200 · remaining_before_s=7115 · 창 시작 2026-10-10T03:19:17Z 후 40m03s | docs/logs/main/events/2026-10.jsonl:294 |
| 39 | 2026-10-10T04:00:47Z | main | `budget_renew` | `smoke-q38fn030-nv4-res-262k` | budget_renew · floor_mib=28350 · ttl_s=7200 · remaining_before_s=7113 · 창 시작 2026-10-10T03:19:17Z 후 41m30s | docs/logs/main/events/2026-10.jsonl:295 |
| 40 | 2026-10-10T04:02:10Z | main | `budget_renew` | `smoke-q38fn030-nv4-res-262k` | budget_renew · floor_mib=28350 · ttl_s=7200 · remaining_before_s=7117 · 창 시작 2026-10-10T03:19:17Z 후 42m53s | docs/logs/main/events/2026-10.jsonl:296 |
| 41 | 2026-10-10T04:03:07Z | main | `budget_renew` | `smoke-q38fn030-nv4-res-262k` | budget_renew · floor_mib=28350 · ttl_s=7200 · remaining_before_s=7143 · 창 시작 2026-10-10T03:19:17Z 후 43m50s | docs/logs/main/events/2026-10.jsonl:297 |
| 42 | 2026-10-10T04:04:00Z | main | `budget_renew` | `smoke-q38fn030-nv4-res-262k` | budget_renew · floor_mib=28350 · ttl_s=7200 · remaining_before_s=7147 · 창 시작 2026-10-10T03:19:17Z 후 44m43s | docs/logs/main/events/2026-10.jsonl:298 |
| 43 | 2026-10-10T04:05:00Z | main | `budget_renew` | `smoke-q38fn030-nv4-res-262k` | budget_renew · floor_mib=28350 · ttl_s=7200 · remaining_before_s=7140 · 창 시작 2026-10-10T03:19:17Z 후 45m43s | docs/logs/main/events/2026-10.jsonl:299 |
| 44 | 2026-10-10T04:05:43Z | main | `budget_renew` | `smoke-q38fn030-nv4-res-262k` | budget_renew · floor_mib=28350 · ttl_s=7200 · remaining_before_s=7157 · 창 시작 2026-10-10T03:19:17Z 후 46m26s | docs/logs/main/events/2026-10.jsonl:300 |
| 45 | 2026-10-10T04:06:30Z | main | `budget_renew` | `smoke-q38fn030-nv4-res-262k` | budget_renew · floor_mib=28350 · ttl_s=7200 · remaining_before_s=7153 · 창 시작 2026-10-10T03:19:17Z 후 47m13s | docs/logs/main/events/2026-10.jsonl:301 |
| 46 | 2026-10-10T04:07:56Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-q38fn030-nv4-res-262k 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-10T03:19:17Z 후 48m39s) | docs/logs/main/events/2026-10.jsonl:302 |
| 47 | 2026-10-10T04:07:57Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-q38fn030-nv4-res-262k 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-10T03:19:18Z 후 48m39s) | docs/logs/main/events/watchdog.jsonl:708 |

**기동 시도**(예산 선언 `budget_declare` 마다 한 시도 · 같은 노드의 이어지는 행과 선언 없는 노드(엔진 로그)의 그 시각 행을 묶었다 · 사살 · 트립 · 거부는 닫힘과 따로 센다 · `measurement` 행은 측정 기록이라 시도로 세지 않는다(위 표에만) — 해석은 저작자 몫)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 1 | main | `smoke-q38fn030-nv4-res-262k` | 2026-10-10T03:19:17Z | 13회 · 첫 2026-10-10T03:50:09Z | 없음 | `budget_clear` 2026-10-10T04:07:56Z | 11 |

**같은 셀 이름 · 앞선 캠페인의 기동 1회**(캠페인 경계 2026-10-10T03:19:17Z 이전 — 라벨이 같아 원장에서 함께 묶였을 뿐 이 셀의 시도로 세지 않는다 · 행 전문 = 01 §1.4)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 앞선-1 | main | `smoke-q38fn030-nv4-res-262k` | 2026-09-30T23:58:21Z | 16회 · 첫 2026-10-01T00:29:25Z | 없음 | `budget_clear` 2026-10-01T00:58:02Z | 3 |
<!-- /FACT:event_timeline -->

<!-- FACT:measurement_env -->
**측정 실행의 env 관측 29행**(측정한 실행의 엔진 로그가 echo 한 값 · 기계 발췌 — 측정 당시 값의 정본이다. 표에 없는 키는 로그가 echo 하지 않은 것이지 설정되지 않았다는 뜻이 아니다 · 측정 뒤 재생성된 env 형상은 01 §1.1 을 본다)

| # | 키 | 값 | 노드 | 종류 | 출처(첫 줄) | 관측 |
|---|---|---|---|---|---|---|
| 1 | `NCCL_SOCKET_IFNAME` | `<nic:cluster>` | main | env-echo | output/multi/benchlog/sweep_q38fn030-nv4-res-262k/lite_engine_head_q38fn030-nv4-res-262k.log:L124 | 로그 1개 |
| 2 | `NCCL_SOCKET_IFNAME` | `<nic:cluster>` | main | param-echo | output/multi/benchlog/sweep_q38fn030-nv4-res-262k/lite_engine_head_q38fn030-nv4-res-262k.log:L125 | 로그 1개 |
| 3 | `nccl:version` | `2.30.7+cuda13.3` | main | runtime | output/multi/benchlog/sweep_q38fn030-nv4-res-262k/lite_engine_head_q38fn030-nv4-res-262k.log:L128 | 로그 1개 |
| 4 | `NCCL_NET_PLUGIN` | `spcx` | main | env-echo | output/multi/benchlog/sweep_q38fn030-nv4-res-262k/lite_engine_head_q38fn030-nv4-res-262k.log:L130 | 로그 1개 |
| 5 | `NCCL_IB_DISABLE` | `0` | main | env-echo | output/multi/benchlog/sweep_q38fn030-nv4-res-262k/lite_engine_head_q38fn030-nv4-res-262k.log:L135 | 로그 1개 |
| 6 | `NCCL_IB_HCA` | `=<nic:cluster>,<nic:cluster>` | main | param-echo | output/multi/benchlog/sweep_q38fn030-nv4-res-262k/lite_engine_head_q38fn030-nv4-res-262k.log:L137 | 로그 1개 |
| 7 | `NCCL_IB_MERGE_NICS` | `1` | main | env-echo | output/multi/benchlog/sweep_q38fn030-nv4-res-262k/lite_engine_head_q38fn030-nv4-res-262k.log:L140 | 로그 1개 |
| 8 | `nccl:net_ib_devices` | `[0]<nic:cluster>:1/RoCE [1]<nic:cluster>:1/RoCE [RO]; OOB <nic:cluster>:<node:main><0>` | main | runtime | output/multi/benchlog/sweep_q38fn030-nv4-res-262k/lite_engine_head_q38fn030-nv4-res-262k.log:L143 | 로그 1개 |
| 9 | `NCCL_IB_QPS_PER_CONNECTION` | `4` | main | env-echo | output/multi/benchlog/sweep_q38fn030-nv4-res-262k/lite_engine_head_q38fn030-nv4-res-262k.log:L156 | 로그 1개 |
| 10 | `nccl:net_ib_devices` | `[RO]; OOB <nic:cluster>:<node:main><0>` | main | runtime | output/multi/benchlog/sweep_q38fn030-nv4-res-262k/lite_engine_head_q38fn030-nv4-res-262k.log:L168 | 로그 1개 |
| 11 | `nccl:network` | `IB` | main | runtime | output/multi/benchlog/sweep_q38fn030-nv4-res-262k/lite_engine_head_q38fn030-nv4-res-262k.log:L180 | 로그 1개 |
| 12 | `NCCL_DMABUF_ENABLE` | `0` | main | env-echo | output/multi/benchlog/sweep_q38fn030-nv4-res-262k/lite_engine_head_q38fn030-nv4-res-262k.log:L182 | 로그 1개 |
| 13 | `NCCL_CROSS_NIC` | `1` | main | env-echo | output/multi/benchlog/sweep_q38fn030-nv4-res-262k/lite_engine_head_q38fn030-nv4-res-262k.log:L185 | 로그 1개 |
| 14 | `NCCL_IB_SPLIT_DATA_ON_QPS` | `0` | main | env-echo | output/multi/benchlog/sweep_q38fn030-nv4-res-262k/lite_engine_q38fn030-nv4-res-262k.log:L49 | 로그 1개 |
| 15 | `NCCL_IB_GID_INDEX` | `3` | main | env-echo | output/multi/benchlog/sweep_q38fn030-nv4-res-262k/lite_engine_q38fn030-nv4-res-262k.log:L100 | 로그 3개 |
| 16 | `NCCL_SOCKET_IFNAME` | `<nic:cluster>` | sub | env-echo | output/multi/benchlog/sweep_q38fn030-nv4-res-262k/lite_engine_q38fn030-nv4-res-262k.log:L462 | 로그 12개 · Ray 접힘 +3(접힌 사본의 노드 미관측) |
| 17 | `NCCL_SOCKET_IFNAME` | `<nic:cluster>` | sub | param-echo | output/multi/benchlog/sweep_q38fn030-nv4-res-262k/lite_engine_q38fn030-nv4-res-262k.log:L463 | 로그 12개 · Ray 접힘 +2(접힌 사본의 노드 미관측) |
| 18 | `nccl:version` | `2.30.7+cuda13.3` | sub | runtime | output/multi/benchlog/sweep_q38fn030-nv4-res-262k/lite_engine_q38fn030-nv4-res-262k.log:L466 | 로그 12개 |
| 19 | `NCCL_NET_PLUGIN` | `spcx` | sub | env-echo | output/multi/benchlog/sweep_q38fn030-nv4-res-262k/lite_engine_q38fn030-nv4-res-262k.log:L468 | 로그 12개 |
| 20 | `NCCL_IB_DISABLE` | `0` | sub | env-echo | output/multi/benchlog/sweep_q38fn030-nv4-res-262k/lite_engine_q38fn030-nv4-res-262k.log:L473 | 로그 12개 · Ray 접힘 +2(접힌 사본의 노드 미관측) |
| 21 | `NCCL_IB_HCA` | `=<nic:cluster>,<nic:cluster>` | sub | param-echo | output/multi/benchlog/sweep_q38fn030-nv4-res-262k/lite_engine_q38fn030-nv4-res-262k.log:L474 | 로그 12개 · Ray 접힘 +2(접힌 사본의 노드 미관측) |
| 22 | `NCCL_IB_MERGE_NICS` | `1` | sub | env-echo | output/multi/benchlog/sweep_q38fn030-nv4-res-262k/lite_engine_q38fn030-nv4-res-262k.log:L476 | 로그 12개 |
| 23 | `nccl:net_ib_devices` | `[RO]; OOB <nic:cluster>:<node:sub><0>` | sub | runtime | output/multi/benchlog/sweep_q38fn030-nv4-res-262k/lite_engine_q38fn030-nv4-res-262k.log:L478 | 로그 12개 · Ray 접힘 +2(접힌 사본의 노드 미관측) |
| 24 | `NCCL_IB_QPS_PER_CONNECTION` | `4` | sub | env-echo | output/multi/benchlog/sweep_q38fn030-nv4-res-262k/lite_engine_q38fn030-nv4-res-262k.log:L488 | 로그 12개 · Ray 접힘 +3(접힌 사본의 노드 미관측) |
| 25 | `nccl:network` | `IB` | sub | runtime | output/multi/benchlog/sweep_q38fn030-nv4-res-262k/lite_engine_q38fn030-nv4-res-262k.log:L501 | 로그 13개 · Ray 접힘 +3(접힌 사본의 노드 미관측) |
| 26 | `NCCL_DMABUF_ENABLE` | `0` | sub | env-echo | output/multi/benchlog/sweep_q38fn030-nv4-res-262k/lite_engine_q38fn030-nv4-res-262k.log:L503 | 로그 13개 |
| 27 | `NCCL_CROSS_NIC` | `1` | sub | env-echo | output/multi/benchlog/sweep_q38fn030-nv4-res-262k/lite_engine_q38fn030-nv4-res-262k.log:L506 | 로그 13개 |
| 28 | `NCCL_IB_SPLIT_DATA_ON_QPS` | `0` | sub | env-echo | output/multi/benchlog/sweep_q38fn030-nv4-res-262k/lite_engine_q38fn030-nv4-res-262k.log:L560 | 로그 13개 |
| 29 | `NCCL_IB_GID_INDEX` | `3` | sub | env-echo | output/multi/benchlog/sweep_q38fn030-nv4-res-262k/lite_engine_q38fn030-nv4-res-262k.log:L565 | 로그 13개 |

- 값 주: 기계 치환(pii.substitution_table · 장치·호스트·주소는 자리표시 — 원문은 출처 줄)

- Ray 접힘: 출처 줄 끝의 `[repeated Nx across cluster]` — Ray 가 다른 프로세스의 같은 줄 N개를 한 줄로 접었다. 노드 칸은 **첫 줄**의 노드이고, 접힌 사본이 어느 노드의 것인지는 로그에 없다(관측 대상 밖).

- 로그 창(꼬리): `tail-800(lite_bench.sh@5d12e3a0a7bc L197)`(꼬리 창 출처 로그 1개 · 이 표 행 #14–29) — 이 캡처는 엔진 출력의 **마지막 N줄**이다(측정 도구의 `tail -N` 상한에 닿았다 · 생산자 evidence `log_window`). 창 앞에서 echo 한 줄은 그 로그에 없다 — 이 주의는 꼬리 창 출처 행에만 걸린다(머리 캡처 행은 아래).

- 로그 창(머리): `head-600(output/multi/benchlog/sweep_q38fn030-nv4-res-262k/lite_raw_q38fn030-nv4-res-262k.json engine_log_head_lines)`(머리 캡처 출처 로그 1개 · 이 표 행 #1–13) — 이 캡처는 엔진 출력의 **처음 N줄**이다(lite_bench.sh 머리 캡처 `head -n N` 상한에 닿았다 · 생산자 evidence `log_head_window`). 꼬리 창의 주의는 이 행에 걸리지 않는다 · 머리 N줄 뒤에서 echo 한 줄은 그 로그에 없다. 머리 캡처와 꼬리 창 사이(엔진 출력의 가운데)는 어느 캡처에도 없다 — 표에 없는 키 · 한 노드에만 있는 키는 그 사이였을 수 있다.
<!-- /FACT:measurement_env -->

> 이 절이 답하는 질문: 수신자가 이 zip 만으로 이 셀을 다시 띄우려면 어떤 순서로 무엇을 하고, 각 단계의 성공을 무엇으로 판정하며, 얼마나 기다려야 하는가?

**(a) 사전 준비.** 가중치는 양 노드의 같은 컨테이너 경로 `/app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4` 로 마운트된다(호스트 원천 = `<manifest.quant_model_path>` · compose 마운트 변수 `QUANT_MODEL_PATH` — 프로젝트 env 형상 `.env.template` 과 서브 전달 env 의 mount 무리). 이 셀은 PLE resident 라 **스테이징 파일이 없다** — 가중치는 NAS(CIFS)에서 직접 읽는다. 엔진은 auto-prefetch 를 끄고 읽는다 — 사유는 두 조건의 결합이다(이벤트 #26 · 엔진 로그): CIFS 가 인식된 네트워크 FS(NFS/Lustre)가 아니고, 체크포인트 123.57GiB 가 가용 RAM(메인 39.52GiB · 서브 47.20GiB)의 90% 를 넘는다. 메모리가 큰 노드에서는 같은 CIFS 라도 결과가 다를 수 있다. 필수 env 의 자리는 셀 env(트리플렛 `.env.q38fn030-nv4-res-262k`: 이미지 태그 · Dockerfile 선택자 · `VLLM_REF` · 컨테이너 이름) · `.env.cluster`(노드 주소 형상) · `.env.interconnect`(NCCL 형상)다. 측정 당시 인터커넥트 값의 정본은 위 '측정 실행의 env 관측' 표다 — 렌더 단계 명령은 **지금의** 렌더러로 형상을 다시 만들므로, 다시 렌더한 `.env.interconnect` 가 그 표(`NCCL_IB_DISABLE=0` · `NCCL_NET_PLUGIN=spcx` · `NCCL_IB_GID_INDEX=3` · `NCCL_IB_QPS_PER_CONNECTION=4` · `NCCL_CROSS_NIC=1` · `NCCL_IB_MERGE_NICS=1` · `NCCL_DMABUF_ENABLE=0` · `NCCL_IB_SPLIT_DATA_ON_QPS=0`)와 다르면 측정 당시 값에 맞춘다.

**(b) 빌드.** 양 노드에서 각자 짓는다(이미지 전송 ✗ · 같아야 하는 것은 digest 가 아니라 ABI). 선택자는 셀 env `BUILD_DOCKERFILE=Dockerfile.source-build`, build-arg 는 위 2단계 명령의 목록(`VLLM_REF=v0.30.0` · `TORCH_CUDA_ARCH=12.1a` · `RAY_VERSION=2.48.0` · `BUILD_JOBS=8` · 게이트 0 두 개)이다. 측정 이미지의 층 창은 36분 33초이나 캐시 재사용 층을 포함하므로 빌드 소요가 아니다(위 표). 계보에 적힌 소요는 0.30.0 첫 빌드 2차가 양 노드 45분(`testlog_26100109` §2 M7 — 이 측정 이미지가 아니라 그 회차의 이미지)이다. 빌드 성공 판정은 빌드 RUN 의 `[build] verified: vllm/nc/nvcc/ray` 와 이미지 실재다.

**(c) 기동 순서와 성공 판정.** 예산 선언(양 노드) → honored 확인 → master(Ray head + serve) → slave(Ray worker) → master 가 `ray status` 의 GPU 2 합류를 확인 → health 폴링이다(스모크 로그 L18~L30 순서). 성공은 health 200 + chat 추론 1회(`content_len=149 finish_reason=stop`)로 판정했다(스모크 로그 `SMOKE PASS` · `testlog_26101017` §1). 이 셀에는 발화를 확인할 패치가 없다 — 대신 엔진 로그의 형상 서명(`quantization=modelopt_mixed` · PLE `weight_device=cuda:0, pinned=False` · `EngramConfig(cpu_offload=False)`)이 의도한 형상으로 떴음을 보여 준다. 기동 총 소요는 스모크 로그 `READY ~1825s` 이고, 위 표의 serve 소요(≤ 31분 13초)는 예산 선언에서 첫 측정까지의 상한이다. 엔진 로그 기준으로 가중치 적재가 rank 별 857.5s · 889.8s(노드당 62.89GiB) 걸렸고, 그 뒤 `shm_broadcast` 60초 경고가 13회 · 12분 36초 이어지다가 KV 산정(537,040 토큰)과 엔진 초기화(836.93s)가 끝났다(이벤트 #27~#31) — 이 반복 경고는 무서워 보이지만 벽이 아니었다. 계보의 같은 셀 0.30.0 첫 회차는 READY ~1,835s 였다(`testlog_26100109` M8).

이 캠페인의 기동 시도는 위 표대로 **1회**다 — 선언 2026-10-10T03:19:17Z · renew 13회 · 닫힘 `budget_clear` 2026-10-10T04:07:56Z(이벤트 #22~#47)가 이 측정을 서빙한 기동이다. 같은 셀 이름으로 묶인 '앞선-1'(선언 2026-09-30T23:58:21Z · renew 16회 · 닫힘 `budget_clear` 2026-10-01T00:58:02Z · 이벤트 #1~#21)은 앞선 캠페인인 0.30.0 bump 회차의 스모크 · full bench 이며(`testlog_26100109` M8 · M9 · M10 에서 `--down` 으로 회수) 이 셀의 시도로 세지 않는다. 두 기동 모두 사살 · 트립 · 거부가 없다. 각 기동 초반의 `watchdog_highrate_hold`(이벤트 #3 · #25)는 메모리 감소 속도가 옛 규칙의 트립 조건을 넘었으나 보류된 행이다.

**(d) 호스트 메모리 안전 예산.** 선언값은 바닥 28,350MiB · arm 상한 25,278MiB 이고 산식 입력은 weights 63,266(ckpt 126,533 ÷ tp 2 · resident 라 PLE 미감산) · kv 8,192 · overhead 24,800(env `SMOKE_BUDGET_OVERHEAD_MIB` — 2.5 의 승계값) · mem_total 124,608 이다(이벤트 #22 · 스모크 로그 L14~L16). 이것은 선언이다 — 엔진 실측은 노드당 가중치 62.89GiB · 엔진 예약 합계 70.89GiB 이고 측정 중 시스템 RAM 은 메인 88.4GiB · 서브 85.6GiB 였다(03 §3.1 · 벤치 리포트). 스모크는 `READY_MAX=480`(5초 × 480회 ≈ 40분)로 불렀다(`testlog_26101017` §1).

**(e) 측정 명령.** 위 4단계 명령은 bench JSON 에서 재구성한 lite 레그(cold 1요청 · warm 3요청 · 512/128) 명령이지 실행 원문이 아니다. 동시성 스윕(GuideLLM 0.7.3 · 1024/256 · 레벨 1·2·4·8 · 16요청 + warmup 2 · 반복 3)의 호출과 도구가 정한 인자는 03 §3.4 의 도구 원문 발췌로 읽는다.

**(f) 엔진이 스스로 고른 런타임 경로.** 통신은 NCCL 2.30.7 · `nccl:network IB`(RoCE · `NCCL_NET_PLUGIN=spcx`)였고 TP · EP 그룹의 all-reduce 는 `PYNCCL` 을 골랐다(측정 엔진 로그 `Using ['PYNCCL'] all-reduce backends`). MoE 는 미설정(auto)에서 주 experts 가 `FLASHINFER_CUTLASS`(NvFp4), MTP 의 FP8 experts 가 `TRITON`(Fp8)으로 갔고, 후자는 GB10 용 튜닝 config 가 없어 기본 config 를 쓴다는 경고를 낸다(Q6). prefix caching 과 chunked prefill 은 엔진 config 에서 켜져 있었다(`enable_prefix_caching=True` · `enable_chunked_prefill=True` — yaml 에 없는 엔진 기본값). 무시해도 되는 경고: `Unknown vLLM environment variable detected: VLLM_PLE_MMAP`(compose 잔재 env · 0.30.0 이 무시) · `num_speculative_tokens > 1 … lower acceptance rate` · `max_num_scheduled_tokens is set to 2048`(MTP k=3 와 배치 상한의 조합 — 성능 경고이지 실패가 아니다).

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
| `observed` | {"build_ledger": {"compared": "dockerfile · build_args(− exempt) · patches(phase,file,script_sha256,result) · vllm git sha", "identity_exempt_build_args": ["BUILD_JOBS"], "main_identity_sha256": "a9a2460c5a39dbb4d4c33fc8f480f9952f5be976c97d62c355ebf52c067b3f85", "sub_identity_sha256": "92772678d2fb47e8851cf24054fd54bd4fe5cb2e904a623a128556ff58122c14", "verdict": "differ"}, "driver": {"main": "580.178.04", "sub": "580.178.04", "verdict": "equal"}, "torch": {"main": "2.13.0a0+9186a08b2c.nv26.7.59513937", "sub": "2.13.0a0+9186a08b2c.nv26.7.59513937", "verdict": "equal"}, "vllm_sha": {"main": "ced6857afa0ea7b2e3f0846a62e1394e90f15607", "sub": "ced6857afa0ea7b2e3f0846a62e1394e90f15607", "verdict": "equal"}} |
| `source` | output/multi/benchlog/attestation_q38fn030-nv4-res-262k.json |

> **attestation 범위** — config `q38fn030-nv4-res-262k` · phase serve · 작성 2026-10-10T03:50:09Z(output/multi/benchlog/attestation_q38fn030-nv4-res-262k.json 파일 mtime(파일시스템 관측 — 문서에 시각 칸이 없다 · 기록 필드 아님)) · 묶은 근거 cell-name · 파일 `output/multi/benchlog/attestation_q38fn030-nv4-res-262k.json` · 측정 창 대비 작성 2026-10-10T03:50:09Z < 측정 시작 2026-10-10T03:50:30Z — 측정 전(빌드·스모크)의 관측: **same config(q38fn030-nv4-res-262k) · serve — 측정을 서빙한 같은 기동의 스모크 직후 관측(판별: 스모크 로그 docs/simlog/26101012_벤치_knob_프리셋_E2E/m1_serve.log L33 SMOKE PASS → L34 attestation 작성 → L37 --keep-up · 원장 docs/logs/main/events/2026-10.jsonl:288 budget_declare(smoke-q38fn030-nv4-res-262k · 2026-10-10T03:19:17Z) 가 측정 끝 2026-10-10T04:07:13Z 까지 재선언 · 창 닫힘 없이 이어짐(main 원장 기록 범위 안) · 스모크 로그 budget_honored ts 2026-10-10T03:19:18Z = 그 선언의 원장 budget_honored · 측정 엔진 로그 APIServer pid 789 하나 · 측정 시작 2026-10-10T03:50:30Z)**
<!-- /FACT:sub_recipe -->

<!-- FACT:measurement_env_nodes -->
**노드별 측정 env**(01 §1.4 표를 키 × 노드로 접은 것 · `만 관측` = 다른 쪽 줄이 캡처 로그에 없다 · Ray 가 같은 줄을 접은 키(`[repeated Nx across cluster]`)와 머리·꼬리 캡처 로그(01 §1.4 로그 창)에서 한쪽만 남은 키는 다른 쪽 부재를 단언하지 않는다)

- 양 노드 같은 값 12개: `NCCL_CROSS_NIC` · `NCCL_DMABUF_ENABLE` · `NCCL_IB_DISABLE` · `NCCL_IB_GID_INDEX` · `NCCL_IB_HCA` · `NCCL_IB_MERGE_NICS` · `NCCL_IB_QPS_PER_CONNECTION` · `NCCL_IB_SPLIT_DATA_ON_QPS` · `NCCL_NET_PLUGIN` · `NCCL_SOCKET_IFNAME` · `nccl:network` · `nccl:version`

| 키 | main | sub | 노드 미특정 | 판정 |
|---|---|---|---|---|
| `nccl:net_ib_devices` | `[0]<nic:cluster>:1/RoCE [1]<nic:cluster>:1/RoCE [RO]; OOB <nic:cluster>:<node:main><0>` · `[RO]; OOB <nic:cluster>:<node:main><0>` | `[RO]; OOB <nic:cluster>:<node:sub><0>` | — | 다름 |
<!-- /FACT:measurement_env_nodes -->

> 이 절이 답하는 질문: 서브(Ray worker) 노드는 메인과 무엇이 다르고, 무엇을 반드시 같게 맞춰야 하는가?

**(1) 역할 차이.** master 는 Ray head 를 띄우고 트리플렛(yaml · 러너 · 셀 env)을 source 해 `vllm serve` 를 실행한다. slave 는 트리플렛을 받지 않는다 — 같은 이미지 · 같은 compose · 같은 `serve_runner.sh` 가 `NODE_ROLE=slave` 로 분기해 head 주소를 기다렸다가 Ray worker 로 합류(`--block`)하고, master 가 GPU 2 합류를 확인한 뒤 서빙을 시작한다. slave 의 `CONFIG_FILE` 은 `default` 라 모델 런타임 패치를 무장하지 않는다(이 셀은 master 쪽에도 런타임 패치가 없다).

**(2) 서브에 전달돼야 하는 env(무리별).** 이미지 정체성 build-arg(`IMAGE_TAG` · `BUILD_DOCKERFILE` · `VLLM_REF`) — 빠지면 slave 가 compose 기본 태그 · Dockerfile · ref 로 빌드해 다른 이미지가 된다. 빌드 튜닝(`BUILD_JOBS`) — 정체성은 아니지만 slave 만 기본 16 으로 컴파일하면 거기서 OOM 이 났던 이력이 있다. 마운트(`QUANT_MODEL_PATH` · `NAS_MODEL_PATH` · `TIKTOKEN_HOST_PATH` · `PLE_MMAP_HOST_PATH`) — 서브도 자기 몫의 가중치를 직접 올리므로 같은 마운트가 필요하고, `--env-file` 을 쓰면 프로젝트 `.env` 자동 로드가 꺼져 셸 env 로 주입해야 한다. 클러스터(`SLAVE_CONTAINER_NAME`) — 이름이 어긋나면 slave 쪽 협역 워치독 필터가 매칭 0 이 된다(1.5 사실 블록의 이유 칸).

**(3) 선행조건.** 가중치가 서브의 **같은 마운트 경로**에 실재해야 한다(메인 `.env` 의 mount 값이 서브로 전달된다). 이 셀은 resident 라 PLE 스테이징 선행조건은 해당하지 않는다. 양 노드 드라이버 계열이 같아야 하고(이 측정은 양 노드 580.178.04) 인터커넥트(RoCE · NCCL 형상)가 준비돼 있어야 한다. 서브에도 예산 선언이 나간다 — 이 기동에서 서브 원장 쪽으로 관측된 것은 `budget_honored`(2026-10-10T03:19:21Z)뿐이다(서브 원장은 2026-09-11 이후 미회수 · 1.4 이벤트 표 머리).

**(4) ABI 동일성.** attestation(작성 2026-10-10T03:50:09Z — 측정을 서빙한 같은 기동의 스모크 직후 관측)에서 vLLM SHA(`ced6857afa0ea7b2e3f0846a62e1394e90f15607`) · torch(`2.13.0a0+9186a08b2c.nv26.7.59513937`) · driver(580.178.04)는 양 노드 equal 이고, **빌드 원장 identity 는 differ** 다(main `a9a2460c…` · sub `92772678…` · `BUILD_JOBS` 는 비교에서 면제). wheel 메타의 빌드 날짜 접미도 main `d20261001` · sub `d20260930` 으로 다르다(0.4). attestation 의 노드별 원장이 차이를 기록한다 — build-arg 8개는 양 노드 같고 `requirements.txt` sha256 도 같지만, 메인(image `sha256:123c331f…`)은 패치 슬롯이 비어 있고(.gitkeep 뿐) 인라인 `strip-hoist` skip 1건만 있는 반면, 서브(image `sha256:055ce791…`)는 pre 50 · 55 skipped · post 10-deepgemm · 20-triton-kernels · 30-mxfp4-triton-sm121 · 40-humming-nvml-gb10 applied 이고 pip freeze sha256 도 다르다(`output/multi/benchlog/attestation_q38fn030-nv4-res-262k.json` nodes.*.build_ledger). 즉 이 측정은 post 패치가 한쪽 노드에만 구워진 클러스터에서 났다 — 측정에 준 영향은 미결이다(Q4). attestation 의 `blocking` 은 false 라 서빙은 진행됐다.

**(5) 측정 당시 인터커넥트 실효값.** 위 '노드별 측정 env' 대로 12개 키가 양 노드 같은 값이다 — `NCCL_IB_DISABLE=0`(main `lite_engine_head_…log:L135` · sub `lite_engine_…log:L473`) · `NCCL_NET_PLUGIN=spcx`(L130 · L468) · `NCCL_IB_GID_INDEX=3`(main `lite_engine_…log:L100` · sub L565) · `NCCL_IB_QPS_PER_CONNECTION=4`(L156 · L488) · `NCCL_CROSS_NIC=1`(L185 · L506) · `NCCL_IB_MERGE_NICS=1`(L140 · L476) · `NCCL_DMABUF_ENABLE=0`(L182 · L503) · `NCCL_IB_SPLIT_DATA_ON_QPS=0`(main `lite_engine_…log:L49` · sub L560) · `NCCL_IB_HCA` · `NCCL_SOCKET_IFNAME`(장치 이름은 `<nic:cluster>` 자리표시) · `nccl:version 2.30.7+cuda13.3` · `nccl:network IB`. `nccl:net_ib_devices` 만 "다름" 인데, main 은 머리 캡처(처음 600줄)에 RoCE 장치 두 개가 나열된 줄과 OOB 만 있는 줄을 모두 갖고 sub 는 꼬리 캡처(마지막 800줄)의 OOB 줄만 갖는다 — 캡처 창이 달라 생긴 차이일 수 있으므로 sub 의 RoCE 장치가 없었다고 읽지 않는다. sub 행의 다수는 Ray 가 접은 줄(`repeated Nx across cluster`)이라 접힌 사본의 노드는 로그가 말하지 않는다. 지금의 렌더러가 내는 `.env.interconnect` 와 이 값이 다르면 이 표가 측정 당시다.

## 1.6 재현 절차 — 기동 전 준비(사람이 할 일)
> 1.4 의 명령을 치기 **전에** 사람이 손으로 해야 하는 것이다 — 가중치 획득 · 스테이징 생성 · 네트워크가 필요한 단계. 에이전트가 대신 할 수 없는 자리(다운로드 승인 · 물리 경로 · 외부 계정)가 여기 모인다.

> 이 절이 답하는 질문: 이 셀을 기동하기 전에 사람이 준비해야 하는 것은 무엇이며, 각각을 어떤 명령으로 만들고, 그 절차의 출처는 어디인가?

### 가중치 획득 — `nvidia/Qwen3.8-Flash-Next-NVFP4`

- 체크포인트: HF repo `nvidia/Qwen3.8-Flash-Next-NVFP4` · revision = 00 §0.2 표의 40자 git HEAD(`fc694b54fb0174e0913e6adf86691ef85a4ead47` — 측정 시점 checkout 과 같다고 reflog 가 말한다 · 작업트리 일치는 미검증).
- 획득 모드: 이 계보는 관리 NAS 상재본을 썼다 — "모델 획득: `model_source: managed` · NAS 상재 → 다운로드 불요(TERNARY 게이트 해당 없음)"(`plan_26090918` §2.1). 컨테이너 경로 `/app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4` ← 호스트 `<manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4`.
- 크기: safetensors 헤더 합 123.5GiB(`plan_26090918` §2.1) · 엔진 로그의 체크포인트 크기 123.57GiB. du 로 재지 않는다(약 2배 과대보고).
- 노드마다: 양 노드가 각자 자기 TP 몫을 같은 경로에서 읽으므로 두 노드 모두에서 같은 경로로 보여야 한다(공유 NAS 마운트 또는 노드별 사본).
- 받는 명령(NAS 에 없을 때 · 사람의 다운로드 승인 뒤): `huggingface-cli download nvidia/Qwen3.8-Flash-Next-NVFP4 --revision fc694b54fb0174e0913e6adf86691ef85a4ead47 --local-dir <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4` — 추론 — 실행 검증 ✗(이 계보는 다운로드를 하지 않았다).

### PLE · 기타 스테이징 — 해당 없음

- 해당 없음 — 이 셀은 PLE resident 다(측정 엔진 로그 `Initialized PLE embedding … weight_device=cuda:0, pinned=False` · 0.30.0 에서 mmap 패치 62 는 parked — `plan_26100107` §4 D3). mmap 셀이 쓰던 로컬 NVMe 스테이징(53.7GB × 양 노드 · `testlog_26091001` §4)은 이 셀에 필요 없다.
- 다만 compose 와 서브 전달 env 에는 `PLE_MMAP_HOST_PATH` 마운트 변수가 남아 있다(1.5). resident 셀에서 그 경로가 비어 있어도 되는지는 이 계보가 따로 확인하지 않았다. 메인 쪽은 측정 전에 렌더된 프로젝트 `.env`(비추적 · 출처 주석 `PLE_MMAP_HOST_PATH 출처: manifest.ple_mmap_host_path`)가 manifest 값으로 채웠고, 측정 엔진 머리 로그에 `VLLM_PLE_MMAP` · `VLLM_PLE_MMAP_DIR` 미지 env 경고가 남았다(`lite_engine_head_…log` L63-64). 서브 쪽 실제 값은 관측 밖이다(서브 레시피의 노드별 값은 자리표시 형상일 뿐이다).

### 네트워크가 필요한 단계와 오프라인 대체

- 베이스 이미지 pull: `nvcr.io/nvidia/pytorch:26.07-py3`(Dockerfile `FROM`) — 미리 pull 해 두거나 사설 레지스트리 미러.
- vLLM 소스: `git clone` `https://github.com/vllm-project/vllm.git` @ `v0.30.0`(build-arg `VLLM_REPO` · `VLLM_REF`) — 미리 클론해 두고 SHA `ced6857afa0ea7b2e3f0846a62e1394e90f15607` 로 고정.
- apt: `ccache` · `libdw-dev`(빌드 로그가 Ubuntu ports 저장소에서 받는다 — `docs/simlog/26100107_v030_e2e/m7c_mn_build_master.log`) — 사설 apt 미러.
- pip: `requirements.txt`(wheel METADATA 핀) · `ray==2.48.0` — wheelhouse 를 미리 받아 두는 대체가 가능하다(추론 — 실행 검증 ✗).
- 측정 도구: GuideLLM 컨테이너 이미지(`run_bench.sh` 가 `resolve_bench_tool.py` 로 이미지 핀을 해소한다 — 03 §3.4 도구 원문) — 미리 pull. GuideLLM 실행 자체는 `HF_HUB_OFFLINE=1` · `TRANSFORMERS_OFFLINE=1` 로 오프라인이다(같은 원문).
- 가중치 다운로드: 위 첫 항목 — 무인 다운로드를 하지 않는다(사람 승인 게이트).
