# 00 · hint 지도 — 가장 먼저 읽는 파일

<!-- FACT:header -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.31.0/granite-4.2-8b/gb10-1g1n-main-native/qbf16-len32768-kvauto-t2610170951` |
| 형식 | `hint-payload/v7` · 이름 문법 `v7` |
| 생성(UTC · 주입) | 2026-10-17T00:51:01Z |
| 캠페인 · 셀 · 노드 · 모드 | camp-26101623-baseline · granite42-8b-v031 · main · campaign |
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
| 자격 근거 | `output/single/benchlog/sweep_granite42-8b-v031/level_01/post_health_granite42-8b-v031.json` · `output/single/benchlog/sweep_granite42-8b-v031/level_01/guidellm_granite42-8b-v031.json` |
| 증거 등급(task_class) | full_benchmark |
| 측정 등급(bench_mode) | full |
| 셀 등급(cell_grade) | `full` — 성능 판정 · 부하 곡선 · 데이터셋 출처는 `03-benchmark.md` |
| 성능 판정(measurement.verdict) | `PASS` — 출처 output/single/benchlog/sweep_granite42-8b-v031/verdict.json(output · verdict_rule 출력) verdict |
<!-- /FACT:grade -->

<!-- FACT:lite_summary -->
<!-- /FACT:lite_summary -->

## 0.1 요약
> 이 절이 답하는 질문: 이 셀은 무엇을 서빙했고, 무엇이 결정적이었으며, 수신자는 무엇을 가장 조심해야 하는가?

IBM granite-4.2-8b(bf16 · 컨텍스트 32768 · KV auto · spec off · cudagraph)를 GB10 ×1 · 노드 1대에서 vLLM v0.31.0 stock 소스빌드(빌드 패치 0)로 서빙하고, 같은 상주 서빙에 CNN/DailyMail 실텍스트 GuideLLM full 스윕을 다시 걸어 판정점 동시성 1 decode 12.36 t/s(루브릭 권한 weak PASS · 인증서 없음)를 쟀다.
이번 재측정에서 넘은 벽은 서빙이 아니라 측정 하네스였다 — 상주 서버의 엔진 로그가 전날 · 다른 실행의 트래픽까지 덮던 결함을 실행 범위 캡처와 `/metrics` 실행 차분으로 고쳤고(W5), 그 새 가드가 1차 재측정 레벨 1 첫 런의 prefix cache 재적중을 잡아 스윕별 결정론 행 오프셋으로 다시 쟀다(W6 · R4).
가장 오해하기 쉬운 값은 레벨 1 TTFT 다 — 이 측정의 263.31 ms 는 실행 차분 적중 0 의 값이고 앞선(10-16) 측정의 레벨 1 대표 런 TTFT 는 그 판정 testlog 가 나중에 prefix cache 도움을 받은 값으로 정정한(TTFT 모양 근거 · 카운터 없음) 값이라 나란히 두면 안 되며(M3 · Q1), KV 절대 클램프 25232932864(V4)는 이 셀에서 조정된 적 없는 승계값이다.

## 0.2 유효맥락
<!-- FACT:context -->
| 항목 | 값 | 출처 |
|---|---|---|
| 모델 | granite-4.2-8b | sweep_index.meta(output/single/benchlog/sweep_granite42-8b-v031) |
| HF repo | ibm-granite/granite-4.2-8b | 체크포인트 .git/config remote url(huggingface.co · /app/models/IBM/Granite-4.2/granite-4.2-8b → <manifest.nas_model_path>/IBM/Granite-4.2/granite-4.2-8b) |
| HF revision(체크포인트 git HEAD) | 41a8a2d41c54ef4a71741b3e62604f4caaec9295 | 체크포인트 .git HEAD → refs/heads/main → loose ref(/app/models/IBM/Granite-4.2/granite-4.2-8b → <manifest.nas_model_path>/IBM/Granite-4.2/granite-4.2-8b) · refs/remotes/origin/main 와 일치(받아 온 원격 커밋) — checkout 커밋(작업트리 일치는 미검증) · reflog: 측정 시점 checkout = 지금 HEAD(measured_utc 2026-10-16T22:23:36Z 이전 마지막 이동 2026-08-26T00:28:11Z) |
| base_model | ibm-granite/granite-4.1-8b-base | 체크포인트 README base_model(/app/models/IBM/Granite-4.2/granite-4.2-8b → <manifest.nas_model_path>/IBM/Granite-4.2/granite-4.2-8b) |
| base 슬러그 | granite-4.2-8b | model(granite-4.2-8b) − vocab quant_suffixes(naming.base_slug) |
| 양자화 | bf16 | naming-axis(q) — 측정 identity quantization 미관측(sweep_index.meta(output/single/benchlog/sweep_granite42-8b-v031)) · q 축 = /app/models/IBM/Granite-4.2/granite-4.2-8b → <manifest.nas_model_path>/IBM/Granite-4.2/granite-4.2-8b/config.json quantization_config·hf_quant_config.json 부재 관측 → 비양자화 · vocab quant[bf16]←'none' — 인증서 quantization 이 N/A 라 명명 축 q(체크포인트 config 파생)를 옮겼다 |
| GPU | NVIDIA GB10 | sweep_index.meta(output/single/benchlog/sweep_granite42-8b-v031) |
| 토폴로지 · TP | single · TP=1 | sweep_index.meta(output/single/benchlog/sweep_granite42-8b-v031) |
| 실행 평면 | docker | artifacts.plane_of |
| vLLM(측정 강식별 키 `vllm_version`) | 0.31.0 | sweep_index.meta(output/single/benchlog/sweep_granite42-8b-v031) |

**명명 축** — 태그 이름 = 결정론부(vllm · model · arch · q · len · kv — 도구가 증거에서 파생 · 발행자 입력 ✗) + 꼬리(발행 Agent 가 고르고 서빙 설정 file · key · value 로 근거 대조 · 0.9) + 중복 시 timestamp.

| 축 | 값 | 출처 |
|---|---|---|
| 세그먼트 `vllm` | 0.31.0 | track=선택자 Dockerfile.source-build(docker history build-arg 관측) · ref=docker history build-arg VLLM_REF · version=VLLM_VERSION 미관측 · repo=docker history build-arg VLLM_REPO · sha=output/single/resolved.json upstream_delta.to_sha · VLLM_REF=v0.31.0(업스트림 릴리스 태그 · v 제거) |
| 세그먼트 `model` | granite-4.2-8b | 서빙 yaml model(output/single/configs/granite42-8b-v031.yaml) · hf_repo=체크포인트 .git/config remote url(huggingface.co · /app/models/IBM/Granite-4.2/granite-4.2-8b → <manifest.nas_model_path>/IBM/Granite-4.2/granite-4.2-8b) · hf_repo(ibm-granite/granite-4.2-8b) 마지막 성분 소문자 |
| 세그먼트 `arch` | gb10-1g1n-main-native | derived(hw·gpus_per_node·nodes·role·target·plane) |
| 세그먼트 `recipe` | qbf16-len32768-kvauto-t2610170951 | derived(q·len·kv) + timestamp(이름 중복 · generated_utc KST) |
| 축 `hw` | gb10 | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(캠페인 assignments(배정 SSOT)) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) · vocab hw[gb10]←'NVIDIA GB10' |
| 축 `gpus_per_node` | 1 | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(캠페인 assignments(배정 SSOT)) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `nodes` | 1 | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(캠페인 assignments(배정 SSOT)) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `role` | main | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(캠페인 assignments(배정 SSOT)) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `target` | native | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(캠페인 assignments(배정 SSOT)) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) · target_gpu 'NVIDIA GB10' = 호스트 hw → native |
| 축 `plane` | docker | artifacts.plane_of → cell-env(IMAGE_TAG\|BUILD_DOCKERFILE) · vocab plane[docker]→'' |
| 축 `q` | bf16 | /app/models/IBM/Granite-4.2/granite-4.2-8b → <manifest.nas_model_path>/IBM/Granite-4.2/granite-4.2-8b/config.json quantization_config·hf_quant_config.json 부재 관측 → 비양자화 · vocab quant[bf16]←'none' |
| 축 `len` | 32768 | 서빙 yaml max-model-len(output/single/configs/granite42-8b-v031.yaml) |
| 축 `kv` | auto | 서빙 yaml kv-cache-dtype 부재 관측(output/single/configs/granite42-8b-v031.yaml) → 엔진 기본값 auto · vocab kv[auto]←'auto' |
| timestamp | `t2610170951` | 같은 이름이 이미 있어 붙였다(이 발행의 generated_utc 를 KST 로 · 결정론) |
<!-- /FACT:context -->

> 이 절이 답하는 질문: 이 지도는 어떤 조건에서만 성립하며, 이 형상은 어떤 기전으로 버티고 다른 형상은 왜 실패했는가?

(가) 이 결과가 성립하는 조건

1. **통합메모리 GB10 한 대를 64GB Spark 한 대로 나눠 쓴 상태** → `gpu-memory-utilization` 은 물리 호스트 기준 환산값이고 KV 는 절대 클램프로 고정됐으며, 측정 중에도 같은 노드에 다른 모델(MiniCPM5-2B · 포트 8002)이 상주했다 → 64GB 단독 기기 · 분리 메모리 GPU · 노드를 혼자 쓰는 환경에서는 gmu 의 뜻과 KV 여유가 달라진다. 근거 plan_26101410 §1 · §4(D4) · testlog_26101622_47_43_Q1_prefix_cache_hit_rate_귀속_판정 머리 유효맥락(granite :8001 · minicpm5-2b :8002 · 기동 2일).
2. **오래 상주한 서버 · prefix caching 켜짐(엔진 기본 · 엔진 로그 L48 config `enable_prefix_caching=True`)** → 같은 프롬프트가 캐시에서 밀려나기 전에 다시 오면 prefill 이 적중분을 건너뛰어 TTFT 가 떨어진다(이 계보의 1차 재측정 레벨 1 첫 런: 실행 차분 적중 0.9856 · TTFT 95.2 ms 대 깨끗한 런 262.8 · 263.4 ms — W6) → 측정마다 서버를 새로 띄우거나 앞선 트래픽과 행이 겹치지 않으면 이 교란이 없고, 같은 데이터 행을 되풀이하는 하네스면 HW 와 무관하게 생긴다. 이 측정의 TTFT 는 12 런 모두 실행 차분 적중 0.0 · requests_delta 18 인 값이다. 근거 testlog_26101707_Q6_granite42_8b_v031_CNN_재측정_판정 §1 · §2.
3. **bf16 8B dense 의 decode 가 메모리 대역폭에 묶인다는 루프라인 가정** → 리포트의 R_fp(100% MBU) 15.53 t/s 가 상한이고 측정은 12.36 t/s 다 → 대역폭이 다른 HW(plan_26101410 §3 은 GB10 을 273 GB/s 로 적는다 · 웹 출처)에서는 decode 수치가 그대로 옮겨지지 않는다. 대역폭 묶임은 루프라인 산식의 가정이지 이 셀에서 따로 대조한 기전이 아니다. 근거 bench_report_26101707_granite-4.2-8b_GB10_0.31.0 §루프라인 컨텍스트.
4. **체크포인트가 CIFS 네트워크 파일시스템 위에 있다** → 엔진이 `Filesystem type for checkpoints: CIFS` 를 찍고 가중치 로드에 105.79 s 를 썼다(엔진 로그 `lite_engine_head_granite42-8b-v031.log` L55 · L64) → 로컬 디스크에서는 기동 소요가 달라진다(측정 처리량과는 무관).
5. **입력이 CNN 기사 본문(전처리 1024 토큰 · chat 템플릿 포함 실측 1039) · 출력 256 고정(ignore_eos)** → 합성 입력 셀과는 비교하지 않는다. 이번 스윕의 216 요청도 prompt 1039 · output 256 이다(결과 JSON 관측). 근거 testlog_26101619_CNN_fullbench_셀_granite42_8b_v031_판정 §2 · §4.

(나) 이 형상이 버티는 기전 대 다른 형상

- 1차 재측정(같은 서버 · 같은 프리셋 · 행 0..215 · salt 없음) 대 이 측정(2차 · 행 offset 2727 + 0..215) — 다른 것은 데이터 행의 시작점과 그 직전 서버 캐시에 남아 있던 프롬프트다(하네스 판본도 1차는 ecb7dca 뒤 · 3fa6eac 전으로 다르다 — 커밋 시각과 스윕 시각에서 추론) — 1차 레벨 1 첫 런은 직전 Q1 조사(13:46Z)가 같은 행 0..17 을 보내 GPU KV 블록에 남긴 접두에 적중해 prefill 을 건너뛰었고 decode 는 12.47 대 12.37 · 12.35 로 거의 같았다 · 해제된 KV 블록이 덮이기 전까지 캐시로 남는다는 것은 vLLM prefix caching 의 일반 동작이고 이 계보가 따로 확인하지 않았다 — 적중 사실은 확정(실행 차분 0.9856 대 나머지 11 런 0.0 · 대조군 즉시 재전송 98.6%) — testlog_26101707 §2 · testlog_26101622_47_43 §1 · §5.
- 앞선(10-16) 측정(같은 서버 · 같은 프리셋 · 행 0..215 · 실행 범위 관측 이전 하네스) 대 이 측정 — 앞선 레벨 1 대표 런 TTFT 93.58 ms 가 그 뒤 run 2 · 3 의 262.49 · 260.95 ms 와 갈라지는 모양이 1차 재측정의 재적중과 같다 · 그 판정 testlog 머리의 SUPERSEDED-IN-PART 표기(2026-10-17 추가)가 run 1 을 prefix cache 도움을 받은 값으로 정정하고 원인을 같은 날 앞선 라이브 시도의 같은 행 0..17 전송으로 추정한다(추론) — 정정의 근거는 TTFT 모양(같은 날 다른 셀은 run 간 TTFT 가 고르다)이고 당시 실행 차분 카운터는 없었다 — 적중 판단은 모양 근거 · 원인은 미결 — testlog_26101619 머리 표기 · `docs/simlog/26101620_camp_26101616_cnn_hint__main__granite42_8b_v031/level_01_measured.json` runs.
- 다른 서빙 형상(양자화 · KV dtype · spec · eager · 다른 KV 클램프 · 다른 동시 시퀀스 수) — 이 계보에서 이 모델로 시도한 기록이 없다 — 버티는 기전 대 실패 기전을 가를 대조가 없다 — 미결 — plan_26101410 §4 · testlog_26101619 §1.
- 같은 서빙의 lite 레그(random 512/128 · `vllm bench serve`) — 형상은 같고 입력 텍스트 · 길이 · 측정 도구가 다르다 — 형상 비교가 아니므로 기전을 말하지 않는다(warm 12.58 t/s 와 c=1 12.36 t/s 를 나란히 놓지 않는다) — 미결 — bench_report_26101707 §lite 지표.
- 같은 노드의 MiniCPM5-2B 셀은 다른 모델이라 이 셀의 기전 근거로 쓰지 않는다.

## 0.3 벽 지도 요약
> 02-narrative.md §2.2 의 `kind: wall` 블록(과 §2.4 의 `kind: misdiagnosis` 블록)에서 기계가 생성한 표다(손저작 ✗ · 순서 = 넘은 순서). 원인·해소의 전문과 원문 발췌는 02 에 있다.

<!-- FACT:wall_map -->
| 순서 | id | 증상 | 해소 | 전이등급 | 검증 |
|---|---|---|---|---|---|
| 1 | W1 | P2 오프라인 spike 에서 GuideLLM 데이터 로더 · 매퍼가 문서대로 준 인자로 동작하지 않았다 | 이미지의 실제 스키마로 배선(seed = kind=static · load_kwargs split=train) | arch-invariant | 측정 결과 JSON config 의 data load_kwargs split train 과 seed kind static value 42 기록 |
| 2 | W2 | lite 요청은 전부 성공했는데 lite 판정 measurement_path_failed(KV N/A)로 GuideLLM full 스윕이 시작되지 않았다 | 커밋 0f8800c — lite 파서가 엔진 로그 머리를 먼저 읽고 비면 꼬리로 메운다 · 재빌드와 서빙 재기동 없음 | arch-invariant | 재판정 pass · 머리 로그의 KV 예약 줄(L77-78) · 이번 측정 lite 판정 pass |
| 3 | W3 | granite 동시성 4 의 GuideLLM 라이브 실행 1회가 데이터 열 매핑 인자 해석에서 실패했다 | 열 매핑 인자를 JSON 객체 한 덩이로 넘긴다 | arch-invariant | 측정 결과 JSON config 의 column_mappings 가 세 키를 온전히 기록 · 전 레벨 완료 16 실패 0 |
| 4 | W4 | 열 매핑을 주면 출력 길이 열이 빠져 max_tokens 없이 컨텍스트 끝까지 생성할 수 있음이 이미지 소스 판독에서 드러났다(실패 런 기록 없음) | 실행 시점에 text · prompt_tokens_count · output_tokens_count 세 열을 모두 명시 | arch-invariant | 측정 결과 JSON config 의 세 열 매핑 · 실측 output 256(전 216 요청) |
| 5 | W5 | 상주 서버에서 레벨별 엔진 로그가 그 실행의 요청 외에 전날 · 다른 클라이언트 트래픽을 포함했다(granite 레벨 01 POST 61건 중 이 실행분 18) | ecb7dca — docker logs --since 실행 시작 시각으로 실행 범위 캡처 · /metrics prefix cache 카운터 실행 전후 차분을 prefix_cache_<cfg>.json 과 measured.json prefix_cache_run 으로 기록(기록 전용) | arch-invariant | 레벨 1 대표 run 실행 범위 로그 62줄(POST 18) · c=1 engine_gen_throughput_max 12.5 와 decode 12.36 일치 · 12 런 requests_delta 18 |
| 6 | W6 | 1차 재측정 레벨 1 run 1 의 실행 차분 적중률 0.9856 · 대표 TTFT 95.2 ms(깨끗한 run 262.8 · 263.4 ms) | 1차 스윕 폐기 · 3fa6eac 스윕 시작 UTC salt 기반 결정론 행 시작 오프셋(sidecar sweep_salt · sweep_offset) · 사람 결정으로 재측정 | arch-invariant | 2차 스윕 12 런 모두 hit_rate_run 0.0 · requests_delta 18 · 행 offset 2727 + 0..215 |

**오진 · 정정**(02 §2.4 — 현재지위가 `가설(강등)` 인 기전은 처방이 아니다)

| id | 증상 | 현재지위 | 검증 |
|---|---|---|---|
| M1 | 합격 기준이 실측 입력 토큰을 전처리 목표 길이와 같은 값으로 기대했다 | 반증 | testlog_26101619_CNN_fullbench_셀_granite42_8b_v031_판정 §2 · §4 |
| M2 | CNN 셀 레벨 01 엔진 로그(tail-800)와 그것을 옮긴 종결 testlog 에 prefix cache hit rate 64.5~65.0%(granite 레벨 1 끝)가 찍혔고 행이 겹치지 않는데 높은 이유를 데이터 쪽에서 찾았다 | 반증 | testlog_26101622_47_43_Q1_prefix_cache_hit_rate_귀속_판정 §1 · §2 · 이번 측정 12 런 실행 차분 0.0 |
| M3 | 앞선(10-16) granite CNN 측정의 레벨 1 대표 run 1 TTFT 93.58 ms 가 run 2 · 3(262.49 · 260.95 ms)과 갈라진다 | 반증 | testlog_26101619_CNN_fullbench_셀_granite42_8b_v031_판정 머리 표기 · simlog level_01_measured.json runs 의 TTFT 모양 · 이번 측정 12 런 실행 차분 0.0 |
<!-- /FACT:wall_map -->

## 0.4 결정론 해소값
<!-- FACT:resolved -->
| 항목 | 값 | 출처 |
|---|---|---|
| vLLM 빌드 입력(종류) | release | naming.vllm_build_input |
| vLLM 빌드 입력(ref) | v0.31.0 | docker history build-arg VLLM_REF |
| vLLM 소스 SHA(40자) | db9527a46873454610df6dbedf79a36d6bf1a7f6 | output/single/resolved.json upstream_delta.to_sha |
| 직전 릴리스(prev_release) | 미관측 | 미관측 |
| vLLM 엔진 자기보고(엔진 로그 기동 배너) | 0.31.1.dev0+gdb9527a46.d20261014 | output/single/benchlog/sweep_granite42-8b-v031/lite_engine_head_granite42-8b-v031.log:38(이 측정의 엔진 로그 기동 배너) · 지위 `this-measurement` |
| 인증서 `vllm_version`(강한 일치 키) | 미관측 | 생산자 `absent` · 인증서 vllm_version 없음 |
| wheel 메타(`vllm_build`) | 미관측 | 미관측 — attestation vllm_dist_version · native pip freeze 모두 없다(sweep_index.meta.vllm_build='NA' 는 sweep_bench 가 엔진 로그에서 `v<x.y.z…>` 정규식으로 잡는 값이다 → engine_self_report 의 원천) |
| 체크포인트 revision(HF · git HEAD) | 41a8a2d41c54ef4a71741b3e62604f4caaec9295 | 체크포인트 .git HEAD → refs/heads/main → loose ref(/app/models/IBM/Granite-4.2/granite-4.2-8b → <manifest.nas_model_path>/IBM/Granite-4.2/granite-4.2-8b) · refs/remotes/origin/main 와 일치(받아 온 원격 커밋) — checkout 커밋(작업트리 일치는 미검증) · reflog: 측정 시점 checkout = 지금 HEAD(measured_utc 2026-10-16T22:23:36Z 이전 마지막 이동 2026-08-26T00:28:11Z) |
| 빌드 트랙 | source-build | artifacts 선택자(build-ledger.dockerfile) |
| Dockerfile(선택자) | Dockerfile.source-build | build-ledger.dockerfile |
| vLLM 저장소 | https://github.com/vllm-project/vllm.git | docker history build-arg VLLM_REPO |
| 이미지 태그 | easy-vllm:0.31.0-cu133-aarch64-source | 셀 env(output/single/envs/.env.granite42-8b-v031) IMAGE_TAG |
| 이미지 digest | sha256:339737df6571ce00993e731ef064a8db66085beaa650e4aa63bdb24f414b2034 | sweep_index.meta.image_digest(measured(docker inspect .Image)) |
| torch | 2.13.0a0+9186a08 | docker image inspect Config.Env PYTORCH_VERSION |
| CUDA | 13.3.1.008 | docker image inspect Config.Env CUDA_VERSION |
| NGC 베이스 | nvcr.io/nvidia/pytorch:26.07-py3 | docker image inspect Config.Env NVIDIA_PYTORCH_VERSION = resolved.json ngc_base.image 접두 일치 |
| CPU arch | arm64 | docker image inspect Architecture |
| 드라이버 | 580.173.02 | sweep_index.meta.driver_version(output/single/benchlog/sweep_granite42-8b-v031 · 측정 시 조립 — sweep_bench.sh 가 manifest 에서 읽는다) (manifest 선언 옮김) |
| 드라이버(노드별 관측) | 미관측 | 미관측 |
| ⚠ 드라이버 관측 ≠ 선언 | 미관측 | 미관측 |
| 호스트 OS | 미관측 | 미관측 |

> 재현 좌표는 빌드 입력(릴리스 태그 또는 40자 SHA)이다 — 엔진 자기보고 · 인증서 · wheel 의 버전 문자열은 빌드 입력이 아니다(각 값을 만든 생산자는 출처 칸).

**양자화 구성**(체크포인트 config 관측 · 이름의 `q` 축은 선언 방식 하나 — ModelOpt MIXED 는 층 개수 우세가 보조 규칙)

| 범위 | dtype | 출처 |
|---|---|---|
| 기본 dtype(비양자화 가중치·계산) | `bfloat16` | /app/models/IBM/Granite-4.2/granite-4.2-8b → <manifest.nas_model_path>/IBM/Granite-4.2/granite-4.2-8b/config.json torch_dtype |

**이 모델에 필요한 패치**: 실린 빌드 패치 없음(또는 파일 기록 미관측 — 01 §1.1).
<!-- /FACT:resolved -->

> 이 절이 답하는 질문: 위 값들 중 무엇을 그대로 참조하고 무엇을 네 환경에서 다시 도출해야 하며, 이 빌드 트랙은 왜 골랐는가?

(1) **빌드 트랙** — source-build 다. 근거는 plan_26101410 §3 의 트랙 행 "source-build(torch ≥2.11 ABI 벽) · wheel 은 `manylinux_2_28_aarch64`(cuXXX 없음)" 이다. 다른 트랙으로 갈 수 있는 조건도 그 문장에서만 읽는다 — 대상 vLLM 의 torch 핀이 그 ABI 벽에 걸리지 않는 조합이면 트랙을 다시 판정해야 하고, 판정은 `upstream-version-watch` 의 해소 스크립트가 한다(여기서 조건을 따로 떼어 단정하지 않는다).
(2) **vLLM 버전 문자열** — 재현 좌표는 빌드 입력 ref `v0.31.0`(docker history build-arg `VLLM_REF` · 관측) 또는 40자 SHA `db9527a46873454610df6dbedf79a36d6bf1a7f6`(resolved.json `upstream_delta.to_sha` · 선언)이다. 엔진 기동 배너 `0.31.1.dev0+gdb9527a46.d20261014` 는 엔진 자신이 찍은 자기보고이고 빌드 입력이 아니다 — 배너의 `gdb9527a46` 이 위 SHA 의 앞부분과 같다는 것은 관측이고, `.dev0` 꼴이 소스 트리의 버전 산정 방식에서 나왔다는 것은 추론이다. 인증서 강한 키 `vllm_version` 은 인증서가 없어(루브릭 권한 weak) 미관측이다. 리포트 · 스윕 색인의 `0.31.0` 은 측정 메타의 값이고 그 생산자는 이 페이로드가 특정하지 않았다 — 재현에는 쓰지 말고 ref 또는 40자 SHA 를 쓴다.
(3) **arch-locked 값** — `TORCH_CUDA_ARCH=12.1a`(build-arg · GB10 sm_121 계열): 네 GPU 의 compute capability 로 다시 정한다. CPU arch `arm64`(image inspect): 네 호스트 아키텍처의 NGC 베이스를 고른다. 드라이버 `580.173.02` 는 manifest 선언을 옮긴 값이고 노드 관측이 아니다: 네 노드에서 `nvidia-smi` 로 직접 읽는다. NGC 베이스 `26.07-py3` 는 torch 2.13 계열과 묶여 있다(plan_26101410 §3 — 26.08/26.09 는 torch 2.14): vLLM 핀을 바꾸면 베이스도 다시 해소한다.
(4) **선언 대 관측** — torch `2.13.0a0+9186a08`(이미지 env 관측)은 plan_26101410 §3 의 NGC `26.07-py3` 해소값과 같다. 다른 곳은 빌드 레시피다 — 실린 `Dockerfile.source-build` 는 측정 이미지 docker history 와 16/17 만 맞고 그 파일의 마지막 커밋이 이미지 Created 뒤다(01 §1.1 경고). 서빙 이미지의 실제는 이미지(digest 0.4 표)다. 어긋나는 한 줄은 torch 핀 가드 RUN 이다(docker history 관측): 이미지에는 `source-build key (26.07-py3 x vLLM 0.31.0) is UNVALIDATED — attempt-build under HITL override (--allow-unvalidated).` 가, 실린 worktree 레시피에는 `[guard] source-build key (26.07-py3 x vLLM 0.31.0): validated patch set …` 가 있다. 이미지는 plan_26101410 §4 D1 의 `--allow-unvalidated` 시도-빌드로 지었고 그 뒤 레시피가 이 키를 검증 키로 바꿔 렌더가 달라졌다(커밋 순서에서 읽은 추론).
(5) **환경 좌표** — 드라이버 580.173.02 · 호스트 CUDA 13.2 는 plan_26101410 머리 유효맥락에 적혀 있다(호스트 층). 리포트의 `cuda_version 132` 도 호스트 값이다. 이미지 층은 CUDA `13.3.1.008` · torch 2.13.0a0 이다(섞지 않는다). 노드 메모리 총량은 블랙박스 원장 `budget_declare` 의 `mem_total_mib=124608`(원장 기록 · 01 §1.4)이고 엔진은 가중치 로드 때 `Available RAM: 96.72 GiB` 를 찍었다(엔진 로그 L55). 호스트 OS 는 이 계보에 미기록이다(GuideLLM 결과 JSON 의 platform 칸이 커널 `7.0.0-1019-nvidia` aarch64 를 적지만 배포판은 없다). 인터커넥트는 단일 노드라 해당 없음.
(6) **핵심 의존 핀** — `flashinfer-python==0.7.0.post1` 등 requirements.txt 는 v0.31.0 wheel 의 Requires-Dist 를 옮긴 것이라 vLLM 의 요구이지 이 모델의 요구가 아니다(파일 머리 주석 L1-L3). `RAY_VERSION=2.48.0` 은 공유 이미지 build-arg 이고 TP=1 단일 노드인 이 셀은 Ray 를 쓰지 않는다(추론 — 엔진 로그는 world_size=1 · L49). 엔진은 `FLASH_ATTN` 백엔드(FlashAttention 2)를 스스로 골랐다(엔진 로그 L53-54). 이 모델 전용으로 추가한 의존은 없다.

## 0.5 서빙 노브와 값의 지위
> 01-artifacts.md §1.3 의 `kind: value-status` 블록에서 기계가 생성한 요약이다. 지위가 `tuned` · `declared-requirement` 가 아닌 값은 **이 셀에서 조정된 적도, 빼면 깨진다고 확인된 적도 없다** — 필요조건으로 복사하지 마라. `declared-requirement` 는 빼거나 바꾸면 실패가 관측된 값이다(근거의 W id · artifacts 헤더를 보고 네 환경에서도 그 실패 조건이 성립하는지 판단한다).

<!-- FACT:knob_status -->
노브 4개 — `inherited` 4.

| 노브 | 값 | 지위 | 근거 | id |
|---|---|---|---|---|
| `gpu-memory-utilization` | 0.39 | inherited | 앞선 계획이 64GB Spark 시뮬 천장을 물리 호스트 기준으로 환산해 둔 값(천장÷호스트 총량 약 0.39 · 두 인스턴스 공존용)이고 이 계보에서 바꿔 잰 기록이 없다 · KV 절대 클램프가 있으면 엔진은 이 값을 KV 산정에 쓰지 않는다고 로그에 적는다 | V1 |
| `max-model-len` | 32768 | inherited | 사용자가 4종 상주 서빙의 요구로 32768 고정을 지시했다 · 이 계보에서 다른 값을 시도한 기록이 없다 | V2 |
| `max-num-seqs` | 8 | inherited | 계획 D5 가 정한 산식 꼴 min(8 · KV_fit@8192) 의 값과 같지만 실행에서 explorer 가 이 산식으로 계산했다는 기록은 계보에 없다(계보 표 밖 devlog_26101412 · testlog_26101412 는 KV 와 gmu 의 hand 출처만 적고 max-num-seqs 의 산정 기록은 없다) · 이 셀에서 바꿔 잰 기록 없음 · 스윕 레벨 16 은 이 값 때문에 자동 제외됐다 | V3 |
| `kv-cache-memory-bytes` | 25232932864 | inherited | 앞선 계획은 절대 클램프라는 방식만 정했고 값은 앞선 캠페인 실행에서 손으로 산정됐다(천장 − 가중치 − 오버헤드의 max_safe 를 사용자 64/64 의도로 min(required, max_safe) 대신 택함 · lockset kv_source=hand — 계보 표 밖 devlog_26101412 가 기록한다) · 엔진 예약 23.5 GiB · 154000 토큰 · 32768 토큰 요청 기준 최대 동시성 4.70x(엔진 로그 · 클램프의 되읽기) · 이 셀에서 조정 기록 없음 | V4 |
<!-- /FACT:knob_status -->

> 이 절이 답하는 질문: 이 레시피를 다른 환경으로 옮길 때 어떤 노브를 먼저 다시 재야 하고, 어떤 노브는 빼면 깨지는가?

① **네 환경에서 반드시 다시 도출할 값(arch-scaled)** — `kv-cache-memory-bytes`(V4) · `gpu-memory-utilization`(V1) · `max-num-seqs`(V3). KV 는 엔진 로그의 `GPU KV cache size: <토큰>` 줄과 plan 의 토큰당 KV(160KiB)로 목표 컨텍스트 × 동시 시퀀스를 담는지 확인해 다시 정하고, gmu 는 네 호스트 총량 기준으로 다시 환산한다(이 셀의 0.39 는 128GB급 노드를 64GB 둘로 나눈 환산이다). `max-num-seqs` 는 네 KV 예산에서 대표 요청 길이로 다시 산출한다(plan_26101410 §4 D5 의 산식 꼴). `max-model-len`(V2)은 사용자 요구로 고정한 값이라 네 요구로 다시 정한다.
② **필요조건으로 확인된 값(declared-requirement)** — 없다. 이 계보에서 어떤 서빙 노브도 빼거나 바꿔서 실패가 관측된 적이 없다. 명시하면 깨지는 노브도 관측되지 않았다. 다만 KV 절대 클램프를 주면 엔진이 gmu 를 KV 산정에 쓰지 않는다는 것은 엔진 로그가 적은 거동이다(V1 근거 · 엔진 로그 L77).
③ **승계값(inherited)** — 네 노브 전부다. gmu · KV · max-num-seqs 는 plan_26101410 의 64GB Spark 시뮬 · 같은 노드 2모델 공존 결정(D4 · D5)을 앞선 캠페인이 손으로 값으로 옮긴 것이고(KV 는 kv_source=hand · 계보 표 밖 devlog_26101412), 이 셀(CNN 재측정 캠페인)은 서빙을 다시 띄우지도 값을 바꾸지도 않았다 — 바뀐 것은 측정 하네스와 데이터 행뿐이다. 최적값이라는 뜻이 아니다 — 수렴 기록은 이 계보 밖이다.

## 0.6 재검증 · 라우팅
- 이 자료를 `vllm-recipe-explorer` 인터뷰의 **warm-start 근거로만** 투입하고, 평소대로 plan-gate → 레시피 수렴 → 스모크 게이트를 그대로 밟아라(가속이지 우회 ✗).

> 이 절이 답하는 질문: 다른 환경(다른 HW · 다른 vLLM 버전 · 다른 노드 수)에서 이 지도를 쓸 때 무엇부터, 어떤 순서로 확인해야 하며, 실패하면 어디로 돌리는가?

1. **빌드 · 버전 핀** → 네 vLLM 에서 이 셀이 stock 이었다는 전제가 성립하는가: `Dockerfile.source-build` 빌드 로그에서 `[patch] strip-hoist` 줄과 `use_existing_torch.py` 단계를 본다 → strip-hoist 가 `skipped` 또는 `applied` 면 자기적응대로 그대로 · `use_existing_torch.py 를 찾지 못했다` 로 빌드가 멈추면 상류 경로 이동이므로 재도출(`upstream-version-watch`). 이 셀은 빌드 패치 · 런타임 패치 · 포크 핀이 0 이라 "자체 패치가 아직 필요한가" 물음은 해당 없다(01 §1.1).
2. **기동 성공 판정** → health 200 + chat 추론 1회 + 엔진 로그 `reserved <N> GiB memory for KV Cache as specified by kv_cache_memory_bytes` 줄로 KV 클램프가 먹었는지 확인한다 → 줄이 없거나 다른 값이면 KV 노브 재도출(`vllm-recipe-explorer`). 패치가 없으니 패치 발화 서명은 없다. 툴콜 파서(`qwen3_coder`)와 추론 파서(`granite_thinking_parser`)의 스모크 기록은 계보 표 밖(testlog_26101412)에 있다 — 네 환경에서도 툴콜 1회를 직접 쳐서 `tool_calls` 로 나오는지 본다.
3. **호스트 메모리 안전 예산** → 기동 전에 예산을 선언한다. 이 셀의 선언 입력은 가중치 16768 MiB · KV 24064 MiB · overhead 7600 MiB · 총량 124608 MiB 였다(01 §1.4 원장 · 선언값) → 같은 노드에 둘째 모델을 띄우면 선언 파일이 노드당 1개라 둘째 선언이 첫째를 덮는다(plan_26101410 §3 · §4 D6) — 합산 선언으로 다시 세운다(`terraforming_node` 블랙박스 절차).
4. **측정 경로 — 엔진 로그 범위** → 상주 서버를 재는 하네스가 엔진 로그를 실행 시작 시각부터만 담는지(`docker logs --since`) 확인한다(W5) → `tail -N` 꼴이면 그 로그의 throughput · hit rate 를 이 실행의 값으로 읽지 않는다(재측정이 아니라 하네스 교정). lite 파서가 기동 KV 줄을 엔진 로그 머리에서 읽는지도 본다(W2).
5. **측정 경로 — prefix cache 재적중** → 런마다 `prefix_cache_<cfg>.json`(`/metrics` 실행 차분)의 `hit_rate_run` 이 0 이고 `requests_delta` 가 그 런의 요청 수와 같은지 본다(W6) → 첫 런만 적중이 높고 TTFT 가 다른 런보다 크게 낮으면 앞선 트래픽이 같은 행을 보낸 것이다 — 그 스윕을 버리고 행 시작점을 바꿔 다시 잰다(R4 · 서버 쪽 `/reset_prefix_cache` 는 DEV_MODE 가 아니면 404 · R5).
6. **데이터셋 입력** → `cnn_preset.yaml` 은 비추적 사용자 프리셋이라 이 zip 에 없다 — `config/guide-llm/README.md` 절차로 다시 쓰고 원본 sha256 이 03 데이터셋 출처 표와 같은지 본다 → 다르면 이 지도의 수치와 비교 불가. 토크나이저가 BOS 를 세면 목표 길이가 1024 가 아니다(R2). 같은 데이터셋 · 형상으로 c=1 을 재고 03 §3.5 의 비교 규칙으로만 나란히 둔다 → 기대 이하면 `adversarial-benchmark`.

실패 신호별 행선지:
- 빌드 · 버전 핀 문제 → `upstream-version-watch`
- 서빙 설정 · KV · 노브 문제 → `vllm-recipe-explorer`
- 성능 기대 이하 · 측정 하네스 교란(엔진 로그 범위 · prefix cache 재적중) → `adversarial-benchmark`
- 노드 · 토폴로지 · 인터커넥트 · 호스트 예산 문제 → `terraforming_node`

## 0.7 메타
<!-- FACT:meta -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.31.0/granite-4.2-8b/gb10-1g1n-main-native/qbf16-len32768-kvauto-t2610170951` |
| 형식 | `hint-payload/v7` |
| 앵커 | `PROVENANCE.json` — 봉인 때 hint 브랜치 페이로드 커밋에 묶인다 |
| 캠페인 · 셀 · 노드 | camp-26101623-baseline · granite42-8b-v031 · main |
| 발행 모드 | campaign |
| 발행 토픽 | camp_26101623_baseline__main__granite42_8b_v031 |
| work-manifest | docs/_evidence/camp_26101623_baseline__main__granite42_8b_v031.work-manifest.json |
| 증거 등급(task_class) | full_benchmark |
| 승인 출처 | campaign:hint_targets |
| 승인 시각(UTC) | 2026-10-16T14:03:00Z |
| 승인 발화(전사) | 사용자(캠페인 개설 팝업 발화 전사) — "열기 · 2셀 발행 승인 (Recommended)" |
| 현행 full 정의 충족(측정 등급) | 예 · 이 측정의 반복 3 · 정의 출처 `.claude/rules/docs.md:42`(03 §3.5) |
| 독립 사실 검증(발행 전) | 통과 — 검증자 `메인 검증 Agent(granite 독립 검증 하위 에이전트)`(저작자 `메인 저작 Agent(granite 저작 하위 에이전트)` 아님) · 주장 118건 · 지적 10건 중 고침 10건 · 열림 0 · 2026-10-17T01:14:24Z |
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

