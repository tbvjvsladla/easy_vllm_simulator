# 00 · hint 지도 — 가장 먼저 읽는 파일

<!-- FACT:header -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.31.0/solar-open2-250b-nota-nvfp4/gb10-1g2n-cluster-native/qnvfp4-len1048576-kvfp8` |
| 형식 | `hint-payload/v7` · 이름 문법 `v7` |
| 생성(UTC · 주입) | 2026-10-25T04:10:00Z |
| 캠페인 · 셀 · 노드 · 모드 | 캠페인 밖(`_bootstrap` · output/multi/benchlog/sweep_solar31-nv4-1m-tp2/sweep_index.json bench_knobs.campaign = null · campaign_pins_note '활성 캠페인 없음(_bootstrap)'(측정 시점 ACTIVE 해소 · repeat_axis)) · solar31-nv4-1m-tp2 · cluster · publication-replay |
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
| 자격 근거 | `output/multi/benchlog/serve_proof_solar31-nv4-1m-tp2.json` — 시점 범위: 작성 2026-10-25T02:57:42Z(output/multi/benchlog/serve_proof_solar31-nv4-1m-tp2.json 파일 mtime(파일시스템 관측 — 문서에 시각 칸이 없다 · 기록 필드 아님)) · 측정 창 대비 작성 2026-10-25T02:57:42Z < 측정 시작 2026-10-25T02:58:03Z — 측정 전(빌드·스모크)의 관측 |
| 증거 등급(task_class) | full_benchmark |
| 측정 등급(bench_mode) | full |
| 셀 등급(cell_grade) | `full` — 성능 판정 · 부하 곡선 · 데이터셋 출처는 `03-benchmark.md` |
| 성능 판정(measurement.verdict) | `PASS` — 출처 output/multi/benchlog/sweep_solar31-nv4-1m-tp2/verdict.json(output · verdict_rule 출력) verdict |
<!-- /FACT:grade -->

<!-- FACT:lite_summary -->
<!-- /FACT:lite_summary -->

## 0.1 요약
> 이 절이 답하는 질문: 이 셀은 무엇을 서빙했고, 무엇이 결정적이었으며, 수신자는 무엇을 가장 조심해야 하는가?

Solar-Open2-250B-Nota-NVFP4 를 GB10 2노드(노드당 GPU 1 · Ray TP=2 · expert-parallel) · vLLM `v0.31.0` 공유 이미지에 **자체 이식 런타임 플러그인**을 얹어 max-model-len 1048576(native · YaRN 없음) · KV fp8_e4m3 · eager · spec decode 없음으로 서빙했고, GuideLLM 동시성 1 decode 16.67 t/s(weak PASS · 인증서 없음)다.
가장 비싼 벽은 stock 0.31.0 의 Solar-Open2 미지원(W1 — Upstage 포크 델타를 `v0.31.0` API 로 다시 쓴 플러그인 `solar31-nv4-1m-tp2_patch.py`)과 서브 Ray 워커의 플러그인 부재(W3 — `RUNTIME_PATCH_STEM` 전달 · 양 노드 무장)다.
가장 오해하기 쉬운 값: yaml 의 `async-scheduling: false` 는 무효이고(M1) 러너 `--no-async-scheduling` 이 V2 프로세서의 필요조건이다(W7) — yaml 서빙 노브 16개는 전부 조정 · 단독 제거 대조가 없는 승계값이다(V1~V16).

## 0.2 유효맥락
<!-- FACT:context -->
| 항목 | 값 | 출처 |
|---|---|---|
| 모델 | solar-open2-250b-nota-nvfp4 | sweep_index.meta(output/multi/benchlog/sweep_solar31-nv4-1m-tp2) |
| HF repo | nota-ai/Solar-Open2-250B-Nota-NVFP4 | 체크포인트 .git/config remote url(huggingface.co · /app/quant_models/Upstage/Solar-Open2-250B-Nota-NVFP4 → <manifest.quant_model_path>/Upstage/Solar-Open2-250B-Nota-NVFP4) |
| HF revision(체크포인트 git HEAD) | de88f6226788077e2d340204fd79d37720c9eda0 | 체크포인트 .git HEAD → refs/heads/main → loose ref(/app/quant_models/Upstage/Solar-Open2-250B-Nota-NVFP4 → <manifest.quant_model_path>/Upstage/Solar-Open2-250B-Nota-NVFP4) · refs/remotes/origin/main 와 일치(받아 온 원격 커밋) — checkout 커밋(작업트리 일치는 미검증) · reflog: 측정 시점 checkout = 지금 HEAD(measured_utc 2026-10-25T03:13:30Z 이전 마지막 이동 2026-07-24T04:28:17Z) |
| base_model | upstage/Solar-Open2-250B | 체크포인트 README base_model(/app/quant_models/Upstage/Solar-Open2-250B-Nota-NVFP4 → <manifest.quant_model_path>/Upstage/Solar-Open2-250B-Nota-NVFP4) |
| base 슬러그 | solar-open2-250b-nota | model(solar-open2-250b-nota-nvfp4) − vocab quant_suffixes(naming.base_slug) |
| 양자화 | nvfp4 | naming-axis(q) — 측정 identity quantization 미관측(sweep_index.meta(output/multi/benchlog/sweep_solar31-nv4-1m-tp2)) · q 축 = /app/quant_models/Upstage/Solar-Open2-250B-Nota-NVFP4 → <manifest.quant_model_path>/Upstage/Solar-Open2-250B-Nota-NVFP4/config.json quantization_config quant_method=compressed-tensors format=nvfp4-pack-quantized · vocab quant[nvfp4]←'compressed-tensors:nvfp4-pack-quantized' — 인증서 quantization 이 N/A 라 명명 축 q(체크포인트 config 파생)를 옮겼다 |
| GPU | NVIDIA GB10 | sweep_index.meta(output/multi/benchlog/sweep_solar31-nv4-1m-tp2) |
| 토폴로지 · TP | multi · TP=2 | sweep_index.meta(output/multi/benchlog/sweep_solar31-nv4-1m-tp2) |
| 실행 평면 | docker | artifacts.plane_of |
| vLLM(측정 강식별 키 `vllm_version`) | 0.31.0 | sweep_index.meta(output/multi/benchlog/sweep_solar31-nv4-1m-tp2) |

**명명 축** — 태그 이름 = 결정론부(vllm · model · arch · q · len · kv — 도구가 증거에서 파생 · 발행자 입력 ✗) + 꼬리(발행 Agent 가 고르고 서빙 설정 file · key · value 로 근거 대조 · 0.9) + 중복 시 timestamp.

| 축 | 값 | 출처 |
|---|---|---|
| 세그먼트 `vllm` | 0.31.0 | track=선택자 Dockerfile.source-build · ref=docker history build-arg VLLM_REF · version=VLLM_VERSION 미관측 · repo=docker history build-arg VLLM_REPO · sha=빌드 원장 vllm.git_sha(output/multi/benchlog/attestation_solar31-nv4-1m-tp2.json · measured build-time in-image · describe=v0.31.0 · parity.vllm_sha=equal — resolved.json 미바인딩) · VLLM_REF=v0.31.0(업스트림 릴리스 태그 · v 제거) |
| 세그먼트 `model` | solar-open2-250b-nota-nvfp4 | 서빙 yaml model(output/multi/configs/solar31-nv4-1m-tp2.yaml) · hf_repo=체크포인트 .git/config remote url(huggingface.co · /app/quant_models/Upstage/Solar-Open2-250B-Nota-NVFP4 → <manifest.quant_model_path>/Upstage/Solar-Open2-250B-Nota-NVFP4) · hf_repo(nota-ai/Solar-Open2-250B-Nota-NVFP4) 마지막 성분 소문자 |
| 세그먼트 `arch` | gb10-1g2n-cluster-native | derived(hw·gpus_per_node·nodes·role·target·plane) |
| 세그먼트 `recipe` | qnvfp4-len1048576-kvfp8 | derived(q·len·kv) |
| 축 `hw` | gb10 | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(자동 파생 — 측정 TP=2(sweep_index.meta.tensor_parallel_size(output/multi/benchlog/sweep_solar31-nv4-1m-tp2 · generated_utc = 측정 키)) > gpus_per_node=1(output/multi/manifest.yaml(manifest_contract 로더)) · 단일 노드 형상일 수 없다 · 일치 신호: 멀티 serve proof output/multi/benchlog/serve_proof_solar31-nv4-1m-tp2.json(kind=multinode_serve_proof) · 노드 정합 attestation output/multi/benchlog/attestation_solar31-nv4-1m-tp2.json) · sweep meta gpu_model 일치 · target: output/multi/benchlog/sweep_solar31-nv4-1m-tp2/roofline.json gpu_model — 루브릭 계산 HW(측정 호스트 manifest) · 캠페인 밖 측정(측정 시점 ACTIVE=_bootstrap · 이 측정의 캠페인 인스턴스 없음) 로 타겟 선언 미관측(재생 전용 대체 · 선언 아님) · vocab hw[gb10]←'NVIDIA GB10' |
| 축 `gpus_per_node` | 1 | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(자동 파생 — 측정 TP=2(sweep_index.meta.tensor_parallel_size(output/multi/benchlog/sweep_solar31-nv4-1m-tp2 · generated_utc = 측정 키)) > gpus_per_node=1(output/multi/manifest.yaml(manifest_contract 로더)) · 단일 노드 형상일 수 없다 · 일치 신호: 멀티 serve proof output/multi/benchlog/serve_proof_solar31-nv4-1m-tp2.json(kind=multinode_serve_proof) · 노드 정합 attestation output/multi/benchlog/attestation_solar31-nv4-1m-tp2.json) · sweep meta gpu_model 일치 · target: output/multi/benchlog/sweep_solar31-nv4-1m-tp2/roofline.json gpu_model — 루브릭 계산 HW(측정 호스트 manifest) · 캠페인 밖 측정(측정 시점 ACTIVE=_bootstrap · 이 측정의 캠페인 인스턴스 없음) 로 타겟 선언 미관측(재생 전용 대체 · 선언 아님) |
| 축 `nodes` | 2 | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(자동 파생 — 측정 TP=2(sweep_index.meta.tensor_parallel_size(output/multi/benchlog/sweep_solar31-nv4-1m-tp2 · generated_utc = 측정 키)) > gpus_per_node=1(output/multi/manifest.yaml(manifest_contract 로더)) · 단일 노드 형상일 수 없다 · 일치 신호: 멀티 serve proof output/multi/benchlog/serve_proof_solar31-nv4-1m-tp2.json(kind=multinode_serve_proof) · 노드 정합 attestation output/multi/benchlog/attestation_solar31-nv4-1m-tp2.json) · sweep meta gpu_model 일치 · target: output/multi/benchlog/sweep_solar31-nv4-1m-tp2/roofline.json gpu_model — 루브릭 계산 HW(측정 호스트 manifest) · 캠페인 밖 측정(측정 시점 ACTIVE=_bootstrap · 이 측정의 캠페인 인스턴스 없음) 로 타겟 선언 미관측(재생 전용 대체 · 선언 아님) |
| 축 `role` | cluster | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(자동 파생 — 측정 TP=2(sweep_index.meta.tensor_parallel_size(output/multi/benchlog/sweep_solar31-nv4-1m-tp2 · generated_utc = 측정 키)) > gpus_per_node=1(output/multi/manifest.yaml(manifest_contract 로더)) · 단일 노드 형상일 수 없다 · 일치 신호: 멀티 serve proof output/multi/benchlog/serve_proof_solar31-nv4-1m-tp2.json(kind=multinode_serve_proof) · 노드 정합 attestation output/multi/benchlog/attestation_solar31-nv4-1m-tp2.json) · sweep meta gpu_model 일치 · target: output/multi/benchlog/sweep_solar31-nv4-1m-tp2/roofline.json gpu_model — 루브릭 계산 HW(측정 호스트 manifest) · 캠페인 밖 측정(측정 시점 ACTIVE=_bootstrap · 이 측정의 캠페인 인스턴스 없음) 로 타겟 선언 미관측(재생 전용 대체 · 선언 아님) |
| 축 `target` | native | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(자동 파생 — 측정 TP=2(sweep_index.meta.tensor_parallel_size(output/multi/benchlog/sweep_solar31-nv4-1m-tp2 · generated_utc = 측정 키)) > gpus_per_node=1(output/multi/manifest.yaml(manifest_contract 로더)) · 단일 노드 형상일 수 없다 · 일치 신호: 멀티 serve proof output/multi/benchlog/serve_proof_solar31-nv4-1m-tp2.json(kind=multinode_serve_proof) · 노드 정합 attestation output/multi/benchlog/attestation_solar31-nv4-1m-tp2.json) · sweep meta gpu_model 일치 · target: output/multi/benchlog/sweep_solar31-nv4-1m-tp2/roofline.json gpu_model — 루브릭 계산 HW(측정 호스트 manifest) · 캠페인 밖 측정(측정 시점 ACTIVE=_bootstrap · 이 측정의 캠페인 인스턴스 없음) 로 타겟 선언 미관측(재생 전용 대체 · 선언 아님) · target_gpu 'NVIDIA GB10' = 호스트 hw → native |
| 축 `plane` | docker | artifacts.plane_of → cell-env(IMAGE_TAG\|BUILD_DOCKERFILE) · vocab plane[docker]→'' |
| 축 `q` | nvfp4 | /app/quant_models/Upstage/Solar-Open2-250B-Nota-NVFP4 → <manifest.quant_model_path>/Upstage/Solar-Open2-250B-Nota-NVFP4/config.json quantization_config quant_method=compressed-tensors format=nvfp4-pack-quantized · vocab quant[nvfp4]←'compressed-tensors:nvfp4-pack-quantized' |
| 축 `len` | 1048576 | 서빙 yaml max-model-len(output/multi/configs/solar31-nv4-1m-tp2.yaml) |
| 축 `kv` | fp8 | 서빙 yaml kv-cache-dtype(output/multi/configs/solar31-nv4-1m-tp2.yaml) · vocab kv[fp8]←'fp8_e4m3' |
<!-- /FACT:context -->

> 이 절이 답하는 질문: 이 지도는 어떤 조건에서만 성립하며, 이 형상은 어떤 기전으로 버티고 다른 형상은 왜 실패했는가?

(가) **이 조건에서만 성립하는 것**
- **통합메모리 노드 2대 · 노드당 GPU 1** → 가중치가 노드당 71.6 GiB(엔진 실측)로 갈리고 KV 14 GiB 클램프를 더해 측정 중 시스템 RAM 이 main 83% · sub 82% 까지 찼다 → 노드 메모리가 더 작거나 GPU 메모리가 분리된 HW 에서는 KV · gmu · TP 를 처음부터 다시 도출해야 한다. 근거 plan_26102401 §NAS 사실 (실측 2026-10-24) · bench_report_26102512 §lite 지표 (full ⊇ lite — 열 집합 중첩 보장).
- **NVFP4 MoE 를 yaml `moe-backend: marlin` 으로 고정** → 엔진이 `MARLIN` NvFp4 백엔드(weight-only FP4 경고 동반)로 돈다 — 이것은 HW 판별 결과가 아니라 노브 선택이다(엔진 후보 목록에는 FLASHINFER_* · VLLM_CUTLASS 등이 있었고 시도 0 · 카드의 triton 은 목록에 없음 · R3 · V4) → 다른 HW · 다른 백엔드에서는 decode 수치가 달라질 수 있다. 근거 testlog_26102418_solar_open2_nvfp4_멀티_TP2_1M_자체이식_기동_needle_lite_판정 §2 · 측정 엔진 로그(lite_engine_head L64 · lite_engine L545 · L694).
- **TP=2 가 노드 간 RoCE 를 탄다 · 서브 GPU 전력 가드(80 W/60 s 버킷)** → 단발 프리필은 266,129 토큰까지 통과하고 400k 에서 서브 가드가 slave 를 사살했다(W5 · 단발 한계는 그 사이) → 노드 수 · 인터커넥트 · 가드 설정이 다르면 장문 검증 단위를 다시 잡는다. 근거 plan_26102412 §셀 (TP=2 이므로 노드 병렬 아님 · 순차) · testlog_26102418 §2.

(나) **이 형상이 버티는 기전 대 다른 형상**
- 싱글 TP=1(같은 NVFP4 · 1M) — 노드 1대에 전체 가중치 — 체크포인트가 노드 통합메모리(~121GiB)를 넘는다는 산술(판정 당시 입력은 du 286G · 뒤 기재는 142.76 GiB) — 미결(기동 대조 없음 · 산술 사전판정) — testlog_26102418_4모델_싱글멀티_연속캠페인_결과요약_종결 §결과표 · plan_26102401 §⚠ 사전에 짚을 전제.
- 400k 단발 프리필 대 40k 이하 증분(같은 형상) — 프리필 길이 — 단발은 서브 GPU 가 81 W · 버킷 60/60 으로 전력 가드를 채워 사살, 증분은 1,001,129 토큰까지 TRIP 0 — 확정(사살 기록과 증분 재시험의 대조) — testlog_26102418 §2 · §4.
- V2 logits processor + 비동기 스케줄링 대 + 동기 스케줄링 — 러너 `--no-async-scheduling` 유무(yaml 의 false 는 무효) — 프로세서가 apply 시점에 마지막 샘플 토큰이 확정돼 있어야 해서 비동기면 fail-loud 로 거부 — 확정(실패 로그 · 교정 뒤 기동) — `master_failure.log` L1265 · W7.
- 이전 구성(프로세서 · chat-template kwargs · 동기 플래그 없음) 대 측정 구성 — 세 설정이 함께 바뀜 — 이전 구성도 기동 · 1M needle · lite 는 통과했고 차이는 reasoning 분리 · 템플릿 강제와 lite −4.7% — lite 차이의 기전은 미결 — testlog_26102418 §5 · 03 §3.5.

## 0.3 벽 지도 요약
> 02-narrative.md §2.2 의 `kind: wall` 블록(과 §2.4 의 `kind: misdiagnosis` 블록)에서 기계가 생성한 표다(손저작 ✗ · 순서 = 넘은 순서). 원인·해소의 전문과 원문 발췌는 02 에 있다.

<!-- FACT:wall_map -->
| 순서 | id | 증상 | 해소 | 전이등급 | 검증 |
|---|---|---|---|---|---|
| 1 | W1 | stock 0.31.0 레지스트리에 SolarOpen2 모델 · 파서가 없어 기동 불가 판정(기동 시도 없음) | 포크 델타를 0.31 API 로 다시 쓴 런타임 플러그인 번들(solar31-nv4-1m-tp2_patch.py · entry point solar_open2 · register 가 config · 모델 · reasoning/tool 파서 등록) | arch-invariant | 측정 기동 엔진 로그 Resolved architecture SolarOpen2ForCausalLM(lite_engine_head L68) · serve proof health 200 + 추론 1회 |
| 2 | W2 | logits processor 클래스가 0.31 의 V2 인터페이스 subclass 검사에 걸려 기동 불가 | 첫 셀은 프로세서를 빼고 기동 · plan_26102420 S4 에서 슬롯 기반 V2 LogitsProcessor 로 재저작해 다시 넣음 | arch-invariant | 측정 기동 엔진 non-default args 에 logits_processors 가 실리고 서버 기동(lite_engine_head L64 · lite_engine L736) |
| 3 | W3 | 서브 TP rank 1 Ray 워커 생성 중 모듈 부재로 액터가 죽어 엔진 기동 실패 | sync_to_sub 스냅샷에 등재 stem 패치쌍 편입 · compose RUNTIME_PATCH_STEM 서브 전달 · arm_patch.sh 가 RUNTIME_PATCH_STEM 을 CONFIG_FILE 보다 먼저 읽음(커밋 d781f26) | judgment | 교정 뒤 slave 로그의 arm_patch armed 줄(serve_fail slave_failure.log L34) · 측정 기동에서 서브 rank 가 가중치 적재 완료(lite_engine L700 · L703) |
| 4 | W4 | 가중치 적재 중 conv1d weight_loader 호출이 인자 부족으로 실패 | KDA 서브클래스에서 conv1d 로더를 텐서 속성 읽기 어댑터로 감쌈(플러그인 kda_layer.py) | arch-invariant | 측정 기동 lite_engine 로그 L696 · L703 Model loading took 71.6 GiB(양 rank) |
| 5 | W5 | 400k 토큰 단발 프리필 중 서브 slave 컨테이너가 exit 137 로 사살 | 가드 우회 · 수정 없이 재기동 후 40k 이하 단위 증분 프리필 | arch-scaled | testlog_26102418 §4 2차 증분 1,001,129 토큰 PASS · TRIP 0 |
| 6 | W6 | 응답 content 에 사고 서술이 섞이고 reasoning 필드가 비어 파서 분리가 동작하지 않음 | default-chat-template-kwargs think_render_option=preserved + V2 logits processor(plan_26102420 S4) | arch-invariant | plan_26102420 §정정·진행 기록 (2026-10-25 KST · S2~S4 완료) 의 reasoning 분리 완결 · 측정 기동 non-default args(lite_engine_head L64) |
| 7 | W7 | 가중치 적재 후 워커 초기화에서 NotImplementedError 로 엔진 기동 실패 | 러너 solar31-nv4-1m-tp2.sh 명령줄에 --no-async-scheduling | arch-invariant | 측정 기동 non-default args 의 async_scheduling False(lite_engine_head L64) · 서버 기동(lite_engine L736) |
| 8 | W8 | full 벤치 예산 선언이 BLOCK | bench 예산 2048 MiB 로 선언 | arch-scaled | testlog_26102504 §2 표 2번 행 처방 · 측정 완주(bench_report 반복 3/3) |

**오진 · 정정**(02 §2.4 — 현재지위가 `가설(강등)` 인 기전은 처방이 아니다)

| id | 증상 | 현재지위 | 검증 |
|---|---|---|---|
| M1 | yaml 에 async-scheduling false 를 두었는데 V2 logits processor 가 비동기 스케줄링이라며 기동을 막았다 | 반증 | output/multi/benchlog/serve_fail_solar31-nv4-1m-tp2/master_failure.log L1265(교정 전 NotImplementedError) · lite_engine_head 로그 L64(교정 후 async_scheduling False) |
| M2 | sweep 실패(rc=3) 뒤 판정 PASS 가 나왔다 | 반증 | testlog_26102504_S5_full벤치_5셀_GuideLLM_판정_및_하네스_결함 §2 표 4번 행 · bench_report_26102512 생성일 |
| M3 | NAS du 가 Solar NVFP4 를 286G 로 보고했다 | 반증 | plan_26102412 §셀 · testlog_26102412 §이관(du 286G 는 부풀려짐) · lite_engine 로그 L546 · L696 · L703 |
<!-- /FACT:wall_map -->

## 0.4 결정론 해소값
<!-- FACT:resolved -->
| 항목 | 값 | 출처 |
|---|---|---|
| vLLM 빌드 입력(종류) | release | naming.vllm_build_input |
| vLLM 빌드 입력(ref) | v0.31.0 | docker history build-arg VLLM_REF |
| vLLM 소스 SHA(40자) | db9527a46873454610df6dbedf79a36d6bf1a7f6 | 빌드 원장 vllm.git_sha(output/multi/benchlog/attestation_solar31-nv4-1m-tp2.json · measured build-time in-image · describe=v0.31.0 · parity.vllm_sha=equal — resolved.json 미바인딩) |
| 직전 릴리스(prev_release) | 미관측 | 미관측 |
| vLLM 엔진 자기보고(엔진 로그 기동 배너) | 0.31.1.dev0+gdb9527a46.d20261014 | output/multi/benchlog/sweep_solar31-nv4-1m-tp2/lite_engine_head_solar31-nv4-1m-tp2.log:60(이 측정의 엔진 로그 기동 배너) · 지위 `this-measurement` |
| 인증서 `vllm_version`(강한 일치 키) | 미관측 | 생산자 `absent` · 인증서 vllm_version 없음 |
| wheel 메타(`vllm_build`) | 0.31.1.dev0+gdb9527a46.d20261014.cu133 | attestation nodes.<n>.vllm_dist_version(output/multi/benchlog/attestation_solar31-nv4-1m-tp2.json · 노드별 관측) — main=sub 일치 · 작성 2026-10-25T02:57:42Z < 측정 시작 2026-10-25T02:58:03Z — 측정 전(빌드·스모크)의 관측 · 원장 순서로 판별한 같은 기동의 관측(스모크 로그 미보존 · 약한 판별)(판별 근거 = attestation 범위 줄 · 원장 docs/logs/main/events/2026-10.jsonl) |
| 체크포인트 revision(HF · git HEAD) | de88f6226788077e2d340204fd79d37720c9eda0 | 체크포인트 .git HEAD → refs/heads/main → loose ref(/app/quant_models/Upstage/Solar-Open2-250B-Nota-NVFP4 → <manifest.quant_model_path>/Upstage/Solar-Open2-250B-Nota-NVFP4) · refs/remotes/origin/main 와 일치(받아 온 원격 커밋) — checkout 커밋(작업트리 일치는 미검증) · reflog: 측정 시점 checkout = 지금 HEAD(measured_utc 2026-10-25T03:13:30Z 이전 마지막 이동 2026-07-24T04:28:17Z) |
| 빌드 트랙 | source-build | artifacts 선택자(build-ledger.dockerfile) |
| Dockerfile(선택자) | Dockerfile.source-build | build-ledger.dockerfile |
| vLLM 저장소 | https://github.com/vllm-project/vllm.git | docker history build-arg VLLM_REPO |
| 이미지 태그 | easy-vllm:0.31.0-cu133-aarch64-source | 셀 env(output/multi/envs/.env.solar31-nv4-1m-tp2) IMAGE_TAG |
| 이미지 digest | sha256:339737df6571ce00993e731ef064a8db66085beaa650e4aa63bdb24f414b2034 | sweep_index.meta.image_digest(measured(docker inspect .Image)) |
| torch | 2.13.0a0+9186a08 | docker image inspect Config.Env PYTORCH_VERSION |
| CUDA | 13.3.1.008 | docker image inspect Config.Env CUDA_VERSION |
| NGC 베이스 | 26.07 | docker image inspect Config.Env NVIDIA_PYTORCH_VERSION |
| CPU arch | arm64 | docker image inspect Architecture |
| 드라이버 | 580.178.04 | attestation parity.driver(output/multi/benchlog/attestation_solar31-nv4-1m-tp2.json · 노드별 관측 · verdict=equal) — main=sub 일치 · 작성 2026-10-25T02:57:42Z < 측정 시작 2026-10-25T02:58:03Z — 측정 전(빌드·스모크)의 관측 · 원장 순서로 판별한 같은 기동의 관측(스모크 로그 미보존 · 약한 판별)(판별 근거 = attestation 범위 줄 · 원장 docs/logs/main/events/2026-10.jsonl) |
| 드라이버(노드별 관측) | main=580.178.04 · sub=580.178.04 | attestation parity.driver(output/multi/benchlog/attestation_solar31-nv4-1m-tp2.json · 노드별 관측 · verdict=equal) — main=sub 일치 · 작성 2026-10-25T02:57:42Z < 측정 시작 2026-10-25T02:58:03Z — 측정 전(빌드·스모크)의 관측 · 원장 순서로 판별한 같은 기동의 관측(스모크 로그 미보존 · 약한 판별)(판별 근거 = attestation 범위 줄 · 원장 docs/logs/main/events/2026-10.jsonl) |
| ⚠ 드라이버 관측 ≠ 선언 | 충돌 없음 — 관측 580.178.04 = 선언 580.178.04 | 관측: attestation parity.driver(output/multi/benchlog/attestation_solar31-nv4-1m-tp2.json · 노드별 관측 · verdict=equal) — main=sub 일치 · 작성 2026-10-25T02:57:42Z < 측정 시작 2026-10-25T02:58:03Z — 측정 전(빌드·스모크)의 관측 · 원장 순서로 판별한 같은 기동의 관측(스모크 로그 미보존 · 약한 판별)(판별 근거 = attestation 범위 줄 · 원장 docs/logs/main/events/2026-10.jsonl) · 선언: sweep_index.meta.driver_version(output/multi/benchlog/sweep_solar31-nv4-1m-tp2 · 측정 시 조립 — sweep_bench.sh 가 manifest 에서 읽는다) (manifest 선언 옮김) |
| 호스트 OS | 미관측 | 미관측 |

> 재현 좌표는 빌드 입력(릴리스 태그 또는 40자 SHA)이다 — 엔진 자기보고 · 인증서 · wheel 의 버전 문자열은 빌드 입력이 아니다(각 값을 만든 생산자는 출처 칸).

**양자화 구성**(체크포인트 config 관측 · 이름의 `q` 축은 선언 방식 하나 — ModelOpt MIXED 는 층 개수 우세가 보조 규칙)

| 범위 | dtype | 출처 |
|---|---|---|
| quantization_config 선언 대상(양자화 층 · 제외 목록 밖) | `compressed-tensors` | /app/quant_models/Upstage/Solar-Open2-250B-Nota-NVFP4 → <manifest.quant_model_path>/Upstage/Solar-Open2-250B-Nota-NVFP4/config.json quantization_config quant_method |
| 양자화 제외 모듈 530개 | `unquantized(아래 기본 dtype)` | /app/quant_models/Upstage/Solar-Open2-250B-Nota-NVFP4 → <manifest.quant_model_path>/Upstage/Solar-Open2-250B-Nota-NVFP4/config.json quantization_config ignore·exclude_modules |
| 기본 dtype(비양자화 가중치·계산) | `bfloat16` | /app/quant_models/Upstage/Solar-Open2-250B-Nota-NVFP4 → <manifest.quant_model_path>/Upstage/Solar-Open2-250B-Nota-NVFP4/config.json dtype |

**이 모델에 필요한 패치**: 실린 빌드 패치 없음(또는 파일 기록 미관측 — 01 §1.1).
<!-- /FACT:resolved -->

> 이 절이 답하는 질문: 위 값들 중 무엇을 그대로 참조하고 무엇을 네 환경에서 다시 도출해야 하며, 이 빌드 트랙은 왜 골랐는가?

(1) **빌드 트랙** — source-build 다(위 표 · 선택자 `Dockerfile.source-build`). 이유는 빌드 레시피 머리의 판정 사유다 — torch 2.11+ 구간에서 prebuilt vLLM wheel 의 `_C` 가 NGC alpha torch 와 C++ ABI 가 맞지 않아 import 단계에서 막히므로 `_C` 를 NGC torch 에 맞춰 직접 컴파일한다(`artifacts/build_recipe/Dockerfile.source-build` L29-32 · 이미지 torch 는 위 표의 2.13.0a0). 그 문장에서 파생하면 wheel 트랙으로 갈 조건은 "그 wheel 의 `_C` 가 베이스 이미지의 torch 와 ABI 가 맞을 때" 이고, 최종 판정은 어느 트랙이든 스모크다. 이 셀은 이미지를 새로 짓지 않았다 — 측정 이미지(digest `sha256:339737df…`)는 v0.31.0 bump 때 single-node 브랜치 체크아웃에서 `output/single/Dockerfile.source-build` 로 지은 공유 이미지다(계보 밖 문서 testlog_26101412_v031_14B이하_4종_양노드_상주서빙_판정 §표 · devlog_26101412 — 서브는 같은 태그를 자기 빌드킷으로 따로 지어 digest 가 다르다). Solar 지원은 런타임 패치에 있다.

(2) **vLLM 버전 문자열** — 빌드 입력은 docker history build-arg `VLLM_REF=v0.31.0`(위 표). 엔진 기동 배너 `0.31.1.dev0+gdb9527a46.d20261014` 와 wheel 메타 `…cu133` 은 빌드된 소스 트리에서 매겨진 개발 버전 문자열로 읽힌다(생산자 = 엔진 기동 배너 · attestation 의 설치 배포판 메타 — 빌드 입력이 아니다 · `dev0`·`d<날짜>` 꼬리의 뜻은 문자열 모양에서 읽은 해석). 인증서 `vllm_version` 은 인증서가 없어 비었고, 0.2 의 `vllm_version` 0.31.0 은 측정 도구의 스윕 meta 값이다. **재현 좌표는 릴리스 태그 `v0.31.0`** 이고, 40자로는 attestation 의 양 노드 `vllm_sha` `db9527a46873454610df6dbedf79a36d6bf1a7f6` 이다 (위 표의 `vLLM 소스 SHA(40자)` 행 = 빌드 원장 `vllm.git_sha` · describe=v0.31.0 과 같은 값) — 저작 시(2026-10-25) `git ls-remote https://github.com/vllm-project/vllm.git refs/tags/v0.31.0` 도 같은 SHA 를 냈다. 배너의 `gdb9527a46` 은 그 SHA 의 앞부분이다.

