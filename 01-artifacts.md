# 01 · 산출물 — 무엇이 실제로 쓰였고, 어떻게 다시 띄우는가

## 1.1 적용 판정 표
> 기계가 적용 증거(빌드 원장 · 원장이 없으면 라벨된 재구성) × 파일 × 선언을 대사한 결과다. `artifacts/` 에는 **적용된 것만** 실렸다 — 빌드 때 스스로 skip 된 패치와 쓰이지 않은 평면의 파일은 표에만 남고 싣지 않는다.

<!-- FACT:slots -->
| 슬롯 | 실림 | 파일 | 신호 | 적용 증거 | 사유 |
|---|---|---|---|---|---|
| `build_patch_post` | 안 실림 | — | 1-signal | build-ledger:attestation:output/multi/benchlog/attestation_q38fn31-nv4-1m-tp2.json#main | 적용 판정 없음 — 원장 관측(기계 파생) |
| `build_patch_pre` | 안 실림 | — | 1-signal | build-ledger:attestation:output/multi/benchlog/attestation_q38fn31-nv4-1m-tp2.json#main | 적용 판정 없음 — 원장 관측(기계 파생) |
| `build_recipe` | 실림 | `artifacts/build_recipe/Dockerfile.source-build` · `artifacts/build_recipe/requirements.txt` | 3-signal | build-ledger:build-ledger.dockerfile | 결과 출처: 선택자는 이미지 안 빌드 원장(`build-ledger.dockerfile`)이고, 실린 두 파일은 single-node 개정 `12ef50a89b86:output/single/` 의 바이트다(1.1 실린 리비전). 이 이미지는 single-node 체크아웃에서 지어졌고 그때 입력은 '`689610f` + 미커밋 렌더' 였다(계보 밖 testlog_26101412 유효맥락 · §1) — 12ef50a 는 그 빌드 뒤의 커밋이므로 '이 이미지를 지은 개정' 이 아니라 원장과 대조해 가장 가까운 이력 개정이다. `requirements.txt` 는 빌드 원장 sha256 과 일치해 **`verified`** 이고, `Dockerfile.source-build` 는 측정 이미지 docker hist… |
| `compose` | 실림 | `artifacts/compose/.env.cluster.template` · `artifacts/compose/.env.interconnect.template` · `artifacts/compose/.env.template` · `artifacts/compose/docker-compose.yaml` · `artifacts/compose/serve_runner.sh` · `artifacts/compose/sub_recipe.json` | 2-signal | file:output/multi/docker-compose.yaml | (a) 멀티 기동 경로다 — compose 가 master(Ray head + vllm serve)와 slave(Ray worker)를 같은 이미지 · 같은 `serve_runner.sh` 로 띄우고 `NODE_ROLE` 로 가른다. compose 는 `VLLM_PLE_MMAP` 을 셀 env 가 주지 않으면 `0` 으로 넣는다(L118 · L147) — 이 셀의 엔진 로그가 `Unknown vLLM environment variable detected: VLLM_PLE_MMAP` 를 찍는 것은 그 기본값 주입 때문이고(lite_engine_head L63), 런타임 패치가 없으므로 그 값을 읽는 코드가 없다. (b) W7(서브에 compose 부재)이 이 슬롯의 배달 문제였다. (c) 측정 당시 개정이… |
| `fork_pin` | 안 실림 | — | 1-signal | none:.claude/policies/arch_variant_ledger.json | 셀 env 에 VARIANT 줄 없음 = stock(workflow.md §변종 좌표의 거처)(기계 파생) |
| `runtime_patch` | 안 실림 | — | 1-signal | none:output/multi/configs/q38fn31-nv4-1m-tp2_patch.py | output/multi/configs/q38fn31-nv4-1m-tp2_patch.py 없음 — arm_patch.sh 는 no-op(런타임 패치 불해당)(기계 파생) |
| `triplet` | 실림 | `artifacts/triplet/.env.q38fn31-nv4-1m-tp2` · `artifacts/triplet/q38fn31-nv4-1m-tp2.sh` · `artifacts/triplet/q38fn31-nv4-1m-tp2.yaml` | 3-signal | file:output/multi/configs/q38fn31-nv4-1m-tp2.yaml | (a) 서빙 설정 · 러너 · 셀 env 다. 러너는 `arm_patch.sh` 가 있으면 source 하고(이 셀에서는 no-op) `vllm serve --config <yaml>` 을 부른다. yaml 의 hf-overrides 가 이 셀의 유일한 필요조건 노브다(W8 · 01 V12). (b) W8 은 yaml 의 hf-overrides 로 넘었다. (c) 측정이 이 셀 config 로 돌았다(스윕 색인 · `non-default args` lite_engine_head L62). (d) 0.31.0 의 YaRN 최대 길이 의미가 다시 바뀌면 hf-overrides 의 max_position_embeddings 줄이 불필요하거나 다르게 필요해진다 — 기동 인자 검증 줄(W8 서명)로 판정한다. |

**레시피 경고 · 실린 리비전**(기계 관측 — 수신자 빌드가 여기서 갈라질 수 있다)

- ⚠ `build_recipe` `output/single/Dockerfile.source-build` — 실린 바이트(git:12ef50a89b86cb30d4c5b87c32506c5563ff4655)가 측정 이미지 docker history 의 RUN·COPY 와 전수 일치하지 않는다(16/17)(이미지 Created 2026-10-14T01:45:19Z · 실린 개정 커밋 2026-10-14T03:07:26Z · 이 체크아웃 통로 `output/multi/Dockerfile.source-build` 의 파일이 아니다)
- `build_recipe` `output/single/Dockerfile.source-build`(이 체크아웃 통로 `output/multi/Dockerfile.source-build` 대신) 실린 리비전: 커밋 `12ef50a89b86` · 선택 방법 `git log --all -- output/*/Dockerfile.source-build · 12ef50a89b86:output/single/Dockerfile.source-build 가 docker history RUN·COPY 와 가장 잘 맞는 이력 개정(16/17 · 전수 일치 개정 없음 · 불일치 레시피에만: RUN echo "[guard] source-build key (26.07-py3 x vLLM 0.31.0): validated patch set ((ngc_base x vllm_version) keyed) — pr · 작업트리 15/18) — 이 체크아웃 통로(output/multi/Dockerfile.source-build) 밖 · 커밋 2026-10-14T03:07:26Z > 이미지 Created 2026-10-14T01:45:19Z(빌드 뒤 커밋 · 위 불일치 단계만큼 다르다) — 이 개정의 바이트를 싣는다(git cat-file)` · docker history 대조 16/17 · 실린 바이트 `git:12ef50a89b86cb30d4c5b87c32506c5563ff4655` · 쓰인 바이트임을 관측으로 확인 아니오
- `build_recipe` `output/single/requirements.txt`(이 체크아웃 통로 `output/multi/requirements.txt` 대신) 실린 리비전: 커밋 `12ef50a89b86` · 선택 방법 `git log --all -- output/*/requirements.txt · 12ef50a89b86:output/single/requirements.txt · 빌드 원장 sha256 일치(작업트리·이미지 Created 이전 개정은 다르다) — 이 체크아웃 통로(output/multi/requirements.txt) 밖 · 커밋 2026-10-14T03:07:26Z > 이미지 Created 2026-10-14T01:45:19Z(빌드 뒤 커밋된 같은 바이트)` · docker history 대조 없음 · 실린 바이트 `git:12ef50a89b86cb30d4c5b87c32506c5563ff4655` · 쓰인 바이트임을 관측으로 확인 예
- `compose` `.claude/skills/upstream-version-watch/assets/configs/serve_runner.sh` 실린 리비전: 커밋 `50a5ce486738` · 선택 방법 `worktree(mtime 2026-09-06T21:57:53Z ≤ 측정 2026-10-25T02:27:13Z(sweep_index.generated_utc) · 바이트 = 측정 전 마지막 커밋 50a5ce486738)` · docker history 대조 없음 · 실린 바이트 `worktree` · 근거 `mtime≤measured` · 쓰인 바이트임을 관측으로 확인 예
- `compose` `output/multi/docker-compose.yaml` 실린 리비전: 커밋 `d781f26e4ebc` · 선택 방법 `worktree(git log -1 --before=2026-10-25T02:27:13Z -- output/multi/docker-compose.yaml → d781f26e4ebc · 작업트리 mtime 2026-10-25T03:14:49Z > 측정(sweep_index.generated_utc)이지만 바이트가 그 개정과 같다 · 측정 뒤 커밋 0 · 작업트리 = HEAD)` · docker history 대조 없음 · 실린 바이트 `worktree` · 근거 `git-rev-before-measurement` · 쓰인 바이트임을 관측으로 확인 아니오

**싣지 않은 파일**(슬롯별 사유 — 빌드 때 skip 된 패치는 아래 적용 집합 표에 있다)

- `build_patch_post` known-non-patch 1건: `.gitkeep`
- `build_patch_pre` known-non-patch 1건: `.gitkeep`
- `compose` `arm_patch.sh` — `not-armed(output/multi/configs/q38fn31-nv4-1m-tp2_patch.py 없음(CONFIG_FILE) · slave default_patch.py(CONFIG_FILE) 없음 — arm_patch.sh 는 no-op · arm_patch.sh 측정 당시 개정(d781f26e4ebc) `_cfg=` 키 체인 RUNTIME_PATCH_STEM > CONFIG_FILE > 'default')`

**파일별 검증 · 관련성**(`generated-unverified` = 실행 검증되지 않은 생성물 — 파일 첫 줄 경고 · PAYLOAD 파일 기록과 같은 표시 · 관련성 = ① 패치 선언 대상 × 모델 config ② 엔진 로그 발화 서명 — 둘 다 관측일 때만 required/inactive-inferred)

