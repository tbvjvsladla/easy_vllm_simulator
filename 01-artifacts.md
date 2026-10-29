# 01 · 산출물 — 무엇이 실제로 쓰였고, 어떻게 다시 띄우는가

## 1.1 적용 판정 표
> 기계가 적용 증거(빌드 원장 · 원장이 없으면 라벨된 재구성) × 파일 × 선언을 대사한 결과다. `artifacts/` 에는 재현 입력과 확인되지 않은 참고 형상도 포함될 수 있다. 파일별 검증 상태와 근거를 아래 표에서 확인한다.

<!-- FACT:slots -->
| 슬롯 | 실림 | 파일 | 신호 | 적용 증거 | 사유 |
|---|---|---|---|---|---|
| `build_patch_post` | 안 실림 | — | 1-signal | none:no-build-ledger | 적용 판정 없음 — 원장 부재 — 라벨된 재구성: applied 0 · skip 판정 = docker-history+gate(기계 파생) |
| `build_patch_pre` | 안 실림 | — | 1-signal | none:no-build-ledger | 적용 판정 없음 — 원장 부재 — 라벨된 재구성: applied 0 · skip 판정 = docker-history+gate(기계 파생) |
| `build_recipe` | 실림 | `artifacts/build_recipe/Dockerfile.source-build` · `artifacts/build_recipe/requirements.txt` | 2-signal | file:image-history:docker history build-arg 관측(VLLM_REF=v0.31.0 · VLLM_VERSION 없음 → 소스빌드 Dockerfile) — 셀 env 선택자 없음 · output/single/docker-compose.yaml 리터럴 build.dockerfile: Dockerfile.source-build(이 파일만 단독 적용하고 override 가 없을 때 Dockerfile.source-build 선택 · 과거 빌드에 이 compose 또는 override 를 사용했는지는 미관측) | Dockerfile.source-build와 requirements.txt는 이미지 history 및 이미지 안 COPY 대상 대조를 근거로 실렸다(01 §1.1의 선택 리비전/범례). 원장 부재로 모든 빌드 동작을 확정한 것은 아니다. 전자는 NGC torch에 _C를 링크하는 빌드 구조, 후자는 의존 제약을 담는다. 이번 재빌드/패치 적용 시험은 없었고, 다른 ABI 조합에서는 다시 검증한다. |
| `compose` | 실림 | `artifacts/compose/.env.template` · `artifacts/compose/docker-compose.yaml` | 2-signal | file:output/single/docker-compose.yaml | compose와 프로젝트 env 형상은 마운트·port mapping·선택된 triplet을 연결한다. host8002와 container8000의 구분은 이전 포트 교정 기록에 근거한다(testlog_26101412_lfm25-26b-v031_싱글서브_서빙_lite §1. 트리플렛). env 형상은 토큰을 포함한 운영자 실물 env가 아니므로 대상 manifest에서 준비한다. 다른 토폴로지/마운트이면 재도출한다. |
| `fork_pin` | 안 실림 | — | 1-signal | none:.claude/policies/arch_variant_ledger.json | 셀 env 에 VARIANT 줄 없음 = stock(workflow.md §변종 좌표의 거처)(기계 파생) |
| `runtime_patch` | 안 실림 | — | 1-signal | none:output/single/configs/lfm25-26b-v031_patch.py | output/single/configs/lfm25-26b-v031_patch.py 없음 — arm_patch.sh 는 no-op(런타임 패치 불해당)(기계 파생) |
| `triplet` | 실림 | `artifacts/triplet/.env.lfm25-26b-v031` · `artifacts/triplet/lfm25-26b-v031.sh` · `artifacts/triplet/lfm25-26b-v031.yaml` | 3-signal | file:output/single/configs/lfm25-26b-v031.yaml | yaml의 절대 KV/GMU/동시성, sh의 qwen3/lfm2 파서, 셀 env의 이미지/서빙 이름이 실제 서빙 입력이다. 기능 발화는 completion/reasoning/tool smoke로 확인했다. 파서 제거 대조나 다른 버전 검증을 대신하지 않는다. |

**레시피 경고 · 실린 리비전**(기계 관측 — 수신자 빌드가 여기서 갈라질 수 있다)

- ⚠ `build_recipe` `output/single/Dockerfile.source-build` — 이미지 빌드 뒤 바뀐 레시피일 수 있다 — 마지막 커밋이 이미지 Created 뒤(이미지 Created 2026-10-14T01:50:16Z · 마지막 커밋 2026-10-14T01:51:45Z)
- `build_recipe` `output/single/Dockerfile.source-build` 실린 리비전: 리비전 미특정 · 선택 방법 `worktree(측정 이미지 docker history 의 RUN·COPY 와 전수 일치 · Dockerfile 전체 바이트는 미검증)` · docker history 대조 16/16 · 실린 바이트 `worktree` · 쓰인 바이트임을 관측으로 확인 아니오
- `build_recipe` `output/single/requirements.txt` 실린 리비전: 리비전 미특정 · 선택 방법 `worktree(image-probe: 이미지 안 COPY 대상 바이트와 sha256 일치)` · docker history 대조 없음 · 실린 바이트 `worktree` · 쓰인 바이트임을 관측으로 확인 예
- `compose` `output/single/docker-compose.yaml` 실린 리비전: 커밋 `80284db86dd7` · 선택 방법 `worktree(mtime 2026-10-25T22:53:44Z ≤ 측정 2026-10-29T09:43:49Z(sweep_index.generated_utc) · 바이트 = 측정 전 마지막 커밋 80284db86dd7)` · docker history 대조 없음 · 실린 바이트 `worktree` · 근거 `mtime≤measured` · 쓰인 바이트임을 관측으로 확인 아니오

**싣지 않은 파일**(슬롯별 사유 — 빌드 때 skip 된 패치는 아래 적용 집합 표에 있다)

- `build_patch_post` known-non-patch 1건: `.gitkeep`
- `build_patch_pre` known-non-patch 1건: `.gitkeep`
- `compose` `arm_patch.sh` — `not-armed(output/single/configs/lfm25-26b-v031_patch.py 없음(CONFIG_FILE) — arm_patch.sh 는 no-op · arm_patch.sh 측정 당시 개정(fad2d7c6f157) `_cfg=` 키 체인 RUNTIME_PATCH_STEM > CONFIG_FILE > 'default')`

**파일별 검증 · 관련성**(`generated-unverified` = 실행 검증되지 않은 생성물 — 파일 첫 줄 경고 · PAYLOAD 파일 기록과 같은 표시 · 관련성 = ① 패치 선언 대상 × 모델 config ② 엔진 로그 발화 서명 — 둘 다 관측일 때만 required/inactive-inferred)

| 슬롯 | 파일 | 검증 | 검증 근거 | 관련성 | 관련성 근거 |
|---|---|---|---|---|---|
| `build_recipe` | `artifacts/build_recipe/Dockerfile.source-build` | `generated-unverified` · renderer | 실린 Dockerfile.source-build(worktree) · docker history RUN·COPY 16/16 일치 — Dockerfile 전체 원본 바이트의 동일성은 확인하지 못했다(_select_dockerfile · worktree(측정 이미지 docker history 의 RUN·COPY 와 전수 일치 · Dockerfile 전체 바이트는 미검증)) | `unknown` | 파일의 실행 당시 바이트가 확인되지 않았다(generated-unverified · 생성 renderer) — 이 표시만으로 경로 실행 여부나 필요 여부를 판정하지 않는다(참고 형상) |
| `build_recipe` | `artifacts/build_recipe/requirements.txt` | `verified` · cell-run | 쓰인 레시피(image-history:docker history build-arg 관측(VLLM_REF=v0.31.0 · VLLM_VERSION 없음 → 소스빌드 Dockerfile) — 셀 env 선택자 없음 · output/single/docker-compose.yaml 리터럴 build.dockerfile: Dockerfile.source-build(이 파일만 단독 적용하고 override 가 없을 때 Dockerfile.source-build 선택 · 과거 빌드에 이 compose 또는 override 를 사용했는지는 미관측)) — 개정 선택 slots.build_recipe.evidence.selected_revisions | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/.env.template` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/docker-compose.yaml` | `generated-unverified` · renderer | docker-compose.yaml: worktree(mtime 2026-10-25T22:53:44Z ≤ 측정 2026-10-29T09:43:49Z(sweep_index.generated_utc) · 바이트 = 측정 전 마지막 커밋 80284db86dd7) — 측정 당시 실행 바이트는 별도 관측되지 않았다 | `unknown` | 파일의 실행 당시 바이트가 확인되지 않았다(generated-unverified · 생성 renderer) — 이 표시만으로 경로 실행 여부나 필요 여부를 판정하지 않는다(참고 형상) |
| `triplet` | `artifacts/triplet/.env.lfm25-26b-v031` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/lfm25-26b-v031.sh` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/lfm25-26b-v031.yaml` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
<!-- /FACT:slots -->

