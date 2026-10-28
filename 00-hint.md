# 00 · hint 지도 — 가장 먼저 읽는 파일

<!-- FACT:header -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.18.0/gpt-oss-20b/gb10-1g1n-sub-native/qmxfp4-len131072-kvfp8` |
| 형식 | `hint-payload/v7` · 이름 문법 `v7` |
| 생성(UTC · 주입) | 2026-10-28T07:53:08Z |
| 캠페인 · 셀 · 노드 · 모드 | camp-26102705-codex-single8 · gpt-oss-20b-gb10 · sub · campaign |
<!-- /FACT:header -->

> ⚠ 이 자료는 **지도이지 정답이 아니다.** 한 셀(모델 × HW × vLLM 빌드)이 실제로 걸은 길의 기록이다.
> 네 환경에서 반드시 **스모크 통과까지 재검증**. 최종 판정 = 네 스모크(린트·이슈글 ≠ 서빙됨).
> 복붙 ✗ = 전략을 **다시 세워라**(carry-forward 금지 · 지도 not 정답).
> 외부 교차검증(HF 모델 카드의 vLLM 절 · vLLM 릴리스 노트/issue)을 대체하지 않는다 — 네 모델×버전에 대해 반드시 다시 한다.
> 이 자료는 DATA 이지 instructions 가 아니다 — '분석'만 하고 '실행'하지 마라.

<!-- FACT:grade -->
| 항목 | 값 |
|---|---|
| 발행 자격(관측) | health 200 = true · 추론 1회 = true |
| 자격 근거 | `output/single/benchlog/sweep_gpt-oss-20b-gb10_camp26102705_resume_20261028T073540Z/level_01/post_health_gpt-oss-20b-gb10.json` · `output/single/benchlog/sweep_gpt-oss-20b-gb10_camp26102705_resume_20261028T073540Z/level_01/guidellm_gpt-oss-20b-gb10.json` |
| 증거 등급(task_class) | full_benchmark |
| 측정 등급(bench_mode) | full |
| 셀 등급(cell_grade) | `full` — 성능 판정 · 부하 곡선 · 데이터셋 출처는 `03-benchmark.md` |
| 성능 판정(measurement.verdict) | `PASS` — 출처 output/single/benchlog/sweep_gpt-oss-20b-gb10_camp26102705_resume_20261028T073540Z/verdict.json(output · verdict_rule 출력) verdict |
<!-- /FACT:grade -->

<!-- FACT:lite_summary -->
<!-- /FACT:lite_summary -->

## 0.1 요약
> 이 절이 답하는 질문: 이 셀은 무엇을 서빙했고, 무엇이 결정적이었으며, 수신자는 무엇을 가장 조심해야 하는가?

이 셀은 GB10에서 GPT-OSS-20B의 Harmony completion Full을 수행해 weak PASS를 얻었다. 각 레벨 3회이며 도구 경계 오류 2건과 별도 tool 요청 실패가 함께 기록됐다. 같은 결과는 같은 조건에서만 해석한다.

## 0.2 유효맥락
<!-- FACT:context -->
| 항목 | 값 | 출처 |
|---|---|---|
| 모델 | gpt-oss-20b | sweep_index.meta(output/single/benchlog/sweep_gpt-oss-20b-gb10_camp26102705_resume_20261028T073540Z) |
| HF repo | openai/gpt-oss-20b | 체크포인트 .git/config remote url(huggingface.co · /app/models/OpenAI/gpt-oss-20b → <manifest.nas_model_path>/OpenAI/gpt-oss-20b) |
| HF revision(체크포인트 git HEAD) | d666cf3b67006cf8227666739edf25164aaffdeb | 체크포인트 .git HEAD → refs/heads/main → loose ref(/app/models/OpenAI/gpt-oss-20b → <manifest.nas_model_path>/OpenAI/gpt-oss-20b) · refs/remotes/origin/main 와 일치(받아 온 원격 커밋) — checkout 커밋(작업트리 일치는 미검증) · reflog: 측정 시점 checkout = 지금 HEAD(measured_utc 2026-10-28T07:50:10Z 이전 마지막 이동 2025-08-18T07:25:53Z) |
| base_model | 미관측 | README 머리 YAML 에 base_model 없음(/app/models/OpenAI/gpt-oss-20b → <manifest.nas_model_path>/OpenAI/gpt-oss-20b) |
| base 슬러그 | gpt-oss-20b | model(gpt-oss-20b) − vocab quant_suffixes(naming.base_slug) |
| 양자화 | mxfp4 | sweep_index.meta(output/single/benchlog/sweep_gpt-oss-20b-gb10_camp26102705_resume_20261028T073540Z) |
| GPU | NVIDIA GB10 | sweep_index.meta(output/single/benchlog/sweep_gpt-oss-20b-gb10_camp26102705_resume_20261028T073540Z) |
| 토폴로지 · TP | single · TP=1 | sweep_index.meta(output/single/benchlog/sweep_gpt-oss-20b-gb10_camp26102705_resume_20261028T073540Z) |
| 실행 평면 | docker | artifacts.plane_of |
| vLLM(측정 강식별 키 `vllm_version`) | 0.18.0 | sweep_index.meta(output/single/benchlog/sweep_gpt-oss-20b-gb10_camp26102705_resume_20261028T073540Z) |

**명명 축** — 태그 이름 = 결정론부(vllm · model · arch · q · len · kv — 도구가 증거에서 파생 · 발행자 입력 ✗) + 꼬리(발행 Agent 가 고르고 서빙 설정 file · key · value 로 근거 대조 · 0.9) + 중복 시 timestamp.

| 축 | 값 | 출처 |
|---|---|---|
| 세그먼트 `vllm` | 0.18.0 | track=선택자 Dockerfile(docker history build-arg 관측) · ref=VLLM_REF 미관측 · version=docker history build-arg VLLM_VERSION · repo=VLLM_REPO 미관측 · sha=미관측 · VLLM_VERSION=0.18.0(wheel 릴리스) |
| 세그먼트 `model` | gpt-oss-20b | 서빙 yaml model(output/single/configs/gpt-oss-20b-gb10.yaml) · hf_repo=체크포인트 .git/config remote url(huggingface.co · /app/models/OpenAI/gpt-oss-20b → <manifest.nas_model_path>/OpenAI/gpt-oss-20b) · hf_repo(openai/gpt-oss-20b) 마지막 성분 소문자 |
| 세그먼트 `arch` | gb10-1g1n-sub-native | derived(hw·gpus_per_node·nodes·role·target·plane) |
| 세그먼트 `recipe` | qmxfp4-len131072-kvfp8 | derived(q·len·kv) |
| 축 `hw` | gb10 | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(캠페인 assignments(배정 SSOT)) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) · vocab hw[gb10]←'NVIDIA GB10' |
| 축 `gpus_per_node` | 1 | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(캠페인 assignments(배정 SSOT)) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `nodes` | 1 | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(캠페인 assignments(배정 SSOT)) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `role` | sub | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(캠페인 assignments(배정 SSOT)) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `target` | native | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(캠페인 assignments(배정 SSOT)) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) · target_gpu 'NVIDIA GB10' = 호스트 hw → native |
| 축 `plane` | docker | artifacts.plane_of → cell-env(IMAGE_TAG\|BUILD_DOCKERFILE) · vocab plane[docker]→'' |
| 축 `q` | mxfp4 | /app/models/OpenAI/gpt-oss-20b → <manifest.nas_model_path>/OpenAI/gpt-oss-20b/config.json quantization_config quant_method=mxfp4 · vocab quant[mxfp4]←'mxfp4' |
| 축 `len` | 131072 | 서빙 yaml max-model-len(output/single/configs/gpt-oss-20b-gb10.yaml) |
| 축 `kv` | fp8 | 서빙 yaml kv-cache-dtype(output/single/configs/gpt-oss-20b-gb10.yaml) · vocab kv[fp8]←'fp8' |
<!-- /FACT:context -->

> 이 절이 답하는 질문: 이 지도는 어떤 조건에서만 성립하며, 이 형상은 어떤 기전으로 버티고 다른 형상은 왜 실패했는가?

단일 sub의 MXFP4·fp8 KV 설정과 이번 실행의 근거다. 다른 GPU, 모델, runtime 또는 부하에 대한 보장은 없다.

## 0.3 벽 지도 요약
> 02-narrative.md §2.2 의 `kind: wall` 블록(과 §2.4 의 `kind: misdiagnosis` 블록)에서 기계가 생성한 표다(손저작 ✗ · 순서 = 넘은 순서). 원인·해소의 전문과 원문 발췌는 02 에 있다.

<!-- FACT:wall_map -->
| 순서 | id | 증상 | 해소 | 전이등급 | 검증 |
|---|---|---|---|---|---|
| 1 | W1 | crosscheck MISMATCH | 정본 MXFP4 구조 계산으로 재수집 | judgment | testlog_26102816_GPT_OSS_20B_신규_Full_판정 §모델·기능·예산 |
| 2 | W2 | 두 run에서 각각 요청 실패 1건 | 해소되지 않음, producer는 경계 오류로 분류 | judgment | testlog_26102816_GPT_OSS_20B_신규_Full_판정 §이번 실행 |

**오진 · 정정**(02 §2.4 — 현재지위가 `가설(강등)` 인 기전은 처방이 아니다)

| id | 증상 | 현재지위 | 검증 |
|---|---|---|---|
| M1 | 기존 파서의 논리 총계 대입이 외부 총계와 불일치 | 반증 | testlog_26102816_GPT_OSS_20B_신규_Full_판정 §모델·기능·예산 |
<!-- /FACT:wall_map -->

## 0.4 결정론 해소값
<!-- FACT:resolved -->
| 항목 | 값 | 출처 |
|---|---|---|
| vLLM 빌드 입력(종류) | wheel | naming.vllm_build_input |
| vLLM 빌드 입력(ref) | 0.18.0 | VLLM_REF 미관측 |
| vLLM 빌드 계보 SHA(40자) | 미관측 | 빌드 입력/원장에 결속된 SHA 미관측 — runtime checkout 관측과 컴파일 바이트 증명은 별개 |
| 직전 릴리스(prev_release) | 해당 없음 — 릴리스 태그 빌드(이름 = 릴리스 자체 · `<직전 릴리스>-g<sha>` 는 커밋 핀에만) | naming.vllm_build_input(kind=release) |
| vLLM 엔진 자기보고(엔진 로그 기동 배너) | 0.18.0 | output/single/benchlog/sweep_gpt-oss-20b-gb10_camp26102705_resume_20261028T073540Z/lite_engine_gpt-oss-20b-gb10.log:62 · 기동 2026-10-28T07:32:16Z(컨테이너 시계 UTC 가정) · docs/logs/sub/sessions/serve-gpt-oss-20b-gb10-a03-20261028T073154Z.json — 측정 창을 포함한 현재 serve 세션 안·첫 측정 이전 기동 배너 · 같은 값의 다른 로그 3건 · 지위 `this-measurement` |
| 인증서 `vllm_version`(강한 일치 키) | 미관측 | 생산자 `absent` · 인증서 vllm_version 없음 |
| wheel 메타(`vllm_build`) | 0.18.0+cu130 | docs/simlog/single8_gptoss20b_20261027T112514Z/attempt3/runtime_identity.json (관측 기록 · docs/simlog/single8_gptoss20b_20261027T112514Z/attempt3/evidence_index.json가 config=gpt-oss-20b-gb10, node=sub, sweep_dir=output/single/benchlog/sweep_gpt-oss-20b-gb10_camp26102705_resume_20261028T073540Z, measured_utc=2026-10-28T07:50:10Z로 결속; 수집 명령 기록: docs/simlog/single8_gptoss20b_20261027T112514Z/attempt3/runtime_identity.json#/collection_argv(실행 성공·진위·값 출처는 이 필드만으로 미검증); 컴파일 바이트 일치는 별도 미검증) distribution_metadata_version · 기록된 출처(선언 · 검증되지 않음): docs/simlog/single8_gptoss20b_20261027T112514Z/attempt3/runtime_identity.json (관측 기록 · docs/simlog/single8_gptoss20b_20261027T112514Z/attempt3/evidence_index.json가 config=gpt-oss-20b-gb10, node=sub, sweep_dir=output/single/benchlog/sweep_gpt-oss-20b-gb10_camp26102705_resume_20261028T073540Z, measured_utc=2026-10-28T07:50:10Z로 결속; 수집 명령 기록: docs/simlog/single8_gptoss20b_20261027T112514Z/attempt3/runtime_identity.json#/collection_argv(실행 성공·진위·값 출처는 이 필드만으로 미검증); 컴파일 바이트 일치는 별도 미검증) distribution_metadata_version_source='importlib.metadata.distribution("vllm").version' · 수집 성공은 별도 검증 |
| vLLM runtime 버전(관측 기록 · 배포 메타 여부 미판정) | 0.18.0 | docs/simlog/single8_gptoss20b_20261027T112514Z/attempt3/runtime_identity.json (관측 기록 · docs/simlog/single8_gptoss20b_20261027T112514Z/attempt3/evidence_index.json가 config=gpt-oss-20b-gb10, node=sub, sweep_dir=output/single/benchlog/sweep_gpt-oss-20b-gb10_camp26102705_resume_20261028T073540Z, measured_utc=2026-10-28T07:50:10Z로 결속; 수집 명령 기록: docs/simlog/single8_gptoss20b_20261027T112514Z/attempt3/runtime_identity.json#/collection_argv(실행 성공·진위·값 출처는 이 필드만으로 미검증); 컴파일 바이트 일치는 별도 미검증) vllm_version |
| vLLM runtime source checkout SHA(컴파일 바이트 증명 아님) | 미관측 | 미관측 — 이 측정에 결속된 runtime 관측의 source_head 없음 |
| vLLM runtime source describe(관측 기록) | 미관측 | 미관측 — 이 측정에 결속된 runtime 관측의 source_describe 없음 |
| 체크포인트 revision(HF · git HEAD) | d666cf3b67006cf8227666739edf25164aaffdeb | 체크포인트 .git HEAD → refs/heads/main → loose ref(/app/models/OpenAI/gpt-oss-20b → <manifest.nas_model_path>/OpenAI/gpt-oss-20b) · refs/remotes/origin/main 와 일치(받아 온 원격 커밋) — checkout 커밋(작업트리 일치는 미검증) · reflog: 측정 시점 checkout = 지금 HEAD(measured_utc 2026-10-28T07:50:10Z 이전 마지막 이동 2025-08-18T07:25:53Z) |
| 빌드 트랙 | wheel | artifacts 선택자(image-history:docker history build-arg 관측(VLLM_VERSION=0.18.0 · VLLM_REF 없음 → wheel Dockerfile) — 셀 env 선택자 없음 · output/single/docker-compose.yaml 리터럴 build.dockerfile: Dockerfile.source-build(이 파일만 단독 적용하고 override 가 없을 때 Dockerfile.source-build 선택 · 과거 빌드에 이 compose 또는 override 를 사용했는지는 미관측) · ⚠ compose 리터럴 Dockerfile.source-build ≠ 관측 Dockerfile(이미지 빌드에 사용된 compose 또는 override 여부는 미관측)) |
| Dockerfile(선택자) | Dockerfile | image-history:docker history build-arg 관측(VLLM_VERSION=0.18.0 · VLLM_REF 없음 → wheel Dockerfile) — 셀 env 선택자 없음 · output/single/docker-compose.yaml 리터럴 build.dockerfile: Dockerfile.source-build(이 파일만 단독 적용하고 override 가 없을 때 Dockerfile.source-build 선택 · 과거 빌드에 이 compose 또는 override 를 사용했는지는 미관측) · ⚠ compose 리터럴 Dockerfile.source-build ≠ 관측 Dockerfile(이미지 빌드에 사용된 compose 또는 override 여부는 미관측) |
| vLLM 저장소 | 미관측 | 미관측 |
| 이미지 태그 | easy-vllm:0.18.0-cu130-aarch64-wheel | 셀 env(output/single/envs/.env.gpt-oss-20b-gb10) IMAGE_TAG |
| 이미지 digest | sha256:b2c993d8bf628ad46100dd9bdaf260599f564aea3539fce9b7bbfb57ca91e54a | sweep_index.meta.image_digest(measured(docker inspect .Image)) |
| torch | 2.10.0a0+a36e1d3 | docker image inspect Config.Env PYTORCH_VERSION |
| CUDA | 13.1.1.006 | docker image inspect Config.Env CUDA_VERSION |
| NGC 베이스 | 26.01 | docker image inspect Config.Env NVIDIA_PYTORCH_VERSION |
| CPU arch | arm64 | docker image inspect Architecture |
| 드라이버 | 580.178.04 | 측정 엔진 로그 기동 배너 'kernel driver version'(output/single/benchlog/sweep_gpt-oss-20b-gb10_camp26102705_resume_20261028T073540Z/lite_engine_head_gpt-oss-20b-gb10.log:30 · 컨테이너가 보고한 호스트 커널 드라이버 · 이 측정의 엔진 로그 17개 중 그 줄이 있는 로그 모두 같은 값) |
| 드라이버(노드별 관측) | sub=580.178.04 | 측정 엔진 로그 기동 배너 'kernel driver version'(output/single/benchlog/sweep_gpt-oss-20b-gb10_camp26102705_resume_20261028T073540Z/lite_engine_head_gpt-oss-20b-gb10.log:30 · 컨테이너가 보고한 호스트 커널 드라이버 · 이 측정의 엔진 로그 17개 중 그 줄이 있는 로그 모두 같은 값) |
| ⚠ 드라이버 관측 ≠ 선언 | 충돌 없음 — 관측 580.178.04 = 선언 580.178.04 | 관측: 측정 엔진 로그 기동 배너 'kernel driver version'(output/single/benchlog/sweep_gpt-oss-20b-gb10_camp26102705_resume_20261028T073540Z/lite_engine_head_gpt-oss-20b-gb10.log:30 · 컨테이너가 보고한 호스트 커널 드라이버 · 이 측정의 엔진 로그 17개 중 그 줄이 있는 로그 모두 같은 값) · 선언: sweep_index.meta.driver_version(output/single/benchlog/sweep_gpt-oss-20b-gb10_camp26102705_resume_20261028T073540Z · 측정 시 조립 — sweep_bench.sh 가 manifest 에서 읽는다) (manifest 선언 옮김) |
| 호스트 OS | 미관측 | 미관측 |

> 재현 좌표는 빌드 입력(릴리스 태그 또는 40자 SHA)이다 — 엔진 자기보고 · 인증서 · wheel 의 버전 문자열은 빌드 입력이 아니다(각 값을 만든 생산자는 출처 칸).

**양자화 구성**(체크포인트 config 관측 · 이름의 `q` 축은 선언 방식 하나 — ModelOpt MIXED 는 층 개수 우세가 보조 규칙)

| 범위 | dtype | 출처 |
|---|---|---|
| quantization_config 선언 대상(양자화 층 · 제외 목록 밖) | `mxfp4` | /app/models/OpenAI/gpt-oss-20b → <manifest.nas_model_path>/OpenAI/gpt-oss-20b/config.json quantization_config quant_method |

**이 모델에 필요한 패치**: 실린 빌드 패치 없음(또는 파일 기록 미관측 — 01 §1.1).
<!-- /FACT:resolved -->

> 이 절이 답하는 질문: 위 값들 중 무엇을 그대로 참조하고 무엇을 네 환경에서 다시 도출해야 하며, 이 빌드 트랙은 왜 골랐는가?

배정 빌드 입력은 0.18.0 wheel이다. 실제 모듈 자기보고 0.18.0과 별도 배포 메타데이터 0.18.0+cu130을 관측했다. source checkout과 빌드 원장 및 컴파일 바이트는 확인되지 않았다.

## 0.5 서빙 노브와 값의 지위
> 01-artifacts.md §1.3 의 `kind: value-status` 블록에서 기계가 생성한 요약이다. 지위가 `tuned` · `declared-requirement` 가 아닌 값은 **이 셀에서 조정된 적도, 빼면 깨진다고 확인된 적도 없다** — 필요조건으로 복사하지 마라. `declared-requirement` 는 빼거나 바꾸면 실패가 관측된 값이다(근거의 W id · artifacts 헤더를 보고 네 환경에서도 그 실패 조건이 성립하는지 판단한다).

<!-- FACT:knob_status -->
노브 6개 — `inherited` 6.

| 노브 | 값 | 지위 | 근거 | id |
|---|---|---|---|---|
| `gpu-memory-utilization` | 0.85 | inherited | 기존 재개 후보 손레버, 이 계보에서 비교 스윕 없음 | V1 |
| `max-model-len` | 131072 | inherited | 기존 후보 설정, 이 계보에서 비교 스윕 없음 | V2 |
| `max-num-seqs` | 16 | inherited | 동일 모델 과거 후보의 손레버, 이번 trial로 확인 | V3 |
| `kv-cache-memory-bytes` | 51539607552 | inherited | 동일 모델 과거 후보의 손레버, 이번 trial로 확인 | V4 |
| `kv-cache-dtype` | fp8 | inherited | 기존 재개 후보, 다른 KV dtype 비교 없음 | V5 |
| `quantization` | mxfp4 | inherited | 모델 체크포인트의 선언을 승계, 양자화 비교 없음 | V6 |
<!-- /FACT:knob_status -->

> 이 절이 답하는 질문: 이 레시피를 다른 환경으로 옮길 때 어떤 노브를 먼저 다시 재야 하고, 어떤 노브는 빼면 깨지는가?

V1~V6은 비교 스윕이 없는 후보 또는 손레버다. 이번 trial과 Full은 이 조합을 검증했지만 개별 값의 필요조건을 증명하지 않는다.

## 0.6 재검증 · 라우팅
- 이 자료를 `vllm-recipe-explorer` 인터뷰의 **warm-start 근거로만** 투입하고, 평소대로 plan-gate → 레시피 수렴 → 스모크 게이트를 그대로 밟아라(가속이지 우회 ✗).

> 이 절이 답하는 질문: 다른 환경(다른 HW · 다른 vLLM 버전 · 다른 노드 수)에서 이 지도를 쓸 때 무엇부터, 어떤 순서로 확인해야 하며, 실패하면 어디로 돌리는가?

1. 모델 가중치와 설정·설치 runtime을 직접 확인한다.
2. 예산과 health 및 기능을 확인하고 새 raw 경로로 동일 부하를 측정한다.
3. 오류 분류와 report를 확인하고 하드웨어나 부하가 다르면 재검증한다.

## 0.7 메타
<!-- FACT:meta -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.18.0/gpt-oss-20b/gb10-1g1n-sub-native/qmxfp4-len131072-kvfp8` |
| 형식 | `hint-payload/v7` |
| 앵커 | `PROVENANCE.json` — 봉인 때 hint 브랜치 페이로드 커밋에 묶인다 |
| 캠페인 · 셀 · 노드 | camp-26102705-codex-single8 · gpt-oss-20b-gb10 · sub |
| 발행 모드 | campaign |
| 발행 토픽 | camp_26102705_codex_single8__sub__gpt_oss_20b_gb10 |
| work-manifest | docs/_evidence/camp_26102705_codex_single8__sub__gpt_oss_20b_gb10.work-manifest.json |
| 증거 등급(task_class) | full_benchmark |
| 승인 출처 | campaign:hint_targets |
| 승인 시각(UTC) | 2026-10-26T21:47:32Z |
| 승인 발화(전사) | 사용자: 싱글 메인·서브 각각 서로 다른 4개 모델 총 8개 고유 모델, 모든 모델 Full bench 성공 후 hint 태그 발행까지 수행 |
| 현행 full 정의 충족(측정 등급) | 예 · 이 측정의 반복 3 · 정의 출처 `.claude/skills/adversarial-benchmark/SKILL.md:33`(03 §3.5) |
| 독립 사실 검증(발행 전) | 통과 — 검증자 `gptoss20b-a09-independent-checker`(저작자 `gptoss20b-hint-author` 아님) · 주장 12건 · 지적 12건 중 고침 12건 · 열림 0 · 2026-10-28T13:47:43Z |
<!-- /FACT:meta -->

