# 01 · 산출물 — 무엇이 실제로 쓰였고, 어떻게 다시 띄우는가

## 1.1 적용 판정 표
> 기계가 적용 증거(빌드 원장 · 원장이 없으면 라벨된 재구성) × 파일 × 선언을 대사한 결과다. `artifacts/` 에는 재현 입력과 확인되지 않은 참고 형상도 포함될 수 있다. 파일별 검증 상태와 근거를 아래 표에서 확인한다.

<!-- FACT:slots -->
| 슬롯 | 실림 | 파일 | 신호 | 적용 증거 | 사유 |
|---|---|---|---|---|---|
| `build_patch_post` | 안 실림 | — | 1-signal | build-ledger:docker-image(sub-local):sha256:5534cf107221da5513496c6b4b47bbe1ef63a794c0034a0ac1a037ec17e59991:/opt/easy-vllm/build_ledger.json · 빌드 시 원장(후속 이미지 층은 별도 관측) | 적용 판정 없음 — 원장 관측(기계 파생) |
| `build_patch_pre` | 안 실림 | — | 1-signal | build-ledger:docker-image(sub-local):sha256:5534cf107221da5513496c6b4b47bbe1ef63a794c0034a0ac1a037ec17e59991:/opt/easy-vllm/build_ledger.json · 빌드 시 원장(후속 이미지 층은 별도 관측) | 적용 판정 없음 — 원장 관측(기계 파생) |
| `build_recipe` | 실림 | `artifacts/build_recipe/Dockerfile.source-build` · `artifacts/build_recipe/requirements.txt` | 3-signal | build-ledger:build-ledger.dockerfile | 실린 Dockerfile과 requirements는 실제 빌드 바이트와 일치가 확인되지 않은 참고 형상이다. source build 원장은 후속 dependency 계층을 기록하지 않는다. |
| `compose` | 실림 | `artifacts/compose/.env.template` · `artifacts/compose/docker-compose.yaml` | 1-signal | file:output/single/docker-compose.yaml | single 서빙 경로(compose · 러너 · env 형상) — 측정 당시 개정(selected_revisions)(기계 파생) |
| `fork_pin` | 안 실림 | — | 1-signal | none:.claude/policies/arch_variant_ledger.json | 셀 env 에 VARIANT 줄 없음 = stock(workflow.md §변종 좌표의 거처)(기계 파생) |
| `runtime_patch` | 안 실림 | — | 1-signal | none:output/single/configs/gptoss120b-0191-c0_patch.py | output/single/configs/gptoss120b-0191-c0_patch.py 없음 — arm_patch.sh 는 no-op(런타임 패치 불해당)(기계 파생) |
| `triplet` | 실림 | `artifacts/triplet/.env.gptoss120b-0191-c0` · `artifacts/triplet/gptoss120b-0191-c0.sh` · `artifacts/triplet/gptoss120b-0191-c0.yaml` | 2-signal | file:output/single/configs/gptoss120b-0191-c0.yaml | 셀 id gptoss120b-0191-c0 = 트리플렛 이름(configs/gptoss120b-0191-c0.yaml · .sh · envs/.env.gptoss120b-0191-c0)(기계 파생) |

**레시피 경고 · 실린 리비전**(기계 관측 — 수신자 빌드가 여기서 갈라질 수 있다)

