# 00 · hint 지도 — 가장 먼저 읽는 파일

<!-- FACT:header -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.31.0/qwen3.8-flash-next-fp8/gb10-1g2n-cluster-native/qfp8-len1048576-kvfp8-plemmap` |
| 형식 | `hint-payload/v7` · 이름 문법 `v7` |
| 생성(UTC · 주입) | 2026-10-25T04:10:00Z |
| 캠페인 · 셀 · 노드 · 모드 | 캠페인 밖(`_bootstrap` · output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/sweep_index.json bench_knobs.campaign = null · campaign_pins_note '활성 캠페인 없음(_bootstrap)'(측정 시점 ACTIVE 해소 · repeat_axis)) · q38fn31-fp8-1m-tp2 · cluster · publication-replay |
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
| 자격 근거 | `output/multi/benchlog/serve_proof_q38fn31-fp8-1m-tp2.json` — 시점 범위: 작성 2026-10-25T01:39:26Z(output/multi/benchlog/serve_proof_q38fn31-fp8-1m-tp2.json 파일 mtime(파일시스템 관측 — 문서에 시각 칸이 없다 · 기록 필드 아님)) · 측정 창 대비 작성 2026-10-25T01:39:26Z < 측정 시작 2026-10-25T01:39:47Z — 측정 전(빌드·스모크)의 관측 |
| 증거 등급(task_class) | full_benchmark |
| 측정 등급(bench_mode) | full |
| 셀 등급(cell_grade) | `full` — 성능 판정 · 부하 곡선 · 데이터셋 출처는 `03-benchmark.md` |
| 성능 판정(measurement.verdict) | `REFUTE` — 출처 output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/verdict.json(output · verdict_rule 출력) verdict |

> **REFUTE** — 이 셀의 성능 판정은 **기각**이다(측정은 됐으나 판정 기준 미달). 수치는 관측이지 baseline · 권고가 아니다 — 판정 출처 output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/verdict.json(output · verdict_rule 출력) verdict.
> 기각 사유: M(22.78) < 32.5×(1−0.15)=27.62 [E(external_reference)] (ratio 0.701) → 기대 이하
<!-- /FACT:grade -->

<!-- FACT:lite_summary -->
<!-- /FACT:lite_summary -->

## 0.1 요약
> 이 절이 답하는 질문: 이 셀은 무엇을 서빙했고, 무엇이 결정적이었으며, 수신자는 무엇을 가장 조심해야 하는가?

Qwen3.8-Flash-Next-FP8(unsloth)을 GB10 2노드(노드당 GPU 1 · Ray TP=2) · vLLM `v0.31.0` 소스빌드 공유 이미지에서 max-model-len 1048576(YaRN factor 4) · KV fp8_e4m3 · eager · spec decode 없음 · **PLE 테이블을 디스크 mmap 으로 읽는 런타임 플러그인**으로 서빙했고, GuideLLM 동시성 1 decode 22.78 t/s 는 외부 기준 32.5 대비 ratio 0.701 의 weak REFUTE(표시만 · 서빙 실패 아님)다.
가장 비싼 벽은 PLE 상주 TP=2 의 예산 선판정 차단(W5)과 이전 vLLM 빌드 패치 62 의 앵커 전멸(W6)이며, stock 임베딩 클래스를 바꾸는 플러그인 `q38fn31-fp8-1m-tp2_patch.py`(`VLLM_PLE_MMAP=1` · FP8 스테이징 `VLLM_PLE_MMAP_DIR` · 이미지 재빌드 없음)로 넘었다.
가장 오해하기 쉬운 값: yaml 의 `async-scheduling: false` 는 설정에 도달하지 않고(M3) `enforce-eager` 는 플러그인 제약이며(V7) — yaml 노브 중 필요조건은 `hf-overrides`(V12) 하나이고 나머지는 바꿔 잰 적 없는 승계값이다.

## 0.2 유효맥락
<!-- FACT:context -->
| 항목 | 값 | 출처 |
|---|---|---|
| 모델 | qwen3.8-flash-next-fp8 | sweep_index.meta(output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2) |
| HF repo | unsloth/Qwen3.8-Flash-Next-FP8 | 체크포인트 .git/config remote url(huggingface.co · /app/quant_models/Qwen/Qwen3.8-Flash-Next-FP8 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-FP8) |
| HF revision(체크포인트 git HEAD) | 41cc25fe32cc20053a59c89716196897580cddf6 | 체크포인트 .git HEAD → refs/heads/main → loose ref(/app/quant_models/Qwen/Qwen3.8-Flash-Next-FP8 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-FP8) · refs/remotes/origin/main 와 일치(받아 온 원격 커밋) — checkout 커밋(작업트리 일치는 미검증) · reflog: 측정 시점 checkout = 지금 HEAD(measured_utc 2026-10-25T01:51:17Z 이전 마지막 이동 2026-09-10T09:02:18Z) |
| base_model | Qwen/Qwen3.8-Flash-Next | 체크포인트 README base_model(/app/quant_models/Qwen/Qwen3.8-Flash-Next-FP8 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-FP8) |
| base 슬러그 | qwen3.8-flash-next | model(qwen3.8-flash-next-fp8) − vocab quant_suffixes(naming.base_slug) |
| 양자화 | fp8 | naming-axis(q) — 측정 identity quantization 미관측(sweep_index.meta(output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2)) · q 축 = /app/quant_models/Qwen/Qwen3.8-Flash-Next-FP8 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-FP8/config.json quantization_config quant_method=fp8 · vocab quant[fp8]←'fp8' — 인증서 quantization 이 N/A 라 명명 축 q(체크포인트 config 파생)를 옮겼다 |
| GPU | NVIDIA GB10 | sweep_index.meta(output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2) |
| 토폴로지 · TP | multi · TP=2 | sweep_index.meta(output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2) |
| 실행 평면 | docker | artifacts.plane_of |
| vLLM(측정 강식별 키 `vllm_version`) | 0.31.0 | sweep_index.meta(output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2) |

**명명 축** — 태그 이름 = 결정론부(vllm · model · arch · q · len · kv — 도구가 증거에서 파생 · 발행자 입력 ✗) + 꼬리(발행 Agent 가 고르고 서빙 설정 file · key · value 로 근거 대조 · 0.9) + 중복 시 timestamp.

| 축 | 값 | 출처 |
|---|---|---|
| 세그먼트 `vllm` | 0.31.0 | track=선택자 Dockerfile.source-build · ref=docker history build-arg VLLM_REF · version=VLLM_VERSION 미관측 · repo=docker history build-arg VLLM_REPO · sha=빌드 원장 vllm.git_sha(output/multi/benchlog/attestation_q38fn31-fp8-1m-tp2.json · measured build-time in-image · describe=v0.31.0 · parity.vllm_sha=equal — resolved.json 미바인딩) · VLLM_REF=v0.31.0(업스트림 릴리스 태그 · v 제거) |
| 세그먼트 `model` | qwen3.8-flash-next-fp8 | 서빙 yaml model(output/multi/configs/q38fn31-fp8-1m-tp2.yaml) · hf_repo=체크포인트 .git/config remote url(huggingface.co · /app/quant_models/Qwen/Qwen3.8-Flash-Next-FP8 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-FP8) · hf_repo(unsloth/Qwen3.8-Flash-Next-FP8) 마지막 성분 소문자 |
| 세그먼트 `arch` | gb10-1g2n-cluster-native | derived(hw·gpus_per_node·nodes·role·target·plane) |
| 세그먼트 `recipe` | qfp8-len1048576-kvfp8-plemmap | derived(q·len·kv) + tail(agent · naming.tail[] 근거) |
| 축 `hw` | gb10 | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(자동 파생 — 측정 TP=2(sweep_index.meta.tensor_parallel_size(output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2 · generated_utc = 측정 키)) > gpus_per_node=1(output/multi/manifest.yaml(manifest_contract 로더)) · 단일 노드 형상일 수 없다 · 일치 신호: 멀티 serve proof output/multi/benchlog/serve_proof_q38fn31-fp8-1m-tp2.json(kind=multinode_serve_proof) · 노드 정합 attestation output/multi/benchlog/attestation_q38fn31-fp8-1m-tp2.json) · sweep meta gpu_model 일치 · target: output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/roofline.json gpu_model — 루브릭 계산 HW(측정 호스트 manifest) · 캠페인 밖 측정(측정 시점 ACTIVE=_bootstrap · 이 측정의 캠페인 인스턴스 없음) 로 타겟 선언 미관측(재생 전용 대체 · 선언 아님) · vocab hw[gb10]←'NVIDIA GB10' |
| 축 `gpus_per_node` | 1 | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(자동 파생 — 측정 TP=2(sweep_index.meta.tensor_parallel_size(output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2 · generated_utc = 측정 키)) > gpus_per_node=1(output/multi/manifest.yaml(manifest_contract 로더)) · 단일 노드 형상일 수 없다 · 일치 신호: 멀티 serve proof output/multi/benchlog/serve_proof_q38fn31-fp8-1m-tp2.json(kind=multinode_serve_proof) · 노드 정합 attestation output/multi/benchlog/attestation_q38fn31-fp8-1m-tp2.json) · sweep meta gpu_model 일치 · target: output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/roofline.json gpu_model — 루브릭 계산 HW(측정 호스트 manifest) · 캠페인 밖 측정(측정 시점 ACTIVE=_bootstrap · 이 측정의 캠페인 인스턴스 없음) 로 타겟 선언 미관측(재생 전용 대체 · 선언 아님) |
| 축 `nodes` | 2 | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(자동 파생 — 측정 TP=2(sweep_index.meta.tensor_parallel_size(output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2 · generated_utc = 측정 키)) > gpus_per_node=1(output/multi/manifest.yaml(manifest_contract 로더)) · 단일 노드 형상일 수 없다 · 일치 신호: 멀티 serve proof output/multi/benchlog/serve_proof_q38fn31-fp8-1m-tp2.json(kind=multinode_serve_proof) · 노드 정합 attestation output/multi/benchlog/attestation_q38fn31-fp8-1m-tp2.json) · sweep meta gpu_model 일치 · target: output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/roofline.json gpu_model — 루브릭 계산 HW(측정 호스트 manifest) · 캠페인 밖 측정(측정 시점 ACTIVE=_bootstrap · 이 측정의 캠페인 인스턴스 없음) 로 타겟 선언 미관측(재생 전용 대체 · 선언 아님) |
| 축 `role` | cluster | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(자동 파생 — 측정 TP=2(sweep_index.meta.tensor_parallel_size(output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2 · generated_utc = 측정 키)) > gpus_per_node=1(output/multi/manifest.yaml(manifest_contract 로더)) · 단일 노드 형상일 수 없다 · 일치 신호: 멀티 serve proof output/multi/benchlog/serve_proof_q38fn31-fp8-1m-tp2.json(kind=multinode_serve_proof) · 노드 정합 attestation output/multi/benchlog/attestation_q38fn31-fp8-1m-tp2.json) · sweep meta gpu_model 일치 · target: output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/roofline.json gpu_model — 루브릭 계산 HW(측정 호스트 manifest) · 캠페인 밖 측정(측정 시점 ACTIVE=_bootstrap · 이 측정의 캠페인 인스턴스 없음) 로 타겟 선언 미관측(재생 전용 대체 · 선언 아님) |
| 축 `target` | native | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(자동 파생 — 측정 TP=2(sweep_index.meta.tensor_parallel_size(output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2 · generated_utc = 측정 키)) > gpus_per_node=1(output/multi/manifest.yaml(manifest_contract 로더)) · 단일 노드 형상일 수 없다 · 일치 신호: 멀티 serve proof output/multi/benchlog/serve_proof_q38fn31-fp8-1m-tp2.json(kind=multinode_serve_proof) · 노드 정합 attestation output/multi/benchlog/attestation_q38fn31-fp8-1m-tp2.json) · sweep meta gpu_model 일치 · target: output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/roofline.json gpu_model — 루브릭 계산 HW(측정 호스트 manifest) · 캠페인 밖 측정(측정 시점 ACTIVE=_bootstrap · 이 측정의 캠페인 인스턴스 없음) 로 타겟 선언 미관측(재생 전용 대체 · 선언 아님) · target_gpu 'NVIDIA GB10' = 호스트 hw → native |
| 축 `plane` | docker | artifacts.plane_of → cell-env(IMAGE_TAG\|BUILD_DOCKERFILE) · vocab plane[docker]→'' |
| 축 `q` | fp8 | /app/quant_models/Qwen/Qwen3.8-Flash-Next-FP8 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-FP8/config.json quantization_config quant_method=fp8 · vocab quant[fp8]←'fp8' |
| 축 `len` | 1048576 | 서빙 yaml max-model-len(output/multi/configs/q38fn31-fp8-1m-tp2.yaml) |
| 축 `kv` | fp8 | 서빙 yaml kv-cache-dtype(output/multi/configs/q38fn31-fp8-1m-tp2.yaml) · vocab kv[fp8]←'fp8_e4m3' |
| 꼬리 `plemmap` | PLE n-gram 테이블을 상주 대신 로컬 디스크 mmap 으로 읽는 런타임 플러그인 활성(stock 상주 형상은 같은 HW 에서 예산 차단) | `output/multi/envs/.env.q38fn31-fp8-1m-tp2` · 키 `VLLM_PLE_MMAP` = `1` |
<!-- /FACT:context -->

> 이 절이 답하는 질문: 이 지도는 어떤 조건에서만 성립하며, 이 형상은 어떤 기전으로 버티고 다른 형상은 왜 실패했는가?

(가) **이 조건에서만 성립하는 것**
- **통합메모리(호스트 RAM = GPU 메모리) 노드** → stock `v0.31.0` 의 PLE 백엔드 둘(Device · PinnedHost)은 47.7 GiB 테이블을 모두 같은 풀에 재료화한다. 그래서 FP8 TP=2 상주 형상은 노드 예산을 넘고, 테이블을 재료화하지 않는 디스크 mmap 만 절감을 낸다. 호스트 RAM 이 GPU 메모리와 분리된 HW 에서는 stock PinnedHost 가 이미 GPU 메모리를 덜 쓸 수 있다. 그 경우 이 플러그인의 이유가 약해진다(추론 · 계보에 대조 없음). 근거 `testlog_26102420_PLE_mmap_0.31.0_재이식_S1_선판정_런타임패치_슬롯_정정` §3.
- **노드당 메모리 대비 가중치 비율** → mmap 형상에서도 노드당 모델 적재가 62.75 GiB(엔진 실측)다. 예산 선언은 weights 64050 MiB(R2 산식 · 선언) · KV 14,336 MiB · overhead 24,800 MiB 로 바닥 21422 MiB 를 적었다(원장 선언 — 엔진 실측 62.75 GiB 를 넣으면 같은 산식의 바닥은 더 작다). 측정 중 시스템 RAM 은 main 77% · sub 74% 였다. 노드 메모리가 작으면 이 형상도 다시 재야 하고, 크면 상주 형상(R1)이 열릴 수 있다. 근거 `testlog_26102414_q38fn_fp8_멀티_TP2_예산선판정_차단` §근거 · `bench_report_26102510_qwen3.8-flash-next-fp8_GB10_0.31.0` §lite 지표 (full ⊇ lite — 열 집합 중첩 보장).
- **노드당 GPU 1 · TP=2 가 RoCE 를 탄다** → 플러그인은 매 조회 행을 호스트에서 모아 장치로 올린 뒤 rank 간 all-reduce 한다(번들 `backend.py`). 그래서 eager 전용이고, 조회마다 호스트 동기화와 노드 간 통신이 들어간다. 노드 수 · 인터커넥트 · CUDA graph 가능 여부가 다르면 디코드 비용이 달라진다(크기는 미측정 · 02 Q1 · Q2). 근거 `testlog_26102420_…` §4.
- **가중치가 NAS(CIFS)에 있다** → 엔진이 auto-prefetch 를 끄고 가중치 적재에 main 716.60 s · sub 430.34 s 가 걸렸다(측정 엔진 로그 L608 · L610 · L677). 로컬 디스크나 다른 FS 에서는 기동 시간이 다르다.

(나) **이 형상이 버티는 기전 대 다른 형상**
- PLE 상주(stock Device · `cpu_offload false`) — 테이블을 노드 메모리 텐서로 둔다 — 노드당 상주 weights 88,464 MiB 로 예산 선판정이 음수 바닥을 내 로드 0초에서 막혔다 — 확정(게이트 산술 · 원장 `budget_blocked`), 실기동 거동은 미결 — `testlog_26102414_q38fn_fp8_멀티_TP2_예산선판정_차단` §근거.
- PinnedHost(`cpu_offload true`) — 테이블을 pinned 호스트 텐서로 둔다 — 통합메모리라 같은 풀을 써 절감 0 — 미결(소스 판독 · 기동 대조 없음) — `testlog_26102420_…` §3.
- 디스크 mmap(이 셀) — 테이블은 디스크 파일 페이지로 남고 조회 행만 읽는다 — 엔진 적재 62.75 GiB · 예산 선언 수리 · 원장 사살 0 으로 버텼다 — 버틴 사실은 확정(엔진 로그 · 원장)이다. 페이지 캐시(회수 가능) 대 상주 anonymous(회수 불가)가 로드 피크 완충을 가른다는 기전은 이전 vLLM NVFP4 셀의 MemAvail 산술 대조에 근거하고(`plan_26091216_qwen38fn_res최소조건_후속캠페인` §2.3 · `perf_26091305_qwen38fn_res최소조건_후속캠페인` §3), 이 셀에서는 미검증이다 — 02 W6.
- 싱글 TP=1 — 노드 1대에 전체 가중치 — 체크포인트 172.78 GiB 가 노드 예산을 넘는다는 산술 — 미결(사전판정 · 기동 없음) — `testlog_26102412_q38fn_fp8_싱글_사전판정_불가_멀티이관` §근거.
- 형상을 연 설정 변경 전부(상주 시도 → 이 셀). 셀 env 에 `VLLM_PLE_MMAP=1` · `VLLM_PLE_MMAP_DIR=/app/ple_mmap/q38fn-fp8` · `RUNTIME_PATCH_STEM` 을 추가했고, `VLLM_PLE_MMAP_VERIFY` 를 넣고, 런타임 패치 파일을 만들고, 서브 배달 허용목록에 stem 을 등재했다(0daa0d0). 로컬 스테이징 33파일은 새로 만든 것이 아니라 이전 캠페인 때 이미 있었다(`plan_26091108_qwen38fn_22셀_재수행_캠페인` §2 · 01 §1.6 (b)). 상주 시도 쪽 다른 노브는 차단 testlog 유효맥락(KV fp8 14,336 MiB)만 기록돼 있어 전체 대조는 미확인이다 — 02 §2.5.

## 0.3 벽 지도 요약
> 02-narrative.md §2.2 의 `kind: wall` 블록(과 §2.4 의 `kind: misdiagnosis` 블록)에서 기계가 생성한 표다(손저작 ✗ · 순서 = 넘은 순서). 원인·해소의 전문과 원문 발췌는 02 에 있다.

<!-- FACT:wall_map -->
| 순서 | id | 증상 | 해소 | 전이등급 | 검증 |
|---|---|---|---|---|---|
| 1 | W1 | YaRN 확장 길이(512K)를 선언한 셀이 기동 즉시 max_model_len 검증 실패로 종료(관측 1셀 · 나머지 512k/1m 7셀은 미시도) | 교정 R8 — hf-overrides 에 rope_parameters(mrope_interleaved · partial_rotary_factor 0.25 · rope_type yarn · factor 4 · original_max_position_embeddings 262144)를 싣는다 | arch-invariant | testlog_26091121_qwen38fn_22셀재수행_캠페인완주_판정 §R8 실서빙 재검증 — factor=4 완결 (1m mmp 4셀 기동 · 측정) |
| 2 | W2 | FP8 셀이 KV 캐시 산출 직후 컨테이너 종료(당시 hang 으로 기록) | 교정 R1(셀 기동을 예산 게이트 경로로 일원화) + R2(resident weights = (ckpt − PLE×[mmap]) ÷ tp) | arch-scaled | testlog_26091121_qwen38fn_22셀재수행_캠페인완주_판정 §패턴 최종 확정 — PLE mmap 유무가 262k~1m 전 구간의 생사 분기축 (FP8 mmp 측정 · FP8 res budget_block) |
| 3 | W3 | 서빙 성공 셀에서 GuideLLM 벤치 도중 측정 사망 | 교정 R3 — 벤치 진입 게이트 floor − bench_budget ≥ abs_band(10240 MiB) · GuideLLM 컨테이너 --memory 하드 제한 · 로드 게이트에는 더하지 않음 | arch-scaled | 이 측정 level 1 반복 3/3 완주 · server_alive_at_bench_end true(output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/level_01/measured.json) |
| 4 | W4 | rope_parameters 만 오버라이드한 1M 요청이 v0.31.0 에서 ValidationError(derived 길이 262144) | hf-overrides text_config 에 max_position_embeddings=1048576 을 rope_parameters 와 함께 싣는다(우회 env 미사용) | arch-invariant | 이 셀 측정 기동 non-default args 의 max_position_embeddings 1048576 · max_model_len 1048576(lite_engine_head L64) · KV 1048576 tokens per request 기준 산정(lite_engine L695) |
| 5 | W5 | PLE 상주 FP8 TP=2 셀이 스모크 진입 직후 rc=4 budget_blocked(로드 0초) | PLE 를 디스크 mmap 으로 읽는 런타임 플러그인(W6) — 예산 weights 64050 MiB · floor 21422 MiB 로 선언 수리 | arch-scaled | 원장 budget_honored floor_mib 21422 · arm_ceiling_mib 18350(docs/logs/main/events/watchdog.jsonl) · serve proof health 200 + 추론 1회 |
| 6 | W6 | 이전 vLLM 의 PLE mmap 빌드 패치 62 를 v0.31.0 소스에 얹을 수 없음(소스 판독 · 기동 없음) | out-of-tree 플러그인 qwen_ple_mmap_plugin 0.1.0(q38fn31-fp8-1m-tp2_patch.py) — register() 가 Device/PinnedHost 전역을 Qwen4ExpPLEMmapEmbedding 으로 교체 · VLLM_PLE_MMAP=1 일 때만 · 런타임 패치 슬롯 · 이미지 재빌드 없음 | arch-locked | 측정 기동 엔진 로그 PLE mmap backend installed(lite_engine L402 · lite_engine_head L109) · 128 shards in 33 files(L428 · L618) · verify OK 3줄 FAILED 0(L693 · L706 · L717) · Model loading 62.75 GiB(L625 · L679) |

**오진 · 정정**(02 §2.4 — 현재지위가 `가설(강등)` 인 기전은 처방이 아니다)

| id | 증상 | 현재지위 | 검증 |
|---|---|---|---|
| M1 | FP8 셀 컨테이너가 기동 중 종료 | 반증 | plan_26091108_캠페인재수행_선행_하네스교정 §2.1 사망 원인은 hang 이 아니라 호스트 워치독 사살 |
| M2 | 이전 vLLM 의 NVFP4 hint 태그 본문이 FP8 계열 실패를 서술 | 반증 | plan_26091108_캠페인재수행_선행_하네스교정 §2.8 발행된 hint 태그가 오진을 배포 중이다 |
| M3 | yaml 에 async-scheduling false 를 적으면 비동기 스케줄링이 꺼진다고 보고 셀 yaml 마다 승계 | 반증 | output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/lite_engine_head_q38fn31-fp8-1m-tp2.log L64(non-default args) |
| M4 | NAS du 가 FP8 체크포인트를 346G 로 보고 | 반증 | plan_26102401_4모델_싱글멀티_최대context_양자화_기동검증 §정정·진행 기록 (2026-10-24 KST · P0~P1 진행 중) |
<!-- /FACT:wall_map -->

## 0.4 결정론 해소값
<!-- FACT:resolved -->
| 항목 | 값 | 출처 |
|---|---|---|
| vLLM 빌드 입력(종류) | release | naming.vllm_build_input |
| vLLM 빌드 입력(ref) | v0.31.0 | docker history build-arg VLLM_REF |
| vLLM 소스 SHA(40자) | db9527a46873454610df6dbedf79a36d6bf1a7f6 | 빌드 원장 vllm.git_sha(output/multi/benchlog/attestation_q38fn31-fp8-1m-tp2.json · measured build-time in-image · describe=v0.31.0 · parity.vllm_sha=equal — resolved.json 미바인딩) |
| 직전 릴리스(prev_release) | 미관측 | 미관측 |
| vLLM 엔진 자기보고(엔진 로그 기동 배너) | 0.31.1.dev0+gdb9527a46.d20261014 | output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/lite_engine_head_q38fn31-fp8-1m-tp2.log:60(이 측정의 엔진 로그 기동 배너) · 지위 `this-measurement` |
| 인증서 `vllm_version`(강한 일치 키) | 미관측 | 생산자 `absent` · 인증서 vllm_version 없음 |
| wheel 메타(`vllm_build`) | 0.31.1.dev0+gdb9527a46.d20261014.cu133 | attestation nodes.<n>.vllm_dist_version(output/multi/benchlog/attestation_q38fn31-fp8-1m-tp2.json · 노드별 관측) — main=sub 일치 · 작성 2026-10-25T01:39:26Z < 측정 시작 2026-10-25T01:39:47Z — 측정 전(빌드·스모크)의 관측 · 원장 순서로 판별한 같은 기동의 관측(스모크 로그 미보존 · 약한 판별)(판별 근거 = attestation 범위 줄 · 원장 docs/logs/main/events/2026-10.jsonl) |
| 체크포인트 revision(HF · git HEAD) | 41cc25fe32cc20053a59c89716196897580cddf6 | 체크포인트 .git HEAD → refs/heads/main → loose ref(/app/quant_models/Qwen/Qwen3.8-Flash-Next-FP8 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-FP8) · refs/remotes/origin/main 와 일치(받아 온 원격 커밋) — checkout 커밋(작업트리 일치는 미검증) · reflog: 측정 시점 checkout = 지금 HEAD(measured_utc 2026-10-25T01:51:17Z 이전 마지막 이동 2026-09-10T09:02:18Z) |
| 빌드 트랙 | source-build | artifacts 선택자(build-ledger.dockerfile) |
| Dockerfile(선택자) | Dockerfile.source-build | build-ledger.dockerfile |
| vLLM 저장소 | https://github.com/vllm-project/vllm.git | docker history build-arg VLLM_REPO |
| 이미지 태그 | easy-vllm:0.31.0-cu133-aarch64-source | 셀 env(output/multi/envs/.env.q38fn31-fp8-1m-tp2) IMAGE_TAG |
| 이미지 digest | sha256:339737df6571ce00993e731ef064a8db66085beaa650e4aa63bdb24f414b2034 | sweep_index.meta.image_digest(measured(docker inspect .Image)) |
| torch | 2.13.0a0+9186a08 | docker image inspect Config.Env PYTORCH_VERSION |
| CUDA | 13.3.1.008 | docker image inspect Config.Env CUDA_VERSION |
| NGC 베이스 | 26.07 | docker image inspect Config.Env NVIDIA_PYTORCH_VERSION |
| CPU arch | arm64 | docker image inspect Architecture |
| 드라이버 | 580.178.04 | attestation parity.driver(output/multi/benchlog/attestation_q38fn31-fp8-1m-tp2.json · 노드별 관측 · verdict=equal) — main=sub 일치 · 작성 2026-10-25T01:39:26Z < 측정 시작 2026-10-25T01:39:47Z — 측정 전(빌드·스모크)의 관측 · 원장 순서로 판별한 같은 기동의 관측(스모크 로그 미보존 · 약한 판별)(판별 근거 = attestation 범위 줄 · 원장 docs/logs/main/events/2026-10.jsonl) |
| 드라이버(노드별 관측) | main=580.178.04 · sub=580.178.04 | attestation parity.driver(output/multi/benchlog/attestation_q38fn31-fp8-1m-tp2.json · 노드별 관측 · verdict=equal) — main=sub 일치 · 작성 2026-10-25T01:39:26Z < 측정 시작 2026-10-25T01:39:47Z — 측정 전(빌드·스모크)의 관측 · 원장 순서로 판별한 같은 기동의 관측(스모크 로그 미보존 · 약한 판별)(판별 근거 = attestation 범위 줄 · 원장 docs/logs/main/events/2026-10.jsonl) |
| ⚠ 드라이버 관측 ≠ 선언 | 충돌 없음 — 관측 580.178.04 = 선언 580.178.04 | 관측: attestation parity.driver(output/multi/benchlog/attestation_q38fn31-fp8-1m-tp2.json · 노드별 관측 · verdict=equal) — main=sub 일치 · 작성 2026-10-25T01:39:26Z < 측정 시작 2026-10-25T01:39:47Z — 측정 전(빌드·스모크)의 관측 · 원장 순서로 판별한 같은 기동의 관측(스모크 로그 미보존 · 약한 판별)(판별 근거 = attestation 범위 줄 · 원장 docs/logs/main/events/2026-10.jsonl) · 선언: sweep_index.meta.driver_version(output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2 · 측정 시 조립 — sweep_bench.sh 가 manifest 에서 읽는다) (manifest 선언 옮김) |
| 호스트 OS | 미관측 | 미관측 |

> 재현 좌표는 빌드 입력(릴리스 태그 또는 40자 SHA)이다 — 엔진 자기보고 · 인증서 · wheel 의 버전 문자열은 빌드 입력이 아니다(각 값을 만든 생산자는 출처 칸).

**양자화 구성**(체크포인트 config 관측 · 이름의 `q` 축은 선언 방식 하나 — ModelOpt MIXED 는 층 개수 우세가 보조 규칙)

| 범위 | dtype | 출처 |
|---|---|---|
| quantization_config 선언 대상(양자화 층 · 제외 목록 밖) | `fp8 (weight_block_size=[128, 128])` | /app/quant_models/Qwen/Qwen3.8-Flash-Next-FP8 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-FP8/config.json quantization_config quant_method |
| 기본 dtype(비양자화 가중치·계산) | `bfloat16` | /app/quant_models/Qwen/Qwen3.8-Flash-Next-FP8 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-FP8/config.json text_config dtype |

**이 모델에 필요한 패치**: 실린 빌드 패치 없음(또는 파일 기록 미관측 — 01 §1.1).
<!-- /FACT:resolved -->

> 이 절이 답하는 질문: 위 값들 중 무엇을 그대로 참조하고 무엇을 네 환경에서 다시 도출해야 하며, 이 빌드 트랙은 왜 골랐는가?

(1) **빌드 트랙** — source-build 다(위 표 · 선택자 `Dockerfile.source-build`). 이유는 빌드 레시피 머리의 판정 사유다. torch 2.11+ 구간에서 prebuilt vLLM wheel 의 `_C` 가 NGC alpha torch 와 C++ ABI 가 맞지 않아 import 단계에서 막힌다. 그래서 `_C` 를 NGC torch 에 맞춰 직접 컴파일한다(`artifacts/build_recipe/Dockerfile.source-build` L29-32 · 이미지 torch 는 위 표의 2.13.0a0). 그 문장에서 파생하면 wheel 트랙으로 갈 조건은 "그 wheel 의 `_C` 가 베이스 이미지 torch 와 ABI 가 맞을 때" 이고, 최종 판정은 어느 트랙이든 스모크다. 이 셀은 이미지를 새로 짓지 않았다(공유 이미지 · PLE mmap 은 런타임 패치).

(2) **vLLM 버전 문자열** — 빌드 입력은 docker history build-arg `VLLM_REF=v0.31.0` 이다(위 표). 엔진 기동 배너 `0.31.1.dev0+gdb9527a46.d20261014` 와 wheel 메타 `…cu133` 은 빌드된 소스 트리에서 매겨진 개발 버전 문자열이다. 생산자는 엔진 기동 배너와 attestation 의 설치 배포판 메타이고, 빌드 입력이 아니다(`dev0` · `d<날짜>` 꼬리의 뜻은 문자열 모양에서 읽은 해석). 인증서 `vllm_version` 은 인증서가 없어 비었다. 0.2 의 `vllm_version` 0.31.0 은 측정 도구의 스윕 meta 값이다. **재현 좌표는 릴리스 태그 `v0.31.0`** 이다. 40자로는 attestation 의 양 노드 `vllm_sha` `db9527a46873454610df6dbedf79a36d6bf1a7f6`(위 표 `vLLM 소스 SHA(40자)` 행 · describe=v0.31.0)이다. 배너의 `gdb9527a46` 은 그 SHA 의 앞부분이다.

(3) **arch-locked 값과 재도출** — 네 HW 에서 하나씩 다시 정한다.
- `TORCH_CUDA_ARCH=12.1a`(build-arg · GB10 compute capability) → 네 GPU 의 compute capability 로 다시 정한다.
- CPU arch arm64(이미지 · 베이스 `nvcr.io/nvidia/pytorch:26.07-py3` 의 arm64 판) → 네 호스트 `uname -m` 에 맞는 베이스를 고른다.
- 드라이버 580.178.04 계열(호스트) → 네 드라이버가 이미지 CUDA 13.3 을 받는지 확인한다.
- NCCL 인터커넥트 값(`NCCL_IB_GID_INDEX=3` · RoCE 장치 2개 병합 — 01 §1.5) → 네 NIC · GID 표로 다시 정한다.
- 플러그인의 PLE 샤드 레이아웃(FP8 판 = 33파일 분산) → 네 체크포인트 index 로 다시 확인한다(01 §1.6).

(4) **선언 대 관측** — 해소값 파일(resolved.json)은 이 빌드에 묶이지 않았다(결손 `HINT_MISSING_RESOLVE`). 그래서 torch · NGC · CUDA 는 위 표의 이미지 관측값뿐이다. 실린 빌드 파일은 커밋 `12ef50a` 의 `output/single/` 바이트다 — 이 이미지는 single-node 체크아웃의 미커밋 렌더(`689610f` 기준)로 지어졌고 그 커밋은 이미지 Created 뒤라, 이 이미지를 지은 개정이라고 단정하지 않는다(01 §1.2). `Dockerfile.source-build` 의 `ARG VLLM_REF` 기본값은 v0.31.0 이고(L46) docker history build-arg 도 `VLLM_REF=v0.31.0` 이다. 다만 Dockerfile 은 history 와 16/17 일치라 `generated-unverified` 이고(가드 echo 한 단계 차이 · 01 §1.1), `requirements.txt` 는 빌드 원장 sha256 일치로 `verified` 다. `use_existing_torch.py` 는 `v0.31.0` 에서 `tools/` 로 옮겨졌는데, 실린 Dockerfile 은 `tools/` 를 먼저 찾아(L82) 이 단계에서 멈추지 않는다 — 이 multi 체크아웃 작업트리의 `output/multi/Dockerfile.source-build` 는 루트만 찾아 멈춘다(01 §1.2 build_recipe).

(5) **사실 블록이 비운 환경 좌표** — 호스트 OS 는 계보에 **미기록**이다. 노드당 메모리는 `plan_26091108_qwen38fn_22셀_재수행_캠페인` §2 의 `MemTotal 124,610 MiB`(121.7 GiB)이고, 예산 원장의 `mem_total_mib 124608` 은 선언값이다. 인터커넥트는 RoCE 다(`plan_26090918` §1 · 측정 엔진 NCCL `Using network IB` · RoCE 장치 · GPU Direct RDMA 꺼짐 — 01 §1.4 (f)). CUDA 는 층이 다르다. 스윕 meta `cuda_version 132` 는 manifest 선언을 옮긴 호스트 층 값이다(manifest `cuda_version: "132"` · `cuda_version_raw: "13.2"`). 이미지 층은 CUDA 13.3.1.008 · torch 2.13.0a0+9186a08 · NGC 26.07 이다(위 표). 둘을 섞지 않는다.

(6) **핵심 의존 핀** — 관측된 것은 셋이다. Ray `RAY_VERSION=2.48.0`(build-arg · 공유 이미지의 멀티용 기본값) · NCCL 2.30.7+cuda13.3(측정 엔진 로그 · 이미지 안) · 측정 클라이언트 GuideLLM 0.7.3(이미지 digest 기록)이다. 이 모델의 요구는 이미지 핀이 아니다. 런타임 플러그인 하나(번들 `qwen_ple_mmap_plugin` 0.1.0 · numpy · torch · vllm 은 이미지 것을 import)다. 실린 `requirements.txt` 는 빌드 원장 sha256 과 일치하므로(01 §1.1) 그 핀은 이 이미지가 설치하라고 받은 값이다 — `flashinfer-python==0.7.0.post1`(L20) · `compressed-tensors==0.17.0`(L14) · `transformers<5.18.0,>=5.10.4,<5.15.0`(L73). 공유 이미지의 기본 의존이지 이 모델의 요구가 아니다. 실제 설치된 버전(pip 해소 결과) · Triton · DeepGEMM 의 이미지 안 버전은 **미관측**이다.

## 0.5 서빙 노브와 값의 지위
> 01-artifacts.md §1.3 의 `kind: value-status` 블록에서 기계가 생성한 요약이다. 지위가 `tuned` · `declared-requirement` 가 아닌 값은 **이 셀에서 조정된 적도, 빼면 깨진다고 확인된 적도 없다** — 필요조건으로 복사하지 마라. `declared-requirement` 는 빼거나 바꾸면 실패가 관측된 값이다(근거의 W id · artifacts 헤더를 보고 네 환경에서도 그 실패 조건이 성립하는지 판단한다).

<!-- FACT:knob_status -->
노브 12개 — `inherited` 11 · `declared-requirement` 1.

| 노브 | 값 | 지위 | 근거 | id |
|---|---|---|---|---|
| `tensor-parallel-size` | 2 | inherited | TP=1 은 산술 사전판정으로 배제(R3 · 기동 실패 관측 아님) — 노드 2 × GPU 1 manifest 형상에서 온 값이며 다른 TP 를 잰 기록 없음 | V1 |
| `distributed-executor-backend` | ray | inherited | 멀티 서빙 경로(serve_runner 가 Ray head/worker 를 띄운다)의 공통값 — 다른 백엔드 시도 0 | V2 |
| `gpu-memory-utilization` | 0.80 | inherited | 이 셀에서 바꿔 잰 기록 없음 · kv-cache-memory-bytes 를 주므로 엔진이 KV 산정에 gmu 를 따르지 않는다고 스스로 적는다(lite_engine L694) | V3 |
| `max-model-len` | 1048576 | inherited | 사람이 정한 목표(지원 최대치 기동)이자 이전 vLLM 1m 축의 값 — 이 셀에서 길이를 바꿔 잰 기록 없음 · 1M needle 통과는 정정 기록(plan_26102420)이 적는다 | V4 |
| `kv-cache-dtype` | fp8_e4m3 | inherited | 이전 vLLM 22셀 재수행에서 bf16 KV 와 함께 잰 축이나(MTP on · vLLM bench 조건) 이 셀에서는 바꿔 잰 적 없음 — 같은 캠페인 NVFP4 셀과 같은 값 | V5 |
| `kv-cache-memory-bytes` | 15032385536 | inherited | 14 GiB 절대 클램프 — 엔진 2,112,392 토큰 = 1,048,576 요청 기준 2.01배(lite_engine L695) · 이전 vLLM 20 GiB 에서 상주 NVFP4 1M TRIP 이 있어 낮춘 값(yaml 주석) · 이 셀에서 바꿔 잰 기록 없음 | V6 |
| `enforce-eager` | true | inherited | 이 셀에서 cudagraph 시도 0 · 플러그인 v1 이 eager 가 아니면 생성자에서 RuntimeError 로 막는다(번들 backend.py · 실기동 관측 없음 — 그래서 필요조건 관측은 아니다) | V7 |
| `async-scheduling` | false | inherited | 이전 vLLM 스모크 yaml 에서 승계 · 이 셀 측정 기동 non-default args 에 실리지 않아 효과 없음(M3 · lite_engine_head L64) | V8 |
| `max-num-seqs` | 1 | inherited | 동시 시퀀스 1 — 같은 캠페인 NVFP4 셀과 같은 값 · 다른 값을 잰 기록 없음(측정도 동시성 1 레벨뿐) | V9 |
| `max-num-batched-tokens` | 2048 | inherited | 이 셀 계보에서 값을 바꿔 잰 기록 없음(손으로 둔 값 · 근거 문서 없음) | V10 |
| `engram-config` | {"cpu_offload": false} | inherited | stock 상주 형상에서 쓰던 값(PinnedHost 는 GB10 에서 절감 0 · R2) · VLLM_PLE_MMAP=1 이면 플러그인이 Device · PinnedHost 를 둘 다 교체해 이 값과 무관해진다(번들 주석) | V11 |
| `hf-overrides` | {"text_config":{"max_position_embeddings":1048576,"rope_parameters":{"mrope_interleaved":true,"mrope_section":[11,11,10],"partial_rotary_factor":0.25,"rope_theta":10000000,"rope_type":"yarn","factor":4,"original_max_position_embeddings":262144}}} | declared-requirement | max_position_embeddings 를 빼면 v0.31.0 에서 1M 이 ValidationError(W4 · 같은 캠페인 NVFP4 1M 셀) · rope 오버라이드가 없으면 YaRN 확장 길이가 ValidationError(W1 · 이전 vLLM NVFP4 512K 셀 1개 관측) — 두 실패 모두 이 FP8 셀이 아니라 같은 모델의 다른 양자화 셀에서 관측(FP8 config 도 rope_type default · max_position_embeddings 262144 로 같은 형상) | V12 |
<!-- /FACT:knob_status -->

> 이 절이 답하는 질문: 이 레시피를 다른 환경으로 옮길 때 어떤 노브를 먼저 다시 재야 하고, 어떤 노브는 빼면 깨지는가?

① **네 환경에서 다시 도출할 값(arch-scaled)** — `kv-cache-memory-bytes`(V6) · `gpu-memory-utilization`(V3) · `max-num-seqs`(V9) · `max-model-len`(V4) · `max-num-batched-tokens`(V10) · `tensor-parallel-size`(V1). 도출 절차는 이렇다.
- 노드당 가중치는 엔진 `Model loading took … GiB` 실측으로 잡는다. 이 형상은 PLE 를 빼고 센다.
- KV 는 엔진 `GPU KV cache size: N tokens` 를 클램프 바이트로 나눈 토큰당 바이트 × 목표 컨텍스트 × 동시 시퀀스로 잡는다.
- 남는 호스트 메모리를 예산 선언으로 확인한다.

이 셀의 클램프는 1M 요청 하나를 2.01배로 덮는 값이다(V6).

② **필요조건으로 확인된 값(declared-requirement)** — `hf-overrides`(V12) 하나다. rope 오버라이드가 없으면 YaRN 확장 길이가 ValidationError 였고(W1 · 이전 vLLM NVFP4 512K 셀 1개의 관측), `max_position_embeddings` 를 빼면 `v0.31.0` 에서 1M 이 ValidationError 였다(W4 · 같은 캠페인 NVFP4 1M 셀). 두 실패 모두 같은 모델의 NVFP4 셀에서 관측됐고, 이 FP8 셀에서는 관측되지 않았다. yaml 노브는 아니지만 빼면 깨지는 것은 셀 env 의 플러그인 활성 줄이다(`VLLM_PLE_MMAP=1` · `VLLM_PLE_MMAP_DIR` · `RUNTIME_PATCH_STEM` — 빼면 W5 의 상주 형상으로 돌아간다). **설정하지 않아야 하는 것**: CUDA graph(`enforce-eager` 해제)다. 플러그인 v1 이 기동 때 `RuntimeError` 로 막는다(번들 소스 · 실기동 관측 없음 · V7). **적어도 효과가 없는 것**: yaml 의 `async-scheduling: false`(M3 · V8) · PLE mmap 활성 시 `engram-config` 의 `cpu_offload`(플러그인이 두 백엔드를 다 교체 · V11).

③ **조정된 적 없는 승계값(inherited)** — `distributed-executor-backend` ray(V2) · `kv-cache-dtype` fp8_e4m3(V5 · 이전 vLLM 에서 bf16 과 함께 잰 축이나 이 셀에선 고정) · `enforce-eager`(V7 · 플러그인 제약) · `async-scheduling`(V8) · `engram-config`(V11). ① 의 값들도 지위는 전부 inherited 다(이 셀 스윕 0). "이 값으로 한 번 떴다" 는 뜻이지 최적이라는 뜻이 아니다. `max-num-batched-tokens` 2048(V10)은 근거 문서 없이 손으로 둔 값이다.

## 0.6 재검증 · 라우팅
- 이 자료를 `vllm-recipe-explorer` 인터뷰의 **warm-start 근거로만** 투입하고, 평소대로 plan-gate → 레시피 수렴 → 스모크 게이트를 그대로 밟아라(가속이지 우회 ✗).

> 이 절이 답하는 질문: 다른 환경(다른 HW · 다른 vLLM 버전 · 다른 노드 수)에서 이 지도를 쓸 때 무엇부터, 어떤 순서로 확인해야 하며, 실패하면 어디로 돌리는가?

1. **대상 vLLM 의 PLE 임베딩 구조가 이 플러그인과 맞는가** — 확인 방법: 대상 버전 소스(또는 이미지 안 `/workspace/vllm-src`)에서 다음을 grep 한다. `vllm/models/qwen4_exp/common/ngram_embedding.py` 의 `Qwen4ExpPLEEmbedding`, `vllm/models/qwen4_exp/nvidia/ngram_embedding.py` 의 모듈 전역 `Qwen4ExpPLEDeviceEmbedding` · `Qwen4ExpPLEPinnedHostEmbedding`, 그리고 디스크 기반 백엔드가 새로 있는지(이 셀의 판정 방법 · W6 · 02 Q4). 분기:
   - 디스크 기반 백엔드가 정식으로 있으면 → 플러그인을 **빼고** 정식 경로로 다시 레시피를 세운다.
   - ABC · 전역 이름이 그대로면 → 플러그인을 그대로 시도하되 스모크로 중재한다.
   - 이름 · 생성자가 바뀌었으면 → 번들(01 §1.2 해제 명령)을 그 API 로 다시 이식한다(지도는 참고로만).
2. **노드 메모리가 PLE 상주를 덮는가** — 확인 방법: 예산 선판정(이 하네스 `budget_preflight.py`)에 노드 MemTotal · `(ckpt − PLE×[mmap]) ÷ tp` · KV · overhead 를 넣는다. 분기: 상주 형상도 바닥이 양수면 → mmap 이 필요 없을 수 있다(상주로 재도출 · R1 재개조건). 통합메모리이고 상주가 음수면 → mmap 형상으로 간다. 분리 메모리 HW 면 → stock PinnedHost 도 후보다(R2 재개조건).
3. **스테이징이 양 노드 로컬의 같은 경로에 있고 FP8 판을 가리키는가** — 확인 방법: 01 §1.6 의 index 명령으로 PLE 샤드 파일 목록을 뽑고, 노드마다 `ls … | wc -l` 과 원본 `cmp` 를 한다. 셀 env `VLLM_PLE_MMAP_DIR` 를 확인한다(compose 기본값은 NVFP4 경로). 분기: 맞으면 그대로. 경로가 비거나 다른 판이면 기동하지 않고 고친다.
4. **기동 성공 판정** — health 200 + 추론 1회 + 양 rank `PLE mmap backend installed` + `128 shards in <파일 수> files … dir=<FP8 경로>` + `verify OK`(`VLLM_PLE_MMAP_VERIFY>0` 일 때)이고, `verify FAILED` 0 이어야 한다. 분기: 서브 rank 에 설치 줄이 없으면 → 서브의 패치 파일 실재와 `RUNTIME_PATCH_STEM` 전달을 고친다(01 §1.5) · FAILED 가 있으면 → 이 지도 폐기(조회 정합 붕괴).
5. **호스트 메모리 예산 선언** — 확인 방법: 기동 전 예산을 선언하고 원장 `budget_honored` 를 확인한다. 입력은 노드당 가중치(엔진 실측 — 이 셀 62.75 GiB) · KV 클램프 · overhead · MemTotal 이다. 벤치는 `floor − bench_budget ≥ abs_band` 로 따로 확인한다. 분기: 선언이 거부되면 → KV 를 줄이거나 형상을 바꾼다. 가드를 우회하지 않는다(W2 · W5).
6. **1M 길이 검증** — 확인 방법: 엔진 `non-default args` 에 `max_position_embeddings` 와 rope_parameters 가 함께 실렸는지 본다. `/v1/models` 의 `max_model_len` 을 확인한다. 길이를 증분 프리필로 올리며 needle 을 본다. 분기: ValidationError 면 → W4 대로 `max_position_embeddings` 를 함께 오버라이드한다. 우회 env 는 쓰지 않는다(R6).

실패 신호별 행선지는 다음과 같다.
- 빌드 · 버전 핀 · 상위 버전의 PLE 구조 → `upstream-version-watch`
- 서빙 설정 · KV · 노브 · 런타임 패치 내용 · 스테이징 → `vllm-recipe-explorer`
- 성능 기대 이하(이 셀의 weak REFUTE 포함) → `adversarial-benchmark`
- 노드 · 토폴로지 · 인터커넥트(서브 합류 · NCCL · 서브 스테이징 실재) → `terraforming_node`

## 0.7 메타
<!-- FACT:meta -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.31.0/qwen3.8-flash-next-fp8/gb10-1g2n-cluster-native/qfp8-len1048576-kvfp8-plemmap` |
| 형식 | `hint-payload/v7` |
| 앵커 | `PROVENANCE.json` — 봉인 때 hint 브랜치 페이로드 커밋에 묶인다 |
| 캠페인 · 셀 · 노드 | 캠페인 밖(`_bootstrap` · output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/sweep_index.json bench_knobs.campaign = null · campaign_pins_note '활성 캠페인 없음(_bootstrap)'(측정 시점 ACTIVE 해소 · repeat_axis)) · q38fn31-fp8-1m-tp2 · cluster |
| 발행 모드 | publication-replay |
| 발행 토픽 | q38fn31_fp8_1m_tp2 |
| work-manifest | docs/_evidence/q38fn31_fp8_1m_tp2.work-manifest.json |
| 증거 등급(task_class) | full_benchmark |
| 승인 출처 | cli:--approved-by |
| 승인 시각(UTC) | 2026-10-25T04:00:00Z |
| 승인 발화(전사) | 사용자(발화 요지): 5셀 hint 발행 제안(Qwen FP8·NVFP4 멀티, Solar 멀티, Gemma 싱글, Qwen NVFP4 싱글 mmap)에 '오케이 힌트 태그도 발행해줘' — 태그 push 는 발행 직전 재확인 |
| 현행 full 정의 충족(측정 등급) | 예 · 이 측정의 반복 3 · 정의 출처 `.claude/rules/docs.md:42`(03 §3.5) |
| 독립 사실 검증(발행 전) | 통과 — 검증자 `독립 사실 검증 에이전트`(저작자 `hint 저작 에이전트` 아님) · 주장 345건 · 지적 18건 중 고침 18건 · 열림 0 · 2026-10-25T13:46:25Z |
<!-- /FACT:meta -->

