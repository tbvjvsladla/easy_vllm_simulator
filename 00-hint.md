# 00 · hint 지도 — 가장 먼저 읽는 파일

<!-- FACT:header -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.26.0/qwen3-4b/gb10-1g1n-sub-native/qbf16-len32768-kvturboquant4bitnc` |
| 형식 | `hint-payload/v7` · 이름 문법 `v7` |
| 생성(UTC · 주입) | 2026-10-27T10:08:19Z |
| 캠페인 · 셀 · 노드 · 모드 | camp-26102705-codex-single8 · c-tq4nc · sub · campaign |
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
| 자격 근거 | `output/single/benchlog/sweep_c-tq4nc_camp-26102705_qwen_a01_20261027T093346Z/level_01/post_health_c-tq4nc.json` · `output/single/benchlog/sweep_c-tq4nc_camp-26102705_qwen_a01_20261027T093346Z/level_01/guidellm_c-tq4nc.json` |
| 증거 등급(task_class) | full_benchmark |
| 측정 등급(bench_mode) | full |
| 셀 등급(cell_grade) | `full` — 성능 판정 · 부하 곡선 · 데이터셋 출처는 `03-benchmark.md` |
| 성능 판정(measurement.verdict) | `PASS` — 출처 output/single/benchlog/sweep_c-tq4nc_camp-26102705_qwen_a01_20261027T093346Z/verdict.json(output · verdict_rule 출력) verdict |
<!-- /FACT:grade -->

<!-- FACT:lite_summary -->
<!-- /FACT:lite_summary -->

## 0.1 요약
> 이 절이 답하는 질문: 이 셀은 무엇을 서빙했고, 무엇이 결정적이었으며, 수신자는 무엇을 가장 조심해야 하는가?

Qwen3-4B를 NVIDIA GB10 한 노드·GPU 한 개에서 vLLM 빌드 입력 v0.26.0, BF16 가중치·TurboQuant KV·비투기 graph 형상으로 재측정했고 동시성 1 decode 21.68 t/s의 weak PASS를 얻었다. 과거 backend 번역 오류와 하네스 종료를 구분한 W2/W3를 계승하되 이번에는 기본 Full을 새 경로에서 완주했다. V4의 고정 KV는 승계값이며 최적값·다른 GPU의 필요조건이 아니다.

## 0.2 유효맥락
<!-- FACT:context -->
| 항목 | 값 | 출처 |
|---|---|---|
| 모델 | qwen3-4b | sweep_index.meta(output/single/benchlog/sweep_c-tq4nc_camp-26102705_qwen_a01_20261027T093346Z) |
| HF repo | Qwen/Qwen3-4B | 체크포인트 .git/config remote url(huggingface.co · /app/models/Qwen/Qwen3/Qwen3-4B → <manifest.nas_model_path>/Qwen/Qwen3/Qwen3-4B) |
| HF revision(체크포인트 git HEAD) | 1cfa9a7208912126459214e8b04321603b3df60c | 체크포인트 .git HEAD → refs/heads/main → loose ref(/app/models/Qwen/Qwen3/Qwen3-4B → <manifest.nas_model_path>/Qwen/Qwen3/Qwen3-4B) · refs/remotes/origin/main 와 일치(받아 온 원격 커밋) — checkout 커밋(작업트리 일치는 미검증) · reflog: 측정 시점 checkout = 지금 HEAD(measured_utc 2026-10-27T10:05:40Z 이전 마지막 이동 2026-08-10T02:50:42Z) |
| base_model | Qwen/Qwen3-4B-Base | 체크포인트 README base_model(/app/models/Qwen/Qwen3/Qwen3-4B → <manifest.nas_model_path>/Qwen/Qwen3/Qwen3-4B) |
| base 슬러그 | qwen3-4b | model(qwen3-4b) − vocab quant_suffixes(naming.base_slug) |
| 양자화 | bf16 | naming-axis(q) — 측정 identity quantization 미관측(sweep_index.meta(output/single/benchlog/sweep_c-tq4nc_camp-26102705_qwen_a01_20261027T093346Z)) · q 축 = /app/models/Qwen/Qwen3/Qwen3-4B → <manifest.nas_model_path>/Qwen/Qwen3/Qwen3-4B/config.json quantization_config·hf_quant_config.json 부재 관측 → 비양자화 · vocab quant[bf16]←'none' — 인증서 quantization 이 N/A 라 명명 축 q(체크포인트 config 파생)를 옮겼다 |
| GPU | NVIDIA GB10 | sweep_index.meta(output/single/benchlog/sweep_c-tq4nc_camp-26102705_qwen_a01_20261027T093346Z) |
| 토폴로지 · TP | single · TP=1 | sweep_index.meta(output/single/benchlog/sweep_c-tq4nc_camp-26102705_qwen_a01_20261027T093346Z) |
| 실행 평면 | docker | artifacts.plane_of |
| vLLM(측정 강식별 키 `vllm_version`) | 0.26.0 | sweep_index.meta(output/single/benchlog/sweep_c-tq4nc_camp-26102705_qwen_a01_20261027T093346Z) |

**명명 축** — 태그 이름 = 결정론부(vllm · model · arch · q · len · kv — 도구가 증거에서 파생 · 발행자 입력 ✗) + 꼬리(발행 Agent 가 고르고 서빙 설정 file · key · value 로 근거 대조 · 0.9) + 중복 시 timestamp.

| 축 | 값 | 출처 |
|---|---|---|
| 세그먼트 `vllm` | 0.26.0 | track=선택자 Dockerfile.source-build(docker history build-arg 관측) · ref=docker history build-arg VLLM_REF · version=VLLM_VERSION 미관측 · repo=docker history build-arg VLLM_REPO · sha=미관측 · VLLM_REF=v0.26.0(업스트림 릴리스 태그 · v 제거) |
| 세그먼트 `model` | qwen3-4b | 서빙 yaml model(output/single/configs/c-tq4nc.yaml) · hf_repo=체크포인트 .git/config remote url(huggingface.co · /app/models/Qwen/Qwen3/Qwen3-4B → <manifest.nas_model_path>/Qwen/Qwen3/Qwen3-4B) · hf_repo(Qwen/Qwen3-4B) 마지막 성분 소문자 |
| 세그먼트 `arch` | gb10-1g1n-sub-native | derived(hw·gpus_per_node·nodes·role·target·plane) |
| 세그먼트 `recipe` | qbf16-len32768-kvturboquant4bitnc | derived(q·len·kv) |
| 축 `hw` | gb10 | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(캠페인 assignments(배정 SSOT)) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) · vocab hw[gb10]←'NVIDIA GB10' |
| 축 `gpus_per_node` | 1 | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(캠페인 assignments(배정 SSOT)) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `nodes` | 1 | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(캠페인 assignments(배정 SSOT)) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `role` | sub | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(캠페인 assignments(배정 SSOT)) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `target` | native | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(캠페인 assignments(배정 SSOT)) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) · target_gpu 'NVIDIA GB10' = 호스트 hw → native |
| 축 `plane` | docker | artifacts.plane_of → cell-env(IMAGE_TAG\|BUILD_DOCKERFILE) · vocab plane[docker]→'' |
| 축 `q` | bf16 | /app/models/Qwen/Qwen3/Qwen3-4B → <manifest.nas_model_path>/Qwen/Qwen3/Qwen3-4B/config.json quantization_config·hf_quant_config.json 부재 관측 → 비양자화 · vocab quant[bf16]←'none' |
| 축 `len` | 32768 | 서빙 yaml max-model-len(output/single/configs/c-tq4nc.yaml) |
| 축 `kv` | turboquant4bitnc | 서빙 yaml kv-cache-dtype(output/single/configs/c-tq4nc.yaml) · vocab kv[turboquant4bitnc]←'turboquant_4bit_nc' |
<!-- /FACT:context -->

> 이 절이 답하는 질문: 이 지도는 어떤 조건에서만 성립하며, 이 형상은 어떤 기전으로 버티고 다른 형상은 왜 실패했는가?

현재 유효맥락은 실제 GB10 단일 노드의 통합메모리와 BF16 가중치, 고정 TurboQuant KV다. KV 설정을 엔진이 되읽은 값과 토큰 용량 보고를 구분한다(devlog_26102718_Qwen3_4B_c_tq4nc_신규실측_서사 §기동과 기능). 과거 24GiB 캡은 다른 GPU의 실측이 아니며 이번 target은 GB10이다.

과거 3bit 자매 셀은 KV dtype이 달랐고 같은 바이트에서 토큰 용량이 달랐다(testlog_26090807_qwen3_4b_tq_kvquant_서브 §판정 요약). 이는 보고된 용량 차이이고 디코드 비용의 정확한 인과는 대조가 부족하다. 다른 PLE/offload 형상을 이번에 실행하지 않았으므로 그 실패 기전은 미결이다. 고정 KV가 전체 gmu 몫을 실제로 쓰는 것은 아니라고 엔진이 경고했다(devlog_26102718_Qwen3_4B_c_tq4nc_신규실측_서사 §기동과 기능).

## 0.3 벽 지도 요약
> 02-narrative.md §2.2 의 `kind: wall` 블록(과 §2.4 의 `kind: misdiagnosis` 블록)에서 기계가 생성한 표다(손저작 ✗ · 순서 = 넘은 순서). 원인·해소의 전문과 원문 발췌는 02 에 있다.

<!-- FACT:wall_map -->
| 순서 | id | 증상 | 해소 | 전이등급 | 검증 |
|---|---|---|---|---|---|
| 1 | W1 | 이전 source-build 의존 핀 충돌 | 당시 regen_requirements.py로 wheel METADATA에서 요구를 다시 생성했다. 이번 requirements 사본 불일치는 별도 재현 결손이다. | arch-locked | testlog_26090807_qwen3_4b_tq_kvquant_서브 §환경 스냅샷 |
| 2 | W2 | 합성 backend 값 거부 | openai-chat 값과 backend=chat_template reason을 분리했고 이번에도 그 정식 인자를 사용했다. | judgment | testlog_26101015_knob프리셋_E2E_셀D_qwen3-4b §4. 운영 사건 |
| 3 | W3 | 이전 하네스 제한으로 스윕 중단 | 과거 setsid nohup 재실행은 역사 기록이며 이번 권한으로 승계하지 않았다. 이번에는 wall-time3600초의 연결된 공식 실행에서 새 raw를 완주했다. | judgment | testlog_26101015_knob프리셋_E2E_셀D_qwen3-4b §4. 운영 사건 |
| 4 | W4 | 관측이 서브에 있는 발행 경계 | 10-11 계획은 single 서브가 기존 승인·별도 clone·독립 검증으로 자기 셀을 발행하고 카탈로그는 메인이 소유하도록 했다. 새 캠페인은 그 승인 운반체를 보존한다. | judgment | plan_26101104_hint발행체제_개편_서브자체발행 §1. 무엇이 문제였나 |
| 5 | W5 | 적용 가능한 외부 E 공백 | e-search empty와 weak 루프라인 fallback을 공식 판정에 남겼다. 외부 E가 있다고 바꾸지 않았다. | judgment | testlog_26102719_Qwen3_4B_c_tq4nc_신규_Full_판정 §외부 검색 |

**오진 · 정정**(02 §2.4 — 현재지위가 `가설(강등)` 인 기전은 처방이 아니다)

| id | 증상 | 현재지위 | 검증 |
|---|---|---|---|
| M1 | 과거 판정점 decode 세 값이 소수2자리까지 같음 | 가설(강등) | devlog_26101108_c-tq4nc_셀D_서빙_측정_knob해소 §측정 |
<!-- /FACT:wall_map -->

## 0.4 결정론 해소값
<!-- FACT:resolved -->
| 항목 | 값 | 출처 |
|---|---|---|
| vLLM 빌드 입력(종류) | release | naming.vllm_build_input |
| vLLM 빌드 입력(ref) | v0.26.0 | docker history build-arg VLLM_REF |
| vLLM 빌드 계보 SHA(40자) | 미관측 | 빌드 입력/원장에 결속된 SHA 미관측 — runtime checkout 관측과 컴파일 바이트 증명은 별개 |
| 직전 릴리스(prev_release) | 해당 없음 — 릴리스 태그 빌드(이름 = 릴리스 자체 · `<직전 릴리스>-g<sha>` 는 커밋 핀에만) | naming.vllm_build_input(kind=release) |
| vLLM 엔진 자기보고(엔진 로그 기동 배너) | 0.26.1.dev0+g568afb3a1.d20260907 | output/single/benchlog/sweep_c-tq4nc_camp-26102705_qwen_a01_20261027T093346Z/lite_engine_c-tq4nc.log:52 · 기동 2026-10-27T09:40:27Z(컨테이너 시계 UTC 가정) · docs/logs/sub/sessions/qwen3_4b_camp26102705_a01.json — 측정 창을 포함한 현재 serve 세션 안·첫 측정 이전 기동 배너 · 같은 값의 다른 로그 3건 · 지위 `this-measurement` |
| 인증서 `vllm_version`(강한 일치 키) | 미관측 | 생산자 `absent` · 인증서 vllm_version 없음 |
| wheel 메타(`vllm_build`) | 미관측 | 미관측 — 확인한 attestation vllm_dist_version · native pip freeze · 빌드 원장 vllm.dist_version에 값 없음(runtime_version 기록은 배포 메타로 간주하지 않는다)(sweep_index.meta.vllm_build='NA' 는 sweep_bench 가 엔진 로그에서 `v<x.y.z…>` 정규식으로 잡는 값이다 → engine_self_report 의 원천) |
| vLLM runtime 버전(관측 기록 · 배포 메타 여부 미판정) | 0.26.1.dev0+g568afb3a1.d20260907 | docs/simlog/single8_qwen3_4b_20261027T093346Z/attempt1/runtime_identity.json (관측 기록 · docs/simlog/single8_qwen3_4b_20261027T093346Z/attempt1/evidence_index.json가 config=c-tq4nc, node=sub, sweep_dir=output/single/benchlog/sweep_c-tq4nc_camp-26102705_qwen_a01_20261027T093346Z, measured_utc=2026-10-27T10:05:40Z로 결속; 수집 명령 기록: docs/simlog/single8_qwen3_4b_20261027T093346Z/attempt1/runtime_identity.json#/collection_argv(실행 성공·진위·값 출처는 이 필드만으로 미검증); 컴파일 바이트 일치는 별도 미검증) vllm_version |
| vLLM runtime source checkout SHA(컴파일 바이트 증명 아님) | 568afb3a13806beb53bb2e6bd518269357b237c0 | docs/simlog/single8_qwen3_4b_20261027T093346Z/attempt1/runtime_identity.json (관측 기록 · docs/simlog/single8_qwen3_4b_20261027T093346Z/attempt1/evidence_index.json가 config=c-tq4nc, node=sub, sweep_dir=output/single/benchlog/sweep_c-tq4nc_camp-26102705_qwen_a01_20261027T093346Z, measured_utc=2026-10-27T10:05:40Z로 결속; 수집 명령 기록: docs/simlog/single8_qwen3_4b_20261027T093346Z/attempt1/runtime_identity.json#/collection_argv(실행 성공·진위·값 출처는 이 필드만으로 미검증); 컴파일 바이트 일치는 별도 미검증) source_head |
| vLLM runtime source describe(관측 기록) | v0.26.0-dirty | docs/simlog/single8_qwen3_4b_20261027T093346Z/attempt1/runtime_identity.json (관측 기록 · docs/simlog/single8_qwen3_4b_20261027T093346Z/attempt1/evidence_index.json가 config=c-tq4nc, node=sub, sweep_dir=output/single/benchlog/sweep_c-tq4nc_camp-26102705_qwen_a01_20261027T093346Z, measured_utc=2026-10-27T10:05:40Z로 결속; 수집 명령 기록: docs/simlog/single8_qwen3_4b_20261027T093346Z/attempt1/runtime_identity.json#/collection_argv(실행 성공·진위·값 출처는 이 필드만으로 미검증); 컴파일 바이트 일치는 별도 미검증) source_describe |
| 체크포인트 revision(HF · git HEAD) | 1cfa9a7208912126459214e8b04321603b3df60c | 체크포인트 .git HEAD → refs/heads/main → loose ref(/app/models/Qwen/Qwen3/Qwen3-4B → <manifest.nas_model_path>/Qwen/Qwen3/Qwen3-4B) · refs/remotes/origin/main 와 일치(받아 온 원격 커밋) — checkout 커밋(작업트리 일치는 미검증) · reflog: 측정 시점 checkout = 지금 HEAD(measured_utc 2026-10-27T10:05:40Z 이전 마지막 이동 2026-08-10T02:50:42Z) |
| 빌드 트랙 | source-build | artifacts 선택자(image-history:docker history build-arg 관측(VLLM_REF=v0.26.0 · VLLM_VERSION 없음 → 소스빌드 Dockerfile) — 셀 env 선택자 없음 · output/single/docker-compose.yaml 리터럴 build.dockerfile: Dockerfile.source-build(선택자 변수 없음 — compose build 는 늘 Dockerfile.source-build 를 짓는다 · 이 이미지를 지은 증거 아님)) |
| Dockerfile(선택자) | Dockerfile.source-build | image-history:docker history build-arg 관측(VLLM_REF=v0.26.0 · VLLM_VERSION 없음 → 소스빌드 Dockerfile) — 셀 env 선택자 없음 · output/single/docker-compose.yaml 리터럴 build.dockerfile: Dockerfile.source-build(선택자 변수 없음 — compose build 는 늘 Dockerfile.source-build 를 짓는다 · 이 이미지를 지은 증거 아님) |
| vLLM 저장소 | https://github.com/vllm-project/vllm.git | docker history build-arg VLLM_REPO |
| 이미지 태그 | easy-vllm:0.26.0-cu132-aarch64-source | 셀 env(output/single/envs/.env.c-tq4nc) IMAGE_TAG |
| 이미지 digest | sha256:629942890c0570c8f1c3235bf6927ae4eb315e30336cd68551fdf474a60a44f7 | sweep_index.meta.image_digest(measured(docker inspect .Image)) |
| torch | 2.12.0a0+5aff392 | docker image inspect Config.Env PYTORCH_VERSION |
| CUDA | 13.2.1.009 | docker image inspect Config.Env CUDA_VERSION |
| NGC 베이스 | 26.05 | docker image inspect Config.Env NVIDIA_PYTORCH_VERSION |
| CPU arch | arm64 | docker image inspect Architecture |
| 드라이버 | 580.178.04 | sweep_index.meta.driver_version(output/single/benchlog/sweep_c-tq4nc_camp-26102705_qwen_a01_20261027T093346Z · 측정 시 조립 — sweep_bench.sh 가 manifest 에서 읽는다) (manifest 선언 옮김) |
| 드라이버(노드별 관측) | 미관측 | 미관측 |
| ⚠ 드라이버 관측 ≠ 선언 | 대조 불가 — 선언 580.178.04 · 관측 없음 | 선언: sweep_index.meta.driver_version(output/single/benchlog/sweep_c-tq4nc_camp-26102705_qwen_a01_20261027T093346Z · 측정 시 조립 — sweep_bench.sh 가 manifest 에서 읽는다) (manifest 선언 옮김) |
| 호스트 OS | 미관측 | 미관측 |

> 재현 좌표는 빌드 입력(릴리스 태그 또는 40자 SHA)이다 — 엔진 자기보고 · 인증서 · wheel 의 버전 문자열은 빌드 입력이 아니다(각 값을 만든 생산자는 출처 칸).

**양자화 구성**(체크포인트 config 관측 · 이름의 `q` 축은 선언 방식 하나 — ModelOpt MIXED 는 층 개수 우세가 보조 규칙)

| 범위 | dtype | 출처 |
|---|---|---|
| 기본 dtype(비양자화 가중치·계산) | `bfloat16` | /app/models/Qwen/Qwen3/Qwen3-4B → <manifest.nas_model_path>/Qwen/Qwen3/Qwen3-4B/config.json torch_dtype |

**이 모델에 필요한 패치**: 실린 빌드 패치 없음(또는 파일 기록 미관측 — 01 §1.1).
<!-- /FACT:resolved -->

> 이 절이 답하는 질문: 위 값들 중 무엇을 그대로 참조하고 무엇을 네 환경에서 다시 도출해야 하며, 이 빌드 트랙은 왜 골랐는가?

빌드 입력은 0.4의 공식 저장소 릴리스 ref v0.26.0이다. source-build 선택은 로컬 기존 이미지의 history 관측이며, 이번 호출에서 새 빌드를 했다는 뜻은 아니다. runtime checkout SHA와 dirty describe, 설치 문자열은 0.4의 별도 관측이고 컴파일 바이트 증명이 아니다. requirements 사본은 이미지 안 바이트와 달라 generated-unverified다. 재현 빌드는 그 경고를 해소한 뒤 스모크해야 한다.

arch-locked 항목은 sm12.1a/CUDA/torch·attention 커널 조합이다. 다른 GPU에서 upstream 지원과 설치 ABI를 다시 확인한다. 핵심 의존은 artifacts/build_recipe/Dockerfile.source-build 및 requirements.txt의 torch·Ray·FlashInfer·Triton 핀을 확인하되, 현재 requirements는 참고 형상이므로 실제 설치 핀이라고 단정하지 않는다. 이번 vllm.__version__ 모듈 자기보고와 이미지 환경 관측은 서로 출처가 다르다. vLLM distribution metadata는 수집되지 않았고 runtime 버전으로 승격하지 않는다.

## 0.5 서빙 노브와 값의 지위
> 01-artifacts.md §1.3 의 `kind: value-status` 블록에서 기계가 생성한 요약이다. 지위가 `tuned` · `declared-requirement` 가 아닌 값은 **이 셀에서 조정된 적도, 빼면 깨진다고 확인된 적도 없다** — 필요조건으로 복사하지 마라. `declared-requirement` 는 빼거나 바꾸면 실패가 관측된 값이다(근거의 W id · artifacts 헤더를 보고 네 환경에서도 그 실패 조건이 성립하는지 판단한다).

<!-- FACT:knob_status -->
노브 5개 — `inherited` 5.

| 노브 | 값 | 지위 | 근거 | id |
|---|---|---|---|---|
| `gpu-memory-utilization` | 0.9 | inherited | 기존 c-tq4nc recipe 유지; 새 Phase2 수렴/필요조건 대조를 주장하지 않음 | V1 |
| `max-model-len` | 32768 | inherited | 기존 c-tq4nc recipe 유지; 새 Phase2 수렴/필요조건 대조를 주장하지 않음 | V2 |
| `max-num-seqs` | 8 | inherited | 기존 c-tq4nc recipe 유지; 새 Phase2 수렴/필요조건 대조를 주장하지 않음 | V3 |
| `kv-cache-memory-bytes` | 11811160064 | inherited | 기존 c-tq4nc recipe 유지; 새 Phase2 수렴/필요조건 대조를 주장하지 않음 | V4 |
| `kv-cache-dtype` | turboquant_4bit_nc | inherited | 기존 c-tq4nc recipe 유지; 새 Phase2 수렴/필요조건 대조를 주장하지 않음 | V5 |
<!-- /FACT:knob_status -->

> 이 절이 답하는 질문: 이 레시피를 다른 환경으로 옮길 때 어떤 노브를 먼저 다시 재야 하고, 어떤 노브는 빼면 깨지는가?

KV 바이트·gmu·문맥·시퀀스 상한은 새 호스트에서 RAM gate→엔진 KV 토큰 보고→대표 요청 길이와 동시성→클램프 재검증 순서로 다시 도출한다. 다섯 V 항목은 모두 inherited다. 이번에는 노브를 빼서 실패하는 대조를 하지 않아 declared-requirement로 입증된 값이 없다. FLASH_ATTN export의 유효성도 엔진 경고와 TURBOQUANT 선택을 따로 읽는다. 손레버를 최적값으로 복사하지 않는다.

## 0.6 재검증 · 라우팅
- 이 자료를 `vllm-recipe-explorer` 인터뷰의 **warm-start 근거로만** 투입하고, 평소대로 plan-gate → 레시피 수렴 → 스모크 게이트를 그대로 밟아라(가속이지 우회 ✗).

> 이 절이 답하는 질문: 다른 환경(다른 HW · 다른 vLLM 버전 · 다른 노드 수)에서 이 지도를 쓸 때 무엇부터, 어떤 순서로 확인해야 하며, 실패하면 어디로 돌리는가?

1. 체크포인트 config·샤드 dtype·tokenizer template를 parse/crosscheck하고, 다른 모델이면 이 지도를 폐기한다.
2. 이미지 입력 ref와 실제 설치 버전·소스·ABI를 대조한다. 상위 버전의 정식 TurboQuant/attention 지원 여부를 확인하며 이번 셀에 별도 runtime/build 패치가 실리지 않았다는 사실을 상위 버전 보증으로 확대하지 않는다.
3. 실제 baseline·가중치·KV·runtime allowance로 보호 예산을 선언하고 공식 RAM gate를 통과한다. 거부면 예산/전략을 재도출한다.
4. 공식 single-up의 ready 상한 내 HTTP200과 completion/reasoning/tool 응답을 확인한다. 경고만으로 실패·성공을 추정하지 않는다.
5. lite와 기본 Full 반복을 새 경로에 남기고 weak 판정 및 요청수 차이를 확인한다. 빌드·핀은 upstream-version-watch, 설정·KV는 vllm-recipe-explorer, 기대 이하 성능은 adversarial-benchmark, 노드·토폴로지는 해당 manifest owner/terraforming_node로 반환한다.

## 0.7 메타
<!-- FACT:meta -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.26.0/qwen3-4b/gb10-1g1n-sub-native/qbf16-len32768-kvturboquant4bitnc` |
| 형식 | `hint-payload/v7` |
| 앵커 | `PROVENANCE.json` — 봉인 때 hint 브랜치 페이로드 커밋에 묶인다 |
| 캠페인 · 셀 · 노드 | camp-26102705-codex-single8 · c-tq4nc · sub |
| 발행 모드 | campaign |
| 발행 토픽 | camp_26102705_codex_single8__sub__c_tq4nc |
| work-manifest | docs/_evidence/camp_26102705_codex_single8__sub__c_tq4nc.work-manifest.json |
| 증거 등급(task_class) | full_benchmark |
| 승인 출처 | campaign:hint_targets |
| 승인 시각(UTC) | 2026-10-26T21:47:32Z |
| 승인 발화(전사) | 사용자: 싱글 메인·서브 각각 서로 다른 4개 모델 총 8개 고유 모델, 모든 모델 Full bench 성공 후 hint 태그 발행까지 수행 |
| 현행 full 정의 충족(측정 등급) | 미관측 · 이 측정의 반복 3 · 정의 출처 `.claude/rules/docs.md`(03 §3.5) |
| 독립 사실 검증(발행 전) | 통과 — 검증자 `sub-qwen3-4b-independent-checker-a03`(저작자 `sub-qwen3-4b-author` 아님) · 주장 54건 · 지적 4건 중 고침 4건 · 열림 0 · 2026-10-27T11:17:41Z |
<!-- /FACT:meta -->

