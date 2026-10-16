# 00 · hint 지도 — 가장 먼저 읽는 파일

<!-- FACT:header -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.31.0/lfm2.5-2.6b/gb10-1g1n-sub-native/qbf16-len32768-kvauto-t2610162309` |
| 형식 | `hint-payload/v7` · 이름 문법 `v7` |
| 생성(UTC · 주입) | 2026-10-16T14:09:45Z |
| 캠페인 · 셀 · 노드 · 모드 | camp-26101623-baseline · lfm25-26b-v031 · sub · campaign |
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
| 자격 근거 | `output/single/benchlog/lite_publish/lfm25-26b-v031_20261016T140827Z/lite_raw_lfm25-26b-v031.json` · `output/single/benchlog/lite_publish/lfm25-26b-v031_20261016T140827Z/lite_warm_lfm25-26b-v031.json` |
| 증거 등급(task_class) | hint_map_only |
| 측정 등급(bench_mode) | lite |
| 셀 등급(cell_grade) | `lite(기동 가능)` — lite 벤치만 통과한 셀(이 HW 에서 기동 · 추론 최소조건 증명 · **성능 판정 아님**) · `03-benchmark.md` 미발행 |
| 성능 판정(measurement.verdict) | `lite(기동 가능)` — **성능 판정 아님**(full 판정 PASS · REFUTE 와 분리 · 기계 판정 칸 `OBSERVATION-ONLY`) · 출처 lite-only 셀 — verdict_rule 에 투입되지 않았다(경량 리포트 `inform-only` · 판정 원천 없음 · benchmark_mode=lite · report_kind=lite) |

> **lite(기동 가능)** · **OBSERVATION-ONLY** — 이 셀은 lite 벤치(서빙 직후 1회 스냅샷)만 통과했다: 이 HW 에서 기동 · 추론이 된다는 **최소조건 증명이지 성능 판정이 아니다**(full 판정 PASS · REFUTE 와 분리). 수치는 관측 게재이며 baseline · 권고가 아니다 — `03-benchmark.md`(부하 곡선 · like-with-like)는 싣지 않는다. baseline 을 주장하려면 full 측정이 필요하다.
<!-- /FACT:grade -->

<!-- FACT:lite_summary -->
### lite 측정 — `lite(기동 가능)` · 성능 판정 아님

> lite 셀은 `03-benchmark.md` 를 싣지 않는다 — lite 벤치 1회의 수치 · 측정 구성 · 측정 도구 원문을 여기 둔다(파싱만 · 재계산 ✗). 이 HW 에서 기동 · 추론이 된다는 최소 증명이며 baseline · 권고가 아니다.

| 키 | 값 | 출처 |
|---|---|---|
| `measured_utc` | 2026-10-16T14:08:27Z | certificate(benchmark_26101623_lfm2.5-2.6b_GB10_0.31.0.yaml) measured_utc |
| `accept_len` | 미기재 | lite-only 셀 — 동시성 1 스윕·루브릭이 없다(lite 수치는 `lite`) |
| `bench_tool` | vllm-bench-serve | certificate(benchmark_26101623_lfm2.5-2.6b_GB10_0.31.0.yaml) bench_tool |
| `benchmark_mode` | lite | certificate(benchmark_26101623_lfm2.5-2.6b_GB10_0.31.0.yaml) benchmark_mode |
| `decode_tps_conc1` | 미기재 | lite-only 셀 — 동시성 1 스윕·루브릭이 없다(lite 수치는 `lite`) |
| `floor_tps` | 미기재 | lite-only 셀 — 동시성 1 스윕·루브릭이 없다(lite 수치는 `lite`) |
| `lite_accept_len` | N/A | certificate(benchmark_26101623_lfm2.5-2.6b_GB10_0.31.0.yaml) lite_accept_len |
| `lite_cold_ttft_ms` | 38.9 | certificate(benchmark_26101623_lfm2.5-2.6b_GB10_0.31.0.yaml) lite_cold_ttft_ms |
| `lite_gen_src` | median_tpot | certificate(benchmark_26101623_lfm2.5-2.6b_GB10_0.31.0.yaml) lite_gen_src |
| `lite_gen_tps_warm` | 36.25 | certificate(benchmark_26101623_lfm2.5-2.6b_GB10_0.31.0.yaml) lite_gen_tps_warm |
| `lite_included` | true | certificate(benchmark_26101623_lfm2.5-2.6b_GB10_0.31.0.yaml) lite_included |
| `lite_kv_gib` | 37.9 | certificate(benchmark_26101623_lfm2.5-2.6b_GB10_0.31.0.yaml) lite_kv_gib — 서빙 yaml `kv-cache-memory-bytes: 40694815129`(37.9 GiB)의 **선언 클램프를 엔진이 되읽은 값**(측정 아님 · output/single/configs/lfm25-26b-v031.yaml) |
| `ratio_M_over_primary` | 미기재 | lite-only 셀 — 동시성 1 스윕·루브릭이 없다(lite 수치는 `lite`) |
| `rubric_authority` | 미기재 | lite-only 셀 — 동시성 1 스윕·루브릭이 없다(lite 수치는 `lite`) |
| `verdict` | OBSERVATION-ONLY | lite-only 셀 — verdict_rule 에 투입되지 않았다(경량 리포트 `inform-only` · 판정 원천 없음 · benchmark_mode=lite · report_kind=lite) |
| `source` | certificate(benchmark_26101623_lfm2.5-2.6b_GB10_0.31.0.yaml) | — |

**측정 구성**

| 키 | 값 |
|---|---|
| `bench_mode` | lite |
| `bench_mode_kind` | declared-lite |
| `downgrade_reason` | 미기재 |
| `bench_tool` | vllm-bench-serve |
| `bench_tool_version` | 미기재 |
| `repeats` | 1 |
| `repeats_completed` | 1 |
| `source` | bench_report(bench_report_26101623_lfm2.5-2.6b_GB10_0.31.0.md) |

**측정 도구 원문**

**측정 도구 원문 1건**(측정 시각 이전 마지막 커밋의 바이트 · 발췌 머리 = `> [원문] <이름>@<rev12> §L<a>-<b>` · 다음 변경 = 측정 뒤 이 파일을 처음 바꾼 커밋 — 두 측정 사이에 도구가 바뀌었는지는 이 칸으로 가른다)

| 도구 | 역할 | 저장소 경로 | 리비전(커밋 UTC) | 다음 변경 | 발췌 출처 토큰 | 다시 얻기 |
|---|---|---|---|---|---|---|
| `lite_bench.sh` | lite | `.claude/skills/adversarial-benchmark/scripts/lite_bench.sh` | `e2b67d3320fd` 2026-10-10T04:54:37Z | 5dc9e2275380..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `lite_bench.sh@e2b67d3320fd` | `git show e2b67d3320fd68531d9c6bc717aecfb3961239a9:.claude/skills/adversarial-benchmark/scripts/lite_bench.sh` |

- `lite_bench.sh` — 경량 리포트 셀(lite_bench.sh 가 리포트를 발행한다)
- `다시 얻기` = 그 커밋을 가진 클론에서의 명령이다(발행 원격 도달은 이 표가 판정하지 않았다).
- 측정 시각의 워킹트리(미커밋 편집)는 관측 대상 밖 — 이 바이트는 그때의 **커밋된** 판본이다
<!-- /FACT:lite_summary -->

## 0.1 요약
> 이 절이 답하는 질문: 이 셀은 무엇을 서빙했고, 무엇이 결정적이었으며, 수신자는 무엇을 가장 조심해야 하는가?

이 셀은 LFM2.5-2.6B(bf16, max-model-len 32768)를 NVIDIA GB10 1장 단일노드(TP=1)에서 vLLM v0.31.0 소스빌드 이미지로 서빙하고 lite 벤치 1회만 통과한 기록이며, warm 생성 36.25 t/s · cold TTFT 38.9 ms 는 성능 판정이 아닌 관측값이다. 이 태그의 계보에서 확인된 벽은 측정·발행 도구 쪽 다섯이다 — CNN 프리셋 쓰기 거부(W1)·sweep 인자 누락(W2)·이미지에 빌드 원장 스탠자가 없음(W3)·정본 렌더 호출이 서브에서 성립하지 않음(W4)·arch 값이 null 인데 렌더가 None 을 씀(W5)이고 서빙 단계의 벽은 계보 밖이다. 서빙 노브는 전부 승계값(V1~V4)이며 빌드 원장이 이미지에 없어 패치 적용 판정은 재구성(unobservable)이다.

## 0.2 유효맥락
<!-- FACT:context -->
| 항목 | 값 | 출처 |
|---|---|---|
| 모델 | lfm2.5-2.6b | 인증서(benchmark_26101623_lfm2.5-2.6b_GB10_0.31.0.yaml) |
| HF repo | LiquidAI/LFM2.5-2.6B | 체크포인트 .git/config remote url(huggingface.co · /app/models/Liquid-AI/LFM2.5-2.6B → <manifest.nas_model_path>/Liquid-AI/LFM2.5-2.6B) |
| HF revision(체크포인트 git HEAD) | ab00687315bc1298e9d54e9c4b611dde9867ccc2 | 체크포인트 .git HEAD → refs/heads/main → loose ref(/app/models/Liquid-AI/LFM2.5-2.6B → <manifest.nas_model_path>/Liquid-AI/LFM2.5-2.6B) · refs/remotes/origin/main 와 일치(받아 온 원격 커밋) — checkout 커밋(작업트리 일치는 미검증) · reflog: 측정 시점 checkout = 지금 HEAD(measured_utc 2026-10-16T14:08:27Z 이전 마지막 이동 2026-08-11T01:46:56Z) |
| base_model | LiquidAI/LFM2.5-2.6B-Base | 체크포인트 README base_model(/app/models/Liquid-AI/LFM2.5-2.6B → <manifest.nas_model_path>/Liquid-AI/LFM2.5-2.6B) |
| base 슬러그 | lfm2.5-2.6b | model(lfm2.5-2.6b) − vocab quant_suffixes(naming.base_slug) |
| 양자화 | bf16 | naming-axis(q) — 측정 identity quantization 미관측(인증서(benchmark_26101623_lfm2.5-2.6b_GB10_0.31.0.yaml)) · q 축 = /app/models/Liquid-AI/LFM2.5-2.6B → <manifest.nas_model_path>/Liquid-AI/LFM2.5-2.6B/config.json quantization_config·hf_quant_config.json 부재 관측 → 비양자화 · vocab quant[bf16]←'none' — 인증서 quantization 이 N/A 라 명명 축 q(체크포인트 config 파생)를 옮겼다 |
| GPU | NVIDIA GB10 | 인증서(benchmark_26101623_lfm2.5-2.6b_GB10_0.31.0.yaml) |
| 토폴로지 · TP | single · TP=1 | 인증서(benchmark_26101623_lfm2.5-2.6b_GB10_0.31.0.yaml) |
| 실행 평면 | docker | artifacts.plane_of |
| vLLM(측정 강식별 키 `vllm_version`) | 0.31.0 | 인증서(benchmark_26101623_lfm2.5-2.6b_GB10_0.31.0.yaml) · 생산자 `lite-report-identity`(0.4 표 `인증서 vllm_version` 행 — 엔진 자기보고는 그 표의 다른 행) |

**명명 축** — 태그 이름 = 결정론부(vllm · model · arch · q · len · kv — 도구가 증거에서 파생 · 발행자 입력 ✗) + 꼬리(발행 Agent 가 고르고 서빙 설정 file · key · value 로 근거 대조 · 0.9) + 중복 시 timestamp.

| 축 | 값 | 출처 |
|---|---|---|
| 세그먼트 `vllm` | 0.31.0 | track=선택자 Dockerfile.source-build(docker history build-arg 관측) · ref=docker history build-arg VLLM_REF · version=VLLM_VERSION 미관측 · repo=docker history build-arg VLLM_REPO · sha=미관측 · VLLM_REF=v0.31.0(업스트림 릴리스 태그 · v 제거) |
| 세그먼트 `model` | lfm2.5-2.6b | 서빙 yaml model(output/single/configs/lfm25-26b-v031.yaml) · hf_repo=체크포인트 .git/config remote url(huggingface.co · /app/models/Liquid-AI/LFM2.5-2.6B → <manifest.nas_model_path>/Liquid-AI/LFM2.5-2.6B) · hf_repo(LiquidAI/LFM2.5-2.6B) 마지막 성분 소문자 |
| 세그먼트 `arch` | gb10-1g1n-sub-native | derived(hw·gpus_per_node·nodes·role·target·plane) |
| 세그먼트 `recipe` | qbf16-len32768-kvauto-t2610162309 | derived(q·len·kv) + timestamp(이름 중복 · generated_utc KST) |
| 축 `hw` | gb10 | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(인증서 measured_node) · 인증서 gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) · vocab hw[gb10]←'NVIDIA GB10' |
| 축 `gpus_per_node` | 1 | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(인증서 measured_node) · 인증서 gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `nodes` | 1 | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(인증서 measured_node) · 인증서 gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `role` | sub | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(인증서 measured_node) · 인증서 gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `target` | native | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(인증서 measured_node) · 인증서 gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) · target_gpu 'NVIDIA GB10' = 호스트 hw → native |
| 축 `plane` | docker | artifacts.plane_of → cell-env(IMAGE_TAG\|BUILD_DOCKERFILE) · vocab plane[docker]→'' |
| 축 `q` | bf16 | /app/models/Liquid-AI/LFM2.5-2.6B → <manifest.nas_model_path>/Liquid-AI/LFM2.5-2.6B/config.json quantization_config·hf_quant_config.json 부재 관측 → 비양자화 · vocab quant[bf16]←'none' |
| 축 `len` | 32768 | 서빙 yaml max-model-len(output/single/configs/lfm25-26b-v031.yaml) |
| 축 `kv` | auto | 서빙 yaml kv-cache-dtype 부재 관측(output/single/configs/lfm25-26b-v031.yaml) → 엔진 기본값 auto · vocab kv[auto]←'auto' |
| timestamp | `t2610162309` | 같은 이름이 이미 있어 붙였다(이 발행의 generated_utc 를 KST 로 · 결정론) |
<!-- /FACT:context -->

> 이 절이 답하는 질문: 이 지도는 어떤 조건에서만 성립하며, 이 형상은 어떤 기전으로 버티고 다른 형상은 왜 실패했는가?

(가) 이 지도가 성립하는 조건. ① 통합메모리 GB10 한 장에서 같은 노드에 두 모델(포트 8001·8002)이 상주하는 배치다 — 서빙 yaml 주석이 gmu 0.39 를 "같은 노드 2모델 공존용 free-memory 검사" 값으로 적는다(근거 artifacts yaml 주석 C2-1). yaml 주석은 gmu 를 기동 전 free-memory 게이트로 적고(한 셀 관측 · 기전 미확정), 엔진 로그는 kv_cache_memory_bytes 가 설정돼 있으면 메모리 프로파일링을 건너뛴다고 적는다 — 이 셀의 실제 KV 는 클램프가 정한다. 다른 메모리 구조에서의 거동은 이 계보에 관측이 없다. ② 측정은 lite 한 번이다 — cold 단일 요청 1회와 warm burst(N=3 · 동시성 1)가 전부이며 합성(synthetic) 입력이다(bench_report_26101623_lfm2.5-2.6b_GB10_0.31.0 §측정 구성 · §lite 지표). 반복·동시성 곡선·판정이 없으므로 이 수치를 full 측정(예: CNN 프리셋 수치)과 나란히 놓지 않는다. ③ 이 이미지에는 빌드 원장이 없다(01 §1.1) — 어떤 빌드 패치가 적용됐는지는 재구성으로만 말할 수 있다.

(나) 이 계보에서 다른 형상과 비교한 기록은 없다. 서빙 형상(PLE·offload·KV 설정·양자화)을 바꿔 실패시킨 기록이 계보 문서 10건에 없으므로 "이 형상이 버티는 기전 대 실패 기전" 은 적지 않는다 — 기전 주장은 만들지 않는다. 지위: 미결(비교 실험 부재). 서빙 단계의 시행착오(기동·클램프 수렴)는 이 태그 계보에 포함되지 않았다(§0.8).

## 0.3 벽 지도 요약
> 02-narrative.md §2.2 의 `kind: wall` 블록(과 §2.4 의 `kind: misdiagnosis` 블록)에서 기계가 생성한 표다(손저작 ✗ · 순서 = 넘은 순서). 원인·해소의 전문과 원문 발췌는 02 에 있다.

<!-- FACT:wall_map -->
| 순서 | id | 증상 | 해소 | 전이등급 | 검증 |
|---|---|---|---|---|---|
| 1 | W1 | CNN 프리셋 작성 단계에서 attempt 1 이 중단됐다 | 프리셋을 output/single/configs/cnn_preset.yaml 에 두고 --preset 에 경로로 지정(메인 지시) | arch-invariant | testlog_26101620_p10_cnn_fullbench §전제 — bench_preset.py validate OK, 이후 sweep 완주 |
| 2 | W2 | sweep_bench.sh 가 --topology 없이 시작되자 중단했다 | sweep_bench.sh 에 --topology single 을 붙인다 | judgment | testlog_26101620_p10_cnn_fullbench §전제 — 명령에 --topology single 포함, sweep 완주 |
| 3 | W3 | 발행 결손에 HINT_MISSING_BUILD_LEDGER 가 남는다 | 이 셀에서는 해소하지 않았다(재빌드 ✗) — 결손으로 기재한 채 발행 | arch-invariant | testlog_26101623_q6_서브_lite셀_materialize_resolve_렌더점검 §렌더 점검 상세 — 현 파일에 스탠자 없음, 렌더본에는 있음 |
| 4 | W4 | 정본 렌더 호출이 서브에서 실패했다 | 같은 템플릿을 --template 과 --resolved 로 직접 렌더(임시 경로) | judgment | testlog_26101623_q6_서브_lite셀_materialize_resolve_렌더점검 §렌더 점검 상세 — 렌더 성공, 스탠자 존재 |
| 5 | W5 | 렌더본 Dockerfile 에 ARG TORCH_CUDA_ARCH=None 이 쓰였다 | 해소하지 않았다 — resolved.json 에 null 을 그대로 기재했다 | arch-locked | testlog_26101623_q6_서브_lite셀_materialize_resolve_렌더점검 §렌더 점검 상세 |

**오진 · 정정**(02 §2.4 — 현재지위가 `가설(강등)` 인 기전은 처방이 아니다)

| id | 증상 | 현재지위 | 검증 |
|---|---|---|---|
| M1 | 서브 hint 의 HINT_MISSING_RESOLVE 결손이 서브 평면에서 해소 불가인 항목으로 읽혔다 | 반증 | testlog_26101623_q6_서브_lite셀_materialize_resolve_렌더점검 §결과 표 — resolve 대조 일치, publish 결손 목록에 RESOLVE 없음 |
<!-- /FACT:wall_map -->

## 0.4 결정론 해소값
<!-- FACT:resolved -->
| 항목 | 값 | 출처 |
|---|---|---|
| vLLM 빌드 입력(종류) | release | naming.vllm_build_input |
| vLLM 빌드 입력(ref) | v0.31.0 | docker history build-arg VLLM_REF |
| vLLM 소스 SHA(40자) | 미관측 | 미관측 |
| 직전 릴리스(prev_release) | 미관측 | 미관측 |
| vLLM 엔진 자기보고(엔진 로그 기동 배너) | 0.31.1.dev0+gdb9527a46.d20261014 | output/single/benchlog/lite_publish/lfm25-26b-v031_20261016T140827Z/lite_engine_head_lfm25-26b-v031.log:38(이 측정의 엔진 로그 기동 배너) · 지위 `this-measurement` |
| 인증서 `vllm_version`(강한 일치 키) | 0.31.0 | 생산자 `lite-report-identity` · 경량 리포트 bench_report_26101623_lfm2.5-2.6b_GB10_0.31.0.md 측정 환경 표 vllm_version=0.31.0 · vllm_version_source=derived(envfile IMAGE_TAG) (= 인증서 값 · 같은 측정) — render_report.lite_identity 가 envfile IMAGE_TAG(`easy-vllm:X.Y.Z`)에서 파생 — **엔진 자기보고가 아니다**(빌드 입력 이미지 태그를 x.y.z 로 자른 값 · rc/dev 접미가 사라진다) · env 덮어쓰기(EASY_VLLM_VERSION)였다면 declared(env EASY_VLLM_VERSION) 로 남았을 것이다 |
| wheel 메타(`vllm_build`) | 미관측 | 미관측 — attestation vllm_dist_version · native pip freeze 모두 없다(sweep_index.meta.vllm_build=None 는 sweep_bench 가 엔진 로그에서 `v<x.y.z…>` 정규식으로 잡는 값이다 → engine_self_report 의 원천) |
| 체크포인트 revision(HF · git HEAD) | ab00687315bc1298e9d54e9c4b611dde9867ccc2 | 체크포인트 .git HEAD → refs/heads/main → loose ref(/app/models/Liquid-AI/LFM2.5-2.6B → <manifest.nas_model_path>/Liquid-AI/LFM2.5-2.6B) · refs/remotes/origin/main 와 일치(받아 온 원격 커밋) — checkout 커밋(작업트리 일치는 미검증) · reflog: 측정 시점 checkout = 지금 HEAD(measured_utc 2026-10-16T14:08:27Z 이전 마지막 이동 2026-08-11T01:46:56Z) |
| 빌드 트랙 | source-build | artifacts 선택자(image-history:docker history build-arg 관측(VLLM_REF=v0.31.0 · VLLM_VERSION 없음 → 소스빌드 Dockerfile) — 셀 env 선택자 없음 · output/single/docker-compose.yaml 리터럴 build.dockerfile: Dockerfile.source-build(선택자 변수 없음 — compose build 는 늘 Dockerfile.source-build 를 짓는다 · 이 이미지를 지은 증거 아님)) |
| Dockerfile(선택자) | Dockerfile.source-build | image-history:docker history build-arg 관측(VLLM_REF=v0.31.0 · VLLM_VERSION 없음 → 소스빌드 Dockerfile) — 셀 env 선택자 없음 · output/single/docker-compose.yaml 리터럴 build.dockerfile: Dockerfile.source-build(선택자 변수 없음 — compose build 는 늘 Dockerfile.source-build 를 짓는다 · 이 이미지를 지은 증거 아님) |
| vLLM 저장소 | https://github.com/vllm-project/vllm.git | docker history build-arg VLLM_REPO |
| 이미지 태그 | easy-vllm:0.31.0-cu133-aarch64-source | 셀 env(output/single/envs/.env.lfm25-26b-v031) IMAGE_TAG |
| 이미지 digest | sha256:667eee7bd3b4e84b204ad4887bd811c87dd3537428d026eaeb580ab825d88c8e | docker image inspect easy-vllm:0.31.0-cu133-aarch64-source .Id · .Created 2026-10-14T01:50:16Z(현 로컬 이미지 관측 · 태그는 가변 포인터) · Created < 측정 2026-10-16T14:08:27Z → 측정 전 빌드 — 같은 빌드로 판정 가능(그 사이 재태깅은 관측 밖) |
| torch | 2.13.0a0+9186a08 | docker image inspect Config.Env PYTORCH_VERSION |
| CUDA | 13.3.1.008 | docker image inspect Config.Env CUDA_VERSION |
| NGC 베이스 | nvcr.io/nvidia/pytorch:26.07-py3 | docker image inspect Config.Env NVIDIA_PYTORCH_VERSION = resolved.json ngc_base.image 접두 일치 |
| CPU arch | arm64 | docker image inspect Architecture |
| 드라이버 | 580.173.02 | output/single/manifest.yaml(manifest_contract 로더) driver_version(스캔 시점 선언 — 측정 시각 값 미보장) |
| 드라이버(노드별 관측) | 미관측 | 미관측 |
| ⚠ 드라이버 관측 ≠ 선언 | 미관측 | 미관측 |
| 호스트 OS | 미관측 | 미관측 |

> 재현 좌표는 빌드 입력(릴리스 태그 또는 40자 SHA)이다 — 엔진 자기보고 · 인증서 · wheel 의 버전 문자열은 빌드 입력이 아니다(각 값을 만든 생산자는 출처 칸).

**양자화 구성**(체크포인트 config 관측 · 이름의 `q` 축은 선언 방식 하나 — ModelOpt MIXED 는 층 개수 우세가 보조 규칙)

| 범위 | dtype | 출처 |
|---|---|---|
| 기본 dtype(비양자화 가중치·계산) | `bfloat16` | /app/models/Liquid-AI/LFM2.5-2.6B → <manifest.nas_model_path>/Liquid-AI/LFM2.5-2.6B/config.json dtype |

**이 모델에 필요한 패치**: 실린 빌드 패치 없음(또는 파일 기록 미관측 — 01 §1.1).
<!-- /FACT:resolved -->

> 이 절이 답하는 질문: 위 값들 중 무엇을 그대로 참조하고 무엇을 네 환경에서 다시 도출해야 하며, 이 빌드 트랙은 왜 골랐는가?

(1) 빌드 트랙: 소스빌드(source-build)다. 위 표의 트랙 행이 근거이며(docker history 의 build-arg VLLM_REF=v0.31.0 · VLLM_VERSION 없음), 고른 이유는 계보 문서에는 없고 실린 `artifacts/build_recipe/Dockerfile.source-build` 머리 주석에 있다 — prebuilt vLLM wheel 의 _C 가 NGC torch 와 C++ ABI 불일치로 import 단계에서 죽어 _C 를 NGC torch 에 맞춰 직접 컴파일한다(그 주석의 검증 버전은 0.22.1 이며 이 셀의 v0.31.0 에서 같은 실패를 다시 확인한 기록은 이 계보에 없다). 사후 resolve 스크립트는 같은 결론을 냈다 — torch 핀 2.13.0 ≥ 2.11 이면 source-build 제안(testlog_26101623_q6_서브_lite셀_materialize_resolve_렌더점검 §결과 표). 단 그 resolve 는 이미지 빌드 후에 돌린 사후 실행이며 빌드 때의 해소 기록이 아니다.
(2) vLLM 버전 문자열: 재현 좌표는 빌드 입력 ref `v0.31.0` 이다(docker history build-arg). 엔진 기동 배너의 자기보고 문자열은 이름이 다르며 빌드 입력이 아니다 — 표의 엔진 자기보고 행은 이 측정의 엔진 로그가 만든 값이다. 인증서 `vllm_version` 은 측정 도구가 이미지 태그를 x.y.z 로 자른 파생값이라 엔진 자기보고가 아니다. wheel 메타는 미관측이다.
(3) arch-locked: `TORCH_CUDA_ARCH=12.1a`(01 §1.1 재구성 입력 build-arg 표)와 CPU arch arm64 는 GB10(aarch64) 전용이다. 다른 GPU 에서는 대상 GPU 의 compute capability 로 다시 정한다. 주의: 이 서버의 manifest 에는 compute_capability 가 없어 resolve_build_track 이 torch_cuda_arch 를 null 로 냈다(W5) — 12.1a 는 resolve 출력이 아니라 이미지 build-arg 관측이다.
(4) 선언 대 관측: 표의 이미지 digest·torch·CUDA·NGC 값은 이미지 관측(docker image inspect)이고 드라이버 값은 manifest 선언을 옮긴 것이다. NGC 베이스는 resolve 스크립트가 낸 `26.07-py3` 와 빌드 로그의 FROM 줄(`nvcr.io/nvidia/pytorch:26.07-py3`)이 일치했다(testlog_26101623_q6_서브_lite셀_materialize_resolve_렌더점검 §결과 표).
(5) 미관측 좌표: 호스트 OS 와 드라이버의 노드별 관측은 계보 문서·증거 어디에도 없어 미기록이다. 소스 SHA 는 40자 값이 없고 엔진 기동 배너(`0.31.1.dev0+gdb9527a46.d20261014`)에 `g` 뒤 9자만 있다 — 재현 좌표로 쓰지 않는다. 호스트 좌표와 이미지 좌표는 다른 층이다.
(6) 핵심 의존 핀: 실린 `requirements.txt` 가 핀한 값은 그 파일 원문을 따르고, build-arg 는 위 (3)의 표(RAY_VERSION · BUILD_JOBS 등)다. 이 중 어느 것이 이 모델의 요구이고 어느 것이 공유 이미지 기본값인지 가르는 기록은 없다.

## 0.5 서빙 노브와 값의 지위
> 01-artifacts.md §1.3 의 `kind: value-status` 블록에서 기계가 생성한 요약이다. 지위가 `tuned` · `declared-requirement` 가 아닌 값은 **이 셀에서 조정된 적도, 빼면 깨진다고 확인된 적도 없다** — 필요조건으로 복사하지 마라. `declared-requirement` 는 빼거나 바꾸면 실패가 관측된 값이다(근거의 W id · artifacts 헤더를 보고 네 환경에서도 그 실패 조건이 성립하는지 판단한다).

<!-- FACT:knob_status -->
노브 4개 — `inherited` 4.

| 노브 | 값 | 지위 | 근거 | id |
|---|---|---|---|---|
| `gpu-memory-utilization` | 0.39 | inherited | yaml 주석 C2-1 은 gmu_source=hand 라 적지만 같은 yaml 의 explorer 각인 줄은 gmu_source=target_gmu 라 적는다 — 두 표기가 갈린다. 어느 쪽이든 이 계보 문서에 값을 바꿔 잰 기록은 없다. 같은 노드 두 모델 공존용 free-memory 검사값이며 KV 는 클램프가 정한다고 yaml 주석이 적는다. | V1 |
| `max-model-len` | 32768 | inherited | 레시피 헤더 max_model_len=32768 — 이 계보 문서에 값을 바꿔 잰 기록이 없다. | V2 |
| `max-num-seqs` | 8 | inherited | yaml 주석 batch_source=hand-lever(손레버) — 스윕 기록 없음. | V3 |
| `kv-cache-memory-bytes` | 40694815129 | inherited | yaml 주석: 클램프 37.9 GiB = 천장 47.68 − weights 5.05 − overhead 실측 ≈4.7, kv_source=hand(손레버). 이 계보 문서에 클램프를 바꿔 잰 기록이 없다. | V4 |
<!-- /FACT:knob_status -->

> 이 절이 답하는 질문: 이 레시피를 다른 환경으로 옮길 때 어떤 노브를 먼저 다시 재야 하고, 어떤 노브는 빼면 깨지는가?

① 다시 도출할 값(arch-scaled): `kv-cache-memory-bytes`, `gpu-memory-utilization`, `max-model-len`, `max-num-seqs`. 이 계보에는 값을 바꿔 잰 기록이 없으므로 네 환경의 메모리와 목표 동시성으로 다시 정한다 — 엔진 로그의 가중치 GiB 와 KV 토큰 수(Model loading 5.05 GiB · GPU KV cache size 2,476,552 tokens)가 이 셀의 실측 출발점이다.
② 필요조건으로 확인된 값(declared-requirement): 없음 — 빼거나 바꿔서 실패가 관측된 기록이 이 계보에 없다. `--tool-call-parser lfm2` · `--reasoning-parser qwen3` 는 러너(.sh)가 지정하지만 필요조건으로 확인한 문서도 없다.
③ 승계값: 위 네 노브 전부 inherited 다 — 손레버로 정했거나 레시피에서 가져온 값이며 최적이라는 뜻이 아니다.

## 0.6 재검증 · 라우팅
- 이 자료를 `vllm-recipe-explorer` 인터뷰의 **warm-start 근거로만** 투입하고, 평소대로 plan-gate → 레시피 수렴 → 스모크 게이트를 그대로 밟아라(가속이지 우회 ✗).

> 이 절이 답하는 질문: 다른 환경(다른 HW · 다른 vLLM 버전 · 다른 노드 수)에서 이 지도를 쓸 때 무엇부터, 어떤 순서로 확인해야 하며, 실패하면 어디로 돌리는가?

1. 빌드 좌표 → 네 이미지의 docker history 에서 build-arg `VLLM_REF` 를 읽어 `v0.31.0` 과 같은지 본다 → 같으면 그대로, 다르면 이 지도를 폐기하고 `upstream-version-watch` 로.
2. 기동 → 서빙 후 `/health` 200 과 chat 추론 1회(01 §1.4 의 serve 단계 성공 판정) → 실패하면 `vllm-recipe-explorer` 로 서빙 노브를 다시 수렴시킨다.
3. 측정 → 이 셀은 lite 한 번이다. 같은 `lite_bench.sh`(00 머리 측정 도구 원문 좌표)로 네 환경에서 lite 를 다시 돌려 pass 인지 본다 — 성능 baseline 은 full 측정(`adversarial-benchmark`)이 있어야 주장할 수 있다.
4. 이 태그에는 자체 패치가 실리지 않았다(01 §1.1 슬롯 표). 그러나 빌드 원장이 없어 "패치가 없었다" 는 관측이 아니라 재구성(unobservable)이다 — 상위 버전에서 패치 필요성은 확인 대상으로 남긴다. 엔진 기동 배너의 날짜 접미(.d…)가 빌드 때 소스 수정을 뜻하는지는 이 계보에 관측이 없다(미확정).
5. 빌드 트랙 → 네 환경에서 `resolve_torch_pin` · `resolve_ngc_tag` 를 돌려 source-build 가 맞는지 확인하고, manifest 에 compute_capability 가 없으면 `torch_cuda_arch` 가 null 로 나온다(W5)는 점을 감안해 대상 GPU 값을 명시한다.
행선지: 빌드·버전 핀 문제 → `upstream-version-watch` · 서빙 설정·KV·노브 문제 → `vllm-recipe-explorer` · 성능 기대 이하 → `adversarial-benchmark` · 노드·토폴로지 문제 → `terraforming_node`.
호스트 메모리 예산 선언은 단일노드 두 모델 상주 때 쓰였으나 산식 입력은 01 §1.4 이벤트 표(floor·weights·kv·overhead 선언값)를 따르며, 선언값은 측정이 아니다.

## 0.7 메타
<!-- FACT:meta -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.31.0/lfm2.5-2.6b/gb10-1g1n-sub-native/qbf16-len32768-kvauto-t2610162309` |
| 형식 | `hint-payload/v7` |
| 앵커 | `PROVENANCE.json` — 봉인 때 hint 브랜치 페이로드 커밋에 묶인다 |
| 캠페인 · 셀 · 노드 | camp-26101623-baseline · lfm25-26b-v031 · sub |
| 발행 모드 | campaign |
| 발행 토픽 | camp_26101623_baseline__sub__lfm25_26b_v031 |
| work-manifest | docs/_evidence/camp_26101623_baseline__sub__lfm25_26b_v031.work-manifest.json |
| 증거 등급(task_class) | hint_map_only |
| 승인 출처 | campaign:hint_targets |
| 승인 시각(UTC) | 2026-10-16T14:03:00Z |
| 승인 발화(전사) | 사용자(캠페인 개설 팝업 발화 전사) — "열기 · 2셀 발행 승인 (Recommended)" |
| 현행 full 정의 충족(측정 등급) | 미관측 · 이 측정의 반복 1 · 정의 출처 `.claude/rules/docs.md`(03 §3.5) |
| 독립 사실 검증(발행 전) | 통과 — 검증자 `서브 검증 Agent(q7-sub-check 세션)`(저작자 `서브 저작 Agent(q6-sub-lite 세션)` 아님) · 주장 74건 · 지적 4건 중 고침 4건 · 열림 0 · 2026-10-16T14:18:12Z |
<!-- /FACT:meta -->