(3) **arch-locked 값과 재도출** — `TORCH_CUDA_ARCH=12.1a`(build-arg · GB10 의 compute capability 에 맞춘 값 — 네 GPU 의 compute capability 로 다시 정한다) · CPU arch arm64(이미지 · 베이스 `nvcr.io/nvidia/pytorch:26.07-py3` 의 arm64 판 — 네 호스트 `uname -m` 에 맞는 베이스를 고른다) · 드라이버 580.178.04 계열(호스트 · 네 드라이버가 이미지 CUDA 13.3 을 받는지 확인) · NCCL 인터커넥트 값(`NCCL_IB_GID_INDEX=3` · RoCE 장치 2개 병합 — 01 §1.5 · 네 NIC · GID 표로 다시 정한다).

(4) **선언 대 관측** — 해소값 파일(resolved.json)은 이 빌드에 묶이지 않았다(결손 `HINT_MISSING_RESOLVE`) — 그래서 torch · NGC · CUDA 는 위 표의 이미지 관측값뿐이다. 실린 빌드 레시피는 `12ef50a:output/single/…` 이다 — 이 이미지를 지은 바이트 그대로는 아니다: 12ef50a 는 이미지 Created(2026-10-14T01:45:19Z) **뒤** 03:07:26Z 의 커밋으로 스모크 PASS 뒤 검증키 (26.07-py3, 0.31.0) 를 승급했고(커밋 메시지), 계보 밖 testlog_26101412 는 빌드 입력을 'single-node 체크아웃(689610f + 미커밋 렌더)' 이라 적는다. 그래서 guard RUN 한 단계가 다르다 — 실린 L40 은 `[guard] … validated patch set … proceeding.` 을 echo 하고, 이미지 history(2026-10-14T10:06:46+09:00 층)는 `WARN: source-build key (26.07-py3 x vLLM 0.31.0) is UNVALIDATED — attempt-build under HITL override` 를 echo 한다(docker history 16/17 · generated-unverified · 01 §1.1 경고). `requirements.txt` 는 빌드 원장 sha256 과 바이트가 같아 쓰인 바이트로 확인됐다(verified · 머리 주석 v0.31.0 wheel 메타 기준). 실린 Dockerfile 의 `ARG VLLM_REF` 기본값은 v0.31.0(L46)이다. 서빙 이미지의 빌드 입력은 docker history build-arg `VLLM_REF=v0.31.0` 이다.