**발행 경위(사실)** — 이 페이로드는 같은 셀(같은 서빙 · 같은 트리플렛 바이트 · 같은 결정론부 이름)의 두 번째 판이다. 앞선 판은 2026-10-16 CNN 캠페인(camp-26101616-cnn-hint)의 측정으로 발행됐고, 이 판은 plan_26101622 의 검증 캠페인(camp-26101623-baseline)이 측정 하네스 교정(ecb7dca 실행 범위 관측 · 3fa6eac 스윕별 행 오프셋) 뒤 같은 서빙을 다시 잰 것이다. 서빙 노브가 같아 이름 꼬리는 비우고, 같은 이름이 이미 있으므로 continue 가 발행 시각 접미를 붙인다. 앞선 판은 고치거나 회수하지 않는다(리콜 금지).
**앞선 판과의 관계(사실)** — 앞선 판의 열린 물음 Q1 은 "최종 스윕 전 같은 서빙에 들어간 요청 150건과 prefix cache 가 이 측정의 TTFT 에 영향을 줬는가" 였고 현재상태에 앞선(끊긴) 스윕이 같은 행을 썼는지와 hit rate 집계 창을 미확인으로 적었다. 이 판은 그 물음을 **부분 해소**한다 — 집계 창 부분은 M2(로그 값은 최근 요청 창 통계 · 이 셀 요청에 귀속되지 않음)가 답하고, TTFT 영향 부분은 앞선 판정 testlog 의 SUPERSEDED-IN-PART 표기(run 1 은 prefix cache 도움 · TTFT 모양 근거 · 원인 추정)와 이 판의 M3 · Q1(원인 미결)로 남는다.
**의견** — 두 판의 decode 는 레벨마다 1% 안에서 맞지만(03 §3.5) 레벨 1 TTFT 는 약 2.8배 다르다. 수신자는 TTFT 기준점으로 이 판을 쓰는 편이 안전하다고 본다 — 이 판만 실행 차분으로 캐시 적중 0 이 관측됐다.