<!-- FACT:applied_set -->
**적용 집합** — 상태 `unobservable` · 출처 `no-build-ledger` · 원장 없음: 아래 결과는 **라벨된 재구성**이다(관측 아님)

_빌드 패치 없음 — 이 이미지에 빌드 패치 슬롯 입력이 없다._

**원장 탐침**(순위: attestation → 메인 로컬 이미지 → 재구성)

| 순위 | 결과 | 대상 |
|---|---|---|
| attestation | absent | output/single/benchlog/attestation_lfm25-26b-v031.json(부재) |
| local-image | no-ledger(이미지에 /opt/easy-vllm/build_ledger.json 없음 — docker cp) | sha256:667eee7bd3b4e84b204ad4887bd811c87dd3537428d026eaeb580ab825d88c8e |
| reconstruction | done | — |
| image-probe | done | sha256:667eee7bd3b4e84b204ad4887bd811c87dd3537428d026eaeb580ab825d88c8e |

**재구성 입력** — `docker-history+gate-rule · fail-loud-rule · image-probe`(docker history build-arg × 패치 자기게이트 규칙)

| build-arg | 값 |
|---|---|
| `BUILD_JOBS` | 8 |
| `RAY_VERSION` | 2.48.0 |
| `SM12X_PORT` | 0 |
| `SRC_DEPS_AUTHORITY` | 0 |
| `TORCH_CUDA_ARCH` | 12.1a |
| `VLLM_PRETEND_VERSION` | (빈 값) |
| `VLLM_REF` | v0.31.0 |
| `VLLM_REPO` | https://github.com/vllm-project/vllm.git |
<!-- /FACT:applied_set -->

## 1.2 슬롯별 적용 사유
> 이 절이 답하는 질문: 실린 산출물 하나하나는 무엇을 고치며, 왜 이 셀에 필요했고, 이 셀에서 실제로 발화했으며, 언제 불필요해지는가?

### build_recipe
Dockerfile.source-build와 requirements.txt는 이미지 history 및 이미지 안 COPY 대상 대조를 근거로 실렸다(01 §1.1의 선택 리비전/범례). 원장 부재로 모든 빌드 동작을 확정한 것은 아니다. 전자는 NGC torch에 _C를 링크하는 빌드 구조, 후자는 의존 제약을 담는다. 이번 재빌드/패치 적용 시험은 없었고, 다른 ABI 조합에서는 다시 검증한다.

### compose
compose와 프로젝트 env 형상은 마운트·port mapping·선택된 triplet을 연결한다. host8002와 container8000의 구분은 이전 포트 교정 기록에 근거한다(testlog_26101412_lfm25-26b-v031_싱글서브_서빙_lite §1. 트리플렛). env 형상은 토큰을 포함한 운영자 실물 env가 아니므로 대상 manifest에서 준비한다. 다른 토폴로지/마운트이면 재도출한다.

### triplet
yaml의 절대 KV/GMU/동시성, sh의 qwen3/lfm2 파서, 셀 env의 이미지/서빙 이름이 실제 서빙 입력이다. 기능 발화는 completion/reasoning/tool smoke로 확인했다. 파서 제거 대조나 다른 버전 검증을 대신하지 않는다.

### 실리지 않은 슬롯
build_patch_pre/post는 원장이 없어 적용 없음으로 재구성됐고 실린 파일은 없다. fork_pin·runtime_patch도 실리지 않았다. 따라서 별도 패치의 발화·트립와이어·폐기 조건을 이 셀에서 관측했다고 주장하지 않는다. 공유 이미지의 조건부 코드를 LFM 고유 요구로 옮기지 않는다.

## 1.3 값의 지위표
> 서빙 설정의 노브 **전부**와 기계가 모은 후보 지위다(lockset 출처 · yaml 주석 · 엔진 기본값). 최종 지위는 아래 `kind: value-status` 블록이 정한다 — 표의 노브마다 블록이 정확히 하나 있어야 린터를 통과한다.

<!-- FACT:value_status -->
서빙 노브 4개 — 노브마다 `kind: value-status` 블록이 **정확히 1개** 있어야 한다(노브·값은 이 표의 글자 그대로 · 값 칸의 `\|` 는 `|` 로 적는다).

| 노브 | 값 | 후보 지위 | 후보 근거 | yaml 주석 | 출처 |
|---|---|---|---|---|---|
| `gpu-memory-utilization` | 0.39 | inherited | lockset 출처 gmu_source=hand · lockset provenance=hand-authored | — | lockset.gmu_source=hand |
| `max-model-len` | 32768 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `max-num-seqs` | 8 | inherited | lockset 출처 batch_source=hand-lever · lockset provenance=hand-authored | — | lockset.batch_source=hand-lever |
| `kv-cache-memory-bytes` | 40694815129 | inherited | lockset 출처 kv_source=hand · lockset provenance=hand-authored | — | lockset.kv_source=hand |
<!-- /FACT:value_status -->

> 이 절이 답하는 질문: 각 서빙 노브의 값은 이 셀에서 어떤 지위인가 — 조정됐나, 승계됐나, 음성대조로 일부러 둔 것인가, 엔진 기본값인가, 필요조건인가?

```hint-event
id: V1
kind: value-status
노브: "gpu-memory-utilization"
값: "0.39"
지위: "inherited"
근거: "이번에는 hand-authored lockset의 hand 값 승계. 과거 free-memory 실패 뒤 선택된 값이며 이번 GMU 탐색은 하지 않았다."
출처: ["testlog_26101412_lfm25-26b-v031_싱글서브_서빙_lite §1. 트리플렛", "devlog_26102716_lfm25_default_full_신규실측 §Grounded recipe retained"]
```

```hint-event
id: V2
kind: value-status
노브: "max-model-len"
값: "32768"
지위: "inherited"
근거: "이전 로컬 recipe/이번 선언에서 승계한 목표 길이. 체크포인트 최대 길이나 이번 최대치 탐색 결과가 아니다."
출처: ["testlog_26101412_lfm25-26b-v031_싱글서브_서빙_lite §1. 트리플렛", "devlog_26102716_lfm25_default_full_신규실측 §Grounded recipe retained"]
```

```hint-event
id: V3
kind: value-status
노브: "max-num-seqs"
값: "8"
지위: "inherited"
근거: "손레버 batch8 승계. 이번에는 capacity filter 및 c1/2/4/8 부하만 검증했다."
출처: ["testlog_26101412_lfm25-26b-v031_싱글서브_서빙_lite §1. 트리플렛", "devlog_26102716_lfm25_default_full_신규실측 §Grounded recipe retained"]
```

```hint-event
id: V4
kind: value-status
노브: "kv-cache-memory-bytes"
값: "40694815129"
지위: "inherited"
근거: "손레버 절대 clamp 승계. 과거10GiB/37.9GiB 시험은 있으나 이번에 최소 KV를 수렴한 것은 아니다."
출처: ["testlog_26101412_lfm25-26b-v031_싱글서브_서빙_lite §1. 트리플렛", "devlog_26102716_lfm25_default_full_신규실측 §Grounded recipe retained"]
```

이번 스윕은 동시성 부하 스윕이며 위 네 값의 최적화 실험이 아니다. dtype·메모리 총량·동시 요청 요구가 다르면 V1~V4를 다시 산정한다. graph는 명시 enforce-eager 부재의 엔진 기본 경로이고, 파서의 필요조건 여부는 이번 제거 대조로 확인하지 않았다.

## 1.4 재현 절차
> 셀 실행 기록에서 기계가 파생한 명령 순서와 단계별 소요다. 소요가 `미관측` 인 단계는 시각 기록이 없다는 뜻이다(0 이 아니다). 그 아래는 블랙박스 이벤트 원장에서 이 셀의 행을 기계가 뽑아 기동 시도로 묶은 것이고, 마지막은 측정한 실행의 엔진 로그가 echo 한 env 다(측정 당시 값의 정본).