(5) **사실 블록이 비운 환경 좌표** — 호스트 OS: 계보에 **미기록**. 노드당 메모리: plan_26102401 §NAS 사실 "노드당 통합메모리 ~121GiB"(예산 원장의 mem_total_mib 124608 은 선언값). 인터커넥트: RoCE(plan_26102412 §목표 · 측정 엔진 NCCL `NET/IB` RoCE 장치 · GPU Direct RDMA 꺼짐). CUDA 두 값은 층이 다르다: 스윕 meta `cuda_version` 132 는 측정값이 아니라 manifest 선언을 옮긴 것이다(`sweep_bench.sh@d33b0878a6c3` L978 이 manifest 의 `cuda_version` 을 grep · manifest `cuda_version: "132"` · 서브 `cuda_version_raw: "13.2"` = 호스트 드라이버의 CUDA 지원 상한) — 호스트 층. 측정 엔진 NCCL 줄 `cudaDriverVersion 13030`(lite_engine_head L97)은 컨테이너 안에서 본 libcuda 버전이고, 이미지 env 가 `LD_LIBRARY_PATH` 에 `/usr/local/cuda/compat/lib` · `CUDA_DRIVER_VERSION=610.43.02` 를 싣는다(docker image inspect) — 호스트 13.2 보다 높은 13.3 은 이미지의 forward-compat libcuda 로 읽는 것이 정합이다(해석 · 이미지 층). 이미지 층은 CUDA 13.3.1.008 · torch 2.13.0a0+9186a08 · NGC 26.07(위 표) — 섞지 않는다.

