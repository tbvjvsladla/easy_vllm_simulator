# 01 · 산출물 — 무엇이 실제로 쓰였고, 어떻게 다시 띄우는가

## 1.1 적용 판정 표
> 기계가 적용 증거(빌드 원장 · 원장이 없으면 라벨된 재구성) × 파일 × 선언을 대사한 결과다. `artifacts/` 에는 **적용된 것만** 실렸다 — 빌드 때 스스로 skip 된 패치와 쓰이지 않은 평면의 파일은 표에만 남고 싣지 않는다.

<!-- FACT:slots -->
| 슬롯 | 실림 | 파일 | 신호 | 적용 증거 | 사유 |
|---|---|---|---|---|---|
| `build_patch_post` | 안 실림 | — | 1-signal | build-ledger:docker-image(main-local):easy-vllm:0.31.0-cu133-aarch64-source:/opt/easy-vllm/build_ledger.json | 적용 판정 없음 — 원장 관측(기계 파생) |
| `build_patch_pre` | 안 실림 | — | 1-signal | build-ledger:docker-image(main-local):easy-vllm:0.31.0-cu133-aarch64-source:/opt/easy-vllm/build_ledger.json | 적용 판정 없음 — 원장 관측(기계 파생) |
| `build_recipe` | 실림 | `artifacts/build_recipe/Dockerfile.source-build` · `artifacts/build_recipe/requirements.txt` | 3-signal | build-ledger:build-ledger.dockerfile | (a) NGC `pytorch:26.07-py3` 위에 vLLM `v0.31.0` 을 소스로 짓는 **stock** 스켈레톤이다 — 빌드 패치 슬롯은 비어 있고(plan_26101410_vllm031_bump_14B이하_4종_양노드_서빙_lite §3 "싱글 빌드 슬롯 … 비어 있음"), NGC 의 torch 를 그대로 쓰도록 use_existing_torch 단계를 돌리며(0.31.0 부터 `tools/` 로 옮겨진 경로를 자기적응 — 파일 주석), 인라인 `strip-hoist` 단계를 품는다. (b) 이 셀의 벽(W1 · W2)은 측정 · 발행 하네스 쪽이라 이 레시피와 무관하다 — 필요 근거는 빌드 트랙 판정(같은 plan §3 · torch ≥2.11 ABI 벽)이다. (c) 1.1 표가 `gen… |
| `compose` | 실림 | `artifacts/compose/.env.template` · `artifacts/compose/docker-compose.yaml` | 2-signal | file:output/single/docker-compose.yaml | single 서빙 경로다 — 이미지(`${IMAGE_TAG}`) · 관리 NAS 를 `/app/models` 로 읽기 전용 마운트(`${NAS_MODEL_PATH}`) · configs 마운트 · 포트 매핑. 측정 당시 개정은 커밋 12ef50a 바이트다(1.1 실린 리비전). 패치가 아니다. 같은 노드 두 모델의 분리는 이 파일이 아니라 셀 env 의 `COMPOSE_PROJECT_NAME` · `CONTAINER_NAME` · `SERVING_PORT` 가 맡는다(plan_26101410 §4 D6). 러너가 source 하는 `arm_patch.sh` 는 런타임 패치 파일이 없어 no-op(not-armed)이라 싣지 않았다. |
| `fork_pin` | 안 실림 | — | 1-signal | none:.claude/policies/arch_variant_ledger.json | 셀 env 에 VARIANT 줄 없음 = stock(workflow.md §변종 좌표의 거처)(기계 파생) |
| `runtime_patch` | 안 실림 | — | 1-signal | none:output/single/configs/minicpm5-2b-v031_patch.py | output/single/configs/minicpm5-2b-v031_patch.py 없음 — arm_patch.sh 는 no-op(런타임 패치 불해당)(기계 파생) |
| `triplet` | 실림 | `artifacts/triplet/.env.minicpm5-2b-v031` · `artifacts/triplet/minicpm5-2b-v031.sh` · `artifacts/triplet/minicpm5-2b-v031.yaml` | 2-signal | file:output/single/configs/minicpm5-2b-v031.yaml | 서빙 노브 4개(§1.3)와 러너 플래그 `--enable-auto-tool-choice --tool-call-parser minicpm5 --reasoning-parser qwen3`, 셀 env(호스트 포트 8002 · 이미지 태그 · compose 프로젝트명)다. yaml 의 `port: 8000` 은 컨테이너 내부 포트이고 compose 가 `${SERVING_PORT}` 로 매핑한다(yaml 주석). lite 측정이 이 config 로 돌았다(lite raw `config_name`). |

**레시피 경고 · 실린 리비전**(기계 관측 — 수신자 빌드가 여기서 갈라질 수 있다)

- ⚠ `build_recipe` `output/single/Dockerfile.source-build` — 실린 바이트(worktree)가 측정 이미지 docker history 의 RUN·COPY 와 전수 일치하지 않는다(16/17) · 이미지 빌드 뒤 바뀐 레시피일 수 있다 — 마지막 커밋이 이미지 Created 뒤(이미지 Created 2026-10-14T01:45:19Z · 마지막 커밋 2026-10-14T03:07:26Z)
- `build_recipe` `output/single/Dockerfile.source-build` 실린 리비전: 리비전 미특정 · 선택 방법 `worktree(어느 개정도 이미지 history 와 더 맞지 않는다 · 작업트리 16/17 — recipe_vs_image 경고 유지)` · docker history 대조 16/17 · 실린 바이트 `worktree` · 쓰인 바이트임을 관측으로 확인 아니오
- `build_recipe` `output/single/requirements.txt` 실린 리비전: 리비전 미특정 · 선택 방법 `worktree(빌드 원장 sha256 일치 — 쓰인 바이트 관측 확인 = 예)` · docker history 대조 없음 · 실린 바이트 `worktree` · 쓰인 바이트임을 관측으로 확인 예
- `compose` `output/single/docker-compose.yaml` 실린 리비전: 커밋 `12ef50a89b86` · 선택 방법 `worktree(mtime 2026-10-14T01:06:26Z ≤ 측정 2026-10-16T12:38:15Z(certificate.measured_utc) · 바이트 = 측정 전 마지막 커밋 12ef50a89b86)` · docker history 대조 없음 · 실린 바이트 `worktree` · 근거 `mtime≤measured` · 쓰인 바이트임을 관측으로 확인 예

**싣지 않은 파일**(슬롯별 사유 — 빌드 때 skip 된 패치는 아래 적용 집합 표에 있다)

- `build_patch_post` known-non-patch 1건: `.gitkeep`
- `build_patch_pre` known-non-patch 1건: `.gitkeep`
- `compose` `arm_patch.sh` — `not-armed(output/single/configs/minicpm5-2b-v031_patch.py 없음 — arm_patch.sh 는 no-op)`

**파일별 검증 · 관련성**(`generated-unverified` = 실행 검증되지 않은 생성물 — 파일 첫 줄 경고 · PAYLOAD 파일 기록과 같은 표시 · 관련성 = ① 패치 선언 대상 × 모델 config ② 엔진 로그 발화 서명 — 둘 다 관측일 때만 required/inactive-inferred)

| 슬롯 | 파일 | 검증 | 검증 근거 | 관련성 | 관련성 근거 |
|---|---|---|---|---|---|
| `build_recipe` | `artifacts/build_recipe/Dockerfile.source-build` | `generated-unverified` · renderer | 실린 Dockerfile.source-build(worktree)이 측정 이미지 docker history RUN·COPY 와 16/17 만 일치 — 이 이미지를 지은 바이트임을 관측으로 확인하지 못했다(_select_dockerfile · worktree(어느 개정도 이미지 history 와 더 맞지 않는다 · 작업트리 16/17 — recipe_vs_image 경고 유지)) | `unknown` | 이 셀은 이 경로로 실행되지 않았다(generated-unverified · 생성 renderer) — 실행 기록이 없어 필요 여부를 판정하지 않는다(참고 형상) |
| `build_recipe` | `artifacts/build_recipe/requirements.txt` | `verified` · cell-run | 쓰인 레시피(build-ledger.dockerfile) — 개정 선택 slots.build_recipe.evidence.selected_revisions | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/.env.template` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/docker-compose.yaml` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/.env.minicpm5-2b-v031` | `verified` · cell-run | 셀 트리플렛 — 측정 대조 없음 | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/minicpm5-2b-v031.sh` | `verified` · cell-run | 셀 트리플렛 — 측정 대조 없음 | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/minicpm5-2b-v031.yaml` | `verified` · cell-run | 셀 트리플렛 — 측정 대조 없음 | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
<!-- /FACT:slots -->

