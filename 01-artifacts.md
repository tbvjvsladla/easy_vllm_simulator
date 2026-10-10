# 01 · 산출물 — 무엇이 실제로 쓰였고, 어떻게 다시 띄우는가

## 1.1 적용 판정 표
> 기계가 적용 증거(빌드 원장 · 원장이 없으면 라벨된 재구성) × 파일 × 선언을 대사한 결과다. `artifacts/` 에는 **적용된 것만** 실렸다 — 빌드 때 스스로 skip 된 패치와 쓰이지 않은 평면의 파일은 표에만 남고 싣지 않는다.

<!-- FACT:slots -->
| 슬롯 | 실림 | 파일 | 신호 | 적용 증거 | 사유 |
|---|---|---|---|---|---|
| `build_patch_post` | 안 실림 | — | 1-signal | none:no-build-ledger | 적용 판정 없음 — 원장 부재 — 라벨된 재구성: applied 0 · skip 판정 = docker-history+gate(기계 파생) |
| `build_patch_pre` | 안 실림 | — | 1-signal | none:no-build-ledger | 적용 판정 없음 — 원장 부재 — 라벨된 재구성: applied 0 · skip 판정 = docker-history+gate(기계 파생) |
| `build_recipe` | 실림 | `artifacts/build_recipe/Dockerfile.source-build` · `artifacts/build_recipe/requirements.txt` | 2-signal | file:image-history:docker history build-arg 관측(VLLM_REF=v0.26.0 · VLLM_VERSION 없음 → 소스빌드 Dockerfile) — 셀 env 선택자 없음 · output/single/docker-compose.yaml 리터럴 build.dockerfile: Dockerfile.source-build(선택자 변수 없음 — compose build 는 늘 Dockerfile.source-build 를 짓는다 · 이 이미지를 지은 증거 아님) | (a) 무엇을 고치나: 패치가 아니라 이미지를 짓는 레시피다. NGC `pytorch:26.05-py3` 위에서 vLLM 을 소스로 컴파일한다(`VLLM_REF=v0.26.0`, `TORCH_CUDA_ARCH=12.1a` — 1.1 재구성 입력). requirements 는 wheel 의 Requires-Dist 를 옮긴 목록이며 `flashinfer-python==0.6.14` 를 고정한다. (b) 왜 필요했나: 헤더가 소스빌드의 이유를 적는다 — prebuilt wheel 의 `_C` 가 NGC alpha torch 와 C++ ABI 가 맞지 않는다는 것. **그 검증은 헤더가 vLLM 0.22.1 에서 한 것으로 적었고, 이 셀의 0.26.0 에서 wheel 을 시도해 본 기록은 없다.** (c) 이… |
| `compose` | 실림 | `artifacts/compose/.env.template` · `artifacts/compose/docker-compose.yaml` | 2-signal | file:output/single/docker-compose.yaml | (a) single 서빙 경로의 형상이다(compose · env 변수 틀). `.env.template` 은 운영자 값을 `<manifest.…>` 자리표시로 바꾼 사본이다. (b) 이 셀을 컨테이너로 띄우는 데 필요한 배선이며 특정 벽을 해소하려고 넣은 것이 아니다. (c) 이 셀에서 쓰였다 — 측정이 이 compose 형상의 컨테이너에서 돌았다(1.1 `cell-run`). (d) 폐기 조건은 compose 개정 주기를 따른다. 1.1 이 알리듯 `arm_patch.sh` 는 `c-tq4nc_patch.py` 가 없어 no-op 이다 — 런타임 패치는 이 셀에 해당하지 않는다. |
| `fork_pin` | 안 실림 | — | 1-signal | none:.claude/policies/arch_variant_ledger.json | 셀 env 에 VARIANT 줄 없음 = stock(workflow.md §변종 좌표의 거처)(기계 파생) |
| `runtime_patch` | 안 실림 | — | 1-signal | none:output/single/configs/c-tq4nc_patch.py | output/single/configs/c-tq4nc_patch.py 없음 — arm_patch.sh 는 no-op(런타임 패치 불해당)(기계 파생) |
| `triplet` | 실림 | `artifacts/triplet/.env.c-tq4nc` · `artifacts/triplet/c-tq4nc.sh` · `artifacts/triplet/c-tq4nc.yaml` | 3-signal | file:output/single/configs/c-tq4nc.yaml | (a) 서빙 인자 세 종이다. yaml 은 5개 서빙 노브(1.3)와 24GB 시뮬 출신 주석을, 러너는 `vllm serve --config` 위에 served-model-name · auto-tool-choice · `--tool-call-parser hermes` · `--reasoning-parser qwen3` 를, env 는 포트 8000 · served-model-name `qwen3-4b` · 이미지 태그를 정한다. (b) 파서 두 개와 `TIKTOKEN_ENABLED=true` 는 이 계보에 그 필요를 보인 로그가 없다 — 헤더가 "gpt-oss-20b-normal.sh 구조 동일" 이라 적은 상속 구조일 가능성이 있으나 확인하지 않았다. (c) 이 셀에서 발화한 것: yaml 노브는 엔진… |

**레시피 경고 · 실린 리비전**(기계 관측 — 수신자 빌드가 여기서 갈라질 수 있다)

- `build_recipe` `output/single/Dockerfile.source-build` 실린 리비전: 리비전 미특정 · 선택 방법 `worktree(측정 이미지 docker history 의 RUN·COPY 와 전수 일치)` · docker history 대조 16/16 · 실린 바이트 `worktree` · 쓰인 바이트임을 관측으로 확인 예
- `build_recipe` `output/single/requirements.txt` 실린 리비전: 리비전 미특정 · 선택 방법 `worktree(image-probe: 이미지 안 COPY 대상 바이트와 sha256 일치)` · docker history 대조 없음 · 실린 바이트 `worktree` · 쓰인 바이트임을 관측으로 확인 예
- `compose` `output/single/docker-compose.yaml` 실린 리비전: 커밋 `80284db86dd7` · 선택 방법 `worktree(mtime 2026-10-10T04:54:24Z ≤ 측정 2026-10-10T06:26:21Z(sweep_index.generated_utc) · 바이트 = 측정 전 마지막 커밋 80284db86dd7)` · docker history 대조 없음 · 실린 바이트 `worktree` · 근거 `mtime≤measured` · 쓰인 바이트임을 관측으로 확인 예

**싣지 않은 파일**(슬롯별 사유 — 빌드 때 skip 된 패치는 아래 적용 집합 표에 있다)

- `build_patch_post` known-non-patch 1건: `.gitkeep`
- `build_patch_pre` known-non-patch 1건: `.gitkeep`
- `compose` `arm_patch.sh` — `not-armed(output/single/configs/c-tq4nc_patch.py 없음 — arm_patch.sh 는 no-op)`

