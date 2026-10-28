# 01 · 산출물 — 무엇이 실제로 쓰였고, 어떻게 다시 띄우는가

## 1.1 적용 판정 표
> 기계가 적용 증거(빌드 원장 · 원장이 없으면 라벨된 재구성) × 파일 × 선언을 대사한 결과다. `artifacts/` 에는 재현 입력과 확인되지 않은 참고 형상도 포함될 수 있다. 파일별 검증 상태와 근거를 아래 표에서 확인한다.

<!-- FACT:slots -->
| 슬롯 | 실림 | 파일 | 신호 | 적용 증거 | 사유 |
|---|---|---|---|---|---|
| `build_patch_post` | 안 실림 | — | 1-signal | none:no-build-ledger | 적용 판정 없음 — 원장 부재 — 라벨된 재구성: applied 0 · skip 판정 = docker-history+gate(기계 파생) |
| `build_patch_pre` | 안 실림 | — | 1-signal | none:no-build-ledger | 적용 판정 없음 — 원장 부재 — 라벨된 재구성: applied 0 · skip 판정 = docker-history+gate(기계 파생) |
| `build_recipe` | 실림 | `artifacts/build_recipe/Dockerfile` · `artifacts/build_recipe/requirements.txt` | 2-signal | file:image-history:docker history build-arg 관측(VLLM_VERSION=0.18.0 · VLLM_REF 없음 → wheel Dockerfile) — 셀 env 선택자 없음 · output/single/docker-compose.yaml 리터럴 build.dockerfile: Dockerfile.source-build(이 파일만 단독 적용하고 override 가 없을 때 Dockerfile.source-build 선택 · 과거 빌드에 이 compose 또는 override 를 사용했는지는 미관측) · ⚠ compose 리터럴 Dockerfile.source-build ≠ 관측 Dockerfile(이미지 빌드에 사용된 compose 또는 override 여부는 미관측) | 이미지 history에서 wheel 입력을 관측했다. 실린 requirements 바이트는 이미지 안 사본과 달라 이 빌드의 바이트로 입증되지 않았다. 빌드 원장은 없어 패치 적용 여부를 단정할 수 없다. |
| `compose` | 실림 | `artifacts/compose/.env.template` · `artifacts/compose/docker-compose.yaml` | 2-signal | file:output/single/docker-compose.yaml | 이 셀의 serving 설정과 compose 경로다. 생성된 포트와 compose 포트의 불일치는 입력과 생성물에서 정정했다. |
| `fork_pin` | 안 실림 | — | 1-signal | none:.claude/policies/arch_variant_ledger.json | 셀 env 에 VARIANT 줄 없음 = stock(workflow.md §변종 좌표의 거처)(기계 파생) |
| `runtime_patch` | 안 실림 | — | 1-signal | none:output/single/configs/gpt-oss-20b-gb10_patch.py | output/single/configs/gpt-oss-20b-gb10_patch.py 없음 — arm_patch.sh 는 no-op(런타임 패치 불해당)(기계 파생) |
| `triplet` | 실림 | `artifacts/triplet/.env.gpt-oss-20b-gb10` · `artifacts/triplet/gpt-oss-20b-gb10.sh` · `artifacts/triplet/gpt-oss-20b-gb10.yaml` | 2-signal | file:output/single/configs/gpt-oss-20b-gb10.yaml | 셀 id gpt-oss-20b-gb10 = 트리플렛 이름(configs/gpt-oss-20b-gb10.yaml · .sh · envs/.env.gpt-oss-20b-gb10)(기계 파생) |

**레시피 경고 · 실린 리비전**(기계 관측 — 수신자 빌드가 여기서 갈라질 수 있다)

- ⚠ `build_recipe` `output/single/Dockerfile` — 이미지 빌드 뒤 바뀐 레시피일 수 있다 — 마지막 커밋이 이미지 Created 뒤(이미지 Created 2026-09-05T12:00:11Z · 마지막 커밋 2026-09-05T12:01:35Z)
- ⚠ `build_recipe` `output/single/requirements.txt` — 이미지 안 COPY 대상 바이트가 실린 작업트리 바이트와 다르다(일치하는 개정 없음) · 이미지 빌드 뒤 바뀐 레시피일 수 있다 — 마지막 커밋이 이미지 Created 뒤(이미지 Created 2026-09-05T12:00:11Z · 마지막 커밋 2026-10-14T01:51:45Z)
- `build_recipe` `output/single/Dockerfile` 실린 리비전: 리비전 미특정 · 선택 방법 `worktree(측정 이미지 docker history 의 RUN·COPY 와 전수 일치 · Dockerfile 전체 바이트는 미검증)` · docker history 대조 7/7 · 실린 바이트 `worktree` · 쓰인 바이트임을 관측으로 확인 아니오
- `build_recipe` `output/single/requirements.txt` 실린 리비전: 리비전 미특정 · 선택 방법 `worktree(이미지 안 바이트와 다르고 이미지 Created 이전 개정도 같지 않다 — recipe_vs_image 경고 · 이미지 sha256 d2645eefe2d4722e)` · docker history 대조 없음 · 실린 바이트 `worktree` · 쓰인 바이트임을 관측으로 확인 아니오
- `compose` `output/single/docker-compose.yaml` 실린 리비전: 커밋 `80284db86dd7` · 선택 방법 `worktree(mtime 2026-10-25T22:53:44Z ≤ 측정 2026-10-28T07:50:10Z(sweep_index.generated_utc) · 바이트 = 측정 전 마지막 커밋 80284db86dd7)` · docker history 대조 없음 · 실린 바이트 `worktree` · 근거 `mtime≤measured` · 쓰인 바이트임을 관측으로 확인 아니오

**싣지 않은 파일**(슬롯별 사유 — 빌드 때 skip 된 패치는 아래 적용 집합 표에 있다)

- `build_patch_post` known-non-patch 1건: `.gitkeep`
- `build_patch_pre` known-non-patch 1건: `.gitkeep`
- `compose` `arm_patch.sh` — `not-armed(output/single/configs/gpt-oss-20b-gb10_patch.py 없음(CONFIG_FILE) — arm_patch.sh 는 no-op · arm_patch.sh 측정 당시 개정(fad2d7c6f157) `_cfg=` 키 체인 RUNTIME_PATCH_STEM > CONFIG_FILE > 'default')`

**파일별 검증 · 관련성**(`generated-unverified` = 실행 검증되지 않은 생성물 — 파일 첫 줄 경고 · PAYLOAD 파일 기록과 같은 표시 · 관련성 = ① 패치 선언 대상 × 모델 config ② 엔진 로그 발화 서명 — 둘 다 관측일 때만 required/inactive-inferred)

