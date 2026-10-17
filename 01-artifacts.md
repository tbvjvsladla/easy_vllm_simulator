# 01 · 산출물 — 무엇이 실제로 쓰였고, 어떻게 다시 띄우는가

## 1.1 적용 판정 표
> 기계가 적용 증거(빌드 원장 · 원장이 없으면 라벨된 재구성) × 파일 × 선언을 대사한 결과다. `artifacts/` 에는 **적용된 것만** 실렸다 — 빌드 때 스스로 skip 된 패치와 쓰이지 않은 평면의 파일은 표에만 남고 싣지 않는다.

<!-- FACT:slots -->
| 슬롯 | 실림 | 파일 | 신호 | 적용 증거 | 사유 |
|---|---|---|---|---|---|
| `build_patch_post` | 안 실림 | — | 1-signal | build-ledger:docker-image(main-local):sha256:339737df6571ce00993e731ef064a8db66085beaa650e4aa63bdb24f414b2034:/opt/easy-vllm/build_ledger.json | 적용 판정 없음 — 원장 관측(기계 파생) |
| `build_patch_pre` | 안 실림 | — | 1-signal | build-ledger:docker-image(main-local):sha256:339737df6571ce00993e731ef064a8db66085beaa650e4aa63bdb24f414b2034:/opt/easy-vllm/build_ledger.json | 적용 판정 없음 — 원장 관측(기계 파생) |
| `build_recipe` | 실림 | `artifacts/build_recipe/Dockerfile.source-build` · `artifacts/build_recipe/requirements.txt` | 3-signal | build-ledger:build-ledger.dockerfile | (a) NGC `pytorch:26.07-py3` 위에서 vLLM 을 소스에서 짓는 레시피다 — build-arg `VLLM_REPO` · `VLLM_REF=v0.31.0` 로 업스트림 릴리스를 받고, `use_existing_torch.py` 로 베이스 torch 를 그대로 쓰며, 인라인 `strip-hoist` 블록(torch 의 `register_opaque_type` 가 `hoist` 를 받지 않을 때만 소스에서 그 kwarg 를 지우는 자기적응)을 품는다. 이 셀용 이식 · 자체 설계 패치는 없다(stock · plan_26101410 §4 D2 "빌드 패치 0"). (b) 빌드 트랙이 source-build 인 이유는 plan_26101410 §3(torch ≥2.11 ABI 벽)이다 — 이 레… |
| `compose` | 실림 | `artifacts/compose/.env.template` · `artifacts/compose/docker-compose.yaml` | 2-signal | file:output/single/docker-compose.yaml | (a) single 서빙 경로(빌드 컨텍스트 · 볼륨 `./configs:/app/configs` · env_file)다. (b) 같은 노드에 두 모델을 띄우려면 모델 env 마다 `COMPOSE_PROJECT_NAME` 을 따로 둬야 둘째 up 이 첫째를 교체하지 않는다(plan_26101410 §3 실측 · §4 D6) — 이 셀의 env 가 그 값을 가진다. (c) 측정 당시 개정(커밋 12ef50a 바이트)이 실렸다(1.1). (d) 노드에 모델을 하나만 띄우면 프로젝트명 분리는 필요 없다. |
| `fork_pin` | 안 실림 | — | 1-signal | none:.claude/policies/arch_variant_ledger.json | 셀 env 에 VARIANT 줄 없음 = stock(workflow.md §변종 좌표의 거처)(기계 파생) |
| `runtime_patch` | 안 실림 | — | 1-signal | none:output/single/configs/granite42-8b-v031_patch.py | output/single/configs/granite42-8b-v031_patch.py 없음 — arm_patch.sh 는 no-op(런타임 패치 불해당)(기계 파생) |
| `triplet` | 실림 | `artifacts/triplet/.env.granite42-8b-v031` · `artifacts/triplet/granite42-8b-v031.sh` · `artifacts/triplet/granite42-8b-v031.yaml` | 3-signal | file:output/single/configs/granite42-8b-v031.yaml | (a) `vllm-recipe-explorer` 가 생성한 서빙 설정 · 러너 · 환경이다 — 러너는 `--enable-auto-tool-choice` · `--tool-call-parser qwen3_coder` · `--reasoning-parser granite_thinking_parser` 를 붙이고, env 는 `IMAGE_TAG` 와 포트 8001 · 프로젝트명을 든다. 세 파일의 바이트는 앞선(10-16) 측정분과 같다(이 재측정은 서빙을 다시 띄우지 않았다). (b) 이 셀 형상 자체다(값의 지위는 1.3). (c) 엔진 로그 `non-default args` 줄이 yaml 의 네 노브와 러너 인자를 되읊는다(엔진 로그 `lite_engine_head_granite42-8b-v031.log`… |

**레시피 경고 · 실린 리비전**(기계 관측 — 수신자 빌드가 여기서 갈라질 수 있다)

- ⚠ `build_recipe` `output/single/Dockerfile.source-build` — 실린 바이트(worktree)가 측정 이미지 docker history 의 RUN·COPY 와 전수 일치하지 않는다(16/17) · 이미지 빌드 뒤 바뀐 레시피일 수 있다 — 마지막 커밋이 이미지 Created 뒤(이미지 Created 2026-10-14T01:45:19Z · 마지막 커밋 2026-10-14T03:07:26Z)
- `build_recipe` `output/single/Dockerfile.source-build` 실린 리비전: 리비전 미특정 · 선택 방법 `worktree(어느 개정도 이미지 history 와 더 맞지 않는다 · 작업트리 16/17 — recipe_vs_image 경고 유지)` · docker history 대조 16/17 · 실린 바이트 `worktree` · 쓰인 바이트임을 관측으로 확인 아니오
- `build_recipe` `output/single/requirements.txt` 실린 리비전: 리비전 미특정 · 선택 방법 `worktree(빌드 원장 sha256 일치 — 쓰인 바이트 관측 확인 = 예)` · docker history 대조 없음 · 실린 바이트 `worktree` · 쓰인 바이트임을 관측으로 확인 예
- `compose` `output/single/docker-compose.yaml` 실린 리비전: 커밋 `12ef50a89b86` · 선택 방법 `worktree(mtime 2026-10-14T01:06:26Z ≤ 측정 2026-10-16T22:23:36Z(sweep_index.generated_utc) · 바이트 = 측정 전 마지막 커밋 12ef50a89b86)` · docker history 대조 없음 · 실린 바이트 `worktree` · 근거 `mtime≤measured` · 쓰인 바이트임을 관측으로 확인 예

**싣지 않은 파일**(슬롯별 사유 — 빌드 때 skip 된 패치는 아래 적용 집합 표에 있다)

- `build_patch_post` known-non-patch 1건: `.gitkeep`
- `build_patch_pre` known-non-patch 1건: `.gitkeep`
- `compose` `arm_patch.sh` — `not-armed(output/single/configs/granite42-8b-v031_patch.py 없음 — arm_patch.sh 는 no-op)`

**파일별 검증 · 관련성**(`generated-unverified` = 실행 검증되지 않은 생성물 — 파일 첫 줄 경고 · PAYLOAD 파일 기록과 같은 표시 · 관련성 = ① 패치 선언 대상 × 모델 config ② 엔진 로그 발화 서명 — 둘 다 관측일 때만 required/inactive-inferred)

| 슬롯 | 파일 | 검증 | 검증 근거 | 관련성 | 관련성 근거 |
|---|---|---|---|---|---|
| `build_recipe` | `artifacts/build_recipe/Dockerfile.source-build` | `generated-unverified` · renderer | 실린 Dockerfile.source-build(worktree)이 측정 이미지 docker history RUN·COPY 와 16/17 만 일치 — 이 이미지를 지은 바이트임을 관측으로 확인하지 못했다(_select_dockerfile · worktree(어느 개정도 이미지 history 와 더 맞지 않는다 · 작업트리 16/17 — recipe_vs_image 경고 유지)) | `unknown` | 이 셀은 이 경로로 실행되지 않았다(generated-unverified · 생성 renderer) — 실행 기록이 없어 필요 여부를 판정하지 않는다(참고 형상) |
| `build_recipe` | `artifacts/build_recipe/requirements.txt` | `verified` · cell-run | 쓰인 레시피(build-ledger.dockerfile) — 개정 선택 slots.build_recipe.evidence.selected_revisions | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/.env.template` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/docker-compose.yaml` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/.env.granite42-8b-v031` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/granite42-8b-v031.sh` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/granite42-8b-v031.yaml` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
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
| attestation | absent | output/single/benchlog/attestation_granite42-8b-v031.json(부재) |
| local-image | observed | sha256:339737df6571ce00993e731ef064a8db66085beaa650e4aa63bdb24f414b2034 |
<!-- /FACT:applied_set -->

## 1.2 슬롯별 적용 사유
> 이 절이 답하는 질문: 실린 산출물 하나하나는 무엇을 고치며, 왜 이 셀에 필요했고, 이 셀에서 실제로 발화했으며, 언제 불필요해지는가?

### build_recipe — `Dockerfile.source-build`
(a) NGC `pytorch:26.07-py3` 위에서 vLLM 을 소스에서 짓는 레시피다 — build-arg `VLLM_REPO` · `VLLM_REF=v0.31.0` 로 업스트림 릴리스를 받고, `use_existing_torch.py` 로 베이스 torch 를 그대로 쓰며, 인라인 `strip-hoist` 블록(torch 의 `register_opaque_type` 가 `hoist` 를 받지 않을 때만 소스에서 그 kwarg 를 지우는 자기적응)을 품는다. 이 셀용 이식 · 자체 설계 패치는 없다(stock · plan_26101410 §4 D2 "빌드 패치 0").
(b) 빌드 트랙이 source-build 인 이유는 plan_26101410 §3(torch ≥2.11 ABI 벽)이다 — 이 레시피가 없으면 이미지가 없다. 이 셀 서사의 벽(W1~W6)은 모두 측정 경로라 이 파일과 무관하다.
(c) 1.1 판정대로 실린 바이트는 측정 이미지 history 와 16/17 만 맞아 `generated-unverified` 다 — 이 이미지를 지은 바이트라는 관측이 없다. 인라인 `strip-hoist` 는 1.1 적용 집합 표에서 `skipped`(빌드 원장 관측)다 — 이 셀의 torch 에서는 발화하지 않았다.
(d) `strip-hoist` 의 종료 경로는 빌드를 멈추지 않고 skip 하며 상태 파일에 사유를 남긴다:

> [원문] Dockerfile.source-build §L115-117
> else:
>     print("[patch] strip-hoist skipped (register_opaque_type accepts hoist / opaque_object 부재)")
>     status = "skipped\tregister_opaque_type accepts hoist / opaque_object 부재"

반면 `use_existing_torch.py` 를 루트 · `tools/` 어디에서도 못 찾으면 빌드를 멈춘다(상류가 파일을 옮기면 여기서 드러난다):

> [원문] Dockerfile.source-build §L82-84
> RUN if [ -f tools/use_existing_torch.py ]; then python3 tools/use_existing_torch.py; \
>     elif [ -f use_existing_torch.py ]; then python3 use_existing_torch.py; \
>     else echo "FAIL: use_existing_torch.py 를 찾지 못했다(루트·tools/ 모두 부재) — 상류 이동을 확인하라" >&2; exit 1; fi

불필요해지는 조건: 업스트림이 aarch64 · 이 CUDA 계열에 맞는 wheel ABI 를 내서 트랙 판정이 바뀔 때(판정 owner = `upstream-version-watch`).

### build_recipe — `requirements.txt`
(a) v0.31.0 wheel 의 `Requires-Dist` 를 옮기고 베이스 제공분(torch 계열)을 뺀 런타임 의존 목록이다(파일 머리 주석) — 빌드 원장 sha256 과 일치해 쓰인 바이트임이 관측됐다(1.1 `verified`). (b) 소스빌드 이미지의 Python 의존을 채운다 — 이 모델 전용 항목은 없다. (c) 패치가 아니라 발화 개념이 없다. (d) vLLM 버전을 바꾸면 다시 생성한다(같은 파일을 다른 버전에 쓰지 않는다).

### compose — `docker-compose.yaml` · `.env.template`
(a) single 서빙 경로(빌드 컨텍스트 · 볼륨 `./configs:/app/configs` · env_file)다. (b) 같은 노드에 두 모델을 띄우려면 모델 env 마다 `COMPOSE_PROJECT_NAME` 을 따로 둬야 둘째 up 이 첫째를 교체하지 않는다(plan_26101410 §3 실측 · §4 D6) — 이 셀의 env 가 그 값을 가진다. (c) 측정 당시 개정(커밋 12ef50a 바이트)이 실렸다(1.1). (d) 노드에 모델을 하나만 띄우면 프로젝트명 분리는 필요 없다.

### triplet — `granite42-8b-v031.yaml` · `.sh` · `.env.granite42-8b-v031`
(a) `vllm-recipe-explorer` 가 생성한 서빙 설정 · 러너 · 환경이다 — 러너는 `--enable-auto-tool-choice` · `--tool-call-parser qwen3_coder` · `--reasoning-parser granite_thinking_parser` 를 붙이고, env 는 `IMAGE_TAG` 와 포트 8001 · 프로젝트명을 든다. 세 파일의 바이트는 앞선(10-16) 측정분과 같다(이 재측정은 서빙을 다시 띄우지 않았다). (b) 이 셀 형상 자체다(값의 지위는 1.3). (c) 엔진 로그 `non-default args` 줄이 yaml 의 네 노브와 러너 인자를 되읊는다(엔진 로그 `lite_engine_head_granite42-8b-v031.log` L42 — 실제로 먹었다). (d) 다른 HW · 다른 동시성 요구면 1.3 의 arch-scaled 값부터 다시 정한다.

### 싣지 않은 슬롯
- `build_patch_pre` · `build_patch_post` — 원장에 적용 판정 없음 · 슬롯에 `.gitkeep` 뿐(stock 빌드 · plan_26101410 §3 "싱글 빌드 슬롯 비어 있음").
- `fork_pin` — 셀 env 에 `VARIANT` 줄이 없어 stock 이다.
- `runtime_patch` — `granite42-8b-v031_patch.py` 가 없어 `arm_patch.sh` 가 no-op 이다. 부재는 "이 셀에 해당 없음" 이지 "검증 통과" 가 아니다.

## 1.3 값의 지위표
> 서빙 설정의 노브 **전부**와 기계가 모은 후보 지위다(lockset 출처 · yaml 주석 · 엔진 기본값). 최종 지위는 아래 `kind: value-status` 블록이 정한다 — 표의 노브마다 블록이 정확히 하나 있어야 린터를 통과한다.

<!-- FACT:value_status -->
서빙 노브 4개 — 노브마다 `kind: value-status` 블록이 **정확히 1개** 있어야 한다(노브·값은 이 표의 글자 그대로 · 값 칸의 `\|` 는 `|` 로 적는다).

| 노브 | 값 | 후보 지위 | 후보 근거 | yaml 주석 | 출처 |
|---|---|---|---|---|---|
| `gpu-memory-utilization` | 0.39 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `max-model-len` | 32768 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `max-num-seqs` | 8 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `kv-cache-memory-bytes` | 25232932864 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
<!-- /FACT:value_status -->

> 이 절이 답하는 질문: 각 서빙 노브의 값은 이 셀에서 어떤 지위인가 — 조정됐나, 승계됐나, 음성대조로 일부러 둔 것인가, 엔진 기본값인가, 필요조건인가?

```hint-event
id: V1
kind: value-status
노브: gpu-memory-utilization
값: 0.39
지위: inherited
근거: 앞선 계획이 64GB Spark 시뮬 천장을 물리 호스트 기준으로 환산해 둔 값(천장÷호스트 총량 약 0.39 · 두 인스턴스 공존용)이고 이 계보에서 바꿔 잰 기록이 없다 · KV 절대 클램프가 있으면 엔진은 이 값을 KV 산정에 쓰지 않는다고 로그에 적는다
출처: [plan_26101410_vllm031_bump_14B이하_4종_양노드_서빙_lite §4, output/single/benchlog/sweep_granite42-8b-v031/lite_engine_head_granite42-8b-v031.log]
```

```hint-event
id: V2
kind: value-status
노브: max-model-len
값: 32768
지위: inherited
근거: 사용자가 4종 상주 서빙의 요구로 32768 고정을 지시했다 · 이 계보에서 다른 값을 시도한 기록이 없다
출처: [plan_26101410_vllm031_bump_14B이하_4종_양노드_서빙_lite §1]
```

```hint-event
id: V3
kind: value-status
노브: max-num-seqs
값: 8
지위: inherited
근거: 계획 D5 가 정한 산식 꼴 min(8 · KV_fit@8192) 의 값과 같지만 실행에서 explorer 가 이 산식으로 계산했다는 기록은 계보에 없다(계보 표 밖 devlog_26101412 · testlog_26101412 는 KV 와 gmu 의 hand 출처만 적고 max-num-seqs 의 산정 기록은 없다) · 이 셀에서 바꿔 잰 기록 없음 · 스윕 레벨 16 은 이 값 때문에 자동 제외됐다
출처: [plan_26101410_vllm031_bump_14B이하_4종_양노드_서빙_lite §4, output/single/benchlog/sweep_granite42-8b-v031/sweep_index.json]
```

```hint-event
id: V4
kind: value-status
노브: kv-cache-memory-bytes
값: 25232932864
지위: inherited
근거: 앞선 계획은 절대 클램프라는 방식만 정했고 값은 앞선 캠페인 실행에서 손으로 산정됐다(천장 − 가중치 − 오버헤드의 max_safe 를 사용자 64/64 의도로 min(required, max_safe) 대신 택함 · lockset kv_source=hand — 계보 표 밖 devlog_26101412 가 기록한다) · 엔진 예약 23.5 GiB · 154000 토큰 · 32768 토큰 요청 기준 최대 동시성 4.70x(엔진 로그 · 클램프의 되읽기) · 이 셀에서 조정 기록 없음
출처: [plan_26101410_vllm031_bump_14B이하_4종_양노드_서빙_lite §4, output/single/benchlog/sweep_granite42-8b-v031/lite_engine_head_granite42-8b-v031.log]
```

네 노브 모두 이 셀에서 조정된 적이 없다 — 이 셀(CNN 재측정)은 앞선 캠페인이 띄워 상주 중인 서빙에 측정만 다시 걸었다. 복사하면 가장 위험한 값은 `kv-cache-memory-bytes`(V4)와 `gpu-memory-utilization`(V1)이다: 둘 다 "128GB급 통합메모리 노드를 64GB 둘로 나눠 두 모델을 공존시킨다" 는 이 노드 사정에서 나왔고, 엔진 로그대로 KV 클램프가 있으면 gmu 는 KV 크기를 정하지 않는다. 03 §3.1 의 lite KV 23.5 GiB 도 이 클램프 선언을 엔진이 되읽은 값이고(154000 토큰은 그 예약을 엔진이 토큰으로 환산한 것) 측정으로 정해진 크기가 아니다. `max-num-seqs`(V3)는 스윕 상한도 정했다(레벨 16 제외). 네 환경에서는 0.5 ① 절차로 다시 도출한다.

## 1.4 재현 절차
> 셀 실행 기록에서 기계가 파생한 명령 순서와 단계별 소요다. 소요가 `미관측` 인 단계는 시각 기록이 없다는 뜻이다(0 이 아니다). 그 아래는 블랙박스 이벤트 원장에서 이 셀의 행을 기계가 뽑아 기동 시도로 묶은 것이고, 마지막은 측정한 실행의 엔진 로그가 echo 한 env 다(측정 당시 값의 정본).

<!-- FACT:reproduce -->
| 순서 | 단계 | 성공 판정 | 소요 | 관측 구간(UTC) | 소요 출처 | 명령 출처 |
|---|---|---|---|---|---|---|
| 1 | build | 빌드 OK · 이미지 실재 | 층 창 38분 19초(docker history 첫 층 → 끝 층 CreatedAt · 캐시 재사용 층 포함 — 빌드 소요 아님) | 2026-10-14T01:07:00Z → 2026-10-14T01:45:19Z (layer-window) | docker history --no-trunc --format '{{json .}}' sha256:339737df6571ce00993e731ef064a8db66085beaa650e4aa63bdb24f414b2034 · 우리 Dockerfile 층 24개의 CreatedAt 첫 층 → 끝 층(캐시 재사용 층은 이전 빌드 시각 — 빌드 소요 아님) | reconstructed(build-ledger build_args + compose build) |
| 2 | serve | health 200 + 추론 1회(스모크 판정) | ≤ 67시간 44분 48초(상한)(끝 = 최종 스윕 첫 측정 · 그 전 요청 455건 — 소요 출처 칸) | 2026-10-14T01:59:39Z → 2026-10-16T21:44:27Z (upper) | docs/logs/main/events/2026-10.jsonl budget_declare(label serve-granite42-8b-v031) → 첫 측정 시각(output/single/benchlog/sweep_granite42-8b-v031/lite_raw_granite42-8b-v031.json measured_utc(호스트 date -u · lite_bench.sh 가 cold 레그 직전에 찍는 측정 시작)) · ⚠ 최종(재실행) 스윕 기준 — 같은 serve 세션 안 최종 스윕 첫 측정 전 completions 요청 455건 관측(상한 시각 2026-10-14T02:03:35Z~2026-10-16T14:43:10Z · 스모크 추론 1회 초과 = 앞선 벤치 실행(끊긴 스윕 등)이 있었을 수 있다 · 출처 output/single/benchlog/sweep_granite42-8b-v031/lite_engine_head_granite42-8b-v031.log · output/single/benchlog/sweep_granite42-8b-v031/lite_engine_granite42-8b-v031.log 의 POST 접근 줄 × 다음 vLLM 시각 줄(컨테이너 시계 UTC 가정)) | derived(스모크 사용법) |
| 3 | bench | 리포트 발행(+PASS 면 인증서) | 39분 9초(최종 스윕 기준 · 그 전 요청 455건 — 소요 출처 칸) | 2026-10-16T21:44:27Z → 2026-10-16T22:23:36Z (exact) | output/single/benchlog/sweep_granite42-8b-v031/lite_raw_granite42-8b-v031.json measured_utc(호스트 date -u · lite_bench.sh 가 cold 레그 직전에 찍는 측정 시작) → sweep_index.generated_utc(호스트 UTC · 두 끝 같은 시계) · ⚠ 최종(재실행) 스윕 기준 — 같은 serve 세션 안 최종 스윕 첫 측정 전 completions 요청 455건 관측(상한 시각 2026-10-14T02:03:35Z~2026-10-16T14:43:10Z · 스모크 추론 1회 초과 = 앞선 벤치 실행(끊긴 스윕 등)이 있었을 수 있다 · 출처 output/single/benchlog/sweep_granite42-8b-v031/lite_engine_head_granite42-8b-v031.log · output/single/benchlog/sweep_granite42-8b-v031/lite_engine_granite42-8b-v031.log 의 POST 접근 줄 × 다음 vLLM 시각 줄(컨테이너 시계 UTC 가정)) | reconstructed(bench json) |

**빌드 층 시각**(관측 · 소요가 아니라 층 창 — 캐시 재사용 층은 이전 빌드의 시각을 가진다)

- `build` 이미지 층 CreatedAt(docker history): 2026-10-14T01:07:00Z → 2026-10-14T01:45:19Z · 층 24개 · 출처 `docker history --no-trunc --format '{{json .}}' sha256:339737df6571ce00993e731ef064a8db66085beaa650e4aa63bdb24f414b2034` — 우리 Dockerfile 층의 CreatedAt 범위 — 캐시 재사용 층은 이전 빌드 시각을 가진다(소요 아님)

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
# 동등 경로(compose): docker compose -f output/single/docker-compose.yaml --env-file output/single/envs/.env.granite42-8b-v031 build
```

**2. serve**

```sh
bash .claude/skills/upstream-version-watch/scripts/single_serve_up.sh granite42-8b-v031
```

**3. bench**

```sh
# bench JSON 필드로 복원: backend · model_id→--model · tokenizer_id→--tokenizer · num_prompts · max_concurrency · request_rate · burstiness · 요청당 입출력 길이 = total_*_tokens ÷ completed(나누어떨어질 때만 — 평균이다 · 요청별 길이는 JSON 에 없다) · 나누어떨어지면 `--dataset-name random` 으로 추정했다(측정 도구가 그 꼴로 부른다 — 아래 도구 원문).
# JSON 에 없는 인자(--endpoint · --base-url · --ignore-eos · --num-warmups · --temperature · --seed · --random-range-ratio · --trust-remote-code)는 측정 도구가 정했다 — level_* = run_bench.sh · lite_cold/lite_warm = lite_bench.sh(.claude/skills/adversarial-benchmark/scripts/) · 셀 스윕 순서는 sweep_bench.sh.
# output/single/benchlog/sweep_granite42-8b-v031/lite_cold_granite42-8b-v031.json (date 20261016-214446) · ⚠ --random-input-len/--random-output-len = total ÷ completed 평균(chat 템플릿 포함 실측 토큰 · 측정 도구 원문 인자 미관측 — lite_bench.sh 원문 인자와 다를 수 있다)
vllm bench serve --backend openai-chat --model granite-4.2-8b --tokenizer /app/models/IBM/Granite-4.2/granite-4.2-8b --dataset-name random --random-input-len 527 --random-output-len 128 --num-prompts 1 --max-concurrency 1 --request-rate inf
# output/single/benchlog/sweep_granite42-8b-v031/lite_warm_granite42-8b-v031.json (date 20261016-214536) · ⚠ --random-input-len/--random-output-len = total ÷ completed 평균(chat 템플릿 포함 실측 토큰 · 측정 도구 원문 인자 미관측 — lite_bench.sh 원문 인자와 다를 수 있다)
vllm bench serve --backend openai-chat --model granite-4.2-8b --tokenizer /app/models/IBM/Granite-4.2/granite-4.2-8b --dataset-name random --random-input-len 527 --random-output-len 128 --num-prompts 3 --max-concurrency 1 --request-rate inf
```

**슬롯 → 빌드 컨텍스트 매핑**(빌드 컨텍스트 = `output/<토폴로지>/` · 원천 = 실린 Dockerfile 의 COPY · compose 의 env_file · volumes · command — `—` 는 빌드 컨텍스트 입력이 아니다)

| zip 경로 | 빌드 컨텍스트 자리 | 근거 |
|---|---|---|
| `artifacts/triplet/.env.granite42-8b-v031` | `envs/.env.granite42-8b-v031` | compose env_file envs/.env.${CONFIG_FILE}(셀 env) |
| `artifacts/triplet/granite42-8b-v031.sh` | `configs/granite42-8b-v031.sh` | compose volumes ./configs:/app/configs |
| `artifacts/triplet/granite42-8b-v031.yaml` | `configs/granite42-8b-v031.yaml` | compose volumes ./configs:/app/configs |
| `artifacts/build_recipe/Dockerfile.source-build` | `Dockerfile.source-build` | compose build.dockerfile Dockerfile.source-build(리터럴) · build.context . |
| `artifacts/build_recipe/requirements.txt` | `requirements.txt` | Dockerfile.source-build COPY requirements.txt |
| `artifacts/compose/.env.template` | `.env` | compose 변수치환 프로젝트 .env(${…} 참조) |
| `artifacts/compose/docker-compose.yaml` | `docker-compose.yaml` | compose 파일(빌드 컨텍스트 루트) |

**기동 성공 판정(관측 · 발행 자격)**: health 200 = true · 추론 1회 = true · 근거 `output/single/benchlog/sweep_granite42-8b-v031/level_01/post_health_granite42-8b-v031.json` · `output/single/benchlog/sweep_granite42-8b-v031/level_01/guidellm_granite42-8b-v031.json`
<!-- /FACT:reproduce -->

<!-- FACT:event_timeline -->
**블랙박스 이벤트 5행**(원장에서 이 셀 label·config 에 맞는 행만 · 시각순 · 기계 발췌 · 이번 캠페인 0행 · 같은 셀 이름 · 앞선 캠페인 5행(내용 칸에 표기))

> **캠페인 경계** 2026-10-16T14:03:30Z(`camp-26101623-baseline` · evidence.event_campaign_boundary): campaigns/camp-26101623-baseline/campaign.yaml declared_utc 2026-10-16T14:03:30Z → 경계 = 앞선 값 2026-10-16T14:03:30Z(이전 행 = 같은 셀 이름 · 앞선 캠페인)

> **원장 관측 범위**(노드별 첫 행 → 마지막 행 · evidence.event_ledger_spans): main = 2026-07-31T03:27:43Z → 2026-10-16T16:42:14Z(`docs/logs/main/events/2026-07.jsonl` · `docs/logs/main/events/2026-08.jsonl` · `docs/logs/main/events/2026-09.jsonl` · `docs/logs/main/events/2026-10.jsonl` · `docs/logs/main/events/thermal.jsonl` · `docs/logs/main/events/watchdog.jsonl`) — main(마지막 행 2026-10-16T16:42:14Z < 측정 끝 2026-10-16T22:23:36Z): 마지막 행 뒤 행 없음(이 노드의 로컬 원장 · 조회 시각까지 관측 · 미회수 아님 — 행은 사건 · 6시간 갱신 때만 쓰인다)

| # | UTC | 노드 | kind | label | 내용 | 출처 |
|---|---|---|---|---|---|---|
| 1 | 2026-10-14T01:59:39Z | main | `budget_declare` | `serve-granite42-8b-v031` | 예산 선언(선언값 — 측정 아님) · floor_mib=76176 · weights_mib=16768 · kv_mib=24064 · overhead_mib=7600 · mem_total_mib=124608 · 기동 창 1/1: budget_renew 1회 · 닫힘 replaced(budget_declare) 2026-10-14T02:04:02Z(선언 후 4m23s) · measured_utc 2026-10-16T22:23:36Z 는 이 창 밖(다른 기동 · 측정 전) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:373 |
| 2 | 2026-10-14T01:59:39Z | main | `serve_start` | — | serve_start · model=granite-4.2-8b · tensor_parallel_size=1 · topology=single · session=serve-granite42-8b-v031-20261014T015939Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:374 |
| 3 | 2026-10-14T01:59:40Z | main | `budget_honored` | `serve-granite42-8b-v031` | budget_honored · floor_mib=76176 · arm_ceiling_mib=73104 · remaining_s=86399 · 창 시작 2026-10-14T01:59:40Z 후 0m00s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/watchdog.jsonl:726 |
| 4 | 2026-10-14T01:59:41Z | main | `budget_renew` | `serve-granite42-8b-v031` | budget_renew · floor_mib=76176 · ttl_s=86400 · remaining_before_s=86398 · 창 시작 2026-10-14T01:59:39Z 후 0m02s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/main/events/2026-10.jsonl:375 |
| 5 | 2026-10-16T07:25:36Z | 미관측 | `measurement` | `measurement (other label's budget window)` | 같은 셀의 다른 측정(측정 노드 축 main) · 시각 = 측정 조립(sweep generated_utc = 인증서 measured_utc · 시작 시각 미기록) · 이 발행의 측정 2026-10-16T22:23:36Z 전 · 발행 기록 camp_26101616_cnn_hint__main__granite42_8b_v031(셀 대조 simlog:sweep_index.meta.config_name · verdict PASS) · simlog 사본 generated_utc 2026-10-16T07:25:36Z(같음) · 원장: 이 셀 라벨의 선언은 없고 다른 라벨의 창 안: serve-main-combined-granite-minicpm 2026-10-14T02:04:02Z~닫힘 미관측(스트림 마지막 행 2026-10-16T15:02:37Z) (docs/logs/main/events/2026-10.jsonl:376) · 직전 예산 행 budget_renew(serve-main-combined-granite-minicpm) 2026-10-16T07:24:22Z(docs/logs/main/events/2026-10.jsonl:409) · 직후 예산 행 budget_renew(serve-main-combined-granite-minicpm) 2026-10-16T07:26:49Z(docs/logs/main/events/2026-10.jsonl:410) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-16T14:03:30Z 이전 — 이 셀의 시도로 세지 않는다) | docs/simlog/26101620_camp_26101616_cnn_hint__main__granite42_8b_v031/sweep_index.json:4 |

