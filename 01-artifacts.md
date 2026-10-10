# 01 · 산출물 — 무엇이 실제로 쓰였고, 어떻게 다시 띄우는가

## 1.1 적용 판정 표
> 기계가 적용 증거(빌드 원장 · 원장이 없으면 라벨된 재구성) × 파일 × 선언을 대사한 결과다. `artifacts/` 에는 **적용된 것만** 실렸다 — 빌드 때 스스로 skip 된 패치와 쓰이지 않은 평면의 파일은 표에만 남고 싣지 않는다.

<!-- FACT:slots -->
| 슬롯 | 실림 | 파일 | 신호 | 적용 증거 | 사유 |
|---|---|---|---|---|---|
| `build_patch_post` | 안 실림 | — | 1-signal | build-ledger:docker-image(main-local):sha256:123c331f7a5266b069fcb8b9b28009e862d5662f50dcd12e9321fa08937c1c16:/opt/easy-vllm/build_ledger.json | 적용 판정 없음 — 원장 관측(기계 파생) |
| `build_patch_pre` | 안 실림 | — | 1-signal | build-ledger:docker-image(main-local):sha256:123c331f7a5266b069fcb8b9b28009e862d5662f50dcd12e9321fa08937c1c16:/opt/easy-vllm/build_ledger.json | 적용 판정 없음 — 원장 관측(기계 파생) |
| `build_recipe` | 실림 | `artifacts/build_recipe/Dockerfile.source-build` · `artifacts/build_recipe/requirements.txt` | 3-signal | build-ledger:build-ledger.dockerfile | (a) NGC PyTorch 26.07 베이스(torch 2.13.0a0 · CUDA 13.3.1) 위에서 vLLM `v0.30.0` 을 소스 빌드하는 stock 스켈레톤이다(build-arg `TORCH_CUDA_ARCH=12.1a` · `SM12X_PORT=0` · `SRC_DEPS_AUTHORITY=0` — 01 §1.4 build 단계). requirements 는 v0.30.0 wheel METADATA 의 Requires-Dist 를 옮긴 것이다(파일 머리 주석). (b) 이 모델 고유의 요구가 아니라 0.30.0 의 torch 핀이 NGC 베이스의 torch 빌드와 wheel ABI 가 맞지 않아 source-build 트랙이 된 공유 이미지 레시피다(plan_26100107 §3 빌드 트랙… |
| `compose` | 실림 | `artifacts/compose/.env.template` · `artifacts/compose/docker-compose.yaml` | 2-signal | file:output/single/docker-compose.yaml | (a) 단일 노드 서빙 경로다 — NAS 모델 루트를 `/app/models` 에 읽기 전용으로, 설정 디렉터리를 `/app/configs` 에, vLLM · FlashInfer 캐시를 `./cache/` 에 마운트하고 러너 `${CONFIG_FILE}.sh` 를 실행한다. env 형상의 경로 값은 manifest 필드 자리표시다(`<manifest.nas_model_path>` 등). (b) 재현 경로 입력이며 W id 와 무관하다. (c) 측정 당시 개정 `f9b4dae0add0` 과 바이트가 같다(1.1 · 측정 뒤 커밋 0). 이 셀은 캐시 마운트 덕에 엔진이 AOT 컴파일 산출을 다시 읽었다(엔진 로그 L77-79 · torch.compile 0.79 s) — 캐시가 빈 첫 기동은 더 오래 걸린다… |
| `fork_pin` | 안 실림 | — | 1-signal | none:.claude/policies/arch_variant_ledger.json | 셀 env 에 VARIANT 줄 없음 = stock(workflow.md §변종 좌표의 거처)(기계 파생) |
| `runtime_patch` | 안 실림 | — | 1-signal | none:output/single/configs/granite42-30b-v030_patch.py | output/single/configs/granite42-30b-v030_patch.py 없음 — arm_patch.sh 는 no-op(런타임 패치 불해당)(기계 파생) |
| `triplet` | 실림 | `artifacts/triplet/.env.granite42-30b-v030` · `artifacts/triplet/granite42-30b-v030.sh` · `artifacts/triplet/granite42-30b-v030.yaml` | 3-signal | file:output/single/configs/granite42-30b-v030.yaml | (a) yaml = 서빙 노브 6개(1.3) · 러너 = 추론 파서 플러그인 `granite_thinking_parser`(모델 폴더 동봉 파일을 `--reasoning-parser-plugin` 으로 적재) · 툴 파서 `qwen3_coder` · auto tool choice · env = 이미지 태그 · 컨테이너 · 서빙 모델 이름. (b) KV 클램프 값이 W1 · W2 의 해소다. 파서 인자는 빼면 깨지는지 시험한 기록이 없다(모델 카드 권장으로 넣었다 — yaml 머리 L7). (c) 이 측정 엔진 로그의 non-default args 에 `reasoning_parser: granite_thinking_parser` · `reasoning_parser_plugin` · `kv_cache_memo… |

**레시피 경고 · 실린 리비전**(기계 관측 — 수신자 빌드가 여기서 갈라질 수 있다)

- `build_recipe` `output/single/Dockerfile.source-build` 실린 리비전: 리비전 미특정 · 선택 방법 `worktree(측정 이미지 docker history 의 RUN·COPY 와 전수 일치)` · docker history 대조 17/17 · 실린 바이트 `worktree` · 쓰인 바이트임을 관측으로 확인 예
- `build_recipe` `output/single/requirements.txt` 실린 리비전: 리비전 미특정 · 선택 방법 `worktree(빌드 원장 sha256 일치 — 쓰인 바이트 관측 확인 = 예)` · docker history 대조 없음 · 실린 바이트 `worktree` · 쓰인 바이트임을 관측으로 확인 예
- `compose` `output/single/docker-compose.yaml` 실린 리비전: 커밋 `f9b4dae0add0` · 선택 방법 `worktree(git log -1 --before=2026-10-10T07:14:23Z -- output/single/docker-compose.yaml → f9b4dae0add0 · 작업트리 mtime 2026-10-10T08:55:08Z > 측정(sweep_index.generated_utc)이지만 바이트가 그 개정과 같다 · 측정 뒤 커밋 0 · 작업트리 = HEAD)` · docker history 대조 없음 · 실린 바이트 `worktree` · 근거 `git-rev-before-measurement` · 쓰인 바이트임을 관측으로 확인 아니오

**싣지 않은 파일**(슬롯별 사유 — 빌드 때 skip 된 패치는 아래 적용 집합 표에 있다)

- `build_patch_post` known-non-patch 1건: `.gitkeep`
- `build_patch_pre` known-non-patch 1건: `.gitkeep`
- `compose` `arm_patch.sh` — `not-armed(output/single/configs/granite42-30b-v030_patch.py 없음 — arm_patch.sh 는 no-op)`

**파일별 검증 · 관련성**(`generated-unverified` = 실행 검증되지 않은 생성물 — 파일 첫 줄 경고 · PAYLOAD 파일 기록과 같은 표시 · 관련성 = ① 패치 선언 대상 × 모델 config ② 엔진 로그 발화 서명 — 둘 다 관측일 때만 required/inactive-inferred)

| 슬롯 | 파일 | 검증 | 검증 근거 | 관련성 | 관련성 근거 |
|---|---|---|---|---|---|
| `build_recipe` | `artifacts/build_recipe/Dockerfile.source-build` | `verified` · cell-run | 쓰인 레시피(build-ledger.dockerfile) — 개정 선택 slots.build_recipe.evidence.selected_revisions | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `build_recipe` | `artifacts/build_recipe/requirements.txt` | `verified` · cell-run | 쓰인 레시피(build-ledger.dockerfile) — 개정 선택 slots.build_recipe.evidence.selected_revisions | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/.env.template` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/docker-compose.yaml` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/.env.granite42-30b-v030` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/granite42-30b-v030.sh` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/granite42-30b-v030.yaml` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
<!-- /FACT:slots -->

<!-- FACT:applied_set -->
**적용 집합** — 상태 `observed` · 출처 `docker-image(main-local):sha256:123c331f7a5266b069fcb8b9b28009e862d5662f50dcd12e9321fa08937c1c16:/opt/easy-vllm/build_ledger.json`

| 단계 | 파일 | 결과 | 결과 출처 | 사유 | 실림 | model-trigger |
|---|---|---|---|---|---|---|
| inline | `strip-hoist` | skipped | build-ledger | register_opaque_type accepts hoist / opaque_object 부재 | 안 실림 | — |

**결과 출처 범례**

- `build-ledger` — 이미지 안 빌드 원장이 적용 결과를 기록했다(관측)

**판정 근거**(패치별 — artifacts 가 판정에 쓴 관측 · 스크립트 바이트 정체 = 실린 바이트가 측정 이미지에 구워진 바이트인가)

| 파일 | 스크립트 바이트 정체 | 이미지 사본 sha256 | 적용 표지(상태 찾음/선언) | 정적 규칙 | 원장 분류 |
|---|---|---|---|---|---|
| `strip-hoist` | — | — | — | — | status-file |

**원장 탐침**(순위: attestation → 메인 로컬 이미지 → 재구성)

| 순위 | 결과 | 대상 |
|---|---|---|
| attestation | absent | output/single/benchlog/attestation_granite42-30b-v030.json(부재) |
| local-image | observed | sha256:123c331f7a5266b069fcb8b9b28009e862d5662f50dcd12e9321fa08937c1c16 |
<!-- /FACT:applied_set -->

## 1.2 슬롯별 적용 사유
> 이 절이 답하는 질문: 실린 산출물 하나하나는 무엇을 고치며, 왜 이 셀에 필요했고, 이 셀에서 실제로 발화했으며, 언제 불필요해지는가?

### build_recipe — `Dockerfile.source-build` · `requirements.txt`

(a) NGC PyTorch 26.07 베이스(torch 2.13.0a0 · CUDA 13.3.1) 위에서 vLLM `v0.30.0` 을 소스 빌드하는 stock 스켈레톤이다(build-arg `TORCH_CUDA_ARCH=12.1a` · `SM12X_PORT=0` · `SRC_DEPS_AUTHORITY=0` — 01 §1.4 build 단계). requirements 는 v0.30.0 wheel METADATA 의 Requires-Dist 를 옮긴 것이다(파일 머리 주석). (b) 이 모델 고유의 요구가 아니라 0.30.0 의 torch 핀이 NGC 베이스의 torch 빌드와 wheel ABI 가 맞지 않아 source-build 트랙이 된 공유 이미지 레시피다(plan_26100107 §3 빌드 트랙 행) — 빼면 다시 나오는 이 셀의 벽(W id)은 없다. (c) 측정 이미지의 docker history 와 RUN · COPY 17/17 일치(1.1). 레시피 안의 인라인 패치 `strip-hoist` 는 이 이미지에서 **skipped** 로 원장에 기록됐다(1.1 적용 집합) — 원장 사유 `register_opaque_type accepts hoist / opaque_object 부재` 는 두 갈래(베이스 torch 의 `register_opaque_type` 가 이미 `hoist` 를 받음 · 또는 `opaque_object` import 실패)를 한 문자열로 묶은 것이고(Dockerfile L91-96), 어느 갈래였는지는 관측되지 않았다. (d) 트립와이어가 맞지 않을 때 이 인라인 패치는 빌드를 멈추지 않고 skip 하며 상태 파일만 쓴다:

> [원문] Dockerfile.source-build §L112-114
> else:
>     print("[patch] strip-hoist skipped (register_opaque_type accepts hoist / opaque_object 부재)")
>     status = "skipped\tregister_opaque_type accepts hoist / opaque_object 부재"

이 레시피가 불필요해지는 조건은 대상 vLLM 이 선언한 torch 가 NGC 베이스의 torch 와 접두 일치해 prebuilt wheel 트랙으로 갈 수 있을 때다(트랙 판정은 `upstream-version-watch` 소유).

### compose — `docker-compose.yaml` · `.env.template`

(a) 단일 노드 서빙 경로다 — NAS 모델 루트를 `/app/models` 에 읽기 전용으로, 설정 디렉터리를 `/app/configs` 에, vLLM · FlashInfer 캐시를 `./cache/` 에 마운트하고 러너 `${CONFIG_FILE}.sh` 를 실행한다. env 형상의 경로 값은 manifest 필드 자리표시다(`<manifest.nas_model_path>` 등). (b) 재현 경로 입력이며 W id 와 무관하다. (c) 측정 당시 개정 `f9b4dae0add0` 과 바이트가 같다(1.1 · 측정 뒤 커밋 0). 이 셀은 캐시 마운트 덕에 엔진이 AOT 컴파일 산출을 다시 읽었다(엔진 로그 L77-79 · torch.compile 0.79 s) — 캐시가 빈 첫 기동은 더 오래 걸린다는 것은 추론이다. (d) 폐기 조건 없음(경로 형상).

### triplet — `granite42-30b-v030.yaml` · `granite42-30b-v030.sh` · `.env.granite42-30b-v030`

(a) yaml = 서빙 노브 6개(1.3) · 러너 = 추론 파서 플러그인 `granite_thinking_parser`(모델 폴더 동봉 파일을 `--reasoning-parser-plugin` 으로 적재) · 툴 파서 `qwen3_coder` · auto tool choice · env = 이미지 태그 · 컨테이너 · 서빙 모델 이름. (b) KV 클램프 값이 W1 · W2 의 해소다. 파서 인자는 빼면 깨지는지 시험한 기록이 없다(모델 카드 권장으로 넣었다 — yaml 머리 L7). (c) 이 측정 엔진 로그의 non-default args 에 `reasoning_parser: granite_thinking_parser` · `reasoning_parser_plugin` · `kv_cache_memory_bytes: 35433480192` 가 그대로 찍혔다(L42) — 설정이 엔진에 도달했다. 파서가 응답을 실제로 분리했는지는 이 측정 계보에 관측이 없다(GuideLLM 은 처리량만 잰다). (d) 모델 · vLLM 조합이 바뀌면 트리플렛은 다시 세운다(carry-forward 금지).

### 싣지 않은 슬롯

- `build_patch_pre` · `build_patch_post` — 두 디렉터리 모두 `.gitkeep` 뿐이고 원장에 적용 판정이 없다(1.1). 이 셀은 빌드 패치를 쓰지 않았다. plan_26100107 D4 가 말한 "post 패치 10~40" 은 이 측정 이미지의 원장에 기록되지 않았다 — 계보 밖 기록(testlog_26100117 §1 C2 · §2)에 따르면 이 이미지는 빈 패치 슬롯으로 다시 지은 single 렌더 이미지다.
- `fork_pin` — 셀 env 에 `VARIANT` 줄이 없어 stock 이다.
- `runtime_patch` — `granite42-30b-v030_patch.py` 가 없어 `arm_patch.sh` 는 no-op 이다.

## 1.3 값의 지위표
> 서빙 설정의 노브 **전부**와 기계가 모은 후보 지위다(lockset 출처 · yaml 주석 · 엔진 기본값). 최종 지위는 아래 `kind: value-status` 블록이 정한다 — 표의 노브마다 블록이 정확히 하나 있어야 린터를 통과한다.

<!-- FACT:value_status -->
서빙 노브 6개 — 노브마다 `kind: value-status` 블록이 **정확히 1개** 있어야 한다(노브·값은 이 표의 글자 그대로 · 값 칸의 `\|` 는 `|` 로 적는다).

| 노브 | 값 | 후보 지위 | 후보 근거 | yaml 주석 | 출처 |
|---|---|---|---|---|---|
| `dtype` | bfloat16 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `gpu-memory-utilization` | 0.80 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `max-model-len` | 131072 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `kv-cache-memory-bytes` | 35433480192 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `kv-cache-dtype` | auto | engine-default | 값 `auto` = 엔진이 고른다 | — | serving-yaml 값 토큰 |
| `max-num-seqs` | 8 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
<!-- /FACT:value_status -->

> 이 절이 답하는 질문: 각 서빙 노브의 값은 이 셀에서 어떤 지위인가 — 조정됐나, 승계됐나, 음성대조로 일부러 둔 것인가, 엔진 기본값인가, 필요조건인가?

```hint-event
id: V1
kind: value-status
노브: dtype
값: bfloat16
지위: inherited
근거: 체크포인트 torch_dtype(bfloat16)을 그대로 옮긴 값 · 이 셀에서 조정 기록 없음
출처: [plan_26100107_vllm030_bump_서브지침배달_멀티싱글_E2E캠페인 §2, granite42-30b-v030.yaml]
```

```hint-event
id: V2
kind: value-status
노브: gpu-memory-utilization
값: 0.80
지위: inherited
근거: 2026-10-01 셀 저작에서 정한 천장을 승계 · 조정 기록 없음 · kv-cache-memory-bytes 지정으로 엔진이 KV 산정에 이 값을 쓰지 않는다고 로그에 명시(L81)
출처: [plan_26100107_vllm030_bump_서브지침배달_멀티싱글_E2E캠페인 §Phase S, output/single/benchlog/sweep_granite42-30b-v030/lite_engine_granite42-30b-v030.log]
```

```hint-event
id: V3
kind: value-status
노브: max-model-len
값: 131072
지위: inherited
근거: 모델 네이티브 컨텍스트(max_position 131072)를 그대로 씀 · 줄이거나 늘려 잰 기록 없음
출처: [plan_26100107_vllm030_bump_서브지침배달_멀티싱글_E2E캠페인 §3, granite42-30b-v030.yaml]
```

```hint-event
id: V4
kind: value-status
노브: kv-cache-memory-bytes
값: 35433480192
지위: tuned
근거: 2026-10-01 에 16GiB(거부 W1) → 32GiB(16 토큰 미달 거부 W2) → 33.0 GiB 로 바꿔 가며 기동 · 엔진 보고 135168 토큰 · 131072 기준 1.03x
출처: [granite42-30b-v030.yaml, output/single/benchlog/sweep_granite42-30b-v030/lite_engine_granite42-30b-v030.log]
```

```hint-event
id: V5
kind: value-status
노브: kv-cache-dtype
값: auto
지위: engine-default
근거: auto = 엔진이 모델 dtype 을 따른다 · 측정 도구가 엔진 로그에서 auto 를 관측(kv_cache_dtype_source measured)
출처: [bench_report_26101016_granite-4.2-30b_GB10_0.30.0 §측정 환경 스냅샷]
```

```hint-event
id: V6
kind: value-status
노브: max-num-seqs
값: 8
지위: inherited
근거: 값의 출처를 적은 계보 문서 없음 · 이 셀에서 조정 기록 없음 · 측정에서는 프리셋 레벨 16 을 거르는 상한으로만 작용
출처: [testlog_26101016_벤치_knob_프리셋_E2E_6셀_판정 §2.1, granite42-30b-v030.yaml]
```

복사하면 위험한 값은 둘이다. kv-cache-memory-bytes(V4)는 이 모델 · 이 컨텍스트에서 1시퀀스 하한에 블록 여유를 더한 값이라 모델 · max-model-len · 블록 크기가 바뀌면 다시 도출해야 하고, 통합메모리 노드에서는 이 값이 호스트 예산 바닥과 벤치 몫 상한까지 정한다(W3). gpu-memory-utilization(V2)은 KV 클램프가 있는 한 엔진이 KV 산정에 쓰지 않으므로 이 값을 바꿔 메모리를 조절하려 하면 아무 일도 일어나지 않는다. max-num-seqs(V6)는 근거 없는 승계값이며 긴 요청 동시성에서는 KV(1.03x)가 먼저 막힌다(02 Q4). 이 셀에 declared-requirement 지위의 값은 없다 — 빼거나 바꿔서 실패를 관측한 노브는 KV 클램프 하나이고, 그것은 값을 바꿔 가며 잰 tuned 로 적었다.

## 1.4 재현 절차
> 셀 실행 기록에서 기계가 파생한 명령 순서와 단계별 소요다. 소요가 `미관측` 인 단계는 시각 기록이 없다는 뜻이다(0 이 아니다). 그 아래는 블랙박스 이벤트 원장에서 이 셀의 행을 기계가 뽑아 기동 시도로 묶은 것이고, 마지막은 측정한 실행의 엔진 로그가 echo 한 env 다(측정 당시 값의 정본).

<!-- FACT:reproduce -->
| 순서 | 단계 | 성공 판정 | 소요 | 관측 구간(UTC) | 소요 출처 | 명령 출처 |
|---|---|---|---|---|---|---|
| 1 | build | 빌드 OK · 이미지 실재 | 층 창 36분 33초(docker history 첫 층 → 끝 층 CreatedAt · 캐시 재사용 층 포함 — 빌드 소요 아님) | 2026-10-01T07:38:01Z → 2026-10-01T08:14:34Z (layer-window) | docker history --no-trunc --format '{{json .}}' sha256:123c331f7a5266b069fcb8b9b28009e862d5662f50dcd12e9321fa08937c1c16 · 우리 Dockerfile 층 30개의 CreatedAt 첫 층 → 끝 층(캐시 재사용 층은 이전 빌드 시각 — 빌드 소요 아님) | reconstructed(build-ledger build_args + compose build) |
| 2 | serve | health 200 + 추론 1회(스모크 판정) | ≤ 8분 1초(상한) | 2026-10-10T05:07:56Z → 2026-10-10T05:15:57Z (upper) | docs/logs/main/events/2026-10.jsonl budget_declare(label serve-granite42-30b-v030) → 첫 측정 시각(output/single/benchlog/sweep_granite42-30b-v030/lite_raw_granite42-30b-v030.json measured_utc(호스트 date -u · lite_bench.sh 가 cold 레그 직전에 찍는 측정 시작)) | derived(스모크 사용법) |
| 3 | bench | 리포트 발행(+PASS 면 인증서) | 1시간 58분 26초 | 2026-10-10T05:15:57Z → 2026-10-10T07:14:23Z (exact) | output/single/benchlog/sweep_granite42-30b-v030/lite_raw_granite42-30b-v030.json measured_utc(호스트 date -u · lite_bench.sh 가 cold 레그 직전에 찍는 측정 시작) → sweep_index.generated_utc(호스트 UTC · 두 끝 같은 시계) | reconstructed(bench json) |

**빌드 층 시각**(관측 · 소요가 아니라 층 창 — 캐시 재사용 층은 이전 빌드의 시각을 가진다)

- `build` 이미지 층 CreatedAt(docker history): 2026-10-01T07:38:01Z → 2026-10-01T08:14:34Z · 층 30개 · 출처 `docker history --no-trunc --format '{{json .}}' sha256:123c331f7a5266b069fcb8b9b28009e862d5662f50dcd12e9321fa08937c1c16` — 우리 Dockerfile 층의 CreatedAt 범위 — 캐시 재사용 층은 이전 빌드 시각을 가진다(소요 아님)

**단계별 명령**(기계 파생 — `명령 출처` 가 `derived(…)` 면 사용법에서 파생 · `reconstructed(…)` 면 괄호 안 관측(docker history build-arg · compose build · bench JSON 필드)에서 재구성한 명령이다. 어느 쪽도 실행 원문(로그에 남은 줄)은 아니다)

**1. build**

```sh
# 실린 artifacts/build_recipe/Dockerfile.source-build(이미지를 지은 개정: worktree)를 output/single/Dockerfile.source-build 자리에 두고 컨텍스트에서 짓는다.
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
# 동등 경로(compose): docker compose -f output/single/docker-compose.yaml --env-file output/single/envs/.env.granite42-30b-v030 build
```

**2. serve**

```sh
bash .claude/skills/upstream-version-watch/scripts/single_serve_up.sh granite42-30b-v030
```

**3. bench**

```sh
# bench JSON 필드로 복원: backend · model_id→--model · tokenizer_id→--tokenizer · num_prompts · max_concurrency · request_rate · burstiness · 요청당 입출력 길이 = total_*_tokens ÷ completed(나누어떨어질 때만 — 평균이다 · 요청별 길이는 JSON 에 없다) · 나누어떨어지면 `--dataset-name random` 으로 추정했다(측정 도구가 그 꼴로 부른다 — 아래 도구 원문).
# JSON 에 없는 인자(--endpoint · --base-url · --ignore-eos · --num-warmups · --temperature · --seed · --random-range-ratio · --trust-remote-code)는 측정 도구가 정했다 — level_* = run_bench.sh · lite_cold/lite_warm = lite_bench.sh(.claude/skills/adversarial-benchmark/scripts/) · 셀 스윕 순서는 sweep_bench.sh.
# output/single/benchlog/sweep_granite42-30b-v030/lite_cold_granite42-30b-v030.json (date 20261010-051640) · ⚠ --random-input-len/--random-output-len = total ÷ completed 평균(chat 템플릿 포함 실측 토큰 · 측정 도구 원문 인자 미관측 — lite_bench.sh 원문 인자와 다를 수 있다)
vllm bench serve --backend openai-chat --model granite-4.2-30b --tokenizer /app/models/IBM/Granite-4.2/granite-4.2-30b --dataset-name random --random-input-len 527 --random-output-len 128 --num-prompts 1 --max-concurrency 1 --request-rate inf
# output/single/benchlog/sweep_granite42-30b-v030/lite_warm_granite42-30b-v030.json (date 20261010-051858) · ⚠ --random-input-len/--random-output-len = total ÷ completed 평균(chat 템플릿 포함 실측 토큰 · 측정 도구 원문 인자 미관측 — lite_bench.sh 원문 인자와 다를 수 있다)
vllm bench serve --backend openai-chat --model granite-4.2-30b --tokenizer /app/models/IBM/Granite-4.2/granite-4.2-30b --dataset-name random --random-input-len 527 --random-output-len 128 --num-prompts 3 --max-concurrency 1 --request-rate inf
```

**슬롯 → 빌드 컨텍스트 매핑**(빌드 컨텍스트 = `output/<토폴로지>/` · 원천 = 실린 Dockerfile 의 COPY · compose 의 env_file · volumes · command — `—` 는 빌드 컨텍스트 입력이 아니다)

| zip 경로 | 빌드 컨텍스트 자리 | 근거 |
|---|---|---|
| `artifacts/triplet/.env.granite42-30b-v030` | `envs/.env.granite42-30b-v030` | compose env_file envs/.env.${CONFIG_FILE}(셀 env) |
| `artifacts/triplet/granite42-30b-v030.sh` | `configs/granite42-30b-v030.sh` | compose volumes ./configs:/app/configs |
| `artifacts/triplet/granite42-30b-v030.yaml` | `configs/granite42-30b-v030.yaml` | compose volumes ./configs:/app/configs |
| `artifacts/build_recipe/Dockerfile.source-build` | `Dockerfile.source-build` | compose build.dockerfile Dockerfile.source-build(리터럴) · build.context . |
| `artifacts/build_recipe/requirements.txt` | `requirements.txt` | Dockerfile.source-build COPY requirements.txt |
| `artifacts/compose/.env.template` | `.env` | compose 변수치환 프로젝트 .env(${…} 참조) |
| `artifacts/compose/docker-compose.yaml` | `docker-compose.yaml` | compose 파일(빌드 컨텍스트 루트) |

**기동 성공 판정(관측 · 발행 자격)**: health 200 = true · 추론 1회 = true · 근거 `output/single/benchlog/sweep_granite42-30b-v030/level_01/post_health_granite42-30b-v030.json` · `output/single/benchlog/sweep_granite42-30b-v030/level_01/guidellm_granite42-30b-v030.json`
<!-- /FACT:reproduce -->

<!-- FACT:event_timeline -->
**블랙박스 이벤트 57행**(원장에서 이 셀 label·config 에 맞는 행만 · 시각순 · 기계 발췌 · 이번 캠페인 32행 · 캠페인 선언 전 · 같은 셀 기동 8행(이 셀의 시도로 센다) · 같은 셀 이름 · 앞선 캠페인 17행(내용 칸에 표기))

> **캠페인 경계** 2026-10-10T05:07:56Z(`camp-26101017-knob-e2e-single` · evidence.event_campaign_boundary): campaigns/camp-26101017-knob-e2e-single/campaign.yaml declared_utc 2026-10-10T08:04:57Z · 이 발행이 묶인 측정을 서빙한 budget_declare 창 시작 2026-10-10T05:07:56Z → 경계 = 앞선 값 2026-10-10T05:07:56Z(이전 행 = 같은 셀 이름 · 앞선 캠페인) · 단 이전 기동 중 1건은 캠페인 선언 전 · 같은 셀 기동(세션 id 가 이 셀 기록에 관측: serve-granite42-30b-v030-20261010T045514Z = docs/simlog/26101012_벤치_knob_프리셋_E2E/a_serve.log:15)

> **원장 관측 범위**(노드별 첫 행 → 마지막 행 · evidence.event_ledger_spans): main = 2026-07-31T03:27:43Z → 2026-10-10T07:46:20Z(`docs/logs/main/events/2026-07.jsonl` · `docs/logs/main/events/2026-08.jsonl` · `docs/logs/main/events/2026-09.jsonl` · `docs/logs/main/events/2026-10.jsonl` · `docs/logs/main/events/thermal.jsonl` · `docs/logs/main/events/watchdog.jsonl`)

| # | UTC | 노드 | kind | label | 내용 | 출처 |
|---|---|---|---|---|---|---|
| 1 | 2026-10-01T01:42:26Z | main | `serve_start` | — | serve_start · model=granite-4.2-30b · tensor_parallel_size=1 · topology=single · session=serve-granite42-30b-v030-20261001T014226Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:07:56Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:19 |
| 2 | 2026-10-01T01:49:52Z | main | `serve_stop` | — | serve_stop · model=granite-4.2-30b · duration_s=446 · session=serve-granite42-30b-v030-20261001T014226Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:07:56Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:22 |
| 3 | 2026-10-01T01:50:26Z | main | `serve_start` | — | serve_start · model=granite-4.2-30b · tensor_parallel_size=1 · topology=single · session=serve-granite42-30b-v030-20261001T015026Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:07:56Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:24 |
| 4 | 2026-10-01T01:57:22Z | main | `serve_stop` | — | serve_stop · model=granite-4.2-30b · duration_s=416 · session=serve-granite42-30b-v030-20261001T015026Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:07:56Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:27 |
| 5 | 2026-10-01T01:57:43Z | main | `serve_start` | — | serve_start · model=granite-4.2-30b · tensor_parallel_size=1 · topology=single · session=serve-granite42-30b-v030-20261001T015743Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:07:56Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:29 |
| 6 | 2026-10-01T02:04:49Z | main | `serve_stop` | — | serve_stop · model=granite-4.2-30b · duration_s=426 · session=serve-granite42-30b-v030-20261001T015743Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:07:56Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:32 |
| 7 | 2026-10-01T02:05:04Z | main | `serve_start` | — | serve_start · model=granite-4.2-30b · tensor_parallel_size=1 · topology=single · session=serve-granite42-30b-v030-20261001T020504Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:07:56Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:34 |
| 8 | 2026-10-01T02:23:54Z | main | `serve_stop` | — | serve_stop · model=granite-4.2-30b · duration_s=1130 · session=serve-granite42-30b-v030-20261001T020504Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:07:56Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:37 |
| 9 | 2026-10-01T08:26:06Z | main | `budget_declare` | `serve-granite42-30b-v030` | 예산 선언(선언값 — 측정 아님) · floor_mib=18591 · weights_mib=55841 · kv_mib=33792 · overhead_mib=16384 · mem_total_mib=124608 · 기동 창 1/3: budget_renew 1회 · 닫힘 budget_clear 2026-10-01T08:41:10Z(선언 후 15m04s) · measured_utc 2026-10-10T07:14:23Z 는 이 창 밖(다른 기동 · 측정 전) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:07:56Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:43 |
| 10 | 2026-10-01T08:26:06Z | main | `serve_start` | — | serve_start · model=granite-4.2-30b · tensor_parallel_size=1 · topology=single · session=serve-granite42-30b-v030-20261001T082606Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:07:56Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:44 |
| 11 | 2026-10-01T08:26:07Z | main | `budget_honored` | `serve-granite42-30b-v030` | budget_honored · floor_mib=18591 · arm_ceiling_mib=15519 · remaining_s=7199 · 창 시작 2026-10-01T08:26:07Z 후 0m00s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:07:56Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:572 |
| 12 | 2026-10-01T08:26:08Z | main | `budget_renew` | `serve-granite42-30b-v030` | budget_renew · floor_mib=18591 · ttl_s=7200 · remaining_before_s=7198 · 창 시작 2026-10-01T08:26:06Z 후 0m02s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:07:56Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:45 |
| 13 | 2026-10-01T08:26:34Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=78533 · rate_mib_s=28994 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → serve-granite42-30b-v030 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-01T08:26:07Z 후 0m27s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:07:56Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:573 |
| 14 | 2026-10-01T08:32:28Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=24714 · rate_mib_s=33929 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → serve-granite42-30b-v030 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-01T08:26:07Z 후 6m21s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:07:56Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:574 |
| 15 | 2026-10-01T08:41:10Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → serve-granite42-30b-v030 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-01T08:26:06Z 후 15m04s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:07:56Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:46 |
| 16 | 2026-10-01T08:41:10Z | main | `serve_stop` | — | serve_stop · model=granite-4.2-30b · duration_s=904 · session=serve-granite42-30b-v030-20261001T082606Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:07:56Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:47 |
| 17 | 2026-10-01T08:41:10Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → serve-granite42-30b-v030 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-01T08:26:07Z 후 15m03s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-10T05:07:56Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:575 |
| 18 | 2026-10-10T04:55:14Z | main | `budget_declare` | `serve-granite42-30b-v030` | 예산 선언(선언값 — 측정 아님) · floor_mib=16543 · weights_mib=55841 · kv_mib=33792 · overhead_mib=18432 · mem_total_mib=124608 · 기동 창 2/3: budget_renew 1회 · 닫힘 budget_clear 2026-10-10T05:07:27Z(선언 후 12m13s) · measured_utc 2026-10-10T07:14:23Z 는 이 창 밖(다른 기동 · 측정 전) · 캠페인 선언 전 · 같은 셀 기동(캠페인 경계 2026-10-10T05:07:56Z 이전이나 세션 serve-granite42-30b-v030-20261010T045514Z 이 이 셀 기록 docs/simlog/26101012_벤치_knob_프리셋_E2E/a_serve.log:15 에 관측 — 이 셀의 시도로 센다) | docs/logs/main/events/2026-10.jsonl:310 |
| 19 | 2026-10-10T04:55:14Z | main | `serve_start` | — | serve_start · model=granite-4.2-30b · tensor_parallel_size=1 · topology=single · session=serve-granite42-30b-v030-20261010T045514Z · 캠페인 선언 전 · 같은 셀 기동(캠페인 경계 2026-10-10T05:07:56Z 이전이나 세션 serve-granite42-30b-v030-20261010T045514Z 이 이 셀 기록 docs/simlog/26101012_벤치_knob_프리셋_E2E/a_serve.log:15 에 관측 — 이 셀의 시도로 센다) | docs/logs/main/events/2026-10.jsonl:311 |
| 20 | 2026-10-10T04:55:15Z | main | `budget_renew` | `serve-granite42-30b-v030` | budget_renew · floor_mib=16543 · ttl_s=7200 · remaining_before_s=7199 · 창 시작 2026-10-10T04:55:14Z 후 0m01s · 캠페인 선언 전 · 같은 셀 기동(캠페인 경계 2026-10-10T05:07:56Z 이전이나 세션 serve-granite42-30b-v030-20261010T045514Z 이 이 셀 기록 docs/simlog/26101012_벤치_knob_프리셋_E2E/a_serve.log:15 에 관측 — 이 셀의 시도로 센다) | docs/logs/main/events/2026-10.jsonl:312 |
| 21 | 2026-10-10T04:55:15Z | main | `budget_honored` | `serve-granite42-30b-v030` | budget_honored · floor_mib=16543 · arm_ceiling_mib=13471 · remaining_s=7199 · 창 시작 2026-10-10T04:55:15Z 후 0m00s · 캠페인 선언 전 · 같은 셀 기동(캠페인 경계 2026-10-10T05:07:56Z 이전이나 세션 serve-granite42-30b-v030-20261010T045514Z 이 이 셀 기록 docs/simlog/26101012_벤치_knob_프리셋_E2E/a_serve.log:15 에 관측 — 이 셀의 시도로 센다) | docs/logs/main/events/watchdog.jsonl:712 |
| 22 | 2026-10-10T04:55:42Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=72261 · rate_mib_s=27103 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → serve-granite42-30b-v030 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-10T04:55:15Z 후 0m27s) · 캠페인 선언 전 · 같은 셀 기동(캠페인 경계 2026-10-10T05:07:56Z 이전이나 세션 serve-granite42-30b-v030-20261010T045514Z 이 이 셀 기록 docs/simlog/26101012_벤치_knob_프리셋_E2E/a_serve.log:15 에 관측 — 이 셀의 시도로 센다) | docs/logs/main/events/watchdog.jsonl:713 |
| 23 | 2026-10-10T05:07:27Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → serve-granite42-30b-v030 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-10T04:55:14Z 후 12m13s) · 캠페인 선언 전 · 같은 셀 기동(캠페인 경계 2026-10-10T05:07:56Z 이전이나 세션 serve-granite42-30b-v030-20261010T045514Z 이 이 셀 기록 docs/simlog/26101012_벤치_knob_프리셋_E2E/a_serve.log:15 에 관측 — 이 셀의 시도로 센다) | docs/logs/main/events/2026-10.jsonl:313 |
| 24 | 2026-10-10T05:07:27Z | main | `serve_stop` | — | serve_stop · model=granite-4.2-30b · duration_s=733 · session=serve-granite42-30b-v030-20261010T045514Z · 캠페인 선언 전 · 같은 셀 기동(캠페인 경계 2026-10-10T05:07:56Z 이전이나 세션 serve-granite42-30b-v030-20261010T045514Z 이 이 셀 기록 docs/simlog/26101012_벤치_knob_프리셋_E2E/a_serve.log:15 에 관측 — 이 셀의 시도로 센다) | docs/logs/main/events/2026-10.jsonl:314 |
| 25 | 2026-10-10T05:07:28Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → serve-granite42-30b-v030 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-10T04:55:15Z 후 12m13s) · 캠페인 선언 전 · 같은 셀 기동(캠페인 경계 2026-10-10T05:07:56Z 이전이나 세션 serve-granite42-30b-v030-20261010T045514Z 이 이 셀 기록 docs/simlog/26101012_벤치_knob_프리셋_E2E/a_serve.log:15 에 관측 — 이 셀의 시도로 센다) | docs/logs/main/events/watchdog.jsonl:714 |
| 26 | 2026-10-10T05:07:56Z | main | `budget_declare` | `serve-granite42-30b-v030` | 예산 선언(선언값 — 측정 아님) · floor_mib=16543 · weights_mib=55841 · kv_mib=33792 · overhead_mib=18432 · mem_total_mib=124608 · 기동 창 3/3: budget_renew 17회 · 닫힘 budget_clear 2026-10-10T07:15:02Z(선언 후 2h07m06s) · measured_utc 2026-10-10T07:14:23Z 가 이 창 안(이 측정의 기동) | docs/logs/main/events/2026-10.jsonl:315 |
| 27 | 2026-10-10T05:07:56Z | main | `serve_start` | — | serve_start · model=granite-4.2-30b · tensor_parallel_size=1 · topology=single · session=serve-granite42-30b-v030-20261010T050756Z | docs/logs/main/events/2026-10.jsonl:316 |
| 28 | 2026-10-10T05:07:56Z | main | `budget_honored` | `serve-granite42-30b-v030` | budget_honored · floor_mib=16543 · arm_ceiling_mib=13471 · remaining_s=7200 · 창 시작 2026-10-10T05:07:56Z 후 0m00s | docs/logs/main/events/watchdog.jsonl:715 |
| 29 | 2026-10-10T05:07:57Z | main | `budget_renew` | `serve-granite42-30b-v030` | budget_renew · floor_mib=16543 · ttl_s=7200 · remaining_before_s=7199 · 창 시작 2026-10-10T05:07:56Z 후 0m01s | docs/logs/main/events/2026-10.jsonl:317 |
| 30 | 2026-10-10T05:08:11Z | main | `engine_log_window_start` | — | 캡처된 엔진 로그의 첫 시각(꼬리 캡처일 수 있다 — 엔진 시작 시각이 아닐 수 있다) · 선언 2026-10-10T05:07:56Z 후 0m15s · 시계: 컨테이너 시각을 UTC 로 읽어 측정 창 [2026-10-10T05:07:56Z, 2026-10-10T07:15:02Z] 안에 드는 것을 확인 | output/single/benchlog/sweep_granite42-30b-v030/lite_engine_granite42-30b-v030.log:36 |
| 31 | 2026-10-10T05:08:23Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=69825 · rate_mib_s=29889 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → serve-granite42-30b-v030 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-10T05:07:56Z 후 0m27s) | docs/logs/main/events/watchdog.jsonl:716 |
| 32 | 2026-10-10T05:08:24Z | main | `engine_prefetch_disabled` | — | 가중치 auto-prefetch 꺼짐 줄 ×1 — filesystem CIFS(vLLM 로그 문구) | output/single/benchlog/sweep_granite42-30b-v030/lite_engine_granite42-30b-v030.log:58 |
| 33 | 2026-10-10T05:14:34Z | main | `engine_weights_loaded` | — | 가중치 적재 완료 줄 ×1(rank·단계별) · 최장 370.04s · 이 행 = 마지막 줄 | output/single/benchlog/sweep_granite42-30b-v030/lite_engine_granite42-30b-v030.log:73 |
| 34 | 2026-10-10T05:14:34Z | main | `engine_model_loaded` | — | 모델 적재 완료 줄 ×1(rank 별) · 마지막 줄 54.58 GiB · 374.2s | output/single/benchlog/sweep_granite42-30b-v030/lite_engine_granite42-30b-v030.log:74 |
| 35 | 2026-10-10T05:14:39Z | main | `engine_kv_cache_sized` | — | GPU KV cache 135,168 tokens · 요청당 131,072 tokens 기준 최대 동시성 1.03x | output/single/benchlog/sweep_granite42-30b-v030/lite_engine_granite42-30b-v030.log:82 |
| 36 | 2026-10-10T05:14:40Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=36926 · rate_mib_s=22423 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → serve-granite42-30b-v030 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-10T05:07:56Z 후 6m44s) | docs/logs/main/events/watchdog.jsonl:717 |
| 37 | 2026-10-10T05:15:52Z | main | `engine_init_done` | — | 엔진 초기화(profile·KV 할당·warmup) 77.15s | output/single/benchlog/sweep_granite42-30b-v030/lite_engine_granite42-30b-v030.log:97 |
| 38 | 2026-10-10T05:15:53Z | main | `server_starting` | — | API 서버 기동 — 이 측정 창 선언 2026-10-10T05:07:56Z 후 7m57s · 캡처 로그 첫 시각 2026-10-10T05:08:11Z 후 7m42s | output/single/benchlog/sweep_granite42-30b-v030/lite_engine_granite42-30b-v030.log:104 |
| 39 | 2026-10-10T05:18:59Z | main | `budget_renew` | `serve-granite42-30b-v030` | budget_renew · floor_mib=16543 · ttl_s=7200 · remaining_before_s=6538 · 창 시작 2026-10-10T05:07:56Z 후 11m03s | docs/logs/main/events/2026-10.jsonl:318 |
| 40 | 2026-10-10T05:37:57Z | main | `budget_renew` | `serve-granite42-30b-v030` | budget_renew · floor_mib=16543 · ttl_s=7200 · remaining_before_s=6062 · 창 시작 2026-10-10T05:07:56Z 후 30m01s | docs/logs/main/events/2026-10.jsonl:319 |
| 41 | 2026-10-10T05:38:45Z | main | `budget_renew` | `serve-granite42-30b-v030` | budget_renew · floor_mib=16543 · ttl_s=7200 · remaining_before_s=7152 · 창 시작 2026-10-10T05:07:56Z 후 30m49s | docs/logs/main/events/2026-10.jsonl:320 |
| 42 | 2026-10-10T05:58:14Z | main | `budget_renew` | `serve-granite42-30b-v030` | budget_renew · floor_mib=16543 · ttl_s=7200 · remaining_before_s=6031 · 창 시작 2026-10-10T05:07:56Z 후 50m18s | docs/logs/main/events/2026-10.jsonl:321 |
| 43 | 2026-10-10T06:07:57Z | main | `budget_renew` | `serve-granite42-30b-v030` | budget_renew · floor_mib=16543 · ttl_s=7200 · remaining_before_s=6617 · 창 시작 2026-10-10T05:07:56Z 후 1h00m01s | docs/logs/main/events/2026-10.jsonl:322 |
| 44 | 2026-10-10T06:17:44Z | main | `budget_renew` | `serve-granite42-30b-v030` | budget_renew · floor_mib=16543 · ttl_s=7200 · remaining_before_s=6613 · 창 시작 2026-10-10T05:07:56Z 후 1h09m48s | docs/logs/main/events/2026-10.jsonl:323 |
| 45 | 2026-10-10T06:27:35Z | main | `budget_renew` | `serve-granite42-30b-v030` | budget_renew · floor_mib=16543 · ttl_s=7200 · remaining_before_s=6609 · 창 시작 2026-10-10T05:07:56Z 후 1h19m39s | docs/logs/main/events/2026-10.jsonl:324 |
| 46 | 2026-10-10T06:37:26Z | main | `budget_renew` | `serve-granite42-30b-v030` | budget_renew · floor_mib=16543 · ttl_s=7200 · remaining_before_s=6609 · 창 시작 2026-10-10T05:07:56Z 후 1h29m30s | docs/logs/main/events/2026-10.jsonl:325 |
| 47 | 2026-10-10T06:37:57Z | main | `budget_renew` | `serve-granite42-30b-v030` | budget_renew · floor_mib=16543 · ttl_s=7200 · remaining_before_s=7169 · 창 시작 2026-10-10T05:07:56Z 후 1h30m01s | docs/logs/main/events/2026-10.jsonl:326 |
| 48 | 2026-10-10T06:47:16Z | main | `budget_renew` | `serve-granite42-30b-v030` | budget_renew · floor_mib=16543 · ttl_s=7200 · remaining_before_s=6641 · 창 시작 2026-10-10T05:07:56Z 후 1h39m20s | docs/logs/main/events/2026-10.jsonl:327 |
| 49 | 2026-10-10T06:52:51Z | main | `budget_renew` | `serve-granite42-30b-v030` | budget_renew · floor_mib=16543 · ttl_s=7200 · remaining_before_s=6865 · 창 시작 2026-10-10T05:07:56Z 후 1h44m55s | docs/logs/main/events/2026-10.jsonl:328 |
| 50 | 2026-10-10T06:58:26Z | main | `budget_renew` | `serve-granite42-30b-v030` | budget_renew · floor_mib=16543 · ttl_s=7200 · remaining_before_s=6865 · 창 시작 2026-10-10T05:07:56Z 후 1h50m30s | docs/logs/main/events/2026-10.jsonl:329 |
| 51 | 2026-10-10T07:04:02Z | main | `budget_renew` | `serve-granite42-30b-v030` | budget_renew · floor_mib=16543 · ttl_s=7200 · remaining_before_s=6864 · 창 시작 2026-10-10T05:07:56Z 후 1h56m06s | docs/logs/main/events/2026-10.jsonl:330 |
| 52 | 2026-10-10T07:07:29Z | main | `budget_renew` | `serve-granite42-30b-v030` | budget_renew · floor_mib=16543 · ttl_s=7200 · remaining_before_s=6993 · 창 시작 2026-10-10T05:07:56Z 후 1h59m33s | docs/logs/main/events/2026-10.jsonl:331 |
| 53 | 2026-10-10T07:07:58Z | main | `budget_renew` | `serve-granite42-30b-v030` | budget_renew · floor_mib=16543 · ttl_s=7200 · remaining_before_s=7171 · 창 시작 2026-10-10T05:07:56Z 후 2h00m02s | docs/logs/main/events/2026-10.jsonl:332 |
| 54 | 2026-10-10T07:10:56Z | main | `budget_renew` | `serve-granite42-30b-v030` | budget_renew · floor_mib=16543 · ttl_s=7200 · remaining_before_s=7022 · 창 시작 2026-10-10T05:07:56Z 후 2h03m00s | docs/logs/main/events/2026-10.jsonl:333 |
| 55 | 2026-10-10T07:15:02Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → serve-granite42-30b-v030 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-10T05:07:56Z 후 2h07m06s) | docs/logs/main/events/2026-10.jsonl:334 |
| 56 | 2026-10-10T07:15:02Z | main | `serve_stop` | — | serve_stop · model=granite-4.2-30b · duration_s=7626 · session=serve-granite42-30b-v030-20261010T050756Z | docs/logs/main/events/2026-10.jsonl:335 |
| 57 | 2026-10-10T07:15:03Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → serve-granite42-30b-v030 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-10T05:07:56Z 후 2h07m07s) | docs/logs/main/events/watchdog.jsonl:718 |

