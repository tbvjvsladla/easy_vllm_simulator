# 01 · 산출물 — 무엇이 실제로 쓰였고, 어떻게 다시 띄우는가

## 1.1 적용 판정 표
> 기계가 적용 증거(빌드 원장 · 원장이 없으면 라벨된 재구성) × 파일 × 선언을 대사한 결과다. `artifacts/` 에는 **적용된 것만** 실렸다 — 빌드 때 스스로 skip 된 패치와 쓰이지 않은 평면의 파일은 표에만 남고 싣지 않는다.

<!-- FACT:slots -->
| 슬롯 | 실림 | 파일 | 신호 | 적용 증거 | 사유 |
|---|---|---|---|---|---|
| `build_patch_post` | 안 실림 | — | 1-signal | none:no-build-ledger | 적용 판정 없음 — 원장 부재 — 라벨된 재구성: applied 0 · skip 판정 = docker-history+gate(기계 파생) |
| `build_patch_pre` | 안 실림 | — | 1-signal | none:no-build-ledger | 적용 판정 없음 — 원장 부재 — 라벨된 재구성: applied 0 · skip 판정 = docker-history+gate(기계 파생) |
| `build_recipe` | 실림 | `artifacts/build_recipe/Dockerfile.source-build` · `artifacts/build_recipe/requirements.txt` | 2-signal | file:image-history:docker history build-arg 관측(VLLM_REF=v0.26.0 · VLLM_VERSION 없음 → 소스빌드 Dockerfile) — 셀 env 선택자 없음 · output/single/docker-compose.yaml 리터럴 build.dockerfile: Dockerfile.source-build(선택자 변수 없음 — compose build 는 늘 Dockerfile.source-build 를 짓는다 · 이 이미지를 지은 증거 아님) | 기존 source-build 이미지의 선택된 Dockerfile과 requirements 참고 사본이다. Dockerfile history 일치와 requirements 불일치의 증거 지위는 1.1 표 그대로다. 새 빌드를 실행하지 않았으며, upstream native 의존/핀 검증이 다른 환경에서 다시 필요하다. |
| `compose` | 실림 | `artifacts/compose/.env.template` · `artifacts/compose/docker-compose.yaml` | 2-signal | file:output/single/docker-compose.yaml | compose는 NAS read-only 마운트와 GPU 컨테이너·모델별 runner를 연결하고 triplet은 BF16 가중치와 TurboQuant KV·qwen3 reasoning/hermes tool 파서를 설정한다. 실제 기능 스모크와 Full이 이 형상에서 응답했다. arm_patch.sh는 대상 c-tq4nc_patch.py가 없어 no-op이며 별도 runtime patch는 실리지 않았다. build_patch pre/post와 fork_pin도 실린 파일이 없다. 이를 모든 환경에서 패치 불필요라는 주장으로 바꾸지 않는다. |
| `fork_pin` | 안 실림 | — | 1-signal | none:.claude/policies/arch_variant_ledger.json | 셀 env 에 VARIANT 줄 없음 = stock(workflow.md §변종 좌표의 거처)(기계 파생) |
| `runtime_patch` | 안 실림 | — | 1-signal | none:output/single/configs/c-tq4nc_patch.py | output/single/configs/c-tq4nc_patch.py 없음 — arm_patch.sh 는 no-op(런타임 패치 불해당)(기계 파생) |
| `triplet` | 실림 | `artifacts/triplet/.env.c-tq4nc` · `artifacts/triplet/c-tq4nc.sh` · `artifacts/triplet/c-tq4nc.yaml` | 2-signal | file:output/single/configs/c-tq4nc.yaml | 셀 id c-tq4nc = 트리플렛 이름(configs/c-tq4nc.yaml · .sh · envs/.env.c-tq4nc)(기계 파생) |

**레시피 경고 · 실린 리비전**(기계 관측 — 수신자 빌드가 여기서 갈라질 수 있다)

- ⚠ `build_recipe` `output/single/requirements.txt` — 이미지 안 COPY 대상 바이트가 실린 작업트리 바이트와 다르다(일치하는 개정 없음) · 이미지 빌드 뒤 바뀐 레시피일 수 있다 — 마지막 커밋이 이미지 Created 뒤(이미지 Created 2026-09-07T19:43:27Z · 마지막 커밋 2026-10-14T01:51:45Z)
- `build_recipe` `output/single/Dockerfile.source-build`(이 체크아웃 통로 `output/single/Dockerfile.source-build` 대신) 실린 리비전: 커밋 `80284db86dd7` · 선택 방법 `git log --all -- output/*/Dockerfile.source-build · 80284db86dd7:output/single/Dockerfile.source-build 가 docker history RUN·COPY 와 전수 일치(16/16 · 작업트리 13/17) · 커밋 2026-09-08T06:19:07Z > 이미지 Created 2026-09-07T19:43:27Z(빌드 뒤 커밋된 같은 단계) — 이 개정의 바이트를 싣는다(git cat-file)` · docker history 대조 16/16 · 실린 바이트 `git:80284db86dd7bc6dbb0d27141c0708cd0e7f4deb` · 쓰인 바이트임을 관측으로 확인 예
- `build_recipe` `output/single/requirements.txt` 실린 리비전: 리비전 미특정 · 선택 방법 `worktree(이미지 안 바이트와 다르고 이미지 Created 이전 개정도 같지 않다 — recipe_vs_image 경고 · 이미지 sha256 0f6f8d20779dbb22)` · docker history 대조 없음 · 실린 바이트 `worktree` · 쓰인 바이트임을 관측으로 확인 아니오
- `compose` `output/single/docker-compose.yaml` 실린 리비전: 커밋 `80284db86dd7` · 선택 방법 `worktree(mtime 2026-10-25T22:53:44Z ≤ 측정 2026-10-27T10:05:40Z(sweep_index.generated_utc) · 바이트 = 측정 전 마지막 커밋 80284db86dd7)` · docker history 대조 없음 · 실린 바이트 `worktree` · 근거 `mtime≤measured` · 쓰인 바이트임을 관측으로 확인 예

