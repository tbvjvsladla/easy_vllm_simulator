# 01 · 산출물 — 무엇이 실제로 쓰였고, 어떻게 다시 띄우는가

## 1.1 적용 판정 표
> 기계가 적용 증거(빌드 원장 · 원장이 없으면 라벨된 재구성) × 파일 × 선언을 대사한 결과다. `artifacts/` 에는 **적용된 것만** 실렸다 — 빌드 때 스스로 skip 된 패치와 쓰이지 않은 평면의 파일은 표에만 남고 싣지 않는다.

<!-- FACT:slots -->
| 슬롯 | 실림 | 파일 | 신호 | 적용 증거 | 사유 |
|---|---|---|---|---|---|
| `build_patch_post` | 안 실림 | — | 1-signal | none:no-build-ledger | 적용 판정 없음 — 원장 부재 — 라벨된 재구성: applied 0 · skip 판정 = docker-history+gate(기계 파생) |
| `build_patch_pre` | 안 실림 | — | 1-signal | none:no-build-ledger | 적용 판정 없음 — 원장 부재 — 라벨된 재구성: applied 0 · skip 판정 = docker-history+gate(기계 파생) |
| `build_recipe` | 실림 | `artifacts/build_recipe/Dockerfile` · `artifacts/build_recipe/requirements.txt` | 2-signal | file:image-history:docker history build-arg 관측(VLLM_VERSION=0.19.1 · VLLM_REF 없음 → wheel Dockerfile) — 셀 env 선택자 없음 · output/single/docker-compose.yaml 리터럴 build.dockerfile: Dockerfile.source-build(선택자 변수 없음 — compose build 는 늘 Dockerfile.source-build 를 짓는다 · 이 이미지를 지은 증거 아님) · ⚠ compose 리터럴 Dockerfile.source-build ≠ 관측 Dockerfile(이 이미지는 compose build 로 지어지지 않았다) | (a) NGC `pytorch:26.01-py3` 위에 GitHub Release 의 vLLM wheel 을 `--no-deps` 로 설치한다(ARG 기본값은 0.18.0 이지만 측정 이미지는 build-arg `VLLM_VERSION=0.19.1` 로 지었다 — 1.1 재구성 입력 표). 그 뒤 조건부 HAS_OPAQUE_TYPE 패치 블록 · requirements 설치 · 분산 서빙용 netcat · `ray==2.48.0` 설치가 이어진다. ray · netcat 은 TP=2 Ray 경로용 공유 이미지 스탠자이며 이 single 셀은 쓰지 않는다 — 이 모델의 요구가 아니라 공유 이미지의 일부다. (b) 왜 wheel 인가 — 0.19.1 은 torch 2.10.0 을 핀하고 NGC 26.01 의… |
| `compose` | 실림 | `artifacts/compose/.env.template` · `artifacts/compose/docker-compose.yaml` | 2-signal | file:output/single/docker-compose.yaml | (a) single 서빙 경로다: `vllm-serve` 서비스가 셀 env(`envs/.env.${CONFIG_FILE}`)를 읽고 `/app/configs/<셀>.sh` 를 실행한다. 볼륨은 NAS 모델 루트(`/app/models` ro) · quant 모델 루트 · tiktoken 평탄 폴더(`/encodings` · gpt-oss 용) · configs · JIT 캐시 두 개다. `.env.template` 은 세 마운트 변수(NAS_MODEL_PATH · QUANT_MODEL_PATH · TIKTOKEN_HOST_PATH)의 형상이며 값의 정본은 manifest 동명 필드다. (b) 서빙 진입 경로 자체라 빼면 기동이 없다. 주의: compose 의 이미지 기본값은 다른 vLLM 버전의 sour… |
| `fork_pin` | 안 실림 | — | 1-signal | none:.claude/policies/arch_variant_ledger.json | 셀 env 에 VARIANT 줄 없음 = stock(workflow.md §변종 좌표의 거처)(기계 파생) |
| `runtime_patch` | 안 실림 | — | 1-signal | none:output/single/configs/gptoss120b-0191-c0_patch.py | output/single/configs/gptoss120b-0191-c0_patch.py 없음 — arm_patch.sh 는 no-op(런타임 패치 불해당)(기계 파생) |
| `triplet` | 실림 | `artifacts/triplet/.env.gptoss120b-0191-c0` · `artifacts/triplet/gptoss120b-0191-c0.sh` · `artifacts/triplet/gptoss120b-0191-c0.yaml` | 3-signal | file:output/single/configs/gptoss120b-0191-c0.yaml | (a) 서빙 노브 7개(1.3) · 러너(tiktoken env · arm_patch 소스 · `vllm serve --config`) · 셀 env(포트 8081 · `IMAGE_TAG` · served-model-name). (b) 이 셀의 정의 그 자체다. 러너에는 어텐션 백엔드 export 가 없다 — 옛 FLASHINFER export 는 효과가 없었다(02 M3). reasoning parser 플래그도 러너에 없지만 엔진은 gpt-oss 에 대해 tool use 를 항상 켠다고 경고했다(엔진 로그 `For gpt-oss, we ignore --enable-auto-tool-choice and always enable tool use.`). (c) 이번 측정이 이 트리플렛으로 돌았다(1.1 스윕… |

**레시피 경고 · 실린 리비전**(기계 관측 — 수신자 빌드가 여기서 갈라질 수 있다)

- ⚠ `build_recipe` `output/single/Dockerfile` — 실린 바이트(worktree)가 측정 이미지 docker history 의 RUN·COPY 와 전수 일치하지 않는다(5/8) · 이미지 빌드 뒤 바뀐 레시피일 수 있다 — 마지막 커밋이 이미지 Created 뒤(이미지 Created 2026-09-03T22:22:09Z · 마지막 커밋 2026-09-05T11:42:30Z)
- `build_recipe` `output/single/Dockerfile` 실린 리비전: 리비전 미특정 · 선택 방법 `worktree(어느 개정도 이미지 history 와 더 맞지 않는다 · 작업트리 5/8 — recipe_vs_image 경고 유지)` · docker history 대조 5/8 · 실린 바이트 `worktree` · 쓰인 바이트임을 관측으로 확인 아니오
- `build_recipe` `output/single/requirements.txt` 실린 리비전: 커밋 `330fc68cbba8` · 선택 방법 `git log -1 --before=2026-09-03T22:22:09Z -- output/single/requirements.txt · image-probe 바이트 일치(작업트리는 다르다)` · docker history 대조 없음 · 실린 바이트 `git:330fc68cbba81f6e747d4531770263056c7fd3da` · 쓰인 바이트임을 관측으로 확인 예
- `compose` `output/single/docker-compose.yaml` 실린 리비전: 커밋 `f9b4dae0add0` · 선택 방법 `worktree(git log -1 --before=2026-10-10T07:45:58Z -- output/single/docker-compose.yaml → f9b4dae0add0 · 작업트리 mtime 2026-10-10T08:55:08Z > 측정(sweep_index.generated_utc)이지만 바이트가 그 개정과 같다 · 측정 뒤 커밋 0 · 작업트리 = HEAD)` · docker history 대조 없음 · 실린 바이트 `worktree` · 근거 `git-rev-before-measurement` · 쓰인 바이트임을 관측으로 확인 아니오

**싣지 않은 파일**(슬롯별 사유 — 빌드 때 skip 된 패치는 아래 적용 집합 표에 있다)

- `build_patch_post` known-non-patch 1건: `.gitkeep`
- `build_patch_pre` known-non-patch 1건: `.gitkeep`
- `compose` `arm_patch.sh` — `not-armed(output/single/configs/gptoss120b-0191-c0_patch.py 없음 — arm_patch.sh 는 no-op)`

**파일별 검증 · 관련성**(`generated-unverified` = 실행 검증되지 않은 생성물 — 파일 첫 줄 경고 · PAYLOAD 파일 기록과 같은 표시 · 관련성 = ① 패치 선언 대상 × 모델 config ② 엔진 로그 발화 서명 — 둘 다 관측일 때만 required/inactive-inferred)

| 슬롯 | 파일 | 검증 | 검증 근거 | 관련성 | 관련성 근거 |
|---|---|---|---|---|---|
| `build_recipe` | `artifacts/build_recipe/Dockerfile` | `generated-unverified` · renderer | 실린 Dockerfile(worktree)이 측정 이미지 docker history RUN·COPY 와 5/8 만 일치 — 이 이미지를 지은 바이트임을 관측으로 확인하지 못했다(_select_dockerfile · worktree(어느 개정도 이미지 history 와 더 맞지 않는다 · 작업트리 5/8 — recipe_vs_image 경고 유지)) | `unknown` | 이 셀은 이 경로로 실행되지 않았다(generated-unverified · 생성 renderer) — 실행 기록이 없어 필요 여부를 판정하지 않는다(참고 형상) |
| `build_recipe` | `artifacts/build_recipe/requirements.txt` | `verified` · cell-run | 쓰인 레시피(image-history:docker history build-arg 관측(VLLM_VERSION=0.19.1 · VLLM_REF 없음 → wheel Dockerfile) — 셀 env 선택자 없음 · output/single/docker-compose.yaml 리터럴 build.dockerfile: Dockerfile.source-build(선택자 변수 없음 — compose build 는 늘 Dockerfile.source-build 를 짓는다 · 이 이미지를 지은 증거 아님) · ⚠ compose 리터럴 Dockerfile.source-build ≠ 관측 Dockerfile(이 이미지는 compose build 로 지어지지 않았다)) — 개정 선택 slots.build_recipe.evidence.selected_revisions | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/.env.template` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/docker-compose.yaml` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/.env.gptoss120b-0191-c0` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/gptoss120b-0191-c0.sh` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/gptoss120b-0191-c0.yaml` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
<!-- /FACT:slots -->

<!-- FACT:applied_set -->
**적용 집합** — 상태 `unobservable` · 출처 `no-build-ledger` · 원장 없음: 아래 결과는 **라벨된 재구성**이다(관측 아님)

_빌드 패치 없음 — 이 이미지에 빌드 패치 슬롯 입력이 없다._

**원장 탐침**(순위: attestation → 메인 로컬 이미지 → 재구성)

| 순위 | 결과 | 대상 |
|---|---|---|
| attestation | absent | output/single/benchlog/attestation_gptoss120b-0191-c0.json(부재) |
| local-image | no-ledger(이미지에 /opt/easy-vllm/build_ledger.json 없음 — docker cp) | sha256:026e9d87403db08a6c4d4ced91ac49f3efcafaf78fdc206dc66048d8914abd52 |
| reconstruction | done | — |
| image-probe | done | sha256:026e9d87403db08a6c4d4ced91ac49f3efcafaf78fdc206dc66048d8914abd52 |

**재구성 입력** — `docker-history+gate-rule · fail-loud-rule · image-probe`(docker history build-arg × 패치 자기게이트 규칙)

| build-arg | 값 |
|---|---|
| `CUDA_VERSION` | 130 |
| `RAY_VERSION` | 2.48.0 |
| `VLLM_VERSION` | 0.19.1 |
<!-- /FACT:applied_set -->

## 1.2 슬롯별 적용 사유
> 이 절이 답하는 질문: 실린 산출물 하나하나는 무엇을 고치며, 왜 이 셀에 필요했고, 이 셀에서 실제로 발화했으며, 언제 불필요해지는가?

이 셀에는 빌드 패치 · 런타임 패치 · 포크 핀이 없다 — stock vLLM 0.19.1 wheel 이 그대로 서빙했다. 실린 것은 재현 경로 입력(빌드 레시피 · compose · 트리플렛)뿐이고, 1.1 의 적용 판정은 빌드 원장이 없는 이미지라 `docker history` build-arg 와 이미지 탐침으로 한 **라벨된 재구성**이다.

### build_recipe — `Dockerfile`

(a) NGC `pytorch:26.01-py3` 위에 GitHub Release 의 vLLM wheel 을 `--no-deps` 로 설치한다(ARG 기본값은 0.18.0 이지만 측정 이미지는 build-arg `VLLM_VERSION=0.19.1` 로 지었다 — 1.1 재구성 입력 표). 그 뒤 조건부 HAS_OPAQUE_TYPE 패치 블록 · requirements 설치 · 분산 서빙용 netcat · `ray==2.48.0` 설치가 이어진다. ray · netcat 은 TP=2 Ray 경로용 공유 이미지 스탠자이며 이 single 셀은 쓰지 않는다 — 이 모델의 요구가 아니라 공유 이미지의 일부다. (b) 왜 wheel 인가 — 0.19.1 은 torch 2.10.0 을 핀하고 NGC 26.01 의 torch 가 2.10 계열이라 wheel 의 `_C` ABI 가 맞는다(testlog_26090117_gptoss120b_단일노드_FullE2E_판정 §판정 S1 행). 빼면 다시 나오는 W 는 없다(이 계보에 빌드 벽이 없었다). (c) HAS_OPAQUE_TYPE 블록은 torch 의 `register_opaque_type` 이 `hoist` 인자를 받지 않을 때만 파일을 고친다. 이 이미지에서 그 블록이 무엇을 출력했는지는 빌드 로그가 남지 않아 미관측이다. 스크립트 원문상 조건이 맞지 않으면 빌드를 멈추지 않고 skip 메시지를 찍고 지나간다:

> [원문] Dockerfile §L40-42
>     print("[patch] HAS_OPAQUE_TYPE disabled (NGC torch hoist mismatch)")
> else:
>     print("[patch] skipped (torch supports hoist / no opaque_object / file missing)")

(d) 이 레시피는 torch 핀이 2.11 이상인 vLLM(0.20.0+)으로 가면 wheel 트랙이 아니라 source-build 트랙으로 바뀐다(같은 testlog §판정 S1 행의 경계 실측). ⚠ 1.1 경고대로 실린 Dockerfile(작업트리)은 측정 이미지의 `docker history` RUN·COPY 와 5/8 만 일치한다 — 이미지가 지어진 뒤 레시피가 바뀌었을 수 있다. 수신자가 이 파일로 지으면 측정 이미지와 층이 다를 수 있으니 1.4 의 build-arg 와 함께 쓰고, 지은 뒤 `docker history` 를 대조하라.

### build_recipe — `requirements.txt`

(a) vLLM 런타임 의존 핀 목록이다(예: `flashinfer-python==0.6.6`). 이미지 안 사본과 바이트가 같은 개정(330fc68)이 실렸다(1.1 실린 리비전 — 관측 확인 "예"). 머리 주석은 출처를 0.18.0 wheel 의 METADATA 로 적는다:

> [원문] requirements.txt §L1-3
> # vLLM 런타임 의존성 — wheel 의 Requires-Dist(METADATA) 기준(권위 소스).
> # source: https://github.com/vllm-project/vllm/releases/download/v0.18.0/vllm-0.18.0+cu130-cp38-abi3-manylinux_2_35_aarch64.whl
> # base 제공분(torch/torchvision/torchaudio/setuptools)은 제외(--no-deps 보호).

(b) 즉 측정 이미지는 0.19.1 wheel 을 `--no-deps` 로 깔고 의존은 0.18.0 기준 핀으로 채웠다. 이 셀은 그 상태로 서빙 · 측정이 성립했다. 0.19.1 의 Requires-Dist 와 다른 핀이 있는지는 대조되지 않았다(02 Q7). (c) 빌드 시점 성립분이라 런타임 발화 개념이 없다. (d) 재렌더 때 해소기가 대상 버전의 METADATA 로 다시 만들면 이 사본은 낡는다 — 수신자는 자기 대상 버전으로 다시 렌더하는 것이 기본이다.

### compose — `docker-compose.yaml` · `.env.template`

(a) single 서빙 경로다: `vllm-serve` 서비스가 셀 env(`envs/.env.${CONFIG_FILE}`)를 읽고 `/app/configs/<셀>.sh` 를 실행한다. 볼륨은 NAS 모델 루트(`/app/models` ro) · quant 모델 루트 · tiktoken 평탄 폴더(`/encodings` · gpt-oss 용) · configs · JIT 캐시 두 개다. `.env.template` 은 세 마운트 변수(NAS_MODEL_PATH · QUANT_MODEL_PATH · TIKTOKEN_HOST_PATH)의 형상이며 값의 정본은 manifest 동명 필드다. (b) 서빙 진입 경로 자체라 빼면 기동이 없다. 주의: compose 의 이미지 기본값은 다른 vLLM 버전의 source-build 태그라, 셀 env 의 `IMAGE_TAG` 가 비면 조용히 다른 이미지를 띄운다(02 §2.6). (c) 이 셀은 `single_serve_up.sh` 를 거쳐 기동했다(1.4). `arm_patch.sh` 는 셀 런타임 패치 파일이 없어 no-op 이라 싣지 않았다. (d) 토폴로지 경로가 바뀌지 않는 한 폐기 대상이 아니다.

### triplet — `gptoss120b-0191-c0.yaml` · `.sh` · `.env.gptoss120b-0191-c0`

(a) 서빙 노브 7개(1.3) · 러너(tiktoken env · arm_patch 소스 · `vllm serve --config`) · 셀 env(포트 8081 · `IMAGE_TAG` · served-model-name). (b) 이 셀의 정의 그 자체다. 러너에는 어텐션 백엔드 export 가 없다 — 옛 FLASHINFER export 는 효과가 없었다(02 M3). reasoning parser 플래그도 러너에 없지만 엔진은 gpt-oss 에 대해 tool use 를 항상 켠다고 경고했다(엔진 로그 `For gpt-oss, we ignore --enable-auto-tool-choice and always enable tool use.`). (c) 이번 측정이 이 트리플렛으로 돌았다(1.1 스윕 색인). (d) 모델별 서빙전략은 carry-forward 하지 않는다 — 다른 HW 에서는 1.3 의 arch-scaled 값부터 다시 잰다.

### 싣지 않은 슬롯

- `build_patch_pre` · `build_patch_post` — 입력 없음(`.gitkeep` 뿐) · 원장 부재라 재구성 판정(applied 0). 이 모델은 sm_121 용 빌드 패치 없이 섰다.
- `runtime_patch` — 셀 `_patch.py` 없음(`arm_patch.sh` no-op).
- `fork_pin` — 셀 env 에 `VARIANT` 줄 없음 = stock.

## 1.3 값의 지위표
> 서빙 설정의 노브 **전부**와 기계가 모은 후보 지위다(lockset 출처 · yaml 주석 · 엔진 기본값). 최종 지위는 아래 `kind: value-status` 블록이 정한다 — 표의 노브마다 블록이 정확히 하나 있어야 린터를 통과한다.

<!-- FACT:value_status -->
서빙 노브 7개 — 노브마다 `kind: value-status` 블록이 **정확히 1개** 있어야 한다(노브·값은 이 표의 글자 그대로 · 값 칸의 `\|` 는 `|` 로 적는다).

| 노브 | 값 | 후보 지위 | 후보 근거 | yaml 주석 | 출처 |
|---|---|---|---|---|---|
| `gpu-memory-utilization` | 0.9 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `max-model-len` | 131072 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `max-num-seqs` | 16 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `kv-cache-memory-bytes` | 32212254720 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `kv-cache-dtype` | fp8 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `quantization` | mxfp4 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `max-num-batched-tokens` | 8192 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
<!-- /FACT:value_status -->

> 이 절이 답하는 질문: 각 서빙 노브의 값은 이 셀에서 어떤 지위인가 — 조정됐나, 승계됐나, 음성대조로 일부러 둔 것인가, 엔진 기본값인가, 필요조건인가?

```hint-event
id: V1
kind: value-status
노브: gpu-memory-utilization
값: 0.9
지위: inherited
근거: 손레버 — 2026-09-01 사용자 선택 · 스윕 0회 · 0.95 는 예산 선언 거부로 막힘(R3) · 엔진은 KV 클램프가 있으면 gmu 를 KV 산정에 쓰지 않는다고 로그에 적었다
출처: [devlog_26090117_15_53_gptoss120b_단일노드_E2E_서사 §결정과 근거, gptoss120b-0191-c0.yaml]
```

```hint-event
id: V2
kind: value-status
노브: max-model-len
값: 131072
지위: inherited
근거: 모델 max_position_embeddings 와 같은 값으로 고정(광의탐색의 통제변인 · 축 아님) · 2026-09-01 사용자 선택 131K 승계 · 스윕 0회
출처: [plan_26090419_광의탐색_캠페인안_gptoss120b_0191 §1. 통제변인과 기준선, gptoss120b-0191-c0.yaml]
```

```hint-event
id: V3
kind: value-status
노브: max-num-seqs
값: 16
지위: inherited
근거: 2026-09-01 인증서 기준선 값을 광의탐색이 공통 노브로 승계 · 조정 기록 없음 · 엔진 보고 전장(131072) 동시성 12.54x 보다 크다 — 짧은 요청 부하 전제
출처: [plan_26090419_광의탐색_캠페인안_gptoss120b_0191 §8.6, gptoss120b-0191-c0.yaml]
```

```hint-event
id: V4
kind: value-status
노브: kv-cache-memory-bytes
값: 32212254720
지위: declared-requirement
근거: 키의 존재가 필요조건 — 없으면 예산 선언이 생략돼 정상 로드가 워치독에 사살됐다(W2). 값 30 GiB 자체는 2026-09-01 레시피에서 승계했고 조정된 적 없다(산식 미기록)
출처: [plan_26090419_광의탐색_캠페인안_gptoss120b_0191 §8.4]
```

```hint-event
id: V5
kind: value-status
노브: kv-cache-dtype
값: fp8
지위: tuned
근거: fp16(2026-09-01 · 436,896 토큰 · 34.26 t/s) 대 fp8(873,808 토큰 · 34.55 t/s) · 2026-09-04 auto(C2 · 32.71) 대 fp8(C0 · 32.83) · fp8_e5m2(C1) 로드 실패 — 값을 바꿔 잰 기록이 있다
출처: [testlog_26090117_gptoss120b_단일노드_FullE2E_판정 §참조 그라운드 재탐색의 소득과 한계, sweep_map_26090504_gptoss120b_0191_single_완주 §2.]
```

```hint-event
id: V6
kind: value-status
노브: quantization
값: mxfp4
지위: inherited
근거: 체크포인트 quantization_config quant_method 와 같은 값(warm-start hint 의 모델 축에서 승계) · 다른 값을 시도한 기록 없음
출처: [plan_26090112_59_49_메인_단일노드_gptoss120b_FullE2E §3. warm-start 근거, gptoss120b-0191-c0.yaml]
```

```hint-event
id: V7
kind: value-status
노브: max-num-batched-tokens
값: 8192
지위: inherited
근거: 2026-09-01 기준선 값을 광의탐색이 공통 노브로 승계 · 조정 기록 없음(누락한 C0 1차는 R9 로 폐기)
출처: [plan_26090419_광의탐색_캠페인안_gptoss120b_0191 §8.6, gptoss120b-0191-c0.yaml]
```

복사하면 위험한 값은 셋이다. kv-cache-memory-bytes(V4)는 **키가 있어야** 한다는 것만 확인된 필요조건이고 30 GiB 라는 값은 이 HW(통합메모리 124610MiB · 상주 가중치 65.96 GiB)에서 승계된 것이다 — 메모리가 다른 HW 에서는 엔진 로그의 KV 토큰 수와 호스트 바닥(총량 − 가중치 − KV − overhead)으로 다시 정한다. gmu 0.9(V1)는 최적값이 아니라 손레버이며 이 셀에서는 KV 산정에 쓰이지도 않는다. max-num-seqs 16(V3)은 전장 컨텍스트 동시성(12.54x)보다 커서 긴 요청이 몰리면 대기가 생긴다 — 측정 부하는 1024/256 토큰이었다. 조정된 값은 KV dtype(V5) 하나뿐이다.

## 1.4 재현 절차
> 셀 실행 기록에서 기계가 파생한 명령 순서와 단계별 소요다. 소요가 `미관측` 인 단계는 시각 기록이 없다는 뜻이다(0 이 아니다). 그 아래는 블랙박스 이벤트 원장에서 이 셀의 행을 기계가 뽑아 기동 시도로 묶은 것이고, 마지막은 측정한 실행의 엔진 로그가 echo 한 env 다(측정 당시 값의 정본).

<!-- FACT:reproduce -->
| 순서 | 단계 | 성공 판정 | 소요 | 관측 구간(UTC) | 소요 출처 | 명령 출처 |
|---|---|---|---|---|---|---|
| 1 | build | 빌드 OK · 이미지 실재 | 층 창 58분 8초(docker history 첫 층 → 끝 층 CreatedAt · 캐시 재사용 층 포함 — 빌드 소요 아님) | 2026-09-03T21:24:01Z → 2026-09-03T22:22:09Z (layer-window) | docker history --no-trunc --format '{{json .}}' sha256:026e9d87403db08a6c4d4ced91ac49f3efcafaf78fdc206dc66048d8914abd52 · 우리 Dockerfile 층 11개의 CreatedAt 첫 층 → 끝 층(캐시 재사용 층은 이전 빌드 시각 — 빌드 소요 아님) | reconstructed(docker-history build-args + compose build) |
| 2 | serve | health 200 + 추론 1회(스모크 판정) | ≤ 10분 34초(상한) | 2026-10-10T07:15:17Z → 2026-10-10T07:25:51Z (upper) | docs/logs/main/events/2026-10.jsonl budget_declare(label serve-gptoss120b-0191-c0) → 첫 측정 시각(output/single/benchlog/sweep_gptoss120b-0191-c0/lite_raw_gptoss120b-0191-c0.json measured_utc(호스트 date -u · lite_bench.sh 가 cold 레그 직전에 찍는 측정 시작)) | derived(스모크 사용법) |
| 3 | bench | 리포트 발행(+PASS 면 인증서) | 20분 7초 | 2026-10-10T07:25:51Z → 2026-10-10T07:45:58Z (exact) | output/single/benchlog/sweep_gptoss120b-0191-c0/lite_raw_gptoss120b-0191-c0.json measured_utc(호스트 date -u · lite_bench.sh 가 cold 레그 직전에 찍는 측정 시작) → sweep_index.generated_utc(호스트 UTC · 두 끝 같은 시계) | reconstructed(bench json) |

**빌드 층 시각**(관측 · 소요가 아니라 층 창 — 캐시 재사용 층은 이전 빌드의 시각을 가진다)

- `build` 이미지 층 CreatedAt(docker history): 2026-09-03T21:24:01Z → 2026-09-03T22:22:09Z · 층 11개 · 출처 `docker history --no-trunc --format '{{json .}}' sha256:026e9d87403db08a6c4d4ced91ac49f3efcafaf78fdc206dc66048d8914abd52` — 우리 Dockerfile 층의 CreatedAt 범위 — 캐시 재사용 층은 이전 빌드 시각을 가진다(소요 아님)

**단계별 명령**(기계 파생 — `명령 출처` 가 `derived(…)` 면 사용법에서 파생 · `reconstructed(…)` 면 괄호 안 관측(docker history build-arg · compose build · bench JSON 필드)에서 재구성한 명령이다. 어느 쪽도 실행 원문(로그에 남은 줄)은 아니다)

**1. build**

```sh
# 실린 artifacts/build_recipe/Dockerfile(실린 개정: worktree — 이미지를 지은 바이트임을 관측으로 확인 못함(docker history RUN·COPY 5/8))를 output/single/Dockerfile 자리에 두고 컨텍스트에서 짓는다.
docker build -f output/single/Dockerfile \
  --build-arg CUDA_VERSION=130 \
  --build-arg RAY_VERSION=2.48.0 \
  --build-arg VLLM_VERSION=0.19.1 \
  -t easy-vllm:0.19.1-cu130-aarch64-wheel \
  output/single
# compose build 는 동등 경로가 아니다 — output/single/docker-compose.yaml build.dockerfile 리터럴이 Dockerfile.source-build 를 짓는다(이 이미지의 Dockerfile Dockerfile 아님) · 위 docker build 명령만 쓴다.
```

**2. serve**

```sh
bash .claude/skills/upstream-version-watch/scripts/single_serve_up.sh gptoss120b-0191-c0
```

**3. bench**

```sh
# bench JSON 필드로 복원: backend · model_id→--model · tokenizer_id→--tokenizer · num_prompts · max_concurrency · request_rate · burstiness · 요청당 입출력 길이 = total_*_tokens ÷ completed(나누어떨어질 때만 — 평균이다 · 요청별 길이는 JSON 에 없다) · 나누어떨어지면 `--dataset-name random` 으로 추정했다(측정 도구가 그 꼴로 부른다 — 아래 도구 원문).
# JSON 에 없는 인자(--endpoint · --base-url · --ignore-eos · --num-warmups · --temperature · --seed · --random-range-ratio · --trust-remote-code)는 측정 도구가 정했다 — level_* = run_bench.sh · lite_cold/lite_warm = lite_bench.sh(.claude/skills/adversarial-benchmark/scripts/) · 셀 스윕 순서는 sweep_bench.sh.
# output/single/benchlog/sweep_gptoss120b-0191-c0/lite_cold_gptoss120b-0191-c0.json (date 20261010-072602) · ⚠ --random-input-len/--random-output-len = total ÷ completed 평균(chat 템플릿 포함 실측 토큰 · 측정 도구 원문 인자 미관측 — lite_bench.sh 원문 인자와 다를 수 있다)
vllm bench serve --backend openai --model gpt-oss-120b --tokenizer /app/models/OpenAI/gpt-oss-120b --dataset-name random --random-input-len 512 --random-output-len 128 --num-prompts 1 --max-concurrency 1 --request-rate inf
# output/single/benchlog/sweep_gptoss120b-0191-c0/lite_warm_gptoss120b-0191-c0.json (date 20261010-072624) · ⚠ --random-input-len/--random-output-len = total ÷ completed 평균(chat 템플릿 포함 실측 토큰 · 측정 도구 원문 인자 미관측 — lite_bench.sh 원문 인자와 다를 수 있다)
vllm bench serve --backend openai --model gpt-oss-120b --tokenizer /app/models/OpenAI/gpt-oss-120b --dataset-name random --random-input-len 512 --random-output-len 128 --num-prompts 3 --max-concurrency 1 --request-rate inf
```

**슬롯 → 빌드 컨텍스트 매핑**(빌드 컨텍스트 = `output/<토폴로지>/` · 원천 = 실린 Dockerfile 의 COPY · compose 의 env_file · volumes · command — `—` 는 빌드 컨텍스트 입력이 아니다)

| zip 경로 | 빌드 컨텍스트 자리 | 근거 |
|---|---|---|
| `artifacts/triplet/.env.gptoss120b-0191-c0` | `envs/.env.gptoss120b-0191-c0` | compose env_file envs/.env.${CONFIG_FILE}(셀 env) |
| `artifacts/triplet/gptoss120b-0191-c0.sh` | `configs/gptoss120b-0191-c0.sh` | compose volumes ./configs:/app/configs |
| `artifacts/triplet/gptoss120b-0191-c0.yaml` | `configs/gptoss120b-0191-c0.yaml` | compose volumes ./configs:/app/configs |
| `artifacts/build_recipe/Dockerfile` | `Dockerfile` | docker build -f Dockerfile 의 입력 · build.context .(compose build.dockerfile 은 리터럴 Dockerfile.source-build — compose build 는 이 파일을 짓지 않는다) |
| `artifacts/build_recipe/requirements.txt` | `requirements.txt` | Dockerfile COPY requirements.txt |
| `artifacts/compose/.env.template` | `.env` | compose 변수치환 프로젝트 .env(${…} 참조) |
| `artifacts/compose/docker-compose.yaml` | `docker-compose.yaml` | compose 파일(빌드 컨텍스트 루트) |

**기동 성공 판정(관측 · 발행 자격)**: health 200 = true · 추론 1회 = true · 근거 `output/single/benchlog/sweep_gptoss120b-0191-c0/level_01/post_health_gptoss120b-0191-c0.json` · `output/single/benchlog/sweep_gptoss120b-0191-c0/level_01/guidellm_gptoss120b-0191-c0.json`
<!-- /FACT:reproduce -->

<!-- FACT:event_timeline -->
**블랙박스 이벤트 51행**(원장에서 이 셀 label·config 에 맞는 행만 · 시각순 · 기계 발췌 · 이번 캠페인 30행 · 같은 셀 이름 · 앞선 캠페인 21행(내용 칸에 표기))

> **캠페인 경계** 2026-10-10T07:15:17Z(`camp-26101017-knob-e2e-single` · evidence.event_campaign_boundary): campaigns/camp-26101017-knob-e2e-single/campaign.yaml declared_utc 2026-10-10T08:04:57Z · 이 발행이 묶인 측정을 서빙한 budget_declare 창 시작 2026-10-10T07:15:17Z → 경계 = 앞선 값 2026-10-10T07:15:17Z(이전 행 = 같은 셀 이름 · 앞선 캠페인)

> **원장 관측 범위**(노드별 첫 행 → 마지막 행 · evidence.event_ledger_spans): main = 2026-07-31T03:27:43Z → 2026-10-10T07:46:20Z(`docs/logs/main/events/2026-07.jsonl` · `docs/logs/main/events/2026-08.jsonl` · `docs/logs/main/events/2026-09.jsonl` · `docs/logs/main/events/2026-10.jsonl` · `docs/logs/main/events/thermal.jsonl` · `docs/logs/main/events/watchdog.jsonl`)

| # | UTC | 노드 | kind | label | 내용 | 출처 |
|---|---|---|---|---|---|---|
| 1 | 2026-09-04T11:11:24Z | main | `budget_declare` | `serve-gptoss120b-0191-c0` | 예산 선언(선언값 — 측정 아님) · floor_mib=19376 · weights_mib=62226 · kv_mib=30720 · overhead_mib=12288 · mem_total_mib=124610 · 기동 창 1/4: budget_renew 0회 · 닫힘 budget_clear 2026-09-04T11:11:24Z(선언 후 0m00s) · measured_utc 2026-10-10T07:45:58Z 는 이 창 밖(다른 기동 · 측정 전) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T07:15:17Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:108 |
| 2 | 2026-09-04T11:11:24Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → serve-gptoss120b-0191-c0 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-04T11:11:24Z 후 0m00s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T07:15:17Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:109 |
| 3 | 2026-09-04T11:11:24Z | main | `budget_honored` | `serve-gptoss120b-0191-c0` | budget_honored · floor_mib=19376 · arm_ceiling_mib=16304 · remaining_s=7200 · 창 시작 2026-09-04T11:11:24Z 후 0m00s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T07:15:17Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:74 |
| 4 | 2026-09-04T11:11:25Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → serve-gptoss120b-0191-c0 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-04T11:11:24Z 후 0m01s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T07:15:17Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:75 |
| 5 | 2026-09-04T11:12:07Z | main | `budget_declare` | `serve-gptoss120b-0191-c0` | 예산 선언(선언값 — 측정 아님) · floor_mib=19376 · weights_mib=62226 · kv_mib=30720 · overhead_mib=12288 · mem_total_mib=124610 · 기동 창 2/4: budget_renew 1회 · 닫힘 budget_clear 2026-09-04T11:19:08Z(선언 후 7m01s) · measured_utc 2026-10-10T07:45:58Z 는 이 창 밖(다른 기동 · 측정 전) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T07:15:17Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:110 |
| 6 | 2026-09-04T11:12:07Z | main | `serve_start` | — | serve_start · model=gpt-oss-120b · tensor_parallel_size=1 · topology=single · session=serve-gptoss120b-0191-c0-20260904T111207Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T07:15:17Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:111 |
| 7 | 2026-09-04T11:12:07Z | main | `budget_honored` | `serve-gptoss120b-0191-c0` | budget_honored · floor_mib=19376 · arm_ceiling_mib=16304 · remaining_s=7200 · 창 시작 2026-09-04T11:12:07Z 후 0m00s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T07:15:17Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:76 |
| 8 | 2026-09-04T11:12:08Z | main | `budget_renew` | `serve-gptoss120b-0191-c0` | budget_renew · floor_mib=19376 · ttl_s=7200 · remaining_before_s=7199 · 창 시작 2026-09-04T11:12:07Z 후 0m01s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T07:15:17Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:112 |
| 9 | 2026-09-04T11:12:42Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=93689 · rate_mib_s=21310 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → serve-gptoss120b-0191-c0 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-04T11:12:07Z 후 0m35s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T07:15:17Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:77 |
| 10 | 2026-09-04T11:19:08Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → serve-gptoss120b-0191-c0 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-04T11:12:07Z 후 7m01s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T07:15:17Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:113 |
| 11 | 2026-09-04T11:19:08Z | main | `serve_stop` | — | serve_stop · model=gpt-oss-120b · duration_s=421 · session=serve-gptoss120b-0191-c0-20260904T111207Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T07:15:17Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:114 |
| 12 | 2026-09-04T11:19:08Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → serve-gptoss120b-0191-c0 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-04T11:12:07Z 후 7m01s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T07:15:17Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:78 |
| 13 | 2026-09-04T11:21:34Z | main | `budget_declare` | `serve-gptoss120b-0191-c0` | 예산 선언(선언값 — 측정 아님) · floor_mib=19376 · weights_mib=62226 · kv_mib=30720 · overhead_mib=12288 · mem_total_mib=124610 · 기동 창 3/4: budget_renew 2회 · 닫힘 budget_clear 2026-09-04T12:17:59Z(선언 후 56m25s) · measured_utc 2026-10-10T07:45:58Z 는 이 창 밖(다른 기동 · 측정 전) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T07:15:17Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:115 |
| 14 | 2026-09-04T11:21:34Z | main | `serve_start` | — | serve_start · model=gpt-oss-120b · tensor_parallel_size=1 · topology=single · session=serve-gptoss120b-0191-c0-20260904T112134Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T07:15:17Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:116 |
| 15 | 2026-09-04T11:21:35Z | main | `budget_renew` | `serve-gptoss120b-0191-c0` | budget_renew · floor_mib=19376 · ttl_s=7200 · remaining_before_s=7199 · 창 시작 2026-09-04T11:21:34Z 후 0m01s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T07:15:17Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:117 |
| 16 | 2026-09-04T11:21:35Z | main | `budget_honored` | `serve-gptoss120b-0191-c0` | budget_honored · floor_mib=19376 · arm_ceiling_mib=16304 · remaining_s=7199 · 창 시작 2026-09-04T11:21:35Z 후 0m00s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T07:15:17Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:79 |
| 17 | 2026-09-04T11:22:05Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=72182 · rate_mib_s=28010 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → serve-gptoss120b-0191-c0 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-04T11:21:35Z 후 0m30s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T07:15:17Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:80 |
| 18 | 2026-09-04T11:51:35Z | main | `budget_renew` | `serve-gptoss120b-0191-c0` | budget_renew · floor_mib=19376 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-09-04T11:21:34Z 후 30m01s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T07:15:17Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:118 |
| 19 | 2026-09-04T12:17:59Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → serve-gptoss120b-0191-c0 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-04T11:21:34Z 후 56m25s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T07:15:17Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:119 |
| 20 | 2026-09-04T12:17:59Z | main | `serve_stop` | — | serve_stop · model=gpt-oss-120b · duration_s=3385 · session=serve-gptoss120b-0191-c0-20260904T112134Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T07:15:17Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-09.jsonl:120 |
| 21 | 2026-09-04T12:18:00Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → serve-gptoss120b-0191-c0 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-04T11:21:35Z 후 56m25s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T07:15:17Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:81 |
| 22 | 2026-10-10T07:15:17Z | main | `budget_declare` | `serve-gptoss120b-0191-c0` | 예산 선언(선언값 — 측정 아님) · floor_mib=13230 · weights_mib=62226 · kv_mib=30720 · overhead_mib=18432 · mem_total_mib=124608 · 기동 창 4/4: budget_renew 17회 · 닫힘 budget_clear 2026-10-10T07:46:19Z(선언 후 31m02s) · measured_utc 2026-10-10T07:45:58Z 가 이 창 안(이 측정의 기동) | docs/logs/main/events/2026-10.jsonl:336 |
| 23 | 2026-10-10T07:15:17Z | main | `serve_start` | — | serve_start · model=gpt-oss-120b · tensor_parallel_size=1 · topology=single · session=serve-gptoss120b-0191-c0-20261010T071517Z | docs/logs/main/events/2026-10.jsonl:337 |
| 24 | 2026-10-10T07:15:18Z | main | `budget_honored` | `serve-gptoss120b-0191-c0` | budget_honored · floor_mib=13230 · arm_ceiling_mib=10158 · remaining_s=7199 · 창 시작 2026-10-10T07:15:18Z 후 0m00s | docs/logs/main/events/watchdog.jsonl:719 |
| 25 | 2026-10-10T07:15:19Z | main | `budget_renew` | `serve-gptoss120b-0191-c0` | budget_renew · floor_mib=13230 · ttl_s=7200 · remaining_before_s=7198 · 창 시작 2026-10-10T07:15:17Z 후 0m02s | docs/logs/main/events/2026-10.jsonl:338 |
| 26 | 2026-10-10T07:15:43Z | main | `engine_log_window_start` | — | 캡처된 엔진 로그의 첫 시각(꼬리 캡처일 수 있다 — 엔진 시작 시각이 아닐 수 있다) · 선언 2026-10-10T07:15:17Z 후 0m26s · 시계: 컨테이너 시각을 UTC 로 읽어 측정 창 [2026-10-10T07:15:17Z, 2026-10-10T07:46:19Z] 안에 드는 것을 확인 | output/single/benchlog/sweep_gptoss120b-0191-c0/lite_engine_gptoss120b-0191-c0.log:41 |
| 27 | 2026-10-10T07:16:19Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=66408 · rate_mib_s=28305 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → serve-gptoss120b-0191-c0 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-10T07:15:18Z 후 1m01s) | docs/logs/main/events/watchdog.jsonl:720 |
| 28 | 2026-10-10T07:22:20Z | main | `engine_weights_loaded` | — | 가중치 적재 완료 줄 ×1(rank·단계별) · 최장 358.87s · 이 행 = 마지막 줄 | output/single/benchlog/sweep_gptoss120b-0191-c0/lite_engine_gptoss120b-0191-c0.log:86 |
| 29 | 2026-10-10T07:22:23Z | main | `engine_model_loaded` | — | 모델 적재 완료 줄 ×1(rank 별) · 마지막 줄 65.96 GiB · 365.4s | output/single/benchlog/sweep_gptoss120b-0191-c0/lite_engine_gptoss120b-0191-c0.log:88 |
| 30 | 2026-10-10T07:22:43Z | main | `engine_kv_cache_sized` | — | GPU KV cache 873,808 tokens | output/single/benchlog/sweep_gptoss120b-0191-c0/lite_engine_gptoss120b-0191-c0.log:94 |
| 31 | 2026-10-10T07:22:59Z | main | `engine_init_done` | — | 엔진 초기화(profile·KV 할당·warmup) 35.70s | output/single/benchlog/sweep_gptoss120b-0191-c0/lite_engine_gptoss120b-0191-c0.log:101 |
| 32 | 2026-10-10T07:23:06Z | main | `server_starting` | — | API 서버 기동 — 이 측정 창 선언 2026-10-10T07:15:17Z 후 7m49s · 캡처 로그 첫 시각 2026-10-10T07:15:43Z 후 7m23s | output/single/benchlog/sweep_gptoss120b-0191-c0/lite_engine_gptoss120b-0191-c0.log:106 |
| 33 | 2026-10-10T07:26:25Z | main | `budget_renew` | `serve-gptoss120b-0191-c0` | budget_renew · floor_mib=13230 · ttl_s=7200 · remaining_before_s=6534 · 창 시작 2026-10-10T07:15:17Z 후 11m08s | docs/logs/main/events/2026-10.jsonl:339 |
| 34 | 2026-10-10T07:28:55Z | main | `budget_renew` | `serve-gptoss120b-0191-c0` | budget_renew · floor_mib=13230 · ttl_s=7200 · remaining_before_s=7050 · 창 시작 2026-10-10T07:15:17Z 후 13m38s | docs/logs/main/events/2026-10.jsonl:340 |
| 35 | 2026-10-10T07:31:18Z | main | `budget_renew` | `serve-gptoss120b-0191-c0` | budget_renew · floor_mib=13230 · ttl_s=7200 · remaining_before_s=7057 · 창 시작 2026-10-10T07:15:17Z 후 16m01s | docs/logs/main/events/2026-10.jsonl:341 |
| 36 | 2026-10-10T07:33:40Z | main | `budget_renew` | `serve-gptoss120b-0191-c0` | budget_renew · floor_mib=13230 · ttl_s=7200 · remaining_before_s=7058 · 창 시작 2026-10-10T07:15:17Z 후 18m23s | docs/logs/main/events/2026-10.jsonl:342 |
| 37 | 2026-10-10T07:35:06Z | main | `budget_renew` | `serve-gptoss120b-0191-c0` | budget_renew · floor_mib=13230 · ttl_s=7200 · remaining_before_s=7114 · 창 시작 2026-10-10T07:15:17Z 후 19m49s | docs/logs/main/events/2026-10.jsonl:343 |
| 38 | 2026-10-10T07:36:31Z | main | `budget_renew` | `serve-gptoss120b-0191-c0` | budget_renew · floor_mib=13230 · ttl_s=7200 · remaining_before_s=7115 · 창 시작 2026-10-10T07:15:17Z 후 21m14s | docs/logs/main/events/2026-10.jsonl:344 |
| 39 | 2026-10-10T07:37:56Z | main | `budget_renew` | `serve-gptoss120b-0191-c0` | budget_renew · floor_mib=13230 · ttl_s=7200 · remaining_before_s=7115 · 창 시작 2026-10-10T07:15:17Z 후 22m39s | docs/logs/main/events/2026-10.jsonl:345 |
| 40 | 2026-10-10T07:39:02Z | main | `budget_renew` | `serve-gptoss120b-0191-c0` | budget_renew · floor_mib=13230 · ttl_s=7200 · remaining_before_s=7134 · 창 시작 2026-10-10T07:15:17Z 후 23m45s | docs/logs/main/events/2026-10.jsonl:346 |
| 41 | 2026-10-10T07:40:07Z | main | `budget_renew` | `serve-gptoss120b-0191-c0` | budget_renew · floor_mib=13230 · ttl_s=7200 · remaining_before_s=7135 · 창 시작 2026-10-10T07:15:17Z 후 24m50s | docs/logs/main/events/2026-10.jsonl:347 |
| 42 | 2026-10-10T07:41:11Z | main | `budget_renew` | `serve-gptoss120b-0191-c0` | budget_renew · floor_mib=13230 · ttl_s=7200 · remaining_before_s=7136 · 창 시작 2026-10-10T07:15:17Z 후 25m54s | docs/logs/main/events/2026-10.jsonl:348 |
| 43 | 2026-10-10T07:42:03Z | main | `budget_renew` | `serve-gptoss120b-0191-c0` | budget_renew · floor_mib=13230 · ttl_s=7200 · remaining_before_s=7148 · 창 시작 2026-10-10T07:15:17Z 후 26m46s | docs/logs/main/events/2026-10.jsonl:349 |
| 44 | 2026-10-10T07:42:55Z | main | `budget_renew` | `serve-gptoss120b-0191-c0` | budget_renew · floor_mib=13230 · ttl_s=7200 · remaining_before_s=7148 · 창 시작 2026-10-10T07:15:17Z 후 27m38s | docs/logs/main/events/2026-10.jsonl:350 |
| 45 | 2026-10-10T07:43:47Z | main | `budget_renew` | `serve-gptoss120b-0191-c0` | budget_renew · floor_mib=13230 · ttl_s=7200 · remaining_before_s=7148 · 창 시작 2026-10-10T07:15:17Z 후 28m30s | docs/logs/main/events/2026-10.jsonl:351 |
| 46 | 2026-10-10T07:44:30Z | main | `budget_renew` | `serve-gptoss120b-0191-c0` | budget_renew · floor_mib=13230 · ttl_s=7200 · remaining_before_s=7157 · 창 시작 2026-10-10T07:15:17Z 후 29m13s | docs/logs/main/events/2026-10.jsonl:352 |
| 47 | 2026-10-10T07:45:16Z | main | `budget_renew` | `serve-gptoss120b-0191-c0` | budget_renew · floor_mib=13230 · ttl_s=7200 · remaining_before_s=7154 · 창 시작 2026-10-10T07:15:17Z 후 29m59s | docs/logs/main/events/2026-10.jsonl:353 |
| 48 | 2026-10-10T07:45:19Z | main | `budget_renew` | `serve-gptoss120b-0191-c0` | budget_renew · floor_mib=13230 · ttl_s=7200 · remaining_before_s=7197 · 창 시작 2026-10-10T07:15:17Z 후 30m02s | docs/logs/main/events/2026-10.jsonl:354 |
| 49 | 2026-10-10T07:46:19Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → serve-gptoss120b-0191-c0 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-10T07:15:17Z 후 31m02s) | docs/logs/main/events/2026-10.jsonl:355 |
| 50 | 2026-10-10T07:46:19Z | main | `serve_stop` | — | serve_stop · model=gpt-oss-120b · duration_s=1862 · session=serve-gptoss120b-0191-c0-20261010T071517Z | docs/logs/main/events/2026-10.jsonl:356 |
| 51 | 2026-10-10T07:46:20Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → serve-gptoss120b-0191-c0 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-10T07:15:18Z 후 31m02s) | docs/logs/main/events/watchdog.jsonl:721 |

**기동 시도**(예산 선언 `budget_declare` 마다 한 시도 · 같은 노드의 이어지는 행과 선언 없는 노드(엔진 로그)의 그 시각 행을 묶었다 · 사살 · 트립 · 거부는 닫힘과 따로 센다 · `measurement` 행은 측정 기록이라 시도로 세지 않는다(위 표에만) — 해석은 저작자 몫)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 1 | main | `serve-gptoss120b-0191-c0` | 2026-10-10T07:15:17Z | 17회 · 첫 2026-10-10T07:15:19Z | 없음 | `budget_clear` 2026-10-10T07:46:19Z | 11 |

**같은 셀 이름 · 앞선 캠페인의 기동 3회**(캠페인 경계 2026-10-10T07:15:17Z 이전 — 라벨이 같아 원장에서 함께 묶였을 뿐 이 셀의 시도로 세지 않는다 · 행 전문 = 01 §1.4)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 앞선-1 | main | `serve-gptoss120b-0191-c0` | 2026-09-04T11:11:24Z | 없음 | 없음 | `budget_clear` 2026-09-04T11:11:24Z | 2 |
| 앞선-2 | main | `serve-gptoss120b-0191-c0` | 2026-09-04T11:12:07Z | 1회 · 첫 2026-09-04T11:12:08Z | 없음 | `budget_clear` 2026-09-04T11:19:08Z | 5 |
| 앞선-3 | main | `serve-gptoss120b-0191-c0` | 2026-09-04T11:21:34Z | 2회 · 첫 2026-09-04T11:21:35Z | 없음 | `budget_clear` 2026-09-04T12:17:59Z | 5 |
<!-- /FACT:event_timeline -->

<!-- FACT:measurement_env -->
_측정 실행의 env 관측 없음 — 측정 엔진 로그에서 env echo 줄(`<KEY> set by environment to <VALUE>` 등)을 찾지 못했다(evidence.measurement_env_observed · 부재 ≠ 미설정)._
<!-- /FACT:measurement_env -->

> 이 절이 답하는 질문: 수신자가 이 zip 만으로 이 셀을 다시 띄우려면 어떤 순서로 무엇을 하고, 각 단계의 성공을 무엇으로 판정하며, 얼마나 기다려야 하는가?

**기동 시도 수 · 총 소요**: 이번 캠페인 안에서 기동 1회(위 기동 시도 표 시도 1)이며 사살 · 트립 · 거부 없이 `budget_clear` 로 닫혔다. 예산 선언(07:15:17Z)부터 API 서버 기동 줄까지 7분 49초(원장 32행 · 엔진 로그 관측), 기동 스크립트의 health 대기는 475초였다(`b_serve.log` L22 · 1200초 상한 안). 2026-09-01 계보는 READY 376~406초를 적었다(testlog_26090117_gptoss120b_단일노드_FullE2E_판정 §측정값 · NAS 543 MB/s) — 두 값은 다른 날의 관측이고 이번이 더 길다.

(a) **사전 준비** — 가중치는 관리 NAS 를 읽기 전용으로 마운트한다(`/app/models` ← `<manifest.nas_model_path>` · compose 볼륨 · 획득 모드 managed). 스테이징할 파일은 없다(PLE 없음 · 노드 1대). gpt-oss 용 tiktoken 평탄 폴더를 `/encodings` 로 마운트하는 변수(`TIKTOKEN_HOST_PATH`)가 compose 에 있고, 러너는 셀 env 의 `TIKTOKEN_ENABLED=true` 일 때 `TIKTOKEN_ENCODINGS_BASE=/encodings` 를 export 한다. 필수 env 자리: 셀 env `artifacts/triplet/.env.gptoss120b-0191-c0`(IMAGE_TAG · CONFIG_FILE · SERVING_PORT 8081 · SERVING_MODEL_NAME) · 프로젝트 env `artifacts/compose/.env.template`(마운트 세 변수 — 값은 manifest). 측정 실행의 env echo 는 엔진 로그에서 잡히지 않았다(위 사실 블록 — 부재 ≠ 미설정).

(b) **빌드** — 메인 노드에서 1.4 build 명령(Dockerfile 선택자 = wheel `Dockerfile` · build-arg `VLLM_VERSION=0.19.1` · `CUDA_VERSION=130` · `RAY_VERSION=2.48.0` · 태그 `easy-vllm:0.19.1-cu130-aarch64-wheel`). 빌드 소요는 미관측이다 — 위 표의 58분은 docker history 층 창이지 빌드 시간이 아니다. 측정 이미지는 2026-09-03 에 같은 태그로 다시 지은 이미지다(층 창 2026-09-03T21:24:01Z → 22:22:09Z · 1.1 레시피 경고의 Created) — 2026-09-01 testlog 의 '34.3 GB' 는 그날 지은 다른 이미지의 기록이라 이 이미지의 크기로 읽지 않는다.

(c) **기동과 성공 판정** — `single_serve_up.sh gptoss120b-0191-c0 --overhead-mib 18432 --ready-max 1200`(testlog_26101017_08_29 §1. 서빙 — 위 표의 serve 명령은 사용법에서 파생한 형태이고 실제 실행은 이 두 인자를 더했다). 이 스크립트가 통로 · NAS · RAM 게이트(이번 기동: MemAvailable 119159MiB ≥ 요구 72467MiB) → 예산 선언 → 세션 시작 → compose up → 갱신 루프 → health 대기를 차례로 한다(`b_serve.log` L1-L23). 엔진 로그 기준 진행: 가중치 적재 358.87초 · 모델 적재 65.96 GiB / 365.4초 · KV 873,808 토큰 · 엔진 초기화(프로파일 생략 · KV 할당 · 그래프 캡처 12초) 35.70초 · API 서버 기동. 성공 판정은 health 200 + 추론 1회(위 표 · 발행 자격 근거 파일 둘)다. 확인할 로그 서명: `Using TRITON_ATTN attention backend out of potential backends: ['TRITON_ATTN']` · `Using 'MARLIN' Mxfp4 MoE backend.` · `Using fp8 data type to store kv cache.` · `reserved 30.0 GiB memory for KV Cache as specified by kv_cache_memory_bytes config`. 패치 발화 서명은 해당 없다(패치 없음). 정상인데 무서워 보이는 것: 로드 1분쯤의 `watchdog_highrate_hold`(`legacy_rule_would_trip=true` · 원장 27행) — 선언이 있어 사살로 가지 않는다 · torch `library.py` 의 flash_attn 커널 재등록 UserWarning · `For gpt-oss, we ignore --enable-auto-tool-choice` 경고 — 셋 다 벽이 아니다.

(d) **호스트 메모리 안전 예산 선언**(선언값 — 측정 아님): weights 62226MiB(체크포인트 크기) · kv 30720MiB · overhead 18432MiB · 총량 124608MiB → 바닥 13230MiB · arm 상한 10158MiB(원장 22·24행). 엔진 실측 상주는 가중치 65.96 GiB + KV 30.0 GiB(합 95.96 GiB · 03 §3.1 `lite.engine_reserved_total_gib`)이고, 체크포인트 크기로 넣은 weights 는 그보다 작다 — 기동 스크립트가 "바닥이 트립선 바로 위 밴드(+3072)" 경고를 냈다(`b_serve.log` L11). overhead 18432 의 산정 근거는 기록돼 있지 않다(02 Q2). TTL 7200초 선언을 기동 창 전체에서 17회 갱신했다 — 첫 1회(07:15:19Z)는 로드 중이고 나머지 16회(07:26:25Z~)가 측정 구간이다.

(e) **측정** — 위 표 bench 단계의 명령은 lite 레그(vLLM bench serve) 두 개를 bench JSON 에서 재구성한 것이다. 판정점과 동시성 스윕은 GuideLLM 0.7.3 컨테이너(`--backend openai` · 완결 엔드포인트 · 입력 1024 / 출력 256 · 레벨당 16 + warmup 2 · 반복 3)로 쟀고, 실제 실행 명령은 testlog_26101017_08_29 §2 의 `sweep_bench.sh … --tool guidellm --backend openai … --bench-budget-mib 2048 …` 이다. JSON 에 없는 도구 인자는 03 §3.4 의 측정 도구 원문 발췌를 본다. 판정은 `judge_bench.sh … --authority weak --reference-tps 33.53 --e-search hit`(같은 testlog §3).

(f) **엔진이 스스로 고른 경로** — 어텐션 TRITON_ATTN(후보가 그것 하나) · mxfp4 MoE MARLIN(auto) · KV fp8 · cudagraph(PIECEWISE 혼합 + FULL decode · 캡처 크기 상한을 1024 로 덮어씀) · torch.compile 캐시 재사용(`Directly load the compiled graph(s) … from the cache`). gmu 는 KV 산정에 쓰이지 않았다(엔진이 "does not respect the gpu_memory_utilization config" 라고 적음). 통신 백엔드는 world_size=1 이라 의미가 없다. 컨테이너는 CUDA Forward Compatibility 모드로 떴고 배너에 커널 드라이버 580.178.04 를 찍었다 — manifest 선언 드라이버(580.173.02)와 다르다(02 Q4).

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

체크포인트는 `openai/gpt-oss-120b` · git HEAD `b5c939de8f754692c1647ca79fbf85e8c1e70f8a`(00 §0.2) · 디스크 약 60.8 GiB(testlog_26090117_gptoss120b_단일노드_FullE2E_판정 §측정값 · 측정 방법 미기록)다. 이 셀의 획득 모드는 관리 NAS 읽기 전용 마운트였다(plan_26090112_59_49_메인_단일노드_gptoss120b_FullE2E §2. 확정된 입력 — 모델 실경로는 `<manifest.nas_model_path>/OpenAI/gpt-oss-120b`). 노드는 1대라 사본도 하나다. 새로 받아야 한다면 사람이 승인한 뒤 다음 꼴로 받는다 — 추론 — 실행 검증 ✗(이 계보는 다운로드를 기록하지 않았다):

```sh
huggingface-cli download openai/gpt-oss-120b --revision b5c939de8f754692c1647ca79fbf85e8c1e70f8a --local-dir <manifest.nas_model_path>/OpenAI/gpt-oss-120b
```

받은 뒤 엔진 로그의 `Model loading took … GiB` 로 상주 크기를 확인해 예산 선언의 weights 입력과 대조한다(02 §2.6).

### PLE · 스테이징

해당 없음 — 셀 config `declared_axes.ple_mode: none` · 노드 1대 · 런타임 패치 없음. 별도로 생성할 스테이징 파일이 없다.

### tiktoken 인코딩 폴더

compose 가 `${TIKTOKEN_HOST_PATH}` 를 `/encodings` 로 마운트하고(artifacts/compose/docker-compose.yaml — 주석 "tiktoken 평탄폴더(gpt-oss용)") 러너가 `TIKTOKEN_ENCODINGS_BASE=/encodings` 를 export 한다(artifacts/triplet/gptoss120b-0191-c0.sh). 폴더에 무엇을 둬야 하는지(인코딩 파일 이름)와 그것이 없을 때 오프라인 기동이 실패하는지는 이 계보에 기록이 없다 — 추론 — 실행 검증 ✗: 오프라인 환경이면 gpt-oss 토크나이저가 쓰는 tiktoken 인코딩 파일을 미리 받아 이 폴더에 둔다.

### 네트워크가 필요한 단계

- 가중치 다운로드(위 · 사람 승인 게이트) — 오프라인 대체: 관리 NAS 에 미리 둔다.
- 이미지 빌드: NGC 베이스 `nvcr.io/nvidia/pytorch:26.01-py3` pull · GitHub Release 의 vLLM 0.19.1 wheel 다운로드 · `requirements.txt` 와 `ray==2.48.0` 의 pip 설치 · apt(netcat 등) — artifacts/build_recipe/Dockerfile. 오프라인 대체: 연결된 곳에서 지은 이미지를 쓰는 대신 각 노드가 자기 이미지를 짓는 것이 이 저장소의 규약이므로, 베이스 이미지 · wheel · pip 패키지를 미리 받아 둔다(추론 — 실행 검증 ✗).
- 측정: GuideLLM 이미지 `ghcr.io/vllm-project/guidellm:v0.7.3` pull(`b_sweep.log` L10) — 컨테이너는 `HF_HUB_OFFLINE=1` 로 돌고 토크나이저 파일만 스테이징해 마운트한다(run_bench.sh@2661d614cf52 L397 · L318-L322).
