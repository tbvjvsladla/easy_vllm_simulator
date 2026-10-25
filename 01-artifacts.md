# 01 · 산출물 — 무엇이 실제로 쓰였고, 어떻게 다시 띄우는가

## 1.1 적용 판정 표
> 기계가 적용 증거(빌드 원장 · 원장이 없으면 라벨된 재구성) × 파일 × 선언을 대사한 결과다. `artifacts/` 에는 **적용된 것만** 실렸다 — 빌드 때 스스로 skip 된 패치와 쓰이지 않은 평면의 파일은 표에만 남고 싣지 않는다.

<!-- FACT:slots -->
| 슬롯 | 실림 | 파일 | 신호 | 적용 증거 | 사유 |
|---|---|---|---|---|---|
| `build_patch_post` | 안 실림 | — | 1-signal | build-ledger:attestation:output/multi/benchlog/attestation_q38fn31-fp8-1m-tp2.json#main | 적용 판정 없음 — 원장 관측(기계 파생) |
| `build_patch_pre` | 안 실림 | — | 1-signal | build-ledger:attestation:output/multi/benchlog/attestation_q38fn31-fp8-1m-tp2.json#main | 적용 판정 없음 — 원장 관측(기계 파생) |
| `build_recipe` | 실림 | `artifacts/build_recipe/Dockerfile.source-build` · `artifacts/build_recipe/requirements.txt` | 3-signal | build-ledger:build-ledger.dockerfile | 이미지 `easy-vllm:0.31.0-cu133-aarch64-source` 는 이 셀을 위해 다시 짓지 않았다(S1 슬롯 정정 · `plan_26102420_…` 정정 기록 "이미지 재빌드가 필요 없어"). 이 이미지는 2026-10-14 vLLM 0.31.0 bump 때 single-node 체크아웃에서 지어졌다 — 판정 testlog 는 빌드 입력을 "single-node 체크아웃(`689610f` + 미커밋 렌더)" 이라 적는다(`testlog_26101412_v031_14B이하_4종_양노드_상주서빙_판정` 유효맥락 · `devlog_26101412_v031_bump_4종_양노드_상주서빙_서사` — 둘 다 계보 읽기 목록 밖). 그 렌더 중 `requirements.txt` 는 빌드 원장 `req… |
| `compose` | 실림 | `artifacts/compose/.env.cluster.template` · `artifacts/compose/.env.interconnect.template` · `artifacts/compose/.env.template` · `artifacts/compose/arm_patch.sh` · `artifacts/compose/docker-compose.yaml` · `artifacts/compose/serve_runner.sh` · `artifacts/compose/sub_recipe.json` | 2-signal | file:output/multi/docker-compose.yaml | 멀티 공통 서빙 경로이며 이 모델 전용이 아니다. 이 셀에 걸린 것은 셋이다. - ① compose 가 `${PLE_MMAP_HOST_PATH}:/app/ple_mmap:ro` 볼륨과 `VLLM_PLE_MMAP` · `VLLM_PLE_MMAP_DIR` 를 master · slave 양쪽 environment 로 넘긴다. **`VLLM_PLE_MMAP_DIR` 의 compose 기본값은 `/app/ple_mmap/q38fn-nvfp4`** 다. 그래서 이 FP8 셀은 셀 env 로 `/app/ple_mmap/q38fn-fp8` 을 반드시 준다(02 §2.6). - ② `arm_patch.sh` 가 `RUNTIME_PATCH_STEM` 을 `CONFIG_FILE` 보다 먼저 읽는다. slave 는 `CONFI… |
| `fork_pin` | 안 실림 | — | 1-signal | none:.claude/policies/arch_variant_ledger.json | 셀 env 에 VARIANT 줄 없음 = stock(workflow.md §변종 좌표의 거처)(기계 파생) |
| `runtime_patch` | 실림 | `artifacts/runtime_patch/q38fn31-fp8-1m-tp2_patch.py` | 3-signal | none:output/multi/configs/q38fn31-fp8-1m-tp2_patch.py | (a) **무엇을 고치나** — Qwen4Exp PLE n-gram 테이블을 디스크에서 mmap 으로 읽는 out-of-tree vLLM 플러그인 `qwen_ple_mmap_plugin` 0.1.0 의 번들이다. 파일 자체는 두 부분이다. 하나는 stdlib 만 쓰는 짧은 로더(머리 주석 5줄 · `_arm()`)이고, 다른 하나는 zlib+base64 로 압축한 tar 한 덩어리(`_B`)다. 로더는 번들을 `/tmp/qwen_ple_mmap_plugin_dist_957f66252c` 에 풀어 `sys.path` 에 얹는다. 그러면 vLLM 이 `load_general_plugins()` 에서 entry point `qwen_ple_mmap = qwen_ple_mmap_plugin:register` 를… |
| `triplet` | 실림 | `artifacts/triplet/.env.q38fn31-fp8-1m-tp2` · `artifacts/triplet/q38fn31-fp8-1m-tp2.sh` · `artifacts/triplet/q38fn31-fp8-1m-tp2.yaml` | 3-signal | file:output/multi/configs/q38fn31-fp8-1m-tp2.yaml | 서빙 노브(1.3) · 러너 · 셀 env 다. 이 셀 고유의 줄은 둘로 나뉜다. - 셀 env 의 플러그인 활성 네 줄: `VLLM_PLE_MMAP=1` · `VLLM_PLE_MMAP_DIR=/app/ple_mmap/q38fn-fp8` · `RUNTIME_PATCH_STEM=q38fn31-fp8-1m-tp2` · `VLLM_PLE_MMAP_VERIFY=16`. 마지막 줄은 디버그 대조 노브이고, rank 마다 처음 16회 조회에서만 추가 수집을 한다. - yaml 의 `hf-overrides`: `max_position_embeddings` 를 함께 실은 YaRN factor 4 다(W1 · W4). |

**레시피 경고 · 실린 리비전**(기계 관측 — 수신자 빌드가 여기서 갈라질 수 있다)

- ⚠ `build_recipe` `output/single/Dockerfile.source-build` — 실린 바이트(git:12ef50a89b86cb30d4c5b87c32506c5563ff4655)가 측정 이미지 docker history 의 RUN·COPY 와 전수 일치하지 않는다(16/17)(이미지 Created 2026-10-14T01:45:19Z · 실린 개정 커밋 2026-10-14T03:07:26Z · 이 체크아웃 통로 `output/multi/Dockerfile.source-build` 의 파일이 아니다)
- `build_recipe` `output/single/Dockerfile.source-build`(이 체크아웃 통로 `output/multi/Dockerfile.source-build` 대신) 실린 리비전: 커밋 `12ef50a89b86` · 선택 방법 `git log --all -- output/*/Dockerfile.source-build · 12ef50a89b86:output/single/Dockerfile.source-build 가 docker history RUN·COPY 와 가장 잘 맞는 이력 개정(16/17 · 전수 일치 개정 없음 · 불일치 레시피에만: RUN echo "[guard] source-build key (26.07-py3 x vLLM 0.31.0): validated patch set ((ngc_base x vllm_version) keyed) — pr · 작업트리 15/18) — 이 체크아웃 통로(output/multi/Dockerfile.source-build) 밖 · 커밋 2026-10-14T03:07:26Z > 이미지 Created 2026-10-14T01:45:19Z(빌드 뒤 커밋 · 위 불일치 단계만큼 다르다) — 이 개정의 바이트를 싣는다(git cat-file)` · docker history 대조 16/17 · 실린 바이트 `git:12ef50a89b86cb30d4c5b87c32506c5563ff4655` · 쓰인 바이트임을 관측으로 확인 아니오
- `build_recipe` `output/single/requirements.txt`(이 체크아웃 통로 `output/multi/requirements.txt` 대신) 실린 리비전: 커밋 `12ef50a89b86` · 선택 방법 `git log --all -- output/*/requirements.txt · 12ef50a89b86:output/single/requirements.txt · 빌드 원장 sha256 일치(작업트리·이미지 Created 이전 개정은 다르다) — 이 체크아웃 통로(output/multi/requirements.txt) 밖 · 커밋 2026-10-14T03:07:26Z > 이미지 Created 2026-10-14T01:45:19Z(빌드 뒤 커밋된 같은 바이트)` · docker history 대조 없음 · 실린 바이트 `git:12ef50a89b86cb30d4c5b87c32506c5563ff4655` · 쓰인 바이트임을 관측으로 확인 예
- `compose` `.claude/skills/upstream-version-watch/assets/configs/arm_patch.sh` 실린 리비전: 커밋 `d781f26e4ebc` · 선택 방법 `worktree(git log -1 --before=2026-10-25T01:51:17Z -- .claude/skills/upstream-version-watch/assets/configs/arm_patch.sh → d781f26e4ebc · 작업트리 mtime 2026-10-25T03:14:49Z > 측정(sweep_index.generated_utc)이지만 바이트가 그 개정과 같다 · 측정 뒤 커밋 0 · 작업트리 = HEAD)` · docker history 대조 없음 · 실린 바이트 `worktree` · 근거 `git-rev-before-measurement` · 쓰인 바이트임을 관측으로 확인 아니오
- `compose` `.claude/skills/upstream-version-watch/assets/configs/serve_runner.sh` 실린 리비전: 커밋 `50a5ce486738` · 선택 방법 `worktree(mtime 2026-09-06T21:57:53Z ≤ 측정 2026-10-25T01:51:17Z(sweep_index.generated_utc) · 바이트 = 측정 전 마지막 커밋 50a5ce486738)` · docker history 대조 없음 · 실린 바이트 `worktree` · 근거 `mtime≤measured` · 쓰인 바이트임을 관측으로 확인 예
- `compose` `output/multi/docker-compose.yaml` 실린 리비전: 커밋 `d781f26e4ebc` · 선택 방법 `worktree(git log -1 --before=2026-10-25T01:51:17Z -- output/multi/docker-compose.yaml → d781f26e4ebc · 작업트리 mtime 2026-10-25T03:14:49Z > 측정(sweep_index.generated_utc)이지만 바이트가 그 개정과 같다 · 측정 뒤 커밋 0 · 작업트리 = HEAD)` · docker history 대조 없음 · 실린 바이트 `worktree` · 근거 `git-rev-before-measurement` · 쓰인 바이트임을 관측으로 확인 아니오

**싣지 않은 파일**(슬롯별 사유 — 빌드 때 skip 된 패치는 아래 적용 집합 표에 있다)

- `build_patch_post` known-non-patch 1건: `.gitkeep`
- `build_patch_pre` known-non-patch 1건: `.gitkeep`

**파일별 검증 · 관련성**(`generated-unverified` = 실행 검증되지 않은 생성물 — 파일 첫 줄 경고 · PAYLOAD 파일 기록과 같은 표시 · 관련성 = ① 패치 선언 대상 × 모델 config ② 엔진 로그 발화 서명 — 둘 다 관측일 때만 required/inactive-inferred)