**싣지 않은 파일**(슬롯별 사유 — 빌드 때 skip 된 패치는 아래 적용 집합 표에 있다)

- `build_patch_post` known-non-patch 1건: `.gitkeep`
- `build_patch_pre` known-non-patch 1건: `.gitkeep`
- `compose` `arm_patch.sh` — `not-armed(output/single/configs/c-tq4nc_patch.py 없음(CONFIG_FILE) — arm_patch.sh 는 no-op · arm_patch.sh 측정 당시 개정(fad2d7c6f157) `_cfg=` 키 체인 RUNTIME_PATCH_STEM > CONFIG_FILE > 'default')`

**파일별 검증 · 관련성**(`generated-unverified` = 실행 검증되지 않은 생성물 — 파일 첫 줄 경고 · PAYLOAD 파일 기록과 같은 표시 · 관련성 = ① 패치 선언 대상 × 모델 config ② 엔진 로그 발화 서명 — 둘 다 관측일 때만 required/inactive-inferred)

| 슬롯 | 파일 | 검증 | 검증 근거 | 관련성 | 관련성 근거 |
|---|---|---|---|---|---|
| `build_recipe` | `artifacts/build_recipe/Dockerfile.source-build` | `verified` · cell-run | 쓰인 레시피(image-history:docker history build-arg 관측(VLLM_REF=v0.26.0 · VLLM_VERSION 없음 → 소스빌드 Dockerfile) — 셀 env 선택자 없음 · output/single/docker-compose.yaml 리터럴 build.dockerfile: Dockerfile.source-build(선택자 변수 없음 — compose build 는 늘 Dockerfile.source-build 를 짓는다 · 이 이미지를 지은 증거 아님)) — 개정 선택 slots.build_recipe.evidence.selected_revisions | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `build_recipe` | `artifacts/build_recipe/requirements.txt` | `generated-unverified` · renderer | 실린 requirements.txt(worktree · sha256 ff5b5c72731459c9)이 측정 이미지 안 COPY 대상 바이트와 다르다 — 이 이미지를 지은 바이트임을 관측으로 확인하지 못했다(참고 형상)(_select_copied · worktree(이미지 안 바이트와 다르고 이미지 Created 이전 개정도 같지 않다 — recipe_vs_image 경고 · 이미지 sha256 0f6f8d20779dbb22)) | `unknown` | 이 셀은 이 경로로 실행되지 않았다(generated-unverified · 생성 renderer) — 실행 기록이 없어 필요 여부를 판정하지 않는다(참고 형상) |
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

### build_recipe
기존 source-build 이미지의 선택된 Dockerfile과 requirements 참고 사본이다. Dockerfile history 일치와 requirements 불일치의 증거 지위는 1.1 표 그대로다. 새 빌드를 실행하지 않았으며, upstream native 의존/핀 검증이 다른 환경에서 다시 필요하다.

### compose · triplet
compose는 NAS read-only 마운트와 GPU 컨테이너·모델별 runner를 연결하고 triplet은 BF16 가중치와 TurboQuant KV·qwen3 reasoning/hermes tool 파서를 설정한다. 실제 기능 스모크와 Full이 이 형상에서 응답했다. arm_patch.sh는 대상 c-tq4nc_patch.py가 없어 no-op이며 별도 runtime patch는 실리지 않았다. build_patch pre/post와 fork_pin도 실린 파일이 없다. 이를 모든 환경에서 패치 불필요라는 주장으로 바꾸지 않는다.

### 기타 파일
이 셀은 Docker single 실행이므로 native 설치/멀티 Ray worker 파일은 해당하지 않는다. requirements generated-unverified 경고는 그대로 유지한다.

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

모든 값은 이전 레시피에서 유지한 inherited다. 이번에는 파라미터 최적화나 음성대조를 실행하지 않았다.

```hint-event
id: "V1"
kind: "value-status"
노브: "gpu-memory-utilization"
값: "0.9"
지위: "inherited"
근거: "기존 c-tq4nc recipe 유지; 새 Phase2 수렴/필요조건 대조를 주장하지 않음"
출처: ["devlog_26102718_Qwen3_4B_c_tq4nc_신규실측_서사 §레시피·모델·이미지"]
```

