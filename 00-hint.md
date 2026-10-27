# 00 · hint 지도 — 가장 먼저 읽는 파일

<!-- FACT:header -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.31.0/gemma-4-26b-a4b-it/gb10-1g1n-main-native/qbf16-len262144-kvfp8-t2610271153` |
| 형식 | `hint-payload/v7` · 이름 문법 `v7` |
| 생성(UTC · 주입) | 2026-10-27T02:53:21Z |
| 캠페인 · 셀 · 노드 · 모드 | camp-26102705-codex-single8 · gemma26b-ctx-single · main · campaign |
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
| 자격 근거 | `output/single/benchlog/sweep_gemma26b-ctx-single/level_01/post_health_gemma26b-ctx-single.json` · `output/single/benchlog/sweep_gemma26b-ctx-single/level_01/guidellm_gemma26b-ctx-single.json` |
| 증거 등급(task_class) | full_benchmark |
| 측정 등급(bench_mode) | full |
| 셀 등급(cell_grade) | `full` — 성능 판정 · 부하 곡선 · 데이터셋 출처는 `03-benchmark.md` |
| 성능 판정(measurement.verdict) | `PASS` — 출처 output/single/benchlog/sweep_gemma26b-ctx-single/verdict.json(output · verdict_rule 출력) verdict |
<!-- /FACT:grade -->

<!-- FACT:lite_summary -->
<!-- /FACT:lite_summary -->

## 0.1 요약
> 이 절이 답하는 질문: 이 셀은 무엇을 서빙했고, 무엇이 결정적이었으며, 수신자는 무엇을 가장 조심해야 하는가?

Gemma-4-26B-A4B-it BF16를GB10 GPU1·노드1·TP1에서0.31.0 source이미지·native262144·KVfp8·PLE없음·specoff형상으로서빙했고이번신규Full 동시성1 decode23.32t/s를측정했다.
과거unclamped KV의memwatch종료(W1)와내부포트혼동(W2)을넘은형상을승계했으며이번새레버조정은없다.
KV클램프존재의과거음성대조(V3)와정확3368402176바이트값의최적성은구분한다.

## 0.2 유효맥락
<!-- FACT:context -->
| 항목 | 값 | 출처 |
|---|---|---|
| 모델 | gemma-4-26b-a4b-it | sweep_index.meta(output/single/benchlog/sweep_gemma26b-ctx-single) |
| HF repo | google/gemma-4-26B-A4B-it | 체크포인트 .git/config remote url(huggingface.co · /app/models/Google/Gemma_4/gemma-4-26B-A4B-it → <manifest.nas_model_path>/Google/Gemma_4/gemma-4-26B-A4B-it) |
| HF revision(체크포인트 git HEAD) | 01e5b3ee840d3a9e0b0b493c593e85398a30ef75 | 체크포인트 .git HEAD → refs/heads/main → loose ref(/app/models/Google/Gemma_4/gemma-4-26B-A4B-it → <manifest.nas_model_path>/Google/Gemma_4/gemma-4-26B-A4B-it) · refs/remotes/origin/main 와 일치(받아 온 원격 커밋) — checkout 커밋(작업트리 일치는 미검증) · reflog: 측정 시점 checkout = 지금 HEAD(measured_utc 2026-10-27T02:09:53Z 이전 마지막 이동 2026-07-18T04:49:16Z) |
| base_model | google/gemma-4-26B-A4B | 체크포인트 README base_model(/app/models/Google/Gemma_4/gemma-4-26B-A4B-it → <manifest.nas_model_path>/Google/Gemma_4/gemma-4-26B-A4B-it) |
| base 슬러그 | gemma-4-26b-a4b-it | model(gemma-4-26b-a4b-it) − vocab quant_suffixes(naming.base_slug) |
| 양자화 | bf16 | naming-axis(q) — 측정 identity quantization 미관측(sweep_index.meta(output/single/benchlog/sweep_gemma26b-ctx-single)) · q 축 = /app/models/Google/Gemma_4/gemma-4-26B-A4B-it → <manifest.nas_model_path>/Google/Gemma_4/gemma-4-26B-A4B-it/config.json quantization_config·hf_quant_config.json 부재 관측 → 비양자화 · vocab quant[bf16]←'none' — 인증서 quantization 이 N/A 라 명명 축 q(체크포인트 config 파생)를 옮겼다 |
| GPU | NVIDIA GB10 | sweep_index.meta(output/single/benchlog/sweep_gemma26b-ctx-single) |
| 토폴로지 · TP | single · TP=1 | sweep_index.meta(output/single/benchlog/sweep_gemma26b-ctx-single) |
| 실행 평면 | docker | artifacts.plane_of |
| vLLM(측정 강식별 키 `vllm_version`) | 0.31.0 | sweep_index.meta(output/single/benchlog/sweep_gemma26b-ctx-single) |

**명명 축** — 태그 이름 = 결정론부(vllm · model · arch · q · len · kv — 도구가 증거에서 파생 · 발행자 입력 ✗) + 꼬리(발행 Agent 가 고르고 서빙 설정 file · key · value 로 근거 대조 · 0.9) + 중복 시 timestamp.

| 축 | 값 | 출처 |
|---|---|---|
| 세그먼트 `vllm` | 0.31.0 | track=선택자 Dockerfile.source-build(docker history build-arg 관측) · ref=docker history build-arg VLLM_REF · version=VLLM_VERSION 미관측 · repo=docker history build-arg VLLM_REPO · sha=output/single/resolved.json upstream_delta.to_sha · VLLM_REF=v0.31.0(업스트림 릴리스 태그 · v 제거) |
| 세그먼트 `model` | gemma-4-26b-a4b-it | 서빙 yaml model(output/single/configs/gemma26b-ctx-single.yaml) · hf_repo=체크포인트 .git/config remote url(huggingface.co · /app/models/Google/Gemma_4/gemma-4-26B-A4B-it → <manifest.nas_model_path>/Google/Gemma_4/gemma-4-26B-A4B-it) · hf_repo(google/gemma-4-26B-A4B-it) 마지막 성분 소문자 |
| 세그먼트 `arch` | gb10-1g1n-main-native | derived(hw·gpus_per_node·nodes·role·target·plane) |
| 세그먼트 `recipe` | qbf16-len262144-kvfp8-t2610271153 | derived(q·len·kv) + timestamp(이름 중복 · generated_utc KST) |
| 축 `hw` | gb10 | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(캠페인 assignments(배정 SSOT)) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) · vocab hw[gb10]←'NVIDIA GB10' |
| 축 `gpus_per_node` | 1 | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(캠페인 assignments(배정 SSOT)) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `nodes` | 1 | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(캠페인 assignments(배정 SSOT)) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `role` | main | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(캠페인 assignments(배정 SSOT)) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `target` | native | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(캠페인 assignments(배정 SSOT)) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) · target_gpu 'NVIDIA GB10' = 호스트 hw → native |
| 축 `plane` | docker | artifacts.plane_of → cell-env(IMAGE_TAG\|BUILD_DOCKERFILE) · vocab plane[docker]→'' |
| 축 `q` | bf16 | /app/models/Google/Gemma_4/gemma-4-26B-A4B-it → <manifest.nas_model_path>/Google/Gemma_4/gemma-4-26B-A4B-it/config.json quantization_config·hf_quant_config.json 부재 관측 → 비양자화 · vocab quant[bf16]←'none' |
| 축 `len` | 262144 | 서빙 yaml max-model-len(output/single/configs/gemma26b-ctx-single.yaml) |
| 축 `kv` | fp8 | 서빙 yaml kv-cache-dtype(output/single/configs/gemma26b-ctx-single.yaml) · vocab kv[fp8]←'fp8' |
| timestamp | `t2610271153` | 같은 이름이 이미 있어 붙였다(이 발행의 generated_utc 를 KST 로 · 결정론) |
<!-- /FACT:context -->

> 이 절이 답하는 질문: 이 지도는 어떤 조건에서만 성립하며, 이 형상은 어떤 기전으로 버티고 다른 형상은 왜 실패했는가?

GB10통합메모리에서가중치와KV는호스트총량을같이나눈다. 과거gmu0.85만으로잡힌KV52.89GiB와weights48.54GiB뒤MemAvailable이절대문턱밑으로내려가memwatch가종료했다(W1). 현재절대KV는3.14GiB형상이다. 선언예산과실제로그를구분하며전용VRAM환경에같은호스트산식을그대로쓰지않는다.

Gemma4ForConditionalGeneration의30층중25sliding/5full, window1024, slidinghead_dim256/full512, expert128/top8구조다(00 FACT·plan_26102401). 이 형상의 실제 엔진은TRITON_ATTN을선택했다. 셀env의명시적강제값이나다른backend실패대조로읽지않는다. PLE·spec/MTP는이셀에선언하지않았다. fp8은KV이고가중치BF16와분리한다. scalingfactor없는fp8정확도경고를성능/기능스모크PASS만으로해소했다고주장하지않는다.

목표길이는native262144이며과거240932토큰needle성공은역사적검증이다. 이번새Full은1024/256부하이고최대길이품질이나긴요청동시성을새로입증하지않는다. 새엔진config/backend/graph와일반completion관측은testlog_26102711_gemma26b_ctx_single_코덱스_메인_Full_판정 §4를따르며report의NA를실제미관측으로바꾸지않는다.

## 0.3 벽 지도 요약
> 02-narrative.md §2.2 의 `kind: wall` 블록(과 §2.4 의 `kind: misdiagnosis` 블록)에서 기계가 생성한 표다(손저작 ✗ · 순서 = 넘은 순서). 원인·해소의 전문과 원문 발췌는 02 에 있다.

<!-- FACT:wall_map -->
| 순서 | id | 증상 | 해소 | 전이등급 | 검증 |
|---|---|---|---|---|---|
| 1 | W1 | 과거KV절대클램프null트라이얼이KV52.89GiB적재뒤memwatch종료 | 목표262144×batch1의KV3368402176절대클램프를넣어재기동 | arch-scaled | trial02전용스모크통과와과거engineKV3.14GiB; 이번도같은클램프의신규health/FullPASS |
| 2 | W2 | 과거정규기동rc5 health대기timeout; 엔진은내부8912에서기동 | 이셀yamlport를8000으로교정; host포트는셀env8912유지 | arch-invariant | 다음과거기동health200470초; 현재도교정된yaml/공식health성공 |

**오진 · 정정**(02 §2.4 — 현재지위가 `가설(강등)` 인 기전은 처방이 아니다)

| id | 증상 | 현재지위 | 검증 |
|---|---|---|---|
| M1 | 과거요약괄호가kv_clamp_absent를사살사유처럼읽게함 | 미결 | 과거원장L491/L493과trialwatchdog9962별도대조 |
<!-- /FACT:wall_map -->

## 0.4 결정론 해소값
<!-- FACT:resolved -->
| 항목 | 값 | 출처 |
|---|---|---|
| vLLM 빌드 입력(종류) | release | naming.vllm_build_input |
| vLLM 빌드 입력(ref) | v0.31.0 | docker history build-arg VLLM_REF |
| vLLM 소스 SHA(40자) | db9527a46873454610df6dbedf79a36d6bf1a7f6 | output/single/resolved.json upstream_delta.to_sha |
| 직전 릴리스(prev_release) | 해당 없음 — 릴리스 태그 빌드(이름 = 릴리스 자체 · `<직전 릴리스>-g<sha>` 는 커밋 핀에만) · 관측: 빌드 원장 describe `v0.31.0` · 업스트림 델타 from_ref `v0.30.0`(이 빌드가 올라온 이전 릴리스) | 빌드 원장(docker-image(main-local):sha256:339737df6571ce00993e731ef064a8db66085beaa650e4aa63bdb24f414b2034:/opt/easy-vllm/build_ledger.json · measured(build-time, in-image)) vllm.describe(git describe --tags · 빌드 시점) · output/single/resolved.json upstream_delta.from_ref(해소 기록) |
| vLLM 엔진 자기보고(엔진 로그 기동 배너) | 0.31.1.dev0+gdb9527a46.d20261014 | output/single/benchlog/sweep_gemma26b-ctx-single/lite_engine_head_gemma26b-ctx-single.log:38(이 측정의 엔진 로그 기동 배너) · 지위 `this-measurement` |
| 인증서 `vllm_version`(강한 일치 키) | 미관측 | 생산자 `absent` · 인증서 vllm_version 없음 |
| wheel 메타(`vllm_build`) | 0.31.1.dev0+gdb9527a46.d20261014.cu133 | 빌드 원장(docker-image(main-local):sha256:339737df6571ce00993e731ef064a8db66085beaa650e4aa63bdb24f414b2034:/opt/easy-vllm/build_ledger.json · measured(build-time, in-image)) vllm.dist_version(빌드 시점 measured · 이미지 안 배포 메타) |
| 체크포인트 revision(HF · git HEAD) | 01e5b3ee840d3a9e0b0b493c593e85398a30ef75 | 체크포인트 .git HEAD → refs/heads/main → loose ref(/app/models/Google/Gemma_4/gemma-4-26B-A4B-it → <manifest.nas_model_path>/Google/Gemma_4/gemma-4-26B-A4B-it) · refs/remotes/origin/main 와 일치(받아 온 원격 커밋) — checkout 커밋(작업트리 일치는 미검증) · reflog: 측정 시점 checkout = 지금 HEAD(measured_utc 2026-10-27T02:09:53Z 이전 마지막 이동 2026-07-18T04:49:16Z) |
| 빌드 트랙 | source-build | artifacts 선택자(build-ledger.dockerfile) |
| Dockerfile(선택자) | Dockerfile.source-build | build-ledger.dockerfile |
| vLLM 저장소 | https://github.com/vllm-project/vllm.git | docker history build-arg VLLM_REPO |
| 이미지 태그 | easy-vllm:0.31.0-cu133-aarch64-source | 셀 env(output/single/envs/.env.gemma26b-ctx-single) IMAGE_TAG |
| 이미지 digest | sha256:339737df6571ce00993e731ef064a8db66085beaa650e4aa63bdb24f414b2034 | sweep_index.meta.image_digest(measured(docker inspect .Image)) |
| torch | 2.13.0a0+9186a08 | docker image inspect Config.Env PYTORCH_VERSION |
| CUDA | 13.3.1.008 | docker image inspect Config.Env CUDA_VERSION |
| NGC 베이스 | nvcr.io/nvidia/pytorch:26.07-py3 | docker image inspect Config.Env NVIDIA_PYTORCH_VERSION = resolved.json ngc_base.image 접두 일치 |
| CPU arch | arm64 | docker image inspect Architecture |
| 드라이버 | 580.178.04 | sweep_index.meta.driver_version(output/single/benchlog/sweep_gemma26b-ctx-single · 측정 시 조립 — sweep_bench.sh 가 manifest 에서 읽는다) (manifest 선언 옮김) |
| 드라이버(노드별 관측) | 미관측 | 미관측 |
| ⚠ 드라이버 관측 ≠ 선언 | 대조 불가 — 선언 580.178.04 · 관측 없음 | 선언: sweep_index.meta.driver_version(output/single/benchlog/sweep_gemma26b-ctx-single · 측정 시 조립 — sweep_bench.sh 가 manifest 에서 읽는다) (manifest 선언 옮김) |
| 호스트 OS | 미관측 | 미관측 |

> 재현 좌표는 빌드 입력(릴리스 태그 또는 40자 SHA)이다 — 엔진 자기보고 · 인증서 · wheel 의 버전 문자열은 빌드 입력이 아니다(각 값을 만든 생산자는 출처 칸).

**양자화 구성**(체크포인트 config 관측 · 이름의 `q` 축은 선언 방식 하나 — ModelOpt MIXED 는 층 개수 우세가 보조 규칙)

| 범위 | dtype | 출처 |
|---|---|---|
| 기본 dtype(비양자화 가중치·계산) | `bfloat16` | /app/models/Google/Gemma_4/gemma-4-26B-A4B-it → <manifest.nas_model_path>/Google/Gemma_4/gemma-4-26B-A4B-it/config.json dtype |

**이 모델에 필요한 패치**: 실린 빌드 패치 없음(또는 파일 기록 미관측 — 01 §1.1).
<!-- /FACT:resolved -->

> 이 절이 답하는 질문: 위 값들 중 무엇을 그대로 참조하고 무엇을 네 환경에서 다시 도출해야 하며, 이 빌드 트랙은 왜 골랐는가?

source-build는NGCalpha torch와prebuiltwheel의ABI벽에따른공유이미지선택이다(plan_26101410 §3). 빌드입력은v0.31.0또는FACT의40자SHA이며이미지태그·실제엔진dev자기보고·wheel메타를구분한다. 실린Dockerfile의generated-unverified경고와requirements검증지위는01 §1.1정본이다. 이번새이미지빌드는없다.

arm64/TORCH_CUDA_ARCH12.1a는GB10빌드좌표다. 다른CPU/GPU에서는manifest와지원target을다시대조한다. driver580.178.04·hostCUDA132는manifest선언이고imageCUDA13.3.1.008·torch2.13.0a0+9186a08는이미지관측이다. hostOS는이자료에서해소하지않았다. 실린requirements제약flashinfer-python==0.7.0.post1 · humming-kernels[cu12]==0.1.16 · transformers<5.18.0,>=5.10.4,<5.15.0는설치버전새실측이아니다. Ray2.48.0은공유멀티지원의존이지TP1모델고유필수조건이아니다.

## 0.5 서빙 노브와 값의 지위
> 01-artifacts.md §1.3 의 `kind: value-status` 블록에서 기계가 생성한 요약이다. 지위가 `tuned` · `declared-requirement` 가 아닌 값은 **이 셀에서 조정된 적도, 빼면 깨진다고 확인된 적도 없다** — 필요조건으로 복사하지 마라. `declared-requirement` 는 빼거나 바꾸면 실패가 관측된 값이다(근거의 W id · artifacts 헤더를 보고 네 환경에서도 그 실패 조건이 성립하는지 판단한다).

<!-- FACT:knob_status -->
노브 4개 — `inherited` 3 · `declared-requirement` 1.

| 노브 | 값 | 지위 | 근거 | id |
|---|---|---|---|---|
| `gpu-memory-utilization` | 0.85 | inherited | yamlgmu_source=hand와과거safety_margin승계; 이번레버대조없음; 절대KV예약산정과구분 | V1 |
| `max-model-len` | 262144 | inherited | config네이티브최대길이승계; YaRN선언없어확장미시도; 과거needle240932토큰과이번1024부하구분 | V2 |
| `kv-cache-memory-bytes` | 3368402176 | declared-requirement | 과거클램프null은KV52.89GiB뒤memwatch종료, 클램프존재후통과(W1); 정확바이트값은262144×batch1파생이며다른값대조없음 | V3 |
| `kv-cache-dtype` | fp8 | inherited | 과거레시피후보값승계; auto대조없고fp8정확도검증미수행 | V4 |
<!-- /FACT:knob_status -->

> 이 절이 답하는 질문: 이 레시피를 다른 환경으로 옮길 때 어떤 노브를 먼저 다시 재야 하고, 어떤 노브는 빼면 깨지는가?

① KV절대바이트·gmu·목표길이는대상총량과hybrid KV식·목표동시요청에서다시도출한다. fp8은정확도대조를별도요구한다.
② V3의declared-requirement는과거클램프제거시memwatch종료대조로확인한클램프존재다. 정확3.14GiB의보편최소값/최적값증명이아니다.
③ gmu·native길이·KVdtype는승계했다. 이번엔서빙노브조정없이동일형상의신규Full을검증했으며이전needle나전용parser스모크를새실행으로세지않는다.

## 0.6 재검증 · 라우팅
- 이 자료를 `vllm-recipe-explorer` 인터뷰의 **warm-start 근거로만** 투입하고, 평소대로 plan-gate → 레시피 수렴 → 스모크 게이트를 그대로 밟아라(가속이지 우회 ✗).

> 이 절이 답하는 질문: 다른 환경(다른 HW · 다른 vLLM 버전 · 다른 노드 수)에서 이 지도를 쓸 때 무엇부터, 어떤 순서로 확인해야 하며, 실패하면 어디로 돌리는가?

1. 모델revision·BF16weights·hybrid층/head_dim·native길이를00 §0.2와대조한다 → 다른형상이면KV식을다시도출한다.
2. image원장·source핀·requirements·실린Dockerfile검증경고를읽는다 → 상위버전에서는공유build호환성과inline패치skip조건을다시확인한다.
3. host예산과KV클램프를선언하고공식기동한다 → 내부8000/호스트8912매핑과health200+실제completion을확인한다(W1·W2).
4. 필요한기능은capability를선언해별도시험한다 → 이번completion과과거전용tool/reasoning·needle시험을구분한다.
5. lite→GuideLLM각3회·fresh키·실제PASS·posthealth·오류·E검색조건을확인한다 → 다른세션수치로대신하지않는다.

빌드/버전은upstream-version-watch,서빙노브는vllm-recipe-explorer,성능판정은adversarial-benchmark,노드/토폴로지는terraforming_node가담당한다. 위는재검증경로설명이며자동실행지시가아니다.

## 0.7 메타
<!-- FACT:meta -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.31.0/gemma-4-26b-a4b-it/gb10-1g1n-main-native/qbf16-len262144-kvfp8-t2610271153` |
| 형식 | `hint-payload/v7` |
| 앵커 | `PROVENANCE.json` — 봉인 때 hint 브랜치 페이로드 커밋에 묶인다 |
| 캠페인 · 셀 · 노드 | camp-26102705-codex-single8 · gemma26b-ctx-single · main |
| 발행 모드 | campaign |
| 발행 토픽 | camp_26102705_codex_single8__main__gemma26b_ctx_single |
| work-manifest | docs/_evidence/camp_26102705_codex_single8__main__gemma26b_ctx_single.work-manifest.json |
| 증거 등급(task_class) | full_benchmark |
| 승인 출처 | campaign:hint_targets |
| 승인 시각(UTC) | 2026-10-26T21:47:32Z |
| 승인 발화(전사) | 사용자: 싱글 메인·서브 각각 서로 다른 4개 모델 총 8개 고유 모델, 모든 모델 Full bench 성공 후 hint 태그 발행까지 수행 |
| 현행 full 정의 충족(측정 등급) | 예 · 이 측정의 반복 3 · 정의 출처 `.claude/rules/docs.md:42`(03 §3.5) |
| 독립 사실 검증(발행 전) | 통과 — 검증자 `layering_adversarial_review (independent Gemma hint fact checker)`(저작자 `campaign_inventory (Codex gpt-6.1-sol, main serving/evidence author)` 아님) · 주장 981건 · 지적 0건 중 고침 0건 · 열림 0 · 2026-10-27T04:00:32Z |
<!-- /FACT:meta -->

