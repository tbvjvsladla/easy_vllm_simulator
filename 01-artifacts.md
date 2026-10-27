# 01 · 산출물 — 무엇이 실제로 쓰였고, 어떻게 다시 띄우는가

## 1.1 적용 판정 표
> 기계가 적용 증거(빌드 원장 · 원장이 없으면 라벨된 재구성) × 파일 × 선언을 대사한 결과다. `artifacts/` 에는 **적용된 것만** 실렸다 — 빌드 때 스스로 skip 된 패치와 쓰이지 않은 평면의 파일은 표에만 남고 싣지 않는다.

<!-- FACT:slots -->
| 슬롯 | 실림 | 파일 | 신호 | 적용 증거 | 사유 |
|---|---|---|---|---|---|
| `build_patch_post` | 안 실림 | — | 1-signal | build-ledger:docker-image(main-local):sha256:123c331f7a5266b069fcb8b9b28009e862d5662f50dcd12e9321fa08937c1c16:/opt/easy-vllm/build_ledger.json | 적용 판정 없음 — 원장 관측(기계 파생) |
| `build_patch_pre` | 안 실림 | — | 1-signal | build-ledger:docker-image(main-local):sha256:123c331f7a5266b069fcb8b9b28009e862d5662f50dcd12e9321fa08937c1c16:/opt/easy-vllm/build_ledger.json | 적용 판정 없음 — 원장 관측(기계 파생) |
| `build_recipe` | 실림 | `artifacts/build_recipe/Dockerfile.source-build` · `artifacts/build_recipe/requirements.txt` | 3-signal | build-ledger:build-ledger.dockerfile | 실린 Dockerfile·requirements의 리비전과 검증 지위는 위 FACT 표를 따른다. source 트랙은 NGC torch ABI에 맞춰 컴파일하는 공유 이미지 경로이며, 원장/이미지history와 맞는 바이트가 쓰인 것인지 별도로 확인한다. generated-unverified 표지가 있으면 참고 형상임을 그대로 보존한다. 이번에 이미지를 새로 빌드하지 않았다. |
| `compose` | 실림 | `artifacts/compose/.env.template` · `artifacts/compose/docker-compose.yaml` | 2-signal | file:output/single/docker-compose.yaml | compose·env형상은0.30.0이미지, NAS마운트, 컨테이너내8000포트, 자원 배선을 연결한다. 측정 당시 env관측과 선택 리비전이 정본이며, 다른 호스트의 실제 경로는 다시 맞춘다. |
| `fork_pin` | 안 실림 | — | 1-signal | none:.claude/policies/arch_variant_ledger.json | 셀 env 에 VARIANT 줄 없음 = stock(workflow.md §변종 좌표의 거처)(기계 파생) |
| `runtime_patch` | 안 실림 | — | 1-signal | none:output/single/configs/granite42-30b-v030_patch.py | output/single/configs/granite42-30b-v030_patch.py 없음 — arm_patch.sh 는 no-op(런타임 패치 불해당)(기계 파생) |
| `triplet` | 실림 | `artifacts/triplet/.env.granite42-30b-v030` · `artifacts/triplet/granite42-30b-v030.sh` · `artifacts/triplet/granite42-30b-v030.yaml` | 3-signal | file:output/single/configs/granite42-30b-v030.yaml | yaml·runner·셀env는 이 셀의 실제입력이다. granite_thinking_parser 플러그인은 모델 폴더 동봉 파일을 runner가 읽는 경로이며 +1 arm_patch 모델런타임패치와 다르다. tool parser qwen3_coder와 함께 실제 모델/버전에서 다시 확인해야 한다. 이번 새 스모크는 completion만 확인했으므로 parser flags만으로 전용 tool/reasoning 시험PASS를 주장하지 않는다. |

**레시피 경고 · 실린 리비전**(기계 관측 — 수신자 빌드가 여기서 갈라질 수 있다)

- `build_recipe` `output/single/Dockerfile.source-build`(이 체크아웃 통로 `output/single/Dockerfile.source-build` 대신) 실린 리비전: 커밋 `1575bf4f8d05` · 선택 방법 `git log --all -- output/*/Dockerfile.source-build · 1575bf4f8d05:output/single/Dockerfile.source-build 가 docker history RUN·COPY 와 전수 일치(17/17 · 작업트리 15/18) · 커밋 2026-10-01T12:52:40Z > 이미지 Created 2026-10-01T08:14:34Z(빌드 뒤 커밋된 같은 단계) — 이 개정의 바이트를 싣는다(git cat-file)` · docker history 대조 17/17 · 실린 바이트 `git:1575bf4f8d05587d186bbc5932b4f870d01157f5` · 쓰인 바이트임을 관측으로 확인 예
- `build_recipe` `output/single/requirements.txt`(이 체크아웃 통로 `output/single/requirements.txt` 대신) 실린 리비전: 커밋 `1575bf4f8d05` · 선택 방법 `git log --all -- output/*/requirements.txt · 1575bf4f8d05:output/single/requirements.txt · 빌드 원장 sha256 일치(작업트리·이미지 Created 이전 개정은 다르다) · 커밋 2026-10-01T12:52:40Z > 이미지 Created 2026-10-01T08:14:34Z(빌드 뒤 커밋된 같은 바이트)` · docker history 대조 없음 · 실린 바이트 `git:1575bf4f8d05587d186bbc5932b4f870d01157f5` · 쓰인 바이트임을 관측으로 확인 예
- `compose` `output/single/docker-compose.yaml` 실린 리비전: 커밋 `2199257f7f71` · 선택 방법 `worktree(mtime 2026-10-26T21:40:37Z ≤ 측정 2026-10-27T01:17:15Z(sweep_index.generated_utc) · 바이트 = 측정 전 마지막 커밋 2199257f7f71)` · docker history 대조 없음 · 실린 바이트 `worktree` · 근거 `mtime≤measured` · 쓰인 바이트임을 관측으로 확인 예

**싣지 않은 파일**(슬롯별 사유 — 빌드 때 skip 된 패치는 아래 적용 집합 표에 있다)

- `build_patch_post` known-non-patch 1건: `.gitkeep`
- `build_patch_pre` known-non-patch 1건: `.gitkeep`
- `compose` `arm_patch.sh` — `not-armed(output/single/configs/granite42-30b-v030_patch.py 없음(CONFIG_FILE) — arm_patch.sh 는 no-op · arm_patch.sh 측정 당시 개정(c8bb20e77e52) `_cfg=` 키 체인 RUNTIME_PATCH_STEM > CONFIG_FILE > 'default')`

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

### build_recipe
실린 Dockerfile·requirements의 리비전과 검증 지위는 위 FACT 표를 따른다. source 트랙은 NGC torch ABI에 맞춰 컴파일하는 공유 이미지 경로이며, 원장/이미지history와 맞는 바이트가 쓰인 것인지 별도로 확인한다. generated-unverified 표지가 있으면 참고 형상임을 그대로 보존한다. 이번에 이미지를 새로 빌드하지 않았다.

### compose
compose·env형상은0.30.0이미지, NAS마운트, 컨테이너내8000포트, 자원 배선을 연결한다. 측정 당시 env관측과 선택 리비전이 정본이며, 다른 호스트의 실제 경로는 다시 맞춘다.

