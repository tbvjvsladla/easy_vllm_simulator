# 00 · hint 지도 — 가장 먼저 읽는 파일

<!-- FACT:header -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.32.0-g5df63a7203cd/qwen3.8-flash-next-nvfp4/gb10-1g2n-cluster-native/qnvfp4-len262144-kvfp8` |
| 형식 | `hint-payload/v7` · 이름 문법 `v7` |
| 생성(UTC · 주입) | 2026-10-29T15:19:36Z |
| 캠페인 · 셀 · 노드 · 모드 | camp-26102919-codex-multi4-v032 · q38fn032-nv4-safe-tp2 · cluster · campaign |
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
| 자격 근거 | `output/multi/benchlog/serve_proof_q38fn032-nv4-safe-tp2.json` — 시점 범위: 작성 2026-10-29T11:42:14Z(output/multi/benchlog/serve_proof_q38fn032-nv4-safe-tp2.json 파일 mtime(파일시스템 관측 — 문서에 시각 칸이 없다 · 기록 필드 아님)) · 측정 창 대비 작성 2026-10-29T11:42:14Z > 측정 끝 2026-10-29T11:41:26Z — 측정 뒤 작성된 스모크 산출물(측정 시점·같은 기동 여부는 시각만으로 미검증) |
| 증거 등급(task_class) | full_benchmark |
| 측정 등급(bench_mode) | full |
| 셀 등급(cell_grade) | `full` — 성능 판정 · 부하 곡선 · 데이터셋 출처는 `03-benchmark.md` |
| 성능 판정(measurement.verdict) | `REFUTE` — 출처 output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B/verdict.json(output · verdict_rule 출력) verdict |

> **REFUTE** — 이 셀의 성능 판정은 **기각**이다(측정은 됐으나 판정 기준 미달). 수치는 관측이지 baseline · 권고가 아니다 — 판정 출처 output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B/verdict.json(output · verdict_rule 출력) verdict.
> 기각 사유: M(41.11) < 53.7×(1−0.15)=45.65 [E(external_reference)] (ratio 0.766) → 기대 이하
<!-- /FACT:grade -->

<!-- FACT:lite_summary -->
<!-- /FACT:lite_summary -->

## 0.1 요약
> 이 절이 답하는 질문: 이 셀은 무엇을 서빙했고, 무엇이 결정적이었으며, 수신자는 무엇을 가장 조심해야 하는가?

Qwen3.8-Flash-Next-NVFP4를 GB10 GPU1개씩 2노드 Ray TP2에서 공식 v0.32.0 소스 커밋으로 서빙한 후보B다. resident PLE·MTP3·graph·KV fp8_e4m3·길이262144 설정에서 Full 동시성1 대표41.11 t/s로 weak E53.7 대비 REFUTE다.
과거 혼합PLE/백엔드/메모리 벽(W1~W8) 뒤 공식0.32의 target NVFP4 CUTLASS·draft FP8 TRITON 조합을 선택했고, 후속 분리백엔드·메모리 실패와 단위오류는 별도로 기록했다(W10~W14).
seq6와 절대KV클램프는 손레버 승계값(V6·V8)이며 Full 실측은 동시성1/2/4의 짧은 요청이다. 설정 최대길이 전체의 품질·동시성6 검증으로 읽지 않는다.

## 0.2 유효맥락
<!-- FACT:context -->
| 항목 | 값 | 출처 |
|---|---|---|
| 모델 | qwen3.8-flash-next-nvfp4 | sweep_index.meta(output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B) |
| HF repo | nvidia/Qwen3.8-Flash-Next-NVFP4 | 체크포인트 .git/config remote url(huggingface.co · /app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4) |
| HF revision(체크포인트 git HEAD) | fc694b54fb0174e0913e6adf86691ef85a4ead47 | 체크포인트 .git HEAD → refs/heads/main → loose ref(/app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4) · refs/remotes/origin/main 와 일치(받아 온 원격 커밋) — checkout 커밋(작업트리 일치는 미검증) · reflog: 측정 시점 checkout = 지금 HEAD(measured_utc 2026-10-29T11:41:26Z 이전 마지막 이동 2026-09-08T10:37:13Z) |
| base_model | Qwen/Qwen3.8-Flash-Next | 체크포인트 README base_model(/app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4) |
| base 슬러그 | qwen3.8-flash-next | model(qwen3.8-flash-next-nvfp4) − vocab quant_suffixes(naming.base_slug) |
| 양자화 | nvfp4 | naming-axis(q) — 측정 identity quantization 미관측(sweep_index.meta(output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B)) · q 축 = /app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4/config.json quantization_config(modelopt MIXED_PRECISION) quantized_layers 우세 NVFP4 48/50 · vocab quant[nvfp4]←'modelopt-dominant:NVFP4' — 인증서 quantization 이 N/A 라 명명 축 q(체크포인트 config 파생)를 옮겼다 |
| GPU | NVIDIA GB10 | sweep_index.meta(output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B) |
| 토폴로지 · TP | multi · TP=2 | sweep_index.meta(output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B) |
| 실행 평면 | docker | artifacts.plane_of |
| vLLM(측정 강식별 키 `vllm_version`) | 0.32.0 | sweep_index.meta(output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B) |

**명명 축** — 태그 이름 = 결정론부(vllm · model · arch · q · len · kv — 도구가 증거에서 파생 · 발행자 입력 ✗) + 꼬리(발행 Agent 가 고르고 서빙 설정 file · key · value 로 근거 대조 · 0.9) + 중복 시 timestamp.

| 축 | 값 | 출처 |
|---|---|---|
| 세그먼트 `vllm` | 0.32.0-g5df63a7203cd | track=선택자 Dockerfile.source-build-deps2 · ref=docker history build-arg VLLM_REF · version=VLLM_VERSION 미관측 · repo=docker history build-arg VLLM_REPO · sha=VLLM_REF(40자 커밋 · docker history build-arg VLLM_REF) · exact_tag=measured image sha256:8227e7d79471b6b92f21157acfb38c7cc617d1510a5c56f3dec7eb8e48567884: /workspace/vllm-src/.git HEAD and exact tag peeled to 5df63a7203cdb9328d824a27b682e77c7b9928be (create/cp, no start) · VLLM_REF=5df63a7203cd…(40자 커밋) → <직전 릴리스 0.32.0>-g<sha12> |
| 세그먼트 `model` | qwen3.8-flash-next-nvfp4 | 서빙 yaml model(output/multi/configs/q38fn032-nv4-safe-tp2.yaml) · hf_repo=체크포인트 .git/config remote url(huggingface.co · /app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4) · hf_repo(nvidia/Qwen3.8-Flash-Next-NVFP4) 마지막 성분 소문자 |
| 세그먼트 `arch` | gb10-1g2n-cluster-native | derived(hw·gpus_per_node·nodes·role·target·plane) |
| 세그먼트 `recipe` | qnvfp4-len262144-kvfp8 | derived(q·len·kv) |
| 축 `hw` | gb10 | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(자동 파생 — 측정 TP=2(sweep_index.meta.tensor_parallel_size(output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B · generated_utc = 측정 키)) > gpus_per_node=1(output/multi/manifest.yaml(manifest_contract 로더)) · 단일 노드 형상일 수 없다 · 일치 신호: 멀티 serve proof output/multi/benchlog/serve_proof_q38fn032-nv4-safe-tp2.json(kind=multinode_serve_proof) · 노드 정합 attestation output/multi/benchlog/attestation_q38fn032-nv4-safe-tp2.json) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) · vocab hw[gb10]←'NVIDIA GB10' |
| 축 `gpus_per_node` | 1 | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(자동 파생 — 측정 TP=2(sweep_index.meta.tensor_parallel_size(output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B · generated_utc = 측정 키)) > gpus_per_node=1(output/multi/manifest.yaml(manifest_contract 로더)) · 단일 노드 형상일 수 없다 · 일치 신호: 멀티 serve proof output/multi/benchlog/serve_proof_q38fn032-nv4-safe-tp2.json(kind=multinode_serve_proof) · 노드 정합 attestation output/multi/benchlog/attestation_q38fn032-nv4-safe-tp2.json) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `nodes` | 2 | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(자동 파생 — 측정 TP=2(sweep_index.meta.tensor_parallel_size(output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B · generated_utc = 측정 키)) > gpus_per_node=1(output/multi/manifest.yaml(manifest_contract 로더)) · 단일 노드 형상일 수 없다 · 일치 신호: 멀티 serve proof output/multi/benchlog/serve_proof_q38fn032-nv4-safe-tp2.json(kind=multinode_serve_proof) · 노드 정합 attestation output/multi/benchlog/attestation_q38fn032-nv4-safe-tp2.json) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `role` | cluster | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(자동 파생 — 측정 TP=2(sweep_index.meta.tensor_parallel_size(output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B · generated_utc = 측정 키)) > gpus_per_node=1(output/multi/manifest.yaml(manifest_contract 로더)) · 단일 노드 형상일 수 없다 · 일치 신호: 멀티 serve proof output/multi/benchlog/serve_proof_q38fn032-nv4-safe-tp2.json(kind=multinode_serve_proof) · 노드 정합 attestation output/multi/benchlog/attestation_q38fn032-nv4-safe-tp2.json) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `target` | native | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(자동 파생 — 측정 TP=2(sweep_index.meta.tensor_parallel_size(output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B · generated_utc = 측정 키)) > gpus_per_node=1(output/multi/manifest.yaml(manifest_contract 로더)) · 단일 노드 형상일 수 없다 · 일치 신호: 멀티 serve proof output/multi/benchlog/serve_proof_q38fn032-nv4-safe-tp2.json(kind=multinode_serve_proof) · 노드 정합 attestation output/multi/benchlog/attestation_q38fn032-nv4-safe-tp2.json) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) · target_gpu 'NVIDIA GB10' = 호스트 hw → native |
| 축 `plane` | docker | artifacts.plane_of → cell-env(IMAGE_TAG\|BUILD_DOCKERFILE) · vocab plane[docker]→'' |
| 축 `q` | nvfp4 | /app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4/config.json quantization_config(modelopt MIXED_PRECISION) quantized_layers 우세 NVFP4 48/50 · vocab quant[nvfp4]←'modelopt-dominant:NVFP4' |
| 축 `len` | 262144 | 서빙 yaml max-model-len(output/multi/configs/q38fn032-nv4-safe-tp2.yaml) |
| 축 `kv` | fp8 | 서빙 yaml kv-cache-dtype(output/multi/configs/q38fn032-nv4-safe-tp2.yaml) · vocab kv[fp8]←'fp8_e4m3' |
<!-- /FACT:context -->

> 이 절이 답하는 질문: 이 지도는 어떤 조건에서만 성립하며, 이 형상은 어떤 기전으로 버티고 다른 형상은 왜 실패했는가?

GB10의 통합메모리에서는 PLE·가중치·KV와 호스트 여유가 같은 물리총량을 나눈다. CPU pinned offload가 별도 VRAM을 회수하는 구조와 다르다. 이 B는 PLE resident이며 메모리총량·KV·overhead를 다시 선언해야 다른 노드에 이식할 수 있다(plan_26090918 §2.3, testlog_26102420 §3). TP2는 노드당GPU1인 두 노드의 RoCE/Ray 집단연산이며 싱글 mmap 성공사례와 같은 조건이 아니다.

B는 NVFP4 target을 auto FLASHINFER_CUTLASS, FP8 MTP를 TRITON으로 처리했다. 전역 Marlin C도 기능·Full은 성립했으나 대표39.68로 REFUTE다. 같은 E를 유지했고 C의 일부 반복43.77을 숨기거나 B의 통계적 우월성을 주장하지 않는다(testlog_26103000_Qwen032_유효Full과_발행후보_선택). draft CUTLASS D는 실제 FP8 block quant-scheme 미지원으로 실패했다. 정적 mapping 존재는 해당 mixed quant의 GPU 성공 보증이 아니다.

E/F의 FlashInfer B12X 경로는 메모리 안전중단, G의 static cutover128은 actor환경 도달 후 ETA가 급락률로 선제종료했다. N320→384 padding의 신규배열 전체를 캐시에 보존하는 경로에서48층 신규가중치33.75GiB/랭크라는 수식은 전체RAM 실측이 아니다. H는 별도 native b12x 원본/view 경로와 text-only·context131072·seq2·chunk1024·KV4GiB를 함께 바꿔 기능과 유효Full을 얻은 메모리 대안이다. 메모리여유는 관측시점 값이며 backend 하나의 단독인과나 모든 부하의 안전보장이 아니다(devlog_26103000_Qwen032_경로탐색과_B_인계 §Qwen 공식0.32 경로 탐색과 후보 B 인계).

B의262144는 설정/엔진KV수용량이고 실제Full은1024/256이다. 장문needle·멀티모달품질·요청6 동시성의 신규검증은 없다. 과거0.31 1M needle 성공은 다른 버전·형상이므로 이 셀의 장문검증으로 승계하지 않는다.

## 0.3 벽 지도 요약
> 02-narrative.md §2.2 의 `kind: wall` 블록(과 §2.4 의 `kind: misdiagnosis` 블록)에서 기계가 생성한 표다(손저작 ✗ · 순서 = 넘은 순서). 원인·해소의 전문과 원문 발췌는 02 에 있다.

<!-- FACT:wall_map -->
| 순서 | id | 증상 | 해소 | 전이등급 | 검증 |
|---|---|---|---|---|---|
| 1 | W1 | 0.29.0rc6 stock 에서 NVFP4 혼합 체크포인트의 PLE 양자화 방식이 None 이 되어 로드 거부가 예상됨 | 0.29 계보는 빌드 패치 60(PLE mixed 라우트 + MTP 디스패치 + 리맵) · 0.31.0 은 stock 이 처리(역사 0.31 셀 패치 0) | judgment | testlog_26090921 §판정 스모크 5차 PASS · 역사 0.31 셀 역사 측정 기동 lite_engine L631 Initialized PLE embedding ... Qwen4ExpPLEFp8EmbeddingMethod |
| 2 | W2 | P0-1 스모크 1차가 기동 전에 차단됨 | kv-cache-memory-bytes 절대 클램프 + 예산 선언(budget_declare) 후 진입 | arch-invariant | testlog_26090921 §판정 스모크 5차 PASS · 역사 0.31 셀 원장 docs/logs/main/events/2026-10.jsonl 643행 budget_declare 와 watchdog.jsonl budget_honored |
| 3 | W3 | P0-1 스모크 2차가 MoE 백엔드 검사에서 fail-loud 로 종료 | moe-backend 를 명시하지 않음(auto) — 역사 0.31 셀의 엔진은 FLASHINFER_CUTLASS 를 자동 선택 | arch-scaled | testlog_26090921 §판정 최종 설정 moe-backend 미설정(auto) · 역사 0.31 셀 lite_engine L520 Using 'FLASHINFER_CUTLASS' NvFp4 MoE backend |
| 4 | W4 | P0-1 스모크 3차가 MoE 백엔드 가드 충돌로 종료 | moe-backend 를 명시하지 않음(auto) | judgment | testlog_26090921 §판정 스모크 5차 PASS(최종 설정 moe-backend 미설정) |
| 5 | W5 | P0-1 스모크 4차 중 cudagraph 캡처 구간에서 컨테이너가 워치독에 사살됨 | enforce-eager true 로 cudagraph 캡처 제거 | arch-scaled | testlog_26090921 §판정 스모크 5차(eager) PASS |
| 6 | W6 | 0.29.0rc6 의 NVFP4 · PLE resident · 1M 셀 2개가 로드 중 서브 노드에서 사살됨 | 역사 0.31 셀은 KV fp8 14 GiB · gmu 0.80 · max-num-seqs 1 · max-num-batched-tokens 2048 로 재확정(역사 발행본 `hints/.drafts/replay__q38fn31_nv4_1m_tp2/payload/02-narrative.md` §2.5의 0.29→0.31 변경 목록) | arch-scaled | testlog_26102414 §1 선례 대비 TRIP 0 · 역사 발행본 `hints/.drafts/replay__q38fn31_nv4_1m_tp2/payload/01-artifacts.md` §1.4의 2026-10-25T01:52:06Z 선언창 사살 칸 없음(당시 메인 원장 기준; 현재032 시도 번호가 아님) |
| 7 | W7 | 멀티 스모크가 서브 통로 미완결로 기동 전에 멈춤 | sync_to_sub.sh --branch multi(dry-run 후 apply · 승인 앵커 plan_26102412) | judgment | testlog_26102414 §1 3차 health 200 · 스모크 rc=0 |
| 8 | W8 | 기동 인자 검증에서 max_model_len 1048576 이 derived 262144 보다 크다며 ValidationError 로 종료 | hf-overrides text_config 에 max_position_embeddings 1048576 을 rope_parameters(YaRN factor 4 · original 262144)와 함께 실음 | arch-invariant | 역사 측정 기동 lite_engine_head L62 non-default args 의 max_position_embeddings 1048576 · L72 Using max model len 1048576 · testlog_26102414 §2 needle 1,001,130 PASS |
| 9 | W9 | resident·noMTP·async off 대조에서도 run-to-run 차이가 남았다 | top-5·margin·logprob 중재로 바꾸고 exact토큰실패를모델불가로읽지않음 | judgment | testlog_26091001_p0-2_ple_mmap_p0-3_qsa_fp8kv_스모크_판정 §1 |
| 10 | W10 | 기능·유효Full이 성립해도 외부E를 넘지 못했다 | C와후속대안탐색후 B를관측발행대상으로선택;PASS로바꾸지않음 | judgment | devlog_26103000_Qwen032_경로탐색과_B_인계 §Qwen 공식0.32 경로 탐색과 후보 B 인계 |
| 11 | W11 | 분리설정 D에서 MTP quant scheme가 거부됐다 | 최초성공B의실제draftTRITON으로정정;해당D는기능실패로보존 | judgment | devlog_26103000_Qwen032_경로탐색과_B_인계 §Qwen 공식0.32 경로 탐색과 후보 B 인계 |
| 12 | W12 | E/F의FlashInferB12X 경로가메모리여유를잃었다 | E/F를각기보존하고cutover128 G로이동;가드완화없음 | judgment | devlog_26103000_Qwen032_경로탐색과_B_인계 §Qwen 공식0.32 경로 탐색과 후보 B 인계 |
| 13 | W13 | cutover128 actor환경을 확인한 뒤G가ETA가드로종료됐다 | padding전체신규캐시수식을검토후별도nativeb12x/text-only/작은형상H로기능Full성립;인과단독확정없음 | judgment | devlog_26103000_Qwen032_경로탐색과_B_인계 §Qwen 공식0.32 경로 탐색과 후보 B 인계 |
| 14 | W14 | request_latency_ms_p99키에초단위값이들었다 | 8f0d009 producer수정과B/C/H24raw정식재파생;decode/TPOT/verdict/시각불변 | judgment | testlog_26103000_11_58_GuideLLM_요청지연_단위_교정 §원인과 producer 수정 |

**오진 · 정정**(02 §2.4 — 현재지위가 `가설(강등)` 인 기전은 처방이 아니다)

| id | 증상 | 현재지위 | 검증 |
|---|---|---|---|
| M1 | PLEresident축전부실패라는일반화 | 반증 | testlog_26091304_qwen38fn_res최소조건_262k재발행_판정 §3.2 |
| M2 | GuideLLM요청지연초값을ms출력키로표시 | 반증 | testlog_26103000_11_58_GuideLLM_요청지연_단위_교정 §B·C·H 역사 raw 재파생 |
<!-- /FACT:wall_map -->

## 0.4 결정론 해소값
<!-- FACT:resolved -->
| 항목 | 값 | 출처 |
|---|---|---|
| vLLM 빌드 입력(종류) | commit | naming.vllm_build_input |
| vLLM 빌드 입력(ref) | 5df63a7203cdb9328d824a27b682e77c7b9928be | docker history build-arg VLLM_REF |
| vLLM 빌드 계보 SHA(40자) | 5df63a7203cdb9328d824a27b682e77c7b9928be | VLLM_REF(40자 커밋 · docker history build-arg VLLM_REF) |
| 직전 릴리스(prev_release) | 0.32.0 | naming.vllm_build_input |
| vLLM 엔진 자기보고(엔진 로그 기동 배너) | 0.32.1.dev0+g5df63a720.d20261029 | output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B/lite_engine_head_q38fn032-nv4-safe-tp2.log:59 — 측정 캡처에 기록된 값 · 현재 serve 세션 경계 미관측 — 누적 로그의 기동 epoch 귀속 미검증 · 같은 값의 다른 로그 1건 · 지위 `measurement-capture-unbound` |
| 인증서 `vllm_version`(강한 일치 키) | 미관측 | 생산자 `absent` · 인증서 vllm_version 없음 |
| vLLM 설치 distribution 메타(출처별 관측 · 다운로드 wheel 증명 아님) | 노드별 상이·공통값 미선택(각 값은 출처) | 노드별 상이 — 값을 고르지 않는다: attestation nodes.<n>.vllm_dist_version(output/multi/benchlog/attestation_q38fn032-nv4-safe-tp2.json · 노드별 관측) — ⚠ 노드별 상이(main=0.32.1.dev0+g5df63a720.d20261029.cu133 · sub=0.32.1.dev0+g5df63a720.d20261028.cu133 · 값 = main) · 이 측정 실행과 같은 실행인지 미검증(같은 호스트의 관측) · 작성 2026-10-29T11:42:14Z > 측정 끝 2026-10-29T11:41:26Z — 측정 뒤 작성된 관측(측정 시점·같은 기동 여부는 시각만으로 미검증)(sweep_index.meta.vllm_build='NA' 는 sweep_bench 가 엔진 로그에서 `v<x.y.z…>` 정규식으로 잡는 값이다 → engine_self_report 의 원천) · 지위 `observed-node-difference` |
| 체크포인트 revision(HF · git HEAD) | fc694b54fb0174e0913e6adf86691ef85a4ead47 | 체크포인트 .git HEAD → refs/heads/main → loose ref(/app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4) · refs/remotes/origin/main 와 일치(받아 온 원격 커밋) — checkout 커밋(작업트리 일치는 미검증) · reflog: 측정 시점 checkout = 지금 HEAD(measured_utc 2026-10-29T11:41:26Z 이전 마지막 이동 2026-09-08T10:37:13Z) |
| 빌드 트랙 | source-build | artifacts 선택자(build-ledger.dockerfile) |
| Dockerfile(선택자) | Dockerfile.source-build-deps2 | build-ledger.dockerfile |
| vLLM 저장소 | https://github.com/vllm-project/vllm.git | docker history build-arg VLLM_REPO |
| 이미지 태그 | easy-vllm:0.32.0-cu133-aarch64-source-deps2 | 셀 env(output/multi/envs/.env.q38fn032-nv4-safe-tp2) IMAGE_TAG |
| 이미지 digest | sha256:8227e7d79471b6b92f21157acfb38c7cc617d1510a5c56f3dec7eb8e48567884 | sweep_index.meta.image_digest(measured(docker inspect .Image)) |
| torch | 2.13.0a0+9186a08 | docker image inspect Config.Env PYTORCH_VERSION |
| CUDA | 13.3.1.008 | docker image inspect Config.Env CUDA_VERSION |
| NGC 베이스 | 26.07 | docker image inspect Config.Env NVIDIA_PYTORCH_VERSION |
| CPU arch | arm64 | docker image inspect Architecture |
| 드라이버 | 580.178.04 | attestation parity.driver(output/multi/benchlog/attestation_q38fn032-nv4-safe-tp2.json · 노드별 관측 · verdict=equal) — main=sub 일치 · 이 측정 실행과 같은 실행인지 미검증(같은 호스트의 관측) · 작성 2026-10-29T11:42:14Z > 측정 끝 2026-10-29T11:41:26Z — 측정 뒤 작성된 관측(측정 시점·같은 기동 여부는 시각만으로 미검증) |
| 드라이버(노드별 관측) | main=580.178.04 · sub=580.178.04 | attestation parity.driver(output/multi/benchlog/attestation_q38fn032-nv4-safe-tp2.json · 노드별 관측 · verdict=equal) — main=sub 일치 · 이 측정 실행과 같은 실행인지 미검증(같은 호스트의 관측) · 작성 2026-10-29T11:42:14Z > 측정 끝 2026-10-29T11:41:26Z — 측정 뒤 작성된 관측(측정 시점·같은 기동 여부는 시각만으로 미검증) |
| ⚠ 드라이버 관측 ≠ 선언 | 충돌 없음 — 관측 580.178.04 = 선언 580.178.04 | 관측: attestation parity.driver(output/multi/benchlog/attestation_q38fn032-nv4-safe-tp2.json · 노드별 관측 · verdict=equal) — main=sub 일치 · 이 측정 실행과 같은 실행인지 미검증(같은 호스트의 관측) · 작성 2026-10-29T11:42:14Z > 측정 끝 2026-10-29T11:41:26Z — 측정 뒤 작성된 관측(측정 시점·같은 기동 여부는 시각만으로 미검증) · 선언: sweep_index.meta.driver_version(output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B · 측정 시 조립 — sweep_bench.sh 가 manifest 에서 읽는다) (manifest 선언 옮김) |
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

**이 모델에 필요한 패치**(01 §1.1 관련성 · ① 패치 선언 대상 × 모델 config ② 엔진 로그 발화 — 둘 다 관측일 때만 확정): required 0개 · inactive-inferred 0개 · unknown 1개(unknown = 판정 신호 부족 — 필요 여부를 이 표로 단정하지 않는다).
<!-- /FACT:resolved -->

> 이 절이 답하는 질문: 위 값들 중 무엇을 그대로 참조하고 무엇을 네 환경에서 다시 도출해야 하며, 이 빌드 트랙은 왜 골랐는가?

(1) 트랙은source-build이며 NGC alpha torch를 유지하는 기존 경로를 공식0.32에 맞춰 사용했다. main core는 새 로컬빌드, sub의 검증deps1 core는 재사용했고 양노드deps2/B12X1.5.1층은 각각 로컬빌드했다. 이미지save/load를 하지 않았다(testlog_26102920_Qwen032_CUTLASS_Full_판정 §빌드와 기능).
(2) 재현입력은40SHA `5df63a7203cdb9328d824a27b682e77c7b9928be`다. exact 공식tag는v0.32.0이며 명명은40SHA build-arg 근거로 파생한다. 엔진/배포판의0.32.1.dev SCM표시와main/sub 빌드날짜 차이는 별도생산자값이다. 공식0.32.1 릴리스나 다른 소스를 썼다는 뜻이 아니다. sweepmeta0.32.0도 엔진자기보고와 다르다.
(3) arm64·SM121 계열·CUDA13.3·NGC26.07/torch2.13alpha는 이 로컬이미지의 좌표다. 다른GPU의compute capability와 베이스torch/커널지원을 새로 대조한다. 호스트driver580.178.04와 CUDA132 선언은 이미지CUDA13.3.1.008과 다른 층이다.
(4) 양노드SHA/torch/driver equal과 imageId·상속원장·REF표현 차이를 함께 보존한다. wholeimage 바이트동등을 주장하지 않는다. main env의tag별칭은 실제build-arg40SHA로 정정했고, flat Dockerfile선택자만 동일바이트로 정정했다. serving인자·이미지·TPS는 바뀌지 않았다(testlog_26103000_Qwen032_유효Full과_발행후보_선택). leaf Dockerfile은history5/10의generated-unverified 참고형상이며 정확leaf빌드바이트 재현보증이 아니다. 별도parent-source-core/context에는main이실제로쓴5파일과core명령/alias/leaf원장근거가수집됐다. 그키트의README/manifest와새재현명령은generated-unverified이며아직양노드새빌드를수행한것이아니다.
(5) 노드물리총량은 이전 사전판정 MemTotal약121GiB와 이번원장 mem_total_mib를 구분해 읽는다. OS는 현재FACT에 미관측이다. RoCE/NCCL 실효값은01의 실행env표가 소유하며 노드별관측범위를 넘겨 일반화하지 않는다.
(6) post06은공식선택B12X1.5.1 exactwheel/해시·pipcheck를 고정하는 공유이미지층이다. targetB가 실제쓴NVFP4 CUTLASS/draftTRITON을 B12X 커널성공으로 바꾸어 적지 않는다. torch/CUDA/NGC의실제값은imageinspect, SHA는원장/attestation, 의존설치는06과부모원장범위로 구분한다.

## 0.5 서빙 노브와 값의 지위
> 01-artifacts.md §1.3 의 `kind: value-status` 블록에서 기계가 생성한 요약이다. 지위가 `tuned` · `declared-requirement` 가 아닌 값은 **이 셀에서 조정된 적도, 빼면 깨진다고 확인된 적도 없다** — 필요조건으로 복사하지 마라. `declared-requirement` 는 빼거나 바꾸면 실패가 관측된 값이다(근거의 W id · artifacts 헤더를 보고 네 환경에서도 그 실패 조건이 성립하는지 판단한다).

<!-- FACT:knob_status -->
노브 12개 — `inherited` 12.

| 노브 | 값 | 지위 | 근거 | id |
|---|---|---|---|---|
| `tensor-parallel-size` | 2 | inherited | manifest의노드당GPU1×2/Ray분산형상 | V1 |
| `distributed-executor-backend` | ray | inherited | 승인된TP2클러스터실행기 | V2 |
| `gpu-memory-utilization` | 0.8 | inherited | hand gmu; 이전동일모델근거에서새후보로선언 | V3 |
| `max-model-len` | 262144 | inherited | 네이티브context설정;Full부하길이측정과구분 | V4 |
| `kv-cache-dtype` | fp8_e4m3 | inherited | 초기후보의FP8 KV;제거대조없음 | V5 |
| `kv-cache-memory-bytes` | 15032385536 | inherited | hand KV14GiB/rank;최소필요량수렴아님 | V6 |
| `enforce-eager` | false | inherited | 초기후보graph경로;동일B에서on/off대조없음 | V7 |
| `max-num-seqs` | 6 | inherited | hand-lever상한;Full실측동시성1/2/4 | V8 |
| `max-num-batched-tokens` | 4096 | inherited | 초기후보chunk입력;최적chunk탐색아님 | V9 |
| `engram-config` | {"cpu_offload": false} | inherited | UMA residentPLE선택;현재cpuoffload대조없음 | V10 |
| `compilation-config` | {"mode": 0, "cudagraph_mode": "FULL_DECODE_ONLY"} | inherited | 초기compile/graph조합;단독효과미검증 | V11 |
| `speculative-config` | {"method": "mtp", "num_speculative_tokens": 3} | inherited | 기존MTP3근거에서선택;수용길이는litewarm실측 | V12 |
<!-- /FACT:knob_status -->

> 이 절이 답하는 질문: 이 레시피를 다른 환경으로 옮길 때 어떤 노브를 먼저 다시 재야 하고, 어떤 노브는 빼면 깨지는가?

① KV bytes·gmu·context·seq·chunk는 노드총량/체크포인트/목표부하에서 다시 산정하고 실제KV토큰·MemAvailable·기능응답으로 검증한다. V4의길이설정과V8의seq6는 전체길이·동시성6측정이 아니다.
② 이번B의 값중 제거대조로 확인된 declared-requirement는 없다. 과거W3/W4의NVFP4 backend가드와0.31 YaRN W8은 그 버전조건의 기록이다. D의FP8 draftCUTLASS 미지원은 새0.32 mixed체크포인트의 실제 실패다.
③ gmu0.8/KV14GiB/seq6와resident PLE는hand-authored 후보입력이며 최적값이 아니다. graph/compile0/MTP3도 현재성립형상으로 기록하고 단일원인으로 성능차를 설명하지 않는다. 음성대조를 위해 고른 값은 없다.

## 0.6 재검증 · 라우팅
- 이 자료를 `vllm-recipe-explorer` 인터뷰의 **warm-start 근거로만** 투입하고, 평소대로 plan-gate → 레시피 수렴 → 스모크 게이트를 그대로 밟아라(가속이지 우회 ✗).

> 이 절이 답하는 질문: 다른 환경(다른 HW · 다른 vLLM 버전 · 다른 노드 수)에서 이 지도를 쓸 때 무엇부터, 어떤 순서로 확인해야 하며, 실패하면 어디로 돌리는가?

1. config/HF checkout의mixed quant와PLE/MTP구성을00축에 대조한다 → 다르면 이backend결론을 폐기하고 대상별oracle를 다시 확인한다.
2. 공식40SHA·torch·driver를 양노드의로컬이미지에서 확인한다 → ABI3축이 다르면 분산기동전해소하고 imageId자체의차이를 실패로 오인하지 않는다. 06의pipcheck는GPUkernel스모크와 구분한다.
3. 상위vLLM에서는60/62/64의과거patch를carry-forward하지 않는다 → stock PLE/MTP지원과06 optional pin의필요를소스/실행으로 다시판정한다. 이번B별도runtimepatch는없다.
4. MemTotal·weights·KV·overhead로 양노드보호예산을 선언하고 정식master→slave→GPU합류기동을 따른다 → health200+추론1회 없이는성공으로세지않고 ETA/메모리사살은호스트이벤트와 대조한다.
5. 로드의긴JIT/공유메모리대기를hang으로단정하지말고 compile/autotune진행과가드신호를함께본다 → watchdog가드를끄지않는다. 긴프롬프트와vision은요구시별도검증한다.
6. lite후같은GuideLLM형상/3반복을재고REFUTE를그대로표시한다 → E조건이달라지면like-with-like를다시평가하며기준을실패후낮추지않는다. 요청지연의초/ms는W14producer교정상태를확인한다.

빌드·핀·커널의존은upstream-version-watch, KV/서빙노브는vllm-recipe-explorer, 성능은adversarial-benchmark, 노드·RoCE·배달은terraforming_node로라우팅한다. 자동실행지시가 아니라수신자검토순서다.

## 0.7 메타
<!-- FACT:meta -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.32.0-g5df63a7203cd/qwen3.8-flash-next-nvfp4/gb10-1g2n-cluster-native/qnvfp4-len262144-kvfp8` |
| 형식 | `hint-payload/v7` |
| 앵커 | `PROVENANCE.json` — 봉인 때 hint 브랜치 페이로드 커밋에 묶인다 |
| 캠페인 · 셀 · 노드 | camp-26102919-codex-multi4-v032 · q38fn032-nv4-safe-tp2 · cluster |
| 발행 모드 | campaign |
| 발행 토픽 | camp_26102919_codex_multi4_v032__cluster__q38fn032_nv4_safe_tp2 |
| work-manifest | docs/_evidence/camp_26102919_codex_multi4_v032__cluster__q38fn032_nv4_safe_tp2.work-manifest.json |
| 증거 등급(task_class) | full_benchmark |
| 승인 출처 | campaign:hint_targets |
| 승인 시각(UTC) | 2026-10-29T09:57:19Z |
| 승인 발화(전사) | 사용자: 마지막 멀티브랜치 4종 캠페인을 공식 vLLM0.32.0으로 고정하고 자율형상/Full3회/hint까지 끝까지 진행 |
| 현행 full 정의 충족(측정 등급) | 예 · 이 측정의 반복 3 · 정의 출처 `.claude/rules/docs.md:42`(03 §3.5) |
| 독립 사실 검증(발행 전) | 통과 — 검증자 `root_independent_factchecker`(저작자 `main_hint_author` 아님) · 주장 120건 · 지적 7건 중 고침 7건 · 열림 0 · 2026-10-29T16:09:26Z |
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

