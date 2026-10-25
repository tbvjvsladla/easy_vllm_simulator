# 01 · 산출물 — 무엇이 실제로 쓰였고, 어떻게 다시 띄우는가

## 1.1 적용 판정 표
> 기계가 적용 증거(빌드 원장 · 원장이 없으면 라벨된 재구성) × 파일 × 선언을 대사한 결과다. `artifacts/` 에는 **적용된 것만** 실렸다 — 빌드 때 스스로 skip 된 패치와 쓰이지 않은 평면의 파일은 표에만 남고 싣지 않는다.

<!-- FACT:slots -->
| 슬롯 | 실림 | 파일 | 신호 | 적용 증거 | 사유 |
|---|---|---|---|---|---|
| `build_patch_post` | 안 실림 | — | 1-signal | build-ledger:attestation:output/multi/benchlog/attestation_solar31-nv4-1m-tp2.json#main | 적용 판정 없음 — 원장 관측(기계 파생) |
| `build_patch_pre` | 안 실림 | — | 1-signal | build-ledger:attestation:output/multi/benchlog/attestation_solar31-nv4-1m-tp2.json#main | 적용 판정 없음 — 원장 관측(기계 파생) |
| `build_recipe` | 실림 | `artifacts/build_recipe/Dockerfile.source-build` · `artifacts/build_recipe/requirements.txt` | 3-signal | build-ledger:build-ledger.dockerfile | 이미지 `easy-vllm:0.31.0-cu133-aarch64-source` 는 이 셀을 위해 다시 짓지 않았다(devlog_26102504 · 이미지 불변). 이 이미지는 v0.31.0 bump 때 single-node 브랜치 체크아웃의 `output/single/` 통로에서 지어졌고(계보 밖 문서 testlog_26101412_v031_14B이하_4종_양노드_상주서빙_판정 §표 — main 이미지 `sha256:339737df…` · single `Dockerfile.source-build`), 실린 두 파일은 `12ef50a:output/single/…` 이다. 12ef50a 는 이미지 Created(01:45:19Z) 뒤 03:07:26Z 커밋으로 스모크 PASS 뒤 검증키를 승급한 개정이고, 빌… |
| `compose` | 실림 | `artifacts/compose/.env.cluster.template` · `artifacts/compose/.env.interconnect.template` · `artifacts/compose/.env.template` · `artifacts/compose/arm_patch.sh` · `artifacts/compose/docker-compose.yaml` · `artifacts/compose/serve_runner.sh` · `artifacts/compose/sub_recipe.json` | 2-signal | file:output/multi/docker-compose.yaml | 멀티 공통 서빙 경로이며 이 모델 전용이 아니다. 이 셀에 걸린 것은 둘이다. ① `arm_patch.sh` 가 `RUNTIME_PATCH_STEM` 을 `CONFIG_FILE` 보다 먼저 읽고, compose 가 그 키를 master · slave 양쪽 environment 로 넘긴다 — W3 의 교정(커밋 d781f26)이라 빼면 W3 이 다시 나온다. 측정 당시 개정이 그 커밋이다(1.1 실린 리비전). ② `serve_runner.sh` 는 master 가 `ray status` 의 GPU 합류가 리터럴 `2` 가 될 때까지 기다린다(L79 · 1.5 `runner_gpu_join_literal`) — 노드당 GPU · 노드 수가 다른 클러스터에서는 고쳐야 한다. arm_patch 는 serve_r… |
| `fork_pin` | 안 실림 | — | 1-signal | none:.claude/policies/arch_variant_ledger.json | 셀 env 에 VARIANT 줄 없음 = stock(workflow.md §변종 좌표의 거처)(기계 파생) |
| `runtime_patch` | 실림 | `artifacts/runtime_patch/solar31-nv4-1m-tp2_patch.py` | 3-signal | none:output/multi/configs/solar31-nv4-1m-tp2_patch.py | (a) **무엇을 고치나** — Solar-Open2 지원 전체를 out-of-tree vLLM 플러그인으로 담은 번들이다. 파일 자체는 stdlib 만 쓰는 짧은 로더(머리 주석 4줄 · `_arm()`)와 zlib+base64 로 압축한 tar 한 덩어리(`_B`)다. 로더는 번들을 `/tmp/solar_open2_plugin_dist_d61b873ad8` 에 풀어 `sys.path` 에 얹고, vLLM 이 `load_general_plugins()` 에서 entry point `solar_open2 = solar_open2_plugin:register` 를 찾아 부른다. `register()` 는 HF config(`solar_open2`) · 모델(`SolarOpen2ForCausalLM` → `s… |
| `triplet` | 실림 | `artifacts/triplet/.env.solar31-nv4-1m-tp2` · `artifacts/triplet/solar31-nv4-1m-tp2.sh` · `artifacts/triplet/solar31-nv4-1m-tp2.yaml` | 3-signal | file:output/multi/configs/solar31-nv4-1m-tp2.yaml | 서빙 노브(1.3) · 러너 · 셀 env 다. 이 셀 고유의 필요조건 두 줄: 러너 `.sh` 의 `--no-async-scheduling`(빼면 W7) · 셀 env 의 `RUNTIME_PATCH_STEM=solar31-nv4-1m-tp2`(빼면 서브가 패치를 못 찾아 W3). yaml 의 `async-scheduling` 줄은 무효라 주석으로 남겨 두었다(M1). 러너는 `arm_patch.sh` 를 한 번 더 source 한다(멱등 — arm_patch 머리 주석). |

**레시피 경고 · 실린 리비전**(기계 관측 — 수신자 빌드가 여기서 갈라질 수 있다)

- ⚠ `build_recipe` `output/single/Dockerfile.source-build` — 실린 바이트(git:12ef50a89b86cb30d4c5b87c32506c5563ff4655)가 측정 이미지 docker history 의 RUN·COPY 와 전수 일치하지 않는다(16/17)(이미지 Created 2026-10-14T01:45:19Z · 실린 개정 커밋 2026-10-14T03:07:26Z · 이 체크아웃 통로 `output/multi/Dockerfile.source-build` 의 파일이 아니다)
- `build_recipe` `output/single/Dockerfile.source-build`(이 체크아웃 통로 `output/multi/Dockerfile.source-build` 대신) 실린 리비전: 커밋 `12ef50a89b86` · 선택 방법 `git log --all -- output/*/Dockerfile.source-build · 12ef50a89b86:output/single/Dockerfile.source-build 가 docker history RUN·COPY 와 가장 잘 맞는 이력 개정(16/17 · 전수 일치 개정 없음 · 불일치 레시피에만: RUN echo "[guard] source-build key (26.07-py3 x vLLM 0.31.0): validated patch set ((ngc_base x vllm_version) keyed) — pr · 작업트리 15/18) — 이 체크아웃 통로(output/multi/Dockerfile.source-build) 밖 · 커밋 2026-10-14T03:07:26Z > 이미지 Created 2026-10-14T01:45:19Z(빌드 뒤 커밋 · 위 불일치 단계만큼 다르다) — 이 개정의 바이트를 싣는다(git cat-file)` · docker history 대조 16/17 · 실린 바이트 `git:12ef50a89b86cb30d4c5b87c32506c5563ff4655` · 쓰인 바이트임을 관측으로 확인 아니오
- `build_recipe` `output/single/requirements.txt`(이 체크아웃 통로 `output/multi/requirements.txt` 대신) 실린 리비전: 커밋 `12ef50a89b86` · 선택 방법 `git log --all -- output/*/requirements.txt · 12ef50a89b86:output/single/requirements.txt · 빌드 원장 sha256 일치(작업트리·이미지 Created 이전 개정은 다르다) — 이 체크아웃 통로(output/multi/requirements.txt) 밖 · 커밋 2026-10-14T03:07:26Z > 이미지 Created 2026-10-14T01:45:19Z(빌드 뒤 커밋된 같은 바이트)` · docker history 대조 없음 · 실린 바이트 `git:12ef50a89b86cb30d4c5b87c32506c5563ff4655` · 쓰인 바이트임을 관측으로 확인 예
- `compose` `.claude/skills/upstream-version-watch/assets/configs/arm_patch.sh` 실린 리비전: 커밋 `d781f26e4ebc` · 선택 방법 `worktree(git log -1 --before=2026-10-25T03:13:30Z -- .claude/skills/upstream-version-watch/assets/configs/arm_patch.sh → d781f26e4ebc · 작업트리 mtime 2026-10-25T03:14:49Z > 측정(sweep_index.generated_utc)이지만 바이트가 그 개정과 같다 · 측정 뒤 커밋 0 · 작업트리 = HEAD)` · docker history 대조 없음 · 실린 바이트 `worktree` · 근거 `git-rev-before-measurement` · 쓰인 바이트임을 관측으로 확인 아니오
- `compose` `.claude/skills/upstream-version-watch/assets/configs/serve_runner.sh` 실린 리비전: 커밋 `50a5ce486738` · 선택 방법 `worktree(mtime 2026-09-06T21:57:53Z ≤ 측정 2026-10-25T03:13:30Z(sweep_index.generated_utc) · 바이트 = 측정 전 마지막 커밋 50a5ce486738)` · docker history 대조 없음 · 실린 바이트 `worktree` · 근거 `mtime≤measured` · 쓰인 바이트임을 관측으로 확인 예
- `compose` `output/multi/docker-compose.yaml` 실린 리비전: 커밋 `d781f26e4ebc` · 선택 방법 `worktree(git log -1 --before=2026-10-25T03:13:30Z -- output/multi/docker-compose.yaml → d781f26e4ebc · 작업트리 mtime 2026-10-25T03:14:49Z > 측정(sweep_index.generated_utc)이지만 바이트가 그 개정과 같다 · 측정 뒤 커밋 0 · 작업트리 = HEAD)` · docker history 대조 없음 · 실린 바이트 `worktree` · 근거 `git-rev-before-measurement` · 쓰인 바이트임을 관측으로 확인 아니오

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
| `runtime_patch` | `artifacts/runtime_patch/solar31-nv4-1m-tp2_patch.py` | `verified` · cell-run | 셀 configs 의 런타임 패치(arming 관측 — 측정 기동 로그 output/multi/benchlog/sweep_solar31-nv4-1m-tp2/lite_engine_head_solar31-nv4-1m-tp2.log:34,55 · 실패 기동 로그(측정 기동 아님) master output/multi/benchlog/serve_fail_solar31-nv4-1m-tp2/master_exited.log:34,55 · master output/multi/benchlog/serve_fail_solar31-nv4-1m-tp2/master_failure.log:34,55 · slave output/multi/benchlog/serve_fail_solar31-nv4-1m-tp2/slave_failure.log:34 (`[arm_patch] runtime patch armed → /app/configs/solar31-nv4-1m-tp2_patch.py` = .pth 무장 · 패치 효과 자체는 별도 관측)) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/.env.solar31-nv4-1m-tp2` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/solar31-nv4-1m-tp2.sh` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/solar31-nv4-1m-tp2.yaml` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
<!-- /FACT:slots -->

<!-- FACT:applied_set -->
**적용 집합** — 상태 `observed` · 출처 `attestation:output/multi/benchlog/attestation_solar31-nv4-1m-tp2.json#main`

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
| attestation | observed | output/multi/benchlog/attestation_solar31-nv4-1m-tp2.json |

> **attestation 범위** — config `solar31-nv4-1m-tp2` · phase serve · 작성 2026-10-25T02:57:42Z(output/multi/benchlog/attestation_solar31-nv4-1m-tp2.json 파일 mtime(파일시스템 관측 — 문서에 시각 칸이 없다 · 기록 필드 아님)) · 묶은 근거 cell-name · 파일 `output/multi/benchlog/attestation_solar31-nv4-1m-tp2.json` · 측정 창 대비 작성 2026-10-25T02:57:42Z < 측정 시작 2026-10-25T02:58:03Z — 측정 전(빌드·스모크)의 관측: **same config(solar31-nv4-1m-tp2) · serve — 원장 순서로 판별한 같은 기동의 관측(스모크 로그 미보존 · 약한 판별)(판별: 원장 순서로 판별(스모크 로그 미보존 — docs/simlog/**/*serve*.log 에 이 attestation 을 적은 스모크 로그 없음): 원장 docs/logs/main/events/2026-10.jsonl:654 budget_declare(smoke-solar31-nv4-1m-tp2 · 2026-10-25T02:43:38Z) ≤ attestation 작성 2026-10-25T02:57:42Z · serve_proof 작성 2026-10-25T02:57:42Z < 측정 시작 2026-10-25T02:58:03Z · 그 선언이 측정 끝 2026-10-25T03:13:30Z 까지 재선언 · 창 닫힘 없이 이어짐(main 원장 기록 범위 안) · API 서버 기동 2026-10-25T02:57:07Z(output/multi/benchlog/sweep_solar31-nv4-1m-tp2/lite_engine_solar31-nv4-1m-tp2.log:736 · 컨테이너 시계 = UTC 가정) ∈ [선언, attestation 작성] · 측정 엔진 로그 APIServer pid 791 하나)**
<!-- /FACT:applied_set -->

## 1.2 슬롯별 적용 사유
> 이 절이 답하는 질문: 실린 산출물 하나하나는 무엇을 고치며, 왜 이 셀에 필요했고, 이 셀에서 실제로 발화했으며, 언제 불필요해지는가?

### runtime_patch — `solar31-nv4-1m-tp2_patch.py`(이 셀의 핵심 · +1 런타임 패치 슬롯 · owner `vllm-recipe-explorer`)

(a) **무엇을 고치나** — Solar-Open2 지원 전체를 out-of-tree vLLM 플러그인으로 담은 번들이다. 파일 자체는 stdlib 만 쓰는 짧은 로더(머리 주석 4줄 · `_arm()`)와 zlib+base64 로 압축한 tar 한 덩어리(`_B`)다. 로더는 번들을 `/tmp/solar_open2_plugin_dist_d61b873ad8` 에 풀어 `sys.path` 에 얹고, vLLM 이 `load_general_plugins()` 에서 entry point `solar_open2 = solar_open2_plugin:register` 를 찾아 부른다. `register()` 는 HF config(`solar_open2`) · 모델(`SolarOpen2ForCausalLM` → `solar_open2_plugin.model`) · reasoning 파서 · tool 파서를 등록하고, logits processor 는 등록하지 않는다(서빙 yaml `logits-processors` 의 FQCN 으로 넣는다). 이식 출처는 Upstage 포크 브랜치 `v0.22.0-solar-open2` 커밋 `00907fc9b982f973808a0193ad0f8c61fbb62854`(태그 아님 · 커밋 1개 · merge-base v0.22.0)이고, 0.31 API 로 다시 쓴 부분(파손 6종 · V2 logits processor · conv1d 로더 어댑터)이 자체 설계다. 업스트림 PR 이식이 아니다.

> [원문] solar31-nv4-1m-tp2_patch.py §L1-4
> # solar31-nv4-1m-tp2 런타임 패치 — Solar-Open2 → vLLM 0.31.0 자체 이식(out-of-tree 플러그인 번들)
> # 출처: UpstageAI/vllm 브랜치 v0.22.0-solar-open2 @ 00907fc9b982f973808a0193ad0f8c61fbb62854 (커밋 1개, merge-base = v0.22.0)를 0.31.1.dev0+gdb9527a46 API 에 맞춰 다시 쓴 이식본.
> # provenance=hand-ported(미검증 → 서빙 스모크가 유일한 중재자). site-init safe: stdlib 만 사용, vllm/transformers import ✗.
> # 동작: 번들을 /tmp 에 풀고 sys.path 에 얹는다 → vLLM 이 load_general_plugins() 로 entry point(solar_open2)를 발견·register().

**번들에서 읽을 수 있는 소스 트리 꺼내기**(패치 파일을 **실행하지 않고** `_B` 문자열만 AST 로 읽어 푼다 — 저작 시 Python 3.12.3 · 빈 임시 디렉터리에서 실행해 아래 파일 목록을 확인했다 · `filter="data"` 는 Python 3.12 / 3.11.4 이상):

```sh
mkdir -p ./solar_plugin_src && python3 -I - artifacts/runtime_patch/solar31-nv4-1m-tp2_patch.py ./solar_plugin_src <<'EOF_PY'
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

출력(확인한 목록 · 디렉터리 1 + 메타 2 + Python 7): `solar_open2_plugin-0.1.0.dist-info/{METADATA, entry_points.txt}` · `solar_open2_plugin/{__init__.py, configuration.py, kda_layer.py, logits_processor.py, model.py, reasoning_parser.py, tool_parser.py}`. 번들 안 `model.py` 머리 주석은 스스로를 `DRAFT / UNTESTED (never executed)` 라 적고 포크와 다른 자리를 `PORT:` 로 표시한다(그 주석은 번들 저작 시점의 것이고 뒤의 서빙 스모크 · needle 이 실행을 검증했다).

(b) **왜 필요했나** — 빼면 W1(stock 레지스트리에 모델 · 파서 없음)이 그대로 나온다. 번들 안의 V2 logits processor · conv1d 어댑터가 W2 · W4 를 넘긴 수단이고, chat-template kwargs 와 함께 W6 을 닫았다.

(c) **이 셀에서 발화했나** — 예(1.1 의 arming 관측과 같다). 측정 기동의 머리 캡처 로그에 무장 줄과 아키텍처 해소 줄이 있다: `[arm_patch] runtime patch armed → /app/configs/solar31-nv4-1m-tp2_patch.py`(lite_engine_head L34 · L55) · `Resolved architecture: SolarOpen2ForCausalLM`(L68). 서브 쪽 무장 줄은 측정 로그의 꼬리 창에는 없고(창 밖), 교정 뒤 기동의 `serve_fail_*/slave_failure.log` L34 에 있다 — 측정 기동에서 서브 rank 가 가중치를 다 올렸다(lite_engine L700 · L703)는 것이 서브에서 모델 클래스가 해소됐다는 간접 증거다. logits processor 의 마스크 발화(예산 강제)는 계보 경유 문서(testlog_26102500 §4 · 읽기 목록 밖)의 `solar_open2_reasoning_budget=20` 실험이 보였다고 적는다.

(d) **언제 불필요해지나** — 대상 vLLM 이 `SolarOpen2ForCausalLM` 과 파서 `solar_open2` 를 정식 등록하면(02 Q3) 이 번들은 빼야 한다(같은 이름을 플러그인이 다시 등록하면 충돌 여부 미확인). 트립와이어는 없다 — 무장 자체는 `arm_patch.sh` 가 패치 파일이 **있을 때만** `.pth` 를 쓰고, 없으면 아무것도 하지 않는다(빌드 · 기동을 멈추지 않는다).

> [원문] arm_patch.sh §L15-18
> # 패치 키: RUNTIME_PATCH_STEM(셀 env 가 선언 · slave 로도 전달) > CONFIG_FILE — slave 는 CONFIG_FILE=default 라 model-keyed 패치를 못 찾던 침묵 누락 교정(2026-10-24 Solar-Open2)
> _cfg="${RUNTIME_PATCH_STEM:-${CONFIG_FILE:-default}}"
> _patch="/app/configs/${_cfg}_patch.py"
> if [ -f "$_patch" ]; then

번들 자체의 fail-loud 경로: V2 프로세서는 spec decode 설정이 있거나 비동기 스케줄링이면 기동 때 `NotImplementedError` 로 막는다(W7 · R7 — 번들 `logits_processor.py`). KDA 레이어(`kda_layer.py`)는 기동 검사가 아니라 forward 중 spec 메타데이터를 만나면 raise 한다. 패치 파일은 비추적(휘발)이며 원래 거처 `output/multi/configs/solar31-nv4-1m-tp2_patch.py` 와 이 zip 의 사본은 바이트가 같다(저작 시 cmp).

### compose — `docker-compose.yaml` · `serve_runner.sh` · `arm_patch.sh` · env 형상 3종 · `sub_recipe.json`

멀티 공통 서빙 경로이며 이 모델 전용이 아니다. 이 셀에 걸린 것은 둘이다. ① `arm_patch.sh` 가 `RUNTIME_PATCH_STEM` 을 `CONFIG_FILE` 보다 먼저 읽고, compose 가 그 키를 master · slave 양쪽 environment 로 넘긴다 — W3 의 교정(커밋 d781f26)이라 빼면 W3 이 다시 나온다. 측정 당시 개정이 그 커밋이다(1.1 실린 리비전). ② `serve_runner.sh` 는 master 가 `ray status` 의 GPU 합류가 리터럴 `2` 가 될 때까지 기다린다(L79 · 1.5 `runner_gpu_join_literal`) — 노드당 GPU · 노드 수가 다른 클러스터에서는 고쳐야 한다. arm_patch 는 serve_runner 가 양 노드에서 `ray start` 전에 source 한다. 폐기 조건: 런타임 패치가 필요 없는 모델에서는 `RUNTIME_PATCH_STEM` 을 비우면 종전 동작이다(compose 주석).

### triplet — `solar31-nv4-1m-tp2.yaml` · `.sh` · `.env.solar31-nv4-1m-tp2`

서빙 노브(1.3) · 러너 · 셀 env 다. 이 셀 고유의 필요조건 두 줄: 러너 `.sh` 의 `--no-async-scheduling`(빼면 W7) · 셀 env 의 `RUNTIME_PATCH_STEM=solar31-nv4-1m-tp2`(빼면 서브가 패치를 못 찾아 W3). yaml 의 `async-scheduling` 줄은 무효라 주석으로 남겨 두었다(M1). 러너는 `arm_patch.sh` 를 한 번 더 source 한다(멱등 — arm_patch 머리 주석).

### build_recipe — `Dockerfile.source-build` · `requirements.txt`(공유 이미지 · 이 모델의 요구 아님)

이미지 `easy-vllm:0.31.0-cu133-aarch64-source` 는 이 셀을 위해 다시 짓지 않았다(devlog_26102504 · 이미지 불변). 이 이미지는 v0.31.0 bump 때 single-node 브랜치 체크아웃의 `output/single/` 통로에서 지어졌고(계보 밖 문서 testlog_26101412_v031_14B이하_4종_양노드_상주서빙_판정 §표 — main 이미지 `sha256:339737df…` · single `Dockerfile.source-build`), 실린 두 파일은 `12ef50a:output/single/…` 이다. 12ef50a 는 이미지 Created(01:45:19Z) 뒤 03:07:26Z 커밋으로 스모크 PASS 뒤 검증키를 승급한 개정이고, 빌드 입력은 '689610f + 미커밋 렌더' 였다(testlog_26101412 유효맥락). `requirements.txt` 는 빌드 원장 sha256 과 바이트가 같다(verified). `Dockerfile.source-build` 는 docker history RUN · COPY 와 16/17 만 맞아 `generated-unverified` 다 — 다른 한 단계는 guard RUN 으로, 실린 L40 은 `validated … proceeding` 을, 이미지 history 는 `WARN: … is UNVALIDATED — attempt-build under HITL override` 를 echo 한다(1.1 경고). 이 차이는 echo 문구뿐인지 그 단계의 다른 동작까지인지 이 저작에서 대조하지 않았다. 그래서 재현 빌드는 이 체크아웃의 `output/multi/` 스모크 · compose build 경로가 아니라 1.4 의 `docker build -f output/single/Dockerfile.source-build … output/single` 명령이다(동등 경로 없음). **상류 파일 이동 확인**: v0.31.0 은 `use_existing_torch.py` 를 `tools/` 로 옮겼는데(devlog_26101412 — 첫 빌드가 이것으로 38초 만에 죽었다), 실린 Dockerfile L82-84 는 `tools/use_existing_torch.py` 를 먼저 찾고 없으면 루트를, 둘 다 없으면 `exit 1` 로 멈춘다. 측정 이미지 안에는 `/workspace/vllm-src/tools/use_existing_torch.py` 가 있고 루트 사본은 없다(저작 시 시작하지 않는 컨테이너 create · cp 로 확인) — 같은 ref(v0.31.0)를 지으면 이 단계에서 멈추지 않는다. Solar 지원은 이미지가 아니라 런타임 패치에 있다(R2).

### 싣지 않은 슬롯
- `build_patch_pre` · `build_patch_post` — 측정 이미지의 빌드 원장(attestation)에 적용된 빌드 패치 파일이 없다. 원장이 기록한 것은 inline `strip-hoist` 하나이고 결과는 **skipped**(NGC torch 의 `register_opaque_type` 가 hoist 를 받아들임)다. 이 이미지의 빌드 컨텍스트(`output/single/`)의 두 패치 디렉터리는 `.gitkeep` 만 있었다 — 원장 `context_inventory` 가 두 슬롯 모두 `.gitkeep` 만 기록하고 1.1 '싣지 않은 파일' 도 `.gitkeep` 뿐이다. 재현 빌드 전에 빈 디렉터리를 만들어 두는 명령이 1.4 빌드 블록에 있다(`mkdir -p output/single/build_patches_src output/single/build_patches`). 이 체크아웃의 `output/multi/` 패치 디렉터리 파일들은 이 이미지의 컨텍스트가 아니다. 싣지 않은 것은 이 셀에 무엇이 필요했는지의 판정이 아니다.
- `fork_pin` — 셀 env 에 `VARIANT` 줄이 없다 = stock 빌드. 포크 핀은 다운그레이드라 기각했다(R1).

## 1.3 값의 지위표
> 서빙 설정의 노브 **전부**와 기계가 모은 후보 지위다(lockset 출처 · yaml 주석 · 엔진 기본값). 최종 지위는 아래 `kind: value-status` 블록이 정한다 — 표의 노브마다 블록이 정확히 하나 있어야 린터를 통과한다.

<!-- FACT:value_status -->
서빙 노브 16개 — 노브마다 `kind: value-status` 블록이 **정확히 1개** 있어야 한다(노브·값은 이 표의 글자 그대로 · 값 칸의 `\|` 는 `|` 로 적는다).

| 노브 | 값 | 후보 지위 | 후보 근거 | yaml 주석 | 출처 |
|---|---|---|---|---|---|
| `tensor-parallel-size` | 2 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `distributed-executor-backend` | ray | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `enable-expert-parallel` | true | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `moe-backend` | marlin | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `gpu-memory-utilization` | 0.80 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `max-model-len` | 1048576 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `kv-cache-dtype` | fp8_e4m3 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `kv-cache-memory-bytes` | 15032385536 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `enforce-eager` | true | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `max-num-seqs` | 1 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `max-num-batched-tokens` | 2048 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `reasoning-parser` | solar_open2 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `tool-call-parser` | solar_open2 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `enable-auto-tool-choice` | true | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `logits-processors` | solar_open2_plugin.logits_processor:SolarOpen2TemplateLogitsProcessor | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `default-chat-template-kwargs` | {"think_render_option":"preserved"} | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
<!-- /FACT:value_status -->

> 이 절이 답하는 질문: 각 서빙 노브의 값은 이 셀에서 어떤 지위인가 — 조정됐나, 승계됐나, 음성대조로 일부러 둔 것인가, 엔진 기본값인가, 필요조건인가?

```hint-event
id: V1
kind: value-status
노브: tensor-parallel-size
값: 2
지위: inherited
근거: 멀티 캠페인 plan 이 처음부터 TP=2 로 정했다 — 싱글 TP=1 배제는 산술 사전판정(R4 · 시도 0)이지 관측된 실패가 아니다 · 다른 TP 시도 0
출처: [plan_26102412_4모델_멀티캠페인_Solar_Qwen_TP2_최대context §셀 (TP=2 이므로 노드 병렬 아님 · 순차)]
```

```hint-event
id: V2
kind: value-status
노브: distributed-executor-backend
값: ray
지위: inherited
근거: 멀티 하네스의 Ray head/worker 기동 경로를 따른 값 · 다른 백엔드 시도 0
출처: [plan_26102412_4모델_멀티캠페인_Solar_Qwen_TP2_최대context §목표, serve_runner.sh]
```

```hint-event
id: V3
kind: value-status
노브: enable-expert-parallel
값: true
지위: inherited
근거: HF 모델 카드 서빙 인자를 옮긴 값 · 끈 구성 시도 0
출처: [testlog_26102413_solar_open2_stock_0.31.0_아키텍처_미지원_판정 §② 외부 확증]
```

```hint-event
id: V4
kind: value-status
노브: moe-backend
값: marlin
지위: inherited
근거: 손레버 — 카드의 triton 이 0.31 NVFP4 후보에 없어 marlin 을 처음부터 골랐다(R3) · 다른 후보(FLASHINFER_* · VLLM_CUTLASS · HUMMING) 시도 0
출처: [testlog_26102418_solar_open2_nvfp4_멀티_TP2_1M_자체이식_기동_needle_lite_판정 §2. 기동까지의 결함 4건(전부 라이브가 알려줌), output/multi/benchlog/sweep_solar31-nv4-1m-tp2/lite_engine_solar31-nv4-1m-tp2.log]
```

```hint-event
id: V5
kind: value-status
노브: gpu-memory-utilization
값: 0.80
지위: inherited
근거: 계보 문서에 다른 값 · 스윕 기록이 없다(첫 셀 testlog 유효맥락에도 적히지 않음) · KV 는 kv-cache-memory-bytes 절대 클램프로 따로 정했다
출처: [testlog_26102418_solar_open2_nvfp4_멀티_TP2_1M_자체이식_기동_needle_lite_판정 §3. 기동 실측]
```

```hint-event
id: V6
kind: value-status
노브: max-model-len
값: 1048576
지위: inherited
근거: 체크포인트 max_position_embeddings 그대로(rope_scaling 없음 · YaRN 없음) — 캠페인 목표가 지원 최대치라 바꾼 적 없다
출처: [plan_26102412_4모델_멀티캠페인_Solar_Qwen_TP2_최대context §셀 (TP=2 이므로 노드 병렬 아님 · 순차)]
```

```hint-event
id: V7
kind: value-status
노브: kv-cache-dtype
값: fp8_e4m3
지위: inherited
근거: 첫 구성부터 같은 값 · 다른 dtype 시도 0
출처: [testlog_26102418_solar_open2_nvfp4_멀티_TP2_1M_자체이식_기동_needle_lite_판정 §3. 기동 실측]
```

```hint-event
id: V8
kind: value-status
노브: kv-cache-memory-bytes
값: 15032385536
지위: inherited
근거: 14 GiB 절대 클램프 · 스윕 0 · 엔진이 잡은 KV 1,206,177 토큰 = 1M 요청 1개의 1.15배(엔진 로그) — 필요량을 조금 넘긴 값이지 음성대조 과잉이 아니다
출처: [testlog_26102418_solar_open2_nvfp4_멀티_TP2_1M_자체이식_기동_needle_lite_판정 §3. 기동 실측, output/multi/benchlog/sweep_solar31-nv4-1m-tp2/lite_engine_solar31-nv4-1m-tp2.log]
```

```hint-event
id: V9
kind: value-status
노브: enforce-eager
값: true
지위: inherited
근거: 첫 셀 testlog 유효맥락(eager)부터 같은 값 · cudagraph 시도 0 · eager 선택 이유(캡처 스파이크 실측 등) 기록 없음
출처: [testlog_26102418_solar_open2_nvfp4_멀티_TP2_1M_자체이식_기동_needle_lite_판정 §5. lite, output/multi/benchlog/sweep_solar31-nv4-1m-tp2/lite_engine_head_solar31-nv4-1m-tp2.log]
```

```hint-event
id: V10
kind: value-status
노브: max-num-seqs
값: 1
지위: inherited
근거: 첫 셀 testlog 유효맥락(max-num-seqs 1)부터 같은 값 · 1M 요청 1개 기준 KV 1.15배라 동시 2 는 KV 를 다시 도출해야 한다 · 스윕 0
출처: [testlog_26102418_solar_open2_nvfp4_멀티_TP2_1M_자체이식_기동_needle_lite_판정 §3. 기동 실측]
```

```hint-event
id: V11
kind: value-status
노브: max-num-batched-tokens
값: 2048
지위: inherited
근거: 계보 문서에 다른 값 · 스윕 기록이 없다
출처: [solar31-nv4-1m-tp2.yaml]
```

```hint-event
id: V12
kind: value-status
노브: reasoning-parser
값: solar_open2
지위: inherited
근거: HF 모델 카드 인자 · 이름은 플러그인 register 가 등록한다(stock 0.31 에 없음) · 뺀 구성 시도 0
출처: [testlog_26102413_solar_open2_stock_0.31.0_아키텍처_미지원_판정 §② 외부 확증, solar31-nv4-1m-tp2_patch.py]
```

```hint-event
id: V13
kind: value-status
노브: tool-call-parser
값: solar_open2
지위: inherited
근거: HF 모델 카드 인자 · 플러그인이 등록 · 뺀 구성 시도 0
출처: [testlog_26102413_solar_open2_stock_0.31.0_아키텍처_미지원_판정 §② 외부 확증, solar31-nv4-1m-tp2_patch.py]
```

```hint-event
id: V14
kind: value-status
노브: enable-auto-tool-choice
값: true
지위: inherited
근거: HF 모델 카드 인자 · 뺀 구성 시도 0
출처: [testlog_26102413_solar_open2_stock_0.31.0_아키텍처_미지원_판정 §② 외부 확증]
```

```hint-event
id: V15
kind: value-status
노브: logits-processors
값: "solar_open2_plugin.logits_processor:SolarOpen2TemplateLogitsProcessor"
지위: inherited
근거: 모델 카드 인자 · 첫 셀은 v1 구현이라 빼고 기동했고(W2 · 기동 · 장문은 영향 없음) S4 에서 V2 로 다시 넣었다 — 넣으면 러너 --no-async-scheduling 이 필요하다(W7) · spec decode 를 막는다(R7)
출처: [testlog_26102418_solar_open2_nvfp4_멀티_TP2_1M_자체이식_기동_needle_lite_판정 §2. 기동까지의 결함 4건(전부 라이브가 알려줌), plan_26102420_PLE_mmap_재이식_full벤치_hint_Solar_슬롯화 §정정·진행 기록 (2026-10-25 KST · S2~S4 완료)]
```

```hint-event
id: V16
kind: value-status
노브: default-chat-template-kwargs
값: '{"think_render_option":"preserved"}'
지위: inherited
근거: 모델 카드 권장값 · W6 교정에 V2 logits processor 와 함께 넣었다 — 실패한 첫 구성은 프로세서도 없었고 kwargs 만 뺀 대조가 없어 단독 필요조건으로 확인되지 않았다(함께 옮긴다)
출처: [testlog_26102418_solar_open2_nvfp4_멀티_TP2_1M_자체이식_기동_needle_lite_판정 §2. 기동까지의 결함 4건(전부 라이브가 알려줌), plan_26102420_PLE_mmap_재이식_full벤치_hint_Solar_슬롯화 §정정·진행 기록 (2026-10-25 KST · S2~S4 완료)]
```

**요약** — 16개 모두 `inherited`(조정 · 단독 제거 대조 기록 없음)다. `default-chat-template-kwargs`(V16)와 `logits-processors`(V15)는 W6 교정에 함께 들어간 짝이라 함께 옮긴다. 복사하면 위험한 값: `kv-cache-memory-bytes` · `gpu-memory-utilization` · `max-num-seqs` 는 GB10 통합메모리 · 노드당 71.6 GiB 적재 · 1M 1요청에 맞춘 값이라 네 HW 에서 다시 도출해야 한다(V5 · V8 · V10). `moe-backend` marlin 은 0.31 후보 목록에서 손으로 고른 값이며 최적이라는 근거가 없다(V4). `logits-processors` 는 단독으로 옮기면 안 된다 — 러너의 `--no-async-scheduling`(yaml 에 적으면 무효 · M1)과 짝이고 spec decode 와 함께 쓸 수 없다(V15 · W7 · R7). 러너의 `--no-async-scheduling` 자체는 yaml 노브가 아니라 이 표에 없다 — 빼면 V2 프로세서가 기동을 막는다(W7).

## 1.4 재현 절차
> 셀 실행 기록에서 기계가 파생한 명령 순서와 단계별 소요다. 소요가 `미관측` 인 단계는 시각 기록이 없다는 뜻이다(0 이 아니다). 그 아래는 블랙박스 이벤트 원장에서 이 셀의 행을 기계가 뽑아 기동 시도로 묶은 것이고, 마지막은 측정한 실행의 엔진 로그가 echo 한 env 다(측정 당시 값의 정본).

<!-- FACT:reproduce -->
| 순서 | 단계 | 성공 판정 | 소요 | 관측 구간(UTC) | 소요 출처 | 명령 출처 |
|---|---|---|---|---|---|---|
| 1 | render | 러너 3종·.env·.env.cluster·.env.interconnect 생성(렌더러 fail-loud) | 미관측 | 미관측 | .claude/skills/upstream-version-watch/scripts/multinode_serve_smoke.sh 선행조건 안내 · .claude/skills/upstream-version-watch/scripts/render_dockerfile.py CLI | derived(렌더러 CLI) |
| 2 | build | 빌드 OK · 이미지 실재 | 층 창 38분 19초(docker history 첫 층 → 끝 층 CreatedAt · 캐시 재사용 층 포함 — 빌드 소요 아님) | 2026-10-14T01:07:00Z → 2026-10-14T01:45:19Z (layer-window) | docker history --no-trunc --format '{{json .}}' sha256:339737df6571ce00993e731ef064a8db66085beaa650e4aa63bdb24f414b2034 · 우리 Dockerfile 층 24개의 CreatedAt 첫 층 → 끝 층(캐시 재사용 층은 이전 빌드 시각 — 빌드 소요 아님) | reconstructed(build-ledger build_args + compose build) |
| 3 | serve | health 200 + 추론 1회(스모크 판정) | ≤ 14분 25초(상한) | 2026-10-25T02:43:38Z → 2026-10-25T02:58:03Z (upper) | docs/logs/main/events/2026-10.jsonl budget_declare(label smoke-solar31-nv4-1m-tp2) → 첫 측정 시각(output/multi/benchlog/sweep_solar31-nv4-1m-tp2/lite_raw_solar31-nv4-1m-tp2.json measured_utc(호스트 date -u · lite_bench.sh 가 cold 레그 직전에 찍는 측정 시작)) | derived(스모크 사용법) |
| 4 | bench | 리포트 발행(+PASS 면 인증서) | 15분 27초 | 2026-10-25T02:58:03Z → 2026-10-25T03:13:30Z (exact) | output/multi/benchlog/sweep_solar31-nv4-1m-tp2/lite_raw_solar31-nv4-1m-tp2.json measured_utc(호스트 date -u · lite_bench.sh 가 cold 레그 직전에 찍는 측정 시작) → sweep_index.generated_utc(호스트 UTC · 두 끝 같은 시계) | reconstructed(bench json) |

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
bash .claude/skills/upstream-version-watch/scripts/multinode_serve_smoke.sh solar31-nv4-1m-tp2 --keep-up
```

**4. bench**

```sh
# bench JSON 필드로 복원: backend · model_id→--model · tokenizer_id→--tokenizer · num_prompts · max_concurrency · request_rate · burstiness · 요청당 입출력 길이 = total_*_tokens ÷ completed(나누어떨어질 때만 — 평균이다 · 요청별 길이는 JSON 에 없다) · 나누어떨어지면 `--dataset-name random` 으로 추정했다(측정 도구가 그 꼴로 부른다 — 아래 도구 원문).
# JSON 에 없는 인자(--endpoint · --base-url · --ignore-eos · --num-warmups · --temperature · --seed · --random-range-ratio · --trust-remote-code)는 측정 도구가 정했다 — level_* = run_bench.sh · lite_cold/lite_warm = lite_bench.sh(.claude/skills/adversarial-benchmark/scripts/) · 셀 스윕 순서는 sweep_bench.sh.
# output/multi/benchlog/sweep_solar31-nv4-1m-tp2/lite_cold_solar31-nv4-1m-tp2.json (date 20261025-025826) · ⚠ --random-input-len/--random-output-len = total ÷ completed 평균(chat 템플릿 포함 실측 토큰 · 측정 도구 원문 인자 미관측 — lite_bench.sh 원문 인자와 다를 수 있다)
vllm bench serve --backend openai-chat --model solar-open2-250b --tokenizer /app/quant_models/Upstage/Solar-Open2-250B-Nota-NVFP4 --dataset-name random --random-input-len 521 --random-output-len 128 --num-prompts 1 --max-concurrency 1 --request-rate inf
# output/multi/benchlog/sweep_solar31-nv4-1m-tp2/lite_warm_solar31-nv4-1m-tp2.json (date 20261025-025907) · ⚠ --random-input-len/--random-output-len = total ÷ completed 평균(chat 템플릿 포함 실측 토큰 · 측정 도구 원문 인자 미관측 — lite_bench.sh 원문 인자와 다를 수 있다)
vllm bench serve --backend openai-chat --model solar-open2-250b --tokenizer /app/quant_models/Upstage/Solar-Open2-250B-Nota-NVFP4 --dataset-name random --random-input-len 521 --random-output-len 128 --num-prompts 3 --max-concurrency 1 --request-rate inf
```

**슬롯 → 빌드 컨텍스트 매핑**(빌드 컨텍스트 = `output/<토폴로지>/` · 원천 = 실린 Dockerfile 의 COPY · compose 의 env_file · volumes · command — `—` 는 빌드 컨텍스트 입력이 아니다) — ⚠ 빌드 레시피 · 빌드 패치 행은 다른 컨텍스트 `output/single/`(이 이미지의 레시피 통로 · 2단계 docker build 와 같은 자리 · 자리 칸에 그 뿌리를 붙였다)

| zip 경로 | 빌드 컨텍스트 자리 | 근거 |
|---|---|---|
| `artifacts/triplet/.env.solar31-nv4-1m-tp2` | `envs/.env.solar31-nv4-1m-tp2` | compose env_file envs/.env.${CONFIG_FILE}(셀 env) |
| `artifacts/triplet/solar31-nv4-1m-tp2.sh` | `configs/solar31-nv4-1m-tp2.sh` | compose volumes ./configs:/app/configs |
| `artifacts/triplet/solar31-nv4-1m-tp2.yaml` | `configs/solar31-nv4-1m-tp2.yaml` | compose volumes ./configs:/app/configs |
| `artifacts/runtime_patch/solar31-nv4-1m-tp2_patch.py` | `configs/solar31-nv4-1m-tp2_patch.py` | compose volumes ./configs:/app/configs |
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

**기동 성공 판정(관측 · 발행 자격)**: health 200 = true · 추론 1회 = true · 근거 `output/multi/benchlog/serve_proof_solar31-nv4-1m-tp2.json` — 시점 범위: 작성 2026-10-25T02:57:42Z(output/multi/benchlog/serve_proof_solar31-nv4-1m-tp2.json 파일 mtime(파일시스템 관측 — 문서에 시각 칸이 없다 · 기록 필드 아님)) · 측정 창 대비 작성 2026-10-25T02:57:42Z < 측정 시작 2026-10-25T02:58:03Z — 측정 전(빌드·스모크)의 관측
<!-- /FACT:reproduce -->

<!-- FACT:event_timeline -->
**블랙박스 이벤트 74행**(원장에서 이 셀 label·config 에 맞는 행만 · 시각순 · 기계 발췌)

> **캠페인 경계 미관측** — 재생 모드(캠페인 밖 측정(측정 시점 ACTIVE=_bootstrap · 이 측정의 캠페인 인스턴스 없음)) — 캠페인 선언을 관측하지 못한다 · 경계 미관측 — 라벨(셀 이름)로 묶은 행 전부를 이 셀의 행으로 센다(같은 셀 이름의 앞선 캠페인 행이 섞였을 수 있다)

> **원장 관측 범위**(노드별 첫 행 → 마지막 행 · evidence.event_ledger_spans): main = 2026-07-31T03:27:43Z → 2026-10-25T03:13:55Z(`docs/logs/main/events/2026-07.jsonl` · `docs/logs/main/events/2026-08.jsonl` · `docs/logs/main/events/2026-09.jsonl` · `docs/logs/main/events/2026-10.jsonl` · `docs/logs/main/events/thermal.jsonl` · `docs/logs/main/events/watchdog.jsonl`) · sub = 2026-06-13T22:25:01Z → 2026-10-24T00:02:35Z(`sync_staging/sub_docs/logs/sub/events/2026-06.jsonl` · `sync_staging/sub_docs/logs/sub/events/2026-07.jsonl` · `sync_staging/sub_docs/logs/sub/events/2026-08.jsonl` · `sync_staging/sub_docs/logs/sub/events/2026-09.jsonl` · `sync_staging/sub_docs/logs/sub/events/2026-10.jsonl` · `sync_staging/sub_docs/logs/sub/events/thermal.jsonl` · `sync_staging/sub_docs/logs/sub/events/watchdog.jsonl` · 회수 미러가 가린 옛 로컬 사본 `docs/logs/sub/events/2026-09.jsonl`) — ⚠ sub(마지막 행 2026-10-24T00:02:35Z < 측정 끝 2026-10-25T03:13:30Z): 그 뒤 그 노드의 선언 · 사건은 **관측 범위 밖**이다(없음이 아니다 · 원장 미회수)

| # | UTC | 노드 | kind | label | 내용 | 출처 |
|---|---|---|---|---|---|---|
| 1 | 2026-10-24T05:57:40Z | main | `budget_declare` | `smoke-solar31-nv4-1m-tp2` | 예산 선언(선언값 — 측정 아님) · floor_mib=12368 · weights_mib=73104 · kv_mib=14336 · overhead_mib=24800 · mem_total_mib=124608 · 기동 창 1/11: budget_renew 0회 · 닫힘 budget_clear 2026-10-24T05:59:11Z(선언 후 1m31s) · measured_utc 2026-10-25T03:13:30Z 는 이 창 밖(다른 기동 · 측정 전) | docs/logs/main/events/2026-10.jsonl:522 |
| 2 | 2026-10-24T05:57:40Z | main | `budget_honored` | `smoke-solar31-nv4-1m-tp2` | budget_honored · floor_mib=12368 · arm_ceiling_mib=9296 · remaining_s=7200 · 창 시작 2026-10-24T05:57:40Z 후 0m00s | docs/logs/main/events/watchdog.jsonl:756 |
| 3 | 2026-10-24T05:59:11Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-solar31-nv4-1m-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-24T05:57:40Z 후 1m31s) | docs/logs/main/events/2026-10.jsonl:523 |
| 4 | 2026-10-24T05:59:11Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-solar31-nv4-1m-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-24T05:57:40Z 후 1m31s) | docs/logs/main/events/watchdog.jsonl:757 |
| 5 | 2026-10-24T06:00:00Z | main | `budget_declare` | `smoke-solar31-nv4-1m-tp2` | 예산 선언(선언값 — 측정 아님) · floor_mib=12368 · weights_mib=73104 · kv_mib=14336 · overhead_mib=24800 · mem_total_mib=124608 · 기동 창 2/11: budget_renew 0회 · 닫힘 budget_clear 2026-10-24T06:04:36Z(선언 후 4m36s) · measured_utc 2026-10-25T03:13:30Z 는 이 창 밖(다른 기동 · 측정 전) | docs/logs/main/events/2026-10.jsonl:525 |
| 6 | 2026-10-24T06:00:01Z | main | `budget_honored` | `smoke-solar31-nv4-1m-tp2` | budget_honored · floor_mib=12368 · arm_ceiling_mib=9296 · remaining_s=7199 · 창 시작 2026-10-24T06:00:01Z 후 0m00s | docs/logs/main/events/watchdog.jsonl:758 |
| 7 | 2026-10-24T06:04:36Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-solar31-nv4-1m-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-24T06:00:00Z 후 4m36s) | docs/logs/main/events/2026-10.jsonl:526 |
| 8 | 2026-10-24T06:04:36Z | main | `budget_rejected` | — | budget_rejected · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-solar31-nv4-1m-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-24T06:00:01Z 후 4m35s) | docs/logs/main/events/watchdog.jsonl:759 |
| 9 | 2026-10-24T06:04:37Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-solar31-nv4-1m-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-24T06:00:01Z 후 4m36s) | docs/logs/main/events/watchdog.jsonl:760 |
| 10 | 2026-10-24T06:05:00Z | main | `budget_declare` | `smoke-solar31-nv4-1m-tp2` | 예산 선언(선언값 — 측정 아님) · floor_mib=12368 · weights_mib=73104 · kv_mib=14336 · overhead_mib=24800 · mem_total_mib=124608 · 기동 창 3/11: budget_renew 0회 · 닫힘 budget_clear 2026-10-24T06:10:17Z(선언 후 5m17s) · measured_utc 2026-10-25T03:13:30Z 는 이 창 밖(다른 기동 · 측정 전) | docs/logs/main/events/2026-10.jsonl:528 |
| 11 | 2026-10-24T06:05:00Z | main | `budget_honored` | `smoke-solar31-nv4-1m-tp2` | budget_honored · floor_mib=12368 · arm_ceiling_mib=9296 · remaining_s=7200 · 창 시작 2026-10-24T06:05:00Z 후 0m00s | docs/logs/main/events/watchdog.jsonl:761 |
| 12 | 2026-10-24T06:10:17Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-solar31-nv4-1m-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-24T06:05:00Z 후 5m17s) | docs/logs/main/events/2026-10.jsonl:529 |
| 13 | 2026-10-24T06:10:18Z | main | `budget_declare` | `smoke-solar31-nv4-1m-tp2` | 예산 선언(선언값 — 측정 아님) · floor_mib=12368 · weights_mib=73104 · kv_mib=14336 · overhead_mib=24800 · mem_total_mib=124608 · 기동 창 4/11: budget_renew 1회 · 닫힘 budget_clear 2026-10-24T06:21:33Z(선언 후 11m15s) · measured_utc 2026-10-25T03:13:30Z 는 이 창 밖(다른 기동 · 측정 전) | docs/logs/main/events/2026-10.jsonl:530 |
| 14 | 2026-10-24T06:10:18Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-solar31-nv4-1m-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-24T06:05:00Z 후 5m18s) | docs/logs/main/events/watchdog.jsonl:762 |
| 15 | 2026-10-24T06:10:19Z | main | `budget_honored` | `smoke-solar31-nv4-1m-tp2` | budget_honored · floor_mib=12368 · arm_ceiling_mib=9296 · remaining_s=7199 · 창 시작 2026-10-24T06:10:19Z 후 0m00s | docs/logs/main/events/watchdog.jsonl:763 |
| 16 | 2026-10-24T06:11:22Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=84704 · rate_mib_s=24755 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → smoke-solar31-nv4-1m-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-24T06:10:19Z 후 1m03s) | docs/logs/main/events/watchdog.jsonl:764 |
| 17 | 2026-10-24T06:19:39Z | main | `budget_renew` | `smoke-solar31-nv4-1m-tp2` | budget_renew · floor_mib=12368 · ttl_s=7200 · remaining_before_s=6639 · 창 시작 2026-10-24T06:10:18Z 후 9m21s | docs/logs/main/events/2026-10.jsonl:531 |
| 18 | 2026-10-24T06:21:33Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-solar31-nv4-1m-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-24T06:10:18Z 후 11m15s) | docs/logs/main/events/2026-10.jsonl:532 |
| 19 | 2026-10-24T06:21:33Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-solar31-nv4-1m-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-24T06:10:19Z 후 11m14s) | docs/logs/main/events/watchdog.jsonl:765 |
| 20 | 2026-10-24T06:22:22Z | main | `budget_declare` | `smoke-solar31-nv4-1m-tp2` | 예산 선언(선언값 — 측정 아님) · floor_mib=12368 · weights_mib=73104 · kv_mib=14336 · overhead_mib=24800 · mem_total_mib=124608 · 기동 창 5/11: budget_renew 1회 · 닫힘 budget_clear 2026-10-24T06:49:21Z(선언 후 26m59s) · measured_utc 2026-10-25T03:13:30Z 는 이 창 밖(다른 기동 · 측정 전) | docs/logs/main/events/2026-10.jsonl:534 |
| 21 | 2026-10-24T06:22:22Z | main | `budget_honored` | `smoke-solar31-nv4-1m-tp2` | budget_honored · floor_mib=12368 · arm_ceiling_mib=9296 · remaining_s=7200 · 창 시작 2026-10-24T06:22:22Z 후 0m00s | docs/logs/main/events/watchdog.jsonl:766 |
| 22 | 2026-10-24T06:23:37Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=78961 · rate_mib_s=24612 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → smoke-solar31-nv4-1m-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-24T06:22:22Z 후 1m15s) | docs/logs/main/events/watchdog.jsonl:767 |
| 23 | 2026-10-24T06:37:16Z | main | `budget_renew` | `smoke-solar31-nv4-1m-tp2` | budget_renew · floor_mib=12368 · ttl_s=7200 · remaining_before_s=6306 · 창 시작 2026-10-24T06:22:22Z 후 14m54s | docs/logs/main/events/2026-10.jsonl:535 |
| 24 | 2026-10-24T06:49:21Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-solar31-nv4-1m-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-24T06:22:22Z 후 26m59s) | docs/logs/main/events/2026-10.jsonl:536 |
| 25 | 2026-10-24T06:49:22Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-solar31-nv4-1m-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-24T06:22:22Z 후 27m00s) | docs/logs/main/events/watchdog.jsonl:768 |
| 26 | 2026-10-24T06:50:10Z | main | `budget_declare` | `smoke-solar31-nv4-1m-tp2` | 예산 선언(선언값 — 측정 아님) · floor_mib=12368 · weights_mib=73104 · kv_mib=14336 · overhead_mib=24800 · mem_total_mib=124608 · 기동 창 6/11: budget_renew 4회 · 닫힘 budget_clear 2026-10-24T08:48:13Z(선언 후 1h58m03s) · measured_utc 2026-10-25T03:13:30Z 는 이 창 밖(다른 기동 · 측정 전) | docs/logs/main/events/2026-10.jsonl:538 |
| 27 | 2026-10-24T06:50:11Z | main | `budget_honored` | `smoke-solar31-nv4-1m-tp2` | budget_honored · floor_mib=12368 · arm_ceiling_mib=9296 · remaining_s=7199 · 창 시작 2026-10-24T06:50:11Z 후 0m00s | docs/logs/main/events/watchdog.jsonl:769 |
| 28 | 2026-10-24T06:51:25Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=72934 · rate_mib_s=25208 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → smoke-solar31-nv4-1m-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-24T06:50:11Z 후 1m14s) | docs/logs/main/events/watchdog.jsonl:770 |
| 29 | 2026-10-24T07:05:33Z | main | `budget_renew` | `smoke-solar31-nv4-1m-tp2` | budget_renew · floor_mib=12368 · ttl_s=7200 · remaining_before_s=6277 · 창 시작 2026-10-24T06:50:10Z 후 15m23s | docs/logs/main/events/2026-10.jsonl:539 |
| 30 | 2026-10-24T07:35:33Z | main | `budget_renew` | `smoke-solar31-nv4-1m-tp2` | budget_renew · floor_mib=12368 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-24T06:50:10Z 후 45m23s | docs/logs/main/events/2026-10.jsonl:540 |
| 31 | 2026-10-24T08:05:33Z | main | `budget_renew` | `smoke-solar31-nv4-1m-tp2` | budget_renew · floor_mib=12368 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-24T06:50:10Z 후 1h15m23s | docs/logs/main/events/2026-10.jsonl:541 |
| 32 | 2026-10-24T08:35:33Z | main | `budget_renew` | `smoke-solar31-nv4-1m-tp2` | budget_renew · floor_mib=12368 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-24T06:50:10Z 후 1h45m23s | docs/logs/main/events/2026-10.jsonl:542 |
| 33 | 2026-10-24T08:48:13Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-solar31-nv4-1m-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-24T06:50:10Z 후 1h58m03s) | docs/logs/main/events/2026-10.jsonl:543 |
| 34 | 2026-10-24T08:48:14Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-solar31-nv4-1m-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-24T06:50:11Z 후 1h58m03s) | docs/logs/main/events/watchdog.jsonl:771 |
| 35 | 2026-10-24T14:34:00Z | main | `budget_declare` | `smoke-solar31-nv4-1m-tp2` | 예산 선언(선언값 — 측정 아님) · floor_mib=12368 · weights_mib=73104 · kv_mib=14336 · overhead_mib=24800 · mem_total_mib=124608 · 기동 창 7/11: budget_renew 1회 · 닫힘 budget_clear 2026-10-24T14:47:54Z(선언 후 13m54s) · measured_utc 2026-10-25T03:13:30Z 는 이 창 밖(다른 기동 · 측정 전) | docs/logs/main/events/2026-10.jsonl:571 |
| 36 | 2026-10-24T14:34:01Z | main | `budget_honored` | `smoke-solar31-nv4-1m-tp2` | budget_honored · floor_mib=12368 · arm_ceiling_mib=9296 · remaining_s=7199 · 창 시작 2026-10-24T14:34:01Z 후 0m00s | docs/logs/main/events/watchdog.jsonl:790 |
| 37 | 2026-10-24T14:35:13Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=84886 · rate_mib_s=24390 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → smoke-solar31-nv4-1m-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-24T14:34:01Z 후 1m12s) | docs/logs/main/events/watchdog.jsonl:791 |
| 38 | 2026-10-24T14:46:16Z | main | `budget_renew` | `smoke-solar31-nv4-1m-tp2` | budget_renew · floor_mib=12368 · ttl_s=7200 · remaining_before_s=6464 · 창 시작 2026-10-24T14:34:00Z 후 12m16s | docs/logs/main/events/2026-10.jsonl:572 |
| 39 | 2026-10-24T14:47:54Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-solar31-nv4-1m-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-24T14:34:00Z 후 13m54s) | docs/logs/main/events/2026-10.jsonl:573 |
| 40 | 2026-10-24T14:47:54Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-solar31-nv4-1m-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-24T14:34:01Z 후 13m53s) | docs/logs/main/events/watchdog.jsonl:792 |
| 41 | 2026-10-24T14:48:44Z | main | `budget_declare` | `smoke-solar31-nv4-1m-tp2` | 예산 선언(선언값 — 측정 아님) · floor_mib=12368 · weights_mib=73104 · kv_mib=14336 · overhead_mib=24800 · mem_total_mib=124608 · 기동 창 8/11: budget_renew 1회 · 닫힘 budget_clear 2026-10-24T15:18:23Z(선언 후 29m39s) · measured_utc 2026-10-25T03:13:30Z 는 이 창 밖(다른 기동 · 측정 전) | docs/logs/main/events/2026-10.jsonl:575 |
| 42 | 2026-10-24T14:48:44Z | main | `budget_honored` | `smoke-solar31-nv4-1m-tp2` | budget_honored · floor_mib=12368 · arm_ceiling_mib=9296 · remaining_s=7200 · 창 시작 2026-10-24T14:48:44Z 후 0m00s | docs/logs/main/events/watchdog.jsonl:793 |
| 43 | 2026-10-24T14:49:59Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=85618 · rate_mib_s=24003 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → smoke-solar31-nv4-1m-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-24T14:48:44Z 후 1m15s) | docs/logs/main/events/watchdog.jsonl:794 |
| 44 | 2026-10-24T15:01:57Z | main | `budget_renew` | `smoke-solar31-nv4-1m-tp2` | budget_renew · floor_mib=12368 · ttl_s=7200 · remaining_before_s=6407 · 창 시작 2026-10-24T14:48:44Z 후 13m13s | docs/logs/main/events/2026-10.jsonl:576 |
| 45 | 2026-10-24T15:18:23Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-solar31-nv4-1m-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-24T14:48:44Z 후 29m39s) | docs/logs/main/events/2026-10.jsonl:580 |
| 46 | 2026-10-24T15:18:24Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-solar31-nv4-1m-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-24T14:48:44Z 후 29m40s) | docs/logs/main/events/watchdog.jsonl:795 |
| 47 | 2026-10-24T20:51:16Z | main | `budget_declare` | `smoke-solar31-nv4-1m-tp2` | 예산 선언(선언값 — 측정 아님) · floor_mib=12368 · weights_mib=73104 · kv_mib=14336 · overhead_mib=24800 · mem_total_mib=124608 · 기동 창 9/11: budget_renew 1회 · 닫힘 budget_clear 2026-10-24T21:20:21Z(선언 후 29m05s) · measured_utc 2026-10-25T03:13:30Z 는 이 창 밖(다른 기동 · 측정 전) | docs/logs/main/events/2026-10.jsonl:590 |
| 48 | 2026-10-24T20:51:16Z | main | `budget_honored` | `smoke-solar31-nv4-1m-tp2` | budget_honored · floor_mib=12368 · arm_ceiling_mib=9296 · remaining_s=7200 · 창 시작 2026-10-24T20:51:16Z 후 0m00s | docs/logs/main/events/watchdog.jsonl:802 |
| 49 | 2026-10-24T20:52:30Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=81212 · rate_mib_s=26568 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → smoke-solar31-nv4-1m-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-24T20:51:16Z 후 1m14s) | docs/logs/main/events/watchdog.jsonl:803 |
| 50 | 2026-10-24T21:04:14Z | main | `budget_renew` | `smoke-solar31-nv4-1m-tp2` | budget_renew · floor_mib=12368 · ttl_s=7200 · remaining_before_s=6422 · 창 시작 2026-10-24T20:51:16Z 후 12m58s | docs/logs/main/events/2026-10.jsonl:591 |
| 51 | 2026-10-24T21:20:21Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-solar31-nv4-1m-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-24T20:51:16Z 후 29m05s) | docs/logs/main/events/2026-10.jsonl:592 |
| 52 | 2026-10-24T21:20:22Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-solar31-nv4-1m-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-24T20:51:16Z 후 29m06s) | docs/logs/main/events/watchdog.jsonl:804 |
| 53 | 2026-10-25T02:28:01Z | main | `budget_declare` | `smoke-solar31-nv4-1m-tp2` | 예산 선언(선언값 — 측정 아님) · floor_mib=12368 · weights_mib=73104 · kv_mib=14336 · overhead_mib=24800 · mem_total_mib=124608 · 기동 창 10/11: budget_renew 1회 · 닫힘 budget_clear 2026-10-25T02:42:44Z(선언 후 14m43s) · measured_utc 2026-10-25T03:13:30Z 는 이 창 밖(다른 기동 · 측정 전) | docs/logs/main/events/2026-10.jsonl:650 |
| 54 | 2026-10-25T02:28:02Z | main | `budget_honored` | `smoke-solar31-nv4-1m-tp2` | budget_honored · floor_mib=12368 · arm_ceiling_mib=9296 · remaining_s=7199 · 창 시작 2026-10-25T02:28:02Z 후 0m00s | docs/logs/main/events/watchdog.jsonl:823 |
| 55 | 2026-10-25T02:29:15Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=66994 · rate_mib_s=24859 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → smoke-solar31-nv4-1m-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-25T02:28:02Z 후 1m13s) | docs/logs/main/events/watchdog.jsonl:824 |
| 56 | 2026-10-25T02:40:52Z | main | `budget_renew` | `smoke-solar31-nv4-1m-tp2` | budget_renew · floor_mib=12368 · ttl_s=7200 · remaining_before_s=6429 · 창 시작 2026-10-25T02:28:01Z 후 12m51s | docs/logs/main/events/2026-10.jsonl:651 |
| 57 | 2026-10-25T02:42:44Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-solar31-nv4-1m-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-25T02:28:01Z 후 14m43s) | docs/logs/main/events/2026-10.jsonl:652 |
| 58 | 2026-10-25T02:42:45Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-solar31-nv4-1m-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-25T02:28:02Z 후 14m43s) | docs/logs/main/events/watchdog.jsonl:825 |
| 59 | 2026-10-25T02:43:38Z | main | `budget_declare` | `smoke-solar31-nv4-1m-tp2` | 예산 선언(선언값 — 측정 아님) · floor_mib=12368 · weights_mib=73104 · kv_mib=14336 · overhead_mib=24800 · mem_total_mib=124608 · 기동 창 11/11: budget_renew 4회 · 닫힘 budget_clear 2026-10-25T03:13:54Z(선언 후 30m16s) · measured_utc 2026-10-25T03:13:30Z 가 이 창 안(이 측정의 기동) | docs/logs/main/events/2026-10.jsonl:654 |
| 60 | 2026-10-25T02:43:38Z | main | `budget_honored` | `smoke-solar31-nv4-1m-tp2` | budget_honored · floor_mib=12368 · arm_ceiling_mib=9296 · remaining_s=7200 · 창 시작 2026-10-25T02:43:38Z 후 0m00s | docs/logs/main/events/watchdog.jsonl:826 |
| 61 | 2026-10-25T02:44:48Z | cluster | `engine_log_window_start` | — | 캡처된 엔진 로그의 첫 시각(꼬리 캡처일 수 있다 — 엔진 시작 시각이 아닐 수 있다) · 선언 2026-10-25T02:43:38Z 후 1m10s · 시계: 컨테이너 시각을 UTC 로 읽어 측정 창 [2026-10-25T02:43:38Z, 2026-10-25T03:13:54Z] 안에 드는 것을 확인 | output/multi/benchlog/sweep_solar31-nv4-1m-tp2/lite_engine_solar31-nv4-1m-tp2.log:415 |
| 62 | 2026-10-25T02:44:53Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=70137 · rate_mib_s=26985 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → smoke-solar31-nv4-1m-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-25T02:43:38Z 후 1m15s) | docs/logs/main/events/watchdog.jsonl:827 |
| 63 | 2026-10-25T02:44:54Z | cluster | `engine_prefetch_disabled` | — | 가중치 auto-prefetch 꺼짐 줄 ×2 — filesystem CIFS(vLLM 로그 문구) | output/multi/benchlog/sweep_solar31-nv4-1m-tp2/lite_engine_solar31-nv4-1m-tp2.log:547 |
| 64 | 2026-10-25T02:54:47Z | cluster | `engine_weights_loaded` | — | 가중치 적재 완료 줄 ×2(rank·단계별) · 최장 592.59s · 이 행 = 마지막 줄 | output/multi/benchlog/sweep_solar31-nv4-1m-tp2/lite_engine_solar31-nv4-1m-tp2.log:700 |
| 65 | 2026-10-25T02:55:10Z | cluster | `engine_model_loaded` | — | 모델 적재 완료 줄 ×2(rank 별) · 마지막 줄 71.6 GiB · 619.7s | output/multi/benchlog/sweep_solar31-nv4-1m-tp2/lite_engine_solar31-nv4-1m-tp2.log:703 |
| 66 | 2026-10-25T02:56:10Z | cluster | `engine_kv_cache_sized` | — | GPU KV cache 1,206,177 tokens · 요청당 1,048,576 tokens 기준 최대 동시성 1.15x | output/multi/benchlog/sweep_solar31-nv4-1m-tp2/lite_engine_solar31-nv4-1m-tp2.log:710 |
| 67 | 2026-10-25T02:56:47Z | cluster | `engine_init_done` | — | 엔진 초기화(profile·KV 할당·warmup) 95.69s | output/multi/benchlog/sweep_solar31-nv4-1m-tp2/lite_engine_solar31-nv4-1m-tp2.log:723 |
| 68 | 2026-10-25T02:57:07Z | cluster | `server_starting` | — | API 서버 기동 — 이 측정 창 선언 2026-10-25T02:43:38Z 후 13m29s · 캡처 로그 첫 시각 2026-10-25T02:44:48Z 후 12m19s | output/multi/benchlog/sweep_solar31-nv4-1m-tp2/lite_engine_solar31-nv4-1m-tp2.log:736 |
| 69 | 2026-10-25T02:57:42Z | main | `budget_renew` | `smoke-solar31-nv4-1m-tp2` | budget_renew · floor_mib=12368 · ttl_s=7200 · remaining_before_s=6356 · 창 시작 2026-10-25T02:43:38Z 후 14m04s | docs/logs/main/events/2026-10.jsonl:655 |
| 70 | 2026-10-25T02:59:09Z | main | `budget_renew` | `smoke-solar31-nv4-1m-tp2` | budget_renew · floor_mib=12368 · ttl_s=7200 · remaining_before_s=7113 · 창 시작 2026-10-25T02:43:38Z 후 15m31s | docs/logs/main/events/2026-10.jsonl:656 |
| 71 | 2026-10-25T03:03:58Z | main | `budget_renew` | `smoke-solar31-nv4-1m-tp2` | budget_renew · floor_mib=12368 · ttl_s=7200 · remaining_before_s=6911 · 창 시작 2026-10-25T02:43:38Z 후 20m20s | docs/logs/main/events/2026-10.jsonl:657 |
| 72 | 2026-10-25T03:08:44Z | main | `budget_renew` | `smoke-solar31-nv4-1m-tp2` | budget_renew · floor_mib=12368 · ttl_s=7200 · remaining_before_s=6914 · 창 시작 2026-10-25T02:43:38Z 후 25m06s | docs/logs/main/events/2026-10.jsonl:658 |
| 73 | 2026-10-25T03:13:54Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-solar31-nv4-1m-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-25T02:43:38Z 후 30m16s) | docs/logs/main/events/2026-10.jsonl:659 |
| 74 | 2026-10-25T03:13:55Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-solar31-nv4-1m-tp2 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-25T02:43:38Z 후 30m17s) | docs/logs/main/events/watchdog.jsonl:828 |