**기동 시도**(예산 선언 `budget_declare` 마다 한 시도 · 같은 노드의 이어지는 행과 선언 없는 노드(엔진 로그)의 그 시각 행을 묶었다 · 사살 · 트립 · 거부는 닫힘과 따로 센다 · `measurement` 행은 측정 기록이라 시도로 세지 않는다(위 표에만) — 해석은 저작자 몫)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 1 | main | `serve-granite42-30b-v030` | 2026-10-10T04:55:14Z | 1회 · 첫 2026-10-10T04:55:15Z | 없음 | `budget_clear` 2026-10-10T05:07:27Z | 5 |
| 2 | main | `serve-granite42-30b-v030` | 2026-10-10T05:07:56Z | 17회 · 첫 2026-10-10T05:07:57Z | 없음 | `budget_clear` 2026-10-10T07:15:02Z | 13 |

**같은 셀 이름 · 앞선 캠페인의 기동 2회**(캠페인 경계 2026-10-10T05:07:56Z 이전 — 라벨이 같아 원장에서 함께 묶였을 뿐 이 셀의 시도로 세지 않는다 · 행 전문 = 01 §1.4)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 앞선-1 | main | — | —(선언 행 없음) | 없음 | 없음 | —(닫힘 행 없음) | 8 |
| 앞선-2 | main | `serve-granite42-30b-v030` | 2026-10-01T08:26:06Z | 1회 · 첫 2026-10-01T08:26:08Z | 없음 | `budget_clear` 2026-10-01T08:41:10Z | 6 |
<!-- /FACT:event_timeline -->

