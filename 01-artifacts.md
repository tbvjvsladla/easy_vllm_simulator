# 01 · 산출물 — 무엇이 실제로 쓰였고, 어떻게 다시 띄우는가

## 1.1 적용 판정 표
> 기계가 적용 증거(빌드 원장 · 원장이 없으면 라벨된 재구성) × 파일 × 선언을 대사한 결과다. `artifacts/` 에는 **적용된 것만** 실렸다 — 빌드 때 스스로 skip 된 패치와 쓰이지 않은 평면의 파일은 표에만 남고 싣지 않는다.

<!-- FACT:slots -->
| 슬롯 | 실림 | 파일 | 신호 | 적용 증거 | 사유 |
|---|---|---|---|---|---|
| `build_patch_post` | 안 실림 | — | 1-signal | build-ledger:docker-image(main-local):sha256:339737df6571ce00993e731ef064a8db66085beaa650e4aa63bdb24f414b2034:/opt/easy-vllm/build_ledger.json | 적용 판정 없음 — 원장 관측(기계 파생) |
| `build_patch_pre` | 안 실림 | — | 1-signal | build-ledger:docker-image(main-local):sha256:339737df6571ce00993e731ef064a8db66085beaa650e4aa63bdb24f414b2034:/opt/easy-vllm/build_ledger.json | 적용 판정 없음 — 원장 관측(기계 파생) |
| `build_recipe` | 실림 | `artifacts/build_recipe/Dockerfile.source-build` · `artifacts/build_recipe/requirements.txt` | 3-signal | build-ledger:build-ledger.dockerfile | 결과 출처: 선택자는 이미지 안 빌드 원장(`build-ledger.dockerfile`)이고 실린 두 파일은 작업트리 바이트다(1.1 실린 리비전 · 리비전 미특정). `requirements.txt` 는 빌드 원장 sha256 과 일치해 **`verified`** 이고, `Dockerfile.source-build` 는 측정 이미지 docker history 의 RUN · COPY 와 16/17 만 맞아 **`generated-unverified`** 다 — 1.1 경고대로 이 파일의 마지막 커밋(2026-10-14T03:07:26Z)이 이미지 Created(01:45:19Z) 뒤이고, 전 이력에도 전수 일치 개정이 없다. 어느 단계가 어긋나는지는 사실 블록이 특정하지 않았다(미확인). 이 이미지를 지… |
| `compose` | 실림 | `artifacts/compose/.env.template` · `artifacts/compose/docker-compose.yaml` | 2-signal | file:output/single/docker-compose.yaml | (a) 싱글 기동 경로다 — `vllm-serve` 서비스가 셀 env(`envs/.env.${CONFIG_FILE}`)를 읽고 `/app/configs/${CONFIG_FILE}.sh` 를 실행하며 호스트 `${SERVING_PORT}` 를 컨테이너 8000 에 매핑한다(L47 ~ L58). 모델은 `NAS_MODEL_PATH` 를 `/app/models` 에 읽기 전용으로 마운트한다(L18). PLE mmap 마운트(L24)는 셀 env 에 `VLLM_PLE_MMAP` 이 없으므로 이 셀에서는 읽히지 않는다(같은 줄 주석 · 측정 색인 `ple_mode_source: defaulted(envfile VLLM_PLE_MMAP absent/0 = stock resident)`). `.env.template… |
| `fork_pin` | 안 실림 | — | 1-signal | none:.claude/policies/arch_variant_ledger.json | 셀 env 에 VARIANT 줄 없음 = stock(workflow.md §변종 좌표의 거처)(기계 파생) |
| `runtime_patch` | 안 실림 | — | 1-signal | none:output/single/configs/gemma26b-ctx-single_patch.py | output/single/configs/gemma26b-ctx-single_patch.py 없음 — arm_patch.sh 는 no-op(런타임 패치 불해당)(기계 파생) |
| `triplet` | 실림 | `artifacts/triplet/.env.gemma26b-ctx-single` · `artifacts/triplet/gemma26b-ctx-single.sh` · `artifacts/triplet/gemma26b-ctx-single.yaml` | 3-signal | file:output/single/configs/gemma26b-ctx-single.yaml | (a) 서빙 설정 · 러너 · 셀 env 다. 러너는 `arm_patch.sh` 가 있으면 source 하고(이 셀에서는 no-op) `vllm serve --config <yaml>` 에 `--enable-auto-tool-choice --tool-call-parser gemma4 --reasoning-parser gemma4` 를 붙인다(sh L12 · L15 ~ L19). yaml 은 노브 4개(gmu · max-model-len · KV 바이트 · KV dtype)와 전송 노브를 갖는다. (b) W1 은 yaml 의 `kv-cache-memory-bytes`, W2 는 yaml 의 `port` 교정으로 넘었다. (c) 측정 기동의 `non-default args`(lite_engine L42)에 y… |

**레시피 경고 · 실린 리비전**(기계 관측 — 수신자 빌드가 여기서 갈라질 수 있다)

- ⚠ `build_recipe` `output/single/Dockerfile.source-build` — 실린 바이트(worktree)가 측정 이미지 docker history 의 RUN·COPY 와 전수 일치하지 않는다(16/17) · 이미지 빌드 뒤 바뀐 레시피일 수 있다 — 마지막 커밋이 이미지 Created 뒤(이미지 Created 2026-10-14T01:45:19Z · 마지막 커밋 2026-10-14T03:07:26Z)
- `build_recipe` `output/single/Dockerfile.source-build` 실린 리비전: 리비전 미특정 · 선택 방법 `worktree(어느 개정도 이미지 history 와 더 맞지 않는다 · 작업트리 16/17 · 전 이력(output/*/Dockerfile.source-build · 모든 ref)에도 전수 일치 개정 없음 — recipe_vs_image 경고 유지)` · docker history 대조 16/17 · 실린 바이트 `worktree` · 쓰인 바이트임을 관측으로 확인 아니오
- `build_recipe` `output/single/requirements.txt` 실린 리비전: 리비전 미특정 · 선택 방법 `worktree(빌드 원장 sha256 일치 — 쓰인 바이트 관측 확인 = 예)` · docker history 대조 없음 · 실린 바이트 `worktree` · 쓰인 바이트임을 관측으로 확인 예
- `compose` `output/single/docker-compose.yaml` 실린 리비전: 커밋 `20c7690b560e` · 선택 방법 `git log -1 --before=2026-10-25T00:44:08Z -- output/single/docker-compose.yaml → 20c7690b560e · 작업트리 mtime 2026-10-25T14:10:42Z > 측정 2026-10-25T00:44:08Z(sweep_index.generated_utc) · 측정 뒤 커밋 1(2199257f7f71) · 작업트리 = HEAD — 측정 전 마지막 커밋의 바이트를 싣는다(git show · 측정 당시 미커밋 수정이 있었는지는 관측 불가)` · docker history 대조 없음 · 실린 바이트 `git:20c7690b560e38d326fb4b7bf853af990945dda0` · 근거 `git-rev-before-measurement` · 쓰인 바이트임을 관측으로 확인 아니오 · 측정 뒤 커밋 1건

**싣지 않은 파일**(슬롯별 사유 — 빌드 때 skip 된 패치는 아래 적용 집합 표에 있다)

- `build_patch_post` known-non-patch 1건: `.gitkeep`
- `build_patch_pre` known-non-patch 1건: `.gitkeep`
- `compose` `arm_patch.sh` — `not-armed(output/single/configs/gemma26b-ctx-single_patch.py 없음(CONFIG_FILE) — arm_patch.sh 는 no-op · arm_patch.sh 측정 당시 개정(44d1f5b05e16) `_cfg=` 키 체인 CONFIG_FILE > 'default')`

**파일별 검증 · 관련성**(`generated-unverified` = 실행 검증되지 않은 생성물 — 파일 첫 줄 경고 · PAYLOAD 파일 기록과 같은 표시 · 관련성 = ① 패치 선언 대상 × 모델 config ② 엔진 로그 발화 서명 — 둘 다 관측일 때만 required/inactive-inferred)

| 슬롯 | 파일 | 검증 | 검증 근거 | 관련성 | 관련성 근거 |
|---|---|---|---|---|---|
| `build_recipe` | `artifacts/build_recipe/Dockerfile.source-build` | `generated-unverified` · renderer | 실린 Dockerfile.source-build(worktree)이 측정 이미지 docker history RUN·COPY 와 16/17 만 일치 — 이 이미지를 지은 바이트임을 관측으로 확인하지 못했다(_select_dockerfile · worktree(어느 개정도 이미지 history 와 더 맞지 않는다 · 작업트리 16/17 · 전 이력(output/*/Dockerfile.source-build · 모든 ref)에도 전수 일치 개정 없음 — recipe_vs_image 경고 유지)) | `unknown` | 이 셀은 이 경로로 실행되지 않았다(generated-unverified · 생성 renderer) — 실행 기록이 없어 필요 여부를 판정하지 않는다(참고 형상) |
| `build_recipe` | `artifacts/build_recipe/requirements.txt` | `verified` · cell-run | 쓰인 레시피(build-ledger.dockerfile) — 개정 선택 slots.build_recipe.evidence.selected_revisions | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/.env.template` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/docker-compose.yaml` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/.env.gemma26b-ctx-single` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/gemma26b-ctx-single.sh` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/gemma26b-ctx-single.yaml` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
<!-- /FACT:slots -->

<!-- FACT:applied_set -->
**적용 집합** — 상태 `observed` · 출처 `docker-image(main-local):sha256:339737df6571ce00993e731ef064a8db66085beaa650e4aa63bdb24f414b2034:/opt/easy-vllm/build_ledger.json`

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
| attestation | absent | output/single/benchlog/attestation_gemma26b-ctx-single.json(부재) |
| local-image | observed | sha256:339737df6571ce00993e731ef064a8db66085beaa650e4aa63bdb24f414b2034 |
<!-- /FACT:applied_set -->

## 1.2 슬롯별 적용 사유
> 이 절이 답하는 질문: 실린 산출물 하나하나는 무엇을 고치며, 왜 이 셀에 필요했고, 이 셀에서 실제로 발화했으며, 언제 불필요해지는가?

### `build_recipe` — `Dockerfile.source-build` · `requirements.txt`

결과 출처: 선택자는 이미지 안 빌드 원장(`build-ledger.dockerfile`)이고 실린 두 파일은 작업트리 바이트다(1.1 실린 리비전 · 리비전 미특정). `requirements.txt` 는 빌드 원장 sha256 과 일치해 **`verified`** 이고, `Dockerfile.source-build` 는 측정 이미지 docker history 의 RUN · COPY 와 16/17 만 맞아 **`generated-unverified`** 다 — 1.1 경고대로 이 파일의 마지막 커밋(2026-10-14T03:07:26Z)이 이미지 Created(01:45:19Z) 뒤이고, 전 이력에도 전수 일치 개정이 없다. 어느 단계가 어긋나는지는 사실 블록이 특정하지 않았다(미확인). 이 이미지를 지은 빌드는 single-node 체크아웃(`689610f` + 미커밋 렌더)이었다고 계보 밖 testlog_26101412(유효맥락 · 이 페이로드의 읽기 목록 밖)가 적는다.
(a) NGC `nvcr.io/nvidia/pytorch:26.07-py3`(L37) 위에서 `VLLM_REPO`/`VLLM_REF` 소스를 클론하고(L76 ~ L78) `use_existing_torch.py` 를 `tools/` 와 루트 중 있는 쪽으로 골라 torch 핀을 걷어 NGC torch 를 그대로 쓴 뒤(L82 ~ L84) `pip install --no-build-isolation -e .` 로 `_C` 를 컴파일한다(L188 ~ L194). requirements.txt 는 설치 목록이 아니라 pip constraint(`/etc/pip/constraint.txt`)로 COPY 된다(L69 ~ L71) — 머리 주석은 v0.31.0 wheel 메타에서 뽑았고 `transformers<5.15.0` 등 KNOWN_INCOMPAT 천장을 얹었다고 적는다(requirements L1 ~ L5). (b) 이 셀의 벽(W1 · W2)을 넘은 수단은 이 레시피에 없다 — 필요한 이유는 재현 입력이라는 것뿐이다. Dockerfile 머리 주석(L7)은 constraint 없이 transformers 5.15.0 이 설치되면 gemma-4 서빙이 로드 전 즉사했다는 이전 기록을 적는다 — 이 셀에서 재현한 실패가 아니다. (c) 원장이 적은 인라인 단계는 `strip-hoist` 하나이고 결과는 `skipped` 다(1.1 적용 집합) — NGC torch 의 `register_opaque_type` 가 hoist 인자를 받으면 소스를 고치지 않는다. (d) 그 단계는 조건이 맞지 않으면 빌드를 멈추지 않고 skip 상태를 원장에 쓴다:

> [원문] Dockerfile.source-build §L115-117
> else:
>     print("[patch] strip-hoist skipped (register_opaque_type accepts hoist / opaque_object 부재)")
>     status = "skipped\tregister_opaque_type accepts hoist / opaque_object 부재"

⚠ `COPY build_patches_src/`(L165) · `COPY build_patches/`(L203)는 빌드 컨텍스트에 그 디렉터리가 있어야 통과한다 — 이 이미지는 두 디렉터리에 `.gitkeep` 만 든 컨텍스트로 빌드됐고(원장 `context_inventory`) zip 에는 그 디렉터리가 없다(1.4 매핑 표의 빈 디렉터리 생성 행 · Dockerfile L133 ~ L134 주석).

### `compose` — `docker-compose.yaml` · `.env.template`

(a) 싱글 기동 경로다 — `vllm-serve` 서비스가 셀 env(`envs/.env.${CONFIG_FILE}`)를 읽고 `/app/configs/${CONFIG_FILE}.sh` 를 실행하며 호스트 `${SERVING_PORT}` 를 컨테이너 8000 에 매핑한다(L47 ~ L58). 모델은 `NAS_MODEL_PATH` 를 `/app/models` 에 읽기 전용으로 마운트한다(L18). PLE mmap 마운트(L24)는 셀 env 에 `VLLM_PLE_MMAP` 이 없으므로 이 셀에서는 읽히지 않는다(같은 줄 주석 · 측정 색인 `ple_mode_source: defaulted(envfile VLLM_PLE_MMAP absent/0 = stock resident)`). `.env.template` 은 마운트 변수 4개의 형상이다(값은 manifest 필드 자리표시). (b) W2 는 이 매핑(호스트 포트 → 컨테이너 8000)과 yaml `port` 의 불일치였다. (c) 측정 당시 개정 `20c7690b560e` 이 실렸다(1.1 실린 리비전 · 측정 뒤 커밋 1건은 PLE mmap 주석의 경로 리터럴 제거). (d) 패치가 아니라 기동 경로라 폐기 조건이 없다.

### `triplet` — `gemma26b-ctx-single.{yaml,sh}` · `.env.gemma26b-ctx-single`

(a) 서빙 설정 · 러너 · 셀 env 다. 러너는 `arm_patch.sh` 가 있으면 source 하고(이 셀에서는 no-op) `vllm serve --config <yaml>` 에 `--enable-auto-tool-choice --tool-call-parser gemma4 --reasoning-parser gemma4` 를 붙인다(sh L12 · L15 ~ L19). yaml 은 노브 4개(gmu · max-model-len · KV 바이트 · KV dtype)와 전송 노브를 갖는다. (b) W1 은 yaml 의 `kv-cache-memory-bytes`, W2 는 yaml 의 `port` 교정으로 넘었다. (c) 측정 기동의 `non-default args`(lite_engine L42)에 yaml 값이 그대로 실렸다 — 측정이 이 config 로 돌았다. (d) 레시피 생성기의 포트 결함이 고쳐지면 yaml L10 의 수작업 주석은 불필요해진다(testlog_26102412 §1 후속).

### 싣지 않은 슬롯

- `build_patch_pre` · `build_patch_post` — 적용된 패치가 없다(원장 `context_inventory` = `.gitkeep` 뿐 · 1.1 '싣지 않은 파일').
- `runtime_patch` — `output/single/configs/gemma26b-ctx-single_patch.py` 가 없어 `arm_patch.sh` 는 no-op 이다(1.1). 이 셀은 런타임 패치 없이 stock 으로 서빙됐다 — 같은 시기의 PLE mmap 플러그인(plan_26102420)은 Qwen 셀의 것이고 Gemma config 에는 PLE 키가 없다.
- `fork_pin` — 셀 env 에 `VARIANT` 줄이 없다 = stock(`vllm-project/vllm` v0.31.0).

## 1.3 값의 지위표
> 서빙 설정의 노브 **전부**와 기계가 모은 후보 지위다(lockset 출처 · yaml 주석 · 엔진 기본값). 최종 지위는 아래 `kind: value-status` 블록이 정한다 — 표의 노브마다 블록이 정확히 하나 있어야 린터를 통과한다.

<!-- FACT:value_status -->
서빙 노브 4개 — 노브마다 `kind: value-status` 블록이 **정확히 1개** 있어야 한다(노브·값은 이 표의 글자 그대로 · 값 칸의 `\|` 는 `|` 로 적는다).

| 노브 | 값 | 후보 지위 | 후보 근거 | yaml 주석 | 출처 |
|---|---|---|---|---|---|
| `gpu-memory-utilization` | 0.85 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `max-model-len` | 262144 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `kv-cache-memory-bytes` | 3368402176 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `kv-cache-dtype` | fp8 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
<!-- /FACT:value_status -->

> 이 절이 답하는 질문: 각 서빙 노브의 값은 이 셀에서 어떤 지위인가 — 조정됐나, 승계됐나, 음성대조로 일부러 둔 것인가, 엔진 기본값인가, 필요조건인가?

```hint-event
id: V1
kind: value-status
노브: gpu-memory-utilization
값: 0.85
지위: inherited
근거: 손레버 — yaml 머리는 gmu_source=hand · 트라이얼 요약은 config.safety_margin 승계로 적는다 · 바꿔 잰 기록 없음 · KV 클램프가 있어 엔진은 KV 산정에 이 값을 쓰지 않는다고 기록(lite_engine L84)
출처: [docs/simlog/2026102406_2_gemma4-26b-a4b-262k/run_summary.json, output/single/benchlog/sweep_gemma26b-ctx-single/lite_engine_gemma26b-ctx-single.log]
```

```hint-event
id: V2
kind: value-status
노브: max-model-len
값: 262144
지위: inherited
근거: config max_position_embeddings(native 최대) · YaRN 미적용(R1) · 240,932 토큰 needle 까지 검증 · 다른 길이로 바꿔 잰 기록 없음
출처: [testlog_26102412_gemma26b_싱글_262k_기동_근접추론_lite_판정 §1. 기동, testlog_26102412_gemma26b_싱글_262k_기동_근접추론_lite_판정 §2. 근접 길이 needle]
```

```hint-event
id: V3
kind: value-status
노브: kv-cache-memory-bytes
값: 3368402176
지위: declared-requirement
근거: W1 — 이 노브를 비운 트라이얼(null)이 KV 52.89 GiB 를 잡아 memwatch 에 사살됐다 · 필요조건은 클램프의 존재이고 값 자체는 262144 × batch 1 필요량 파생(다른 값 시도 0 · 엔진 1.02x)
출처: [docs/simlog/2026102406_2_gemma4-26b-a4b-262k/run_summary.json, docs/simlog/2026102406_2_gemma4-26b-a4b-262k/trial01_memwatch.log, testlog_26102412_gemma26b_싱글_262k_기동_근접추론_lite_판정 §1. 기동]
```

```hint-event
id: V4
kind: value-status
노브: kv-cache-dtype
값: fp8
지위: inherited
근거: Phase 1 레시피 후보의 KV dtype(run_summary kv_cache_quant fp8) · auto 등 다른 dtype 으로 잰 기록 없음 · 엔진이 scaling factor 없는 fp8 의 정확도 저하 가능성을 경고(lite_engine L45)
출처: [docs/simlog/2026102406_2_gemma4-26b-a4b-262k/run_summary.json, output/single/benchlog/sweep_gemma26b-ctx-single/lite_engine_gemma26b-ctx-single.log]
```

**요약** — 필요조건으로 확인된 것은 KV 절대 클램프의 **존재**(V3 · W1)다. 그 값 3368402176 은 262144 토큰 한 요청분이라 엔진 동시성 1.02x 로, 같은 길이의 요청 둘을 동시에 받을 KV 가 없다 — 다른 동시성 · 컨텍스트 목표로 옮기면 반드시 다시 계산한다. gmu 0.85(V1)는 손레버이고 이 셀에서는 KV 산정에 쓰이지 않았다. max-model-len(V2)은 native 최대이고 92% 지점까지만 검증됐다(Q1). KV dtype fp8(V4)은 비교 없이 고른 값이다 — 정확도 영향은 이 계보가 재지 않았다. 러너 인자(`--tool-call-parser gemma4` · `--reasoning-parser gemma4`)는 yaml 노브가 아니라 이 표에 없다 — 트라이얼 스모크가 툴콜 · reasoning 분리를 통과했다(trial02_smoke.json).

## 1.4 재현 절차
> 셀 실행 기록에서 기계가 파생한 명령 순서와 단계별 소요다. 소요가 `미관측` 인 단계는 시각 기록이 없다는 뜻이다(0 이 아니다). 그 아래는 블랙박스 이벤트 원장에서 이 셀의 행을 기계가 뽑아 기동 시도로 묶은 것이고, 마지막은 측정한 실행의 엔진 로그가 echo 한 env 다(측정 당시 값의 정본).

<!-- FACT:reproduce -->
| 순서 | 단계 | 성공 판정 | 소요 | 관측 구간(UTC) | 소요 출처 | 명령 출처 |
|---|---|---|---|---|---|---|
| 1 | build | 빌드 OK · 이미지 실재 | 층 창 38분 19초(docker history 첫 층 → 끝 층 CreatedAt · 캐시 재사용 층 포함 — 빌드 소요 아님) | 2026-10-14T01:07:00Z → 2026-10-14T01:45:19Z (layer-window) | docker history --no-trunc --format '{{json .}}' sha256:339737df6571ce00993e731ef064a8db66085beaa650e4aa63bdb24f414b2034 · 우리 Dockerfile 층 24개의 CreatedAt 첫 층 → 끝 층(캐시 재사용 층은 이전 빌드 시각 — 빌드 소요 아님) | reconstructed(build-ledger build_args + compose build) |
| 2 | serve | health 200 + 추론 1회(스모크 판정) | ≤ 6분 0초(상한) | 2026-10-25T00:11:00Z → 2026-10-25T00:17:00Z (upper) | docs/logs/main/events/2026-10.jsonl budget_declare(label serve-gemma26b-ctx-single) → 첫 측정 시각(output/single/benchlog/sweep_gemma26b-ctx-single/lite_raw_gemma26b-ctx-single.json measured_utc(호스트 date -u · lite_bench.sh 가 cold 레그 직전에 찍는 측정 시작)) | derived(스모크 사용법) |
| 3 | bench | 리포트 발행(+PASS 면 인증서) | 27분 8초 | 2026-10-25T00:17:00Z → 2026-10-25T00:44:08Z (exact) | output/single/benchlog/sweep_gemma26b-ctx-single/lite_raw_gemma26b-ctx-single.json measured_utc(호스트 date -u · lite_bench.sh 가 cold 레그 직전에 찍는 측정 시작) → sweep_index.generated_utc(호스트 UTC · 두 끝 같은 시계) | reconstructed(bench json) |

**빌드 층 시각**(관측 · 소요가 아니라 층 창 — 캐시 재사용 층은 이전 빌드의 시각을 가진다)

- `build` 이미지 층 CreatedAt(docker history): 2026-10-14T01:07:00Z → 2026-10-14T01:45:19Z · 층 24개 · 출처 `docker history --no-trunc --format '{{json .}}' sha256:339737df6571ce00993e731ef064a8db66085beaa650e4aa63bdb24f414b2034` — 우리 Dockerfile 층의 CreatedAt 범위 — 캐시 재사용 층은 이전 빌드 시각을 가진다(소요 아님)

**단계별 명령**(기계 파생 — `명령 출처` 가 `derived(…)` 면 사용법에서 파생 · `reconstructed(…)` 면 괄호 안 관측(docker history build-arg · compose build · bench JSON 필드)에서 재구성한 명령이다. 어느 쪽도 실행 원문(로그에 남은 줄)은 아니다)

**1. build**

```sh
# 실린 artifacts/build_recipe/Dockerfile.source-build(실린 개정: worktree — 이미지를 지은 바이트임을 관측으로 확인 못함(docker history RUN·COPY 16/17))를 output/single/Dockerfile.source-build 자리에 두고 컨텍스트에서 짓는다.
mkdir -p output/single/build_patches_src output/single/build_patches   # 빌드 원장 context_inventory.build_patch_pre = ['.gitkeep'] · 빌드 원장 context_inventory.build_patch_post = ['.gitkeep']
docker build -f output/single/Dockerfile.source-build \
  --build-arg BUILD_JOBS=8 \
  --build-arg RAY_VERSION=2.48.0 \
  --build-arg SM12X_PORT=0 \
  --build-arg SRC_DEPS_AUTHORITY=0 \
  --build-arg TORCH_CUDA_ARCH=12.1a \
  --build-arg VLLM_PRETEND_VERSION= \
  --build-arg VLLM_REF=v0.31.0 \
  --build-arg VLLM_REPO=https://github.com/vllm-project/vllm.git \
  -t easy-vllm:0.31.0-cu133-aarch64-source \
  output/single