| 슬롯 | 파일 | 검증 | 검증 근거 | 관련성 | 관련성 근거 |
|---|---|---|---|---|---|
| `build_recipe` | `artifacts/build_recipe/Dockerfile.source-build` | `generated-unverified` · renderer | 실린 Dockerfile.source-build(git:12ef50a89b86cb30d4c5b87c32506c5563ff4655)이 측정 이미지 docker history RUN·COPY 와 16/17 만 일치 — 이 이미지를 지은 바이트임을 관측으로 확인하지 못했다(_select_dockerfile · git log --all -- output/*/Dockerfile.source-build · 12ef50a89b86:output/single/Dockerfile.source-build 가 docker history RUN·COPY 와 가장 잘 맞는 이력 개정(16/17 · 전수 일치 개정 없음 · 불일치 레시피에만: RUN echo "[guard] source-build key (26.07-py3 x vLLM 0.31.0): validated patch set ((ngc_base x vllm_version) keyed) — pr · 작업트리 15/18) — 이 체크아웃 통로(output/multi/Dockerfile.source-build) 밖 · 커밋 2026-10-14T03:07:26Z > 이미지 Created 2026-10-14T01:45:19Z(빌드 뒤 커밋 · 위 불일치 단계만큼 다르다) — 이 개정의 바이트를 싣는다(git cat-file)) | `unknown` | 이 셀은 이 경로로 실행되지 않았다(generated-unverified · 생성 renderer) — 실행 기록이 없어 필요 여부를 판정하지 않는다(참고 형상) |
| `build_recipe` | `artifacts/build_recipe/requirements.txt` | `verified` · cell-run | 쓰인 레시피(build-ledger.dockerfile) — 개정 선택 slots.build_recipe.evidence.selected_revisions | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/.env.cluster.template` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/.env.interconnect.template` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/.env.template` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/arm_patch.sh` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/docker-compose.yaml` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/serve_runner.sh` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/sub_recipe.json` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `runtime_patch` | `artifacts/runtime_patch/q38fn31-fp8-1m-tp2_patch.py` | `verified` · cell-run | 셀 configs 의 런타임 패치(arming 관측 — 측정 기동 로그 output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/lite_engine_head_q38fn31-fp8-1m-tp2.log:34,55 (`[arm_patch] runtime patch armed → /app/configs/q38fn31-fp8-1m-tp2_patch.py` = .pth 무장 · 패치 효과 자체는 별도 관측)) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/.env.q38fn31-fp8-1m-tp2` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/q38fn31-fp8-1m-tp2.sh` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/q38fn31-fp8-1m-tp2.yaml` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
<!-- /FACT:slots -->

<!-- FACT:applied_set -->
**적용 집합** — 상태 `observed` · 출처 `attestation:output/multi/benchlog/attestation_q38fn31-fp8-1m-tp2.json#main`

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
| attestation | observed | output/multi/benchlog/attestation_q38fn31-fp8-1m-tp2.json |

> **attestation 범위** — config `q38fn31-fp8-1m-tp2` · phase serve · 작성 2026-10-25T01:39:26Z(output/multi/benchlog/attestation_q38fn31-fp8-1m-tp2.json 파일 mtime(파일시스템 관측 — 문서에 시각 칸이 없다 · 기록 필드 아님)) · 묶은 근거 cell-name · 파일 `output/multi/benchlog/attestation_q38fn31-fp8-1m-tp2.json` · 측정 창 대비 작성 2026-10-25T01:39:26Z < 측정 시작 2026-10-25T01:39:47Z — 측정 전(빌드·스모크)의 관측: **same config(q38fn31-fp8-1m-tp2) · serve — 원장 순서로 판별한 같은 기동의 관측(스모크 로그 미보존 · 약한 판별)(판별: 원장 순서로 판별(스모크 로그 미보존 — docs/simlog/**/*serve*.log 에 이 attestation 을 적은 스모크 로그 없음): 원장 docs/logs/main/events/2026-10.jsonl:636 budget_declare(smoke-q38fn31-fp8-1m-tp2 · 2026-10-25T01:23:44Z) ≤ attestation 작성 2026-10-25T01:39:26Z · serve_proof 작성 2026-10-25T01:39:26Z < 측정 시작 2026-10-25T01:39:47Z · 그 선언이 측정 끝 2026-10-25T01:51:17Z 까지 재선언 · 창 닫힘 없이 이어짐(main 원장 기록 범위 안) · API 서버 기동 2026-10-25T01:38:26Z(output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/lite_engine_q38fn31-fp8-1m-tp2.log:743 · 컨테이너 시계 = UTC 가정) ∈ [선언, attestation 작성] · 측정 엔진 로그 APIServer pid 793 하나)**
<!-- /FACT:applied_set -->

## 1.2 슬롯별 적용 사유
> 이 절이 답하는 질문: 실린 산출물 하나하나는 무엇을 고치며, 왜 이 셀에 필요했고, 이 셀에서 실제로 발화했으며, 언제 불필요해지는가?

### runtime_patch — `q38fn31-fp8-1m-tp2_patch.py`(이 셀의 핵심 · +1 런타임 패치 슬롯 · owner `vllm-recipe-explorer`)

(a) **무엇을 고치나** — Qwen4Exp PLE n-gram 테이블을 디스크에서 mmap 으로 읽는 out-of-tree vLLM 플러그인 `qwen_ple_mmap_plugin` 0.1.0 의 번들이다. 파일 자체는 두 부분이다. 하나는 stdlib 만 쓰는 짧은 로더(머리 주석 5줄 · `_arm()`)이고, 다른 하나는 zlib+base64 로 압축한 tar 한 덩어리(`_B`)다. 로더는 번들을 `/tmp/qwen_ple_mmap_plugin_dist_957f66252c` 에 풀어 `sys.path` 에 얹는다. 그러면 vLLM 이 `load_general_plugins()` 에서 entry point `qwen_ple_mmap = qwen_ple_mmap_plugin:register` 를 부른다. `register()` 는 `VLLM_PLE_MMAP` 이 1 · true · yes 일 때만 `backend.install()` 을 부른다. `install()` 은 `vllm.models.qwen4_exp.nvidia.ngram_embedding` 모듈의 전역 `Qwen4ExpPLEDeviceEmbedding` · `Qwen4ExpPLEPinnedHostEmbedding` 을 둘 다 `Qwen4ExpPLEMmapEmbedding` 으로 바꾼다. 둘 다 바꾸므로 `engram-config.cpu_offload` 값과 무관해진다(번들 주석). 새 클래스가 하는 일은 넷이다.
- 0행 placeholder 가중치를 둔다.
- 체크포인트 샤드 적재(`checkpoint_start` 가 있는 `weight_loader` 호출)를 no-op 로 버린다.
- forward 에서 이 rank 의 ETP 어휘 구간 행만 `MmapRowStore`(`table.py` · safetensors 헤더에서 샤드별 파일 · 오프셋을 찾는다 · 1파일 NVFP4 와 다파일 FP8 레이아웃을 모두 처리)로 모아 장치로 올린다.
- FP8 은 int8 뷰로 all-reduce 한다.

이식 출처는 업스트림 PR 이 아니다. 머리 주석은 provenance 를 `hand-written(자체 저작 · 참조 설계: blazux … 코드 비복사)` 라 적는다. 설계는 S1 판정 문서 §4 이고, 이전 vLLM 빌드 패치 62 의 `v0.31.0` 재저작이다(W6).

> [원문] q38fn31-fp8-1m-tp2_patch.py §L1-5
> # q38fn31-fp8-1m-tp2 런타임 패치 — Qwen4Exp PLE n-gram 테이블 mmap 백엔드(vLLM 0.31.x out-of-tree 플러그인 번들)
> # 목적: GB10 은 호스트 RAM = GPU 메모리라 stock Device/PinnedHost 임베딩이 47.7GiB 테이블을 전부 재료화한다 → 디스크(mmap)에서 필요한 행만 읽는 백엔드로 교체. 62-qwen4exp-ple-mmap(0.29) 의 0.31 재저작.
> # 활성: 셀 env VLLM_PLE_MMAP=1 일 때만(미설정 = stock 동일). 스테이징 경로 VLLM_PLE_MMAP_DIR. provenance=hand-written(자체 저작 · 참조 설계: blazux qwen3.8-Flash-DGX ple mmap, 코드 비복사) · 미검증 → 서빙 스모크가 유일한 중재자.
> # site-init safe: stdlib 만 사용, vllm/torch import ✗.
> # 동작: 번들을 /tmp 에 풀고 sys.path 에 얹는다 → vLLM 이 load_general_plugins() 로 entry point(qwen_ple_mmap)를 발견·register().

**번들에서 읽을 수 있는 소스 트리 꺼내기.** 패치 파일을 **실행하지 않고** `_B` 문자열만 AST 로 읽어 푼다. 저작 때 Python 3.12.3 으로, 빈 임시 디렉터리에서 패치 사본을 입력으로 실행해 아래 파일 목록을 확인했다. `filter="data"` 는 Python 3.12 또는 3.11.4 이상에서 쓸 수 있다.

```sh
mkdir -p ./ple_mmap_plugin_src && python3 -I - artifacts/runtime_patch/q38fn31-fp8-1m-tp2_patch.py ./ple_mmap_plugin_src <<'EOF_PY'
import ast, base64, io, sys, tarfile, zlib
src, dst = sys.argv[1], sys.argv[2]
tree = ast.parse(open(src, encoding="utf-8").read())
blob = next(n.value.value for n in tree.body
            if isinstance(n, ast.Assign) and getattr(n.targets[0], "id", "") == "_B")
tf = tarfile.open(fileobj=io.BytesIO(zlib.decompress(base64.b64decode(blob))))
tf.extractall(dst, filter="data")
print("\n".join(m.name for m in tf.getmembers()))
EOF_PY
```

출력(확인한 목록 6항목 · 디렉터리 1 · 메타 2 · Python 3): `qwen_ple_mmap_plugin-0.1.0.dist-info/{METADATA, entry_points.txt}` · `qwen_ple_mmap_plugin/{__init__.py, backend.py, table.py}`(줄 수 20 · 196 · 132). 이 zip 의 패치 사본과 원래 거처 `output/multi/configs/q38fn31-fp8-1m-tp2_patch.py` 는 저작 때 `cmp` 로 바이트가 같았다. 원래 거처는 비추적이라 휘발한다.

(b) **왜 필요했나** — 빼면 W5 가 그대로 다시 나온다. stock 임베딩이 PLE 를 재료화해 노드당 상주가 88,464 MiB 가 되고, 예산 선판정이 로드 0초에서 막는다. 이 패치만 넣고 `VLLM_PLE_MMAP` 을 켜지 않아도 같은 결과다(`register()` 가 아무것도 하지 않는다).

(c) **이 셀에서 발화했나** — 예. 무장과 활성을 측정 기동의 엔진 로그가 따로 보여 준다.
- 무장: master 컨테이너 머리 로그에 `[arm_patch] runtime patch armed → /app/configs/q38fn31-fp8-1m-tp2_patch.py` 가 두 줄 있다. 러너가 Ray 시작 전에 한 번(lite_engine_head L34), 모델 스크립트가 한 번 더(L55) 무장한다.
- 활성: `PLE mmap backend installed` 줄이 EngineCore(head L96), 서브 Ray 워커(head L109 · `ip=<node:sub>`), 메인 워커(lite_engine L402)에 있다. 서브 쪽 arm_patch 줄은 캡처 창에 없지만, 서브 워커가 플러그인을 설치했다는 줄이 서브 무장의 관측이다.
- 테이블: `128 shards in 33 files … dir=/app/ple_mmap/q38fn-fp8` 가 양 rank(L428 · L618)에 있다.
- 정확도 대조: `VLLM_PLE_MMAP_VERIFY=16`(셀 env) 이 켠다. 처음 16회 조회마다 마스크 없는 독립 수집과 마스크 + all-reduce 결과를 바이트로 대조한다(번들 `_verify`). 캡처 창에 `verify OK … 32768 rows identical` 3줄(L693 · L706 · L717)이 있고 FAILED 는 0줄이다.

이것은 임베딩 행 바이트의 대조다. 모델 출력이 상주 경로와 비트 단위로 같다는 확증은 아니다. 계보 경유 문서 `testlog_26102500_…`(읽기 목록 밖) §2 는 원시 pread 오라클과의 행 대조 불일치 0, 그리고 `prompt_logprobs` 기준 mmap-상주 차이가 상주 서버 자체의 반복 노이즈와 같은 크기라고 적는다. 그 측정은 이 셀의 측정 기동이 아니다.

(d) **언제 불필요해지나** — 대상 vLLM 이 디스크 기반(재료화하지 않는) PLE 백엔드를 정식 제공하거나, 노드 메모리가 PLE 상주를 덮을 때다(02 Q4 · R1). 트립와이어 성격의 종료 경로는 모두 **fail-loud** 다(번들 `backend.py` · `table.py`).
- eager 가 아니면 생성자에서 `RuntimeError` 가 난다.
- 스테이징 경로가 디렉터리가 아니어도, 샤드 텐서가 없어도, 샤드 폭이나 dtype 이 임베딩과 다르면 기동이 멈춘다.
- 조용히 stock 으로 떨어지는 경로는 없다. 다만 `VLLM_PLE_MMAP` 이 없으면 아무것도 하지 않는 것이 설계다.

무장 자체의 경로는 `arm_patch.sh` 다. 패치 파일이 있을 때만 `.pth` 를 쓰고, 없으면 아무것도 하지 않는다(빌드 · 기동을 멈추지 않는다).