<!-- FACT:measurement_env -->
_측정 실행의 env 관측 없음 — 측정 엔진 로그에서 env echo 줄(`<KEY> set by environment to <VALUE>` 등)을 찾지 못했다(evidence.measurement_env_observed · 부재 ≠ 미설정)._
<!-- /FACT:measurement_env -->

> 이 절이 답하는 질문: 수신자가 이 zip 만으로 이 셀을 다시 띄우려면 어떤 순서로 무엇을 하고, 각 단계의 성공을 무엇으로 판정하며, 얼마나 기다려야 하는가?

**(a) 사전 준비.** 가중치는 노드의 NAS 마운트 `<manifest.nas_model_path>/IBM/Granite-4.2/granite-4.2-30b` 를 compose 가 `/app/models` 로 읽기 전용 마운트해 그대로 읽는다(스테이징 없음 · PLE 없음 — 1.6). 체크포인트 크기는 54.53 GiB 이고 엔진은 "CIFS 가 인식되는 네트워크 FS 가 아니고 체크포인트가 가용 RAM(58.74 GiB)의 90% 를 넘는다" 는 두 조건으로 auto-prefetch 를 껐으며, 가중치 적재는 370.04 s 였다(엔진 로그 L57-58 · L73 — 적재 시간이 prefetch 꺼짐 때문이라는 인과는 대조 없는 추론) — 네트워크 파일시스템 위 mmap 금지 같은 위치 조건은 이 셀에 없다(PLE · mmap 미사용). 추론 파서 플러그인 `granite_thinking_parser.py` 가 체크포인트 폴더 안에 있어야 한다(러너가 그 경로를 연다 — `artifacts/triplet/granite42-30b-v030.sh`). 필수 env 는 셀 env(`artifacts/triplet/.env.granite42-30b-v030` — `IMAGE_TAG` · `CONFIG_FILE` · `SERVING_MODEL_NAME`)와 프로젝트 env 형상(`artifacts/compose/.env.template` — `NAS_MODEL_PATH` 등 manifest 필드)에 있다. 측정 실행의 엔진 로그에는 env echo 줄이 없어 측정 당시 env 관측 표는 비어 있다(위 사실 블록 — 부재 ≠ 미설정). 이 셀은 측정 뒤 재생성된 env 형상이 없다(1.1 '싣지 않은 파일' 에 해당 사유 없음).