| 슬롯 | 파일 | 검증 | 검증 근거 | 관련성 | 관련성 근거 |
|---|---|---|---|---|---|
| `build_recipe` | `artifacts/build_recipe/Dockerfile.source-build` | `generated-unverified` · renderer | 실린 Dockerfile.source-build(git:12ef50a89b86cb30d4c5b87c32506c5563ff4655)이 측정 이미지 docker history RUN·COPY 와 16/17 만 일치 — 이 이미지를 지은 바이트임을 관측으로 확인하지 못했다(_select_dockerfile · git log --all -- output/*/Dockerfile.source-build · 12ef50a89b86:output/single/Dockerfile.source-build 가 docker history RUN·COPY 와 가장 잘 맞는 이력 개정(16/17 · 전수 일치 개정 없음 · 불일치 레시피에만: RUN echo "[guard] source-build key (26.07-py3 x vLLM 0.31.0): validated patch set ((ngc_base x vllm_version) keyed) — pr · 작업트리 15/18) — 이 체크아웃 통로(output/multi/Dockerfile.source-build) 밖 · 커밋 2026-10-14T03:07:26Z > 이미지 Created 2026-10-14T01:45:19Z(빌드 뒤 커밋 · 위 불일치 단계만큼 다르다) — 이 개정의 바이트를 싣는다(git cat-file)) | `unknown` | 이 셀은 이 경로로 실행되지 않았다(generated-unverified · 생성 renderer) — 실행 기록이 없어 필요 여부를 판정하지 않는다(참고 형상) |
| `build_recipe` | `artifacts/build_recipe/requirements.txt` | `verified` · cell-run | 쓰인 레시피(build-ledger.dockerfile) — 개정 선택 slots.build_recipe.evidence.selected_revisions | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/.env.cluster.template` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/.env.interconnect.template` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/.env.template` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/docker-compose.yaml` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/serve_runner.sh` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/sub_recipe.json` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/.env.q38fn31-nv4-1m-tp2` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/q38fn31-nv4-1m-tp2.sh` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/q38fn31-nv4-1m-tp2.yaml` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
<!-- /FACT:slots -->

<!-- FACT:applied_set -->
**적용 집합** — 상태 `observed` · 출처 `attestation:output/multi/benchlog/attestation_q38fn31-nv4-1m-tp2.json#main`

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
| attestation | observed | output/multi/benchlog/attestation_q38fn31-nv4-1m-tp2.json |

> **attestation 범위** — config `q38fn31-nv4-1m-tp2` · phase serve · 작성 2026-10-25T02:16:28Z(output/multi/benchlog/attestation_q38fn31-nv4-1m-tp2.json 파일 mtime(파일시스템 관측 — 문서에 시각 칸이 없다 · 기록 필드 아님)) · 묶은 근거 cell-name · 파일 `output/multi/benchlog/attestation_q38fn31-nv4-1m-tp2.json` · 측정 창 대비 작성 2026-10-25T02:16:28Z < 측정 시작 2026-10-25T02:16:49Z — 측정 전(빌드·스모크)의 관측: **same config(q38fn31-nv4-1m-tp2) · serve — 원장 순서로 판별한 같은 기동의 관측(스모크 로그 미보존 · 약한 판별)(판별: 원장 순서로 판별(스모크 로그 미보존 — docs/simlog/**/*serve*.log 에 이 attestation 을 적은 스모크 로그 없음): 원장 docs/logs/main/events/2026-10.jsonl:643 budget_declare(smoke-q38fn31-nv4-1m-tp2 · 2026-10-25T01:52:06Z) ≤ attestation 작성 2026-10-25T02:16:28Z · serve_proof 작성 2026-10-25T02:16:28Z < 측정 시작 2026-10-25T02:16:49Z · 그 선언이 측정 끝 2026-10-25T02:27:13Z 까지 재선언 · 창 닫힘 없이 이어짐(main 원장 기록 범위 안) · API 서버 기동 2026-10-25T02:15:29Z(output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/lite_engine_q38fn31-nv4-1m-tp2.log:742 · 컨테이너 시계 = UTC 가정) ∈ [선언, attestation 작성] · 측정 엔진 로그 APIServer pid 789 하나)**
<!-- /FACT:applied_set -->

## 1.2 슬롯별 적용 사유
> 이 절이 답하는 질문: 실린 산출물 하나하나는 무엇을 고치며, 왜 이 셀에 필요했고, 이 셀에서 실제로 발화했으며, 언제 불필요해지는가?

### `build_recipe` — `Dockerfile.source-build` · `requirements.txt`

결과 출처: 선택자는 이미지 안 빌드 원장(`build-ledger.dockerfile`)이고, 실린 두 파일은 single-node 개정 `12ef50a89b86:output/single/` 의 바이트다(1.1 실린 리비전). 이 이미지는 single-node 체크아웃에서 지어졌고 그때 입력은 '`689610f` + 미커밋 렌더' 였다(계보 밖 testlog_26101412 유효맥락 · §1) — 12ef50a 는 그 빌드 뒤의 커밋이므로 '이 이미지를 지은 개정' 이 아니라 원장과 대조해 가장 가까운 이력 개정이다. `requirements.txt` 는 빌드 원장 sha256 과 일치해 **`verified`** 이고, `Dockerfile.source-build` 는 측정 이미지 docker history 의 RUN · COPY 와 16/17 만 맞아 **`generated-unverified`** 다 — 어긋난 단계는 L40 `RUN echo "[guard] source-build key (26.07-py3 x vLLM 0.31.0) …"` 하나로, 이 개정이 이미지 Created(2026-10-14T01:45:19Z) 뒤인 03:07:26Z 에 커밋돼 그 가드 줄만큼 다르다(1.1 경고). (a) NGC `nvcr.io/nvidia/pytorch:26.07-py3`(L37) 위에 `VLLM_REPO`/`VLLM_REF` 소스를 클론해(L76) `use_existing_torch.py` 를 `tools/` 와 루트 중 있는 쪽으로 골라 NGC torch 를 그대로 쓰며(L82 ~ L84 — v0.31.0 이 그 파일을 `tools/` 로 옮겨 이 이미지의 1차 빌드가 루트 경로에서 죽고 이 자기적응 단계로 고친 기록이 계보 밖 testlog_26101412 §2 P4 에 있다) 빌드한다. requirements.txt 는 설치 목록이 아니라 pip constraint(`/etc/pip/constraint.txt`)로 COPY 된다(L69 ~ L71) — 머리 주석은 v0.31.0 wheel 메타에서 뽑았다고 적는다. (b) 이 셀의 벽을 넘은 수단은 이 레시피에 없다(빌드 패치 0) — 필요한 이유는 재현 입력(A 층)이라는 것뿐이다. (c) 이미지 안 원장이 적은 인라인 단계는 `strip-hoist` 하나이고 결과는 `skipped` 다(1.1 적용 집합) — torch 의 `register_opaque_type` 가 hoist 인자를 받으면 소스를 고치지 않는 자기적응 단계다. (d) 그 단계는 조건이 맞지 않으면 빌드를 멈추지 않고 skip 상태만 원장에 쓴다:

> [원문] Dockerfile.source-build §L115-117
> else:
>     print("[patch] strip-hoist skipped (register_opaque_type accepts hoist / opaque_object 부재)")
>     status = "skipped\tregister_opaque_type accepts hoist / opaque_object 부재"

⚠ `COPY build_patches_src/`(L165) · `COPY build_patches/`(L203)는 빌드 컨텍스트에 그 디렉터리가 있어야 통과한다 — 이 이미지는 두 디렉터리에 `.gitkeep` 만 든 컨텍스트로 빌드됐고(원장 `context_inventory`) zip 에는 그 디렉터리가 없다(실린 Dockerfile L133 ~ L134 주석도 `.gitkeep` 유지를 적는다 · 독립 검증이 디렉터리 부재 시 cache key not found 로 멈추는 것을 재현했다). 또 이 레시피의 컨텍스트는 `output/single/` 이다 — 멀티 스모크 · compose build 는 `output/multi/` 의 Dockerfile 을 지으므로 이 이미지와 동등한 경로가 아니다(1.4 2단계 명령의 주석).

### `compose` — `docker-compose.yaml` · `serve_runner.sh` · env 형상 3종 · `sub_recipe.json`

(a) 멀티 기동 경로다 — compose 가 master(Ray head + vllm serve)와 slave(Ray worker)를 같은 이미지 · 같은 `serve_runner.sh` 로 띄우고 `NODE_ROLE` 로 가른다. compose 는 `VLLM_PLE_MMAP` 을 셀 env 가 주지 않으면 `0` 으로 넣는다(L118 · L147) — 이 셀의 엔진 로그가 `Unknown vLLM environment variable detected: VLLM_PLE_MMAP` 를 찍는 것은 그 기본값 주입 때문이고(lite_engine_head L63), 런타임 패치가 없으므로 그 값을 읽는 코드가 없다. (b) W7(서브에 compose 부재)이 이 슬롯의 배달 문제였다. (c) 측정 당시 개정이 실렸다(1.1 실린 리비전 — compose `d781f26e4ebc` · serve_runner `50a5ce486738`). (d) 패치가 아니라 기동 경로라 폐기 조건이 없다.

### `triplet` — `q38fn31-nv4-1m-tp2.{yaml,sh}` · `.env.q38fn31-nv4-1m-tp2`

(a) 서빙 설정 · 러너 · 셀 env 다. 러너는 `arm_patch.sh` 가 있으면 source 하고(이 셀에서는 no-op) `vllm serve --config <yaml>` 을 부른다. yaml 의 hf-overrides 가 이 셀의 유일한 필요조건 노브다(W8 · 01 V12). (b) W8 은 yaml 의 hf-overrides 로 넘었다. (c) 측정이 이 셀 config 로 돌았다(스윕 색인 · `non-default args` lite_engine_head L62). (d) 0.31.0 의 YaRN 최대 길이 의미가 다시 바뀌면 hf-overrides 의 max_position_embeddings 줄이 불필요하거나 다르게 필요해진다 — 기동 인자 검증 줄(W8 서명)로 판정한다.

### 싣지 않은 슬롯

- `build_patch_pre` · `build_patch_post` — 적용된 패치가 없다(원장 `context_inventory` = `.gitkeep` 뿐 · 작업트리의 패치 파일들은 빌드 컨텍스트에 들지 않았다 · 1.1 '싣지 않은 파일'). 0.29 계보의 패치 60(W1)이나 62(R2)는 이 이미지에 없다.
- `runtime_patch` — `output/multi/configs/q38fn31-nv4-1m-tp2_patch.py` 가 없어 `arm_patch.sh` 는 no-op 이다. PLE mmap 플러그인(`qwen_ple_mmap_plugin`)은 다른 셀(`q38fn31-nv4-mmap-1m-tp2` 등)의 런타임 패치이고 이 셀에는 무장되지 않았다 — 측정 기동에서 PLE 는 `weight_device=cuda:0, pinned=False` 로 장치에 상주했다(lite_engine L631 · L649).
- `fork_pin` — 셀 env 에 `VARIANT` 줄이 없다 = stock(`vllm-project/vllm` v0.31.0).

## 1.3 값의 지위표
> 서빙 설정의 노브 **전부**와 기계가 모은 후보 지위다(lockset 출처 · yaml 주석 · 엔진 기본값). 최종 지위는 아래 `kind: value-status` 블록이 정한다 — 표의 노브마다 블록이 정확히 하나 있어야 린터를 통과한다.

<!-- FACT:value_status -->
서빙 노브 12개 — 노브마다 `kind: value-status` 블록이 **정확히 1개** 있어야 한다(노브·값은 이 표의 글자 그대로 · 값 칸의 `\|` 는 `|` 로 적는다).

| 노브 | 값 | 후보 지위 | 후보 근거 | yaml 주석 | 출처 |
|---|---|---|---|---|---|
| `tensor-parallel-size` | 2 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `distributed-executor-backend` | ray | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `gpu-memory-utilization` | 0.80 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `max-model-len` | 1048576 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `kv-cache-dtype` | fp8_e4m3 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `kv-cache-memory-bytes` | 15032385536 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `enforce-eager` | true | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `async-scheduling` | false | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `max-num-seqs` | 1 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `max-num-batched-tokens` | 2048 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `engram-config` | {"cpu_offload": false} | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `hf-overrides` | {"text_config":{"max_position_embeddings":1048576,"rope_parameters":{"mrope_interleaved":true,"mrope_section":[11,11,10],"partial_rotary_factor":0.25,"rope_theta":10000000,"rope_type":"yarn","factor":4,"original_max_position_embeddings":262144}}} | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
<!-- /FACT:value_status -->

> 이 절이 답하는 질문: 각 서빙 노브의 값은 이 셀에서 어떤 지위인가 — 조정됐나, 승계됐나, 음성대조로 일부러 둔 것인가, 엔진 기본값인가, 필요조건인가?

```hint-event
id: V1
kind: value-status
노브: tensor-parallel-size
값: 2
지위: inherited
근거: 싱글 TP=1 은 산술 사전판정으로 기동 없이 배제됐다(R1 · 관측된 실패 아님) · 멀티 캠페인 plan 이 TP=2 로 정했고 다른 값 시도 0
출처: [plan_26102412_4모델_멀티캠페인_Solar_Qwen_TP2_최대context §셀 (TP=2 이므로 노드 병렬 아님 · 순차), testlog_26102406_q38fn_nv4_싱글_사전판정_불가_멀티이관 §처분]
```