<!-- FACT:missing -->
**결손**: _없음 — 결손 코드표를 전수 대조한 결과 0건이다(침묵은 '없음' 과 구분되지 않는다 — 없음도 적는다)._
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
**확정 이름** `hint/0.31.0/gemma-4-26b-a4b-it/gb10-1g1n-main-native/qbf16-len262144-kvfp8-t2610271153` — 꼬리 0토큰 · timestamp `t2610271153`.

_빈 꼬리 — 발행 Agent 가 가르는 노브가 없다고 명시했다(`inputs/tail.json` = `[]`)._

- `t2610271153` = 같은 이름(결정론부 + 꼬리)이 원격 · 로컬에 이미 있어 붙인 발행 시각(generated_utc 의 KST `YYMMDDHHMM`) — 같은 셀의 새 판이다(옛 태그는 교정하지 않는다).
<!-- /FACT:name_tail -->

> 이 절이 답하는 질문: 이 셀을 같은 q·len·kv 의 다른 셀과 가르는 노브는 무엇이며 그것을 이름 꼬리로 어떻게 적는가 — 가르는 노브가 없으면 왜 빈 꼬리인가?

inputs/tail.json은빈목록이다. 같은BF16·길이·KVfp8형상의이전셀과구분할새serving노브가없어과거꼬리를복사하지않았다. 이번차이는신규측정과외부E근거이며03에서별도기재한다. 충돌새판시각은공식도구가붙인다.