**(b) 빌드.** 메인 노드에서 `Dockerfile.source-build` 선택자 · 위 표의 build-arg(`VLLM_REF=v0.30.0` · `TORCH_CUDA_ARCH=12.1a` · `BUILD_JOBS=8` · `RAY_VERSION=2.48.0` · 게이트 `SM12X_PORT=0` · `SRC_DEPS_AUTHORITY=0`)로 지었다(docker history 관측에서 재구성 — 실행 원문 아님). 이 draft 의 관측으로는 빌드 소요가 미관측이다 — 위 36분 33초는 층 CreatedAt 창이다. 계보 밖 기록(docs/testlog/testlog_26100117_하네스결함교정_재빌드일관성_E2E_판정.md §1 C2 행)은 이 이미지를 `docker compose … build --no-cache --build-arg BUILD_JOBS=8` · 07:38→08:18Z(40분) · rc 0 으로 적는다 — `--no-cache` 빌드이므로 표의 '캐시 재사용 층 포함' 단서는 이 빌드에는 해당하지 않는다고 본다.

**(c) 기동 순서와 성공 판정.** `single_serve_up.sh granite42-30b-v030` 이 ① 통로 · manifest 정합 ② NAS 경로 · RAM 게이트(MemAvailable ≥ ckpt÷tp + 바닥) ③ 예산 선언 ④ 세션 ⑤ compose up ⑥ 예산 갱신 루프 ⑦ health 대기(최대 900 s)를 차례로 밟는다(`docs/simlog/26101012_벤치_knob_프리셋_E2E/a_serve2.log`). 성공 판정은 health 200 + 추론 1회(스모크)이고 이 셀의 발행 자격 관측도 그것이다(00 머리의 등급 블록 — post_health 기록). 패치 발화 서명은 해당 없음(패치 0). 측정 서빙의 기동 소요는 health 대기 475 s 이고, 원장의 예산 선언(05:07:56Z)에서 엔진 로그의 API 서버 기동 줄(L104 · 05:15:53)까지가 7m57s 다(이벤트 표 38행 — 출처는 엔진 로그). 510 s 는 1차 기동의 값이며, 셀 판정 testlog 초판이 이를 2차로 잘못 적었다가 2026-10-10 에 475 s 로 정정했다(02 M1). 엔진 안에서는 가중치 적재 370.04 s · 엔진 초기화(KV 할당 · FlashInfer autotune · CUDA graph 캡처) 77.15 s 였다(엔진 로그 L73 · L97). 기동 중 반복되는 `watchdog_highrate_hold`(이 시도의 원장 31 · 36행 — 메모리 소비 속도가 옛 규칙 기준을 넘었으나 보류)는 사살이 아니며, 이 시도의 사살 · 트립 · 거부는 0 이다(기동 시도 표).