> [원문] arm_patch.sh §L15-18
> # 패치 키: RUNTIME_PATCH_STEM(셀 env 가 선언 · slave 로도 전달) > CONFIG_FILE — slave 는 CONFIG_FILE=default 라 model-keyed 패치를 못 찾던 침묵 누락 교정(2026-10-24 Solar-Open2)
> _cfg="${RUNTIME_PATCH_STEM:-${CONFIG_FILE:-default}}"
> _patch="/app/configs/${_cfg}_patch.py"
> if [ -f "$_patch" ]; then

### compose — `docker-compose.yaml` · `serve_runner.sh` · `arm_patch.sh` · env 형상 3종 · `sub_recipe.json`

멀티 공통 서빙 경로이며 이 모델 전용이 아니다. 이 셀에 걸린 것은 셋이다.
- ① compose 가 `${PLE_MMAP_HOST_PATH}:/app/ple_mmap:ro` 볼륨과 `VLLM_PLE_MMAP` · `VLLM_PLE_MMAP_DIR` 를 master · slave 양쪽 environment 로 넘긴다. **`VLLM_PLE_MMAP_DIR` 의 compose 기본값은 `/app/ple_mmap/q38fn-nvfp4`** 다. 그래서 이 FP8 셀은 셀 env 로 `/app/ple_mmap/q38fn-fp8` 을 반드시 준다(02 §2.6).
- ② `arm_patch.sh` 가 `RUNTIME_PATCH_STEM` 을 `CONFIG_FILE` 보다 먼저 읽는다. slave 는 `CONFIG_FILE=default` 라 이 키가 없으면 패치를 못 찾는다. 이 배선은 Solar 셀에서 드러난 침묵 누락의 교정(커밋 d781f26)이고, Qwen PLE-mmap stem 2종의 서브 배달 허용목록 등재가 0daa0d0 이다(커밋 메시지 · `plan_26102420_…` 정정 기록의 하네스 수정 커밋 목록 · 배선 일반은 `testlog_26102418_4모델_…` §하네스 교정 3). 이 셀에서 그 배선 없이 slave 가 실패한 기록은 계보 안 문서에 없다. 배선은 Solar 실패 뒤에 미리 들어갔다.
- ③ `serve_runner.sh` 의 master 는 `ray status` 의 GPU 합류가 리터럴 `2` 가 될 때까지 기다린다(L79 · 1.5 `runner_gpu_join_literal`). 노드당 GPU · 노드 수가 다른 클러스터에서는 이 줄을 고친다.

측정 당시 개정은 1.1 '실린 리비전' 의 커밋이다. 폐기 조건: 런타임 패치가 필요 없는 셀에서는 `RUNTIME_PATCH_STEM` 을 비우면 종전 동작이다(compose 주석).

### triplet — `q38fn31-fp8-1m-tp2.yaml` · `.sh` · `.env.q38fn31-fp8-1m-tp2`

서빙 노브(1.3) · 러너 · 셀 env 다. 이 셀 고유의 줄은 둘로 나뉜다.
- 셀 env 의 플러그인 활성 네 줄: `VLLM_PLE_MMAP=1` · `VLLM_PLE_MMAP_DIR=/app/ple_mmap/q38fn-fp8` · `RUNTIME_PATCH_STEM=q38fn31-fp8-1m-tp2` · `VLLM_PLE_MMAP_VERIFY=16`. 마지막 줄은 디버그 대조 노브이고, rank 마다 처음 16회 조회에서만 추가 수집을 한다.
- yaml 의 `hf-overrides`: `max_position_embeddings` 를 함께 실은 YaRN factor 4 다(W1 · W4).

yaml 머리 주석의 "참조(승계 아님): camp-26090918 nv4-bf-1m-res … memwatch TRIP" 은 캠페인 id 가 계보 판정 문서와 다르다. 그 TRIP 은 22셀 재수행 캠페인(`camp-26091110-qwen38fn-multi-r2`)의 셀 #16 으로 기록돼 있다(`testlog_26091121_…` 비-measured 표). 수치(KV 20 GiB 에서 TRIP)는 같다. yaml 의 `async-scheduling: false` 는 설정에 도달하지 않았다(M3). 러너 `.sh` 는 `arm_patch.sh` 를 한 번 더 source 한다(멱등 · arm_patch 머리 주석).

### build_recipe — `Dockerfile.source-build` · `requirements.txt`(공유 이미지 · 이 모델의 요구 아님)

이미지 `easy-vllm:0.31.0-cu133-aarch64-source` 는 이 셀을 위해 다시 짓지 않았다(S1 슬롯 정정 · `plan_26102420_…` 정정 기록 "이미지 재빌드가 필요 없어"). 이 이미지는 2026-10-14 vLLM 0.31.0 bump 때 single-node 체크아웃에서 지어졌다 — 판정 testlog 는 빌드 입력을 "single-node 체크아웃(`689610f` + 미커밋 렌더)" 이라 적는다(`testlog_26101412_v031_14B이하_4종_양노드_상주서빙_판정` 유효맥락 · `devlog_26101412_v031_bump_4종_양노드_상주서빙_서사` — 둘 다 계보 읽기 목록 밖). 그 렌더 중 `requirements.txt` 는 빌드 원장 `requirements_sha256` `849a0502…` 가 커밋 `12ef50a` 의 `output/single/requirements.txt` 와 같아 커밋으로 남았다(저작 때 `git show 12ef50a:output/single/requirements.txt | sha256sum` 재계산). 빌드에 쓰인 Dockerfile 바이트가 커밋으로 남았는지는 확인되지 않는다. 실린 두 파일은 이 multi 체크아웃 작업트리가 아니라 그 뒤 커밋 `12ef50a`(2026-10-14T03:07:26Z · 이미지 Created 01:45:19Z 보다 뒤)의 `output/single/` 바이트이고, docker history · 빌드 원장과 가장 잘 맞는 이력 개정으로 골랐다(1.1 실린 리비전). '이 이미지를 지은 개정' 이라고 단정하지 않는다.
- Dockerfile(`generated-unverified`): 측정 이미지 docker history RUN · COPY 와 16/17 이 일치한다. 불일치는 실린 파일에만 있는 가드 echo 한 단계(`RUN echo "[guard] source-build key (26.07-py3 x vLLM 0.31.0) …"` · L40)다. 그 커밋은 이미지 Created 보다 뒤이고 빌드 입력은 커밋되지 않은 렌더였으므로, 빌드에 쓰인 바이트 그대로라고는 확인되지 않는다. `ARG VLLM_REF` 기본값이 v0.31.0 이다(L46). 실린 Dockerfile 은 `use_existing_torch.py` 위치를 `tools/` 먼저, 루트 다음으로 찾고 둘 다 없으면 fail-loud 로 멈춘다(L80-84 · L82). `v0.31.0` 에서는 그 파일이 `tools/` 로 옮겨졌다 — 저작 때 측정 이미지를 시작하지 않는 컨테이너로 create · cp 해 보니 `/workspace/vllm-src/tools/use_existing_torch.py` 는 있고 루트 `use_existing_torch.py` 는 없었으며, 이미지 docker history 에도 이 분기 단계가 있다. 그래서 실린 Dockerfile 로는 이 단계에서 멈추지 않는다(실린 바이트로 실제 빌드는 하지 않음 — 추론 · 단계 판독). 반면 이 체크아웃의 작업트리 `output/multi/Dockerfile.source-build` 는 루트 경로만 부르는 줄(`RUN python3 use_existing_torch.py`)이라 `v0.31.0` 에서 `can't open file` 로 멈춘다 — 이 이미지를 처음 지을 때 메인 1차 빌드가 바로 그렇게 실패했다(`testlog_26101412_v031_14B이하_4종_양노드_상주서빙_판정` P4 · 계보 읽기 목록 밖). 작업트리 multi 파일로 짓지 말고 실린 파일을 쓴다.
- `requirements.txt`(`verified`): 빌드 원장 sha256(`849a0502…`)과 바이트가 일치한다 — 이 이미지 빌드가 COPY 한 의존 목록과 같은 바이트다(머리 주석은 v0.31.0 wheel Requires-Dist 기준이라 적는다 · L1-2).

**참고 형상**은 Dockerfile 쪽이다(16/17 · 가드 echo 한 단계 차이 · 1.1 경고). requirements 는 원장과 바이트 일치다. PLE mmap 은 이미지가 아니라 런타임 패치에 있다.