**기동 시도**(예산 선언 `budget_declare` 마다 한 시도 · 같은 노드의 이어지는 행과 선언 없는 노드(엔진 로그)의 그 시각 행을 묶었다 · 사살 · 트립 · 거부는 닫힘과 따로 센다 · `measurement` 행은 측정 기록이라 시도로 세지 않는다(위 표에만) — 해석은 저작자 몫)

_이번 캠페인(경계 2026-10-16T14:03:30Z 이후)의 기동 시도 미관측 — 아래는 같은 셀 이름의 앞선 캠페인 행뿐이다._

**같은 셀 이름 · 앞선 캠페인의 기동 1회**(캠페인 경계 2026-10-16T14:03:30Z 이전 — 라벨이 같아 원장에서 함께 묶였을 뿐 이 셀의 시도로 세지 않는다 · 행 전문 = 01 §1.4)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 앞선-1 | main | `serve-granite42-8b-v031` | 2026-10-14T01:59:39Z | 1회 · 첫 2026-10-14T01:59:41Z | 없음 | —(닫힘 행 없음) | 2 |
<!-- /FACT:event_timeline -->

<!-- FACT:measurement_env -->
_측정 실행의 env 관측 없음 — 측정 엔진 로그에서 env echo 줄(`<KEY> set by environment to <VALUE>` 등)을 찾지 못했다(evidence.measurement_env_observed · 부재 ≠ 미설정)._
<!-- /FACT:measurement_env -->