**기동 시도 수.** 이 셀(이 측정)의 기동 시도는 위 표의 1회다(2026-10-10T05:07:56Z 선언 · 갱신 17회 · `budget_clear` 07:15:02Z). 같은 날 04:55:14Z 의 앞선-3 은 이번 E2E 의 1차 기동으로, 1차 스윕이 벤치 예산 게이트에 막힌 뒤(02 W3) 05:07:27Z 에 측정 없이 내려갔다(셀 판정 testlog §1 · 셀 devlog) — 같은 서빙 위에서 4096 으로 스윕만 다시 돌리지 않고 내렸다 다시 띄운 이유는 계보에 적혀 있지 않다. 앞선-1 · 앞선-2 는 2026-10-01 의 같은 셀 이름 기동이다.

**(d) 호스트 메모리 안전 예산 선언.** 선언값: 총량 124608 · 가중치 55841 · KV 33792 · overhead 18432 → 바닥 16543 MiB · arm 상한 13471 MiB(원장 26행 · 기동 로그). 엔진 실측은 가중치 54.58 GiB · KV 33.0 GiB · lite 의 예약 합계 ~87.6 GiB · 시스템 RAM 98.0 GiB(81%)다(03 §3.1). overhead 는 사람이 기동 명령에 넣은 선언값이다(`--overhead-mib 18432` — testlog_26101017_08_29_knob프리셋_E2E_셀A_granite42_30b_v030_판정 §1).