- ⚠ `build_recipe` `output/multi/Dockerfile.source-build` — 실린 바이트(git:c50b821f4bb18380d019a1589e7b6d5c61406483)가 측정 이미지 docker history 의 RUN·COPY 와 전수 일치하지 않는다(15/20)(이미지 Created 2026-10-28T21:21:51Z · 실린 개정 커밋 2026-09-30T23:12:15Z · 이 체크아웃 통로 `output/single/Dockerfile.source-build` 의 파일이 아니다)
- ⚠ `build_recipe` `output/single/requirements.txt` — 빌드 시 원장의 COPY 원천 sha256과 실린 작업트리 바이트가 다르다(이미지 파일 직접 관측 아님)(이미지 Created 2026-10-28T21:21:51Z · 마지막 커밋 2026-10-14T01:51:45Z)
- `build_recipe` `output/multi/Dockerfile.source-build`(이 체크아웃 통로 `output/single/Dockerfile.source-build` 대신) 실린 리비전: 커밋 `c50b821f4bb1` · 선택 방법 `git log --all -- output/*/Dockerfile.source-build · c50b821f4bb1:output/multi/Dockerfile.source-build 가 docker history RUN·COPY 와 가장 잘 맞는 이력 개정(15/20 · 전수 일치 개정 없음 · 불일치 레시피에만: RUN echo "WARN: source-build key (26.07-py3 x vLLM 0.30.0) is UNVALIDATED — attempt-build under HITL override (--allow-u \| 레시피에만: RUN python3 use_existing_torch.py \| 이미지에만: RUN python3 -m pip install --no-cache-dir -r /tmp/compat-requirements.txt && python3 -m pip check \| 이미지에만: COPY compat-requirements.txt /tmp/compat-requirements.txt \| 이미지에만: RUN if [ -f tools/use_existing_torch.py ]; then python3 tools/use_existing_torch.py; elif [ -f use_existing_torch.py ];  · 작업트리 11/23) — 이 체크아웃 통로(output/single/Dockerfile.source-build) 밖 — 이 개정의 바이트를 싣는다(git cat-file)` · docker history 대조 15/20 · 실린 바이트 `git:c50b821f4bb18380d019a1589e7b6d5c61406483` · 쓰인 바이트임을 관측으로 확인 아니오
- `build_recipe` `output/single/requirements.txt` 실린 리비전: 리비전 미특정 · 선택 방법 `worktree(빌드 원장 sha256 3c21baedb2550515 과 작업트리·이미지 Created 이전 개정 · 전 이력(output/*/requirements.txt · 모든 ref)이 모두 다르다 — 쓰인 바이트가 아니다 · recipe_vs_image 경고)` · docker history 대조 없음 · 실린 바이트 `worktree` · 쓰인 바이트임을 관측으로 확인 아니오
- `compose` `output/single/docker-compose.yaml` 실린 리비전: 커밋 `80284db86dd7` · 선택 방법 `worktree(mtime 2026-10-25T22:53:44Z ≤ 측정 2026-10-28T22:11:36Z(sweep_index.generated_utc) · 바이트 = 측정 전 마지막 커밋 80284db86dd7)` · docker history 대조 없음 · 실린 바이트 `worktree` · 근거 `mtime≤measured` · 쓰인 바이트임을 관측으로 확인 아니오

**싣지 않은 파일**(슬롯별 사유 — 빌드 때 skip 된 패치는 아래 적용 집합 표에 있다)

- `build_patch_post` `10-deepgemm.sh` — `not-in-ledger(원장에 없음 — 원장 context_inventory.build_patch_post = ['.gitkeep'] — 빌드 때 빌드 컨텍스트에 들지 않았다 · 싣지 않음)`
- `build_patch_post` `20-triton-kernels.sh` — `not-in-ledger(원장에 없음 — 원장 context_inventory.build_patch_post = ['.gitkeep'] — 빌드 때 빌드 컨텍스트에 들지 않았다 · 싣지 않음)`
- `build_patch_post` `30-mxfp4-triton-sm121.sh` — `not-in-ledger(원장에 없음 — 원장 context_inventory.build_patch_post = ['.gitkeep'] — 빌드 때 빌드 컨텍스트에 들지 않았다 · 싣지 않음)`
- `build_patch_post` `40-humming-nvml-gb10.sh` — `not-in-ledger(원장에 없음 — 원장 context_inventory.build_patch_post = ['.gitkeep'] — 빌드 때 빌드 컨텍스트에 들지 않았다 · 싣지 않음)`
- `build_patch_pre` `50-dsv4-sm12x-port.sh` — `not-in-ledger(원장에 없음 — 원장 context_inventory.build_patch_pre = ['.gitkeep'] — 빌드 때 빌드 컨텍스트에 들지 않았다 · 싣지 않음)`
- `build_patch_pre` `55-src-deps-authority.sh` — `not-in-ledger(원장에 없음 — 원장 context_inventory.build_patch_pre = ['.gitkeep'] — 빌드 때 빌드 컨텍스트에 들지 않았다 · 싣지 않음)`
- `build_patch_pre` known-non-patch 1건: `PROVENANCE.json`
- `compose` `arm_patch.sh` — `not-armed(output/single/configs/gptoss120b-0191-c0_patch.py 없음(CONFIG_FILE) — arm_patch.sh 는 no-op · arm_patch.sh 측정 당시 개정(fad2d7c6f157) `_cfg=` 키 체인 RUNTIME_PATCH_STEM > CONFIG_FILE > 'default')`