<!-- FACT:applied_set -->
**적용 집합** — 상태 `observed` · 출처 `docker-image(main-local):easy-vllm:0.31.0-cu133-aarch64-source:/opt/easy-vllm/build_ledger.json`

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
| attestation | absent | output/single/benchlog/attestation_minicpm5-2b-v031.json(부재) |
| local-image | observed | easy-vllm:0.31.0-cu133-aarch64-source |
<!-- /FACT:applied_set -->

## 1.2 슬롯별 적용 사유
> 이 절이 답하는 질문: 실린 산출물 하나하나는 무엇을 고치며, 왜 이 셀에 필요했고, 이 셀에서 실제로 발화했으며, 언제 불필요해지는가?

### build_recipe — `Dockerfile.source-build`

(a) NGC `pytorch:26.07-py3` 위에 vLLM `v0.31.0` 을 소스로 짓는 **stock** 스켈레톤이다 — 빌드 패치 슬롯은 비어 있고(plan_26101410_vllm031_bump_14B이하_4종_양노드_서빙_lite §3 "싱글 빌드 슬롯 … 비어 있음"), NGC 의 torch 를 그대로 쓰도록 use_existing_torch 단계를 돌리며(0.31.0 부터 `tools/` 로 옮겨진 경로를 자기적응 — 파일 주석), 인라인 `strip-hoist` 단계를 품는다. (b) 이 셀의 벽(W1 · W2)은 측정 · 발행 하네스 쪽이라 이 레시피와 무관하다 — 필요 근거는 빌드 트랙 판정(같은 plan §3 · torch ≥2.11 ABI 벽)이다. (c) 1.1 표가 `generated-unverified` 로 표시했다 — 실린 바이트는 측정 이미지 history 와 16/17 만 맞아 이 이미지를 지은 바이트임이 관측으로 확인되지 않았다. 인라인 `strip-hoist` 는 빌드 원장이 `skipped`(register_opaque_type 이 hoist 를 받음)로 기록했다(결과 출처 `build-ledger`). (d) skip 은 아래 원문의 `needs` 판정에서 갈린다 — hoist 를 받는 torch 이거나 opaque_object 가 없으면 `needs = False` 로 패치 본문을 건너뛰고 빌드를 멈추지 않는다:

> [원문] Dockerfile.source-build §L95-100
> try:
>     from torch._library.opaque_object import register_opaque_type
>     needs = "hoist" not in inspect.signature(register_opaque_type).parameters
> except Exception:
>     needs = False  # opaque_object 미존재(torch 2.10대) → 패치 불요
> if needs:

이 레시피가 불필요해지는 조건은 빌드 트랙 판정이 source-build 를 요구하지 않게 되는 때뿐이다(이 계보에서 시험하지 않음).

### build_recipe — `requirements.txt`

vLLM 0.31.0 wheel METADATA 의 Requires-Dist 를 옮긴 런타임 의존 목록이다(torch 계열은 베이스 제공이라 제외 · `fastapi<0.137.0` · `transformers<5.15.0` 천장 — 파일 머리 주석). 1.1 표에서 `verified · cell-run`(빌드 원장 sha256 일치)이다. 이 모델 고유의 요구가 아니라 공유 이미지의 일부이고, vLLM 버전이 바뀌면 다시 생성한다(머리 주석의 regen 경로).

### compose — `docker-compose.yaml` · `.env.template`

single 서빙 경로다 — 이미지(`${IMAGE_TAG}`) · 관리 NAS 를 `/app/models` 로 읽기 전용 마운트(`${NAS_MODEL_PATH}`) · configs 마운트 · 포트 매핑. 측정 당시 개정은 커밋 12ef50a 바이트다(1.1 실린 리비전). 패치가 아니다. 같은 노드 두 모델의 분리는 이 파일이 아니라 셀 env 의 `COMPOSE_PROJECT_NAME` · `CONTAINER_NAME` · `SERVING_PORT` 가 맡는다(plan_26101410 §4 D6). 러너가 source 하는 `arm_patch.sh` 는 런타임 패치 파일이 없어 no-op(not-armed)이라 싣지 않았다.

### triplet — `minicpm5-2b-v031.yaml` · `.sh` · `.env.minicpm5-2b-v031`

서빙 노브 4개(§1.3)와 러너 플래그 `--enable-auto-tool-choice --tool-call-parser minicpm5 --reasoning-parser qwen3`, 셀 env(호스트 포트 8002 · 이미지 태그 · compose 프로젝트명)다. yaml 의 `port: 8000` 은 컨테이너 내부 포트이고 compose 가 `${SERVING_PORT}` 로 매핑한다(yaml 주석). lite 측정이 이 config 로 돌았다(lite raw `config_name`).

