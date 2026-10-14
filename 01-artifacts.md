# 01 · 산출물 — 무엇이 실제로 쓰였고, 어떻게 다시 띄우는가

## 1.1 적용 판정 표
> 기계가 적용 증거(빌드 원장 · 원장이 없으면 라벨된 재구성) × 파일 × 선언을 대사한 결과다. `artifacts/` 에는 **적용된 것만** 실렸다 — 빌드 때 스스로 skip 된 패치와 쓰이지 않은 평면의 파일은 표에만 남고 싣지 않는다.

<!-- FACT:slots -->
| 슬롯 | 실림 | 파일 | 신호 | 적용 증거 | 사유 |
|---|---|---|---|---|---|
| `build_patch_post` | 안 실림 | — | 1-signal | build-ledger:docker-image(main-local):easy-vllm:0.31.0-cu133-x86_64-source:/opt/easy-vllm/build_ledger.json | 적용 판정 없음 — 원장 관측(기계 파생) |
| `build_patch_pre` | 안 실림 | — | 1-signal | build-ledger:docker-image(main-local):easy-vllm:0.31.0-cu133-x86_64-source:/opt/easy-vllm/build_ledger.json | 적용 판정 없음 — 원장 관측(기계 파생) |
| `build_recipe` | 실림 | `artifacts/build_recipe/Dockerfile.source-build` · `artifacts/build_recipe/requirements.txt` | 3-signal | build-ledger:build-ledger.dockerfile | 이 캠페인 전 모델 공통의 소스빌드 레시피다 — 32B 고유 추가는 없다. v0.31.0의 `tools/use_existing_torch.py` 이동을 버전적응 스탠자가 감지한다(4B 셀 W1). 62GB 모델도 이 이미지 하나로 기동된다. |
| `compose` | 실림 | `artifacts/compose/.env.template` · `artifacts/compose/docker-compose.yaml` | 2-signal | file:output/single/docker-compose.yaml | NAS 모델 루트 `/app/models` ro 마운트 + JIT 캐시 영속 마운트. 이 셀에서는 캐시 마운트가 결정적이었다 — fp8 첫 부팅은 torch.compile 구간 SoC 105°C 킬로 실패했으나, 영속 캐시가 쌓인 재시도는 컴파일 구간을 건��뛰어 부팅·측정 완주(testlog_26101319 §백필 fp8 행). |
| `fork_pin` | 안 실림 | — | 1-signal | none:.claude/policies/arch_variant_ledger.json | 셀 env 에 VARIANT 줄 없음 = stock(workflow.md §변종 좌표의 거처)(기계 파생) |
| `runtime_patch` | 안 실림 | — | 1-signal | none:output/single/configs/qwen3-32b_patch.py | output/single/configs/qwen3-32b_patch.py 없음 — arm_patch.sh 는 no-op(런타임 패치 불해당)(기계 파생) |
| `triplet` | 실림 | `artifacts/triplet/.env.qwen3-32b-kv-bf16` · `artifacts/triplet/qwen3-32b-kv-bf16.sh` · `artifacts/triplet/qwen3-32b-kv-bf16.yaml` | 2-signal | file:output/single/configs/qwen3-32b-kv-bf16.yaml | 32B의 서빙 입력: 모델 경로 · max-model-len 40960 · max-num-seqs 512 · KV 절대 클램프 22,999,549,870 · prefix cache OFF · serve_env(MAX_JOBS=1). KV 클램프는 bf16 무클��프 부팅 실측(21.42 GiB · 87,744 tokens)으로 고정(V4) — 이 셀은 가중치가 VRAM의 대부분이라 이 값이 작다. |

**레시피 경고 · 실린 리비전**(기계 관측 — 수신자 빌드가 여기서 갈라질 수 있다)

