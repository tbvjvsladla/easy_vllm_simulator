# 00 · hint 지도 — 가장 먼저 읽는 파일

<!-- FACT:header -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.32.0-g5df63a7203cd/deepseek-v4.1-flash-exl3-2.0bpw-sfxnz/gb10-1g2n-cluster-native/qexl3-len131072-kvfp8-eager` |
| 형식 | `hint-payload/v7` · 이름 문법 `v7` |
| 생성(UTC · 주입) | 2026-10-29T20:38:57Z |
| 캠페인 · 셀 · 노드 · 모드 | camp-26102919-codex-multi4-v032 · i5-032-sm12x · cluster · campaign |
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
| 자격 근거 | `output/multi/benchlog/serve_proof_i5-032-sm12x.json` — 시점 범위: 작성 2026-10-29T16:31:49Z(output/multi/benchlog/serve_proof_i5-032-sm12x.json 파일 mtime(파일시스템 관측 — 문서에 시각 칸이 없다 · 기록 필드 아님)) · 측정 창 대비 작성 2026-10-29T16:31:49Z < 측정 시작 2026-10-29T16:32:21Z — 측정 전(빌드·스모크)의 관측 |
| 증거 등급(task_class) | full_benchmark |
| 측정 등급(bench_mode) | full |
| 셀 등급(cell_grade) | `full` — 성능 판정 · 부하 곡선 · 데이터셋 출처는 `03-benchmark.md` |
| 성능 판정(measurement.verdict) | `REFUTE` — 출처 output/multi/benchlog/sweep_i5-032-sm12x_261030A/verdict.json(output · verdict_rule 출력) verdict |

> **REFUTE** — 이 셀의 성능 판정은 **기각**이다(측정은 됐으나 판정 기준 미달). 수치는 관측이지 baseline · 권고가 아니다 — 판정 출처 output/multi/benchlog/sweep_i5-032-sm12x_261030A/verdict.json(output · verdict_rule 출력) verdict.
> 기각 사유: M(24.04) < 43.73×(1−0.15)=37.17 [E(external_reference)] (ratio 0.55) → 기대 이하
<!-- /FACT:grade -->

<!-- FACT:lite_summary -->
<!-- /FACT:lite_summary -->

## 0.1 요약
> 이 절이 답하는 질문: 이 셀은 무엇을 서빙했고, 무엇이 결정적이었으며, 수신자는 무엇을 가장 조심해야 하는가?

DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz를GB10 노드당GPU하나×두노드TP2,공식vLLM0.32.0 exactSHA 기반SM12x후보에서EXL3·131072·KVfp8·fileEngram·DSpark3·eager로측정해동시성1 decode24.04t/s의weakREFUTE를기록했다. commonEngram/V2이식과native의존결손은새PRE/POST로해소했지만(W11/W12)과거graph10kernel성공은승계하지않았다. KV/context/seq는손레버inherited(V9/V11/V12)이며회귀부재로CANDIDATE를유지한다.

## 0.2 유효맥락
<!-- FACT:context -->
| 항목 | 값 | 출처 |
|---|---|---|
| 모델 | deepseek-v4.1-flash-exl3-2.0bpw-sfxnz | sweep_index.meta(output/multi/benchlog/sweep_i5-032-sm12x_261030A) |
| HF repo | 미관측 | 체크포인트 .git/config 부재(/app/quant_models/DeepSeek/DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz → <manifest.quant_model_path>/DeepSeek/DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz) — HF repo 미관측 |
| HF revision(체크포인트 git HEAD) | 미관측 | 체크포인트 .git/HEAD 부재(/app/quant_models/DeepSeek/DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz → <manifest.quant_model_path>/DeepSeek/DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz) — revision 미관측 |
| base_model | deepseek-ai/DeepSeek-V4.1-Flash | 체크포인트 README base_model(/app/quant_models/DeepSeek/DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz → <manifest.quant_model_path>/DeepSeek/DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz) |
| base 슬러그 | deepseek-v4.1-flash-exl3-2.0bpw-sfxnz | model(deepseek-v4.1-flash-exl3-2.0bpw-sfxnz) − vocab quant_suffixes(naming.base_slug) |
| 양자화 | exl3 | sweep_index.meta(output/multi/benchlog/sweep_i5-032-sm12x_261030A) |
| GPU | NVIDIA GB10 | sweep_index.meta(output/multi/benchlog/sweep_i5-032-sm12x_261030A) |
| 토폴로지 · TP | multi · TP=2 | sweep_index.meta(output/multi/benchlog/sweep_i5-032-sm12x_261030A) |
| 실행 평면 | docker | artifacts.plane_of |
| vLLM(측정 강식별 키 `vllm_version`) | 0.32.0 | sweep_index.meta(output/multi/benchlog/sweep_i5-032-sm12x_261030A) |

**명명 축** — 태그 이름 = 결정론부(vllm · model · arch · q · len · kv — 도구가 증거에서 파생 · 발행자 입력 ✗) + 꼬리(발행 Agent 가 고르고 서빙 설정 file · key · value 로 근거 대조 · 0.9) + 중복 시 timestamp.

| 축 | 값 | 출처 |
|---|---|---|
| 세그먼트 `vllm` | 0.32.0-g5df63a7203cd | track=선택자 Dockerfile.source-build-sm12x · ref=docker history build-arg VLLM_REF · version=VLLM_VERSION 미관측 · repo=docker history build-arg VLLM_REPO · sha=VLLM_REF(40자 커밋 · docker history build-arg VLLM_REF) · exact_tag=measured image sha256:436bd187022965d2e41f8969695d8446baeca6c5d50692d8b6db76ae2877a916: /workspace/vllm-src/.git HEAD and exact tag peeled to 5df63a7203cdb9328d824a27b682e77c7b9928be (create/cp, no start) · VLLM_REF=5df63a7203cd…(40자 커밋) → <직전 릴리스 0.32.0>-g<sha12> |
| 세그먼트 `model` | deepseek-v4.1-flash-exl3-2.0bpw-sfxnz | 서빙 yaml model(output/multi/configs/i5-032-sm12x.yaml) · model_path basename(DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz) 소문자 = 인증서 model 키 규칙 |
| 세그먼트 `arch` | gb10-1g2n-cluster-native | derived(hw·gpus_per_node·nodes·role·target·plane) |
| 세그먼트 `recipe` | qexl3-len131072-kvfp8-eager | derived(q·len·kv) + tail(agent · naming.tail[] 근거) |
| 축 `hw` | gb10 | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(자동 파생 — 측정 TP=2(sweep_index.meta.tensor_parallel_size(output/multi/benchlog/sweep_i5-032-sm12x_261030A · generated_utc = 측정 키)) > gpus_per_node=1(output/multi/manifest.yaml(manifest_contract 로더)) · 단일 노드 형상일 수 없다 · 일치 신호: 멀티 serve proof output/multi/benchlog/serve_proof_i5-032-sm12x.json(kind=multinode_serve_proof) · 노드 정합 attestation output/multi/benchlog/attestation_i5-032-sm12x.json) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) · vocab hw[gb10]←'NVIDIA GB10' |
| 축 `gpus_per_node` | 1 | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(자동 파생 — 측정 TP=2(sweep_index.meta.tensor_parallel_size(output/multi/benchlog/sweep_i5-032-sm12x_261030A · generated_utc = 측정 키)) > gpus_per_node=1(output/multi/manifest.yaml(manifest_contract 로더)) · 단일 노드 형상일 수 없다 · 일치 신호: 멀티 serve proof output/multi/benchlog/serve_proof_i5-032-sm12x.json(kind=multinode_serve_proof) · 노드 정합 attestation output/multi/benchlog/attestation_i5-032-sm12x.json) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `nodes` | 2 | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(자동 파생 — 측정 TP=2(sweep_index.meta.tensor_parallel_size(output/multi/benchlog/sweep_i5-032-sm12x_261030A · generated_utc = 측정 키)) > gpus_per_node=1(output/multi/manifest.yaml(manifest_contract 로더)) · 단일 노드 형상일 수 없다 · 일치 신호: 멀티 serve proof output/multi/benchlog/serve_proof_i5-032-sm12x.json(kind=multinode_serve_proof) · 노드 정합 attestation output/multi/benchlog/attestation_i5-032-sm12x.json) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `role` | cluster | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(자동 파생 — 측정 TP=2(sweep_index.meta.tensor_parallel_size(output/multi/benchlog/sweep_i5-032-sm12x_261030A · generated_utc = 측정 키)) > gpus_per_node=1(output/multi/manifest.yaml(manifest_contract 로더)) · 단일 노드 형상일 수 없다 · 일치 신호: 멀티 serve proof output/multi/benchlog/serve_proof_i5-032-sm12x.json(kind=multinode_serve_proof) · 노드 정합 attestation output/multi/benchlog/attestation_i5-032-sm12x.json) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `target` | native | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(자동 파생 — 측정 TP=2(sweep_index.meta.tensor_parallel_size(output/multi/benchlog/sweep_i5-032-sm12x_261030A · generated_utc = 측정 키)) > gpus_per_node=1(output/multi/manifest.yaml(manifest_contract 로더)) · 단일 노드 형상일 수 없다 · 일치 신호: 멀티 serve proof output/multi/benchlog/serve_proof_i5-032-sm12x.json(kind=multinode_serve_proof) · 노드 정합 attestation output/multi/benchlog/attestation_i5-032-sm12x.json) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) · target_gpu 'NVIDIA GB10' = 호스트 hw → native |
| 축 `plane` | docker | artifacts.plane_of → cell-env(IMAGE_TAG\|BUILD_DOCKERFILE) · vocab plane[docker]→'' |
| 축 `q` | exl3 | /app/quant_models/DeepSeek/DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz → <manifest.quant_model_path>/DeepSeek/DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz/config.json quantization_config quant_method=exl3 · vocab quant[exl3]←'exl3' |
| 축 `len` | 131072 | 서빙 yaml max-model-len(output/multi/configs/i5-032-sm12x.yaml) |
| 축 `kv` | fp8 | 서빙 yaml kv-cache-dtype(output/multi/configs/i5-032-sm12x.yaml) · vocab kv[fp8]←'fp8' |
| 꼬리 `eager` | 현재 최소 port의 eager 실행; graph 형상과 구분 | `output/multi/configs/i5-032-sm12x.yaml` · 키 `enforce-eager` = `true` |
<!-- /FACT:context -->

> 이 절이 답하는 질문: 이 지도는 어떤 조건에서만 성립하며, 이 형상은 어떤 기전으로 버티고 다른 형상은 왜 실패했는가?

(가) GB10통합메모리에서EXL3target/draft73.96GiB/노드와KV8GiB,호스트overhead/filecache가같은물리풀을쓴다. fileEngram과iterator skip/MCG실관측이성립조건이며CPUoffload만으로새용량이생기지않는다(testlog_26103005_i5_032_TP2와_Full_판정). 노드수/메모리구조가달라지면예산과통신을다시도출한다.
팩의EXL3MCG bits2/routed384/lmheadMXFP8와filetable행조회가서로다른경로다. qexl3를모든텐서가같은저장/연산형식이라고읽지않는다. Engramper-token제외는물리상주삭제가아니고정확packrevision도미관측이다(devlog_26103005_i5_032_최소이식과_실측_인계).
(나) 과거stockEngram상주는UMA산술/설계벽,graphNaN은KVnull대조,열c2kill은실제집행으로구분한다(W2/W4/W10). 현재eager/131k/seq1/optionaloff등여러변경중하나가모든메모리·처리량차이를만들었다고하지않는다. 현재적용74/89도runtime해당경로는off이며옛graph10kernel성공을현재최소port의검증으로쓰지않는다.

## 0.3 벽 지도 요약
> 02-narrative.md §2.2 의 `kind: wall` 블록(과 §2.4 의 `kind: misdiagnosis` 블록)에서 기계가 생성한 표다(손저작 ✗ · 순서 = 넘은 순서). 원인·해소의 전문과 원문 발췌는 02 에 있다.

<!-- FACT:wall_map -->
| 순서 | id | 증상 | 해소 | 전이등급 | 검증 |
|---|---|---|---|---|---|
| 1 | W1 | 역사0.30계보의 stock0.30 EXL3 경로 부재 | 역사POST70/73·PRE80/81·79이식 | arch-invariant | 역사 plan_26100208_V4_1_Flash_멀티노드_1M_캠페인의후속관측;이번0.32성공에합산하지않음 |
| 2 | W2 | 역사0.30계보의 Engram 통합메모리 상주 불가 설계 | 역사filebackend/row_store/embed skip 및로컬Engramdir | arch-scaled | 역사 plan_26100319_V4_1_Flash_2노드_광의탐색_캠페인_재설계의후속관측;이번0.32성공에합산하지않음 |
| 3 | W3 | 역사0.30계보의 역사1M 장문 열 하드다운 | 역사세션열차단기/장문상한125k;가드우회없음 | arch-scaled | 역사 plan_26100319_V4_1_Flash_2노드_광의탐색_캠페인_재설계의후속관측;이번0.32성공에합산하지않음 |
| 4 | W4 | 역사0.30계보의 역사graph logits NaN | 역사89capture후zero;88은capture밖Engram조회정합경로 | arch-invariant | 역사 devlog_26100610_V4_1_Flash_트랙K_커널레버_이식과_정식화의후속관측;이번0.32성공에합산하지않음 |
| 5 | W5 | 역사0.30계보의 역사의존wheel 서명URL 만료 | 역사55청크리다이렉트/Range이어받기/해시검증 | arch-invariant | 역사 devlog_26100610_V4_1_Flash_트랙K_커널레버_이식과_정식화의후속관측;이번0.32성공에합산하지않음 |
| 6 | W6 | 역사0.30계보의 역사graph 이후 성능 격차 | 역사73p2b체인/90wo_a및관련env | arch-invariant | 역사 devlog_26100610_V4_1_Flash_트랙K_커널레버_이식과_정식화의후속관측;이번0.32성공에합산하지않음 |
| 7 | W7 | 역사0.30계보의 역사dense MXFP8 cutlass 비용 | 역사91preferB12X/auto | arch-invariant | 역사 devlog_26100610_V4_1_Flash_트랙K_커널레버_이식과_정식화의후속관측;이번0.32성공에합산하지않음 |
| 8 | W8 | 역사0.30계보의 역사small-M dense GEMV 레버 | 역사92/90/91묶음과env | arch-invariant | 역사 devlog_26100610_V4_1_Flash_트랙K_커널레버_이식과_정식화의후속관측;이번0.32성공에합산하지않음 |
| 9 | W9 | 역사0.30계보의 역사MoE prep glue 비용 | 역사74fusedprep및envon | arch-invariant | 역사 devlog_26100610_V4_1_Flash_트랙K_커널레버_이식과_정식화의후속관측;이번0.32성공에합산하지않음 |
| 10 | W10 | 역사0.30계보의 역사Full c2 열 중단 | 해당스윕Full근거제외,역사c1×3로별도실측 | arch-scaled | 역사 testlog_26100510_V4_1_Flash_i5_a_kern_정식레시피_판정의후속관측;이번0.32성공에합산하지않음 |
| 11 | W11 | 새0.32 common Engram 접점 변경 | PRE83/88common클래스및V2가드재도출,84옛네앵커제거/공식workspace노브 | judgment | testlog_26103005_i5_032_TP2와_Full_판정 및 devlog_26103005_i5_032_최소이식과_실측_인계 |
| 12 | W12 | 새native attempt1 pipcheck 의존결손 | POST75 SHA고정3wheel설치후finalattempt2 native실제재컴파일 | judgment | testlog_26103005_i5_032_TP2와_Full_판정 및 devlog_26103005_i5_032_최소이식과_실측_인계 |
| 13 | W13 | 새선택성능90~92 제외 | 최소port만선택;실제로90~92 GPU성공이아님 | judgment | testlog_26103005_i5_032_TP2와_Full_판정 및 devlog_26103005_i5_032_최소이식과_실측_인계 |
| 14 | W14 | 현재 사전 외부E 미달 | 미해소기록/기준유지,추가최적화성공합성없음 | judgment | testlog_26103005_i5_032_TP2와_Full_판정 및 devlog_26103005_i5_032_최소이식과_실측_인계 |

**오진 · 정정**(02 §2.4 — 현재지위가 `가설(강등)` 인 기전은 처방이 아니다)

| id | 증상 | 현재지위 | 검증 |
|---|---|---|---|
| M1 | 역사남은37ms를동기Engram prestage탓으로의심 | 반증 | devlog_26100610_V4_1_Flash_트랙K_커널레버_이식과_정식화 §2. 무엇을 했나(순서대로) |
| M2 | 역사EXL3/Engram을200GiB active로계산한낮은루브릭 | 반증 | devlog_26103005_i5_032_최소이식과_실측_인계의행조회정정 및현재Rfp44.32 |
<!-- /FACT:wall_map -->

## 0.4 결정론 해소값
<!-- FACT:resolved -->
| 항목 | 값 | 출처 |
|---|---|---|
| vLLM 빌드 입력(종류) | commit | naming.vllm_build_input |
| vLLM 빌드 입력(ref) | 5df63a7203cdb9328d824a27b682e77c7b9928be | docker history build-arg VLLM_REF |
| vLLM 빌드 계보 SHA(40자) | 5df63a7203cdb9328d824a27b682e77c7b9928be | VLLM_REF(40자 커밋 · docker history build-arg VLLM_REF) |
| 직전 릴리스(prev_release) | 0.32.0 | naming.vllm_build_input |
| vLLM 엔진 자기보고(엔진 로그 기동 배너) | 0.32.1.dev0+g5df63a720.d20261029 | output/multi/benchlog/sweep_i5-032-sm12x_261030A/lite_engine_head_i5-032-sm12x.log:59 — 측정 캡처에 기록된 값 · 현재 serve 세션 경계 미관측 — 누적 로그의 기동 epoch 귀속 미검증 · 같은 값의 다른 로그 1건 · 지위 `measurement-capture-unbound` |
| 인증서 `vllm_version`(강한 일치 키) | 미관측 | 생산자 `absent` · 인증서 vllm_version 없음 |
| vLLM 설치 distribution 메타(출처별 관측 · 다운로드 wheel 증명 아님) | 노드별 상이·공통값 미선택(각 값은 출처) | 노드별 상이 — 값을 고르지 않는다: attestation nodes.<n>.vllm_dist_version(output/multi/benchlog/attestation_i5-032-sm12x.json · 노드별 관측) — ⚠ 노드별 상이(main=0.32.1.dev0+g5df63a720.d20261029.cu133 · sub=0.32.1.dev0+g5df63a720.d20261028.cu133 · 값 = main) · 작성 2026-10-29T16:31:49Z < 측정 시작 2026-10-29T16:32:21Z — 측정 전(빌드·스모크)의 관측 · 원장 순서로 판별한 같은 기동의 관측(결속된 스모크 로그 미확인 · 제한검색 · 약한 판별)(판별 근거 = attestation 범위 줄 · 원장 docs/logs/main/events/2026-10.jsonl)(sweep_index.meta.vllm_build='NA' 는 sweep_bench 가 엔진 로그에서 `v<x.y.z…>` 정규식으로 잡는 값이다 → engine_self_report 의 원천) · 지위 `observed-node-difference` |
| 체크포인트 revision(HF · git HEAD) | 미관측 | 미관측 |
| 빌드 트랙 | source-build | artifacts 선택자(build-ledger.dockerfile) |
| Dockerfile(선택자) | Dockerfile.source-build-sm12x | build-ledger.dockerfile |
| vLLM 저장소 | https://github.com/vllm-project/vllm.git | docker history build-arg VLLM_REPO |
| 이미지 태그 | easy-vllm:0.32.0-cu133-aarch64-source-sm12x | 셀 env(output/multi/envs/.env.i5-032-sm12x) IMAGE_TAG |
| 이미지 digest | sha256:436bd187022965d2e41f8969695d8446baeca6c5d50692d8b6db76ae2877a916 | sweep_index.meta.image_digest(measured(docker inspect .Image)) |
| torch | 2.13.0a0+9186a08 | docker image inspect Config.Env PYTORCH_VERSION |
| CUDA | 13.3.1.008 | docker image inspect Config.Env CUDA_VERSION |
| NGC 베이스 | 26.07 | docker image inspect Config.Env NVIDIA_PYTORCH_VERSION |
| CPU arch | arm64 | docker image inspect Architecture |
| 드라이버 | 580.178.04 | attestation parity.driver(output/multi/benchlog/attestation_i5-032-sm12x.json · 노드별 관측 · verdict=equal) — main=sub 일치 · 작성 2026-10-29T16:31:49Z < 측정 시작 2026-10-29T16:32:21Z — 측정 전(빌드·스모크)의 관측 · 원장 순서로 판별한 같은 기동의 관측(결속된 스모크 로그 미확인 · 제한검색 · 약한 판별)(판별 근거 = attestation 범위 줄 · 원장 docs/logs/main/events/2026-10.jsonl) |
| 드라이버(노드별 관측) | main=580.178.04 · sub=580.178.04 | attestation parity.driver(output/multi/benchlog/attestation_i5-032-sm12x.json · 노드별 관측 · verdict=equal) — main=sub 일치 · 작성 2026-10-29T16:31:49Z < 측정 시작 2026-10-29T16:32:21Z — 측정 전(빌드·스모크)의 관측 · 원장 순서로 판별한 같은 기동의 관측(결속된 스모크 로그 미확인 · 제한검색 · 약한 판별)(판별 근거 = attestation 범위 줄 · 원장 docs/logs/main/events/2026-10.jsonl) |
| ⚠ 드라이버 관측 ≠ 선언 | 충돌 없음 — 관측 580.178.04 = 선언 580.178.04 | 관측: attestation parity.driver(output/multi/benchlog/attestation_i5-032-sm12x.json · 노드별 관측 · verdict=equal) — main=sub 일치 · 작성 2026-10-29T16:31:49Z < 측정 시작 2026-10-29T16:32:21Z — 측정 전(빌드·스모크)의 관측 · 원장 순서로 판별한 같은 기동의 관측(결속된 스모크 로그 미확인 · 제한검색 · 약한 판별)(판별 근거 = attestation 범위 줄 · 원장 docs/logs/main/events/2026-10.jsonl) · 선언: sweep_index.meta.driver_version(output/multi/benchlog/sweep_i5-032-sm12x_261030A · 측정 시 조립 — sweep_bench.sh 가 manifest 에서 읽는다) (manifest 선언 옮김) |
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

1. 이셀은node-local공식0.32source-deps2위SM12xchild다. 원본Dockerfile/parentmanifest가선택트랙의근거이며corecsrc/CMake/setup불변을검사했다. core전체재빌드없이PRE Python수정과별도EXL3nativePOST컴파일을했다. sourcecore12.1/jobs8과child12.1a/jobs4는실제다른명령이다.
2. 재현vLLM좌표는공식v0.32.0의40SHA `5df63a7203cdb9328d824a27b682e77c7b9928be`다. 원장gitdescribe/보고서0.32.0은빌드입력·측정메타,SCMdev/날짜표시는패키지생산자다. 노드별날짜차이를다른SHA의증거로쓰지않는다. env는원실행스냅샷보존후actualSHA/arch/jobs만documentary정정됐고raw/노브/이미지를바꾸지않았다(testlog_26103005_i5_032_TP2와_Full_판정).
3. arm64/GB10SM12.1/driver580.178.04는HW결속이며nativearch12.1a를다른GPU에복사하지않는다. 실제GPUcapability/메모리와targettorchABI로다시도출한다. imageCUDA13.3.1.008/torch2.13.0a0+9186a08과reportcuda132/호스트driver는다른층이다.
4. core5/context→sameIDalias→deps2중간2/context→childfinal24의기계결속과새조립README명령의미실행지위를구분한다. rawDockerfile25fca737…와공식header추가배포b93b29ea…는범위가다르고generated-unverified를유지한다. sub원core미회수/과거수리층차이와wholeimage동등미증명을남긴다.
5. 호스트MemTotal124608은러너실측,OS/커널은미기록,driver/ABI는attestation범위줄을읽는다. 결속된스모크로그미확인은한정검색이며다른경로보존부재가아니다. RoCE실효값은측정env/NCCL관측을정본으로한다.
6. 공유POST05hub1.33/pytest9.2,06B12X1.5.1,75FLA0.6/fla-core0.6/marisa1.4.1은실제pin과폐포다. exllamav31.4.5/Mia/plugin/p2b의정확원문커밋·tarball/파일해시는leafPROVENANCE/helper가권위다. Ray/FlashInfer/Triton/DeepGEMM은부모requirements/원장의핀을읽고옛0.30버전을임의대입하지않는다. 적용된patch와runtime활성/성능기여는다르다.

## 0.5 서빙 노브와 값의 지위
> 01-artifacts.md §1.3 의 `kind: value-status` 블록에서 기계가 생성한 요약이다. 지위가 `tuned` · `declared-requirement` 가 아닌 값은 **이 셀에서 조정된 적도, 빼면 깨진다고 확인된 적도 없다** — 필요조건으로 복사하지 마라. `declared-requirement` 는 빼거나 바꾸면 실패가 관측된 값이다(근거의 W id · artifacts 헤더를 보고 네 환경에서도 그 실패 조건이 성립하는지 판단한다).

<!-- FACT:knob_status -->
노브 21개 — `inherited` 21.

| 노브 | 값 | 지위 | 근거 | id |
|---|---|---|---|---|
| `tensor-parallel-size` | 2 | inherited | 현재0.32최소port안전형상의손레버/모델별접점선택;이셀값변경스윕·제거실패대조없음 | V1 |
| `distributed-executor-backend` | ray | inherited | 현재0.32최소port안전형상의손레버/모델별접점선택;이셀값변경스윕·제거실패대조없음 | V2 |
| `quantization` | exl3 | inherited | 현재0.32최소port안전형상의손레버/모델별접점선택;이셀값변경스윕·제거실패대조없음 | V3 |
| `tokenizer-mode` | deepseek_v41 | inherited | 현재0.32최소port안전형상의손레버/모델별접점선택;이셀값변경스윕·제거실패대조없음 | V4 |
| `hf-overrides` | {"engram_table_dir":"/app/ple_mmap/v41-engram-a"} | inherited | 현재0.32최소port안전형상의손레버/모델별접점선택;이셀값변경스윕·제거실패대조없음 | V5 |
| `block-size` | 64 | inherited | 현재0.32최소port안전형상의손레버/모델별접점선택;이셀값변경스윕·제거실패대조없음 | V6 |
| `kernel-config` | {"sparse_indexer_topk_backend":"per_row"} | inherited | 현재0.32최소port안전형상의손레버/모델별접점선택;이셀값변경스윕·제거실패대조없음 | V7 |
| `kv-cache-dtype` | fp8 | inherited | 현재0.32최소port안전형상의손레버/모델별접점선택;이셀값변경스윕·제거실패대조없음 | V8 |
| `kv-cache-memory-bytes` | 8589934592 | inherited | 현재0.32최소port안전형상의손레버/모델별접점선택;이셀값변경스윕·제거실패대조없음 | V9 |
| `gpu-memory-utilization` | 0.8 | inherited | 현재0.32최소port안전형상의손레버/모델별접점선택;이셀값변경스윕·제거실패대조없음 | V10 |
| `max-model-len` | 131072 | inherited | 현재0.32최소port안전형상의손레버/모델별접점선택;이셀값변경스윕·제거실패대조없음 | V11 |
| `max-num-seqs` | 1 | inherited | 현재0.32최소port안전형상의손레버/모델별접점선택;이셀값변경스윕·제거실패대조없음 | V12 |
| `max-num-batched-tokens` | 1536 | inherited | 현재0.32최소port안전형상의손레버/모델별접점선택;이셀값변경스윕·제거실패대조없음 | V13 |
| `enforce-eager` | true | inherited | 현재0.32최소port안전형상의손레버/모델별접점선택;이셀값변경스윕·제거실패대조없음 | V14 |
| `no-enable-flashinfer-autotune` | true | inherited | 현재0.32최소port안전형상의손레버/모델별접점선택;이셀값변경스윕·제거실패대조없음 | V15 |
| `reasoning-parser` | deepseek_v41 | inherited | 현재0.32최소port안전형상의손레버/모델별접점선택;이셀값변경스윕·제거실패대조없음 | V16 |
| `tool-call-parser` | deepseek_v41 | inherited | 현재0.32최소port안전형상의손레버/모델별접점선택;이셀값변경스윕·제거실패대조없음 | V17 |
| `enable-auto-tool-choice` | true | inherited | 현재0.32최소port안전형상의손레버/모델별접점선택;이셀값변경스윕·제거실패대조없음 | V18 |
| `speculative-config` | {"method":"dspark","num_speculative_tokens":3,"draft_sample_method":"greedy"} | inherited | 현재0.32최소port안전형상의손레버/모델별접점선택;이셀값변경스윕·제거실패대조없음 | V19 |
| `language-model-only` | true | inherited | 현재0.32최소port안전형상의손레버/모델별접점선택;이셀값변경스윕·제거실패대조없음 | V20 |
| `engram-config` | {"cpu_offload": true, "dp_shared_memory": false, "use_thp": false} | inherited | 현재0.32최소port안전형상의손레버/모델별접점선택;이셀값변경스윕·제거실패대조없음 | V21 |
<!-- /FACT:knob_status -->

> 이 절이 답하는 질문: 이 레시피를 다른 환경으로 옮길 때 어떤 노브를 먼저 다시 재야 하고, 어떤 노브는 빼면 깨지는가?

① KV/GMU/context/seq/prefill은MemTotal−가중치−KV−overhead/bench로호스트바닥을다시도출하고엔진KV토큰/목표부하를대조할값이다. fixedKV는GPU당이며GMU가그클램프를다시정하는것이아니다.
② 이0.32셀에서노브를빼거나바꿔실패한대조는없어declared-requirement는없다. fileEngram/TP2DP1/use_thpfalse/dpsharedfalse/V2지원가드는소스상범위와실행조건이며현재제거실패실험으로승격하지않는다. language-only와noasync/noprefix도선택범위다.
③ V1~V21은inherited손레버/모델별접점선택이다. eager/spec3/greedy/COOP2/WOAPREPACK1과optionaloff는최적설정탐색완료값이아니다. 과거graph/90~92의튜닝기록이있어도현재0.32의같은지위로복사하지않는다.

## 0.6 재검증 · 라우팅
- 이 자료를 `vllm-recipe-explorer` 인터뷰의 **warm-start 근거로만** 투입하고, 평소대로 plan-gate → 레시피 수렴 → 스모크 게이트를 그대로 밟아라(가속이지 우회 ✗).

> 이 절이 답하는 질문: 다른 환경(다른 HW · 다른 vLLM 버전 · 다른 노드 수)에서 이 지도를 쓸 때 무엇부터, 어떤 순서로 확인해야 하며, 실패하면 어디로 돌리는가?

1. exactSHA의EXL3registry/commonEngram/V2/FlashInfer접점과PROVENANCEpin을본다 → 상류동등지원이나접점변경이면patch필요성을재그라운딩한다. drift는편집개수/필수심볼가드로중단하며무작정skip하지않는다.
2. core/alias/deps2/child별context와실제argv·24재고·raw/배포hash범위를본다 → 새README조립명령은수신자localbuild/스모크가필요하다. PREcache와native재컴파일을구분한다.
3. 각노드로컬fileEngramdir/가중치mount와지원옵션을준비하고호스트floor/bench예산을선언한다 → 거부면새형상을정식도출하고가드는유지한다.
4. Rayhead/worker·NCCL실효값,MCG/P2B/lmhead/filemapping/iterator skip,health200+chat추론을함께본다 → Pythonimport/nativeprimitive만으로TP2서빙통과라고하지않는다. 회귀부재candidate지위를유지한다.
5. 실제용처부하의lite/GuideLLM Full과acceptance계승을대조한다 → 고정weakE/REFUTE와성능·품질한계를표시한다. 내부6/KV17.87x를부하실측으로늘리지않는다.
실패행선지:빌드/핀/nativeABI는upstream-version-watch,서빙/KV/EXL3/fileEngram은vllm-recipe-explorer,기대미달성능은adversarial-benchmark,Ray/RoCE/노드는terraforming_node다.

## 0.7 메타
<!-- FACT:meta -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.32.0-g5df63a7203cd/deepseek-v4.1-flash-exl3-2.0bpw-sfxnz/gb10-1g2n-cluster-native/qexl3-len131072-kvfp8-eager` |
| 형식 | `hint-payload/v7` |
| 앵커 | `PROVENANCE.json` — 봉인 때 hint 브랜치 페이로드 커밋에 묶인다 |
| 캠페인 · 셀 · 노드 | camp-26102919-codex-multi4-v032 · i5-032-sm12x · cluster |
| 발행 모드 | campaign |
| 발행 토픽 | camp_26102919_codex_multi4_v032__cluster__i5_032_sm12x |
| work-manifest | docs/_evidence/camp_26102919_codex_multi4_v032__cluster__i5_032_sm12x.work-manifest.json |
| 증거 등급(task_class) | full_benchmark |
| 승인 출처 | campaign:hint_targets |
| 승인 시각(UTC) | 2026-10-29T09:57:19Z |
| 승인 발화(전사) | 사용자: 마지막 멀티브랜치 4종 캠페인을 공식 vLLM0.32.0으로 고정하고 자율형상/Full3회/hint까지 끝까지 진행 |
| 현행 full 정의 충족(측정 등급) | 예 · 이 측정의 반복 3 · 정의 출처 `.claude/rules/docs.md:42`(03 §3.5) |
| 독립 사실 검증(발행 전) | 통과 — 검증자 `root_independent_factchecker`(저작자 `main_hint_author` 아님) · 주장 171건 · 지적 1건 중 고침 1건 · 열림 0 · 2026-10-29T20:50:49Z |
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