### 싣지 않은 슬롯
- `build_patch_pre` · `build_patch_post` — 측정 이미지의 빌드 원장(attestation)에 적용된 빌드 패치 파일이 없다. 원장이 기록한 것은 inline `strip-hoist` 하나이고 결과는 **skipped** 다(`register_opaque_type accepts hoist`). `10-…`~`92-…` 는 이 multi 체크아웃의 `output/multi/build_patches/` · `output/multi/build_patches_src/` 에 있는 파일이다. 이 이미지를 지은 통로는 `output/single/` 이고, 원장 `context_inventory` 는 그 통로의 두 슬롯 모두 `.gitkeep` 만 기록한다 — 그 패치들은 이미지를 지은 빌드 컨텍스트에 애초에 없었다. 이전 vLLM 의 PLE mmap 빌드 패치 62 는 parked 디렉터리에 있고 이 이미지에 들어가지 않았다(W6). 싣지 않은 것은 이 셀에 무엇이 필요했는지에 대한 판정이 아니다.
- `fork_pin` — 셀 env 에 `VARIANT` 줄이 없다. stock 빌드다.

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
근거: TP=1 은 산술 사전판정으로 배제(R3 · 기동 실패 관측 아님) — 노드 2 × GPU 1 manifest 형상에서 온 값이며 다른 TP 를 잰 기록 없음
출처: [testlog_26102412_q38fn_fp8_싱글_사전판정_불가_멀티이관 §근거, plan_26102412_4모델_멀티캠페인_Solar_Qwen_TP2_최대context §셀 (TP=2 이므로 노드 병렬 아님 · 순차)]
```

```hint-event
id: V2
kind: value-status
노브: distributed-executor-backend
값: ray
지위: inherited
근거: 멀티 서빙 경로(serve_runner 가 Ray head/worker 를 띄운다)의 공통값 — 다른 백엔드 시도 0
출처: [plan_26090918_qwen38_flashnext_multi_딥캠페인 §1. 목표, serve_runner.sh]
```

```hint-event
id: V3
kind: value-status
노브: gpu-memory-utilization
값: 0.80
지위: inherited
근거: 이 셀에서 바꿔 잰 기록 없음 · kv-cache-memory-bytes 를 주므로 엔진이 KV 산정에 gmu 를 따르지 않는다고 스스로 적는다(lite_engine L694)
출처: [testlog_26102414_q38fn_nv4_멀티_TP2_1M_YaRN_기동_needle_lite_판정 §1. 기동과 교정, output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/lite_engine_q38fn31-fp8-1m-tp2.log]
```

```hint-event
id: V4
kind: value-status
노브: max-model-len
값: 1048576
지위: inherited
근거: 사람이 정한 목표(지원 최대치 기동)이자 이전 vLLM 1m 축의 값 — 이 셀에서 길이를 바꿔 잰 기록 없음 · 1M needle 통과는 정정 기록(plan_26102420)이 적는다
출처: [plan_26102401_4모델_싱글멀티_최대context_양자화_기동검증 §사용자 결정 (2026-10-24 인터뷰), plan_26102420_PLE_mmap_재이식_full벤치_hint_Solar_슬롯화 §정정·진행 기록 (2026-10-25 KST · S2~S4 완료)]
```

```hint-event
id: V5
kind: value-status
노브: kv-cache-dtype
값: fp8_e4m3
지위: inherited
근거: 이전 vLLM 22셀 재수행에서 bf16 KV 와 함께 잰 축이나(MTP on · vLLM bench 조건) 이 셀에서는 바꿔 잰 적 없음 — 같은 캠페인 NVFP4 셀과 같은 값
출처: [testlog_26091121_qwen38fn_22셀재수행_캠페인완주_판정 §판정 결과 — measured 4건, testlog_26102414_q38fn_nv4_멀티_TP2_1M_YaRN_기동_needle_lite_판정 §1. 기동과 교정]
```

```hint-event
id: V6
kind: value-status
노브: kv-cache-memory-bytes
값: 15032385536
지위: inherited
근거: 14 GiB 절대 클램프 — 엔진 2,112,392 토큰 = 1,048,576 요청 기준 2.01배(lite_engine L695) · 이전 vLLM 20 GiB 에서 상주 NVFP4 1M TRIP 이 있어 낮춘 값(yaml 주석) · 이 셀에서 바꿔 잰 기록 없음
출처: [testlog_26102414_q38fn_nv4_멀티_TP2_1M_YaRN_기동_needle_lite_판정 §1. 기동과 교정, output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/lite_engine_q38fn31-fp8-1m-tp2.log]
```

```hint-event
id: V7
kind: value-status
노브: enforce-eager
값: true
지위: inherited
근거: 이 셀에서 cudagraph 시도 0 · 플러그인 v1 이 eager 가 아니면 생성자에서 RuntimeError 로 막는다(번들 backend.py · 실기동 관측 없음 — 그래서 필요조건 관측은 아니다)
출처: [q38fn31-fp8-1m-tp2_patch.py, testlog_26102420_PLE_mmap_0.31.0_재이식_S1_선판정_런타임패치_슬롯_정정 §4. 재저작 설계 (S2 에서 구현 · 이 문서는 판정만)]
```

```hint-event
id: V8
kind: value-status
노브: async-scheduling
값: false
지위: inherited
근거: 이전 vLLM 스모크 yaml 에서 승계 · 이 셀 측정 기동 non-default args 에 실리지 않아 효과 없음(M3 · lite_engine_head L64)
출처: [testlog_26091001_p0-2_ple_mmap_p0-3_qsa_fp8kv_스모크_판정 §§1. ★ 중재 술어 재설계 — "토큰 정확 일치"는 이 스택에서 도달 불가, output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/lite_engine_head_q38fn31-fp8-1m-tp2.log]
```

```hint-event
id: V9
kind: value-status
노브: max-num-seqs
값: 1
지위: inherited
근거: 동시 시퀀스 1 — 같은 캠페인 NVFP4 셀과 같은 값 · 다른 값을 잰 기록 없음(측정도 동시성 1 레벨뿐)
출처: [testlog_26102414_q38fn_nv4_멀티_TP2_1M_YaRN_기동_needle_lite_판정 §1. 기동과 교정]
```

```hint-event
id: V10
kind: value-status
노브: max-num-batched-tokens
값: 2048
지위: inherited
근거: 이 셀 계보에서 값을 바꿔 잰 기록 없음(손으로 둔 값 · 근거 문서 없음)
출처: [q38fn31-fp8-1m-tp2.yaml]
```

```hint-event
id: V11
kind: value-status
노브: engram-config
값: '{"cpu_offload": false}'
지위: inherited
근거: stock 상주 형상에서 쓰던 값(PinnedHost 는 GB10 에서 절감 0 · R2) · VLLM_PLE_MMAP=1 이면 플러그인이 Device · PinnedHost 를 둘 다 교체해 이 값과 무관해진다(번들 주석)
출처: [testlog_26102420_PLE_mmap_0.31.0_재이식_S1_선판정_런타임패치_슬롯_정정 §3. stock 0.31.0 의 두 백엔드가 GB10 에서 왜 상주를 못 줄이나, q38fn31-fp8-1m-tp2_patch.py]
```

```hint-event
id: V12
kind: value-status
노브: hf-overrides
값: '{"text_config":{"max_position_embeddings":1048576,"rope_parameters":{"mrope_interleaved":true,"mrope_section":[11,11,10],"partial_rotary_factor":0.25,"rope_theta":10000000,"rope_type":"yarn","factor":4,"original_max_position_embeddings":262144}}}'
지위: declared-requirement
근거: max_position_embeddings 를 빼면 v0.31.0 에서 1M 이 ValidationError(W4 · 같은 캠페인 NVFP4 1M 셀) · rope 오버라이드가 없으면 YaRN 확장 길이가 ValidationError(W1 · 이전 vLLM NVFP4 512K 셀 1개 관측) — 두 실패 모두 이 FP8 셀이 아니라 같은 모델의 다른 양자화 셀에서 관측(FP8 config 도 rope_type default · max_position_embeddings 262144 로 같은 형상)
출처: [testlog_26091009_qwen38fn_24셀_판정 §void 셀 (21) — 결함별 분류, testlog_26102414_q38fn_nv4_멀티_TP2_1M_YaRN_기동_needle_lite_판정 §1. 기동과 교정]
```

요약 — 복사하면 위험한 값은 셋이다.
- `kv-cache-memory-bytes`(V6): 1M 요청 하나를 2배로 덮게 이 HW 에서 고른 절대 바이트다. 노드 메모리가 다르면 엔진의 토큰당 KV 바이트에서 다시 잡는다.
- `enforce-eager`(V7): 이 플러그인을 쓰는 한 바꿀 수 없다.
- `async-scheduling`(V8): yaml 에 적어도 효과가 없다.

`hf-overrides`(V12)만 필요조건이다. 실패 관측이 같은 모델의 다른 양자화 셀에서 나왔으므로, 네 vLLM 버전의 길이 검증 규칙으로 다시 확인한다. 나머지는 모두 이 셀에서 한 번도 바꿔 잰 적 없는 승계값이다. "이 값으로 한 번 떴다" 는 뜻이지 최적이라는 뜻이 아니다. 노브 밖의 필요 조건은 셀 env 의 플러그인 활성 줄이다(`VLLM_PLE_MMAP=1` · FP8 스테이징 `VLLM_PLE_MMAP_DIR` · `RUNTIME_PATCH_STEM` — 빼면 W5).

## 1.4 재현 절차
> 셀 실행 기록에서 기계가 파생한 명령 순서와 단계별 소요다. 소요가 `미관측` 인 단계는 시각 기록이 없다는 뜻이다(0 이 아니다). 그 아래는 블랙박스 이벤트 원장에서 이 셀의 행을 기계가 뽑아 기동 시도로 묶은 것이고, 마지막은 측정한 실행의 엔진 로그가 echo 한 env 다(측정 당시 값의 정본).

<!-- FACT:reproduce -->
| 순서 | 단계 | 성공 판정 | 소요 | 관측 구간(UTC) | 소요 출처 | 명령 출처 |
|---|---|---|---|---|---|---|
| 1 | render | 러너 3종·.env·.env.cluster·.env.interconnect 생성(렌더러 fail-loud) | 미관측 | 미관측 | .claude/skills/upstream-version-watch/scripts/multinode_serve_smoke.sh 선행조건 안내 · .claude/skills/upstream-version-watch/scripts/render_dockerfile.py CLI | derived(렌더러 CLI) |
| 2 | build | 빌드 OK · 이미지 실재 | 층 창 38분 19초(docker history 첫 층 → 끝 층 CreatedAt · 캐시 재사용 층 포함 — 빌드 소요 아님) | 2026-10-14T01:07:00Z → 2026-10-14T01:45:19Z (layer-window) | docker history --no-trunc --format '{{json .}}' sha256:339737df6571ce00993e731ef064a8db66085beaa650e4aa63bdb24f414b2034 · 우리 Dockerfile 층 24개의 CreatedAt 첫 층 → 끝 층(캐시 재사용 층은 이전 빌드 시각 — 빌드 소요 아님) | reconstructed(build-ledger build_args + compose build) |
| 3 | serve | health 200 + 추론 1회(스모크 판정) | ≤ 16분 3초(상한) | 2026-10-25T01:23:44Z → 2026-10-25T01:39:47Z (upper) | docs/logs/main/events/2026-10.jsonl budget_declare(label smoke-q38fn31-fp8-1m-tp2) → 첫 측정 시각(output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/lite_raw_q38fn31-fp8-1m-tp2.json measured_utc(호스트 date -u · lite_bench.sh 가 cold 레그 직전에 찍는 측정 시작)) | derived(스모크 사용법) |
| 4 | bench | 리포트 발행(+PASS 면 인증서) | 11분 30초 | 2026-10-25T01:39:47Z → 2026-10-25T01:51:17Z (exact) | output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/lite_raw_q38fn31-fp8-1m-tp2.json measured_utc(호스트 date -u · lite_bench.sh 가 cold 레그 직전에 찍는 측정 시작) → sweep_index.generated_utc(호스트 UTC · 두 끝 같은 시계) | reconstructed(bench json) |

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
bash .claude/skills/upstream-version-watch/scripts/multinode_serve_smoke.sh q38fn31-fp8-1m-tp2 --keep-up
```

**4. bench**

```sh
# bench JSON 필드로 복원: backend · model_id→--model · tokenizer_id→--tokenizer · num_prompts · max_concurrency · request_rate · burstiness · 요청당 입출력 길이 = total_*_tokens ÷ completed(나누어떨어질 때만 — 평균이다 · 요청별 길이는 JSON 에 없다) · 나누어떨어지면 `--dataset-name random` 으로 추정했다(측정 도구가 그 꼴로 부른다 — 아래 도구 원문).
# JSON 에 없는 인자(--endpoint · --base-url · --ignore-eos · --num-warmups · --temperature · --seed · --random-range-ratio · --trust-remote-code)는 측정 도구가 정했다 — level_* = run_bench.sh · lite_cold/lite_warm = lite_bench.sh(.claude/skills/adversarial-benchmark/scripts/) · 셀 스윕 순서는 sweep_bench.sh.
# output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/lite_cold_q38fn31-fp8-1m-tp2.json (date 20261025-014009) · ⚠ --random-input-len/--random-output-len = total ÷ completed 평균(chat 템플릿 포함 실측 토큰 · 측정 도구 원문 인자 미관측 — lite_bench.sh 원문 인자와 다를 수 있다)
vllm bench serve --backend openai-chat --model qwen3.8-flash-next --tokenizer /app/quant_models/Qwen/Qwen3.8-Flash-Next-FP8 --dataset-name random --random-input-len 566 --random-output-len 128 --num-prompts 1 --max-concurrency 1 --request-rate inf
# output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/lite_warm_q38fn31-fp8-1m-tp2.json (date 20261025-014043) · ⚠ --random-input-len/--random-output-len = total ÷ completed 평균(chat 템플릿 포함 실측 토큰 · 측정 도구 원문 인자 미관측 — lite_bench.sh 원문 인자와 다를 수 있다)
vllm bench serve --backend openai-chat --model qwen3.8-flash-next --tokenizer /app/quant_models/Qwen/Qwen3.8-Flash-Next-FP8 --dataset-name random --random-input-len 565 --random-output-len 128 --num-prompts 3 --max-concurrency 1 --request-rate inf
```

**슬롯 → 빌드 컨텍스트 매핑**(빌드 컨텍스트 = `output/<토폴로지>/` · 원천 = 실린 Dockerfile 의 COPY · compose 의 env_file · volumes · command — `—` 는 빌드 컨텍스트 입력이 아니다) — ⚠ 빌드 레시피 · 빌드 패치 행은 다른 컨텍스트 `output/single/`(이 이미지의 레시피 통로 · 2단계 docker build 와 같은 자리 · 자리 칸에 그 뿌리를 붙였다)

| zip 경로 | 빌드 컨텍스트 자리 | 근거 |
|---|---|---|
| `artifacts/triplet/.env.q38fn31-fp8-1m-tp2` | `envs/.env.q38fn31-fp8-1m-tp2` | compose env_file envs/.env.${CONFIG_FILE}(셀 env) |
| `artifacts/triplet/q38fn31-fp8-1m-tp2.sh` | `configs/q38fn31-fp8-1m-tp2.sh` | compose volumes ./configs:/app/configs |
| `artifacts/triplet/q38fn31-fp8-1m-tp2.yaml` | `configs/q38fn31-fp8-1m-tp2.yaml` | compose volumes ./configs:/app/configs |
| `artifacts/runtime_patch/q38fn31-fp8-1m-tp2_patch.py` | `configs/q38fn31-fp8-1m-tp2_patch.py` | compose volumes ./configs:/app/configs |
| `artifacts/build_recipe/Dockerfile.source-build` | `output/single/Dockerfile.source-build` | docker build -f output/single/Dockerfile.source-build · 컨텍스트 output/single(01 §1.4 2단계 — 이 이미지의 레시피 통로 · compose build 는 output/multi/ 의 Dockerfile 을 지어 동등 경로가 아니다) |
| `artifacts/build_recipe/requirements.txt` | `output/single/requirements.txt` | Dockerfile.source-build COPY requirements.txt |
| `artifacts/compose/.env.cluster.template` | `envs/.env.cluster` | compose env_file envs/.env.cluster |
| `artifacts/compose/.env.interconnect.template` | `envs/.env.interconnect` | compose env_file envs/.env.interconnect |
| `artifacts/compose/.env.template` | `.env` | compose 변수치환 프로젝트 .env(${…} 참조) |
| `artifacts/compose/arm_patch.sh` | `configs/arm_patch.sh` | compose volumes ./configs:/app/configs |
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