**(e) 측정 명령.** 위 표의 bench 단계 명령은 bench JSON 에서 재구성한 것이고 실행 원문이 아니다. 실제로 친 명령은 셀 판정 testlog §2 의 `sweep_bench.sh … --tool guidellm --backend openai-chat --bench-budget-mib 4096` 이고, JSON 에 없는 도구 인자(ignore_eos · warmup 비율 · 데이터 형상)는 03 §3.4 의 측정 도구 원문에서 읽는다. 벤치 컨테이너는 `--bench-budget-mib` 를 메모리 상한으로 받으므로 통합메모리 노드에서는 (d) 의 바닥과 함께 정한다(02 W3).

**(f) 엔진이 고른 런타임 경로.** 어텐션 백엔드는 FLASH_ATTN(FlashAttention 2)로 자동 선택됐고(L54-55) · KV 레이아웃 LBNHC · top-p/top-k 샘플링은 FlashInfer · V2 Model Runner · 통신 백엔드는 world_size 1 의 nccl 이다(L50 — 집단 통신 없음). KV 는 클램프를 따라 프로파일링 없이 33.0 GiB 를 잡았고 gmu 를 따르지 않는다고 명시했다(L81). 서버는 모델의 `generation_config.json` 으로 기본 샘플링을 `temperature 1.0 · top_p 0.95` 로 덮었다고 경고했다(L103) — 측정 도구가 temperature 를 넘기지 않으면 이 값으로 샘플링한다(03 §3.4). 무서워 보이지만 측정을 막지 않은 줄: `NVIDIA Driver was not detected`(L30 · 컨테이너 진입 스크립트의 검사 · 엔진은 GPU 를 잡았다) · `Failed to import Triton kernels … matmul_ogs`(L49 · L56) · `Failed to import the DeepSelect extension`(L85). 이 줄들이 이 모델 경로와 무관하다는 것은 기동 · 측정이 끝까지 성공했다는 사실에서 나온 판단이다(경로별 원인은 미조사).

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