| 슬롯 | 파일 | 검증 | 검증 근거 | 관련성 | 관련성 근거 |
|---|---|---|---|---|---|
| `build_recipe` | `artifacts/build_recipe/Dockerfile` | `generated-unverified` · renderer | 실린 Dockerfile(worktree) · docker history RUN·COPY 7/7 일치 — Dockerfile 전체 원본 바이트의 동일성은 확인하지 못했다(_select_dockerfile · worktree(측정 이미지 docker history 의 RUN·COPY 와 전수 일치 · Dockerfile 전체 바이트는 미검증)) | `unknown` | 파일의 실행 당시 바이트가 확인되지 않았다(generated-unverified · 생성 renderer) — 이 표시만으로 경로 실행 여부나 필요 여부를 판정하지 않는다(참고 형상) |
| `build_recipe` | `artifacts/build_recipe/requirements.txt` | `generated-unverified` · renderer | 실린 requirements.txt(worktree · sha256 ff5b5c72731459c9)이 측정 이미지 안 COPY 대상 바이트와 다르다 — 이 이미지를 지은 바이트임을 관측으로 확인하지 못했다(참고 형상)(_select_copied · worktree(이미지 안 바이트와 다르고 이미지 Created 이전 개정도 같지 않다 — recipe_vs_image 경고 · 이미지 sha256 d2645eefe2d4722e)) | `unknown` | 파일의 실행 당시 바이트가 확인되지 않았다(generated-unverified · 생성 renderer) — 이 표시만으로 경로 실행 여부나 필요 여부를 판정하지 않는다(참고 형상) |
| `compose` | `artifacts/compose/.env.template` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/docker-compose.yaml` | `generated-unverified` · renderer | docker-compose.yaml: worktree(mtime 2026-10-25T22:53:44Z ≤ 측정 2026-10-28T07:50:10Z(sweep_index.generated_utc) · 바이트 = 측정 전 마지막 커밋 80284db86dd7) — 측정 당시 실행 바이트는 별도 관측되지 않았다 | `unknown` | 파일의 실행 당시 바이트가 확인되지 않았다(generated-unverified · 생성 renderer) — 이 표시만으로 경로 실행 여부나 필요 여부를 판정하지 않는다(참고 형상) |
| `triplet` | `artifacts/triplet/.env.gpt-oss-20b-gb10` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/gpt-oss-20b-gb10.sh` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/gpt-oss-20b-gb10.yaml` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
<!-- /FACT:slots -->

<!-- FACT:applied_set -->
**적용 집합** — 상태 `unobservable` · 출처 `no-build-ledger` · 원장 없음: 아래 결과는 **라벨된 재구성**이다(관측 아님)

_빌드 패치 없음 — 이 이미지에 빌드 패치 슬롯 입력이 없다._

**원장 탐침**(순위: attestation → 메인 로컬 이미지 → 재구성)

| 순위 | 결과 | 대상 |
|---|---|---|
| attestation | absent | output/single/benchlog/attestation_gpt-oss-20b-gb10.json(부재) |
| local-image | skipped(node=sub — 서브 스캔 ✗ · 문서기반 회수만) | — |
| reconstruction | done | — |
| image-probe | done | sha256:b2c993d8bf628ad46100dd9bdaf260599f564aea3539fce9b7bbfb57ca91e54a |

**재구성 입력** — `docker-history+gate-rule · fail-loud-rule · image-probe`(docker history build-arg × 패치 자기게이트 규칙)

| build-arg | 값 |
|---|---|
| `CUDA_VERSION` | 130 |
| `RAY_VERSION` | 2.48.0 |
| `VLLM_MANYLINUX` | manylinux_2_35 |
| `VLLM_VERSION` | 0.18.0 |
<!-- /FACT:applied_set -->

## 1.2 슬롯별 적용 사유
> 이 절이 답하는 질문: 실린 산출물 하나하나는 무엇을 고치며, 왜 이 셀에 필요했고, 이 셀에서 실제로 발화했으며, 언제 불필요해지는가?

### build_recipe
이미지 history에서 wheel 입력을 관측했다. 실린 requirements 바이트는 이미지 안 사본과 달라 이 빌드의 바이트로 입증되지 않았다. 빌드 원장은 없어 패치 적용 여부를 단정할 수 없다.

### compose · triplet
이 셀의 serving 설정과 compose 경로다. 생성된 포트와 compose 포트의 불일치는 입력과 생성물에서 정정했다.

### patch · fork
빌드 패치 원장이 없어 적용 집합은 재구성이다. runtime patch 파일 부재는 해당 슬롯이 실행되지 않았다는 범위이며 상위 버전에서의 필요 여부는 미관측이다.

## 1.3 값의 지위표
> 서빙 설정의 노브 **전부**와 기계가 모은 후보 지위다(lockset 출처 · yaml 주석 · 엔진 기본값). 최종 지위는 아래 `kind: value-status` 블록이 정한다 — 표의 노브마다 블록이 정확히 하나 있어야 린터를 통과한다.

<!-- FACT:value_status -->
서빙 노브 6개 — 노브마다 `kind: value-status` 블록이 **정확히 1개** 있어야 한다(노브·값은 이 표의 글자 그대로 · 값 칸의 `\|` 는 `|` 로 적는다).

| 노브 | 값 | 후보 지위 | 후보 근거 | yaml 주석 | 출처 |
|---|---|---|---|---|---|
| `gpu-memory-utilization` | 0.85 | declared-requirement | lockset 출처 gmu_source=target_gmu · lockset provenance=explorer-phase2 | — | lockset.gmu_source=target_gmu |
| `max-model-len` | 131072 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `max-num-seqs` | 16 | inherited | lockset 출처 batch_source=hand-lever · lockset provenance=explorer-phase2 | — | lockset.batch_source=hand-lever |
| `kv-cache-memory-bytes` | 51539607552 | inherited | lockset 출처 kv_source=hand · lockset provenance=explorer-phase2 | — | lockset.kv_source=hand |
| `kv-cache-dtype` | fp8 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `quantization` | mxfp4 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
<!-- /FACT:value_status -->

> 이 절이 답하는 질문: 각 서빙 노브의 값은 이 셀에서 어떤 지위인가 — 조정됐나, 승계됐나, 음성대조로 일부러 둔 것인가, 엔진 기본값인가, 필요조건인가?

```hint-event
id: V1
kind: value-status
노브: gpu-memory-utilization
값: 0.85
지위: inherited
근거: 기존 재개 후보 손레버, 이 계보에서 비교 스윕 없음
출처: [plan_26102816_GPT_OSS_20B_재개_Full_hint §근거와 후보]
```

```hint-event
id: V2
kind: value-status
노브: max-model-len
값: 131072
지위: inherited
근거: 기존 후보 설정, 이 계보에서 비교 스윕 없음
출처: [plan_26102816_GPT_OSS_20B_재개_Full_hint §근거와 후보]
```

```hint-event
id: V3
kind: value-status
노브: max-num-seqs
값: 16
지위: inherited
근거: 동일 모델 과거 후보의 손레버, 이번 trial로 확인
출처: [plan_26102816_GPT_OSS_20B_재개_Full_hint §근거와 후보]
```

```hint-event
id: V4
kind: value-status
노브: kv-cache-memory-bytes
값: 51539607552
지위: inherited
근거: 동일 모델 과거 후보의 손레버, 이번 trial로 확인
출처: [plan_26102816_GPT_OSS_20B_재개_Full_hint §근거와 후보]
```

```hint-event
id: V5
kind: value-status
노브: kv-cache-dtype
값: fp8
지위: inherited
근거: 기존 재개 후보, 다른 KV dtype 비교 없음
출처: [plan_26102816_GPT_OSS_20B_재개_Full_hint §근거와 후보]
```

```hint-event
id: V6
kind: value-status
노브: quantization
값: mxfp4
지위: inherited
근거: 모델 체크포인트의 선언을 승계, 양자화 비교 없음
출처: [plan_26102816_GPT_OSS_20B_재개_Full_hint §근거와 후보]
```

각 후보는 설정 조합의 관측을 뜻한다. 특히 손레버 KV와 batch는 최적점이나 필요조건이 아니다.

## 1.4 재현 절차
> 셀 실행 기록에서 기계가 파생한 명령 순서와 단계별 소요다. 소요가 `미관측` 인 단계는 시각 기록이 없다는 뜻이다(0 이 아니다). 그 아래는 블랙박스 이벤트 원장에서 이 셀의 행을 기계가 뽑아 기동 시도로 묶은 것이고, 마지막은 측정한 실행의 엔진 로그가 echo 한 env 다(측정 당시 값의 정본).

<!-- FACT:reproduce -->
| 순서 | 단계 | 성공 판정 | 소요 | 관측 구간(UTC) | 소요 출처 | 명령 출처 |
|---|---|---|---|---|---|---|
| 1 | build | 빌드 OK · 이미지 실재 | 층 창 2분 47초(docker history 첫 층 → 끝 층 CreatedAt · 캐시 재사용 층 포함 — 빌드 소요 아님) | 2026-09-05T11:57:24Z → 2026-09-05T12:00:11Z (layer-window) | docker history --no-trunc --format '{{json .}}' sha256:b2c993d8bf628ad46100dd9bdaf260599f564aea3539fce9b7bbfb57ca91e54a · 우리 Dockerfile 층 10개의 CreatedAt 첫 층 → 끝 층(캐시 재사용 층은 이전 빌드 시각 — 빌드 소요 아님) | reconstructed(docker-history build-args + compose build 선언/설정(실행 관측 아님)) |
| 2 | serve | health 200 + 추론 1회(스모크 판정) | ≤ 3분 47초(상한)(끝 = 최종 스윕 첫 측정 · 그 전 요청 4건 — 소요 출처 칸) | 2026-10-28T07:31:54Z → 2026-10-28T07:35:41Z (upper) | docs/logs/sub/sessions/serve-gpt-oss-20b-gb10-a03-20261028T073154Z.json started_utc(측정을 포함하는 세션) → 첫 측정 시각(output/single/benchlog/sweep_gpt-oss-20b-gb10_camp26102705_resume_20261028T073540Z/lite_raw_gpt-oss-20b-gb10.json measured_utc(호스트 date -u · lite_bench.sh 가 cold 레그 직전에 찍는 측정 시작)) · ⚠ 최종 스윕 기준 — 관측된 serve 세션 경계 안 첫 측정 전 completions 요청 4건 관측(상한 시각 2026-10-28T07:34:43Z~2026-10-28T07:35:03Z · 스모크 최소 추론 1회 초과(요청 목적·앞선 벤치 여부는 미관측) · 세션 docs/logs/sub/sessions/serve-gpt-oss-20b-gb10-a03-20261028T073154Z.json · 출처 output/single/benchlog/sweep_gpt-oss-20b-gb10_camp26102705_resume_20261028T073540Z/lite_engine_head_gpt-oss-20b-gb10.log · output/single/benchlog/sweep_gpt-oss-20b-gb10_camp26102705_resume_20261028T073540Z/lite_engine_gpt-oss-20b-gb10.log 의 POST 접근 줄 × 전후 vLLM 시각 줄(컨테이너 시계 UTC 가정 · 구간을 확인한 요청만, 세션 전체 요청 수 아님)) | derived(스모크 사용법) |
| 3 | bench | 리포트 발행(루브릭 권한 explicit ∧ PASS일 때만 full 인증서) | 14분 29초(최종 스윕 기준 · 그 전 요청 4건 — 소요 출처 칸) | 2026-10-28T07:35:41Z → 2026-10-28T07:50:10Z (exact) | output/single/benchlog/sweep_gpt-oss-20b-gb10_camp26102705_resume_20261028T073540Z/lite_raw_gpt-oss-20b-gb10.json measured_utc(호스트 date -u · lite_bench.sh 가 cold 레그 직전에 찍는 측정 시작) → sweep_index.generated_utc(호스트 UTC · 두 끝 같은 시계) · ⚠ 최종 스윕 기준 — 관측된 serve 세션 경계 안 첫 측정 전 completions 요청 4건 관측(상한 시각 2026-10-28T07:34:43Z~2026-10-28T07:35:03Z · 스모크 최소 추론 1회 초과(요청 목적·앞선 벤치 여부는 미관측) · 세션 docs/logs/sub/sessions/serve-gpt-oss-20b-gb10-a03-20261028T073154Z.json · 출처 output/single/benchlog/sweep_gpt-oss-20b-gb10_camp26102705_resume_20261028T073540Z/lite_engine_head_gpt-oss-20b-gb10.log · output/single/benchlog/sweep_gpt-oss-20b-gb10_camp26102705_resume_20261028T073540Z/lite_engine_gpt-oss-20b-gb10.log 의 POST 접근 줄 × 전후 vLLM 시각 줄(컨테이너 시계 UTC 가정 · 구간을 확인한 요청만, 세션 전체 요청 수 아님)) | reconstructed(bench json + 측정 도구 원문 lite 인자) |

**빌드 층 시각**(관측 · 소요가 아니라 층 창 — 캐시 재사용 층은 이전 빌드의 시각을 가진다)

- `build` 이미지 층 CreatedAt(docker history): 2026-09-05T11:57:24Z → 2026-09-05T12:00:11Z · 층 10개 · 출처 `docker history --no-trunc --format '{{json .}}' sha256:b2c993d8bf628ad46100dd9bdaf260599f564aea3539fce9b7bbfb57ca91e54a` — 우리 Dockerfile 층의 CreatedAt 범위 — 캐시 재사용 층은 이전 빌드 시각을 가진다(소요 아님)

**단계별 명령**(기계 파생 — `명령 출처`가 `derived(…)`이면 사용법에서 파생하고 `reconstructed(…)`이면 괄호 안에 명시된 관측, 파일 선언·설정 또는 추론을 조합해 재구성한다. 예를 들어 docker history와 bench JSON은 기록된 관측이며 compose 파일은 선언·설정이다. 설정만으로 과거 compose build 실행 여부는 알 수 없다. 어느 명령도 실행 원문(로그에 남은 줄)은 아니다)

**1. build**

```sh
# 실린 artifacts/build_recipe/Dockerfile(실린 개정: worktree — 이미지를 지은 바이트임을 관측으로 확인 못함(docker history RUN·COPY 7/7))를 output/single/Dockerfile 자리에 두고 컨텍스트에서 짓는다.
docker build -f output/single/Dockerfile \
  --build-arg CUDA_VERSION=130 \
  --build-arg RAY_VERSION=2.48.0 \
  --build-arg VLLM_MANYLINUX=manylinux_2_35 \
  --build-arg VLLM_VERSION=0.18.0 \
  -t easy-vllm:0.18.0-cu130-aarch64-wheel \
  output/single