- `build_recipe` `output/single/Dockerfile.source-build` 실린 리비전: 리비전 미특정 · 선택 방법 `worktree(측정 이미지 docker history 의 RUN·COPY 와 전수 일치)` · docker history 대조 17/17 · 실린 바이트 `worktree` · 쓰인 바이트임을 관측으로 확인 예
- `build_recipe` `output/single/requirements.txt` 실린 리비전: 리비전 미특정 · 선택 방법 `worktree(빌드 원장 sha256 일치 — 쓰인 바이트 관측 확인 = 예)` · docker history 대조 없음 · 실린 바이트 `worktree` · 쓰인 바이트임을 관측으로 확인 예
- `compose` `output/single/docker-compose.yaml` 실린 리비전: 커밋 `ca7bfb99a397` · 선택 방법 `worktree(mtime 2026-10-12T10:49:21Z ≤ 측정 2026-10-14T06:32:45Z(certificate.measured_utc) · 바이트 = 측정 전 마지막 커밋 ca7bfb99a397)` · docker history 대조 없음 · 실린 바이트 `worktree` · 근거 `mtime≤measured` · 쓰인 바이트임을 관측으로 확인 예

**싣지 않은 파일**(슬롯별 사유 — 빌드 때 skip 된 패치는 아래 적용 집합 표에 있다)

- `build_patch_post` known-non-patch 1건: `.gitkeep`
- `build_patch_pre` known-non-patch 1건: `.gitkeep`
- `compose` `arm_patch.sh` — `not-armed(output/single/configs/qwen3-32b_patch.py 없음 — arm_patch.sh 는 no-op)`

**파일별 검증 · 관련성**(`generated-unverified` = 실행 검증되지 않은 생성물 — 파일 첫 줄 경고 · PAYLOAD 파일 기록과 같은 표시 · 관련성 = ① 패치 선언 대상 × 모델 config ② 엔진 로그 발화 서명 — 둘 다 관측일 때만 required/inactive-inferred)