### 가중치 획득 경로

체크포인트 `ibm-granite/granite-4.2-30b` revision `70fc514076785017d93087f3d8b0676426b6b355`(00 §0.2 — 체크포인트 git HEAD 관측) · 크기 54.53 GiB(safetensors index `total_size` · 이 측정 엔진 로그 L57) · 11 샤드(엔진 로그 L70). 획득 모드는 **관리 NAS**다 — 노드가 `<manifest.nas_model_path>` 에 마운트한 공유 경로 `<manifest.nas_model_path>/IBM/Granite-4.2/granite-4.2-30b` 를 compose 가 `/app/models` 로 읽기 전용 마운트한다(`artifacts/compose/docker-compose.yaml` · 기동 로그 `[NAS-check] OK` — `docs/simlog/26101012_벤치_knob_프리셋_E2E/a_serve2.log` L4 — 측정을 서빙한 2차 기동). 단일 노드이므로 한 벌이면 된다. 이 체크포인트를 처음 받은 명령은 계보에 기록이 없다. 네 환경에서 받아야 한다면 아래는 **추론 — 실행 검증 ✗** 이며, 모델 획득은 사람의 승인 게이트다(무인 다운로드 ✗):

```sh
# 추론 — 실행 검증 ✗ · 사람 승인 후에만
huggingface-cli download ibm-granite/granite-4.2-30b --revision 70fc514076785017d93087f3d8b0676426b6b355 \
  --local-dir <manifest.nas_model_path>/IBM/Granite-4.2/granite-4.2-30b
```