<!-- FACT:missing -->
**결손** — 발행을 막지 않고 기재한다(부재와 실패는 다른 사실이다 · 차단은 양성 검출만).

| 코드 | 뜻 |
|---|---|
| `HINT_MISSING_BUILD_LEDGER` | 빌드 원장(`/opt/easy-vllm/build_ledger.json`) 부재 — 도입 시점과 부재 원인은 미관측이다. 패치 적용 결과는 `applied_set.status: unobservable` 과 라벨된 재구성으로만 말한다(X9). |
| `HINT_MISSING_RESOLVE` | `output/<t>/resolved.json` 을 이 빌드에 묶지 못했다(부재 또는 다른 vLLM 버전의 해소값) — torch·NGC·CUDA 는 이미지 관측값만 싣는다. |
<!-- /FACT:missing -->

<!-- FACT:legend -->
| kind | id | 필수 필드 | 자리 |
|---|---|---|---|
| `wall` | `W<n>` | id · kind · 증상 · 서명 · 원인 · 해소 · 검증 · 전이등급 · 출처 | 02 §2.2 |
| `rejected` | `R<n>` | id · kind · 시도 · 기각사유 · 출처 | 02 §2.3 |
| `misdiagnosis` | `M<n>` | id · kind · 증상 · 서명 · 원인 · 해소 · 검증 · 전이등급 · 출처 | 02 §2.4 |
| `value-status` | `V<n>` | id · kind · 노브 · 값 · 지위 · 근거 · 출처 | 01 §1.3 |
| `open-question` | `Q<n>` | id · kind · 물음 · 현재상태 · 출처 | 02 §2.7 |