## 0.9 이름 꼬리
> 태그 이름의 결정론부(`<vllm>/<model>/<arch>/q·len·kv`)는 도구가 정한다. 꼬리는 이 셀을 **같은 q·len·kv 의 다른 셀과 가르는 노브**를 발행 Agent 가 골라 근거와 함께 적는 자리다(plan_26092908 §4.1 U5). 아래 표는 publish 때 기계가 낸 **후보**이고(강제 ✗), `continue` 가 draft `inputs/tail.json` 을 근거 대조한 뒤 확정 꼬리로 바꾼다. 같은 이름이 이미 있으면 끝에 발행 시각 `-t<YYMMDDHHMM>`(KST)이 붙는다.

<!-- FACT:name_tail -->
**확정 이름** `hint/0.31.0/granite-4.2-8b/gb10-1g1n-main-native/qbf16-len32768-kvauto-t2610170951` — 꼬리 0토큰 · timestamp `t2610170951`.

_빈 꼬리 — 발행 Agent 가 가르는 노브가 없다고 명시했다(`inputs/tail.json` = `[]`)._

- `t2610170951` = 같은 이름(결정론부 + 꼬리)이 원격 · 로컬에 이미 있어 붙인 발행 시각(generated_utc 의 KST `YYMMDDHHMM`) — 같은 셀의 새 판이다(옛 태그는 교정하지 않는다).
<!-- /FACT:name_tail -->