<!-- FACT:missing -->
**결손** — 발행을 막지 않고 기재한다(부재와 실패는 다른 사실이다 · 차단은 양성 검출만).

| 코드 | 뜻 |
|---|---|
| `BENCH_MODE_LITE` | full bench 정의(lite ∪ GuideLLM × 반복 ≥3)를 충족하지 않았다고 **기재된** 측정이다 — 선언된 lite-only 셀이거나 반복 불성립(기계 이벤트)으로 강등된 셀이다. 수치는 lite 스냅샷(또는 강등 셀의 대표 run 1회)이라 산포 추정치·인증서가 없다. 어느 쪽인지·강등 사유는 측정 구성 표(bench_mode_kind · downgrade_reason)가 말한다. bench_mode 를 읽지 못한 측정(미확정·미기재)에는 붙이지 않는다 — 모름을 lite 로 접으면 합성이다(그 사실은 카탈로그 bench_mode 칸이 말한다). |
| `HINT_MISSING_BUILD_LEDGER` | 빌드 원장(`/opt/easy-vllm/build_ledger.json`) 부재 — 원장 도입 전 이미지다. 패치 적용 결과는 `applied_set.status: unobservable` 과 라벨된 재구성으로만 말한다(X9). |
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
**확정 이름** `hint/0.31.0/lfm2.5-2.6b/gb10-1g1n-sub-native/qbf16-len32768-kvauto-t2610162309` — 꼬리 0토큰 · timestamp `t2610162309`.

_빈 꼬리 — 발행 Agent 가 가르는 노브가 없다고 명시했다(`inputs/tail.json` = `[]`)._

- `t2610162309` = 같은 이름(결정론부 + 꼬리)이 원격 · 로컬에 이미 있어 붙인 발행 시각(generated_utc 의 KST `YYMMDDHHMM`) — 같은 셀의 새 판이다(옛 태그는 교정하지 않는다).
<!-- /FACT:name_tail -->

> 이 절이 답하는 질문: 이 셀을 같은 q·len·kv 의 다른 셀과 가르는 노브는 무엇이며 그것을 이름 꼬리로 어떻게 적는가 — 가르는 노브가 없으면 왜 빈 꼬리인가?

가르는 노브가 없어 꼬리는 빈 목록이다 — 같은 q·len·kv 의 다른 셀과 가르는 노브는 모델 세그먼트가 이미 가르고, 후보 셋(ple·spec·graph)은 모두 엔진 기본값이거나 부재 관측이라 이 셀을 다른 셀과 가르는 노브가 아니다.