**범례** — 린터가 hint-event 값 검증에 쓰는 어휘와 같은 상수에서 생성했다.

- 전이등급: **arch-invariant**(그대로 참조해도 된다) · **arch-scaled**(네 HW 에서 다시 잰다(메모리·노드 수에 비례)) · **arch-locked**(복사 금지 — 네 HW 에서 재도출한다) · **judgment**(맥락을 다시 해석한다)
- 지위: **tuned**(이 셀 계보에서 값을 바꿔 가며 잰 기록이 있다) · **inherited**(이전 셀·계획에서 가져왔고 이 셀에서 조정된 적 없다(손레버 포함)) · **negative-control**(비교를 위해 일부러 과잉·과소로 둔 값) · **engine-default**(설정하지 않았거나 엔진이 자동으로 정한 값) · **declared-requirement**(빼거나 바꾸면 실패가 관측된 필요조건(근거 = 그 실패의 W id 또는 실패를 기록한 artifacts/ 파일 헤더 — 값을 선언한 트리플렛 자신은 근거가 아니다))
- 현재지위(오진 · 권장 — **원래 주장**의 지위): **확증**(원래 주장이 참으로 확인됐다) · **반증**(원래 주장이 거짓으로 확인됐다(정정된 이해는 `해소` 칸에)) · **가설(강등)**(원래 주장(기전)이 확인되지 않아 가설로 내려왔다 — 처방이 아니다) · **미결**(원래 주장의 참 · 거짓을 아직 가르지 못했다)
<!-- /FACT:legend -->