> 이 절이 답하는 질문: 수신자가 이 zip 만으로 이 셀을 다시 띄우려면 어떤 순서로 무엇을 하고, 각 단계의 성공을 무엇으로 판정하며, 얼마나 기다려야 하는가?

(a) **사전 준비** — 가중치는 manifest 의 관리 NAS 경로(`<manifest.nas_model_path>/IBM/Granite-4.2/granite-4.2-8b`)를 컨테이너 `/app/models/IBM/Granite-4.2/granite-4.2-8b` 로 마운트한다(yaml `model:`). 엔진은 이 체크포인트를 CIFS 위에서 읽었고 크기를 16.38 GiB 로 보고했다(엔진 로그 L55). PLE 등 스테이징은 없다(노드 로컬 디스크 조건 해당 없음). 필수 env 는 `artifacts/triplet/.env.granite42-8b-v031`(IMAGE_TAG · SERVING_PORT · COMPOSE_PROJECT_NAME · TIKTOKEN_ENABLED)에 있다. 측정 실행의 env echo 는 엔진 로그에서 찾지 못했다(위 사실 블록 — 부재 ≠ 미설정). 셀 입력(캠페인 셀 config)은 이번에 손 복사 없이 `campaign_init --cell-materialize granite42-8b-v031`(파생 11키 · 선언 7키 `--set`)로 만들었다(testlog_26101707 §3) — 그 파일은 캠페인 인스턴스(비추적 · 휘발)라 이 zip 에 없다.
(b) **빌드** — 메인 노드에서 `Dockerfile.source-build` 선택자와 위 표의 build-arg 로 짓는다. 위 표의 38분 19초는 층 CreatedAt 창이지 빌드 소요가 아니다(캐시 층 포함) — 실제 빌드 소요는 이 계보 문서에 없다.
(c) **기동과 판정** — `single_serve_up.sh granite42-8b-v031`. 원장 기준 기동 시도는 **이번 캠페인 0회**다(서빙을 다시 띄우지 않았다) · 같은 셀 이름의 앞선 캠페인 기동 1회(2026-10-14T01:59:39Z 선언 · 갱신 1회 · 사살 · 트립 · 거부 없음)가 이 측정이 붙은 서빙이다. 그 기동의 엔진 로그 타임라인: 가중치 로드 105.79 s(L64) → `Model loading took 16.42 GiB memory`(L65) → KV 23.5 GiB 예약 · 154,000 토큰(L77-78) → 그래프 캡처 31 s · 0.29 GiB(L89) → `init engine … took 82.24 s`(L91) → API 라우트 등록 02:03:15(L97 이후). 성공 판정은 health 200 + chat 추론 1회이고 이 측정에서는 벤치 뒤 health 200 이 기록됐다(`post_health_granite42-8b-v031.json`). 위 serve 행이 적은 최종 스윕 전 completions 요청 455건은 이 서빙이 받은 요청 전체가 아니라 **하한**이다 — 두 캡처 조각(엔진 로그 머리 600줄 · 10-14 01:59 → 10-15 06:04 와 꼬리 800줄 · 10-16 07:20 → 21:45)에서 센 값이고, 그 사이 약 25시간(앞선 캠페인 측정 10-16 06:45Z~07:25Z 의 대부분 포함)은 캡처되지 않았다. 꼬리 조각에는 1차 재측정 스윕(14:04Z~)과 Q1 조사 전송이 들어 있을 것으로 보지만 건수를 대사하지 않았다(추론). 패치가 없어 패치 발화 서명은 없다. 정상인데 무서워 보이는 줄: L30 `WARNING: The NVIDIA Driver was not detected.`(엔진이 뜨기 전 컨테이너 진입 배너 — 뒤이어 엔진이 CUDA 로 정상 기동했다), L81 `Failed to import the DeepSelect extension`(이 dense 모델 경로에서 쓰이지 않는 확장이라고 본다 — 추론), L97 `generation_config.json` 이 기본 샘플링을 `temperature 1.0 · top_p 0.95` 로 덮는다는 경고(ignore_eos 라 출력 길이는 고정).
(d) **호스트 메모리 예산 선언** — 원장 `budget_declare`: floor_mib=76176 · weights_mib=16768 · kv_mib=24064 · overhead_mib=7600 · mem_total_mib=124608(**선언값**). 엔진 실측은 가중치 16.42 GiB 이고 KV 23.5 GiB 는 클램프 선언을 엔진이 되읽은 예약 크기다(나란히). 그 선언 창은 4분 23초 뒤(2026-10-14T02:04:02Z) 원장의 label `serve-main-combined-granite-minicpm` 선언으로 교체됐다(위 이벤트 표 1행의 닫힘 · 5행의 창) — granite + MiniCPM5 **합산** 선언이며 plan_26101410 §4 D6 의 처방대로다. 위 이벤트 표의 granite 기동 창에는 사살 · 트립 · 거부 행이 없고, 이번 측정 시각(10-16 21:44Z~22:23Z)은 원장 마지막 행(16:42:14Z) 뒤라 원장에 측정 구간 행이 없다(행은 사건 · 갱신 때만 쓰인다).
(e) **측정 명령** — 위 bench 단계의 두 명령은 lite 레그(cold · warm)를 bench JSON 에서 재구성한 것이고 실행 원문이 아니다. GuideLLM 동시성 스윕의 실제 인자는 03 §3.4(측정 도구 원문 + GuideLLM 결과 JSON 의 config)로 간다. 런마다 `prefix_cache_<cfg>.json`(실행 차분)과 실행 범위 `engine_<cfg>.log` 가 함께 남는다(W5) — 첫 런 적중이 0 이 아니면 그 스윕은 다시 잰다(W6).
(f) **엔진이 고른 런타임 경로** — 어텐션 `FLASH_ATTN`(FlashAttention 2 · L53-54) · cudagraph `FULL_AND_PIECEWISE` · prefix caching 과 chunked prefill 은 켜짐(L48 엔진 config) · KV 레이아웃 LBNHC(L68). 단일 노드 world_size=1 이라 NCCL 은 통신에 쓰이지 않는다(L49). KV 클램프 때문에 메모리 프로파일링을 건너뛰었다고 엔진이 적는다(L77). 엔진 로그의 `Prefix cache hit rate` 는 최근 요청 창 통계다 — 이번 레벨 1 실행 범위 로그에는 20.8% 가 찍히지만 같은 런의 `/metrics` 실행 차분은 0.0 이다(M2) — 로그 값을 이 실행의 적중으로 읽지 않는다.

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
- 체크포인트: HF `ibm-granite/granite-4.2-8b` · revision `41a8a2d41c54ef4a71741b3e62604f4caaec9295`(00 §0.2 — 체크포인트 git HEAD 관측) · base_model `ibm-granite/granite-4.1-8b-base`.
- 획득 모드: 관리 NAS(managed) — 메인 노드만 필요하다(single · TP=1). 크기: 엔진 보고 16.38 GiB(엔진 로그 L55).
- 명령(추론 — 실행 검증 ✗ · 이 계보에 다운로드 기록이 없다): `huggingface-cli download ibm-granite/granite-4.2-8b --revision 41a8a2d41c54ef4a71741b3e62604f4caaec9295 --local-dir <manifest.nas_model_path>/IBM/Granite-4.2/granite-4.2-8b` — 모델 획득은 사람의 승인 게이트다(무인 다운로드 ✗).

