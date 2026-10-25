# 00 · hint 지도 — 가장 먼저 읽는 파일

<!-- FACT:header -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.31.0/gemma-4-26b-a4b-it/gb10-1g1n-main-native/qbf16-len262144-kvfp8` |
| 형식 | `hint-payload/v7` · 이름 문법 `v7` |
| 생성(UTC · 주입) | 2026-10-25T04:10:00Z |
| 캠페인 · 셀 · 노드 · 모드 | 캠페인 밖(`_bootstrap` · output/single/benchlog/sweep_gemma26b-ctx-single/sweep_index.json bench_knobs.campaign = null · campaign_pins_note '활성 캠페인 없음(_bootstrap)'(측정 시점 ACTIVE 해소 · repeat_axis)) · gemma26b-ctx-single · main · publication-replay |
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

Gemma-4-26B-A4B-it(BF16)을 GB10 1대(TP=1 · 싱글)에서 vLLM v0.31.0 stock 소스빌드로 max-model-len 262144 · KV fp8 · PLE 없음 · spec off · cudagraph 로 서빙했고, 판정점 동시성 1 decode 23.29 t/s(GuideLLM × 반복 3 · synthetic)는 roofline 하한 13.69 대비 PASS(권한 weak · 인증서 없음)다.
가장 비싼 벽은 KV 절대 클램프 없이 뜬 첫 트라이얼이 통합메모리에서 KV 를 과다하게 잡아 호스트 memwatch 에 사살된 것이고 kv-cache-memory-bytes 절대 클램프로 넘었다(W1 · M1) — 그다음은 생성기가 yaml port 에 호스트 포트를 적어 health 가 닿지 않은 것이다(W2).
가장 오해하기 쉬운 값은 KV 클램프 3368402176(V3)이다 — 필요조건은 클램프의 존재이고 값은 262144 토큰 한 요청분이라 동시 장문 요청을 받지 못하며, gmu 0.85(V1)는 손레버 승계값이다.

## 0.2 유효맥락
<!-- FACT:context -->
| 항목 | 값 | 출처 |
|---|---|---|
| 모델 | gemma-4-26b-a4b-it | sweep_index.meta(output/single/benchlog/sweep_gemma26b-ctx-single) |
| HF repo | google/gemma-4-26B-A4B-it | 체크포인트 .git/config remote url(huggingface.co · /app/models/Google/Gemma_4/gemma-4-26B-A4B-it → <manifest.nas_model_path>/Google/Gemma_4/gemma-4-26B-A4B-it) |
| HF revision(체크포인트 git HEAD) | 01e5b3ee840d3a9e0b0b493c593e85398a30ef75 | 체크포인트 .git HEAD → refs/heads/main → loose ref(/app/models/Google/Gemma_4/gemma-4-26B-A4B-it → <manifest.nas_model_path>/Google/Gemma_4/gemma-4-26B-A4B-it) · refs/remotes/origin/main 와 일치(받아 온 원격 커밋) — checkout 커밋(작업트리 일치는 미검증) · reflog: 측정 시점 checkout = 지금 HEAD(measured_utc 2026-10-25T00:44:08Z 이전 마지막 이동 2026-07-18T04:49:16Z) |
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
| 세그먼트 `recipe` | qbf16-len262144-kvfp8 | derived(q·len·kv) |
| 축 `hw` | gb10 | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(발행 인자 --node) · sweep meta gpu_model 일치 · target: output/single/benchlog/sweep_gemma26b-ctx-single/roofline.json gpu_model — 루브릭 계산 HW(측정 호스트 manifest) · 캠페인 밖 측정(측정 시점 ACTIVE=_bootstrap · 이 측정의 캠페인 인스턴스 없음) 로 타겟 선언 미관측(재생 전용 대체 · 선언 아님) · vocab hw[gb10]←'NVIDIA GB10' |
| 축 `gpus_per_node` | 1 | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(발행 인자 --node) · sweep meta gpu_model 일치 · target: output/single/benchlog/sweep_gemma26b-ctx-single/roofline.json gpu_model — 루브릭 계산 HW(측정 호스트 manifest) · 캠페인 밖 측정(측정 시점 ACTIVE=_bootstrap · 이 측정의 캠페인 인스턴스 없음) 로 타겟 선언 미관측(재생 전용 대체 · 선언 아님) |
| 축 `nodes` | 1 | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(발행 인자 --node) · sweep meta gpu_model 일치 · target: output/single/benchlog/sweep_gemma26b-ctx-single/roofline.json gpu_model — 루브릭 계산 HW(측정 호스트 manifest) · 캠페인 밖 측정(측정 시점 ACTIVE=_bootstrap · 이 측정의 캠페인 인스턴스 없음) 로 타겟 선언 미관측(재생 전용 대체 · 선언 아님) |
| 축 `role` | main | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(발행 인자 --node) · sweep meta gpu_model 일치 · target: output/single/benchlog/sweep_gemma26b-ctx-single/roofline.json gpu_model — 루브릭 계산 HW(측정 호스트 manifest) · 캠페인 밖 측정(측정 시점 ACTIVE=_bootstrap · 이 측정의 캠페인 인스턴스 없음) 로 타겟 선언 미관측(재생 전용 대체 · 선언 아님) |
| 축 `target` | native | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(발행 인자 --node) · sweep meta gpu_model 일치 · target: output/single/benchlog/sweep_gemma26b-ctx-single/roofline.json gpu_model — 루브릭 계산 HW(측정 호스트 manifest) · 캠페인 밖 측정(측정 시점 ACTIVE=_bootstrap · 이 측정의 캠페인 인스턴스 없음) 로 타겟 선언 미관측(재생 전용 대체 · 선언 아님) · target_gpu 'NVIDIA GB10' = 호스트 hw → native |
| 축 `plane` | docker | artifacts.plane_of → cell-env(IMAGE_TAG\|BUILD_DOCKERFILE) · vocab plane[docker]→'' |
| 축 `q` | bf16 | /app/models/Google/Gemma_4/gemma-4-26B-A4B-it → <manifest.nas_model_path>/Google/Gemma_4/gemma-4-26B-A4B-it/config.json quantization_config·hf_quant_config.json 부재 관측 → 비양자화 · vocab quant[bf16]←'none' |
| 축 `len` | 262144 | 서빙 yaml max-model-len(output/single/configs/gemma26b-ctx-single.yaml) |
| 축 `kv` | fp8 | 서빙 yaml kv-cache-dtype(output/single/configs/gemma26b-ctx-single.yaml) · vocab kv[fp8]←'fp8' |
<!-- /FACT:context -->

> 이 절이 답하는 질문: 이 지도는 어떤 조건에서만 성립하며, 이 형상은 어떤 기전으로 버티고 다른 형상은 왜 실패했는가?

**(가) 이 결과를 이 조건에서만 성립하게 만든 전제**

1. **통합메모리 단일 노드 · BF16 가중치 상주** — GB10 은 GPU 와 CPU 가 한 메모리 풀이라 엔진이 잡는 KV 가 곧 호스트 MemAvailable 을 깎는다 → KV 절대 클램프 없이 gmu 0.85 로 띄우면 엔진이 KV 52.89 GiB 를 잡아 가중치 48.54 GiB 와 함께 memwatch 절대 문턱(10,240 MiB) 아래로 내려갔다 → 전용 VRAM 이 분리된 HW 에서는 gmu 가 VRAM 비율만 뜻하므로 같은 실패가 같은 모양으로 나지 않는다(02 W1 · testlog_26102412 §1. 기동).
2. **체크포인트가 CIFS(NAS) 위** — 엔진이 CIFS 를 인식된 네트워크 FS 로 보지 않아 auto-prefetch 를 끄고(lite_engine L65) 가중치 적재가 265.25 s 걸렸다 → 기동 소요(선언 → API 서버 5분 58초)는 이 저장 구성의 값이다 → 로컬 NVMe · NFS 에서는 적재 시간이 달라진다(추론 · 01 §1.4 (a)).
3. **Gemma4 의 이질 헤드 차원** — sliding 256 · full 512 라 엔진이 FA4 가 없다며 TRITON_ATTN 을 강제했다(lite_engine L48) → 이 셀의 decode 수치는 그 attention 경로의 값이다 → 다른 GPU 세대 · vLLM 버전에서 다른 백엔드가 고를 수 있으면 수치가 달라진다(01 §1.4 (f)).

**(나) 이 형상이 버티는 기전 대 다른 형상이 실패한 이유**

- KV 클램프 없는 형상(trial01) — 후보 yaml 차이는 `kv_cache_memory_bytes` null 대 3368402176 한 줄이고 그 밖에 예산 선언 유무(trial01 생략 · trial02 선언)가 다르다(선언 유무는 사살한 절대 임계 워치독의 동작과 무관하다 · 02 W1) — 클램프가 없으면 엔진이 gmu 기준으로 KV 를 최대로 잡아 통합메모리의 OS 몫을 문턱 밑으로 누르고, 클램프가 있으면 KV 가 3.14 GiB 로 고정되며 엔진은 프로파일링을 건너뛴다(lite_engine L84) — 확정(트라이얼 대조 · 원장 `watchdog_trip` · 엔진 로그) — testlog_26102412 §1. 기동 · 02 W1. 엔진이 쥔 KV 가 회수되지 않는 메모리라는 기전 서술은 추론이다.
- yaml `port` 8912 형상(캠페인 정규 기동 1차) — 메모리 형상은 같고 포트 칸만 다르다 — 컨테이너가 compose 매핑 대상(8000)이 아닌 포트를 들어 health 가 닿지 않았다(메모리 기전 아님) — 확정 — testlog_26102412 §1. 기동 · 02 W2.
- 다른 KV 값 · gmu · 동시 시퀀스 · YaRN 확장 · 양자화본 — 시도 0(02 R1 · R2 · §2.5) — 대조가 없다 — 미결.

## 0.3 벽 지도 요약
> 02-narrative.md §2.2 의 `kind: wall` 블록(과 §2.4 의 `kind: misdiagnosis` 블록)에서 기계가 생성한 표다(손저작 ✗ · 순서 = 넘은 순서). 원인·해소의 전문과 원문 발췌는 02 에 있다.

<!-- FACT:wall_map -->
| 순서 | id | 증상 | 해소 | 전이등급 | 검증 |
|---|---|---|---|---|---|
| 1 | W1 | KV 클램프 없이 뜬 트라이얼이 로드 뒤 호스트 memwatch 에 사살됨(예산 선언은 kv_clamp_absent 로 생략) | kv-cache-memory-bytes 3368402176(262144 × batch 1 필요량) 절대 클램프 후 재기동(클램프가 있어 예산 선언도 생략되지 않았다) | arch-scaled | trial02_smoke.json passed true · 측정 기동 lite_engine L84 reserved 3.14 GiB for KV Cache as specified by kv_cache_memory_bytes |
| 2 | W2 | single_serve_up.sh 1차 rc=5 — 엔진은 떴으나 health 프로브가 닿지 않아 기동 대기 타임아웃 | 이 트리플렛 yaml 의 port 를 8000 으로 수작업 교정(생성기 결함은 미수정 후속) | arch-invariant | testlog_26102412 §1. 기동 정규 기동 재시도 health 200 (470 s) rc=0 |

**오진 · 정정**(02 §2.4 — 현재지위가 `가설(강등)` 인 기전은 처방이 아니다)

| id | 증상 | 현재지위 | 검증 |
|---|---|---|---|
| M1 | 첫 트라이얼이 로드 뒤 컨테이너 사살로 끝남 | 미결 | docs/logs/main/events/2026-10.jsonl L491 budget_skipped reason kv_clamp_absent · L493 watchdog_trip source unit:easy-vllm-memwatch rule absolute |
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
| 체크포인트 revision(HF · git HEAD) | 01e5b3ee840d3a9e0b0b493c593e85398a30ef75 | 체크포인트 .git HEAD → refs/heads/main → loose ref(/app/models/Google/Gemma_4/gemma-4-26B-A4B-it → <manifest.nas_model_path>/Google/Gemma_4/gemma-4-26B-A4B-it) · refs/remotes/origin/main 와 일치(받아 온 원격 커밋) — checkout 커밋(작업트리 일치는 미검증) · reflog: 측정 시점 checkout = 지금 HEAD(measured_utc 2026-10-25T00:44:08Z 이전 마지막 이동 2026-07-18T04:49:16Z) |
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

(1) **빌드 트랙** — source-build 다(위 표 트랙 행 · 출처 빌드 원장 `dockerfile`). 트랙 이유는 실린 Dockerfile 머리 주석에 있다(L29 ~ L32 — prebuilt wheel 의 `_C` 가 NGC alpha torch 와 C++ ABI 가 맞지 않아 import 단계에서 막히므로 `_C` 를 NGC torch 에 맞춰 직접 컴파일한다). 이 이미지는 계보 밖 testlog_26101412(읽기 목록 밖 · 0.31.0 bump 판정)가 single `Dockerfile.source-build` · stock · 빌드 패치 0 · 메인 40분으로 적은 빌드이고, 첫 빌드는 v0.31.0 이 `use_existing_torch.py` 를 `tools/` 로 옮겨 실패했다가 자기적응 단계로 고쳐 통과했다(실린 Dockerfile L82 ~ L84). 다른 트랙으로 갈 조건은 이 계보에 기록이 없다.

(2) **vLLM 버전 문자열** — 넷이 관측된다(빌드 입력 ref · 엔진 기동 배너 · 측정 강식별 키 · wheel 메타). 빌드 입력 ref `v0.31.0` 은 docker history build-arg `VLLM_REF` 이고 **재현 좌표는 이것 또는 40자 SHA `db9527a46873454610df6dbedf79a36d6bf1a7f6`**(`resolved.json upstream_delta.to_sha`)다. 엔진 기동 배너 `0.31.1.dev0+gdb9527a46.d20261014` 는 소스 트리가 빌드 때 붙인 개발 버전 문자열이라 빌드 입력이 아니다(`gdb9527a46` 은 위 SHA 의 앞부분). 측정 강식별 키 `vllm_version` 0.31.0 은 측정 도구가 sweep meta 에 적은 값이다 — 인증서는 없다(위 표 `absent`). wheel 메타 `0.31.1.dev0+gdb9527a46.d20261014.cu133` 은 위 표의 출처대로 이미지 안 빌드 원장 `/opt/easy-vllm/build_ledger.json` 의 `vllm.dist_version`(빌드 시점 measured)이고, 엔진 배너와 같은 개발 버전에 `.cu133` 이 붙은 값이라 역시 빌드 입력이 아니다.

(3) **arch-locked 값** — `TORCH_CUDA_ARCH=12.1a`(build-arg · 네 GPU 의 compute capability 로 다시 정한다 — `nvidia-smi --query-gpu=compute_cap` 등) · CPU arch `arm64`(image inspect · 네 호스트 `uname -m` 과 맞는 NGC 베이스를 고른다) · 드라이버 580.178.04(호스트 층 · manifest 선언 · 이미지 CUDA 13.3.1 을 받는 드라이버 계열인지 네 호스트에서 확인한다) · NGC 베이스 `26.07`(이미지 CUDA 13.3.1 · torch 2.13.0a0 을 함께 정한다) · `BUILD_JOBS=8`(build-arg · 네 빌드 호스트 메모리에 맞춘다 — Dockerfile 기본값은 16 · L48).

(4) **선언 대 관측** — NGC 베이스는 선언(`resolved.json ngc_base.image`)과 관측(image inspect `NVIDIA_PYTORCH_VERSION`)이 접두 일치한다(위 표). 실린 레시피 가운데 requirements.txt 는 빌드 원장 sha256 과 일치하고(verified), Dockerfile 은 docker history 와 16/17 만 맞는다(generated-unverified · 이미지 빌드 뒤 커밋 · 01 §1.1). 서빙 이미지의 실제는 image inspect · 빌드 원장(관측)이다. 리포트 스냅샷의 `cuda_version 132` 는 측정 도구가 manifest 에서 옮긴 호스트 층 값이고 이미지 CUDA 13.3.1 과 다른 층이다.

(5) **미관측 환경 좌표** — 호스트 OS 는 이 계보에서 찾지 못했다(**미기록**). 노드당 메모리는 첫 plan 이 `~121GiB` 로 적었고(plan_26102401 §NAS 사실), 실측 MemTotal 은 이 측정 lite raw 의 `ram_total_kib 127598772`(= 124608 MiB · 121.69 GiB)와 트라이얼 요약의 `measured(/proc/meminfo MemTotal)` 121.69 GiB 다(run_summary L73 ~ L74). 원장 선언 `mem_total_mib 124608` 은 이 실측과 같은 값을 선언에 옮긴 것이다. 인터커넥트는 싱글이라 해당 없다. 드라이버는 manifest 선언(580.178.04)만 있고 노드 관측은 없다(위 표 · 대조 불가).

(6) **핵심 의존 핀** — Ray 2.48.0(build-arg `RAY_VERSION` · Dockerfile L219 주석이 2노드 분산 서빙용이라고 적는다 — 공유 이미지 의존이며 이 셀의 엔진은 world_size=1 로 떴다 · lite_engine L54) · 실린 requirements.txt(원장 sha256 일치)의 `flashinfer-python==0.7.0.post1`(L20) · `transformers<5.18.0,>=5.10.4,<5.15.0` constraint(L73 · KNOWN_INCOMPAT 천장 `<5.15.0`) · `humming-kernels[cu12]==0.1.16`(L22) · `compressed-tensors==0.17.0`(L14). 이 모델 전용 핀은 없다 — 전부 공유 이미지의 의존이다. Dockerfile 머리 주석(L7)은 transformers 5.15.0 이 gemma-4 서빙을 로드 전에 죽였다는 이전 기록으로 그 천장의 이유를 적는다(이 셀에서 재현한 실패 아님). 이 모델이 실제로 쓴 커널 경로는 TRITON_ATTN · FlashInfer CUTLASS 비양자화 MoE 다(01 §1.4 (f)).

## 0.5 서빙 노브와 값의 지위
> 01-artifacts.md §1.3 의 `kind: value-status` 블록에서 기계가 생성한 요약이다. 지위가 `tuned` · `declared-requirement` 가 아닌 값은 **이 셀에서 조정된 적도, 빼면 깨진다고 확인된 적도 없다** — 필요조건으로 복사하지 마라. `declared-requirement` 는 빼거나 바꾸면 실패가 관측된 값이다(근거의 W id · artifacts 헤더를 보고 네 환경에서도 그 실패 조건이 성립하는지 판단한다).

<!-- FACT:knob_status -->
노브 4개 — `inherited` 3 · `declared-requirement` 1.

| 노브 | 값 | 지위 | 근거 | id |
|---|---|---|---|---|
| `gpu-memory-utilization` | 0.85 | inherited | 손레버 — yaml 머리는 gmu_source=hand · 트라이얼 요약은 config.safety_margin 승계로 적는다 · 바꿔 잰 기록 없음 · KV 클램프가 있어 엔진은 KV 산정에 이 값을 쓰지 않는다고 기록(lite_engine L84) | V1 |
| `max-model-len` | 262144 | inherited | config max_position_embeddings(native 최대) · YaRN 미적용(R1) · 240,932 토큰 needle 까지 검증 · 다른 길이로 바꿔 잰 기록 없음 | V2 |
| `kv-cache-memory-bytes` | 3368402176 | declared-requirement | W1 — 이 노브를 비운 트라이얼(null)이 KV 52.89 GiB 를 잡아 memwatch 에 사살됐다 · 필요조건은 클램프의 존재이고 값 자체는 262144 × batch 1 필요량 파생(다른 값 시도 0 · 엔진 1.02x) | V3 |
| `kv-cache-dtype` | fp8 | inherited | Phase 1 레시피 후보의 KV dtype(run_summary kv_cache_quant fp8) · auto 등 다른 dtype 으로 잰 기록 없음 · 엔진이 scaling factor 없는 fp8 의 정확도 저하 가능성을 경고(lite_engine L45) | V4 |
<!-- /FACT:knob_status -->

> 이 절이 답하는 질문: 이 레시피를 다른 환경으로 옮길 때 어떤 노브를 먼저 다시 재야 하고, 어떤 노브는 빼면 깨지는가?

**① 네 환경에서 반드시 다시 도출할 값(arch-scaled)** — `kv-cache-memory-bytes`(V3) · `gpu-memory-utilization`(V1) · `max-model-len`(V2). 절차: 목표 컨텍스트 × 동시 요청 수로 KV 필요량을 정하고(이 셀은 262144 × batch 1 · 02 §2.5 발췌), `budget_preflight.py` 에 네 노드의 MemTotal · 상주 가중치 · KV · overhead 를 넣어 floor · 무장 상한이 선언 가능한지 본다 → 기동 뒤 엔진 로그의 `GPU KV cache size: … tokens, Maximum concurrency for … tokens per request` 줄로 들어갔는지 확인한다(이 셀은 1.02x).

**② 필요조건으로 확인된 값(declared-requirement)** — `kv-cache-memory-bytes` 의 **존재**(V3 · W1): 비우면 엔진이 gmu 기준으로 KV 를 최대로 잡아 통합메모리에서 memwatch 사살이 났다. 값 3368402176 자체는 필요조건이 아니라 이 목표의 필요량이다. **명시하면 깨지는 노브** 는 yaml `port` 에 호스트 포트를 적는 것이다(W2 · 전송 노브라 지위표 밖) — 컨테이너 내부 포트 8000 을 두고 호스트 포트는 셀 env `SERVING_PORT` 로 준다.

**③ 조정된 적 없는 승계값** — `gpu-memory-utilization 0.85`(V1 — 손레버 · 클램프 경로에서는 KV 산정에 쓰이지 않는다) · `max-model-len 262144`(V2 — native 최대 · YaRN 미적용 · 92% 지점까지만 검증) · `kv-cache-dtype fp8`(V4 — 비교 없이 고른 값 · 정확도 영향 미측정). 셋 다 최적이라는 근거가 아니라 이 셀이 바꿔 보지 않았다는 뜻이다.

## 0.6 재검증 · 라우팅
- 이 자료를 `vllm-recipe-explorer` 인터뷰의 **warm-start 근거로만** 투입하고, 평소대로 plan-gate → 레시피 수렴 → 스모크 게이트를 그대로 밟아라(가속이지 우회 ✗).

> 이 절이 답하는 질문: 다른 환경(다른 HW · 다른 vLLM 버전 · 다른 노드 수)에서 이 지도를 쓸 때 무엇부터, 어떤 순서로 확인해야 하며, 실패하면 어디로 돌리는가?

1. **빌드 좌표와 의존** → 확인: 네 vLLM 이 `v0.31.0`(또는 40자 SHA)과 같은지 · NGC 베이스 torch 와 vLLM 이 요구하는 torch(00 §0.4 (1)) · 빌드 원장(`/opt/easy-vllm/build_ledger.json`)의 패치 적용 상태 → 같으면 그대로 · 상위 버전이면 이 셀은 빌드 패치 · 런타임 패치가 0 이라 "자체 패치가 아직 필요한가" 는 물을 것이 없고, 대신 transformers 천장(`<5.15.0` · requirements L5)이 그 버전에서도 필요한지 업스트림에서 다시 확인한다 → 빌드가 깨지면 `upstream-version-watch`.
2. **모델 메모리 형상** → 확인: 네 HW 가 통합메모리인지 · `budget_preflight.py --mem-total-mib <네 MemTotal> --weights-mib <상주 가중치> --kv-mib <필요량> --overhead-mib <값>` 의 floor · 무장 상한 → `declare=OK` 면 그 값으로 예산 선언 · 아니면 KV · 컨텍스트 목표를 낮춰 재도출 → 클램프 없이 기동하지 않는다(클램프가 없으면 선언도 생략된다 · W1) · 선언은 하네스의 기동 절차이고 MemAvailable 절대 임계 워치독은 선언과 무관하게 돈다(M1).
3. **기동 성공 판정** → 확인: `single_serve_up.sh` rc 0 · health 200 · 추론 1회(01 §1.4 기동 성공 판정) · 엔진 로그의 `reserved … memory for KV Cache as specified by kv_cache_memory_bytes`(클램프 적용 서명 · lite_engine L84) · `non-default args` 에 yaml 값이 실렸는지(L42) · 이 셀은 패치가 없으므로 패치 발화 서명은 없다 → health 가 안 닿으면 먼저 yaml `port` 가 8000 인지 본다(W2) → 서빙 설정 문제는 `vllm-recipe-explorer`.
4. **컨텍스트 목표** → 확인: 네 목표가 262144 이하인지 · 그 이상이면 모델 카드 · 업스트림이 확장(YaRN 등)을 선언했는지 → 선언 없으면 native 최대로 정직 기록(R1) → 근접 길이 needle(증분 프리필)로 검증하고, 262k 근처는 이 지도가 검증하지 않았다(Q1).
5. **성능** → 확인: 같은 부하(GuideLLM 0.7.3 · synthetic 1024/256 · 동시성 1)로 잰 decode 를 이 셀의 판정점과 비교 · 네 HW 의 roofline 기대치를 판정기로 다시 낸다 → 기대 이하면 `adversarial-benchmark` · lite 값과 full 값을 섞지 않는다(03 §3.5).
6. **노드 · 저장 구성** → 확인: 체크포인트 파일시스템(CIFS 면 auto-prefetch 꺼짐 · lite_engine L65) · 노드 메모리 총량 → 적재가 비정상적으로 느리거나 노드 사실이 manifest 와 다르면 `terraforming_node`.

실패 신호별 행선지: 빌드 · 버전 핀 문제 → `upstream-version-watch` · 서빙 설정 · KV · 노브 문제 → `vllm-recipe-explorer` · 성능 기대 이하 → `adversarial-benchmark` · 노드 · 토폴로지 · 인터커넥트 문제 → `terraforming_node`.

## 0.7 메타
<!-- FACT:meta -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.31.0/gemma-4-26b-a4b-it/gb10-1g1n-main-native/qbf16-len262144-kvfp8` |
| 형식 | `hint-payload/v7` |
| 앵커 | `PROVENANCE.json` — 봉인 때 hint 브랜치 페이로드 커밋에 묶인다 |
| 캠페인 · 셀 · 노드 | 캠페인 밖(`_bootstrap` · output/single/benchlog/sweep_gemma26b-ctx-single/sweep_index.json bench_knobs.campaign = null · campaign_pins_note '활성 캠페인 없음(_bootstrap)'(측정 시점 ACTIVE 해소 · repeat_axis)) · gemma26b-ctx-single · main |
| 발행 모드 | publication-replay |
| 발행 토픽 | gemma26b_ctx_single |
| work-manifest | docs/_evidence/gemma26b_ctx_single.work-manifest.json |
| 증거 등급(task_class) | full_benchmark |
| 승인 출처 | cli:--approved-by |
| 승인 시각(UTC) | 2026-10-25T04:00:00Z |
| 승인 발화(전사) | 사용자(발화 요지): 5셀 hint 발행 제안(Qwen FP8·NVFP4 멀티, Solar 멀티, Gemma 싱글, Qwen NVFP4 싱글 mmap)에 '오케이 힌트 태그도 발행해줘' — 태그 push 는 발행 직전 재확인 |
| 현행 full 정의 충족(측정 등급) | 예 · 이 측정의 반복 3 · 정의 출처 `.claude/rules/docs.md:42`(03 §3.5) |
| 독립 사실 검증(발행 전) | 통과 — 검증자 `독립 사실 검증 에이전트`(저작자 `hint 저작 에이전트` 아님) · 주장 298건 · 지적 12건 중 고침 12건 · 열림 0 · 2026-10-25T14:57:53Z |
<!-- /FACT:meta -->