**파일별 검증 · 관련성**(`generated-unverified` = 실행 검증되지 않은 생성물 — 파일 첫 줄 경고 · PAYLOAD 파일 기록과 같은 표시 · 관련성 = ① 패치 선언 대상 × 모델 config ② 엔진 로그 발화 서명 — 둘 다 관측일 때만 required/inactive-inferred)

| 슬롯 | 파일 | 검증 | 검증 근거 | 관련성 | 관련성 근거 |
|---|---|---|---|---|---|
| `build_recipe` | `artifacts/build_recipe/Dockerfile.source-build` | `verified` · cell-run | 쓰인 레시피(image-history:docker history build-arg 관측(VLLM_REF=v0.26.0 · VLLM_VERSION 없음 → 소스빌드 Dockerfile) — 셀 env 선택자 없음 · output/single/docker-compose.yaml 리터럴 build.dockerfile: Dockerfile.source-build(선택자 변수 없음 — compose build 는 늘 Dockerfile.source-build 를 짓는다 · 이 이미지를 지은 증거 아님)) — 개정 선택 slots.build_recipe.evidence.selected_revisions | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `build_recipe` | `artifacts/build_recipe/requirements.txt` | `verified` · cell-run | 쓰인 레시피(image-history:docker history build-arg 관측(VLLM_REF=v0.26.0 · VLLM_VERSION 없음 → 소스빌드 Dockerfile) — 셀 env 선택자 없음 · output/single/docker-compose.yaml 리터럴 build.dockerfile: Dockerfile.source-build(선택자 변수 없음 — compose build 는 늘 Dockerfile.source-build 를 짓는다 · 이 이미지를 지은 증거 아님)) — 개정 선택 slots.build_recipe.evidence.selected_revisions | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/.env.template` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/docker-compose.yaml` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/.env.c-tq4nc` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/c-tq4nc.sh` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/c-tq4nc.yaml` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
<!-- /FACT:slots -->

<!-- FACT:applied_set -->
**적용 집합** — 상태 `unobservable` · 출처 `no-build-ledger` · 원장 없음: 아래 결과는 **라벨된 재구성**이다(관측 아님)

_빌드 패치 없음 — 이 이미지에 빌드 패치 슬롯 입력이 없다._

**원장 탐침**(순위: attestation → 메인 로컬 이미지 → 재구성)

| 순위 | 결과 | 대상 |
|---|---|---|
| attestation | absent | output/single/benchlog/attestation_c-tq4nc.json(부재) |
| local-image | skipped(node=sub — 서브 스캔 ✗ · 문서기반 회수만) | — |
| reconstruction | done | — |
| image-probe | done | sha256:629942890c0570c8f1c3235bf6927ae4eb315e30336cd68551fdf474a60a44f7 |

**재구성 입력** — `docker-history+gate-rule · fail-loud-rule · image-probe`(docker history build-arg × 패치 자기게이트 규칙)

| build-arg | 값 |
|---|---|
| `BUILD_JOBS` | 16 |
| `RAY_VERSION` | 2.48.0 |
| `SM12X_PORT` | 0 |
| `SRC_DEPS_AUTHORITY` | 0 |
| `TORCH_CUDA_ARCH` | 12.1a |
| `VLLM_PRETEND_VERSION` | (빈 값) |
| `VLLM_REF` | v0.26.0 |
| `VLLM_REPO` | https://github.com/vllm-project/vllm.git |
<!-- /FACT:applied_set -->

## 1.2 슬롯별 적용 사유
> 이 절이 답하는 질문: 실린 산출물 하나하나는 무엇을 고치며, 왜 이 셀에 필요했고, 이 셀에서 실제로 발화했으며, 언제 불필요해지는가?

1.1 표 기준: 적용(실림) 슬롯은 `build_recipe` · `compose` · `triplet` 셋이다. 빌드 원장이 없는 이미지라 1.1 의 패치 적용 판정은 `unobservable` 이며, 아래는 라벨된 재구성이 아니라 **실린 파일의 헤더 주석과 로그**가 말하는 것이다.

### build_recipe — `Dockerfile.source-build` · `requirements.txt`
(a) 무엇을 고치나: 패치가 아니라 이미지를 짓는 레시피다. NGC `pytorch:26.05-py3` 위에서 vLLM 을 소스로 컴파일한다(`VLLM_REF=v0.26.0`, `TORCH_CUDA_ARCH=12.1a` — 1.1 재구성 입력). requirements 는 wheel 의 Requires-Dist 를 옮긴 목록이며 `flashinfer-python==0.6.14` 를 고정한다. (b) 왜 필요했나: 헤더가 소스빌드의 이유를 적는다 — prebuilt wheel 의 `_C` 가 NGC alpha torch 와 C++ ABI 가 맞지 않는다는 것. **그 검증은 헤더가 vLLM 0.22.1 에서 한 것으로 적었고, 이 셀의 0.26.0 에서 wheel 을 시도해 본 기록은 없다.** (c) 이 셀에서 발화했나: 이미지는 이 레시피로 지어졌다(1.1: docker history 대조 16/16). 패치 가드는 `RUN echo "[guard] source-build key (26.05-py3 x vLLM 0.26.0): validated patch set …"` 로 통과 표시만 남기는 형태다. (d) 언제 불필요해지나: 헤더 서술상 wheel 의 ABI 가 베이스 torch 와 맞을 때이며, 그 시점은 이 계보가 관측하지 않았다. 미인식 torch 핀에서 가드가 빌드를 멈추는지 skip 하는지는 헤더가 "빌드를 명시 실패시켜" 라고 적는다(아래 발췌 — 가드를 실제로 실패시켜 본 적은 없다).

> [원문] Dockerfile.source-build §L28-34
> # 왜 source-build 인가 (torch 2.11+ 구간): prebuilt vLLM wheel 의 _C(public torch 빌드)가
> # NGC alpha torch 와 C++ ABI 불일치(_C.abi3.so 가 c10::cuda::CUDAStream::query() 를 요구하나
> # NGC libc10_cuda.so 에 부재) → import 단계 하드 월. _C 를 NGC torch 에 맞춰 직접 컴파일하면
> # undefined-symbol 이 사라진다. 검증: vLLM 0.22.1 (.claude/skills/upstream-version-watch/references/source-build.md §3 / testlog_260608_1).
> #
> # ⚠ 패치 가드: 검증된 torch핀에서만 빌드한다(아래 guard). 미인식 핀이면 빌드를 명시 실패시켜
> #   "false determinism"(상류가 그 줄을 바꿨거나 torch 핀이 다른데도 옛 패치를 박는 것)을 막는다.

### compose — `docker-compose.yaml` · `.env.template`
(a) single 서빙 경로의 형상이다(compose · env 변수 틀). `.env.template` 은 운영자 값을 `<manifest.…>` 자리표시로 바꾼 사본이다. (b) 이 셀을 컨테이너로 띄우는 데 필요한 배선이며 특정 벽을 해소하려고 넣은 것이 아니다. (c) 이 셀에서 쓰였다 — 측정이 이 compose 형상의 컨테이너에서 돌았다(1.1 `cell-run`). (d) 폐기 조건은 compose 개정 주기를 따른다. 1.1 이 알리듯 `arm_patch.sh` 는 `c-tq4nc_patch.py` 가 없어 no-op 이다 — 런타임 패치는 이 셀에 해당하지 않는다.

### triplet — `c-tq4nc.yaml` · `c-tq4nc.sh` · `.env.c-tq4nc`
(a) 서빙 인자 세 종이다. yaml 은 5개 서빙 노브(1.3)와 24GB 시뮬 출신 주석을, 러너는 `vllm serve --config` 위에 served-model-name · auto-tool-choice · `--tool-call-parser hermes` · `--reasoning-parser qwen3` 를, env 는 포트 8000 · served-model-name `qwen3-4b` · 이미지 태그를 정한다. (b) 파서 두 개와 `TIKTOKEN_ENABLED=true` 는 이 계보에 그 필요를 보인 로그가 없다 — 헤더가 "gpt-oss-20b-normal.sh 구조 동일" 이라 적은 상속 구조일 가능성이 있으나 확인하지 않았다. (c) 이 셀에서 발화한 것: yaml 노브는 엔진 로그에 모두 반영되었다(L77 클램프 · L78 토큰 수). 러너의 `VLLM_ATTENTION_BACKEND` 는 엔진이 알 수 없는 변수라 경고했다(W1). (d) 폐기 조건: 노브는 승계값이므로 GPU 가 바뀌면 다시 도출한다(V1~V5).

### 싣지 않은 슬롯
`build_patch_pre` · `build_patch_post` — 빌드 패치 입력 없음(`.gitkeep` 만 있음 — 1.1). `fork_pin` — 셀 env 에 VARIANT 줄 없음 = stock. `runtime_patch` — `c-tq4nc_patch.py` 없음. 부재는 미판정이 아니라 1.1 이 기계로 가른 불해당이다.

## 1.3 값의 지위표
> 서빙 설정의 노브 **전부**와 기계가 모은 후보 지위다(lockset 출처 · yaml 주석 · 엔진 기본값). 최종 지위는 아래 `kind: value-status` 블록이 정한다 — 표의 노브마다 블록이 정확히 하나 있어야 린터를 통과한다.

<!-- FACT:value_status -->
서빙 노브 5개 — 노브마다 `kind: value-status` 블록이 **정확히 1개** 있어야 한다(노브·값은 이 표의 글자 그대로 · 값 칸의 `\|` 는 `|` 로 적는다).

| 노브 | 값 | 후보 지위 | 후보 근거 | yaml 주석 | 출처 |
|---|---|---|---|---|---|
| `gpu-memory-utilization` | 0.9 | inherited | lockset 출처 gmu_source=hand · lockset provenance=hand-authored | — | lockset.gmu_source=hand |
| `max-model-len` | 32768 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `max-num-seqs` | 8 | inherited | lockset 출처 batch_source=hand-lever · lockset provenance=hand-authored | — | lockset.batch_source=hand-lever |
| `kv-cache-memory-bytes` | 11811160064 | inherited | lockset 출처 kv_source=hand · lockset provenance=hand-authored | — | lockset.kv_source=hand |
| `kv-cache-dtype` | turboquant_4bit_nc | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
<!-- /FACT:value_status -->

> 이 절이 답하는 질문: 각 서빙 노브의 값은 이 셀에서 어떤 지위인가 — 조정됐나, 승계됐나, 음성대조로 일부러 둔 것인가, 엔진 기본값인가, 필요조건인가?

```hint-event
id: V1
kind: value-status
노브: gpu-memory-utilization
값: 0.9
지위: inherited
근거: lockset gmu_source=hand · 24GB 시뮬 셀에서 가져온 값이며 이 계보에서 바꿔 가며 잰 적이 없다. KV 를 바이트로 고정하면 엔진은 이 값을 따르지 않는다고 로그가 직접 말한다(L77)
출처: [lite_engine_c-tq4nc.log §L77-77, c-tq4nc.yaml]
```

```hint-event
id: V2
kind: value-status
노브: max-model-len
값: 32768
지위: inherited
근거: 24GB 시뮬 셀의 컨텍스트를 승계 — 다른 길이로 잰 기록이 없다. 엔진은 이 길이 기준 동시성 7.11x 를 보고했다(L79)
출처: [c-tq4nc.yaml, lite_engine_c-tq4nc.log §L79-79]
```

```hint-event
id: V3
kind: value-status
노브: max-num-seqs
값: 8
지위: inherited
근거: lockset batch_source=hand-lever (손레버) · 스윕 기록 없음. 벤치 레벨 16 이 이 상한 때문에 걸러졌다는 기록이 있다(testlog §5)
출처: [c-tq4nc.yaml, testlog_26101015_knob프리셋_E2E_셀D_qwen3-4b §5.]
```

```hint-event
id: V4
kind: value-status
노브: kv-cache-memory-bytes
값: 11811160064
지위: inherited
근거: lockset kv_source=hand · 24GB 시뮬 예산에서 정한 11 GiB 클램프를 승계했다. GB10 에서 재도출한 기록이 없다. 엔진은 이 값으로 메모리 프로파일링을 건너뛰고 KV 를 예약했다(L77)
출처: [c-tq4nc.yaml, lite_engine_c-tq4nc.log §L77-77]
```

```hint-event
id: V5
kind: value-status
노브: kv-cache-dtype
값: turboquant_4bit_nc
지위: inherited
근거: 이 셀을 정의하는 축으로 셀 생성 때 정해졌고, 같은 계보에서 다른 KV dtype 과 대조한 기록이 없다(대안 config 는 이 노드에 없었다 — testlog §2). TurboQuant 때문에 엔진이 FA2 로 내렸다(W2)
출처: [c-tq4nc.yaml, testlog_26101015_knob프리셋_E2E_셀D_qwen3-4b §2.]
```

**요약.** 다섯 노브 전부 승계값이고 조정된 값은 없다. 복사하면 위험한 것은 두 가지다: ① V2·V4 의 길이·KV 클램프는 24GB 카드를 가정해 정한 숫자라 다른 메모리의 GPU 에서 그대로 쓰면 KV 가 남거나 모자란다(엔진 로그의 토큰 수 L78 로 다시 잰다). ② V1 의 gmu 0.9 는 KV 바이트를 고정한 상태에서는 KV 크기를 정하지 않는다(L77) — gmu 를 만지면 KV 가 바뀔 거라 기대하지 말 것. 이 표에 없는 러너 쪽 `--tool-call-parser hermes` · `--reasoning-parser qwen3` 는 노브 표 밖이며 지위를 부여하지 않았다.

## 1.4 재현 절차
> 셀 실행 기록에서 기계가 파생한 명령 순서와 단계별 소요다. 소요가 `미관측` 인 단계는 시각 기록이 없다는 뜻이다(0 이 아니다). 그 아래는 블랙박스 이벤트 원장에서 이 셀의 행을 기계가 뽑아 기동 시도로 묶은 것이고, 마지막은 측정한 실행의 엔진 로그가 echo 한 env 다(측정 당시 값의 정본).

<!-- FACT:reproduce -->
| 순서 | 단계 | 성공 판정 | 소요 | 관측 구간(UTC) | 소요 출처 | 명령 출처 |
|---|---|---|---|---|---|---|
| 1 | build | 빌드 OK · 이미지 실재 | 층 창 876시간 27분 13초(docker history 첫 층 → 끝 층 CreatedAt · 캐시 재사용 층 포함 — 빌드 소요 아님) | 2026-08-02T07:16:14Z → 2026-09-07T19:43:27Z (layer-window) | docker history --no-trunc --format '{{json .}}' sha256:629942890c0570c8f1c3235bf6927ae4eb315e30336cd68551fdf474a60a44f7 · 우리 Dockerfile 층 29개의 CreatedAt 첫 층 → 끝 층(캐시 재사용 층은 이전 빌드 시각 — 빌드 소요 아님) | reconstructed(docker-history build-args + compose build) |
| 2 | serve | health 200 + 추론 1회(스모크 판정) | ≤ 31분 57초(상한)(끝 = 최종 스윕 첫 측정 · 그 전 요청 202건 — 소요 출처 칸) | 2026-10-10T05:33:14Z → 2026-10-10T06:05:11Z (upper) | docs/logs/sub/events/2026-10.jsonl budget_declare(label serve-c-tq4nc) → 첫 측정 시각(output/single/benchlog/sweep_c-tq4nc/lite_raw_c-tq4nc.json measured_utc(호스트 date -u · lite_bench.sh 가 cold 레그 직전에 찍는 측정 시작)) · ⚠ 최종(재실행) 스윕 기준 — 같은 serve 세션 안 최종 스윕 첫 측정 전 completions 요청 202건 관측(상한 시각 2026-10-10T05:35:53Z~2026-10-10T05:56:13Z · 스모크 추론 1회 초과 = 앞선 벤치 실행(끊긴 스윕 등)이 있었을 수 있다 · 출처 output/single/benchlog/sweep_c-tq4nc/lite_engine_head_c-tq4nc.log · output/single/benchlog/sweep_c-tq4nc/lite_engine_c-tq4nc.log 의 POST 접근 줄 × 다음 vLLM 시각 줄(컨테이너 시계 UTC 가정)) | derived(스모크 사용법) |
| 3 | bench | 리포트 발행(+PASS 면 인증서) | 21분 10초(최종 스윕 기준 · 그 전 요청 202건 — 소요 출처 칸) | 2026-10-10T06:05:11Z → 2026-10-10T06:26:21Z (exact) | output/single/benchlog/sweep_c-tq4nc/lite_raw_c-tq4nc.json measured_utc(호스트 date -u · lite_bench.sh 가 cold 레그 직전에 찍는 측정 시작) → sweep_index.generated_utc(호스트 UTC · 두 끝 같은 시계) · ⚠ 최종(재실행) 스윕 기준 — 같은 serve 세션 안 최종 스윕 첫 측정 전 completions 요청 202건 관측(상한 시각 2026-10-10T05:35:53Z~2026-10-10T05:56:13Z · 스모크 추론 1회 초과 = 앞선 벤치 실행(끊긴 스윕 등)이 있었을 수 있다 · 출처 output/single/benchlog/sweep_c-tq4nc/lite_engine_head_c-tq4nc.log · output/single/benchlog/sweep_c-tq4nc/lite_engine_c-tq4nc.log 의 POST 접근 줄 × 다음 vLLM 시각 줄(컨테이너 시계 UTC 가정)) | reconstructed(bench json) |

**빌드 층 시각**(관측 · 소요가 아니라 층 창 — 캐시 재사용 층은 이전 빌드의 시각을 가진다)

- `build` 이미지 층 CreatedAt(docker history): 2026-08-02T07:16:14Z → 2026-09-07T19:43:27Z · 층 29개 · 출처 `docker history --no-trunc --format '{{json .}}' sha256:629942890c0570c8f1c3235bf6927ae4eb315e30336cd68551fdf474a60a44f7` — 우리 Dockerfile 층의 CreatedAt 범위 — 캐시 재사용 층은 이전 빌드 시각을 가진다(소요 아님)

**단계별 명령**(기계 파생 — `명령 출처` 가 `derived(…)` 면 사용법에서 파생 · `reconstructed(…)` 면 괄호 안 관측(docker history build-arg · compose build · bench JSON 필드)에서 재구성한 명령이다. 어느 쪽도 실행 원문(로그에 남은 줄)은 아니다)

**1. build**

```sh
# 실린 artifacts/build_recipe/Dockerfile.source-build(이미지를 지은 개정: worktree)를 output/single/Dockerfile.source-build 자리에 두고 컨텍스트에서 짓는다.
docker build -f output/single/Dockerfile.source-build \
  --build-arg BUILD_JOBS=16 \
  --build-arg RAY_VERSION=2.48.0 \
  --build-arg SM12X_PORT=0 \
  --build-arg SRC_DEPS_AUTHORITY=0 \
  --build-arg TORCH_CUDA_ARCH=12.1a \
  --build-arg VLLM_PRETEND_VERSION= \
  --build-arg VLLM_REF=v0.26.0 \
  --build-arg VLLM_REPO=https://github.com/vllm-project/vllm.git \
  -t easy-vllm:0.26.0-cu132-aarch64-source \
  output/single