**기동 성공 판정(관측 · 발행 자격)**: health 200 = true · 추론 1회 = true · 근거 `output/multi/benchlog/serve_proof_q38fn31-fp8-1m-tp2.json` — 시점 범위: 작성 2026-10-25T01:39:26Z(output/multi/benchlog/serve_proof_q38fn31-fp8-1m-tp2.json 파일 mtime(파일시스템 관측 — 문서에 시각 칸이 없다 · 기록 필드 아님)) · 측정 창 대비 작성 2026-10-25T01:39:26Z < 측정 시작 2026-10-25T01:39:47Z — 측정 전(빌드·스모크)의 관측
<!-- /FACT:reproduce -->

<!-- FACT:event_timeline -->
**블랙박스 이벤트 29행**(원장에서 이 셀 label·config 에 맞는 행만 · 시각순 · 기계 발췌)

> **캠페인 경계 미관측** — 재생 모드(캠페인 밖 측정(측정 시점 ACTIVE=_bootstrap · 이 측정의 캠페인 인스턴스 없음)) — 캠페인 선언을 관측하지 못한다 · 경계 미관측 — 라벨(셀 이름)로 묶은 행 전부를 이 셀의 행으로 센다(같은 셀 이름의 앞선 캠페인 행이 섞였을 수 있다)

> **원장 관측 범위**(노드별 첫 행 → 마지막 행 · evidence.event_ledger_spans): main = 2026-07-31T03:27:43Z → 2026-10-25T03:13:55Z(`docs/logs/main/events/2026-07.jsonl` · `docs/logs/main/events/2026-08.jsonl` · `docs/logs/main/events/2026-09.jsonl` · `docs/logs/main/events/2026-10.jsonl` · `docs/logs/main/events/thermal.jsonl` · `docs/logs/main/events/watchdog.jsonl`) · sub = 2026-06-13T22:25:01Z → 2026-10-24T00:02:35Z(`sync_staging/sub_docs/logs/sub/events/2026-06.jsonl` · `sync_staging/sub_docs/logs/sub/events/2026-07.jsonl` · `sync_staging/sub_docs/logs/sub/events/2026-08.jsonl` · `sync_staging/sub_docs/logs/sub/events/2026-09.jsonl` · `sync_staging/sub_docs/logs/sub/events/2026-10.jsonl` · `sync_staging/sub_docs/logs/sub/events/thermal.jsonl` · `sync_staging/sub_docs/logs/sub/events/watchdog.jsonl` · 회수 미러가 가린 옛 로컬 사본 `docs/logs/sub/events/2026-09.jsonl`) — ⚠ sub(마지막 행 2026-10-24T00:02:35Z < 측정 끝 2026-10-25T01:51:17Z): 그 뒤 그 노드의 선언 · 사건은 **관측 범위 밖**이다(없음이 아니다 · 원장 미회수)

| # | UTC | 노드 | kind | label | 내용 | 출처 |
|---|---|---|---|---|---|---|
| 1 | 2026-10-24T05:53:28Z | main | `budget_blocked` | `smoke-q38fn31-fp8-1m-tp2` | budget_blocked · weights_mib=88464 · kv_mib=14336 · overhead_mib=24800 · mem_total_mib=124608 · reason=arm_ceiling_below_min · stage=preflight_ceiling · short_by_mib=14256 | docs/logs/main/events/2026-10.jsonl:520 |
| 2 | 2026-10-24T13:35:16Z | main | `budget_declare` | `smoke-q38fn31-fp8-1m-tp2` | 예산 선언(선언값 — 측정 아님) · floor_mib=21422 · weights_mib=64050 · kv_mib=14336 · overhead_mib=24800 · mem_total_mib=124608 · 기동 창 1/3: budget_renew 1회 · 닫힘 budget_clear 2026-10-24T14:05:23Z(선언 후 30m07s) · measured_utc 2026-10-25T01:51:17Z 는 이 창 밖(다른 기동 · 측정 전) | docs/logs/main/events/2026-10.jsonl:562 |
| 3 | 2026-10-24T13:35:17Z | main | `budget_honored` | `smoke-q38fn31-fp8-1m-tp2` | budget_honored · floor_mib=21422 · arm_ceiling_mib=18350 · remaining_s=7199 · 창 시작 2026-10-24T13:35:17Z 후 0m00s | docs/logs/main/events/watchdog.jsonl:784 |
| 4 | 2026-10-24T13:36:37Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=86193 · rate_mib_s=22707 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → smoke-q38fn31-fp8-1m-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-24T13:35:17Z 후 1m20s) | docs/logs/main/events/watchdog.jsonl:785 |
| 5 | 2026-10-24T13:50:24Z | main | `budget_renew` | `smoke-q38fn31-fp8-1m-tp2` | budget_renew · floor_mib=21422 · ttl_s=7200 · remaining_before_s=6292 · 창 시작 2026-10-24T13:35:16Z 후 15m08s | docs/logs/main/events/2026-10.jsonl:563 |
| 6 | 2026-10-24T14:05:23Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-q38fn31-fp8-1m-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-24T13:35:16Z 후 30m07s) | docs/logs/main/events/2026-10.jsonl:564 |
| 7 | 2026-10-24T14:05:23Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-q38fn31-fp8-1m-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-24T13:35:17Z 후 30m06s) | docs/logs/main/events/watchdog.jsonl:786 |
| 8 | 2026-10-24T19:48:28Z | main | `budget_declare` | `smoke-q38fn31-fp8-1m-tp2` | 예산 선언(선언값 — 측정 아님) · floor_mib=21422 · weights_mib=64050 · kv_mib=14336 · overhead_mib=24800 · mem_total_mib=124608 · 기동 창 2/3: budget_renew 1회 · 닫힘 budget_clear 2026-10-24T20:15:40Z(선언 후 27m12s) · measured_utc 2026-10-25T01:51:17Z 는 이 창 밖(다른 기동 · 측정 전) | docs/logs/main/events/2026-10.jsonl:582 |
| 9 | 2026-10-24T19:48:29Z | main | `budget_honored` | `smoke-q38fn31-fp8-1m-tp2` | budget_honored · floor_mib=21422 · arm_ceiling_mib=18350 · remaining_s=7199 · 창 시작 2026-10-24T19:48:29Z 후 0m00s | docs/logs/main/events/watchdog.jsonl:796 |
| 10 | 2026-10-24T19:49:48Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=85836 · rate_mib_s=21525 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → smoke-q38fn31-fp8-1m-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-24T19:48:29Z 후 1m19s) | docs/logs/main/events/watchdog.jsonl:797 |
| 11 | 2026-10-24T20:03:49Z | main | `budget_renew` | `smoke-q38fn31-fp8-1m-tp2` | budget_renew · floor_mib=21422 · ttl_s=7200 · remaining_before_s=6279 · 창 시작 2026-10-24T19:48:28Z 후 15m21s | docs/logs/main/events/2026-10.jsonl:583 |
| 12 | 2026-10-24T20:15:40Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-q38fn31-fp8-1m-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-24T19:48:28Z 후 27m12s) | docs/logs/main/events/2026-10.jsonl:584 |
| 13 | 2026-10-24T20:15:41Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-q38fn31-fp8-1m-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-24T19:48:29Z 후 27m12s) | docs/logs/main/events/watchdog.jsonl:798 |
| 14 | 2026-10-25T01:23:44Z | main | `budget_declare` | `smoke-q38fn31-fp8-1m-tp2` | 예산 선언(선언값 — 측정 아님) · floor_mib=21422 · weights_mib=64050 · kv_mib=14336 · overhead_mib=24800 · mem_total_mib=124608 · 기동 창 3/3: budget_renew 4회 · 닫힘 budget_clear 2026-10-25T01:51:43Z(선언 후 27m59s) · measured_utc 2026-10-25T01:51:17Z 가 이 창 안(이 측정의 기동) | docs/logs/main/events/2026-10.jsonl:636 |
| 15 | 2026-10-25T01:23:45Z | main | `budget_honored` | `smoke-q38fn31-fp8-1m-tp2` | budget_honored · floor_mib=21422 · arm_ceiling_mib=18350 · remaining_s=7199 · 창 시작 2026-10-25T01:23:45Z 후 0m00s | docs/logs/main/events/watchdog.jsonl:817 |
| 16 | 2026-10-25T01:24:59Z | cluster | `engine_log_window_start` | — | 캡처된 엔진 로그의 첫 시각(꼬리 캡처일 수 있다 — 엔진 시작 시각이 아닐 수 있다) · 선언 2026-10-25T01:23:44Z 후 1m15s · 시계: 컨테이너 시각을 UTC 로 읽어 측정 창 [2026-10-25T01:23:44Z, 2026-10-25T01:51:43Z] 안에 드는 것을 확인 | output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/lite_engine_q38fn31-fp8-1m-tp2.log:292 |
| 17 | 2026-10-25T01:25:04Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=80338 · rate_mib_s=24539 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → smoke-q38fn31-fp8-1m-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-25T01:23:45Z 후 1m19s) | docs/logs/main/events/watchdog.jsonl:818 |
| 18 | 2026-10-25T01:25:05Z | cluster | `engine_prefetch_disabled` | — | 가중치 auto-prefetch 꺼짐 줄 ×2 — filesystem CIFS(vLLM 로그 문구) | output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/lite_engine_q38fn31-fp8-1m-tp2.log:608 |
| 19 | 2026-10-25T01:37:02Z | cluster | `engine_weights_loaded` | — | 가중치 적재 완료 줄 ×2(rank·단계별) · 최장 716.60s · 이 행 = 마지막 줄 | output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/lite_engine_q38fn31-fp8-1m-tp2.log:677 |
| 20 | 2026-10-25T01:37:11Z | cluster | `engine_model_loaded` | — | 모델 적재 완료 줄 ×2(rank 별) · 마지막 줄 62.75 GiB · 729.7s | output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/lite_engine_q38fn31-fp8-1m-tp2.log:679 |
| 21 | 2026-10-25T01:37:38Z | cluster | `engine_kv_cache_sized` | — | GPU KV cache 2,112,392 tokens · 요청당 1,048,576 tokens 기준 최대 동시성 2.01x | output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/lite_engine_q38fn31-fp8-1m-tp2.log:695 |
| 22 | 2026-10-25T01:38:01Z | cluster | `engine_init_done` | — | 엔진 초기화(profile·KV 할당·warmup) 49.45s | output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/lite_engine_q38fn31-fp8-1m-tp2.log:718 |
| 23 | 2026-10-25T01:38:26Z | cluster | `server_starting` | — | API 서버 기동 — 이 측정 창 선언 2026-10-25T01:23:44Z 후 14m42s · 캡처 로그 첫 시각 2026-10-25T01:24:59Z 후 13m27s | output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/lite_engine_q38fn31-fp8-1m-tp2.log:743 |
| 24 | 2026-10-25T01:39:26Z | main | `budget_renew` | `smoke-q38fn31-fp8-1m-tp2` | budget_renew · floor_mib=21422 · ttl_s=7200 · remaining_before_s=6258 · 창 시작 2026-10-25T01:23:44Z 후 15m42s | docs/logs/main/events/2026-10.jsonl:637 |
| 25 | 2026-10-25T01:40:45Z | main | `budget_renew` | `smoke-q38fn31-fp8-1m-tp2` | budget_renew · floor_mib=21422 · ttl_s=7200 · remaining_before_s=7121 · 창 시작 2026-10-25T01:23:44Z 후 17m01s | docs/logs/main/events/2026-10.jsonl:638 |
| 26 | 2026-10-25T01:44:20Z | main | `budget_renew` | `smoke-q38fn31-fp8-1m-tp2` | budget_renew · floor_mib=21422 · ttl_s=7200 · remaining_before_s=6985 · 창 시작 2026-10-25T01:23:44Z 후 20m36s | docs/logs/main/events/2026-10.jsonl:639 |
| 27 | 2026-10-25T01:47:51Z | main | `budget_renew` | `smoke-q38fn31-fp8-1m-tp2` | budget_renew · floor_mib=21422 · ttl_s=7200 · remaining_before_s=6989 · 창 시작 2026-10-25T01:23:44Z 후 24m07s | docs/logs/main/events/2026-10.jsonl:640 |
| 28 | 2026-10-25T01:51:43Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-q38fn31-fp8-1m-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-25T01:23:44Z 후 27m59s) | docs/logs/main/events/2026-10.jsonl:641 |
| 29 | 2026-10-25T01:51:43Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-q38fn31-fp8-1m-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-25T01:23:45Z 후 27m58s) | docs/logs/main/events/watchdog.jsonl:819 |