### triplet
yaml·runner·셀env는 이 셀의 실제입력이다. granite_thinking_parser 플러그인은 모델 폴더 동봉 파일을 runner가 읽는 경로이며 +1 arm_patch 모델런타임패치와 다르다. tool parser qwen3_coder와 함께 실제 모델/버전에서 다시 확인해야 한다. 이번 새 스모크는 completion만 확인했으므로 parser flags만으로 전용 tool/reasoning 시험PASS를 주장하지 않는다.

### 불해당 슬롯
빌드 패치·포크·셀 runtime_patch의 적용/skip 여부는 위 표와 원장을 따른다. 실린 패치가 없고 inline strip-hoist가skip이면 활성 패치라 부르지 않는다. 과거 멀티이미지를 재사용한다는 계획은 실제 single 재빌드 이미지의 관측과 구분한다(02 §2.1). 새 버전에서 자체 패치가 필요하다는 일반화도 하지 않는다.
이 이미지 원장은 strip-hoist를 skipped로 기록한다. 공유 Dockerfile은 hoist 인자를 받거나 opaque_object가 없으면 아래처럼 skip 상태를 남긴다. 미수용 경로에서 대상이 없어도 WARN+skipped로 기록하며 이것만으로 빌드를 중단하지 않는다. 다음 버전에서는 이 조건과 실제 원장을 다시 확인한다.

> [원문] Dockerfile.source-build §L112-114
> else:
>     print("[patch] strip-hoist skipped (register_opaque_type accepts hoist / opaque_object 부재)")
>     status = "skipped\tregister_opaque_type accepts hoist / opaque_object 부재"

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
근거: 체크포인트 dtype을 승계했고 이번에 다른 dtype대조 없음
출처: [granite42-30b-v030.yaml, testlog_26102710_granite42_30b_v030_코덱스_메인_Full_판정 §1]
```

```hint-event
id: V2
kind: value-status
노브: gpu-memory-utilization
값: "0.80"
지위: inherited
근거: 과거 셀의 손 선언 천장 승계; 이번 조정 없고 절대KV클램프가 KV산정에 우선함
출처: [granite42-30b-v030.yaml, testlog_26102710_granite42_30b_v030_코덱스_메인_Full_판정 §1]
```

```hint-event
id: V3
kind: value-status
노브: max-model-len
값: 131072
지위: inherited
근거: 네이티브 길이를 그대로 승계; 이번 길이 레버 대조 없음
출처: [granite42-30b-v030.yaml, plan_26100107_vllm030_bump_서브지침배달_멀티싱글_E2E캠페인 §3]
```

```hint-event
id: V4
kind: value-status
노브: kv-cache-memory-bytes
값: 35433480192
지위: tuned
근거: 과거16GiB거부(W1)와32GiB16토큰미달거부(W2) 뒤33GiB에서 기동; 이번은 같은값의 새 Full검증이며 새조정 아님
출처: [granite42-30b-v030.yaml, testlog_26101017_08_29_knob프리셋_E2E_셀A_granite42_30b_v030_판정 §1]
```

```hint-event
id: V5
kind: value-status
노브: kv-cache-dtype
값: auto
지위: engine-default
근거: auto선택을 명시했고 현재 엔진 관측은 testlog에 따로 기록; 다른dtype대조 없음
출처: [granite42-30b-v030.yaml, testlog_26102710_granite42_30b_v030_코덱스_메인_Full_판정 §4]
```

```hint-event
id: V6
kind: value-status
노브: max-num-seqs
값: 8
지위: inherited
근거: 값의 최초 산정 출처는 계보에 미기록이며 기존값 승계; 이번에 레버조정 없고 레벨16필터 상한으로 작용
출처: [granite42-30b-v030.yaml, testlog_26102710_granite42_30b_v030_코덱스_메인_Full_판정 §2]
```

V4의 tuned는 과거 계보의 기동 조정 이력이다. 이번 캠페인에서는 여섯 노브를 바꾸지 않았다. gmu와 절대KV를 같은역할로 복사하지 않고 목표길이·수용토큰·호스트 예산을 함께 다시 도출한다.

## 1.4 재현 절차
> 셀 실행 기록에서 기계가 파생한 명령 순서와 단계별 소요다. 소요가 `미관측` 인 단계는 시각 기록이 없다는 뜻이다(0 이 아니다). 그 아래는 블랙박스 이벤트 원장에서 이 셀의 행을 기계가 뽑아 기동 시도로 묶은 것이고, 마지막은 측정한 실행의 엔진 로그가 echo 한 env 다(측정 당시 값의 정본).

<!-- FACT:reproduce -->
| 순서 | 단계 | 성공 판정 | 소요 | 관측 구간(UTC) | 소요 출처 | 명령 출처 |
|---|---|---|---|---|---|---|
| 1 | build | 빌드 OK · 이미지 실재 | 층 창 36분 20초(docker history 첫 층 → 끝 층 CreatedAt · 캐시 재사용 층 포함 — 빌드 소요 아님) | 2026-10-01T07:38:14Z → 2026-10-01T08:14:34Z (layer-window) | docker history --no-trunc --format '{{json .}}' sha256:123c331f7a5266b069fcb8b9b28009e862d5662f50dcd12e9321fa08937c1c16 · 우리 Dockerfile 층 24개의 CreatedAt 첫 층 → 끝 층(캐시 재사용 층은 이전 빌드 시각 — 빌드 소요 아님) | reconstructed(build-ledger build_args + compose build) |
| 2 | serve | health 200 + 추론 1회(스모크 판정) | ≤ 10분 20초(상한) | 2026-10-26T23:07:07Z → 2026-10-26T23:17:27Z (upper) | docs/logs/main/events/2026-10.jsonl budget_declare(label serve-granite42-30b-v030) → 첫 측정 시각(output/single/benchlog/sweep_granite42-30b-v030/lite_raw_granite42-30b-v030.json measured_utc(호스트 date -u · lite_bench.sh 가 cold 레그 직전에 찍는 측정 시작)) | derived(스모크 사용법) |
| 3 | bench | 리포트 발행(+PASS 면 인증서) | 1시간 59분 48초 | 2026-10-26T23:17:27Z → 2026-10-27T01:17:15Z (exact) | output/single/benchlog/sweep_granite42-30b-v030/lite_raw_granite42-30b-v030.json measured_utc(호스트 date -u · lite_bench.sh 가 cold 레그 직전에 찍는 측정 시작) → sweep_index.generated_utc(호스트 UTC · 두 끝 같은 시계) | reconstructed(bench json + 측정 도구 원문 lite 인자) |

**빌드 층 시각**(관측 · 소요가 아니라 층 창 — 캐시 재사용 층은 이전 빌드의 시각을 가진다)

- `build` 이미지 층 CreatedAt(docker history): 2026-10-01T07:38:14Z → 2026-10-01T08:14:34Z · 층 24개 · 출처 `docker history --no-trunc --format '{{json .}}' sha256:123c331f7a5266b069fcb8b9b28009e862d5662f50dcd12e9321fa08937c1c16` — 우리 Dockerfile 층의 CreatedAt 범위 — 캐시 재사용 층은 이전 빌드 시각을 가진다(소요 아님)

**단계별 명령**(기계 파생 — `명령 출처` 가 `derived(…)` 면 사용법에서 파생 · `reconstructed(…)` 면 괄호 안 관측(docker history build-arg · compose build · bench JSON 필드)에서 재구성한 명령이다. 어느 쪽도 실행 원문(로그에 남은 줄)은 아니다)

**1. build**

```sh
# 실린 artifacts/build_recipe/Dockerfile.source-build(이미지를 지은 개정: git:1575bf4f8d05587d186bbc5932b4f870d01157f5(output/single/Dockerfile.source-build))를 output/single/Dockerfile.source-build 자리에 두고 컨텍스트에서 짓는다.
mkdir -p output/single/build_patches_src output/single/build_patches   # 빌드 원장 context_inventory.build_patch_pre = ['.gitkeep'] · 빌드 원장 context_inventory.build_patch_post = ['.gitkeep']
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
# lite_cold/lite_warm 의 데이터셋 · 부하 인자 = 측정 시점 도구 원문 lite_bench.sh@90a3cdd56a94 L177-L183(리터럴 인자만 · `$변수` 인자는 도구가 값을 정하지 않아 JSON 필드로 채운다) — `total_*_tokens ÷ completed` 는 실측 토큰(chat 템플릿 포함)이지 인자가 아니다.
# output/single/benchlog/sweep_granite42-30b-v030/lite_cold_granite42-30b-v030.json (date 20261026-231814) · 실측 토큰(chat 템플릿 포함 · total ÷ completed · 인자 아님): 입력 527/1 = 527 · 출력 128/1 = 128
vllm bench serve --backend openai-chat --model granite-4.2-30b --tokenizer /app/models/IBM/Granite-4.2/granite-4.2-30b --trust-remote-code --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 --ignore-eos --num-prompts 1 --max-concurrency 1 --request-rate inf
# output/single/benchlog/sweep_granite42-30b-v030/lite_warm_granite42-30b-v030.json (date 20261026-232034) · 실측 토큰(chat 템플릿 포함 · total ÷ completed · 인자 아님): 입력 1581/3 = 527 · 출력 384/3 = 128
vllm bench serve --backend openai-chat --model granite-4.2-30b --tokenizer /app/models/IBM/Granite-4.2/granite-4.2-30b --trust-remote-code --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 --ignore-eos --num-prompts 3 --max-concurrency 1 --request-rate inf
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
| (실린 파일 없음 — 빈 디렉터리 생성) | `build_patches_src/` | Dockerfile.source-build COPY build_patches_src/ · 실린 패치 없음 — 빈 디렉터리를 만든다(없으면 COPY 단계에서 빌드가 멈춘다) · 빌드 원장 context_inventory.build_patch_pre = ['.gitkeep'] |
| (실린 파일 없음 — 빈 디렉터리 생성) | `build_patches/` | Dockerfile.source-build COPY build_patches/ · 실린 패치 없음 — 빈 디렉터리를 만든다(없으면 COPY 단계에서 빌드가 멈춘다) · 빌드 원장 context_inventory.build_patch_post = ['.gitkeep'] |