# 동등 경로(compose): docker compose -f output/single/docker-compose.yaml --env-file output/single/envs/.env.c-tq4nc build
```

**2. serve**

```sh
bash .claude/skills/upstream-version-watch/scripts/single_serve_up.sh c-tq4nc
```

**3. bench**

```sh
# bench JSON 필드로 복원: backend · model_id→--model · tokenizer_id→--tokenizer · num_prompts · max_concurrency · request_rate · burstiness · 요청당 입출력 길이 = total_*_tokens ÷ completed(나누어떨어질 때만 — 평균이다 · 요청별 길이는 JSON 에 없다) · 나누어떨어지면 `--dataset-name random` 으로 추정했다(측정 도구가 그 꼴로 부른다 — 아래 도구 원문).
# JSON 에 없는 인자(--endpoint · --base-url · --ignore-eos · --num-warmups · --temperature · --seed · --random-range-ratio · --trust-remote-code)는 측정 도구가 정했다 — level_* = run_bench.sh · lite_cold/lite_warm = lite_bench.sh(.claude/skills/adversarial-benchmark/scripts/) · 셀 스윕 순서는 sweep_bench.sh.
# output/single/benchlog/sweep_c-tq4nc/lite_cold_c-tq4nc.json (date 20261010-060524) · ⚠ --random-input-len/--random-output-len = total ÷ completed 평균(chat 템플릿 포함 실측 토큰 · 측정 도구 원문 인자 미관측 — lite_bench.sh 원문 인자와 다를 수 있다)
vllm bench serve --backend openai-chat --model qwen3-4b --tokenizer /app/models/Qwen/Qwen3/Qwen3-4B --dataset-name random --random-input-len 520 --random-output-len 128 --num-prompts 1 --max-concurrency 1 --request-rate inf
# output/single/benchlog/sweep_c-tq4nc/lite_warm_c-tq4nc.json (date 20261010-060554) · ⚠ --random-input-len/--random-output-len = total ÷ completed 평균(chat 템플릿 포함 실측 토큰 · 측정 도구 원문 인자 미관측 — lite_bench.sh 원문 인자와 다를 수 있다)
vllm bench serve --backend openai-chat --model qwen3-4b --tokenizer /app/models/Qwen/Qwen3/Qwen3-4B --dataset-name random --random-input-len 520 --random-output-len 128 --num-prompts 3 --max-concurrency 1 --request-rate inf
```

**슬롯 → 빌드 컨텍스트 매핑**(빌드 컨텍스트 = `output/<토폴로지>/` · 원천 = 실린 Dockerfile 의 COPY · compose 의 env_file · volumes · command — `—` 는 빌드 컨텍스트 입력이 아니다)

| zip 경로 | 빌드 컨텍스트 자리 | 근거 |
|---|---|---|
| `artifacts/triplet/.env.c-tq4nc` | `envs/.env.c-tq4nc` | compose env_file envs/.env.${CONFIG_FILE}(셀 env) |
| `artifacts/triplet/c-tq4nc.sh` | `configs/c-tq4nc.sh` | compose volumes ./configs:/app/configs |
| `artifacts/triplet/c-tq4nc.yaml` | `configs/c-tq4nc.yaml` | compose volumes ./configs:/app/configs |
| `artifacts/build_recipe/Dockerfile.source-build` | `Dockerfile.source-build` | compose build.dockerfile Dockerfile.source-build(리터럴) · build.context . |
| `artifacts/build_recipe/requirements.txt` | `requirements.txt` | Dockerfile.source-build COPY requirements.txt |
| `artifacts/compose/.env.template` | `.env` | compose 변수치환 프로젝트 .env(${…} 참조) |
| `artifacts/compose/docker-compose.yaml` | `docker-compose.yaml` | compose 파일(빌드 컨텍스트 루트) |

**기동 성공 판정(관측 · 발행 자격)**: health 200 = true · 추론 1회 = true · 근거 `output/single/benchlog/sweep_c-tq4nc/level_01/post_health_c-tq4nc.json` · `output/single/benchlog/sweep_c-tq4nc/level_01/guidellm_c-tq4nc.json`
<!-- /FACT:reproduce -->

<!-- FACT:event_timeline -->
**블랙박스 이벤트 32행**(원장에서 이 셀 label·config 에 맞는 행과 그와 시간이 겹치는 같은 노드의 다른 라벨 창 · 시각순 · 기계 발췌 · 이번 캠페인 14행 · 같은 셀 이름 · 앞선 캠페인 18행(내용 칸에 표기) · 라벨 다름(같은 노드 · 창 밖 이 셀 행과 시간이 겹치는 다른 라벨 선언 창) 7행 — 이 셀의 선언으로 세지 않는다)

> **캠페인 경계** 2026-10-10T05:33:14Z(`camp-26101104-hint-cd` · evidence.event_campaign_boundary): campaigns/camp-26101104-hint-cd/campaign.yaml declared_utc 2026-10-10T19:53:02Z · 이 발행이 묶인 측정을 서빙한 budget_declare 창 시작 2026-10-10T05:33:14Z → 경계 = 앞선 값 2026-10-10T05:33:14Z(이전 행 = 같은 셀 이름 · 앞선 캠페인)

> **원장 관측 범위**(노드별 첫 행 → 마지막 행 · evidence.event_ledger_spans): sub = 2026-06-13T22:25:01Z → 2026-10-10T15:17:46Z(`docs/logs/sub/events/2026-06.jsonl` · `docs/logs/sub/events/2026-07.jsonl` · `docs/logs/sub/events/2026-09.jsonl` · `docs/logs/sub/events/2026-10.jsonl` · `docs/logs/sub/events/thermal.jsonl` · `docs/logs/sub/events/watchdog.jsonl`)

| # | UTC | 노드 | kind | label | 내용 | 출처 |
|---|---|---|---|---|---|---|
| 1 | 2026-09-07T20:06:36Z | sub | `budget_declare` | `serve-c-tq4nc` | 예산 선언(선언값 — 측정 아님) · floor_mib=101578 · weights_mib=7672 · kv_mib=11264 · overhead_mib=4096 · mem_total_mib=124610 · 기동 창 1/2: budget_renew 3회 · 닫힘 budget_clear 2026-09-07T21:12:30Z(선언 후 1h05m54s) · measured_utc 2026-10-10T06:26:21Z 는 이 창 밖(다른 기동 · 측정 전) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:33:14Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-09.jsonl:135 |
| 2 | 2026-09-07T20:06:36Z | sub | `serve_start` | — | serve_start · model=qwen3-4b · tensor_parallel_size=1 · topology=single · session=serve-c-tq4nc-20260907T200636Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:33:14Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-09.jsonl:136 |
| 3 | 2026-09-07T20:06:37Z | sub | `budget_renew` | `serve-c-tq4nc` | budget_renew · floor_mib=101578 · ttl_s=7200 · remaining_before_s=7199 · 창 시작 2026-09-07T20:06:36Z 후 0m01s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:33:14Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-09.jsonl:137 |
| 4 | 2026-09-07T20:06:37Z | sub | `budget_honored` | `serve-c-tq4nc` | budget_honored · floor_mib=101578 · arm_ceiling_mib=98506 · remaining_s=7199 · 창 시작 2026-09-07T20:06:37Z 후 0m00s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:33:14Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:80 |
| 5 | 2026-09-07T20:36:37Z | sub | `budget_renew` | `serve-c-tq4nc` | budget_renew · floor_mib=101578 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-09-07T20:06:36Z 후 30m01s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:33:14Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-09.jsonl:138 |
| 6 | 2026-09-07T21:06:37Z | sub | `budget_renew` | `serve-c-tq4nc` | budget_renew · floor_mib=101578 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-09-07T20:06:36Z 후 1h00m01s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:33:14Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-09.jsonl:139 |
| 7 | 2026-09-07T21:12:30Z | sub | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → serve-c-tq4nc 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-07T20:06:36Z 후 1h05m54s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:33:14Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-09.jsonl:140 |
| 8 | 2026-09-07T21:12:30Z | sub | `serve_stop` | — | serve_stop · model=qwen3-4b · duration_s=3954 · session=serve-c-tq4nc-20260907T200636Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:33:14Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-09.jsonl:141 |
| 9 | 2026-09-07T21:12:31Z | sub | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → serve-c-tq4nc 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-07T20:06:37Z 후 1h05m54s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:33:14Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:81 |
| 10 | 2026-09-08T06:39:09Z | sub | `budget_declare` | `camp-26090814.cell-c-tq4nc.rerun` | 예산 선언(선언값 — 측정 아님) · floor_mib=101578 · weights_mib=7672 · kv_mib=11264 · overhead_mib=4096 · mem_total_mib=124610 · 라벨 다름: `camp-26090814.cell-c-tq4nc.rerun` — 같은 노드 · 이 셀 라벨(`c-tq4nc`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · budget_renew 3회 · 닫힘 budget_clear 2026-09-08T07:41:39Z(선언 후 1h02m30s) · measured_utc 2026-10-10T06:26:21Z 는 이 창 밖(측정 전) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:33:14Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-09.jsonl:150 |
| 11 | 2026-09-08T06:39:09Z | sub | `serve_start` | — | serve_start · model=qwen3-4b · tensor_parallel_size=1 · topology=single · session=serve-c-tq4nc-20260908T063909Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:33:14Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-09.jsonl:151 |
| 12 | 2026-09-08T06:39:09Z | sub | `budget_honored` | `camp-26090814.cell-c-tq4nc.rerun` | budget_honored · floor_mib=101578 · arm_ceiling_mib=98506 · remaining_s=7200 · 창 시작 2026-09-08T06:39:09Z 후 0m00s · 라벨 다름: `camp-26090814.cell-c-tq4nc.rerun` — 같은 노드 · 이 셀 라벨(`c-tq4nc`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:33:14Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:84 |
| 13 | 2026-09-08T06:39:10Z | sub | `budget_renew` | `camp-26090814.cell-c-tq4nc.rerun` | budget_renew · floor_mib=101578 · ttl_s=7200 · remaining_before_s=7199 · 창 시작 2026-09-08T06:39:09Z 후 0m01s · 라벨 다름: `camp-26090814.cell-c-tq4nc.rerun` — 같은 노드 · 이 셀 라벨(`c-tq4nc`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:33:14Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-09.jsonl:152 |
| 14 | 2026-09-08T07:09:10Z | sub | `budget_renew` | `camp-26090814.cell-c-tq4nc.rerun` | budget_renew · floor_mib=101578 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-09-08T06:39:09Z 후 30m01s · 라벨 다름: `camp-26090814.cell-c-tq4nc.rerun` — 같은 노드 · 이 셀 라벨(`c-tq4nc`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:33:14Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-09.jsonl:153 |
| 15 | 2026-09-08T07:39:10Z | sub | `budget_renew` | `camp-26090814.cell-c-tq4nc.rerun` | budget_renew · floor_mib=101578 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-09-08T06:39:09Z 후 1h00m01s · 라벨 다름: `camp-26090814.cell-c-tq4nc.rerun` — 같은 노드 · 이 셀 라벨(`c-tq4nc`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:33:14Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-09.jsonl:154 |
| 16 | 2026-09-08T07:41:39Z | sub | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → camp-26090814.cell-c-tq4nc.rerun 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-08T06:39:09Z 후 1h02m30s) · 라벨 다름: `camp-26090814.cell-c-tq4nc.rerun` — 같은 노드 · 이 셀 라벨(`c-tq4nc`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:33:14Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-09.jsonl:155 |
| 17 | 2026-09-08T07:41:39Z | sub | `serve_stop` | — | serve_stop · model=qwen3-4b · duration_s=3750 · session=serve-c-tq4nc-20260908T063909Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:33:14Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-09.jsonl:156 |
| 18 | 2026-09-08T07:41:39Z | sub | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → camp-26090814.cell-c-tq4nc.rerun 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-08T06:39:09Z 후 1h02m30s) · 라벨 다름: `camp-26090814.cell-c-tq4nc.rerun` — 같은 노드 · 이 셀 라벨(`c-tq4nc`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:33:14Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:85 |
| 19 | 2026-10-10T05:33:14Z | sub | `budget_declare` | `serve-c-tq4nc` | 예산 선언(선언값 — 측정 아님) · floor_mib=87240 · weights_mib=7672 · kv_mib=11264 · overhead_mib=18432 · mem_total_mib=124608 · 기동 창 2/2: budget_renew 2회 · 닫힘 budget_clear 2026-10-10T06:26:39Z(선언 후 53m25s) · measured_utc 2026-10-10T06:26:21Z 가 이 창 안(이 측정의 기동) | docs/logs/sub/events/2026-10.jsonl:239 |
| 20 | 2026-10-10T05:33:14Z | sub | `serve_start` | — | serve_start · model=qwen3-4b · tensor_parallel_size=1 · topology=single · session=serve-c-tq4nc-20261010T053314Z | docs/logs/sub/events/2026-10.jsonl:240 |
| 21 | 2026-10-10T05:33:14Z | sub | `budget_honored` | `serve-c-tq4nc` | budget_honored · floor_mib=87240 · arm_ceiling_mib=84168 · remaining_s=7200 · 창 시작 2026-10-10T05:33:14Z 후 0m00s | docs/logs/sub/events/watchdog.jsonl:532 |
| 22 | 2026-10-10T05:33:15Z | sub | `budget_renew` | `serve-c-tq4nc` | budget_renew · floor_mib=87240 · ttl_s=7200 · remaining_before_s=7199 · 창 시작 2026-10-10T05:33:14Z 후 0m01s | docs/logs/sub/events/2026-10.jsonl:241 |
| 23 | 2026-10-10T05:33:29Z | sub | `engine_log_window_start` | — | 캡처된 엔진 로그의 첫 시각(꼬리 캡처일 수 있다 — 엔진 시작 시각이 아닐 수 있다) · 선언 2026-10-10T05:33:14Z 후 0m15s · 시계: 컨테이너 시각을 UTC 로 읽어 측정 창 [2026-10-10T05:33:14Z, 2026-10-10T06:26:39Z] 안에 드는 것을 확인 | output/single/benchlog/sweep_c-tq4nc/lite_engine_c-tq4nc.log:36 |
| 24 | 2026-10-10T05:33:40Z | sub | `engine_prefetch_disabled` | — | 가중치 auto-prefetch 꺼짐 줄 ×1 — filesystem CIFS(vLLM 로그 문구) | output/single/benchlog/sweep_c-tq4nc/lite_engine_c-tq4nc.log:62 |
| 25 | 2026-10-10T05:34:25Z | sub | `engine_weights_loaded` | — | 가중치 적재 완료 줄 ×1(rank·단계별) · 최장 44.90s · 이 행 = 마지막 줄 | output/single/benchlog/sweep_c-tq4nc/lite_engine_c-tq4nc.log:69 |
| 26 | 2026-10-10T05:34:29Z | sub | `engine_kv_cache_sized` | — | GPU KV cache 232,992 tokens | output/single/benchlog/sweep_c-tq4nc/lite_engine_c-tq4nc.log:78 |
| 27 | 2026-10-10T05:35:31Z | sub | `engine_init_done` | — | 엔진 초기화(profile·KV 할당·warmup) 66.04s | output/single/benchlog/sweep_c-tq4nc/lite_engine_c-tq4nc.log:90 |
| 28 | 2026-10-10T05:35:33Z | sub | `server_starting` | — | API 서버 기동 — 이 측정 창 선언 2026-10-10T05:33:14Z 후 2m19s · 캡처 로그 첫 시각 2026-10-10T05:33:29Z 후 2m04s | output/single/benchlog/sweep_c-tq4nc/lite_engine_c-tq4nc.log:98 |
| 29 | 2026-10-10T06:03:15Z | sub | `budget_renew` | `serve-c-tq4nc` | budget_renew · floor_mib=87240 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-10T05:33:14Z 후 30m01s | docs/logs/sub/events/2026-10.jsonl:242 |
| 30 | 2026-10-10T06:26:39Z | sub | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → serve-c-tq4nc 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-10T05:33:14Z 후 53m25s) | docs/logs/sub/events/2026-10.jsonl:243 |
| 31 | 2026-10-10T06:26:39Z | sub | `serve_stop` | — | serve_stop · model=qwen3-4b · duration_s=3205 · session=serve-c-tq4nc-20261010T053314Z | docs/logs/sub/events/2026-10.jsonl:244 |
| 32 | 2026-10-10T06:26:39Z | sub | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → serve-c-tq4nc 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-10T05:33:14Z 후 53m25s) | docs/logs/sub/events/watchdog.jsonl:533 |

**기동 시도**(예산 선언 `budget_declare` 마다 한 시도 · 같은 노드의 이어지는 행과 선언 없는 노드(엔진 로그)의 그 시각 행을 묶었다 · 사살 · 트립 · 거부는 닫힘과 따로 센다 · `measurement` 행은 측정 기록이라 시도로 세지 않는다(위 표에만) — 해석은 저작자 몫)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 1 | sub | `serve-c-tq4nc` | 2026-10-10T05:33:14Z | 2회 · 첫 2026-10-10T05:33:15Z | 없음 | `budget_clear` 2026-10-10T06:26:39Z | 10 |

**캠페인 경계 이전 기동 2회**(캠페인 경계 2026-10-10T05:33:14Z 이전 — 이 셀 라벨로 묶인 1회 · 라벨 다름(같은 노드 · 창 밖 이 셀 행과 시간이 겹치는 다른 라벨 선언 창) 1회 · 어느 쪽도 이 셀의 시도로 세지 않는다 · 행 전문 = 01 §1.4)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 앞선-1 | sub | `serve-c-tq4nc` | 2026-09-07T20:06:36Z | 3회 · 첫 2026-09-07T20:06:37Z | 없음 | `budget_clear` 2026-09-07T21:12:30Z | 4 |
| 앞선-2 | sub | `camp-26090814.cell-c-tq4nc.rerun`(라벨 다름 — 같은 노드 · 이 셀 라벨 아님) | 2026-09-08T06:39:09Z | 3회 · 첫 2026-09-08T06:39:10Z | 없음 | `budget_clear` 2026-09-08T07:41:39Z | 4 |
<!-- /FACT:event_timeline -->

<!-- FACT:measurement_env -->
_측정 실행의 env 관측 없음 — 측정 엔진 로그에서 env echo 줄(`<KEY> set by environment to <VALUE>` 등)을 찾지 못했다(evidence.measurement_env_observed · 부재 ≠ 미설정)._
<!-- /FACT:measurement_env -->

> 이 절이 답하는 질문: 수신자가 이 zip 만으로 이 셀을 다시 띄우려면 어떤 순서로 무엇을 하고, 각 단계의 성공을 무엇으로 판정하며, 얼마나 기다려야 하는가?

**(a) 사전 준비.** 가중치는 컨테이너 안 `/app/models/Qwen/Qwen3/Qwen3-4B` 로 마운트된다(서빙 yaml `model:`). 호스트 쪽 경로는 compose `.env` 의 `NAS_MODEL_PATH=<manifest.nas_model_path>` 로 주입한다. 체크포인트는 7.49 GiB 이고 로드 경로의 파일시스템은 CIFS 였다(L61) — 엔진이 자동 prefetch 를 끄고 `Loading weights took 44.90 seconds`(L69)였다. 필수 env 의 자리: `.env.c-tq4nc`(포트 8000 · served-model-name `qwen3-4b` · 이미지 태그 · `CONFIG_FILE=c-tq4nc`) — 측정 실행의 env echo 관측은 없으므로(위 '측정 실행의 env 관측' 사실 블록) 측정 당시 env 값은 이 파일 사본이 정본이다.

**(b) 빌드.** 이미지 `easy-vllm:0.26.0-cu132-aarch64-source`(digest 는 0.4 표)는 이 호스트에 이미 있었고 이번 실행은 짓지 않았다(testlog §2 "로컬 보유"). 위 명령 블록은 docker history 의 build-arg 로 **재구성한 것**이며 실행 원문이 아니다. 소요: docker history 의 층 창이 876시간 27분 13초지만 이는 캐시 재사용 층을 포함한 CreatedAt 범위이지 빌드 소요가 아니다 — 빌드 소요는 미관측.

**(c) 기동과 성공 판정.** 기동은 1회였다(위 기동 시도 표). 원장: 예산 선언 05:33:14Z → API 서버 기동 줄 2m19s 뒤(이벤트 28번 행). testlog 는 READY 140 s 로 적었다. 엔진 로그의 첫 요청은 05:35:53(L139)이다. 위 표 serve·bench 행의 소요는 **최종 재실행 스윕 기준**이며, 서버 기동과 최종 스윕 첫 측정(06:05:11Z) 사이에 하네스 제한으로 끊긴 첫 스윕(W4)의 요청이 있었다(소요 출처 칸의 '그 전 요청 202건'). 엔진 로그의 서명: `Using TURBOQUANT attention backend`(L60), `GPU KV cache size: 232,992 tokens`(L78), 엔진 초기화 66.04 s(L90). 성공 판정은 위 표대로 health 200 + 추론 1회이며 근거 파일은 `post_health_c-tq4nc.json` · `guidellm_c-tq4nc.json` 이다. **정상인데 무서워 보이는 줄**: `NVIDIA Driver was not detected`(L30 — 직후 적재 성공, 원인 미조사), `Unknown vLLM environment variable`(W1), TurboQuant FA 경고(W2), `No FlashInfer autotune cache entries found`(L83 — 기본 전술로 대체), 첫 요청 구간(05:36:01)의 Triton JIT 경고(L144 — Q6; 최종 lite cold 측정은 그보다 늦다).

**(d) 호스트 메모리 안전 예산 선언.** 이 노드는 `--overhead-mib 18432` 로 선언했고(testlog §2), 원장의 선언 행은 `floor_mib=87240 · weights_mib=7672 · kv_mib=11264 · overhead_mib=18432 · mem_total_mib=124608`(이벤트 19번 행)이다. 이는 **선언값이며 측정이 아니다** — 같은 기동에서 엔진이 실측한 쪽은 `Model loading took 7.56 GiB`(L70)와 예약 KV 11.0 GiB(L77)다. 지시가 18432 를 기본값으로 준 이유는 "10-01 값을 알 수 없어"(testlog §2)였고, 이 값의 산출 근거는 계보에 없다. 사살·트립은 없었다.

**(e) 측정 명령.** 위 bench 단계의 재구성 명령은 lite 레그(`vllm bench serve`)이며, 풀 레그는 guidellm 이다. 도구가 정한 인자는 03 §3.4 의 도구 원문 발췌에 있다. 형상은 입력 1024 · 출력 256 · n=16 · 워밍업 2 · 반복 3(testlog §5).

**(f) 엔진이 기동 중 고른 런타임 경로.** attention 은 FlashAttention 2 + TURBOQUANT 백엔드(L59 · L60), Model Runner V2 사용(`Using V2 Model Runner`, L56), CUDA 그래프 PIECEWISE·FULL 캡처 `Graph capturing finished in 31 secs, took 0.43 GiB`(L88), top-p/top-k 샘플링은 FlashInfer(L71), torch.compile 은 AOT 캐시를 읽어 0.49 s(L75). 통신 백엔드는 단일 GPU 라 해당 없음. 가중치 auto-prefetch 는 CIFS 이유로 꺼짐(L62). 서버 경고 `Default vLLM sampling parameters have been overridden by the model's generation_config.json`(L96)은 temperature 0.6 · top_k 20 · top_p 0.95 를 기본 샘플링으로 삼는다는 뜻이다.

