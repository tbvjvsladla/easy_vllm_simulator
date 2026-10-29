# 00 · hint 지도 — 가장 먼저 읽는 파일

<!-- FACT:header -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.32.0-g5df63a7203cd/solar-open2-250b-nota-nvfp4/gb10-1g2n-cluster-native/qnvfp4-len262144-kvfp8` |
| 형식 | `hint-payload/v7` · 이름 문법 `v7` |
| 생성(UTC · 주입) | 2026-10-29T16:10:45Z |
| 캠페인 · 셀 · 노드 · 모드 | camp-26102919-codex-multi4-v032 · solar032-nv4-safe-tp2 · cluster · campaign |
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
| 자격 근거 | `output/multi/benchlog/serve_proof_solar032-nv4-safe-tp2.json` — 시점 범위: 작성 2026-10-29T15:15:54Z(output/multi/benchlog/serve_proof_solar032-nv4-safe-tp2.json 파일 mtime(파일시스템 관측 — 문서에 시각 칸이 없다 · 기록 필드 아님)) · 측정 창 대비 작성 2026-10-29T15:15:54Z < 측정 시작 2026-10-29T15:19:18Z — 측정 전(빌드·스모크)의 관측 |
| 증거 등급(task_class) | full_benchmark |
| 측정 등급(bench_mode) | full |
| 셀 등급(cell_grade) | `full` — 성능 판정 · 부하 곡선 · 데이터셋 출처는 `03-benchmark.md` |
| 성능 판정(measurement.verdict) | `PASS` — 출처 output/multi/benchlog/sweep_solar032-nv4-safe-tp2_261030A/verdict.json(output · verdict_rule 출력) verdict |
<!-- /FACT:grade -->

<!-- FACT:lite_summary -->
<!-- /FACT:lite_summary -->

## 0.1 요약
> 이 절이 답하는 질문: 이 셀은 무엇을 서빙했고, 무엇이 결정적이었으며, 수신자는 무엇을 가장 조심해야 하는가?

Solar-Open2-250B-Nota-NVFP4를 GB10 노드당GPU하나×두노드 TP2, 공식vLLM0.32.0 exactSHA 입력에서 NVFP4·262144·KV fp8_e4m3·PLE없음·specoff·eager로 측정했고 동시성1 decode16.65t/s의 weak PASS다. stock 모델/파서 부재와 V2·배달 접점은 새런타임 플러그인으로 재확정했다(W1~W7); CPU검사와배달provenance를 GPU성공으로세지않았다(W9/W10). KV와context는손레버승계값(V6/V8)이며 전체길이품질·최적성·외부E달성을뜻하지않는다.

## 0.2 유효맥락
<!-- FACT:context -->
| 항목 | 값 | 출처 |
|---|---|---|
| 모델 | solar-open2-250b-nota-nvfp4 | sweep_index.meta(output/multi/benchlog/sweep_solar032-nv4-safe-tp2_261030A) |
| HF repo | nota-ai/Solar-Open2-250B-Nota-NVFP4 | 체크포인트 .git/config remote url(huggingface.co · /app/quant_models/Upstage/Solar-Open2-250B-Nota-NVFP4 → <manifest.quant_model_path>/Upstage/Solar-Open2-250B-Nota-NVFP4) |
| HF revision(체크포인트 git HEAD) | de88f6226788077e2d340204fd79d37720c9eda0 | 체크포인트 .git HEAD → refs/heads/main → loose ref(/app/quant_models/Upstage/Solar-Open2-250B-Nota-NVFP4 → <manifest.quant_model_path>/Upstage/Solar-Open2-250B-Nota-NVFP4) · refs/remotes/origin/main 와 일치(받아 온 원격 커밋) — checkout 커밋(작업트리 일치는 미검증) · reflog: 측정 시점 checkout = 지금 HEAD(measured_utc 2026-10-29T15:34:50Z 이전 마지막 이동 2026-07-24T04:28:17Z) |
| base_model | upstage/Solar-Open2-250B | 체크포인트 README base_model(/app/quant_models/Upstage/Solar-Open2-250B-Nota-NVFP4 → <manifest.quant_model_path>/Upstage/Solar-Open2-250B-Nota-NVFP4) |
| base 슬러그 | solar-open2-250b-nota | model(solar-open2-250b-nota-nvfp4) − vocab quant_suffixes(naming.base_slug) |
| 양자화 | nvfp4 | naming-axis(q) — 측정 identity quantization 미관측(sweep_index.meta(output/multi/benchlog/sweep_solar032-nv4-safe-tp2_261030A)) · q 축 = /app/quant_models/Upstage/Solar-Open2-250B-Nota-NVFP4 → <manifest.quant_model_path>/Upstage/Solar-Open2-250B-Nota-NVFP4/config.json quantization_config quant_method=compressed-tensors format=nvfp4-pack-quantized · vocab quant[nvfp4]←'compressed-tensors:nvfp4-pack-quantized' — 인증서 quantization 이 N/A 라 명명 축 q(체크포인트 config 파생)를 옮겼다 |
| GPU | NVIDIA GB10 | sweep_index.meta(output/multi/benchlog/sweep_solar032-nv4-safe-tp2_261030A) |
| 토폴로지 · TP | multi · TP=2 | sweep_index.meta(output/multi/benchlog/sweep_solar032-nv4-safe-tp2_261030A) |
| 실행 평면 | docker | artifacts.plane_of |
| vLLM(측정 강식별 키 `vllm_version`) | 0.32.0 | sweep_index.meta(output/multi/benchlog/sweep_solar032-nv4-safe-tp2_261030A) |

**명명 축** — 태그 이름 = 결정론부(vllm · model · arch · q · len · kv — 도구가 증거에서 파생 · 발행자 입력 ✗) + 꼬리(발행 Agent 가 고르고 서빙 설정 file · key · value 로 근거 대조 · 0.9) + 중복 시 timestamp.

| 축 | 값 | 출처 |
|---|---|---|
| 세그먼트 `vllm` | 0.32.0-g5df63a7203cd | track=선택자 Dockerfile.source-build-deps2 · ref=docker history build-arg VLLM_REF · version=VLLM_VERSION 미관측 · repo=docker history build-arg VLLM_REPO · sha=VLLM_REF(40자 커밋 · docker history build-arg VLLM_REF) · exact_tag=measured image sha256:8227e7d79471b6b92f21157acfb38c7cc617d1510a5c56f3dec7eb8e48567884: /workspace/vllm-src/.git HEAD and exact tag peeled to 5df63a7203cdb9328d824a27b682e77c7b9928be (create/cp, no start) · VLLM_REF=5df63a7203cd…(40자 커밋) → <직전 릴리스 0.32.0>-g<sha12> |
| 세그먼트 `model` | solar-open2-250b-nota-nvfp4 | 서빙 yaml model(output/multi/configs/solar032-nv4-safe-tp2.yaml) · hf_repo=체크포인트 .git/config remote url(huggingface.co · /app/quant_models/Upstage/Solar-Open2-250B-Nota-NVFP4 → <manifest.quant_model_path>/Upstage/Solar-Open2-250B-Nota-NVFP4) · hf_repo(nota-ai/Solar-Open2-250B-Nota-NVFP4) 마지막 성분 소문자 |
| 세그먼트 `arch` | gb10-1g2n-cluster-native | derived(hw·gpus_per_node·nodes·role·target·plane) |
| 세그먼트 `recipe` | qnvfp4-len262144-kvfp8 | derived(q·len·kv) |
| 축 `hw` | gb10 | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(자동 파생 — 측정 TP=2(sweep_index.meta.tensor_parallel_size(output/multi/benchlog/sweep_solar032-nv4-safe-tp2_261030A · generated_utc = 측정 키)) > gpus_per_node=1(output/multi/manifest.yaml(manifest_contract 로더)) · 단일 노드 형상일 수 없다 · 일치 신호: 멀티 serve proof output/multi/benchlog/serve_proof_solar032-nv4-safe-tp2.json(kind=multinode_serve_proof) · 노드 정합 attestation output/multi/benchlog/attestation_solar032-nv4-safe-tp2.json) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) · vocab hw[gb10]←'NVIDIA GB10' |
| 축 `gpus_per_node` | 1 | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(자동 파생 — 측정 TP=2(sweep_index.meta.tensor_parallel_size(output/multi/benchlog/sweep_solar032-nv4-safe-tp2_261030A · generated_utc = 측정 키)) > gpus_per_node=1(output/multi/manifest.yaml(manifest_contract 로더)) · 단일 노드 형상일 수 없다 · 일치 신호: 멀티 serve proof output/multi/benchlog/serve_proof_solar032-nv4-safe-tp2.json(kind=multinode_serve_proof) · 노드 정합 attestation output/multi/benchlog/attestation_solar032-nv4-safe-tp2.json) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `nodes` | 2 | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(자동 파생 — 측정 TP=2(sweep_index.meta.tensor_parallel_size(output/multi/benchlog/sweep_solar032-nv4-safe-tp2_261030A · generated_utc = 측정 키)) > gpus_per_node=1(output/multi/manifest.yaml(manifest_contract 로더)) · 단일 노드 형상일 수 없다 · 일치 신호: 멀티 serve proof output/multi/benchlog/serve_proof_solar032-nv4-safe-tp2.json(kind=multinode_serve_proof) · 노드 정합 attestation output/multi/benchlog/attestation_solar032-nv4-safe-tp2.json) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `role` | cluster | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(자동 파생 — 측정 TP=2(sweep_index.meta.tensor_parallel_size(output/multi/benchlog/sweep_solar032-nv4-safe-tp2_261030A · generated_utc = 측정 키)) > gpus_per_node=1(output/multi/manifest.yaml(manifest_contract 로더)) · 단일 노드 형상일 수 없다 · 일치 신호: 멀티 serve proof output/multi/benchlog/serve_proof_solar032-nv4-safe-tp2.json(kind=multinode_serve_proof) · 노드 정합 attestation output/multi/benchlog/attestation_solar032-nv4-safe-tp2.json) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `target` | native | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(자동 파생 — 측정 TP=2(sweep_index.meta.tensor_parallel_size(output/multi/benchlog/sweep_solar032-nv4-safe-tp2_261030A · generated_utc = 측정 키)) > gpus_per_node=1(output/multi/manifest.yaml(manifest_contract 로더)) · 단일 노드 형상일 수 없다 · 일치 신호: 멀티 serve proof output/multi/benchlog/serve_proof_solar032-nv4-safe-tp2.json(kind=multinode_serve_proof) · 노드 정합 attestation output/multi/benchlog/attestation_solar032-nv4-safe-tp2.json) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) · target_gpu 'NVIDIA GB10' = 호스트 hw → native |
| 축 `plane` | docker | artifacts.plane_of → cell-env(IMAGE_TAG\|BUILD_DOCKERFILE) · vocab plane[docker]→'' |
| 축 `q` | nvfp4 | /app/quant_models/Upstage/Solar-Open2-250B-Nota-NVFP4 → <manifest.quant_model_path>/Upstage/Solar-Open2-250B-Nota-NVFP4/config.json quantization_config quant_method=compressed-tensors format=nvfp4-pack-quantized · vocab quant[nvfp4]←'compressed-tensors:nvfp4-pack-quantized' |
| 축 `len` | 262144 | 서빙 yaml max-model-len(output/multi/configs/solar032-nv4-safe-tp2.yaml) |
| 축 `kv` | fp8 | 서빙 yaml kv-cache-dtype(output/multi/configs/solar032-nv4-safe-tp2.yaml) · vocab kv[fp8]←'fp8_e4m3' |
<!-- /FACT:context -->

> 이 절이 답하는 질문: 이 지도는 어떤 조건에서만 성립하며, 이 형상은 어떤 기전으로 버티고 다른 형상은 왜 실패했는가?

(가) GB10통합메모리와노드당GPU하나에서TP2는RoCE를탄다. 모델적재71.6GiB/노드와KV8GiB/노드외호스트overhead까지같은물리메모리에서예산을차지한다(testlog_26103000_Solar032_TP2와_Full_판정). 다른분리VRAM구조나노드수에같은floor를옮길수없다.
NVFP4 checkpoint와비양자화530모듈이혼합돼있으며Marlin을선택했다. 카드의B300TP4조건을GB10TP2로축소환산하지않는다(plan_26102919_solar032_레시피_준비 §모델별 근거). 새hand-port의KDA/V2지원범위도형상전제다.
(나) 현재262k/KV8GiB/seq1/prefill2048/GMU0.80/eager는안전선언과실로드/Full을통과한관측이다. 과거1M/KV14GiB도별도0.31에서성공했으므로실패형상이아니다. 과거400k단발은전력가드,과거bench4096은floor차단이확정사인이고(W5/W8),현재여러설정차이의독립메모리기여는미결이다. CPUoffload나PLEmmap으로UMA의새용량이생긴다고가정하지않는다.

## 0.3 벽 지도 요약
> 02-narrative.md §2.2 의 `kind: wall` 블록(과 §2.4 의 `kind: misdiagnosis` 블록)에서 기계가 생성한 표다(손저작 ✗ · 순서 = 넘은 순서). 원인·해소의 전문과 원문 발췌는 02 에 있다.

<!-- FACT:wall_map -->
| 순서 | id | 증상 | 해소 | 전이등급 | 검증 |
|---|---|---|---|---|---|
| 1 | W1 | 기동 전 registry 조사에서 SolarOpen2 미지원 판정 | fork 델타를 out-of-tree 런타임 플러그인으로 자체이식 | arch-invariant | testlog_26102418_solar_open2_nvfp4_멀티_TP2_1M_자체이식_기동_needle_lite_판정 §1. 아키텍처 미지원 판정과 이식 경로 |
| 2 | W2 | 첫0.31 기동에서 V1 processor를 거부 | 초기에는 processor 제외; plan_26102420 S4에서 V2로 재저작 | arch-invariant | plan_26102420_PLE_mmap_재이식_full벤치_hint_Solar_슬롯화 §정정·진행 기록 (2026-10-25 KST · S2~S4 완료) |
| 3 | W3 | Ray worker가 모듈을 찾지 못함 | 패치쌍 스냅샷과 RUNTIME_PATCH_STEM/arm_patch/slave_forward 배선 교정 | judgment | 역사 testlog_26102418의 후속 기동/needle PASS |
| 4 | W4 | 가중치 적재 중 loader 호출 실패 | KDA subclass에서 tensor속성의 shard id를 읽는 어댑터 | arch-invariant | 역사 testlog_26102418 §3 양노드71.6GiB 적재 |
| 5 | W5 | sub container kill137 | 가드 유지,40k 이후32k/26k/20k/16k 증분 프리필과 재기동 | arch-scaled | 역사 testlog_26102418 §4 최종1001129토큰 needle PASS |
| 6 | W6 | 초기 응답의 Thinking Process가 content에 섞임 | S4에서 V2와 preserved reasoning 경로를 함께 완결 | arch-invariant | plan_26102420 S2~S4 완료 및 현재 Solar032 추론의 content/reasoning 분리 |
| 7 | W7 | 역사 YAML false만으로 동기화되지 않았고 V2가 sync를 요구 | runner CLI --no-async-scheduling, spec 설정 없음 | arch-invariant | 현재 Solar032 health200+추론 및 Full3 |
| 8 | W8 | 과거 Solar Full에서4096MiB BLOCK | 역사 Solar만2048MiB 선언으로 실행 | arch-scaled | testlog_26102504 §2 표2 및 §1 Solar Full |
| 9 | W9 | 유효 import 검사 전 첫 launch가 SyntaxError | 종료식 교정 후 단일 유효 CPU probe | judgment | 7단계 import/registration PASS, GPU노출0 |
| 10 | W10 | 초기 실험해소값 stamp와 정식 sync 정본 불일치 | 정식 tracked canonical baseline과 새모델 입력으로 restamp/verify | judgment | 정식 stamp와 verify PASS; 실제0.32는 sourcepin/로그로 별도검증 |

**오진 · 정정**(02 §2.4 — 현재지위가 `가설(강등)` 인 기전은 처방이 아니다)

| id | 증상 | 현재지위 | 검증 |
|---|---|---|---|
| M1 | 역사 bench4096 BLOCK를 arm_ceiling 초과로 오진 | 반증 | testlog_26102504_S5_full벤치_5셀_GuideLLM_판정_및_하네스_결함 §2. 하네스 결함 · 처방 |
| M2 | 역사 실패한 sweep 뒤 stale PASS | 반증 | testlog_26102504_S5_full벤치_5셀_GuideLLM_판정_및_하네스_결함 §4. 후속 해소 · 정정 |
| M3 | du286G를 실제 가중치로 받아들인 출발 가정 | 반증 | plan_26102412_4모델_멀티캠페인_Solar_Qwen_TP2_최대context §셀 및 testlog_26103000_Solar032_TP2와_Full_판정 |
| M4 | 역사 modeling파일 부재로 HF대조 불가라고 단정 | 반증 | testlog_26102504_S5_full벤치_5셀_GuideLLM_판정_및_하네스_결함 §3. 정정 · hint 발행 결과 |
<!-- /FACT:wall_map -->

## 0.4 결정론 해소값
<!-- FACT:resolved -->
| 항목 | 값 | 출처 |
|---|---|---|
| vLLM 빌드 입력(종류) | commit | naming.vllm_build_input |
| vLLM 빌드 입력(ref) | 5df63a7203cdb9328d824a27b682e77c7b9928be | docker history build-arg VLLM_REF |
| vLLM 빌드 계보 SHA(40자) | 5df63a7203cdb9328d824a27b682e77c7b9928be | VLLM_REF(40자 커밋 · docker history build-arg VLLM_REF) |
| 직전 릴리스(prev_release) | 0.32.0 | naming.vllm_build_input |
| vLLM 엔진 자기보고(엔진 로그 기동 배너) | 0.32.1.dev0+g5df63a720.d20261029 | output/multi/benchlog/sweep_solar032-nv4-safe-tp2_261030A/lite_engine_head_solar032-nv4-safe-tp2.log:61 — 측정 캡처에 기록된 값 · 현재 serve 세션 경계 미관측 — 누적 로그의 기동 epoch 귀속 미검증 · 같은 값의 다른 로그 1건 · 지위 `measurement-capture-unbound` |
| 인증서 `vllm_version`(강한 일치 키) | 미관측 | 생산자 `absent` · 인증서 vllm_version 없음 |
| vLLM 설치 distribution 메타(출처별 관측 · 다운로드 wheel 증명 아님) | 노드별 상이·공통값 미선택(각 값은 출처) | 노드별 상이 — 값을 고르지 않는다: attestation nodes.<n>.vllm_dist_version(output/multi/benchlog/attestation_solar032-nv4-safe-tp2.json · 노드별 관측) — ⚠ 노드별 상이(main=0.32.1.dev0+g5df63a720.d20261029.cu133 · sub=0.32.1.dev0+g5df63a720.d20261028.cu133 · 값 = main) · 작성 2026-10-29T15:15:54Z < 측정 시작 2026-10-29T15:19:18Z — 측정 전(빌드·스모크)의 관측 · 원장 순서로 판별한 같은 기동의 관측(스모크 로그 미보존 · 약한 판별)(판별 근거 = attestation 범위 줄 · 원장 docs/logs/main/events/2026-10.jsonl)(sweep_index.meta.vllm_build='NA' 는 sweep_bench 가 엔진 로그에서 `v<x.y.z…>` 정규식으로 잡는 값이다 → engine_self_report 의 원천) · 지위 `observed-node-difference` |
| 체크포인트 revision(HF · git HEAD) | de88f6226788077e2d340204fd79d37720c9eda0 | 체크포인트 .git HEAD → refs/heads/main → loose ref(/app/quant_models/Upstage/Solar-Open2-250B-Nota-NVFP4 → <manifest.quant_model_path>/Upstage/Solar-Open2-250B-Nota-NVFP4) · refs/remotes/origin/main 와 일치(받아 온 원격 커밋) — checkout 커밋(작업트리 일치는 미검증) · reflog: 측정 시점 checkout = 지금 HEAD(measured_utc 2026-10-29T15:34:50Z 이전 마지막 이동 2026-07-24T04:28:17Z) |
| 빌드 트랙 | source-build | artifacts 선택자(build-ledger.dockerfile) |
| Dockerfile(선택자) | Dockerfile.source-build-deps2 | build-ledger.dockerfile |
| vLLM 저장소 | https://github.com/vllm-project/vllm.git | docker history build-arg VLLM_REPO |
| 이미지 태그 | easy-vllm:0.32.0-cu133-aarch64-source-deps2 | 셀 env(output/multi/envs/.env.solar032-nv4-safe-tp2) IMAGE_TAG |
| 이미지 digest | sha256:8227e7d79471b6b92f21157acfb38c7cc617d1510a5c56f3dec7eb8e48567884 | sweep_index.meta.image_digest(measured(docker inspect .Image)) |
| torch | 2.13.0a0+9186a08 | docker image inspect Config.Env PYTORCH_VERSION |
| CUDA | 13.3.1.008 | docker image inspect Config.Env CUDA_VERSION |
| NGC 베이스 | 26.07 | docker image inspect Config.Env NVIDIA_PYTORCH_VERSION |
| CPU arch | arm64 | docker image inspect Architecture |
| 드라이버 | 580.178.04 | attestation parity.driver(output/multi/benchlog/attestation_solar032-nv4-safe-tp2.json · 노드별 관측 · verdict=equal) — main=sub 일치 · 작성 2026-10-29T15:15:54Z < 측정 시작 2026-10-29T15:19:18Z — 측정 전(빌드·스모크)의 관측 · 원장 순서로 판별한 같은 기동의 관측(스모크 로그 미보존 · 약한 판별)(판별 근거 = attestation 범위 줄 · 원장 docs/logs/main/events/2026-10.jsonl) |
| 드라이버(노드별 관측) | main=580.178.04 · sub=580.178.04 | attestation parity.driver(output/multi/benchlog/attestation_solar032-nv4-safe-tp2.json · 노드별 관측 · verdict=equal) — main=sub 일치 · 작성 2026-10-29T15:15:54Z < 측정 시작 2026-10-29T15:19:18Z — 측정 전(빌드·스모크)의 관측 · 원장 순서로 판별한 같은 기동의 관측(스모크 로그 미보존 · 약한 판별)(판별 근거 = attestation 범위 줄 · 원장 docs/logs/main/events/2026-10.jsonl) |
| ⚠ 드라이버 관측 ≠ 선언 | 충돌 없음 — 관측 580.178.04 = 선언 580.178.04 | 관측: attestation parity.driver(output/multi/benchlog/attestation_solar032-nv4-safe-tp2.json · 노드별 관측 · verdict=equal) — main=sub 일치 · 작성 2026-10-29T15:15:54Z < 측정 시작 2026-10-29T15:19:18Z — 측정 전(빌드·스모크)의 관측 · 원장 순서로 판별한 같은 기동의 관측(스모크 로그 미보존 · 약한 판별)(판별 근거 = attestation 범위 줄 · 원장 docs/logs/main/events/2026-10.jsonl) · 선언: sweep_index.meta.driver_version(output/multi/benchlog/sweep_solar032-nv4-safe-tp2_261030A · 측정 시 조립 — sweep_bench.sh 가 manifest 에서 읽는다) (manifest 선언 옮김) |
| 호스트 OS | 미관측 | 미관측 |

> 재현 좌표는 빌드 입력(릴리스 태그 또는 40자 SHA)이다 — 엔진 자기보고 · 인증서 · wheel 의 버전 문자열은 빌드 입력이 아니다(각 값을 만든 생산자는 출처 칸).

**양자화 구성**(체크포인트 config 관측 · 이름의 `q` 축은 선언 방식 하나 — ModelOpt MIXED 는 층 개수 우세가 보조 규칙)

| 범위 | dtype | 출처 |
|---|---|---|
| quantization_config 선언 대상(양자화 층 · 제외 목록 밖) | `compressed-tensors` | /app/quant_models/Upstage/Solar-Open2-250B-Nota-NVFP4 → <manifest.quant_model_path>/Upstage/Solar-Open2-250B-Nota-NVFP4/config.json quantization_config quant_method |
| 양자화 제외 모듈 530개 | `unquantized(아래 기본 dtype)` | /app/quant_models/Upstage/Solar-Open2-250B-Nota-NVFP4 → <manifest.quant_model_path>/Upstage/Solar-Open2-250B-Nota-NVFP4/config.json quantization_config ignore·exclude_modules |
| 기본 dtype(비양자화 가중치·계산) | `bfloat16` | /app/quant_models/Upstage/Solar-Open2-250B-Nota-NVFP4 → <manifest.quant_model_path>/Upstage/Solar-Open2-250B-Nota-NVFP4/config.json dtype |

**이 모델에 필요한 패치**(01 §1.1 관련성 · ① 패치 선언 대상 × 모델 config ② 엔진 로그 발화 — 둘 다 관측일 때만 확정): required 0개 · inactive-inferred 0개 · unknown 1개(unknown = 판정 신호 부족 — 필요 여부를 이 표로 단정하지 않는다).
<!-- /FACT:resolved -->

> 이 절이 답하는 질문: 위 값들 중 무엇을 그대로 참조하고 무엇을 네 환경에서 다시 도출해야 하며, 이 빌드 트랙은 왜 골랐는가?

1. 이셀은공식0.32 source-build+NGC26.07 공유이미지를선택했다. 부모core실제Dockerfile/requirements와leafDockerfile의빌드트랙이근거이며,모델전용이미지분기는없다. wheel과의성능우열을실험한것은아니다.
2. 재현좌표는공식v0.32.0의40SHA `5df63a7203cdb9328d824a27b682e77c7b9928be`다. 태그/원장git describe와report키0.32.0은빌드입력·측정메타,SCM0.32.1.dev0+g5df63a720.d20261029는실제패키지생산자표시다(plan_26102919_solar032_레시피_준비 §메인 stock 이미지 CPU import 검사). 노드별날짜차이는다른SHA증거가아니다.
3. arm64/GB10 SM12.1/driver580.178.04는HW결속이다. core실제argv의arch12.1/jobs8을다른GPU에복사하지말고새capability·메모리로도출한다. CUDA13.3.1.008은imageenv이고report cuda132·호스트driver와층이다르다.
4. 선언resolution의전역0.26 hash는runtime배달provenance만결속한다(W10). actualmodelenv/imagehistory는위exactSHA0.32다. Dockerfile history5/10과generated-unverified는그대로남기며선언으로전체바이트동등을증명하지않는다.
5. 호스트OS는미기록,MemTotal124608MiB는러너실측,RoCE는역사계획과현재NCCL로그,driver는노드별attestation관측이다. attestation의같은기동판별강도는범위줄이정본이다.
6. torch2.13.0a0+9186a08은imageenv관측,hub1.33.0/pytest9.2.0은parentPOST05의공유의존교집합,B12X1.5.1은POST06의optional공유설치다. Ray/FlashInfer/Triton/DeepGEMM은원본requirements/Dockerfile/원장에있는핀만재현입력으로읽고Solar고유필수판정을새로붙이지않는다.

## 0.5 서빙 노브와 값의 지위
> 01-artifacts.md §1.3 의 `kind: value-status` 블록에서 기계가 생성한 요약이다. 지위가 `tuned` · `declared-requirement` 가 아닌 값은 **이 셀에서 조정된 적도, 빼면 깨진다고 확인된 적도 없다** — 필요조건으로 복사하지 마라. `declared-requirement` 는 빼거나 바꾸면 실패가 관측된 값이다(근거의 W id · artifacts 헤더를 보고 네 환경에서도 그 실패 조건이 성립하는지 판단한다).

<!-- FACT:knob_status -->
노브 17개 — `inherited` 17.

| 노브 | 값 | 지위 | 근거 | id |
|---|---|---|---|---|
| `tensor-parallel-size` | 2 | inherited | 모델별 계획에서 고른 손레버이며 이 셀의 값 변경 스윕은0 | V1 |
| `distributed-executor-backend` | ray | inherited | 모델별 계획에서 고른 손레버이며 이 셀의 값 변경 스윕은0 | V2 |
| `enable-expert-parallel` | true | inherited | 모델별 계획에서 고른 손레버이며 이 셀의 값 변경 스윕은0 | V3 |
| `moe-backend` | marlin | inherited | 모델별 계획에서 고른 손레버이며 이 셀의 값 변경 스윕은0 | V4 |
| `gpu-memory-utilization` | 0.80 | inherited | 모델별 계획에서 고른 손레버이며 이 셀의 값 변경 스윕은0 | V5 |
| `max-model-len` | 262144 | inherited | 모델별 계획에서 고른 손레버이며 이 셀의 값 변경 스윕은0 | V6 |
| `kv-cache-dtype` | fp8_e4m3 | inherited | 모델별 계획에서 고른 손레버이며 이 셀의 값 변경 스윕은0 | V7 |
| `kv-cache-memory-bytes` | 8589934592 | inherited | 모델별 계획에서 고른 손레버이며 이 셀의 값 변경 스윕은0 | V8 |
| `enforce-eager` | true | inherited | 모델별 계획에서 고른 손레버이며 이 셀의 값 변경 스윕은0 | V9 |
| `max-num-seqs` | 1 | inherited | 모델별 계획에서 고른 손레버이며 이 셀의 값 변경 스윕은0 | V10 |
| `max-num-batched-tokens` | 2048 | inherited | 모델별 계획에서 고른 손레버이며 이 셀의 값 변경 스윕은0 | V11 |
| `mamba-cache-mode` | none | inherited | 모델별 계획에서 고른 손레버이며 이 셀의 값 변경 스윕은0 | V12 |
| `reasoning-parser` | solar_open2 | inherited | 모델별 계획에서 고른 손레버이며 이 셀의 값 변경 스윕은0 | V13 |
| `tool-call-parser` | solar_open2 | inherited | 모델별 계획에서 고른 손레버이며 이 셀의 값 변경 스윕은0 | V14 |
| `enable-auto-tool-choice` | true | inherited | 모델별 계획에서 고른 손레버이며 이 셀의 값 변경 스윕은0 | V15 |
| `logits-processors` | solar_open2_plugin.logits_processor:SolarOpen2TemplateLogitsProcessor | inherited | 역사 W2/W6의 기능 필요 경로를 새0.32 접점 확인 후 선택한 손레버; 이0.32셀에서 제거·변경 실패 대조는 없음 | V16 |
| `default-chat-template-kwargs` | {"think_render_option":"preserved"} | inherited | 모델별 계획에서 고른 손레버이며 이 셀의 값 변경 스윕은0 | V17 |
<!-- /FACT:knob_status -->

> 이 절이 답하는 질문: 이 레시피를 다른 환경으로 옮길 때 어떤 노브를 먼저 다시 재야 하고, 어떤 노브는 빼면 깨지는가?

① max-model-len/max-num-seqs/max-num-batched-tokens/KV바이트/GMU는현재호스트MemTotal−가중치−KV−overhead로안전floor를도출하고엔진KV토큰수와목표길이를대조할값이다. 고정KV는GPU당이며TP로나누지않는다.
② 이0.32셀에서 노브를 빼거나 바꿔 실패한 대조는 없어 declared-requirement는 없다. logits-processors의 역사 W2/W6 필요 경로와 CLI no-async/no-prefix·mamba-cache-mode:none의 소스상 지원 경계(W7)를 현재 제거 실패 실측으로 승격하지 않는다. spec/checkpoint 미지원과 HF 수학 정밀동치 미증명은 별도 한계다.
③ 표의나머지inherited손레버는이셀에서변경스윕하지않았다. eager/Marlin/EP/fp8와262k/seq1/8GiB/2048은시작후통과한형상이지최적값이나음성대조값이아니다.

## 0.6 재검증 · 라우팅
- 이 자료를 `vllm-recipe-explorer` 인터뷰의 **warm-start 근거로만** 투입하고, 평소대로 plan-gate → 레시피 수렴 → 스모크 게이트를 그대로 밟아라(가속이지 우회 ✗).

> 이 절이 답하는 질문: 다른 환경(다른 HW · 다른 vLLM 버전 · 다른 노드 수)에서 이 지도를 쓸 때 무엇부터, 어떤 순서로 확인해야 하며, 실패하면 어디로 돌리는가?

1. 대상exactSHA의SolarOpen2 registry/KDA/GDN/V2와패치헤더접점을확인한다 → 정식지원이생겼으면패치필요성을재판정하고지원이달라졌으면이지도를다시그라운딩한다.
2. 부모5파일context/빈PRE/POST05→alias→POST06와새재현README의표시를확인한다 → 환경핀을도출하고양노드로컬빌드/ABI스모크가필요하다. generated-unverified는실행성공증거가아니다.
3. MemTotal·가중치·KV·overhead로호스트바닥과bench예산을선언한다 → 가드거부면형상을재도출하고가드임계는유지한다.
4. 정식배달patch쌍/workerstem과같은가중치mount를확인한다 → arming/model등록/양rank로드/health200/추론1회를함께본다. CPUimport나.ptharming만으로통과하지않는다.
5. 실제목표부하에서lite/GuideLLM Full과성능루브릭을구분한다 → Eempty 내부weak기준을외부SLA로쓰지않고전체길이/tool품질은별도범위로판단한다.
실패신호별행선지: 빌드·핀은upstream-version-watch,서빙/KV/플러그인접점은vllm-recipe-explorer,성능은adversarial-benchmark,노드/Ray/RoCE는terraforming_node다.

## 0.7 메타
<!-- FACT:meta -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.32.0-g5df63a7203cd/solar-open2-250b-nota-nvfp4/gb10-1g2n-cluster-native/qnvfp4-len262144-kvfp8` |
| 형식 | `hint-payload/v7` |
| 앵커 | `PROVENANCE.json` — 봉인 때 hint 브랜치 페이로드 커밋에 묶인다 |
| 캠페인 · 셀 · 노드 | camp-26102919-codex-multi4-v032 · solar032-nv4-safe-tp2 · cluster |
| 발행 모드 | campaign |
| 발행 토픽 | camp_26102919_codex_multi4_v032__cluster__solar032_nv4_safe_tp2 |
| work-manifest | docs/_evidence/camp_26102919_codex_multi4_v032__cluster__solar032_nv4_safe_tp2.work-manifest.json |
| 증거 등급(task_class) | full_benchmark |
| 승인 출처 | campaign:hint_targets |
| 승인 시각(UTC) | 2026-10-29T09:57:19Z |
| 승인 발화(전사) | 사용자: 마지막 멀티브랜치 4종 캠페인을 공식 vLLM0.32.0으로 고정하고 자율형상/Full3회/hint까지 끝까지 진행 |
| 현행 full 정의 충족(측정 등급) | 예 · 이 측정의 반복 3 · 정의 출처 `.claude/rules/docs.md:42`(03 §3.5) |
| 독립 사실 검증(발행 전) | 통과 — 검증자 `root_independent_factchecker`(저작자 `main_hint_author` 아님) · 주장 124건 · 지적 2건 중 고침 2건 · 열림 0 · 2026-10-29T16:26:02Z |
<!-- /FACT:meta -->