```hint-event
id: V2
kind: value-status
노브: distributed-executor-backend
값: ray
지위: inherited
근거: 멀티 브랜치 분산 서빙의 표준 실행기(서브는 Ray 워커) · 다른 실행기 시도 0
출처: [plan_26090918_qwen38_flashnext_multi_딥캠페인 §4. 단계 계획, docker-compose.yaml]
```

```hint-event
id: V3
kind: value-status
노브: gpu-memory-utilization
값: 0.80
지위: inherited
근거: 0.29 의 262k resident 셀에서 0.85 → 0.80 으로 낮춰 로드 피크 여유를 연 값의 승계 · 이 셀에서 바꿔 잰 기록 없음 · 이 셀 엔진은 KV 절대 클램프 때문에 KV 산정에 이 값을 쓰지 않는다고 기록(lite_engine L685 · L687)
출처: [testlog_26091304_qwen38fn_res최소조건_262k재발행_판정 §3.2, output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/lite_engine_q38fn31-nv4-1m-tp2.log]
```

```hint-event
id: V4
kind: value-status
노브: max-model-len
값: 1048576
지위: inherited
근거: 캠페인 목표값(지원 최대 context · YaRN f4) — 기동 검증 1,001,130 토큰 needle PASS · 다른 길이로 바꿔 잰 기록 없음
출처: [plan_26102412_4모델_멀티캠페인_Solar_Qwen_TP2_최대context §셀 (TP=2 이므로 노드 병렬 아님 · 순차), testlog_26102414_q38fn_nv4_멀티_TP2_1M_YaRN_기동_needle_lite_판정 §2. needle(MAGNOLIA-7734 · 증분 프리필 · thinking off · temperature 0)]
```

```hint-event
id: V5
kind: value-status
노브: kv-cache-dtype
값: fp8_e4m3
지위: inherited
근거: 첫 plan 이 1M 을 fp8 KV 전제로 계획한 값 · 0.29 1M resident 사살(W6)은 auto 와 fp8 둘 다에서 났으므로 dtype 이 그 실패를 가른 것은 아니다 · 이 셀에서 auto 로 잰 기록 없음
출처: [plan_26090918_qwen38_flashnext_multi_딥캠페인 §3. 셀 매트릭스 (기본 24셀 + 레버 서브스윕), testlog_26091121_qwen38fn_22셀재수행_캠페인완주_판정 §비-measured 셀(같은 체크포인트) — 4건]
```

```hint-event
id: V6
kind: value-status
노브: kv-cache-memory-bytes
값: 15032385536
지위: inherited
근거: 손레버 — 0.29 의 KV 20 GiB 사살 선례보다 줄여 14 GiB 로 한 번 정했고 스윕 0 · 엔진 용량은 1M 요청 2.01개분(max-num-seqs 1 대비 약 2배 · 음성대조 기록 없음)
출처: [testlog_26102414_q38fn_nv4_멀티_TP2_1M_YaRN_기동_needle_lite_판정 §1. 기동과 교정, output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/lite_engine_q38fn31-nv4-1m-tp2.log]
```

```hint-event
id: V7
kind: value-status
노브: enforce-eager
값: true
지위: inherited
근거: 0.29 계보 스모크 4차의 cudagraph 캡처 피크 사살(W5 · MTP k=3 · gmu 0.85 · KV 20 GiB 조건)에서 승계 · 이 셀 조건에서 cudagraph 시도 0
출처: [testlog_26090921_p0-1_nvfp4_mixed_패치_스모크_판정 §판정]
```

```hint-event
id: V8
kind: value-status
노브: async-scheduling
값: false
지위: engine-default
근거: yaml 의 false 가 엔진 non-default args 에 실리지 않았다 — 실효값은 엔진 기본값 규칙으로 켜짐이다(M3 · resolve 값을 찍는 줄은 없고 비활성 경고도 없다)
출처: [output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/lite_engine_head_q38fn31-nv4-1m-tp2.log]
```

```hint-event
id: V9
kind: value-status
노브: max-num-seqs
값: 1
지위: inherited
근거: 손레버 — 고른 이유는 계보에 기록이 없고 스윕 0 · 이 값 때문에 full 스윕 동시성 레벨 2 이상이 걸러졌다
출처: [output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/sweep_index.json, testlog_26102414_q38fn_nv4_멀티_TP2_1M_YaRN_기동_needle_lite_판정 §1. 기동과 교정]
```

```hint-event
id: V10
kind: value-status
노브: max-num-batched-tokens
값: 2048
지위: inherited
근거: 0.29 res 후속 캠페인 확정 조건(262k)의 값 승계 · 이 셀에서 바꿔 잰 기록 없음
출처: [perf_26091122_qwen38fn_24전략_종합리포트 §4. 정직성 노트 — nv4-bf-262k-res 레거시 "성공"은 재현 대상이 아니다]
```

```hint-event
id: V11
kind: value-status
노브: engram-config
값: '{"cpu_offload": false}'
지위: inherited
근거: PinnedHost offload 는 GB10 통합메모리에서 절감 0 이라는 메모리 구조 판단(R3) · 켜서 실패한 관측은 없다 · 측정 기동에서 PLE 는 장치 상주(weight_device=cuda:0 · pinned=False)
출처: [testlog_26102420_PLE_mmap_0.31.0_재이식_S1_선판정_런타임패치_슬롯_정정 §3. stock 0.31.0 의 두 백엔드가 GB10 에서 왜 상주를 못 줄이나, output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/lite_engine_q38fn31-nv4-1m-tp2.log]
```

```hint-event
id: V12
kind: value-status
노브: hf-overrides
값: '{"text_config":{"max_position_embeddings":1048576,"rope_parameters":{"mrope_interleaved":true,"mrope_section":[11,11,10],"partial_rotary_factor":0.25,"rope_theta":10000000,"rope_type":"yarn","factor":4,"original_max_position_embeddings":262144}}}'
지위: declared-requirement
근거: W8 — max_position_embeddings 없이 rope_parameters 만 실은 기동이 ValidationError(derived 262144)로 종료됐다
출처: [output/multi/benchlog/serve_fail_q38fn31-nv4-1m-tp2/master_exited.log, testlog_26102414_q38fn_nv4_멀티_TP2_1M_YaRN_기동_needle_lite_판정 §1. 기동과 교정]
```

**요약** — 이 셀에서 필요조건으로 확인된 노브는 hf-overrides(V12 · W8) 하나다. 나머지는 바꿔 잰 적 없는 승계값이고, 그중 복사하면 위험한 것은 셋이다. KV 클램프 14 GiB(V6)와 max-num-seqs 1(V9)은 손으로 고른 값이고 최적이라는 근거가 없다 — KV 는 1M 요청 두 개분을 잡고, max-num-seqs 1 은 동시성 곡선을 지웠다. eager(V7)는 다른 조건(MTP on · gmu 0.85 · KV 20 GiB)의 캡처 피크 사살에서 온 승계라 이 셀 조건에서 필요한지 확인되지 않았다(Q3). gmu 0.80(V3)은 262k resident 셀에서 효과가 확정된 값이지만 이 셀의 엔진은 KV 산정에 그 값을 쓰지 않는다. async-scheduling false(V8)는 효과가 없는 줄이고 이 측정은 비동기 스케줄링이 켜진 상태였다(M3).

## 1.4 재현 절차
> 셀 실행 기록에서 기계가 파생한 명령 순서와 단계별 소요다. 소요가 `미관측` 인 단계는 시각 기록이 없다는 뜻이다(0 이 아니다). 그 아래는 블랙박스 이벤트 원장에서 이 셀의 행을 기계가 뽑아 기동 시도로 묶은 것이고, 마지막은 측정한 실행의 엔진 로그가 echo 한 env 다(측정 당시 값의 정본).

<!-- FACT:reproduce -->
| 순서 | 단계 | 성공 판정 | 소요 | 관측 구간(UTC) | 소요 출처 | 명령 출처 |
|---|---|---|---|---|---|---|
| 1 | render | 러너 3종·.env·.env.cluster·.env.interconnect 생성(렌더러 fail-loud) | 미관측 | 미관측 | .claude/skills/upstream-version-watch/scripts/multinode_serve_smoke.sh 선행조건 안내 · .claude/skills/upstream-version-watch/scripts/render_dockerfile.py CLI | derived(렌더러 CLI) |
| 2 | build | 빌드 OK · 이미지 실재 | 층 창 38분 19초(docker history 첫 층 → 끝 층 CreatedAt · 캐시 재사용 층 포함 — 빌드 소요 아님) | 2026-10-14T01:07:00Z → 2026-10-14T01:45:19Z (layer-window) | docker history --no-trunc --format '{{json .}}' sha256:339737df6571ce00993e731ef064a8db66085beaa650e4aa63bdb24f414b2034 · 우리 Dockerfile 층 24개의 CreatedAt 첫 층 → 끝 층(캐시 재사용 층은 이전 빌드 시각 — 빌드 소요 아님) | reconstructed(build-ledger build_args + compose build) |
| 3 | serve | health 200 + 추론 1회(스모크 판정) | ≤ 24분 43초(상한) | 2026-10-25T01:52:06Z → 2026-10-25T02:16:49Z (upper) | docs/logs/main/events/2026-10.jsonl budget_declare(label smoke-q38fn31-nv4-1m-tp2) → 첫 측정 시각(output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/lite_raw_q38fn31-nv4-1m-tp2.json measured_utc(호스트 date -u · lite_bench.sh 가 cold 레그 직전에 찍는 측정 시작)) | derived(스모크 사용법) |
| 4 | bench | 리포트 발행(+PASS 면 인증서) | 10분 24초 | 2026-10-25T02:16:49Z → 2026-10-25T02:27:13Z (exact) | output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/lite_raw_q38fn31-nv4-1m-tp2.json measured_utc(호스트 date -u · lite_bench.sh 가 cold 레그 직전에 찍는 측정 시작) → sweep_index.generated_utc(호스트 UTC · 두 끝 같은 시계) | reconstructed(bench json) |

**빌드 층 시각**(관측 · 소요가 아니라 층 창 — 캐시 재사용 층은 이전 빌드의 시각을 가진다)