## 0.8 comment
> 이 절이 답하는 질문: 위 규격에 들어가지 않지만 수신자에게 알려야 할 것이 있는가?

## 0.9 이름 꼬리
> 태그 이름의 결정론부(`<vllm>/<model>/<arch>/q·len·kv`)는 도구가 정한다. 꼬리는 이 셀을 **같은 q·len·kv 의 다른 셀과 가르는 노브**를 발행 Agent 가 골라 근거와 함께 적는 자리다(plan_26092908 §4.1 U5). 아래 표는 publish 때 기계가 낸 **후보**이고(강제 ✗), `continue` 가 draft `inputs/tail.json` 을 근거 대조한 뒤 확정 꼬리로 바꾼다. 같은 이름이 이미 있으면 끝에 발행 시각 `-t<YYMMDDHHMM>`(KST)이 붙는다.

<!-- FACT:name_tail -->
**확정 이름** `hint/0.18.0/gpt-oss-20b/gb10-1g1n-sub-native/qmxfp4-len131072-kvfp8` — 꼬리 0토큰.

_빈 꼬리 — 발행 Agent 가 가르는 노브가 없다고 명시했다(`inputs/tail.json` = `[]`)._
<!-- /FACT:name_tail -->

> 이 절이 답하는 질문: 이 셀을 같은 q·len·kv 의 다른 셀과 가르는 노브는 무엇이며 그것을 이름 꼬리로 어떻게 적는가 — 가르는 노브가 없으면 왜 빈 꼬리인가?

추가 꼬리는 선택하지 않았다. 이 셀에 고유한 비교 검증된 추가 노브가 없어 기본 이름 충돌 시 공식 발행기의 충돌 처리를 따른다.