**기동 시도**(예산 선언 `budget_declare` 마다 한 시도 · 같은 노드의 이어지는 행과 선언 없는 노드(엔진 로그)의 그 시각 행을 묶었다 · 사살 · 트립 · 거부는 닫힘과 따로 센다 · `measurement` 행은 측정 기록이라 시도로 세지 않는다(위 표에만) — 해석은 저작자 몫)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 1 | main | `smoke-q38fn31-fp8-1m-tp2` | —(선언 행 없음) | 없음 | `budget_blocked` 1 | —(닫힘 행 없음) | 0 |
| 2 | main | `smoke-q38fn31-fp8-1m-tp2` | 2026-10-24T13:35:16Z | 1회 · 첫 2026-10-24T13:50:24Z | 없음 | `budget_clear` 2026-10-24T14:05:23Z | 3 |
| 3 | main | `smoke-q38fn31-fp8-1m-tp2` | 2026-10-24T19:48:28Z | 1회 · 첫 2026-10-24T20:03:49Z | 없음 | `budget_clear` 2026-10-24T20:15:40Z | 3 |
| 4 | main | `smoke-q38fn31-fp8-1m-tp2` | 2026-10-25T01:23:44Z | 4회 · 첫 2026-10-25T01:39:26Z | 없음 | `budget_clear` 2026-10-25T01:51:43Z | 10 |
<!-- /FACT:event_timeline -->

<!-- FACT:measurement_env -->
**측정 실행의 env 관측 28행**(측정한 실행의 엔진 로그가 echo 한 값 · 기계 발췌 — 측정 당시 값의 정본이다. 표에 없는 키는 로그가 echo 하지 않은 것이지 설정되지 않았다는 뜻이 아니다 · 측정 뒤 재생성된 env 형상은 01 §1.1 을 본다)

| # | 키 | 값 | 노드 | 종류 | 출처(첫 줄) | 관측 |
|---|---|---|---|---|---|---|
| 1 | `NCCL_SOCKET_IFNAME` | `<nic:cluster>` | sub | env-echo | output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/lite_engine_head_q38fn31-fp8-1m-tp2.log:L115 | 로그 1개 |
| 2 | `NCCL_SOCKET_IFNAME` | `<nic:cluster>` | sub | param-echo | output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/lite_engine_head_q38fn31-fp8-1m-tp2.log:L116 | 로그 1개 |
| 3 | `nccl:version` | `2.30.7+cuda13.3` | sub | runtime | output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/lite_engine_head_q38fn31-fp8-1m-tp2.log:L117 | 로그 1개 |
| 4 | `NCCL_NET_PLUGIN` | `spcx` | sub | env-echo | output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/lite_engine_head_q38fn31-fp8-1m-tp2.log:L119 | 로그 1개 |
| 5 | `NCCL_IB_DISABLE` | `0` | sub | env-echo | output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/lite_engine_head_q38fn31-fp8-1m-tp2.log:L124 | 로그 1개 |
| 6 | `NCCL_IB_HCA` | `=<nic:cluster>,<nic:cluster>` | sub | param-echo | output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/lite_engine_head_q38fn31-fp8-1m-tp2.log:L126 | 로그 1개 |
| 7 | `NCCL_IB_MERGE_NICS` | `1` | sub | env-echo | output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/lite_engine_head_q38fn31-fp8-1m-tp2.log:L129 | 로그 1개 |
| 8 | `nccl:net_ib_devices` | `[0]<nic:cluster>:1/RoCE [1]<nic:cluster>:1/RoCE [RO]; OOB <nic:cluster>:<node:sub><0>` | sub | runtime | output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/lite_engine_head_q38fn31-fp8-1m-tp2.log:L132 | 로그 1개 |
| 9 | `NCCL_IB_QPS_PER_CONNECTION` | `4` | sub | env-echo | output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/lite_engine_head_q38fn31-fp8-1m-tp2.log:L145 | 로그 1개 |
| 10 | `nccl:net_ib_devices` | `[RO]; OOB <nic:cluster>:<node:sub><0>` | sub | runtime | output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/lite_engine_head_q38fn31-fp8-1m-tp2.log:L156 | 로그 1개 |
| 11 | `nccl:network` | `IB` | sub | runtime | output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/lite_engine_head_q38fn31-fp8-1m-tp2.log:L168 | 로그 2개 |
| 12 | `NCCL_DMABUF_ENABLE` | `0` | sub | env-echo | output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/lite_engine_head_q38fn31-fp8-1m-tp2.log:L170 | 로그 1개 |
| 13 | `NCCL_CROSS_NIC` | `1` | sub | env-echo | output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/lite_engine_head_q38fn31-fp8-1m-tp2.log:L174 | 로그 1개 |
| 14 | `NCCL_IB_SPLIT_DATA_ON_QPS` | `0` | sub | env-echo | output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/lite_engine_q38fn31-fp8-1m-tp2.log:L170 | 로그 1개 |
| 15 | `NCCL_IB_GID_INDEX` | `3` | sub | env-echo | output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/lite_engine_q38fn31-fp8-1m-tp2.log:L185 | 로그 1개 |
| 16 | `NCCL_SOCKET_IFNAME` | `<nic:cluster>` | main | env-echo | output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/lite_engine_q38fn31-fp8-1m-tp2.log:L434 | 로그 1개 · Ray 접힘 +3(접힌 사본의 노드 미관측) |
| 17 | `NCCL_SOCKET_IFNAME` | `<nic:cluster>` | main | param-echo | output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/lite_engine_q38fn31-fp8-1m-tp2.log:L435 | 로그 1개 · Ray 접힘 +2(접힌 사본의 노드 미관측) |
| 18 | `nccl:version` | `2.30.7+cuda13.3` | main | runtime | output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/lite_engine_q38fn31-fp8-1m-tp2.log:L436 | 로그 1개 |
| 19 | `NCCL_NET_PLUGIN` | `spcx` | main | env-echo | output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/lite_engine_q38fn31-fp8-1m-tp2.log:L438 | 로그 1개 |
| 20 | `NCCL_IB_DISABLE` | `0` | main | env-echo | output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/lite_engine_q38fn31-fp8-1m-tp2.log:L443 | 로그 1개 · Ray 접힘 +2(접힌 사본의 노드 미관측) |
| 21 | `NCCL_IB_HCA` | `=<nic:cluster>,<nic:cluster>` | main | param-echo | output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/lite_engine_q38fn31-fp8-1m-tp2.log:L444 | 로그 1개 · Ray 접힘 +2(접힌 사본의 노드 미관측) |
| 22 | `NCCL_IB_MERGE_NICS` | `1` | main | env-echo | output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/lite_engine_q38fn31-fp8-1m-tp2.log:L446 | 로그 1개 |
| 23 | `nccl:net_ib_devices` | `[RO]; OOB <nic:cluster>:<node:main><0>` | main | runtime | output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/lite_engine_q38fn31-fp8-1m-tp2.log:L448 | 로그 1개 · Ray 접힘 +2(접힌 사본의 노드 미관측) |
| 24 | `NCCL_IB_QPS_PER_CONNECTION` | `4` | main | env-echo | output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/lite_engine_q38fn31-fp8-1m-tp2.log:L458 | 로그 1개 · Ray 접힘 +3(접힌 사본의 노드 미관측) |
| 25 | `NCCL_DMABUF_ENABLE` | `0` | main | env-echo | output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/lite_engine_q38fn31-fp8-1m-tp2.log:L473 | 로그 1개 |
| 26 | `NCCL_CROSS_NIC` | `1` | main | env-echo | output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/lite_engine_q38fn31-fp8-1m-tp2.log:L477 | 로그 1개 |
| 27 | `NCCL_IB_SPLIT_DATA_ON_QPS` | `0` | main | env-echo | output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/lite_engine_q38fn31-fp8-1m-tp2.log:L588 | 로그 1개 |
| 28 | `NCCL_IB_GID_INDEX` | `3` | main | env-echo | output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/lite_engine_q38fn31-fp8-1m-tp2.log:L593 | 로그 1개 |

- 값 주: 기계 치환(pii.substitution_table · 장치·호스트·주소는 자리표시 — 원문은 출처 줄)

- Ray 접힘: 출처 줄 끝의 `[repeated Nx across cluster]` — Ray 가 다른 프로세스의 같은 줄 N개를 한 줄로 접었다. 노드 칸은 **첫 줄**의 노드이고, 접힌 사본이 어느 노드의 것인지는 로그에 없다(관측 대상 밖).

