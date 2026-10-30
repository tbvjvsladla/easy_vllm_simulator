# 00 · hint 지도 — 가장 먼저 읽는 파일

<!-- FACT:header -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.32.0-g5df63a7203cd/deepseek-v4.1-flash-exl3-2.0bpw-sfxnz/gb10-1g2n-cluster-native/qexl3-len1048576-kvfp8` |
| 형식 | `hint-payload/v7` · 이름 문법 `v7` |
| 생성(UTC · 주입) | 2026-10-30T16:00:30Z |
| 캠페인 · 셀 · 노드 · 모드 | camp-26103017-nas-nvme-loading · ds41-load-local-1m · cluster · campaign |
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
| 자격 근거 | `output/multi/benchlog/serve_proof_ds41-load-local-1m.json` — 시점 범위: 작성 2026-10-30T15:42:25Z(output/multi/benchlog/serve_proof_ds41-load-local-1m.json 파일 mtime(파일시스템 관측 — 문서에 시각 칸이 없다 · 기록 필드 아님)) · 측정 창 대비 작성 2026-10-30T15:42:25Z < 측정 시작 2026-10-30T15:42:48Z — 측정 전(빌드·스모크)의 관측 |
| 증거 등급(task_class) | full_benchmark |
| 측정 등급(bench_mode) | full |
| 셀 등급(cell_grade) | `full` — 성능 판정 · 부하 곡선 · 데이터셋 출처는 `03-benchmark.md` |
| 성능 판정(measurement.verdict) | `REFUTE` — 출처 output/multi/benchlog/sweep_loading_ds41-load-local-1m/verdict.json(output · verdict_rule 출력) verdict |

> **REFUTE** — 이 셀의 성능 판정은 **기각**이다(측정은 됐으나 판정 기준 미달). 수치는 관측이지 baseline · 권고가 아니다 — 판정 출처 output/multi/benchlog/sweep_loading_ds41-load-local-1m/verdict.json(output · verdict_rule 출력) verdict.
> 기각 사유: M(22.26) < 43.73×(1−0.15)=37.17 [E(external_reference)] (ratio 0.509) → 기대 이하
<!-- /FACT:grade -->

<!-- FACT:lite_summary -->
<!-- /FACT:lite_summary -->

## 0.1 요약
> 이 절이 답하는 질문: 이 셀은 무엇을 서빙했고, 무엇이 결정적이었으며, 수신자는 무엇을 가장 조심해야 하는가?

DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz를 GB10 두 노드·TP2, vLLM 소스5df63a7203cdb9328d824a27b682e77c7b9928be에서 max-model-len1048576·FP8 KV·file-backed Engram·DSpark3·eager로 서빙했고 local 대표 full의 C1 decode는22.26t/s, weak REFUTE다. EXL3 최소 이식과 Engram source 결속을 확인했다(W13~W18). KV·동시성은 고정 손레버이며 최소 필요량·자동 수렴값이 아니다.

## 0.2 유효맥락
<!-- FACT:context -->
| 항목 | 값 | 출처 |
|---|---|---|
| 모델 | deepseek-v4.1-flash-exl3-2.0bpw-sfxnz | sweep_index.meta(output/multi/benchlog/sweep_loading_ds41-load-local-1m) |
| HF repo | 미관측 | 체크포인트 .git/config 부재(/app/quant_models/DeepSeek/DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz → <manifest.quant_model_path>/DeepSeek/DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz) — HF repo 미관측 |
| HF revision(체크포인트 git HEAD) | 미관측 | 체크포인트 .git/HEAD 부재(/app/quant_models/DeepSeek/DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz → <manifest.quant_model_path>/DeepSeek/DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz) — revision 미관측 |
| base_model | deepseek-ai/DeepSeek-V4.1-Flash | 체크포인트 README base_model(/app/quant_models/DeepSeek/DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz → <manifest.quant_model_path>/DeepSeek/DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz) |
| base 슬러그 | deepseek-v4.1-flash-exl3-2.0bpw-sfxnz | model(deepseek-v4.1-flash-exl3-2.0bpw-sfxnz) − vocab quant_suffixes(naming.base_slug) |
| 양자화 | exl3 | sweep_index.meta(output/multi/benchlog/sweep_loading_ds41-load-local-1m) |
| GPU | NVIDIA GB10 | sweep_index.meta(output/multi/benchlog/sweep_loading_ds41-load-local-1m) |
| 토폴로지 · TP | multi · TP=2 | sweep_index.meta(output/multi/benchlog/sweep_loading_ds41-load-local-1m) |
| 실행 평면 | docker | artifacts.plane_of |
| vLLM(측정 강식별 키 `vllm_version`) | 0.32.0 | sweep_index.meta(output/multi/benchlog/sweep_loading_ds41-load-local-1m) |

**명명 축** — 태그 이름 = 결정론부(vllm · model · arch · q · len · kv — 도구가 증거에서 파생 · 발행자 입력 ✗) + 꼬리(발행 Agent 가 고르고 서빙 설정 file · key · value 로 근거 대조 · 0.9) + 중복 시 timestamp.

| 축 | 값 | 출처 |
|---|---|---|
| 세그먼트 `vllm` | 0.32.0-g5df63a7203cd | track=선택자 Dockerfile.source-build-sm12x · ref=docker history build-arg VLLM_REF · version=VLLM_VERSION 미관측 · repo=docker history build-arg VLLM_REPO · sha=VLLM_REF(40자 커밋 · docker history build-arg VLLM_REF) · exact_tag=measured image sha256:436bd187022965d2e41f8969695d8446baeca6c5d50692d8b6db76ae2877a916: /workspace/vllm-src/.git HEAD and exact tag peeled to 5df63a7203cdb9328d824a27b682e77c7b9928be (create/cp, no start) · VLLM_REF=5df63a7203cd…(40자 커밋) → <직전 릴리스 0.32.0>-g<sha12> |
| 세그먼트 `model` | deepseek-v4.1-flash-exl3-2.0bpw-sfxnz | 서빙 yaml model(output/multi/configs/ds41-load-local-1m.yaml) · model_path basename(DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz) 소문자 = 인증서 model 키 규칙 |
| 세그먼트 `arch` | gb10-1g2n-cluster-native | derived(hw·gpus_per_node·nodes·role·target·plane) |
| 세그먼트 `recipe` | qexl3-len1048576-kvfp8 | derived(q·len·kv) |
| 축 `hw` | gb10 | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(자동 파생 — 측정 TP=2(sweep_index.meta.tensor_parallel_size(output/multi/benchlog/sweep_loading_ds41-load-local-1m · generated_utc = 측정 키)) > gpus_per_node=1(output/multi/manifest.yaml(manifest_contract 로더)) · 단일 노드 형상일 수 없다 · 일치 신호: 멀티 serve proof output/multi/benchlog/serve_proof_ds41-load-local-1m.json(kind=multinode_serve_proof) · 노드 정합 attestation output/multi/benchlog/attestation_ds41-load-local-1m.json) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) · vocab hw[gb10]←'NVIDIA GB10' |
| 축 `gpus_per_node` | 1 | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(자동 파생 — 측정 TP=2(sweep_index.meta.tensor_parallel_size(output/multi/benchlog/sweep_loading_ds41-load-local-1m · generated_utc = 측정 키)) > gpus_per_node=1(output/multi/manifest.yaml(manifest_contract 로더)) · 단일 노드 형상일 수 없다 · 일치 신호: 멀티 serve proof output/multi/benchlog/serve_proof_ds41-load-local-1m.json(kind=multinode_serve_proof) · 노드 정합 attestation output/multi/benchlog/attestation_ds41-load-local-1m.json) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `nodes` | 2 | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(자동 파생 — 측정 TP=2(sweep_index.meta.tensor_parallel_size(output/multi/benchlog/sweep_loading_ds41-load-local-1m · generated_utc = 측정 키)) > gpus_per_node=1(output/multi/manifest.yaml(manifest_contract 로더)) · 단일 노드 형상일 수 없다 · 일치 신호: 멀티 serve proof output/multi/benchlog/serve_proof_ds41-load-local-1m.json(kind=multinode_serve_proof) · 노드 정합 attestation output/multi/benchlog/attestation_ds41-load-local-1m.json) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `role` | cluster | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(자동 파생 — 측정 TP=2(sweep_index.meta.tensor_parallel_size(output/multi/benchlog/sweep_loading_ds41-load-local-1m · generated_utc = 측정 키)) > gpus_per_node=1(output/multi/manifest.yaml(manifest_contract 로더)) · 단일 노드 형상일 수 없다 · 일치 신호: 멀티 serve proof output/multi/benchlog/serve_proof_ds41-load-local-1m.json(kind=multinode_serve_proof) · 노드 정합 attestation output/multi/benchlog/attestation_ds41-load-local-1m.json) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `target` | native | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(자동 파생 — 측정 TP=2(sweep_index.meta.tensor_parallel_size(output/multi/benchlog/sweep_loading_ds41-load-local-1m · generated_utc = 측정 키)) > gpus_per_node=1(output/multi/manifest.yaml(manifest_contract 로더)) · 단일 노드 형상일 수 없다 · 일치 신호: 멀티 serve proof output/multi/benchlog/serve_proof_ds41-load-local-1m.json(kind=multinode_serve_proof) · 노드 정합 attestation output/multi/benchlog/attestation_ds41-load-local-1m.json) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) · target_gpu 'NVIDIA GB10' = 호스트 hw → native |
| 축 `plane` | docker | artifacts.plane_of → cell-env(IMAGE_TAG\|BUILD_DOCKERFILE) · vocab plane[docker]→'' |
| 축 `q` | exl3 | /app/quant_models/DeepSeek/DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz → <manifest.quant_model_path>/DeepSeek/DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz/config.json quantization_config quant_method=exl3 · vocab quant[exl3]←'exl3' |
| 축 `len` | 1048576 | 서빙 yaml max-model-len(output/multi/configs/ds41-load-local-1m.yaml) |
| 축 `kv` | fp8 | 서빙 yaml kv-cache-dtype(output/multi/configs/ds41-load-local-1m.yaml) · vocab kv[fp8]←'fp8' |
<!-- /FACT:context -->

> 이 절이 답하는 질문: 이 지도는 어떤 조건에서만 성립하며, 이 형상은 어떤 기전으로 버티고 다른 형상은 왜 실패했는가?

GB10 한 GPU씩 두 노드의 Ray TP2/DP1·UMA와 인터커넥트가 조건이다. UMA에서 CPU offload는 다른 물리 메모리를 만들지 않으며 Engram 전체 상주·모델 payload·row lookup·page cache를 구별해야 한다(W2, 현재 testlog). 다른 메모리 구조에서는 절대KV/동시성/호스트 예산을 재도출한다.
현재는 EXL3 MCG routed experts와 lm_head MXFP8, file-backed Engram 및 iterator skip 경로를 관측한 형상이다. 설치·적용만으로 선택90~92나74 성능 경로가 활성이라고 하지 않는다(W9/W16). 현재 고정eager 셀은 graph NaN 해결·장문 열 안전성을 새로 입증하지 않았다.
과거0.30 graph와 열 부하,0.32의131K 형상, 이번1M·모델-root table 경로는 여러 조건이 다르다. 버틴 경로와 실제 실패는02에서 시기별로 나눴으며 단일 노브의 유일 인과는 미결이다. 실제1M 설정·짧은 입력 성공과 million-token 품질/스트레스는 다르다.

## 0.3 벽 지도 요약
> 02-narrative.md §2.2 의 `kind: wall` 블록(과 §2.4 의 `kind: misdiagnosis` 블록)에서 기계가 생성한 표다(손저작 ✗ · 순서 = 넘은 순서). 원인·해소의 전문과 원문 발췌는 02 에 있다.

<!-- FACT:wall_map -->
| 순서 | id | 증상 | 해소 | 전이등급 | 검증 |
|---|---|---|---|---|---|
| 1 | W1 | stock0.30 EXL3 경로 부재 | 당시 EXL3 native/plugin 및 소스 이식; 현재0.32는 별도 재도출 | arch-invariant | plan_26100208_V4_1_Flash_멀티노드_1M_캠페인의 해당 관측과 후속 절; 새 결과와 구분 |
| 2 | W2 | Engram UMA 상주 용량의 산술 제약 | file-backed lookup와 iterator skip을 분리해 설계하고 후속에서 실제 발화 확인 | arch-scaled | plan_26100319_V4_1_Flash_2노드_광의탐색_캠페인_재설계의 해당 관측과 후속 절; 새 결과와 구분 |
| 3 | W3 | 과거 1M 장문 열 하드다운 | 당시 열 차단기와 별도 장문 범위 제한; 현재는 1M 설정·짧은 입력만 검증 | arch-scaled | plan_26100319_V4_1_Flash_2노드_광의탐색_캠페인_재설계의 해당 관측과 후속 절; 새 결과와 구분 |
| 4 | W4 | 과거 graph NaN과 KV null block | 당시89 capture 후 zero; 현재 eager 셀은 graph 교정의 새 검증이 아니다 | arch-invariant | devlog_26100610_V4_1_Flash_트랙K_커널레버_이식과_정식화의 해당 관측과 후속 절; 새 결과와 구분 |
| 5 | W5 | 과거 의존 wheel 서명 URL 만료 | 당시55 청크·Range 이어받기·해시 검증; 현재55를 자동 이월하지 않음 | arch-invariant | devlog_26100610_V4_1_Flash_트랙K_커널레버_이식과_정식화의 해당 관측과 후속 절; 새 결과와 구분 |
| 6 | W6 | 과거 graph 교정 뒤 성능 격차 | 당시p2b/WOA 및 커널 레버를 개별 진단; 현재 최소 port에 모두 활성이라는 뜻이 아님 | judgment | devlog_26100610_V4_1_Flash_트랙K_커널레버_이식과_정식화의 해당 관측과 후속 절; 새 결과와 구분 |
| 7 | W7 | 과거 dense MXFP8 Cutlass 비용 | 과거91 B12X/auto 선택; 현재 선택성능91 미적용 | arch-locked | devlog_26100610_V4_1_Flash_트랙K_커널레버_이식과_정식화의 해당 관측과 후속 절; 새 결과와 구분 |
| 8 | W8 | 과거 small-M dense GEMV 후보 | 당시92는90/91 전제; 현재 선택성능90~92 GPU 성공으로 승계하지 않음 | arch-locked | devlog_26100610_V4_1_Flash_트랙K_커널레버_이식과_정식화의 해당 관측과 후속 절; 새 결과와 구분 |
| 9 | W9 | 과거 MoE preparation glue | 과거 fused-prep 활성 검증과 현재74 설치·env0을 구별한다 | arch-locked | devlog_26100610_V4_1_Flash_트랙K_커널레버_이식과_정식화의 해당 관측과 후속 절; 새 결과와 구분 |
| 10 | W10 | i5-a-kern boot1 장문 열 중단 | 당시 요청 abort와 kill; 현재1M 설정 성공을 장문 품질·열 검증으로 확대하지 않음 | arch-scaled | testlog_26100510_V4_1_Flash_i5_a_kern_정식레시피_판정의 해당 관측과 후속 절; 새 결과와 구분 |
| 11 | W11 | i5-a-kern boot2 C2 run2 열 사살 | 그 스윕을 Full 근거에서 제외; 별도 C1 측정으로 분리 | arch-scaled | testlog_26100510_V4_1_Flash_i5_a_kern_정식레시피_판정의 해당 관측과 후속 절; 새 결과와 구분 |
| 12 | W12 | i5-a-kern boot3 C2 run3 끝 열 사살 | 이 스윕도 제외하고 boot4 C1×3을 별도로 검증; 현재 C2 실패로 세지 않음 | arch-scaled | testlog_26100510_V4_1_Flash_i5_a_kern_정식레시피_판정의 해당 관측과 후속 절; 새 결과와 구분 |
| 13 | W13 | 새0.32 stock EXL3 registry 거부 | 공식0.32 위 최소 PRE80~89와 native POST port를 새로 구성 | arch-invariant | testlog_26102919_i5_032_stock_EXL3_등록부_거부의 해당 관측과 후속 절; 새 결과와 구분 |
| 14 | W14 | 0.32 common Engram API와 anchor 드리프트 | PRE83/88 common Engram/V2 접점을 재도출하고84를 새 형상으로 정합 | arch-invariant | devlog_26103005_i5_032_최소이식과_실측_인계의 해당 관측과 후속 절; 새 결과와 구분 |
| 15 | W15 | native attempt1 Python 의존 결손 | POST75 고정3wheel 수리 뒤 attempt2 native POST 실제 재컴파일; core 불변/PRE cache와 구분 | arch-invariant | devlog_26103005_i5_032_최소이식과_실측_인계의 해당 관측과 후속 절; 새 결과와 구분 |
| 16 | W16 | 선택 성능90~92의 미적용 | 최소 port를 유지; graph·10kernel bundle을 달성했다고 하지 않음 | arch-locked | devlog_26103005_i5_032_최소이식과_실측_인계의 해당 관측과 후속 절; 새 결과와 구분 |
| 17 | W17 | 과거131K full의 외부 참조 미달 | 당시 유효 Full과 REFUTE를 보존; 그 처리량·acceptance를 현1M 값으로 쓰지 않음 | judgment | testlog_26103005_i5_032_TP2와_Full_판정의 해당 관측과 후속 절; 새 결과와 구분 |
| 18 | W18 | 이번 저장정책의 Engram 경로 결속 | 첫 accepted start 전에 두 입력을 정합하고 양rank file-backed/iterator skip 실제 발화를 확인 | arch-invariant | devlog_26103023_DeepSeek_1M_로딩비교_진행의 해당 관측과 후속 절; 새 결과와 구분 |

**오진 · 정정**(02 §2.4 — 현재지위가 `가설(강등)` 인 기전은 처방이 아니다)

| id | 증상 | 현재지위 | 검증 |
|---|---|---|---|
| M1 | 과거 남은 step 비용을 동기 Engram prestage 탓으로 추측 | 반증 | devlog_26100610_V4_1_Flash_트랙K_커널레버_이식과_정식화의 K3 기록 |
| M2 | Engram row table을 매 토큰 dense 전체 읽기로 집계한 루프라인 | 반증 | devlog_26103005_i5_032_최소이식과_실측_인계의 루프라인 정정 및 현재 raw roofline |
<!-- /FACT:wall_map -->

## 0.4 결정론 해소값
<!-- FACT:resolved -->
| 항목 | 값 | 출처 |
|---|---|---|
| vLLM 빌드 입력(종류) | commit | naming.vllm_build_input |
| vLLM 빌드 입력(ref) | 5df63a7203cdb9328d824a27b682e77c7b9928be | docker history build-arg VLLM_REF |
| vLLM 빌드 계보 SHA(40자) | 5df63a7203cdb9328d824a27b682e77c7b9928be | VLLM_REF(40자 커밋 · docker history build-arg VLLM_REF) |
| 직전 릴리스(prev_release) | 0.32.0 | naming.vllm_build_input |
| vLLM 엔진 자기보고(엔진 로그 기동 배너) | 0.32.1.dev0+g5df63a720.d20261029 | output/multi/benchlog/sweep_loading_ds41-load-local-1m/lite_engine_head_ds41-load-local-1m.log:60 — 측정 캡처에 기록된 값 · 현재 serve 세션 경계 미관측 — 누적 로그의 기동 epoch 귀속 미검증 · 같은 값의 다른 로그 1건 · 지위 `measurement-capture-unbound` |
| 인증서 `vllm_version`(강한 일치 키) | 미관측 | 생산자 `absent` · 인증서 vllm_version 없음 |
| vLLM 설치 distribution 메타(출처별 관측 · 다운로드 wheel 증명 아님) | 노드별 상이·공통값 미선택(각 값은 출처) | 노드별 상이 — 값을 고르지 않는다: attestation nodes.<n>.vllm_dist_version(output/multi/benchlog/attestation_ds41-load-local-1m.json · 노드별 관측) — ⚠ 노드별 상이(main=0.32.1.dev0+g5df63a720.d20261029.cu133 · sub=0.32.1.dev0+g5df63a720.d20261028.cu133 · 값 = main) · 작성 2026-10-30T15:42:25Z < 측정 시작 2026-10-30T15:42:48Z — 측정 전(빌드·스모크)의 관측 · 원장 순서로 판별한 같은 기동의 관측(결속된 스모크 로그 미확인 · 제한검색 · 약한 판별)(판별 근거 = attestation 범위 줄 · 원장 docs/logs/main/events/2026-10.jsonl)(sweep_index.meta.vllm_build='NA' 는 sweep_bench 가 엔진 로그에서 `v<x.y.z…>` 정규식으로 잡는 값이다 → engine_self_report 의 원천) · 지위 `observed-node-difference` |
| 체크포인트 revision(HF · git HEAD) | 미관측 | 미관측 |
| 빌드 트랙 | source-build | artifacts 선택자(build-ledger.dockerfile) |
| Dockerfile(선택자) | Dockerfile.source-build-sm12x | build-ledger.dockerfile |
| vLLM 저장소 | https://github.com/vllm-project/vllm.git | docker history build-arg VLLM_REPO |
| 이미지 태그 | easy-vllm:0.32.0-cu133-aarch64-source-sm12x | 셀 env(output/multi/envs/.env.ds41-load-local-1m) IMAGE_TAG |
| 이미지 digest | sha256:436bd187022965d2e41f8969695d8446baeca6c5d50692d8b6db76ae2877a916 | sweep_index.meta.image_digest(measured(docker inspect .Image)) |
| torch | 2.13.0a0+9186a08 | docker image inspect Config.Env PYTORCH_VERSION |
| CUDA | 13.3.1.008 | docker image inspect Config.Env CUDA_VERSION |
| NGC 베이스 | 26.07 | docker image inspect Config.Env NVIDIA_PYTORCH_VERSION |
| CPU arch | arm64 | docker image inspect Architecture |
| 드라이버 | 580.178.04 | attestation parity.driver(output/multi/benchlog/attestation_ds41-load-local-1m.json · 노드별 관측 · verdict=equal) — main=sub 일치 · 작성 2026-10-30T15:42:25Z < 측정 시작 2026-10-30T15:42:48Z — 측정 전(빌드·스모크)의 관측 · 원장 순서로 판별한 같은 기동의 관측(결속된 스모크 로그 미확인 · 제한검색 · 약한 판별)(판별 근거 = attestation 범위 줄 · 원장 docs/logs/main/events/2026-10.jsonl) |
| 드라이버(노드별 관측) | main=580.178.04 · sub=580.178.04 | attestation parity.driver(output/multi/benchlog/attestation_ds41-load-local-1m.json · 노드별 관측 · verdict=equal) — main=sub 일치 · 작성 2026-10-30T15:42:25Z < 측정 시작 2026-10-30T15:42:48Z — 측정 전(빌드·스모크)의 관측 · 원장 순서로 판별한 같은 기동의 관측(결속된 스모크 로그 미확인 · 제한검색 · 약한 판별)(판별 근거 = attestation 범위 줄 · 원장 docs/logs/main/events/2026-10.jsonl) |
| ⚠ 드라이버 관측 ≠ 선언 | 충돌 없음 — 관측 580.178.04 = 선언 580.178.04 | 관측: attestation parity.driver(output/multi/benchlog/attestation_ds41-load-local-1m.json · 노드별 관측 · verdict=equal) — main=sub 일치 · 작성 2026-10-30T15:42:25Z < 측정 시작 2026-10-30T15:42:48Z — 측정 전(빌드·스모크)의 관측 · 원장 순서로 판별한 같은 기동의 관측(결속된 스모크 로그 미확인 · 제한검색 · 약한 판별)(판별 근거 = attestation 범위 줄 · 원장 docs/logs/main/events/2026-10.jsonl) · 선언: sweep_index.meta.driver_version(output/multi/benchlog/sweep_loading_ds41-load-local-1m · 측정 시 조립 — sweep_bench.sh 가 manifest 에서 읽는다) (manifest 선언 옮김) |
| 호스트 OS | 미관측 | 미관측 |

> 재현 좌표는 빌드 입력(릴리스 태그 또는 40자 SHA)이다 — 엔진 자기보고 · 인증서 · wheel 의 버전 문자열은 빌드 입력이 아니다(각 값을 만든 생산자는 출처 칸).

**양자화 구성**(체크포인트 config 관측 · 이름의 `q` 축은 선언 방식 하나 — ModelOpt MIXED 는 층 개수 우세가 보조 규칙)

| 범위 | dtype | 출처 |
|---|---|---|
| quantization_config 선언 대상(양자화 층 · 제외 목록 밖) | `exl3 (weight_block_size=[32, 32])` | /app/quant_models/DeepSeek/DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz → <manifest.quant_model_path>/DeepSeek/DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz/config.json quantization_config quant_method |
| 기본 dtype(비양자화 가중치·계산) | `bfloat16` | /app/quant_models/DeepSeek/DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz → <manifest.quant_model_path>/DeepSeek/DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz/config.json dtype |

**이 모델에 필요한 패치**(01 §1.1 관련성 · ① 패치 선언 대상 × 모델 config ② 엔진 로그 발화 — 둘 다 관측일 때만 확정): required 0개 · inactive-inferred 0개 · unknown 18개(unknown = 판정 신호 부족 — 필요 여부를 이 표로 단정하지 않는다).
<!-- /FACT:resolved -->

> 이 절이 답하는 질문: 위 값들 중 무엇을 그대로 참조하고 무엇을 네 환경에서 다시 도출해야 하며, 이 빌드 트랙은 왜 골랐는가?

이번은 기존 source-build SM12x 후보 이미지를 재사용했다. stock0.32 EXL3 registry의 실제 거부와 common Engram 접점 변경을 최소 PRE/native POST로 메운 선례가 선택 근거이며 새 wheel/source 트랙 비교를 하지 않았다(선행 devlog_26103005 및W13~W15).
재현 좌표는5df63a7203cdb9328d824a27b682e77c7b9928be다. image label0.32.0, 패키지 SCM dev 문자열, benchmark의 버전 키는 각각 다른 생산물이다. 사실 블록의 노드별 실제 metadata와 SHA를 따른다. main/sub image ID 및 build-ledger identity differ와 SHA/torch/driver equal을 구별한다.
aarch64·SM12x arch 및 host driver는 recipient HW에서 다시 도출한다. parent core arch/jobs와 child native arch/jobs가 다른 계보를 하나의 빌드명령으로 합치지 않는다. 새 parent 재구성은 generated-unverified다. 호스트 좌표와 이미지 CUDA/torch 층도 구별한다.
공유 핵심 의존 핀은05 hub/pytest,06 B12X 및75 FLA/fla-core/marisa-trie와 provenance의 native source를 따른다. Ray·FlashInfer·Triton 등의 나머지 핀은 실린 build recipe/requirements·원장으로 읽는다. 이 값들을 전부 DeepSeek 전용 최소 요구라고 입증한 것은 아니다.

## 0.5 서빙 노브와 값의 지위
> 01-artifacts.md §1.3 의 `kind: value-status` 블록에서 기계가 생성한 요약이다. 지위가 `tuned` · `declared-requirement` 가 아닌 값은 **이 셀에서 조정된 적도, 빼면 깨진다고 확인된 적도 없다** — 필요조건으로 복사하지 마라. `declared-requirement` 는 빼거나 바꾸면 실패가 관측된 값이다(근거의 W id · artifacts 헤더를 보고 네 환경에서도 그 실패 조건이 성립하는지 판단한다).

<!-- FACT:knob_status -->
노브 21개 — `inherited` 21.

| 노브 | 값 | 지위 | 근거 | id |
|---|---|---|---|---|
| `tensor-parallel-size` | 2 | inherited | 이번 저장정책 비교의 고정 손레버 또는 선례 승계값; 현재 셀에서 조정 스윕·제거 실패 대조는 수행하지 않았다. | V1 |
| `distributed-executor-backend` | ray | inherited | 이번 저장정책 비교의 고정 손레버 또는 선례 승계값; 현재 셀에서 조정 스윕·제거 실패 대조는 수행하지 않았다. | V2 |
| `quantization` | exl3 | inherited | 이번 저장정책 비교의 고정 손레버 또는 선례 승계값; 현재 셀에서 조정 스윕·제거 실패 대조는 수행하지 않았다. | V3 |
| `tokenizer-mode` | deepseek_v41 | inherited | 이번 저장정책 비교의 고정 손레버 또는 선례 승계값; 현재 셀에서 조정 스윕·제거 실패 대조는 수행하지 않았다. | V4 |
| `hf-overrides` | {"engram_table_dir":"/app/quant_models/DeepSeek/DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz"} | inherited | 이번 저장정책 비교의 고정 손레버 또는 선례 승계값; 현재 셀에서 조정 스윕·제거 실패 대조는 수행하지 않았다. | V5 |
| `block-size` | 64 | inherited | 이번 저장정책 비교의 고정 손레버 또는 선례 승계값; 현재 셀에서 조정 스윕·제거 실패 대조는 수행하지 않았다. | V6 |
| `kernel-config` | {"sparse_indexer_topk_backend":"per_row"} | inherited | 이번 저장정책 비교의 고정 손레버 또는 선례 승계값; 현재 셀에서 조정 스윕·제거 실패 대조는 수행하지 않았다. | V7 |
| `kv-cache-dtype` | fp8 | inherited | 이번 저장정책 비교의 고정 손레버 또는 선례 승계값; 현재 셀에서 조정 스윕·제거 실패 대조는 수행하지 않았다. | V8 |
| `kv-cache-memory-bytes` | 8589934592 | inherited | 이번 저장정책 비교의 고정 손레버 또는 선례 승계값; 현재 셀에서 조정 스윕·제거 실패 대조는 수행하지 않았다. | V9 |
| `gpu-memory-utilization` | 0.8 | inherited | 이번 저장정책 비교의 고정 손레버 또는 선례 승계값; 현재 셀에서 조정 스윕·제거 실패 대조는 수행하지 않았다. | V10 |
| `max-model-len` | 1048576 | inherited | 이번 저장정책 비교의 고정 손레버 또는 선례 승계값; 현재 셀에서 조정 스윕·제거 실패 대조는 수행하지 않았다. | V11 |
| `max-num-seqs` | 1 | inherited | 이번 저장정책 비교의 고정 손레버 또는 선례 승계값; 현재 셀에서 조정 스윕·제거 실패 대조는 수행하지 않았다. | V12 |
| `max-num-batched-tokens` | 1536 | inherited | 이번 저장정책 비교의 고정 손레버 또는 선례 승계값; 현재 셀에서 조정 스윕·제거 실패 대조는 수행하지 않았다. | V13 |
| `enforce-eager` | true | inherited | 이번 저장정책 비교의 고정 손레버 또는 선례 승계값; 현재 셀에서 조정 스윕·제거 실패 대조는 수행하지 않았다. | V14 |
| `no-enable-flashinfer-autotune` | true | inherited | 이번 저장정책 비교의 고정 손레버 또는 선례 승계값; 현재 셀에서 조정 스윕·제거 실패 대조는 수행하지 않았다. | V15 |
| `reasoning-parser` | deepseek_v41 | inherited | 이번 저장정책 비교의 고정 손레버 또는 선례 승계값; 현재 셀에서 조정 스윕·제거 실패 대조는 수행하지 않았다. | V16 |
| `tool-call-parser` | deepseek_v41 | inherited | 이번 저장정책 비교의 고정 손레버 또는 선례 승계값; 현재 셀에서 조정 스윕·제거 실패 대조는 수행하지 않았다. | V17 |
| `enable-auto-tool-choice` | true | inherited | 이번 저장정책 비교의 고정 손레버 또는 선례 승계값; 현재 셀에서 조정 스윕·제거 실패 대조는 수행하지 않았다. | V18 |
| `speculative-config` | {"method":"dspark","num_speculative_tokens":3,"draft_sample_method":"greedy"} | inherited | 이번 저장정책 비교의 고정 손레버 또는 선례 승계값; 현재 셀에서 조정 스윕·제거 실패 대조는 수행하지 않았다. | V19 |
| `language-model-only` | true | inherited | 이번 저장정책 비교의 고정 손레버 또는 선례 승계값; 현재 셀에서 조정 스윕·제거 실패 대조는 수행하지 않았다. | V20 |
| `engram-config` | {"cpu_offload": true, "dp_shared_memory": false, "use_thp": false} | inherited | 이번 저장정책 비교의 고정 손레버 또는 선례 승계값; 현재 셀에서 조정 스윕·제거 실패 대조는 수행하지 않았다. | V21 |
<!-- /FACT:knob_status -->

> 이 절이 답하는 질문: 이 레시피를 다른 환경으로 옮길 때 어떤 노브를 먼저 다시 재야 하고, 어떤 노브는 빼면 깨지는가?

① recipient에서 재도출할 값은 절대KV·gmu의 실제 엔진 해석·context·max-num-seqs·batched tokens·호스트/벤치 예산이다. 현재 token capacity를 actual C2 성능 또는 최소KV 배수로 읽지 않는다.
② 현재 셀에서 knob 제거 실패 대조로 확정한 declared-requirement는 없다. EXL3 stock 부재와 source 가드는W13/W14 및 해당 파일의 종료 경로에 한정해 읽고 상위 버전에서 재검증한다.
③ V 블록은 현재 고정 손레버/선례 승계값이다. DSpark3의 실제 acceptance와 설정k를 구별하고, 과거131K acceptance나0.30 tuned 지위를 현재로 옮기지 않는다.

## 0.6 재검증 · 라우팅
- 이 자료를 `vllm-recipe-explorer` 인터뷰의 **warm-start 근거로만** 투입하고, 평소대로 plan-gate → 레시피 수렴 → 스모크 게이트를 그대로 밟아라(가속이지 우회 ✗).

> 이 절이 답하는 질문: 다른 환경(다른 HW · 다른 vLLM 버전 · 다른 노드 수)에서 이 지도를 쓸 때 무엇부터, 어떤 순서로 확인해야 하며, 실패하면 어디로 돌리는가?

1. config/index와 metadata revision·파일명/크기/SHA를 확인한다 → payload나 quant 구성이 달라지면 새 모델 입력으로 그라운딩한다.
2. EXL3 registry·common Engram/V2·patch 대상심벌과 상위 지원을 읽는다 → 그대로 carry-forward하지 않고 실제 skip/fail 분기에 따라 이식 범위를 재도출한다.
3. 양 노드의 실제 ro bind·DSV41_SOURCE/hf table dir·SHA/torch/driver와 runtime env/actor 전달을 확인한다 → 다르면 모델 source나 노드 통로부터 정합한다.
4. weights·KV·overhead의 호스트 예산을 선언·honored 후 기동한다 → 부족하면 가드 우회 대신 입력을 재도출한다. declared floor와 엔진 RAM/용량 관측을 구별한다.
5. health200 +1M 모델 목록 +짧은 추론 +EXL3/file-backed/iterator skip/prestage 경로를 확인한 뒤 lite/full3회를 별도로 측정한다 → 장문/C2나 다른 부하를 주장하려면 새 검증이 필요하다.
빌드·버전은 upstream-version-watch, 설정/KV는 vllm-recipe-explorer, 처리량 기대 미달은 adversarial-benchmark, 노드/인터커넥트는 terraforming_node가 소유한다.

## 0.7 메타
<!-- FACT:meta -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.32.0-g5df63a7203cd/deepseek-v4.1-flash-exl3-2.0bpw-sfxnz/gb10-1g2n-cluster-native/qexl3-len1048576-kvfp8` |
| 형식 | `hint-payload/v7` |
| 앵커 | `PROVENANCE.json` — 봉인 때 hint 브랜치 페이로드 커밋에 묶인다 |
| 캠페인 · 셀 · 노드 | camp-26103017-nas-nvme-loading · ds41-load-local-1m · cluster |
| 발행 모드 | campaign |
| 발행 토픽 | camp_26103017_nas_nvme_loading__cluster__ds41_load_local_1m |
| work-manifest | docs/_evidence/camp_26103017_nas_nvme_loading__cluster__ds41_load_local_1m.work-manifest.json |
| 증거 등급(task_class) | full_benchmark |
| 승인 출처 | campaign:hint_targets |
| 승인 시각(UTC) | 2026-10-30T08:39:00Z |
| 승인 발화(전사) | 사용자: hint태그도 발행해야 하니 full bench도 수행해줘. 범위2(모델별 대표 정책에서 full bench 및 hint) 선택. 대표 정책③을 포함한 실행 계획에 서브 모델 지정 후 진행 지시. |
| 현행 full 정의 충족(측정 등급) | 예 · 이 측정의 반복 3 · 정의 출처 `.claude/rules/docs.md:42`(03 §3.5) |
| 독립 사실 검증(발행 전) | 통과 — 검증자 `independent-factchecker`(저작자 `main-author` 아님) · 주장 1043건 · 지적 2건 중 고침 2건 · 열림 0 · 2026-10-30T16:06:58Z |
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
**확정 이름** `hint/0.32.0-g5df63a7203cd/deepseek-v4.1-flash-exl3-2.0bpw-sfxnz/gb10-1g2n-cluster-native/qexl3-len1048576-kvfp8` — 꼬리 0토큰.

_빈 꼬리 — 발행 Agent 가 가르는 노브가 없다고 명시했다(`inputs/tail.json` = `[]`)._
<!-- /FACT:name_tail -->

> 이 절이 답하는 질문: 이 셀을 같은 q·len·kv 의 다른 셀과 가르는 노브는 무엇이며 그것을 이름 꼬리로 어떻게 적는가 — 가르는 노브가 없으면 왜 빈 꼬리인가?

inputs/tail.json은 빈 목록이다. 기본 축 이외에 resident/spec/graph의 자동 세 토큰 묶음을 반복하지 않는다. 현재 file-backed/DSpark/eager 및 값의 지위는 서사와 표에 남기고, 같은 기본 이름의 충돌과 시각 접미사는 도구가 결정한다.