(6) **핵심 의존 핀** — 관측된 것: Ray `RAY_VERSION=2.48.0`(build-arg · 공유 이미지의 멀티용 기본값) · NCCL 2.30.7+cuda13.3(측정 엔진 로그 · 이미지 안) · 측정 클라이언트 GuideLLM 0.7.3(이미지 digest 기록). 이 모델의 요구는 이미지 핀이 아니라 런타임 플러그인이다 — compressed-tensors 체크포인트라 엔진 config 가 `quantization=compressed-tensors` 로 읽었고(lite_engine_head L81), 플러그인 `__init__.py` 주석은 `transformers 5.14.1 only knows solar_open (v1), not solar_open2` 라 적고 그래서 `solar_open2` config 를 직접 등록한다(번들 저작자의 서술상 transformers 5.14.1 · 이미지 안 실측은 미관측). 실린 `requirements.txt`(빌드 원장 sha256 과 같은 바이트)의 핀: `flashinfer-python==0.7.0.post1`(L20) · `compressed-tensors==0.17.0`(L14) · `transformers<5.18.0,>=5.10.4,<5.15.0`(L73 · KNOWN_INCOMPAT 천장 transformers<5.15.0 — 머리 L5). 이 핀들은 공유 이미지의 vLLM 0.31.0 wheel 메타 기준값이지 Solar 전용 요구가 아니다. 설치 constraint 로 쓰였으니 실제 설치 버전은 이미지 안에서 다시 확인해야 한다(Triton · DeepGEMM 의 이미지 안 버전은 **미관측**).

