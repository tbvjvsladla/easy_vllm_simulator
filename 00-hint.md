# 00 · hint 지도 — 가장 먼저 읽는 파일

<!-- FACT:header -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.32.0-g5df63a7203cd/qwen3.8-flash-next-nvfp4/gb10-1g2n-cluster-native/qnvfp4-len1048576-kvfp8` |
| 형식 | `hint-payload/v7` · 이름 문법 `v7` |
| 생성(UTC · 주입) | 2026-10-30T13:32:43Z |
| 캠페인 · 셀 · 노드 · 모드 | camp-26103017-nas-nvme-loading · q38-load-local-1m · cluster · campaign |
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
| 자격 근거 | `output/multi/benchlog/serve_proof_q38-load-local-1m.json` — 시점 범위: 작성 2026-10-30T13:01:19Z(output/multi/benchlog/serve_proof_q38-load-local-1m.json 파일 mtime(파일시스템 관측 — 문서에 시각 칸이 없다 · 기록 필드 아님)) · 측정 창 대비 작성 2026-10-30T13:01:19Z < 측정 시작 2026-10-30T13:01:41Z — 측정 전(빌드·스모크)의 관측 |
| 증거 등급(task_class) | full_benchmark |
| 측정 등급(bench_mode) | full |
| 셀 등급(cell_grade) | `full` — 성능 판정 · 부하 곡선 · 데이터셋 출처는 `03-benchmark.md` |
| 성능 판정(measurement.verdict) | `PASS` — 출처 output/multi/benchlog/sweep_loading_q38-load-local-1m/verdict.json(output · verdict_rule 출력) verdict |
<!-- /FACT:grade -->

<!-- FACT:lite_summary -->
<!-- /FACT:lite_summary -->

## 0.1 요약
> 이 절이 답하는 질문: 이 셀은 무엇을 서빙했고, 무엇이 결정적이었으며, 수신자는 무엇을 가장 조심해야 하는가?

Qwen3.8-Flash-Next-NVFP4를 GB10 두 노드·TP2, vLLM 소스5df63a7203cdb9328d824a27b682e77c7b9928be 빌드에서 max-model-len1048576·FP8 KV·resident PLE·MTP off·eager로 서빙했고, local NVMe 대표 full의 concurrency1 decode는21.40t/s다. YaRN 길이 의미와 worker mount 입력을 확인했다(W2/W3). KV와 동시성은 이번 비교에서 고정한 손레버이며 최소 필요값이나 자동 수렴값이 아니다(V6/V9).

## 0.2 유효맥락
<!-- FACT:context -->
| 항목 | 값 | 출처 |
|---|---|---|
| 모델 | qwen3.8-flash-next-nvfp4 | sweep_index.meta(output/multi/benchlog/sweep_loading_q38-load-local-1m) |
| HF repo | nvidia/Qwen3.8-Flash-Next-NVFP4 | 체크포인트 .git/config remote url(huggingface.co · /app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4) |
| HF revision(체크포인트 git HEAD) | fc694b54fb0174e0913e6adf86691ef85a4ead47 | 체크포인트 .git HEAD → refs/heads/main → loose ref(/app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4) · refs/remotes/origin/main 와 일치(받아 온 원격 커밋) — checkout 커밋(작업트리 일치는 미검증) · reflog: 측정 시점 checkout = 지금 HEAD(measured_utc 2026-10-30T13:14:33Z 이전 마지막 이동 2026-09-08T10:37:13Z) |
| base_model | Qwen/Qwen3.8-Flash-Next | 체크포인트 README base_model(/app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4) |
| base 슬러그 | qwen3.8-flash-next | model(qwen3.8-flash-next-nvfp4) − vocab quant_suffixes(naming.base_slug) |
| 양자화 | nvfp4 | naming-axis(q) — 측정 identity quantization 미관측(sweep_index.meta(output/multi/benchlog/sweep_loading_q38-load-local-1m)) · q 축 = /app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4/config.json quantization_config(modelopt MIXED_PRECISION) quantized_layers 우세 NVFP4 48/50 · vocab quant[nvfp4]←'modelopt-dominant:NVFP4' — 인증서 quantization 이 N/A 라 명명 축 q(체크포인트 config 파생)를 옮겼다 |
| GPU | NVIDIA GB10 | sweep_index.meta(output/multi/benchlog/sweep_loading_q38-load-local-1m) |
| 토폴로지 · TP | multi · TP=2 | sweep_index.meta(output/multi/benchlog/sweep_loading_q38-load-local-1m) |
| 실행 평면 | docker | artifacts.plane_of |
| vLLM(측정 강식별 키 `vllm_version`) | 0.32.0 | sweep_index.meta(output/multi/benchlog/sweep_loading_q38-load-local-1m) |

**명명 축** — 태그 이름 = 결정론부(vllm · model · arch · q · len · kv — 도구가 증거에서 파생 · 발행자 입력 ✗) + 꼬리(발행 Agent 가 고르고 서빙 설정 file · key · value 로 근거 대조 · 0.9) + 중복 시 timestamp.

| 축 | 값 | 출처 |
|---|---|---|
| 세그먼트 `vllm` | 0.32.0-g5df63a7203cd | track=선택자 Dockerfile.source-build-sm12x · ref=docker history build-arg VLLM_REF · version=VLLM_VERSION 미관측 · repo=docker history build-arg VLLM_REPO · sha=VLLM_REF(40자 커밋 · docker history build-arg VLLM_REF) · exact_tag=measured image sha256:436bd187022965d2e41f8969695d8446baeca6c5d50692d8b6db76ae2877a916: /workspace/vllm-src/.git HEAD and exact tag peeled to 5df63a7203cdb9328d824a27b682e77c7b9928be (create/cp, no start) · VLLM_REF=5df63a7203cd…(40자 커밋) → <직전 릴리스 0.32.0>-g<sha12> |
| 세그먼트 `model` | qwen3.8-flash-next-nvfp4 | 서빙 yaml model(output/multi/configs/q38-load-local-1m.yaml) · hf_repo=체크포인트 .git/config remote url(huggingface.co · /app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4) · hf_repo(nvidia/Qwen3.8-Flash-Next-NVFP4) 마지막 성분 소문자 |
| 세그먼트 `arch` | gb10-1g2n-cluster-native | derived(hw·gpus_per_node·nodes·role·target·plane) |
| 세그먼트 `recipe` | qnvfp4-len1048576-kvfp8 | derived(q·len·kv) |
| 축 `hw` | gb10 | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(자동 파생 — 측정 TP=2(sweep_index.meta.tensor_parallel_size(output/multi/benchlog/sweep_loading_q38-load-local-1m · generated_utc = 측정 키)) > gpus_per_node=1(output/multi/manifest.yaml(manifest_contract 로더)) · 단일 노드 형상일 수 없다 · 일치 신호: 멀티 serve proof output/multi/benchlog/serve_proof_q38-load-local-1m.json(kind=multinode_serve_proof) · 노드 정합 attestation output/multi/benchlog/attestation_q38-load-local-1m.json) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) · vocab hw[gb10]←'NVIDIA GB10' |
| 축 `gpus_per_node` | 1 | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(자동 파생 — 측정 TP=2(sweep_index.meta.tensor_parallel_size(output/multi/benchlog/sweep_loading_q38-load-local-1m · generated_utc = 측정 키)) > gpus_per_node=1(output/multi/manifest.yaml(manifest_contract 로더)) · 단일 노드 형상일 수 없다 · 일치 신호: 멀티 serve proof output/multi/benchlog/serve_proof_q38-load-local-1m.json(kind=multinode_serve_proof) · 노드 정합 attestation output/multi/benchlog/attestation_q38-load-local-1m.json) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `nodes` | 2 | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(자동 파생 — 측정 TP=2(sweep_index.meta.tensor_parallel_size(output/multi/benchlog/sweep_loading_q38-load-local-1m · generated_utc = 측정 키)) > gpus_per_node=1(output/multi/manifest.yaml(manifest_contract 로더)) · 단일 노드 형상일 수 없다 · 일치 신호: 멀티 serve proof output/multi/benchlog/serve_proof_q38-load-local-1m.json(kind=multinode_serve_proof) · 노드 정합 attestation output/multi/benchlog/attestation_q38-load-local-1m.json) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `role` | cluster | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(자동 파생 — 측정 TP=2(sweep_index.meta.tensor_parallel_size(output/multi/benchlog/sweep_loading_q38-load-local-1m · generated_utc = 측정 키)) > gpus_per_node=1(output/multi/manifest.yaml(manifest_contract 로더)) · 단일 노드 형상일 수 없다 · 일치 신호: 멀티 serve proof output/multi/benchlog/serve_proof_q38-load-local-1m.json(kind=multinode_serve_proof) · 노드 정합 attestation output/multi/benchlog/attestation_q38-load-local-1m.json) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `target` | native | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(자동 파생 — 측정 TP=2(sweep_index.meta.tensor_parallel_size(output/multi/benchlog/sweep_loading_q38-load-local-1m · generated_utc = 측정 키)) > gpus_per_node=1(output/multi/manifest.yaml(manifest_contract 로더)) · 단일 노드 형상일 수 없다 · 일치 신호: 멀티 serve proof output/multi/benchlog/serve_proof_q38-load-local-1m.json(kind=multinode_serve_proof) · 노드 정합 attestation output/multi/benchlog/attestation_q38-load-local-1m.json) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) · target_gpu 'NVIDIA GB10' = 호스트 hw → native |
| 축 `plane` | docker | artifacts.plane_of → cell-env(IMAGE_TAG\|BUILD_DOCKERFILE) · vocab plane[docker]→'' |
| 축 `q` | nvfp4 | /app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4/config.json quantization_config(modelopt MIXED_PRECISION) quantized_layers 우세 NVFP4 48/50 · vocab quant[nvfp4]←'modelopt-dominant:NVFP4' |
| 축 `len` | 1048576 | 서빙 yaml max-model-len(output/multi/configs/q38-load-local-1m.yaml) |
| 축 `kv` | fp8 | 서빙 yaml kv-cache-dtype(output/multi/configs/q38-load-local-1m.yaml) · vocab kv[fp8]←'fp8_e4m3' |
<!-- /FACT:context -->

> 이 절이 답하는 질문: 이 지도는 어떤 조건에서만 성립하며, 이 형상은 어떤 기전으로 버티고 다른 형상은 왜 실패했는가?

GB10 노드마다 GPU 하나이므로 TP2는 노드 간 인터커넥트를 사용하고 각 노드가 모델 파일을 읽는다(현 testlog Runtime 및 계측 판정). 통합메모리 예산은 가중치·KV·호스트·벤치 도구 몫을 함께 고려해야 한다. 다른 메모리 구조에서는 같은 절대KV·gmu를 재도출한다(현 devlog 계측과 지위).
NVFP4 이름은 모든 텐서가 한 정밀도라는 뜻이 아니며 native mixed composition과 실제 loader를 기준으로 읽는다(M1). PLE resident는 CPU offload false의 고정 형상이다. 과거262K resident에서는 KV축소만으로 실패가 해소되지 않고 gmu도 바뀌었으므로 단일노브 인과는 미결이다(W1).
선행0.31 YaRN의 text max_position 동시 override 실패/교정은 관측됐고 현 이미지에서도1M 설정·짧은 추론을 재확인했다(W2). 선행0.32 B12X 형상의 ETA중단은 다른 context·KV·MTP 조합이며 이번 eager resident 형상의 제거 대조가 아니다. 상세 기전·설정 차이는02와 선행 devlog_26103000에 남긴다.

## 0.3 벽 지도 요약
> 02-narrative.md §2.2 의 `kind: wall` 블록(과 §2.4 의 `kind: misdiagnosis` 블록)에서 기계가 생성한 표다(손저작 ✗ · 순서 = 넘은 순서). 원인·해소의 전문과 원문 발췌는 02 에 있다.

<!-- FACT:wall_map -->
| 순서 | id | 증상 | 해소 | 전이등급 | 검증 |
|---|---|---|---|---|---|
| 1 | W6 | P0-1 1차 예산/KV 선언 차단 | 선언과 클램프를 넣은 후 다음 회차 진행 | arch-scaled | testlog_26090921_p0-1_nvfp4_mixed_패치_스모크_판정의 해당 회차 기록; 현재 셀 결과와 분리 |
| 2 | W7 | P0-1 2차 Triton NVFP4 미지원 | 해당 backend 명시를 버리고 후속 후보 진행 | arch-scaled | testlog_26090921_p0-1_nvfp4_mixed_패치_스모크_판정의 해당 회차 기록; 현재 셀 결과와 분리 |
| 3 | W8 | P0-1 3차 explicit CUTLASS 가드 | 당시 auto dispatch로 전환; 현재 stock에 같은금지라고 일반화하지 않음 | arch-scaled | testlog_26090921_p0-1_nvfp4_mixed_패치_스모크_판정의 해당 회차 기록; 현재 셀 결과와 분리 |
| 4 | W9 | P0-1 4차 graph capture 사살 | 당시5차 eager에서 health/chat 통과 | arch-scaled | testlog_26090921_p0-1_nvfp4_mixed_패치_스모크_판정의 해당 회차 기록; 현재 셀 결과와 분리 |
| 5 | W10 | P0-1 attestation unbound 변수 | 당시 multi 리터럴3곳 교정 | arch-scaled | testlog_26090921_p0-1_nvfp4_mixed_패치_스모크_판정의 해당 회차 기록; 현재 셀 결과와 분리 |
| 6 | W11 | P0-2 Sequence 범위 접근 예외 | rng[0]/rng[1]로 교정 후 당시 smoke통과 | arch-invariant | testlog_26091001_p0-2_ple_mmap_p0-3_qsa_fp8kv_스모크_판정의 해당 회차 기록; 현재 셀 결과와 분리 |
| 7 | W12 | P0-3 eager 누락 graph 사살 | eager 추가 후 당시 smoke통과 | arch-scaled | testlog_26091001_p0-2_ple_mmap_p0-3_qsa_fp8kv_스모크_판정의 해당 회차 기록; 현재 셀 결과와 분리 |
| 8 | W19 | 과거262K resident 20GiB KV 로드피크 사살 | 후속B1-a에서KV8192MiB로 줄였으나 다시사살; 별도W20 | arch-scaled | testlog_26091304_qwen38fn_res최소조건_262k재발행_판정의 해당 회차 기록; 현재 셀 결과와 분리 |
| 9 | W20 | 과거262K B1-a KV8192 후에도 사살 | B1-b는seq8/batched2048 억제 후최저10630MiB 생존; B1-c는gmu0.85→0.80도함께바꿔16192MiB; 복수변경인과를보존 | arch-scaled | testlog_26091304_qwen38fn_res최소조건_262k재발행_판정의 해당 회차 기록; 현재 셀 결과와 분리 |
| 10 | W1 | 과거 resident 메모리 실패를 불가능으로 일반화하지 않음 | 과거 gmu와 KV 조정 이력을 참고하되 현재 1M recipe를 실제 재검증 | arch-scaled | 현재 startup9/9와 full3 |
| 11 | W13 | 0.30 M7 최초 빌드 elfutils 헤더 부재 | 승인 후 libdw-dev 추가·재배달·양 노드 재빌드 PASS | arch-invariant | testlog_26100109_v030_멀티_q38fn_fullbench_서브지침배달_판정의 해당 회차 기록; 현재 셀 결과와 분리 |
| 12 | W14 | 0.31 최초 서브 compose 통로 부재 | 당시 승인된 sync_to_sub dryrun/apply로 통로 해소 | arch-invariant | testlog_26102414_q38fn_nv4_멀티_TP2_1M_YaRN_기동_needle_lite_판정의 해당 회차 기록; 현재 셀 결과와 분리 |
| 13 | W2 | 긴 컨텍스트 override 의미 변경 | factor4와 text max_position 동시 override | arch-scaled | 현재 세 정책 maxlen1048576 +short inference |
| 14 | W21 | 0.31 첫 full 패스가 vLLM 도구로 오측정 | 명시적--tool guidellm으로 재측정; 현0.32A와 별개역사 | arch-invariant | testlog_26102504_S5_full벤치_5셀_GuideLLM_판정_및_하네스_결함의 해당 회차 기록; 현재 셀 결과와 분리 |
| 15 | W5 | 벤치 도구·서빙 메모리 예산을 별도로 고려 | 현재 명시적 --tool guidellm과 actual full 기록 | arch-invariant | 현재 bench_mode full 및 세 measured run |
| 16 | W15 | 0.32 후보D FP8 MTP CUTLASS 생성 실패 | target와 draft를 분리하고 draft TRITON 실제 선례로 정정; 현 셀은MTP off | arch-locked | devlog_26103000_Qwen032_경로탐색과_B_인계의 해당 회차 기록; 현재 셀 결과와 분리 |
| 17 | W16 | 0.32 후보E 메모리 안전중단 | F에서 context/seq/chunk/KV를 함께 줄였으나 그 변화만으로 성공하지 않음 | arch-scaled | devlog_26103000_Qwen032_경로탐색과_B_인계의 해당 회차 기록; 현재 셀 결과와 분리 |
| 18 | W17 | 0.32 후보F 메모리 안전중단 | G에서 공식cutover128을 별도 시도했으나 ETA가 중단 | arch-scaled | devlog_26103000_Qwen032_경로탐색과_B_인계의 해당 회차 기록; 현재 셀 결과와 분리 |
| 19 | W18 | 0.32 후보G ETA 선제 사살 | H에서 native b12x/text-only/context/seq/KV등 복수변경 후 기능과Full; 단일노브 인과아님 | arch-scaled | devlog_26103000_Qwen032_경로탐색과_B_인계의 해당 회차 기록; 현재 셀 결과와 분리 |
| 20 | W3 | worker mount 선택의 project env 우선 | project QUANT_MODEL_PATH 변경과 양 노드 runtime mount 확인 | arch-invariant | 세 정책 실제 runtime_model_mount_verified |
| 21 | W4 | calibration 중 실행 Bash 편집 | canonical runner 동결과 각 회차 스냅샷 | arch-invariant | 9개 accepted start에서 runner unchanged |

**오진 · 정정**(02 §2.4 — 현재지위가 `가설(강등)` 인 기전은 처방이 아니다)

| id | 증상 | 현재지위 | 검증 |
|---|---|---|---|
| M1 | 초기 모델 클래스 import probe의 잘못된 심벌 | 확증 | devlog_26103021_Qwen_1M_로딩비교와_대표_NVMe_full_bench §실제로 넘은 벽 |
<!-- /FACT:wall_map -->

## 0.4 결정론 해소값
<!-- FACT:resolved -->
| 항목 | 값 | 출처 |
|---|---|---|
| vLLM 빌드 입력(종류) | commit | naming.vllm_build_input |
| vLLM 빌드 입력(ref) | 5df63a7203cdb9328d824a27b682e77c7b9928be | docker history build-arg VLLM_REF |
| vLLM 빌드 계보 SHA(40자) | 5df63a7203cdb9328d824a27b682e77c7b9928be | VLLM_REF(40자 커밋 · docker history build-arg VLLM_REF) |
| 직전 릴리스(prev_release) | 0.32.0 | naming.vllm_build_input |
| vLLM 엔진 자기보고(엔진 로그 기동 배너) | 0.32.1.dev0+g5df63a720.d20261029 | output/multi/benchlog/sweep_loading_q38-load-local-1m/lite_engine_head_q38-load-local-1m.log:60 — 측정 캡처에 기록된 값 · 현재 serve 세션 경계 미관측 — 누적 로그의 기동 epoch 귀속 미검증 · 같은 값의 다른 로그 1건 · 지위 `measurement-capture-unbound` |
| 인증서 `vllm_version`(강한 일치 키) | 미관측 | 생산자 `absent` · 인증서 vllm_version 없음 |
| vLLM 설치 distribution 메타(출처별 관측 · 다운로드 wheel 증명 아님) | 노드별 상이·공통값 미선택(각 값은 출처) | 노드별 상이 — 값을 고르지 않는다: attestation nodes.<n>.vllm_dist_version(output/multi/benchlog/attestation_q38-load-local-1m.json · 노드별 관측) — ⚠ 노드별 상이(main=0.32.1.dev0+g5df63a720.d20261029.cu133 · sub=0.32.1.dev0+g5df63a720.d20261028.cu133 · 값 = main) · 작성 2026-10-30T13:01:19Z < 측정 시작 2026-10-30T13:01:41Z — 측정 전(빌드·스모크)의 관측 · 원장 순서로 판별한 같은 기동의 관측(결속된 스모크 로그 미확인 · 제한검색 · 약한 판별)(판별 근거 = attestation 범위 줄 · 원장 docs/logs/main/events/2026-10.jsonl)(sweep_index.meta.vllm_build='NA' 는 sweep_bench 가 엔진 로그에서 `v<x.y.z…>` 정규식으로 잡는 값이다 → engine_self_report 의 원천) · 지위 `observed-node-difference` |
| 체크포인트 revision(HF · git HEAD) | fc694b54fb0174e0913e6adf86691ef85a4ead47 | 체크포인트 .git HEAD → refs/heads/main → loose ref(/app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4) · refs/remotes/origin/main 와 일치(받아 온 원격 커밋) — checkout 커밋(작업트리 일치는 미검증) · reflog: 측정 시점 checkout = 지금 HEAD(measured_utc 2026-10-30T13:14:33Z 이전 마지막 이동 2026-09-08T10:37:13Z) |
| 빌드 트랙 | source-build | artifacts 선택자(build-ledger.dockerfile) |
| Dockerfile(선택자) | Dockerfile.source-build-sm12x | build-ledger.dockerfile |
| vLLM 저장소 | https://github.com/vllm-project/vllm.git | docker history build-arg VLLM_REPO |
| 이미지 태그 | easy-vllm:0.32.0-cu133-aarch64-source-sm12x | 셀 env(output/multi/envs/.env.q38-load-local-1m) IMAGE_TAG |
| 이미지 digest | sha256:436bd187022965d2e41f8969695d8446baeca6c5d50692d8b6db76ae2877a916 | sweep_index.meta.image_digest(measured(docker inspect .Image)) |
| torch | 2.13.0a0+9186a08 | docker image inspect Config.Env PYTORCH_VERSION |
| CUDA | 13.3.1.008 | docker image inspect Config.Env CUDA_VERSION |
| NGC 베이스 | 26.07 | docker image inspect Config.Env NVIDIA_PYTORCH_VERSION |
| CPU arch | arm64 | docker image inspect Architecture |
| 드라이버 | 580.178.04 | attestation parity.driver(output/multi/benchlog/attestation_q38-load-local-1m.json · 노드별 관측 · verdict=equal) — main=sub 일치 · 작성 2026-10-30T13:01:19Z < 측정 시작 2026-10-30T13:01:41Z — 측정 전(빌드·스모크)의 관측 · 원장 순서로 판별한 같은 기동의 관측(결속된 스모크 로그 미확인 · 제한검색 · 약한 판별)(판별 근거 = attestation 범위 줄 · 원장 docs/logs/main/events/2026-10.jsonl) |
| 드라이버(노드별 관측) | main=580.178.04 · sub=580.178.04 | attestation parity.driver(output/multi/benchlog/attestation_q38-load-local-1m.json · 노드별 관측 · verdict=equal) — main=sub 일치 · 작성 2026-10-30T13:01:19Z < 측정 시작 2026-10-30T13:01:41Z — 측정 전(빌드·스모크)의 관측 · 원장 순서로 판별한 같은 기동의 관측(결속된 스모크 로그 미확인 · 제한검색 · 약한 판별)(판별 근거 = attestation 범위 줄 · 원장 docs/logs/main/events/2026-10.jsonl) |
| ⚠ 드라이버 관측 ≠ 선언 | 충돌 없음 — 관측 580.178.04 = 선언 580.178.04 | 관측: attestation parity.driver(output/multi/benchlog/attestation_q38-load-local-1m.json · 노드별 관측 · verdict=equal) — main=sub 일치 · 작성 2026-10-30T13:01:19Z < 측정 시작 2026-10-30T13:01:41Z — 측정 전(빌드·스모크)의 관측 · 원장 순서로 판별한 같은 기동의 관측(결속된 스모크 로그 미확인 · 제한검색 · 약한 판별)(판별 근거 = attestation 범위 줄 · 원장 docs/logs/main/events/2026-10.jsonl) · 선언: sweep_index.meta.driver_version(output/multi/benchlog/sweep_loading_q38-load-local-1m · 측정 시 조립 — sweep_bench.sh 가 manifest 에서 읽는다) (manifest 선언 옮김) |
| 호스트 OS | 미관측 | 미관측 |

> 재현 좌표는 빌드 입력(릴리스 태그 또는 40자 SHA)이다 — 엔진 자기보고 · 인증서 · wheel 의 버전 문자열은 빌드 입력이 아니다(각 값을 만든 생산자는 출처 칸).

**양자화 구성**(체크포인트 config 관측 · 이름의 `q` 축은 선언 방식 하나 — ModelOpt MIXED 는 층 개수 우세가 보조 규칙)

| 범위 | dtype | 출처 |
|---|---|---|
| quantized_layers 48개(예 model.language_model.layers.0.mlp.experts) | `NVFP4` | /app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4/config.json quantization_config quantized_layers(modelopt MIXED_PRECISION) |
| quantized_layers 1개(예 model.language_model.layers.1.ple.ple_embedding.ngram_embedding) | `FP8` | /app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4/config.json quantization_config quantized_layers(modelopt MIXED_PRECISION) |
| quantized_layers 1개(예 mtp.layers.0.mlp.experts) | `FP8_PB_WO` | /app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4/config.json quantization_config quantized_layers(modelopt MIXED_PRECISION) |
| 양자화 제외 모듈 292개 | `unquantized(아래 기본 dtype)` | /app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4/config.json quantization_config ignore·exclude_modules |
| 기본 dtype(비양자화 가중치·계산) | `bfloat16` | /app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4/config.json dtype |

**이 모델에 필요한 패치**(01 §1.1 관련성 · ① 패치 선언 대상 × 모델 config ② 엔진 로그 발화 — 둘 다 관측일 때만 확정): required 0개 · inactive-inferred 1개 · unknown 17개(unknown = 판정 신호 부족 — 필요 여부를 이 표로 단정하지 않는다).
<!-- /FACT:resolved -->

> 이 절이 답하는 질문: 위 값들 중 무엇을 그대로 참조하고 무엇을 네 환경에서 다시 도출해야 하며, 이 빌드 트랙은 왜 골랐는가?

이번 실험은 기존 source-build 이미지를 사용했으며 새 빌드 트랙 비교를 하지 않았다. 공유 SM12x/EXL3 포트와 NGC torch를 사용한 source 트랙의 계보는 선행 devlog_26103000 및 parent-source-core 슬롯이다. 새 재구성 명령은 generated-unverified다.
재현 vLLM 좌표는5df63a7203cdb9328d824a27b682e77c7b9928be다. image label0.32.0, 실제 패키지의0.32.1.dev SCM 문자열, 도구가 이미지명에서 얻은 benchmark 키는 서로 다른 생산물이다. 노드별 실제 문자열·빌드원장 차이는 FACT대로 보존한다.
aarch64·SM12x arch와 CUDA/driver는 호스트·이미지 층을 구별하고 수신자 HW로 다시 도출한다. actual SHA/torch/driver equality는 whole-image equality가 아니다. manifest 선언보다 attestation·이미지 원장의 관측을 우선한다.
핵심 공유 의존 핀은05의 hub1.33.0/pytest9.2.0,06의 B12X1.5.1,75의 FLA0.6.0/fla-core0.6.0/marisa-trie1.4.1이며 Qwen 전용 최소 요구임을 입증한 것이 아니다. Ray·FlashInfer·Triton 등 나머지 값은 build recipe ARG/requirements와 해당 관측 FACT를 읽으며 임의로 새 버전으로 바꾸지 않는다.

## 0.5 서빙 노브와 값의 지위
> 01-artifacts.md §1.3 의 `kind: value-status` 블록에서 기계가 생성한 요약이다. 지위가 `tuned` · `declared-requirement` 가 아닌 값은 **이 셀에서 조정된 적도, 빼면 깨진다고 확인된 적도 없다** — 필요조건으로 복사하지 마라. `declared-requirement` 는 빼거나 바꾸면 실패가 관측된 값이다(근거의 W id · artifacts 헤더를 보고 네 환경에서도 그 실패 조건이 성립하는지 판단한다).

<!-- FACT:knob_status -->
노브 13개 — `inherited` 13.

| 노브 | 값 | 지위 | 근거 | id |
|---|---|---|---|---|
| `tensor-parallel-size` | 2 | inherited | 손레버 또는 검증된 선례에서 승계한 고정 통제값; 이번 저장정책 비교에서 값을 스윕하거나 제거 대조하지 않았다. 선언한 목표도 필요조건의 증명이 아니다. | V1 |
| `distributed-executor-backend` | ray | inherited | 손레버 또는 검증된 선례에서 승계한 고정 통제값; 이번 저장정책 비교에서 값을 스윕하거나 제거 대조하지 않았다. 선언한 목표도 필요조건의 증명이 아니다. | V2 |
| `gpu-memory-utilization` | 0.80 | inherited | 손레버 또는 검증된 선례에서 승계한 고정 통제값; 이번 저장정책 비교에서 값을 스윕하거나 제거 대조하지 않았다. 선언한 목표도 필요조건의 증명이 아니다. | V3 |
| `max-model-len` | 1048576 | inherited | 손레버 또는 검증된 선례에서 승계한 고정 통제값; 이번 저장정책 비교에서 값을 스윕하거나 제거 대조하지 않았다. 선언한 목표도 필요조건의 증명이 아니다. | V4 |
| `kv-cache-dtype` | fp8_e4m3 | inherited | 손레버 또는 검증된 선례에서 승계한 고정 통제값; 이번 저장정책 비교에서 값을 스윕하거나 제거 대조하지 않았다. 선언한 목표도 필요조건의 증명이 아니다. | V5 |
| `kv-cache-memory-bytes` | 15032385536 | inherited | 손레버 또는 검증된 선례에서 승계한 고정 통제값; 이번 저장정책 비교에서 값을 스윕하거나 제거 대조하지 않았다. 선언한 목표도 필요조건의 증명이 아니다. | V6 |
| `enforce-eager` | true | inherited | 손레버 또는 검증된 선례에서 승계한 고정 통제값; 이번 저장정책 비교에서 값을 스윕하거나 제거 대조하지 않았다. 선언한 목표도 필요조건의 증명이 아니다. | V7 |
| `async-scheduling` | false | inherited | 손레버 또는 검증된 선례에서 승계한 고정 통제값; 이번 저장정책 비교에서 값을 스윕하거나 제거 대조하지 않았다. 선언한 목표도 필요조건의 증명이 아니다. | V8 |
| `max-num-seqs` | 1 | inherited | 손레버 또는 검증된 선례에서 승계한 고정 통제값; 이번 저장정책 비교에서 값을 스윕하거나 제거 대조하지 않았다. 선언한 목표도 필요조건의 증명이 아니다. | V9 |
| `max-num-batched-tokens` | 2048 | inherited | 손레버 또는 검증된 선례에서 승계한 고정 통제값; 이번 저장정책 비교에서 값을 스윕하거나 제거 대조하지 않았다. 선언한 목표도 필요조건의 증명이 아니다. | V10 |
| `language-model-only` | true | inherited | 손레버 또는 검증된 선례에서 승계한 고정 통제값; 이번 저장정책 비교에서 값을 스윕하거나 제거 대조하지 않았다. 선언한 목표도 필요조건의 증명이 아니다. | V11 |
| `engram-config` | {"cpu_offload": false} | inherited | 손레버 또는 검증된 선례에서 승계한 고정 통제값; 이번 저장정책 비교에서 값을 스윕하거나 제거 대조하지 않았다. 선언한 목표도 필요조건의 증명이 아니다. | V12 |
| `hf-overrides` | {"text_config":{"max_position_embeddings":1048576,"rope_parameters":{"mrope_interleaved":true,"mrope_section":[11,11,10],"partial_rotary_factor":0.25,"rope_theta":10000000,"rope_type":"yarn","factor":4,"original_max_position_embeddings":262144}}} | inherited | 손레버 또는 검증된 선례에서 승계한 고정 통제값; 이번 저장정책 비교에서 값을 스윕하거나 제거 대조하지 않았다. 선언한 목표도 필요조건의 증명이 아니다. | V13 |
<!-- /FACT:knob_status -->

> 이 절이 답하는 질문: 이 레시피를 다른 환경으로 옮길 때 어떤 노브를 먼저 다시 재야 하고, 어떤 노브는 빼면 깨지는가?

① V3~V10의 gmu/context/KV/동시성/batched tokens는 수신자 메모리·엔진 KV 용량·목표 부하로 재도출하고 새 smoke/lite/full을 수행할 값이다.
② 현 셀은 제거 실패 대조를 하지 않아 declared-requirement로 확정한 knob가 없다. YaRN 선례의 실패조건은W2에 한정하며 상위버전에서 재검증한다.
③ V1~V13은 상기 요구 또는 손레버/선례에서 승계한 고정값이다. 후보 gmu 지위를 필요조건으로 읽거나 KV14GiB를 최소량으로 복사하지 않는다.

## 0.6 재검증 · 라우팅
- 이 자료를 `vllm-recipe-explorer` 인터뷰의 **warm-start 근거로만** 투입하고, 평소대로 plan-gate → 레시피 수렴 → 스모크 게이트를 그대로 밟아라(가속이지 우회 ✗).

> 이 절이 답하는 질문: 다른 환경(다른 HW · 다른 vLLM 버전 · 다른 노드 수)에서 이 지도를 쓸 때 무엇부터, 어떤 순서로 확인해야 하며, 실패하면 어디로 돌리는가?

1. 모델바이트·revision·실제 quant composition을 config/index와 hash로 확인한다 → 달라지면 이 결과를 새 모델 baseline으로 쓰지 않는다.
2. 각 build patch의 대상 심벌·upstream 지원·SM12X_PORT 게이트/실패 경로와 빌드원장을 읽는다 → 상위버전에서 불필요한 코드는 제거 후보로 두되 새 빌드·스모크로 확인한다.
3. 양 노드 같은 container model 경로·실제 ro bind와 vLLM SHA/torch/driver를 관측한다 → 경로/ABI가 다르면 먼저 노드 통로를 정합한다.
4. weights/KV/overhead의 선언 예산과 honored를 확인한 뒤 기동한다 → 부족하면 절대KV/동시성 등 입력을 재도출한다. engine 관측과 선언은 나눠 기록한다.
5. health200 +비어 있지 않은 short inference +해당 runtime 경로 로그를 확인한 뒤 lite gate/GuideLLM full3회를 수행한다 → 부하/형상이 바뀌면 이전 수치는 직접 비교하지 않는다.
빌드·버전 문제는 upstream-version-watch, 설정·KV는 vllm-recipe-explorer, 성능 기대 미달은 adversarial-benchmark, 노드·인터커넥트는 terraforming_node가 소유한다.

## 0.7 메타
<!-- FACT:meta -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.32.0-g5df63a7203cd/qwen3.8-flash-next-nvfp4/gb10-1g2n-cluster-native/qnvfp4-len1048576-kvfp8` |
| 형식 | `hint-payload/v7` |
| 앵커 | `PROVENANCE.json` — 봉인 때 hint 브랜치 페이로드 커밋에 묶인다 |
| 캠페인 · 셀 · 노드 | camp-26103017-nas-nvme-loading · q38-load-local-1m · cluster |
| 발행 모드 | campaign |
| 발행 토픽 | camp_26103017_nas_nvme_loading__cluster__q38_load_local_1m |
| work-manifest | docs/_evidence/camp_26103017_nas_nvme_loading__cluster__q38_load_local_1m.work-manifest.json |
| 증거 등급(task_class) | full_benchmark |
| 승인 출처 | campaign:hint_targets |
| 승인 시각(UTC) | 2026-10-30T08:39:00Z |
| 승인 발화(전사) | 사용자: hint태그도 발행해야 하니 full bench도 수행해줘. 범위2(모델별 대표 정책에서 full bench 및 hint) 선택. 대표 정책③을 포함한 실행 계획에 서브 모델 지정 후 진행 지시. |
| 현행 full 정의 충족(측정 등급) | 예 · 이 측정의 반복 3 · 정의 출처 `.claude/rules/docs.md:42`(03 §3.5) |
| 독립 사실 검증(발행 전) | 통과 — 검증자 `independent-factchecker`(저작자 `main-author` 아님) · 주장 1034건 · 지적 7건 중 고침 7건 · 열림 0 · 2026-10-30T14:05:12Z |
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
**확정 이름** `hint/0.32.0-g5df63a7203cd/qwen3.8-flash-next-nvfp4/gb10-1g2n-cluster-native/qnvfp4-len1048576-kvfp8` — 꼬리 0토큰.

_빈 꼬리 — 발행 Agent 가 가르는 노브가 없다고 명시했다(`inputs/tail.json` = `[]`)._
<!-- /FACT:name_tail -->

> 이 절이 답하는 질문: 이 셀을 같은 q·len·kv 의 다른 셀과 가르는 노브는 무엇이며 그것을 이름 꼬리로 어떻게 적는가 — 가르는 노브가 없으면 왜 빈 꼬리인가?

inputs/tail.json에 빈 목록을 명시했다. 이번 기본 축의1M·FP8 KV에 더해 resident/specoff/eager를 자동 꼬리 세트로 반복하지 않는다. 해당 형상은 값 지위와 서사에 적혀 있으며 동일 기본 이름 충돌의 시간 접미사는 도구가 판단한다.