<!-- FACT:missing -->
**결손** — 발행을 막지 않고 기재한다(부재와 실패는 다른 사실이다 · 차단은 양성 검출만).

| 코드 | 뜻 | 이 셀의 사유 |
|---|---|---|
| `HINT_MISSING_CAMPAIGN_INSTANCE` | 캠페인 인스턴스를 읽지 않았다(발행 기록 재생 — purge · 캠페인 밖 측정 · 보존됐으나 재생 모드라 읽지 않음 중 어느 쪽인지는 이 셀의 사유) — 셀 lockset·config·cell.status·journey·grounding 이 이 페이로드에 없다. 값의 지위와 여정 한 줄은 문서(testlog·devlog) 서사에서만 복원된다. | 캠페인 밖 측정 — purge 된 인스턴스가 아니라 이 측정의 인스턴스는 애초에 없다(output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/sweep_index.json bench_knobs.campaign = null · campaign_pins_note '활성 캠페인 없음(_bootstrap)'(측정 시점 ACTIVE 해소 · repeat_axis)) · 같은 셀의 앞선 캠페인 인스턴스 campaigns/camp-26102412-multi-sol-q38/cells/q38fn-fp8-1m-tp2(journey 마지막 campaign_closed) 는 보존돼 있으나 이 측정의 것이 아니라 읽지 않았다 |
| `HINT_MISSING_RESOLVE` | `output/<t>/resolved.json` 을 이 빌드에 묶지 못했다(부재 또는 다른 vLLM 버전의 해소값) — torch·NGC·CUDA 는 이미지 관측값만 싣는다. | — |
| `HINT_MISSING_SWEEP_LEVELS` | 부하 레벨이 1개 이하 — 부하 거동을 알 수 없다(경량 리포트는 동시성 곡선을 재지 않는다). | — |
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