<!-- FACT:reproduce -->
| 순서 | 단계 | 성공 판정 | 소요 | 관측 구간(UTC) | 소요 출처 | 명령 출처 |
|---|---|---|---|---|---|---|
| 1 | build | 빌드 OK · 이미지 실재 | 층 창 37분 3초(docker history 첫 층 → 끝 층 CreatedAt · 캐시 재사용 층 포함 — 빌드 소요 아님) | 2026-10-14T01:13:13Z → 2026-10-14T01:50:16Z (layer-window) | docker history --no-trunc --format '{{json .}}' sha256:667eee7bd3b4e84b204ad4887bd811c87dd3537428d026eaeb580ab825d88c8e · 우리 Dockerfile 층 29개의 CreatedAt 첫 층 → 끝 층(캐시 재사용 층은 이전 빌드 시각 — 빌드 소요 아님) | reconstructed(docker-history build-args + compose build 선언/설정(실행 관측 아님)) |
| 2 | serve | health 200 + 추론 1회(스모크 판정) | ≤ 3분 5초(상한)(끝 = 최종 스윕 첫 측정 · 그 전 요청 3건 — 소요 출처 칸) | 2026-10-29T09:26:07Z → 2026-10-29T09:29:12Z (upper) | docs/logs/sub/sessions/serve-handoff2-lfm25.json started_utc(측정을 포함하는 세션) → 첫 측정 시각(output/single/benchlog/sweep_lfm25_handoff2_20261029_run3/lite_raw_lfm25-26b-v031.json measured_utc(호스트 date -u · lite_bench.sh 가 cold 레그 직전에 찍는 측정 시작)) · ⚠ 최종 스윕 기준 — 관측된 serve 세션 경계 안 첫 측정 전 completions 요청 3건 관측(상한 시각 2026-10-29T09:28:37Z~2026-10-29T09:28:57Z · 스모크 최소 추론 1회 초과(요청 목적·앞선 벤치 여부는 미관측) · 세션 docs/logs/sub/sessions/serve-handoff2-lfm25.json · 출처 output/single/benchlog/sweep_lfm25_handoff2_20261029_run3/lite_engine_head_lfm25-26b-v031.log · output/single/benchlog/sweep_lfm25_handoff2_20261029_run3/lite_engine_lfm25-26b-v031.log 의 POST 접근 줄 × 전후 vLLM 시각 줄(컨테이너 시계 UTC 가정 · 구간을 확인한 요청만, 세션 전체 요청 수 아님)) | derived(스모크 사용법) |
| 3 | bench | 리포트 발행(루브릭 권한 explicit ∧ PASS일 때만 full 인증서) | 14분 37초(최종 스윕 기준 · 그 전 요청 3건 — 소요 출처 칸) | 2026-10-29T09:29:12Z → 2026-10-29T09:43:49Z (exact) | output/single/benchlog/sweep_lfm25_handoff2_20261029_run3/lite_raw_lfm25-26b-v031.json measured_utc(호스트 date -u · lite_bench.sh 가 cold 레그 직전에 찍는 측정 시작) → sweep_index.generated_utc(호스트 UTC · 두 끝 같은 시계) · ⚠ 최종 스윕 기준 — 관측된 serve 세션 경계 안 첫 측정 전 completions 요청 3건 관측(상한 시각 2026-10-29T09:28:37Z~2026-10-29T09:28:57Z · 스모크 최소 추론 1회 초과(요청 목적·앞선 벤치 여부는 미관측) · 세션 docs/logs/sub/sessions/serve-handoff2-lfm25.json · 출처 output/single/benchlog/sweep_lfm25_handoff2_20261029_run3/lite_engine_head_lfm25-26b-v031.log · output/single/benchlog/sweep_lfm25_handoff2_20261029_run3/lite_engine_lfm25-26b-v031.log 의 POST 접근 줄 × 전후 vLLM 시각 줄(컨테이너 시계 UTC 가정 · 구간을 확인한 요청만, 세션 전체 요청 수 아님)) | reconstructed(bench json + 측정 도구 원문 lite 인자) |

**빌드 층 시각**(관측 · 소요가 아니라 층 창 — 캐시 재사용 층은 이전 빌드의 시각을 가진다)

- `build` 이미지 층 CreatedAt(docker history): 2026-10-14T01:13:13Z → 2026-10-14T01:50:16Z · 층 29개 · 출처 `docker history --no-trunc --format '{{json .}}' sha256:667eee7bd3b4e84b204ad4887bd811c87dd3537428d026eaeb580ab825d88c8e` — 우리 Dockerfile 층의 CreatedAt 범위 — 캐시 재사용 층은 이전 빌드 시각을 가진다(소요 아님)

**단계별 명령**(기계 파생 — `명령 출처`가 `derived(…)`이면 사용법에서 파생하고 `reconstructed(…)`이면 괄호 안에 명시된 관측, 파일 선언·설정 또는 추론을 조합해 재구성한다. 예를 들어 docker history와 bench JSON은 기록된 관측이며 compose 파일은 선언·설정이다. 설정만으로 과거 compose build 실행 여부는 알 수 없다. 어느 명령도 실행 원문(로그에 남은 줄)은 아니다)

**1. build**

```sh
# 실린 artifacts/build_recipe/Dockerfile.source-build(실린 개정: worktree — 이미지를 지은 바이트임을 관측으로 확인 못함(docker history RUN·COPY 16/16))를 output/single/Dockerfile.source-build 자리에 두는 재현 예시다(실제 빌드 컨텍스트 아님).
# 독립 재현용 태그를 지정한다. 측정 이미지와 동일한 바이트·후속 빌드 층까지 재현되는지는 별도 검증한다.
mkdir -p output/single/build_patches_src output/single/build_patches   # 빌드 원장 context_inventory 미관측 — 빌드 때 내용 모름(실린 패치 없음) · 빌드 원장 context_inventory 미관측 — 빌드 때 내용 모름(실린 패치 없음)
docker build -f output/single/Dockerfile.source-build \
  --build-arg BUILD_JOBS=8 \
  --build-arg RAY_VERSION=2.48.0 \
  --build-arg SM12X_PORT=0 \
  --build-arg SRC_DEPS_AUTHORITY=0 \
  --build-arg TORCH_CUDA_ARCH=12.1a \
  --build-arg VLLM_PRETEND_VERSION= \
  --build-arg VLLM_REF=v0.31.0 \
  --build-arg VLLM_REPO=https://github.com/vllm-project/vllm.git \
  -t "${REPRO_IMAGE_TAG:?set a separate reproduction image tag}" \
  output/single
# compose 빌드 사용법(실제 실행·재현 등치는 미검증): docker compose -f output/single/docker-compose.yaml --env-file output/single/envs/.env.lfm25-26b-v031 build
```

**2. serve**

```sh
bash .claude/skills/upstream-version-watch/scripts/single_serve_up.sh lfm25-26b-v031
```

**3. bench**

```sh
# bench JSON 필드로 복원: backend · model_id→--model · tokenizer_id→--tokenizer · num_prompts · max_concurrency · request_rate · burstiness · 요청당 입출력 길이 = total_*_tokens ÷ completed(나누어떨어질 때만 — 평균이다 · 요청별 길이는 JSON 에 없다) · 나누어떨어지면 `--dataset-name random` 으로 추정했다(측정 도구가 그 꼴로 부른다 — 아래 도구 원문).
# JSON 에 없는 인자(--endpoint · --base-url · --ignore-eos · --num-warmups · --temperature · --seed · --random-range-ratio · --trust-remote-code)는 측정 도구가 정했다 — level_* = run_bench.sh · lite_cold/lite_warm = lite_bench.sh(.claude/skills/adversarial-benchmark/scripts/) · 셀 스윕 순서는 sweep_bench.sh.
# lite_cold/lite_warm 의 데이터셋 · 부하 인자 = 측정 시점 도구 원문 lite_bench.sh@a7083a8f933d L177-L183(리터럴 인자만 · `$변수` 인자는 도구가 값을 정하지 않아 JSON 필드로 채운다) — `total_*_tokens ÷ completed` 는 실측 토큰(chat 템플릿 포함)이지 인자가 아니다.
# output/single/benchlog/sweep_lfm25_handoff2_20261029_run3/lite_cold_lfm25-26b-v031.json (date 20261029-092925) · 실측 토큰(chat 템플릿 포함 · total ÷ completed · 인자 아님): 입력 522/1 = 522 · 출력 128/1 = 128
vllm bench serve --backend openai-chat --model lfm2.5-2.6b --tokenizer /app/models/Liquid-AI/LFM2.5-2.6B --trust-remote-code --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 --ignore-eos --num-prompts 1 --max-concurrency 1 --request-rate inf
# output/single/benchlog/sweep_lfm25_handoff2_20261029_run3/lite_warm_lfm25-26b-v031.json (date 20261029-092950) · 실측 토큰(chat 템플릿 포함 · total ÷ completed · 인자 아님): 입력 1566/3 = 522 · 출력 384/3 = 128
vllm bench serve --backend openai-chat --model lfm2.5-2.6b --tokenizer /app/models/Liquid-AI/LFM2.5-2.6B --trust-remote-code --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 --ignore-eos --num-prompts 3 --max-concurrency 1 --request-rate inf
```

**슬롯 → 재구성 예시 컨텍스트 매핑**(예시 위치 = `output/<토폴로지>/` · 원천 = 실린 Dockerfile 의 COPY · compose 설정; 실제 실행 경로는 별도 기록 확인)