### 싣지 않은 슬롯

- `build_patch_pre` · `build_patch_post` — `.gitkeep` 뿐(stock 빌드).
- `runtime_patch` — `minicpm5-2b-v031_patch.py` 없음.
- `fork_pin` — 셀 env 에 VARIANT 줄 없음 = stock.

부재는 이 계보에서 패치가 필요한 벽이 없었다는 기록이지, 다른 HW · 다른 vLLM 에서도 불필요하다는 판정이 아니다.

## 1.3 값의 지위표
> 서빙 설정의 노브 **전부**와 기계가 모은 후보 지위다(lockset 출처 · yaml 주석 · 엔진 기본값). 최종 지위는 아래 `kind: value-status` 블록이 정한다 — 표의 노브마다 블록이 정확히 하나 있어야 린터를 통과한다.

<!-- FACT:value_status -->
서빙 노브 4개 — 노브마다 `kind: value-status` 블록이 **정확히 1개** 있어야 한다(노브·값은 이 표의 글자 그대로 · 값 칸의 `\|` 는 `|` 로 적는다).

| 노브 | 값 | 후보 지위 | 후보 근거 | yaml 주석 | 출처 |
|---|---|---|---|---|---|
| `gpu-memory-utilization` | 0.39 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `max-model-len` | 32768 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `max-num-seqs` | 8 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `kv-cache-memory-bytes` | 37580963840 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
<!-- /FACT:value_status -->

> 이 절이 답하는 질문: 각 서빙 노브의 값은 이 셀에서 어떤 지위인가 — 조정됐나, 승계됐나, 음성대조로 일부러 둔 것인가, 엔진 기본값인가, 필요조건인가?

```hint-event
id: V1
kind: value-status
노브: gpu-memory-utilization
값: 0.39
지위: inherited
근거: 사람 결정 D4 — 64GB Spark 천장(≈47.7GiB)을 호스트 총량으로 환산한 값(두 인스턴스 기동 게이트 공존용) · 이 계보에서 바꿔 잰 기록 없음 · KV 클램프 지정으로 엔진이 이 값을 KV 산정에 쓰지 않는다고 로그에 명시(L74)
출처: [plan_26101410_vllm031_bump_14B이하_4종_양노드_서빙_lite §4, output/single/benchlog/lite_publish/minicpm5-2b-v031_20261016T123814Z/lite_engine_head_minicpm5-2b-v031.log]
```

```hint-event
id: V2
kind: value-status
노브: max-model-len
값: 32768
지위: inherited
근거: 사용자 지시로 4모델 공통 고정(LangGraph RAG 용) · 줄이거나 늘려 잰 기록 없음 · lite 는 512/128 입력만 보내 이 길이를 시험하지 않았다
출처: [plan_26101410_vllm031_bump_14B이하_4종_양노드_서빙_lite §1, minicpm5-2b-v031.yaml]
```

```hint-event
id: V3
kind: value-status
노브: max-num-seqs
값: 8
지위: inherited
근거: 동시성 요구 8(D5 사람 결정)을 그대로 씀 · 바꿔 잰 기록 없음 · lite 는 동시성 1 만 보냈다
출처: [plan_26101410_vllm031_bump_14B이하_4종_양노드_서빙_lite §4, minicpm5-2b-v031.yaml]
```

```hint-event
id: V4
kind: value-status
노브: kv-cache-memory-bytes
값: 37580963840
지위: inherited
근거: D4 절대 클램프 방침으로 앞선 캠페인에서 손 산정한 35.0 GiB(계보 표 안에는 산정 기록이 없고 이 계보 밖 문서 devlog_26101412 · testlog_26101412 가 KV = 천장 − 가중치 − 오버헤드 · max_safe 선택 · kv_source=hand 로 기록한다) · 이 계보에서 조정 없음 · 엔진 보고 873,808 토큰 · 32768 기준 최대 동시성 26.67x 로 max-num-seqs 8 보다 크다(음성대조라는 기록 없음)
출처: [plan_26101410_vllm031_bump_14B이하_4종_양노드_서빙_lite §4, output/single/benchlog/lite_publish/minicpm5-2b-v031_20261016T123814Z/lite_engine_head_minicpm5-2b-v031.log]
```

네 노브 모두 승계값이다 — 이 계보(lite 등급 분리 · lite-only 셀)는 서빙 설정을 건드리지 않고 이미 상주하던 서빙에 lite 1회를 보냈다. 복사하면 위험한 값은 둘이다: `gpu-memory-utilization 0.39` 는 "같은 노드에 다른 모델이 함께 뜬다" 는 전제의 환산값이라 단독 서빙 · 전용 VRAM 에서는 의미가 없고, `kv-cache-memory-bytes` 는 필요량(plan §모델 4종: 32k 1시퀀스 1.3GiB)보다 훨씬 크게 잡혀 있어 그대로 옮기면 작은 메모리 호스트에서 기동이 거부되거나 다른 프로세스의 몫을 빼앗는다. lite 는 이 값들 중 어느 것도 한계까지 시험하지 않았다.

## 1.4 재현 절차
> 셀 실행 기록에서 기계가 파생한 명령 순서와 단계별 소요다. 소요가 `미관측` 인 단계는 시각 기록이 없다는 뜻이다(0 이 아니다). 그 아래는 블랙박스 이벤트 원장에서 이 셀의 행을 기계가 뽑아 기동 시도로 묶은 것이고, 마지막은 측정한 실행의 엔진 로그가 echo 한 env 다(측정 당시 값의 정본).