기동 시도 표에 시도는 1회뿐이며, 캠페인 경계 이전 기동 2회(이 셀 라벨 1 · 다른 라벨 1)는 이 셀의 시도로 세지 않는다(표 참조).
> [원문] lite_engine_c-tq4nc.log §L60-62
> (EngineCore pid=240) INFO 10-10 05:33:39 [cuda.py:482] Using TURBOQUANT attention backend out of potential backends: ['TURBOQUANT'].
> (EngineCore pid=240) INFO 10-10 05:33:40 [weight_utils.py:869] Filesystem type for checkpoints: CIFS. Checkpoint size: 7.49 GiB. Available RAM: 105.82 GiB.
> (EngineCore pid=240) INFO 10-10 05:33:40 [weight_utils.py:892] Auto-prefetch is disabled because the filesystem (CIFS) is not a recognized network FS (NFS/Lustre). If you want to force prefetching, start vLLM with --safetensors-load-strategy=prefetch.

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
체크포인트는 HF `Qwen/Qwen3-4B` 이며 revision 은 0.2 사실 블록에 있다. 이 계보의 서빙은 **관리되는 읽기 전용 모델 저장소**에서 가져온 파일을 마운트했고, 이 발행의 어느 단계도 가중치를 내려받지 않았다. 다운로드가 필요하면 사람의 승인 게이트를 거친 뒤 `huggingface-cli download Qwen/Qwen3-4B --revision <0.2 의 revision> --local-dir <manifest.nas_model_path>/Qwen/Qwen3/Qwen3-4B` 꼴이 될 것이다 — **추론 — 실행 검증 ✗**(이 명령은 이 노드에서 실행된 적이 없다). 크기는 엔진이 7.49 GiB 로 보고했다(L61). 단일 노드이므로 노드마다 필요한 복제는 없다.