| zip 경로 | 빌드 컨텍스트 자리 | 근거 |
|---|---|---|
| `artifacts/triplet/.env.lfm25-26b-v031` | `envs/.env.lfm25-26b-v031` | compose env_file envs/.env.${CONFIG_FILE}(셀 env) |
| `artifacts/triplet/lfm25-26b-v031.sh` | `configs/lfm25-26b-v031.sh` | compose volumes ./configs:/app/configs |
| `artifacts/triplet/lfm25-26b-v031.yaml` | `configs/lfm25-26b-v031.yaml` | compose volumes ./configs:/app/configs |
| `artifacts/build_recipe/Dockerfile.source-build` | `Dockerfile.source-build` | compose build.dockerfile Dockerfile.source-build(리터럴) · build.context . |
| `artifacts/build_recipe/requirements.txt` | `requirements.txt` | Dockerfile.source-build COPY requirements.txt |
| `artifacts/compose/.env.template` | `.env` | compose 변수치환 프로젝트 .env(${…} 참조) |
| `artifacts/compose/docker-compose.yaml` | `docker-compose.yaml` | compose 파일(빌드 컨텍스트 루트) |
| (실린 파일 없음 — 빈 디렉터리 생성) | `build_patches_src/` | Dockerfile.source-build COPY build_patches_src/ · 실린 패치 없음 — 빈 디렉터리를 만든다(없으면 COPY 단계에서 빌드가 멈춘다) · 빌드 원장 context_inventory 미관측 — 빌드 때 내용 모름(실린 패치 없음) |
| (실린 파일 없음 — 빈 디렉터리 생성) | `build_patches/` | Dockerfile.source-build COPY build_patches/ · 실린 패치 없음 — 빈 디렉터리를 만든다(없으면 COPY 단계에서 빌드가 멈춘다) · 빌드 원장 context_inventory 미관측 — 빌드 때 내용 모름(실린 패치 없음) |

**기동 성공 판정(관측 · 발행 자격)**: health 200 = true · 추론 1회 = true · 근거 `output/single/benchlog/sweep_lfm25_handoff2_20261029_run3/level_01/post_health_lfm25-26b-v031.json` · `output/single/benchlog/sweep_lfm25_handoff2_20261029_run3/level_01/guidellm_lfm25-26b-v031.json`
<!-- /FACT:reproduce -->

<!-- FACT:event_timeline -->
**블랙박스 이벤트 266행**(원장에서 이 셀 label·config 에 맞는 행과 그와 시간이 겹치는 같은 노드의 다른 라벨 창 · 시각순 · 기계 발췌 · 이번 캠페인 15행 · 같은 셀 이름 · 앞선 캠페인 251행(내용 칸에 표기) · 라벨 다름(같은 노드 · 창 밖 이 셀 행과 시간이 겹치는 다른 라벨 선언 창) 6행 — 셀 라벨 일치 집계에서 제외(세션·호출 기록의 귀속 확인과 별도))

> **캠페인 경계** 2026-10-29T09:21:44Z(`camp-26102918-handoff-single2` · evidence.event_campaign_boundary): campaigns/camp-26102918-handoff-single2/campaign.yaml declared_utc 2026-10-29T09:21:44Z · 이 발행이 묶인 측정을 서빙한 budget_declare 창 시작 2026-10-29T09:26:07Z → 경계 = 앞선 값 2026-10-29T09:21:44Z(이전 행 = 같은 셀 이름 · 앞선 캠페인)

> **원장 관측 범위**(노드별 첫 행 → 마지막 행 · evidence.event_ledger_spans): sub = 2026-06-13T22:25:01Z → 2026-10-29T09:44:22Z(`docs/logs/sub/events/2026-06.jsonl` · `docs/logs/sub/events/2026-07.jsonl` · `docs/logs/sub/events/2026-09.jsonl` · `docs/logs/sub/events/2026-10.jsonl` · `docs/logs/sub/events/thermal.jsonl` · `docs/logs/sub/events/watchdog.jsonl`)