<!-- FACT:reproduce -->
| 순서 | 단계 | 성공 판정 | 소요 | 관측 구간(UTC) | 소요 출처 | 명령 출처 |
|---|---|---|---|---|---|---|
| 1 | build | 빌드 OK · 이미지 실재 | 층 창 38분 19초(docker history 첫 층 → 끝 층 CreatedAt · 캐시 재사용 층 포함 — 빌드 소요 아님) | 2026-10-14T01:07:00Z → 2026-10-14T01:45:19Z (layer-window) | docker history --no-trunc --format '{{json .}}' easy-vllm:0.31.0-cu133-aarch64-source · 우리 Dockerfile 층 24개의 CreatedAt 첫 층 → 끝 층(캐시 재사용 층은 이전 빌드 시각 — 빌드 소요 아님) | reconstructed(build-ledger build_args + compose build) |
| 2 | serve | health 200 + 추론 1회(스모크 판정) | 미관측(관측 시각: 2026-10-16T12:38:15Z) | ? → 2026-10-16T12:38:15Z (none) | 미관측(예산 선언 또는 첫 측정 시각 없음) | derived(스모크 사용법) |
| 3 | bench | 리포트 발행(+PASS 면 인증서) | ≈ 0분 34초(두 끝 시계 다름 — 컨테이너 date · 호스트 UTC · 근사) | 2026-10-16T12:38:15Z → 2026-10-16T12:38:49Z (clock-mixed) | 시작 = output/single/benchlog/lite_publish/minicpm5-2b-v031_20261016T123814Z/lite_raw_minicpm5-2b-v031.json measured_utc(= bench_report 머리 생성일 · lite_bench.sh 가 cold 레그 직전(부하 직전)에 찍는 측정 **시작** 시각) → 끝 = output/single/benchlog/lite_publish/minicpm5-2b-v031_20261016T123814Z/lite_warm_minicpm5-2b-v031.json date(마지막 레그 결과 저장 · 컨테이너 시계 = UTC 가정) | reconstructed(bench json) |

**빌드 층 시각**(관측 · 소요가 아니라 층 창 — 캐시 재사용 층은 이전 빌드의 시각을 가진다)

- `build` 이미지 층 CreatedAt(docker history): 2026-10-14T01:07:00Z → 2026-10-14T01:45:19Z · 층 24개 · 출처 `docker history --no-trunc --format '{{json .}}' easy-vllm:0.31.0-cu133-aarch64-source` — 우리 Dockerfile 층의 CreatedAt 범위 — 캐시 재사용 층은 이전 빌드 시각을 가진다(소요 아님)

**단계별 명령**(기계 파생 — `명령 출처` 가 `derived(…)` 면 사용법에서 파생 · `reconstructed(…)` 면 괄호 안 관측(docker history build-arg · compose build · bench JSON 필드)에서 재구성한 명령이다. 어느 쪽도 실행 원문(로그에 남은 줄)은 아니다)

**1. build**

```sh
# 실린 artifacts/build_recipe/Dockerfile.source-build(실린 개정: worktree — 이미지를 지은 바이트임을 관측으로 확인 못함(docker history RUN·COPY 16/17))를 output/single/Dockerfile.source-build 자리에 두고 컨텍스트에서 짓는다.
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
# 동등 경로(compose): docker compose -f output/single/docker-compose.yaml --env-file output/single/envs/.env.minicpm5-2b-v031 build
```

**2. serve**

```sh
bash .claude/skills/upstream-version-watch/scripts/single_serve_up.sh minicpm5-2b-v031
```

**3. bench**

```sh
# bench JSON 필드로 복원: backend · model_id→--model · tokenizer_id→--tokenizer · num_prompts · max_concurrency · request_rate · burstiness · 요청당 입출력 길이 = total_*_tokens ÷ completed(나누어떨어질 때만 — 평균이다 · 요청별 길이는 JSON 에 없다) · 나누어떨어지면 `--dataset-name random` 으로 추정했다(측정 도구가 그 꼴로 부른다 — 아래 도구 원문).
# JSON 에 없는 인자(--endpoint · --base-url · --ignore-eos · --num-warmups · --temperature · --seed · --random-range-ratio · --trust-remote-code)는 측정 도구가 정했다 — level_* = run_bench.sh · lite_cold/lite_warm = lite_bench.sh(.claude/skills/adversarial-benchmark/scripts/) · 셀 스윕 순서는 sweep_bench.sh.
# 호출(재구성 — lite_raw config_name · topology · backend + 바인딩된 경량 리포트 = --publish-report · 원문 호출 줄 아님): bash .claude/skills/adversarial-benchmark/scripts/lite_bench.sh minicpm5-2b-v031 --topology single --backend openai-chat --publish-report
# 측정 창: 시작 2026-10-16T12:38:15Z(output/single/benchlog/lite_publish/minicpm5-2b-v031_20261016T123814Z/lite_raw_minicpm5-2b-v031.json measured_utc(= bench_report 머리 생성일 · lite_bench.sh 가 cold 레그 직전(부하 직전)에 찍는 측정 **시작** 시각)) → 끝 2026-10-16T12:38:49Z(output/single/benchlog/lite_publish/minicpm5-2b-v031_20261016T123814Z/lite_warm_minicpm5-2b-v031.json date(마지막 레그 결과 저장 · 컨테이너 시계 = UTC 가정))
# output/single/benchlog/lite_publish/minicpm5-2b-v031_20261016T123814Z/lite_cold_minicpm5-2b-v031.json (date 20261016-123826) · ⚠ --random-input-len/--random-output-len = total ÷ completed 평균(chat 템플릿 포함 실측 토큰 · 측정 도구 원문 인자 미관측 — lite_bench.sh 원문 인자와 다를 수 있다)
vllm bench serve --backend openai-chat --model minicpm5-2b --tokenizer /app/models/Openbmb/MiniCPM5/MiniCPM5-2B --dataset-name random --random-input-len 520 --random-output-len 128 --num-prompts 1 --max-concurrency 1 --request-rate inf
# output/single/benchlog/lite_publish/minicpm5-2b-v031_20261016T123814Z/lite_warm_minicpm5-2b-v031.json (date 20261016-123849) · ⚠ --random-input-len/--random-output-len = total ÷ completed 평균(chat 템플릿 포함 실측 토큰 · 측정 도구 원문 인자 미관측 — lite_bench.sh 원문 인자와 다를 수 있다)
vllm bench serve --backend openai-chat --model minicpm5-2b --tokenizer /app/models/Openbmb/MiniCPM5/MiniCPM5-2B --dataset-name random --random-input-len 520 --random-output-len 128 --num-prompts 3 --max-concurrency 1 --request-rate inf
```

**슬롯 → 빌드 컨텍스트 매핑**(빌드 컨텍스트 = `output/<토폴로지>/` · 원천 = 실린 Dockerfile 의 COPY · compose 의 env_file · volumes · command — `—` 는 빌드 컨텍스트 입력이 아니다)