**기동 성공 판정(관측 · 발행 자격)**: health 200 = true · 추론 1회 = true · 근거 `output/single/benchlog/sweep_granite42-30b-v030/level_01/post_health_granite42-30b-v030.json` · `output/single/benchlog/sweep_granite42-30b-v030/level_01/guidellm_granite42-30b-v030.json`
<!-- /FACT:reproduce -->

<!-- FACT:event_timeline -->
**블랙박스 이벤트 107행**(원장에서 이 셀 label·config 에 맞는 행과 그와 시간이 겹치는 같은 노드의 다른 라벨 창 · 시각순 · 기계 발췌 · 이번 캠페인 31행 · 같은 셀 이름 · 앞선 캠페인 76행(내용 칸에 표기) · 라벨 다름(같은 노드 · 창 밖 이 셀 행과 시간이 겹치는 다른 라벨 선언 창) 25행 — 이 셀의 선언으로 세지 않는다)

> **캠페인 경계** 2026-10-26T21:46:47Z(`camp-26102705-codex-single8` · evidence.event_campaign_boundary): campaigns/camp-26102705-codex-single8/campaign.yaml declared_utc 2026-10-26T21:46:47Z · 이 발행이 묶인 측정을 서빙한 budget_declare 창 시작 2026-10-26T23:07:07Z → 경계 = 앞선 값 2026-10-26T21:46:47Z(이전 행 = 같은 셀 이름 · 앞선 캠페인)

> **원장 관측 범위**(노드별 첫 행 → 마지막 행 · evidence.event_ledger_spans): main = 2026-07-31T03:27:43Z → 2026-10-27T01:59:43Z(`docs/logs/main/events/2026-07.jsonl` · `docs/logs/main/events/2026-08.jsonl` · `docs/logs/main/events/2026-09.jsonl` · `docs/logs/main/events/2026-10.jsonl` · `docs/logs/main/events/thermal.jsonl` · `docs/logs/main/events/watchdog.jsonl`)