<!-- FACT:missing -->
**결손** — 발행을 막지 않고 기재한다(부재와 실패는 다른 사실이다 · 차단은 양성 검출만).

| 코드 | 뜻 | 이 셀의 사유 |
|---|---|---|
| `HINT_MISSING_CAMPAIGN_INSTANCE` | 캠페인 인스턴스를 읽지 않았다(발행 기록 재생 — purge · 캠페인 밖 측정 · 보존됐으나 재생 모드라 읽지 않음 중 어느 쪽인지는 이 셀의 사유) — 셀 lockset·config·cell.status·journey·grounding 이 이 페이로드에 없다. 값의 지위와 여정 한 줄은 문서(testlog·devlog) 서사에서만 복원된다. | 캠페인 밖 측정 — purge 된 인스턴스가 아니라 이 측정의 인스턴스는 애초에 없다(output/single/benchlog/sweep_gemma26b-ctx-single/sweep_index.json bench_knobs.campaign = null · campaign_pins_note '활성 캠페인 없음(_bootstrap)'(측정 시점 ACTIVE 해소 · repeat_axis)) · 같은 셀의 앞선 캠페인 인스턴스 campaigns/camp-26102401-4model-ctx/cells/gemma26b-ctx(journey 마지막 sub_control_echo) 는 보존돼 있으나 이 측정의 것이 아니라 읽지 않았다 |
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
**확정 이름** `hint/0.31.0/gemma-4-26b-a4b-it/gb10-1g1n-main-native/qbf16-len262144-kvfp8` — 꼬리 0토큰.

_빈 꼬리 — 발행 Agent 가 가르는 노브가 없다고 명시했다(`inputs/tail.json` = `[]`)._
<!-- /FACT:name_tail -->

> 이 절이 답하는 질문: 이 셀을 같은 q·len·kv 의 다른 셀과 가르는 노브는 무엇이며 그것을 이름 꼬리로 어떻게 적는가 — 가르는 노브가 없으면 왜 빈 꼬리인가?

빈 꼬리(`inputs/tail.json` = `[]`)다 — 코드 브랜치 카탈로그(`HINTS.md`)에 이 모델의 태그가 없어 결정론부가 같은 다른 셀이 없고, 가를 대상이 없다. 후보 `plenone`(PLE 키 부재)과 `specoff`(speculative-config 부재)는 노브가 아니라 부재 관측이고, `graph`(enforce-eager 부재 → cudagraph)는 엔진 기본값이라 이 셀이 고른 구분 노브가 아니다.