| # | UTC | 노드 | kind | label | 내용 | 출처 |
|---|---|---|---|---|---|---|
| 1 | 2026-10-14T02:51:50Z | sub | `budget_declare` | `serve-lfm25-26b-v031` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=38862 · weights_mib=5144 · kv_mib=38809 · overhead_mib=41793 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 기동 창 1/2: budget_renew 230회 · 닫힘 replaced(budget_declare) 2026-10-23T16:49:06Z(선언 후 229h57m16s) · measured_utc 2026-10-29T09:43:49Z 는 이 창 밖(다른 기동 · 측정 전) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:279 |
| 2 | 2026-10-14T02:51:50Z | sub | `serve_start` | — | serve_start · model=lfm2.5-2.6b · tensor_parallel_size=1 · topology=single · session=serve-lfm25-26b-v031-20261014T025150Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:280 |
| 3 | 2026-10-14T02:51:50Z | sub | `budget_honored` | `serve-lfm25-26b-v031` | budget_honored · floor_mib=38862 · arm_ceiling_mib=35790 · remaining_s=7200 · 창 시작 2026-10-14T02:51:50Z 후 0m00s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:551 |
| 4 | 2026-10-14T02:51:51Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=7199 · 창 시작 2026-10-14T02:51:50Z 후 0m01s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:281 |
| 5 | 2026-10-14T02:52:50Z | sub | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=55494 · rate_mib_s=23142 · max_rate_mib_s=17801 · legacy_rule_would_trip=true · 라벨 없음 → serve-lfm25-26b-v031 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-14T02:51:50Z 후 1m00s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:552 |
| 6 | 2026-10-14T03:21:51Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 30m01s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:282 |
| 7 | 2026-10-14T03:51:51Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 1h00m01s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:283 |
| 8 | 2026-10-14T04:21:51Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 1h30m01s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:284 |
| 9 | 2026-10-14T04:51:51Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 2h00m01s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:285 |
| 10 | 2026-10-14T05:21:52Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5399 · 창 시작 2026-10-14T02:51:50Z 후 2h30m02s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:286 |
| 11 | 2026-10-14T05:51:52Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 3h00m02s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:287 |
| 12 | 2026-10-14T06:21:52Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 3h30m02s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:288 |
| 13 | 2026-10-14T06:51:52Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 4h00m02s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:289 |
| 14 | 2026-10-14T07:21:52Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 4h30m02s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:290 |
| 15 | 2026-10-14T07:51:52Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 5h00m02s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:291 |
| 16 | 2026-10-14T08:21:52Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 5h30m02s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:292 |
| 17 | 2026-10-14T08:51:52Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 6h00m02s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:293 |
| 18 | 2026-10-14T09:21:52Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 6h30m02s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:294 |
| 19 | 2026-10-14T09:51:52Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 7h00m02s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:295 |
| 20 | 2026-10-14T10:21:52Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 7h30m02s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:296 |
| 21 | 2026-10-14T10:51:52Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 8h00m02s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:297 |
| 22 | 2026-10-14T11:21:52Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 8h30m02s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:298 |
| 23 | 2026-10-14T11:51:52Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 9h00m02s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:299 |
| 24 | 2026-10-14T12:21:52Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 9h30m02s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:300 |
| 25 | 2026-10-14T12:51:52Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 10h00m02s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:301 |
| 26 | 2026-10-14T13:21:53Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5399 · 창 시작 2026-10-14T02:51:50Z 후 10h30m03s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:302 |
| 27 | 2026-10-14T13:51:53Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 11h00m03s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:303 |
| 28 | 2026-10-14T14:21:53Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 11h30m03s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:304 |
| 29 | 2026-10-14T14:51:53Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 12h00m03s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:305 |
| 30 | 2026-10-14T15:21:53Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 12h30m03s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:309 |
| 31 | 2026-10-14T15:51:53Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 13h00m03s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:310 |
| 32 | 2026-10-14T16:21:53Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 13h30m03s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:311 |
| 33 | 2026-10-14T16:51:53Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 14h00m03s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:312 |
| 34 | 2026-10-14T17:21:53Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 14h30m03s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:313 |
| 35 | 2026-10-14T17:51:53Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 15h00m03s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:314 |
| 36 | 2026-10-14T18:21:53Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 15h30m03s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:315 |
| 37 | 2026-10-14T18:51:53Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 16h00m03s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:316 |
| 38 | 2026-10-14T19:21:53Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 16h30m03s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:317 |
| 39 | 2026-10-14T19:51:53Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 17h00m03s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:318 |
| 40 | 2026-10-14T20:21:53Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 17h30m03s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:319 |
| 41 | 2026-10-14T20:51:53Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 18h00m03s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:320 |
| 42 | 2026-10-14T21:21:53Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 18h30m03s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:321 |
| 43 | 2026-10-14T21:51:54Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5399 · 창 시작 2026-10-14T02:51:50Z 후 19h00m04s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:322 |
| 44 | 2026-10-14T22:21:54Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 19h30m04s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:323 |
| 45 | 2026-10-14T22:51:54Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 20h00m04s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:324 |
| 46 | 2026-10-14T23:21:54Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 20h30m04s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:325 |
| 47 | 2026-10-14T23:51:54Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 21h00m04s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:326 |
| 48 | 2026-10-15T00:21:54Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 21h30m04s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:327 |
| 49 | 2026-10-15T00:51:54Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 22h00m04s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:328 |
| 50 | 2026-10-15T01:21:54Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 22h30m04s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:329 |
| 51 | 2026-10-15T01:51:54Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 23h00m04s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:330 |
| 52 | 2026-10-15T02:21:54Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 23h30m04s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:331 |
| 53 | 2026-10-15T02:51:54Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 24h00m04s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:332 |
| 54 | 2026-10-15T03:21:54Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 24h30m04s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:333 |
| 55 | 2026-10-15T03:51:54Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 25h00m04s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:334 |
| 56 | 2026-10-15T04:21:54Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 25h30m04s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:335 |
| 57 | 2026-10-15T04:51:54Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 26h00m04s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:336 |
| 58 | 2026-10-15T05:21:55Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5399 · 창 시작 2026-10-14T02:51:50Z 후 26h30m05s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:337 |
| 59 | 2026-10-15T05:51:55Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 27h00m05s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:338 |
| 60 | 2026-10-15T06:21:55Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 27h30m05s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:339 |
| 61 | 2026-10-15T06:51:55Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 28h00m05s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:340 |
| 62 | 2026-10-15T07:21:55Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 28h30m05s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:341 |
| 63 | 2026-10-15T07:51:55Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 29h00m05s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:342 |
| 64 | 2026-10-15T08:21:55Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 29h30m05s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:343 |
| 65 | 2026-10-15T08:51:55Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 30h00m05s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:344 |
| 66 | 2026-10-15T09:21:55Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 30h30m05s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:345 |
| 67 | 2026-10-15T09:51:55Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 31h00m05s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:346 |
| 68 | 2026-10-15T10:21:55Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 31h30m05s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:347 |
| 69 | 2026-10-15T10:51:55Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 32h00m05s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:348 |
| 70 | 2026-10-15T11:21:55Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 32h30m05s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:349 |
| 71 | 2026-10-15T11:51:55Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 33h00m05s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:350 |
| 72 | 2026-10-15T12:21:55Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 33h30m05s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:351 |
| 73 | 2026-10-15T12:51:56Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5399 · 창 시작 2026-10-14T02:51:50Z 후 34h00m06s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:352 |
| 74 | 2026-10-15T13:21:56Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 34h30m06s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:353 |
| 75 | 2026-10-15T13:51:56Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 35h00m06s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:354 |
| 76 | 2026-10-15T14:21:56Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 35h30m06s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:355 |
| 77 | 2026-10-15T14:51:56Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 36h00m06s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:356 |
| 78 | 2026-10-15T15:21:56Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 36h30m06s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:360 |
| 79 | 2026-10-15T15:51:56Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 37h00m06s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:361 |
| 80 | 2026-10-15T16:21:56Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 37h30m06s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:362 |
| 81 | 2026-10-15T16:51:56Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 38h00m06s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:363 |
| 82 | 2026-10-15T17:21:56Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 38h30m06s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:364 |
| 83 | 2026-10-15T17:51:56Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 39h00m06s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:365 |
| 84 | 2026-10-15T18:21:56Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 39h30m06s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:366 |
| 85 | 2026-10-15T18:51:56Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 40h00m06s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:367 |
| 86 | 2026-10-15T19:21:56Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 40h30m06s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:368 |
| 87 | 2026-10-15T19:51:56Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 41h00m06s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:369 |
| 88 | 2026-10-15T20:21:56Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 41h30m06s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:370 |
| 89 | 2026-10-15T20:51:56Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 42h00m06s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:371 |
| 90 | 2026-10-15T21:21:57Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5399 · 창 시작 2026-10-14T02:51:50Z 후 42h30m07s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:372 |
| 91 | 2026-10-15T21:51:57Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 43h00m07s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:373 |
| 92 | 2026-10-15T22:21:57Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 43h30m07s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:374 |
| 93 | 2026-10-15T22:51:57Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 44h00m07s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:375 |
| 94 | 2026-10-15T23:21:57Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 44h30m07s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:376 |
| 95 | 2026-10-15T23:51:57Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 45h00m07s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:377 |
| 96 | 2026-10-16T00:21:57Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 45h30m07s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:378 |
| 97 | 2026-10-16T00:51:57Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 46h00m07s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:379 |
| 98 | 2026-10-16T01:21:57Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 46h30m07s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:380 |
| 99 | 2026-10-16T01:51:57Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 47h00m07s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:381 |
| 100 | 2026-10-16T02:21:57Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 47h30m07s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:382 |
| 101 | 2026-10-16T02:51:57Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 48h00m07s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:383 |
| 102 | 2026-10-16T03:21:57Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 48h30m07s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:384 |
| 103 | 2026-10-16T03:51:57Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 49h00m07s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:385 |
| 104 | 2026-10-16T04:21:57Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 49h30m07s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:386 |
| 105 | 2026-10-16T04:51:57Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 50h00m07s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:387 |
| 106 | 2026-10-16T05:21:57Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 50h30m07s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:388 |
| 107 | 2026-10-16T05:51:58Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5399 · 창 시작 2026-10-14T02:51:50Z 후 51h00m08s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:389 |
| 108 | 2026-10-16T06:21:58Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 51h30m08s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:390 |
| 109 | 2026-10-16T06:51:58Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 52h00m08s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:391 |
| 110 | 2026-10-16T07:21:58Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 52h30m08s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:392 |
| 111 | 2026-10-16T07:51:58Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 53h00m08s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:393 |
| 112 | 2026-10-16T08:21:58Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 53h30m08s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:394 |
| 113 | 2026-10-16T08:51:58Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 54h00m08s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:395 |
| 114 | 2026-10-16T09:21:58Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 54h30m08s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:396 |
| 115 | 2026-10-16T09:51:58Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 55h00m08s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:397 |
| 116 | 2026-10-16T10:21:58Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 55h30m08s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:398 |
| 117 | 2026-10-16T10:51:58Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 56h00m08s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:399 |
| 118 | 2026-10-16T11:21:58Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 56h30m08s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:400 |
| 119 | 2026-10-16T11:40:37Z | 미관측 | `measurement` | `measurement (budget window recorded)` | 같은 셀의 다른 측정(측정 노드 축 sub) · 시각 = 측정 조립(sweep generated_utc = 인증서 measured_utc · 시작 시각 미기록) · 이 발행의 측정 2026-10-29T09:43:49Z 전 · 발행 기록 camp_26101616_cnn_hint__sub__lfm25_26b_v031(셀 대조 simlog:sweep_index.meta.config_name · verdict PASS) · simlog 사본 generated_utc 2026-10-16T11:40:37Z(같음) / 출력 평면에 보존된 같은 셀의 다른 측정 · 원장: 이 셀 선언 창 안: serve-lfm25-26b-v031 2026-10-14T02:51:50Z~2026-10-23T16:49:06Z (docs/logs/sub/events/2026-10.jsonl:279) · serve-lfm25-26b-v031 2026-10-14T02:51:50Z~2026-10-18T21:45:34Z (docs/logs/sub/events/watchdog.jsonl:551) · 직전 예산 행 budget_renew(serve-lfm25-26b-v031) 2026-10-16T11:21:58Z(docs/logs/sub/events/2026-10.jsonl:400) · 직후 예산 행 budget_renew(serve-lfm25-26b-v031) 2026-10-16T11:51:58Z(docs/logs/sub/events/2026-10.jsonl:401) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/simlog/26101620_52_00_camp_26101616_cnn_hint__sub__lfm25_26b_v031/sweep_index.json:4 |
| 120 | 2026-10-16T11:51:58Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 57h00m08s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:401 |
| 121 | 2026-10-16T12:21:58Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 57h30m08s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:402 |
| 122 | 2026-10-16T12:51:58Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 58h00m08s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:403 |
| 123 | 2026-10-16T13:21:58Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 58h30m08s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:404 |
| 124 | 2026-10-16T13:51:58Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 59h00m08s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:405 |
| 125 | 2026-10-16T14:08:27Z | 미관측 | `measurement` | `measurement (budget window recorded)` | 같은 셀의 다른 측정(측정 노드 축 sub) · 시각 = 측정 조립(sweep generated_utc = 인증서 measured_utc · 시작 시각 미기록) · 이 발행의 측정 2026-10-29T09:43:49Z 전 · 발행 기록 camp_26101623_baseline__sub__lfm25_26b_v031(셀 대조 publication-id-suffix(simlog 미관측) · verdict 미기재) · 원장: 이 셀 선언 창 안: serve-lfm25-26b-v031 2026-10-14T02:51:50Z~2026-10-23T16:49:06Z (docs/logs/sub/events/2026-10.jsonl:279) · serve-lfm25-26b-v031 2026-10-14T02:51:50Z~2026-10-18T21:45:34Z (docs/logs/sub/events/watchdog.jsonl:551) · 직전 예산 행 budget_renew(serve-lfm25-26b-v031) 2026-10-16T13:51:58Z(docs/logs/sub/events/2026-10.jsonl:405) · 직후 예산 행 budget_renew(serve-lfm25-26b-v031) 2026-10-16T14:21:59Z(docs/logs/sub/events/2026-10.jsonl:406) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/_evidence/camp_26101623_baseline__sub__lfm25_26b_v031.json:3 |
| 126 | 2026-10-16T14:21:59Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5399 · 창 시작 2026-10-14T02:51:50Z 후 59h30m09s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:406 |
| 127 | 2026-10-16T14:51:59Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 60h00m09s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:407 |
| 128 | 2026-10-16T15:21:59Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 60h30m09s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:408 |
| 129 | 2026-10-16T15:51:59Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 61h00m09s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:412 |
| 130 | 2026-10-16T16:21:59Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 61h30m09s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:413 |
| 131 | 2026-10-16T16:51:59Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 62h00m09s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:414 |
| 132 | 2026-10-16T17:21:59Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 62h30m09s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:415 |
| 133 | 2026-10-16T17:51:59Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 63h00m09s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:416 |
| 134 | 2026-10-16T18:21:59Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 63h30m09s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:417 |
| 135 | 2026-10-16T18:51:59Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 64h00m09s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:418 |
| 136 | 2026-10-16T19:21:59Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 64h30m09s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:419 |
| 137 | 2026-10-16T19:51:59Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 65h00m09s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:420 |
| 138 | 2026-10-16T20:21:59Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 65h30m09s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:421 |
| 139 | 2026-10-16T20:51:59Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 66h00m09s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:422 |
| 140 | 2026-10-16T21:21:59Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 66h30m09s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:423 |
| 141 | 2026-10-16T21:51:59Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 67h00m09s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:424 |
| 142 | 2026-10-16T22:22:00Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5399 · 창 시작 2026-10-14T02:51:50Z 후 67h30m10s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:425 |
| 143 | 2026-10-16T22:52:00Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 68h00m10s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:426 |
| 144 | 2026-10-16T23:22:00Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 68h30m10s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:427 |
| 145 | 2026-10-16T23:52:00Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 69h00m10s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:428 |
| 146 | 2026-10-17T00:22:00Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 69h30m10s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:429 |
| 147 | 2026-10-17T00:52:00Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 70h00m10s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:430 |
| 148 | 2026-10-17T01:22:00Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 70h30m10s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:431 |
| 149 | 2026-10-17T01:52:00Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 71h00m10s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:432 |
| 150 | 2026-10-17T02:22:00Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 71h30m10s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:433 |
| 151 | 2026-10-17T02:52:00Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 72h00m10s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:434 |
| 152 | 2026-10-17T03:22:00Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 72h30m10s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:435 |
| 153 | 2026-10-17T03:52:00Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 73h00m10s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:436 |
| 154 | 2026-10-17T04:22:00Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 73h30m10s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:437 |
| 155 | 2026-10-17T04:52:00Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 74h00m10s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:438 |
| 156 | 2026-10-17T05:22:00Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 74h30m10s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:439 |
| 157 | 2026-10-17T05:52:00Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 75h00m10s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:440 |
| 158 | 2026-10-17T06:22:01Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5399 · 창 시작 2026-10-14T02:51:50Z 후 75h30m11s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:441 |
| 159 | 2026-10-17T06:52:01Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 76h00m11s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:442 |
| 160 | 2026-10-17T07:22:01Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 76h30m11s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:443 |
| 161 | 2026-10-17T07:52:01Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 77h00m11s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:444 |
| 162 | 2026-10-17T08:22:01Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 77h30m11s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:445 |
| 163 | 2026-10-17T08:52:01Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 78h00m11s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:446 |
| 164 | 2026-10-17T09:22:01Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 78h30m11s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:447 |
| 165 | 2026-10-17T09:52:01Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 79h00m11s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:448 |
| 166 | 2026-10-17T10:22:01Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 79h30m11s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:449 |
| 167 | 2026-10-17T10:52:01Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 80h00m11s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:450 |
| 168 | 2026-10-17T11:22:01Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 80h30m11s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:451 |
| 169 | 2026-10-17T11:52:01Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 81h00m11s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:452 |
| 170 | 2026-10-17T12:22:01Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 81h30m11s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:453 |
| 171 | 2026-10-17T12:52:01Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 82h00m11s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:454 |
| 172 | 2026-10-17T13:22:01Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 82h30m11s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:455 |
| 173 | 2026-10-17T13:52:01Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 83h00m11s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:456 |
| 174 | 2026-10-17T14:22:01Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 83h30m11s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:457 |
| 175 | 2026-10-17T14:52:01Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 84h00m11s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:458 |
| 176 | 2026-10-17T15:22:02Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5399 · 창 시작 2026-10-14T02:51:50Z 후 84h30m12s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:462 |
| 177 | 2026-10-17T15:52:02Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 85h00m12s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:463 |
| 178 | 2026-10-17T16:22:02Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 85h30m12s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:464 |
| 179 | 2026-10-17T16:52:02Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 86h00m12s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:465 |
| 180 | 2026-10-17T17:22:02Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 86h30m12s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:466 |
| 181 | 2026-10-17T17:52:02Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 87h00m12s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:467 |
| 182 | 2026-10-17T18:22:02Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 87h30m12s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:468 |
| 183 | 2026-10-17T18:52:02Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 88h00m12s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:469 |
| 184 | 2026-10-17T19:22:02Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 88h30m12s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:470 |
| 185 | 2026-10-17T19:52:02Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 89h00m12s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:471 |
| 186 | 2026-10-17T20:22:02Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 89h30m12s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:472 |
| 187 | 2026-10-17T20:52:02Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 90h00m12s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:473 |
| 188 | 2026-10-17T21:22:02Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 90h30m12s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:474 |
| 189 | 2026-10-17T21:52:02Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 91h00m12s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:475 |
| 190 | 2026-10-17T22:22:02Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 91h30m12s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:476 |
| 191 | 2026-10-17T22:52:03Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5399 · 창 시작 2026-10-14T02:51:50Z 후 92h00m13s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:477 |
| 192 | 2026-10-17T23:22:03Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 92h30m13s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:478 |
| 193 | 2026-10-17T23:52:03Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 93h00m13s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:479 |
| 194 | 2026-10-18T00:22:03Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 93h30m13s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:480 |
| 195 | 2026-10-18T00:52:03Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 94h00m13s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:481 |
| 196 | 2026-10-18T01:22:03Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 94h30m13s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:482 |
| 197 | 2026-10-18T01:52:03Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 95h00m13s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:483 |
| 198 | 2026-10-18T02:22:03Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 95h30m13s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:484 |
| 199 | 2026-10-18T02:52:03Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 96h00m13s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:485 |
| 200 | 2026-10-18T03:22:03Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 96h30m13s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:486 |
| 201 | 2026-10-18T03:52:03Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 97h00m13s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:487 |
| 202 | 2026-10-18T04:22:03Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 97h30m13s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:488 |
| 203 | 2026-10-18T04:52:03Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 98h00m13s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:489 |
| 204 | 2026-10-18T05:22:03Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 98h30m13s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:490 |
| 205 | 2026-10-18T05:52:03Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 99h00m13s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:491 |
| 206 | 2026-10-18T06:22:03Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 99h30m13s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:492 |
| 207 | 2026-10-18T06:52:04Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5399 · 창 시작 2026-10-14T02:51:50Z 후 100h00m14s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:493 |
| 208 | 2026-10-18T07:22:04Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 100h30m14s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:494 |
| 209 | 2026-10-18T07:52:04Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 101h00m14s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:495 |
| 210 | 2026-10-18T08:22:04Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 101h30m14s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:496 |
| 211 | 2026-10-18T08:52:04Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 102h00m14s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:497 |
| 212 | 2026-10-18T09:22:04Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 102h30m14s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:498 |
| 213 | 2026-10-18T09:52:04Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 103h00m14s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:499 |
| 214 | 2026-10-18T10:22:04Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 103h30m14s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:500 |
| 215 | 2026-10-18T10:52:04Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 104h00m14s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:501 |
| 216 | 2026-10-18T11:22:04Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 104h30m14s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:502 |
| 217 | 2026-10-18T11:52:04Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 105h00m14s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:503 |
| 218 | 2026-10-18T12:22:04Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 105h30m14s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:504 |
| 219 | 2026-10-18T12:52:04Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 106h00m14s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:505 |
| 220 | 2026-10-18T13:22:04Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 106h30m14s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:506 |
| 221 | 2026-10-18T13:52:04Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 107h00m14s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:507 |
| 222 | 2026-10-18T14:22:04Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 107h30m14s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:508 |
| 223 | 2026-10-18T14:52:05Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5399 · 창 시작 2026-10-14T02:51:50Z 후 108h00m15s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:509 |
| 224 | 2026-10-18T15:22:05Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 108h30m15s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:510 |
| 225 | 2026-10-18T15:52:05Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 109h00m15s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:514 |
| 226 | 2026-10-18T16:22:05Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 109h30m15s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:515 |
| 227 | 2026-10-18T16:52:05Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 110h00m15s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:516 |
| 228 | 2026-10-18T17:22:05Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 110h30m15s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:517 |
| 229 | 2026-10-18T17:52:05Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 111h00m15s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:518 |
| 230 | 2026-10-18T18:22:05Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 111h30m15s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:519 |
| 231 | 2026-10-18T18:52:05Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 112h00m15s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:520 |
| 232 | 2026-10-18T19:22:05Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 112h30m15s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:521 |
| 233 | 2026-10-18T19:52:05Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 113h00m15s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:522 |
| 234 | 2026-10-18T20:22:05Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 113h30m15s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:523 |
| 235 | 2026-10-18T20:52:05Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 114h00m15s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:524 |
| 236 | 2026-10-18T21:22:05Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=38862 · ttl_s=7200 · remaining_before_s=5400 · 창 시작 2026-10-14T02:51:50Z 후 114h30m15s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:525 |
| 237 | 2026-10-18T21:45:34Z | sub | `budget_honored` | `serve-lfm25-26b-v031` | budget_honored · floor_mib=38862 · arm_ceiling_mib=35790 · remaining_s=5791 · 창 시작 2026-10-18T21:45:34Z 후 0m00s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:555 |
| 238 | 2026-10-18T23:07:05Z | sub | `budget_expiring` | `serve-lfm25-26b-v031` | budget_expiring · floor_mib=38862 · arm_ceiling_mib=35790 · remaining_s=900 · 창 시작 2026-10-18T21:45:34Z 후 1h21m31s · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:556 |
| 239 | 2026-10-18T23:22:05Z | sub | `budget_expired` | — | budget_expired · arm_ceiling_mib=999999999 · 라벨 없음 → serve-lfm25-26b-v031 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-18T21:45:34Z 후 1h36m31s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:557 |
| 240 | 2026-10-23T16:46:54Z | sub | `watchdog_trip` | — | watchdog_trip · mem_avail_mib=9761 · rule=absolute · threshold_mib=10240 · action=docker_kill · target=2dbdbba9d6af · 라벨 없음 → serve-lfm25-26b-v031 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-14T02:51:50Z 후 229h55m04s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:547 |
| 241 | 2026-10-23T16:46:59Z | sub | `watchdog_kill_ack` | — | watchdog_kill_ack · target=2dbdbba9d6af · 라벨 없음 → serve-lfm25-26b-v031 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-14T02:51:50Z 후 229h55m09s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:548 |
| 242 | 2026-10-23T16:48:23Z | sub | `watchdog_kill_ack` | — | watchdog_kill_ack · target=40cfc8d75da7 · 라벨 없음 → serve-lfm25-26b-v031 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-14T02:51:50Z 후 229h56m33s) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:546 |
| 243 | 2026-10-27T07:31:41Z | sub | `budget_declare` | `single8-sub-lfm25-20261027T062701Z-a02` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=67299 · weights_mib=5144 · kv_mib=38809 · overhead_mib=13356 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 라벨 다름: `single8-sub-lfm25-20261027T062701Z-a02` — 같은 노드 · 이 셀 라벨(`lfm25-26b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · budget_renew 1회 · 닫힘 budget_clear 2026-10-27T07:50:14Z(선언 후 18m33s) · measured_utc 2026-10-29T09:43:49Z 는 이 창 밖(측정 전) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:669 |
| 244 | 2026-10-27T07:31:41Z | sub | `serve_start` | — | serve_start · model=lfm2.5-2.6b · tensor_parallel_size=1 · topology=single · session=serve-lfm25-26b-v031-20261027T073141Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:670 |
| 245 | 2026-10-27T07:31:42Z | sub | `budget_renew` | `single8-sub-lfm25-20261027T062701Z-a02` | budget_renew · floor_mib=67299 · ttl_s=7200 · remaining_before_s=7199 · 창 시작 2026-10-27T07:31:41Z 후 0m01s · 라벨 다름: `single8-sub-lfm25-20261027T062701Z-a02` — 같은 노드 · 이 셀 라벨(`lfm25-26b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:671 |
| 246 | 2026-10-27T07:31:42Z | sub | `budget_honored` | `single8-sub-lfm25-20261027T062701Z-a02` | budget_honored · floor_mib=67299 · arm_ceiling_mib=64227 · remaining_s=7199 · 창 시작 2026-10-27T07:31:42Z 후 0m00s · 라벨 다름: `single8-sub-lfm25-20261027T062701Z-a02` — 같은 노드 · 이 셀 라벨(`lfm25-26b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:637 |
| 247 | 2026-10-27T07:33:24Z | sub | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=70787 · rate_mib_s=36558 · max_rate_mib_s=17801 · legacy_rule_would_trip=true · 라벨 없음 → single8-sub-lfm25-20261027T062701Z-a02 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-27T07:31:42Z 후 1m42s) · 라벨 다름: `single8-sub-lfm25-20261027T062701Z-a02` — 같은 노드 · 이 셀 라벨(`lfm25-26b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:638 |
| 248 | 2026-10-27T07:49:00Z | 미관측 | `measurement` | `measurement (other label's budget window)` | 같은 셀의 다른 측정(측정 노드 축 sub) · 시각 = 측정 조립(sweep generated_utc = 인증서 measured_utc · 시작 시각 미기록) · 이 발행의 측정 2026-10-29T09:43:49Z 전 · 발행 기록 camp_26102705_codex_single8__sub__lfm25_26b_v031(셀 대조 simlog:sweep_index.meta.config_name · verdict PASS) · simlog 사본 generated_utc 2026-10-27T07:49:00Z(같음) / 출력 평면에 보존된 같은 셀의 다른 측정 · 원장: 이 셀 라벨의 선언은 없고 다른 라벨의 창 안: single8-sub-lfm25-20261027T062701Z-a02 2026-10-27T07:31:41Z~2026-10-27T07:50:14Z (docs/logs/sub/events/2026-10.jsonl:669) · single8-sub-lfm25-20261027T062701Z-a02 2026-10-27T07:31:42Z~2026-10-27T07:50:15Z (docs/logs/sub/events/watchdog.jsonl:637) · 직전 예산 행 budget_honored(single8-sub-lfm25-20261027T062701Z-a02) 2026-10-27T07:31:42Z(docs/logs/sub/events/watchdog.jsonl:637) · 직후 예산 행 budget_clear 2026-10-27T07:50:14Z(docs/logs/sub/events/2026-10.jsonl:672) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/simlog/26102717_camp_26102705_codex_single8__sub__lfm25_26b_v031/sweep_index.json:4 |
| 249 | 2026-10-27T07:50:14Z | sub | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → single8-sub-lfm25-20261027T062701Z-a02 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-27T07:31:41Z 후 18m33s) · 라벨 다름: `single8-sub-lfm25-20261027T062701Z-a02` — 같은 노드 · 이 셀 라벨(`lfm25-26b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:672 |
| 250 | 2026-10-27T07:50:15Z | sub | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → single8-sub-lfm25-20261027T062701Z-a02 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-27T07:31:42Z 후 18m33s) · 라벨 다름: `single8-sub-lfm25-20261027T062701Z-a02` — 같은 노드 · 이 셀 라벨(`lfm25-26b-v031`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/watchdog.jsonl:639 |
| 251 | 2026-10-27T07:50:58Z | sub | `serve_stop` | — | serve_stop · model=lfm2.5-2.6b · duration_s=1157 · session=serve-lfm25-26b-v031-20261027T073141Z · 같은 셀 이름 · 앞선 캠페인(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀의 시도로 세지 않는다) | docs/logs/sub/events/2026-10.jsonl:674 |
| 252 | 2026-10-29T09:26:07Z | sub | `budget_declare` | `serve-lfm25-26b-v031` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=55855 · weights_mib=5144 · kv_mib=38809 · overhead_mib=24800 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 기동 창 2/2: budget_renew 1회 · 닫힘 budget_clear 2026-10-29T09:44:21Z(선언 후 18m14s) · measured_utc 2026-10-29T09:43:49Z 가 이 창 안(이 측정의 기동) | docs/logs/sub/events/2026-10.jsonl:703 |
| 253 | 2026-10-29T09:26:07Z | sub | `serve_start` | — | serve_start · model=lfm2.5-2.6b · tensor_parallel_size=1 · topology=single · session=serve-handoff2-lfm25 | docs/logs/sub/events/2026-10.jsonl:704 |
| 254 | 2026-10-29T09:26:07Z | sub | `budget_honored` | `serve-lfm25-26b-v031` | budget_honored · floor_mib=55855 · arm_ceiling_mib=52783 · remaining_s=7200 · 창 시작 2026-10-29T09:26:07Z 후 0m00s | docs/logs/sub/events/watchdog.jsonl:656 |
| 255 | 2026-10-29T09:26:08Z | sub | `budget_renew` | `serve-lfm25-26b-v031` | budget_renew · floor_mib=55855 · ttl_s=7200 · remaining_before_s=7199 · 창 시작 2026-10-29T09:26:07Z 후 0m01s | docs/logs/sub/events/2026-10.jsonl:705 |
| 256 | 2026-10-29T09:26:18Z | sub | `engine_log_window_start` | — | 캡처된 엔진 로그의 첫 시각(꼬리 캡처일 수 있다 — 엔진 시작 시각이 아닐 수 있다) · 선언 2026-10-29T09:26:07Z 후 0m11s · 시계: 컨테이너 시각을 UTC 로 읽어 측정 창 [2026-10-29T09:26:07Z, 2026-10-29T09:44:21Z] 안에 드는 것을 확인 | output/single/benchlog/sweep_lfm25_handoff2_20261029_run3/lite_engine_lfm25-26b-v031.log:36 |
| 257 | 2026-10-29T09:26:30Z | sub | `engine_prefetch_disabled` | — | 가중치 auto-prefetch 꺼짐 줄 ×1 — filesystem CIFS(vLLM 로그 문구) | output/single/benchlog/sweep_lfm25_handoff2_20261029_run3/lite_engine_lfm25-26b-v031.log:57 |
| 258 | 2026-10-29T09:27:01Z | sub | `engine_weights_loaded` | — | 가중치 적재 완료 줄 ×1(rank·단계별) · 최장 31.08s · 이 행 = 마지막 줄 | output/single/benchlog/sweep_lfm25_handoff2_20261029_run3/lite_engine_lfm25-26b-v031.log:63 |
| 259 | 2026-10-29T09:27:01Z | sub | `engine_model_loaded` | — | 모델 적재 완료 줄 ×1(rank 별) · 마지막 줄 5.05 GiB · 33.6s | output/single/benchlog/sweep_lfm25_handoff2_20261029_run3/lite_engine_lfm25-26b-v031.log:64 |
| 260 | 2026-10-29T09:27:03Z | sub | `engine_kv_cache_sized` | — | GPU KV cache 2,476,552 tokens · 요청당 32,768 tokens 기준 최대 동시성 75.58x | output/single/benchlog/sweep_lfm25_handoff2_20261029_run3/lite_engine_lfm25-26b-v031.log:75 |
| 261 | 2026-10-29T09:27:04Z | sub | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=70537 · rate_mib_s=39199 · max_rate_mib_s=17801 · legacy_rule_would_trip=true · 라벨 없음 → serve-lfm25-26b-v031 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T09:26:07Z 후 0m57s) | docs/logs/sub/events/watchdog.jsonl:657 |
| 262 | 2026-10-29T09:28:24Z | sub | `engine_init_done` | — | 엔진 초기화(profile·KV 할당·warmup) 82.56s | output/single/benchlog/sweep_lfm25_handoff2_20261029_run3/lite_engine_lfm25-26b-v031.log:90 |
| 263 | 2026-10-29T09:28:26Z | sub | `server_starting` | — | API 서버 기동 — 이 측정 창 선언 2026-10-29T09:26:07Z 후 2m19s · 캡처 로그 첫 시각 2026-10-29T09:26:18Z 후 2m08s | output/single/benchlog/sweep_lfm25_handoff2_20261029_run3/lite_engine_lfm25-26b-v031.log:101 |
| 264 | 2026-10-29T09:44:21Z | sub | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → serve-lfm25-26b-v031 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T09:26:07Z 후 18m14s) | docs/logs/sub/events/2026-10.jsonl:706 |
| 265 | 2026-10-29T09:44:22Z | sub | `serve_stop` | — | serve_stop · model=lfm2.5-2.6b · duration_s=1095 · session=serve-handoff2-lfm25 | docs/logs/sub/events/2026-10.jsonl:707 |
| 266 | 2026-10-29T09:44:22Z | sub | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → serve-lfm25-26b-v031 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-29T09:26:07Z 후 18m15s) | docs/logs/sub/events/watchdog.jsonl:658 |

**기동 시도**(예산 선언 `budget_declare` 마다 한 시도 · 같은 노드의 이어지는 행과 선언 없는 노드(엔진 로그)의 그 시각 행을 묶었다 · 사살 · 트립 · 거부는 닫힘과 따로 센다 · `measurement` 행은 측정 기록이라 시도로 세지 않는다(위 표에만) — 해석은 저작자 몫)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 1 | sub | `serve-lfm25-26b-v031` | 2026-10-29T09:26:07Z | 1회 · 첫 2026-10-29T09:26:08Z | 없음 | `budget_clear` 2026-10-29T09:44:21Z | 12 |

**캠페인 경계 이전 기동 2회**(캠페인 경계 2026-10-29T09:21:44Z 이전 — 이 셀 라벨로 묶인 1회 · 라벨 다름(같은 노드 · 창 밖 이 셀 행과 시간이 겹치는 다른 라벨 선언 창) 1회 · 어느 쪽도 이 셀의 시도로 세지 않는다 · 행 전문 = 01 §1.4)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 앞선-1 | sub | `serve-lfm25-26b-v031` | 2026-10-14T02:51:50Z | 230회 · 첫 2026-10-14T02:51:51Z | `watchdog_kill_ack` 2 · `watchdog_trip` 1 | `budget_expired` 2026-10-18T23:22:05Z | 5 |
| 앞선-2 | sub | `single8-sub-lfm25-20261027T062701Z-a02`(라벨 다름 — 같은 노드 · 이 셀 라벨 아님) | 2026-10-27T07:31:41Z | 1회 · 첫 2026-10-27T07:31:42Z | 없음 | `budget_clear` 2026-10-27T07:50:14Z | 5 |
<!-- /FACT:event_timeline -->

<!-- FACT:measurement_env -->
_측정 실행의 env 관측 없음 — 측정 엔진 로그에서 env echo 줄(`<KEY> set by environment to <VALUE>` 등)을 찾지 못했다(evidence.measurement_env_observed · 부재 ≠ 미설정)._
<!-- /FACT:measurement_env -->

> 이 절이 답하는 질문: 수신자가 이 zip 만으로 이 셀을 다시 띄우려면 어떤 순서로 무엇을 하고, 각 단계의 성공을 무엇으로 판정하며, 얼마나 기다려야 하는가?

사람이 1.6의 모델과 마운트를 준비한다. 기존 이미지 및 triplet을 확인하고 single_serve_up으로 예산을 선언한 뒤 기동한다. 이번 선언은 ready-max 900초와 overhead 24800 MiB이며 health 200까지 140초였다. completion, reasoning, tool smoke를 확인한 뒤 lite 및 GuideLLM 기본 형상의 각 레벨 3회를 실행했다. 측정 뒤 같은 세션을 지정하여 정규 down으로 컨테이너, 예산 및 갱신 루프를 회수했다. 이 절차의 값은 대상 환경에서 다시 산정한다(새 셀 testlog).

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

### 가중치
모델 획득은 managed 모드다. 이 작업은 다운로드하지 않았다. 사람이 기존 LiquidAI/LFM2.5-2.6B의 config.json, 두 safetensors shard, tokenizer 파일, chat_template.jinja 존재와 읽기 권한을 준비하고 manifest.nas_model_path에서 /app/models로 마운트되는지 확인한다. 재현 전 `python3 .claude/skills/vllm-recipe-explorer/scripts/parse_model_config.py '<manifest.nas_model_path>/Liquid-AI/LFM2.5-2.6B' --json`과 crosscheck_model_card.py로 헤더/dtype를 검사한다. 이 명령의 자리표시 경로는 대상 노드에서 해소한다(출처 devlog_26102716_lfm25_default_full_신규실측 §Grounded recipe retained 및 testlog_26102716_lfm25_default_full_신규판정 §Version and observability).

### 스테이징
PLE/별도 runtime patch 스테이징은 해당 없음이다. 모델 경로·셀 env·triplet은 artifacts의 대응 자리에 두고, GuideLLM tokenizer stage는 공식 run_bench가 로컬로 만든다. 엔진 파일과 레시피의 포트 층을 맞추며, 네트워크 FS라는 이유만으로 이 셀에 PLE mmap 처방을 추가하지 않는다(artifacts/triplet/lfm25-26b-v031.yaml, 03 §3.4).

### 네트워크/오프라인 준비
이미지 빌드를 새로 하면 Dockerfile의 apt/git/pip 단계와 베이스 이미지가 외부 접근을 요구한다. 이번 실행은 사전 설치된 serving 이미지와 GuideLLM 이미지를 사용했다. 오프라인 재현이면 그 입력을 사람이 미리 확보하고, 모델 acquisition gate를 별도로 따른다. 근거 없는 다운로드 명령을 검증된 절차로 제시하지 않는다. 실행 환경/자격증명 원문은 배포하지 않는다.