# 동등 경로(compose): docker compose -f output/single/docker-compose.yaml --env-file output/single/envs/.env.gemma26b-ctx-single build
```

**2. serve**

```sh
bash .claude/skills/upstream-version-watch/scripts/single_serve_up.sh gemma26b-ctx-single
```

**3. bench**

```sh
# bench JSON 필드로 복원: backend · model_id→--model · tokenizer_id→--tokenizer · num_prompts · max_concurrency · request_rate · burstiness · 요청당 입출력 길이 = total_*_tokens ÷ completed(나누어떨어질 때만 — 평균이다 · 요청별 길이는 JSON 에 없다) · 나누어떨어지면 `--dataset-name random` 으로 추정했다(측정 도구가 그 꼴로 부른다 — 아래 도구 원문).
# JSON 에 없는 인자(--endpoint · --base-url · --ignore-eos · --num-warmups · --temperature · --seed · --random-range-ratio · --trust-remote-code)는 측정 도구가 정했다 — level_* = run_bench.sh · lite_cold/lite_warm = lite_bench.sh(.claude/skills/adversarial-benchmark/scripts/) · 셀 스윕 순서는 sweep_bench.sh.
# output/single/benchlog/sweep_gemma26b-ctx-single/lite_cold_gemma26b-ctx-single.json (date 20261025-001723) · ⚠ --random-input-len/--random-output-len = total ÷ completed 평균(chat 템플릿 포함 실측 토큰 · 측정 도구 원문 인자 미관측 — lite_bench.sh 원문 인자와 다를 수 있다)
vllm bench serve --backend openai-chat --model gemma-4-26b-a4b --tokenizer /app/models/Google/Gemma_4/gemma-4-26B-A4B-it --dataset-name random --random-input-len 525 --random-output-len 128 --num-prompts 1 --max-concurrency 1 --request-rate inf
# output/single/benchlog/sweep_gemma26b-ctx-single/lite_warm_gemma26b-ctx-single.json (date 20261025-001756) · total 토큰이 completed 로 나누어떨어지지 않는다(요청 길이 불균일) — 데이터셋 인자 복원 ✗
vllm bench serve --backend openai-chat --model gemma-4-26b-a4b --tokenizer /app/models/Google/Gemma_4/gemma-4-26B-A4B-it --num-prompts 3 --max-concurrency 1 --request-rate inf
```

**슬롯 → 빌드 컨텍스트 매핑**(빌드 컨텍스트 = `output/<토폴로지>/` · 원천 = 실린 Dockerfile 의 COPY · compose 의 env_file · volumes · command — `—` 는 빌드 컨텍스트 입력이 아니다)

| zip 경로 | 빌드 컨텍스트 자리 | 근거 |
|---|---|---|
| `artifacts/triplet/.env.gemma26b-ctx-single` | `envs/.env.gemma26b-ctx-single` | compose env_file envs/.env.${CONFIG_FILE}(셀 env) |
| `artifacts/triplet/gemma26b-ctx-single.sh` | `configs/gemma26b-ctx-single.sh` | compose volumes ./configs:/app/configs |
| `artifacts/triplet/gemma26b-ctx-single.yaml` | `configs/gemma26b-ctx-single.yaml` | compose volumes ./configs:/app/configs |
| `artifacts/build_recipe/Dockerfile.source-build` | `Dockerfile.source-build` | compose build.dockerfile Dockerfile.source-build(리터럴) · build.context . |
| `artifacts/build_recipe/requirements.txt` | `requirements.txt` | Dockerfile.source-build COPY requirements.txt |
| `artifacts/compose/.env.template` | `.env` | compose 변수치환 프로젝트 .env(${…} 참조) |
| `artifacts/compose/docker-compose.yaml` | `docker-compose.yaml` | compose 파일(빌드 컨텍스트 루트) |
| (실린 파일 없음 — 빈 디렉터리 생성) | `build_patches_src/` | Dockerfile.source-build COPY build_patches_src/ · 실린 패치 없음 — 빈 디렉터리를 만든다(없으면 COPY 단계에서 빌드가 멈춘다) · 빌드 원장 context_inventory.build_patch_pre = ['.gitkeep'] |
| (실린 파일 없음 — 빈 디렉터리 생성) | `build_patches/` | Dockerfile.source-build COPY build_patches/ · 실린 패치 없음 — 빈 디렉터리를 만든다(없으면 COPY 단계에서 빌드가 멈춘다) · 빌드 원장 context_inventory.build_patch_post = ['.gitkeep'] |

**기동 성공 판정(관측 · 발행 자격)**: health 200 = true · 추론 1회 = true · 근거 `output/single/benchlog/sweep_gemma26b-ctx-single/level_01/post_health_gemma26b-ctx-single.json` · `output/single/benchlog/sweep_gemma26b-ctx-single/level_01/guidellm_gemma26b-ctx-single.json`
<!-- /FACT:reproduce -->

<!-- FACT:event_timeline -->
**블랙박스 이벤트 62행**(원장에서 이 셀 label·config 에 맞는 행과 그와 시간이 겹치는 같은 노드의 다른 라벨 창 · 시각순 · 기계 발췌 · 라벨 다름(같은 노드 · 창 밖 이 셀 행과 시간이 겹치는 다른 라벨 선언 창) 19행 — 이 셀의 선언으로 세지 않는다)

> **캠페인 경계 미관측** — 재생 모드(캠페인 밖 측정(측정 시점 ACTIVE=_bootstrap · 이 측정의 캠페인 인스턴스 없음)) — 캠페인 선언을 관측하지 못한다 · 경계 미관측 — 라벨(셀 이름)로 묶은 행 전부를 이 셀의 행으로 센다(같은 셀 이름의 앞선 캠페인 행이 섞였을 수 있다)

> **원장 관측 범위**(노드별 첫 행 → 마지막 행 · evidence.event_ledger_spans): main = 2026-07-31T03:27:43Z → 2026-10-25T03:13:55Z(`docs/logs/main/events/2026-07.jsonl` · `docs/logs/main/events/2026-08.jsonl` · `docs/logs/main/events/2026-09.jsonl` · `docs/logs/main/events/2026-10.jsonl` · `docs/logs/main/events/thermal.jsonl` · `docs/logs/main/events/watchdog.jsonl`)

| # | UTC | 노드 | kind | label | 내용 | 출처 |
|---|---|---|---|---|---|---|
| 1 | 2026-10-23T21:48:33Z | main | `budget_declare` | `camp-26102401-gemma26b` | 예산 선언(선언값 — 측정 아님) · floor_mib=47375 · weights_mib=49221 · kv_mib=3212 · overhead_mib=24800 · mem_total_mib=124608 · 라벨 다름: `camp-26102401-gemma26b` — 같은 노드 · 이 셀 라벨(`gemma26b-ctx-single`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · budget_renew 1회 · 닫힘 budget_clear 2026-10-23T22:03:55Z(선언 후 15m22s) · measured_utc 2026-10-25T00:44:08Z 는 이 창 밖(측정 전) | docs/logs/main/events/2026-10.jsonl:496 |
| 2 | 2026-10-23T21:48:33Z | main | `serve_start` | — | serve_start · model=gemma-4-26b-a4b · tensor_parallel_size=1 · topology=single · session=serve-gemma26b-ctx-single-20261023T214833Z | docs/logs/main/events/2026-10.jsonl:497 |
| 3 | 2026-10-23T21:48:33Z | main | `budget_honored` | `camp-26102401-gemma26b` | budget_honored · floor_mib=47375 · arm_ceiling_mib=44303 · remaining_s=7200 · 창 시작 2026-10-23T21:48:33Z 후 0m00s · 라벨 다름: `camp-26102401-gemma26b` — 같은 노드 · 이 셀 라벨(`gemma26b-ctx-single`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:745 |
| 4 | 2026-10-23T21:48:34Z | main | `budget_renew` | `camp-26102401-gemma26b` | budget_renew · floor_mib=47375 · ttl_s=7200 · remaining_before_s=7199 · 창 시작 2026-10-23T21:48:33Z 후 0m01s · 라벨 다름: `camp-26102401-gemma26b` — 같은 노드 · 이 셀 라벨(`gemma26b-ctx-single`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:498 |
| 5 | 2026-10-23T21:49:07Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=63404 · rate_mib_s=28704 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → camp-26102401-gemma26b 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-23T21:48:33Z 후 0m34s) · 라벨 다름: `camp-26102401-gemma26b` — 같은 노드 · 이 셀 라벨(`gemma26b-ctx-single`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:746 |
| 6 | 2026-10-23T22:03:55Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → camp-26102401-gemma26b 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-23T21:48:33Z 후 15m22s) · 라벨 다름: `camp-26102401-gemma26b` — 같은 노드 · 이 셀 라벨(`gemma26b-ctx-single`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:499 |
| 7 | 2026-10-23T22:03:55Z | main | `serve_stop` | — | serve_stop · model=gemma-4-26b-a4b · duration_s=922 · session=serve-gemma26b-ctx-single-20261023T214833Z | docs/logs/main/events/2026-10.jsonl:500 |
| 8 | 2026-10-23T22:03:55Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → camp-26102401-gemma26b 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-23T21:48:33Z 후 15m22s) · 라벨 다름: `camp-26102401-gemma26b` — 같은 노드 · 이 셀 라벨(`gemma26b-ctx-single`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:747 |
| 9 | 2026-10-23T23:52:08Z | main | `budget_declare` | `camp-26102401-gemma26b` | 예산 선언(선언값 — 측정 아님) · floor_mib=47375 · weights_mib=49221 · kv_mib=3212 · overhead_mib=24800 · mem_total_mib=124608 · 라벨 다름: `camp-26102401-gemma26b` — 같은 노드 · 이 셀 라벨(`gemma26b-ctx-single`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · budget_renew 8회 · 닫힘 budget_clear 2026-10-24T03:50:45Z(선언 후 3h58m37s) · measured_utc 2026-10-25T00:44:08Z 는 이 창 밖(측정 전) | docs/logs/main/events/2026-10.jsonl:501 |
| 10 | 2026-10-23T23:52:08Z | main | `serve_start` | — | serve_start · model=gemma-4-26b-a4b · tensor_parallel_size=1 · topology=single · session=serve-gemma26b-ctx-single-20261023T235208Z | docs/logs/main/events/2026-10.jsonl:502 |
| 11 | 2026-10-23T23:52:09Z | main | `budget_honored` | `camp-26102401-gemma26b` | budget_honored · floor_mib=47375 · arm_ceiling_mib=44303 · remaining_s=7199 · 창 시작 2026-10-23T23:52:09Z 후 0m00s · 라벨 다름: `camp-26102401-gemma26b` — 같은 노드 · 이 셀 라벨(`gemma26b-ctx-single`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:748 |
| 12 | 2026-10-23T23:52:10Z | main | `budget_renew` | `camp-26102401-gemma26b` | budget_renew · floor_mib=47375 · ttl_s=7200 · remaining_before_s=7198 · 창 시작 2026-10-23T23:52:08Z 후 0m02s · 라벨 다름: `camp-26102401-gemma26b` — 같은 노드 · 이 셀 라벨(`gemma26b-ctx-single`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:503 |
| 13 | 2026-10-23T23:52:41Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=84180 · rate_mib_s=27629 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → camp-26102401-gemma26b 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-23T23:52:09Z 후 0m32s) · 라벨 다름: `camp-26102401-gemma26b` — 같은 노드 · 이 셀 라벨(`gemma26b-ctx-single`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:749 |
| 14 | 2026-10-24T00:22:10Z | main | `budget_renew` | `camp-26102401-gemma26b` | budget_renew · floor_mib=47375 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-23T23:52:08Z 후 30m02s · 라벨 다름: `camp-26102401-gemma26b` — 같은 노드 · 이 셀 라벨(`gemma26b-ctx-single`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:504 |
| 15 | 2026-10-24T00:52:10Z | main | `budget_renew` | `camp-26102401-gemma26b` | budget_renew · floor_mib=47375 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-23T23:52:08Z 후 1h00m02s · 라벨 다름: `camp-26102401-gemma26b` — 같은 노드 · 이 셀 라벨(`gemma26b-ctx-single`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:505 |
| 16 | 2026-10-24T01:22:10Z | main | `budget_renew` | `camp-26102401-gemma26b` | budget_renew · floor_mib=47375 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-23T23:52:08Z 후 1h30m02s · 라벨 다름: `camp-26102401-gemma26b` — 같은 노드 · 이 셀 라벨(`gemma26b-ctx-single`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:506 |
| 17 | 2026-10-24T01:52:10Z | main | `budget_renew` | `camp-26102401-gemma26b` | budget_renew · floor_mib=47375 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-23T23:52:08Z 후 2h00m02s · 라벨 다름: `camp-26102401-gemma26b` — 같은 노드 · 이 셀 라벨(`gemma26b-ctx-single`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:507 |
| 18 | 2026-10-24T02:22:10Z | main | `budget_renew` | `camp-26102401-gemma26b` | budget_renew · floor_mib=47375 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-23T23:52:08Z 후 2h30m02s · 라벨 다름: `camp-26102401-gemma26b` — 같은 노드 · 이 셀 라벨(`gemma26b-ctx-single`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:508 |
| 19 | 2026-10-24T02:52:10Z | main | `budget_renew` | `camp-26102401-gemma26b` | budget_renew · floor_mib=47375 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-23T23:52:08Z 후 3h00m02s · 라벨 다름: `camp-26102401-gemma26b` — 같은 노드 · 이 셀 라벨(`gemma26b-ctx-single`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:509 |
| 20 | 2026-10-24T03:22:10Z | main | `budget_renew` | `camp-26102401-gemma26b` | budget_renew · floor_mib=47375 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-23T23:52:08Z 후 3h30m02s · 라벨 다름: `camp-26102401-gemma26b` — 같은 노드 · 이 셀 라벨(`gemma26b-ctx-single`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:510 |
| 21 | 2026-10-24T03:50:45Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → camp-26102401-gemma26b 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-23T23:52:08Z 후 3h58m37s) · 라벨 다름: `camp-26102401-gemma26b` — 같은 노드 · 이 셀 라벨(`gemma26b-ctx-single`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:511 |
| 22 | 2026-10-24T03:50:46Z | main | `serve_stop` | — | serve_stop · model=gemma-4-26b-a4b · duration_s=14318 · session=serve-gemma26b-ctx-single-20261023T235208Z | docs/logs/main/events/2026-10.jsonl:512 |
| 23 | 2026-10-24T03:50:46Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → camp-26102401-gemma26b 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-23T23:52:09Z 후 3h58m37s) · 라벨 다름: `camp-26102401-gemma26b` — 같은 노드 · 이 셀 라벨(`gemma26b-ctx-single`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:750 |
| 24 | 2026-10-25T00:02:06Z | main | `budget_declare` | `serve-gemma26b-ctx-single` | 예산 선언(선언값 — 측정 아님) · floor_mib=53743 · weights_mib=49221 · kv_mib=3212 · overhead_mib=18432 · mem_total_mib=124608 · 기동 창 1/2: budget_renew 1회 · 닫힘 budget_clear 2026-10-25T00:09:42Z(선언 후 7m36s) · measured_utc 2026-10-25T00:44:08Z 는 이 창 밖(다른 기동 · 측정 전) | docs/logs/main/events/2026-10.jsonl:595 |
| 25 | 2026-10-25T00:02:06Z | main | `serve_start` | — | serve_start · model=gemma-4-26b-a4b · tensor_parallel_size=1 · topology=single · session=serve-gemma26b-ctx-single-20261025T000206Z | docs/logs/main/events/2026-10.jsonl:596 |
| 26 | 2026-10-25T00:02:06Z | main | `budget_honored` | `serve-gemma26b-ctx-single` | budget_honored · floor_mib=53743 · arm_ceiling_mib=50671 · remaining_s=7200 · 창 시작 2026-10-25T00:02:06Z 후 0m00s | docs/logs/main/events/watchdog.jsonl:805 |
| 27 | 2026-10-25T00:02:08Z | main | `budget_renew` | `serve-gemma26b-ctx-single` | budget_renew · floor_mib=53743 · ttl_s=7200 · remaining_before_s=7198 · 창 시작 2026-10-25T00:02:06Z 후 0m02s | docs/logs/main/events/2026-10.jsonl:597 |
| 28 | 2026-10-25T00:02:39Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=86372 · rate_mib_s=25208 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → serve-gemma26b-ctx-single 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-25T00:02:06Z 후 0m33s) | docs/logs/main/events/watchdog.jsonl:806 |
| 29 | 2026-10-25T00:09:42Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → serve-gemma26b-ctx-single 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-25T00:02:06Z 후 7m36s) | docs/logs/main/events/2026-10.jsonl:598 |
| 30 | 2026-10-25T00:09:42Z | main | `serve_stop` | — | serve_stop · model=gemma-4-26b-a4b · duration_s=456 · session=serve-gemma26b-ctx-single-20261025T000206Z | docs/logs/main/events/2026-10.jsonl:599 |
| 31 | 2026-10-25T00:09:43Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → serve-gemma26b-ctx-single 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-25T00:02:06Z 후 7m37s) | docs/logs/main/events/watchdog.jsonl:807 |
| 32 | 2026-10-25T00:11:00Z | main | `budget_declare` | `serve-gemma26b-ctx-single` | 예산 선언(선언값 — 측정 아님) · floor_mib=53743 · weights_mib=49221 · kv_mib=3212 · overhead_mib=18432 · mem_total_mib=124608 · 기동 창 2/2: budget_renew 17회 · 닫힘 budget_clear 2026-10-25T00:44:20Z(선언 후 33m20s) · measured_utc 2026-10-25T00:44:08Z 가 이 창 안(이 측정의 기동) | docs/logs/main/events/2026-10.jsonl:601 |
| 33 | 2026-10-25T00:11:00Z | main | `serve_start` | — | serve_start · model=gemma-4-26b-a4b · tensor_parallel_size=1 · topology=single · session=serve-gemma26b-ctx-single-20261025T001100Z | docs/logs/main/events/2026-10.jsonl:602 |
| 34 | 2026-10-25T00:11:00Z | main | `budget_honored` | `serve-gemma26b-ctx-single` | budget_honored · floor_mib=53743 · arm_ceiling_mib=50671 · remaining_s=7200 · 창 시작 2026-10-25T00:11:00Z 후 0m00s | docs/logs/main/events/watchdog.jsonl:808 |
| 35 | 2026-10-25T00:11:01Z | main | `budget_renew` | `serve-gemma26b-ctx-single` | budget_renew · floor_mib=53743 · ttl_s=7200 · remaining_before_s=7199 · 창 시작 2026-10-25T00:11:00Z 후 0m01s | docs/logs/main/events/2026-10.jsonl:603 |
| 36 | 2026-10-25T00:11:16Z | main | `engine_log_window_start` | — | 캡처된 엔진 로그의 첫 시각(꼬리 캡처일 수 있다 — 엔진 시작 시각이 아닐 수 있다) · 선언 2026-10-25T00:11:00Z 후 0m16s · 시계: 컨테이너 시각을 UTC 로 읽어 측정 창 [2026-10-25T00:11:00Z, 2026-10-25T00:44:20Z] 안에 드는 것을 확인 | output/single/benchlog/sweep_gemma26b-ctx-single/lite_engine_gemma26b-ctx-single.log:36 |
| 37 | 2026-10-25T00:11:33Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=77433 · rate_mib_s=29684 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → serve-gemma26b-ctx-single 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-25T00:11:00Z 후 0m33s) | docs/logs/main/events/watchdog.jsonl:809 |
| 38 | 2026-10-25T00:11:34Z | main | `engine_prefetch_disabled` | — | 가중치 auto-prefetch 꺼짐 줄 ×1 — filesystem CIFS(vLLM 로그 문구) | output/single/benchlog/sweep_gemma26b-ctx-single/lite_engine_gemma26b-ctx-single.log:65 |
| 39 | 2026-10-25T00:16:00Z | main | `engine_weights_loaded` | — | 가중치 적재 완료 줄 ×1(rank·단계별) · 최장 265.25s · 이 행 = 마지막 줄 | output/single/benchlog/sweep_gemma26b-ctx-single/lite_engine_gemma26b-ctx-single.log:72 |
| 40 | 2026-10-25T00:16:02Z | main | `engine_model_loaded` | — | 모델 적재 완료 줄 ×1(rank 별) · 마지막 줄 48.54 GiB · 271.2s | output/single/benchlog/sweep_gemma26b-ctx-single/lite_engine_gemma26b-ctx-single.log:75 |
| 41 | 2026-10-25T00:16:26Z | main | `engine_kv_cache_sized` | — | GPU KV cache 267,398 tokens · 요청당 262,144 tokens 기준 최대 동시성 1.02x | output/single/benchlog/sweep_gemma26b-ctx-single/lite_engine_gemma26b-ctx-single.log:85 |
| 42 | 2026-10-25T00:16:38Z | main | `engine_init_done` | — | 엔진 초기화(profile·KV 할당·warmup) 36.16s | output/single/benchlog/sweep_gemma26b-ctx-single/lite_engine_gemma26b-ctx-single.log:102 |
| 43 | 2026-10-25T00:16:58Z | main | `server_starting` | — | API 서버 기동 — 이 측정 창 선언 2026-10-25T00:11:00Z 후 5m58s · 캡처 로그 첫 시각 2026-10-25T00:11:16Z 후 5m42s | output/single/benchlog/sweep_gemma26b-ctx-single/lite_engine_gemma26b-ctx-single.log:111 |
| 44 | 2026-10-25T00:17:56Z | main | `budget_renew` | `serve-gemma26b-ctx-single` | budget_renew · floor_mib=53743 · ttl_s=7200 · remaining_before_s=6785 · 창 시작 2026-10-25T00:11:00Z 후 6m56s | docs/logs/main/events/2026-10.jsonl:604 |
| 45 | 2026-10-25T00:21:28Z | main | `budget_renew` | `serve-gemma26b-ctx-single` | budget_renew · floor_mib=53743 · ttl_s=7200 · remaining_before_s=6988 · 창 시작 2026-10-25T00:11:00Z 후 10m28s | docs/logs/main/events/2026-10.jsonl:605 |
| 46 | 2026-10-25T00:24:52Z | main | `budget_renew` | `serve-gemma26b-ctx-single` | budget_renew · floor_mib=53743 · ttl_s=7200 · remaining_before_s=6996 · 창 시작 2026-10-25T00:11:00Z 후 13m52s | docs/logs/main/events/2026-10.jsonl:606 |
| 47 | 2026-10-25T00:28:16Z | main | `budget_renew` | `serve-gemma26b-ctx-single` | budget_renew · floor_mib=53743 · ttl_s=7200 · remaining_before_s=6996 · 창 시작 2026-10-25T00:11:00Z 후 17m16s | docs/logs/main/events/2026-10.jsonl:607 |
| 48 | 2026-10-25T00:30:15Z | main | `budget_renew` | `serve-gemma26b-ctx-single` | budget_renew · floor_mib=53743 · ttl_s=7200 · remaining_before_s=7081 · 창 시작 2026-10-25T00:11:00Z 후 19m15s | docs/logs/main/events/2026-10.jsonl:608 |
| 49 | 2026-10-25T00:32:13Z | main | `budget_renew` | `serve-gemma26b-ctx-single` | budget_renew · floor_mib=53743 · ttl_s=7200 · remaining_before_s=7082 · 창 시작 2026-10-25T00:11:00Z 후 21m13s | docs/logs/main/events/2026-10.jsonl:609 |
| 50 | 2026-10-25T00:34:12Z | main | `budget_renew` | `serve-gemma26b-ctx-single` | budget_renew · floor_mib=53743 · ttl_s=7200 · remaining_before_s=7081 · 창 시작 2026-10-25T00:11:00Z 후 23m12s | docs/logs/main/events/2026-10.jsonl:610 |
| 51 | 2026-10-25T00:35:38Z | main | `budget_renew` | `serve-gemma26b-ctx-single` | budget_renew · floor_mib=53743 · ttl_s=7200 · remaining_before_s=7114 · 창 시작 2026-10-25T00:11:00Z 후 24m38s | docs/logs/main/events/2026-10.jsonl:611 |
| 52 | 2026-10-25T00:37:03Z | main | `budget_renew` | `serve-gemma26b-ctx-single` | budget_renew · floor_mib=53743 · ttl_s=7200 · remaining_before_s=7115 · 창 시작 2026-10-25T00:11:00Z 후 26m03s | docs/logs/main/events/2026-10.jsonl:612 |
| 53 | 2026-10-25T00:38:28Z | main | `budget_renew` | `serve-gemma26b-ctx-single` | budget_renew · floor_mib=53743 · ttl_s=7200 · remaining_before_s=7115 · 창 시작 2026-10-25T00:11:00Z 후 27m28s | docs/logs/main/events/2026-10.jsonl:613 |
| 54 | 2026-10-25T00:39:32Z | main | `budget_renew` | `serve-gemma26b-ctx-single` | budget_renew · floor_mib=53743 · ttl_s=7200 · remaining_before_s=7136 · 창 시작 2026-10-25T00:11:00Z 후 28m32s | docs/logs/main/events/2026-10.jsonl:614 |
| 55 | 2026-10-25T00:40:36Z | main | `budget_renew` | `serve-gemma26b-ctx-single` | budget_renew · floor_mib=53743 · ttl_s=7200 · remaining_before_s=7136 · 창 시작 2026-10-25T00:11:00Z 후 29m36s | docs/logs/main/events/2026-10.jsonl:615 |
| 56 | 2026-10-25T00:41:01Z | main | `budget_renew` | `serve-gemma26b-ctx-single` | budget_renew · floor_mib=53743 · ttl_s=7200 · remaining_before_s=7175 · 창 시작 2026-10-25T00:11:00Z 후 30m01s | docs/logs/main/events/2026-10.jsonl:616 |
| 57 | 2026-10-25T00:41:40Z | main | `budget_renew` | `serve-gemma26b-ctx-single` | budget_renew · floor_mib=53743 · ttl_s=7200 · remaining_before_s=7161 · 창 시작 2026-10-25T00:11:00Z 후 30m40s | docs/logs/main/events/2026-10.jsonl:617 |
| 58 | 2026-10-25T00:42:29Z | main | `budget_renew` | `serve-gemma26b-ctx-single` | budget_renew · floor_mib=53743 · ttl_s=7200 · remaining_before_s=7151 · 창 시작 2026-10-25T00:11:00Z 후 31m29s | docs/logs/main/events/2026-10.jsonl:618 |
| 59 | 2026-10-25T00:43:19Z | main | `budget_renew` | `serve-gemma26b-ctx-single` | budget_renew · floor_mib=53743 · ttl_s=7200 · remaining_before_s=7150 · 창 시작 2026-10-25T00:11:00Z 후 32m19s | docs/logs/main/events/2026-10.jsonl:619 |
| 60 | 2026-10-25T00:44:20Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → serve-gemma26b-ctx-single 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-25T00:11:00Z 후 33m20s) | docs/logs/main/events/2026-10.jsonl:620 |
| 61 | 2026-10-25T00:44:20Z | main | `serve_stop` | — | serve_stop · model=gemma-4-26b-a4b · duration_s=2000 · session=serve-gemma26b-ctx-single-20261025T001100Z | docs/logs/main/events/2026-10.jsonl:621 |
| 62 | 2026-10-25T00:44:21Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → serve-gemma26b-ctx-single 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-25T00:11:00Z 후 33m21s) | docs/logs/main/events/watchdog.jsonl:810 |

**기동 시도**(예산 선언 `budget_declare` 마다 한 시도 · 같은 노드의 이어지는 행과 선언 없는 노드(엔진 로그)의 그 시각 행을 묶었다 · 사살 · 트립 · 거부는 닫힘과 따로 센다 · `measurement` 행은 측정 기록이라 시도로 세지 않는다(위 표에만) — 해석은 저작자 몫)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 1 | main | `camp-26102401-gemma26b`(라벨 다름 — 같은 노드 · 이 셀 라벨 아님) | 2026-10-23T21:48:33Z | 1회 · 첫 2026-10-23T21:48:34Z | 없음 | `budget_clear` 2026-10-23T22:03:55Z | 5 |
| 2 | main | `camp-26102401-gemma26b`(라벨 다름 — 같은 노드 · 이 셀 라벨 아님) | 2026-10-23T23:52:08Z | 8회 · 첫 2026-10-23T23:52:10Z | 없음 | `budget_clear` 2026-10-24T03:50:45Z | 5 |
| 3 | main | `serve-gemma26b-ctx-single` | 2026-10-25T00:02:06Z | 1회 · 첫 2026-10-25T00:02:08Z | 없음 | `budget_clear` 2026-10-25T00:09:42Z | 5 |
| 4 | main | `serve-gemma26b-ctx-single` | 2026-10-25T00:11:00Z | 17회 · 첫 2026-10-25T00:11:01Z | 없음 | `budget_clear` 2026-10-25T00:44:20Z | 12 |
<!-- /FACT:event_timeline -->

<!-- FACT:measurement_env -->
_측정 실행의 env 관측 없음 — 측정 엔진 로그에서 env echo 줄(`<KEY> set by environment to <VALUE>` 등)을 찾지 못했다(evidence.measurement_env_observed · 부재 ≠ 미설정)._
<!-- /FACT:measurement_env -->

> 이 절이 답하는 질문: 수신자가 이 zip 만으로 이 셀을 다시 띄우려면 어떤 순서로 무엇을 하고, 각 단계의 성공을 무엇으로 판정하며, 얼마나 기다려야 하는가?

이 절 · 02 의 `lite_engine L<n>` 은 측정 기동(시도 4)의 엔진 로그 `output/single/benchlog/sweep_gemma26b-ctx-single/lite_engine_gemma26b-ctx-single.log` 의 줄 번호다(캠페인 기동의 같은 이름 파일 `output/single/benchlog/lite_engine_gemma26b-ctx-single.log` 가 아니다).

**(a) 사전 준비** — 가중치는 NAS 를 compose 가 `/app/models` 에 읽기 전용으로 마운트해 직접 읽는다(compose L18 · `NAS_MODEL_PATH` 는 `.env.template` 의 `<manifest.nas_model_path>` · 획득 모드 managed — plan_26102401 §안전 경계). 스테이징할 파일은 없다 — PLE mmap 을 쓰지 않는다(셀 env 에 `VLLM_PLE_MMAP` 없음 · 1.2). 위치 조건: 측정 엔진은 체크포인트 파일시스템을 CIFS 로 인식해 auto-prefetch 를 끄고(lite_engine L64 · L65) 가중치 적재에 265.25 s 를 썼다(L72) — 로컬 디스크나 NFS/Lustre 면 이 소요가 달라진다(추론). 필수 env 의 자리: 셀 env(`artifacts/triplet/.env.gemma26b-ctx-single` — `CONFIG_FILE` · `SERVING_PORT` · `SERVING_MODEL_NAME` · `IMAGE_TAG` · `TIKTOKEN_ENABLED`)와 프로젝트 env 형상(`artifacts/compose/.env.template` — 마운트 변수 4개). 측정 실행의 env echo 는 관측되지 않았다(위 사실 블록 · 부재 ≠ 미설정) — 셀 env 는 측정 당시 바이트이고(1.1 `verified`), 측정 뒤 재생성된 env 형상은 없다.

**(b) 빌드** — 메인 노드 1대에서 `Dockerfile.source-build` 선택자로 짓는다(위 1단계 명령 — docker history build-arg 에서 재구성 · `BUILD_JOBS=8` 은 Dockerfile 기본값 16 과 다르다 · L48). 사실 블록의 38분 19초는 층 CreatedAt 창이지 소요가 아니다. 계보 밖 testlog_26101412(§2 P4)는 이 이미지의 메인 빌드 2차를 rc=0 · `01:08:32→01:48:59Z` 로 적고, 1차는 v0.31.0 이 `use_existing_torch.py` 를 `tools/` 로 옮겨 실패했다고 적는다 — 실린 Dockerfile L82 ~ L84 가 그 자기적응 단계다.

**(c) 기동 순서와 성공 판정** — `single_serve_up.sh gemma26b-ctx-single`(위 2단계 · 사용법에서 파생). 판정은 health 200 + 추론 1회다(위 기동 성공 판정 행 · 측정 실행의 post_health JSON). 측정 기동(시도 4)의 관측 소요: 선언 00:11:00Z → 엔진 로그 첫 줄 00:11:16Z → 가중치 적재 완료 00:16:00Z(265.25 s) → 모델 적재 48.54 GiB · 271.2 s → KV 267,398 tokens(00:16:26Z) → 엔진 초기화 36.16 s → API 서버 기동 00:16:58Z(선언 후 5분 58초 · 01 §1.4 표 #36 ~ #43). 캠페인 기동은 health 200 까지 470 s 로 적혔다(testlog_26102412 §1) — 두 값은 다른 기동 회차다. 이 셀은 런타임 패치가 없으므로 패치 발화 서명은 확인할 것이 없다. 기동 시도 표는 4개다 — 시도 1 · 2 는 캠페인 라벨(시도 1 = W2 포트 결함 · 시도 2 = needle · lite 회차), 시도 3 은 측정 없이 7분 36초에 닫혔고 원인은 계보에 **미기록**이다(02 Q2), 시도 4 가 이 측정이다. 정상인데 무서워 보이는 것: ① 엔진 로그 머리의 `WARNING: The NVIDIA Driver was not detected` — 바로 앞 줄이 `50-gpu-driver-check.sh: line 5: ldconfig: command not found` 이고(lite_engine L28 ~ L30) 같은 실행이 KV 할당 · CUDA graph 캡처를 마쳤으므로 엔트리포인트 점검 스크립트의 오탐으로 보인다(추론) ② 로드 중 `watchdog_highrate_hold`(표 #37 · 보류 · 사살 아님) ③ `Failed to import the DeepSelect extension (vllm._deepselect_C)`(L88 · 이 셀은 그 경로 없이 서빙됐다) ④ 첫 요청의 `Triton kernel JIT compilation during inference: kernel_unified_attention`(L148 · 지연 스파이크 경고).

**(d) 호스트 메모리 안전 예산** — 측정 기동 선언은 `floor_mib 53743 = mem_total 124608 − weights 49221 − kv 3212 − overhead 18432` 이고(선언값 · 2026-10.jsonl L601) 무장 상한은 50671 MiB 다(표 #34). 엔진 로그의 적재 실측은 가중치 48.54 GiB(lite_engine L75) 하나다 — KV 3.14 GiB 는 `kv_cache_memory_bytes` 클램프를 엔진이 되읽은 값이고(L84 · 03 §3.1 사실 블록도 '측정 아님'), 예약 합계 51.68 GiB 는 두 값의 합이다. 선언 weights 49221 MiB 와 실측 48.54 GiB 는 다른 생산자의 값이다. 벤치 진입 게이트는 `floor − bench_budget ≥ abs_band` 이고 이 셀 값으로 `budget_preflight.py --floor-mib 53743 --bench-budget-mib 4096` 을 저작 시 실행하면 `bench=OK`(53743 − 4096 = 49647 ≥ 10240 · bench 예산 상한 43503)다. 이 셀의 bench 예산이 4096(스윕 색인 bench_knobs)인지 2048(S5 testlog 머리)인지는 기록이 갈린다 — 둘 다 게이트를 통과한다(02 Q3).

**(e) 측정 명령** — 위 3단계 명령은 bench JSON 에서 재구성한 lite 레그 명령이고 실행 원문이 아니다. 판정점(GuideLLM)의 도구 인자는 03 §3.4 의 측정 도구 원문 스냅샷을 본다.

**(f) 엔진이 스스로 고른 런타임 경로** — attention: Gemma4 의 이질 헤드 차원(sliding 256 · full 512) 때문에 FA4 가 없다며 TRITON_ATTN 을 강제(L48 · L58) · MoE: `FlashInfer CUTLASS Unquantized MoE backend`(L62 · L74) · KV: fp8 저장(L45) · `max_num_batched_tokens` 를 2048 → 2496 으로 올림(멀티모달 prefix-LM 때문 · L46) · `--disable_chunked_mm_input` 강제(L50) · KV 클램프 경로라 메모리 프로파일링 생략(L84) · torch.compile AOT 캐시 적중(L81) · CUDA graph PIECEWISE 51 · FULL 35 캡처(L98 · L99) · 분산 백엔드는 world_size=1 의 nccl(L54 · 통신 없음) · 샘플링 기본값이 `generation_config.json` 의 temperature 1.0 · top_k 64 · top_p 0.95 로 덮였다(L110 · GuideLLM 호출에는 temperature 인자가 없다 · 03 §3.4).

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

- 체크포인트: `google/gemma-4-26B-A4B-it` · revision `01e5b3ee840d3a9e0b0b493c593e85398a30ef75`(00 §0.2 · 체크포인트 git HEAD = origin/main). 크기 51,611,872,412 B(safetensors index `total_size` · 엔진 로그 `Checkpoint size: 48.07 GiB`).
- 이 셀의 획득 모드는 관리 NAS 직독이다 — 다운로드 없이 NAS 마운트를 읽었다(plan_26102401 §안전 경계 "모델 획득은 NAS 마운트 직독(`model_source: managed`) — 다운로드 없음"). 싱글이라 노드 1대에만 있으면 된다.
- 네 환경에 없으면 사람이 획득 승인 게이트를 거쳐 받는다(무인 다운로드 ✗). 명령 예(추론 — 실행 검증 ✗ · 이 계보는 받지 않았다): `huggingface-cli download google/gemma-4-26B-A4B-it --revision 01e5b3ee840d3a9e0b0b493c593e85398a30ef75 --local-dir <manifest.nas_model_path>/Google/Gemma_4/gemma-4-26B-A4B-it` — compose 가 `<manifest.nas_model_path>` 를 `/app/models` 로 마운트하므로 yaml `model:` 경로(`/app/models/Google/Gemma_4/gemma-4-26B-A4B-it`)와 하위 경로가 맞아야 한다(compose L18 · yaml L8).

### 스테이징 생성

해당 없음 — 이 체크포인트에는 PLE 키가 없고(00 §0.9 후보 표 근거 칸 · config 관측) 셀 env 에 `VLLM_PLE_MMAP` 이 없어 PLE mmap 스테이징을 쓰지 않는다(측정 색인 `ple_mode: resident` · `defaulted(envfile VLLM_PLE_MMAP absent/0 = stock resident)`). 런타임 패치 · 빌드 패치도 없다(1.1). compose 의 tiktoken 마운트(`TIKTOKEN_HOST_PATH` · 기본 `./tiktoken_cache`)는 셀 env `TIKTOKEN_ENABLED=true` 로 러너가 env 를 export 하지만, 이 모델이 tiktoken 파일을 필요로 하는지는 계보가 적지 않았다(미확인 · compose L20 주석은 gpt-oss 용이라고 적는다).

### 네트워크가 필요한 단계

- NGC 베이스 pull `nvcr.io/nvidia/pytorch:26.07-py3`(Dockerfile L37) · vLLM 소스 클론 `https://github.com/vllm-project/vllm.git` @ `v0.31.0`(L76 ~ L78) · 빌드 중 pip 설치(build-system requires · constraint 경유 의존 · `ray==2.48.0` — L89 · L194 · L227) — 오프라인이면 베이스 이미지 · 소스 트리 · wheel 캐시를 미리 받아 둔다(추론 — 오프라인 빌드는 이 계보가 검증하지 않았다).
- 측정 도구 이미지 `ghcr.io/vllm-project/guidellm:v0.7.3`(digest `sha256:e3ad2371bfa8e42f2c3d1251b62d0d9c9706c27ae2143c8c048eb5fc6aebb558` · 레벨 1 bench_tool JSON) — 미리 pull 해 둔다. 측정 컨테이너는 `HF_HUB_OFFLINE=1` 로 돌고 토크나이저는 스테이징 사본을 쓴다(run_bench.sh 스냅샷 L439 ~ L449).
- 가중치 다운로드(위 · 사람 승인 게이트).