이번발행은마지막후보H가아닌보존B를선택했다. B입력/proof/attestation/correctedFull재결속과env40SHA·flatselector정정은재기동/재측정이아니다. 원raw24개에서요청지연단위만정식재파생했고TPS/verdict/시각은불변이다. 초안의파일시각만으로재기동을단정하던FACT는정식producer재파생으로작성시각/동일기동미검증범위로정정했다. 06 exactCOPY제외오판도재파생으로해소했고, 두예산창관측은단일기동시간상한으로합성하지않는다. 부모core재현키트가추가됐지만그README/manifest의새양노드local-build안은미실행이다. 태그성능REFUTE와설정/검증범위한계를숨기지않는다.

## 0.9 이름 꼬리
> 태그 이름의 결정론부(`<vllm>/<model>/<arch>/q·len·kv`)는 도구가 정한다. 꼬리는 이 셀을 **같은 q·len·kv 의 다른 셀과 가르는 노브**를 발행 Agent 가 골라 근거와 함께 적는 자리다(plan_26092908 §4.1 U5). 아래 표는 publish 때 기계가 낸 **후보**이고(강제 ✗), `continue` 가 draft `inputs/tail.json` 을 근거 대조한 뒤 확정 꼬리로 바꾼다. 같은 이름이 이미 있으면 끝에 발행 시각 `-t<YYMMDDHHMM>`(KST)이 붙는다.

<!-- FACT:name_tail -->
**확정 이름** `hint/0.32.0-g5df63a7203cd/qwen3.8-flash-next-nvfp4/gb10-1g2n-cluster-native/qnvfp4-len262144-kvfp8` — 꼬리 0토큰.

_빈 꼬리 — 발행 Agent 가 가르는 노브가 없다고 명시했다(`inputs/tail.json` = `[]`)._
<!-- /FACT:name_tail -->

> 이 절이 답하는 질문: 이 셀을 같은 q·len·kv 의 다른 셀과 가르는 노브는 무엇이며 그것을 이름 꼬리로 어떻게 적는가 — 가르는 노브가 없으면 왜 빈 꼬리인가?

이발행은같은결정론부의기존032태그와구분할새서빙노브를추가하지않아tail.json을빈목록으로명시한다. candidate의PLE/spec/graph를v6고정세토큰처럼기계적으로붙이지않는다. B의MTP3·resident·graph조건은00/01에남고측정시각은03이구분한다.