```hint-event
id: "V2"
kind: "value-status"
노브: "max-model-len"
값: "32768"
지위: "inherited"
근거: "기존 c-tq4nc recipe 유지; 새 Phase2 수렴/필요조건 대조를 주장하지 않음"
출처: ["devlog_26102718_Qwen3_4B_c_tq4nc_신규실측_서사 §레시피·모델·이미지"]
```

```hint-event
id: "V3"
kind: "value-status"
노브: "max-num-seqs"
값: "8"
지위: "inherited"
근거: "기존 c-tq4nc recipe 유지; 새 Phase2 수렴/필요조건 대조를 주장하지 않음"
출처: ["devlog_26102718_Qwen3_4B_c_tq4nc_신규실측_서사 §레시피·모델·이미지"]
```

```hint-event
id: "V4"
kind: "value-status"
노브: "kv-cache-memory-bytes"
값: "11811160064"
지위: "inherited"
근거: "기존 c-tq4nc recipe 유지; 새 Phase2 수렴/필요조건 대조를 주장하지 않음"
출처: ["devlog_26102718_Qwen3_4B_c_tq4nc_신규실측_서사 §레시피·모델·이미지"]
```

```hint-event
id: "V5"
kind: "value-status"
노브: "kv-cache-dtype"
값: "turboquant_4bit_nc"
지위: "inherited"
근거: "기존 c-tq4nc recipe 유지; 새 Phase2 수렴/필요조건 대조를 주장하지 않음"
출처: ["devlog_26102718_Qwen3_4B_c_tq4nc_신규실측_서사 §레시피·모델·이미지"]
```

## 1.4 재현 절차
> 셀 실행 기록에서 기계가 파생한 명령 순서와 단계별 소요다. 소요가 `미관측` 인 단계는 시각 기록이 없다는 뜻이다(0 이 아니다). 그 아래는 블랙박스 이벤트 원장에서 이 셀의 행을 기계가 뽑아 기동 시도로 묶은 것이고, 마지막은 측정한 실행의 엔진 로그가 echo 한 env 다(측정 당시 값의 정본).

<!-- FACT:reproduce -->
| 순서 | 단계 | 성공 판정 | 소요 | 관측 구간(UTC) | 소요 출처 | 명령 출처 |
|---|---|---|---|---|---|---|
| 1 | build | 빌드 OK · 이미지 실재 | 층 창 26분 20초(docker history 첫 층 → 끝 층 CreatedAt · 캐시 재사용 층 포함 — 빌드 소요 아님) | 2026-09-07T19:17:07Z → 2026-09-07T19:43:27Z (layer-window) | docker history --no-trunc --format '{{json .}}' sha256:629942890c0570c8f1c3235bf6927ae4eb315e30336cd68551fdf474a60a44f7 · 우리 Dockerfile 층 22개의 CreatedAt 첫 층 → 끝 층(캐시 재사용 층은 이전 빌드 시각 — 빌드 소요 아님) | reconstructed(docker-history build-args + compose build) |
| 2 | serve | health 200 + 추론 1회(스모크 판정) | ≤ 3분 44초(상한)(끝 = 최종 스윕 첫 측정 · 그 전 요청 3건 — 소요 출처 칸) | 2026-10-27T09:40:09Z → 2026-10-27T09:43:53Z (upper) | docs/logs/sub/sessions/qwen3_4b_camp26102705_a01.json started_utc(측정을 포함하는 세션) → 첫 측정 시각(output/single/benchlog/sweep_c-tq4nc_camp-26102705_qwen_a01_20261027T093346Z/lite_raw_c-tq4nc.json measured_utc(호스트 date -u · lite_bench.sh 가 cold 레그 직전에 찍는 측정 시작)) · ⚠ 최종 스윕 기준 — 관측된 serve 세션 경계 안 첫 측정 전 completions 요청 3건 관측(상한 시각 2026-10-27T09:43:03Z~2026-10-27T09:43:43Z · 스모크 최소 추론 1회 초과(요청 목적·앞선 벤치 여부는 미관측) · 세션 docs/logs/sub/sessions/qwen3_4b_camp26102705_a01.json · 출처 output/single/benchlog/sweep_c-tq4nc_camp-26102705_qwen_a01_20261027T093346Z/lite_engine_head_c-tq4nc.log · output/single/benchlog/sweep_c-tq4nc_camp-26102705_qwen_a01_20261027T093346Z/lite_engine_c-tq4nc.log 의 POST 접근 줄 × 전후 vLLM 시각 줄(컨테이너 시계 UTC 가정 · 구간을 확인한 요청만, 세션 전체 요청 수 아님)) | derived(스모크 사용법) |
| 3 | bench | 리포트 발행(루브릭 권한 explicit ∧ PASS일 때만 full 인증서) | 21분 47초(최종 스윕 기준 · 그 전 요청 3건 — 소요 출처 칸) | 2026-10-27T09:43:53Z → 2026-10-27T10:05:40Z (exact) | output/single/benchlog/sweep_c-tq4nc_camp-26102705_qwen_a01_20261027T093346Z/lite_raw_c-tq4nc.json measured_utc(호스트 date -u · lite_bench.sh 가 cold 레그 직전에 찍는 측정 시작) → sweep_index.generated_utc(호스트 UTC · 두 끝 같은 시계) · ⚠ 최종 스윕 기준 — 관측된 serve 세션 경계 안 첫 측정 전 completions 요청 3건 관측(상한 시각 2026-10-27T09:43:03Z~2026-10-27T09:43:43Z · 스모크 최소 추론 1회 초과(요청 목적·앞선 벤치 여부는 미관측) · 세션 docs/logs/sub/sessions/qwen3_4b_camp26102705_a01.json · 출처 output/single/benchlog/sweep_c-tq4nc_camp-26102705_qwen_a01_20261027T093346Z/lite_engine_head_c-tq4nc.log · output/single/benchlog/sweep_c-tq4nc_camp-26102705_qwen_a01_20261027T093346Z/lite_engine_c-tq4nc.log 의 POST 접근 줄 × 전후 vLLM 시각 줄(컨테이너 시계 UTC 가정 · 구간을 확인한 요청만, 세션 전체 요청 수 아님)) | reconstructed(bench json + 측정 도구 원문 lite 인자) |