# 이 zip 재현 예시: 실린 output/single/docker-compose.yaml 파일만 단독 사용하고 override가 없으면 build.dockerfile 리터럴은 Dockerfile.source-build이다(실린 레시피 Dockerfile과 다름). 그 조건의 compose build는 위 예시와 동등 경로가 아니다. 위 docker build는 재현 권장 예시이며 과거 실행 경로는 미관측이다.
```

**2. serve**

```sh
bash .claude/skills/upstream-version-watch/scripts/single_serve_up.sh gpt-oss-20b-gb10
```

**3. bench**

```sh
# bench JSON 필드로 복원: backend · model_id→--model · tokenizer_id→--tokenizer · num_prompts · max_concurrency · request_rate · burstiness · 요청당 입출력 길이 = total_*_tokens ÷ completed(나누어떨어질 때만 — 평균이다 · 요청별 길이는 JSON 에 없다) · 나누어떨어지면 `--dataset-name random` 으로 추정했다(측정 도구가 그 꼴로 부른다 — 아래 도구 원문).
# JSON 에 없는 인자(--endpoint · --base-url · --ignore-eos · --num-warmups · --temperature · --seed · --random-range-ratio · --trust-remote-code)는 측정 도구가 정했다 — level_* = run_bench.sh · lite_cold/lite_warm = lite_bench.sh(.claude/skills/adversarial-benchmark/scripts/) · 셀 스윕 순서는 sweep_bench.sh.
# lite_cold/lite_warm 의 데이터셋 · 부하 인자 = 측정 시점 도구 원문 lite_bench.sh@a7083a8f933d L177-L183(리터럴 인자만 · `$변수` 인자는 도구가 값을 정하지 않아 JSON 필드로 채운다) — `total_*_tokens ÷ completed` 는 실측 토큰(chat 템플릿 포함)이지 인자가 아니다.
# output/single/benchlog/sweep_gpt-oss-20b-gb10_camp26102705_resume_20261028T073540Z/lite_cold_gpt-oss-20b-gb10.json (date 20261028-073551) · 실측 토큰(chat 템플릿 포함 · total ÷ completed · 인자 아님): 입력 512/1 = 512 · 출력 128/1 = 128
vllm bench serve --backend openai --model gpt-oss-20b --tokenizer /app/models/OpenAI/gpt-oss-20b --trust-remote-code --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 --ignore-eos --num-prompts 1 --max-concurrency 1 --request-rate inf
# output/single/benchlog/sweep_gpt-oss-20b-gb10_camp26102705_resume_20261028T073540Z/lite_warm_gpt-oss-20b-gb10.json (date 20261028-073609) · 실측 토큰(chat 템플릿 포함 · total ÷ completed · 인자 아님): 입력 1536/3 = 512 · 출력 384/3 = 128
vllm bench serve --backend openai --model gpt-oss-20b --tokenizer /app/models/OpenAI/gpt-oss-20b --trust-remote-code --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 --ignore-eos --num-prompts 3 --max-concurrency 1 --request-rate inf
```

**슬롯 → 빌드 컨텍스트 매핑**(빌드 컨텍스트 = `output/<토폴로지>/` · 원천 = 실린 Dockerfile 의 COPY · compose 의 env_file · volumes · command — `—` 는 빌드 컨텍스트 입력이 아니다)

| zip 경로 | 빌드 컨텍스트 자리 | 근거 |
|---|---|---|
| `artifacts/triplet/.env.gpt-oss-20b-gb10` | `envs/.env.gpt-oss-20b-gb10` | compose env_file envs/.env.${CONFIG_FILE}(셀 env) |
| `artifacts/triplet/gpt-oss-20b-gb10.sh` | `configs/gpt-oss-20b-gb10.sh` | compose volumes ./configs:/app/configs |
| `artifacts/triplet/gpt-oss-20b-gb10.yaml` | `configs/gpt-oss-20b-gb10.yaml` | compose volumes ./configs:/app/configs |
| `artifacts/build_recipe/Dockerfile` | `Dockerfile` | 이 zip 재현 예시의 docker build -f Dockerfile 입력 · build.context .(실린 compose 파일만 단독 사용하고 override가 없으면 build.dockerfile 리터럴은 Dockerfile.source-build이므로 이 파일과 다름; 과거 실행 경로는 미관측) |
| `artifacts/build_recipe/requirements.txt` | `requirements.txt` | Dockerfile COPY requirements.txt |
| `artifacts/compose/.env.template` | `.env` | compose 변수치환 프로젝트 .env(${…} 참조) |
| `artifacts/compose/docker-compose.yaml` | `docker-compose.yaml` | compose 파일(빌드 컨텍스트 루트) |

**기동 성공 판정(관측 · 발행 자격)**: health 200 = true · 추론 1회 = true · 근거 `output/single/benchlog/sweep_gpt-oss-20b-gb10_camp26102705_resume_20261028T073540Z/level_01/post_health_gpt-oss-20b-gb10.json` · `output/single/benchlog/sweep_gpt-oss-20b-gb10_camp26102705_resume_20261028T073540Z/level_01/guidellm_gpt-oss-20b-gb10.json`
<!-- /FACT:reproduce -->

<!-- FACT:event_timeline -->
**블랙박스 이벤트 68행**(원장에서 이 셀 label·config 에 맞는 행만 · 시각순 · 기계 발췌 · 이번 캠페인 14행 · 같은 셀 이름 · 앞선 캠페인 54행(내용 칸에 표기))

> **캠페인 경계** 2026-10-26T21:46:47Z(`camp-26102705-codex-single8` · evidence.event_campaign_boundary): campaigns/camp-26102705-codex-single8/campaign.yaml declared_utc 2026-10-26T21:46:47Z · 이 발행이 묶인 측정을 서빙한 budget_declare 창 시작 2026-10-28T07:31:54Z → 경계 = 앞선 값 2026-10-26T21:46:47Z(이전 행 = 같은 셀 이름 · 앞선 캠페인)

> **원장 관측 범위**(노드별 첫 행 → 마지막 행 · evidence.event_ledger_spans): sub = 2026-06-13T22:25:01Z → 2026-10-28T07:50:46Z(`docs/logs/sub/events/2026-06.jsonl` · `docs/logs/sub/events/2026-07.jsonl` · `docs/logs/sub/events/2026-09.jsonl` · `docs/logs/sub/events/2026-10.jsonl` · `docs/logs/sub/events/thermal.jsonl` · `docs/logs/sub/events/watchdog.jsonl`)

| # | UTC | 노드 | kind | label | 내용 | 출처 |
|---|---|---|---|---|---|---|
| 1 | 2026-09-17T09:27:12Z | sub | `budget_declare` | `smoke-gpt-oss-20b-gb10` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=96543 · weights_mib=6561 · kv_mib=5120 · overhead_mib=16384 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 기동 창 1/14: budget_renew 0회 · 닫힘 budget_clear 2026-09-17T09:29:34Z(선언 후 2m22s) · measured_utc 2026-10-28T07:50:10Z 는 이 창 밖(다른 기동 · 측정 전) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-09.jsonl:448 |
| 2 | 2026-09-17T09:27:13Z | sub | `budget_honored` | `smoke-gpt-oss-20b-gb10` | budget_honored · floor_mib=96543 · arm_ceiling_mib=93471 · remaining_s=7199 · 창 시작 2026-09-17T09:27:13Z 후 0m00s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:277 |
| 3 | 2026-09-17T09:29:34Z | sub | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-gpt-oss-20b-gb10 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-17T09:27:12Z 후 2m22s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-09.jsonl:449 |
| 4 | 2026-09-17T09:29:34Z | sub | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-gpt-oss-20b-gb10 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-17T09:27:13Z 후 2m21s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:278 |
| 5 | 2026-09-17T09:48:01Z | sub | `budget_declare` | `smoke-gpt-oss-20b-gb10` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=96543 · weights_mib=6561 · kv_mib=5120 · overhead_mib=16384 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 기동 창 2/14: budget_renew 0회 · 닫힘 budget_clear 2026-09-17T09:50:27Z(선언 후 2m26s) · measured_utc 2026-10-28T07:50:10Z 는 이 창 밖(다른 기동 · 측정 전) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-09.jsonl:451 |
| 6 | 2026-09-17T09:48:02Z | sub | `budget_honored` | `smoke-gpt-oss-20b-gb10` | budget_honored · floor_mib=96543 · arm_ceiling_mib=93471 · remaining_s=7199 · 창 시작 2026-09-17T09:48:02Z 후 0m00s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:279 |
| 7 | 2026-09-17T09:50:27Z | sub | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-gpt-oss-20b-gb10 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-17T09:48:01Z 후 2m26s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-09.jsonl:452 |
| 8 | 2026-09-17T09:50:28Z | sub | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-gpt-oss-20b-gb10 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-17T09:48:02Z 후 2m26s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:280 |
| 9 | 2026-09-17T10:07:12Z | sub | `budget_declare` | `smoke-gpt-oss-20b-gb10` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=96543 · weights_mib=6561 · kv_mib=5120 · overhead_mib=16384 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 기동 창 3/14: budget_renew 0회 · 닫힘 budget_clear 2026-09-17T10:09:31Z(선언 후 2m19s) · measured_utc 2026-10-28T07:50:10Z 는 이 창 밖(다른 기동 · 측정 전) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-09.jsonl:454 |
| 10 | 2026-09-17T10:07:13Z | sub | `budget_honored` | `smoke-gpt-oss-20b-gb10` | budget_honored · floor_mib=96543 · arm_ceiling_mib=93471 · remaining_s=7199 · 창 시작 2026-09-17T10:07:13Z 후 0m00s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:281 |
| 11 | 2026-09-17T10:09:31Z | sub | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-gpt-oss-20b-gb10 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-17T10:07:12Z 후 2m19s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-09.jsonl:455 |
| 12 | 2026-09-17T10:09:32Z | sub | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-gpt-oss-20b-gb10 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-17T10:07:13Z 후 2m19s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:282 |
| 13 | 2026-09-17T10:30:12Z | sub | `budget_declare` | `smoke-gpt-oss-20b-gb10` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=96543 · weights_mib=6561 · kv_mib=5120 · overhead_mib=16384 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 기동 창 4/14: budget_renew 0회 · 닫힘 budget_clear 2026-09-17T10:32:33Z(선언 후 2m21s) · measured_utc 2026-10-28T07:50:10Z 는 이 창 밖(다른 기동 · 측정 전) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-09.jsonl:457 |
| 14 | 2026-09-17T10:30:13Z | sub | `budget_honored` | `smoke-gpt-oss-20b-gb10` | budget_honored · floor_mib=96543 · arm_ceiling_mib=93471 · remaining_s=7199 · 창 시작 2026-09-17T10:30:13Z 후 0m00s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:283 |
| 15 | 2026-09-17T10:32:33Z | sub | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-gpt-oss-20b-gb10 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-17T10:30:12Z 후 2m21s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-09.jsonl:458 |
| 16 | 2026-09-17T10:32:34Z | sub | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-gpt-oss-20b-gb10 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-17T10:30:13Z 후 2m21s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:284 |
| 17 | 2026-09-17T10:44:24Z | sub | `budget_declare` | `smoke-gpt-oss-20b-gb10` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=96543 · weights_mib=6561 · kv_mib=5120 · overhead_mib=16384 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 기동 창 5/14: budget_renew 0회 · 닫힘 budget_clear 2026-09-17T10:46:45Z(선언 후 2m21s) · measured_utc 2026-10-28T07:50:10Z 는 이 창 밖(다른 기동 · 측정 전) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-09.jsonl:460 |
| 18 | 2026-09-17T10:44:25Z | sub | `budget_honored` | `smoke-gpt-oss-20b-gb10` | budget_honored · floor_mib=96543 · arm_ceiling_mib=93471 · remaining_s=7199 · 창 시작 2026-09-17T10:44:25Z 후 0m00s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:285 |
| 19 | 2026-09-17T10:46:45Z | sub | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-gpt-oss-20b-gb10 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-17T10:44:24Z 후 2m21s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-09.jsonl:461 |
| 20 | 2026-09-17T10:46:46Z | sub | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-gpt-oss-20b-gb10 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-17T10:44:25Z 후 2m21s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:286 |
| 21 | 2026-09-17T10:53:41Z | sub | `budget_declare` | `smoke-gpt-oss-20b-gb10` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=96543 · weights_mib=6561 · kv_mib=5120 · overhead_mib=16384 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 기동 창 6/14: budget_renew 0회 · 닫힘 budget_clear 2026-09-17T10:56:00Z(선언 후 2m19s) · measured_utc 2026-10-28T07:50:10Z 는 이 창 밖(다른 기동 · 측정 전) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-09.jsonl:463 |
| 22 | 2026-09-17T10:53:41Z | sub | `budget_honored` | `smoke-gpt-oss-20b-gb10` | budget_honored · floor_mib=96543 · arm_ceiling_mib=93471 · remaining_s=7200 · 창 시작 2026-09-17T10:53:41Z 후 0m00s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:287 |
| 23 | 2026-09-17T10:56:00Z | sub | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-gpt-oss-20b-gb10 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-17T10:53:41Z 후 2m19s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-09.jsonl:464 |
| 24 | 2026-09-17T10:56:01Z | sub | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-gpt-oss-20b-gb10 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-17T10:53:41Z 후 2m20s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:288 |
| 25 | 2026-09-17T10:59:36Z | sub | `budget_declare` | `smoke-gpt-oss-20b-gb10` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=96543 · weights_mib=6561 · kv_mib=5120 · overhead_mib=16384 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 기동 창 7/14: budget_renew 0회 · 닫힘 budget_clear 2026-09-17T11:02:17Z(선언 후 2m41s) · measured_utc 2026-10-28T07:50:10Z 는 이 창 밖(다른 기동 · 측정 전) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-09.jsonl:466 |
| 26 | 2026-09-17T10:59:37Z | sub | `budget_honored` | `smoke-gpt-oss-20b-gb10` | budget_honored · floor_mib=96543 · arm_ceiling_mib=93471 · remaining_s=7199 · 창 시작 2026-09-17T10:59:37Z 후 0m00s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:289 |
| 27 | 2026-09-17T11:02:17Z | sub | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-gpt-oss-20b-gb10 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-17T10:59:36Z 후 2m41s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-09.jsonl:467 |
| 28 | 2026-09-17T11:02:18Z | sub | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-gpt-oss-20b-gb10 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-17T10:59:37Z 후 2m41s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:290 |
| 29 | 2026-09-17T11:07:14Z | sub | `budget_declare` | `smoke-gpt-oss-20b-gb10` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=96543 · weights_mib=6561 · kv_mib=5120 · overhead_mib=16384 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 기동 창 8/14: budget_renew 0회 · 닫힘 budget_clear 2026-09-17T11:10:08Z(선언 후 2m54s) · measured_utc 2026-10-28T07:50:10Z 는 이 창 밖(다른 기동 · 측정 전) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-09.jsonl:469 |
| 30 | 2026-09-17T11:07:15Z | sub | `budget_honored` | `smoke-gpt-oss-20b-gb10` | budget_honored · floor_mib=96543 · arm_ceiling_mib=93471 · remaining_s=7199 · 창 시작 2026-09-17T11:07:15Z 후 0m00s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:291 |
| 31 | 2026-09-17T11:10:08Z | sub | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-gpt-oss-20b-gb10 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-17T11:07:14Z 후 2m54s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-09.jsonl:470 |
| 32 | 2026-09-17T11:10:08Z | sub | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-gpt-oss-20b-gb10 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-17T11:07:15Z 후 2m53s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:292 |
| 33 | 2026-09-17T11:11:10Z | sub | `budget_declare` | `smoke-gpt-oss-20b-gb10` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=96543 · weights_mib=6561 · kv_mib=5120 · overhead_mib=16384 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 기동 창 9/14: budget_renew 1회 · 닫힘 budget_clear 2026-09-17T11:15:50Z(선언 후 4m40s) · measured_utc 2026-10-28T07:50:10Z 는 이 창 밖(다른 기동 · 측정 전) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-09.jsonl:472 |
| 34 | 2026-09-17T11:11:11Z | sub | `budget_honored` | `smoke-gpt-oss-20b-gb10` | budget_honored · floor_mib=96543 · arm_ceiling_mib=93471 · remaining_s=7199 · 창 시작 2026-09-17T11:11:11Z 후 0m00s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:293 |
| 35 | 2026-09-17T11:13:48Z | sub | `budget_renew` | `smoke-gpt-oss-20b-gb10` | budget_renew · floor_mib=96543 · ttl_s=7200 · remaining_before_s=7042 · 창 시작 2026-09-17T11:11:10Z 후 2m38s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-09.jsonl:473 |
| 36 | 2026-09-17T11:15:50Z | sub | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-gpt-oss-20b-gb10 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-17T11:11:10Z 후 4m40s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-09.jsonl:474 |
| 37 | 2026-09-17T11:15:50Z | sub | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-gpt-oss-20b-gb10 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-17T11:11:11Z 후 4m39s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:294 |
| 38 | 2026-09-29T15:21:55Z | sub | `budget_declare` | `smoke-gpt-oss-20b-gb10` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=96543 · weights_mib=6561 · kv_mib=5120 · overhead_mib=16384 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 기동 창 10/14: budget_renew 0회 · 닫힘 budget_clear 2026-09-29T15:30:25Z(선언 후 8m30s) · measured_utc 2026-10-28T07:50:10Z 는 이 창 밖(다른 기동 · 측정 전) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-09.jsonl:730 |
| 39 | 2026-09-29T15:21:57Z | sub | `budget_honored` | `smoke-gpt-oss-20b-gb10` | budget_honored · floor_mib=96543 · arm_ceiling_mib=93471 · remaining_s=7198 · 창 시작 2026-09-29T15:21:57Z 후 0m00s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:363 |
| 40 | 2026-09-29T15:30:25Z | sub | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-gpt-oss-20b-gb10 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-29T15:21:55Z 후 8m30s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-09.jsonl:731 |
| 41 | 2026-09-29T15:30:26Z | sub | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-gpt-oss-20b-gb10 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-29T15:21:57Z 후 8m29s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:364 |
| 42 | 2026-09-29T15:58:19Z | sub | `budget_declare` | `smoke-gpt-oss-20b-gb10` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=96543 · weights_mib=6561 · kv_mib=5120 · overhead_mib=16384 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 기동 창 11/14: budget_renew 1회 · 닫힘 budget_clear 2026-09-29T16:02:49Z(선언 후 4m30s) · measured_utc 2026-10-28T07:50:10Z 는 이 창 밖(다른 기동 · 측정 전) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-09.jsonl:733 |
| 43 | 2026-09-29T15:58:19Z | sub | `budget_honored` | `smoke-gpt-oss-20b-gb10` | budget_honored · floor_mib=96543 · arm_ceiling_mib=93471 · remaining_s=7200 · 창 시작 2026-09-29T15:58:19Z 후 0m00s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:365 |
| 44 | 2026-09-29T16:00:49Z | sub | `budget_renew` | `smoke-gpt-oss-20b-gb10` | budget_renew · floor_mib=96543 · ttl_s=7200 · remaining_before_s=7050 · 창 시작 2026-09-29T15:58:19Z 후 2m30s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-09.jsonl:734 |
| 45 | 2026-09-29T16:02:49Z | sub | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-gpt-oss-20b-gb10 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-29T15:58:19Z 후 4m30s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-09.jsonl:735 |
| 46 | 2026-09-29T16:02:51Z | sub | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-gpt-oss-20b-gb10 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-29T15:58:19Z 후 4m32s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:366 |
| 47 | 2026-09-29T18:29:06Z | sub | `budget_declare` | `smoke-gpt-oss-20b-gb10` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=96543 · weights_mib=6561 · kv_mib=5120 · overhead_mib=16384 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 기동 창 12/14: budget_renew 0회 · 닫힘 budget_clear 2026-09-29T18:29:23Z(선언 후 0m17s) · measured_utc 2026-10-28T07:50:10Z 는 이 창 밖(다른 기동 · 측정 전) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-09.jsonl:737 |
| 48 | 2026-09-29T18:29:08Z | sub | `budget_honored` | `smoke-gpt-oss-20b-gb10` | budget_honored · floor_mib=96543 · arm_ceiling_mib=93471 · remaining_s=7198 · 창 시작 2026-09-29T18:29:08Z 후 0m00s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:367 |
| 49 | 2026-09-29T18:29:23Z | sub | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-gpt-oss-20b-gb10 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-29T18:29:06Z 후 0m17s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-09.jsonl:738 |
| 50 | 2026-09-29T18:29:24Z | sub | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-gpt-oss-20b-gb10 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-29T18:29:08Z 후 0m16s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:368 |
| 51 | 2026-09-29T19:02:09Z | sub | `budget_declare` | `smoke-gpt-oss-20b-gb10` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=96543 · weights_mib=6561 · kv_mib=5120 · overhead_mib=16384 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 기동 창 13/14: budget_renew 0회 · 닫힘 budget_clear 2026-09-29T19:04:57Z(선언 후 2m48s) · measured_utc 2026-10-28T07:50:10Z 는 이 창 밖(다른 기동 · 측정 전) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-09.jsonl:740 |
| 52 | 2026-09-29T19:02:10Z | sub | `budget_honored` | `smoke-gpt-oss-20b-gb10` | budget_honored · floor_mib=96543 · arm_ceiling_mib=93471 · remaining_s=7199 · 창 시작 2026-09-29T19:02:10Z 후 0m00s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:369 |
| 53 | 2026-09-29T19:04:57Z | sub | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-gpt-oss-20b-gb10 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-29T19:02:09Z 후 2m48s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-09.jsonl:741 |
| 54 | 2026-09-29T19:04:57Z | sub | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-gpt-oss-20b-gb10 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-29T19:02:10Z 후 2m47s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:370 |
| 55 | 2026-10-28T07:31:54Z | sub | `budget_declare` | `serve-gpt-oss-20b-gb10` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=45949 · weights_mib=13123 · kv_mib=49152 · overhead_mib=16384 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 기동 창 14/14: budget_renew 1회 · 닫힘 budget_clear 2026-10-28T07:50:46Z(선언 후 18m52s) · measured_utc 2026-10-28T07:50:10Z 가 이 창 안(이 측정의 기동) | docs/logs/sub/events/2026-10.jsonl:686 |
| 56 | 2026-10-28T07:31:54Z | sub | `serve_start` | — | serve_start · model=gpt-oss-20b · tensor_parallel_size=1 · topology=single · session=serve-gpt-oss-20b-gb10-a03-20261028T073154Z | docs/logs/sub/events/2026-10.jsonl:687 |
| 57 | 2026-10-28T07:31:54Z | sub | `budget_honored` | `serve-gpt-oss-20b-gb10` | budget_honored · floor_mib=45949 · arm_ceiling_mib=42877 · remaining_s=7200 · 창 시작 2026-10-28T07:31:54Z 후 0m00s | docs/logs/sub/events/watchdog.jsonl:645 |
| 58 | 2026-10-28T07:31:55Z | sub | `budget_renew` | `serve-gpt-oss-20b-gb10` | budget_renew · floor_mib=45949 · ttl_s=7200 · remaining_before_s=7199 · 창 시작 2026-10-28T07:31:54Z 후 0m01s | docs/logs/sub/events/2026-10.jsonl:688 |
| 59 | 2026-10-28T07:32:10Z | sub | `engine_log_window_start` | — | 캡처된 엔진 로그의 첫 시각(꼬리 캡처일 수 있다 — 엔진 시작 시각이 아닐 수 있다) · 선언 2026-10-28T07:31:54Z 후 0m16s · 시계: 컨테이너 시각을 UTC 로 읽어 측정 창 [2026-10-28T07:31:54Z, 2026-10-28T07:50:46Z] 안에 드는 것을 확인 | output/single/benchlog/sweep_gpt-oss-20b-gb10_camp26102705_resume_20261028T073540Z/lite_engine_gpt-oss-20b-gb10.log:41 |
| 60 | 2026-10-28T07:33:44Z | sub | `engine_weights_loaded` | — | 가중치 적재 완료 줄 ×1(rank·단계별) · 최장 86.81s · 이 행 = 마지막 줄 | output/single/benchlog/sweep_gpt-oss-20b-gb10_camp26102705_resume_20261028T073540Z/lite_engine_gpt-oss-20b-gb10.log:74 |
| 61 | 2026-10-28T07:33:45Z | sub | `engine_model_loaded` | — | 모델 적재 완료 줄 ×1(rank 별) · 마지막 줄 13.72 GiB · 88.5s | output/single/benchlog/sweep_gpt-oss-20b-gb10_camp26102705_resume_20261028T073540Z/lite_engine_gpt-oss-20b-gb10.log:75 |
| 62 | 2026-10-28T07:34:01Z | sub | `engine_kv_cache_sized` | — | GPU KV cache 2,097,152 tokens | output/single/benchlog/sweep_gpt-oss-20b-gb10_camp26102705_resume_20261028T073540Z/lite_engine_gpt-oss-20b-gb10.log:81 |
| 63 | 2026-10-28T07:34:02Z | sub | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=75900 · rate_mib_s=24647 · max_rate_mib_s=17801 · legacy_rule_would_trip=true · 라벨 없음 → serve-gpt-oss-20b-gb10 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-28T07:31:54Z 후 2m08s) | docs/logs/sub/events/watchdog.jsonl:646 |
| 64 | 2026-10-28T07:34:11Z | sub | `engine_init_done` | — | 엔진 초기화(profile·KV 할당·warmup) 25.49s | output/single/benchlog/sweep_gpt-oss-20b-gb10_camp26102705_resume_20261028T073540Z/lite_engine_gpt-oss-20b-gb10.log:88 |
| 65 | 2026-10-28T07:34:13Z | sub | `server_starting` | — | API 서버 기동 — 이 측정 창 선언 2026-10-28T07:31:54Z 후 2m19s · 캡처 로그 첫 시각 2026-10-28T07:32:10Z 후 2m03s | output/single/benchlog/sweep_gpt-oss-20b-gb10_camp26102705_resume_20261028T073540Z/lite_engine_gpt-oss-20b-gb10.log:93 |
| 66 | 2026-10-28T07:50:46Z | sub | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → serve-gpt-oss-20b-gb10 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-28T07:31:54Z 후 18m52s) | docs/logs/sub/events/2026-10.jsonl:689 |
| 67 | 2026-10-28T07:50:46Z | sub | `serve_stop` | — | serve_stop · model=gpt-oss-20b · duration_s=1132 · session=serve-gpt-oss-20b-gb10-a03-20261028T073154Z | docs/logs/sub/events/2026-10.jsonl:690 |
| 68 | 2026-10-28T07:50:46Z | sub | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → serve-gpt-oss-20b-gb10 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-28T07:31:54Z 후 18m52s) | docs/logs/sub/events/watchdog.jsonl:647 |

**기동 시도**(예산 선언 `budget_declare` 마다 한 시도 · 같은 노드의 이어지는 행과 선언 없는 노드(엔진 로그)의 그 시각 행을 묶었다 · 사살 · 트립 · 거부는 닫힘과 따로 센다 · `measurement` 행은 측정 기록이라 시도로 세지 않는다(위 표에만) — 해석은 저작자 몫)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 1 | sub | `serve-gpt-oss-20b-gb10` | 2026-10-28T07:31:54Z | 1회 · 첫 2026-10-28T07:31:55Z | 없음 | `budget_clear` 2026-10-28T07:50:46Z | 11 |

**같은 셀 이름 · 앞선 캠페인의 기동 13회**(캠페인 경계 2026-10-26T21:46:47Z 이전 — 라벨이 같아 원장에서 함께 묶였을 뿐 이 셀의 시도로 세지 않는다 · 행 전문 = 01 §1.4)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 앞선-1 | sub | `smoke-gpt-oss-20b-gb10` | 2026-09-17T09:27:12Z | 없음 | 없음 | `budget_clear` 2026-09-17T09:29:34Z | 2 |
| 앞선-2 | sub | `smoke-gpt-oss-20b-gb10` | 2026-09-17T09:48:01Z | 없음 | 없음 | `budget_clear` 2026-09-17T09:50:27Z | 2 |
| 앞선-3 | sub | `smoke-gpt-oss-20b-gb10` | 2026-09-17T10:07:12Z | 없음 | 없음 | `budget_clear` 2026-09-17T10:09:31Z | 2 |
| 앞선-4 | sub | `smoke-gpt-oss-20b-gb10` | 2026-09-17T10:30:12Z | 없음 | 없음 | `budget_clear` 2026-09-17T10:32:33Z | 2 |
| 앞선-5 | sub | `smoke-gpt-oss-20b-gb10` | 2026-09-17T10:44:24Z | 없음 | 없음 | `budget_clear` 2026-09-17T10:46:45Z | 2 |
| 앞선-6 | sub | `smoke-gpt-oss-20b-gb10` | 2026-09-17T10:53:41Z | 없음 | 없음 | `budget_clear` 2026-09-17T10:56:00Z | 2 |
| 앞선-7 | sub | `smoke-gpt-oss-20b-gb10` | 2026-09-17T10:59:36Z | 없음 | 없음 | `budget_clear` 2026-09-17T11:02:17Z | 2 |
| 앞선-8 | sub | `smoke-gpt-oss-20b-gb10` | 2026-09-17T11:07:14Z | 없음 | 없음 | `budget_clear` 2026-09-17T11:10:08Z | 2 |
| 앞선-9 | sub | `smoke-gpt-oss-20b-gb10` | 2026-09-17T11:11:10Z | 1회 · 첫 2026-09-17T11:13:48Z | 없음 | `budget_clear` 2026-09-17T11:15:50Z | 2 |
| 앞선-10 | sub | `smoke-gpt-oss-20b-gb10` | 2026-09-29T15:21:55Z | 없음 | 없음 | `budget_clear` 2026-09-29T15:30:25Z | 2 |
| 앞선-11 | sub | `smoke-gpt-oss-20b-gb10` | 2026-09-29T15:58:19Z | 1회 · 첫 2026-09-29T16:00:49Z | 없음 | `budget_clear` 2026-09-29T16:02:49Z | 2 |
| 앞선-12 | sub | `smoke-gpt-oss-20b-gb10` | 2026-09-29T18:29:06Z | 없음 | 없음 | `budget_clear` 2026-09-29T18:29:23Z | 2 |
| 앞선-13 | sub | `smoke-gpt-oss-20b-gb10` | 2026-09-29T19:02:09Z | 없음 | 없음 | `budget_clear` 2026-09-29T19:04:57Z | 2 |
<!-- /FACT:event_timeline -->

<!-- FACT:measurement_env -->
_측정 실행의 env 관측 없음 — 측정 엔진 로그에서 env echo 줄(`<KEY> set by environment to <VALUE>` 등)을 찾지 못했다(evidence.measurement_env_observed · 부재 ≠ 미설정)._
<!-- /FACT:measurement_env -->

> 이 절이 답하는 질문: 수신자가 이 zip 만으로 이 셀을 다시 띄우려면 어떤 순서로 무엇을 하고, 각 단계의 성공을 무엇으로 판정하며, 얼마나 기다려야 하는가?

원장은 빌드 명령을 재구성했으며 이번에 빌드하지 않았다. 현재 이미지와 로컬 모델을 확인하고 예산을 선언해 공식 single serve를 기동했다. health 및 기능 확인 후 lite와 Full을 실행하고 같은 session을 down했다. 정확한 실행 argv와 종료 근거는 testlog_26102816_GPT_OSS_20B_신규_Full_판정 §시각·정리·근거에 있다.

## 1.5 서브 레시피 해설
<!-- FACT:sub_recipe -->
해당 없음 — single 토폴로지(서브 노드 · Ray worker 없음).
<!-- /FACT:sub_recipe -->

<!-- FACT:measurement_env_nodes -->
해당 없음 — single 토폴로지의 단일 측정 노드에는 노드 간 비교 대상이 없다.
<!-- /FACT:measurement_env_nodes -->

> 해당 없음 — 이 절의 조건 `topology=multi` 이 이 셀에서 성립하지 않는다(`topology=single`).

## 1.6 재현 절차 — 기동 전 준비(사람이 할 일)
> 1.4 의 명령을 치기 **전에** 사람이 손으로 해야 하는 것이다 — 가중치 획득 · 스테이징 생성 · 네트워크가 필요한 단계. 에이전트가 대신 할 수 없는 자리(다운로드 승인 · 물리 경로 · 외부 계정)가 여기 모인다.

> 이 절이 답하는 질문: 이 셀을 기동하기 전에 사람이 준비해야 하는 것은 무엇이며, 각각을 어떤 명령으로 만들고, 그 절차의 출처는 어디인가?

### 모델과 이미지
수신자는 자기 manifest의 모델 경로와 기존 이미지를 확인한다. 모델 다운로드나 빌드가 필요하면 해당 owner 경로를 따른다.

### 자원과 서비스
자기 노드 메모리 및 가드를 확인하고 보수 예약을 선언한다. plan_26102816_GPT_OSS_20B_재개_Full_hint §Trial 이후 확정에 trial 관측과 선언의 차이가 있다.

### 측정
같은 endpoint와 부하를 사용하는 공식 도구에서 새 경로를 검사하고 요청·완료·서버 계수를 따로 기록한다. testlog_26102816_GPT_OSS_20B_신규_Full_판정 §이번 실행.