**기동 시도**(예산 선언 `budget_declare` 마다 한 시도 · 같은 노드의 이어지는 행과 선언 없는 노드(엔진 로그)의 그 시각 행을 묶었다 · 사살 · 트립 · 거부는 닫힘과 따로 센다 · `measurement` 행은 측정 기록이라 시도로 세지 않는다(위 표에만) — 해석은 저작자 몫)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 1 | main | `smoke-solar31-nv4-1m-tp2` | 2026-10-24T05:57:40Z | 없음 | 없음 | `budget_clear` 2026-10-24T05:59:11Z | 2 |
| 2 | main | `smoke-solar31-nv4-1m-tp2` | 2026-10-24T06:00:00Z | 없음 | `budget_rejected` 1 | `budget_clear` 2026-10-24T06:04:36Z | 2 |
| 3 | main | `smoke-solar31-nv4-1m-tp2` | 2026-10-24T06:05:00Z | 없음 | 없음 | `budget_clear` 2026-10-24T06:10:17Z | 1 |
| 4 | main | `smoke-solar31-nv4-1m-tp2` | 2026-10-24T06:10:18Z | 1회 · 첫 2026-10-24T06:19:39Z | 없음 | `budget_clear` 2026-10-24T06:21:33Z | 4 |
| 5 | main | `smoke-solar31-nv4-1m-tp2` | 2026-10-24T06:22:22Z | 1회 · 첫 2026-10-24T06:37:16Z | 없음 | `budget_clear` 2026-10-24T06:49:21Z | 3 |
| 6 | main | `smoke-solar31-nv4-1m-tp2` | 2026-10-24T06:50:10Z | 4회 · 첫 2026-10-24T07:05:33Z | 없음 | `budget_clear` 2026-10-24T08:48:13Z | 3 |
| 7 | main | `smoke-solar31-nv4-1m-tp2` | 2026-10-24T14:34:00Z | 1회 · 첫 2026-10-24T14:46:16Z | 없음 | `budget_clear` 2026-10-24T14:47:54Z | 3 |
| 8 | main | `smoke-solar31-nv4-1m-tp2` | 2026-10-24T14:48:44Z | 1회 · 첫 2026-10-24T15:01:57Z | 없음 | `budget_clear` 2026-10-24T15:18:23Z | 3 |
| 9 | main | `smoke-solar31-nv4-1m-tp2` | 2026-10-24T20:51:16Z | 1회 · 첫 2026-10-24T21:04:14Z | 없음 | `budget_clear` 2026-10-24T21:20:21Z | 3 |
| 10 | main | `smoke-solar31-nv4-1m-tp2` | 2026-10-25T02:28:01Z | 1회 · 첫 2026-10-25T02:40:52Z | 없음 | `budget_clear` 2026-10-25T02:42:44Z | 3 |
| 11 | main | `smoke-solar31-nv4-1m-tp2` | 2026-10-25T02:43:38Z | 4회 · 첫 2026-10-25T02:57:42Z | 없음 | `budget_clear` 2026-10-25T03:13:54Z | 10 |
<!-- /FACT:event_timeline -->