**빌드 층 시각**(관측 · 소요가 아니라 층 창 — 캐시 재사용 층은 이전 빌드의 시각을 가진다)

- `build` 이미지 층 CreatedAt(docker history): 2026-09-07T19:17:07Z → 2026-09-07T19:43:27Z · 층 22개 · 출처 `docker history --no-trunc --format '{{json .}}' sha256:629942890c0570c8f1c3235bf6927ae4eb315e30336cd68551fdf474a60a44f7` — 우리 Dockerfile 층의 CreatedAt 범위 — 캐시 재사용 층은 이전 빌드 시각을 가진다(소요 아님)

**단계별 명령**(기계 파생 — `명령 출처` 가 `derived(…)` 면 사용법에서 파생 · `reconstructed(…)` 면 괄호 안 관측(docker history build-arg · compose build · bench JSON 필드)에서 재구성한 명령이다. 어느 쪽도 실행 원문(로그에 남은 줄)은 아니다)

**1. build**

```sh
# 실린 artifacts/build_recipe/Dockerfile.source-build(이미지를 지은 개정: git:80284db86dd7bc6dbb0d27141c0708cd0e7f4deb(output/single/Dockerfile.source-build))를 output/single/Dockerfile.source-build 자리에 두고 컨텍스트에서 짓는다.
mkdir -p output/single/build_patches_src output/single/build_patches   # 빌드 원장 context_inventory 미관측 — 빌드 때 내용 모름(실린 패치 없음) · 빌드 원장 context_inventory 미관측 — 빌드 때 내용 모름(실린 패치 없음)
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
# lite_cold/lite_warm 의 데이터셋 · 부하 인자 = 측정 시점 도구 원문 lite_bench.sh@a7083a8f933d L177-L183(리터럴 인자만 · `$변수` 인자는 도구가 값을 정하지 않아 JSON 필드로 채운다) — `total_*_tokens ÷ completed` 는 실측 토큰(chat 템플릿 포함)이지 인자가 아니다.
# output/single/benchlog/sweep_c-tq4nc_camp-26102705_qwen_a01_20261027T093346Z/lite_cold_c-tq4nc.json (date 20261027-094406) · 실측 토큰(chat 템플릿 포함 · total ÷ completed · 인자 아님): 입력 520/1 = 520 · 출력 128/1 = 128
vllm bench serve --backend openai-chat --model qwen3-4b --tokenizer /app/models/Qwen/Qwen3/Qwen3-4B --trust-remote-code --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 --ignore-eos --num-prompts 1 --max-concurrency 1 --request-rate inf
# output/single/benchlog/sweep_c-tq4nc_camp-26102705_qwen_a01_20261027T093346Z/lite_warm_c-tq4nc.json (date 20261027-094437) · 실측 토큰(chat 템플릿 포함 · total ÷ completed · 인자 아님): 입력 1560/3 = 520 · 출력 384/3 = 128
vllm bench serve --backend openai-chat --model qwen3-4b --tokenizer /app/models/Qwen/Qwen3/Qwen3-4B --trust-remote-code --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 --ignore-eos --num-prompts 3 --max-concurrency 1 --request-rate inf
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
| (실린 파일 없음 — 빈 디렉터리 생성) | `build_patches_src/` | Dockerfile.source-build COPY build_patches_src/ · 실린 패치 없음 — 빈 디렉터리를 만든다(없으면 COPY 단계에서 빌드가 멈춘다) · 빌드 원장 context_inventory 미관측 — 빌드 때 내용 모름(실린 패치 없음) |
| (실린 파일 없음 — 빈 디렉터리 생성) | `build_patches/` | Dockerfile.source-build COPY build_patches/ · 실린 패치 없음 — 빈 디렉터리를 만든다(없으면 COPY 단계에서 빌드가 멈춘다) · 빌드 원장 context_inventory 미관측 — 빌드 때 내용 모름(실린 패치 없음) |

**기동 성공 판정(관측 · 발행 자격)**: health 200 = true · 추론 1회 = true · 근거 `output/single/benchlog/sweep_c-tq4nc_camp-26102705_qwen_a01_20261027T093346Z/level_01/post_health_c-tq4nc.json` · `output/single/benchlog/sweep_c-tq4nc_camp-26102705_qwen_a01_20261027T093346Z/level_01/guidellm_c-tq4nc.json`
<!-- /FACT:reproduce -->

<!-- FACT:event_timeline -->
**블랙박스 이벤트 34행**(원장에서 이 셀 label·config 에 맞는 행과 그와 시간이 겹치는 같은 노드의 다른 라벨 창 · 시각순 · 기계 발췌 · 이번 캠페인 7행 · 같은 셀 이름 · 앞선 캠페인 27행(내용 칸에 표기) · 라벨 다름(같은 노드 · 창 밖 이 셀 행과 시간이 겹치는 다른 라벨 선언 창) 12행 — 셀 라벨 일치 집계에서 제외(세션·호출 기록의 귀속 확인과 별도))

> **캠페인 경계** 2026-10-26T21:46:47Z(`camp-26102705-codex-single8` · evidence.event_campaign_boundary): campaigns/camp-26102705-codex-single8/campaign.yaml declared_utc 2026-10-26T21:46:47Z → 경계 = 앞선 값 2026-10-26T21:46:47Z(이전 행 = 같은 셀 이름 · 앞선 캠페인)

> **원장 관측 범위**(노드별 첫 행 → 마지막 행 · evidence.event_ledger_spans): sub = 2026-06-13T22:25:01Z → 2026-10-27T10:07:20Z(`docs/logs/sub/events/2026-06.jsonl` · `docs/logs/sub/events/2026-07.jsonl` · `docs/logs/sub/events/2026-09.jsonl` · `docs/logs/sub/events/2026-10.jsonl` · `docs/logs/sub/events/thermal.jsonl` · `docs/logs/sub/events/watchdog.jsonl`)

| # | UTC | 노드 | kind | label | 내용 | 출처 |
|---|---|---|---|---|---|---|
| 1 | 2026-09-07T20:06:36Z | sub | `budget_declare` | `serve-c-tq4nc` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=101578 · weights_mib=7672 · kv_mib=11264 · overhead_mib=4096 · mem_total_mib=124610(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 기동 창 1/2: budget_renew 3회 · 닫힘 budget_clear 2026-09-07T21:12:30Z(선언 후 1h05m54s) · measured_utc 2026-10-27T10:05:40Z 는 이 창 밖(다른 기동 · 측정 전) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-09.jsonl:135 |
| 2 | 2026-09-07T20:06:36Z | sub | `serve_start` | — | serve_start · model=qwen3-4b · tensor_parallel_size=1 · topology=single · session=serve-c-tq4nc-20260907T200636Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-09.jsonl:136 |
| 3 | 2026-09-07T20:06:37Z | sub | `budget_renew` | `serve-c-tq4nc` | budget_renew · floor_mib=101578 · ttl_s=7200 · remaining_before_s=7199 · 창 시작 2026-09-07T20:06:36Z 후 0m01s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-09.jsonl:137 |
| 4 | 2026-09-07T20:06:37Z | sub | `budget_honored` | `serve-c-tq4nc` | budget_honored · floor_mib=101578 · arm_ceiling_mib=98506 · remaining_s=7199 · 창 시작 2026-09-07T20:06:37Z 후 0m00s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:80 |
| 5 | 2026-09-07T20:36:37Z | sub | `budget_renew` | `serve-c-tq4nc` | budget_renew · floor_mib=101578 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-09-07T20:06:36Z 후 30m01s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-09.jsonl:138 |
| 6 | 2026-09-07T21:06:37Z | sub | `budget_renew` | `serve-c-tq4nc` | budget_renew · floor_mib=101578 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-09-07T20:06:36Z 후 1h00m01s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-09.jsonl:139 |
| 7 | 2026-09-07T21:12:30Z | sub | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → serve-c-tq4nc 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-07T20:06:36Z 후 1h05m54s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-09.jsonl:140 |
| 8 | 2026-09-07T21:12:30Z | sub | `serve_stop` | — | serve_stop · model=qwen3-4b · duration_s=3954 · session=serve-c-tq4nc-20260907T200636Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-09.jsonl:141 |
| 9 | 2026-09-07T21:12:31Z | sub | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → serve-c-tq4nc 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-07T20:06:37Z 후 1h05m54s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:81 |
| 10 | 2026-09-08T06:39:09Z | sub | `budget_declare` | `camp-26090814.cell-c-tq4nc.rerun` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=101578 · weights_mib=7672 · kv_mib=11264 · overhead_mib=4096 · mem_total_mib=124610(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 라벨 다름: `camp-26090814.cell-c-tq4nc.rerun` — 같은 노드 · 이 셀 라벨(`c-tq4nc`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · budget_renew 3회 · 닫힘 budget_clear 2026-09-08T07:41:39Z(선언 후 1h02m30s) · measured_utc 2026-10-27T10:05:40Z 는 이 창 밖(측정 전) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-09.jsonl:150 |
| 11 | 2026-09-08T06:39:09Z | sub | `serve_start` | — | serve_start · model=qwen3-4b · tensor_parallel_size=1 · topology=single · session=serve-c-tq4nc-20260908T063909Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-09.jsonl:151 |
| 12 | 2026-09-08T06:39:09Z | sub | `budget_honored` | `camp-26090814.cell-c-tq4nc.rerun` | budget_honored · floor_mib=101578 · arm_ceiling_mib=98506 · remaining_s=7200 · 창 시작 2026-09-08T06:39:09Z 후 0m00s · 라벨 다름: `camp-26090814.cell-c-tq4nc.rerun` — 같은 노드 · 이 셀 라벨(`c-tq4nc`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:84 |
| 13 | 2026-09-08T06:39:10Z | sub | `budget_renew` | `camp-26090814.cell-c-tq4nc.rerun` | budget_renew · floor_mib=101578 · ttl_s=7200 · remaining_before_s=7199 · 창 시작 2026-09-08T06:39:09Z 후 0m01s · 라벨 다름: `camp-26090814.cell-c-tq4nc.rerun` — 같은 노드 · 이 셀 라벨(`c-tq4nc`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-09.jsonl:152 |
| 14 | 2026-09-08T07:09:10Z | sub | `budget_renew` | `camp-26090814.cell-c-tq4nc.rerun` | budget_renew · floor_mib=101578 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-09-08T06:39:09Z 후 30m01s · 라벨 다름: `camp-26090814.cell-c-tq4nc.rerun` — 같은 노드 · 이 셀 라벨(`c-tq4nc`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-09.jsonl:153 |
| 15 | 2026-09-08T07:39:10Z | sub | `budget_renew` | `camp-26090814.cell-c-tq4nc.rerun` | budget_renew · floor_mib=101578 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-09-08T06:39:09Z 후 1h00m01s · 라벨 다름: `camp-26090814.cell-c-tq4nc.rerun` — 같은 노드 · 이 셀 라벨(`c-tq4nc`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-09.jsonl:154 |
| 16 | 2026-09-08T07:41:39Z | sub | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → camp-26090814.cell-c-tq4nc.rerun 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-08T06:39:09Z 후 1h02m30s) · 라벨 다름: `camp-26090814.cell-c-tq4nc.rerun` — 같은 노드 · 이 셀 라벨(`c-tq4nc`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-09.jsonl:155 |
| 17 | 2026-09-08T07:41:39Z | sub | `serve_stop` | — | serve_stop · model=qwen3-4b · duration_s=3750 · session=serve-c-tq4nc-20260908T063909Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-09.jsonl:156 |
| 18 | 2026-09-08T07:41:39Z | sub | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → camp-26090814.cell-c-tq4nc.rerun 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-08T06:39:09Z 후 1h02m30s) · 라벨 다름: `camp-26090814.cell-c-tq4nc.rerun` — 같은 노드 · 이 셀 라벨(`c-tq4nc`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:85 |
| 19 | 2026-10-10T05:33:14Z | sub | `budget_declare` | `serve-c-tq4nc` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=87240 · weights_mib=7672 · kv_mib=11264 · overhead_mib=18432 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 기동 창 2/2: budget_renew 2회 · 닫힘 budget_clear 2026-10-10T06:26:39Z(선언 후 53m25s) · measured_utc 2026-10-27T10:05:40Z 는 이 창 밖(다른 기동 · 측정 전) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:239 |
| 20 | 2026-10-10T05:33:14Z | sub | `serve_start` | — | serve_start · model=qwen3-4b · tensor_parallel_size=1 · topology=single · session=serve-c-tq4nc-20261010T053314Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:240 |
| 21 | 2026-10-10T05:33:14Z | sub | `budget_honored` | `serve-c-tq4nc` | budget_honored · floor_mib=87240 · arm_ceiling_mib=84168 · remaining_s=7200 · 창 시작 2026-10-10T05:33:14Z 후 0m00s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:532 |
| 22 | 2026-10-10T05:33:15Z | sub | `budget_renew` | `serve-c-tq4nc` | budget_renew · floor_mib=87240 · ttl_s=7200 · remaining_before_s=7199 · 창 시작 2026-10-10T05:33:14Z 후 0m01s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:241 |
| 23 | 2026-10-10T06:03:15Z | sub | `budget_renew` | `serve-c-tq4nc` | budget_renew · floor_mib=87240 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-10T05:33:14Z 후 30m01s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:242 |
| 24 | 2026-10-10T06:26:21Z | 미관측 | `measurement` | `measurement (budget window recorded)` | 같은 셀의 다른 측정(측정 노드 축 sub) · 시각 = 측정 조립(sweep generated_utc = 인증서 measured_utc · 시작 시각 미기록) · 이 발행의 측정 2026-10-27T10:05:40Z 전 · 발행 기록 camp_26101104_hint_cd__sub__c_tq4nc(셀 대조 simlog:sweep_index.meta.config_name · verdict PASS) · simlog 사본 generated_utc 2026-10-10T06:26:21Z(같음) / 출력 평면에 보존된 같은 셀의 다른 측정 · 원장: 이 셀 선언 창 안: serve-c-tq4nc 2026-10-10T05:33:14Z~2026-10-10T06:26:39Z (docs/logs/sub/events/2026-10.jsonl:239) · serve-c-tq4nc 2026-10-10T05:33:14Z~2026-10-10T06:26:39Z (docs/logs/sub/events/watchdog.jsonl:532) · 직전 예산 행 budget_renew(serve-c-tq4nc) 2026-10-10T06:03:15Z(docs/logs/sub/events/2026-10.jsonl:242) · 직후 예산 행 budget_clear 2026-10-10T06:26:39Z(docs/logs/sub/events/2026-10.jsonl:243) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/simlog/26101108_camp_26101104_hint_cd__sub__c_tq4nc/sweep_index.json:4 |
| 25 | 2026-10-10T06:26:39Z | sub | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → serve-c-tq4nc 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-10T05:33:14Z 후 53m25s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:243 |
| 26 | 2026-10-10T06:26:39Z | sub | `serve_stop` | — | serve_stop · model=qwen3-4b · duration_s=3205 · session=serve-c-tq4nc-20261010T053314Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:244 |
| 27 | 2026-10-10T06:26:39Z | sub | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → serve-c-tq4nc 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-10T05:33:14Z 후 53m25s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:533 |
| 28 | 2026-10-27T09:40:08Z | sub | `budget_declare` | `single8-qwen3-4b-a01` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=92164 · weights_mib=7672 · kv_mib=11264 · overhead_mib=13508 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 라벨 다름: `single8-qwen3-4b-a01` — 같은 노드 · 이 셀 라벨(`c-tq4nc`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · budget_renew 1회 · 닫힘 budget_clear 2026-10-27T10:06:26Z(선언 후 26m18s) · measured_utc 2026-10-27T10:05:40Z 가 이 창 시간 안 — 라벨 불일치만으로 이 측정·기동의 귀속 여부를 판단하지 않는다 | docs/logs/sub/events/2026-10.jsonl:675 |
| 29 | 2026-10-27T09:40:09Z | sub | `serve_start` | — | serve_start · model=qwen3-4b · tensor_parallel_size=1 · topology=single · session=qwen3_4b_camp26102705_a01 | docs/logs/sub/events/2026-10.jsonl:676 |
| 30 | 2026-10-27T09:40:09Z | sub | `budget_renew` | `single8-qwen3-4b-a01` | budget_renew · floor_mib=92164 · ttl_s=7200 · remaining_before_s=7199 · 창 시작 2026-10-27T09:40:08Z 후 0m01s · 라벨 다름: `single8-qwen3-4b-a01` — 같은 노드 · 이 셀 라벨(`c-tq4nc`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) | docs/logs/sub/events/2026-10.jsonl:677 |
| 31 | 2026-10-27T09:40:09Z | sub | `budget_honored` | `single8-qwen3-4b-a01` | budget_honored · floor_mib=92164 · arm_ceiling_mib=89092 · remaining_s=7199 · 창 시작 2026-10-27T09:40:09Z 후 0m00s · 라벨 다름: `single8-qwen3-4b-a01` — 같은 노드 · 이 셀 라벨(`c-tq4nc`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) | docs/logs/sub/events/watchdog.jsonl:640 |
| 32 | 2026-10-27T10:06:26Z | sub | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → single8-qwen3-4b-a01 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-27T09:40:08Z 후 26m18s) · 라벨 다름: `single8-qwen3-4b-a01` — 같은 노드 · 이 셀 라벨(`c-tq4nc`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) | docs/logs/sub/events/2026-10.jsonl:678 |
| 33 | 2026-10-27T10:06:27Z | sub | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → single8-qwen3-4b-a01 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-27T09:40:09Z 후 26m18s) · 라벨 다름: `single8-qwen3-4b-a01` — 같은 노드 · 이 셀 라벨(`c-tq4nc`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) | docs/logs/sub/events/watchdog.jsonl:641 |
| 34 | 2026-10-27T10:07:20Z | sub | `serve_stop` | — | serve_stop · model=qwen3-4b · duration_s=1631 · session=qwen3_4b_camp26102705_a01 | docs/logs/sub/events/2026-10.jsonl:680 |

**기동 시도**(예산 선언 `budget_declare` 마다 한 시도 · 같은 노드의 이어지는 행과 선언 없는 노드(엔진 로그)의 그 시각 행을 묶었다 · 사살 · 트립 · 거부는 닫힘과 따로 센다 · `measurement` 행은 측정 기록이라 시도로 세지 않는다(위 표에만) — 해석은 저작자 몫)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 1 | sub | `single8-qwen3-4b-a01`(라벨 다름 — 같은 노드 · 이 셀 라벨 아님) | 2026-10-27T09:40:08Z | 1회 · 첫 2026-10-27T09:40:09Z | 없음 | `budget_clear` 2026-10-27T10:06:26Z | 4 |

**캠페인 경계 이전 기동 3회**(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀 라벨로 묶인 2회 · 라벨 다름(같은 노드 · 창 밖 이 셀 행과 시간이 겹치는 다른 라벨 선언 창) 1회 · 어느 쪽도 이 셀의 시도로 세지 않는다 · 행 전문 = 01 §1.4)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 앞선-1 | sub | `serve-c-tq4nc` | 2026-09-07T20:06:36Z | 3회 · 첫 2026-09-07T20:06:37Z | 없음 | `budget_clear` 2026-09-07T21:12:30Z | 4 |
| 앞선-2 | sub | `camp-26090814.cell-c-tq4nc.rerun`(라벨 다름 — 같은 노드 · 이 셀 라벨 아님) | 2026-09-08T06:39:09Z | 3회 · 첫 2026-09-08T06:39:10Z | 없음 | `budget_clear` 2026-09-08T07:41:39Z | 4 |
| 앞선-3 | sub | `serve-c-tq4nc` | 2026-10-10T05:33:14Z | 2회 · 첫 2026-10-10T05:33:15Z | 없음 | `budget_clear` 2026-10-10T06:26:39Z | 4 |
<!-- /FACT:event_timeline -->

<!-- FACT:measurement_env -->
**측정 실행의 env 관측 1행**(측정한 실행의 엔진 로그가 echo 한 값 · 기계 발췌 — 측정 당시 값의 정본이다. 표에 없는 키는 로그가 echo 하지 않은 것이지 설정되지 않았다는 뜻이 아니다 · 측정 뒤 재생성된 env 형상은 01 §1.1 을 본다) — 그중 1행은 종류 `key-present`: 키가 프로세스 env 에 **있었다**는 관측뿐이고 값은 로그에 없다(값 미관측)

| # | 키 | 값 | 노드 | 종류 | 출처(첫 줄) | 관측 |
|---|---|---|---|---|---|---|
| 1 | `VLLM_ATTENTION_BACKEND` | (키 존재 · 값 미관측) | 미특정 | key-present | output/single/benchlog/sweep_c-tq4nc_camp-26102705_qwen_a01_20261027T093346Z/lite_engine_head_c-tq4nc.log:L43 | 로그 2개 |

- 값 주: 키 존재만 관측 · 값 미관측(vLLM 'Unknown vLLM environment variable detected' 경고 — 값은 로그에 없다)
<!-- /FACT:measurement_env -->

> 이 절이 답하는 질문: 수신자가 이 zip 만으로 이 셀을 다시 띄우려면 어떤 순서로 무엇을 하고, 각 단계의 성공을 무엇으로 판정하며, 얼마나 기다려야 하는가?

준비→빌드 입력 검토→서빙→스모크→lite/Full→종료 순서다. 1.4 FACT의 build 명령은 history 기반 재구성이며 이번 빌드 실행 원문이 아니다. requirements 불일치 때문에 zip만으로 동일 컴파일 결과를 보장하지 못한다. 이번에는 기존 로컬 이미지를 사용했다.

실제 공식 single-up에는 ready-max900과 overhead13508MiB를 명시했고 READY145초였다(devlog_26102718_Qwen3_4B_c_tq4nc_신규실측_서사 §기동과 기능). overhead는 baseline+allowance 선언이며 엔진 실측 overhead가 아니다. health200 및 completion/reasoning stop, tool_calls를 확인한 다음 lite와 GuideLLM을 실행했다. bench의21분47초 구간은 lite 직전부터 index 생성까지이고 wrapper 구간은 다르다. 이번 세션 시작/끝은09:40:09Z/10:07:20Z의 실제 session 파일에 있다. 수정된 producer는 같은 노드의 config/topology와 정규 session journal·정확한 start/stop event로 이 창을 결속한다. 다른 문자열의 budget label은 표에서 다른 라벨로 표시된다. budget_clear10:06:26Z는 예산 회수이고 session stop10:07:20Z와 다른 사건이다.

공식 down이 과거 열린 세션과 충돌하면 이번 session-id를 정확히 지정한다. 이번 targeted down rc0, 컨테이너0/port8000 닫힘을 확인했다(testlog_26102719_Qwen3_4B_c_tq4nc_신규_Full_판정 §정리). 다른 세션을 정리하지 않는다. 성공의 의미는 각 단계의 실제 증거이며 린트는 서빙을 대신하지 않는다.

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

### 가중치
관리 NAS의 기존 Qwen/Qwen3-4B를 read-only로 사용했다. 0.4의 checkpoint HEAD는 checkout 관측이며 모든 파일의 HF 바이트 일치는 미검증이다. 가중치 신규 획득 명령은 이번에 실행하지 않았다. 준비 점검에 사용한 명령은 `python3 .claude/skills/upstream-version-watch/scripts/check_smoke_model.py c-tq4nc --topology single --emit-gate-params`이며 출처는 devlog_26102718_Qwen3_4B_c_tq4nc_신규실측_서사 §레시피·모델·이미지 및 원시다. 가중치 부재면 사람 획득 게이트로 반환한다.

### 스테이징
PLE 별도 스테이징은 해당 없음: 이 dense Qwen3 recipe는 PLE 스테이징을 선언하지 않았다. tokenizer/config/세 샤드 존재를 점검했다. 빌드 컨텍스트 빈 patch 디렉터리는 1.4의 재구성 매핑을 확인하되 이번에 생성/재빌드하지 않았다.

### 네트워크
이번 serve는 기존 로컬 이미지와 NAS를 사용했다. 새 빌드를 한다면 Dockerfile의 base image·pip/source 획득에 네트워크나 사전 적재가 필요하다. 그 재빌드의 오프라인 절차는 이 실행에서 검증하지 않았다. 가중치 다운로드를 자동 지시하지 않는다. 외부검색/wiki/hint remote는 별도 근거·발행 통로다.