## 0.5 서빙 노브와 값의 지위
> 01-artifacts.md §1.3 의 `kind: value-status` 블록에서 기계가 생성한 요약이다. 지위가 `tuned` · `declared-requirement` 가 아닌 값은 **이 셀에서 조정된 적도, 빼면 깨진다고 확인된 적도 없다** — 필요조건으로 복사하지 마라. `declared-requirement` 는 빼거나 바꾸면 실패가 관측된 값이다(근거의 W id · artifacts 헤더를 보고 네 환경에서도 그 실패 조건이 성립하는지 판단한다).

<!-- FACT:knob_status -->
노브 16개 — `inherited` 16.

| 노브 | 값 | 지위 | 근거 | id |
|---|---|---|---|---|
| `tensor-parallel-size` | 2 | inherited | 멀티 캠페인 plan 이 처음부터 TP=2 로 정했다 — 싱글 TP=1 배제는 산술 사전판정(R4 · 시도 0)이지 관측된 실패가 아니다 · 다른 TP 시도 0 | V1 |
| `distributed-executor-backend` | ray | inherited | 멀티 하네스의 Ray head/worker 기동 경로를 따른 값 · 다른 백엔드 시도 0 | V2 |
| `enable-expert-parallel` | true | inherited | HF 모델 카드 서빙 인자를 옮긴 값 · 끈 구성 시도 0 | V3 |
| `moe-backend` | marlin | inherited | 손레버 — 카드의 triton 이 0.31 NVFP4 후보에 없어 marlin 을 처음부터 골랐다(R3) · 다른 후보(FLASHINFER_* · VLLM_CUTLASS · HUMMING) 시도 0 | V4 |
| `gpu-memory-utilization` | 0.80 | inherited | 계보 문서에 다른 값 · 스윕 기록이 없다(첫 셀 testlog 유효맥락에도 적히지 않음) · KV 는 kv-cache-memory-bytes 절대 클램프로 따로 정했다 | V5 |
| `max-model-len` | 1048576 | inherited | 체크포인트 max_position_embeddings 그대로(rope_scaling 없음 · YaRN 없음) — 캠페인 목표가 지원 최대치라 바꾼 적 없다 | V6 |
| `kv-cache-dtype` | fp8_e4m3 | inherited | 첫 구성부터 같은 값 · 다른 dtype 시도 0 | V7 |
| `kv-cache-memory-bytes` | 15032385536 | inherited | 14 GiB 절대 클램프 · 스윕 0 · 엔진이 잡은 KV 1,206,177 토큰 = 1M 요청 1개의 1.15배(엔진 로그) — 필요량을 조금 넘긴 값이지 음성대조 과잉이 아니다 | V8 |
| `enforce-eager` | true | inherited | 첫 셀 testlog 유효맥락(eager)부터 같은 값 · cudagraph 시도 0 · eager 선택 이유(캡처 스파이크 실측 등) 기록 없음 | V9 |
| `max-num-seqs` | 1 | inherited | 첫 셀 testlog 유효맥락(max-num-seqs 1)부터 같은 값 · 1M 요청 1개 기준 KV 1.15배라 동시 2 는 KV 를 다시 도출해야 한다 · 스윕 0 | V10 |
| `max-num-batched-tokens` | 2048 | inherited | 계보 문서에 다른 값 · 스윕 기록이 없다 | V11 |
| `reasoning-parser` | solar_open2 | inherited | HF 모델 카드 인자 · 이름은 플러그인 register 가 등록한다(stock 0.31 에 없음) · 뺀 구성 시도 0 | V12 |
| `tool-call-parser` | solar_open2 | inherited | HF 모델 카드 인자 · 플러그인이 등록 · 뺀 구성 시도 0 | V13 |
| `enable-auto-tool-choice` | true | inherited | HF 모델 카드 인자 · 뺀 구성 시도 0 | V14 |
| `logits-processors` | solar_open2_plugin.logits_processor:SolarOpen2TemplateLogitsProcessor | inherited | 모델 카드 인자 · 첫 셀은 v1 구현이라 빼고 기동했고(W2 · 기동 · 장문은 영향 없음) S4 에서 V2 로 다시 넣었다 — 넣으면 러너 --no-async-scheduling 이 필요하다(W7) · spec decode 를 막는다(R7) | V15 |
| `default-chat-template-kwargs` | {"think_render_option":"preserved"} | inherited | 모델 카드 권장값 · W6 교정에 V2 logits processor 와 함께 넣었다 — 실패한 첫 구성은 프로세서도 없었고 kwargs 만 뺀 대조가 없어 단독 필요조건으로 확인되지 않았다(함께 옮긴다) | V16 |
<!-- /FACT:knob_status -->