**파일별 검증 · 관련성**(`generated-unverified` = 실행 검증되지 않은 생성물 — 파일 첫 줄 경고 · PAYLOAD 파일 기록과 같은 표시 · 관련성 = ① 패치 선언 대상 × 모델 config ② 엔진 로그 발화 서명 — 둘 다 관측일 때만 required/inactive-inferred)

| 슬롯 | 파일 | 검증 | 검증 근거 | 관련성 | 관련성 근거 |
|---|---|---|---|---|---|
| `build_recipe` | `artifacts/build_recipe/Dockerfile.source-build` | `generated-unverified` · renderer | 실린 Dockerfile.source-build(git:c50b821f4bb18380d019a1589e7b6d5c61406483) · docker history RUN·COPY 15/20 일치 — Dockerfile 전체 원본 바이트의 동일성은 확인하지 못했다(_select_dockerfile · git log --all -- output/*/Dockerfile.source-build · c50b821f4bb1:output/multi/Dockerfile.source-build 가 docker history RUN·COPY 와 가장 잘 맞는 이력 개정(15/20 · 전수 일치 개정 없음 · 불일치 레시피에만: RUN echo "WARN: source-build key (26.07-py3 x vLLM 0.30.0) is UNVALIDATED — attempt-build under HITL override (--allow-u \| 레시피에만: RUN python3 use_existing_torch.py \| 이미지에만: RUN python3 -m pip install --no-cache-dir -r /tmp/compat-requirements.txt && python3 -m pip check \| 이미지에만: COPY compat-requirements.txt /tmp/compat-requirements.txt \| 이미지에만: RUN if [ -f tools/use_existing_torch.py ]; then python3 tools/use_existing_torch.py; elif [ -f use_existing_torch.py ];  · 작업트리 11/23) — 이 체크아웃 통로(output/single/Dockerfile.source-build) 밖 — 이 개정의 바이트를 싣는다(git cat-file)) | `unknown` | 파일의 실행 당시 바이트가 확인되지 않았다(generated-unverified · 생성 renderer) — 이 표시만으로 경로 실행 여부나 필요 여부를 판정하지 않는다(참고 형상) |
| `build_recipe` | `artifacts/build_recipe/requirements.txt` | `generated-unverified` · renderer | 실린 requirements.txt(worktree · sha256 ff5b5c72731459c9)이 빌드 원장 sha256 3c21baedb2550515 과 다르다 — 이 이미지를 지은 바이트임을 관측으로 확인하지 못했다(참고 형상)(_select_copied · worktree(빌드 원장 sha256 3c21baedb2550515 과 작업트리·이미지 Created 이전 개정 · 전 이력(output/*/requirements.txt · 모든 ref)이 모두 다르다 — 쓰인 바이트가 아니다 · recipe_vs_image 경고)) | `unknown` | 파일의 실행 당시 바이트가 확인되지 않았다(generated-unverified · 생성 renderer) — 이 표시만으로 경로 실행 여부나 필요 여부를 판정하지 않는다(참고 형상) |
| `compose` | `artifacts/compose/.env.template` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/docker-compose.yaml` | `generated-unverified` · renderer | docker-compose.yaml: worktree(mtime 2026-10-25T22:53:44Z ≤ 측정 2026-10-28T22:11:36Z(sweep_index.generated_utc) · 바이트 = 측정 전 마지막 커밋 80284db86dd7) — 측정 당시 실행 바이트는 별도 관측되지 않았다 | `unknown` | 파일의 실행 당시 바이트가 확인되지 않았다(generated-unverified · 생성 renderer) — 이 표시만으로 경로 실행 여부나 필요 여부를 판정하지 않는다(참고 형상) |
| `triplet` | `artifacts/triplet/.env.gptoss120b-0191-c0` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/gptoss120b-0191-c0.sh` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/gptoss120b-0191-c0.yaml` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
<!-- /FACT:slots -->

<!-- FACT:applied_set -->
**적용 집합** — 상태 `observed` · 출처 `docker-image(sub-local):sha256:5534cf107221da5513496c6b4b47bbe1ef63a794c0034a0ac1a037ec17e59991:/opt/easy-vllm/build_ledger.json · 빌드 시 원장(후속 이미지 층은 별도 관측)`

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
| attestation | absent | output/single/benchlog/attestation_gptoss120b-0191-c0.json(부재) |
| local-image | observed | sha256:5534cf107221da5513496c6b4b47bbe1ef63a794c0034a0ac1a037ec17e59991 |
<!-- /FACT:applied_set -->

## 1.2 슬롯별 적용 사유
> 이 절이 답하는 질문: 실린 산출물 하나하나는 무엇을 고치며, 왜 이 셀에 필요했고, 이 셀에서 실제로 발화했으며, 언제 불필요해지는가?

### build_recipe
실린 Dockerfile과 requirements는 실제 빌드 바이트와 일치가 확인되지 않은 참고 형상이다. source build 원장은 후속 dependency 계층을 기록하지 않는다.

### compose와 triplet
compose는 단일 서비스 기동 형상이고 triplet은 실제 셀 설정이다. 별도 런타임 패치는 기록되지 않았다. 빌드 패치의 적용 여부는 위 원장 범위를 따른다.

## 1.3 값의 지위표
> 서빙 설정의 노브 **전부**와 기계가 모은 후보 지위다(lockset 출처 · yaml 주석 · 엔진 기본값). 최종 지위는 아래 `kind: value-status` 블록이 정한다 — 표의 노브마다 블록이 정확히 하나 있어야 린터를 통과한다.

<!-- FACT:value_status -->
서빙 노브 4개 — 노브마다 `kind: value-status` 블록이 **정확히 1개** 있어야 한다(노브·값은 이 표의 글자 그대로 · 값 칸의 `\|` 는 `|` 로 적는다).

| 노브 | 값 | 후보 지위 | 후보 근거 | yaml 주석 | 출처 |
|---|---|---|---|---|---|
| `gpu-memory-utilization` | 0.8 | declared-requirement | lockset 출처 gmu_source=target_gmu · lockset provenance=explorer-phase2 | — | lockset.gmu_source=target_gmu |
| `max-model-len` | 8192 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `kv-cache-memory-bytes` | 467380993 | tuned | lockset 출처 kv_source=measured-clamp · lockset provenance=explorer-phase2 | — | lockset.kv_source=measured-clamp |
| `quantization` | mxfp4 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
<!-- /FACT:value_status -->

> 이 절이 답하는 질문: 각 서빙 노브의 값은 이 셀에서 어떤 지위인가 — 조정됐나, 승계됐나, 음성대조로 일부러 둔 것인가, 엔진 기본값인가, 필요조건인가?

```hint-event
id: V1
kind: value-status
노브: gpu-memory-utilization
값: 0.8
지위: inherited
근거: 손레버 선언, 제거 실패 미측정
출처: [devlog_26102907_GPT120_v0320_실측과_인계 §GPT-OSS-120B v0.32.0 sub 실측 인계]
```
```hint-event
id: V2
kind: value-status
노브: max-model-len
값: 8192
지위: inherited
근거: 손레버 후보, 최적화 미측정
출처: [devlog_26102907_GPT120_v0320_실측과_인계 §GPT-OSS-120B v0.32.0 sub 실측 인계]
```
```hint-event
id: V3
kind: value-status
노브: kv-cache-memory-bytes
값: 467380993
지위: tuned
근거: 두 trial의 측정 기반 클램프
출처: [devlog_26102907_GPT120_v0320_실측과_인계 §GPT-OSS-120B v0.32.0 sub 실측 인계]
```
```hint-event
id: V4
kind: value-status
노브: quantization
값: mxfp4
지위: inherited
근거: 체크포인트 고유 양자화
출처: [devlog_26102907_GPT120_v0320_실측과_인계 §GPT-OSS-120B v0.32.0 sub 실측 인계]
```

KV와 gmu는 새 환경에서 다시 측정한다. max-num-seqs는 지정하지 않았다.

## 1.4 재현 절차
> 셀 실행 기록에서 기계가 파생한 명령 순서와 단계별 소요다. 소요가 `미관측` 인 단계는 시각 기록이 없다는 뜻이다(0 이 아니다). 그 아래는 블랙박스 이벤트 원장에서 이 셀의 행을 기계가 뽑아 기동 시도로 묶은 것이고, 마지막은 측정한 실행의 엔진 로그가 echo 한 env 다(측정 당시 값의 정본).

<!-- FACT:reproduce -->
| 순서 | 단계 | 성공 판정 | 소요 | 관측 구간(UTC) | 소요 출처 | 명령 출처 |
|---|---|---|---|---|---|---|
| 1 | build | 재현 예시 · 측정 이미지와 동일 여부 별도 확인 | 37분 55초 | 2026-10-28T20:26:07Z → 2026-10-28T21:04:02Z (exact) | campaigns/camp-26102705-codex-single8/phases/sub/build.status.json · 기록된 phase 구간(이 재구성 명령 또는 측정 이미지의 전체 빌드 소요로 결속되지 않음) | reconstructed(build-ledger build_args + compose build 선언/설정(실행 관측 아님)) |
| 2 | serve | health 200 + 추론 1회(스모크 판정) | ≤ 7분 4초(상한) | 2026-10-28T21:43:23Z → 2026-10-28T21:50:27Z (upper) | docs/logs/sub/sessions/gpt120-v0320-20261028T2143.json started_utc(측정을 포함하는 세션) → 첫 측정 시각(output/single/benchlog/sweep_gpt120v0320_20261028T2143/lite_raw_gptoss120b-0191-c0.json measured_utc(호스트 date -u · lite_bench.sh 가 cold 레그 직전에 찍는 측정 시작)) | derived(스모크 사용법) |
| 3 | bench | 리포트 발행(루브릭 권한 explicit ∧ PASS일 때만 full 인증서) | 21분 9초 | 2026-10-28T21:50:27Z → 2026-10-28T22:11:36Z (exact) | output/single/benchlog/sweep_gpt120v0320_20261028T2143/lite_raw_gptoss120b-0191-c0.json measured_utc(호스트 date -u · lite_bench.sh 가 cold 레그 직전에 찍는 측정 시작) → sweep_index.generated_utc(호스트 UTC · 두 끝 같은 시계) | reconstructed(bench json + 측정 도구 원문 lite 인자) |

**빌드 층 시각**(관측 · 소요가 아니라 층 창 — 캐시 재사용 층은 이전 빌드의 시각을 가진다)

- `build` 이미지 층 CreatedAt(docker history): 2026-10-28T20:26:31Z → 2026-10-28T21:21:51Z · 층 26개 · 출처 `docker history --no-trunc --format '{{json .}}' sha256:5534cf107221da5513496c6b4b47bbe1ef63a794c0034a0ac1a037ec17e59991` — 우리 Dockerfile 층의 CreatedAt 범위 — 캐시 재사용 층은 이전 빌드 시각을 가진다(소요 아님)

**단계별 명령**(기계 파생 — `명령 출처`가 `derived(…)`이면 사용법에서 파생하고 `reconstructed(…)`이면 괄호 안에 명시된 관측, 파일 선언·설정 또는 추론을 조합해 재구성한다. 예를 들어 docker history와 bench JSON은 기록된 관측이며 compose 파일은 선언·설정이다. 설정만으로 과거 compose build 실행 여부는 알 수 없다. 어느 명령도 실행 원문(로그에 남은 줄)은 아니다)

**1. build**

```sh
# 실린 artifacts/build_recipe/Dockerfile.source-build(실린 개정: git:c50b821f4bb18380d019a1589e7b6d5c61406483(output/multi/Dockerfile.source-build) — 이미지를 지은 바이트임을 관측으로 확인 못함(docker history RUN·COPY 15/20))를 output/single/Dockerfile.source-build 자리에 두는 재현 예시다(실제 빌드 컨텍스트 아님).
# 독립 재현용 태그를 지정한다. 측정 이미지와 동일한 바이트·후속 빌드 층까지 재현되는지는 별도 검증한다.
mkdir -p output/single/build_patches_src output/single/build_patches   # 빌드 원장 context_inventory.build_patch_pre = ['.gitkeep'] · 빌드 원장 context_inventory.build_patch_post = ['.gitkeep']
docker build -f output/single/Dockerfile.source-build \
  --build-arg BUILD_JOBS=8 \
  --build-arg RAY_VERSION=2.48.0 \
  --build-arg SM12X_PORT=0 \
  --build-arg SRC_DEPS_AUTHORITY=0 \
  --build-arg TORCH_CUDA_ARCH=12.1 \
  --build-arg VLLM_PRETEND_VERSION= \
  --build-arg VLLM_REF=5df63a7203cdb9328d824a27b682e77c7b9928be \
  --build-arg VLLM_REPO=https://github.com/vllm-project/vllm.git \
  -t "${REPRO_IMAGE_TAG:?set a separate reproduction image tag}" \
  output/single