### PLE · 기타 스테이징 생성
해당 없음 — 이 셀은 PLE 를 쓰지 않는다(`ple_mode: none` 선언 · 모델 config 에 PLE 키 부재 관측은 0.9 후보 표의 출처 칸). 다른 스테이징 산출물도 없다. 위치 조건으로는 가중치가 CIFS 위에 있었고 로드에 44.90 s 가 걸렸다는 관측뿐이다(L61 · L69).

### 네트워크가 필요한 단계
- 베이스 이미지 `nvcr.io/nvidia/pytorch:26.05-py3` pull 과 vLLM 소스 `https://github.com/vllm-project/vllm.git` 의 `v0.26.0` 체크아웃, requirements 의 pip 설치 — 이미지를 직접 지을 때만 필요하다(`Dockerfile.source-build` · build-arg). 오프라인 대체: 이미 지어진 이미지를 가져온다.
- 풀 벤치 도구 이미지 `ghcr.io/vllm-project/guidellm:v0.7.3` — 측정 때 실행기는 `HF_HUB_OFFLINE=1` 로 돌며(`run_bench.sh` L397) 최신 버전 해소는 벤치 도중이 아니라 스테이징 평면의 일이라고 주석이 적는다. 오프라인 대체: 이미지를 미리 받아 둔다. 출처: sweep_index 의 도구 이미지 기록과 03 §3.4 도구 원문.