| 슬롯 | 파일 | 검증 | 검증 근거 | 관련성 | 관련성 근거 |
|---|---|---|---|---|---|
| `build_recipe` | `artifacts/build_recipe/Dockerfile.source-build` | `verified` · cell-run | 쓰인 레시피(build-ledger.dockerfile) — 개정 선택 slots.build_recipe.evidence.selected_revisions | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `build_recipe` | `artifacts/build_recipe/requirements.txt` | `verified` · cell-run | 쓰인 레시피(build-ledger.dockerfile) — 개정 선택 slots.build_recipe.evidence.selected_revisions | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/.env.template` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/docker-compose.yaml` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/.env.qwen3-32b-kv-bf16` | `verified` · cell-run | 셀 트리플렛 — 측정 대조 없음 | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/qwen3-32b-kv-bf16.sh` | `verified` · cell-run | 셀 트리플렛 — 측정 대조 없음 | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/qwen3-32b-kv-bf16.yaml` | `verified` · cell-run | 셀 트리플렛 — 측정 대조 없음 | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
<!-- /FACT:slots -->

<!-- FACT:applied_set -->
**적용 집합** — 상태 `observed` · 출처 `docker-image(main-local):easy-vllm:0.31.0-cu133-x86_64-source:/opt/easy-vllm/build_ledger.json`

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
| attestation | absent | output/single/benchlog/attestation_qwen3-32b.json(부재) |
| local-image | observed | easy-vllm:0.31.0-cu133-x86_64-source |
<!-- /FACT:applied_set -->

## 1.2 슬롯별 적용 사유
> 이 절이 답하는 질문: 실린 산출물 하나하나는 무엇을 고치며, 왜 이 셀에 필요했고, 이 셀에서 실제로 발화했으며, 언제 불필요해지는가?

### build_recipe (Dockerfile.source-build + requirements.txt)
이 캠페인 전 모델 공통의 소스빌드 레시피다 — 32B 고유 추가는 없다. v0.31.0의 `tools/use_existing_torch.py` 이동을 버전적응 스탠자가 감지한다(4B 셀 W1). 62GB 모델도 이 이미지 하나로 기동된다.

### compose (docker-compose.yaml + .env.template)
NAS 모델 루트 `/app/models` ro 마운트 + JIT 캐시 영속 마운트. 이 셀에서는 캐시 마운트가 결정적이었다 — fp8 첫 부팅은 torch.compile 구간 SoC 105°C 킬로 실패했으나, 영속 캐시가 쌓인 재시도는 컴파일 구간을 건��뛰어 부팅·측정 완주(testlog_26101319 §백필 fp8 행).

### triplet (.env + .sh + .yaml, qwen3-32b-kv-bf16)
32B의 서빙 입력: 모델 경로 · max-model-len 40960 · max-num-seqs 512 · KV 절대 클램프 22,999,549,870 · prefix cache OFF · serve_env(MAX_JOBS=1). KV 클램프는 bf16 무클��프 부팅 실측(21.42 GiB · 87,744 tokens)으로 고정(V4) — 이 셀은 가중치가 VRAM의 대부분이라 이 값이 작다.

### 실리지 않은 슬롯
- build_patch_pre/post: 자체 빌드 패치 0건(strip-hoist skipped — 원장).
- fork_pin: stock(vLLM 공식 태그).
- runtime_patch: configs/qwen3-32b_patch.py 없음 — arm_patch.sh no-op.

## 1.3 값의 지위표
> 서빙 설정의 노브 **전부**와 기계가 모은 후보 지위다(lockset 출처 · yaml 주석 · 엔진 기본값). 최종 지위는 아래 `kind: value-status` 블록이 정한다 — 표의 노브마다 블록이 정확히 하나 있어야 린터를 통과한다.

<!-- FACT:value_status -->
서빙 노브 5개 — 노브마다 `kind: value-status` 블록이 **정확히 1개** 있어야 한다(노브·값은 이 표의 글자 그대로 · 값 칸의 `\|` 는 `|` 로 적는다).

| 노브 | 값 | 후보 지위 | 후보 근거 | yaml 주석 | 출처 |
|---|---|---|---|---|---|
| `gpu-memory-utilization` | 0.9 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `max-model-len` | 40960 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `max-num-seqs` | 512 | inherited | lockset 출처 batch_source=hand-lever · lockset provenance=hand-authored | — | lockset.batch_source=hand-lever |
| `kv-cache-memory-bytes` | 22999549870 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `no-enable-prefix-caching` | true | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
<!-- /FACT:value_status -->

> 이 절이 답하는 질문: 각 서빙 노브의 값은 이 셀에서 어떤 지위인가 — 조정됐나, 승계됐나, 음성대조로 일부러 둔 것인가, 엔진 기본값인가, 필요조건인가?

```hint-event
id: V1
kind: value-status
노브: gpu-memory-utilization
값: 0.9
지위: inherited
근거: 캠페인 셀 config 선언(target_gpu.target_gmu) — 이 셀에서 값을 바꿔 잰 기록 없음
출처: [testlog_26101319_qwen3-32b_KV양자화_동시성_판정_1차 §머리 셀 lockset]
```
```hint-event
id: V2
kind: value-status
노브: max-model-len
값: 40960
지위: inherited
근거: 모델 config.json max_position_embeddings 선언을 그대로 옮긴 측정 통제변인(RoPE/YaRN 미적용) · 조정 기록 없음
출처: [plan_26101219 §2. 인터뷰 확정값]
```
```hint-event
id: V3
kind: value-status
노브: max-num-seqs
값: 512
지위: inherited
근거: 손레버(lockset batch_source=hand-lever) — 동시성 측정을 배치 상한이 막지 않게 · 스윕 미실시
출처: [testlog_26101319_qwen3-32b_KV양자화_동시성_판정_1차 §머리 셀 lockset]
```
```hint-event
id: V4
kind: value-status
노브: kv-cache-memory-bytes
값: 22999549870
지위: tuned
근거: 이 셀 계보에서 도출 — bf16 무클��프 부팅 엔진 실측 Available KV 21.42 GiB(87,744 tokens)로 고정(measured-clamp · arch-scaled)
출처: [testlog_26101319_qwen3-32b_KV양자화_동시성_판정_1차 §머리 KV 실측]
```
```hint-event
id: V5
kind: value-status
노브: no-enable-prefix-caching
값: true
지위: inherited
근거: 측정 통제변인(셀 lockset no_prefix_caching) — dtype 비교에서 prefix cache 효과 배제
출처: [testlog_26101319_qwen3-32b_KV양자화_동시성_판정_1차 §머리 셀 lockset]
```

**요약** — V4는 이 셀의 핵심 제약이다: 61GiB 가중치가 VRAM 대부분이라 KV가 21.42 GiB뿐이고, 이 값이 bf16 동시성 한 자리의 직접 원인이다. 다른 VRAM·다른 모델 크기에서는 반드시 재도출.

## 1.4 재현 절차
> 셀 실행 기록에서 기계가 파생한 명령 순서와 단계별 소요다. 소요가 `미관측` 인 단계는 시각 기록이 없다는 뜻이다(0 이 아니다). 그 아래는 블랙박스 이벤트 원장에서 이 셀의 행을 기계가 뽑아 기동 시도로 묶은 것이고, 마지막은 측정한 실행의 엔진 로그가 echo 한 env 다(측정 당시 값의 정본).

<!-- FACT:reproduce -->
| 순서 | 단계 | 성공 판정 | 소요 | 관측 구간(UTC) | 소요 출처 | 명령 출처 |
|---|---|---|---|---|---|---|
| 1 | build | 빌드 OK · 이미지 실재 | 층 창 1시간 39분 25초(docker history 첫 층 → 끝 층 CreatedAt · 캐시 재사용 층 포함 — 빌드 소요 아님) | 2026-10-12T10:55:01Z → 2026-10-12T12:34:26Z (layer-window) | docker history --no-trunc --format '{{json .}}' easy-vllm:0.31.0-cu133-x86_64-source · 우리 Dockerfile 층 30개의 CreatedAt 첫 층 → 끝 층(캐시 재사용 층은 이전 빌드 시각 — 빌드 소요 아님) | reconstructed(build-ledger build_args + compose build) |
| 2 | serve | health 200 + 추론 1회(스모크 판정) | ≤ 2분 12초(상한) | 2026-10-14T06:30:33Z → 2026-10-14T06:32:45Z (upper) | docs/logs/main/events/2026-10.jsonl budget_declare(label serve-qwen3-32b) → 첫 측정 시각(output/single/benchlog/lite_publish/qwen3-32b_20261014T063245Z/lite_raw_qwen3-32b.json measured_utc(= bench_report 머리 생성일 · lite_bench.sh 가 cold 레그 직전(부하 직전)에 찍는 측정 **시작** 시각)) | derived(스모크 사용법) |
| 3 | bench | 리포트 발행(+PASS 면 인증서) | ≈ 0분 44초(두 끝 시계 다름 — 컨테이너 date · 호스트 UTC · 근사) | 2026-10-14T06:32:45Z → 2026-10-14T06:33:29Z (clock-mixed) | 시작 = output/single/benchlog/lite_publish/qwen3-32b_20261014T063245Z/lite_raw_qwen3-32b.json measured_utc(= bench_report 머리 생성일 · lite_bench.sh 가 cold 레그 직전(부하 직전)에 찍는 측정 **시작** 시각) → 끝 = output/single/benchlog/lite_publish/qwen3-32b_20261014T063245Z/lite_warm_qwen3-32b.json date(마지막 레그 결과 저장 · 컨테이너 시계 = UTC 가정) | reconstructed(bench json) |

**빌드 층 시각**(관측 · 소요가 아니라 층 창 — 캐시 재사용 층은 이전 빌드의 시각을 가진다)

- `build` 이미지 층 CreatedAt(docker history): 2026-10-12T10:55:01Z → 2026-10-12T12:34:26Z · 층 30개 · 출처 `docker history --no-trunc --format '{{json .}}' easy-vllm:0.31.0-cu133-x86_64-source` — 우리 Dockerfile 층의 CreatedAt 범위 — 캐시 재사용 층은 이전 빌드 시각을 가진다(소요 아님)

**단계별 명령**(기계 파생 — `명령 출처` 가 `derived(…)` 면 사용법에서 파생 · `reconstructed(…)` 면 괄호 안 관측(docker history build-arg · compose build · bench JSON 필드)에서 재구성한 명령이다. 어느 쪽도 실행 원문(로그에 남은 줄)은 아니다)

**1. build**

```sh
# 실린 artifacts/build_recipe/Dockerfile.source-build(이미지를 지은 개정: worktree)를 output/single/Dockerfile.source-build 자리에 두고 컨텍스트에서 짓는다.
docker build -f output/single/Dockerfile.source-build \
  --build-arg BUILD_JOBS=8 \
  --build-arg RAY_VERSION=2.48.0 \
  --build-arg SM12X_PORT=0 \
  --build-arg SRC_DEPS_AUTHORITY=0 \
  --build-arg TORCH_CUDA_ARCH=12.0 \
  --build-arg VLLM_PRETEND_VERSION= \
  --build-arg VLLM_REF=v0.31.0 \
  --build-arg VLLM_REPO=https://github.com/vllm-project/vllm.git \
  -t easy-vllm:0.31.0-cu133-x86_64-source \
  output/single