받은 폴더 안에 추론 파서 플러그인 `granite_thinking_parser.py` 가 있어야 한다 — 러너가 `--reasoning-parser-plugin /app/models/IBM/Granite-4.2/granite-4.2-30b/granite_thinking_parser.py` 로 연다(`artifacts/triplet/granite42-30b-v030.sh`). 플러그인이 체크포인트 저장소에 동봉돼 있다는 것은 yaml 머리 주석(L7 "폴더 동봉 플러그인")이 출처이고, 위 명령으로 함께 받아지는지는 검증하지 않았다.

### PLE · 기타 스테이징 생성

해당 없음 — 모델 config 에 PLE 키가 없고(00 §0.9 후보 표 출처 칸) 서빙 경로가 가중치를 NAS 마운트에서 직접 적재한다(측정 기동 로그 `ple_mib=0` — a_serve2.log L5). 로컬 디스크 스테이징 · 해시 확인 절차는 이 셀에 없다. 이 측정의 적재는 370 s 였다(1.4 (a) — CIFS · 가용 RAM 두 조건으로 prefetch 꺼짐). 로컬 디스크로 옮겨 줄일 수 있다는 것은 시도하지 않은 추론이다.

### 네트워크가 필요한 단계

- 베이스 이미지 pull `nvcr.io/nvidia/pytorch:26.07-py3`(00 §0.4) — 오프라인 대체: 미리 pull 해 두거나 이미지 save 사본을 준비한다.
- vLLM 소스 `https://github.com/vllm-project/vllm.git` 의 `v0.30.0`(빌드 중 clone · `artifacts/build_recipe/Dockerfile.source-build`) · pip 의존(`requirements.txt` · `ray==2.48.0`) — 오프라인 대체: 소스 tarball 과 wheel 캐시를 미리 받아 빌드 컨텍스트에 둔다(추론 — 이 레시피에 오프라인 모드는 없다).
- 가중치 다운로드(위) — 사람 승인 게이트.
- 측정용 GuideLLM 컨테이너 이미지(GuideLLM 0.7.3 · 03 §3.3) — `run_bench.sh` 가 `docker run --network host` 로 띄우고 `HF_HUB_OFFLINE=1` 로 토크나이저를 로컬 마운트에서만 읽는다(03 §3.4 도구 원문 L392-398) — 이미지는 미리 pull 해 둔다.