<!-- FACT:measurement_env -->
**측정 실행의 env 관측 29행**(측정한 실행의 엔진 로그가 echo 한 값 · 기계 발췌 — 측정 당시 값의 정본이다. 표에 없는 키는 로그가 echo 하지 않은 것이지 설정되지 않았다는 뜻이 아니다 · 측정 뒤 재생성된 env 형상은 01 §1.1 을 본다)

| # | 키 | 값 | 노드 | 종류 | 출처(첫 줄) | 관측 |
|---|---|---|---|---|---|---|
| 1 | `NCCL_SOCKET_IFNAME` | `<nic:cluster>` | main | env-echo | output/multi/benchlog/sweep_solar31-nv4-1m-tp2/lite_engine_head_solar31-nv4-1m-tp2.log:L94 | 로그 1개 |
| 2 | `NCCL_SOCKET_IFNAME` | `<nic:cluster>` | main | param-echo | output/multi/benchlog/sweep_solar31-nv4-1m-tp2/lite_engine_head_solar31-nv4-1m-tp2.log:L95 | 로그 1개 |
| 3 | `nccl:version` | `2.30.7+cuda13.3` | main | runtime | output/multi/benchlog/sweep_solar31-nv4-1m-tp2/lite_engine_head_solar31-nv4-1m-tp2.log:L98 | 로그 1개 |
| 4 | `NCCL_NET_PLUGIN` | `spcx` | main | env-echo | output/multi/benchlog/sweep_solar31-nv4-1m-tp2/lite_engine_head_solar31-nv4-1m-tp2.log:L100 | 로그 1개 |
| 5 | `NCCL_IB_DISABLE` | `0` | main | env-echo | output/multi/benchlog/sweep_solar31-nv4-1m-tp2/lite_engine_head_solar31-nv4-1m-tp2.log:L105 | 로그 1개 |
| 6 | `NCCL_IB_HCA` | `=<nic:cluster>,<nic:cluster>` | main | param-echo | output/multi/benchlog/sweep_solar31-nv4-1m-tp2/lite_engine_head_solar31-nv4-1m-tp2.log:L107 | 로그 1개 |
| 7 | `NCCL_IB_MERGE_NICS` | `1` | main | env-echo | output/multi/benchlog/sweep_solar31-nv4-1m-tp2/lite_engine_head_solar31-nv4-1m-tp2.log:L110 | 로그 1개 |
| 8 | `nccl:net_ib_devices` | `[0]<nic:cluster>:1/RoCE [1]<nic:cluster>:1/RoCE [RO]; OOB <nic:cluster>:<node:main><0>` | main | runtime | output/multi/benchlog/sweep_solar31-nv4-1m-tp2/lite_engine_head_solar31-nv4-1m-tp2.log:L113 | 로그 1개 |
| 9 | `NCCL_IB_QPS_PER_CONNECTION` | `4` | main | env-echo | output/multi/benchlog/sweep_solar31-nv4-1m-tp2/lite_engine_head_solar31-nv4-1m-tp2.log:L126 | 로그 1개 |
| 10 | `nccl:net_ib_devices` | `[RO]; OOB <nic:cluster>:<node:main><0>` | main | runtime | output/multi/benchlog/sweep_solar31-nv4-1m-tp2/lite_engine_head_solar31-nv4-1m-tp2.log:L137 | 로그 1개 |
| 11 | `nccl:network` | `IB` | main | runtime | output/multi/benchlog/sweep_solar31-nv4-1m-tp2/lite_engine_head_solar31-nv4-1m-tp2.log:L149 | 로그 1개 |
| 12 | `NCCL_DMABUF_ENABLE` | `0` | main | env-echo | output/multi/benchlog/sweep_solar31-nv4-1m-tp2/lite_engine_head_solar31-nv4-1m-tp2.log:L151 | 로그 1개 |
| 13 | `NCCL_CROSS_NIC` | `1` | main | env-echo | output/multi/benchlog/sweep_solar31-nv4-1m-tp2/lite_engine_head_solar31-nv4-1m-tp2.log:L154 | 로그 1개 |
| 14 | `NCCL_IB_SPLIT_DATA_ON_QPS` | `0` | main | env-echo | output/multi/benchlog/sweep_solar31-nv4-1m-tp2/lite_engine_head_solar31-nv4-1m-tp2.log:L598 | 로그 2개 |
| 15 | `NCCL_IB_GID_INDEX` | `3` | main | env-echo | output/multi/benchlog/sweep_solar31-nv4-1m-tp2/lite_engine_solar31-nv4-1m-tp2.log:L181 | 로그 1개 |
| 16 | `NCCL_SOCKET_IFNAME` | `<nic:cluster>` | sub | env-echo | output/multi/benchlog/sweep_solar31-nv4-1m-tp2/lite_engine_solar31-nv4-1m-tp2.log:L550 | 로그 1개 · Ray 접힘 +3(접힌 사본의 노드 미관측) |
| 17 | `NCCL_SOCKET_IFNAME` | `<nic:cluster>` | sub | param-echo | output/multi/benchlog/sweep_solar31-nv4-1m-tp2/lite_engine_solar31-nv4-1m-tp2.log:L551 | 로그 1개 · Ray 접힘 +2(접힌 사본의 노드 미관측) |
| 18 | `nccl:version` | `2.30.7+cuda13.3` | sub | runtime | output/multi/benchlog/sweep_solar31-nv4-1m-tp2/lite_engine_solar31-nv4-1m-tp2.log:L554 | 로그 1개 |
| 19 | `NCCL_NET_PLUGIN` | `spcx` | sub | env-echo | output/multi/benchlog/sweep_solar31-nv4-1m-tp2/lite_engine_solar31-nv4-1m-tp2.log:L556 | 로그 1개 |
| 20 | `NCCL_IB_DISABLE` | `0` | sub | env-echo | output/multi/benchlog/sweep_solar31-nv4-1m-tp2/lite_engine_solar31-nv4-1m-tp2.log:L561 | 로그 1개 · Ray 접힘 +2(접힌 사본의 노드 미관측) |
| 21 | `NCCL_IB_HCA` | `=<nic:cluster>,<nic:cluster>` | sub | param-echo | output/multi/benchlog/sweep_solar31-nv4-1m-tp2/lite_engine_solar31-nv4-1m-tp2.log:L562 | 로그 1개 · Ray 접힘 +2(접힌 사본의 노드 미관측) |
| 22 | `NCCL_IB_MERGE_NICS` | `1` | sub | env-echo | output/multi/benchlog/sweep_solar31-nv4-1m-tp2/lite_engine_solar31-nv4-1m-tp2.log:L564 | 로그 1개 |
| 23 | `nccl:net_ib_devices` | `[RO]; OOB <nic:cluster>:<node:sub><0>` | sub | runtime | output/multi/benchlog/sweep_solar31-nv4-1m-tp2/lite_engine_solar31-nv4-1m-tp2.log:L566 | 로그 1개 · Ray 접힘 +2(접힌 사본의 노드 미관측) |
| 24 | `NCCL_IB_QPS_PER_CONNECTION` | `4` | sub | env-echo | output/multi/benchlog/sweep_solar31-nv4-1m-tp2/lite_engine_solar31-nv4-1m-tp2.log:L576 | 로그 1개 · Ray 접힘 +3(접힌 사본의 노드 미관측) |
| 25 | `nccl:network` | `IB` | sub | runtime | output/multi/benchlog/sweep_solar31-nv4-1m-tp2/lite_engine_solar31-nv4-1m-tp2.log:L589 | 로그 1개 · Ray 접힘 +3(접힌 사본의 노드 미관측) |
| 26 | `NCCL_DMABUF_ENABLE` | `0` | sub | env-echo | output/multi/benchlog/sweep_solar31-nv4-1m-tp2/lite_engine_solar31-nv4-1m-tp2.log:L591 | 로그 1개 |
| 27 | `NCCL_CROSS_NIC` | `1` | sub | env-echo | output/multi/benchlog/sweep_solar31-nv4-1m-tp2/lite_engine_solar31-nv4-1m-tp2.log:L594 | 로그 1개 |
| 28 | `NCCL_IB_SPLIT_DATA_ON_QPS` | `0` | sub | env-echo | output/multi/benchlog/sweep_solar31-nv4-1m-tp2/lite_engine_solar31-nv4-1m-tp2.log:L671 | 로그 1개 |
| 29 | `NCCL_IB_GID_INDEX` | `3` | sub | env-echo | output/multi/benchlog/sweep_solar31-nv4-1m-tp2/lite_engine_solar31-nv4-1m-tp2.log:L676 | 로그 1개 |