- 발행 절차의 사정(사실): 이 페이로드는 **발행 기록 재생**(publication-replay)이고, 측정 자체가 캠페인 밖(`_bootstrap`)이라 캠페인 인스턴스가 애초에 없다. 그래서 셀 lockset · config · 캠페인 선언은 실리지 않았고, 값의 지위는 문서 서사에서 복원했다(00 결손 `HINT_MISSING_CAMPAIGN_INSTANCE`).
- 계보 읽기 목록 밖 문서(사실): 이 셀의 S2~S3 판정(플러그인 구현 · 1M needle 단계 · S3 lite · 원시 pread 오라클 대조 · `prompt_logprobs` 동치 · yaml `async-scheduling: false` 무효 발견)을 적은 `testlog_26102500_PLE_mmap_재이식_Qwen_FP8멀티_NVFP4싱글_Solar_완결이식_S2_S4_판정` 은 계보 그래프에서 경유(transit · 기반 슬러그 필터 불일치)로만 잡혔다. 그래서 발췌 · 사건 출처로 쓸 수 없었다. 산문에서 그 문서를 가리킨 자리는 모두 "계보 경유 · 읽기 목록 밖" 으로 표시했다. 같은 사실은 가능한 한 계보 안 문서(`plan_26102420_…` 정정 기록)와 이 측정의 엔진 로그로 근거를 댔다.
- 의견: 이 셀 재현 키트의 중심은 빌드가 아니라 런타임 패치 한 파일과 노드별 로컬 스테이징이다. 01 §1.2 해제 명령으로 소스를 읽고, 네 vLLM 의 PLE 임베딩 구조부터 확인하라(0.6 1번).