> 이 절이 답하는 질문: 이 레시피를 다른 환경으로 옮길 때 어떤 노브를 먼저 다시 재야 하고, 어떤 노브는 빼면 깨지는가?

① **네 환경에서 다시 도출할 값(arch-scaled)** — `kv-cache-memory-bytes`(V8) · `gpu-memory-utilization`(V5) · `max-num-seqs`(V10) · `max-model-len`(V6) · `max-num-batched-tokens`(V11) · `tensor-parallel-size`(V1). 도출 절차: 노드당 가중치는 엔진 `Model loading took … GiB` 실측으로, KV 는 엔진 `GPU KV cache size: N tokens` 를 클램프 바이트로 나눈 토큰당 바이트 × 목표 컨텍스트 × 동시 시퀀스로 잡고, 남는 호스트 메모리를 예산 선언으로 확인한다. 이 셀의 클램프는 1M 요청 1개를 1.15배로 겨우 덮는 값이다(V8).

② **필요조건으로 확인된 값(declared-requirement)** — yaml 노브 중에는 **없다**. `default-chat-template-kwargs`(V16)는 W6 교정에 쓰였지만 실패한 첫 구성은 V2 logits processor 도 함께 빠져 있었고 kwargs 만 뺀 대조가 없어 단독 필요조건으로 확인되지 않았다(inherited · 둘을 함께 옮긴다). yaml 노브가 아니라 표에 없는 필요조건: 러너 `--no-async-scheduling`(빼면 V2 프로세서가 `NotImplementedError` 로 기동을 막았다 · W7). **설정하면 깨지는 것**: speculative decoding 설정(번들 소스상 V2 프로세서가 기동 때 `NotImplementedError` 로 거부하고, 프로세서가 없으면 KDA 레이어가 spec 스텝에서 raise 한다 — 실기동 관측은 없음 · R7). **적어도 효과가 없는 것**: yaml 의 `async-scheduling: false`(설정 파서가 버린다 · M1).