| # | UTC | 노드 | kind | label | 내용 | 출처 |
|---|---|---|---|---|---|---|
| 1 | 2026-10-01T01:42:26Z | main | `budget_declare` | `camp-26100110-granite` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=35999 · weights_mib=55841 · kv_mib=16384 · overhead_mib=16384 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 라벨 다름: `camp-26100110-granite` — 같은 노드 · 이 셀 라벨(`granite42-30b-v030`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · budget_renew 1회 · 닫힘 budget_clear 2026-10-01T01:49:52Z(선언 후 7m26s) · measured_utc 2026-10-27T01:17:15Z 는 이 창 밖(측정 전) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:18 |
| 2 | 2026-10-01T01:42:26Z | main | `serve_start` | — | serve_start · model=granite-4.2-30b · tensor_parallel_size=1 · topology=single · session=serve-granite42-30b-v030-20261001T014226Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:19 |
| 3 | 2026-10-01T01:42:27Z | main | `budget_renew` | `camp-26100110-granite` | budget_renew · floor_mib=35999 · ttl_s=7200 · remaining_before_s=7199 · 창 시작 2026-10-01T01:42:26Z 후 0m01s · 라벨 다름: `camp-26100110-granite` — 같은 노드 · 이 셀 라벨(`granite42-30b-v030`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:20 |
| 4 | 2026-10-01T01:42:27Z | main | `budget_honored` | `camp-26100110-granite` | budget_honored · floor_mib=35999 · arm_ceiling_mib=32927 · remaining_s=7199 · 창 시작 2026-10-01T01:42:27Z 후 0m00s · 라벨 다름: `camp-26100110-granite` — 같은 노드 · 이 셀 라벨(`granite42-30b-v030`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:556 |
| 5 | 2026-10-01T01:42:59Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=78076 · rate_mib_s=29191 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → camp-26100110-granite 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-01T01:42:27Z 후 0m32s) · 라벨 다름: `camp-26100110-granite` — 같은 노드 · 이 셀 라벨(`granite42-30b-v030`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:557 |
| 6 | 2026-10-01T01:49:52Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → camp-26100110-granite 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-01T01:42:26Z 후 7m26s) · 라벨 다름: `camp-26100110-granite` — 같은 노드 · 이 셀 라벨(`granite42-30b-v030`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:21 |
| 7 | 2026-10-01T01:49:52Z | main | `serve_stop` | — | serve_stop · model=granite-4.2-30b · duration_s=446 · session=serve-granite42-30b-v030-20261001T014226Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:22 |
| 8 | 2026-10-01T01:49:53Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → camp-26100110-granite 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-01T01:42:27Z 후 7m26s) · 라벨 다름: `camp-26100110-granite` — 같은 노드 · 이 셀 라벨(`granite42-30b-v030`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:558 |
| 9 | 2026-10-01T01:50:26Z | main | `budget_declare` | `camp-26100110-granite-try2` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=35999 · weights_mib=55841 · kv_mib=16384 · overhead_mib=16384 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 라벨 다름: `camp-26100110-granite-try2` — 같은 노드 · 이 셀 라벨(`granite42-30b-v030`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · budget_renew 1회 · 닫힘 budget_clear 2026-10-01T01:57:22Z(선언 후 6m56s) · measured_utc 2026-10-27T01:17:15Z 는 이 창 밖(측정 전) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:23 |
| 10 | 2026-10-01T01:50:26Z | main | `serve_start` | — | serve_start · model=granite-4.2-30b · tensor_parallel_size=1 · topology=single · session=serve-granite42-30b-v030-20261001T015026Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:24 |
| 11 | 2026-10-01T01:50:27Z | main | `budget_honored` | `camp-26100110-granite-try2` | budget_honored · floor_mib=35999 · arm_ceiling_mib=32927 · remaining_s=7199 · 창 시작 2026-10-01T01:50:27Z 후 0m00s · 라벨 다름: `camp-26100110-granite-try2` — 같은 노드 · 이 셀 라벨(`granite42-30b-v030`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:559 |
| 12 | 2026-10-01T01:50:28Z | main | `budget_renew` | `camp-26100110-granite-try2` | budget_renew · floor_mib=35999 · ttl_s=7200 · remaining_before_s=7198 · 창 시작 2026-10-01T01:50:26Z 후 0m02s · 라벨 다름: `camp-26100110-granite-try2` — 같은 노드 · 이 셀 라벨(`granite42-30b-v030`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:25 |
| 13 | 2026-10-01T01:50:53Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=92247 · rate_mib_s=22841 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → camp-26100110-granite-try2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-01T01:50:27Z 후 0m26s) · 라벨 다름: `camp-26100110-granite-try2` — 같은 노드 · 이 셀 라벨(`granite42-30b-v030`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:560 |
| 14 | 2026-10-01T01:57:22Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → camp-26100110-granite-try2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-01T01:50:26Z 후 6m56s) · 라벨 다름: `camp-26100110-granite-try2` — 같은 노드 · 이 셀 라벨(`granite42-30b-v030`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:26 |
| 15 | 2026-10-01T01:57:22Z | main | `serve_stop` | — | serve_stop · model=granite-4.2-30b · duration_s=416 · session=serve-granite42-30b-v030-20261001T015026Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:27 |
| 16 | 2026-10-01T01:57:22Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → camp-26100110-granite-try2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-01T01:50:27Z 후 6m55s) · 라벨 다름: `camp-26100110-granite-try2` — 같은 노드 · 이 셀 라벨(`granite42-30b-v030`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:561 |
| 17 | 2026-10-01T01:57:43Z | main | `budget_declare` | `camp-26100110-granite-try3` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=19615 · weights_mib=55841 · kv_mib=32768 · overhead_mib=16384 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 라벨 다름: `camp-26100110-granite-try3` — 같은 노드 · 이 셀 라벨(`granite42-30b-v030`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · budget_renew 1회 · 닫힘 budget_clear 2026-10-01T02:04:49Z(선언 후 7m06s) · measured_utc 2026-10-27T01:17:15Z 는 이 창 밖(측정 전) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:28 |
| 18 | 2026-10-01T01:57:43Z | main | `serve_start` | — | serve_start · model=granite-4.2-30b · tensor_parallel_size=1 · topology=single · session=serve-granite42-30b-v030-20261001T015743Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:29 |
| 19 | 2026-10-01T01:57:44Z | main | `budget_renew` | `camp-26100110-granite-try3` | budget_renew · floor_mib=19615 · ttl_s=7200 · remaining_before_s=7199 · 창 시작 2026-10-01T01:57:43Z 후 0m01s · 라벨 다름: `camp-26100110-granite-try3` — 같은 노드 · 이 셀 라벨(`granite42-30b-v030`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:30 |
| 20 | 2026-10-01T01:57:44Z | main | `budget_honored` | `camp-26100110-granite-try3` | budget_honored · floor_mib=19615 · arm_ceiling_mib=16543 · remaining_s=7199 · 창 시작 2026-10-01T01:57:44Z 후 0m00s · 라벨 다름: `camp-26100110-granite-try3` — 같은 노드 · 이 셀 라벨(`granite42-30b-v030`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:562 |
| 21 | 2026-10-01T01:58:10Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=83589 · rate_mib_s=29317 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → camp-26100110-granite-try3 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-01T01:57:44Z 후 0m26s) · 라벨 다름: `camp-26100110-granite-try3` — 같은 노드 · 이 셀 라벨(`granite42-30b-v030`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:563 |
| 22 | 2026-10-01T02:04:49Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → camp-26100110-granite-try3 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-01T01:57:43Z 후 7m06s) · 라벨 다름: `camp-26100110-granite-try3` — 같은 노드 · 이 셀 라벨(`granite42-30b-v030`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:31 |
| 23 | 2026-10-01T02:04:49Z | main | `serve_stop` | — | serve_stop · model=granite-4.2-30b · duration_s=426 · session=serve-granite42-30b-v030-20261001T015743Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:32 |
| 24 | 2026-10-01T02:04:49Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → camp-26100110-granite-try3 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-01T01:57:44Z 후 7m05s) · 라벨 다름: `camp-26100110-granite-try3` — 같은 노드 · 이 셀 라벨(`granite42-30b-v030`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:564 |
| 25 | 2026-10-01T02:05:04Z | main | `budget_declare` | `camp-26100110-granite-try4` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=18591 · weights_mib=55841 · kv_mib=33792 · overhead_mib=16384 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 라벨 다름: `camp-26100110-granite-try4` — 같은 노드 · 이 셀 라벨(`granite42-30b-v030`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · budget_renew 1회 · 닫힘 budget_clear 2026-10-01T02:23:54Z(선언 후 18m50s) · measured_utc 2026-10-27T01:17:15Z 는 이 창 밖(측정 전) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:33 |
| 26 | 2026-10-01T02:05:04Z | main | `serve_start` | — | serve_start · model=granite-4.2-30b · tensor_parallel_size=1 · topology=single · session=serve-granite42-30b-v030-20261001T020504Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:34 |
| 27 | 2026-10-01T02:05:04Z | main | `budget_honored` | `camp-26100110-granite-try4` | budget_honored · floor_mib=18591 · arm_ceiling_mib=15519 · remaining_s=7200 · 창 시작 2026-10-01T02:05:04Z 후 0m00s · 라벨 다름: `camp-26100110-granite-try4` — 같은 노드 · 이 셀 라벨(`granite42-30b-v030`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:565 |
| 28 | 2026-10-01T02:05:05Z | main | `budget_renew` | `camp-26100110-granite-try4` | budget_renew · floor_mib=18591 · ttl_s=7200 · remaining_before_s=7199 · 창 시작 2026-10-01T02:05:04Z 후 0m01s · 라벨 다름: `camp-26100110-granite-try4` — 같은 노드 · 이 셀 라벨(`granite42-30b-v030`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:35 |
| 29 | 2026-10-01T02:05:31Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=90052 · rate_mib_s=25352 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → camp-26100110-granite-try4 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-01T02:05:04Z 후 0m27s) · 라벨 다름: `camp-26100110-granite-try4` — 같은 노드 · 이 셀 라벨(`granite42-30b-v030`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:566 |
| 30 | 2026-10-01T02:11:57Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=37543 · rate_mib_s=21371 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → camp-26100110-granite-try4 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-01T02:05:04Z 후 6m53s) · 라벨 다름: `camp-26100110-granite-try4` — 같은 노드 · 이 셀 라벨(`granite42-30b-v030`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:567 |
| 31 | 2026-10-01T02:23:54Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → camp-26100110-granite-try4 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-01T02:05:04Z 후 18m50s) · 라벨 다름: `camp-26100110-granite-try4` — 같은 노드 · 이 셀 라벨(`granite42-30b-v030`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:36 |
| 32 | 2026-10-01T02:23:54Z | main | `serve_stop` | — | serve_stop · model=granite-4.2-30b · duration_s=1130 · session=serve-granite42-30b-v030-20261001T020504Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:37 |
| 33 | 2026-10-01T02:23:55Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → camp-26100110-granite-try4 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-01T02:05:04Z 후 18m51s) · 라벨 다름: `camp-26100110-granite-try4` — 같은 노드 · 이 셀 라벨(`granite42-30b-v030`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:568 |
| 34 | 2026-10-01T08:26:06Z | main | `budget_declare` | `serve-granite42-30b-v030` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=18591 · weights_mib=55841 · kv_mib=33792 · overhead_mib=16384 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 기동 창 1/4: budget_renew 1회 · 닫힘 budget_clear 2026-10-01T08:41:10Z(선언 후 15m04s) · measured_utc 2026-10-27T01:17:15Z 는 이 창 밖(다른 기동 · 측정 전) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:43 |
| 35 | 2026-10-01T08:26:06Z | main | `serve_start` | — | serve_start · model=granite-4.2-30b · tensor_parallel_size=1 · topology=single · session=serve-granite42-30b-v030-20261001T082606Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:44 |
| 36 | 2026-10-01T08:26:07Z | main | `budget_honored` | `serve-granite42-30b-v030` | budget_honored · floor_mib=18591 · arm_ceiling_mib=15519 · remaining_s=7199 · 창 시작 2026-10-01T08:26:07Z 후 0m00s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:572 |
| 37 | 2026-10-01T08:26:08Z | main | `budget_renew` | `serve-granite42-30b-v030` | budget_renew · floor_mib=18591 · ttl_s=7200 · remaining_before_s=7198 · 창 시작 2026-10-01T08:26:06Z 후 0m02s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:45 |
| 38 | 2026-10-01T08:26:34Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=78533 · rate_mib_s=28994 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → serve-granite42-30b-v030 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-01T08:26:07Z 후 0m27s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:573 |
| 39 | 2026-10-01T08:32:28Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=24714 · rate_mib_s=33929 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → serve-granite42-30b-v030 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-01T08:26:07Z 후 6m21s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:574 |
| 40 | 2026-10-01T08:41:10Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → serve-granite42-30b-v030 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-01T08:26:06Z 후 15m04s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:46 |
| 41 | 2026-10-01T08:41:10Z | main | `serve_stop` | — | serve_stop · model=granite-4.2-30b · duration_s=904 · session=serve-granite42-30b-v030-20261001T082606Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:47 |
| 42 | 2026-10-01T08:41:10Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → serve-granite42-30b-v030 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-01T08:26:07Z 후 15m03s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:575 |
| 43 | 2026-10-10T04:55:14Z | main | `budget_declare` | `serve-granite42-30b-v030` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=16543 · weights_mib=55841 · kv_mib=33792 · overhead_mib=18432 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 기동 창 2/4: budget_renew 1회 · 닫힘 budget_clear 2026-10-10T05:07:27Z(선언 후 12m13s) · measured_utc 2026-10-27T01:17:15Z 는 이 창 밖(다른 기동 · 측정 전) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:310 |
| 44 | 2026-10-10T04:55:14Z | main | `serve_start` | — | serve_start · model=granite-4.2-30b · tensor_parallel_size=1 · topology=single · session=serve-granite42-30b-v030-20261010T045514Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:311 |
| 45 | 2026-10-10T04:55:15Z | main | `budget_renew` | `serve-granite42-30b-v030` | budget_renew · floor_mib=16543 · ttl_s=7200 · remaining_before_s=7199 · 창 시작 2026-10-10T04:55:14Z 후 0m01s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:312 |
| 46 | 2026-10-10T04:55:15Z | main | `budget_honored` | `serve-granite42-30b-v030` | budget_honored · floor_mib=16543 · arm_ceiling_mib=13471 · remaining_s=7199 · 창 시작 2026-10-10T04:55:15Z 후 0m00s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:712 |
| 47 | 2026-10-10T04:55:42Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=72261 · rate_mib_s=27103 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → serve-granite42-30b-v030 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-10T04:55:15Z 후 0m27s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:713 |
| 48 | 2026-10-10T05:07:27Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → serve-granite42-30b-v030 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-10T04:55:14Z 후 12m13s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:313 |
| 49 | 2026-10-10T05:07:27Z | main | `serve_stop` | — | serve_stop · model=granite-4.2-30b · duration_s=733 · session=serve-granite42-30b-v030-20261010T045514Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:314 |
| 50 | 2026-10-10T05:07:28Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → serve-granite42-30b-v030 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-10T04:55:15Z 후 12m13s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:714 |
| 51 | 2026-10-10T05:07:56Z | main | `budget_declare` | `serve-granite42-30b-v030` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=16543 · weights_mib=55841 · kv_mib=33792 · overhead_mib=18432 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 기동 창 3/4: budget_renew 17회 · 닫힘 budget_clear 2026-10-10T07:15:02Z(선언 후 2h07m06s) · measured_utc 2026-10-27T01:17:15Z 는 이 창 밖(다른 기동 · 측정 전) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:315 |
| 52 | 2026-10-10T05:07:56Z | main | `serve_start` | — | serve_start · model=granite-4.2-30b · tensor_parallel_size=1 · topology=single · session=serve-granite42-30b-v030-20261010T050756Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:316 |
| 53 | 2026-10-10T05:07:56Z | main | `budget_honored` | `serve-granite42-30b-v030` | budget_honored · floor_mib=16543 · arm_ceiling_mib=13471 · remaining_s=7200 · 창 시작 2026-10-10T05:07:56Z 후 0m00s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:715 |
| 54 | 2026-10-10T05:07:57Z | main | `budget_renew` | `serve-granite42-30b-v030` | budget_renew · floor_mib=16543 · ttl_s=7200 · remaining_before_s=7199 · 창 시작 2026-10-10T05:07:56Z 후 0m01s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:317 |
| 55 | 2026-10-10T05:08:23Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=69825 · rate_mib_s=29889 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → serve-granite42-30b-v030 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-10T05:07:56Z 후 0m27s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:716 |
| 56 | 2026-10-10T05:14:40Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=36926 · rate_mib_s=22423 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → serve-granite42-30b-v030 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-10T05:07:56Z 후 6m44s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:717 |
| 57 | 2026-10-10T05:18:59Z | main | `budget_renew` | `serve-granite42-30b-v030` | budget_renew · floor_mib=16543 · ttl_s=7200 · remaining_before_s=6538 · 창 시작 2026-10-10T05:07:56Z 후 11m03s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:318 |
| 58 | 2026-10-10T05:37:57Z | main | `budget_renew` | `serve-granite42-30b-v030` | budget_renew · floor_mib=16543 · ttl_s=7200 · remaining_before_s=6062 · 창 시작 2026-10-10T05:07:56Z 후 30m01s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:319 |
| 59 | 2026-10-10T05:38:45Z | main | `budget_renew` | `serve-granite42-30b-v030` | budget_renew · floor_mib=16543 · ttl_s=7200 · remaining_before_s=7152 · 창 시작 2026-10-10T05:07:56Z 후 30m49s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:320 |
| 60 | 2026-10-10T05:58:14Z | main | `budget_renew` | `serve-granite42-30b-v030` | budget_renew · floor_mib=16543 · ttl_s=7200 · remaining_before_s=6031 · 창 시작 2026-10-10T05:07:56Z 후 50m18s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:321 |
| 61 | 2026-10-10T06:07:57Z | main | `budget_renew` | `serve-granite42-30b-v030` | budget_renew · floor_mib=16543 · ttl_s=7200 · remaining_before_s=6617 · 창 시작 2026-10-10T05:07:56Z 후 1h00m01s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:322 |
| 62 | 2026-10-10T06:17:44Z | main | `budget_renew` | `serve-granite42-30b-v030` | budget_renew · floor_mib=16543 · ttl_s=7200 · remaining_before_s=6613 · 창 시작 2026-10-10T05:07:56Z 후 1h09m48s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:323 |
| 63 | 2026-10-10T06:27:35Z | main | `budget_renew` | `serve-granite42-30b-v030` | budget_renew · floor_mib=16543 · ttl_s=7200 · remaining_before_s=6609 · 창 시작 2026-10-10T05:07:56Z 후 1h19m39s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:324 |
| 64 | 2026-10-10T06:37:26Z | main | `budget_renew` | `serve-granite42-30b-v030` | budget_renew · floor_mib=16543 · ttl_s=7200 · remaining_before_s=6609 · 창 시작 2026-10-10T05:07:56Z 후 1h29m30s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:325 |
| 65 | 2026-10-10T06:37:57Z | main | `budget_renew` | `serve-granite42-30b-v030` | budget_renew · floor_mib=16543 · ttl_s=7200 · remaining_before_s=7169 · 창 시작 2026-10-10T05:07:56Z 후 1h30m01s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:326 |
| 66 | 2026-10-10T06:47:16Z | main | `budget_renew` | `serve-granite42-30b-v030` | budget_renew · floor_mib=16543 · ttl_s=7200 · remaining_before_s=6641 · 창 시작 2026-10-10T05:07:56Z 후 1h39m20s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:327 |
| 67 | 2026-10-10T06:52:51Z | main | `budget_renew` | `serve-granite42-30b-v030` | budget_renew · floor_mib=16543 · ttl_s=7200 · remaining_before_s=6865 · 창 시작 2026-10-10T05:07:56Z 후 1h44m55s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:328 |
| 68 | 2026-10-10T06:58:26Z | main | `budget_renew` | `serve-granite42-30b-v030` | budget_renew · floor_mib=16543 · ttl_s=7200 · remaining_before_s=6865 · 창 시작 2026-10-10T05:07:56Z 후 1h50m30s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:329 |
| 69 | 2026-10-10T07:04:02Z | main | `budget_renew` | `serve-granite42-30b-v030` | budget_renew · floor_mib=16543 · ttl_s=7200 · remaining_before_s=6864 · 창 시작 2026-10-10T05:07:56Z 후 1h56m06s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:330 |
| 70 | 2026-10-10T07:07:29Z | main | `budget_renew` | `serve-granite42-30b-v030` | budget_renew · floor_mib=16543 · ttl_s=7200 · remaining_before_s=6993 · 창 시작 2026-10-10T05:07:56Z 후 1h59m33s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:331 |
| 71 | 2026-10-10T07:07:58Z | main | `budget_renew` | `serve-granite42-30b-v030` | budget_renew · floor_mib=16543 · ttl_s=7200 · remaining_before_s=7171 · 창 시작 2026-10-10T05:07:56Z 후 2h00m02s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:332 |
| 72 | 2026-10-10T07:10:56Z | main | `budget_renew` | `serve-granite42-30b-v030` | budget_renew · floor_mib=16543 · ttl_s=7200 · remaining_before_s=7022 · 창 시작 2026-10-10T05:07:56Z 후 2h03m00s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:333 |
| 73 | 2026-10-10T07:14:23Z | 미관측 | `measurement` | `measurement (budget window recorded)` | 같은 셀의 다른 측정(측정 노드 축 main) · 시각 = 측정 조립(sweep generated_utc = 인증서 measured_utc · 시작 시각 미기록) · 이 발행의 측정 2026-10-27T01:17:15Z 전 · 발행 기록 camp_26101017_knob_e2e_single__main__granite42_30b_v030(셀 대조 simlog:sweep_index.meta.config_name · verdict PASS) · simlog 사본 generated_utc 2026-10-10T07:14:23Z(같음) · 원장: 이 셀 선언 창 안: serve-granite42-30b-v030 2026-10-10T05:07:56Z~2026-10-10T07:15:02Z (docs/logs/main/events/2026-10.jsonl:315) · serve-granite42-30b-v030 2026-10-10T05:07:56Z~2026-10-10T07:15:03Z (docs/logs/main/events/watchdog.jsonl:715) · 직전 예산 행 budget_renew(serve-granite42-30b-v030) 2026-10-10T07:10:56Z(docs/logs/main/events/2026-10.jsonl:333) · 직후 예산 행 budget_clear 2026-10-10T07:15:02Z(docs/logs/main/events/2026-10.jsonl:334) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/simlog/26101017_57_46_camp_26101017_knob_e2e_single__main__granite42_30b_v030/sweep_index.json:4 |
| 74 | 2026-10-10T07:15:02Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → serve-granite42-30b-v030 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-10T05:07:56Z 후 2h07m06s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:334 |
| 75 | 2026-10-10T07:15:02Z | main | `serve_stop` | — | serve_stop · model=granite-4.2-30b · duration_s=7626 · session=serve-granite42-30b-v030-20261010T050756Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:335 |
| 76 | 2026-10-10T07:15:03Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → serve-granite42-30b-v030 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-10T05:07:56Z 후 2h07m07s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:718 |
| 77 | 2026-10-26T23:07:07Z | main | `budget_declare` | `serve-granite42-30b-v030` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=16543 · weights_mib=55841 · kv_mib=33792 · overhead_mib=18432 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 기동 창 4/4: budget_renew 17회 · 닫힘 budget_clear 2026-10-27T01:22:48Z(선언 후 2h15m41s) · measured_utc 2026-10-27T01:17:15Z 가 이 창 안(이 측정의 기동) | docs/logs/main/events/2026-10.jsonl:701 |
| 78 | 2026-10-26T23:07:08Z | main | `serve_start` | — | serve_start · model=granite-4.2-30b · tensor_parallel_size=1 · topology=single · session=serve-granite42-30b-v030-20261026T230708Z | docs/logs/main/events/2026-10.jsonl:702 |
| 79 | 2026-10-26T23:07:08Z | main | `budget_honored` | `serve-granite42-30b-v030` | budget_honored · floor_mib=16543 · arm_ceiling_mib=13471 · remaining_s=7199 · 창 시작 2026-10-26T23:07:08Z 후 0m00s | docs/logs/main/events/watchdog.jsonl:834 |
| 80 | 2026-10-26T23:07:09Z | main | `budget_renew` | `serve-granite42-30b-v030` | budget_renew · floor_mib=16543 · ttl_s=7200 · remaining_before_s=7198 · 창 시작 2026-10-26T23:07:07Z 후 0m02s | docs/logs/main/events/2026-10.jsonl:703 |
| 81 | 2026-10-26T23:07:23Z | main | `engine_log_window_start` | — | 캡처된 엔진 로그의 첫 시각(꼬리 캡처일 수 있다 — 엔진 시작 시각이 아닐 수 있다) · 선언 2026-10-26T23:07:07Z 후 0m16s · 시계: 컨테이너 시각을 UTC 로 읽어 측정 창 [2026-10-26T23:07:07Z, 2026-10-27T01:22:48Z] 안에 드는 것을 확인 | output/single/benchlog/sweep_granite42-30b-v030/lite_engine_granite42-30b-v030.log:36 |
| 82 | 2026-10-26T23:07:41Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=91922 · rate_mib_s=22622 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → serve-granite42-30b-v030 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-26T23:07:08Z 후 0m33s) | docs/logs/main/events/watchdog.jsonl:835 |
| 83 | 2026-10-26T23:07:43Z | main | `engine_prefetch_disabled` | — | 가중치 auto-prefetch 꺼짐 줄 ×1 — filesystem CIFS(vLLM 로그 문구) | output/single/benchlog/sweep_granite42-30b-v030/lite_engine_granite42-30b-v030.log:58 |
| 84 | 2026-10-26T23:14:04Z | main | `engine_weights_loaded` | — | 가중치 적재 완료 줄 ×1(rank·단계별) · 최장 380.73s · 이 행 = 마지막 줄 | output/single/benchlog/sweep_granite42-30b-v030/lite_engine_granite42-30b-v030.log:73 |
| 85 | 2026-10-26T23:14:04Z | main | `engine_model_loaded` | — | 모델 적재 완료 줄 ×1(rank 별) · 마지막 줄 54.58 GiB · 385.2s | output/single/benchlog/sweep_granite42-30b-v030/lite_engine_granite42-30b-v030.log:74 |
| 86 | 2026-10-26T23:14:10Z | main | `engine_kv_cache_sized` | — | GPU KV cache 135,168 tokens · 요청당 131,072 tokens 기준 최대 동시성 1.03x | output/single/benchlog/sweep_granite42-30b-v030/lite_engine_granite42-30b-v030.log:82 |
| 87 | 2026-10-26T23:15:24Z | main | `engine_init_done` | — | 엔진 초기화(profile·KV 할당·warmup) 80.28s | output/single/benchlog/sweep_granite42-30b-v030/lite_engine_granite42-30b-v030.log:97 |
| 88 | 2026-10-26T23:15:32Z | main | `server_starting` | — | API 서버 기동 — 이 측정 창 선언 2026-10-26T23:07:07Z 후 8m25s · 캡처 로그 첫 시각 2026-10-26T23:07:23Z 후 8m09s | output/single/benchlog/sweep_granite42-30b-v030/lite_engine_granite42-30b-v030.log:104 |
| 89 | 2026-10-26T23:20:35Z | main | `budget_renew` | `serve-granite42-30b-v030` | budget_renew · floor_mib=16543 · ttl_s=7200 · remaining_before_s=6394 · 창 시작 2026-10-26T23:07:07Z 후 13m28s | docs/logs/main/events/2026-10.jsonl:704 |
| 90 | 2026-10-26T23:37:09Z | main | `budget_renew` | `serve-granite42-30b-v030` | budget_renew · floor_mib=16543 · ttl_s=7200 · remaining_before_s=6206 · 창 시작 2026-10-26T23:07:07Z 후 30m02s | docs/logs/main/events/2026-10.jsonl:705 |
| 91 | 2026-10-26T23:40:29Z | main | `budget_renew` | `serve-granite42-30b-v030` | budget_renew · floor_mib=16543 · ttl_s=7200 · remaining_before_s=7000 · 창 시작 2026-10-26T23:07:07Z 후 33m22s | docs/logs/main/events/2026-10.jsonl:706 |
| 92 | 2026-10-27T00:00:06Z | main | `budget_renew` | `serve-granite42-30b-v030` | budget_renew · floor_mib=16543 · ttl_s=7200 · remaining_before_s=6023 · 창 시작 2026-10-26T23:07:07Z 후 52m59s | docs/logs/main/events/2026-10.jsonl:707 |
| 93 | 2026-10-27T00:07:09Z | main | `budget_renew` | `serve-granite42-30b-v030` | budget_renew · floor_mib=16543 · ttl_s=7200 · remaining_before_s=6777 · 창 시작 2026-10-26T23:07:07Z 후 1h00m02s | docs/logs/main/events/2026-10.jsonl:708 |
| 94 | 2026-10-27T00:19:43Z | main | `budget_renew` | `serve-granite42-30b-v030` | budget_renew · floor_mib=16543 · ttl_s=7200 · remaining_before_s=6446 · 창 시작 2026-10-26T23:07:07Z 후 1h12m36s | docs/logs/main/events/2026-10.jsonl:709 |
| 95 | 2026-10-27T00:29:43Z | main | `budget_renew` | `serve-granite42-30b-v030` | budget_renew · floor_mib=16543 · ttl_s=7200 · remaining_before_s=6600 · 창 시작 2026-10-26T23:07:07Z 후 1h22m36s | docs/logs/main/events/2026-10.jsonl:710 |
| 96 | 2026-10-27T00:37:09Z | main | `budget_renew` | `serve-granite42-30b-v030` | budget_renew · floor_mib=16543 · ttl_s=7200 · remaining_before_s=6754 · 창 시작 2026-10-26T23:07:07Z 후 1h30m02s | docs/logs/main/events/2026-10.jsonl:711 |
| 97 | 2026-10-27T00:39:43Z | main | `budget_renew` | `serve-granite42-30b-v030` | budget_renew · floor_mib=16543 · ttl_s=7200 · remaining_before_s=7046 · 창 시작 2026-10-26T23:07:07Z 후 1h32m36s | docs/logs/main/events/2026-10.jsonl:712 |
| 98 | 2026-10-27T00:49:44Z | main | `budget_renew` | `serve-granite42-30b-v030` | budget_renew · floor_mib=16543 · ttl_s=7200 · remaining_before_s=6599 · 창 시작 2026-10-26T23:07:07Z 후 1h42m37s | docs/logs/main/events/2026-10.jsonl:713 |
| 99 | 2026-10-27T00:55:24Z | main | `budget_renew` | `serve-granite42-30b-v030` | budget_renew · floor_mib=16543 · ttl_s=7200 · remaining_before_s=6860 · 창 시작 2026-10-26T23:07:07Z 후 1h48m17s | docs/logs/main/events/2026-10.jsonl:714 |
| 100 | 2026-10-27T01:01:04Z | main | `budget_renew` | `serve-granite42-30b-v030` | budget_renew · floor_mib=16543 · ttl_s=7200 · remaining_before_s=6860 · 창 시작 2026-10-26T23:07:07Z 후 1h53m57s | docs/logs/main/events/2026-10.jsonl:715 |
| 101 | 2026-10-27T01:06:44Z | main | `budget_renew` | `serve-granite42-30b-v030` | budget_renew · floor_mib=16543 · ttl_s=7200 · remaining_before_s=6860 · 창 시작 2026-10-26T23:07:07Z 후 1h59m37s | docs/logs/main/events/2026-10.jsonl:716 |
| 102 | 2026-10-27T01:07:09Z | main | `budget_renew` | `serve-granite42-30b-v030` | budget_renew · floor_mib=16543 · ttl_s=7200 · remaining_before_s=7175 · 창 시작 2026-10-26T23:07:07Z 후 2h00m02s | docs/logs/main/events/2026-10.jsonl:717 |
| 103 | 2026-10-27T01:10:14Z | main | `budget_renew` | `serve-granite42-30b-v030` | budget_renew · floor_mib=16543 · ttl_s=7200 · remaining_before_s=7015 · 창 시작 2026-10-26T23:07:07Z 후 2h03m07s | docs/logs/main/events/2026-10.jsonl:718 |
| 104 | 2026-10-27T01:13:45Z | main | `budget_renew` | `serve-granite42-30b-v030` | budget_renew · floor_mib=16543 · ttl_s=7200 · remaining_before_s=6989 · 창 시작 2026-10-26T23:07:07Z 후 2h06m38s | docs/logs/main/events/2026-10.jsonl:719 |
| 105 | 2026-10-27T01:22:48Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → serve-granite42-30b-v030 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-26T23:07:07Z 후 2h15m41s) | docs/logs/main/events/2026-10.jsonl:720 |
| 106 | 2026-10-27T01:22:48Z | main | `serve_stop` | — | serve_stop · model=granite-4.2-30b · duration_s=8140 · session=serve-granite42-30b-v030-20261026T230708Z | docs/logs/main/events/2026-10.jsonl:721 |
| 107 | 2026-10-27T01:22:49Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → serve-granite42-30b-v030 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-26T23:07:08Z 후 2h15m41s) | docs/logs/main/events/watchdog.jsonl:836 |

**기동 시도**(예산 선언 `budget_declare` 마다 한 시도 · 같은 노드의 이어지는 행과 선언 없는 노드(엔진 로그)의 그 시각 행을 묶었다 · 사살 · 트립 · 거부는 닫힘과 따로 센다 · `measurement` 행은 측정 기록이라 시도로 세지 않는다(위 표에만) — 해석은 저작자 몫)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 1 | main | `serve-granite42-30b-v030` | 2026-10-26T23:07:07Z | 17회 · 첫 2026-10-26T23:07:09Z | 없음 | `budget_clear` 2026-10-27T01:22:48Z | 12 |

**캠페인 경계 이전 기동 7회**(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀 라벨로 묶인 3회 · 라벨 다름(같은 노드 · 창 밖 이 셀 행과 시간이 겹치는 다른 라벨 선언 창) 4회 · 어느 쪽도 이 셀의 시도로 세지 않는다 · 행 전문 = 01 §1.4)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 앞선-1 | main | `camp-26100110-granite`(라벨 다름 — 같은 노드 · 이 셀 라벨 아님) | 2026-10-01T01:42:26Z | 1회 · 첫 2026-10-01T01:42:27Z | 없음 | `budget_clear` 2026-10-01T01:49:52Z | 5 |
| 앞선-2 | main | `camp-26100110-granite-try2`(라벨 다름 — 같은 노드 · 이 셀 라벨 아님) | 2026-10-01T01:50:26Z | 1회 · 첫 2026-10-01T01:50:28Z | 없음 | `budget_clear` 2026-10-01T01:57:22Z | 5 |
| 앞선-3 | main | `camp-26100110-granite-try3`(라벨 다름 — 같은 노드 · 이 셀 라벨 아님) | 2026-10-01T01:57:43Z | 1회 · 첫 2026-10-01T01:57:44Z | 없음 | `budget_clear` 2026-10-01T02:04:49Z | 5 |
| 앞선-4 | main | `camp-26100110-granite-try4`(라벨 다름 — 같은 노드 · 이 셀 라벨 아님) | 2026-10-01T02:05:04Z | 1회 · 첫 2026-10-01T02:05:05Z | 없음 | `budget_clear` 2026-10-01T02:23:54Z | 6 |
| 앞선-5 | main | `serve-granite42-30b-v030` | 2026-10-01T08:26:06Z | 1회 · 첫 2026-10-01T08:26:08Z | 없음 | `budget_clear` 2026-10-01T08:41:10Z | 6 |
| 앞선-6 | main | `serve-granite42-30b-v030` | 2026-10-10T04:55:14Z | 1회 · 첫 2026-10-10T04:55:15Z | 없음 | `budget_clear` 2026-10-10T05:07:27Z | 5 |
| 앞선-7 | main | `serve-granite42-30b-v030` | 2026-10-10T05:07:56Z | 17회 · 첫 2026-10-10T05:07:57Z | 없음 | `budget_clear` 2026-10-10T07:15:02Z | 6 |
<!-- /FACT:event_timeline -->

<!-- FACT:measurement_env -->
_측정 실행의 env 관측 없음 — 측정 엔진 로그에서 env echo 줄(`<KEY> set by environment to <VALUE>` 등)을 찾지 못했다(evidence.measurement_env_observed · 부재 ≠ 미설정)._
<!-- /FACT:measurement_env -->

> 이 절이 답하는 질문: 수신자가 이 zip 만으로 이 셀을 다시 띄우려면 어떤 순서로 무엇을 하고, 각 단계의 성공을 무엇으로 판정하며, 얼마나 기다려야 하는가?

1. 관리NAS의 체크포인트와 모델폴더의 parser플러그인을 compose의컨테이너경로에read-only로마운트한다. env자리는artifacts/compose/.env.template과triplet셀env다. PLE스테이징은없다.
2. 기존0.30.0이미지를사용했으며 새빌드는없다. FACT층창은실제벽시계빌드소요가아니다. 빌드재구성명령은실린파일검증표와함께읽는다.
3. 공식기동에overhead18432MiB·ready-max900을명시했고, 이번health대기 510초를측정했다. 과거475/510초와섞지않는다. 기동시도수/닫힘은이번FACT표를따르며health200+completion실제응답으로확인했다. 기능HTTP기한180초는느린dense모델의기능검사용이며Full기준은바꾸지않았다.
4. 총124608−weights55841−KV33792−overhead18432=floor16543MiB·arm13471MiB는이번보호선언이다. 엔진checkpoint54.53GiB·loading54.58GiB와구분한다. bench4096은이구성상한6303이하에서선언했다.
5. Full은lite뒤GuideLLM1/2/4/8각3회이며, 신규측정키 2026-10-27T01:17:15Z·posthealth·오류·실제PASS를함께확인했다. 위재구성명령과03의측정시점도구원문은서로다른종류의기록이다.
6. TP1이라노드간Ray/NCCL통신경로는없다. 현재엔진config·backend·graph와일반completionreasoning관측은testlog§4의원문전사를따른다. CIFS모델로수분로드와절대KV때문에profiling을건너뛴분기를실패로단정하지않는다.

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

### 가중치와 parser
이번획득모드는managed NAS 기존자산재사용이며다운로드하지않았다. ibm-granite/granite-4.2-30b의00 §0.2revision과 `<manifest.nas_model_path>/IBM/Granite-4.2/granite-4.2-30b`를대조한다. `test -r <manifest.nas_model_path>/IBM/Granite-4.2/granite-4.2-30b/config.json` 및모델폴더 `granite_thinking_parser.py`의읽기가준비조건이다(artifacts/triplet runner·yaml; 경로형상예시이며명령자체새실행검증주장은아님). 엔진checkpoint54.53GiB는du값이아니다. 한노드만필요하다.

### PLE·기타스테이징
PLE는해당없음 — 모델과트리플렛에설정없다. GuideLLM토크나이저스테이징은측정도구가만든다. 추가가중치변환명령은없다.

### 네트워크
기존서빙이미지·GuideLLM0.7.3·가중치를사용했다. 새source빌드는Dockerfile의basepull·apt·git·pip접속이필요하므로오프라인환경은사전승인된이미지와체크포인트를준비한다. 계보에실행원문이없는새다운로드명령을검증된것처럼추가하지않는다.