# 동등 경로(compose): docker compose -f output/single/docker-compose.yaml --env-file output/single/envs/.env.qwen3-32b build
```

**2. serve**

```sh
bash .claude/skills/upstream-version-watch/scripts/single_serve_up.sh qwen3-32b
```

**3. bench**

```sh
# bench JSON 필드로 복원: backend · model_id→--model · tokenizer_id→--tokenizer · num_prompts · max_concurrency · request_rate · burstiness · 요청당 입출력 길이 = total_*_tokens ÷ completed(나누어떨어질 때만 — 평균이다 · 요청별 길이는 JSON 에 없다) · 나누어떨어지면 `--dataset-name random` 으로 추정했다(측정 도구가 그 꼴로 부른다 — 아래 도구 원문).
# JSON 에 없는 인자(--endpoint · --base-url · --ignore-eos · --num-warmups · --temperature · --seed · --random-range-ratio · --trust-remote-code)는 측정 도구가 정했다 — level_* = run_bench.sh · lite_cold/lite_warm = lite_bench.sh(.claude/skills/adversarial-benchmark/scripts/) · 셀 스윕 순서는 sweep_bench.sh.
# 호출(재구성 — lite_raw config_name · topology · backend + 바인딩된 경량 리포트 = --publish-report · 원문 호출 줄 아님): bash .claude/skills/adversarial-benchmark/scripts/lite_bench.sh qwen3-32b --topology single --backend openai-chat --publish-report
# 측정 창: 시작 2026-10-14T06:32:45Z(output/single/benchlog/lite_publish/qwen3-32b_20261014T063245Z/lite_raw_qwen3-32b.json measured_utc(= bench_report 머리 생성일 · lite_bench.sh 가 cold 레그 직전(부하 직전)에 찍는 측정 **시작** 시각)) → 끝 2026-10-14T06:33:29Z(output/single/benchlog/lite_publish/qwen3-32b_20261014T063245Z/lite_warm_qwen3-32b.json date(마지막 레그 결과 저장 · 컨테이너 시계 = UTC 가정))
# output/single/benchlog/lite_publish/qwen3-32b_20261014T063245Z/lite_cold_qwen3-32b.json (date 20261014-063300) · ⚠ --random-input-len/--random-output-len = total ÷ completed 평균(chat 템플릿 포함 실측 토큰 · 측정 도구 원문 인자 미관측 — lite_bench.sh 원문 인자와 다를 수 있다)
vllm bench serve --backend openai-chat --model Qwen3-32B --tokenizer /app/models/Qwen/Qwen3-32B --dataset-name random --random-input-len 520 --random-output-len 128 --num-prompts 1 --max-concurrency 1 --request-rate inf
# output/single/benchlog/lite_publish/qwen3-32b_20261014T063245Z/lite_warm_qwen3-32b.json (date 20261014-063329) · ⚠ --random-input-len/--random-output-len = total ÷ completed 평균(chat 템플릿 포함 실측 토큰 · 측정 도구 원문 인자 미관측 — lite_bench.sh 원문 인자와 다를 수 있다)
vllm bench serve --backend openai-chat --model Qwen3-32B --tokenizer /app/models/Qwen/Qwen3-32B --dataset-name random --random-input-len 520 --random-output-len 128 --num-prompts 3 --max-concurrency 1 --request-rate inf
```

**슬롯 → 빌드 컨텍스트 매핑**(빌드 컨텍스트 = `output/<토폴로지>/` · 원천 = 실린 Dockerfile 의 COPY · compose 의 env_file · volumes · command — `—` 는 빌드 컨텍스트 입력이 아니다)

| zip 경로 | 빌드 컨텍스트 자리 | 근거 |
|---|---|---|
| `artifacts/triplet/.env.qwen3-32b-kv-bf16` | `envs/.env.qwen3-32b-kv-bf16` | compose env_file envs/.env.${CONFIG_FILE}(셀 env) |
| `artifacts/triplet/qwen3-32b-kv-bf16.sh` | — | 빌드 컨텍스트 입력 아님(Dockerfile COPY · compose 참조 밖) |
| `artifacts/triplet/qwen3-32b-kv-bf16.yaml` | — | 빌드 컨텍스트 입력 아님(Dockerfile COPY · compose 참조 밖) |
| `artifacts/build_recipe/Dockerfile.source-build` | `Dockerfile.source-build` | compose build.dockerfile Dockerfile.source-build(리터럴) · build.context . |
| `artifacts/build_recipe/requirements.txt` | `requirements.txt` | Dockerfile.source-build COPY requirements.txt |
| `artifacts/compose/.env.template` | `.env` | compose 변수치환 프로젝트 .env(${…} 참조) |
| `artifacts/compose/docker-compose.yaml` | `docker-compose.yaml` | compose 파일(빌드 컨텍스트 루트) |

**기동 성공 판정(관측 · 발행 자격)**: health 200 = true · 추론 1회 = true · 근거 `output/single/benchlog/lite_publish/qwen3-32b_20261014T063245Z/lite_raw_qwen3-32b.json` · `output/single/benchlog/lite_publish/qwen3-32b_20261014T063245Z/lite_warm_qwen3-32b.json`
<!-- /FACT:reproduce -->

<!-- FACT:event_timeline -->
**블랙박스 이벤트 7행**(원장에서 이 셀 label·config 에 맞는 행만 · 시각순 · 기계 발췌)

> **캠페인 경계** 2026-10-12T10:49:53Z(`camp-26101210-kvq-concurrency` · evidence.event_campaign_boundary): campaigns/camp-26101210-kvq-concurrency/campaign.yaml declared_utc 2026-10-12T10:49:53Z · 이 발행이 묶인 측정을 서빙한 budget_declare 창 시작 2026-10-14T06:30:33Z → 경계 = 앞선 값 2026-10-12T10:49:53Z(이전 행 = 같은 셀 이름 · 앞선 캠페인)

> **원장 관측 범위**(노드별 첫 행 → 마지막 행 · evidence.event_ledger_spans): main = 2026-10-12T10:41:33Z → 2026-10-14T06:33:41Z(`docs/logs/main/events/2026-10.jsonl` · `docs/logs/main/events/thermal.jsonl` · `docs/logs/main/events/watchdog.jsonl`)

| # | UTC | 노드 | kind | label | 내용 | 출처 |
|---|---|---|---|---|---|---|
| 1 | 2026-10-14T06:30:33Z | main | `budget_declare` | `serve-qwen3-32b` | 예산 선언(선언값 — 측정 아님) · floor_mib=43765 · weights_mib=3773 · kv_mib=0 · overhead_mib=16384 · mem_total_mib=63922 · 기동 창 1/1: budget_renew 1회 · 닫힘 budget_clear 2026-10-14T06:33:41Z(선언 후 3m08s) · measured_utc 2026-10-14T06:32:45Z 가 이 창 안(이 측정의 기동) | docs/logs/main/events/2026-10.jsonl:744 |
| 2 | 2026-10-14T06:30:33Z | main | `serve_start` | — | serve_start · model=Qwen3-32B · tensor_parallel_size=1 · topology=single · session=serve-qwen3-32b-20261014T063033Z | docs/logs/main/events/2026-10.jsonl:745 |
| 3 | 2026-10-14T06:30:34Z | main | `budget_renew` | `serve-qwen3-32b` | budget_renew · floor_mib=43765 · ttl_s=7200 · remaining_before_s=7199 · 창 시작 2026-10-14T06:30:33Z 후 0m01s | docs/logs/main/events/2026-10.jsonl:746 |
| 4 | 2026-10-14T06:30:34Z | main | `budget_honored` | `serve-qwen3-32b` | budget_honored · floor_mib=43765 · arm_ceiling_mib=40693 · remaining_s=7199 · 창 시작 2026-10-14T06:30:34Z 후 0m00s | docs/logs/main/events/watchdog.jsonl:77 |
| 5 | 2026-10-14T06:33:41Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → serve-qwen3-32b 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-14T06:30:33Z 후 3m08s) | docs/logs/main/events/2026-10.jsonl:747 |
| 6 | 2026-10-14T06:33:41Z | main | `serve_stop` | — | serve_stop · model=Qwen3-32B · duration_s=188 · session=serve-qwen3-32b-20261014T063033Z | docs/logs/main/events/2026-10.jsonl:748 |
| 7 | 2026-10-14T06:33:41Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → serve-qwen3-32b 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-14T06:30:34Z 후 3m07s) | docs/logs/main/events/watchdog.jsonl:78 |

**기동 시도**(예산 선언 `budget_declare` 마다 한 시도 · 같은 노드의 이어지는 행과 선언 없는 노드(엔진 로그)의 그 시각 행을 묶었다 · 사살 · 트립 · 거부는 닫힘과 따로 센다 · `measurement` 행은 측정 기록이라 시도로 세지 않는다(위 표에만) — 해석은 저작자 몫)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 1 | main | `serve-qwen3-32b` | 2026-10-14T06:30:33Z | 1회 · 첫 2026-10-14T06:30:34Z | 없음 | `budget_clear` 2026-10-14T06:33:41Z | 4 |
<!-- /FACT:event_timeline -->

<!-- FACT:measurement_env -->
_측정 실행의 env 관측 없음 — 측정 엔진 로그에서 env echo 줄(`<KEY> set by environment to <VALUE>` 등)을 찾지 못했다(evidence.measurement_env_observed · 부재 ≠ 미설정)._
<!-- /FACT:measurement_env -->

> 이 절이 답하는 질문: 수신자가 이 zip 만으로 이 셀을 다시 띄우려면 어떤 순서로 무엇을 하고, 각 단계의 성공을 무엇으로 판정하며, 얼마나 기다려야 하는가?

(a) **사전 준비**: 가중치 Qwen/Qwen3-32B(62GB)를 `<manifest.nas_model_path>/Qwen/Qwen3-32B`에 둔다(§1.6). 호스트 RAM이 ckpt와 같은 62GB급이면 discrete RAM 게이트(최대 샤드+floord)로 판정되는지 기동 전 확인(W1).
(b) **빌드**: 4B 셀과 같은 이미지(easy-vllm:0.31.0-cu133-x86_64-source)를 그대로 쓴다 — 별도 빌드 없음.
(c) **기동**: `bash .claude/skills/upstream-version-watch/scripts/single_serve_up.sh qwen3-32b` → health 200 + 추론 1회. 62GB 로드에 수 분. fp8 첫 부팅은 torch.compile 구간 열 킬로 실패할 수 있고, 영속 JIT 캐시가 쌓인 재시도는 통과한다(compose §캐시 마운트).
(d) **호스트 메모리 예산 선언**: discrete 회계 — 최대 샤드(3.7GiB)+floor가 호스트 상주 상한이고 KV(21.42 GiB)는 VRAM이라 호스트 예산 0. unified 가정이면 72.7GiB를 요구해 거부된다(W1).
(e) **측정**: 이 태그 바인딩은 lite(`lite_bench.sh qwen3-32b --topology single --publish-report`). 캠페인 N* 측정은 프로브 드라이버로, 30K 셀은 KV 산술 상한 확인 + 저N 확인 런 병기로 확정했다.
(f) **런타임 경로**: bf16은 FLASH_ATTN 자동 선택(엔진 로그 `Using FLASH_ATTN attention backend` 행 관측). TP=1 단순 구성.

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

### (a) 가중치 획득
Qwen/Qwen3-32B(62GB · BF16 · revision 미관측)를 사람 승인 게이트 뒤 HF에서 받아 `<manifest.nas_model_path>/Qwen/Qwen3-32B`에 둔다. 이 캠페인은 측정 후 삭제했다가 hint 발행을 위해 재다운로드했다(62GB · sha256 검증 — devlog_26101415 §6 · 다운로더 정체는 미기록). 일반 명령: `huggingface-cli download Qwen/Qwen3-32B --local-dir <manifest.nas_model_path>/Qwen/Qwen3-32B`(추론 — 이 캠페인 도구는 hf_fetch.py).

### (b) PLE · 스테이징 생성
해당 없음 — PLE 키 부재 관측. 스테이징 별도 자산 없음.

### (c) 네트워크가 필요한 단계
- HF 가중치 다운로드(승인 게이트 · 62GB — 디스크 여유를 먼저 확인)
- 측정 도구 이미지(guidellm) pull — 토크나이저는 모델 디렉터리에서 읽는다(HF_HUB_OFFLINE=1)