- 값 주: 기계 치환(pii.substitution_table · 장치·호스트·주소는 자리표시 — 원문은 출처 줄)

- Ray 접힘: 출처 줄 끝의 `[repeated Nx across cluster]` — Ray 가 다른 프로세스의 같은 줄 N개를 한 줄로 접었다. 노드 칸은 **첫 줄**의 노드이고, 접힌 사본이 어느 노드의 것인지는 로그에 없다(관측 대상 밖).

- 로그 창(꼬리): `tail-800(lite_bench.sh@d33b0878a6c3 L234)`(꼬리 창 출처 로그 1개 · 이 표 행 #15–29) — 이 캡처는 엔진 출력의 **마지막 N줄**이다(측정 도구의 `tail -N` 상한에 닿았다 · 생산자 evidence `log_window`). 창 앞에서 echo 한 줄은 그 로그에 없다 — 이 주의는 꼬리 창 출처 행에만 걸린다(머리 캡처 행은 아래).

- 로그 창(머리): `head-600(output/multi/benchlog/sweep_solar31-nv4-1m-tp2/lite_raw_solar31-nv4-1m-tp2.json engine_log_head_lines)`(머리 캡처 출처 로그 1개 · 이 표 행 #1–14) — 이 캡처는 엔진 출력의 **처음 N줄**이다(lite_bench.sh 머리 캡처 `head -n N` 상한에 닿았다 · 생산자 evidence `log_head_window`). 꼬리 창의 주의는 이 행에 걸리지 않는다 · 머리 N줄 뒤에서 echo 한 줄은 그 로그에 없다. 머리 캡처와 꼬리 창 사이(엔진 출력의 가운데)는 어느 캡처에도 없다 — 표에 없는 키 · 한 노드에만 있는 키는 그 사이였을 수 있다.
<!-- /FACT:measurement_env -->

> 이 절이 답하는 질문: 수신자가 이 zip 만으로 이 셀을 다시 띄우려면 어떤 순서로 무엇을 하고, 각 단계의 성공을 무엇으로 판정하며, 얼마나 기다려야 하는가?

**(a) 사전 준비** — 가중치는 스테이징 없이 NAS 마운트를 양 노드가 직접 읽는다: 서빙 yaml `model: /app/quant_models/Upstage/Solar-Open2-250B-Nota-NVFP4` ← 호스트 `<manifest.quant_model_path>/Upstage/Solar-Open2-250B-Nota-NVFP4`(프로젝트 env `QUANT_MODEL_PATH` · 서브에도 같은 경로가 실재해야 한다 — 1.5). 측정 기동의 엔진 로그는 그 파일시스템을 `CIFS` 로 보고 가중치 auto-prefetch 를 껐다(01 §1.4 이벤트 표 63행 · lite_engine L547). 이 모델은 PLE 가 없어 PLE mmap 스테이징은 해당 없다(00 §0.9 후보 표 · config 에 PLE 키 부재). 필수 env 의 자리: 셀 env `artifacts/triplet/.env.solar31-nv4-1m-tp2`(`IMAGE_TAG` · `RUNTIME_PATCH_STEM` 등) · 클러스터 형상 `artifacts/compose/.env.cluster.template` · 인터커넥트 형상 `artifacts/compose/.env.interconnect.template` · 프로젝트 `.env` 형상 `artifacts/compose/.env.template`. 런타임 패치는 `configs/solar31-nv4-1m-tp2_patch.py` 자리에 **양 노드 모두** 둔다(W3). 측정 당시 인터커넥트 값은 위 '측정 실행의 env 관측' 표가 정본이다 — 1단계 render 명령은 지금의 렌더러로 다시 만드는 것이므로, 다시 렌더한 `.env.interconnect` 를 그 표의 값(`NCCL_IB_DISABLE=0` · `NCCL_NET_PLUGIN=spcx` · `NCCL_IB_MERGE_NICS=1` · `NCCL_IB_QPS_PER_CONNECTION=4` · `NCCL_DMABUF_ENABLE=0` · `NCCL_CROSS_NIC=1` · `NCCL_IB_SPLIT_DATA_ON_QPS=0` · `NCCL_IB_GID_INDEX=3` 등)과 맞춘다.

**(b) 빌드** — 양 노드에서 각자 짓는다(이미지 전송 ✗). 선택자는 `Dockerfile.source-build` · build-arg `VLLM_REF=v0.31.0` · `VLLM_REPO=https://github.com/vllm-project/vllm.git` · `TORCH_CUDA_ARCH=12.1a` · `RAY_VERSION=2.48.0` · `BUILD_JOBS=8`(위 명령 블록 · docker history 재구성). 빌드 경로는 위 명령 블록 그대로 `-f output/single/Dockerfile.source-build` · 컨텍스트 `output/single` 이다 — 이 이미지의 레시피 통로가 single-node 브랜치의 `output/single/` 이라 스모크 · compose build(`output/multi/`)는 동등 경로가 아니다(1.2). 실린 `requirements.txt` 는 쓰인 바이트이고 Dockerfile 은 16/17 일치(guard echo RUN 한 단계 차이)다. 이 셀은 빌드하지 않았다 — 측정 이미지는 2026-10-14 에 지어진 공유 이미지이고 Solar 지원은 런타임 패치에 있다. 빌드 소요는 **미관측**이다(위 표의 38분 19초는 층 CreatedAt 창이지 소요가 아니다 · 계보 밖 testlog_26101412 는 main 빌드 40분 · sub 37분이라 적는다).

**(c) 기동 순서와 성공 판정** — master up(Ray head) → slave up(join) → master 가 GPU 2 합류를 확인 → `vllm serve`(1.5 기동 순서). 측정 기동(시도 11)의 관측 타임라인(엔진 로그 · 컨테이너 시계 UTC 가정):
- 02:43:38Z 예산 선언 → 무장 줄 `[arm_patch] runtime patch armed`(lite_engine_head L34 · L55) → `Resolved architecture: SolarOpen2ForCausalLM`(L68).
- 가중치 적재 `Loading weights took` 565.89 s(main · L686) · 592.59 s(sub · L700) → `Model loading took 71.6 GiB` 593.0 s / 619.7 s(L696 · L703).
- KV 할당 `GPU KV cache size: 1,206,177 tokens … 1.15x`(L710) → `init engine … took 95.69 s`(L723) → `Starting vLLM server` 02:57:07Z(L736 · 선언 후 13분 29초).
- serve proof(health 200 + 추론 1회) 작성 02:57:42Z → 측정 시작 02:58:03Z.
기동 총 소요 = 선언 → API 서버 13분 29초(관측). 계보 문서 값은 따로 있다: 이전 구성 가중치 적재 622~649 s(testlog_26102418 §3) · 계보 경유 문서 testlog_26102500 의 S4 구성 READY 760 s(읽기 목록 밖). 패치 발화 판정 서명은 위 무장 줄과 아키텍처 해소 줄이다.
기동 중 무서워 보이지만 이 셀에서 기동을 막지 않은 것: 원장의 `watchdog_highrate_hold`(선언 후 약 1분 · `rate_mib_s` 가 `max_rate_mib_s=20768` 을 넘고 `legacy_rule_would_trip=true` 인 hold 행 — 11번 기동 중 시도 4~11 에 한 번씩 있다 · 뒤따른 사살 0) · `Auto-prefetch is disabled … (CIFS)` · `Your GPU does not have native support for FP4 computation … Marlin kernel` · `SymmMemCommunicator: Device capability 12.1 not supported` · `FlashInfer All Reduce is disabled … world_size=2` · `Custom collectives are disabled … MNNVL` · `tensor_parallel_size=2 is bigger than a reserved number of GPUs (1 GPUs) in a node` · NCCL 의 `Spectrum-X (SPCX) NCCL plugin is not supported on device … skipping` · triton `tl.make_block_ptr is deprecated`. 해석이 남은 경고 하나: `Auto-initialization of reasoning token IDs failed`(L730 · 02 끝 주석).

**(d) 호스트 메모리 안전 예산** — 선언(원장 `budget_declare` · 측정 아님): floor_mib 12368 · weights_mib 73104 · kv_mib 14336 · overhead_mib 24800 · mem_total_mib 124608 — 11번 시도 모두 같다. arm 상한은 `budget_honored` 행의 arm_ceiling_mib 9296 이다. bench 클라이언트 예산의 상한은 arm 상한이 아니라 `floor − abs_band`(이 셀 2128 MiB · budget_preflight.py)이고 2048 MiB 로 선언했다(W8). 실측과 나란히: 엔진 노드당 적재 71.6 GiB · 측정 중 시스템 RAM main 101.2 GiB(83%) · sub 99.3 GiB(82%)(bench_report §lite 지표). 기동 시도는 표대로 11번이며 메인 원장 기준 사살 · 트립은 0, 거부는 시도 2 의 `budget_rejected` 1건이다(워치독 사유 `floor_mib/expires_epoch 파싱 실패` · 같은 초에 그 시도의 `budget_clear` · 사살 아님 — 원인 해석은 미확인). 서브의 사살(W5)은 서브 원장 회수 미러가 2026-10-24T00:02:35Z 에서 끝나 이 표 밖이다. 시도별 원인은 02 §2.2 머리 문단(로그로 확인된 것은 시도 3 = W3 · 시도 7 = W7).

**(e) 측정 명령** — 위 4단계 블록의 두 줄은 lite 레그(cold · warm) JSON 에서 재구성한 `vllm bench serve` 명령이다(실행 원문 아님 · 평균 입력 521 은 chat 템플릿 포함 실측 평균이고 도구 원문 인자는 512/128 — 03 §3.4). full 레그(GuideLLM 0.7.3 · 동시성 1 · 반복 3)의 인자는 bench JSON 이 아니라 측정 도구 원문 `run_bench.sh@29cbdbc89a3d` 에 있다(03 §3.4 발췌).

**(f) 엔진이 고른 런타임 경로** — NCCL 2.30.7+cuda13.3 · net plugin SPCX 를 로드했으나 장치 미지원으로 건너뛰고 `NET/IB`(RoCE 장치 2개 병합 · `NCCL_IB_MERGE_NICS=1`)를 썼고 GPU Direct RDMA 는 꺼져 있다(lite_engine_head L113 · L131 · L157~L167). MoE 는 `MARLIN` NvFp4 백엔드(lite_engine L545 · L694 weight-only FP4 경고). eager 때문에 torch.compile · CUDAGraphs 가 꺼졌다(L73 · L76). prefix caching 은 켜졌고(엔진 config `enable_prefix_caching=True` · L81) KDA 상태 때문에 `Mamba cache mode … 'align'` 과 attention 블록 크기 2128 토큰이 정해졌다(L72 · lite_engine L698). FlashInfer all-reduce 와 custom all-reduce 는 2노드 · MNNVL 부재로 꺼졌다(lite_engine L417 · L418). 동기 스케줄링은 러너 플래그로 걸렸다(`'async_scheduling': False` · L64).

## 1.5 서브 레시피 해설
<!-- FACT:sub_recipe -->
원본: `artifacts/compose/sub_recipe.json`(기계 파생 — 스모크와 같은 함수 `slave_forward.derive` 의 출력).

| 역할 | 하는 일 |
|---|---|
| `master` | ray head + vllm serve(트리플렛 source · 모델 러너 실행) |
| `master_runtime_patch` | armed — RUNTIME_PATCH_STEM='solar31-nv4-1m-tp2' → /app/configs/solar31-nv4-1m-tp2_patch.py |
| `runner_gpu_join_literal` | {"lines": [79], "note": "master 는 ray status 의 GPU 합류가 이 리터럴과 같아질 때까지 기다린다 — 노드당 GPU·노드 수가 다른 클러스터에서는 이 줄을 고치지 않으면 대기에서 멈춘다(arch 의 <G>g<N>n 과 숨은 결합).", "value": "2"} |
| `runtime_patch_key_chain` | RUNTIME_PATCH_STEM · CONFIG_FILE · 'default' |
| `slave` | ray worker(트리플렛 미수령 — 같은 이미지·같은 compose·같은 serve_runner.sh 가 NODE_ROLE 로 분기) |
| `slave_config_file` | default |
| `slave_runtime_patch` | armed — slave RUNTIME_PATCH_STEM='solar31-nv4-1m-tp2'(slave_forward 전달) → /app/configs/solar31-nv4-1m-tp2_patch.py |
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
| `source` | output/multi/benchlog/attestation_solar31-nv4-1m-tp2.json |

> **attestation 범위** — config `solar31-nv4-1m-tp2` · phase serve · 작성 2026-10-25T02:57:42Z(output/multi/benchlog/attestation_solar31-nv4-1m-tp2.json 파일 mtime(파일시스템 관측 — 문서에 시각 칸이 없다 · 기록 필드 아님)) · 묶은 근거 cell-name · 파일 `output/multi/benchlog/attestation_solar31-nv4-1m-tp2.json` · 측정 창 대비 작성 2026-10-25T02:57:42Z < 측정 시작 2026-10-25T02:58:03Z — 측정 전(빌드·스모크)의 관측: **same config(solar31-nv4-1m-tp2) · serve — 원장 순서로 판별한 같은 기동의 관측(스모크 로그 미보존 · 약한 판별)(판별: 원장 순서로 판별(스모크 로그 미보존 — docs/simlog/**/*serve*.log 에 이 attestation 을 적은 스모크 로그 없음): 원장 docs/logs/main/events/2026-10.jsonl:654 budget_declare(smoke-solar31-nv4-1m-tp2 · 2026-10-25T02:43:38Z) ≤ attestation 작성 2026-10-25T02:57:42Z · serve_proof 작성 2026-10-25T02:57:42Z < 측정 시작 2026-10-25T02:58:03Z · 그 선언이 측정 끝 2026-10-25T03:13:30Z 까지 재선언 · 창 닫힘 없이 이어짐(main 원장 기록 범위 안) · API 서버 기동 2026-10-25T02:57:07Z(output/multi/benchlog/sweep_solar31-nv4-1m-tp2/lite_engine_solar31-nv4-1m-tp2.log:736 · 컨테이너 시계 = UTC 가정) ∈ [선언, attestation 작성] · 측정 엔진 로그 APIServer pid 791 하나)**
<!-- /FACT:sub_recipe -->

<!-- FACT:measurement_env_nodes -->
**노드별 측정 env**(01 §1.4 표를 키 × 노드로 접은 것 · `만 관측` = 다른 쪽 줄이 캡처 로그에 없다 · Ray 가 같은 줄을 접은 키(`[repeated Nx across cluster]`)와 머리·꼬리 캡처 로그(01 §1.4 로그 창)에서 한쪽만 남은 키는 다른 쪽 부재를 단언하지 않는다)

- 양 노드 같은 값 12개: `NCCL_CROSS_NIC` · `NCCL_DMABUF_ENABLE` · `NCCL_IB_DISABLE` · `NCCL_IB_GID_INDEX` · `NCCL_IB_HCA` · `NCCL_IB_MERGE_NICS` · `NCCL_IB_QPS_PER_CONNECTION` · `NCCL_IB_SPLIT_DATA_ON_QPS` · `NCCL_NET_PLUGIN` · `NCCL_SOCKET_IFNAME` · `nccl:network` · `nccl:version`

| 키 | main | sub | 노드 미특정 | 판정 |
|---|---|---|---|---|
| `nccl:net_ib_devices` | `[0]<nic:cluster>:1/RoCE [1]<nic:cluster>:1/RoCE [RO]; OOB <nic:cluster>:<node:main><0>` · `[RO]; OOB <nic:cluster>:<node:main><0>` | `[RO]; OOB <nic:cluster>:<node:sub><0>` | — | 다름 |
<!-- /FACT:measurement_env_nodes -->

> 이 절이 답하는 질문: 서브(Ray worker) 노드는 메인과 무엇이 다르고, 무엇을 반드시 같게 맞춰야 하는가?

**(1) 역할 차이** — master 는 Ray head 를 띄우고 slave 의 GPU 합류(`ray status` 가 GPU 2 — serve_runner.sh L79 리터럴)를 기다린 뒤 트리플렛(`/app/configs/${CONFIG_FILE}.sh`)으로 `vllm serve` 를 실행한다. slave 는 트리플렛을 받지 않고(`CONFIG_FILE=default`) master 를 기다렸다가 `ray start --address … --block` 으로 워커가 된다 — 같은 이미지 이름 · 같은 compose · 같은 serve_runner.sh 가 `NODE_ROLE` 로 갈린다. slave 는 트리플렛 대신 `RUNTIME_PATCH_STEM` 으로 모델 런타임 패치를 **무장한다**(1.5 사실 블록 `slave_runtime_patch = armed` · 서브 전달 env 표 `RUNTIME_PATCH_STEM` 행 · arm_patch.sh L15-18 · 교정 뒤 slave 로그 `slave_failure.log` L34 의 armed 줄). 이 셀에서 slave 무장은 필수다(Ray 워커가 모델 클래스를 unpickle 한다 · W3).

**(2) 서브에 전달돼야 하는 env(무리별)** — 이미지 정체성 build-arg(`IMAGE_TAG` · `BUILD_DOCKERFILE` · `VLLM_REF`): 빠지면 slave 가 compose 기본 태그 · Dockerfile · ref 로 지어 ABI 가 갈린다. 빌드 튜닝(`BUILD_JOBS`): 정체성은 아니지만 slave 만 기본 16 으로 컴파일하면 OOM 선례가 있다. 마운트(`QUANT_MODEL_PATH` 등): 서브도 가중치를 직접 읽으므로 같은 마운트원이 필요하다(이 셀은 PLE 가 없어 `PLE_MMAP_HOST_PATH` 는 쓰이지 않는다). 클러스터(`SLAVE_CONTAINER_NAME`): 협역 워치독 필터가 이 이름으로 slave 를 찾는다. 서빙 env(`RUNTIME_PATCH_STEM`): 이 셀이 새로 만든 무리다 — 비면 종전 동작.

**(3) 선행조건** — 가중치가 서브의 **같은 마운트 경로**(`<manifest.quant_model_path>/Upstage/Solar-Open2-250B-Nota-NVFP4`)에 실재 · 런타임 패치 파일이 서브의 `configs/` 에도 실재(배달 스냅샷 편입 · W3) · 드라이버 계열 동일(측정 전 attestation: 양 노드 580.178.04) · RoCE 인터커넥트 준비(NCCL `NET/IB` 로 양 노드가 연결돼야 한다).

**(4) ABI 동일성** — attestation(작성 02:57:42Z · 측정 시작 전 · 같은 기동으로 원장 순서 판별 · 약한 판별)에서 vllm_sha 양 노드 `db9527a46873454610df6dbedf79a36d6bf1a7f6` · torch 양 노드 `2.13.0a0+9186a08b2c.nv26.7.59513937` · driver 양 노드 `580.178.04` 가 같음이고, build_ledger 축은 서브 identity 가 비어 **unobservable** 이다(02 Q7). 같아야 하는 것은 이미지 digest 가 아니라 이 ABI 다 — 노드마다 로컬 빌드다.

**(5) 측정 당시 인터커넥트 실효값** — '노드별 측정 env' 표: 양 노드 같은 값 12키(`NCCL_IB_DISABLE=0` · `NCCL_NET_PLUGIN=spcx` · `NCCL_IB_HCA=<nic:cluster>,<nic:cluster>`(앞에 `=`) · `NCCL_IB_MERGE_NICS=1` · `NCCL_IB_QPS_PER_CONNECTION=4` · `NCCL_DMABUF_ENABLE=0` · `NCCL_CROSS_NIC=1` · `NCCL_IB_SPLIT_DATA_ON_QPS=0` · `NCCL_IB_GID_INDEX=3` · `NCCL_SOCKET_IFNAME=<nic:cluster>` · `nccl:network=IB` · `nccl:version=2.30.7+cuda13.3`). 다른 것은 `nccl:net_ib_devices` 하나로, 메인 줄은 RoCE 장치 2개 목록 + OOB 를, 서브 줄은 OOB 만 담는다 — 서브의 장치 목록 줄은 꼬리 캡처 창 밖이었을 수 있어(01 §1.4 로그 창 주) 서브에 RoCE 장치가 없었다는 뜻으로 읽지 않는다. 서브 행 일부는 Ray 접힘(`[repeated Nx across cluster]`)이라 접힌 사본의 노드는 로그에 없다. 이 값들은 측정 당시 엔진 echo 이고, 지금 렌더러가 내는 `.env.interconnect` 와 다르면 이 표가 측정 당시다(1.4 (a)).

## 1.6 재현 절차 — 기동 전 준비(사람이 할 일)
> 1.4 의 명령을 치기 **전에** 사람이 손으로 해야 하는 것이다 — 가중치 획득 · 스테이징 생성 · 네트워크가 필요한 단계. 에이전트가 대신 할 수 없는 자리(다운로드 승인 · 물리 경로 · 외부 계정)가 여기 모인다.

> 이 절이 답하는 질문: 이 셀을 기동하기 전에 사람이 준비해야 하는 것은 무엇이며, 각각을 어떤 명령으로 만들고, 그 절차의 출처는 어디인가?

### 가중치 획득 — `nota-ai/Solar-Open2-250B-Nota-NVFP4`

- 이 셀의 획득 모드는 **관리 NAS 직독**이다 — 다운로드 없이 NAS 의 양자화본을 양 노드가 같은 마운트 경로로 읽었다(plan_26102412 §안전 "모델 다운로드 없음(NAS 직독)" · plan_26102401 §안전 경계 `model_source: managed`). 컨테이너 안 경로 `/app/quant_models/Upstage/Solar-Open2-250B-Nota-NVFP4` ← 호스트 `<manifest.quant_model_path>/Upstage/Solar-Open2-250B-Nota-NVFP4`(compose 의 `QUANT_MODEL_PATH` 마운트 · `artifacts/compose/.env.template`).
- 체크포인트 좌표: HF repo `nota-ai/Solar-Open2-250B-Nota-NVFP4` · revision `de88f6226788077e2d340204fd79d37720c9eda0`(00 §0.2 — 체크포인트 git HEAD) · 크기 safetensors total_size 142.76 GiB(`du` 로 재면 부풀려진다 · M3).
- 네가 받아야 한다면(사람 승인 뒤 · 무인 다운로드 ✗) 예: `huggingface-cli download nota-ai/Solar-Open2-250B-Nota-NVFP4 --revision de88f6226788077e2d340204fd79d37720c9eda0 --local-dir <manifest.quant_model_path>/Upstage/Solar-Open2-250B-Nota-NVFP4` — **추론 — 실행 검증 ✗**(이 계보는 다운로드하지 않았다). 받은 위치가 두 노드에서 같은 경로로 보여야 한다(공유 NAS 또는 노드마다 같은 경로의 사본).

### 런타임 패치 배치 — `solar31-nv4-1m-tp2_patch.py`

- `artifacts/runtime_patch/solar31-nv4-1m-tp2_patch.py` 를 빌드 컨텍스트의 `configs/solar31-nv4-1m-tp2_patch.py` 에 둔다 — **양 노드 모두**(01 §1.4 매핑 표 · W3). 이 저장소의 하네스는 메인 → 서브 배달 스냅샷이 등재 stem 의 패치쌍을 편입해 옮긴다(testlog_26102418 §2 표 2번 행 교정 (a)) — 다른 하네스라면 같은 효과(서브의 같은 경로에 같은 바이트)를 손으로 만든다.
- 셀 env 의 `RUNTIME_PATCH_STEM=solar31-nv4-1m-tp2` 가 서브로 전달되게 한다(1.5 (2)).
- 무장 확인: 양 노드 컨테이너 로그에 `[arm_patch] runtime patch armed → /app/configs/solar31-nv4-1m-tp2_patch.py`.
- 그 밖의 스테이징(PLE mmap 등)은 **해당 없음** — 이 체크포인트 config 에 PLE 키가 없다(00 §0.9 후보 표 출처 칸).

### 네트워크가 필요한 단계

- 가중치 다운로드(위 · 사람 승인 게이트) — 오프라인 대체: 미리 받아 NAS 에 둔다.
- 이미지 빌드 — NGC 베이스 `nvcr.io/nvidia/pytorch:26.07-py3` pull · vLLM 소스 clone(`VLLM_REPO` · `VLLM_REF=v0.31.0`) · pip 의존(`requirements.txt` · 빌드 의존)(`artifacts/build_recipe/Dockerfile.source-build`). 오프라인 대체: 베이스 이미지 사전 pull · 노드마다 미리 빌드(이미지 전송 대신 각 노드 로컬 빌드).
- 측정 도구 — GuideLLM 이미지 `ghcr.io/vllm-project/guidellm:v0.7.3`(측정 산출물 `level_01/bench_tool_solar31-nv4-1m-tp2.json` 의 image_ref · digest 기록). 측정 도구는 `HF_HUB_OFFLINE=1` 로 돌고 토크나이저를 호스트에 스테이징한 디렉터리(컨테이너 `/tok` 마운트)에서 읽는다(`run_bench.sh@29cbdbc89a3d` L444 · L449) — 이미지만 미리 받아 두면 된다.
- 서빙 기동 자체는 외부 접속이 필요 없다(NAS 직독 · 패치는 stdlib 로 번들을 푼다).