이번발행은collector의frozenleaf원본문대조와generated-unverified표시추가의충돌로한번중단됐다. 정식producer수리뒤원본문/배포본문범위를분리해같은경고를유지했다. 첫실패는발행시각결속기록이없어mtime로시각을꾸미지않고재개때실제UTC를주입했다. documentaryenv정정은실행원형스냅샷/down뒤출처키만맞춘것이며재서빙/재측정이아니다. nativeusage중단후같은원Full/session을회수했고대체executor성공을합산하지않았다.

## 0.9 이름 꼬리
> 태그 이름의 결정론부(`<vllm>/<model>/<arch>/q·len·kv`)는 도구가 정한다. 꼬리는 이 셀을 **같은 q·len·kv 의 다른 셀과 가르는 노브**를 발행 Agent 가 골라 근거와 함께 적는 자리다(plan_26092908 §4.1 U5). 아래 표는 publish 때 기계가 낸 **후보**이고(강제 ✗), `continue` 가 draft `inputs/tail.json` 을 근거 대조한 뒤 확정 꼬리로 바꾼다. 같은 이름이 이미 있으면 끝에 발행 시각 `-t<YYMMDDHHMM>`(KST)이 붙는다.

<!-- FACT:name_tail -->
**확정 이름** `hint/0.32.0-g5df63a7203cd/deepseek-v4.1-flash-exl3-2.0bpw-sfxnz/gb10-1g2n-cluster-native/qexl3-len131072-kvfp8-eager` — 꼬리 1토큰.

| 토큰 | 뜻 | 근거(서빙 설정) |
|---|---|---|
| `eager` | 현재 최소 port의 eager 실행; graph 형상과 구분 | `output/multi/configs/i5-032-sm12x.yaml` · 키 `enforce-eager` = `true` |
<!-- /FACT:name_tail -->

> 이 절이 답하는 질문: 이 셀을 같은 q·len·kv 의 다른 셀과 가르는 노브는 무엇이며 그것을 이름 꼬리로 어떻게 적는가 — 가르는 노브가 없으면 왜 빈 꼬리인가?

eager를선택해같은q·len·kv에서graph형상과구분한다. 실제enforce-eager:true가근거이고graph성공이나최적성을뜻하지않는다. PLE없음/spec3은본문의조건으로남겨꼬리를설정덤프로늘리지않는다.