<!-- FACT:missing -->
**결손** — 발행을 막지 않고 기재한다(부재와 실패는 다른 사실이다 · 차단은 양성 검출만).

| 코드 | 뜻 |
|---|---|
| `HINT_MISSING_SWEEP_LEVELS` | 부하 레벨이 1개 이하 — 부하 거동을 알 수 없다(경량 리포트는 동시성 곡선을 재지 않는다). |
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
**확정 이름** `hint/0.32.0-g5df63a7203cd/solar-open2-250b-nota-nvfp4/gb10-1g2n-cluster-native/qnvfp4-len262144-kvfp8` — 꼬리 0토큰.

_빈 꼬리 — 발행 Agent 가 가르는 노브가 없다고 명시했다(`inputs/tail.json` = `[]`)._
<!-- /FACT:name_tail -->

> 이 절이 답하는 질문: 이 셀을 같은 q·len·kv 의 다른 셀과 가르는 노브는 무엇이며 그것을 이름 꼬리로 어떻게 적는가 — 가르는 노브가 없으면 왜 빈 꼬리인가?

빈꼬리를선택했다. plenone/specoff는부재관측이며같은결정론부를가르는특수기능이아니고,eager도기본안전형상으로서이번이름에추가하지않는다. 실제지위와구분조건은본문에남긴다.