- `build` 이미지 층 CreatedAt(docker history): 2026-10-14T01:07:00Z → 2026-10-14T01:45:19Z · 층 24개 · 출처 `docker history --no-trunc --format '{{json .}}' sha256:339737df6571ce00993e731ef064a8db66085beaa650e4aa63bdb24f414b2034` — 우리 Dockerfile 층의 CreatedAt 범위 — 캐시 재사용 층은 이전 빌드 시각을 가진다(소요 아님)

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
# 실린 artifacts/build_recipe/Dockerfile.source-build(실린 개정: git:12ef50a89b86cb30d4c5b87c32506c5563ff4655(output/single/Dockerfile.source-build) — 이미지를 지은 바이트임을 관측으로 확인 못함(docker history RUN·COPY 16/17))를 output/single/Dockerfile.source-build 자리에 두고 컨텍스트에서 짓는다.
mkdir -p output/single/build_patches_src output/single/build_patches   # 빌드 원장 context_inventory.build_patch_pre = ['.gitkeep'] · 빌드 원장 context_inventory.build_patch_post = ['.gitkeep']
# 양 노드에서 각각 짓는다(이미지 save/load 전송 ✗ — 노드마다 자기 이미지 · 요건은 동일 digest 가 아니라 동일 ABI).
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
# 동등 경로 없음 — 스모크 · compose build 는 output/multi/ 의 Dockerfile 을 짓는다(이 이미지의 레시피 통로 output/single 아님) · 위 docker build 명령만 쓴다.
```

**3. serve**

```sh
bash .claude/skills/upstream-version-watch/scripts/multinode_serve_smoke.sh q38fn31-nv4-1m-tp2 --keep-up
```

**4. bench**

```sh
# bench JSON 필드로 복원: backend · model_id→--model · tokenizer_id→--tokenizer · num_prompts · max_concurrency · request_rate · burstiness · 요청당 입출력 길이 = total_*_tokens ÷ completed(나누어떨어질 때만 — 평균이다 · 요청별 길이는 JSON 에 없다) · 나누어떨어지면 `--dataset-name random` 으로 추정했다(측정 도구가 그 꼴로 부른다 — 아래 도구 원문).
# JSON 에 없는 인자(--endpoint · --base-url · --ignore-eos · --num-warmups · --temperature · --seed · --random-range-ratio · --trust-remote-code)는 측정 도구가 정했다 — level_* = run_bench.sh · lite_cold/lite_warm = lite_bench.sh(.claude/skills/adversarial-benchmark/scripts/) · 셀 스윕 순서는 sweep_bench.sh.
# output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/lite_cold_q38fn31-nv4-1m-tp2.json (date 20261025-021709) · ⚠ --random-input-len/--random-output-len = total ÷ completed 평균(chat 템플릿 포함 실측 토큰 · 측정 도구 원문 인자 미관측 — lite_bench.sh 원문 인자와 다를 수 있다)
vllm bench serve --backend openai-chat --model qwen3.8-flash-next --tokenizer /app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 --dataset-name random --random-input-len 566 --random-output-len 128 --num-prompts 1 --max-concurrency 1 --request-rate inf
# output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/lite_warm_q38fn31-nv4-1m-tp2.json (date 20261025-021739) · ⚠ --random-input-len/--random-output-len = total ÷ completed 평균(chat 템플릿 포함 실측 토큰 · 측정 도구 원문 인자 미관측 — lite_bench.sh 원문 인자와 다를 수 있다)
vllm bench serve --backend openai-chat --model qwen3.8-flash-next --tokenizer /app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 --dataset-name random --random-input-len 565 --random-output-len 128 --num-prompts 3 --max-concurrency 1 --request-rate inf
```

**슬롯 → 빌드 컨텍스트 매핑**(빌드 컨텍스트 = `output/<토폴로지>/` · 원천 = 실린 Dockerfile 의 COPY · compose 의 env_file · volumes · command — `—` 는 빌드 컨텍스트 입력이 아니다) — ⚠ 빌드 레시피 · 빌드 패치 행은 다른 컨텍스트 `output/single/`(이 이미지의 레시피 통로 · 2단계 docker build 와 같은 자리 · 자리 칸에 그 뿌리를 붙였다)

| zip 경로 | 빌드 컨텍스트 자리 | 근거 |
|---|---|---|
| `artifacts/triplet/.env.q38fn31-nv4-1m-tp2` | `envs/.env.q38fn31-nv4-1m-tp2` | compose env_file envs/.env.${CONFIG_FILE}(셀 env) |
| `artifacts/triplet/q38fn31-nv4-1m-tp2.sh` | `configs/q38fn31-nv4-1m-tp2.sh` | compose volumes ./configs:/app/configs |
| `artifacts/triplet/q38fn31-nv4-1m-tp2.yaml` | `configs/q38fn31-nv4-1m-tp2.yaml` | compose volumes ./configs:/app/configs |
| `artifacts/build_recipe/Dockerfile.source-build` | `output/single/Dockerfile.source-build` | docker build -f output/single/Dockerfile.source-build · 컨텍스트 output/single(01 §1.4 2단계 — 이 이미지의 레시피 통로 · compose build 는 output/multi/ 의 Dockerfile 을 지어 동등 경로가 아니다) |
| `artifacts/build_recipe/requirements.txt` | `output/single/requirements.txt` | Dockerfile.source-build COPY requirements.txt |
| `artifacts/compose/.env.cluster.template` | `envs/.env.cluster` | compose env_file envs/.env.cluster |
| `artifacts/compose/.env.interconnect.template` | `envs/.env.interconnect` | compose env_file envs/.env.interconnect |
| `artifacts/compose/.env.template` | `.env` | compose 변수치환 프로젝트 .env(${…} 참조) |
| `artifacts/compose/docker-compose.yaml` | `docker-compose.yaml` | compose 파일(빌드 컨텍스트 루트) |
| `artifacts/compose/serve_runner.sh` | `configs/serve_runner.sh` | compose volumes ./configs:/app/configs |
| `artifacts/compose/sub_recipe.json` | — | 빌드 컨텍스트 입력 아님(Dockerfile COPY · compose 참조 밖) |
| (실린 파일 없음 — 빈 디렉터리 생성) | `output/single/build_patches_src/` | Dockerfile.source-build COPY build_patches_src/ · 실린 패치 없음 — 빈 디렉터리를 만든다(없으면 COPY 단계에서 빌드가 멈춘다) · 빌드 원장 context_inventory.build_patch_pre = ['.gitkeep'] |
| (실린 파일 없음 — 빈 디렉터리 생성) | `output/single/build_patches/` | Dockerfile.source-build COPY build_patches/ · 실린 패치 없음 — 빈 디렉터리를 만든다(없으면 COPY 단계에서 빌드가 멈춘다) · 빌드 원장 context_inventory.build_patch_post = ['.gitkeep'] |

**env 형상의 파생 키 · 실효값**(한 manifest 필드 → 여러 키 · 키마다 자기 자리표시 — 값은 필드 값이 아니라 아래 규칙의 결과다)

| 형상 | manifest 필드 | 키 | 자리표시 | 실효값 규칙 | 출처 |
|---|---|---|---|---|---|
| `artifacts/compose/.env.interconnect.template` | `interconnect.nccl_transport` | `NCCL_IB_DISABLE` · `NCCL_NET` | `<derived:interconnect.nccl_transport→NCCL_IB_DISABLE>` · `<derived:interconnect.nccl_transport→NCCL_NET>` | rdma → NCCL_IB_DISABLE=0 · NCCL_NET=IB \| socket → NCCL_IB_DISABLE=1 · NCCL_NET=Socket | render_dockerfile.NCCL_TRANSPORTS(선택지 → 키 값) |
| `artifacts/compose/.env.interconnect.template` | `interconnect.socket_iface` | `GLOO_SOCKET_IFNAME` · `NCCL_SOCKET_IFNAME` · `OMPI_MCA_btl_tcp_if_include` · `TP_SOCKET_IFNAME` · `UCX_NET_DEVICES` | `<derived:interconnect.socket_iface→GLOO_SOCKET_IFNAME>` · `<derived:interconnect.socket_iface→NCCL_SOCKET_IFNAME>` · `<derived:interconnect.socket_iface→OMPI_MCA_btl_tcp_if_include>` · `<derived:interconnect.socket_iface→TP_SOCKET_IFNAME>` · `<derived:interconnect.socket_iface→UCX_NET_DEVICES>` | GLOO_SOCKET_IFNAME={interconnect.socket_iface} · NCCL_SOCKET_IFNAME={interconnect.socket_iface} · OMPI_MCA_btl_tcp_if_include={interconnect.socket_iface} · TP_SOCKET_IFNAME={interconnect.socket_iface} · UCX_NET_DEVICES={interconnect.socket_iface} | render_dockerfile.env_tier 탐침 manifest(탐침 값 → {field} 규칙) |

**기동 성공 판정(관측 · 발행 자격)**: health 200 = true · 추론 1회 = true · 근거 `output/multi/benchlog/serve_proof_q38fn31-nv4-1m-tp2.json` — 시점 범위: 작성 2026-10-25T02:16:28Z(output/multi/benchlog/serve_proof_q38fn31-nv4-1m-tp2.json 파일 mtime(파일시스템 관측 — 문서에 시각 칸이 없다 · 기록 필드 아님)) · 측정 창 대비 작성 2026-10-25T02:16:28Z < 측정 시작 2026-10-25T02:16:49Z — 측정 전(빌드·스모크)의 관측
<!-- /FACT:reproduce -->

<!-- FACT:event_timeline -->
**블랙박스 이벤트 39행**(원장에서 이 셀 label·config 에 맞는 행만 · 시각순 · 기계 발췌)

> **캠페인 경계 미관측** — 재생 모드(캠페인 밖 측정(측정 시점 ACTIVE=_bootstrap · 이 측정의 캠페인 인스턴스 없음)) — 캠페인 선언을 관측하지 못한다 · 경계 미관측 — 라벨(셀 이름)로 묶은 행 전부를 이 셀의 행으로 센다(같은 셀 이름의 앞선 캠페인 행이 섞였을 수 있다)

> **원장 관측 범위**(노드별 첫 행 → 마지막 행 · evidence.event_ledger_spans): main = 2026-07-31T03:27:43Z → 2026-10-25T03:13:55Z(`docs/logs/main/events/2026-07.jsonl` · `docs/logs/main/events/2026-08.jsonl` · `docs/logs/main/events/2026-09.jsonl` · `docs/logs/main/events/2026-10.jsonl` · `docs/logs/main/events/thermal.jsonl` · `docs/logs/main/events/watchdog.jsonl`) · sub = 2026-06-13T22:25:01Z → 2026-10-24T00:02:35Z(`sync_staging/sub_docs/logs/sub/events/2026-06.jsonl` · `sync_staging/sub_docs/logs/sub/events/2026-07.jsonl` · `sync_staging/sub_docs/logs/sub/events/2026-08.jsonl` · `sync_staging/sub_docs/logs/sub/events/2026-09.jsonl` · `sync_staging/sub_docs/logs/sub/events/2026-10.jsonl` · `sync_staging/sub_docs/logs/sub/events/thermal.jsonl` · `sync_staging/sub_docs/logs/sub/events/watchdog.jsonl` · 회수 미러가 가린 옛 로컬 사본 `docs/logs/sub/events/2026-09.jsonl`) — ⚠ sub(마지막 행 2026-10-24T00:02:35Z < 측정 끝 2026-10-25T02:27:13Z): 그 뒤 그 노드의 선언 · 사건은 **관측 범위 밖**이다(없음이 아니다 · 원장 미회수)

| # | UTC | 노드 | kind | label | 내용 | 출처 |
|---|---|---|---|---|---|---|
| 1 | 2026-10-24T05:01:27Z | main | `budget_declare` | `smoke-q38fn31-nv4-1m-tp2` | 예산 선언(선언값 — 측정 아님) · floor_mib=22206 · weights_mib=63266 · kv_mib=14336 · overhead_mib=24800 · mem_total_mib=124608 · 기동 창 1/5: budget_renew 0회 · 닫힘 budget_clear 2026-10-24T05:03:25Z(선언 후 1m58s) · measured_utc 2026-10-25T02:27:13Z 는 이 창 밖(다른 기동 · 측정 전) | docs/logs/main/events/2026-10.jsonl:514 |
| 2 | 2026-10-24T05:01:28Z | main | `budget_honored` | `smoke-q38fn31-nv4-1m-tp2` | budget_honored · floor_mib=22206 · arm_ceiling_mib=19134 · remaining_s=7199 · 창 시작 2026-10-24T05:01:28Z 후 0m00s | docs/logs/main/events/watchdog.jsonl:751 |
| 3 | 2026-10-24T05:03:25Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-q38fn31-nv4-1m-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-24T05:01:27Z 후 1m58s) | docs/logs/main/events/2026-10.jsonl:515 |
| 4 | 2026-10-24T05:03:25Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-q38fn31-nv4-1m-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-24T05:01:28Z 후 1m57s) | docs/logs/main/events/watchdog.jsonl:752 |
| 5 | 2026-10-24T05:03:48Z | main | `budget_declare` | `smoke-q38fn31-nv4-1m-tp2` | 예산 선언(선언값 — 측정 아님) · floor_mib=22206 · weights_mib=63266 · kv_mib=14336 · overhead_mib=24800 · mem_total_mib=124608 · 기동 창 2/5: budget_renew 1회 · 닫힘 budget_clear 2026-10-24T05:52:51Z(선언 후 49m03s) · measured_utc 2026-10-25T02:27:13Z 는 이 창 밖(다른 기동 · 측정 전) | docs/logs/main/events/2026-10.jsonl:517 |
| 6 | 2026-10-24T05:03:48Z | main | `budget_honored` | `smoke-q38fn31-nv4-1m-tp2` | budget_honored · floor_mib=22206 · arm_ceiling_mib=19134 · remaining_s=7200 · 창 시작 2026-10-24T05:03:48Z 후 0m00s | docs/logs/main/events/watchdog.jsonl:753 |
| 7 | 2026-10-24T05:05:08Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=76536 · rate_mib_s=28230 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → smoke-q38fn31-nv4-1m-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-24T05:03:48Z 후 1m20s) | docs/logs/main/events/watchdog.jsonl:754 |
| 8 | 2026-10-24T05:27:36Z | main | `budget_renew` | `smoke-q38fn31-nv4-1m-tp2` | budget_renew · floor_mib=22206 · ttl_s=7200 · remaining_before_s=5772 · 창 시작 2026-10-24T05:03:48Z 후 23m48s | docs/logs/main/events/2026-10.jsonl:518 |
| 9 | 2026-10-24T05:52:51Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-q38fn31-nv4-1m-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-24T05:03:48Z 후 49m03s) | docs/logs/main/events/2026-10.jsonl:519 |
| 10 | 2026-10-24T05:52:51Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-q38fn31-nv4-1m-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-24T05:03:48Z 후 49m03s) | docs/logs/main/events/watchdog.jsonl:755 |
| 11 | 2026-10-24T12:22:03Z | main | `budget_declare` | `smoke-q38fn31-nv4-1m-tp2` | 예산 선언(선언값 — 측정 아님) · floor_mib=22206 · weights_mib=63266 · kv_mib=14336 · overhead_mib=24800 · mem_total_mib=124608 · 기동 창 3/5: budget_renew 1회 · 닫힘 budget_clear 2026-10-24T12:47:59Z(선언 후 25m56s) · measured_utc 2026-10-25T02:27:13Z 는 이 창 밖(다른 기동 · 측정 전) | docs/logs/main/events/2026-10.jsonl:549 |
| 12 | 2026-10-24T12:22:04Z | main | `budget_honored` | `smoke-q38fn31-nv4-1m-tp2` | budget_honored · floor_mib=22206 · arm_ceiling_mib=19134 · remaining_s=7199 · 창 시작 2026-10-24T12:22:04Z 후 0m00s | docs/logs/main/events/watchdog.jsonl:775 |
| 13 | 2026-10-24T12:23:24Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=71952 · rate_mib_s=23428 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → smoke-q38fn31-nv4-1m-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-24T12:22:04Z 후 1m20s) | docs/logs/main/events/watchdog.jsonl:776 |
| 14 | 2026-10-24T12:46:06Z | main | `budget_renew` | `smoke-q38fn31-nv4-1m-tp2` | budget_renew · floor_mib=22206 · ttl_s=7200 · remaining_before_s=5757 · 창 시작 2026-10-24T12:22:03Z 후 24m03s | docs/logs/main/events/2026-10.jsonl:550 |
| 15 | 2026-10-24T12:47:59Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-q38fn31-nv4-1m-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-24T12:22:03Z 후 25m56s) | docs/logs/main/events/2026-10.jsonl:551 |
| 16 | 2026-10-24T12:48:00Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-q38fn31-nv4-1m-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-24T12:22:04Z 후 25m56s) | docs/logs/main/events/watchdog.jsonl:777 |
| 17 | 2026-10-24T20:16:04Z | main | `budget_declare` | `smoke-q38fn31-nv4-1m-tp2` | 예산 선언(선언값 — 측정 아님) · floor_mib=22206 · weights_mib=63266 · kv_mib=14336 · overhead_mib=24800 · mem_total_mib=124608 · 기동 창 4/5: budget_renew 1회 · 닫힘 budget_clear 2026-10-24T20:50:53Z(선언 후 34m49s) · measured_utc 2026-10-25T02:27:13Z 는 이 창 밖(다른 기동 · 측정 전) | docs/logs/main/events/2026-10.jsonl:586 |
| 18 | 2026-10-24T20:16:04Z | main | `budget_honored` | `smoke-q38fn31-nv4-1m-tp2` | budget_honored · floor_mib=22206 · arm_ceiling_mib=19134 · remaining_s=7200 · 창 시작 2026-10-24T20:16:04Z 후 0m00s | docs/logs/main/events/watchdog.jsonl:799 |
| 19 | 2026-10-24T20:17:24Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=68539 · rate_mib_s=24040 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → smoke-q38fn31-nv4-1m-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-24T20:16:04Z 후 1m20s) | docs/logs/main/events/watchdog.jsonl:800 |
| 20 | 2026-10-24T20:39:46Z | main | `budget_renew` | `smoke-q38fn31-nv4-1m-tp2` | budget_renew · floor_mib=22206 · ttl_s=7200 · remaining_before_s=5778 · 창 시작 2026-10-24T20:16:04Z 후 23m42s | docs/logs/main/events/2026-10.jsonl:587 |
| 21 | 2026-10-24T20:50:53Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-q38fn31-nv4-1m-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-24T20:16:04Z 후 34m49s) | docs/logs/main/events/2026-10.jsonl:588 |
| 22 | 2026-10-24T20:50:53Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-q38fn31-nv4-1m-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-24T20:16:04Z 후 34m49s) | docs/logs/main/events/watchdog.jsonl:801 |
| 23 | 2026-10-25T01:52:06Z | main | `budget_declare` | `smoke-q38fn31-nv4-1m-tp2` | 예산 선언(선언값 — 측정 아님) · floor_mib=22206 · weights_mib=63266 · kv_mib=14336 · overhead_mib=24800 · mem_total_mib=124608 · 기동 창 5/5: budget_renew 4회 · 닫힘 budget_clear 2026-10-25T02:27:37Z(선언 후 35m31s) · measured_utc 2026-10-25T02:27:13Z 가 이 창 안(이 측정의 기동) | docs/logs/main/events/2026-10.jsonl:643 |
| 24 | 2026-10-25T01:52:06Z | main | `budget_honored` | `smoke-q38fn31-nv4-1m-tp2` | budget_honored · floor_mib=22206 · arm_ceiling_mib=19134 · remaining_s=7200 · 창 시작 2026-10-25T01:52:06Z 후 0m00s | docs/logs/main/events/watchdog.jsonl:820 |
| 25 | 2026-10-25T01:53:22Z | cluster | `engine_log_window_start` | — | 캡처된 엔진 로그의 첫 시각(꼬리 캡처일 수 있다 — 엔진 시작 시각이 아닐 수 있다) · 선언 2026-10-25T01:52:06Z 후 1m16s · 시계: 컨테이너 시각을 UTC 로 읽어 측정 창 [2026-10-25T01:52:06Z, 2026-10-25T02:27:37Z] 안에 드는 것을 확인 | output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/lite_engine_q38fn31-nv4-1m-tp2.log:382 |
| 26 | 2026-10-25T01:53:26Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=77856 · rate_mib_s=29106 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → smoke-q38fn31-nv4-1m-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-25T01:52:06Z 후 1m20s) | docs/logs/main/events/watchdog.jsonl:821 |
| 27 | 2026-10-25T01:53:28Z | cluster | `engine_prefetch_disabled` | — | 가중치 auto-prefetch 꺼짐 줄 ×2 — filesystem CIFS(vLLM 로그 문구) | output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/lite_engine_q38fn31-nv4-1m-tp2.log:639 |
| 28 | 2026-10-25T02:01:32Z | cluster | `engine_weights_loaded` | — | 가중치 적재 완료 줄 ×2(rank·단계별) · 최장 483.60s · 이 행 = 마지막 줄 | output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/lite_engine_q38fn31-nv4-1m-tp2.log:661 |
| 29 | 2026-10-25T02:01:43Z | cluster | `engine_model_loaded` | — | 모델 적재 완료 줄 ×2(rank 별) · 마지막 줄 61.93 GiB · 498.9s | output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/lite_engine_q38fn31-nv4-1m-tp2.log:663 |
| 30 | 2026-10-25T02:02:46Z | cluster | `engine_shm_broadcast_wait` | — | shm_broadcast 대기 경고(60s 단위) ×12 · 2026-10-25T02:02:46Z–2026-10-25T02:13:46Z(11m00s) — vLLM 문구상 프로세스 hang 또는 긴 작업(컴파일·가중치/KV 양자화) 중 출현 | output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/lite_engine_q38fn31-nv4-1m-tp2.log:673 |
| 31 | 2026-10-25T02:14:39Z | cluster | `engine_kv_cache_sized` | — | GPU KV cache 2,112,392 tokens · 요청당 1,048,576 tokens 기준 최대 동시성 2.01x | output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/lite_engine_q38fn31-nv4-1m-tp2.log:688 |
| 32 | 2026-10-25T02:15:04Z | cluster | `engine_init_done` | — | 엔진 초기화(profile·KV 할당·warmup) 799.95s | output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/lite_engine_q38fn31-nv4-1m-tp2.log:717 |
| 33 | 2026-10-25T02:15:29Z | cluster | `server_starting` | — | API 서버 기동 — 이 측정 창 선언 2026-10-25T01:52:06Z 후 23m23s · 캡처 로그 첫 시각 2026-10-25T01:53:22Z 후 22m07s | output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/lite_engine_q38fn31-nv4-1m-tp2.log:742 |
| 34 | 2026-10-25T02:16:29Z | main | `budget_renew` | `smoke-q38fn31-nv4-1m-tp2` | budget_renew · floor_mib=22206 · ttl_s=7200 · remaining_before_s=5737 · 창 시작 2026-10-25T01:52:06Z 후 24m23s | docs/logs/main/events/2026-10.jsonl:644 |
| 35 | 2026-10-25T02:17:42Z | main | `budget_renew` | `smoke-q38fn31-nv4-1m-tp2` | budget_renew · floor_mib=22206 · ttl_s=7200 · remaining_before_s=7127 · 창 시작 2026-10-25T01:52:06Z 후 25m36s | docs/logs/main/events/2026-10.jsonl:645 |
| 36 | 2026-10-25T02:20:54Z | main | `budget_renew` | `smoke-q38fn31-nv4-1m-tp2` | budget_renew · floor_mib=22206 · ttl_s=7200 · remaining_before_s=7008 · 창 시작 2026-10-25T01:52:06Z 후 28m48s | docs/logs/main/events/2026-10.jsonl:646 |
| 37 | 2026-10-25T02:24:03Z | main | `budget_renew` | `smoke-q38fn31-nv4-1m-tp2` | budget_renew · floor_mib=22206 · ttl_s=7200 · remaining_before_s=7011 · 창 시작 2026-10-25T01:52:06Z 후 31m57s | docs/logs/main/events/2026-10.jsonl:647 |
| 38 | 2026-10-25T02:27:37Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-q38fn31-nv4-1m-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-25T01:52:06Z 후 35m31s) | docs/logs/main/events/2026-10.jsonl:648 |
| 39 | 2026-10-25T02:27:38Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-q38fn31-nv4-1m-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-25T01:52:06Z 후 35m32s) | docs/logs/main/events/watchdog.jsonl:822 |

**기동 시도**(예산 선언 `budget_declare` 마다 한 시도 · 같은 노드의 이어지는 행과 선언 없는 노드(엔진 로그)의 그 시각 행을 묶었다 · 사살 · 트립 · 거부는 닫힘과 따로 센다 · `measurement` 행은 측정 기록이라 시도로 세지 않는다(위 표에만) — 해석은 저작자 몫)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 1 | main | `smoke-q38fn31-nv4-1m-tp2` | 2026-10-24T05:01:27Z | 없음 | 없음 | `budget_clear` 2026-10-24T05:03:25Z | 2 |
| 2 | main | `smoke-q38fn31-nv4-1m-tp2` | 2026-10-24T05:03:48Z | 1회 · 첫 2026-10-24T05:27:36Z | 없음 | `budget_clear` 2026-10-24T05:52:51Z | 3 |
| 3 | main | `smoke-q38fn31-nv4-1m-tp2` | 2026-10-24T12:22:03Z | 1회 · 첫 2026-10-24T12:46:06Z | 없음 | `budget_clear` 2026-10-24T12:47:59Z | 3 |
| 4 | main | `smoke-q38fn31-nv4-1m-tp2` | 2026-10-24T20:16:04Z | 1회 · 첫 2026-10-24T20:39:46Z | 없음 | `budget_clear` 2026-10-24T20:50:53Z | 3 |
| 5 | main | `smoke-q38fn31-nv4-1m-tp2` | 2026-10-25T01:52:06Z | 4회 · 첫 2026-10-25T02:16:29Z | 없음 | `budget_clear` 2026-10-25T02:27:37Z | 11 |
<!-- /FACT:event_timeline -->

<!-- FACT:measurement_env -->
**측정 실행의 env 관측 29행**(측정한 실행의 엔진 로그가 echo 한 값 · 기계 발췌 — 측정 당시 값의 정본이다. 표에 없는 키는 로그가 echo 하지 않은 것이지 설정되지 않았다는 뜻이 아니다 · 측정 뒤 재생성된 env 형상은 01 §1.1 을 본다)

| # | 키 | 값 | 노드 | 종류 | 출처(첫 줄) | 관측 |
|---|---|---|---|---|---|---|
| 1 | `NCCL_SOCKET_IFNAME` | `<nic:cluster>` | sub | env-echo | output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/lite_engine_head_q38fn31-nv4-1m-tp2.log:L110 | 로그 1개 |
| 2 | `NCCL_SOCKET_IFNAME` | `<nic:cluster>` | sub | param-echo | output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/lite_engine_head_q38fn31-nv4-1m-tp2.log:L111 | 로그 1개 |
| 3 | `nccl:version` | `2.30.7+cuda13.3` | sub | runtime | output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/lite_engine_head_q38fn31-nv4-1m-tp2.log:L112 | 로그 1개 |
| 4 | `NCCL_NET_PLUGIN` | `spcx` | sub | env-echo | output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/lite_engine_head_q38fn31-nv4-1m-tp2.log:L114 | 로그 1개 |
| 5 | `NCCL_IB_DISABLE` | `0` | sub | env-echo | output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/lite_engine_head_q38fn31-nv4-1m-tp2.log:L119 | 로그 1개 |
| 6 | `NCCL_IB_HCA` | `=<nic:cluster>,<nic:cluster>` | sub | param-echo | output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/lite_engine_head_q38fn31-nv4-1m-tp2.log:L121 | 로그 1개 |
| 7 | `NCCL_IB_MERGE_NICS` | `1` | sub | env-echo | output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/lite_engine_head_q38fn31-nv4-1m-tp2.log:L124 | 로그 1개 |
| 8 | `nccl:net_ib_devices` | `[0]<nic:cluster>:1/RoCE [1]<nic:cluster>:1/RoCE [RO]; OOB <nic:cluster>:<node:sub><0>` | sub | runtime | output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/lite_engine_head_q38fn31-nv4-1m-tp2.log:L127 | 로그 1개 |
| 9 | `NCCL_IB_QPS_PER_CONNECTION` | `4` | sub | env-echo | output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/lite_engine_head_q38fn31-nv4-1m-tp2.log:L140 | 로그 1개 |
| 10 | `nccl:net_ib_devices` | `[RO]; OOB <nic:cluster>:<node:sub><0>` | sub | runtime | output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/lite_engine_head_q38fn31-nv4-1m-tp2.log:L151 | 로그 1개 |
| 11 | `nccl:network` | `IB` | sub | runtime | output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/lite_engine_head_q38fn31-nv4-1m-tp2.log:L163 | 로그 1개 |
| 12 | `NCCL_DMABUF_ENABLE` | `0` | sub | env-echo | output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/lite_engine_head_q38fn31-nv4-1m-tp2.log:L165 | 로그 1개 |
| 13 | `NCCL_CROSS_NIC` | `1` | sub | env-echo | output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/lite_engine_head_q38fn31-nv4-1m-tp2.log:L169 | 로그 1개 |
| 14 | `NCCL_IB_SPLIT_DATA_ON_QPS` | `0` | sub | env-echo | output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/lite_engine_q38fn31-nv4-1m-tp2.log:L268 | 로그 1개 |
| 15 | `NCCL_IB_GID_INDEX` | `3` | sub | env-echo | output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/lite_engine_q38fn31-nv4-1m-tp2.log:L280 | 로그 1개 |
| 16 | `NCCL_SOCKET_IFNAME` | `<nic:cluster>` | main | env-echo | output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/lite_engine_q38fn31-nv4-1m-tp2.log:L524 | 로그 1개 · Ray 접힘 +3(접힌 사본의 노드 미관측) |
| 17 | `NCCL_SOCKET_IFNAME` | `<nic:cluster>` | main | param-echo | output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/lite_engine_q38fn31-nv4-1m-tp2.log:L525 | 로그 1개 · Ray 접힘 +2(접힌 사본의 노드 미관측) |
| 18 | `nccl:version` | `2.30.7+cuda13.3` | main | runtime | output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/lite_engine_q38fn31-nv4-1m-tp2.log:L526 | 로그 1개 |
| 19 | `NCCL_NET_PLUGIN` | `spcx` | main | env-echo | output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/lite_engine_q38fn31-nv4-1m-tp2.log:L528 | 로그 1개 |
| 20 | `NCCL_IB_DISABLE` | `0` | main | env-echo | output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/lite_engine_q38fn31-nv4-1m-tp2.log:L533 | 로그 1개 · Ray 접힘 +2(접힌 사본의 노드 미관측) |
| 21 | `NCCL_IB_HCA` | `=<nic:cluster>,<nic:cluster>` | main | param-echo | output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/lite_engine_q38fn31-nv4-1m-tp2.log:L534 | 로그 1개 · Ray 접힘 +2(접힌 사본의 노드 미관측) |
| 22 | `NCCL_IB_MERGE_NICS` | `1` | main | env-echo | output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/lite_engine_q38fn31-nv4-1m-tp2.log:L536 | 로그 1개 |
| 23 | `nccl:net_ib_devices` | `[RO]; OOB <nic:cluster>:<node:main><0>` | main | runtime | output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/lite_engine_q38fn31-nv4-1m-tp2.log:L538 | 로그 1개 · Ray 접힘 +2(접힌 사본의 노드 미관측) |
| 24 | `NCCL_IB_QPS_PER_CONNECTION` | `4` | main | env-echo | output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/lite_engine_q38fn31-nv4-1m-tp2.log:L548 | 로그 1개 · Ray 접힘 +3(접힌 사본의 노드 미관측) |
| 25 | `nccl:network` | `IB` | main | runtime | output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/lite_engine_q38fn31-nv4-1m-tp2.log:L561 | 로그 1개 · Ray 접힘 +3(접힌 사본의 노드 미관측) |
| 26 | `NCCL_DMABUF_ENABLE` | `0` | main | env-echo | output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/lite_engine_q38fn31-nv4-1m-tp2.log:L563 | 로그 1개 |
| 27 | `NCCL_CROSS_NIC` | `1` | main | env-echo | output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/lite_engine_q38fn31-nv4-1m-tp2.log:L567 | 로그 1개 |
| 28 | `NCCL_IB_SPLIT_DATA_ON_QPS` | `0` | main | env-echo | output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/lite_engine_q38fn31-nv4-1m-tp2.log:L620 | 로그 1개 |
| 29 | `NCCL_IB_GID_INDEX` | `3` | main | env-echo | output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/lite_engine_q38fn31-nv4-1m-tp2.log:L625 | 로그 1개 |

- 값 주: 기계 치환(pii.substitution_table · 장치·호스트·주소는 자리표시 — 원문은 출처 줄)

- Ray 접힘: 출처 줄 끝의 `[repeated Nx across cluster]` — Ray 가 다른 프로세스의 같은 줄 N개를 한 줄로 접었다. 노드 칸은 **첫 줄**의 노드이고, 접힌 사본이 어느 노드의 것인지는 로그에 없다(관측 대상 밖).

- 로그 창(꼬리): `tail-800(lite_bench.sh@d33b0878a6c3 L234)`(꼬리 창 출처 로그 1개 · 이 표 행 #14–29) — 이 캡처는 엔진 출력의 **마지막 N줄**이다(측정 도구의 `tail -N` 상한에 닿았다 · 생산자 evidence `log_window`). 창 앞에서 echo 한 줄은 그 로그에 없다 — 이 주의는 꼬리 창 출처 행에만 걸린다(머리 캡처 행은 아래).

- 로그 창(머리): `head-600(output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/lite_raw_q38fn31-nv4-1m-tp2.json engine_log_head_lines)`(머리 캡처 출처 로그 1개 · 이 표 행 #1–13) — 이 캡처는 엔진 출력의 **처음 N줄**이다(lite_bench.sh 머리 캡처 `head -n N` 상한에 닿았다 · 생산자 evidence `log_head_window`). 꼬리 창의 주의는 이 행에 걸리지 않는다 · 머리 N줄 뒤에서 echo 한 줄은 그 로그에 없다. 머리 캡처와 꼬리 창 사이(엔진 출력의 가운데)는 어느 캡처에도 없다 — 표에 없는 키 · 한 노드에만 있는 키는 그 사이였을 수 있다.
<!-- /FACT:measurement_env -->

> 이 절이 답하는 질문: 수신자가 이 zip 만으로 이 셀을 다시 띄우려면 어떤 순서로 무엇을 하고, 각 단계의 성공을 무엇으로 판정하며, 얼마나 기다려야 하는가?

**(a) 사전 준비** — 가중치는 NAS 관리 경로를 컨테이너 `/app/quant_models` 로 읽기 전용 마운트한다(compose `${QUANT_MODEL_PATH}:/app/quant_models:ro` · 프로젝트 `.env` 형상의 `QUANT_MODEL_PATH=<manifest.quant_model_path>`). 체크포인트는 `<manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4`(safetensors 합 123.53 GiB · 엔진 로그 `Checkpoint size: 123.57 GiB`)이고 **양 노드의 같은 경로**에 있어야 한다(1.5 선행조건). 이 셀은 PLE resident 라 PLE 스테이징이 필요 없다 — compose 의 `PLE_MMAP_HOST_PATH` 마운트는 셀 env 에 `VLLM_PLE_MMAP` 이 없으면 읽히지 않는다. 필수 env 의 자리: 셀 env `artifacts/triplet/.env.q38fn31-nv4-1m-tp2`(IMAGE_TAG · BUILD_DOCKERFILE · VLLM_REF · BUILD_JOBS · CONFIG_FILE · 컨테이너 이름 3종) · 프로젝트 `.env`(마운트 4종) · `.env.cluster`(두 노드 주소 · Ray 포트) · `.env.interconnect`(NCCL). 측정 당시 인터커넥트 값은 위 '측정 실행의 env 관측' 표가 정본이다 — `NCCL_IB_DISABLE=0` · `NCCL_NET_PLUGIN=spcx` · `NCCL_IB_GID_INDEX=3` · `NCCL_IB_QPS_PER_CONNECTION=4` · `NCCL_IB_MERGE_NICS=1` · `NCCL_CROSS_NIC=1` · `NCCL_DMABUF_ENABLE=0` · `NCCL_IB_SPLIT_DATA_ON_QPS=0`(양 노드 같음 · 1.5). 렌더 단계 명령은 **지금의** 렌더러로 다시 만드는 명령이다 — 다시 렌더한 `.env.interconnect` 가 이 값들과 다르면 측정 당시 값에 맞춘다.

**(b) 빌드** — 양 노드에서 **각자** 짓는다(이미지 전송 ✗ · 요건은 동일 digest 가 아니라 동일 ABI). 선택자는 `BUILD_DOCKERFILE=Dockerfile.source-build` 이고 build-arg 는 위 2단계 명령 그대로(`VLLM_REF=v0.31.0` · `TORCH_CUDA_ARCH=12.1a` · `RAY_VERSION=2.48.0` · `SM12X_PORT=0` · `SRC_DEPS_AUTHORITY=0` · `BUILD_JOBS=8`)다. 빌드 컨텍스트는 `output/single/` 이고 명령은 위 2단계의 `docker build -f output/single/Dockerfile.source-build … output/single` 하나다 — 스모크의 `--build-only` · compose build 는 `output/multi/` 의 Dockerfile 을 지어 이 이미지와 동등하지 않다(2단계 주석 '동등 경로 없음'). 컨텍스트에 `build_patches_src/` · `build_patches/` 가 있어야 COPY(L165 · L203)가 통과하므로 위 2단계의 `mkdir -p output/single/build_patches_src output/single/build_patches` 를 먼저 둔다(빌드 원장 `context_inventory` = `.gitkeep` 뿐 · 1.2). 빌드 소요는 계보 밖 기록에 있다 — main 40분(01:08:32 → 01:48:59Z) · sub 37분(testlog_26101412 §1 · §2 P4) · 위 표의 38분 19초는 층 CreatedAt 창이다. 실린 Dockerfile 은 측정 이미지 history 와 16/17 만 맞으므로(가드 echo 단계 하나 · 1.2) 지은 이미지가 같은 바이트라고 가정하지 말고 1.5 의 ABI 축(vLLM SHA `db9527a46873454610df6dbedf79a36d6bf1a7f6` · torch · driver)으로 맞춘다.

**(c) 기동 순서와 성공 판정** — `multinode_serve_smoke.sh q38fn31-nv4-1m-tp2 --keep-up` 이 서브 slave(Ray worker) → 메인 master(Ray head · GPU 2개 합류 대기 · `vllm serve`) 순으로 띄운다(1.5 기동 순서). 측정 기동(시도 5)의 관측 타임라인은 원장 표 #23 ~ #33 이다: 예산 선언 01:52:06Z → 가중치 적재 완료 02:01:32Z(최장 483.60 s · CIFS 라 auto-prefetch 꺼짐) → 모델 적재 61.93 GiB/rank(02:01:43Z) → `shm_broadcast` 60초 대기 경고 12회(02:02:46Z ~ 02:13:46Z · 11분) → KV 예약 14.0 GiB · 2,112,392 토큰(02:14:39Z) → 엔진 초기화 799.95 s → `Starting vLLM server`(02:15:29Z). 선언 → 서버 기동은 **23분 23초**이고, 그 사이의 반복 대기 경고는 벽이 아니다 — 같은 창 직전 02:02:08 에 양 rank 가 flashinfer JIT `fused_moe_120` 빌드를 시작했다(lite_engine L670 · L690 · 대기가 그 빌드라는 것은 시각 대조에 의한 추론 · 02 Q7). 성공 판정은 health 200 + 추론 1회(serve proof `health 200 = true · 추론 1회 = true`)이고, 패치가 없으므로 패치 발화 서명은 없다 — 대신 `non-default args` 에 hf_overrides 의 `max_position_embeddings: 1048576` 이 실리고 `Using max model len 1048576` 이 찍히는지(lite_engine_head L62 · L72), PLE 가 `weight_device=cuda:0` 으로 초기화되는지(lite_engine L631 · L649)를 본다. 기동 시도는 원장 기준 **5번**이다(02 §2.2 대응 문단 — 시도 1 = W8 ValidationError · 시도 2 = health · needle · 캠페인 lite · 시도 4 = 무효 첫 full 패스와 시각 겹침(추론) · 시도 3 = 미확인 · 시도 5 = 측정). testlog_26102414 의 1차 시도(W7)는 원장에 선언 행이 없다.

**(d) 호스트 메모리 안전 예산 선언** — 선언값(측정 아님): `weights_mib 63266` · `kv_mib 14336` · `overhead_mib 24800` · `mem_total_mib 124608` → `floor_mib 22206` · 워치독이 답한 `arm_ceiling_mib 19134`(원장 표 #23 · #24). 산식은 `floor = mem_total − weights − kv − overhead` · `arm_ceiling = floor − decl_margin(3072)` · 선언 가능 ⟺ `arm_ceiling ≥ 8192`(budget_preflight.py 머리 주석 · 상수 출처 `blackbox_eta.DEFAULTS`)다. 엔진 실측은 rank 당 적재 61.93 GiB · KV 14.0 GiB 이고, 측정 중 시스템 RAM 점유는 main 93.6 GiB(77%) · sub 90.3 GiB(74%)다(bench_report_26102511 §lite 지표). 벤치 클라이언트(GuideLLM 컨테이너) 예산은 별도 게이트 `floor − bench ≥ abs_band(10240)` 를 따른다 — 이 셀은 상한 11966 MiB 이고 스윕 색인 기록값은 4096 MiB 다(02 M4).

**(e) 측정 명령** — 위 4단계 명령은 bench JSON 에서 재구성한 lite 레그(vLLM bench)뿐이고 실행 원문이 아니다. full 판정점은 GuideLLM 0.7.3 컨테이너(`ghcr.io/vllm-project/guidellm:v0.7.3`)가 동시성 1 · 입력 1024 · 출력 256 · 요청 16(+warmup 2) · 반복 3 으로 쟀다 — 인자 원문은 03 §3.4 의 `run_bench.sh@29cbdbc89a3d` · `lite_bench.sh@d33b0878a6c3` 발췌다.

**(f) 엔진이 스스로 고른 런타임 경로** — 통신은 NCCL 2.30.7 · `nccl:network = IB`(RoCE 장치) · all-reduce 백엔드 `PYNCCL`(lite_engine L386 · L512)이다. 서브 머리 로그에 `Spectrum-X (SPCX) NCCL plugin is not supported on device ... skipping` 이 찍힌다(lite_engine_head L145 · L149 — NCCL_NET_PLUGIN=spcx 를 줬지만 장치에서 건너뜀). MoE 는 `FLASHINFER_CUTLASS`(auto · L520) · 선형 attention prefill 은 FlashInfer GDN(L518) · attention 은 FlashAttention 2(L632)다. 엔진은 `enforce-eager` 때문에 torch.compile · CUDAGraph · JIT warmup 을 끄고(lite_engine_head L82), prefix caching 을 켠 채 mamba cache 를 `align` 으로 두며(L80 · L93 `enable_prefix_caching=True`), KV 절대 클램프 때문에 메모리 프로파일링을 건너뛰고 gpu-memory-utilization 을 KV 산정에 쓰지 않는다고 적는다(lite_engine L685 · L687). 무시해도 되는 경고: `Unknown vLLM environment variable detected: VLLM_PLE_MMAP`(compose 기본값 0 주입) · `Unrecognized keys in rope_parameters for 'rope_type'='default'`(체크포인트 config 의 rope_type 이 default 다 · 02 Q4) · Ray 의 `Tensor parallel size (2) exceeds available GPUs (1)`(노드당 GPU 1 · 2노드라 정상 · lite_engine_head L94) · 기본 샘플링이 `generation_config.json` 으로 덮였다는 경고(temperature 1.0 · top_k 20 · top_p 0.95 · lite_engine L739 — 벤치 요청이 temperature 를 주지 않으면 이 값이 쓰인다).

## 1.5 서브 레시피 해설
<!-- FACT:sub_recipe -->
원본: `artifacts/compose/sub_recipe.json`(기계 파생 — 스모크와 같은 함수 `slave_forward.derive` 의 출력).

| 역할 | 하는 일 |
|---|---|
| `master` | ray head + vllm serve(트리플렛 source · 모델 러너 실행) |
| `master_runtime_patch` | none(런타임 패치 파일 없음 — CONFIG_FILE='q38fn31-nv4-1m-tp2' → q38fn31-nv4-1m-tp2_patch.py 부재) |
| `runner_gpu_join_literal` | {"lines": [79], "note": "master 는 ray status 의 GPU 합류가 이 리터럴과 같아질 때까지 기다린다 — 노드당 GPU·노드 수가 다른 클러스터에서는 이 줄을 고치지 않으면 대기에서 멈춘다(arch 의 <G>g<N>n 과 숨은 결합).", "value": "2"} |
| `runtime_patch_key_chain` | RUNTIME_PATCH_STEM · CONFIG_FILE · 'default' |
| `slave` | ray worker(트리플렛 미수령 — 같은 이미지·같은 compose·같은 serve_runner.sh 가 NODE_ROLE 로 분기) |
| `slave_config_file` | default |
| `slave_runtime_patch` | not-armed — slave CONFIG_FILE='default' → arm_patch.sh 가 /app/configs/default_patch.py 를 찾고 없으면 no-op (K11) |
| `source` | {"runner_gpu_join_literal": "render_dockerfile.SHARED_ASSET_DIR/serve_runner.sh(측정 당시 개정 — slots.compose.evidence.selected_revisions)", "runtime_patch": "output/multi/configs/<stem>_patch.py 실재 여부 × arm_patch.sh 측정 당시 개정(d781f26e4ebc) `_cfg=` 키 체인 RUNTIME_PATCH_STEM > CONFIG_FILE > 'default'", "slave_config_file": "compose-default:CONFIG_FILE × slave_forward.NEVER_FORWARD"} |

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
| `observed` | {"build_ledger": {"compared": "dockerfile · build_args(− exempt) · patches(phase,file,script_sha256,result) · vllm git sha", "identity_exempt_build_args": ["BUILD_JOBS"], "main_identity_sha256": "15eb65137dbc8feb8a39af6bdd1ee7114d2c45774aee110529f3ed0a81d91295", "sub_identity_sha256": null, "verdict": "unobservable"}, "driver": {"main": "580.178.04", "sub": "580.178.04", "verdict": "equal"}, "torch": {"main": "2.13.0a0+9186a08b2c.nv26.7.59513937", "sub": "2.13.0a0+9186a08b2c.nv26.7.59513937", "verdict": "equal"}, "vllm_sha": {"main": "db9527a46873454610df6dbedf79a36d6bf1a7f6", "sub": "db9527a46873454610df6dbedf79a36d6bf1a7f6", "verdict": "equal"}} |
| `source` | output/multi/benchlog/attestation_q38fn31-nv4-1m-tp2.json |

> **attestation 범위** — config `q38fn31-nv4-1m-tp2` · phase serve · 작성 2026-10-25T02:16:28Z(output/multi/benchlog/attestation_q38fn31-nv4-1m-tp2.json 파일 mtime(파일시스템 관측 — 문서에 시각 칸이 없다 · 기록 필드 아님)) · 묶은 근거 cell-name · 파일 `output/multi/benchlog/attestation_q38fn31-nv4-1m-tp2.json` · 측정 창 대비 작성 2026-10-25T02:16:28Z < 측정 시작 2026-10-25T02:16:49Z — 측정 전(빌드·스모크)의 관측: **same config(q38fn31-nv4-1m-tp2) · serve — 원장 순서로 판별한 같은 기동의 관측(스모크 로그 미보존 · 약한 판별)(판별: 원장 순서로 판별(스모크 로그 미보존 — docs/simlog/**/*serve*.log 에 이 attestation 을 적은 스모크 로그 없음): 원장 docs/logs/main/events/2026-10.jsonl:643 budget_declare(smoke-q38fn31-nv4-1m-tp2 · 2026-10-25T01:52:06Z) ≤ attestation 작성 2026-10-25T02:16:28Z · serve_proof 작성 2026-10-25T02:16:28Z < 측정 시작 2026-10-25T02:16:49Z · 그 선언이 측정 끝 2026-10-25T02:27:13Z 까지 재선언 · 창 닫힘 없이 이어짐(main 원장 기록 범위 안) · API 서버 기동 2026-10-25T02:15:29Z(output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/lite_engine_q38fn31-nv4-1m-tp2.log:742 · 컨테이너 시계 = UTC 가정) ∈ [선언, attestation 작성] · 측정 엔진 로그 APIServer pid 789 하나)**
<!-- /FACT:sub_recipe -->

<!-- FACT:measurement_env_nodes -->
**노드별 측정 env**(01 §1.4 표를 키 × 노드로 접은 것 · `만 관측` = 다른 쪽 줄이 캡처 로그에 없다 · Ray 가 같은 줄을 접은 키(`[repeated Nx across cluster]`)와 머리·꼬리 캡처 로그(01 §1.4 로그 창)에서 한쪽만 남은 키는 다른 쪽 부재를 단언하지 않는다)

- 양 노드 같은 값 12개: `NCCL_CROSS_NIC` · `NCCL_DMABUF_ENABLE` · `NCCL_IB_DISABLE` · `NCCL_IB_GID_INDEX` · `NCCL_IB_HCA` · `NCCL_IB_MERGE_NICS` · `NCCL_IB_QPS_PER_CONNECTION` · `NCCL_IB_SPLIT_DATA_ON_QPS` · `NCCL_NET_PLUGIN` · `NCCL_SOCKET_IFNAME` · `nccl:network` · `nccl:version`

| 키 | main | sub | 노드 미특정 | 판정 |
|---|---|---|---|---|
| `nccl:net_ib_devices` | `[RO]; OOB <nic:cluster>:<node:main><0>` | `[0]<nic:cluster>:1/RoCE [1]<nic:cluster>:1/RoCE [RO]; OOB <nic:cluster>:<node:sub><0>` · `[RO]; OOB <nic:cluster>:<node:sub><0>` | — | 다름 |
<!-- /FACT:measurement_env_nodes -->

> 이 절이 답하는 질문: 서브(Ray worker) 노드는 메인과 무엇이 다르고, 무엇을 반드시 같게 맞춰야 하는가?

**(1) 역할 차이** — master 는 Ray head + `vllm serve` 이고 셀 트리플렛(yaml · 러너 · 셀 env)을 받는다. slave 는 Ray worker 이고 트리플렛을 받지 않는다 — 같은 이미지 · 같은 compose · 같은 `serve_runner.sh` 가 `NODE_ROLE` 로 갈라지며, slave 의 `CONFIG_FILE` 은 `default` 다. master 는 `ray status` 의 GPU 합류가 리터럴 `2` 와 같아질 때까지 기다린 뒤 serve 한다(sub_recipe `runner_gpu_join_literal` — 노드당 GPU · 노드 수가 다르면 이 줄을 고쳐야 한다). 이 셀은 런타임 패치가 없으므로 양쪽 모두 `arm_patch.sh` 가 no-op 이다(master · slave 런타임 패치 행).

**(2) 서브에 전달되는 env(무리별)** — 이미지 정체성(`IMAGE_TAG` · `BUILD_DOCKERFILE` · `VLLM_REF` — 빠지면 slave 가 compose 기본 태그 · Dockerfile · ref 로 빌드한다) · 빌드 튜닝(`BUILD_JOBS` — 정체성은 아니지만 slave 만 기본 16 으로 컴파일하면 OOM 선례) · 마운트(`QUANT_MODEL_PATH` 등 — slave 도 가중치를 올리므로 같은 마운트원이 필요하고, `--env-file` 사용 시 프로젝트 `.env` 자동 로드가 꺼지는 결함 때문에 셸 env 로 명시 주입한다) · 클러스터(`SLAVE_CONTAINER_NAME` — 서브 워치독 필터가 이 이름으로 매칭한다). 이유 원문은 1.5 사실 블록의 서브 전달 env 표다. 이 셀은 PLE mmap 을 쓰지 않으므로 `PLE_MMAP_HOST_PATH` 는 전달되지만 읽히지 않는다.

**(3) 선행조건** — 체크포인트가 서브의 **같은 마운트 경로**에 실재(`<manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4`) · 드라이버 계열 동일(측정 시 양 노드 580.178.04) · 멀티 브랜치 산출물(compose 등)이 서브에 배달돼 있을 것(W7 — 없으면 스모크가 서브 통로 미완결로 멈춘다) · RoCE 인터커넥트(NCCL `IB` 경로)가 준비돼 있을 것.

**(4) ABI 동일성** — attestation(측정 시작 2026-10-25T02:16:49Z 직전 02:16:28Z 작성 · 원장 순서로 같은 기동의 관측으로 판별한 **약한 판별** — 사실 블록 범위 줄)의 노드별 관측은 vLLM SHA `db9527a46873454610df6dbedf79a36d6bf1a7f6` equal · torch `2.13.0a0+9186a08b2c.nv26.7.59513937` equal · driver `580.178.04` equal 이다. 빌드 원장 축은 `sub_identity_sha256: null` 로 **unobservable** 이다 — 서브 이미지의 빌드 인자 · 패치 집합이 메인과 같은지는 관측하지 못했다(02 Q5). 같아야 하는 것은 이미지 digest 가 아니라 이 ABI 다(노드마다 로컬 빌드).

**(5) 측정 당시 인터커넥트 실효값** — 노드별 측정 env 표의 12키가 양 노드 같은 값이다: `NCCL_IB_DISABLE=0`(01 §1.4 표 #5 · #20) · `NCCL_NET_PLUGIN=spcx`(#4 · #19) · `NCCL_IB_GID_INDEX=3`(#15 · #29) · `NCCL_IB_QPS_PER_CONNECTION=4`(#9 · #24) · `NCCL_IB_MERGE_NICS=1`(#7 · #22) · `NCCL_CROSS_NIC=1`(#13 · #27) · `NCCL_DMABUF_ENABLE=0`(#12 · #26) · `NCCL_IB_SPLIT_DATA_ON_QPS=0`(#14 · #28) · `NCCL_SOCKET_IFNAME` · `NCCL_IB_HCA`(장치 이름은 자리표시) · `nccl:network=IB` · `nccl:version=2.30.7+cuda13.3`. `nccl:net_ib_devices` 만 '다름' 인데, 서브 쪽은 머리 캡처에서 장치 목록 전체(`[0]…:1/RoCE [1]…:1/RoCE`)가 · 메인 쪽은 꼬리 캡처에서 `[RO]; OOB …` 조각만 남았다 — 캡처 창이 다르고 Ray 가 접은 줄이라 메인에 장치가 없었다는 뜻이 아니다. 렌더러가 지금 내는 `.env.interconnect` 와 이 값의 대조는 하지 않았다 — 다르면 위 측정 당시 값이 정본이다.

## 1.6 재현 절차 — 기동 전 준비(사람이 할 일)
> 1.4 의 명령을 치기 **전에** 사람이 손으로 해야 하는 것이다 — 가중치 획득 · 스테이징 생성 · 네트워크가 필요한 단계. 에이전트가 대신 할 수 없는 자리(다운로드 승인 · 물리 경로 · 외부 계정)가 여기 모인다.

> 이 절이 답하는 질문: 이 셀을 기동하기 전에 사람이 준비해야 하는 것은 무엇이며, 각각을 어떤 명령으로 만들고, 그 절차의 출처는 어디인가?

### 가중치 획득

- 체크포인트: HF `nvidia/Qwen3.8-Flash-Next-NVFP4` · revision `fc694b54fb0174e0913e6adf86691ef85a4ead47`(00 §0.2 — 체크포인트 git HEAD) · safetensors 합 123.53 GiB(testlog_26102406 §근거 1).
- 획득 모드: 관리 NAS 상재(`model_source: managed` · 다운로드 불요 — plan_26090918 §2.1). 이 계보는 다운로드 명령을 실행한 적이 없다.
- 새 환경에서 받아야 한다면(추론 — 실행 검증 ✗ · 모델 획득은 사람의 승인 게이트다): `huggingface-cli download nvidia/Qwen3.8-Flash-Next-NVFP4 --revision fc694b54fb0174e0913e6adf86691ef85a4ead47 --local-dir <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4`. 받은 뒤 `model.safetensors.index.json` 의 `metadata.total_size` 로 크기를 확인한다(`du` ✗ — 02 §2.6).
- 노드마다: 두 노드 모두 같은 경로에서 읽어야 한다(1.5 선행조건 · compose 마운트 `${QUANT_MODEL_PATH}:/app/quant_models:ro`). 측정 기동에서 두 rank 모두 CIFS 위의 체크포인트를 읽었다(엔진 로그 `Filesystem type for checkpoints: CIFS` · lite_engine L638 · L652 — 엔진은 CIFS 를 인식된 네트워크 FS 가 아니라며 auto-prefetch 를 껐고(L639 · L653), 가중치 적재는 rank 당 450.67 s · 483.60 s 였다(L642 · L661 · 둘의 인과는 대조가 없다)).

### PLE · 기타 스테이징

해당 없음 — 이 셀은 PLE resident 다(01 §1.2 `runtime_patch` 안 실림 · 측정 기동 PLE `weight_device=cuda:0`). PLE 테이블을 로컬 디스크로 스테이징하는 절차는 mmap 셀(`q38fn31-nv4-mmap-1m-tp2` 등)의 것이다. JIT · autotune 캐시도 이 계보는 미리 만들지 않았다(측정 기동에서 `0 from previous config` · 02 Q7).

### 네트워크가 필요한 단계

- 베이스 이미지 pull: `nvcr.io/nvidia/pytorch:26.07-py3`(Dockerfile.source-build L37).
- vLLM 소스 클론: `git clone --filter=blob:none https://github.com/vllm-project/vllm.git` → `checkout --detach v0.31.0`(Dockerfile.source-build L76 ~ L78 · build-arg `VLLM_REPO` · `VLLM_REF`).
- pip: vLLM build-system requires · `ray==2.48.0`(L226 ~ L227) · apt `ccache libdw-dev`(L59) · `iproute2 netcat-openbsd …`(Dockerfile RUN 들).
- 벤치 클라이언트 이미지: `ghcr.io/vllm-project/guidellm:v0.7.3`(digest `sha256:e3ad2371bfa8e42f2c3d1251b62d0d9c9706c27ae2143c8c048eb5fc6aebb558` · 레벨 1 bench_tool JSON). GuideLLM 컨테이너 자체는 `HF_HUB_OFFLINE=1` 로 돌고 토크나이저를 호스트에서 마운트한다(run_bench.sh@29cbdbc89a3d L444 · L449).
- 가중치 다운로드(위 — 관리 NAS 면 불요).
- 오프라인 대체: 베이스 이미지 · vLLM 소스(v0.31.0 · 40자 SHA `db9527a46873454610df6dbedf79a36d6bf1a7f6`) · pip wheel · GuideLLM 이미지를 미리 받아 두면 빌드 · 측정은 외부 접속 없이 된다(추론 — 오프라인 빌드를 실행한 기록은 없다).