# 실린 레시피의 이력 출처는 output/multi다. 이 zip 재현 예시는 현재 토폴로지 통로 output/single/를 사용하며 실제 빌드 경로와 동등 여부는 별도 확인한다. 실린 compose 파일만 단독 사용하고 override가 없을 때의 설정을 확인하라. 실제 원본·후속층 실행 경로는 별도 실행 기록에서 확인한다.
```

**2. serve**

```sh
bash .claude/skills/upstream-version-watch/scripts/single_serve_up.sh gptoss120b-0191-c0
```

**3. bench**

```sh
# bench JSON 필드로 복원: backend · model_id→--model · tokenizer_id→--tokenizer · num_prompts · max_concurrency · request_rate · burstiness · 요청당 입출력 길이 = total_*_tokens ÷ completed(나누어떨어질 때만 — 평균이다 · 요청별 길이는 JSON 에 없다) · 나누어떨어지면 `--dataset-name random` 으로 추정했다(측정 도구가 그 꼴로 부른다 — 아래 도구 원문).
# JSON 에 없는 인자(--endpoint · --base-url · --ignore-eos · --num-warmups · --temperature · --seed · --random-range-ratio · --trust-remote-code)는 측정 도구가 정했다 — level_* = run_bench.sh · lite_cold/lite_warm = lite_bench.sh(.claude/skills/adversarial-benchmark/scripts/) · 셀 스윕 순서는 sweep_bench.sh.
# lite_cold/lite_warm 의 데이터셋 · 부하 인자 = 측정 시점 도구 원문 lite_bench.sh@a7083a8f933d L177-L183(리터럴 인자만 · `$변수` 인자는 도구가 값을 정하지 않아 JSON 필드로 채운다) — `total_*_tokens ÷ completed` 는 실측 토큰(chat 템플릿 포함)이지 인자가 아니다.
# output/single/benchlog/sweep_gpt120v0320_20261028T2143/lite_cold_gptoss120b-0191-c0.json (date 20261028-215044) · 실측 토큰(chat 템플릿 포함 · total ÷ completed · 인자 아님): 입력 512/1 = 512 · 출력 128/1 = 128
vllm bench serve --backend openai --model gpt-oss-120b --tokenizer /app/models/OpenAI/gpt-oss-120b --trust-remote-code --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 --ignore-eos --num-prompts 1 --max-concurrency 1 --request-rate inf
# output/single/benchlog/sweep_gpt120v0320_20261028T2143/lite_warm_gptoss120b-0191-c0.json (date 20261028-215109) · 실측 토큰(chat 템플릿 포함 · total ÷ completed · 인자 아님): 입력 1536/3 = 512 · 출력 384/3 = 128
vllm bench serve --backend openai --model gpt-oss-120b --tokenizer /app/models/OpenAI/gpt-oss-120b --trust-remote-code --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 --ignore-eos --num-prompts 3 --max-concurrency 1 --request-rate inf
```

**슬롯 → 재구성 예시 컨텍스트 매핑**(예시 위치 = `output/<토폴로지>/` · 원천 = 실린 Dockerfile 의 COPY · compose 설정; 실제 실행 경로는 별도 기록 확인) — 재구성 예시 통로 `output/single/`(실제 원본·후속층의 빌드 경로 증명 아님 · 별도 실행 기록 확인)

| zip 경로 | 빌드 컨텍스트 자리 | 근거 |
|---|---|---|
| `artifacts/triplet/.env.gptoss120b-0191-c0` | `envs/.env.gptoss120b-0191-c0` | compose env_file envs/.env.${CONFIG_FILE}(셀 env) |
| `artifacts/triplet/gptoss120b-0191-c0.sh` | `configs/gptoss120b-0191-c0.sh` | compose volumes ./configs:/app/configs |
| `artifacts/triplet/gptoss120b-0191-c0.yaml` | `configs/gptoss120b-0191-c0.yaml` | compose volumes ./configs:/app/configs |
| `artifacts/build_recipe/Dockerfile.source-build` | `output/single/Dockerfile.source-build` | 이 zip 재현 예시: docker build -f output/single/Dockerfile.source-build · 컨텍스트 output/single(현재 토폴로지 통로; 실린 바이트 이력 출처 output/multi/Dockerfile.source-build). 실린 compose 파일만 단독 사용하고 override가 없는 구성은 output/single/의 기본 통로와 대조한다. 실제 원본·후속층 실행 경로는 별도 실행 기록에서 확인한다. |
| `artifacts/build_recipe/requirements.txt` | `output/single/requirements.txt` | Dockerfile.source-build COPY requirements.txt |
| `artifacts/compose/.env.template` | `.env` | compose 변수치환 프로젝트 .env(${…} 참조) |
| `artifacts/compose/docker-compose.yaml` | `docker-compose.yaml` | compose 파일(빌드 컨텍스트 루트) |
| (실린 파일 없음 — 빈 디렉터리 생성) | `output/single/build_patches_src/` | Dockerfile.source-build COPY build_patches_src/ · 실린 패치 없음 — 빈 디렉터리를 만든다(없으면 COPY 단계에서 빌드가 멈춘다) · 빌드 원장 context_inventory.build_patch_pre = ['.gitkeep'] |
| (실린 파일 없음 — 빈 디렉터리 생성) | `output/single/build_patches/` | Dockerfile.source-build COPY build_patches/ · 실린 패치 없음 — 빈 디렉터리를 만든다(없으면 COPY 단계에서 빌드가 멈춘다) · 빌드 원장 context_inventory.build_patch_post = ['.gitkeep'] |

**기동 성공 판정(관측 · 발행 자격)**: health 200 = true · 추론 1회 = true · 근거 `output/single/benchlog/sweep_gpt120v0320_20261028T2143/level_01/post_health_gptoss120b-0191-c0.json` · `output/single/benchlog/sweep_gpt120v0320_20261028T2143/level_01/guidellm_gptoss120b-0191-c0.json`
<!-- /FACT:reproduce -->

<!-- FACT:event_timeline -->
**블랙박스 이벤트 8행**(원장에서 이 셀 label·config 에 맞는 행과 그와 시간이 겹치는 같은 노드의 다른 라벨 창 · 시각순 · 기계 발췌 · 라벨 다름(같은 노드 · 창 밖 이 셀 행과 시간이 겹치는 다른 라벨 선언 창) 6행 — 셀 라벨 일치 집계에서 제외(세션·호출 기록의 귀속 확인과 별도))

> **캠페인 경계** 2026-10-26T21:46:47Z(`camp-26102705-codex-single8` · evidence.event_campaign_boundary): campaigns/camp-26102705-codex-single8/campaign.yaml declared_utc 2026-10-26T21:46:47Z → 경계 = 앞선 값 2026-10-26T21:46:47Z(이전 행 = 같은 셀 이름 · 앞선 캠페인)

> **원장 관측 범위**(노드별 첫 행 → 마지막 행 · evidence.event_ledger_spans): sub = 2026-06-13T22:25:01Z → 2026-10-28T22:12:04Z(`docs/logs/sub/events/2026-06.jsonl` · `docs/logs/sub/events/2026-07.jsonl` · `docs/logs/sub/events/2026-09.jsonl` · `docs/logs/sub/events/2026-10.jsonl` · `docs/logs/sub/events/thermal.jsonl` · `docs/logs/sub/events/watchdog.jsonl`)

| # | UTC | 노드 | kind | label | 내용 | 출처 |
|---|---|---|---|---|---|---|
| 1 | 2026-10-28T21:43:22Z | sub | `budget_declare` | `gpt120-v0320-deps1` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=37137 · weights_mib=62226 · kv_mib=445 · overhead_mib=24800 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 라벨 다름: `gpt120-v0320-deps1` — 같은 노드 · 이 셀 라벨(`gptoss120b-0191-c0`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) · budget_renew 1회 · 닫힘 budget_clear 2026-10-28T22:12:03Z(선언 후 28m41s) · measured_utc 2026-10-28T22:11:36Z 가 이 창 시간 안 — 라벨 불일치만으로 이 측정·기동의 귀속 여부를 판단하지 않는다 | docs/logs/sub/events/2026-10.jsonl:698 |
| 2 | 2026-10-28T21:43:23Z | sub | `serve_start` | — | serve_start · model=gpt-oss-120b · tensor_parallel_size=1 · topology=single · session=gpt120-v0320-20261028T2143 | docs/logs/sub/events/2026-10.jsonl:699 |
| 3 | 2026-10-28T21:43:23Z | sub | `budget_honored` | `gpt120-v0320-deps1` | budget_honored · floor_mib=37137 · arm_ceiling_mib=34065 · remaining_s=7199 · 창 시작 2026-10-28T21:43:23Z 후 0m00s · 라벨 다름: `gpt120-v0320-deps1` — 같은 노드 · 이 셀 라벨(`gptoss120b-0191-c0`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) | docs/logs/sub/events/watchdog.jsonl:653 |
| 4 | 2026-10-28T21:43:24Z | sub | `budget_renew` | `gpt120-v0320-deps1` | budget_renew · floor_mib=37137 · ttl_s=7200 · remaining_before_s=7198 · 창 시작 2026-10-28T21:43:22Z 후 0m02s · 라벨 다름: `gpt120-v0320-deps1` — 같은 노드 · 이 셀 라벨(`gptoss120b-0191-c0`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) | docs/logs/sub/events/2026-10.jsonl:700 |
| 5 | 2026-10-28T21:43:49Z | sub | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=96556 · rate_mib_s=19308 · max_rate_mib_s=17801 · legacy_rule_would_trip=true · 라벨 없음 → gpt120-v0320-deps1 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-28T21:43:23Z 후 0m26s) · 라벨 다름: `gpt120-v0320-deps1` — 같은 노드 · 이 셀 라벨(`gptoss120b-0191-c0`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) | docs/logs/sub/events/watchdog.jsonl:654 |
| 6 | 2026-10-28T22:12:03Z | sub | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → gpt120-v0320-deps1 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-28T21:43:22Z 후 28m41s) · 라벨 다름: `gpt120-v0320-deps1` — 같은 노드 · 이 셀 라벨(`gptoss120b-0191-c0`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) | docs/logs/sub/events/2026-10.jsonl:701 |
| 7 | 2026-10-28T22:12:03Z | sub | `serve_stop` | — | serve_stop · model=gpt-oss-120b · duration_s=1720 · session=gpt120-v0320-20261028T2143 | docs/logs/sub/events/2026-10.jsonl:702 |
| 8 | 2026-10-28T22:12:04Z | sub | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → gpt120-v0320-deps1 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-28T21:43:23Z 후 28m41s) · 라벨 다름: `gpt120-v0320-deps1` — 같은 노드 · 이 셀 라벨(`gptoss120b-0191-c0`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도) | docs/logs/sub/events/watchdog.jsonl:655 |

**기동 시도**(예산 선언 `budget_declare` 마다 한 시도 · 같은 노드의 이어지는 행과 선언 없는 노드(엔진 로그)의 그 시각 행을 묶었다 · 사살 · 트립 · 거부는 닫힘과 따로 센다 · `measurement` 행은 측정 기록이라 시도로 세지 않는다(위 표에만) — 해석은 저작자 몫)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 1 | sub | `gpt120-v0320-deps1`(라벨 다름 — 같은 노드 · 이 셀 라벨 아님) | 2026-10-28T21:43:22Z | 1회 · 첫 2026-10-28T21:43:24Z | 없음 | `budget_clear` 2026-10-28T22:12:03Z | 5 |
<!-- /FACT:event_timeline -->

<!-- FACT:measurement_env -->
_측정 실행의 env 관측 없음 — 측정 엔진 로그에서 env echo 줄(`<KEY> set by environment to <VALUE>` 등)을 찾지 못했다(evidence.measurement_env_observed · 부재 ≠ 미설정)._
<!-- /FACT:measurement_env -->

> 이 절이 답하는 질문: 수신자가 이 zip 만으로 이 셀을 다시 띄우려면 어떤 순서로 무엇을 하고, 각 단계의 성공을 무엇으로 판정하며, 얼마나 기다려야 하는가?

원본 source 이미지의 build phase 37분 55초와 나중에 별도 Dockerfile로 만든 deps1 계층은 서로 다른 작업이다. 실제 원본 빌드 컨텍스트는 output/single/cache/build-gpt120-v0320-attempt6이다. 위의 미검증 Dockerfile 배치 예시는 실제 빌드 경로가 아니다. 원시와 위 단계 표를 함께 본다. 로드 후 health와 completion을 확인하고 lite와 Full을 별도로 판정한다. 405초는 이번 서비스의 health 대기 관측이며 다른 기동의 보장은 아니다. 공식 down으로 이 서비스와 예산을 회수했다.

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

### 준비
사람이 승인된 모델 가중치와 tokenizer를 준비하고 manifest의 경로·GPU·예산을 확인한다. 로드 전 RAM 검사와 watchdog 선언은 실행 환경의 공식 도구로 수행한다. 실린 빌드 레시피는 원본 바이트가 검증되지 않았으므로 재현에 앞서 실제 빌드 입력을 확보해야 한다.