- 로그 창(꼬리): `tail-800(lite_bench.sh@d33b0878a6c3 L234)`(꼬리 창 출처 로그 1개 · 이 표 행 #14–28) — 이 캡처는 엔진 출력의 **마지막 N줄**이다(측정 도구의 `tail -N` 상한에 닿았다 · 생산자 evidence `log_window`). 창 앞에서 echo 한 줄은 그 로그에 없다 — 이 주의는 꼬리 창 출처 행에만 걸린다(머리 캡처 행은 아래).

- 로그 창(머리): `head-600(output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/lite_raw_q38fn31-fp8-1m-tp2.json engine_log_head_lines)`(머리 캡처 출처 로그 1개 · 이 표 행 #1–13) — 이 캡처는 엔진 출력의 **처음 N줄**이다(lite_bench.sh 머리 캡처 `head -n N` 상한에 닿았다 · 생산자 evidence `log_head_window`). 꼬리 창의 주의는 이 행에 걸리지 않는다 · 머리 N줄 뒤에서 echo 한 줄은 그 로그에 없다. 머리 캡처와 꼬리 창 사이(엔진 출력의 가운데)는 어느 캡처에도 없다 — 표에 없는 키 · 한 노드에만 있는 키는 그 사이였을 수 있다.
<!-- /FACT:measurement_env -->

> 이 절이 답하는 질문: 수신자가 이 zip 만으로 이 셀을 다시 띄우려면 어떤 순서로 무엇을 하고, 각 단계의 성공을 무엇으로 판정하며, 얼마나 기다려야 하는가?

**(a) 사전 준비.**
- 가중치: 양 노드의 같은 경로 `<manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-FP8` 에 체크포인트가 있어야 한다. 컨테이너 안에서는 `/app/quant_models/…` 다. 이 셀은 NAS 를 직접 읽었고, 엔진은 그 파일시스템을 CIFS 로 보고 auto-prefetch 를 껐다(lite_engine L608).
- PLE 스테이징: 각 노드 **로컬 NVMe** 의 같은 경로 `<manifest.ple_mmap_host_path>/q38fn-fp8/` 에 체크포인트 safetensors 33개를 둔다. 33개는 PLE 샤드 128개를 담은 파일들이다. 컨테이너 안에서는 `/app/ple_mmap/q38fn-fp8`(ro)이다. 파일 목록과 생성 방법은 1.6 에 있다. 저작 때 메인 노드 스테이징을 `du` 로 보니 49G 였고, 서브는 보지 않았다.
- 필수 env 의 자리: 셀 env(`artifacts/triplet/.env.q38fn31-fp8-1m-tp2` — `VLLM_PLE_MMAP=1` · `VLLM_PLE_MMAP_DIR` · `RUNTIME_PATCH_STEM` · `VLLM_PLE_MMAP_VERIFY`). 마운트 원천은 프로젝트 `.env` 형상(`artifacts/compose/.env.template` 의 `QUANT_MODEL_PATH` · `PLE_MMAP_HOST_PATH`)이다. 인터커넥트는 `.env.interconnect` 형상이다.
- 측정 당시 NCCL 값은 위 '측정 실행의 env 관측' 표가 정본이다. 다시 렌더하면 그 표와 맞는지 대조한다. 1.1 '싣지 않은 파일' 에 측정 뒤 재생성 형상 항목은 없다.

**(b) 빌드.** 이 셀은 이미지를 새로 짓지 않았다. 다른 셀과 공유하는 `easy-vllm:0.31.0-cu133-aarch64-source` 를 그대로 썼다(이미지 층 창 2026-10-14). 재현하려면 위 2단계 명령(`docker build -f output/single/Dockerfile.source-build … output/single` · 실린 두 파일을 `output/single/` 자리에 둔다)을 양 노드에서 각자 실행한다. 실린 Dockerfile 은 `COPY build_patches_src/ /tmp/build_patches_src/`(L165)와 `COPY build_patches/ /tmp/build_patches/`(L203)도 하므로, 컨텍스트에 그 두 디렉터리가 없으면 COPY 단계에서 멈춘다. 빌드 원장 `context_inventory` 는 두 슬롯 모두 `.gitkeep` 만 기록하므로 빈 디렉터리 두 개를 만든다(위 명령의 `mkdir -p output/single/build_patches_src output/single/build_patches`). 스모크 · compose build 는 multi 작업트리의 `output/multi/Dockerfile.source-build` 를 짓기 때문에 동등 경로가 아니다(위 명령 주석). 그 작업트리 파일은 `use_existing_torch.py` 를 루트에서만 찾아 `v0.31.0` 에서 멈춘다(1.2 build_recipe · 이 이미지 첫 빌드 1차 실패의 원인). 실린 Dockerfile 은 `tools/` 를 먼저 찾는다(L82). build-arg 는 docker history 에서 복원한 `VLLM_REF=v0.31.0` · `TORCH_CUDA_ARCH=12.1a` · `BUILD_JOBS=8` 등이다. 이미지 전송은 하지 않는다. 층 창 38분 19초는 빌드 소요가 아니다(캐시 층 포함). 실제 빌드 소요는 미관측이다.

**(c) 기동 순서와 성공 판정.** master 가 Ray head 를 띄우고, slave 가 join 하고, master 가 GPU 2 합류를 확인한 뒤 `vllm serve` 를 실행한다(1.5 기동 순서 · 머리 로그 L35 · L53). 측정 기동(시도 4)의 관측 타임라인은 다음과 같다. 엔진 로그 기준이고, 컨테이너 시계를 UTC 로 가정했다.
- 01:23:44Z 예산 선언(원장 행 14)
- 01:24:41 EngineCore 에서 플러그인 설치(head L96)
- 01:25:03 테이블 샤드 매핑(L428)
- 가중치 적재 완료: sub 01:32:17(430.34 s · L610), main 01:37:02(716.60 s · L677). 둘 다 `Model loading took 62.75 GiB`(L625 · L679)
- 01:37:38 KV 2,112,392 토큰(L695)
- 엔진 초기화 49.45 s(L718)
- 01:38:26 `Starting vLLM server`(L743 · 선언 후 14분 42초)
- 01:39:26 serve proof(health 200 + 추론 1회)

계보 경유 문서 `testlog_26102500_…`(읽기 목록 밖)는 S3 기동의 READY 를 845 s 로 적는다. 그것은 이 측정 기동과 다른 기동의 값이다.

성공 판정은 다음을 모두 본다.
- health 200 + 추론 1회
- 양 rank 의 `PLE mmap backend installed`
- `128 shards in 33 files … dir=/app/ple_mmap/q38fn-fp8` 의 dir 이 FP8 경로인지
- `verify OK` 가 있고 `verify FAILED` 가 없는지

**정상인데 무서워 보이는 줄**(이 측정 기동에서 관측 · 사살로 이어지지 않음):
- `Unknown vLLM environment variable detected: VLLM_PLE_MMAP*`(head L65-68). 플러그인 env 라 vLLM 이 모른다.
- 컨테이너 시작 배너 `The NVIDIA Driver was not detected`(head L30). 그 뒤 Ray 는 GPU 2 로 떴다.
- `SymmMemCommunicator: Device capability 12.1 not supported` · FlashInfer all-reduce · custom collectives 비활성(lite_engine L292-294 · L602-604).
- `Using default MoE config. Performance might be sub-optimal!`(L691 · L704).
- 가중치 적재 7~12분.
- 원장의 `watchdog_highrate_hold`. 세 mmap 기동 모두 선언 후 1분 20초 안팎에 1회씩 있었다.

기동 시도는 원장 표대로 4건이다. 시도 1 은 상주 형상의 `budget_blocked`(로드 0초 · W5)이고, 시도 2 · 3 은 사살 없이 정상 닫혔다. 시도 2 가 S3 기동이다(S3 lite raw `output/multi/benchlog/lite_raw_q38fn31-fp8-1m-tp2.json` measured_utc 2026-10-24T14:03:52Z 가 그 창 안). 1M needle 이 시도 2 · 3 중 어느 쪽이었는지는 시각 기록이 없어 미확인이다.

**(d) 호스트 메모리 예산 선언.** 원장 선언(측정 아님)은 `floor_mib=21422` · `weights_mib=64050` · `kv_mib=14336` · `overhead_mib=24800` · `mem_total_mib=124608` 이다. 워치독은 그것을 `arm_ceiling_mib=18350` 으로 수리했다(`budget_honored`). 이 기동 창에서 갱신은 4회였다(행 24~27 · 벤치 진입 `budget_renew` 포함). 엔진 실측은 노드당 모델 적재 62.75 GiB 였고, 측정 중 시스템 RAM 은 main 93.7 GiB(77%) · sub 89.9 GiB(74%)였다(bench_report lite 지표). 선언은 main 노드 원장에만 있다. sub 원장은 회수 미러(`sync_staging/sub_docs/logs/sub/events/`)에 2026-10-24T00:02Z 까지만 있어 이 기동 창의 sub 쪽 선언 여부는 관측 범위 밖이다.

**(e) 측정 명령.** 위 4단계 명령은 bench JSON 에서 재구성한 lite 레그(vllm bench serve)이고 실행 원문이 아니다. full 레벨은 GuideLLM 이 쟀다. 도구 원문 인자(입출력 1024/256 · 16요청 · warmup 비율 · `ignore_eos`)는 03 §3.4 의 측정 도구 스냅샷 발췌를 본다.

**(f) 엔진이 스스로 고른 런타임 경로.**
- 통신은 NCCL 2.30.7 `Using network IB` 이고, RoCE 장치 2개를 병합했다(`NCCL_IB_MERGE_NICS=1` · env 관측 표). `NCCL_NET_PLUGIN=spcx` 를 줬지만 엔진은 그 장치에서 SPCX 를 지원하지 않는다며 건너뛰었다(head L150 · L154). 서브 머리 로그의 GIN 경로 줄은 HCA 2개 모두 `GPU Direct RDMA Disabled` 로 찍혔다(head L177 · L179 · main 쪽 같은 줄은 캡처 창 밖).
- `enforce_eager` 라 torch.compile · CUDAGraph 가 꺼졌다(head L85).
- prefix caching 은 엔진 기본으로 켜졌다(head L97 `enable_prefix_caching=True`). 합성 데이터라 측정 중 적중은 0 이었다(level_01 `measured.json` `prefix_cache_run.hit_rate_run 0.0`).
- 샘플링 기본값은 체크포인트 `generation_config.json` 으로 덮였다(L740 · temperature 1.0 · top_k 20 · top_p 0.95).
- 측정 엔진 로그의 `non-default args` 에는 `async_scheduling` 이 없다(M3).

## 1.5 서브 레시피 해설
<!-- FACT:sub_recipe -->
원본: `artifacts/compose/sub_recipe.json`(기계 파생 — 스모크와 같은 함수 `slave_forward.derive` 의 출력).

| 역할 | 하는 일 |
|---|---|
| `master` | ray head + vllm serve(트리플렛 source · 모델 러너 실행) |
| `master_runtime_patch` | armed — RUNTIME_PATCH_STEM='q38fn31-fp8-1m-tp2' → /app/configs/q38fn31-fp8-1m-tp2_patch.py |
| `runner_gpu_join_literal` | {"lines": [79], "note": "master 는 ray status 의 GPU 합류가 이 리터럴과 같아질 때까지 기다린다 — 노드당 GPU·노드 수가 다른 클러스터에서는 이 줄을 고치지 않으면 대기에서 멈춘다(arch 의 <G>g<N>n 과 숨은 결합).", "value": "2"} |
| `runtime_patch_key_chain` | RUNTIME_PATCH_STEM · CONFIG_FILE · 'default' |
| `slave` | ray worker(트리플렛 미수령 — 같은 이미지·같은 compose·같은 serve_runner.sh 가 NODE_ROLE 로 분기) |
| `slave_config_file` | default |
| `slave_runtime_patch` | armed — slave RUNTIME_PATCH_STEM='q38fn31-fp8-1m-tp2'(slave_forward 전달) → /app/configs/q38fn31-fp8-1m-tp2_patch.py |
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
| `RUNTIME_PATCH_STEM` | serve_env | cell_env | 2026-10-24(Solar-Open2 자체 이식 플러그인): slave 는 CONFIG_FILE=default 라 model-keyed 런타임 패치를 arming 못 했다 — Ray 워커가 모델 클래스를 import 하는 모델은 slave 가 패치를 먹어야 뜬다. 패치 stem 만 별도 키로 전달(비면 생략 = 종전 동작). |
| `VLLM_PLE_MMAP` | serve_env | cell_env | 2026-09-09(62-qwen4exp-ple-mmap): Ray 워커도 가중치를 올린다 — 마스터만 켜면 slave 는 상주 로드로 OOM/불일치. |
| `VLLM_PLE_MMAP_DIR` | serve_env | cell_env | 2026-09-09: 워커 로컬 mmap 스테이징 경로 — 마스터와 같은 값이어야 한다. |

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
| `source` | output/multi/benchlog/attestation_q38fn31-fp8-1m-tp2.json |

> **attestation 범위** — config `q38fn31-fp8-1m-tp2` · phase serve · 작성 2026-10-25T01:39:26Z(output/multi/benchlog/attestation_q38fn31-fp8-1m-tp2.json 파일 mtime(파일시스템 관측 — 문서에 시각 칸이 없다 · 기록 필드 아님)) · 묶은 근거 cell-name · 파일 `output/multi/benchlog/attestation_q38fn31-fp8-1m-tp2.json` · 측정 창 대비 작성 2026-10-25T01:39:26Z < 측정 시작 2026-10-25T01:39:47Z — 측정 전(빌드·스모크)의 관측: **same config(q38fn31-fp8-1m-tp2) · serve — 원장 순서로 판별한 같은 기동의 관측(스모크 로그 미보존 · 약한 판별)(판별: 원장 순서로 판별(스모크 로그 미보존 — docs/simlog/**/*serve*.log 에 이 attestation 을 적은 스모크 로그 없음): 원장 docs/logs/main/events/2026-10.jsonl:636 budget_declare(smoke-q38fn31-fp8-1m-tp2 · 2026-10-25T01:23:44Z) ≤ attestation 작성 2026-10-25T01:39:26Z · serve_proof 작성 2026-10-25T01:39:26Z < 측정 시작 2026-10-25T01:39:47Z · 그 선언이 측정 끝 2026-10-25T01:51:17Z 까지 재선언 · 창 닫힘 없이 이어짐(main 원장 기록 범위 안) · API 서버 기동 2026-10-25T01:38:26Z(output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/lite_engine_q38fn31-fp8-1m-tp2.log:743 · 컨테이너 시계 = UTC 가정) ∈ [선언, attestation 작성] · 측정 엔진 로그 APIServer pid 793 하나)**
<!-- /FACT:sub_recipe -->

<!-- FACT:measurement_env_nodes -->
**노드별 측정 env**(01 §1.4 표를 키 × 노드로 접은 것 · `만 관측` = 다른 쪽 줄이 캡처 로그에 없다 · Ray 가 같은 줄을 접은 키(`[repeated Nx across cluster]`)와 머리·꼬리 캡처 로그(01 §1.4 로그 창)에서 한쪽만 남은 키는 다른 쪽 부재를 단언하지 않는다)

- 양 노드 같은 값 11개: `NCCL_CROSS_NIC` · `NCCL_DMABUF_ENABLE` · `NCCL_IB_DISABLE` · `NCCL_IB_GID_INDEX` · `NCCL_IB_HCA` · `NCCL_IB_MERGE_NICS` · `NCCL_IB_QPS_PER_CONNECTION` · `NCCL_IB_SPLIT_DATA_ON_QPS` · `NCCL_NET_PLUGIN` · `NCCL_SOCKET_IFNAME` · `nccl:version`

| 키 | main | sub | 노드 미특정 | 판정 |
|---|---|---|---|---|
| `nccl:net_ib_devices` | `[RO]; OOB <nic:cluster>:<node:main><0>` | `[0]<nic:cluster>:1/RoCE [1]<nic:cluster>:1/RoCE [RO]; OOB <nic:cluster>:<node:sub><0>` · `[RO]; OOB <nic:cluster>:<node:sub><0>` | — | 다름 |
| `nccl:network` | — | `IB` | — | sub 만 캡처 창에 남음 — main 여부 미관측(머리 캡처) |
<!-- /FACT:measurement_env_nodes -->

> 이 절이 답하는 질문: 서브(Ray worker) 노드는 메인과 무엇이 다르고, 무엇을 반드시 같게 맞춰야 하는가?

**(1) 역할 차이.** master 는 Ray head 를 띄우고 slave 의 GPU 합류(`ray status` 가 GPU 2 — serve_runner.sh L79 리터럴)를 기다린다. 그다음 트리플렛 러너(`/app/configs/${CONFIG_FILE}.sh`)로 `vllm serve` 를 실행한다. slave 는 트리플렛을 받지 않고(`CONFIG_FILE=default`) Ray 워커로 합류한다. 같은 이미지 이름 · 같은 compose · 같은 serve_runner.sh 가 `NODE_ROLE` 로 갈린다. 다만 slave 도 모델 런타임 패치를 **무장한다**. 근거는 1.5 사실 블록의 `slave_runtime_patch = armed`(전달 키 `RUNTIME_PATCH_STEM`)와 측정 머리 로그의 서브 워커 플러그인 설치 줄(head L109 · `ip=<node:sub>`)이다. 서브 Ray 워커도 PLE 임베딩을 만든다(서브 rank 의 샤드 매핑 줄 L618). 그래서 서브에서도 플러그인이 활성이어야 한다.

**(2) 서브에 전달되는 env 무리와 이유.**
- 이미지 정체성(`IMAGE_TAG` · `BUILD_DOCKERFILE` · `VLLM_REF`): slave 가 compose 기본값으로 다른 이미지를 짓지 않게 한다.
- 빌드 튜닝(`BUILD_JOBS`): 양 노드에서 같아야 서브만 OOM 나는 일이 없다.
- 마운트(`QUANT_MODEL_PATH` · `PLE_MMAP_HOST_PATH` 등): `--env-file` 경로에서 프로젝트 `.env` 가 자동 로드되지 않기 때문이다.
- 클러스터(`SLAVE_CONTAINER_NAME`): 서브 워치독 필터가 이 이름으로 매칭한다.
- 서빙 env(`RUNTIME_PATCH_STEM` · `VLLM_PLE_MMAP` · `VLLM_PLE_MMAP_DIR`): 서브도 가중치와 PLE 임베딩을 올리므로 마스터와 같은 값이어야 한다.

이 셀 env 의 `VLLM_PLE_MMAP_VERIFY` 는 서브 전달 목록에 없다. 그런데도 서브 rank 도 `verify OK` 를 남겼다(lite_engine L693). 전달 경로는 이 계보에서 확인하지 않았다(미확인 · vLLM Ray 실행기가 `VLLM_*` env 를 워커에 복사한다는 추론이 있으나 대조 없음).

**(3) 선행조건.** 체크포인트는 양 노드의 같은 마운트 경로에 있어야 한다. PLE 스테이징 33파일도 **서브 로컬 디스크의 같은 경로**(`<manifest.ple_mmap_host_path>/q38fn-fp8`)에 있어야 한다. 서브 스테이징 실재는 서브 rank 의 `128 shards in 33 files … dir=/app/ple_mmap/q38fn-fp8`(L618)로 측정 기동 때 관측됐다. 저작 때 직접 확인하지는 않았다. 드라이버 계열이 같아야 하고, 인터커넥트(RoCE · `.env.interconnect`)가 준비돼 있어야 한다.

**(4) ABI 동일성.** attestation 관측값은 vLLM SHA `db9527a46873454610df6dbedf79a36d6bf1a7f6`(양 노드 equal) · torch `2.13.0a0+9186a08b2c.nv26.7.59513937`(equal) · 드라이버 580.178.04(equal)이다. 빌드 원장 축은 main identity sha256 만 있고 sub 는 null 이라 `unobservable` 이다(02 Q5). 이 attestation 은 범위 줄이 말하듯 같은 config 의 serve 단계에서 측정 직전에 쓰였다. 원장 순서로 같은 기동이라고 판별했고(스모크 로그 미보존 · 약한 판별), 다른 빌드 config 의 관측은 아니다. 같아야 하는 것은 digest 가 아니라 ABI 다. 노드마다 로컬로 빌드한다.

**(5) 측정 당시 인터커넥트 실효값**(위 '노드별 측정 env' 와 01 §1.4 env 관측 표 그대로).
- 양 노드 같은 값: `NCCL_IB_DISABLE=0` · `NCCL_NET_PLUGIN=spcx` · `NCCL_IB_HCA` 장치 2개 · `NCCL_IB_MERGE_NICS=1` · `NCCL_IB_QPS_PER_CONNECTION=4` · `NCCL_IB_GID_INDEX=3` · `NCCL_CROSS_NIC=1` · `NCCL_DMABUF_ENABLE=0` · `NCCL_IB_SPLIT_DATA_ON_QPS=0` · `NCCL_SOCKET_IFNAME` · NCCL `2.30.7+cuda13.3`. 출처는 sub 가 lite_engine_head L115~L174, main 이 lite_engine L434~L593 이다.
- `nccl:network IB` 는 sub 머리 캡처에만 남았다. main 여부는 미관측이고, 한 노드만 설정됐다는 뜻이 아니다.
- `nccl:net_ib_devices` 의 값 차이(main 은 `[RO]; OOB …` 만, sub 는 RoCE 장치 2개 줄도 있음)는 머리 캡처와 꼬리 창이 다른 줄을 잡은 결과일 수 있다. 장치 구성 차이로 읽지 않는다.
- Ray 가 접은 줄(`Ray 접힘`)의 키는 접힌 사본의 노드를 로그가 말하지 않는다.
- 1.1 '싣지 않은 파일' 에 측정 뒤 재생성된 env 형상은 없다. 실린 `.env.interconnect.template` 의 파생 규칙(1.4 실효값 표)으로 다시 렌더한 값이 위 관측과 맞는지 대조한다.

## 1.6 재현 절차 — 기동 전 준비(사람이 할 일)
> 1.4 의 명령을 치기 **전에** 사람이 손으로 해야 하는 것이다 — 가중치 획득 · 스테이징 생성 · 네트워크가 필요한 단계. 에이전트가 대신 할 수 없는 자리(다운로드 승인 · 물리 경로 · 외부 계정)가 여기 모인다.

> 이 절이 답하는 질문: 이 셀을 기동하기 전에 사람이 준비해야 하는 것은 무엇이며, 각각을 어떤 명령으로 만들고, 그 절차의 출처는 어디인가?

### (a) 가중치 획득 — `unsloth/Qwen3.8-Flash-Next-FP8`

- 체크포인트: HF repo `unsloth/Qwen3.8-Flash-Next-FP8` · revision `41cc25fe32cc20053a59c89716196897580cddf6`(00 §0.2 · 체크포인트 git HEAD). 크기는 safetensors 헤더 기준 172.78 GiB 이고 샤드 131개다(`plan_26091108_qwen38fn_22셀_재수행_캠페인` §2 · NAS `du` 는 과대보고 · 02 M4). `model.safetensors.index.json` `metadata.total_size` 로 확인한다.
- 획득 모드: 이 셀은 관리 NAS 에 이미 있는 사본을 양 노드가 같은 마운트 경로로 직접 읽었다(`model_source: managed` · NAS 상재 · 다운로드 불요 — `plan_26090918` §2.1). FP8 사본은 이전 캠페인 중 Unsloth 판으로 다시 받은 것이다(`plan_26091108_qwen38fn_22셀_재수행_캠페인` §2). 그때의 다운로드 명령은 계보에 없다.
- 받는 명령(**추론 — 실행 검증 ✗** · 모델 획득은 사람 승인 게이트다):

```sh
huggingface-cli download unsloth/Qwen3.8-Flash-Next-FP8 --revision 41cc25fe32cc20053a59c89716196897580cddf6 \
  --local-dir <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-FP8
```

- 노드마다 같은 경로에 보여야 한다. 공유 NAS 마운트면 한 벌, 로컬이면 노드마다 한 벌이다.

### (b) PLE 스테이징 생성 — 노드마다 로컬 NVMe 의 같은 경로

- 무엇: 플러그인은 `VLLM_PLE_MMAP_DIR` 아래 `*.safetensors` 의 헤더를 읽어 `layers.<n>.ple.ple_embedding.ngram_embedding.shard_<i>.weight` 텐서를 찾는다(번들 `table.py` `find_shards` · index 파일은 읽지 않는다). 그래서 PLE 샤드를 담은 체크포인트 파일들을 원본 그대로 복사한다. 변환은 없다.
- 어느 파일인지(저작 때 실행해 확인 · 읽기 전용 명령): 아래 명령은 index 에서 샤드 텐서 128개가 33개 파일 `model-00005-of-00131.safetensors` ~ `model-00037-of-00131.safetensors` 에 있다고 냈다. 이 셀 엔진 로그의 `128 shards in 33 files`(L428)와 같다.

```sh
python3 -I - <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-FP8/model.safetensors.index.json <<'EOF_PY'
import json, sys
wm = json.load(open(sys.argv[1]))["weight_map"]
fs = sorted({f for k, f in wm.items() if ".ple.ple_embedding.ngram_embedding.shard_" in k})
print(len(fs)); print("\n".join(fs))
EOF_PY
```

- 복사(**추론 — 실행 검증 ✗** · 스테이징을 만든 명령은 계보에 없다. 계보에는 "mmap 스테이징 양 변종 준비됨(`ple_staging/q38fn-{fp8,nvfp4}`)" 만 있다 — `plan_26091108_qwen38fn_22셀_재수행_캠페인` §2). 노드마다 위 목록의 파일을 `<manifest.ple_mmap_host_path>/q38fn-fp8/` 로 복사한다. 예: `mkdir -p <manifest.ple_mmap_host_path>/q38fn-fp8 && cp <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-FP8/model-000{05..37}-of-00131.safetensors <manifest.ple_mmap_host_path>/q38fn-fp8/`. 메인 노드에서 저작 때 본 스테이징은 이 33파일이었고 `du` 49G 였다. 파일 시각은 2026-09-08~09 로, 이전 캠페인 때 만든 것이다. 서브는 보지 않았다.
- 위치 조건: 로컬 NVMe 다. NAS mmap 은 디코드 지연 위험이 있다(`plan_26090918` §5 위험표 · compose 주석). 양 노드 같은 문자열 경로여야 하고, manifest `ple_mmap_host_path` → compose 볼륨 `/app/ple_mmap:ro` 로 이어진다.
- 확인: 노드마다 `ls <manifest.ple_mmap_host_path>/q38fn-fp8 | wc -l` 이 33 인지, 각 파일을 원본과 `cmp` 해 바이트가 같은지(**추론 — 실행 검증 ✗**) 본다. 기동 후 양 rank 로그에서 `128 shards in 33 files … dir=/app/ple_mmap/q38fn-fp8` 를 확인한다(이 셀에서 관측).

### (c) 네트워크가 필요한 단계

- 가중치 다운로드(위 (a) · 약 173 GiB · 사람 승인 뒤). 오프라인 대체는 관리 NAS 사본을 마운트하는 것이다.
- 이미지 빌드: 베이스 `nvcr.io/nvidia/pytorch:26.07-py3` pull · vLLM 소스 `VLLM_REPO` 의 `v0.31.0` clone · pip 의존(`requirements.txt` · Ray `2.48.0`). 오프라인 대체는 베이스 이미지와 소스를 미리 받아 두거나, 이미 지은 이미지를 노드마다 로컬로 갖는 것이다(이미지 save/load 전송은 이 하네스가 금지).
- 측정 클라이언트: GuideLLM 이미지 `ghcr.io/vllm-project/guidellm:v0.7.3`(level_01 `bench_tool_*.json` · digest 기록). 미리 pull 해 둔다. 측정 중에는 `HF_HUB_OFFLINE=1` 로 돈다(`run_bench.sh@29cbdbc89a3d` L444).
- 런타임 패치(플러그인)는 번들이 패치 파일 안에 있어 네트워크가 필요 없다.