| zip 경로 | 빌드 컨텍스트 자리 | 근거 |
|---|---|---|
| `artifacts/triplet/.env.minicpm5-2b-v031` | `envs/.env.minicpm5-2b-v031` | compose env_file envs/.env.${CONFIG_FILE}(셀 env) |
| `artifacts/triplet/minicpm5-2b-v031.sh` | `configs/minicpm5-2b-v031.sh` | compose volumes ./configs:/app/configs |
| `artifacts/triplet/minicpm5-2b-v031.yaml` | `configs/minicpm5-2b-v031.yaml` | compose volumes ./configs:/app/configs |
| `artifacts/build_recipe/Dockerfile.source-build` | `Dockerfile.source-build` | compose build.dockerfile Dockerfile.source-build(리터럴) · build.context . |
| `artifacts/build_recipe/requirements.txt` | `requirements.txt` | Dockerfile.source-build COPY requirements.txt |
| `artifacts/compose/.env.template` | `.env` | compose 변수치환 프로젝트 .env(${…} 참조) |
| `artifacts/compose/docker-compose.yaml` | `docker-compose.yaml` | compose 파일(빌드 컨텍스트 루트) |

**기동 성공 판정(관측 · 발행 자격)**: health 200 = true · 추론 1회 = true · 근거 `output/single/benchlog/lite_publish/minicpm5-2b-v031_20261016T123814Z/lite_raw_minicpm5-2b-v031.json` · `output/single/benchlog/lite_publish/minicpm5-2b-v031_20261016T123814Z/lite_warm_minicpm5-2b-v031.json`
<!-- /FACT:reproduce -->

<!-- FACT:event_timeline -->
**블랙박스 이벤트 57행**(원장에서 이 셀 label·config 에 맞는 행과 그와 시간이 겹치는 같은 노드의 다른 라벨 창 · 시각순 · 기계 발췌 · 이번 캠페인 0행 · 같은 셀 이름 · 앞선 캠페인 57행(내용 칸에 표기) · 라벨 다름(같은 노드 · 창 밖 이 셀 행과 시간이 겹치는 다른 라벨 선언 창) 55행 — 이 셀의 선언으로 세지 않는다)

> **캠페인 경계** 2026-10-16T12:40:00Z(`camp-26101621-minicpm-lite` · evidence.event_campaign_boundary): campaigns/camp-26101621-minicpm-lite/campaign.yaml declared_utc 2026-10-16T12:40:00Z → 경계 = 앞선 값 2026-10-16T12:40:00Z(이전 행 = 같은 셀 이름 · 앞선 캠페인)

> **원장 관측 범위**(노드별 첫 행 → 마지막 행 · evidence.event_ledger_spans): main = 2026-07-31T03:27:43Z → 2026-10-16T08:04:04Z(`docs/logs/main/events/2026-07.jsonl` · `docs/logs/main/events/2026-08.jsonl` · `docs/logs/main/events/2026-09.jsonl` · `docs/logs/main/events/2026-10.jsonl` · `docs/logs/main/events/thermal.jsonl` · `docs/logs/main/events/watchdog.jsonl`) — main(마지막 행 2026-10-16T08:04:04Z < 측정 끝 2026-10-16T12:38:49Z): 마지막 행 뒤 행 없음(이 노드의 로컬 원장 · 조회 시각까지 관측 · 미회수 아님 — 행은 사건 · 6시간 갱신 때만 쓰인다)