## 0.9 이름 꼬리
> 태그 이름의 결정론부(`<vllm>/<model>/<arch>/q·len·kv`)는 도구가 정한다. 꼬리는 이 셀을 **같은 q·len·kv 의 다른 셀과 가르는 노브**를 발행 Agent 가 골라 근거와 함께 적는 자리다(plan_26092908 §4.1 U5). 아래 표는 publish 때 기계가 낸 **후보**이고(강제 ✗), `continue` 가 draft `inputs/tail.json` 을 근거 대조한 뒤 확정 꼬리로 바꾼다. 같은 이름이 이미 있으면 끝에 발행 시각 `-t<YYMMDDHHMM>`(KST)이 붙는다.

<!-- FACT:name_tail -->
**확정 이름** `hint/0.31.0/qwen3.8-flash-next-fp8/gb10-1g2n-cluster-native/qfp8-len1048576-kvfp8-plemmap` — 꼬리 1토큰.

| 토큰 | 뜻 | 근거(서빙 설정) |
|---|---|---|
| `plemmap` | PLE n-gram 테이블을 상주 대신 로컬 디스크 mmap 으로 읽는 런타임 플러그인 활성(stock 상주 형상은 같은 HW 에서 예산 차단) | `output/multi/envs/.env.q38fn31-fp8-1m-tp2` · 키 `VLLM_PLE_MMAP` = `1` |
<!-- /FACT:name_tail -->

> 이 절이 답하는 질문: 이 셀을 같은 q·len·kv 의 다른 셀과 가르는 노브는 무엇이며 그것을 이름 꼬리로 어떻게 적는가 — 가르는 노브가 없으면 왜 빈 꼬리인가?

꼬리는 `plemmap` 하나다(근거 = 셀 env `VLLM_PLE_MMAP=1`). 결정론부가 같은 형상은 stock PLE 상주 FP8 TP=2 셀이다. 그 셀은 같은 HW 에서 예산 선판정에 막혀 서빙되지 않았다(R1 · W5). 이 셀을 그것과 가르는 노브가 PLE 적재 방식이고, 카탈로그의 같은 모델 계열 태그도 PLE 방식을 꼬리 토큰(`plemmap` · `pleresident`)으로 가른다. `eager` 는 쓰지 않았다. 이 플러그인이 eager 를 강제하므로(V7) mmap 형상 안에서 고를 수 있는 노브가 아니고, 계보에 짝 셀도 없다. `specoff` 도 쓰지 않았다. 이 계보의 `v0.31.0` 문서에 이 모델을 spec decode 로 띄운 기동 기록이 없어 가를 상대가 없다.