③ **조정된 적 없는 승계값(inherited)** — `distributed-executor-backend` ray(V2) · `enable-expert-parallel`(V3) · `moe-backend` marlin(V4 · 0.31 NVFP4 후보 목록에서 카드의 triton 대신 손으로 고른 값 — 다른 후보 시도 0) · `kv-cache-dtype` fp8_e4m3(V7) · `enforce-eager`(V9 · cudagraph 시도 0) · 파서 · auto-tool-choice · logits-processors(V12~V15 · 모델 카드 인자 그대로). 이들은 이 셀에서 바꿔 잰 적이 없다 — 최적이라는 뜻이 아니라 "이 값으로 한 번 떴다" 는 뜻이다. ① 의 값들도 지위는 전부 inherited 다(스윕 0).

## 0.6 재검증 · 라우팅
- 이 자료를 `vllm-recipe-explorer` 인터뷰의 **warm-start 근거로만** 투입하고, 평소대로 plan-gate → 레시피 수렴 → 스모크 게이트를 그대로 밟아라(가속이지 우회 ✗).

> 이 절이 답하는 질문: 다른 환경(다른 HW · 다른 vLLM 버전 · 다른 노드 수)에서 이 지도를 쓸 때 무엇부터, 어떤 순서로 확인해야 하며, 실패하면 어디로 돌리는가?

1. **대상 vLLM 이 Solar-Open2 를 정식 지원하는가** — 확인 방법: 대상 버전 소스(또는 이미지 안 `/workspace/vllm-src`)의 `vllm/model_executor/models/registry.py` 에서 `SolarOpen2ForCausalLM` 을, 파서 등록부에서 `solar_open2` 를 grep(이 셀의 판정 방법 · 02 W1). 분기: 있으면 → 이 런타임 플러그인을 **빼고** 정식 경로로 다시 레시피를 세운다(같은 이름을 플러그인이 다시 등록할 때의 거동은 미확인) · 없고 버전이 0.31.x 면 → 플러그인을 그대로 시도하되 스모크로 중재 · 없고 API 가 다른 버전이면 → 이 번들(01 §1.2 해제 명령으로 소스를 꺼낸다)을 그 버전 API 로 다시 이식해야 한다(0.31 이식 때 파손 6종 · V2 logits processor — 지도는 참고로만).
2. **플러그인이 양 노드에서 무장됐는가** — 확인 방법: master · slave 컨테이너 로그의 `[arm_patch] runtime patch armed → /app/configs/solar31-nv4-1m-tp2_patch.py` 두 줄 · 엔진 `Resolved architecture: SolarOpen2ForCausalLM`. 분기: slave 쪽 줄이 없거나 `No module named 'solar_open2_plugin'` 이면 → 서브의 패치 파일 실재와 `RUNTIME_PATCH_STEM` 전달을 고친다(W3) · 둘 다 있으면 그대로.
3. **스케줄링 · 프로세서 조합이 맞는가** — 확인 방법: 엔진 `non-default args` 줄에 `'async_scheduling': False` 와 `logits_processors` 가 함께 있는지 · spec decode 설정이 없는지. 분기: async 가 빠졌으면 → 러너에 `--no-async-scheduling`(yaml 에 적으면 무효 · M1 · W7) · spec decode 를 켜야 하는 환경이면 → 이 지도 폐기(이식본이 받지 않는다 · R7).
4. **호스트 메모리 예산 선언** — 확인 방법: 기동 전 예산을 선언하고(이 하네스는 원장 `budget_declare`) 입력은 노드당 가중치(엔진 `Model loading took … GiB` 실측 — 이 셀 71.6 GiB) · KV 클램프 · overhead · 노드 MemTotal 이다. 분기: 노드 메모리가 GB10(통합메모리 ~121GiB)과 다르면 → KV · gmu · TP 를 다시 도출(00 §0.5 ①) · 선언 없이 기동하지 않는다.
5. **기동 성공 판정** — health 200 + 추론 1회 + 위 2 의 무장 · 아키텍처 줄 + reasoning 분리(응답의 reasoning 필드가 차고 content 에 사고 서술이 없는지 · W6). 분기: reasoning 이 비면 → `default-chat-template-kwargs` 와 프로세서 적용을 확인(V16).
6. **장문 검증은 증분으로** — 40k 이하 단위로 올리며 각 단계 PASS 와 호스트 가드 무발동을 본다(W5). 분기: 전력 · 열 가드가 사살하면 → 단위를 줄이고 가드는 우회하지 않는다 · 1M 근처가 필요하면 측정 구성 그대로 다시 올린다(02 Q2).

실패 신호별 행선지: 빌드 · 버전 핀 · 정식 지원 여부 → `upstream-version-watch` · 서빙 설정 · KV · 노브 · 런타임 패치 내용 → `vllm-recipe-explorer` · 성능 기대 이하 → `adversarial-benchmark` · 노드 · 토폴로지 · 인터커넥트(서브 합류 · NCCL) → `terraforming_node`.