### PLE · 기타 스테이징
해당 없음 — 이 모델 config 에 PLE 키가 없고(00 §0.9 후보 표의 관측) 런타임 패치도 없다(01 §1.1). 체크포인트를 NAS 에서 바로 읽는다.

### 벤치 데이터셋(CNN/DailyMail)
- 원본: `abisee/cnn_dailymail` config `3.0.0` `test` parquet 1파일 · revision `96df5e686bee6baa90b8bee7c28b81fa3fa6223d` · sha256 `04e322d2634a96dba76bf9a6294fbbe48e0b36abeae43f13d86ba2c3bebffe4e`(03 데이터셋 출처 표). 놓는 자리는 `<NAS 데이터셋 루트>/abisee__cnn_dailymail/<revision>/` 이다(plan_26101614 §4 D1). 원본은 노드당 1회 사람 승인으로 받는다(plan_26101614 §2 U3).
- 프리셋: `cnn_preset.yaml` 은 비추적 사용자 프리셋이라 이 zip 에 없다 — 저장소의 `config/guide-llm/README.md` 절차로 다시 쓴다(plan_26101614 §2 U6 · U16: `article` 원문만 · input 1024 / output 256 · 나머지 knob 기본).
- 전처리 · 행 선택: 벤치 직전 자동이다(GuideLLM `preprocess dataset --strategy concatenate` · 고정 이미지 오프라인 컨테이너 · plan_26101614 §2 U2 · U3). 행 시작점은 스윕 시작 UTC 를 salt 로 도구가 정한다(이 측정 offset 2727 · 런별 사이드카 `dataset_granite42-8b-v031.json`) — 사람이 할 일은 없다. 다만 측정 직전에 같은 서버로 데이터셋 행을 직접 보내는 조사를 하지 않는다(W6).

### 네트워크가 필요한 단계
- 베이스 이미지 pull `nvcr.io/nvidia/pytorch:26.07-py3` · vLLM 소스 clone(`v0.31.0`) · pip 의존(`requirements.txt`) — 빌드 때. 오프라인 대체: 미리 받아 둔다.
- GuideLLM 이미지 `ghcr.io/vllm-project/guidellm:v0.7.3`(digest `sha256:e3ad2371bfa8e42f2c3d1251b62d0d9c9706c27ae2143c8c048eb5fc6aebb558` — 스윕 색인 meta `bench_tool_image_digest` 관측) — 벤치 때. 벤치 컨테이너 자체는 `HF_HUB_OFFLINE=1` 로 돈다(run_bench.sh@3fa6eac00a18 L444) — 토크나이저는 로컬 사본이어야 한다.
- 가중치 · 데이터셋 다운로드 — 위 두 절. 사람 승인 뒤에만.