<!-- FACT:missing -->
**결손** — 발행을 막지 않고 기재한다(부재와 실패는 다른 사실이다 · 차단은 양성 검출만).

| 코드 | 뜻 |
|---|---|
| `HINT_MISSING_BUILD_LEDGER` | 빌드 원장(`/opt/easy-vllm/build_ledger.json`) 부재 — 원장 도입 전 이미지다. 패치 적용 결과는 `applied_set.status: unobservable` 과 라벨된 재구성으로만 말한다(X9). |
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
**확정 이름** `hint/0.26.0/qwen3-4b/gb10-1g1n-sub-native/qbf16-len32768-kvturboquant4bitnc` — 꼬리 0토큰.

_빈 꼬리 — 발행 Agent 가 가르는 노브가 없다고 명시했다(`inputs/tail.json` = `[]`)._
<!-- /FACT:name_tail -->

> 이 절이 답하는 질문: 이 셀을 같은 q·len·kv 의 다른 셀과 가르는 노브는 무엇이며 그것을 이름 꼬리로 어떻게 적는가 — 가르는 노브가 없으면 왜 빈 꼬리인가?

빈 꼬리 []를 선택했다. 추가 노브 차이를 유일한 구별 조건으로 입증하지 않았으므로 결정론 q·len·kv 외 꼬리를 붙이지 않는다. 기존 이름 충돌은 continue의 정식 timestamp 경로가 판단한다.