## 0.7 메타
<!-- FACT:meta -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.31.0/solar-open2-250b-nota-nvfp4/gb10-1g2n-cluster-native/qnvfp4-len1048576-kvfp8` |
| 형식 | `hint-payload/v7` |
| 앵커 | `PROVENANCE.json` — 봉인 때 hint 브랜치 페이로드 커밋에 묶인다 |
| 캠페인 · 셀 · 노드 | 캠페인 밖(`_bootstrap` · output/multi/benchlog/sweep_solar31-nv4-1m-tp2/sweep_index.json bench_knobs.campaign = null · campaign_pins_note '활성 캠페인 없음(_bootstrap)'(측정 시점 ACTIVE 해소 · repeat_axis)) · solar31-nv4-1m-tp2 · cluster |
| 발행 모드 | publication-replay |
| 발행 토픽 | solar31_nv4_1m_tp2 |
| work-manifest | docs/_evidence/solar31_nv4_1m_tp2.work-manifest.json |
| 증거 등급(task_class) | full_benchmark |
| 승인 출처 | cli:--approved-by |
| 승인 시각(UTC) | 2026-10-25T04:00:00Z |
| 승인 발화(전사) | 사용자(발화 요지): 5셀 hint 발행 제안(Qwen FP8·NVFP4 멀티, Solar 멀티, Gemma 싱글, Qwen NVFP4 싱글 mmap)에 '오케이 힌트 태그도 발행해줘' — 태그 push 는 발행 직전 재확인 |
| 현행 full 정의 충족(측정 등급) | 예 · 이 측정의 반복 3 · 정의 출처 `.claude/rules/docs.md:42`(03 §3.5) |
| 독립 사실 검증(발행 전) | 통과 — 검증자 `독립 사실 검증 에이전트`(저작자 `hint 저작 에이전트` 아님) · 주장 334건 · 지적 27건 중 고침 27건 · 열림 0 · 2026-10-25T13:46:18Z |
<!-- /FACT:meta -->

<!-- FACT:missing -->
**결손** — 발행을 막지 않고 기재한다(부재와 실패는 다른 사실이다 · 차단은 양성 검출만).

| 코드 | 뜻 | 이 셀의 사유 |
|---|---|---|
| `HINT_MISSING_CAMPAIGN_INSTANCE` | 캠페인 인스턴스를 읽지 않았다(발행 기록 재생 — purge · 캠페인 밖 측정 · 보존됐으나 재생 모드라 읽지 않음 중 어느 쪽인지는 이 셀의 사유) — 셀 lockset·config·cell.status·journey·grounding 이 이 페이로드에 없다. 값의 지위와 여정 한 줄은 문서(testlog·devlog) 서사에서만 복원된다. | 캠페인 밖 측정 — purge 된 인스턴스가 아니라 이 측정의 인스턴스는 애초에 없다(output/multi/benchlog/sweep_solar31-nv4-1m-tp2/sweep_index.json bench_knobs.campaign = null · campaign_pins_note '활성 캠페인 없음(_bootstrap)'(측정 시점 ACTIVE 해소 · repeat_axis)) · 같은 셀의 앞선 캠페인 인스턴스 campaigns/camp-26102412-multi-sol-q38/cells/solar-open2-nv4-1m-tp2(journey 마지막 campaign_closed) 는 보존돼 있으나 이 측정의 것이 아니라 읽지 않았다 |
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

- 발행 절차의 사정(사실): 이 측정은 캠페인 밖(측정 시점 ACTIVE=_bootstrap)이고, 이 페이로드는 **발행 기록 재생**(publication-replay)이다. 앞선 멀티 캠페인 `camp-26102412-multi-sol-q38` 의 인스턴스는 purge 되지 않고 남아 있으나(journey 마지막 행 `campaign_closed` 2026-10-24T08:00:00Z '인스턴스 보존' · 셀 `solar-open2-nv4-1m-tp2` 의 config.yaml · lockset.json · cell.status.json) 재생 모드는 그것을 읽지 않았다 — 그래서 셀 lockset · config · 캠페인 선언은 실리지 않았고 값의 지위는 문서 서사에서 복원했다(00 결손 `HINT_MISSING_CAMPAIGN_INSTANCE`).
- 계보 읽기 목록 밖 문서(사실): S4 완결 이식(V2 logits processor · reasoning 분리 · 동기 스케줄링 발견)과 그 구성의 needle · lite · READY 수치를 적은 `testlog_26102500_PLE_mmap_재이식_Qwen_FP8멀티_NVFP4싱글_Solar_완결이식_S2_S4_판정` 은 계보 그래프에서 경유(transit · 기반 슬러그 필터 불일치)로만 잡혀 발췌 · 사건 출처로 쓸 수 없었다. 산문에서 그 문서를 가리킨 자리는 모두 "계보 경유 문서 · 읽기 목록 밖" 으로 표시했고, 같은 사실은 가능한 한 계보 안 문서(plan_26102420 정정 기록)와 측정 엔진 로그로 대신 근거를 댔다.
- 의견: 이 셀의 재현 키트의 중심은 빌드가 아니라 런타임 패치 한 파일이다 — 01 §1.2 의 해제 명령으로 소스를 읽고, 네 vLLM 버전에서 정식 지원 여부부터 확인하라(00 §0.6 1번).

## 0.9 이름 꼬리
> 태그 이름의 결정론부(`<vllm>/<model>/<arch>/q·len·kv`)는 도구가 정한다. 꼬리는 이 셀을 **같은 q·len·kv 의 다른 셀과 가르는 노브**를 발행 Agent 가 골라 근거와 함께 적는 자리다(plan_26092908 §4.1 U5). 아래 표는 publish 때 기계가 낸 **후보**이고(강제 ✗), `continue` 가 draft `inputs/tail.json` 을 근거 대조한 뒤 확정 꼬리로 바꾼다. 같은 이름이 이미 있으면 끝에 발행 시각 `-t<YYMMDDHHMM>`(KST)이 붙는다.

<!-- FACT:name_tail -->
**확정 이름** `hint/0.31.0/solar-open2-250b-nota-nvfp4/gb10-1g2n-cluster-native/qnvfp4-len1048576-kvfp8` — 꼬리 0토큰.

_빈 꼬리 — 발행 Agent 가 가르는 노브가 없다고 명시했다(`inputs/tail.json` = `[]`)._
<!-- /FACT:name_tail -->

> 이 절이 답하는 질문: 이 셀을 같은 q·len·kv 의 다른 셀과 가르는 노브는 무엇이며 그것을 이름 꼬리로 어떻게 적는가 — 가르는 노브가 없으면 왜 빈 꼬리인가?

빈 꼬리(`inputs/tail.json` = `[]`)다 — 결정론부(vLLM 0.31.0 · 이 모델 · gb10-1g2n-cluster-native · q·len·kv)가 같은 셀이 코드 브랜치 카탈로그(`HINTS.md` · `hints/index.json`)와 로컬 · 원격 태그에 하나도 없어 가를 상대가 없다. 후보 `plenone` 은 체크포인트에 PLE 가 없다는 모델 사실이라 같은 모델의 어느 셀과도 갈리지 않고, `specoff` 는 이식본이 spec decode 를 받지 않아(02 R7) 이 모델 · 버전에서 고를 수 있는 노브가 아니며, `eager` 는 계보에서 바꿔 본 적 없는 승계값(01 V9)이라 비교할 짝 셀이 없다. 이 셀을 가장 크게 가르는 것은 노브가 아니라 자체 이식 런타임 패치(01 §1.2)이고, 0.31.0 에서 이 모델을 띄우는 모든 셀이 그것을 필요로 하므로 꼬리로 적지 않았다.