| # | UTC | 노드 | kind | label | 내용 | 출처 |
|---|---|---|---|---|---|---|
| 1 | 2026-10-14T01:59:39Z | main | `budget_declare` | `serve-granite42-8b-v031` | 예산 선언(선언값 — 측정 아님) · floor_mib=76176 · weights_mib=16768 · kv_mib=24064 · overhead_mib=7600 · mem_total_mib=124608 · 라벨 다름: `serve-granite42-8b-v031` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · budget_renew 1회 · 닫힘 replaced(budget_declare) 2026-10-14T02:04:02Z(선언 후 4m23s) · measured_utc 2026-10-16T12:38:15Z 는 이 창 밖(측정 전) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T12:40:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:373 |
| 2 | 2026-10-14T01:59:40Z | main | `budget_honored` | `serve-granite42-8b-v031` | budget_honored · floor_mib=76176 · arm_ceiling_mib=73104 · remaining_s=86399 · 창 시작 2026-10-14T01:59:40Z 후 0m00s · 라벨 다름: `serve-granite42-8b-v031` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T12:40:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:726 |
| 3 | 2026-10-14T01:59:41Z | main | `budget_renew` | `serve-granite42-8b-v031` | budget_renew · floor_mib=76176 · ttl_s=86400 · remaining_before_s=86398 · 창 시작 2026-10-14T01:59:39Z 후 0m02s · 라벨 다름: `serve-granite42-8b-v031` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T12:40:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:375 |
| 4 | 2026-10-14T02:04:02Z | main | `budget_declare` | `serve-main-combined-granite-minicpm` | 예산 선언(선언값 — 측정 아님) · floor_mib=23966 · weights_mib=4800 · kv_mib=35840 · overhead_mib=60002 · mem_total_mib=124608 · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · budget_renew 51회 · 닫힘 미관측(창이 열린 채 원장이 끝난다) · measured_utc 2026-10-16T12:38:15Z 는 이 창 밖(측정 전) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T12:40:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:376 |
| 5 | 2026-10-14T02:04:02Z | main | `serve_start` | — | serve_start · model=minicpm5-2b · tensor_parallel_size=1 · topology=single · session=serve-minicpm5-2b-v031-20261014T020402Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T12:40:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:377 |
| 6 | 2026-10-14T02:04:03Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=86400 · remaining_before_s=86399 · 창 시작 2026-10-14T02:04:02Z 후 0m01s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T12:40:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:378 |
| 7 | 2026-10-14T08:04:03Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=86400 · remaining_before_s=64800 · 창 시작 2026-10-14T02:04:02Z 후 6h00m01s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T12:40:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:379 |
| 8 | 2026-10-14T14:04:03Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=86400 · remaining_before_s=64800 · 창 시작 2026-10-14T02:04:02Z 후 12h00m01s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T12:40:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:380 |
| 9 | 2026-10-14T20:04:03Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=86400 · remaining_before_s=64800 · 창 시작 2026-10-14T02:04:02Z 후 18h00m01s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T12:40:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:384 |
| 10 | 2026-10-15T02:04:03Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=86400 · remaining_before_s=64800 · 창 시작 2026-10-14T02:04:02Z 후 24h00m01s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T12:40:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:385 |
| 11 | 2026-10-15T08:04:03Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=86400 · remaining_before_s=64800 · 창 시작 2026-10-14T02:04:02Z 후 30h00m01s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T12:40:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:386 |
| 12 | 2026-10-15T14:04:03Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=86400 · remaining_before_s=64800 · 창 시작 2026-10-14T02:04:02Z 후 36h00m01s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T12:40:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:387 |
| 13 | 2026-10-15T20:04:04Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=86400 · remaining_before_s=64799 · 창 시작 2026-10-14T02:04:02Z 후 42h00m02s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T12:40:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:391 |
| 14 | 2026-10-16T02:04:04Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=86400 · remaining_before_s=64800 · 창 시작 2026-10-14T02:04:02Z 후 48h00m02s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T12:40:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:392 |
| 15 | 2026-10-16T06:22:00Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=70924 · 창 시작 2026-10-14T02:04:02Z 후 52h17m58s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T12:40:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:393 |
| 16 | 2026-10-16T06:22:00Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7200 · 창 시작 2026-10-14T02:04:02Z 후 52h17m58s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T12:40:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:394 |
| 17 | 2026-10-16T06:22:40Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7160 · 창 시작 2026-10-14T02:04:02Z 후 52h18m38s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T12:40:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:395 |
| 18 | 2026-10-16T06:23:33Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7147 · 창 시작 2026-10-14T02:04:02Z 후 52h19m31s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T12:40:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:396 |
| 19 | 2026-10-16T06:24:29Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7144 · 창 시작 2026-10-14T02:04:02Z 후 52h20m27s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T12:40:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:397 |
| 20 | 2026-10-16T06:47:05Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=5844 · 창 시작 2026-10-14T02:04:02Z 후 52h43m03s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T12:40:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:398 |
| 21 | 2026-10-16T06:54:03Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=6782 · 창 시작 2026-10-14T02:04:02Z 후 52h50m01s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T12:40:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:399 |
| 22 | 2026-10-16T07:00:24Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=6819 · 창 시작 2026-10-14T02:04:02Z 후 52h56m22s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T12:40:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:400 |
| 23 | 2026-10-16T07:06:45Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=6819 · 창 시작 2026-10-14T02:04:02Z 후 53h02m43s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T12:40:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:401 |
| 24 | 2026-10-16T07:09:55Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7010 · 창 시작 2026-10-14T02:04:02Z 후 53h05m53s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T12:40:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:402 |
| 25 | 2026-10-16T07:13:05Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7010 · 창 시작 2026-10-14T02:04:02Z 후 53h09m03s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T12:40:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:403 |
| 26 | 2026-10-16T07:16:16Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7009 · 창 시작 2026-10-14T02:04:02Z 후 53h12m14s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T12:40:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:404 |
| 27 | 2026-10-16T07:18:08Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7088 · 창 시작 2026-10-14T02:04:02Z 후 53h14m06s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T12:40:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:405 |
| 28 | 2026-10-16T07:20:00Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7088 · 창 시작 2026-10-14T02:04:02Z 후 53h15m58s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T12:40:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:406 |
| 29 | 2026-10-16T07:21:53Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7087 · 창 시작 2026-10-14T02:04:02Z 후 53h17m51s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T12:40:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:407 |
| 30 | 2026-10-16T07:23:07Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7126 · 창 시작 2026-10-14T02:04:02Z 후 53h19m05s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T12:40:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:408 |
| 31 | 2026-10-16T07:24:22Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7125 · 창 시작 2026-10-14T02:04:02Z 후 53h20m20s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T12:40:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:409 |
| 32 | 2026-10-16T07:26:49Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7053 · 창 시작 2026-10-14T02:04:02Z 후 53h22m47s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T12:40:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:410 |
| 33 | 2026-10-16T07:29:41Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7028 · 창 시작 2026-10-14T02:04:02Z 후 53h25m39s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T12:40:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:411 |
| 34 | 2026-10-16T07:31:48Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7073 · 창 시작 2026-10-14T02:04:02Z 후 53h27m46s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T12:40:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:412 |
| 35 | 2026-10-16T07:33:57Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7071 · 창 시작 2026-10-14T02:04:02Z 후 53h29m55s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T12:40:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:413 |
| 36 | 2026-10-16T07:34:55Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7142 · 창 시작 2026-10-14T02:04:02Z 후 53h30m53s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T12:40:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:414 |
| 37 | 2026-10-16T07:35:54Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7141 · 창 시작 2026-10-14T02:04:02Z 후 53h31m52s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T12:40:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:415 |
| 38 | 2026-10-16T07:36:52Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7142 · 창 시작 2026-10-14T02:04:02Z 후 53h32m50s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T12:40:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:416 |
| 39 | 2026-10-16T07:37:28Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7164 · 창 시작 2026-10-14T02:04:02Z 후 53h33m26s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T12:40:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:417 |
| 40 | 2026-10-16T07:38:05Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7163 · 창 시작 2026-10-14T02:04:02Z 후 53h34m03s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T12:40:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:418 |
| 41 | 2026-10-16T07:38:41Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7164 · 창 시작 2026-10-14T02:04:02Z 후 53h34m39s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T12:40:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:419 |
| 42 | 2026-10-16T07:39:07Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7174 · 창 시작 2026-10-14T02:04:02Z 후 53h35m05s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T12:40:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:420 |
| 43 | 2026-10-16T07:39:32Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7175 · 창 시작 2026-10-14T02:04:02Z 후 53h35m30s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T12:40:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:421 |
| 44 | 2026-10-16T07:41:46Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7066 · 창 시작 2026-10-14T02:04:02Z 후 53h37m44s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T12:40:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:422 |
| 45 | 2026-10-16T07:43:55Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7071 · 창 시작 2026-10-14T02:04:02Z 후 53h39m53s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T12:40:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:423 |
| 46 | 2026-10-16T07:46:04Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7071 · 창 시작 2026-10-14T02:04:02Z 후 53h42m02s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T12:40:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:424 |
| 47 | 2026-10-16T07:48:12Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7072 · 창 시작 2026-10-14T02:04:02Z 후 53h44m10s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T12:40:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:425 |
| 48 | 2026-10-16T07:49:12Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7140 · 창 시작 2026-10-14T02:04:02Z 후 53h45m10s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T12:40:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:426 |
| 49 | 2026-10-16T07:50:11Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7141 · 창 시작 2026-10-14T02:04:02Z 후 53h46m09s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T12:40:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:427 |
| 50 | 2026-10-16T07:51:10Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7141 · 창 시작 2026-10-14T02:04:02Z 후 53h47m08s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T12:40:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:428 |
| 51 | 2026-10-16T07:51:48Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7162 · 창 시작 2026-10-14T02:04:02Z 후 53h47m46s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T12:40:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:429 |
| 52 | 2026-10-16T07:52:25Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7163 · 창 시작 2026-10-14T02:04:02Z 후 53h48m23s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T12:40:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:430 |
| 53 | 2026-10-16T07:53:03Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7162 · 창 시작 2026-10-14T02:04:02Z 후 53h49m01s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T12:40:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:431 |
| 54 | 2026-10-16T07:53:29Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7174 · 창 시작 2026-10-14T02:04:02Z 후 53h49m27s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T12:40:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:432 |
| 55 | 2026-10-16T07:53:56Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=7200 · remaining_before_s=7173 · 창 시작 2026-10-14T02:04:02Z 후 53h49m54s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T12:40:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:433 |
| 56 | 2026-10-16T07:54:23Z | 미관측 | `measurement` | `measurement (other label's budget window)` | 같은 셀의 다른 측정(측정 노드 축 main) · 시각 = 측정 조립(sweep generated_utc = 인증서 measured_utc · 시작 시각 미기록) · 이 발행의 측정 2026-10-16T12:38:15Z 전 · 발행 기록 camp_26101616_cnn_hint__main__minicpm5_2b_v031(셀 대조 simlog:sweep_index.meta.config_name · verdict PASS) · simlog 사본 generated_utc 2026-10-16T07:54:23Z(같음) / 출력 평면 스윕(이 측정에 묶이지 않음 — 다른 측정이 덮었다) · 원장: 이 셀 라벨의 선언은 없고 다른 라벨의 창 안: serve-main-combined-granite-minicpm 2026-10-14T02:04:02Z~닫힘 미관측(스트림 마지막 행 2026-10-16T08:04:04Z) (docs/logs/main/events/2026-10.jsonl:376) · 직전 예산 행 budget_renew(serve-main-combined-granite-minicpm) 2026-10-16T07:53:56Z(docs/logs/main/events/2026-10.jsonl:433) · 직후 예산 행 budget_renew(serve-main-combined-granite-minicpm) 2026-10-16T08:04:04Z(docs/logs/main/events/2026-10.jsonl:434) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T12:40:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/simlog/26101620_45_28_camp_26101616_cnn_hint__main__minicpm5_2b_v031/sweep_index.json:4 |
| 57 | 2026-10-16T08:04:04Z | main | `budget_renew` | `serve-main-combined-granite-minicpm` | budget_renew · floor_mib=23966 · ttl_s=86400 · remaining_before_s=6592 · 창 시작 2026-10-14T02:04:02Z 후 54h00m02s · 라벨 다름: `serve-main-combined-granite-minicpm` — 같은 노드 · 이 셀 라벨(`minicpm5-2b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(이 셀의 선언으로 세지 않는다) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T12:40:00Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:434 |

**기동 시도**(예산 선언 `budget_declare` 마다 한 시도 · 같은 노드의 이어지는 행과 선언 없는 노드(엔진 로그)의 그 시각 행을 묶었다 · 사살 · 트립 · 거부는 닫힘과 따로 센다 · `measurement` 행은 측정 기록이라 시도로 세지 않는다(위 표에만) — 해석은 저작자 몫)

_이번 캠페인(경계 2026-10-16T12:40:00Z 이후)의 기동 시도 미관측 — 아래는 경계 이전 행뿐이다._

**캠페인 경계 이전 기동 2회**(캠페인 경계 2026-10-16T12:40:00Z 이전 — 라벨 다름(같은 노드 · 창 밖 이 셀 행과 시간이 겹치는 다른 라벨 선언 창) 2회 · 어느 쪽도 이 셀의 시도로 세지 않는다 · 행 전문 = 01 §1.4)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 앞선-1 | main | `serve-granite42-8b-v031`(라벨 다름 — 같은 노드 · 이 셀 라벨 아님) | 2026-10-14T01:59:39Z | 1회 · 첫 2026-10-14T01:59:41Z | 없음 | —(닫힘 행 없음) | 1 |
| 앞선-2 | main | `serve-main-combined-granite-minicpm`(라벨 다름 — 같은 노드 · 이 셀 라벨 아님) | 2026-10-14T02:04:02Z | 51회 · 첫 2026-10-14T02:04:03Z | 없음 | —(닫힘 행 없음) | 1 |
<!-- /FACT:event_timeline -->

<!-- FACT:measurement_env -->
_측정 실행의 env 관측 없음 — 측정 엔진 로그에서 env echo 줄(`<KEY> set by environment to <VALUE>` 등)을 찾지 못했다(evidence.measurement_env_observed · 부재 ≠ 미설정)._
<!-- /FACT:measurement_env -->

> 이 절이 답하는 질문: 수신자가 이 zip 만으로 이 셀을 다시 띄우려면 어떤 순서로 무엇을 하고, 각 단계의 성공을 무엇으로 판정하며, 얼마나 기다려야 하는가?

**(a) 사전 준비** — 가중치는 관리 NAS 의 `<manifest.nas_model_path>/Openbmb/MiniCPM5/MiniCPM5-2B` 를 compose 가 `/app/models` 로 읽기 전용 마운트한다(`${NAS_MODEL_PATH}` — artifacts/compose 의 docker-compose.yaml · 프로젝트 `.env` 형상은 `.env.template`). 스테이징은 없다(PLE 없음 · single). 필수 env 의 자리는 셀 env(`IMAGE_TAG` · `CONFIG_FILE` · `SERVING_PORT` · `COMPOSE_PROJECT_NAME` · `CONTAINER_NAME` — `.env.minicpm5-2b-v031`)와 프로젝트 `.env`(`NAS_MODEL_PATH`)다. 측정 실행의 env 관측은 없다(엔진 로그에 env echo 줄이 없다 — 위 사실 블록) → 기준은 실린 셀 env 이며, 다시 렌더한 env 와 키 · 값을 대조한다. lite 는 데이터셋이 필요 없다(random 합성 입력).

**(b) 빌드** — 메인 노드에서 `Dockerfile.source-build` 선택자 · build-arg `VLLM_REF=v0.31.0` · `TORCH_CUDA_ARCH=12.1a` · `BUILD_JOBS=8` 등(위 재구성 명령 — docker history build-arg 에서 재구성 · 실행 원문 아님). 소요는 계보 표 안에는 없다(층 창 38분 19초는 캐시 재사용 층을 포함해 소요가 아니다) — 같은 이미지의 빌드를 이 계보 밖 testlog_26101412 §2 P4 가 "메인 빌드 2차 rc=0 · 01:08:32→01:48:59Z"(같은 문서 §1: 40분)로 기록한다.

**(c) 기동과 성공 판정** — 명령은 `single_serve_up.sh minicpm5-2b-v031`(사용법에서 파생 · 실행 원문 아님). 이 서빙은 2026-10-14 에 섰고 lite 측정(10-16 12:38Z)은 그 상주 세션 위에서 했다(측정 전 재기동 없음 — 엔진 로그 머리는 10-14 기동분이다). 엔진 로그(lite_engine_head)의 시각: 기동 배너 10-14 02:04:11(L38) → 가중치 로드 31.98 초(L61) · `Model loading took 4.75 GiB memory and 33.656631 seconds`(L62) → KV 35.0 GiB 예약(L74) → FlashInfer autotune 02:05:12~02:05:29(L80 · L82) → `Graph capturing finished in 33 secs, took 0.23 GiB`(L86) · `init engine (profile, create kv cache, warmup model) took 81.41 s`(L88) → API route 등록 02:06:23(L102). 표의 기동 소요는 미관측이고, 엔진 로그의 두 시각은 위와 같다. 성공 판정 = health 200 + 추론 1회(발행 자격 근거 = lite raw · lite warm JSON). 패치 발화 서명은 해당 없음(패치 0).
기동 시도: 위 원장 표에서 이 셀의 기동 시도는 0 이다 — 첫 사유는 캠페인 경계(2026-10-16T12:40:00Z)다: 이 서빙은 2026-10-14T02:04:02Z 의 합산 선언 `serve-main-combined-granite-minicpm` 창(갱신 51회 · 사살 · 트립 없음)과 같은 시각의 `serve_start`(session `serve-minicpm5-2b-v031-20261014T020402Z`)로 섰고, 이 행들은 경계 이전(같은 셀 이름 · 앞선 캠페인)이라 라벨과 무관하게 세지 않는다. 선언 창의 라벨이 이 셀 라벨과 다른 것은 그다음 사유다. 원장 `watchdog.jsonl` 의 02:05:11Z `watchdog_highrate_hold`(라벨 없음 · legacy_rule_would_trip=true)는 위 표에 실리지 않았다 — 사살이 아니라 보류 기록이며, 시각은 이 모델의 KV 예약(L74 · 02:05:10) 직후다.

**(d) 호스트 메모리 안전 예산 선언** — 합산 선언 `floor_mib=23966` · `weights_mib=4800` · `kv_mib=35840` · `overhead_mib=60002` · `mem_total_mib=124608`(원장 행 4 · **선언값**). overhead 는 plan_26101410 §4 D6 처방(두 번째 기동의 overhead = 첫 모델 기동 후 실측 used + 컨테이너 오버헤드)이라 granite 몫을 품는다. 엔진 실측은 로드 메모리 4.75 GiB(L62)이고, KV 35.0 GiB(L74)는 클램프 선언값대로 예약했다는 엔진 보고다(측정 아님). 단독으로 띄우면 overhead 를 다시 잰다. main 원장은 로컬 원장이라 마지막 행(08:04:04Z) 뒤로 조회 시각까지 행이 없음이 관측된다(위 사실 블록) — 갱신 루프가 6시간 주기라 측정 시각(12:38Z)에 갱신 행이 없는 것과 어긋나지 않는다.

**(e) 측정 명령** — 위 bench 단계의 호출 줄은 재구성(`lite_bench.sh minicpm5-2b-v031 --topology single --backend openai-chat --publish-report`)이고, testlog 는 `lite_bench.sh minicpm5-2b-v031 --backend openai-chat --publish-report` → rc 0 을 적는다(testlog_26101621_36_59 §1). 측정 창은 12:38:15Z → 12:38:49Z 근사(두 끝 시계가 다르다 — 위 표). 도구가 정한 인자는 00 머리 lite 측정 블록의 도구 원문(`lite_bench.sh@803064d23d51`)과 02 §2.5 의 원문 발췌에 있다.

**(f) 엔진이 스스로 고른 런타임 경로** — FlashAttention version 2(L54) · `enable_prefix_caching=True` · `enable_chunked_prefill=True` · `cudagraph_mode` FULL_AND_PIECEWISE · capture sizes `[1, 2, 4, 8, 16]`(L48 엔진 config 줄) · LBNHC KV 레이아웃(L65). 통신 백엔드는 해당 없음(TP=1). L74 의 "This does not respect the gpu_memory_utilization config" 는 KV 클램프를 지정했을 때의 정상 안내이지 벽이 아니다. L78 의 `Failed to import the DeepSelect extension (vllm._deepselect_C)` WARNING 은 기동 · 측정이 성립한 채 남은 경고다 — 이 모델 경로에서 그 확장이 쓰이는지는 확인하지 않았다.

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

체크포인트 `openbmb/MiniCPM5-2B` · revision `f97400052a43d642bbc6e9975e2397e3ae6a6b52`(00 §0.4 — 체크포인트 git HEAD) · 획득 모드 = 관리 NAS(메인 manifest `model_source: managed` — plan_26101614_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리 §3) · 위치 `<manifest.nas_model_path>/Openbmb/MiniCPM5/MiniCPM5-2B` · 노드 1개(single). 크기: 엔진 로그 체크포인트 크기 4.69 GiB(lite_engine_head L55 · 관측) · plan_26101410 §모델 4종의 bf16 가중치 ≈4.4GiB 는 config 판독 산식 · 로드 뒤 메모리 4.75 GiB(L62)는 크기가 아니다. 계보에 다운로드 명령 기록이 없다 — 아래는 **추론 — 실행 검증 ✗** 이며 사람의 획득 승인 뒤에만 친다:

```sh
huggingface-cli download openbmb/MiniCPM5-2B --revision f97400052a43d642bbc6e9975e2397e3ae6a6b52 \
  --local-dir <manifest.nas_model_path>/Openbmb/MiniCPM5/MiniCPM5-2B
```

### 스테이징 생성

해당 없음 — 모델 config 에 PLE 키가 없고(00 §0.9 후보 표 출처) 런타임 패치도 없다.

### 측정 입력

해당 없음 — lite 는 `vllm bench serve` 의 random 합성 입력을 쓰고 데이터셋 knob 를 받지 않는다(plan_26101614 §2 U14). 측정 토크나이저는 서빙 컨테이너 안의 모델 경로를 그대로 읽는다(lite_bench 원문 `--tokenizer '$MODEL_PATH'`).

### 네트워크가 필요한 단계

- 가중치 다운로드(위 절 — 사람 승인).
- NGC 베이스 pull(`nvcr.io/nvidia/pytorch:26.07-py3`) · 빌드 중 vLLM 소스 git clone(`VLLM_REPO`) · pip(requirements · ray).

lite 측정 자체는 서빙 컨테이너 안에서 `docker exec` 로 돌아 외부 접속이 필요 없다(lite_bench 원문의 `docker exec "$CTR" … vllm bench serve` 경로 — 이 측정이 그 가지로 돌았는지는 lite raw `serve_plane: docker` 로 본다). 오프라인 대체: 위 이미지 · 가중치를 미리 받아 둔다.