> 이 절이 답하는 질문: 이 셀을 같은 q·len·kv 의 다른 셀과 가르는 노브는 무엇이며 그것을 이름 꼬리로 어떻게 적는가 — 가르는 노브가 없으면 왜 빈 꼬리인가?

빈 꼬리(`[]`)다. 결정론부가 같은 기존 태그는 같은 셀의 10-16 CNN 측정분 하나이고, 두 측정의 서빙 yaml · 러너 · 셀 env 바이트가 같다 — **서빙 노브 차이는 없다**. 두 측정은 측정 하네스 판본(실행 범위 엔진 로그 · `/metrics` 실행 차분 · 스윕별 행 오프셋) · 데이터 행 시작점 · 반복 수의 출처(defaulted 대 declared · 값 3 같음) · 측정 시각과 그 직전 서버 상태(관측 요청 수 · 캐시 내용 — 레벨 1 TTFT 차이의 후보)에서 다르지만 어느 것도 서빙 노브가 아니라서 꼬리로 적지 않는다(같은 이름의 새 판은 continue 가 발행 시각 접미로 가른다).
후보 `plenone`(이 모델은 PLE 가 없어 구분력이 없다) · `specoff` · `graph`(둘 다 설정하지 않은 엔진 기본이고 두 측정에서 같다)는 쓰지 않았다. 데이터셋(CNN/DailyMail)도 이름이 아니라 03 에만 둔다는 결정을 따른다(plan_26101614 §2 U9).
